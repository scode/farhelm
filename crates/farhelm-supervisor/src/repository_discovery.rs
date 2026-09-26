//! Bounded, local discovery of GitHub repository identities beneath one root.
//!
//! This module is deliberately an observation seam, not working-copy
//! ownership or launch preparation. It reads only immediate children that
//! prove they own a `.git` directory or gitfile, asks Git for their local
//! origin without includes, and returns parser-validated identities. It does
//! not recurse, fetch, contact GitHub, run hooks, or retain a remote URL.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use farhelm_proto::{GithubRepo, parse_github_repo};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::{Instant, timeout_at};

use crate::bounded_command::{Ended, GroupChild, OutputCaps};
use crate::working_copies::ARCHIVE_DIR_NAME;

/// Maximum immediate entries examined during one discovery request.
pub const ENTRY_CAP: usize = 1024;
/// Maximum validated suggestions returned from one discovery request.
pub const REPOSITORY_CAP: usize = 100;
/// Maximum JSON encoding of a discovery response, including its status flag.
pub const SERIALIZED_RESULT_CAP: usize = 64 * 1024;
/// Maximum stdout captured from one Git child.
pub const GIT_STDOUT_CAP: usize = 4096;

const DEFAULT_COMMAND_DEADLINE: Duration = Duration::from_secs(1);
const DEFAULT_SCAN_DEADLINE: Duration = Duration::from_secs(5);

/// Repository identities found for a query, plus whether bounded observation
/// ended before the root could be fully represented.
///
/// `truncated` means callers must not treat absence as evidence that a
/// repository is unavailable. The list itself remains safe to offer: every
/// item has passed the shared GitHub-pair parser and is sorted by canonical
/// `owner/repo` spelling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryResult {
    pub repos: Vec<GithubRepo>,
    pub truncated: bool,
}

/// Child-process limits kept injectable so tests can make timeout boundaries
/// observable without weakening production limits.
#[derive(Clone, Copy, Debug)]
pub struct DiscoveryLimits {
    command_deadline: Duration,
    scan_deadline: Duration,
}

impl DiscoveryLimits {
    /// Production limits from Design F: one second per Git command and five
    /// seconds for the entire request, including semaphore contention.
    pub const fn production() -> Self {
        Self {
            command_deadline: DEFAULT_COMMAND_DEADLINE,
            scan_deadline: DEFAULT_SCAN_DEADLINE,
        }
    }

    #[cfg(test)]
    /// Short limits for owned test children. These remain private to tests so
    /// production callers cannot accidentally relax the discovery budget.
    const fn for_test(command_deadline: Duration, scan_deadline: Duration) -> Self {
        Self {
            command_deadline,
            scan_deadline,
        }
    }
}

/// Reusable scanner state. Clones share exactly two child-process permits.
///
/// The executable and added environment are constructor inputs so tests can
/// use a real, owned fake-Git process. Production callers use [`Self::new`],
/// which inherits normal Git lookup while removing Git redirection variables
/// from every inspection child.
#[derive(Clone)]
pub struct RepositoryScanner {
    git: PathBuf,
    child_env: Arc<Vec<(OsString, OsString)>>,
    limits: DiscoveryLimits,
    permits: Arc<Semaphore>,
    #[cfg(test)]
    cleanup_gate: Option<Arc<CleanupGate>>,
}

impl RepositoryScanner {
    /// Create a production scanner whose clones share a two-child budget.
    pub fn new(git: impl Into<PathBuf>) -> Self {
        Self::with_options(git, Vec::new(), DiscoveryLimits::production())
    }

    /// Build a scanner with an executable path and child-only environment.
    ///
    /// This is public because callers may resolve a particular Git binary on
    /// the target host. `child_env` augments that child's environment only; it
    /// never mutates the supervisor process and cannot restore stripped Git
    /// redirection variables.
    pub fn with_options(
        git: impl Into<PathBuf>,
        child_env: Vec<(OsString, OsString)>,
        limits: DiscoveryLimits,
    ) -> Self {
        Self {
            git: git.into(),
            child_env: Arc::new(child_env),
            limits,
            permits: Arc::new(Semaphore::new(2)),
            #[cfg(test)]
            cleanup_gate: None,
        }
    }

    /// Discover matching local origins under an already resolved root.
    ///
    /// A missing Git executable is an error even for an empty root. Other
    /// filesystem or child inspection failures preserve already validated
    /// suggestions but set [`DiscoveryResult::truncated`]. Dropping this
    /// future kills and reaps started children through [`ChildCleanup`] before
    /// their shared permits become available to another scan.
    pub async fn scan(&self, root: &Path, query: &str) -> Result<DiscoveryResult> {
        let deadline = Instant::now() + self.limits.scan_deadline;
        self.ensure_git_available(deadline).await?;

        let mut repos = BTreeMap::new();
        let mut truncated = false;
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(_) => return Ok(self.result(repos, true)),
        };

        let mut examined_entries = 0;
        // `take` refuses to call `next` once the bounded iterator has yielded
        // its limit. A top-of-loop guard would already have read entry 1025.
        for entry in bounded_entries(entries) {
            examined_entries += 1;
            if Instant::now() >= deadline {
                truncated = true;
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    truncated = true;
                    continue;
                }
            };
            let path = entry.path();
            if entry.file_name() == OsStr::new(ARCHIVE_DIR_NAME) {
                continue;
            }
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(_) => {
                    truncated = true;
                    continue;
                }
            };
            if kind.is_symlink()
                || !kind.is_dir()
                || !has_local_git_dir_or_file(&path, &mut truncated)
            {
                continue;
            }

            match self.read_origin(&path, deadline).await {
                Ok(CommandResult::Value(stdout)) => {
                    if let Some(repo) = parse_origin(&stdout)
                        && query_matches(&repo, query)
                    {
                        repos.insert(repo_key(&repo), repo);
                        if repos.len() > REPOSITORY_CAP
                            || self.result_is_too_large(&repos, truncated)
                        {
                            repos.pop_last();
                            truncated = true;
                            break;
                        }
                    }
                }
                Ok(CommandResult::MissingConfig) => {}
                Ok(CommandResult::Incomplete) | Err(_) => truncated = true,
            }
        }

        // Without a 1025th lookahead, a root at the cap may have more work
        // behind it. The contract chooses this conservative status over an
        // unbounded directory probe merely to distinguish exact-cap roots.
        if examined_entries == ENTRY_CAP {
            truncated = true;
        }

        Ok(self.result(repos, truncated))
    }

    /// Prove the configured executable can start before directory enumeration
    /// so an empty root cannot be confused with a Git-less target.
    async fn ensure_git_available(&self, deadline: Instant) -> Result<()> {
        match self.run_git(&[OsStr::new("--version")], deadline).await {
            Ok(CommandResult::Value(_)) => Ok(()),
            Ok(_) => anyhow::bail!("Git is unavailable for repository discovery"),
            Err(error) => Err(error).context("Git is unavailable for repository discovery"),
        }
    }

    /// Read one candidate's local origin without permitting repository config
    /// includes or inherited Git directory/config overrides.
    async fn read_origin(&self, child: &Path, deadline: Instant) -> Result<CommandResult> {
        self.run_git(
            &[
                OsStr::new("-C"),
                child.as_os_str(),
                OsStr::new("config"),
                OsStr::new("--local"),
                OsStr::new("--no-includes"),
                OsStr::new("--get"),
                OsStr::new("remote.origin.url"),
            ],
            deadline,
        )
        .await
    }

    /// Run one bounded Git command, retaining its permit until reaping ends.
    async fn run_git(&self, args: &[&OsStr], deadline: Instant) -> Result<CommandResult> {
        let permit = timeout_at(deadline, Arc::clone(&self.permits).acquire_owned())
            .await
            .map_err(|_| anyhow::anyhow!("repository discovery deadline elapsed waiting for Git"))?
            .map_err(|_| anyhow::anyhow!("repository discovery scanner was shut down"))?;
        let mut command = tokio::process::Command::new(&self.git);
        command.args(args);
        command.env_remove("GIT_DIR").env_remove("GIT_WORK_TREE");
        command.env_remove("GIT_COMMON_DIR");
        for (name, _) in std::env::vars_os()
            .filter(|(name, _)| name.to_string_lossy().starts_with("GIT_CONFIG_"))
        {
            command.env_remove(name);
        }
        for (name, value) in self.child_env.iter() {
            if !name.to_string_lossy().starts_with("GIT_CONFIG_")
                && name != OsStr::new("GIT_DIR")
                && name != OsStr::new("GIT_WORK_TREE")
                && name != OsStr::new("GIT_COMMON_DIR")
            {
                command.env(name, value);
            }
        }
        // Its own process group, stdout capped one byte past the limit, and
        // stderr never read: see `crate::bounded_command`. A Git that forks
        // (a credential or config helper) is killed along with everything it
        // started when the deadline or the cap ends the command.
        let caps = OutputCaps {
            stdout: GIT_STDOUT_CAP,
            stderr: None,
        };
        let mut child = ChildCleanup::new(
            GroupChild::spawn(&mut command, caps).context("spawn Git discovery child")?,
            permit,
        );
        #[cfg(test)]
        {
            child.before_reap = self.cleanup_gate.clone();
        }
        let command_deadline =
            std::cmp::min(deadline, Instant::now() + self.limits.command_deadline);
        let ended = child
            .child
            .as_mut()
            .expect("new cleanup owns the spawned child")
            .finish(command_deadline, caps)
            .await;
        let output = match ended {
            Ok(Ended::Exited(output)) => {
                child.child.take();
                child.disarm();
                output
            }
            // Killed and reaped by `finish`: the slot is free.
            Ok(Ended::TimedOut | Ended::OutputOverCap) => {
                child.child.take();
                child.disarm();
                return Ok(CommandResult::Incomplete);
            }
            // A read or wait failure. The group was signalled; reap it here,
            // or leave the owner armed for Drop's retry if even that fails.
            Err(_) => return Ok(child.kill_reap().await),
        };
        if output.status.success() {
            return Ok(CommandResult::Value(output.stdout));
        }
        if output.status.code() == Some(1) {
            return Ok(CommandResult::MissingConfig);
        }
        Ok(CommandResult::Incomplete)
    }

    /// Preserve canonical ordering while discarding internal deduplication keys.
    fn result(&self, repos: BTreeMap<String, GithubRepo>, truncated: bool) -> DiscoveryResult {
        DiscoveryResult {
            repos: repos.into_values().collect(),
            truncated,
        }
    }

    /// Bound the complete serialized payload rather than only repository text.
    /// This remains a separate guard if the shared identifier limits grow.
    fn result_is_too_large(&self, repos: &BTreeMap<String, GithubRepo>, truncated: bool) -> bool {
        serde_json::to_vec(&serde_json::json!({
            "repos": repos.values().collect::<Vec<_>>(),
            "truncated": truncated,
        }))
        .map_or(true, |encoded| encoded.len() > SERIALIZED_RESULT_CAP)
    }
}

/// The only child outcomes which may be interpreted without exposing stderr.
enum CommandResult {
    Value(Vec<u8>),
    MissingConfig,
    Incomplete,
}

/// Owns a spawned child and its semaphore permit until `wait` has reaped it.
///
/// `Drop` cannot await. It transfers both resources to a small owned cleanup
/// task instead, preventing a cancelled scan from detaching a process or
/// freeing a permit while that process still consumes the two-child budget.
struct ChildCleanup {
    child: Option<GroupChild>,
    permit: Option<OwnedSemaphorePermit>,
    #[cfg(test)]
    before_reap: Option<Arc<CleanupGate>>,
}

/// Park cancelled-child cleanup before wait so a test can distinguish a
/// retained process permit from one released as soon as cancellation occurs.
#[cfg(test)]
#[derive(Default)]
struct CleanupGate {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

impl ChildCleanup {
    /// Transfer exclusive child and budget ownership into cancellation cleanup.
    fn new(child: GroupChild, permit: OwnedSemaphorePermit) -> Self {
        Self {
            child: Some(child),
            permit: Some(permit),
            #[cfg(test)]
            before_reap: None,
        }
    }

    /// A timeout is incomplete discovery. Release its budget only after wait
    /// establishes exit; a wait error leaves the owner armed for Drop's retry.
    async fn kill_reap(&mut self) -> CommandResult {
        let child = self.child.as_mut().expect("live cleanup owns one child");
        if child.kill_and_reap().await.is_ok() {
            self.child.take();
            self.disarm();
        }
        CommandResult::Incomplete
    }

    /// Release the budget after the caller has established the child exited.
    fn disarm(&mut self) {
        self.permit.take();
    }
}

impl Drop for ChildCleanup {
    fn drop(&mut self) {
        let Some(permit) = self.permit.take() else {
            return;
        };
        let Some(mut child) = self.child.take() else {
            drop(permit);
            return;
        };
        child.kill_group();
        #[cfg(test)]
        let gate = self.before_reap.take();
        tokio::spawn(async move {
            #[cfg(test)]
            if let Some(gate) = gate {
                gate.entered.notify_one();
                gate.release.notified().await;
            }
            match child.kill_and_reap().await {
                Ok(_) => drop(permit),
                Err(_) => {
                    // Unknown exit cannot free a slot: another scan would
                    // otherwise exceed the advertised live-child bound. Losing
                    // this slot fails closed for the scanner's remaining life.
                    permit.forget();
                    tracing::warn!(
                        "repository discovery could not reap a child; its process slot remains unavailable"
                    );
                }
            }
        });
    }
}

/// Limit calls into the directory iterator itself, including error entries.
/// A loop-body check would consume one extra entry before it could refuse it.
fn bounded_entries<I: Iterator>(entries: I) -> impl Iterator<Item = I::Item> {
    entries.take(ENTRY_CAP)
}

/// Require a real candidate-local Git marker so Git cannot inherit the root's
/// origin for an ordinary subdirectory.
fn has_local_git_dir_or_file(child: &Path, truncated: &mut bool) -> bool {
    match std::fs::symlink_metadata(child.join(".git")) {
        Ok(metadata) => metadata.is_dir() || metadata.is_file(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(_) => {
            *truncated = true;
            false
        }
    }
}

/// Parse only the three supported remote spellings and return no raw origin.
fn parse_origin(stdout: &[u8]) -> Option<GithubRepo> {
    let origin = std::str::from_utf8(stdout)
        .ok()?
        .trim_end_matches(['\n', '\r']);
    if origin.is_empty() || origin.contains(['\n', '\r', '?', '#']) {
        return None;
    }
    let pair = origin
        .strip_prefix("https://github.com/")
        .or_else(|| origin.strip_prefix("git@github.com:"))
        .or_else(|| origin.strip_prefix("ssh://git@github.com/"))?;
    let pair = pair.strip_suffix(".git").unwrap_or(pair);
    if pair.contains(['@', ':']) || pair.split('/').count() != 2 {
        return None;
    }
    parse_github_repo(pair).ok()
}

/// Match canonical identities without making query spelling part of identity.
fn query_matches(repo: &GithubRepo, query: &str) -> bool {
    repo_key(repo).contains(&query.to_ascii_lowercase())
}

/// Render the one canonical identity spelling used for sort, deduplication,
/// and query matching. `GithubRepo` intentionally has no `Display` because
/// its wire shape remains structured; discovery needs this local string key.
fn repo_key(repo: &GithubRepo) -> String {
    format!("{}/{}", repo.owner, repo.name)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::process::Command;

    use super::*;

    /// A real repository's local config is the contract this scanner reads:
    /// accepted transports become canonical suggestions, while unsafe origins
    /// never escape as a raw URL or a partial identity.
    #[farhelm_testtrace::test]
    async fn real_immediate_repositories_are_validated_sorted_and_deduplicated() {
        let fixture = tempfile::tempdir().unwrap();
        init_repo(fixture.path(), "https", "https://github.com/Acme/One.git");
        init_repo(fixture.path(), "scp", "git@github.com:acme/two.git");
        init_repo(fixture.path(), "ssh", "ssh://git@github.com/acme/three.git");
        init_repo(
            fixture.path(),
            "other-host",
            "https://example.test/acme/nope.git",
        );
        init_repo(
            fixture.path(),
            "credential",
            "https://user:secret@github.com/acme/nope.git",
        );
        init_repo(
            fixture.path(),
            "duplicate",
            "https://github.com/acme/one.git",
        );

        let result = RepositoryScanner::new("git")
            .scan(fixture.path(), "ACME")
            .await
            .unwrap();
        assert!(!result.truncated);
        assert_eq!(repo_names(&result), ["acme/one", "acme/three", "acme/two"]);
    }

    /// A child needs its own real `.git` directory or gitfile; otherwise Git's
    /// `-C` lookup reports the root's origin and turns ordinary folders into
    /// suggestions. The separate-Git-dir fixture proves a gitfile works
    /// through Git itself rather than through the fake executable.
    #[farhelm_testtrace::test]
    async fn local_git_markers_include_gitfiles_but_exclude_parent_and_symlink_inheritance() {
        let fixture = tempfile::tempdir().unwrap();
        run_git(fixture.path(), ["init", "."]);
        run_git(
            fixture.path(),
            [
                "remote",
                "add",
                "origin",
                "https://github.com/acme/root.git",
            ],
        );
        fs::create_dir(fixture.path().join("ordinary-child")).unwrap();
        let gitdir = fixture.path().join("separate-git-dir");
        run_git(
            fixture.path(),
            [
                "init",
                "--separate-git-dir",
                gitdir.to_str().unwrap(),
                "gitfile",
            ],
        );
        let gitfile = fixture.path().join("gitfile");
        run_git(
            &gitfile,
            [
                "remote",
                "add",
                "origin",
                "https://github.com/acme/gitfile.git",
            ],
        );
        assert!(gitfile.join(".git").is_file());
        let linked = fixture.path().join("linked");
        symlink(&gitfile, &linked).unwrap();
        let marker_symlink = fixture.path().join("marker-symlink");
        fs::create_dir(&marker_symlink).unwrap();
        symlink(gitfile.join(".git"), marker_symlink.join(".git")).unwrap();
        fs::create_dir(fixture.path().join(ARCHIVE_DIR_NAME)).unwrap();

        let result = RepositoryScanner::new("git")
            .scan(fixture.path(), "")
            .await
            .unwrap();
        assert_eq!(repo_names(&result), ["acme/gitfile"]);
        assert!(!result.truncated);
    }

    /// Includes could redirect discovery through configuration outside the
    /// candidate. `--no-includes` must leave that candidate suggestion-free.
    #[farhelm_testtrace::test]
    async fn local_config_includes_are_not_followed() {
        let fixture = tempfile::tempdir().unwrap();
        let repo = fixture.path().join("included");
        run_git(fixture.path(), ["init", "included"]);
        let included = fixture.path().join("origin.conf");
        fs::write(
            &included,
            "[remote \"origin\"]\nurl = https://github.com/acme/hidden.git\n",
        )
        .unwrap();
        run_git(
            &repo,
            ["config", "include.path", included.to_str().unwrap()],
        );

        let result = RepositoryScanner::new("git")
            .scan(fixture.path(), "")
            .await
            .unwrap();
        assert!(result.repos.is_empty());
        assert!(!result.truncated);
    }

    /// Oversized and hanging children are not merely ignored: both cases mark
    /// the list incomplete, and the hanging child's owned PID is gone before
    /// the scanner returns.
    #[farhelm_testtrace::test]
    async fn bounded_children_truncate_and_reap_on_timeout() {
        let fixture = tempfile::tempdir().unwrap();
        candidate(fixture.path(), "candidate");
        let fake = fake_git(fixture.path());
        let oversized = scanner_with(
            fake.clone(),
            vec![("FAKE_MODE", "oversized")],
            DiscoveryLimits::for_test(Duration::from_millis(200), Duration::from_secs(1)),
        );
        assert!(oversized.scan(fixture.path(), "").await.unwrap().truncated);

        let pid_file = fixture.path().join("pid");
        let ready = fixture.path().join("ready");
        let hanging = scanner_with(
            fake,
            vec![
                ("FAKE_MODE", "hang"),
                ("PID_FILE", pid_file.to_str().unwrap()),
                ("READY_FILE", ready.to_str().unwrap()),
            ],
            DiscoveryLimits::for_test(Duration::from_millis(40), Duration::from_millis(300)),
        );
        let result = hanging.scan(fixture.path(), "").await.unwrap();
        assert!(result.truncated);
        let pid: i32 = fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_pid_is_gone(pid).await;
    }

    /// Cancellation takes the same cleanup path as a timeout. The test waits
    /// for the fake Git readiness oracle before aborting, then proves its PID
    /// was reaped rather than relying on the abort call or elapsed time.
    #[farhelm_testtrace::test]
    async fn dropping_a_scan_kills_and_reaps_its_owned_child() {
        let fixture = tempfile::tempdir().unwrap();
        candidate(fixture.path(), "candidate");
        let pid_file = fixture.path().join("pid");
        let ready = fixture.path().join("ready");
        let scanner = scanner_with(
            fake_git(fixture.path()),
            vec![
                ("FAKE_MODE", "hang"),
                ("PID_FILE", pid_file.to_str().unwrap()),
                ("READY_FILE", ready.to_str().unwrap()),
            ],
            DiscoveryLimits::for_test(Duration::from_secs(2), Duration::from_secs(3)),
        );
        let task = tokio::spawn({
            let root = fixture.path().to_owned();
            async move { scanner.scan(&root, "").await }
        });
        wait_for_path(&ready).await;
        task.abort();
        let _ = task.await;
        let pid: i32 = fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_pid_is_gone(pid).await;
    }

    /// Cancellation cannot make the child slot reusable before wait reaps it.
    /// Park cleanup at that exact boundary, consume the other slot, observe
    /// Pending on the next acquisition, then release cleanup and prove both
    /// process disappearance and budget reuse. No scheduling delay is an oracle.
    #[farhelm_testtrace::test]
    async fn cancellation_retains_its_permit_until_reap() {
        let fixture = tempfile::tempdir().unwrap();
        candidate(fixture.path(), "candidate");
        let pid_file = fixture.path().join("pid");
        let ready = fixture.path().join("ready");
        let gate = Arc::new(CleanupGate::default());
        let mut scanner = scanner_with(
            fake_git(fixture.path()),
            vec![
                ("FAKE_MODE", "hang"),
                ("PID_FILE", pid_file.to_str().unwrap()),
                ("READY_FILE", ready.to_str().unwrap()),
            ],
            DiscoveryLimits::for_test(Duration::from_secs(5), Duration::from_secs(10)),
        );
        scanner.cleanup_gate = Some(gate.clone());
        let task = tokio::spawn({
            let scanner = scanner.clone();
            let root = fixture.path().to_owned();
            async move { scanner.scan(&root, "").await }
        });
        wait_for_path(&ready).await;
        let pid: i32 = fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(
            unsafe { libc::kill(pid, 0) },
            0,
            "owned child must be alive before cancellation"
        );
        assert_eq!(scanner.permits.available_permits(), 1);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        timeout_at(
            Instant::now() + Duration::from_secs(2),
            gate.entered.notified(),
        )
        .await
        .expect("cancelled child reaches pre-reap boundary");
        let other = scanner
            .permits
            .clone()
            .try_acquire_owned()
            .expect("one unused slot");
        let pending = scanner.permits.clone().acquire_owned();
        tokio::pin!(pending);
        let was_pending = std::future::poll_fn(|cx| {
            std::task::Poll::Ready(std::future::Future::poll(pending.as_mut(), cx).is_pending())
        })
        .await;
        // Always unblock owned cleanup before asserting the observation, so
        // an early-permit-release regression does not leave a parked reaper.
        gate.release.notify_one();
        assert!(
            was_pending,
            "cancelled child's slot must remain held before reap"
        );
        let reused = timeout_at(Instant::now() + Duration::from_secs(2), &mut pending)
            .await
            .expect("reaping releases the slot")
            .unwrap();
        assert_pid_is_gone(pid).await;
        drop((other, reused));
        assert_eq!(scanner.permits.available_permits(), 2);
    }

    /// Git availability is probed before enumeration, so an empty root does
    /// not turn a missing executable into a misleading complete empty list.
    #[farhelm_testtrace::test]
    async fn missing_git_is_an_error_even_for_an_empty_root() {
        let fixture = tempfile::tempdir().unwrap();
        let error = RepositoryScanner::new(fixture.path().join("missing-git"))
            .scan(fixture.path(), "")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Git is unavailable"));
    }

    /// The exact entry boundary is conservatively incomplete without a
    /// lookahead. Keep the fake executable outside the scanned root so it
    /// cannot silently turn this into a 1,025-entry fixture.
    #[farhelm_testtrace::test]
    async fn an_exact_entry_cap_is_conservatively_truncated_without_extra_inspection() {
        let fixture = tempfile::tempdir().unwrap();
        let executable = tempfile::tempdir().unwrap();
        for index in 0..ENTRY_CAP {
            fs::create_dir(fixture.path().join(format!("plain-{index}"))).unwrap();
        }
        let result = scanner_with(
            fake_git(executable.path()),
            vec![("FAKE_ORIGIN", "https://github.com/acme/unused.git")],
            DiscoveryLimits::for_test(Duration::from_millis(200), Duration::from_secs(1)),
        )
        .scan(fixture.path(), "")
        .await
        .unwrap();
        assert!(result.repos.is_empty());
        assert!(result.truncated);
    }

    /// Count actual requests to an unbounded source, not yielded values after
    /// a loop-body guard. Enumeration errors also consume the inspection
    /// budget, so a faulty filesystem cannot turn the cap into an endless scan.
    #[test]
    fn entry_budget_never_requests_a_lookahead() {
        let calls = std::cell::Cell::new(0);
        let entries = std::iter::from_fn(|| {
            calls.set(calls.get() + 1);
            Some(Err::<(), _>(io::Error::other("fixture entry error")))
        });
        assert_eq!(bounded_entries(entries).count(), ENTRY_CAP);
        assert_eq!(calls.get(), ENTRY_CAP);
    }

    /// A full-sized invalid origin is still a complete observation, whereas
    /// one extra byte must mark the scan incomplete. This distinguishes an
    /// output-size refusal from ordinary parser rejection.
    #[farhelm_testtrace::test]
    async fn stdout_limit_accepts_4096_bytes_and_refuses_4097() {
        let fixture = tempfile::tempdir().unwrap();
        candidate(fixture.path(), "candidate");
        let fake = fake_git(fixture.path());
        for (size, truncated) in [("4096", false), ("4097", true)] {
            let result = scanner_with(
                fake.clone(),
                vec![("FAKE_MODE", "sized"), ("FAKE_SIZE", size)],
                DiscoveryLimits::production(),
            )
            .scan(fixture.path(), "")
            .await
            .unwrap();
            assert!(result.repos.is_empty());
            assert_eq!(result.truncated, truncated, "stdout bytes: {size}");
        }
    }

    /// Child-only injection must not restore the Git overrides stripped by
    /// inspection. The fake reports a different valid identity if any leaks,
    /// making the returned repository the environment-isolation oracle.
    #[farhelm_testtrace::test]
    async fn child_git_directory_and_config_overrides_are_stripped() {
        let fixture = tempfile::tempdir().unwrap();
        candidate(fixture.path(), "candidate");
        let result = scanner_with(
            fake_git(fixture.path()),
            vec![
                ("FAKE_MODE", "environment"),
                ("GIT_DIR", "/foreign"),
                ("GIT_WORK_TREE", "/foreign"),
                ("GIT_COMMON_DIR", "/foreign"),
                ("GIT_CONFIG_COUNT", "1"),
                ("GIT_CONFIG_KEY_0", "remote.origin.url"),
                ("GIT_CONFIG_VALUE_0", "https://github.com/foreign/repo.git"),
                ("GIT_CONFIG_PARAMETERS", "hostile"),
            ],
            DiscoveryLimits::production(),
        )
        .scan(fixture.path(), "")
        .await
        .unwrap();
        assert_eq!(repo_names(&result), ["acme/clean"]);
        assert!(!result.truncated);
    }

    /// Exit 1 is Git's normal missing-key result; other status failures leave
    /// discovery incomplete. The result cannot collapse those cases because a
    /// corrupt local config is not evidence that the candidate lacks origin.
    #[farhelm_testtrace::test]
    async fn a_missing_origin_is_complete_but_other_git_failures_truncate() {
        let fixture = tempfile::tempdir().unwrap();
        candidate(fixture.path(), "candidate");
        let missing = scanner_with(
            fake_git(fixture.path()),
            vec![("FAKE_MODE", "exit-one")],
            DiscoveryLimits::for_test(Duration::from_millis(200), Duration::from_secs(1)),
        )
        .scan(fixture.path(), "")
        .await
        .unwrap();
        assert!(missing.repos.is_empty());
        assert!(!missing.truncated);
        let failed = scanner_with(
            fake_git(fixture.path()),
            vec![("FAKE_MODE", "exit-two")],
            DiscoveryLimits::for_test(Duration::from_millis(200), Duration::from_secs(1)),
        )
        .scan(fixture.path(), "")
        .await
        .unwrap();
        assert!(failed.truncated);
    }

    /// Discovery must make its result cap visible instead of returning a
    /// complete-looking prefix. The fake derives each valid identity from an
    /// owned candidate directory, so the test exercises normal spawning and
    /// parser validation for all 101 distinct origins.
    #[farhelm_testtrace::test]
    async fn more_than_one_hundred_distinct_matches_are_truncated() {
        let fixture = tempfile::tempdir().unwrap();
        for index in 0..=REPOSITORY_CAP {
            candidate(fixture.path(), &format!("repo{index}"));
        }
        let result = scanner_with(
            fake_git(fixture.path()),
            vec![("FAKE_MODE", "by-directory")],
            DiscoveryLimits::for_test(Duration::from_millis(200), Duration::from_secs(5)),
        )
        .scan(fixture.path(), "")
        .await
        .unwrap();
        assert_eq!(result.repos.len(), REPOSITORY_CAP);
        assert!(result.truncated);
        let encoded = serde_json::to_vec(&serde_json::json!({
            "repos": result.repos, "truncated": result.truncated,
        }))
        .unwrap();
        assert!(encoded.len() <= SERIALIZED_RESULT_CAP);
    }

    /// The identifier and count limits currently keep even maximum-sized
    /// accepted identities below 64 KiB. Pin the actual encoded payload rather
    /// than assuming the repository count alone bounds future wire changes.
    #[test]
    fn maximum_valid_repository_list_fits_the_serialized_budget() {
        let scanner = RepositoryScanner::new("git");
        let owner = "a".repeat(39);
        let repos: BTreeMap<_, _> = (0..REPOSITORY_CAP)
            .map(|index| {
                let name = format!("{}{:03}", "r".repeat(97), index);
                let repo = parse_github_repo(&format!("{owner}/{name}"))
                    .expect("maximum valid identifier");
                (repo_key(&repo), repo)
            })
            .collect();
        assert_eq!(repos.len(), REPOSITORY_CAP);
        assert!(!scanner.result_is_too_large(&repos, true));
        let result = scanner.result(repos, true);
        let encoded = serde_json::to_vec(&serde_json::json!({
            "repos": result.repos, "truncated": result.truncated,
        }))
        .unwrap();
        assert!(encoded.len() <= SERIALIZED_RESULT_CAP);
    }

    /// Two concurrent scans may own two live children, while a third waits
    /// under its own overall deadline. The readiness directory is an oracle
    /// for actual spawned children; task scheduling alone cannot prove the
    /// semaphore cap.
    #[farhelm_testtrace::test]
    async fn shared_scanners_hold_at_most_two_children_and_bound_waiters() {
        let fixture = tempfile::tempdir().unwrap();
        candidate(fixture.path(), "candidate");
        let ready_dir = fixture.path().join("ready");
        fs::create_dir(&ready_dir).unwrap();
        let pids = fixture.path().join("pids");
        let scanner = scanner_with(
            fake_git(fixture.path()),
            vec![
                ("FAKE_MODE", "hang-count"),
                ("PID_FILE", pids.to_str().unwrap()),
                ("READY_DIR", ready_dir.to_str().unwrap()),
            ],
            DiscoveryLimits::for_test(Duration::from_secs(5), Duration::from_secs(10)),
        );
        let root = fixture.path().to_owned();
        let first = tokio::spawn({
            let scanner = scanner.clone();
            let root = root.clone();
            async move { scanner.scan(&root, "").await }
        });
        let second = tokio::spawn({
            let scanner = scanner.clone();
            let root = root.clone();
            async move { scanner.scan(&root, "").await }
        });
        wait_for_entry_count(&ready_dir, 2).await;
        assert_eq!(scanner.permits.available_permits(), 0);
        let owned_pids: Vec<i32> = fs::read_to_string(&pids)
            .unwrap()
            .lines()
            .map(|pid| pid.parse().unwrap())
            .collect();
        assert_eq!(owned_pids.len(), 2);
        for &pid in &owned_pids {
            assert_eq!(
                unsafe { libc::kill(pid, 0) },
                0,
                "each budget holder must still be alive"
            );
        }
        let mut waiter = scanner.clone();
        waiter.limits =
            DiscoveryLimits::for_test(Duration::from_secs(1), Duration::from_millis(40));
        let error = waiter
            .scan(&root, "")
            .await
            .expect_err("third scan must exhaust its permit deadline");
        assert!(format!("{error:#}").contains("deadline elapsed waiting for Git"));
        assert_eq!(fs::read_to_string(&pids).unwrap().lines().count(), 2);
        first.abort();
        second.abort();
        let (first, second) = tokio::join!(first, second);
        assert!(first.unwrap_err().is_cancelled() && second.unwrap_err().is_cancelled());
        for pid in owned_pids {
            assert_pid_is_gone(pid).await;
        }
        assert_eq!(scanner.permits.available_permits(), 2);
    }

    /// Initialize one on-disk Git repository and give it the exact origin the
    /// scanner should observe; test setup must establish that premise itself.
    fn init_repo(root: &Path, name: &str, origin: &str) {
        run_git(root, ["init", name]);
        run_git(&root.join(name), ["remote", "add", "origin", origin]);
    }

    /// Create the minimal candidate-local `.git` directory for fake-Git tests.
    fn candidate(root: &Path, name: &str) {
        fs::create_dir(root.join(name)).unwrap();
        fs::create_dir(root.join(name).join(".git")).unwrap();
    }

    /// Run fixture Git synchronously and fail at setup time when its required
    /// repository state was not actually established.
    fn run_git<const N: usize>(directory: &Path, args: [&str; N]) {
        let status = Command::new("git")
            .current_dir(directory)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "fixture Git command must succeed");
    }

    /// Create an owned Git-shaped executable whose modes exercise the real
    /// spawn/read/kill/reap path without network access or process env edits.
    fn fake_git(root: &Path) -> PathBuf {
        let path = root.join("fake-git");
        fs::write(
            &path,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then echo fake-git; exit 0; fi
case "$FAKE_MODE" in
  sized) head -c "$FAKE_SIZE" /dev/zero ;;
  environment)
    if env | grep -E '^GIT_(DIR|WORK_TREE|COMMON_DIR|CONFIG_[^=]*)=' >/dev/null; then
      echo https://github.com/foreign/repo.git
    else
      echo https://github.com/acme/clean.git
    fi ;;
  oversized) head -c 4097 /dev/zero ;;
  exit-one) exit 1 ;;
  exit-two) exit 2 ;;
  by-directory) printf 'https://github.com/acme/%s.git\n' "$(basename "$2")" ;;
  hang) echo "$$" > "$PID_FILE"; : > "$READY_FILE"; exec /bin/sleep 1000 ;;
  hang-count) echo "$$" >> "$PID_FILE"; mkdir "$READY_DIR/$$"; exec /bin/sleep 1000 ;;
  *) printf '%s\n' "$FAKE_ORIGIN" ;;
esac
"#,
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    /// Give each test a scanner whose injected executable and environment are
    /// confined to spawned inspection children.
    fn scanner_with(
        git: PathBuf,
        env: Vec<(&str, &str)>,
        limits: DiscoveryLimits,
    ) -> RepositoryScanner {
        RepositoryScanner::with_options(
            git,
            env.into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
            limits,
        )
    }

    /// Render structured identities only for readable assertion diagnostics.
    fn repo_names(result: &DiscoveryResult) -> Vec<String> {
        result.repos.iter().map(repo_key).collect()
    }

    /// Await an owned fake child's readiness file before an action that claims
    /// to exercise its live lifetime.
    async fn wait_for_path(path: &Path) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while !path.exists() {
                // sleep-ok: polls an owned fake-Git readiness file before cancellation.
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("owned fake Git must announce readiness");
    }

    /// Prove cleanup terminated the owned child instead of inferring it from
    /// the scanner future's completion.
    async fn assert_pid_is_gone(pid: i32) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while unsafe { libc::kill(pid, 0) } == 0 {
                // sleep-ok: polls the owned fake-Git PID until cleanup reaps it.
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("scanner cleanup must terminate its owned fake Git child");
    }

    /// Await a count of fake-child readiness markers without turning an
    /// elapsed delay into evidence that subprocesses actually started.
    async fn wait_for_entry_count(path: &Path, expected: usize) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while fs::read_dir(path).unwrap().count() < expected {
                // sleep-ok: polls owned child readiness markers for semaphore contention.
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the shared scanner must admit two owned children");
    }
}

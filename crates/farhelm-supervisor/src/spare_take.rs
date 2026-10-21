//! Consume a complete spare inside the shim's existing clone stage.
//!
//! Before the first moved entry, structural uncertainty means ordinary clone.
//! Fetch errors and every error after that boundary instead retain evidence and
//! fail preparation. The supervisor alone removes abandoned claimed trees.

use super::{
    CheckoutPreparation, LaunchSpec, git_preparation_command, parse_launch_file_name,
    run_preparation_child_process,
};
use crate::bounded_command::{Ended, GroupChild, OutputCaps};
use crate::checkout_trash::{entry_metadata, open_child_dir};
use crate::working_copies::{SPARE_DIR_NAME, rename_at_noreplace};
use anyhow::{Context, bail};
use std::ffi::{CStr, CString};
use std::fs::{self, File};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

/// Local inspection is an optimization, unlike visible credential-bearing
/// fetch/merge. Refuse slow or verbose wrappers using discovery's existing
/// one-second/4 KiB boundary; never accept truncated metadata.
const QUERY_CAPS: OutputCaps = OutputCaps {
    stdout: 4096,
    stderr: None,
};
const QUERY_DEADLINE: Duration = Duration::from_secs(1);
/// A normal tree can have many top-level entries, but inspection must not
/// retain an unbounded directory listing before committing to any move.
const ENTRY_CAP: usize = 100_000;

/// Try a take without acquiring the cache lock or reading ownership records.
/// False leaves ordinary clone safe; Err forbids another credential request.
pub(super) fn take(
    spec: &LaunchSpec,
    preparation: &CheckoutPreparation,
    status: &Path,
) -> Result<bool, String> {
    if !preparation.keep_spare {
        return Ok(false);
    }
    // The real shim has no runtime, but async callers/tests may reach this
    // synchronous boundary. Enter the private query reactor on another thread
    // only in that case, rather than nesting Tokio and panicking.
    if tokio::runtime::Handle::try_current().is_ok() {
        return std::thread::scope(|scope| {
            scope
                .spawn(|| take_with_rename(spec, preparation, status, &mut rename_at_noreplace))
                .join()
                .map_err(|_| "spare inspection thread panicked".to_string())?
        });
    }
    take_with_rename(spec, preparation, status, &mut rename_at_noreplace)
}

/// Keep fault injection at the real rename boundary: once a rename succeeds
/// there is no rollback or plain-clone fallback, even if a later rename fails.
pub(super) fn take_with_rename(
    spec: &LaunchSpec,
    preparation: &CheckoutPreparation,
    status: &Path,
    rename: &mut impl FnMut(RawFd, &CStr, RawFd, &CStr) -> std::io::Result<()>,
) -> Result<bool, String> {
    if !preparation.keep_spare {
        return Ok(false);
    }
    let Some(query) = Queries::new(spec) else {
        return Ok(false);
    };
    let Some(version) = query.line(None, &["--version"]) else {
        return Ok(false);
    };
    if !supports_take(&version) {
        return Ok(false);
    }
    let Some(candidate) = Candidate::claim(spec, preparation, status, rename)? else {
        return Ok(false);
    };
    let Some((remote, tracking)) = candidate.git_facts(&query, &preparation.clone_url) else {
        return Ok(false);
    };

    println!("Farhelm: updating a checkout prepared earlier from GitHub.");
    let mut fetch = git_preparation_command(spec);
    fetch.current_dir(&candidate.path).args([
        "-c",
        &format!("remote.{remote}.followRemoteHEAD=always"),
        "fetch",
        "--prune",
        "--prune-tags",
        &remote,
    ]);
    run_preparation_child_process(&mut fetch, "spare fetch")?;

    // A pruned source branch is caught even when old Git leaves HEAD dangling.
    // Use the saved full tracking ref, never the multi-ref FETCH_HEAD file.
    if query
        .line(
            Some(&candidate.path),
            &["rev-parse", "--verify", &format!("{tracking}^{{commit}}")],
        )
        .is_none()
        || query
            .line(
                Some(&candidate.path),
                &[
                    "symbolic-ref",
                    "--quiet",
                    &format!("refs/remotes/{remote}/HEAD"),
                ],
            )
            .as_deref()
            != Some(tracking.as_str())
    {
        return Ok(false);
    }
    candidate
        .verify_destination(preparation)
        .map_err(|e| format!("spare destination: {e:#}"))?;
    let Ok(mut entries) = entries(&candidate.directory) else {
        return Ok(false);
    };
    let Some(git_index) = entries.iter().position(|name| name.as_c_str() == c".git") else {
        return Ok(false);
    };
    entries.swap(0, git_index);
    if candidate.preflight(&entries).is_err() {
        return Ok(false);
    }

    for (index, name) in entries.iter().enumerate() {
        if let Err(error) = rename(
            candidate.directory.as_raw_fd(),
            name,
            candidate.destination.as_raw_fd(),
            name,
        ) {
            if index == 0 {
                // A failed rename on a remote filesystem can have an
                // ambiguous outcome. Clone is safe only if cwd stayed empty.
                candidate.verify_destination(preparation).map_err(|e| {
                    format!("prepared checkout move failed with uncertain destination: {e:#}")
                })?;
                return Ok(false);
            }
            return Err(format!(
                "moving prepared checkout entry after a partial move: {error}"
            ));
        }
    }
    rustix::fs::unlinkat(
        &candidate.owner,
        candidate.name.as_c_str(),
        rustix::fs::AtFlags::REMOVEDIR,
    )
    .map_err(|e| format!("removing empty prepared checkout claim: {e}"))?;
    let mut merge = git_preparation_command(spec);
    merge
        .current_dir(&preparation.cwd)
        .args(["merge", "--ff-only", &tracking]);
    run_preparation_child_process(&mut merge, "spare fast-forward")?;
    Ok(true)
}

/// The flag floor is not a product Git minimum. Unknown/vendor output that
/// cannot prove --prune-tags support keeps the pre-existing clone behavior.
fn supports_take(version: &str) -> bool {
    let Some(version) = version.strip_prefix("git version ") else {
        return false;
    };
    let mut components = version.split('.');
    let Some(major) = components.next().and_then(|n| n.parse::<u64>().ok()) else {
        return false;
    };
    let Some(minor) = components.next().and_then(|n| n.parse::<u64>().ok()) else {
        return false;
    };
    (major, minor) >= (2, 17)
}

/// A private current-thread reactor reuses the existing bounded subprocess
/// implementation. Query failures have no terminal output or partial answers.
struct Queries<'a> {
    runtime: tokio::runtime::Runtime,
    spec: &'a LaunchSpec,
}

impl<'a> Queries<'a> {
    /// Creating the optimization's local reactor may fail without disabling
    /// ordinary clones; no runtime is retained into agent exec.
    fn new(spec: &'a LaunchSpec) -> Option<Self> {
        Some(Self {
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok()?,
            spec,
        })
    }

    /// Bound capture and child lifetime, including descendants holding a pipe.
    /// Stderr is discarded because these are silent local inspections.
    fn run(&self, cwd: Option<&Path>, args: &[&str]) -> Option<Output> {
        let mut command = git_preparation_command(self.spec);
        command.args(args);
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let mut command = tokio::process::Command::from(command);
        self.runtime.block_on(async {
            let mut child = GroupChild::spawn(&mut command, QUERY_CAPS).ok()?;
            let ended = child
                .finish(tokio::time::Instant::now() + QUERY_DEADLINE, QUERY_CAPS)
                .await;
            if !child.is_reaped() {
                child.kill_and_reap().await.ok()?;
            }
            match ended.ok()? {
                Ended::Exited(output) => Some(output),
                Ended::TimedOut | Ended::OutputOverCap => None,
            }
        })
    }

    /// Accept exactly one nonempty output line. Ambiguous wrapper chatter
    /// cannot become a remote, ref or pathname later used by Git.
    fn line(&self, cwd: Option<&Path>, args: &[&str]) -> Option<String> {
        let output = self.run(cwd, args)?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        let line = text.strip_suffix('\n').unwrap_or(&text);
        (!line.is_empty() && !line.contains(['\n', '\r', '\0'])).then(|| line.to_owned())
    }

    /// Clone-owned values must be singular and local. Includes/global config
    /// remain active for actual Git operations and credentials, not these facts.
    fn config(&self, cwd: &Path, key: &str) -> Option<String> {
        self.line(
            Some(cwd),
            &["config", "--local", "--no-includes", "--get-all", key],
        )
    }
}

/// Open handles retain the selected namespace and allocated destination across
/// a possibly slow fetch. This object confers no recursive-delete authority.
struct Candidate {
    owner: File,
    directory: File,
    destination: File,
    name: CString,
    path: PathBuf,
}

impl Candidate {
    /// Claim one existing ready slot in the canonical direct-parent root.
    /// Known unsafe destinations are required errors; unavailable spare
    /// structure declines. Every outcome still precedes content movement.
    fn claim(
        spec: &LaunchSpec,
        preparation: &CheckoutPreparation,
        status: &Path,
        rename: &mut impl FnMut(RawFd, &CStr, RawFd, &CStr) -> std::io::Result<()>,
    ) -> Result<Option<Self>, String> {
        // A positively observed unsafe destination must not be turned into an
        // optional-path decline: plain clone would target that same replacement.
        let cwd = Path::new(&preparation.cwd);
        let current =
            fs::symlink_metadata(cwd).map_err(|e| format!("spare destination observation: {e}"))?;
        if !current.is_dir()
            || (current.dev(), current.ino())
                != (preparation.directory_device, preparation.directory_inode)
        {
            return Err("spare destination: allocated checkout was replaced".to_string());
        }
        let Some(root_path) = cwd.parent() else {
            return Ok(None);
        };
        let opened = (|| -> anyhow::Result<_> {
            if !cwd.is_absolute() || cwd.canonicalize()? != cwd {
                bail!("checkout path is not canonical");
            }
            let root = fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(root_path)?;
            let destination = open_child_dir(
                &root,
                &CString::new(
                    cwd.file_name()
                        .context("checkout child")?
                        .as_encoded_bytes(),
                )?,
                true,
            )?;
            Ok((root, destination))
        })();
        let Ok((root, destination)) = opened else {
            return Ok(None);
        };
        verify_empty(&destination, preparation).map_err(|e| format!("spare destination: {e:#}"))?;
        let candidate = (|| -> anyhow::Result<_> {
            let (session, generation) = parse_launch_file_name(
                status
                    .file_name()
                    .and_then(|n| n.to_str())
                    .context("launch name")?,
            )
            .context("launch generation")?;
            if session != spec.session_id {
                bail!("claim session differs from launch");
            }
            let pair = preparation
                .clone_url
                .strip_prefix("https://github.com/")
                .and_then(|url| url.strip_suffix(".git"))
                .context("clone URL")?;
            let repo = farhelm_proto::github_checkout::parse_github_repo(pair)?;
            let name = crate::spare_checkouts::claim_name(&repo.name, session, generation)
                .context("claim identity")?;
            let reserved = open_child_dir(&root, &CString::new(SPARE_DIR_NAME)?, true)?;
            let owner = open_child_dir(&reserved, &CString::new(repo.owner.as_str())?, true)?;
            rename(
                owner.as_raw_fd(),
                &CString::new(repo.name.as_str())?,
                owner.as_raw_fd(),
                &name,
            )?;
            let directory = open_child_dir(&owner, &name, true)?;
            let path = root_path
                .join(SPARE_DIR_NAME)
                .join(&repo.owner)
                .join(std::ffi::OsStr::from_bytes(name.to_bytes()));
            Ok(Self {
                owner,
                directory,
                destination,
                name,
                path,
            })
        })();
        Ok(candidate.ok())
    }

    /// Accept only a self-contained ordinary clone. Absolute worktree config,
    /// gitfiles/common dirs and alternates do not retain their meaning on move.
    fn git_facts(&self, query: &Queries<'_>, clone_url: &str) -> Option<(String, String)> {
        let _git = open_child_dir(&self.directory, c".git", true).ok()?;
        let git_path = self.path.join(".git").canonicalize().ok()?;
        if query
            .line(Some(&self.path), &["rev-parse", "--is-bare-repository"])
            .as_deref()
            != Some("false")
        {
            return None;
        }
        for argument in ["--git-dir", "--git-common-dir"] {
            let path = query.line(Some(&self.path), &["rev-parse", argument])?;
            if self.path.join(path).canonicalize().ok()? != git_path {
                return None;
            }
        }
        if Path::new(&query.line(Some(&self.path), &["rev-parse", "--show-toplevel"])?)
            .canonicalize()
            .ok()?
            != self.path
        {
            return None;
        }
        let worktree = query.run(Some(&self.path), &["config", "--get-all", "core.worktree"])?;
        if worktree.status.code() != Some(1) || !worktree.stdout.is_empty() {
            return None;
        }
        if self.path.join(".git/commondir").try_exists().ok()? {
            return None;
        }
        match fs::symlink_metadata(self.path.join(".git/objects/info/alternates")) {
            Ok(metadata) if metadata.is_file() && metadata.len() == 0 => {}
            Ok(_) => return None,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return None,
        }
        let branch = query.line(Some(&self.path), &["symbolic-ref", "--quiet", "HEAD"])?;
        let branch = branch.strip_prefix("refs/heads/")?;
        let remote = query.config(&self.path, &format!("branch.{branch}.remote"))?;
        if remote == "." || remote.starts_with('-') {
            return None;
        }
        let merge = query.config(&self.path, &format!("branch.{branch}.merge"))?;
        let branch = merge.strip_prefix("refs/heads/")?;
        if query
            .config(&self.path, &format!("remote.{remote}.url"))
            .as_deref()
            != Some(clone_url)
        {
            return None;
        }
        let tracking = query.line(
            Some(&self.path),
            &[
                "rev-parse",
                "--symbolic-full-name",
                "--verify",
                "@{upstream}",
            ],
        )?;
        (tracking == format!("refs/remotes/{remote}/{branch}")).then_some((remote, tracking))
    }

    /// Recheck the named cwd too: an open old handle alone cannot authorize
    /// Git later entering a replacement at the same pathname.
    fn verify_destination(&self, preparation: &CheckoutPreparation) -> anyhow::Result<()> {
        verify_empty(&self.destination, preparation)?;
        let current = fs::symlink_metadata(&preparation.cwd)?;
        if !current.is_dir()
            || (current.dev(), current.ino())
                != (preparation.directory_device, preparation.directory_inode)
        {
            bail!("allocated checkout was replaced");
        }
        Ok(())
    }

    /// Inspect all entries before moving .git first. The shared observation
    /// rejects file binds as well as directory mounts; ordinary symlinks move
    /// without following their target. No recursive traversal is needed.
    fn preflight(&self, names: &[CString]) -> anyhow::Result<()> {
        let parent = rustix::fs::fstat(&self.directory)?;
        for name in names {
            let metadata = entry_metadata(&self.directory, name)?;
            if metadata.st_dev != parent.st_dev {
                bail!("prepared entry crosses filesystems");
            }
            if rustix::fs::FileType::from_raw_mode(metadata.st_mode)
                == rustix::fs::FileType::Directory
            {
                open_child_dir(&self.directory, name, true)?;
            }
        }
        Ok(())
    }
}

/// Enumerate raw names once, with a bound including the call proving EOF.
/// The caller explicitly selects .git first; other names need no ordering.
fn entries(directory: &File) -> anyhow::Result<Vec<CString>> {
    let mut names = Vec::new();
    let mut iterator = rustix::fs::Dir::read_from(directory)?;
    for _ in 0..ENTRY_CAP {
        let Some(entry) = iterator.next() else {
            return Ok(names);
        };
        let entry = entry?;
        let name = entry.file_name();
        if name != c"." && name != c".." {
            names.push(name.to_owned());
        }
    }
    bail!("prepared checkout has too many top-level entries")
}

/// Allocation identity and emptiness remain mandatory after waiting for fetch.
fn verify_empty(directory: &File, preparation: &CheckoutPreparation) -> anyhow::Result<()> {
    let metadata = directory.metadata()?;
    if (metadata.dev(), metadata.ino())
        != (preparation.directory_device, preparation.directory_inode)
    {
        bail!("allocated checkout identity changed");
    }
    if !entries(directory)?.is_empty() {
        bail!("allocated checkout is not empty");
    }
    Ok(())
}

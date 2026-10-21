//! One login-shell clone under an owner tether and independent wall-clock bound.
//!
//! The supervisor alone owns publication and removal. These synchronous internal
//! commands carry no session authority: the guard owns ordinary same-group signal
//! cleanup, and the clone helper owns only cache-locked Git work. A delayed helper
//! still checks target identity because group signals are not filesystem ordering.

use std::io::Read;
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::spare_checkouts::BuildTarget;

/// Immutable input for one private build. The duration is carried across the
/// exec boundary so deterministic short-deadline tests use the real guard too.
/// Production supplies the remaining part of one ten-minute attempt, including
/// cache admission; the reader refuses a larger value.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BuildSpec {
    pub(crate) cache: PathBuf,
    pub(crate) repo: farhelm_proto::GithubRepo,
    pub(crate) target: BuildTarget,
    pub(crate) shell: String,
    pub(crate) farhelm_exe: PathBuf,
    pub(crate) farhelm_bin_dir: PathBuf,
    pub(crate) deadline_millis: u64,
}

/// Completion is ephemeral and spec-derived. A later supervisor never adopts it
/// as an owed job, so no durability/version/history protocol is appropriate.
pub(crate) fn result_path(spec: &Path) -> PathBuf {
    spec.with_extension("result")
}

/// The helper writes this only after dissociation. Guard promotion then proves
/// rc files did not merely exit zero before executing their supplied command.
pub(crate) fn receipt_path(spec: &Path) -> PathBuf {
    spec.with_extension("receipt")
}

/// Bound private input as well as output; corruption is failure, never a partial
/// specification. Repository validation remains at this process boundary.
fn read_spec(path: &Path) -> anyhow::Result<BuildSpec> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(65_537)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= 65_536, "spare build spec exceeds its limit");
    let spec: BuildSpec = serde_json::from_slice(&bytes)?;
    anyhow::ensure!(
        spec.deadline_millis > 0 && spec.deadline_millis <= 600_000,
        "invalid spare deadline"
    );
    anyhow::ensure!(
        spec.farhelm_exe.is_absolute()
            && spec.farhelm_bin_dir.is_absolute()
            && !spec.shell.is_empty()
            && !spec.shell.contains('\0'),
        "invalid spare executable/shell"
    );
    let repo =
        farhelm_proto::parse_github_repo(&format!("{}/{}", spec.repo.owner, spec.repo.name))?;
    anyhow::ensure!(repo == spec.repo, "invalid spare repository");
    Ok(spec)
}

/// Execute only Git, in the environment established by the user's login shell.
/// The target must already exist; startup removal cannot be undone by this step.
pub fn build_spare(path: &Path) -> anyhow::Result<()> {
    let spec = read_spec(path)?;
    crate::launch::clone_from_repo_cache(
        &spec.cache,
        &spec.repo.clone_url(),
        &spec.target.path(),
        || crate::launch::background_git_command(&spec.farhelm_bin_dir),
        || {
            spec.target
                .verify()
                .map_err(|error| format!("building target: {error:#}"))
        },
    )
    .map_err(anyhow::Error::msg)?;
    crate::files::write_private_file_sync(&receipt_path(path), b"true", &crate::files::RealFs)?;
    Ok(())
}

/// Kill only the group this guard proved it leads. SIGKILL includes the guard,
/// so it cannot return success through its exit status; the private bool does.
fn retire_group(group: i32) -> ! {
    // SAFETY: group was checked against this process's positive pid/pgrp/sid.
    // No PID from a file or another process is accepted here.
    unsafe {
        libc::kill(-group, libc::SIGKILL);
        libc::_exit(1);
    }
}

/// Include user rc files in the tether/deadline, before any cache lock can exist.
/// Ordinary descendants share this group. Explicit session/group escape by user
/// code is outside the process-group contract; this does not reap grandchildren.
pub fn guard_spare_build(path: &Path) -> anyhow::Result<()> {
    // SAFETY: these identity queries do not mutate process state or touch memory.
    let (pid, group, session) = unsafe { (libc::getpid(), libc::getpgrp(), libc::getsid(0)) };
    anyhow::ensure!(
        pid > 0 && pid == group && pid == session,
        "spare guard requires its own detached session"
    );
    let spec = read_spec(path)?;
    for artifact in [receipt_path(path), result_path(path)] {
        match std::fs::symlink_metadata(&artifact) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
            Ok(_) => anyhow::bail!("spare completion name is already occupied"),
        }
    }
    let deadline = std::time::Instant::now() + Duration::from_millis(spec.deadline_millis);
    std::thread::Builder::new()
        .name("spare-owner-tether".into())
        .spawn(move || {
            // Interrupted reads are retried by copy. EOF and every other read error
            // both mean owner authority is gone, even after destructor-bypassing exit.
            let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
            retire_group(group);
        })
        .context("starting spare owner watch")?;
    std::thread::Builder::new()
        .name("spare-deadline".into())
        .spawn(move || {
            // This independent OS thread still runs if the supervisor is gone or
            // suspended, and starts before any user-controlled login initialization.
            std::thread::sleep(deadline.saturating_duration_since(std::time::Instant::now()));
            retire_group(group);
        })
        .context("starting spare deadline watch")?;
    let quote = farhelm_proto::text::shell_quote;
    let script = format!(
        "exec {} internal build-spare {}",
        quote(&spec.farhelm_exe.to_string_lossy()),
        quote(&path.to_string_lossy())
    );
    let completed = crate::launch::background_guard_command(Path::new(&spec.shell))
        .args(["-l", "-i", "-c", &script])
        .stdin(Stdio::null())
        .status();
    let success = completed.as_ref().is_ok_and(|status| status.success());
    if let Err(error) = &completed {
        eprintln!("could not start spare login shell: {error}");
    }
    if let Ok(status) = &completed
        && !status.success()
    {
        eprintln!("spare login/clone failed: {status}");
    }
    if success && let Err(error) = std::fs::hard_link(receipt_path(path), result_path(path)) {
        // Hard-link publication refuses collisions and preserves the receipt's
        // private mode. A successful shell alone is not successful clone proof.
        eprintln!("could not promote spare helper completion: {error}");
    }
    retire_group(group)
}

/// A marker cannot override an outer refusal. Only the expected natural guard
/// retirement plus fresh complete true is permission to consider publication.
pub(crate) fn completed(ended: &crate::bounded_command::Ended, path: &Path) -> bool {
    if !matches!(ended, crate::bounded_command::Ended::Exited(output)
        if output.status.signal() == Some(libc::SIGKILL))
    {
        return false;
    }
    let result = (|| -> std::io::Result<bool> {
        let receipt = std::fs::metadata(receipt_path(path))?;
        let file = std::fs::File::open(result_path(path))?;
        let promoted = file.metadata()?;
        // Same live inode proves hard-link promotion of this helper's receipt,
        // rather than accepting an unrelated file occupying the final name.
        if !receipt.is_file()
            || !promoted.is_file()
            || (receipt.dev(), receipt.ino()) != (promoted.dev(), promoted.ino())
        {
            return Ok(false);
        }
        let mut bytes = Vec::new();
        file.take(5).read_to_end(&mut bytes)?;
        Ok(bytes == b"true")
    })();
    result.unwrap_or(false)
}

/// Startup never adopts old jobs. Remove only our strict private filenames;
/// delayed helpers still need the cache-locked target identity before any Git.
pub(crate) fn sweep_specs(state: &Path) -> std::io::Result<()> {
    let mut entries = match std::fs::read_dir(state.join("spare-builds")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for _ in 0..100_000 {
        let Some(entry) = entries.next() else {
            return Ok(());
        };
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        let owned = path
            .file_stem()
            .and_then(|name| name.to_str())
            .is_some_and(|stem| uuid::Uuid::parse_str(stem).is_ok_and(|id| id.to_string() == stem))
            && path.extension().is_some_and(|extension| {
                extension == "json" || extension == "receipt" || extension == "result"
            });
        if owned
            || entry
                .file_name()
                .to_str()
                .is_some_and(crate::files::is_staged_temp_name)
        {
            std::fs::remove_file(path)?;
        }
    }
    Err(std::io::Error::other(
        "spare private artifact sweep exhausted its entry budget",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounded_command::Ended;
    use std::os::unix::process::ExitStatusExt;

    /// A true marker is necessary but cannot turn a timed-out, overflowing or
    /// unrelated run into permission to publish an otherwise completed clone.
    #[farhelm_testtrace::test]
    fn completion_marker_cannot_override_outer_refusal_or_another_run() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("current.json");
        std::fs::write(receipt_path(&path), b"true").unwrap();
        std::fs::hard_link(receipt_path(&path), result_path(&path)).unwrap();
        let killed = Ended::Exited(std::process::Output {
            status: std::process::ExitStatus::from_raw(libc::SIGKILL),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
        assert!(completed(&killed, &path));
        assert!(!completed(&Ended::TimedOut, &path));
        assert!(!completed(&Ended::OutputOverCap, &path));
        assert!(!completed(&killed, &directory.path().join("another.json")));
        std::fs::write(result_path(&path), b"tru").unwrap();
        assert!(!completed(&killed, &path));
    }
}

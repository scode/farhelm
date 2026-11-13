//! One serving helm per state directory, enforced by a lifetime flock.
//!
//! A serving helm (a desktop app's embedded helm included) holds an exclusive
//! advisory lock on [`LOCK_NAME`] in its state directory for its whole life,
//! from before it opens `helm.db` until its runtime has drained. Other programs
//! that must not act while a helm serves that directory (uninstall, a desktop
//! launch deciding whether to replace its supervisor) take or test the same
//! lock.

use anyhow::Context;
use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// The helm's state-directory lock file.
///
/// NOTE: The name still says "token" because it predates the removal of
/// browser sign-in, when the same lock also serialized `farhelm helm token`.
/// Installed apps and uninstall check this exact file, so it keeps its name.
pub const LOCK_NAME: &str = "helm-token.lock";
/// How long a starting helm keeps retrying a held ownership lock.
///
/// Uninstall and a desktop launch hold the lock only briefly. A serving helm
/// holds it for its whole life. Waiting this long rides out a brief hold and
/// still refuses a rival helm within seconds.
pub const SERVING_CLAIM_WAIT: std::time::Duration = std::time::Duration::from_secs(5);
/// Pause between a starting helm's ownership attempts.
const SERVING_CLAIM_RETRY: std::time::Duration = std::time::Duration::from_millis(10);

/// The lock is held by another process right now.
#[derive(Debug, thiserror::Error)]
#[error("another helm owns this state directory")]
struct OwnershipBusy;

/// Exclusive ownership of one state directory by a serving helm.
///
/// A serving helm claims this with [`claim_serving_ownership`] before it opens
/// (and migrates) `helm.db`, and its runtime owner keeps it until runtime
/// teardown finishes.
pub(crate) struct OwnershipLock {
    file: File,
}

impl Drop for OwnershipLock {
    fn drop(&mut self) {
        // SAFETY: `file` remains open for this call, and unlocking an owned
        // advisory flock neither aliases memory nor transfers the fd.
        unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }
}

/// Claim the state directory for a serving helm.
///
/// Taken before the helm opens `helm.db`, because opening it as the serving
/// helm migrates the schema, and startup then writes the host registry and
/// dials the fleet. A second helm on another port must be refused before any
/// of that, not after it has upgraded the database under a running incumbent.
///
/// A held lock is retried for up to [`SERVING_CLAIM_WAIT`], because another
/// program may be holding it for a moment; only a lock still held after that
/// is reported as another owner.
pub(crate) async fn claim_serving_ownership(state_dir: &Path) -> anyhow::Result<OwnershipLock> {
    claim_serving_ownership_observed(state_dir, || {}).await
}

/// [`claim_serving_ownership`], calling `on_busy` each time an attempt finds
/// the lock held, so a test can prove the claim met contention before it
/// releases the lock.
async fn claim_serving_ownership_observed(
    state_dir: &Path,
    on_busy: impl Fn(),
) -> anyhow::Result<OwnershipLock> {
    farhelm_supervisor::ensure_private_dir(state_dir).await?;
    let deadline = tokio::time::Instant::now() + SERVING_CLAIM_WAIT;
    loop {
        match acquire_ownership(state_dir.to_path_buf()).await {
            Err(error) if error.downcast_ref::<OwnershipBusy>().is_some() => {
                on_busy();
                if tokio::time::Instant::now() >= deadline {
                    return Err(error.context(format!(
                        "the helm lock {} stayed held for {SERVING_CLAIM_WAIT:?}; is another \
                         helm already serving this state directory?",
                        state_dir.join(LOCK_NAME).display()
                    )));
                }
                tokio::time::sleep(SERVING_CLAIM_RETRY).await;
            }
            other => return other,
        }
    }
}

/// Try once to take ownership without parking an async worker.
pub(crate) async fn acquire_ownership(state_dir: PathBuf) -> anyhow::Result<OwnershipLock> {
    tokio::task::spawn_blocking(move || {
        let path = state_dir.join(LOCK_NAME);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&path)
            .with_context(|| format!("opening the helm lock {}", path.display()))?;
        let operation = libc::LOCK_EX | libc::LOCK_NB;
        // SAFETY: `file` owns a live fd for this call. flock changes kernel
        // advisory-lock state only and does not access Rust memory.
        let result = unsafe { libc::flock(file.as_raw_fd(), operation) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::WouldBlock {
                return Err(OwnershipBusy.into());
            }
            return Err(anyhow::Error::new(error)
                .context(format!("locking the helm lock for {}", state_dir.display())));
        }
        Ok(OwnershipLock { file })
    })
    .await
    .context("helm lock task panicked")?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A starting helm waits out another program's brief hold on the
    /// ownership lock instead of failing startup.
    ///
    /// Why it matters: uninstall and a desktop launch take the lock for a
    /// moment, and a single non-blocking attempt would make a helm starting
    /// in that moment exit as if another helm owned the directory. Spec: a
    /// lock released within the wait is claimed.
    #[farhelm_testtrace::test]
    async fn a_starting_helm_waits_out_a_brief_hold() {
        let dir = tempfile::tempdir().unwrap();
        let brief_hold = acquire_ownership(dir.path().to_path_buf()).await.unwrap();
        let busy = std::sync::Arc::new(tokio::sync::Notify::new());
        let claim = tokio::spawn({
            let dir = dir.path().to_path_buf();
            let busy = std::sync::Arc::clone(&busy);
            async move { claim_serving_ownership_observed(&dir, || busy.notify_one()).await }
        });
        tokio::time::timeout(std::time::Duration::from_secs(5), busy.notified())
            .await
            .expect("fixture premise: the claim found the lock held");
        assert!(
            !claim.is_finished(),
            "a claim that met the held lock must keep waiting rather than fail"
        );

        drop(brief_hold);
        tokio::time::timeout(std::time::Duration::from_secs(5), claim)
            .await
            .expect("the claim must finish once the lock is free")
            .expect("claim task")
            .expect("a lock released within the wait is claimed");
    }
}

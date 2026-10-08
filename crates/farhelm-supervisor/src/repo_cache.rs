//! Host-local Git objects shared only while a fresh checkout is prepared.
//!
//! Launch shims fetch and clone under the same per-repository flock that the
//! startup sweep takes before eviction. The lock file is permanent: unlinking
//! it would split waiting shims and new arrivals across different lock inodes.
//! Checkouts dissociate from the cache before releasing this lock, so eviction
//! never affects an existing checkout.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::launch::PreparationLock;

/// Derive the cache from the supervisor's state and an already validated repo.
/// This host-local detail is not part of a create request's frozen fingerprint.
pub(crate) fn path(state_dir: &Path, repo: &farhelm_proto::github_checkout::GithubRepo) -> PathBuf {
    state_dir
        .join("repo-cache")
        .join(&repo.owner)
        .join(format!("{}.git", repo.name))
}

/// Serialize cache mutation and checkout copying, after the preparation lock.
/// Git children never inherit this guard, even if a launch is interrupted.
pub(crate) fn acquire(cache: &Path) -> std::io::Result<PreparationLock> {
    let parent = cache.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "cache has no parent")
    })?;
    std::fs::create_dir_all(parent)?;
    let lock = PreparationLock::open(&cache.with_extension("lock"))?;
    lock.acquire()?;
    Ok(lock)
}

/// Evict caches unused for 30 days without disturbing surviving launch shims.
///
/// A supervisor restart does not stop the tmux panes preparing checkouts. A
/// held lock therefore skips both trash cleanup and eviction. Rename precedes
/// recursive removal so interruption can leave only trash, never a live cache
/// partially deleted. The next startup removes that trash under the same lock.
pub(crate) fn sweep(state_dir: &Path) -> std::io::Result<()> {
    let root = state_dir.join("repo-cache");
    let owners = match std::fs::read_dir(&root) {
        Ok(owners) => owners,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let now = SystemTime::now();
    let retention = Duration::from_secs(30 * 24 * 60 * 60);
    for owner in owners {
        let owner = owner?;
        if !owner.file_type()?.is_dir() {
            continue;
        }
        // Every cache starts by opening its permanent lock file. Enumerating
        // those files also finds interrupted evictions with only trash left.
        for entry in std::fs::read_dir(owner.path())? {
            let entry = entry?;
            let lock_path = entry.path();
            if lock_path
                .extension()
                .is_none_or(|extension| extension != "lock")
            {
                continue;
            }
            let lock = PreparationLock::open(&lock_path)?;
            if !lock.try_acquire()? {
                continue;
            }
            let cache = lock_path.with_extension("git");
            let trash = lock_path.with_extension("git.trash");
            remove_if_present(&trash)?;
            let unused = now
                .duration_since(lock.modified()?)
                .is_ok_and(|age| age > retention);
            if unused {
                match std::fs::rename(&cache, &trash) {
                    Ok(()) => remove_if_present(&trash)?,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e),
                }
            }
        }
    }
    Ok(())
}

/// Missing trash means no interrupted eviction; other failures remain visible.
/// `remove_dir_all` does not traverse a symlink to another directory.
fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create observable cached content and backdate the lock's last-use time.
    /// Tests inspect these premises before sweeping rather than relying on age
    /// inferred from how long fixture setup took.
    fn fixture(age_days: u64) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let state = tempfile::tempdir().unwrap();
        let repo = farhelm_proto::github_checkout::GithubRepo {
            owner: "example".into(),
            name: "repo".into(),
        };
        let cache = path(state.path(), &repo);
        let lock = acquire(&cache).unwrap();
        std::fs::create_dir(&cache).unwrap();
        std::fs::write(cache.join("objects"), "cached objects").unwrap();
        let modified = SystemTime::now() - Duration::from_secs(age_days * 24 * 60 * 60);
        std::fs::File::open(cache.with_extension("lock"))
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(lock.modified().unwrap(), modified);
        drop(lock);
        let lock_path = cache.with_extension("lock");
        (state, cache, lock_path)
    }

    /// Eviction removes stale cached contents while retaining the lock inode
    /// that waiting and future shims use for mutual exclusion.
    #[farhelm_testtrace::test]
    fn stale_cache_is_removed_but_lock_survives() {
        let (state, cache, lock_path) = fixture(31);
        assert!(cache.join("objects").exists());
        sweep(state.path()).unwrap();
        assert!(!cache.exists());
        assert!(lock_path.exists());
        assert!(!cache.with_extension("git.trash").exists());
    }

    /// Recent use retains the cache, independent of the repository's contents.
    #[farhelm_testtrace::test]
    fn fresh_cache_is_retained() {
        let (state, cache, _) = fixture(1);
        sweep(state.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(cache.join("objects")).unwrap(),
            "cached objects"
        );
    }

    /// A surviving shim can still borrow an old cache across supervisor restart;
    /// the sweep must skip it until the shim's independently opened flock ends.
    #[farhelm_testtrace::test]
    fn locked_cache_is_retained_until_guard_ends() {
        let (state, cache, lock_path) = fixture(31);
        let lock = acquire(&cache).unwrap();
        let contender = PreparationLock::open(&lock_path).unwrap();
        assert!(
            !contender.try_acquire().unwrap(),
            "premise: the shim owns the lock"
        );
        sweep(state.path()).unwrap();
        assert!(cache.join("objects").exists());
        drop(lock);
        sweep(state.path()).unwrap();
        assert!(!cache.exists());
        assert!(lock_path.exists());
    }

    /// An eviction interrupted after rename leaves trash that the next startup
    /// removes even when a new live cache has subsequently been used.
    #[farhelm_testtrace::test]
    fn leftover_trash_is_removed_without_evicting_recent_cache() {
        let (state, cache, lock_path) = fixture(1);
        let trash = cache.with_extension("git.trash");
        std::fs::create_dir(&trash).unwrap();
        std::fs::write(trash.join("objects"), "interrupted eviction").unwrap();
        assert!(trash.join("objects").exists());
        sweep(state.path()).unwrap();
        assert!(!trash.exists());
        assert!(cache.join("objects").exists());
        assert!(lock_path.exists());
    }
}

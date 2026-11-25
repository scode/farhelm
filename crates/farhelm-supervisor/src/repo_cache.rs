//! Host-local Git objects shared only while a fresh checkout is prepared.
//!
//! Terminal preparations and background builders fetch and clone under the same per-repository flock that the
//! startup sweep takes before eviction. The lock file is permanent: unlinking
//! it would split waiting shims and new arrivals across different lock inodes.
//! Checkouts dissociate from the cache before releasing this lock, so eviction
//! never affects an existing checkout. Each physical cache gets its own path
//! because a Git child can outlive the process that held Farhelm's flock.
//! Spare recovery shares the sweep's lock and age observation, but uses its
//! own recorded namespace authority and the checkout trash's confined remover.

use std::collections::HashMap;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use uuid::Uuid;

use crate::launch::PreparationLock;

/// Derive the stable cache key from an already validated repo. The lock and
/// spare record keep their names across both upgrades and cache replacements.
pub(crate) fn path(state_dir: &Path, repo: &farhelm_proto::github_checkout::GithubRepo) -> PathBuf {
    state_dir
        .join("repo-cache")
        .join(&repo.owner)
        .join(format!("{}.git", repo.name))
}

/// Serialize cache mutation and checkout copying, after the preparation lock.
/// Git children never inherit this guard, even if a launch is interrupted.
pub(crate) fn acquire(cache: &Path) -> std::io::Result<PreparationLock> {
    let lock = open_lock(cache)?;
    lock.acquire()?;
    Ok(lock)
}

const SELECTOR_SUFFIX: &str = ":current";
const GENERATION_PREFIX: &str = ":gen-";

/// A colon is forbidden in a GitHub repo name and reserves private siblings
/// without overlapping another repository's cache key.
fn private_path(cache: &Path, suffix: &str) -> PathBuf {
    let mut path = cache.as_os_str().to_os_string();
    path.push(suffix);
    path.into()
}

/// Accept only names produced by the cache and staged-write code. The same
/// check keeps a selector's contents from naming anything outside this repo.
fn is_canonical_uuid(text: &str) -> bool {
    Uuid::parse_str(text).is_ok_and(|uuid| uuid.to_string() == text)
}

/// Read the selected physical path under the repo's flock. An absent record
/// leaves the legacy path as the only possible existing cache. A malformed
/// record gives the sweep no authority to delete this repo's generations.
pub(crate) fn current(cache: &Path, _held: &PreparationLock) -> io::Result<Option<PathBuf>> {
    let selector = private_path(cache, SELECTOR_SUFFIX);
    let metadata = match selector.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let invalid = || io::Error::new(ErrorKind::InvalidData, "invalid repository cache selector");
    if !metadata.is_file() || metadata.len() != uuid::fmt::Hyphenated::LENGTH as u64 {
        return Err(invalid());
    }
    let id = fs::read_to_string(selector)?;
    if !is_canonical_uuid(&id) {
        return Err(invalid());
    }
    Ok(Some(private_path(
        cache,
        &format!("{GENERATION_PREFIX}{id}"),
    )))
}

/// Refresh a selected cache in place using Git's own locking. If that fails,
/// prepare one fresh generation before changing the selection. An upgrade
/// also builds in a fresh path, leaving the legacy cache available on failure.
pub(crate) fn refresh(
    cache: &Path,
    held: &PreparationLock,
    mut prepare: impl FnMut(&Path) -> Result<(), String>,
) -> Result<PathBuf, String> {
    let previous =
        current(cache, held).map_err(|e| format!("could not read cache selection: {e}"))?;
    let mut previous_error = None;
    if let Some(path) = &previous
        && path
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.is_dir())
    {
        match prepare(path) {
            Ok(()) => return Ok(path.clone()),
            Err(error) => previous_error = Some(error),
        }
    }

    let id = Uuid::new_v4().to_string();
    let fresh = private_path(cache, &format!("{GENERATION_PREFIX}{id}"));
    // Reserve the new name before Git runs. Even an unexpected UUID collision
    // must fail here instead of opening another generation in place.
    fs::create_dir(&fresh).map_err(|e| format!("could not create cache directory: {e}"))?;
    if let Err(error) = prepare(&fresh) {
        retire(&fresh);
        return Err(match previous_error {
            Some(first) => format!("{first}; rebuilding the cache also failed: {error}"),
            None => error,
        });
    }

    // An error syncing the parent can happen after the selector rename. Keep
    // both paths in that case so whichever selection persists remains usable.
    crate::files::write_durable_sync(
        &private_path(cache, SELECTOR_SUFFIX),
        id.as_bytes(),
        &crate::files::RealFs,
    )
    .map_err(|e| format!("could not publish cache selection: {e}"))?;
    if let Some(path) = previous {
        retire(&path);
    }
    retire(cache);
    Ok(fresh)
}

/// An unselected generation can be removed even if an old Git child is still
/// using its path. No future generation is installed under that name.
fn retire(path: &Path) {
    if let Err(error) = remove_if_present(path) {
        tracing::warn!(path = %path.display(), %error, "obsolete repository cache retained for startup sweep");
    }
}

/// Open the same permanent cache lock without waiting for another owner. The
/// runtime builder uses try_acquire so a credential prompt in a visible clone
/// cannot strand an unbounded blocking worker across cancellation or shutdown.
pub(crate) fn open_lock(cache: &Path) -> std::io::Result<PreparationLock> {
    let parent = cache.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "cache has no parent")
    })?;
    std::fs::create_dir_all(parent)?;
    PreparationLock::open(&cache.with_extension("lock"))
}

/// Evict caches unused for 30 days without disturbing surviving launch shims.
///
/// A supervisor restart does not stop the terminal hosts preparing checkouts. A
/// held lock skips cleanup and eviction. Clear the selector durably before
/// removing its generation so an interruption leaves an unselected directory.
/// Spare leftovers are recovered under this same guard and ready spares share
/// the cache's eviction decision; a failed spare cleanup retains its record.
/// Report incomplete cleanup after visiting every independently usable repo.
pub(crate) fn sweep(
    state_dir: &Path,
    live: &crate::spare_checkouts::LiveClaims,
) -> std::io::Result<()> {
    sweep_with_effects(
        state_dir,
        live,
        &mut |owner| fs::File::open(owner)?.sync_all(),
        &mut remove_if_present,
    )
}

/// Vary only durability and final removal so tests still exercise the real
/// enumeration, lock, selector and eviction namespace.
fn sweep_with_effects(
    state_dir: &Path,
    live: &crate::spare_checkouts::LiveClaims,
    sync_owner: &mut impl FnMut(&Path) -> io::Result<()>,
    remove: &mut impl FnMut(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let root = state_dir.join("repo-cache");
    let owners = match fs::read_dir(&root) {
        Ok(owners) => owners,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let now = SystemTime::now();
    let mut failure = None;
    for owner in owners {
        match owner {
            Ok(owner) => {
                let path = owner.path();
                let result = owner.file_type().and_then(|kind| {
                    if kind.is_dir() {
                        sweep_owner(&path, live, now, sync_owner, remove, &mut failure)?;
                    }
                    Ok(())
                });
                if let Err(error) = result {
                    record_sweep_failure(&mut failure, &path, error);
                }
            }
            Err(error) => record_sweep_failure(&mut failure, &root, error),
        }
    }
    failure.map_or(Ok(()), Err)
}

/// A complete owner listing associates debris with known repository locks.
/// An enumeration failure leaves the owner alone so other owners can proceed.
fn sweep_owner(
    owner: &Path,
    live: &crate::spare_checkouts::LiveClaims,
    now: SystemTime,
    sync_owner: &mut impl FnMut(&Path) -> io::Result<()>,
    remove: &mut impl FnMut(&Path) -> io::Result<()>,
    failure: &mut Option<io::Error>,
) -> io::Result<()> {
    let mut locks = Vec::new();
    let mut debris = HashMap::<String, Vec<PathBuf>>::new();
    for entry in fs::read_dir(owner)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "lock")
        {
            locks.push(path);
        } else if let Some(cache_name) = entry.file_name().to_str().and_then(debris_cache_name) {
            debris.entry(cache_name.to_owned()).or_default().push(path);
        }
    }

    // Visit a single owner's locks in a stable order so tests can place a failure before a sibling repo.
    locks.sort_unstable();
    let retention = Duration::from_secs(30 * 24 * 60 * 60);
    for lock_path in locks {
        let cache = lock_path.with_extension("git");
        // Keep this repo's prerequisites fallible without leaving the owner loop.
        let result = (|| {
            let lock = PreparationLock::open(&lock_path)?;
            if !lock.try_acquire()? {
                return Ok(());
            }
            let unused = now
                .duration_since(lock.modified()?)
                .is_ok_and(|age| age > retention);
            let repo_text = format!(
                "{}/{}",
                owner.file_name().unwrap_or_default().to_string_lossy(),
                lock_path.file_stem().unwrap_or_default().to_string_lossy()
            );
            // Permanent locks also locate a spare record after its cache is
            // already gone. Invalid foreign names never become checkout paths.
            if let Ok(repo) = farhelm_proto::github_checkout::parse_github_repo(&repo_text)
                && let Err(error) = crate::spare_checkouts::sweep(&cache, &repo, live, unused)
            {
                record_sweep_failure(
                    failure,
                    &cache.with_extension("spare.json"),
                    io::Error::other(error),
                );
            }
            let paths = cache
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| debris.remove(name))
                .unwrap_or_default();
            sweep_generations(&cache, &lock, paths, unused, sync_owner, remove, failure)
        })();
        if let Err(error) = result {
            record_sweep_failure(failure, &cache, error);
        }
    }
    Ok(())
}

/// Log each failed location while keeping a bounded result for the caller.
fn record_sweep_failure(failure: &mut Option<io::Error>, path: &Path, error: io::Error) {
    tracing::warn!(path = %path.display(), %error, "repository cache sweep step failed");
    failure.get_or_insert(error);
}

/// Recover both unpublished generations and selector temps left by a crash.
/// The latter follow `files::temp_path_for` with the selector as destination.
fn debris_cache_name(name: &str) -> Option<&str> {
    let (cache, id) = name
        .rsplit_once(GENERATION_PREFIX)
        .or_else(|| name.strip_prefix('.')?.rsplit_once(":current.tmp-"))?;
    (cache.ends_with(".git") && !cache.contains(':') && is_canonical_uuid(id)).then_some(cache)
}

/// Reclaim a repo's unselected paths. Sync the owner directory before deleting
/// any generation because a previous selector write may have failed after its
/// rename. During eviction, detach both ways of selecting an expired cache
/// before deleting its contents: move the legacy path and clear the selector.
/// A barrier failure stops this repository. Once paths are known to be
/// detached, a failed removal leaves the other paths eligible.
fn sweep_generations(
    cache: &Path,
    held: &PreparationLock,
    mut paths: Vec<PathBuf>,
    unused: bool,
    sync_owner: &mut impl FnMut(&Path) -> io::Result<()>,
    remove: &mut impl FnMut(&Path) -> io::Result<()>,
    failure: &mut Option<io::Error>,
) -> io::Result<()> {
    // The selector is authoritative only after we take the repo lock.
    let selected = current(cache, held)?;
    let owner = cache.parent().expect("cache in owner directory");
    let trash = cache.with_extension("git.trash");
    let mut trash_clear = true;
    if entry_exists(&trash)? {
        sync_owner(owner)?;
        if let Err(error) = remove(&trash) {
            record_sweep_failure(failure, &trash, error);
            trash_clear = false;
        }
    }
    let legacy_present = unused && entry_exists(cache)?;
    if unused && (trash_clear || !legacy_present) {
        if legacy_present {
            if selected.is_some() {
                // A migration could have renamed its selector and failed the
                // directory sync. Stabilize the selection before moving legacy data.
                sync_owner(owner)?;
            }
            match fs::rename(cache, &trash) {
                Ok(()) => paths.push(trash),
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        if let Some(selected) = selected {
            fs::remove_file(private_path(cache, SELECTOR_SUFFIX))?;
            paths.push(selected);
        }
    } else if let Some(selected) = selected {
        paths.retain(|path| *path != selected);
        // If an expired legacy cache needs the occupied trash slot, keep its
        // selector too. A recent cache can still finish migration.
        if !unused && entry_exists(cache)? {
            paths.push(cache.to_owned());
        }
    }

    if !paths.is_empty() {
        sync_owner(owner)?;
        for path in paths {
            if let Err(error) = remove(&path) {
                record_sweep_failure(failure, &path, error);
            }
        }
    }
    Ok(())
}

/// A dangling link occupies a cache slot. Only a missing entry permits the
/// sweep to proceed as if that slot were empty.
fn entry_exists(path: &Path) -> io::Result<bool> {
    match path.symlink_metadata() {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Remove obsolete cache entries without following a symlink in the directory
/// slot. Interrupted cleanup can also leave a plain file under a cache name.
fn remove_if_present(path: &Path) -> io::Result<()> {
    let removed = match path.symlink_metadata() {
        Ok(metadata) if metadata.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
        Err(error) => Err(error),
    };
    match removed {
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
        let (cache, lock_path) = seed_cache(state.path(), "repo", age_days);
        (state, cache, lock_path)
    }

    /// Add another repo to the same owner listing so tests can arrange
    /// which repo fails before the sweep reaches its siblings.
    fn seed_cache(state: &Path, name: &str, age_days: u64) -> (PathBuf, PathBuf) {
        let repo = farhelm_proto::github_checkout::GithubRepo {
            owner: "example".into(),
            name: name.into(),
        };
        let cache = path(state, &repo);
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
        (cache, lock_path)
    }

    /// Publish a selected generation through the real writer alongside
    /// debris that would be eligible once the next sweep reads the selector.
    fn selected_and_debris(cache: &Path) -> (PathBuf, PathBuf) {
        let lock = acquire(cache).unwrap();
        let selected = refresh(cache, &lock, |_| Ok(())).unwrap();
        let debris = private_path(cache, &format!("{GENERATION_PREFIX}{}", Uuid::new_v4()));
        fs::create_dir(&debris).unwrap();
        assert_eq!(current(cache, &lock).unwrap().as_ref(), Some(&selected));
        assert!(selected.is_dir() && debris.is_dir());
        (selected, debris)
    }

    /// A repo without a usable lock has no cleanup authority. Its failure
    /// must leave a later expired cache eligible.
    #[farhelm_testtrace::test]
    fn sweep_skips_a_repo_with_an_unusable_lock() {
        let (state, cache, lock_path) = fixture(31);
        let (later, _) = seed_cache(state.path(), "z-later", 31);
        fs::remove_file(&lock_path).unwrap();
        fs::create_dir(&lock_path).unwrap();
        let lock_error = PreparationLock::open(&lock_path)
            .err()
            .expect("a directory cannot serve as a writable lock file");
        assert!(cache.is_dir() && later.is_dir());

        let failure = sweep(state.path(), &Default::default()).unwrap_err();

        assert_eq!(failure.raw_os_error(), lock_error.raw_os_error());
        assert!(cache.join("objects").is_file());
        assert!(!later.exists());
    }

    /// The durable selector can still refer to either layout after a failed
    /// owner sync. Keep both generations in this repo and clean the next.
    #[farhelm_testtrace::test]
    fn sweep_keeps_repo_generations_when_owner_sync_fails() {
        let (state, cache, _) = fixture(1);
        let (selected, debris) = selected_and_debris(&cache);
        let (later, _) = seed_cache(state.path(), "z-later", 31);
        assert!(later.is_dir());
        let mut first_sync = true;
        let error = sweep_with_effects(
            state.path(),
            &Default::default(),
            &mut |owner| {
                if std::mem::take(&mut first_sync) {
                    return Err(io::Error::other("fixture owner sync failure"));
                }
                fs::File::open(owner)?.sync_all()
            },
            &mut remove_if_present,
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "fixture owner sync failure");
        assert!(selected.is_dir() && debris.is_dir());
        let lock = acquire(&cache).unwrap();
        assert_eq!(current(&cache, &lock).unwrap().as_ref(), Some(&selected));
        assert!(!later.exists());
    }

    /// A damaged selector supplies no authority to delete that repo's
    /// generations. The next repo can still expire independently.
    #[farhelm_testtrace::test]
    fn sweep_keeps_repo_generations_when_its_selector_is_malformed() {
        let (state, cache, _) = fixture(1);
        let (selected, debris) = selected_and_debris(&cache);
        let (later, _) = seed_cache(state.path(), "z-later", 31);
        let selector = private_path(&cache, SELECTOR_SUFFIX);
        fs::write(&selector, "invalid selection").unwrap();
        assert!(later.is_dir());

        let error = sweep(state.path(), &Default::default()).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::InvalidData);
        assert!(selected.is_dir() && debris.is_dir());
        assert_eq!(fs::read_to_string(selector).unwrap(), "invalid selection");
        assert!(!later.exists());
    }

    /// A failed first deletion does not strand a sibling whose absence the
    /// same selector already allows, and the current generation is kept.
    #[farhelm_testtrace::test]
    fn sweep_continues_after_a_debris_removal_fails() {
        let (state, cache, _) = fixture(1);
        let (selected, first_debris) = selected_and_debris(&cache);
        let second_debris = private_path(&cache, &format!("{GENERATION_PREFIX}{}", Uuid::new_v4()));
        fs::create_dir(&second_debris).unwrap();
        let (later, _) = seed_cache(state.path(), "z-later", 31);
        assert!(later.is_dir() && second_debris.is_dir());
        let mut retained = None;
        let error = sweep_with_effects(
            state.path(),
            &Default::default(),
            &mut |owner| fs::File::open(owner)?.sync_all(),
            &mut |path| {
                if retained.is_none() {
                    assert!(path == first_debris || path == second_debris);
                    retained = Some(path.to_owned());
                    return Err(io::Error::other("fixture debris removal failure"));
                }
                remove_if_present(path)
            },
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "fixture debris removal failure");
        let retained = retained.unwrap();
        for debris in [first_debris, second_debris] {
            assert_eq!(debris.exists(), debris == retained);
        }
        assert!(selected.is_dir());
        assert!(!later.exists());
    }

    /// A leftover trash slot can still contain interrupted work. If a legacy
    /// cache needs that slot, keep the selector and both layouts. Otherwise
    /// the selected generation can expire alongside ordinary debris.
    #[farhelm_testtrace::test]
    fn sweep_defers_eviction_only_when_legacy_cache_needs_the_trash_slot() {
        for legacy_present in [false, true] {
            let (state, cache, _) = fixture(31);
            let (selected, debris) = selected_and_debris(&cache);
            if legacy_present {
                fs::create_dir(&cache).unwrap();
                fs::write(cache.join("objects"), "legacy objects").unwrap();
            }
            let trash = cache.with_extension("git.trash");
            fs::create_dir(&trash).unwrap();
            fs::write(trash.join("objects"), "older trash").unwrap();
            let (later, _) = seed_cache(state.path(), "z-later", 31);
            assert!(later.is_dir() && trash.is_dir());
            assert_eq!(cache.is_dir(), legacy_present);

            let error = sweep_with_effects(
                state.path(),
                &Default::default(),
                &mut |owner| fs::File::open(owner)?.sync_all(),
                &mut |path| {
                    if path == trash {
                        Err(io::Error::other("fixture trash removal failure"))
                    } else {
                        remove_if_present(path)
                    }
                },
            )
            .unwrap_err();

            assert_eq!(error.to_string(), "fixture trash removal failure");
            assert_eq!(
                fs::read_to_string(trash.join("objects")).unwrap(),
                "older trash"
            );
            assert_eq!(cache.is_dir(), legacy_present);
            assert_eq!(selected.is_dir(), legacy_present);
            let lock = acquire(&cache).unwrap();
            assert_eq!(
                current(&cache, &lock).unwrap().as_ref(),
                legacy_present.then_some(&selected)
            );
            assert!(!debris.exists() && !later.exists());
        }
    }

    /// Eviction removes stale cached contents while retaining the lock inode
    /// that waiting and future shims use for mutual exclusion.
    #[farhelm_testtrace::test]
    fn stale_cache_is_removed_but_lock_survives() {
        let (state, cache, lock_path) = fixture(31);
        assert!(cache.join("objects").exists());
        sweep(state.path(), &Default::default()).unwrap();
        assert!(!cache.exists());
        assert!(lock_path.exists());
        assert!(!cache.with_extension("git.trash").exists());
    }

    /// A malformed cache can be a plain file. Once it is due for eviction, the sweep removes it without leaving
    /// a trash file that would fail every later sweep. The permanent lock still guards future checkouts.
    #[farhelm_testtrace::test]
    fn stale_plain_file_cache_is_removed_but_lock_survives() {
        let (state, cache, lock_path) = fixture(31);
        std::fs::remove_dir_all(&cache).unwrap();
        std::fs::write(&cache, "interrupted cache fixture").unwrap();
        assert!(cache.is_file(), "premise: the stale cache is a plain file");

        sweep(state.path(), &Default::default()).unwrap();

        assert!(!cache.exists());
        assert!(!cache.with_extension("git.trash").exists());
        assert!(lock_path.exists());
    }

    /// Interrupted eviction can leave a symlink in its trash slot. The sweep unlinks it and preserves both
    /// the directory it pointed to and the live cache that was opened afterward.
    #[farhelm_testtrace::test]
    fn leftover_trash_symlink_is_unlinked_without_visiting_its_target() {
        let (state, cache, _) = fixture(1);
        let target = state.path().join("retained");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("sentinel"), "keep").unwrap();
        let trash = cache.with_extension("git.trash");
        std::os::unix::fs::symlink(&target, &trash).unwrap();
        assert_eq!(
            std::fs::read_link(&trash).unwrap(),
            target,
            "premise: the trash link points at the retained directory"
        );

        sweep(state.path(), &Default::default()).unwrap();

        assert_eq!(
            trash.symlink_metadata().unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(
            std::fs::read_to_string(target.join("sentinel")).unwrap(),
            "keep"
        );
        assert!(cache.join("objects").exists());
    }

    /// Recent use retains the cache, independent of the repository's contents.
    #[farhelm_testtrace::test]
    fn fresh_cache_is_retained() {
        let (state, cache, _) = fixture(1);
        sweep(state.path(), &Default::default()).unwrap();
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
        sweep(state.path(), &Default::default()).unwrap();
        assert!(cache.join("objects").exists());
        drop(lock);
        sweep(state.path(), &Default::default()).unwrap();
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
        sweep(state.path(), &Default::default()).unwrap();
        assert!(!trash.exists());
        assert!(cache.join("objects").exists());
        assert!(lock_path.exists());
    }

    /// An upgrade builds in a new path. An unavailable remote can leave the
    /// old cache in place, and only a published replacement retires it.
    #[farhelm_testtrace::test]
    fn legacy_cache_is_retained_until_migration_succeeds() {
        let (_state, cache, _) = fixture(1);
        let lock = acquire(&cache).unwrap();
        assert!(current(&cache, &lock).unwrap().is_none());
        let mut failed_path = None;
        let error = refresh(&cache, &lock, |fresh| {
            assert_ne!(fresh, cache);
            assert!(cache.join("objects").exists());
            assert!(fresh.is_dir());
            assert!(failed_path.replace(fresh.to_owned()).is_none());
            fs::write(fresh.join("partial"), "fetched objects").unwrap();
            Err("unavailable remote".into())
        });
        assert_eq!(error.unwrap_err(), "unavailable remote");
        let failed_path = failed_path.unwrap();
        assert!(!failed_path.exists());
        assert!(current(&cache, &lock).unwrap().is_none());

        let selected = refresh(&cache, &lock, |fresh| {
            assert_ne!(fresh, cache);
            assert_ne!(fresh, failed_path);
            assert!(cache.join("objects").exists());
            fs::write(fresh.join("objects"), "migrated objects").unwrap();
            Ok(())
        })
        .unwrap();
        assert_eq!(current(&cache, &lock).unwrap().as_ref(), Some(&selected));
        assert_eq!(
            fs::read_to_string(selected.join("objects")).unwrap(),
            "migrated objects"
        );
        assert!(!cache.exists());
    }

    /// An interrupted refresh can leave unpublished paths. The sweep keeps
    /// their selected sibling, skips the whole repo while its flock is held,
    /// and only reclaims names it can attribute to a permanent lock.
    #[farhelm_testtrace::test]
    fn sweep_reclaims_only_unselected_owned_paths() {
        let (state, cache, lock_path) = fixture(1);
        let lock = acquire(&cache).unwrap();
        let selected = refresh(&cache, &lock, |_| Ok(())).unwrap();
        assert!(selected.is_dir());

        let debris_dir = private_path(&cache, &format!("{GENERATION_PREFIX}{}", Uuid::new_v4()));
        fs::create_dir(&debris_dir).unwrap();
        let debris_file = private_path(&cache, &format!("{GENERATION_PREFIX}{}", Uuid::new_v4()));
        fs::write(&debris_file, "malformed generation").unwrap();
        let staged = cache.with_file_name(format!(".repo.git:current.tmp-{}", Uuid::new_v4()));
        fs::write(&staged, "partial selector").unwrap();
        fs::create_dir(&cache).unwrap();
        fs::write(
            cache.join("objects"),
            "migration interrupted after publication",
        )
        .unwrap();
        let legacy_trash = cache.with_extension("git.trash");
        fs::write(&legacy_trash, "pre-upgrade debris").unwrap();
        let invalid = private_path(&cache, ":gen-not-a-uuid");
        let other = cache.with_file_name("repo.git.trash-other.git");
        let lockless = private_path(&other, &format!("{GENERATION_PREFIX}{}", Uuid::new_v4()));
        for retained in [&invalid, &other, &lockless] {
            fs::create_dir(retained).unwrap();
        }
        let debris = [&debris_dir, &debris_file, &staged, &legacy_trash, &cache];
        let contender = open_lock(&cache).unwrap();
        assert!(!contender.try_acquire().unwrap());
        assert!(debris.iter().all(|path| path.exists()));

        sweep(state.path(), &Default::default()).unwrap();
        assert!(debris.iter().all(|path| path.exists()));
        drop(lock);
        sweep(state.path(), &Default::default()).unwrap();
        assert!(debris.iter().all(|path| !path.exists()));
        assert!(selected.is_dir());
        assert!(invalid.is_dir() && other.is_dir() && lockless.is_dir());

        // The same generation becomes reclaimable when the checkout-use lock ages out.
        // A legacy file in the old slot cannot become current again during eviction.
        fs::write(&cache, "legacy entry beside expired selection").unwrap();
        let expired = SystemTime::now() - Duration::from_secs(31 * 24 * 60 * 60);
        fs::File::open(&lock_path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(expired))
            .unwrap();
        assert_eq!(contender.modified().unwrap(), expired);
        sweep(state.path(), &Default::default()).unwrap();
        assert!(!selected.exists());
        assert!(!cache.exists());
        assert!(!legacy_trash.exists());
        assert!(!private_path(&cache, SELECTOR_SUFFIX).exists());
        assert!(lock_path.exists());
    }
}

//! Permanent cleanup of recorded retired checkouts, separate from archiving.
//!
//! The registry supplies deletion authority; directory names supplied by a
//! client never do. The supervisor holds directory admission through this
//! deletion work, including after its caller stops waiting, so a new
//! session cannot enter an archive between the live-directory check and removal.
//! Spare recovery reuses only the confined opened-tree traversal below; its
//! namespace ownership and trash rename remain in `spare_checkouts`.

use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::fd::AsFd;
#[cfg(test)]
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use farhelm_proto::{
    ArchivedCheckout, CheckoutTrashDeleted, CheckoutTrashIssue, CheckoutTrashListing,
};

use crate::db::Db;
use crate::working_copies::{self, AllocationState, WorkingCopyError, WorkingCopyRow};
use rustix::fs::{AtFlags, Dir, Mode, OFlags};

/// One host shares this budget across all requested sizes, rather than granting
/// each checkout a new deadline. A blocked syscall may outlive it; the shared
/// directory worker permit remains held until that syscall returns.
pub(crate) const SIZE_BUDGET: Duration = Duration::from_secs(2);
const SIZE_ENTRY_CAP: usize = 100_000;
/// Three concurrent traversals (two reads and one delete) must leave descriptor
/// headroom for the supervisor on macOS's low default process limit.
const MAX_DEPTH: usize = 32;
/// A finite retry absorbs directory-iterator skips without following a writer
/// that continuously repopulates a checkout forever.
const REMOVAL_RESCANS: usize = 3;
/// Refuse an oversized selection before allocating per-ID state. The control
/// frame already bounds input bytes; this also bounds small-ID request work.
pub(crate) const DELETE_ID_CAP: usize = 10_000;

/// The only paths that may become trash entries are single-component recorded
/// names beneath a root whose identity still matches allocation evidence.
fn archive_name(row: &WorkingCopyRow) -> anyhow::Result<&str> {
    let name = row
        .archive_destination
        .as_deref()
        .context("archive destination is missing")?;
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(WorkingCopyError::WrongState(row.id.clone()).into());
    }
    Ok(name)
}

/// Hold the verified archive and its parent independently of path resolution.
/// `path` is only for display and the session cwd guard, never recursive I/O.
struct OpenArchive {
    parent: fs::File,
    directory: fs::File,
    name: CString,
    path: PathBuf,
}

/// Open one child without crossing symlinks or mount boundaries. Linux's
/// NO_XDEV covers same-device bind mounts too; comparing device numbers alone
/// would delete their external contents. An old Linux kernel may still list
/// identities, but traversal and permanent deletion require openat2 support.
pub(crate) fn open_child_dir(
    parent: impl AsFd,
    name: &CStr,
    confined: bool,
) -> std::io::Result<fs::File> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    #[cfg(target_os = "linux")]
    {
        use rustix::fs::ResolveFlags;
        match rustix::fs::openat2(
            parent.as_fd(),
            name,
            flags,
            Mode::empty(),
            ResolveFlags::NO_XDEV | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        ) {
            Ok(fd) => Ok(fd.into()),
            Err(rustix::io::Errno::NOSYS | rustix::io::Errno::PERM) if !confined => {
                let fd = rustix::fs::openat(parent.as_fd(), name, flags, Mode::empty())?;
                if rustix::fs::fstat(&fd)?.st_dev != rustix::fs::fstat(parent)?.st_dev {
                    return Err(rustix::io::Errno::XDEV.into());
                }
                Ok(fd.into())
            }
            Err(error) => Err(error.into()),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = confined;
        let fd = rustix::fs::openat(parent.as_fd(), name, flags, Mode::empty())?;
        if rustix::fs::fstat(&fd)?.st_dev != rustix::fs::fstat(parent)?.st_dev {
            return Err(rustix::io::Errno::XDEV.into());
        }
        Ok(fd.into())
    }
}

/// Migrate before selecting a recorded archive beneath its verified opened root.
/// A missing new name proves absence only after legacy trash has been resolved;
/// migration and root errors remain uncertainty, so reads retain the record.
fn open_archive(
    db: &Db,
    row: &WorkingCopyRow,
    confined: bool,
) -> anyhow::Result<Option<OpenArchive>> {
    let name = CString::new(archive_name(row)?)?;
    let root = working_copies::verified_root_dir(row)?;
    // Allocation uses this same lock through mkdir and registry commit. Keep
    // its overlap check atomic with migration, then release it before opening
    // individual archives or measuring their contents.
    working_copies::migrate_legacy_archive(&db.lock(), row, &root)?;
    let path = Path::new(&row.canonical_root)
        .join(working_copies::ARCHIVE_DIR_NAME)
        .join(archive_name(row)?);
    let parent_name = CString::new(working_copies::ARCHIVE_DIR_NAME).expect("constant name");
    let opened = (|| {
        let parent = open_child_dir(&root, &parent_name, confined)?;
        let directory = open_child_dir(&parent, &name, confined)?;
        Ok::<_, std::io::Error>((parent, directory))
    })();
    let (parent, directory) = match opened {
        Ok(opened) => opened,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) if matches!(error.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR)) => {
            return Err(WorkingCopyError::IdentityMismatch { path }.into());
        }
        Err(error)
            if matches!(error.raw_os_error(), Some(libc::ENOSYS | libc::EPERM)) && confined =>
        {
            bail!(
                "this system cannot confine permanent deletion to the archive (openat2 is unavailable or denied); folder left untouched"
            );
        }
        Err(error) => return Err(error.into()),
    };
    working_copies::verify_archived_handle(row, &directory, &path)?;
    Ok(Some(OpenArchive {
        parent,
        directory,
        name,
        path,
    }))
}

/// One open directory in a post-order removal. The explicit stack avoids
/// overflowing the Rust call stack on deeply nested checkouts; OS descriptor
/// exhaustion is a reported partial failure, never a traversal fallback.
struct RemovalFrame {
    entries: Dir,
    name: CString,
    identity: rustix::fs::Stat,
    rescans: usize,
}

/// Successful unlink operations, retained even when a later operation fails.
/// Files include symlinks and other non-directory entries; directories include
/// the selected tree's root. Retried directory scans never add to these counts.
#[derive(Default)]
pub(crate) struct RemovalCounts {
    pub(crate) files: u64,
    pub(crate) directories: u64,
}

/// Remove entries reachable from the verified handle, without following links
/// or mounts. POSIX names can change during removal: a final rmdir can only
/// remove an empty directory, and recursive work always stays on opened objects.
fn remove_archive(archive: &OpenArchive, counts: &mut RemovalCounts) -> anyhow::Result<()> {
    remove_archive_with(archive, counts, &mut |parent, name| {
        open_child_dir(parent, name, true)
    })
}

/// Inject only the confined child-open boundary. Tests can model a mount refusal
/// without requiring mount privileges or replacing the deletion algorithm.
fn remove_archive_with(
    archive: &OpenArchive,
    counts: &mut RemovalCounts,
    open: &mut impl FnMut(std::os::fd::BorrowedFd<'_>, &CStr) -> std::io::Result<fs::File>,
) -> anyhow::Result<()> {
    remove_opened_tree_with(
        &archive.parent,
        &archive.directory,
        &archive.name,
        counts,
        open,
    )
}

/// Remove a tree whose caller already established ownership on these handles.
/// Names alone grant no authority: the caller must verify the selected object
/// and park it under a private trash name before entering this shared traversal.
/// Counts describe successful unlinks even when the traversal later fails.
pub(crate) fn remove_opened_tree(
    parent: &fs::File,
    directory: &fs::File,
    name: &CStr,
    counts: &mut RemovalCounts,
) -> anyhow::Result<()> {
    remove_opened_tree_with(parent, directory, name, counts, &mut |parent, name| {
        open_child_dir(parent, name, true)
    })
}

/// Keep mount-refusal injection on the same traversal used by archives and
/// spares. No error permits an unconfined or pathname-recursive fallback.
/// Counting follows successful mutations, so rescans add no filesystem work
/// and cannot count the same directory more than once.
pub(crate) fn remove_opened_tree_with(
    parent_directory: &fs::File,
    directory: &fs::File,
    name: &CStr,
    counts: &mut RemovalCounts,
    open: &mut impl FnMut(std::os::fd::BorrowedFd<'_>, &CStr) -> std::io::Result<fs::File>,
) -> anyhow::Result<()> {
    let metadata = rustix::fs::fstat(directory)?;
    let mut stack = vec![RemovalFrame {
        entries: Dir::read_from(directory)?,
        name: name.to_owned(),
        identity: metadata,
        rescans: 0,
    }];
    while !stack.is_empty() {
        let next = stack.last_mut().expect("nonempty stack").entries.next();
        if let Some(entry) = next {
            let entry = entry?;
            let name = entry.file_name();
            if name.to_bytes() == b"." || name.to_bytes() == b".." {
                continue;
            }
            let parent = stack.last().expect("current directory").entries.fd()?;
            match open(parent, name) {
                Ok(child) => {
                    if stack.len() >= MAX_DEPTH {
                        bail!(
                            "archive nesting exceeds the {MAX_DEPTH}-directory limit; deeper contents were left untouched"
                        );
                    }
                    let metadata = rustix::fs::fstat(&child)?;
                    stack.push(RemovalFrame {
                        entries: Dir::new(child)?,
                        name: name.to_owned(),
                        identity: metadata,
                        rescans: 0,
                    });
                }
                Err(error) if matches!(error.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR)) => {
                    rustix::fs::unlinkat(parent, name, AtFlags::empty())?;
                    counts.files += 1;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) if error.raw_os_error() == Some(libc::EXDEV) => {
                    bail!(
                        "something is mounted inside this archive; the mount was left untouched; unmount it before retrying"
                    );
                }
                Err(error) if error.raw_os_error() == Some(libc::ENOSYS) => {
                    bail!(
                        "this system cannot confine deletion to the archive (openat2 is unavailable)"
                    );
                }
                Err(error) => return Err(error.into()),
            }
        } else {
            let mut completed = stack.pop().expect("completed directory");
            let parent = match stack.last() {
                Some(frame) => frame.entries.fd()?,
                None => parent_directory.as_fd(),
            };
            // Refuse a moved/replaced name before final rmdir. A swap after this
            // check cannot lose file contents: rmdir refuses nonempty objects.
            let current = rustix::fs::statat(parent, &completed.name, AtFlags::SYMLINK_NOFOLLOW)?;
            if current.st_dev != completed.identity.st_dev
                || current.st_ino != completed.identity.st_ino
            {
                bail!("archive directory name changed during deletion; replacement left untouched");
            }
            match rustix::fs::unlinkat(parent, &completed.name, AtFlags::REMOVEDIR) {
                Err(rustix::io::Errno::NOTEMPTY) if completed.rescans < REMOVAL_RESCANS => {
                    // Directory changes during iteration may skip entries on
                    // some filesystems. Reuse the verified descriptor; never
                    // reopen its name and inherit a replacement's authority.
                    completed.entries.rewind();
                    completed.rescans += 1;
                    stack.push(completed);
                }
                result => {
                    result?;
                    counts.directories += 1;
                }
            }
        }
    }
    Ok(())
}

/// Diagnostic paths never grant authority. Even a corrupt destination stays
/// visible to the user without being passed to a filesystem mutation.
fn diagnostic_path(row: &WorkingCopyRow) -> String {
    Path::new(&row.canonical_root)
        .join(working_copies::ARCHIVE_DIR_NAME)
        .join(row.archive_destination.as_deref().unwrap_or("<unknown>"))
        .display()
        .to_string()
}

/// A confirmed Delete can forget unusable ownership evidence without touching
/// the folder. Passive reads retain these failures so transient root conditions
/// cannot permanently hide recoverable archives.
fn should_forget(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<WorkingCopyError>()
            .is_some_and(|error| {
                matches!(
                    error,
                    WorkingCopyError::IdentityMismatch { .. }
                        | WorkingCopyError::DeviceChangedUnconfirmed { .. }
                        | WorkingCopyError::CreationTimeUnconfirmed { .. }
                        | WorkingCopyError::WrongState(_)
                ) || matches!(error, WorkingCopyError::Io(error) if error.kind() == std::io::ErrorKind::NotFound)
            })
            || cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    })
}

/// Read current on-disk trash, optionally measuring sizes under one budget.
/// Only missing archives beneath a verified root are pruned on a read. Other
/// failures recur until a deliberate Delete, so a count fetch cannot consume
/// the only diagnostic or forget archives on a temporarily unmounted volume.
pub(crate) fn list(db: &Db, measure_sizes: bool) -> anyhow::Result<CheckoutTrashListing> {
    let rows = working_copies::all_working_copies(&db.lock())?;
    let mut listing = CheckoutTrashListing::default();
    let deadline = Instant::now() + SIZE_BUDGET;
    let mut remaining = SIZE_ENTRY_CAP;
    for row in rows.into_iter().filter(|row| {
        row.allocation_state == AllocationState::Retired && row.archive_destination.is_some()
    }) {
        if working_copies::member_count(&db.lock(), &row.id)? != 0 {
            listing.issues.push(CheckoutTrashIssue {
                id: row.id.clone(),
                path: diagnostic_path(&row),
                message: "left untouched: a session still references this retired checkout".into(),
            });
            continue;
        }
        let opened = open_archive(db, &row, false);
        match opened {
            Ok(None) => working_copies::forget_retired(&db.lock(), &row.id)?,
            Ok(Some(archive)) => {
                let path = archive.path.clone();
                let bytes = measure_sizes
                    .then(|| disk_usage_dir(&archive.directory, deadline, &mut remaining))
                    .flatten();
                listing.checkouts.push(ArchivedCheckout {
                    id: row.id.clone(),
                    name: row
                        .archive_destination
                        .clone()
                        .expect("filtered archive name"),
                    repository: format!("{}/{}", row.repo_owner, row.repo_name),
                    path: path.display().to_string(),
                    archived_at: archived_at(
                        &row.original_basename,
                        row.archive_destination
                            .as_deref()
                            .expect("filtered archive name"),
                    ),
                    bytes,
                });
            }
            Err(error) => {
                listing.issues.push(CheckoutTrashIssue {
                    id: row.id.clone(),
                    path: diagnostic_path(&row),
                    message: format!("left untouched: {error:#}"),
                });
            }
        }
    }
    listing.checkouts.sort_by(|a, b| {
        b.archived_at
            .cmp(&a.archived_at)
            .then_with(|| b.name.cmp(&a.name))
    });
    Ok(listing)
}

/// Permanently delete exactly the selected terminal records that still match.
/// `session_directories` is captured under directory admission and includes
/// retained stopped sessions, since those can restart in their recorded cwd.
#[cfg(test)]
fn delete(
    db: &Db,
    ids: &[String],
    session_directories: &[PathBuf],
) -> anyhow::Result<CheckoutTrashDeleted> {
    delete_with(db, ids, session_directories, &mut remove_archive)
}

/// Request-wide measurements are repeated on every selected checkout's line.
/// The GUI sends one checkout per request. Database timings include waiting for
/// the shared connection, and total time starts before directory admission.
pub(crate) struct DeleteTiming {
    pub(crate) started: Instant,
    pub(crate) admission: Duration,
    pub(crate) session_paths: Duration,
    rows: Duration,
}

impl Default for DeleteTiming {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            admission: Duration::ZERO,
            session_paths: Duration::ZERO,
            rows: Duration::ZERO,
        }
    }
}

/// Report shared-read failures without converting them into per-record replies.
/// No archive path can be trusted before the registry read succeeds.
pub(crate) fn log_delete_read_failure(ids: &[String], timing: &DeleteTiming) {
    let mut seen = HashSet::new();
    for id in ids.iter().filter(|id| seen.insert(id.as_str())) {
        log_deletion(
            id,
            "",
            "failed",
            timing,
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
            &RemovalCounts::default(),
        );
    }
}

/// Emit the one completion event shared by success, refusal and partial failure.
/// Peer-controlled strings use Debug formatting so control characters stay data.
#[allow(clippy::too_many_arguments)]
fn log_deletion(
    id: &str,
    path: &str,
    outcome: &str,
    timing: &DeleteTiming,
    checks: Duration,
    opening: Duration,
    removal: Duration,
    update: Duration,
    counts: &RemovalCounts,
) {
    tracing::info!(
        checkout_id = ?id, path = ?path, outcome,
        total_ms = timing.started.elapsed().as_millis() as u64,
        admission_ms = timing.admission.as_millis() as u64,
        session_paths_ms = timing.session_paths.as_millis() as u64,
        working_copies_ms = timing.rows.as_millis() as u64,
        record_checks_ms = checks.as_millis() as u64,
        open_verify_ms = opening.as_millis() as u64,
        removal_ms = removal.as_millis() as u64,
        record_update_ms = update.as_millis() as u64,
        files_removed = counts.files, directories_removed = counts.directories,
        "permanent checkout trash deletion finished"
    );
}

/// Delete selected archives while preserving timings captured under admission.
/// Total time is measured from the request's start through each completion;
/// later entries in a batch therefore include time spent on earlier entries.
pub(crate) fn delete_timed(
    db: &Db,
    ids: &[String],
    session_directories: &[PathBuf],
    timing: DeleteTiming,
) -> anyhow::Result<CheckoutTrashDeleted> {
    delete_with_timing(db, ids, session_directories, timing, &mut remove_archive)
}

/// Keep result bookkeeping on the real delete path while tests inject only
/// the removal boundary. Per-record failures must not hide earlier outcomes.
#[cfg(test)]
fn delete_with(
    db: &Db,
    ids: &[String],
    session_directories: &[PathBuf],
    remove: &mut impl FnMut(&OpenArchive, &mut RemovalCounts) -> anyhow::Result<()>,
) -> anyhow::Result<CheckoutTrashDeleted> {
    delete_with_timing(
        db,
        ids,
        session_directories,
        DeleteTiming::default(),
        remove,
    )
}

/// Keep every early per-record exit inside the completion-log boundary.
/// A shared registry failure still returns the original request error, after
/// emitting one failure event per distinct id with an unavailable (empty) path.
fn delete_with_timing(
    db: &Db,
    ids: &[String],
    session_directories: &[PathBuf],
    mut timing: DeleteTiming,
    remove: &mut impl FnMut(&OpenArchive, &mut RemovalCounts) -> anyhow::Result<()>,
) -> anyhow::Result<CheckoutTrashDeleted> {
    let read = Instant::now();
    let rows = working_copies::all_working_copies(&db.lock());
    timing.rows = read.elapsed();
    let rows = rows.inspect_err(|_| log_delete_read_failure(ids, &timing))?;
    let rows: std::collections::HashMap<_, _> =
        rows.iter().map(|row| (row.id.as_str(), row)).collect();
    let mut reply = CheckoutTrashDeleted::default();
    let mut seen = HashSet::new();
    for id in ids.iter().filter(|id| seen.insert(id.as_str())) {
        let mut counts = RemovalCounts::default();
        let mut checks = Duration::ZERO;
        let mut opening = Duration::ZERO;
        let mut removal = Duration::ZERO;
        let mut update = Duration::ZERO;
        let mut status = "refused";
        let path = rows
            .get(id.as_str())
            .map(|row| diagnostic_path(row))
            .unwrap_or_default();
        let checked = Instant::now();
        (|| {
            let Some(row) = rows.get(id.as_str()).copied().filter(|row| {
                row.allocation_state == AllocationState::Retired
                    && row.archive_destination.is_some()
            }) else {
                reply.issues.push(CheckoutTrashIssue {
                    id: id.clone(),
                    path: String::new(),
                    message: "no recorded archive with this id; nothing was touched".into(),
                });
                checks = checked.elapsed();
                return;
            };
            let membership = working_copies::member_count(&db.lock(), id);
            checks = checked.elapsed();
            if !matches!(membership, Ok(0)) {
                status = if membership.is_err() {
                    "failed"
                } else {
                    "refused"
                };
                let message = match membership {
                    Ok(_) => {
                        "left untouched: a session still references this retired checkout".into()
                    }
                    Err(error) => {
                        format!(
                            "folder left untouched: could not check session references: {error:#}"
                        )
                    }
                };
                reply.issues.push(CheckoutTrashIssue {
                    id: id.clone(),
                    path: diagnostic_path(row),
                    message,
                });
                return;
            }
            let mut removal_started = false;
            let opened_at = Instant::now();
            let opened = open_archive(db, row, true);
            let outcome = opened.and_then(|archive| {
                let Some(archive) = archive else {
                    opening = opened_at.elapsed();
                    return Ok(());
                };
                let current_path = fs::canonicalize(&archive.path).ok();
                let legacy_path = Path::new(&row.canonical_root)
                    .join(working_copies::LEGACY_ARCHIVE_DIR_NAME)
                    .join(archive_name(row)?);
                // Migration moves the trash without rewriting sessions' recorded
                // working directories. Their old spelling still protects this tree.
                if session_directories.iter().any(|cwd| {
                    cwd.starts_with(&archive.path)
                        || cwd.starts_with(&legacy_path)
                        || current_path
                            .as_ref()
                            .is_some_and(|current| cwd.starts_with(current))
                }) {
                    bail!("a session uses this archive or a folder inside it");
                }
                removal_started = true;
                opening = opened_at.elapsed();
                let removing = Instant::now();
                let result = remove(&archive, &mut counts);
                removal = removing.elapsed();
                result
            });
            if !removal_started {
                opening = opened_at.elapsed();
            }
            match outcome {
                Ok(()) => {
                    let updating = Instant::now();
                    let forgotten = working_copies::forget_retired(&db.lock(), id);
                    update = updating.elapsed();
                    status = "failed";
                    match forgotten {
                    Ok(()) => {
                        status = if removal_started { "removed" } else { "record_dropped" };
                        reply.removed.push(id.clone());
                    },
                    Err(error) => reply.issues.push(CheckoutTrashIssue {
                        id: id.clone(), path: diagnostic_path(row),
                        message: format!("folder removed or already absent, but its trash record could not be dropped: {error:#}"),
                    }),
                }
                }
                Err(error) => {
                    status = if removal_started { "failed" } else { "refused" };
                    let forgotten = if should_forget(&error) {
                        let updating = Instant::now();
                        let result = working_copies::forget_retired(&db.lock(), id);
                        update = updating.elapsed();
                        match result {
                            Ok(()) => {
                                status = "record_dropped";
                                reply.removed.push(id.clone());
                                true
                            }
                            Err(bookkeeping) => {
                                status = "failed";
                                reply.issues.push(CheckoutTrashIssue {
                                id: id.clone(), path: diagnostic_path(row),
                                message: format!("folder left untouched; its trash record could not be dropped: {bookkeeping:#}"),
                            });
                                false
                            }
                        }
                    } else {
                        false
                    };
                    let message = if removal_started {
                        format!(
                            "permanent deletion did not complete; some contents may already be gone; what remains is at {}: {error:#}",
                            diagnostic_path(row)
                        )
                    } else if forgotten {
                        format!("folder left untouched; Farhelm no longer tracks it: {error:#}")
                    } else {
                        format!("folder left untouched: {error:#}")
                    };
                    reply.issues.push(CheckoutTrashIssue {
                        id: id.clone(),
                        path: diagnostic_path(row),
                        message,
                    });
                }
            }
        })();
        log_deletion(
            id, &path, status, &timing, checks, opening, removal, update, &counts,
        );
    }
    Ok(reply)
}

/// Convert the native stat block count without assuming its signedness.
/// macOS uses a signed field while Linux rustix uses an unsigned field.
#[allow(clippy::useless_conversion)]
fn allocated_bytes(metadata: &rustix::fs::Stat) -> Option<u64> {
    u64::try_from(metadata.st_blocks).ok()?.checked_mul(512)
}

/// Observe an entry without following links or accepting data from a mount.
/// Size accounting and spare-take preflight share this boundary: Linux O_PATH
/// observes a symlink itself and refuses even same-device file binds; macOS's
/// no-follow device check covers its mounts. This grants no delete authority.
pub(crate) fn entry_metadata(parent: impl AsFd, name: &CStr) -> std::io::Result<rustix::fs::Stat> {
    #[cfg(target_os = "linux")]
    {
        let fd = rustix::fs::openat2(
            parent,
            name,
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
            rustix::fs::ResolveFlags::NO_XDEV
                | rustix::fs::ResolveFlags::NO_SYMLINKS
                | rustix::fs::ResolveFlags::NO_MAGICLINKS,
        )?;
        Ok(rustix::fs::fstat(fd)?)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let metadata = rustix::fs::statat(parent.as_fd(), name, AtFlags::SYMLINK_NOFOLLOW)?;
        if metadata.st_dev != rustix::fs::fstat(parent)?.st_dev {
            return Err(rustix::io::Errno::XDEV.into());
        }
        Ok(metadata)
    }
}

/// Count disk blocks using opened directories and no-follow entry observations.
/// Any incomplete scan has no total. Mount boundaries likewise make the size
/// unknown: the confirmation must never count external mounted data as trash.
fn disk_usage_dir(directory: &fs::File, deadline: Instant, remaining: &mut usize) -> Option<u64> {
    if Instant::now() >= deadline || *remaining == 0 {
        return None;
    }
    *remaining -= 1;
    // Probe confinement even for empty trees and trees containing only files.
    // A listing may use the old-kernel identity fallback; a size must not imply
    // that the unavailable mount-boundary guarantee was actually exercised.
    let confined = open_child_dir(directory, c".", true).ok()?;
    let metadata = rustix::fs::fstat(&confined).ok()?;
    let mut identities = HashSet::from([(metadata.st_dev, metadata.st_ino)]);
    let mut bytes = allocated_bytes(&metadata)?;
    let mut stack = vec![Dir::new(confined).ok()?];
    while !stack.is_empty() {
        if Instant::now() >= deadline {
            return None;
        }
        let next = stack.last_mut()?.next();
        let Some(entry) = next else {
            stack.pop();
            continue;
        };
        let entry = entry.ok()?;
        let name = entry.file_name();
        if name.to_bytes() == b"." || name.to_bytes() == b".." {
            continue;
        }
        if *remaining == 0 {
            return None;
        }
        *remaining -= 1;
        let parent = stack.last()?.fd().ok()?;
        let metadata = entry_metadata(parent, name).ok()?;
        if identities.insert((metadata.st_dev, metadata.st_ino)) {
            bytes = bytes.checked_add(allocated_bytes(&metadata)?)?;
        }
        if rustix::fs::FileType::from_raw_mode(metadata.st_mode) == rustix::fs::FileType::Directory
        {
            if stack.len() >= MAX_DEPTH {
                return None;
            }
            let child = open_child_dir(parent, name, true).ok()?;
            let opened = rustix::fs::fstat(&child).ok()?;
            if opened.st_dev != metadata.st_dev || opened.st_ino != metadata.st_ino {
                return None;
            }
            stack.push(Dir::new(child).ok()?);
        }
    }
    Some(bytes)
}

/// Tests enter through a no-follow handle just as the verified listing does.
#[cfg(test)]
fn disk_usage(path: &Path, deadline: Instant, remaining: &mut usize) -> Option<u64> {
    use std::os::unix::fs::OpenOptionsExt;
    let directory = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .ok()?;
    disk_usage_dir(&directory, deadline, remaining)
}

/// Decode only the timestamp following the separately recorded original name.
/// Collision suffixes cannot be mistaken for date text; invalid dates get no
/// time rather than being normalized into another day.
fn archived_at(original: &str, destination: &str) -> Option<i64> {
    let suffix = destination.strip_prefix(original)?.strip_prefix('-')?;
    let stamp = suffix.get(..16)?;
    if stamp
        .bytes()
        .enumerate()
        .any(|(index, byte)| index != 8 && index != 15 && !byte.is_ascii_digit())
    {
        return None;
    }
    if stamp.as_bytes().get(8) != Some(&b'T') || stamp.as_bytes().get(15) != Some(&b'Z') {
        return None;
    }
    if suffix.len() != 16
        && !(suffix.len() == 49
            && suffix.as_bytes()[16] == b'-'
            && suffix[17..].bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return None;
    }
    let year: i64 = stamp.get(..4)?.parse().ok()?;
    let month: i64 = stamp.get(4..6)?.parse().ok()?;
    let day: i64 = stamp.get(6..8)?.parse().ok()?;
    let hour: i64 = stamp.get(9..11)?.parse().ok()?;
    let minute: i64 = stamp.get(11..13)?.parse().ok()?;
    let second: i64 = stamp.get(13..15)?.parse().ok()?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    // Hinnant's March-first civil-to-days conversion, inverse of the shared
    // formatter. Round-tripping rejects invalid leap days and month lengths.
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468;
    if farhelm_proto::time::civil_from_days(days) != (year, month as u32, day as u32) {
        return None;
    }
    Some(days * 86400 + hour * 3600 + minute * 60 + second)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::SessionStore;
    use crate::working_copies::PlannedWorkingCopy;
    use std::os::unix::fs::symlink;

    /// Real allocation and archival establish ownership evidence; tests do not
    /// fabricate matching inode numbers or bypass the registry's lifecycle.
    struct Fixture {
        _scratch: tempfile::TempDir,
        db: Db,
        root: PathBuf,
        archive: PathBuf,
        id: String,
    }

    impl Fixture {
        /// Build a retired checkout containing an observable uncommitted file.
        /// The kept original inode makes replacement tests independent of inode
        /// reuse and of whether this filesystem supports birth timestamps.
        async fn new() -> Self {
            let scratch = tempfile::tempdir().unwrap();
            let root = scratch.path().join("checkouts");
            fs::create_dir(&root).unwrap();
            let root = root.canonicalize().unwrap();
            let store = SessionStore::open(&scratch.path().join("sessions.sqlite"), true)
                .await
                .unwrap();
            let id = "recorded-archive".to_string();
            let archive = {
                let conn = store.conn.lock();
                working_copies::record_planned(
                    &conn,
                    &PlannedWorkingCopy {
                        id: id.clone(),
                        canonical_root: root.to_str().unwrap().into(),
                        repo_owner: "acme".into(),
                        repo_name: "project".into(),
                        original_basename: "project-1".into(),
                        origin_session_id: "deleted-origin".into(),
                        root_identity: None,
                        preparation_snapshot: None,
                    },
                )
                .unwrap();
                working_copies::allocate(&conn, &id, None).unwrap();
                fs::write(root.join("project-1/work"), b"uncommitted work").unwrap();
                let outcome = working_copies::archive_move(&conn, &id).unwrap();
                let working_copies::ArchiveOutcome::Archived { destination } = outcome else {
                    panic!("fixture archive must move");
                };
                working_copies::remove_member(&conn, "deleted-origin", &id).unwrap();
                working_copies::retire(&conn, &id).unwrap();
                root.join(working_copies::ARCHIVE_DIR_NAME)
                    .join(destination)
            };
            assert_eq!(fs::read(archive.join("work")).unwrap(), b"uncommitted work");
            assert_eq!(
                working_copies::get_working_copy(&store.conn.lock(), &id)
                    .unwrap()
                    .unwrap()
                    .allocation_state,
                AllocationState::Retired
            );
            Self {
                _scratch: scratch,
                db: store.conn.clone(),
                root,
                archive,
                id,
            }
        }

        /// Reproduce the old namespace without inventing ownership evidence.
        /// The same archived inode and bare destination survive the simulated
        /// upgrade, so the first list/delete must migrate before checking absence.
        async fn legacy() -> Self {
            let mut fixture = Self::new().await;
            let name = fixture.archive.file_name().unwrap().to_owned();
            let legacy = fixture.root.join(working_copies::LEGACY_ARCHIVE_DIR_NAME);
            fs::rename(fixture.root.join(working_copies::ARCHIVE_DIR_NAME), &legacy).unwrap();
            fixture.archive = legacy.join(name);
            assert!(fixture.archive.join("work").is_file());
            assert!(!fixture.root.join(working_copies::ARCHIVE_DIR_NAME).exists());
            fixture
        }
    }

    /// Completion logs expose the actual traversal's counts without changing
    /// deletion authority. A nested directory and symlink distinguish files
    /// from directories and prove that links are counted, not followed.
    #[farhelm_testtrace::test]
    async fn deletion_logs_one_completion_with_counts() {
        let fixture = Fixture::new().await;
        fs::create_dir(fixture.archive.join("nested")).unwrap();
        fs::write(fixture.archive.join("nested/file"), b"data").unwrap();
        std::os::unix::fs::symlink("work", fixture.archive.join("link")).unwrap();
        assert!(fixture.archive.join("nested/file").is_file());
        let reply = delete(&fixture.db, &[fixture.id.clone(), fixture.id.clone()], &[]).unwrap();
        assert_eq!(reply.removed, vec![fixture.id.clone()]);
        assert!(reply.issues.is_empty());
        assert!(!fixture.archive.exists());
        let lines = farhelm_testtrace::current_capture()
            .unwrap()
            .matching("permanent checkout trash deletion finished")
            .unwrap();
        assert_eq!(lines.len(), 1);
        let fields = &lines[0].fields;
        assert_eq!(
            fields.get("checkout_id"),
            Some(&format!("{:?}", fixture.id))
        );
        assert_eq!(
            fields.get("path"),
            Some(&format!("{:?}", fixture.archive.display().to_string()))
        );
        assert_eq!(fields.get("outcome").map(String::as_str), Some("removed"));
        assert_eq!(fields.get("files_removed").map(String::as_str), Some("3"));
        assert_eq!(
            fields.get("directories_removed").map(String::as_str),
            Some("2")
        );
        for field in [
            "total_ms",
            "admission_ms",
            "session_paths_ms",
            "working_copies_ms",
            "record_checks_ms",
            "open_verify_ms",
            "removal_ms",
            "record_update_ms",
        ] {
            assert!(fields.get(field).unwrap().parse::<u64>().is_ok(), "{field}");
        }
    }

    /// Refusals need the same completion trail as successful deletion, and
    /// peer-controlled ids must not inject physical lines into plain-text logs.
    #[farhelm_testtrace::test]
    async fn deletion_logs_refused_unknown_id_with_escaped_text() {
        let fixture = Fixture::new().await;
        let id = "unknown\ncheckout".to_string();
        let reply = delete(&fixture.db, std::slice::from_ref(&id), &[]).unwrap();
        assert!(reply.removed.is_empty());
        assert_eq!(reply.issues.len(), 1);
        assert!(fixture.archive.join("work").is_file());
        let lines = farhelm_testtrace::current_capture()
            .unwrap()
            .matching("permanent checkout trash deletion finished")
            .unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].fields.get("checkout_id"), Some(&format!("{id:?}")));
        assert_eq!(
            lines[0].fields.get("outcome").map(String::as_str),
            Some("refused")
        );
        assert_eq!(
            lines[0].fields.get("files_removed").map(String::as_str),
            Some("0")
        );
    }

    /// A root unavailable at startup must not lose legacy trash to a later
    /// passive read's missing-archive prune. Its first successful read migrates.
    #[farhelm_testtrace::test]
    async fn listing_migrates_legacy_trash_when_an_unreachable_root_returns() {
        let fixture = Fixture::legacy().await;
        let unavailable = fixture.root.with_extension("unavailable");
        fs::rename(&fixture.root, &unavailable).unwrap();
        let failed = list(&fixture.db, false).unwrap();
        assert!(failed.checkouts.is_empty());
        assert_eq!(failed.issues.len(), 1);
        assert!(
            working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
                .unwrap()
                .is_some()
        );
        fs::rename(&unavailable, &fixture.root).unwrap();
        let listing = list(&fixture.db, true).unwrap();
        assert!(listing.issues.is_empty());
        assert_eq!(listing.checkouts.len(), 1);
        assert_eq!(listing.checkouts[0].id, fixture.id);
        let migrated = fixture
            .root
            .join(working_copies::ARCHIVE_DIR_NAME)
            .join(fixture.archive.file_name().unwrap());
        assert_eq!(listing.checkouts[0].path, migrated.to_str().unwrap());
        assert_eq!(
            fs::read(migrated.join("work")).unwrap(),
            b"uncommitted work"
        );
        assert!(
            !fixture
                .root
                .join(working_copies::LEGACY_ARCHIVE_DIR_NAME)
                .exists()
        );
    }

    /// Delete can be the first post-upgrade operation; migration must precede
    /// its absence check rather than forgetting a still-present legacy archive.
    #[farhelm_testtrace::test]
    async fn deletion_migrates_legacy_trash_before_removing_the_selected_archive() {
        let fixture = Fixture::legacy().await;
        let deleted = delete(&fixture.db, std::slice::from_ref(&fixture.id), &[]).unwrap();
        assert!(deleted.issues.is_empty());
        assert_eq!(
            deleted.removed.as_slice(),
            std::slice::from_ref(&fixture.id)
        );
        assert!(
            !fixture
                .root
                .join(working_copies::LEGACY_ARCHIVE_DIR_NAME)
                .exists()
        );
        assert!(
            fs::read_dir(fixture.root.join(working_copies::ARCHIVE_DIR_NAME))
                .unwrap()
                .next()
                .is_none()
        );
        assert!(
            working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
                .unwrap()
                .is_none()
        );
    }

    /// A passive read never creates an empty trash directory, including when
    /// it legitimately prunes a recorded archive absent from both namespaces.
    #[farhelm_testtrace::test]
    async fn listing_missing_trash_does_not_create_a_new_namespace() {
        let fixture = Fixture::new().await;
        fs::remove_dir_all(fixture.root.join(working_copies::ARCHIVE_DIR_NAME)).unwrap();
        let listing = list(&fixture.db, false).unwrap();
        assert!(listing.checkouts.is_empty() && listing.issues.is_empty());
        assert!(!fixture.root.join(working_copies::ARCHIVE_DIR_NAME).exists());
        assert!(
            !fixture
                .root
                .join(working_copies::LEGACY_ARCHIVE_DIR_NAME)
                .exists()
        );
    }

    /// Migration does not rewrite retained sessions. Their old exact directory
    /// and subdirectory spellings must still prevent permanent deletion.
    #[farhelm_testtrace::test]
    async fn legacy_spelled_session_directories_protect_a_migrated_archive() {
        for subdirectory in [false, true] {
            let fixture = Fixture::legacy().await;
            let cwd = if subdirectory {
                fixture.archive.join("retained")
            } else {
                fixture.archive.clone()
            };
            fs::create_dir_all(&cwd).unwrap();
            let deleted = delete(&fixture.db, std::slice::from_ref(&fixture.id), &[cwd]).unwrap();
            assert!(deleted.removed.is_empty());
            assert_eq!(deleted.issues.len(), 1);
            assert!(
                deleted.issues[0]
                    .message
                    .contains("a session uses this archive")
            );
            let listing = list(&fixture.db, false).unwrap();
            assert_eq!(listing.checkouts.len(), 1);
            assert_eq!(
                fs::read(Path::new(&listing.checkouts[0].path).join("work")).unwrap(),
                b"uncommitted work"
            );
            assert!(!fixture.archive.exists());
        }
    }

    /// A historical active checkout at the legacy container must never move.
    /// Neither listing nor Delete may mistake its blocked migration for absence
    /// and forget a separately recorded archive still inside that directory.
    #[farhelm_testtrace::test]
    async fn blocked_legacy_migration_retains_the_archive_record_and_active_checkout() {
        let fixture = Fixture::new().await;
        let legacy = fixture.root.join(working_copies::LEGACY_ARCHIVE_DIR_NAME);
        let active = "historical-legacy-checkout";
        {
            let conn = fixture.db.lock();
            working_copies::record_planned(
                &conn,
                &PlannedWorkingCopy {
                    id: active.into(),
                    canonical_root: fixture.root.to_str().unwrap().into(),
                    repo_owner: "acme".into(),
                    repo_name: "old".into(),
                    original_basename: working_copies::LEGACY_ARCHIVE_DIR_NAME.into(),
                    origin_session_id: "retained-origin".into(),
                    root_identity: None,
                    preparation_snapshot: None,
                },
            )
            .unwrap();
            working_copies::allocate(&conn, active, None).unwrap();
        }
        fs::write(legacy.join("retained-work"), b"active checkout").unwrap();
        let archived = legacy.join(fixture.archive.file_name().unwrap());
        fs::rename(&fixture.archive, &archived).unwrap();
        fs::remove_dir(fixture.root.join(working_copies::ARCHIVE_DIR_NAME)).unwrap();
        let before = fs::metadata(&legacy).unwrap().ino();
        let listing = list(&fixture.db, false).unwrap();
        assert!(listing.checkouts.is_empty());
        assert_eq!(listing.issues.len(), 1);
        let deleted = delete(&fixture.db, std::slice::from_ref(&fixture.id), &[]).unwrap();
        assert!(deleted.removed.is_empty());
        assert_eq!(deleted.issues.len(), 1);
        assert!(
            working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
                .unwrap()
                .is_some()
        );
        assert_eq!(fs::metadata(&legacy).unwrap().ino(), before);
        assert_eq!(
            fs::read(legacy.join("retained-work")).unwrap(),
            b"active checkout"
        );
        assert_eq!(
            fs::read(archived.join("work")).unwrap(),
            b"uncommitted work"
        );
        assert!(!fixture.root.join(working_copies::ARCHIVE_DIR_NAME).exists());
    }

    /// A count is ownership-backed rather than a scan of arbitrary neighbours.
    /// Gone folders disappear from both the listing and its persisted records.
    #[farhelm_testtrace::test]
    async fn listing_counts_only_existing_recorded_archives_and_prunes_missing() {
        let fixture = Fixture::new().await;
        let neighbour = fixture
            .root
            .join(working_copies::ARCHIVE_DIR_NAME)
            .join("foreign");
        fs::create_dir(&neighbour).unwrap();
        fs::write(neighbour.join("work"), b"foreign work").unwrap();
        let listing = list(&fixture.db, false).unwrap();
        assert_eq!(listing.checkouts.len(), 1);
        assert_eq!(listing.checkouts[0].id, fixture.id);
        assert_eq!(listing.checkouts[0].bytes, None);
        assert_eq!(listing.checkouts[0].repository, "acme/project");
        assert!(listing.checkouts[0].archived_at.is_some());
        fs::remove_dir_all(&fixture.archive).unwrap();
        assert!(list(&fixture.db, false).unwrap().checkouts.is_empty());
        assert!(
            working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
                .unwrap()
                .is_none()
        );
        assert_eq!(fs::read(neighbour.join("work")).unwrap(), b"foreign work");
    }

    /// A path reused by another directory must never inherit the old archive's
    /// deletion authority. Keep the original inode alive to prove the premise.
    #[farhelm_testtrace::test]
    async fn listing_retains_a_replaced_archive_until_confirmed_deletion() {
        let fixture = Fixture::new().await;
        let original = fixture.archive.with_extension("kept-original");
        fs::rename(&fixture.archive, &original).unwrap();
        fs::create_dir(&fixture.archive).unwrap();
        fs::write(fixture.archive.join("foreign"), b"foreign work").unwrap();
        assert_ne!(
            fs::metadata(&original).unwrap().ino(),
            fs::metadata(&fixture.archive).unwrap().ino()
        );
        let listing = list(&fixture.db, true).unwrap();
        assert!(listing.checkouts.is_empty());
        assert_eq!(listing.issues.len(), 1);
        assert!(
            listing.issues[0]
                .path
                .contains(fixture.archive.file_name().unwrap().to_str().unwrap())
        );
        assert_eq!(
            fs::read(fixture.archive.join("foreign")).unwrap(),
            b"foreign work"
        );
        assert!(
            working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
                .unwrap()
                .is_some()
        );
        let deleted = delete(&fixture.db, std::slice::from_ref(&fixture.id), &[]).unwrap();
        assert_eq!(deleted.removed, vec![fixture.id.clone()]);
        assert_eq!(deleted.issues.len(), 1);
        assert_eq!(
            fs::read(fixture.archive.join("foreign")).unwrap(),
            b"foreign work"
        );
        assert!(
            working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
                .unwrap()
                .is_none()
        );
    }

    /// Retirement with a retained member is corrupt evidence, not permission
    /// to remove a folder. Refuse before mutation and keep both its record and
    /// sentinel even when the caller supplies no session cwd paths.
    #[farhelm_testtrace::test]
    async fn retained_membership_refuses_a_corrupt_retired_record() {
        let fixture = Fixture::new().await;
        working_copies::add_member(&fixture.db.lock(), "retained", &fixture.id).unwrap();
        assert_eq!(
            working_copies::member_count(&fixture.db.lock(), &fixture.id).unwrap(),
            1
        );
        let listing = list(&fixture.db, false).unwrap();
        assert!(listing.checkouts.is_empty());
        assert_eq!(listing.issues.len(), 1);
        let reply = delete(&fixture.db, std::slice::from_ref(&fixture.id), &[]).unwrap();
        assert!(reply.removed.is_empty());
        assert_eq!(reply.issues.len(), 1);
        assert!(reply.issues[0].message.contains("still references"));
        assert_eq!(
            fs::read(fixture.archive.join("work")).unwrap(),
            b"uncommitted work"
        );
        assert!(
            working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
                .unwrap()
                .is_some()
        );
    }

    /// Real OS confinement refuses a separate mount without mutating it.
    /// Assert the substrate premise first so a platform fixture change cannot
    /// turn a passing call into supposed mount-boundary coverage.
    #[farhelm_testtrace::test]
    fn confined_open_refuses_a_real_mount_boundary() {
        let root = fs::File::open("/").unwrap();
        #[cfg(target_os = "linux")]
        let child_name = c"proc";
        #[cfg(not(target_os = "linux"))]
        let child_name = c"dev";
        let child = Path::new("/").join(child_name.to_str().unwrap());
        assert_ne!(
            root.metadata().unwrap().dev(),
            fs::metadata(&child).unwrap().dev()
        );
        let error = open_child_dir(&root, child_name, true).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(libc::EXDEV));
        assert!(child.is_dir());
    }

    /// A reported mount refusal leaves the mounted subtree and record for a
    /// retry. The test seam acts at the real confined-open boundary; traversal
    /// and unlink operations still run on the actual archive descriptors.
    #[farhelm_testtrace::test]
    async fn removal_leaves_a_refused_mount_subtree_untouched() {
        let fixture = Fixture::new().await;
        let mount = fixture.archive.join("mounted");
        fs::create_dir(&mount).unwrap();
        fs::write(mount.join("sentinel"), b"external mounted work").unwrap();
        assert_eq!(
            fs::read(mount.join("sentinel")).unwrap(),
            b"external mounted work"
        );
        let mut boundary_observed = false;
        let reply = delete_with(
            &fixture.db,
            std::slice::from_ref(&fixture.id),
            &[],
            &mut |archive, counts| {
                remove_archive_with(archive, counts, &mut |parent, name| {
                    if name == c"mounted" {
                        boundary_observed = true;
                        Err(std::io::Error::from_raw_os_error(libc::EXDEV))
                    } else {
                        open_child_dir(parent, name, true)
                    }
                })
            },
        )
        .unwrap();
        assert!(boundary_observed);
        assert!(reply.removed.is_empty());
        assert_eq!(reply.issues.len(), 1);
        assert!(
            reply.issues[0]
                .message
                .contains("some contents may already be gone")
        );
        assert!(reply.issues[0].message.contains("mounted"));
        assert_eq!(
            fs::read(mount.join("sentinel")).unwrap(),
            b"external mounted work"
        );
        assert!(
            working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
                .unwrap()
                .is_some()
        );
    }

    /// Deep archives must fail before exhausting the supervisor's process-wide
    /// descriptors. The remaining subtree and ownership record permit recovery;
    /// an incomplete size must never become a complete confirmation total.
    #[farhelm_testtrace::test]
    async fn traversal_depth_preserves_descriptor_headroom() {
        let fixture = Fixture::new().await;
        let mut deepest = fixture.archive.clone();
        for _ in 0..MAX_DEPTH {
            deepest = deepest.join("nested");
            fs::create_dir(&deepest).unwrap();
        }
        fs::write(deepest.join("sentinel"), b"deep work").unwrap();
        assert_eq!(fs::read(deepest.join("sentinel")).unwrap(), b"deep work");
        assert_eq!(list(&fixture.db, true).unwrap().checkouts[0].bytes, None);
        let reply = delete(&fixture.db, std::slice::from_ref(&fixture.id), &[]).unwrap();
        assert!(reply.removed.is_empty());
        assert_eq!(reply.issues.len(), 1);
        assert!(reply.issues[0].message.contains("nesting exceeds"));
        assert_eq!(fs::read(deepest.join("sentinel")).unwrap(), b"deep work");
        assert!(
            working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
                .unwrap()
                .is_some()
        );
    }

    /// A post-removal bookkeeping error must preserve that checkout's outcome
    /// and continue reporting later selections. Inject corruption at the actual
    /// removal boundary so the guarded registry deletion really refuses.
    #[farhelm_testtrace::test]
    async fn bookkeeping_failure_is_reported_without_losing_batch_results() {
        let fixture = Fixture::new().await;
        let reply = delete_with(
            &fixture.db,
            &[fixture.id.clone(), "unknown-id".into()],
            &[],
            &mut |archive, counts| {
                remove_archive(archive, counts)?;
                working_copies::add_member(&fixture.db.lock(), "corrupt", &fixture.id)?;
                Ok(())
            },
        )
        .unwrap();
        assert!(!fixture.archive.exists());
        assert!(reply.removed.is_empty());
        assert_eq!(reply.issues.len(), 2);
        assert!(
            reply.issues[0]
                .message
                .contains("record could not be dropped")
        );
        assert_eq!(reply.issues[1].id, "unknown-id");
        assert!(
            working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
                .unwrap()
                .is_some()
        );
    }

    /// Renaming an ancestor after ownership verification must not transfer
    /// deletion authority to the replacement path. Open descriptors continue
    /// removing the original archive, and the stranger keeps its sentinel.
    #[farhelm_testtrace::test]
    async fn removal_uses_the_verified_handle_after_an_ancestor_replacement() {
        let fixture = Fixture::new().await;
        let row = working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
            .unwrap()
            .unwrap();
        let archive = open_archive(&fixture.db, &row, true).unwrap().unwrap();
        let old_root = fixture.root.with_extension("opened-original");
        fs::rename(&fixture.root, &old_root).unwrap();
        fs::create_dir(&fixture.root).unwrap();
        fs::create_dir(fixture.root.join(working_copies::ARCHIVE_DIR_NAME)).unwrap();
        fs::create_dir(&fixture.archive).unwrap();
        fs::write(fixture.archive.join("foreign"), b"keep this replacement").unwrap();
        assert_ne!(
            archive.directory.metadata().unwrap().ino(),
            fs::metadata(&fixture.archive).unwrap().ino()
        );
        assert_eq!(
            fs::read(fixture.archive.join("foreign")).unwrap(),
            b"keep this replacement"
        );
        remove_archive(&archive, &mut RemovalCounts::default()).unwrap();
        assert_eq!(
            fs::read(fixture.archive.join("foreign")).unwrap(),
            b"keep this replacement"
        );
        assert!(
            !old_root
                .join(working_copies::ARCHIVE_DIR_NAME)
                .join(archive.name.to_str().unwrap())
                .exists()
        );
    }

    /// A root can be temporarily unavailable, so passive reads retain its
    /// diagnostic. Explicit emptying may discard that unusable record without
    /// deleting either the old root or a replacement folder.
    #[farhelm_testtrace::test]
    async fn listing_retains_a_missing_root_until_confirmed_deletion() {
        let fixture = Fixture::new().await;
        let moved_root = fixture.root.with_extension("unavailable");
        fs::rename(&fixture.root, &moved_root).unwrap();
        assert!(!fixture.root.exists());
        assert_eq!(
            fs::read(
                moved_root
                    .join(working_copies::ARCHIVE_DIR_NAME)
                    .join(fixture.archive.file_name().unwrap())
                    .join("work")
            )
            .unwrap(),
            b"uncommitted work"
        );
        let listing = list(&fixture.db, false).unwrap();
        assert!(listing.checkouts.is_empty());
        assert_eq!(listing.issues.len(), 1);
        assert!(
            working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
                .unwrap()
                .is_some()
        );
        let reply = delete(&fixture.db, std::slice::from_ref(&fixture.id), &[]).unwrap();
        assert_eq!(reply.removed, vec![fixture.id.clone()]);
        assert_eq!(reply.issues.len(), 1);
        assert!(
            working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            fs::read(
                moved_root
                    .join(working_copies::ARCHIVE_DIR_NAME)
                    .join(fixture.archive.file_name().unwrap())
                    .join("work")
            )
            .unwrap(),
            b"uncommitted work"
        );
    }

    /// Permanent deletion must remove the archive, not its symlink targets or
    /// nearby folders. Both survivors carry sentinels checked after removal.
    #[farhelm_testtrace::test]
    async fn deletion_preserves_unrecorded_neighbours_and_symlink_targets() {
        let fixture = Fixture::new().await;
        let foreign = fixture.root.join("foreign");
        fs::create_dir(&foreign).unwrap();
        fs::write(foreign.join("sentinel"), b"keep").unwrap();
        symlink(&foreign, fixture.archive.join("outside")).unwrap();
        let neighbour = fixture.archive.with_extension("neighbour");
        fs::create_dir(&neighbour).unwrap();
        fs::write(neighbour.join("sentinel"), b"neighbour").unwrap();
        assert!(
            fs::symlink_metadata(fixture.archive.join("outside"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let reply = delete(&fixture.db, std::slice::from_ref(&fixture.id), &[]).unwrap();
        assert_eq!(reply.removed, vec![fixture.id.clone()]);
        assert!(reply.issues.is_empty());
        assert!(!fixture.archive.exists());
        assert_eq!(fs::read(foreign.join("sentinel")).unwrap(), b"keep");
        assert_eq!(fs::read(neighbour.join("sentinel")).unwrap(), b"neighbour");
        assert!(
            working_copies::get_working_copy(&fixture.db.lock(), &fixture.id)
                .unwrap()
                .is_none()
        );
    }

    /// Recovery sessions retain their cwd through a refused empty operation.
    /// The component boundary must not mistake a similarly prefixed neighbour
    /// for a session inside the archive.
    #[farhelm_testtrace::test]
    async fn deletion_refuses_a_session_inside_the_archive() {
        let fixture = Fixture::new().await;
        let nested = fixture.archive.join("recover");
        fs::create_dir(&nested).unwrap();
        let reply = delete(&fixture.db, std::slice::from_ref(&fixture.id), &[nested]).unwrap();
        assert!(reply.removed.is_empty());
        assert_eq!(reply.issues.len(), 1);
        assert!(reply.issues[0].message.contains("a session uses"));
        assert_eq!(
            fs::read(fixture.archive.join("work")).unwrap(),
            b"uncommitted work"
        );
        let neighbour = fixture.archive.with_extension("other");
        let reply = delete(&fixture.db, std::slice::from_ref(&fixture.id), &[neighbour]).unwrap();
        assert_eq!(reply.removed, vec![fixture.id]);
    }

    /// A stopped recovery session entered through a symlink still protects its
    /// actual archived folder. An unrelated cwd that is now a file must neither
    /// abort collection nor hide that protected directory.
    #[farhelm_testtrace::test]
    async fn stored_session_paths_protect_recovery_through_a_symlink() {
        let fixture = Fixture::new().await;
        let recovery = fixture.archive.join("recover");
        fs::create_dir(&recovery).unwrap();
        let alias = fixture.root.join("alias");
        symlink(&recovery, &alias).unwrap();
        assert_eq!(alias.canonicalize().unwrap(), recovery);
        let file_cwd = fixture.root.join("not-a-folder");
        fs::write(&file_cwd, b"file").unwrap();
        assert!(fs::canonicalize(file_cwd.join("nested")).is_err());
        let store = SessionStore {
            conn: fixture.db.clone(),
        };
        for (id, cwd, canonical) in [
            ("recovery", alias.clone(), Some(recovery.clone())),
            ("unrelated", file_cwd.join("nested"), None),
        ] {
            store
                .insert_session(
                    crate::store::StoredSession {
                        id: id.into(),
                        parent: None,
                        title: id.into(),
                        created_at: 1,
                        last_activity_at: 1,
                        last_work_started_at: 0,
                        creation_seq: 0,
                        cwd: cwd.display().to_string(),
                        launch: farhelm_proto::SessionLaunch::plain_command("exit 0"),
                        tmux_name: format!("fh-{id}"),
                        pane: String::new(),
                        outcome: crate::store::LastOutcome::Exited {
                            exit_code: Some(0),
                            annotation: Some(farhelm_proto::STOP_ANNOTATION.into()),
                        },
                        canonical_cwd: canonical.map(|path| path.display().to_string()),
                        captured_conversation: None,
                        generation: 0,
                        launch_scoped: false,
                        launch_hooked: false,
                        launch_boot_ended: false,
                        conversation_source: None,
                        capture_ownership_version: 0,
                        omp_reporter_asset: None,
                        omp_launch_program: None,
                    },
                    None,
                )
                .await
                .unwrap();
        }
        assert!(store.session("recovery").await.unwrap().is_some());
        let paths = crate::store::session_directory_paths(&fixture.db.lock()).unwrap();
        assert!(paths.contains(&alias));
        assert!(paths.contains(&recovery));
        assert!(paths.contains(&file_cwd.join("nested")));
        let reply = delete(&fixture.db, std::slice::from_ref(&fixture.id), &paths).unwrap();
        assert!(reply.removed.is_empty());
        assert_eq!(reply.issues.len(), 1);
        assert!(reply.issues[0].message.contains("a session uses"));
        assert_eq!(
            fs::read(fixture.archive.join("work")).unwrap(),
            b"uncommitted work"
        );
    }

    /// Both a symlink at the archive name and a replaced root are foreign,
    /// even when the old checkout itself is still present under that root.
    #[farhelm_testtrace::test]
    async fn deletion_refuses_symlink_archives_and_replaced_roots() {
        for replace_root in [false, true] {
            let fixture = Fixture::new().await;
            if replace_root {
                let original = fixture.root.with_extension("original");
                fs::rename(&fixture.root, &original).unwrap();
                fs::create_dir(&fixture.root).unwrap();
                fs::create_dir(fixture.root.join(working_copies::ARCHIVE_DIR_NAME)).unwrap();
                fs::rename(
                    original
                        .join(working_copies::ARCHIVE_DIR_NAME)
                        .join(fixture.archive.file_name().unwrap()),
                    &fixture.archive,
                )
                .unwrap();
                assert_ne!(
                    fs::metadata(&original).unwrap().ino(),
                    fs::metadata(&fixture.root).unwrap().ino()
                );
            } else {
                let original = fixture.archive.with_extension("original");
                fs::rename(&fixture.archive, &original).unwrap();
                symlink(&original, &fixture.archive).unwrap();
                assert!(
                    fs::symlink_metadata(&fixture.archive)
                        .unwrap()
                        .file_type()
                        .is_symlink()
                );
            }
            let reply = delete(&fixture.db, std::slice::from_ref(&fixture.id), &[]).unwrap();
            assert_eq!(reply.removed, vec![fixture.id]);
            assert_eq!(reply.issues.len(), 1);
            assert_eq!(
                fs::read(fixture.archive.join("work")).unwrap(),
                b"uncommitted work"
            );
        }
    }

    /// Disk usage counts allocated blocks once for hardlinks and does not
    /// inspect a symlink's target. Budget exhaustion yields unknown, not a
    /// partial byte count that the dialog might present as complete.
    #[farhelm_testtrace::test]
    async fn sizes_use_disk_blocks_and_have_one_finite_scan_budget() {
        let fixture = Fixture::new().await;
        let outside = fixture.root.join("outside");
        fs::write(&outside, vec![0_u8; 65536]).unwrap();
        fs::hard_link(
            fixture.archive.join("work"),
            fixture.archive.join("hardlink"),
        )
        .unwrap();
        symlink(&outside, fixture.archive.join("link")).unwrap();
        let names: std::collections::BTreeSet<_> = fs::read_dir(&fixture.archive)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            names,
            ["work", "hardlink", "link"]
                .into_iter()
                .map(std::ffi::OsString::from)
                .collect()
        );
        let expected = fs::symlink_metadata(&fixture.archive).unwrap().blocks() * 512
            + fs::symlink_metadata(fixture.archive.join("work"))
                .unwrap()
                .blocks()
                * 512
            + fs::symlink_metadata(fixture.archive.join("link"))
                .unwrap()
                .blocks()
                * 512;
        let listing = list(&fixture.db, true).unwrap();
        assert_eq!(listing.checkouts[0].bytes, Some(expected));
        assert_eq!(disk_usage(&fixture.archive, Instant::now(), &mut 100), None);
        assert_eq!(
            disk_usage(&fixture.archive, Instant::now() + SIZE_BUDGET, &mut 1),
            None
        );
    }

    /// Archived time comes from a recorded rename, not the directory's mtime
    /// (which recovered work can change). Names may contain their own dates.
    #[farhelm_testtrace::test]
    fn archive_time_requires_the_recorded_prefix_and_a_valid_calendar_date() {
        assert_eq!(
            archived_at("project-1", "project-1-19700101T000000Z"),
            Some(0)
        );
        assert_eq!(
            archived_at(
                "project-20240101T000000Z",
                "project-20240101T000000Z-20240229T120000Z"
            ),
            Some(1709208000)
        );
        assert_eq!(
            archived_at(
                "project",
                "project-20240229T120000Z-0123456789abcdef0123456789abcdef"
            ),
            Some(1709208000)
        );
        for name in [
            "other-20240229T120000Z",
            "project-20230229T120000Z",
            "project-20241301T000000Z",
            "project-20240101T240000Z",
            "project-20240101T000000Z-bad",
        ] {
            assert_eq!(archived_at("project", name), None, "{name}");
        }
    }
}

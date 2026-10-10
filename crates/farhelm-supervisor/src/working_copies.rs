//! The supervisor's durable working-copy registry: which directories under
//! a canonical repo root it allocated, for whom, and the filesystem
//! identities that make those rows trustworthy.
//!
//! This module IS the ownership-evidence contract for supervisor-managed
//! working copies (schema 18's `working_copies` / `working_copy_members`
//! tables; see `store`'s migration docs). Every invariant below exists so
//! that the supervisor can later clean up against these directories
//! WITHOUT ever touching a directory it did not allocate — the no-adoption
//! policy. Identity evidence is captured at allocation time and checked
//! before every destructive act:
//!
//! - **One-winner mkdir.** [`allocate`] creates the directory with
//!   exclusive `mkdir(2)` semantics: under any race exactly one caller
//!   wins and every loser gets [`WorkingCopyError::Conflict`]. There is
//!   never a moment where two rows could claim the same directory.
//! - **Identity capture.** The winner immediately records the directory's
//!   `(device, inode)` and its canonicalized path in the same SQLite
//!   transaction that flips the row to `allocated` and attaches the
//!   allocating session as a member. Later steps trust the ROW, never the
//!   path: [`verify_identity`] and the archive primitives compare fresh
//!   `stat` results against the captured `(dev, ino)` and fail closed on
//!   any mismatch. A path that merely looks right (a recreated directory,
//!   a swapped name) is a different object and is never operated on.
//! - **Durability before journal retirement.** The archive move persists
//!   its destination (the rename journal) BEFORE renaming, and
//!   [`reconcile_archive`] refuses to let the journal retire until the
//!   moved directory's identity has been re-verified and both parent
//!   directories re-fsynced. The teardown slice retires the journal only
//!   after its final SQLite commit; this module provides the [`retire`]
//!   primitive.
//! - **Explicit, transactional membership.** Lifetime association between
//!   sessions and a working copy lives in `working_copy_members` rows,
//!   never a counter and never a column: inserts and removals go through
//!   [`add_member`] / [`remove_member`], and the create-path caller
//!   composes them into the SAME transaction as its session writes.
//!
//! **Transaction composition.** The SQLite-touching functions here all
//! take `&rusqlite::Connection` rather than owning transactions. A
//! rusqlite `Transaction` derefs to `Connection`, so the create-path
//! slice can open one transaction and call [`record_planned`],
//! [`add_member`], and its own session-row insert through the same
//! handle — one commit covers all of it, which is exactly the atomicity
//! the membership table's no-foreign-keys convention demands. The
//! filesystem-acting functions ([`allocate`], [`archive_move`],
//! [`reconcile_archive`]) manage their own short transactions internally
//! because they must commit only after the filesystem step has actually
//! succeeded.
//!
//! Registry rows are private persistence. The preparation snapshot carries
//! the original hook and shell plus the first-publication boundary; it must
//! never be exposed through public session metadata or diagnostics.
//!
//! **Startup reconciliation.** A supervisor that finds `archive_pending`
//! rows must call [`reconcile_archive`] for each BEFORE accepting new
//! directory admissions, so no concurrent allocation can observe a
//! half-moved directory.

use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;

use crate::store::now_unix;
use rusqlite::Connection;
use thiserror::Error;

/// Fixed name of the archive directory under a canonical root. Occupancy
/// scans skip it exactly, and the archive move requires it to be a real
/// same-device directory.
pub const ARCHIVE_DIR_NAME: &str = "farhelm-archived-working-copies";

/// The longest checkout path, in bytes, that admission accepts: short enough
/// that its longest archive destination still fits the platform's path limit.
///
/// Archiving turns `<root>/<name>` into
/// `<root>/farhelm-archived-working-copies/<name>-<YYYYMMDDTHHMMSSZ>`, and
/// after a name collision appends `-<32 hex digits>` as well, 82 bytes
/// longer in all. A checkout admitted within that margin of the limit could
/// never be archived: every rename fails with `ENAMETOOLONG`. The limit is
/// the platform's `PATH_MAX` less its terminating NUL (4096 bytes on Linux,
/// 1024 on macOS), because the archive rename passes both full paths to one
/// system call.
pub const MAX_ADMITTED_CHECKOUT_PATH: usize = libc::PATH_MAX as usize - 1 - ARCHIVE_PATH_GROWTH;

/// How many bytes archiving can add to a checkout path: a separator and the
/// archive folder, the `-<UTC timestamp>` suffix, and one collision suffix.
const ARCHIVE_PATH_GROWTH: usize = 1 + ARCHIVE_DIR_NAME.len() + "-YYYYMMDDTHHMMSSZ".len() + 1 + 32;
// SPEC.md and the docs above quote this figure; keep them in step.
const _: () = assert!(ARCHIVE_PATH_GROWTH == 82);

/// Hard ceiling on directory entries an occupancy scan will inspect. A
/// root at or beyond this size refuses to answer rather than reporting a
/// wrong "lowest free" name (naming two working copies alike would break
/// the one-winner contract at a layer mkdir cannot repair).
pub const OCCUPANCY_SCAN_CAP: usize = 100_000;

/// Bound on no-replace rename attempts during an archive move before
/// failing closed with the row left in the journal state for
/// [`reconcile_archive`].
const ARCHIVE_ATTEMPTS: usize = 8;

pub type Result<T> = std::result::Result<T, WorkingCopyError>;

/// The allocation effect whose failure a test injects. Each variant sits
/// directly beside the operation it names, so tests exercise the same
/// post-mkdir retention path production uses rather than a synthetic phase
/// marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllocationStage {
    /// The newly created directory's parent must be durable before SQLite
    /// can say the allocation exists.
    BeforeParentFsync,
}

/// An optional fault at a real allocation effect. Production supplies no
/// fault; tests use it to prove post-mkdir failures retain diagnostic
/// evidence without ever claiming an identity they did not capture.
pub type AllocationFault = Arc<dyn Fn(AllocationStage) -> anyhow::Result<()> + Send + Sync>;

/// Whether a failed allocation is positively known to precede `mkdir`, or
/// may have created the planned directory. Cleanup may roll back only the
/// first case: error wrapping cannot substitute for evidence about a
/// filesystem side effect.
#[derive(Debug, Error)]
pub enum AllocationFailure {
    /// Validation or an `EEXIST` collision completed before a successful
    /// mkdir. The colliding object remains foreign.
    #[error("allocation failed before mkdir: {0}")]
    PreMkdir(#[source] WorkingCopyError),
    /// mkdir succeeded and later identity, durability, or SQL work failed.
    #[error("allocation failed after mkdir: {0}")]
    PostMkdir(#[source] WorkingCopyError),
    /// The blocking allocation task did not return a result, so mkdir's
    /// effect cannot be proven absent.
    #[error("allocation outcome is uncertain: {0:#}")]
    Uncertain(#[source] anyhow::Error),
}

/// The `stat` fingerprint that makes a filesystem path a specific object
/// rather than a name: `(st_dev, st_ino)`.
pub type DirectoryIdentity = (u64, u64);

/// The lifecycle of a registered working copy, stored as
/// `allocation_state` TEXT. Transitions are explicit and each is owned by
/// exactly one function in this module: `planned` → `allocated`
/// ([`allocate`]), `allocated` → `archive_pending` ([`archive_move`]'s
/// journal persist), `archive_pending` → `retired` ([`retire`], called by
/// the teardown slice after its final SQLite transaction).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllocationState {
    /// mkdir not yet done, or done before a crash lost the identity
    /// capture — the row claims nothing about the filesystem yet.
    Planned,
    /// Exclusive mkdir succeeded and `(dev, ino)` + canonical path were
    /// captured in one transaction.
    Allocated,
    /// A rename destination is journaled on the row; the rename may or
    /// may not have happened. Only [`reconcile_archive`] resolves this
    /// state forward.
    ArchivePending,
    /// Post-archival, or explicitly retired. Terminal.
    Retired,
}

impl AllocationState {
    pub fn as_str(self) -> &'static str {
        match self {
            AllocationState::Planned => "planned",
            AllocationState::Allocated => "allocated",
            AllocationState::ArchivePending => "archive_pending",
            AllocationState::Retired => "retired",
        }
    }

    fn from_str(raw: &str) -> Result<Self> {
        match raw {
            "planned" => Ok(AllocationState::Planned),
            "allocated" => Ok(AllocationState::Allocated),
            "archive_pending" => Ok(AllocationState::ArchivePending),
            "retired" => Ok(AllocationState::Retired),
            _ => Err(WorkingCopyError::UnknownState(raw.to_string())),
        }
    }
}

/// One row of the `working_copies` table.
#[derive(Clone, Debug)]
pub struct WorkingCopyRow {
    pub id: String,
    /// The canonical repo root the directory lives under, exactly as
    /// recorded at planning time.
    pub canonical_root: String,
    /// The directory's canonicalized path, captured at allocation time;
    /// `None` until then.
    pub canonical_path: Option<String>,
    /// The validated lowercase GitHub owner/name pair, as persisted.
    pub repo_owner: String,
    pub repo_name: String,
    /// The name the directory is allocated under (already carries any
    /// `-N` numeric suffix the occupancy scan chose).
    pub original_basename: String,
    /// Provenance, immutable: the session that first allocated this
    /// directory. CURRENT lifetime association lives in
    /// `working_copy_members`, not here — that separation is why both
    /// exist (see store's 17→18 migration docs).
    pub origin_session_id: String,
    /// Identity of `canonical_root` captured before allocation — evidence that
    /// the root itself was not swapped underneath the registry.
    pub root_identity: Option<DirectoryIdentity>,
    /// Identity of the allocated directory itself. The fingerprint every
    /// destructive act verifies against.
    pub path_identity: Option<DirectoryIdentity>,
    /// Birth time (nanoseconds since the epoch) of `canonical_root` when the
    /// directory was allocated, or `None`: a row recorded before birth times
    /// were kept, or a filesystem that reports none.
    ///
    /// `(dev, ino)` alone cannot tell a directory from one recreated at the
    /// same path, because filesystems reuse inode numbers routinely (on
    /// ext4 a removed-and-recreated directory commonly gets the same one).
    /// The birth time is what changes. See [`same_directory`] for the rule.
    pub root_birth_ns: Option<i64>,
    /// Birth time of the allocated directory itself; see `root_birth_ns`.
    pub path_birth_ns: Option<i64>,
    pub allocation_state: AllocationState,
    /// The journaled rename destination while `archive_pending` (kept for
    /// diagnostics afterwards).
    pub archive_destination: Option<String>,
    /// Private create-time resolution and preparation-publication boundary.
    /// Never expose this JSON in diagnostics: it contains the configured hook.
    pub preparation_snapshot: Option<String>,
    pub created_at: i64,
}

/// Result of answering "what is at the row's canonical path right now?".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityStatus {
    /// The object at the recorded path has the recorded `(dev, ino)`.
    Matches,
    /// A DIFFERENT object sits at the recorded path. Fail closed: never
    /// adopt, never move, never remove.
    DifferentObject,
    /// The object at the recorded path has the recorded inode but a
    /// different device number, and no birth time confirms it is the same
    /// folder (see [`same_directory`]). Fails closed like
    /// `DifferentObject`, but a refusal should say that a remount may be
    /// the cause rather than that the folder was replaced.
    DeviceChangedUnconfirmed,
    /// Nothing at the recorded path.
    Missing,
    /// The row has no captured identity yet (a `planned` row), so there
    /// is nothing to verify against.
    NoCapturedIdentity,
}

/// What [`allocate`] did: won the exclusive mkdir and captured identity.
#[derive(Clone, Debug)]
pub struct AcceptedDirectory {
    pub row: WorkingCopyRow,
    /// The `(dev, ino)` the in-terminal launcher must find at the path:
    /// captured by the mkdir on a first attempt, or as observed moments ago
    /// by [`verify_identity_observed`] on a retry. Not necessarily the
    /// identity the row recorded, whose device number a remount may have
    /// changed since.
    pub identity: DirectoryIdentity,
    pub canonical_path: PathBuf,
    /// [`Self::canonical_path`] as text, which the session row records as
    /// its working directory and identity. Checked rather than converted
    /// lossily: [`allocate`] refuses a canonical path that is not valid
    /// UTF-8 (SPEC.md "Paths that are not valid UTF-8").
    pub canonical_text: String,
}

/// What [`archive_move`] did.
#[derive(Clone, Debug)]
pub enum ArchiveOutcome {
    /// The rename happened; `destination` is the persisted final name.
    Archived { destination: String },
    /// The source is already gone. A visible outcome, not an error: the
    /// cleanup diagnostic may proceed and the metadata may be deleted.
    SourceMissing,
}

/// What [`reconcile_archive`] resolved a crashed `archive_pending` row
/// into. Startup reconciliation must call this for every pending row
/// before accepting new directory admissions.
#[derive(Clone, Debug)]
pub enum ReconcileOutcome {
    /// The rename was completed during reconciliation — either retried
    /// against the journaled destination, or moved under a new suffix
    /// after an unrelated object was found occupying it.
    Moved { destination: String },
    /// The rename had already happened before the crash: the
    /// destination's identity was verified against the row and both
    /// parent directories were re-fsynced. The caller may now retire the
    /// journal (the retirement transaction belongs to the teardown
    /// slice).
    MetadataComplete,
    /// Source and destination are both absent. Metadata deletion is
    /// permitted: the journal is only ever written after identity
    /// verification, so a crash cannot have hidden a live directory here.
    SourceMissing,
}

#[derive(Error, Debug)]
pub enum WorkingCopyError {
    /// The exclusive mkdir lost its race (or the target name was occupied
    /// in any other way). Normal, expected, and creates no duplicate
    /// anywhere else.
    #[error("working-copy path already exists: {path}")]
    Conflict { path: PathBuf },
    /// The occupancy scan hit its entry cap; the lowest free number would
    /// be a lie, so it refuses instead.
    #[error(
        "occupancy scan exceeded the {scanned}-entry cap; refusing to claim a lowest free name"
    )]
    IncompleteScan { scanned: usize },
    /// The object at a path of interest is not the object the row
    /// captured. Destructive acts fail closed.
    #[error("identity mismatch at {path}: refusing to act on an object the row does not own")]
    IdentityMismatch { path: PathBuf },
    /// The object at a path of interest has the captured inode but a new
    /// device number, and no birth time can confirm it is the same object
    /// (see `same_directory`). Fails closed like `IdentityMismatch`.
    #[error(
        "the device number of {path} changed since it was recorded, as a reboot or remount can do \
         on btrfs, NFS or overlayfs, and this filesystem records no creation time to confirm it is \
         still the same folder; refusing to act on it"
    )]
    DeviceChangedUnconfirmed { path: PathBuf },
    /// Overlapping active records are inconsistent ownership evidence, even
    /// when each inode matches. Moving either would invalidate the other.
    #[error(
        "working-copy path {path} overlaps active record {other_id}; refusing automated archival"
    )]
    OverlappingRecord { path: PathBuf, other_id: String },
    /// The checkout being archived IS (or contains, or sits inside) its
    /// root's archive directory. Moving a directory into its own
    /// subdirectory is impossible (the kernel answers `EINVAL`), so this is
    /// named up front instead of surfacing as a raw rename error forever.
    #[error(
        "the checkout {path} occupies its root's reserved archive directory, so it cannot be \
         archived into it; move or rename it by hand"
    )]
    SourceIsArchiveRoot { path: PathBuf },
    /// Another active managed checkout is, contains, or sits inside the
    /// archive directory. Archiving would move this checkout into that one.
    #[error(
        "the archive directory {path} overlaps active managed checkout {other_id}; refusing to \
         archive into another checkout"
    )]
    ArchiveRootIsCheckout { path: PathBuf, other_id: String },
    #[error("the archive directory {path} is a symlink; refusing to archive through it")]
    ArchiveRootSymlink { path: PathBuf },
    #[error("the archive directory {path} is not a directory")]
    ArchiveRootNotDirectory { path: PathBuf },
    #[error("the archive directory {path} is on a different filesystem than the canonical root")]
    ArchiveRootForeignDevice { path: PathBuf },
    #[error("the rename destination stayed occupied after {attempts} attempts")]
    DestinationExhausted { attempts: usize },
    /// The filesystem refused the no-overwrite rename outright, so the
    /// checkout did not move and its journal was rolled back to
    /// `allocated`. See [`rename_refused_without_moving`] for which errors
    /// count; a filesystem that lacks no-overwrite rename answers this way
    /// on every attempt.
    #[error("the filesystem refused to move {path} into the archive, so it was not moved: {cause}")]
    RenameRefused {
        path: PathBuf,
        cause: std::io::Error,
    },
    #[error("working-copy row {0} is not in the state this operation requires")]
    WrongState(String),
    #[error("working-copy row {0} not found")]
    RowMissing(String),
    /// One create cannot originate two checkouts. Picking either record
    /// would turn corrupt provenance into permission to prepare a directory.
    #[error("multiple working copies record session {0} as their origin")]
    AmbiguousOrigin(String),
    #[error("unrecognized allocation_state: {0}")]
    UnknownState(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Stat a path's `(device, inode)` without following symlinks.
fn identity_of(path: &Path) -> Result<DirectoryIdentity> {
    let meta = fs::symlink_metadata(path)?;
    Ok((meta.dev(), meta.ino()))
}

/// What one no-follow stat of a path observed: its identity, whether it
/// is a directory, and its birth time when the filesystem reports one.
///
/// All three come from the SAME system call, so they describe one object
/// even if the path is being replaced concurrently.
#[derive(Clone, Copy, Debug)]
struct Observed {
    dev: u64,
    ino: u64,
    is_dir: bool,
    /// Nanoseconds since the epoch; `None` when the filesystem (or, on
    /// Linux, a kernel older than `statx`) does not report a birth time.
    birth_ns: Option<i64>,
}

impl Observed {
    fn identity(&self) -> DirectoryIdentity {
        (self.dev, self.ino)
    }
}

/// Stat `path` without following symlinks, including its birth time.
///
/// Linux asks `statx` for `STATX_BTIME` directly rather than going through
/// `Metadata::created`: the standard library only uses `statx` on glibc,
/// and on musl, which is what the released Linux binaries are built
/// against, `created` always reports "unsupported". The `libc` crate gates
/// its own `statx` binding on musl behind a cfg this build does not set, so
/// the syscall is made directly with the kernel's fixed, versioned struct
/// layout. A kernel without `statx` falls back to plain `lstat` with no
/// birth time.
#[cfg(target_os = "linux")]
fn observe(path: &Path) -> std::io::Result<Observed> {
    use std::os::unix::ffi::OsStrExt as _;

    /// `struct statx_timestamp` from `<linux/stat.h>`.
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct StatxTimestamp {
        tv_sec: i64,
        tv_nsec: u32,
        reserved: i32,
    }
    /// `struct statx` from `<linux/stat.h>`: 256 bytes, fields fixed by the
    /// kernel ABI and extended only into the trailing spare space.
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Statx {
        mask: u32,
        blksize: u32,
        attributes: u64,
        nlink: u32,
        uid: u32,
        gid: u32,
        mode: u16,
        spare0: u16,
        ino: u64,
        size: u64,
        blocks: u64,
        attributes_mask: u64,
        atime: StatxTimestamp,
        btime: StatxTimestamp,
        ctime: StatxTimestamp,
        mtime: StatxTimestamp,
        rdev_major: u32,
        rdev_minor: u32,
        dev_major: u32,
        dev_minor: u32,
        spare2: [u64; 14],
    }
    const _: () = assert!(std::mem::size_of::<Statx>() == 256);
    const STATX_TYPE: u32 = 0x0001;
    const STATX_INO: u32 = 0x0100;
    const STATX_BTIME: u32 = 0x0800;

    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let mut buf = Statx::default();
    // SAFETY: `c_path` is a valid NUL-terminated string and `buf` is a
    // writable, correctly sized `struct statx` for the duration of the call.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_statx,
            libc::AT_FDCWD,
            c_path.as_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
            STATX_TYPE | STATX_INO | STATX_BTIME,
            &mut buf as *mut Statx,
        )
    };
    if rc != 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOSYS) {
            let meta = fs::symlink_metadata(path)?;
            return Ok(Observed {
                dev: meta.dev(),
                ino: meta.ino(),
                is_dir: meta.is_dir(),
                birth_ns: None,
            });
        }
        return Err(error);
    }
    let birth_ns = (buf.mask & STATX_BTIME != 0)
        .then(|| {
            buf.btime
                .tv_sec
                .checked_mul(1_000_000_000)?
                .checked_add(i64::from(buf.btime.tv_nsec))
        })
        .flatten();
    Ok(Observed {
        dev: libc::makedev(buf.dev_major, buf.dev_minor),
        ino: buf.ino,
        is_dir: u32::from(buf.mode) & libc::S_IFMT == libc::S_IFDIR,
        birth_ns,
    })
}

/// Stat `path` without following symlinks, including its birth time
/// (`st_birthtime`, which `Metadata::created` reads on macOS).
#[cfg(not(target_os = "linux"))]
fn observe(path: &Path) -> std::io::Result<Observed> {
    let meta = fs::symlink_metadata(path)?;
    let birth_ns = meta
        .created()
        .ok()
        .and_then(|created| created.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|since| i64::try_from(since.as_nanos()).ok());
    Ok(Observed {
        dev: meta.dev(),
        ino: meta.ino(),
        is_dir: meta.is_dir(),
        birth_ns,
    })
}

/// Whether `observed` is the directory a row recorded: the same inode and,
/// when a birth time was recorded, the same birth time; without a recorded
/// birth time, the same device as well.
///
/// The one rule every ownership check in this module applies, because
/// inode numbers are reused: a directory removed and recreated at the same
/// path (a user re-cloning over a checkout Farhelm made, say) can carry the
/// old `(dev, ino)`, and a match on those alone would let Delete archive the
/// user's new folder. A recorded birth time that differs, or cannot be read
/// now, means a different object. A row without one (recorded before birth
/// times were kept, or on a filesystem without them) falls back to
/// `(dev, ino)` alone: the accepted residual for those rows.
///
/// The device number is deliberately NOT required to match when a birth time
/// did. btrfs subvolumes (Fedora's default `/home`), NFS, overlayfs and some
/// device-mapper setups assign device numbers when the filesystem is mounted,
/// so a reboot or remount can change it while the folder is untouched. Those
/// are ordinary, supported setups, and requiring the old number made the
/// session that created such a checkout unrestartable for good. Inode plus
/// birth time still tells a folder replaced at the same path from the
/// original, which is what the check exists for. Without a birth time there
/// is nothing to confirm a changed device number with, so that case stays a
/// mismatch; [`device_change_unconfirmed`] names it for the refusal.
fn same_directory(observed: &Observed, identity: DirectoryIdentity, birth: Option<i64>) -> bool {
    let (dev, ino) = identity;
    observed.ino == ino
        && match birth {
            Some(recorded) => observed.birth_ns == Some(recorded),
            None => observed.dev == dev,
        }
}

/// Whether `observed` fails [`same_directory`] only because its device
/// number changed, with no birth time on one side to confirm it is the same
/// folder. Kept apart from an ordinary mismatch so the refusal can say why
/// Farhelm cannot tell (a remount on a filesystem without birth times)
/// rather than claim the folder was replaced.
fn device_change_unconfirmed(
    observed: &Observed,
    identity: DirectoryIdentity,
    birth: Option<i64>,
) -> bool {
    let (dev, ino) = identity;
    observed.ino == ino && observed.dev != dev && (birth.is_none() || observed.birth_ns.is_none())
}

/// fsync an openable directory. The durability primitive
/// `files.rs::write_durable_sync` lacks for NEW child entries: it fsyncs a
/// file's parent after renaming the file over a destination, but a freshly
/// mkdir'd child directory's persistence rides on ITS parent's entry, so
/// that parent needs the fsync itself.
fn fsync_dir(path: &Path) -> Result<()> {
    let file = fs::File::open(path)?;
    file.sync_all()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Row plumbing
// ---------------------------------------------------------------------------

/// Load one registry row, or `None` if absent.
pub fn get_working_copy(conn: &Connection, id: &str) -> Result<Option<WorkingCopyRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, canonical_root, canonical_path, repo_owner, repo_name, \
         original_basename, origin_session_id, root_device, root_inode, \
         path_device, path_inode, root_birth_ns, path_birth_ns, allocation_state, archive_destination, \
         preparation_snapshot, created_at \
         FROM working_copies WHERE id = ?1",
    )?;
    let mut rows = stmt.query(rusqlite::params![id])?;
    rows.next()?
        .map(decode_row)
        .transpose()
        .map_err(WorkingCopyError::from)
}

fn decode_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkingCopyRow> {
    let state: String = row.get("allocation_state")?;
    Ok(WorkingCopyRow {
        id: row.get("id")?,
        canonical_root: row.get("canonical_root")?,
        canonical_path: row.get("canonical_path")?,
        repo_owner: row.get("repo_owner")?,
        repo_name: row.get("repo_name")?,
        original_basename: row.get("original_basename")?,
        origin_session_id: row.get("origin_session_id")?,
        root_identity: decode_identity(row, "root_device", "root_inode"),
        path_identity: decode_identity(row, "path_device", "path_inode"),
        root_birth_ns: row.get("root_birth_ns")?,
        path_birth_ns: row.get("path_birth_ns")?,
        allocation_state: AllocationState::from_str(&state).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        archive_destination: row.get("archive_destination")?,
        preparation_snapshot: row.get("preparation_snapshot")?,
        created_at: row.get("created_at")?,
    })
}

fn decode_identity(
    row: &rusqlite::Row<'_>,
    device: &str,
    inode: &str,
) -> Option<DirectoryIdentity> {
    let device: Option<i64> = row.get(device).ok()?;
    let inode: Option<i64> = row.get(inode).ok()?;
    Some((device? as u64, inode? as u64))
}

/// The caller-supplied facts behind a new `planned` row. `id` is minted by
/// the caller (a UUID) so it can carry the same identifier into its
/// session-row bookkeeping inside the same transaction.
#[derive(Clone)]
pub struct PlannedWorkingCopy {
    pub id: String,
    pub canonical_root: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub original_basename: String,
    pub origin_session_id: String,
    /// Captured before mkdir so a pending retry cannot allocate under a
    /// replacement root. Primitive registry fixtures may omit it.
    pub root_identity: Option<DirectoryIdentity>,
    /// Original resolved configuration, shell and publication provenance.
    pub preparation_snapshot: Option<String>,
}

/// Private recovery input, committed before the checkout directory exists.
/// Publication is marked before writing NotStarted: once that boundary is
/// crossed, a missing file is ambiguous and must never be initialized again.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationSnapshot {
    pub resolved: farhelm_proto::ResolvedGithubCheckout,
    pub shell: String,
    pub publication_started: bool,
}

// ---------------------------------------------------------------------------
// Composition primitives: the create path's transaction partners
// ---------------------------------------------------------------------------

/// Insert a `planned` registry row with the admitted root and preparation
/// snapshot, but no checkout identity or membership. Deliberately `&Connection`-taking rather
/// than transaction-owning — see the module docs for why this is the
/// pattern the create-path slice composes into (same-transaction session
/// row + membership, one commit, no FKs to lean on).
///
/// `original_basename` is the FINAL name including any `-N` suffix the
/// preview chose ([`farhelm_proto::github_checkout::checkout_basename`]
/// over [`occupied_related_names`]); allocation does not rename, it creates
/// exactly this name.
pub fn record_planned(conn: &Connection, spec: &PlannedWorkingCopy) -> Result<WorkingCopyRow> {
    // The root's birth time is recorded with the plan, so a create that is
    // interrupted before mkdir can still tell its root from one recreated
    // at the same path (with a reused inode) before it retries. It is taken
    // only when the root still has the identity admission saw moments
    // earlier; otherwise the stored identity already disagrees with the
    // disk, and the pre-mkdir check refuses the retry on `(dev, ino)` alone.
    let root_birth = spec.root_identity.and_then(|identity| {
        observe(Path::new(&spec.canonical_root))
            .ok()
            .filter(|observed| observed.identity() == identity)
            .and_then(|observed| observed.birth_ns)
    });
    conn.execute(
        "INSERT INTO working_copies \
         (id, canonical_root, canonical_path, repo_owner, repo_name, \
          original_basename, origin_session_id, root_device, root_inode, \
          path_device, path_inode, allocation_state, archive_destination, \
          preparation_snapshot, created_at, root_birth_ns) \
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?9, ?10, NULL, NULL, ?7, \
                 NULL, ?11, ?8, ?12)",
        rusqlite::params![
            spec.id,
            spec.canonical_root,
            spec.repo_owner,
            spec.repo_name,
            spec.original_basename,
            spec.origin_session_id,
            AllocationState::Planned.as_str(),
            now_unix(),
            spec.root_identity.map(|identity| identity.0 as i64),
            spec.root_identity.map(|identity| identity.1 as i64),
            spec.preparation_snapshot,
            root_birth,
        ],
    )?;
    get_working_copy(conn, &spec.id)?.ok_or_else(|| WorkingCopyError::RowMissing(spec.id.clone()))
}

/// Attach `session_id` to `working_copy_id`'s lifetime. Explicit because
/// the schema has no foreign keys by repo convention: callers keep
/// membership and session-row writes atomic by passing the same `&tx` to
/// both. Attaching an existing member is a no-op.
pub fn add_member(conn: &Connection, session_id: &str, working_copy_id: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO working_copy_members (session_id, working_copy_id) \
         VALUES (?1, ?2) ON CONFLICT(session_id, working_copy_id) DO NOTHING",
        rusqlite::params![session_id, working_copy_id],
    )?;
    Ok(())
}

/// Detach `session_id` from `working_copy_id`. Removing a non-member is a
/// no-op so callers need not pre-check.
pub fn remove_member(conn: &Connection, session_id: &str, working_copy_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM working_copy_members \
         WHERE session_id = ?1 AND working_copy_id = ?2",
        rusqlite::params![session_id, working_copy_id],
    )?;
    Ok(())
}

/// Delete a `planned` registry row outright: the create path's rollback
/// for a mkdir that never won (a collision). Deliberately restricted to
/// `planned` rows — a row that captured a directory identity is ownership
/// evidence and is never removed by a refusal; only the archive/teardown
/// machinery may retire an `allocated` row. Fails with
/// [`WorkingCopyError::WrongState`] when the row is missing or no longer
/// planned, so a racing transition (the row was allocated under us) can
/// never silently drop evidence.
pub fn delete_planned(conn: &Connection, working_copy_id: &str) -> Result<()> {
    let deleted = conn.execute(
        "DELETE FROM working_copies WHERE id = ?1 AND allocation_state = ?2",
        rusqlite::params![working_copy_id, AllocationState::Planned.as_str()],
    )?;
    if deleted == 0 {
        return Err(WorkingCopyError::WrongState(working_copy_id.to_string()));
    }
    Ok(())
}

/// Find the checkout this session originally requested, independently of
/// its current lifetime memberships. Borrowing a directory never grants
/// permission to run that directory's clone or preparation again. More
/// than one origin record is corruption and refuses recovery.
pub fn origin_working_copy(conn: &Connection, session_id: &str) -> Result<Option<WorkingCopyRow>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, canonical_root, canonical_path, repo_owner, repo_name, \
         original_basename, origin_session_id, root_device, root_inode, \
         path_device, path_inode, root_birth_ns, path_birth_ns, allocation_state, archive_destination, \
         preparation_snapshot, created_at \
         FROM working_copies WHERE origin_session_id = ?1 LIMIT 2",
    )?;
    let mut rows = stmt.query(rusqlite::params![session_id])?;
    let origin = rows.next()?.map(decode_row).transpose()?;
    if rows.next()?.is_some() {
        return Err(WorkingCopyError::AmbiguousOrigin(session_id.to_string()));
    }
    Ok(origin)
}

/// Every registry row `session_id` keeps alive, in stable creation order.
/// Membership says nothing about which session originated a checkout;
/// callers recovering preparation must use [`origin_working_copy`].
pub fn member_working_copies(conn: &Connection, session_id: &str) -> Result<Vec<WorkingCopyRow>> {
    let mut stmt = conn.prepare_cached(
        "SELECT w.id, w.canonical_root, w.canonical_path, w.repo_owner, w.repo_name, \
         w.original_basename, w.origin_session_id, w.root_device, w.root_inode, \
         w.path_device, w.path_inode, w.root_birth_ns, w.path_birth_ns, w.allocation_state, w.archive_destination, \
         w.preparation_snapshot, w.created_at \
         FROM working_copies w \
         JOIN working_copy_members m ON m.working_copy_id = w.id \
         WHERE m.session_id = ?1 ORDER BY w.created_at, w.id",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![session_id], decode_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Count a working copy's members by ACTUAL ROWS. Deliberately no counter
/// column: a counter can drift from its rows, and the registry is
/// ownership evidence, not an approximation.
pub fn member_count(conn: &Connection, working_copy_id: &str) -> Result<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM working_copy_members WHERE working_copy_id = ?1",
        rusqlite::params![working_copy_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

/// Every registry row, in insertion order. Startup reconciliation uses it
/// to find `archive_pending` rows; delete-time archival uses it for the
/// corrupt-evidence overlap check. No filtering here: callers see retired
/// rows too and skip what they do not need.
pub fn all_working_copies(conn: &Connection) -> Result<Vec<WorkingCopyRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, canonical_root, canonical_path, repo_owner, repo_name, \
         original_basename, origin_session_id, root_device, root_inode, \
         path_device, path_inode, root_birth_ns, path_birth_ns, allocation_state, archive_destination, \
         preparation_snapshot, created_at \
         FROM working_copies ORDER BY created_at, id",
    )?;
    let rows = stmt
        .query_map([], decode_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Compare recorded paths by components for the corrupt-registry guard.
/// Planned rows have no accepted path, so they cannot establish overlap.
pub(crate) fn path_overlaps(a: &Option<String>, b: &str) -> bool {
    let Some(a) = a else { return false };
    let (a, b) = (Path::new(a), Path::new(b));
    a.starts_with(b) || b.starts_with(a)
}

/// Admission normally excludes overlapping checkouts. Recovery cannot assume
/// that invariant survived damaged registry evidence: inspect all active rows
/// before creating archive directories, journaling or moving any content.
///
/// Also refuses a row that is, contains, or sits inside its own root's
/// archive directory. Admission reserves that name, but a registry recorded
/// before the reservation can hold such a checkout, and moving a directory
/// into its own subdirectory can never succeed (the kernel answers
/// `EINVAL`), so it is named up front rather than retried forever. No
/// earlier attempt can have completed for such a row, which is why this
/// half of the check is safe to apply before recovery looks at anything.
fn refuse_overlapping_archive(conn: &Connection, row: &WorkingCopyRow) -> Result<()> {
    let archive_dir = Path::new(&row.canonical_root).join(ARCHIVE_DIR_NAME);
    if path_overlaps(&row.canonical_path, &archive_dir.to_string_lossy()) {
        return Err(WorkingCopyError::SourceIsArchiveRoot {
            path: PathBuf::from(row.canonical_path.as_deref().unwrap_or(&row.canonical_root)),
        });
    }
    for other in all_working_copies(conn)? {
        if other.id != row.id
            && other.allocation_state != AllocationState::Retired
            && other
                .canonical_path
                .as_deref()
                .is_some_and(|path| path_overlaps(&row.canonical_path, path))
        {
            return Err(WorkingCopyError::OverlappingRecord {
                path: PathBuf::from(row.canonical_path.as_deref().unwrap_or(&row.canonical_root)),
                other_id: other.id,
            });
        }
    }
    Ok(())
}

/// Refuse to MOVE a checkout into its root's archive directory while another
/// active managed checkout is, contains, or sits inside that directory:
/// the move would land this checkout inside an unrelated repository.
///
/// A registry recorded before the archive name was reserved can hold such a
/// checkout. Kept separate from [`refuse_overlapping_archive`] because it
/// guards the rename, not the row: a fresh archive checks it before writing
/// its journal, while recovery checks it only on paths that would still
/// rename. A pending archive whose rename already completed (its journaled
/// destination matches) must still finish, since nothing moves any more.
fn refuse_archive_destination_checkout(conn: &Connection, row: &WorkingCopyRow) -> Result<()> {
    let archive_dir = Path::new(&row.canonical_root).join(ARCHIVE_DIR_NAME);
    let archive_dir_str = archive_dir.to_string_lossy();
    for other in all_working_copies(conn)? {
        if other.id != row.id
            && other.allocation_state != AllocationState::Retired
            && path_overlaps(&other.canonical_path, &archive_dir_str)
        {
            return Err(WorkingCopyError::ArchiveRootIsCheckout {
                path: archive_dir,
                other_id: other.id,
            });
        }
    }
    Ok(())
}

/// Settle an `allocated` row whose recorded source is PROVED absent:
/// verify its recorded root and re-check absence, then delete the row. This
/// is Design E's "missing source is a visible cleanup diagnostic and permits
/// metadata deletion without moving anything" — the only path that may
/// remove an `allocated` row without a rename, and it stays gated on the
/// row's own identity evidence (a source that reappeared, or was never
/// ours, fails closed with [`WorkingCopyError::WrongState`] /
/// [`WorkingCopyError::IdentityMismatch`] rather than dropping ownership
/// evidence). Callers run this inside their final SQLite transaction so
/// the deletion commits atomically with the session row's removal.
pub fn retire_missing(conn: &Connection, working_copy_id: &str) -> Result<()> {
    let row = get_working_copy(conn, working_copy_id)?
        .ok_or_else(|| WorkingCopyError::RowMissing(working_copy_id.to_string()))?;
    if row.allocation_state != AllocationState::Allocated {
        return Err(WorkingCopyError::WrongState(row.id.clone()));
    }
    let Some(path) = &row.canonical_path else {
        return Err(WorkingCopyError::WrongState(row.id.clone()));
    };
    // Absence under a replacement root says nothing about the checkout
    // captured under the original root, which may still exist elsewhere.
    verified_root(&row)?;
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Ok(_) => {
            return Err(WorkingCopyError::IdentityMismatch {
                path: PathBuf::from(path),
            });
        }
        Err(e) => return Err(e.into()),
    }
    let deleted = conn.execute(
        "DELETE FROM working_copies WHERE id = ?1 AND allocation_state = ?2",
        rusqlite::params![working_copy_id, AllocationState::Allocated.as_str()],
    )?;
    if deleted == 0 {
        return Err(WorkingCopyError::WrongState(working_copy_id.to_string()));
    }
    Ok(())
}

/// Attach `session_id` to every ACTIVE (`allocated`) managed checkout that
/// is the accepted canonical cwd itself or an ancestor of it — Design C's
/// `bind_existing_memberships` idea. This is what keeps a same-directory
/// replacement (helm Replace = create-then-delete) from opening a
/// zero-reference window: the replacement registers as a member BEFORE the
/// source session's delete can decide this checkout is unreferenced.
///
/// Returns the number of memberships added. Runs against the caller's
/// connection so the create path composes it into its own admission-held
/// sequence; the directory-admission mutex (held by every caller) is what
/// makes the ancestor read and these inserts one decision rather than a
/// race with a concurrent last-reference archive.
pub fn attach_existing_ancestors(
    conn: &Connection,
    session_id: &str,
    canonical_cwd: &str,
) -> Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT id, canonical_path FROM working_copies \
         WHERE allocation_state = ?1 AND canonical_path IS NOT NULL",
    )?;
    let candidates: Vec<String> = stmt
        .query_map(
            rusqlite::params![AllocationState::Allocated.as_str()],
            |row| row.get(0),
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let cwd = PathBuf::from(canonical_cwd);
    let mut added = 0;
    for id in candidates {
        let Some(row) = get_working_copy(conn, &id)? else {
            continue;
        };
        let Some(path) = &row.canonical_path else {
            continue;
        };
        let managed = PathBuf::from(path);
        let is_self_or_ancestor =
            managed == cwd || (cwd.starts_with(&managed) && managed != Path::new("/"));
        if is_self_or_ancestor {
            let before = member_count(conn, &id)?;
            add_member(conn, session_id, &id)?;
            let after = member_count(conn, &id)?;
            added += usize::try_from(after - before).unwrap_or(0);
        }
    }
    Ok(added)
}

// ---------------------------------------------------------------------------
// Naming: occupancy scan
// ---------------------------------------------------------------------------

/// The result of [`occupied_related_names`]: every directory entry under
/// `root` related to `basename` (the plain name and any `basename-...`), plus
/// whether the scan stayed under its cap. A `complete == false` scan means
/// the caller must refuse rather than claim any name is free.
pub struct OccupiedScan {
    pub names: std::collections::HashSet<String>,
    pub complete: bool,
}

/// Scan `root` once and collect every entry related to `basename` — the
/// plain name and every name beginning with `basename-`, including titled
/// names with arbitrary suffixes. Every entry occupies its exact name —
/// files, directories, and symlinks alike, including DANGLING ones (a name
/// claimed on disk is claimed as far as mkdir is concerned).
///
/// [`ARCHIVE_DIR_NAME`] is reported as occupied whenever it is related to
/// `basename`, whether or not it exists yet: it is reserved for archived
/// checkouts, and ordinary titles produce it exactly (repo `farhelm`,
/// title "archived working copies"). A checkout that took the name became
/// the archive directory for every later delete in that root, and its own
/// delete tried to move it into itself. Reserving it here, rather than
/// relying on the nesting guard, holds even in a root whose registry has
/// no active rows to derive the archive path from. It returns
/// a SET because the naming helper's `occupied_names` callback checks
/// arbitrary candidates for titled names. The preview uses it to propose a
/// name and the create path re-runs it to confirm the proposal still holds;
/// [`allocate`] remains the actual arbitration because only the exclusive
/// mkdir can race.
///
/// `cap` bounds iterator calls, including the call needed to prove EOF.
/// Exhausting it without EOF yields [`WorkingCopyError::IncompleteScan`]
/// so a preview never proposes a name from an incomplete view. Root-open
/// and entry-read failures propagate rather than implying a free name.
///
/// `basename` must be lowercase, as every candidate is; entry names are
/// lowercased before comparison and reported lowercased.
pub fn occupied_related_names(root: &Path, basename: &str, cap: usize) -> Result<OccupiedScan> {
    let entries = fs::read_dir(root)?;
    occupied_related_names_from_entries(entries, basename, cap)
}

/// Scan an already-open directory iterator with a hard bound on `next` calls.
///
/// Keeping the iterator as the seam lets tests inject an entry read failure
/// and count calls without changing how production opens the canonical root.
/// The helper deliberately consumes no call when `cap == 0`, and it never
/// performs an extra call after the cap is reached: a directory that fills the
/// budget cannot establish that no further entry exists.
fn occupied_related_names_from_entries<I>(
    mut entries: I,
    basename: &str,
    cap: usize,
) -> Result<OccupiedScan>
where
    I: Iterator<Item = std::io::Result<fs::DirEntry>>,
{
    let mut names = std::collections::HashSet::new();
    let basename_prefix = format!("{basename}-");
    if ARCHIVE_DIR_NAME == basename || ARCHIVE_DIR_NAME.starts_with(&basename_prefix) {
        names.insert(ARCHIVE_DIR_NAME.to_owned());
    }
    for _ in 0..cap {
        let entry = match entries.next() {
            Some(Ok(entry)) => entry,
            Some(Err(error)) => return Err(error.into()),
            None => {
                return Ok(OccupiedScan {
                    names,
                    complete: true,
                });
            }
        };
        // Compared case-insensitively: candidates are always lowercase, and
        // on a case-insensitive filesystem (macOS's default) an entry `Bar-1`
        // makes `mkdir bar-1` fail. Read case-sensitively, the scan called
        // that name free, every preview proposed it again, and every create
        // failed. On a case-sensitive filesystem this only passes over a few
        // names that were free.
        let name = entry.file_name();
        if let Some(name) = name.to_str().map(str::to_lowercase)
            && (name == basename || name.starts_with(&basename_prefix))
        {
            names.insert(name);
        }
    }
    Err(WorkingCopyError::IncompleteScan { scanned: cap })
}

// ---------------------------------------------------------------------------
// Allocation
// ---------------------------------------------------------------------------

/// Allocate the directory for a `planned` row: exclusive mkdir, then
/// identity capture, then membership — the row leaves `planned` in ONE
/// transaction only after the mkdir has actually won.
///
/// The mkdir is `mkdir(2)` — `create_new(true)` semantics — so under any
/// race exactly one caller wins and every loser receives
/// [`WorkingCopyError::Conflict`] having created nothing anywhere else.
/// The winner stats the new directory for its `(device, inode)`,
/// canonicalizes its path, captures the root's identity alongside (a later
/// root swap is exactly what the archive primitive must refuse to operate
/// under), and commits row update + member insert in a single SQLite
/// transaction.
///
/// `expected_root_identity`, when given (the create path stats the root
/// before planning), is verified against a fresh stat of `canonical_root`
/// BEFORE the mkdir: a root swapped underneath the registry fails closed
/// instead of allocating inside a stranger's tree.
///
/// Returns a phase-aware failure when allocation cannot finish. Only
/// [`AllocationFailure::PreMkdir`] proves that rollback may delete the
/// planned evidence; post-mkdir and uncertain failures retain the row's
/// last committed state until an explicit recovery decision can inspect it.
pub fn allocate(
    conn: &Connection,
    working_copy_id: &str,
    expected_root_identity: Option<DirectoryIdentity>,
) -> std::result::Result<AcceptedDirectory, AllocationFailure> {
    allocate_with_fault(conn, working_copy_id, expected_root_identity, None)
}

/// Allocate a planned checkout while optionally failing a real post-mkdir
/// effect. The ordinary allocator above keeps the production and existing
/// primitive-test call shape free of test-only configuration.
pub fn allocate_with_fault(
    conn: &Connection,
    working_copy_id: &str,
    expected_root_identity: Option<DirectoryIdentity>,
    fault: Option<&AllocationFault>,
) -> std::result::Result<AcceptedDirectory, AllocationFailure> {
    let row = get_working_copy(conn, working_copy_id)
        .map_err(AllocationFailure::PreMkdir)?
        .ok_or_else(|| {
            AllocationFailure::PreMkdir(WorkingCopyError::RowMissing(working_copy_id.to_string()))
        })?;
    if row.allocation_state != AllocationState::Planned {
        return Err(AllocationFailure::PreMkdir(WorkingCopyError::WrongState(
            row.id.clone(),
        )));
    }
    let root = PathBuf::from(&row.canonical_root);
    let target = root.join(&row.original_basename);

    if let Some(expected) = expected_root_identity {
        // A missing root is ALSO an identity failure (it cannot match an
        // expected identity), and so is any other stat failure — fail
        // closed rather than allocate inside a tree we cannot vouch for.
        let actual = observe(&root)
            .map_err(WorkingCopyError::Io)
            .map_err(AllocationFailure::PreMkdir)?;
        if !same_directory(&actual, expected, row.root_birth_ns) {
            return Err(AllocationFailure::PreMkdir(
                WorkingCopyError::IdentityMismatch { path: root },
            ));
        }
    }

    // Exclusive creation: mkdir(2) fails with EEXIST for files, real
    // directories, and symlinks (including dangling ones) alike. This one
    // syscall is the whole race policy — there is no check-then-create
    // window anywhere in this function.
    fs::create_dir(&target).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            AllocationFailure::PreMkdir(WorkingCopyError::Conflict {
                path: target.clone(),
            })
        } else {
            // A failed mkdir can report an interrupted or transport-like
            // error after the kernel created the entry. Without a positive
            // success result it is still not proof of absence, so retain.
            AllocationFailure::Uncertain(anyhow::Error::new(WorkingCopyError::Io(e)))
        }
    })?;

    // Before identity capture commits, post-mkdir failures retain Planned:
    // it accurately says Farhelm has no captured identity while the Error
    // session makes the possible directory visible. A later read failure
    // after the allocation transaction may instead retain Allocated.
    let target_observed = observe(&target)
        .map_err(WorkingCopyError::Io)
        .map_err(AllocationFailure::PostMkdir)?;
    let identity = target_observed.identity();
    let path_birth = target_observed.birth_ns;
    // Canonicalize AFTER exclusive creation: the only path canonicalized
    // is a directory this call just created, so no concurrent rename of
    // parents can smuggle in a different object without the identity
    // capture below exposing it.
    let canonical_path = target
        .canonicalize()
        .map_err(WorkingCopyError::Io)
        .map_err(AllocationFailure::PostMkdir)?;
    // The root and basename are valid UTF-8 text, but `canonicalize`
    // follows symlinks in the root's ancestors, so the real path can still
    // fail to be. Recording a lossy spelling would name a directory that
    // does not exist, and the session would be launched there.
    let canonical_text = canonical_path.to_str().map(str::to_string).ok_or_else(|| {
        AllocationFailure::PostMkdir(WorkingCopyError::Other(anyhow::anyhow!(
            "checkout directory {} resolves to {}, which is not valid UTF-8; Farhelm does \
                 not support such paths",
            target.display(),
            canonical_path.display()
        )))
    })?;
    let root_observed = observe(&root)
        .map_err(WorkingCopyError::Io)
        .map_err(AllocationFailure::PostMkdir)?;
    let root_identity = root_observed.identity();
    let root_birth = root_observed.birth_ns;

    // Make the new directory's entry durable before the database claims
    // it: without this parent fsync a crash could leave the row
    // `allocated` over a directory that never reached disk.
    // Inject the effect result before classifying it, so a regression in
    // production's phase mapping also changes the fault-injection test.
    let sync_result = match fault {
        Some(fault) => fault(AllocationStage::BeforeParentFsync)
            .map_err(WorkingCopyError::Other)
            .and_then(|()| fsync_dir(&root)),
        None => fsync_dir(&root),
    };
    sync_result.map_err(AllocationFailure::PostMkdir)?;

    let tx = conn
        .unchecked_transaction()
        .context("beginning working-copy allocation transaction")
        .map_err(|error| AllocationFailure::PostMkdir(WorkingCopyError::Other(error)))?;
    let updated = tx
        .execute(
            "UPDATE working_copies \
         SET allocation_state = ?2, root_device = ?3, root_inode = ?4, \
             path_device = ?5, path_inode = ?6, canonical_path = ?7, \
             root_birth_ns = ?9, path_birth_ns = ?10 \
         WHERE id = ?1 AND allocation_state = ?8",
            rusqlite::params![
                row.id,
                AllocationState::Allocated.as_str(),
                root_identity.0 as i64,
                root_identity.1 as i64,
                identity.0 as i64,
                identity.1 as i64,
                canonical_text,
                AllocationState::Planned.as_str(),
                root_birth,
                path_birth,
            ],
        )
        .map_err(|error| AllocationFailure::PostMkdir(WorkingCopyError::Db(error)))?;
    if updated == 0 {
        // The row's state moved between our read and this write — it is
        // no longer ours to allocate. (The mkdir we lost will be
        // reconciled by whoever owns the row now; we must not attach
        // membership or touch anything else.)
        return Err(AllocationFailure::PostMkdir(WorkingCopyError::WrongState(
            row.id.clone(),
        )));
    }
    add_member(&tx, &row.origin_session_id, &row.id).map_err(AllocationFailure::PostMkdir)?;
    attach_retained_sessions(&tx, &row.id, &canonical_path)
        .map_err(AllocationFailure::PostMkdir)?;
    tx.commit()
        .context("committing working-copy allocation")
        .map_err(|error| AllocationFailure::PostMkdir(WorkingCopyError::Other(error)))?;
    let fresh = get_working_copy(conn, working_copy_id)
        .map_err(AllocationFailure::PostMkdir)?
        .ok_or_else(|| {
            AllocationFailure::PostMkdir(WorkingCopyError::RowMissing(row.id.clone()))
        })?;
    Ok(AcceptedDirectory {
        row: fresh,
        identity,
        canonical_path,
        canonical_text,
    })
}

/// Reusing a deleted directory's name must preserve sessions that still
/// refer to that name. Their stopped or errored status does not
/// shorten the checkout's lifetime. This runs in the identity-publication
/// transaction so no Allocated row can omit those retained references.
fn attach_retained_sessions(conn: &Connection, working_copy_id: &str, path: &Path) -> Result<()> {
    let mut stmt = conn.prepare("SELECT id, canonical_cwd, cwd FROM sessions")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (session_id, canonical, cwd) = row?;
        // A legacy path may not exist anymore. Resolving it through the
        // filesystem would discard exactly the reference being recovered.
        let recorded = canonical.as_deref().unwrap_or(&cwd);
        if normalized_absolute_path(Path::new(recorded)).is_some_and(|cwd| cwd.starts_with(path)) {
            add_member(conn, &session_id, working_copy_id)?;
        }
    }
    Ok(())
}

/// Compare legacy absolute paths by components without following today's
/// symlinks. Relative spellings carry no stable host-local identity and
/// cannot establish an ancestor relationship after a process restart.
fn normalized_absolute_path(path: &Path) -> Option<PathBuf> {
    use std::path::Component;
    if !path.is_absolute() {
        return None;
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
        }
    }
    Some(normalized)
}

/// Compare the object at the row's recorded canonical path against the
/// row's captured identity. Pure observation: never mutates the
/// filesystem or the row. See [`IdentityStatus`] for the four answers.
pub fn verify_identity(row: &WorkingCopyRow) -> Result<IdentityStatus> {
    Ok(verify_identity_observed(row)?.0)
}

/// [`verify_identity`], plus the `(dev, ino)` it just observed when the
/// folder matched.
///
/// For a caller that hands the folder's identity on to a later, plain
/// comparison: a retried fresh create gives it to the in-terminal launcher,
/// which compares `(dev, ino)` exactly. Handing it the identity recorded at
/// allocation reintroduced the device-number check [`same_directory`]
/// exists to avoid, so a retry after a remount that renumbered the device
/// was refused as "replaced" for good. The reading returned here was taken
/// by the same `stat` that [`same_directory`] accepted, so the launcher
/// compares two readings of one folder taken seconds apart within one
/// create. `None` for every status but [`IdentityStatus::Matches`].
pub fn verify_identity_observed(
    row: &WorkingCopyRow,
) -> Result<(IdentityStatus, Option<DirectoryIdentity>)> {
    let Some(identity) = row.path_identity else {
        return Ok((IdentityStatus::NoCapturedIdentity, None));
    };
    let Some(path) = &row.canonical_path else {
        return Ok((IdentityStatus::NoCapturedIdentity, None));
    };
    match observe(Path::new(path)) {
        Ok(observed) if same_directory(&observed, identity, row.path_birth_ns) => {
            Ok((IdentityStatus::Matches, Some((observed.dev, observed.ino))))
        }
        Ok(observed) if device_change_unconfirmed(&observed, identity, row.path_birth_ns) => {
            Ok((IdentityStatus::DeviceChangedUnconfirmed, None))
        }
        Ok(_) => Ok((IdentityStatus::DifferentObject, None)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((IdentityStatus::Missing, None)),
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------
// Archive move and its crash recovery
// ---------------------------------------------------------------------------

/// Replace an archive parent's actual durability operation in tests.
/// The opened directory and its path identify the barrier being exercised;
/// callbacks that permit progress must sync the file themselves. Production
/// installs no callback and calls `File::sync_all` at both parent barriers.
pub type ArchiveParentSync =
    std::sync::Arc<dyn Fn(&Path, &fs::File) -> std::io::Result<()> + Send + Sync>;

/// Sync the parent whose directory entries changed during an archive move.
/// Recovery repeats these same barriers before allowing metadata retirement:
/// finding the renamed inode alone cannot prove that its name is durable.
fn sync_archive_parent(path: &Path, sync: Option<&ArchiveParentSync>) -> Result<()> {
    let file = fs::File::open(path)?;
    match sync {
        Some(sync) => sync(path, &file)?,
        None => file.sync_all()?,
    }
    Ok(())
}

/// Move an `allocated` working copy into
/// `<canonical_root>/farhelm-archived-working-copies/` WITHOUT replacement,
/// journaling the destination first.
///
/// Sequence, and the invariant each step owes:
///
/// 1. Source identity is verified first. A missing source is
///    [`ArchiveOutcome::SourceMissing`] (a visible outcome: the cleanup
///    diagnostic may proceed and the metadata may be deleted); a DIFFERENT
///    object at the source fails closed — this primitive never moves a
///    stranger.
/// 2. The recorded root identity must still match a real directory before
///    any archive child is created. The archive child must itself be a real
///    directory on the source's device; symlinks and non-directories are
///    refused, and a foreign device cannot receive the atomic rename. When
///    the directory must be created, the creation is durable: the PARENT
///    of the new directory (`canonical_root`) is fsynced after mkdir — the
///    new-child-entry gap `files.rs::write_durable_sync` does not cover.
/// 3. The destination name
///    `<original_basename>-<UTC YYYYMMDDTHHMMSSZ>` is journaled on the row
///    (state `archive_pending`) BEFORE the rename. The journal is what
///    lets [`reconcile_archive`] distinguish a crashed commit from a lost
///    one.
/// 4. The rename is without replacement: `renameat2(RENAME_NOREPLACE)` on
///    Linux via `syscall(SYS_renameat2, ...)` (the pinned libc 0.2.189
///    declares the named wrapper for gnu targets only, while
///    `SYS_renameat2` exists for both gnu and musl, and this workspace
///    ships musl-static release binaries) and `renameatx_np(RENAME_EXCL)`
///    on macOS. On collision one fresh UUID is appended to the original
///    stem, the journal is updated, and the rename retries; after [`ARCHIVE_ATTEMPTS`]
///    attempts the error fails closed with the row left `archive_pending`
///    for reconciliation. There is NO recursive-copy fallback and NO
///    recursive removal anywhere in this module.
/// 5. Both parent directories are fsynced after the move.
pub fn archive_move(conn: &Connection, working_copy_id: &str) -> Result<ArchiveOutcome> {
    archive_move_with_parent_sync(conn, working_copy_id, None)
}

/// Execute [`archive_move`] with an optional replacement for its two
/// post-rename barriers, so teardown tests can fail the actual durability
/// operation while retaining the real rename and SQLite journal.
pub(crate) fn archive_move_with_parent_sync(
    conn: &Connection,
    working_copy_id: &str,
    parent_sync: Option<&ArchiveParentSync>,
) -> Result<ArchiveOutcome> {
    archive_move_with_effects(conn, working_copy_id, parent_sync, &mut rename_noreplace)
}

/// Keep collision injection at the actual rename boundary, after the real
/// destination has been journaled. Tests can occupy that exact name without
/// predicting wall-clock timestamps or changing the production naming policy.
fn archive_move_with_effects(
    conn: &Connection,
    working_copy_id: &str,
    parent_sync: Option<&ArchiveParentSync>,
    rename: &mut dyn FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<ArchiveOutcome> {
    let row = get_working_copy(conn, working_copy_id)?
        .ok_or_else(|| WorkingCopyError::RowMissing(working_copy_id.to_string()))?;
    if row.allocation_state != AllocationState::Allocated {
        return Err(WorkingCopyError::WrongState(row.id.clone()));
    }
    let Some(expected) = row.path_identity else {
        return Err(WorkingCopyError::WrongState(row.id.clone()));
    };
    let Some(path) = &row.canonical_path else {
        return Err(WorkingCopyError::WrongState(row.id.clone()));
    };
    let source = PathBuf::from(path);
    refuse_overlapping_archive(conn, &row)?;
    refuse_archive_destination_checkout(conn, &row)?;
    let root = verified_root(&row)?;
    let source_observed = match observe(&source) {
        Ok(observed) => observed,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ArchiveOutcome::SourceMissing);
        }
        Err(e) => return Err(e.into()),
    };
    if !same_directory(&source_observed, expected, row.path_birth_ns) {
        return Err(WorkingCopyError::IdentityMismatch { path: source });
    }
    let source_identity = source_observed.identity();
    let archive_root = ensure_archive_root(&root)?;
    if identity_of(&archive_root)?.0 != source_identity.0 {
        return Err(WorkingCopyError::ArchiveRootForeignDevice { path: archive_root });
    }
    let stem = format!("{}-{}", row.original_basename, utc_compact(now_unix()));
    persist_journal(conn, &row.id, &stem, AllocationState::Allocated)?;
    let final_destination =
        rename_exclusive_into(conn, &row.id, &archive_root, &source, &stem, &stem, rename)?;
    sync_archive_parent(&archive_root, parent_sync)?;
    sync_archive_parent(&root, parent_sync)?;
    Ok(ArchiveOutcome::Archived {
        destination: final_destination,
    })
}

/// Verify the recorded root before recovery, restart or archive mutation.
///
/// A matching checkout inode does not establish ownership of its parent: a
/// user can move that checkout into a replacement root at the same path.
/// Missing identity and symlink roots therefore refuse even when the source
/// itself still matches. Planned retries use the admission-time identity;
/// archive paths use it before `ensure_archive_root` can mutate anything.
pub(crate) fn verified_root(row: &WorkingCopyRow) -> Result<PathBuf> {
    let expected = row
        .root_identity
        .ok_or_else(|| WorkingCopyError::WrongState(row.id.clone()))?;
    let root = PathBuf::from(&row.canonical_root);
    let observed = observe(&root)?;
    if observed.is_dir && device_change_unconfirmed(&observed, expected, row.root_birth_ns) {
        return Err(WorkingCopyError::DeviceChangedUnconfirmed { path: root });
    }
    if !observed.is_dir || !same_directory(&observed, expected, row.root_birth_ns) {
        return Err(WorkingCopyError::IdentityMismatch { path: root });
    }
    Ok(root)
}

/// Create (durably) or validate the canonical root's archive directory and
/// return its path. Symlinks and non-directories are refused outright.
fn ensure_archive_root(root: &Path) -> Result<PathBuf> {
    let archive_root = root.join(ARCHIVE_DIR_NAME);
    match fs::symlink_metadata(&archive_root) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(WorkingCopyError::ArchiveRootSymlink { path: archive_root });
            }
            if !meta.is_dir() {
                return Err(WorkingCopyError::ArchiveRootNotDirectory { path: archive_root });
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&archive_root)?;
            // Durability gap handler: fsync the NEW directory's PARENT so
            // the child entry itself survives a crash. A plain mkdir
            // fsyncs nothing, and `write_durable_sync`'s parent fsync only
            // covers a rename's existing parent, never this creation.
            fsync_dir(root)?;
        }
        Err(e) => return Err(e.into()),
    }
    Ok(archive_root)
}

/// Persist the rename journal: destination + `archive_pending`, refusing
/// any state other than `expected` so a replay cannot double-journal a row
/// that already moved on.
fn persist_journal(
    conn: &Connection,
    id: &str,
    destination: &str,
    expected: AllocationState,
) -> Result<()> {
    let updated = conn.execute(
        "UPDATE working_copies \
         SET archive_destination = ?2, allocation_state = ?3 \
         WHERE id = ?1 AND allocation_state = ?4",
        rusqlite::params![
            id,
            destination,
            AllocationState::ArchivePending.as_str(),
            expected.as_str(),
        ],
    )?;
    if updated == 0 {
        return Err(WorkingCopyError::WrongState(id.to_string()));
    }
    Ok(())
}

/// Return a pending row to `allocated` with no journaled destination, for a
/// rename that provably did not move anything.
fn unjournal(conn: &Connection, id: &str) -> Result<()> {
    let updated = conn.execute(
        "UPDATE working_copies SET archive_destination = NULL, allocation_state = ?2 \
         WHERE id = ?1 AND allocation_state = ?3",
        rusqlite::params![
            id,
            AllocationState::Allocated.as_str(),
            AllocationState::ArchivePending.as_str(),
        ],
    )?;
    if updated == 0 {
        return Err(WorkingCopyError::WrongState(id.to_string()));
    }
    Ok(())
}

/// Whether a failed no-overwrite rename is a refusal to move at all, as
/// opposed to a failure whose outcome is unknown.
///
/// Each of these errors reports that the rename was not performed: the
/// filesystem does not support the no-overwrite flag (`EINVAL`, `ENOSYS`,
/// `ENOTSUP`/`EOPNOTSUPP`; network filesystems such as NFS and CIFS, and
/// some FUSE mounts, are the usual cause), the destination is on another
/// filesystem (`EXDEV`), permission is missing (`EACCES`, `EPERM`), or the
/// filesystem is read-only (`EROFS`), or the destination path is too long
/// to resolve (`ENAMETOOLONG`, for a checkout admitted before admission
/// reserved room for the archive suffix). Anything else, an I/O error or a
/// timeout above all, is kept as "may have moved", because on a network
/// filesystem the server can complete a rename whose reply is lost.
fn rename_refused_without_moving(error: &std::io::Error) -> bool {
    // A list rather than a pattern: `ENOTSUP` and `EOPNOTSUPP` are the same
    // number on Linux and different ones on macOS.
    const REFUSALS: [i32; 9] = [
        libc::ENAMETOOLONG,
        libc::EINVAL,
        libc::ENOSYS,
        libc::ENOTSUP,
        libc::EOPNOTSUPP,
        libc::EXDEV,
        libc::EACCES,
        libc::EPERM,
        libc::EROFS,
    ];
    error
        .raw_os_error()
        .is_some_and(|code| REFUSALS.contains(&code))
}

/// Update ONLY the journaled destination on an already-pending row (the
/// collision-retry and unrelated-destination paths).
fn repoint_journal(conn: &Connection, id: &str, destination: &str) -> Result<()> {
    let updated = conn.execute(
        "UPDATE working_copies SET archive_destination = ?2 \
         WHERE id = ?1 AND allocation_state = ?3",
        rusqlite::params![id, destination, AllocationState::ArchivePending.as_str(),],
    )?;
    if updated == 0 {
        return Err(WorkingCopyError::WrongState(id.to_string()));
    }
    Ok(())
}

/// The no-replace rename with bounded collision retry. `destination` is a
/// full file NAME (not a path) inside `archive_root`; on collision a fresh
/// UUID suffix on `collision_stem` is minted and journaled until
/// [`ARCHIVE_ATTEMPTS`] attempts are exhausted and the error fails closed
/// (the row stays `archive_pending` with the last destination for
/// reconciliation). Tests replace the rename operation itself to introduce
/// a competing directory after recovery's observation but before the syscall.
/// The stem never includes a prior collision suffix: the 200-byte checkout
/// name leaves room for a timestamp and ONE UUID, not an accumulating chain.
fn rename_exclusive_into(
    conn: &Connection,
    working_copy_id: &str,
    archive_root: &Path,
    source: &Path,
    destination: &str,
    collision_stem: &str,
    rename: &mut dyn FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<String> {
    let mut destination = destination.to_string();
    for _ in 0..ARCHIVE_ATTEMPTS {
        match rename(source, &archive_root.join(&destination)) {
            Ok(()) => return Ok(destination),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                destination = format!("{}-{}", collision_stem, uuid::Uuid::new_v4().simple());
                repoint_journal(conn, working_copy_id, &destination)?;
            }
            // Nothing moved, so the journal describes a move that cannot
            // have happened. Left pending, the row would make every later
            // recovery retry the same refused rename, and keep refusing
            // Restart and new sessions in the folder meanwhile.
            Err(e) if rename_refused_without_moving(&e) => {
                unjournal(conn, working_copy_id)?;
                return Err(WorkingCopyError::RenameRefused {
                    path: source.to_path_buf(),
                    cause: e,
                });
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(WorkingCopyError::DestinationExhausted {
        attempts: ARCHIVE_ATTEMPTS,
    })
}

/// Platform rename-without-replacement. Linux goes through
/// `syscall(SYS_renameat2, ...)` because the pinned libc (0.2.189)
/// declares the named `renameat2` wrapper only for gnu targets, while
/// musl — the release target — ships only the syscall number, and NO libc
/// additions are permitted. macOS uses `renameatx_np` with `RENAME_EXCL`.
/// Both symbols/constants verified present in the vendored libc for every
/// target this workspace builds.
fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    let from = cstring(from)?;
    let to = cstring(to)?;
    let rc;
    #[cfg(target_os = "linux")]
    {
        // SAFETY: both CStrings outlive the call; the syscall number and
        // flag are the platform's own constants, and renameat2 with
        // AT_FDCWD + RENAME_NOREPLACE creates or fails, never replaces.
        rc = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
    }
    #[cfg(target_os = "macos")]
    {
        // SAFETY: both CStrings outlive the call; RENAME_EXCL makes the
        // rename fail rather than replace.
        rc = unsafe {
            libc::renameatx_np(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_EXCL,
            )
        };
    }
    if rc == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::EEXIST) | Some(libc::ENOTEMPTY) => {
            Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists))
        }
        _ => Err(err),
    }
}

fn cstring(path: &Path) -> std::io::Result<std::ffi::CString> {
    std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))
}

/// Resolve a row left `archive_pending` by a crash between the journal
/// persist and the journal retirement. Call this for every pending row at
/// startup BEFORE accepting new directory admissions, so no concurrent
/// allocation can observe a half-moved directory.
///
/// Cases (source = the row's recorded canonical path, destination = the
/// journaled destination):
///
/// - source present, destination absent → the commit may be retried: the
///   rename runs against the JOURNALED destination and both parents are
///   fsynced → [`ReconcileOutcome::Moved`].
/// - source absent, destination present, identity matching the row → the
///   rename had committed; both parents are RE-FSYNCED (durability before
///   journal retirement) → [`ReconcileOutcome::MetadataComplete`]. The
///   retirement transaction itself belongs to the teardown slice.
/// - destination occupied by an UNRELATED object → the journaled name was
///   taken between crash and reconciliation; a fresh UUID suffix is
///   persisted and the rename retried.
/// - identity mismatch anywhere → fail closed. This helper never moves,
///   removes, or adopts a stranger.
/// - source absent, destination absent → [`ReconcileOutcome::SourceMissing`].
pub fn reconcile_archive(conn: &Connection, working_copy_id: &str) -> Result<ReconcileOutcome> {
    reconcile_archive_with_parent_sync(conn, working_copy_id, None)
}

/// Recover through the same parent barriers used by the initial move.
/// A persistent injected sync failure must prevent both startup recovery
/// and repeated Delete from treating inode evidence as durable settlement.
pub(crate) fn reconcile_archive_with_parent_sync(
    conn: &Connection,
    working_copy_id: &str,
    parent_sync: Option<&ArchiveParentSync>,
) -> Result<ReconcileOutcome> {
    reconcile_archive_with_effects(conn, working_copy_id, parent_sync, &mut rename_noreplace)
}

/// Keep recovery's observations and journal updates real while allowing a
/// test to place a collision at the actual rename boundary. Replacing this
/// operation also permits deterministic filesystem refusal tests without
/// depending on the runner's uid or filesystem permissions.
fn reconcile_archive_with_effects(
    conn: &Connection,
    working_copy_id: &str,
    parent_sync: Option<&ArchiveParentSync>,
    rename: &mut dyn FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<ReconcileOutcome> {
    let row = get_working_copy(conn, working_copy_id)?
        .ok_or_else(|| WorkingCopyError::RowMissing(working_copy_id.to_string()))?;
    if row.allocation_state != AllocationState::ArchivePending {
        return Err(WorkingCopyError::WrongState(row.id.clone()));
    }
    let Some(journaled) = row.archive_destination.clone() else {
        return Err(WorkingCopyError::WrongState(row.id.clone()));
    };
    refuse_overlapping_archive(conn, &row)?;
    let root = verified_root(&row)?;
    let archive_root = ensure_archive_root(&root)?;
    let source = PathBuf::from(row.canonical_path.as_deref().unwrap_or_default());
    let destination_path = archive_root.join(&journaled);

    // The verified root bounds the lookup. Check the destination first:
    // its matching identity proves the rename completed, even when a
    // foreign object has since appeared at the old source name.
    let mut overlong_destination = false;
    let destination_meta = match observe(&destination_path) {
        Ok(observed) => Some(observed),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) if e.raw_os_error() == Some(libc::ENAMETOOLONG) => {
            // Older collision retries could journal a growing UUID chain.
            // An unreadable destination does not prove a completed move or
            // absence. Only a still-matching source below permits choosing
            // a legal replacement name; otherwise retain this evidence.
            overlong_destination = true;
            None
        }
        Err(e) => return Err(e.into()),
    };
    // A matching destination PROVES the rename already succeeded: the
    // destination is our directory (identity match), so we complete the
    // metadata (re-fsync both parents for durability) and leave whatever
    // occupies the old source path untouched. This is R1.6's "matching
    // destination wins" rule.
    if let Some(actual) = &destination_meta {
        let Some(expected) = row.path_identity else {
            return Err(WorkingCopyError::WrongState(row.id.clone()));
        };
        if same_directory(actual, expected, row.path_birth_ns) {
            sync_archive_parent(&archive_root, parent_sync)?;
            sync_archive_parent(&root, parent_sync)?;
            return Ok(ReconcileOutcome::MetadataComplete);
        }
        // Destination exists but does NOT match our identity: a stranger
        // holds the journaled name. Fall through to the source checks —
        // if the source is still present and valid, we move aside under
        // a fresh suffix; if not, there is nothing of ours left.
    }
    let source_present = match fs::symlink_metadata(&source) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.into()),
    };

    // Identity evidence is re-checked here too: a pending row's source
    // may have been swapped while the supervisor was down. Verify BEFORE
    // any rename; mismatch anywhere fails closed.
    let expected_identity = row.path_identity;
    if source_present {
        let Some(expected) = expected_identity else {
            return Err(WorkingCopyError::WrongState(row.id.clone()));
        };
        if !same_directory(&observe(&source)?, expected, row.path_birth_ns) {
            return Err(WorkingCopyError::IdentityMismatch {
                path: source.clone(),
            });
        }
    }

    if overlong_destination && !source_present {
        return Err(std::io::Error::from_raw_os_error(libc::ENAMETOOLONG).into());
    }
    // Every branch below that still renames needs the source present; a
    // completed move already returned above and is not refused here.
    if source_present {
        refuse_archive_destination_checkout(conn, &row)?;
    }
    // A recovery may happen much later than the original archive attempt.
    // Keep its journaled candidate first, but base any replacement on the
    // recorded basename and this recovery's timestamp, never on old suffixes.
    let collision_stem = format!("{}-{}", row.original_basename, utc_compact(now_unix()));
    match (source_present, &destination_meta) {
        (true, None) if !overlong_destination => {
            // Crash between journal and rename: the commit may be retried.
            let destination = rename_exclusive_into(
                conn,
                &row.id,
                &archive_root,
                &source,
                &journaled,
                &collision_stem,
                rename,
            )?;
            sync_archive_parent(&archive_root, parent_sync)?;
            sync_archive_parent(&root, parent_sync)?;
            Ok(ReconcileOutcome::Moved { destination })
        }
        (false, Some(actual)) => {
            let Some(expected) = row.path_identity else {
                return Err(WorkingCopyError::WrongState(row.id.clone()));
            };
            if !same_directory(actual, expected, row.path_birth_ns) {
                // An unrelated object holds the journaled name while the
                // source is gone. There is nothing left to move and
                // nothing here is ours: fail closed rather than adopt the
                // stranger or invent a suffix for a directory that no
                // longer exists.
                return Err(WorkingCopyError::IdentityMismatch {
                    path: destination_path,
                });
            }
            // Durability before journal retirement: re-fsync both parents
            // so the completed rename is known-persistent before the
            // journal is allowed to die.
            sync_archive_parent(&archive_root, parent_sync)?;
            sync_archive_parent(&root, parent_sync)?;
            Ok(ReconcileOutcome::MetadataComplete)
        }
        (true, _) => {
            // Both exist: the destination cannot be our directory (same
            // identity as the source is impossible for a directory, and a
            // directory hardlink does not exist), so it is an unrelated
            // object. An unreadable overlong destination also reaches here
            // only after verifying the source: one directory inode cannot
            // already be archived and still occupy the recorded source.
            // Never overwrite — move under one fresh bounded suffix.
            let retry = format!("{}-{}", collision_stem, uuid::Uuid::new_v4().simple());
            repoint_journal(conn, &row.id, &retry)?;
            let destination = rename_exclusive_into(
                conn,
                &row.id,
                &archive_root,
                &source,
                &retry,
                &collision_stem,
                rename,
            )?;
            sync_archive_parent(&archive_root, parent_sync)?;
            sync_archive_parent(&root, parent_sync)?;
            Ok(ReconcileOutcome::Moved { destination })
        }
        (false, None) => Ok(ReconcileOutcome::SourceMissing),
    }
}

/// Give up managing a checkout that its last session's Delete could not
/// archive safely: the registry row goes, and the folder stays wherever it is.
///
/// Deliberately not [`retire`], whose `retired` state promises the directory
/// was archived or confirmed gone. SPEC.md "Managed checkouts" makes
/// archiving never block Delete, so this is the outcome for every archive
/// failure, including a removed or remounted root, a filesystem without
/// no-replace rename, and an archive destination too long for the system.
/// The Delete's reply names the folder so the user can deal with it; a
/// folder still at its path keeps occupying its name for later checkouts.
pub fn release_unarchived(conn: &Connection, working_copy_id: &str) -> Result<()> {
    let deleted = conn.execute(
        "DELETE FROM working_copies WHERE id = ?1 AND allocation_state IN (?2, ?3)",
        rusqlite::params![
            working_copy_id,
            AllocationState::Allocated.as_str(),
            AllocationState::ArchivePending.as_str(),
        ],
    )?;
    if deleted == 0 {
        return Err(WorkingCopyError::WrongState(working_copy_id.to_string()));
    }
    Ok(())
}

/// Terminal transition `archive_pending` → `retired`. Owned by the
/// teardown slice, called AFTER its final SQLite transaction: a `retired`
/// row is a promise that the directory is archived (or confirmed gone)
/// and the registry entry is history.
pub fn retire(conn: &Connection, working_copy_id: &str) -> Result<()> {
    let updated = conn.execute(
        "UPDATE working_copies SET allocation_state = ?2 \
         WHERE id = ?1 AND allocation_state = ?3",
        rusqlite::params![
            working_copy_id,
            AllocationState::Retired.as_str(),
            AllocationState::ArchivePending.as_str(),
        ],
    )?;
    if updated == 0 {
        return Err(WorkingCopyError::WrongState(working_copy_id.to_string()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Time formatting (the archive destination's suffix)
// ---------------------------------------------------------------------------

/// `UTC YYYYMMDDTHHMMSSZ` from unix seconds. The civil-date math is inline
/// because the crate carries no calendar dependency for one suffix.
fn utc_compact(unix_seconds: i64) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let secs_of_day = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = farhelm_proto::time::civil_from_days(days);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        year,
        month,
        day,
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::{Arc, Barrier};

    /// Registry tables plus the session fields allocation reads to recover
    /// retained references. The reduced session table deliberately has no
    /// runtime status: every retained row protects its recorded directory.
    /// Store tests separately pin the complete production schema.
    fn registry_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        create_registry_tables(&conn);
        conn
    }

    /// A connection over a SHARED database file, so concurrent threads
    /// can race the same registry rows without sharing a connection
    /// (rusqlite connections are Send, not Sync).
    fn open_registry(db_path: &Path) -> Connection {
        Connection::open(db_path).expect("open db")
    }

    /// Shared schema for in-memory tests and independently opened racing
    /// connections; both must exercise the same allocation transaction.
    fn create_registry_tables(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE working_copies (
                 id                   TEXT PRIMARY KEY,
                 canonical_root       TEXT NOT NULL,
                 canonical_path       TEXT,
                 repo_owner           TEXT NOT NULL,
                 repo_name            TEXT NOT NULL,
                 original_basename    TEXT NOT NULL,
                 origin_session_id    TEXT NOT NULL,
                 root_device          INTEGER,
                 root_inode           INTEGER,
                 path_device          INTEGER,
                 path_inode           INTEGER,
                 allocation_state     TEXT NOT NULL,
                 archive_destination  TEXT,
                 preparation_snapshot TEXT,
                 created_at           INTEGER NOT NULL,
                 root_birth_ns        INTEGER,
                 path_birth_ns        INTEGER
             ) STRICT;
             CREATE TABLE sessions (
                 id TEXT PRIMARY KEY,
                 canonical_cwd TEXT,
                 cwd TEXT NOT NULL
             ) STRICT;
             CREATE TABLE working_copy_members (
                 session_id      TEXT NOT NULL,
                 working_copy_id TEXT NOT NULL,
                 PRIMARY KEY (session_id, working_copy_id)
             ) STRICT;",
        )
        .expect("registry tables");
    }

    fn planned_spec(root: &Path, basename: &str) -> PlannedWorkingCopy {
        PlannedWorkingCopy {
            id: uuid::Uuid::new_v4().to_string(),
            canonical_root: root.to_string_lossy().into_owned(),
            repo_owner: "octocat".to_string(),
            repo_name: "hello-world".to_string(),
            original_basename: basename.to_string(),
            origin_session_id: format!("s-{}", basename),
            root_identity: None,
            preparation_snapshot: None,
        }
    }

    fn planned_row(conn: &Connection, root: &Path, basename: &str) -> WorkingCopyRow {
        record_planned(conn, &planned_spec(root, basename)).expect("planned row")
    }

    /// Whether the filesystem under `path` reports birth times, asked of
    /// coreutils' `stat` (`%W`, 0 or `-` when unknown) rather than of the
    /// code under test, so a broken `observe` fails the birth-time tests
    /// instead of making them skip.
    fn filesystem_reports_birth_time(path: &Path) -> bool {
        std::process::Command::new("stat")
            .args(["-c", "%W"])
            .arg(path)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|text| text.trim().parse::<i64>().ok())
            .is_some_and(|seconds| seconds > 0)
    }

    fn identity_of_path(path: &Path) -> DirectoryIdentity {
        let meta = fs::symlink_metadata(path).expect("stat");
        (meta.dev(), meta.ino())
    }

    /// Wrap a real directory iterator while counting the actual `next` calls,
    /// including the call that observes EOF.
    fn counted_entries(
        path: &Path,
        calls: Rc<Cell<usize>>,
    ) -> impl Iterator<Item = std::io::Result<fs::DirEntry>> {
        let mut entries = fs::read_dir(path).expect("read fixture root");
        std::iter::from_fn(move || {
            calls.set(calls.get() + 1);
            entries.next()
        })
    }

    fn entries(path: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(path)
            .expect("read dir")
            .map(|e| e.expect("entry").file_name().into_string().expect("utf8"))
            .collect();
        names.sort();
        names
    }

    // -- occupancy scan ---------------------------------------------------

    /// Titled and numeric aliases both occupy generated names, while the
    /// reserved archive entry remains outside the naming namespace.
    #[test]
    fn occupied_scan_collects_titled_aliases_and_skips_archive() {
        let dir = tempfile::tempdir().expect("tempdir");
        for name in ["bar", "bar-1", "bar-fix-parser", "bar-01", "other"] {
            fs::write(dir.path().join(name), b"").expect("fixture entry");
        }
        fs::create_dir(dir.path().join("bar-directory")).expect("directory fixture");
        let absent = dir.path().join("absent-target");
        assert!(!absent.exists());
        std::os::unix::fs::symlink(&absent, dir.path().join("bar-link"))
            .expect("dangling symlink fixture");
        assert!(
            fs::symlink_metadata(dir.path().join("bar-link"))
                .unwrap()
                .is_symlink()
        );
        assert!(!dir.path().join("bar-link").exists());
        fs::create_dir(dir.path().join(ARCHIVE_DIR_NAME)).expect("archive directory");

        let scan = occupied_related_names(dir.path(), "bar", 16).expect("complete scan");
        assert_eq!(scan.names.len(), 6);
        assert!(scan.complete);
        for name in [
            "bar",
            "bar-1",
            "bar-fix-parser",
            "bar-01",
            "bar-directory",
            "bar-link",
        ] {
            assert!(scan.names.contains(name), "{name} must be occupied");
        }
        assert!(!scan.names.contains("other"));
        assert!(!scan.names.contains(ARCHIVE_DIR_NAME));
    }

    /// The preview naming helper must see titled collisions and must treat
    /// `bar-01` as distinct from the numeric candidate `bar-1`.
    #[test]
    fn occupied_scan_drives_checkout_basename_without_numeric_aliasing() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("bar-1"), b"").expect("numeric fixture");
        fs::write(dir.path().join("bar-fix"), b"").expect("title fixture");
        let scan = occupied_related_names(dir.path(), "bar", 8).expect("complete scan");
        let repo = farhelm_proto::github_checkout::parse_github_repo("acme/bar")
            .expect("valid repository");
        let occupied = |name: &str| scan.names.contains(name);

        let numbered = farhelm_proto::github_checkout::checkout_basename(&repo, None, &occupied)
            .expect("lowest positive candidate");
        assert_eq!(numbered.basename, "bar-2");
        assert_eq!(
            farhelm_proto::github_checkout::checkout_basename(&repo, Some("fix"), &occupied)
                .expect_err("titled collision"),
            farhelm_proto::github_checkout::NameError::Occupied
        );

        let alias_only = tempfile::tempdir().expect("alias-only tempdir");
        fs::write(alias_only.path().join("bar-01"), b"").expect("non-alias fixture");
        let scan = occupied_related_names(alias_only.path(), "bar", 8).expect("complete rescan");
        let occupied = |name: &str| scan.names.contains(name);
        let numbered = farhelm_proto::github_checkout::checkout_basename(&repo, None, &occupied)
            .expect("bar-01 does not occupy bar-1");
        assert_eq!(numbered.basename, "bar-1");
    }

    /// A folder whose name differs from a candidate only in letter case
    /// occupies that candidate.
    ///
    /// Why it matters: on a case-insensitive filesystem, macOS's default,
    /// `Bar-1` makes `mkdir bar-1` fail. A scan that called `bar-1` free
    /// made every preview propose it and every create fail, with no way out
    /// for an unnamed checkout. Specified: with `Bar-1` and `BAR-FIX` in the
    /// root, the scan reports `bar-1` and `bar-fix` occupied, the unnamed
    /// candidate moves on to `bar-2`, and the title "fix" is refused as
    /// occupied.
    #[test]
    fn occupied_scan_treats_case_variants_as_occupied() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir(dir.path().join("Bar-1")).expect("case-variant numeric");
        fs::create_dir(dir.path().join("BAR-FIX")).expect("case-variant title");
        let scan = occupied_related_names(dir.path(), "bar", 8).expect("complete scan");
        assert!(scan.names.contains("bar-1") && scan.names.contains("bar-fix"));
        let repo = farhelm_proto::github_checkout::parse_github_repo("acme/bar")
            .expect("valid repository");
        let occupied = |name: &str| scan.names.contains(name);
        assert_eq!(
            farhelm_proto::github_checkout::checkout_basename(&repo, None, &occupied)
                .expect("lowest free candidate")
                .basename,
            "bar-2"
        );
        assert_eq!(
            farhelm_proto::github_checkout::checkout_basename(&repo, Some("fix"), &occupied)
                .expect_err("titled case-variant collision"),
            farhelm_proto::github_checkout::NameError::Occupied
        );
    }

    /// Spec: a fresh checkout can never be named `ARCHIVE_DIR_NAME`, even in
    /// an empty root where no registry row exists to derive the archive path
    /// from, and in a root where the archive directory already exists.
    ///
    /// Ordinary titles produce the reserved name exactly. A checkout that took
    /// it became the destination for every later archive in that root, and
    /// its own delete failed forever trying to move it into itself. The
    /// earlier nesting guard only knew the archive path from active registry
    /// rows, so the first checkout in a root was unprotected; this pins the
    /// name reservation that does not depend on the registry.
    #[test]
    fn the_archive_directory_name_is_never_a_free_checkout_name() {
        for repo in [
            "acme/farhelm",
            "acme/farhelm-archived",
            "acme/farhelm-archived-working",
        ] {
            let repo =
                farhelm_proto::github_checkout::parse_github_repo(repo).expect("valid repository");
            let title = &ARCHIVE_DIR_NAME[repo.name.len() + 1..].replace('-', " ");
            for existing_archive in [false, true] {
                let root = tempfile::tempdir().expect("empty root");
                if existing_archive {
                    fs::create_dir(root.path().join(ARCHIVE_DIR_NAME)).expect("archive directory");
                }
                let scan =
                    occupied_related_names(root.path(), &repo.name, 8).expect("complete scan");
                assert!(scan.names.contains(ARCHIVE_DIR_NAME));
                let occupied = |name: &str| scan.names.contains(name);
                for title in [title.as_str(), ARCHIVE_DIR_NAME] {
                    assert_eq!(
                        farhelm_proto::github_checkout::checkout_basename(
                            &repo,
                            Some(title),
                            &occupied
                        )
                        .expect_err("the reserved name must not be proposed"),
                        farhelm_proto::github_checkout::NameError::Occupied,
                        "repo {} title {title:?}",
                        repo.name
                    );
                }
            }
        }
    }

    /// A missing root is an unknown scan, and injected iterator failures are
    /// surfaced instead of being mistaken for an empty directory.
    #[test]
    fn occupied_scan_fails_closed_for_missing_root_and_iterator_errors() {
        let parent = tempfile::tempdir().expect("tempdir");
        let missing = parent.path().join("missing-root");
        assert!(!missing.exists(), "fixture root must be absent before scan");
        assert!(matches!(
            occupied_related_names(&missing, "bar", 8),
            Err(WorkingCopyError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound
        ));

        let iterator = std::iter::once(Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "injected directory read failure",
        )));
        assert!(matches!(
            occupied_related_names_from_entries(iterator, "bar", 8),
            Err(WorkingCopyError::Io(error)) if error.kind() == std::io::ErrorKind::PermissionDenied
        ));
    }

    /// The cap bounds calls to `next`: EOF is queried once within budget,
    /// while exact-cap and over-cap sources stop without an extra call.
    #[test]
    fn occupied_scan_cap_counts_iterator_calls_and_refuses_at_boundary() {
        let dir = tempfile::tempdir().expect("tempdir");
        for name in ["one", "two", "three"] {
            fs::write(dir.path().join(name), b"").expect("fixture entry");
        }

        let calls = Rc::new(Cell::new(0));
        let entries = counted_entries(dir.path(), Rc::clone(&calls));
        assert!(matches!(
            occupied_related_names_from_entries(entries, "bar", 3),
            Err(WorkingCopyError::IncompleteScan { scanned: 3 })
        ));
        assert_eq!(calls.get(), 3);

        let calls = Rc::new(Cell::new(0));
        let entries = counted_entries(dir.path(), Rc::clone(&calls));
        assert!(matches!(
            occupied_related_names_from_entries(entries, "bar", 2),
            Err(WorkingCopyError::IncompleteScan { scanned: 2 })
        ));
        assert_eq!(calls.get(), 2);

        let calls = Rc::new(Cell::new(0));
        let entries = counted_entries(dir.path(), Rc::clone(&calls));
        let scan = occupied_related_names_from_entries(entries, "bar", 4).expect("EOF at cap");
        assert!(scan.complete);
        assert_eq!(calls.get(), 4);

        let empty = tempfile::tempdir().expect("empty tempdir");
        let calls = Rc::new(Cell::new(0));
        let entries = counted_entries(empty.path(), Rc::clone(&calls));
        let scan = occupied_related_names_from_entries(entries, "bar", 1).expect("EOF");
        assert!(scan.complete);
        assert!(scan.names.is_empty());
        assert_eq!(calls.get(), 1);

        let archive_only = tempfile::tempdir().expect("archive-only tempdir");
        fs::create_dir(archive_only.path().join(ARCHIVE_DIR_NAME)).expect("archive directory");
        let calls = Rc::new(Cell::new(0));
        let entries = counted_entries(archive_only.path(), Rc::clone(&calls));
        assert!(matches!(
            occupied_related_names_from_entries(entries, "bar", 1),
            Err(WorkingCopyError::IncompleteScan { scanned: 1 })
        ));
        assert_eq!(calls.get(), 1);

        let calls = Rc::new(Cell::new(0));
        let entries = counted_entries(dir.path(), Rc::clone(&calls));
        assert!(matches!(
            occupied_related_names_from_entries(entries, "bar", 0),
            Err(WorkingCopyError::IncompleteScan { scanned: 0 })
        ));
        assert_eq!(calls.get(), 0);
    }

    /// Invalid UTF-8 cannot equal an ASCII candidate, but it still consumes
    /// the bounded observation budget and can therefore force refusal.
    #[test]
    #[cfg(unix)]
    fn occupied_scan_counts_invalid_utf8_toward_cap() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join(OsString::from_vec(vec![b'x', 0xff])), b"")
            .expect("invalid UTF-8 fixture");
        let scan = occupied_related_names(dir.path(), "bar", 2).expect("complete scan");
        assert!(scan.names.is_empty());
        assert!(scan.complete);
        assert!(matches!(
            occupied_related_names(dir.path(), "bar", 1),
            Err(WorkingCopyError::IncompleteScan { scanned: 1 })
        ));
    }

    // -- allocation: the one-winner contract ------------------------------

    /// Two independent planned creates compete for the same name:
    /// both start allocation together (barrier), exactly one wins, the loser
    /// conflicts, NO other directory exists anywhere, the captured
    /// identity equals stat of the surviving directory, and the row is
    /// allocated with its member attached.
    #[test]
    fn concurrent_allocate_is_one_winner_one_conflict_and_no_duplicate_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        // The database lives OUTSIDE the scanned root so the root shows
        // exactly what the allocator did to the filesystem.
        let db_dir = tempfile::tempdir().expect("tempdir");
        let db_path = db_dir.path().join("registry.db");
        let rows = {
            let conn = open_registry(&db_path);
            create_registry_tables(&conn);
            ["first", "second"].map(|origin| {
                let mut spec = planned_spec(dir.path(), "bar");
                spec.origin_session_id = origin.to_string();
                record_planned(&conn, &spec).expect("independent planned request")
            })
        };
        let barrier = Arc::new(Barrier::new(2));
        let db_path = Arc::new(db_path);
        let root_identity = identity_of_path(dir.path());

        let mut handles = Vec::new();
        for row in &rows {
            let db_path = Arc::clone(&db_path);
            let barrier = Arc::clone(&barrier);
            let id = row.id.clone();
            let root_identity = Some(root_identity);
            handles.push(std::thread::spawn(move || {
                let conn = open_registry(&db_path);
                let _ = barrier.wait();
                allocate(&conn, &id, root_identity)
            }));
        }
        let results: Vec<_> = handles
            .into_iter()
            .map(|h| h.join().expect("join"))
            .collect();
        let winners: Vec<_> = results.iter().filter(|r| r.is_ok()).collect();
        assert_eq!(
            winners.len(),
            1,
            "exactly one allocate may win: got {results:?}"
        );
        for r in &results {
            if let Err(AllocationFailure::PreMkdir(WorkingCopyError::Conflict { .. })) = r {
                // the expected loser shape
            } else if r.is_err() {
                panic!("loser must conflict, not fail otherwise: {r:?}");
            }
        }

        // No duplicate directory anywhere in the root: exactly the winner.
        assert_eq!(entries(dir.path()), vec!["bar"]);
        let surviving = identity_of_path(&dir.path().join("bar"));

        let accepted = winners.into_iter().next().unwrap().as_ref().unwrap();
        assert_eq!(
            accepted.identity, surviving,
            "captured identity matches stat"
        );
        assert_eq!(accepted.row.allocation_state, AllocationState::Allocated);
        assert_eq!(accepted.row.path_identity, Some(surviving));
        assert_eq!(
            accepted.row.canonical_path.as_deref(),
            Some(
                dir.path()
                    .join("bar")
                    .canonicalize()
                    .expect("canonicalize")
                    .to_str()
                    .expect("utf8")
            )
        );
        {
            let conn = open_registry(&db_path);
            assert_eq!(member_count(&conn, &accepted.row.id).expect("count"), 1);
            let loser = rows.iter().find(|row| row.id != accepted.row.id).unwrap();
            let retained = get_working_copy(&conn, &loser.id).unwrap().unwrap();
            assert_eq!(retained.allocation_state, AllocationState::Planned);
            assert_eq!(retained.path_identity, None);
            assert_eq!(member_count(&conn, &loser.id).unwrap(), 0);
        }
    }

    /// All three occupancy shapes at the explicit target name refuse with
    /// Conflict, leave the row planned, and create nothing extra.
    #[test]
    fn allocate_conflicts_on_occupied_target_names_in_every_shape() {
        for shape in ["file", "dir", "dangling-symlink"] {
            let conn = registry_conn();
            let dir = tempfile::tempdir().expect("tempdir");
            let root = dir.path();
            match shape {
                "file" => fs::write(root.join("bar"), b"").expect("file"),
                "dir" => fs::create_dir(root.join("bar")).expect("dir"),
                _ => std::os::unix::fs::symlink("/nowhere", root.join("bar"))
                    .expect("dangling symlink"),
            }
            let row = planned_row(&conn, root, "bar");
            let err = allocate(&conn, &row.id, None).expect_err("must conflict");
            assert!(
                matches!(
                    err,
                    AllocationFailure::PreMkdir(WorkingCopyError::Conflict { .. })
                ),
                "shape {shape}: {err}"
            );
            let fresh = get_working_copy(&conn, &row.id)
                .expect("row")
                .expect("present");
            assert_eq!(fresh.allocation_state, AllocationState::Planned);
            assert_eq!(
                entries(root),
                vec!["bar"],
                "allocation created nothing beyond the pre-existing occupant"
            );
        }
    }

    /// A root whose identity does not match the caller's expectation is
    /// refused before any mkdir happens.
    #[test]
    fn allocate_fails_closed_when_the_root_identity_does_not_match() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        let fake: DirectoryIdentity = (u64::MAX - 1, u64::MAX - 1);
        assert!(matches!(
            allocate(&conn, &row.id, Some(fake)),
            Err(AllocationFailure::PreMkdir(
                WorkingCopyError::IdentityMismatch { .. }
            ))
        ));
        assert!(
            !dir.path().join("bar").exists(),
            "no mkdir may happen after a failed root-identity check"
        );
    }

    /// An already-allocated row cannot be allocated again.
    #[test]
    fn allocate_rejects_a_row_that_is_not_planned() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("first allocate");
        assert!(matches!(
            allocate(&conn, &row.id, None),
            Err(AllocationFailure::PreMkdir(WorkingCopyError::WrongState(_)))
        ));
    }

    /// Allocation must recover retained references when an externally
    /// removed directory name is reused. Component containment excludes
    /// similarly prefixed siblings; canonical history wins over raw cwd.
    #[test]
    fn allocation_attaches_retained_paths_atomically() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().unwrap();
        let target = root.join("bar");
        let row = planned_row(&conn, &root, "bar");
        let fixtures = [
            ("same", Some(target.clone()), root.join("elsewhere")),
            ("child", Some(target.join("subdir")), root.join("elsewhere")),
            ("legacy", None, target.join("old/../subdir")),
            ("prefix", None, root.join("bar-other")),
            ("relative", None, PathBuf::from("bar/subdir")),
            (
                "canonical-wins",
                Some(root.join("elsewhere")),
                target.clone(),
            ),
        ];
        for (id, canonical, cwd) in fixtures {
            conn.execute(
                "INSERT INTO sessions VALUES (?1, ?2, ?3)",
                rusqlite::params![
                    id,
                    canonical.map(|p| p.to_string_lossy().into_owned()),
                    cwd.to_string_lossy(),
                ],
            )
            .unwrap();
        }
        assert!(
            !target.exists(),
            "retained references must not require a live directory"
        );
        assert_eq!(member_count(&conn, &row.id).unwrap(), 0);
        allocate(&conn, &row.id, None).expect("allocate the reused name");
        assert_eq!(
            member_count(&conn, &row.id).unwrap(),
            4,
            "origin plus three retained references"
        );
        for id in ["same", "child", "legacy"] {
            assert_eq!(member_working_copies(&conn, id).unwrap()[0].id, row.id);
        }
        for id in ["prefix", "relative", "canonical-wins"] {
            assert!(member_working_copies(&conn, id).unwrap().is_empty(), "{id}");
        }
    }

    /// A retained-reference write failure must roll back identity publication
    /// along with memberships. The successful mkdir remains uncertain evidence
    /// for the caller's post-mkdir refusal, never an unprotected Allocated row.
    #[test]
    fn allocation_retained_membership_failure_keeps_planned_evidence() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().unwrap();
        let target = root.join("bar");
        let row = planned_row(&conn, &root, "bar");
        conn.execute(
            "INSERT INTO sessions VALUES ('retained', NULL, ?1)",
            [target.to_string_lossy()],
        )
        .unwrap();
        conn.execute_batch("CREATE TRIGGER refuse_retained BEFORE INSERT ON working_copy_members
            WHEN NEW.session_id = 'retained' BEGIN SELECT RAISE(ABORT, 'retained reference refused'); END;").unwrap();
        assert!(!target.exists());
        assert_eq!(member_count(&conn, &row.id).unwrap(), 0);
        let failure = allocate(&conn, &row.id, None).expect_err("membership failure");
        assert!(matches!(
            failure,
            AllocationFailure::PostMkdir(WorkingCopyError::Db(_))
        ));
        assert!(target.is_dir(), "mkdir evidence is retained");
        let persisted = get_working_copy(&conn, &row.id).unwrap().unwrap();
        assert_eq!(persisted.allocation_state, AllocationState::Planned);
        assert!(persisted.path_identity.is_none());
        assert_eq!(member_count(&conn, &row.id).unwrap(), 0);
    }

    /// Membership is explicit, row-counted, and composable: add and
    /// remove inside one transaction, count by actual rows.
    #[test]
    fn membership_is_explicit_row_counted_and_transaction_composable() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        assert_eq!(member_count(&conn, &row.id).expect("count"), 0);

        let tx = conn.unchecked_transaction().expect("tx");
        add_member(&tx, "s1", &row.id).expect("attach");
        add_member(&tx, "s2", &row.id).expect("attach");
        tx.commit().expect("commit");
        assert_eq!(member_count(&conn, &row.id).expect("count"), 2);

        // Re-attach is a no-op; remove of a non-member is a no-op.
        {
            let tx = conn.unchecked_transaction().expect("tx");
            add_member(&tx, "s1", &row.id).expect("re-attach");
            remove_member(&tx, "no-such-session", &row.id).expect("non-member removal");
            tx.commit().expect("commit");
        }
        assert_eq!(member_count(&conn, &row.id).expect("count"), 2);
        {
            let tx = conn.unchecked_transaction().expect("tx");
            remove_member(&tx, "s1", &row.id).expect("detach");
            tx.commit().expect("commit");
        }
        assert_eq!(member_count(&conn, &row.id).expect("count"), 1);
    }

    // -- identity verification ---------------------------------------------

    /// Lifetime membership must neither confer fresh-origin authority on a
    /// borrower nor hide the original plan when its membership is missing.
    #[test]
    fn origin_lookup_is_independent_of_membership() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "origin");
        add_member(&conn, "borrower", &row.id).expect("borrower membership");
        assert_eq!(member_working_copies(&conn, "borrower").unwrap().len(), 1);
        assert!(
            member_working_copies(&conn, &row.origin_session_id)
                .unwrap()
                .is_empty()
        );

        assert!(origin_working_copy(&conn, "borrower").unwrap().is_none());
        assert_eq!(
            origin_working_copy(&conn, &row.origin_session_id)
                .unwrap()
                .unwrap()
                .id,
            row.id
        );
    }

    /// Corrupt duplicate provenance cannot choose an arbitrary checkout to
    /// recover, even when membership happens to favor one of the records.
    #[test]
    fn origin_lookup_refuses_duplicate_provenance() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let first = planned_row(&conn, dir.path(), "first");
        let mut second = planned_spec(dir.path(), "second");
        second.origin_session_id = first.origin_session_id.clone();
        record_planned(&conn, &second).expect("duplicate fixture provenance");
        add_member(&conn, &first.origin_session_id, &first.id).unwrap();
        assert_eq!(all_working_copies(&conn).unwrap().len(), 2);
        assert_eq!(
            member_working_copies(&conn, &first.origin_session_id)
                .unwrap()
                .len(),
            1
        );

        assert!(matches!(
            origin_working_copy(&conn, &first.origin_session_id),
            Err(WorkingCopyError::AmbiguousOrigin(id)) if id == first.origin_session_id
        ));
    }

    /// verify_identity distinguishes the four answers the registry
    /// contract needs: match, missing, different object, uncaptured.
    #[test]
    fn verify_identity_reports_match_missing_and_different_object() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        // planned rows have no captured identity to verify against
        assert_eq!(
            verify_identity(&row).expect("verify"),
            IdentityStatus::NoCapturedIdentity
        );

        allocate(&conn, &row.id, None).expect("allocate");
        let allocated = get_working_copy(&conn, &row.id)
            .expect("row")
            .expect("present");
        assert_eq!(
            verify_identity(&allocated).expect("verify"),
            IdentityStatus::Matches
        );

        // A DIFFERENT object that coexisted with the original is moved
        // into the path: DifferentObject. The swap directory is created
        // BEFORE the original is removed — a remove-then-recreate could
        // legitimately reuse the inode, which would be a false match, so
        // the replacement must be provably another object. The MISSING
        // answer is checked in between, while nothing is at the path.
        let swap = dir.path().join("swap-source");
        fs::create_dir(&swap).expect("coexisting swap dir");
        fs::remove_dir(dir.path().join("bar")).expect("remove");
        assert_eq!(
            verify_identity(&allocated).expect("verify"),
            IdentityStatus::Missing
        );
        fs::rename(&swap, dir.path().join("bar")).expect("swap");
        assert_eq!(
            verify_identity(&allocated).expect("verify"),
            IdentityStatus::DifferentObject
        );
        assert_eq!(
            verify_identity(&allocated).expect("verify"),
            IdentityStatus::DifferentObject
        );
    }

    // -- archive move -------------------------------------------------------

    /// Happy path: the directory appears exactly once under the archive
    /// with a timestamp suffix, the source is gone, and the row is
    /// archive_pending with the destination persisted.
    #[test]
    fn archive_move_happy_path_archives_once_and_journals() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        fs::write(dir.path().join("bar").join("payload.txt"), b"data").expect("content");

        match archive_move(&conn, &row.id).expect("archive") {
            ArchiveOutcome::Archived { destination } => {
                assert!(destination.starts_with("bar-"), "{destination}");
                assert!(destination.ends_with('Z'), "{destination}");
            }
            other => panic!("expected Archived, got {other:?}"),
        }
        let fresh = get_working_copy(&conn, &row.id)
            .expect("row")
            .expect("present");
        assert_eq!(fresh.allocation_state, AllocationState::ArchivePending);
        assert!(fresh.archive_destination.is_some());
        assert_eq!(entries(dir.path()), vec![ARCHIVE_DIR_NAME]);
        assert_eq!(
            entries(&dir.path().join(ARCHIVE_DIR_NAME)),
            vec![fresh.archive_destination.clone().expect("dest")]
        );
        assert!(
            dir.path()
                .join(ARCHIVE_DIR_NAME)
                .join(fresh.archive_destination.expect("dest"))
                .join("payload.txt")
                .is_file()
        );
    }

    /// An empty occupant created at the actual rename boundary forces a
    /// UUID suffix. Its identity must survive: ordinary replacing rename
    /// would overwrite an empty directory, so a nonempty fixture would
    /// fail to distinguish that regression from no-replace semantics.
    #[test]
    fn archive_move_collision_appends_a_uuid_suffix_and_persists_it() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        let archive_root = dir.path().join(ARCHIVE_DIR_NAME);
        fs::create_dir(&archive_root).expect("archive root");
        let mut occupant = None;
        let mut occupant_identity = None;
        let mut attempts = 0;
        let outcome = archive_move_with_effects(&conn, &row.id, None, &mut |from, to| {
            attempts += 1;
            if attempts == 1 {
                fs::create_dir(to)?;
                occupant_identity = Some(identity_of_path(to));
                occupant = Some(to.to_path_buf());
                let error = rename_noreplace(from, to).expect_err("real collision");
                assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
                return Err(error);
            }
            rename_noreplace(from, to)
        })
        .expect("archive");
        assert_eq!(attempts, 2, "one collision followed by one successful move");
        let occupant = occupant.expect("collision fixture executed");
        let occupant_name = occupant.file_name().unwrap().to_str().unwrap();
        assert_eq!(Some(identity_of_path(&occupant)), occupant_identity);
        assert!(entries(&occupant).is_empty());

        match outcome {
            ArchiveOutcome::Archived { destination } => {
                assert!(
                    destination.starts_with(&format!("{occupant_name}-")),
                    "{destination} must be a UUID-suffixed form of an occupied stem"
                );
                assert_eq!(
                    destination.len(),
                    destination
                        .rsplit_once('-')
                        .map(|(prefix, _)| prefix.len() + 1 + 32)
                        .unwrap(),
                    "the suffix is one dash plus a 32-char UUID"
                );
            }
            other => panic!("expected Archived, got {other:?}"),
        }
        let fresh = get_working_copy(&conn, &row.id)
            .expect("row")
            .expect("present");
        let names = entries(&archive_root);
        assert_eq!(
            fresh.archive_destination.as_deref(),
            names
                .iter()
                .find(|name| name.as_str() != occupant_name)
                .map(String::as_str),
            "the persisted journal names exactly the suffixed archive"
        );
        assert_eq!(
            names.len(),
            2,
            "the occupant and the suffixed archive both exist"
        );
    }

    /// Maximum-length names must survive repeated real collisions without
    /// growing the journal past the filesystem component limit. Reopening
    /// an occupied, already-suffixed journal exercises the recovery branch
    /// as well as initial archival; empty foreign directories distinguish
    /// no-replace rename from a replacing syscall.
    #[test]
    fn maximum_archive_names_survive_repeated_collisions_and_reopen() {
        for recovering in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("root");
            fs::create_dir(&root).unwrap();
            let db = dir.path().join("registry.sqlite");
            let conn = open_registry(&db);
            create_registry_tables(&conn);
            let basename = format!(
                "bar-{}",
                "x".repeat(farhelm_proto::github_checkout::MAX_BASENAME_BYTES - 4)
            );
            let row = planned_row(&conn, &root, &basename);
            let allocated = allocate(&conn, &row.id, None).unwrap();
            let source = root.join(&basename);
            fs::write(source.join("owned"), b"retained content").unwrap();
            let archive_root = ensure_archive_root(&root).unwrap();
            let mut occupants = Vec::new();
            if recovering {
                let journaled =
                    format!("{basename}-20000101T000000Z-{}", uuid::Uuid::nil().simple());
                assert_eq!(journaled.len(), 250);
                persist_journal(&conn, &row.id, &journaled, AllocationState::Allocated).unwrap();
                let occupant = archive_root.join(journaled);
                fs::create_dir(&occupant).unwrap();
                occupants.push((occupant.clone(), identity_of_path(&occupant)));
            }
            drop(conn);
            let conn = open_registry(&db);
            assert_eq!(identity_of_path(&source), allocated.identity);
            let mut attempts = 0;
            let mut collide = |from: &Path, to: &Path| {
                attempts += 1;
                assert_eq!(identity_of_path(from), allocated.identity);
                let candidate = to.file_name().unwrap().to_str().unwrap();
                assert!(
                    candidate.len() <= 250,
                    "unrecoverable candidate: {} bytes",
                    candidate.len()
                );
                let pending = get_working_copy(&conn, &row.id).unwrap().unwrap();
                assert_eq!(pending.archive_destination.as_deref(), Some(candidate));
                if attempts <= 2 {
                    fs::create_dir(to)?;
                    occupants.push((to.to_path_buf(), identity_of_path(to)));
                    let error =
                        rename_noreplace(from, to).expect_err("real empty-directory collision");
                    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
                    Err(error)
                } else {
                    rename_noreplace(from, to)
                }
            };
            let destination = if recovering {
                match reconcile_archive_with_effects(&conn, &row.id, None, &mut collide).unwrap() {
                    ReconcileOutcome::Moved { destination } => destination,
                    other => panic!("expected one move: {other:?}"),
                }
            } else {
                match archive_move_with_effects(&conn, &row.id, None, &mut collide).unwrap() {
                    ArchiveOutcome::Archived { destination } => destination,
                    other => panic!("expected one move: {other:?}"),
                }
            };
            assert_eq!(attempts, 3);
            assert_eq!(destination.len(), 250);
            let moved = archive_root.join(&destination);
            assert_eq!(identity_of_path(&moved), allocated.identity);
            assert_eq!(fs::read(moved.join("owned")).unwrap(), b"retained content");
            assert!(!source.exists());
            for (occupant, identity) in &occupants {
                assert_eq!(identity_of_path(occupant), *identity);
                assert!(entries(occupant).is_empty());
            }
            assert_eq!(entries(&archive_root).len(), occupants.len() + 1);
            assert_eq!(
                get_working_copy(&conn, &row.id)
                    .unwrap()
                    .unwrap()
                    .archive_destination,
                Some(destination)
            );
        }
    }

    /// An old oversized collision journal is repairable only while the
    /// recorded source and root still prove ownership. Missing or replaced
    /// objects retain the original journal; an unreadable destination must
    /// never be mistaken for proof that the owned directory disappeared.
    #[test]
    fn overlong_archive_journal_requires_matching_source_and_root_after_reopen() {
        for state in ["matching", "missing", "foreign-source", "foreign-root"] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("root");
            fs::create_dir(&root).unwrap();
            let db = dir.path().join("registry.sqlite");
            let conn = open_registry(&db);
            create_registry_tables(&conn);
            let basename = format!(
                "bar-{}",
                "x".repeat(farhelm_proto::github_checkout::MAX_BASENAME_BYTES - 4)
            );
            let row = planned_row(&conn, &root, &basename);
            let allocated = allocate(&conn, &row.id, None).unwrap();
            let source = root.join(&basename);
            fs::write(source.join("owned"), b"preserve me").unwrap();
            let archive_root = ensure_archive_root(&root).unwrap();
            let uuid = uuid::Uuid::nil().simple().to_string();
            let journaled = format!("{basename}-20000101T000000Z-{uuid}-{uuid}");
            assert_eq!(journaled.len(), 283);
            assert_eq!(
                fs::symlink_metadata(archive_root.join(&journaled))
                    .unwrap_err()
                    .raw_os_error(),
                Some(libc::ENAMETOOLONG)
            );
            persist_journal(&conn, &row.id, &journaled, AllocationState::Allocated).unwrap();
            let preserved = dir.path().join("preserved");
            let mut foreign_identity = None;
            match state {
                "missing" | "foreign-source" => {
                    fs::rename(&source, &preserved).unwrap();
                    if state == "foreign-source" {
                        fs::create_dir(&source).unwrap();
                        foreign_identity = Some(identity_of_path(&source));
                    }
                }
                "foreign-root" => {
                    fs::rename(&root, &preserved).unwrap();
                    fs::create_dir(&root).unwrap();
                    foreign_identity = Some(identity_of_path(&root));
                }
                _ => {}
            }
            drop(conn);
            let conn = open_registry(&db);
            let result = reconcile_archive(&conn, &row.id);
            let pending = get_working_copy(&conn, &row.id).unwrap().unwrap();
            if state == "matching" {
                let ReconcileOutcome::Moved { destination } = result.unwrap() else {
                    panic!("the verified source must move to a legal name");
                };
                assert_eq!(destination.len(), 250);
                assert_eq!(
                    identity_of_path(&archive_root.join(&destination)),
                    allocated.identity
                );
                assert_eq!(
                    fs::read(archive_root.join(&destination).join("owned")).unwrap(),
                    b"preserve me"
                );
                assert_eq!(
                    pending.archive_destination.as_deref(),
                    Some(destination.as_str())
                );
                assert!(!source.exists());
            } else {
                assert!(
                    result.is_err(),
                    "{state} cannot establish recovery authority"
                );
                assert_eq!(
                    pending.archive_destination.as_deref(),
                    Some(journaled.as_str())
                );
                assert_eq!(pending.allocation_state, AllocationState::ArchivePending);
                let owned = if state == "foreign-root" {
                    preserved.join(&basename)
                } else {
                    preserved.clone()
                };
                assert_eq!(identity_of_path(&owned), allocated.identity);
                assert_eq!(fs::read(owned.join("owned")).unwrap(), b"preserve me");
                if let Some(identity) = foreign_identity {
                    let foreign = if state == "foreign-root" {
                        &root
                    } else {
                        &source
                    };
                    assert_eq!(identity_of_path(foreign), identity);
                    assert!(entries(foreign).is_empty());
                }
            }
        }
    }

    /// A vanished source is a visible outcome, not an error, and changes
    /// no state by itself.
    #[test]
    fn archive_move_reports_a_missing_source_as_an_outcome() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        let fresh = get_working_copy(&conn, &row.id)
            .expect("row")
            .expect("present");
        let path = fresh.canonical_path.clone().expect("path");
        fs::remove_dir(&path).expect("remove source");

        match archive_move(&conn, &row.id).expect("outcome") {
            ArchiveOutcome::SourceMissing => {}
            other => panic!("expected SourceMissing, got {other:?}"),
        }
        assert_eq!(
            entries(dir.path()),
            Vec::<String>::new(),
            "a vanished source is reported before any directory is created"
        );
    }

    /// A different object at the recorded path fails closed and is left
    /// untouched: the registry never moves a stranger.
    #[test]
    fn archive_move_fails_closed_on_a_foreign_object_at_the_source() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        let fresh = get_working_copy(&conn, &row.id)
            .expect("row")
            .expect("present");
        let path = fresh.canonical_path.clone().expect("path");
        // Swap the directory for a different one that COEXISTED with the
        // original — a remove-then-recreate could legitimately land on a
        // reused inode, which would be a false match, not a swap.
        let swap = dir.path().join("swap-source");
        fs::create_dir(&swap).expect("coexisting swap dir");
        fs::rename(&swap, &path).expect("swap");

        assert!(matches!(
            archive_move(&conn, &row.id),
            Err(WorkingCopyError::IdentityMismatch { .. })
        ));
        assert_eq!(
            entries(dir.path()),
            vec!["bar"],
            "the foreign object must be untouched and nothing archived"
        );
    }

    /// Spec: archiving refuses, before writing its journal, when an active
    /// checkout occupies the root's archive directory, both when archiving
    /// another checkout (which would move it inside that one) and when
    /// archiving the occupying checkout itself (which can never succeed).
    /// Startup reconciliation refuses the same way for a row that a registry
    /// written before this guard already left `archive_pending`.
    ///
    /// Admission now reserves the archive name, but a database from before
    /// that reservation can hold such a checkout. Without this guard the
    /// other checkout silently landed inside an unrelated repository, and
    /// the occupying one stayed `archive_pending` failing `EINVAL` on every
    /// delete retry and startup.
    #[test]
    fn archive_refuses_an_active_checkout_at_the_archive_directory() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let squatter = planned_row(&conn, dir.path(), ARCHIVE_DIR_NAME);
        allocate(&conn, &squatter.id, None).expect("allocate the squatting checkout");
        let other = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &other.id, None).expect("allocate another checkout");

        assert!(matches!(
            archive_move(&conn, &other.id),
            Err(WorkingCopyError::ArchiveRootIsCheckout { ref other_id, .. }) if *other_id == squatter.id
        ));
        assert!(matches!(
            archive_move(&conn, &squatter.id),
            Err(WorkingCopyError::SourceIsArchiveRoot { .. })
        ));
        for id in [&other.id, &squatter.id] {
            assert_eq!(
                get_working_copy(&conn, id)
                    .expect("row")
                    .expect("present")
                    .allocation_state,
                AllocationState::Allocated,
                "a refused archive must not leave the row archive_pending"
            );
        }
        let mut names = entries(dir.path());
        names.sort();
        assert_eq!(names, vec!["bar".to_string(), ARCHIVE_DIR_NAME.to_string()]);
        assert!(
            entries(&dir.path().join(ARCHIVE_DIR_NAME)).is_empty(),
            "nothing may be moved into the squatting checkout"
        );

        persist_journal(&conn, &squatter.id, "stem", AllocationState::Allocated)
            .expect("journal as an older database would have");
        assert!(matches!(
            reconcile_archive(&conn, &squatter.id),
            Err(WorkingCopyError::SourceIsArchiveRoot { .. })
        ));
    }

    /// Spec: recovery still completes an archive whose rename into a
    /// squatting checkout already happened, and leaves the squatter alone.
    ///
    /// The destination refusal exists to stop a MOVE into another
    /// checkout. When an older version already made that move and crashed
    /// before retiring the journal, nothing is left to move; refusing would
    /// leave a finished archive `archive_pending` on every delete retry and
    /// startup until someone dealt with an unrelated checkout by hand.
    #[test]
    fn recovery_completes_a_finished_move_into_a_squatting_checkout() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let squatter = planned_row(&conn, dir.path(), ARCHIVE_DIR_NAME);
        allocate(&conn, &squatter.id, None).expect("allocate the squatting checkout");
        let other = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &other.id, None).expect("allocate another checkout");
        let other = get_working_copy(&conn, &other.id)
            .expect("row")
            .expect("present");
        // What an older version left behind: the journal written and the
        // rename done, the row not yet retired.
        persist_journal(&conn, &other.id, "bar-archived", AllocationState::Allocated)
            .expect("journal");
        fs::rename(
            other.canonical_path.as_deref().expect("path"),
            dir.path().join(ARCHIVE_DIR_NAME).join("bar-archived"),
        )
        .expect("the older version's completed move");

        assert!(matches!(
            reconcile_archive(&conn, &other.id),
            Ok(ReconcileOutcome::MetadataComplete)
        ));
        assert_eq!(
            entries(&dir.path().join(ARCHIVE_DIR_NAME)),
            vec!["bar-archived".to_string()],
            "the squatting checkout keeps its content and nothing else moves"
        );
    }

    /// Spec: a directory with the recorded `(dev, ino)` but a different
    /// birth time is a different object: `verify_identity` reports
    /// `DifferentObject`, archiving refuses and moves nothing, and a root
    /// with a different birth time fails `verified_root`. A row with no
    /// recorded birth time keeps the `(dev, ino)` comparison.
    ///
    /// Why: filesystems reuse inode numbers (on ext4 a removed and recreated
    /// directory commonly gets the same one), so a user who removed a
    /// Farhelm checkout and re-cloned at the same path could have that new
    /// folder archived by the old session's Delete. The mismatch is planted
    /// in the recorded value because a test cannot make the filesystem
    /// reuse an inode on demand; the real reuse is exercised separately
    /// where the filesystem cooperates.
    #[test]
    fn a_different_birth_time_is_a_different_object() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        let fresh = get_working_copy(&conn, &row.id)
            .expect("row")
            .expect("present");
        let Some(recorded) = fresh.path_birth_ns else {
            assert!(
                !filesystem_reports_birth_time(dir.path()),
                "the filesystem reports birth times, but none was recorded"
            );
            println!("SKIPPED: this filesystem reports no birth time; the legacy rule applies");
            return;
        };
        assert_eq!(
            verify_identity(&fresh).expect("verify"),
            IdentityStatus::Matches
        );

        conn.execute(
            "UPDATE working_copies SET path_birth_ns = ?2 WHERE id = ?1",
            rusqlite::params![row.id, recorded - 1],
        )
        .expect("plant a different birth time");
        let planted = get_working_copy(&conn, &row.id)
            .expect("row")
            .expect("present");
        assert_eq!(
            verify_identity(&planted).expect("verify"),
            IdentityStatus::DifferentObject
        );
        assert!(matches!(
            archive_move(&conn, &row.id),
            Err(WorkingCopyError::IdentityMismatch { .. })
        ));
        assert_eq!(
            entries(dir.path()),
            vec!["bar".to_string()],
            "nothing may move"
        );

        conn.execute(
            "UPDATE working_copies SET path_birth_ns = NULL, root_birth_ns = root_birth_ns - 1 \
             WHERE id = ?1",
            rusqlite::params![row.id],
        )
        .expect("a legacy path identity and a planted root birth time");
        let legacy = get_working_copy(&conn, &row.id)
            .expect("row")
            .expect("present");
        assert_eq!(
            verify_identity(&legacy).expect("verify"),
            IdentityStatus::Matches,
            "a row without a recorded birth time keeps the (dev, ino) rule"
        );
        assert!(matches!(
            verified_root(&legacy),
            Err(WorkingCopyError::IdentityMismatch { .. })
        ));
    }

    /// Spec: a checkout and root whose recorded device number no longer
    /// matches, while inode and birth time still do, are the same folders:
    /// `verify_identity` matches, `verified_root` passes, and archiving moves
    /// the checkout. Without a recorded birth time the same device change is
    /// refused as `DeviceChangedUnconfirmed`, never as a match, and a birth
    /// time that differs as well is still a different object.
    ///
    /// Why: btrfs subvolumes, NFS, overlayfs and some device-mapper setups
    /// assign device numbers at mount time, so a reboot or remount can change
    /// them under an untouched folder, and requiring the old number made the
    /// session that created such a checkout unrestartable for good. The
    /// device change is planted in the recorded values because a test cannot
    /// remount the filesystem it runs on.
    #[test]
    fn a_changed_device_number_with_matching_birth_time_is_the_same_folder() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        let fresh = get_working_copy(&conn, &row.id)
            .expect("row")
            .expect("present");
        if fresh.path_birth_ns.is_none() {
            assert!(
                !filesystem_reports_birth_time(dir.path()),
                "the filesystem reports birth times, but none was recorded"
            );
            println!("SKIPPED: this filesystem reports no birth time to confirm a device change");
            return;
        }
        let reload = || {
            get_working_copy(&conn, &row.id)
                .expect("row")
                .expect("present")
        };

        conn.execute(
            "UPDATE working_copies SET path_device = path_device + 1, \
             root_device = root_device + 1 WHERE id = ?1",
            rusqlite::params![row.id],
        )
        .expect("plant a remount's new device number");
        let remounted = reload();
        assert_eq!(
            verify_identity(&remounted).expect("verify"),
            IdentityStatus::Matches
        );
        assert!(verified_root(&remounted).is_ok());

        conn.execute(
            "UPDATE working_copies SET path_birth_ns = path_birth_ns - 1 WHERE id = ?1",
            rusqlite::params![row.id],
        )
        .expect("plant a different birth time as well");
        assert_eq!(
            verify_identity(&reload()).expect("verify"),
            IdentityStatus::DifferentObject,
            "a replaced folder is still refused after a device change"
        );

        conn.execute(
            "UPDATE working_copies SET path_birth_ns = NULL, root_birth_ns = NULL WHERE id = ?1",
            rusqlite::params![row.id],
        )
        .expect("a row recorded without birth times");
        let unconfirmed = reload();
        assert_eq!(
            verify_identity(&unconfirmed).expect("verify"),
            IdentityStatus::DeviceChangedUnconfirmed
        );
        assert!(matches!(
            verified_root(&unconfirmed),
            Err(WorkingCopyError::DeviceChangedUnconfirmed { .. })
        ));

        conn.execute(
            "UPDATE working_copies SET path_birth_ns = ?2, root_birth_ns = ?3 WHERE id = ?1",
            rusqlite::params![row.id, fresh.path_birth_ns, fresh.root_birth_ns],
        )
        .expect("restore the recorded birth times");
        assert!(matches!(
            archive_move(&conn, &row.id).expect("archive after the device change"),
            ArchiveOutcome::Archived { .. }
        ));
        assert!(
            !dir.path().join("bar").exists(),
            "the checkout must have moved to the archive"
        );
    }

    /// Spec: removing an allocated checkout and recreating a directory at the
    /// same path yields `DifferentObject`, even when the filesystem hands the
    /// new directory the old inode number.
    ///
    /// Why: this is the user-visible shape of the bug (re-cloning over a
    /// checkout Farhelm made). It runs on the real filesystem, so it only
    /// proves something where birth times exist; it says which premise held.
    #[test]
    fn a_recreated_checkout_directory_is_a_different_object() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        let fresh = get_working_copy(&conn, &row.id)
            .expect("row")
            .expect("present");
        if fresh.path_birth_ns.is_none() {
            assert!(
                !filesystem_reports_birth_time(dir.path()),
                "the filesystem reports birth times, but none was recorded"
            );
            println!("SKIPPED: this filesystem reports no birth time; the legacy rule applies");
            return;
        }
        let path = fresh.canonical_path.clone().expect("path");
        let before = identity_of_path(Path::new(&path));
        fs::remove_dir(&path).expect("remove the checkout");
        fs::create_dir(&path).expect("recreate a directory at the same path");
        let reused = identity_of_path(Path::new(&path)) == before;
        println!("inode reused by the filesystem: {reused}");
        assert_eq!(
            verify_identity(&fresh).expect("verify"),
            IdentityStatus::DifferentObject
        );
    }

    /// Spec: a planned row records its root's birth time with the plan, and
    /// allocation refuses, creating nothing, when the root at that path has
    /// the recorded `(dev, ino)` but a different birth time.
    ///
    /// Why: a create interrupted after its plan committed but before mkdir
    /// is retried later; if the root was removed and recreated meanwhile
    /// with a reused inode, a `(dev, ino)` check alone would let the retry
    /// allocate and prepare a checkout inside the replacement root. The
    /// mismatch is planted in the recorded value because a test cannot make
    /// the filesystem reuse an inode on demand.
    #[test]
    fn allocation_refuses_a_root_with_a_different_birth_time() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let root_identity = identity_of_path(dir.path());
        let mut spec = planned_spec(dir.path(), "bar");
        spec.root_identity = Some(root_identity);
        let planned = record_planned(&conn, &spec).expect("planned row");
        let Some(recorded) = planned.root_birth_ns else {
            assert!(
                !filesystem_reports_birth_time(dir.path()),
                "the filesystem reports birth times, but planning recorded none"
            );
            println!("SKIPPED: this filesystem reports no birth time; the legacy rule applies");
            return;
        };
        conn.execute(
            "UPDATE working_copies SET root_birth_ns = ?2 WHERE id = ?1",
            rusqlite::params![planned.id, recorded - 1],
        )
        .expect("plant a different root birth time");

        assert!(matches!(
            allocate(&conn, &planned.id, Some(root_identity)),
            Err(AllocationFailure::PreMkdir(
                WorkingCopyError::IdentityMismatch { .. }
            ))
        ));
        assert!(entries(dir.path()).is_empty(), "nothing may be created");
    }

    /// A symlinked archive root is refused before any mutation.
    #[test]
    fn archive_move_rejects_a_symlinked_archive_root() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let elsewhere = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        std::os::unix::fs::symlink(elsewhere.path(), dir.path().join(ARCHIVE_DIR_NAME))
            .expect("symlink the archive root");

        assert!(matches!(
            archive_move(&conn, &row.id),
            Err(WorkingCopyError::ArchiveRootSymlink { .. })
        ));
        assert!(
            fs::symlink_metadata(dir.path().join("bar")).is_ok(),
            "the source must be untouched"
        );
    }

    /// Cross-device archive roots are refused. A genuinely different
    /// filesystem is required; when none is cheaply available the test is
    /// skipped rather than faked. PREMISE NOTE: this test only runs when
    /// /dev/shm is a tmpfs distinct from the tempdir's filesystem, which
    /// is true on ordinary Linux CI; when the premise fails the SKIP is
    /// printed and the case is left to environments where it holds.
    #[test]
    fn archive_move_rejects_a_foreign_device_archive_root_when_two_filesystems_exist() {
        let host = match tempfile::tempdir_in("/dev/shm") {
            Ok(host) => host,
            Err(e) => {
                println!("SKIPPED: no /dev/shm available ({e}); cross-device shape untested here");
                return;
            }
        };
        let dir = tempfile::tempdir().expect("tempdir");
        let shm_meta = fs::symlink_metadata("/dev/shm").expect("stat /dev/shm");
        let tmp_meta = fs::symlink_metadata(dir.path()).expect("stat tempdir");
        if (shm_meta.dev(), shm_meta.ino()) == (tmp_meta.dev(), tmp_meta.ino())
            || shm_meta.dev() == tmp_meta.dev()
        {
            println!(
                "SKIPPED: /dev/shm and the tempdir share one filesystem; cross-device shape untested here"
            );
            return;
        }
        let conn = registry_conn();
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        // Establish matching root identity on the other filesystem while
        // leaving the source here. This deliberately inconsistent record
        // reaches the device guard rather than failing the root guard first.
        let root_identity = identity_of_path(host.path());
        let root_birth = observe(host.path()).expect("stat host root").birth_ns;
        conn.execute(
            "UPDATE working_copies SET canonical_root = ?2, root_device = ?3, root_inode = ?4, root_birth_ns = ?5 WHERE id = ?1",
            rusqlite::params![row.id, host.path().to_str().expect("utf8"), root_identity.0 as i64, root_identity.1 as i64, root_birth],
        )
        .expect("repoint root");
        assert!(matches!(
            archive_move(&conn, &row.id),
            Err(WorkingCopyError::ArchiveRootForeignDevice { .. })
        ));
    }

    // -- crash recovery -------------------------------------------------------

    /// A missing pathname under a replacement root is not proof that the
    /// owned checkout vanished. Both archive admission and the final metadata
    /// transaction must retain that evidence, including for symlink roots.
    #[test]
    fn missing_source_retirement_requires_the_recorded_root() {
        for symlink_root in [false, true] {
            let fixture = tempfile::tempdir().unwrap();
            let root = fixture.path().join("root");
            let parked = fixture.path().join("parked");
            let foreign = fixture.path().join("foreign");
            fs::create_dir(&root).unwrap();
            let conn = registry_conn();
            let plan = planned_row(&conn, &root, "bar");
            let accepted = allocate(&conn, &plan.id, None).unwrap();
            fs::write(root.join("bar/payload"), b"retained").unwrap();
            fs::rename(&root, &parked).unwrap();
            if symlink_root {
                fs::create_dir(&foreign).unwrap();
                std::os::unix::fs::symlink(&foreign, &root).unwrap();
            } else {
                fs::create_dir(&root).unwrap();
            }
            assert!(!root.join("bar").exists());
            assert_eq!(identity_of_path(&parked.join("bar")), accepted.identity);
            assert_ne!(identity_of_path(&root), accepted.row.root_identity.unwrap());
            assert!(matches!(
                archive_move(&conn, &plan.id),
                Err(WorkingCopyError::IdentityMismatch { .. })
            ));
            assert!(matches!(
                retire_missing(&conn, &plan.id),
                Err(WorkingCopyError::IdentityMismatch { .. })
            ));
            let retained = get_working_copy(&conn, &plan.id).unwrap().unwrap();
            assert_eq!(retained.allocation_state, AllocationState::Allocated);
            assert_eq!(retained.path_identity, Some(accepted.identity));
            assert_eq!(retained.archive_destination, None);
            assert_eq!(member_count(&conn, &plan.id).unwrap(), 1);
            assert_eq!(fs::read(parked.join("bar/payload")).unwrap(), b"retained");
            assert!(!root.join(ARCHIVE_DIR_NAME).exists());
            assert!(!parked.join(ARCHIVE_DIR_NAME).exists());

            // Restore the matching root, then make only the checkout absent.
            // This positive control distinguishes root verification from a
            // blanket refusal to retire missing sources.
            if symlink_root {
                fs::remove_file(&root).unwrap();
            } else {
                fs::remove_dir(&root).unwrap();
            }
            fs::rename(&parked, &root).unwrap();
            fs::remove_file(root.join("bar/payload")).unwrap();
            fs::remove_dir(root.join("bar")).unwrap();
            assert!(matches!(
                archive_move(&conn, &plan.id).unwrap(),
                ArchiveOutcome::SourceMissing
            ));
            retire_missing(&conn, &plan.id).unwrap();
            assert!(get_working_copy(&conn, &plan.id).unwrap().is_none());
            assert!(!root.join(ARCHIVE_DIR_NAME).exists());
        }
    }

    /// A matching checkout inode cannot authorize mutation of a replacement
    /// parent. Initial archival and crash recovery both refuse before creating
    /// an archive child, preserve contents, and leave their journal unchanged.
    #[test]
    fn archive_paths_refuse_replaced_roots_before_any_mutation() {
        for pending in [false, true] {
            for symlink_root in [false, true] {
                let fixture = tempfile::tempdir().expect("fixture");
                let root = fixture.path().join("root");
                let parked = fixture.path().join("parked");
                fs::create_dir(&root).expect("original root");
                let conn = registry_conn();
                let row = planned_row(&conn, &root, "bar");
                let accepted = allocate(&conn, &row.id, None).expect("allocate");
                fs::write(root.join("bar/payload"), b"owned content").expect("payload");
                if pending {
                    persist_journal(&conn, &row.id, "bar-journaled", AllocationState::Allocated)
                        .expect("journal before simulated interruption");
                }
                let before = get_working_copy(&conn, &row.id)
                    .expect("row")
                    .expect("present");
                fs::rename(&root, &parked).expect("park original root");
                if symlink_root {
                    std::os::unix::fs::symlink(&parked, &root).expect("symlink root");
                } else {
                    fs::create_dir(&root).expect("foreign replacement root");
                    fs::rename(parked.join("bar"), root.join("bar"))
                        .expect("move the same checkout into the foreign parent");
                    fs::write(root.join("foreign"), b"foreign content").expect("foreign sentinel");
                }
                assert_eq!(
                    identity_of_path(&root.join("bar")),
                    accepted.identity,
                    "the checkout still matches; only the parent boundary changed"
                );
                assert_ne!(
                    identity_of_path(&root),
                    before.root_identity.expect("recorded root")
                );
                assert!(
                    !root.join(ARCHIVE_DIR_NAME).exists(),
                    "no archive child before operation"
                );

                let error = if pending {
                    reconcile_archive(&conn, &row.id).expect_err("recovery must reject root")
                } else {
                    archive_move(&conn, &row.id).expect_err("archival must reject root")
                };
                assert!(matches!(error, WorkingCopyError::IdentityMismatch { .. }));
                assert!(
                    !root.join(ARCHIVE_DIR_NAME).exists(),
                    "refusal must not create an archive child"
                );
                assert_eq!(
                    fs::read(root.join("bar/payload")).expect("payload remains"),
                    b"owned content"
                );
                if !symlink_root {
                    assert_eq!(
                        fs::read(root.join("foreign")).expect("foreign remains"),
                        b"foreign content"
                    );
                }
                let after = get_working_copy(&conn, &row.id)
                    .expect("row")
                    .expect("present");
                assert_eq!(after.allocation_state, before.allocation_state);
                assert_eq!(after.archive_destination, before.archive_destination);
            }
        }
    }

    /// Corrupt or incomplete allocated records cannot treat missing root
    /// identity as permission to create an archive directory or move content.
    #[test]
    fn archive_paths_require_recorded_root_identity() {
        for pending in [false, true] {
            let dir = tempfile::tempdir().expect("root");
            let conn = registry_conn();
            let row = planned_row(&conn, dir.path(), "bar");
            allocate(&conn, &row.id, None).expect("allocate");
            if pending {
                persist_journal(&conn, &row.id, "bar-journaled", AllocationState::Allocated)
                    .expect("journal");
            }
            conn.execute(
                "UPDATE working_copies SET root_device = NULL, root_inode = NULL WHERE id = ?1",
                [&row.id],
            )
            .expect("remove identity evidence");
            assert!(
                get_working_copy(&conn, &row.id)
                    .expect("row")
                    .expect("present")
                    .root_identity
                    .is_none()
            );
            let error = if pending {
                reconcile_archive(&conn, &row.id).expect_err("missing evidence")
            } else {
                archive_move(&conn, &row.id).expect_err("missing evidence")
            };
            assert!(matches!(error, WorkingCopyError::WrongState(_)));
            assert_eq!(
                entries(dir.path()),
                ["bar"],
                "no filesystem mutation on refusal"
            );
        }
    }

    /// Crash after the journal, before the rename: source present,
    /// destination absent → the commit may be retried and completes
    /// against the JOURNALED destination.
    #[test]
    fn reconcile_retries_the_commit_after_a_journalled_crash() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        // Simulate the crash window: journal persist, no rename.
        let destination = format!("bar-{}", utc_compact(now_unix()));
        persist_journal(&conn, &row.id, &destination, AllocationState::Allocated).expect("journal");

        match reconcile_archive(&conn, &row.id).expect("reconcile") {
            ReconcileOutcome::Moved { destination: dest } => assert_eq!(dest, destination),
            other => panic!("expected Moved, got {other:?}"),
        }
        let fresh = get_working_copy(&conn, &row.id)
            .expect("row")
            .expect("present");
        assert_eq!(
            fresh.archive_destination.as_deref(),
            Some(destination.as_str())
        );
        assert_eq!(
            entries(&dir.path().join(ARCHIVE_DIR_NAME)),
            vec![destination]
        );
    }

    /// A collision after recovery's destination lookup must report the final
    /// name chosen by the inner retry, in both the initially-free and already-
    /// occupied branches. Merely pre-creating the journaled destination would
    /// not detect a caller discarding the rename helper's returned name.
    #[test]
    fn reconcile_reports_the_actual_destination_after_a_rename_boundary_collision() {
        for initially_occupied in [false, true] {
            let conn = registry_conn();
            let dir = tempfile::tempdir().unwrap();
            let row = planned_row(&conn, dir.path(), "bar");
            let allocated = allocate(&conn, &row.id, None).unwrap();
            let source = dir.path().join("bar");
            fs::write(source.join("owned"), b"keep checkout content").unwrap();
            let archive_root = ensure_archive_root(dir.path()).unwrap();
            let journaled = "bar-journaled";
            persist_journal(&conn, &row.id, journaled, AllocationState::Allocated).unwrap();
            if initially_occupied {
                fs::create_dir(archive_root.join(journaled)).unwrap();
                fs::write(
                    archive_root.join(journaled).join("original-competitor"),
                    b"untouched",
                )
                .unwrap();
            }
            let mut attempts = Vec::new();
            let outcome = reconcile_archive_with_effects(&conn, &row.id, None, &mut |from, to| {
                assert_eq!(from, source);
                let pending = get_working_copy(&conn, &row.id).unwrap().unwrap();
                assert_eq!(pending.allocation_state, AllocationState::ArchivePending);
                assert_eq!(
                    pending.archive_destination.as_deref(),
                    to.file_name().unwrap().to_str()
                );
                assert_eq!(identity_of(from).unwrap(), allocated.identity);
                assert!(
                    !to.exists(),
                    "fixture inserts its competitor at the rename boundary"
                );
                if attempts.is_empty() {
                    fs::create_dir(to)?;
                    fs::write(to.join("racing-competitor"), b"also untouched")?;
                }
                attempts.push(to.to_path_buf());
                rename_noreplace(from, to)
            })
            .unwrap();
            let ReconcileOutcome::Moved { destination } = outcome else {
                panic!("the source must move after the collision");
            };
            assert_eq!(
                attempts.len(),
                2,
                "one failed real rename followed by one successful rename"
            );
            assert_ne!(
                destination,
                attempts[0].file_name().unwrap().to_str().unwrap()
            );
            assert_eq!(archive_root.join(&destination), attempts[1]);
            let pending = get_working_copy(&conn, &row.id).unwrap().unwrap();
            assert_eq!(
                pending.archive_destination.as_deref(),
                Some(destination.as_str())
            );
            assert_eq!(identity_of(&attempts[1]).unwrap(), allocated.identity);
            assert_eq!(
                fs::read(attempts[1].join("owned")).unwrap(),
                b"keep checkout content"
            );
            assert_eq!(
                fs::read(attempts[0].join("racing-competitor")).unwrap(),
                b"also untouched"
            );
            if initially_occupied {
                assert_eq!(
                    fs::read(archive_root.join(journaled).join("original-competitor")).unwrap(),
                    b"untouched"
                );
            }
            assert_eq!(
                fs::read_dir(&archive_root).unwrap().count(),
                if initially_occupied { 3 } else { 2 }
            );
            assert!(!source.exists());
        }
    }

    /// A rename the filesystem refuses outright, during crash recovery or a
    /// fresh archive, rolls the journal back to `allocated` and leaves the
    /// checkout where it was; a later archive can still move it.
    ///
    /// Why it matters: a filesystem without no-overwrite rename (NFS, CIFS,
    /// some FUSE mounts) refuses every attempt, and a row left pending made
    /// every recovery retry the same rename while Restart and new sessions
    /// in that folder stayed refused. These errors report that nothing moved,
    /// so the pending journal describes a move that cannot have happened.
    /// Specified, for an unsupported flag (`EINVAL`) and a permission refusal
    /// (`EACCES`), injected at the syscall boundary so the test holds even
    /// with permission overrides: the error is `RenameRefused`, the row is
    /// `allocated` with no destination, the source keeps its identity, the
    /// archive is empty, and a later ordinary archive succeeds.
    #[test]
    fn a_refused_rename_rolls_the_journal_back_and_leaves_the_checkout() {
        for errno in [libc::EINVAL, libc::EACCES] {
            for recovering in [true, false] {
                let conn = registry_conn();
                let dir = tempfile::tempdir().unwrap();
                let row = planned_row(&conn, dir.path(), "bar");
                let allocated = allocate(&conn, &row.id, None).unwrap();
                let archive_root = ensure_archive_root(dir.path()).unwrap();
                let mut attempts = 0;
                let mut refuse = |from: &Path, _to: &Path| {
                    assert_eq!(identity_of(from).unwrap(), allocated.identity);
                    attempts += 1;
                    Err(std::io::Error::from_raw_os_error(errno))
                };
                let error = if recovering {
                    persist_journal(&conn, &row.id, "bar-journaled", AllocationState::Allocated)
                        .unwrap();
                    reconcile_archive_with_effects(&conn, &row.id, None, &mut refuse)
                        .map(|_| ())
                        .unwrap_err()
                } else {
                    archive_move_with_effects(&conn, &row.id, None, &mut refuse)
                        .map(|_| ())
                        .unwrap_err()
                };
                let case = format!("errno {errno}, recovering {recovering}");
                assert!(
                    matches!(&error, WorkingCopyError::RenameRefused { cause, .. } if cause.raw_os_error() == Some(errno)),
                    "{case}: {error:?}"
                );
                assert_eq!(attempts, 1, "{case}: only collisions retry a name");
                let current = get_working_copy(&conn, &row.id).unwrap().unwrap();
                assert_eq!(
                    current.allocation_state,
                    AllocationState::Allocated,
                    "{case}"
                );
                assert_eq!(current.archive_destination, None, "{case}");
                assert_eq!(
                    identity_of(&dir.path().join("bar")).unwrap(),
                    allocated.identity,
                    "{case}"
                );
                assert_eq!(fs::read_dir(&archive_root).unwrap().count(), 0, "{case}");
                assert!(
                    matches!(
                        archive_move(&conn, &row.id).unwrap(),
                        ArchiveOutcome::Archived { .. }
                    ),
                    "{case}: a later archive still moves the checkout"
                );
            }
        }
    }

    /// A rename error that does not prove nothing moved keeps the pending
    /// journal, so recovery can still find a move that did happen.
    ///
    /// Why it matters: on a network filesystem the server can complete a
    /// rename whose reply is lost, surfacing as an I/O error. Rolling that
    /// back would forget the destination the checkout may now occupy.
    /// Specified: an injected `EIO` leaves the row `archive_pending` with its
    /// journaled destination.
    #[test]
    fn an_ambiguous_rename_error_keeps_the_pending_journal() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().unwrap();
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).unwrap();
        ensure_archive_root(dir.path()).unwrap();
        persist_journal(&conn, &row.id, "bar-journaled", AllocationState::Allocated).unwrap();
        let error = reconcile_archive_with_effects(&conn, &row.id, None, &mut |_, _| {
            Err(std::io::Error::from_raw_os_error(libc::EIO))
        })
        .map(|_| ())
        .unwrap_err();
        assert!(matches!(error, WorkingCopyError::Io(_)), "{error:?}");
        let pending = get_working_copy(&conn, &row.id).unwrap().unwrap();
        assert_eq!(pending.allocation_state, AllocationState::ArchivePending);
        assert_eq!(
            pending.archive_destination.as_deref(),
            Some("bar-journaled")
        );
    }

    /// Crash after the rename, before the metadata step: source absent,
    /// destination present with the row's identity → re-fsync, report
    /// MetadataComplete, and the teardown slice may then retire.
    #[test]
    fn reconcile_completes_metadata_after_a_renamed_crash_then_retire() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        let destination = format!("bar-{}", utc_compact(now_unix()));
        persist_journal(&conn, &row.id, &destination, AllocationState::Allocated).expect("journal");
        // Simulate the crash AFTER the rename: do the rename by hand.
        fs::create_dir_all(dir.path().join(ARCHIVE_DIR_NAME)).expect("archive root");
        fs::rename(
            dir.path().join("bar"),
            dir.path().join(ARCHIVE_DIR_NAME).join(&destination),
        )
        .expect("rename");

        match reconcile_archive(&conn, &row.id).expect("reconcile") {
            ReconcileOutcome::MetadataComplete => {}
            other => panic!("expected MetadataComplete, got {other:?}"),
        }
        retire(&conn, &row.id).expect("retire");
        let fresh = get_working_copy(&conn, &row.id)
            .expect("row")
            .expect("present");
        assert_eq!(fresh.allocation_state, AllocationState::Retired);
    }

    /// An unrelated object at the journaled destination moves the archive
    /// aside under a fresh suffix rather than overwriting the stranger.
    #[test]
    fn reconcile_moves_aside_for_an_unrelated_object_at_the_journalled_destination() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        let destination = format!("bar-{}", utc_compact(now_unix()));
        persist_journal(&conn, &row.id, &destination, AllocationState::Allocated).expect("journal");
        // A stranger occupies the journaled destination.
        fs::create_dir_all(dir.path().join(ARCHIVE_DIR_NAME).join(&destination))
            .expect("stranger at destination");

        match reconcile_archive(&conn, &row.id).expect("reconcile") {
            ReconcileOutcome::Moved { destination: dest } => {
                assert!(
                    dest.starts_with("bar-"),
                    "{dest} must retain the original basename"
                );
                assert_eq!(dest.len(), "bar-".len() + 16 + 1 + 32);
                assert_ne!(dest, destination);
            }
            other => panic!("expected Moved, got {other:?}"),
        }
        // Both the stranger and our archived copy exist; the stranger is
        // untouched.
        assert_eq!(
            entries(&dir.path().join(ARCHIVE_DIR_NAME)).len(),
            2,
            "the stranger and the suffixed archive both survive"
        );
    }

    /// A source-identity mismatch during reconciliation fails closed.
    #[test]
    fn reconcile_fails_closed_on_an_identity_mismatch() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        let destination = format!("bar-{}", utc_compact(now_unix()));
        persist_journal(&conn, &row.id, &destination, AllocationState::Allocated).expect("journal");
        // Source swapped for a different object; destination absent. The
        // swap dir is created BEFORE the original is removed so the
        // replacement cannot land on a reused inode and falsely match.
        let swap = dir.path().join("swap-source");
        fs::create_dir(&swap).expect("coexisting swap dir");
        fs::remove_dir(dir.path().join("bar")).expect("remove");
        fs::rename(&swap, dir.path().join("bar")).expect("swap stranger");

        assert!(matches!(
            reconcile_archive(&conn, &row.id),
            Err(WorkingCopyError::IdentityMismatch { .. })
        ));
    }

    /// Destination present but a different object while the source is
    /// gone: reconcile cannot claim the stranger — fail closed.
    #[test]
    fn reconcile_fails_closed_when_a_stranger_holds_the_destination_and_the_source_is_gone() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        let destination = format!("bar-{}", utc_compact(now_unix()));
        persist_journal(&conn, &row.id, &destination, AllocationState::Allocated).expect("journal");
        fs::remove_dir(dir.path().join("bar")).expect("source gone");
        fs::create_dir_all(dir.path().join(ARCHIVE_DIR_NAME).join(&destination))
            .expect("stranger at destination");

        assert!(matches!(
            reconcile_archive(&conn, &row.id),
            Err(WorkingCopyError::IdentityMismatch { .. })
        ));
    }

    /// Source and destination both absent: a visible SourceMissing, and
    /// nothing is created anywhere as a side effect.
    #[test]
    fn reconcile_reports_source_missing_when_nothing_remains() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        let destination = format!("bar-{}", utc_compact(now_unix()));
        persist_journal(&conn, &row.id, &destination, AllocationState::Allocated).expect("journal");
        fs::remove_dir(dir.path().join("bar")).expect("source gone");

        match reconcile_archive(&conn, &row.id).expect("reconcile") {
            ReconcileOutcome::SourceMissing => {}
            other => panic!("expected SourceMissing, got {other:?}"),
        }
        assert_eq!(
            entries(&dir.path().join(ARCHIVE_DIR_NAME)),
            Vec::<String>::new(),
            "reconciliation invented no directory"
        );
    }

    /// Only a pending row may be reconciled; allocated and retired rows
    /// are refused so a startup sweep cannot double-process.
    #[test]
    fn reconcile_requires_the_archive_pending_state() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().expect("tempdir");
        let row = planned_row(&conn, dir.path(), "bar");
        allocate(&conn, &row.id, None).expect("allocate");
        assert!(matches!(
            reconcile_archive(&conn, &row.id),
            Err(WorkingCopyError::WrongState(_))
        ));
    }

    // -- time formatting -------------------------------------------------------

    /// The epoch itself formats as 19700101T000000Z, and the conversion
    /// agrees with a known later instant.
    #[test]
    fn utc_compact_formats_known_instants() {
        assert_eq!(utc_compact(0), "19700101T000000Z");
        // 2033-05-18T03:33:20Z = 2000000000
        assert_eq!(utc_compact(2_000_000_000), "20330518T033320Z");
        // 2009-02-13T23:31:30Z = 1234567890
        assert_eq!(utc_compact(1_234_567_890), "20090213T233130Z");
    }
}

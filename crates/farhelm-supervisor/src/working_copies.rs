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
use rusqlite::Connection;
use thiserror::Error;

/// Fixed name of the archive directory under a canonical root. Occupancy
/// scans skip it exactly, and the archive move requires it to be a real
/// same-device directory.
pub const ARCHIVE_DIR_NAME: &str = "farhelm-archived-working-copies";

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
    /// Nothing at the recorded path.
    Missing,
    /// The row has no captured identity yet (a `planned` row), so there
    /// is nothing to verify against.
    NoCapturedIdentity,
}

/// What [`lowest_free_suffix`] concluded about a canonical root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OccupancyOutcome {
    /// `{basename}` itself is unoccupied; the plain name may be used.
    BasenameAvailable,
    /// `{basename}` is taken; the lowest free numeric name is
    /// `{basename}-{n}`.
    LowestFree(u32),
}

/// What [`allocate`] did: won the exclusive mkdir and captured identity.
#[derive(Clone, Debug)]
pub struct AcceptedDirectory {
    pub row: WorkingCopyRow,
    pub identity: DirectoryIdentity,
    pub canonical_path: PathBuf,
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
    /// Overlapping active records are inconsistent ownership evidence, even
    /// when each inode matches. Moving either would invalidate the other.
    #[error(
        "working-copy path {path} overlaps active record {other_id}; refusing automated archival"
    )]
    OverlappingRecord { path: PathBuf, other_id: String },
    #[error("the archive directory {path} is a symlink; refusing to archive through it")]
    ArchiveRootSymlink { path: PathBuf },
    #[error("the archive directory {path} is not a directory")]
    ArchiveRootNotDirectory { path: PathBuf },
    #[error("the archive directory {path} is on a different filesystem than the canonical root")]
    ArchiveRootForeignDevice { path: PathBuf },
    #[error("the rename destination stayed occupied after {attempts} attempts")]
    DestinationExhausted { attempts: usize },
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
         path_device, path_inode, allocation_state, archive_destination, \
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
/// occupancy scan ([`lowest_free_suffix`]) chose; allocation does not
/// rename, it creates exactly this name.
pub fn record_planned(conn: &Connection, spec: &PlannedWorkingCopy) -> Result<WorkingCopyRow> {
    conn.execute(
        "INSERT INTO working_copies \
         (id, canonical_root, canonical_path, repo_owner, repo_name, \
          original_basename, origin_session_id, root_device, root_inode, \
          path_device, path_inode, allocation_state, archive_destination, \
          preparation_snapshot, created_at) \
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?9, ?10, NULL, NULL, ?7, \
                 NULL, ?11, ?8)",
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
    let mut stmt = conn.prepare(
        "SELECT id, canonical_root, canonical_path, repo_owner, repo_name, \
         original_basename, origin_session_id, root_device, root_inode, \
         path_device, path_inode, allocation_state, archive_destination, \
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
    let mut stmt = conn.prepare(
        "SELECT w.id, w.canonical_root, w.canonical_path, w.repo_owner, w.repo_name, \
         w.original_basename, w.origin_session_id, w.root_device, w.root_inode, \
         w.path_device, w.path_inode, w.allocation_state, w.archive_destination, \
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
         path_device, path_inode, allocation_state, archive_destination, \
         preparation_snapshot, created_at \
         FROM working_copies ORDER BY created_at, id",
    )?;
    let rows = stmt
        .query_map([], decode_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
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
fn refuse_overlapping_archive(conn: &Connection, row: &WorkingCopyRow) -> Result<()> {
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

/// Find the lowest free `{basename}-N` name under `root`.
///
/// Every directory entry except [`ARCHIVE_DIR_NAME`] occupies its exact
/// name — files, real directories, and symlinks alike, including DANGLING
/// symlinks (a name claimed on disk is claimed as far as mkdir is
/// concerned, whatever the entry points at). The preview path calls this
/// before suggesting a name; [`allocate`] remains the actual arbitration
/// because only the exclusive mkdir can race.
///
/// `cap` is injected so tests can exercise the refusal path; production
/// passes [`OCCUPANCY_SCAN_CAP`]. Hitting the cap yields
/// [`WorkingCopyError::IncompleteScan`] — a wrong "lowest free" would
/// collapse two creates onto one name, and the explicit refusal is
/// cheaper than the repair.
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
/// names with arbitrary suffixes. The reserved archive directory is skipped
/// exactly like the allocator skips it. This is the preview-side complement
/// of [`lowest_free_suffix`]: the naming helper's `occupied_names` callback
/// needs a SET because titled names check arbitrary candidates, while the
/// allocator only needs the lowest free number.
///
/// `cap` bounds iterator calls, including the call needed to prove EOF.
/// Exhausting it without EOF yields [`WorkingCopyError::IncompleteScan`]
/// so a preview never proposes a name from an incomplete view. Root-open
/// and entry-read failures propagate rather than implying a free name.
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
        let name = entry.file_name();
        if let Some(name) = name.to_str()
            && name != ARCHIVE_DIR_NAME
            && (name == basename || name.starts_with(&basename_prefix))
        {
            names.insert(name.to_owned());
        }
    }
    Err(WorkingCopyError::IncompleteScan { scanned: cap })
}

/// Propose the lowest free `basename-N` (or that `basename` itself is
/// free) by scanning `root` once: every entry occupying a related shape —
/// name — files, real directories, and symlinks alike, including DANGLING
/// symlinks (a name claimed on disk is claimed as far as mkdir is
/// concerned, whatever the entry points at). The preview path calls this
/// before suggesting a name; [`allocate`] remains the actual arbitration
/// because only the exclusive mkdir can race.
///
pub fn lowest_free_suffix(root: &Path, basename: &str, cap: usize) -> Result<OccupancyOutcome> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        // A missing canonical root has nothing in it — the plain basename
        // is trivially free. Any other read failure is real and surfaces.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(OccupancyOutcome::BasenameAvailable);
        }
        Err(e) => return Err(e.into()),
    };
    let mut plain_occupied = false;
    let mut occupied: Vec<u32> = Vec::new();
    let mut scanned = 0usize;
    for entry in entries {
        scanned += 1;
        if scanned > cap {
            return Err(WorkingCopyError::IncompleteScan { scanned });
        }
        let Ok(entry) = entry else {
            // The root's contents churned under us (or the entry was
            // unreadable). Naming stays best-effort — allocate still
            // arbitrates the truth with the exclusive mkdir.
            continue;
        };
        let Some(name) = entry.file_name().into_string().ok() else {
            continue;
        };
        if name == basename {
            plain_occupied = true;
        }
        if name == ARCHIVE_DIR_NAME {
            continue;
        }
        let Some(suffix) = name
            .strip_prefix(basename)
            .and_then(|s| s.strip_prefix('-'))
        else {
            continue;
        };
        // A leading '+' would make u32 parsing accept what the user never
        // typed as a suffix (`bar-+5`); anything else unparseable is just
        // an unrelated entry, not an occupancy.
        if !suffix.starts_with('+')
            && let Ok(n) = suffix.parse::<u32>()
        {
            occupied.push(n);
        }
    }
    // Any occupied related shape — the plain name itself OR a numeric
    // suffix — makes the answer the lowest free suffix. `bar-1` existing
    // with `bar` free still means the next create should take a numbered
    // name, not silently reuse the unnumbered one.
    if plain_occupied || !occupied.is_empty() {
        Ok(OccupancyOutcome::LowestFree(lowest_absent(&mut occupied)))
    } else {
        Ok(OccupancyOutcome::BasenameAvailable)
    }
}

/// The smallest POSITIVE integer absent from a list (which is sorted in
/// place). Numbered names start at 1 — `bar-0` is not a shape the naming
/// scheme ever suggests — so `[1, 3]` → 2 and `[]` → 1.
fn lowest_absent(occupied: &mut [u32]) -> u32 {
    occupied.sort_unstable();
    let mut expected = 1u32;
    for &n in occupied.iter() {
        if n != expected {
            return expected;
        }
        if n == u32::MAX {
            // Every positive number is taken; callers cannot use another
            // suffix. Returning MAX keeps this total.
            return u32::MAX;
        }
        expected += 1;
    }
    expected
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
        let actual = identity_of(&root).map_err(AllocationFailure::PreMkdir)?;
        if actual != expected {
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
    let identity = identity_of(&target).map_err(AllocationFailure::PostMkdir)?;
    // Canonicalize AFTER exclusive creation: the only path canonicalized
    // is a directory this call just created, so no concurrent rename of
    // parents can smuggle in a different object without the identity
    // capture below exposing it.
    let canonical_path = target
        .canonicalize()
        .map_err(WorkingCopyError::Io)
        .map_err(AllocationFailure::PostMkdir)?;
    let root_identity = identity_of(&root).map_err(AllocationFailure::PostMkdir)?;

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
             path_device = ?5, path_inode = ?6, canonical_path = ?7 \
         WHERE id = ?1 AND allocation_state = ?8",
            rusqlite::params![
                row.id,
                AllocationState::Allocated.as_str(),
                root_identity.0 as i64,
                root_identity.1 as i64,
                identity.0 as i64,
                identity.1 as i64,
                canonical_path.to_string_lossy(),
                AllocationState::Planned.as_str(),
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
    let Some(identity) = row.path_identity else {
        return Ok(IdentityStatus::NoCapturedIdentity);
    };
    let Some(path) = &row.canonical_path else {
        return Ok(IdentityStatus::NoCapturedIdentity);
    };
    match fs::symlink_metadata(path) {
        Ok(meta) if (meta.dev(), meta.ino()) == identity => Ok(IdentityStatus::Matches),
        Ok(_) => Ok(IdentityStatus::DifferentObject),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(IdentityStatus::Missing),
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
    let root = verified_root(&row)?;
    let source_identity = match fs::symlink_metadata(&source) {
        Ok(meta) => (meta.dev(), meta.ino()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ArchiveOutcome::SourceMissing);
        }
        Err(e) => return Err(e.into()),
    };
    if source_identity != expected {
        return Err(WorkingCopyError::IdentityMismatch { path: source });
    }
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
    let metadata = fs::symlink_metadata(&root)?;
    if !metadata.is_dir() || (metadata.dev(), metadata.ino()) != expected {
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
    let destination_identity = match fs::symlink_metadata(&destination_path) {
        Ok(meta) => Some((meta.dev(), meta.ino())),
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
    if let Some(actual) = destination_identity {
        let Some(expected) = row.path_identity else {
            return Err(WorkingCopyError::WrongState(row.id.clone()));
        };
        if actual == expected {
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
        if identity_of(&source)? != expected {
            return Err(WorkingCopyError::IdentityMismatch {
                path: source.clone(),
            });
        }
    }

    if overlong_destination && !source_present {
        return Err(std::io::Error::from_raw_os_error(libc::ENAMETOOLONG).into());
    }
    // A recovery may happen much later than the original archive attempt.
    // Keep its journaled candidate first, but base any replacement on the
    // recorded basename and this recovery's timestamp, never on old suffixes.
    let collision_stem = format!("{}-{}", row.original_basename, utc_compact(now_unix()));
    match (source_present, destination_identity) {
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
            if actual != expected {
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
    let (year, month, day) = civil_from_days(days);
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

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 to
/// (year, month, day) in the proleptic Gregorian calendar.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
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
                 created_at           INTEGER NOT NULL
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

    /// The A2 occupancy shape: files, directories, and DANGLING symlinks
    /// all occupy their exact name, and the lowest numeric gap wins.
    #[test]
    fn occupancy_finds_lowest_free_gap_between_files_dirs_and_dangling_symlinks() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        fs::write(root.join("bar-1"), b"file").expect("file at bar-1");
        fs::create_dir(root.join("bar-3")).expect("dir at bar-3");
        std::os::unix::fs::symlink("/nowhere/at/all", root.join("bar-5"))
            .expect("dangling symlink at bar-5");
        assert_eq!(
            lowest_free_suffix(root, "bar", OCCUPANCY_SCAN_CAP).expect("scan"),
            OccupancyOutcome::LowestFree(2)
        );
    }

    /// When the plain basename is itself occupied (whatever the entry
    /// kind), the answer is the lowest free suffix starting at zero.
    #[test]
    fn occupancy_counts_a_plain_file_directory_and_dangling_symlink_as_occupied() {
        for shape in ["file", "dir", "dangling-symlink"] {
            let dir = tempfile::tempdir().expect("tempdir");
            let root = dir.path();
            match shape {
                "file" => fs::write(root.join("bar"), b"").expect("file"),
                "dir" => fs::create_dir(root.join("bar")).expect("dir"),
                _ => std::os::unix::fs::symlink("/nowhere", root.join("bar"))
                    .expect("dangling symlink"),
            }
            assert_eq!(
                lowest_free_suffix(root, "bar", OCCUPANCY_SCAN_CAP).expect("scan"),
                OccupancyOutcome::LowestFree(1),
                "shape {shape} must occupy the plain name"
            );
        }
    }

    /// The reserved archive directory name never participates in naming.
    #[test]
    fn occupancy_skips_the_reserved_archive_directory_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir(dir.path().join(ARCHIVE_DIR_NAME)).expect("archive dir");
        assert_eq!(
            lowest_free_suffix(dir.path(), "bar", OCCUPANCY_SCAN_CAP).expect("scan"),
            OccupancyOutcome::BasenameAvailable
        );
    }

    /// A missing root scans as empty rather than erroring: the plain name
    /// is trivially free.
    #[test]
    fn occupancy_of_a_missing_root_reports_the_basename_free() {
        assert_eq!(
            lowest_free_suffix(Path::new("/nowhere/definitely-missing"), "bar", 10).expect("scan"),
            OccupancyOutcome::BasenameAvailable
        );
    }

    /// The cap is enforced by REFUSAL: an over-cap root yields
    /// IncompleteScan rather than a possibly-wrong lowest number. The cap
    /// is an injected parameter precisely so this path is testable at a
    /// small size.
    #[test]
    fn occupancy_refuses_to_answer_when_the_entry_cap_is_hit() {
        let dir = tempfile::tempdir().expect("tempdir");
        for n in 0..5 {
            fs::write(dir.path().join(format!("other-{n}")), b"").expect("entry");
        }
        match lowest_free_suffix(dir.path(), "bar", 4) {
            Err(WorkingCopyError::IncompleteScan { scanned }) => assert_eq!(scanned, 5),
            other => panic!("expected IncompleteScan, got {other:?}"),
        }
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
        conn.execute(
            "UPDATE working_copies SET canonical_root = ?2, root_device = ?3, root_inode = ?4 WHERE id = ?1",
            rusqlite::params![row.id, host.path().to_str().expect("utf8"), root_identity.0 as i64, root_identity.1 as i64],
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

    /// A refused rename must preserve the pending journal and original inode
    /// so a later recovery can retry the same move. Injecting the syscall's
    /// error is deterministic even when tests run with permission overrides.
    #[test]
    fn reconcile_retains_ownership_after_a_permission_refusal_then_retries() {
        let conn = registry_conn();
        let dir = tempfile::tempdir().unwrap();
        let row = planned_row(&conn, dir.path(), "bar");
        let allocated = allocate(&conn, &row.id, None).unwrap();
        let archive_root = ensure_archive_root(dir.path()).unwrap();
        persist_journal(&conn, &row.id, "bar-journaled", AllocationState::Allocated).unwrap();
        let mut attempts = 0;
        let error = reconcile_archive_with_effects(&conn, &row.id, None, &mut |from, to| {
            assert_eq!(identity_of(from).unwrap(), allocated.identity);
            assert_eq!(to, archive_root.join("bar-journaled"));
            assert!(!to.exists());
            attempts += 1;
            Err(std::io::Error::from_raw_os_error(libc::EACCES))
        })
        .unwrap_err();
        assert!(
            matches!(error, WorkingCopyError::Io(error) if error.kind() == std::io::ErrorKind::PermissionDenied)
        );
        assert_eq!(
            attempts, 1,
            "only collisions permit an automatic name retry"
        );
        let pending = get_working_copy(&conn, &row.id).unwrap().unwrap();
        assert_eq!(pending.allocation_state, AllocationState::ArchivePending);
        assert_eq!(
            pending.archive_destination.as_deref(),
            Some("bar-journaled")
        );
        assert_eq!(
            identity_of(&dir.path().join("bar")).unwrap(),
            allocated.identity
        );
        assert_eq!(fs::read_dir(&archive_root).unwrap().count(), 0);
        assert!(
            matches!(reconcile_archive(&conn, &row.id).unwrap(), ReconcileOutcome::Moved { destination } if destination == "bar-journaled")
        );
        assert_eq!(
            identity_of(&archive_root.join("bar-journaled")).unwrap(),
            allocated.identity
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

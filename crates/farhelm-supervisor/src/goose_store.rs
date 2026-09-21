//! Exact, read-only, bounded reads of one Goose session row.
//!
//! Admission needs one fact no process evidence can supply: that the
//! reported id names a foreground root (`user`, NULL parent) in the
//! attributed runtime's OWN store, in schema 16. This module supplies
//! exactly that — one row plus the same-snapshot schema evidence —
//! through a reader that cannot write, cannot wait past its backstop,
//! and cannot be made to do unbounded work:
//!
//! - a dedicated delegating VFS (`farhelm_goose_ro`) forces every
//!   main/WAL/journal open `O_RDONLY` (no create, no delete-on-close),
//!   refuses temp/transient opens, refuses `xWrite`/`xTruncate` and
//!   VFS-level deletes, caps locks at `SHARED`, forces SHM unmaps to
//!   keep the vendor's `-shm` file, and counts VFS reads per file;
//! - the path must stat as a regular file through a descriptor-free
//!   `metadata` preflight — a FIFO at the store path would stall
//!   the read-only open itself, before SQLite or any progress
//!   handler ever runs. The preflight opens nothing on purpose:
//!   on Unix, closing any descriptor for an inode releases every
//!   POSIX record lock the process holds on that inode, so an
//!   open+drop here could release a concurrent worker's SQLite
//!   `SHARED` lock mid-probe;
//! - the connection opens `READ_ONLY|URI|NO_MUTEX|PRIVATE_CACHE` with
//!   `?mode=ro&readonly_shm=1`, zero busy wait, `DEFENSIVE=true`,
//!   `TRUSTED_SCHEMA=false`, engine caps (1 MiB value/row), a
//!   read-only authorizer, and a progress handler enforcing the 20k
//!   VM-op and 250 ms budgets plus cooperative cancellation;
//! - the one read transaction (deferred `BEGIN`, `COMMIT` on success,
//!   best-effort `ROLLBACK` otherwise) validates the schema and reads
//!   the row in a single snapshot; its `SHARED` lock is never held
//!   across our own commit.
//!
//! The delegation is audited against the PINNED SQLite (bundled
//! 3.53.2 via `libsqlite3-sys 0.38`, behind `rusqlite 0.40`):
//! `sqlite3_vfs_register` takes the master mutex, so one-time
//! registration is safe while other connections are active; the unix
//! VFS serves `readonly_shm=1` opens of `-shm` `O_RDONLY` (no
//! create); and `COMMIT` authorizes as `SQLITE_TRANSACTION/"COMMIT"`
//! (which `rusqlite` surfaces as an unknown transaction operation).
//! The `iVersion` assertion in `xOpen` pins the methods-table shape
//! this audit assumes: anything else fails the open, never calls
//! through to a half-understood table.
//!
//! What this module deliberately does NOT do: scan for sessions (the
//! id arrives bound to the report), follow the `parent_session_id`
//! link (lineage is never evidence here), cache anything across
//! attempts (every admission re-reads the live store), or hold any
//! vendor lock across our commit. Policy over the returned metadata —
//! which types prove a foreground root — lives in
//! [`crate::agent_kind::goose`], not here.

use rusqlite::ffi;
use std::ffi::c_int;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The only Goose store schema this reader accepts: `MAX(version)`
/// from the store's own `schema_version` table must equal this. Older
/// (pre-15, no `parent_session_id`) and newer schemas refuse as
/// unsupported — a store whose layout no audit covers authorizes
/// nothing.
const GOOSE_SCHEMA_VERSION: i64 = 16;

/// VM-operation budget per attempt, enforced by the progress handler.
const READER_VM_BUDGET_OPS: u64 = 20_000;

/// Worker-time budget per attempt, enforced by the progress handler
/// (which aborts the query) — the cancelling half of the ≤ 250 ms
/// reader contract.
const READER_BUDGET: Duration = Duration::from_millis(250);

/// Stall backstop around the worker: a worker stuck OUTSIDE the VM
/// (a pathological syscall stall — the zero busy wait and
/// non-blocking locks leave no legitimate way to block) is refused
/// here WITHOUT rejoining it, because the progress handler cannot
/// interrupt a blocked open or read and awaiting would stall the
/// caller exactly as long as the filesystem does. The refusal sets
/// cooperative cancellation (also armed for an outer drop of this
/// future, via the drop guard in [`read_reported_session`]), and a
/// timed-out worker keeps its semaphore slot until it actually
/// finishes — the permit travels with the worker — so a stall can
/// never let later calls exceed the two-worker bound. Timed-out
/// workers get no mutation access: the refusal stands even if the
/// worker later returns `Ok`.
const READER_STALL_BACKSTOP: Duration = Duration::from_secs(1);

/// VFS-read budget per open file per attempt, enforced in the VFS
/// `xRead` wrapper. An attempt touches at most two files (main db
/// plus WAL), so the attempt total stays within twice this.
const MAX_VFS_READ_BYTES_PER_FILE: u64 = 8 * 1024 * 1024;

/// Engine value/row cap (`SQLITE_LIMIT_LENGTH` covers the maximum
/// size of any string, BLOB, or table row).
const MAX_ENGINE_VALUE_BYTES: i32 = 1024 * 1024;

/// Schema-entry cap for the `pragma_table_info` probe, plus a total
/// byte cap so many small entries cannot sum into unbounded work.
const MAX_SCHEMA_ENTRIES: usize = 64;
const MAX_SCHEMA_BYTES: usize = 4 * 1024;

/// Row cap for the `schema_version` fetch the `MAX(version)` gate is
/// computed over. Versions are a primary key of smallints and Goose
/// is at 16; a store with more version rows than this is malformed,
/// and refusing it loses no legitimate shape.
const MAX_SCHEMA_VERSION_ROWS: usize = 64;

/// One reported session's metadata, as read: the type string (≤ 32
/// bytes) and the raw parent link (≤ 128 bytes). Whether this proves
/// a foreground root is policy, decided by the caller.
#[derive(Debug)]
pub(crate) struct GooseSessionMetadata {
    pub session_type: String,
    pub parent_session_id: Option<String>,
}

/// Why one store read refused. Every variant refuses admission the
/// same way; the message is static (no vendor bytes) for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GooseStoreRefusal {
    pub reason: &'static str,
}

impl std::fmt::Display for GooseStoreRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "goose store refused: {}", self.reason)
    }
}

impl std::error::Error for GooseStoreRefusal {}

fn refuse(reason: &'static str) -> GooseStoreRefusal {
    GooseStoreRefusal { reason }
}

/// Reader slots, process-wide: at most two concurrent store reads;
/// saturation rejects rather than queues, so an admission burst
/// cannot pile workers behind one slow disk.
///
/// The semaphore lives behind an `Arc` so the permit can travel
/// INTO the blocking worker: a worker that outlives its caller's
/// backstop keeps its slot until it actually finishes, and a later
/// call can never observe a freed slot for still-running work.
static READER_SLOTS: std::sync::LazyLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(tokio::sync::Semaphore::new(2)));

/// Sets the reader's cooperative cancellation flag when the read
/// future ends for ANY reason — backstop refusal, success, or an
/// outer cancellation dropped mid-await. The progress handler only
/// runs inside the VM, so this is best-effort for a worker blocked
/// in a syscall, but it is the only signal such a worker can ever
/// observe, and without the drop arm an outer cancellation would
/// leave it running uncancelled while its caller is already gone.
struct CancelOnDrop(std::sync::Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Read one reported session's metadata from the store at an
/// absolute, UTF-8, ≤ 4 KiB path — the resolver's output, re-checked
/// here so a caller bug cannot widen what the reader opens.
///
/// The id is the REPORTED one (≤ 128 bytes, the doorway shape gate's
/// bound, re-checked): the lookup is by primary key, never a scan.
pub(crate) async fn read_reported_session(
    store_path: &str,
    session_id: &str,
) -> Result<GooseSessionMetadata, GooseStoreRefusal> {
    if store_path.is_empty()
        || !store_path.starts_with('/')
        || store_path.len() > 4 * 1024
        || store_path.contains('\0')
    {
        return Err(refuse("the store path is not an absolute bounded path"));
    }
    if session_id.is_empty() || session_id.len() > 128 {
        return Err(refuse("the reported id is not a bounded id"));
    }
    // SAFETY: `geteuid` takes no arguments and has no failure mode.
    if unsafe { libc::geteuid() } == 0 {
        return Err(refuse("store reads refuse privileged operation"));
    }
    let slot = std::sync::Arc::clone(&READER_SLOTS)
        .try_acquire_owned()
        .map_err(|_| refuse("the reader is saturated"))?;
    ensure_vfs()?;
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let worker_cancel = std::sync::Arc::clone(&cancel);
    // Armed for every exit, including an outer drop mid-await: the
    // flag is the only signal a syscall-blocked worker can observe.
    let _cancel_on_drop = CancelOnDrop(std::sync::Arc::clone(&cancel));
    let worker_path = store_path.to_string();
    let worker_id = session_id.to_string();
    let started = Instant::now();
    let mut worker = tokio::task::spawn_blocking(move || {
        // Held until the worker finishes — including past a caller
        // that already took its backstop refusal — so outstanding
        // work keeps occupying its slot.
        let _permit = slot;
        #[cfg(test)]
        park_worker_for_test();
        read_session_blocking(&worker_path, &worker_id, worker_cancel, started)
    });
    let finished = tokio::select! {
        result = &mut worker => Some(result),
        () = tokio::time::sleep(READER_STALL_BACKSTOP) => None,
    };
    match finished {
        Some(Ok(outcome)) => outcome,
        Some(Err(_)) => Err(refuse("the reader worker failed")),
        // No rejoin: awaiting a worker the progress handler cannot
        // interrupt would stall the caller exactly as long as the
        // filesystem does. The worker keeps its slot (see above)
        // and observes cancellation cooperatively; dropping its
        // handle detaches it, and the guard above has already set
        // the flag.
        None => Err(refuse("the reader overran its budget")),
    }
}

/// Test-only stall point for the blocking worker: while parked,
/// workers wait here holding their semaphore slot. A real
/// filesystem stall cannot be produced on demand, and a FIFO no
/// longer reaches the worker past [`require_regular_file`] — so
/// without this seam the backstop's no-rejoin path would have no
/// deterministic test at all.
#[cfg(test)]
static READER_TEST_PARK: std::sync::LazyLock<
    std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
> = std::sync::LazyLock::new(|| {
    std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()))
});

/// Park the calling worker while the test holds the gate. Panics
/// only on a poisoned test mutex, never in production (this
/// function does not exist there).
#[cfg(test)]
fn park_worker_for_test() {
    let (lock, parked) = &**READER_TEST_PARK;
    let mut guard = lock.lock().expect("reader test park mutex poisoned");
    while *guard {
        guard = parked
            .wait(guard)
            .expect("reader test park condvar poisoned");
    }
}

/// Hold or release the worker stall gate from a test. Clearing
/// wakes every parked worker; a worker that wakes proceeds with
/// its normal read, so the gate must be cleared before the test
/// ends or detached workers linger (harmlessly — they hold no test
/// resources — but noisily).
#[cfg(test)]
pub(crate) fn set_reader_test_park(park: bool) {
    let (lock, parked) = &**READER_TEST_PARK;
    *lock.lock().expect("reader test park mutex poisoned") = park;
    if !park {
        parked.notify_all();
    }
}

/// Refuse anything that is not a regular file, WITHOUT opening it:
/// `std::fs::metadata` stats the path and takes no descriptor at
/// all. Opening one here — even `O_NONBLOCK`, even dropped on
/// return — would be a locking hazard, not just a stall question.
/// On Unix, closing any descriptor for an inode releases every
/// POSIX record lock the process holds on that inode, including the
/// `SHARED` locks a concurrent worker's SQLite connection took
/// through its own descriptors (the bundled `sqlite3.c` documents
/// both the release and the deferred-close discipline SQLite uses
/// to live with it; this gate sits outside that machinery). Two
/// workers may read the same Goose database at once, so a preflight
/// open+drop in worker B could release worker A's kernel lock
/// mid-probe while SQLite still believes it is held — and in
/// rollback-journal mode an external Goose writer could then take
/// `EXCLUSIVE` and modify the store underneath A's schema and row
/// reads.
///
/// A stat never blocks the way `O_RDONLY` on a FIFO does (it takes
/// no read end), so a FIFO, socket, device, or directory at the
/// store path refuses here with no stall and no fd. Symlinks are
/// followed — a symlinked data dir is legitimate — what matters is
/// that the target stats as a regular file.
///
/// The residual this leaves is a path swap between this gate and
/// SQLite's own open, which needs write access to the store's
/// directory. A swapped-in FIFO stalls only the bounded worker,
/// which the 1 s backstop refuses without rejoining; handing SQLite
/// an already-open fd instead would break its `-wal`/`-shm`
/// sidecar derivation, so the window stays open by design and is
/// named here instead.
fn require_regular_file(store_path: &str) -> Result<(), GooseStoreRefusal> {
    let is_file = std::fs::metadata(store_path)
        .map_err(|_| refuse("the store path does not stat"))?
        .is_file();
    if !is_file {
        return Err(refuse("the store path is not a regular file"));
    }
    Ok(())
}

/// The whole read, on a blocking worker: refuse non-regular files,
/// open through the read-only VFS, armor the connection, and
/// validate schema plus row inside one short read transaction. Any
/// error at any step refuses — there is no partial result worth
/// returning.
fn read_session_blocking(
    store_path: &str,
    session_id: &str,
    cancel: std::sync::Arc<AtomicBool>,
    started: Instant,
) -> Result<GooseSessionMetadata, GooseStoreRefusal> {
    require_regular_file(store_path)?;
    let uri = store_uri(store_path);
    let connection = rusqlite::Connection::open_with_flags_and_vfs(
        uri,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | rusqlite::OpenFlags::SQLITE_OPEN_URI
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX
            | rusqlite::OpenFlags::SQLITE_OPEN_PRIVATE_CACHE,
        "farhelm_goose_ro",
    )
    .map_err(|_| refuse("the store does not open read-only"))?;
    // Single-threaded use: this connection is created, used, and
    // dropped on this worker, matching `NO_MUTEX`.
    connection
        .busy_timeout(Duration::ZERO)
        .map_err(|_| refuse("the zero busy wait does not apply"))?;
    connection
        .set_limit(
            rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
            MAX_ENGINE_VALUE_BYTES,
        )
        .map_err(|_| refuse("the value cap does not apply"))?;
    connection
        .set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_ATTACHED, 0)
        .map_err(|_| refuse("the attach cap does not apply"))?;
    connection
        .set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_WORKER_THREADS, 0)
        .map_err(|_| refuse("the worker-thread cap does not apply"))?;
    connection
        .set_db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)
        .map_err(|_| refuse("defensive mode does not apply"))?;
    connection
        .set_db_config(
            rusqlite::config::DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA,
            false,
        )
        .map_err(|_| refuse("untrusted schema does not apply"))?;
    connection
        .authorizer(Some(
            |context: rusqlite::hooks::AuthContext<'_>| -> rusqlite::hooks::Authorization {
                use rusqlite::hooks::{AuthAction, Authorization};
                match context.action {
                    AuthAction::Select | AuthAction::Read { .. } => Authorization::Allow,
                    AuthAction::Transaction { operation } => {
                        use rusqlite::hooks::TransactionOperation;
                        match operation {
                            // COMMIT authorizes as
                            // `SQLITE_TRANSACTION/"COMMIT"`, which
                            // `rusqlite` surfaces as `Unknown` —
                            // audited in the bundled `sqlite3.c` —
                            // so `Unknown` here is the commit half
                            // of our own transaction, allowed.
                            // `Release` (savepoints) has no sender
                            // on this read path and stays denied.
                            TransactionOperation::Begin
                            | TransactionOperation::Rollback
                            | TransactionOperation::Unknown => Authorization::Allow,
                            _ => Authorization::Deny,
                        }
                    }
                    // The schema probe's table-valued pragma, which
                    // authorizes as a `Pragma` (name `table_info`,
                    // value the table argument — observed, not
                    // assumed), and the `Function` spelling of the
                    // same probe if a build emits it instead: the
                    // one read the probe needs, pinned to the one
                    // table it reads. Every other pragma and every
                    // other function stays denied.
                    AuthAction::Pragma {
                        pragma_name: "table_info",
                        pragma_value: Some("sessions"),
                    }
                    | AuthAction::Function {
                        function_name: "pragma_table_info",
                    } => Authorization::Allow,
                    _ => Authorization::Deny,
                }
            },
        ))
        .map_err(|_| refuse("the read-only authorizer does not apply"))?;
    let mut operations = 0u64;
    connection
        .progress_handler(
            64,
            Some(move || {
                operations += 64;
                operations > READER_VM_BUDGET_OPS
                    || started.elapsed() > READER_BUDGET
                    || cancel.load(Ordering::SeqCst)
            }),
        )
        .map_err(|_| refuse("the progress bound does not apply"))?;

    connection
        .execute_batch("BEGIN")
        .map_err(|_| refuse("the read transaction does not begin"))?;
    let outcome = read_session_in_transaction(&connection, session_id);
    match outcome {
        Ok(metadata) => connection
            .execute_batch("COMMIT")
            .map(|()| metadata)
            .map_err(|_| refuse("the read transaction does not commit")),
        Err(refusal) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(refusal)
        }
    }
}

/// The evidence, in one snapshot: `sessions` is a real table with the
/// pinned columns, the store is schema 16, and the reported id reads
/// back exactly one bounded row. Order is cheapest-first, but every
/// statement runs inside the caller's transaction, so all of it is
/// one snapshot either way.
fn read_session_in_transaction(
    connection: &rusqlite::Connection,
    session_id: &str,
) -> Result<GooseSessionMetadata, GooseStoreRefusal> {
    let tables: Vec<String> = connection
        .prepare("SELECT type FROM sqlite_master WHERE name = 'sessions'")
        .map_err(|_| refuse("the schema probe does not prepare"))?
        .query_map([], |row| row.get(0))
        .map_err(|_| refuse("the schema probe does not run"))?
        .take(2)
        .collect::<Result<_, _>>()
        .map_err(|_| refuse("the schema probe does not read"))?;
    if tables.as_slice() != ["table"] {
        return Err(refuse("sessions is not a real table"));
    }
    require_pinned_columns(connection)?;
    // No aggregate: even a built-in like `MAX` authorizes as a
    // function call, and the authorizer denies every function but the
    // schema probe's. The maximum is computed over a bounded fetch
    // instead — semantically `MAX(version)`, fail-closed past the
    // bound. The bound matters because fresh and migrated stores
    // differ in ROW COUNT, not in maximum: a fresh store carries
    // exactly one version row, while Goose's migration chain appends
    // one row per applied migration — observed live, a v15 store
    // migrated by Goose 1.50.1 carries (15, 16). Requiring a single
    // row would refuse every upgraded install; the gate is the
    // maximum, exactly 16.
    let mut version_probe = connection
        .prepare("SELECT version FROM schema_version")
        .map_err(|_| refuse("the version probe does not prepare"))?;
    let mut versions = version_probe
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|_| refuse("the version probe does not run"))?
        .take(MAX_SCHEMA_VERSION_ROWS + 1);
    let mut maximum: Option<i64> = None;
    let mut count = 0usize;
    for version in &mut versions {
        let version =
            version.map_err(|_| refuse("a schema version does not decode as an integer"))?;
        count += 1;
        if count > MAX_SCHEMA_VERSION_ROWS {
            return Err(refuse("the schema versions overrun their row bound"));
        }
        maximum = Some(maximum.map_or(version, |best| best.max(version)));
    }
    if maximum != Some(GOOSE_SCHEMA_VERSION) {
        return Err(refuse("the store is not schema 16"));
    }
    let mut probe = connection
        .prepare("SELECT id, session_type, parent_session_id FROM sessions WHERE id = ?1")
        .map_err(|_| refuse("the row probe does not prepare"))?;
    let mut rows = probe
        .query_map([session_id], |row| {
            let id: String = row.get(0)?;
            let session_type: String = row.get(1)?;
            let parent_session_id: Option<String> = row.get(2)?;
            Ok((id, session_type, parent_session_id))
        })
        .map_err(|_| refuse("the row probe does not run"))?
        .take(2);
    let Some(first) = rows.next() else {
        return Err(refuse("the reported id names no row"));
    };
    if rows.next().is_some() {
        return Err(refuse("the reported id names two rows"));
    }
    let (stored_id, session_type, parent_session_id) =
        first.map_err(|_| refuse("the reported row does not decode"))?;
    if stored_id != session_id {
        return Err(refuse("the reported row is not the reported id"));
    }
    if session_type.len() > 32 {
        return Err(refuse("the session type overruns its bound"));
    }
    if parent_session_id
        .as_deref()
        .is_some_and(|parent| parent.len() > 128)
    {
        return Err(refuse("the parent link overruns its bound"));
    }
    Ok(GooseSessionMetadata {
        session_type,
        parent_session_id,
    })
}

/// Require the pinned `sessions` columns — `id TEXT` as the primary
/// key, `session_type TEXT`, `parent_session_id` present — via the
/// read-only table-valued `pragma_table_info('sessions')` (a probe,
/// not a pragma change: no `PRAGMA` statement ever runs here).
///
/// Any deviation refuses as an unsupported schema, including a
/// missing `parent_session_id`: pre-15 stores cannot tell a root from
/// a child, so their rows can never authorize. Declared types compare
/// ASCII-case-insensitively (SQLite type names are); column names
/// compare exactly (the pinned DDL spells them lowercase).
fn require_pinned_columns(connection: &rusqlite::Connection) -> Result<(), GooseStoreRefusal> {
    let mut probe = connection
        .prepare("SELECT name, type, pk FROM pragma_table_info('sessions')")
        .map_err(|_| refuse("the column probe does not prepare"))?;
    let mut entries = probe
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let column_type: String = row.get(1)?;
            let pk: i64 = row.get(2)?;
            Ok((name, column_type, pk))
        })
        .map_err(|_| refuse("the column probe does not run"))?
        .take(MAX_SCHEMA_ENTRIES + 1);
    let mut seen = 0usize;
    let mut bytes = 0usize;
    let mut id_is_text_pk = false;
    let mut session_type_is_text = false;
    let mut parent_present = false;
    for entry in &mut entries {
        let (name, column_type, pk) =
            entry.map_err(|_| refuse("a schema entry does not decode"))?;
        seen += 1;
        if seen > MAX_SCHEMA_ENTRIES {
            return Err(refuse("the schema overruns its entry bound"));
        }
        bytes += name.len() + column_type.len();
        if bytes > MAX_SCHEMA_BYTES {
            return Err(refuse("the schema overruns its byte bound"));
        }
        match name.as_str() {
            "id" if column_type.eq_ignore_ascii_case("text") && pk == 1 => {
                id_is_text_pk = true;
            }
            "session_type" if column_type.eq_ignore_ascii_case("text") => {
                session_type_is_text = true;
            }
            "parent_session_id" if column_type.eq_ignore_ascii_case("text") => {
                parent_present = true;
            }
            _ => {}
        }
    }
    if !(id_is_text_pk && session_type_is_text && parent_present) {
        return Err(refuse("the sessions columns are not the pinned shape"));
    }
    Ok(())
}

/// Build the read-only URI for an already-validated absolute path:
/// `mode=ro` plus `readonly_shm=1` (served `O_RDONLY`, no `-shm`
/// create — audited in the bundled `sqlite3.c`), over a
/// percent-encoded path so `?`, `#`, and `%` in directory names
/// cannot escape into the query string.
fn store_uri(store_path: &str) -> String {
    const UNRESERVED: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~/";
    let mut encoded = String::with_capacity(store_path.len() + 32);
    encoded.push_str("file:");
    for byte in store_path.bytes() {
        if UNRESERVED.contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded.push_str("?mode=ro&readonly_shm=1");
    encoded
}

// ---------------------------------------------------------------------------
// The read-only VFS: `farhelm_goose_ro`
// ---------------------------------------------------------------------------
//
// A delegating wrapper around the default ("unix", on both Linux and
// macOS) VFS. Registration copies the default VFS's function table and
// overrides two entries — `xOpen` and `xDelete` — plus swaps every
// opened file's methods table for the read-only one below. The
// no-write guarantee therefore rests on five independent layers, so
// that no single SQLite behavior the audit missed can open a write:
// O_RDONLY opens, refused xWrite/xTruncate, refused VFS-level
// deletes, SHARED-only locks, and SHM unmaps that never delete. A
// sixth layer — the read-only authorizer plus a read-only statement
// set — means the core never even ASKS for a write on this path.

/// The VFS name, NUL-terminated for `sqlite3_vfs.zName`.
static VFS_NAME: &[u8] = b"farhelm_goose_ro\0";

/// Per-file trailer bytes past the underlying VFS's `szOsFile`: the
/// running VFS-read count (`u64`, bytes `0..8`) and the underlying
/// methods table (`usize`, bytes `8..16`), both native-endian and
/// both accessed with unaligned-safe byte copies — the underlying
/// size is NOT guaranteed 8-aligned.
const TRAILER_BYTES: usize = 16;

/// Registration outcome, computed once per process. A failure caches —
/// every failure mode here is deterministic (no default VFS, a size
/// overflow), never transient, so retrying could not help.
static VFS_INIT: OnceLock<Result<(), GooseStoreRefusal>> = OnceLock::new();

/// The underlying VFS's `szOsFile`, published by [`ensure_vfs`] so
/// the methods-table wrappers can find their per-file trailer.
static UNDERLYING_FILE_SIZE: OnceLock<c_int> = OnceLock::new();

/// Register `farhelm_goose_ro` once per process. Registration takes
/// SQLite's master mutex (audited in the bundled `sqlite3.c`), so
/// calling this while other connections are active is safe; and
/// `makeDflt=0` keeps the default VFS untouched for everyone else.
/// The registered VFS needs no stored pointer afterwards: SQLite
/// passes it to `xOpen`, and `xOpen` recovers the underlying VFS
/// from `pAppData`.
fn ensure_vfs() -> Result<(), GooseStoreRefusal> {
    *VFS_INIT.get_or_init(|| {
        // SAFETY: `vfs_find(NULL)` returns the default VFS or NULL
        // and takes the master mutex.
        let underlying = unsafe { ffi::sqlite3_vfs_find(std::ptr::null()) };
        if underlying.is_null() {
            return Err(refuse("SQLite has no default VFS"));
        }
        // SAFETY: bitwise copy of a fully initialized VFS struct,
        // which holds only integers and function pointers.
        let mut vfs: ffi::sqlite3_vfs = unsafe { std::ptr::read(underlying) };
        let underlying_size = vfs.szOsFile;
        let Some(grown) = underlying_size.checked_add(TRAILER_BYTES as c_int) else {
            return Err(refuse("the VFS file size overflows"));
        };
        vfs.szOsFile = grown;
        vfs.pNext = std::ptr::null_mut();
        vfs.zName = VFS_NAME.as_ptr() as *const std::ffi::c_char;
        vfs.pAppData = underlying as *mut std::ffi::c_void;
        vfs.xOpen = Some(goose_ro_open);
        vfs.xDelete = Some(goose_ro_delete);
        // Leaked for the process: SQLite holds this pointer for as
        // long as the VFS is registered, which is forever.
        let raw = Box::into_raw(Box::new(vfs));
        // SAFETY: `raw` points to a fully initialized VFS whose name
        // storage is `'static`; registration takes the master mutex.
        let registered = unsafe { ffi::sqlite3_vfs_register(raw, 0) };
        if registered != ffi::SQLITE_OK {
            // SAFETY: registration failed, so nothing references the
            // box; reclaim it rather than leaking on the error path.
            unsafe {
                drop(Box::from_raw(raw));
            }
            return Err(refuse("the read-only VFS does not register"));
        }
        UNDERLYING_FILE_SIZE.get_or_init(|| underlying_size);
        Ok(())
    })
}

/// The file kinds one read needs. Every other kind — temp,
/// transient, sub/super journals — fails the open: the reader has no
/// legitimate temp-file use, and `DELETEONCLOSE` temporaries would be
/// writes by another name.
fn goose_ro_file_type_allowed(flags: c_int) -> bool {
    let file_type = flags
        & (ffi::SQLITE_OPEN_MAIN_DB
            | ffi::SQLITE_OPEN_TEMP_DB
            | ffi::SQLITE_OPEN_TRANSIENT_DB
            | ffi::SQLITE_OPEN_MAIN_JOURNAL
            | ffi::SQLITE_OPEN_TEMP_JOURNAL
            | ffi::SQLITE_OPEN_SUBJOURNAL
            | ffi::SQLITE_OPEN_SUPER_JOURNAL
            | ffi::SQLITE_OPEN_WAL);
    file_type == ffi::SQLITE_OPEN_MAIN_DB
        || file_type == ffi::SQLITE_OPEN_MAIN_JOURNAL
        || file_type == ffi::SQLITE_OPEN_WAL
}

/// Open through the underlying VFS with the access forced read-only,
/// then swap the file's methods table for the read-only one.
///
/// Unnamed (`NULL`-name) opens fail: those are temporaries, denied
/// above. The methods-table's `iVersion` must be exactly the audited
/// shape (3, carrying the SHM and memory-map methods the WAL read
/// forwards): anything else closes the file and fails the open
/// rather than calling through to a half-understood table.
unsafe extern "C" fn goose_ro_open(
    vfs: *mut ffi::sqlite3_vfs,
    name: ffi::sqlite3_filename,
    file: *mut ffi::sqlite3_file,
    flags: c_int,
    out_flags: *mut c_int,
) -> c_int {
    if vfs.is_null() || file.is_null() || name.is_null() {
        return ffi::SQLITE_CANTOPEN;
    }
    // SAFETY: non-null per the check above; `pAppData` is the
    // underlying VFS pointer `ensure_vfs` stored at registration.
    let underlying = unsafe { (*vfs).pAppData as *mut ffi::sqlite3_vfs };
    if underlying.is_null() || !goose_ro_file_type_allowed(flags) {
        return ffi::SQLITE_CANTOPEN;
    }
    let forced = (flags
        & !(ffi::SQLITE_OPEN_READWRITE
            | ffi::SQLITE_OPEN_CREATE
            | ffi::SQLITE_OPEN_DELETEONCLOSE
            | ffi::SQLITE_OPEN_EXCLUSIVE))
        | ffi::SQLITE_OPEN_READONLY;
    // SAFETY: `underlying` is a live registered VFS; `name`/`file`
    // are the core's valid open arguments.
    let open = unsafe { (*underlying).xOpen };
    let Some(open) = open else {
        return ffi::SQLITE_CANTOPEN;
    };
    let opened = unsafe { open(underlying, name, file, forced, out_flags) };
    if opened != ffi::SQLITE_OK {
        return opened;
    }
    // SAFETY: the open succeeded, so the file holds a methods table.
    let underlying_methods = unsafe { (*file).pMethods };
    if underlying_methods.is_null() {
        // Impossible for the unix VFS (a successful open always sets
        // the table); fail closed. The fd leaks on this
        // cannot-happen path — there is no table to close through.
        return ffi::SQLITE_CANTOPEN;
    }
    // SAFETY: non-null per the check above.
    if unsafe { (*underlying_methods).iVersion } < 3 {
        // SAFETY: the table is live; closing a just-opened file.
        if let Some(close) = unsafe { (*underlying_methods).xClose } {
            unsafe {
                close(file);
            }
        }
        return ffi::SQLITE_CANTOPEN;
    }
    let Some(underlying_size) = UNDERLYING_FILE_SIZE.get().copied() else {
        // Impossible: published by the registration that installed
        // this `xOpen`. Fail closed through the live table.
        // SAFETY: the table is live; closing a just-opened file.
        if let Some(close) = unsafe { (*underlying_methods).xClose } {
            unsafe {
                close(file);
            }
        }
        return ffi::SQLITE_CANTOPEN;
    };
    // SAFETY: the core allocated `szOsFile` bytes for this file —
    // the underlying size plus the trailer — so the trailer range is
    // ours. Byte copies, because the offset may not be aligned.
    unsafe {
        let trailer = (file as *mut u8).add(underlying_size as usize);
        std::ptr::write_bytes(trailer, 0, TRAILER_BYTES);
        let methods = (underlying_methods as usize).to_ne_bytes();
        std::ptr::copy_nonoverlapping(methods.as_ptr(), trailer.add(8), 8);
        (*file).pMethods = &READONLY_METHODS;
    }
    ffi::SQLITE_OK
}

/// VFS-level deletes never happen: the reader must not remove a
/// journal, a WAL, or anything else beside the store.
unsafe extern "C" fn goose_ro_delete(
    _vfs: *mut ffi::sqlite3_vfs,
    _name: *const std::ffi::c_char,
    _sync_dir: c_int,
) -> c_int {
    ffi::SQLITE_READONLY
}

/// Load the underlying methods table from the file's trailer. Returns
/// NULL when the trailer is unreachable — every caller fails closed
/// on that — which cannot happen for files this VFS opened.
fn goose_ro_underlying(file: *mut ffi::sqlite3_file) -> *const ffi::sqlite3_io_methods {
    if file.is_null() {
        return std::ptr::null();
    }
    let Some(underlying_size) = UNDERLYING_FILE_SIZE.get().copied() else {
        return std::ptr::null();
    };
    // SAFETY: files reaching the methods table came through
    // `goose_ro_open`, which wrote exactly this trailer. Unaligned-safe
    // byte copy for the pointer-sized field.
    unsafe {
        let slot = (file as *const u8).add(underlying_size as usize + 8);
        let mut bytes = [0u8; (usize::BITS / 8) as usize];
        std::ptr::copy_nonoverlapping(slot, bytes.as_mut_ptr(), bytes.len());
        usize::from_ne_bytes(bytes) as *const ffi::sqlite3_io_methods
    }
}

/// Load and add the running VFS-read count, enforcing the per-file
/// budget BEFORE the read: overruns refuse rather than partially
/// exceed. Failures to reach the trailer refuse — they cannot happen
/// for files this VFS opened.
fn goose_ro_charge_read(file: *mut ffi::sqlite3_file, amount: c_int) -> bool {
    if file.is_null() {
        return false;
    }
    let Some(underlying_size) = UNDERLYING_FILE_SIZE.get().copied() else {
        return false;
    };
    // SAFETY: as in `goose_ro_underlying`; the count field is the
    // trailer's first eight bytes.
    unsafe {
        let slot = (file as *mut u8).add(underlying_size as usize);
        let mut bytes = [0u8; 8];
        std::ptr::copy_nonoverlapping(slot, bytes.as_mut_ptr(), 8);
        let charged = u64::from_ne_bytes(bytes).saturating_add(amount as u64);
        if charged > MAX_VFS_READ_BYTES_PER_FILE {
            return false;
        }
        let bytes = charged.to_ne_bytes();
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), slot, 8);
        true
    }
}

unsafe extern "C" fn goose_ro_close(file: *mut ffi::sqlite3_file) -> c_int {
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return ffi::SQLITE_IOERR;
    }
    // SAFETY: the trailer holds the live underlying table for a file
    // this VFS opened; closing is always safe.
    match unsafe { (*methods).xClose } {
        Some(close) => unsafe { close(file) },
        None => ffi::SQLITE_IOERR,
    }
}

unsafe extern "C" fn goose_ro_read(
    file: *mut ffi::sqlite3_file,
    buffer: *mut std::ffi::c_void,
    amount: c_int,
    offset: ffi::sqlite3_int64,
) -> c_int {
    let methods = goose_ro_underlying(file);
    if methods.is_null() || !goose_ro_charge_read(file, amount) {
        return ffi::SQLITE_IOERR_READ;
    }
    // SAFETY: the trailer holds the live underlying table; the core's
    // buffer, amount, and offset are valid read arguments.
    match unsafe { (*methods).xRead } {
        Some(read) => unsafe { read(file, buffer, amount, offset) },
        None => ffi::SQLITE_IOERR_READ,
    }
}

unsafe extern "C" fn goose_ro_write(
    _file: *mut ffi::sqlite3_file,
    _buffer: *const std::ffi::c_void,
    _amount: c_int,
    _offset: ffi::sqlite3_int64,
) -> c_int {
    ffi::SQLITE_READONLY
}

unsafe extern "C" fn goose_ro_truncate(
    _file: *mut ffi::sqlite3_file,
    _size: ffi::sqlite3_int64,
) -> c_int {
    ffi::SQLITE_READONLY
}

unsafe extern "C" fn goose_ro_sync(file: *mut ffi::sqlite3_file, flags: c_int) -> c_int {
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return ffi::SQLITE_IOERR;
    }
    // SAFETY: the trailer holds the live underlying table; syncing an
    // O_RDONLY file is a harmless no-op the core may still request.
    match unsafe { (*methods).xSync } {
        Some(sync) => unsafe { sync(file, flags) },
        None => ffi::SQLITE_IOERR,
    }
}

unsafe extern "C" fn goose_ro_file_size(
    file: *mut ffi::sqlite3_file,
    size: *mut ffi::sqlite3_int64,
) -> c_int {
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return ffi::SQLITE_IOERR;
    }
    // SAFETY: the trailer holds the live underlying table; the core's
    // out-pointer is valid.
    match unsafe { (*methods).xFileSize } {
        Some(file_size) => unsafe { file_size(file, size) },
        None => ffi::SQLITE_IOERR,
    }
}

unsafe extern "C" fn goose_ro_lock(file: *mut ffi::sqlite3_file, lock: c_int) -> c_int {
    // SHARED (and below) is the read transaction's single snapshot;
    // RESERVED and EXCLUSIVE have no sender on a read and refuse.
    // (A hot rollback journal needing recovery therefore refuses the
    // read — recovery is a write, correctly out of reach.)
    if lock > ffi::SQLITE_LOCK_SHARED {
        return ffi::SQLITE_READONLY;
    }
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return ffi::SQLITE_IOERR;
    }
    // SAFETY: the trailer holds the live underlying table.
    match unsafe { (*methods).xLock } {
        Some(take) => unsafe { take(file, lock) },
        None => ffi::SQLITE_IOERR,
    }
}

unsafe extern "C" fn goose_ro_unlock(file: *mut ffi::sqlite3_file, lock: c_int) -> c_int {
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return ffi::SQLITE_IOERR;
    }
    // SAFETY: the trailer holds the live underlying table; releasing
    // a lock is always safe.
    match unsafe { (*methods).xUnlock } {
        Some(release) => unsafe { release(file, lock) },
        None => ffi::SQLITE_IOERR,
    }
}

unsafe extern "C" fn goose_ro_check_reserved_lock(
    file: *mut ffi::sqlite3_file,
    reserved: *mut c_int,
) -> c_int {
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return ffi::SQLITE_IOERR;
    }
    // SAFETY: the trailer holds the live underlying table; the core's
    // out-pointer is valid.
    match unsafe { (*methods).xCheckReservedLock } {
        Some(check) => unsafe { check(file, reserved) },
        None => ffi::SQLITE_IOERR,
    }
}

/// Whether one file-control opcode forwards to the underlying VFS.
/// The allow-list is the harmless query opcodes plus the sync the
/// core may request; EVERYTHING else — the mutating controls (chunk
/// and mmap sizing, WAL persistence and blocking, checkpoint
/// fencing, atomic-write phases, sync omission), the fd-exposing
/// controls (file and journal pointers), and the platform and
/// third-party controls — answers `NOTFOUND`.
///
/// Denying by default can only cost availability (a refused read),
/// never integrity: no write on this path reaches the fd except
/// through `xWrite`/memory-map/lock-upgrade, all refused at their
/// own layer. The passing tests prove the allowed set suffices for
/// WAL and rollback reads on the pinned build.
fn goose_ro_file_control_allowed(op: c_int) -> bool {
    matches!(
        op,
        ffi::SQLITE_FCNTL_LOCKSTATE
            | ffi::SQLITE_FCNTL_LAST_ERRNO
            | ffi::SQLITE_FCNTL_VFSNAME
            | ffi::SQLITE_FCNTL_HAS_MOVED
            | ffi::SQLITE_FCNTL_SYNC
            | ffi::SQLITE_FCNTL_LOCK_TIMEOUT
            | ffi::SQLITE_FCNTL_DATA_VERSION
            | ffi::SQLITE_FCNTL_CKSM_FILE
            | ffi::SQLITE_FCNTL_TRACE
            | ffi::SQLITE_FCNTL_VFS_POINTER
            | ffi::SQLITE_FCNTL_FILESTAT
    )
}

unsafe extern "C" fn goose_ro_file_control(
    file: *mut ffi::sqlite3_file,
    op: c_int,
    arg: *mut std::ffi::c_void,
) -> c_int {
    if !goose_ro_file_control_allowed(op) {
        return ffi::SQLITE_NOTFOUND;
    }
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return ffi::SQLITE_IOERR;
    }
    // SAFETY: the trailer holds the live underlying table; the
    // allow-listed opcodes take the core's argument safely.
    match unsafe { (*methods).xFileControl } {
        Some(control) => unsafe { control(file, op, arg) },
        None => ffi::SQLITE_NOTFOUND,
    }
}

unsafe extern "C" fn goose_ro_sector_size(file: *mut ffi::sqlite3_file) -> c_int {
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return 0;
    }
    // SAFETY: the trailer holds the live underlying table.
    match unsafe { (*methods).xSectorSize } {
        Some(sector_size) => unsafe { sector_size(file) },
        None => 0,
    }
}

unsafe extern "C" fn goose_ro_device_characteristics(file: *mut ffi::sqlite3_file) -> c_int {
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return 0;
    }
    // SAFETY: the trailer holds the live underlying table.
    match unsafe { (*methods).xDeviceCharacteristics } {
        Some(characteristics) => unsafe { characteristics(file) },
        None => 0,
    }
}

unsafe extern "C" fn goose_ro_shm_map(
    file: *mut ffi::sqlite3_file,
    page: c_int,
    page_size: c_int,
    extend: c_int,
    region: *mut *mut std::ffi::c_void,
) -> c_int {
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return ffi::SQLITE_IOERR;
    }
    // SAFETY: the trailer holds the live underlying table; with
    // `readonly_shm=1` the `-shm` fd underneath is O_RDONLY and the
    // node read-only, so mapping it cannot write.
    match unsafe { (*methods).xShmMap } {
        Some(map) => unsafe { map(file, page, page_size, extend, region) },
        None => ffi::SQLITE_IOERR,
    }
}

unsafe extern "C" fn goose_ro_shm_lock(
    file: *mut ffi::sqlite3_file,
    offset: c_int,
    count: c_int,
    flags: c_int,
) -> c_int {
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return ffi::SQLITE_IOERR;
    }
    // SAFETY: the trailer holds the live underlying table; SHM locks
    // coordinate the read against the vendor writer.
    match unsafe { (*methods).xShmLock } {
        Some(lock) => unsafe { lock(file, offset, count, flags) },
        None => ffi::SQLITE_IOERR,
    }
}

unsafe extern "C" fn goose_ro_shm_barrier(file: *mut ffi::sqlite3_file) {
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return;
    }
    // SAFETY: the trailer holds the live underlying table.
    if let Some(barrier) = unsafe { (*methods).xShmBarrier } {
        unsafe {
            barrier(file);
        }
    }
}

unsafe extern "C" fn goose_ro_shm_unmap(file: *mut ffi::sqlite3_file, _delete: c_int) -> c_int {
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return ffi::SQLITE_IOERR;
    }
    // SAFETY: the trailer holds the live underlying table. The
    // delete flag is FORCED to zero: unmapping must never unlink the
    // vendor's live `-shm` file, which the core would otherwise do
    // when our short-lived connection is the last one out.
    match unsafe { (*methods).xShmUnmap } {
        Some(unmap) => unsafe { unmap(file, 0) },
        None => ffi::SQLITE_IOERR,
    }
}

unsafe extern "C" fn goose_ro_fetch(
    file: *mut ffi::sqlite3_file,
    offset: ffi::sqlite3_int64,
    amount: c_int,
    region: *mut *mut std::ffi::c_void,
) -> c_int {
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return ffi::SQLITE_IOERR;
    }
    // SAFETY: the trailer holds the live underlying table. The
    // mapping derives `PROT_READ` from our O_RDONLY open, so a write
    // through it faults rather than corrupting — and the read-only
    // transaction never issues one.
    match unsafe { (*methods).xFetch } {
        Some(fetch) => unsafe { fetch(file, offset, amount, region) },
        None => ffi::SQLITE_IOERR,
    }
}

unsafe extern "C" fn goose_ro_unfetch(
    file: *mut ffi::sqlite3_file,
    offset: ffi::sqlite3_int64,
    region: *mut std::ffi::c_void,
) -> c_int {
    let methods = goose_ro_underlying(file);
    if methods.is_null() {
        return ffi::SQLITE_IOERR;
    }
    // SAFETY: the trailer holds the live underlying table.
    match unsafe { (*methods).xUnfetch } {
        Some(unfetch) => unsafe { unfetch(file, offset, region) },
        None => ffi::SQLITE_IOERR,
    }
}

/// The read-only methods table, swapped onto every file the VFS
/// opens. `iVersion` 3 is the audited shape (SHM plus memory-map
/// methods); `xOpen` refuses any underlying table below it.
static READONLY_METHODS: ffi::sqlite3_io_methods = ffi::sqlite3_io_methods {
    iVersion: 3,
    xClose: Some(goose_ro_close),
    xRead: Some(goose_ro_read),
    xWrite: Some(goose_ro_write),
    xTruncate: Some(goose_ro_truncate),
    xSync: Some(goose_ro_sync),
    xFileSize: Some(goose_ro_file_size),
    xLock: Some(goose_ro_lock),
    xUnlock: Some(goose_ro_unlock),
    xCheckReservedLock: Some(goose_ro_check_reserved_lock),
    xFileControl: Some(goose_ro_file_control),
    xSectorSize: Some(goose_ro_sector_size),
    xDeviceCharacteristics: Some(goose_ro_device_characteristics),
    xShmMap: Some(goose_ro_shm_map),
    xShmLock: Some(goose_ro_shm_lock),
    xShmBarrier: Some(goose_ro_shm_barrier),
    xShmUnmap: Some(goose_ro_shm_unmap),
    xFetch: Some(goose_ro_fetch),
    xUnfetch: Some(goose_ro_unfetch),
};

/// The pinned Goose `sessions` layout: the exact table, indexes, and
/// `schema_version` row Goose 1.50.1's own initializer writes
/// (verified against a live `goose session list` store and the v1.50.1
/// tag's `session_manager.rs`, which matches the pinned revision on
/// the schema version, the session-type wire values, and the
/// create-then-link subagent semantics).
///
/// Shared by this module's tests and the admission fixture so the
/// vendor DDL has exactly one spelling in the tree.
#[cfg(test)]
pub(crate) fn plant_goose_schema_16(connection: &rusqlite::Connection) {
    connection
        .execute_batch(
            "CREATE TABLE sessions(
               id TEXT PRIMARY KEY,
               name TEXT NOT NULL DEFAULT '',
               description TEXT NOT NULL DEFAULT '',
               user_set_name BOOLEAN DEFAULT FALSE,
               session_type TEXT NOT NULL DEFAULT 'user',
               working_dir TEXT NOT NULL,
               created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
               updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
               extension_data TEXT DEFAULT '{}',
               total_tokens INTEGER,
               input_tokens INTEGER,
               output_tokens INTEGER,
               cache_read_tokens INTEGER,
               cache_write_tokens INTEGER,
               accumulated_total_tokens INTEGER,
               accumulated_input_tokens INTEGER,
               accumulated_output_tokens INTEGER,
               accumulated_cache_read_tokens INTEGER,
               accumulated_cache_write_tokens INTEGER,
               accumulated_cost REAL,
               schedule_id TEXT,
               recipe_json TEXT,
               user_recipe_values_json TEXT,
               provider_name TEXT,
               model_config_json TEXT,
               goose_mode TEXT NOT NULL DEFAULT 'auto',
               archived_at TIMESTAMP,
               project_id TEXT,
               parent_session_id TEXT
             );
             CREATE INDEX idx_sessions_type ON sessions(session_type);
             CREATE INDEX idx_sessions_parent ON sessions(parent_session_id);
             CREATE TABLE schema_version(
               version INTEGER PRIMARY KEY,
               applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
             );
             INSERT INTO schema_version(version) VALUES(16);",
        )
        .expect("the pinned DDL applies");
}

/// One store row with the columns the reader consults set
/// explicitly; everything else takes Goose's own defaults.
#[cfg(test)]
pub(crate) fn plant_goose_session(
    connection: &rusqlite::Connection,
    id: &str,
    session_type: &str,
    parent_session_id: Option<&str>,
) {
    connection
        .execute(
            "INSERT INTO sessions(id, session_type, parent_session_id, working_dir) \
             VALUES (?1, ?2, ?3, '/tmp')",
            rusqlite::params![id, session_type, parent_session_id],
        )
        .expect("the session row plants");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Open a scratch store file through plain `rusqlite` (the
    /// writer's side of every fixture): the reader under test never
    /// shares a connection with the plant.
    fn planted_store(
        dir: &farhelm_teststate::TestDir,
        name: &str,
    ) -> (std::path::PathBuf, rusqlite::Connection) {
        let path = dir.path().join(name);
        let connection =
            rusqlite::Connection::open(&path).expect("the scratch store opens for planting");
        (path, connection)
    }

    fn read_blocking(path: &std::path::Path, id: &str) -> Result<GooseSessionMetadata, String> {
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        ensure_vfs().map_err(|refusal| refusal.reason.to_string())?;
        read_session_blocking(
            path.to_str().expect("fixture paths are UTF-8"),
            id,
            cancel,
            Instant::now(),
        )
        .map_err(|refusal| refusal.reason.to_string())
    }

    /// A child process the test owns: killed and reaped on drop, so a
    /// panicking assertion cannot orphan a lock-holder behind it.
    struct OwnedChild(std::process::Child);

    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// The cross-process lock-holder: `python3` with its `sqlite3`
    /// stdlib module, the one SQLite speaker the repo already
    /// requires everywhere (AGENTS.md runs everything under Python
    /// 3.11+). Returns `None` — after the usual `SKIPPED` line —
    /// when either is missing.
    ///
    /// A child PROCESS, not a second connection, because POSIX file
    /// locks never contend within one process and the `-shm` mapping
    /// is shared per process: an in-process "writer" neither blocks
    /// the reader nor isolates the `-shm` bytes, so both the busy
    /// refusal and the byte-identity assertions would test nothing.
    /// Production is cross-process (Goose writes, the supervisor
    /// reads), and so is this holder.
    fn require_python_sqlite() -> Option<()> {
        let probe = std::process::Command::new("python3")
            .arg("-c")
            .arg("import sqlite3")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        match probe {
            Ok(status) if status.success() => Some(()),
            _ => {
                println!("SKIPPED: python3 with sqlite3 is required for the cross-process holder");
                None
            }
        }
    }

    /// Spawn `script` under `python3` with `argv`, wait for it to
    /// create `ready` (bounded), and hand back the owned child. The
    /// script must then hold its state until killed.
    fn spawn_holder(
        dir: &farhelm_teststate::TestDir,
        name: &str,
        script: &str,
        argv: &[&str],
    ) -> OwnedChild {
        let script_path = dir.path().join(name);
        std::fs::write(&script_path, script).expect("the holder script plants");
        let ready = dir.path().join(format!("{name}.ready"));
        let mut child = std::process::Command::new("python3")
            .arg(&script_path)
            .args(argv)
            .arg(&ready)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("the holder spawns");
        let deadline = Instant::now() + Duration::from_secs(15);
        while !ready.exists() {
            assert!(
                Instant::now() < deadline,
                "the holder signals readiness within 15 s"
            );
            match child.try_wait().expect("the holder polls") {
                Some(status) => panic!("the holder exited before readiness: {status}"),
                // sleep-ok: polling interval while the holder opens the store.
                None => std::thread::sleep(Duration::from_millis(20)),
            }
        }
        OwnedChild(child)
    }

    /// The happy path, in rollback-journal mode: a `user` row with a
    /// NULL parent reads back its metadata. The reader returns
    /// metadata UNFILTERED — the foreground allowlist is the caller's
    /// policy, pinned where it lives — so this also proves the reader
    /// itself draws no type conclusions.
    #[farhelm_testtrace::test]
    fn a_schema_16_user_row_reads_its_metadata() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        plant_goose_session(&writer, "parent-1", "user", None);
        drop(writer);

        let metadata = read_blocking(&path, "parent-1").expect("the user row reads");
        assert_eq!(metadata.session_type, "user");
        assert_eq!(metadata.parent_session_id, None);
    }

    /// The async entry admits a well-formed read end to end:
    /// validation, a reader slot, the blocking worker, and the stall
    /// backstop all pass a legitimate store through. The matrix above
    /// drives the worker directly for speed; this one proves the
    /// production path around it.
    #[farhelm_testtrace::test]
    async fn the_async_entry_reads_end_to_end() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        plant_goose_session(&writer, "entry-1", "user", None);
        drop(writer);

        let metadata =
            read_reported_session(path.to_str().expect("fixture paths are UTF-8"), "entry-1")
                .await
                .expect("the async entry reads");
        assert_eq!(metadata.session_type, "user");
        assert_eq!(metadata.parent_session_id, None);
    }

    /// The reader returns non-foreground metadata as data: a
    /// `sub_agent` row with a NULL parent — the creation race made
    /// visible — reads back `Ok`, because rejecting it is the
    /// allowlist's job at admission, not the reader's. Collapsing the
    /// two would hide whether a refusal came from malformed storage
    /// or from policy.
    #[farhelm_testtrace::test]
    fn non_foreground_metadata_reads_unfiltered() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        plant_goose_session(&writer, "child-1", "sub_agent", None);
        plant_goose_session(&writer, "child-2", "sub_agent", Some("parent-1"));
        drop(writer);

        let racing = read_blocking(&path, "child-1").expect("the racing row reads as data");
        assert_eq!(racing.session_type, "sub_agent");
        assert_eq!(racing.parent_session_id, None);
        let linked = read_blocking(&path, "child-2").expect("the linked row reads as data");
        assert_eq!(linked.session_type, "sub_agent");
        assert_eq!(linked.parent_session_id.as_deref(), Some("parent-1"));
    }

    /// A live WAL store with uncheckpointed frames reads CURRENT
    /// data — the row the writer just inserted, visible only in the
    /// WAL — and the read leaves every byte and every directory entry
    /// untouched: no checkpoint, no `-shm` removal, no temp files.
    /// This is the test that would catch an `immutable=1`-style stale
    /// open (its row would be missing) and a checkpointing reader
    /// (its bytes would move).
    ///
    /// The writer is the cross-process holder, holding an open IDLE
    /// connection: its `-shm` mapping is genuinely another process's,
    /// so the reader's `O_RDONLY` mapping is what the byte-identity
    /// proves. An in-process writer would share the mapping and move
    /// the `-shm` bytes itself.
    #[farhelm_testtrace::test]
    fn a_live_wal_store_reads_current_data_without_touching_a_byte() {
        let Some(()) = require_python_sqlite() else {
            return;
        };
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        writer
            .execute_batch("PRAGMA journal_mode = WAL;")
            .expect("WAL mode applies");
        drop(writer);
        let path_str = path.to_str().expect("fixture paths are UTF-8").to_string();
        let _holder = spawn_holder(
            &dir,
            "wal_holder.py",
            "import sqlite3, sys, time\n\
             db, ready = sys.argv[1], sys.argv[2]\n\
             connection = sqlite3.connect(db, timeout=30)\n\
             connection.execute(\n\
             \"INSERT INTO sessions(id, session_type, working_dir) \
             VALUES('wal-parent', 'user', '/tmp')\")\n\
             connection.commit()\n\
             open(ready, 'w').write('ready')\n\
             time.sleep(120)\n",
            &[&path_str],
        );

        let snapshot = |label: &str| {
            let mut entries: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir.path())
                .expect("the store dir lists")
                .map(|entry| {
                    let entry = entry.expect("a dir entry reads");
                    let bytes = std::fs::read(entry.path()).expect("a store file reads");
                    (entry.file_name().to_string_lossy().into_owned(), bytes)
                })
                .collect();
            entries.sort();
            assert!(
                entries.iter().any(|(name, _)| name == "sessions.db-wal"),
                "{label}: the fixture premise is uncheckpointed WAL frames"
            );
            entries
        };
        let before = snapshot("before");

        let metadata = read_blocking(&path, "wal-parent").expect("the WAL row reads");
        assert_eq!(metadata.session_type, "user");
        assert_eq!(metadata.parent_session_id, None);

        // The writer stays open across the assertion on purpose: an
        // `-shm` unlink by the reader would only show while the
        // vendor side still holds the mapping.
        let after = snapshot("after");
        assert_eq!(
            before.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            after.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            "the read creates and removes no files"
        );
        for ((name, before_bytes), (_, after_bytes)) in before.iter().zip(after.iter()) {
            if before_bytes == after_bytes {
                continue;
            }
            let differing = before_bytes
                .iter()
                .zip(after_bytes.iter())
                .filter(|(a, b)| a != b)
                .count()
                + before_bytes.len().abs_diff(after_bytes.len());
            let first = before_bytes
                .iter()
                .zip(after_bytes.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(before_bytes.len().min(after_bytes.len()));
            panic!(
                "{name}: {differing} bytes differ starting at offset {first} \
                 ({} -> {} bytes)",
                before_bytes.len(),
                after_bytes.len()
            );
        }
    }

    /// WAL frames without a `-shm` index refuse: without the index
    /// the reader cannot establish the current snapshot, and reading
    /// the main db alone would be a STALE read. Failing the open is
    /// what keeps a torn vendor state from authorizing anything.
    ///
    /// The torn state is built the way only a crash builds it: the
    /// cross-process holder commits frames and stays open, and the
    /// test unlinks the `-shm` under it (a clean close would
    /// checkpoint the frames away instead). The holder keeps its own
    /// mapping; the reader, arriving fresh, finds frames with no
    /// index and refuses.
    #[farhelm_testtrace::test]
    fn wal_without_shm_refuses_rather_than_reading_stale() {
        let Some(()) = require_python_sqlite() else {
            return;
        };
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        writer
            .execute_batch("PRAGMA journal_mode = WAL;")
            .expect("WAL mode applies");
        drop(writer);
        let path_str = path.to_str().expect("fixture paths are UTF-8").to_string();
        let _holder = spawn_holder(
            &dir,
            "wal_holder.py",
            "import sqlite3, sys, time\n\
             db, ready = sys.argv[1], sys.argv[2]\n\
             connection = sqlite3.connect(db, timeout=30)\n\
             connection.execute(\n\
             \"INSERT INTO sessions(id, session_type, working_dir) \
             VALUES('wal-orphan', 'user', '/tmp')\")\n\
             connection.commit()\n\
             open(ready, 'w').write('ready')\n\
             time.sleep(120)\n",
            &[&path_str],
        );
        assert!(
            dir.path().join("sessions.db-shm").exists(),
            "the fixture premise: the holder mapped the index"
        );
        assert!(
            dir.path().join("sessions.db-wal").exists(),
            "the fixture premise: uncheckpointed frames"
        );
        std::fs::remove_file(dir.path().join("sessions.db-shm")).expect("the shm unlinks");

        let refusal = read_blocking(&path, "wal-orphan").expect_err("stale reads must refuse");
        assert!(
            !refusal.is_empty(),
            "the refusal carries a diagnostic, not silence"
        );
    }

    /// An absent store refuses WITHOUT creating anything: no db, no
    /// journal, no WAL, no SHM. A reader that conjured files beside a
    /// vendor store would corrupt Goose's own first-run layout.
    #[farhelm_testtrace::test]
    fn a_missing_store_refuses_and_creates_nothing() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let path = dir.path().join("sessions.db");

        read_blocking(&path, "any-id").expect_err("a missing store must refuse");
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .expect("the dir lists")
            .collect();
        assert!(
            leftovers.is_empty(),
            "the refused open creates no files at all"
        );
    }

    /// Garbage bytes, an empty database, a `sessions` VIEW, and a
    /// short `sessions` table all refuse: none of them is a store
    /// whose rows could authorize. Each shape gets its own file so a
    /// failure names the shape, not a row in a loop.
    #[farhelm_testtrace::test]
    fn non_store_shapes_refuse() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");

        let garbage = dir.path().join("garbage.db");
        std::fs::write(&garbage, b"this is not a sqlite database at all").expect("plant garbage");
        read_blocking(&garbage, "any-id").expect_err("garbage must refuse");

        let (empty, writer) = planted_store(&dir, "empty.db");
        drop(writer);
        read_blocking(&empty, "any-id").expect_err("an empty database must refuse");

        let (view, writer) = planted_store(&dir, "view.db");
        writer
            .execute_batch(
                "CREATE TABLE backing(id TEXT PRIMARY KEY);
                 CREATE VIEW sessions AS SELECT * FROM backing;",
            )
            .expect("plant the view");
        drop(writer);
        read_blocking(&view, "any-id").expect_err("a sessions view must refuse");

        let (short, writer) = planted_store(&dir, "short.db");
        writer
            .execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY);")
            .expect("plant the short table");
        drop(writer);
        read_blocking(&short, "any-id").expect_err("a short sessions table must refuse");
    }

    /// A `sessions` table without `parent_session_id` — the pre-15
    /// layout — refuses as an unsupported schema even though every
    /// other column matches: without the link column a root is
    /// indistinguishable from a child, so its rows can never
    /// authorize.
    #[farhelm_testtrace::test]
    fn the_pre_15_layout_without_a_parent_link_refuses() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        writer
            .execute_batch(
                "CREATE TABLE sessions(
                   id TEXT PRIMARY KEY,
                   session_type TEXT NOT NULL DEFAULT 'user',
                   working_dir TEXT NOT NULL
                 );
                 CREATE TABLE schema_version(version INTEGER PRIMARY KEY);
                 INSERT INTO schema_version(version) VALUES(15);",
            )
            .expect("plant the pre-15 layout");
        writer
            .execute(
                "INSERT INTO sessions(id, session_type, working_dir) VALUES('old-1', 'user', '/tmp')",
                [],
            )
            .expect("plant the old row");
        drop(writer);

        read_blocking(&path, "old-1").expect_err("a parent-less layout must refuse");
    }

    /// The version gate is exact: 15 and 17 refuse even with the
    /// pinned columns present, and a missing `schema_version` table
    /// refuses rather than assuming. A store whose version no audit
    /// covers authorizes nothing, in either direction.
    #[farhelm_testtrace::test]
    fn the_schema_version_gate_is_exactly_16() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        for (name, version) in [("v15.db", 15), ("v17.db", 17)] {
            let (path, writer) = planted_store(&dir, name);
            plant_goose_schema_16(&writer);
            writer
                .execute("UPDATE schema_version SET version = ?1", [version])
                .expect("restamp the version");
            plant_goose_session(&writer, "v-row", "user", None);
            drop(writer);
            read_blocking(&path, "v-row")
                .expect_err(&format!("schema version {version} must refuse"));
        }

        let (unversioned, writer) = planted_store(&dir, "unversioned.db");
        writer
            .execute_batch(
                "CREATE TABLE sessions(
                   id TEXT PRIMARY KEY,
                   session_type TEXT NOT NULL,
                   parent_session_id TEXT
                 );",
            )
            .expect("plant sessions without a version table");
        writer
            .execute(
                "INSERT INTO sessions(id, session_type) VALUES('nov-1', 'user')",
                [],
            )
            .expect("plant the row");
        drop(writer);
        read_blocking(&unversioned, "nov-1").expect_err("a version-less store must refuse");
    }

    /// The version gate is the MAXIMUM, not a row count: a store
    /// migrated from 15 to 16 carries both rows — observed live from
    /// Goose 1.50.1's own migration — and must still authorize, while
    /// a 17 anywhere in the history, an empty version table, a
    /// non-integer version, or a version history past its row bound
    /// all refuse. The gate pins which semantics wrote the row, not
    /// how the store got there.
    #[farhelm_testtrace::test]
    fn the_schema_version_gate_reads_the_maximum() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");

        let (migrated, writer) = planted_store(&dir, "migrated.db");
        plant_goose_schema_16(&writer);
        writer
            .execute("INSERT INTO schema_version(version) VALUES(15)", [])
            .expect("append the older migration row");
        plant_goose_session(&writer, "mig-1", "user", None);
        drop(writer);
        read_blocking(&migrated, "mig-1").expect("a (15, 16) history still authorizes");

        let (newer, writer) = planted_store(&dir, "newer.db");
        plant_goose_schema_16(&writer);
        writer
            .execute("INSERT INTO schema_version(version) VALUES(17)", [])
            .expect("append the newer migration row");
        plant_goose_session(&writer, "new-1", "user", None);
        drop(writer);
        read_blocking(&newer, "new-1").expect_err("a (16, 17) history must refuse");

        let (unversioned, writer) = planted_store(&dir, "empty-versions.db");
        plant_goose_schema_16(&writer);
        writer
            .execute("DELETE FROM schema_version", [])
            .expect("empty the version table");
        plant_goose_session(&writer, "emp-1", "user", None);
        drop(writer);
        read_blocking(&unversioned, "emp-1").expect_err("an empty version table must refuse");

        // A loosely-typed version table: the pinned DDL's
        // `INTEGER PRIMARY KEY` would reject the TEXT plant itself,
        // so the malformed shape builds its own version table while
        // the version probe — which assumes no DDL — still runs.
        let (textual, writer) = planted_store(&dir, "textual.db");
        writer
            .execute_batch(
                "CREATE TABLE sessions(
                   id TEXT PRIMARY KEY,
                   session_type TEXT NOT NULL,
                   working_dir TEXT NOT NULL,
                   parent_session_id TEXT
                 );
                 CREATE TABLE schema_version(version);",
            )
            .expect("plant the loose layout");
        writer
            .execute("INSERT INTO schema_version(version) VALUES('sixteen')", [])
            .expect("plant the non-integer version");
        plant_goose_session(&writer, "tex-1", "user", None);
        drop(writer);
        read_blocking(&textual, "tex-1").expect_err("a non-integer version must refuse");

        // The row bound, pinned exactly: 65 distinct versions with a
        // maximum of 16 refuses on COUNT, not on the maximum. No sane
        // migration history looks like this — Goose writes small
        // positive counters — so past-the-bound is malformed, refused.
        let (wide, writer) = planted_store(&dir, "wide.db");
        plant_goose_schema_16(&writer);
        writer
            .execute("DELETE FROM schema_version", [])
            .expect("clear the versions");
        for version in -48..=16 {
            writer
                .execute("INSERT INTO schema_version(version) VALUES(?1)", [version])
                .expect("append a history row");
        }
        plant_goose_session(&writer, "wide-1", "user", None);
        drop(writer);
        read_blocking(&wide, "wide-1").expect_err("an over-wide history must refuse");
    }

    /// The id gate is a primary-key lookup, not a scan: an unknown id
    /// refuses, and the row that reads back is byte-compared against
    /// the reported id before anything else is trusted about it.
    #[farhelm_testtrace::test]
    fn an_absent_row_refuses() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        plant_goose_session(&writer, "present-1", "user", None);
        drop(writer);

        read_blocking(&path, "absent-1").expect_err("an absent row must refuse");
        read_blocking(&path, "present-1").expect("the present row still reads");
    }

    /// Over-long values refuse as malformed rather than truncating
    /// into a different identity: a 33-byte type and a 129-byte
    /// parent link are both outside the contract.
    #[farhelm_testtrace::test]
    fn over_long_values_refuse() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        plant_goose_session(&writer, "long-type", &"t".repeat(33), None);
        plant_goose_session(&writer, "long-parent", "user", Some(&"p".repeat(129)));
        drop(writer);

        read_blocking(&path, "long-type").expect_err("an over-long type must refuse");
        read_blocking(&path, "long-parent").expect_err("an over-long parent must refuse");
        // The reported-id bound lives at the async entry, pinned by
        // `malformed_inputs_refuse_before_any_io`, not here.
    }

    /// A NULL `session_type` refuses at decode. The pinned DDL
    /// declares the column `NOT NULL`, so this fixture builds its
    /// own sessions table with a nullable type — same pinned names,
    /// types, and key — and plants the NULL there. The allowlist
    /// never sees a value the row did not hold, so the admission
    /// layer needs no NULL-type arm: the reader refuses first.
    #[farhelm_testtrace::test]
    fn a_null_session_type_refuses_at_decode() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        writer
            .execute_batch(
                "CREATE TABLE sessions(
                   id TEXT PRIMARY KEY,
                   session_type TEXT,
                   parent_session_id TEXT
                 );
                 CREATE TABLE schema_version(version INTEGER PRIMARY KEY);
                 INSERT INTO schema_version(version) VALUES(16);",
            )
            .expect("plant the nullable-type layout");
        writer
            .execute(
                "INSERT INTO sessions(id, session_type) VALUES('null-type', NULL)",
                [],
            )
            .expect("plant the NULL-typed row");
        drop(writer);

        read_blocking(&path, "null-type").expect_err("a NULL session type must refuse");
    }

    /// Non-UTF-8 storage refuses: a BLOB where the type string
    /// belongs is not a type the allowlist could ever match, and
    /// decoding it lossily would invent metadata the store never
    /// held.
    #[farhelm_testtrace::test]
    fn non_utf8_values_refuse() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        writer
            .execute(
                "INSERT INTO sessions(id, session_type, working_dir) VALUES('bin-1', x'ff', '/tmp')",
                [],
            )
            .expect("plant the non-UTF-8 row");
        drop(writer);

        read_blocking(&path, "bin-1").expect_err("a non-UTF-8 type must refuse");
    }

    /// A store whose writer holds `EXCLUSIVE` refuses WITHOUT
    /// waiting: the zero busy wait surfaces `BUSY` immediately
    /// instead of queueing behind the vendor. The 5 s bound is
    /// honest, not a tautology — it proves no 30 s-class wait, with
    /// two orders of magnitude of headroom over the millisecond-scale
    /// legitimate path, so machine load cannot flake it.
    ///
    /// The lock is `EXCLUSIVE`, not `RESERVED`, because readers never
    /// contend with `RESERVED` — POSIX byte-range locks make a read
    /// lock compatible with another process's reserved byte, which is
    /// the whole point of rollback-journal concurrency. `EXCLUSIVE`
    /// (held across the holder's open transaction) is the only state
    /// that makes a store busy FOR A READER. And the lock comes from
    /// the cross-process holder, because POSIX locks never contend
    /// within one process at all.
    #[farhelm_testtrace::test]
    fn a_busy_store_refuses_without_waiting() {
        let Some(()) = require_python_sqlite() else {
            return;
        };
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        plant_goose_session(&writer, "busy-1", "user", None);
        drop(writer);
        let path_str = path.to_str().expect("fixture paths are UTF-8").to_string();
        let _holder = spawn_holder(
            &dir,
            "busy_holder.py",
            "import sqlite3, sys, time\n\
             db, ready = sys.argv[1], sys.argv[2]\n\
             connection = sqlite3.connect(db, timeout=30)\n\
             connection.execute('BEGIN EXCLUSIVE')\n\
             connection.execute(\n\
             \"INSERT INTO sessions(id, session_type, working_dir) \
             VALUES('busy-holder', 'user', '/tmp')\")\n\
             open(ready, 'w').write('ready')\n\
             time.sleep(120)\n",
            &[&path_str],
        );

        let started = Instant::now();
        read_blocking(&path, "busy-1").expect_err("a busy store must refuse");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the refusal waits on nothing (took {:?})",
            started.elapsed()
        );
    }

    /// Nothing is cached across attempts: a row mutated between two
    /// reads reports its new metadata on the second read. A cached
    /// verdict would let a delegation that landed after the first
    /// read ride the earlier proof.
    #[farhelm_testtrace::test]
    fn consecutive_reads_see_vendor_mutations() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        plant_goose_session(&writer, "fresh-1", "user", None);

        let before = read_blocking(&path, "fresh-1").expect("the first read succeeds");
        assert_eq!(before.parent_session_id, None);
        writer
            .execute(
                "UPDATE sessions SET session_type = 'sub_agent', parent_session_id = 'p' WHERE id = 'fresh-1'",
                [],
            )
            .expect("the vendor mutates the row");
        let after = read_blocking(&path, "fresh-1").expect("the second read succeeds");
        assert_eq!(after.session_type, "sub_agent");
        assert_eq!(after.parent_session_id.as_deref(), Some("p"));
    }

    /// A read-only directory tree reads fine and stays byte-identical:
    /// the point of read-only is working where writers cannot, and a
    /// mode-555 tree turns any create attempt into a fatal error, so
    /// success proves zero attempts. Permissions restore before the
    /// scratch dir drops, or cleanup itself would fail.
    #[farhelm_testtrace::test]
    fn a_read_only_tree_reads_without_writing() {
        use std::os::unix::fs::PermissionsExt;

        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        plant_goose_session(&writer, "ro-1", "user", None);
        drop(writer);
        let hashed = std::fs::read(&path).expect("the store reads for hashing");

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444))
            .expect("the store locks read-only");
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555))
            .expect("the dir locks read-only");

        let outcome = read_blocking(&path, "ro-1");

        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755))
            .expect("the dir unlocks for cleanup");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("the store unlocks for cleanup");

        let metadata = outcome.expect("a read-only tree still reads");
        assert_eq!(metadata.session_type, "user");
        assert_eq!(
            std::fs::read(&path).expect("the store rereads"),
            hashed,
            "the read changes no byte"
        );
    }

    /// Inputs validate before the VFS is even consulted: relative and
    /// empty paths, NUL-bearing paths, over-long paths, and empty or
    /// over-long ids all refuse through the real async entry without
    /// touching disk.
    #[farhelm_testtrace::test]
    async fn malformed_inputs_refuse_before_any_io() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let over_long_path = format!("/{}", "p".repeat(4 * 1024));
        let with_nul = format!("{}\0evil", dir.path().display());
        for bad_path in [
            "relative/path.db",
            "",
            over_long_path.as_str(),
            with_nul.as_str(),
        ] {
            read_reported_session(bad_path, "some-id")
                .await
                .expect_err(&format!("{bad_path:?} must refuse"));
        }
        for bad_id in ["", "i".repeat(129).as_str()] {
            read_reported_session("/tmp/definitely-absent-goose-store.db", bad_id)
                .await
                .expect_err("the id bound must refuse");
        }
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .expect("the dir lists")
            .collect();
        assert!(
            leftovers.is_empty(),
            "validation refuses before any file exists"
        );
    }

    /// A store nested under `?`, `#`, and `%` reads fine: the URI
    /// builder percent-encodes the path so directory punctuation
    /// cannot escape into the query string.
    #[farhelm_testtrace::test]
    fn punctuation_in_the_path_stays_in_the_path() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let nested = dir.path().join("a?b#c%d");
        std::fs::create_dir(&nested).expect("the punctuated dir creates");
        let path = nested.join("sessions.db");
        let writer = rusqlite::Connection::open(&path).expect("the scratch store opens");
        plant_goose_schema_16(&writer);
        plant_goose_session(&writer, "punct-1", "user", None);
        drop(writer);

        let metadata = read_blocking(&path, "punct-1").expect("the punctuated path reads");
        assert_eq!(metadata.session_type, "user");
        assert_eq!(
            store_uri(path.to_str().expect("fixture paths are UTF-8")),
            format!(
                "file:{}?mode=ro&readonly_shm=1",
                path.to_str()
                    .expect("fixture paths are UTF-8")
                    .replace('%', "%25")
                    .replace('?', "%3F")
                    .replace('#', "%23")
            ),
            "the encoding is exactly the three escapes (percent first, or the escapes double-encode)"
        );
    }

    /// The schema probe is bounded in both dimensions: a 65-column
    /// table overruns the entry bound, and 64 wide-named columns
    /// overrun the byte bound. Without the caps a hostile schema
    /// could size the probe's work.
    #[farhelm_testtrace::test]
    fn over_wide_schemas_refuse() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");

        let (wide, writer) = planted_store(&dir, "wide.db");
        let mut ddl = String::from(
            "CREATE TABLE sessions(id TEXT PRIMARY KEY, session_type TEXT NOT NULL, parent_session_id TEXT",
        );
        for column in 0..62 {
            ddl.push_str(&format!(", extra_{column} TEXT"));
        }
        ddl.push_str(");");
        writer
            .execute_batch(&ddl)
            .expect("the 65-column table plants");
        writer
            .execute_batch(
                "CREATE TABLE schema_version(version INTEGER PRIMARY KEY);
                 INSERT INTO schema_version(version) VALUES(16);",
            )
            .expect("the version plants");
        drop(writer);
        read_blocking(&wide, "any-id").expect_err("65 columns must refuse");

        let (fat, writer) = planted_store(&dir, "fat.db");
        let mut ddl = String::from("CREATE TABLE sessions(id TEXT PRIMARY KEY");
        for column in 0..63 {
            ddl.push_str(&format!(", n{column:0>60} TEXT"));
        }
        ddl.push_str(");");
        writer
            .execute_batch(&ddl)
            .expect("the fat-named table plants");
        writer
            .execute_batch(
                "CREATE TABLE schema_version(version INTEGER PRIMARY KEY);
                 INSERT INTO schema_version(version) VALUES(16);",
            )
            .expect("the version plants");
        drop(writer);
        read_blocking(&fat, "any-id").expect_err("over-wide names must refuse");
    }

    /// The column probe is exact where the pinned DDL is exact: an
    /// uppercase `ID` is not the pinned `id`, and an `INTEGER`
    /// session type is not the pinned `TEXT`. Near-miss schemas
    /// refuse rather than reading under the wrong assumptions.
    #[farhelm_testtrace::test]
    fn near_miss_columns_refuse() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");

        let (cased, writer) = planted_store(&dir, "cased.db");
        writer
            .execute_batch(
                "CREATE TABLE sessions(
                   ID TEXT PRIMARY KEY,
                   session_type TEXT NOT NULL,
                   parent_session_id TEXT
                 );
                 CREATE TABLE schema_version(version INTEGER PRIMARY KEY);
                 INSERT INTO schema_version(version) VALUES(16);",
            )
            .expect("plant the cased table");
        drop(writer);
        read_blocking(&cased, "any-id").expect_err("an uppercase ID must refuse");

        let (typed, writer) = planted_store(&dir, "typed.db");
        writer
            .execute_batch(
                "CREATE TABLE sessions(
                   id TEXT PRIMARY KEY,
                   session_type INTEGER NOT NULL,
                   parent_session_id TEXT
                 );
                 CREATE TABLE schema_version(version INTEGER PRIMARY KEY);
                 INSERT INTO schema_version(version) VALUES(16);",
            )
            .expect("plant the mistyped table");
        drop(writer);
        read_blocking(&typed, "any-id").expect_err("an INTEGER session_type must refuse");
    }

    /// The VFS file-type gate admits exactly the three kinds one read
    /// needs — main db, main journal, WAL — and refuses every temp,
    /// transient, and sub/super journal kind, the untyped zero, and
    /// any combined type word.
    #[farhelm_testtrace::test]
    fn the_vfs_opens_only_what_one_read_needs() {
        assert!(goose_ro_file_type_allowed(ffi::SQLITE_OPEN_MAIN_DB));
        assert!(goose_ro_file_type_allowed(ffi::SQLITE_OPEN_MAIN_JOURNAL));
        assert!(goose_ro_file_type_allowed(ffi::SQLITE_OPEN_WAL));
        for denied in [
            0,
            ffi::SQLITE_OPEN_TEMP_DB,
            ffi::SQLITE_OPEN_TRANSIENT_DB,
            ffi::SQLITE_OPEN_TEMP_JOURNAL,
            ffi::SQLITE_OPEN_SUBJOURNAL,
            ffi::SQLITE_OPEN_SUPER_JOURNAL,
            ffi::SQLITE_OPEN_MAIN_DB | ffi::SQLITE_OPEN_TEMP_DB,
        ] {
            assert!(
                !goose_ro_file_type_allowed(denied),
                "type word {denied:#x} must refuse"
            );
        }
    }

    /// The file-control gate forwards exactly the harmless query
    /// opcodes and answers `NOTFOUND` to every mutating,
    /// fd-exposing, platform, and third-party control — plus anything
    /// unknown. Denying by default costs availability at worst (a
    /// refused read), never integrity.
    #[farhelm_testtrace::test]
    fn the_vfs_forwards_only_harmless_file_controls() {
        for allowed in [
            ffi::SQLITE_FCNTL_LOCKSTATE,
            ffi::SQLITE_FCNTL_LAST_ERRNO,
            ffi::SQLITE_FCNTL_VFSNAME,
            ffi::SQLITE_FCNTL_HAS_MOVED,
            ffi::SQLITE_FCNTL_SYNC,
            ffi::SQLITE_FCNTL_LOCK_TIMEOUT,
            ffi::SQLITE_FCNTL_DATA_VERSION,
            ffi::SQLITE_FCNTL_CKSM_FILE,
            ffi::SQLITE_FCNTL_TRACE,
            ffi::SQLITE_FCNTL_VFS_POINTER,
            ffi::SQLITE_FCNTL_FILESTAT,
        ] {
            assert!(
                goose_ro_file_control_allowed(allowed),
                "opcode {allowed} forwards"
            );
        }
        for denied in [
            ffi::SQLITE_FCNTL_SIZE_HINT,
            ffi::SQLITE_FCNTL_CHUNK_SIZE,
            ffi::SQLITE_FCNTL_FILE_POINTER,
            ffi::SQLITE_FCNTL_SYNC_OMITTED,
            ffi::SQLITE_FCNTL_PERSIST_WAL,
            ffi::SQLITE_FCNTL_OVERWRITE,
            ffi::SQLITE_FCNTL_POWERSAFE_OVERWRITE,
            ffi::SQLITE_FCNTL_PRAGMA,
            ffi::SQLITE_FCNTL_BUSYHANDLER,
            ffi::SQLITE_FCNTL_TEMPFILENAME,
            ffi::SQLITE_FCNTL_MMAP_SIZE,
            ffi::SQLITE_FCNTL_COMMIT_PHASETWO,
            ffi::SQLITE_FCNTL_WAL_BLOCK,
            ffi::SQLITE_FCNTL_ZIPVFS,
            ffi::SQLITE_FCNTL_RBU,
            ffi::SQLITE_FCNTL_JOURNAL_POINTER,
            ffi::SQLITE_FCNTL_BEGIN_ATOMIC_WRITE,
            ffi::SQLITE_FCNTL_COMMIT_ATOMIC_WRITE,
            ffi::SQLITE_FCNTL_ROLLBACK_ATOMIC_WRITE,
            ffi::SQLITE_FCNTL_SIZE_LIMIT,
            ffi::SQLITE_FCNTL_CKPT_DONE,
            ffi::SQLITE_FCNTL_RESERVE_BYTES,
            ffi::SQLITE_FCNTL_CKPT_START,
            ffi::SQLITE_FCNTL_EXTERNAL_READER,
            ffi::SQLITE_FCNTL_RESET_CACHE,
            999,
        ] {
            assert!(
                !goose_ro_file_control_allowed(denied),
                "opcode {denied} must refuse"
            );
        }
    }

    /// The unconditional refusals answer without touching their
    /// arguments: writes, truncates, and VFS-level deletes are
    /// `READONLY`, and a lock upgrade past `SHARED` is `READONLY`
    /// before any file is consulted. A lock AT `SHARED` on an
    /// unreachable file fails closed as `IOERR` — the trailer path
    /// refuses rather than forwarding blind.
    #[farhelm_testtrace::test]
    fn the_vfs_never_write_never_deletes_never_upgrades() {
        // SAFETY: the three unconditional refusals ignore their
        // arguments by construction, so NULL is a valid probe.
        unsafe {
            assert_eq!(
                goose_ro_write(std::ptr::null_mut(), std::ptr::null(), 0, 0),
                ffi::SQLITE_READONLY
            );
            assert_eq!(
                goose_ro_truncate(std::ptr::null_mut(), 0),
                ffi::SQLITE_READONLY
            );
            assert_eq!(
                goose_ro_delete(std::ptr::null_mut(), std::ptr::null(), 0),
                ffi::SQLITE_READONLY
            );
            assert_eq!(
                goose_ro_lock(std::ptr::null_mut(), ffi::SQLITE_LOCK_EXCLUSIVE),
                ffi::SQLITE_READONLY,
                "an upgrade refuses before any file is consulted"
            );
            assert_eq!(
                goose_ro_lock(std::ptr::null_mut(), ffi::SQLITE_LOCK_SHARED),
                ffi::SQLITE_IOERR,
                "an unreachable file fails closed, never forwards blind"
            );
        }
    }

    /// Releases the worker stall gate when the test ends, however it
    /// ends: parked workers are detached blocking threads, so a gate
    /// left set would linger them past the test (they hold no test
    /// resources, but there is no reason to leave them parked).
    struct UnparkOnDrop;

    impl Drop for UnparkOnDrop {
        fn drop(&mut self) {
            set_reader_test_park(false);
        }
    }

    /// A FIFO at the store path refuses WITHOUT blocking, through the
    /// async entry: the descriptor-free `metadata` gate refuses
    /// before SQLite ever opens the path — and opens nothing
    /// itself, so no close can disturb a concurrent reader's POSIX
    /// locks — which means the `O_RDONLY`-waits-for-a-writer stall
    /// never happens. The refusal lands well under the stall
    /// backstop, which proves it never reached the worker wait at
    /// all — the backstop is the competing mechanism this
    /// distinguishes.
    ///
    /// Why this test matters: the progress handler cannot interrupt
    /// a blocked open, so without the gate a FIFO path stalls the
    /// admitting caller past every documented bound while holding
    /// its capture claim.
    #[farhelm_testtrace::test]
    async fn a_fifo_store_path_refuses_without_blocking() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let fifo = dir.path().join("sessions.db");
        let c_fifo = std::ffi::CString::new(fifo.to_str().expect("fixture paths are UTF-8"))
            .expect("the fifo path encodes");
        // SAFETY: mkfifo on a fresh path inside the fixture tempdir.
        assert_eq!(unsafe { libc::mkfifo(c_fifo.as_ptr(), 0o600) }, 0);
        assert!(
            fifo.exists(),
            "the fifo premise holds before the refusal is measured"
        );

        let started = Instant::now();
        let refusal =
            read_reported_session(fifo.to_str().expect("fixture paths are UTF-8"), "parent-1")
                .await
                .expect_err("a FIFO at the store path refuses");
        assert_eq!(
            refusal.reason, "the store path is not a regular file",
            "the gate names the shape, not a downstream open failure"
        );
        assert!(
            started.elapsed() < READER_STALL_BACKSTOP,
            "the refusal never reaches the backstop wait: {:?}",
            started.elapsed()
        );
    }

    /// The gate itself names every shape it can meet: a planted
    /// store passes, a FIFO and a directory refuse as non-regular,
    /// and a missing path refuses without creating anything. This
    /// pins the unit the worker calls — including the exact reasons
    /// the async tests observe only through the worker — so a future
    /// edit that remaps a reason fails here, at the gate, rather
    /// than surfacing as a changed refusal downstream.
    ///
    /// Why this test matters: the FIFO refusal is the stall story —
    /// without the gate the worker would block on the open — and
    /// the missing-path case proves the stat creates nothing beside
    /// the vendor store.
    ///
    /// What this does NOT cover: whether the gate takes a
    /// descriptor. An open/fstat/drop preflight passes every
    /// assertion here, so descriptor reintroduction is invisible to
    /// this test. That property is pinned by
    /// `the_preflight_preserves_a_held_read_lock` below instead;
    /// the descriptor-free shape itself is structural (`metadata`
    /// takes no fd to close).
    #[farhelm_testtrace::test]
    fn the_regular_file_gate_names_every_shape() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        drop(writer);
        require_regular_file(path.to_str().expect("fixture paths are UTF-8"))
            .expect("a planted store passes the gate");

        let fifo = dir.path().join("pipe.db");
        let c_fifo = std::ffi::CString::new(fifo.to_str().expect("fixture paths are UTF-8"))
            .expect("the fifo path encodes");
        // SAFETY: mkfifo on a fresh path inside the fixture tempdir.
        assert_eq!(unsafe { libc::mkfifo(c_fifo.as_ptr(), 0o600) }, 0);
        assert!(
            fifo.exists(),
            "the fifo premise holds before the gate is measured"
        );
        assert_eq!(
            require_regular_file(fifo.to_str().expect("fixture paths are UTF-8"))
                .expect_err("a FIFO must refuse")
                .reason,
            "the store path is not a regular file",
            "the gate names the shape, not a downstream open failure"
        );

        let subdir = dir.path().join("subdir");
        std::fs::create_dir(&subdir).expect("the subdir creates");
        assert_eq!(
            require_regular_file(subdir.to_str().expect("fixture paths are UTF-8"))
                .expect_err("a directory must refuse")
                .reason,
            "the store path is not a regular file",
            "a directory is not a store either"
        );

        let missing = dir.path().join("absent.db");
        assert!(
            !missing.exists(),
            "the missing premise holds before the gate is measured"
        );
        require_regular_file(missing.to_str().expect("fixture paths are UTF-8"))
            .expect_err("a missing path must refuse");
        assert!(
            !missing.exists(),
            "the refused stat creates nothing beside the vendor store"
        );
    }

    /// One synchronous exclusive-lock probe from an owned external
    /// process: `BEGIN EXCLUSIVE` with zero busy wait, printing
    /// `locked` when the store is busy and `acquired` (then rolling
    /// back, so the probe leaves nothing behind) when it is free. A
    /// fresh process per probe, so no lock state leaks between
    /// attempts. Anything else — a missing interpreter, an
    /// unexpected SQLite error — exits nonzero with its traceback,
    /// failing the test closed instead of reporting a verdict.
    fn exclusive_attempt(store_path: &str) -> String {
        let probe = std::process::Command::new("python3")
            .arg("-c")
            .arg(
                "import sqlite3, sys
db = sys.argv[1]
connection = sqlite3.connect(db, timeout=0, isolation_level=None)
try:
    connection.execute(\"BEGIN EXCLUSIVE\")
except sqlite3.OperationalError as error:
    if \"locked\" not in str(error):
        raise
    print(\"locked\")
else:
    print(\"acquired\")
    connection.execute(\"ROLLBACK\")
",
            )
            .arg(store_path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .expect("the lock probe runs");
        assert!(
            probe.status.success(),
            "the lock probe exits cleanly: {}",
            String::from_utf8_lossy(&probe.stderr)
        );
        String::from_utf8(probe.stdout)
            .expect("the lock probe prints UTF-8")
            .trim()
            .to_string()
    }

    /// The preflight preserves a `SHARED` lock this process already
    /// holds: a read transaction opens on the planted store and stays
    /// holding, the gate runs, and an owned external process attempts
    /// `BEGIN EXCLUSIVE` — refused while the read is held, acquired
    /// once it commits. An open/fstat/drop preflight fails here: its
    /// close releases the process's `SHARED` lock, so the mid-hold
    /// attempt acquires instead of refusing. No scheduling luck is
    /// involved — POSIX locks are deterministic, and every step runs
    /// in order on this thread.
    ///
    /// Why this test matters: this is the regression pin for the
    /// descriptor-free gate, the one the shape tests above cannot
    /// supply. POSIX locks never contend within one process, so the
    /// contender has to be an owned external process, and the store
    /// stays in rollback-journal mode on purpose — only there does a
    /// held `SHARED` lock refuse an `EXCLUSIVE` attempt at all. The
    /// held lock across the spawn is the deliberate instrument here,
    /// not leaked fixture state: the read transaction commits before
    /// the test ends.
    #[farhelm_testtrace::test]
    fn the_preflight_preserves_a_held_read_lock() {
        let Some(()) = require_python_sqlite() else {
            return;
        };
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        plant_goose_session(&writer, "lock-1", "user", None);
        let journal: String = writer
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("the journal mode reads");
        assert_eq!(
            journal, "delete",
            "the fixture premise: rollback-journal locking, where SHARED refuses EXCLUSIVE"
        );
        drop(writer);

        let path_str = path.to_str().expect("fixture paths are UTF-8");
        let reader = rusqlite::Connection::open(path_str).expect("the read side opens");
        reader
            .execute_batch("BEGIN")
            .expect("the read transaction begins");
        let seen: String = reader
            .query_row(
                "SELECT session_type FROM sessions WHERE id = 'lock-1'",
                [],
                |row| row.get(0),
            )
            .expect("the held transaction reads the planted row");
        assert_eq!(
            seen, "user",
            "the fixture premise: the read transaction is live and holding SHARED"
        );

        // The action under test: the gate passes the planted store
        // while taking no descriptor whose close would release the
        // held lock.
        require_regular_file(path_str).expect("the gate passes a planted store");

        assert_eq!(
            exclusive_attempt(path_str),
            "locked",
            "an EXCLUSIVE attempt refuses while the read transaction is held"
        );

        reader
            .execute_batch("COMMIT")
            .expect("the read transaction releases");
        assert_eq!(
            exclusive_attempt(path_str),
            "acquired",
            "the same attempt succeeds once the read transaction is gone"
        );
    }

    /// Two reads of ONE database, issued together, both return
    /// correct metadata through the production async entry:
    /// different sessions share one Goose database, so repeated
    /// reads of the same file are the normal case rather than a
    /// corner. Both calls go through validation, a reader slot, and
    /// the blocking worker against the same planted row; both must
    /// see it, with no refusal and no cross-talk.
    ///
    /// Why this test matters: it smokes the path the fix touched —
    /// gate, worker, and backstop — end to end through the async
    /// entry a real admission uses.
    ///
    /// What this does NOT cover: genuine overlap. `tokio::join!`
    /// permits concurrent futures but never guarantees both workers
    /// hold a SQLite transaction lock at once — one may finish
    /// before the other begins — and with no external contender a
    /// lock release would be unobservable in-process anyway (POSIX
    /// locks never contend within one process). Lock preservation
    /// across the preflight is pinned by
    /// `the_preflight_preserves_a_held_read_lock` above instead.
    #[farhelm_testtrace::test]
    async fn concurrent_reads_of_one_database_stay_correct() {
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let (path, writer) = planted_store(&dir, "sessions.db");
        plant_goose_schema_16(&writer);
        plant_goose_session(&writer, "shared-1", "user", None);
        drop(writer);
        let path_str = path.to_str().expect("fixture paths are UTF-8");

        let (one, two) = tokio::join!(
            read_reported_session(path_str, "shared-1"),
            read_reported_session(path_str, "shared-1"),
        );
        for outcome in [one, two] {
            let metadata = outcome.expect("an overlapping read of one database succeeds");
            assert_eq!(metadata.session_type, "user");
            assert_eq!(metadata.parent_session_id, None);
        }
    }

    /// A worker stalled past the backstop still refuses on time, and
    /// the stalled work keeps occupying its reader slot: two parked
    /// workers refuse with the overrun reason at the backstop
    /// (proving the timeout does not rejoin them — the old code
    /// awaited here and never returned), and a third read made while
    /// both are still parked is saturated (proving their permits
    /// traveled with them instead of being released by the timeout).
    ///
    /// Why this test matters: rejoining the worker stalls the
    /// admitting caller past every documented bound while holding
    /// its capture claim, and releasing its permit lets later calls
    /// exceed the two-worker bound the module documents.
    #[farhelm_testtrace::test]
    async fn a_stalled_worker_refuses_on_time_and_keeps_its_slot() {
        let _unpark = UnparkOnDrop;
        // Set before either worker exists: a worker parks whenever
        // its thread starts, so no readiness wait is needed and none
        // could be written without racing the scheduler.
        set_reader_test_park(true);
        let dir = farhelm_teststate::tempdir().expect("scratch dir");
        let first = dir.path().join("a.db");
        let second = dir.path().join("b.db");
        let started = Instant::now();
        let (one, two) = tokio::join!(
            read_reported_session(first.to_str().expect("fixture paths are UTF-8"), "a"),
            read_reported_session(second.to_str().expect("fixture paths are UTF-8"), "b"),
        );
        let elapsed = started.elapsed();
        for outcome in [one, two] {
            let refusal = outcome.expect_err("a stalled worker refuses");
            assert_eq!(
                refusal.reason, "the reader overran its budget",
                "the backstop refuses, it does not rejoin the worker"
            );
        }
        assert!(
            elapsed >= READER_STALL_BACKSTOP,
            "the refusal comes from the backstop, not from a fast path: {elapsed:?}",
        );
        assert!(
            elapsed < Duration::from_secs(30),
            "the refusal is timely despite the stalled workers: {elapsed:?}",
        );
        // Both workers are still parked — the gate is still set — so
        // both permits are still held: the third read must observe
        // saturation rather than a freed slot.
        let saturated = read_reported_session(
            dir.path()
                .join("c.db")
                .to_str()
                .expect("fixture paths are UTF-8"),
            "c",
        )
        .await
        .expect_err("outstanding workers keep their slots");
        assert_eq!(
            saturated.reason, "the reader is saturated",
            "a third read while two workers are stalled rejects, never queues"
        );
    }
}

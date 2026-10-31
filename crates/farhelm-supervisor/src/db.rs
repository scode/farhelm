//! The SQLite plumbing both Farhelm stores share: how a private database file
//! is opened, and how an async method runs one synchronous piece of work on
//! the shared connection.
//!
//! The supervisor's `SessionStore` and the helm's `HelmStore` each keep one
//! `rusqlite::Connection` behind a mutex and reach it from async code through
//! `spawn_blocking`, because SQLite calls block. Each store used to spell that
//! out at every method (clone the `Arc`, spawn, lock, name the panic), about
//! fifty times apiece, and each opened its file with its own copy of the same
//! flags, permission fix, and busy timeout. What stays in each store is what
//! genuinely differs: its schema, its migrations, and per-connection pragmas
//! such as the helm's `foreign_keys`. Journal conversion and sync policy stay
//! shared, with conversion authority supplied by the state-directory owner.

use anyhow::Context;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// How long a query waits on `SQLITE_BUSY` before giving up.
///
/// Two processes briefly holding the same database is the normal shape of a
/// handoff restart (the old supervisor still running while the new one
/// constructs; a `farhelm helm` CLI command beside a running helm). Without a
/// busy timeout SQLite fails the loser of that overlap immediately; a bounded
/// wait lets its request go through once the winner's transaction releases the
/// lock, at the cost of stalling that one call for up to this long in the
/// pathological case.
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Open (creating if needed) a database file only this user can read, with the
/// shared busy timeout. Blocking; call it from `spawn_blocking`.
///
/// The caller must already have made the parent directory private (mode
/// 0700, as `ensure_private_dir` does for the state directory). That, not the
/// chmod below, is the confidentiality boundary: SQLite creates the file
/// before this function can narrow its mode, so under a permissive umask a
/// newly created file is briefly readable by anyone who can reach the parent
/// directory. The mode-0600 fix repairs the file's own permissions; it cannot
/// close that window.
///
/// Explicit flags rather than `Connection::open`'s defaults. `SQLITE_OPEN_URI`
/// is deliberately absent: with it SQLite would interpret a path starting with
/// `file:` as a URI (`?mode=...` and all). The state directory is fixed by this
/// process, not attacker input, but URI mode is not a feature either store
/// wants, so it is left out rather than relied upon to stay harmless.
/// `SQLITE_OPEN_NO_MUTEX` matches `Connection::open`'s own default: every
/// access is already serialized through [`Db`]'s mutex, so SQLite's internal
/// connection mutex would be redundant locking.
///
/// The file is forced to mode 0600 on every open, not only at creation, so a
/// database left more permissive by an older build or a copy is tightened
/// before anything is read from it. `label` names the database in errors
/// ("session database", "helm database").
/// Store openers then call [`configure_durability`] with their migration
/// authority before beginning schema work.
pub fn open_private(path: &Path, label: &str) -> anyhow::Result<Connection> {
    open_private_with(path, label, OpenFlags::SQLITE_OPEN_CREATE)
}

/// [`open_private`] for a database that must already exist: identical
/// except that `SQLITE_OPEN_CREATE` is omitted, so an absent file is an
/// error rather than a fresh, empty database.
///
/// For callers whose whole point is to act on state someone else created,
/// where a mistyped path answered with a new empty database would report
/// success while the real one is untouched (offline `token rotate` is the
/// case that needed it). The flag omission is the guarantee; a caller that
/// wants a friendlier message for the absent case checks first, but that
/// check alone would race the file's deletion.
pub fn open_private_existing(path: &Path, label: &str) -> anyhow::Result<Connection> {
    open_private_with(path, label, OpenFlags::empty())
}

/// Share file-opening policy without guessing whether this caller owns migration.
/// The stores supply that authority separately to [`configure_durability`].
fn open_private_with(path: &Path, label: &str, create: OpenFlags) -> anyhow::Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | create | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening {label} {}", path.display()))?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("restricting mode of {}", path.display()))?;
    }
    conn.busy_timeout(BUSY_TIMEOUT)
        .context("setting sqlite busy timeout")?;
    Ok(conn)
}

/// Configure commit durability before either store starts schema work.
///
/// Only an owning open may change the file's persistent journal mode. Every
/// ordinary read-write connection still needs its own NORMAL sync setting
/// once the file is actually in WAL mode. A refused conversion leaves FULL
/// intact: NORMAL with a rollback journal would weaken crash consistency,
/// rather than only allowing the accepted loss of the last committed writes.
/// Call outside a transaction, before schema migrations or initialization.
pub fn configure_durability(
    conn: &Connection,
    label: &str,
    may_migrate: bool,
) -> anyhow::Result<()> {
    if may_migrate {
        try_enable_wal(conn, label);
    }
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .context("reading sqlite journal mode")?;
    if mode.eq_ignore_ascii_case("wal") {
        conn.pragma_update(None, "synchronous", "NORMAL")
            .context("setting sqlite WAL synchronous mode")?;
    }
    Ok(())
}

/// Attempt the persistent conversion once; contention must not prevent startup.
///
/// A handoff can overlap an older process's transaction. SQLite may wait up to
/// the connection's busy timeout here, adding that much to startup. A later
/// owning open tries again; there is no retry task or checkpoint policy.
fn try_enable_wal(conn: &Connection, label: &str) {
    match conn.query_row("PRAGMA journal_mode=WAL", [], |row| row.get::<_, String>(0)) {
        Ok(mode) if mode.eq_ignore_ascii_case("wal") => {}
        Ok(mode) => tracing::warn!(
            database = label,
            journal_mode = %mode,
            "sqlite WAL conversion was not applied; keeping the current journal mode"
        ),
        Err(error) => tracing::warn!(
            database = label,
            ?error,
            "sqlite WAL conversion failed; keeping the current journal mode"
        ),
    }
}

/// Make a short write power-loss durable without changing ordinary WAL writes.
///
/// Call while holding the store's connection mutex, outside any transaction.
/// The mutex must cover this entire call: synchronous is connection-local, so
/// another operation must not slip between selecting FULL and restoring the
/// previous setting. WAL requires FULL; rollback journaling requires EXTRA
/// to sync removal of its journal as well. Stronger settings are never weakened.
///
/// `work` must finish or drop every transaction before returning. Its commits
/// must succeed before it returns a value that another component can depend
/// on. The previous setting is restored on both success and error; restoration
/// failure also returns an error, even if the write itself committed. Panics
/// retain Db's ordinary poisoned-mutex behavior rather than returning success.
pub fn with_full_sync<T>(
    conn: &mut Connection,
    work: impl FnOnce(&mut Connection) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    anyhow::ensure!(
        conn.is_autocommit(),
        "FULL-synchronous work must start outside a transaction"
    );
    let previous: i64 = conn
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .context("reading sqlite synchronous mode before durable write")?;
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .context("reading sqlite journal mode before durable write")?;
    let required = if mode.eq_ignore_ascii_case("wal") {
        2
    } else {
        3
    };
    if previous >= required {
        return work(conn);
    }
    conn.pragma_update(None, "synchronous", required)
        .context("selecting sqlite synchronous mode for durable write")?;
    let result = work(conn);
    let restored = conn
        .pragma_update(None, "synchronous", previous)
        .context("restoring sqlite synchronous mode after durable write");
    match (result, restored) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(restore_error)) => Err(error.context(format!(
            "also failed to restore sqlite synchronous mode: {restore_error:#}"
        ))),
    }
}

/// One store's connection, shared by its async methods.
///
/// Cloning shares the connection. Every use holds the mutex only inside one
/// synchronous closure, so no lock is ever held across an await.
#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
    /// Names this database in the poisoned-mutex panic, so a crash report says
    /// which store's earlier panic poisoned it.
    label: &'static str,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db")
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

impl Db {
    /// Wrap an opened connection. `label` names the database in a poisoned
    /// mutex panic ("session db", "helm db").
    pub fn new(conn: Connection, label: &'static str) -> Db {
        Db {
            conn: Arc::new(Mutex::new(conn)),
            label,
        }
    }

    /// Run `work` on the connection on the blocking pool and return its
    /// result.
    ///
    /// `panic_context` is attached if the blocking task itself panics (it
    /// should name the operation, like "web token read task panicked"); errors
    /// `work` returns come back unchanged. A poisoned mutex (an earlier panic
    /// while it was held) panics here too, as every store method always has:
    /// the connection's state is unknown, and continuing on it would be worse
    /// than stopping.
    pub async fn call<T, F>(&self, panic_context: &'static str, work: F) -> anyhow::Result<T>
    where
        F: FnOnce(&mut Connection) -> anyhow::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.conn.clone();
        let label = self.label;
        tokio::task::spawn_blocking(move || {
            let mut guard = conn
                .lock()
                .unwrap_or_else(|_| panic!("{label} mutex poisoned"));
            work(&mut guard)
        })
        .await
        .context(panic_context)?
    }

    /// Lock the connection directly, for synchronous callers that already run
    /// off the async executor (tests, and code inside another blocking task).
    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn
            .lock()
            .unwrap_or_else(|_| panic!("{} mutex poisoned", self.label))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read both settings so a test cannot mistake WAL's persistent flag for
    /// the per-connection sync policy that must be renewed on every open.
    fn durability(conn: &Connection) -> (String, i64) {
        (
            conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))
                .unwrap(),
            conn.query_row("PRAGMA synchronous", [], |row| row.get(0))
                .unwrap(),
        )
    }

    /// A rare durable write must not downgrade a connection already configured
    /// for FULL or EXTRA, including when the operation returns an error.
    #[test]
    fn full_sync_preserves_stronger_existing_settings() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = open_private(&dir.path().join("store.db"), "test database").unwrap();
        configure_durability(&conn, "test database", true).unwrap();
        assert_eq!(durability(&conn), ("wal".into(), 1));
        for mode in [2, 3] {
            conn.pragma_update(None, "synchronous", mode).unwrap();
            for fail in [false, true] {
                let result = with_full_sync(&mut conn, |conn| {
                    let during: i64 = conn.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
                    assert_eq!(during, mode);
                    anyhow::ensure!(!fail, "scripted write refusal");
                    Ok(())
                });
                assert_eq!(result.is_err(), fail);
                assert_eq!(durability(&conn).1, mode);
            }
        }
    }

    /// A busy conversion can leave the store using a rollback journal. Its
    /// durable write needs EXTRA, which syncs journal removal; FULL alone
    /// does not provide that directory-entry guarantee in DELETE mode.
    #[test]
    fn full_sync_uses_extra_for_rollback_and_restores_previous_mode() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = open_private(&dir.path().join("store.db"), "test database").unwrap();
        conn.execute_batch("CREATE TABLE observed(value INTEGER)")
            .unwrap();
        assert_eq!(durability(&conn), ("delete".into(), 2));
        for fail in [false, true] {
            let result = with_full_sync(&mut conn, |conn| {
                conn.execute(
                    "INSERT INTO observed SELECT synchronous FROM pragma_synchronous",
                    [],
                )?;
                anyhow::ensure!(!fail, "scripted write refusal");
                Ok(())
            });
            assert_eq!(result.is_err(), fail);
            assert_eq!(durability(&conn), ("delete".into(), 2));
        }
        let observed: Vec<i64> = conn
            .prepare("SELECT value FROM observed")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(observed, vec![3, 3]);
    }

    /// Owning opens convert both a new file and existing rollback data in
    /// place. Conversion must preserve rows, and a later non-owning connection
    /// must select NORMAL for itself without needing conversion authority.
    #[test]
    fn owning_open_uses_wal_normal_and_preserves_existing_rows() {
        for existing in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("store.db");
            if existing {
                let seed = Connection::open(&path).unwrap();
                seed.execute_batch("CREATE TABLE saved(value); INSERT INTO saved VALUES (7)")
                    .unwrap();
                assert_eq!(durability(&seed), ("delete".into(), 2));
            }
            let owner = open_private(&path, "test database").unwrap();
            configure_durability(&owner, "test database", true).unwrap();
            assert_eq!(durability(&owner), ("wal".into(), 1));
            if existing {
                assert_eq!(
                    owner
                        .query_row("SELECT value FROM saved", [], |row| row.get::<_, i64>(0))
                        .unwrap(),
                    7
                );
            }
            let reader = open_private_existing(&path, "test database").unwrap();
            assert_eq!(durability(&reader), ("wal".into(), 2));
            configure_durability(&reader, "test database", false).unwrap();
            assert_eq!(durability(&reader), ("wal".into(), 1));
        }
    }

    /// A non-owner beside an old process leaves rollback journaling and FULL
    /// sync untouched; it has no authority to change the shared file format.
    #[test]
    fn non_owning_open_keeps_rollback_full() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_private(&dir.path().join("store.db"), "test database").unwrap();
        assert_eq!(durability(&conn), ("delete".into(), 2));
        configure_durability(&conn, "test database", false).unwrap();
        assert_eq!(durability(&conn), ("delete".into(), 2));
    }

    /// SQLite can return an unchanged mode without an error. An in-memory
    /// database exercises that real result and must never acquire NORMAL just
    /// because the conversion statement itself succeeded.
    #[farhelm_testtrace::test]
    fn unchanged_journal_mode_warns_and_keeps_full() {
        let conn = Connection::open_in_memory().unwrap();
        configure_durability(&conn, "memory test database", true).unwrap();
        assert_eq!(durability(&conn), ("memory".into(), 2));
        let warnings = farhelm_testtrace::current_capture()
            .unwrap()
            .matching("sqlite WAL conversion was not applied")
            .unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].level, "WARN");
        assert_eq!(warnings[0].fields["journal_mode"], "memory");
    }

    /// An active old reader prevents WAL conversion with the bundled SQLite.
    /// Startup still succeeds at FULL, warns about the actual busy result,
    /// and a subsequent owner converts after that transaction releases.
    #[farhelm_testtrace::test]
    fn busy_wal_conversion_warns_and_keeps_rollback_full() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.db");
        let blocker = Connection::open(&path).unwrap();
        blocker
            .execute_batch("CREATE TABLE saved(value); INSERT INTO saved VALUES (7); BEGIN")
            .unwrap();
        assert_eq!(
            blocker
                .query_row("SELECT value FROM saved", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            7,
            "the old connection must hold a real read snapshot"
        );
        assert!(!blocker.is_autocommit());
        let conn = open_private(&path, "busy test database").unwrap();
        conn.busy_timeout(Duration::from_millis(1)).unwrap();
        configure_durability(&conn, "busy test database", true).unwrap();
        assert_eq!(durability(&conn), ("delete".into(), 2));
        let warnings = farhelm_testtrace::current_capture()
            .unwrap()
            .matching("sqlite WAL conversion failed")
            .unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].level, "WARN");
        assert!(
            warnings[0].fields["error"].contains("DatabaseBusy"),
            "{warnings:?}"
        );
        assert_eq!(warnings[0].fields["database"], "busy test database");

        blocker.execute_batch("ROLLBACK").unwrap();
        configure_durability(&conn, "busy test database", true).unwrap();
        assert_eq!(durability(&conn), ("wal".into(), 1));
    }
}

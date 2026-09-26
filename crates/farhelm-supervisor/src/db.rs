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
//! such as the helm's `foreign_keys`.

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
pub fn open_private(path: &Path, label: &str) -> anyhow::Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
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

//! SQLite session persistence: the durable half of the split introduced
//! by M2 (PLAN_M2.md, "Supervisor SQLite"). tmux stays the truth for
//! whether a session's terminal is alive; this module is the truth for
//! the coarser fact that a session exists at all and what its metadata
//! is, which must survive the supervisor process even though tmux (or
//! the whole host) might not have restarted alongside it.
//!
//! Schema versioning uses SQLite's `PRAGMA user_version` rather than a
//! separate version-tracking table. A version table was considered and
//! rejected: until a migration needs its own per-step metadata (applied
//! timestamps, checksums, ...), the pragma is atomic with the database
//! file itself, needs no bootstrap ordering (there is no chicken-and-egg
//! "which table records that the version table exists"), and — unlike a
//! table — cannot itself be missing from an otherwise-valid database.
//!
//! Journal mode and synchronous pragmas are left at SQLite's defaults.
//! M3 is where PLAN.md places the explicit crash-safety/atomicity policy
//! for the state store; this module does not invent one ahead of that.

use anyhow::Context;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How long a query waits on `SQLITE_BUSY` before giving up.
///
/// A brief window where two supervisor processes both hold this database
/// open is the normal shape of a handoff restart (the old process still
/// running while the new one constructs — see `Supervisor::serve`'s
/// second `reload_sessions` call). Without a busy timeout, SQLite returns
/// `SQLITE_BUSY` to the loser of that overlap immediately, turning an
/// ordinary handoff into a spurious open/query failure; a bounded wait
/// instead lets the loser's request go through once the winner's
/// transaction releases the lock, at the cost of stalling that one call
/// for up to this long in the pathological case.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The `sessions` table's current shape. Bumping this requires a matching
/// migration step in `apply_schema` — there is exactly one version so far,
/// so no migration path exists yet.
const SCHEMA_VERSION: i64 = 1;

/// The stored fields the supervisor actually consumes: wire metadata plus
/// the tmux handles a session was created with.
///
/// Not the whole `sessions` row — `created_at` is write-only from this
/// type's perspective. `insert_session` fills it in itself (see
/// `now_unix`) rather than accepting it here, and nothing yet reads it
/// back (it exists for a human inspecting the database, and as a schema
/// field a future migration can build on); adding a field to this struct
/// for it would invite call sites to treat an informational timestamp as
/// load-bearing.
///
/// This is the store's own type rather than a reuse of `SessionInfo`
/// (the wire type) or `service::SessionEntry` (the live in-memory type):
/// the database additionally needs `tmux_name`/`pane`, which `SessionInfo`
/// has no reason to carry over the wire, and must not depend on
/// `SessionEntry`'s shape, which is free to keep evolving (e.g. gaining
/// the restart-gap `terminal: Option<Terminal>` field) independently of
/// what is stored on disk.
///
/// Deliberately missing: `SessionInfo::status`. Liveness is never
/// persisted — tmux is its only truth (module docs above), and a status
/// written at some past moment would be stale the instant the process it
/// described changed state, with nothing to invalidate it on the way
/// back out of SQLite. `service::Supervisor::reload_sessions` always
/// recomputes a freshly loaded row's terminal from a live `has_session`
/// probe, and `ListSessions` recomputes `status` itself on every reply
/// (`service::session_status`) — a persisted status column would be
/// redundant at best and actively misleading at worst.
#[derive(Debug, Clone)]
pub struct StoredSession {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub invocation: String,
    pub tmux_name: String,
    pub pane: String,
}

/// The supervisor's session database.
///
/// Wraps the connection in `Arc<Mutex<..>>` (a std, not tokio, mutex —
/// every hold is confined to a single synchronous `spawn_blocking`
/// closure, so there is never an await point inside the critical
/// section) so the store can be cloned into request handlers freely while
/// every actual query still runs serialized against the one connection.
/// rusqlite calls are synchronous, and a commit's fsync can block for
/// real disk-flush time; running them inline on an async worker thread
/// would stall that thread's entire share of the runtime — every other
/// session's terminal forwarding included — for the duration of one
/// session's write. `spawn_blocking` is what keeps that cost off the
/// async workers.
#[derive(Clone, Debug)]
pub struct SessionStore {
    conn: Arc<Mutex<Connection>>,
}

/// Create the `sessions` table and stamp `user_version` when the database
/// is fresh (`user_version` reads 0, SQLite's default for a database that
/// has never set it). A `user_version` already at [`SCHEMA_VERSION`] is
/// left untouched — the table is assumed to match, since this build wrote
/// it. Any other value means a schema this build does not understand
/// (a downgrade, or a future migration this build predates), which is
/// refused rather than silently misread.
fn apply_schema(conn: &Connection) -> anyhow::Result<()> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .context("reading schema version")?;
    match version {
        0 => {
            conn.execute_batch(
                "CREATE TABLE sessions (
                    id         TEXT PRIMARY KEY,
                    title      TEXT NOT NULL,
                    cwd        TEXT NOT NULL,
                    invocation TEXT NOT NULL,
                    tmux_name  TEXT NOT NULL UNIQUE,
                    pane       TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                ) STRICT;",
            )
            .context("creating sessions table")?;
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)
                .context("stamping schema version")?;
            Ok(())
        }
        SCHEMA_VERSION => Ok(()),
        other => anyhow::bail!(
            "supervisor.db has schema version {other}, but this build only understands \
             version {SCHEMA_VERSION}; refusing to open it rather than risk misreading it"
        ),
    }
}

/// Seconds since the Unix epoch, for `created_at`'s informational
/// timestamp. Never fails the caller over a clock reading: `created_at`
/// is documented (see the schema) as informational only, nothing in this
/// module's own logic depends on it, so a pre-epoch system clock — the
/// only way `duration_since` errors — degrades to `0` instead of
/// rejecting an otherwise-successful session creation.
fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

impl SessionStore {
    /// Open (or create) the database at `path`, applying the schema if it
    /// is fresh.
    ///
    /// Confidentiality of the rows this stores (an invocation may embed
    /// credentials passed on an agent's command line, exactly like the
    /// launch specs in `crate::write_private_file`) rests on the state
    /// directory's 0700 mode (`ensure_private_dir`), which every caller of
    /// this function is required to have already established. The
    /// `set_permissions` call below narrows the file's own mode too, but
    /// it is a repair for whatever the ambient umask left behind, not the
    /// boundary: rusqlite creates the file itself before this function
    /// gets a chance to touch it, so a permissive umask leaves a
    /// create-then-chmod window that only the private directory actually
    /// closes.
    pub async fn open(path: &Path) -> anyhow::Result<SessionStore> {
        let path = path.to_path_buf();
        let conn = tokio::task::spawn_blocking(move || -> anyhow::Result<Connection> {
            // Explicit flags, not `Connection::open`'s default set: that
            // default includes `SQLITE_OPEN_URI`, which reinterprets a
            // path starting with `file:` as a URI (query parameters,
            // `?mode=...`, and all) instead of a plain filesystem path.
            // `state_dir` is fixed by this process, not attacker input, but
            // a state directory can end up somewhere a caller named after
            // something that happens to start with `file:` regardless —
            // and URI mode is not a feature this module wants at all, so
            // it is left out rather than relied upon to stay harmless.
            // `SQLITE_OPEN_NO_MUTEX` matches `Connection::open`'s own
            // default: this module already serializes every access
            // through its own `Mutex`, so SQLite's internal connection
            // mutex would be redundant locking for no added safety.
            let conn = Connection::open_with_flags(
                &path,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .with_context(|| format!("opening session database {}", path.display()))?;
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                    .with_context(|| format!("restricting mode of {}", path.display()))?;
            }
            // See `BUSY_TIMEOUT`'s docs: this is what turns a handoff-
            // restart's overlapping access into a brief wait instead of an
            // immediate `SQLITE_BUSY` failure.
            conn.busy_timeout(BUSY_TIMEOUT)
                .context("setting sqlite busy timeout")?;
            apply_schema(&conn)?;
            Ok(conn)
        })
        .await
        .context("session store open task panicked")??;
        Ok(SessionStore {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Persist a freshly created session's metadata.
    ///
    /// Called only after the session's tmux window already exists
    /// (`service::Supervisor::create_session`'s ordering: tmux, then this
    /// insert, then the in-memory map) — a failure here is the caller's
    /// signal to tear the just-created tmux session back down rather than
    /// leave a session that is running but was never durably recorded, and
    /// would therefore silently vanish from the list on the next restart.
    pub async fn insert_session(&self, row: StoredSession) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.lock().expect("session db mutex poisoned");
            conn.execute(
                "INSERT INTO sessions (id, title, cwd, invocation, tmux_name, pane, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    row.id,
                    row.title,
                    row.cwd,
                    row.invocation,
                    row.tmux_name,
                    row.pane,
                    now_unix(),
                ],
            )
            .context("inserting session row")?;
            Ok(())
        })
        .await
        .context("session insert task panicked")?
    }

    /// Remove a session's row, if any. Deleting an id with no matching row
    /// is success, not an error — `DELETE` affecting zero rows is simply
    /// what SQLite already does, not a promise this module has to keep on
    /// SPEC.md's behalf — and the supervisor's delete handler relies on
    /// exactly that to call this unconditionally rather than checking
    /// existence first (a check-then-delete would just be a second query
    /// racing nothing, since this connection is already serialized
    /// through one mutex).
    pub async fn delete_session(&self, id: &str) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.lock().expect("session db mutex poisoned");
            conn.execute("DELETE FROM sessions WHERE id = ?1", rusqlite::params![id])
                .context("deleting session row")?;
            Ok(())
        })
        .await
        .context("session delete task panicked")?
    }

    /// Load every persisted session, for `Supervisor::reload_sessions`
    /// (called both from construction and again from `serve`) to turn into
    /// `SessionEntry`s — live if tmux still knows the session, terminal-
    /// less (the restart gap) otherwise. Order is unspecified; the
    /// in-memory map this feeds is keyed by id anyway.
    pub async fn load_all(&self) -> anyhow::Result<Vec<StoredSession>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<StoredSession>> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let mut stmt = conn
                .prepare("SELECT id, title, cwd, invocation, tmux_name, pane FROM sessions")
                .context("preparing session load query")?;
            stmt.query_map([], |r| {
                Ok(StoredSession {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    cwd: r.get(2)?,
                    invocation: r.get(3)?,
                    tmux_name: r.get(4)?,
                    pane: r.get(5)?,
                })
            })
            .context("querying sessions")?
            .collect::<Result<Vec<_>, rusqlite::Error>>()
            .context("decoding session rows")
        })
        .await
        .context("session load task panicked")?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of this module: a session inserted before the
    /// store is dropped must read back byte-identical from a fresh
    /// `SessionStore` opened on the same path — the on-disk round-trip
    /// `Supervisor::reload_sessions` depends on, both at construction and
    /// again from `serve`.
    #[tokio::test]
    async fn insert_then_reopen_round_trips_a_session() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");

        let store = SessionStore::open(&db_path).await.expect("open");
        store
            .insert_session(StoredSession {
                id: "s1".to_string(),
                title: "demo".to_string(),
                cwd: "/tmp/work".to_string(),
                invocation: "agent --flag".to_string(),
                tmux_name: "fh-abc".to_string(),
                pane: "%3".to_string(),
            })
            .await
            .expect("insert");
        drop(store);

        let reopened = SessionStore::open(&db_path).await.expect("reopen");
        let rows = reopened.load_all().await.expect("load");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "s1");
        assert_eq!(rows[0].title, "demo");
        assert_eq!(rows[0].cwd, "/tmp/work");
        assert_eq!(rows[0].invocation, "agent --flag");
        assert_eq!(rows[0].tmux_name, "fh-abc");
        assert_eq!(rows[0].pane, "%3");
    }

    /// A fresh database (`user_version` 0) must come up on `user_version`
    /// [`SCHEMA_VERSION`] after `open` — the invariant every other
    /// `SessionStore` method assumes without re-checking on each call.
    #[tokio::test]
    async fn open_stamps_schema_version_on_a_fresh_database() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        let store = SessionStore::open(&db_path).await.expect("open");
        let conn = Arc::clone(&store.conn);
        let version: i64 = tokio::task::spawn_blocking(move || {
            conn.lock()
                .unwrap()
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap()
        })
        .await
        .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    /// A database claiming a schema version this build does not
    /// understand must be refused outright rather than opened and
    /// silently misread — the honest failure mode for a version this
    /// build has no migration for (a downgrade, or a future version this
    /// build predates).
    #[tokio::test]
    async fn open_refuses_an_unrecognized_schema_version() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        {
            let conn = Connection::open(&db_path).expect("create raw db");
            conn.pragma_update(None, "user_version", 99).unwrap();
        }
        let err = SessionStore::open(&db_path)
            .await
            .expect_err("unrecognized schema version must be refused");
        assert!(
            format!("{err:#}").contains("99"),
            "error must name the unrecognized version: {err:#}"
        );
    }

    /// The confidentiality repair this module performs on top of the
    /// state directory's own 0700 boundary: even a file that starts out
    /// world-writable ends up owner-only after `open`.
    ///
    /// Relying on the test runner's ambient umask here would let this
    /// pass vacuously on any runner whose umask already narrows new files
    /// to 0600 on its own, without ever exercising `open`'s own
    /// `set_permissions` repair. Planting the file at 0o666 BEFORE
    /// calling `open` (rather than mutating the process umask, which
    /// this project's tests must not do) is what forces the repair path
    /// to actually run for the assertion below to mean anything.
    #[tokio::test]
    async fn open_restricts_the_database_files_mode() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        std::fs::write(&db_path, b"").expect("plant a fresh file");
        std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o666))
            .expect("widen the planted file's mode");

        let _store = SessionStore::open(&db_path).await.expect("open");
        let mode = std::fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "database file must be repaired to owner-only, got {mode:o}"
        );
    }

    /// A deleted row must actually be gone on reload, and deleting an id
    /// that was never inserted (or was already deleted) must succeed
    /// rather than error — the idempotence `delete_session`'s docs promise.
    #[tokio::test]
    async fn delete_session_removes_the_row_and_tolerates_a_missing_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        let store = SessionStore::open(&db_path).await.expect("open");
        store
            .insert_session(StoredSession {
                id: "s1".to_string(),
                title: "demo".to_string(),
                cwd: "/tmp/work".to_string(),
                invocation: "agent".to_string(),
                tmux_name: "fh-abc".to_string(),
                pane: "%0".to_string(),
            })
            .await
            .expect("insert");

        store.delete_session("s1").await.expect("delete");
        assert!(
            store.load_all().await.expect("load").is_empty(),
            "deleted row must not survive a reload"
        );

        // Deleting again (an already-deleted row is, by now, exactly the
        // same "no matching row" case as an id that never existed at
        // all — one call suffices for both) must not error.
        store
            .delete_session("s1")
            .await
            .expect("deleting an already-deleted row must be idempotent");
    }

    /// `created_at` is written on every insert but read back by nothing in
    /// this module (see `StoredSession`'s docs) — it exists for a human or
    /// a future migration to consult directly. Assert it is at least
    /// wired correctly: a timestamp captured around the insert (before and
    /// after, since the write happens between the two reads) must bracket
    /// the value SQLite actually stored, queried directly rather than
    /// through `StoredSession` (which does not carry the field at all).
    #[tokio::test]
    async fn insert_session_records_created_at_within_the_surrounding_window() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        let store = SessionStore::open(&db_path).await.expect("open");

        let before = now_unix();
        store
            .insert_session(StoredSession {
                id: "s1".to_string(),
                title: "demo".to_string(),
                cwd: "/tmp/work".to_string(),
                invocation: "agent".to_string(),
                tmux_name: "fh-abc".to_string(),
                pane: "%0".to_string(),
            })
            .await
            .expect("insert");
        let after = now_unix();

        let conn = Arc::clone(&store.conn);
        let created_at: i64 = tokio::task::spawn_blocking(move || {
            conn.lock().unwrap().query_row(
                "SELECT created_at FROM sessions WHERE id = ?1",
                ["s1"],
                |r| r.get(0),
            )
        })
        .await
        .unwrap()
        .unwrap();

        assert!(
            (before..=after).contains(&created_at),
            "created_at {created_at} must fall within [{before}, {after}]"
        );
    }
}

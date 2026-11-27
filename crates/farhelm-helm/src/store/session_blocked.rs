//! A session's blocked mark: its agent said it is blocked (`farhelm agent notify`) and the user has not attended to the
//! session since (SPEC.md, Status).
//!
//! ## Where the mark lives
//!
//! In `session_blocked`, keyed by entry and session like mutes and pins, so a mark survives a helm restart, a retarget
//! and an adoption, and goes with its host entry when that is removed. It is not a `session_cache` column: the cache is
//! replaced wholesale by every refresh, and the mark is the helm's fact, not the supervisor's. Nor is it a row in
//! `session_seen`, whose stamp is required and whose rows a Mark unread deletes: the two facts have different writers
//! and different lifetimes. The supervisor that names the session is not trusted (SPEC.md, "Local authority and trust
//! between hosts"), so a host keeps at most [`LIST_SESSIONS_CAP`] marks, as many as its listing can show sessions; the
//! newest win.
//!
//! ## The in-memory gate
//!
//! The store also keeps the set of keys it holds marks for. It exists for the two paths that clear a mark and run far
//! more often than any mark is set: every terminal input frame and every host refresh ask whether a session is
//! blocked, and the answer is almost always no. The set answers that without a database call, so ordinary typing and
//! refreshing never touch SQLite. Listings do not read the set; they read the table, like every other piece of viewer
//! state, so what a client sees is exactly what is stored.
//!
//! The set is kept a SUPERSET of the table, which is the property the gate needs: a key missing from the set would be
//! a mark nothing could clear, while a key the table no longer holds only costs one delete that finds nothing. Every
//! change to the set happens inside the same connection call as its table write, so changes are serialized by the
//! connection's mutex. A mark enters the set before its row is written, and leaves only after its row is deleted. A
//! host entry's removal cascades its rows away without touching the set; the keys it leaves behind are harmless,
//! because host ids are never reused ([`super::HostId`]) and no listing reads the set.
//!
//! When the set could not be read at open, it is INCOMPLETE ([`Gate`]): it then answers "may be blocked" for every
//! session, so every clear goes to the database, until a later connection call reads the table successfully and
//! completes it. That costs database calls, never a mark that cannot be cleared, and it never refuses the helm's open
//! over a table nothing else depends on.
//!
//! Every removal of a mark leaves through `crate::attention` (`attention::clear` and `attention::cleared_marks`), which
//! wakes clients and is the one place for whatever else must follow a mark's removal, except a host entry's removal,
//! whose rows go by the cascade above.

use super::{HelmStore, HostId, SessionKey};
use farhelm_proto::LIST_SESSIONS_CAP;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// The table, shared by fresh creation and the schema 105 step. `IF NOT EXISTS` so a database rewound by changing just
/// its version keeps its marks, as for mutes. `blocked_at` is the helm's Unix-millisecond time of the latest report,
/// kept only to decide which marks the per-host cap keeps.
pub(super) const SESSION_BLOCKED_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS session_blocked (
    host_id INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
    session_id TEXT NOT NULL,
    blocked_at INTEGER NOT NULL,
    PRIMARY KEY (host_id, session_id)
) STRICT;";

/// Schema 105: add the table, and drop the `agent_blocked` notices that `farhelm agent notify` wrote into the
/// navigation rail's bell before the mark replaced them. Those rows are deleted rather than converted: an old notice
/// says nothing about whether its session is still blocked, and turning every one of them into a red row on upgrade
/// would raise alerts for questions long since answered. The notices table keeps the kind in its constraint, because
/// rebuilding that table again only to narrow the constraint would move every user's notices for no gain; reads skip
/// the kind instead (`crate::notices`).
pub(super) fn migrate(tx: &rusqlite::Transaction<'_>) -> anyhow::Result<()> {
    tx.execute_batch(SESSION_BLOCKED_SCHEMA)?;
    tx.execute("DELETE FROM notices WHERE kind = 'agent_blocked'", [])?;
    Ok(())
}

/// The in-memory gate: the keys the table may hold marks for, and whether that set is complete.
///
/// `complete` is false only after a failed read of the table (at open, or at a later attempt to complete it). While it
/// is false the gate cannot answer "not blocked" for anyone, so [`Gate::may_hold`] says yes to every key and clears go
/// to the database. Each connection call that writes the table, and the host refresh's read, try again to complete it
/// ([`Gate::complete_from`]), under the connection's mutex like every other change to the gate. Should the table stay
/// unreadable while its deletes still succeed (an unlikely split), every counted terminal input frame costs one delete,
/// one at a time per session: more database calls, never a mark that cannot be cleared.
#[derive(Debug, Default)]
pub(crate) struct Gate {
    keys: HashSet<SessionKey>,
    complete: bool,
}

/// The gate as the store shares it between its handles and their connection calls.
pub(crate) type SharedGate = Arc<Mutex<Gate>>;

impl Gate {
    /// Whether the table may hold a mark for `key`. `false` is exact; `true` may be a key the table no longer holds,
    /// or any key at all while the gate is incomplete.
    fn may_hold(&self, key: &SessionKey) -> bool {
        !self.complete || self.keys.contains(key)
    }

    /// Complete an incomplete gate from the table, keeping the keys it already holds. Call only inside a connection
    /// call, so no write to the table interleaves with the read. A failed read leaves the gate incomplete for the next
    /// attempt, with a warning.
    fn complete_from(&mut self, conn: &Connection) {
        if self.complete {
            return;
        }
        match read_keys(conn) {
            Ok(keys) => {
                self.keys.extend(keys);
                self.complete = true;
            }
            Err(error) => tracing::warn!(
                %error,
                "could not read blocked sessions; every clear still asks the database"
            ),
        }
    }
}

/// Every key the table holds marks for. An offline open of an older schema has no table, which holds none.
fn read_keys(conn: &Connection) -> rusqlite::Result<HashSet<SessionKey>> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'session_blocked')",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(HashSet::new());
    }
    let mut statement = conn.prepare("SELECT host_id, session_id FROM session_blocked")?;
    statement
        .query_map([], |row| {
            Ok(SessionKey::new(row.get(0)?, row.get::<_, String>(1)?))
        })?
        .collect()
}

/// The gate's starting contents, read when the store opens. A failed read does not refuse the open: the gate starts
/// incomplete (see [`Gate`]), so marks stay clearable through the database until a later read completes it.
pub(super) fn seed(conn: &Connection) -> SharedGate {
    let mut gate = Gate::default();
    gate.complete_from(conn);
    Arc::new(Mutex::new(gate))
}

/// What [`HelmStore::mark_blocked`] did: whether the session was newly blocked, and which other sessions of the same
/// host lost their marks to the per-host cap. The caller passes the evicted keys to `attention::cleared_marks`, like
/// every other removal.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Marked {
    pub(crate) newly_blocked: bool,
    pub(crate) evicted: Vec<SessionKey>,
}

/// The helm's wall clock in Unix milliseconds, for ordering marks under the cap. Only the order between this helm's
/// own writes matters, so a clock step merely changes which marks a hostile host's flood evicts first.
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

impl HelmStore {
    /// Whether `key` may be blocked, from memory alone. `false` is exact: the table holds no mark for the session.
    /// `true` may be a mark the table no longer holds (see the module docs), which [`Self::clear_blocked`] resolves.
    pub(crate) fn may_be_blocked(&self, key: &SessionKey) -> bool {
        self.blocked
            .lock()
            .expect("blocked session set poisoned")
            .may_hold(key)
    }

    /// The bare ids on `host` that may be blocked, for the host refresh to check against the listing it just read.
    /// From memory alone while the gate is complete, which leaves the usual host, with no blocked agent, an empty
    /// answer that allocates nothing. An incomplete gate is completed from the table first, in a connection call; if
    /// that read fails too, the error says so and the next refresh tries again.
    pub(crate) async fn may_be_blocked_on(&self, host: HostId) -> anyhow::Result<Vec<String>> {
        let on_host = move |gate: &Gate| -> Vec<String> {
            gate.keys
                .iter()
                .filter(|key| key.host == host)
                .map(|key| key.session_id.clone())
                .collect()
        };
        {
            let gate = self.blocked.lock().expect("blocked session set poisoned");
            if gate.complete {
                return Ok(on_host(&gate));
            }
        }
        let blocked = Arc::clone(&self.blocked);
        self.conn
            .call(
                "blocked mark read task panicked",
                move |conn: &mut Connection| {
                    let mut gate = blocked.lock().expect("blocked session set poisoned");
                    gate.complete_from(conn);
                    anyhow::ensure!(gate.complete, "the blocked sessions could not be read");
                    Ok(on_host(&gate))
                },
            )
            .await
    }

    /// Record that `key`'s agent said it is blocked. Reports whether the session was not blocked before, which is when
    /// the listing changes and the caller should wake clients (a repeated report only refreshes the mark's place under
    /// the cap), and which marks past the host's [`LIST_SESSIONS_CAP`] newest were dropped in the same transaction.
    ///
    /// Callers other than tests go through `attention`, which hands the evicted keys to the one exit for removals.
    pub(crate) async fn mark_blocked(&self, key: &SessionKey) -> anyhow::Result<Marked> {
        let key = key.clone();
        let blocked = Arc::clone(&self.blocked);
        self.conn
            .call(
                "blocked mark write task panicked",
                move |conn: &mut Connection| {
                    // Into the gate first: a failure below leaves an extra key, never a missing one.
                    blocked
                        .lock()
                        .expect("blocked session set poisoned")
                        .keys
                        .insert(key.clone());
                    let tx = conn.transaction()?;
                    let existed = tx
                        .query_row(
                            "SELECT 1 FROM session_blocked WHERE host_id = ?1 AND session_id = ?2",
                            params![key.host, key.session_id],
                            |_| Ok(()),
                        )
                        .optional()?
                        .is_some();
                    tx.execute(
                        "INSERT INTO session_blocked (host_id, session_id, blocked_at) VALUES (?1, ?2, ?3) \
                         ON CONFLICT (host_id, session_id) DO UPDATE SET blocked_at = excluded.blocked_at",
                        params![key.host, key.session_id, now_ms()],
                    )?;
                    let evicted: Vec<String> = {
                        let mut statement = tx.prepare(
                            "SELECT session_id FROM session_blocked WHERE host_id = ?1 \
                             ORDER BY blocked_at DESC, rowid DESC LIMIT -1 OFFSET ?2",
                        )?;
                        statement
                            .query_map(params![key.host, LIST_SESSIONS_CAP as i64], |row| {
                                row.get(0)
                            })?
                            .collect::<Result<_, _>>()?
                    };
                    for session_id in &evicted {
                        tx.execute(
                            "DELETE FROM session_blocked WHERE host_id = ?1 AND session_id = ?2",
                            params![key.host, session_id],
                        )?;
                    }
                    tx.commit()?;
                    // Out of the gate only once the rows are gone.
                    let evicted: Vec<SessionKey> = evicted
                        .into_iter()
                        .map(|session_id| SessionKey::new(key.host, session_id))
                        .collect();
                    let mut gate = blocked.lock().expect("blocked session set poisoned");
                    for evicted in &evicted {
                        gate.keys.remove(evicted);
                    }
                    gate.complete_from(conn);
                    Ok(Marked {
                        newly_blocked: !existed,
                        evicted,
                    })
                },
            )
            .await
    }

    /// Clear `key`'s blocked mark, returning whether there was one. A session the gate says is not blocked returns at
    /// once without a database call: the clearing paths run on terminal input and on every refresh of a blocked host's
    /// sessions.
    ///
    /// Callers other than tests go through `attention::clear`, the one exit that wakes clients for a removal.
    pub(crate) async fn clear_blocked(&self, key: &SessionKey) -> anyhow::Result<bool> {
        if !self.may_be_blocked(key) {
            return Ok(false);
        }
        let key = key.clone();
        let blocked = Arc::clone(&self.blocked);
        self.conn
            .call(
                "blocked mark clear task panicked",
                move |conn: &mut Connection| {
                    let changed = conn.execute(
                        "DELETE FROM session_blocked WHERE host_id = ?1 AND session_id = ?2",
                        params![key.host, key.session_id],
                    )? != 0;
                    let mut gate = blocked.lock().expect("blocked session set poisoned");
                    gate.keys.remove(&key);
                    gate.complete_from(conn);
                    Ok(changed)
                },
            )
            .await
    }

    /// Which of `keys` are blocked, read from the table in the same bounded batches as the other viewer state the
    /// listing joins.
    pub(crate) async fn blocked_sessions(
        &self,
        keys: &[SessionKey],
    ) -> anyhow::Result<HashSet<SessionKey>> {
        Ok(self
            .read_keyed_viewer_state(
                keys,
                "blocked mark read task panicked",
                "SELECT host_id, session_id FROM requested JOIN session_blocked USING (host_id, session_id)",
                |_| Ok(()),
            )
            .await?
            .into_keys()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store on a fresh database, with the reserved local row's id for keys.
    async fn fresh() -> (tempfile::TempDir, HelmStore, HostId) {
        let dir = tempfile::tempdir().unwrap();
        let store = HelmStore::open_fixture(&dir.path().join("helm.db"))
            .await
            .unwrap();
        let host = crate::rest_harness::local_id(&store).await;
        (dir, store, host)
    }

    /// The mark is what the listing shows and what every clearing path removes, so marking, reading and clearing must
    /// agree, and must report changes exactly, since each change wakes every client. Specified: marking an unmarked
    /// session reports a change and a repeat does not; the listing read reports exactly the marked session; clearing
    /// reports a change once; and a mark survives reopening the database, gate included.
    #[farhelm_testtrace::test]
    async fn a_mark_is_set_once_read_back_cleared_once_and_survives_reopening() {
        let (dir, store, host) = fresh().await;
        let blocked = SessionKey::new(host, "blocked");
        let other = SessionKey::new(host, "other");
        let keys = [blocked.clone(), other.clone()];
        assert!(store.blocked_sessions(&keys).await.unwrap().is_empty());
        assert!(!store.may_be_blocked(&blocked));

        assert!(store.mark_blocked(&blocked).await.unwrap().newly_blocked);
        assert!(
            !store.mark_blocked(&blocked).await.unwrap().newly_blocked,
            "a repeated report changes nothing a client can see"
        );
        assert_eq!(
            store.blocked_sessions(&keys).await.unwrap(),
            HashSet::from([blocked.clone()])
        );
        assert!(store.may_be_blocked(&blocked) && !store.may_be_blocked(&other));
        assert_eq!(store.may_be_blocked_on(host).await.unwrap(), ["blocked"]);

        drop(store);
        let store = HelmStore::open_fixture(&dir.path().join("helm.db"))
            .await
            .unwrap();
        assert!(
            store.may_be_blocked(&blocked),
            "reopening seeds the gate from the table"
        );
        assert!(store.clear_blocked(&blocked).await.unwrap());
        assert!(!store.clear_blocked(&blocked).await.unwrap());
        assert!(store.blocked_sessions(&keys).await.unwrap().is_empty());
        assert!(!store.may_be_blocked(&blocked));
    }

    /// Every terminal input frame asks whether its session is blocked, so the answer for an unmarked session must come
    /// from memory: typing must never queue behind a database write. Specified: while another thread holds the store's
    /// connection, clearing a session that was never marked answers "no change" within five seconds (a database call
    /// would wait for the connection until it is released); once released, a marked session still clears.
    #[farhelm_testtrace::test]
    async fn clearing_an_unmarked_session_never_waits_on_the_database() {
        let (_dir, store, host) = fresh().await;
        let marked = SessionKey::new(host, "marked");
        assert!(store.mark_blocked(&marked).await.unwrap().newly_blocked);
        // Hold the connection on a blocking thread, so no lock guard lives across this task's awaits.
        let db = store.connection_for_test();
        let (held_tx, held_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let holder = tokio::task::spawn_blocking(move || {
            let _held = db.lock();
            let _ = held_tx.send(());
            let _ = release_rx.recv();
        });
        held_rx.await.expect("the holder takes the connection");
        let unmarked = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            store.clear_blocked(&SessionKey::new(host, "unmarked")),
        )
        .await
        .expect("clearing an unmarked session must not wait for the connection");
        assert!(!unmarked.unwrap());
        release_tx.send(()).unwrap();
        holder.await.unwrap();
        assert!(store.clear_blocked(&marked).await.unwrap());
    }

    /// The session a mark names comes from a host's supervisor, which the helm does not trust, so the marks one host
    /// can leave must be bounded by what its listing could show. Specified: after `LIST_SESSIONS_CAP` + 1 reports
    /// from as many sessions of one host, that host keeps exactly `LIST_SESSIONS_CAP` marks with the oldest gone from
    /// the table and the gate (and reported as evicted), and another host's mark is untouched.
    #[farhelm_testtrace::test]
    async fn a_host_keeps_at_most_a_listing_of_marks() {
        let (_dir, store, host) = fresh().await;
        let other_host = store
            .add_ssh_host("build.example.invalid", None, None)
            .await
            .unwrap();
        let elsewhere = SessionKey::new(other_host, "s0");
        assert!(store.mark_blocked(&elsewhere).await.unwrap().newly_blocked);
        let keys: Vec<SessionKey> = (0..=LIST_SESSIONS_CAP)
            .map(|n| SessionKey::new(host, format!("s{n}")))
            .collect();
        let mut evicted = Vec::new();
        for key in &keys {
            let marked = store.mark_blocked(key).await.unwrap();
            assert!(marked.newly_blocked);
            evicted.extend(marked.evicted);
        }
        let marked = store.blocked_sessions(&keys).await.unwrap();
        assert_eq!(marked.len(), LIST_SESSIONS_CAP);
        assert!(
            !marked.contains(&keys[0]),
            "the oldest mark is the one dropped"
        );
        assert_eq!(
            evicted,
            [keys[0].clone()],
            "the eviction is reported, for the one exit that follows a removal"
        );
        assert!(!store.may_be_blocked(&keys[0]));
        assert_eq!(
            store.may_be_blocked_on(host).await.unwrap().len(),
            LIST_SESSIONS_CAP
        );
        assert!(
            store
                .blocked_sessions(std::slice::from_ref(&elsewhere))
                .await
                .unwrap()
                .contains(&elsewhere),
            "another host's mark does not count toward this host's cap"
        );
    }

    /// Removing a host entry removes everything the helm kept about its sessions, marks included, through the foreign
    /// key rather than a sweep. Specified: a mark on a removed host's session is gone from the table afterwards, and
    /// clearing it through the leftover gate key reports no change.
    #[farhelm_testtrace::test]
    async fn removing_a_host_removes_its_marks() {
        let (_dir, store, _local) = fresh().await;
        let host = store
            .add_ssh_host("build.example.invalid", None, None)
            .await
            .unwrap();
        let key = SessionKey::new(host, "blocked");
        assert!(store.mark_blocked(&key).await.unwrap().newly_blocked);
        store
            .connection_for_test()
            .lock()
            .execute("DELETE FROM hosts WHERE id = ?1", [host])
            .unwrap();
        let rows: i64 = store
            .connection_for_test()
            .lock()
            .query_row("SELECT count(*) FROM session_blocked", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0, "the host's removal cascades its marks");
        assert!(!store.clear_blocked(&key).await.unwrap());
        assert!(!store.may_be_blocked(&key));
    }

    /// A gate that could not be read at open must not answer "not blocked" for anyone, or a mark in the table would
    /// be one nothing could clear (every clearing path asks the gate first). Specified: against a `session_blocked` the
    /// seed cannot read, the gate is incomplete and may hold any key; once the table is readable again, completing it
    /// keeps the keys it already held, adds the table's, and answers exactly from then on.
    #[farhelm_testtrace::test]
    fn an_unreadable_seed_leaves_every_key_clearable_until_it_completes() {
        let conn = Connection::open_in_memory().unwrap();
        // A host row for the restored table's foreign key, and a table the seed's read cannot use.
        conn.execute_batch(
            "CREATE TABLE hosts (id INTEGER PRIMARY KEY); INSERT INTO hosts (id) VALUES (1);
             CREATE TABLE session_blocked (host_id INTEGER NOT NULL);",
        )
        .unwrap();
        let shared = seed(&conn);
        let mut gate = shared.lock().unwrap();
        let in_table = SessionKey::new(1, "in-table");
        let marked_since = SessionKey::new(1, "marked-since");
        let never = SessionKey::new(1, "never");
        assert!(!gate.complete, "the premise: the seed read failed");
        assert!(gate.may_hold(&in_table) && gate.may_hold(&never));

        gate.keys.insert(marked_since.clone());
        conn.execute_batch(&format!(
            "DROP TABLE session_blocked; {SESSION_BLOCKED_SCHEMA}
             INSERT INTO session_blocked (host_id, session_id, blocked_at) VALUES (1, 'in-table', 0);"
        ))
        .unwrap();
        gate.complete_from(&conn);
        assert!(gate.complete);
        assert!(gate.may_hold(&in_table) && gate.may_hold(&marked_since));
        assert!(!gate.may_hold(&never));
    }

    /// Helm schema 105 adds the table and drops the bell notices `farhelm agent notify` used to write, since a blocked
    /// agent now marks its session instead. The step must leave every other notice and the notice id high-water mark
    /// alone, and the table it creates must match a fresh database's. Specified: a database at 104 without the table,
    /// holding one unread `agent_blocked` notice and one late Delete notice, opens at the current version with only
    /// the late Delete notice left (unchanged), the id sequence kept, an empty `session_blocked` whose stored DDL equals
    /// a fresh database's, and a working mark.
    #[farhelm_testtrace::test]
    async fn schema_105_adds_marks_and_drops_the_old_bell_notices() {
        let (dir, store, host) = fresh().await;
        store
            .record_late_delete(
                SessionKey::new(host, "former"),
                "Checkout left at /work/left".into(),
                Arc::new(crate::manager::FleetEvents::default()),
            )
            .await
            .unwrap();
        let kept = store.notices().await.unwrap().notices;
        assert_eq!(kept.len(), 1, "the fixture holds one late Delete notice");
        {
            let db = store.connection_for_test();
            let conn = db.lock();
            conn.execute_batch("DROP TABLE session_blocked; PRAGMA user_version = 104;")
                .unwrap();
            conn.execute(
                "INSERT INTO notices (host_id, kind, condition_key, raised_at, read, resolved, content)
                 VALUES (?1, 'agent_blocked', 's', 0, 0, 0,
                     '{\"kind\":\"agent_blocked\",\"session_id\":\"s\",\"session_title\":\"t\",\"reason\":\"r\"}')",
                [host],
            )
            .unwrap();
            conn.execute(
                "UPDATE sqlite_sequence SET seq = 100 WHERE name = 'notices'",
                [],
            )
            .unwrap();
        }
        drop(store);

        let migrated = HelmStore::open_fixture(&dir.path().join("helm.db"))
            .await
            .unwrap();
        assert_eq!(migrated.schema_version, super::super::SCHEMA_VERSION);
        assert_eq!(migrated.notices().await.unwrap().notices, kept);
        let (legacy, sequence): (i64, i64) = migrated
            .connection_for_test()
            .lock()
            .query_row(
                "SELECT (SELECT count(*) FROM notices WHERE kind = 'agent_blocked'),
                        (SELECT seq FROM sqlite_sequence WHERE name = 'notices')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            legacy, 0,
            "the old bell notices are deleted, not just hidden"
        );
        assert_eq!(sequence, 100, "the notice id high-water mark survives");
        let fresh_dir = tempfile::tempdir().unwrap();
        let fresh = HelmStore::open_fixture(&fresh_dir.path().join("helm.db"))
            .await
            .unwrap();
        let ddl = |store: &HelmStore| -> String {
            store
                .connection_for_test()
                .lock()
                .query_row(
                    "SELECT sql FROM sqlite_schema WHERE name = 'session_blocked'",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
        };
        assert_eq!(ddl(&migrated), ddl(&fresh));
        let key = SessionKey::new(host, "s");
        assert!(
            migrated
                .blocked_sessions(std::slice::from_ref(&key))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(migrated.mark_blocked(&key).await.unwrap().newly_blocked);
    }
}

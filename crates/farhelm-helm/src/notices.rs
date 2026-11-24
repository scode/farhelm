//! Shared notices describe persistent conditions, independently of a session.
//!
//! A successful forwarded trash read supplies evidence, rather than a new
//! observer. Stable condition keys retain read state across repeated failures;
//! only a capable listing's authoritative absence resolves them. Request gates
//! order different clients' observations without blocking session refreshes.

use crate::store::{HelmStore, HostId};
use crate::{AppState, http_error, manager};
use anyhow::Context;
use axum::extract::State;
use axum::response::IntoResponse;
use farhelm_proto::{CheckoutTrashListing, LIST_SESSIONS_CAP};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

/// The original trash-only table, used by the schema 81 upgrade.
pub(crate) const TRASH_ONLY_SCHEMA: &str = "
    CREATE TABLE notices (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        host_id INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
        kind TEXT NOT NULL CHECK (kind = 'stuck_trash'),
        condition_key TEXT NOT NULL,
        raised_at INTEGER NOT NULL,
        read INTEGER NOT NULL CHECK (read IN (0, 1)),
        resolved INTEGER NOT NULL CHECK (resolved IN (0, 1)),
        content TEXT NOT NULL,
        UNIQUE (host_id, kind, condition_key)
    ) STRICT;
";

/// The schema introduced for session ceilings, before desktop PATH notices.
/// The older upgrade must keep its historical shape as new kinds are added.
const SESSION_LIMIT_SCHEMA: &str = "
    CREATE TABLE notices (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        host_id INTEGER REFERENCES hosts (id) ON DELETE CASCADE,
        kind TEXT NOT NULL CHECK (
            (kind IN ('stuck_trash', 'host_session_limit') AND host_id IS NOT NULL)
            OR (kind = 'fleet_session_limit' AND host_id IS NULL)
        ),
        condition_key TEXT NOT NULL,
        raised_at INTEGER NOT NULL,
        read INTEGER NOT NULL CHECK (read IN (0, 1)),
        resolved INTEGER NOT NULL CHECK (resolved IN (0, 1)),
        content TEXT NOT NULL,
        UNIQUE (host_id, kind, condition_key)
    ) STRICT;
    CREATE UNIQUE INDEX notices_app_condition ON notices (kind, condition_key) WHERE host_id IS NULL;
";

/// The notices table as it stood before the late Delete notice kind. The upgrade step that added the desktop
/// login-path notice rebuilds to exactly this, so a database passing through that step matches what that release
/// created. The late Delete step then rebuilds to [`LATE_DELETE_SCHEMA`].
const DESKTOP_PATH_SCHEMA: &str = "
    CREATE TABLE notices (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        host_id INTEGER REFERENCES hosts (id) ON DELETE CASCADE,
        kind TEXT NOT NULL CHECK (
            (kind IN ('stuck_trash', 'host_session_limit') AND host_id IS NOT NULL)
            OR (kind IN ('fleet_session_limit', 'desktop_login_path_unavailable') AND host_id IS NULL)
        ),
        condition_key TEXT NOT NULL,
        raised_at INTEGER NOT NULL,
        read INTEGER NOT NULL CHECK (read IN (0, 1)),
        resolved INTEGER NOT NULL CHECK (resolved IN (0, 1)),
        content TEXT NOT NULL,
        UNIQUE (host_id, kind, condition_key)
    ) STRICT;
    CREATE UNIQUE INDEX notices_app_condition ON notices (kind, condition_key) WHERE host_id IS NULL;
";

/// The table added by the late Delete migration. Keep this constraint at its historical shape.
const LATE_DELETE_SCHEMA: &str = "
    CREATE TABLE notices (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        host_id INTEGER REFERENCES hosts (id) ON DELETE CASCADE,
        kind TEXT NOT NULL CHECK (
            (kind IN ('stuck_trash', 'host_session_limit', 'late_delete') AND host_id IS NOT NULL)
            OR (kind IN ('fleet_session_limit', 'desktop_login_path_unavailable') AND host_id IS NULL)
        ),
        condition_key TEXT NOT NULL,
        raised_at INTEGER NOT NULL,
        read INTEGER NOT NULL CHECK (read IN (0, 1)),
        resolved INTEGER NOT NULL CHECK (resolved IN (0, 1)),
        content TEXT NOT NULL,
        UNIQUE (host_id, kind, condition_key)
    ) STRICT;
    CREATE UNIQUE INDEX notices_app_condition ON notices (kind, condition_key) WHERE host_id IS NULL;
";

/// App-wide conditions have one identity per kind and key via the partial
/// index, since SQLite treats NULLs as distinct in the table constraint.
/// AUTOINCREMENT keeps a stale panel's IDs from naming later notices.
pub(crate) const SCHEMA: &str = "
    CREATE TABLE notices (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        host_id INTEGER REFERENCES hosts (id) ON DELETE CASCADE,
        kind TEXT NOT NULL CHECK (
            (kind IN ('stuck_trash', 'host_session_limit', 'late_delete') AND host_id IS NOT NULL)
            OR (kind IN ('fleet_session_limit', 'desktop_login_path_unavailable', 'new_announcement') AND host_id IS NULL)
        ),
        condition_key TEXT NOT NULL,
        raised_at INTEGER NOT NULL,
        read INTEGER NOT NULL CHECK (read IN (0, 1)),
        resolved INTEGER NOT NULL CHECK (resolved IN (0, 1)),
        content TEXT NOT NULL,
        UNIQUE (host_id, kind, condition_key)
    ) STRICT;
    CREATE UNIQUE INDEX notices_app_condition ON notices (kind, condition_key) WHERE host_id IS NULL;
";

/// Preserve trash history and even IDs already cleared by a panel. Other
/// clients may still hold marks for them. Seed host notices from cut caches
/// so an unreachable host gets the same notice as one that just refreshed.
pub(crate) fn upgrade_session_limits(tx: &rusqlite::Transaction<'_>) -> anyhow::Result<()> {
    let needs_rebuild: bool = tx.query_row(
        "SELECT \"notnull\" FROM pragma_table_info('notices') WHERE name = 'host_id'",
        [],
        |row| row.get(0),
    )?;
    if needs_rebuild {
        rebuild_notices(tx, SESSION_LIMIT_SCHEMA)?;
    }
    let mut statement =
        tx.prepare("SELECT id FROM hosts WHERE cache_truncated AND host_identity IS NOT NULL")?;
    let hosts = statement.query_map([], |row| row.get(0))?;
    for host in hosts {
        reconcile_session_limit(tx, Some(host?), true)?;
    }
    Ok(())
}

/// Widen the set of app-wide notice kinds without changing stored history.
pub(crate) fn upgrade_desktop_login_path_notice(
    tx: &rusqlite::Transaction<'_>,
) -> anyhow::Result<()> {
    rebuild_notices(tx, DESKTOP_PATH_SCHEMA)
}

/// Add one-time cleanup notices for Deletes that finish after their reply.
pub(crate) fn upgrade_late_delete_notice(tx: &rusqlite::Transaction<'_>) -> anyhow::Result<()> {
    rebuild_notices(tx, LATE_DELETE_SCHEMA)
}

/// Widen the app-wide condition set while retaining existing notices and their IDs.
pub(crate) fn upgrade_new_announcement_notice(
    tx: &rusqlite::Transaction<'_>,
) -> anyhow::Result<()> {
    rebuild_notices(tx, SCHEMA)
}

/// Publish one unread reminder for the current batch in the enqueue transaction.
/// A new ID keeps an acknowledgement of an older bell frame from reading the new notice.
pub(crate) fn raise_new_announcement_notice(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO notices (host_id, kind, condition_key, raised_at, read, resolved, content)
         VALUES (NULL, 'new_announcement', '', ?1, 0, 0, ?2)",
        rusqlite::params![
            raised_now(),
            serde_json::to_string(&NoticeContent::NewAnnouncement)?
        ],
    )?;
    Ok(())
}

/// Retire the reminder only after the last pending entry is recorded as shown.
/// The predicate shares the dismissal's transaction, so concurrent work cannot hide a new entry.
pub(crate) fn resolve_new_announcement_notice(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE notices SET read = 1, resolved = 1
         WHERE host_id IS NULL AND kind = 'new_announcement' AND condition_key = ''
             AND resolved = 0 AND NOT EXISTS (SELECT 1 FROM announcements WHERE shown_at IS NULL)",
        [],
    )?;
    Ok(())
}

/// Rebuild a notice constraint with the same columns. Drop the old index
/// before creating its replacement, and carry the allocator high-water mark
/// so an acknowledgement for a cleared notice cannot name a future one.
fn rebuild_notices(tx: &rusqlite::Transaction<'_>, schema: &str) -> anyhow::Result<()> {
    tx.execute_batch(
        "DROP INDEX IF EXISTS notices_app_condition;
                      ALTER TABLE notices RENAME TO previous_notices;",
    )?;
    tx.execute_batch(schema)?;
    tx.execute_batch(
        "INSERT INTO notices SELECT * FROM previous_notices;
         DELETE FROM sqlite_sequence WHERE name = 'notices';
         INSERT INTO sqlite_sequence (name, seq)
             SELECT 'notices', seq FROM sqlite_sequence WHERE name = 'previous_notices';
         DROP TABLE previous_notices;",
    )?;
    Ok(())
}

// The notice shapes live in `farhelm-proto` so the GUI decodes the same types this module serves.
pub(crate) use farhelm_proto::notices::{Notice, NoticeContent};

/// Exact current panel snapshot, newest first, retained until explicit clear.
///
/// Only this wrapper stays here: the GUI decodes the [`Notice`] entries the UI channel splits out of it, never the
/// wrapper, so it is not part of the shared shape.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct Notices {
    pub(crate) notices: Vec<Notice>,
}

/// The gate exists only while a host has live or waiting trash requests.
/// Registry/cache locks cannot span these RPCs: older hosts may take minutes.
#[derive(Default)]
pub(crate) struct TrashRequests {
    gates: Mutex<HashMap<HostId, Weak<AsyncMutex<()>>>>,
    /// Retained between requests: dropping an idle gate must not erase news.
    epochs: Mutex<HashMap<HostId, u64>>,
}

impl TrashRequests {
    /// Serialize a host's reads and deletes across every caller of this helm.
    pub(crate) async fn lock(&self, host: HostId) -> OwnedMutexGuard<()> {
        let gate = {
            let mut gates = self.gates.lock().expect("trash request map poisoned");
            gates.retain(|_, gate| gate.strong_count() != 0);
            match gates.get(&host).and_then(Weak::upgrade) {
                Some(gate) => gate,
                None => {
                    let gate = Arc::new(AsyncMutex::new(()));
                    gates.insert(host, Arc::downgrade(&gate));
                    gate
                }
            }
        };
        gate.lock_owned().await
    }

    /// Compare only within this helm process; connection incarnations remount
    /// readers after restart. A status or acknowledgement changes no Trash.
    pub(crate) fn epoch(&self, host: HostId) -> u64 {
        self.epochs
            .lock()
            .expect("trash epochs poisoned")
            .get(&host)
            .copied()
            .unwrap_or(0)
    }

    /// Publish before the fleet wake, so its hosts read cannot miss this change.
    pub(crate) fn changed(&self, host: HostId) {
        let mut epochs = self.epochs.lock().expect("trash epochs poisoned");
        let epoch = epochs.entry(host).or_default();
        *epoch = epoch.wrapping_add(1);
    }
}

/// Read under one SQLite snapshot; content decoding failures are ordinary store
/// errors, never an empty panel that could acknowledge unseen conditions.
fn read_notices(conn: &Connection, trash_host: Option<HostId>) -> anyhow::Result<Vec<Notice>> {
    let mut statement = conn.prepare(
        "SELECT id, host_id, raised_at, read, resolved, content FROM notices
         WHERE ?1 IS NULL OR (host_id = ?1 AND kind = 'stuck_trash')
         ORDER BY raised_at DESC, id DESC",
    )?;
    let rows = statement.query_map([trash_host], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get::<_, bool>(3)?,
            row.get::<_, bool>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;
    rows.map(|row| {
        let (id, host, raised_at, read, resolved, content) = row?;
        Ok(Notice {
            id,
            host,
            raised_at,
            read: read || resolved,
            resolved,
            content: serde_json::from_str(&content).context("decoding shared notice")?,
        })
    })
    .collect()
}

/// Both ceiling notices follow the trash history rule. Reopening a retained
/// notice keeps the read mark set by resolution. Clearing it allows a future
/// occurrence to raise a new notice.
pub(crate) fn reconcile_session_limit(
    tx: &rusqlite::Transaction<'_>,
    host: Option<HostId>,
    reached: bool,
) -> anyhow::Result<bool> {
    let (kind, content) = match host {
        Some(_) => ("host_session_limit", NoticeContent::HostSessionLimit),
        None => ("fleet_session_limit", NoticeContent::FleetSessionLimit),
    };
    reconcile_condition(tx, host, kind, content, reached)
}

/// Keep one notice for a condition with no per-item key. Resolving marks
/// it read, so a recurrence reopens quietly unless history was cleared.
fn reconcile_condition(
    tx: &rusqlite::Transaction<'_>,
    host: Option<HostId>,
    kind: &str,
    content: NoticeContent,
    active: bool,
) -> anyhow::Result<bool> {
    let previous: Option<(i64, bool)> = tx
        .query_row(
            "SELECT id, resolved FROM notices WHERE host_id IS ?1 AND kind = ?2 AND condition_key = ''",
            rusqlite::params![host, kind],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match previous {
        Some((id, resolved)) if resolved == active => {
            tx.execute(
                "UPDATE notices SET resolved = ?2, read = 1 WHERE id = ?1",
                rusqlite::params![id, !active],
            )?;
        }
        None if active => {
            tx.execute(
                "INSERT INTO notices (host_id, kind, condition_key, raised_at, read, resolved, content)
                 VALUES (?1, ?2, '', ?3, 0, 0, ?4)",
                rusqlite::params![host, kind, raised_now(), serde_json::to_string(&content)?],
            )?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// First observation time for a new notice. Recurrences keep the existing date.
fn raised_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs().min(i64::MAX as u64) as i64)
}

/// Watch the same invalidations that drive the session collection. Every
/// published change to the fleet's membership has a wake here, including
/// live-only lists and host removal. A weak manager keeps this observer
/// from owning the connection lifetime while it waits.
pub(crate) fn watch_fleet_session_limit(
    manager: &Arc<manager::ConnectionManager>,
    store: &HelmStore,
) {
    let mut revisions = manager.events().subscribe();
    let manager = Arc::downgrade(manager);
    let store = store.clone();
    tokio::spawn(async move {
        loop {
            revisions.borrow_and_update();
            let Some(manager) = manager.upgrade() else {
                return;
            };
            if let Err(error) = observe_fleet_session_limit(manager, &store).await {
                tracing::warn!(%error, "could not record the app-wide session ceiling");
            }
            if revisions.changed().await.is_err() {
                return;
            }
        }
    });
}

/// Count the same decodable rows as the unfiltered merged list. Hold the store
/// transaction across the live scope and cache reads, so a durable write
/// cannot invalidate the count before we record the condition.
async fn observe_fleet_session_limit(
    manager: Arc<manager::ConnectionManager>,
    store: &HelmStore,
) -> anyhow::Result<()> {
    let events = Arc::clone(manager.events());
    let changed = store
        .conn()
        .call("app-wide session ceiling task panicked", move |conn| {
            let tx = conn.transaction()?;
            let Some((cache_hosts, live_count)) = manager.session_limit_scope() else {
                return Ok(false);
            };
            let cache = HelmStore::cached_slice_on_connection(&tx, &cache_hosts)?;
            let reached = cache.rows.len() + live_count > LIST_SESSIONS_CAP;
            let changed = reconcile_session_limit(&tx, None, reached)?;
            tx.commit()?;
            Ok(changed)
        })
        .await?;
    if changed {
        events.bump();
    }
    Ok(())
}

/// Record one ordered capable observation atomically. Issue IDs retain their
/// notice because a diagnostic is uncertainty, not evidence that files are gone.
fn record_trash(
    conn: &mut Connection,
    claim: &manager::SessionClaim,
    listing: &CheckoutTrashListing,
) -> anyhow::Result<bool> {
    if !listing.reports_retired {
        return Ok(false);
    }
    let tx = conn.transaction()?;
    let binding: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT entry_uuid, host_identity FROM hosts WHERE id = ?1",
            [claim.host],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if binding.as_ref() != Some(&(claim.entry_uuid.clone(), claim.identity.clone())) {
        return Ok(false);
    }
    let previous = read_notices(&tx, Some(claim.host))?
        .into_iter()
        .filter_map(|notice| {
            let NoticeContent::StuckTrash { checkout_id, .. } = &notice.content else {
                return None;
            };
            Some((checkout_id.clone(), notice))
        })
        .collect::<HashMap<_, _>>();
    let present: HashSet<&str> = listing
        .checkouts
        .iter()
        .map(|entry| entry.id.as_str())
        .chain(listing.issues.iter().map(|issue| issue.id.as_str()))
        .collect();
    let raised_at = raised_now();
    let mut changed = false;
    for entry in &listing.checkouts {
        let old = previous.get(&entry.id);
        let known_bytes = old.and_then(|notice| match &notice.content {
            NoticeContent::StuckTrash { bytes, .. } => *bytes,
            NoticeContent::HostSessionLimit
            | NoticeContent::FleetSessionLimit
            | NoticeContent::DesktopLoginPathUnavailable
            | NoticeContent::NewAnnouncement
            | NoticeContent::LateDelete { .. } => None,
        });
        let content = match &entry.stuck {
            Some(stuck) => NoticeContent::StuckTrash {
                checkout_id: entry.id.clone(),
                name: entry.name.clone(),
                repository: entry.repository.clone(),
                bytes: entry.bytes.or(known_bytes),
                reason: stuck.reason.clone(),
                at: stuck.at,
            },
            None => {
                // A queued Retry temporarily has no failure. Preserve its last
                // explanation, while still learning a successful measurement.
                let Some(old) = old else {
                    continue;
                };
                let mut content = old.content.clone();
                let NoticeContent::StuckTrash { bytes, .. } = &mut content else {
                    continue;
                };
                *bytes = entry.bytes.or(*bytes);
                content
            }
        };
        if let Some(old) = old {
            let resolved = old.resolved && entry.stuck.is_none();
            if old.content != content || old.resolved != resolved {
                tx.execute(
                    "UPDATE notices SET content = ?1, resolved = ?2 WHERE id = ?3",
                    rusqlite::params![serde_json::to_string(&content)?, resolved, old.id],
                )?;
                changed = true;
            }
        } else {
            tx.execute("INSERT INTO notices (host_id, kind, condition_key, raised_at, read, resolved, content) VALUES (?1, 'stuck_trash', ?2, ?3, 0, 0, ?4)",
                rusqlite::params![claim.host, entry.id, raised_at, serde_json::to_string(&content)?])?;
            changed = true;
        }
    }
    if !present.contains("") {
        for (key, notice) in &previous {
            if !notice.resolved && !present.contains(key.as_str()) {
                tx.execute(
                    "UPDATE notices SET resolved = 1, read = 1 WHERE id = ?1",
                    [notice.id],
                )?;
                changed = true;
            }
        }
    }
    tx.commit().context("committing shared trash notices")?;
    Ok(changed)
}

/// Finish an observation under the registry fence. Started blocking work owns
/// both guards and its invalidation even if the HTTP read is cancelled meanwhile.
pub(crate) async fn observe_trash(
    state: &Arc<AppState>,
    claim: manager::SessionClaim,
    listing: CheckoutTrashListing,
    request_guard: OwnedMutexGuard<()>,
) -> anyhow::Result<()> {
    let Some(registry_guard) = state.manager.host_write_lock(claim.host).await else {
        return Ok(());
    };
    let manager = Arc::clone(&state.manager);
    let trash_requests = Arc::clone(&state.trash_requests);
    state.store.conn().call("trash notice observation task panicked", move |conn| {
        let _guards = (request_guard, registry_guard);
        let current = manager.status(claim.host).is_some_and(|status| {
            status.entry_uuid == claim.entry_uuid && status.incarnation == claim.incarnation
                && status.client.is_some()
                && matches!(status.state, manager::HostState::Connected { identity, .. } if identity == claim.identity)
        });
        if !current { return Ok(()); }
        match record_trash(conn, &claim, &listing) {
            Ok(changed) => {
                if changed {
                    trash_requests.changed(claim.host);
                    manager.events().bump();
                }
                Ok(())
            }
            Err(error) => {
                tracing::warn!(host = claim.host, %error, "could not persist shared trash notices");
                Err(error)
            }
        }
    }).await
}

impl HelmStore {
    /// Persist a late Delete's cleanup details independently of the session it removed.
    /// A second observation of the same outcome keeps the original notice's
    /// read state. Wake the feed after the write so its first read sees it.
    pub(crate) async fn record_late_delete(
        &self,
        key: crate::store::SessionKey,
        detail: String,
        events: Arc<manager::FleetEvents>,
    ) -> anyhow::Result<()> {
        self.conn()
            .call("late checkout notice task panicked", move |conn| {
                let content = serde_json::to_string(&NoticeContent::LateDelete { detail })?;
                let changed = conn.execute(
                    "INSERT INTO notices (host_id, kind, condition_key, raised_at, read, resolved, content)
                     VALUES (?1, 'late_delete', ?2, ?3, 0, 0, ?4)
                     ON CONFLICT(host_id, kind, condition_key) DO NOTHING",
                    rusqlite::params![key.host, key.session_id, raised_now(), content],
                )?;
                if changed != 0 {
                    events.bump();
                }
                Ok(())
            })
            .await
    }

    /// Apply the desktop's lookup result before the first client reads its notices.
    /// One recurrence keeps its identity and any read state until explicitly cleared.
    pub(crate) async fn record_desktop_login_path(&self, available: bool) -> anyhow::Result<()> {
        self.conn()
            .call("desktop login PATH notice task panicked", move |conn| {
                let tx = conn.transaction()?;
                reconcile_condition(
                    &tx,
                    None,
                    "desktop_login_path_unavailable",
                    NoticeContent::DesktopLoginPathUnavailable,
                    !available,
                )?;
                tx.commit()?;
                Ok(())
            })
            .await
    }

    /// Return current notices without consulting host reachability.
    pub(crate) async fn notices(&self) -> anyhow::Result<Notices> {
        self.conn()
            .call("notice read task panicked", |conn| {
                Ok(Notices {
                    notices: read_notices(conn, None)?,
                })
            })
            .await
    }
}

/// Shared acknowledgement accepts only the exact IDs displayed by a client.
/// Unknown fields cannot silently turn a misspelled selection into a broad mark.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Marks {
    action: MarkAction,
    ids: Vec<i64>,
}

/// There is deliberately no operation that makes a known problem unread again.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MarkAction {
    Read,
    ClearResolved,
}

/// Ordinary authenticated read, including resolved history until cleared.
pub(crate) async fn get(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.store.notices().await {
        Ok(notices) => axum::Json(notices).into_response(),
        Err(error) => http_error(error),
    }
}

/// Persist exact acknowledgements under ordinary router mutation ownership.
/// The event bump belongs to the blocking commit, so cancellation cannot lose it.
pub(crate) async fn marks(
    State(state): State<Arc<AppState>>,
    axum::Json(body): axum::Json<Marks>,
) -> impl IntoResponse {
    let events = Arc::clone(state.manager.events());
    let result = state
        .store
        .conn()
        .call("notice mark task panicked", move |conn| {
            let tx = conn.transaction()?;
            let changed = {
                let sql = match body.action {
                    // A late Delete is an event. Once read, it has no active
                    // condition to resolve and is available to Clear resolved.
                    MarkAction::Read => {
                        "UPDATE notices SET read = 1,
                        resolved = CASE kind WHEN 'late_delete' THEN 1 ELSE resolved END
                        WHERE id = ?1 AND read = 0"
                    }
                    MarkAction::ClearResolved => {
                        "DELETE FROM notices WHERE id = ?1 AND resolved = 1"
                    }
                };
                let mut statement = tx.prepare(sql)?;
                let mut changed = false;
                for id in body.ids {
                    changed |= statement.execute([id])? != 0;
                }
                changed
            };
            tx.commit()?;
            if changed {
                events.bump();
            }
            Ok(())
        })
        .await;
    match result {
        Ok(()) => axum::http::StatusCode::NO_CONTENT.into_response(),
        Err(error) => http_error(error),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::rest_harness::{self, FleetBuilder, HostScript};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ArchivedCheckout, CheckoutTrashIssue, CheckoutTrashStuck, ControlMsg};
    use futures_util::FutureExt;
    use tower::ServiceExt;

    /// Supply real capable-listing evidence without requiring filesystem removal
    /// in storage tests; the supervisor owns and tests that safety boundary.
    pub(crate) fn stuck(id: &str) -> CheckoutTrashListing {
        CheckoutTrashListing {
            checkouts: vec![ArchivedCheckout {
                id: id.into(),
                name: format!("{id}-archive"),
                repository: "example/project".into(),
                path: format!("/checkouts/.farhelm-trash/{id}"),
                archived_at: Some(10),
                bytes: Some(4096),
                removing: false,
                stuck: Some(CheckoutTrashStuck {
                    reason: "permission denied".into(),
                    at: 20,
                }),
            }],
            reports_retired: true,
            ..Default::default()
        }
    }

    /// The reserved entry is enough for database behavior, including the
    /// identity-less host compatibility path.
    pub(crate) async fn claim(store: &HelmStore) -> manager::SessionClaim {
        let host = rest_harness::local_id(store).await;
        manager::SessionClaim {
            host,
            entry_uuid: store.entry_uuid_of(host).await.unwrap().unwrap(),
            incarnation: 1,
            identity: None,
        }
    }

    /// Record through the exact production transaction, with a fixed fixture
    /// connection rather than mutating the process environment or a live helm.
    pub(crate) fn record(
        store: &HelmStore,
        claim: &manager::SessionClaim,
        listing: &CheckoutTrashListing,
    ) -> bool {
        record_trash(&mut store.connection_for_test().lock(), claim, listing).unwrap()
    }

    /// Exercise extraction/auth/mutation ownership through the same router as
    /// a real client. Bodies and response reads have ordinary finite bounds.
    fn request(method: &str, path: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(path)
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    /// Exact marks must stay safe when a displayed notice was removed meanwhile
    /// or a new failure arrives before an old panel closes.
    pub(crate) async fn mark(harness: &rest_harness::Harness, action: &str, ids: &[i64]) {
        let response = harness
            .router()
            .oneshot(request(
                "POST",
                "/api/notices/marks",
                serde_json::json!({"action":action,"ids":ids}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    /// Preserve the visible wire answer, including optional local persistence
    /// errors, instead of proving only a Rust store value.
    async fn json(response: axum::response::Response) -> serde_json::Value {
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 128 * 1024)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// Repeated failures update in place and a restart retains read state.
    /// Unknown sizes never erase a known snapshot, even after failed sizing.
    #[farhelm_testtrace::test]
    async fn repeated_failures_preserve_identity_read_state_and_known_size_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        let claim = claim(&store).await;
        let mut listing = stuck("first");
        assert!(record(&store, &claim, &listing));
        let first = store.notices().await.unwrap().notices.remove(0);
        store
            .connection_for_test()
            .lock()
            .execute("UPDATE notices SET read = 1", [])
            .unwrap();
        listing.checkouts[0].stuck.as_mut().unwrap().at = 30;
        listing.checkouts[0].stuck.as_mut().unwrap().reason = "still denied".into();
        listing.checkouts[0].bytes = None;
        assert!(record(&store, &claim, &listing));
        assert!(!record(&store, &claim, &listing));
        listing.checkouts[0].bytes = Some(4096);
        assert!(!record(&store, &claim, &listing));
        drop(store);
        let store = HelmStore::open(&path).await.unwrap();
        let updated = store.notices().await.unwrap().notices.remove(0);
        assert_eq!((updated.id, updated.raised_at), (first.id, first.raised_at));
        assert!(updated.read && !updated.resolved);
        let NoticeContent::StuckTrash {
            bytes, at, reason, ..
        } = updated.content
        else {
            panic!("expected the retained trash notice");
        };
        assert_eq!(
            (bytes, at, reason.as_str()),
            (Some(4096), 30, "still denied")
        );
    }

    /// A retry's transient shape, diagnostic-only presence and an old peer's
    /// empty answer cannot establish that a known folder has been freed.
    #[farhelm_testtrace::test]
    async fn only_capable_authoritative_absence_resolves_and_reopens_without_realerting() {
        let dir = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let claim = claim(&store).await;
        let failure = stuck("first");
        assert!(record(&store, &claim, &failure));
        assert!(!record(&store, &claim, &CheckoutTrashListing::default()));
        let mut transient = failure.clone();
        transient.checkouts[0].stuck = None;
        transient.checkouts[0].removing = true;
        assert!(!record(&store, &claim, &transient));
        let mut uncertain = CheckoutTrashListing {
            reports_retired: true,
            issues: vec![CheckoutTrashIssue {
                id: "first".into(),
                path: "/trash/first".into(),
                message: "cannot inspect".into(),
            }],
            ..Default::default()
        };
        assert!(!record(&store, &claim, &uncertain));
        uncertain.issues[0].id.clear();
        assert!(!record(&store, &claim, &uncertain));
        let empty = CheckoutTrashListing {
            reports_retired: true,
            ..Default::default()
        };
        assert!(record(&store, &claim, &empty));
        let resolved = store.notices().await.unwrap().notices.remove(0);
        assert!(resolved.read && resolved.resolved);
        assert!(!record(&store, &claim, &empty));
        assert!(record(&store, &claim, &failure));
        let reopened = store.notices().await.unwrap().notices.remove(0);
        assert_eq!(reopened.id, resolved.id);
        assert!(reopened.read && !reopened.resolved);
    }

    /// A version-80 upgrade adds only notice storage and preserves existing
    /// entry identity and preferences. Its DDL must match fresh creation.
    #[farhelm_testtrace::test]
    async fn schema_81_upgrade_matches_fresh_and_preserves_registry_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("upgrade.db");
        let store = HelmStore::open(&path).await.unwrap();
        let before = claim(&store).await;
        store
            .update_preferences(crate::store::PreferencePatch {
                compact: Some(Some(true)),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(
            store.preferences().await.unwrap().compact,
            Some(true),
            "fixture has an existing preference to preserve"
        );
        {
            let db = store.connection_for_test();
            let conn = db.lock();
            conn.execute_batch("DROP TABLE notices; PRAGMA user_version = 80;")
                .unwrap();
            let count: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name = 'notices'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0, "fixture starts without notice storage");
        }
        drop(store);
        let upgraded = HelmStore::open(&path).await.unwrap();
        assert_eq!(claim(&upgraded).await.entry_uuid, before.entry_uuid);
        assert_eq!(upgraded.preferences().await.unwrap().compact, Some(true));
        assert!(upgraded.notices().await.unwrap().notices.is_empty());
        let fresh = HelmStore::open(&dir.path().join("fresh.db")).await.unwrap();
        let schema = |store: &HelmStore| -> String {
            store
                .connection_for_test()
                .lock()
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE name = 'notices'",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
        };
        assert_eq!(schema(&upgraded), schema(&fresh));
    }

    /// Upgrading a cut, unreachable host raises its notice while preserving
    /// trash history and the IDs retired before the upgrade.
    #[farhelm_testtrace::test]
    async fn ceiling_migration_preserves_trash_and_backfills_a_cut_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        let host = rest_harness::local_id(&store).await;
        {
            let db = store.connection_for_test();
            let conn = db.lock();
            conn.execute_batch(&format!(
                "DROP TABLE notices; {TRASH_ONLY_SCHEMA} PRAGMA user_version = 87;"
            ))
            .unwrap();
            conn.execute(
                "UPDATE hosts SET host_identity = 'old-install', cache_truncated = 1 WHERE id = ?1",
                [host],
            )
            .unwrap();
        }
        let old_claim = manager::SessionClaim {
            identity: Some("old-install".into()),
            ..claim(&store).await
        };
        assert!(record(&store, &old_claim, &stuck("preserved")));
        assert!(record(
            &store,
            &old_claim,
            &CheckoutTrashListing {
                reports_retired: true,
                ..Default::default()
            }
        ));
        let original = store.notices().await.unwrap().notices.remove(0);
        assert!(original.read && original.resolved);
        store
            .connection_for_test()
            .lock()
            .execute(
                "UPDATE sqlite_sequence SET seq = 100 WHERE name = 'notices'",
                [],
            )
            .unwrap();
        drop(store);

        let store = HelmStore::open(&path).await.unwrap();
        let notices = store.notices().await.unwrap().notices;
        assert!(notices.contains(&original));
        let ceiling = notices
            .iter()
            .find(|notice| notice.content == NoticeContent::HostSessionLimit)
            .expect("the existing cache was already cut");
        assert_eq!(ceiling.host, Some(host));
        assert!(ceiling.id > 100 && !ceiling.read && !ceiling.resolved);
    }

    /// A version-91 database already has the notice index and may have
    /// cleared entries. The constraint change must carry the allocator
    /// and both host and app-wide history into the new schema.
    #[farhelm_testtrace::test]
    async fn desktop_path_migration_preserves_history_and_the_next_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        let host = rest_harness::local_id(&store).await;
        {
            let db = store.connection_for_test();
            let mut conn = db.lock();
            conn.execute_batch(&format!(
                "DROP TABLE notices; {SESSION_LIMIT_SCHEMA} PRAGMA user_version = 91;"
            ))
            .unwrap();
            let tx = conn.transaction().unwrap();
            assert!(reconcile_session_limit(&tx, Some(host), true).unwrap());
            assert!(reconcile_session_limit(&tx, None, true).unwrap());
            assert!(reconcile_session_limit(&tx, None, false).unwrap());
            assert!(
                reconcile_condition(
                    &tx,
                    None,
                    "desktop_login_path_unavailable",
                    NoticeContent::DesktopLoginPathUnavailable,
                    true
                )
                .is_err()
            );
            tx.execute(
                "UPDATE sqlite_sequence SET seq = 100 WHERE name = 'notices'",
                [],
            )
            .unwrap();
            tx.commit().unwrap();
        }
        let old = store.notices().await.unwrap().notices;
        drop(store);

        let upgraded = HelmStore::open(&path).await.unwrap();
        assert_eq!(upgraded.notices().await.unwrap().notices, old);
        upgraded.record_desktop_login_path(false).await.unwrap();
        let after = upgraded.notices().await.unwrap().notices;
        let added = after
            .iter()
            .find(|notice| notice.content == NoticeContent::DesktopLoginPathUnavailable)
            .unwrap();
        assert!(added.id > 100 && added.host.is_none());
        assert!(old.iter().all(|notice| after.contains(notice)));
        upgraded
            .record_late_delete(
                crate::store::SessionKey::new(host, "former-session"),
                "Checkout left at /work/left".into(),
                Arc::new(manager::FleetEvents::default()),
            )
            .await
            .unwrap();
        let notices = upgraded.notices().await.unwrap().notices;
        let late = notices
            .iter()
            .find(|notice| matches!(notice.content, NoticeContent::LateDelete { .. }))
            .unwrap();
        assert!(late.id > added.id);
        assert!(after.iter().all(|notice| notices.contains(notice)));
    }

    /// The announcement upgrade must retain a pre-existing notice and the
    /// allocator high-water mark of notices a client has already cleared.
    #[farhelm_testtrace::test]
    async fn announcement_migration_keeps_the_previous_table_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        {
            let db = store.connection_for_test();
            let mut conn = db.lock();
            conn.execute_batch(&format!(
                "DROP TABLE notices; {LATE_DELETE_SCHEMA}
                 ALTER TABLE preferences DROP COLUMN check_announcements;
                 PRAGMA user_version = 100;"
            ))
            .unwrap();
            let tx = conn.transaction().unwrap();
            assert!(reconcile_session_limit(&tx, None, true).unwrap());
            assert!(reconcile_session_limit(&tx, None, false).unwrap());
            assert!(raise_new_announcement_notice(&tx).is_err());
            tx.execute(
                "UPDATE sqlite_sequence SET seq = 100 WHERE name = 'notices'",
                [],
            )
            .unwrap();
            tx.commit().unwrap();
        }
        let old = store.notices().await.unwrap().notices;
        assert_eq!(old.len(), 1);
        assert!(old[0].read && old[0].resolved);
        drop(store);

        let migrated = HelmStore::open(&path).await.unwrap();
        assert_eq!(migrated.notices().await.unwrap().notices, old);
        assert_eq!(
            migrated.preferences().await.unwrap().check_announcements,
            None
        );
        migrated
            .connection_for_test()
            .lock()
            .execute_batch(
                "INSERT INTO announcements (id, title, body) VALUES ('one', 'Title', 'Body');",
            )
            .unwrap();
        raise_new_announcement_notice(&migrated.connection_for_test().lock()).unwrap();
        let notices = migrated.notices().await.unwrap().notices;
        assert!(notices.contains(&old[0]));
        let announcement = notices
            .iter()
            .find(|n| n.content == NoticeContent::NewAnnouncement)
            .unwrap();
        assert!(announcement.id > 100);
        assert!(announcement.host.is_none() && !announcement.read && !announcement.resolved);
    }

    /// A completed Delete has no later condition read to resolve its notice.
    /// Read acknowledgement makes it clearable with the ordinary panel action.
    #[farhelm_testtrace::test]
    async fn a_late_checkout_notice_can_be_cleared_after_it_is_read() {
        let harness = FleetBuilder::new().await.start().await;
        let host = rest_harness::local_id(&harness.store).await;
        harness
            .store
            .record_late_delete(
                crate::store::SessionKey::new(host, "former-session"),
                "Checkout left at /work/left".into(),
                Arc::clone(harness.manager.events()),
            )
            .await
            .unwrap();
        let notice = harness.store.notices().await.unwrap().notices.remove(0);
        let ids = [notice.id];
        assert!(!notice.read && !notice.resolved);
        mark(&harness, "clear_resolved", &ids).await;
        assert_eq!(harness.store.notices().await.unwrap().notices, [notice]);
        mark(&harness, "read", &ids).await;
        assert!(harness.store.notices().await.unwrap().notices[0].resolved);
        mark(&harness, "clear_resolved", &ids).await;
        assert!(harness.store.notices().await.unwrap().notices.is_empty());
    }

    /// Wait for a particular state of a persisted ceiling notice. The
    /// observer publishes its own revision after saving it, even when the
    /// original session-list change was published earlier.
    async fn wait_for_limit(
        harness: &rest_harness::Harness,
        kind: NoticeContent,
        resolved: bool,
    ) -> Notice {
        let mut revisions = harness.manager.events().subscribe();
        tokio::time::timeout(std::time::Duration::from_secs(8), async {
            loop {
                if let Some(notice) = harness
                    .store
                    .notices()
                    .await
                    .unwrap()
                    .notices
                    .into_iter()
                    .find(|notice| notice.content == kind && notice.resolved == resolved)
                {
                    return notice;
                }
                revisions.changed().await.unwrap();
            }
        })
        .await
        .unwrap_or_else(|_| panic!("did not observe {kind:?} with resolved={resolved}"))
    }

    /// The app-wide condition includes live-only rows and can resolve while
    /// a host remains cut. A recurring condition keeps its identity and mark.
    #[farhelm_testtrace::test]
    async fn host_and_fleet_session_ceiling_notices_follow_independent_conditions() {
        let builder = FleetBuilder::new()
            .await
            .local(HostScript {
                sessions: vec![rest_harness::session("live-only", 1)],
                ..Default::default()
            })
            .await;
        let (builder, remote) = builder
            .ssh(
                "build.example.invalid",
                HostScript {
                    identity: Some("durable-install".into()),
                    sessions: (0..LIST_SESSIONS_CAP)
                        .map(|id| rest_harness::session(&format!("remote-{id}"), id as i64))
                        .collect(),
                    truncated: true,
                    ..Default::default()
                },
            )
            .await;
        let harness = builder.start().await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;
        harness.await_refreshed(remote).await;
        let listing = crate::aggregate::session_list(
            &harness.manager,
            &harness.store,
            &crate::store::SessionFilter::default(),
            crate::store::ListSort::Created,
        )
        .await
        .unwrap();
        assert_eq!(listing.total, LIST_SESSIONS_CAP as u64 + 1);
        let host = wait_for_limit(&harness, NoticeContent::HostSessionLimit, false).await;
        let fleet = wait_for_limit(&harness, NoticeContent::FleetSessionLimit, false).await;
        assert_eq!((host.host, fleet.host), (Some(remote), None));
        assert!(!host.read && !fleet.read);
        mark(&harness, "read", &[host.id, fleet.id]).await;
        harness.refresh_to_completion(remote).await;
        assert_eq!(
            wait_for_limit(&harness, NoticeContent::HostSessionLimit, false).await,
            Notice {
                read: true,
                ..host.clone()
            }
        );
        {
            let connection = harness.store.connection_for_test();
            let mut conn = connection.lock();
            let tx = conn.transaction().unwrap();
            assert!(
                !reconcile_session_limit(&tx, None, true).unwrap(),
                "another observation of the same fleet cut changes nothing"
            );
            tx.commit().unwrap();
        }

        let status = harness
            .manager
            .status(local)
            .expect("live-only host is connected");
        harness
            .manager
            .forget_session(
                &manager::SessionClaim {
                    host: local,
                    entry_uuid: status.entry_uuid,
                    incarnation: status.incarnation,
                    identity: None,
                },
                "live-only",
            )
            .await
            .unwrap();
        let resolved = wait_for_limit(&harness, NoticeContent::FleetSessionLimit, true).await;
        assert_eq!(resolved.id, fleet.id);
        assert!(resolved.read);
        assert_eq!(
            wait_for_limit(&harness, NoticeContent::HostSessionLimit, false)
                .await
                .id,
            host.id
        );

        harness
            .fleet
            .edit(remote, |script| script.truncated = false);
        harness.manager.refresh_now(remote);
        let resolved = wait_for_limit(&harness, NoticeContent::HostSessionLimit, true).await;
        assert_eq!(resolved.id, host.id);
        assert!(resolved.read);
        harness.fleet.edit(remote, |script| script.truncated = true);
        harness.manager.refresh_now(remote);
        let reopened = wait_for_limit(&harness, NoticeContent::HostSessionLimit, false).await;
        assert_eq!(reopened.id, host.id);
        assert!(reopened.read);

        harness.manager.refresh_now(local);
        let reopened = wait_for_limit(&harness, NoticeContent::FleetSessionLimit, false).await;
        assert_eq!(reopened.id, fleet.id);
        assert!(reopened.read);

        let reply = harness
            .router()
            .oneshot(request(
                "DELETE",
                &format!("/api/hosts/{remote}"),
                serde_json::json!({}),
            ))
            .await
            .unwrap();
        assert_eq!(reply.status(), StatusCode::OK);
        assert_eq!(
            wait_for_limit(&harness, NoticeContent::FleetSessionLimit, true)
                .await
                .id,
            fleet.id
        );
        assert!(
            !harness
                .store
                .notices()
                .await
                .unwrap()
                .notices
                .iter()
                .any(|notice| notice.id == host.id)
        );
    }

    /// Exact IDs prevent a stale close from reading a newer notice, and clear
    /// never hides unresolved conditions. IDs remain retired across deletion.
    #[farhelm_testtrace::test]
    async fn marks_only_affect_supplied_ids_and_cleared_ids_are_never_reused() {
        let harness = FleetBuilder::new()
            .await
            .local(HostScript::default())
            .await
            .start()
            .await;
        let host = rest_harness::local_id(&harness.store).await;
        // A settled live peer removes connection-ladder invalidations from
        // the exact event-count comparison below.
        harness.await_refreshed(host).await;
        let claim = claim(&harness.store).await;
        record(&harness.store, &claim, &stuck("first"));
        let first = harness.store.notices().await.unwrap().notices[0].id;
        record(
            &harness.store,
            &claim,
            &CheckoutTrashListing {
                checkouts: vec![
                    stuck("first").checkouts.remove(0),
                    stuck("second").checkouts.remove(0),
                ],
                reports_retired: true,
                ..Default::default()
            },
        );
        let notices = harness.store.notices().await.unwrap().notices;
        let second = notices[0].id;
        assert!(second > first);
        let before = harness.manager.events().revision();
        mark(&harness, "read", &[first, first, -1]).await;
        assert_eq!(harness.manager.events().revision(), before + 1);
        assert!(!harness.store.notices().await.unwrap().notices[0].read);
        mark(&harness, "read", &[first]).await;
        assert_eq!(harness.manager.events().revision(), before + 1);
        mark(&harness, "clear_resolved", &[first, second]).await;
        assert_eq!(harness.store.notices().await.unwrap().notices.len(), 2);
        record(&harness.store, &claim, &stuck("second"));
        mark(&harness, "clear_resolved", &[first, second]).await;
        assert_eq!(harness.store.notices().await.unwrap().notices.len(), 1);
        record(&harness.store, &claim, &stuck("first"));
        assert!(harness.store.notices().await.unwrap().notices[0].id > second);
        mark(&harness, "read", &[first]).await;
        assert!(!harness.store.notices().await.unwrap().notices[0].read);
    }

    /// New routes inherit authentication and refuse incomplete/misspelled
    /// selectors rather than interpreting them as permission to mark all.
    #[farhelm_testtrace::test]
    async fn routes_authenticate_and_reject_ambiguous_mark_requests() {
        let harness = FleetBuilder::new().await.start().await;
        for (method, path, body) in [
            ("GET", "/api/notices", serde_json::Value::Null),
            (
                "POST",
                "/api/notices/marks",
                serde_json::json!({"action":"read","ids":[]}),
            ),
        ] {
            let response = harness
                .unauthenticated_router()
                .oneshot(request(method, path, body))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
        for body in [
            serde_json::json!({"action":"read"}),
            serde_json::json!({"action":"read","id":[]}),
            serde_json::json!({"action":"unread","ids":[]}),
            serde_json::json!({"action":"read","ids":[],"all":true}),
        ] {
            let response = harness
                .router()
                .oneshot(request("POST", "/api/notices/marks", body))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        }
        mark(&harness, "read", &[]).await;
        let value = json(
            harness
                .router()
                .oneshot(request("GET", "/api/notices", serde_json::Value::Null))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(value, serde_json::json!({"notices":[]}));
    }

    /// Retarget/adoption must not file an old installation's failure beneath
    /// an entry that now belongs to another installation; removal cascades.
    #[farhelm_testtrace::test]
    async fn binding_checks_refuse_old_entries_and_host_removal_cascades() {
        let dir = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let mut claim = claim(&store).await;
        let original = claim.clone();
        claim.entry_uuid = "11111111-1111-4111-8111-111111111111".into();
        assert!(!record(&store, &claim, &stuck("first")));
        claim = original;
        claim.identity = Some("obsolete".into());
        assert!(!record(&store, &claim, &stuck("first")));
        claim.identity = None;
        assert!(record(&store, &claim, &stuck("first")));
        store
            .connection_for_test()
            .lock()
            .execute("DELETE FROM hosts WHERE id = ?1", [claim.host])
            .unwrap();
        assert!(store.notices().await.unwrap().notices.is_empty());
        assert!(!record(&store, &claim, &stuck("first")));
    }

    /// A cancelled HTTP read cannot release ordering while its already-started
    /// SQLite work is waiting for the connection. No scheduling sleep is needed:
    /// polling the future once hands the guards to Db::call synchronously.
    #[farhelm_testtrace::test]
    async fn cancelled_observation_keeps_request_and_registry_guards_until_database_completion() {
        let harness = FleetBuilder::new()
            .await
            .local(HostScript::default())
            .await
            .start()
            .await;
        let host = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(host).await;
        // No host-refresh wake may stand in for the cancelled observation.
        harness.refresh_to_completion(host).await;
        let before = harness.manager.events().revision();
        let (claim, _) = crate::sessions::host_client(&harness.state, host).unwrap();
        let gate = harness.state.trash_requests.lock(host).await;
        let mut next = Box::pin(harness.state.trash_requests.lock(host));
        let mut registry = Box::pin(harness.manager.host_write_lock(host));
        {
            let db = harness.store.connection_for_test();
            let _held = db.lock();
            let mut observation =
                Box::pin(observe_trash(&harness.state, claim, stuck("first"), gate));
            assert!(observation.as_mut().now_or_never().is_none());
            drop(observation);
            assert!(
                next.as_mut().now_or_never().is_none(),
                "started DB work still owns trash ordering"
            );
            assert!(
                registry.as_mut().now_or_never().is_none(),
                "started DB work still owns registry fence"
            );
        }
        let _next = tokio::time::timeout(std::time::Duration::from_secs(10), next)
            .await
            .unwrap();
        let _registry = tokio::time::timeout(std::time::Duration::from_secs(10), registry)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(harness.store.notices().await.unwrap().notices.len(), 1);
        assert_eq!(
            harness.state.trash_requests.epoch(host),
            1,
            "cancellation cannot lose the committed Trash epoch"
        );
        assert!(
            harness.manager.events().revision() > before,
            "the committed revision must wake the rest of the fleet"
        );
    }

    /// A reply from a disconnected incarnation must not create shared evidence,
    /// even though the original caller may still receive its remote answer.
    #[farhelm_testtrace::test]
    async fn stale_incarnation_is_ignored_before_shared_observation() {
        let harness = FleetBuilder::new()
            .await
            .local(HostScript::default())
            .await
            .start()
            .await;
        let host = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(host).await;
        let (mut claim, _) = crate::sessions::host_client(&harness.state, host).unwrap();
        claim.incarnation += 1;
        let gate = harness.state.trash_requests.lock(host).await;
        observe_trash(&harness.state, claim, stuck("first"), gate)
            .await
            .unwrap();
        assert!(harness.store.notices().await.unwrap().notices.is_empty());
        assert_eq!(harness.state.trash_requests.epoch(host), 0);
    }

    /// Different clients must pass through one host's ordering gate before
    /// routing, while another host's requests remain independent. An unreachable
    /// fixture makes bypass observable as an immediate routing error. Poll the
    /// handlers directly: auth's database read and mutation ownership's task
    /// handoff could otherwise explain a pending router future without a gate.
    /// The Delete hands its work to a task of its own (its reply is bounded),
    /// so the test lets that task run before checking it still waits.
    #[farhelm_testtrace::test]
    async fn trash_handlers_wait_for_their_host_gate_without_blocking_other_hosts() {
        let harness = FleetBuilder::new().await.start().await;
        let host = rest_harness::local_id(&harness.store).await;
        let guard = harness.state.trash_requests.lock(host).await;
        let mut list = Box::pin(crate::sessions::list_checkout_trash(
            State(Arc::clone(&harness.state)),
            axum::Json(serde_json::from_value(serde_json::json!({"host":host})).unwrap()),
        ));
        let mut delete = Box::pin(crate::sessions::delete_checkout_trash(
            State(Arc::clone(&harness.state)),
            axum::Json(
                serde_json::from_value(serde_json::json!({"host":host,"ids":["first"]})).unwrap(),
            ),
        ));
        assert!(
            list.as_mut().now_or_never().is_none(),
            "list waits before routing"
        );
        // The Delete's first poll only starts its work on a task of its own, which a bypass of the gate would let
        // finish (a routing error) as soon as it runs; give it every chance to, then check it is still waiting.
        let _ = delete.as_mut().now_or_never();
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        assert!(
            delete.as_mut().now_or_never().is_none(),
            "Delete's own task waits for the gate too"
        );
        let unrelated = crate::sessions::list_checkout_trash(
            State(Arc::clone(&harness.state)),
            axum::Json(serde_json::from_value(serde_json::json!({"host":i64::MAX})).unwrap()),
        )
        .now_or_never()
        .expect("another host is never blocked by this host's gate")
        .into_response();
        assert_eq!(unrelated.status(), StatusCode::NOT_FOUND);
        drop(guard);
        assert_eq!(
            list.now_or_never()
                .expect("first queued caller owns the released gate")
                .into_response()
                .status(),
            StatusCode::CONFLICT
        );
        // The Delete's work runs on a task of its own (its reply is bounded while the cleanup goes on), so it takes the
        // released gate once that task runs rather than within one synchronous poll.
        assert_eq!(
            delete.await.into_response().status(),
            StatusCode::CONFLICT,
            "next caller proceeds after the first releases"
        );
    }

    /// The real HTTP/RPC seam records notices and wakes the window after an
    /// accepted Delete, with only that host's epoch changing. A storage failure
    /// preserves actual checkout evidence and carries a visible error instead
    /// of disguising it as an empty list.
    #[farhelm_testtrace::test]
    async fn forwarded_lists_and_delete_receipts_drive_changed_only_notices_and_errors() {
        let (client, peer) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (read, write) = tokio::io::split(peer);
            let mut reader = FrameReader::new(read);
            let mut writer = FrameWriter::new(write);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            for step in 0..6 {
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let reply = if step == 2 {
                    let ControlMsg::DeleteCheckoutTrash { req_id, ids } = request else {
                        panic!("expected Delete, got {request:?}");
                    };
                    assert_eq!(ids, ["first"]);
                    ControlMsg::CheckoutTrashDeleted {
                        req_id,
                        deleted: farhelm_proto::CheckoutTrashDeleted {
                            removed: ids,
                            issues: vec![],
                        },
                    }
                } else {
                    let ControlMsg::ListCheckoutTrash { req_id, .. } = request else {
                        panic!("expected List, got {request:?}");
                    };
                    let mut listing = stuck("first");
                    if step == 3 {
                        listing.checkouts[0].stuck = None;
                        listing.checkouts[0].removing = true;
                        listing.removing = true;
                    } else if step == 4 {
                        listing.checkouts.clear();
                    }
                    ControlMsg::CheckoutTrashListed { req_id, listing }
                };
                writer.write_control(&reply).await.unwrap();
            }
        });
        let harness = rest_harness::spliced_helm(client).await;
        let host = rest_harness::local_id(&harness.store).await;
        let mut id = None;
        for step in 0..6 {
            let before = harness.manager.events().revision();
            let epoch = harness.state.trash_requests.epoch(host);
            if step == 5 {
                harness
                    .store
                    .connection_for_test()
                    .lock()
                    .execute_batch("DROP TABLE notices;")
                    .unwrap();
            }
            let (path, body) = if step == 2 {
                (
                    "/api/checkout-trash/delete",
                    serde_json::json!({"host":host,"ids":["first"]}),
                )
            } else {
                ("/api/checkout-trash/list", serde_json::json!({"host":host}))
            };
            let value = json(
                harness
                    .router()
                    .oneshot(request("POST", path, body))
                    .await
                    .unwrap(),
            )
            .await;
            assert_eq!(
                harness.manager.events().revision(),
                before + u64::from(matches!(step, 0 | 2 | 4)),
                "step {step}"
            );
            let expected = epoch + u64::from(matches!(step, 0 | 2 | 4));
            let views = crate::hosts::host_views(&harness.state).await.unwrap();
            assert_eq!(
                views
                    .iter()
                    .find(|view| view.id == host)
                    .unwrap()
                    .trash_epoch,
                expected
            );
            assert_eq!(harness.state.trash_requests.epoch(i64::MAX), 0);
            if step == 0 {
                let notice = harness.store.notices().await.unwrap().notices.remove(0);
                let response = marks(
                    State(Arc::clone(&harness.state)),
                    axum::Json(Marks {
                        action: MarkAction::Read,
                        ids: vec![notice.id],
                    }),
                )
                .await
                .into_response();
                assert_eq!(response.status(), StatusCode::NO_CONTENT);
                assert_eq!(
                    harness.state.trash_requests.epoch(host),
                    expected,
                    "acknowledging a notice does not change Trash"
                );
            }
            if step == 5 {
                assert_eq!(value["checkouts"][0]["id"], "first");
                assert!(
                    value["notice_error"]
                        .as_str()
                        .unwrap()
                        .contains("Could not save")
                );
            } else {
                assert!(value.get("notice_error").is_none());
                let notice = harness.store.notices().await.unwrap().notices.remove(0);
                assert_eq!(*id.get_or_insert(notice.id), notice.id);
                assert_eq!(notice.resolved, step == 4);
                if step == 2 {
                    assert_eq!(value["removed"], serde_json::json!(["first"]));
                }
            }
        }
        peer.await.unwrap();
    }
}

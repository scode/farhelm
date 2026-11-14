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
use farhelm_proto::CheckoutTrashListing;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

/// Fresh databases and upgrades use exactly the same condition identity and
/// cascade. AUTOINCREMENT prevents a stale panel's IDs naming later notices.
pub(crate) const SCHEMA: &str = "
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
    /// Serialize a host's reads and deletes across all clients of this helm.
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
fn read_notices(conn: &Connection) -> anyhow::Result<Vec<Notice>> {
    let mut statement = conn.prepare(
        "SELECT id, host_id, raised_at, read, resolved, content FROM notices ORDER BY raised_at DESC, id DESC",
    )?;
    let rows = statement.query_map([], |row| {
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
    let previous = read_notices(&tx)?
        .into_iter()
        .filter(|notice| notice.host == claim.host)
        .map(|notice| {
            let NoticeContent::StuckTrash { checkout_id, .. } = &notice.content;
            (checkout_id.clone(), notice)
        })
        .collect::<HashMap<_, _>>();
    let present: HashSet<&str> = listing
        .checkouts
        .iter()
        .map(|entry| entry.id.as_str())
        .chain(listing.issues.iter().map(|issue| issue.id.as_str()))
        .collect();
    let raised_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs().min(i64::MAX as u64) as i64);
    let mut changed = false;
    for entry in &listing.checkouts {
        let old = previous.get(&entry.id);
        let known_bytes = old.and_then(|notice| {
            let NoticeContent::StuckTrash { bytes, .. } = notice.content;
            bytes
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
                let NoticeContent::StuckTrash { bytes, .. } = &mut content;
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
    /// Return current notices without consulting host reachability.
    pub(crate) async fn notices(&self) -> anyhow::Result<Notices> {
        self.conn()
            .call("notice read task panicked", |conn| {
                Ok(Notices {
                    notices: read_notices(conn)?,
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
                    MarkAction::Read => "UPDATE notices SET read = 1 WHERE id = ?1 AND read = 0",
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

    /// Exact marks must stay safe when another client has removed a displayed
    /// notice or a new failure arrives before an old panel closes.
    async fn mark(harness: &rest_harness::Harness, action: &str, ids: &[i64]) {
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
        } = updated.content;
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
            "cancellation cannot lose the committed observation's wake"
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
        assert!(
            delete.as_mut().now_or_never().is_none(),
            "Delete shares list ordering"
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
        assert_eq!(
            delete
                .now_or_never()
                .expect("next caller proceeds after the first releases")
                .into_response()
                .status(),
            StatusCode::CONFLICT
        );
    }

    /// The real HTTP/RPC seam records notices and wakes other clients after an
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

//! A durable per-helm queue, independent of what produced an announcement.
//!
//! Content is copied into the queue once and retained after dismissal. A later
//! producer run with the same id cannot replace text a user already received or
//! make a shown announcement pending again. Clients submit only the ids their
//! dialog showed, so dismissing one window never drains unseen, newer entries.

use crate::store::HelmStore;
use crate::{AppState, http_error};
use anyhow::Context;
use axum::extract::State;
use axum::response::IntoResponse;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Shared by fresh creation and migration so both paths have identical tables.
/// The creation-build row is inserted only by fresh schema creation. Queue
/// sequence numbers, unlike clocks, also order a batch enqueued at one instant.
pub(crate) const SCHEMA: &str = "
    CREATE TABLE helm_creation_build (
        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
        build TEXT NOT NULL
    ) STRICT;
    CREATE TABLE announcements (
        sequence INTEGER PRIMARY KEY AUTOINCREMENT,
        id TEXT NOT NULL UNIQUE,
        title TEXT NOT NULL,
        body TEXT NOT NULL,
        detail TEXT,
        link_label TEXT,
        link_url TEXT,
        shown_at INTEGER,
        CHECK ((link_label IS NULL) = (link_url IS NULL))
    ) STRICT;
";

/// Display-ready content, kept unchanged for the lifetime of its id.
/// Optional detail and link have no source-specific meaning to the queue:
/// producers supply their wording and destination, and clients render them.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct Announcement {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) detail: Option<String>,
    pub(crate) link: Option<AnnouncementLink>,
}

/// One optional, labelled destination supplied by the announcement's producer.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct AnnouncementLink {
    pub(crate) label: String,
    pub(crate) url: String,
}

/// One consistent queue snapshot. Pending runs oldest insertion first;
/// history runs newest shown time first, then newest insertion first.
#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct AnnouncementQueue {
    pub(crate) pending: Vec<Announcement>,
    pub(crate) history: Vec<Announcement>,
}

impl HelmStore {
    /// The build that created this database, never the build that migrated it.
    /// No row means the database predates recording this fact. Producers decide
    /// what that absence, or an unreleased creation build, means for them.
    pub(crate) async fn creation_build(&self) -> anyhow::Result<Option<String>> {
        self.conn()
            .call("creation build read task panicked", |conn| {
                conn.query_row(
                    "SELECT build FROM helm_creation_build WHERE singleton = 1",
                    [],
                    |row| row.get(0),
                )
                .optional()
                .context("reading the helm database's creation build")
            })
            .await
    }

    /// Append previously unknown ids in the producer's order, atomically.
    /// Duplicate ids are permanent no-ops, including after dismissal: their
    /// original content, insertion order and shown time all remain unchanged.
    pub(crate) async fn enqueue_announcements(
        &self,
        announcements: Vec<Announcement>,
    ) -> anyhow::Result<()> {
        self.conn()
            .call("announcement enqueue task panicked", move |conn| {
                let tx = conn
                    .transaction()
                    .context("beginning announcement enqueue")?;
                for announcement in announcements {
                    let (link_label, link_url) = match announcement.link {
                        Some(link) => (Some(link.label), Some(link.url)),
                        None => (None, None),
                    };
                    tx.execute(
                        "INSERT INTO announcements (id, title, body, detail, link_label, link_url)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT (id) DO NOTHING",
                        rusqlite::params![
                            announcement.id,
                            announcement.title,
                            announcement.body,
                            announcement.detail,
                            link_label,
                            link_url,
                        ],
                    )
                    .context("enqueueing an announcement")?;
                }
                tx.commit().context("committing announcement enqueue")
            })
            .await
    }

    /// Read pending and history from the same SQLite snapshot. An announcement
    /// cannot disappear between two reads when another connection dismisses it.
    pub(crate) async fn announcements(&self) -> anyhow::Result<AnnouncementQueue> {
        self.conn()
            .call("announcement read task panicked", |conn| {
                let tx = conn.transaction().context("beginning announcement read")?;
                let queue = AnnouncementQueue {
                    pending: read_list(
                        &tx,
                        "SELECT id, title, body, detail, link_label, link_url FROM announcements
                         WHERE shown_at IS NULL ORDER BY sequence ASC",
                    )?,
                    history: read_list(
                        &tx,
                        "SELECT id, title, body, detail, link_label, link_url FROM announcements
                         WHERE shown_at IS NOT NULL ORDER BY shown_at DESC, sequence DESC",
                    )?,
                };
                tx.commit().context("finishing announcement read")?;
                Ok(queue)
            })
            .await
    }

    /// Move only the named pending entries into shared history, as one batch.
    /// Replays and unknown ids do nothing. In particular, reopening history or
    /// dismissing an older window cannot refresh an existing shown timestamp.
    pub(crate) async fn mark_announcements_shown(&self, ids: Vec<String>) -> anyhow::Result<()> {
        self.conn()
            .call("announcement dismissal task panicked", move |conn| {
                // Wall time describes when the user dismissed this batch;
                // sequence breaks ties without requiring a different clock tick.
                let shown_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |elapsed| {
                        elapsed.as_millis().min(i64::MAX as u128) as i64
                    });
                let tx = conn
                    .transaction()
                    .context("beginning announcement dismissal")?;
                for id in ids {
                    tx.execute(
                        "UPDATE announcements SET shown_at = ?1 WHERE id = ?2 AND shown_at IS NULL",
                        rusqlite::params![shown_at, id],
                    )
                    .context("marking an announcement shown")?;
                }
                tx.commit().context("committing announcement dismissal")
            })
            .await
    }
}

/// Decode either list using the caller's snapshot and explicit ordering.
/// The schema's paired-null constraint makes a partially stored link invalid.
fn read_list(conn: &Connection, sql: &str) -> anyhow::Result<Vec<Announcement>> {
    let mut statement = conn.prepare(sql).context("preparing announcement read")?;
    statement
        .query_map([], |row| {
            let label: Option<String> = row.get(4)?;
            let url: Option<String> = row.get(5)?;
            Ok(Announcement {
                id: row.get(0)?,
                title: row.get(1)?,
                body: row.get(2)?,
                detail: row.get(3)?,
                link: label
                    .zip(url)
                    .map(|(label, url)| AnnouncementLink { label, url }),
            })
        })
        .context("reading announcements")?
        .collect::<Result<Vec<_>, _>>()
        .context("decoding announcements")
}

/// `GET /api/announcements` reads both lists behind ordinary device auth.
pub(crate) async fn get(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.store.announcements().await {
        Ok(queue) => axum::Json(queue).into_response(),
        Err(error) => http_error(error),
    }
}

/// The exact set the dialog displayed; omission never means "drain everything".
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShownAnnouncements {
    ids: Vec<String>,
}

/// `POST /api/announcements/shown` records a dismissal on this helm.
/// A failed write leaves the queue intact and returns an ordinary error; the
/// client owns the best-effort policy that still closes the dialog.
pub(crate) async fn shown(
    State(state): State<Arc<AppState>>,
    axum::Json(body): axum::Json<ShownAnnouncements>,
) -> impl IntoResponse {
    match state.store.mark_announcements_shown(body.ids).await {
        Ok(()) => {
            // Moves entries from pending to history, which a connected GUI
            // learns only through the feed. Unconditional for the same reason
            // as the ordinary preference write: the store cannot tell a no-op,
            // and a dismissal is a rare user action, never periodic.
            state.manager.events().bump();
            axum::http::StatusCode::NO_CONTENT.into_response()
        }
        Err(error) => http_error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest_harness;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// Non-release content exercises the queue's source-independent contract.
    fn announcement(id: &str) -> Announcement {
        Announcement {
            id: id.into(),
            title: format!("Notice {id}"),
            body: "First paragraph.\n\nSecond paragraph.".into(),
            detail: Some("An optional explanation".into()),
            link: Some(AnnouncementLink {
                label: "Learn more".into(),
                url: "https://example.invalid/notice".into(),
            }),
        }
    }

    /// Test through the real protected router, including JSON extraction.
    fn request(method: &str, path: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(path)
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    /// A bounded HTTP read pins the serialized wire shape, rather than only
    /// checking the same Rust values that the store already returns.
    async fn read(harness: &rest_harness::Harness) -> serde_json::Value {
        let response = harness
            .router()
            .oneshot(request(
                "GET",
                "/api/announcements",
                serde_json::Value::Null,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 16 * 1024)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// Repeated producer runs, later wording edits and repeated dismissals must
    /// never rewrite what this helm received, even across database reopenings.
    #[farhelm_testtrace::test]
    async fn announcement_queue_is_durable_and_duplicate_ids_are_immutable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        assert_eq!(
            store.announcements().await.unwrap(),
            AnnouncementQueue::default()
        );
        let first = announcement("first");
        let second = announcement("second");
        store
            .enqueue_announcements(vec![first.clone(), second.clone()])
            .await
            .unwrap();
        assert_eq!(
            store.announcements().await.unwrap().pending,
            [first.clone(), second.clone()]
        );
        let mut rewritten = first.clone();
        rewritten.title = "A later edit must not replace the original".into();
        rewritten.body = "Changed body".into();
        rewritten.detail = None;
        rewritten.link = None;
        store
            .enqueue_announcements(vec![rewritten.clone()])
            .await
            .unwrap();
        assert_eq!(store.announcements().await.unwrap().pending[0], first);

        store
            .mark_announcements_shown(vec![first.id.clone()])
            .await
            .unwrap();
        let db = store.connection_for_test();
        let recorded: i64 = db
            .lock()
            .query_row(
                "SELECT shown_at FROM announcements WHERE id = 'first'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(recorded > 0, "dismissal records a wall-clock shown time");
        // A known earlier time makes an accidental rewrite observable without
        // sleeping for the wall clock to advance between two dismissals.
        db.lock()
            .execute(
                "UPDATE announcements SET shown_at = 123 WHERE id = 'first'",
                [],
            )
            .unwrap();
        store.enqueue_announcements(vec![rewritten]).await.unwrap();
        store
            .mark_announcements_shown(vec!["first".into(), "missing".into(), "first".into()])
            .await
            .unwrap();
        let unchanged: i64 = db
            .lock()
            .query_row(
                "SELECT shown_at FROM announcements WHERE id = 'first'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unchanged, 123, "neither replay changes a history timestamp");
        drop(db);
        drop(store);

        let reopened = HelmStore::open(&path).await.unwrap();
        assert_eq!(
            reopened.announcements().await.unwrap(),
            AnnouncementQueue {
                pending: vec![second],
                history: vec![first],
            }
        );
    }

    /// Shown time outranks enqueue order across dismissals, while sequence is
    /// the deterministic newest-first order inside a single dismissed batch.
    #[farhelm_testtrace::test]
    async fn announcement_history_orders_batches_by_time_then_insertion() {
        let dir = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let entries: Vec<_> = ["a", "b", "c", "d"].into_iter().map(announcement).collect();
        store.enqueue_announcements(entries.clone()).await.unwrap();
        assert_eq!(store.announcements().await.unwrap().pending, entries);
        store
            .mark_announcements_shown(vec!["b".into(), "c".into(), "d".into()])
            .await
            .unwrap();
        let queue = store.announcements().await.unwrap();
        assert_eq!(queue.pending, [announcement("a")]);
        assert_eq!(
            queue.history,
            [announcement("d"), announcement("c"), announcement("b")]
        );
        let db = store.connection_for_test();
        db.lock()
            .execute(
                "UPDATE announcements SET shown_at = 1 WHERE shown_at IS NOT NULL",
                [],
            )
            .unwrap();
        store
            .mark_announcements_shown(vec!["a".into()])
            .await
            .unwrap();
        assert_eq!(
            store.announcements().await.unwrap().history,
            [
                announcement("a"),
                announcement("d"),
                announcement("c"),
                announcement("b"),
            ]
        );
    }

    /// A rejected row must roll back the entire dismissal; a best-effort
    /// client can close safely because every shown id remains pending to retry.
    #[farhelm_testtrace::test]
    async fn announcement_dismissal_failure_keeps_the_whole_batch_pending() {
        let harness = rest_harness::idle_helm().await;
        let entries = vec![announcement("a"), announcement("b")];
        harness
            .store
            .enqueue_announcements(entries.clone())
            .await
            .unwrap();
        assert_eq!(
            harness.store.announcements().await.unwrap().pending,
            entries
        );
        harness
            .store
            .connection_for_test()
            .lock()
            .execute_batch(
                "CREATE TRIGGER refuse_second_dismissal BEFORE UPDATE ON announcements
             WHEN OLD.id = 'b' BEGIN SELECT RAISE(ABORT, 'refused dismissal'); END;",
            )
            .unwrap();
        let response = harness
            .router()
            .oneshot(request(
                "POST",
                "/api/announcements/shown",
                serde_json::json!({"ids":["a", "b"]}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            harness.store.announcements().await.unwrap(),
            AnnouncementQueue {
                pending: entries,
                history: vec![],
            }
        );
    }

    /// Clients may dismiss only what they displayed. A newer queued entry
    /// survives, and the same route exposes shared history to another client.
    #[farhelm_testtrace::test]
    async fn announcement_routes_expose_display_content_and_dismiss_named_ids() {
        let harness = rest_harness::idle_helm().await;
        assert_eq!(
            read(&harness).await,
            serde_json::json!({"pending":[],"history":[]})
        );
        let mut plain = announcement("plain");
        plain.detail = None;
        plain.link = None;
        harness
            .store
            .enqueue_announcements(vec![announcement("first"), plain.clone()])
            .await
            .unwrap();
        assert_eq!(
            read(&harness).await,
            serde_json::json!({
                "pending":[{
                    "id":"first", "title":"Notice first", "body":"First paragraph.\n\nSecond paragraph.",
                    "detail":"An optional explanation", "link":{"label":"Learn more","url":"https://example.invalid/notice"}
                }, {"id":"plain", "title":"Notice plain", "body":"First paragraph.\n\nSecond paragraph.", "detail":null, "link":null}],
                "history":[]
            })
        );
        for ids in [
            serde_json::json!(["first"]),
            serde_json::json!(["first", "missing", "first"]),
        ] {
            let response = harness
                .router()
                .oneshot(request(
                    "POST",
                    "/api/announcements/shown",
                    serde_json::json!({"ids":ids}),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            assert!(
                axum::body::to_bytes(response.into_body(), 1)
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
        assert_eq!(
            read(&harness).await,
            serde_json::json!({
                "pending":[plain], "history":[announcement("first")]
            })
        );
    }

    /// Both read and write belong behind device authentication. Refused JSON
    /// must not silently become a broad dismissal or change the durable queue.
    #[farhelm_testtrace::test]
    async fn announcement_routes_require_auth_and_an_explicit_id_list() {
        let harness = rest_harness::idle_helm().await;
        harness
            .store
            .enqueue_announcements(vec![announcement("a")])
            .await
            .unwrap();
        for (method, path) in [
            ("GET", "/api/announcements"),
            ("POST", "/api/announcements/shown"),
        ] {
            let response = harness
                .unauthenticated_router()
                .oneshot(request(method, path, serde_json::json!({"ids":["a"]})))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
        for invalid in [
            serde_json::json!({}),
            serde_json::json!({"ids":null}),
            serde_json::json!({"ids":[1]}),
            serde_json::json!({"ids":["a"],"all":true}),
        ] {
            let response = harness
                .router()
                .oneshot(request("POST", "/api/announcements/shown", invalid))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        }
        let response = harness
            .router()
            .oneshot(request(
                "POST",
                "/api/announcements/shown",
                serde_json::json!({"ids":[]}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            harness.store.announcements().await.unwrap(),
            AnnouncementQueue {
                pending: vec![announcement("a")],
                history: vec![],
            }
        );
    }
}

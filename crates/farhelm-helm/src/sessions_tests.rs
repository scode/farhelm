//! Session tests live in a sibling file because they occupy three quarters of the module.
//!
//! `#[path]` keeps this as `sessions::tests`, preserving private-item access without widening
//! production visibility.

use super::{resolve_owner, resolve_session_profiles_from_store, store};
use crate::rest_harness::{self, WsTestClient, silent_supervisor};
use std::time::Duration;

/// Raw session rows do not read or decode the profile catalog.
///
/// The store's public writers reject malformed profiles, so this fixture
/// plants one through SQLite as a damaged database could. A catalog read
/// would fail on its unknown agent kind; successful resolution therefore
/// proves the raw-only early return happens before any store access.
#[tokio::test]
async fn raw_only_profile_resolution_ignores_a_corrupt_catalog_row() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("helm.db");
    let store = store::HelmStore::open(&db).await.expect("open store");
    {
        let conn = rusqlite::Connection::open(&db).expect("open corruption fixture");
        conn.execute(
            "INSERT INTO profiles (id, name, invocation, agent_kind, resume_template) \
             VALUES ('corrupt-profile', 'broken', 'agent', 'unknown', NULL)",
            [],
        )
        .expect("plant corrupt profile");
    }
    assert!(
        store.profiles().await.is_err(),
        "the fixture must fail a real catalog read"
    );

    let mut sessions = vec![rest_harness::session("raw", 1)];
    sessions[0].source_profile = None;
    resolve_session_profiles_from_store(&store, &mut sessions)
        .await
        .expect("raw rows do not need the catalog");
    assert_eq!(sessions[0].source_profile, None);
}

/// `POST /api/sessions` end to end through the real axum handler and
/// middleware stack, with a scripted supervisor peer standing in for
/// `farhelm-supervisor`.
///
/// PLAN_M1.md makes this endpoint the single creation path: every
/// caller — the M1 CLI flags today, the M2 GUI session-creation dialog
/// next — lands on the same API, never bypassing it. Despite that, no
/// *successful* request previously exercised the handler: the
/// Playwright e2e suite deliberately covers only a failure path
/// (create in a nonexistent working directory), and the Rust e2e
/// tests call `SupervisorClient::create_session` directly, which
/// bypasses both the handler and the `CreateReq` struct's
/// `#[serde(default)]` cols/rows fields entirely. That left the 80x24
/// default — the size an agent's first output wraps to before any
/// browser has attached and reported a real size — pinned nowhere.
/// This test closes that gap: it omits cols/rows/title from the
/// request body, asserts the peer received exactly the defaults, and
/// checks the JSON reply shape a caller actually depends on.
///
/// This same minimal body is also the pre-M3 caller posture for
/// `agent_kind`/`resume_template` (PLAN_M3.md item 7): the UI and CLI
/// currently send neither field, so this test also pins that an
/// absent override decodes and forwards as `None` rather than
/// inventing a value — the fields are deliberately accepted here for
/// non-UI API callers that basename recognition cannot classify, not
/// because every production caller is expected to omit them forever.
#[tokio::test]
async fn create_session_request_with_omitted_dimensions_uses_80x24_defaults() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame, SessionInfo};
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession {
            req_id,
            parent: None,
            profile_name: None,
            cwd,
            invocation,
            // Bound rather than swept into the `..` below: which MODE
            // the helm forwards is exactly what the assertions here are
            // about, and a `..` would let a future change start sending
            // a profile selection alongside the invocation — the
            // ambiguous request the supervisor refuses — with this test
            // still green.
            source_profile,
            title,
            cols,
            rows,
            agent_kind,
            resume_template,
            // Not under test here (the assertions below only check
            // cwd/invocation/profile/title/cols/rows/agent_kind/
            // resume_template); PLAN_M3.md's `intent_key` is exercised
            // by `create_session_forwards_the_bodys_extras_to_the_supervisor`
            // instead.
            ..
        } = request
        else {
            panic!("expected CreateSession, got {request:?}");
        };
        // The contract under test: a caller that omits cols/rows must
        // still reach the supervisor with the documented 80x24
        // defaults. (Without the serde defaults the request would not
        // reach the supervisor at all — axum rejects a body missing
        // non-optional fields during deserialization.)
        assert_eq!((cols, rows), (80, 24), "serde defaults must be 80x24");
        assert_eq!(cwd, "/some/dir");
        // The raw mode has no source snapshot. Profile-backed creates are
        // resolved by the helm and carry both an invocation and snapshot.
        assert_eq!(invocation, Some("some-agent".to_string()));
        assert_eq!(
            source_profile, None,
            "the raw mode must not claim catalog provenance"
        );
        assert_eq!(title, None);
        assert_eq!(agent_kind, None);
        assert_eq!(resume_template, None);
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: SessionInfo {
                    parent: None,
                    archived: false,
                    id: "sess-1".into(),
                    title: "some-agent".into(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    creation_seq: None,
                    cwd: "/some/dir".into(),
                    invocation: "some-agent".into(),
                    // Matches real `create_session` output: `Unknown`,
                    // not a live status (creation does not establish the
                    // agent's later exec succeeded).
                    status: farhelm_proto::SessionStatus::Unknown,
                    annotation: None,
                    restart_offer: farhelm_proto::RestartOffer::default(),
                    tabs: Vec::new(),
                    source_profile: None,
                },
            }))
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({"cwd": "/some/dir", "invocation": "some-agent"}).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let session: SessionInfo = serde_json::from_slice(&body).unwrap();
    assert_eq!(session.id, "sess-1");
    assert_eq!(session.cwd, "/some/dir");

    peer.await.unwrap();
}

/// The create body's `intent_key`, `agent_kind`, and `resume_template`
/// all reach the supervisor verbatim (PLAN_M3.md items 6 and 7).
///
/// Worth its own test because the helm is a pure pass-through here and
/// pass-throughs are exactly what silently stop passing things
/// through: nothing else in this crate would notice if a field were
/// dropped, and for `intent_key` specifically the symptom in production
/// would not be an error but a SECOND session appearing on a retry —
/// the failure the whole feature exists to prevent, visible only under
/// a lost reply.
#[tokio::test]
async fn create_session_forwards_the_bodys_extras_to_the_supervisor() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{AgentKind, ControlMsg, Frame, SessionInfo};
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession {
            req_id,
            parent: None,
            profile_name: None,
            intent_key,
            agent_kind,
            resume_template,
            ..
        } = request
        else {
            panic!("expected CreateSession, got {request:?}");
        };
        assert_eq!(
            intent_key.as_deref(),
            Some("intent-from-the-browser"),
            "the key belongs to whoever can retry, so it must arrive unaltered"
        );
        assert_eq!(agent_kind, Some(AgentKind::Claude));
        assert_eq!(
            resume_template,
            Some(vec!["claude".to_string(), "{conversation}".to_string()])
        );
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: SessionInfo {
                    parent: None,
                    archived: false,
                    id: "sess-1".into(),
                    title: "t".into(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    creation_seq: None,
                    cwd: "/some/dir".into(),
                    invocation: "some-agent".into(),
                    status: farhelm_proto::SessionStatus::Unknown,
                    annotation: None,
                    restart_offer: farhelm_proto::RestartOffer::default(),
                    tabs: Vec::new(),
                    source_profile: None,
                },
            }))
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({
                "cwd": "/some/dir",
                "invocation": "some-agent",
                "intent_key": "intent-from-the-browser",
                "agent_kind": "claude",
                "resume_template": ["claude", "{conversation}"],
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    peer.await.unwrap();
}

/// `POST /api/sessions/{id}/stop` end to end: a scripted peer replies
/// `SessionStopped`, and the route must answer 200 with an empty JSON
/// object — the uniform success body `stop`/`delete` share so a caller
/// does not need to special-case "no content".
#[tokio::test]
async fn stop_session_happy_path_returns_200_with_empty_object_body() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::StopSession { req_id, session_id } = request else {
            panic!("expected StopSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        writer
            .write_control(&ControlMsg::SessionStopped { req_id })
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions/sess-1/stop")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value, serde_json::json!({}));

    peer.await.unwrap();
}

/// Stopping an unknown id must surface as a 404 carrying the
/// helm's OWN message, without the request ever reaching a supervisor.
///
/// This contract INVERTED with PLAN_M6.md item 5, and the inversion is
/// the point of keeping the test. Before owner routing, an unknown id
/// was the supervisor's question to answer, and this test pinned the
/// verbatim passthrough of its 404. Now the helm resolves a session's
/// owning host in its merged view first, so an id nobody owns has no
/// host to ask — answering it locally is not an optimization but the
/// only honest thing available, since "which supervisor would you even
/// forward this to" has no answer.
///
/// Both halves are asserted: the status and body a caller sees, and —
/// through [`silent_supervisor`] — that the connected host was not
/// asked. Without the second half a helm that forwarded the request AND
/// answered locally would pass.
#[tokio::test]
async fn stop_session_unknown_id_returns_404_with_supervisor_message() {
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(silent_supervisor(peer_side));

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions/sess-missing/stop")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&body),
        "no such session: sess-missing",
        "the helm's own refusal must name the id it could not place"
    );

    peer.await.unwrap();
}

/// `DELETE /api/sessions/{id}` happy path, mirroring the stop test
/// above: a scripted `SessionDeleted` reply must reach the caller as
/// 200 with the same empty-object body shape.
#[tokio::test]
async fn delete_session_happy_path_returns_200_with_empty_object_body() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession { req_id, session_id } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        writer
            .write_control(&ControlMsg::SessionDeleted { req_id })
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("DELETE")
        .uri("/api/sessions/sess-1")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value, serde_json::json!({}));

    peer.await.unwrap();
}

/// Deleting an unknown id must 404 from the helm's own owner lookup,
/// the delete-side twin of
/// `stop_session_unknown_id_returns_404_with_supervisor_message` — see
/// that test's docs for why this contract inverted with M6's routing,
/// and why the silent supervisor is half the assertion.
#[tokio::test]
async fn delete_session_unknown_id_returns_404_with_supervisor_message() {
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(silent_supervisor(peer_side));

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("DELETE")
        .uri("/api/sessions/sess-missing")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&body),
        "no such session: sess-missing",
        "the helm's own refusal must name the id it could not place"
    );

    peer.await.unwrap();
}

/// A successful delete also drops the session's `session_seen` row
/// (SPEC_impl.md's `session_seen` paragraph), not merely the session itself — a
/// stray row would sit in the table forever with nothing to ever read it
/// back out, since the id it names is gone. Mirrors
/// `delete_session_happy_path_returns_200_with_empty_object_body`'s
/// scripted-peer shape; the only addition is marking the session seen
/// before the delete and checking the store directly afterward, since the
/// REST reply carries no evidence either way (SPEC.md's "delete" leaves
/// nothing behind to inspect through the API once the session is gone).
#[tokio::test]
async fn delete_session_drops_the_seen_row() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession { req_id, session_id } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        writer
            .write_control(&ControlMsg::SessionDeleted { req_id })
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    harness
        .store
        .mark_seen("sess-1", 1_700_000_000)
        .await
        .unwrap();

    let app = harness.router();
    let request = axum::http::Request::builder()
        .method("DELETE")
        .uri("/api/sessions/sess-1")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    assert!(
        harness
            .store
            .seen_activity(&["sess-1".to_string()])
            .await
            .unwrap()
            .is_empty(),
        "the seen-state row must not survive the session it names"
    );

    peer.await.unwrap();
}

/// `PUT /api/sessions/{id}/seen` end to end: a mark, a listing read, a
/// clear, and another listing read, checking `seen_activity_at` at each
/// step — the round trip the idle dot's grey/blue split actually depends
/// on (SPEC.md, Status).
#[tokio::test]
async fn mark_seen_route_records_and_clears_the_stamp() {
    let harness =
        rest_harness::helm_listing(vec![rest_harness::session("sess-1", 1_700_000_000)]).await;

    let (status, body) = put_json(
        &harness,
        "/api/sessions/sess-1/seen",
        serde_json::json!({ "seen_activity_at": 1_700_000_000 }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body, serde_json::json!({}));

    let (_, listing) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        listing["sessions"][0]["seen_activity_at"], 1_700_000_000,
        "a mark must be visible on the very next listing read"
    );

    let (status, _) = put_json(
        &harness,
        "/api/sessions/sess-1/seen",
        serde_json::json!({ "seen_activity_at": null }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let (_, listing) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        listing["sessions"][0]["seen_activity_at"],
        serde_json::Value::Null,
        "a clear (mark unread) must read back as null, not merely absent \
         or reverted to some prior value"
    );
}

/// The existence check is the same helm-wide "no host has ever reported
/// this id" lookup every other per-session route uses
/// ([`super::resolve_owner`]) — a session nothing knows about is a 404
/// whether or not any host is even connected.
#[tokio::test]
async fn mark_seen_route_unknown_id_returns_404() {
    let harness = rest_harness::idle_helm().await;
    let (status, body) = put_json(
        &harness,
        "/api/sessions/sess-missing/seen",
        serde_json::json!({ "seen_activity_at": 100 }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    assert_eq!(
        body,
        serde_json::json!("no such session: sess-missing"),
        "the refusal must name the id it could not place, like every \
         other per-session 404"
    );
}

/// The fleet-events revision moves on a genuine change and stands still on
/// a repeat of the SAME write (SPEC_impl.md's "bump the fleet-events
/// revision on a change, not on a no-op" rule) — the session view's
/// auto-mark effect can reissue this exact PUT with no new activity behind
/// it (reopening a session, or a staleness/capability transition —
/// `session_view.rs`'s `mark_key`), and a bump on every one of those would
/// wake every other connected client for nothing.
#[tokio::test]
async fn mark_seen_route_bumps_on_change_and_not_on_a_repeat() {
    let harness =
        rest_harness::helm_listing(vec![rest_harness::session("sess-1", 1_700_000_000)]).await;

    let before_first_mark = harness.manager.events().revision();
    let (status, _) = put_json(
        &harness,
        "/api/sessions/sess-1/seen",
        serde_json::json!({ "seen_activity_at": 1_700_000_000 }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert!(
        harness.manager.events().revision() > before_first_mark,
        "the first mark is a real change and must bump"
    );

    let before_repeat = harness.manager.events().revision();
    let (status, _) = put_json(
        &harness,
        "/api/sessions/sess-1/seen",
        serde_json::json!({ "seen_activity_at": 1_700_000_000 }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        harness.manager.events().revision(),
        before_repeat,
        "re-marking the same stamp is a no-op and must not bump"
    );

    let before_clear = harness.manager.events().revision();
    let (status, _) = put_json(
        &harness,
        "/api/sessions/sess-1/seen",
        serde_json::json!({ "seen_activity_at": null }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert!(
        harness.manager.events().revision() > before_clear,
        "clearing a real stamp is also a genuine change and must bump"
    );

    let before_repeat_clear = harness.manager.events().revision();
    let (status, _) = put_json(
        &harness,
        "/api/sessions/sess-1/seen",
        serde_json::json!({ "seen_activity_at": null }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        harness.manager.events().revision(),
        before_repeat_clear,
        "clearing an already-clear stamp is the no-op's mirror image and \
         must not bump either"
    );
}

/// An OMITTED `seen_activity_at` key must be rejected outright, not
/// silently treated as `null` (a clear/"mark unread") — see
/// `require_present_seen_activity_at`'s doc for why a plain `Option<i64>`
/// field would otherwise swallow a missing key into `None`. A malformed or
/// truncated client request must not be able to clear a session's seen
/// state by accident, and must leave the store and the fleet-events
/// revision exactly as they were.
#[tokio::test]
async fn mark_seen_route_rejects_a_body_with_the_key_omitted() {
    let harness =
        rest_harness::helm_listing(vec![rest_harness::session("sess-1", 1_700_000_000)]).await;
    harness
        .store
        .mark_seen("sess-1", 1_700_000_000)
        .await
        .unwrap();
    let before = harness.manager.events().revision();

    let (status, _) = put_json(&harness, "/api/sessions/sess-1/seen", serde_json::json!({})).await;
    assert_eq!(
        status,
        // axum 0.8's `JsonRejection::JsonDataError` for a body that parses
        // as JSON but fails to deserialize into the target struct — the
        // same 422 `RenameReq`'s own missing-field doc names, distinct
        // from the 400 a body that is not even valid JSON gets.
        axum::http::StatusCode::UNPROCESSABLE_ENTITY,
        "an omitted key is a malformed request, not an implicit clear"
    );
    assert_eq!(
        harness
            .store
            .seen_activity(&["sess-1".to_string()])
            .await
            .unwrap()
            .get("sess-1"),
        Some(&1_700_000_000),
        "the stored stamp must be untouched by the refused request"
    );
    assert_eq!(
        harness.manager.events().revision(),
        before,
        "a refused request must not bump fleet-events either"
    );
}

/// `PUT /api/sessions/{id}/seen` on a session whose host is unreachable
/// still succeeds — the whole point of NOT routing this write through
/// `route_session` (see `mark_seen`'s own doc). A wrong implementation
/// that reused `route_session`'s reachability gate would refuse this
/// request instead of returning 200 and storing the write, which is
/// exactly what this test would catch.
#[tokio::test]
async fn mark_seen_route_succeeds_on_an_unreachable_host() {
    let (builder, host) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@breaks",
            rest_harness::HostScript {
                identity: Some("identity-original".to_string()),
                sessions: vec![rest_harness::session("owned", 1_700_000_000)],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    harness.await_refreshed(host).await;
    harness.fleet.take_down(host);
    harness
        .await_state(host, |state| state.phase() == "unreachable-reprobing")
        .await;

    let before = harness.manager.events().revision();
    let (status, body) = put_json(
        &harness,
        "/api/sessions/owned/seen",
        serde_json::json!({ "seen_activity_at": 1_700_000_000 }),
    )
    .await;
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "an unreachable host must not block a helm-local write"
    );
    assert_eq!(body, serde_json::json!({}));
    assert!(
        harness.manager.events().revision() > before,
        "the write is real and must still bump fleet-events"
    );
    assert_eq!(
        harness
            .store
            .seen_activity(&["owned".to_string()])
            .await
            .unwrap()
            .get("owned"),
        Some(&1_700_000_000),
        "the stamp must be durably stored despite the host being down"
    );

    let before_clear = harness.manager.events().revision();
    let (status, _) = put_json(
        &harness,
        "/api/sessions/owned/seen",
        serde_json::json!({ "seen_activity_at": null }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert!(harness.manager.events().revision() > before_clear);
    assert!(
        harness
            .store
            .seen_activity(&["owned".to_string()])
            .await
            .unwrap()
            .is_empty(),
        "a clear must also land while the host is down"
    );
}

/// The single-session detail route (`GET /api/sessions/{id}`) carries
/// `seen_activity_at` on a LIVE reply, matching the merged listing's own
/// field (`aggregate::row_of`) — a direct fetch (the recovery path
/// `list::ListView`'s remembered-selection resolver uses) must see the
/// same seen-state a listing read would have shown.
#[tokio::test]
async fn get_session_route_carries_seen_activity_at_when_live() {
    let harness =
        rest_harness::helm_listing(vec![rest_harness::session("sess-1", 1_700_000_000)]).await;

    let (status, detail) = get_json(&harness, "/api/sessions/sess-1").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        detail["seen_activity_at"],
        serde_json::Value::Null,
        "an id nothing has marked reads back null, not merely absent"
    );

    harness
        .store
        .mark_seen("sess-1", 1_700_000_000)
        .await
        .unwrap();
    let (status, detail) = get_json(&harness, "/api/sessions/sess-1").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(detail["seen_activity_at"], 1_700_000_000);
}

/// The same field on the STALE branch (host down, answered from the
/// cache) — `get_session` constructs a separate `SessionRow` literal for
/// each branch (see its own doc), so a fix that carries the field on the
/// live branch is not evidence the stale one does too.
#[tokio::test]
async fn get_session_route_carries_seen_activity_at_when_stale() {
    let (builder, host) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@breaks",
            rest_harness::HostScript {
                identity: Some("identity-original".to_string()),
                sessions: vec![rest_harness::session("owned", 1_700_000_000)],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    harness.await_refreshed(host).await;
    harness
        .store
        .mark_seen("owned", 1_700_000_000)
        .await
        .unwrap();
    harness.fleet.take_down(host);
    harness
        .await_state(host, |state| state.phase() == "unreachable-reprobing")
        .await;

    let (status, detail) = get_json(&harness, "/api/sessions/owned").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(detail["stale"], true);
    assert_eq!(detail["seen_activity_at"], 1_700_000_000);
}

/// A failed `clear_seen` must not fail the delete: SPEC.md's "delete
/// removes the session" cannot be held hostage by a local bookkeeping
/// table that has nothing to do with the session's real, host-side
/// removal — see `delete_session`'s own doc for why that call is
/// best-effort. `break_session_seen_table_for_test` makes the failure
/// real (the table is genuinely gone) rather than simulated, without a
/// mock trait or an env var.
#[tokio::test]
async fn delete_session_succeeds_even_when_clearing_the_seen_row_fails() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession { req_id, session_id } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        writer
            .write_control(&ControlMsg::SessionDeleted { req_id })
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    harness
        .store
        .mark_seen("sess-1", 1_700_000_000)
        .await
        .unwrap();
    harness
        .store
        .break_session_seen_table_for_test()
        .await
        .expect("drop the table for the fixture");

    let app = harness.router();
    let request = axum::http::Request::builder()
        .method("DELETE")
        .uri("/api/sessions/sess-1")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::OK,
        "a local bookkeeping failure must not surface as a failed delete"
    );

    let (status, _) = get_json(&harness, "/api/sessions/sess-1").await;
    assert_eq!(
        status,
        axum::http::StatusCode::NOT_FOUND,
        "the manager-side forget must still run — the session is gone \
         from the helm's own records even though clearing its seen row \
         failed"
    );

    peer.await.unwrap();
}

// ===== `POST /api/sessions/{id}/replace` (SPEC.md's "replace") ====
//
// One route composed of a create and a delete on the SAME connection, so
// the tests below exercise it as that composition rather than as a new
// primitive: each scripted peer answers a `CreateSession` and then a
// `DeleteSession` in that order (except the refusal cases, which never
// reach the second), matching `do_replace_session`'s own call order.

/// Build the single-host harness a replace test needs, WITHOUT waiting for
/// its first refresh the way `spliced_helm`/`spliced_helm_listing` do —
/// callers get `(harness, local host id)` back immediately so a peer can be
/// spawned with `harness.fleet` already in hand, and wait for the refresh
/// themselves once that peer is running.
///
/// Every happy-path replace test needs that `fleet` handle for a reason
/// specific to this route: [`crate::sessions::record_session`]'s underlying
/// `remember_session` fires an IMMEDIATE background refresh for every
/// `Unknown`-status create (see that function's own doc — "ask for it NOW
/// rather than at the end of the refresh interval"), and that refresh's
/// `ListSessions` is answered from THIS harness's scripted `HostScript`,
/// not from whatever the peer just told the client it created. A real
/// supervisor's own list already reflects a session the instant after it
/// created it, so the race is harmless in production — but a scripted peer
/// whose list never moves can lose exactly the row `record_session` just
/// wrote to that same refresh's wholesale cache replace, before the
/// route's own delete half ever runs. Every test below keeps the script in
/// step with `harness.fleet.edit` right after acknowledging a create, which
/// is what this helper exists to make possible: `spliced_helm`'s own
/// construction hands out no fleet reference before the connection is
/// already up and refreshed.
async fn spliced_replace_harness(
    peer: tokio::io::DuplexStream,
    sessions: Vec<farhelm_proto::SessionInfo>,
) -> (rest_harness::Harness, store::HostId) {
    let harness = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("local-identity".to_string()),
            sessions,
            peer: Some(peer),
            ..rest_harness::HostScript::default()
        })
        .await
        .start()
        .await;
    let local = rest_harness::local_id(&harness.store).await;
    (harness, local)
}

/// The simplest replace: a live, raw-invocation source. SPEC.md's contract
/// is a NEW id carrying the source's cwd, title, and invocation, with the
/// old id gone from the list at once — this pins that promise against the
/// real handler and a scripted supervisor.
#[tokio::test]
async fn replace_of_a_live_raw_session_creates_a_new_id_and_removes_the_old() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame, SessionInfo};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let (harness, local) = spliced_replace_harness(
        client_side,
        vec![rest_harness::session("sess-1", 1_700_000_000)],
    )
    .await;
    let fleet = harness.fleet.clone();
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();

        // The create half: cwd, title, and invocation copied verbatim from
        // the source; no profile, no overrides, no idempotency key (the
        // request body below sends none).
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession {
            req_id,
            parent: None,
            profile_name: None,
            cwd,
            invocation,
            source_profile,
            title,
            intent_key,
            agent_kind,
            resume_template,
            ..
        } = request
        else {
            panic!("expected CreateSession, got {request:?}");
        };
        assert_eq!(cwd, "/sess-1");
        assert_eq!(invocation, Some("agent".to_string()));
        assert_eq!(source_profile, None);
        assert_eq!(title, Some("sess-1".to_string()));
        assert_eq!(intent_key, None);
        assert_eq!(agent_kind, None);
        assert_eq!(resume_template, None);
        let created = SessionInfo {
            parent: None,
            archived: false,
            id: "sess-2".into(),
            title: "sess-1".into(),
            created_at: 1_700_000_500,
            last_activity_at: 1_700_000_500,
            creation_seq: None,
            cwd: "/sess-1".into(),
            invocation: "agent".into(),
            status: farhelm_proto::SessionStatus::Unknown,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            source_profile: None,
        };
        // See `spliced_replace_harness`'s doc: the fixture is updated BEFORE
        // the reply that tells the client about it, matching what a REAL
        // supervisor's own list would already show if asked — a background
        // refresh this create's `Unknown` status triggers can otherwise read
        // this harness's still-stale scripted list before the peer gets
        // here and overwrite the row `record_session` just wrote.
        fleet.edit(local, |script| script.sessions.push(created.clone()));
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: created,
            }))
            .await
            .unwrap();

        // The delete half: the OLD id, sent only once the new one exists.
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession { req_id, session_id } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        // Same ordering rule as the create half, mirrored for the delete:
        // the fixture drops the source before the reply that tells the
        // client it is gone.
        fleet.edit(local, |script| script.sessions.retain(|s| s.id != "sess-1"));
        writer
            .write_control(&ControlMsg::SessionDeleted { req_id })
            .await
            .unwrap();
    });

    harness.await_refreshed(local).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let session: farhelm_proto::SessionInfo = serde_json::from_str(&body).unwrap();
    assert_eq!(session.id, "sess-2");
    assert_eq!(session.cwd, "/sess-1");
    assert_eq!(session.title, "sess-1");
    assert_eq!(session.invocation, "agent");

    // Waits for the background refresh this create's `Unknown` status
    // triggered to actually finish (`refresh_to_completion`'s own doc: the
    // second request arriving proves the first has committed), so this
    // assertion proves the STABLE fixture state rather than winning — or
    // losing — a scheduling race against that refresh.
    harness.refresh_to_completion(local).await;
    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        row_ids(&value),
        vec!["sess-2"],
        "the old id must be gone and the new one routable at once, with no \
         wait for a refresh"
    );

    peer.await.unwrap();
}

/// A CREATE reply that replays the SOURCE's own id is refused before any
/// delete is sent — the reachable idempotency-key-reuse case
/// `do_replace_session`'s own `accept_result` veto exists to catch,
/// mirroring `clone_for_agent`'s identical guard.
///
/// The collision is real, not merely hostile-peer input: a same-host
/// replace with no field overrides reconstructs the EXACT fingerprint the
/// source's own creation used (same cwd, title, raw invocation or profile,
/// default dimensions, no parent), so a caller that reuses the source's own
/// creation key hits a legitimate reservation REPLAY at the target, which
/// answers with the source row rather than a new one. Accepting that reply
/// would send `DeleteSession` for the id just "created", forget it, and
/// report success describing the session it had just destroyed — violating
/// every promise replace makes. This pins the refusal instead.
#[tokio::test]
async fn a_create_reply_that_replays_the_source_id_is_refused_before_any_delete() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame, SessionInfo};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();

        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession { req_id, .. } = request else {
            panic!("expected CreateSession, got {request:?}");
        };
        // A REPLAY: the target answers with the SOURCE's own row rather
        // than a new one, exactly as a supervisor's fingerprint match would
        // for a create key already used to make "sess-1" itself.
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: SessionInfo {
                    parent: None,
                    archived: false,
                    id: "sess-1".into(),
                    title: "sess-1".into(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    creation_seq: None,
                    cwd: "/sess-1".into(),
                    invocation: "agent".into(),
                    status: farhelm_proto::SessionStatus::Running,
                    annotation: None,
                    restart_offer: farhelm_proto::RestartOffer::default(),
                    tabs: Vec::new(),
                    source_profile: None,
                },
            }))
            .await
            .unwrap();
        // No `DeleteSession` must follow: `do_create_session`'s veto refuses
        // BEFORE any bookkeeping (see `CreateSpec::accept_result`'s own
        // "Two phases" doc), so `do_replace_session` never reaches its own
        // delete call — this peer reads nothing more.
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({"intent_key": "reused-key"}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{body}");
    assert!(
        body.contains("no replacement was made") && body.contains("a key that has not been used"),
        "the refusal must tell the caller which key mistake to fix: {body}"
    );

    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        row_ids(&value),
        vec!["sess-1"],
        "a rejected replay must leave the source exactly as it was — no delete was ever sent"
    );

    peer.await.unwrap();
}

/// A profile-backed source's replacement follows the SAME profile — the
/// happy path `mode_from_source` shares with `clone_for_agent`, exercised
/// here through the REST route instead of the agent relay.
#[tokio::test]
async fn replace_of_a_profile_backed_session_follows_its_profile() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame, ProfileSnapshot, SessionInfo, SourceProfile};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let source = farhelm_proto::SessionInfo {
        source_profile: Some(farhelm_proto::SourceProfile {
            id: "starter-claude".to_string(),
            name: "claude".to_string(),
            existence: farhelm_proto::ProfileExistence::Unresolved,
        }),
        ..rest_harness::session("sess-1", 1_700_000_000)
    };
    let (harness, local) = spliced_replace_harness(client_side, vec![source]).await;
    let fleet = harness.fleet.clone();
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();

        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession {
            req_id,
            invocation,
            source_profile,
            agent_kind,
            resume_template,
            ..
        } = request
        else {
            panic!("expected CreateSession, got {request:?}");
        };
        // `starter-claude` is one of the two seeded starter profiles every
        // fresh `helm.db` carries (`store.rs`'s `STARTER_PROFILES`), so the
        // fixture below can name it with no profile-creation setup of its
        // own.
        assert_eq!(invocation, Some("claude".to_string()));
        assert_eq!(
            source_profile,
            Some(ProfileSnapshot {
                id: "starter-claude".to_string(),
                name: "claude".to_string(),
            })
        );
        assert_eq!(agent_kind, Some(farhelm_proto::AgentKind::Claude));
        assert_eq!(resume_template, None);
        let created = SessionInfo {
            parent: None,
            archived: false,
            id: "sess-2".into(),
            title: "sess-1".into(),
            created_at: 1_700_000_500,
            last_activity_at: 1_700_000_500,
            creation_seq: None,
            cwd: "/sess-1".into(),
            invocation: "claude".into(),
            status: farhelm_proto::SessionStatus::Unknown,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            // The exact `existence` the supervisor sends here does
            // not matter: `do_create_session` recomputes it against
            // the live catalog before this reaches the caller (see
            // `resolve_session_profiles`), so `Unresolved` is the
            // ordinary placeholder every other fixture in this file
            // uses.
            source_profile: Some(SourceProfile {
                id: "starter-claude".to_string(),
                name: "claude".to_string(),
                existence: farhelm_proto::ProfileExistence::Unresolved,
            }),
        };
        // See `spliced_replace_harness`'s doc: fixture updated before reply.
        fleet.edit(local, |script| script.sessions.push(created.clone()));
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: created,
            }))
            .await
            .unwrap();

        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession { req_id, session_id } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        fleet.edit(local, |script| script.sessions.retain(|s| s.id != "sess-1"));
        writer
            .write_control(&ControlMsg::SessionDeleted { req_id })
            .await
            .unwrap();
    });

    harness.await_refreshed(local).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let session: farhelm_proto::SessionInfo = serde_json::from_str(&body).unwrap();
    assert_eq!(session.id, "sess-2");
    assert_eq!(
        session.source_profile.as_ref().map(|p| p.id.as_str()),
        Some("starter-claude")
    );

    peer.await.unwrap();
}

/// A source whose snapshotted profile has been DELETED still replaces,
/// falling back to its raw invocation — the deliberate DIVERGENCE from
/// `clone_for_agent`'s own
/// `profile_clone_with_a_dangling_snapshot_never_contacts_the_target`: the
/// agent-CLI clone refuses this exact shape because a raw invocation
/// written for one machine may not run on another, while replace never
/// changes machine, so the refusal clone needs has nothing to guard against
/// here. Pinned so this divergence is not "fixed" into a refusal later.
#[tokio::test]
async fn replace_of_a_session_whose_profile_was_deleted_falls_back_to_its_invocation() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame, SessionInfo};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let source = farhelm_proto::SessionInfo {
        invocation: "sh -c 'echo hi'".to_string(),
        source_profile: Some(farhelm_proto::SourceProfile {
            id: "deleted-profile".to_string(),
            name: "Former agent".to_string(),
            existence: farhelm_proto::ProfileExistence::Unresolved,
        }),
        ..rest_harness::session("sess-1", 1_700_000_000)
    };
    let (harness, local) = spliced_replace_harness(client_side, vec![source]).await;
    let fleet = harness.fleet.clone();
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();

        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession {
            req_id,
            invocation,
            source_profile,
            ..
        } = request
        else {
            panic!("expected CreateSession, got {request:?}");
        };
        assert_eq!(invocation, Some("sh -c 'echo hi'".to_string()));
        assert_eq!(
            source_profile, None,
            "a raw fallback carries no profile provenance"
        );
        let created = SessionInfo {
            parent: None,
            archived: false,
            id: "sess-2".into(),
            title: "sess-1".into(),
            created_at: 1_700_000_500,
            last_activity_at: 1_700_000_500,
            creation_seq: None,
            cwd: "/sess-1".into(),
            invocation: "sh -c 'echo hi'".into(),
            status: farhelm_proto::SessionStatus::Unknown,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            source_profile: None,
        };
        // See `spliced_replace_harness`'s doc: fixture updated before reply.
        fleet.edit(local, |script| script.sessions.push(created.clone()));
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: created,
            }))
            .await
            .unwrap();

        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession { req_id, session_id } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        fleet.edit(local, |script| script.sessions.retain(|s| s.id != "sess-1"));
        writer
            .write_control(&ControlMsg::SessionDeleted { req_id })
            .await
            .unwrap();
    });

    harness.await_refreshed(local).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let session: farhelm_proto::SessionInfo = serde_json::from_str(&body).unwrap();
    assert_eq!(session.invocation, "sh -c 'echo hi'");

    peer.await.unwrap();
}

/// An archived source is a legitimate replace target (SPEC.md's Replace
/// bullet: there is no agent to kill on one, only a record to delete). The
/// replacement is an ordinary, RUNNING create — archiving is never a
/// create-time flag.
#[tokio::test]
async fn replace_of_an_archived_session_creates_a_fresh_replacement() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame, SessionInfo};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let source = farhelm_proto::SessionInfo {
        archived: true,
        ..rest_harness::session("sess-1", 1_700_000_000)
    };
    let (harness, local) = spliced_replace_harness(client_side, vec![source]).await;
    let fleet = harness.fleet.clone();
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();

        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession { req_id, cwd, .. } = request else {
            panic!("expected CreateSession, got {request:?}");
        };
        assert_eq!(cwd, "/sess-1");
        let created = SessionInfo {
            parent: None,
            archived: false,
            id: "sess-2".into(),
            title: "sess-1".into(),
            created_at: 1_700_000_500,
            last_activity_at: 1_700_000_500,
            creation_seq: None,
            cwd: "/sess-1".into(),
            invocation: "agent".into(),
            status: farhelm_proto::SessionStatus::Unknown,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            source_profile: None,
        };
        // See `spliced_replace_harness`'s doc: fixture updated before reply.
        fleet.edit(local, |script| script.sessions.push(created.clone()));
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: created,
            }))
            .await
            .unwrap();

        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession { req_id, session_id } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        fleet.edit(local, |script| script.sessions.retain(|s| s.id != "sess-1"));
        writer
            .write_control(&ControlMsg::SessionDeleted { req_id })
            .await
            .unwrap();
    });

    harness.await_refreshed(local).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let session: farhelm_proto::SessionInfo = serde_json::from_str(&body).unwrap();
    assert_eq!(session.id, "sess-2");
    assert!(!session.archived, "a replacement is never born archived");

    peer.await.unwrap();
}

/// If the create fails, the source is untouched: the handler returns before
/// ever sending a delete, and the row stays listed exactly as it was — the
/// "nothing was lost" half of `do_replace_session`'s asymmetric failure
/// rule (see its own doc).
#[tokio::test]
async fn a_create_refusal_leaves_the_source_listed() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, ErrorKind, Frame};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession { req_id, .. } = request else {
            panic!("expected CreateSession, got {request:?}");
        };
        writer
            .write_frame(&Frame::control(&ControlMsg::Error {
                req_id,
                message: "working directory does not exist: /sess-1".into(),
                kind: ErrorKind::InvalidRequest,
            }))
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("does not exist"), "{body}");

    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(row_ids(&value), vec!["sess-1"]);

    peer.await.unwrap();
}

/// An EXPLICIT supervisor refusal of the delete, after a successful create,
/// must not hide either session: the reply names both ids and says both
/// rows stay listed, definitely — `do_replace_session`'s asymmetric
/// failure rule (see its own doc), for the branch where the refusal itself
/// proves the delete did not happen. Rolling the create back would kill an
/// agent the caller just asked for, and plain failure would hide a session
/// the caller was never told still exists.
#[tokio::test]
async fn a_delete_failure_after_a_successful_create_reports_both_ids_and_leaves_both_rows() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, ErrorKind, Frame, SessionInfo};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let (harness, local) = spliced_replace_harness(
        client_side,
        vec![rest_harness::session("sess-1", 1_700_000_000)],
    )
    .await;
    let fleet = harness.fleet.clone();
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();

        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession { req_id, .. } = request else {
            panic!("expected CreateSession, got {request:?}");
        };
        let created = SessionInfo {
            parent: None,
            archived: false,
            id: "sess-2".into(),
            title: "sess-1".into(),
            created_at: 1_700_000_500,
            last_activity_at: 1_700_000_500,
            creation_seq: None,
            cwd: "/sess-1".into(),
            invocation: "agent".into(),
            status: farhelm_proto::SessionStatus::Unknown,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            source_profile: None,
        };
        // See `spliced_replace_harness`'s doc: this test's whole point is
        // what the LIST shows after the failure below, so the fixture must
        // not itself be the reason "sess-2" is missing from it — updated
        // before the reply, like every other successful create in this file.
        fleet.edit(local, |script| script.sessions.push(created.clone()));
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: created,
            }))
            .await
            .unwrap();

        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession { req_id, session_id } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        // No fixture change here: the supervisor REFUSED the delete, so
        // "sess-1" genuinely still exists on the (simulated) far end too —
        // unlike the happy-path tests, this peer must NOT remove it.
        writer
            .write_frame(&Frame::control(&ControlMsg::Error {
                req_id,
                message: "supervisor lost the process table entry".into(),
                kind: ErrorKind::Internal,
            }))
            .await
            .unwrap();
    });

    harness.await_refreshed(local).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(
        status,
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "{body}"
    );
    assert!(body.contains("sess-1"), "must name the source: {body}");
    assert!(body.contains("sess-2"), "must name the replacement: {body}");
    assert!(
        body.contains("both sessions still exist"),
        "an explicit supervisor refusal is a DEFINITE answer, not an unknown outcome: {body}"
    );

    // See `replace_of_a_live_raw_session_creates_a_new_id_and_removes_the_old`'s
    // own comment on why this waits for the create's background refresh to
    // finish before reading the list.
    harness.refresh_to_completion(local).await;
    let (_, value) = get_json(&harness, "/api/sessions").await;
    let mut ids = row_ids(&value);
    ids.sort();
    assert_eq!(
        ids,
        vec!["sess-1".to_string(), "sess-2".to_string()],
        "both sessions must stay listed until the user removes the old one by hand"
    );

    peer.await.unwrap();
}

/// A delete whose frame reaches the peer, but whose CONNECTION ends before
/// any reply comes back, is NOT the same failure as an explicit refusal —
/// `do_replace_session`'s own doc draws the line between them. The
/// supervisor may have completed the deletion and lost only the
/// confirmation, so the reply must not claim the source definitely still
/// exists (unlike the explicit-refusal test above); it must say the
/// replacement is real and that the source's fate needs checking.
///
/// Dropping the PEER's own `reader`/`writer` does NOT model this: this
/// harness's spliced relay deliberately keeps the manager-side connection
/// open after a scripted peer's own exchange ends (`rest_harness`'s own
/// module doc — "a spliced peer's EXIT is deliberately not propagated"),
/// so a peer that simply stops talking leaves `client.delete_session`
/// waiting forever rather than failing it. The connection has to be killed
/// from the OUTSIDE instead — `ScriptedFleet::kill_connection`, the same
/// primitive the reconnect-path tests use — timed with a handshake so it
/// fires only once the delete frame has genuinely reached the peer.
#[tokio::test]
async fn a_delete_lost_after_the_supervisor_applied_it_reports_an_unknown_outcome() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame, SessionInfo};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let (harness, local) = spliced_replace_harness(
        client_side,
        vec![rest_harness::session("sess-1", 1_700_000_000)],
    )
    .await;
    let fleet = harness.fleet.clone();
    // Signals that the delete frame has been read, so the connection is
    // only killed once the request has GENUINELY reached the peer — killing
    // it any earlier would test `NotSent`, not `SentUnanswered`.
    let (delete_seen_tx, delete_seen_rx) = tokio::sync::oneshot::channel();
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();

        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession { req_id, .. } = request else {
            panic!("expected CreateSession, got {request:?}");
        };
        let created = SessionInfo {
            parent: None,
            archived: false,
            id: "sess-2".into(),
            title: "sess-1".into(),
            created_at: 1_700_000_500,
            last_activity_at: 1_700_000_500,
            creation_seq: None,
            cwd: "/sess-1".into(),
            invocation: "agent".into(),
            status: farhelm_proto::SessionStatus::Unknown,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            source_profile: None,
        };
        fleet.edit(local, |script| script.sessions.push(created.clone()));
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: created,
            }))
            .await
            .unwrap();

        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession { session_id, .. } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        // The supervisor APPLIES the delete — the fixture drops "sess-1",
        // modeling that reality — but never gets to answer: the test kills
        // the connection out from under this exchange the moment it learns
        // the frame arrived (see `delete_seen_tx` below), which is what
        // actually produces `SupervisorTransportError::SentUnanswered` on
        // the client side.
        fleet.edit(local, |script| script.sessions.retain(|s| s.id != "sess-1"));
        let _ = delete_seen_tx.send(());
    });

    harness.await_refreshed(local).await;
    let kill_fleet = harness.fleet.clone();
    let (status, body) = {
        let post = post_text(
            &harness,
            "/api/sessions/sess-1/replace",
            serde_json::json!({}),
        );
        let kill = async move {
            delete_seen_rx
                .await
                .expect("peer dropped before reading the delete");
            kill_fleet.kill_connection(local);
        };
        let (response, ()) = tokio::join!(post, kill);
        response
    };
    assert_eq!(
        status,
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "{body}"
    );
    assert!(body.contains("sess-1"), "must name the source: {body}");
    assert!(body.contains("sess-2"), "must name the replacement: {body}");
    assert!(
        !body.contains("both sessions still exist"),
        "a lost reply is not proof the source survived: {body}"
    );
    assert!(
        body.contains("unknown") && body.contains("check"),
        "the honest reply says the outcome is unknown and must be checked before cleanup: {body}"
    );

    peer.await.unwrap();
}

/// A replace on a session whose host is unreachable is refused, with the
/// state named, before anything is created — the same owner-lookup refusal
/// every lifecycle operation gets (`route_session`), exercised here through
/// `/replace`.
#[tokio::test]
async fn replace_on_an_unreachable_host_is_refused_before_anything_is_created() {
    let (builder, host) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@breaks",
            rest_harness::HostScript {
                identity: Some("identity-original".to_string()),
                sessions: vec![rest_harness::session("sess-1", 100)],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    harness.await_refreshed(host).await;

    harness.fleet.take_down(host);
    harness
        .await_state(host, |state| state.phase() == "unreachable-reprobing")
        .await;

    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{body}");
    assert!(body.contains("unreachable-reprobing"), "{body}");

    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        row_ids(&value),
        vec!["sess-1"],
        "nothing may be created while the source's host is unreachable"
    );
}

/// `intent_key` reaches the create half exactly as an ordinary create's
/// does: retried under the same key, the create REPLAYS rather than
/// minting a second session. The only place a retry can meaningfully land
/// is after a delete failure — a fully successful replace 404s the source
/// on any later retry (see `do_replace_session`'s own doc) — so this test
/// chains the retry off exactly that state, and the peer script plays the
/// supervisor's own idempotent reply by hand (this helm keeps no local
/// dedup of its own; the guarantee is entirely the supervisor's).
#[tokio::test]
async fn a_replace_retried_with_the_same_intent_key_after_a_delete_failure_creates_only_once() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, ErrorKind, Frame, SessionInfo};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let (harness, local) = spliced_replace_harness(
        client_side,
        vec![rest_harness::session("sess-1", 1_700_000_000)],
    )
    .await;
    let fleet = harness.fleet.clone();
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();

        let created = SessionInfo {
            parent: None,
            archived: false,
            id: "sess-2".into(),
            title: "sess-1".into(),
            created_at: 1_700_000_500,
            last_activity_at: 1_700_000_500,
            creation_seq: None,
            cwd: "/sess-1".into(),
            invocation: "agent".into(),
            status: farhelm_proto::SessionStatus::Unknown,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            source_profile: None,
        };
        let created_reply = |req_id| {
            Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: created.clone(),
            })
        };
        // See `spliced_replace_harness`'s doc. `retain`-then-`push` rather
        // than a plain `push`, because THIS test calls it twice for the
        // SAME session id (the replay below is scripted as a second,
        // identical create) — a bare push would leave the scripted list
        // naming "sess-2" twice, which `drain_sessions`'s own duplicate-id
        // guard would then refuse to read at all.
        let sync_created = || {
            fleet.edit(local, |script| {
                script.sessions.retain(|s| s.id != created.id);
                script.sessions.push(created.clone());
            });
        };

        // First attempt: create succeeds, delete fails.
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession {
            req_id, intent_key, ..
        } = request
        else {
            panic!("expected CreateSession, got {request:?}");
        };
        assert_eq!(intent_key.as_deref(), Some("replace-key"));
        sync_created();
        writer.write_frame(&created_reply(req_id)).await.unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession { req_id, session_id } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        // No fixture change: the supervisor REFUSES this delete, so
        // "sess-1" genuinely still exists on the far end too.
        writer
            .write_frame(&Frame::control(&ControlMsg::Error {
                req_id,
                message: "transient failure".into(),
                kind: ErrorKind::Internal,
            }))
            .await
            .unwrap();

        // Retry, same key: the reply below is a hand-played REPLAY of the
        // first create, exactly what the supervisor's own fingerprint match
        // would answer with — same session, no second launch.
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession {
            req_id, intent_key, ..
        } = request
        else {
            panic!("expected CreateSession, got {request:?}");
        };
        assert_eq!(intent_key.as_deref(), Some("replace-key"));
        sync_created();
        writer.write_frame(&created_reply(req_id)).await.unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession { req_id, session_id } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        // This delete SUCCEEDS, so the fixture drops the source before the
        // reply that tells the client it is gone — same rule as every other
        // successful delete in this file.
        fleet.edit(local, |script| script.sessions.retain(|s| s.id != "sess-1"));
        writer
            .write_control(&ControlMsg::SessionDeleted { req_id })
            .await
            .unwrap();
    });

    harness.await_refreshed(local).await;
    let (status, _) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({"intent_key": "replace-key"}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);

    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({"intent_key": "replace-key"}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let session: farhelm_proto::SessionInfo = serde_json::from_str(&body).unwrap();
    assert_eq!(
        session.id, "sess-2",
        "the retry must replay the same session"
    );

    // See the live-raw-session test's own comment on why this waits for the
    // create's background refresh to finish before reading the list — this
    // test triggers TWO such refreshes (one per attempt), and both must have
    // settled before the assertion below means anything.
    harness.refresh_to_completion(local).await;
    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        row_ids(&value),
        vec!["sess-2"],
        "exactly one new session must exist once the retry succeeds"
    );

    peer.await.unwrap();
}

/// `GET /api/sessions`'s JSON shape, which the UI decodes and which
/// PLAN_M6.md item 5 extended without breaking.
///
/// Two halves, both load-bearing for the UI. The M2 envelope
/// (`sessions`/`total`/`truncated`) is still there under the same names,
/// so the list UI keeps decoding it unchanged — and there is no
/// `next_cursor`: the list is served whole, by contract;
/// and each row now carries `host`/`host_identity`/`host_name`/`stale`
/// as ADDITIVE siblings of the session's own fields, never nested under
/// a wrapper — which is the whole reason `SessionRow` flattens
/// `SessionInfo` instead of embedding it.
///
/// Asserted on raw JSON rather than a decoded type, because the UI
/// decodes JSON: a serialization change that a round trip through the
/// same Rust types would hide is exactly what would break the list in
/// the browser.
#[tokio::test]
async fn list_sessions_returns_the_merged_listing_object_shape() {
    let harness =
        rest_harness::helm_listing(vec![rest_harness::session("sess-1", 1_700_000_000)]).await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/api/sessions")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = tower::ServiceExt::oneshot(app, request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["total"], 1, "total counts the merged view");
    assert_eq!(
        value["truncated"], false,
        "the whole view fit under the cap"
    );
    assert!(
        value.get("next_cursor").is_none(),
        "the reply carries no resume key: the list is one object, never a page"
    );

    let row = &value["sessions"][0];
    assert_eq!(row["id"], "sess-1");
    assert_eq!(
        row["title"], "sess-1",
        "the session's own fields stay at the row's top level"
    );
    assert_eq!(
        row["host"],
        rest_harness::local_id(&harness.store).await,
        "every row names the host it lives on"
    );
    assert_eq!(
        row["host_name"], "this machine",
        "the reserved local row is described, never addressed"
    );
    assert_eq!(
        row["stale"], false,
        "a connected host's rows are live knowledge"
    );
    assert_eq!(
        row["host_identity"], "local-identity",
        "the row carries the registry's RECORDED identity for its host — \
         the helm.db join, not a snapshot-side copy that would sit null \
         until the next reconcile (the serialized-even-when-NULL half of \
         the wire contract is pinned where a host is actually \
         identity-less, in \
         an_identity_less_hosts_sessions_serve_while_connected_and_vanish_after)"
    );
}

/// Each row's `host_identity` is ITS host's registry identity, not the
/// fleet's first or only one.
///
/// Why this exists: the single-host shape test above cannot tell a
/// correct per-host join from a join by the wrong key, a copy of the
/// first identity onto every row, or a connection-scoped value that
/// happens to coincide — any of those regressions stays green with one
/// host whose identities all agree. Two hosts with distinct recorded
/// identities make the join's key observable.
#[tokio::test]
async fn each_listing_row_carries_its_own_hosts_identity() {
    let (builder, remote) = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("install-local".to_string()),
            sessions: vec![rest_harness::session("on-local", 200)],
            ..rest_harness::HostScript::default()
        })
        .await
        .ssh(
            "user@remote",
            rest_harness::HostScript {
                identity: Some("install-remote".to_string()),
                sessions: vec![rest_harness::session("on-remote", 100)],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    let local = rest_harness::local_id(&harness.store).await;
    harness.await_refreshed(local).await;
    harness.await_refreshed(remote).await;

    let (status, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let rows = value["sessions"].as_array().expect("rows array");
    assert_eq!(rows.len(), 2);
    for row in rows {
        let expected = if row["host"] == serde_json::json!(local) {
            "install-local"
        } else {
            "install-remote"
        };
        assert_eq!(
            row["host_identity"], expected,
            "row {} must carry the identity recorded for ITS host",
            row["id"]
        );
    }
}

/// `GET /api/sessions/{id}` — the session-detail route a session view
/// actually fetches — must pass a NON-EMPTY `tabs` list through
/// intact. `farhelm-proto`'s own tests already pin `SessionInfo`'s
/// JSON shape exhaustively (order, nesting, everything); what the helm
/// still owes is exactly one HTTP-boundary check that THIS route does
/// not drop or mangle the field on its way from the supervisor's
/// `ListSessions` reply to the JSON body a browser decodes — this
/// replaces an earlier version of the same check aimed at the bulk
/// LISTING route, which no session view reads tabs from.
#[tokio::test]
async fn get_session_passes_a_non_empty_tabs_list_through() {
    let harness = rest_harness::helm_listing(vec![farhelm_proto::SessionInfo {
        tabs: vec![
            farhelm_proto::TabInfo { id: "tab-1".into() },
            farhelm_proto::TabInfo { id: "tab-2".into() },
        ],
        ..rest_harness::session("sess-1", 1_700_000_000)
    }])
    .await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/api/sessions/sess-1")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = tower::ServiceExt::oneshot(app, request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        value["tabs"],
        serde_json::json!([{"id": "tab-1"}, {"id": "tab-2"}])
    );
    assert_eq!(
        value["stale"], false,
        "a connected host's detail is live, and says so"
    );
    assert_eq!(
        value["host"],
        rest_harness::local_id(&harness.store).await,
        "the detail route carries the same host fields a list row does"
    );
    assert_eq!(
        value["host_identity"], "local-identity",
        "the live detail branch joins the registry identity like a list \
         row — a detail refresh that dropped it would silently downgrade \
         the create default to the row-id-only check"
    );
}

/// `GET /api/sessions/{id}` derives `host_name` from
/// `aggregate::host_display_name` on its OWN call
/// (`sessions.rs:1481`), independently of the merged-listing route's
/// derivation — the two share a function, not a code path, so a future
/// regression at this call specifically would leave list rows renamed
/// while a session's own detail view kept showing the destination.
/// `hosts.rs`'s `session_rows_carry_the_alias_in_host_name` pins the
/// listing side; this pins the detail side.
#[tokio::test]
async fn get_session_carries_the_hosts_alias() {
    let harness =
        rest_harness::helm_listing(vec![rest_harness::session("aliased-detail", 1_700_000_000)])
            .await;
    let local = rest_harness::local_id(&harness.store).await;
    harness
        .store
        .update_alias(local, Some("Aliased Detail Host"))
        .await
        .expect("alias the local host");
    // A direct store write does not itself reach the manager's live
    // snapshot — see `hosts.rs`'s `set_alias` for why a real client always
    // resyncs after writing through the route.
    harness
        .manager
        .sync_registry()
        .await
        .expect("resync after aliasing directly through the store");
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/api/sessions/aliased-detail")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = tower::ServiceExt::oneshot(app, request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["host_name"], "Aliased Detail Host");
}

/// The helm is a passthrough for classification, and PLAN_M3.md item 2
/// is the first change that makes that claim testable with something
/// the helm could plausibly get wrong: `interrupted` is a status
/// variant no earlier milestone had, and the stop annotation is a
/// field nothing used to populate. Neither is invented, renamed, or
/// dropped here — the supervisor is authoritative (SPEC.md), and this
/// pins that the JSON the browser receives says exactly what the
/// supervisor said.
///
/// Asserted on the raw JSON rather than a decoded `SessionInfo`,
/// because the UI decodes JSON, not proto types: a serialization
/// change that a round trip through the same Rust types would hide is
/// precisely what would break the badge in the browser.
///
/// As of PLAN_M6.md item 5 the claim is stronger than it was: these
/// rows now reach the browser by way of helm.db's session cache, so
/// they survive a serialize/store/deserialize round trip on the way.
/// A status variant or annotation field that failed to persist would
/// fail here too, which is exactly the coverage a durable cache of
/// supervisor-authored data needs.
#[tokio::test]
async fn list_sessions_passes_interrupted_status_and_stop_annotation_through() {
    let session = |id: &str, status, annotation: Option<&str>| farhelm_proto::SessionInfo {
        status,
        annotation: annotation.map(str::to_string),
        // `created_at` is shared, so the merged order falls to the id
        // tiebreak — which is what fixes "lost" ahead of "stopped"
        // below rather than leaving the two positions to chance.
        ..rest_harness::session(id, 1_700_000_000)
    };
    let harness = rest_harness::helm_listing(vec![
        session("lost", farhelm_proto::SessionStatus::Interrupted, None),
        session(
            "stopped",
            farhelm_proto::SessionStatus::Exited { exit_code: Some(0) },
            Some(farhelm_proto::STOP_ANNOTATION),
        ),
    ])
    .await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/api/sessions")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = tower::ServiceExt::oneshot(app, request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["sessions"][0]["status"]["state"], "interrupted");
    assert_eq!(value["sessions"][0]["annotation"], serde_json::Value::Null);
    assert_eq!(value["sessions"][1]["status"]["state"], "exited");
    assert_eq!(value["sessions"][1]["annotation"], "stopped by user");
}

/// The DNS-rebinding origin guard is route-agnostic middleware, and the
/// Playwright suite (`terminal.spec.ts`, "requests from a foreign
/// origin are refused") already proves it holds through the real
/// stack. What that suite does NOT cover is this PR's own change: that
/// the guard sits in front of the new mutating routes too, not just
/// `GET /api/sessions`, and that a refused request never reaches the
/// supervisor at all. A loopback `Host` (same-origin by that half of
/// the check) paired with a foreign `Origin` isolates exactly the
/// half the browser itself supplies from the requesting page's origin
/// — same setup as `foreign_or_missing_authorities_are_refused`
/// above, aimed at the stop route instead of the pure function.
///
/// Proof that no frame reached the supervisor comes from EOF, not a
/// timeout: `oneshot` consumes the router (and with it the only
/// remaining `Arc<SupervisorClient>`) once the response is produced, so
/// the transport closes right after — the scripted peer reading a clean
/// `Ok(None)` at that point means nothing but the handshake was ever
/// written to it. A frame arriving instead (a bypassed guard) would read
/// as `Ok(Some(_))`, which is what this actually distinguishes.
#[tokio::test]
async fn foreign_origin_is_refused_on_the_stop_route() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        // A bounded silence, not an EOF: the harness keeps this
        // connection open for the whole test (see `rest_harness`), so
        // "nothing reached the supervisor" is only observable as
        // nothing ARRIVING. The window is generous because a false
        // pass needs the frame to be merely late, and a stop that the
        // middleware failed to refuse would be sent immediately.
        let leaked = tokio::time::timeout(Duration::from_secs(2), reader.read_frame()).await;
        assert!(
            leaked.is_err(),
            "stop request must never reach the supervisor for a foreign origin, but one \
             arrived: {leaked:?}"
        );
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions/sess-1/stop")
        .header("host", "127.0.0.1:7433")
        .header("origin", "http://evil.example")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);

    peer.await.unwrap();
}

/// `http_error`'s status mapping, pinned through the real handler and
/// middleware stack rather than by calling `http_error` directly: what
/// actually matters is that a `ControlMsg::Error`'s `kind` survives
/// `SupervisorClient::request`'s downcast and reaches the HTTP status,
/// not just that the mapping function has the right match arms.
///
/// `InvalidRequest` is exercised here (400) rather than `NotFound`
/// (404): both go through the identical downcast path in `http_error`,
/// and the supervisor-side classification for a bad cwd — the
/// realistic `InvalidRequest` case — is itself pinned end-to-end
/// against a real supervisor in `farhelm/tests/e2e.rs`
/// (`create_in_missing_directory_errors`). This test's job is narrower
/// and complementary: prove the *client-and-HTTP* half of the chain
/// (scripted `Error` reply in, status code out) without needing a real
/// supervisor, tmux, or filesystem precondition to produce one.
#[tokio::test]
async fn create_session_error_reply_maps_to_bad_request_status() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, ErrorKind, Frame};
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession { req_id, .. } = request else {
            panic!("expected CreateSession, got {request:?}");
        };
        writer
            .write_frame(&Frame::control(&ControlMsg::Error {
                req_id,
                message: "working directory does not exist: /nope".into(),
                kind: ErrorKind::InvalidRequest,
            }))
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({"cwd": "/nope", "invocation": "some-agent"}).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(
        String::from_utf8_lossy(&body).contains("does not exist"),
        "body must still carry the supervisor's concrete message"
    );

    peer.await.unwrap();
}

/// `POST /api/sessions/{id}/restart` end to end (PLAN_M3.md item 9):
/// the body's `mode` and `stop_if_running` reach the supervisor
/// unaltered, and the success body is the session's own recomputed
/// `SessionInfo` — including the freshly computed `restart_offer` a
/// caller re-renders its row from without listing again.
///
/// Both body fields are asserted at the WIRE, not merely accepted by
/// the handler: `stop_if_running` is the user's consent to kill a
/// running agent and `mode` is the choice the supervisor validates
/// against the current offer, so a route that dropped or defaulted
/// either would be a silent safety regression rather than a visible
/// failure.
#[tokio::test]
async fn restart_session_passes_mode_and_consent_through_and_returns_the_session() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::RestartSession {
            req_id,
            session_id,
            mode,
            stop_if_running,
        } = request
        else {
            panic!("expected RestartSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        assert_eq!(mode, farhelm_proto::RestartMode::Resume);
        assert!(
            stop_if_running,
            "the user's consent to stop a live agent must reach the supervisor"
        );
        writer
            .write_control(&ControlMsg::SessionRestarted {
                req_id,
                session: farhelm_proto::SessionInfo {
                    parent: None,
                    archived: false,
                    id: "sess-1".into(),
                    title: "t".into(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    creation_seq: None,
                    cwd: "/some/dir".into(),
                    invocation: "some-agent".into(),
                    status: farhelm_proto::SessionStatus::Unknown,
                    annotation: None,
                    restart_offer: farhelm_proto::RestartOffer::Resume,
                    tabs: Vec::new(),
                    source_profile: None,
                },
            })
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions/sess-1/restart")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({ "mode": "resume", "stop_if_running": true }).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["id"], "sess-1");
    assert_eq!(
        value["restart_offer"], "resume",
        "the reply carries the offer the session has NOW, which is what a client re-renders"
    );

    peer.await.unwrap();
}

/// A stale-offer refusal must reach the browser as a 409 carrying the
/// supervisor's own prose — that message names the CURRENT offer, and
/// re-presenting it is the client's prescribed response (the wire
/// vocabulary's staleness contract). A route that flattened it to a
/// generic 500 would leave the UI with nothing to say.
#[tokio::test]
async fn restart_session_conflict_reaches_the_caller_as_409_with_its_message() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, ErrorKind};
    use tower::ServiceExt;

    const SENTINEL: &str = "SENTINEL-restart-4b1e: the offer is now resume";

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::RestartSession { req_id, .. } = request else {
            panic!("expected RestartSession, got {request:?}");
        };
        writer
            .write_control(&ControlMsg::Error {
                req_id,
                message: SENTINEL.to_string(),
                kind: ErrorKind::Conflict,
            })
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions/sess-1/restart")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({ "mode": "fresh" }).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&body).trim(), SENTINEL);

    peer.await.unwrap();
}

/// `POST /api/sessions/{id}/rename` end to end (PLAN_M5.md item 4), for
/// every shape a title can take: an ordinary title, the empty string
/// (an explicit empty title is a legal rename, symmetric with an
/// explicit empty title on create — PLAN_M5.md item 3), leading/
/// trailing whitespace, and embedded control characters (the very
/// thing the supervisor's own validation refuses, so this helm-level
/// hop must not pre-filter or normalize it away before the refusal can
/// even run). One route, four shapes, because the property under
/// test — "no trimming, no validation, no rewriting" — is the same
/// claim for each and a shared body keeps the cases from drifting
/// into subtly different assertions.
///
/// The success body is checked as a FULL `SessionInfo`, field for
/// field against the scripted reply, not just `id`/`title`: a route
/// that echoed a stale or partially-rebuilt session (the bug
/// `SessionRenamed`'s own docs warn against — see
/// `ControlMsg::SessionRenamed`) would still pass an id/title-only
/// check while failing every other field.
#[tokio::test]
async fn rename_session_forwards_the_title_verbatim() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, RestartOffer, SessionInfo, SessionStatus, TabInfo};
    use tower::ServiceExt;

    let cases = [
        "an ordinary title",
        "",
        "  leading and trailing spaces  ",
        "bell\u{7}esc\u{1b}nl\ntab\t",
    ];

    for title in cases {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let expected_title = title.to_string();
        // Distinctive on every field, not just `title`: a handler
        // that echoed back a stale or default-filled `SessionInfo`
        // must fail the full-struct comparison below even if the
        // title alone looked right.
        let expected_session = SessionInfo {
            parent: None,
            archived: false,
            id: "sess-1".into(),
            title: expected_title.clone(),
            created_at: 1_700_000_000,
            last_activity_at: 1_700_000_000,
            creation_seq: None,
            cwd: "/distinctive/dir".into(),
            invocation: "distinctive-agent --flag".into(),
            status: SessionStatus::Running,
            annotation: None,
            restart_offer: RestartOffer::Resume,
            tabs: vec![TabInfo { id: "tab-1".into() }],
            source_profile: None,
        };
        let reply_session = expected_session.clone();
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::RenameSession {
                req_id, title: got, ..
            } = request
            else {
                panic!("expected RenameSession, got {request:?}");
            };
            assert_eq!(
                got, expected_title,
                "the title must reach the supervisor byte-for-byte unchanged"
            );
            writer
                .write_control(&ControlMsg::SessionRenamed {
                    req_id,
                    session: reply_session,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/rename")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "title": title }).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::OK,
            "for title {title:?}"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let got_session: SessionInfo = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            got_session, expected_session,
            "the success body must be the supervisor's FULL SessionInfo, not a partial \
             echo, for title {title:?}"
        );

        peer.await.unwrap();
    }
}

/// A body whose `title` field is MISSING entirely must be refused
/// before this route's handler ever runs — 422 from axum 0.8's `Json`
/// extractor rejecting a body that parses as JSON but fails to
/// deserialize into `RenameReq` (a missing required field), distinct
/// from the 400 a body that is not valid JSON at all would get
/// (`RenameReq`'s own docs name the same distinction) — and
/// distinctly from a body whose `title` is PRESENT but explicitly
/// empty, which must reach the supervisor and be accepted (SPEC.md
/// names control characters, not absence of content, as rename's
/// refusal — PLAN_M5.md item 3; `rename_session_forwards_the_title_verbatim`
/// also carries the empty-string case among its shapes). Both halves
/// live in this one test, rather than as two that could quietly drift
/// apart, because "missing" and "explicit empty" are exactly the pair
/// a route that collapsed `Option<String>` handling could confuse.
#[tokio::test]
async fn rename_session_missing_title_is_422_but_an_explicit_empty_title_is_accepted() {
    use tower::ServiceExt;

    // Half 1: `title` absent. No frame may reach the supervisor at
    // all — a rejected extractor never calls `rename_session`'s body.
    {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = farhelm_proto::io::FrameReader::new(r);
            let mut writer = farhelm_proto::io::FrameWriter::new(w);
            farhelm_proto::io::handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            reader
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/rename")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(serde_json::json!({}).to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "a body missing the required `title` field must be a 422 from the JSON extractor"
        );

        // Dropping `app`/`response` above already dropped this
        // block's only `SupervisorClient` handle, which closes the
        // transport — so the peer seeing EOF proves nothing about
        // whether a frame was sent first; only the SHAPE of what (if
        // anything) arrives does. A still-open connection with
        // nothing to read, or a clean EOF with nothing read, are both
        // consistent with "no frame was ever sent"; an actual frame
        // is the one outcome that is not.
        let mut reader = peer.await.unwrap();
        match tokio::time::timeout(Duration::from_millis(200), reader.read_frame()).await {
            Err(_) | Ok(Ok(None)) => {}
            Ok(Ok(Some(frame))) => panic!(
                "a rejected extractor must never let a RenameSession reach the \
                 supervisor, but this frame arrived: {frame:?}"
            ),
            Ok(Err(e)) => {
                panic!("unexpected transport error while checking for a stray frame: {e}")
            }
        }
    }

    // Half 2: `title` present and explicitly empty. Must reach the
    // supervisor (not be treated as if it were absent) and succeed.
    {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::RenameSession { req_id, title, .. } = request else {
                panic!("expected RenameSession, got {request:?}");
            };
            assert_eq!(
                title, "",
                "an explicit empty title must reach the supervisor, not be treated as \
                 though it were absent"
            );
            writer
                .write_control(&ControlMsg::SessionRenamed {
                    req_id,
                    session: farhelm_proto::SessionInfo {
                        parent: None,
                        archived: false,
                        id: "sess-1".into(),
                        title: String::new(),
                        created_at: 1_700_000_000,
                        last_activity_at: 1_700_000_000,
                        creation_seq: None,
                        cwd: "/some/dir".into(),
                        invocation: "some-agent".into(),
                        status: farhelm_proto::SessionStatus::Unknown,
                        annotation: None,
                        restart_offer: farhelm_proto::RestartOffer::default(),
                        tabs: Vec::new(),
                        source_profile: None,
                    },
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/rename")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "title": "" }).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::OK,
            "an explicit empty title must be ACCEPTED, distinctly from the missing-field \
             case in the first half of this test"
        );

        peer.await.unwrap();
    }
}

/// Renaming an unknown session must 404 from the helm's own owner
/// lookup, without reaching a supervisor — the rename-side twin of
/// `stop_session_unknown_id_returns_404_with_supervisor_message`, whose
/// docs carry the reasoning.
#[tokio::test]
async fn rename_session_unknown_id_returns_404_with_supervisor_message() {
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(silent_supervisor(peer_side));

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions/sess-missing/rename")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({ "title": "doesn't matter" }).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&body),
        "no such session: sess-missing",
        "the helm's own refusal must name the id it could not place"
    );

    peer.await.unwrap();
}

/// A title the supervisor refuses (control characters, per PLAN_M5.md
/// item 3's validation) must surface as a 400 carrying the
/// supervisor's own refusal text — the UI's only source for that
/// message, since this route performs no local validation of its own
/// to phrase a redundant one from.
#[tokio::test]
async fn rename_session_invalid_title_returns_400_with_supervisor_message() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, ErrorKind};
    use tower::ServiceExt;

    const SENTINEL: &str = "SENTINEL-rename-e91f: title must not contain control characters";

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::RenameSession { req_id, .. } = request else {
            panic!("expected RenameSession, got {request:?}");
        };
        writer
            .write_control(&ControlMsg::Error {
                req_id,
                message: SENTINEL.to_string(),
                kind: ErrorKind::InvalidRequest,
            })
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions/sess-1/rename")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({ "title": "bad\u{7}title" }).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&body),
        SENTINEL,
        "body must carry the supervisor's own refusal text verbatim"
    );

    peer.await.unwrap();
}

/// `POST /api/sessions/{id}/tabs` happy path (PLAN_M4.md item 5): the
/// scripted `TabOpened` reply's `TabInfo` must round-trip through the
/// success body under a `tab` key — the shape a client needs before it
/// can attach the new tab via `?tab=<id>` on `term_ws`.
#[tokio::test]
async fn open_tab_happy_path_returns_200_with_tab() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, TabInfo};
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::OpenTab { req_id, session_id } = request else {
            panic!("expected OpenTab, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        writer
            .write_control(&ControlMsg::TabOpened {
                req_id,
                tab: TabInfo { id: "tab-1".into() },
            })
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions/sess-1/tabs")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["tab"]["id"], "tab-1");

    peer.await.unwrap();
}

/// `DELETE /api/sessions/{id}/tabs/{tab_id}` happy path, mirroring
/// `stop_session_happy_path_returns_200_with_empty_object_body`: a
/// scripted `TabClosed` reply must reach the caller as 200 with the
/// same empty-object body every no-payload success shares. The peer
/// asserts both path segments landed in the right `CloseTab` fields —
/// a route that swapped `id`/`tab_id` would still 200 here, just
/// against the wrong tab.
#[tokio::test]
async fn close_tab_happy_path_returns_200_with_empty_object_body() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CloseTab {
            req_id,
            session_id,
            tab_id,
        } = request
        else {
            panic!("expected CloseTab, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        assert_eq!(tab_id, "tab-1");
        writer
            .write_control(&ControlMsg::TabClosed { req_id })
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("DELETE")
        .uri("/api/sessions/sess-1/tabs/tab-1")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value, serde_json::json!({}));

    peer.await.unwrap();
}

/// `POST /api/sessions/{id}/tabs` must map a supervisor `Error` reply
/// to the right HTTP status AND carry its message through verbatim.
/// `http_error`'s own unit tests already pin the full four-`ErrorKind`
/// table exhaustively, so this route owes only ONE representative
/// case through the real handler — `NotFound`, the same choice
/// `stop_session_unknown_id_returns_404_with_supervisor_message` made
/// for the same reason. The body assertion is the COMPLETE sentinel,
/// not a substring: a handler that truncated or rewrapped the
/// supervisor's message would still pass a status-only check here,
/// which is exactly the gap an exact-body assertion closes.
#[tokio::test]
async fn open_tab_error_reply_maps_to_404_with_the_supervisors_message() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, ErrorKind};
    use tower::ServiceExt;

    const SENTINEL: &str = "SENTINEL-open-tab-3f1a2c: no such session";

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::OpenTab { req_id, .. } = request else {
            panic!("expected OpenTab, got {request:?}");
        };
        writer
            .write_control(&ControlMsg::Error {
                req_id,
                message: SENTINEL.to_string(),
                kind: ErrorKind::NotFound,
            })
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions/sess-1/tabs")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&body),
        SENTINEL,
        "body must carry the supervisor's own message verbatim, not a substring of it"
    );

    peer.await.unwrap();
}

/// `DELETE /api/sessions/{id}/tabs/{tab_id}`'s twin of
/// `open_tab_error_reply_maps_to_404_with_the_supervisors_message` —
/// same reasoning (one representative `ErrorKind`, exact-body
/// assertion), aimed at `close_tab` instead so a route wired to the
/// wrong client method (or dropping `http_error` entirely) cannot hide
/// behind the open-tab coverage above.
#[tokio::test]
async fn close_tab_error_reply_maps_to_404_with_the_supervisors_message() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, ErrorKind};
    use tower::ServiceExt;

    const SENTINEL: &str = "SENTINEL-close-tab-9d4e17: no such tab";

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CloseTab { req_id, .. } = request else {
            panic!("expected CloseTab, got {request:?}");
        };
        writer
            .write_control(&ControlMsg::Error {
                req_id,
                message: SENTINEL.to_string(),
                kind: ErrorKind::NotFound,
            })
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("DELETE")
        .uri("/api/sessions/sess-1/tabs/tab-1")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&body),
        SENTINEL,
        "body must carry the supervisor's own message verbatim, not a substring of it"
    );

    peer.await.unwrap();
}

/// A create prepared against a connection that is no longer this host's
/// reaches NO supervisor.
///
/// Spec: `POST /api/sessions` with an `expected_incarnation` that does not
/// match the host's current connection is a 409 carrying
/// [`crate::precondition::INCARNATION_MARKER`], forwarded nowhere; the same
/// body naming the current connection is created normally.
///
/// Profile ids are helm-wide, but the action still names one installation.
/// The client checks before it sends so a retarget or adoption cannot launch
/// the resolved bundle on a successor host the user did not choose.
///
/// "Reaches no supervisor" is asserted by ORDER rather than by a timeout:
/// the peer asserts on the FIRST create it is sent, and a forwarded stale
/// create would arrive in that slot and fail there by name.
#[tokio::test]
async fn a_create_prepared_against_a_replaced_connection_reaches_no_supervisor() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame, SessionInfo};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession {
            req_id,
            source_profile,
            invocation,
            ..
        } = request
        else {
            panic!("expected CreateSession, got {request:?}");
        };
        assert_eq!(
            source_profile,
            Some(farhelm_proto::ProfileSnapshot {
                id: "starter-claude".to_string(),
                name: "claude".to_string(),
            }),
            "the only create that may reach a supervisor carries the resolved profile"
        );
        assert_eq!(invocation.as_deref(), Some("claude"));
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: SessionInfo {
                    parent: None,
                    archived: false,
                    id: "sess-new".into(),
                    title: "sess-new".into(),
                    created_at: 1_700_000_500,
                    last_activity_at: 1_700_000_500,
                    creation_seq: None,
                    cwd: "/work".into(),
                    invocation: "claude".into(),
                    status: farhelm_proto::SessionStatus::Unknown,
                    annotation: None,
                    restart_offer: farhelm_proto::RestartOffer::default(),
                    tabs: Vec::new(),
                    source_profile: None,
                },
            }))
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let local = rest_harness::local_id(&harness.store).await;
    let current = harness
        .manager
        .status(local)
        .expect("the local host has an actor")
        .incarnation;

    let (status, body) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({
            "cwd": "/work",
            "profile_id": "starter-claude",
            "expected_incarnation": current - 1,
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT);
    assert!(
        body.contains(crate::precondition::INCARNATION_MARKER),
        "a client must be able to tell this from a host that is merely busy: {body}"
    );

    // The same body naming the connection this host is actually on goes
    // through, which is what makes the refusal a precondition rather than
    // a broken path.
    let (status, _) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({
            "cwd": "/work",
            "profile_id": "starter-claude",
            "expected_incarnation": current,
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    peer.await.unwrap();
}

/// A stale claim fails the session-cache seed, and the remembered default
/// still lands — with its invalidation published.
///
/// This is the central consistency split the bare-id default introduced,
/// and nothing else pins it: [`record_session`]'s cache seed is
/// CLAIM-CHECKED (a write prepared on a replaced connection must not file a
/// session under whatever answers on the host now), while the remembered
/// default is a registry-row preference that survives exactly that
/// replacement. Reintroducing a claim check around the default — or letting
/// a refused cache seed short-circuit the bookkeeping — would pass every
/// other test in this file and fail here.
#[tokio::test]
async fn a_stale_claim_blocks_the_cache_seed_but_not_the_remembered_default() {
    use farhelm_proto::SessionInfo;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        // Hold the connection open; nothing is forwarded in this test.
        while let Ok(Some(_)) = reader.read_frame().await {}
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let local = rest_harness::local_id(&harness.store).await;
    let current = harness
        .manager
        .status(local)
        .expect("the local host has an actor")
        .incarnation;
    // A claim from the connection BEFORE this one — the shape a reply's
    // bookkeeping sees when the host reconnected mid-create.
    let stale = crate::manager::SessionClaim {
        host: local,
        incarnation: current.wrapping_sub(1),
        identity: None,
    };
    let session = SessionInfo {
        parent: None,
        archived: false,
        id: "sess-stale".into(),
        title: "sess-stale".into(),
        created_at: 1_700_000_700,
        last_activity_at: 1_700_000_700,
        creation_seq: Some(5),
        cwd: "/work".into(),
        invocation: "claude".into(),
        status: farhelm_proto::SessionStatus::Unknown,
        annotation: None,
        restart_offer: farhelm_proto::RestartOffer::default(),
        tabs: Vec::new(),
        source_profile: None,
    };

    assert!(
        harness
            .manager
            .remember_session(&stale, &session)
            .await
            .is_err(),
        "a cache seed prepared against a replaced connection must be refused"
    );

    let before = harness.manager.events().revision();
    super::remember_default_profile(&harness.state, local, "p-favorite", &session).await;
    assert_eq!(
        harness.store.remembered_profile().await.unwrap().as_deref(),
        Some("p-favorite"),
        "the default is a registry-row write and must land regardless of the claim"
    );
    assert!(
        harness.manager.events().revision() > before,
        "and its change is published so open create dialogs learn of it"
    );
    drop(harness);
    let _ = peer.await;
}

/// Issue one request against `app` and return its status and JSON body.
///
/// The tests below make several requests each and none of them is about
/// HTTP mechanics, so the builder boilerplate lives here once.
async fn get_json(
    harness: &rest_harness::Harness,
    uri: &str,
) -> (axum::http::StatusCode, serde_json::Value) {
    let request = axum::http::Request::builder()
        .method("GET")
        .uri(uri)
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = tower::ServiceExt::oneshot(harness.router(), request)
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value = serde_json::from_slice(&body)
        .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&body).into()));
    (status, value)
}

/// POST with a JSON body, returning the status and the body as text —
/// the shape every refusal assertion below needs, since a refusal's
/// body is prose rather than JSON.
async fn post_text(
    harness: &rest_harness::Harness,
    uri: &str,
    body: serde_json::Value,
) -> (axum::http::StatusCode, String) {
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    let response = tower::ServiceExt::oneshot(harness.router(), request)
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}

/// PUT with a JSON body, returning the status and the body — mirroring
/// [`get_json`] rather than [`post_text`]: `PUT /api/sessions/{id}/seen`'s
/// success body is JSON (`{}`), and its refusal is the same plain-text
/// `http_error` prose every other route's 404 uses, which the fallback
/// wraps as a JSON string the same way `get_json` does for a non-JSON body.
async fn put_json(
    harness: &rest_harness::Harness,
    uri: &str,
    body: serde_json::Value,
) -> (axum::http::StatusCode, serde_json::Value) {
    let request = axum::http::Request::builder()
        .method("PUT")
        .uri(uri)
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    let response = tower::ServiceExt::oneshot(harness.router(), request)
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value = serde_json::from_slice(&body)
        .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&body).into()));
    (status, value)
}

/// The `id` of every row of a session-list body, in order.
fn row_ids(value: &serde_json::Value) -> Vec<String> {
    value["sessions"]
        .as_array()
        .expect("sessions is an array")
        .iter()
        .map(|row| row["id"].as_str().expect("id is a string").to_string())
        .collect()
}

/// A three-host fleet where every host has sessions, sharing one
/// interleaved creation order — the fixture the merge, ordering, and
/// staleness assertions all need.
///
/// The interleaving is the point: `created_at` values alternate between
/// hosts, so a merge that concatenated per-host lists (or sorted only
/// within a host) would produce a visibly different order rather than
/// happening to agree.
async fn three_host_fleet() -> (rest_harness::Harness, store::HostId, store::HostId) {
    let (builder, alpha) = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("identity-local".to_string()),
            sessions: vec![rest_harness::session("local-mid", 200)],
            ..rest_harness::HostScript::default()
        })
        .await
        .ssh(
            "user@alpha",
            rest_harness::HostScript {
                identity: Some("identity-alpha".to_string()),
                sessions: vec![
                    rest_harness::session("alpha-new", 300),
                    rest_harness::session("alpha-old", 100),
                ],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let (builder, beta) = builder
        .ssh(
            "user@beta",
            rest_harness::HostScript {
                identity: Some("identity-beta".to_string()),
                sessions: vec![
                    rest_harness::session("beta-newest", 400),
                    rest_harness::session("beta-oldest", 50),
                ],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    let local = rest_harness::local_id(&harness.store).await;
    for host in [local, alpha, beta] {
        harness.await_refreshed(host).await;
    }
    (harness, alpha, beta)
}

/// The merged list is ONE list: every connected host's sessions in a
/// single creation-time order, each row naming its host.
///
/// SPEC.md promises "one flat list across all registered hosts, with
/// each row saying which host it lives on", and the ordering half is
/// what makes it a list rather than a concatenation. The fixture
/// interleaves creation times across hosts specifically so a
/// per-host-then-append implementation fails here instead of passing by
/// coincidence.
#[tokio::test]
async fn the_session_list_merges_every_host_into_one_creation_order() {
    let (harness, alpha, beta) = three_host_fleet().await;
    let local = rest_harness::local_id(&harness.store).await;

    let (status, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        row_ids(&value),
        vec![
            "beta-newest",
            "alpha-new",
            "local-mid",
            "alpha-old",
            "beta-oldest"
        ],
        "the merge is creation-time descending across hosts, not host by host"
    );
    assert_eq!(value["total"], 5, "total is the merged count");

    let rows = value["sessions"].as_array().unwrap();
    assert_eq!(rows[0]["host"], beta);
    assert_eq!(rows[0]["host_name"], "user@beta");
    assert_eq!(rows[1]["host"], alpha);
    assert_eq!(rows[1]["host_name"], "user@alpha");
    assert_eq!(rows[2]["host"], local);
    assert_eq!(
        rows[2]["host_name"], "this machine",
        "the helm's own machine is described rather than addressed"
    );
    assert!(
        rows.iter().all(|row| row["stale"] == false),
        "every host is connected, so nothing is last-known knowledge"
    );
}

/// A host going dark must not remove its sessions from the list: they
/// stay, marked stale, while every other host's rows keep their place
/// in the same order.
///
/// This is SPEC.md's central multi-host promise — "sessions on an
/// unreachable host stay in the list from the helm's last-known
/// knowledge, clearly marked stale, rather than vanishing" — at the
/// REST boundary, where the UI actually reads it.
#[tokio::test]
async fn a_down_hosts_sessions_stay_listed_and_marked_stale() {
    let (harness, alpha, beta) = three_host_fleet().await;

    harness.fleet.take_down(beta);
    harness
        .await_state(beta, |state| state.phase() == "unreachable-reprobing")
        .await;

    let (status, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        row_ids(&value),
        vec![
            "beta-newest",
            "alpha-new",
            "local-mid",
            "alpha-old",
            "beta-oldest"
        ],
        "a down host's rows keep their place in the merged order"
    );
    let rows = value["sessions"].as_array().unwrap();
    for row in rows {
        let expected_stale = row["host"] == beta;
        assert_eq!(
            row["stale"], expected_stale,
            "only the down host's rows are stale: {row}"
        );
    }
    assert_eq!(
        rows[1]["host"], alpha,
        "one host going down must not disturb another's rows"
    );
}

/// A session operation must reach the host that OWNS the session, and
/// only that host.
///
/// The assertion needs two live hosts, because a single-host fleet
/// cannot distinguish "routed correctly" from "sent to the only
/// connection there is" — which is exactly the bug this whole lookup
/// exists to prevent once a fleet has more than one member.
#[tokio::test]
async fn a_session_operation_routes_to_the_host_that_owns_it() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

    // The host that must NOT be asked, and the one that must.
    let (alpha_client, alpha_peer) = tokio::io::duplex(64 * 1024);
    let alpha_task = tokio::spawn(silent_supervisor(alpha_peer));
    let (beta_client, beta_peer) = tokio::io::duplex(64 * 1024);
    let beta_task = tokio::spawn(async move {
        let (r, w) = tokio::io::split(beta_peer);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::StopSession { req_id, session_id } = request else {
            panic!("expected StopSession, got {request:?}");
        };
        assert_eq!(session_id, "beta-1");
        writer
            .write_control(&ControlMsg::SessionStopped { req_id })
            .await
            .unwrap();
    });

    let (builder, alpha) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@alpha",
            rest_harness::HostScript {
                identity: Some("identity-alpha".to_string()),
                sessions: vec![rest_harness::session("alpha-1", 100)],
                peer: Some(alpha_client),
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let (builder, beta) = builder
        .ssh(
            "user@beta",
            rest_harness::HostScript {
                identity: Some("identity-beta".to_string()),
                sessions: vec![rest_harness::session("beta-1", 200)],
                peer: Some(beta_client),
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    harness.await_refreshed(alpha).await;
    harness.await_refreshed(beta).await;

    let (status, body) =
        post_text(&harness, "/api/sessions/beta-1/stop", serde_json::json!({})).await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");

    beta_task.await.unwrap();
    alpha_task.await.unwrap();
}

/// Every non-connected state refuses a session operation, names itself
/// in the error, and queues nothing.
///
/// Three states are reached the way they are reached in life — a host
/// switched off, a host upgraded past this helm's protocol, a host
/// reinstalled under a new identity — and the assertion is deliberately
/// uniform across them. SPEC.md refuses lifecycle operations against an
/// unreachable host; PLAN_M6.md item 5 makes explicit that unreachable
/// is not special, only common, and that all of these refuse alike. A
/// helm that special-cased one of them would pass a test written per
/// state and fail this one.
///
/// The other three states — `connecting`, `duplicate`, `retired` — are
/// covered by `refusal_text_names_every_non_connected_state` rather than
/// here. Reaching them through the integration path is either
/// impractical (a connecting host has to be caught mid-ladder) or
/// meaningless for a SESSION operation (a duplicate entry and a retired
/// one connect nothing, so they can never have cached a session to
/// operate on). What actually has to hold for all six is that the
/// refusal names the state, and that is what the sibling test pins —
/// against the same function this path uses.
///
/// Each host CONNECTS first, so its session is genuinely in the merged
/// view before the host breaks — otherwise there would be nothing to
/// operate on and the 409 under test would be a 404 instead.
#[tokio::test]
async fn every_non_connected_state_refuses_a_session_operation_naming_itself() {
    struct Case {
        /// How the far side changes under the host's feet.
        break_it: fn(&rest_harness::ScriptedFleet, store::HostId),
        /// The phase label the refusal must carry — the same
        /// vocabulary `/api/hosts` chips and the log lines use.
        phase: &'static str,
    }

    let cases = [
        Case {
            break_it: |fleet, host| fleet.take_down(host),
            phase: "unreachable-reprobing",
        },
        Case {
            break_it: |fleet, host| {
                fleet.edit(host, |script| {
                    script.protocol = farhelm_proto::PROTOCOL_VERSION + 1;
                });
                fleet.kill_connection(host);
            },
            phase: "version-skew",
        },
        Case {
            break_it: |fleet, host| {
                fleet.edit(host, |script| {
                    script.identity = Some("a-different-install".to_string());
                });
                fleet.kill_connection(host);
            },
            phase: "identity-mismatch",
        },
    ];

    for case in cases {
        let (builder, host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@breaks",
                rest_harness::HostScript {
                    identity: Some("identity-original".to_string()),
                    sessions: vec![rest_harness::session("owned", 100)],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(host).await;

        (case.break_it)(&harness.fleet, host);
        harness
            .await_state(host, |state| state.phase() == case.phase)
            .await;

        let (status, body) =
            post_text(&harness, "/api/sessions/owned/stop", serde_json::json!({})).await;
        assert_eq!(
            status,
            axum::http::StatusCode::CONFLICT,
            "a {} host must refuse rather than 404 or 500: {body}",
            case.phase
        );
        assert!(
            body.contains(case.phase),
            "the refusal must name the host's state ({}): {body}",
            case.phase
        );
        assert!(
            body.contains("nothing was queued"),
            "the refusal must say nothing was deferred: {body}"
        );

        // Still listed, and still marked as what it is: refusing an
        // operation must not make the session disappear.
        let (_, value) = get_json(&harness, "/api/sessions").await;
        assert_eq!(row_ids(&value), vec!["owned"]);
        assert_eq!(value["sessions"][0]["stale"], true);
    }
}

/// Every one of the six non-connected states must name itself in the
/// refusal, including the three the integration path above cannot
/// practically reach.
///
/// Asserted against `refusal_text` directly — the single function every
/// refusal in this crate is built from — because what matters is that
/// no state falls through to a generic message. A seventh state added
/// later without a case here fails this test rather than silently
/// refusing operations with nothing a user can act on.
#[test]
fn refusal_text_names_every_non_connected_state() {
    use crate::manager::{HostState, UnreachableCause};

    let cases = [
        (
            HostState::Connecting {
                attempt: 2,
                last_error: Some("ssh: connect to host timed out".to_string()),
            },
            "connecting",
            "timed out",
        ),
        (
            HostState::Unreachable {
                cause: UnreachableCause::TransportFailure,
                last_error: "no route to host".to_string(),
            },
            "unreachable-reprobing",
            "no route to host",
        ),
        (
            HostState::VersionSkew {
                peer_protocol: 9,
                peer_build: "0.0.2".to_string(),
                our_protocol: 8,
                our_build: "0.0.1".to_string(),
                remediation: "update this helm".to_string(),
            },
            "version-skew",
            "update this helm",
        ),
        (
            HostState::IdentityMismatch {
                recorded: "identity-old".to_string(),
                reported: "identity-new".to_string(),
            },
            "identity-mismatch",
            "identity-new",
        ),
        (
            HostState::Duplicate {
                twin: 7,
                identity: "identity-shared".to_string(),
            },
            "duplicate",
            "host 7",
        ),
        (
            HostState::Retired {
                reason: "its connection actor panicked".to_string(),
            },
            "retired",
            "panicked",
        ),
    ];
    assert_eq!(
        cases.len(),
        6,
        "all six non-connected states are covered; a seventh needs a case here"
    );
    for (state, phase, detail) in cases {
        let text = super::refusal_text(42, &state);
        assert!(
            text.contains(phase),
            "the refusal must name the phase {phase:?}: {text}"
        );
        assert!(
            text.contains(detail),
            "the refusal must carry the state's own evidence ({detail:?}): {text}"
        );
        assert!(
            text.contains("nothing was queued"),
            "every refusal must say nothing was deferred: {text}"
        );
        assert!(
            text.contains("host 42"),
            "every refusal must name the host: {text}"
        );
    }
}

/// Creating on a non-connected host is a PRECONDITION FAILURE: a
/// visible error naming the host's state, and no session anywhere.
///
/// SPEC.md lists "unreachable host" beside "nonexistent directory" as a
/// precondition that fails a create outright, and the silent supervisor
/// is what turns "no session anywhere" into an assertion rather than a
/// claim — a helm that refused the caller but still sent the create
/// would leave a real agent running that nobody asked for.
#[tokio::test]
async fn creating_on_a_non_connected_host_is_refused_with_no_session() {
    let (alpha_client, alpha_peer) = tokio::io::duplex(64 * 1024);
    let alpha_task = tokio::spawn(silent_supervisor(alpha_peer));

    let (builder, down) = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("identity-local".to_string()),
            peer: Some(alpha_client),
            ..rest_harness::HostScript::default()
        })
        .await
        .ssh(
            "user@down",
            rest_harness::HostScript {
                reachable: false,
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    let local = rest_harness::local_id(&harness.store).await;
    harness.await_refreshed(local).await;
    harness
        .await_state(down, |state| state.phase() == "unreachable-reprobing")
        .await;

    let (status, body) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({
            "cwd": "/tmp",
            "invocation": "agent",
            "host": down,
        }),
    )
    .await;
    assert_eq!(
        status,
        axum::http::StatusCode::CONFLICT,
        "a create against a down host fails as a precondition: {body}"
    );
    assert!(
        body.contains("unreachable-reprobing"),
        "the error must name the host's state: {body}"
    );

    // The connected host must not have been used as a fallback: a
    // create that silently landed somewhere else would be worse than
    // one that failed.
    alpha_task.await.unwrap();
}

/// A create that names no host lands on the reserved LOCAL row, and one
/// that names a host lands there instead.
///
/// The default is the tail of SPEC.md's own creation default ("…else
/// the helm's own host"), and keeping it a default rather than a
/// requirement is what leaves a curl or a script meaning the obvious
/// thing. Both halves are asserted against a two-host fleet, since a
/// single-host fleet cannot tell a default from an accident.
#[tokio::test]
async fn a_create_defaults_to_the_local_host_and_honors_an_explicit_one() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

    /// Answer one `CreateSession` with a session whose id says which
    /// host answered.
    async fn create_once(peer_side: tokio::io::DuplexStream, id: &'static str) {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession { req_id, .. } = request else {
            panic!("expected CreateSession, got {request:?}");
        };
        writer
            .write_control(&ControlMsg::SessionCreated {
                req_id,
                session: rest_harness::session(id, 1),
            })
            .await
            .unwrap();
    }

    for (explicit, expected) in [(false, "created-on-local"), (true, "created-on-remote")] {
        let (local_client, local_peer) = tokio::io::duplex(64 * 1024);
        let local_task = tokio::spawn(create_once(local_peer, "created-on-local"));
        let (remote_client, remote_peer) = tokio::io::duplex(64 * 1024);
        let remote_task = tokio::spawn(create_once(remote_peer, "created-on-remote"));

        let (builder, remote) = rest_harness::FleetBuilder::new()
            .await
            .local(rest_harness::HostScript {
                identity: Some("identity-local".to_string()),
                peer: Some(local_client),
                ..rest_harness::HostScript::default()
            })
            .await
            .ssh(
                "user@remote",
                rest_harness::HostScript {
                    identity: Some("identity-remote".to_string()),
                    peer: Some(remote_client),
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;
        harness.await_refreshed(remote).await;

        let mut body = serde_json::json!({ "cwd": "/tmp", "invocation": "agent" });
        if explicit {
            body["host"] = serde_json::json!(remote);
        }
        let (status, text) = post_text(&harness, "/api/sessions", body).await;
        assert_eq!(status, axum::http::StatusCode::OK, "{text}");
        let created: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            created["id"],
            expected,
            "a create with host {} must land on {expected}",
            if explicit { "named" } else { "omitted" }
        );

        // Whichever peer was not chosen is still parked on its read;
        // aborting is how this test declines to wait for it.
        local_task.abort();
        remote_task.abort();
    }
}

/// A terminal socket for a session on a non-connected host must be
/// refused the same way every other operation is — and must SAY so, as
/// the ordinary `detached` notice, rather than closing bare.
///
/// SPEC.md wants "no terminal to show and no pretense of one", and a
/// silent close is exactly a pretense the browser would blame on the
/// network. Riding the existing notice shape is also what lets the UI
/// render this without a new message type.
#[tokio::test]
async fn a_terminal_socket_on_a_down_host_is_refused_with_the_hosts_state() {
    let (builder, host) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@breaks",
            rest_harness::HostScript {
                identity: Some("identity-original".to_string()),
                sessions: vec![rest_harness::session("owned", 100)],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let mut harness = builder.start().await;
    harness.await_refreshed(host).await;
    harness.fleet.take_down(host);
    harness
        .await_state(host, |state| state.phase() == "unreachable-reprobing")
        .await;

    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, "/api/sessions/owned/term").await;
    let (opcode, payload) = ws.recv().await.expect("a notice, not a bare close");
    assert_eq!(opcode, 1, "the refusal arrives as a text notice");
    let notice: serde_json::Value = serde_json::from_slice(&payload).unwrap();
    assert_eq!(notice["type"], "detached");
    let reason = notice["reason"].as_str().unwrap();
    assert!(
        reason.contains("unreachable-reprobing"),
        "the notice must name the host's state: {reason}"
    );
    assert!(
        ws.recv().await.is_none(),
        "the socket closes once its refusal is delivered"
    );
}

/// A session created HERE must be operable at once — the create's own
/// reply is not a promise the helm may then take a refresh interval to
/// honour.
///
/// This is a regression test for a real gap, not a hypothetical: owner
/// routing resolves hosts from the cache, and for a while `create`
/// never seeded it, so the create dialog's own flow — create, then open
/// the terminal — 404'd until the owning host's next refresh. Every
/// verb is exercised because they route through one lookup and the
/// failure was in the lookup, not in any one of them.
///
/// No refresh tick is allowed to rescue it: the harness's cadence
/// refreshes once at connect and then not for an hour, so anything that
/// works here worked because the create seeded it.
#[tokio::test]
async fn a_session_created_here_is_routable_before_any_refresh() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        // Create, then answer every later request for the session it
        // just minted. The point is that these are REACHED at all.
        loop {
            let Ok(Some(frame)) = reader.read_frame().await else {
                return;
            };
            match parse_control(&frame) {
                Ok(ControlMsg::CreateSession { req_id, .. }) => writer
                    .write_control(&ControlMsg::SessionCreated {
                        req_id,
                        session: rest_harness::session("brand-new", 900),
                    })
                    .await
                    .unwrap(),
                Ok(ControlMsg::StopSession { req_id, session_id }) => {
                    assert_eq!(session_id, "brand-new");
                    writer
                        .write_control(&ControlMsg::SessionStopped { req_id })
                        .await
                        .unwrap();
                }
                Ok(ControlMsg::RenameSession {
                    req_id, session_id, ..
                }) => {
                    assert_eq!(session_id, "brand-new");
                    writer
                        .write_control(&ControlMsg::SessionRenamed {
                            req_id,
                            session: rest_harness::session("brand-new", 900),
                        })
                        .await
                        .unwrap();
                }
                _ => return,
            }
        }
    });

    // The scripted host's own list is EMPTY, so nothing but the create
    // can put this session where routing will find it.
    let harness = rest_harness::spliced_helm_listing(client_side, Vec::new()).await;

    let (status, body) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({ "cwd": "/tmp", "invocation": "agent" }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(created["id"], "brand-new");

    for (uri, request_body) in [
        ("/api/sessions/brand-new/stop", serde_json::json!({})),
        (
            "/api/sessions/brand-new/rename",
            serde_json::json!({ "title": "renamed" }),
        ),
    ] {
        let (status, body) = post_text(&harness, uri, request_body).await;
        assert_eq!(
            status,
            axum::http::StatusCode::OK,
            "{uri} must route immediately after the create that made it: {body}"
        );
    }

    // Deliberately NOT asserted here: the detail route asks the owning
    // host live rather than reading the cache, so what it reports is
    // the scripted list (empty) and not the seed. That is the correct
    // division — the seed exists to make the session ROUTABLE, and the
    // host remains authority for what it is — and asserting otherwise
    // would pin the cache as a detail-serving layer, which PLAN_M6.md
    // explicitly rules out.
    peer.abort();
}

/// A connected host reporting NO identity caches nothing, and its
/// sessions must still list and route — then vanish when it drops.
///
/// The gap this closes was total and silent: the manager deliberately
/// skips persisting an identity-less host's refreshes (the cache write
/// is identity-bound), while aggregation and owner lookup read only
/// persisted rows — so such a host read as connected and EMPTY, with
/// its sessions absent from the list and unroutable for every
/// operation.
///
/// The disappearance half is equally deliberate and is asserted here so
/// nobody "fixes" it later: with no durable copy there is nothing to
/// vouch for these rows once the connection is gone, so they must not
/// linger as stale entries the helm cannot stand behind.
#[tokio::test]
async fn an_identity_less_hosts_sessions_serve_while_connected_and_vanish_after() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::StopSession { req_id, session_id } = request else {
            panic!("expected StopSession, got {request:?}");
        };
        assert_eq!(session_id, "unbound-1");
        writer
            .write_control(&ControlMsg::SessionStopped { req_id })
            .await
            .unwrap();
    });

    let (builder, host) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@no-identity",
            rest_harness::HostScript {
                // A supervisor with no standing to mint one reports
                // none; the wire allows it and the store cannot bind a
                // cache write to it.
                identity: None,
                sessions: vec![rest_harness::session("unbound-1", 100)],
                peer: Some(client_side),
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    harness.await_refreshed(host).await;

    let (status, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        row_ids(&value),
        vec!["unbound-1"],
        "an identity-less host's sessions must appear in the merged list"
    );
    assert_eq!(value["total"], 1, "and must be counted in the total");
    assert_eq!(value["sessions"][0]["host"], host);
    assert_eq!(
        value["sessions"][0]["stale"], false,
        "it is connected, so these are live rows"
    );
    assert!(
        value["sessions"][0]
            .as_object()
            .unwrap()
            .contains_key("host_identity"),
        "host_identity is serialized even when null: key PRESENCE is how \
         a client tells \"this helm records no identity\" apart from \
         \"this helm predates the field\", and only the latter may \
         degrade the create default to the row-id-only check"
    );
    assert_eq!(
        value["sessions"][0]["host_identity"],
        serde_json::Value::Null,
        "an identity-less host's rows say null explicitly — a \
         skip_serializing_if regression would erase the \
         current-helm-vs-old-helm distinction and no other test would \
         notice"
    );

    // Nothing is persisted — the identity binding has nothing to bind
    // to — which is exactly why the manager has to hold them.
    assert!(
        harness
            .store
            .cached_sessions(host)
            .await
            .expect("cache read")
            .is_empty(),
        "an identity-less host must write no cache at all"
    );

    let (status, body) = post_text(
        &harness,
        "/api/sessions/unbound-1/stop",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "an identity-less host's sessions must route like any other: {body}"
    );
    peer.await.unwrap();

    harness.fleet.take_down(host);
    harness
        .await_state(host, |state| state.phase() == "unreachable-reprobing")
        .await;

    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert!(
        row_ids(&value).is_empty(),
        "with no durable copy there is nothing to serve stale: {value}"
    );
    assert_eq!(value["total"], 0);
    let (status, _) = get_json(&harness, "/api/sessions/unbound-1").await;
    assert_eq!(
        status,
        axum::http::StatusCode::NOT_FOUND,
        "and nothing to show behind a host-unreachable notice either"
    );
}

/// A hostile or buggy supervisor claiming another host's session id must
/// not be able to steer an operation to the wrong machine — and while
/// the claim STANDS, no operation goes anywhere at all.
///
/// Two rules, and the second is the one worth being explicit about.
/// helm.db refuses the second claim outright, so the LIST is coherent:
/// the first host keeps the session and the impostor's row is dropped.
/// But a session two hosts both report is genuinely ambiguous, and the
/// helm has no basis for deciding which of them the user meant — so
/// ROUTING fails closed for as long as both keep reporting it, rather
/// than quietly choosing the one that happened to cache first.
///
/// The contest is refresh STATE, not a remembered incident: when the
/// impostor stops reporting the id, the next drain rebuilds its
/// contested set without it and routing resumes with no intervention.
/// That second half is asserted here because it is what makes the
/// refusal a temporary, self-clearing condition rather than a session
/// bricked by someone else's bug.
#[tokio::test]
async fn a_second_host_claiming_a_session_id_never_steals_its_routing() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

    let (owner_client, owner_peer) = tokio::io::duplex(64 * 1024);
    let owner_task = tokio::spawn(async move {
        let (r, w) = tokio::io::split(owner_peer);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::StopSession { req_id, session_id } = request else {
            panic!("expected StopSession, got {request:?}");
        };
        assert_eq!(session_id, "contested");
        writer
            .write_control(&ControlMsg::SessionStopped { req_id })
            .await
            .unwrap();
    });
    let (impostor_client, impostor_peer) = tokio::io::duplex(64 * 1024);
    let impostor_task = tokio::spawn(silent_supervisor(impostor_peer));

    let (builder, owner) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@owner",
            rest_harness::HostScript {
                identity: Some("identity-owner".to_string()),
                sessions: vec![rest_harness::session("contested", 100)],
                peer: Some(owner_client),
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let (builder, impostor) = builder
        .ssh(
            "user@impostor",
            rest_harness::HostScript {
                identity: Some("identity-impostor".to_string()),
                // The same id, from a machine that does not own it.
                sessions: vec![rest_harness::session("contested", 100)],
                peer: Some(impostor_client),
                // Held down until the owner has cached, so "first claim
                // holds" has a defined first. Two hosts racing to claim
                // one id is a real situation and either may win it, but
                // a test whose subject is what happens to the LOSER
                // cannot also leave who loses to chance.
                reachable: false,
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    harness.await_refreshed(owner).await;

    harness
        .fleet
        .edit(impostor, |script| script.reachable = true);
    harness
        .manager
        .retry_now(impostor)
        .await
        .expect("the impostor is registered");
    harness.await_refreshed(impostor).await;

    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        row_ids(&value),
        vec!["contested"],
        "the contested id appears exactly once, not once per claimant: {value}"
    );
    assert_eq!(
        value["sessions"][0]["host"], owner,
        "the first claim holds; the later claimant's row is dropped"
    );

    // While BOTH report it, there is no honest owner to route to.
    let (status, body) = post_text(
        &harness,
        "/api/sessions/contested/stop",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(
        status,
        axum::http::StatusCode::CONFLICT,
        "a session two hosts both claim must not be routed to either: {body}"
    );
    assert!(
        body.contains(&owner.to_string()) && body.contains(&impostor.to_string()),
        "and the refusal must name both candidates so the user can fix it: {body}"
    );

    // The impostor stops claiming it. Nothing is told to forget
    // anything — the contest is rebuilt from the next drain's evidence,
    // and that evidence no longer contains the id.
    harness
        .fleet
        .edit(impostor, |script| script.sessions = Vec::new());
    harness.fleet.kill_connection(impostor);
    harness
        .await_refreshed_as(impostor, "identity-impostor", 0)
        .await;

    let (status, body) = post_text(
        &harness,
        "/api/sessions/contested/stop",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "a contest clears itself when the claimant stops claiming: {body}"
    );
    owner_task.await.unwrap();
    // The impostor must never have been asked anything about it.
    impostor_task.await.unwrap();

    let (status, value) = get_json(&harness, "/api/sessions/contested").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        value["host"], owner,
        "the detail route and the routing decision must name the SAME host"
    );
}

/// A refresh whose drain PREDATES a create must not erase the create.
///
/// The window is wide and entirely ordinary: a refresh drains a host's
/// whole list over the network, a create lands during that drain and is
/// recorded, and the drain then commits a wholesale replacement built
/// from a snapshot in which the new session did not exist. The caller
/// has already been told its session exists; the list and the routing
/// would then contradict the answer they just gave it.
///
/// Driven by a BARRIER rather than by timing: the scripted host's second
/// list reply is held until the create has completed, so the
/// interleaving under test is the one that actually happens rather than
/// whichever one a sleep happened to produce.
#[tokio::test]
async fn a_refresh_that_predates_a_create_cannot_erase_it() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        loop {
            let Ok(Some(frame)) = reader.read_frame().await else {
                return;
            };
            let Ok(ControlMsg::CreateSession { req_id, .. }) = parse_control(&frame) else {
                return;
            };
            writer
                .write_control(&ControlMsg::SessionCreated {
                    req_id,
                    session: rest_harness::session("created-mid-drain", 900),
                })
                .await
                .unwrap();
        }
    });

    // Refreshing briskly, so a second walk really is in flight while the
    // create runs. The host's canned list never mentions the new
    // session — which is the point: it describes the world before it.
    let harness = rest_harness::FleetBuilder::new()
        .await
        .refresh_every(std::time::Duration::from_millis(20))
        .local(rest_harness::HostScript {
            identity: Some("local-identity".to_string()),
            sessions: vec![rest_harness::session("pre-existing", 100)],
            peer: Some(client_side),
            ..rest_harness::HostScript::default()
        })
        .await
        .start()
        .await;
    let local = rest_harness::local_id(&harness.store).await;
    harness.await_refreshed(local).await;

    // Arm the barrier, then wait until the held walk has actually
    // STARTED: from here on, whatever it eventually replies describes a
    // world that predates the create below.
    let release = harness.fleet.hold_next_list(local);
    harness.fleet.await_list_requests(2).await;

    let (status, body) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({ "cwd": "/tmp", "invocation": "agent" }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");

    // The host now genuinely has the session, as a real one would the
    // moment it answered the create. Only the HELD reply — built before
    // any of this — still describes the world without it.
    harness.fleet.edit(local, |script| {
        script.sessions = vec![
            rest_harness::session("created-mid-drain", 900),
            rest_harness::session("pre-existing", 100),
        ];
    });

    // Let the stale walk commit — or rather, discover that it may not.
    let _ = release.send(());
    // The held walk has committed (or declined to) by the time the NEXT
    // one has started, which is a state the fleet reports rather than a
    // duration this test has to guess at.
    harness.fleet.await_list_requests(3).await;

    let (status, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let mut ids = row_ids(&value);
    ids.sort();
    assert_eq!(
        ids,
        vec!["created-mid-drain", "pre-existing"],
        "a refresh built before the create must not erase it: {value}"
    );

    // And it is routable, which is the promise the create made.
    let (host, _) = resolve_owner(&harness.state, "created-mid-drain")
        .await
        .expect("the created session must still have an owner");
    assert_eq!(host, local);
    peer.abort();
}

/// A create naming a host id nothing holds must 404, and must not fall
/// back to any other host.
///
/// The fallback is the dangerous half: a create that quietly landed on
/// the local machine because the named host was gone would put a live
/// agent somewhere the user never asked for, and the reply would look
/// like success. The silent supervisor is what turns "no fallback" into
/// an assertion rather than a claim.
#[tokio::test]
async fn creating_on_an_unknown_host_is_refused_without_falling_back() {
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(silent_supervisor(peer_side));
    let harness = rest_harness::spliced_helm(client_side).await;

    let (status, body) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({ "cwd": "/tmp", "invocation": "agent", "host": 9999 }),
    )
    .await;
    assert_eq!(
        status,
        axum::http::StatusCode::NOT_FOUND,
        "a create naming a host nothing holds is a 404, not a create somewhere else: {body}"
    );
    peer.await.unwrap();
}

/// A supervisor may list in ANY order: a reply in reverse creation order
/// refreshes cleanly, caches every row, and the REST list comes back in the
/// order the CLIENT asked for.
///
/// Protocol 14 removed the wire-order contract and its ingress validation;
/// this is the test that keeps them removed. Every scripted fixture in the
/// suite happens to list newest-first, so a validator accidentally retained
/// (or a REST path trusting peer order) would pass everything else and fail
/// only here.
#[tokio::test]
async fn a_supervisors_reply_order_is_accepted_and_resorted_per_request() {
    let harness = rest_harness::helm_listing(vec![
        // OLDEST first — the reverse of what every real supervisor sends.
        rest_harness::session("oldest", 100),
        rest_harness::session("middle", 200),
        rest_harness::session("newest", 300),
    ])
    .await;
    let (status, value) = get_json(&harness, "/api/sessions?sort=created").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        row_ids(&value),
        vec!["newest", "middle", "oldest"],
        "the helm sorts for itself; the peer's order neither fails the refresh nor leaks through"
    );
    assert_eq!(value["truncated"], false);
}

/// An identity-less host's cap flag reaches the REST reply through the
/// in-memory path.
///
/// The persisted path's flag is pinned elsewhere; this host writes no
/// cache, so its `truncated` word travels on the snapshot beside its rows —
/// a different branch of the merge, and one nothing else exercises end to
/// end.
#[tokio::test]
async fn an_identity_less_hosts_cap_flag_reaches_the_rest_reply() {
    let harness = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            // No identity: this host caches nothing, and its sessions and
            // flag are merged from the actor's own memory.
            identity: None,
            sessions: vec![rest_harness::session("memory-1", 100)],
            truncated: true,
            ..rest_harness::HostScript::default()
        })
        .await
        .start()
        .await;
    let local = rest_harness::local_id(&harness.store).await;
    harness.await_refreshed(local).await;

    let (status, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(row_ids(&value), vec!["memory-1"]);
    assert_eq!(
        value["truncated"], true,
        "the in-memory host's cap flag must reach the reply: {value}"
    );
}

/// A host whose supervisor cut its list at the cap makes the merged reply
/// say `truncated: true` — while the host is connected, after it goes
/// DOWN, and after the helm is restarted over the same helm.db.
///
/// SPEC.md's Session list section forbids presenting a cut list as the
/// whole one, and the two later legs are where that used to fail: the cut
/// rows go on being served stale in every non-connected state and across a
/// restart, so the flag has to be a property of the cached list rather than
/// of the connection. The whole path is exercised end to end — scripted
/// supervisor reply, drain, registry row, merge, REST body — because each
/// hop is a place the word can be dropped.
#[tokio::test]
async fn a_capped_hosts_notice_survives_the_host_going_down_and_a_helm_restart() {
    let (builder, host) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@capped",
            rest_harness::HostScript {
                identity: Some("identity-capped".to_string()),
                sessions: vec![rest_harness::session("cut-survivor", 100)],
                truncated: true,
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    harness.await_refreshed(host).await;

    let (status, connected) = get_json(&harness, "/api/sessions").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(row_ids(&connected), vec!["cut-survivor"]);
    assert_eq!(
        connected["truncated"], true,
        "a supervisor's cut reaches the merged reply: {connected}"
    );

    harness.fleet.take_down(host);
    harness
        .await_state(host, |state| state.phase() == "unreachable-reprobing")
        .await;
    let (_, down) = get_json(&harness, "/api/sessions").await;
    assert_eq!(row_ids(&down), vec!["cut-survivor"]);
    assert_eq!(
        down["truncated"], true,
        "the stale rows are still the cut rows, so the notice stays: {down}"
    );

    let restarted = harness.restart_with(|fleet| fleet.take_down(host)).await;
    restarted
        .await_state(host, |state| state.phase() == "unreachable-reprobing")
        .await;
    let (_, after_restart) = get_json(&restarted, "/api/sessions").await;
    assert_eq!(row_ids(&after_restart), vec!["cut-survivor"]);
    assert_eq!(
        after_restart["truncated"], true,
        "a helm restarted over the same helm.db must not present the cut list as whole: \
         {after_restart}"
    );
}

/// The stale list must survive a HELM restart: a fresh helm over the
/// same helm.db, with the host still down and no ensure file, serves
/// its sessions from the database alone.
///
/// PLAN_M6.md's testing decisions are explicit that the restart leg runs
/// WITHOUT the ensure file, because an ensure file would rebuild the
/// registry entry and mask a broken persistence path — the assertion is
/// that the destination, the identity, and the stale sessions all come
/// from helm.db.
#[tokio::test]
async fn the_stale_list_survives_a_helm_restart_from_helm_db_alone() {
    let (builder, host) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@remembered",
            rest_harness::HostScript {
                identity: Some("identity-remembered".to_string()),
                sessions: vec![rest_harness::session("survivor", 100)],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let first = builder.start().await;
    first.await_refreshed(host).await;
    let (_, before) = get_json(&first, "/api/sessions").await;
    assert_eq!(row_ids(&before), vec!["survivor"]);

    // A NEW helm over the same database, with the host now down — the
    // manager, its actors, and the router are all built from scratch.
    let restarted = first.restart_with(|fleet| fleet.take_down(host)).await;
    restarted
        .await_state(host, |state| state.phase() == "unreachable-reprobing")
        .await;

    let (status, value) = get_json(&restarted, "/api/sessions").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        row_ids(&value),
        vec!["survivor"],
        "the stale list must come back from helm.db alone: {value}"
    );
    assert_eq!(value["sessions"][0]["stale"], true);
    assert_eq!(value["sessions"][0]["host_name"], "user@remembered");

    let (status, body) = post_text(
        &restarted,
        "/api/sessions/survivor/archive",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT);
    assert!(
        body.contains("unreachable-reprobing") && body.contains(&host.to_string()),
        "archive must name the stale owner's host and current state: {body}"
    );

    let (_, hosts) = get_json(&restarted, "/api/hosts").await;
    let row = hosts["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == host)
        .expect("the registry entry survived too");
    assert_eq!(
        row["identity"], "identity-remembered",
        "the identity is durable, not re-learned from a host that is down"
    );
}

/// A session created on an IDENTITY-LESS host must be routable at once,
/// exactly like one created on a host that caches.
///
/// Such a host writes no cache at all, so the create's durable seed has
/// nowhere to go — and the version of this that only seeded the store
/// skipped it silently, leaving every immediate operation 404ing on
/// precisely the hosts whose sessions are hardest to see. The promise is
/// "created here is routable now", and it cannot hold for one storage
/// shape and not the other.
#[tokio::test]
async fn a_session_created_on_an_identity_less_host_is_routable_at_once() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        loop {
            let Ok(Some(frame)) = reader.read_frame().await else {
                return;
            };
            match parse_control(&frame) {
                Ok(ControlMsg::CreateSession { req_id, .. }) => writer
                    .write_control(&ControlMsg::SessionCreated {
                        req_id,
                        session: rest_harness::session("unbound-new", 900),
                    })
                    .await
                    .unwrap(),
                Ok(ControlMsg::StopSession { req_id, session_id }) => {
                    assert_eq!(session_id, "unbound-new");
                    writer
                        .write_control(&ControlMsg::SessionStopped { req_id })
                        .await
                        .unwrap();
                }
                _ => return,
            }
        }
    });

    let (builder, host) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@no-identity",
            rest_harness::HostScript {
                identity: None,
                sessions: Vec::new(),
                peer: Some(client_side),
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    harness.await_refreshed(host).await;

    let (status, body) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({ "cwd": "/tmp", "invocation": "agent", "host": host }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");

    let (status, body) = post_text(
        &harness,
        "/api/sessions/unbound-new/stop",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "a session created on an identity-less host must route immediately too: {body}"
    );

    // And it is in the list, in order, without waiting for a refresh.
    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(row_ids(&value), vec!["unbound-new"]);
    assert_eq!(value["total"], 1);
    peer.abort();
}

/// Lifecycle mutation replies must reach the list immediately rather
/// than waiting for the owning host's next refresh tick.
///
/// The browser suite caught both as user-visible lies. A restart of an
/// exited session succeeded while the list went on saying `exited` for a
/// poll interval; and its own shared-session reset (delete, then create)
/// left the deleted row listed beside the new one, so a strict locator
/// found two rows where the test meant one. The merged view serves what
/// the helm has RECORDED, so every mutation that changes what a session
/// is — or whether it is — records the result.
#[tokio::test]
async fn lifecycle_mutations_reach_the_list_without_waiting_for_a_refresh() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

    let exited = farhelm_proto::SessionInfo {
        status: farhelm_proto::SessionStatus::Exited { exit_code: Some(1) },
        ..rest_harness::session("sess-1", 500)
    };
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let renamed = farhelm_proto::SessionInfo {
        title: "renamed-later".to_string(),
        ..rest_harness::session("sess-1", 500)
    };
    let archived = farhelm_proto::SessionInfo {
        archived: true,
        title: "renamed-later".to_string(),
        status: farhelm_proto::SessionStatus::Exited { exit_code: None },
        ..rest_harness::session("sess-1", 500)
    };
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        loop {
            let Ok(Some(frame)) = reader.read_frame().await else {
                return;
            };
            match parse_control(&frame) {
                // The host now reports it ALIVE — the whole point of
                // the restart the caller just made.
                Ok(ControlMsg::RestartSession { req_id, .. }) => writer
                    .write_control(&ControlMsg::SessionRestarted {
                        req_id,
                        session: rest_harness::session("sess-1", 500),
                    })
                    .await
                    .unwrap(),
                Ok(ControlMsg::RenameSession { req_id, .. }) => writer
                    .write_control(&ControlMsg::SessionRenamed {
                        req_id,
                        session: renamed.clone(),
                    })
                    .await
                    .unwrap(),
                Ok(ControlMsg::ArchiveSession { req_id, .. }) => writer
                    .write_control(&ControlMsg::SessionArchived {
                        req_id,
                        session: archived.clone(),
                    })
                    .await
                    .unwrap(),
                Ok(ControlMsg::DeleteSession { req_id, .. }) => writer
                    .write_control(&ControlMsg::SessionDeleted { req_id })
                    .await
                    .unwrap(),
                _ => return,
            }
        }
    });

    // The cached row says `exited`, and the harness refreshes once an
    // hour — so anything the list shows differently was recorded by the
    // mutation itself.
    let harness = rest_harness::spliced_helm_listing(client_side, vec![exited]).await;
    let (_, before) = get_json(&harness, "/api/sessions").await;
    assert_eq!(before["sessions"][0]["status"]["state"], "exited");

    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/restart",
        serde_json::json!({ "mode": "fresh" }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let (_, after) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        after["sessions"][0]["status"]["state"], "running",
        "a completed restart must not leave the list showing the state it restarted FROM"
    );

    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/rename",
        serde_json::json!({ "title": "renamed-later" }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let (_, after) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        after["sessions"][0]["title"], "renamed-later",
        "and a completed rename must not either"
    );

    let before_archive_revision = harness.manager.events().revision();
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/archive",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["archived"],
        true
    );
    assert!(
        harness.manager.events().revision() > before_archive_revision,
        "recording a changed archive reply must wake feed subscribers"
    );
    let (_, ordinary) = get_json(&harness, "/api/sessions").await;
    assert!(row_ids(&ordinary).is_empty());
    assert_eq!(ordinary["matching"], 0);
    assert_eq!(
        ordinary["total"], 0,
        "archiving takes the session out of the default view, denominator included — the \
         list and its own count describe the same view"
    );
    let (_, retained) = get_json(&harness, "/api/sessions?include_archived=true").await;
    assert_eq!(row_ids(&retained), vec!["sess-1"]);
    assert_eq!(retained["sessions"][0]["archived"], true);
    assert_eq!(
        retained["total"], 1,
        "the session is still a fleet member; the widened view counts it"
    );

    let archived_revision = harness.manager.events().revision();
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/archive",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(
        harness.manager.events().revision(),
        archived_revision,
        "an idempotent archive reply must not publish another fleet revision"
    );

    // A delete is the quadrant the browser suite found missing: the row
    // must be gone from the list the moment the delete answers, not at
    // the next refresh.
    let request = axum::http::Request::builder()
        .method("DELETE")
        .uri("/api/sessions/sess-1")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = tower::ServiceExt::oneshot(harness.router(), request)
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let (_, after) = get_json(&harness, "/api/sessions").await;
    assert!(
        row_ids(&after).is_empty(),
        "a deleted session must leave the list at once, or a delete-then-create shows both: \
         {after}"
    );
    peer.abort();
}

/// Every session mutation reads the catalog before it asks the supervisor.
///
/// A corrupt catalog must not turn a completed create, restart, rename, or
/// archive into an error reply. This test keeps routing healthy while making
/// only profile decoding fail, then proves all four handlers refuse without
/// sending a mutation frame. The profile-backed create also pins that its
/// otherwise necessary bundle lookup shares this preflight read.
#[tokio::test]
async fn catalog_failure_precedes_every_session_mutation() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let leaked = tokio::time::timeout(Duration::from_secs(2), reader.read_frame()).await;
        assert!(
            !matches!(leaked, Ok(Ok(Some(_)))),
            "catalog failure must happen before any supervisor mutation, but one arrived: \
             {leaked:?}"
        );
    });

    let harness = rest_harness::spliced_helm_listing(
        client_side,
        vec![rest_harness::session("profile-order", 500)],
    )
    .await;
    harness
        .store
        .plant_invalid_profile_for_test()
        .await
        .expect("plant corrupt catalog row");
    assert!(
        harness.store.profiles().await.is_err(),
        "the fixture must make the mutation preflight fail"
    );

    for (path, body) in [
        (
            "/api/sessions",
            serde_json::json!({
                "cwd": "/tmp",
                "profile_id": "starter-claude",
            }),
        ),
        (
            "/api/sessions/profile-order/restart",
            serde_json::json!({ "mode": "fresh" }),
        ),
        (
            "/api/sessions/profile-order/rename",
            serde_json::json!({ "title": "not-applied" }),
        ),
        ("/api/sessions/profile-order/archive", serde_json::json!({})),
    ] {
        let (status, response) = post_text(&harness, path, body).await;
        assert_eq!(
            status,
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "{path} must fail on the catalog preflight: {response}"
        );
    }
    peer.await.unwrap();
}

/// Create, detail, restart, rename, and archive never expose `Unresolved`.
///
/// The supervisor deliberately reports only immutable profile provenance;
/// the helm owns the current existence verdict. This test returns the
/// supervisor-only marker from every live reply shape and checks the public
/// JSON after a catalog rename and deletion, covering all three public
/// verdicts without relying on a periodic refresh to rewrite the rows first.
#[tokio::test]
async fn every_live_session_reply_resolves_profile_existence_before_json() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, ProfileExistence, SessionInfo, SourceProfile};

    /// Build the supervisor's provenance-only view of the profiled session.
    ///
    /// Reconstructing it for every mutation ensures no earlier helm verdict
    /// can accidentally make a later assertion pass through cached state.
    fn unresolved_profiled_session(archived: bool) -> SessionInfo {
        SessionInfo {
            archived,
            source_profile: Some(SourceProfile {
                id: "starter-claude".to_string(),
                name: "claude".to_string(),
                existence: ProfileExistence::Unresolved,
            }),
            ..rest_harness::session("profile-live", 500)
        }
    }

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        loop {
            let Ok(Some(frame)) = reader.read_frame().await else {
                return;
            };
            match parse_control(&frame) {
                Ok(ControlMsg::CreateSession {
                    req_id,
                    source_profile,
                    ..
                }) => {
                    let snapshot = source_profile.expect("profile create carries a snapshot");
                    writer
                        .write_control(&ControlMsg::SessionCreated {
                            req_id,
                            session: SessionInfo {
                                source_profile: Some(SourceProfile {
                                    id: snapshot.id,
                                    name: snapshot.name,
                                    existence: ProfileExistence::Unresolved,
                                }),
                                ..rest_harness::session("profile-created", 600)
                            },
                        })
                        .await
                        .unwrap();
                }
                Ok(ControlMsg::RestartSession { req_id, .. }) => writer
                    .write_control(&ControlMsg::SessionRestarted {
                        req_id,
                        session: unresolved_profiled_session(false),
                    })
                    .await
                    .unwrap(),
                Ok(ControlMsg::RenameSession { req_id, .. }) => writer
                    .write_control(&ControlMsg::SessionRenamed {
                        req_id,
                        session: unresolved_profiled_session(false),
                    })
                    .await
                    .unwrap(),
                Ok(ControlMsg::ArchiveSession { req_id, .. }) => writer
                    .write_control(&ControlMsg::SessionArchived {
                        req_id,
                        session: unresolved_profiled_session(true),
                    })
                    .await
                    .unwrap(),
                other => panic!("unexpected supervisor request: {other:?}"),
            }
        }
    });

    let harness =
        rest_harness::spliced_helm_listing(client_side, vec![unresolved_profiled_session(false)])
            .await;

    let (status, created) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({ "cwd": "/tmp", "profile_id": "starter-codex" }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{created}");
    let created: serde_json::Value = serde_json::from_str(&created).unwrap();
    assert_eq!(created["source_profile"]["existence"], "present");

    let mut profile = harness
        .store
        .profiles()
        .await
        .unwrap()
        .into_iter()
        .find(|profile| profile.id == "starter-claude")
        .expect("starter profile");
    profile.name = "claude-renamed".to_string();
    harness
        .store
        .update_profile(profile)
        .await
        .unwrap()
        .expect("profile still exists");

    let (status, detail) = get_json(&harness, "/api/sessions/profile-live").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(detail["source_profile"]["existence"], "renamed");

    for path in [
        "/api/sessions/profile-live/restart",
        "/api/sessions/profile-live/rename",
    ] {
        let body = if path.ends_with("restart") {
            serde_json::json!({ "mode": "fresh" })
        } else {
            serde_json::json!({ "title": "renamed session" })
        };
        let (status, response) = post_text(&harness, path, body).await;
        assert_eq!(status, axum::http::StatusCode::OK, "{response}");
        let response: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["source_profile"]["existence"], "renamed");
    }

    assert!(
        harness
            .store
            .delete_profile("starter-claude")
            .await
            .unwrap()
    );
    let (status, archived) = post_text(
        &harness,
        "/api/sessions/profile-live/archive",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{archived}");
    let archived: serde_json::Value = serde_json::from_str(&archived).unwrap();
    assert_eq!(archived["source_profile"]["existence"], "deleted");
    peer.abort();
}

/// A mutation reply that says `Unknown` must not erase a status the
/// helm already knew.
///
/// This is the lost-restart-reply case, reduced to the fact that
/// produced it. The browser suite's `a restart whose response is lost
/// still recovers the terminal` restarts a LIVE session with the reply
/// dropped on the client side, then reads the list and expects `alive`.
/// The restart itself really happened — the supervisor relaunched, and
/// the helm received and recorded the reply — but that reply carries
/// `SessionStatus::Unknown` BY CONTRACT: at the instant it is built the
/// pane exists and the agent's own `exec` inside it has not been
/// observed, and `SessionStatus::Unknown`'s own docs are explicit that
/// `ListSessions` is the only reply computing a real answer. Recording
/// it verbatim answered a successful restart with "the helm has no
/// idea", for a session it had definite knowledge about a moment
/// earlier.
///
/// Both directions are pinned, because the rule is narrow on purpose: a
/// DEFINITE status in a reply is authoritative and wins immediately
/// (that is what makes a restart show `alive` without a refresh), and
/// only `Unknown` defers to what was already known. Every other field
/// of the reply is taken as given in both cases — the status alone is
/// knowledge the reply does not have.
#[tokio::test]
async fn a_reply_carrying_unknown_never_erases_a_known_status() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

    let alive = rest_harness::session("sess-1", 500);
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        loop {
            let Ok(Some(frame)) = reader.read_frame().await else {
                return;
            };
            let Ok(ControlMsg::RestartSession { req_id, .. }) = parse_control(&frame) else {
                return;
            };
            // Exactly what a real supervisor sends: a fresh offer and
            // a deliberately unknown status (`publish_relaunched`).
            writer
                .write_control(&ControlMsg::SessionRestarted {
                    req_id,
                    session: farhelm_proto::SessionInfo {
                        status: farhelm_proto::SessionStatus::Unknown,
                        restart_offer: farhelm_proto::RestartOffer::FreshOnly,
                        title: "restarted".to_string(),
                        ..rest_harness::session("sess-1", 500)
                    },
                })
                .await
                .unwrap();
        }
    });

    // The cached row is ALIVE, and the harness refreshes once an hour —
    // so nothing but this restart can change what the list says.
    let harness = rest_harness::spliced_helm_listing(client_side, vec![alive]).await;
    let (_, before) = get_json(&harness, "/api/sessions").await;
    assert_eq!(before["sessions"][0]["status"]["state"], "running");

    // An Unknown-carrying mutation deliberately wakes a refresh so a
    // previously exited status can converge promptly. Hold that refresh
    // here: this test is about what the MUTATION REPLY records, and its
    // fixed list fixture still describes the world before the restart.
    // Letting the refresh race the assertion made the test depend on how
    // much unrelated middleware work happened between the POST and GET.
    let local = rest_harness::local_id(&harness.store).await;
    let release_refresh = harness.fleet.hold_next_list(local);

    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/restart",
        serde_json::json!({ "mode": "fresh" }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");

    let (_, after) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        after["sessions"][0]["status"]["state"], "running",
        "a reply that says 'not yet known' must not replace knowledge with its absence: \
         {after}"
    );
    assert_eq!(
        after["sessions"][0]["title"], "restarted",
        "every other field of the reply is authoritative and lands at once"
    );
    assert_eq!(
        after["sessions"][0]["restart_offer"], "fresh_only",
        "including the freshly recomputed offer the restart exists to produce"
    );
    drop(release_refresh);
    peer.abort();
}

/// A mutation whose reply could not improve the cached status must
/// WAKE the owning host's refresh, so the definite answer arrives in one
/// round trip rather than one refresh interval.
///
/// This is the other half of the no-degrade rule, and without it that
/// rule pays for its own correctness with a visible lag. Restarting an
/// EXITED session is the case that shows it: the reply says `Unknown`
/// (deliberately — the pane exists, the agent's exec has not been
/// observed), the merge declines to record it over the cached `exited`,
/// and the list therefore goes on saying `exited` after a restart that
/// succeeded. A user watching that sees their own successful action
/// look like a failed one, for as long as the cadence says — which is
/// exactly what the browser suite caught, on one engine and not the
/// other, because a one-shot assertion races the interval.
///
/// The harness refreshes once an HOUR and this test never advances the
/// clock, so the transition asserted below cannot have come from the
/// ordinary cadence: only the wake can have produced it. The woken drain
/// must also be a POST-seed one — it samples the seed epoch when it
/// starts, so a pre-seed snapshot would correctly decline to commit and
/// leave the lag in place.
#[tokio::test]
async fn a_restart_that_cannot_improve_the_status_wakes_the_refresh() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

    let exited = farhelm_proto::SessionInfo {
        status: farhelm_proto::SessionStatus::Exited { exit_code: Some(1) },
        ..rest_harness::session("sess-1", 500)
    };
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        loop {
            let Ok(Some(frame)) = reader.read_frame().await else {
                return;
            };
            let Ok(ControlMsg::RestartSession { req_id, .. }) = parse_control(&frame) else {
                return;
            };
            // What a real supervisor sends: the relaunch happened, and
            // its liveness is not yet knowable.
            writer
                .write_control(&ControlMsg::SessionRestarted {
                    req_id,
                    session: farhelm_proto::SessionInfo {
                        status: farhelm_proto::SessionStatus::Unknown,
                        ..rest_harness::session("sess-1", 500)
                    },
                })
                .await
                .unwrap();
        }
    });

    let harness = rest_harness::spliced_helm_listing(client_side, vec![exited]).await;
    let (_, before) = get_json(&harness, "/api/sessions").await;
    assert_eq!(before["sessions"][0]["status"]["state"], "exited");

    // The host is ALIVE from here on — which the helm can only learn by
    // listing again.
    harness
        .fleet
        .edit(rest_harness::local_id(&harness.store).await, |script| {
            script.sessions = vec![rest_harness::session("sess-1", 500)];
        });

    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/restart",
        serde_json::json!({ "mode": "fresh" }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");

    // Wait for the SECOND list request — the woken drain. Counting
    // requests rather than watching for a refresh state is what makes
    // this deterministic: the connect-time refresh already produced a
    // successful one-session result, so a state-shaped wait is
    // satisfied by the pre-restart pass and proves nothing. No clock is
    // advanced anywhere in this test, so a second request can only have
    // come from the wake — and the bound turns a missing one into a
    // failed test rather than a hung CI run.
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        harness.fleet.await_list_requests(2),
    )
    .await
    .expect("the write must wake a refresh; no second list request ever arrived");
    // The wait above resolves when the fake RECEIVES the second request;
    // the helm still has to process the reply and commit it, and a loaded
    // runner can stretch that gap past a one-shot assertion (seen twice
    // in full-workspace runs, never in isolation). Polling briefly does
    // not weaken the proof: the cadence is an hour of real time, so
    // within this window the woken drain is still the only thing that
    // can have produced the transition.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    let after = loop {
        let (_, after) = get_json(&harness, "/api/sessions").await;
        if after["sessions"][0]["status"]["state"] == "running"
            || tokio::time::Instant::now() >= deadline
        {
            break after;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    };
    assert_eq!(
        after["sessions"][0]["status"]["state"], "running",
        "the definite status must arrive in one round trip, not one refresh interval: {after}"
    );
    peer.abort();
}

/// A stale session's DETAIL is served, not refused — the one
/// `/api/sessions/{id}` route a down host does not turn away.
///
/// SPEC.md: "opening such a session shows its metadata — title,
/// directory, last-known status — behind a clear host-unreachable
/// notice". Refusing here would leave the UI nothing to draw behind
/// that notice, so the read is served from the cache and marked
/// `stale`, while every mutating route on the same session still refuses
/// (pinned above). The source profile is deleted after the cache write so
/// this also pins that detail replies re-read the helm catalog instead of
/// leaking the cache's older existence verdict.
#[tokio::test]
async fn a_stale_sessions_detail_is_served_from_the_cache_and_marked_stale() {
    let (builder, host) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@breaks",
            rest_harness::HostScript {
                identity: Some("identity-original".to_string()),
                sessions: vec![farhelm_proto::SessionInfo {
                    title: "the work in progress".to_string(),
                    cwd: "/home/user/project".to_string(),
                    source_profile: Some(farhelm_proto::SourceProfile {
                        id: "starter-claude".to_string(),
                        name: "claude".to_string(),
                        existence: farhelm_proto::ProfileExistence::Unresolved,
                    }),
                    ..rest_harness::session("owned", 100)
                }],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    harness.await_refreshed(host).await;
    assert!(
        harness
            .store
            .delete_profile("starter-claude")
            .await
            .expect("delete the cached session's profile")
    );
    harness.fleet.take_down(host);
    harness
        .await_state(host, |state| state.phase() == "unreachable-reprobing")
        .await;

    let (status, value) = get_json(&harness, "/api/sessions/owned").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(value["title"], "the work in progress");
    assert_eq!(value["cwd"], "/home/user/project");
    assert_eq!(value["host"], host);
    assert_eq!(value["source_profile"]["existence"], "deleted");
    assert_eq!(
        value["stale"], true,
        "the metadata is last-known knowledge and must say so"
    );
    assert_eq!(
        value["host_identity"], "identity-original",
        "the cached detail branch must carry the registry identity too — \
         a stale session is exactly the one whose install binding the \
         create default still leans on"
    );
}

// ---- Server-side filtering (PLAN_M6_75.md item 5) ----------------
//
// Every test below drives the REAL query string through the real
// handler, because the contract is the query string: the UI builds
// these parameters and the helm answers them, and a filter asserted
// only against `SessionFilter::matches` would prove the predicate
// works without proving anything reaches it.

/// A session with the fields the filters actually read.
///
/// `rest_harness::session` fills everything with the id, which is fine
/// for ordering tests and useless here — a directory filter that
/// matched the title would pass against it.
fn filterable(
    id: &str,
    created_at: i64,
    cwd: &str,
    title: &str,
    status: farhelm_proto::SessionStatus,
    source_profile: Option<farhelm_proto::SourceProfile>,
) -> farhelm_proto::SessionInfo {
    farhelm_proto::SessionInfo {
        cwd: cwd.to_string(),
        title: title.to_string(),
        status,
        source_profile,
        ..rest_harness::session(id, created_at)
    }
}

/// The profile reference a session created from a profile carries.
///
/// `existence` is a PARAMETER rather than a fixed `Present`, and the
/// reason is the property these fixtures exist to pin: a session's
/// snapshot (`id` and `name`) is durable and never rewritten, while the
/// helm derives existence from its catalog before serving a row. So a
/// cached row can legitimately carry `Deleted` beside a name no catalog
/// holds any more — which is exactly the row the profile filter must
/// still match, by that name. Fixing this field at `Present` would make
/// every fixture describe the easy case and leave the interesting one
/// unrepresentable.
fn source(
    id: &str,
    name: &str,
    existence: farhelm_proto::ProfileExistence,
) -> farhelm_proto::SourceProfile {
    farhelm_proto::SourceProfile {
        id: id.to_string(),
        name: name.to_string(),
        existence,
    }
}

/// A two-host fleet whose four sessions differ along every filter
/// dimension at once, so that each single-dimension assertion below
/// distinguishes ONE property rather than accidentally selecting on
/// several.
///
/// One session was created from a profile that has since been DELETED,
/// which is the case SPEC.md's snapshot rule makes load-bearing: it
/// still filters, under the name it snapshotted, because nothing
/// rewrote its row when the profile went away.
async fn filterable_fleet() -> (rest_harness::Harness, store::HostId, store::HostId) {
    use farhelm_proto::{ProfileExistence, SessionStatus};

    let (builder, alpha) = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("identity-local".to_string()),
            sessions: vec![
                farhelm_proto::SessionInfo {
                    parent: Some("root-session".to_string()),
                    ..filterable(
                        "local-running",
                        400,
                        "/home/me/src/farhelm",
                        "Refactor the drain",
                        SessionStatus::Running,
                        Some(source("p-claude", "Claude Code", ProfileExistence::Present)),
                    )
                },
                filterable(
                    "local-idle",
                    300,
                    "/home/me/notes",
                    "Read the SPEC",
                    SessionStatus::Idle,
                    None,
                ),
            ],
            ..rest_harness::HostScript::default()
        })
        .await
        .ssh(
            "user@alpha",
            rest_harness::HostScript {
                identity: Some("identity-alpha".to_string()),
                sessions: vec![
                    filterable(
                        "alpha-waiting",
                        200,
                        "/srv/alpha/work",
                        "Nightly sweep",
                        SessionStatus::Waiting,
                        Some(source("p-gone", "Codex", ProfileExistence::Deleted)),
                    ),
                    farhelm_proto::SessionInfo {
                        parent: Some("root-session".to_string()),
                        ..filterable(
                            "alpha-exited",
                            100,
                            "/srv/alpha/other",
                            "Drain the queue",
                            SessionStatus::Exited { exit_code: Some(0) },
                            Some(source("p-claude", "Claude Code", ProfileExistence::Present)),
                        )
                    },
                ],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    let local = rest_harness::local_id(&harness.store).await;
    for host in [local, alpha] {
        harness.await_refreshed(host).await;
    }
    (harness, local, alpha)
}

/// Every dimension SPEC.md names — host, parent, directory, profile,
/// status, title — narrows the list by itself, with the semantics
/// `store::SessionFilter` documents.
///
/// One test rather than five because the fixture is the expensive part
/// and the assertions are one line each; what matters is that each
/// parameter selects a DIFFERENT subset, which is only visible with all
/// six side by side.
#[tokio::test]
async fn every_filter_dimension_narrows_the_list_on_its_own() {
    let (harness, local, alpha) = filterable_fleet().await;

    let (status, value) = get_json(&harness, &format!("/api/sessions?host={alpha}")).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(row_ids(&value), vec!["alpha-waiting", "alpha-exited"]);

    // Substring, and case-insensitive: a user searching a path types a
    // fragment of it, not the whole thing.
    let (_, value) = get_json(&harness, "/api/sessions?directory=SRC/farhelm").await;
    assert_eq!(row_ids(&value), vec!["local-running"]);

    // Exact, on the state tag the wire uses.
    let (_, value) = get_json(&harness, "/api/sessions?status=waiting").await;
    assert_eq!(row_ids(&value), vec!["alpha-waiting"]);

    // A status that carries a payload still filters by its tag alone —
    // `exited` selects the session whatever its exit code was.
    let (_, value) = get_json(&harness, "/api/sessions?status=exited").await;
    assert_eq!(row_ids(&value), vec!["alpha-exited"]);

    // By profile ID: exact, opaque, and rename-proof. Both hosts'
    // sessions from that profile come back, in the merged order.
    let (_, value) = get_json(&harness, "/api/sessions?profile=p-claude").await;
    assert_eq!(row_ids(&value), vec!["local-running", "alpha-exited"]);

    // Substring again, and case-insensitive again.
    let (_, value) = get_json(&harness, "/api/sessions?title=drain").await;
    assert_eq!(row_ids(&value), vec!["local-running", "alpha-exited"]);

    // Parent ids are opaque and exact; only direct children match.
    let (_, value) = get_json(&harness, "/api/sessions?parent=root-session").await;
    assert_eq!(row_ids(&value), vec!["local-running", "alpha-exited"]);

    // And the local host, so the host filter is shown selecting rather
    // than merely excluding the other one.
    let (_, value) = get_json(&harness, &format!("/api/sessions?host={local}")).await;
    assert_eq!(row_ids(&value), vec!["local-running", "local-idle"]);
}

/// SPEC.md's snapshot rule, as the LIST sees it: a session created from
/// a profile that has since been deleted still filters — under the name
/// it snapshotted, because that is the only handle anyone still has.
///
/// The alternative implementations all fail here in different ways:
/// matching only by id loses the session as soon as a user picks the
/// name they remember, and rewriting historical rows on a profile
/// delete (the shape PLAN_M6_75.md item 3 rejects) would have erased
/// the name this filter matches.
#[tokio::test]
async fn a_deleted_profiles_sessions_still_filter_under_their_snapshotted_name() {
    let (harness, _local, _alpha) = filterable_fleet().await;

    let (status, value) = get_json(&harness, "/api/sessions?profile=Codex").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        row_ids(&value),
        vec!["alpha-waiting"],
        "a deleted profile's sessions stay findable by the name they snapshotted"
    );
    assert_eq!(value["matching"], 1);

    // The id half of the same rule, for the same session: the id
    // outlives the profile too, so a client holding one still resolves.
    let (_, value) = get_json(&harness, "/api/sessions?profile=p-gone").await;
    assert_eq!(row_ids(&value), vec!["alpha-waiting"]);

    // A raw-created session matches NO profile filter — it was never
    // shaped by one, and "sessions from profile X" must not quietly
    // include sessions from no profile at all.
    let (_, value) = get_json(&harness, "/api/sessions?profile=").await;
    assert_eq!(
        row_ids(&value).len(),
        4,
        "an empty profile parameter is a cleared search box, not a filter matching nothing"
    );
}

/// Filters AND together, and the combination narrows further than
/// either alone.
///
/// Pinned because the alternative (OR, or last-parameter-wins) reads
/// identically in a single-filter test: a client refining a search would
/// see the list GROW, which is the opposite of what refining means.
#[tokio::test]
async fn combined_filters_narrow_rather_than_widen() {
    let (harness, _local, alpha) = filterable_fleet().await;

    let (_, value) = get_json(&harness, "/api/sessions?title=drain").await;
    assert_eq!(row_ids(&value), vec!["local-running", "alpha-exited"]);

    let (status, value) = get_json(
        &harness,
        &format!("/api/sessions?title=drain&host={alpha}&status=exited"),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(row_ids(&value), vec!["alpha-exited"]);
    assert_eq!(value["matching"], 1);
    assert_eq!(
        value["total"], 4,
        "the fleet total is not a function of the filter"
    );

    // A combination nothing satisfies is an empty list with an honest
    // pair of counts, never an error.
    let (status, value) = get_json(&harness, "/api/sessions?title=drain&status=waiting").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert!(row_ids(&value).is_empty());
    assert_eq!(value["matching"], 0);
    assert_eq!(value["total"], 4);
}

/// The default list excludes archived rows, so it reports a matching
/// count beside the fleet total even when no text dimension is present.
///
/// Reporting no `matching` there would have been the convenient answer,
/// and the UI does substitute `total` where a count is absent — but that
/// substitution is meant for a helm that predates filtering, and the
/// default view's archive exclusion IS a predicate this helm applied. A
/// present count is what lets the client tell the two apart.
#[tokio::test]
async fn the_default_archive_predicate_reports_both_counts() {
    let (harness, _local, _alpha) = filterable_fleet().await;

    let (status, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(value["total"], 4);
    assert_eq!(value["matching"], 4);
}

/// A status word this build does not know is a 400 naming the
/// vocabulary, never an empty list.
///
/// The failure mode this prevents is a silent lie: a client (or a user
/// hand-editing a URL) that misspells a status would otherwise be told
/// there are no such sessions, which is indistinguishable from the truth
/// and far more likely to be believed.
#[tokio::test]
async fn an_unknown_status_filter_is_refused_rather_than_matching_nothing() {
    let (harness, _local, _alpha) = filterable_fleet().await;

    let (status, body) = get_json(&harness, "/api/sessions?status=alive").await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    let text = body.as_str().unwrap_or_default();
    assert!(
        text.contains("running") && text.contains("interrupted"),
        "the refusal must name the vocabulary it accepts, got {text:?}"
    );
}

/// Whitespace is CONTENT, not noise: only the exactly-empty value clears
/// a filter.
///
/// Trimming looks harmless and is not. Directories and titles genuinely
/// contain spaces, so a trimmed search cannot find `/srv/my project`
/// by what the user typed; and trimming makes `?title=%20` mean the same
/// as `?title=`, so typing a space would silently clear the filter and
/// show everything — a change the user can see and cannot explain.
#[tokio::test]
async fn only_an_empty_filter_value_clears_it_and_whitespace_is_content() {
    use farhelm_proto::SessionStatus;

    let harness = rest_harness::helm_listing(vec![
        filterable(
            "spaced",
            200,
            "/srv/my project",
            "fix  the  spacing",
            SessionStatus::Running,
            None,
        ),
        filterable(
            "plain",
            100,
            "/srv/plain",
            "ordinary",
            SessionStatus::Running,
            None,
        ),
    ])
    .await;

    // Searchable by text that only exists WITH its whitespace.
    let (status, value) = get_json(&harness, "/api/sessions?directory=my%20project").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(row_ids(&value), vec!["spaced"]);
    let (_, value) = get_json(&harness, "/api/sessions?title=the%20%20spacing").await;
    assert_eq!(row_ids(&value), vec!["spaced"]);

    // A lone space is a real search that matches only what contains one
    // — emphatically not a cleared filter.
    let (_, value) = get_json(&harness, "/api/sessions?title=%20").await;
    assert_eq!(row_ids(&value), vec!["spaced"]);
    assert_eq!(value["matching"], 1);

    // The exactly-empty value clears the TEXT dimension. The implicit
    // archive exclusion remains, so this is still a counted predicate.
    let (_, value) = get_json(&harness, "/api/sessions?title=").await;
    assert_eq!(row_ids(&value), vec!["spaced", "plain"]);
    assert_eq!(value["total"], 2);
    assert_eq!(value["matching"], 2);
}

/// Archived sessions leave the ordinary view — its rows AND its total — and
/// reappear in both only through the inclusion switch.
///
/// The denominator half is the point: the served `total` is a count of the
/// view the request asked for, so the ordinary list can never show ten rows
/// above "of 12 sessions" with nothing typed into any filter.
#[tokio::test]
async fn archived_sessions_are_hidden_by_default_and_included_on_request() {
    let mut archived = filterable(
        "archived",
        200,
        "/tmp/archive",
        "retained",
        farhelm_proto::SessionStatus::Exited { exit_code: None },
        None,
    );
    archived.archived = true;
    let harness = rest_harness::helm_listing(vec![
        archived,
        filterable(
            "active",
            100,
            "/tmp/active",
            "ordinary",
            farhelm_proto::SessionStatus::Running,
            None,
        ),
    ])
    .await;

    let (_, ordinary) = get_json(&harness, "/api/sessions").await;
    assert_eq!(row_ids(&ordinary), vec!["active"]);
    assert_eq!(ordinary["matching"], 1);
    assert_eq!(
        ordinary["total"], 1,
        "the default view's total counts the default view: one active session"
    );

    let (_, all) = get_json(&harness, "/api/sessions?include_archived=true").await;
    assert_eq!(row_ids(&all), vec!["archived", "active"]);
    assert!(all.get("matching").is_none());
    assert_eq!(
        all["total"], 2,
        "the switch widens the view, so it widens the count with it"
    );
}

/// An identity-less host's sessions live in the manager's MEMORY rather
/// than in helm.db, so they reach the merged list by a different path —
/// and the filter has to apply on that path too.
///
/// The bug this pins is a silent one: a filter applied to one source and
/// not the other would let such a host's rows flow through unfiltered, so
/// a search would return sessions that plainly do not match beside ones
/// that do, with `matching` counting them.
#[tokio::test]
async fn a_filter_applies_to_an_identity_less_hosts_in_memory_rows() {
    use farhelm_proto::SessionStatus;

    let harness = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            // No identity: this host caches nothing, and its sessions
            // are merged in from the actor's own memory.
            identity: None,
            sessions: vec![
                filterable(
                    "memory-running",
                    200,
                    "/opt/work",
                    "Live one",
                    SessionStatus::Running,
                    None,
                ),
                filterable(
                    "memory-idle",
                    100,
                    "/opt/other",
                    "Quiet one",
                    SessionStatus::Idle,
                    None,
                ),
            ],
            ..rest_harness::HostScript::default()
        })
        .await
        .start()
        .await;
    let local = rest_harness::local_id(&harness.store).await;
    harness.await_refreshed(local).await;

    let (status, value) = get_json(&harness, "/api/sessions?status=idle").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(row_ids(&value), vec!["memory-idle"]);
    assert_eq!(value["matching"], 1);
    assert_eq!(
        value["total"], 2,
        "an identity-less host's rows count toward the fleet total like any other"
    );
}

/// The archive switch moves the denominator on the IN-MEMORY path too.
///
/// The two sources reach `total` by different code — the persisted side by
/// an indexed count over a column, the identity-less side by walking the
/// actor's own list — so the rule has to be stated twice and can drift
/// exactly once. Drifted, a fleet whose only archived session lives on an
/// identity-less host would show the ordinary list under a total that counts
/// one row it does not display, which is the incoherence the whole change
/// removes.
#[tokio::test]
async fn an_identity_less_hosts_archived_rows_leave_the_default_view_s_total() {
    use farhelm_proto::SessionStatus;

    let mut archived = filterable(
        "memory-archived",
        200,
        "/opt/work",
        "Put away",
        SessionStatus::Exited { exit_code: None },
        None,
    );
    archived.archived = true;
    let harness = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: None,
            sessions: vec![
                archived,
                filterable(
                    "memory-active",
                    100,
                    "/opt/other",
                    "Still here",
                    SessionStatus::Running,
                    None,
                ),
            ],
            ..rest_harness::HostScript::default()
        })
        .await
        .start()
        .await;
    let local = rest_harness::local_id(&harness.store).await;
    harness.await_refreshed(local).await;

    let (status, ordinary) = get_json(&harness, "/api/sessions").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(row_ids(&ordinary), vec!["memory-active"]);
    assert_eq!(
        ordinary["total"], 1,
        "an in-memory archived row is outside the default view and outside its count"
    );

    let (_, all) = get_json(&harness, "/api/sessions?include_archived=true").await;
    assert_eq!(row_ids(&all), vec!["memory-archived", "memory-active"]);
    assert_eq!(
        all["total"], 2,
        "and the switch brings it back to both, exactly as it does for a cached host"
    );
}

// ---- Listing order (`?sort=`) ------------------------------------
//
// Same posture as the filter tests above: every assertion drives the real
// query string through the real handler, because the query string IS the
// contract. An order asserted only against `ListSort::position` would prove
// the comparison works without proving a request can ask for it.

/// A session with the two fields the non-default orders read.
///
/// `rest_harness::session` copies `created_at` into `last_activity_at` and
/// the id into the title, which makes every order the same order — useless
/// here, where the whole point is telling three sequences apart.
fn sortable(
    id: &str,
    created_at: i64,
    last_activity_at: i64,
    title: &str,
) -> farhelm_proto::SessionInfo {
    farhelm_proto::SessionInfo {
        last_activity_at,
        title: title.to_string(),
        ..rest_harness::session(id, created_at)
    }
}

/// A two-host fleet whose four sessions produce three DIFFERENT sequences
/// under the three orders.
///
/// That distinctness is the fixture's job: an implementation that ignored
/// `?sort=` and served creation order throughout would pass a fixture whose
/// orders happened to coincide. Every leading component also TIES for one
/// pair, so the shared creation-order tail is under test rather than merely
/// present.
async fn sortable_fleet() -> rest_harness::Harness {
    let (builder, alpha) = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("identity-local".to_string()),
            sessions: vec![
                sortable("local-quiet", 400, 100, "Ärger"),
                sortable("local-busy", 300, 900, "Alpha"),
            ],
            ..rest_harness::HostScript::default()
        })
        .await
        .ssh(
            "user@alpha",
            rest_harness::HostScript {
                identity: Some("identity-alpha".to_string()),
                sessions: vec![
                    sortable("alpha-busy", 200, 900, "alpha"),
                    // The legacy shape: a supervisor that predates
                    // `last_activity_at` sends 0, which is "unknown" and must
                    // sort by creation time rather than at the epoch.
                    sortable("alpha-legacy", 100, 0, "zeta"),
                ],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    let local = rest_harness::local_id(&harness.store).await;
    for host in [local, alpha] {
        harness.await_refreshed(host).await;
    }
    harness
}

/// `?sort=` selects which of the three orders the merged list is served
/// in, and an unrecognized word is a 400 naming the vocabulary.
///
/// Spec: SPEC.md's session list can be ordered by recent activity, by
/// creation, or by title. The refusal matters for an unknown status's
/// reason, one dimension over: a list quietly served in a different order
/// than the one asked for looks entirely plausible, so a typo would be
/// believed rather than noticed.
#[tokio::test]
async fn the_sort_parameter_selects_the_order_and_an_unknown_word_is_refused() {
    let harness = sortable_fleet().await;

    let created = vec!["local-quiet", "local-busy", "alpha-busy", "alpha-legacy"];
    for uri in [
        // Absent, empty (a cleared control), and named: the default order is
        // what every client written before there was a choice keeps getting.
        "/api/sessions",
        "/api/sessions?sort=",
        "/api/sessions?sort=created",
    ] {
        let (status, value) = get_json(&harness, uri).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(row_ids(&value), created, "{uri} must serve creation order");
    }

    let (status, value) = get_json(&harness, "/api/sessions?sort=activity").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        row_ids(&value),
        vec!["local-busy", "alpha-busy", "local-quiet", "alpha-legacy"],
        "equal activity falls through to creation time descending, and a legacy 0 stamp sorts \
         by its own creation time rather than at the epoch"
    );

    let (status, value) = get_json(&harness, "/api/sessions?sort=title").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        row_ids(&value),
        vec!["local-busy", "alpha-busy", "alpha-legacy", "local-quiet"],
        "the title order folds case, so Alpha and alpha tie and split on the tail — and Ärger \
         sorts after zeta, which is the ordinal, locale-free collation this helm documents"
    );

    // The order is not a filter: it changes the sequence, never the
    // membership, so the numbers beside the rows do not move with it.
    for uri in [
        "/api/sessions",
        "/api/sessions?sort=activity",
        "/api/sessions?sort=title",
    ] {
        let (_, value) = get_json(&harness, uri).await;
        assert_eq!(value["total"], 4, "{uri} changed the fleet total");
        assert_eq!(value["matching"], 4, "{uri} changed the matching count");
    }

    let (status, body) = get_json(&harness, "/api/sessions?sort=cwd").await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    let text = body.as_str().unwrap_or_default();
    assert!(
        text.contains("created") && text.contains("activity") && text.contains("title"),
        "the refusal must name the vocabulary it accepts, got {text:?}"
    );
}

/// An identity-less host's IN-MEMORY rows are sorted into ONE sequence with
/// the cached rows, under every order the request can ask for.
///
/// The two sources reach the merge by different paths — helm.db for hosts
/// with an identity, the actor's memory for hosts without — and the order
/// is established over the union, in memory, per request. A merge that
/// sorted only the cached side, or trusted the host's own reported order
/// for the in-memory side, would interleave the two by whichever happened
/// to be at the front and serve a list in neither order. The fixture keeps
/// all three sequences distinct precisely so that failure could not hide
/// behind orders that happen to coincide.
#[tokio::test]
async fn an_identity_less_hosts_rows_are_reordered_into_the_requested_order() {
    let (builder, alpha) = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            // No identity: this host caches nothing, and its sessions are
            // merged in from the actor's own memory, in the order the host
            // reported them.
            identity: None,
            sessions: vec![
                // "Alpha" capitalized on the IN-MEMORY side deliberately: the
                // title order folds case, and a fold applied to one source
                // but not the other is where the two would come apart.
                sortable("memory-quiet", 300, 100, "Alpha"),
                sortable("memory-busy", 200, 900, "mid"),
            ],
            ..rest_harness::HostScript::default()
        })
        .await
        .ssh(
            "user@alpha",
            rest_harness::HostScript {
                identity: Some("identity-alpha".to_string()),
                sessions: vec![sortable("cached-mid", 250, 500, "zebra")],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    let local = rest_harness::local_id(&harness.store).await;
    for host in [local, alpha] {
        harness.await_refreshed(host).await;
    }

    let (status, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        row_ids(&value),
        vec!["memory-quiet", "cached-mid", "memory-busy"],
        "creation order, for the contrast the other two are against"
    );

    let (_, value) = get_json(&harness, "/api/sessions?sort=activity").await;
    assert_eq!(
        row_ids(&value),
        vec!["memory-busy", "cached-mid", "memory-quiet"],
        "the in-memory rows must land on either side of the cached one, once each"
    );
    let (_, value) = get_json(&harness, "/api/sessions?sort=title").await;
    assert_eq!(
        row_ids(&value),
        vec!["memory-quiet", "memory-busy", "cached-mid"],
        "and the title order is a third sequence again: the cached row's own title puts it last, \
         behind two in-memory rows that neither of the other orders puts together"
    );
}

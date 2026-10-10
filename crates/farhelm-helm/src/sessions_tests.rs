//! Session tests live in a sibling file because they occupy three quarters of the module.
//!
//! `#[path]` keeps this as `sessions::tests`, preserving private-item access without widening
//! production visibility.

use super::{resolve_owner, store};
use crate::rest_harness::{self, WsTestClient, silent_supervisor};
use std::time::Duration;

/// The composer reads its release-owned catalog and its successful-create
/// suggestions through HTTP, rather than deriving either from a session list
/// or browser-owned command rules.
///
/// This test keeps both routes on the real router. It proves the catalog is
/// available without a create request and that history is scoped through the
/// connected host identity before it becomes browser-visible.
#[farhelm_testtrace::test]
async fn composer_catalog_and_history_routes_serve_helm_owned_choices() {
    let harness = rest_harness::idle_helm().await;
    let local = rest_harness::local_id(&harness.store).await;
    let created = farhelm_proto::SessionInfo {
        creation_seq: Some(9),
        launch: crate::launches::test_agent_launch(farhelm_proto::LaunchSelection {
            harness: farhelm_proto::LaunchHarness::Codex,
            model: Some("gpt-6-astra".to_string()),
            effort: Some(farhelm_proto::LaunchEffort::High),
            permissions: Some(farhelm_proto::LaunchPermission::Yolo),
            workspace_trust: None,
        }),
        ..rest_harness::session("composer-history", 100)
    };
    assert!(
        harness
            .store
            .record_create_history(local, "local-identity", &created)
            .await
            .expect("record successful structured create")
    );

    let (catalog_status, catalog) = get_json(&harness, "/api/launch-catalog").await;
    assert_eq!(catalog_status, axum::http::StatusCode::OK);
    assert!(
        catalog
            .as_array()
            .expect("catalog array")
            .iter()
            .any(|model| {
                model["id"] == "gpt-6-astra"
                    && model["harness"] == "codex"
                    && model["efforts"] == serde_json::json!(["high"])
            }),
        "the endpoint must expose the same constrained known model the helm compiles"
    );

    let (history_status, history) =
        get_json(&harness, &format!("/api/launch-history?host={local}")).await;
    assert_eq!(history_status, axum::http::StatusCode::OK);
    assert_eq!(history["checkout_config_revision"], 0);
    harness
        .store
        .set_checkout_root(None, "/new-checkouts")
        .await
        .unwrap();
    let (_, refreshed) = get_json(&harness, &format!("/api/launch-history?host={local}")).await;
    assert_eq!(
        refreshed["checkout_config_revision"], 1,
        "feed-driven history readers must observe externally editable checkout configuration"
    );
    assert_eq!(
        history["launches"],
        serde_json::json!([{
            "host": local,
            "cwd": "/composer-history",
            "canonical_cwd": null,
            "github_repo": null,
            "selection": {
                "harness": "codex",
                "model": "gpt-6-astra",
                "effort": "high",
                "permissions": "yolo",
            },
            "created_at": 100,
            "creation_seq": 9,
        }]),
        "history returns the saved declarative selection, never a reparsed invocation"
    );
    assert_eq!(
        history["folders"].as_array().expect("folder array").len(),
        1,
        "every successful create also supplies one folder suggestion"
    );
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
/// The body's command launch names only its command and YOLO assertion,
/// so this also pins that the omitted agent type and resume command
/// forward as absent rather than as invented values.
#[farhelm_testtrace::test]
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
            cwd,
            launch,
            title,
            cols,
            rows,
            // Not under test here (the assertions below only check
            // cwd/launch/title/cols/rows); PLAN_M3.md's `intent_key` is
            // exercised by
            // `create_session_forwards_the_bodys_extras_to_the_supervisor`
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
        assert_eq!(cwd, "~/project");
        assert_eq!(
            launch,
            Some(farhelm_proto::SessionLaunch::plain_command("some-agent"))
        );
        assert_eq!(title, None);
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: SessionInfo {
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    parent: None,
                    id: "sess-1".into(),
                    title: "some-agent".into(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: None,
                    // This fixture covers a legacy supervisor reply that
                    // supplies an accepted expanded directory but no
                    // separately proven canonical identity.
                    cwd: "/canonical/home/project".into(),
                    canonical_cwd: None,
                    invocation: "some-agent".into(),
                    launch: farhelm_proto::SessionLaunch::plain_command("some-agent"),
                    // Matches real `create_session` output: `Unknown`,
                    // not a live status (creation does not establish the
                    // agent's later exec succeeded).
                    status: farhelm_proto::SessionStatus::Unknown,
                    annotation: None,
                    restart_offer: farhelm_proto::RestartOffer::default(),
                    tabs: Vec::new(),
                    github_repo: None,
                    working_copy: None,
                    notifications: Vec::new(),
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
            serde_json::json!({"cwd": "~/project", "command": {"command": "some-agent", "yolo": false}}).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let session: SessionInfo = serde_json::from_slice(&body).unwrap();
    assert_eq!(session.id, "sess-1");
    assert_eq!(session.cwd, "/canonical/home/project");

    let local = rest_harness::local_id(&harness.store).await;
    let (history_status, history) =
        get_json(&harness, &format!("/api/launch-history?host={local}")).await;
    assert_eq!(history_status, axum::http::StatusCode::OK);
    assert_eq!(
        history["folders"],
        serde_json::json!([{
            "host": local,
            "canonical_cwd": "/canonical/home/project",
            "canonical_proven": false,
            "display_cwd": "~/project",
            "created_at": 1_700_000_000_i64,
            "creation_seq": null,
        }]),
        "history must search the submitted spelling while deduplicating the verified target path"
    );

    peer.await.unwrap();
}

/// Spec: a YOLO create on a host that asks before YOLO launches is refused with a 409
/// carrying the helm's YOLO-confirmation header and the definitely-unaccepted create
/// outcome, and nothing reaches the supervisor; the same create with `confirm_yolo` is
/// dispatched; once the host allows YOLO without asking a YOLO create needs no override.
///
/// Why: the header is the browser's only cue to show the confirmation (it is
/// never read from text a supervisor could write), the unaccepted outcome is
/// what lets the confirmed retry be a new request, and "nothing dispatched"
/// is the whole point of refusing.
#[farhelm_testtrace::test]
async fn a_yolo_create_on_a_host_that_asks_is_refused_until_confirmed() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame, SessionInfo};
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    // Answers exactly the two creates that should arrive (the confirmed one
    // and the one on the safe host); a refused create that leaked through
    // would be answered here instead and fail the ids asserted below.
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        for id in ["sess-confirmed", "sess-safe"] {
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CreateSession { req_id, launch, .. } = request else {
                panic!("expected CreateSession, got {request:?}");
            };
            assert_eq!(
                launch.as_ref().map(|launch| launch.display_command()),
                Some("codex --yolo".to_string())
            );
            writer
                .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                    req_id,
                    session: SessionInfo {
                        agent_kind: farhelm_proto::AgentKind::Codex,
                        parent: None,
                        id: id.into(),
                        title: "codex".into(),
                        created_at: 1_700_000_000,
                        last_activity_at: 1_700_000_000,
                        last_work_started_at: 0,
                        creation_seq: None,
                        cwd: "/project".into(),
                        canonical_cwd: None,
                        invocation: "codex --yolo".into(),
                        launch: farhelm_proto::SessionLaunch::plain_command("codex --yolo"),
                        status: farhelm_proto::SessionStatus::Unknown,
                        annotation: None,
                        restart_offer: farhelm_proto::RestartOffer::default(),
                        tabs: Vec::new(),
                        github_repo: None,
                        working_copy: None,
                        notifications: Vec::new(),
                    },
                }))
                .await
                .unwrap();
        }
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let local = rest_harness::local_id(&harness.store).await;
    assert!(
        !host_yolo_without_asking(&harness.store, local).await,
        "premise: the local host starts asking before YOLO launches"
    );
    let post = |body: serde_json::Value| {
        axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap()
    };

    let refused = harness
        .router()
        .oneshot(post(
            serde_json::json!({"cwd": "/project", "command": {"command": "codex --yolo", "yolo": true}}),
        ))
        .await
        .unwrap();
    assert_eq!(refused.status(), axum::http::StatusCode::CONFLICT);
    assert_eq!(
        refused.headers()[farhelm_proto::http::YOLO_CONFIRMATION_HEADER],
        farhelm_proto::http::YOLO_CONFIRMATION_REQUIRED
    );
    assert_eq!(
        refused.headers()[farhelm_proto::http::CREATE_OUTCOME_HEADER],
        farhelm_proto::http::CREATE_OUTCOME_DEFINITELY_UNACCEPTED
    );

    let confirmed = harness
        .router()
        .oneshot(post(serde_json::json!({
            "cwd": "/project",
            "command": {"command": "codex --yolo", "yolo": true},
            "confirm_yolo": true,
        })))
        .await
        .unwrap();
    assert_eq!(confirmed.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(confirmed.into_body(), usize::MAX)
        .await
        .unwrap();
    let session: SessionInfo = serde_json::from_slice(&body).unwrap();
    assert_eq!(session.id, "sess-confirmed");

    allow_yolo_here(&harness.store).await;
    let safe = harness
        .router()
        .oneshot(post(
            serde_json::json!({"cwd": "/project", "command": {"command": "codex --yolo", "yolo": true}}),
        ))
        .await
        .unwrap();
    assert_eq!(safe.status(), axum::http::StatusCode::OK);

    peer.await.unwrap();
}

/// Spec: a command launch asserted YOLO is refused on a host that asks
/// before YOLO launches, with the helm's YOLO-confirmation header, whatever
/// its command line spells: a plain `sleep` and Codex's no-approvals mode
/// alike.
///
/// Why: SPEC.md makes a command launch's YOLO verdict the assertion alone,
/// with no command-line parsing; a guard that still read the command would
/// let an asserted YOLO launch through when its spelling looked harmless.
#[farhelm_testtrace::test]
async fn an_asserted_yolo_command_on_a_host_that_asks_is_refused_whatever_it_spells() {
    use tower::ServiceExt;

    let harness = rest_harness::idle_helm().await;
    let local = rest_harness::local_id(&harness.store).await;
    assert!(
        !host_yolo_without_asking(&harness.store, local).await,
        "premise: the local host starts asking before YOLO launches"
    );
    for command in ["sleep 300", "codex -a never -s danger-full-access"] {
        let refused = harness
            .router()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/sessions")
                    .header("host", "127.0.0.1:7433")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::json!({"cwd": "/project", "command": {"command": command, "yolo": true}})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            refused.status(),
            axum::http::StatusCode::CONFLICT,
            "{command}"
        );
        assert_eq!(
            refused.headers()[farhelm_proto::http::YOLO_CONFIRMATION_HEADER],
            farhelm_proto::http::YOLO_CONFIRMATION_REQUIRED,
            "{command}"
        );
    }
}

/// Mark the harness's local host safe for YOLO launches.
///
/// Every host starts asking before YOLO launches, so a test that launches YOLO for some other
/// reason would otherwise meet the guard's refusal instead of the behavior it
/// is about.
async fn allow_yolo_here(store: &crate::store::HelmStore) {
    let local = rest_harness::local_id(store).await;
    store.set_yolo_without_asking(local, true).await.unwrap();
    // The premise the caller's launch relies on, read back from the store.
    assert!(
        host_yolo_without_asking(store, local).await,
        "the local host must read back as safe for YOLO launches"
    );
}

/// The stored YOLO setting of one host, read back from the registry so a
/// guard test can establish its premise independently of the launch whose
/// behavior it is testing.
async fn host_yolo_without_asking(
    store: &crate::store::HelmStore,
    host: crate::store::HostId,
) -> bool {
    store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == host)
        .expect("the host row exists")
        .yolo_without_asking
}

/// A structured launch keeps the submitted home-relative spelling for history
/// search while the session itself carries the supervisor's accepted path and
/// its separately verified canonical destination. Replaying an intent key is
/// deliberately included: it is where a second history observation could
/// silently inflate the setup's frequency or replace its display spelling.
#[farhelm_testtrace::test]
async fn structured_tilde_create_replay_keeps_all_three_path_facts_distinct() {
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
            .expect("complete supervisor handshake");
        for _ in 0..2 {
            let request = parse_control(
                &reader
                    .read_frame()
                    .await
                    .expect("read create frame")
                    .expect("create frame present"),
            )
            .expect("decode create frame");
            let ControlMsg::CreateSession {
                req_id,
                cwd,
                intent_key,
                ..
            } = request
            else {
                panic!("expected CreateSession, got {request:?}");
            };
            assert_eq!(cwd, "~/work/project");
            assert_eq!(intent_key.as_deref(), Some("same-intent"));
            writer
                .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                    req_id,
                    session: SessionInfo {
                        agent_kind: farhelm_proto::AgentKind::Generic,
                        parent: None,
                        id: "structured-tilde".into(),
                        title: "structured tilde".into(),
                        created_at: 1_700_000_001,
                        last_activity_at: 1_700_000_001,
                        last_work_started_at: 0,
                        creation_seq: Some(77),
                        cwd: "/home/person/work/project".into(),
                        canonical_cwd: Some("/srv/repo/project".into()),
                        invocation: "codex --model gpt-6-astra".into(),
                        launch: crate::launches::test_agent_launch(
                            farhelm_proto::LaunchSelection {
                                harness: farhelm_proto::LaunchHarness::Codex,
                                model: Some("gpt-6-astra".into()),
                                effort: Some(farhelm_proto::LaunchEffort::High),
                                permissions: Some(farhelm_proto::LaunchPermission::Yolo),
                                workspace_trust: None,
                            },
                        ),
                        status: farhelm_proto::SessionStatus::Unknown,
                        annotation: None,
                        restart_offer: farhelm_proto::RestartOffer::default(),
                        tabs: Vec::new(),
                        github_repo: None,
                        working_copy: None,
                        notifications: Vec::new(),
                    },
                }))
                .await
                .expect("reply to structured create");
        }
    });
    let harness = rest_harness::spliced_helm(client_side).await;
    // YOLO launch on the fixture's ask-first host; the guard
    // is not what this test is about (see `yolo_guard`'s own tests).
    allow_yolo_here(&harness.store).await;
    let app = harness.router();
    let body = serde_json::json!({
        "cwd": "~/work/project",
        "intent_key": "same-intent",
        "launch": {
            "harness": "codex",
            "model": "gpt-6-astra",
            "effort": "high",
            "permissions": "yolo"
        }
    })
    .to_string();
    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/sessions")
                    .header("host", "127.0.0.1:7433")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(body.clone()))
                    .expect("build structured create request"),
            )
            .await
            .expect("run structured create request");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let created: SessionInfo = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("read structured create reply"),
        )
        .expect("decode structured create reply");
        assert_eq!(created.cwd, "/home/person/work/project");
        assert_eq!(created.canonical_cwd.as_deref(), Some("/srv/repo/project"));
    }
    let local = rest_harness::local_id(&harness.store).await;
    let (status, history) = get_json(&harness, &format!("/api/launch-history?host={local}")).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        history["launches"].as_array().expect("launch array").len(),
        1
    );
    assert_eq!(history["launches"][0]["cwd"], "~/work/project");
    assert_eq!(history["launches"][0]["canonical_cwd"], "/srv/repo/project");
    assert_eq!(history["folders"][0]["display_cwd"], "~/work/project");
    assert_eq!(history["folders"][0]["canonical_cwd"], "/srv/repo/project");
    assert_eq!(history["folders"][0]["canonical_proven"], true);
    peer.await.expect("join scripted supervisor");
}

/// A successful STRUCTURED launch is the only kind that may move the
/// helm-wide "last permissions used" memory (SPEC.md's launch-composer
/// carve-out, decided alongside this test): a yolo launch sets it, a
/// LEGACY (raw) launch in between leaves it exactly as it was — proving
/// non-interference against a non-default baseline, not merely against an
/// already-empty one — and a later structured launch with explicit default
/// permissions clears it again.
///
/// Driven through the real `POST /api/sessions` handler end to end, not
/// `HelmStore::record_create_history` directly: the write lives at the same
/// instrumentation point as `launch_history`'s own recording
/// (`sessions::do_create_session`), and what this test pins is that an
/// ordinary browser-facing request reaches it — `store.rs`'s own
/// preference tests already cover the store function in isolation.
#[farhelm_testtrace::test]
async fn a_successful_structured_launch_remembers_its_permissions_choice() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame, LaunchPermission, LaunchSelection, SessionInfo};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .expect("complete supervisor handshake");
        // One scripted reply per create below, in the exact order the test
        // drives them: yolo, then a legacy raw create, then an
        // explicit-default structured create, then Goose's omitted YOLO
        // default.
        let replies: [(&str, Option<LaunchSelection>); 4] = [
            (
                "yolo-launch",
                Some(LaunchSelection {
                    harness: farhelm_proto::LaunchHarness::Codex,
                    model: None,
                    effort: None,
                    permissions: Some(LaunchPermission::Yolo),
                    workspace_trust: None,
                }),
            ),
            ("legacy-launch", None),
            (
                "default-launch",
                Some(LaunchSelection {
                    harness: farhelm_proto::LaunchHarness::Codex,
                    model: None,
                    effort: None,
                    permissions: None,
                    workspace_trust: None,
                }),
            ),
            (
                "goose-default-launch",
                Some(LaunchSelection {
                    harness: farhelm_proto::LaunchHarness::Goose,
                    model: None,
                    effort: None,
                    permissions: Some(LaunchPermission::Yolo),
                    workspace_trust: None,
                }),
            ),
        ];
        for (id, launch) in replies {
            let request = parse_control(
                &reader
                    .read_frame()
                    .await
                    .expect("read create frame")
                    .expect("create frame present"),
            )
            .expect("decode create frame");
            let ControlMsg::CreateSession { req_id, .. } = request else {
                panic!("expected CreateSession, got {request:?}");
            };
            writer
                .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                    req_id,
                    session: SessionInfo {
                        agent_kind: farhelm_proto::AgentKind::Generic,
                        parent: None,
                        id: id.into(),
                        title: id.into(),
                        created_at: 1_700_000_000,
                        last_activity_at: 1_700_000_000,
                        last_work_started_at: 0,
                        creation_seq: Some(1),
                        cwd: "/work".into(),
                        canonical_cwd: None,
                        invocation: "codex".into(),
                        launch: launch
                            .map(crate::launches::test_agent_launch)
                            .unwrap_or_else(|| {
                                farhelm_proto::SessionLaunch::plain_command("codex")
                            }),
                        status: farhelm_proto::SessionStatus::Unknown,
                        annotation: None,
                        restart_offer: farhelm_proto::RestartOffer::default(),
                        tabs: Vec::new(),
                        github_repo: None,
                        working_copy: None,
                        notifications: Vec::new(),
                    },
                }))
                .await
                .expect("reply to create");
        }
    });

    let harness = rest_harness::spliced_helm(client_side).await;

    // YOLO launch on the fixture's ask-first host; the guard

    // is not what this test is about (see `yolo_guard`'s own tests).

    allow_yolo_here(&harness.store).await;

    let (status, _) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({
            "cwd": "/work",
            "launch": { "harness": "codex", "model": null, "effort": null, "permissions": "yolo" },
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let (_, preferences) = get_json(&harness, "/api/preferences").await;
    assert_eq!(
        preferences["remembered_permissions"], "yolo",
        "a successful structured launch with yolo sets the preference"
    );

    let (status, _) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({ "cwd": "/work", "command": {"command": "codex", "yolo": false} }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let (_, preferences) = get_json(&harness, "/api/preferences").await;
    assert_eq!(
        preferences["remembered_permissions"], "yolo",
        "a legacy (raw) launch leaves the remembered permissions mode alone"
    );

    let (status, _) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({
            "cwd": "/work",
            "launch": { "harness": "codex", "model": null, "effort": null, "permissions": null },
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let (_, preferences) = get_json(&harness, "/api/preferences").await;
    assert_eq!(
        preferences.get("remembered_permissions"),
        None,
        "a following structured launch with default permissions clears the memory"
    );

    let (status, _) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({
            "cwd": "/work",
            "launch": { "harness": "goose", "model": null, "effort": null, "permissions": null },
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let (_, preferences) = get_json(&harness, "/api/preferences").await;
    assert_eq!(
        preferences.get("remembered_permissions"),
        None,
        "a harness's omitted YOLO default must not become a remembered override"
    );

    peer.await.expect("join scripted supervisor");
}

/// Spec: remembered launch defaults come from what the user submitted, not
/// from the supervisor's reply: a structured create that asked for the
/// default permissions remembers no permission mode even when the reply
/// claims `yolo` with workspace trust, and a raw create whose reply carries
/// a launch moves nothing.
///
/// Why: the reply is written by the remote host. A compromised host
/// answering creates with `yolo` plus trust used to make those the helm-wide
/// defaults the next New dialog preselects on every host. SPEC.md's rule is
/// that only explicit GUI selections shape GUI defaults.
#[farhelm_testtrace::test]
async fn remembered_defaults_follow_the_submitted_launch_not_the_reply() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame, LaunchPermission, LaunchSelection};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .expect("complete supervisor handshake");
        // Every reply claims yolo with workspace trust, whatever was asked.
        for (index, id) in ["structured-asked-default", "raw-create"]
            .into_iter()
            .enumerate()
        {
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CreateSession { req_id, .. } = request else {
                panic!("expected CreateSession, got {request:?}");
            };
            let mut session = rest_harness::session(id, 1_700_000_000);
            session.creation_seq = Some(index as u64 + 1);
            session.launch = crate::launches::test_agent_launch(LaunchSelection {
                harness: farhelm_proto::LaunchHarness::Codex,
                model: None,
                effort: None,
                permissions: Some(LaunchPermission::Yolo),
                workspace_trust: Some(true),
            });
            writer
                .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                    req_id,
                    session,
                }))
                .await
                .unwrap();
        }
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({
            "cwd": "/work",
            "launch": { "harness": "codex", "model": null, "effort": null, "permissions": null },
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let (_, preferences) = get_json(&harness, "/api/preferences").await;
    assert_eq!(
        preferences.get("remembered_permissions"),
        None,
        "the user asked for the default, so the reply's yolo must not be remembered: {preferences}"
    );
    assert_ne!(
        preferences.get("remembered_workspace_trust"),
        Some(&serde_json::json!(true)),
        "the reply's workspace trust must not become the default: {preferences}"
    );

    // An asserted-YOLO command is the strong case: the launch IS YOLO, but
    // the user said so about a command, not about the launcher's agent
    // choices, so it must not become the launcher's remembered default.
    // The host is made safe for YOLO so the guard does not refuse first.
    allow_yolo_here(&harness.store).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({ "cwd": "/work", "command": {"command": "codex", "yolo": true} }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let (_, preferences) = get_json(&harness, "/api/preferences").await;
    assert_eq!(
        preferences.get("remembered_permissions"),
        None,
        "a command launch, even an asserted-YOLO one, must not move the defaults: {preferences}"
    );

    peer.await.expect("join scripted supervisor");
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
#[farhelm_testtrace::test]
async fn create_session_forwards_the_bodys_extras_to_the_supervisor() {
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
            intent_key,
            launch,
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
        assert_eq!(
            launch,
            Some(farhelm_proto::SessionLaunch::Command(
                farhelm_proto::CommandLaunch {
                    command: "some-agent {farhelm_args}".to_string(),
                    yolo: false,
                    agent: Some(farhelm_proto::LaunchHarness::Claude),
                    resume: Some("some-agent --resume {conversation} {farhelm_args}".to_string()),
                }
            )),
            "the command launch reaches the supervisor as written"
        );
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: SessionInfo {
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    parent: None,
                    id: "sess-1".into(),
                    title: "t".into(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: None,
                    cwd: "/some/dir".into(),
                    canonical_cwd: None,
                    invocation: "some-agent".into(),
                    launch: farhelm_proto::SessionLaunch::plain_command("some-agent"),
                    status: farhelm_proto::SessionStatus::Unknown,
                    annotation: None,
                    restart_offer: farhelm_proto::RestartOffer::default(),
                    tabs: Vec::new(),
                    github_repo: None,
                    working_copy: None,
                    notifications: Vec::new(),
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
                "command": {
                    "command": "some-agent {farhelm_args}",
                    "yolo": false,
                    "agent": "claude",
                    "resume": "some-agent --resume {conversation} {farhelm_args}",
                },
                "intent_key": "intent-from-the-browser",
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
/// 200, preserving bare successes and carrying a true archive fact through
/// to the client that will decide whether to animate its own Delete.
#[farhelm_testtrace::test]
async fn delete_session_happy_path_returns_200_with_the_archive_fact() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use tower::ServiceExt;

    for archived in [false, true] {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::DeleteSession {
                req_id, session_id, ..
            } = request
            else {
                panic!("expected DeleteSession, got {request:?}");
            };
            assert_eq!(session_id, "sess-1");
            writer
                .write_control(&ControlMsg::SessionDeleted {
                    req_id,
                    archived,
                    notice: None,
                })
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
        assert_eq!(
            value,
            if archived {
                serde_json::json!({"archived":true})
            } else {
                serde_json::json!({})
            }
        );

        peer.await.unwrap();
    }
}

/// A completed delete that left a checkout in place reaches the browser as
/// a `notice` in the success body.
///
/// Why it matters: SPEC.md "Managed checkouts" makes archiving never
/// block Delete, and a checkout left unarchived must never be silent; the
/// supervisor's notice is the only thing that tells the user where the
/// folder is. Specified: a scripted `SessionDeleted` with a notice answers
/// 200 with `{"notice": ...}` carrying that text.
#[farhelm_testtrace::test]
async fn delete_session_passes_the_supervisors_notice_to_the_caller() {
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
        let ControlMsg::DeleteSession { req_id, .. } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        writer
            .write_control(&ControlMsg::SessionDeleted {
                req_id,
                archived: false,
                notice: Some("The managed checkout at /work/bar-1 was not archived".to_string()),
            })
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let request = axum::http::Request::builder()
        .method("DELETE")
        .uri("/api/sessions/sess-1")
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = harness.router().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"notice": "The managed checkout at /work/bar-1 was not archived"})
    );

    peer.await.unwrap();
}

/// Why this matters: the browser's unconfirmed delete, and a confirmed one
/// whose prompt promised less than what is alive by the time it lands, rely
/// on the supervisor refusing, which only works if the helm forwards the
/// precondition instead of dropping it. Spec: `?only_if_nothing_alive=true`
/// and `?only_if_agent_ended=true` reach the supervisor as the matching
/// `farhelm_proto::DeleteGuard`, and a plain DELETE sends neither (the
/// unconditional delete).
#[farhelm_testtrace::test]
async fn delete_session_forwards_its_precondition() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use tower::ServiceExt;

    use farhelm_proto::DeleteGuard;
    for (uri, expected) in [
        (
            "/api/sessions/sess-1?only_if_nothing_alive=true",
            DeleteGuard::NothingAlive,
        ),
        (
            "/api/sessions/sess-1?only_if_agent_ended=true",
            DeleteGuard::AgentEnded,
        ),
        ("/api/sessions/sess-1", DeleteGuard::Unconditional),
    ] {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::DeleteSession {
                req_id,
                only_if_nothing_alive,
                only_if_agent_ended,
                ..
            } = request
            else {
                panic!("expected DeleteSession, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::SessionDeleted {
                    req_id,
                    archived: false,
                    notice: None,
                })
                .await
                .unwrap();
            DeleteGuard::from_flags(only_if_nothing_alive, only_if_agent_ended)
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let request = axum::http::Request::builder()
            .method("DELETE")
            .uri(uri)
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = harness.router().oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK, "{uri}");
        assert_eq!(peer.await.unwrap(), expected, "{uri}");
    }
}

/// Deleting an unknown id must 404 from the helm's own owner lookup,
/// the delete-side twin of
/// `stop_session_unknown_id_returns_404_with_supervisor_message` — see
/// that test's docs for why this contract inverted with M6's routing,
/// and why the silent supervisor is half the assertion.
#[farhelm_testtrace::test]
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

/// A successful delete also drops the session's `session_seen` and
/// notification-mark rows (SPEC_impl.md's `session_seen` paragraph and its
/// notification storage note), not merely the session itself — a
/// stray row would sit in the table forever with nothing to ever read it
/// back out, since the id it names is gone. Mirrors
/// `delete_session_happy_path_returns_200_with_the_archive_fact`'s
/// scripted-peer shape; the only addition is marking the session seen
/// before the delete and checking the store directly afterward, since the
/// REST reply carries no evidence either way (SPEC.md's "delete" leaves
/// nothing behind to inspect through the API once the session is gone).
#[farhelm_testtrace::test]
async fn delete_session_drops_the_seen_and_notification_mark_rows() {
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
        let ControlMsg::DeleteSession {
            req_id, session_id, ..
        } = request
        else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        writer
            .write_control(&ControlMsg::SessionDeleted {
                req_id,
                archived: false,
                notice: None,
            })
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
        .raise_notification_marks("sess-1", 2, crate::store::NotificationMark::Cleared)
        .await
        .unwrap();
    assert_eq!(
        harness
            .store
            .notification_marks(&["sess-1".to_string()])
            .await
            .unwrap()
            .get("sess-1"),
        Some(&(2, 2)),
        "fixture premise: the marks exist before the delete"
    );

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
    assert!(
        harness
            .store
            .notification_marks(&["sess-1".to_string()])
            .await
            .unwrap()
            .is_empty(),
        "the notification marks must not survive the session they name"
    );

    peer.await.unwrap();
}

/// `PUT /api/sessions/{id}/seen` end to end: a mark, a listing read, a
/// clear, and another listing read, checking `seen_activity_at` at each
/// step — the round trip the idle dot's grey/blue split actually depends
/// on (SPEC.md, Status).
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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

/// A listed session carrying notifications 1 through `count`, as a
/// supervisor reports them (newest first).
fn session_with_notifications(id: &str, count: u64) -> farhelm_proto::SessionInfo {
    let mut info = rest_harness::session(id, 1_700_000_000);
    info.notifications = (1..=count)
        .rev()
        .map(|seq| farhelm_proto::SessionNotification {
            seq,
            at: 1_700_000_000 + seq as i64,
            text: format!("notification {seq}"),
            resolved: false,
        })
        .collect();
    info
}

/// The notification routes end to end through the listing: reading marks
/// entries read without hiding them, clearing hides them from the row, the
/// row reports the read mark the UI uses to tell unread from read, and the
/// fleet-events revision moves only when a mark did (SPEC.md, Status;
/// SPEC_impl.md's notification storage note). This is the round trip the
/// bell's loud and quiet states depend on.
#[farhelm_testtrace::test]
async fn notification_routes_read_then_clear_through_the_listing() {
    let harness = rest_harness::helm_listing(vec![session_with_notifications("sess-1", 3)]).await;
    let seqs = |listing: &serde_json::Value| -> Vec<u64> {
        listing["sessions"][0]["notifications"]
            .as_array()
            .map(|entries| entries.iter().map(|n| n["seq"].as_u64().unwrap()).collect())
            .unwrap_or_default()
    };

    let (_, listing) = get_json(&harness, "/api/sessions").await;
    assert_eq!(seqs(&listing), vec![3, 2, 1]);
    assert_eq!(listing["sessions"][0]["notifications_read_through"], 0);

    let before = harness.manager.events().revision();
    let (status, body) = put_json(
        &harness,
        "/api/sessions/sess-1/notifications/read",
        serde_json::json!({ "through": 3 }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body, serde_json::json!({}));
    assert!(
        harness.manager.events().revision() > before,
        "a moved mark bumps"
    );
    let (_, listing) = get_json(&harness, "/api/sessions").await;
    assert_eq!(seqs(&listing), vec![3, 2, 1], "reading hides nothing");
    assert_eq!(listing["sessions"][0]["notifications_read_through"], 3);

    let before_repeat = harness.manager.events().revision();
    let (status, _) = put_json(
        &harness,
        "/api/sessions/sess-1/notifications/read",
        serde_json::json!({ "through": 2 }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        harness.manager.events().revision(),
        before_repeat,
        "an older read mark moves nothing and must not bump"
    );

    let (status, _) = put_json(
        &harness,
        "/api/sessions/sess-1/notifications/cleared",
        serde_json::json!({ "through": 2 }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let (_, listing) = get_json(&harness, "/api/sessions").await;
    assert_eq!(seqs(&listing), vec![3], "cleared entries leave the row");
    assert_eq!(listing["sessions"][0]["notifications_read_through"], 3);

    let (_, single) = get_json(&harness, "/api/sessions/sess-1").await;
    assert_eq!(
        single["notifications"].as_array().map(Vec::len),
        Some(1),
        "the single-session read applies the same marks as the listing"
    );
}

/// A mark is clamped to the newest notification the helm holds for the
/// session: a client can only mean entries it was shown, and a mark past
/// them would hide a notification the supervisor records later before any
/// client saw it. A session with no notifications therefore cannot be
/// marked at all, and that is not a change worth a bump.
#[farhelm_testtrace::test]
async fn notification_marks_are_clamped_to_the_newest_known_entry() {
    let harness = rest_harness::helm_listing(vec![
        session_with_notifications("sess-1", 3),
        rest_harness::session("quiet", 1_700_000_000),
    ])
    .await;

    let (status, _) = put_json(
        &harness,
        "/api/sessions/sess-1/notifications/cleared",
        serde_json::json!({ "through": 99 }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        harness
            .store
            .notification_marks(&["sess-1".to_string()])
            .await
            .unwrap()
            .get("sess-1"),
        Some(&(3, 3))
    );

    let before = harness.manager.events().revision();
    let (status, _) = put_json(
        &harness,
        "/api/sessions/quiet/notifications/read",
        serde_json::json!({ "through": 5 }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(harness.manager.events().revision(), before);
    assert!(
        harness
            .store
            .notification_marks(&["quiet".to_string()])
            .await
            .unwrap()
            .get("quiet")
            .is_none_or(|marks| *marks == (0, 0)),
        "nothing to mark leaves the session where it stood"
    );
}

/// Both notification routes share the per-session routes' refusals: an id
/// no host has reported is a 404 naming it, and a body without `through`
/// is a 422 rather than an implicit mark of 0 that answered success.
#[farhelm_testtrace::test]
async fn notification_routes_refuse_unknown_ids_and_missing_marks() {
    let harness = rest_harness::helm_listing(vec![session_with_notifications("sess-1", 1)]).await;
    for route in ["read", "cleared"] {
        let (status, body) = put_json(
            &harness,
            &format!("/api/sessions/sess-missing/notifications/{route}"),
            serde_json::json!({ "through": 1 }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::NOT_FOUND, "{route}");
        assert_eq!(body, serde_json::json!("no such session: sess-missing"));

        let (status, _) = put_json(
            &harness,
            &format!("/api/sessions/sess-1/notifications/{route}"),
            serde_json::json!({}),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "{route}"
        );
    }
}

/// Like the seen route, the notification routes are helm-local writes: a
/// session on an unreachable host stays listed with its last-known bell, so
/// reading or clearing it must still work, clamped against the cached row.
#[farhelm_testtrace::test]
async fn notification_marks_land_while_the_host_is_down() {
    let (builder, host) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@breaks",
            rest_harness::HostScript {
                identity: Some("identity-original".to_string()),
                sessions: vec![session_with_notifications("owned", 2)],
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
    let (status, _) = put_json(
        &harness,
        "/api/sessions/owned/notifications/cleared",
        serde_json::json!({ "through": 9 }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert!(harness.manager.events().revision() > before);
    assert_eq!(
        harness
            .store
            .notification_marks(&["owned".to_string()])
            .await
            .unwrap()
            .get("owned"),
        Some(&(2, 2)),
        "the mark lands, clamped to the newest entry the cached row holds"
    );
}

/// The single-session detail route (`GET /api/sessions/{id}`) carries
/// `seen_activity_at` on a LIVE reply, matching the merged listing's own
/// field (`aggregate::row_of`) — a direct fetch (the recovery path
/// `list::ListView`'s remembered-selection resolver uses) must see the
/// same seen-state a listing read would have shown.
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
        let ControlMsg::DeleteSession {
            req_id, session_id, ..
        } = request
        else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        writer
            .write_control(&ControlMsg::SessionDeleted {
                req_id,
                archived: false,
                notice: None,
            })
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

/// A replace whose request is dropped after its create reached the host still
/// deletes the source.
///
/// Why it matters: axum drops a handler's future when the client goes away (a
/// reload, a closed tab, the client's own request timeout on a slow create),
/// and a replace is a create followed by a delete. Run on the request's task,
/// a drop between the two left the new session created and the source never
/// deleted, usually with no error anywhere (SPEC_impl.md "Who owns an
/// accepted action"). Specified: once the create has been sent, dropping the
/// request loses only the reply; the delete of the source still reaches the
/// host.
#[farhelm_testtrace::test]
async fn a_replace_dropped_after_its_create_still_deletes_the_source() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame, SessionInfo};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let (harness, local) = spliced_replace_harness(
        client_side,
        vec![rest_harness::session("sess-1", 1_700_000_000)],
    )
    .await;
    let fleet = harness.fleet.clone();
    let (create_seen_tx, create_seen_rx) = tokio::sync::oneshot::channel();
    let (answer_tx, answer_rx) = tokio::sync::oneshot::channel::<()>();
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
        let _ = create_seen_tx.send(());
        // Answered only once the request that asked for it is gone.
        answer_rx.await.unwrap();
        let created = SessionInfo {
            id: "sess-2".into(),
            ..rest_harness::session("sess-1", 1_700_000_500)
        };
        // Kept in step with the reply, as in the other replace tests (see
        // `spliced_replace_harness`).
        fleet.edit(local, |script| script.sessions.push(created.clone()));
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: created,
            }))
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession {
            req_id, session_id, ..
        } = request
        else {
            panic!("expected DeleteSession, got {request:?}");
        };
        fleet.edit(local, |script| script.sessions.retain(|s| s.id != "sess-1"));
        writer
            .write_control(&ControlMsg::SessionDeleted {
                req_id,
                archived: false,
                notice: None,
            })
            .await
            .unwrap();
        session_id
    });

    harness.await_refreshed(local).await;
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions/sess-1/replace")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from("{}"))
        .unwrap();
    let call = tokio::spawn(tower::ServiceExt::oneshot(harness.router(), request));
    tokio::time::timeout(std::time::Duration::from_secs(10), create_seen_rx)
        .await
        .expect("test premise: the replace must send its create")
        .expect("test premise: the fake supervisor must still be running");
    // The client goes away, which is axum dropping the handler's future.
    call.abort();
    let dropped = call.await;
    assert!(
        dropped.as_ref().is_err_and(|join| join.is_cancelled()),
        "test premise: the request must still be in flight when it is dropped, got {dropped:?}"
    );
    answer_tx.send(()).unwrap();

    let deleted = tokio::time::timeout(std::time::Duration::from_secs(10), peer)
        .await
        .expect("the source's delete must still be sent after the request is dropped")
        .unwrap();
    assert_eq!(deleted, "sess-1");
}

/// Spec: a plain Replace (no `with` body) of a session the host lists as a
/// structured yolo launch with workspace trust records no remembered
/// defaults and no recent setup.
///
/// Why: a plain Replace copies the source row's settings, and that row is
/// what the owning host listed, which a remote host controls. Recording
/// them as the user's choice let a compromised host set the helm-wide
/// defaults to yolo with trust just by listing such a row and waiting for
/// the user to press Replace. Only settings the user chose in the GUI (a
/// "replace with" body, the composer) may do that.
#[farhelm_testtrace::test]
async fn a_plain_replace_records_no_launch_choices_from_the_listed_row() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame, LaunchPermission, LaunchSelection};

    let yolo = LaunchSelection {
        harness: farhelm_proto::LaunchHarness::Codex,
        model: None,
        effort: None,
        permissions: Some(LaunchPermission::Yolo),
        workspace_trust: Some(true),
    };
    let mut source = rest_harness::session("yolo-src", 1_700_000_000);
    source.launch = crate::launches::test_agent_launch(yolo.clone());
    source.invocation = "codex --dangerously-bypass-approvals-and-sandbox".to_string();
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let (harness, local) = spliced_replace_harness(client_side, vec![source]).await;
    // YOLO launch on the fixture's ask-first host; the guard
    // is not what this test is about (see `yolo_guard`'s own tests).
    allow_yolo_here(&harness.store).await;
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
        let mut created = rest_harness::session("yolo-new", 1_700_000_500);
        created.launch = crate::launches::test_agent_launch(yolo);
        fleet.edit(local, |script| script.sessions.push(created.clone()));
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: created,
            }))
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession { req_id, .. } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        fleet.edit(local, |script| {
            script.sessions.retain(|s| s.id != "yolo-src")
        });
        writer
            .write_control(&ControlMsg::SessionDeleted {
                req_id,
                archived: false,
                notice: None,
            })
            .await
            .unwrap();
    });

    harness.await_refreshed(local).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions/yolo-src/replace",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    peer.await.unwrap();
    let reply: serde_json::Value = serde_json::from_str(&body).expect("a JSON reply");
    assert_eq!(
        reply.get("delete_notice"),
        None,
        "a delete without a notice adds no field: {reply}"
    );

    let (_, preferences) = get_json(&harness, "/api/preferences").await;
    assert_eq!(
        preferences.get("remembered_permissions"),
        None,
        "a plain Replace must not remember the listed row's permissions: {preferences}"
    );
    assert_ne!(
        preferences.get("remembered_workspace_trust"),
        Some(&serde_json::json!(true)),
        "a plain Replace must not remember the listed row's workspace trust: {preferences}"
    );
    assert!(
        harness
            .store
            .launch_history(local, "local-identity")
            .await
            .unwrap()
            .is_empty(),
        "a plain Replace must not add the listed row's launch to recent setups"
    );
}

/// Spec: when the source's Delete completes with a notice, the Replace reply
/// carries it as `delete_notice` beside the new session's own fields, and a
/// Delete with no notice adds no such field.
///
/// Why: a Replace whose new session uses a different folder (an override or
/// a fresh checkout) can release the source's last checkout reference, and
/// a checkout that could not be archived must never be released silently
/// (SPEC.md "Managed checkouts"). The reply stays a session object so a
/// client that ignores the field is unaffected.
#[farhelm_testtrace::test]
async fn a_replace_reply_carries_the_source_deletes_notice() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame};

    let source = rest_harness::session("noted-src", 1_700_000_000);
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
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
        let ControlMsg::CreateSession { req_id, .. } = request else {
            panic!("expected CreateSession, got {request:?}");
        };
        let created = rest_harness::session("noted-new", 1_700_000_500);
        fleet.edit(local, |script| script.sessions.push(created.clone()));
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: created,
            }))
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession { req_id, .. } = request else {
            panic!("expected DeleteSession, got {request:?}");
        };
        fleet.edit(local, |script| {
            script.sessions.retain(|s| s.id != "noted-src")
        });
        writer
            .write_control(&ControlMsg::SessionDeleted {
                req_id,
                archived: false,
                notice: Some("The managed checkout at /work/bar was not archived".to_string()),
            })
            .await
            .unwrap();
    });

    harness.await_refreshed(local).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions/noted-src/replace",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    peer.await.unwrap();
    let body: serde_json::Value = serde_json::from_str(&body).expect("a JSON reply");
    assert_eq!(
        body["id"], "noted-new",
        "the reply is still the new session"
    );
    assert_eq!(
        body["delete_notice"], "The managed checkout at /work/bar was not archived",
        "the source delete's notice reaches the caller verbatim"
    );
}

/// Spec: the source delete that ends a successful Replace carries the
/// precondition the request body asked for, as the matching
/// `farhelm_proto::DeleteGuard`: `only_if_nothing_alive`, `only_if_agent_ended`,
/// or neither.
///
/// Why: a Replace prompt that warned only about open tabs, or one that said
/// nothing was alive, authorizes no more than that (SPEC.md "Lifecycle
/// operations"). The supervisor can refuse a source that is running again
/// only if the helm forwards the level instead of collapsing it.
#[farhelm_testtrace::test]
async fn a_replace_forwards_its_source_delete_precondition() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, DeleteGuard, Frame};

    for (body, expected) in [
        (
            serde_json::json!({ "only_if_nothing_alive": true }),
            DeleteGuard::NothingAlive,
        ),
        (
            serde_json::json!({ "only_if_agent_ended": true }),
            DeleteGuard::AgentEnded,
        ),
        (serde_json::json!({}), DeleteGuard::Unconditional),
    ] {
        let source = rest_harness::session("guarded-src", 1_700_000_000);
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
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
            let ControlMsg::CreateSession { req_id, .. } = request else {
                panic!("expected CreateSession, got {request:?}");
            };
            let created = rest_harness::session("guarded-new", 1_700_000_500);
            fleet.edit(local, |script| script.sessions.push(created.clone()));
            writer
                .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                    req_id,
                    session: created,
                }))
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::DeleteSession {
                req_id,
                only_if_nothing_alive,
                only_if_agent_ended,
                ..
            } = request
            else {
                panic!("expected DeleteSession, got {request:?}");
            };
            fleet.edit(local, |script| {
                script.sessions.retain(|s| s.id != "guarded-src")
            });
            writer
                .write_control(&ControlMsg::SessionDeleted {
                    req_id,
                    archived: false,
                    notice: None,
                })
                .await
                .unwrap();
            DeleteGuard::from_flags(only_if_nothing_alive, only_if_agent_ended)
        });

        harness.await_refreshed(local).await;
        let (status, reply) =
            post_text(&harness, "/api/sessions/guarded-src/replace", body.clone()).await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}: {reply}");
        assert_eq!(peer.await.unwrap(), expected, "{body}");
    }
}

/// The simplest replace: a live, raw-invocation source. SPEC.md's contract
/// is a NEW id carrying the source's cwd, title, and invocation, with the
/// old id gone from the list at once — this pins that promise against the
/// real handler and a scripted supervisor. The removed source's read/unread
/// row goes too, as it does for a plain delete (SPEC_impl.md's
/// `session_seen` paragraph): replace used to leave it behind.
#[farhelm_testtrace::test]
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
        // the source; no overrides, no idempotency key (the
        // request body below sends none).
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession {
            req_id,
            parent: None,
            cwd,
            launch,
            title,
            intent_key,
            ..
        } = request
        else {
            panic!("expected CreateSession, got {request:?}");
        };
        assert_eq!(cwd, "/sess-1");
        assert_eq!(
            launch,
            Some(farhelm_proto::SessionLaunch::plain_command("agent"))
        );
        assert_eq!(title, Some("sess-1".to_string()));
        assert_eq!(intent_key, None);
        let created = SessionInfo {
            agent_kind: farhelm_proto::AgentKind::Generic,
            parent: None,
            id: "sess-2".into(),
            title: "sess-1".into(),
            created_at: 1_700_000_500,
            last_activity_at: 1_700_000_500,
            last_work_started_at: 0,
            creation_seq: None,
            cwd: "/sess-1".into(),
            canonical_cwd: None,
            invocation: "agent".into(),
            launch: farhelm_proto::SessionLaunch::plain_command("agent"),
            status: farhelm_proto::SessionStatus::Unknown,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            github_repo: None,
            working_copy: None,
            notifications: Vec::new(),
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
        let ControlMsg::DeleteSession {
            req_id, session_id, ..
        } = request
        else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        // Same ordering rule as the create half, mirrored for the delete:
        // the fixture drops the source before the reply that tells the
        // client it is gone.
        fleet.edit(local, |script| script.sessions.retain(|s| s.id != "sess-1"));
        writer
            .write_control(&ControlMsg::SessionDeleted {
                req_id,
                archived: false,
                notice: None,
            })
            .await
            .unwrap();
    });

    harness.await_refreshed(local).await;
    harness
        .store
        .mark_seen("sess-1", 1_700_000_000)
        .await
        .unwrap();
    assert_eq!(
        harness
            .store
            .seen_activity(&["sess-1".to_string()])
            .await
            .unwrap()
            .get("sess-1"),
        Some(&1_700_000_000),
        "fixture premise: the source's seen-state row exists before the replace"
    );
    harness
        .store
        .raise_notification_marks("sess-1", 2, crate::store::NotificationMark::Cleared)
        .await
        .unwrap();
    assert_eq!(
        harness
            .store
            .notification_marks(&["sess-1".to_string()])
            .await
            .unwrap()
            .get("sess-1"),
        Some(&(2, 2)),
        "fixture premise: the source's notification marks exist before the replace"
    );
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert!(
        harness
            .store
            .seen_activity(&["sess-1".to_string()])
            .await
            .unwrap()
            .is_empty(),
        "the replaced source's seen-state row must not survive it"
    );
    assert!(
        harness
            .store
            .notification_marks(&["sess-1".to_string()])
            .await
            .unwrap()
            .is_empty(),
        "the replaced source's notification marks must not survive it"
    );
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
/// source's own creation used (same cwd, title, raw invocation,
/// default dimensions, no parent), so a caller that reuses the source's own
/// creation key hits a legitimate reservation REPLAY at the target, which
/// answers with the source row rather than a new one. Accepting that reply
/// would send `DeleteSession` for the id just "created", forget it, and
/// report success describing the session it had just destroyed — violating
/// every promise replace makes. This pins the refusal instead.
#[farhelm_testtrace::test]
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
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    parent: None,
                    id: "sess-1".into(),
                    title: "sess-1".into(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: None,
                    cwd: "/sess-1".into(),
                    canonical_cwd: None,
                    invocation: "agent".into(),
                    launch: farhelm_proto::SessionLaunch::plain_command("agent"),
                    status: farhelm_proto::SessionStatus::Running,
                    annotation: None,
                    restart_offer: farhelm_proto::RestartOffer::default(),
                    tabs: Vec::new(),
                    github_repo: None,
                    working_copy: None,
                    notifications: Vec::new(),
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

/// Spec: plain Replace of a legacy session (one created before launch
/// kinds) is refused with a remedy naming Replace with, and nothing is
/// created or deleted.
///
/// Why: a legacy launch carries no YOLO answer and was never classified,
/// so copying it as a new session would launch something nobody described
/// under the new rules (SPEC.md's launch-kinds upgrade). The refusal must
/// come before the create, or the user would be left with two sessions.
#[farhelm_testtrace::test]
async fn plain_replace_of_a_legacy_session_is_refused_with_its_remedy() {
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let source = farhelm_proto::SessionInfo {
        invocation: "claude --model opus".to_string(),
        launch: farhelm_proto::SessionLaunch::Legacy {
            invocation: "claude --model opus".to_string(),
            agent_kind: farhelm_proto::AgentKind::Claude,
            resume_template: None,
        },
        ..rest_harness::session("sess-1", 1_700_000_000)
    };
    let (harness, local) = spliced_replace_harness(client_side, vec![source]).await;
    let peer = tokio::spawn(silent_supervisor(peer_side));
    harness.await_refreshed(local).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body.contains("before launch kinds") && body.contains("Replace with"),
        "{body}"
    );
    drop(harness);
    peer.await.unwrap();
}

/// Plain Replace of an agent launch copies its stored launch exactly —
/// selection, composed start and resume commands — rather than composing
/// anew through today's catalog.
///
/// Why: SPEC.md has plain Replace copy the launch as stored; recompiling
/// could change an older choice (a catalog change, a retired flag) behind
/// a confirmation that promised the same settings.
#[farhelm_testtrace::test]
async fn replace_of_an_agent_launch_copies_its_stored_launch() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{
        ControlMsg, Frame, LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection,
        SessionInfo,
    };

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let selection = LaunchSelection {
        harness: LaunchHarness::Claude,
        model: Some("claude-opus-4-6".to_string()),
        effort: Some(LaunchEffort::High),
        permissions: Some(LaunchPermission::Yolo),
        workspace_trust: None,
    };
    // A launch an older catalog composed: the start command carries a flag
    // today's compiler would not write, which a recompile would lose.
    let stored = farhelm_proto::SessionLaunch::Agent {
        selection: selection.clone(),
        start: vec![
            "claude".to_string(),
            "--model".to_string(),
            "claude-opus-4-6".to_string(),
            "--dangerously-skip-permissions".to_string(),
            "--older-flag".to_string(),
            "{farhelm_args}".to_string(),
        ],
        resume: Some(vec![
            "claude".to_string(),
            "--resume".to_string(),
            "{conversation}".to_string(),
            "{farhelm_args}".to_string(),
        ]),
    };
    let source = SessionInfo {
        invocation: stored.display_command(),
        launch: stored.clone(),
        ..rest_harness::session("sess-1", 1_700_000_000)
    };
    let (harness, local) = spliced_replace_harness(client_side, vec![source]).await;
    // YOLO launch on the fixture's ask-first host; the guard
    // is not what this test is about (see `yolo_guard`'s own tests).
    allow_yolo_here(&harness.store).await;
    let fleet = harness.fleet.clone();
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();

        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession { req_id, launch, .. } = request else {
            panic!("expected CreateSession, got {request:?}");
        };
        assert_eq!(launch.as_ref(), Some(&stored));

        let created = SessionInfo {
            id: "sess-2".to_string(),
            launch: stored,
            ..rest_harness::session("sess-2", 1_700_000_500)
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
        let ControlMsg::DeleteSession {
            req_id, session_id, ..
        } = request
        else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        fleet.edit(local, |script| script.sessions.retain(|s| s.id != "sess-1"));
        writer
            .write_control(&ControlMsg::SessionDeleted {
                req_id,
                archived: false,
                notice: None,
            })
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
    assert_eq!(
        serde_json::from_str::<SessionInfo>(&body).unwrap().id,
        "sess-2"
    );
    peer.await.unwrap();
}

/// If the create fails, the source is untouched: the handler returns before
/// ever sending a delete, and the row stays listed exactly as it was — the
/// "nothing was lost" half of `do_replace_session`'s asymmetric failure
/// rule (see its own doc).
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
            agent_kind: farhelm_proto::AgentKind::Generic,
            parent: None,
            id: "sess-2".into(),
            title: "sess-1".into(),
            created_at: 1_700_000_500,
            last_activity_at: 1_700_000_500,
            last_work_started_at: 0,
            creation_seq: None,
            cwd: "/sess-1".into(),
            canonical_cwd: None,
            invocation: "agent".into(),
            launch: farhelm_proto::SessionLaunch::plain_command("agent"),
            status: farhelm_proto::SessionStatus::Unknown,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            github_repo: None,
            working_copy: None,
            notifications: Vec::new(),
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
        let ControlMsg::DeleteSession {
            req_id, session_id, ..
        } = request
        else {
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
#[farhelm_testtrace::test]
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
            agent_kind: farhelm_proto::AgentKind::Generic,
            parent: None,
            id: "sess-2".into(),
            title: "sess-1".into(),
            created_at: 1_700_000_500,
            last_activity_at: 1_700_000_500,
            last_work_started_at: 0,
            creation_seq: None,
            cwd: "/sess-1".into(),
            canonical_cwd: None,
            invocation: "agent".into(),
            launch: farhelm_proto::SessionLaunch::plain_command("agent"),
            status: farhelm_proto::SessionStatus::Unknown,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            github_repo: None,
            working_copy: None,
            notifications: Vec::new(),
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
            agent_kind: farhelm_proto::AgentKind::Generic,
            parent: None,
            id: "sess-2".into(),
            title: "sess-1".into(),
            created_at: 1_700_000_500,
            last_activity_at: 1_700_000_500,
            last_work_started_at: 0,
            creation_seq: None,
            cwd: "/sess-1".into(),
            canonical_cwd: None,
            invocation: "agent".into(),
            launch: farhelm_proto::SessionLaunch::plain_command("agent"),
            status: farhelm_proto::SessionStatus::Unknown,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            github_repo: None,
            working_copy: None,
            notifications: Vec::new(),
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
        let ControlMsg::DeleteSession {
            req_id, session_id, ..
        } = request
        else {
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
        let ControlMsg::DeleteSession {
            req_id, session_id, ..
        } = request
        else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        // This delete SUCCEEDS, so the fixture drops the source before the
        // reply that tells the client it is gone — same rule as every other
        // successful delete in this file.
        fleet.edit(local, |script| script.sessions.retain(|s| s.id != "sess-1"));
        writer
            .write_control(&ControlMsg::SessionDeleted {
                req_id,
                archived: false,
                notice: None,
            })
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

// ----- "replace with": `ReplaceReq::with` overriding the create half -----
//
// Everything above this point pins UNQUALIFIED replace (`with` absent),
// which the tests below leave completely alone. These pin the override
// body: the create half launches `with`'s fields instead of the source's
// own, while every failure shape, the same-host rule, and the delete half
// stay exactly as `do_replace_session`'s doc describes for either form.

/// The plainest "replace with": a raw override of cwd, invocation, and
/// title. SPEC.md's replace-with bullet promises every field can be edited
/// before launching, and this pins that the OVERRIDE — not the source's own
/// cwd/title/invocation — is what reaches the supervisor, while the source
/// is still removed exactly as an unqualified replace removes it.
#[farhelm_testtrace::test]
async fn a_replace_with_override_of_invocation_title_and_cwd_creates_it_and_removes_the_source() {
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

        // The create half must carry the OVERRIDE's cwd, invocation, and
        // title — never the source's own — with no idempotency key (the
        // request body below sends none).
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession {
            req_id,
            parent: None,
            cwd,
            launch,
            title,
            intent_key,
            ..
        } = request
        else {
            panic!("expected CreateSession, got {request:?}");
        };
        assert_eq!(cwd, "/replaced-with");
        assert_eq!(
            launch,
            Some(farhelm_proto::SessionLaunch::plain_command(
                "new-agent --flag"
            ))
        );
        assert_eq!(title, Some("replaced-with-title".to_string()));
        assert_eq!(intent_key, None);
        let created = SessionInfo {
            agent_kind: farhelm_proto::AgentKind::Generic,
            parent: None,
            id: "sess-2".into(),
            title: "replaced-with-title".into(),
            created_at: 1_700_000_500,
            last_activity_at: 1_700_000_500,
            last_work_started_at: 0,
            creation_seq: None,
            cwd: "/replaced-with".into(),
            canonical_cwd: None,
            invocation: "new-agent --flag".into(),
            launch: farhelm_proto::SessionLaunch::plain_command("new-agent --flag"),
            status: farhelm_proto::SessionStatus::Unknown,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            github_repo: None,
            working_copy: None,
            notifications: Vec::new(),
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
        let ControlMsg::DeleteSession {
            req_id, session_id, ..
        } = request
        else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        fleet.edit(local, |script| script.sessions.retain(|s| s.id != "sess-1"));
        writer
            .write_control(&ControlMsg::SessionDeleted {
                req_id,
                archived: false,
                notice: None,
            })
            .await
            .unwrap();
    });

    harness.await_refreshed(local).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({
            "with": {
                "cwd": "/replaced-with",
                "command": {"command": "new-agent --flag", "yolo": false},
                "title": "replaced-with-title",
            }
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let session: farhelm_proto::SessionInfo = serde_json::from_str(&body).unwrap();
    assert_eq!(session.id, "sess-2");
    assert_eq!(session.cwd, "/replaced-with");
    assert_eq!(session.invocation, "new-agent --flag");
    assert_eq!(session.title, "replaced-with-title");

    harness.refresh_to_completion(local).await;
    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        row_ids(&value),
        vec!["sess-2"],
        "the source must be gone once the override's create and the delete both succeed"
    );

    peer.await.unwrap();
}
/// "Replace with" keeps the source's own host: SPEC.md draws that line
/// explicitly (clone is the way to a different host), and this pins the
/// helm-side refusal that enforces it — a 409 before either the create or
/// the delete runs, with both hosts' listings left exactly as they were.
#[farhelm_testtrace::test]
async fn a_replace_with_body_naming_another_host_is_refused_before_anything_is_created() {
    let (builder, other) = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("identity-local".to_string()),
            sessions: vec![rest_harness::session("sess-1", 100)],
            ..rest_harness::HostScript::default()
        })
        .await
        .ssh(
            "user@other",
            rest_harness::HostScript {
                identity: Some("identity-other".to_string()),
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    let local = rest_harness::local_id(&harness.store).await;
    harness.await_refreshed(local).await;
    harness.await_refreshed(other).await;

    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({
            "with": { "cwd": "/sess-1", "command": {"command": "agent", "yolo": false}, "host": other }
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{body}");
    assert!(
        body.contains("clone"),
        "must point at clone as the way to start a session elsewhere: {body}"
    );

    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        row_ids(&value),
        vec!["sess-1"],
        "nothing may be created or removed while replace with names another host"
    );
}

/// A "replace with" override whose CREATE fails leaves the source
/// untouched, exactly like an unqualified replace's own create-refusal
/// case (`a_create_refusal_leaves_the_source_listed`) — the override
/// changes what would have been launched, never the promise that a failed
/// create loses nothing.
#[farhelm_testtrace::test]
async fn a_replace_with_override_whose_create_fails_leaves_the_source_untouched() {
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
        let ControlMsg::CreateSession { req_id, cwd, .. } = request else {
            panic!("expected CreateSession, got {request:?}");
        };
        assert_eq!(cwd, "/does-not-exist");
        writer
            .write_frame(&Frame::control(&ControlMsg::Error {
                req_id,
                message: "working directory does not exist: /does-not-exist".into(),
                kind: ErrorKind::InvalidRequest,
            }))
            .await
            .unwrap();
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({ "with": { "cwd": "/does-not-exist", "command": {"command": "agent", "yolo": false} } }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("does not exist"), "{body}");

    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(row_ids(&value), vec!["sess-1"]);

    peer.await.unwrap();
}

/// A "replace with" override whose DELETE fails after a successful create
/// still answers with the message naming both ids — the same asymmetric
/// failure rule an unqualified replace's own delete-failure gets
/// (`a_delete_failure_after_a_successful_create_reports_both_ids_and_leaves_both_rows`),
/// unaffected by which fields the create half actually launched.
#[farhelm_testtrace::test]
async fn a_replace_with_override_whose_delete_fails_after_a_successful_create_reports_both_ids() {
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
        let ControlMsg::CreateSession {
            req_id,
            cwd,
            launch,
            ..
        } = request
        else {
            panic!("expected CreateSession, got {request:?}");
        };
        assert_eq!(cwd, "/override-delete-fails");
        assert_eq!(
            launch,
            Some(farhelm_proto::SessionLaunch::plain_command(
                "override-agent"
            ))
        );
        let created = SessionInfo {
            agent_kind: farhelm_proto::AgentKind::Generic,
            parent: None,
            id: "sess-2".into(),
            title: "sess-1".into(),
            created_at: 1_700_000_500,
            last_activity_at: 1_700_000_500,
            last_work_started_at: 0,
            creation_seq: None,
            cwd: "/override-delete-fails".into(),
            canonical_cwd: None,
            invocation: "override-agent".into(),
            launch: farhelm_proto::SessionLaunch::plain_command("override-agent"),
            status: farhelm_proto::SessionStatus::Unknown,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            github_repo: None,
            working_copy: None,
            notifications: Vec::new(),
        };
        // See `spliced_replace_harness`'s doc: fixture updated before the
        // reply, matching every other successful create in this file.
        fleet.edit(local, |script| script.sessions.push(created.clone()));
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: created,
            }))
            .await
            .unwrap();

        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteSession {
            req_id, session_id, ..
        } = request
        else {
            panic!("expected DeleteSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        // No fixture change: the supervisor REFUSES this delete, so
        // "sess-1" genuinely still exists on the far end too.
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
        serde_json::json!({
            "with": { "cwd": "/override-delete-fails", "command": {"command": "override-agent", "yolo": false} }
        }),
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

/// A "replace with" body whose OWN `intent_key` disagrees with the
/// top-level one is refused as a bad request — see `ReplaceReq::with`'s own
/// doc for why the wire must not silently pick a winner between two
/// candidate keys for what is supposed to be one intended create. Checked
/// before any routing happens, so it 400s even for a session id this fleet
/// has never heard of and a host it never had to contact.
#[farhelm_testtrace::test]
async fn a_replace_with_mismatched_intent_key_is_refused_as_a_bad_request() {
    let harness = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("identity-local".to_string()),
            ..rest_harness::HostScript::default()
        })
        .await
        .start()
        .await;

    let (status, body) = post_text(
        &harness,
        "/api/sessions/does-not-exist/replace",
        serde_json::json!({
            "intent_key": "top-level-key",
            "with": { "cwd": "/x", "command": {"command": "agent", "yolo": false}, "intent_key": "a-different-key" }
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("key"), "{body}");
}

/// A "replace with" body with a create-shape problem — here naming BOTH
/// `invocation` and `launch` — is a 400 with the SAME precedence an
/// ordinary create gives it: before routing, before any supervisor round
/// trip, and therefore even for a session id this fleet has never heard
/// of. `ReplaceReq::with`'s doc promises the override is resolved "exactly
/// as an ordinary create's body is", and this pins that the promise covers
/// WHEN the refusal happens, not only what it says — a version that
/// resolved the mode after the live source read answered such a body with
/// a 404 for the missing session instead.
#[farhelm_testtrace::test]
async fn a_replace_with_body_shape_problem_is_refused_before_routing() {
    let harness = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("identity-local".to_string()),
            ..rest_harness::HostScript::default()
        })
        .await
        .start()
        .await;

    let (status, body) = post_text(
        &harness,
        "/api/sessions/does-not-exist/replace",
        serde_json::json!({
            "with": {
                "cwd": "/x",
                "command": {"command": "agent", "yolo": false},
                "launch": { "harness": "codex", "model": null, "effort": null, "permissions": null }
            }
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
}

/// A "replace with" body whose `expected_incarnation` names a connection
/// the source's host is no longer on is a 409 carrying
/// the precondition header (`farhelm_proto::http::PRECONDITION_HEADER`) before the live read, the
/// create, or the delete — the same precondition an ordinary create's
/// `expected_incarnation` gets, applied to the override's own claim. The
/// listing is untouched afterwards.
#[farhelm_testtrace::test]
async fn a_replace_with_stale_incarnation_is_refused_before_anything_is_created() {
    let harness = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("identity-local".to_string()),
            sessions: vec![rest_harness::session("sess-1", 100)],
            ..rest_harness::HostScript::default()
        })
        .await
        .start()
        .await;
    let local = rest_harness::local_id(&harness.store).await;
    harness.await_refreshed(local).await;
    let current = harness
        .manager
        .status(local)
        .expect("the local host has an actor")
        .incarnation;

    let (status, precondition_headers, body) = post_text_headers(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({
            "with": { "cwd": "/sess-1", "command": {"command": "agent", "yolo": false}, "expected_incarnation": current - 1 }
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{body}");
    assert!(
        is_stale_precondition(&precondition_headers),
        "a client must be able to tell this from a host that is merely busy: {body}"
    );

    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        row_ids(&value),
        vec!["sess-1"],
        "nothing may be created or removed on a stale incarnation claim"
    );
}

/// The idempotency-replay veto holds under `with` too. A "replace with"
/// whose override reproduces the source's own fingerprint, sent with the
/// source's own creation key, hits the same reservation REPLAY a plain
/// replace can (see `a_create_reply_that_replays_the_source_id_is_refused_before_any_delete`):
/// the target answers with the SOURCE row, and accepting it would delete
/// the very session just "created". The veto is shared code, and this pins
/// that a future "skip the veto when the caller supplied its own fields"
/// shortcut would fail rather than pass silently.
#[farhelm_testtrace::test]
async fn a_replace_with_create_reply_that_replays_the_source_id_is_refused_before_any_delete() {
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
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: SessionInfo {
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    parent: None,
                    id: "sess-1".into(),
                    title: "sess-1".into(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: None,
                    cwd: "/sess-1".into(),
                    canonical_cwd: None,
                    invocation: "agent".into(),
                    launch: farhelm_proto::SessionLaunch::plain_command("agent"),
                    status: farhelm_proto::SessionStatus::Running,
                    annotation: None,
                    restart_offer: farhelm_proto::RestartOffer::default(),
                    tabs: Vec::new(),
                    github_repo: None,
                    working_copy: None,
                    notifications: Vec::new(),
                },
            }))
            .await
            .unwrap();
        // No `DeleteSession` follows: the veto refuses before any
        // bookkeeping, so this peer reads nothing more.
    });

    let harness = rest_harness::spliced_helm(client_side).await;
    let (status, body) = post_text(
        &harness,
        "/api/sessions/sess-1/replace",
        serde_json::json!({
            "intent_key": "reused-key",
            "with": { "cwd": "/sess-1", "command": {"command": "agent", "yolo": false}, "title": "sess-1" }
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{body}");
    assert!(
        body.contains("no replacement was made"),
        "the refusal must say the replay made nothing: {body}"
    );

    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        row_ids(&value),
        vec!["sess-1"],
        "a rejected replay must leave the source exactly as it was — no delete was ever sent"
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
/// Observation lasts through the HTTP response, then the test explicitly ends
/// and joins the peer. The harness retains the connection, so EOF cannot mark
/// completion; an elapsed window could end during setup before the request.
/// The original two-second silence window starts after that signal, keeping
/// the peer alive for frames still queued in the harness's asynchronous relay.
/// Neither setup time nor the HTTP request can consume that window.
#[farhelm_testtrace::test]
async fn foreign_origin_is_refused_on_the_stop_route() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let (finish, finished) = tokio::sync::oneshot::channel();
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let incoming = reader.read_frame();
        tokio::pin!(incoming);
        tokio::select! {
            leaked = &mut incoming => {
                panic!("foreign-origin stop must not reach the supervisor: {leaked:?}");
            }
            result = finished => result.expect("the response must finish observation"),
        }
        // Continue the same read so a partially relayed frame is not discarded
        // at the response boundary. Only silence through this window passes.
        let leaked = tokio::time::timeout(Duration::from_secs(2), &mut incoming).await;
        assert!(
            leaked.is_err(),
            "foreign-origin stop reached the supervisor: {leaked:?}"
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
    finish.send(()).expect("the peer must still be observing");
    peer.await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
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
#[farhelm_testtrace::test]
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
            serde_json::json!({"cwd": "/nope", "command": {"command": "some-agent", "yolo": false}}).to_string(),
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
/// the body's `stop_if_running` reaches the supervisor unaltered, and the
/// success body is the session's own recomputed `SessionInfo` — including
/// the freshly computed `restart_offer` a caller re-renders its row from
/// without listing again.
///
/// The consent is asserted at the WIRE, not merely accepted by the
/// handler: it is the user's permission to kill a running agent, so a
/// route that dropped or defaulted it would be a silent safety regression
/// rather than a visible failure.
#[farhelm_testtrace::test]
async fn restart_session_passes_consent_through_and_returns_the_session() {
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
            stop_if_running,
            ..
        } = request
        else {
            panic!("expected RestartSession, got {request:?}");
        };
        assert_eq!(session_id, "sess-1");
        assert!(
            stop_if_running,
            "the user's consent to stop a live agent must reach the supervisor"
        );
        writer
            .write_control(&ControlMsg::SessionRestarted {
                req_id,
                session: farhelm_proto::SessionInfo {
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    parent: None,
                    id: "sess-1".into(),
                    title: "t".into(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: None,
                    cwd: "/some/dir".into(),
                    canonical_cwd: None,
                    invocation: "some-agent".into(),
                    launch: farhelm_proto::SessionLaunch::plain_command("some-agent"),
                    status: farhelm_proto::SessionStatus::Unknown,
                    annotation: None,
                    restart_offer: farhelm_proto::RestartOffer::Resume,
                    tabs: Vec::new(),
                    github_repo: None,
                    working_copy: None,
                    notifications: Vec::new(),
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
            serde_json::json!({ "stop_if_running": true }).to_string(),
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

/// A structured restart-with request is compiled by the helm and forwards
/// every compiled launch field to the supervisor wire message.
#[farhelm_testtrace::test]
async fn restart_with_compiles_and_forwards_structured_launch_bundle() {
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
        let ControlMsg::RestartSession { req_id, with, .. } = request else {
            panic!("expected restart request");
        };
        let with = with.expect("restart with carries a launch");
        assert_eq!(
            with.display_command(),
            "claude --dangerously-skip-permissions"
        );
        assert_eq!(
            with.agent_selection().expect("an agent launch").permissions,
            Some(farhelm_proto::LaunchPermission::Yolo)
        );
        assert_eq!(
            with.resume_argv().unwrap(),
            Some(vec![
                "claude".to_string(),
                "--dangerously-skip-permissions".to_string(),
                "--resume".to_string(),
                "{conversation}".to_string(),
                "{farhelm_args}".to_string(),
            ]),
            "the helm composes the resume command from the same choices"
        );
        writer
            .write_control(&ControlMsg::SessionRestarted {
                req_id,
                session: rest_harness::session("sess-1", 1),
            })
            .await
            .unwrap();
    });
    let harness = rest_harness::spliced_helm(client_side).await;
    // YOLO launch on the fixture's ask-first host; the guard
    // is not what this test is about (see `yolo_guard`'s own tests).
    allow_yolo_here(&harness.store).await;
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions/sess-1/restart")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::json!({
            "with": {"harness":"claude","model":null,"effort":null,"permissions":"yolo","workspace_trust":null}
        }).to_string())).unwrap();
    assert_eq!(
        harness.router().oneshot(request).await.unwrap().status(),
        axum::http::StatusCode::OK
    );
    peer.await.unwrap();
}

/// POST a JSON body to `uri` through the router, returning the status, the
/// headers and the body text.
async fn post_json(
    harness: &rest_harness::Harness,
    uri: &str,
    body: serde_json::Value,
) -> (axum::http::StatusCode, axum::http::HeaderMap, String) {
    post_text_headers(harness, uri, body).await
}

/// Spec: Restart with of a command launch sends the edited command launch
/// (`with_command`) to the supervisor whole, refuses a body naming both an
/// agent selection and a command, and holds an asserted-YOLO command to
/// the host's YOLO question like any other YOLO launch.
///
/// Why: this is the API the changelog advertises for changing a command
/// launch's command, resume command and YOLO answer. A body naming both
/// shapes has no single meaning, and an asserted-YOLO restart that skipped
/// the guard would be the one YOLO launch a host that asks never asked
/// about. Both refusals happen before anything reaches the supervisor.
#[farhelm_testtrace::test]
async fn restart_with_a_command_launch_forwards_it_and_refuses_bad_shapes() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

    let edited = farhelm_proto::CommandLaunch {
        command: "claude --model sonnet {farhelm_args}".to_string(),
        yolo: false,
        agent: Some(farhelm_proto::LaunchHarness::Claude),
        resume: Some("claude --model sonnet --resume {conversation} {farhelm_args}".to_string()),
    };
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let expected = edited.clone();
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::RestartSession { req_id, with, .. } = request else {
            panic!("expected restart request, got {request:?}");
        };
        assert_eq!(with, Some(farhelm_proto::SessionLaunch::Command(expected)));
        writer
            .write_control(&ControlMsg::SessionRestarted {
                req_id,
                session: rest_harness::session("sess-1", 1),
            })
            .await
            .unwrap();
    });
    let harness = rest_harness::spliced_helm(client_side).await;
    let (status, _, body) = post_json(
        &harness,
        "/api/sessions/sess-1/restart",
        serde_json::json!({ "with_command": edited }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    peer.await.unwrap();

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(silent_supervisor(peer_side));
    let harness = rest_harness::spliced_helm(client_side).await;
    let (status, _, body) = post_json(
        &harness,
        "/api/sessions/sess-1/restart",
        serde_json::json!({
            "with": {"harness":"claude","model":null,"effort":null,"permissions":null,"workspace_trust":null},
            "with_command": edited,
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    let local = rest_harness::local_id(&harness.store).await;
    assert!(
        !host_yolo_without_asking(&harness.store, local).await,
        "premise: the local host starts asking before YOLO launches"
    );
    let (status, headers, body) = post_json(
        &harness,
        "/api/sessions/sess-1/restart",
        serde_json::json!({ "with_command": farhelm_proto::CommandLaunch { yolo: true, ..edited.clone() } }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{body}");
    assert_eq!(
        headers[farhelm_proto::http::YOLO_CONFIRMATION_HEADER],
        farhelm_proto::http::YOLO_CONFIRMATION_REQUIRED
    );
    drop(harness);
    peer.await.unwrap();
}

/// Spec: a create or replace-with body still carrying a field launch kinds
/// retired (`invocation`, `agent_kind`, `resume_template`) is refused with
/// a 400 naming that field and what replaced it, before anything is sent
/// to a supervisor.
///
/// Why: a client written before launch kinds would otherwise have its old
/// field silently ignored, launching something other than what it asked
/// for; naming the replacement lets its author fix it from the message.
#[farhelm_testtrace::test]
async fn bodies_naming_a_retired_launch_field_are_refused_by_name() {
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(silent_supervisor(peer_side));
    let harness = rest_harness::spliced_helm(client_side).await;
    let command = serde_json::json!({"command": "agent", "yolo": false});
    for (field, value, replacement) in [
        ("invocation", serde_json::json!("agent"), "command"),
        ("agent_kind", serde_json::json!("claude"), "command.agent"),
        (
            "resume_template",
            serde_json::json!(["agent", "{conversation}"]),
            "command.resume",
        ),
    ] {
        let mut create = serde_json::json!({"cwd": "/project", "command": command});
        create[field] = value.clone();
        let (status, _, body) = post_json(&harness, "/api/sessions", create).await;
        assert_eq!(
            status,
            axum::http::StatusCode::BAD_REQUEST,
            "{field}: {body}"
        );
        assert!(
            body.contains(&format!("{field} was replaced by {replacement}")),
            "{field}: {body}"
        );
        let mut with = serde_json::json!({"cwd": "/project", "command": command});
        with[field] = value;
        let (status, _, body) = post_json(
            &harness,
            "/api/sessions/sess-1/replace",
            serde_json::json!({ "with": with }),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::BAD_REQUEST,
            "{field}: {body}"
        );
        assert!(
            body.contains(&format!("{field} was replaced")),
            "{field}: {body}"
        );
    }
    drop(harness);
    peer.await.unwrap();
}

/// A restart-with selection the catalog refuses is the caller's mistake, so
/// the route must answer 400 with the catalog's reason and send nothing to
/// the supervisor. Grok refuses an explicit model, which makes the refusal
/// deterministic. Before this was classified, the untyped compile error
/// reached the browser as a 500 with no usable explanation.
#[farhelm_testtrace::test]
async fn restart_with_an_invalid_selection_is_a_400_and_never_reaches_the_supervisor() {
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
        // The next read ends only when the helm side closes: any frame here
        // would be a request the refused selection should never have sent.
        let next = reader.read_frame().await;
        assert!(
            !matches!(next, Ok(Some(_))),
            "a refused selection must not reach the supervisor"
        );
    });
    let harness = rest_harness::spliced_helm(client_side).await;
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions/sess-1/restart")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({
                "with": {"harness":"grok","model":"x-ai/grok-4.5","effort":null,"permissions":null,"workspace_trust":null}
            })
            .to_string(),
        ))
        .unwrap();
    let response = harness.router().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    drop(harness);
    peer.await.unwrap();
}

/// A stale-offer refusal must reach the browser as a 409 carrying the
/// supervisor's own prose — that message names the CURRENT offer, and
/// re-presenting it is the client's prescribed response (the wire
/// vocabulary's staleness contract). A route that flattened it to a
/// generic 500 would leave the UI with nothing to say.
#[farhelm_testtrace::test]
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
        .body(axum::body::Body::from(serde_json::json!({}).to_string()))
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
#[farhelm_testtrace::test]
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
            agent_kind: farhelm_proto::AgentKind::Generic,
            parent: None,
            id: "sess-1".into(),
            title: expected_title.clone(),
            created_at: 1_700_000_000,
            last_activity_at: 1_700_000_000,
            last_work_started_at: 0,
            creation_seq: None,
            cwd: "/distinctive/dir".into(),
            canonical_cwd: None,
            invocation: "distinctive-agent --flag".into(),
            launch: farhelm_proto::SessionLaunch::plain_command("distinctive-agent --flag"),
            status: SessionStatus::Running,
            annotation: None,
            restart_offer: RestartOffer::Resume,
            tabs: vec![TabInfo { id: "tab-1".into() }],
            github_repo: None,
            working_copy: None,
            notifications: Vec::new(),
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
#[farhelm_testtrace::test]
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
                        agent_kind: farhelm_proto::AgentKind::Generic,
                        parent: None,
                        id: "sess-1".into(),
                        title: String::new(),
                        created_at: 1_700_000_000,
                        last_activity_at: 1_700_000_000,
                        last_work_started_at: 0,
                        creation_seq: None,
                        cwd: "/some/dir".into(),
                        canonical_cwd: None,
                        invocation: "some-agent".into(),
                        launch: farhelm_proto::SessionLaunch::plain_command("some-agent"),
                        status: farhelm_proto::SessionStatus::Unknown,
                        annotation: None,
                        restart_offer: farhelm_proto::RestartOffer::default(),
                        tabs: Vec::new(),
                        github_repo: None,
                        working_copy: None,
                        notifications: Vec::new(),
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
/// the precondition header (`farhelm_proto::http::PRECONDITION_HEADER`), forwarded nowhere; the same
/// body naming the current connection is created normally.
///
/// The action names one installation. The client checks before it sends so
/// a retarget or adoption cannot launch the bundle on a successor host the
/// user did not choose.
///
/// "Reaches no supervisor" is asserted by ORDER rather than by a timeout:
/// the peer asserts on the FIRST create it is sent, and a forwarded stale
/// create would arrive in that slot and fail there by name.
#[farhelm_testtrace::test]
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
        let ControlMsg::CreateSession { req_id, launch, .. } = request else {
            panic!("expected CreateSession, got {request:?}");
        };
        assert_eq!(
            launch.map(|launch| launch.display_command()).as_deref(),
            Some("claude"),
            "the only create that may reach a supervisor is the current-connection one"
        );
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: SessionInfo {
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    parent: None,
                    id: "sess-new".into(),
                    title: "sess-new".into(),
                    created_at: 1_700_000_500,
                    last_activity_at: 1_700_000_500,
                    last_work_started_at: 0,
                    creation_seq: None,
                    cwd: "/work".into(),
                    canonical_cwd: None,
                    invocation: "claude".into(),
                    launch: farhelm_proto::SessionLaunch::plain_command("claude"),
                    status: farhelm_proto::SessionStatus::Unknown,
                    annotation: None,
                    restart_offer: farhelm_proto::RestartOffer::default(),
                    tabs: Vec::new(),
                    github_repo: None,
                    working_copy: None,
                    notifications: Vec::new(),
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

    let (status, precondition_headers, body) = post_text_headers(
        &harness,
        "/api/sessions",
        serde_json::json!({
            "cwd": "/work",
            "command": {"command": "claude", "yolo": false},
            "expected_incarnation": current - 1,
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT);
    assert!(
        is_stale_precondition(&precondition_headers),
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
            "command": {"command": "claude", "yolo": false},
            "expected_incarnation": current,
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    peer.await.unwrap();
}

/// Browse results are destination-specific just like creates. A stale
/// connection claim must be refused before dispatch, or an old dialog could
/// populate itself with paths from the replacement installation.
#[farhelm_testtrace::test]
async fn a_browse_prepared_against_a_replaced_connection_reaches_no_supervisor() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (reader, writer) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(reader);
        let mut writer = FrameWriter::new(writer);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::BrowseDirectory { req_id, cwd } = request else {
            panic!("expected the current browse request, got {request:?}");
        };
        assert_eq!(cwd, "/work");
        writer
            .write_frame(&Frame::control(&ControlMsg::DirectoryListing {
                req_id,
                cwd: "/work".to_string(),
                parent: Some("/".to_string()),
                children: Vec::new(),
                truncated: false,
            }))
            .await
            .unwrap();
    });
    let harness = rest_harness::spliced_helm(client_side).await;
    let local = rest_harness::local_id(&harness.store).await;
    let current = harness
        .manager
        .status(local)
        .expect("local actor")
        .incarnation;

    let (status, precondition_headers, body) = post_text_headers(
        &harness,
        "/api/browse-directory",
        serde_json::json!({"host": local, "cwd": "/work", "expected_incarnation": current - 1}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT);
    assert!(is_stale_precondition(&precondition_headers), "{body}");

    let (status, body) = post_text(
        &harness,
        "/api/browse-directory",
        serde_json::json!({"host": local, "cwd": "/work", "expected_incarnation": current}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    peer.await
        .expect("only the current request reaches the peer");
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
    let (status, _, text) = post_text_headers(harness, uri, body).await;
    (status, text)
}

/// Whether a refusal carries the helm's stale-connection precondition header,
/// the one signal a client may act on by discarding its intent and
/// re-reading. Checked on the header because the body can quote a supervisor.
fn is_stale_precondition(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(farhelm_proto::http::PRECONDITION_HEADER)
        .is_some_and(|value| value == farhelm_proto::http::PRECONDITION_INCARNATION)
}

/// Retain outcome headers as well as prose: a Conflict's text cannot prove
/// whether a fresh create was rejected before dispatch or already accepted.
async fn post_text_headers(
    harness: &rest_harness::Harness,
    uri: &str,
    body: serde_json::Value,
) -> (axum::http::StatusCode, axum::http::HeaderMap, String) {
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
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
            "command": {"command": "agent", "yolo": false},
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
#[farhelm_testtrace::test]
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

        let mut body =
            serde_json::json!({ "cwd": "/tmp", "command": {"command": "agent", "yolo": false} });
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

/// Directory browse has the same target-host safety boundary as create: a
/// remote path is only meaningful on the remote supervisor, and silently
/// reading the helm's local filesystem would present a valid-looking but
/// wrong destination to the person selecting it.
#[farhelm_testtrace::test]
async fn browse_directory_routes_to_the_named_host_and_preserves_its_reply() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame};

    let (local_client, local_peer) = tokio::io::duplex(64 * 1024);
    let local_task = tokio::spawn(silent_supervisor(local_peer));
    let (remote_client, remote_peer) = tokio::io::duplex(64 * 1024);
    let remote_task = tokio::spawn(async move {
        let (reader, writer) = tokio::io::split(remote_peer);
        let mut reader = FrameReader::new(reader);
        let mut writer = FrameWriter::new(writer);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::BrowseDirectory { req_id, cwd } = request else {
            panic!("expected BrowseDirectory, got {request:?}");
        };
        assert_eq!(cwd, "~/project");
        writer
            .write_frame(&Frame::control(&ControlMsg::DirectoryListing {
                req_id,
                cwd: "/remote/home/project".to_string(),
                parent: Some("/remote/home".to_string()),
                children: vec!["/remote/home/project/src".to_string()],
                truncated: false,
            }))
            .await
            .unwrap();
    });
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

    let (status, body) = post_text(
        &harness,
        "/api/browse-directory",
        serde_json::json!({"host": remote, "cwd": "~/project"}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let listing: serde_json::Value = serde_json::from_str(&body).expect("browse JSON");
    assert_eq!(listing["cwd"], "/remote/home/project");
    assert_eq!(
        listing["children"],
        serde_json::json!(["/remote/home/project/src"])
    );

    local_task.abort();
    remote_task.await.expect("remote browse peer");
}

/// A terminal socket for a session on a non-connected host must be
/// refused the same way every other operation is — and must SAY so, as
/// the ordinary `detached` notice, rather than closing bare.
///
/// SPEC.md wants "no terminal to show and no pretense of one", and a
/// silent close is exactly a pretense the browser would blame on the
/// network. Riding the existing notice shape is also what lets the UI
/// render this without a new message type.
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
        serde_json::json!({ "cwd": "/tmp", "command": {"command": "agent", "yolo": false} }),
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
#[farhelm_testtrace::test]
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

/// Spec: a create whose reply names a session id another host already
/// caches fails with a conflict naming the creating host, and the existing
/// session stays listed, and routed, under its original host.
///
/// Why: before this, the create reported success while the write-back was
/// refused, and until the creating host's next refresh marked the id
/// contested, opening, typing into, or stopping the "new" session reached
/// the other machine's session. Only a buggy or hostile supervisor can
/// reply this way, since honest ones mint random ids, so the test plays
/// one.
#[farhelm_testtrace::test]
async fn a_create_reply_naming_another_hosts_session_id_is_refused() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame};

    let (creator_client, creator_peer) = tokio::io::duplex(64 * 1024);
    let creator_task = tokio::spawn(async move {
        let (r, w) = tokio::io::split(creator_peer);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CreateSession { req_id, .. } = request else {
            panic!("expected CreateSession, got {request:?}");
        };
        // The id the OWNER host already holds.
        writer
            .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                req_id,
                session: rest_harness::session("shared", 200),
            }))
            .await
            .unwrap();
    });

    let (builder, owner) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@owner",
            rest_harness::HostScript {
                identity: Some("identity-owner".to_string()),
                sessions: vec![rest_harness::session("shared", 100)],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let (builder, creator) = builder
        .ssh(
            "user@creator",
            rest_harness::HostScript {
                identity: Some("identity-creator".to_string()),
                peer: Some(creator_client),
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    harness.await_refreshed(owner).await;
    harness.await_refreshed(creator).await;

    let (status, body) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({"host": creator, "cwd": "/work", "command": {"command": "agent", "yolo": false}}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{body}");
    assert!(
        body.contains(&format!("created on host {creator}")) && body.contains("shared"),
        "the refusal must name the creating host and the colliding id: {body}"
    );
    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(row_ids(&value), vec!["shared"]);
    assert_eq!(
        value["sessions"][0]["host"], owner,
        "the owner's session keeps its host"
    );
    creator_task.await.unwrap();
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
#[farhelm_testtrace::test]
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

/// Every contested claimant must be compared with the cached owner, not
/// merely the first claimant in host-id order. A cache handoff can leave an
/// old contested observation in place until its next refresh; if that host
/// is now the cached owner, a later claimant must still keep routing closed.
///
/// The fixture performs that handoff through the real cache API after two
/// actors have reported their standing collision. It then proves both sides
/// of the boundary: `[owner, other]` refuses the route, while the remaining
/// sole self-claim is not itself ambiguous once the other actor refreshes.
#[farhelm_testtrace::test]
async fn resolve_owner_rejects_a_later_contested_claimant_after_a_self_claim() {
    let (builder, original_owner) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "user@original-owner",
            rest_harness::HostScript {
                identity: Some("identity-original-owner".to_string()),
                sessions: vec![rest_harness::session("contested-handoff", 100)],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let (builder, cached_owner) = builder
        .ssh(
            "user@cached-owner",
            rest_harness::HostScript {
                identity: Some("identity-cached-owner".to_string()),
                sessions: vec![rest_harness::session("contested-handoff", 100)],
                reachable: false,
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let (builder, later_claimant) = builder
        .ssh(
            "user@later-claimant",
            rest_harness::HostScript {
                identity: Some("identity-later-claimant".to_string()),
                sessions: vec![rest_harness::session("contested-handoff", 100)],
                reachable: false,
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    harness.await_refreshed(original_owner).await;

    for claimant in [cached_owner, later_claimant] {
        harness
            .fleet
            .edit(claimant, |script| script.reachable = true);
        harness
            .manager
            .retry_now(claimant)
            .await
            .expect("the claimant actor is registered");
        harness.await_refreshed(claimant).await;
    }
    assert_eq!(
        harness.manager.contested_claimants("contested-handoff"),
        vec![cached_owner, later_claimant],
        "both actors must still report the id before moving the cache owner"
    );

    harness
        .store
        .replace_host_sessions(original_owner, "identity-original-owner", Vec::new(), false)
        .await
        .expect("clear the original cache owner");
    harness
        .store
        .replace_host_sessions(
            cached_owner,
            "identity-cached-owner",
            vec![rest_harness::session("contested-handoff", 100)],
            false,
        )
        .await
        .expect("move the cache entry while the actor observations remain live");
    assert_eq!(
        harness
            .store
            .host_of_session("contested-handoff")
            .await
            .unwrap(),
        Some(cached_owner),
        "the cached owner must be the first sorted contested claimant"
    );

    let err = resolve_owner(&harness.state, "contested-handoff")
        .await
        .err()
        .expect("a later different claimant must keep the route fail-closed");
    assert!(
        matches!(
            err.downcast_ref::<store::HostStoreError>(),
            Some(store::HostStoreError::SessionOwnerAmbiguous { session, first, second })
                if session == "contested-handoff"
                    && *first == cached_owner
                    && *second == later_claimant
        ),
        "the refusal must preserve the sorted cache-owner and claimant pair: {err:#}"
    );

    harness
        .fleet
        .edit(later_claimant, |script| script.sessions = Vec::new());
    harness.fleet.kill_connection(later_claimant);
    harness
        .await_refreshed_as(later_claimant, "identity-later-claimant", 0)
        .await;
    assert_eq!(
        harness.manager.contested_claimants("contested-handoff"),
        vec![cached_owner],
        "the remaining claimant is the cached owner itself"
    );
    let (owner, _) = resolve_owner(&harness.state, "contested-handoff")
        .await
        .expect("a sole self-claim must remain routable");
    assert_eq!(owner, cached_owner);
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
/// Driven by a BARRIER rather than by timing: the scripted host's next
/// list reply is held until the create has completed, so the
/// interleaving under test is the one that actually happens rather than
/// whichever one a sleep happened to produce.
#[farhelm_testtrace::test]
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

    // Drive refreshes explicitly so no automatic successor can repair a
    // stale publication before it is inspected. The initial canned list
    // omits the new session: it describes the world before the create.
    let harness = rest_harness::FleetBuilder::new()
        .await
        .refresh_every(std::time::Duration::from_secs(3600))
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
    let before = harness.fleet.host_requests(local);
    let release = harness.fleet.hold_next_list(local);
    harness.manager.refresh_now(local);
    harness.fleet.await_host_requests(local, before + 1).await;
    // The successor must not repair an erroneously published stale snapshot
    // before either the list or routing assertion observes it.
    let successor = harness.fleet.hold_next_list(local);

    let (status, body) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({ "cwd": "/tmp", "command": {"command": "agent", "yolo": false} }),
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
    harness.manager.refresh_now(local);
    harness.fleet.await_host_requests(local, before + 2).await;

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
    let _ = successor.send(());
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
#[farhelm_testtrace::test]
async fn creating_on_an_unknown_host_is_refused_without_falling_back() {
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(silent_supervisor(peer_side));
    let harness = rest_harness::spliced_helm(client_side).await;

    let (status, body) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({ "cwd": "/tmp", "command": {"command": "agent", "yolo": false}, "host": 9999 }),
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
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
        "/api/sessions/survivor/stop",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT);
    assert!(
        body.contains("unreachable-reprobing") && body.contains(&host.to_string()),
        "stop must name the stale owner's host and current state: {body}"
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
#[farhelm_testtrace::test]
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
        serde_json::json!({ "cwd": "/tmp", "command": {"command": "agent", "yolo": false}, "host": host }),
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

/// A session created on an identity-less host BEFORE its first session
/// listing has completed must be routable at once too.
///
/// Such a host's in-memory list used to exist only after a successful
/// refresh, and recording a created session into a list that does not exist
/// is refused. A create accepted in the window after the host connected (or
/// while its refreshes kept failing) therefore answered "no such session" to
/// every operation until a refresh succeeded. The host's first listing is
/// held for the whole test, so the window stays open throughout.
#[farhelm_testtrace::test]
async fn a_session_created_before_an_identity_less_hosts_first_refresh_is_routable() {
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
                        session: rest_harness::session("early-new", 900),
                    })
                    .await
                    .unwrap(),
                Ok(ControlMsg::StopSession { req_id, session_id }) => {
                    assert_eq!(session_id, "early-new");
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
            "user@no-identity-early",
            rest_harness::HostScript {
                identity: None,
                sessions: Vec::new(),
                peer: Some(client_side),
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let release_first_list = builder.hold_first_list(host);
    let harness = builder.start().await;
    let state = harness
        .await_state(host, |state| state.phase() == "connected")
        .await;
    assert!(
        matches!(
            state,
            crate::manager::HostState::Connected {
                last_refresh: crate::manager::RefreshHealth::Pending,
                ..
            }
        ),
        "fixture premise: no refresh has completed: {state:?}"
    );

    let (status, body) = post_text(
        &harness,
        "/api/sessions",
        serde_json::json!({ "cwd": "/tmp", "command": {"command": "agent", "yolo": false}, "host": host }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");

    let (status, body) = post_text(
        &harness,
        "/api/sessions/early-new/stop",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "a session created before the first refresh must route immediately: {body}"
    );
    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(row_ids(&value), vec!["early-new"]);
    drop(release_first_list);
    peer.abort();
}

/// An identity-less host reporting an id another host already caches is
/// listed once, under the cached (first) claimant, and counted once.
///
/// Within the cache the first claim already holds (SPEC_impl.md's duplicate
/// id rule), but an identity-less host's rows come from memory and were
/// appended to the merged list unchecked, so the session showed twice and
/// `total` counted it twice, while routing refused both copies. The rule is
/// about keeping the LIST coherent, so it has to hold on both storage paths.
#[farhelm_testtrace::test]
async fn an_identity_less_duplicate_of_a_cached_id_is_listed_once() {
    let (builder, unbound) = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("local-identity".to_string()),
            sessions: vec![rest_harness::session("shared-id", 100)],
            ..rest_harness::HostScript::default()
        })
        .await
        .ssh(
            "user@no-identity-dup",
            rest_harness::HostScript {
                identity: None,
                sessions: vec![
                    rest_harness::session("shared-id", 200),
                    rest_harness::session("only-unbound", 300),
                ],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    let local = rest_harness::local_id(&harness.store).await;
    harness.await_refreshed(local).await;
    harness.await_refreshed(unbound).await;

    let (status, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let mut ids = row_ids(&value);
    ids.sort();
    assert_eq!(ids, vec!["only-unbound", "shared-id"], "{value}");
    assert_eq!(value["total"], 2, "the duplicate must not be counted twice");
    let shared = value["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "shared-id")
        .expect("the shared id is listed");
    assert_eq!(shared["host"], local, "the cached (first) claim holds");
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
#[farhelm_testtrace::test]
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
            // Exactly what a real supervisor sends: a freshly computed
            // offer and a deliberately unknown status (`publish_relaunched`).
            writer
                .write_control(&ControlMsg::SessionRestarted {
                    req_id,
                    session: farhelm_proto::SessionInfo {
                        status: farhelm_proto::SessionStatus::Unknown,
                        restart_offer: farhelm_proto::RestartOffer::Resume,
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
        serde_json::json!({}),
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
        after["sessions"][0]["restart_offer"], "resume",
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
#[farhelm_testtrace::test]
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
        serde_json::json!({}),
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
        // sleep-ok: poll committed running status after the refresh request, checking time between reads.
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
/// (pinned above).
#[farhelm_testtrace::test]
async fn a_stale_sessions_detail_is_served_from_the_cache_and_marked_stale() {
    let builder = rest_harness::FleetBuilder::new().await;
    let (builder, host) = builder
        .ssh(
            "user@breaks",
            rest_harness::HostScript {
                identity: Some("identity-original".to_string()),
                sessions: vec![farhelm_proto::SessionInfo {
                    title: "the work in progress".to_string(),
                    cwd: "/home/user/project".to_string(),
                    canonical_cwd: None,
                    ..rest_harness::session("owned", 100)
                }],
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

    let (status, value) = get_json(&harness, "/api/sessions/owned").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(value["title"], "the work in progress");
    assert_eq!(value["cwd"], "/home/user/project");
    assert_eq!(value["host"], host);
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
) -> farhelm_proto::SessionInfo {
    farhelm_proto::SessionInfo {
        cwd: cwd.to_string(),
        canonical_cwd: None,
        title: title.to_string(),
        status,
        ..rest_harness::session(id, created_at)
    }
}

/// A two-host fleet whose four sessions differ along every filter
/// dimension at once, so that each single-dimension assertion below
/// distinguishes ONE property rather than accidentally selecting on
/// several.
async fn filterable_fleet() -> (rest_harness::Harness, store::HostId, store::HostId) {
    use farhelm_proto::SessionStatus;

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
                    )
                },
                filterable(
                    "local-idle",
                    300,
                    "/home/me/notes",
                    "Read the SPEC",
                    SessionStatus::Idle,
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
                    ),
                    farhelm_proto::SessionInfo {
                        parent: Some("root-session".to_string()),
                        ..filterable(
                            "alpha-exited",
                            100,
                            "/srv/alpha/other",
                            "Drain the queue",
                            SessionStatus::Exited { exit_code: Some(0) },
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

/// Every dimension SPEC.md names — host, parent, directory, status,
/// title — narrows the list by itself, with the semantics
/// `store::SessionFilter` documents.
///
/// One test rather than five because the fixture is the expensive part
/// and the assertions are one line each; what matters is that each
/// parameter selects a DIFFERENT subset, which is only visible with all
/// five side by side.
#[farhelm_testtrace::test]
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

/// Filters AND together, and the combination narrows further than
/// either alone.
///
/// Pinned because the alternative (OR, or last-parameter-wins) reads
/// identically in a single-filter test: a client refining a search would
/// see the list GROW, which is the opposite of what refining means.
#[farhelm_testtrace::test]
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
/// A status word this build does not know is a 400 naming the
/// vocabulary, never an empty list.
///
/// The failure mode this prevents is a silent lie: a client (or a user
/// hand-editing a URL) that misspells a status would otherwise be told
/// there are no such sessions, which is indistinguishable from the truth
/// and far more likely to be believed.
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
async fn only_an_empty_filter_value_clears_it_and_whitespace_is_content() {
    use farhelm_proto::SessionStatus;

    let harness = rest_harness::helm_listing(vec![
        filterable(
            "spaced",
            200,
            "/srv/my project",
            "fix  the  spacing",
            SessionStatus::Running,
        ),
        filterable(
            "plain",
            100,
            "/srv/plain",
            "ordinary",
            SessionStatus::Running,
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

    // Clearing the only predicate restores the unfiltered response, which
    // omits the matching count rather than duplicating the fleet total.
    let (_, value) = get_json(&harness, "/api/sessions?title=").await;
    assert_eq!(row_ids(&value), vec!["spaced", "plain"]);
    assert_eq!(value["total"], 2);
    assert!(value.get("matching").is_none());
}
/// An identity-less host's sessions live in the manager's MEMORY rather
/// than in helm.db, so they reach the merged list by a different path —
/// and the filter has to apply on that path too.
///
/// The bug this pins is a silent one: a filter applied to one source and
/// not the other would let such a host's rows flow through unfiltered, so
/// a search would return sessions that plainly do not match beside ones
/// that do, with `matching` counting them.
#[farhelm_testtrace::test]
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
                ),
                filterable(
                    "memory-idle",
                    100,
                    "/opt/other",
                    "Quiet one",
                    SessionStatus::Idle,
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
// ---- Listing order (`?sort=`) ------------------------------------
//
// Same posture as the filter tests above: every assertion drives the real
// query string through the real handler, because the query string IS the
// contract. An order asserted only against `ListSort::position` would prove
// the comparison works without proving a request can ask for it.

/// A session with the two fields the non-default orders read.
///
/// The default fixture falls back to creation for work ordering and copies
/// the id into the title, which makes every order the same order — useless
/// here, where the whole point is telling three sequences apart.
fn sortable(
    id: &str,
    created_at: i64,
    work_start_seconds: i64,
    title: &str,
) -> farhelm_proto::SessionInfo {
    sortable_as(
        id,
        created_at,
        work_start_seconds,
        title,
        farhelm_proto::SessionStatus::Running,
    )
}

/// [`sortable`], with the reported status the activity grouping reads.
///
/// Statuses are explicit per row rather than inherited from the harness
/// default, because a mixed-status sort test that left them implicit would
/// be a same-status test wearing a comment.
fn sortable_as(
    id: &str,
    created_at: i64,
    work_start_seconds: i64,
    title: &str,
    status: farhelm_proto::SessionStatus,
) -> farhelm_proto::SessionInfo {
    farhelm_proto::SessionInfo {
        last_work_started_at: work_start_seconds * 1000,
        title: title.to_string(),
        status,
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
                    // `last_work_started_at` sends 0, which is "unknown" and must
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
#[farhelm_testtrace::test]
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
        assert!(
            value.get("matching").is_none(),
            "{uri} added a filter count"
        );
    }

    let (status, body) = get_json(&harness, "/api/sessions?sort=cwd").await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    let text = body.as_str().unwrap_or_default();
    assert!(
        text.contains("created") && text.contains("activity") && text.contains("title"),
        "the refusal must name the vocabulary it accepts, got {text:?}"
    );
}

/// `?sort=activity` serves connected Running and Waiting rows first even
/// when idle and ended rows hold newer burst keys, while the other two
/// orders and both counts ignore the grouping entirely.
///
/// Spec: SPEC.md's most-recent-activity order groups by reported liveness
/// before comparing burst keys. The fixture inverts the keys on purpose —
/// the active rows are the oldest bursts — so a key-only order would come
/// out backwards. The `status=running` leg pins that a filter still matches
/// the REPORTED status: sorting and filtering are independent dimensions,
/// and grouping changes only the sequence.
#[farhelm_testtrace::test]
async fn activity_groups_by_reported_status_while_other_orders_and_counts_ignore_it() {
    use farhelm_proto::SessionStatus;

    let (builder, alpha) = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("identity-local".to_string()),
            sessions: vec![
                sortable_as("local-idle", 400, 900, "Delta", SessionStatus::Idle),
                sortable_as("local-running", 300, 100, "Charlie", SessionStatus::Running),
            ],
            ..rest_harness::HostScript::default()
        })
        .await
        .ssh(
            "user@alpha",
            rest_harness::HostScript {
                identity: Some("identity-alpha".to_string()),
                sessions: vec![
                    sortable_as("alpha-waiting", 200, 500, "Bravo", SessionStatus::Waiting),
                    sortable_as(
                        "alpha-exited",
                        100,
                        950,
                        "Alpha",
                        SessionStatus::Exited { exit_code: Some(0) },
                    ),
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

    let (status, value) = get_json(&harness, "/api/sessions?sort=activity").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        row_ids(&value),
        vec![
            "alpha-waiting",
            "local-running",
            "alpha-exited",
            "local-idle"
        ],
        "Waiting sorts with Running on its older key; the ended and idle rows follow in key \
         order despite holding the newest bursts"
    );

    let (_, value) = get_json(&harness, "/api/sessions").await;
    assert_eq!(
        row_ids(&value),
        vec![
            "local-idle",
            "local-running",
            "alpha-waiting",
            "alpha-exited"
        ],
        "creation order follows creation time across the mixed statuses"
    );
    let (_, value) = get_json(&harness, "/api/sessions?sort=title").await;
    assert_eq!(
        row_ids(&value),
        vec![
            "alpha-exited",
            "alpha-waiting",
            "local-running",
            "local-idle"
        ],
        "title order follows titles across the mixed statuses"
    );

    for uri in [
        "/api/sessions",
        "/api/sessions?sort=activity",
        "/api/sessions?sort=title",
    ] {
        let (_, value) = get_json(&harness, uri).await;
        assert_eq!(value["total"], 4, "{uri} changed the fleet total");
        assert!(
            value.get("matching").is_none(),
            "{uri} added a filter count"
        );
    }

    let (_, value) = get_json(&harness, "/api/sessions?sort=activity&status=running").await;
    assert_eq!(
        row_ids(&value),
        vec!["local-running"],
        "a status filter matches the reported status, not the group"
    );
    assert_eq!(value["matching"], 1);
    assert_eq!(
        value["total"], 4,
        "the denominator still describes the whole view"
    );
}

/// Taking a host down demotes its Running rows to the inactive group —
/// through the real connection state, not a hand-built flag.
///
/// The rows keep their cached burst keys and their reported Running status
/// (a `status=running` filter still matches them); what changes is the
/// `stale` bit the grouping reads, set from the host actually leaving the
/// connected state. That is the whole stale plumbing under test: scripted
/// disconnect, awaited non-connected state, cached rows re-served marked.
#[farhelm_testtrace::test]
async fn disconnecting_a_host_demotes_its_running_rows_through_real_stale_plumbing() {
    let (builder, alpha) = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("identity-local".to_string()),
            sessions: vec![sortable("local-running", 300, 100, "Local")],
            ..rest_harness::HostScript::default()
        })
        .await
        .ssh(
            "user@alpha",
            rest_harness::HostScript {
                identity: Some("identity-alpha".to_string()),
                sessions: vec![sortable("alpha-running", 200, 900, "Alpha")],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    let local = rest_harness::local_id(&harness.store).await;
    for host in [local, alpha] {
        harness.await_refreshed(host).await;
    }

    let (_, value) = get_json(&harness, "/api/sessions?sort=activity").await;
    assert_eq!(
        row_ids(&value),
        vec!["alpha-running", "local-running"],
        "while both hosts are connected, the newer burst leads"
    );

    harness.fleet.take_down(alpha);
    harness
        .await_state(alpha, |state| state.phase() == "unreachable-reprobing")
        .await;

    let (status, value) = get_json(&harness, "/api/sessions?sort=activity").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        row_ids(&value),
        vec!["local-running", "alpha-running"],
        "the disconnected host's newer burst drops below the connected row"
    );
    let rows = value["sessions"].as_array().expect("sessions is an array");
    assert_eq!(rows[0]["stale"], false);
    assert_eq!(
        rows[1]["stale"], true,
        "the demoted row is last-known knowledge and says so"
    );
    assert_eq!(
        rows[1]["last_work_started_at"], 900_000,
        "the demotion moved the row without touching its cached key"
    );
    assert_eq!(value["total"], 2);
    assert!(value.get("matching").is_none());

    let (_, value) = get_json(&harness, "/api/sessions?sort=activity&status=running").await;
    assert_eq!(
        row_ids(&value),
        vec!["local-running", "alpha-running"],
        "a stale Running row still matches a Running filter — it sorts second, it is not filtered out"
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
#[farhelm_testtrace::test]
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

/// Malformed repository text is refused before preview or launch validation.
/// A connected silent peer proves that neither lookup nor create reaches the
/// supervisor, while the error still names the user's malformed identifier.
#[farhelm_testtrace::test]
async fn create_with_invalid_github_repo_names_the_parse_error() {
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(silent_supervisor(peer_side));
    let harness = rest_harness::spliced_helm(client_side).await;
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({
                "cwd": "/tmp/whatever",
                "command": {"command": "some-agent", "yolo": false},
                "github_checkout": {"repo": "no separator here", "title": null}
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        text.contains("invalid GitHub repository"),
        "the refusal must be the parse error, not the backend refusal: {text}"
    );
    peer.await.unwrap();
}

/// A create body with a valid repository and accepted preview proceeds to the
/// fresh-checkout resolution, and with no checkout root configured the
/// refusal names the exact CLI command that fixes it — the operator-gap
/// message, not a generic failure, and still before any host contact,
/// history write, or supervisor frame.
///
/// No intent key is supplied, so no reconciliation lookup is needed. The
/// connected silent peer is joined to prove that no create frame is sent.
#[farhelm_testtrace::test]
async fn create_with_valid_github_repo_without_a_root_names_the_set_root_command() {
    use tower::ServiceExt;

    // A CONNECTED silent supervisor, not a dropped peer: the refusal must
    // fire at the helm's resolution (before any create frame reaches the
    // host), which needs the host dial to succeed first.
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(silent_supervisor(peer_side));
    let harness = rest_harness::spliced_helm(client_side).await;
    let (claim, _) = super::create_target(&harness.state, None).unwrap();
    let preview = serde_json::json!({
        "canonical_root": "/old-root", "basename": "bar-1", "cwd": "/old-root/bar-1",
        "config_revision": 1, "host": claim.host.to_string(), "incarnation": claim.incarnation,
        "installation_identity": claim.identity,
    });
    let app = harness.router();

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/sessions")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({
                "cwd": "",
                "command": {"command": "some-agent", "yolo": false},
                "github_checkout": {"repo": "acme/bar", "title": null, "preview": preview}
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        text.contains("no checkout root is configured")
            && text.contains("checkout-config set-root"),
        "a valid repository without a configured root must hit the actionable \
         set-root refusal: {text}"
    );
    // The refusal happened at the helm: the connected silent supervisor
    // proves no create frame reached it. (It panics inside its own task on
    // a leaked frame; joining asserts it stayed silent for the window.)
    peer.await.unwrap();
}

/// A recorded fresh request must reach lookup before stale settings can
/// refuse it. Unknown keys still face current
/// preconditions; a different installation must not receive even the lookup.
/// The scripted peer observes the actual frame sequence and stays open until
/// both REST calls finish, so an accidental create cannot hide behind EOF.
#[farhelm_testtrace::test]
async fn fresh_rest_reconciliation_precedes_mutable_resolution_and_binds_installation() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let (finished_tx, mut finished_rx) = tokio::sync::oneshot::channel();
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        for expected_key in ["recorded-key", "unknown-key", "unknown-stale-config"] {
            let frame = reader.read_frame().await.unwrap().expect("lookup frame");
            let ControlMsg::ReconcileGithubCheckout {
                req_id,
                intent_key,
                client_identity,
                refuse_unknown,
                ..
            } = parse_control(&frame).unwrap()
            else {
                panic!("expected lookup, never create");
            };
            assert_eq!(intent_key, expected_key);
            assert!(
                !refuse_unknown,
                "the initial lookup must not spend an unknown key"
            );
            let identity: serde_json::Value = serde_json::from_str(&client_identity).unwrap();
            assert_eq!(identity[0], "github_create_request_v2");
            assert!(client_identity.contains("recorded-agent"));
            assert!(client_identity.contains("local-identity"));
            let session = if expected_key == "recorded-key" {
                Some(rest_harness::session("recorded-session", 12))
            } else {
                None
            };
            writer
                .write_control(&ControlMsg::GithubCheckoutReconciled { req_id, session })
                .await
                .unwrap();
            if expected_key != "recorded-key" {
                let frame = reader
                    .read_frame()
                    .await
                    .unwrap()
                    .expect("durable refusal request");
                let ControlMsg::ReconcileGithubCheckout {
                    req_id,
                    intent_key,
                    client_identity: refused_identity,
                    refuse_unknown: true,
                    ..
                } = parse_control(&frame).unwrap()
                else {
                    panic!("a local keyed refusal must be settled, never dispatched as create");
                };
                assert_eq!(intent_key, expected_key);
                assert_eq!(refused_identity, client_identity);
                writer
                    .write_control(&ControlMsg::Error {
                        req_id,
                        kind: farhelm_proto::ErrorKind::CheckoutConflict,
                        message: "fixture durable refusal".into(),
                    })
                    .await
                    .unwrap();
            }
        }
        tokio::select! {
            biased;
            frame = reader.read_frame() => panic!("unexpected frame after lookup: {frame:?}"),
            result = &mut finished_rx => result.unwrap(),
        }
    });
    let harness = rest_harness::spliced_helm(client_side).await;
    let (claim, _) = super::create_target(&harness.state, None).unwrap();
    harness
        .store
        .set_checkout_root(None, "/current-root")
        .await
        .unwrap();
    let current = harness.store.resolve_checkout_config(None).await.unwrap();
    let mut body = serde_json::json!({
        "cwd": "", "command": {"command": "recorded-agent", "yolo": false}, "intent_key": "recorded-key",
        "expected_incarnation": claim.incarnation + 100,
        "github_checkout": { "repo": "acme/bar", "title": null, "preview": {
            "canonical_root": "/old-root", "basename": "bar-1", "cwd": "/old-root/bar-1",
            "config_revision": current.config_revision - 1,
            "host": claim.host.to_string(), "incarnation": claim.incarnation + 100,
            "installation_identity": "local-identity"
        }}
    });
    let (status, text) = post_text(&harness, "/api/sessions", body.clone()).await;
    assert_eq!(status, axum::http::StatusCode::OK, "{text}");
    let returned: farhelm_proto::SessionInfo = serde_json::from_str(&text).unwrap();
    assert_eq!(returned.id, "recorded-session");

    body["intent_key"] = serde_json::json!("unknown-key");
    let (status, precondition_headers, text) =
        post_text_headers(&harness, "/api/sessions", body.clone()).await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{text}");
    assert!(is_stale_precondition(&precondition_headers), "{text}");

    body["intent_key"] = serde_json::json!("unknown-stale-config");
    body["expected_incarnation"] = serde_json::json!(claim.incarnation);
    body["github_checkout"]["preview"]["incarnation"] = serde_json::json!(claim.incarnation);
    let (status, headers, text) = post_text_headers(&harness, "/api/sessions", body.clone()).await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{text}");
    assert_eq!(headers["x-farhelm-create-outcome"], "definitely-unaccepted");
    assert!(text.contains("checkout settings changed"), "{text}");

    body["github_checkout"]["preview"]["installation_identity"] =
        serde_json::json!("different-installation");
    let (status, text) = post_text(&harness, "/api/sessions", body).await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{text}");
    assert!(text.contains("different host installation"), "{text}");
    finished_tx.send(()).unwrap();
    peer.await.unwrap();
}

/// Local resolution failures need a second, atomic supervisor decision. This
/// covers late launch compilation as well as stale settings, and proves that
/// unknown/transport/ordinary failure replies cannot acquire the durable-proof
/// marker. A source-id winner is vetoed before history/default side effects.
#[farhelm_testtrace::test]
async fn fresh_local_refusals_require_durable_proof_on_both_routes() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, ErrorKind};

    for replacing in [false, true] {
        for cause in ["settings", "launch"] {
            for disposition in [
                "refused",
                "unknown",
                "unavailable",
                "stored-failure",
                "source-veto",
            ] {
                if disposition == "source-veto" && !replacing {
                    continue;
                }
                let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
                let (harness, local) =
                    spliced_replace_harness(client_side, vec![rest_harness::session("sess-1", 12)])
                        .await;
                let (finished_tx, mut finished_rx) = tokio::sync::oneshot::channel();
                let peer = tokio::spawn(async move {
                    let (r, w) = tokio::io::split(peer_side);
                    let mut reader = FrameReader::new(r);
                    let mut writer = FrameWriter::new(w);
                    handshake(&mut reader, &mut writer, "supervisor")
                        .await
                        .unwrap();
                    let message =
                        parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                    let ControlMsg::ReconcileGithubCheckout {
                        req_id,
                        intent_key,
                        client_identity,
                        refuse_unknown: false,
                        ..
                    } = message
                    else {
                        panic!("expected non-mutating lookup: {message:?}")
                    };
                    assert_eq!(intent_key, "failed-local-resolution");
                    writer
                        .write_control(&ControlMsg::GithubCheckoutReconciled {
                            req_id,
                            session: None,
                        })
                        .await
                        .unwrap();
                    let message =
                        parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                    let ControlMsg::ReconcileGithubCheckout {
                        req_id,
                        intent_key: refused_key,
                        client_identity: refused_identity,
                        refuse_unknown: true,
                        ..
                    } = message
                    else {
                        panic!("expected atomic refusal, never create: {message:?}")
                    };
                    assert_eq!(refused_key, intent_key);
                    assert_eq!(
                        refused_identity, client_identity,
                        "selector consumption must not change the original identity"
                    );
                    let reply = match disposition {
                        "refused" => ControlMsg::Error {
                            req_id,
                            kind: ErrorKind::CheckoutConflict,
                            message: "stored durable refusal".into(),
                        },
                        "unknown" => ControlMsg::GithubCheckoutReconciled {
                            req_id,
                            session: None,
                        },
                        "unavailable" => ControlMsg::Error {
                            req_id,
                            kind: ErrorKind::Unavailable,
                            message: "refusal outcome unavailable".into(),
                        },
                        "stored-failure" => ControlMsg::Error {
                            req_id,
                            kind: ErrorKind::InvalidRequest,
                            message: "original stored failure".into(),
                        },
                        "source-veto" => ControlMsg::GithubCheckoutReconciled {
                            req_id,
                            session: Some(rest_harness::session("sess-1", 12)),
                        },
                        _ => unreachable!(),
                    };
                    writer.write_control(&reply).await.unwrap();
                    tokio::select! {
                        biased;
                        frame = reader.read_frame() => panic!("refusal or source veto sent a mutation: {frame:?}"),
                        result = &mut finished_rx => result.unwrap(),
                    }
                });
                harness.await_refreshed(local).await;
                harness
                    .store
                    .set_checkout_root(None, "/configured-root")
                    .await
                    .unwrap();
                let config = harness.store.resolve_checkout_config(None).await.unwrap();
                let (claim, _) = super::create_target(&harness.state, None).unwrap();
                assert!(
                    harness
                        .store
                        .github_repository_history(local, "local-identity")
                        .await
                        .unwrap()
                        .is_empty()
                );
                let mut request = serde_json::json!({
                    "cwd": "",
                    "github_checkout": {"repo": "acme/bar", "preview": {
                        "canonical_root": "/configured-root", "basename": "bar-1", "cwd": "/configured-root/bar-1",
                        "config_revision": config.config_revision - i64::from(cause == "settings"),
                        "host": claim.host.to_string(), "incarnation": claim.incarnation,
                        "installation_identity": "local-identity"
                    }}
                });
                // A Grok launch naming a model is refused only when the
                // selection compiles, which is the late local step this
                // cause needs; the settings cause uses a valid raw command.
                if cause == "launch" {
                    request["launch"] = serde_json::json!({
                        "harness": "grok", "model": "grok-x", "effort": null, "permissions": null
                    });
                } else {
                    request["command"] = serde_json::json!({"command": "claude", "yolo": false});
                }
                let (route, body) = if replacing {
                    (
                        "/api/sessions/sess-1/replace",
                        serde_json::json!({"intent_key": "failed-local-resolution", "with": request}),
                    )
                } else {
                    request["intent_key"] = serde_json::json!("failed-local-resolution");
                    ("/api/sessions", request)
                };
                let (status, headers, text) = post_text_headers(&harness, route, body).await;
                let expected = match disposition {
                    "refused" | "source-veto" => axum::http::StatusCode::CONFLICT,
                    "unavailable" => axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    "stored-failure" => axum::http::StatusCode::BAD_REQUEST,
                    "unknown" if cause == "launch" => axum::http::StatusCode::BAD_REQUEST,
                    _ => axum::http::StatusCode::CONFLICT,
                };
                assert_eq!(
                    status, expected,
                    "{replacing}/{cause}/{disposition}: {text}"
                );
                assert_eq!(
                    headers.contains_key("x-farhelm-create-outcome"),
                    disposition == "refused",
                    "{text}"
                );
                if disposition == "stored-failure" {
                    assert!(text.contains("original stored failure"), "{text}");
                }
                if disposition != "source-veto" {
                    assert!(
                        text.contains(if cause == "settings" {
                            "checkout settings changed"
                        } else {
                            "Grok does not currently expose a verified model choice"
                        }),
                        "{text}"
                    );
                }
                assert!(
                    harness
                        .store
                        .github_repository_history(local, "local-identity")
                        .await
                        .unwrap()
                        .is_empty()
                );
                finished_tx.send(()).unwrap();
                peer.await.unwrap();
            }
        }
    }
}

/// Two identical submissions can both observe an unknown key before one wins.
/// The first reaches CreateSession under the original settings; a latch then
/// changes configuration before the second resolves locally. Atomic refusal
/// must return the accepted winner, including on replacement, without
/// another create or a false non-acceptance marker.
#[farhelm_testtrace::test]
async fn fresh_local_refusal_recovers_a_concurrent_winner_after_settings_change() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, ErrorKind};

    for replacing in [false, true] {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let (harness, local) =
            spliced_replace_harness(client_side, vec![rest_harness::session("sess-1", 12)]).await;
        let (create_seen_tx, create_seen_rx) = tokio::sync::oneshot::channel();
        let (finished_tx, mut finished_rx) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let message = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::ReconcileGithubCheckout {
                req_id,
                client_identity,
                refuse_unknown: false,
                ..
            } = message
            else {
                panic!("first request must begin with lookup: {message:?}")
            };
            writer
                .write_control(&ControlMsg::GithubCheckoutReconciled {
                    req_id,
                    session: None,
                })
                .await
                .unwrap();
            let message = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CreateSession {
                req_id: create_id,
                github_checkout: Some(checkout),
                ..
            } = message
            else {
                panic!("first request must reach one fresh create: {message:?}")
            };
            assert_eq!(checkout.root, "/original-root");
            assert_eq!(checkout.client_identity, client_identity);
            let mut winner = rest_harness::session("concurrent-winner", 13);
            winner.cwd = checkout.preview.cwd;
            winner.canonical_cwd = Some(winner.cwd.clone());
            create_seen_tx.send(()).unwrap();
            let message = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::ReconcileGithubCheckout {
                req_id,
                client_identity: second_identity,
                refuse_unknown: false,
                ..
            } = message
            else {
                panic!("second request must observe unknown before the winner commits: {message:?}")
            };
            assert_eq!(second_identity, client_identity);
            writer
                .write_control(&ControlMsg::GithubCheckoutReconciled {
                    req_id,
                    session: None,
                })
                .await
                .unwrap();
            // Acceptance occurs after both unknown observations. The first
            // response and second local-resolution failure may now interleave.
            writer
                .write_control(&ControlMsg::SessionCreated {
                    req_id: create_id,
                    session: winner.clone(),
                })
                .await
                .unwrap();
            let mut refusals = 0;
            let mut deletes = 0;
            for _ in 0..if replacing { 3 } else { 1 } {
                let message = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                match message {
                    ControlMsg::ReconcileGithubCheckout {
                        req_id,
                        client_identity: refused_identity,
                        refuse_unknown: true,
                        ..
                    } => {
                        assert_eq!(refused_identity, client_identity);
                        refusals += 1;
                        writer
                            .write_control(&ControlMsg::GithubCheckoutReconciled {
                                req_id,
                                session: Some(winner.clone()),
                            })
                            .await
                            .unwrap();
                    }
                    ControlMsg::DeleteSession {
                        req_id, session_id, ..
                    } if replacing => {
                        assert_eq!(session_id, "sess-1");
                        deletes += 1;
                        writer
                            .write_control(&ControlMsg::Error {
                                req_id,
                                kind: ErrorKind::Conflict,
                                message: "retain source".into(),
                            })
                            .await
                            .unwrap();
                    }
                    other => {
                        panic!("only winner recovery and source deletion may follow: {other:?}")
                    }
                }
            }
            assert_eq!(refusals, 1);
            assert_eq!(deletes, if replacing { 2 } else { 0 });
            tokio::select! {
                biased;
                frame = reader.read_frame() => panic!("unexpected duplicate mutation: {frame:?}"),
                result = &mut finished_rx => result.unwrap(),
            }
        });
        harness.await_refreshed(local).await;
        harness
            .store
            .set_checkout_root(None, "/original-root")
            .await
            .unwrap();
        let config = harness.store.resolve_checkout_config(None).await.unwrap();
        let (claim, _) = super::create_target(&harness.state, None).unwrap();
        let mut request = serde_json::json!({
            "cwd": "", "command": {"command": "agent", "yolo": false},
            "github_checkout": {"repo": "acme/bar", "preview": {
                "canonical_root": "/original-root", "basename": "bar-1", "cwd": "/original-root/bar-1",
                "config_revision": config.config_revision, "host": claim.host.to_string(),
                "incarnation": claim.incarnation, "installation_identity": "local-identity"
            }}
        });
        let (route, body) = if replacing {
            (
                "/api/sessions/sess-1/replace",
                serde_json::json!({"intent_key": "concurrent-key", "with": request}),
            )
        } else {
            request["intent_key"] = serde_json::json!("concurrent-key");
            ("/api/sessions", request)
        };
        let first = post_text_headers(&harness, route, body.clone());
        let second = async {
            create_seen_rx
                .await
                .expect("first request must reach dispatch before settings change");
            harness
                .store
                .set_checkout_root(None, "/changed-root")
                .await
                .unwrap();
            assert!(
                harness
                    .store
                    .resolve_checkout_config(None)
                    .await
                    .unwrap()
                    .config_revision
                    > config.config_revision
            );
            post_text_headers(&harness, route, body).await
        };
        let (first, second) = tokio::join!(first, second);
        for (status, headers, text) in [&first, &second] {
            assert_eq!(
                *status,
                if replacing {
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR
                } else {
                    axum::http::StatusCode::OK
                },
                "{text}"
            );
            assert!(
                !headers.contains_key("x-farhelm-create-outcome"),
                "an accepted winner is not a refusal"
            );
            assert!(text.contains("concurrent-winner"), "{text}");
            if replacing {
                assert!(text.contains("both sessions still exist"), "{text}");
            }
        }
        assert_eq!(
            harness
                .store
                .github_repository_history(local, "local-identity")
                .await
                .unwrap(),
            vec![farhelm_proto::parse_github_repo("acme/bar").unwrap()]
        );
        finished_tx.send(()).unwrap();
        peer.await.unwrap();
    }
}

/// Fresh destination validation must reject a contradictory folder before any
/// create or delete reaches the host. Both public entry points share this
/// contract, independently of which agent selector the request uses.
#[farhelm_testtrace::test]
async fn fresh_create_and_replace_refuse_a_contradictory_cwd() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};

    for replacing in [false, true] {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let (harness, local) =
            spliced_replace_harness(client_side, vec![rest_harness::session("sess-1", 12)]).await;
        let (finished_tx, mut finished_rx) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            tokio::select! {
                biased;
                frame = reader.read_frame() => panic!("contradictory destination contacted peer: {frame:?}"),
                result = &mut finished_rx => result.unwrap(),
            }
        });
        harness.await_refreshed(local).await;
        harness
            .store
            .set_checkout_root(None, "/fresh-root")
            .await
            .unwrap();
        let config = harness.store.resolve_checkout_config(None).await.unwrap();
        let (claim, _) = super::create_target(&harness.state, None).unwrap();
        for (selector, value) in [
            (
                "command",
                serde_json::json!({"command": "agent", "yolo": false}),
            ),
            ("launch", serde_json::json!({"harness": "codex"})),
        ] {
            let mut request = serde_json::json!({
                "cwd": "/unrelated-existing-directory",
                "github_checkout": {"repo": "acme/bar", "preview": {
                    "canonical_root": "/fresh-root", "basename": "bar-1", "cwd": "/fresh-root/bar-1",
                    "config_revision": config.config_revision, "host": claim.host.to_string(),
                    "incarnation": claim.incarnation, "installation_identity": "local-identity"
                }}
            });
            request[selector] = value;
            let (route, body) = if replacing {
                (
                    "/api/sessions/sess-1/replace",
                    serde_json::json!({"with": request}),
                )
            } else {
                ("/api/sessions", request)
            };
            let (status, headers, text) = post_text_headers(&harness, route, body).await;
            assert_eq!(
                status,
                axum::http::StatusCode::BAD_REQUEST,
                "{selector}: {text}"
            );
            assert!(
                text.contains("cwd must be empty or match the accepted preview"),
                "{selector}: {text}"
            );
            assert_eq!(headers["x-farhelm-create-outcome"], "definitely-unaccepted");
        }
        finished_tx.send(()).unwrap();
        peer.await.unwrap();
    }
}

/// Fresh replacement forwards the accepted payload once, then reconciles the
/// same request after configuration changes. A delete refusal retains both
/// sessions. Source-id replays and unknown stale intents must send no delete
/// or create and must not change history; a foreign installation gets no lookup.
/// The peer remains live until an explicit completion signal, so unexpected
/// mutation frames cannot be mistaken for a harmless closed connection.
#[farhelm_testtrace::test]
async fn fresh_replace_reconciles_original_payload_and_vetoes_source_replays() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, ErrorKind};

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let (harness, local) =
        spliced_replace_harness(client_side, vec![rest_harness::session("sess-1", 12)]).await;
    let fleet = harness.fleet.clone();
    let (finished_tx, mut finished_rx) = tokio::sync::oneshot::channel();
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let mut original_identity = None;
        let mut replacement = rest_harness::session("replacement", 13);
        replacement.cwd = "/original-root/bar-fix".into();
        replacement.canonical_cwd = Some(replacement.cwd.clone());
        for (attempt, key) in ["original", "original", "source-replay", "unknown"]
            .into_iter()
            .enumerate()
        {
            let message = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::ReconcileGithubCheckout {
                req_id,
                intent_key,
                client_identity,
                refuse_unknown,
                ..
            } = message
            else {
                panic!("expected reconciliation, got {message:?}");
            };
            assert_eq!(intent_key, key);
            assert!(!refuse_unknown);
            let identity: serde_json::Value = serde_json::from_str(&client_identity).unwrap();
            assert_eq!(identity[0], "github_replace_request_v1");
            assert_eq!(identity[1], "sess-1");
            if let Some(original) = &original_identity {
                assert_eq!(
                    &client_identity, original,
                    "retry must retain the original request"
                );
            } else {
                original_identity = Some(client_identity.clone());
            }
            let session = match attempt {
                1 => Some(replacement.clone()),
                2 => Some(rest_harness::session("sess-1", 999)),
                _ => None,
            };
            writer
                .write_control(&ControlMsg::GithubCheckoutReconciled { req_id, session })
                .await
                .unwrap();
            if attempt == 3 {
                let message = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::ReconcileGithubCheckout {
                    req_id,
                    intent_key,
                    client_identity: refused_identity,
                    refuse_unknown: true,
                    ..
                } = message
                else {
                    panic!("expected durable refusal, got {message:?}")
                };
                assert_eq!(intent_key, key);
                assert_eq!(refused_identity, client_identity);
                writer
                    .write_control(&ControlMsg::Error {
                        req_id,
                        kind: ErrorKind::CheckoutConflict,
                        message: "fixture durable refusal".into(),
                    })
                    .await
                    .unwrap();
            }
            if attempt == 0 {
                let message = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::CreateSession {
                    req_id,
                    cwd,
                    title,
                    github_checkout: Some(checkout),
                    ..
                } = message
                else {
                    panic!("expected fresh create, got {message:?}");
                };
                assert!(cwd.is_empty());
                assert_eq!(title.as_deref(), Some("Fix"));
                assert_eq!(checkout.root, "/original-root");
                assert_eq!(checkout.preview.cwd, "/original-root/bar-fix");
                assert_eq!(checkout.post_clone.as_deref(), Some("original-hook"));
                assert_eq!(checkout.client_identity, client_identity);
                fleet.edit(local, |script| script.sessions.push(replacement.clone()));
                writer
                    .write_control(&ControlMsg::SessionCreated {
                        req_id,
                        session: replacement.clone(),
                    })
                    .await
                    .unwrap();
            }
            if attempt < 2 {
                let message = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::DeleteSession {
                    req_id, session_id, ..
                } = message
                else {
                    panic!("expected deletion of original, got {message:?}");
                };
                assert_eq!(session_id, "sess-1");
                writer
                    .write_control(&ControlMsg::Error {
                        req_id,
                        kind: ErrorKind::Conflict,
                        message: "fixture retains source".into(),
                    })
                    .await
                    .unwrap();
            }
        }
        tokio::select! {
            biased;
            frame = reader.read_frame() => panic!("unexpected mutation after refusals: {frame:?}"),
            result = &mut finished_rx => result.unwrap(),
        }
    });
    harness.await_refreshed(local).await;
    let (claim, _) = super::create_target(&harness.state, None).unwrap();
    harness
        .store
        .set_checkout_root(None, "/original-root")
        .await
        .unwrap();
    harness
        .store
        .set_checkout_post_clone(None, "original-hook")
        .await
        .unwrap();
    let revision = harness
        .store
        .resolve_checkout_config(None)
        .await
        .unwrap()
        .config_revision;
    let mut body = serde_json::json!({
        "intent_key": "original",
        "with": { "cwd": "/original-root/bar-fix", "command": {"command": "agent", "yolo": false}, "expected_incarnation": claim.incarnation,
            "github_checkout": { "repo": "acme/bar", "title": "Fix", "preview": {
                "canonical_root": "/original-root", "basename": "bar-fix", "cwd": "/original-root/bar-fix",
                "config_revision": revision, "host": claim.host.to_string(), "incarnation": claim.incarnation,
                "installation_identity": "local-identity"
            }}
        }
    });
    let route = "/api/sessions/sess-1/replace";
    for attempt in 0..2 {
        let (status, text) = post_text(&harness, route, body.clone()).await;
        assert_eq!(
            status,
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "{text}"
        );
        assert!(text.contains("both sessions still exist"), "{text}");
        assert!(
            text.contains("replacement") && text.contains("sess-1"),
            "{text}"
        );
        if attempt == 0 {
            harness
                .store
                .set_checkout_root(None, "/changed-root")
                .await
                .unwrap();
            harness
                .store
                .set_checkout_post_clone(None, "changed-hook")
                .await
                .unwrap();
        }
    }
    let history = harness
        .store
        .folder_history(local, "local-identity")
        .await
        .unwrap();
    assert!(
        history.is_empty(),
        "fresh replacement paths are not folder intent"
    );
    assert_eq!(
        harness
            .store
            .github_repository_history(local, "local-identity")
            .await
            .unwrap(),
        vec![farhelm_proto::parse_github_repo("acme/bar").unwrap()]
    );
    body["intent_key"] = serde_json::json!("source-replay");
    let (status, headers, text) = post_text_headers(&harness, route, body.clone()).await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{text}");
    assert!(
        !headers.contains_key("x-farhelm-create-outcome"),
        "a known-key source veto is not an unknown-key refusal"
    );
    assert!(text.contains("no replacement was made"), "{text}");
    assert_eq!(
        harness
            .store
            .folder_history(local, "local-identity")
            .await
            .unwrap(),
        history
    );
    body["intent_key"] = serde_json::json!("unknown");
    let (status, headers, text) = post_text_headers(&harness, route, body.clone()).await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{text}");
    assert_eq!(headers["x-farhelm-create-outcome"], "definitely-unaccepted");
    assert!(text.contains("checkout settings changed"), "{text}");
    body["with"]["github_checkout"]["preview"]["installation_identity"] =
        serde_json::json!("foreign-installation");
    let (status, text) = post_text(&harness, route, body).await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{text}");
    assert!(text.contains("different host installation"), "{text}");
    finished_tx.send(()).unwrap();
    peer.await.unwrap();
}

/// Repository completion resolves only helm-owned root settings and returns
/// installation-scoped, validated identities. Missing configuration, a failed
/// scan and partial results remain distinguishable from a complete empty scan;
/// malformed remote identities are never reflected into suggestions. Recent
/// intent ranks before scan results, dedupes with them and survives scan failure.
#[farhelm_testtrace::test]
async fn github_repository_rest_routes_config_and_preserves_incomplete_status() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, ErrorKind, GithubRepo};
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let (finished_tx, mut finished_rx) = tokio::sync::oneshot::channel();
    let peer = tokio::spawn(async move {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        for attempt in 0..2 {
            let message = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::GithubRepoSearch {
                req_id,
                claim,
                query,
                root,
            } = message
            else {
                panic!("expected repository search, got {message:?}");
            };
            assert_eq!(root.as_deref(), Some("~/target-checkouts"));
            assert_eq!(query, "acme");
            assert!(!claim.host.is_empty());
            assert!(claim.incarnation > 0);
            let reply = if attempt == 0 {
                ControlMsg::GithubRepoResults {
                    req_id,
                    truncated: false,
                    repos: vec![
                        farhelm_proto::parse_github_repo("acme/bar").unwrap(),
                        farhelm_proto::parse_github_repo("acme/bar").unwrap(),
                        GithubRepo {
                            owner: "user:secret@github.com".into(),
                            name: "bad".into(),
                        },
                    ],
                }
            } else {
                ControlMsg::Error {
                    req_id,
                    kind: ErrorKind::Internal,
                    message: "Git failed inspecting https://user:secret@github.com/acme/bar".into(),
                }
            };
            writer.write_control(&reply).await.unwrap();
        }
        tokio::select! {
            biased;
            frame = reader.read_frame() => panic!("unexpected discovery request: {frame:?}"),
            result = &mut finished_rx => result.unwrap(),
        }
    });
    let harness = rest_harness::spliced_helm(client_side).await;
    let (claim, _) = super::create_target(&harness.state, None).unwrap();
    // The most recent matching repo sorts after the scanned name, proving
    // accepted-create order wins. An unrelated recent must be filtered.
    for (index, name) in ["acme/bar", "acme/zebra", "unrelated/repo"]
        .iter()
        .enumerate()
    {
        let repo = farhelm_proto::parse_github_repo(name).unwrap();
        harness
            .store
            .record_create_history_with_destination(
                claim.host,
                "local-identity",
                &rest_harness::session(&format!("recent-{index}"), index as i64),
                crate::store::HistoryPaths {
                    canonical_cwd: "/ephemeral",
                    display_cwd: "/ephemeral",
                },
                Some(&repo),
                None,
            )
            .await
            .unwrap();
    }
    let body = serde_json::json!({ "host": claim.host, "expected_incarnation": claim.incarnation, "query": "acme" });
    assert!(
        harness
            .store
            .resolve_checkout_config(Some(claim.host))
            .await
            .unwrap()
            .root
            .is_none()
    );
    let route = "/api/github-repositories";
    let (status, text) = post_text(&harness, route, body.clone()).await;
    assert_eq!(status, axum::http::StatusCode::OK, "{text}");
    let missing: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(missing["truncated"], true);
    let expected = serde_json::json!([
        { "owner": "acme", "name": "zebra" },
        { "owner": "acme", "name": "bar" }
    ]);
    assert_eq!(
        missing["repos"], expected,
        "missing config retains accepted suggestions"
    );
    assert!(
        missing["scan_error"]
            .as_str()
            .unwrap()
            .contains("checkout-config set-root")
    );
    harness
        .store
        .set_checkout_root(None, "~/target-checkouts")
        .await
        .unwrap();
    let (status, text) = post_text(&harness, route, body.clone()).await;
    assert_eq!(status, axum::http::StatusCode::OK, "{text}");
    let result: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(result["repos"], expected);
    assert_eq!(
        result["truncated"], true,
        "discarded invalid identity makes the result incomplete"
    );
    assert!(result["scan_error"].is_null());
    assert_eq!(result["installation_identity"], "local-identity");
    assert_eq!(result["incarnation"], claim.incarnation);
    assert_eq!(result["host"], claim.host.to_string());
    assert!(!text.contains("secret"));
    let (status, text) = post_text(&harness, route, body.clone()).await;
    assert_eq!(status, axum::http::StatusCode::OK, "{text}");
    let failed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(failed["repos"], expected);
    assert_eq!(failed["truncated"], true);
    assert!(
        failed["scan_error"]
            .as_str()
            .unwrap()
            .contains("verify Git")
    );
    assert!(!text.contains("secret"));
    let mut stale = body;
    stale["expected_incarnation"] = serde_json::json!(claim.incarnation + 1);
    let (status, precondition_headers, text) = post_text_headers(&harness, route, stale).await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{text}");
    assert!(is_stale_precondition(&precondition_headers));
    finished_tx.send(()).unwrap();
    peer.await.unwrap();
}

/// Losing the supervisor must not erase installation-scoped repo recents.
/// The fixture waits until routing actually refuses that host, so success
/// proves the endpoint reads durable intent without a live scan connection.
#[farhelm_testtrace::test]
async fn github_repository_rest_preserves_recents_offline() {
    let (builder, host) = rest_harness::FleetBuilder::new()
        .await
        .ssh(
            "offline.example",
            rest_harness::HostScript {
                identity: Some("repo-installation".into()),
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    harness.await_refreshed(host).await;
    harness
        .store
        .set_checkout_root(None, "/checkouts")
        .await
        .unwrap();
    let repo = farhelm_proto::parse_github_repo("acme/bar").unwrap();
    harness
        .store
        .record_create_history_with_destination(
            host,
            "repo-installation",
            &rest_harness::session("fresh", 1),
            crate::store::HistoryPaths {
                canonical_cwd: "/checkouts/bar-1",
                display_cwd: "/checkouts/bar-1",
            },
            Some(&repo),
            None,
        )
        .await
        .unwrap();
    harness.fleet.take_down(host);
    harness
        .await_state(host, |state| state.phase() == "unreachable-reprobing")
        .await;
    assert!(super::host_client(&harness.state, host).is_err());
    let incarnation = harness.manager.status(host).unwrap().incarnation;
    let (status, text) = post_text(
        &harness,
        "/api/github-repositories",
        serde_json::json!({
            "host": host, "expected_incarnation": incarnation, "query": "acme"
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{text}");
    let response: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(response["repos"], serde_json::json!([repo]));
    assert_eq!(response["installation_identity"], "repo-installation");
    assert_eq!(response["incarnation"], incarnation);
    assert_eq!(response["truncated"], true);
    assert!(response["scan_error"].as_str().unwrap().contains("offline"));
}

/// A3, the preview contract at the helm's REST edge: the composer asks for
/// a preview of `acme/bar` with a root stored UNEXPANDED as `~/work`, and
/// the answer must prove the expansion happened on the SUPERVISOR's home —
/// the scripted peer answers with an absent child of an owned temporary root,
/// deliberately distinct from the stored home-relative spelling —
/// and the helm must pass that through verbatim alongside its OWN
/// authoritative claim (host id + incarnation) and the config revision it
/// resolved. NOTHING may be created by a preview: no mkdir on the helm's
/// fixture tree, and the scripted peer creates nothing either.
///
/// Configuration revision is checked by the helm on unknown creates; the
/// supervisor checks the actual root and occupancy. This preview test pins
/// the stored revision and unexpanded root sent to that supervisor.
#[farhelm_testtrace::test]
async fn a_preview_resolves_the_stored_root_and_passes_the_supervisor_answer_through() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use tower::ServiceExt;

    let remote_home = tempfile::tempdir().unwrap();
    let remote_root = remote_home.path().join("work");
    assert!(!remote_root.exists(), "preview target must start absent");
    let canonical_root = remote_root.to_str().unwrap().to_owned();
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let seen_root = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let seen_revision = std::sync::Arc::new(std::sync::Mutex::new(None::<i64>));
    // Not joined: the peer loops answering previews until the harness drop
    // closes the stream, and the harness keeps the manager connection open
    // by design, so the peer task's exit is not observable here.
    let _peer = tokio::spawn({
        let seen_root = seen_root.clone();
        let seen_revision = seen_revision.clone();
        let canonical_root = canonical_root.clone();
        async move {
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
                    Ok(ControlMsg::GithubCheckoutPreview {
                        req_id,
                        request:
                            farhelm_proto::GithubPreviewRequest {
                                root,
                                config_revision,
                                ..
                            },
                    }) => {
                        *seen_root.lock().unwrap() = root.clone();
                        *seen_revision.lock().unwrap() = config_revision;
                        // The supervisor's answer uses ITS OWN home's
                        // canonical form — deliberately different from
                        // anything the helm could derive locally.
                        writer
                            .write_control(&ControlMsg::GithubCheckoutPreviewed {
                                req_id,
                                preview: farhelm_proto::GithubPreviewResponse {
                                    canonical_root: canonical_root.clone(),
                                    basename: "bar".to_string(),
                                    cwd: canonical_root.clone(),
                                    config_revision: config_revision.unwrap_or(0),
                                    claim_context: farhelm_proto::ClaimContext {
                                        host: String::new(),
                                        incarnation: 0,
                                    },
                                },
                            })
                            .await
                            .unwrap();
                    }
                    _ => return,
                }
            }
        }
    });

    let harness = rest_harness::spliced_helm_listing(
        client_side,
        vec![rest_harness::session("sess-1", 1_700_000_000)],
    )
    .await;
    let local = rest_harness::local_id(&harness.store).await;
    // The stored root is UNEXPANDED — exactly what the CLI stores.
    harness
        .store
        .set_checkout_root(None, "~/work")
        .await
        .expect("store the checkout root");
    let revision = harness
        .store
        .resolve_checkout_config(None)
        .await
        .unwrap()
        .config_revision;

    let app = harness.router();
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/github-checkout-preview")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({
                "host": local,
                "repo": "acme/bar",
                "title": null
            })
            .to_string(),
        ))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // The helm sent the root UNEXPANDED; expansion is the supervisor's job.
    assert_eq!(
        seen_root.lock().unwrap().as_deref(),
        Some("~/work"),
        "the helm must send the stored root unexpanded"
    );
    assert_eq!(
        *seen_revision.lock().unwrap(),
        Some(revision),
        "the binding carries the revision the root was resolved under"
    );
    // The supervisor's answer passes through, and the helm stamps its own
    // authoritative claim over the placeholder.
    assert_eq!(body["canonical_root"], canonical_root);
    assert_eq!(body["basename"], "bar");
    assert_eq!(body["cwd"], canonical_root);
    assert_eq!(body["config_revision"], revision);
    assert_eq!(body["host"], local.to_string());
    assert!(body["incarnation"].as_u64().unwrap() >= 1);
    assert_eq!(body["installation_identity"], "local-identity");
    // The parent is owned and the child was absent before the request, so
    // this oracle cannot fail because of an unrelated directory on the host.
    assert!(
        !remote_root.exists(),
        "a preview must not create the checkout directory"
    );
    // The scripted peer loops answering previews until the harness drop
    // closes the stream; it is NOT joined (the harness keeps the manager
    // connection open by design, so the peer task's exit is not observable
    // here).
}

/// The stale-incarnation half of the preview contract: a preview prepared
/// against one incarnation is refused when the connection's incarnation
/// has moved on, BEFORE any frame reaches the host — the same precondition
/// ordering the browse/create routes use.
#[farhelm_testtrace::test]
async fn a_preview_with_a_stale_incarnation_is_refused_before_any_frame() {
    use tower::ServiceExt;

    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    // Keep the peer alive until the response completes. A fixed silence
    // window can expire during setup under load and cannot prove that the
    // whole request stayed on the helm side of the boundary.
    let (finished_tx, mut finished_rx) = tokio::sync::oneshot::channel();
    let peer = tokio::spawn({
        async move {
            use farhelm_proto::io::{FrameReader, FrameWriter, handshake};
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            tokio::select! {
                biased;
                leaked = reader.read_frame() => {
                    panic!("stale-incarnation preview contacted or lost its peer: {leaked:?}");
                }
                result = &mut finished_rx => result.unwrap(),
            }
        }
    });
    let harness = rest_harness::spliced_helm_listing(
        client_side,
        vec![rest_harness::session("sess-1", 1_700_000_000)],
    )
    .await;
    let local = rest_harness::local_id(&harness.store).await;
    harness
        .store
        .set_checkout_root(None, "~/work")
        .await
        .expect("store the checkout root");

    let app = harness.router();
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/github-checkout-preview")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({
                "host": local,
                "expected_incarnation": 999,
                "repo": "acme/bar"
            })
            .to_string(),
        ))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
    finished_tx.send(()).expect("preview peer must remain live");
    peer.await.expect("preview peer must observe no frame");
}

/// Spec: a repository search the supervisor refused as busy shows the shared
/// busy sentence; every other failure, including an `Unavailable` the helm
/// decided itself, keeps the Git and checkout-root hint.
///
/// Why: the helm never passes a supervisor's error text through for this
/// route, so before the busy case was recognised a search refused on a busy
/// host told the user to check their Git install. Reading only the kind of a
/// supervisor reply, never its text, is what keeps the route's rule against
/// remote text intact.
#[farhelm_testtrace::test]
fn repository_discovery_failure_names_a_busy_host_and_nothing_else() {
    let reply = |origin, kind| {
        anyhow::Error::new(crate::SupervisorError {
            origin,
            kind,
            message: "remote text that must never be shown".to_string(),
        })
        .context("searching repositories on host builder")
    };
    let git_hint = "repository discovery is unavailable on this host; verify Git and the configured checkout root";

    assert_eq!(
        super::repository_discovery_failure(&reply(
            crate::client::ErrorOrigin::SupervisorReply,
            farhelm_proto::ErrorKind::Unavailable
        )),
        farhelm_proto::HOST_BUSY_REFUSAL
    );
    for (origin, kind) in [
        (
            crate::client::ErrorOrigin::SupervisorReply,
            farhelm_proto::ErrorKind::Internal,
        ),
        (
            crate::client::ErrorOrigin::Helm,
            farhelm_proto::ErrorKind::Unavailable,
        ),
    ] {
        assert_eq!(
            super::repository_discovery_failure(&reply(origin, kind)),
            git_hint,
            "{origin:?} {kind:?} is not a busy supervisor"
        );
    }
    assert_eq!(
        super::repository_discovery_failure(&anyhow::anyhow!("connection lost")),
        git_hint
    );
}

/// Spec: the fresh-create request identity of a command-launch create is
/// pinned byte for byte: `github_create_request_v2`, then the cwd, the agent
/// launch choices (null here), the command launch whole, the title, host,
/// expected incarnation and checkout request.
///
/// Why: the identity is the key's binding across a lost reply, and the
/// supervisor compares it by string equality, so any change to its shape
/// turns every in-flight fresh-checkout retry into a conflict. Launch kinds
/// made that change once, deliberately (SPEC_impl.md, "Launch-kinds
/// reservations"); a later edit that reordered or renamed a slot would
/// compile and pass every other test while doing it again by accident.
#[farhelm_testtrace::test]
fn a_command_launch_request_identity_is_pinned() {
    let req: super::CreateReq = serde_json::from_value(serde_json::json!({
        "cwd": "/w",
        "command": {"command": "agent", "yolo": false},
        "title": "t",
    }))
    .expect("a command-launch create body decodes");
    assert_eq!(
        super::fresh_create_request_identity(&req),
        r#"["github_create_request_v2","/w",null,{"command":"agent","yolo":false,"agent":null,"resume":null},"t",null,null,null]"#
    );
}

/// Trash uses the observed host connection and never interprets its paths on
/// the helm. A stale confirmation must be refused before a request reaches the
/// supervisor; the next peer frame distinguishes that from a superficial HTTP
/// error after dispatch.
#[farhelm_testtrace::test]
async fn checkout_trash_routes_pass_host_results_and_fence_stale_deletion() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use tower::ServiceExt;
    let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
    let peer = tokio::spawn(async move {
        let (read, write) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(read);
        let mut writer = FrameWriter::new(write);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::ListCheckoutTrash {
            req_id,
            measure_sizes: true,
        } = request
        else {
            panic!("expected size listing, got {request:?}");
        };
        writer.write_control(&ControlMsg::CheckoutTrashListed {
            req_id,
            listing: farhelm_proto::CheckoutTrashListing {
                checkouts: vec![farhelm_proto::ArchivedCheckout {
                    id: "recorded".into(),
                    name: "project-1-20260101T000000Z".into(),
                    repository: "acme/project".into(),
                    path: "/remote/checkouts/farhelm-archived-working-copies/project-1-20260101T000000Z".into(),
                    archived_at: Some(1767225600),
                    bytes: Some(4096),
                }],
                issues: vec![],
            },
        }).await.unwrap();
        let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::DeleteCheckoutTrash { req_id, ids } = request else {
            panic!("expected deletion, got {request:?}");
        };
        assert_eq!(
            ids,
            vec!["recorded"],
            "stale deletion must never reach the peer"
        );
        writer
            .write_control(&ControlMsg::CheckoutTrashDeleted {
                req_id,
                deleted: farhelm_proto::CheckoutTrashDeleted {
                    removed: ids,
                    issues: vec![],
                },
            })
            .await
            .unwrap();
    });
    let harness = rest_harness::spliced_helm(client_side).await;
    let local = rest_harness::local_id(&harness.store).await;
    for (endpoint, body, status) in [
        (
            "list",
            serde_json::json!({"host":local,"measure_sizes":true}),
            axum::http::StatusCode::OK,
        ),
        (
            "delete",
            serde_json::json!({"host":local,"expected_incarnation":u64::MAX,"ids":["stale"]}),
            axum::http::StatusCode::CONFLICT,
        ),
        // Missing selection is a malformed request, not an emptying success.
        // The next accepted peer frame also proves it was never dispatched.
        (
            "delete",
            serde_json::json!({"host":local,"id":["misspelled"]}),
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "delete",
            serde_json::json!({"host":local,"ids":["recorded"]}),
            axum::http::StatusCode::OK,
        ),
    ] {
        let request = axum::http::Request::builder()
            .method("POST")
            .uri(format!("/api/checkout-trash/{endpoint}"))
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap();
        let response = harness.router().oneshot(request).await.unwrap();
        assert_eq!(response.status(), status);
        let bytes = axum::body::to_bytes(response.into_body(), 65536)
            .await
            .unwrap();
        if status == axum::http::StatusCode::OK {
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            if endpoint == "list" {
                assert_eq!(value["checkouts"][0]["bytes"], 4096);
                assert_eq!(value["checkouts"][0]["repository"], "acme/project");
            } else {
                assert_eq!(value["removed"], serde_json::json!(["recorded"]));
            }
        }
    }
    peer.await.unwrap();
}

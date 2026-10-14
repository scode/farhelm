//! Policy proofs use an injected clock; outbound proofs use owned loopback
//! servers and request-arrival oracles. No test changes process configuration.

use super::*;
use crate::rest_harness::{FleetBuilder, Harness, HostScript, session};
use axum::{
    body::Body,
    http::{HeaderMap, Request},
};
use farhelm_proto::ntfy::Priority;
use std::sync::Mutex;
use tokio::sync::watch;
use tower::ServiceExt;

const TOPIC: &str = "private-topic-marker";
const TOKEN: &str = "private-token-marker";
const TITLE: &str = "private-title-marker";

/// Pure evidence with a distinct title makes accidental id/body substitution visible.
fn observation(id: &str, status: SessionStatus) -> Observation {
    Observation {
        id: id.into(),
        title: format!("title {id}"),
        status,
    }
}

/// A restart/reconnect suppresses sessions already waiting, while a new session
/// first appearing after that baseline still gets one settled alert.
#[farhelm_testtrace::test]
fn baseline_and_new_waiting_session() {
    let now = Instant::now();
    let mut policy = Policy::default();
    let baseline = observation("old", SessionStatus::Waiting);
    assert!(policy.observe(&[baseline], now).is_empty());
    assert!(
        policy
            .observe(&[observation("old", SessionStatus::Waiting)], now + SETTLE)
            .is_empty()
    );
    let list = [
        observation("old", SessionStatus::Waiting),
        observation("new", SessionStatus::Waiting),
    ];
    assert!(policy.observe(&list, now + SETTLE).is_empty());
    assert_eq!(policy.observe(&list, now + SETTLE * 2), vec!["title new"]);
    assert!(policy.observe(&list, now + SETTLE * 3).is_empty());
    assert!(
        Policy::default()
            .observe(&list, now + SETTLE * 4)
            .is_empty()
    );
}

/// Flickering readings never alert; settled waiting alerts exactly once, and
/// leaving/re-entering starts a separate spell with its own settle boundary.
#[farhelm_testtrace::test]
fn settle_flicker_and_reentry() {
    let now = Instant::now();
    let mut policy = Policy::default();
    let running = [observation("one", SessionStatus::Running)];
    let waiting = [observation("one", SessionStatus::Waiting)];
    assert!(policy.observe(&running, now).is_empty());
    assert!(policy.observe(&waiting, now).is_empty());
    assert!(
        policy
            .observe(&waiting, now + SETTLE - Duration::from_nanos(1))
            .is_empty()
    );
    assert!(policy.observe(&running, now + SETTLE).is_empty());
    assert!(policy.observe(&waiting, now + SETTLE).is_empty());
    assert_eq!(
        policy.observe(&waiting, now + SETTLE * 2),
        vec!["title one"]
    );
    assert!(policy.observe(&waiting, now + SETTLE * 3).is_empty());
    assert!(policy.observe(&running, now + SETTLE * 3).is_empty());
    assert!(policy.observe(&waiting, now + SETTLE * 3).is_empty());
    assert_eq!(
        policy.observe(&waiting, now + SETTLE * 4),
        vec!["title one"]
    );
}

/// Unknown is absence of new evidence, including at the settle boundary; exited
/// and removed sessions cancel pending spells rather than leaving delayed work.
#[farhelm_testtrace::test]
fn unknown_exit_and_removal() {
    let now = Instant::now();
    let mut policy = Policy::default();
    policy.observe(&[], now);
    policy.observe(&[observation("one", SessionStatus::Waiting)], now);
    assert!(
        policy
            .observe(&[observation("one", SessionStatus::Unknown)], now + SETTLE)
            .is_empty()
    );
    assert_eq!(
        policy.observe(&[observation("one", SessionStatus::Waiting)], now + SETTLE),
        vec!["title one"]
    );
    policy.observe(&[observation("two", SessionStatus::Waiting)], now + SETTLE);
    policy.observe(
        &[observation(
            "two",
            SessionStatus::Exited { exit_code: Some(0) },
        )],
        now + SETTLE * 2,
    );
    assert!(
        policy
            .observe(
                &[observation("two", SessionStatus::Waiting)],
                now + SETTLE * 2
            )
            .is_empty()
    );
    policy.observe(&[], now + SETTLE * 3);
    assert!(
        policy
            .observe(
                &[observation("two", SessionStatus::Waiting)],
                now + SETTLE * 4
            )
            .is_empty()
    );
}

/// The fleet window drops excess immediately and carries its count only into
/// the next admitted alert. Both the minute boundary and one-log rule are pinned.
#[farhelm_testtrace::test]
fn fleet_rate_boundary_and_count() {
    let capture = crate::test_capture::current();
    let now = Instant::now();
    let mut window = RateWindow::new(now);
    for _ in 0..PER_MINUTE {
        assert_eq!(window.admit(now), Some(0));
    }
    assert_eq!(window.admit(now), None);
    assert_eq!(window.admit(now + Duration::from_secs(59)), None);
    assert_eq!(
        crate::test_capture::matching(&capture, "rate cap reached").len(),
        1
    );
    assert_eq!(window.admit(now + Duration::from_secs(60)), Some(2));
    for _ in 1..PER_MINUTE {
        assert_eq!(window.admit(now + Duration::from_secs(60)), Some(0));
    }
    assert_eq!(window.admit(now + Duration::from_secs(60)), None);
    assert_eq!(
        crate::test_capture::matching(&capture, "rate cap reached").len(),
        2
    );
}

/// Body fields are free of controls and of the invisible or reordering
/// characters the shared presentation list names, and capped by Unicode
/// characters, not bytes; a folder-derived title is preserved rather than
/// classified as a forbidden path. The bidirectional override matters because
/// an agent sets its own title, and an override in it could make the phone's
/// alert read as if it named another host.
#[farhelm_testtrace::test]
fn titles_are_bounded_without_rewriting_their_meaning() {
    assert_eq!(clean_text("a\n\r\t\u{7f}b"), "ab");
    assert_eq!(clean_text("a\u{202E}b\u{200B}c\u{2066}d"), "abcd");
    assert_eq!(clean_text("café ❤️"), "café ❤️");
    assert_eq!(clean_text(&"界".repeat(300)).chars().count(), 200);
    assert_eq!(clean_text("/project/work"), "/project/work");
}

/// Each request is observed whole before the stand-in decides how to answer.
#[derive(Clone)]
struct Received {
    method: String,
    path: String,
    headers: HeaderMap,
    body: String,
}
#[derive(Clone, Copy)]
/// Response branches distinguish accepted bytes, untrusted refusal bodies,
/// a forbidden redirect and a request whose completion requires timeout.
enum Answer {
    Success,
    Refuse,
    Redirect,
    Hang,
}

/// An owned endpoint with an arrival oracle; Drop aborts the server on failures
/// too, so a refused or stalled send cannot leave a fixture listener behind.
struct StandIn {
    server_url: String,
    received: Arc<Mutex<Vec<Received>>>,
    arrivals: watch::Receiver<usize>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for StandIn {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl StandIn {
    /// Wait for actual request bytes, rather than treating a completed client
    /// spawn or elapsed delay as proof the server entered its stalled branch.
    async fn arrived(&mut self, count: usize) {
        tokio::time::timeout(
            Duration::from_secs(10),
            self.arrivals.wait_for(|n| *n >= count),
        )
        .await
        .expect("stand-in did not receive request")
        .expect("server arrival channel closed");
        assert!(
            !self.task.is_finished(),
            "stand-in died after recording a request"
        );
    }
}

/// Port 0 isolates the test from other executors. Refusals deliberately contain
/// private text in their body so a failure proof detects accidental body echoing.
async fn stand_in(answer: Answer) -> StandIn {
    let received: Arc<Mutex<Vec<Received>>> = Arc::default();
    let record = received.clone();
    let (arrivals, watch_arrivals) = watch::channel(0usize);
    let app = axum::Router::new().fallback(move |request: axum::extract::Request| {
        let record = record.clone();
        let arrivals = arrivals.clone();
        async move {
            let method = request.method().to_string();
            let path = request.uri().path().to_string();
            let headers = request.headers().clone();
            let body = axum::body::to_bytes(request.into_body(), 8192)
                .await
                .unwrap();
            let count = {
                let mut record = record.lock().unwrap();
                record.push(Received {
                    method,
                    path,
                    headers,
                    body: String::from_utf8(body.to_vec()).unwrap(),
                });
                record.len()
            };
            arrivals.send_replace(count);
            match answer {
                Answer::Success => StatusCode::OK.into_response(),
                Answer::Refuse => {
                    (StatusCode::FORBIDDEN, format!("{TOPIC} {TOKEN} {TITLE}")).into_response()
                }
                Answer::Redirect => {
                    (StatusCode::TEMPORARY_REDIRECT, [("location", "/elsewhere")]).into_response()
                }
                Answer::Hang => std::future::pending::<Response>().await,
            }
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    StandIn {
        server_url: format!("http://{addr}/prefix"),
        received,
        arrivals: watch_arrivals,
        task,
    }
}

/// No proxy or ambient configuration participates in loopback endpoint proofs.
fn client() -> reqwest::Client {
    client_builder().no_proxy().build().unwrap()
}
/// A realistic configuration with distinct private markers; changing only one
/// field lets tests separate saved state from an unsaved request draft.
fn save(server: &str, token: Option<&str>, enabled: bool) -> SaveSettings {
    SaveSettings {
        enabled,
        server_url: server.into(),
        topic: TOPIC.into(),
        priority: Priority::Urgent,
        token: token.map(str::to_string),
    }
}
/// Offline host actors plus an explicitly injected outbound client keep route
/// proofs independent of any live session or user notification subscription.
async fn helm() -> Harness {
    FleetBuilder::new()
        .await
        .ntfy_client(client())
        .start()
        .await
}
/// Exercise the real protected router with the harness's accepted Host header.
/// JSON bodies are built as values so malformed-field tests vary types safely.
fn request(method: &str, path: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
/// Bound retained API text independently of the server's own body limit.
async fn text(response: Response) -> String {
    String::from_utf8(
        axum::body::to_bytes(response.into_body(), 16384)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

/// The API preserves, replaces and clears a token without ever returning it.
/// Reopening the database proves this is helm configuration, not only memory.
#[farhelm_testtrace::test]
async fn settings_secret_lifecycle_and_persistence() {
    let harness = helm().await;
    let before = text(
        harness
            .router()
            .oneshot(request("GET", "/api/ntfy-alerts", serde_json::Value::Null))
            .await
            .unwrap(),
    )
    .await;
    assert!(before.contains("https://ntfy.sh") && before.contains("\"enabled\":false"));
    for (replacement, present) in [(Some(TOKEN), true), (None, true), (Some(""), false)] {
        let body = serde_json::to_value(save("https://ntfy.sh", replacement, true)).unwrap();
        let response = harness
            .router()
            .oneshot(request("PUT", "/api/ntfy-alerts", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = text(response).await;
        assert!(!body.contains(TOKEN));
        assert_eq!(
            serde_json::from_str::<Settings>(&body).unwrap().token_set,
            present
        );
        let config = harness.store.ntfy_config().await.unwrap();
        assert_eq!(
            config.token.as_deref(),
            if present { Some(TOKEN) } else { None }
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("helm.db");
    let store = HelmStore::open(&path).await.unwrap();
    store
        .save_ntfy_config(save("https://ntfy.sh", Some(TOKEN), true))
        .await
        .ok()
        .unwrap();
    drop(store);
    let reopened = HelmStore::open(&path).await.unwrap();
    assert_eq!(
        reopened.ntfy_config().await.unwrap().token.as_deref(),
        Some(TOKEN)
    );
}

/// Validation and malformed JSON use fixed refusals even when a secret was
/// submitted where serde expects another type. No failed save changes settings.
#[farhelm_testtrace::test]
async fn validation_never_echoes_values_or_changes_saved_settings() {
    let harness = helm().await;
    let mut cases = Vec::new();
    for url in [
        "bad URL",
        "file:///secret",
        "https://user:password@example.invalid",
        "https://example.invalid/?token=secret",
    ] {
        cases.push(serde_json::to_value(save(url, Some(TOKEN), true)).unwrap());
    }
    for topic in ["bad/topic".to_string(), "x".repeat(65), String::new()] {
        let mut value = serde_json::to_value(save("https://ntfy.sh", Some(TOKEN), true)).unwrap();
        value["topic"] = topic.into();
        cases.push(value);
    }
    for (key, value) in [
        ("priority", serde_json::json!(TOKEN)),
        ("enabled", serde_json::json!(TOKEN)),
        ("token", serde_json::json!({"secret":TOKEN})),
    ] {
        let mut body = serde_json::to_value(save("https://ntfy.sh", None, false)).unwrap();
        body[key] = value;
        cases.push(body);
    }
    cases.push(serde_json::to_value(save("https://ntfy.sh", Some("bad\ntoken"), false)).unwrap());
    for body in cases {
        let response = harness
            .router()
            .oneshot(request("PUT", "/api/ntfy-alerts", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = text(response).await;
        assert!(!body.contains(TOKEN) && !body.contains(TOPIC));
        assert!(!harness.store.ntfy_config().await.unwrap().settings.enabled);
    }
}

/// Send test uses the saved settings, succeeds while the switch is off, and
/// sends the expected method/path/headers/body without returning the token.
#[farhelm_testtrace::test]
async fn test_send_request_shape_and_saved_secret() {
    let mut server = stand_in(Answer::Success).await;
    let harness = helm().await;
    harness
        .store
        .save_ntfy_config(save(&server.server_url, Some(TOKEN), false))
        .await
        .ok()
        .unwrap();
    let response = harness
        .router()
        .oneshot(request(
            "POST",
            "/api/ntfy-alerts/test",
            serde_json::Value::Null,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    server.arrived(1).await;
    let received = server.received.lock().unwrap()[0].clone();
    assert_eq!(received.method, "POST");
    assert_eq!(received.path, format!("/prefix/{TOPIC}"));
    assert_eq!(received.headers["title"], "Farhelm");
    assert_eq!(received.headers["priority"], "urgent");
    assert_eq!(received.headers["authorization"], format!("Bearer {TOKEN}"));
    assert_eq!(received.body, "Farhelm ntfy alerts are working");
    harness
        .store
        .save_ntfy_config(save(&server.server_url, Some(""), false))
        .await
        .ok()
        .unwrap();
    assert!(harness.state.ntfy.test().await.is_ok());
    server.arrived(2).await;
    assert!(
        !server.received.lock().unwrap()[1]
            .headers
            .contains_key("authorization")
    );
}

/// An ntfy refusal body is untrusted and a redirect is not permission to send
/// elsewhere. Both produce one fixed failure and exactly one server request.
#[farhelm_testtrace::test]
async fn refusal_and_redirect_do_not_leak_or_retry() {
    let capture = crate::test_capture::current();
    for (answer, code) in [(Answer::Refuse, 403), (Answer::Redirect, 307)] {
        let mut server = stand_in(answer).await;
        let harness = helm().await;
        harness
            .store
            .save_ntfy_config(save(&server.server_url, Some(TOKEN), true))
            .await
            .ok()
            .unwrap();
        let response = harness
            .router()
            .oneshot(request(
                "POST",
                "/api/ntfy-alerts/test",
                serde_json::Value::Null,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            text(response).await,
            format!("the ntfy server refused it (HTTP {code})")
        );
        server.arrived(1).await;
        assert_eq!(server.received.lock().unwrap().len(), 1);
    }
    assert!(
        capture
            .matching_events(
                |event| event
                    .fields
                    .values()
                    .chain(event.span_fields.values())
                    .any(|value| [TOKEN, TOPIC, TITLE]
                        .iter()
                        .any(|secret| value.contains(secret)))
            )
            .expect("complete trace evidence")
            .is_empty()
    );
}

/// A reserved but non-listening port proves a connect refusal, not a server
/// response. URL-bearing reqwest errors never expose the private topic.
#[farhelm_testtrace::test]
async fn connection_failure_is_secret_safe() {
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let server = format!("http://{}", socket.local_addr().unwrap());
    let harness = helm().await;
    harness
        .store
        .save_ntfy_config(save(&server, Some(TOKEN), true))
        .await
        .ok()
        .unwrap();
    assert_eq!(
        harness.state.ntfy.test().await.unwrap_err(),
        "the ntfy server could not be reached"
    );
    drop(socket);
}

/// A slow server cannot block enqueue or permit unbounded pending work. The
/// request-arrival oracle establishes the sender is stalled before filling the
/// queue; an injected Tokio clock then exercises the production short timeout.
#[farhelm_testtrace::test]
async fn stalled_sender_queue_bound_and_timeout() {
    let capture = crate::test_capture::current();
    let mut server = stand_in(Answer::Hang).await;
    let harness = helm().await;
    harness
        .store
        .save_ntfy_config(save(&server.server_url, Some(TOKEN), true))
        .await
        .ok()
        .unwrap();
    let host = harness.store.list_hosts().await.unwrap()[0].id;
    harness.state.ntfy.enqueue(TITLE.into(), host);
    server.arrived(1).await;
    assert_eq!(
        server.received.lock().unwrap()[0].body,
        format!("{TITLE} on this machine is waiting for you")
    );
    for _ in 0..QUEUE_CAPACITY {
        harness.state.ntfy.enqueue(TITLE.into(), host);
    }
    assert_eq!(harness.state.ntfy.0.queue.capacity(), 0);
    harness.state.ntfy.enqueue(TITLE.into(), host);
    assert_eq!(
        crate::test_capture::matching(&capture, "queue unavailable or full").len(),
        1
    );
    let direct = harness.state.ntfy.clone();
    let send = tokio::spawn(async move { direct.test().await });
    server.arrived(2).await;
    tokio::time::pause();
    tokio::time::advance(SEND_TIMEOUT + Duration::from_secs(1)).await;
    assert_eq!(
        send.await.unwrap().unwrap_err(),
        "the ntfy server did not answer in time"
    );
    tokio::time::resume();
    assert!(
        capture
            .matching_events(
                |event| event
                    .fields
                    .values()
                    .chain(event.span_fields.values())
                    .any(|value| [TOKEN, TOPIC, TITLE]
                        .iter()
                        .any(|secret| value.contains(secret)))
            )
            .expect("complete trace evidence")
            .is_empty()
    );
}

/// A real host actor feeds accepted lists to the policy for both durable and
/// identity-less hosts. Waiting settles on refresh evidence, not the durable
/// cache; flicker cancels it, Unknown cannot finish it, and one spell posts once.
#[farhelm_testtrace::test]
async fn manager_observations_settle_without_flicker_or_unknown_alerts() {
    for identity in [Some("ntfy-host".to_string()), None] {
        let mut server = stand_in(Answer::Success).await;
        let harness = FleetBuilder::new()
            .await
            .ntfy_client(client())
            .local(HostScript {
                identity: identity.clone(),
                sessions: vec![session("one", 1)],
                ..HostScript::default()
            })
            .await
            .start()
            .await;
        let host = harness.store.list_hosts().await.unwrap()[0].id;
        harness.await_refreshed(host).await;
        harness
            .store
            .save_ntfy_config(save(&server.server_url, None, true))
            .await
            .ok()
            .unwrap();
        let change = |status: SessionStatus| {
            harness.fleet.edit(host, |script| {
                script.sessions[0].status = status;
            })
        };
        change(SessionStatus::Waiting);
        harness.refresh_to_completion(host).await;
        change(SessionStatus::Running);
        harness.refresh_to_completion(host).await;
        tokio::time::pause();
        tokio::time::advance(SETTLE).await;
        tokio::time::resume();
        harness.refresh_to_completion(host).await;
        assert!(
            server.received.lock().unwrap().is_empty(),
            "flickering spell sent an alert"
        );
        change(SessionStatus::Waiting);
        harness.refresh_to_completion(host).await;
        // The helper proves a prior refresh completed by starting its successor.
        // Make that successor Unknown before advancing time: an already-drained
        // Waiting reply would otherwise legitimately settle during the clock jump.
        change(SessionStatus::Unknown);
        harness.refresh_to_completion(host).await;
        tokio::time::pause();
        tokio::time::advance(SETTLE).await;
        tokio::time::resume();
        harness.refresh_to_completion(host).await;
        assert!(
            server.received.lock().unwrap().is_empty(),
            "retained cache status fabricated Waiting evidence"
        );
        change(SessionStatus::Waiting);
        // A forbidden earlier enqueue must not masquerade as this allowed send.
        // The single sender preserves FIFO, and titles are captured at enqueue.
        harness.fleet.edit(host, |script| {
            script.sessions[0].title = "allowed final waiting".into()
        });
        harness.refresh_to_completion(host).await;
        server.arrived(1).await;
        assert_eq!(
            server.received.lock().unwrap()[0].body,
            "allowed final waiting on this machine is waiting for you"
        );
        harness.refresh_to_completion(host).await;
        assert_eq!(server.received.lock().unwrap().len(), 1);
    }
}

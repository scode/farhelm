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

/// The session the delivery tests' alerts are about. Only its key matters to the sender; the alert names the title.
const SESSION: &str = "sess-alerted";

/// A waiting spell of [`SESSION`] titled [`TITLE`] that settled just now, for queueing a waiting alert directly.
fn settled_now() -> Settled {
    Settled {
        id: SESSION.into(),
        title: TITLE.into(),
        since: Instant::now(),
    }
}

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
    assert!(policy.observe(&[baseline], now, true).is_empty());
    assert!(
        policy
            .observe(
                &[observation("old", SessionStatus::Waiting)],
                now + SETTLE,
                true
            )
            .is_empty()
    );
    let list = [
        observation("old", SessionStatus::Waiting),
        observation("new", SessionStatus::Waiting),
    ];
    assert!(policy.observe(&list, now + SETTLE, true).is_empty());
    assert_eq!(
        policy.observe(&list, now + SETTLE * 2, true),
        vec!["title new"]
    );
    assert!(policy.observe(&list, now + SETTLE * 3, true).is_empty());
    assert!(
        Policy::default()
            .observe(&list, now + SETTLE * 4, true)
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
    assert!(policy.observe(&running, now, true).is_empty());
    assert!(policy.observe(&waiting, now, true).is_empty());
    assert!(
        policy
            .observe(&waiting, now + SETTLE - Duration::from_nanos(1), true)
            .is_empty()
    );
    assert!(policy.observe(&running, now + SETTLE, true).is_empty());
    assert!(policy.observe(&waiting, now + SETTLE, true).is_empty());
    assert_eq!(
        policy.observe(&waiting, now + SETTLE * 2, true),
        vec!["title one"]
    );
    assert!(policy.observe(&waiting, now + SETTLE * 3, true).is_empty());
    assert!(policy.observe(&running, now + SETTLE * 3, true).is_empty());
    assert!(policy.observe(&waiting, now + SETTLE * 3, true).is_empty());
    assert_eq!(
        policy.observe(&waiting, now + SETTLE * 4, true),
        vec!["title one"]
    );
}

/// The deadline the host loop schedules a refresh for is the earliest pending
/// spell's settling time, and there is none before the baseline, once every
/// spell has alerted, once waiting ends, or while waiting alerts are off. Why
/// it matters: with the backstop poll at thirty seconds, this deadline is what
/// keeps the alert at SPEC.md's three seconds; a deadline that stayed after its
/// spell was handled, or with alerts off, would make the loop refresh for
/// nothing.
#[farhelm_testtrace::test]
fn next_deadline_is_the_earliest_pending_spell_settling_time() {
    let now = Instant::now();
    let mut policy = Policy::default();
    assert_eq!(policy.next_deadline(), None, "nothing observed yet");
    let baseline = [observation("old", SessionStatus::Waiting)];
    assert!(policy.observe(&baseline, now, true).is_empty());
    assert_eq!(
        policy.next_deadline(),
        None,
        "a baseline spell is already handled"
    );

    let waiting = |id: &str| observation(id, SessionStatus::Waiting);
    assert!(policy.observe(&[waiting("first")], now, true).is_empty());
    let later = now + SETTLE / 3;
    assert!(
        policy
            .observe(&[waiting("first"), waiting("second")], later, true)
            .is_empty()
    );
    assert_eq!(
        policy.next_deadline(),
        Some(now + SETTLE),
        "the earliest spell"
    );

    assert_eq!(
        policy.observe(&[waiting("first"), waiting("second")], now + SETTLE, true),
        vec!["title first"]
    );
    assert_eq!(
        policy.next_deadline(),
        Some(later + SETTLE),
        "the next spell"
    );

    let resolved = [
        waiting("first"),
        observation("second", SessionStatus::Running),
    ];
    assert!(
        policy
            .observe(&resolved, now + SETTLE + Duration::from_secs(1), true)
            .is_empty()
    );
    assert_eq!(
        policy.next_deadline(),
        None,
        "a spell that ended has no deadline"
    );

    // With waiting alerts off the policy keeps no spells, so no deadline
    // wakes the host for an alert that would not be sent.
    let fresh = now + SETTLE * 4;
    assert!(
        policy
            .observe(&[waiting("first"), waiting("third")], fresh, true)
            .is_empty()
    );
    assert!(policy.next_deadline().is_some(), "premise: a pending spell");
    assert!(policy.observe(&[waiting("third")], fresh, false).is_empty());
    assert_eq!(policy.next_deadline(), None, "alerts off: no deadline");
}

/// The host loop arms its wake on the earliest deadline after the refresh it
/// just ran. A spell whose session went on to read Unknown stays pending past
/// its deadline (Unknown adds no evidence), and if that overdue deadline were
/// taken as the minimum and then discarded, a second session's fresh spell
/// would get no wake and its alert would slip to the thirty-second poll.
#[farhelm_testtrace::test]
fn an_overdue_spell_does_not_hide_a_later_deadline() {
    let now = Instant::now();
    let mut policy = Policy::default();
    assert!(policy.observe(&[], now, true).is_empty(), "empty baseline");
    let stuck_since = now + Duration::from_secs(1);
    assert!(
        policy
            .observe(
                &[observation("stuck", SessionStatus::Waiting)],
                stuck_since,
                true
            )
            .is_empty()
    );
    let refreshed = stuck_since + SETTLE + Duration::from_secs(5);
    let fleet = [
        observation("stuck", SessionStatus::Unknown),
        observation("fresh", SessionStatus::Waiting),
    ];
    assert!(policy.observe(&fleet, refreshed, true).is_empty());
    assert_eq!(
        policy.next_deadline(),
        Some(stuck_since + SETTLE),
        "premise: the Unknown session's spell is still pending and overdue"
    );
    assert_eq!(
        policy.next_deadline_after(refreshed),
        Some(refreshed + SETTLE),
        "the fresh spell's deadline is armed despite the overdue one"
    );
    assert_eq!(
        policy.next_deadline_after(refreshed + SETTLE),
        None,
        "a deadline at the bound itself is not after it"
    );
}

/// Unknown is absence of new evidence, including at the settle boundary; exited
/// and removed sessions cancel pending spells rather than leaving delayed work.
#[farhelm_testtrace::test]
fn unknown_exit_and_removal() {
    let now = Instant::now();
    let mut policy = Policy::default();
    policy.observe(&[], now, true);
    policy.observe(&[observation("one", SessionStatus::Waiting)], now, true);
    assert!(
        policy
            .observe(
                &[observation("one", SessionStatus::Unknown)],
                now + SETTLE,
                true
            )
            .is_empty()
    );
    assert_eq!(
        policy.observe(
            &[observation("one", SessionStatus::Waiting)],
            now + SETTLE,
            true
        ),
        vec!["title one"]
    );
    policy.observe(
        &[observation("two", SessionStatus::Waiting)],
        now + SETTLE,
        true,
    );
    policy.observe(
        &[observation(
            "two",
            SessionStatus::Exited { exit_code: Some(0) },
        )],
        now + SETTLE * 2,
        true,
    );
    assert!(
        policy
            .observe(
                &[observation("two", SessionStatus::Waiting)],
                now + SETTLE * 2,
                true
            )
            .is_empty()
    );
    policy.observe(&[], now + SETTLE * 3, true);
    assert!(
        policy
            .observe(
                &[observation("two", SessionStatus::Waiting)],
                now + SETTLE * 4,
                true
            )
            .is_empty()
    );
}

/// With alerts off, spells used to keep settling and be marked handled while the sender dropped their alerts. Off now
/// keeps no evidence, and turning alerts on is a fresh baseline, as after a reconnect (the plan
/// `agent-blocked-notifications`). Specified: a session that waits past the settle time while alerts are off returns
/// nothing; once they are on, the first listing is the baseline (the session still waiting gets no alert for this
/// wait), a session that starts waiting after it alerts once it has waited the settle time, and the first session
/// alerts again after it stops waiting and waits anew.
#[farhelm_testtrace::test]
fn alerts_off_keep_no_evidence_and_turning_them_on_is_a_fresh_baseline() {
    let now = Instant::now();
    let mut policy = Policy::default();
    let waiting = [observation("one", SessionStatus::Waiting)];
    assert!(policy.observe(&[], now, true).is_empty());
    assert!(policy.observe(&waiting, now, false).is_empty());
    assert!(policy.observe(&waiting, now + SETTLE * 2, false).is_empty());
    assert!(policy.observe(&waiting, now + SETTLE * 3, true).is_empty());
    assert!(
        policy.observe(&waiting, now + SETTLE * 5, true).is_empty(),
        "the wait under way when alerts came on is the baseline's"
    );
    let both = [
        observation("one", SessionStatus::Waiting),
        observation("two", SessionStatus::Waiting),
    ];
    assert!(policy.observe(&both, now + SETTLE * 5, true).is_empty());
    assert_eq!(
        policy.observe(&both, now + SETTLE * 6, true),
        vec!["title two"]
    );
    let one_running = [
        observation("one", SessionStatus::Running),
        observation("two", SessionStatus::Waiting),
    ];
    assert!(
        policy
            .observe(&one_running, now + SETTLE * 6, true)
            .is_empty()
    );
    assert!(policy.observe(&both, now + SETTLE * 7, true).is_empty());
    assert_eq!(
        policy.observe(&both, now + SETTLE * 8, true),
        vec!["title one"]
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
///
/// No grace period, so an alert goes out as soon as the sender takes it: the
/// delivery tests here wait on a real stand-in server and must not also wait
/// out ten seconds. The grace period has tests of its own.
fn save(server: &str, token: Option<&str>, enabled: bool) -> SaveSettings {
    SaveSettings {
        notify_waiting: Some(true),
        notify_blocked: Some(true),
        custom_blocked_criteria: None,
        ask_agents_when_blocked: None,
        grace_period_secs: Some(0),
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

/// The host refresh decides whether to track waiting spells from the store's cache, never from the database, so the
/// cache must follow every save and survive a reopen; and a client older than the waiting switch saves without it, which
/// must not turn waiting alerts back on. Specified: a fresh store reads off; saving alerts on with the waiting switch on
/// reads on; the same with the waiting switch off reads off; a save without the switch stores it still off and reads
/// off; on again, then a reopen of the same database reads on before anything else is asked.
#[farhelm_testtrace::test]
async fn the_waiting_cache_follows_saves_and_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("helm.db");
    let store = HelmStore::open(&path).await.unwrap();
    assert!(!store.waiting_alerts_enabled());
    store
        .save_ntfy_config(save("https://ntfy.sh", None, true))
        .await
        .ok()
        .unwrap();
    assert!(store.waiting_alerts_enabled());
    let mut waiting_off = save("https://ntfy.sh", None, true);
    waiting_off.notify_waiting = Some(false);
    store.save_ntfy_config(waiting_off).await.ok().unwrap();
    assert!(!store.waiting_alerts_enabled());
    let mut older_client = save("https://ntfy.sh", None, true);
    older_client.notify_waiting = None;
    let kept = store.save_ntfy_config(older_client).await.ok().unwrap();
    assert!(
        !kept.notify_waiting && !store.waiting_alerts_enabled(),
        "a save without the switch keeps it off"
    );
    store
        .save_ntfy_config(save("https://ntfy.sh", None, true))
        .await
        .ok()
        .unwrap();
    drop(store);
    let reopened = HelmStore::open(&path).await.unwrap();
    assert!(reopened.waiting_alerts_enabled());
}

/// Custom criteria follow the same save rules as the switches, plus one of their own: emptying the field is how the
/// user goes back to Farhelm's wording, and what is sent to agents is what is stored. Specified: a save with criteria
/// text stores it normalized; a save without the field keeps it; a save with whitespace-only text goes back to the
/// default (none stored); text over the cap is refused with the fixed message and changes nothing.
#[farhelm_testtrace::test]
async fn custom_criteria_save_keep_reset_and_refuse() {
    let dir = tempfile::tempdir().unwrap();
    let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
    let with = |criteria: Option<&str>| SaveSettings {
        custom_blocked_criteria: criteria.map(str::to_owned),
        ..save("https://ntfy.sh", None, true)
    };
    let stored = store
        .save_ntfy_config(with(Some("  the build\n is red ")))
        .await
        .ok()
        .unwrap();
    assert_eq!(
        stored.custom_blocked_criteria.as_deref(),
        Some("the build is red")
    );
    let kept = store.save_ntfy_config(with(None)).await.ok().unwrap();
    assert_eq!(
        kept.custom_blocked_criteria.as_deref(),
        Some("the build is red")
    );
    let Err(SaveError::Invalid(reason)) = store
        .save_ntfy_config(with(Some(
            &"x".repeat(farhelm_proto::ntfy::BLOCKED_CRITERIA_CAP + 1),
        )))
        .await
    else {
        panic!("over-long criteria were saved");
    };
    assert!(reason.contains("1000 characters"), "{reason}");
    let reset = store
        .save_ntfy_config(with(Some(" \n ")))
        .await
        .ok()
        .unwrap();
    assert_eq!(reset.custom_blocked_criteria, None);
}

/// Each create and Restart sends the blocked-agent instruction the saved settings call for at that moment, so a save
/// reaches the next session without touching running ones. Specified: on a helm with blocked alerts off, a launch
/// sends no instructions; after saving them on with custom criteria, it sends `blocked_instructions` of the stored
/// settings, ending with those criteria.
#[farhelm_testtrace::test]
async fn a_launch_sends_the_blocked_instruction_the_saved_settings_call_for() {
    let harness = helm().await;
    let mut off = save("https://ntfy.sh", None, true);
    off.notify_blocked = Some(false);
    harness.store.save_ntfy_config(off).await.ok().unwrap();
    assert_eq!(
        crate::sessions::launch_instructions(&harness.state).await,
        None
    );
    let on = SaveSettings {
        custom_blocked_criteria: Some("the build is red".into()),
        ..save("https://ntfy.sh", None, true)
    };
    let stored = harness.store.save_ntfy_config(on).await.ok().unwrap();
    let sent = crate::sessions::launch_instructions(&harness.state)
        .await
        .expect("instructions while blocked alerts are on");
    assert_eq!(
        Some(sent.clone()),
        farhelm_proto::ntfy::blocked_instructions(&stored)
    );
    assert!(
        sent.ends_with("What counts as blocked: the build is red"),
        "{sent}"
    );
}

/// "Ask agents to notify when blocked" works with or without ntfy, and an upgraded configuration keeps asking exactly
/// when it did before the choice had its own switch. Specified: a configuration saved with ntfy and its blocked switch
/// on but no choice of its own sends the instruction; a save that leaves the choice out keeps it; asking on with ntfy
/// alerts off still sends it; asking off with ntfy's blocked alerts on sends none.
#[farhelm_testtrace::test]
async fn asking_agents_when_blocked_is_independent_of_ntfy() {
    let harness = helm().await;
    let instructions = || crate::sessions::launch_instructions(&harness.state);
    let stored = harness
        .store
        .save_ntfy_config(save("https://ntfy.sh", None, true))
        .await
        .ok()
        .unwrap();
    assert_eq!(
        stored.ask_agents_when_blocked, None,
        "the premise: no choice of its own"
    );
    assert!(
        instructions().await.is_some(),
        "an upgraded configuration keeps asking"
    );

    let ask_without_ntfy = SaveSettings {
        notify_blocked: Some(false),
        ask_agents_when_blocked: Some(true),
        ..save("https://ntfy.sh", None, false)
    };
    harness
        .store
        .save_ntfy_config(ask_without_ntfy)
        .await
        .ok()
        .unwrap();
    assert!(instructions().await.is_some(), "asking does not need ntfy");
    let kept = harness
        .store
        .save_ntfy_config(save("https://ntfy.sh", None, false))
        .await
        .ok()
        .unwrap();
    assert_eq!(
        kept.ask_agents_when_blocked,
        Some(true),
        "a save without the choice keeps it"
    );

    let alert_without_asking = SaveSettings {
        ask_agents_when_blocked: Some(false),
        ..save("https://ntfy.sh", None, true)
    };
    harness
        .store
        .save_ntfy_config(alert_without_asking)
        .await
        .ok()
        .unwrap();
    assert_eq!(
        instructions().await,
        None,
        "blocked alerts alone do not ask"
    );
}

/// Only the URL origin owns a retained token: path/topic edits and equivalent
/// default-port spellings preserve it, while scheme, host or effective-port
/// changes clear it. Explicit replacement and clearing always take precedence.
#[farhelm_testtrace::test]
fn token_preservation_is_limited_to_the_saved_origin() {
    let previous = || {
        Config::saved(
            Config::default(),
            save("https://example.test", Some(TOKEN), true),
        )
        .unwrap()
    };
    for server in [
        "https://example.test",
        "https://example.test:443/prefix",
        "https://EXAMPLE.test/other",
    ] {
        let result = Config::saved(previous(), save(server, None, true)).unwrap();
        assert_eq!(result.token.as_deref(), Some(TOKEN));
    }
    for server in [
        "http://example.test",
        "https://other.test",
        "https://example.test:8443",
    ] {
        let result = Config::saved(previous(), save(server, None, true)).unwrap();
        assert!(result.token.is_none());
        assert!(!result.settings.token_set);
        let replaced = Config::saved(previous(), save(server, Some("replacement"), true)).unwrap();
        assert_eq!(replaced.token.as_deref(), Some("replacement"));
        let cleared = Config::saved(previous(), save(server, Some(""), true)).unwrap();
        assert!(cleared.token.is_none());
    }
}

/// A stale form has never seen the saved secret, yet can change the server.
/// Drive the real protected save and send routes against two live listeners:
/// the first must receive its token, and the second must receive the same
/// test alert without it after either an omitted or null token submission.
/// This pins the database save rule, not just the form's advisory warning.
#[farhelm_testtrace::test]
async fn changing_server_from_a_stale_form_never_forwards_the_saved_token() {
    let mut first = stand_in(Answer::Success).await;
    let mut second = stand_in(Answer::Success).await;
    let harness = helm().await;
    for (index, omit) in [false, true].into_iter().enumerate() {
        // Capture a token-free draft before a later save stores a secret.
        let mut stale = serde_json::to_value(save(&second.server_url, None, false)).unwrap();
        if omit {
            stale.as_object_mut().unwrap().remove("token");
        }
        harness
            .store
            .save_ntfy_config(save(&first.server_url, Some(TOKEN), false))
            .await
            .ok()
            .unwrap();
        assert!(
            harness
                .store
                .ntfy_config()
                .await
                .unwrap()
                .settings
                .token_set
        );
        assert!(!first.task.is_finished() && !second.task.is_finished());
        assert!(harness.state.ntfy.test().await.is_ok());
        first.arrived(index + 1).await;
        assert_eq!(
            first.received.lock().unwrap()[index].headers["authorization"],
            format!("Bearer {TOKEN}")
        );

        let response = harness
            .router()
            .oneshot(request("PUT", "/api/ntfy-alerts", stale))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let saved = text(response).await;
        assert!(!saved.contains(TOKEN));
        assert!(!serde_json::from_str::<Settings>(&saved).unwrap().token_set);
        assert!(harness.store.ntfy_config().await.unwrap().token.is_none());
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
        second.arrived(index + 1).await;
        let received = second.received.lock().unwrap()[index].clone();
        assert_eq!(received.body, "Farhelm ntfy alerts are working");
        assert!(!received.headers.contains_key("authorization"));
    }
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

/// A blocked alert is the agent's own request to reach the user, so it goes out only under its own switch, and says
/// why. The sender handles the queue in order, so a waiting alert queued after an unsent blocked one is the next arrival
/// whether or not the blocked one was dropped; the arrival count tells them apart. Specified: with the blocked switch
/// off, a queued blocked alert is dropped and the waiting alert after it is the first and only arrival; with it on,
/// the next blocked alert arrives with the body "<title> on this machine is blocked: <reason>".
#[farhelm_testtrace::test]
async fn blocked_alerts_follow_their_switch_and_carry_the_reason() {
    let mut server = stand_in(Answer::Success).await;
    let harness = helm().await;
    let mut blocked_off = save(&server.server_url, Some(TOKEN), true);
    blocked_off.notify_blocked = Some(false);
    harness
        .store
        .save_ntfy_config(blocked_off)
        .await
        .ok()
        .unwrap();
    let host = harness.store.list_hosts().await.unwrap()[0].id;
    harness.state.ntfy.enqueue_blocked(
        SessionKey::new(host, SESSION),
        TITLE.into(),
        "deploy: no credentials".into(),
        Instant::now(),
    );
    harness.state.ntfy.enqueue(host, settled_now());
    server.arrived(1).await;
    assert_eq!(
        server
            .received
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.body.clone())
            .collect::<Vec<_>>(),
        [format!("{TITLE} on this machine is waiting for you")]
    );
    harness
        .store
        .save_ntfy_config(save(&server.server_url, Some(TOKEN), true))
        .await
        .ok()
        .unwrap();
    harness.state.ntfy.enqueue_blocked(
        SessionKey::new(host, SESSION),
        TITLE.into(),
        "deploy: no credentials".into(),
        Instant::now(),
    );
    server.arrived(2).await;
    assert_eq!(
        server.received.lock().unwrap()[1].body,
        format!("{TITLE} on this machine is blocked: deploy: no credentials")
    );
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
    harness.state.ntfy.enqueue(host, settled_now());
    server.arrived(1).await;
    assert_eq!(
        server.received.lock().unwrap()[0].body,
        format!("{TITLE} on this machine is waiting for you")
    );
    for _ in 0..QUEUE_CAPACITY {
        harness.state.ntfy.enqueue(host, settled_now());
    }
    assert_eq!(harness.state.ntfy.0.queue.capacity(), 0);
    harness.state.ntfy.enqueue(host, settled_now());
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

/// A session that becomes waiting and stays waiting gets its alert once its
/// settling time is up, with nothing else refreshing the host: no further
/// hint, no explicit refresh, and the harness's backstop poll an hour away. Why
/// it matters: this is SPEC.md's "remains so for 3 seconds" promise, which
/// since the poll became thirty seconds only the refresh the host loop
/// schedules at the spell's deadline keeps; a test that only counted
/// refreshes could not tell an alert from a refresh that came too early to
/// settle the spell.
#[farhelm_testtrace::test]
async fn a_waiting_session_is_alerted_at_its_deadline_without_another_refresh() {
    let mut server = stand_in(Answer::Success).await;
    let harness = FleetBuilder::new()
        .await
        .ntfy_client(client())
        .local(HostScript {
            identity: Some("deadline-host".to_string()),
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
    // The first listing after alerts come on is the waiting baseline; take it
    // while the session is not waiting, so the wait below is a fresh spell.
    harness.refresh_to_completion(host).await;
    let status = |status: SessionStatus| {
        harness.fleet.edit(host, |script| {
            script.sessions[0].status = status;
        })
    };
    // Read before the spell can start, so the premise check below errs on
    // the safe side: the spell starts on the real clock, and the jump is only
    // meaningful while its deadline is still ahead.
    let before_spell = std::time::Instant::now();
    status(SessionStatus::Waiting);
    harness.refresh_to_completion(host).await;
    // The helper proves a refresh completed by starting its successor, whose
    // reply is already drained. Were that reply Waiting, it could settle the
    // spell itself during the clock jump below, and the test would pass
    // without the deadline's wake; make it Unknown, which keeps the spell
    // pending without settling it.
    status(SessionStatus::Unknown);
    harness.refresh_to_completion(host).await;
    assert!(
        server.received.lock().unwrap().is_empty(),
        "the premise: nothing is sent when the spell begins"
    );
    // Only a listing drained after this edit carries the title, so the
    // alert's text proves which refresh settled the spell.
    harness.fleet.edit(host, |script| {
        script.sessions[0].status = SessionStatus::Waiting;
        script.sessions[0].title = "deadline waiting".into();
    });

    assert!(
        before_spell.elapsed() < SETTLE,
        "the premise: the spell's deadline is still ahead on the real clock \
         before the jump (this run was too slow to test the deadline)"
    );
    // Only the deadline can refresh the host now (the harness polls hourly):
    // jump past the settling time and let the actor run.
    tokio::time::pause();
    tokio::time::advance(SETTLE + Duration::from_millis(100)).await;
    tokio::time::resume();
    server.arrived(1).await;
    assert_eq!(
        server.received.lock().unwrap()[0].body,
        "deadline waiting on this machine is waiting for you"
    );
}

/// A save that turns waiting alerts on refreshes every serving host at once,
/// with no hint and the harness's backstop poll an hour away. Why it matters:
/// the first listing after alerts come on is each host's waiting baseline, so
/// a session that starts waiting before that listing is baselined and never
/// alerts for that wait; with the poll at thirty seconds that window would be
/// up to half a minute instead of one round trip.
#[farhelm_testtrace::test]
async fn turning_waiting_alerts_on_refreshes_every_host_at_once() {
    let server = stand_in(Answer::Success).await;
    let (builder, remote) = FleetBuilder::new()
        .await
        .ntfy_client(client())
        .local(HostScript {
            identity: Some("baseline-local".to_string()),
            sessions: vec![session("one", 1)],
            ..HostScript::default()
        })
        .await
        .ssh(
            "baseline-remote.example",
            HostScript {
                identity: Some("baseline-remote".to_string()),
                sessions: vec![session("two", 2)],
                ..HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    let local = crate::rest_harness::local_id(&harness.store).await;
    for host in [local, remote] {
        harness.await_refreshed(host).await;
    }
    assert!(
        !harness.store.waiting_alerts_enabled(),
        "the premise: waiting alerts start off"
    );

    let before = [local, remote].map(|host| harness.fleet.host_requests(host));
    let body = serde_json::to_value(save(&server.server_url, None, true)).unwrap();
    let response = harness
        .router()
        .oneshot(request("PUT", "/api/ntfy-alerts", body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        harness.store.waiting_alerts_enabled(),
        "the premise: the save turned them on"
    );
    for (host, before) in [local, remote].into_iter().zip(before) {
        tokio::time::timeout(
            Duration::from_secs(10),
            harness.fleet.await_host_requests(host, before + 1),
        )
        .await
        .unwrap_or_else(|_| panic!("the save refreshes host {host} without waiting for the poll"));
    }
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

/// The blocked alert is the one place the user reads which session `farhelm agent notify` came from, so it must name
/// the session by the title they know it by, and a session the helm has not listed yet (one that has only just
/// started) must still be named somehow. Specified: with blocked alerts on, a Notify from the listed session
/// `local-live` titled "Build console" sends "Build console on this machine is blocked: <reason>", and one from
/// `never-listed`, which the helm has not cached, sends "never-listed on this machine is blocked: <reason>".
#[farhelm_testtrace::test]
async fn a_blocked_alert_names_the_session_by_its_cached_title_or_else_its_id() {
    use crate::agent_requests::{AgentOrigin, AgentRequestHandler, HelmAgentRequests};
    use farhelm_proto::{AgentOutcome, AgentVerb};

    let mut server = stand_in(Answer::Success).await;
    let harness = FleetBuilder::new()
        .await
        .ntfy_client(client())
        .local(HostScript {
            identity: Some("identity-local".to_string()),
            sessions: vec![SessionInfo {
                title: "Build console".into(),
                ..session("local-live", 30)
            }],
            ..HostScript::default()
        })
        .await
        .start()
        .await;
    let host = crate::rest_harness::local_id(&harness.store).await;
    harness.await_refreshed(host).await;
    harness
        .store
        .save_ntfy_config(save(&server.server_url, Some(TOKEN), true))
        .await
        .ok()
        .unwrap();
    let connection = harness
        .manager
        .status(host)
        .and_then(|status| status.client)
        .expect("the premise: the local host is connected")
        .connection_id();
    let handler: std::sync::Arc<dyn AgentRequestHandler> =
        HelmAgentRequests::for_state(&harness.state);
    for session_id in ["local-live", "never-listed"] {
        let outcome = handler
            .handle(
                AgentOrigin { host, connection },
                session_id,
                AgentVerb::Notify {
                    reason: "deploy: no credentials".into(),
                },
            )
            .await;
        assert!(matches!(outcome, AgentOutcome::Ok { .. }), "{outcome:?}");
    }
    server.arrived(2).await;
    assert_eq!(
        server
            .received
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.body.clone())
            .collect::<Vec<_>>(),
        [
            "Build console on this machine is blocked: deploy: no credentials",
            "never-listed on this machine is blocked: deploy: no credentials",
        ]
    );
}

// ---- The grace period ------------------------------------------------------------------------------------------------
//
// The rules are pinned on `Attention` alone with injected times, where every edge (exactly at the start, one
// millisecond before the due time) can be named. The sender's own use of them, which reads the clock, is pinned
// against the stand-in server on a paused clock, and the withdrawals' wiring (a cleared mark, a disconnect, a host
// entry's removal) against a real helm.

/// The grace period the rule tests hold alerts for.
const GRACE: Duration = Duration::from_secs(10);

/// An alert of `kind` about `id` on `host`, begun at `started_at`, titled "title <id>" so the alerts a test takes back
/// say which they were.
fn alert(host: HostId, id: &str, kind: AlertKind, started_at: Instant) -> Alert {
    Alert {
        key: SessionKey::new(host, id),
        title: format!("title {id}"),
        kind,
        started_at,
    }
}

/// A waiting alert about `id` on host 1.
fn waiting(id: &str, started_at: Instant) -> Alert {
    alert(1, id, AlertKind::Waiting, started_at)
}

/// A blocked alert about `id` on host 1, for `reason`.
fn blocked(id: &str, reason: &str, started_at: Instant) -> Alert {
    alert(
        1,
        id,
        AlertKind::Blocked {
            reason: reason.into(),
        },
        started_at,
    )
}

/// The alerts' texts as the sender would send them on host "h", so a test reads which alerts, of which kind, came back.
fn bodies(alerts: Vec<Alert>) -> Vec<String> {
    alerts
        .into_iter()
        .map(|alert| alert.kind.body(&alert.title, "h"))
        .collect()
}

/// The grace period exists so an alert the user would not need is never sent, which only works if it is really held
/// until its time and then really sent. Specified: an alert taken at `t` under a 10-second grace period is held, due
/// at `t + 10 s`; nothing is due a millisecond before that; at the due time it comes back once, and nothing is held
/// after.
#[farhelm_testtrace::test]
fn an_alert_is_held_until_its_grace_period_ends() {
    let start = Instant::now();
    let taken = start + SETTLE;
    let mut attention = Attention::default();
    assert!(attention.admit(waiting("a", start), taken, GRACE).is_none());
    let due = taken + GRACE;
    assert_eq!(attention.next_due(), Some(due));
    assert!(
        attention
            .take_due(due - Duration::from_millis(1))
            .is_empty()
    );
    assert_eq!(
        bodies(attention.take_due(due)),
        ["title a on h is waiting for you"]
    );
    assert_eq!(attention.next_due(), None);
}

/// A grace period of zero is how a user keeps the old behavior, so it must not hold anything. Specified: under a zero
/// grace period an alert comes back to be sent at once and nothing is held.
#[farhelm_testtrace::test]
fn a_zero_grace_period_sends_at_once() {
    let start = Instant::now();
    let mut attention = Attention::default();
    let sent = attention.admit(blocked("a", "stuck", start), start, Duration::ZERO);
    assert_eq!(
        bodies(sent.into_iter().collect()),
        ["title a on h is blocked: stuck"]
    );
    assert_eq!(attention.next_due(), None);
}

/// Attention is the evidence that the user already knows, so it must withdraw an alert whether the alert is held or
/// still on its way: the waiting spell settles three seconds after it begins, and a user who switched to the session
/// meanwhile must not be told about it ten seconds later. Specified: attention after a held alert began withdraws it;
/// attention during the settling time, before the alert is even taken, drops it when it is; attention at the very
/// moment an alert began counts; and attention withdraws both kinds for that session and nothing for another.
#[farhelm_testtrace::test]
fn attention_since_an_alert_began_withdraws_it() {
    let start = Instant::now();
    let second = Duration::from_secs(1);
    let mut attention = Attention::default();
    assert!(
        attention
            .admit(waiting("a", start), start + SETTLE, GRACE)
            .is_none()
    );
    assert!(
        attention
            .admit(blocked("a", "stuck", start), start + SETTLE, GRACE)
            .is_none()
    );
    assert!(
        attention
            .admit(waiting("b", start), start + SETTLE, GRACE)
            .is_none()
    );
    attention.note_aware(&SessionKey::new(1, "a"), start + SETTLE + second);
    assert_eq!(
        bodies(attention.take_due(start + SETTLE * 10 + GRACE)),
        ["title b on h is waiting for you"],
        "a's held alerts were withdrawn, b's was not"
    );

    let mut attention = Attention::default();
    attention.note_aware(&SessionKey::new(1, "a"), start + second);
    assert!(
        attention
            .admit(waiting("a", start), start + SETTLE, GRACE)
            .is_none()
    );
    assert_eq!(
        attention.next_due(),
        None,
        "attention during the settling time dropped the alert"
    );

    let mut attention = Attention::default();
    attention.note_aware(&SessionKey::new(1, "a"), start);
    assert!(
        attention
            .admit(waiting("a", start), start + SETTLE, GRACE)
            .is_none()
    );
    assert_eq!(attention.next_due(), None, "attention at the start counts");
}

/// A delayed signal reports when the user acted, and the alerts must take that time, not the arrival's: an alert that
/// began after the interaction but before its signal arrived is not something the user saw. Specified: with two
/// alerts held, one begun before the interaction and one after it, attention noted at the interaction's time
/// withdraws only the first; a delayed record older than the session's latest one does not move that record back.
#[farhelm_testtrace::test]
fn attention_dated_in_the_past_withdraws_only_what_began_by_then() {
    let start = Instant::now();
    let acted = start + Duration::from_secs(1);
    let arrived = acted + Duration::from_secs(2);
    let mut attention = Attention::default();
    assert!(
        attention
            .admit(waiting("a", start), arrived, GRACE)
            .is_none()
    );
    let begun_after = acted + Duration::from_millis(500);
    assert!(
        attention
            .admit(blocked("a", "stuck", begun_after), arrived, GRACE)
            .is_none()
    );
    attention.note_aware(&SessionKey::new(1, "a"), acted);
    assert_eq!(
        bodies(attention.take_due(arrived + GRACE)),
        ["title a on h is blocked: stuck"],
        "the alert begun between the interaction and its signal stays"
    );
    attention.note_aware(&SessionKey::new(1, "a"), arrived);
    attention.note_aware(&SessionKey::new(1, "a"), acted);
    assert_eq!(
        attention.last_aware.get(&SessionKey::new(1, "a")),
        Some(&arrived),
        "an older record does not replace a newer one"
    );
}

/// Attention is about what the user has seen, so attention before something began cannot mean they saw it: the user
/// who typed into a session a minute ago must still hear that it now waits. Specified: attention one second before an
/// alert began leaves it held, and it is sent at its due time.
#[farhelm_testtrace::test]
fn attention_before_an_alert_began_does_not_withdraw_it() {
    let aware = Instant::now();
    let start = aware + Duration::from_secs(1);
    let mut attention = Attention::default();
    attention.note_aware(&SessionKey::new(1, "a"), aware);
    assert!(
        attention
            .admit(waiting("a", start), start + SETTLE, GRACE)
            .is_none()
    );
    assert_eq!(
        bodies(attention.take_due(start + SETTLE + GRACE)),
        ["title a on h is waiting for you"]
    );
}

/// An alert about something that is over would be a false alarm, and the withdrawal may reach the sender before the
/// alert does. Specified: withdrawing a session's waiting alert leaves its blocked one, and the other way round;
/// withdrawing a host's alerts leaves another host's, and withdrawing only its waiting alerts (a connection's end)
/// leaves its blocked ones, held or queued; a withdrawal recorded before an alert is taken drops that alert
/// when it began before the withdrawal, and holds it when it began after (the session's next wait, the host's next
/// connection).
#[farhelm_testtrace::test]
fn withdrawals_cancel_their_own_alerts_held_or_queued() {
    let start = Instant::now();
    let later = start + Duration::from_secs(1);
    let far = start + SETTLE * 10 + GRACE * 10;
    let key = SessionKey::new(1, "a");

    let mut attention = Attention::default();
    assert!(attention.admit(waiting("a", start), start, GRACE).is_none());
    assert!(
        attention
            .admit(blocked("a", "stuck", start), start, GRACE)
            .is_none()
    );
    attention.cancel(&key, Kind::Waiting, later);
    assert_eq!(
        bodies(attention.take_due(far)),
        ["title a on h is blocked: stuck"],
        "leaving Waiting withdraws only the waiting alert"
    );

    let mut attention = Attention::default();
    assert!(attention.admit(waiting("a", start), start, GRACE).is_none());
    assert!(
        attention
            .admit(blocked("a", "stuck", start), start, GRACE)
            .is_none()
    );
    attention.cancel(&key, Kind::Blocked, later);
    assert_eq!(
        bodies(attention.take_due(far)),
        ["title a on h is waiting for you"],
        "a cleared mark withdraws only the blocked alert"
    );

    let mut attention = Attention::default();
    assert!(attention.admit(waiting("a", start), start, GRACE).is_none());
    assert!(
        attention
            .admit(alert(2, "z", AlertKind::Waiting, start), start, GRACE)
            .is_none()
    );
    attention.cancel_host(1, &[Kind::Waiting, Kind::Blocked], later);
    assert_eq!(
        bodies(attention.take_due(far)),
        ["title z on h is waiting for you"],
        "a host's withdrawal leaves another host's alerts"
    );

    let mut attention = Attention::default();
    assert!(attention.admit(waiting("a", start), start, GRACE).is_none());
    assert!(
        attention
            .admit(blocked("a", "stuck", start), start, GRACE)
            .is_none()
    );
    attention.cancel_host(1, &[Kind::Waiting], later);
    assert!(
        attention
            .admit(blocked("b", "queued", start), later, GRACE)
            .is_none()
    );
    let mut sent = bodies(attention.take_due(far));
    sent.sort();
    assert_eq!(
        sent,
        [
            "title a on h is blocked: stuck",
            "title b on h is blocked: queued"
        ],
        "a connection's end withdraws only waiting alerts, held or queued"
    );

    let mut attention = Attention::default();
    attention.cancel(&key, Kind::Blocked, later);
    attention.cancel_host(2, &[Kind::Waiting, Kind::Blocked], later);
    assert!(
        attention
            .admit(blocked("a", "stuck", start), later, GRACE)
            .is_none()
    );
    assert!(
        attention
            .admit(alert(2, "z", AlertKind::Waiting, start), later, GRACE)
            .is_none()
    );
    assert_eq!(
        attention.next_due(),
        None,
        "both were withdrawn in the queue"
    );
    let next = later + Duration::from_secs(1);
    assert!(
        attention
            .admit(blocked("a", "again", next), next, GRACE)
            .is_none()
    );
    assert!(
        attention
            .admit(alert(2, "z", AlertKind::Waiting, next), next, GRACE)
            .is_none()
    );
    assert_eq!(
        bodies(attention.take_due(far)).len(),
        2,
        "what began after the withdrawals is held as usual"
    );
}

/// An agent can report blocked several times in a row; each report must not restart the wait or send twice.
/// Specified: a second blocked alert for a session already holding one keeps the first's due time and replaces its
/// reason, and only one comes back; one taken after the grace period was turned off goes at once and leaves nothing
/// held.
#[farhelm_testtrace::test]
fn a_repeated_alert_keeps_its_due_time_and_sends_once() {
    let start = Instant::now();
    let mut attention = Attention::default();
    assert!(
        attention
            .admit(blocked("a", "first", start), start, GRACE)
            .is_none()
    );
    let again = start + Duration::from_secs(5);
    assert!(
        attention
            .admit(blocked("a", "second", again), again, GRACE)
            .is_none()
    );
    assert_eq!(attention.next_due(), Some(start + GRACE));
    assert_eq!(
        bodies(attention.take_due(start + GRACE)),
        ["title a on h is blocked: second"]
    );

    assert!(
        attention
            .admit(blocked("a", "held", start), start, GRACE)
            .is_none()
    );
    assert!(
        attention
            .admit(blocked("a", "now", again), again, Duration::ZERO)
            .is_some()
    );
    assert_eq!(
        attention.next_due(),
        None,
        "sent at once after the grace period was turned off, it replaces the held one"
    );
}

/// Every session the user ever attends to would otherwise leave a record for the helm's whole life. Specified: past
/// the prune threshold, records older than the horizon go and the fresh one stays; under the threshold, old records
/// stay; a prune that frees nothing doubles the threshold, so a keystroke does not scan every fresh record.
#[farhelm_testtrace::test]
fn withdrawal_records_are_bounded() {
    let start = Instant::now();
    let mut attention = Attention::default();
    for n in 0..RECORDS_PRUNE_AT {
        attention.note_aware(&SessionKey::new(1, format!("s{n}")), start);
    }
    assert_eq!(
        attention.last_aware.len(),
        RECORDS_PRUNE_AT,
        "nothing is pruned at the threshold"
    );
    let fresh = SessionKey::new(1, "fresh");
    let now = start + RECORD_HORIZON;
    attention.note_aware(&fresh, now);
    assert_eq!(attention.last_aware.keys().collect::<Vec<_>>(), [&fresh]);

    // A prune that frees nothing moves the threshold up, so the next records do not each scan them all again.
    for n in 0..RECORDS_PRUNE_AT {
        attention.note_aware(&SessionKey::new(1, format!("t{n}")), now);
    }
    assert_eq!(attention.records(), RECORDS_PRUNE_AT + 1);
    assert_eq!(attention.prune_at, (RECORDS_PRUNE_AT + 1) * 2);
}

/// The host refresh withdraws a held waiting alert from what the policy reports as ended, so every way a spell ends
/// must be reported. Specified: a spell ends when its session reads as something other than Waiting or Unknown, when
/// it leaves the listing, and when waiting alerts are turned off; an Unknown reading ends nothing; each end is
/// reported once.
#[farhelm_testtrace::test]
fn the_policy_reports_every_spell_that_ended() {
    let now = Instant::now();
    let mut policy = Policy::default();
    assert!(policy.observe(&[], now, true).is_empty());
    let list = [
        observation("a", SessionStatus::Waiting),
        observation("b", SessionStatus::Waiting),
        observation("c", SessionStatus::Waiting),
    ];
    assert!(policy.observe(&list, now, true).is_empty());
    assert_eq!(policy.take_ended(), Vec::<String>::new());
    let next = [
        observation("a", SessionStatus::Running),
        observation("c", SessionStatus::Unknown),
    ];
    assert!(policy.observe(&next, now + SETTLE, true).is_empty());
    let mut ended = policy.take_ended();
    ended.sort();
    assert_eq!(ended, ["a", "b"], "a left Waiting and b left the listing");
    assert_eq!(policy.take_ended(), Vec::<String>::new(), "reported once");
    assert!(policy.observe(&next, now + SETTLE, false).is_empty());
    assert_eq!(
        policy.take_ended(),
        ["c"],
        "turning alerts off ends c's spell"
    );
}

/// The grace period is a stored setting with a bound, and an older client's save must not reset it. Specified: a save
/// naming 600 seconds stores it; one naming 601 is refused with a fixed message; a save that leaves it out keeps the
/// stored one; a configuration that never chose one reads as the 10-second default.
#[farhelm_testtrace::test]
fn the_grace_period_saves_within_its_bound() {
    let previous = Config {
        settings: Settings::default(),
        token: None,
    };
    assert_eq!(
        previous.settings.grace_period_secs,
        farhelm_proto::ntfy::DEFAULT_GRACE_PERIOD_SECS
    );
    let with = |secs: Option<u32>| SaveSettings {
        grace_period_secs: secs,
        ..save("https://ntfy.sh", None, true)
    };
    let saved = Config::saved(previous.clone(), with(Some(600))).unwrap();
    assert_eq!(saved.settings.grace_period_secs, 600);
    assert_eq!(
        Config::saved(previous.clone(), with(Some(601))).err(),
        Some("the grace period must be at most 600 seconds")
    );
    let kept = Config::saved(saved, with(None)).unwrap();
    assert_eq!(kept.settings.grace_period_secs, 600);
}

/// The stored grace period must survive the database and come back through the route, or the Settings card would
/// show and resend a value the helm does not hold. Specified: saving 45 seconds through `PUT /api/ntfy-alerts` answers
/// 45, and the next `GET` answers 45; a `PUT` naming 601 is refused with 400 and leaves 45 stored.
#[farhelm_testtrace::test]
async fn the_grace_period_round_trips_through_the_route() {
    let harness = helm().await;
    let mut body = serde_json::to_value(save("https://ntfy.sh", None, true)).unwrap();
    body["grace_period_secs"] = 45.into();
    let response = harness
        .router()
        .oneshot(request("PUT", "/api/ntfy-alerts", body.clone()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let answered: serde_json::Value = serde_json::from_str(&text(response).await).unwrap();
    assert_eq!(answered["grace_period_secs"], 45);
    body["grace_period_secs"] = 601.into();
    let response = harness
        .router()
        .oneshot(request("PUT", "/api/ntfy-alerts", body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = harness
        .router()
        .oneshot(request("GET", "/api/ntfy-alerts", serde_json::Value::Null))
        .await
        .unwrap();
    let read: serde_json::Value = serde_json::from_str(&text(response).await).unwrap();
    assert_eq!(read["grace_period_secs"], 45);
}

/// Save `grace` as the grace period on the stand-in at `server`, with every alert switched on.
async fn save_grace(harness: &Harness, server: &StandIn, grace: Duration) {
    let config = SaveSettings {
        grace_period_secs: Some(u32::try_from(grace.as_secs()).unwrap()),
        ..save(&server.server_url, Some(TOKEN), true)
    };
    harness.store.save_ntfy_config(config).await.ok().unwrap();
}

/// Wait until the sender has taken and decided `count` alerts in all, so a test acts on a held alert only once it is
/// held. Real time: the sender reads the settings from the database before deciding.
async fn taken(harness: &Harness, count: u64) {
    let mut taken = harness.state.ntfy.0.taken.clone();
    tokio::time::timeout(Duration::from_secs(10), taken.wait_for(|n| *n >= count))
        .await
        .expect("the sender did not take the alert")
        .expect("the sender is gone");
}

/// Whether the sender holds an alert of `kind` for `id` on `host` now.
fn holds(harness: &Harness, host: HostId, id: &str, kind: Kind) -> bool {
    harness
        .state
        .ntfy
        .attention()
        .held
        .contains_key(&(SessionKey::new(host, id), kind))
}

/// A grace period no wiring test can live through: an alert that leaves the held map during one was withdrawn, not
/// sent, which the stand-in receiving nothing confirms.
fn unreachable_grace() -> Duration {
    Duration::from_secs(u64::from(farhelm_proto::ntfy::MAX_GRACE_PERIOD_SECS))
}

/// Assert the stand-in received no alert at all.
fn nothing_sent(server: &StandIn) {
    assert_eq!(
        server
            .received
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.body.clone())
            .collect::<Vec<_>>(),
        Vec::<String>::new(),
        "an alert was sent"
    );
}

/// The sender must really sleep until a held alert is due and then send it, with nothing else waking it: the user
/// relies on the alert arriving even when nothing else happens in the fleet. Specified, on a paused clock: a waiting
/// alert queued under a 10-second grace period is held and unsent; nine seconds on it is still held; past ten it is
/// sent, with the usual text.
#[farhelm_testtrace::test]
async fn the_sender_sends_a_held_alert_when_it_is_due() {
    let mut server = stand_in(Answer::Success).await;
    let harness = helm().await;
    save_grace(&harness, &server, GRACE).await;
    let host = harness.store.list_hosts().await.unwrap()[0].id;
    harness.state.ntfy.enqueue(host, settled_now());
    taken(&harness, 1).await;
    assert!(
        holds(&harness, host, SESSION, Kind::Waiting),
        "the premise: it is held"
    );
    tokio::time::pause();
    tokio::time::advance(GRACE - Duration::from_secs(1)).await;
    assert!(
        holds(&harness, host, SESSION, Kind::Waiting) && server.received.lock().unwrap().is_empty(),
        "still held a second before its due time"
    );
    tokio::time::advance(Duration::from_secs(2)).await;
    tokio::time::resume();
    server.arrived(1).await;
    assert_eq!(
        server.received.lock().unwrap()[0].body,
        format!("{TITLE} on this machine is waiting for you")
    );
}

/// The point of the grace period: an alert the user made unnecessary by attending to the session is never sent. The
/// sender handles the queue in order, so an alert queued after the withdrawal, once the period is past, arriving first
/// shows the withdrawn one was not sent. Specified, on a paused clock: a held waiting alert, then attention to its
/// session, then the clock past its due time: nothing is held, and the next arrival is a later alert for another
/// session.
#[farhelm_testtrace::test]
async fn attention_keeps_a_held_alert_from_being_sent() {
    let mut server = stand_in(Answer::Success).await;
    let harness = helm().await;
    save_grace(&harness, &server, GRACE).await;
    let host = harness.store.list_hosts().await.unwrap()[0].id;
    harness.state.ntfy.enqueue(host, settled_now());
    taken(&harness, 1).await;
    assert!(
        holds(&harness, host, SESSION, Kind::Waiting),
        "the premise: it is held"
    );
    harness
        .state
        .ntfy
        .note_attention(&SessionKey::new(host, SESSION));
    assert!(!holds(&harness, host, SESSION, Kind::Waiting));
    tokio::time::pause();
    tokio::time::advance(GRACE * 2).await;
    tokio::time::resume();
    save_grace(&harness, &server, Duration::ZERO).await;
    harness.state.ntfy.enqueue(
        host,
        Settled {
            id: "other".into(),
            title: "other title".into(),
            since: Instant::now(),
        },
    );
    server.arrived(1).await;
    assert_eq!(
        server
            .received
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.body.clone())
            .collect::<Vec<_>>(),
        ["other title on this machine is waiting for you"]
    );
}

/// A cleared blocked mark means the agent's report is over, whichever path cleared it, so its alert must be withdrawn
/// through the one exit every clear takes. Specified: an agent's report on a session marks it and holds its blocked
/// alert; clearing the mark through `attention::clear` withdraws it, and nothing was sent.
#[farhelm_testtrace::test]
async fn a_cleared_mark_withdraws_its_held_blocked_alert() {
    let server = stand_in(Answer::Success).await;
    let harness = helm().await;
    save_grace(&harness, &server, unreachable_grace()).await;
    let host = harness.store.list_hosts().await.unwrap()[0].id;
    let key = SessionKey::new(host, SESSION);
    harness.store.mark_blocked(&key).await.unwrap();
    harness
        .state
        .ntfy
        .enqueue_blocked(key.clone(), TITLE.into(), "stuck".into(), Instant::now());
    taken(&harness, 1).await;
    assert!(
        holds(&harness, host, SESSION, Kind::Blocked),
        "the premise: it is held"
    );
    assert!(
        crate::attention::clear(
            &harness.store,
            harness.manager.events(),
            &harness.state.ntfy,
            &key
        )
        .await
        .unwrap(),
        "the premise: the mark was there"
    );
    assert!(!holds(&harness, host, SESSION, Kind::Blocked));
    nothing_sent(&server);
}

/// A host whose connection ended can no longer say whether its sessions still wait, and the next connection starts a
/// fresh baseline, so nothing would ever end a spell the old connection settled. A blocked mark is durable, though, and
/// the agent is still blocked, so a connection blip must not cost the user that alert. Specified: with a held waiting
/// alert and a held blocked alert for a session on a connected host, once its connection drops and the host has
/// reconnected, the waiting alert is withdrawn, the blocked one is still held, and nothing was sent.
#[farhelm_testtrace::test]
async fn a_disconnect_withdraws_only_its_hosts_held_waiting_alerts() {
    let server = stand_in(Answer::Success).await;
    let harness = FleetBuilder::new()
        .await
        .ntfy_client(client())
        .local(HostScript {
            identity: Some("grace-local".to_string()),
            sessions: vec![session(SESSION, 1)],
            ..HostScript::default()
        })
        .await
        .start()
        .await;
    let host = harness.store.list_hosts().await.unwrap()[0].id;
    harness.await_refreshed(host).await;
    let incarnation = harness.manager.status(host).unwrap().incarnation;
    save_grace(&harness, &server, unreachable_grace()).await;
    let key = SessionKey::new(host, SESSION);
    harness.store.mark_blocked(&key).await.unwrap();
    harness.state.ntfy.enqueue(host, settled_now());
    harness
        .state
        .ntfy
        .enqueue_blocked(key, TITLE.into(), "stuck".into(), Instant::now());
    taken(&harness, 2).await;
    assert!(
        holds(&harness, host, SESSION, Kind::Waiting)
            && holds(&harness, host, SESSION, Kind::Blocked),
        "the premise: both are held"
    );
    harness.fleet.kill_connection(host);
    harness.await_refreshed_after(host, incarnation).await;
    assert!(!holds(&harness, host, SESSION, Kind::Waiting));
    assert!(
        holds(&harness, host, SESSION, Kind::Blocked),
        "the blocked alert outlives the connection"
    );
    nothing_sent(&server);
}

/// The host refresh withdraws a held waiting alert once the session stops waiting, through the spells the waiting
/// policy reports as ended; a refactor that moved that step out of the observation would leave such alerts to go out.
/// Specified: with waiting alerts on and a session already waiting at the baseline, a held waiting alert for it is
/// withdrawn by the refresh that reads it running, and nothing was sent.
#[farhelm_testtrace::test]
async fn a_refresh_that_reads_a_session_no_longer_waiting_withdraws_its_alert() {
    let server = stand_in(Answer::Success).await;
    let mut waiting_session = session(SESSION, 1);
    waiting_session.status = SessionStatus::Waiting;
    let harness = FleetBuilder::new()
        .await
        .ntfy_client(client())
        .local(HostScript {
            identity: Some("grace-refresh".to_string()),
            sessions: vec![waiting_session],
            ..HostScript::default()
        })
        .await
        .start()
        .await;
    let host = harness.store.list_hosts().await.unwrap()[0].id;
    harness.await_refreshed(host).await;
    save_grace(&harness, &server, unreachable_grace()).await;
    // The first listing after alerts come on is the waiting baseline; the session is in it, waiting, so the policy
    // tracks it and reports its spell's end.
    harness.refresh_to_completion(host).await;
    harness.state.ntfy.enqueue(host, settled_now());
    taken(&harness, 1).await;
    assert!(
        holds(&harness, host, SESSION, Kind::Waiting),
        "the premise: it is held"
    );
    harness.fleet.edit(host, |script| {
        script.sessions[0].status = SessionStatus::Running;
    });
    harness.refresh_to_completion(host).await;
    assert!(!holds(&harness, host, SESSION, Kind::Waiting));
    nothing_sent(&server);
}

/// Removing a host entry aborts its connection without its own teardown, and its blocked marks go by the database's
/// cascade rather than through `attention::cleared_marks`, so the removal must withdraw the host's alerts itself.
/// Specified: a held alert for a session on an ssh host is withdrawn when the host's entry is removed, a held alert on
/// another host stays, and nothing was sent.
#[farhelm_testtrace::test]
async fn removing_a_host_withdraws_its_held_alerts() {
    let server = stand_in(Answer::Success).await;
    let (builder, remote) = FleetBuilder::new()
        .await
        .ntfy_client(client())
        .ssh(
            "grace-remote.example",
            HostScript {
                identity: Some("grace-remote".to_string()),
                sessions: vec![session(SESSION, 1)],
                ..HostScript::default()
            },
        )
        .await;
    let harness = builder.start().await;
    let local = crate::rest_harness::local_id(&harness.store).await;
    harness.await_refreshed(remote).await;
    save_grace(&harness, &server, unreachable_grace()).await;
    harness.state.ntfy.enqueue(remote, settled_now());
    harness.state.ntfy.enqueue(local, settled_now());
    taken(&harness, 2).await;
    assert!(
        holds(&harness, remote, SESSION, Kind::Waiting)
            && holds(&harness, local, SESSION, Kind::Waiting),
        "the premise: both are held"
    );
    let response = crate::hosts::remove_host(
        axum::extract::State(Arc::clone(&harness.state)),
        axum::extract::Path(remote),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!holds(&harness, remote, SESSION, Kind::Waiting));
    assert!(
        holds(&harness, local, SESSION, Kind::Waiting),
        "another host's alert stays"
    );
    nothing_sent(&server);
}

/// The attention route dates an interaction by the age the desktop app reports, so a delayed signal withdraws only
/// what began before the user acted, and refuses an age no honest client sends. Specified: with a waiting alert held
/// for a listed session, an interaction aged 30 seconds (before the alert began) leaves it held; one aged over a
/// minute is refused with 400 and leaves it held; one with no age withdraws it; nothing was sent.
#[farhelm_testtrace::test]
async fn an_aged_interaction_withdraws_only_what_began_before_it() {
    let server = stand_in(Answer::Success).await;
    let harness = FleetBuilder::new()
        .await
        .ntfy_client(client())
        .local(HostScript {
            identity: Some("grace-aged".to_string()),
            sessions: vec![session(SESSION, 1)],
            ..HostScript::default()
        })
        .await
        .start()
        .await;
    let host = harness.store.list_hosts().await.unwrap()[0].id;
    harness.await_refreshed(host).await;
    save_grace(&harness, &server, unreachable_grace()).await;
    harness.state.ntfy.enqueue(host, settled_now());
    taken(&harness, 1).await;
    assert!(
        holds(&harness, host, SESSION, Kind::Waiting),
        "the premise: it is held"
    );
    let path = format!(
        "/api/sessions/{}/attention",
        harness.session_ref(host, SESSION)
    );
    let post = |body: serde_json::Value| harness.router().oneshot(request("POST", &path, body));
    let response = post(serde_json::json!({"kind": "interaction", "age_ms": 30_000}))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        holds(&harness, host, SESSION, Kind::Waiting),
        "attention from before the alert began withdraws nothing"
    );
    let response = post(serde_json::json!({
        "kind": "interaction",
        "age_ms": farhelm_proto::MAX_ATTENTION_AGE_MS + 1
    }))
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(holds(&harness, host, SESSION, Kind::Waiting));
    let response = post(serde_json::json!({"kind": "interaction"}))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!holds(&harness, host, SESSION, Kind::Waiting));
    nothing_sent(&server);
}

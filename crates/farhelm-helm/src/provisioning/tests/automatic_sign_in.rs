//! Drive automatic Update through the production listener, setup SSH relay and reply routes.
//! Only the transport peers and remote provisioning actions are scripted.

use super::super::setup_sign_in::Pending;
use super::*;
use crate::askpass::Questions;
use crate::transport::{HostTransport, TransportConnection};
use farhelm_proto::ui_channel::{
    PendingSshSignIn, SshConnectionPurpose, SshQuestion, SshSignInOutcome,
};
use std::future::Future;
use std::os::unix::fs::PermissionsExt as _;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};

const AUTH_DESTINATION: &str = "auth-update.example";
const BATCH_DESTINATION: &str = "batch-update.example";

/// Change only the Sessions preflight, so a host can lose sign-in and then reach the same old supervisor on Retry.
struct SessionsGate {
    fleet: Arc<crate::rest_harness::ScriptedFleet>,
    host: HostId,
    ready: AtomicBool,
}

impl HostTransport for SessionsGate {
    fn sign_in<'a>(
        &'a self,
        row: &'a HostRow,
        _deadline: Duration,
        _startup: bool,
        _question: &'a mut (dyn FnMut(bool) + Send),
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>> {
        Box::pin(std::future::ready(
            if row.id == self.host && !self.ready.load(Ordering::Relaxed) {
                Err(crate::ssh_sign_in::SignInNeeded.into())
            } else {
                Ok(())
            },
        ))
    }

    fn connect<'a>(
        &'a self,
        row: &'a HostRow,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<TransportConnection>> + Send + 'a>> {
        self.fleet.connect(row)
    }
}

struct Fixture {
    harness: Harness,
    service: Arc<ProvisioningService>,
    backend: Arc<FakeBackend>,
    questions: Arc<Questions>,
    sessions: Arc<SessionsGate>,
    host: HostId,
    batch_host: Option<HostId>,
    root: tempfile::TempDir,
}

impl Fixture {
    /// Both hosts need an update. Only the first needs the setup master to authenticate.
    /// Keep automatic updates off until the test subscribes and asserts the starting state.
    async fn new(skewed: bool, with_batch_host: bool) -> Self {
        let old = || HostScript {
            build: "1.0.0".into(),
            identity: None,
            ..Default::default()
        };
        let root = super::super::ssh_stand_in::control_root();
        let farhelm = root.path().join("farhelm");
        let state = root.path().join("state");
        let (builder, host) = FleetBuilder::new()
            .await
            .ssh_at(
                AUTH_DESTINATION,
                farhelm.to_str(),
                state.to_str(),
                HostScript {
                    protocol: farhelm_proto::PROTOCOL_VERSION - u32::from(skewed),
                    ..old()
                },
            )
            .await;
        let (builder, batch_host) = if with_batch_host {
            let (builder, batch_host) = builder
                .ssh_at(BATCH_DESTINATION, farhelm.to_str(), state.to_str(), old())
                .await;
            (builder, Some(batch_host))
        } else {
            (builder, None)
        };
        let mut harness = builder.start().await;
        harness.state._automatic_updates.stop();
        harness.manager.shutdown();
        let sessions = Arc::new(SessionsGate {
            fleet: Arc::clone(&harness.fleet),
            host,
            ready: AtomicBool::new(true),
        });
        harness.manager = ConnectionManager::start(
            harness.store.clone(),
            sessions.clone(),
            crate::manager::Cadence::default(),
        )
        .await
        .unwrap();
        harness
            .await_state(host, |state| {
                if skewed {
                    matches!(state, HostState::VersionSkew { .. })
                } else {
                    state.is_connected()
                }
            })
            .await;
        if let Some(batch_host) = batch_host {
            harness
                .await_state(batch_host, HostState::is_connected)
                .await;
        }
        harness
            .store
            .update_preferences(crate::store::PreferencePatch {
                auto_update_remote_hosts: Some(Some(false)),
                ..Default::default()
            })
            .await
            .unwrap();

        let program = root.path().join("ssh");
        // The files belong to this one test. Each askpass client leaves a launch receipt even if cancelled.
        std::fs::write(
            &program,
            r#"#!/usr/bin/env python3
import os
from pathlib import Path
import socket
import struct
import sys

root = Path(__file__).parent
master = root / "setup-master"
assert any(arg.startswith('ControlPath="') and arg.endswith('/p%C"') for arg in sys.argv)
if "-O" in sys.argv:
    sys.exit(0 if "batch-update.example" in sys.argv or master.is_file() else 255)
if "-G" in sys.argv:
    print(f"controlpath {master}")
    sys.exit(0)
assert "auth-update.example" in sys.argv
assert "BatchMode=no" in sys.argv and sys.argv[-1] == "true"
with (root / "launches").open("a") as launches:
    launches.write(f"{os.getpid()}\n")
prompt = b"Verification code:"
with socket.socket(socket.AF_UNIX) as connection:
    connection.connect(os.environ["FARHELM_SSH_ASKPASS_SOCKET"])
    connection.sendall(struct.pack("!BH", 0, len(prompt)) + prompt)
    with connection.makefile("rb") as response:
        size = struct.unpack("!H", response.read(2))[0]
        if response.read(size) != b"fixture-code":
            sys.exit(255)
master.touch()
"#,
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        *backend.probe.lock().unwrap() = None;
        *backend.repeat_probe.lock().unwrap() = Some(RepeatProbe {
            identity: None,
            dial_farhelm: farhelm,
            dial_state_dir: Some(state),
        });
        let mut service = service(&harness, Arc::clone(&backend), root.path());
        *service.helm_build_override.lock().unwrap() = Some("2.0.0".into());
        let questions = Arc::new(Questions::default());
        questions.publish_to(Arc::clone(harness.manager.events()));
        let relay = Arc::new(crate::ssh_sign_in::SignIn::new(
            root.path().into(),
            "/bin/true".into(),
            Arc::clone(&questions),
        ));
        Arc::get_mut(&mut service).unwrap().setup_sign_in = Some(Arc::new(
            super::super::setup_sign_in::SetupSignIn::new(relay, program),
        ));
        let mut state = AppState::with_provisioning(
            Arc::clone(&harness.manager),
            harness.store.clone(),
            Arc::clone(&service),
        );
        state.ssh_questions = Arc::clone(&questions);
        harness.state = Arc::new(state);
        Self {
            harness,
            service,
            backend,
            questions,
            sessions,
            host,
            batch_host,
            root,
        }
    }

    /// Use the same preference commit and fleet wake a desktop switch sends.
    async fn enabled(&self, enabled: bool) {
        let response = crate::preferences::put_preferences(
            State(Arc::clone(&self.harness.state)),
            axum::Json(
                serde_json::from_value(serde_json::json!({"auto_update_remote_hosts": enabled}))
                    .unwrap(),
            ),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    /// Wait for an actual question lease on the setup connection. A pending result alone proves no child asked.
    async fn question(&self) -> SshQuestion {
        let mut feed = self.harness.manager.events().subscribe();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(question) = self.questions.list().into_iter().find(|question| {
                    question.target.host() == Some(self.host)
                        && question.purpose == SshConnectionPurpose::Setup
                }) {
                    return question;
                }
                feed.changed().await.unwrap();
            }
        })
        .await
        .expect("the setup SSH client should be asking")
    }

    /// Answer or cancel only a question whose SSH child still holds its lease.
    async fn reply(&self, question: &SshQuestion, answer: serde_json::Value) {
        assert!(
            self.questions
                .list()
                .iter()
                .any(|entry| entry.id == question.id)
        );
        let (status, body) = send(
            &self.harness.router(),
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/api/ssh-questions/{}", question.id))
                .header("host", "127.0.0.1:7433")
                .header("content-type", "application/json")
                .body(Body::from(answer.to_string()))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    }

    /// Wait until the task publishes the exact attempt's terminal result, after releasing the question.
    async fn completion(&self, attempt: &str) -> SshSignInOutcome {
        let mut feed = self.harness.manager.events().subscribe();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let (status, body) = send(
                    &self.harness.router(),
                    axum::http::Request::builder()
                        .uri("/api/ssh-sign-in-completions")
                        .header("host", "127.0.0.1:7433")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await;
                assert_eq!(status, StatusCode::OK, "{body}");
                let body: serde_json::Value = serde_json::from_str(&body).unwrap();
                let completions: Vec<farhelm_proto::ui_channel::SshSignInCompletion> =
                    serde_json::from_value(body["completions"].clone()).unwrap();
                if let Some(entry) = completions
                    .into_iter()
                    .find(|entry| entry.attempt == attempt)
                {
                    return entry.outcome;
                }
                feed.changed().await.unwrap();
            }
        })
        .await
        .expect("the setup check must settle")
    }

    /// Count actual interactive children. Repeated control-socket checks do not create receipts.
    fn launches(&self) -> usize {
        std::fs::read_to_string(self.root.path().join("launches"))
            .unwrap_or_default()
            .lines()
            .count()
    }

    /// Separate planning from reply activation to put a second caller or policy commit between them.
    async fn prepare(&self) -> Pending {
        self.service
            .plan_update_for_ui(self.host)
            .await
            .unwrap_err()
            .downcast::<Pending>()
            .unwrap()
    }

    /// Exercise the same response finalization an ordinary Update or Uninstall uses after the host reply bound.
    async fn activate(&self, pending: Pending) -> PendingSshSignIn {
        let reply = activate_pending_sign_in(&self.service, provisioning_error(pending.into()));
        assert_eq!(reply.status(), StatusCode::ACCEPTED);
        let body = to_bytes(reply.into_body(), 4096).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    /// A successful fake installation must start reporting the new build on its own reconnect.
    fn update_build_on_restart(&self, host: HostId) {
        let fleet = Arc::clone(&self.harness.fleet);
        *self.backend.on_operation.lock().unwrap() = Some((
            "restart-supervisor",
            Box::new(move || fleet.edit(host, |script| script.build = "2.0.0".into())),
        ));
    }
}

/// Opening the SSH question must release the fleet scan. Answering it wakes the real listener without manual replay.
#[farhelm_testtrace::test]
async fn waiting_for_setup_sign_in_does_not_stall_other_hosts_and_ready_runs_update() {
    let fixture = Fixture::new(false, true).await;
    let batch = fixture.batch_host.unwrap();
    fixture.update_build_on_restart(batch);
    fixture.enabled(true).await;
    let question = fixture.question().await;
    assert!(!question.startup);
    assert_eq!(
        wait_finished(&fixture.service, batch).await.status,
        RunStatus::Completed
    );
    assert_eq!(
        fixture.service.view(fixture.host).await.unwrap().run_id,
        None
    );
    assert!(
        fixture
            .questions
            .list()
            .iter()
            .any(|entry| entry.id == question.id)
    );

    fixture.update_build_on_restart(fixture.host);
    fixture
        .reply(
            &question,
            serde_json::json!({"action":"send","answer":"fixture-code"}),
        )
        .await;
    assert_eq!(
        fixture.completion(&question.attempt).await,
        SshSignInOutcome::Ready
    );
    assert_eq!(
        wait_finished(&fixture.service, fixture.host).await.status,
        RunStatus::Completed
    );
    assert_eq!(
        fixture.launches(),
        1,
        "resumed batch planning shares the authenticated master"
    );
}

/// A successful Setup answer must reach the batch planner before the authenticated master can sit idle
/// behind unrelated fleet probes. Use the existing admission gate to observe completion of that plan.
#[farhelm_testtrace::test]
async fn ready_setup_sign_in_resumes_while_another_hosts_probe_is_held() {
    let fixture = Fixture::new(false, true).await;
    let batch = fixture.batch_host.unwrap();
    let gate = Arc::new(TestBoundaryGate {
        reached: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    *fixture.service.automatic_admission_gate.lock().unwrap() = Some((Arc::clone(&gate), None));
    fixture.backend.block_probe.store(true, Ordering::Relaxed);
    fixture.update_build_on_restart(fixture.host);
    fixture.enabled(true).await;
    let question = fixture.question().await;
    tokio::time::timeout(Duration::from_secs(5), fixture.backend.entered.notified())
        .await
        .expect("the other host has reached its held batch probe");
    fixture
        .reply(
            &question,
            serde_json::json!({"action":"send","answer":"fixture-code"}),
        )
        .await;
    assert_eq!(
        fixture.completion(&question.attempt).await,
        SshSignInOutcome::Ready
    );
    tokio::time::timeout(Duration::from_secs(5), gate.reached.notified())
        .await
        .expect("the signed-in host finishes batch planning without releasing the other probe");
    assert_eq!(fixture.service.view(batch).await.unwrap().run_id, None);
    gate.release.notify_one();
    assert_eq!(
        wait_finished(&fixture.service, fixture.host).await.status,
        RunStatus::Completed
    );
    fixture.update_build_on_restart(batch);
    fixture.backend.release.notify_one();
    assert_eq!(
        wait_finished(&fixture.service, batch).await.status,
        RunStatus::Completed
    );
    assert_eq!(fixture.launches(), 1);
}

/// A prepared update can outlive the host's eligibility while the connection token stays the same.
/// Admission must check the latest host state before it claims a run.
#[farhelm_testtrace::test]
async fn automatic_admission_declines_when_skew_gives_way_to_sign_in_needed() {
    let fixture = Fixture::new(true, false).await;
    let listener = &fixture.harness.state._automatic_updates;
    let mut scans = listener.completed_scans();
    listener.stop();
    wait_for_automatic_listener_end(&mut scans).await;
    fixture.enabled(true).await;

    let before = fixture.harness.manager.status(fixture.host).unwrap();
    assert!(matches!(before.state, HostState::VersionSkew { .. }));
    let plan = fixture.service.plan_update(fixture.host).await.unwrap();
    fixture.sessions.ready.store(false, Ordering::Relaxed);
    assert!(
        fixture
            .harness
            .manager
            .retry_now(fixture.host)
            .await
            .unwrap()
    );
    fixture
        .harness
        .await_state(fixture.host, |state| {
            matches!(state, HostState::SignInNeeded)
        })
        .await;
    assert_eq!(
        fixture
            .harness
            .manager
            .status(fixture.host)
            .unwrap()
            .incarnation,
        before.incarnation,
        "the earlier connection-token check alone would still allow this plan"
    );

    let accepted = fixture
        .service
        .start_automatic_update(
            fixture.host,
            before.entry_uuid,
            before.incarnation,
            ProvisionRequest {
                probe_id: plan.probe_id,
            },
        )
        .await
        .unwrap();
    assert!(accepted.is_none());
    assert!(
        fixture
            .service
            .view(fixture.host)
            .await
            .unwrap()
            .run_id
            .is_none()
    );
    assert!(fixture.backend.operations.lock().unwrap().is_empty());
}

/// A cancelled question consumes the old skewed connection's attempt. An explicit Sessions sign-in
/// can then reach an old supervisor again and open a fresh Update question.
#[farhelm_testtrace::test]
async fn cancelling_an_automatic_setup_question_waits_for_a_new_incarnation() {
    let fixture = Fixture::new(true, false).await;
    let mut scans = fixture.harness.state._automatic_updates.completed_scans();
    fixture.enabled(true).await;
    let question = fixture.question().await;
    let before = fixture
        .harness
        .manager
        .status(fixture.host)
        .unwrap()
        .incarnation;
    fixture
        .reply(&question, serde_json::json!({"action":"cancel"}))
        .await;
    assert!(matches!(
        fixture.completion(&question.attempt).await,
        SshSignInOutcome::Failed { .. }
    ));
    // Following completion, a fleet change wake must still be unable to admit another attempt on this connection.
    let expected = *scans.borrow_and_update() + 1;
    fixture.harness.manager.events().bump();
    wait_for_automatic_scan(&mut scans, expected).await;
    assert_eq!(fixture.launches(), 1);
    assert_eq!(fixture.backend.probes.load(Ordering::Relaxed), 0);

    fixture.sessions.ready.store(false, Ordering::Relaxed);
    assert!(
        fixture
            .harness
            .manager
            .retry_now(fixture.host)
            .await
            .unwrap()
    );
    fixture
        .harness
        .await_state(fixture.host, |state| {
            matches!(state, HostState::SignInNeeded)
        })
        .await;
    fixture.sessions.ready.store(true, Ordering::Relaxed);
    assert!(
        fixture
            .harness
            .manager
            .retry_now(fixture.host)
            .await
            .unwrap()
    );
    let resumed = fixture.question().await;
    assert_ne!(resumed.attempt, question.attempt);
    assert!(
        fixture
            .harness
            .manager
            .status(fixture.host)
            .unwrap()
            .incarnation
            > before
    );
    assert_eq!(fixture.launches(), 2);
    fixture
        .reply(&resumed, serde_json::json!({"action":"cancel"}))
        .await;
    assert!(matches!(
        fixture.completion(&resumed.attempt).await,
        SshSignInOutcome::Failed { .. }
    ));
}

/// Turning off automatic updates cancels only checks without a manual owner. The manual 202 must name
/// the live question so the desktop can open it. The next opt-in can ask again on the same connection.
#[farhelm_testtrace::test]
async fn opt_out_cancels_only_automatic_checks_and_leaves_the_connection_eligible() {
    let fixture = Fixture::new(false, false).await;
    fixture.enabled(true).await;
    let initial = fixture.question().await;
    let incarnation = fixture
        .harness
        .manager
        .status(fixture.host)
        .unwrap()
        .incarnation;
    fixture.enabled(false).await;
    assert!(matches!(
        fixture.completion(&initial.attempt).await,
        SshSignInOutcome::Failed { .. }
    ));
    assert!(fixture.questions.list().is_empty());

    fixture.enabled(true).await;
    let joined = fixture.question().await;
    assert_ne!(joined.attempt, initial.attempt);
    assert_eq!(
        fixture
            .harness
            .manager
            .status(fixture.host)
            .unwrap()
            .incarnation,
        incarnation
    );
    let (status, body) = send(
        &fixture.harness.router(),
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/api/hosts/{}/update", fixture.host))
            .header("host", "127.0.0.1:7433")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let pending: PendingSshSignIn = serde_json::from_str(&body).unwrap();
    assert_eq!(pending.attempt, joined.attempt);

    let mut scans = fixture.harness.state._automatic_updates.completed_scans();
    let expected = *scans.borrow_and_update() + 1;
    fixture.enabled(false).await;
    wait_for_automatic_scan(&mut scans, expected).await;
    fixture
        .reply(
            &joined,
            serde_json::json!({"action":"send","answer":"fixture-code"}),
        )
        .await;
    assert_eq!(
        fixture.completion(&joined.attempt).await,
        SshSignInOutcome::Ready
    );
    assert_eq!(
        fixture.launches(),
        2,
        "the manual request reused the live check"
    );
    assert_eq!(
        fixture.service.view(fixture.host).await.unwrap().run_id,
        None
    );
}

/// Two callers can both see an absent setup master before either starts. The second caller receives the
/// first caller's id even when their prepared ids differed, and can keep the question alive on its own.
#[farhelm_testtrace::test]
async fn racing_preparations_return_the_single_winning_setup_attempt() {
    let fixture = Fixture::new(false, false).await;
    let automatic = fixture.prepare().await;
    let manual = fixture.prepare().await;
    assert_ne!(automatic.attempt.id, manual.attempt.id);
    let setup = fixture.service.setup_sign_in.as_ref().unwrap();
    let winning = automatic.attempt.id.clone();
    let owner = setup.start_automatic(automatic, &fixture.service);
    assert_eq!(fixture.activate(manual).await.attempt, winning);
    drop(owner);
    let question = fixture.question().await;
    assert_eq!(question.attempt, winning);
    fixture
        .reply(&question, serde_json::json!({"action":"cancel"}))
        .await;
    assert!(matches!(
        fixture.completion(&winning).await,
        SshSignInOutcome::Failed { .. }
    ));
    assert_eq!(fixture.launches(), 1);
}

/// A durable opt-out cannot be lost to the fleet watch while the scan is busy preparing another host.
#[farhelm_testtrace::test]
async fn off_and_on_while_the_scan_is_held_releases_the_old_question() {
    let fixture = Fixture::new(false, true).await;
    fixture.update_build_on_restart(fixture.batch_host.unwrap());
    fixture.backend.block_probe.store(true, Ordering::Relaxed);
    fixture.enabled(true).await;
    let old = fixture.question().await;
    tokio::time::timeout(Duration::from_secs(5), fixture.backend.entered.notified())
        .await
        .expect("the scan is now waiting for the other host");

    fixture.enabled(false).await;
    fixture.enabled(true).await;
    assert!(matches!(
        fixture.completion(&old.attempt).await,
        SshSignInOutcome::Failed { .. }
    ));
    assert!(fixture.questions.list().is_empty());
    fixture.backend.release.notify_one();
    let new = fixture.question().await;
    assert_ne!(new.attempt, old.attempt);
    fixture
        .reply(&new, serde_json::json!({"action":"cancel"}))
        .await;
    assert!(matches!(
        fixture.completion(&new.attempt).await,
        SshSignInOutcome::Failed { .. }
    ));
}

/// A manual reply prepared against an open automatic prompt must remain useful even if the auto-only
/// prompt is released by opt-out before the HTTP reply is activated.
#[farhelm_testtrace::test]
async fn manual_reply_replaces_a_question_released_after_prepare() {
    let fixture = Fixture::new(false, false).await;
    fixture.enabled(true).await;
    let old = fixture.question().await;
    let manual = fixture.prepare().await;
    assert_eq!(manual.attempt.id, old.attempt);

    fixture.enabled(false).await;
    let new = fixture.activate(manual).await;
    assert_ne!(new.attempt, old.attempt);
    assert!(matches!(
        fixture.completion(&old.attempt).await,
        SshSignInOutcome::Failed { .. }
    ));
    let question = fixture.question().await;
    assert_eq!(question.attempt, new.attempt);
    fixture
        .reply(&question, serde_json::json!({"action":"cancel"}))
        .await;
    assert!(matches!(
        fixture.completion(&new.attempt).await,
        SshSignInOutcome::Failed { .. }
    ));
}

/// The listener may see an opt-out before it observes a user's earlier cancellation. Once SSH reported
/// failure, toggling the preference must preserve that failure on the same connection.
#[farhelm_testtrace::test]
async fn opt_out_preserves_a_failure_that_settled_before_the_scan() {
    let fixture = Fixture::new(false, false).await;
    let listener = &fixture.harness.state._automatic_updates;
    let mut scans = listener.completed_scans();
    listener.stop();
    wait_for_automatic_listener_end(&mut scans).await;
    let mut attempts = HashMap::new();
    fixture.enabled(true).await;
    fixture
        .service
        .scan_automatic_updates(&mut attempts)
        .await
        .unwrap();
    let question = fixture.question().await;
    fixture
        .reply(&question, serde_json::json!({"action":"cancel"}))
        .await;
    assert!(matches!(
        fixture.completion(&question.attempt).await,
        SshSignInOutcome::Failed { .. }
    ));
    fixture.enabled(false).await;
    fixture
        .service
        .scan_automatic_updates(&mut attempts)
        .await
        .unwrap();
    assert!(matches!(
        attempts.get(&fixture.host),
        Some(AttemptState::Attempted(_))
    ));
    fixture.enabled(true).await;
    fixture
        .service
        .scan_automatic_updates(&mut attempts)
        .await
        .unwrap();
    assert!(matches!(
        attempts.get(&fixture.host),
        Some(AttemptState::Attempted(_))
    ));
    assert_eq!(fixture.launches(), 1);
}

/// If a planned automatic run loses its setup master before revalidation, the batch probe fails
/// as ordinary provisioning progress. That run must not open a second verification question.
#[farhelm_testtrace::test]
async fn an_automatic_run_with_an_expired_master_fails_without_asking_again() {
    let fixture = Fixture::new(false, false).await;
    let gate = Arc::new(TestBoundaryGate {
        reached: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    *fixture.service.automatic_admission_gate.lock().unwrap() = Some((Arc::clone(&gate), None));
    fixture.enabled(true).await;
    let question = fixture.question().await;
    fixture
        .reply(
            &question,
            serde_json::json!({"action":"send","answer":"fixture-code"}),
        )
        .await;
    assert_eq!(
        fixture.completion(&question.attempt).await,
        SshSignInOutcome::Ready
    );
    tokio::time::timeout(Duration::from_secs(5), gate.reached.notified())
        .await
        .expect("premise: resumed planning finished with the master live");
    std::fs::remove_file(fixture.root.path().join("setup-master")).unwrap();
    *fixture.backend.probe.lock().unwrap() =
        Some(Err(BackendFailure::new("fixture setup master expired", "")));
    gate.release.notify_one();

    let result = wait_finished(&fixture.service, fixture.host).await;
    assert_eq!(result.status, RunStatus::Failed);
    assert!(
        result
            .message
            .unwrap()
            .contains("fixture setup master expired")
    );
    assert_eq!(fixture.launches(), 1);
    assert!(fixture.questions.list().is_empty());
}

/// Stopping the automatic listener must end its idle SSH child and withdraw the question, even with
/// no preference or host event left to run another scan.
#[farhelm_testtrace::test]
async fn stopping_the_listener_cancels_its_automatic_setup_question() {
    let fixture = Fixture::new(false, false).await;
    let listener = &fixture.harness.state._automatic_updates;
    let mut scans = listener.completed_scans();
    fixture.enabled(true).await;
    let question = fixture.question().await;
    listener.stop();
    wait_for_automatic_listener_end(&mut scans).await;
    assert!(matches!(
        fixture.completion(&question.attempt).await,
        SshSignInOutcome::Failed { .. }
    ));
    assert!(fixture.questions.list().is_empty());
    assert_eq!(fixture.launches(), 1);
}

//! Claude's durable process authority, exercised through the report dispatcher.
//!
//! These tests supply recorded, already-anchored ancestry. Process walking and
//! pane anchoring belong to corridor and end-to-end tests; the live test process
//! supplies real liveness only where admission must decide whether to release a pin.

use super::*;
use crate::agent_kind::report_pin::ReportPin;
use crate::procs::{ChainLink, ProcessIdentity};
use crate::store::StoredSessionNotification;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Seed a historical Claude capture in both representations without launching tmux.
/// Its version-0 Resume offer is the upgrade contract: introducing a process pin
/// must not withdraw a capture that already worked before the pin existed.
async fn seed_claude_pin_session(sup: &Arc<Supervisor>, id: &str) {
    let integration = program_snapshot("claude");
    let launch = farhelm_proto::SessionLaunch::Legacy {
        invocation: "claude".into(),
        agent_kind: AgentKind::Claude,
        resume_template: integration.resume_template.clone(),
    };
    let row = StoredSession {
        id: id.into(),
        parent: None,
        title: "Claude pin fixture".into(),
        created_at: now_unix(),
        last_activity_at: now_unix(),
        last_work_started_at: 0,
        creation_seq: 0,
        cwd: "/tmp".into(),
        launch: launch.clone(),
        tmux_name: format!("fh-{id}"),
        pane: String::new(),
        outcome: LastOutcome::Running,
        canonical_cwd: None,
        captured_conversation: Some("historical-claude".into()),
        generation: 0,
        launch_scoped: false,
        conversation_source: None,
        capture_ownership_version: 0,
        omp_reporter_asset: None,
        omp_launch_program: None,
        launch_hooked: false,
        launch_boot_ended: false,
    };
    sup.store.insert_session(row, None).await.unwrap();
    let mut entry = entry_with(None, LastOutcome::Running);
    entry.info.id = id.into();
    entry.info.agent_kind = AgentKind::Claude;
    entry.info.invocation = "claude".into();
    entry.info.launch = launch.clone();
    entry.snapshot = IntegrationSnapshot::of(&launch);
    entry.generation = 0;
    *entry.run.capture.lock().unwrap() = CaptureState::Reported {
        conversation: "historical-claude".into(),
        ownership_version: 0,
    };
    sup.sessions.lock().await.insert(id.into(), Arc::new(entry));
    let observed = observe_claude_pin(sup, id).await;
    assert_eq!(observed.conversation.as_deref(), Some("historical-claude"));
    assert_eq!(observed.source, None);
    assert_eq!(observed.pin, None);
    assert_eq!(observed.row_offer, RestartOffer::Resume);
    assert_eq!(observed.mirror_offer, RestartOffer::Resume);
    assert!(observed.notifications.is_empty());
}

/// Build the mutation-free evidence admission receives after the report drain.
/// Runtime images deliberately convey no Claude identity. Synthetic identities
/// describe saved evidence only; no test relies on a synthetic PID being alive.
fn claude_pin_report(conversation: &str, processes: &[ProcessIdentity]) -> ReportedConversation {
    assert!(!processes.is_empty(), "a report needs an emitter");
    let mut chain = vec![ChainLink {
        pid: 999_001,
        ppid: processes[0].pid,
        start: 1,
        exe: b"/opt/test/farhelm".to_vec(),
        argv: Some(
            [
                "/opt/test/farhelm",
                "internal",
                "hook",
                "--vendor",
                "claude",
            ]
            .iter()
            .map(|word| word.as_bytes().to_vec())
            .collect(),
        ),
        cwd: None,
    }];
    chain.extend(
        processes
            .iter()
            .enumerate()
            .map(|(index, process)| ChainLink {
                pid: process.pid,
                ppid: processes.get(index + 1).map_or(0, |parent| parent.pid),
                start: process.start,
                exe: b"/opt/test/arbitrary-runtime".to_vec(),
                argv: Some(vec![b"arbitrary-runtime".to_vec(), b"--fixture".to_vec()]),
                cwd: None,
            }),
    );
    ReportedConversation {
        vendor: farhelm_proto::ReportVendor::Claude,
        conversation: conversation.into(),
        source: "startup".into(),
        transcript_path: None,
        hook_event_name: Some(serde_json::json!("SessionStart")),
        ancestry: Some(chain),
        launch_generation: Some(0),
    }
}

/// Observe every admission-owned result together, including the published offer.
/// Equality before and after a refusal catches partial writes and speculative
/// mirrors; matching an error string alone would not establish that guarantee.
#[derive(Debug, PartialEq, Eq)]
struct ClaudePinState {
    conversation: Option<String>,
    source: Option<String>,
    version: i64,
    generation: i64,
    pin: Option<String>,
    mirror_conversation: Option<String>,
    mirror_version: Option<i64>,
    row_offer: RestartOffer,
    mirror_offer: RestartOffer,
    notifications: Vec<StoredSessionNotification>,
    mirror_notifications: Vec<StoredSessionNotification>,
}

/// Compare durable and published evidence without retaining a lock across I/O.
async fn observe_claude_pin(sup: &Arc<Supervisor>, id: &str) -> ClaudePinState {
    let row = sup.store.session(id).await.unwrap().unwrap();
    let pin = sup.store.report_pin(id).await.unwrap();
    let notifications = sup.store.session_notification_records(id).await.unwrap();
    let entry = sup.sessions.lock().await.get(id).unwrap().clone();
    let (mirror_conversation, mirror_version, mirror_offer) = {
        let capture = entry.run.capture.lock().unwrap();
        (
            capture.committed_conversation().map(str::to_owned),
            capture.committed_ownership_version(),
            entry.snapshot.restart_offer(
                capture.committed_conversation(),
                capture.committed_ownership_version().unwrap_or(0),
            ),
        )
    };
    let mirror_notifications = entry.session.notifications.lock().unwrap().clone();
    let row_offer = IntegrationSnapshot::of(&row.launch).restart_offer(
        row.captured_conversation.as_deref(),
        row.capture_ownership_version,
    );
    assert_eq!(row.captured_conversation, mirror_conversation);
    assert_eq!(mirror_version, Some(row.capture_ownership_version));
    assert_eq!(notifications, mirror_notifications);
    ClaudePinState {
        conversation: row.captured_conversation,
        source: row.conversation_source,
        version: row.capture_ownership_version,
        generation: row.generation,
        pin,
        mirror_conversation,
        mirror_version,
        row_offer,
        mirror_offer,
        notifications,
        mirror_notifications,
    }
}

/// Assert a successful report kept Claude's old Resume contract while changing
/// authority. The decoded process and ancestor list distinguish the winning
/// report from a conversation-only write or an accidental version-1 promotion.
async fn assert_claude_pin(
    sup: &Arc<Supervisor>,
    id: &str,
    conversation: &str,
    process: ProcessIdentity,
    ancestors: &[ProcessIdentity],
) -> ClaudePinState {
    let observed = observe_claude_pin(sup, id).await;
    assert_eq!(observed.conversation.as_deref(), Some(conversation));
    assert_eq!(observed.source.as_deref(), Some("hook"));
    assert_eq!(observed.version, 0);
    assert_eq!(observed.row_offer, RestartOffer::Resume);
    assert_eq!(observed.mirror_offer, RestartOffer::Resume);
    let pin: ReportPin = serde_json::from_str(observed.pin.as_deref().unwrap()).unwrap();
    assert_eq!(pin.process, process);
    assert_eq!(pin.ancestors, ancestors);
    observed
}

/// Child-first selection is reversible without retracting historical Resume.
/// Repeated reports retain earlier correction authority, and successive ancestors
/// share one unresolved warning describing the earlier mistaken Restart target.
#[farhelm_testtrace::test]
async fn claude_pin_corrects_ancestors_once_and_keeps_version_zero_resume() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .unwrap();
    let id = "claude-pin-correction";
    seed_claude_pin_session(&sup, id).await;
    let parent = ProcessIdentity::read(std::process::id()).unwrap();
    let child = ProcessIdentity {
        pid: 900_010,
        start: 10,
    };
    let higher = ProcessIdentity {
        pid: 900_011,
        start: 11,
    };
    sup.report_conversation(id, claude_pin_report("child", &[child, parent, higher]))
        .await
        .unwrap();
    let first = assert_claude_pin(&sup, id, "child", child, &[parent, higher]).await;
    assert!(first.notifications.is_empty());
    sup.report_conversation(id, claude_pin_report("child-after-clear", &[child]))
        .await
        .unwrap();
    let repeated = assert_claude_pin(&sup, id, "child-after-clear", child, &[parent, higher]).await;
    assert_eq!(
        repeated.pin, first.pin,
        "a shorter repeat must preserve the exact original pin"
    );
    sup.report_conversation(id, claude_pin_report("parent", &[parent, higher]))
        .await
        .unwrap();
    let corrected = assert_claude_pin(&sup, id, "parent", parent, &[higher]).await;
    assert_eq!(corrected.notifications.len(), 1);
    let notice = &corrected.notifications[0];
    assert_eq!(notice.kind, "claude_pin_corrected");
    assert_eq!(notice.generation, 0);
    assert_eq!(
        notice.notification.text,
        crate::service::notifications::CLAUDE_PIN_CORRECTED_TEXT
    );
    assert!(!notice.notification.resolved);
    sup.report_conversation(id, claude_pin_report("higher", &[higher]))
        .await
        .unwrap();
    let higher_state = assert_claude_pin(&sup, id, "higher", higher, &[]).await;
    assert_eq!(higher_state.notifications, corrected.notifications);
}

/// A live emitter retains authority against both direct and shelled descendants.
/// An unrelated reporter and a reused PID cannot acquire that authority either;
/// each refusal must leave the complete durable and published result unchanged.
#[farhelm_testtrace::test]
async fn claude_pin_refuses_descendants_rivals_and_reused_pids() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .unwrap();
    let id = "claude-pin-live";
    seed_claude_pin_session(&sup, id).await;
    let parent = ProcessIdentity::read(std::process::id()).unwrap();
    let child = ProcessIdentity {
        pid: 900_020,
        start: 20,
    };
    let shell = ProcessIdentity {
        pid: 900_021,
        start: 21,
    };
    let reused = ProcessIdentity {
        start: parent.start.wrapping_add(1),
        ..parent
    };
    sup.report_conversation(id, claude_pin_report("parent", &[parent]))
        .await
        .unwrap();
    let before = assert_claude_pin(&sup, id, "parent", parent, &[]).await;
    for (name, processes) in [
        ("direct-child", vec![child, parent]),
        ("shelled-child", vec![child, shell, parent]),
        ("unrelated", vec![child]),
        ("reused-pid", vec![reused]),
    ] {
        let mut report = claude_pin_report(name, &processes);
        if name == "shelled-child" {
            let shell_link = &mut report.ancestry.as_mut().unwrap()[2];
            shell_link.exe = b"/bin/sh".to_vec();
            shell_link.argv = Some(vec![b"sh".to_vec(), b"/work/tool-command".to_vec()]);
        }
        let error = sup.report_conversation(id, report).await.unwrap_err();
        assert_eq!(error.kind, ErrorKind::Conflict, "{name}: {error:?}");
        assert!(
            error.message.contains("pinned emitter"),
            "{name}: {error:?}"
        );
        assert_eq!(
            observe_claude_pin(&sup, id).await,
            before,
            "{name} mutated authority"
        );
    }
}

/// A dead pin releases only for a report outside its recorded descendant chain.
/// The current test process with a different start token proves PID reuse without
/// guessing whether a synthetic PID exists; exec-like image changes do not erase
/// that same identity from a descendant's ancestry.
#[farhelm_testtrace::test]
async fn claude_pin_releases_dead_authority_only_outside_its_chain() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .unwrap();
    let id = "claude-pin-release";
    seed_claude_pin_session(&sup, id).await;
    let live = ProcessIdentity::read(std::process::id()).unwrap();
    let dead = ProcessIdentity {
        start: live.start.wrapping_add(1),
        ..live
    };
    let fresh = ProcessIdentity {
        pid: 900_030,
        start: 30,
    };
    sup.report_conversation(id, claude_pin_report("original", &[live]))
        .await
        .unwrap();
    let row = sup.store.session(id).await.unwrap().unwrap();
    let old_pin = sup.store.report_pin(id).await.unwrap();
    let dead_pin = serde_json::to_string(&ReportPin {
        process: dead,
        ancestors: vec![],
    })
    .unwrap();
    assert!(
        sup.store
            .admit_pinned_conversation(&row, old_pin.as_deref(), "original", &dead_pin, 0, None,)
            .await
            .unwrap()
    );
    let before = assert_claude_pin(&sup, id, "original", dead, &[]).await;
    for changed_image in [false, true] {
        let mut report = claude_pin_report("descendant", &[fresh, dead]);
        if changed_image {
            let link = &mut report.ancestry.as_mut().unwrap()[2];
            link.exe = b"/bin/sh".to_vec();
            link.argv = Some(vec![b"sh".to_vec()]);
        }
        let error = sup.report_conversation(id, report).await.unwrap_err();
        assert!(error.message.contains("pinned emitter"), "{error:?}");
        assert_eq!(observe_claude_pin(&sup, id).await, before);
    }
    sup.report_conversation(id, claude_pin_report("fresh", &[fresh]))
        .await
        .unwrap();
    let released = assert_claude_pin(&sup, id, "fresh", fresh, &[]).await;
    assert!(
        released.notifications.is_empty(),
        "ordinary release is not a correction"
    );
}

/// Unavailable process evidence and failed adoption never publish authority.
/// The liveness and write seams distinguish those failures from a normal rival
/// refusal; ancestor correction reaches adoption while process reads are
/// unavailable because recorded ancestor authority precedes pin liveness.
#[farhelm_testtrace::test]
async fn claude_pin_preserves_all_state_on_evidence_validation_and_write_failures() {
    let state = StateDir::new();
    let fail_process = Arc::new(AtomicBool::new(true));
    let process_reads = Arc::new(AtomicUsize::new(0));
    let fail_write = Arc::new(AtomicBool::new(false));
    let sup = Supervisor::new_with_seams(
        state.path(),
        dummy_exe(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            faults: FaultHooks {
                claude_pin_process_fault: Some(Arc::new({
                    let fail = fail_process.clone();
                    let reads = process_reads.clone();
                    move |_| {
                        reads.fetch_add(1, Ordering::SeqCst);
                        fail.load(Ordering::SeqCst)
                            .then(|| "controlled process evidence unavailable".into())
                    }
                })),
                capture_store_fault: Some(Arc::new({
                    let fail = fail_write.clone();
                    move |write, _| {
                        if write == crate::service::capture::CaptureWrite::Report
                            && fail.load(Ordering::SeqCst)
                        {
                            anyhow::bail!("controlled capture write failure");
                        }
                        Ok(())
                    }
                })),
                ..FaultHooks::default()
            },
            ..SupervisorSeams::default()
        },
    )
    .await
    .unwrap();
    let id = "claude-pin-failure";
    seed_claude_pin_session(&sup, id).await;
    let parent = ProcessIdentity::read(std::process::id()).unwrap();
    let child = ProcessIdentity {
        pid: 900_040,
        start: 40,
    };
    let rival = ProcessIdentity {
        pid: 900_041,
        start: 41,
    };
    sup.report_conversation(id, claude_pin_report("child", &[child, parent]))
        .await
        .unwrap();
    assert_eq!(
        process_reads.load(Ordering::SeqCst),
        0,
        "initial admission needs no liveness read"
    );
    let before = assert_claude_pin(&sup, id, "child", child, &[parent]).await;
    let error = sup
        .report_conversation(id, claude_pin_report("rival", &[rival]))
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);
    assert!(
        error
            .message
            .contains("controlled process evidence unavailable"),
        "{error:?}"
    );
    assert_eq!(process_reads.load(Ordering::SeqCst), 1);
    assert_eq!(
        observe_claude_pin(&sup, id).await,
        before,
        "unavailable evidence released the pin"
    );

    let mut invalid = claude_pin_report("bad\nid", &[parent]);
    let error = sup.report_conversation(id, invalid).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidRequest);
    assert_eq!(observe_claude_pin(&sup, id).await, before);
    invalid = claude_pin_report("parent", &[parent]);
    invalid.ancestry = None;
    let error = sup.report_conversation(id, invalid).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);
    assert!(error.message.contains("no process ancestry"), "{error:?}");
    assert_eq!(observe_claude_pin(&sup, id).await, before);
    invalid = claude_pin_report("parent", &[parent]);
    invalid.launch_generation = Some(1);
    let error = sup.report_conversation(id, invalid).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);
    assert!(error.message.contains("another launch"), "{error:?}");
    assert_eq!(observe_claude_pin(&sup, id).await, before);

    fail_write.store(true, Ordering::SeqCst);
    let error = sup
        .report_conversation(id, claude_pin_report("parent", &[parent]))
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Internal);
    assert_eq!(
        process_reads.load(Ordering::SeqCst),
        1,
        "correction must precede liveness reads"
    );
    assert_eq!(
        observe_claude_pin(&sup, id).await,
        before,
        "failed correction published partial state"
    );
    fail_write.store(false, Ordering::SeqCst);
    fail_process.store(false, Ordering::SeqCst);
    sup.report_conversation(id, claude_pin_report("parent", &[parent]))
        .await
        .unwrap();
    let corrected = assert_claude_pin(&sup, id, "parent", parent, &[]).await;
    assert_eq!(process_reads.load(Ordering::SeqCst), 1);
    assert_eq!(corrected.notifications.len(), 1);
    assert_eq!(corrected.notifications[0].kind, "claude_pin_corrected");
    assert!(!corrected.notifications[0].notification.resolved);
}

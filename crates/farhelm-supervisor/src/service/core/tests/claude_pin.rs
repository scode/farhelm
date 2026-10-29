//! Claude and Muse's durable authority, exercised through the report dispatcher.
//!
//! These tests supply recorded, already-anchored ancestry. Process walking and
//! pane anchoring belong to corridor and end-to-end tests; the live test process
//! supplies real liveness only where admission must decide whether to release a pin.

use super::*;
use crate::agent_kind::report_pin::ReportPin;
use crate::procs::{ChainLink, ProcessIdentity};
use crate::store::StoredSessionNotification;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Each vendor runs the same authority scenarios through its own public dispatch.
/// The notice expectation stays separate so shared admission cannot mislabel Muse.
#[derive(Clone, Copy)]
struct PinCase {
    kind: AgentKind,
    vendor: farhelm_proto::ReportVendor,
    resume_selector: &'static str,
    notice_kind: &'static str,
    notice_text: &'static str,
}

const CLAUDE: PinCase = PinCase {
    kind: AgentKind::Claude,
    vendor: farhelm_proto::ReportVendor::Claude,
    resume_selector: "--resume",
    notice_kind: "claude_pin_corrected",
    notice_text: crate::service::notifications::CLAUDE_PIN_CORRECTED_TEXT,
};
const MUSE: PinCase = PinCase {
    kind: AgentKind::Muse,
    vendor: farhelm_proto::ReportVendor::Muse,
    resume_selector: "resume",
    notice_kind: "muse_pin_corrected",
    notice_text: crate::service::notifications::MUSE_PIN_CORRECTED_TEXT,
};

/// Seed a historical plain-id capture in both representations without tmux.
/// Its version-0 Resume offer is the upgrade contract: introducing a process pin
/// must not withdraw a capture that already worked before the pin existed.
async fn seed_pin_session(sup: &Arc<Supervisor>, id: &str, case: PinCase) {
    let integration = IntegrationSnapshot {
        kind: case.kind,
        wrapper_prefix_words: 0,
        resume_template: Some(vec![
            case.kind.word().into(),
            case.resume_selector.into(),
            crate::agent_kind::CONVERSATION_PLACEHOLDER.into(),
        ]),
    };
    let launch = farhelm_proto::SessionLaunch::Legacy {
        invocation: case.kind.word().into(),
        agent_kind: case.kind,
        resume_template: integration.resume_template.clone(),
    };
    let row = StoredSession {
        id: id.into(),
        parent: None,
        title: "Positional pin fixture".into(),
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
    entry.info.agent_kind = case.kind;
    entry.info.invocation = case.kind.word().into();
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
/// Runtime images deliberately convey no vendor identity. Synthetic identities
/// describe saved evidence only; no test relies on a synthetic PID being alive.
fn positional_pin_report(
    case: PinCase,
    conversation: &str,
    processes: &[ProcessIdentity],
) -> ReportedConversation {
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
                case.kind.word(),
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
        vendor: case.vendor,
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

/// Assert a successful report kept the version-0 Resume contract while changing
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
async fn pin_corrects_ancestors_once_and_keeps_version_zero_resume(case: PinCase) {
    let claude_pin_report = |conversation: &str, processes: &[ProcessIdentity]| {
        positional_pin_report(case, conversation, processes)
    };
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .unwrap();
    let id = "claude-pin-correction";
    seed_pin_session(&sup, id, case).await;
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
    assert_eq!(notice.kind, case.notice_kind);
    assert_eq!(notice.generation, 0);
    assert_eq!(notice.notification.text, case.notice_text);
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
async fn pin_refuses_descendants_rivals_and_reused_pids(case: PinCase) {
    let claude_pin_report = |conversation: &str, processes: &[ProcessIdentity]| {
        positional_pin_report(case, conversation, processes)
    };
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .unwrap();
    let id = "claude-pin-live";
    seed_pin_session(&sup, id, case).await;
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
async fn pin_releases_dead_authority_only_outside_its_chain(case: PinCase) {
    let claude_pin_report = |conversation: &str, processes: &[ProcessIdentity]| {
        positional_pin_report(case, conversation, processes)
    };
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .unwrap();
    let id = "claude-pin-release";
    seed_pin_session(&sup, id, case).await;
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
async fn pin_preserves_all_state_on_evidence_validation_and_write_failures(case: PinCase) {
    let claude_pin_report = |conversation: &str, processes: &[ProcessIdentity]| {
        positional_pin_report(case, conversation, processes)
    };
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
    seed_pin_session(&sup, id, case).await;
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
    assert_eq!(corrected.notifications[0].kind, case.notice_kind);
    assert!(!corrected.notifications[0].notification.resolved);
}

/// These vendors' Stop carries a complete current identity, rather than Codex's
/// confirmation-only evidence. Keeping source empty matches the callback shape
/// whose admission must not depend on a preceding SessionStart reaching Farhelm.
fn positional_stop_report(
    case: PinCase,
    conversation: &str,
    processes: &[ProcessIdentity],
) -> ReportedConversation {
    let mut report = positional_pin_report(case, conversation, processes);
    report.source.clear();
    report.hook_event_name = Some(serde_json::json!("Stop"));
    report
}

/// Stop can recover an unpinned launch and select its later current conversation.
/// A shorter repeat must retain the first report's correction ancestors, and
/// neither selection introduces transcript confirmation or version-1 provenance.
async fn stop_selects_without_session_start_and_preserves_pin_ancestors(case: PinCase) {
    let claude_stop_report = |conversation: &str, processes: &[ProcessIdentity]| {
        positional_stop_report(case, conversation, processes)
    };
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .unwrap();
    let id = "claude-stop-selection";
    seed_pin_session(&sup, id, case).await;
    let emitter = ProcessIdentity::read(std::process::id()).unwrap();
    let ancestor = ProcessIdentity {
        pid: 900_050,
        start: 50,
    };
    assert_eq!(sup.store.report_pin(id).await.unwrap(), None);
    sup.report_conversation(id, claude_stop_report("stop-first", &[emitter, ancestor]))
        .await
        .unwrap();
    let first = assert_claude_pin(&sup, id, "stop-first", emitter, &[ancestor]).await;
    assert!(first.notifications.is_empty());

    sup.report_conversation(id, claude_stop_report("stop-current", &[emitter]))
        .await
        .unwrap();
    let current = assert_claude_pin(&sup, id, "stop-current", emitter, &[ancestor]).await;
    assert_eq!(
        current.pin, first.pin,
        "Stop must preserve the original serialized authority"
    );
    assert!(current.notifications.is_empty());
}

/// An ancestor's end-of-turn report repairs child-first capture immediately.
/// A second, higher ancestor may correct again, but the launch retains one
/// unresolved warning about what an earlier Restart would have resumed.
async fn stop_corrects_child_first_capture_and_notifies_once(case: PinCase) {
    let claude_pin_report = |conversation: &str, processes: &[ProcessIdentity]| {
        positional_pin_report(case, conversation, processes)
    };
    let claude_stop_report = |conversation: &str, processes: &[ProcessIdentity]| {
        positional_stop_report(case, conversation, processes)
    };
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .unwrap();
    let id = "claude-stop-correction";
    seed_pin_session(&sup, id, case).await;
    let parent = ProcessIdentity::read(std::process::id()).unwrap();
    let child = ProcessIdentity {
        pid: 900_060,
        start: 60,
    };
    let higher = ProcessIdentity {
        pid: 900_061,
        start: 61,
    };
    sup.report_conversation(id, claude_pin_report("child", &[child, parent, higher]))
        .await
        .unwrap();
    let child_state = assert_claude_pin(&sup, id, "child", child, &[parent, higher]).await;
    assert!(
        child_state.notifications.is_empty(),
        "the child-first premise must be observable"
    );

    sup.report_conversation(id, claude_stop_report("parent", &[parent, higher]))
        .await
        .unwrap();
    let parent_state = assert_claude_pin(&sup, id, "parent", parent, &[higher]).await;
    assert_eq!(parent_state.notifications.len(), 1);
    let notice = &parent_state.notifications[0];
    assert_eq!(notice.kind, case.notice_kind);
    assert_eq!(notice.generation, 0);
    assert_eq!(notice.notification.text, case.notice_text);
    assert!(!notice.notification.resolved);

    sup.report_conversation(id, claude_stop_report("higher", &[higher]))
        .await
        .unwrap();
    let higher_state = assert_claude_pin(&sup, id, "higher", higher, &[]).await;
    assert_eq!(higher_state.notifications, parent_state.notifications);
}

/// Routine rival Stops acknowledge without warnings or partial capture changes.
/// Unknown liveness evidence has the same silent no-effect outcome, while a
/// malformed report or failed authoritative write remains an actual error.
/// The existing per-test trace capture measures WARN events only after setup,
/// so hook silence cannot pass merely because every hook already hides errors.
/// A failed authoritative write supplies a positive warning control, proving the
/// same capture and level spelling actually observe admission's diagnostics.
async fn stop_silences_rivals_but_preserves_validation_and_write_errors(case: PinCase) {
    let claude_pin_report = |conversation: &str, processes: &[ProcessIdentity]| {
        positional_pin_report(case, conversation, processes)
    };
    let claude_stop_report = |conversation: &str, processes: &[ProcessIdentity]| {
        positional_stop_report(case, conversation, processes)
    };
    let state = StateDir::new();
    let fail_process = Arc::new(AtomicBool::new(false));
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
                            .then(|| "controlled Stop process evidence unavailable".into())
                    }
                })),
                capture_store_fault: Some(Arc::new({
                    let fail = fail_write.clone();
                    move |write, _| {
                        if write == crate::service::capture::CaptureWrite::Report
                            && fail.load(Ordering::SeqCst)
                        {
                            anyhow::bail!("controlled Stop capture write failure");
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
    let id = "claude-stop-silence";
    seed_pin_session(&sup, id, case).await;
    let parent = ProcessIdentity::read(std::process::id()).unwrap();
    let rival = ProcessIdentity {
        pid: 900_070,
        start: 70,
    };
    sup.report_conversation(id, claude_pin_report("parent", &[parent]))
        .await
        .unwrap();
    let before = assert_claude_pin(&sup, id, "parent", parent, &[]).await;
    let capture = farhelm_testtrace::current_capture().expect("annotated test has a trace capture");

    for (unavailable, processes) in [(false, vec![rival, parent]), (true, vec![rival])] {
        fail_process.store(unavailable, Ordering::SeqCst);
        let baseline = capture.snapshot().expect("complete warning baseline");
        let warnings_before = baseline
            .events()
            .iter()
            .filter(|event| event.level == "WARN")
            .count();
        let reads_before = process_reads.load(Ordering::SeqCst);
        sup.report_conversation(id, claude_stop_report("rival", &processes))
            .await
            .expect("a rival Stop is acknowledged without effect");
        assert_eq!(
            process_reads.load(Ordering::SeqCst),
            reads_before + 1,
            "the test must reach the live-pin or unavailable-evidence decision"
        );
        assert_eq!(
            observe_claude_pin(&sup, id).await,
            before,
            "rival Stop changed authority (unavailable={unavailable})"
        );
        let after = capture.snapshot().expect("complete warning observation");
        assert_eq!(
            after
                .events()
                .iter()
                .filter(|event| event.level == "WARN")
                .count(),
            warnings_before,
            "rival Stop warned (unavailable={unavailable}): {:?}",
            after.events()
        );
    }

    let invalid_id = claude_stop_report("bad\nid", &[parent]);
    let error = sup.report_conversation(id, invalid_id).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidRequest);
    assert_eq!(observe_claude_pin(&sup, id).await, before);
    let mut invalid_corridor = claude_stop_report("parent", &[parent]);
    invalid_corridor.ancestry.as_mut().unwrap()[0].argv = Some(vec![b"not-a-hook".to_vec()]);
    let error = sup
        .report_conversation(id, invalid_corridor)
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);
    assert!(
        error.message.contains("supported hook invocation"),
        "{error:?}"
    );
    assert_eq!(observe_claude_pin(&sup, id).await, before);

    let failure_baseline = capture
        .snapshot()
        .expect("complete write-failure warning baseline");
    let warnings_before_failure = failure_baseline
        .events()
        .iter()
        .filter(|event| event.level == "WARN")
        .count();
    fail_write.store(true, Ordering::SeqCst);
    let error = sup
        .report_conversation(id, claude_stop_report("parent-after-clear", &[parent]))
        .await
        .expect_err("an authoritative Stop must expose a failed write");
    assert_eq!(error.kind, ErrorKind::Internal);
    assert_eq!(
        observe_claude_pin(&sup, id).await,
        before,
        "failed authoritative Stop published speculative capture"
    );
    let failure_trace = capture
        .snapshot()
        .expect("complete write-failure warning observation");
    assert!(
        failure_trace
            .events()
            .iter()
            .filter(|event| event.level == "WARN")
            .count()
            > warnings_before_failure,
        "the warning capture must observe the authoritative write failure: {:?}",
        failure_trace.events()
    );
    assert_eq!(
        process_reads.load(Ordering::SeqCst),
        2,
        "same-emitter Stop needs no liveness read even when that read would fail"
    );
}

/// Claude's original ancestor-correction and version-0 contract survives sharing.
#[farhelm_testtrace::test]
async fn claude_pin_corrects_ancestors_once_and_keeps_version_zero_resume() {
    pin_corrects_ancestors_once_and_keeps_version_zero_resume(CLAUDE).await;
}

/// Muse's own report dispatch corrects ancestors and posts one Muse warning.
#[farhelm_testtrace::test]
async fn muse_pin_corrects_ancestors_once_and_keeps_version_zero_resume() {
    pin_corrects_ancestors_once_and_keeps_version_zero_resume(MUSE).await;
}

/// Claude retains authority against nested, unrelated and reused process identities.
#[farhelm_testtrace::test]
async fn claude_pin_refuses_descendants_rivals_and_reused_pids() {
    pin_refuses_descendants_rivals_and_reused_pids(CLAUDE).await;
}

/// Muse must refuse rivals through the public dispatcher, not merely the pin judge.
#[farhelm_testtrace::test]
async fn muse_pin_refuses_descendants_rivals_and_reused_pids() {
    pin_refuses_descendants_rivals_and_reused_pids(MUSE).await;
}

/// Claude releases dead authority only when it is absent from the incoming chain.
#[farhelm_testtrace::test]
async fn claude_pin_releases_dead_authority_only_outside_its_chain() {
    pin_releases_dead_authority_only_outside_its_chain(CLAUDE).await;
}

/// Muse shares the dead-pin release boundary without an executable-name exception.
#[farhelm_testtrace::test]
async fn muse_pin_releases_dead_authority_only_outside_its_chain() {
    pin_releases_dead_authority_only_outside_its_chain(MUSE).await;
}

/// Claude's unknown liveness, generation and write failures remain mutation-free.
#[farhelm_testtrace::test]
async fn claude_pin_preserves_all_state_on_evidence_validation_and_write_failures() {
    pin_preserves_all_state_on_evidence_validation_and_write_failures(CLAUDE).await;
}

/// Muse cannot publish capture or a notice after failed evidence or durable adoption.
#[farhelm_testtrace::test]
async fn muse_pin_preserves_all_state_on_evidence_validation_and_write_failures() {
    pin_preserves_all_state_on_evidence_validation_and_write_failures(MUSE).await;
}

/// Claude's Stop remains an independent selecting report with retained ancestry.
#[farhelm_testtrace::test]
async fn claude_stop_selects_without_session_start_and_preserves_pin_ancestors() {
    stop_selects_without_session_start_and_preserves_pin_ancestors(CLAUDE).await;
}

/// Muse's observed source-less Stop can recover a missed first-prompt report.
#[farhelm_testtrace::test]
async fn muse_stop_selects_without_session_start_and_preserves_pin_ancestors() {
    stop_selects_without_session_start_and_preserves_pin_ancestors(MUSE).await;
}

/// Claude's ancestor Stop corrects child-first capture once per launch as before.
#[farhelm_testtrace::test]
async fn claude_stop_corrects_child_first_capture_and_notifies_once() {
    stop_corrects_child_first_capture_and_notifies_once(CLAUDE).await;
}

/// Muse's parent turn repairs a child-first pin with its own correction wording.
#[farhelm_testtrace::test]
async fn muse_stop_corrects_child_first_capture_and_notifies_once() {
    stop_corrects_child_first_capture_and_notifies_once(MUSE).await;
}

/// Claude's ordinary rival Stops stay quiet while authoritative failures still warn.
#[farhelm_testtrace::test]
async fn claude_stop_silences_rivals_but_preserves_validation_and_write_errors() {
    stop_silences_rivals_but_preserves_validation_and_write_errors(CLAUDE).await;
}

/// Muse's child turn must not create repeated warnings, even with unknown liveness.
#[farhelm_testtrace::test]
async fn muse_stop_silences_rivals_but_preserves_validation_and_write_errors() {
    stop_silences_rivals_but_preserves_validation_and_write_errors(MUSE).await;
}

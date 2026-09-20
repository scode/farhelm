//! Native-process Codex attribution regression coverage.
//!
//! The fixture deliberately imitates only the vendor behavior this failure
//! needs: a foreground native `codex` process fires a real hook, then a
//! nested native `codex` process inherits its launch credential and fires
//! another. The hook, Unix socket, peer identity, ancestry inspection,
//! durable store, offers, and restart are all the shipped implementation.
//! This keeps CI credential-free without replacing the mechanism that failed
//! against Codex 0.155.1.

use crate::boot_id_durable_outcome::listed;
use crate::harness::*;
use crate::hook_identity::{ServeTask, wait_for_after_from};

/// A copied executable is only the filesystem fallback for filesystems that
/// cannot make a hard link. Both shapes give the kernel executable a real
/// `codex` basename; a symlink would resolve back to `farhelm` and fail to
/// establish the premise the production ancestry guard validates.
fn native_codex_image(dir: &std::path::Path) -> std::path::PathBuf {
    let image = dir.join("codex");
    match std::fs::hard_link(farhelm_bin(), &image) {
        Ok(()) => image,
        Err(link_error) => {
            std::fs::copy(farhelm_bin(), &image).unwrap_or_else(|copy_error| {
                panic!(
                    "could not create the native Codex image: hard link failed: {link_error}; \
                     copy fallback failed: {copy_error}"
                )
            });
            image
        }
    }
}

/// A terminal event can end anywhere inside a fixture write. Only a newline
/// establishes that the marker's identity and native-image witness are complete.
fn complete_marker<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let (_, suffix) = text.rsplit_once(marker)?;
    let (line, _) = suffix.split_once('\n')?;
    Some(line.trim_end_matches('\r'))
}

/// Use the shared bounded drain, but wait for the whole fixture record rather
/// than letting a prefix or truncated id satisfy the readiness boundary.
async fn wait_for_marker_line(
    stream: &mut TermStream,
    seen: &mut Vec<u8>,
    from: usize,
    marker: &str,
) {
    wait_until(stream, seen, 30, marker, |data| {
        complete_marker(&String::from_utf8_lossy(&data[from..]), marker).is_some()
    })
    .await;
}

/// Extract a fixture id without relying on the supervisor's private locator
/// serialization. The test only needs to distinguish conversations visible to
/// a user through their resumed history.
fn marker_id(transcript: &[u8], marker: &str) -> String {
    let text = String::from_utf8_lossy(transcript);
    let line = complete_marker(&text, marker)
        .unwrap_or_else(|| panic!("no complete {marker:?} line in terminal:\n{text}"));
    let (id, _) = line
        .split_once(':')
        .filter(|(id, _)| !id.is_empty())
        .unwrap_or_else(|| panic!("no delimited id after {marker:?} in terminal:\n{text}"));
    id.to_owned()
}

/// The hook finishes and writes its diagnostic line before the nested child
/// prints this witness. Reading the owned log at that point proves the child
/// did reach the real socket path; it did not merely exit after a parse or
/// connection failure. A nested child is expected to be refused by the
/// foreground check, while the root may be acknowledged.
fn assert_nested_hook_reached_supervisor(h: &Harness, session_id: &str, child: &str) {
    let path = h
        .state
        .path()
        .join("hook-log")
        .join(format!("{session_id}.log"));
    let log = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "nested hook left no owned log at {}: {error}",
            path.display()
        )
    });
    let line = log
        .lines()
        .find(|line| line.contains(child))
        .unwrap_or_else(|| {
            panic!(
                "nested hook {child} has no log line in {}:\n{log}",
                path.display()
            )
        });
    assert!(
        line.contains(" acked ") || line.contains(" refused "),
        "nested hook must have received a supervisor response, not failed before it: {line}"
    );
}

/// A silent hook timeout also lets the fake agent print its terminal marker.
/// Require this invocation's actual supervisor reply, not an older compact or
/// an ancestry refusal after the peer died, before trusting the gated race.
fn assert_hook_reply_since(
    path: &std::path::Path,
    offset: usize,
    conversation: &str,
    source: &str,
    expected: &str,
) {
    let log = std::fs::read_to_string(path).expect("read the owned hook log");
    let added = log
        .get(offset..)
        .expect("the fixture hook log must not rotate");
    let identity = format!(" {conversation} {source}");
    let line = added
        .lines()
        .find(|line| line.ends_with(&identity))
        .unwrap_or_else(|| panic!("the gated hook left no matching response:\n{added}"));
    assert!(
        line.contains(expected),
        "the gated hook did not receive the required live-supervisor response:\n{line}"
    );
}

/// Wait for the public offer rather than assuming a completed hook or record
/// write has crossed the supervisor's durable capture boundary.
async fn wait_for_offer(
    client: &SupervisorClient,
    session_id: &str,
    offer: farhelm_proto::RestartOffer,
) {
    wait_for_listing(
        client,
        30,
        "the requested Codex restart offer",
        |sessions| {
            sessions
                .iter()
                .any(|session| session.id == session_id && session.restart_offer == offer)
        },
    )
    .await;
}

/// Send one terminal command and wait for its response only after the pty has
/// echoed that command. Repeated fixture markers therefore cannot satisfy a
/// later action from replayed scrollback or an earlier generation.
async fn send_and_wait(
    client: &SupervisorClient,
    channel: u32,
    stream: &mut TermStream,
    seen: &mut Vec<u8>,
    command: &str,
    marker: &str,
) {
    let from = seen.len();
    client
        .send_input(channel, format!("{command}\r").into_bytes())
        .await;
    wait_for_after_from(stream, seen, from, command, marker, 30).await;
    wait_for_marker_line(stream, seen, from, marker).await;
}

/// A nested native Codex must not displace its foreground parent's durable
/// conversation, while a genuine `/clear` is still allowed to withdraw A and
/// later promote B from its one exact transcript.
///
/// This test matters because a child native Codex inherits the real launch
/// credential by normal process inheritance. Before foreground attribution,
/// its valid socket report could become the session's resume target. The
/// actual-resume checks below make that failure observable as history from a
/// child (or a missing child history), not merely as a changed internal id.
///
/// The first launch is deliberately stranded before publication. Its hook must
/// establish the durable identity without an in-memory entry or recorded pane,
/// and a replacement supervisor must recover it before the rest of the journey.
/// Once B is bound, even another valid foreground report must not silently
/// rebind its exact file to a different persistent thread.
#[farhelm_testtrace::test]
async fn nested_native_codex_reports_cannot_replace_the_foreground_conversation() {
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            create_crash: Some(crate::boot_id_durable_outcome::crash_at(
                CreateStage::DuringLaunch,
            )),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let serving = ServeTask::spawn(&h.sup, h.state.path()).await;
    let work = farhelm_teststate::tempdir().expect("working directory");
    let records = farhelm_teststate::tempdir().expect("private Codex records");
    let images = farhelm_teststate::tempdir().expect("native Codex image directory");
    let codex = native_codex_image(images.path());
    let original = std::path::Path::new(farhelm_bin());

    let invocation = format!(
        "{} internal fake-agent --script codex-conversation --record-home {} --hook-binary {}",
        shell_words::quote(&codex.to_string_lossy()),
        shell_words::quote(&records.path().to_string_lossy()),
        shell_words::quote(&original.to_string_lossy()),
    );
    let resume_template = vec![
        codex.to_string_lossy().into_owned(),
        "internal".to_string(),
        "fake-agent".to_string(),
        "--script".to_string(),
        "codex-conversation".to_string(),
        "--record-home".to_string(),
        records.path().to_string_lossy().into_owned(),
        "--hook-binary".to_string(),
        original.to_string_lossy().into_owned(),
        "--resume-id".to_string(),
        farhelm_supervisor::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
    ];
    let failure = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            &invocation,
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras {
                agent_kind: Some(farhelm_proto::AgentKind::Codex),
                resume_template: Some(resume_template),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect_err("the launch must stop before publication");
    assert!(failure.to_string().contains("simulated crash"), "{failure}");

    // A list would reconcile the unfinished launch and erase the boundary this
    // check is about. The hook's diagnostic file is written only after its
    // request finishes; observe that witness without asking the supervisor to
    // publish the entry, then inspect the already-committed durable row.
    let store = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("open owned store");
    let rows = store.load_all().await.expect("read unfinished launch");
    assert_eq!(
        rows.len(),
        1,
        "the injected crash must leave exactly one launching row"
    );
    let id = rows[0].id.clone();
    wait_for_file(
        &h.state.path().join("hook-log").join(format!("{id}.log")),
        30,
    )
    .await;
    let row = store
        .session(&id)
        .await
        .expect("read reported launch")
        .expect("launching row survives");
    assert_eq!(
        row.outcome,
        LastOutcome::Launching,
        "the report must precede launch confirmation"
    );
    assert!(
        row.pane.is_empty(),
        "the report must work before the pane is published"
    );
    assert_eq!(row.conversation_source.as_deref(), Some("hook"));
    assert!(
        row.captured_conversation.is_some(),
        "the foreground hook must commit its identity before publication"
    );
    drop(store);

    serving.stop().await;
    let Harness {
        client,
        sup,
        _tmux,
        state,
        _slot,
    } = h;
    let report_armed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let report_entered = Arc::new(tokio::sync::Notify::new());
    let report_release = Arc::new(tokio::sync::Notify::new());
    let sup = crate::create_idempotency::handoff_to_new_supervisor_with_seams(
        state.path(),
        sup,
        client,
        SupervisorSeams {
            codex_report_gate: Some(Arc::new({
                let armed = Arc::clone(&report_armed);
                let entered = Arc::clone(&report_entered);
                let release = Arc::clone(&report_release);
                move || {
                    let entered = Arc::clone(&entered);
                    let release = Arc::clone(&release);
                    let pause = armed.swap(false, std::sync::atomic::Ordering::SeqCst);
                    Box::pin(async move {
                        if pause {
                            entered.notify_one();
                            tokio::time::timeout(Duration::from_secs(5), release.notified())
                                .await
                                .expect("the owned report interleaving must be released");
                        }
                    })
                }
            })),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let serving = ServeTask::spawn(&sup, state.path()).await;
    let client = connect_client(&sup).await;
    let h = Harness {
        client,
        sup,
        _tmux,
        state,
        _slot,
    };
    let session = listed(&h.client, &id).await;
    assert_eq!(
        h.sup
            .session_snapshot(&id)
            .await
            .expect("recovered snapshot")
            .expect("recovered session")
            .generation,
        row.generation,
        "recovery must adopt the existing foreground process, not silently relaunch it",
    );

    let (channel, initial, mut stream) = h
        .client
        .attach_live(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach root Codex");
    let mut seen = initial;
    wait_for(&mut stream, &mut seen, "CODEX-KERNEL-EXE:codex", 30).await;
    wait_for_marker_line(&mut stream, &mut seen, 0, "CODEX-CONVERSATION READY:").await;
    let root_a = marker_id(&seen, "CODEX-CONVERSATION READY:");
    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "remember root-a",
        "CODEX-REMEMBERED:",
    )
    .await;
    wait_for_offer(&h.client, &session.id, farhelm_proto::RestartOffer::Resume).await;

    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "nested-persisted",
        "CODEX-NESTED:persisted:",
    )
    .await;
    let persisted_child = marker_id(&seen, "CODEX-NESTED:persisted:");
    assert_ne!(
        persisted_child, root_a,
        "a child must have its own conversation id"
    );
    assert!(
        String::from_utf8_lossy(&seen)
            .contains(&format!("CODEX-NESTED:persisted:{persisted_child}:codex")),
        "the nested reporter must execute the real codex-named image, not only inherit its argv"
    );
    assert_nested_hook_reached_supervisor(&h, &session.id, &persisted_child);

    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "nested-ephemeral",
        "CODEX-NESTED:ephemeral:",
    )
    .await;
    let ephemeral_child = marker_id(&seen, "CODEX-NESTED:ephemeral:");
    assert_ne!(
        ephemeral_child, root_a,
        "an ephemeral child must have its own conversation id"
    );
    assert_ne!(
        ephemeral_child, persisted_child,
        "each child invocation must mint a fresh id"
    );
    assert_nested_hook_reached_supervisor(&h, &session.id, &ephemeral_child);

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Resume, true)
        .await
        .expect("the foreground root remains resumable after child reports");
    let (channel, replay, mut stream) = h
        .client
        .attach_live(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach resumed root Codex");
    let mut seen = replay;
    wait_for_after(
        &mut stream,
        &mut seen,
        &format!("CODEX-CONVERSATION RESUMED:{root_a}"),
        &format!("CODEX-CONVERSATION READY:{root_a}:"),
        30,
    )
    .await;
    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "recall",
        "CODEX-RECALL:root-a",
    )
    .await;

    let generation_before_clear = h
        .sup
        .session_snapshot(&session.id)
        .await
        .expect("snapshot")
        .expect("session")
        .generation;
    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "clear-pending",
        "CODEX-CLEAR-PENDING:",
    )
    .await;
    let cleared_b = marker_id(&seen, "CODEX-CLEAR-PENDING:");
    assert_ne!(
        cleared_b, root_a,
        "clear must mint B instead of retaining A"
    );
    wait_for_offer(
        &h.client,
        &session.id,
        farhelm_proto::RestartOffer::FreshOnly,
    )
    .await;

    let refusal = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Resume, true)
        .await
        .expect_err("pending clear has no stale A resume to run");
    let refusal = refusal
        .downcast_ref::<SupervisorError>()
        .expect("resume refusal is a supervisor error");
    assert_eq!(refusal.kind, farhelm_proto::ErrorKind::Conflict);
    let after_refusal = h
        .sup
        .session_snapshot(&session.id)
        .await
        .expect("snapshot")
        .expect("session");
    assert_eq!(
        after_refusal.generation, generation_before_clear,
        "a refused Resume must not stop or relaunch the foreground process"
    );

    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "nested-ephemeral",
        "CODEX-NESTED:ephemeral:",
    )
    .await;
    let pending_child = marker_id(&seen, "CODEX-NESTED:ephemeral:");
    assert_ne!(
        pending_child, cleared_b,
        "an unrelated reporter must not reuse B's identity"
    );
    assert_nested_hook_reached_supervisor(&h, &session.id, &pending_child);
    wait_for_offer(
        &h.client,
        &session.id,
        farhelm_proto::RestartOffer::FreshOnly,
    )
    .await;

    // A valid clear must also survive refresh winning first. The discarded
    // pending conversation gets its file while the next clear report is paused;
    // promoting that old file must not make the supervisor throw away the clear.
    let hook_log = h
        .state
        .path()
        .join("hook-log")
        .join(format!("{}.log", session.id));
    let clear_log_offset = std::fs::read(&hook_log)
        .expect("read prior hook outcomes")
        .len();
    let discarded = cleared_b;
    let a_record = std::fs::read_to_string(records.path().join(format!("{root_a}.jsonl")))
        .expect("read the fixture's root-record shape");
    let mut root: serde_json::Value =
        serde_json::from_str(a_record.lines().next().expect("A's complete header"))
            .expect("fixture root JSON");
    report_armed.store(true, std::sync::atomic::Ordering::SeqCst);
    let clear_from = seen.len();
    h.client
        .send_input(channel, b"clear-pending\r".to_vec())
        .await;
    tokio::time::timeout(Duration::from_secs(5), report_entered.notified())
        .await
        .expect("the next clear must reach the controlled report boundary");
    root["payload"]["id"] = serde_json::json!(discarded);
    root["payload"]["session_id"] = serde_json::json!(discarded);
    std::fs::write(
        records.path().join(format!("{discarded}.jsonl")),
        format!("{root}\n"),
    )
    .expect("publish the discarded conversation's delayed transcript");
    wait_for_offer(&h.client, &session.id, farhelm_proto::RestartOffer::Resume).await;
    report_release.notify_one();
    wait_for_after_from(
        &mut stream,
        &mut seen,
        clear_from,
        "clear-pending",
        "CODEX-CLEAR-PENDING:",
        30,
    )
    .await;
    wait_for_marker_line(&mut stream, &mut seen, clear_from, "CODEX-CLEAR-PENDING:").await;
    let cleared_b = marker_id(&seen, "CODEX-CLEAR-PENDING:");
    assert_ne!(
        cleared_b, discarded,
        "the second clear must name a new conversation"
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly,
        "promoting the discarded conversation must not cause a legitimate clear to be lost"
    );
    assert_hook_reply_since(&hook_log, clear_log_offset, &cleared_b, "clear", " acked ");

    // Pause a real compact report before its capture transaction. Publish B's
    // file and let the public listing bind it, then replace only the persistent
    // ID. The report must read and preserve the established binding rather than
    // bless the replacement under its unchanged runtime ID and path.
    let race_record = records.path().join(format!("{cleared_b}.jsonl"));
    assert!(!race_record.exists(), "B must still lack its exact record");
    report_armed.store(true, std::sync::atomic::Ordering::SeqCst);
    let race_from = seen.len();
    let race_log_offset = std::fs::read(&hook_log)
        .expect("read prior hook outcomes")
        .len();
    h.client.send_input(channel, b"compact\r".to_vec()).await;
    tokio::time::timeout(Duration::from_secs(5), report_entered.notified())
        .await
        .expect("the foreground hook must reach the pre-transaction boundary");
    root["payload"]["id"] = serde_json::json!(cleared_b);
    root["payload"]["session_id"] = serde_json::json!(cleared_b);
    let bound_b = format!("{root}\n");
    std::fs::write(&race_record, &bound_b).expect("publish B's exact root record");
    wait_for_offer(&h.client, &session.id, farhelm_proto::RestartOffer::Resume).await;
    root["payload"]["id"] = serde_json::json!("concurrent-replacement-thread");
    std::fs::write(&race_record, format!("{root}\n"))
        .expect("replace the persistent ID after refresh bound B");
    report_release.notify_one();
    let compact_marker = format!("CODEX-COMPACT:{cleared_b}");
    wait_for_after_from(
        &mut stream,
        &mut seen,
        race_from,
        "compact",
        &compact_marker,
        30,
    )
    .await;
    wait_for_marker_line(&mut stream, &mut seen, race_from, &compact_marker).await;
    assert_hook_reply_since(
        &hook_log,
        race_log_offset,
        &cleared_b,
        "compact",
        concat!(
            " refused invalid_request the Codex report's exact record could not be verified: ",
            "Codex record changed its persistent thread identity",
        ),
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly,
        "the report must preserve the B binding established before its capture transaction"
    );
    std::fs::write(&race_record, bound_b).expect("restore B's owned root record");
    wait_for_offer(&h.client, &session.id, farhelm_proto::RestartOffer::Resume).await;

    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "persist",
        &format!("CODEX-PERSISTED:{cleared_b}"),
    )
    .await;
    wait_for_offer(&h.client, &session.id, farhelm_proto::RestartOffer::Resume).await;
    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "remember clear-b",
        "CODEX-REMEMBERED:",
    )
    .await;
    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "compact",
        &format!("CODEX-COMPACT:{cleared_b}"),
    )
    .await;

    // Keep the reported runtime and exact path while substituting a different
    // persistent thread. Refresh must withdraw Resume, and a subsequent compact
    // report must not forget the old binding and bless the replacement.
    let record_path = records.path().join(format!("{cleared_b}.jsonl"));
    let original_record = std::fs::read_to_string(&record_path).expect("read B's owned transcript");
    let (header, history) = original_record
        .split_once('\n')
        .expect("complete root header");
    let mut header: serde_json::Value = serde_json::from_str(header).expect("fixture root JSON");
    assert_eq!(header["payload"]["id"], cleared_b);
    assert_eq!(header["payload"]["session_id"], cleared_b);
    header["payload"]["id"] = serde_json::json!("replacement-thread");
    std::fs::write(&record_path, format!("{header}\n{history}"))
        .expect("replace only the owned fixture's persistent identity");
    wait_for_offer(
        &h.client,
        &session.id,
        farhelm_proto::RestartOffer::FreshOnly,
    )
    .await;
    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "compact",
        &format!("CODEX-COMPACT:{cleared_b}"),
    )
    .await;
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly,
        "a repeated report must not rebind B's file to a different persistent thread"
    );
    let refusal = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Resume, true)
        .await
        .expect_err("the conflicting persistent identity must not be resumed");
    assert_eq!(
        refusal
            .downcast_ref::<SupervisorError>()
            .expect("supervisor refusal")
            .kind,
        farhelm_proto::ErrorKind::Conflict
    );
    std::fs::write(&record_path, original_record).expect("restore B's original owned transcript");
    wait_for_offer(&h.client, &session.id, farhelm_proto::RestartOffer::Resume).await;

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Resume, true)
        .await
        .expect("persisted B becomes the only resumable foreground conversation");
    let (channel, replay, mut stream) = h
        .client
        .attach_live(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach resumed B");
    let mut seen = replay;
    wait_for_after(
        &mut stream,
        &mut seen,
        &format!("CODEX-CONVERSATION RESUMED:{cleared_b}"),
        &format!("CODEX-CONVERSATION READY:{cleared_b}:"),
        30,
    )
    .await;
    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "recall",
        "CODEX-RECALL:clear-b",
    )
    .await;

    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::Resume
    );
    serving.stop().await;
}

/// Every incomplete prefix is a possible terminal chunk boundary. None may
/// expose an identity before its native-image suffix and line ending arrive.
#[test]
fn fixture_marker_waits_for_its_complete_line() {
    let marker = "CODEX-NESTED:persisted:";
    let line = "CODEX-NESTED:persisted:fake-123:codex\r\n";
    for end in 0..line.len() {
        assert_eq!(
            complete_marker(&line[..end], marker),
            None,
            "split at {end}"
        );
    }
    assert_eq!(complete_marker(line, marker), Some("fake-123:codex"));
    let later_partial = format!("{line}{marker}fake-456");
    assert_eq!(complete_marker(&later_partial, marker), None);
}

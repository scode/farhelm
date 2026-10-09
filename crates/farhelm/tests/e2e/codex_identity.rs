//! Native-process Codex attribution regression coverage.
//!
//! The fixture deliberately imitates only the vendor behavior this failure
//! needs: a foreground native `codex` process fires a real hook, then a
//! nested native `codex` process inherits its launch credential and fires
//! another. The hook, its report file and recorded ancestry, attribution,
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
    match std::fs::hard_link(fixtures_bin(), &image) {
        Ok(()) => image,
        Err(link_error) => {
            std::fs::copy(fixtures_bin(), &image).unwrap_or_else(|copy_error| {
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

/// The supervisor's verdict on a report: the `acked` or `refused` line it
/// appended to the session's hook log after `offset`, for `conversation`
/// (and `source`, when given). Runs a reconciliation pass first, so a
/// report the hook has already dropped has been judged by the time the log
/// is read; the hook's own `written` line for the same identity is skipped.
async fn supervisor_verdict(
    sup: &Supervisor,
    path: &std::path::Path,
    offset: usize,
    conversation: &str,
    source: Option<&str>,
) -> String {
    sup.reconcile_for_test().await;
    let log = std::fs::read_to_string(path).expect("read the owned hook log");
    let added = log
        .get(offset..)
        .expect("the fixture hook log must not rotate");
    added
        .lines()
        .find(|line| {
            let identified = match source {
                Some(source) => line.ends_with(&format!(" {conversation} {source}")),
                None => line.contains(&format!(" {conversation} ")),
            };
            let word = line.split_whitespace().nth(1);
            identified && matches!(word, Some("acked" | "refused"))
        })
        .unwrap_or_else(|| {
            panic!(
                "no supervisor verdict for {conversation} in {}:\n{added}",
                path.display()
            )
        })
        .to_string()
}

/// The nested child's hook finishes before the child prints its witness, so
/// its report is on disk by then. The supervisor's verdict on it proves the
/// report reached the supervisor's judgement; it did not merely fail to
/// parse or to be written. A nested child is expected to be refused by the
/// foreground check, while the root may be acknowledged.
async fn assert_nested_hook_reached_supervisor(h: &Harness, session_id: &str, child: &str) {
    let path = h
        .state
        .path()
        .join("hook-log")
        .join(format!("{session_id}.log"));
    supervisor_verdict(&h.sup, &path, 0, child, None).await;
}

/// Require this invocation's actual supervisor verdict, not an older
/// compact's, before trusting the gated race: the verdict for
/// `conversation`/`source` written after `offset` must contain `expected`.
async fn assert_hook_reply_since(
    sup: &Supervisor,
    path: &std::path::Path,
    offset: usize,
    conversation: &str,
    source: &str,
    expected: &str,
) {
    let line = supervisor_verdict(sup, path, offset, conversation, Some(source)).await;
    assert!(
        line.contains(expected),
        "the report did not get the required supervisor verdict:\n{line}"
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

/// Read the durable capture binding without disturbing the running
/// supervisor: the exact saved target, its ownership provenance, and its
/// source. Every ownership assertion below goes through this real row —
/// never through an in-memory mirror — paired with the real public
/// offer the listing serves.
async fn durable_binding(
    state: &std::path::Path,
    session_id: &str,
) -> (Option<String>, i64, Option<String>) {
    let store = SessionStore::open(&state.join("supervisor.db"), false)
        .await
        .expect("open owned store");
    let row = store
        .session(session_id)
        .await
        .expect("read reported launch")
        .expect("row survives");
    (
        row.captured_conversation,
        row.capture_ownership_version,
        row.conversation_source,
    )
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

/// A descendant must not win the first report merely because no foreground
/// binding exists yet. The root is live in its owned pane and the child's
/// transcript is valid, so the observed intermediary refusal distinguishes
/// ancestry enforcement from missing-pane or missing-record rejection.
#[farhelm_testtrace::test]
async fn a_shell_child_cannot_claim_a_pristine_codex_session() {
    let h = harness().await;
    let serving = ServeTask::spawn(&h.sup, h.state.path()).await;
    let work = farhelm_teststate::tempdir().expect("working directory");
    let records = farhelm_teststate::tempdir().expect("private Codex records");
    let images = farhelm_teststate::tempdir().expect("native Codex image directory");
    let codex = native_codex_image(images.path());
    let invocation = format!(
        "{} fake-agent --script codex-conversation --record-home {} --hook-binary {} --defer-startup",
        shell_words::quote(&codex.to_string_lossy()),
        shell_words::quote(&records.path().to_string_lossy()),
        shell_words::quote(farhelm_bin()),
    );
    let session = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            // The resume command is what makes the row's offer about
            // capture: without one it would read `NoResumeCommand` whatever
            // the child reported.
            declared_command(
                &format!("{} {{farhelm_args}}", invocation),
                farhelm_proto::LaunchHarness::Codex,
                Some(&format!(
                    "{} resume {{conversation}} {{farhelm_args}}",
                    invocation
                )),
            ),
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .expect("create the foreground without its startup report");
    let (channel, initial, mut stream) = h
        .client
        .attach_live(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach the live foreground");
    let mut seen = initial;
    wait_for(&mut stream, &mut seen, "CODEX-KERNEL-EXE:codex", 30).await;
    wait_for_marker_line(&mut stream, &mut seen, 0, "CODEX-CONVERSATION READY:").await;
    let root = marker_id(&seen, "CODEX-CONVERSATION READY:");
    let pristine = (None, 0, None);
    assert_eq!(durable_binding(h.state.path(), &session.id).await, pristine);
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::NotCaptured,
    );

    let from = seen.len();
    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "nested-shell-persisted",
        "CODEX-NESTED-SHELL:",
    )
    .await;
    let child = marker_id(&seen[from..], "CODEX-NESTED-SHELL:persisted:");
    assert_ne!(child, root, "the child has a distinct valid transcript");
    let log = h
        .state
        .path()
        .join("hook-log")
        .join(format!("{}.log", session.id));
    assert_hook_reply_since(&h.sup, &log, 0, &child, "startup", " refused ").await;
    assert_hook_reply_since(
        &h.sup,
        &log,
        0,
        &child,
        "startup",
        "unclassified intermediary",
    )
    .await;
    assert_eq!(
        durable_binding(h.state.path(), &session.id).await,
        pristine,
        "a processed child-first report must leave the row pristine",
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::NotCaptured,
    );

    // The same live foreground must then succeed: a permanently broken
    // report path or a dead pane cannot satisfy the negative assertion alone.
    let offset = std::fs::read(&log).expect("read child refusal").len();
    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "report-startup",
        "CODEX-STARTUP-REPORTED:",
    )
    .await;
    assert_hook_reply_since(&h.sup, &log, offset, &root, "startup", " acked ").await;
    wait_for_offer(&h.client, &session.id, farhelm_proto::RestartOffer::Resume).await;
    let (binding, provenance, source) = durable_binding(h.state.path(), &session.id).await;
    assert!(
        binding.is_some(),
        "the foreground establishes the first binding"
    );
    assert_eq!(provenance, 1);
    assert_eq!(source.as_deref(), Some("hook"));
    serving.stop().await;
}

/// A nested native Codex must not displace its foreground parent's durable
/// conversation, while a genuine `/clear` is still allowed to withdraw A and
/// later promote B from its one exact transcript.
///
/// This test matters because a child native Codex inherits the real launch
/// credential by normal process inheritance. Before foreground attribution,
/// its valid report could become the session's resume target. The
/// actual-resume checks below make that failure observable as history from a
/// child (or a missing child history), not merely as a changed internal id.
///
/// The first launch is deliberately stranded before publication. Its hook's
/// report must wait on disk rather than be applied without an in-memory entry
/// or recorded pane, and the replacement supervisor that publishes the session
/// must apply it, through the full ownership contract, before the rest of the
/// journey.
/// Once B is bound, even another valid foreground report must not silently
/// rebind its exact file to a different persistent thread.
#[farhelm_testtrace::test]
async fn nested_native_codex_reports_cannot_replace_the_foreground_conversation() {
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            faults: FaultHooks {
                create_crash: Some(crate::boot_id_durable_outcome::crash_at(
                    CreateStage::DuringLaunch,
                )),
                ..FaultHooks::default()
            },
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
        "{} fake-agent --script codex-conversation --record-home {} --hook-binary {}",
        shell_words::quote(&codex.to_string_lossy()),
        shell_words::quote(&records.path().to_string_lossy()),
        shell_words::quote(&original.to_string_lossy()),
    );
    let resume_template = vec![
        codex.to_string_lossy().into_owned(),
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
            declared_command(
                &format!("{} {{farhelm_args}}", invocation),
                farhelm_proto::LaunchHarness::Codex,
                Some(format!(
                    "{} {{farhelm_args}}",
                    shell_words::join(&resume_template)
                ))
                .as_deref(),
            ),
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .expect_err("the launch must stop before publication");
    assert!(failure.to_string().contains("simulated crash"), "{failure}");

    // A list would reconcile the unfinished launch and erase the boundary this
    // check is about. The hook's diagnostic line is written only after its
    // report is on disk; observe that witness without asking the supervisor to
    // publish the entry, then inspect the durable row and the drop directory.
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
        "the report must have been made before the pane was published"
    );
    assert_eq!(
        row.captured_conversation, None,
        "a report for an unpublished launch waits instead of being applied"
    );
    let waiting = farhelm_supervisor::hook_report::session_dir(h.state.path(), &id)
        .expect("a session id names a drop directory")
        .join(farhelm_supervisor::hook_report::Slot::Selection.file_name());
    assert!(
        waiting.exists(),
        "the pre-publication report must wait on disk for the supervisor that publishes \
         the session"
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
            faults: FaultHooks {
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
                ..FaultHooks::default()
            },
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
    // Recovery published the session; its waiting report is applied on the
    // next pass, through the full ownership contract, with the pane now
    // recovered from tmux.
    h.sup.reconcile_for_test().await;
    let (recovered, provenance, source) = durable_binding(h.state.path(), &id).await;
    assert_eq!(source.as_deref(), Some("hook"));
    assert!(
        recovered.is_some(),
        "the report made before publication is applied once a supervisor publishes the session"
    );
    assert_eq!(
        provenance, 1,
        "the waiting report passed the full ownership contract"
    );
    assert!(
        !waiting.exists(),
        "the applied report is settled and removed"
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
    assert_nested_hook_reached_supervisor(&h, &session.id, &persisted_child).await;

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
    assert_nested_hook_reached_supervisor(&h, &session.id, &ephemeral_child).await;

    // Shell descendants are refused by the corridor — the live shell
    // between reporter and runtime matches no narrow trampoline — no
    // matter how valid the rest of the report is. The persisted case
    // carries a real record for its own runtime (verified against the
    // file before reporting), so only ancestry can refuse it; the
    // fileless case reports `clear`, the descendant's strongest weapon,
    // and doubles as the live descendant-clear regression. The native
    // case wraps a second Codex image in a live shell, pinning that the
    // nested rule is not bypassed by shell wrapping. Every case leaves
    // the complete saved binding and the public offer byte-identical.
    let shell_log = h
        .state
        .path()
        .join("hook-log")
        .join(format!("{}.log", session.id));
    let pre_shell = durable_binding(h.state.path(), &session.id).await;
    for (command, kind, source) in [
        ("nested-shell-persisted", "persisted", "startup"),
        ("nested-shell-fileless", "fileless", "clear"),
    ] {
        let log_offset = std::fs::read(&shell_log)
            .expect("read prior hook outcomes")
            .len();
        let shell_from = seen.len();
        send_and_wait(
            &h.client,
            channel,
            &mut stream,
            &mut seen,
            command,
            "CODEX-NESTED-SHELL:",
        )
        .await;
        let shell_child = marker_id(&seen[shell_from..], &format!("CODEX-NESTED-SHELL:{kind}:"));
        assert_ne!(
            shell_child, root_a,
            "a shell descendant must have its own conversation id"
        );
        // The intermediary witnessed itself from the live shell process —
        // before the report in the script shape, after it in the hook-first
        // `-c` shape (one invocation either way, which the completion
        // marker proves) — not a fixture constant, and never the runtime
        // the corridor counts.
        let intermediary = String::from_utf8_lossy(&seen[shell_from..])
            .lines()
            .find_map(|line| line.strip_prefix("SHELL-INTERMEDIARY:"))
            .expect("the live intermediary witnessed its own image")
            .to_string();
        assert_ne!(
            intermediary, "codex",
            "the intermediary is a shell, not a second runtime"
        );
        assert!(
            String::from_utf8_lossy(&seen[shell_from..]).contains(&format!(
                "CODEX-NESTED-SHELL:{kind}:{shell_child}:{intermediary}"
            )),
            "the completion witness must tie the runtime to its observed intermediary"
        );
        assert_hook_reply_since(
            &h.sup,
            &shell_log,
            log_offset,
            &shell_child,
            source,
            " refused ",
        )
        .await;
        assert_eq!(
            durable_binding(h.state.path(), &session.id).await,
            pre_shell,
            "a refused descendant report must leave the complete saved binding untouched"
        );
        assert_eq!(
            listed(&h.client, &session.id).await.restart_offer,
            farhelm_proto::RestartOffer::Resume,
            "the foreground root stays resumable through shell-descendant {kind} reports"
        );
    }

    // Shell-launched native Codex: shell script, nested image, hook —
    // all three live. Two Codex images refuse this regardless of the
    // shell between them.
    {
        let log_offset = std::fs::read(&shell_log)
            .expect("read prior hook outcomes")
            .len();
        let shell_from = seen.len();
        send_and_wait(
            &h.client,
            channel,
            &mut stream,
            &mut seen,
            "nested-shell-native",
            "CODEX-NESTED-SHELLNATIVE:",
        )
        .await;
        let shell_child = marker_id(&seen[shell_from..], "CODEX-NESTED-SHELLNATIVE:persisted:");
        assert_ne!(
            shell_child, root_a,
            "a shell-launched nested runtime must have its own conversation id"
        );
        let intermediary = String::from_utf8_lossy(&seen[shell_from..])
            .lines()
            .find_map(|line| line.strip_prefix("SHELL-INTERMEDIARY:"))
            .expect("the launching shell witnessed itself")
            .to_string();
        assert_ne!(
            intermediary, "codex",
            "the launcher is a shell, not a runtime"
        );
        assert!(
            String::from_utf8_lossy(&seen[shell_from..]).contains(&format!(
                "CODEX-NESTED-SHELLNATIVE:persisted:{shell_child}:codex"
            )),
            "the nested image witness must survive its shell wrapping"
        );
        assert_hook_reply_since(
            &h.sup,
            &shell_log,
            log_offset,
            &shell_child,
            "startup",
            " refused ",
        )
        .await;
        assert_eq!(
            durable_binding(h.state.path(), &session.id).await,
            pre_shell,
            "a refused shell-wrapped nested report must leave the complete saved binding untouched"
        );
        assert_eq!(
            listed(&h.client, &session.id).await.restart_offer,
            farhelm_proto::RestartOffer::Resume,
            "the foreground root stays resumable through shell-wrapped nested reports"
        );
    }

    h.client
        .restart_session(&session.id, true)
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
        farhelm_proto::RestartOffer::NotCaptured,
    )
    .await;
    // An admitted fileless foreground transition writes provenance 1
    // with no Resume offer: 1 proves ownership, not file readiness.
    let (saved, version, _) = durable_binding(h.state.path(), &session.id).await;
    let saved = saved.expect("the fileless clear still commits a binding");
    assert!(
        saved.contains(&cleared_b),
        "the fileless clear must name B's runtime: {saved}"
    );
    assert_eq!(
        version, 1,
        "the admitted fileless transition carries provenance 1"
    );

    let refusal = h
        .client
        .restart_session(&session.id, true)
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
    assert_nested_hook_reached_supervisor(&h, &session.id, &pending_child).await;
    wait_for_offer(
        &h.client,
        &session.id,
        farhelm_proto::RestartOffer::NotCaptured,
    )
    .await;

    // A file arriving by itself cannot promote a pending conversation, even
    // while the next selecting report is paused. Only the subscribed callback
    // confirms it, and the next clear must still replace that pending binding.
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
        .send_input(channel, b"clear-fileless\r".to_vec())
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
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::NotCaptured,
        "a delayed transcript alone must not be verified by a listing"
    );
    report_release.notify_one();
    wait_for_after_from(
        &mut stream,
        &mut seen,
        clear_from,
        "clear-fileless",
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
        farhelm_proto::RestartOffer::NotCaptured,
        "the next clear must displace the pending conversation"
    );
    assert_hook_reply_since(
        &h.sup,
        &hook_log,
        clear_log_offset,
        &cleared_b,
        "clear",
        " acked ",
    )
    .await;

    let pending_binding = durable_binding(h.state.path(), &session.id).await;
    let other_offset = std::fs::read(&hook_log).unwrap().len();
    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "stop-other",
        "CODEX-OTHER-STOP:",
    )
    .await;
    let other_text = String::from_utf8_lossy(&seen);
    let other = complete_marker(&other_text, "CODEX-OTHER-STOP:").unwrap();
    assert_ne!(
        other, cleared_b,
        "the rejected Stop must name another runtime"
    );
    let other_record = records.path().join(format!("{other}.jsonl"));
    let other_meta: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(&other_record)
            .expect("other Stop has a persisted record")
            .lines()
            .next()
            .expect("other Stop has a header"),
    )
    .unwrap();
    assert_eq!(other_meta["payload"]["session_id"], other);
    let other_reply = supervisor_verdict(&h.sup, &hook_log, other_offset, other, None).await;
    assert!(other_reply.contains(" acked "), "{other_reply}");
    assert_eq!(
        durable_binding(h.state.path(), &session.id).await,
        pending_binding,
        "Stop cannot select another runtime while the real clear is pending"
    );

    // Confirm B through a real Stop report before pausing compact. Mutating
    // the file after the offer is established cannot rebind the compact report
    // to a different persistent thread under its unchanged runtime and path.
    let race_record = records.path().join(format!("{cleared_b}.jsonl"));
    assert!(!race_record.exists(), "B must still lack its exact record");
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
    // Persist creates B's own timestamp. Preserve the admitted bytes rather
    // than synthesizing B from A's earlier header, which may cross a second.
    let bound_b = std::fs::read_to_string(&race_record).unwrap();
    let mut root: serde_json::Value = serde_json::from_str(&bound_b).unwrap();
    assert_eq!(root["payload"]["id"], cleared_b);
    assert_eq!(root["payload"]["session_id"], cleared_b);
    assert_eq!(root["payload"]["source"], "cli");
    report_armed.store(true, std::sync::atomic::Ordering::SeqCst);
    let race_from = seen.len();
    let race_log_offset = std::fs::read(&hook_log)
        .expect("read prior hook outcomes")
        .len();
    h.client.send_input(channel, b"compact\r".to_vec()).await;
    // The hook only saves its report; a reconciliation pass is what carries
    // it to the gate. Wait for the saved report, then drive that pass here
    // rather than leaving it to the ticker's timing.
    let saved_deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let log = std::fs::read(&hook_log).unwrap_or_default();
        let added = String::from_utf8_lossy(log.get(race_log_offset..).unwrap_or_default());
        if added
            .lines()
            .any(|line| line.contains(" written ") && line.ends_with(" compact"))
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < saved_deadline,
            "the compact hook never saved its report:\n{added}"
        );
        // sleep-ok: polling interval for a file the hook child appends to.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let gated_pass = tokio::spawn({
        let sup = Arc::clone(&h.sup);
        async move { sup.reconcile_for_test().await }
    });
    tokio::time::timeout(Duration::from_secs(5), report_entered.notified())
        .await
        .expect("the report drain must reach the pre-transaction boundary");
    assert_eq!(
        std::fs::read_to_string(&race_record).unwrap(),
        bound_b,
        "the Stop fixture already published B before the compact gate"
    );
    root["payload"]["id"] = serde_json::json!("concurrent-replacement-thread");
    std::fs::write(&race_record, format!("{root}\n"))
        .expect("replace the persistent ID after Stop bound B");
    report_release.notify_one();
    gated_pass
        .await
        .expect("the gated reconciliation pass completes");
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
        &h.sup,
        &hook_log,
        race_log_offset,
        &cleared_b,
        "compact",
        concat!(
            " refused invalid_request the Codex report's exact record could not be verified: ",
            "Codex record changed its persistent thread identity",
        ),
    )
    .await;
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "a refused selecting report must preserve the established B binding"
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

    // A changed persistent thread is discovered at Restart. Listing retains
    // its admitted offer, and compact must not bless the replacement. Restart
    // refuses without relaunching; a later selecting report can restore B.
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
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "list must not verify a changed transcript"
    );
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
        farhelm_proto::RestartOffer::Resume,
        "a refused report must preserve B's admitted binding until Restart"
    );
    let mismatch_notifications = listed(&h.client, &session.id).await.notifications;
    let mismatch_generation = h
        .sup
        .session_snapshot(&session.id)
        .await
        .unwrap()
        .unwrap()
        .generation;
    let refusal = h
        .client
        .restart_session(&session.id, true)
        .await
        .expect_err("the conflicting persistent identity must not be resumed");
    assert_eq!(
        refusal
            .downcast_ref::<SupervisorError>()
            .expect("supervisor refusal")
            .kind,
        farhelm_proto::ErrorKind::Conflict
    );
    assert_eq!(
        listed(&h.client, &session.id).await.notifications,
        mismatch_notifications,
        "a different persistent conversation withdraws silently"
    );
    assert_eq!(
        h.sup
            .session_snapshot(&session.id)
            .await
            .unwrap()
            .unwrap()
            .generation,
        mismatch_generation,
        "a mismatched record launches no generation"
    );
    std::fs::write(&record_path, original_record).expect("restore B's original owned transcript");
    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "compact",
        &format!("CODEX-COMPACT:{cleared_b}"),
    )
    .await;
    wait_for_offer(&h.client, &session.id, farhelm_proto::RestartOffer::Resume).await;

    h.client
        .restart_session(&session.id, true)
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
    // Post-relaunch survivor: the proven binding — exact target plus
    // provenance — survives the owner exit and the Resume relaunch that
    // replaced the foreground process, and the public offer still serves
    // Resume from it.
    let (saved, version, _) = durable_binding(h.state.path(), &session.id).await;
    let saved = saved.expect("the proven binding survives relaunch");
    assert!(
        saved.contains(&cleared_b),
        "relaunch must preserve B's exact saved target: {saved}"
    );
    assert_eq!(
        version, 1,
        "relaunch must preserve the proven binding's provenance"
    );
    // Stop must not recheck an already admitted target. A malformed header
    // would be refused by verification; an acknowledgement without changing
    // the durable binding proves the confirmation exited before that read.
    let path = records.path().join(format!("{cleared_b}.jsonl"));
    let intact = std::fs::read(&path).unwrap();
    let before = listed(&h.client, &session.id).await;
    let generation = h
        .sup
        .session_snapshot(&session.id)
        .await
        .unwrap()
        .unwrap()
        .generation;
    let stop_offset = std::fs::read(&hook_log).unwrap().len();
    std::fs::write(&path, b"not JSON\n").unwrap();
    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "stop",
        &format!("CODEX-STOP:{cleared_b}"),
    )
    .await;
    let stop_reply = supervisor_verdict(&h.sup, &hook_log, stop_offset, &cleared_b, None).await;
    assert!(stop_reply.contains(" acked "), "{stop_reply}");
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "Stop for a ready binding reads no header"
    );
    assert_eq!(
        durable_binding(h.state.path(), &session.id)
            .await
            .0
            .as_deref(),
        Some(saved.as_str())
    );
    std::fs::remove_file(&path).unwrap();
    let failure = h
        .client
        .restart_session(&session.id, true)
        .await
        .expect_err("missing transcript refuses Restart");
    assert!(
        failure.to_string().contains("missing or inconsistent"),
        "{failure:#}"
    );
    let after = listed(&h.client, &session.id).await;
    assert_eq!(
        h.sup
            .session_snapshot(&session.id)
            .await
            .unwrap()
            .unwrap()
            .generation,
        generation,
        "refusal launches no generation"
    );
    assert_eq!(
        after.restart_offer,
        farhelm_proto::RestartOffer::NotCaptured
    );
    assert_eq!(
        after.notifications.len(),
        before.notifications.len() + 1,
        "only Restart finding a missing file records the withdrawal"
    );
    std::fs::write(&path, intact).unwrap();
    send_and_wait(
        &h.client,
        channel,
        &mut stream,
        &mut seen,
        "compact",
        &format!("CODEX-COMPACT:{cleared_b}"),
    )
    .await;
    wait_for_offer(&h.client, &session.id, farhelm_proto::RestartOffer::Resume).await;

    // No supervisor exists while the foreground clears and completes a turn.
    // Its two actual hooks must leave independent slots, then reload must
    // select the replacement before judging the later confirmation.
    serving.stop().await;
    drop(stream);
    let Harness {
        client,
        sup,
        _tmux,
        state,
        _slot,
    } = h;
    drop(client);
    let retiring = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup) > 1 {
        assert!(
            tokio::time::Instant::now() < retiring,
            "connection still holds the supervisor"
        );
        // sleep-ok: observe owned connection drain before dropping the state-directory owner.
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);
    let socket = state.path().join("tmux.sock");
    let tmux_name = format!("fh-{}", session.id);
    let peer = tmux_query(&socket, &["has-session", "-t", &tmux_name]).await;
    assert!(
        peer.status.success(),
        "the owned vendor survives the supervisor outage"
    );
    let sent = tmux_query(
        &socket,
        &[
            "send-keys",
            "-t",
            &tmux_name,
            "clear-pending",
            "Enter",
            "persist",
            "Enter",
        ],
    )
    .await;
    assert!(sent.status.success());
    let dir = farhelm_supervisor::hook_report::session_dir(state.path(), &session.id).unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        if dir.join("selection.json").exists() && dir.join("enrichment.json").exists() {
            break;
        }
        let pane = tmux_query(&socket, &["capture-pane", "-p", "-t", &tmux_name]).await;
        assert!(
            tokio::time::Instant::now() < deadline,
            "both outage slots missing; pane={pane:?}"
        );
        // sleep-ok: poll independent hook slots while the owned vendor finishes both commands.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let selecting: farhelm_supervisor::hook_report::HookReport =
        serde_json::from_slice(&std::fs::read(dir.join("selection.json")).unwrap()).unwrap();
    let enriching: farhelm_supervisor::hook_report::HookReport =
        serde_json::from_slice(&std::fs::read(dir.join("enrichment.json")).unwrap()).unwrap();
    assert_ne!(selecting.conversation, cleared_b);
    assert_eq!(selecting.conversation, enriching.conversation);
    assert_eq!(
        selecting.hook_event_name,
        Some(serde_json::json!("SessionStart"))
    );
    assert_eq!(enriching.hook_event_name, Some(serde_json::json!("Stop")));
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        suite_timeouts(),
        SupervisorSeams::default(),
    )
    .await
    .unwrap();
    assert!(sup.owns_state_dir());
    let serving = ServeTask::spawn(&sup, state.path()).await;
    let client = connect_client(&sup).await;
    wait_for_offer(&client, &session.id, farhelm_proto::RestartOffer::Resume).await;
    let restored = durable_binding(state.path(), &session.id).await.0.unwrap();
    assert!(
        restored.contains(&selecting.conversation),
        "outage must recover the replacement: {restored}"
    );
    assert!(
        !dir.join("selection.json").exists() && !dir.join("enrichment.json").exists(),
        "both reports must settle"
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

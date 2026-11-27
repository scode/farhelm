//! Opt-in checks of the real vendors' conversation reporting hooks.
//!
//! Fixture tests exercise the supervisor without credentials; these ignored
//! tests establish that the vendor still runs the installed hook and reports
//! what Farhelm relies on. The drift suite ([`drift`], run through
//! `scripts/run-harness-drift.py`) covers each agent's launch, resume and
//! in-session changes such as `/clear` or `/new`; this file keeps one
//! standalone Claude audit, the helpers it uses, and the few the drift
//! suite borrows from it. They require vendor
//! authentication and network access, and are never part of ordinary CI.
//!
//! They run the agent the way a shipped supervisor does, in a terminal host
//! of its own. The agent is driven through an ordinary grid attachment
//! (`send_keys`) and read from the supervisor's own emulator of its host
//! ([`screen`]), never through a second connection to the host, which would
//! take it over from the supervisor.

use crate::harness::*;
use crate::hook_identity::ServeTask;

mod drift;

/// How much of the visible screen [`screen`] reads: far more than a 100x30
/// terminal's screen text, so the whole screen always comes back.
const SCREEN_BYTES: usize = 64 * 1024;

/// The agent's visible screen, as the supervisor's emulator of its terminal
/// host holds it, as one string.
///
/// Every polling loop in this file reads the screen rather than the
/// attachment's stream: a grid stream sends a row once per version of it,
/// in the order rows changed, so text in the transcript says nothing about
/// what the screen shows now (`harness::grid_text`). The screen is what a
/// user sees, and it is read from memory, so no read can hang on a wedged
/// terminal. Panics when the session's agent has no terminal host runtime,
/// which for these tests means the create did not start the agent where a
/// shipped supervisor would.
async fn screen(sup: &Supervisor, session_id: &str) -> String {
    sup.agent_host_for_test(session_id)
        .await
        .unwrap_or_else(|| panic!("session {session_id}'s agent has no terminal host runtime"))
        .screen_tail(SCREEN_BYTES)
}

/// The text a grid attachment's stream carried (`harness::grid_text`: every
/// row it sent, each time it changed, and every history line), newest
/// [`STREAM_TEXT_CAP`] bytes of it.
///
/// For an oracle that must not depend on what the screen still shows when
/// it is read: text that was on the screen, or entered the scrollback, is
/// here even after the program redrew over it. It says nothing about order
/// or count (a row is here once per version of it).
#[derive(Clone, Default)]
struct StreamText(Arc<std::sync::Mutex<Vec<u8>>>);

/// How much of a stream's text [`StreamText`] keeps: far more than one
/// screen and its recent history, bounded because a TUI repainting for
/// minutes sends megabytes of rows.
const STREAM_TEXT_CAP: usize = 1024 * 1024;

impl StreamText {
    /// Drain `stream` for the rest of the attachment, keeping its text; the
    /// returned task ends with the stream.
    fn record(&self, mut stream: TermStream) -> tokio::task::JoinHandle<()> {
        let kept = Arc::clone(&self.0);
        tokio::spawn(async move {
            while let Some(event) = stream.recv().await {
                if let TermEvent::Data(bytes) = event {
                    let mut kept = kept.lock().expect("the stream text");
                    kept.extend_from_slice(&grid_text(&bytes));
                    let excess = kept.len().saturating_sub(STREAM_TEXT_CAP);
                    kept.drain(..excess);
                }
            }
        })
    }

    /// Everything kept, as text.
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().expect("the stream text")).into_owned()
    }

    /// The newest `max` bytes kept, as text, for a failure message.
    fn tail(&self, max: usize) -> String {
        let kept = self.0.lock().expect("the stream text");
        String::from_utf8_lossy(&kept[kept.len().saturating_sub(max)..]).into_owned()
    }
}

/// Wait until the real agent's TUI is up and accepting input, answering
/// any folder-trust dialog it puts up on the way.
///
/// A fresh scratch directory is an UNTRUSTED workspace, and a modern agent
/// blocks on its own trust dialog before it will accept a prompt. Accepting
/// it here is not a workaround: farhelm passes the vendor's terminal
/// through untouched and never configures an agent (SPEC.md), so a real
/// user meets this same dialog and presses enter. This simulates only that
/// human half.
///
/// Two orderings are load-bearing, both learned by running this for real
/// against Claude Code v2.1.220:
///
/// 1. Dialog markers are checked BEFORE the ready marker, and a ready
///    marker must never be a substring of any dialog text. The first real
///    run matched "Claude Code" against the dialog's own body ("Claude
///    Code'll be able to read, edit, and execute files here"), broke the
///    wait, and typed the prompt into an unaccepted modal — no conversation
///    was ever started and capture correctly found nothing. Hence "Claude
///    Code v", which only the banner carries.
/// 2. Nothing slow may sit between accepting the dialog and whatever the
///    caller does next: the prompt must reach the ready agent.
///
/// Matching is against the RENDERED screen ([`screen`]), not the stream: a
/// TUI's first paint arrives as cursor-positioned fragments.
///
/// Accepted side effect: accepting trust writes the (soon-deleted) scratch
/// path into the user's real agent config. That is the vendor's own write,
/// and the same class of consequence these tests already embrace by
/// observing the real HOME.
async fn wait_for_agent_tui_ready(
    sup: &Supervisor,
    client: &SupervisorClient,
    session_id: &str,
    chan: u32,
    ready_marker: &str,
    trust_dialog_markers: &[&str],
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        let text = screen(sup, session_id).await;
        // The deadline is checked FIRST, before either branch: a dialog
        // that never advances no matter how often it is answered is a real
        // failure mode (codex's once did exactly that), and a dialog branch
        // that looped straight back to the top would press enter at it
        // forever instead of failing with the screen printed.
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {ready_marker:?}; rendered screen:\n{text}"
        );
        if trust_dialog_markers.iter().any(|m| text.contains(m)) {
            // Claude's trust menu can default to refusal. Move only when
            // the rendered selection proves that Enter would exit; older
            // affirmative defaults still need only Enter.
            let answer: &[u8] =
                if text.contains("❯ No, exit") && text.contains("Yes, I trust this folder") {
                    b"\x1b[B\r"
                } else {
                    b"\r"
                };
            client.send_keys(chan, answer.to_vec()).await;
            // sleep-ok: pace dialog-answer retries; the next rendered marker decides readiness.
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }
        if text.contains(ready_marker) {
            return;
        }
        // sleep-ok: poll the rendered readiness marker between deadline checks.
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

// ---------------------------------------------------------------------
// Real-agent HOOK audit (plan §4.4)
//
// The test below is a tripwire for a vendor fact the injected
// `SessionStart` hook rests on, verified by hand once and guaranteed by
// nothing but the vendor's goodwill: a `claude` started from Claude's own
// Bash tool inherits the session's reporting credential but runs as a
// process of its own, so Farhelm's positional admission rule can refuse
// its report. If that changes, the symptom is SILENT: a shelled-out child
// quietly takes over the session's resume target.
//
// This file once held standalone rotation audits too. Of what they pinned,
// Claude's report at process start and on `/clear`, Codex's lag after
// `/new`, and the `source` each rotation's report carries are now checked
// by the drift suite's `conversation_rotation` category
// (`drift/conversation_rotation.rs`), through the shipped launch and
// Farhelm's own hook. Codex's first-prompt report timing is checked by
// `capture_timing_hooks`. Codex's `Stop` confirming a pending `/clear` is
// covered by the supervisor's fixtures. The Codex release the category was
// written against reported a `/clear` already resumable, so no pending
// clear came up live; if a later release sends one, the category's
// post-prompt wait passes only once that `Stop` has confirmed it.
//
// Claude honouring a `--settings` hook block passed on the command line is
// asserted implicitly and unavoidably: no report arrives at all if the
// injected flags are not honoured.
//
// Run it deliberately, through the recorder like any other runtime test,
// in its generic mode (nextest's per-test limit of five minutes is shorter
// than this audit's own deadlines), with the vendor's own login in the
// real HOME and none of an enclosing agent session's own variables
// (`CLAUDECODE`, its API credentials) in the environment:
//
//     python3 scripts/record-test-run.py --kind development \
//         --selection 'real Claude shelled-out child audit' \
//         --concurrency 'one test; libtest one thread' \
//         -- cargo test -p farhelm --test e2e -- --ignored --exact \
//         --test-threads 1 \
//         real_agent_capture::real_claude_shelled_out_child_cannot_replace_the_session_conversation
// ---------------------------------------------------------------------

/// Bring up a supervisor that is genuinely LISTENING on its unix socket,
/// and one client for it.
///
/// The hook tests cannot use the in-process duplex pipe every other test
/// here rides on: the hook is a separate process whose only way back to the
/// supervisor is `FARHELM_SUPERVISOR_SOCK`.
///
/// The accept loop comes back as a [`ServeTask`] rather than a bare
/// `JoinHandle`, because dropping a `JoinHandle` DETACHES its task instead
/// of stopping it: a test that failed an assertion would leave the loop
/// running against a state directory that is about to be deleted, holding
/// an `Arc<Supervisor>` alive for the rest of the binary's run. The guard
/// aborts on drop and its [`ServeTask::stop`] reports a `serve()` that
/// ended any other way.
///
/// Callers pass default seams unless they must redirect a directory, and
/// that is the point: hooks are on by default, and the agent runs in a
/// terminal host started from the built `farhelm`, so this exercises the
/// configuration a user gets. The drift cases redirect only the vendor
/// configuration directories of agents with global hook files
/// (`drift/private_config.rs`).
async fn serving_supervisor(
    state: &std::path::Path,
    seams: SupervisorSeams,
) -> (Arc<Supervisor>, Arc<SupervisorClient>, ServeTask) {
    let sup = Supervisor::new_with_seams(
        state,
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        seams,
    )
    .await
    .expect("supervisor");
    let accepting = ServeTask::spawn(&sup, state).await;
    let client = connect_client(&sup).await;
    (sup, client, accepting)
}

/// Poll until this session holds a resumable conversation identity that the
/// hook reported, and return it: the parent conversation the Claude audit
/// below then defends against its shelled-out child.
///
/// `list_sessions` is called each round because the supervisor's own
/// cadence is not what a test should depend on, and the two-minute deadline
/// is generous because a real vendor's startup sits inside it.
///
/// The identity is not accepted until the hook's own log ACKNOWLEDGES it.
/// Records alone do not supply an identity; requiring the acknowledgement
/// keeps the audit from passing after the vendor's hook stopped firing. An
/// `acked` line names the runtime id the supervisor accepted, and the
/// supervisor writes it only after picking up a report file that a hook
/// process wrote.
async fn wait_for_reported_identity(
    sup: &Supervisor,
    client: &SupervisorClient,
    state: &std::path::Path,
    session_id: &str,
) -> String {
    let secs = 120;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        client.list_sessions().await.expect("list");
        let snapshot = sup
            .session_snapshot(session_id)
            .await
            .expect("snapshot")
            .expect("present");
        if let Some(conversation) = snapshot.captured_conversation
            && snapshot.restart_offer == farhelm_proto::RestartOffer::Resume
            && hook_acked(state, session_id, &reported_runtime_identity(&conversation))
        {
            return conversation;
        }
        // On timeout, dump everything a human would go looking for by
        // hand: the rendered screen (did the prompt submit? is a dialog up?
        // did the turn error?) and the hook's own trace (did it run, and
        // what did the supervisor answer?). A bare "no identity" message
        // costs whoever runs this audit a full re-run under a probe.
        if tokio::time::Instant::now() >= deadline {
            let hook_log = state.join("hook-log").join(format!("{session_id}.log"));
            panic!(
                "no reported identity within {secs}s\n--- hook log ({}):\n{}\n--- screen:\n{}",
                hook_log.display(),
                std::fs::read_to_string(&hook_log).unwrap_or_else(|e| format!("<unreadable: {e}>")),
                screen(sup, session_id).await,
            );
        }
        // sleep-ok: poll changed identity plus hook acknowledgement between deadline checks.
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Whether the hook log records a supervisor-accepted runtime identity for
/// this session.
///
/// Tolerates a log file that does not exist yet, because it is called while
/// polling: before any hook has run there is nothing to read, and that is
/// an ordinary "not yet" rather than a failure.
fn hook_acked(state: &std::path::Path, session_id: &str, runtime_session_id: &str) -> bool {
    let path = state.join("hook-log").join(format!("{session_id}.log"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        return false;
    };
    text.lines()
        .filter_map(farhelm_supervisor::hook_report::parse_log_line)
        .any(|entry| {
            entry.word == "acked"
                && entry
                    .about
                    .is_some_and(|(id, _)| hook_log_names_identity(id, runtime_session_id))
        })
}

/// Extract the value Claude's resume command must receive from a durable
/// capture. Only the Claude audit below calls it.
///
/// Claude keeps its conversation id in the column, bare or inside a `claude:`
/// locator that adds the transcript Restart checks for; either way the id is
/// what its resume argv names. This helper deliberately proves that
/// observable argv effect without asserting the locator's incidental JSON
/// layout beyond the field needed to identify the vendor's real resume
/// target.
fn resume_identity(conversation: &str) -> String {
    let Some(locator) = conversation.strip_prefix("claude:") else {
        return conversation.to_string();
    };
    serde_json::from_str::<serde_json::Value>(locator)
        .expect("the stored locator is valid JSON")
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .expect("a resumable locator carries the id its resume names")
        .to_string()
}

/// Extract the exact runtime id the hook reported. Stored locators carry other evidence for the later resume check
/// (Claude's, the transcript Restart checks for).
///
/// Only Codex, Grok and Claude locators are parsed. Anything else comes back whole, which is right for a bare id but
/// not for a Pi or OMP locator: its `session_id` is not extracted, so comparing the result with the hook log or with
/// another runtime id cannot be relied on for those two harnesses. `capture_timing_hooks` parses Pi and OMP locators
/// itself (`turn_identity`), and `conversation_rotation` uses its ID checks only for Claude and Codex.
fn reported_runtime_identity(conversation: &str) -> String {
    let Some((locator, field)) = conversation
        .strip_prefix("codex:")
        .map(|locator| (locator, "runtime_session_id"))
        .or_else(|| {
            conversation
                .strip_prefix("grok:")
                .map(|locator| (locator, "session_id"))
        })
        .or_else(|| {
            conversation
                .strip_prefix("claude:")
                .map(|locator| (locator, "session_id"))
        })
    else {
        return conversation.to_string();
    };
    serde_json::from_str::<serde_json::Value>(locator)
        .expect("the stored locator is valid JSON")
        .get(field)
        .and_then(serde_json::Value::as_str)
        .expect("the locator carries the hook's runtime session id")
        .to_string()
}

/// Locator reports carry the ID before their file evidence, which can be sanitized or cut short in the log.
/// Compare the JSON string field without relying on the rest of the logged locator being parseable.
fn hook_log_names_identity(reported: &str, identity: &str) -> bool {
    reported == identity
        || (["grok:", "pi:", "omp:"]
            .into_iter()
            .any(|prefix| reported.starts_with(prefix))
            && reported.contains(&format!("\"session_id\":{}", serde_json::json!(identity))))
}

/// Assert the resume the supervisor would run names the captured persistent
/// conversation target, not necessarily its durable storage representation.
fn assert_resume_names(snapshot: &SessionSnapshot, conversation: &str) {
    assert_eq!(snapshot.restart_offer, farhelm_proto::RestartOffer::Resume);
    let resume = snapshot
        .resume_argv
        .as_ref()
        .expect("a Resume offer has a filled argv");
    let expected = resume_identity(conversation);
    assert!(
        resume.iter().any(|element| element == &expected),
        "the reported identity's resumable target must land in the resume argv: {resume:?}"
    );
    assert!(
        !resume.iter().any(|element| element == "{conversation}"),
        "no placeholder may survive substitution: {resume:?}"
    );
}

/// Claude Code for real: a `claude` the session starts through its own Bash
/// tool, carrying a Farhelm reporting hook of its own, cannot replace the
/// session's conversation.
///
/// This audits the vendor facts Claude's positional admission rule rests
/// on, which no fixture can: that a child started from Claude's Bash tool
/// inherits the session credential (so its report reaches the supervisor,
/// and without the rule it WOULD land), and that the child is a process of
/// its own below the parent `claude`, whose report at startup was admitted
/// and pinned the parent as this launch's emitter, so the rule refuses the
/// child's (`service::core::vendor::positional`). `child.sh` `exec`s the
/// child so the Bash tool's own shell is the only intermediary, the
/// tightest shape a shell-out takes.
/// It records the child's ancestry to `child-ancestry.txt` first, and the
/// test prints that file, so a failure (or a vendor change) names the chain
/// that was actually observed.
///
/// Costs two short Haiku turns (the parent's, and the child's `-p`), and
/// runs the parent with `--dangerously-skip-permissions` inside a scratch
/// directory so its Bash tool does not stop on an approval prompt. The
/// child gets its hook from `--settings`, standing in for a user- or
/// project-level settings hook: that is the path a shelled-out sub-agent
/// has to a reporter, since the parent's injected `--settings` lives in
/// the parent's argv and is not inherited.
#[farhelm_testtrace::test]
#[ignore = "needs real Claude Code credentials and network; two short model turns; run deliberately"]
async fn real_claude_shelled_out_child_cannot_replace_the_session_conversation() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    // Declared before the state directory so that, on a failure too, the
    // state directory's teardown (which ends its terminal hosts, and the
    // vendor agent with them) runs before the agent's folder is removed.
    let work = farhelm_teststate::tempdir().expect("workdir");
    let state = farhelm_teststate::tempdir().expect("state dir");
    let (sup, client, accepting) =
        serving_supervisor(state.path(), SupervisorSeams::default()).await;

    let hook = format!(
        "{} internal hook --vendor claude",
        shell_words::quote(farhelm_bin())
    );
    let settings = serde_json::json!({
        "hooks": {"SessionStart": [{"hooks": [{"type": "command", "command": hook}]}]}
    })
    .to_string();
    let script = format!(
        "#!/bin/sh\n\
         p=$$\n\
         : > child-ancestry.txt\n\
         while [ \"$p\" -gt 1 ]; do\n\
         \tps -o pid=,ppid=,comm= -p \"$p\" >> child-ancestry.txt\n\
         \tp=$(ps -o ppid= -p \"$p\" | tr -d ' ')\n\
         done\n\
         exec claude -p --model haiku --settings {} 'Reply with the single word CHILDOK.'\n",
        shell_words::quote(&settings)
    );
    std::fs::write(work.path().join("child.sh"), script).expect("child.sh");

    let session = client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            real_claude_launch("claude --dangerously-skip-permissions --model haiku"),
            None,
            100,
            30,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .unwrap_or_else(|e| panic!("launching the real claude: {e:#}"));
    let (chan, _replay, mut rx) = client
        .attach_live(&session.id, 100, 30)
        .await
        .expect("attach");
    // Drained for the whole run, not dropped: a viewer that stops reading
    // is eventually detached as stalled, after which every keystroke lands
    // on a dead channel and the agent looks like it is swallowing them.
    // This test reads the SCREEN for its assertions, so the stream's only
    // job is to keep the attachment alive.
    let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
    wait_for_agent_tui_ready(
        &sup,
        &client,
        &session.id,
        chan,
        "Claude Code v",
        &["Accessing workspace", "Do you trust"],
    )
    .await;
    let parent = wait_for_reported_identity(&sup, &client, state.path(), &session.id).await;
    // The hook log names reported ids, while `parent` is the stored value, which may be a locator.
    let parent_id = reported_runtime_identity(&parent);

    let prompt = "Use your Bash tool to run exactly this command and nothing else: sh ./child.sh";
    client.send_keys(chan, prompt.as_bytes().to_vec()).await;
    let log_path = state
        .path()
        .join("hook-log")
        .join(format!("{}.log", session.id));
    // The child's first line is the hook's own `written`; the supervisor's
    // verdict on that report follows on its next pass, and the verdict is what
    // this audit is about.
    let child_line = |text: &str| {
        text.lines()
            .find(|line| {
                farhelm_supervisor::hook_report::parse_log_line(line).is_some_and(|entry| {
                    entry.word == "no-credential"
                        || matches!(entry.word, "refused" | "acked")
                            && entry
                                .identity()
                                .is_some_and(|identity| identity != parent_id)
                })
            })
            .map(str::to_string)
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
    let child = loop {
        let log = std::fs::read_to_string(&log_path).unwrap_or_default();
        if let Some(line) = child_line(&log) {
            break line;
        }
        let text = screen(&sup, &session.id).await;
        assert!(
            tokio::time::Instant::now() < deadline,
            "the shelled-out child never reported; hook log:\n{log}\n--- screen:\n{text}"
        );
        // Enter is re-pressed only while the prompt still sits in the
        // composer: a lone Enter was seen to vanish on a loaded machine, and
        // once the prompt has left the composer a further Enter would land on
        // whatever the agent is doing instead.
        if text.contains(prompt) {
            client.send_keys(chan, b"\r".to_vec()).await;
        }
        // sleep-ok: pace conditional Enter retries while polling the hook log in this manual audit.
        tokio::time::sleep(Duration::from_secs(2)).await;
    };
    let ancestry = std::fs::read_to_string(work.path().join("child-ancestry.txt"))
        .unwrap_or_else(|e| format!("<no ancestry recorded: {e}>"));
    eprintln!("observed child ancestry (pid ppid comm), innermost first:\n{ancestry}");

    assert!(
        farhelm_supervisor::hook_report::parse_log_line(&child).is_some_and(|entry| {
            entry.word == "refused"
                && entry.detail.starts_with("conflict ")
                && entry.detail.contains("pinned emitter")
        }),
        "the child's report must reach the supervisor and be refused as another process than \
         the pinned emitter (`no-credential` would mean the Bash tool no longer passes the \
         credential through); line: {child}\nancestry:\n{ancestry}"
    );
    let snapshot = sup
        .session_snapshot(&session.id)
        .await
        .expect("snapshot")
        .expect("present");
    assert_eq!(
        snapshot.captured_conversation.as_deref(),
        Some(parent.as_str()),
        "the session must still resume the foreground's conversation"
    );
    assert_resume_names(&snapshot, &parent);

    // Teardown BEFORE the permit is released. `SLOTS` bounds how many
    // supervisors, terminal hosts and vendor agents run at once, so handing
    // the permit back while this test's own are still being torn down
    // admits the next harness into a machine that is still carrying this
    // one. A terminal host outlives its supervisor by design, so dropping
    // the supervisor ends nothing: the hosts (hanging up the vendor agent in
    // them) are ended explicitly, then the agent's folder goes.
    accepting.stop().await;
    drain.abort();
    drop(client);
    drop(sup);
    end_all_terminals(state.path()).await;
    drop(state);
    drop(work);
    drop(slot);
}

/// A command launch declaring Claude around the real `claude` `command`,
/// with Claude's resume command, the way a user would type it.
///
/// Declaring the agent is what turns on Farhelm's hooks for a command
/// launch; an undeclared command gets none, so the audit would launch the
/// real agent and then wait for a report that is never sent. Only the Claude
/// audit launches this way; the drift suite goes through the shipped
/// launcher instead.
fn real_claude_launch(command: &str) -> farhelm_proto::SessionLaunch {
    declared_command(
        &format!("{command} {{farhelm_args}}"),
        farhelm_proto::LaunchHarness::Claude,
        Some(&format!(
            "{command} --resume {{conversation}} {{farhelm_args}}"
        )),
    )
}

/// First `name` on `PATH` that is an executable regular file.
fn which_binary(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| {
            std::fs::metadata(candidate).is_ok_and(|m| {
                use std::os::unix::fs::PermissionsExt;
                m.is_file() && m.permissions().mode() & 0o111 != 0
            })
        })
}

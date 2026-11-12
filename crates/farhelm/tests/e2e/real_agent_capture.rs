//! Opt-in checks of the real vendors' conversation reporting hooks.
//!
//! Fixture tests exercise the supervisor without credentials; these ignored
//! tests establish that the vendor still runs the installed hook and reports
//! changes such as `/clear` or `/new`. They require vendor authentication and
//! network access, and are never part of ordinary CI.
//!
//! They run the agent the way a shipped supervisor does, in a terminal host
//! of its own. The agent is driven through an ordinary grid attachment
//! (`send_keys`) and read from the supervisor's own emulator of its host
//! ([`screen`]), never through a second connection to the host, which would
//! take it over from the supervisor.

use crate::harness::*;
use crate::hook_identity::ServeTask;

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
// The tests below are the tripwire for the vendor facts the injected
// `SessionStart` hook rests on. Every one of them was verified by hand
// once; none of them is guaranteed by anything but the vendors' goodwill,
// and if one changes the symptom is SILENT — identity capture quietly
// loses reports about a mid-process `/clear` or
// `/new` at all.
//
// What they pin, fact by fact:
//
// - Codex reports `session_id` to correlate the runtime process, while its
//   root transcript supplies the persistent id a later resume needs.
// - Claude fires `SessionStart` at PROCESS START, before any prompt — the
//   identity appears without the test ever typing.
// - Claude fires it AGAIN after `/clear`, with a NEW `session_id` and
//   `source` `clear`, with no model turn involved.
// - Codex fires it at FIRST PROMPT SUBMISSION rather than at process
//   start, and fires it again after `/new` with a new id and `source`
//   still `startup` — which is why nothing in this codebase keys on
//   `source`.
// - Claude honours a `--settings` hook block passed on the command line,
//   and Codex honours `-c hooks.SessionStart=...` together with
//   `-c features.hooks=true` and `--dangerously-bypass-hook-trust`. Both
//   are asserted implicitly and unavoidably: no report arrives at all if
//   the injected flags are not honoured.
//
// The `source` values are read out of the hook's own per-session log,
// because the supervisor deliberately logs `source` rather than storing it
// (plan §2.6 keeps the schema to one column). That file is the only place
// the value survives, which makes it the only place these facts can be
// checked from.
//
// Run them individually and deliberately, through the recorder like any
// other runtime test, in its generic mode (nextest's per-test limit ends a
// test after five minutes, shorter than a Codex audit's own deadlines),
// with the vendor's own login in the real HOME and none of an enclosing
// agent session's own variables (`CLAUDECODE`, its API credentials) in the
// environment:
//
//     python3 scripts/record-test-run.py --kind development \
//         --selection 'real Claude hook audit across /clear' \
//         --concurrency 'one test; libtest one thread' --tmux none \
//         -- cargo test -p farhelm --test e2e -- --ignored --exact \
//         --test-threads 1 \
//         real_agent_capture::real_claude_session_reports_its_identity_across_clear
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
/// Default seams, and that is the point: hooks are on by default, and the
/// agent runs in a terminal host started from the built `farhelm`, so this
/// exercises the configuration a user gets.
async fn serving_supervisor(
    state: &std::path::Path,
) -> (Arc<Supervisor>, Arc<SupervisorClient>, ServeTask) {
    let sup = Supervisor::new_with_seams(
        state,
        farhelm_bin().into(),
        suite_timeouts(),
        SupervisorSeams::default(),
    )
    .await
    .expect("supervisor");
    let accepting = ServeTask::spawn(&sup, state).await;
    let client = connect_client(&sup).await;
    (sup, client, accepting)
}

/// Poll until this session holds a conversation identity that differs from
/// `previous`, and return it.
///
/// `previous` is what turns this from "an identity appeared" into "the
/// identity CHANGED", which is the whole question after a `/clear` or a
/// `/new`: a build whose hook fired only once would satisfy every
/// first-half assertion and leave the user resuming the conversation they
/// just walked away from.
///
/// `list_sessions` is called each round because the supervisor's own
/// cadence is not what a test should depend on, and the deadline is
/// generous because a real vendor's startup (and, for codex, a real model
/// round trip) sits inside it.
///
/// The identity is not accepted until the hook's own log ACKNOWLEDGES it.
/// Records alone do not supply an identity;
/// requiring the acknowledgement keeps either vendor from passing this audit
/// after its hook stopped firing. An `acked` line names the runtime id the
/// supervisor accepted, and the supervisor writes it only after picking up a
/// report file that a hook process wrote.
async fn wait_for_reported_identity(
    sup: &Supervisor,
    client: &SupervisorClient,
    state: &std::path::Path,
    session_id: &str,
    previous: Option<&str>,
    secs: u64,
) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        client.list_sessions().await.expect("list");
        let snapshot = sup
            .session_snapshot(session_id)
            .await
            .expect("snapshot")
            .expect("present");
        if let Some(conversation) = snapshot.captured_conversation
            && Some(conversation.as_str()) != previous
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
                "no reported identity{} within {secs}s\n--- hook log ({}):\n{}\n--- screen:\n{}",
                match previous {
                    Some(was) => format!(" other than {was}"),
                    None => String::new(),
                },
                hook_log.display(),
                std::fs::read_to_string(&hook_log).unwrap_or_else(|e| format!("<unreadable: {e}>")),
                screen(sup, session_id).await,
            );
        }
        // sleep-ok: poll changed identity plus hook acknowledgement between deadline checks.
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Every line the hook wrote for this session.
///
/// The hook is silent by contract, so this file is the only record of what
/// it saw — including the `source` value the supervisor logs but does not
/// store, which is exactly the vendor fact these tests exist to pin.
fn hook_log_lines(state: &std::path::Path, session_id: &str) -> Vec<String> {
    let path = state.join("hook-log").join(format!("{session_id}.log"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the hook must leave a trace at {}: {e}", path.display()));
    text.lines().map(str::to_string).collect()
}

/// Whether the hook log records a supervisor-accepted runtime identity for
/// this session.
///
/// Tolerates a log file that does not exist yet, because it is called while
/// polling: before any hook has run there is nothing to read, and that is
/// an ordinary "not yet" rather than a failure. The line shape is
/// `<unix-seconds> acked <conversation> <source>` (`crate::hook`'s module
/// docs), and `acked` carries no detail, so the id is always the third
/// field.
fn hook_acked(state: &std::path::Path, session_id: &str, runtime_session_id: &str) -> bool {
    let path = state.join("hook-log").join(format!("{session_id}.log"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        return false;
    };
    text.lines().any(|line| {
        let mut fields = line.split_whitespace().skip(1);
        fields.next() == Some("acked") && fields.next() == Some(runtime_session_id)
    })
}

/// Assert this session's stored identity STAYS `expected` for `secs`, with
/// list passes driving the supervisor throughout.
///
/// A negative check about timing, and deliberately a short one: what it
/// pins is that the identity does not appear (or move) BEFORE the vendor
/// event that is supposed to produce it, and a few seconds of a driven
/// supervisor is enough to catch a build that reports at the wrong moment.
/// Spending a full deadline here would only make an audit that already
/// takes minutes take longer to say the same thing.
async fn assert_identity_stays(
    sup: &Supervisor,
    client: &SupervisorClient,
    session_id: &str,
    expected: Option<&str>,
    secs: u64,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    while tokio::time::Instant::now() < deadline {
        client.list_sessions().await.expect("list");
        let snapshot = sup
            .session_snapshot(session_id)
            .await
            .expect("snapshot")
            .expect("present");
        assert_eq!(
            snapshot.captured_conversation.as_deref(),
            expected,
            "the identity moved before the vendor event that is supposed to move it"
        );
        // sleep-ok: space repeated negative identity samples across the observation interval.
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Extract the value a resume command must receive from a durable capture.
///
/// Claude keeps its persistent conversation id directly in the column. Codex
/// keeps a locator there so a later refresh can revalidate the exact
/// transcript; its resume argv must instead name the persistent thread id.
/// This helper deliberately proves that observable argv effect without
/// asserting the locator's incidental JSON layout beyond the field needed to
/// identify the vendor's real resume target.
fn resume_identity(conversation: &str) -> String {
    let Some(locator) = conversation.strip_prefix("codex:") else {
        return conversation.to_string();
    };
    serde_json::from_str::<serde_json::Value>(locator)
        .expect("the stored Codex locator is valid JSON")
        .get("thread_id")
        .and_then(serde_json::Value::as_str)
        .expect("a resumable Codex locator carries its persistent thread id")
        .to_string()
}

/// Extract the exact runtime id the hook reported. Codex stores this beside
/// its persistent thread id so a later transcript recheck can prove both
/// sides of the vendor's identity contract.
fn reported_runtime_identity(conversation: &str) -> String {
    let Some(locator) = conversation.strip_prefix("codex:") else {
        return conversation.to_string();
    };
    serde_json::from_str::<serde_json::Value>(locator)
        .expect("the stored Codex locator is valid JSON")
        .get("runtime_session_id")
        .and_then(serde_json::Value::as_str)
        .expect("a Codex locator carries the hook's runtime session id")
        .to_string()
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

/// Claude Code for real, across a `/clear`.
///
/// This is the case the whole hook design exists for and the one no amount
/// of outside observation can reach: `/clear` starts a new conversation
/// inside the SAME process, with no new session, no new first input, and
/// nothing for the record scan to correlate. Before the hook, a session
/// cleared this way would keep offering to resume the conversation the user
/// deliberately abandoned.
///
/// Costs nothing at the API: Claude fires `SessionStart` at process start
/// and again on `/clear` without any model turn, so this test never submits
/// a prompt. Requires a working `claude` on PATH, already authenticated.
///
/// The `source` assertion is the vendor fact, not decoration: the payload
/// says `startup` the first time and `clear` the second, and this codebase
/// deliberately ignores the field. If a future version stopped
/// distinguishing them, nothing would break — but if it stopped FIRING the
/// second time, everything would, silently. Both lines are checked so a
/// regression names which of the two it is.
#[farhelm_testtrace::test]
#[ignore = "needs real Claude Code credentials; run deliberately"]
async fn real_claude_session_reports_its_identity_across_clear() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    // Declared before the state directory so that, on a failure too, the
    // state directory's teardown (which ends its terminal hosts, and the
    // vendor agent with them) runs before the agent's folder is removed.
    let work = farhelm_teststate::tempdir().expect("workdir");
    let state = farhelm_teststate::tempdir().expect("state dir");
    let (sup, client, accepting) = serving_supervisor(state.path()).await;

    let session = client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            real_agent_launch("claude", farhelm_proto::LaunchHarness::Claude),
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
    // These tests read the SCREEN for their assertions, so the stream's
    // only job is to keep the attachment alive.
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

    // No prompt is ever submitted: the identity below can only have come
    // from the startup hook, which is precisely the fact under test.
    let first =
        wait_for_reported_identity(&sup, &client, state.path(), &session.id, None, 120).await;
    assert_resume_names(
        &sup.session_snapshot(&session.id)
            .await
            .expect("snapshot")
            .expect("present"),
        &first,
    );

    // `/clear` is driven by what the screen shows, not by sleeps, in both
    // halves — the same discipline `submit_prompt` uses, and for the same
    // reason: individual keystroke bursts were observed to vanish on a
    // loaded machine, and a blind Enter that missed would submit `/clear`
    // as a chat message (costing a model turn and never clearing anything)
    // while this test waited out its whole deadline.
    //
    // Both halves count OCCURRENCES against a baseline taken before
    // anything is typed, rather than testing for the string. Claude's own
    // startup chrome may already name `/clear` (its tip line has), and a
    // bare `contains` would then call the command typed before a key was
    // pressed and never call it executed afterwards — a test that passed
    // and proved nothing, then hung.
    let clears = |text: &str| text.matches("/clear").count();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let baseline = clears(&screen(&sup, &session.id).await);
    // The command and its Enter are separate keystrokes, as a human's are.
    // Retyping is guarded on the composer being genuinely empty of it, or
    // a slow render would leave `/clear/clear` in the box.
    loop {
        let text = screen(&sup, &session.id).await;
        if clears(&text) > baseline {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the composer never showed the typed /clear; rendered screen:\n{text}"
        );
        client.send_keys(chan, b"/clear".to_vec()).await;
        // sleep-ok: pace command retries while polling a new rendered occurrence of /clear.
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    // Executed, not merely typed — and the only proof of that which cannot
    // be misread is the identity itself moving. `/clear` re-renders
    // Claude's welcome chrome, whose tips can name `/clear` too, so no
    // count of the word on screen says anything (an earlier shape of this
    // wait counted it and hung on exactly that). Enter is re-pressed on a
    // cadence because a lone one was observed to vanish, and each press is
    // followed by a short poll of the reported identity; extra Enters on
    // an empty composer are inert, and the wait below is the final word.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    'entered: loop {
        client.send_keys(chan, b"\r".to_vec()).await;
        for _ in 0..10 {
            // sleep-ok: sample identity between Enter retries; the outer loop checks the deadline.
            tokio::time::sleep(Duration::from_millis(500)).await;
            let snapshot = sup
                .session_snapshot(&session.id)
                .await
                .expect("snapshot")
                .expect("present");
            if snapshot
                .captured_conversation
                .as_deref()
                .is_some_and(|c| c != first)
            {
                break 'entered;
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Enter never executed /clear (identity still {first}); rendered screen:\n{}",
            screen(&sup, &session.id).await
        );
    }

    let cleared =
        wait_for_reported_identity(&sup, &client, state.path(), &session.id, Some(&first), 120)
            .await;
    assert_resume_names(
        &sup.session_snapshot(&session.id)
            .await
            .expect("snapshot")
            .expect("present"),
        &cleared,
    );

    let log = hook_log_lines(state.path(), &session.id);
    assert!(
        log.iter()
            .any(|line| line.contains(&first) && line.ends_with(" startup")),
        "claude must report at process start with source `startup`: {log:?}"
    );
    assert!(
        log.iter()
            .any(|line| line.contains(&cleared) && line.ends_with(" clear")),
        "claude must report again after /clear with source `clear`: {log:?}"
    );

    // Teardown BEFORE the permit is released. `SLOTS` bounds how many
    // supervisors, terminal hosts and vendor agents run at once, so handing
    // the permit back while this test's own are still being torn down
    // admits the next harness into a machine that is still carrying this
    // one — which is exactly the load the semaphore exists to prevent. A
    // terminal host outlives its supervisor by design, so dropping the
    // supervisor ends nothing: the hosts (hanging up the vendor agent in
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
    let (sup, client, accepting) = serving_supervisor(state.path()).await;

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
            real_agent_launch(
                "claude --dangerously-skip-permissions --model haiku",
                farhelm_proto::LaunchHarness::Claude,
            ),
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
    // Drained for the same reason as in the `/clear` audit above.
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
    let parent =
        wait_for_reported_identity(&sup, &client, state.path(), &session.id, None, 120).await;

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
            .find(|line| !line.contains(&parent) && !line.contains(" written "))
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
        // composer, as in `submit_prompt`.
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
        child.contains(" refused conflict ") && child.contains("pinned emitter"),
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

    // Teardown before the permit is released, as in the `/clear` audit.
    accepting.stop().await;
    drain.abort();
    drop(client);
    drop(sup);
    end_all_terminals(state.path()).await;
    drop(state);
    drop(work);
    drop(slot);
}

/// Codex for real, across a `/new`.
///
/// Codex's timing is the mirror image of Claude's and the reason nothing in
/// this codebase keys on `source`: the hook fires at FIRST PROMPT
/// SUBMISSION rather than at process start, and after `/new` the next
/// prompt fires it again with a new id and `source` still `startup`. So
/// this test has to pay for two short completions, and the identity is
/// expected to lag the `/new` until a prompt actually goes out — a lag a
/// user sees too, and one worth re-verifying whenever codex is bumped.
///
/// Codex runs with this machine's own configuration and login, and the
/// session's directory is trusted for this invocation only
/// ([`trusted_codex`]), so that no folder-trust modal stands between the
/// launch and the first prompt.
#[farhelm_testtrace::test]
#[ignore = "needs real Codex credentials and network; run deliberately"]
async fn real_codex_session_reports_its_identity_across_new() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    // Declared before the state directory so that, on a failure too, the
    // state directory's teardown (which ends its terminal hosts, and the
    // vendor agent with them) runs before the agent's folder is removed.
    let work = farhelm_teststate::tempdir().expect("workdir");
    let state = farhelm_teststate::tempdir().expect("state dir");
    let agent = trusted_codex(work.path());
    let (sup, client, accepting) = serving_supervisor(state.path()).await;

    let session = client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            real_agent_launch(&agent, farhelm_proto::LaunchHarness::Codex),
            None,
            100,
            30,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .unwrap_or_else(|e| panic!("launching the real codex: {e:#}"));
    let (chan, _replay, rx) = client
        .attach_live(&session.id, 100, 30)
        .await
        .expect("attach");
    // See the claude test above: the stream must stay drained or the
    // attachment overflow-detaches and later input silently stops landing
    // — with far more rendered output here (codex repaints heavily), this
    // test hit exactly that as minutes-in keystroke loss.
    //
    // Kept rather than only drained: `/new`'s closing notice can leave the
    // visible screen before a read of it (the first run on terminal hosts
    // found the screen already showing the new chat and no notice), while
    // every row and history line the stream sent is in this transcript
    // ([`StreamText`]).
    let transcript = StreamText::default();
    let drain = transcript.record(rx);
    // No dialog markers: the invocation trusts the directory, so the modal
    // must not appear at all.
    wait_for_agent_tui_ready(&sup, &client, &session.id, chan, "OpenAI Codex (v", &[]).await;

    // Codex fires its hook at first prompt SUBMISSION, so before one goes
    // out there must be no identity at all. Without this the test would
    // pass just as well against a codex that reported at startup — a
    // different vendor fact, and one this file's banner claims is false.
    assert_identity_stays(&sup, &client, &session.id, None, 5).await;

    // Digit-free, for the same reason the capture test's prompt is.
    submit_prompt(&sup, &client, &session.id, chan, "ok").await;
    // What `submit_prompt` waited for: the answer on the screen.
    let first_answer = "• ok";
    let first =
        wait_for_reported_identity(&sup, &client, state.path(), &session.id, None, 300).await;
    assert_resume_names(
        &sup.session_snapshot(&session.id)
            .await
            .expect("snapshot")
            .expect("present"),
        &first,
    );

    // `/new` is driven the same observed way as a prompt (see
    // `submit_prompt` for the paste-swallow failure a blind sleep invites):
    // type it, wait for the slash-command popup to render, Enter, and then
    // require proof that the command ran: the notice older codex printed
    // when the OLD conversation closed (it names the id we already hold, so
    // matching on it cannot pass early), or the new chat's reset view that
    // codex 0.167 shows instead. An Enter that missed the popup would have
    // submitted "/new" as a chat message instead, and this wait is what
    // turns that from five silent minutes into an immediate readable screen
    // dump.
    // `submit_prompt` returns only once the first ANSWER has rendered, so
    // the turn is over and the composer accepts input again (codex
    // swallows composer keystrokes while a task runs — its own tip says to
    // Tab-queue instead). Even so, both halves of `/new` are driven by
    // what the screen shows rather than by sleeps, with retype/re-press on a
    // cadence: individual keystroke bursts were observed to vanish on a
    // loaded machine. The popup line proves the composer holds the
    // command; the closing notice or the reset view proves Enter executed
    // it (see the wait below).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    let news = |text: &str| text.matches("/new").count();
    let baseline = news(&screen(&sup, &session.id).await);
    loop {
        let text = screen(&sup, &session.id).await;
        if text.contains("start a new chat") {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the /new slash-command popup never rendered; screen:\n{text}"
        );
        // Retyped only when the screen shows NEITHER the popup nor the
        // command itself. Codex renders the popup a moment after the text
        // lands, so a loop that retyped on every miss would type into a
        // composer that already held `/new` and leave `/new/new` there —
        // which matches no command, opens no popup, and would burn this
        // deadline making the failure look like a vendor change. Counted
        // against a baseline taken before anything was typed, because
        // codex's own chrome may already name the command and a bare
        // `contains` would then never type it at all.
        if text.contains("Approaching rate limits")
            && text.contains("Switch to")
            && text.contains("esc back")
        {
            // This vendor prompt can cover the composer after the reply.
            // Dismiss it without changing the model or saving preferences;
            // the next screen observation must still prove `/new` is ready.
            client.send_keys(chan, b"\x1b".to_vec()).await;
        } else if news(&text) <= baseline {
            client.send_keys(chan, b"/new".to_vec()).await;
        }
        // sleep-ok: poll the command popup and pace guarded retyping between deadline checks.
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        client.send_keys(chan, b"\r".to_vec()).await;
        // sleep-ok: pace Enter retries before checking the old conversation's resume marker.
        tokio::time::sleep(Duration::from_secs(2)).await;
        let shown = screen(&sup, &session.id).await;
        let text = format!("{}\n{shown}", transcript.text());
        // Codex up to some version before 0.167 printed a closing notice
        // naming the old conversation; it may offer a titled picker entry
        // instead of a direct resume command, and the terminal can wrap its
        // UUID. Such a notice must name this exact old conversation.
        let closing_notice = text
            .split_once("To continue this session, run codex resume")
            .map(|(_, notice)| {
                notice
                    .lines()
                    .take_while(|line| !line.trim().is_empty())
                    .flat_map(|line| line.chars().filter(|c| !c.is_whitespace()))
                    .collect::<String>()
            });
        // Codex 0.167 prints no such notice (runs on 2026-11-11 found
        // none on the screen or in anything the stream sent): it resets its
        // view to a new chat. That reset is the proof `/new` ran rather
        // than a ready composer alone: the first answer is gone from the
        // screen and the new chat's header is there, while an Enter that
        // missed the popup would have sent "/new" as a message below that
        // answer. What `/new` did to the conversation is then pinned by the
        // identity below: kept until the next prompt, a new one after it.
        let view_reset = shown.contains("OpenAI Codex (v")
            && !shown.contains(first_answer)
            && !shown.contains("/new");
        if text.contains(&format!("codex resume {}", resume_identity(&first)))
            || closing_notice.is_some_and(|notice| notice.contains(&resume_identity(&first)))
            || view_reset
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "/new never closed the persistent conversation {}; screen:\n{shown}\n\
             --- the stream's newest text:\n{}",
            resume_identity(&first),
            transcript.tail(8 * 1024)
        );
    }

    // `/new` alone does not fire the hook — the next PROMPT does. Pinning
    // that lag is the point: it is what a user experiences (a `/new` whose
    // resume offer still names the old conversation until they type), and
    // it is the fact that makes the second prompt below necessary rather
    // than defensive.
    assert_identity_stays(&sup, &client, &session.id, Some(&first), 5).await;

    submit_prompt(&sup, &client, &session.id, chan, "pong").await;

    let renewed =
        wait_for_reported_identity(&sup, &client, state.path(), &session.id, Some(&first), 300)
            .await;
    assert_resume_names(
        &sup.session_snapshot(&session.id)
            .await
            .expect("snapshot")
            .expect("present"),
        &renewed,
    );

    let log = hook_log_lines(state.path(), &session.id);
    for conversation in [&first, &renewed] {
        let runtime = reported_runtime_identity(conversation);
        assert!(
            log.iter()
                .any(|line| line.contains(&runtime) && line.ends_with(" startup")),
            "codex reports `startup` both times, before and after /new: {log:?}"
        );
    }

    // Teardown before the permit, for the reason the claude test above
    // spells out.
    accepting.stop().await;
    drain.abort();
    drop(client);
    drop(sup);
    end_all_terminals(state.path()).await;
    drop(state);
    drop(work);
    drop(slot);
}

/// Current Codex must name an existing root transcript at Stop after `/clear`.
/// The probe reads the header inside the callback: observing it later cannot
/// prove that the event is usable for confirming a pending conversation. Two
/// actual completions also establish that ordinary clear changes the path and
/// leaves the old header intact. Default API authentication remains available;
/// only this private work directory receives an invocation-time trust override.
///
/// Codex runs as an undeclared command launch, so the supervisor injects no
/// hooks of its own and the probe's are the only ones: what is audited is
/// the vendor's callback, not Farhelm's reporting of it. The session still
/// gives Codex the terminal a shipped supervisor would, a terminal host.
#[farhelm_testtrace::test]
#[ignore = "needs real Codex credentials and network; run deliberately"]
async fn real_codex_stop_names_the_persisted_transcript_after_clear() {
    // The harness is declared last so that it drops first, on a failure
    // too: its state directory's teardown ends the terminal host (and
    // Codex in it) and only then releases its slot, before the probe's
    // files and Codex's folder are removed.
    let state = farhelm_teststate::tempdir().expect("private audit state");
    let work = farhelm_teststate::tempdir().expect("private vendor workdir");
    let h = harness().await;
    let log = state.path().join("events.jsonl");
    let probe = state.path().join("probe.py");
    std::fs::write(&probe, r#"import json, pathlib, sys
payload = json.load(sys.stdin)
path = pathlib.Path(payload['transcript_path'])
try:
    with path.open('rb') as record:
        header = record.readline(65536).decode('utf-8')
except FileNotFoundError:
    header = None
with open(sys.argv[1], 'a') as log:
    log.write(json.dumps({'event': payload['hook_event_name'], 'source': payload.get('source'), 'runtime': payload['session_id'], 'path': str(path), 'header': header}) + '\n')
"#).expect("callback probe");
    let hook = shell_words::join(["python3", probe.to_str().unwrap(), log.to_str().unwrap()]);
    let codex = which_binary("codex").expect("real Codex on PATH");
    let mut argv = vec![
        codex.to_string_lossy().into_owned(),
        "--dangerously-bypass-hook-trust".into(),
        "--dangerously-bypass-approvals-and-sandbox".into(),
        "-c".into(),
        CODEX_NO_UPDATE_CHECK.into(),
        "-c".into(),
        "features.hooks=true".into(),
        "-c".into(),
        format!(
            "projects.{}.trust_level=\"trusted\"",
            serde_json::to_string(work.path().to_str().unwrap()).unwrap()
        ),
    ];
    for event in ["SessionStart", "Stop"] {
        argv.extend([
            "-c".into(),
            format!(
                "hooks.{event}=[{{hooks=[{{type=\"command\",command={},timeout=60}}]}}]",
                serde_json::to_string(&hook).unwrap(),
            ),
        ]);
    }
    let command = shell_words::join(&argv);
    let session = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            farhelm_proto::SessionLaunch::plain_command(&command),
            None,
            100,
            30,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .unwrap_or_else(|e| panic!("launching the real codex: {e:#}"));
    let (chan, _replay, mut rx) = h
        .client
        .attach_live(&session.id, 100, 30)
        .await
        .expect("attach");
    // Drained so the attachment stays alive, as in the audits above.
    let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let mut deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        let text = screen(&h.sup, &session.id).await;
        if text.contains("OpenAI Codex (v") {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Codex never became ready: {text}"
        );
        // sleep-ok: poll the vendor's rendered startup marker.
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    for turn in 0..2 {
        if turn == 1 {
            h.client.send_keys(chan, b"/clear".to_vec()).await;
            deadline = tokio::time::Instant::now() + Duration::from_secs(60);
            loop {
                let text = screen(&h.sup, &session.id).await;
                if text.contains("/clear") {
                    break;
                }
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "clear not in composer: {text}"
                );
                // sleep-ok: wait for the literal slash command before submitting it.
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            // Enter goes separately from the text, as a human's does: text
            // and Enter in one write can read as a paste.
            h.client.send_keys(chan, b"\r".to_vec()).await;
        }
        let prompt = if turn == 0 {
            "Reply with exactly OK. Do not use tools."
        } else {
            "Reply with exactly PONG. Do not use tools."
        };
        h.client.send_keys(chan, prompt.as_bytes().to_vec()).await;
        deadline = tokio::time::Instant::now() + Duration::from_secs(300);
        loop {
            let text = screen(&h.sup, &session.id).await;
            if text.contains(prompt) {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "prompt not in composer: {text}"
            );
            // sleep-ok: wait for the vendor to render this turn's prompt.
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        h.client.send_keys(chan, b"\r".to_vec()).await;
        loop {
            let rows = codex_probe_events(&log);
            if rows.iter().filter(|r| r["event"] == "Stop").count() > turn {
                break;
            }
            let text = screen(&h.sup, &session.id).await;
            assert!(
                tokio::time::Instant::now() < deadline,
                "no completed Stop; events={rows:?}; screen={text}"
            );
            // sleep-ok: poll callback evidence while a real model turn completes.
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    let rows = codex_probe_events(&log);
    let stops: Vec<_> = rows.iter().filter(|r| r["event"] == "Stop").collect();
    assert_eq!(stops.len(), 2, "exactly two turns must complete: {rows:?}");
    assert_ne!(stops[0]["runtime"], stops[1]["runtime"]);
    assert_ne!(
        stops[0]["path"], stops[1]["path"],
        "ordinary clear must not rewrite the old path"
    );
    assert!(
        rows.iter().any(|r| r["event"] == "SessionStart"
            && r["source"] == "clear"
            && r["runtime"] == stops[1]["runtime"]),
        "the clear must select the runtime Stop confirms: {rows:?}"
    );
    for stop in stops {
        let header = stop["header"]
            .as_str()
            .expect("transcript exists during Stop");
        assert!(header.ends_with('\n'), "Stop must see a complete header");
        let meta: serde_json::Value = serde_json::from_str(header).unwrap();
        assert_eq!(meta["payload"]["session_id"], stop["runtime"]);
        assert_eq!(meta["payload"]["source"], "cli");
        let text = std::fs::read_to_string(stop["path"].as_str().unwrap()).unwrap();
        assert!(
            text.starts_with(header),
            "the exact callback header survives ordinary clear"
        );
    }
    drain.abort();
    end_all_terminals(h.state.path()).await;
    drop(h);
    drop(work);
    drop(state);
}

/// Read bounded private callback evidence without turning partial appends into
/// readiness. The vendor probe emits one compact JSON line per callback.
fn codex_probe_events(path: &std::path::Path) -> Vec<serde_json::Value> {
    use std::io::Read as _;
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let mut text = String::new();
    file.take(1024 * 1024)
        .read_to_string(&mut text)
        .expect("bounded callback log");
    text.split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .map(|line| serde_json::from_str(line).expect("complete callback JSON"))
        .collect()
}

/// Type one short, digit-free prompt and submit it, VERIFYING each half
/// against the rendered screen rather than trusting sleeps.
///
/// Every property here is learned rather than stylistic. Digit-free
/// because a numbered modal treats a stray digit as an option selection.
/// The text is confirmed visible in the composer before Enter is sent,
/// because codex reads text-plus-CR that arrives in one burst as a paste
/// and inserts the carriage return instead of submitting. And Enter is
/// re-sent while the prompt is still in the composer, because a fixed
/// sleep between the two halves was observed to lose the Enter anyway on
/// a loaded machine — the screen sat with the full prompt rendered and
/// unsubmitted for the test's whole deadline. Submission is a state the
/// screen can be asked about, so the test asks instead of guessing.
///
/// ## What the answer oracle is, and is not
///
/// Waiting for a `• <word>` bullet is a MANUAL-AUDIT CONVENIENCE, not a
/// general fixture, and it should not be borrowed for anything but these
/// `#[ignore]`d tests. It rests on three things nothing enforces: that the
/// model answers with the single word it was asked for, that codex renders
/// an answer with that bullet, and that the caller passes a `word` unique
/// within its own test — [`screen`] reads only the visible screen,
/// so an earlier identical answer that has scrolled off would make any
/// counting scheme (also tried) miss the new one. It is the right trade
/// only because the alternative heuristics are worse: the composer and the
/// submitted echo render the prompt with the same `›` decoration, and
/// warnings render below the composer, so every line-position rule tried
/// here misread some screen sooner or later.
async fn submit_prompt(
    sup: &Supervisor,
    client: &SupervisorClient,
    session_id: &str,
    chan: u32,
    word: &str,
) {
    let prompt = format!("Reply with the single word {word}.");

    client.send_keys(chan, prompt.as_bytes().to_vec()).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let text = screen(sup, session_id).await;
        if text.contains(&prompt) {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the composer never showed the typed prompt; rendered screen:\n{text}"
        );
        // sleep-ok: poll rendered prompt text between deadline checks before submitting it.
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    // Enter is re-pressed only while the prompt is still ON the screen. Once
    // it has left, the composer cannot still be holding it, and further
    // Enters are no longer re-pressing anything — they are keystrokes
    // arriving at whatever state the agent has moved into, which is not a
    // thing to keep doing for four minutes. The deadline is a model
    // turn's, not a keystroke's, so the wait continues either way.
    let marker = format!("• {word}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(240);
    loop {
        let text = screen(sup, session_id).await;
        if text.contains(&marker) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Enter never produced the {marker:?} answer; rendered screen:\n{text}"
        );
        if text.contains(&prompt) {
            client.send_keys(chan, b"\r".to_vec()).await;
        }
        // sleep-ok: pace conditional Enter retries and poll the answer marker in this manual audit.
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

/// A command launch declaring `harness` around the real agent's `command`,
/// with that agent's resume command, the way a user would type it.
///
/// Declaring the agent is what turns on Farhelm's hooks for a command
/// launch; an undeclared command gets none, so these tests would launch
/// the real agent and then wait for a report that is never sent.
fn real_agent_launch(
    command: &str,
    harness: farhelm_proto::LaunchHarness,
) -> farhelm_proto::SessionLaunch {
    let resume = match harness {
        farhelm_proto::LaunchHarness::Claude => {
            format!("{command} --resume {{conversation}} {{farhelm_args}}")
        }
        farhelm_proto::LaunchHarness::Codex => {
            format!("{command} resume {{conversation}} {{farhelm_args}}")
        }
        other => panic!("no real-agent capture test runs {other:?}"),
    };
    declared_command(
        &format!("{command} {{farhelm_args}}"),
        harness,
        Some(&resume),
    )
}

/// The real `codex` on `PATH` with `work` trusted for this invocation only,
/// as one shell-quoted command line for a declared command launch.
///
/// The trust is an invocation-time `-c` override, the same one
/// [`real_codex_stop_names_the_persisted_transcript_after_clear`] passes, so
/// Codex keeps this machine's own configuration and login (a model provider
/// configured in `config.toml` is a login as much as `auth.json` is) and
/// nothing is written to it. Its folder-trust modal is kept away rather than
/// answered: earlier runs of these audits found it would not take the
/// answer typed at it. [`CODEX_NO_UPDATE_CHECK`] is passed too.
fn trusted_codex(work: &std::path::Path) -> String {
    let codex = which_binary("codex").expect("real Codex on PATH");
    let trust = format!(
        "projects.{}.trust_level=\"trusted\"",
        serde_json::to_string(work.to_str().expect("UTF-8 workdir")).expect("quote the workdir")
    );
    shell_words::join([
        codex.to_str().expect("UTF-8 codex path"),
        "-c",
        trust.as_str(),
        "-c",
        CODEX_NO_UPDATE_CHECK,
    ])
}

/// The `-c` override that keeps Codex from checking for a newer release at
/// startup, for this invocation only.
///
/// A release published between two runs of an audit puts Codex's update
/// screen in front of the first prompt (it did, between two runs of these
/// audits on 2026-11-11: 0.167.0 found 0.167.2 and stopped there), and the
/// audit must neither update the vendor's agent nor answer for the user.
/// The check is the vendor's, not part of what is audited.
const CODEX_NO_UPDATE_CHECK: &str = "check_for_update_on_startup=false";

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

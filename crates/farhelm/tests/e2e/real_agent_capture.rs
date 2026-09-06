//! `#[ignore]`-marked tests against real vendor agents; they audit the
//! facts the fixture tests elsewhere cannot, since they need credentials
//! and network access CI does not have.
//!
//! Two sections, auditing the two identity sources. The CAPTURE tests pin
//! what the record scan reads off disk — that a record really appears at
//! first prompt submission, and that its correlator fields are named what
//! this build reads. The HOOK tests pin what the injected `SessionStart`
//! hook reports from inside the agent's own process, including the
//! mid-process `/clear` and `/new` the scan is structurally blind to. Each
//! section's own banner lists the facts it pins.

use crate::harness::*;
use crate::hook_identity::ServeTask;

/// Shortest a `capture-pane` is ever given to answer, whatever the caller's
/// remaining budget is.
///
/// [`pane_within`] bounds every tmux query by the polling loop's own
/// deadline, which is what stops a wedged tmux from hanging a test past it.
/// The floor keeps that from degenerating at the end of a budget, where a
/// zero-length timeout would report "tmux did not answer" for a query that
/// was never given a chance — and the pane that failure message wants to
/// print is exactly what the query would have returned.
const TMUX_ANSWER_FLOOR: Duration = Duration::from_secs(15);

/// The rendered pane, as one string, bounded by the caller's `deadline`.
///
/// Every polling loop in this file reads the pane rather than the terminal
/// stream (a TUI's first paint arrives as cursor-positioned fragments that
/// the raw transcript shows as bare line endings). `tmux_query` awaits a
/// child process with no timeout of its own, so a tmux server that stops
/// answering — a real failure mode on a machine loaded enough to be running
/// two vendor agents — would park a loop forever INSIDE an iteration, where
/// its own deadline check never runs. Bounding the query is what keeps
/// these audits failing with a diagnosis instead of hanging.
async fn pane_within(
    sock: &std::path::Path,
    tmux_name: &str,
    deadline: tokio::time::Instant,
) -> String {
    let budget = deadline
        .saturating_duration_since(tokio::time::Instant::now())
        .max(TMUX_ANSWER_FLOOR);
    let args = ["capture-pane", "-p", "-t", tmux_name];
    match tokio::time::timeout(budget, tmux_query(sock, &args)).await {
        Ok(pane) => String::from_utf8_lossy(&pane.stdout).into_owned(),
        Err(_) => panic!("tmux did not answer a capture-pane for {tmux_name} within {budget:?}"),
    }
}

// ---------------------------------------------------------------------
// Real-agent capture (PLAN_M3.md acceptance 8's second half)
//
// `#[ignore]`-marked, because they need vendor credentials and network
// access CI does not have and must never depend on. The fixture tests
// in `conversation_identity_capture` are what keep the LOGIC honest in
// CI; these are what keep the
// AUDITED FACTS honest — that the record really does appear at first
// prompt submission, that its correlator fields really are named what
// this build reads, and that the resume template really does fill into a
// command the vendor accepts. Nothing but a real agent can tell us that a
// version bump changed one of them, and the failure mode if one did is
// silent: capture would simply stop happening.
//
// Run them individually and deliberately:
//
//     cargo test -p farhelm --test e2e -- --ignored --test-threads 1 \
//         real_claude_session_captures_its_conversation_identity
//
// Record the run and its result with the milestone; a green fixture suite
// is not a substitute.
// ---------------------------------------------------------------------

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
///    caller does next: for the capture tests that next thing is the first
///    input byte, which anchors the capture window.
///
/// Matching is against the RENDERED pane, not the raw stream: a TUI's first
/// paint arrives as cursor-positioned fragments that the raw transcript
/// shows as bare line endings.
///
/// Accepted side effect: accepting trust writes the (soon-deleted) scratch
/// path into the user's real agent config. That is the vendor's own write,
/// and the same class of consequence these tests already embrace by
/// observing the real HOME.
async fn wait_for_agent_ready(
    client: &SupervisorClient,
    sock: &std::path::Path,
    session_id: &str,
    chan: u32,
    ready_marker: &str,
    trust_dialog_markers: &[&str],
) {
    let tmux_name = format!("fh-{session_id}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        let text = pane_within(sock, &tmux_name, deadline).await;
        // The deadline is checked FIRST, before either branch: a dialog
        // that never advances no matter how often it is answered is a real
        // failure mode (codex's does exactly that under tmux), and a
        // dialog branch that looped straight back to the top would press
        // enter at it forever instead of failing with the pane printed.
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {ready_marker:?}; rendered pane:\n{text}"
        );
        if trust_dialog_markers.iter().any(|m| text.contains(m)) {
            client.send_input(chan, b"\r".to_vec()).await;
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }
        if text.contains(ready_marker) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// The shared body of the two real-agent tests: launch `agent` for real in
/// a scratch directory, submit one prompt, and require the supervisor to
/// have captured a conversation identity that fills the resume template.
///
/// Observes the user's REAL home rather than a fixture tree — that is the
/// point, since the whole question is where the vendor actually writes and
/// what it writes there — and therefore uses the production capture window
/// and publication grace: a shortened one would make a slow first response
/// look like a missing record and turn a real regression into a flake, or
/// vice versa. The poll deadline is correspondingly generous, since nothing
/// may be committed until a full minute past first input.
///
/// The prompt is chosen to be answerable without tools and cheap to serve;
/// nothing asserts anything about the ANSWER, only that submitting one
/// caused a record this build can correlate.
///
/// Both agents were run for real on 2026-07-31 and both passed; the run
/// records, and codex's upstream trust-dialog limitation, are in
/// PLAN_M3.md's testing-decisions section.
async fn real_agent_captures_its_conversation(
    ready_marker: &str,
    trust_dialog_markers: &[&str],
    // Given the scratch working directory, produce the home the supervisor
    // should observe, the agent command to launch, and any tempdir that must
    // outlive the run. Claude observes the user's real home directly; codex
    // needs a synthesized one (see its test for why), and this seam is what
    // lets one helper serve both without either knowing the other's needs.
    prepare: impl FnOnce(
        &std::path::Path,
    ) -> (
        std::path::PathBuf,
        String,
        Option<farhelm_teststate::TestDir>,
    ),
) {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("tempdir");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (agent_home, agent, _agent_home_guard) = prepare(work.path());
    let agent = agent.as_str();
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        // Built directly rather than through `harness()`: this test needs
        // `agent_home` seamed in before the real vendor agent ever launches.
        // The suite's loaded-CI tmux floors still apply (this attaches for
        // real below), so `suite_timeouts()` rather than a bare `Default`.
        suite_timeouts(),
        SupervisorSeams {
            agent_home: Some(agent_home),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let client = connect_client(&sup).await;

    let session = client
        .create_session(&work.path().to_string_lossy(), agent, None, 100, 30)
        .await
        .unwrap_or_else(|e| panic!("launching the real {agent}: {e:#}"));

    let (chan, mut seen, mut rx) = client
        .attach_live(&session.id, 100, 30)
        .await
        .expect("attach");
    // See [`wait_for_agent_ready`] for the dialog handling and the two
    // orderings inside it. What matters HERE is that the capture window is
    // anchored on the FIRST input byte this session ever takes — the
    // trust dialog's Enter when one appeared, and otherwise the first byte
    // of the prompt text below, never the submitting Enter that follows
    // it. So the window may already be open before the prompt is typed,
    // and nothing slow may sit between accepting a dialog and prompting.
    wait_for_agent_ready(
        &client,
        &state.path().join("tmux.sock"),
        &session.id,
        chan,
        ready_marker,
        trust_dialog_markers,
    )
    .await;
    // Drain whatever the attach streamed so far; nothing below asserts on it.
    while let Ok(TermEvent::Data(bytes)) = rx.try_recv() {
        seen.extend_from_slice(&bytes);
    }
    let _ = &seen;

    client
        // Deliberately digit-free: a numbered modal (the trust dialogs
        // above offer "1."/"2.") treats a stray digit as an option
        // selection, so a prompt containing one could pick an answer
        // rather than be typed if a dialog ever races this send.
        .send_input(chan, b"Reply with the single word ok.".to_vec())
        .await;
    // The submitting Enter is a SEPARATE keystroke, as a human's is. Sent
    // in the same burst as the text, codex intermittently reads the whole
    // thing as a paste and inserts the carriage return into the composer
    // instead of submitting — observed live as the prompt sitting unsent
    // on the "›" line until the poll deadline, on roughly half of runs,
    // while claude submitted the same burst every time. Splitting it costs
    // nothing (the capture window is anchored on the first input byte and
    // is a minute wide) and removes the whole class of flake.
    tokio::time::sleep(Duration::from_secs(1)).await;
    client.send_input(chan, b"\r".to_vec()).await;

    // The record appears at first prompt SUBMISSION, so this poll is
    // waiting on the agent's own bookkeeping — and then on the production
    // window plus publication grace to elapse before anything may commit.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
    let conversation = loop {
        client.list_sessions().await.expect("list drives capture");
        let snapshot = sup
            .session_snapshot(&session.id)
            .await
            .expect("snapshot")
            .expect("present");
        if let Some(conversation) = snapshot.captured_conversation {
            break conversation;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the real {agent} never produced a record this build could correlate; \
             transcript so far:\n{}",
            String::from_utf8_lossy(&seen)
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    };

    let snapshot = sup
        .session_snapshot(&session.id)
        .await
        .expect("snapshot")
        .expect("present");
    assert_eq!(snapshot.restart_offer, farhelm_proto::RestartOffer::Resume);
    let resume = snapshot
        .resume_argv
        .expect("a Resume offer has a filled argv");
    assert!(
        resume.iter().any(|element| element == &conversation),
        "the captured identity must land in the resume argv: {resume:?}"
    );
    assert!(
        !resume.iter().any(|element| element == "{conversation}"),
        "no placeholder may survive substitution: {resume:?}"
    );
    drop(slot);
}

/// Claude Code for real. Requires a working `claude` on PATH, already
/// authenticated (`claude` run once interactively, or a vendor credential
/// in the environment the login shell sources) and able to reach the API.
/// The prompt costs one short completion.
#[tokio::test]
#[ignore = "needs real Claude Code credentials and network; run deliberately"]
async fn real_claude_session_captures_its_conversation_identity() {
    // No flags and the user's real home: the plain invocation is the one
    // users type, and the one basename derivation must recognize. The
    // marker is "Claude Code v" (only the banner carries the version), not
    // "Claude Code" — the trust dialog's own body says "Claude Code'll be
    // able to read...", and matching that broke this test's first real run.
    real_agent_captures_its_conversation(
        "Claude Code v",
        &["Accessing workspace", "Do you trust"],
        |_work| {
            let home = std::env::var_os("HOME").expect("a real-agent run needs a real HOME");
            (std::path::PathBuf::from(home), "claude".to_string(), None)
        },
    )
    .await;
}

/// Codex for real. Requires an authenticated `codex` on PATH (its
/// `auth.json` is copied into the synthetic home [`synthetic_codex_home`]
/// builds; this docstring is where that helper's reasoning lives, since
/// this is the test that discovered the need for it).
///
/// Unlike the claude test, this one runs codex against a SYNTHESIZED
/// `CODEX_HOME` rather than the user's real one, and that is not a
/// convenience — it is the only path that works. Codex v0.146.0's
/// folder-trust modal is input-dead under tmux: verified with strace, the
/// pane's `\r` reaches codex as a completed `read(0, "\r", 1024) = 1` and
/// is discarded, and the dialog never advances for ANY input tried (CR,
/// numeric option, arrows, kitty-protocol encodings, with and without a
/// rendering client attached). Codex's main TUI accepts input normally in
/// the same pane, so this is an upstream onboarding bug, not a farhelm
/// input-path problem — and it means a human sitting at the terminal is
/// equally stuck, so "have a person accept it" is not a fallback either.
///
/// The synthetic home sidesteps the modal the way codex itself intends:
/// trust is a recorded fact in its config, so a config that already trusts
/// the working directory means the modal never appears. Nothing here
/// configures the AGENT on the user's behalf in production terms — the
/// seam is `SupervisorSeams::agent_home`, which exists for exactly this,
/// and the user's real `~/.codex` is never written to. A `codex`-named
/// shim carries `CODEX_HOME` into the launch, which also keeps basename
/// derivation honest and makes the filled resume argv genuinely runnable.
///
/// No dialog markers are passed: with trust seeded the modal must not
/// appear, and pressing enter at a modal that ignores enter would only
/// burn the deadline two seconds at a time. If it ever does appear, the
/// wait fails with the rendered pane printed, which diagnoses itself.
#[tokio::test]
#[ignore = "needs real Codex credentials and network; run deliberately"]
async fn real_codex_session_captures_its_conversation_identity() {
    real_agent_captures_its_conversation("OpenAI Codex (v", &[], |work| {
        let (synth, agent) = synthetic_codex_home(work);
        (synth.path().to_path_buf(), agent, Some(synth))
    })
    .await;
}

// ---------------------------------------------------------------------
// Real-agent HOOK audit (plan §4.4)
//
// The two tests below are the tripwire for the vendor facts the injected
// `SessionStart` hook rests on. Every one of them was verified by hand
// once; none of them is guaranteed by anything but the vendors' goodwill,
// and if one changes the symptom is SILENT — identity capture quietly
// degrades to the record scan, which cannot see a mid-process `/clear` or
// `/new` at all.
//
// What they pin, fact by fact:
//
// - The payload field is named `session_id`, and its value is the exact
//   string a later resume needs. Asserted by the resume argv naming it.
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
// Run them individually and deliberately:
//
//     cargo test -p farhelm --test e2e -- --ignored --test-threads 1 \
//         real_claude_session_reports_its_identity_across_clear
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
/// Default seams apart from `agent_home`, and that is the point — hooks are
/// on by default, so this exercises the configuration a user gets.
async fn serving_supervisor(
    state: &std::path::Path,
    agent_home: std::path::PathBuf,
) -> (Arc<Supervisor>, Arc<SupervisorClient>, ServeTask) {
    let sup = Supervisor::new_with_seams(
        state,
        farhelm_bin().into(),
        suite_timeouts(),
        SupervisorSeams {
            agent_home: Some(agent_home),
            ..SupervisorSeams::default()
        },
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
/// Both agents here also write records the scan can read, and the scan
/// writes to the very same column — so without that second condition a
/// vendor that stopped firing `SessionStart` entirely would still satisfy
/// every assertion in these tests, which exist for no other purpose than to
/// notice that. An `acked` line names the id the supervisor answered for,
/// and only a hook process can have put it there.
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
            && hook_acked(state, session_id, &conversation)
        {
            return conversation;
        }
        // On timeout, dump everything a human would go looking for by
        // hand: the rendered pane (did the prompt submit? is a dialog up?
        // did the turn error?) and the hook's own trace (did it run, and
        // what did the supervisor answer?). A bare "no identity" message
        // costs whoever runs this audit a full re-run under a probe.
        if tokio::time::Instant::now() >= deadline {
            let pane = tmux_query(
                &state.join("tmux.sock"),
                &["capture-pane", "-p", "-t", &format!("fh-{session_id}")],
            )
            .await;
            let hook_log = state.join("hook-log").join(format!("{session_id}.log"));
            panic!(
                "no reported identity{} within {secs}s\n--- hook log ({}):\n{}\n--- pane:\n{}",
                match previous {
                    Some(was) => format!(" other than {was}"),
                    None => String::new(),
                },
                hook_log.display(),
                std::fs::read_to_string(&hook_log).unwrap_or_else(|e| format!("<unreadable: {e}>")),
                String::from_utf8_lossy(&pane.stdout),
            );
        }
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

/// Whether the hook has recorded a supervisor-ACCEPTED report of exactly
/// `conversation` for this session.
///
/// Tolerates a log file that does not exist yet, because it is called while
/// polling: before any hook has run there is nothing to read, and that is
/// an ordinary "not yet" rather than a failure. The line shape is
/// `<unix-seconds> acked <conversation> <source>` (`crate::hook`'s module
/// docs), and `acked` carries no detail, so the id is always the third
/// field.
fn hook_acked(state: &std::path::Path, session_id: &str, conversation: &str) -> bool {
    let path = state.join("hook-log").join(format!("{session_id}.log"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        return false;
    };
    text.lines().any(|line| {
        let mut fields = line.split_whitespace().skip(1);
        fields.next() == Some("acked") && fields.next() == Some(conversation)
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
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Assert the resume the supervisor would run names `conversation`.
fn assert_resume_names(snapshot: &SessionSnapshot, conversation: &str) {
    assert_eq!(snapshot.restart_offer, farhelm_proto::RestartOffer::Resume);
    let resume = snapshot
        .resume_argv
        .as_ref()
        .expect("a Resume offer has a filled argv");
    assert!(
        resume.iter().any(|element| element == conversation),
        "the reported identity must land in the resume argv: {resume:?}"
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
#[tokio::test]
#[ignore = "needs real Claude Code credentials; run deliberately"]
async fn real_claude_session_reports_its_identity_across_clear() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("state dir");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let home = std::path::PathBuf::from(
        std::env::var_os("HOME").expect("a real-agent run needs a real HOME"),
    );
    let (sup, client, accepting) = serving_supervisor(state.path(), home).await;
    let tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let sock = state.path().join("tmux.sock");

    let session = client
        .create_session(&work.path().to_string_lossy(), "claude", None, 100, 30)
        .await
        .unwrap_or_else(|e| panic!("launching the real claude: {e:#}"));
    let tmux_name = format!("fh-{}", session.id);
    let (chan, _replay, mut rx) = client
        .attach_live(&session.id, 100, 30)
        .await
        .expect("attach");
    // Drained for the whole run, not dropped: the terminal path is
    // flow-controlled with an overflow detach, so an unread receiver
    // eventually detaches this viewer — after which every `send_input`
    // lands on a dead channel and the agent looks like it is swallowing
    // keystrokes. These tests read the PANE for their assertions, so the
    // stream's only job is to keep the attachment alive.
    let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
    wait_for_agent_ready(
        &client,
        &state.path().join("tmux.sock"),
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

    // `/clear` is driven by what the pane shows, not by sleeps, in both
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
    let baseline = clears(&pane_within(&sock, &tmux_name, deadline).await);
    // The command and its Enter are separate keystrokes, as a human's are.
    // Retyping is guarded on the composer being genuinely empty of it, or
    // a slow render would leave `/clear/clear` in the box.
    loop {
        let text = pane_within(&sock, &tmux_name, deadline).await;
        if clears(&text) > baseline {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the composer never showed the typed /clear; rendered pane:\n{text}"
        );
        client.send_input(chan, b"/clear".to_vec()).await;
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
        client.send_input(chan, b"\r".to_vec()).await;
        for _ in 0..10 {
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
            "Enter never executed /clear (identity still {first}); rendered pane:\n{}",
            pane_within(&sock, &tmux_name, deadline).await
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
    // supervisors, tmux servers and vendor agents run at once, so handing
    // the permit back while this test's own are still being torn down
    // admits the next harness into a machine that is still carrying this
    // one — which is exactly the load the semaphore exists to prevent.
    accepting.stop().await;
    drain.abort();
    drop(client);
    drop(sup);
    drop(tmux);
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
/// The synthetic `CODEX_HOME` is not a convenience; see
/// [`real_codex_session_captures_its_conversation_identity`] for why the
/// folder-trust modal makes it the only path that works, and for the shim
/// that carries the variable into the launch while keeping basename
/// derivation honest.
#[tokio::test]
#[ignore = "needs real Codex credentials and network; run deliberately"]
async fn real_codex_session_reports_its_identity_across_new() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("state dir");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (synth, agent) = synthetic_codex_home(work.path());
    let (sup, client, accepting) =
        serving_supervisor(state.path(), synth.path().to_path_buf()).await;
    let tmux = TmuxServerGuard(state.path().join("tmux.sock"));

    let session = client
        .create_session(&work.path().to_string_lossy(), &agent, None, 100, 30)
        .await
        .unwrap_or_else(|e| panic!("launching the real codex: {e:#}"));
    let (chan, _replay, mut rx) = client
        .attach_live(&session.id, 100, 30)
        .await
        .expect("attach");
    // See the claude test above: the stream must stay drained or the
    // attachment overflow-detaches and later input silently stops landing
    // — with far more rendered output here (codex repaints heavily), this
    // test hit exactly that as minutes-in keystroke loss.
    let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
    // No dialog markers: trust is seeded in the synthetic config, so the
    // modal must not appear at all.
    wait_for_agent_ready(
        &client,
        &state.path().join("tmux.sock"),
        &session.id,
        chan,
        "OpenAI Codex (v",
        &[],
    )
    .await;

    // Codex fires its hook at first prompt SUBMISSION, so before one goes
    // out there must be no identity at all. Without this the test would
    // pass just as well against a codex that reported at startup — a
    // different vendor fact, and one this file's banner claims is false.
    assert_identity_stays(&sup, &client, &session.id, None, 5).await;

    // Digit-free, for the same reason the capture test's prompt is.
    submit_prompt(
        &client,
        &state.path().join("tmux.sock"),
        &session.id,
        chan,
        "ok",
    )
    .await;
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
    // require the line codex prints when the OLD conversation is closed —
    // which names the id we already hold, so matching on it cannot pass
    // early. An Enter that missed the popup would have submitted "/new" as
    // a chat message instead, and this wait is what turns that from five
    // silent minutes into an immediate readable pane dump.
    let sock = state.path().join("tmux.sock");
    let tmux_name = format!("fh-{}", session.id);
    // `submit_prompt` returns only once the first ANSWER has rendered, so
    // the turn is over and the composer accepts input again (codex
    // swallows composer keystrokes while a task runs — its own tip says to
    // Tab-queue instead). Even so, both halves of `/new` are driven by
    // what the pane shows rather than by sleeps, with retype/re-press on a
    // cadence: individual keystroke bursts were observed to vanish on a
    // loaded machine. The popup line proves the composer holds the
    // command; the `codex resume <old id>` line codex prints when it
    // closes a conversation proves Enter executed it — and it names the id
    // this test already holds, so the wait cannot pass early.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    let news = |text: &str| text.matches("/new").count();
    let baseline = news(&pane_within(&sock, &tmux_name, deadline).await);
    loop {
        let text = pane_within(&sock, &tmux_name, deadline).await;
        if text.contains("start a new chat") {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the /new slash-command popup never rendered; pane:\n{text}"
        );
        // Retyped only when the pane shows NEITHER the popup nor the
        // command itself. Codex renders the popup a moment after the text
        // lands, so a loop that retyped on every miss would type into a
        // composer that already held `/new` and leave `/new/new` there —
        // which matches no command, opens no popup, and would burn this
        // deadline making the failure look like a vendor change. Counted
        // against a baseline taken before anything was typed, because
        // codex's own chrome may already name the command and a bare
        // `contains` would then never type it at all.
        if news(&text) <= baseline {
            client.send_input(chan, b"/new".to_vec()).await;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        client.send_input(chan, b"\r".to_vec()).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        let text = pane_within(&sock, &tmux_name, deadline).await;
        if text.contains(&format!("codex resume {first}")) {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "/new never closed conversation {first}; pane:\n{text}"
        );
    }

    // `/new` alone does not fire the hook — the next PROMPT does. Pinning
    // that lag is the point: it is what a user experiences (a `/new` whose
    // resume offer still names the old conversation until they type), and
    // it is the fact that makes the second prompt below necessary rather
    // than defensive.
    assert_identity_stays(&sup, &client, &session.id, Some(&first), 5).await;

    submit_prompt(&client, &sock, &session.id, chan, "pong").await;

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
        assert!(
            log.iter()
                .any(|line| line.contains(conversation) && line.ends_with(" startup")),
            "codex reports `startup` both times, before and after /new: {log:?}"
        );
    }

    // Teardown before the permit, for the reason the claude test above
    // spells out.
    accepting.stop().await;
    drain.abort();
    drop(client);
    drop(sup);
    drop(tmux);
    drop(slot);
}

/// Type one short, digit-free prompt and submit it, VERIFYING each half
/// against the rendered pane rather than trusting sleeps.
///
/// Every property here is learned rather than stylistic. Digit-free
/// because a numbered modal treats a stray digit as an option selection.
/// The text is confirmed visible in the composer before Enter is sent,
/// because codex reads text-plus-CR that arrives in one burst as a paste
/// and inserts the carriage return instead of submitting. And Enter is
/// re-sent while the prompt is still in the composer, because a fixed
/// sleep between the two halves was observed to lose the Enter anyway on
/// a loaded machine — the pane sat with the full prompt rendered and
/// unsubmitted for the test's whole deadline. Submission is a state the
/// pane can be asked about, so the test asks instead of guessing.
///
/// ## What the answer oracle is, and is not
///
/// Waiting for a `• <word>` bullet is a MANUAL-AUDIT CONVENIENCE, not a
/// general fixture, and it should not be borrowed for anything but these
/// `#[ignore]`d tests. It rests on three things nothing enforces: that the
/// model answers with the single word it was asked for, that codex renders
/// an answer with that bullet, and that the caller passes a `word` unique
/// within its own test — `capture-pane` renders only the visible screen,
/// so an earlier identical answer that has scrolled off would make any
/// counting scheme (also tried) miss the new one. It is the right trade
/// only because the alternative heuristics are worse: the composer and the
/// submitted echo render the prompt with the same `›` decoration, and
/// warnings render below the composer, so every line-position rule tried
/// here misread some pane sooner or later.
async fn submit_prompt(
    client: &SupervisorClient,
    sock: &std::path::Path,
    session_id: &str,
    chan: u32,
    word: &str,
) {
    let prompt = format!("Reply with the single word {word}.");
    let tmux_name = format!("fh-{session_id}");

    client.send_input(chan, prompt.as_bytes().to_vec()).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let text = pane_within(sock, &tmux_name, deadline).await;
        if text.contains(&prompt) {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the composer never showed the typed prompt; rendered pane:\n{text}"
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    // Enter is re-pressed only while the prompt is still ON the pane. Once
    // it has left, the composer cannot still be holding it, and further
    // Enters are no longer re-pressing anything — they are keystrokes
    // arriving at whatever state the agent has moved into, which is not a
    // thing to keep doing for four minutes. The deadline is a model
    // turn's, not a keystroke's, so the wait continues either way.
    let marker = format!("• {word}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(240);
    loop {
        let text = pane_within(sock, &tmux_name, deadline).await;
        if text.contains(&marker) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Enter never produced the {marker:?} answer; rendered pane:\n{text}"
        );
        if text.contains(&prompt) {
            client.send_input(chan, b"\r".to_vec()).await;
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

/// A private `CODEX_HOME` with the working directory already trusted, plus
/// a `codex`-named shim that carries the variable into the launch.
///
/// Extracted so both real-codex tests share one description of a setup that
/// is entirely about working around codex's input-dead folder-trust modal
/// (see [`real_codex_session_captures_its_conversation_identity`]).
/// Returns the tempdir — which must outlive the run — and the invocation.
fn synthetic_codex_home(work: &std::path::Path) -> (farhelm_teststate::TestDir, String) {
    let real_home = std::env::var_os("HOME").expect("a real-agent run needs a real HOME");
    let real_auth = std::path::Path::new(&real_home).join(".codex/auth.json");
    let auth = std::fs::read(&real_auth).unwrap_or_else(|e| {
        panic!(
            "this test needs an authenticated codex ({}): {e}",
            real_auth.display()
        )
    });

    let synth = farhelm_teststate::tempdir().expect("synthetic codex home");
    let codex_home = synth.path().join(".codex");
    std::fs::create_dir_all(&codex_home).expect("codex home");
    let auth_path = codex_home.join("auth.json");
    std::fs::write(&auth_path, auth).expect("auth.json");
    std::fs::set_permissions(
        &auth_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )
    .expect("auth.json mode");
    // The trust key is the exact path the session is created with.
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            "[projects.\"{}\"]\ntrust_level = \"trusted\"\n",
            work.display()
        ),
    )
    .expect("config.toml");

    let real_codex = which_binary("codex").expect("codex on PATH");
    let bin = synth.path().join("bin");
    std::fs::create_dir_all(&bin).expect("shim dir");
    let shim = bin.join("codex");
    std::fs::write(
        &shim,
        format!(
            "#!/bin/sh\nexec env CODEX_HOME={} {} \"$@\"\n",
            shell_quote(&codex_home.to_string_lossy()),
            shell_quote(&real_codex.to_string_lossy()),
        ),
    )
    .expect("shim");
    std::fs::set_permissions(
        &shim,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .expect("shim mode");

    let agent = shim.to_string_lossy().into_owned();
    (synth, agent)
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

/// Single-quote `s` for `/bin/sh`, closing and reopening around any quote.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

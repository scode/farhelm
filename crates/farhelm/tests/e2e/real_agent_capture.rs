//! `#[ignore]`-marked capture tests against real vendor agents; they audit
//! the facts the fixture tests elsewhere cannot, since they need
//! credentials and network access CI does not have.

use crate::harness::*;

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
    prepare: impl FnOnce(&std::path::Path) -> (std::path::PathBuf, String, Option<tempfile::TempDir>),
) {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let work = tempfile::tempdir().expect("workdir");
    let (agent_home, agent, _agent_home_guard) = prepare(work.path());
    let agent = agent.as_str();
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
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

    let (chan, mut rx) = client.attach(&session.id, 100, 30).await.expect("attach");
    let mut seen = Vec::new();
    // A fresh scratch directory is an UNTRUSTED workspace, and a modern
    // agent blocks on its own folder-trust dialog before it will accept a
    // prompt. Accepting it here is not a workaround: farhelm passes the
    // vendor's terminal through untouched and never configures an agent
    // (SPEC.md), so a real user meets this same dialog and presses enter.
    // The test simulates only that human half.
    //
    // Two orderings are load-bearing, both learned by running this for
    // real against Claude Code v2.1.220:
    //
    // 1. Dialog markers are checked BEFORE the ready marker, and a ready
    //    marker must never be a substring of any dialog text. The first
    //    real run matched "Claude Code" against the dialog's own body
    //    ("Claude Code'll be able to read, edit, and execute files here"),
    //    broke the wait, and typed the prompt into an unaccepted modal —
    //    no conversation was ever started and capture correctly found
    //    nothing. Hence "Claude Code v", which only the banner carries.
    // 2. Nothing slow may sit between accepting the dialog and submitting
    //    the prompt: that enter IS the session's first input byte, so it
    //    anchors the capture window. The slack for a human composing
    //    afterwards is exactly what `CAPTURE_WINDOW_AFTER` is sized for
    //    (see its docs, which name this dialog).
    //
    // Matching is against the RENDERED pane, not the raw stream: a TUI's
    // first paint arrives as cursor-positioned fragments that the raw
    // transcript shows as bare line endings.
    //
    // Accepted side effect: accepting trust writes the (soon-deleted)
    // scratch path into the user's real agent config. That is the vendor's
    // own write, and the same class of consequence this test already
    // embraces by observing the real HOME.
    let sock = state.path().join("tmux.sock");
    let tmux_name = format!("fh-{}", session.id);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        let pane = tmux_query(&sock, &["capture-pane", "-p", "-t", &tmux_name]).await;
        let text = String::from_utf8_lossy(&pane.stdout).to_string();
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
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
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
/// `auth.json` is copied into the synthetic home below).
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
        let real_home = std::env::var_os("HOME").expect("a real-agent run needs a real HOME");
        let real_auth = std::path::Path::new(&real_home).join(".codex/auth.json");
        let auth = std::fs::read(&real_auth).unwrap_or_else(|e| {
            panic!(
                "this test needs an authenticated codex ({}): {e}",
                real_auth.display()
            )
        });

        let synth = tempfile::tempdir().expect("synthetic codex home");
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
        (synth.path().to_path_buf(), agent, Some(synth))
    })
    .await;
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

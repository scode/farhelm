//! Wrapper profiles: the `{cwd}` placeholder, substituted at launch, end
//! to end against a real supervisor.
//!
//! A "wrapper" here is a launcher of the shape `wrapper run <dir> <agent
//! command>`: it takes the working directory as an ARGUMENT, `cd`s into
//! it, runs the agent as a child WITHOUT `exec`ing, and stays resident as
//! the agent's parent for the agent's whole life (the real ones hold a
//! kernel `flock` on the directory while they are up). Farhelm could
//! already launch such a thing, but only with the directory baked into
//! the invocation string — one profile per directory, and a profile whose
//! baked directory disagreed with the session's own cwd silently lost the
//! RECORD-SCAN capture path, because the agent's records then report the
//! wrapper's directory while that scan correlates on the session's. Not
//! every wrapper session loses its identity that way: one whose kind is
//! declared and whose wrapper forwards the injected hook flags is told its
//! conversation id outright, and the hook correlates on nothing. The loss
//! is silent precisely because it only bites the sessions that fall back
//! to the scan — which is the shape these tests run in, since the
//! `claude-record` fixture writes records and never reports through a
//! hook. `{cwd}` is the whole-element placeholder that fixes it, and
//! `Supervisor::spawn_agent` is the one place it is substituted.
//!
//! `sh -c` stands in for the real wrapper, and it is the closest honest
//! stand-in available: it takes the directory as a positional argument,
//! `cd`s into it, runs the agent as a genuine child, and stays resident.
//! What it cannot stand in for is a real wrapper's own argument parsing —
//! whether it stops at the agent command, whether it forwards unknown
//! trailing flags — which is exactly the part these tests are not able to
//! pin and the docs say so.
//!
//! ## Where each property is observed, and why
//!
//! The substituted value lands in the WRAPPER's argv, not the agent's:
//! the wrapper consumes it (`cd "$1" && shift`) and the agent never sees
//! it. So the "the session's directory reached the `{cwd}` slot"
//! assertions read the wrapper process's own `/proc/<pid>/cmdline`
//! ([`wrapper_slot_argv`]) rather than the fixture's `FAKE-AGENT ARGV:`
//! marker, which can only witness what the AGENT was handed. Both are
//! asserted, because they are different claims: the marker proves the
//! agent's own argv came through the wrapper intact (hook injection
//! included), and the wrapper's argv proves the placeholder was filled
//! with this session's directory rather than left literal or filled with
//! something else.
//!
//! That the agent ran at all is itself evidence: an unfilled `{cwd}`
//! makes the wrapper's `cd` fail, `&&` short-circuits, and no agent is
//! ever execed — so every one of these tests would time out waiting for a
//! marker rather than fail on a comparison.
//!
//! ## Which directory, exactly
//!
//! Not always the same string. The value substituted is whatever the
//! launch hands TMUX as the pane's working directory: the user's own
//! spelling at create, and the verified resolution of it on a restart
//! (plan D1). Only
//! [`a_wrapper_gets_the_literal_spelling_at_create_and_the_verified_path_on_restart`]
//! makes that difference observable, by creating through a symlink; every
//! other test here runs in a directory where the two spellings coincide
//! and says nothing about which rule is in force.
//!
//! ## Which restart arm each test covers
//!
//! `relaunch_argv` picks a different vector per mode, and all three reach
//! the same fill in `spawn_agent`, so each arm needs its own test:
//! `Resume` in [`a_wrapper_session_resumes_through_the_wrapper`], `Fresh`
//! in [`a_wrapper_session_fresh_restarts_into_the_same_directory`], and
//! `FallbackTemplate` in
//! [`a_generic_wrapper_with_a_template_falls_back_through_the_wrapper`].

use crate::conversation_identity_capture::{
    CaptureFixtures, capture_harness, marker_value, provoke_record, settle_past_horizon,
    snapshot_of, wait_for_capture,
};
use crate::harness::*;

// ---------------------------------------------------------------------
// Wrapper fixtures (farhelm-wrappers-plan.md §4.2)
// ---------------------------------------------------------------------

/// The wrapper's shell program: `cd` into the directory it was handed,
/// drop it, and run everything after it as a child.
///
/// The trailing `; exit $?` is load-bearing, not tidiness. When `/bin/sh`
/// is bash, the LAST command of a `-c` script is tail-`exec`ed as an
/// optimization, so `cd "$1" && shift && "$@"` alone would REPLACE the
/// shell with the agent and there would be no resident wrapper left to
/// test — bash then reports the agent's parent as the grandparent, while
/// dash forks and keeps `sh`. With a command after it, both shells stay
/// resident as the agent's parent, which is what the wrappers this
/// feature exists for actually do.
///
/// `$1` is where `{cwd}` lands and `wrapper` is `$0` for the inner shell:
/// the "positional slot" pattern `ensure_executable_argv`'s docs describe,
/// which is what keeps the substituted directory out of the script TEXT.
/// A directory spliced into the script would be re-parsed by the shell,
/// and this feature's whole rule is that it never is.
const WRAPPER_SCRIPT: &str = r#"cd "$1" && shift && "$@"; exit $?"#;

/// A flag that appears in the fallback resume template and NOWHERE else,
/// so a relaunch through that template is distinguishable from a replay of
/// the create-time launch.
///
/// The fixture never interprets it: it lands in clap's trailing `extra`
/// catch-all, which accepts anything and echoes it back under
/// [`ARGV_MARKER`]. That is the entire job — the flag exists to be seen in
/// a transcript, not to do something.
const FALLBACK_FLAG: &str = "--fallback-resume-marker";

/// The argv slot `{cwd}` occupies in every wrapper vector this file
/// builds: `sh`, `-c`, the script, `$0`, then `$1`.
///
/// A named constant rather than a bare `4` at the one assertion that uses
/// it, because the number is a property of [`wrapper_argv`]'s shape and
/// belongs next to it. What it buys is a single place to update if that
/// vector ever grows an element: the assertion checks that the filled
/// directory landed in THIS slot rather than merely somewhere in the
/// vector, so a stale constant fails every wrapper test at once, with the
/// whole argv in the message.
const CWD_SLOT: usize = 4;

/// The outer argv of a wrapper launch: `sh -c <script> wrapper {cwd}`
/// followed by the agent command the wrapper is to run.
///
/// Built element by element and returned as a VECTOR, never by splitting
/// a string: that is the shape a resume template travels in, and it makes
/// the whole-element rule hold by construction rather than by luck.
/// Writing the command out as one string and splitting it would agree with
/// this today, but only for as long as none of the paths spliced into it
/// (the fixture binary, the record home) grows a space or a quote for the
/// split to have to undo correctly.
fn wrapper_argv(agent: &std::path::Path, agent_tail: &[&str]) -> Vec<String> {
    let mut argv = vec![
        "sh".to_string(),
        "-c".to_string(),
        WRAPPER_SCRIPT.to_string(),
        "wrapper".to_string(),
        farhelm_supervisor::agent_kind::CWD_PLACEHOLDER.to_string(),
        agent.to_string_lossy().into_owned(),
    ];
    argv.extend(agent_tail.iter().map(|word| (*word).to_string()));
    argv
}

/// [`wrapper_argv`] as the invocation STRING a create carries.
///
/// Quoted with `shell_words` per element rather than formatted by hand:
/// the supervisor splits the invocation back apart with the same crate, so
/// every element arrives as itself — the script text with its spaces and
/// embedded quotes, the fixture paths, and the placeholder with its braces
/// (which may come back quoted; the split strips the quotes and the
/// whole-element rule sees `{cwd}` again).
///
/// What does NOT travel in this string is the session's directory. The
/// invocation carries the literal placeholder and nothing else — the
/// directory is substituted into the argv SLOT at spawn, long after this
/// split — which is exactly why the tests can run in a work directory
/// whose path contains a space without that path ever meeting a quoting
/// rule.
fn wrapper_invocation(agent: &std::path::Path, agent_tail: &[&str]) -> String {
    shell_words::join(wrapper_argv(agent, agent_tail))
}

/// The record-writing fixture's own flags, as the wrapper's trailing
/// agent command.
///
/// Mirrors `record_session`'s invocation exactly (`--script
/// claude-record --record-home <home>`); a divergence here would make the
/// fixture write records somewhere the harness's supervisor is not
/// watching, and the failure would look like capture being broken.
fn record_tail(fixtures: &CaptureFixtures) -> Vec<String> {
    vec![
        "internal".to_string(),
        "fake-agent".to_string(),
        "--script".to_string(),
        "claude-record".to_string(),
        "--record-home".to_string(),
        fixtures.home().to_string_lossy().into_owned(),
    ]
}

/// The wrapper invocation and its matching resume template, both built
/// around the `claude` symlink in `fixtures.bin()`.
///
/// Returned together because the two have to agree: the template's agent
/// argv is the invocation's agent argv with `--resume {conversation}`
/// appended, and both name the same fixture binary. The symlink itself is
/// not returned — no caller needs it, and deriving it in one place is what
/// keeps the pair in step. The `claude-record` script does not interpret
/// `--resume` — the fake agent's trailing `extra` catch-all merely accepts
/// and echoes it — so every resume assertion in this file reads the
/// `FAKE-AGENT ARGV:` marker and nothing else.
fn wrapper_profile(fixtures: &CaptureFixtures) -> (String, Vec<String>) {
    let agent = fixtures.bin().join("claude");
    let tail = record_tail(fixtures);
    let tail_refs: Vec<&str> = tail.iter().map(String::as_str).collect();
    let invocation = wrapper_invocation(&agent, &tail_refs);

    let mut resume_tail = tail_refs.clone();
    resume_tail.push("--resume");
    resume_tail.push(farhelm_supervisor::agent_kind::CONVERSATION_PLACEHOLDER);
    let template = wrapper_argv(&agent, &resume_tail);

    (invocation, template)
}

/// A directory whose path contains a space, under `parent`.
///
/// The whole point of substituting into an argv SLOT rather than into a
/// command string is that a path like this survives as one element. A
/// tempdir alone would never exercise it — nothing in the suite's /tmp
/// scheme produces a space — so the case has to be built deliberately.
fn dir_with_a_space(parent: &std::path::Path) -> std::path::PathBuf {
    let path = parent.join("with space");
    std::fs::create_dir(&path).expect("create a working directory whose path contains a space");
    path
}

// ---------------------------------------------------------------------
// Observing the launch
// ---------------------------------------------------------------------

/// One process's argv, read from `/proc/<pid>/cmdline`, with element
/// positions preserved exactly.
///
/// The file is NUL-SEPARATED and NUL-TERMINATED, so exactly one trailing
/// NUL is stripped before splitting. Empty elements in the middle are kept
/// deliberately: an argv element may legitimately be the empty string, and
/// discarding those would slide every later element down a slot and quietly
/// aim [`CWD_SLOT`] at the wrong one.
///
/// Returns an empty vector both for a process that has already exited and
/// for one with no readable argv at all (a kernel thread, a zombie), which
/// every caller treats the same way: not the wrapper.
fn cmdline_of(pid: u32) -> Vec<String> {
    let Ok(raw) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
        return Vec::new();
    };
    let raw = raw.strip_suffix(&[0]).unwrap_or(&raw);
    if raw.is_empty() {
        return Vec::new();
    }
    raw.split(|&b| b == 0)
        .map(|element| String::from_utf8_lossy(element).into_owned())
        .collect()
}

/// The live wrapper process's own argv for this session.
///
/// Found through the session's environment marker (the same
/// `FARHELM_SESSION_ID` scan the supervisor's kill sweep uses) and then
/// narrowed by the script text, which only the wrapper carries.
///
/// The narrowing is needed because the marker selects a whole subtree.
/// The shim sets it on the command it `exec`s and on nothing else
/// (`agent_command` in `farhelm-supervisor/src/launch.rs`) — but for a
/// wrapper launch that command IS the wrapper, and the environment is
/// inherited across fork and exec, so the agent the wrapper runs and
/// anything the agent spawns wear the marker too. The login shell is not
/// a third candidate: its `-c` script is `exec farhelm internal launch
/// ...`, so by the time any of this is observable it has already been
/// replaced, same pid, by the shim and then by the wrapper.
///
/// This is the ONLY place the substituted directory is observable. The
/// wrapper consumes `$1` and shifts it away, so the agent's own argv —
/// the `FAKE-AGENT ARGV:` marker — cannot witness it, and reading the
/// wrapper's process argv is what turns "the agent ended up in the right
/// directory" into "the right value was substituted into the right slot".
fn wrapper_slot_argv(session_id: &str) -> Vec<String> {
    let mut found: Vec<Vec<String>> = marked_pids(session_id)
        .into_iter()
        .map(cmdline_of)
        .filter(|argv| argv.iter().any(|element| element == WRAPPER_SCRIPT))
        .collect();
    assert_eq!(
        found.len(),
        1,
        "expected exactly one live wrapper process for session {session_id}, found {found:?}"
    );
    found.pop().expect("just asserted there is one")
}

/// Assert the wrapper for `session_id` was handed `cwd` in its `{cwd}`
/// slot, and that no literal placeholder survived anywhere in its argv.
///
/// Only the slot check can fail today, and the scan is kept anyway. Every
/// vector this file builds carries exactly ONE `{cwd}`, so a slot holding
/// the session's directory leaves the scan nothing to find. It is there
/// for the vector this file does not build yet — a wrapper taking the
/// directory in two places, say — where a fill that stopped after the
/// first element would still satisfy the slot check. It also names the
/// failure better on the day it does fire: "still literal" and "wrong
/// directory" are different bugs.
fn assert_wrapper_got(session_id: &str, cwd: &std::path::Path) {
    let argv = wrapper_slot_argv(session_id);
    assert!(
        !argv
            .iter()
            .any(|element| element == farhelm_supervisor::agent_kind::CWD_PLACEHOLDER),
        "the launched wrapper still carries an unfilled placeholder: {argv:?}"
    );
    assert_eq!(
        argv.get(CWD_SLOT).map(String::as_str),
        Some(cwd.to_string_lossy().as_ref()),
        "the session's directory must be substituted into the wrapper's own slot: {argv:?}"
    );
}

/// Keep reading until the LAST `FAKE-AGENT ARGV:` line in `seen` is
/// complete — that is, until the fixture's `FAKE-AGENT READY` has arrived
/// after it.
///
/// Every anchor the tests wait for (`--resume `, the fallback flag, a
/// second marker) sits somewhere INSIDE the argv line, and the terminal
/// stream is chunked with no respect for line boundaries. Asserting on
/// the line the moment the anchor shows up would read a prefix of it and
/// fail, some of the time, on an id or a hook flag that was still in
/// flight — a flake that would look exactly like the fill or the
/// injection being broken. The fixture prints `READY` only after its
/// argv line, so `READY` after the last marker is the proof the line is
/// whole. "Last" matters because a reattach replays earlier generations,
/// each with a complete line and its own `READY`; only the newest one is
/// still being written.
async fn wait_for_settled_argv(rx: &mut TermStream, seen: &mut Vec<u8>, secs: u64) {
    wait_until(rx, seen, secs, "the last argv line to complete", |seen| {
        let text = String::from_utf8_lossy(seen);
        text.rfind(ARGV_MARKER)
            .is_some_and(|idx| text[idx..].contains("FAKE-AGENT READY"))
    })
    .await;
}

/// Attach at [`WIDE_COLS`], wait for the fixture's prompt, send one line,
/// and wait for the record it writes — `provoke_record`'s work at a pane
/// width that does not wrap the argv marker.
///
/// A local copy rather than the shared helper for exactly that reason:
/// `provoke_record` attaches at 80 columns, which is fine for every test
/// that only reads `RECORD-WRITTEN:` but wraps the several-hundred-column
/// argv line these tests assert on (and the attach itself RESIZES the
/// pane, so reading the marker before calling it would not help).
///
/// Returns the conversation id the fixture reported, like the shared
/// helper, so a test can assert the supervisor captured THAT id.
async fn provoke_wide_record(
    h: &Harness,
    session: &SessionInfo,
) -> (u32, TermStream, Vec<u8>, String) {
    let (chan, mut rx) = h
        .client
        .attach(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 30).await;
    h.client.send_input(chan, b"first prompt\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-WRITTEN:", 30).await;
    let id = marker_value(&seen, "RECORD-WRITTEN:");
    (chan, rx, seen, id)
}

/// `/proc/<pid>/stat`, split into the command name and the parent pid.
///
/// The command name is parenthesized and may itself contain spaces and
/// parentheses, so the fields after it are found from the LAST `)` rather
/// than by splitting the whole line — the same parse `process_is_gone`
/// does for the state field. Fields after the name are state, then ppid.
fn comm_and_ppid(pid: u32) -> (String, u32) {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .unwrap_or_else(|e| panic!("reading /proc/{pid}/stat: {e}"));
    let open = stat
        .find('(')
        .unwrap_or_else(|| panic!("/proc/{pid}/stat has no command name: {stat}"));
    let close = stat
        .rfind(')')
        .unwrap_or_else(|| panic!("/proc/{pid}/stat has no command name: {stat}"));
    let comm = stat[open + 1..close].to_string();
    let ppid = stat[close + 1..]
        .split_whitespace()
        .nth(1)
        .and_then(|field| field.parse::<u32>().ok())
        .unwrap_or_else(|| panic!("/proc/{pid}/stat has no parent pid: {stat}"));
    (comm, ppid)
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

/// A wrapper profile is handed the session's own working directory, and
/// the session it launches still captures its conversation.
///
/// This is the feature in one test. The directory contains a SPACE
/// deliberately: substitution is into the argv slot, never into a command
/// string, so a path a shell would have word-split has to arrive as one
/// element — and if it did not, the wrapper's `cd` would fail, the agent
/// would never exec, and this test would time out rather than quietly
/// pass.
///
/// Capture succeeding IS the correlation property, which is why nothing
/// here reads the record file back: the record's `cwd` field is the
/// FIXTURE's `current_dir()`, and the supervisor matches it against the
/// session's canonical cwd. A wrapper handed the wrong directory writes
/// its records under that other directory, correlates against nothing,
/// and leaves the session with no captured identity and no resume offer —
/// the exact silent failure `{cwd}` exists to prevent.
#[tokio::test]
async fn a_wrapper_profile_receives_the_sessions_directory() {
    let (h, fixtures) = capture_harness().await;
    let parent = farhelm_teststate::tempdir().expect("workdir parent");
    let work = dir_with_a_space(parent.path());
    let (invocation, template) = wrapper_profile(&fixtures);

    let session = h
        .client
        .create_session_with_extras(
            &work.to_string_lossy(),
            &invocation,
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras {
                agent_kind: Some(farhelm_proto::AgentKind::Claude),
                resume_template: Some(template),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create a wrapper session in a directory whose path contains a space");

    let (_chan, _rx, seen, reported) = provoke_wide_record(&h, &session).await;

    assert_wrapper_got(&session.id, &work);
    let argv = argv_marker(&seen);
    assert!(
        !argv.contains(farhelm_supervisor::agent_kind::CWD_PLACEHOLDER),
        "no placeholder may reach the agent through the wrapper: {argv}"
    );

    let captured = wait_for_capture(&h, &session.id, 30).await;
    assert_eq!(
        captured, reported,
        "the wrapper's session must capture the conversation the fixture actually wrote"
    );
    assert_eq!(
        snapshot_of(&h, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "a wrapper profile with an explicit kind and a template offers a resume like any other"
    );
}

/// Resuming a wrapper session runs the TEMPLATE through the wrapper, with
/// the directory filled again and the hook flags still appended after the
/// agent's own argv.
///
/// Two things could break independently here, which is why both are
/// asserted. The fill itself is `spawn_agent`'s and is shared by every
/// mode; what differs is the VECTOR that arrives at it. `relaunch_argv`'s
/// `Resume` arm picks the stored template — already substituted for
/// `{conversation}` by `filled_resume_argv` — instead of re-splitting the
/// invocation, so this pins that the template's own `{cwd}` element
/// survives that route intact and reaches the shared fill. A template that
/// lost or mangled it would leave the resumed wrapper `cd`-ing into a
/// literal `{cwd}`. And the hook tail is appended
/// AFTER the fill, at the very end of the outer argv, which means the
/// wrapper has to forward it to the agent — the "a wrapper passes
/// trailing arguments through" property the docs promise, and the thing
/// that keeps a resumed session able to report a later `/clear`.
#[tokio::test]
async fn a_wrapper_session_resumes_through_the_wrapper() {
    let (h, fixtures) = capture_harness().await;
    let parent = farhelm_teststate::tempdir().expect("workdir parent");
    let work = dir_with_a_space(parent.path());
    let (invocation, template) = wrapper_profile(&fixtures);

    let session = h
        .client
        .create_session_with_extras(
            &work.to_string_lossy(),
            &invocation,
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras {
                agent_kind: Some(farhelm_proto::AgentKind::Claude),
                resume_template: Some(template),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create a wrapper session");

    let (_chan, _rx, _seen, _reported) = provoke_wide_record(&h, &session).await;
    let captured = wait_for_capture(&h, &session.id, 30).await;

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Resume, true)
        .await
        .expect("resume the running wrapper session");

    let (_chan, mut rx) = h
        .client
        .attach(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach after the relaunch");
    let mut seen = Vec::new();
    // Anchored on `--resume `, not on the argv marker: a reattach replays
    // the terminal's history, so the previous generation's `FAKE-AGENT
    // ARGV:` line is already in this buffer before the relaunched fixture
    // has printed a byte. `--resume ` appears only in the template, so it
    // is the one token the replay of the create-time launch cannot
    // satisfy. Then wait for the line to be COMPLETE: the stream is
    // chunked, and the id and the hook flags asserted below may still be
    // in flight when the anchor arrives.
    wait_for(&mut rx, &mut seen, "--resume ", 30).await;
    wait_for_settled_argv(&mut rx, &mut seen, 30).await;

    // A restart substitutes the VERIFIED path — see
    // [`a_wrapper_gets_the_literal_spelling_at_create_and_the_verified_path_on_restart`].
    // Asking for the canonical form keeps this honest on a host where the
    // temp directory sits behind a symlink.
    let verified = std::fs::canonicalize(&work).expect("resolve the working directory");
    assert_wrapper_got(&session.id, &verified);
    let argv = argv_marker(&seen);
    assert!(
        argv.contains(&format!("--resume {captured}")),
        "the resumed launch must carry the captured conversation: {argv}"
    );
    assert!(
        !argv.contains(farhelm_supervisor::agent_kind::CWD_PLACEHOLDER),
        "no placeholder may reach the agent through the wrapper: {argv}"
    );
    let settings = argv
        .find("--settings")
        .unwrap_or_else(|| panic!("the relaunch must still be hooked: {argv}"));
    let resume = argv
        .find("--resume")
        .expect("just asserted the resume flag is present");
    assert!(
        settings > resume,
        "the injected hook flags must land after the agent's own argv, not inside it: {argv}"
    );
}

/// A fresh restart of a wrapper session lands in the same directory it
/// was created in.
///
/// `relaunch_argv` has three arms, one per restart mode, and this covers
/// the `Fresh` one: it re-splits the stored INVOCATION, so a fill that
/// lived on the create path alone would relaunch this session into a
/// literal `{cwd}`. (The other two are
/// [`a_wrapper_session_resumes_through_the_wrapper`] and
/// [`a_generic_wrapper_with_a_template_falls_back_through_the_wrapper`].)
/// Nothing is captured before the restart on purpose —
/// an offer of `Resume` would make `Fresh` a conflict, and a wrapper
/// session that has not written a record yet is exactly the state a user
/// restarts out of when the first launch went wrong.
///
/// The proof that the relaunch is in the right directory is the capture
/// that follows it, for the same reason as
/// [`a_wrapper_profile_receives_the_sessions_directory`]: a record only
/// correlates when the fixture's own `current_dir()` matches the
/// session's canonical cwd. The argv marker is asserted too, but it can
/// only say the agent was launched cleanly — the wrapper's own argv is
/// where the substituted value is visible.
///
/// The directory asserted after the restart is the created spelling, which
/// works here only because a tempdir under `/tmp` already IS its own
/// resolution. A restart substitutes the VERIFIED path, and
/// [`a_wrapper_gets_the_literal_spelling_at_create_and_the_verified_path_on_restart`]
/// is where that distinction is made observable.
#[tokio::test]
async fn a_wrapper_session_fresh_restarts_into_the_same_directory() {
    let (h, fixtures) = capture_harness().await;
    let parent = farhelm_teststate::tempdir().expect("workdir parent");
    let work = dir_with_a_space(parent.path());
    let (invocation, template) = wrapper_profile(&fixtures);

    let session = h
        .client
        .create_session_with_extras(
            &work.to_string_lossy(),
            &invocation,
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras {
                agent_kind: Some(farhelm_proto::AgentKind::Claude),
                resume_template: Some(template),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create a wrapper session");

    // Attached once before the restart for a single reason: waiting for
    // READY is what proves the first launch is actually up, so the restart
    // below exercises a live-session relaunch rather than racing the
    // initial spawn.
    let (_chan, mut rx) = h
        .client
        .attach(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 30).await;

    assert_eq!(
        snapshot_of(&h, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly,
        "this test's premise is that nothing has been captured yet, so a fresh restart is the \
         only offer"
    );

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("fresh-restart the running wrapper session");

    let (chan, mut rx) = h
        .client
        .attach(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach after the relaunch");
    let mut seen = Vec::new();
    // The first marker this attach sees IS the relaunched run's. Both
    // generations print the identical argv line (a fresh restart reruns
    // the same invocation), but `respawn-pane` reinitializes the visible
    // grid and the first run never scrolled, so its marker exists nowhere
    // the reattach could replay from — restart does not preserve the
    // previous run's screen (SPEC.md, Lifecycle operations/Restart). This
    // wait once anchored on a SECOND marker, back when the relaunch pushed
    // the old grid into history first; waiting for two markers now times
    // out, because only one can ever arrive. The line still has to be
    // complete before the "no `--resume`" assertion below means anything.
    wait_for(&mut rx, &mut seen, ARGV_MARKER, 30).await;
    wait_for_settled_argv(&mut rx, &mut seen, 30).await;

    // The verified path, as on every restart; identical to the created
    // spelling here unless the temp directory is reached through a symlink.
    let verified = std::fs::canonicalize(&work).expect("resolve the working directory");
    assert_wrapper_got(&session.id, &verified);
    let argv = argv_marker(&seen);
    assert!(
        !argv.contains(farhelm_supervisor::agent_kind::CWD_PLACEHOLDER),
        "no placeholder may reach the agent through the wrapper: {argv}"
    );
    assert!(
        !argv.contains("--resume"),
        "a fresh restart reruns the invocation, not the resume template: {argv}"
    );

    // The record is written only on first input, and no earlier run wrote
    // one, so `RECORD-WRITTEN:` is unambiguous evidence of THIS launch.
    h.client.send_input(chan, b"first prompt\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-WRITTEN:", 30).await;
    let reported = marker_value(&seen, "RECORD-WRITTEN:");
    let captured = wait_for_capture(&h, &session.id, 30).await;
    assert_eq!(
        captured, reported,
        "the relaunched run's record must correlate to this session, which it can only do from \
         the session's own directory"
    );
}

/// A generic wrapper whose profile carries a verbatim fallback resume
/// command relaunches through the wrapper, with `{cwd}` filled again.
///
/// This is `relaunch_argv`'s remaining arm. `FallbackTemplate` reads the
/// stored template like `Resume` does, but without the
/// `{conversation}` substitution in front of it — the vector goes to
/// `spawn_agent` as it was written down. A fill that had been attached to
/// the resume path rather than to `spawn_agent` would therefore relaunch
/// this session into a literal `{cwd}`, and the wrapper's `cd` would fail
/// silently. Nothing else in the file exercises this arm: a wrapper
/// profile with a declared kind cannot reach it, since an integrated kind
/// must have a `{conversation}` template.
///
/// The template is the invocation's own agent command plus
/// [`FALLBACK_FLAG`], which is what makes "ran the template" and "ran the
/// invocation again" tell apart in the transcript. It carries no
/// `{conversation}`, and that placeholder-free shape is the only thing
/// that produces a `FallbackTemplate` offer at all.
#[tokio::test]
async fn a_generic_wrapper_with_a_template_falls_back_through_the_wrapper() {
    let (h, fixtures) = capture_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (invocation, _resume_template) = wrapper_profile(&fixtures);

    let agent = fixtures.bin().join("claude");
    let tail = record_tail(&fixtures);
    let mut fallback_tail: Vec<&str> = tail.iter().map(String::as_str).collect();
    fallback_tail.push(FALLBACK_FLAG);
    let template = wrapper_argv(&agent, &fallback_tail);

    let session = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            &invocation,
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras {
                resume_template: Some(template),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create a generic wrapper session with a verbatim fallback resume command");
    assert_eq!(
        session.restart_offer,
        farhelm_proto::RestartOffer::FallbackTemplate,
        "a generic session with a placeholder-free template offers that template, and the rest \
         of this test has no meaning if it does not"
    );

    let (_chan, mut rx) = h
        .client
        .attach(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 30).await;
    assert_wrapper_got(&session.id, work.path());

    h.client
        .restart_session(
            &session.id,
            farhelm_proto::RestartMode::FallbackTemplate,
            true,
        )
        .await
        .expect("restart the wrapper session through its fallback template");

    let (_chan, mut rx) = h
        .client
        .attach(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach after the relaunch");
    let mut seen = Vec::new();
    // Anchored on the template's own flag rather than on the argv marker:
    // a reattach replays the create-time launch's marker, and the flag is
    // the one token that replay cannot produce.
    wait_for(&mut rx, &mut seen, FALLBACK_FLAG, 30).await;
    wait_for_settled_argv(&mut rx, &mut seen, 30).await;

    // The VERIFIED path, not the literal spelling, because this is a
    // restart — see
    // [`a_wrapper_gets_the_literal_spelling_at_create_and_the_verified_path_on_restart`].
    // The two coincide for an ordinary tempdir; asking for the canonical
    // one keeps this test honest on a host where /tmp is a symlink.
    let verified = std::fs::canonicalize(work.path()).expect("resolve the working directory");
    assert_wrapper_got(&session.id, &verified);
    let argv = argv_marker(&seen);
    assert!(
        argv.contains(FALLBACK_FLAG),
        "the relaunch must run the template, not the launch invocation again: {argv}"
    );
    assert!(
        !argv.contains(farhelm_supervisor::agent_kind::CWD_PLACEHOLDER),
        "no placeholder may reach the agent through the wrapper: {argv}"
    );
}

/// A wrapper is handed the user's own spelling of the directory at create
/// and the VERIFIED resolution of it on a restart (plan D1).
///
/// The session is created through a symlink so the two spellings are
/// different strings, which is the only way this distinction is
/// observable at all — with a direct path the assertion would pass no
/// matter which rule the code followed.
///
/// Both values are right, and they are right for the same reason: the
/// substituted value is whatever the launch hands TMUX as the pane's
/// working directory, never a second opinion computed alongside it. So
/// the wrapper's directory and the pane's directory are the same string
/// on every path, and the agent's `getcwd()` still equals the session's
/// canonical cwd for capture to correlate on. A create has no prior
/// identity to check the path against, so the user's spelling is what
/// tmux gets. A restart does have one: `ensure_cwd_identity` confirms the
/// path still resolves to the identity recorded at create and hands back
/// the RESOLVED path, which the relaunch then uses — a symlink repointed
/// between the check and the launch would otherwise put the agent
/// somewhere nothing validated, and a wrapper handed the unverified
/// spelling would `cd` there itself.
///
/// The restart is `Fresh` and happens before anything is captured, for
/// the same reason as
/// [`a_wrapper_session_fresh_restarts_into_the_same_directory`]: an offer
/// of `Resume` would make `Fresh` a conflict.
#[tokio::test]
async fn a_wrapper_gets_the_literal_spelling_at_create_and_the_verified_path_on_restart() {
    let (h, fixtures) = capture_harness().await;
    let parent = farhelm_teststate::tempdir().expect("workdir parent");
    let real = parent.path().join("real");
    std::fs::create_dir(&real).expect("create the real working directory");
    let link = parent.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("symlink the working directory");
    let verified = std::fs::canonicalize(&link).expect("resolve the symlinked working directory");
    assert_ne!(
        link, verified,
        "the premise is that the created spelling and its resolution differ; a filesystem where \
         they do not would make this test assert nothing"
    );

    let (invocation, template) = wrapper_profile(&fixtures);
    let session = h
        .client
        .create_session_with_extras(
            &link.to_string_lossy(),
            &invocation,
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras {
                agent_kind: Some(farhelm_proto::AgentKind::Claude),
                resume_template: Some(template),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create a wrapper session through a symlinked working directory");

    let (_chan, mut rx) = h
        .client
        .attach(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 30).await;
    assert_wrapper_got(&session.id, &link);

    assert_eq!(
        snapshot_of(&h, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly,
        "nothing has been captured yet, so a fresh restart is the only legal mode here"
    );
    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("fresh-restart the wrapper session");

    let (_chan, mut rx) = h
        .client
        .attach(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach after the relaunch");
    let mut seen = Vec::new();
    // The first marker is the relaunched run's, for the same reason as in
    // the fresh-restart test above: the respawn reinitialized the grid, the
    // first run never scrolled, so no replay can carry the old marker.
    wait_for(&mut rx, &mut seen, ARGV_MARKER, 30).await;
    wait_for_settled_argv(&mut rx, &mut seen, 30).await;

    assert_wrapper_got(&session.id, &verified);
}

/// Stopping a wrapper session kills the wrapper, the agent, and
/// everything under the agent.
///
/// The wrapper's presence is the one structural difference a wrapper
/// profile makes to teardown: there is an extra process between the pane
/// and the agent, holding whatever the real wrapper holds (a `flock`, in
/// the case this feature was built for). The docs claim nothing about
/// that process lets ANY of them escape the stop sweep, and this is where
/// that claim is pinned.
///
/// The whole four-level chain is asserted rather than just the two ends.
/// The `spawner` fixture forks `sh -c 'sleep 3600'`, which forks the
/// `sleep` in turn, so under the wrapper the tree is wrapper → agent →
/// `sh` → `sleep`. Checking only the wrapper and the agent would let a
/// sweep that stopped descending — at a depth limit, or at the first
/// process it could not classify — pass while leaking the leaves, and the
/// extra level a wrapper adds is exactly the kind of change that would
/// push a tree past such a limit.
///
/// Residency is asserted first, and it is not a formality: it is the
/// property [`WRAPPER_SCRIPT`]'s trailing `; exit $?` exists to produce,
/// and it is what makes the rest of this test about a wrapper at all
/// rather than about an ordinary single-process launch. A `/bin/sh` that
/// is dash forks either way, so on a dash host the assertion passes on
/// its own; a `/bin/sh` that is bash tail-`exec`s the last command of a
/// `-c` script, and there the trailing `exit` is the whole difference.
/// The assertion is what keeps the difference from going unnoticed on
/// whichever kind of host this runs.
#[tokio::test]
async fn stopping_a_wrapper_session_reaps_the_wrapper_and_the_agent() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let invocation = wrapper_invocation(
        std::path::Path::new(farhelm_bin()),
        &["internal", "fake-agent", "--script", "spawner"],
    );
    let session = h
        .client
        .create_session(&work.path().to_string_lossy(), &invocation, None, 80, 24)
        .await
        .expect("create a wrapper session running the spawner fixture");

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 30).await;
    let agent_pid = extract_pid(&seen, "SELF-PID:");
    let child_pid = extract_pid(&seen, "CHILD-PID:");
    // The `sleep` the child shell forks is never printed by the fixture,
    // so it has to be discovered — and waited for, since `spawn()`
    // returning says nothing about how far the child has gotten.
    let grandchild_pid = wait_for_child(child_pid, 10).await;

    let (_agent_comm, wrapper_pid) = comm_and_ppid(agent_pid);
    let (wrapper_comm, _) = comm_and_ppid(wrapper_pid);
    assert_eq!(
        wrapper_comm, "sh",
        "the agent's parent must be the resident wrapper shell, not whatever the launch shim \
         left behind"
    );
    let wrapper_argv = cmdline_of(wrapper_pid);
    assert!(
        wrapper_argv.iter().any(|word| word == WRAPPER_SCRIPT),
        "the parent shell must be this test's wrapper: {wrapper_argv:?}"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    wait_until_pid_gone(agent_pid, 15).await;
    wait_until_pid_gone(wrapper_pid, 15).await;
    wait_until_pid_gone(child_pid, 15).await;
    wait_until_pid_gone(grandchild_pid, 15).await;
}

/// A wrapper profile with NEITHER an agent kind NOR a resume template
/// gets no resume offer, however well the launch itself works.
///
/// Both halves of that premise are load-bearing, and the `FreshOnly`
/// result is not attributable to either one alone: a generic kind WITH a
/// placeholder-free template offers `FallbackTemplate` instead, which is
/// what [`a_generic_wrapper_with_a_template_falls_back_through_the_wrapper`]
/// pins. What this test is about is the missing KIND — the template is
/// omitted only so that nothing else can be producing the outcome.
///
/// The missing kind is the failure a user hits when they build a wrapper
/// profile and forget it, and it is silent: the session launches, the
/// agent runs, records get written, and the only symptom is a restart that
/// can only be fresh. Kind derivation reads the invocation's FIRST word,
/// and for a wrapper that word is the wrapper — so `generic` is the
/// correct answer here and the profile has to say otherwise itself.
#[tokio::test]
async fn a_generic_wrapper_profile_gets_no_resume_offer() {
    let (h, fixtures) = capture_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (invocation, _template) = wrapper_profile(&fixtures);

    let session = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            &invocation,
            None,
            80,
            24,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .expect("create a wrapper session with no kind and no template");

    let (_chan, _rx, _seen, _reported) = provoke_record(&h, &session).await;
    assert_wrapper_got(&session.id, work.path());
    settle_past_horizon(&h).await;

    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.captured_conversation, None,
        "a generic session has no integration to parse records with, so there is nothing to \
         capture"
    );
    assert_eq!(
        snapshot.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly,
        "a wrapper profile that never declared its kind can only ever be restarted fresh"
    );
}

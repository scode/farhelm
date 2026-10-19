//! Prompt delivery through the shipped CLI, real helm and real tmux.
//!
//! A raw terminal recorder makes the delivery contract observable without
//! running a vendor agent. The real supervisor samples activity, so the
//! recorder draws static, recognized busy indicators to keep Running stable
//! even after many samples. Status-specific refusals belong in the supervisor
//! tests. Viewer probes establish live input after replay and prove that
//! delivery leaves the original attachment usable.
//!
//! While the helm refuses agent prompts, only the turned-off proof runs; the
//! delivery proofs are ignored until the re-enable (see the note above them).

use crate::agent_listing_real_stack::{attach_terminal, hosts_until_attached, session_token};
use crate::harness::*;
use farhelm_proto::{CommandLaunch, LaunchHarness};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use tokio::io::AsyncReadExt;

/// Raw mode preserves newlines, tabs and submit bytes that a canonical PTY
/// would otherwise buffer or translate. Flush the record before acknowledging
/// its length, so a live acknowledgement is also a file-observation barrier.
/// Python's standard termios support avoids adding a native fixture dependency.
/// The synthetic Claude composer and Codex title use the working shapes pinned
/// in `agent_kind::screen_reader` tests. They keep this real-process fixture
/// working without continuous output or a race against the first idle sample.
const RECORDER: &str = r#"import os
import sys
import tty

tty.setraw(0)
# The declared agent's integration arguments follow the fixture paths. They
# satisfy the production launch contract; this byte recorder does not use them.
record = open(sys.argv[1], "wb", buffering=0)
os.write(1, b"\x1b[?2004hPROMPT-RECORDER READY\r\n")
os.write(1, "\x1b]2;⠏ prompt-fixture\x07✶ Imagining… (2s)\r\n────\r\n❯ \r\n────\r\n".encode("utf-8"))
with open(sys.argv[2], "w") as pid:
    pid.write(str(os.getpid()))
total = 0
while True:
    data = os.read(0, 4096)
    if not data:
        break
    record.write(data)
    total += len(data)
    os.write(1, ("RECORDED:%d\r\n" % total).encode("ascii"))
"#;

/// One owned recorder process, whose exact-input file survives its terminal
/// observer. The parent test directory outlives every supervisor child.
struct Recorder {
    session: SessionInfo,
    record: PathBuf,
    pid: u32,
}

/// A single attachment retained across delivery. Its transcript starts after
/// replay, so acknowledgements cannot be satisfied by old screen contents.
struct Viewer {
    channel: u32,
    stream: TermStream,
    seen: Vec<u8>,
    expected: Vec<u8>,
}

impl Viewer {
    /// Challenge a known live, raw-mode recorder through this attachment.
    /// The acknowledgement proves both input and output remain on the same
    /// attachment; exact bytes also detect unsolicited input from a refusal.
    async fn probe(&mut self, client: &SupervisorClient, recorder: &Recorder, bytes: &[u8]) {
        recorder.assert_alive();
        tokio::time::timeout(
            REAL_STACK_SETTLE,
            client.send_input(self.channel, bytes.to_vec()),
        )
        .await
        .expect("viewer input queue stalled");
        self.expected.extend_from_slice(bytes);
        self.observe(recorder).await;
    }

    /// Wait for the recorder to consume the expected prefix, then compare the
    /// whole file. A count larger than expected settles too, exposing extra
    /// bytes as an equality failure instead of waiting for an impossible count.
    async fn observe(&mut self, recorder: &Recorder) {
        let expected_len = self.expected.len();
        let observed = tokio::time::timeout(
            REAL_STACK_SETTLE,
            wait_until(
                &mut self.stream,
                &mut self.seen,
                REAL_STACK_SETTLE.as_secs() + 1,
                "live raw-input acknowledgement",
                |seen| {
                    assert!(
                        seen.len() <= 16 * 1024,
                        "recorder output exceeded its bound"
                    );
                    String::from_utf8_lossy(seen).lines().any(|line| {
                        line.strip_prefix("RECORDED:")
                            .and_then(|count| count.trim().parse::<usize>().ok())
                            .is_some_and(|count| count >= expected_len)
                    })
                },
            ),
        )
        .await;
        assert!(
            observed.is_ok(),
            "raw-input acknowledgement timed out; recorder {} alive: {}",
            recorder.session.id,
            !process_is_gone(recorder.pid)
        );
        recorder.assert_alive();
        assert_eq!(
            std::fs::read(&recorder.record).expect("read acknowledged raw input"),
            self.expected,
            "the recorder received unexpected input"
        );
    }
}

impl Recorder {
    /// A readiness file identifies the process that actually enabled raw mode.
    /// Checking that identity before writes distinguishes a dead fixture from
    /// a delivery or attachment regression.
    fn assert_alive(&self) {
        assert!(
            !process_is_gone(self.pid),
            "recorder {} exited before the observation",
            self.session.id
        );
    }
}

/// Route through the real local host and grant only the existing command
/// permission. GUI-created command fixtures do not need agent-launch authority.
async fn trust_local_commands(client: &reqwest::Client, base: &str) {
    let hosts = wait_for_listing_with(
        || async { Ok(get_json(client, &format!("{base}/api/hosts")).await) },
        REAL_STACK_SETTLE.as_secs(),
        "the real helm's local host is connected",
        |hosts: &serde_json::Value| {
            hosts["hosts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|host| host["kind"] == "local" && host["state"]["phase"] == "connected")
        },
    )
    .await;
    let host = hosts["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["kind"] == "local")
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let (status, body) = tokio::time::timeout(
        REAL_STACK_SETTLE,
        post(
            client,
            &format!("{base}/api/hosts/{host}/commands-without-asking"),
            serde_json::json!({"commands_without_asking": true}),
        ),
    )
    .await
    .expect("trusting fixture commands stalled");
    assert!(
        status.is_success(),
        "trusting fixture commands failed: {body}"
    );
}

/// Launch a declared or generic command through the real helm's create route.
/// The pid file is published after raw mode and bracketed paste are enabled;
/// attachment readiness is established separately by a live viewer probe.
/// Declared command launches require a whole-argument integration placeholder.
/// Its expansion follows the recorder's paths, which remain argv[1] and argv[2].
/// An undeclared command must omit it: there is no integration to expand.
async fn create_recorder(
    client: &reqwest::Client,
    base: &str,
    work: &Path,
    name: &str,
    agent: Option<LaunchHarness>,
) -> Recorder {
    let record = work.join(format!("{name}.input"));
    let pid_path = work.join(format!("{name}.pid"));
    let integration = if agent.is_some() {
        " {farhelm_args}"
    } else {
        ""
    };
    let command = format!(
        "python3 {} {} {}{integration}",
        shell_words::quote(&work.join("recorder.py").to_string_lossy()),
        shell_words::quote(&record.to_string_lossy()),
        shell_words::quote(&pid_path.to_string_lossy()),
    );
    let launch = CommandLaunch {
        command,
        yolo: false,
        agent,
        resume: None,
    };
    let (status, body) = tokio::time::timeout(
        REAL_STACK_SETTLE,
        post(
            client,
            &format!("{base}/api/sessions"),
            serde_json::json!({"cwd": work, "command": launch, "title": name}),
        ),
    )
    .await
    .expect("creating the recorder stalled");
    assert!(status.is_success(), "creating {name} failed: {body}");
    let session = serde_json::from_str(&body).expect("created session");
    let pid = wait_for_pid_file(&pid_path, REAL_STACK_SETTLE.as_secs()).await;
    let recorder = Recorder {
        session,
        record,
        pid,
    };
    recorder.assert_alive();
    recorder
}

/// Observe a recorder from a fresh full-authority connection without changing
/// the requesting session's helm attachment. Replay is consumed before the
/// first probe, which must produce new bytes from this recorder process.
async fn viewer(client: &SupervisorClient, recorder: &Recorder) -> Viewer {
    recorder.assert_alive();
    let (channel, _, stream) = tokio::time::timeout(
        REAL_STACK_SETTLE,
        client.attach_live(&recorder.session.id, WIDE_COLS, ROWS),
    )
    .await
    .expect("attaching the recorder viewer stalled")
    .expect("attach the recorder viewer");
    let mut viewer = Viewer {
        channel,
        stream,
        seen: Vec::new(),
        expected: Vec::new(),
    };
    viewer.probe(client, recorder, b"viewer-before").await;
    viewer
}

/// Run one authenticated CLI child with bounded pipes and an overall deadline.
/// A timeout kills and reaps the owned child before reporting failure; dropping
/// a bare output future would leave the CLI alive after its test failed.
async fn prompt_cli(
    asking: &Recorder,
    token: &str,
    socket: &Path,
    target: &str,
    text: &str,
) -> Output {
    asking.assert_alive();
    let mut child = tokio::process::Command::new(farhelm_bin())
        .args(["agent", "prompt", "--session", target, "--prompt", text])
        .env(
            farhelm_supervisor::launch::SESSION_ID_ENV_VAR,
            &asking.session.id,
        )
        .env(farhelm_supervisor::launch::SESSION_TOKEN_ENV_VAR, token)
        .env(farhelm_supervisor::launch::SUPERVISOR_SOCK_ENV_VAR, socket)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn the prompt CLI");
    let mut stdout = child.stdout.take().unwrap().take(8193);
    let mut stderr = child.stderr.take().unwrap().take(8193);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let result = tokio::time::timeout(REAL_STACK_SETTLE, async {
        tokio::join!(
            child.wait(),
            stdout.read_to_end(&mut out),
            stderr.read_to_end(&mut err)
        )
    })
    .await;
    let (status, read_out, read_err) = match result {
        Ok(result) => result,
        Err(_) => {
            let cleanup = tokio::time::timeout(Duration::from_secs(5), child.kill()).await;
            panic!(
                "prompt CLI timed out; asking recorder alive: {}; child cleanup: {cleanup:?}; stdout: {}; stderr: {}",
                !process_is_gone(asking.pid),
                String::from_utf8_lossy(&out),
                String::from_utf8_lossy(&err)
            );
        }
    };
    read_out.expect("read prompt stdout");
    read_err.expect("read prompt stderr");
    assert!(
        out.len() <= 8192 && err.len() <= 8192,
        "CLI output exceeded its bound"
    );
    Output {
        status: status.expect("reap prompt CLI"),
        stdout: out,
        stderr: err,
    }
}

/// What one run expects `farhelm agent prompt` to do to a Running target whose
/// host runs farhelm commands without asking, so no approval card stands in
/// the way of delivery.
enum Expect {
    /// Agent prompts are turned off in the helm: the CLI fails with the
    /// helm's refusal and the target's terminal receives no input from it.
    TurnedOff,
    /// One paste followed by `submit`, optionally followed by the generic and
    /// self-target refusals. Only reachable once prompts are re-enabled.
    Delivered { submit: u8, include_refusals: bool },
}

/// Run one prompt command against a Running recorder and check `expect`.
/// Keeping the same viewer for probes on either side of the command detects
/// attachment takeover, while exact raw bytes distinguish one paste from
/// ordinary typed keystrokes, and a refusal from a delivery.
async fn prove_prompt(agent: LaunchHarness, expect: Expect) {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    // The fixture files must outlive the processes and their failure diagnostics.
    let work = farhelm_teststate::tempdir().expect("prompt recorder workdir");
    std::fs::write(work.path().join("recorder.py"), RECORDER).expect("write the raw recorder");
    let supervisor = supervisor_process().await;
    let helm = helm_process(supervisor.state.path(), None).await;
    let secret = device_secret(supervisor.state.path(), &helm.base).await;
    let client = client_with_secret(&secret);
    trust_local_commands(&client, &helm.base).await;
    let asking = create_recorder(&client, &helm.base, work.path(), "asking", Some(agent)).await;
    let target = create_recorder(&client, &helm.base, work.path(), "target", Some(agent)).await;
    let _asking_terminal = attach_terminal(&helm, &secret, &asking.session.id).await;
    let token = session_token(supervisor.state.path(), &asking.session.id).await;
    let socket = supervisor.state.path().join("supervisor.sock");
    hosts_until_attached(&asking.session.id, &token, &socket).await;

    let transport = farhelm_supervisor::service::connect(supervisor.state.path())
        .await
        .expect("connect the owned observer");
    let (read, write) = tokio::io::split(transport);
    let observer = tokio::time::timeout(REAL_STACK_SETTLE, SupervisorClient::start(read, write))
        .await
        .expect("observer handshake stalled")
        .expect("observer handshake");
    let mut target_viewer = viewer(&observer, &target).await;
    let listing = tokio::time::timeout(REAL_STACK_SETTLE, observer.list_sessions())
        .await
        .expect("live fixture listing stalled")
        .expect("live fixture listing");
    let current = listing
        .sessions
        .iter()
        .find(|row| row.id == target.session.id)
        .unwrap();
    assert_eq!(
        current.status,
        SessionStatus::Running,
        "the proof requires a working target"
    );

    let text = "first line\nsecond\tline λ";
    let output = prompt_cli(&asking, &token, &socket, &target.session.id, text).await;
    let (submit, include_refusals) = match expect {
        Expect::TurnedOff => {
            let reason = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "turned-off prompt succeeded");
            assert!(
                reason.contains("turned off in this version of Farhelm"),
                "unexpected refusal: {reason}"
            );
            // The probe's exact-file comparison is the delivery check: the
            // record must hold only the two viewer probes, with no paste or
            // submit byte from the refused command between them.
            target_viewer
                .probe(&observer, &target, b"viewer-after")
                .await;
            return;
        }
        Expect::Delivered {
            submit,
            include_refusals,
        } => (submit, include_refusals),
    };
    assert!(
        output.status.success(),
        "prompt failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    target_viewer.expected.extend_from_slice(b"\x1b[200~");
    target_viewer.expected.extend_from_slice(text.as_bytes());
    target_viewer.expected.extend_from_slice(b"\x1b[201~");
    target_viewer.expected.push(submit);
    target_viewer.observe(&target).await;
    target_viewer
        .probe(&observer, &target, b"viewer-after")
        .await;

    if include_refusals {
        let unsupported = create_recorder(&client, &helm.base, work.path(), "generic", None).await;
        let mut generic_viewer = viewer(&observer, &unsupported).await;
        let refused = prompt_cli(&asking, &token, &socket, &unsupported.session.id, text).await;
        let reason = String::from_utf8_lossy(&refused.stderr).to_lowercase();
        assert!(
            !refused.status.success(),
            "generic target unexpectedly accepted"
        );
        assert!(
            reason.contains("claude") && reason.contains("codex"),
            "unexpected refusal: {reason}"
        );
        generic_viewer
            .probe(&observer, &unsupported, b"generic-after-refusal")
            .await;

        let refused = prompt_cli(&asking, &token, &socket, &asking.session.id, text).await;
        let reason = String::from_utf8_lossy(&refused.stderr).to_lowercase();
        assert!(
            !refused.status.success(),
            "self target unexpectedly accepted"
        );
        assert!(
            reason.contains("self")
                || reason.contains("asking session")
                || reason.contains("own session"),
            "unexpected refusal: {reason}"
        );
        // This can replace the asking helm's attachment, so it happens only
        // after the final CLI call. Its first probe must be the asker's only input.
        let _asking_viewer = viewer(&observer, &asking).await;
    }
}

/// Spec: while agent prompts are turned off, `farhelm agent prompt` from a
/// session, through the real supervisor relay and real helm, fails with the
/// helm's turned-off refusal, and the Running Claude target, on a host that
/// runs farhelm commands without asking, receives no input from it.
///
/// Why: the helm's unit test pins where its switch sits; this one proves the
/// shipped path actually ends there, with an exact record of every byte the
/// target's terminal got. A target that would otherwise accept delivery with
/// no card is the case the switch exists for.
#[farhelm_testtrace::test]
async fn prompt_is_refused_end_to_end_while_turned_off() {
    prove_prompt(LaunchHarness::Claude, Expect::TurnedOff).await;
}

// The two delivery proofs below are ignored, not deleted, while the helm
// refuses agent prompts (`AGENT_PROMPTS_ENABLED` in the helm's
// `agent_requests`): run against the real helm they could only fail. They are
// the end-to-end evidence a re-enable needs, so un-ignore them with it. The
// helm's unit tests keep its approval and delivery path covered meanwhile.

/// Claude delivery must preserve one multiline paste and submit with CR, even
/// with a viewer attached. Refused self and generic targets receive no prompt.
#[farhelm_testtrace::test]
#[ignore = "agent prompts to running sessions are disabled; see TODO.md, Near term"]
async fn claude_prompt_relay_preserves_bytes_and_viewer_and_refuses_invalid_targets() {
    prove_prompt(
        LaunchHarness::Claude,
        Expect::Delivered {
            submit: b'\r',
            include_refusals: true,
        },
    )
    .await;
}

/// Codex uses Tab after the complete paste. A tab inside the message remains
/// data, and the original viewer still accepts input after the relay returns.
#[farhelm_testtrace::test]
#[ignore = "agent prompts to running sessions are disabled; see TODO.md, Near term"]
async fn codex_prompt_relay_preserves_bytes_and_viewer() {
    prove_prompt(
        LaunchHarness::Codex,
        Expect::Delivered {
            submit: b'\t',
            include_refusals: false,
        },
    )
    .await;
}

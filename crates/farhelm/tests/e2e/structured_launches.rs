//! Structured launch forwarding at the process and durable-lifecycle boundary.
//!
//! Compiler tests establish the release catalog's argv vocabulary. This module
//! supplies the complementary evidence that a real owned supervisor launches
//! a fake executable with those literal arguments, keeps the declarative
//! selection in its create/list/store projections, and starts a later process
//! generation from the same frozen bundle. The fake programs are shell
//! wrappers around the shipped multi-call test binary. Their private Bash
//! login home and explicitly pinned child shell keep vendor executables and
//! the test runner's configuration outside the proof.

use crate::agent_relay::{ScriptedHandler, SessionPeer, connect_helm, credential_for};
use crate::harness::*;
use farhelm_helm::CreateExtras;
use farhelm_proto::{
    AgentKind, AgentReply, ControlMsg, LaunchEffort, LaunchHarness, LaunchPermission,
    LaunchSelection, RestartMode,
};

/// An owned named executable plus the working directory it must outlive.
///
/// Harness identity is normally inferred from a vendor program basename. The
/// named wrappers preserve that production input while the target's fake-agent
/// subcommand gives the test a deterministic post-exec readiness and argv
/// witness.
pub(crate) struct FakeHarness {
    bin: farhelm_teststate::TestDir,
    home: farhelm_teststate::TestDir,
    work: farhelm_teststate::TestDir,
    bash: std::path::PathBuf,
}

impl FakeHarness {
    /// Configure the fixture-owned login home to find these fake harnesses.
    ///
    /// The production launcher deliberately uses a login shell, which may
    /// rebuild `PATH` and discard a supervisor child's inherited value.
    /// A private profile exercises that real shell behavior while keeping
    /// the fake names out of the test process and the operator's home.
    pub(crate) fn login_home_with_fake_path(&self) -> std::ffi::OsString {
        std::fs::write(
            self.home.path().join(".bash_profile"),
            format!(
                "export PATH={}:$PATH\n",
                shell_words::quote(&self.bin.path().to_string_lossy())
            ),
        )
        .expect("write fixture login profile");
        self.home.path().as_os_str().to_os_string()
    }

    /// Return the fixture's verified Bash executable for the supervisor child.
    ///
    /// The production launcher intentionally respects `SHELL`. Pinning the
    /// discovered executable here makes `.bash_profile` a fixture premise,
    /// rather than accidentally relying on whichever interactive shell ran
    /// the test process.
    pub(crate) fn bash_shell(&self) -> std::ffi::OsString {
        self.bash.as_os_str().to_os_string()
    }

    /// Build a literal argv for one fake vendor entry point.
    ///
    /// The option tail follows the fake-agent script because the multi-call
    /// fixture reserves its leading words for `internal fake-agent`; the
    /// captured process still receives every option as a distinct argv value.
    fn invocation(&self, selection: &LaunchSelection) -> String {
        let mut argv = vec![
            self.bin
                .path()
                .join(match selection.harness {
                    LaunchHarness::Codex => "codex",
                    LaunchHarness::Claude => "claude",
                    LaunchHarness::Muse => "muse",
                    LaunchHarness::Cursor => "agent",
                    LaunchHarness::Goose => "goose",
                    LaunchHarness::Pi => "pi",
                    LaunchHarness::Omp => "omp",
                    LaunchHarness::OpenCode => "opencode",
                    LaunchHarness::Grok => "grok",
                })
                .to_string_lossy()
                .into_owned(),
            "internal".to_string(),
            "fake-agent".to_string(),
            "--script".to_string(),
            // Record fixtures are the existing fake-agent scripts that emit
            // ARGV_MARKER. Their private home keeps the incidental record
            // write owned by this fixture and outside any real agent state.
            "claude-record".to_string(),
            "--record-home".to_string(),
            self.home.path().to_string_lossy().into_owned(),
        ];
        if selection.harness == LaunchHarness::Grok {
            // The compiler owns exact ordering; this process fixture pins the
            // ownership flag's survival through the supervisor boundary.
            argv.push("--no-leader".to_string());
        }
        if let Some(model) = &selection.model {
            match selection.harness {
                LaunchHarness::Codex => argv.extend(["-m".to_string(), model.clone()]),
                LaunchHarness::Claude
                | LaunchHarness::Cursor
                | LaunchHarness::Muse
                | LaunchHarness::Goose
                | LaunchHarness::Pi
                | LaunchHarness::OpenCode => argv.extend(["--model".to_string(), model.clone()]),
                // OMP compiles the provider-qualified form its CLI's
                // resolver documents; the fixture mirrors that shape so a
                // forwarded argv reads like a real compiled launch.
                LaunchHarness::Omp => argv.extend([
                    "--provider".to_string(),
                    "openrouter".to_string(),
                    "--model".to_string(),
                    model.clone(),
                ]),
                LaunchHarness::Grok => unreachable!("Grok has no supported model"),
            }
        }
        if let Some(effort) = selection.effort {
            match selection.harness {
                LaunchHarness::Codex => argv.extend([
                    "-c".to_string(),
                    format!("model_reasoning_effort={}", effort.as_cli_arg()),
                ]),
                LaunchHarness::Claude => {
                    argv.extend(["--effort".to_string(), effort.as_cli_arg().to_string()])
                }
                LaunchHarness::Muse => argv.extend([
                    "--reasoning-effort".to_string(),
                    effort.as_cli_arg().to_string(),
                ]),
                // Goose carries effort in `GOOSE_THINKING_EFFORT`, outside
                // the executable argv this fixture records.
                LaunchHarness::Goose => {}
                LaunchHarness::Pi | LaunchHarness::Omp => {
                    argv.extend(["--thinking".to_string(), effort.as_cli_arg().to_string()])
                }
                LaunchHarness::OpenCode => unreachable!("OpenCode has no supported effort"),
                LaunchHarness::Cursor => unreachable!("Cursor has no supported effort"),
                LaunchHarness::Grok => unreachable!("Grok has no supported effort"),
            }
        }
        if selection.permissions == Some(LaunchPermission::Yolo) {
            let flag = match selection.harness {
                LaunchHarness::Codex | LaunchHarness::Muse => Some("--yolo"),
                LaunchHarness::Claude => Some("--dangerously-skip-permissions"),
                LaunchHarness::OpenCode => Some("--auto"),
                LaunchHarness::Cursor => Some("--force"),
                LaunchHarness::Grok => Some("--always-approve"),
                LaunchHarness::Goose | LaunchHarness::Pi | LaunchHarness::Omp => None,
            };
            if let Some(flag) = flag {
                argv.push(flag.to_string());
            }
        }
        // OMP carries its permission as one explicit approval-mode flag; the
        // harness default adds nothing, mirroring the compiler's shape.
        if selection.harness == LaunchHarness::Omp {
            match selection.permissions {
                Some(LaunchPermission::Yolo) => {
                    argv.extend(["--approval-mode".to_string(), "yolo".to_string()])
                }
                Some(LaunchPermission::Approve) => {
                    argv.extend(["--approval-mode".to_string(), "always-ask".to_string()])
                }
                _ => {}
            }
        }
        shell_words::join(argv)
    }
}

/// Create the named fixture entry points without changing PATH.
///
/// Each wrapper emits a monotonic, per-harness generation before handing its
/// original argv to the shipped fake-agent binary. A restart can retain old
/// terminal scrollback, so a plain readiness marker would let a predecessor
/// satisfy a successor assertion. The wrapper's counter makes that mistake
/// observable without changing the test process environment.
pub(crate) fn fake_harness() -> FakeHarness {
    let bin = farhelm_teststate::tempdir().expect("fixture executable directory");
    let home = farhelm_teststate::tempdir().expect("structured launch agent home");
    for name in ["codex", "claude", "muse", "opencode", "agent", "grok"] {
        let executable = bin.path().join(name);
        let counter = bin.path().join(format!("{name}.generation"));
        std::fs::write(
            &executable,
            format!(
                "#!/bin/sh\ncounter={}\ngeneration=0\nif [ -f \"$counter\" ]; then\n  IFS= read -r generation < \"$counter\"\nfi\ngeneration=$((generation + 1))\nprintf '%s\\n' \"$generation\" > \"$counter\"\nprintf 'STRUCTURED-LAUNCH-GENERATION:%s\\n' \"$generation\"\nprintf 'STRUCTURED-LAUNCH-ARGV-HEX:'\nprintf '%s\\0' \"$@\" | od -An -tx1 | tr -d ' \\n'\nprintf '\\n'\nif [ \"$1\" = internal ]; then\n  exec {} \"$@\"\nfi\nexec {} internal fake-agent --script claude-record --record-home {} \"$@\"\n",
                shell_words::quote(&counter.to_string_lossy()),
                shell_words::quote(farhelm_bin()),
                shell_words::quote(farhelm_bin()),
                shell_words::quote(&home.path().to_string_lossy()),
            ),
        )
        .expect("write structured fake executable");
        let mut permissions = std::fs::metadata(&executable)
            .expect("read structured fake executable permissions")
            .permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&executable, permissions)
            .expect("make structured fake executable runnable");
    }
    let bash = std::process::Command::new("bash")
        .args(["-c", "command -v bash"])
        .output()
        .expect("discover Bash for the owned login-shell fixture");
    assert!(bash.status.success(), "Bash discovery must succeed");
    let bash = std::path::PathBuf::from(
        String::from_utf8(bash.stdout)
            .expect("Bash path is UTF-8")
            .trim(),
    );
    assert!(
        bash.is_absolute() && bash.is_file(),
        "discovered Bash must be an executable path: {bash:?}"
    );
    assert!(
        std::os::unix::fs::PermissionsExt::mode(
            &std::fs::metadata(&bash)
                .expect("read discovered Bash metadata")
                .permissions()
        ) & 0o111
            != 0,
        "discovered Bash must be executable: {bash:?}"
    );
    FakeHarness {
        bin,
        home,
        work: farhelm_teststate::tempdir().expect("structured launch workdir"),
        bash,
    }
}

/// Return the supervisor runtime identity that a structured harness requires.
fn agent_kind(selection: &LaunchSelection) -> AgentKind {
    match selection.harness {
        LaunchHarness::Codex => AgentKind::Codex,
        LaunchHarness::Claude => AgentKind::Claude,
        LaunchHarness::Goose => AgentKind::Goose,
        LaunchHarness::Pi => AgentKind::Pi,
        LaunchHarness::Omp => AgentKind::Omp,
        LaunchHarness::Grok => AgentKind::Grok,
        // Muse deliberately remains a Generic runtime integration: it has no
        // conversation-resume contract for this release.
        LaunchHarness::Muse => AgentKind::Generic,
        LaunchHarness::Cursor => AgentKind::Generic,
        // OpenCode has the same generic lifecycle: no captured conversation
        // means a restart cannot honestly synthesize a resume command.
        LaunchHarness::OpenCode => AgentKind::Generic,
    }
}

/// Launch one frozen structured bundle through the real private tmux server.
async fn launch(
    h: &Harness,
    fixture: &FakeHarness,
    selection: LaunchSelection,
) -> farhelm_proto::SessionInfo {
    h.client
        .create_session_with_extras(
            &fixture.work.path().to_string_lossy(),
            &fixture.invocation(&selection),
            None,
            WIDE_COLS,
            ROWS,
            CreateExtras {
                agent_kind: Some(agent_kind(&selection)),
                launch: Some(selection),
                ..CreateExtras::default()
            },
        )
        .await
        .expect("structured create")
}

/// A requested generation's decoder outcome.
///
/// An attach may cut across a wrapper's output, so incomplete terminal text is
/// pending rather than an assertion failure to catch in a polling loop.
#[derive(Debug)]
enum GenerationArgv {
    Pending,
    Ready(String),
    Error(String),
}

/// Wait for a complete wrapper/fake-agent successor boundary on one attachment.
///
/// Replay remains context, but a predecessor READY cannot satisfy this wait:
/// the requested generation must contain its own wrapper argv, fake argv, and
/// post-exec READY before the decoder accepts it. The buffer is attach
/// replay followed by live bytes, and the replay carries terminal rows:
/// positioning escapes, width padding, and — when the snapshot lands
/// mid-render — a payload-less argv prefix row ahead of the live complete
/// witness. The decoder normalizes rows to text first, then resolves to
/// the most recent witness, joins its payload across the replay's
/// not-yet-rendered rows, and treats an empty decode as a payload still
/// in flight rather than a bare launch.
async fn wait_for_generation_argv(
    stream: &mut TermStream,
    observed: &mut Vec<u8>,
    session: &str,
    generation: u32,
) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        match decode_generation_argv(observed, session, generation) {
            GenerationArgv::Ready(argv) => return argv,
            GenerationArgv::Error(error) => panic!("{error}"),
            GenerationArgv::Pending => {}
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, stream.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => observed.extend_from_slice(&bytes),
            Ok(Some(TermEvent::ReplayComplete)) => {}
            Ok(Some(TermEvent::Detached(reason))) => panic!(
                "session {session}'s generation {generation} detached ({reason}) before its complete post-exec boundary; transcript:\n{}",
                String::from_utf8_lossy(observed)
            ),
            Ok(None) => panic!(
                "session {session}'s generation {generation} closed before its complete post-exec boundary; transcript:\n{}",
                String::from_utf8_lossy(observed)
            ),
            Err(_) => panic!(
                "session {session}'s generation {generation} never reached its complete post-exec boundary; transcript:\n{}",
                String::from_utf8_lossy(observed)
            ),
        }
    }
}

/// Read one specified generation's argv only after that generation is ready.
async fn observed_argv(h: &Harness, session: &str, generation: u32) -> String {
    let (channel, replay, mut stream) = h
        .client
        .attach_live(session, WIDE_COLS, ROWS)
        .await
        .expect("attach an owned post-replay observer");
    let mut observed = replay;
    let argv = wait_for_generation_argv(&mut stream, &mut observed, session, generation).await;
    assert_live_exchange(&h.client, channel, &mut stream, h.state.path(), session).await;
    argv
}

/// Read an argv witness from either in-process or real-stack owned state.
///
/// Both fixtures use the same private tmux protocol, so keeping the
/// generation boundary here prevents their process evidence from drifting.
pub(crate) async fn observed_argv_in_state(
    state: &std::path::Path,
    session: &str,
    generation: u32,
) -> String {
    let socket = state.join("tmux.sock");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    assert_live_pane_before(&socket, session, deadline, "before attaching the observer").await;
    let transport = farhelm_supervisor::service::connect(state)
        .await
        .expect("dial the real supervisor for the owned successor attachment");
    let (reader, writer) = tokio::io::split(transport);
    let client = SupervisorClient::start(reader, writer)
        .await
        .expect("handshake the owned successor attachment");
    let (channel, replay, mut stream) = client
        .attach_live(session, WIDE_COLS, ROWS)
        .await
        .expect("attach an owned post-replay successor observer");
    let mut observed = replay;
    let argv = wait_for_generation_argv(&mut stream, &mut observed, session, generation).await;
    assert_live_exchange(&client, channel, &mut stream, state, session).await;
    argv
}

/// Prove that the attached fake process still handles input after replay.
///
/// A fresh token prevents an earlier observer's response from satisfying this
/// exchange. Require the fake process's colored response, rather than terminal
/// line-discipline echo, and inspect liveness while the fixture still owns it.
pub(crate) async fn assert_live_exchange(
    client: &SupervisorClient,
    channel: u32,
    stream: &mut TermStream,
    state: &std::path::Path,
    session: &str,
) {
    let socket = state.join("tmux.sock");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    assert_live_pane_before(&socket, session, deadline, "before the live exchange").await;
    static PROBE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = PROBE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let token = format!("live-{}-{sequence}", std::process::id());
    client
        .send_input(channel, format!("{token}\r").into_bytes())
        .await;
    let mut live_bytes = Vec::new();
    let response = format!("echo:\x1b[36m{token}\x1b[0m");
    let observed = tokio::time::timeout(
        Duration::from_secs(20),
        wait_for(stream, &mut live_bytes, &response, 30),
    )
    .await;
    assert_live_pane_before(&socket, session, deadline, "after the live exchange").await;
    assert!(
        observed.is_ok(),
        "session {session} did not answer its fresh live probe: {}",
        String::from_utf8_lossy(&live_bytes)
    );
}

/// Diagnose pane liveness without allowing an unbounded tmux child to hang a test.
async fn assert_live_pane_before(
    socket: &std::path::Path,
    session: &str,
    deadline: tokio::time::Instant,
    phase: &str,
) {
    let target = format!("fh-{session}");
    let output = tmux_query_before_deadline(
        socket,
        &["display-message", "-p", "-t", &target, "#{pane_dead}"],
        deadline,
    )
    .await
    .unwrap_or_else(|| panic!("tmux stopped answering {phase} for session {session}"));
    assert!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "0",
        "session {session}'s peer was not live {phase}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Decode exactly one complete successor boundary without panicking for pending text.
///
/// The buffer mixes attach replay with live bytes, and the replay carries
/// terminal rows rather than the wrapper's byte stream: rows arrive with
/// cursor-positioning escapes, width padding, and carriage returns, and a
/// replay taken mid-render holds a payload-less argv prefix row ahead of
/// the live payload. Normalizing first (via the harness's own pane-text
/// normalizer) strips the escapes and padding so the witness join below
/// reads the text the launch actually printed.
fn decode_generation_argv(observed: &[u8], session: &str, generation: u32) -> GenerationArgv {
    let transcript = normalize_pane_text(observed);
    let marker = format!("STRUCTURED-LAUNCH-GENERATION:{generation}");
    let Some(start) = transcript.rfind(&marker) else {
        return GenerationArgv::Pending;
    };
    let generation_bytes = &transcript[start..];
    if !generation_bytes.contains(ARGV_MARKER) || !generation_bytes.contains(AGENT_READY_MARKER) {
        return GenerationArgv::Pending;
    }
    // Decode the most recent argv witness, not the first: the wrapper prints
    // its `STRUCTURED-LAUNCH-ARGV-HEX:` prefix in one write and the `od`
    // pipeline's payload in a later one, so an attach replay taken between
    // the two holds a payload-less prefix row. That stale row sorts before
    // the live complete witness in this buffer; decoding the first
    // candidate accepts the stale empty row as the generation's argv.
    // This matches how the generation marker itself resolves to its last
    // occurrence: within one generation the witnesses are ordered, oldest
    // first, and the counter makes a repeated generation number observable.
    let hex_prefix = "STRUCTURED-LAUNCH-ARGV-HEX:";
    let Some(hex_start) = generation_bytes.rfind(hex_prefix) else {
        return GenerationArgv::Error(format!(
            "session {session}'s generation {generation} never printed an unambiguous wrapper argv"
        ));
    };
    let mut lines = generation_bytes[hex_start..].lines();
    let first_line = lines
        .next()
        .and_then(|line| line.strip_prefix(hex_prefix))
        .unwrap_or("");
    // Attachment replay contains terminal rows, not the wrapper's
    // original byte stream. A narrow pane can fold this deliberately
    // unambiguous hex witness across physical lines; join only
    // wholly hexadecimal continuation rows, stopping before the
    // fake process's ordinary human-readable output. Blank rows are
    // filtered throughout the run, not just ahead of it: a replay
    // taken while the pipeline still fills the pane holds the prefix
    // row — and any already-rendered payload rows — followed by
    // not-yet-rendered rows, with the live remainder arriving after
    // them textually. Rows below the render point are always blank
    // here — these panes run no cursor-addressing programs, and the
    // launch output never fills the pane — so skipping blanks cannot
    // skip real content, and the hex run past them is this launch's
    // payload. Stopping at the first blank instead would decode a
    // snapshot cut mid-payload as the whole argv.
    let encoded = std::iter::once(first_line.trim())
        .chain(
            lines
                .by_ref()
                .filter(|line| !line.trim().is_empty())
                .take_while(|line| line.trim().bytes().all(|byte| byte.is_ascii_hexdigit()))
                .map(str::trim),
        )
        .collect::<String>();
    // An empty decode is a payload still in flight, never a bare launch:
    // every fixture launch execs with arguments, so only a non-empty
    // witness can complete this generation's boundary.
    if encoded.is_empty() {
        return GenerationArgv::Pending;
    }
    if encoded.len() % 2 != 0 {
        return GenerationArgv::Error("wrapper argv hex has partial bytes".to_string());
    }
    let bytes = (0..encoded.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&encoded[index..index + 2], 16))
        .collect::<Result<Vec<_>, _>>();
    let bytes = match bytes {
        Ok(bytes) => bytes,
        Err(_) => return GenerationArgv::Error("wrapper argv hex is invalid".to_string()),
    };
    // The wrapper NUL-terminates every argument (`printf '%s\0'`), so a
    // complete witness always ends in NUL. A join cut short of the
    // terminator is a truncated snapshot cut, not the generation's argv:
    // staying pending distinguishes the two where the even/odd length
    // check cannot.
    if bytes.last() != Some(&0) {
        return GenerationArgv::Pending;
    }
    let words = bytes
        .split(|byte| *byte == 0)
        .filter(|word| !word.is_empty())
        .map(std::str::from_utf8)
        .collect::<Result<Vec<_>, _>>();
    match words {
        Ok(words) => GenerationArgv::Ready(shell_words::join(words)),
        Err(_) => GenerationArgv::Error("wrapper argv is not UTF-8".to_string()),
    }
}

/// Assert the captured executable argv preserves every explicit choice.
///
/// This intentionally reads the fake process rather than reusing the
/// compiler's expected string. It is the process-boundary witness; compiler
/// unit tests own the exact vendor flag spellings and quoting grammar.
fn assert_forwarded(argv: &str, selection: &LaunchSelection) {
    let words = shell_words::split(argv).expect("fake executable printed shell-safe argv");
    if selection.harness == LaunchHarness::Grok {
        assert!(
            words.iter().any(|word| word == "--no-leader"),
            "Grok's tracked ownership flag must reach the executable: {words:?}"
        );
    }
    if let Some(model) = &selection.model {
        let model_flag = match selection.harness {
            LaunchHarness::Codex => "-m",
            LaunchHarness::Claude
            | LaunchHarness::Cursor
            | LaunchHarness::Muse
            | LaunchHarness::Goose
            | LaunchHarness::Pi
            | LaunchHarness::Omp
            | LaunchHarness::OpenCode => "--model",
            LaunchHarness::Grok => unreachable!("Grok has no supported model"),
        };
        assert!(
            words.windows(2).any(|pair| pair == [model_flag, model]),
            "model must remain one argv element: {words:?}"
        );
        // OMP's provider intent is explicit in every compiled launch; the
        // fixture mirrors it, so the forwarded argv carries the same pair.
        if selection.harness == LaunchHarness::Omp {
            assert!(
                words
                    .windows(2)
                    .any(|pair| pair == ["--provider", "openrouter"]),
                "OMP's provider qualification must reach the fake executable: {words:?}"
            );
        }
    }
    if let Some(effort) = selection.effort {
        let expected = match selection.harness {
            LaunchHarness::Codex => format!("model_reasoning_effort={}", effort.as_cli_arg()),
            LaunchHarness::Claude => effort.as_cli_arg().to_string(),
            LaunchHarness::Muse => effort.as_cli_arg().to_string(),
            LaunchHarness::Goose => return,
            LaunchHarness::Pi | LaunchHarness::Omp => effort.as_cli_arg().to_string(),
            LaunchHarness::OpenCode => unreachable!("OpenCode has no supported effort"),
            LaunchHarness::Cursor => unreachable!("Cursor has no supported effort"),
            LaunchHarness::Grok => unreachable!("Grok has no supported effort"),
        };
        let flag = match selection.harness {
            LaunchHarness::Codex => "-c",
            LaunchHarness::Claude => "--effort",
            LaunchHarness::Muse => "--reasoning-effort",
            LaunchHarness::Goose => return,
            LaunchHarness::Pi | LaunchHarness::Omp => "--thinking",
            LaunchHarness::OpenCode => unreachable!("OpenCode has no supported effort"),
            LaunchHarness::Cursor => unreachable!("Cursor has no supported effort"),
            LaunchHarness::Grok => unreachable!("Grok has no supported effort"),
        };
        assert!(
            words
                .windows(2)
                .any(|pair| pair == [flag, expected.as_str()]),
            "effort must reach the fake executable: {words:?}"
        );
    }
    // OMP's permission rides `--approval-mode` with a VALUE, so the pair —
    // not a bare flag — is the process-boundary witness; `always-ask` must
    // never be misread as yolo by a bare-flag check. An omitted permission
    // is the harness default and must contribute NO approval-mode option at
    // all, so absence is asserted rather than computed from `expected`.
    if selection.harness == LaunchHarness::Omp {
        let expected = match selection.permissions {
            Some(LaunchPermission::Yolo) => Some("yolo"),
            Some(LaunchPermission::Approve) => Some("always-ask"),
            _ => None,
        };
        let modes: Vec<&str> = words
            .windows(2)
            .filter(|pair| pair[0] == "--approval-mode")
            .map(|pair| pair[1].as_str())
            .collect();
        match expected {
            Some(value) => {
                assert_eq!(
                    modes,
                    vec![value],
                    "OMP's approval-mode pair must match the selection exactly: {words:?}"
                );
            }
            None => {
                assert!(
                    modes.is_empty(),
                    "an omitted OMP permission must add no approval-mode option: {words:?}"
                );
            }
        }
    }
    if selection.permissions == Some(LaunchPermission::Yolo) {
        let flag = match selection.harness {
            LaunchHarness::Codex | LaunchHarness::Muse => "--yolo",
            LaunchHarness::Claude => "--dangerously-skip-permissions",
            LaunchHarness::OpenCode => "--auto",
            LaunchHarness::Cursor => "--force",
            LaunchHarness::Grok => "--always-approve",
            LaunchHarness::Goose | LaunchHarness::Pi | LaunchHarness::Omp => return,
        };
        assert!(
            words.iter().any(|word| word == flag),
            "missing YOLO: {words:?}"
        );
    }
}

/// A structured choice remains one intent across process generations.
///
/// Defaults prove omission at the real executable; explicit selections prove
/// model, effort, and permission forwarding for every harness. The Codex
/// custom ID is shell-sensitive on purpose: a quote, semicolon, and dollar
/// sign distinguish argv preservation from a shell-interpolated command.
#[farhelm_testtrace::test]
async fn structured_launches_forward_to_ready_processes_and_survive_a_fresh_generation() {
    let h = harness().await;
    let fixture = fake_harness();
    let defaults = [
        LaunchHarness::Codex,
        LaunchHarness::Claude,
        LaunchHarness::Muse,
        LaunchHarness::Cursor,
        LaunchHarness::Grok,
    ];

    for harness in defaults {
        let selection = LaunchSelection {
            harness,
            model: None,
            effort: None,
            permissions: None,
            workspace_trust: None,
        };
        let created = launch(&h, &fixture, selection.clone()).await;
        assert_eq!(created.launch, Some(selection.clone()));
        let argv = observed_argv(&h, &created.id, 1).await;
        let words = shell_words::split(&argv).expect("default fake argv");
        assert!(
            !words.iter().any(|word| {
                matches!(
                    word.as_str(),
                    "--model" | "--effort" | "--yolo" | "--force" | "--always-approve"
                ) || word.starts_with("model_reasoning_effort=")
            }),
            "default selections must omit structured flags: {words:?}"
        );
    }

    let explicit = [
        LaunchSelection {
            harness: LaunchHarness::Cursor,
            model: Some("composer-2.5".to_string()),
            effort: None,
            permissions: Some(LaunchPermission::Yolo),
            workspace_trust: None,
        },
        LaunchSelection {
            harness: LaunchHarness::Grok,
            model: None,
            effort: None,
            permissions: Some(LaunchPermission::Yolo),
            workspace_trust: None,
        },
        LaunchSelection {
            harness: LaunchHarness::Codex,
            model: Some("release/candidate'42;$literal".to_string()),
            effort: Some(LaunchEffort::High),
            permissions: Some(LaunchPermission::Yolo),
            workspace_trust: None,
        },
        LaunchSelection {
            harness: LaunchHarness::Claude,
            model: Some("claude-fable-5".to_string()),
            effort: Some(LaunchEffort::High),
            permissions: Some(LaunchPermission::Yolo),
            workspace_trust: None,
        },
        LaunchSelection {
            harness: LaunchHarness::Muse,
            model: Some("muse-spark-1.3-contributor".to_string()),
            effort: Some(LaunchEffort::Xhigh),
            permissions: Some(LaunchPermission::Yolo),
            workspace_trust: None,
        },
        LaunchSelection {
            harness: LaunchHarness::OpenCode,
            model: Some("opencode/grok-4.6".to_string()),
            effort: None,
            permissions: Some(LaunchPermission::Yolo),
            workspace_trust: None,
        },
    ];

    let mut explicit_muse_id = None;
    let mut explicit_opencode_id = None;
    for selection in explicit {
        let created = launch(&h, &fixture, selection.clone()).await;
        assert_eq!(created.launch, Some(selection.clone()));
        // OpenCode appears only in the explicit cases. Its first wrapper
        // process is therefore generation 1, while the other explicit
        // cases follow a default launch of the same harness.
        let initial_generation = if selection.harness == LaunchHarness::OpenCode {
            1
        } else {
            2
        };
        assert_forwarded(
            &observed_argv(&h, &created.id, initial_generation).await,
            &selection,
        );

        let live = wait_for_live_status(&h.client, &created.id, 30).await;
        assert_eq!(live.launch, Some(selection.clone()));
        let stored = SessionStore::open(&h.state.path().join("supervisor.db"), false)
            .await
            .expect("reopen durable store")
            .session(&created.id)
            .await
            .expect("read durable session")
            .expect("created session remains stored");
        assert_eq!(stored.launch, Some(selection.clone()));

        // Cursor and the launch-only Grok kind retain launch choices across a
        // fresh process generation, but neither may invent conversation Resume.
        if matches!(
            selection.harness,
            LaunchHarness::Cursor | LaunchHarness::Grok
        ) {
            assert_eq!(live.restart_offer, farhelm_proto::RestartOffer::FreshOnly);
            let restarted = h
                .client
                .restart_session(&created.id, RestartMode::Fresh, true)
                .await
                .expect("fresh-restart launch-only fixture");
            assert_eq!(restarted.launch, Some(selection.clone()));
            assert_forwarded(&observed_argv(&h, &created.id, 3).await, &selection);
        }

        if selection.harness == LaunchHarness::Muse {
            explicit_muse_id = Some(created.id.clone());
            assert_eq!(
                live.restart_offer,
                farhelm_proto::RestartOffer::FreshOnly,
                "Muse remains Generic and fresh-only; this fixture must not invent resume support"
            );
        }
        if selection.harness == LaunchHarness::OpenCode {
            explicit_opencode_id = Some(created.id.clone());
            assert_eq!(
                live.restart_offer,
                farhelm_proto::RestartOffer::FreshOnly,
                "OpenCode remains Generic and fresh-only; this fixture must not invent resume support"
            );
        }
    }

    let explicit_muse_id = explicit_muse_id.expect("the explicit Muse case was created");
    let muse = h
        .client
        .list_sessions()
        .await
        .expect("list explicit sessions")
        .sessions
        .into_iter()
        .find(|session| session.id == explicit_muse_id)
        .expect("the explicitly created Muse session is listed");
    assert_eq!(muse.id, explicit_muse_id);
    let restarted = h
        .client
        .restart_session(&muse.id, RestartMode::Fresh, true)
        .await
        .expect("fresh-restart the ready Muse process");
    assert_eq!(restarted.launch, muse.launch);
    assert_forwarded(
        &observed_argv(&h, &muse.id, 3).await,
        muse.launch.as_ref().unwrap(),
    );

    // OpenCode carries a structured selection through the same persistence
    // path, but it deliberately has no conversation identity to resume.
    let opencode_id = explicit_opencode_id.expect("the explicit OpenCode case was created");
    let opencode = h
        .client
        .list_sessions()
        .await
        .expect("list explicit sessions")
        .sessions
        .into_iter()
        .find(|session| session.id == opencode_id)
        .expect("the explicitly created OpenCode session is listed");
    let restarted = h
        .client
        .restart_session(&opencode.id, RestartMode::Fresh, true)
        .await
        .expect("fresh-restart the ready OpenCode process");
    assert_eq!(restarted.launch, opencode.launch);
    assert_forwarded(
        &observed_argv(&h, &opencode.id, 2).await,
        opencode.launch.as_ref().unwrap(),
    );
}

/// Explicit inheritance preserves a structured parent's frozen bundle.
///
/// The restricted wire path cannot name a new structured selection: its only
/// authority is the authenticated parent. This uses that actual path, then
/// reads the child process and store rather than treating the request body as
/// evidence that inheritance survived admission and launch.
#[farhelm_testtrace::test]
async fn explicit_spawn_inheritance_preserves_a_structured_parent_at_the_process_boundary() {
    let h = harness().await;
    let fixture = fake_harness();
    let selection = LaunchSelection {
        harness: LaunchHarness::Codex,
        model: Some("gpt-6-astra".to_string()),
        effort: Some(LaunchEffort::High),
        permissions: Some(LaunchPermission::Yolo),
        workspace_trust: None,
    };
    let parent = launch(&h, &fixture, selection.clone()).await;
    assert_forwarded(&observed_argv(&h, &parent.id, 1).await, &selection);

    let token = credential_for(&h, &parent.id).await;
    let mut peer = SessionPeer::connect(&h.sup, &parent.id, &token).await;
    let reply = peer
        .control(ControlMsg::CreateSession {
            req_id: 1,
            parent: Some(parent.id.clone()),
            cwd: fixture.work.path().to_string_lossy().into_owned(),
            invocation: None,
            profile_name: None,
            profile_id: None,
            inherit_agent: true,
            title: Some("structured child".to_string()),
            cols: WIDE_COLS,
            rows: ROWS,
            intent_key: Some("structured-inherited-child".to_string()),
            agent_kind: None,
            resume_template: None,
            source_profile: None,
            github_checkout: None,
            launch: None,
        })
        .await;
    let ControlMsg::SessionCreated {
        req_id: 1,
        session: child,
    } = reply
    else {
        panic!("explicit structured inheritance must create a child: {reply:?}");
    };
    assert_eq!(child.launch, Some(selection.clone()));
    assert_forwarded(&observed_argv(&h, &child.id, 2).await, &selection);

    let live = wait_for_live_status(&h.client, &child.id, 30).await;
    assert_eq!(live.launch, Some(selection.clone()));
    let stored = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("reopen durable store")
        .session(&child.id)
        .await
        .expect("read durable child")
        .expect("inherited child remains stored");
    assert_eq!(stored.launch, Some(selection));
}

/// Restricted raw launch data is refused, while a profile selector may
/// replace structured-parent inheritance through the attached helm.
///
/// A session credential can choose inheritance or ask the helm to resolve a
/// profile. It cannot declare that arbitrary invocation metadata is trusted.
/// The refusal must leave no child, and the accepted profile path must clear
/// the parent's structured snapshot in every projection.
#[farhelm_testtrace::test]
async fn restricted_raw_data_is_refused_and_profile_override_clears_structured_metadata() {
    let h = harness().await;
    let fixture = fake_harness();
    let selection = LaunchSelection {
        harness: LaunchHarness::Codex,
        model: Some("gpt-6-astra".to_string()),
        effort: Some(LaunchEffort::High),
        permissions: Some(LaunchPermission::Yolo),
        workspace_trust: None,
    };
    let parent = launch(&h, &fixture, selection.clone()).await;
    assert_forwarded(&observed_argv(&h, &parent.id, 1).await, &selection);
    let token = credential_for(&h, &parent.id).await;
    let mut peer = SessionPeer::connect(&h.sup, &parent.id, &token).await;

    let raw_reply = peer
        .control(ControlMsg::CreateSession {
            req_id: 1,
            parent: Some(parent.id.clone()),
            cwd: fixture.work.path().to_string_lossy().into_owned(),
            invocation: Some(fixture.invocation(&selection)),
            profile_name: None,
            profile_id: None,
            inherit_agent: false,
            title: Some("raw override".to_string()),
            cols: WIDE_COLS,
            rows: ROWS,
            intent_key: Some("structured-parent-raw-override".to_string()),
            agent_kind: Some(AgentKind::Codex),
            resume_template: None,
            source_profile: None,
            launch: None,
            github_checkout: None,
        })
        .await;
    let ControlMsg::Error {
        req_id: 1,
        kind: farhelm_proto::ErrorKind::InvalidRequest,
        message,
    } = raw_reply
    else {
        panic!("raw restricted data must be refused: {raw_reply:?}");
    };
    assert!(message.contains("exactly one profile name, profile id, or explicit inheritance"));
    let sessions = h
        .client
        .list_sessions()
        .await
        .expect("list after raw refusal")
        .sessions;
    assert_eq!(sessions.len(), 1, "a refused raw request creates no child");
    assert_eq!(sessions[0].id, parent.id);

    let profile = farhelm_proto::ProfileSnapshot {
        id: "structured-override-profile".to_string(),
        name: "Structured override profile".to_string(),
    };
    let helm = connect_helm(
        &h.sup,
        ScriptedHandler::answering(AgentReply::ResolvedProfile {
            invocation: fixture.invocation(&selection),
            agent_kind: AgentKind::Codex,
            resume_template: None,
            source_profile: profile,
        }),
    )
    .await;
    let (_channel, _replay, _stream) = helm
        .attach_live(&parent.id, WIDE_COLS, ROWS)
        .await
        .expect("attach the profile-resolving helm to the structured parent");
    let profile_reply = peer
        .control(ControlMsg::CreateSession {
            req_id: 2,
            parent: Some(parent.id),
            cwd: fixture.work.path().to_string_lossy().into_owned(),
            invocation: None,
            profile_name: Some("Structured override profile".to_string()),
            profile_id: None,
            inherit_agent: false,
            title: Some("profile override".to_string()),
            cols: WIDE_COLS,
            rows: ROWS,
            intent_key: Some("structured-parent-profile-override".to_string()),
            agent_kind: None,
            resume_template: None,
            source_profile: None,
            launch: None,
            github_checkout: None,
        })
        .await;
    let ControlMsg::SessionCreated {
        session: profile_child,
        ..
    } = profile_reply
    else {
        panic!("profile override must create a child: {profile_reply:?}");
    };
    assert_eq!(profile_child.launch, None);
    // The refused raw request never executes the wrapper, so the profile
    // child is the second actual launch after the parent.
    assert_forwarded(&observed_argv(&h, &profile_child.id, 2).await, &selection);
    let profile_live = wait_for_live_status(&h.client, &profile_child.id, 30).await;
    assert_eq!(profile_live.launch, None);
    let profile_stored = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("reopen durable store")
        .session(&profile_child.id)
        .await
        .expect("read profile override")
        .expect("profile override remains stored");
    assert_eq!(profile_stored.launch, None);
}

#[cfg(test)]
mod decoder_tests {
    use super::*;

    /// Encode argv words the way the fixture wrapper's `od` pipeline does,
    /// including its trailing NUL: the wrapper prints `printf '%s\0' "$@"`,
    /// so every argument — including the last — is NUL-terminated, and the
    /// decoder requires that terminator as its completeness witness.
    ///
    /// Building the witness from words instead of pasting hex keeps the
    /// regression transcripts readable and proves the expectation against the
    /// same NUL-joined grammar the decoder parses.
    fn hex_of(words: &[&str]) -> String {
        let mut joined = words.join("\0");
        joined.push('\0');
        joined.bytes().map(|byte| format!("{byte:02x}")).collect()
    }

    /// A stale payload-less prefix row must not satisfy the generation wait.
    ///
    /// The wrapper prints its `STRUCTURED-LAUNCH-ARGV-HEX:` prefix in one
    /// write and the pipeline's hex payload in a later one, so an attach
    /// replay taken between the two holds a bare prefix row followed by
    /// stale rows. Once the live stream delivers the fake's argv and ready
    /// markers, the decoder's presence gates pass; accepting the stale row
    /// then returns an empty argv for a launch that actually forwarded its
    /// model. This is the `agent_listing_real_stack` structured-successor
    /// flake: the wait must stay pending until the payload arrives.
    #[test]
    fn a_bare_prefix_row_is_pending_until_its_payload_arrives() {
        let transcript = b"shell-prompt$ \nSTRUCTURED-LAUNCH-GENERATION:1\nSTRUCTURED-LAUNCH-ARGV-HEX:\nFAKE-AGENT ARGV:/bin/fake\nFAKE-AGENT READY\n";
        assert!(matches!(
            decode_generation_argv(transcript, "session", 1),
            GenerationArgv::Pending
        ));
    }

    /// A stale prefix row must not shadow the live complete witness.
    ///
    /// The observation buffer is attach replay followed by live bytes, so a
    /// mid-render replay's bare prefix row sorts before the live stream's
    /// complete witness for the same generation. Decoding the first
    /// candidate accepts the stale empty row even though the complete argv
    /// is already in the buffer; the generation's argv is its most recent
    /// witness, matching how the generation marker itself resolves to its
    /// last occurrence.
    #[test]
    fn a_stale_prefix_row_does_not_shadow_the_complete_witness() {
        let words = ["codex", "internal", "fake-agent", "-m", "gpt-6-astra"];
        let transcript = format!(
            "shell-prompt$ \nSTRUCTURED-LAUNCH-GENERATION:1\nSTRUCTURED-LAUNCH-ARGV-HEX:\nshell-prompt$ \nSTRUCTURED-LAUNCH-ARGV-HEX:{}\nfake-agent starting\nFAKE-AGENT ARGV:/bin/fake -m gpt-6-astra\nFAKE-AGENT READY\n",
            hex_of(&words)
        );
        match decode_generation_argv(transcript.as_bytes(), "session", 1) {
            GenerationArgv::Ready(argv) => assert_eq!(argv, shell_words::join(words)),
            other => panic!("stale prefix row must not win: {other:?}"),
        }
    }

    /// A payload past not-yet-rendered rows still joins its prefix.
    ///
    /// When the replay catches the pane between the wrapper's prefix write
    /// and its pipeline's payload, the prefix row is followed by blank
    /// rows the render point has not reached, with the live payload
    /// arriving after them textually. Stopping the join at the first
    /// blank row strands the payload in the buffer: the wait then sits
    /// pending until its deadline even though the launch completed long
    /// ago. The join must cross those blank rows to the payload.
    #[test]
    fn a_payload_across_blank_rows_still_joins_its_prefix() {
        let words = ["codex", "internal", "fake-agent", "-m", "gpt-6-astra"];
        let transcript = format!(
            "STRUCTURED-LAUNCH-GENERATION:2\nSTRUCTURED-LAUNCH-ARGV-HEX:  \n\n\n\n{}\nfake-agent starting\nFAKE-AGENT ARGV:/bin/fake -m gpt-6-astra\nFAKE-AGENT READY\n",
            hex_of(&words)
        );
        match decode_generation_argv(transcript.as_bytes(), "session", 2) {
            GenerationArgv::Ready(argv) => assert_eq!(argv, shell_words::join(words)),
            other => panic!("payload across blank rows must decode: {other:?}"),
        }
    }

    /// A witness row wearing terminal escapes still decodes.
    ///
    /// Snapshot rows serialize with cursor-positioning escapes, width
    /// padding, and carriage returns, and the fake colors its own output:
    /// the observed buffer can hold `\x1b[2;28H` ahead of the hex payload
    /// and color sequences inside the fake's startup lines. The hex join
    /// must read through those sequences — a payload the join cannot see
    /// strands the wait until its deadline even though the launch is
    /// complete. This is the retained generation-2 timeout shape: the
    /// failure transcript carries the full boundary, escapes included.
    #[test]
    fn a_witness_row_with_terminal_escapes_still_decodes() {
        let words = ["codex", "internal", "fake-agent", "-m", "gpt-6-astra"];
        let transcript = format!(
            "STRUCTURED-LAUNCH-GENERATION:2  \r\nSTRUCTURED-LAUNCH-ARGV-HEX:{pad}  \r\n\r\n\r\n\x1b[2;28H{hex}\r\n\x1b[?2004h\x1b[1;32mfake-agent\x1b[0m starting (script=record)\r\r\nFAKE-AGENT ARGV:/bin/fake -m gpt-6-astra\r\r\nFAKE-AGENT READY\r\r\n> ",
            pad = "                                                                                               ",
            hex = hex_of(&words)
        );
        match decode_generation_argv(transcript.as_bytes(), "session", 2) {
            GenerationArgv::Ready(argv) => assert_eq!(argv, shell_words::join(words)),
            other => panic!("escaped witness row must decode: {other:?}"),
        }
    }

    /// A snapshot cut mid-payload joins through blank rows to the live rest.
    ///
    /// The blank filter must apply throughout the hex run, not just ahead
    /// of it: a replay taken halfway through the payload holds rendered
    /// payload rows, then blank not-yet-rendered rows, then the live
    /// remainder. Stopping the join at the first blank decodes the
    /// retained cut as the whole argv — `Ready("in")` for an `internal`
    /// launch — or errors on an odd cut. The full argv, terminator
    /// included, is the only acceptable decode.
    #[test]
    fn a_snapshot_cut_mid_payload_joins_through_blank_rows() {
        let words = ["codex", "internal", "fake-agent"];
        let full = hex_of(&words);
        let (cut, rest) = full.split_at(6);
        let (first, second) = cut.split_at(4);
        let transcript = format!(
            "STRUCTURED-LAUNCH-GENERATION:1\nSTRUCTURED-LAUNCH-ARGV-HEX:{first}\n{second}\n\n\n{rest}\nfake-agent starting\nFAKE-AGENT ARGV:/bin/fake internal\nFAKE-AGENT READY\n"
        );
        match decode_generation_argv(transcript.as_bytes(), "session", 1) {
            GenerationArgv::Ready(argv) => assert_eq!(argv, shell_words::join(words)),
            other => panic!("cut payload must join to the full argv: {other:?}"),
        }
    }

    /// A witness without its NUL terminator is still in flight.
    ///
    /// The wrapper NUL-terminates every argument, so a decoded payload
    /// that does not end in NUL is a truncated snapshot cut even when
    /// its length is even. Accepting it would return a silently
    /// shortened argv; staying pending waits for the remainder (or
    /// fails loudly at the wait's deadline with the transcript).
    #[test]
    fn a_witness_missing_its_terminator_stays_pending() {
        let transcript = b"STRUCTURED-LAUNCH-GENERATION:1\nSTRUCTURED-LAUNCH-ARGV-HEX:666f6f\nFAKE-AGENT ARGV:/bin/fake\nFAKE-AGENT READY\n";
        assert!(matches!(
            decode_generation_argv(transcript, "session", 1),
            GenerationArgv::Pending
        ));
    }

    /// A corrupted witness stays an error, not a guess.
    ///
    /// Pins the defensive arm: an odd-length hex run cannot split into
    /// bytes, so the decoder must say so rather than pending forever or
    /// decoding a prefix of it.
    #[test]
    fn an_odd_length_witness_is_an_error() {
        let transcript = b"STRUCTURED-LAUNCH-GENERATION:1\nSTRUCTURED-LAUNCH-ARGV-HEX:2d6\nFAKE-AGENT ARGV:/bin/fake\nFAKE-AGENT READY\n";
        assert!(matches!(
            decode_generation_argv(transcript, "session", 1),
            GenerationArgv::Error(_)
        ));
    }

    /// Each fake-side marker gate is necessary on its own.
    ///
    /// The generation wait requires both the fake's argv line and its
    /// ready line; pin that neither gate alone suffices, so losing
    /// either requirement in the disjunction fails loudly here instead
    /// of silently weakening the boundary.
    #[test]
    fn each_fake_marker_gate_is_necessary() {
        let words = ["codex", "internal"];
        let hex = hex_of(&words);
        let without_ready = format!(
            "STRUCTURED-LAUNCH-GENERATION:1\nSTRUCTURED-LAUNCH-ARGV-HEX:{hex}\nFAKE-AGENT ARGV:/bin/fake\n"
        );
        assert!(matches!(
            decode_generation_argv(without_ready.as_bytes(), "session", 1),
            GenerationArgv::Pending
        ));
        let without_argv = format!(
            "STRUCTURED-LAUNCH-GENERATION:1\nSTRUCTURED-LAUNCH-ARGV-HEX:{hex}\nFAKE-AGENT READY\n"
        );
        assert!(matches!(
            decode_generation_argv(without_argv.as_bytes(), "session", 1),
            GenerationArgv::Pending
        ));
    }

    /// A hex witness folded across terminal rows still decodes as one argv.
    ///
    /// The attach replay carries terminal rows rather than the wrapper's
    /// byte stream, so a narrow pane folds the long hex line across
    /// physical rows. Switching the decoder to the most recent witness
    /// must preserve that join: the continuation rows are part of the
    /// same boundary, not a second candidate.
    #[test]
    fn a_wrapped_hex_witness_still_joins_across_rows() {
        let words = ["codex", "internal", "fake-agent", "-m", "gpt-6-astra"];
        let hex = hex_of(&words);
        let (first, second) = hex.split_at(hex.len() / 2);
        let transcript = format!(
            "STRUCTURED-LAUNCH-GENERATION:1\nSTRUCTURED-LAUNCH-ARGV-HEX:{first}\n{second}\nfake-agent starting\nFAKE-AGENT ARGV:/bin/fake -m gpt-6-astra\nFAKE-AGENT READY\n"
        );
        match decode_generation_argv(transcript.as_bytes(), "session", 1) {
            GenerationArgv::Ready(argv) => assert_eq!(argv, shell_words::join(words)),
            other => panic!("wrapped witness must decode: {other:?}"),
        }
    }

    /// The presence gates stay pending until the full boundary arrives.
    ///
    /// Pinning the incomplete shapes guards the fix against collapsing
    /// them into errors: a missing generation, or a generation without
    /// the fake's argv and ready markers yet, is a launch still in
    /// flight, not a broken witness.
    #[test]
    fn an_incomplete_boundary_stays_pending() {
        assert!(matches!(
            decode_generation_argv(b"shell-prompt$ \n", "session", 1),
            GenerationArgv::Pending
        ));
        assert!(matches!(
            decode_generation_argv(
                b"STRUCTURED-LAUNCH-GENERATION:1\nSTRUCTURED-LAUNCH-ARGV-HEX:2d6d\n",
                "session",
                1
            ),
            GenerationArgv::Pending
        ));
    }
    /// The OMP approval-mode witness must reject every wrong shape, not only
    /// accept the right one (review finding 1): an omitted permission means
    /// NO approval-mode option at all, an explicit mode means exactly one
    /// pair with the selection's value, and a conflicting second pair is a
    /// failure even when a correct pair is also present.
    #[test]
    fn omp_approval_mode_witness_rejects_wrong_shapes() {
        let base = LaunchSelection {
            harness: LaunchHarness::Omp,
            model: Some("z-ai/glm-5.3".to_string()),
            effort: None,
            permissions: None,
            workspace_trust: None,
        };

        // Omitted permission: no approval-mode option may exist.
        assert_forwarded("omp --provider openrouter --model z-ai/glm-5.3", &base);

        // Approve: exactly the always-ask pair.
        let approve = LaunchSelection {
            permissions: Some(LaunchPermission::Approve),
            workspace_trust: None,
            ..base.clone()
        };
        assert_forwarded(
            "omp --provider openrouter --model z-ai/glm-5.3 --approval-mode always-ask",
            &approve,
        );

        // YOLO: exactly the yolo pair.
        let yolo = LaunchSelection {
            permissions: Some(LaunchPermission::Yolo),
            workspace_trust: None,
            ..base.clone()
        };
        assert_forwarded(
            "omp --provider openrouter --model z-ai/glm-5.3 --approval-mode yolo",
            &yolo,
        );

        // A deliberately wrong mode must fail the Approve selection.
        let wrong = "omp --provider openrouter --model z-ai/glm-5.3 --approval-mode yolo";
        let panicked = std::panic::catch_unwind(|| assert_forwarded(wrong, &approve));
        assert!(
            panicked.is_err(),
            "a yolo pair must not satisfy an Approve selection"
        );

        // An unexpected approval-mode option must fail the omitted case.
        let unexpected =
            "omp --provider openrouter --model z-ai/glm-5.3 --approval-mode always-ask";
        let panicked = std::panic::catch_unwind(|| assert_forwarded(unexpected, &base));
        assert!(
            panicked.is_err(),
            "an omitted permission must reject any approval-mode option"
        );

        // A conflicting second pair must fail even with a correct pair present.
        let conflicting = "omp --provider openrouter --model z-ai/glm-5.3 --approval-mode always-ask --approval-mode yolo";
        let panicked = std::panic::catch_unwind(|| assert_forwarded(conflicting, &approve));
        assert!(
            panicked.is_err(),
            "conflicting approval-mode pairs must fail"
        );
    }
}

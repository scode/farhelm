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
        if let Some(model) = &selection.model {
            match selection.harness {
                LaunchHarness::Codex => argv.extend(["-m".to_string(), model.clone()]),
                LaunchHarness::Claude | LaunchHarness::Muse => {
                    argv.extend(["--model".to_string(), model.clone()])
                }
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
            }
        }
        if selection.permissions == Some(LaunchPermission::Yolo) {
            argv.push(
                match selection.harness {
                    LaunchHarness::Codex | LaunchHarness::Muse => "--yolo",
                    LaunchHarness::Claude => "--dangerously-skip-permissions",
                }
                .to_string(),
            );
        }
        shell_words::join(argv)
    }
}

/// Create the three named fixture entry points without changing PATH.
///
/// Each wrapper emits a monotonic, per-harness generation before handing its
/// original argv to the shipped fake-agent binary. A restart can retain old
/// terminal scrollback, so a plain readiness marker would let a predecessor
/// satisfy a successor assertion. The wrapper's counter makes that mistake
/// observable without changing the test process environment.
pub(crate) fn fake_harness() -> FakeHarness {
    let bin = farhelm_teststate::tempdir().expect("fixture executable directory");
    let home = farhelm_teststate::tempdir().expect("structured launch agent home");
    for name in ["codex", "claude", "muse"] {
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
        // Muse deliberately remains a Generic runtime integration: it has no
        // conversation-resume contract for this release.
        LaunchHarness::Muse => AgentKind::Generic,
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
enum GenerationArgv {
    Pending,
    Ready(String),
    Error(String),
}

/// Wait for a complete wrapper/fake-agent successor boundary on one attachment.
///
/// Replay remains context, but a predecessor READY cannot satisfy this wait:
/// the requested generation must contain its own wrapper argv, fake argv, and
/// post-exec READY before the decoder accepts it.
async fn wait_for_generation_argv(
    stream: &mut TermStream,
    observed: &mut Vec<u8>,
    session: &str,
    generation: u32,
) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        match decode_generation_argv(&String::from_utf8_lossy(observed), session, generation) {
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
fn decode_generation_argv(transcript: &str, session: &str, generation: u32) -> GenerationArgv {
    let marker = format!("STRUCTURED-LAUNCH-GENERATION:{generation}");
    let Some(start) = transcript.rfind(&marker) else {
        return GenerationArgv::Pending;
    };
    let generation_bytes = &transcript[start..];
    if !generation_bytes.contains(ARGV_MARKER) || !generation_bytes.contains(AGENT_READY_MARKER) {
        return GenerationArgv::Pending;
    }
    let mut lines = generation_bytes.lines();
    let encoded = lines
        .find_map(|line| line.strip_prefix("STRUCTURED-LAUNCH-ARGV-HEX:"))
        .map(|first_line| {
            // Attachment replay contains terminal rows, not the wrapper's
            // original byte stream. A narrow pane can fold this deliberately
            // unambiguous hex witness across physical lines; join only
            // wholly hexadecimal continuation rows, stopping before the
            // fake process's ordinary human-readable output.
            std::iter::once(first_line.trim())
                .chain(lines.take_while(|line| {
                    let line = line.trim();
                    !line.is_empty() && line.bytes().all(|byte| byte.is_ascii_hexdigit())
                }).map(str::trim))
                .collect::<String>()
        })
        .ok_or_else(|| format!("session {session}'s generation {generation} never printed an unambiguous wrapper argv"));
    let encoded = match encoded {
        Ok(encoded) => encoded,
        Err(error) => return GenerationArgv::Error(error),
    };
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
    if let Some(model) = &selection.model {
        let model_flag = match selection.harness {
            LaunchHarness::Codex => "-m",
            LaunchHarness::Claude | LaunchHarness::Muse => "--model",
        };
        assert!(
            words.windows(2).any(|pair| pair == [model_flag, model]),
            "model must remain one argv element: {words:?}"
        );
    }
    if let Some(effort) = selection.effort {
        let expected = match selection.harness {
            LaunchHarness::Codex => format!("model_reasoning_effort={}", effort.as_cli_arg()),
            LaunchHarness::Claude => effort.as_cli_arg().to_string(),
            LaunchHarness::Muse => effort.as_cli_arg().to_string(),
        };
        let flag = match selection.harness {
            LaunchHarness::Codex => "-c",
            LaunchHarness::Claude => "--effort",
            LaunchHarness::Muse => "--reasoning-effort",
        };
        assert!(
            words
                .windows(2)
                .any(|pair| pair == [flag, expected.as_str()]),
            "effort must reach the fake executable: {words:?}"
        );
    }
    if selection.permissions == Some(LaunchPermission::Yolo) {
        let flag = match selection.harness {
            LaunchHarness::Codex | LaunchHarness::Muse => "--yolo",
            LaunchHarness::Claude => "--dangerously-skip-permissions",
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
    ];

    for harness in defaults {
        let selection = LaunchSelection {
            harness,
            model: None,
            effort: None,
            permissions: None,
        };
        let created = launch(&h, &fixture, selection.clone()).await;
        assert_eq!(created.launch, Some(selection.clone()));
        let argv = observed_argv(&h, &created.id, 1).await;
        let words = shell_words::split(&argv).expect("default fake argv");
        assert!(
            !words.iter().any(|word| {
                matches!(word.as_str(), "--model" | "--effort" | "--yolo")
                    || word.starts_with("model_reasoning_effort=")
            }),
            "default selections must omit structured flags: {words:?}"
        );
    }

    let explicit = [
        LaunchSelection {
            harness: LaunchHarness::Codex,
            model: Some("release/candidate'42;$literal".to_string()),
            effort: Some(LaunchEffort::High),
            permissions: Some(LaunchPermission::Yolo),
        },
        LaunchSelection {
            harness: LaunchHarness::Claude,
            model: Some("claude-fable-5".to_string()),
            effort: Some(LaunchEffort::High),
            permissions: Some(LaunchPermission::Yolo),
        },
        LaunchSelection {
            harness: LaunchHarness::Muse,
            model: Some("muse-spark-1.3-contributor".to_string()),
            effort: Some(LaunchEffort::Xhigh),
            permissions: Some(LaunchPermission::Yolo),
        },
    ];

    let mut explicit_muse_id = None;
    for selection in explicit {
        let created = launch(&h, &fixture, selection.clone()).await;
        assert_eq!(created.launch, Some(selection.clone()));
        assert_forwarded(&observed_argv(&h, &created.id, 2).await, &selection);

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

        if selection.harness == LaunchHarness::Muse {
            explicit_muse_id = Some(created.id.clone());
            assert_eq!(
                live.restart_offer,
                farhelm_proto::RestartOffer::FreshOnly,
                "Muse remains Generic and fresh-only; this fixture must not invent resume support"
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
}

/// A session-authenticated selectorless spawn inherits a structured bundle.
///
/// The restricted wire path cannot name a new structured selection: its only
/// authority is the authenticated parent. This uses that actual path, then
/// reads the child process and store rather than treating the request body as
/// evidence that inheritance survived admission and launch.
#[farhelm_testtrace::test]
async fn selectorless_spawn_inherits_a_structured_parent_at_the_process_boundary() {
    let h = harness().await;
    let fixture = fake_harness();
    let selection = LaunchSelection {
        harness: LaunchHarness::Codex,
        model: Some("gpt-6-astra".to_string()),
        effort: Some(LaunchEffort::High),
        permissions: Some(LaunchPermission::Yolo),
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
            title: Some("structured child".to_string()),
            cols: WIDE_COLS,
            rows: ROWS,
            intent_key: Some("structured-selectorless-child".to_string()),
            agent_kind: None,
            resume_template: None,
            source_profile: None,
            launch: None,
        })
        .await;
    let ControlMsg::SessionCreated {
        req_id: 1,
        session: child,
    } = reply
    else {
        panic!("selectorless structured spawn must create a child: {reply:?}");
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
        .expect("selectorless child remains stored");
    assert_eq!(stored.launch, Some(selection));
}

/// Explicit raw and profile selectors supersede structured-parent inheritance.
///
/// A session credential grants reuse of its parent only when the child names
/// no selector. These two ready children prove that an intentional override
/// takes the separate admission path and cannot leave a stale structured
/// snapshot in any of the reply, live, or durable projections.
#[farhelm_testtrace::test]
async fn explicit_overrides_clear_a_structured_parents_metadata_before_launch() {
    let h = harness().await;
    let fixture = fake_harness();
    let selection = LaunchSelection {
        harness: LaunchHarness::Codex,
        model: Some("gpt-6-astra".to_string()),
        effort: Some(LaunchEffort::High),
        permissions: Some(LaunchPermission::Yolo),
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
            title: Some("raw override".to_string()),
            cols: WIDE_COLS,
            rows: ROWS,
            intent_key: Some("structured-parent-raw-override".to_string()),
            agent_kind: Some(AgentKind::Codex),
            resume_template: None,
            source_profile: None,
            launch: None,
        })
        .await;
    let ControlMsg::SessionCreated { session: raw, .. } = raw_reply else {
        panic!("raw override must create a child: {raw_reply:?}");
    };
    assert_eq!(raw.launch, None);
    assert_forwarded(&observed_argv(&h, &raw.id, 2).await, &selection);
    let raw_live = wait_for_live_status(&h.client, &raw.id, 30).await;
    assert_eq!(raw_live.launch, None);
    let raw_stored = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("reopen durable store")
        .session(&raw.id)
        .await
        .expect("read raw override")
        .expect("raw override remains stored");
    assert_eq!(raw_stored.launch, None);

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
            title: Some("profile override".to_string()),
            cols: WIDE_COLS,
            rows: ROWS,
            intent_key: Some("structured-parent-profile-override".to_string()),
            agent_kind: None,
            resume_template: None,
            source_profile: None,
            launch: None,
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
    assert_forwarded(&observed_argv(&h, &profile_child.id, 3).await, &selection);
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

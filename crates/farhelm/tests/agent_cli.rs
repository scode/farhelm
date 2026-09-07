//! Process-level contract tests for `farhelm agent`.
//!
//! `spawn_cli.rs`'s sibling, and written the same way and for the same
//! reason: driving the built binary is what tests stdout, stderr, exit
//! status, environment validation, and the exact bytes on the wire
//! together. Nothing here stands up a helm or a supervisor — the mock
//! answers the frame the CLI sent — because what these tests own is the
//! CLI's half of the contract: what it asks for, and what it prints when
//! it is answered.
//!
//! Child-only environment changes keep the test runner safe for parallel
//! execution; this repo's tests never mutate their own process's
//! environment.

use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
use farhelm_proto::{
    AgentHost, AgentOutcome, AgentReply, AgentSession, AgentVerb, ControlMsg, ErrorKind, Frame,
};
use std::process::{Command, Output};
use std::time::Duration;

/// Serve exactly one authenticated request, asserting the handshake this
/// CLI is required to perform, and answer with whatever `respond` returns.
///
/// The handshake assertions live in the SERVER rather than in each test
/// because they are invariant across every verb: any `farhelm agent`
/// invocation must authenticate as the injected session before it is
/// allowed to ask anything, and a change that dropped the credential would
/// otherwise show up as a confusing failure in whichever test happened to
/// run first.
///
/// `respond` returning `None` closes the connection without answering,
/// which is a real ending rather than a test convenience: a supervisor can
/// die between reading a request and writing its reply, and the CLI blocks
/// with no deadline of its own, so "the peer went away" has to be a failure
/// rather than a hang.
fn mock_supervisor(
    socket: &std::path::Path,
    respond: impl FnOnce(ControlMsg) -> Option<ControlMsg> + Send + 'static,
) -> (
    std::sync::mpsc::Receiver<Result<(), String>>,
    farhelm_teststate::thread::FixtureThread,
) {
    mock_supervisor_frames(socket, |request| {
        respond(request).map(|reply| Frame::control(&reply))
    })
}

/// [`mock_supervisor`] one layer down, answering with a raw [`Frame`].
///
/// Exists for the one case a `ControlMsg` cannot express: a control frame
/// whose body is not decodable at all. That ending is reachable in
/// production from any peer with a version skew or a bug, and it is on the
/// far side of the CLI's write, so it belongs to the outcome-unknown family
/// — which nothing else here could stage.
///
/// The returned owner cancels the entire exchange during assertion unwind,
/// including a blocked handshake or reply write. The six-second transaction
/// allowance leaves room inside finish_server's seven-second result wait;
/// that result is checked before observing the worker's actual join.
fn mock_supervisor_frames(
    socket: &std::path::Path,
    respond: impl FnOnce(ControlMsg) -> Option<Frame> + Send + 'static,
) -> (
    std::sync::mpsc::Receiver<Result<(), String>>,
    farhelm_teststate::thread::FixtureThread,
) {
    let std_listener = std::os::unix::net::UnixListener::bind(socket).expect("bind socket");
    std_listener.set_nonblocking(true).unwrap();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
    // The mock leaves libtest's thread, so it must carry the test's capture
    // through its runtime and back through runtime teardown.
    let context = farhelm_testtrace::current_thread_context().expect("test trace context");
    let thread = std::thread::spawn(move || {
        context.enter(|| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                context
                    .with_runtime(
                        farhelm_testtrace::RuntimeConfig {
                            flavor: farhelm_testtrace::RuntimeFlavor::MultiThread,
                            worker_threads: None,
                            start_paused: false,
                        },
                        |runtime| {
                            runtime.block_on(async move {
                                // Cancellation and the aggregate deadline cover handshake and
                                // response writes too, including peers that stop draining output.
                                let exchange = async move {
                                    let listener =
                                        tokio::net::UnixListener::from_std(std_listener).unwrap();
                                    let (stream, _) =
                                        tokio::time::timeout(Duration::from_secs(5), listener.accept())
                                            .await
                                            .expect("the agent CLI did not connect")
                                            .expect("accept the agent CLI");
                                    let (read, write) = tokio::io::split(stream);
                                    let mut reader = FrameReader::new(read);
                                    let mut writer = FrameWriter::new(write);
                                    let hello = handshake(&mut reader, &mut writer, "supervisor")
                                        .await
                                        .expect("handshake");
                                    let ControlMsg::Hello {
                                        auth: Some(auth), ..
                                    } = hello
                                    else {
                                        panic!(
                                            "the agent CLI must authenticate in its hello: {hello:?}"
                                        );
                                    };
                                    assert_eq!(auth.session_id, "session-1");
                                    assert_eq!(auth.token, "secret");
                                    let frame = tokio::time::timeout(
                                        Duration::from_secs(5),
                                        reader.read_frame(),
                                    )
                                    .await
                                    .expect("the agent CLI did not send a request")
                                    .unwrap()
                                    .expect("agent request");
                                    if let Some(reply) = respond(parse_control(&frame).unwrap()) {
                                        writer.write_frame(&reply).await.unwrap();
                                    }
                                };
                                tokio::select! {
                                    _ = cancel_rx => Err("mock supervisor cancelled".to_string()),
                                    result = tokio::time::timeout(Duration::from_secs(6), exchange) => {
                                        result.map_err(|_| "mock supervisor exchange exceeded six seconds".to_string())
                                    }
                                }
                            })
                        },
                    )
                    .unwrap()
            }))
            .map_err(|panic| {
                panic.downcast_ref::<&str>().map_or_else(
                    || "mock supervisor panicked".to_string(),
                    |s| (*s).to_string(),
                )
            })
            .and_then(|result| result);
            let _ = done_tx.send(result);
        });
    });
    let owner = farhelm_teststate::thread::FixtureThread::new(
        "agent_cli mock supervisor",
        thread,
        move || {
            let _ = cancel_tx.send(());
        },
    )
    .expect("start mock supervisor join observer");
    (done_rx, owner)
}

/// Require protocol success before waiting for runtime and thread destruction.
/// The owner stays armed through both assertions, so a failed result still
/// cancels the mock instead of detaching it on the test's unwind path.
fn finish_server(
    done: std::sync::mpsc::Receiver<Result<(), String>>,
    thread: farhelm_teststate::thread::FixtureThread,
) {
    done.recv_timeout(Duration::from_secs(7))
        .expect("mock supervisor did not finish")
        .expect("mock supervisor failed");
    thread
        .finish(Duration::from_secs(1))
        .expect("join mock supervisor");
}

/// Failed test setup must cancel an accept that has no peer, releasing the
/// listener instead of leaving a detached mock alive until its normal deadline.
#[farhelm_testtrace::test]
fn mock_cleanup_cancels_before_accept() {
    let state = farhelm_teststate::tempdir().unwrap();
    let socket = state.path().join("mock.sock");
    let (done, owner) = mock_supervisor(&socket, |_| panic!("no request expected"));
    drop(owner);
    assert_eq!(
        done.recv_timeout(Duration::from_secs(1)).unwrap(),
        Err("mock supervisor cancelled".to_string())
    );
    assert!(std::os::unix::net::UnixStream::connect(&socket).is_err());
}

/// Receiving the server's hello proves accept completed; withholding the rest
/// of the client's frame then exercises cancellation inside the handshake.
#[farhelm_testtrace::test]
fn mock_cleanup_cancels_a_partial_handshake() {
    use std::io::{Read, Write};
    let state = farhelm_teststate::tempdir().unwrap();
    let socket = state.path().join("mock.sock");
    let (done, owner) = mock_supervisor(&socket, |_| panic!("no request expected"));
    let mut peer = std::os::unix::net::UnixStream::connect(&socket).unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    peer.set_write_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    peer.read_exact(&mut [0_u8; 1]).unwrap();
    peer.write_all(&[0]).unwrap();
    drop(owner);
    assert_eq!(
        done.recv_timeout(Duration::from_secs(1)).unwrap(),
        Err("mock supervisor cancelled".to_string())
    );
    assert!(std::os::unix::net::UnixStream::connect(&socket).is_err());
}

/// Cancellation must interrupt a response write when an authenticated peer
/// stops reading. One received response byte proves the write began. The result
/// channel must be empty before cleanup and report cancellation afterward.
#[farhelm_testtrace::test]
async fn mock_cleanup_cancels_response_backpressure() {
    use tokio::io::AsyncReadExt;
    let state = farhelm_teststate::tempdir().unwrap();
    let socket = state.path().join("mock.sock");
    let (done, owner) = mock_supervisor(&socket, |_| {
        let reply = ControlMsg::Error {
            req_id: 1,
            kind: farhelm_proto::ErrorKind::InvalidRequest,
            message: "x".repeat(4 * 1024 * 1024),
        };
        Some(reply)
    });
    let peer = tokio::time::timeout(Duration::from_secs(5), async {
        let peer = tokio::net::UnixStream::connect(&socket).await.unwrap();
        let (mut read, write) = tokio::io::split(peer);
        let mut reader = FrameReader::new(&mut read);
        let mut writer = FrameWriter::new(write);
        farhelm_proto::io::handshake_with_session_auth(
            &mut reader,
            &mut writer,
            farhelm_proto::SessionAuth {
                session_id: "session-1".to_string(),
                token: "secret".to_string(),
            },
        )
        .await
        .unwrap();
        drop(reader);
        writer
            .write_control(&ControlMsg::ListSessions { req_id: 1 })
            .await
            .unwrap();
        read.read_exact(&mut [0_u8; 1]).await.unwrap();
        (read, writer)
    })
    .await
    .expect("mock response must begin");
    assert!(matches!(
        done.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    // Keep the peer open and undrained through cleanup. Closing it here would
    // make a broken cancellation path pass by causing an ordinary write error.
    drop(owner);
    assert_eq!(
        done.recv_timeout(Duration::from_secs(1)).unwrap(),
        Err("mock supervisor cancelled".to_string())
    );
    drop(peer);
    assert!(std::os::unix::net::UnixStream::connect(&socket).is_err());
}

/// An assertion unwind must retain ownership long enough to cancel the mock;
/// callers do not have to reach finish_server to release its listener.
#[farhelm_testtrace::test]
fn mock_unwind_cancels_its_listener() {
    let state = farhelm_teststate::tempdir().unwrap();
    let socket = state.path().join("mock.sock");
    let (done, owner) = mock_supervisor(&socket, |_| panic!("no request expected"));
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _owner = owner;
        panic!("test assertion unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(
        done.recv_timeout(Duration::from_secs(1)).unwrap(),
        Err("mock supervisor cancelled".to_string())
    );
    assert!(std::os::unix::net::UnixStream::connect(&socket).is_err());
}

/// Run the child with a hard deadline, so a relay regression that hangs
/// fails the test instead of pinning the run.
///
/// `farhelm agent` deliberately has no timeout of its own (the supervisor
/// owns that bound), which is exactly why the TEST has to impose one.
fn output_with_timeout(mut command: Command) -> Output {
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().expect("spawn farhelm");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().expect("poll farhelm").is_some() {
            return child.wait_with_output().expect("collect farhelm output");
        }
        if std::time::Instant::now() >= deadline {
            child.kill().expect("kill wedged farhelm");
            let output = child.wait_with_output().expect("collect killed farhelm");
            panic!(
                "farhelm agent exceeded its 10-second test deadline: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Start with no inherited Farhelm launch contract, then let each case add
/// exactly the values it means to exercise.
///
/// The environment scrub is the load-bearing part: these tests run inside
/// whatever shell invoked them, which for a session-launched run already
/// carries a real `FARHELM_SESSION_ID`/`TOKEN`/`SOCK`, and a case that
/// inherited them would silently talk to the developer's own supervisor
/// instead of its mock.
///
/// The backtrace variables are scrubbed for a different reason, and it is
/// a CI-only trap worth naming: `main` returns `anyhow::Result`, so a
/// failing run's stderr is the error's `Debug` rendering, which appends a
/// `Stack backtrace:` block whenever `RUST_BACKTRACE` is set. Nothing in
/// this repository sets it, but `actions-rust-lang/setup-rust-toolchain`
/// exports `RUST_BACKTRACE=short` for the whole CI job, and the child
/// inherits it — which turned a one-line refusal into 23 lines and failed
/// [`a_refused_lifecycle_verb_is_escaped_and_bounded_on_stderr`] on CI
/// while it passed on every developer machine. What that test owns is how
/// much of stderr the PEER can drive; local diagnostics the operator opted
/// into are not part of the contract, so they are removed rather than
/// counted around.
fn agent_command_with_args(socket: &std::path::Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_farhelm"));
    for name in [
        "FARHELM_SESSION_ID",
        "FARHELM_SESSION_TOKEN",
        "FARHELM_SUPERVISOR_SOCK",
        "RUST_BACKTRACE",
        "RUST_LIB_BACKTRACE",
    ] {
        command.env_remove(name);
    }
    command
        .arg("agent")
        .args(args)
        .env("FARHELM_SESSION_ID", "session-1")
        .env("FARHELM_SESSION_TOKEN", "secret")
        .env("FARHELM_SUPERVISOR_SOCK", socket);
    command
}

/// Spec: `farhelm agent hosts` sends exactly one `AgentRequest` — its own
/// `req_id`, the injected session id, and the `Hosts` verb — and renders
/// the reply as an aligned table whose `*` column marks this session's own
/// host.
///
/// Both halves matter and neither is checked anywhere else. The REQUEST
/// shape is the contract the supervisor's restricted dispatch authorizes
/// against, and a `session_id` that did not match the credential would be
/// refused with an `Unauthorized` that no unit test would explain. The
/// OUTPUT is the whole product here: the reader is a language model
/// quoting its own shell output, so column drift is a user-visible
/// regression, and `current` has no other spelling — it is the one field
/// neither this process nor the supervisor could have computed.
#[farhelm_testtrace::test]
fn hosts_asks_as_its_own_session_and_prints_the_marked_table() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id,
            session_id,
            request,
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        assert_eq!(req_id, 1);
        assert_eq!(session_id, "session-1");
        assert_eq!(request, AgentVerb::Hosts {});
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Hosts {
                    hosts: vec![
                        AgentHost {
                            name: "this machine".to_string(),
                            kind: "local".to_string(),
                            state: "connected".to_string(),
                            current: true,
                        },
                        AgentHost {
                            name: "builder".to_string(),
                            kind: "ssh".to_string(),
                            state: "unreachable-reprobing".to_string(),
                            current: false,
                        },
                    ],
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(&socket, &["hosts"]));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        // Joined rather than written as one literal: every row but the
        // current one starts with the two-space marker column, and a
        // multi-line string literal's backslash continuation eats exactly
        // that leading whitespace.
        [
            "  NAME         KIND  STATE",
            "* this machine local connected",
            "  builder      ssh   unreachable-reprobing",
            "",
        ]
        .join("\n")
    );
}

/// Spec: `farhelm agent sessions` sends the `Sessions` verb and renders the
/// marked table, with archive and staleness visible in the STATUS column.
///
/// The column ORDER is the contract being pinned. `farhelm agent` is a
/// public, scriptable CLI whose output is read by humans and models with no
/// schema to consult, so the column layout is the whole interface: a
/// reordering silently changes what every existing wrapper, alias and
/// transcript means by "the fourth column".
///
/// The STATUS cell carries the two facts a status word cannot. An archived
/// session's live status is history the user filed away — showing `running`
/// there invites an agent to go and interact with it — and a cached row
/// from an unreachable host is indistinguishable from a live one without
/// the `(stale)` mark SPEC.md requires.
#[farhelm_testtrace::test]
fn sessions_prints_the_marked_table_with_archive_and_staleness() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        assert_eq!(request, AgentVerb::Sessions {});
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Sessions {
                    sessions: vec![
                        AgentSession {
                            id: "session-1".to_string(),
                            host: Some("this machine".to_string()),
                            title: "auth".to_string(),
                            cwd: "/w/auth".to_string(),
                            agent: "claude".to_string(),
                            status: "running".to_string(),
                            current: true,
                            archived: false,
                            stale: false,
                        },
                        AgentSession {
                            id: "session-2".to_string(),
                            host: Some("builder".to_string()),
                            title: "docs".to_string(),
                            cwd: "/w".to_string(),
                            agent: "codex".to_string(),
                            status: "idle".to_string(),
                            current: false,
                            archived: false,
                            stale: true,
                        },
                        AgentSession {
                            id: "session-3".to_string(),
                            host: Some("builder".to_string()),
                            title: "old".to_string(),
                            cwd: "/w".to_string(),
                            agent: "codex".to_string(),
                            status: "exited".to_string(),
                            current: false,
                            archived: true,
                            stale: false,
                        },
                    ],
                    truncated: false,
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(&socket, &["sessions"]));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        [
            "  ID        HOST         TITLE CWD     AGENT  STATUS",
            "* session-1 this machine auth  /w/auth claude running",
            "  session-2 builder      docs  /w      codex  idle (stale)",
            "  session-3 builder      old   /w      codex  archived",
            "",
        ]
        .join("\n")
    );
    assert!(
        output.stderr.is_empty(),
        "a complete listing warns about nothing: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Spec: a truncated listing prints its rows on stdout and one warning on
/// stderr naming the cut.
///
/// The split is the point. A partial fleet listing is shaped exactly like a
/// complete one, so an agent that cannot see the difference reads "past the
/// cut" as "does not exist" — but the rows it did get are still the answer,
/// and a script capturing stdout must not find prose mixed into its table.
#[farhelm_testtrace::test]
fn a_truncated_listing_prints_its_rows_and_warns_on_stderr() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest { req_id, .. } = request else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Sessions {
                    sessions: vec![AgentSession {
                        id: "session-1".to_string(),
                        host: Some("this machine".to_string()),
                        title: "auth".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "running".to_string(),
                        current: true,
                        archived: false,
                        stale: false,
                    }],
                    truncated: true,
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(&socket, &["sessions"]));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("session-1"), "{stdout}");
    assert!(
        !stdout.contains("whole fleet"),
        "the warning must not contaminate the table: {stdout}"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("not the whole fleet"),
        "a cut listing must say so: {stderr}"
    );
}

/// Spec: a relay refusal reaches the user as the supervisor's own sentence
/// on stderr, with nothing on stdout and a non-zero exit.
///
/// The verbatim relay is the point. "No helm is attached to this session"
/// is the relay's defining failure and it comes with its own remedy
/// ("open it in the farhelm UI"); a CLI that paraphrased it — or that
/// printed a half-table before failing — would strip the one thing that
/// makes the error actionable. Exit status and the empty stdout are the
/// same contract `farhelm spawn` holds, so a script wrapping either can
/// treat them alike.
#[farhelm_testtrace::test]
fn a_refusal_is_the_supervisors_own_sentence_on_stderr() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest { req_id, .. } = request else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Err {
                kind: ErrorKind::Unavailable,
                message: "no helm is attached to this session — open the session in the \
                          farhelm UI and try again"
                    .to_string(),
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(&socket, &["hosts"]));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("no helm is attached to this session — open the session in the farhelm UI"),
        "the relay's own sentence must reach stderr verbatim, got: {stderr}"
    );
}

/// Spec: a refusal's message is ESCAPED and BOUNDED before it reaches
/// stderr — verbatim in wording, not in bytes.
///
/// The companion to the verbatim-relay test above, and the reason the two
/// have to coexist: "render the peer's sentence unchanged" and "never let a
/// peer drive the terminal" are both real requirements, and only the second
/// one has an attacker in it. This is the ONE outcome path whose text can
/// originate on a machine neither this process nor its own supervisor
/// controls — with the lifecycle verbs landed, a refusal can carry a TARGET
/// supervisor's free-text prose (a rejected rename title quoted back, say)
/// — and it reaches stderr through `anyhow`'s `Result` printer, which
/// escapes nothing on its own. Every SUCCESS path already runs its fields
/// through the cell escaping; without `safe_error_message` the failure path
/// was the one hole in that floor.
///
/// The oversized half is the availability side of the same field: a refusal
/// is prose meant to be read, and a peer that answered with megabytes of it
/// would scroll away whatever the user was looking at. The cap is
/// `MAX_ERROR_MESSAGE_CHARS` (4096) plus the ellipsis that marks the cut;
/// this asserts the bound held and the marker is present rather than
/// re-deriving the exact number, which belongs to `main.rs` alone.
///
/// An `InvalidRequest` for a control-bearing `--session` is the realistic
/// shape rather than a contrived one: it is exactly what the relay's own
/// `validate_agent_verb` answers, which is why the sibling
/// [`a_stop_confirmation_escapes_control_characters_in_the_target`] cannot
/// occur end to end and this can.
#[farhelm_testtrace::test]
fn a_refused_lifecycle_verb_is_escaped_and_bounded_on_stderr() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    // Both defects in one message: a forged line break with an escape
    // sequence behind it, then far more text than the cap allows.
    let hostile = format!(
        "an explicit --session target must not contain control characters\n\x1b[31mFAKE: \
         succeeded{}",
        "x".repeat(8192)
    );
    let (done, thread) = mock_supervisor(&socket, move |request| {
        let ControlMsg::AgentRequest { req_id, .. } = request else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Err {
                kind: ErrorKind::InvalidRequest,
                message: hostile,
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &["stop", "--session", "hostile\nid"],
    ));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "a refusal prints no confirmation");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("must not contain control characters"),
        "the refusal's own wording must survive escaping: {stderr:?}"
    );
    assert!(
        !stderr.contains('\x1b'),
        "no raw ESC may reach the terminal: {stderr:?}"
    );
    assert!(
        stderr.contains("\\n\\x1b[31mFAKE"),
        "the forged line break and its escape sequence must both be visible, not acted on: \
         {stderr:?}"
    );
    assert_eq!(
        stderr.lines().count(),
        1,
        "a refusal must stay one line however many the peer wrote: {stderr:?}"
    );
    let mut tail: Vec<char> = stderr.trim_end().chars().rev().take(16).collect();
    tail.reverse();
    let tail: String = tail.into_iter().collect();
    assert!(
        tail.ends_with('…'),
        "an oversized refusal must be cut with a visible marker, tail was {tail:?}"
    );
    assert!(
        stderr.chars().count() < 5000,
        "the cap must bound the line, got {} characters",
        stderr.chars().count()
    );
}

/// Spec: the injected-environment contract is validated before any socket
/// is opened, and a missing supervisor socket is named rather than guessed
/// around.
///
/// `farhelm agent` shares `spawn_environment` with `farhelm spawn` on
/// purpose — one definition of what a Farhelm session guarantees — and this
/// pins that the sharing is real rather than a coincidence of two similar
/// code paths.
#[farhelm_testtrace::test]
fn a_missing_supervisor_socket_is_a_clean_precondition_failure() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_farhelm"));
    for name in [
        "FARHELM_SESSION_ID",
        "FARHELM_SESSION_TOKEN",
        "FARHELM_SUPERVISOR_SOCK",
    ] {
        command.env_remove(name);
    }
    let output = command
        .args(["agent", "hosts"])
        .env("FARHELM_SESSION_ID", "session-1")
        .env("FARHELM_SESSION_TOKEN", "secret")
        .output()
        .expect("run farhelm agent");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("FARHELM_SUPERVISOR_SOCK"));
    assert!(stderr.contains("will not guess"));
}

/// Spec: `farhelm agent hosts` names the command it is, not `farhelm
/// spawn`, when the injected environment is incomplete.
///
/// The two commands share `spawn_environment`, which is the right sharing —
/// one definition of what a Farhelm session guarantees — but its messages
/// used to be hard-coded to spawn's name. A user running `farhelm agent`
/// then read an error about a feature they had not invoked, and went off to
/// diagnose the wrong thing.
#[farhelm_testtrace::test]
fn a_precondition_failure_names_the_command_that_was_run() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_farhelm"));
    for name in [
        "FARHELM_SESSION_ID",
        "FARHELM_SESSION_TOKEN",
        "FARHELM_SUPERVISOR_SOCK",
    ] {
        command.env_remove(name);
    }
    let output = command
        .args(["agent", "hosts"])
        .env("FARHELM_SESSION_ID", "session-1")
        .env("FARHELM_SESSION_TOKEN", "secret")
        .output()
        .expect("run farhelm agent");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("farhelm agent"), "{stderr}");
    assert!(
        !stderr.contains("farhelm spawn"),
        "the error must not name a command the user did not run: {stderr}"
    );
}

/// One `farhelm agent` run against a mock that answers with `reply`,
/// returning its output.
///
/// A helper because the four protocol-error cases below differ only in the
/// frame the peer sends back, and repeating twenty lines of ceremony around
/// that one difference would bury it.
fn agent_run_against(
    verb: &str,
    reply: impl FnOnce(ControlMsg) -> Option<ControlMsg> + Send + 'static,
) -> Output {
    agent_run_against_frame(verb, |request| {
        reply(request).map(|reply| Frame::control(&reply))
    })
}

/// [`agent_run_against`] answering with a raw frame; see
/// [`mock_supervisor_frames`] for why that shape has to exist.
fn agent_run_against_frame(
    verb: &str,
    reply: impl FnOnce(ControlMsg) -> Option<Frame> + Send + 'static,
) -> Output {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor_frames(&socket, reply);
    let output = output_with_timeout(agent_command_with_args(&socket, &[verb]));
    finish_server(done, thread);
    output
}

/// A control frame whose body is not decodable, for the one ending a
/// well-typed `ControlMsg` cannot express.
fn undecodable_control_frame() -> Frame {
    Frame {
        kind: farhelm_proto::FrameKind::Control,
        channel: 0,
        body: b"{not json".to_vec(),
    }
}

/// The `req_id` of whatever the CLI sent, for a mock that means to answer
/// something else with it.
fn asked_req_id(request: &ControlMsg) -> u64 {
    match request {
        ControlMsg::AgentRequest { req_id, .. } => *req_id,
        other => panic!("farhelm agent must send an AgentRequest, got {other:?}"),
    }
}

/// Spec: a response the CLI cannot trust — mis-correlated, unrelated,
/// uncorrelated, or absent — exits non-zero with nothing on stdout and a
/// diagnostic on stderr.
///
/// These four branches are what stand between a defective or hostile peer
/// and a table the user believes. A response carrying someone else's
/// `req_id` is not this request's answer; an unrelated control message is
/// not an answer at all; a bare `Error` is the shape the supervisor uses
/// when it refuses the CREDENTIAL, before any request has been read, and
/// must reach the user as prose rather than as a decode failure; and EOF
/// before a reply is what a supervisor dying mid-request looks like to a
/// CLI that deliberately carries no deadline of its own. All four are
/// invisible to the success-path tests above, and weakening any of them
/// would let the command print the wrong thing, say the wrong thing, or
/// hang.
#[farhelm_testtrace::test]
fn an_untrustworthy_reply_fails_with_nothing_on_stdout() {
    // A response correlated with a request this process never made.
    let mismatched = agent_run_against("hosts", |request| {
        Some(ControlMsg::AgentResponse {
            req_id: asked_req_id(&request) + 99,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Hosts { hosts: Vec::new() },
            },
        })
    });
    // A perfectly valid message that answers nothing.
    let unrelated = agent_run_against("hosts", |_| {
        Some(ControlMsg::Detached {
            channel: 1,
            reason: "not an answer".to_string(),
        })
    });
    // The uncorrelated refusal shape (`req_id` 0), which must still be
    // rendered verbatim rather than treated as a protocol violation.
    let bare_error = agent_run_against("hosts", |_| {
        Some(ControlMsg::Error {
            req_id: 0,
            message: "the session credential is invalid".to_string(),
            kind: ErrorKind::Unauthorized,
        })
    });
    // The peer goes away without answering.
    let eof = agent_run_against("hosts", |_| None);

    for (case, output) in [
        ("a mis-correlated response", &mismatched),
        ("an unrelated control message", &unrelated),
        ("a bare Error", &bare_error),
        ("EOF before a reply", &eof),
    ] {
        assert_eq!(output.status.code(), Some(1), "{case} must exit non-zero");
        assert!(
            output.stdout.is_empty(),
            "{case} must print no table: {:?}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            !output.stderr.is_empty(),
            "{case} must say something on stderr"
        );
    }

    let stderr = String::from_utf8(mismatched.stderr.clone()).unwrap();
    assert!(stderr.contains("unexpected"), "{stderr}");
    let stderr = String::from_utf8(unrelated.stderr.clone()).unwrap();
    assert!(stderr.contains("unexpected"), "{stderr}");
    let stderr = String::from_utf8(bare_error.stderr.clone()).unwrap();
    assert!(
        stderr.contains("the session credential is invalid"),
        "a refusal reaches the user verbatim: {stderr}"
    );
    let stderr = String::from_utf8(eof.stderr.clone()).unwrap();
    assert!(stderr.contains("closed"), "{stderr}");
}

/// Spec: a supervisor that reads a MUTATING request and then dies without
/// answering is reported as an outcome-unknown failure carrying the
/// check-before-retrying remedy, while the same ending for a listing keeps
/// the plain transport wording.
///
/// This is the one delivered-outcome-unknown ending no `ErrorKind` can
/// describe. Every other one travels as an `AgentOutcome::Err` the relay
/// classified; here the LOCAL socket is what died, after the request was
/// already on it, so nothing is left to carry a classification and the CLI
/// is the only party that still knows both facts — the verb it sent, and
/// that its write completed. The supervisor may already have forwarded the
/// stop to a helm that applied it on another host, so a message reading as
/// "nothing happened, ask again" is an invitation to stop a session someone
/// has since restarted.
///
/// Both classes are driven from one test because the difference is the
/// whole content of the fix: the listing arm is what keeps the remedy from
/// being appended to every transport failure, where it would be noise no
/// caller can act on. `stop` is the verb because it is the one whose repeat
/// is destructive in the plainest way.
#[farhelm_testtrace::test]
fn a_supervisor_dying_after_reading_a_mutation_says_the_outcome_is_unknown() {
    // The mock reads the request (`mock_supervisor` always does) and then
    // returns without writing, which closes the connection — the shape of a
    // supervisor dying between forwarding a request and answering it.
    let mutation = agent_run_against("stop", |_| None);
    let listing = agent_run_against("sessions", |_| None);

    assert_eq!(mutation.status.code(), Some(1));
    assert!(mutation.stdout.is_empty());
    let stderr = String::from_utf8(mutation.stderr).unwrap();
    assert!(
        stderr.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
        "a lost mutation reply must tell the caller to look before retrying: {stderr}"
    );
    assert!(
        stderr.contains("outcome is unknown"),
        "and must say why: {stderr}"
    );

    assert_eq!(listing.status.code(), Some(1));
    let stderr = String::from_utf8(listing.stderr).unwrap();
    assert!(
        !stderr.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
        "a listing has nothing to double-apply and must keep the plain transport wording: \
         {stderr}"
    );
    assert!(stderr.contains("closed"), "{stderr}");
}

/// Spec: a successful reply of the WRONG shape is a protocol error, in both
/// directions.
///
/// The `reply` tag exists for exactly this, and only a client that
/// remembered what it asked can use it: a response is handed back by
/// `req_id` alone across two hops, so a peer that correlated a sessions
/// listing with a hosts request would otherwise have that listing printed
/// under `farhelm agent hosts` — output that looks authoritative while
/// answering a question nobody asked. Both directions are covered because a
/// check written against one verb is easy to write in a way that passes
/// everything else.
#[farhelm_testtrace::test]
fn a_reply_for_the_other_verb_is_refused() {
    let hosts_got_sessions = agent_run_against("hosts", |request| {
        Some(ControlMsg::AgentResponse {
            req_id: asked_req_id(&request),
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Sessions {
                    sessions: Vec::new(),
                    truncated: false,
                },
            },
        })
    });
    let sessions_got_hosts = agent_run_against("sessions", |request| {
        Some(ControlMsg::AgentResponse {
            req_id: asked_req_id(&request),
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Hosts { hosts: Vec::new() },
            },
        })
    });

    for (case, output) in [
        ("hosts answered with sessions", &hosts_got_sessions),
        ("sessions answered with hosts", &sessions_got_hosts),
    ] {
        assert_eq!(output.status.code(), Some(1), "{case} must exit non-zero");
        assert!(
            output.stdout.is_empty(),
            "{case} must print no table: {:?}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8(output.stderr.clone()).unwrap();
        assert!(
            stderr.contains("listing"),
            "{case} must say what went wrong: {stderr}"
        );
    }
}

/// Spec: EVERY untrustworthy answer to a MUTATION carries the
/// check-before-retrying remedy, not only the peer going away — and none of
/// them carries it for a listing.
///
/// The sibling tests above own the shapes; this owns the vocabulary, and
/// the gap it closes was the whole subtlety of the ending. A supervisor
/// dying after reading the request already said "the outcome is unknown",
/// which made the remedy look like a property of connection loss. It is not:
/// it is a property of the WRITE having completed. A frame that will not
/// decode, a response carrying somebody else's `req_id`, a control message
/// that answers nothing, and an `Ok` of the wrong shape are all reached
/// strictly after the `stop` went out, and a peer defective enough to
/// produce any of them is exactly as likely to have stopped the session
/// first as not. Reporting those as bare protocol errors invites the reader
/// to retry a destructive verb on the strength of a decode complaint.
///
/// The listing half is what keeps the remedy meaningful. Appended to every
/// protocol failure it would be noise no caller can act on, so `sessions`
/// is driven through the same four peers and must come back without it.
#[farhelm_testtrace::test]
fn an_untrustworthy_answer_to_a_mutation_says_the_outcome_is_unknown() {
    /// The four post-write endings that are not the peer's own refusal,
    /// each run against `verb`.
    ///
    /// A closure per case rather than a table, because `agent_run_against`
    /// takes a `FnOnce` the mock thread consumes; the shapes cannot be
    /// cloned into two runs.
    fn untrustworthy_endings(verb: &'static str) -> Vec<(&'static str, Output)> {
        vec![
            (
                "an undecodable frame",
                agent_run_against_frame(verb, |_| Some(undecodable_control_frame())),
            ),
            (
                "a mis-correlated response",
                agent_run_against(verb, |request| {
                    Some(ControlMsg::AgentResponse {
                        req_id: asked_req_id(&request) + 99,
                        outcome: AgentOutcome::Ok {
                            reply: AgentReply::Stopped {},
                        },
                    })
                }),
            ),
            (
                "an unrelated control message",
                agent_run_against(verb, |_| {
                    Some(ControlMsg::Detached {
                        channel: 1,
                        reason: "not an answer".to_string(),
                    })
                }),
            ),
            (
                "a success reply of the wrong shape",
                agent_run_against(verb, |request| {
                    Some(ControlMsg::AgentResponse {
                        req_id: asked_req_id(&request),
                        outcome: AgentOutcome::Ok {
                            reply: AgentReply::Hosts { hosts: Vec::new() },
                        },
                    })
                }),
            ),
        ]
    }

    for (case, output) in untrustworthy_endings("stop") {
        assert_eq!(output.status.code(), Some(1), "{case} must exit non-zero");
        assert!(
            output.stdout.is_empty(),
            "{case} must print no confirmation: {:?}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("outcome is unknown"),
            "{case} after a mutation was sent must say the outcome is unknown: {stderr}"
        );
        assert!(
            stderr.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
            "{case} must tell the caller to look before retrying: {stderr}"
        );
    }

    for (case, output) in untrustworthy_endings("sessions") {
        assert_eq!(output.status.code(), Some(1), "{case} must exit non-zero");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            !stderr.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
            "{case} on a listing has nothing to double-apply and must stay plain: {stderr}"
        );
    }
}

// ---------------------------------------------------------------
// The three lifecycle verbs: what `farhelm agent rename/stop/archive`
// sends, and the one confirmation line each prints on success.
//
// "Prints on success" holds unconditionally against a MOCK and not against
// the real stack: a bare `stop`/`archive` targets the asking session, and
// the marker-keyed sweep that ends it reaches this CLI process too, so a
// real self-stop can be SIGTERMed before its own `println!` runs (see
// `main`'s Rename/Stop/Archive comment, and the e2e lifecycle test that
// routes around it). Every case below sends its verb to a mock that stops
// nothing, which is what makes the confirmation observable at all.
// ---------------------------------------------------------------

/// Spec: `farhelm agent rename <title> --session <id>` sends exactly one
/// `Rename` verb naming the given title and target, and prints the
/// updated row's id and title as one plain confirmation line.
///
/// The confirmation reads from the REPLY's own fields (`AgentSession::id`/
/// `title`), not from the CLI's own arguments — proving the helm's answer
/// is what reaches the terminal rather than an echo of what was typed,
/// which would still look right if the helm silently renamed something
/// else. The mock's reply deliberately answers with an id and a title
/// NEITHER of which equal what the CLI sent, which is what makes that
/// distinction one this test can actually fail on: a reply that echoed the
/// request's own `session_id`/`title` back would pass just as well whether
/// the confirmation prints the reply or the argument.
#[farhelm_testtrace::test]
fn rename_sends_the_title_and_named_target_and_prints_the_confirmation() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id,
            session_id,
            request,
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        assert_eq!(session_id, "session-1", "the asking session's own id");
        assert_eq!(
            request,
            AgentVerb::Rename {
                session_id: Some("other-session".to_string()),
                title: "new title".to_string(),
            }
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Session {
                    session: AgentSession {
                        id: "resolved-session".to_string(),
                        host: Some("this machine".to_string()),
                        title: "the helm's own title".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "running".to_string(),
                        current: false,
                        archived: false,
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &["rename", "new title", "--session", "other-session"],
    ));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "renamed resolved-session to \"the helm's own title\"\n",
        "the confirmation must print the REPLY's fields, not an echo of the arguments sent"
    );
    assert!(output.stderr.is_empty());
}

/// Spec: `farhelm agent stop`, with no `--session`, sends `Stop` naming no
/// target — the substitution the helm resolves to the asking session — and
/// the confirmation names the ASKING session, since `Stopped` itself
/// carries no id to read one back from.
#[farhelm_testtrace::test]
fn stop_with_no_session_flag_sends_none_and_names_the_asking_session() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        assert_eq!(request, AgentVerb::Stop { session_id: None });
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Stopped {},
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(&socket, &["stop"]));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "stopped session-1\n",
        "with no --session, the confirmation names the injected asking session"
    );
    assert!(output.stderr.is_empty());
}

/// Spec: `farhelm agent archive --session <id>` sends `Archive` naming that
/// target, and prints the id from the REPLY rather than the one typed.
///
/// The reply's id and the argument are deliberately the same string here,
/// which makes this the weaker half of a pair: the distinction between
/// "printed the answer" and "echoed the argument" is what
/// [`a_rename_confirmation_escapes_and_delimits_both_of_its_fields`] and
/// the rename target test pin, and this exists for the WIRE half —
/// `--session` reaching `AgentVerb::Archive::session_id` as `Some`, which
/// is the encoding the helm's whole target-resolution rule keys off. Its
/// twin [`bare_archive_sends_no_target_and_lets_the_helm_substitute_the_asker`]
/// pins the `None` side; neither is meaningful without the other, since a
/// CLI that hardcoded either one would pass exactly one of them.
#[farhelm_testtrace::test]
fn archive_sends_the_named_target_and_prints_its_id() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        assert_eq!(
            request,
            AgentVerb::Archive {
                session_id: Some("other-session".to_string()),
            }
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Session {
                    session: AgentSession {
                        id: "other-session".to_string(),
                        host: Some("this machine".to_string()),
                        title: "auth".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "exited".to_string(),
                        current: false,
                        archived: true,
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &["archive", "--session", "other-session"],
    ));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "archived other-session\n"
    );
    assert!(output.stderr.is_empty());
}

/// Spec: a rename confirmation is sanitized through the SAME cell-escaping
/// the listing tables use — SPEC_impl.md's contract for every dynamic cell
/// this CLI ever prints, extended here to the lifecycle confirmations —
/// and its quoted TITLE field is additionally escaped so the quotes really
/// do delimit it.
///
/// A title is user-supplied fleet-wide text (the same field the `sessions`
/// table's TITLE column escapes), so a control character in it must not
/// reach the terminal raw: a bare `\x07` rings the bell and a raw newline
/// would let a hostile or careless title split the confirmation into two
/// lines, the second of which a script parsing "one line means success"
/// would not expect.
///
/// THE QUOTE CASE is a separate defect from the control-character one and
/// is why this test carries both. The title is the only value this CLI
/// wraps in literal `"` on stdout, and cell escaping does nothing to a `"`
/// — so a title containing one used to close the field early, and anything
/// reading the line for a quoted value saw a title the sender chose the end
/// of. The backslash goes with it: without doubling, `\` before the closing
/// quote would escape it in any conventional decoder.
///
/// The ID is hostile here too, and not merely for symmetry. Only the title
/// is quoted, so the id's escaping runs through a different path in the
/// same `println!`, and a change that fixed quoting while dropping
/// `safe_cell` from the id would still pass a title-only test.
#[farhelm_testtrace::test]
fn a_rename_confirmation_escapes_and_delimits_both_of_its_fields() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest { req_id, .. } = request else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Session {
                    session: AgentSession {
                        id: "sess\n1".to_string(),
                        host: Some("this machine".to_string()),
                        title: "line one\nsays \"hi\" \\ bye".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "running".to_string(),
                        current: true,
                        archived: false,
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &["rename", "line one\nline two"],
    ));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout, "renamed sess\\n1 to \"line one\\nsays \\\"hi\\\" \\\\ bye\"\n",
        "both fields must be escaped, and the title's own quotes must not close its field"
    );
    assert_eq!(
        stdout.lines().count(),
        1,
        "one confirmation must stay one line even when neither field is: {stdout:?}"
    );
}

/// Spec: a stop confirmation escapes control characters in the target id,
/// the same way a rename confirmation escapes its own fields.
///
/// DEFENCE IN DEPTH, not a reachable end-to-end scenario, and the
/// distinction is worth stating because the fixture looks like one. A
/// control-bearing `--session` no longer survives the round trip: the
/// relay's own `validate_agent_verb` refuses such a target before anything
/// is forwarded, so a real supervisor would answer `InvalidRequest` rather
/// than the `Stopped` this mock returns (that path is
/// [`a_refused_lifecycle_verb_is_escaped_and_bounded_on_stderr`]'s
/// subject). What this pins is the printing layer on its own: `Stop`'s
/// reply is empty (`AgentReply::Stopped` carries no fields), so the id in
/// the confirmation is the one THIS process resolved and never re-read
/// from anyone, and it must stay escaped independently of whichever hop
/// currently happens to reject hostile ids. A validation rule that moved,
/// loosened, or gained an exemption would otherwise silently remove the
/// only thing keeping a forged second line off stdout.
#[farhelm_testtrace::test]
fn a_stop_confirmation_escapes_control_characters_in_the_target() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest { req_id, .. } = request else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Stopped {},
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &["stop", "--session", "line one\nline two"],
    ));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout, "stopped line one\\nline two\n",
        "the embedded newline in the target id must be escaped, not printed raw"
    );
    assert_eq!(stdout.lines().count(), 1, "{stdout:?}");
}

/// Spec: an archive confirmation escapes control characters in the id it
/// reads back from the reply.
///
/// The other half of the pair with the stop test above, and the half that
/// covers PEER-supplied text rather than this process's own argument:
/// `archive`'s confirmation prints the reply's `AgentSession::id` (see
/// `main`'s `Archive` arm), so the hostile value is planted in what the
/// mock answers.
///
/// Defence in depth for the same reason its sibling is. A conforming helm
/// answers with the id it acted on, which cannot carry a control character
/// once the relay has refused one going the other way — so what this pins
/// is that a MALFORMED or nonconforming reply cannot repaint the terminal
/// on the way to being printed. Nothing in this CLI verifies that the id
/// coming back is one it could have sent, and this is deliberately not an
/// argument for adding such a check: escaping every printed field is the
/// cheaper invariant and does not need to know what a legal id looks like.
#[farhelm_testtrace::test]
fn an_archive_confirmation_escapes_control_characters_in_the_id() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest { req_id, .. } = request else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Session {
                    session: AgentSession {
                        id: "line one\nline two".to_string(),
                        host: Some("this machine".to_string()),
                        title: "t".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "exited".to_string(),
                        current: true,
                        archived: true,
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(&socket, &["archive"]));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout, "archived line one\\nline two\n",
        "the embedded newline in the reply's id must be escaped, not printed raw"
    );
    assert_eq!(stdout.lines().count(), 1, "{stdout:?}");
}

/// Spec: bare `farhelm agent rename <title>` (no `--session`) sends
/// `Rename` with `session_id: None` — the asking-session substitution
/// every other lifecycle verb already pins at this wire-encoding layer
/// (see `stop_with_no_session_flag_sends_none_and_names_the_asking_session`
/// for `Stop`'s own version of this contract), and here for `Rename`,
/// which previously had no such coverage below the handler-test and e2e
/// layers.
#[farhelm_testtrace::test]
fn bare_rename_sends_no_target_and_lets_the_helm_substitute_the_asker() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        assert_eq!(
            request,
            AgentVerb::Rename {
                session_id: None,
                title: "new title".to_string(),
            },
            "omitting --session must send no target at all"
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Session {
                    session: AgentSession {
                        id: "session-1".to_string(),
                        host: Some("this machine".to_string()),
                        title: "new title".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "running".to_string(),
                        current: true,
                        archived: false,
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(&socket, &["rename", "new title"]));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "renamed session-1 to \"new title\"\n"
    );
    assert!(output.stderr.is_empty());
}

/// Spec: bare `farhelm agent archive` (no `--session`) sends `Archive` with
/// `session_id: None` — the asking-session substitution `Rename` and `Stop`
/// already pin at this wire-encoding layer, and here for `Archive`, which
/// previously had no such coverage below the handler-test and e2e layers
/// (every existing `archive` test here named an explicit `--session`).
#[farhelm_testtrace::test]
fn bare_archive_sends_no_target_and_lets_the_helm_substitute_the_asker() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        assert_eq!(
            request,
            AgentVerb::Archive { session_id: None },
            "omitting --session must send no target at all"
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Session {
                    session: AgentSession {
                        id: "session-1".to_string(),
                        host: Some("this machine".to_string()),
                        title: "t".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "exited".to_string(),
                        current: true,
                        archived: true,
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(&socket, &["archive"]));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "archived session-1\n"
    );
    assert!(output.stderr.is_empty());
}

/// Spec: a rename title starting with a hyphen is sent verbatim as the
/// title, not misparsed as an unrecognized flag.
///
/// `title` is a positional argument, and without clap's `allow_hyphen_values`
/// a leading-hyphen value is indistinguishable to the parser from a flag it
/// does not recognize — even though the supervisor itself places no such
/// restriction on a title (SPEC.md's only rename refusal is a control
/// character). Without the attribute this test's own title would make the
/// CLI exit before ever opening a socket, and `mock_supervisor`'s
/// "the agent CLI did not connect" timeout would be the failure, not a
/// clean assertion on the request actually sent.
#[farhelm_testtrace::test]
fn a_rename_title_starting_with_a_hyphen_is_not_misparsed_as_a_flag() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        assert_eq!(
            request,
            AgentVerb::Rename {
                session_id: None,
                title: "-not-a-flag".to_string(),
            },
            "a leading hyphen must reach the wire as ordinary title text"
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Session {
                    session: AgentSession {
                        id: "session-1".to_string(),
                        host: Some("this machine".to_string()),
                        title: "-not-a-flag".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "running".to_string(),
                        current: true,
                        archived: false,
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(&socket, &["rename", "-not-a-flag"]));
    finish_server(done, thread);

    assert_eq!(
        output.status.code(),
        Some(0),
        "clap must accept the leading hyphen, got stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "renamed session-1 to \"-not-a-flag\"\n"
    );
}

/// Spec: `farhelm agent instructions` prints every verb the binary carries
/// and exits 0, with no supervisor socket, no session credential, and no
/// helm anywhere.
///
/// This is the reachability claim the whole feature rests on. An agent
/// meets farhelm through one line the `SessionStart` hook prints, and that
/// line's only instruction is to run this command. If the command needed
/// the session environment, the agent's first act on being told about
/// farhelm would be an error message — and the sessions least likely to
/// have a working relay (a host nobody has open in a client) are exactly
/// the ones whose agent most needs to be told what to do about it.
///
/// The unit tests in `agent_instructions.rs` check the text's content
/// against clap. What only a PROCESS can check is the pair this test
/// exists for: that the command runs at all outside a session, and that it
/// exits 0 while doing it.
///
/// `help` is asserted to be byte-identical rather than merely similar
/// because it is documented as the same output. Two texts that drifted
/// apart would mean an agent's answer depended on which spelling it
/// happened to try.
#[farhelm_testtrace::test]
fn instructions_print_every_verb_without_a_session() {
    let run = |verb: &str| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_farhelm"));
        // Stripped rather than merely unset: this test process may itself
        // be running inside a farhelm session, and inheriting a live
        // credential would let the command reach a real supervisor — which
        // is the one thing this test is claiming it never needs.
        for name in [
            "FARHELM_SESSION_ID",
            "FARHELM_SESSION_TOKEN",
            "FARHELM_SUPERVISOR_SOCK",
        ] {
            command.env_remove(name);
        }
        command.args(["agent", verb]);
        output_with_timeout(command)
    };

    let output = run("instructions");
    assert_eq!(
        output.status.code(),
        Some(0),
        "instructions must succeed outside a session: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "instructions are the whole output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("the instructions are UTF-8");
    for verb in [
        "hosts",
        "sessions",
        "rename",
        "stop",
        "archive",
        "create",
        "clone",
        "instructions",
        "help",
    ] {
        assert!(
            text.contains(&format!("farhelm agent {verb}")),
            "the printed instructions never mention `farhelm agent {verb}`:\n{text}"
        );
    }
    // The three conventions an agent cannot infer: the trigger, the marker
    // column, and the failure that has a remedy rather than a cause.
    assert!(text.contains("$farhelm"), "{text}");
    assert!(text.contains("\"*\""), "{text}");
    assert!(
        text.contains("no helm is attached to this session"),
        "{text}"
    );

    let alias = run("help");
    assert_eq!(alias.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(alias.stdout).expect("UTF-8"),
        text,
        "`help` and `instructions` must print the same bytes"
    );
}

/// Spec: `farhelm agent --help` still prints clap's usage screen, not the
/// agent-facing instructions.
///
/// `farhelm agent help` was taken over by farhelm's own verb, which means
/// clap's built-in `help` SUBCOMMAND had to be disabled to stop it
/// shadowing that. The `--help` FLAG is a different mechanism and must
/// survive: it is the surface a human at a terminal reaches for, and it is
/// the only place the full option syntax is spelled out. A regression here
/// would be invisible to every other test in this file, since they all
/// drive real verbs.
#[farhelm_testtrace::test]
fn the_help_flag_still_prints_clap_usage() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_farhelm"));
    command.args(["agent", "--help"]);
    let output = output_with_timeout(command);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(
        // Specifically THIS subcommand's usage line, not merely "some
        // clap usage screen" — a regression that printed the top-level
        // `farhelm --help` or a sibling subcommand's screen would satisfy
        // a bare `"Usage:"` check while answering the wrong question.
        text.contains("Usage: farhelm agent"),
        "--help must be clap's usage screen for `farhelm agent` specifically: {text}"
    );
    assert!(
        !text.contains("$farhelm"),
        "--help must not be the agent instructions: {text}"
    );
}

/// Spec: `farhelm agent create` puts every flag on the wire under its
/// protocol spelling, prints ONLY the new session's id on stdout, and puts
/// the human-readable confirmation on stderr.
///
/// The split streams are the contract, and they invert what every
/// lifecycle verb in this file does. `create` and `clone` are the two verbs
/// whose stdout is meant to be captured as a SINGLE VALUE — the id is what
/// a caller goes on to pass as `--session`, exactly as `farhelm spawn`'s
/// single stdout line is — so a confirmation sentence on stdout would make
/// the two verbs that need parsing the two that cannot be. The listings are
/// parsed too, but as tables, where an extra line is survivable.
///
/// The `--profile` → `profile_name` spelling is checked because it is the
/// one place the CLI's word and the wire's word deliberately differ: the
/// helm resolves that value against the TARGET host's catalog, and sending
/// it as an id would resolve on the wrong catalog rather than fail (ids
/// collide across installs by construction).
#[farhelm_testtrace::test]
fn create_sends_every_flag_and_prints_only_the_new_id_on_stdout() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id,
            session_id,
            request,
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        assert_eq!(session_id, "session-1", "the asking session's own id");
        assert_eq!(
            request,
            AgentVerb::Create {
                host: Some("builder".to_string()),
                cwd: "/srv/work".to_string(),
                profile_name: Some("Claude Code".to_string()),
                invocation: None,
                title: Some("over there".to_string()),
                intent_key: Some("key-1".to_string()),
            }
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Created {
                    session: AgentSession {
                        id: "new-session".to_string(),
                        host: Some("builder".to_string()),
                        title: "over there".to_string(),
                        cwd: "/srv/work".to_string(),
                        agent: "Claude Code".to_string(),
                        status: String::new(),
                        current: false,
                        archived: false,
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &[
            "create",
            "--cwd",
            "/srv/work",
            "--host",
            "builder",
            "--profile",
            "Claude Code",
            "--title",
            "over there",
            "--idempotency-key",
            "key-1",
        ],
    ));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "new-session\n",
        "stdout is the id and nothing else — spawn's contract"
    );
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "created new-session \"over there\" on builder in /srv/work\n"
    );
}

/// Spec: `farhelm agent clone` with no flags sends every field absent,
/// which is how "another one of these, right here" is spelled on the wire.
///
/// Every one of those `None`s is a DEFAULT the helm resolves, not an
/// omission: no host means the asking session's own, no cwd and no title
/// mean the source's. A CLI that filled any of them in locally — the
/// asking session's own directory, say, which this process could read —
/// would be answering a question only the helm has the information to
/// answer, since the source session may not even be this process's own
/// working directory.
#[farhelm_testtrace::test]
fn bare_clone_sends_every_field_absent() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        assert_eq!(
            request,
            AgentVerb::Clone {
                host: None,
                cwd: None,
                title: None,
                intent_key: None,
            }
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Created {
                    session: AgentSession {
                        id: "the-copy".to_string(),
                        host: Some("this machine".to_string()),
                        title: "the original".to_string(),
                        cwd: "/srv/project".to_string(),
                        agent: "Claude".to_string(),
                        status: String::new(),
                        current: false,
                        archived: false,
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(&socket, &["clone"]));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "the-copy\n");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "created the-copy \"the original\" on this machine in /srv/project\n"
    );
}

/// Spec: `farhelm agent clone` puts every option it was given on the wire
/// — including a `--idempotency-key` beginning with a hyphen — and its
/// confirmation on stderr escapes every control character in the fields
/// the helm sent back.
///
/// The two halves are here together because they share one round trip and
/// neither is covered elsewhere for `clone`. Its `--title` and
/// `--idempotency-key` are declared independently of `create`'s, so either
/// could be dropped or transposed without failing any test that only drove
/// `create`; and the key is the field that makes an ambiguous retry safe,
/// so a key silently swallowed by clap as an unrecognized option is the
/// worst possible one to lose. The hyphen-leading key is the shape that
/// needs `allow_hyphen_values`: the downstream contract allows it, so the
/// CLI must carry it rather than refuse it locally.
///
/// The escaping matters here more than on the lifecycle confirmations, and
/// for a reason specific to this verb: three of the four fields printed
/// (title, host name, working directory) are text from ANOTHER host on the
/// fleet, and a newline or an ESC in any of them would forge a line or
/// reach terminal features in the transcript of a model that is about to
/// quote this output back to a user.
#[farhelm_testtrace::test]
fn a_clone_sends_every_option_and_escapes_control_characters_in_its_confirmation() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        assert_eq!(
            request,
            AgentVerb::Clone {
                host: Some("builder".to_string()),
                cwd: Some("/srv/elsewhere".to_string()),
                title: Some("the copy".to_string()),
                intent_key: Some("-leading-hyphen-key".to_string()),
            }
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Created {
                    session: AgentSession {
                        id: "the-copy".to_string(),
                        host: Some("buil\x1b[2Jder".to_string()),
                        title: "forged\nrow".to_string(),
                        cwd: "/srv/\tproject".to_string(),
                        agent: "Claude".to_string(),
                        status: String::new(),
                        current: false,
                        archived: false,
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &[
            "clone",
            "--host",
            "builder",
            "--cwd",
            "/srv/elsewhere",
            "--title",
            "the copy",
            "--idempotency-key",
            "-leading-hyphen-key",
        ],
    ));
    finish_server(done, thread);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "the-copy\n",
        "the id line is the machine-readable one and is never escaped or decorated"
    );
    let confirmation = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        confirmation.lines().count(),
        1,
        "one confirmation is one line, whatever the fields contained: {confirmation:?}"
    );
    assert!(
        confirmation.contains("\\n")
            && confirmation.contains("\\t")
            && confirmation.contains("\\x1b"),
        "every control character must survive as a visible escape: {confirmation:?}"
    );
}

/// Spec: `farhelm agent create` naming both `--profile` and `--invocation`
/// is refused by the CLI itself, with nothing sent and nothing on stdout.
///
/// Refused HERE rather than only at the helm because clap can: the two are
/// mutually exclusive on the wire as well, so the round trip would end in
/// the same refusal, and spending it to learn what a local check knows
/// costs an agent a supervisor hop and a helm hop. The helm's own refusal
/// stays in place for every other client — this is a shortcut, not the
/// authority.
///
/// No mock supervisor is started, which is the sharp end: if this ever
/// regressed into sending the request, there would be no socket to connect
/// to and the failure would still be an error — so the assertion is
/// specifically that clap's own conflict message is what came back.
#[farhelm_testtrace::test]
fn create_naming_both_selectors_is_refused_before_anything_is_sent() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &[
            "create",
            "--cwd",
            "/srv/work",
            "--profile",
            "Claude",
            "--invocation",
            "sh",
        ],
    ));
    assert_eq!(output.status.code(), Some(2), "clap's usage-error status");
    assert!(output.stdout.is_empty(), "a refused create prints no id");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("--invocation") && stderr.contains("--profile"),
        "the refusal must name both flags: {stderr}"
    );
}

/// Spec: `farhelm agent create` without `--cwd` is refused by clap, since
/// a create has no default working directory.
///
/// Not a defaulted field, and this is the test that keeps it that way. The
/// tempting default — the asking session's own directory — would make
/// `create` a `clone` wearing another verb's name, and the CLI is not even
/// the party that knows it: the asking session's directory lives on the
/// helm's side of the relay.
#[farhelm_testtrace::test]
fn create_without_a_cwd_is_refused() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let output = output_with_timeout(agent_command_with_args(&socket, &["create"]));
    assert_eq!(output.status.code(), Some(2), "clap's usage-error status");
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("--cwd"),
        "the refusal must name it: {stderr}"
    );
}

/// Spec: every hyphen-leading VALUE a create can carry reaches the wire
/// verbatim rather than being read as an unrecognized flag.
///
/// The same hazard `a_rename_title_starting_with_a_hyphen_is_not_misparsed_
/// as_a_flag` covers for titles, applied to the whole creating surface.
/// Every one of these values is judged DOWNSTREAM — a profile name by the
/// target's catalog, an idempotency key by its reservation table, a
/// directory by the target's filesystem — and every one of them may
/// legally begin with `-`, so a local refusal here is this CLI declining to
/// carry a value the far end would have accepted or explained.
///
/// The fixture's `--invocation` is deliberately option-shaped
/// (`--weird-program`) rather than a realistic wrapper: what is under test
/// is clap's parse of a hyphen-leading VALUE, and a value that merely
/// contains flags after a normal program name would never have exercised
/// it.
#[farhelm_testtrace::test]
fn hyphen_leading_create_values_are_not_misparsed_as_flags() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        let AgentVerb::Create {
            host,
            cwd,
            invocation,
            title,
            intent_key,
            profile_name,
        } = request
        else {
            panic!("expected a Create verb, got {request:?}");
        };
        assert_eq!(invocation.as_deref(), Some("--weird-program --flag"));
        assert_eq!(host.as_deref(), Some("-odd-host"));
        assert_eq!(cwd, "-odd-dir");
        assert_eq!(title.as_deref(), Some("-odd-title"));
        assert_eq!(intent_key.as_deref(), Some("-odd-key"));
        assert_eq!(
            profile_name, None,
            "the invocation selector was chosen, so no profile travels"
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Created {
                    session: AgentSession {
                        id: "new-session".to_string(),
                        host: Some("this machine".to_string()),
                        title: "t".to_string(),
                        cwd: "/w".to_string(),
                        agent: "weird-program".to_string(),
                        status: String::new(),
                        current: false,
                        archived: false,
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &[
            "create",
            "--cwd",
            "-odd-dir",
            "--host",
            "-odd-host",
            "--invocation",
            "--weird-program --flag",
            "--title",
            "-odd-title",
            "--idempotency-key",
            "-odd-key",
        ],
    ));
    finish_server(done, thread);

    assert_eq!(
        output.status.code(),
        Some(0),
        "clap must carry every hyphen-leading value, got stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "new-session\n");
}

/// Spec: a `--profile` beginning with a hyphen reaches the wire verbatim.
///
/// Separate from the sibling above because `--profile` and `--invocation`
/// are mutually exclusive: they cannot both be exercised in one command
/// line, and a profile name is exactly the kind of value whose legality the
/// TARGET host's catalog decides — refusing it here would tell an agent its
/// name is malformed when the truth is that this CLI would not carry it.
#[farhelm_testtrace::test]
fn a_hyphen_leading_profile_name_is_not_misparsed_as_a_flag() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        let AgentVerb::Create { profile_name, .. } = request else {
            panic!("expected a Create verb, got {request:?}");
        };
        assert_eq!(profile_name.as_deref(), Some("-dash-profile"));
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Created {
                    session: AgentSession {
                        id: "new-session".to_string(),
                        host: Some("this machine".to_string()),
                        title: "t".to_string(),
                        cwd: "/w".to_string(),
                        agent: "-dash-profile".to_string(),
                        status: String::new(),
                        current: false,
                        archived: false,
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &["create", "--cwd", "/w", "--profile", "-dash-profile"],
    ));
    finish_server(done, thread);

    assert_eq!(
        output.status.code(),
        Some(0),
        "clap must carry the hyphen-leading profile name, got stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "new-session\n");
}

/// Spec: a `Session` reply to a CREATING verb is refused — nothing on
/// stdout, and the error names both the shape that came back and the one
/// expected.
///
/// Driven through `clone` because both creating verbs share one expectation
/// and one refusal; the test is about the reply TAG, not about which of the
/// two asked. (It says `clone` for the same reason the CLI's own arm is
/// shared: there is nothing verb-specific left to distinguish here.)
///
/// `AgentReply::Session` and `AgentReply::Created` carry byte-identical
/// payloads, so the tag is the entire difference between "a row that did
/// not exist a moment ago" and "the row you renamed". Accepting the wrong
/// one would print an EXISTING session's id as though this command had
/// created it — a target an agent might then go on to stop or archive. No
/// other test in the stack can see this: the relay hands a response back by
/// `req_id` alone across two hops, and neither hop re-checks the shape.
#[farhelm_testtrace::test]
fn a_session_reply_to_a_creating_verb_is_refused() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let req_id = asked_req_id(&request);
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Session {
                    session: AgentSession {
                        id: "an-existing-session".to_string(),
                        host: Some("this machine".to_string()),
                        title: "t".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "running".to_string(),
                        current: false,
                        archived: false,
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(&socket, &["clone"]));
    finish_server(done, thread);

    assert_ne!(output.status.code(), Some(0));
    assert!(
        output.stdout.is_empty(),
        "an id that was never created must not be printed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("answered with a session row where a created session row was expected"),
        "the error names the shape that came back and the one expected, in words that read the \
         same whichever pair they describe: {stderr}"
    );
}

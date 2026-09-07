//! Process-level contract tests for `farhelm spawn`.
//!
//! These drive the built binary so stdout, stderr, exit status, clap's
//! required-argument boundary, environment validation, and the wire request
//! are tested together. Child-only environment changes keep the test runner
//! safe for parallel execution.

use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
use farhelm_proto::{ControlMsg, RestartOffer, SessionInfo, SessionStatus, SourceProfile, TabInfo};
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// Run a child with a hard deadline so a protocol regression fails instead
/// of pinning the test process forever.
fn output_with_timeout(mut command: Command) -> Output {
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().expect("spawn farhelm");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().expect("poll farhelm").is_some() {
            return child.wait_with_output().expect("collect farhelm output");
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill wedged farhelm");
            let output = child.wait_with_output().expect("collect killed farhelm");
            panic!(
                "farhelm spawn exceeded its 10-second test deadline: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Serve one authenticated request with cancellation across the entire exchange.
///
/// The returned owner cancels blocked handshake and reply writes during unwind.
/// A six-second transaction allowance fits inside finish_server's seven-second
/// result wait, which still requires protocol success before observing the join.
fn mock_supervisor(
    socket: &std::path::Path,
    respond: impl FnOnce(ControlMsg) -> ControlMsg + Send + 'static,
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
                                            .expect("spawn did not connect")
                                            .expect("accept spawn");
                                    let (read, write) = tokio::io::split(stream);
                                    let mut reader = FrameReader::new(read);
                                    let mut writer = FrameWriter::new(write);
                                    let hello = handshake(&mut reader, &mut writer, "supervisor")
                                        .await
                                        .expect("handshake");
                                    let ControlMsg::Hello {
                                        role,
                                        auth: Some(auth),
                                        ..
                                    } = hello
                                    else {
                                        panic!("spawn must authenticate in its hello: {hello:?}");
                                    };
                                    assert_eq!(role, "spawn");
                                    assert_eq!(auth.session_id, "parent-123");
                                    assert_eq!(auth.token, "secret");
                                    let frame = tokio::time::timeout(
                                        Duration::from_secs(5),
                                        reader.read_frame(),
                                    )
                                    .await
                                    .expect("spawn did not send create")
                                    .unwrap()
                                    .expect("create request");
                                    let reply = respond(parse_control(&frame).unwrap());
                                    writer.write_control(&reply).await.unwrap();
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
        "spawn_cli mock supervisor",
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
    let (done, owner) = mock_supervisor(&socket, |_| ControlMsg::Error {
        req_id: 1,
        kind: farhelm_proto::ErrorKind::InvalidRequest,
        message: "x".repeat(4 * 1024 * 1024),
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
                session_id: "parent-123".to_string(),
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

/// Prove a precondition failure returned before opening the supplied socket.
fn assert_zero_accepts(listener: &std::os::unix::net::UnixListener) {
    listener.set_nonblocking(true).unwrap();
    assert!(matches!(
        listener.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
}

/// Start with no inherited Farhelm launch contract, then let each case add
/// exactly the values it means to exercise.
fn spawn_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_farhelm"));
    for name in [
        "FARHELM_SESSION_ID",
        "FARHELM_SESSION_TOKEN",
        "FARHELM_SUPERVISOR_SOCK",
    ] {
        command.env_remove(name);
    }
    command.arg("spawn");
    command
}

/// A compact successful reply; spawn consumes only the id, but sending the
/// real wire shape guards against a test double that accidentally blesses a
/// private shortcut.
fn child_session(cwd: String) -> SessionInfo {
    SessionInfo {
        id: "child-123".to_string(),
        parent: Some("parent-123".to_string()),
        title: "child".to_string(),
        created_at: 1_700_000_000,
        last_activity_at: 1_700_000_000,
        creation_seq: None,
        cwd,
        invocation: "agent".to_string(),
        status: SessionStatus::Running,
        annotation: None,
        restart_offer: RestartOffer::FreshOnly,
        tabs: Vec::<TabInfo>::new(),
        archived: false,
        source_profile: Some(SourceProfile {
            id: "profile-1".to_string(),
            name: "Agent One".to_string(),
            existence: farhelm_proto::ProfileExistence::Present,
        }),
    }
}

/// Runtime preconditions use one ordinary failure status, write no stdout,
/// and name the exact missing socket contract without dialing a fallback.
#[farhelm_testtrace::test]
fn a_missing_supervisor_socket_is_a_clean_precondition_failure() {
    let output = spawn_command()
        .args(["--cwd", "."])
        .env("FARHELM_SESSION_ID", "parent-123")
        .env("FARHELM_SESSION_TOKEN", "secret")
        .output()
        .expect("run spawn");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("FARHELM_SUPERVISOR_SOCK"));
    assert!(stderr.contains("will not guess"));
}

/// A session launched by the pre-credential build has one actionable
/// remedy, and validation reaches it before any attempt to open the socket.
#[farhelm_testtrace::test]
fn a_preupgrade_session_is_told_to_restart_before_spawning() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let output = spawn_command()
        .args(["--cwd", "."])
        .env("FARHELM_SESSION_ID", "parent-123")
        .env("FARHELM_SUPERVISOR_SOCK", &socket)
        .output()
        .expect("run spawn");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("predates spawn support") && stderr.contains("restarted"));
    assert!(!stderr.contains("connecting to supervisor"));
    assert_zero_accepts(&listener);
}

/// A token and socket without an owning session id are not authority, and
/// the failure is detected before the socket can observe a connection.
#[farhelm_testtrace::test]
fn a_missing_session_id_is_a_clean_precondition_failure() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let output = spawn_command()
        .args(["--cwd", "."])
        .env("FARHELM_SESSION_TOKEN", "secret")
        .env("FARHELM_SUPERVISOR_SOCK", &socket)
        .output()
        .expect("run spawn");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("FARHELM_SESSION_ID")
    );
    assert_zero_accepts(&listener);
}

/// Every injected spawn value is a UTF-8 protocol value, even though Unix
/// permits arbitrary bytes in the child environment and in socket paths.
///
/// Refusing each malformed value before the dial prevents replacement-byte
/// laundering from changing credentials or selecting a different endpoint.
#[farhelm_testtrace::test]
fn non_utf8_spawn_environment_values_are_refused_before_dialing() {
    for malformed_name in [
        "FARHELM_SESSION_ID",
        "FARHELM_SESSION_TOKEN",
        "FARHELM_SUPERVISOR_SOCK",
    ] {
        let temp = farhelm_teststate::tempdir().unwrap();
        let socket = temp.path().join("supervisor.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let output = spawn_command()
            .args(["--cwd", "."])
            .env("FARHELM_SESSION_ID", "parent-123")
            .env("FARHELM_SESSION_TOKEN", "secret")
            .env("FARHELM_SUPERVISOR_SOCK", &socket)
            .env(malformed_name, OsString::from_vec(vec![0xff]))
            .output()
            .expect("run spawn");

        assert_eq!(output.status.code(), Some(1), "{malformed_name}");
        assert!(output.stdout.is_empty(), "{malformed_name}");
        let stderr = String::from_utf8(output.stderr).expect("diagnostic is UTF-8");
        assert!(
            stderr.contains(malformed_name) && stderr.contains("not valid UTF-8"),
            "{malformed_name}: {stderr}"
        );
        assert!(!stderr.contains('\u{fffd}'), "{malformed_name}: {stderr}");
        assert!(
            !stderr.contains("connecting to supervisor"),
            "{malformed_name}"
        );
        assert_zero_accepts(&listener);
    }
}

/// The working directory is a required scripting input, not a value inferred
/// from the parent session or a default silently selected by clap.
#[farhelm_testtrace::test]
fn cwd_is_required_by_the_cli_surface() {
    let output = spawn_command().output().expect("run spawn");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().contains("--cwd"));
}

/// A successful command emits exactly one id line and maps every scripting
/// flag onto the authenticated CreateSession request.
#[farhelm_testtrace::test]
fn success_is_one_stdout_line_and_the_wire_request_preserves_every_flag() {
    let temp = farhelm_teststate::tempdir().expect("tempdir");
    let real = temp.path().join("real-work");
    std::fs::create_dir(&real).expect("working directory");
    std::os::unix::fs::symlink("real-work", temp.path().join("alias")).expect("symlink alias");
    let socket = temp.path().join("supervisor.sock");
    let expected_cwd = temp
        .path()
        .join("alias/missing-child")
        .to_string_lossy()
        .into_owned();
    let (done, server) = mock_supervisor(&socket, move |request| {
        let ControlMsg::CreateSession {
            req_id,
            parent,
            cwd,
            invocation,
            profile_name,
            title,
            intent_key,
            agent_kind,
            resume_template,
            source_profile,
            ..
        } = request
        else {
            panic!("spawn must send CreateSession: {request:?}");
        };
        assert_eq!(parent.as_deref(), Some("parent-123"));
        assert_eq!(cwd, expected_cwd);
        assert_eq!(invocation, None);
        assert_eq!(profile_name.as_deref(), Some("Agent One"));
        assert_eq!(agent_kind, None);
        assert_eq!(resume_template, None);
        assert_eq!(source_profile, None);
        assert_eq!(title.as_deref(), Some("scripted child"));
        assert_eq!(intent_key.as_deref(), Some("retry-7"));
        ControlMsg::SessionCreated {
            req_id,
            session: child_session(cwd),
        }
    });

    let Output {
        status,
        stdout,
        stderr,
    } = spawn_command()
        .current_dir(temp.path())
        .args([
            "--cwd",
            "alias/missing-child",
            "--agent",
            "Agent One",
            "--parent",
            "parent-123",
            "--title",
            "scripted child",
            "--idempotency-key",
            "retry-7",
        ])
        .env("FARHELM_SESSION_ID", "parent-123")
        .env("FARHELM_SESSION_TOKEN", "secret")
        .env("FARHELM_SUPERVISOR_SOCK", &socket)
        .output()
        .expect("run spawn");
    finish_server(done, server);
    assert!(status.success());
    assert_eq!(stdout, b"child-123\n");
    assert!(
        stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&stderr)
    );
}

/// Creation success is independent of the status snapshot in the reply.
///
/// A fast-exiting agent can be classified before the CLI receives
/// `SessionCreated`. The session and terminal still exist, so both an
/// ordinary exit and a launch error must produce the child id and exit zero
/// rather than reinterpret a successful create as a failed command.
#[farhelm_testtrace::test]
fn a_created_child_id_succeeds_even_when_its_status_is_already_terminal() {
    for status in [
        SessionStatus::Exited { exit_code: Some(7) },
        SessionStatus::Error {
            detail: "agent executable was missing".to_string(),
        },
    ] {
        let temp = farhelm_teststate::tempdir().unwrap();
        let socket = temp.path().join("supervisor.sock");
        let (done, server) = mock_supervisor(&socket, move |request| {
            let ControlMsg::CreateSession { req_id, cwd, .. } = request else {
                panic!("spawn must send CreateSession: {request:?}");
            };
            let mut session = child_session(cwd);
            session.status = status;
            ControlMsg::SessionCreated { req_id, session }
        });
        let mut command = spawn_command();
        command
            .args(["--cwd", "/tmp"])
            .env("FARHELM_SESSION_ID", "parent-123")
            .env("FARHELM_SESSION_TOKEN", "secret")
            .env("FARHELM_SUPERVISOR_SOCK", &socket);
        let output = output_with_timeout(command);
        finish_server(done, server);
        assert!(output.status.success());
        assert_eq!(output.stdout, b"child-123\n");
        assert!(output.stderr.is_empty());
    }
}

/// Both handshake-adjacent and request-correlated refusals terminate the
/// CLI cleanly without writing a phantom child id.
#[farhelm_testtrace::test]
fn supervisor_error_replies_exit_nonzero_with_empty_stdout() {
    for (req_id, kind, message) in [
        (
            0,
            farhelm_proto::ErrorKind::Unauthorized,
            "session credential refused",
        ),
        (
            1,
            farhelm_proto::ErrorKind::Conflict,
            "idempotency key conflicts",
        ),
    ] {
        let temp = farhelm_teststate::tempdir().unwrap();
        let socket = temp.path().join("supervisor.sock");
        let (done, server) = mock_supervisor(&socket, move |_| ControlMsg::Error {
            req_id,
            kind,
            message: message.to_string(),
        });
        let mut command = spawn_command();
        command
            .args(["--cwd", "/tmp"])
            .env("FARHELM_SESSION_ID", "parent-123")
            .env("FARHELM_SESSION_TOKEN", "secret")
            .env("FARHELM_SUPERVISOR_SOCK", &socket);
        let output = output_with_timeout(command);
        finish_server(done, server);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains(message));
    }
}

/// A syntactically valid reply for another request is a protocol error, not
/// an event to discard while waiting forever for a reply that may never come.
#[farhelm_testtrace::test]
fn an_unexpected_reply_fails_instead_of_hanging() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, server) = mock_supervisor(&socket, |_| ControlMsg::SessionCreated {
        req_id: 99,
        session: child_session("/tmp".to_string()),
    });
    let mut command = spawn_command();
    command
        .args(["--cwd", "/tmp"])
        .env("FARHELM_SESSION_ID", "parent-123")
        .env("FARHELM_SESSION_TOKEN", "secret")
        .env("FARHELM_SUPERVISOR_SOCK", &socket);
    let output = output_with_timeout(command);
    finish_server(done, server);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("unexpected spawn reply")
    );
}

/// `~`-prefixed cwds cross the wire VERBATIM — never absolutized into
/// `<cwd>/~...` — because expansion is the supervisor's contract
/// (SPEC.md: `~` resolves against the supervisor user's home; `~user` is
/// its refusal to give). The ordinary relative path in the same test pins
/// that the tilde exception did not swallow the join-against-cwd rule.
///
/// Guarded by a test because the regression is silent and plausible: a
/// future "simplification" back to unconditional absolutizing would make
/// `~/x` a nonexistent local path at best, and at worst a real directory
/// literally named `~user` that dodges the supervisor's refusal.
#[farhelm_testtrace::test]
fn tilde_cwds_cross_the_wire_verbatim() {
    let temp = farhelm_teststate::tempdir().expect("tempdir");
    let socket = temp.path().join("supervisor.sock");
    for (sent, expected) in [
        ("~", "~".to_string()),
        ("~/child", "~/child".to_string()),
        ("~other/x", "~other/x".to_string()),
        (
            "plain/child",
            temp.path()
                .join("plain/child")
                .to_string_lossy()
                .into_owned(),
        ),
    ] {
        let expected_wire = expected.clone();
        let (done, server) = mock_supervisor(&socket, move |request| {
            let ControlMsg::CreateSession { req_id, cwd, .. } = request else {
                panic!("spawn must send CreateSession: {request:?}");
            };
            assert_eq!(cwd, expected_wire, "cwd for input {sent:?}");
            ControlMsg::SessionCreated {
                req_id,
                session: child_session(cwd),
            }
        });
        let output = spawn_command()
            .current_dir(temp.path())
            .args(["--cwd", sent])
            .env("FARHELM_SESSION_ID", "parent-123")
            .env("FARHELM_SESSION_TOKEN", "secret")
            .env("FARHELM_SUPERVISOR_SOCK", &socket)
            .output()
            .expect("run spawn");
        finish_server(done, server);
        assert!(
            output.status.success(),
            "spawn with cwd {sent:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::remove_file(&socket).ok();
    }
}

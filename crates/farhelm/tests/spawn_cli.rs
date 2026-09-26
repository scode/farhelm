//! Process-level contract tests for `farhelm spawn`.
//!
//! These drive the built binary so stdout, stderr, exit status, clap's
//! required-argument boundary, environment validation, and the wire request
//! are tested together. Child-only environment changes keep the test runner
//! safe for parallel execution.

use farhelm_proto::{ControlMsg, RestartOffer, SessionInfo, SessionStatus, SourceProfile, TabInfo};
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::process::{Command, Output};

mod cli_support;
use cli_support::output_with_timeout;
#[path = "cli_support/mock_supervisor.rs"]
mod mock_supervisor;
use mock_supervisor::finish_server;

/// The session every test here spawns from.
const PARENT: &str = "parent-123";

/// [`mock_supervisor::mock_supervisor`] for this file's session, answering
/// every request.
fn mock_supervisor(
    socket: &std::path::Path,
    respond: impl FnOnce(ControlMsg) -> ControlMsg + Send + 'static,
) -> (
    std::sync::mpsc::Receiver<Result<(), String>>,
    farhelm_teststate::thread::FixtureThread,
) {
    mock_supervisor::mock_supervisor(socket, PARENT, move |request| Some(respond(request)))
}

/// [`mock_supervisor::mock_supervisor`] for this file's session; `None`
/// closes the connection after reading the request, without answering.
fn mock_supervisor_or_close(
    socket: &std::path::Path,
    respond: impl FnOnce(ControlMsg) -> Option<ControlMsg> + Send + 'static,
) -> (
    std::sync::mpsc::Receiver<Result<(), String>>,
    farhelm_teststate::thread::FixtureThread,
) {
    mock_supervisor::mock_supervisor(socket, PARENT, respond)
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
        last_work_started_at: 0,
        creation_seq: None,
        cwd,
        canonical_cwd: None,
        invocation: "agent".to_string(),
        resume_template: None,
        launch: None,
        status: SessionStatus::Running,
        annotation: None,
        restart_offer: RestartOffer::FreshOnly,
        tabs: Vec::<TabInfo>::new(),
        source_profile: Some(SourceProfile {
            id: "profile-1".to_string(),
            name: "Agent One".to_string(),
            existence: farhelm_proto::ProfileExistence::Present,
        }),
        github_repo: None,
        working_copy: None,
    }
}

/// Runtime preconditions use one ordinary failure status, write no stdout,
/// and name the exact missing socket contract without dialing a fallback.
#[farhelm_testtrace::test]
fn a_missing_supervisor_socket_is_a_clean_precondition_failure() {
    let output = spawn_command()
        .args(["--cwd", ".", "--inherit-agent"])
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
        .args(["--cwd", ".", "--inherit-agent"])
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
        .args(["--cwd", ".", "--inherit-agent"])
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
            .args(["--cwd", ".", "--inherit-agent"])
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

/// Spawn requires an explicit agent choice before it can contact the
/// supervisor.
///
/// The inheritance flag is a consequential choice rather than the absence
/// of one. Keeping this refusal at clap also prevents a current CLI from
/// emitting the omitted-selector wire shape that older builds treated as an
/// implicit parent snapshot.
#[farhelm_testtrace::test]
fn an_agent_selector_is_required_by_the_cli_surface() {
    let output = spawn_command()
        .args(["--cwd", "/tmp"])
        .output()
        .expect("run spawn");
    assert_eq!(output.status.code(), Some(2), "clap's usage-error status");
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("--agent"),
        "the refusal names a selector: {stderr}"
    );
    assert!(
        stderr.contains("--inherit-agent"),
        "the refusal names explicit inheritance: {stderr}"
    );
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
            profile_id,
            inherit_agent,
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
        assert_eq!(profile_id, None);
        assert!(!inherit_agent);
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
            .args(["--cwd", "/tmp", "--inherit-agent"])
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
            .args(["--cwd", "/tmp", "--inherit-agent"])
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
///
/// It is also an answer that says nothing about whether the create landed,
/// so it carries the same outcome-unknown warning as a lost reply: a peer
/// broken enough to mis-correlate is as likely to have started the child
/// as not.
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
        .args(["--cwd", "/tmp", "--inherit-agent"])
        .env("FARHELM_SESSION_ID", "parent-123")
        .env("FARHELM_SESSION_TOKEN", "secret")
        .env("FARHELM_SUPERVISOR_SOCK", &socket);
    let output = output_with_timeout(command);
    finish_server(done, server);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unexpected spawn reply"), "{stderr}");
    assert!(
        stderr.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
        "{stderr}"
    );
}

/// Spec: a spawn whose reply is lost after the request went out says the
/// outcome is unknown and tells the caller to look before retrying.
///
/// The supervisor may have created the child before dying, so a bare
/// "connection closed" invites a retry that starts a second one. `farhelm
/// agent create` has always said this; `spawn` used to exit with only the
/// transport error.
#[farhelm_testtrace::test]
fn a_lost_reply_says_the_child_may_already_exist() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, server) = mock_supervisor_or_close(&socket, |request| {
        assert!(matches!(request, ControlMsg::CreateSession { .. }));
        None
    });
    let mut command = spawn_command();
    command
        .args(["--cwd", "/tmp", "--inherit-agent"])
        .env("FARHELM_SESSION_ID", "parent-123")
        .env("FARHELM_SESSION_TOKEN", "secret")
        .env("FARHELM_SUPERVISOR_SOCK", &socket);
    let output = output_with_timeout(command);
    finish_server(done, server);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("outcome is unknown"), "{stderr}");
    assert!(
        stderr.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
        "{stderr}"
    );
}

/// Spec: a supervisor's refusal is printed with control and invisible
/// characters escaped, like every other piece of peer text the CLI shows.
///
/// The message is free text from the supervisor and ends up on the
/// caller's terminal through `main`'s error printer, which escapes
/// nothing; an unescaped escape sequence there can rewrite what the user
/// sees. `farhelm agent` already escaped its refusals; `spawn` did not.
#[farhelm_testtrace::test]
fn a_refusal_is_printed_escaped() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, server) = mock_supervisor(&socket, |_| ControlMsg::Error {
        req_id: 1,
        kind: farhelm_proto::ErrorKind::Conflict,
        message: "refused\u{1b}[2J\nforged line".to_string(),
    });
    let mut command = spawn_command();
    command
        .args(["--cwd", "/tmp", "--inherit-agent"])
        .env("FARHELM_SESSION_ID", "parent-123")
        .env("FARHELM_SESSION_TOKEN", "secret")
        .env("FARHELM_SUPERVISOR_SOCK", &socket);
    let output = output_with_timeout(command);
    finish_server(done, server);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains('\u{1b}'), "{stderr:?}");
    assert!(
        stderr.contains("refused\\x1b[2J\\nforged line"),
        "{stderr:?}"
    );
    assert!(
        !stderr.contains("outcome is unknown"),
        "a refusal is a definite answer: {stderr}"
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
            .args(["--cwd", sent, "--inherit-agent"])
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

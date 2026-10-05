//! Process-level contract tests for `farhelm spawn`.
//!
//! These drive the built binary so stdout, stderr, exit status, clap's
//! required-argument boundary, environment validation, and the wire request
//! are tested together. Child-only environment changes keep the test runner
//! safe for parallel execution.

use farhelm_proto::{ControlMsg, RestartOffer, SessionInfo, SessionStatus, TabInfo};
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
        agent_kind: farhelm_proto::AgentKind::Generic,
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
        launch: farhelm_proto::SessionLaunch::plain_command("agent"),
        status: SessionStatus::Running,
        annotation: None,
        restart_offer: RestartOffer::NoConversationReporting,
        tabs: Vec::<TabInfo>::new(),
        github_repo: None,
        working_copy: None,
        notifications: Vec::new(),
    }
}

/// The helm's answer to a relayed spawn, as the supervisor hands it back:
/// a created session with `id` in `cwd`, in `status` (which the CLI never
/// reads; a create is a success whatever its snapshot says).
fn created_reply(req_id: u64, cwd: String, status: &str) -> ControlMsg {
    use farhelm_proto::{AgentOutcome, AgentReply, AgentSession};
    ControlMsg::AgentResponse {
        req_id,
        outcome: AgentOutcome::Ok {
            reply: AgentReply::Created {
                session: AgentSession {
                    id: "child-123".to_string(),
                    host_id: "1".to_string(),
                    host: Some("this machine".to_string()),
                    title: "child".to_string(),
                    cwd,
                    agent: "custom".to_string(),
                    status: status.to_string(),
                    current: false,
                    restart_offer: Default::default(),
                    stale: false,
                },
            },
        },
    }
}

/// An inheriting spawn's relayed request: its `req_id`, folder, title, key,
/// and parent. Panics on anything else, so a test double cannot answer a
/// request shape the CLI no longer sends.
fn inheriting_spawn(
    request: ControlMsg,
) -> (
    u64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    use farhelm_proto::AgentVerb;
    use farhelm_proto::launcher::TemplateDestination;
    let ControlMsg::AgentRequest {
        req_id,
        request:
            AgentVerb::Create {
                host,
                templates,
                edits,
                intent_key,
                confirm_yolo,
                spawn: Some(spawn),
            },
        ..
    } = request
    else {
        panic!("an inheriting spawn must be relayed as an agent create: {request:?}");
    };
    assert_eq!(host, None, "a spawn names no host");
    assert!(templates.is_empty());
    assert!(!confirm_yolo, "the CLI never sends the YOLO override");
    assert!(spawn.inherit_agent);
    assert_eq!(
        spawn.inherited_launch, None,
        "the supervisor fills the launch in; the CLI never sends one"
    );
    let cwd = match edits.destination {
        Some(TemplateDestination::Folder(cwd)) => Some(cwd),
        _ => None,
    };
    (req_id, cwd, edits.name, intent_key, spawn.parent)
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

/// Spec: an inherited spawn needs `--cwd`, refused before the supervisor
/// is dialed, with nothing on stdout.
///
/// Why: the child's folder is not something an inherited spawn may guess;
/// only a spawn with launch flags can take it from a template.
#[farhelm_testtrace::test]
fn an_inherited_spawn_requires_cwd() {
    let output = spawn_command()
        .arg("--inherit-agent")
        .output()
        .expect("run spawn");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().contains("--cwd"));
}

/// Spec: a spawn naming neither `--inherit-agent` nor any launch flag is
/// refused before the supervisor is contacted, naming both ways to say
/// what the child runs.
///
/// Why: what a child runs is a consequential choice rather than the
/// absence of one, and this also keeps a current CLI from emitting the
/// omitted-selector wire shape older builds treated as an implicit parent
/// snapshot.
#[farhelm_testtrace::test]
fn a_spawn_must_say_what_the_child_runs() {
    let output = spawn_command()
        .args(["--cwd", "/tmp"])
        .output()
        .expect("run spawn");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("--inherit-agent") && stderr.contains("--template"),
        "the refusal names both ways to describe the child: {stderr}"
    );
}

/// Spec: `--inherit-agent` beside a launch flag is refused at parse, before
/// the supervisor is contacted.
///
/// Why: SPEC.md makes inheritance exclusive with every launch flag; a
/// spawn that silently preferred one would run something the caller did
/// not ask for. The variables point at a socket nobody listens on, so a
/// regression that sent the request would fail on the connection rather
/// than with clap's usage error.
#[farhelm_testtrace::test]
fn inheritance_with_a_launch_flag_is_refused() {
    let temp = farhelm_teststate::tempdir().expect("tempdir");
    let output = spawn_command()
        .args(["--cwd", "/tmp", "--agent", "claude", "--inherit-agent"])
        .env("FARHELM_SESSION_ID", "parent-123")
        .env("FARHELM_SESSION_TOKEN", "secret")
        .env("FARHELM_SUPERVISOR_SOCK", temp.path().join("absent.sock"))
        .output()
        .expect("run spawn");
    assert_eq!(output.status.code(), Some(2), "clap's usage-error status");
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("--inherit-agent") && stderr.contains("--agent"),
        "{stderr}"
    );
}

/// Spec: a spawn with launch flags goes to the helm as an agent `create`
/// placed on this session's own host: the flags as launcher edits, the
/// templates in order, the folder (resolved against this process's cwd),
/// title, parent and key carried over; the created id is the one stdout
/// line.
///
/// Why: SPEC.md has the attached helm resolve a spawn's launch flags, since
/// it owns the templates and composes agent launches; the spawn contract
/// (one id on stdout) must hold on that path as on the inherited one.
#[farhelm_testtrace::test]
fn a_spawn_with_launch_flags_is_relayed_to_the_helm() {
    use farhelm_proto::launcher::{TemplateDestination, TemplateFields};
    use farhelm_proto::{AgentOutcome, AgentReply, AgentSession, AgentVerb};
    let temp = farhelm_teststate::tempdir().expect("tempdir");
    let socket = temp.path().join("supervisor.sock");
    let expected_cwd = temp.path().join("child").to_string_lossy().into_owned();
    let (done, server) = mock_supervisor(&socket, move |request| {
        let ControlMsg::AgentRequest {
            req_id,
            request:
                AgentVerb::Create {
                    host,
                    templates,
                    edits,
                    intent_key,
                    confirm_yolo,
                    spawn,
                },
            ..
        } = request
        else {
            panic!("a flag spawn must send an agent create: {request:?}");
        };
        assert_eq!(host, None, "a spawn names no host");
        assert_eq!(templates, ["base"]);
        assert_eq!(
            edits,
            TemplateFields {
                agent: Some(farhelm_proto::LaunchHarness::Codex),
                model: Some(Some("gpt-6-luna".to_string())),
                destination: Some(TemplateDestination::Folder(expected_cwd.clone())),
                name: Some("scripted child".to_string()),
                ..Default::default()
            }
        );
        assert_eq!(intent_key.as_deref(), Some("retry-7"));
        assert!(!confirm_yolo, "the CLI never sends the YOLO override");
        assert_eq!(
            spawn.and_then(|spawn| spawn.parent).as_deref(),
            Some("parent-123")
        );
        ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Created {
                    session: AgentSession {
                        id: "child-123".to_string(),
                        host_id: "1".to_string(),
                        host: Some("this machine".to_string()),
                        title: "scripted child".to_string(),
                        cwd: expected_cwd,
                        agent: "codex".to_string(),
                        status: String::new(),
                        current: false,
                        restart_offer: Default::default(),
                        stale: false,
                    },
                },
            },
        }
    });
    let output = spawn_command()
        .current_dir(temp.path())
        .args([
            "--cwd",
            "child",
            "--template",
            "base",
            "--agent",
            "codex",
            "--model",
            "gpt-6-luna",
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
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"child-123\n");
}

/// Spec: while a relayed spawn waits more than a couple of seconds for its
/// answer, the CLI says once on stderr that it is waiting for the helm and
/// that Farhelm may be asking the user, and the id still comes out alone on
/// stdout when the answer arrives.
///
/// Why: an acting verb can wait minutes for the user (SPEC.md), and an agent
/// watching a command that prints nothing cannot tell that from a hang; the
/// line is local to the CLI, so a stdout that held it would break spawn's
/// one-id contract.
#[farhelm_testtrace::test]
fn a_slow_answer_prints_one_waiting_line_on_stderr() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, server) = mock_supervisor(&socket, move |request| {
        let (req_id, cwd, ..) = inheriting_spawn(request);
        // sleep-ok: the stimulus is an answer slower than the CLI's notice delay (2 s).
        std::thread::sleep(std::time::Duration::from_secs(3));
        created_reply(req_id, cwd.unwrap_or_default(), "running")
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
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        stderr.matches("waiting for the helm").count(),
        1,
        "exactly one waiting line: {stderr}"
    );
    assert!(stderr.contains("approve this request"), "{stderr}");
}

/// A successful inheriting spawn emits exactly one id line and maps every
/// scripting flag onto the relayed agent create, which carries no launch of
/// its own (its supervisor fills the session's in).
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
        let (req_id, cwd, title, intent_key, parent) = inheriting_spawn(request);
        assert_eq!(parent.as_deref(), Some("parent-123"));
        assert_eq!(cwd.as_deref(), Some(expected_cwd.as_str()));
        assert_eq!(title.as_deref(), Some("scripted child"));
        assert_eq!(intent_key.as_deref(), Some("retry-7"));
        created_reply(req_id, expected_cwd, "running")
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
            "--inherit-agent",
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
    for status in ["exited", "error"] {
        let temp = farhelm_teststate::tempdir().unwrap();
        let socket = temp.path().join("supervisor.sock");
        let (done, server) = mock_supervisor(&socket, move |request| {
            let (req_id, cwd, ..) = inheriting_spawn(request);
            created_reply(req_id, cwd.unwrap_or_default(), status)
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
    assert!(stderr.contains("unexpected agent reply"), "{stderr}");
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
        inheriting_spawn(request);
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
            let (req_id, cwd, ..) = inheriting_spawn(request);
            assert_eq!(
                cwd.as_deref(),
                Some(expected_wire.as_str()),
                "cwd for input {sent:?}"
            );
            created_reply(req_id, expected_wire, "running")
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

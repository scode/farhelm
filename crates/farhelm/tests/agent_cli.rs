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
    AgentHost, AgentOutcome, AgentReply, AgentSession, AgentVerb, ControlMsg, ErrorKind,
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
    std::thread::JoinHandle<()>,
) {
    let std_listener = std::os::unix::net::UnixListener::bind(socket).expect("bind socket");
    std_listener.set_nonblocking(true).unwrap();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::UnixListener::from_std(std_listener).unwrap();
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
                        panic!("the agent CLI must authenticate in its hello: {hello:?}");
                    };
                    assert_eq!(auth.session_id, "session-1");
                    assert_eq!(auth.token, "secret");
                    let frame = tokio::time::timeout(Duration::from_secs(5), reader.read_frame())
                        .await
                        .expect("the agent CLI did not send a request")
                        .unwrap()
                        .expect("agent request");
                    if let Some(reply) = respond(parse_control(&frame).unwrap()) {
                        writer.write_control(&reply).await.unwrap();
                    }
                });
        }))
        .map_err(|panic| {
            panic.downcast_ref::<&str>().map_or_else(
                || "mock supervisor panicked".to_string(),
                |s| (*s).to_string(),
            )
        });
        let _ = done_tx.send(result);
    });
    (done_rx, thread)
}

/// Join only after the server has reported completion, so a missing request
/// or a failed assertion inside it gets a deadline of its own instead of
/// hanging the run.
fn finish_server(
    done: std::sync::mpsc::Receiver<Result<(), String>>,
    thread: std::thread::JoinHandle<()>,
) {
    done.recv_timeout(Duration::from_secs(7))
        .expect("mock supervisor did not finish")
        .expect("mock supervisor failed");
    thread.join().expect("join mock supervisor");
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
/// Every call site passes a single-element slice for `hosts`/`sessions`
/// (no lifecycle verb's extra positional or `--session` arguments) —
/// there is no separate single-verb wrapper, since a slice literal at the
/// call site is exactly as short as a forwarding function would be.
fn agent_command_with_args(socket: &std::path::Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_farhelm"));
    for name in [
        "FARHELM_SESSION_ID",
        "FARHELM_SESSION_TOKEN",
        "FARHELM_SUPERVISOR_SOCK",
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
#[test]
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
#[test]
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
                            host: "this machine".to_string(),
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
                            host: "builder".to_string(),
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
                            host: "builder".to_string(),
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
#[test]
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
                        host: "this machine".to_string(),
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
#[test]
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

/// Spec: the injected-environment contract is validated before any socket
/// is opened, and a missing supervisor socket is named rather than guessed
/// around.
///
/// `farhelm agent` shares `spawn_environment` with `farhelm spawn` on
/// purpose — one definition of what a Farhelm session guarantees — and this
/// pins that the sharing is real rather than a coincidence of two similar
/// code paths.
#[test]
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
#[test]
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
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, reply);
    let output = output_with_timeout(agent_command_with_args(&socket, &[verb]));
    finish_server(done, thread);
    output
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
#[test]
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
#[test]
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

// ---------------------------------------------------------------
// The three lifecycle verbs: what `farhelm agent rename/stop/archive`
// sends, and the one confirmation line each prints on success.
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
#[test]
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
                        host: "this machine".to_string(),
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
#[test]
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
/// target and prints the archived row's id.
#[test]
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
                        host: "this machine".to_string(),
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
/// this CLI ever prints, extended here to the lifecycle confirmations.
///
/// A title is user-supplied fleet-wide text (the same field the `sessions`
/// table's TITLE column escapes), so a control character in it must not
/// reach the terminal raw: a bare `\x07` rings the bell and a raw newline
/// would let a hostile or careless title split the confirmation into two
/// lines, the second of which a script parsing "one line means success"
/// would not expect.
#[test]
fn a_rename_confirmation_escapes_control_characters_in_the_title() {
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
                        id: "sess-1".to_string(),
                        host: "this machine".to_string(),
                        title: "line one\nline two".to_string(),
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
        stdout, "renamed sess-1 to \"line one\\nline two\"\n",
        "the embedded newline must be escaped, not printed raw"
    );
    assert_eq!(
        stdout.lines().count(),
        1,
        "one confirmation must stay one line even when the title is not: {stdout:?}"
    );
}

/// Spec: a stop confirmation escapes control characters the same way a
/// rename confirmation does — the sibling coverage
/// [`a_rename_confirmation_escapes_control_characters_in_the_title`]
/// establishes for `rename` alone.
///
/// `Stop`'s reply carries no fields (`AgentReply::Stopped` is empty), so
/// the escaped value here is the TARGET id this process itself resolved —
/// in this case the explicit `--session` argument — rather than anything
/// read back from the helm. That argument is still fleet-wide-reachable
/// text in the sense that matters: an agent forwarding an id it read from
/// elsewhere (another session's listing, say) could carry one, and the
/// printed confirmation must not let it forge a second line.
#[test]
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

/// Spec: an archive confirmation escapes control characters the same way a
/// rename confirmation does — the sibling coverage
/// [`a_rename_confirmation_escapes_control_characters_in_the_title`]
/// establishes for `rename` alone.
///
/// Unlike `stop` above, `archive`'s confirmation prints the REPLY's own
/// `AgentSession::id` (see `main`'s `Archive` arm), so the control
/// character here is planted in the mock's reply rather than in a CLI
/// argument — proving the escaping applies to peer-supplied data, not only
/// to this process's own resolved target.
#[test]
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
                        host: "this machine".to_string(),
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
#[test]
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
                        host: "this machine".to_string(),
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
#[test]
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
                        host: "this machine".to_string(),
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
#[test]
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
                        host: "this machine".to_string(),
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
#[test]
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
#[test]
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

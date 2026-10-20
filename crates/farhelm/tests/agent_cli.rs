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

use farhelm_proto::io::{FrameReader, FrameWriter};
use farhelm_proto::{
    AgentHost, AgentOutcome, AgentReply, AgentSession, AgentVerb, ControlMsg, ErrorKind, Frame,
};
use std::process::{Command, Output};
use std::time::Duration;

mod cli_support;
use cli_support::output_with_timeout;
#[path = "cli_support/mock_supervisor.rs"]
mod mock_supervisor;
use mock_supervisor::{finish_server, mock_supervisor_frames};

/// The session every test here runs as.
const SESSION: &str = "session-1";

/// Distinct entry identities let tests distinguish equal bare IDs on different hosts.
const CALLER_ENTRY: &str = "11111111-1111-4111-8111-111111111111";
const OTHER_ENTRY: &str = "22222222-2222-4222-8222-222222222222";

/// [`mock_supervisor::mock_supervisor`] for this file's session.
fn mock_supervisor(
    socket: &std::path::Path,
    respond: impl FnOnce(ControlMsg) -> Option<ControlMsg> + Send + 'static,
) -> (
    std::sync::mpsc::Receiver<Result<(), String>>,
    farhelm_teststate::thread::FixtureThread,
) {
    mock_supervisor::mock_supervisor(socket, SESSION, respond)
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
            create_unaccepted: false,
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
                    caller_host_id: "host-local".to_string(),
                    hosts: vec![
                        AgentHost {
                            id: "host-local".to_string(),
                            name: "this machine".to_string(),
                            kind: "local".to_string(),
                            state: "connected".to_string(),
                            current: true,
                        },
                        AgentHost {
                            id: "host-builder".to_string(),
                            name: "builder".to_string(),
                            kind: "ssh".to_string(),
                            state: "unreachable-reprobing".to_string(),
                            current: false,
                        },
                    ],
                    complete: true,
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
            "  ID           NAME         KIND  STATE",
            "* host-local   this machine local connected",
            "  host-builder builder      ssh   unreachable-reprobing",
            "",
        ]
        .join("\n")
    );
}

/// JSON discovery is a versioned envelope with explicit caller identity and
/// the reply's completeness signal.
///
/// Parsing and comparing the whole value pins the public schema while
/// avoiding a meaningless dependency on object-key serialization order.
#[farhelm_testtrace::test]
fn hosts_json_has_the_exact_discovery_envelope() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        assert_eq!(request, AgentVerb::Hosts {});
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Hosts {
                    caller_host_id: "11111111-1111-4111-8111-111111111111".to_string(),
                    hosts: vec![AgentHost {
                        id: "11111111-1111-4111-8111-111111111111".to_string(),
                        name: "this machine".to_string(),
                        kind: "local".to_string(),
                        state: "connected".to_string(),
                        current: true,
                    }],
                    complete: false,
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(&socket, &["hosts", "--json"]));
    finish_server(done, thread);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON envelope");
    assert_eq!(
        value,
        serde_json::json!({
            "schema_version": 7,
            "caller": {"session_id": "11111111-1111-4111-8111-111111111111:session-1", "host_id": "11111111-1111-4111-8111-111111111111"},
            "reply": {
                "reply": "hosts",
                "caller_host_id": "11111111-1111-4111-8111-111111111111",
                "hosts": [{
                    "id": "11111111-1111-4111-8111-111111111111",
                    "name": "this machine",
                    "kind": "local",
                    "state": "connected",
                    "current": true
                }],
                "complete": false
            }
        })
    );
}

/// Session discovery must expose full row and caller IDs while retaining the bare
/// authenticated request. Comparing the whole envelope catches a schema bump that
/// only fixes hosts or templates, or accidentally reuses an unrelated row as caller.
#[farhelm_testtrace::test]
fn sessions_json_has_the_exact_entry_bound_discovery_envelope() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, owner) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id,
            session_id,
            request,
        } = request
        else {
            panic!("expected an authenticated agent request, got {request:?}");
        };
        assert_eq!(session_id, SESSION);
        assert_eq!(request, AgentVerb::Sessions {});
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Sessions {
                    caller_host_id: CALLER_ENTRY.to_string(),
                    sessions: vec![AgentSession {
                        id: format!("{OTHER_ENTRY}:other:session"),
                        host_id: OTHER_ENTRY.to_string(),
                        host: Some("builder".to_string()),
                        title: "review".to_string(),
                        cwd: "/w".to_string(),
                        agent: "codex".to_string(),
                        status: "idle".to_string(),
                        current: false,
                        restart_offer: farhelm_proto::RestartOffer::Resume,
                        stale: true,
                    }],
                    truncated: true,
                },
            },
        })
    });
    let output = output_with_timeout(agent_command_with_args(&socket, &["sessions", "--json"]));
    finish_server(done, owner);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).expect("JSON envelope"),
        serde_json::json!({
            "schema_version": 7,
            "caller": {"session_id": format!("{CALLER_ENTRY}:{SESSION}"), "host_id": CALLER_ENTRY},
            "reply": {
                "reply": "sessions",
                "caller_host_id": CALLER_ENTRY,
                "sessions": [{
                    "id": format!("{OTHER_ENTRY}:other:session"),
                    "host_id": OTHER_ENTRY,
                    "host": "builder",
                    "title": "review",
                    "cwd": "/w",
                    "agent": "codex",
                    "status": "idle",
                    "current": false,
                    "restart_offer": "resume",
                    "stale": true
                }],
                "truncated": true
            }
        })
    );
}

/// Human discovery must print reusable full selectors, including the longest
/// admitted ID, and keep the reply's own-row marker when the credential is bare.
/// A 48-character description clamp would silently turn UUID:UUID into a target
/// that no command can resolve, so both the normal and maximum lengths matter.
#[farhelm_testtrace::test]
fn sessions_table_keeps_full_selectors_and_the_callers_marker() {
    const BARE_UUID: &str = "33333333-3333-4333-8333-333333333333";
    let own = format!("{CALLER_ENTRY}:{BARE_UUID}");
    let longest = format!("{OTHER_ENTRY}:{}", "x".repeat(40));
    assert_eq!(own.len(), 73);
    assert_eq!(
        longest.len(),
        farhelm_proto::session_ref::MAX_FULL_SESSION_ID_BYTES
    );
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let expected_own = own.clone();
    let expected_longest = longest.clone();
    let (done, owner) = mock_supervisor::mock_supervisor(&socket, BARE_UUID, move |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("expected an agent request, got {request:?}");
        };
        assert_eq!(request, AgentVerb::Sessions {});
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Sessions {
                    caller_host_id: CALLER_ENTRY.to_string(),
                    sessions: [
                        (expected_own.clone(), CALLER_ENTRY, true),
                        (expected_longest.clone(), OTHER_ENTRY, false),
                    ]
                    .into_iter()
                    .map(|(id, entry, current)| AgentSession {
                        id,
                        host_id: entry.to_string(),
                        host: Some("builder".to_string()),
                        title: "review".to_string(),
                        cwd: "/w".to_string(),
                        agent: "codex".to_string(),
                        status: "idle".to_string(),
                        current,
                        restart_offer: farhelm_proto::RestartOffer::Resume,
                        stale: false,
                    })
                    .collect(),
                    truncated: false,
                },
            },
        })
    });
    let mut command = agent_command_with_args(&socket, &["sessions"]);
    command.env("FARHELM_SESSION_ID", BARE_UUID);
    let output = output_with_timeout(command);
    finish_server(done, owner);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let table = String::from_utf8(output.stdout).unwrap();
    let rows: Vec<_> = table.lines().collect();
    assert_eq!(rows.len(), 3, "{table}");
    let marked: Vec<_> = rows[1].split_whitespace().take(2).collect();
    assert_eq!(marked, ["*", own.as_str()], "{table}");
    assert_eq!(
        rows[2].split_whitespace().next(),
        Some(longest.as_str()),
        "{table}"
    );
}

/// Observe stderr at mutation receipt, while the mock still owns the reply.
///
/// A final-output assertion alone would also pass if the CLI warned after restart
/// had already been dispatched. Private files expose that earlier boundary without
/// a pipe reader or a sleep. The child has a ten-second deadline and is killed and
/// reaped on expiry; the mock owner cancels pending accepts on assertion unwind.
async fn assert_restart_warning_at_dispatch(
    asking: &'static str,
    target: String,
    discover: bool,
    warn: bool,
) {
    const WARNING: &str = "warning: restarting this session can interrupt the invoking CLI; \
        acknowledgement can be lost, and resumed task continuation is not guaranteed\n";
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let stderr_path = temp.path().join("stderr");
    let stdout_path = temp.path().join("stdout");
    let stderr = std::fs::File::create(&stderr_path).unwrap();
    let stdout = std::fs::File::create(&stdout_path).unwrap();
    let observed_stderr = stderr_path.clone();
    let expected_target = target.clone();
    let mut received = 0;
    let (done, owner) = mock_supervisor::mock_supervisor_frames_sequence(
        &socket,
        asking,
        if discover { 2 } else { 1 },
        move |request| {
            let ControlMsg::AgentRequest {
                req_id,
                session_id,
                request,
            } = request
            else {
                panic!("expected an authenticated agent request, got {request:?}");
            };
            assert_eq!(session_id, asking, "wire authentication must remain bare");
            let reply = if discover && received == 0 {
                assert_eq!(
                    request,
                    AgentVerb::Hosts {},
                    "discovery must precede mutation"
                );
                assert_eq!(std::fs::read_to_string(&observed_stderr).unwrap(), "");
                AgentReply::Hosts {
                    caller_host_id: CALLER_ENTRY.to_string(),
                    hosts: Vec::new(),
                    complete: false,
                }
            } else {
                assert_eq!(
                    request,
                    AgentVerb::Restart {
                        session_id: Some(expected_target.clone()),
                        stop_if_running: true,
                    }
                );
                assert_eq!(
                    std::fs::read_to_string(&observed_stderr).unwrap(),
                    if warn { WARNING } else { "" },
                    "the self warning must exist before the destructive request arrives",
                );
                AgentReply::Restarted {
                    session: AgentSession {
                        id: expected_target.clone(),
                        host_id: if warn { CALLER_ENTRY } else { OTHER_ENTRY }.to_string(),
                        host: Some("builder".to_string()),
                        title: "review".to_string(),
                        cwd: "/w".to_string(),
                        agent: "codex".to_string(),
                        status: "running".to_string(),
                        current: warn,
                        restart_offer: farhelm_proto::RestartOffer::Resume,
                        stale: false,
                    },
                }
            };
            received += 1;
            Some(Frame::control(&ControlMsg::AgentResponse {
                req_id,
                outcome: AgentOutcome::Ok { reply },
            }))
        },
    );
    let mut child = {
        let mut command = agent_command_with_args(
            &socket,
            &["restart", "--session", &target, "--stop-if-running"],
        );
        command.env("FARHELM_SESSION_ID", asking);
        let mut command = tokio::process::Command::from(command);
        command
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(stdout)
            .stderr(stderr);
        command.spawn().expect("spawn restart CLI")
    };
    let status = match tokio::time::timeout(Duration::from_secs(10), child.wait()).await {
        Ok(status) => status.expect("wait for restart CLI"),
        Err(_) => {
            child
                .kill()
                .await
                .expect("kill and reap timed-out restart CLI");
            panic!("restart CLI exceeded its deadline");
        }
    };
    finish_server(done, owner);
    assert_eq!(
        status.code(),
        Some(0),
        "{}",
        std::fs::read_to_string(&stderr_path).unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(stdout_path).unwrap(),
        format!("restarted {target}\n")
    );
    assert_eq!(
        std::fs::read_to_string(stderr_path).unwrap(),
        if warn { WARNING } else { "" }
    );
}

/// A full self target must be identified from the caller entry and warned about
/// before restart can tear down the invoking CLI. Equal suffixes on another entry
/// must still dispatch the requested target without claiming the caller will die.
#[farhelm_testtrace::test]
async fn full_self_restart_warns_before_dispatch_but_a_foreign_equal_suffix_does_not() {
    assert_restart_warning_at_dispatch(SESSION, format!("{CALLER_ENTRY}:{SESSION}"), true, true)
        .await;
    assert_restart_warning_at_dispatch(SESSION, format!("{OTHER_ENTRY}:{SESSION}"), true, false)
        .await;
}

/// Exact own-bare equality wins before full-reference parsing, even for an opaque
/// bare ID that looks qualified. An unrelated suffix needs no discovery request;
/// these single-request mocks fail if either path accidentally consults the helm.
#[farhelm_testtrace::test]
async fn bare_self_and_unrelated_restart_targets_do_not_discover() {
    const LOOKS_QUALIFIED: &str = "22222222-2222-4222-8222-222222222222:id";
    assert_restart_warning_at_dispatch(SESSION, SESSION.to_string(), false, true).await;
    assert_restart_warning_at_dispatch(LOOKS_QUALIFIED, LOOKS_QUALIFIED.to_string(), false, true)
        .await;
    assert_restart_warning_at_dispatch(SESSION, format!("{OTHER_ENTRY}:unrelated"), false, false)
        .await;
}

/// An entry-discovery refusal must end the possible self restart before any
/// mutation goes out. Reporting a lost mutation here would invent uncertainty:
/// this CLI has sent only the read-only Hosts request the mock asserts below.
#[farhelm_testtrace::test]
fn full_self_restart_discovery_refusal_does_not_claim_a_mutation() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, owner) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("expected an agent request, got {request:?}");
        };
        assert_eq!(request, AgentVerb::Hosts {});
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Err {
                kind: ErrorKind::Unavailable,
                message: "no helm is attached to this session".to_string(),
            },
        })
    });
    let target = format!("{CALLER_ENTRY}:{SESSION}");
    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &["restart", "--session", &target],
    ));
    finish_server(done, owner);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("no helm is attached to this session"),
        "{stderr}"
    );
    assert!(!stderr.contains("outcome is unknown"), "{stderr}");
    assert!(!stderr.contains("warning: restarting"), "{stderr}");
}

/// Spec: `farhelm agent templates --json` sends the `Templates` verb and
/// prints the versioned envelope with each template's name, its fields
/// without command text, the `sets_*` flags, shortcut assignments and the
/// host name; the table form lists the same template by name with what it
/// sets and which shortcuts select it. A named template shows its effective
/// policy even when the wire reply omits the default Base.
///
/// Why: the listing is an agent's only view of templates, read as JSON when
/// exact names matter; the envelope is a public schema like the hosts one.
#[farhelm_testtrace::test]
fn templates_json_and_table_list_what_each_template_sets() {
    use farhelm_proto::launcher::TemplateNameKind;
    for (title, kind) in [
        (None, None),
        (Some("review"), None),
        (Some("review"), Some(TemplateNameKind::Fixed)),
    ] {
        let reply = move || {
            Some(AgentReply::Templates {
                caller_host_id: "11111111-1111-4111-8111-111111111111".to_string(),
                templates: vec![farhelm_proto::AgentTemplate {
                    name: "my-codex".to_string(),
                    fields: farhelm_proto::launcher::TemplateFields {
                        agent: Some(farhelm_proto::LaunchHarness::Codex),
                        name: title.map(str::to_string),
                        name_kind: kind,
                        ..Default::default()
                    },
                    sets_command: false,
                    sets_resume_command: false,
                    host_name: None,
                    quick_launch: true,
                    key: Some("3".to_string()),
                }],
            })
        };
        let run = |args: &[&str]| {
            let temp = farhelm_teststate::tempdir().unwrap();
            let socket = temp.path().join("supervisor.sock");
            let (done, thread) = mock_supervisor(&socket, move |request| {
                let ControlMsg::AgentRequest {
                    req_id, request, ..
                } = request
                else {
                    panic!("farhelm agent must send an AgentRequest, got {request:?}");
                };
                assert_eq!(request, AgentVerb::Templates {});
                Some(ControlMsg::AgentResponse {
                    req_id,
                    outcome: AgentOutcome::Ok {
                        reply: reply().expect("a reply"),
                    },
                })
            });
            let output = output_with_timeout(agent_command_with_args(&socket, args));
            finish_server(done, thread);
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        };
        let value: serde_json::Value =
            serde_json::from_slice(&run(&["templates", "--json"])).expect("JSON envelope");
        let mut expected_fields = serde_json::json!({"agent": "codex"});
        let policy = if kind == Some(TemplateNameKind::Fixed) {
            "fixed"
        } else {
            "base"
        };
        if let Some(title) = title {
            expected_fields["name"] = title.into();
            expected_fields["name_kind"] = policy.into();
        }
        assert_eq!(
            value,
            serde_json::json!({
                "schema_version": 7,
                "caller": {"session_id": "11111111-1111-4111-8111-111111111111:session-1", "host_id": "11111111-1111-4111-8111-111111111111"},
                "reply": {
                    "reply": "templates",
                    "caller_host_id": "11111111-1111-4111-8111-111111111111",
                    "templates": [{
                        "name": "my-codex",
                        "fields": expected_fields,
                        "sets_command": false,
                        "sets_resume_command": false,
                        "host_name": null,
                        "quick_launch": true,
                        "key": "3"
                    }]
                }
            })
        );
        let table = String::from_utf8(run(&["templates"])).expect("UTF-8");
        let named = title
            .map(|title| format!(", title \"{title}\" ({policy} name)"))
            .unwrap_or_default();
        assert_eq!(
            table,
            format!("NAME     SETS\nmy-codex agent codex{named}, quick launch, number key 3\n")
        );
    }
}

/// Spec: `farhelm agent sessions` sends the `Sessions` verb and renders the
/// marked table, with staleness visible in the STATUS column and
/// the non-secret restart capability in OFFER.
///
/// The column ORDER is the contract being pinned. `farhelm agent` is a
/// public, scriptable CLI whose output is read by humans and models with no
/// schema to consult, so the column layout is the whole interface: a
/// reordering silently changes what every existing wrapper, alias and
/// transcript means by "the fourth column".
///
/// A cached row from an unreachable host is indistinguishable from a live
/// one without the `(stale)` mark SPEC.md requires.
#[farhelm_testtrace::test]
fn sessions_prints_the_marked_table_with_staleness() {
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
                    caller_host_id: "host-local".to_string(),
                    sessions: vec![
                        AgentSession {
                            id: "session-1".to_string(),
                            host_id: "1".to_string(),
                            host: Some("this machine".to_string()),
                            title: "auth".to_string(),
                            cwd: "/w/auth".to_string(),
                            agent: "claude".to_string(),
                            status: "running".to_string(),
                            current: true,
                            restart_offer: Default::default(),
                            stale: false,
                        },
                        AgentSession {
                            id: "session-2".to_string(),
                            host_id: "1".to_string(),
                            host: Some("builder".to_string()),
                            title: "docs".to_string(),
                            cwd: "/w".to_string(),
                            agent: "codex".to_string(),
                            status: "idle".to_string(),
                            current: false,
                            restart_offer: Default::default(),
                            stale: true,
                        },
                        AgentSession {
                            id: "session-3".to_string(),
                            host_id: "1".to_string(),
                            host: Some("builder".to_string()),
                            title: "old".to_string(),
                            cwd: "/w".to_string(),
                            agent: "codex".to_string(),
                            status: "exited".to_string(),
                            current: false,
                            restart_offer: Default::default(),
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
            "  ID        HOST         TITLE CWD     AGENT  STATUS       OFFER",
            "* session-1 this machine auth  /w/auth claude running      not-captured",
            "  session-2 builder      docs  /w      codex  idle (stale) not-captured",
            "  session-3 builder      old   /w      codex  exited       not-captured",
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
                    caller_host_id: "host-local".to_string(),
                    sessions: vec![AgentSession {
                        id: "session-1".to_string(),
                        host_id: "1".to_string(),
                        host: Some("this machine".to_string()),
                        title: "auth".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "running".to_string(),
                        current: true,
                        restart_offer: Default::default(),
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
    let (done, thread) = mock_supervisor_frames(&socket, SESSION, reply);
    // These transport tests require an admitted request. Stop names its
    // fixture target explicitly; omission refusal has separate parser tests.
    let args = if matches!(verb, "stop" | "delete") {
        vec![verb, "--session", "target-session"]
    } else {
        vec![verb]
    };
    let output = output_with_timeout(agent_command_with_args(&socket, &args));
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
                reply: AgentReply::Hosts {
                    caller_host_id: "host-local".to_string(),
                    hosts: Vec::new(),
                    complete: true,
                },
            },
        })
    });
    // A perfectly valid message that answers nothing.
    let unrelated = agent_run_against("hosts", |_| {
        Some(ControlMsg::Detached {
            channel: 1,
            reason: "not an answer".to_string(),
            code: farhelm_proto::DetachCode::Other,
        })
    });
    // The uncorrelated refusal shape (`req_id` 0), which must still be
    // rendered verbatim rather than treated as a protocol violation.
    let bare_error = agent_run_against("hosts", |_| {
        Some(ControlMsg::Error {
            create_unaccepted: false,
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
                    caller_host_id: "host-local".to_string(),
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
                reply: AgentReply::Hosts {
                    caller_host_id: "host-local".to_string(),
                    hosts: Vec::new(),
                    complete: true,
                },
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
                        code: farhelm_proto::DetachCode::Other,
                    })
                }),
            ),
            (
                "a success reply of the wrong shape",
                agent_run_against(verb, |request| {
                    Some(ControlMsg::AgentResponse {
                        req_id: asked_req_id(&request),
                        outcome: AgentOutcome::Ok {
                            reply: AgentReply::Hosts {
                                caller_host_id: "host-local".to_string(),
                                hosts: Vec::new(),
                                complete: true,
                            },
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
// The lifecycle verbs: what `farhelm agent rename/stop`
// sends, and the one confirmation line each prints on success.
//
// "Prints on success" holds unconditionally against a MOCK and not against
// the real stack: an explicit self-stop targets the asking
// session's process tree, whose marker-keyed sweep reaches this CLI process
// too, so it can be SIGTERMed before its own `println!` runs (see
// `main`'s Rename/Stop comment, and the e2e lifecycle test that
// routes around it). Every case below sends its verb to a mock that stops
// nothing, which is what makes the confirmation observable at all.
// ---------------------------------------------------------------

/// Spec: `farhelm agent rename --session <id> --expected-title=<old> --
/// <new>` sends exactly one conditional `Rename` verb and prints the
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
                expected_title: Some("old title".to_string()),
                title: "new title".to_string(),
            }
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Session {
                    session: AgentSession {
                        id: "resolved-session".to_string(),
                        host_id: "1".to_string(),
                        host: Some("this machine".to_string()),
                        title: "the helm's own title".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "running".to_string(),
                        current: false,
                        restart_offer: Default::default(),
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &[
            "rename",
            "new title",
            "--session",
            "other-session",
            "--expected-title",
            "old title",
        ],
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

/// Spec: `farhelm agent stop` and `delete` refuse an omitted `--session` before opening
/// the supervisor socket.
///
/// This keeps the command-line boundary from spelling the old implicit-self
/// request at all. The wire and authority tests separately prove that an old
/// client cannot bypass this check by sending an omitted-target request.
#[farhelm_testtrace::test]
fn stop_and_delete_without_a_session_are_refused_before_anything_is_sent() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    for verb in ["stop", "delete"] {
        let output = output_with_timeout(agent_command_with_args(&socket, &[verb]));
        assert_eq!(output.status.code(), Some(2), "clap's usage-error status");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("--session"),
            "the refusal must name it: {stderr}"
        );
    }
}

/// Delete's CLI preserves explicit consent and reports checkout recovery
/// advice separately from the machine-readable success line. A lost reply
/// remains unknown, since a removed session cannot acknowledge a retry.
#[farhelm_testtrace::test]
fn delete_transmits_consent_reports_notices_and_keeps_lost_outcomes_unknown() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id,
            request:
                AgentVerb::Delete {
                    session_id,
                    stop_if_running,
                },
            ..
        } = request
        else {
            panic!("expected Delete request, got {request:?}");
        };
        assert_eq!(session_id.as_deref(), Some("target-session"));
        assert!(stop_if_running);
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Deleted {
                    outcome: farhelm_proto::SessionDeleteOutcome {
                        notice: Some("checkout preserved\ninspect it".into()),
                        archived: false,
                    },
                },
            },
        })
    });
    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &["delete", "--session", "target-session", "--stop-if-running"],
    ));
    finish_server(done, thread);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "deleted target-session\n"
    );
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "checkout preserved\\ninspect it\n"
    );
    let lost = agent_run_against("delete", |_| None);
    assert!(!lost.status.success());
    assert!(lost.stdout.is_empty());
    assert!(String::from_utf8(lost.stderr).unwrap().contains("outcome"));
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
                        host_id: "1".to_string(),
                        host: Some("this machine".to_string()),
                        title: "line one\nsays \"hi\" \\ bye".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "running".to_string(),
                        current: true,
                        restart_offer: Default::default(),
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &[
            "rename",
            "line one\nline two",
            "--session",
            "session-1",
            "--expected-title",
            "old title",
        ],
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
/// Spec: rename requires both the target id and the title observed during
/// discovery before it opens the supervisor socket.
///
/// Requiring both values makes the command conditional by construction: an
/// agent cannot accidentally recover the old implicit-self or unconditional
/// rename behavior by omitting one flag.
#[farhelm_testtrace::test]
fn rename_without_consequential_selectors_is_refused_before_sending() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let output = output_with_timeout(agent_command_with_args(&socket, &["rename", "new title"]));

    assert_eq!(output.status.code(), Some(2), "clap's usage-error status");
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("--session"),
        "the refusal must name the target selector: {stderr}"
    );
    assert!(
        stderr.contains("--expected-title"),
        "the refusal must name the condition: {stderr}"
    );
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
                session_id: Some("session-1".to_string()),
                expected_title: Some("-old-title".to_string()),
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
                        host_id: "1".to_string(),
                        host: Some("this machine".to_string()),
                        title: "-not-a-flag".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "running".to_string(),
                        current: true,
                        restart_offer: Default::default(),
                        stale: false,
                    },
                },
            },
        })
    });

    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &[
            "rename",
            "-not-a-flag",
            "--session",
            "session-1",
            "--expected-title=-old-title",
        ],
    ));
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
        "delete",
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
                templates: Vec::new(),
                edits: farhelm_proto::launcher::TemplateFields {
                    kind: Some(farhelm_proto::launcher::LauncherKind::Command),
                    agent: Some(farhelm_proto::LaunchHarness::Claude),
                    command: Some("claude --model opus {farhelm_args}".to_string()),
                    yolo: Some(true),
                    resume_command: Some(Some(
                        "claude --resume {conversation} {farhelm_args}".to_string()
                    )),
                    destination: Some(farhelm_proto::launcher::TemplateDestination::Folder(
                        "/srv/work".to_string()
                    )),
                    name: Some("over there".to_string()),
                    ..Default::default()
                },
                intent_key: Some("key-1".to_string()),
                confirm_yolo: false,
                spawn: None,
            }
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Created {
                    session: AgentSession {
                        id: "new-session".to_string(),
                        host_id: "1".to_string(),
                        host: Some("builder".to_string()),
                        title: "over there".to_string(),
                        cwd: "/srv/work".to_string(),
                        agent: "Claude Code".to_string(),
                        status: String::new(),
                        current: false,
                        restart_offer: Default::default(),
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
            "--command",
            "claude --model opus {farhelm_args}",
            "--yolo",
            "--agent",
            "claude",
            "--resume-command",
            "claude --resume {conversation} {farhelm_args}",
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

/// The shared flags must send message contents rather than a file path, with
/// newlines, tabs and leading hyphens intact. Feeding stdin from a file keeps
/// the child bounded without changing the test runner's own environment.
#[farhelm_testtrace::test]
fn create_initial_prompt_reads_literal_file_and_stdin_text() {
    const TEXT: &str = "--help\n\t{farhelm_args}";
    let temp = farhelm_teststate::tempdir().unwrap();
    let file = temp.path().join("message.txt");
    std::fs::write(&file, TEXT).unwrap();
    for (case, (flag, value)) in [
        ("--prompt", TEXT),
        ("--prompt-file", file.to_str().unwrap()),
        ("--prompt-file", "-"),
    ]
    .into_iter()
    .enumerate()
    {
        let socket = temp.path().join(format!("supervisor-{case}.sock"));
        let (done, owner) = mock_supervisor(&socket, |request| {
            let ControlMsg::AgentRequest {
                req_id,
                request: AgentVerb::Create { edits, .. },
                ..
            } = request
            else {
                panic!("expected create, got {request:?}");
            };
            assert_eq!(edits.prompt.as_deref(), Some(TEXT));
            Some(ControlMsg::AgentResponse {
                req_id,
                outcome: AgentOutcome::Err {
                    kind: ErrorKind::InvalidRequest,
                    message: "observed literal prompt".into(),
                },
            })
        });
        let command = agent_command_with_args(
            &socket,
            &[
                "create", "--host", "local", "--cwd", "/work", "--agent", "codex", flag, value,
            ],
        );
        // The bounded runner closes stdin before spawning. A shell performs
        // only this fixture's file redirection, then replaces itself with the
        // CLI. Arguments remain separate positional data; output
        // caps, wait ownership and the shared execution deadline stay intact.
        let command = if value == "-" {
            let mut redirected = Command::new("/bin/sh");
            redirected
                .args(["-c", "exec \"$@\" < \"$0\""])
                .arg(&file)
                .arg(command.get_program())
                .args(command.get_args());
            for (name, value) in command.get_envs() {
                match value {
                    Some(value) => {
                        redirected.env(name, value);
                    }
                    None => {
                        redirected.env_remove(name);
                    }
                }
            }
            redirected
        } else {
            command
        };
        let output = output_with_timeout(command);
        finish_server(done, owner);
        assert!(!output.status.success());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("observed literal prompt")
        );
    }
}

/// Invalid message sources must fail before a supervisor request is possible.
/// A file is bounded even though ordinary filesystem reads could be unlimited.
#[farhelm_testtrace::test]
fn initial_prompt_flags_refuse_controls_conflicts_and_oversized_files() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("not-listening.sock");
    for text in ["escape\u{1b}", "carriage\rreturn"] {
        let output = output_with_timeout(agent_command_with_args(
            &socket,
            &["create", "--prompt", text],
        ));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("control characters")
        );
    }
    let file = temp.path().join("too-long.txt");
    std::fs::write(&file, "x".repeat(64 * 1024 + 1)).unwrap();
    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &["create", "--prompt-file", file.to_str().unwrap()],
    ));
    assert!(String::from_utf8(output.stderr).unwrap().contains("64 KiB"));
    std::fs::write(&file, "message").unwrap();
    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &[
            "create",
            "--prompt",
            "message",
            "--prompt-file",
            file.to_str().unwrap(),
        ],
    ));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("cannot be used")
    );
}

// Running-agent prompts use the same bounded literal-message parser as create,
// but their command surface must not grow any of create's launch choices.
//
// The real helm currently refuses every prompt (`AGENT_PROMPTS_ENABLED` in
// the helm's `agent_requests`) and the verb is hidden from help, but it still
// parses. These tests answer from a scripted relay instead of a helm, so they
// keep covering the CLI half a re-enable would ship: parsing, the request it
// sends, and how replies are reported.

/// Each source must send its literal contents to the exact selected target.
/// Surrounding whitespace remains message data once the text is nonblank;
/// validating a follow-up must not trim what the user approves and receives.
/// Success reports delivery only.
#[farhelm_testtrace::test]
fn prompt_sends_literal_file_and_stdin_text() {
    const TEXT: &str = "--help\n\t{farhelm_args}";
    const PADDED_TEXT: &str = " \tmessage\n ";
    let temp = farhelm_teststate::tempdir().unwrap();
    let file = temp.path().join("message.txt");
    std::fs::write(&file, TEXT).unwrap();
    for (case, (flag, value, expected)) in [
        ("--prompt", TEXT, TEXT),
        ("--prompt-file", file.to_str().unwrap(), TEXT),
        ("--prompt-file", "-", TEXT),
        ("--prompt", PADDED_TEXT, PADDED_TEXT),
    ]
    .into_iter()
    .enumerate()
    {
        let socket = temp.path().join(format!("supervisor-{case}.sock"));
        let (done, owner) = mock_supervisor(&socket, move |request| {
            let ControlMsg::AgentRequest {
                req_id,
                session_id,
                request,
            } = request
            else {
                panic!("expected agent request, got {request:?}");
            };
            assert_eq!(session_id, SESSION);
            assert_eq!(
                request,
                AgentVerb::Prompt {
                    session_id: Some("target-session".into()),
                    prompt: expected.into(),
                }
            );
            Some(ControlMsg::AgentResponse {
                req_id,
                outcome: AgentOutcome::Ok {
                    reply: AgentReply::Prompted {},
                },
            })
        });
        let command = agent_command_with_args(
            &socket,
            &["prompt", "--session", "target-session", flag, value],
        );
        // The bounded runner closes stdin itself. As in the initial-prompt
        // fixture, redirect this owned file before exec without changing the
        // runner or exposing the test process to environment mutations.
        let command = if value == "-" {
            let mut redirected = Command::new("/bin/sh");
            redirected
                .args(["-c", "exec \"$@\" < \"$0\""])
                .arg(&file)
                .arg(command.get_program())
                .args(command.get_args());
            for (name, value) in command.get_envs() {
                match value {
                    Some(value) => {
                        redirected.env(name, value);
                    }
                    None => {
                        redirected.env_remove(name);
                    }
                }
            }
            redirected
        } else {
            command
        };
        let output = output_with_timeout(command);
        finish_server(done, owner);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert_eq!(output.stdout, b"prompt sent to target-session\n");
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}

/// Every source must reject blank contents before dialing the supervisor.
/// A supplied source alone does not make a safe follow-up: its submit key
/// could send text already waiting in the target's composer. Unicode spaces
/// establish that the rule is whitespace-aware rather than a length check.
#[farhelm_testtrace::test]
fn prompt_refuses_blank_literal_file_and_stdin_before_dialing() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("not-listening.sock");
    let file = temp.path().join("blank.txt");
    for text in ["", " \t\n ", "\u{a0}\u{2003}"] {
        std::fs::write(&file, text).unwrap();
        for (flag, value) in [
            ("--prompt", text),
            ("--prompt-file", file.to_str().unwrap()),
            ("--prompt-file", "-"),
        ] {
            let command =
                agent_command_with_args(&socket, &["prompt", "--session", "target", flag, value]);
            // The bounded runner owns stdin closure. As in the success
            // fixture, redirect this owned file in the child before exec
            // so stdin contents are exercised without changing that runner.
            let command = if value == "-" {
                let mut redirected = Command::new("/bin/sh");
                redirected
                    .args(["-c", "exec \"$@\" < \"$0\""])
                    .arg(&file)
                    .arg(command.get_program())
                    .args(command.get_args());
                for (name, value) in command.get_envs() {
                    match value {
                        Some(value) => {
                            redirected.env(name, value);
                        }
                        None => {
                            redirected.env_remove(name);
                        }
                    }
                }
                redirected
            } else {
                command
            };
            let output = output_with_timeout(command);
            assert_eq!(
                output.status.code(),
                Some(2),
                "{text:?}, {flag}: {output:?}"
            );
            assert!(output.stdout.is_empty(), "{output:?}");
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("must contain non-whitespace text"),
                "{text:?}, {flag}: {output:?}"
            );
        }
    }
}

/// A prompt requires a target and exactly one source, and cannot relaunch or
/// configure its target. Usage errors must happen before any supervisor dial.
#[farhelm_testtrace::test]
fn prompt_requires_target_and_one_source_and_refuses_launch_flags() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("not-listening.sock");
    let file = temp.path().join("message.txt");
    std::fs::write(&file, "message").unwrap();
    for (args, diagnostic) in [
        (vec!["prompt", "--prompt", "message"], "--session"),
        (vec!["prompt", "--session", "target"], "--prompt"),
        (
            vec![
                "prompt",
                "--session",
                "target",
                "--prompt",
                "message",
                "--prompt-file",
                file.to_str().unwrap(),
            ],
            "cannot be used",
        ),
    ] {
        let output = output_with_timeout(agent_command_with_args(&socket, &args));
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic),
            "{output:?}"
        );
    }
    for flag in ["--agent", "--template", "--command", "--idempotency-key"] {
        let output = output_with_timeout(agent_command_with_args(
            &socket,
            &[
                "prompt",
                "--session",
                "target",
                "--prompt",
                "message",
                flag,
                "value",
            ],
        ));
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("unexpected argument"), "{stderr}");
        assert!(stderr.contains(flag), "{stderr}");
    }
}

/// The running-agent command must keep the launch parser's byte and control
/// limits. File cases cover NUL (which cannot occur in argv), invalid UTF-8,
/// and cap-plus-one reads without relying on a live relay for validation.
#[farhelm_testtrace::test]
fn prompt_refuses_invalid_literal_and_file_contents_before_dialing() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("not-listening.sock");
    for text in ["escape\u{1b}", "carriage\rreturn"] {
        let output = output_with_timeout(agent_command_with_args(
            &socket,
            &["prompt", "--session", "target", "--prompt", text],
        ));
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("control characters"),
            "{output:?}"
        );
    }
    let file = temp.path().join("invalid.txt");
    for (contents, diagnostic) in [
        (b"contains\0nul".to_vec(), "control characters"),
        (vec![0xff], "UTF-8"),
        (vec![b'x'; 64 * 1024 + 1], "64 KiB"),
    ] {
        std::fs::write(&file, contents).unwrap();
        let output = output_with_timeout(agent_command_with_args(
            &socket,
            &[
                "prompt",
                "--session",
                "target",
                "--prompt-file",
                file.to_str().unwrap(),
            ],
        ));
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic),
            "{output:?}"
        );
    }
}

/// Empty success replies still need an action-specific tag. A stop reply
/// cannot confirm prompt delivery; after dispatch, such an answer leaves an
/// unknown outcome rather than permission to resend the message blindly.
#[farhelm_testtrace::test]
fn prompt_refuses_a_wrong_reply_and_reports_unknown_delivery() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, owner) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id,
            request: AgentVerb::Prompt { .. },
            ..
        } = request
        else {
            panic!("expected prompt, got {request:?}");
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
        &["prompt", "--session", "target", "--prompt", "message"],
    ));
    finish_server(done, owner);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("prompt delivery confirmation"), "{stderr}");
    assert!(stderr.contains("stop confirmation"), "{stderr}");
    assert!(stderr.contains("outcome is unknown"), "{stderr}");
}

/// The confirmation must stay one printable line even if target validation
/// changes later. A real relay rejects this hostile id; this mock isolates the
/// output layer's own obligation to escape it without printing prompt text.
#[farhelm_testtrace::test]
fn prompt_confirmation_escapes_the_target() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, owner) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest { req_id, .. } = request else {
            panic!("expected agent request, got {request:?}");
        };
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Prompted {},
            },
        })
    });
    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &[
            "prompt",
            "--session",
            "line one\nline two\u{1b}",
            "--prompt",
            "private message",
        ],
    ));
    finish_server(done, owner);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"prompt sent to line one\\nline two\\x1b\n");
    assert!(output.stderr.is_empty(), "{output:?}");
}

/// Spec: clone refuses omitted source and destination selectors before it
/// opens the supervisor socket.
///
/// The optional directory and title still inherit from the explicit source,
/// but neither the source nor its destination may be inferred from the
/// calling session.
#[farhelm_testtrace::test]
fn clone_without_source_or_host_is_refused_before_sending() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let output = output_with_timeout(agent_command_with_args(&socket, &["clone"]));

    assert_eq!(output.status.code(), Some(2), "clap's usage-error status");
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("--source-session"),
        "the refusal must name the source: {stderr}"
    );
    assert!(
        stderr.contains("--host"),
        "the refusal must name the destination: {stderr}"
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
                source_session_id: Some("source-session".to_string()),
                host: Some("builder".to_string()),
                cwd: Some("/srv/elsewhere".to_string()),
                title: Some("the copy".to_string()),
                intent_key: Some("-leading-hyphen-key".to_string()),
                confirm_yolo: false,
            }
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Created {
                    session: AgentSession {
                        id: "the-copy".to_string(),
                        host_id: "1".to_string(),
                        host: Some("buil\x1b[2Jder".to_string()),
                        title: "forged\nrow".to_string(),
                        cwd: "/srv/\tproject".to_string(),
                        agent: "Claude".to_string(),
                        status: String::new(),
                        current: false,
                        restart_offer: Default::default(),
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
            "--source-session",
            "source-session",
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

/// Spec: `farhelm agent create --profile`, removed with agent profiles, is
/// refused by the CLI itself with a message naming `--invocation`, with
/// nothing sent and nothing on stdout.
///
/// The parse-level refusal is unit-tested beside the flag; this pins it
/// through the real binary, where an agent meets it: exit status 2 and the
/// message on stderr. No mock supervisor is started, which is the sharp
/// end: if this ever regressed into sending the request, there would be no
/// socket to connect to and the failure would still be an error — so the
/// assertion is specifically that the removal message is what came back.
#[farhelm_testtrace::test]
fn create_with_the_removed_profile_flag_is_refused_before_anything_is_sent() {
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
            "--command",
            "sh",
            "--no-yolo",
        ],
    ));
    assert_eq!(output.status.code(), Some(2), "clap's usage-error status");
    assert!(output.stdout.is_empty(), "a refused create prints no id");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("agent profiles were removed") && stderr.contains("--command"),
        "the refusal must say why and name the replacement: {stderr}"
    );
}

/// Spec: `farhelm agent create --invocation`, replaced by `--command` with
/// launch kinds, is refused by the CLI itself naming `--command` and the
/// YOLO assertion; a create that makes both YOLO assertions, or one twice,
/// is refused naming the flags; and an unknown `--agent` is refused listing
/// the agent types. Nothing is sent and nothing reaches stdout.
///
/// Why: SPEC_impl.md's compatibility rule has a retired spelling refused
/// with its replacement named, so an agent following older instructions
/// learns the new form; and SPEC.md refuses a contradicted or repeated
/// command flag rather than resolving it by order. A MISSING assertion is
/// the helm's refusal, not clap's, because a template may supply it
/// (`farhelm-helm`'s `agent_launch` tests). No mock supervisor runs, so a
/// regression into sending would fail differently.
#[farhelm_testtrace::test]
fn create_refuses_retired_and_malformed_launch_flags() {
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    for (args, expected) in [
        (
            &[
                "create",
                "--cwd",
                "/w",
                "--host",
                "h",
                "--invocation",
                "sh",
                "--command",
                "sh",
                "--no-yolo",
            ][..],
            &[
                "--invocation was replaced by --command",
                "--yolo or --no-yolo",
            ][..],
        ),
        // The call an agent following older instructions actually makes.
        (
            &["create", "--cwd", "/w", "--host", "h", "--invocation", "sh"][..],
            &["--invocation was replaced by --command"][..],
        ),
        // The assertion is one answer, not two.
        (
            &[
                "create",
                "--cwd",
                "/w",
                "--host",
                "h",
                "--command",
                "sh",
                "--yolo",
                "--no-yolo",
            ][..],
            &["--yolo", "--no-yolo"][..],
        ),
        (
            &[
                "create",
                "--cwd",
                "/w",
                "--host",
                "h",
                "--command",
                "sh",
                "--yolo",
                "--yolo",
            ][..],
            &["--yolo"][..],
        ),
        // An unknown agent type lists the real ones.
        (
            &[
                "create",
                "--cwd",
                "/w",
                "--host",
                "h",
                "--command",
                "sh {farhelm_args}",
                "--no-yolo",
                "--agent",
                "bogus",
            ][..],
            &[
                "\"bogus\" is not an agent type",
                "claude",
                "codex",
                "open_code",
            ][..],
        ),
    ] {
        let output = output_with_timeout(agent_command_with_args(&socket, args));
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "a refused create prints no id");
        let stderr = String::from_utf8(output.stderr).unwrap();
        for needle in expected {
            assert!(stderr.contains(needle), "{args:?}: {stderr}");
        }
    }
}

/// Spec: every hyphen-leading VALUE a create can carry reaches the wire
/// verbatim rather than being read as an unrecognized flag.
///
/// The same hazard `a_rename_title_starting_with_a_hyphen_is_not_misparsed_
/// as_a_flag` covers for titles, applied to the whole creating surface.
/// Every one of these values is judged DOWNSTREAM — a host name by the
/// helm's registry, an idempotency key by its reservation table, a
/// directory by the target's filesystem — and every one of them may
/// legally begin with `-`, so a local refusal here is this CLI declining to
/// carry a value the far end would have accepted or explained.
///
/// The fixture's `--command` is deliberately option-shaped
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
            edits,
            intent_key,
            ..
        } = request
        else {
            panic!("expected a Create verb, got {request:?}");
        };
        assert_eq!(edits.command.as_deref(), Some("--weird-program --flag"));
        assert_eq!(host.as_deref(), Some("-odd-host"));
        assert_eq!(
            edits.destination,
            Some(farhelm_proto::launcher::TemplateDestination::Folder(
                "-odd-dir".to_string()
            ))
        );
        assert_eq!(edits.name.as_deref(), Some("-odd-title"));
        assert_eq!(intent_key.as_deref(), Some("-odd-key"));
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::Created {
                    session: AgentSession {
                        id: "new-session".to_string(),
                        host_id: "1".to_string(),
                        host: Some("this machine".to_string()),
                        title: "t".to_string(),
                        cwd: "/w".to_string(),
                        agent: "weird-program".to_string(),
                        status: String::new(),
                        current: false,
                        restart_offer: Default::default(),
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
            "--command",
            "--weird-program --flag",
            "--no-yolo",
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
/// created it — a target an agent might then go on to mutate. No
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
                        host_id: "1".to_string(),
                        host: Some("this machine".to_string()),
                        title: "t".to_string(),
                        cwd: "/w".to_string(),
                        agent: "claude".to_string(),
                        status: "running".to_string(),
                        current: false,
                        restart_offer: Default::default(),
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
            "--source-session",
            "source-session",
            "--host",
            "this machine",
        ],
    ));
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

/// Spec: `farhelm agent template create NAME` sends one `TemplateCreate`
/// carrying create's flags as the template's fields and shortcuts, `--host` by name
/// rather than as a field, and prints one confirmation line naming the
/// template; `--template` is refused before anything is sent.
///
/// Why: SPEC.md gives template writes `farhelm agent create`'s flags plus
/// `--cwd`, `--title`, `--title-kind` and `--host`, never `--template`: a template built
/// from other templates would copy command text the agent never saw into
/// one it wrote. The host travels by name so the helm, not the agent,
/// decides which install it pins.
#[farhelm_testtrace::test]
fn a_template_create_sends_its_fields_and_refuses_template() {
    use farhelm_proto::AgentTemplate;
    use farhelm_proto::launcher::{
        LauncherKind, TemplateDestination, TemplateFields, TemplateNameKind,
    };
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        let fields = TemplateFields {
            kind: Some(LauncherKind::Command),
            command: Some("my-agent --go".to_string()),
            yolo: Some(false),
            destination: Some(TemplateDestination::Folder("/srv/work".to_string())),
            name: Some("worker".to_string()),
            name_kind: Some(TemplateNameKind::Fixed),
            ..Default::default()
        };
        assert_eq!(
            request,
            AgentVerb::TemplateCreate {
                shortcuts: farhelm_proto::TemplateShortcutEdit {
                    quick_launch: Some(true),
                    key: Some(farhelm_proto::TemplateKeyEdit::Set("0".into())),
                },
                name: "my-template".to_string(),
                fields: fields.clone(),
                host: Some("builder".to_string()),
            }
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::TemplateWritten {
                    template: Box::new(AgentTemplate::listed(
                        &farhelm_proto::launcher::LaunchTemplate {
                            name: "my-template".to_string(),
                            fields,
                        },
                        Some("builder".to_string()),
                    )),
                },
            },
        })
    });
    let output = output_with_timeout(agent_command_with_args(
        &socket,
        &[
            "template",
            "create",
            "my-template",
            "--command",
            "my-agent --go",
            "--no-yolo",
            "--cwd",
            "/srv/work",
            "--title",
            "worker",
            "--title-kind",
            "fixed",
            "--host",
            "builder",
            "--quick-launch",
            "--key",
            "0",
        ],
    ));
    finish_server(done, thread);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "created template \"my-template\"\n"
    );

    let refused = output_with_timeout(agent_command_with_args(
        &socket,
        &["template", "create", "t", "--template", "other"],
    ));
    assert_eq!(refused.status.code(), Some(1));
    assert!(
        String::from_utf8(refused.stderr)
            .unwrap()
            .contains("takes no --template")
    );
}

/// Spec: `farhelm agent template edit NAME` sends one `TemplateEdit`
/// carrying only the fields its flags give, and `template delete NAME` one
/// `TemplateDelete`; each prints one confirmation line naming the template.
///
/// Why: an edit sets only what it is given (SPEC.md), so a flag the CLI
/// filled in on its own would overwrite a field the agent never meant to
/// touch, possibly one whose text the listing withholds. The create test
/// covers the shared field flags; this pins the other two verbs' wiring.
#[farhelm_testtrace::test]
fn template_edit_and_delete_send_their_verbs() {
    use farhelm_proto::launcher::TemplateFields;
    let temp = farhelm_teststate::tempdir().unwrap();
    let socket = temp.path().join("supervisor.sock");

    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        let fields = TemplateFields {
            name: Some("renamed".to_string()),
            ..Default::default()
        };
        assert_eq!(
            request,
            AgentVerb::TemplateEdit {
                shortcuts: Default::default(),
                name: "my-template".to_string(),
                fields: fields.clone(),
                host: None,
            }
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::TemplateWritten {
                    template: Box::new(farhelm_proto::AgentTemplate::listed(
                        &farhelm_proto::launcher::LaunchTemplate {
                            name: "my-template".to_string(),
                            fields,
                        },
                        None,
                    )),
                },
            },
        })
    });
    let edited = output_with_timeout(agent_command_with_args(
        &socket,
        &["template", "edit", "my-template", "--title", "renamed"],
    ));
    finish_server(done, thread);
    assert_eq!(
        edited.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&edited.stderr)
    );
    assert!(
        String::from_utf8(edited.stdout)
            .unwrap()
            .contains("\"my-template\""),
        "the confirmation names the template"
    );

    // A fresh path: the first mock's socket file is still bound.
    let socket = temp.path().join("supervisor-delete.sock");
    let (done, thread) = mock_supervisor(&socket, |request| {
        let ControlMsg::AgentRequest {
            req_id, request, ..
        } = request
        else {
            panic!("farhelm agent must send an AgentRequest, got {request:?}");
        };
        assert_eq!(
            request,
            AgentVerb::TemplateDelete {
                name: "my-template".to_string(),
            }
        );
        Some(ControlMsg::AgentResponse {
            req_id,
            outcome: AgentOutcome::Ok {
                reply: AgentReply::TemplateDeleted {},
            },
        })
    });
    let deleted = output_with_timeout(agent_command_with_args(
        &socket,
        &["template", "delete", "my-template"],
    ));
    finish_server(done, thread);
    assert_eq!(
        deleted.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&deleted.stderr)
    );
    assert!(
        String::from_utf8(deleted.stdout)
            .unwrap()
            .contains("\"my-template\""),
        "the confirmation names the template"
    );
}

/// Shortcut-only edits must reach the helm with no template field, whether
/// assigning or clearing. The CLI rejects keys outside the ten physical
/// digits before it can send an approval request.
#[farhelm_testtrace::test]
fn template_shortcut_only_edits_send_their_flags() {
    use farhelm_proto::{TemplateKeyEdit, TemplateShortcutEdit};
    let temp = farhelm_teststate::tempdir().unwrap();
    let cases = [
        (
            vec!["template", "edit", "fast", "--quick-launch", "--key", "3"],
            TemplateShortcutEdit {
                quick_launch: Some(true),
                key: Some(TemplateKeyEdit::Set("3".into())),
            },
        ),
        (
            vec!["template", "edit", "fast", "--no-quick-launch", "--no-key"],
            TemplateShortcutEdit {
                quick_launch: Some(false),
                key: Some(TemplateKeyEdit::Clear),
            },
        ),
    ];
    for (index, (args, expected)) in cases.into_iter().enumerate() {
        let socket = temp.path().join(format!("supervisor-{index}.sock"));
        let (done, thread) = mock_supervisor(&socket, move |request| {
            let ControlMsg::AgentRequest {
                req_id, request, ..
            } = request
            else {
                panic!("farhelm agent must send an AgentRequest, got {request:?}");
            };
            assert_eq!(
                request,
                AgentVerb::TemplateEdit {
                    name: "fast".into(),
                    fields: Default::default(),
                    host: None,
                    shortcuts: expected.clone(),
                }
            );
            Some(ControlMsg::AgentResponse {
                req_id,
                outcome: AgentOutcome::Ok {
                    reply: AgentReply::TemplateWritten {
                        template: Box::new(farhelm_proto::AgentTemplate::listed(
                            &farhelm_proto::launcher::LaunchTemplate {
                                name: "fast".into(),
                                fields: Default::default(),
                            },
                            None,
                        )),
                    },
                },
            })
        });
        let output = output_with_timeout(agent_command_with_args(&socket, &args));
        finish_server(done, thread);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let refused = output_with_timeout(agent_command_with_args(
        &temp.path().join("unused.sock"),
        &["template", "edit", "fast", "--key", "10"],
    ));
    assert_ne!(refused.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("one digit"));
}

//! The mock supervisor `farhelm spawn` and `farhelm agent` are tested
//! against: one Unix socket that accepts one connection, checks the
//! session-authenticated handshake, reads one request, and answers it as
//! the test says.
//!
//! Shared by `agent_cli.rs` and `spawn_cli.rs`, which each used to carry
//! their own copy, and included by path from those two only: `cli_support`
//! is also part of the e2e binary, which has no use for it. The mock's own
//! cleanup tests live in `agent_cli.rs`, once.

use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
use farhelm_proto::{ControlMsg, Frame};
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
pub fn mock_supervisor(
    socket: &std::path::Path,
    session_id: &'static str,
    respond: impl FnOnce(ControlMsg) -> Option<ControlMsg> + Send + 'static,
) -> (
    std::sync::mpsc::Receiver<Result<(), String>>,
    farhelm_teststate::thread::FixtureThread,
) {
    mock_supervisor_frames(socket, session_id, |request| {
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
pub fn mock_supervisor_frames(
    socket: &std::path::Path,
    session_id: &'static str,
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
                                            .expect("the CLI did not connect")
                                            .expect("accept the CLI");
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
                                        panic!("the CLI must authenticate in its hello: {hello:?}");
                                    };
                                    // Every session-authenticated CLI command
                                    // sends the one role spelling
                                    // `handshake_with_session_auth` has.
                                    assert_eq!(role, "spawn");
                                    assert_eq!(auth.session_id, session_id);
                                    assert_eq!(auth.token, "secret");
                                    let frame = tokio::time::timeout(
                                        Duration::from_secs(5),
                                        reader.read_frame(),
                                    )
                                    .await
                                    .expect("the CLI did not send a request")
                                    .unwrap()
                                    .expect("the CLI's request");
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
    let owner =
        farhelm_teststate::thread::FixtureThread::new("CLI mock supervisor", thread, move || {
            let _ = cancel_tx.send(());
        })
        .expect("start mock supervisor join observer");
    (done_rx, owner)
}

/// Require protocol success before waiting for runtime and thread destruction.
/// The owner stays armed through both assertions, so a failed result still
/// cancels the mock instead of detaching it on the test's unwind path.
pub fn finish_server(
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

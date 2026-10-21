//! Process-level proof for stdio progress, protocol purity and absence status.

use std::process::{Output, Stdio};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};

/// Own the shipped helper until it exits, including on assertion failure.
/// Captured streams let the tests check that stderr progress never leaks into
/// the protocol channel; stdin is selectable to exercise runtime shutdown.
fn spawn_stdio(state_dir: &std::path::Path, stdin: Stdio) -> Child {
    Command::new(env!("CARGO_BIN_EXE_farhelm"))
        .args(["internal", "stdio", "--state-dir"])
        .arg(state_dir)
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn the farhelm stdio child")
}

/// These fixtures produce only a short marker or diagnostic. Bound retained
/// output even if a regression makes the child flood one of its pipes.
async fn bounded_output(stream: impl AsyncRead + Unpin) -> std::io::Result<Vec<u8>> {
    const CAP: usize = 4096;
    let mut bytes = Vec::new();
    stream
        .take((CAP + 1) as u64)
        .read_to_end(&mut bytes)
        .await?;
    assert!(
        bytes.len() <= CAP,
        "stdio fixture exceeded its output budget"
    );
    Ok(bytes)
}

/// Drain both pipes while waiting for exit, so a full pipe cannot deadlock the
/// fixture. Borrow the child so timeout cancellation retains kill-on-drop.
async fn collect_stdio(child: &mut Child) -> Output {
    let stdout = child.stdout.take().expect("captured stdout");
    let stderr = child.stderr.take().expect("captured stderr");
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(bounded_output(stdout), bounded_output(stderr), child.wait())
    })
    .await;
    let (stdout, stderr, status) = result
        .unwrap_or_else(|_| panic!("stdio child did not finish; status: {:?}", child.try_wait()))
        .expect("collect the stdio child");
    Output {
        status,
        stdout,
        stderr,
    }
}

/// Initial connection failures must not claim that a supervisor accepted the
/// connection, regardless of which exit status provisioning receives.
async fn assert_initial_failure(state_dir: &std::path::Path, code: i32) {
    let mut child = spawn_stdio(state_dir, Stdio::null());
    let output = collect_stdio(&mut child).await;
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(
        !String::from_utf8_lossy(&output.stderr)
            .contains(farhelm_proto::io::SUPERVISOR_CONNECTED_MARKER),
        "an initial connection failure emitted a connected marker: {output:?}"
    );
}

/// Missing and refused sockets are positive absence; a different connection
/// error must not grant the helm permission to offer provisioning.
#[farhelm_testtrace::test]
async fn stdio_exit_75_is_reserved_for_positive_absence() {
    let root = farhelm_teststate::tempdir().expect("isolated stdio state roots");
    assert_initial_failure(&root.path().join("missing"), 75).await;

    let refused = root.path().join("refused");
    std::fs::create_dir(&refused).expect("create refused state directory");
    let socket = refused.join("supervisor.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind stale socket");
    drop(listener);
    assert_initial_failure(&refused, 75).await;

    let not_directory = root.path().join("not-a-directory");
    std::fs::write(&not_directory, b"not a directory").expect("create ENOTDIR fixture");
    assert_initial_failure(&not_directory, 1).await;
}

/// A real accepted socket proves the marker's meaning, and known forwarded
/// bytes prove stdout remains the proxy stream. Holding stdin open until exit
/// also guards against returning into Tokio's uncancellable stdin shutdown.
#[farhelm_testtrace::test]
async fn connected_marker_is_stderr_only_and_socket_eof_exits_with_stdin_open() {
    const FORWARDED: &[u8] = b"supervisor fixture bytes\0\xff";
    let root = farhelm_teststate::tempdir().expect("isolated stdio socket root");
    let listener = tokio::net::UnixListener::bind(root.path().join("supervisor.sock"))
        .expect("bind the owned supervisor socket");
    let mut child = spawn_stdio(root.path(), Stdio::piped());
    let stdin = child.stdin.take().expect("hold child stdin open");
    let accepted = tokio::time::timeout(Duration::from_secs(10), listener.accept()).await;
    let (mut socket, _) = accepted
        .unwrap_or_else(|_| panic!("helper never connected; status: {:?}", child.try_wait()))
        .expect("accept the helper connection");
    assert!(
        child
            .try_wait()
            .expect("inspect connected helper")
            .is_none(),
        "the accepted helper must still be alive before sending fixture bytes"
    );
    tokio::time::timeout(Duration::from_secs(10), socket.write_all(FORWARDED))
        .await
        .expect("the live helper must consume fixture bytes")
        .expect("send fixture bytes");
    drop(socket);
    let output = collect_stdio(&mut child).await;
    drop(stdin);

    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, FORWARDED, "{output:?}");
    assert_eq!(
        output.stderr,
        format!("{}\n", farhelm_proto::io::SUPERVISOR_CONNECTED_MARKER).as_bytes(),
        "the helper must emit exactly one complete connected line: {output:?}"
    );
}

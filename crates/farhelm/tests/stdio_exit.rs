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

/// A silent, still-open gateway stdin must not keep the remote proxy alive
/// forever after the helm sleeps. Socket closure independently proves teardown,
/// rather than relying only on a diagnostic or elapsed delay.
#[farhelm_testtrace::test]
async fn silent_open_stdin_exits_at_the_idle_limit() {
    let root = farhelm_teststate::tempdir().unwrap();
    let listener = tokio::net::UnixListener::bind(root.path().join("supervisor.sock")).unwrap();
    let mut child = idle_stdio(root.path());
    let stdin = child.stdin.take().unwrap();
    let (mut socket, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
        .await
        .unwrap()
        .unwrap();
    assert!(
        child.try_wait().unwrap().is_none(),
        "premise: connected proxy is alive"
    );
    let output = collect_stdio(&mut child).await;
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no input from the helm"),
        "{output:?}"
    );
    let mut bytes = [0; 1];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), socket.read(&mut bytes))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    drop(stdin);
}

/// Traffic from a healthy idle helm repeatedly resets the silence budget.
/// Each byte is observed at the supervisor socket before the next interval;
/// the test therefore distinguishes reset-on-read from a fixed lifetime cap.
#[farhelm_testtrace::test]
async fn helm_input_resets_the_stdio_idle_limit() {
    let root = farhelm_teststate::tempdir().unwrap();
    let listener = tokio::net::UnixListener::bind(root.path().join("supervisor.sock")).unwrap();
    let mut child = idle_stdio(root.path());
    let mut stdin = child.stdin.take().unwrap();
    let (mut socket, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
        .await
        .unwrap()
        .unwrap();
    for byte in 0..6 {
        assert!(
            child.try_wait().unwrap().is_none(),
            "premise: proxy is alive before writing"
        );
        stdin.write_all(&[byte]).await.unwrap();
        let mut received = [0; 1];
        tokio::time::timeout(Duration::from_secs(1), socket.read_exact(&mut received))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(received, [byte]);
        // sleep-ok: traffic spaced at half the idle limit must extend the proxy beyond its initial deadline
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert!(
        child.try_wait().unwrap().is_none(),
        "traffic kept the proxy alive past one idle period"
    );
    drop(socket);
    let output = collect_stdio(&mut child).await;
    assert!(output.status.success(), "{output:?}");
    drop(stdin);
}

/// Pass the opt-in short budget only to the child. Legacy invocations have
/// no deadline; tests must not alter their own process environment.
fn idle_stdio(state_dir: &std::path::Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_farhelm"))
        .args([
            "internal",
            "stdio",
            "--stdin-idle-limit-secs",
            "1",
            "--state-dir",
        ])
        .arg(state_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}

/// An older helm never opts in. Downstream traffic and open, silent stdin
/// must remain usable beyond the opted-in fixture's one-second budget.
#[farhelm_testtrace::test]
async fn legacy_stdio_never_enables_the_idle_deadline() {
    let root = farhelm_teststate::tempdir().unwrap();
    let listener = tokio::net::UnixListener::bind(root.path().join("supervisor.sock")).unwrap();
    let mut child = spawn_stdio(root.path(), Stdio::piped());
    let stdin = child.stdin.take().unwrap();
    let (mut socket, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    for _ in 0..4 {
        assert!(
            child.try_wait().unwrap().is_none(),
            "legacy helper must still be alive"
        );
        socket.write_all(b"x").await.unwrap();
        let mut byte = [0];
        tokio::time::timeout(Duration::from_secs(2), stdout.read_exact(&mut byte))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(byte, [b'x']);
        // sleep-ok: leave stdin silent beyond the opted-in fixture's budget while downstream remains live.
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert!(child.try_wait().unwrap().is_none());
    child.stdout = Some(stdout);
    drop(socket);
    let output = collect_stdio(&mut child).await;
    assert!(output.status.success(), "{output:?}");
    drop(stdin);
}

/// Filling an unread supervisor socket parks the upstream write while the
/// gateway stdin stays open. The independent watchdog must still end the
/// process and close its socket; a timed stdin read alone cannot do this.
#[farhelm_testtrace::test]
async fn unread_supervisor_socket_cannot_bypass_the_idle_deadline() {
    let root = farhelm_teststate::tempdir().unwrap();
    let listener = tokio::net::UnixListener::bind(root.path().join("supervisor.sock")).unwrap();
    let mut child = idle_stdio(root.path());
    let mut stdin = child.stdin.take().unwrap();
    let (mut socket, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
        .await
        .unwrap()
        .unwrap();
    assert!(child.try_wait().unwrap().is_none());
    // Own this future locally: when it parks, it retains stdin without a
    // detached task that could outlive a failed assertion or test teardown.
    let fill = async {
        for _ in 0..1024 {
            stdin.write_all(&[42; 64 * 1024]).await?;
        }
        std::io::Result::Ok(())
    };
    tokio::pin!(fill);
    assert!(
        tokio::time::timeout(Duration::from_millis(200), &mut fill)
            .await
            .is_err(),
        "premise: 64 MiB cannot fit in the unread socket and stdin buffers"
    );
    assert!(
        child.try_wait().unwrap().is_none(),
        "premise: deadline has not fired yet"
    );
    // One byte proves the pump has run, without draining the full socket or
    // the many MiB still held by the pending stdin write.
    let mut first = [0];
    tokio::time::timeout(Duration::from_millis(200), socket.read_exact(&mut first))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first,
        [42],
        "premise: the blocked pump reached the supervisor socket"
    );
    let output = collect_stdio(&mut child).await;
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no input from the helm"),
        "{output:?}"
    );
    let drained = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::io::copy(
            &mut (&mut socket).take(8 * 1024 * 1024),
            &mut tokio::io::sink(),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        drained < 8 * 1024 * 1024,
        "socket must reach EOF, not only the drain cap"
    );
}

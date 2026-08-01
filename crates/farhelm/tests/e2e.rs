//! End-to-end integration tests: real supervisor, real private tmux,
//! real launch path through the user's login shell, fake agent as the
//! process under supervision.
//!
//! These tests exercise the exact production code path — the same
//! `handle_connection` the unix socket serves and the same
//! `SupervisorClient` the helm uses — over an in-process duplex pipe.
//! What they deliberately do NOT fake: tmux, control-mode streaming,
//! `send-keys -H` input, `capture-pane` replay, and pane-mode
//! restoration.
//! If tmux behavior drifts from the audited assumptions in SPEC_impl.md,
//! it fails here first.
//!
//! They live in the `farhelm` bin crate because `CARGO_BIN_EXE_farhelm`
//! (the built multi-call binary, which carries the fake agent and the
//! launch shim) is only available to the defining crate's tests.

use farhelm_helm::{SupervisorClient, SupervisorError, TermEvent};
use farhelm_proto::io::parse_control;
use farhelm_proto::{ControlMsg, ErrorKind, Frame, FrameKind, SessionInfo};
use farhelm_supervisor::service::{Supervisor, handle_connection};
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc::UnboundedReceiver;

/// A duplex endpoint whose write direction can fail independently.
///
/// Real sockets can remain readable after their peer stops accepting
/// replies. Tokio's in-memory duplex stream does not expose that state,
/// so this wrapper lets the connection-lifecycle test reproduce it
/// without depending on transport-specific half-close behavior.
struct ToggleWriteFailure {
    inner: tokio::io::DuplexStream,
    fail_writes: Arc<AtomicBool>,
}

impl AsyncRead for ToggleWriteFailure {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for ToggleWriteFailure {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.fail_writes.load(Ordering::SeqCst) {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected server write failure",
            )));
        }
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// The built farhelm binary: fake agent + launch shim in one artifact,
/// exactly as production ships it.
fn farhelm_bin() -> &'static str {
    env!("CARGO_BIN_EXE_farhelm")
}

/// Run one tmux command against a private socket, asynchronously.
///
/// Async (tokio Command) rather than `std::process` because `#[tokio::test]`
/// runs a current-thread runtime: a blocking child-process wait in a test
/// body stalls the in-process supervisor and forwarder tasks on that same
/// runtime, distorting exactly the concurrent behavior under test.
async fn tmux_query(sock: &std::path::Path, args: &[&str]) -> std::process::Output {
    tokio::process::Command::new("tmux")
        .arg("-S")
        .arg(sock)
        .args(args)
        .output()
        .await
        .expect("tmux query")
}

/// Kill the harness's tmux server and wait until it has genuinely died.
///
/// `kill-server` returns before the server finishes tearing down; any
/// tmux command racing that teardown (an `ensure_server` from a fresh
/// `Supervisor`, an auto-started server from `new-session`) can connect
/// to the dying server and fail with "server exited unexpectedly" — a
/// flake observed on loaded CI runners, never a product bug. Every test
/// that kills the server must go through this helper rather than a bare
/// `kill-server`.
async fn kill_tmux_server_and_wait(sock: &std::path::Path) {
    let killed = tmux_query(sock, &["kill-server"]).await;
    assert!(
        killed.status.success(),
        "test setup: tmux kill-server must succeed, got: {}",
        String::from_utf8_lossy(&killed.stderr)
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let probe = tmux_query(sock, &["list-sessions"]).await;
        if !probe.status.success()
            && String::from_utf8_lossy(&probe.stderr).contains("no server running")
        {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "tmux server never finished dying after kill-server"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Whether the harness's tmux knows a given format variable, probed
/// against its live server (which always has a session by the time any
/// caller asks).
///
/// The success assert distinguishes "tmux genuinely lacks this format"
/// (expands empty) from "the probe itself broke" (command fails): a
/// silent probe failure would look like an unsupported format and
/// quietly skip the assertion it guards, which is exactly the assertion
/// worth having.
async fn tmux_has_format(h: &Harness, name: &str) -> bool {
    let out = tmux_query(
        &h.state.path().join("tmux.sock"),
        &["display-message", "-p", &format!("#{{{name}}}")],
    )
    .await;
    assert!(
        out.status.success(),
        "tmux format probe failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    !String::from_utf8_lossy(&out.stdout).trim().is_empty()
}

/// Poll tmux until the window reports `expected` ("COLSxROWS"), failing
/// the test if it never does. Resizes are fire-and-forget, so there is
/// no completion to await — polling is the only observation available.
async fn wait_for_geometry(h: &Harness, expected: &str) {
    let sock = h.state.path().join("tmux.sock");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let out = tmux_query(
            &sock,
            &["display-message", "-p", "#{window_width}x#{window_height}"],
        )
        .await;
        let got = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if got == expected {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "window geometry never reached {expected} (last: {got})"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Require the window geometry to STAY at `expected` for a settle
/// window. Asserting an absence needs a period of observation, not a
/// single read: the resize that must be ignored is in flight, and a
/// single check could run before it would have landed.
async fn assert_geometry_stays(h: &Harness, expected: &str, why: &str) {
    let sock = h.state.path().join("tmux.sock");
    let settle = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < settle {
        let out = tmux_query(
            &sock,
            &["display-message", "-p", "#{window_width}x#{window_height}"],
        )
        .await;
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            expected,
            "{why}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Wait for a supervisor's socket to appear, for tests that drive
/// `serve()` directly rather than an in-process duplex pipe. Panics on
/// timeout like every other wait helper here — silently proceeding
/// would surface as a less legible connect failure later.
async fn wait_for_socket(sock: &std::path::Path) {
    for _ in 0..100 {
        if sock.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("supervisor socket never appeared at {}", sock.display());
}

/// Quote a path for an agent invocation string, which the supervisor
/// parses with shell-words. Without this a checkout under a path with
/// spaces would fragment the argv and fail every test confusingly.
fn agent_cmd(args: &str) -> String {
    format!("{} {args}", shell_words::quote(farhelm_bin()))
}

/// Caps how many harnesses run at once.
///
/// Each one is a tmux server plus a login shell plus a fake agent, and
/// libtest runs every test in this file concurrently. Unbounded, the
/// machine gets loaded enough that agent startup exceeds the waits and
/// tests fail for reasons that have nothing to do with the code — a
/// flakiness source worth removing rather than papering over with longer
/// timeouts.
static SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

/// Kills a private tmux server on drop.
///
/// Drop-based on purpose: a test that fails an assertion never reaches
/// an explicit teardown call, and leaked tmux servers accumulate across
/// runs until they visibly slow the machine down (this happened).
/// Synchronous because Drop cannot await. Every test that starts a tmux
/// server — via `harness()` or by hand — must hold one of these, ordered
/// BEFORE the state `TempDir` in its struct so the server dies before
/// the directory holding its socket disappears.
struct TmuxServerGuard(std::path::PathBuf);

impl Drop for TmuxServerGuard {
    fn drop(&mut self) {
        let _ = std::process::Command::new("tmux")
            .arg("-S")
            .arg(&self.0)
            .arg("kill-server")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

struct Harness {
    client: Arc<SupervisorClient>,
    /// Shared so tests can open additional connections to the same
    /// supervisor (`second_client`) — cross-connection enforcement is
    /// exactly what some SPEC.md rules are about.
    sup: Arc<Supervisor>,
    _tmux: TmuxServerGuard,
    // Read in tests (socket paths); ALSO held so the tempdir outlives
    // the tmux server — it must be declared after `_tmux` so the server
    // dies before its socket directory is deleted.
    state: tempfile::TempDir,
    // Released on drop, letting the next test start its stack.
    _slot: tokio::sync::SemaphorePermit<'static>,
}

impl Harness {
    /// Open a second, independent connection to this harness's
    /// supervisor — a stand-in for "another helm" or a restarted one.
    /// Its channel ids number from 1 like any client's, which is the
    /// collision the cross-connection tests rely on.
    async fn second_client(&self) -> Arc<SupervisorClient> {
        connect_client(&self.sup).await
    }
}

/// Connect one client to a supervisor over an in-process duplex pipe
/// (the local-transport shape, minus the socket file).
async fn connect_client(sup: &Arc<Supervisor>) -> Arc<SupervisorClient> {
    let (client_side, server_side) = tokio::io::duplex(1 << 20);
    let sup = Arc::clone(sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup, server_side).await;
    });
    let (r, w) = tokio::io::split(client_side);
    SupervisorClient::start(r, w).await.expect("handshake")
}

/// Create a basic-script fake-agent session in a fresh working
/// directory — the preamble almost every test in this file shares.
/// Returns the workdir too: it must outlive the launch (it is the
/// session's cwd), so callers hold it as `_work`.
async fn basic_session(h: &Harness) -> (SessionInfo, tempfile::TempDir) {
    let work = tempfile::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    (session, work)
}

/// Boot a supervisor on a throwaway state dir and connect a client to it.
async fn harness() -> Harness {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    let guard = TmuxServerGuard(state.path().join("tmux.sock"));
    let client = connect_client(&sup).await;
    Harness {
        client,
        sup,
        _tmux: guard,
        state,
        _slot: slot,
    }
}

/// Drain terminal events until `needle` has appeared in the accumulated
/// output, failing the test after `secs`. Everything received is
/// appended to `seen`, so callers can make further assertions on the
/// transcript after the call returns.
///
/// A `Detached` event ends the stream but is not itself a failure: when
/// an agent exits, its last output and the pane-death notice race, and
/// the bytes may already be in hand. So the needle is re-checked after
/// the stream ends and only then reported missing.
async fn wait_for(
    rx: &mut UnboundedReceiver<TermEvent>,
    seen: &mut Vec<u8>,
    needle: &str,
    secs: u64,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    let mut ended: Option<String> = None;
    loop {
        if String::from_utf8_lossy(seen).contains(needle) {
            return;
        }
        if let Some(reason) = ended {
            panic!(
                "stream ended ({reason}) without {needle:?}; transcript so far:\n{}",
                String::from_utf8_lossy(seen)
            );
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => seen.extend_from_slice(&bytes),
            // Drain whatever is already queued behind the notice before
            // deciding the needle never arrived.
            Ok(Some(TermEvent::Detached(reason))) => {
                while let Ok(TermEvent::Data(bytes)) = rx.try_recv() {
                    seen.extend_from_slice(&bytes);
                }
                ended = Some(reason);
            }
            Ok(None) => ended = Some("closed".to_string()),
            Err(_) => panic!(
                "timed out waiting for {needle:?}; transcript so far:\n{}",
                String::from_utf8_lossy(seen)
            ),
        }
    }
}

/// Extract complete records from the counter fixture without decoding
/// unrelated terminal bytes as UTF-8.
///
/// A record split exactly at replay cutover may have cursor-restoration
/// escapes inserted between its halves, in which case it is deliberately
/// absent here. The fixture flushes each short record as one PTY write so
/// that shape would itself be evidence that tmux split one write across
/// pane reads, not a parser artifact.
fn counter_records(transcript: &[u8]) -> Vec<u64> {
    const PREFIX: &[u8] = b"CUTOVER-";
    const DIGITS: usize = 8;

    let mut records = Vec::new();
    let mut offset = 0;
    while offset + PREFIX.len() + DIGITS <= transcript.len() {
        if transcript[offset..].starts_with(PREFIX) {
            let digits = &transcript[offset + PREFIX.len()..offset + PREFIX.len() + DIGITS];
            if digits.iter().all(u8::is_ascii_digit) {
                let number = std::str::from_utf8(digits)
                    .expect("ASCII digits")
                    .parse()
                    .expect("eight digits fit in u64");
                records.push(number);
                offset += PREFIX.len() + DIGITS;
                continue;
            }
        }
        offset += 1;
    }
    records
}

/// Accumulate one attachment until the counter has advanced past a
/// caller-chosen sequence number.
async fn collect_counter_through(rx: &mut UnboundedReceiver<TermEvent>, target: u64) -> Vec<u8> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let mut transcript = Vec::new();
    loop {
        if counter_records(&transcript)
            .last()
            .is_some_and(|last| *last >= target)
        {
            return transcript;
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => transcript.extend_from_slice(&bytes),
            Ok(Some(TermEvent::Detached(reason))) => {
                panic!("counter attachment ended before {target}: {reason}")
            }
            Ok(None) => panic!("counter attachment closed before {target}"),
            Err(_) => panic!(
                "counter never reached {target}; last records: {:?}",
                counter_records(&transcript)
                    .into_iter()
                    .rev()
                    .take(5)
                    .collect::<Vec<_>>()
            ),
        }
    }
}

/// The core walking-skeleton path: create a session running the fake
/// agent through the real login-shell + shim launch chain, see its
/// output arrive over the attach stream, and round-trip input. This is
/// PLAN_M1.md acceptance criterion 5's "create, output rendering, input
/// round-trip" at the Rust layer.
#[tokio::test]
async fn create_attach_and_roundtrip_input() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    h.client.send_input(chan, b"hello-farhelm\r".to_vec());
    wait_for(&mut rx, &mut seen, "echo:", 10).await;
    wait_for(&mut rx, &mut seen, "hello-farhelm", 5).await;
}

/// Reconnect-with-replay: detach, reattach, and require the replay to
/// contain output produced before the reattach AND the bracketed-paste
/// mode the fake agent enabled. Mode restoration is the audited
/// silent-loss case (SPEC_impl.md) — content alone passing this test
/// would be the bug.
#[tokio::test]
async fn reattach_replays_history_and_modes() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, b"before-reattach\r".to_vec());
    wait_for(&mut rx, &mut seen, "echo:", 10).await;
    h.client.detach(chan).await;

    let (chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "before-reattach", 10).await;
    // Input-mode restoration follows the content prefill, so wait for it
    // explicitly rather than asserting on a prefix of the replay. On a
    // tmux whose format vocabulary predates `bracket_paste_flag`, that
    // one field degrades to "off" (see PaneModes::parse) and there is
    // nothing to assert — hence the probe rather than an unconditional
    // expectation.
    if tmux_has_format(&h, "bracket_paste_flag").await {
        wait_for(&mut rx2, &mut replay, "\x1b[?2004h", 5).await;
    } else {
        eprintln!("tmux lacks bracket_paste_flag; skipping mode-restoration assertion");
    }
    let replay_text = String::from_utf8_lossy(&replay);
    assert!(
        replay_text.contains("FAKE-AGENT READY"),
        "replay missing pre-detach history"
    );

    // A fresh echo, not just replay: detach-then-reattach is one of the
    // three triggers of the frozen-replay hazard (a control-mode client
    // overlap renders the replay and then never updates), and replay
    // content arrives either way — only new output distinguishes a live
    // terminal from a frozen one.
    h.client
        .send_input(chan2, b"live-after-reattach\r".to_vec());
    wait_for(&mut rx2, &mut replay, "echo:", 15).await;
    wait_for(&mut rx2, &mut replay, "live-after-reattach", 10).await;
}

/// Replay and live output meet at one exact tmux command boundary.
///
/// The fixture writes numbered records continuously while this test
/// repeatedly replaces the attachment. Every new transcript must be one
/// consecutive range with no duplicate. Capturing before opening the
/// control client loses records here; enabling a second client before
/// capture duplicates them or triggers the frozen-stream regression.
#[tokio::test]
async fn reattach_cutover_has_no_missing_or_duplicated_output() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script counter"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let mut target = 100;
    let mut final_channel = None;
    for attempt in 0..8 {
        let (channel, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
        final_channel = Some(channel);
        let transcript = collect_counter_through(&mut rx, target).await;
        let records = counter_records(&transcript);
        let first = *records.first().expect("snapshot contains counter output");
        let last = *records.last().expect("checked above");
        let expected: Vec<u64> = (first..=last).collect();
        assert_eq!(
            records, expected,
            "replay/live cutover {attempt} lost or duplicated a counter record"
        );
        target = last + 40;
    }
    h.client
        .detach(final_channel.expect("at least one attachment"))
        .await;
}

/// Invalid UTF-8 is legitimate terminal output and must cross the live
/// control-mode stream byte-for-byte. Any conversion through `String`
/// would replace 0xff while ordinary TUI tests continued to pass.
#[tokio::test]
async fn non_utf8_terminal_output_survives_live_stream() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script binary"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (_channel, mut live) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut live_bytes = Vec::new();
    wait_for(&mut live, &mut live_bytes, "BINARY-MARKER", 20).await;
    assert!(
        live_bytes.contains(&0xff),
        "live output replaced or dropped the invalid byte: {live_bytes:?}"
    );
}

/// Last attach wins (SPEC.md): a second attach visibly detaches the
/// first — the old stream gets a Detached event, and input keeps working
/// on the new attachment.
#[tokio::test]
async fn second_attach_detaches_first() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (_c1, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    let (c2, mut rx2) = h.client.attach(&session.id, 80, 24).await.expect("attach2");

    // First attachment must observe its own takeover.
    let deadline = Duration::from_secs(10);
    let detached = tokio::time::timeout(deadline, async {
        while let Some(ev) = rx1.recv().await {
            if let TermEvent::Detached(reason) = ev {
                return reason;
            }
        }
        panic!("first attachment stream ended without Detached");
    })
    .await
    .expect("timed out waiting for Detached on first attachment");
    assert!(detached.contains("another client"));

    // Second attachment is live.
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 10).await;
    h.client.send_input(c2, b"still-alive\r".to_vec());
    wait_for(&mut rx2, &mut seen2, "still-alive", 10).await;
}

/// Attachment channels are connection-local routing keys, so zero and
/// reuse are protocol errors rather than harmless client choices.
///
/// Reusing a live channel previously overwrote its input route while two
/// forwarders emitted onto the same data channel. The raw client is
/// intentional: `SupervisorClient` normally allocates unique channels
/// and cannot express the hostile protocol sequence this validates.
#[tokio::test]
async fn attachment_channels_must_be_nonzero_and_unique() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};

    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (client_side, server_side) = tokio::io::duplex(1 << 20);
    let sup = Arc::clone(&h.sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup, server_side).await;
    });
    let (read_half, write_half) = tokio::io::split(client_side);
    let mut reader = FrameReader::new(read_half);
    let mut writer = FrameWriter::new(write_half);
    handshake(&mut reader, &mut writer, "helm")
        .await
        .expect("handshake");

    writer
        .write_control(&ControlMsg::Attach {
            req_id: 1,
            session_id: session.id.clone(),
            channel: 0,
            cols: 80,
            rows: 24,
        })
        .await
        .unwrap();
    assert!(matches!(
        parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap(),
        ControlMsg::Error {
            req_id: 1,
            kind: ErrorKind::InvalidRequest,
            ..
        }
    ));

    writer
        .write_control(&ControlMsg::Attach {
            req_id: 2,
            session_id: session.id.clone(),
            channel: 7,
            cols: 80,
            rows: 24,
        })
        .await
        .unwrap();
    assert!(matches!(
        parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap(),
        ControlMsg::Attached {
            req_id: 2,
            channel: 7
        }
    ));

    writer
        .write_control(&ControlMsg::Attach {
            req_id: 3,
            session_id: session.id,
            channel: 7,
            cols: 80,
            rows: 24,
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let frame = reader.read_frame().await.unwrap().unwrap();
            if frame.kind != FrameKind::Control {
                continue;
            }
            match parse_control(&frame).unwrap() {
                ControlMsg::Error {
                    req_id: 3,
                    kind: ErrorKind::InvalidRequest,
                    ..
                } => break,
                ControlMsg::Attached { req_id: 3, .. } => {
                    panic!("duplicate attachment channel was accepted")
                }
                _ => {}
            }
        }
    })
    .await
    .expect("supervisor did not reject duplicate attachment channel");
}

/// Precondition failures fail the create with a visible error and no
/// session (SPEC.md's creation-failure split).
///
/// The in-memory check alone only proves this process's own map stayed
/// empty; it says nothing about whether a row still landed in SQLite
/// despite the rejection (this validation runs before `create_session`
/// ever touches tmux or the store, so today it cannot, but a future
/// reordering could reintroduce exactly that gap silently). Constructing
/// a second, independent `Supervisor` on the same state dir and listing
/// through IT is what actually proves nothing was persisted — a row
/// present only in SQLite, invisible to the original process's map, would
/// still surface here.
#[tokio::test]
async fn create_in_missing_directory_errors() {
    let h = harness().await;
    let err = h
        .client
        .create_session("/nonexistent/definitely/not/here", "true", None, 80, 24)
        .await
        .expect_err("create should fail");
    assert!(err.to_string().contains("working directory does not exist"));
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a bad-cwd failure must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "a missing directory is the caller's mistake, not a server fault"
    );
    assert!(h.client.list_sessions().await.unwrap().is_empty());

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction reading the same state dir");
    let client2 = connect_client(&sup2).await;
    assert!(
        client2.list_sessions().await.unwrap().is_empty(),
        "a rejected create must not have persisted a row visible to a fresh supervisor"
    );
}

/// An existing file is a different caller error from a missing path.
/// Keeping that distinction visible prevents a correct path from being
/// misdiagnosed as a typo.
#[tokio::test]
async fn create_in_a_regular_file_reports_not_a_directory() {
    let h = harness().await;
    let file = tempfile::NamedTempFile::new().unwrap();
    let err = h
        .client
        .create_session(&file.path().to_string_lossy(), "true", None, 80, 24)
        .await
        .expect_err("create should reject a regular file as cwd");
    assert!(err.to_string().contains("is not a directory"));
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a not-a-directory failure must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "cwd being a file is the caller's mistake, not a server fault"
    );
    assert!(h.client.list_sessions().await.unwrap().is_empty());
}

/// A cwd nested UNDER a regular file (`/tmp/somefile/child`) is a
/// different OS error than either "missing" or "cwd itself is a file": the
/// non-final path component being a file surfaces as
/// `io::ErrorKind::NotADirectory`, not `NotFound`. Still the caller's
/// mistake — a typo'd path segment, most likely — so it must classify the
/// same way as the sibling cases above, not fall through to the
/// catch-all `Internal` default.
#[tokio::test]
async fn create_under_a_regular_file_is_invalid_request() {
    let h = harness().await;
    let file = tempfile::NamedTempFile::new().unwrap();
    let nested = file.path().join("child");
    let err = h
        .client
        .create_session(&nested.to_string_lossy(), "true", None, 80, 24)
        .await
        .expect_err("create should reject a cwd nested under a regular file");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a not-a-directory failure must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "a path nested under a file is the caller's mistake, not a server fault"
    );
    assert!(h.client.list_sessions().await.unwrap().is_empty());
}

/// A NUL byte in the cwd text cannot address anything on a POSIX
/// filesystem; the OS rejects it before `create_session` ever reaches a
/// syscall that could distinguish "missing" from "exists". This surfaces
/// as `io::ErrorKind::InvalidInput`, the same caller-fault bucket as the
/// other malformed-path cases.
#[tokio::test]
async fn create_with_nul_byte_in_cwd_is_invalid_request() {
    let h = harness().await;
    let cwd = "/tmp/has-a-\u{0}-nul-byte";
    let err = h
        .client
        .create_session(cwd, "true", None, 80, 24)
        .await
        .expect_err("create should reject a cwd containing a NUL byte");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("an invalid-cwd-text failure must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "a NUL byte in the cwd is the caller's mistake, not a server fault"
    );
    assert!(h.client.list_sessions().await.unwrap().is_empty());
}

/// The helm must claim its HTTP port before creating the argv-requested
/// startup session. A busy port is a retryable local conflict; creating
/// the agent first would strand a durable session on every failed retry.
#[tokio::test]
async fn helm_bind_failure_creates_no_startup_session() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("state");
    let work = tempfile::tempdir().expect("workdir");
    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let occupied = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("reserve loopback port");
    let port = occupied.local_addr().unwrap().port();

    let output = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::process::Command::new(farhelm_bin())
            .args(["helm", "run", "--state-dir"])
            .arg(state.path())
            .arg("--port")
            .arg(port.to_string())
            .arg("--cwd")
            .arg(work.path())
            .args(["--agent", "true"])
            .output(),
    )
    .await
    .expect("helm did not fail promptly on occupied port")
    .expect("run helm");
    assert!(!output.status.success(), "occupied port must fail startup");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("binding"),
        "bind failure should retain its context: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let client = connect_client(&sup).await;
    assert!(
        client.list_sessions().await.unwrap().is_empty(),
        "bind failure must happen before startup session creation"
    );
}

/// Replay must reach into scrollback, not just the visible screen.
///
/// This is the test that would fail if `capture-pane`'s `-S` history
/// range were dropped: the earlier assertions all fit inside one 24-row
/// viewport, so a screen-only capture would pass them while silently
/// violating SPEC.md's replay floor.
#[tokio::test]
async fn reattach_replays_content_scrolled_off_screen() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // 80 lines against a 24-row window: spam-line-1 is far off screen by
    // the time spam-line-80 lands.
    h.client.send_input(chan, b"spam 80\r".to_vec());
    wait_for(&mut rx, &mut seen, "spam-line-80", 15).await;
    h.client.detach(chan).await;

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "spam-line-1", 15).await;
}

/// Reattaching to a full-screen (alternate-screen) app must show the
/// app, not a blank screen.
///
/// The failure this pins is subtle and was live: `\x1b[?1049h` switches
/// to a *cleared* alternate buffer, so emitting it after the content
/// prefill erases the replay. Ordering is the whole point, which is why
/// the assertion checks the switch precedes the content.
#[tokio::test]
async fn reattach_to_alt_screen_app_preserves_content() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "ALT-SCREEN APP", 20).await;
    h.client.detach(chan).await;

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "ALT-SCREEN APP", 15).await;
    let text = String::from_utf8_lossy(&replay);
    let switch = text
        .find("\x1b[?1049h")
        .expect("replay must re-enter the alternate screen");
    let content = text.find("ALT-SCREEN APP").expect("checked above");
    assert!(
        switch < content,
        "alt-screen switch must precede replayed content, else it clears it"
    );
}

/// Resize must reach tmux, not merely leave the terminal usable.
///
/// Asserting "typing still works after a resize" would pass even if
/// every resize message were dropped; this checks the window geometry
/// tmux actually holds.
#[tokio::test]
async fn resize_reaches_tmux() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    h.client.resize(&session.id, chan, 100, 30);
    wait_for_geometry(&h, "100x30").await;
}

/// Attach-time resize must happen before replay capture, not merely
/// before the attach request returns.
///
/// The payload fits on one 80-column row but reflows across rows at 40
/// columns. The old capture-before-resize ordering replayed the whole
/// payload contiguously even though tmux itself already reported the new
/// geometry. A fresh agent echo is the replay-completion barrier: unlike
/// bracketed-paste restoration, it is available on every supported tmux
/// version, and it cannot arrive before the replay queued ahead of it.
#[tokio::test]
async fn attach_replay_uses_the_requested_geometry() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (channel, mut first) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut initial = Vec::new();
    wait_for(&mut first, &mut initial, "FAKE-AGENT READY", 20).await;
    let payload = format!("geometry-{}", "x".repeat(50));
    h.client
        .send_input(channel, format!("{payload}\r").into_bytes());
    wait_for(&mut first, &mut initial, &payload, 10).await;
    h.client.detach(channel).await;

    let (channel, mut second) = h
        .client
        .attach(&session.id, 40, 24)
        .await
        .expect("reattach");
    let mut replay = Vec::new();
    h.client.send_input(channel, b"geometry-barrier\r".to_vec());
    wait_for(
        &mut second,
        &mut replay,
        "echo:\x1b[36mgeometry-barrier",
        10,
    )
    .await;

    assert!(
        !replay
            .windows(payload.len())
            .any(|window| window == payload.as_bytes()),
        "payload stayed contiguous, so replay was captured before the attach-time resize"
    );
}

/// A resize from a kicked CONNECTION must be dropped — the
/// connection-identity (`same_channel`) half of the Resize check.
///
/// The colliding channel ids are the point: both connections number
/// from 1, so the channel-id comparison passes for the kicked client and
/// only connection identity rejects it. Delete the `same_channel` half
/// and this fails. (The channel-id half is pinned separately by
/// `resize_from_a_stale_channel_on_the_same_connection_is_ignored`.)
#[tokio::test]
async fn resize_from_a_kicked_connection_is_ignored() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (loser_chan, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    let winner = h.second_client().await;
    let (winner_chan, mut rx2) = winner.attach(&session.id, 80, 24).await.expect("attach2");
    // The colliding ids are the point: if this ever fails, the test has
    // stopped exercising the case it exists for.
    assert_eq!(loser_chan, winner_chan, "both connections number from 1");
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;

    // Winner establishes a known geometry first.
    winner.resize(&session.id, winner_chan, 100, 30);
    wait_for_geometry(&h, "100x30").await;

    h.client.resize(&session.id, loser_chan, 111, 33);
    h.client
        .list_sessions()
        .await
        .expect("barrier after kicked resize");
    assert_geometry_stays(
        &h,
        "100x30",
        "a kicked client's resize reflowed the winner's terminal",
    )
    .await;
}

/// A resize from a stale CHANNEL on the still-attached connection must
/// be dropped — the channel-id half of the Resize check.
///
/// Within one connection (one helm, two browser tabs), a takeover
/// assigns the session a new channel; `same_channel` passes for the old
/// tab's in-flight resize, and only the channel-id comparison rejects
/// it. Delete that comparison and this fails. The sibling test above
/// pins the connection-identity half.
#[tokio::test]
async fn resize_from_a_stale_channel_on_the_same_connection_is_ignored() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (stale_chan, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    // Second attach on the SAME connection: new channel, kicks the first.
    let (live_chan, mut rx2) = h.client.attach(&session.id, 80, 24).await.expect("attach2");
    assert_ne!(
        stale_chan, live_chan,
        "one connection numbers channels uniquely"
    );
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;

    h.client.resize(&session.id, live_chan, 100, 30);
    wait_for_geometry(&h, "100x30").await;

    h.client.resize(&session.id, stale_chan, 111, 33);
    h.client
        .list_sessions()
        .await
        .expect("barrier after stale-channel resize");
    assert_geometry_stays(
        &h,
        "100x30",
        "a stale channel's resize reflowed the live attachment's terminal",
    )
    .await;
}

/// A session whose agent exits stays viewable and replayable.
///
/// This is what `remain-on-exit on` buys (SPEC.md: a stopped or exited
/// session's terminal stays viewable while its host is up). Without that
/// config line the tmux session disappears on exit and the reattach
/// below fails outright.
#[tokio::test]
async fn exited_agent_leaves_a_viewable_terminal() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, b"quit\r".to_vec());

    // Wait for the pane to actually be dead by asking tmux, not by
    // watching for the agent's farewell text. Output-watching would race
    // the process teardown this test deliberately provokes; `pane_dead`
    // is the state the assertion below actually depends on. (There is no
    // `Detached` to wait for either: `remain-on-exit` keeps the session
    // alive after the process dies, which is the property under test.)
    let sock = h.state.path().join("tmux.sock");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let out = tmux_query(&sock, &["display-message", "-p", "#{pane_dead}"]).await;
        if String::from_utf8_lossy(&out.stdout).trim() == "1" {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "agent never exited after quit"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    h.client.detach(chan).await;

    // The attach succeeding IS the contract: without `remain-on-exit on`
    // the window closes when the process exits, taking the only-window
    // session with it, and every tmux call in the attach path then fails.
    // (The replayed content is deliberately not asserted — a dead pane's
    // captured screen depends on what the exiting program left behind.)
    let (_chan2, _rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("a session whose agent exited must still be attachable");
}

/// Invocations that cannot become an argv fail the create outright, with
/// no session left behind — the same contract as a missing directory.
#[tokio::test]
async fn unparseable_invocations_error_without_creating_a_session() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let cwd = work.path().to_string_lossy().into_owned();

    let empty = h
        .client
        .create_session(&cwd, "", None, 80, 24)
        .await
        .expect_err("empty invocation must fail");
    assert!(empty.to_string().contains("empty"));
    assert_eq!(
        empty
            .downcast_ref::<SupervisorError>()
            .expect("an empty invocation must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "an empty invocation is the caller's mistake, not a server fault"
    );

    let unterminated = h
        .client
        .create_session(&cwd, "claude 'unterminated", None, 80, 24)
        .await
        .expect_err("unparseable invocation must fail");
    let unterminated_text = unterminated.to_string();
    assert!(unterminated_text.contains("parsing agent invocation"));
    // `RequestError` is attached as `.context(...)` over the `shell_words`
    // parse failure specifically so its own diagnostic keeps reaching the
    // user (see that struct's docs) — pin that it actually does, not just
    // that our own classification message survives.
    assert!(
        unterminated_text.contains("missing closing quote"),
        "error lost shell_words's own diagnostic: {unterminated_text}"
    );
    assert_eq!(
        unterminated
            .downcast_ref::<SupervisorError>()
            .expect("an unparseable invocation must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "a shell-syntax error in the invocation is the caller's mistake, not a server fault"
    );

    assert!(h.client.list_sessions().await.unwrap().is_empty());
}

/// The launch shim's sentinel is what separates "could not start" from
/// "ran and exited" (SPEC.md's error/exited split), and it is the entire
/// reason the shim exists — a shell-side sentinel after `exec` never
/// fires under zsh. Both directions are checked: failure writes it,
/// success does not. The shim must also unlink the spec on every path —
/// exec failure, malformed spec, and success alike — because it holds
/// the agent's full command line, which users put credentials into, and
/// nothing else removes it before the next supervisor restart's sweep.
///
/// A plain `#[test]`: everything here is synchronous process spawning,
/// and it needs no tmux, no supervisor, and no runtime.
#[test]
fn launch_shim_records_exec_failure_only_on_failure() {
    let dir = tempfile::tempdir().unwrap();

    let write_spec = |name: &str, argv: Vec<&str>| {
        let status_file = dir.path().join(format!("{name}.status"));
        let spec_path = dir.path().join(format!("{name}.json"));
        let spec = serde_json::json!({
            "argv": argv,
            "status_file": status_file.to_string_lossy(),
        });
        std::fs::write(&spec_path, spec.to_string()).unwrap();
        (spec_path, status_file)
    };

    let (bad_spec, bad_status) = write_spec("bad", vec!["/nonexistent/definitely-not-here"]);
    let out = std::process::Command::new(farhelm_bin())
        .args(["internal", "launch"])
        .arg(&bad_spec)
        .output()
        .expect("run shim");
    assert!(!out.status.success(), "failed exec must exit nonzero");
    let sentinel = std::fs::read_to_string(&bad_status).expect("sentinel must exist");
    assert!(
        sentinel.contains("exec_failed") && sentinel.contains("errno="),
        "sentinel must name the failure and its errno, got: {sentinel}"
    );
    assert!(
        !bad_spec.exists(),
        "the shim must unlink the credential-bearing spec even when exec fails"
    );

    let (ok_spec, ok_status) = write_spec("ok", vec!["true"]);
    let out = std::process::Command::new(farhelm_bin())
        .args(["internal", "launch"])
        .arg(&ok_spec)
        .output()
        .expect("run shim");
    assert!(out.status.success(), "successful exec must exit zero");
    assert!(
        !ok_status.exists(),
        "a successful exec must leave no sentinel — its absence is what makes an exit 'exited', not 'error'"
    );
    assert!(
        !ok_spec.exists(),
        "the shim must unlink the spec before exec — after it, no code of ours runs"
    );

    // A malformed spec takes the early-return path, which must unlink
    // too: a truncated spec still holds a credential prefix.
    let malformed_spec = dir.path().join("malformed.json");
    std::fs::write(&malformed_spec, b"{ not json").unwrap();
    let out = std::process::Command::new(farhelm_bin())
        .args(["internal", "launch"])
        .arg(&malformed_spec)
        .output()
        .expect("run shim");
    assert!(!out.status.success(), "malformed spec must exit nonzero");
    assert!(
        !malformed_spec.exists(),
        "the shim must unlink the spec even when it cannot parse it"
    );
}

/// A client kicked by a takeover must not keep typing into the pane.
///
/// The supervisor enforces this rather than trusting clients to stop, so
/// deleting that check must fail a test — before this one, it did not.
#[tokio::test]
async fn kicked_client_cannot_still_send_input() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (c1, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    let (c2, mut rx2) = h.client.attach(&session.id, 80, 24).await.expect("attach2");
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;

    // Ghost first, marker second, both on the same connection so the
    // supervisor processes them in order — by the time the marker echo
    // arrives, the ghost has already been accepted or dropped.
    h.client.send_input(c1, b"ghost-input\r".to_vec());
    h.client.send_input(c2, b"marker-input\r".to_vec());
    wait_for(&mut rx2, &mut seen2, "marker-input", 15).await;

    let transcript = String::from_utf8_lossy(&seen2);
    assert!(
        !transcript.contains("ghost-input"),
        "input from a kicked attachment reached the pane:\n{transcript}"
    );
}

/// Input authorization must hold ACROSS connections, not just within one.
///
/// Channel ids are only unique per connection — every client numbers
/// from 1 — so when the winner attaches from a different connection, its
/// channel id collides with the kicked client's. The channel-id half of
/// the check passes for the ghost input; only the connection-identity
/// half (`same_channel`) drops it. The single-connection test above
/// cannot see that half, and before this test, deleting it failed
/// nothing.
#[tokio::test]
async fn input_from_a_kicked_connection_is_dropped() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (loser_chan, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    let winner = h.second_client().await;
    let (winner_chan, mut rx2) = winner.attach(&session.id, 80, 24).await.expect("attach2");
    assert_eq!(loser_chan, winner_chan, "both connections number from 1");
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;

    // The two inputs travel on different connections, so the winner's
    // later marker is not an ordering barrier for the loser. A
    // request/reply on the LOSING connection is: its reply proves the
    // supervisor processed the ghost before the winner marker lets this
    // test finish.
    h.client.send_input(loser_chan, b"ghost-xconn\r".to_vec());
    h.client
        .list_sessions()
        .await
        .expect("barrier after kicked input");
    winner.send_input(winner_chan, b"marker-xconn\r".to_vec());
    wait_for(&mut rx2, &mut seen2, "marker-xconn", 15).await;

    let transcript = String::from_utf8_lossy(&seen2);
    assert!(
        !transcript.contains("ghost-xconn"),
        "input from a kicked connection reached the pane:\n{transcript}"
    );
}

/// Losing the supervisor connection must fail everything promptly:
/// attached terminals get an explicit `Detached`, and later requests
/// error instead of hanging their HTTP handler forever. The client
/// carries a deliberate lock-ordering invariant for exactly this
/// (`fail_all`), and nothing else exercises it. Every wait is under a
/// timeout because a hang is precisely the failure under test.
#[tokio::test]
async fn connection_loss_detaches_terminals_and_fails_requests() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();

    // A separate connection routed through a severable relay: two duplex
    // pipes joined by copy tasks. Aborting the copy tasks drops every
    // relay half at once, so BOTH endpoints see a dead transport — which
    // is what a dead socket or broken ssh pipe looks like. (Aborting the
    // server's connection task instead would not work: the split write
    // half lives on in its writer task, so the client would never see
    // EOF.)
    let (client_side, relay_a) = tokio::io::duplex(1 << 20);
    let (relay_b, server_side) = tokio::io::duplex(1 << 20);
    let sup = Arc::clone(&h.sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup, server_side).await;
    });
    let (mut ar, mut aw) = tokio::io::split(relay_a);
    let (mut br, mut bw) = tokio::io::split(relay_b);
    let relay_up = tokio::spawn(async move {
        let _ = tokio::io::copy(&mut ar, &mut bw).await;
    });
    let relay_down = tokio::spawn(async move {
        let _ = tokio::io::copy(&mut br, &mut aw).await;
    });
    let (r, w) = tokio::io::split(client_side);
    let client = SupervisorClient::start(r, w).await.expect("handshake");

    let session = client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let (_chan, mut rx) = client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // Sever the transport.
    relay_up.abort();
    relay_down.abort();

    let detached = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match rx.recv().await {
                Some(TermEvent::Detached(reason)) => return reason,
                Some(TermEvent::Data(_)) => continue,
                None => panic!("terminal stream closed without a Detached event"),
            }
        }
    })
    .await
    .expect("timed out waiting for Detached after connection loss");
    assert!(
        detached.contains("connection lost"),
        "detach reason should say the connection is gone, got: {detached}"
    );

    let err = tokio::time::timeout(Duration::from_secs(10), client.list_sessions())
        .await
        .expect("request after connection loss must fail fast, not hang")
        .expect_err("request on a dead connection must error");
    assert!(
        err.to_string().contains("connection closed"),
        "unexpected error: {err:#}"
    );

    // The session must still be usable from a healthy connection. This
    // is the third trigger of the frozen-replay hazard (the other two —
    // takeover and voluntary detach — have their own tests): the dead
    // connection's forwarder must be aborted AND awaited before a new
    // attach opens its control-mode client, or the reattach renders the
    // replay and then never updates. Asserting on a FRESH echo, not the
    // replay, is what tells those apart — replay alone arrives either
    // way.
    let (chan, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach");
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, b"alive-after-loss\r".to_vec());
    wait_for(&mut rx2, &mut seen2, "echo:", 15).await;
    wait_for(&mut rx2, &mut seen2, "alive-after-loss", 10).await;
}

/// A client may stop reading while it continues writing. The
/// supervisor's writer failure must terminate `handle_connection`
/// without waiting for read EOF, or that half-broken connection retains
/// its attachment state indefinitely.
#[tokio::test]
async fn supervisor_writer_failure_ends_a_half_broken_connection() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};

    let h = harness().await;
    let (client_side, server_inner) = tokio::io::duplex(64 * 1024);
    let fail_writes = Arc::new(AtomicBool::new(false));
    let server_side = ToggleWriteFailure {
        inner: server_inner,
        fail_writes: Arc::clone(&fail_writes),
    };
    let sup = Arc::clone(&h.sup);
    let connection = tokio::spawn(async move { handle_connection(sup, server_side).await });
    let (read_half, write_half) = tokio::io::split(client_side);
    let mut reader = FrameReader::new(read_half);
    let mut writer = FrameWriter::new(write_half);
    handshake(&mut reader, &mut writer, "helm")
        .await
        .expect("handshake");

    // Keep the request direction healthy while making the supervisor's
    // next reply fail. The connection task must not wait for us to close
    // the still-open request writer.
    fail_writes.store(true, Ordering::SeqCst);
    writer
        .write_control(&ControlMsg::ListSessions { req_id: 42 })
        .await
        .expect("request reaches supervisor");

    let result = tokio::time::timeout(Duration::from_secs(5), connection)
        .await
        .expect("supervisor connection task hung after writer failure")
        .expect("connection task panicked");
    assert!(
        result
            .expect_err("writer failure must end the connection")
            .to_string()
            .contains("frame write to client failed")
    );
}

/// A peer that stops reading — without ever erroring — must not pin
/// `handle_connection` open forever.
///
/// Before `WRITER_DRAIN_TIMEOUT` existed, the shutdown tail did
/// `drop(tx); writer_task.await;` unconditionally. That is fine for the
/// write-*error* case (the `writer_failed` oneshot already ends the
/// connection promptly, pinned by the test above), but it has no answer
/// for a peer that just stops reading: a full TCP/pipe window with
/// nothing on the other end. The writer task's `write_frame` call parks
/// with no error to report, `writer_task.await` never resolves, and
/// `handle_connection` — plus every reply still queued for it — leaks
/// for the process lifetime. This test reproduces exactly that: flood
/// the supervisor with requests without ever reading a reply (so a real
/// backlog queues up), then close only the peer's write half so the
/// supervisor's read loop sees EOF and runs the shutdown tail with the
/// writer parked mid-write and a backlog behind it. This peer makes zero
/// progress for the rest of the test, so it stays a "gone" peer under
/// `drain_writer`'s no-progress window too — the case that test coverage
/// still holds for, even though the shutdown tail no longer enforces a
/// flat deadline. Without the fix this hangs forever; with it,
/// `handle_connection` returns once a full `WRITER_DRAIN_TIMEOUT` window
/// passes without a frame landing.
#[tokio::test]
async fn writer_never_reading_peer_does_not_hang_connection_shutdown() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};

    let h = harness().await;

    // A SMALL duplex buffer — unlike the 1 MiB transports the other
    // tests in this file use — so the reply direction fills from a
    // modest, fast-to-send backlog instead of requiring an impractical
    // flood to reproduce the stall.
    let (client_side, server_side) = tokio::io::duplex(4 * 1024);
    let sup = Arc::clone(&h.sup);
    let handle = tokio::spawn(async move { handle_connection(sup, server_side).await });

    let (read_half, write_half) = tokio::io::split(client_side);
    let mut reader = FrameReader::new(read_half);
    let mut writer = FrameWriter::new(write_half);
    handshake(&mut reader, &mut writer, "helm")
        .await
        .expect("handshake");

    // Flood with cheap requests and never read a single reply. This
    // cannot itself deadlock the test: the supervisor's read loop drains
    // the peer->supervisor direction continuously (ListSessions against
    // an empty session map is nearly free — lock, clone an empty vec,
    // hand the reply to the writer task's unbounded queue), so this
    // direction keeps moving no matter how many requests are queued.
    // Only the OTHER direction — supervisor replies, which nothing here
    // ever reads — fills the small duplex buffer and stalls the
    // supervisor's writer task mid-write, which is the condition under
    // test.
    for req_id in 0..5_000u64 {
        writer
            .write_control(&ControlMsg::ListSessions { req_id })
            .await
            .expect("request direction stays open; the supervisor keeps reading it");
    }

    // Half-close: the write half goes away while the read half (which
    // this test never touches) stays open. That is what makes the
    // supervisor's read loop observe EOF and enter the shutdown tail —
    // with the writer task still parked on an unwritable reply and a
    // full backlog queued behind it.
    writer.shutdown().await.expect("half-close write side");

    tokio::time::timeout(Duration::from_secs(30), handle)
        .await
        .expect("handle_connection must return within the bounded writer drain, not hang forever")
        .expect("connection task panicked")
        .expect("a peer closing its write half cleanly is not itself a connection error");
}

/// The real socket transport: `Supervisor::serve` plus
/// `farhelm internal stdio`, which is the remote-host path with ssh
/// removed. Every other test bypasses both via an in-process pipe, so
/// without this the proxy's half-close, its final flush, and the
/// socket-path agreement between serve and connect are unexercised.
///
/// This is also the one test that sees the served socket on disk, so it
/// doubles as the check on serve()'s security-boundary side effects:
/// the launch-dir and socket modes (the ONLY authentication the
/// protocol has — dropping either mode-setting call silently yields
/// world-readable defaults under umask 022), and the startup sweep of
/// orphaned launch specs.
#[tokio::test]
async fn stdio_proxy_carries_a_real_session() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    // Declared after `state`, so it drops first: kill the server, then
    // delete the directory holding its socket. Without this guard a
    // panic anywhere below leaked the tmux server (plus login shell and
    // fake agent) forever — the exact accumulation Harness exists to
    // prevent, on the one test that cannot use it.
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));

    // An orphaned launch spec from a "previous run": serve() must sweep
    // it once — and only once — it owns the socket. It holds an agent
    // command line, which is why nothing may leave it behind.
    let orphan = state.path().join("launch").join("orphan.json");
    std::fs::write(&orphan, b"{}").expect("plant orphan spec");

    let serving = Arc::clone(&sup);
    tokio::spawn(async move {
        let _ = serving.serve().await;
    });
    // Wait for the socket rather than sleeping.
    let sock = state.path().join("supervisor.sock");
    wait_for_socket(&sock).await;

    let mut child = tokio::process::Command::new(farhelm_bin())
        .args(["internal", "stdio", "--state-dir"])
        .arg(state.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn stdio proxy");
    let stdout = child.stdout.take().expect("piped stdout");
    let stdin = child.stdin.take().expect("piped stdin");
    let client = SupervisorClient::start(stdout, stdin)
        .await
        .expect("handshake over the stdio proxy");

    // The handshake above required serve()'s accept loop, which starts
    // only after the sweep — so the orphan must be gone by now.
    assert!(
        !orphan.exists(),
        "serve() must sweep orphaned launch specs at startup"
    );
    {
        use std::os::unix::fs::PermissionsExt;
        // The launch dir, not the state dir: tempfile creates the state
        // dir 0700 itself, so asserting on it would pass with the mode
        // logic deleted. The launch dir is created by ensure_private_dir
        // in this very flow, so its mode is actually the code's doing.
        let launch = state.path().join("launch");
        let dir_mode = std::fs::metadata(&launch).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "launch dir must be owner-only");
        let sock_mode = std::fs::metadata(&sock).unwrap().permissions().mode() & 0o777;
        assert_eq!(sock_mode, 0o600, "supervisor socket must be owner-only");
    }

    let work = tempfile::tempdir().unwrap();
    let session = client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create over proxy");
    let (chan, mut rx) = client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 25).await;
    client.send_input(chan, b"through-the-proxy\r".to_vec());
    wait_for(&mut rx, &mut seen, "through-the-proxy", 15).await;
}

/// Input larger than one protocol frame must arrive intact and in order.
///
/// One `send_input` call of ~48 KiB crosses the frame-chunking boundary:
/// the helm client splits it into two 32 KiB-capped frames, with a line
/// straddling the split, and each arriving frame is handed to
/// `InputClient::send`, which further chunks it into many 256-byte
/// `send-keys -H` commands against the same dedicated input client (see
/// `tmux.rs`). Every other test sends a dozen bytes, so a truncation, a
/// reorder, or a dropped chunk at either boundary — the frame split or
/// any of the many `send-keys` chunk splits inside it — would otherwise
/// go unnoticed.
///
/// The payload is many short lines, not one long one, by necessity: the
/// pane's PTY is in canonical mode, where the kernel caps a single input
/// line at MAX_CANON (4095 bytes on Linux) and silently discards the
/// excess — a single >32 KiB line can never round-trip, no matter how
/// correct the chunking is.
#[tokio::test]
async fn large_input_survives_chunking() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            200,
            50,
        )
        .await
        .expect("create");
    let (chan, mut rx) = h.client.attach(&session.id, 200, 50).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // 3200 numbered lines ≈ 48 KiB in one send_input call. Numbering
    // makes both loss and reordering visible: every line must come back
    // as its own echo, in order.
    const LINES: usize = 3200;
    let mut input = Vec::new();
    for i in 0..LINES {
        input.extend_from_slice(format!("chunkline-{i:04}\r").as_bytes());
    }
    assert!(
        input.len() > 32 * 1024,
        "payload must exceed one 32 KiB frame to exercise the frame-chunking layer"
    );
    h.client.send_input(chan, input);

    // Wait for the final echo, then verify every line echoed in order.
    // The needle includes the fake agent's echo prefix and color code so
    // it cannot match the PTY's input echo of the same text.
    let last = format!("echo:\x1b[36mchunkline-{:04}", LINES - 1);
    wait_for(&mut rx, &mut seen, &last, 60).await;
    let transcript = String::from_utf8_lossy(&seen);
    let mut pos = 0;
    for i in 0..LINES {
        let needle = format!("echo:\x1b[36mchunkline-{i:04}");
        match transcript[pos..].find(&needle) {
            Some(at) => pos += at + needle.len(),
            None => panic!(
                "echo for line {i} missing or out of order after byte {pos} — \
                 a chunk was dropped or reordered at a chunking boundary"
            ),
        }
    }
}

/// A zero-sized attach must still produce a working terminal, at the
/// clamped 1x1 geometry.
///
/// A browser can report 0 columns mid-layout. tmux rejects `resize-window
/// -x 0` outright ("width too small"), so the driver clamps to 1. Both
/// halves are asserted: the stream still flows, AND tmux actually holds
/// the clamped geometry — without the second assertion, deleting the
/// clamp passes this test, because a failed resize during attach is
/// deliberately warn-only.
#[tokio::test]
async fn attach_with_degenerate_size_still_works() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (_chan, mut rx) = h
        .client
        .attach(&session.id, 0, 0)
        .await
        .expect("attach with 0x0 must succeed");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // The attach's resize ran before the replay that carried the marker
    // above, so a single read is race-free here.
    let out = tmux_query(
        &h.state.path().join("tmux.sock"),
        &["display-message", "-p", "#{window_width}x#{window_height}"],
    )
    .await;
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "1x1",
        "0x0 must clamp to 1x1, not fail or leave the old geometry"
    );
}

/// Attaching to a tmux session that does not exist must fail with tmux's
/// own diagnostic, not report success.
///
/// Pins a bug found in review (recorded in lore/): `%begin` only opens a
/// control-mode reply block, but was treated as "attached" — a failed
/// attach reported success and discarded tmux's reason. Nothing else
/// reaches this path: the service layer rejects unknown session ids
/// before tmux is ever asked, so only a driver-level test can regress it.
#[tokio::test]
async fn control_mode_attach_to_missing_session_reports_tmux_reason() {
    use farhelm_supervisor::tmux::TmuxDriver;

    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let driver = TmuxDriver::new(state.path());
    driver.ensure_server().await.expect("ensure server");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    // A decoy session, so tmux's refusal names the missing session
    // ("can't find session: ...") instead of the generic "no sessions" a
    // sessionless server answers with.
    driver
        .create_session(
            "decoy",
            state.path().to_str().expect("tempdir path is UTF-8"),
            80,
            24,
            &["sleep".to_string(), "60".to_string()],
        )
        .await
        .expect("decoy session");

    let err = match driver.open_replay_stream("no-such-session", "%0").await {
        Ok(_) => panic!("attaching to a missing session must fail"),
        Err(err) => err,
    };
    assert!(
        format!("{err:#}").contains("no-such-session"),
        "the error must carry tmux's own diagnostic naming the session, got: {err:#}"
    );
}

/// An untitled session takes its title from the working directory, and a
/// created session actually appears in the list — the positive form of
/// the assertion the error tests only make negatively.
#[tokio::test]
async fn created_sessions_are_listed_with_a_derived_title() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let basename = work
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let invocation = agent_cmd("internal fake-agent --script basic");

    let session = h
        .client
        .create_session(&work.path().to_string_lossy(), &invocation, None, 80, 24)
        .await
        .expect("create");
    assert_eq!(session.title, basename);

    let listed = h.client.list_sessions().await.expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0], session);
    assert_eq!(listed[0].invocation, invocation);
}

/// Attaching to a session id the supervisor does not know must fail with
/// an error naming the session, and must not damage the connection — the
/// handler's contract is that per-request failures answer with an Error
/// message, never by killing the shared connection. This also exercises
/// the client's attach-failure cleanup (the pre-registered terminal
/// channel must be released, not leaked).
#[tokio::test]
async fn attach_to_unknown_session_errors_and_connection_survives() {
    let h = harness().await;

    let err = h
        .client
        .attach("definitely-not-a-session", 80, 24)
        .await
        .expect_err("attach to an unknown session must fail");
    assert!(
        err.to_string().contains("no such session"),
        "error must name the problem, got: {err:#}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("an unknown-session attach must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound,
        "an unknown session id is a not-found, not a bad request or server fault"
    );

    // The connection is still serviceable after the refused request.
    assert!(
        h.client
            .list_sessions()
            .await
            .expect("connection must survive a refused attach")
            .is_empty()
    );
}

/// A tmux failure during cutover belongs to one attach request, not the
/// multiplexed supervisor connection.
///
/// The session remains in the supervisor's M1 in-memory index after its
/// tmux session is killed behind the supervisor's back. That creates a
/// known session whose resize and control-mode attach both fail, reaching
/// the post-takeover error path rather than the early unknown-id check.
#[tokio::test]
async fn cutover_failure_is_request_local() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let socket = h.state.path().join("tmux.sock");
    let sessions = tmux_query(&socket, &["list-sessions", "-F", "#{session_name}"]).await;
    assert!(sessions.status.success(), "list private tmux sessions");
    let tmux_name = String::from_utf8(sessions.stdout)
        .expect("tmux session names are UTF-8")
        .trim()
        .to_string();
    let killed = tmux_query(&socket, &["kill-session", "-t", &tmux_name]).await;
    assert!(killed.status.success(), "kill private tmux session");

    let error = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect_err("attach must report the missing tmux session");
    assert!(
        format!("{error:#}").contains("no sessions"),
        "attach error lost tmux's diagnostic: {error:#}"
    );
    // A tmux hiccup has no `RequestError` opinion attached anywhere in the
    // supervisor, so `error_kind` falls through to its `Internal` default —
    // pin that default explicitly, since it is the realistic path most
    // unclassified supervisor failures take.
    assert_eq!(
        error
            .downcast_ref::<SupervisorError>()
            .expect("a tmux failure during cutover must still carry a SupervisorError")
            .kind,
        ErrorKind::Internal,
        "an unclassified tmux failure is a server fault, not the caller's mistake"
    );
    let listed = h
        .client
        .list_sessions()
        .await
        .expect("connection survives cutover failure");
    assert_eq!(listed, vec![session]);
}

/// Two supervisors must never own one state dir: the second `serve()`
/// has to refuse while the first is alive (atomically — the lock, not a
/// probe-then-remove dance, is what prevents the TOCTOU where the loser
/// unlinks the winner's freshly bound socket), and a stale socket file
/// left by a dead supervisor must not block the next one from binding.
#[tokio::test]
async fn serve_refuses_a_second_supervisor_but_replaces_a_stale_socket() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");

    // Half 1: live supervisor → second serve() refuses.
    let state = tempfile::tempdir().expect("tempdir");
    let sup1 = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor 1");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let serving = Arc::clone(&sup1);
    tokio::spawn(async move {
        let _ = serving.serve().await;
    });
    let sock = state.path().join("supervisor.sock");
    wait_for_socket(&sock).await;
    let sup2 = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor 2");
    let err = sup2
        .serve()
        .await
        .expect_err("a second supervisor on the same state dir must refuse");
    assert!(
        err.to_string().contains("already running"),
        "refusal must say why, got: {err:#}"
    );
    // The winner's socket must still be there — the loser must not have
    // unlinked it on its way out.
    assert!(
        sock.exists(),
        "refused supervisor must not remove the live socket"
    );

    // Half 2: stale socket file (no listener behind it) → serve() binds.
    let state2 = tempfile::tempdir().expect("tempdir");
    let sup3 = Supervisor::new_with_exe(state2.path(), farhelm_bin().into())
        .await
        .expect("supervisor 3");
    let _tmux2 = TmuxServerGuard(state2.path().join("tmux.sock"));
    let stale = state2.path().join("supervisor.sock");
    std::fs::write(&stale, b"").expect("plant stale socket file");
    let serving = Arc::clone(&sup3);
    tokio::spawn(async move {
        let _ = serving.serve().await;
    });
    let connected = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if farhelm_supervisor::service::connect(state2.path())
                .await
                .is_ok()
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        connected.is_ok(),
        "a stale socket file must not stop the next supervisor from binding"
    );
}

/// The stdio proxy's half-close contract: stdin EOF must not tear the
/// proxy down before replies still in flight from the supervisor reach
/// stdout — and once the supervisor closes, the proxy process must
/// actually exit (its stdin read parks on the blocking pool, which a
/// plain runtime drop would wait on forever; over ssh a lingering proxy
/// keeps the channel open and turns a supervisor crash into a silently
/// frozen terminal). The wait_with_output timeout pins the exit; the
/// reply assertion pins the half-close.
#[tokio::test]
async fn stdio_proxy_half_close_delivers_in_flight_replies() {
    use tokio::io::AsyncWriteExt;

    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let serving = Arc::clone(&sup);
    tokio::spawn(async move {
        let _ = serving.serve().await;
    });
    let sock = state.path().join("supervisor.sock");
    wait_for_socket(&sock).await;

    // Raw frames, no SupervisorClient: hello then a request, then EOF —
    // the reply is "in flight" precisely because stdin closed first.
    let mut input = Vec::new();
    Frame::control(&ControlMsg::hello("helm"))
        .encode(&mut input)
        .unwrap();
    Frame::control(&ControlMsg::ListSessions { req_id: 1 })
        .encode(&mut input)
        .unwrap();

    let mut child = tokio::process::Command::new(farhelm_bin())
        .args(["internal", "stdio", "--state-dir"])
        .arg(state.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn stdio proxy");
    let mut stdin = child.stdin.take().expect("piped stdin");
    stdin.write_all(&input).await.expect("write frames");
    drop(stdin); // EOF: half-closes the proxy's upstream

    let out = tokio::time::timeout(Duration::from_secs(15), child.wait_with_output())
        .await
        .expect("proxy must exit once the supervisor side closes — a hang here is the bug")
        .expect("proxy output");
    assert!(out.status.success(), "proxy must exit cleanly");

    let mut rest: &[u8] = &out.stdout;
    let mut got_reply = false;
    while let Some((frame, used)) = Frame::decode(rest).expect("well-formed frames on stdout") {
        if frame.kind == FrameKind::Control
            && let Ok(ControlMsg::SessionList { req_id: 1, .. }) = parse_control(&frame)
        {
            got_reply = true;
        }
        rest = &rest[used..];
    }
    assert!(
        got_reply,
        "the reply in flight at stdin EOF must still reach stdout"
    );
}

/// A Detach from a kicked connection must not tear down the winner's
/// attachment.
///
/// The helm calls `detach` unconditionally on every terminal teardown
/// path, so after a cross-connection takeover the kicked helm's routine
/// cleanup carries the COLLIDING channel id (both connections number
/// from 1) — only the connection-identity half of the Detach guard
/// stands between that cleanup and the winner's live attachment. Its
/// siblings (input, resize) have this exact test; before this one,
/// deleting `same_channel` from the Detach arm failed nothing.
#[tokio::test]
async fn detach_from_a_kicked_connection_does_not_kill_the_winner() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (loser_chan, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    let winner = h.second_client().await;
    let (winner_chan, mut rx2) = winner.attach(&session.id, 80, 24).await.expect("attach2");
    assert_eq!(loser_chan, winner_chan, "both connections number from 1");
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;

    // The kicked helm's routine cleanup: same channel id, wrong
    // connection. The winner's terminal must stay live — proven by a
    // fresh echo, which a torn-down forwarder can never deliver.
    h.client.detach(loser_chan).await;
    h.client
        .list_sessions()
        .await
        .expect("barrier after foreign detach");
    winner.send_input(winner_chan, b"survived-foreign-detach\r".to_vec());
    wait_for(&mut rx2, &mut seen2, "survived-foreign-detach", 15).await;
}

/// Creating a session with degenerate dimensions must clamp, exactly
/// like the resize path: `new-session -x 0` is a hard tmux error, so
/// without the clamp the create fails outright — and every other test
/// creates at sane sizes, so deleting the clamp used to fail nothing.
#[tokio::test]
async fn create_with_degenerate_size_clamps_to_1x1() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            0,
            0,
        )
        .await
        .expect("create with 0x0 must succeed via the clamp");
    wait_for_geometry(&h, "1x1").await;

    // And the session is real, not just accepted: it must be listed.
    let listed = h.client.list_sessions().await.expect("list");
    assert!(listed.iter().any(|s| s.id == session.id));
}

/// Extract every two-hex-digit token from `hexecho`'s output, discarding
/// which line or read() call each token arrived on.
///
/// `hexecho` flushes a fresh line per raw `read()`, and reads can split
/// arbitrarily at PTY/tmux boundaries — a single input byte sequence can
/// legitimately arrive as hex tokens on two or more separate lines. A
/// prior version of this test's assertion instead required the whole
/// expected payload's hex to appear on one line, which is only true when
/// the PTY happens not to split that particular read; this reassembles
/// the byte stream in order regardless of where the line breaks fell, so
/// the assertion below holds independent of read-boundary behavior.
fn hex_tokens(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .filter(|token| token.len() == 2 && token.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(|token| u8::from_str_radix(token, 16).expect("validated two ASCII hex digits"))
        .collect()
}

/// Byte-verbatim input delivery, pinned end to end through a raw-mode
/// fixture the paste-buffer bug could not hide from.
///
/// This is the regression test for the paste-buffer input-mangling bug:
/// `paste-buffer -d -r`, the mechanism this replaced, caret-escaped
/// control bytes on their way into the pane (DEL arrived as the two
/// characters `^?`, ESC as `^[`, ctrl-C as `^C` — verified against tmux
/// 3.7b) while passing every other test in this file, because `basic`'s
/// canonical-mode reading let the pty's own line discipline mask the
/// difference. `hexecho` reads its stdin in raw mode specifically so
/// nothing between the wire and this assertion can paper over a mangled
/// byte.
#[tokio::test]
async fn input_bytes_survive_verbatim_through_hexecho() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script hexecho"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // Exactly the bytes paste-buffer was observed to mangle: DEL, ESC
    // (as the opener of the ArrowUp sequence "\x1b[A"), and ETX (ctrl-C).
    h.client.send_input(chan, b"a\x7fb\x1b[A\x03".to_vec());
    // A plain printable byte with no special meaning to tmux or a
    // raw-mode pty, sent as a separate call. Its own hex line is the sync
    // point that proves the control-byte input above already made it
    // through, without depending on how `hexecho`'s read() calls happen
    // to chunk the payload into lines.
    h.client.send_input(chan, b"z".to_vec());
    wait_for(&mut rx, &mut seen, "7a", 10).await;

    // Reassemble the hex byte stream across every line before asserting:
    // see `hex_tokens` for why line boundaries cannot be trusted here.
    let transcript = String::from_utf8_lossy(&seen);
    let bytes = hex_tokens(&transcript);
    let contains_sequence = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
    assert!(
        contains_sequence(&[0x61, 0x7f, 0x62, 0x1b, 0x5b, 0x41, 0x03]),
        "control bytes must arrive verbatim; transcript:\n{transcript}"
    );
    assert!(
        !contains_sequence(&[0x5e, 0x3f]),
        "DEL must not arrive caret-escaped as ^?: {transcript}"
    );
    assert!(
        !contains_sequence(&[0x5e, 0x5b]),
        "ESC must not arrive caret-escaped as ^[: {transcript}"
    );
    assert!(
        !contains_sequence(&[0x5e, 0x43]),
        "ETX (ctrl-C) must not arrive caret-escaped as ^C: {transcript}"
    );
}

/// PLAN_M2.md's headline SQLite behavior: session metadata must survive
/// the supervisor process, not just the tmux server underneath it.
///
/// A brand-new `Supervisor` on the harness's state dir stands in for a
/// restarted process — `new_with_exe` (unlike `serve()`) takes no
/// socket-exclusivity lock, so nothing here fights the harness's own
/// supervisor for the same reason `serve_refuses_a_second_supervisor_...`
/// already runs several `Supervisor`s side by side. The harness's private
/// tmux server is left running throughout (its `TmuxServerGuard` only
/// tears it down when the harness itself drops, at the end of the test),
/// which is the normal shape PLAN_M2.md describes: the tmux server
/// outliving a supervisor restart. Listing alone would not catch a bug
/// that persists metadata but loses the live reconnect, so this also
/// attaches and round-trips input through the reloaded entry.
#[tokio::test]
async fn persisted_sessions_survive_a_supervisor_restart() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction on the same state dir");
    let client2 = connect_client(&sup2).await;

    let listed = client2.list_sessions().await.expect("list after restart");
    assert_eq!(
        listed,
        vec![session.clone()],
        "session metadata must round-trip identically from SQLite"
    );

    let (chan, mut rx) = client2
        .attach(&session.id, 80, 24)
        .await
        .expect("attach must succeed: the tmux session is still alive");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    client2.send_input(chan, b"still-alive\r".to_vec());
    wait_for(&mut rx, &mut seen, "echo:", 10).await;
    wait_for(&mut rx, &mut seen, "still-alive", 5).await;
}

/// PLAN_M2.md's "restart gap": a session whose tmux server did NOT
/// survive a supervisor restart must still be listed — the whole point of
/// persisting metadata separately from tmux liveness — but attaching to
/// it must fail loudly rather than fabricate a terminal that no longer
/// exists.
///
/// The private tmux server is killed directly on its socket, standing in
/// for "the host rebooted" or "tmux crashed independently of the
/// supervisor" — the case M1 had no answer for at all (the session simply
/// vanished from the in-memory map). The second `Supervisor` construction
/// starts a fresh, empty tmux server on the same socket (an ordinary
/// consequence of `ensure_server`'s idempotent-adopt-or-start behavior),
/// so `has_session` genuinely finds nothing for the reloaded row's
/// `tmux_name` — this is not a mocked failure.
#[tokio::test]
async fn restart_gap_lists_sessions_without_a_terminal_and_attach_fails() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let sock = h.state.path().join("tmux.sock");
    kill_tmux_server_and_wait(&sock).await;

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction after the tmux server died");
    let client2 = connect_client(&sup2).await;

    let listed = client2
        .list_sessions()
        .await
        .expect("list after restart gap");
    assert_eq!(
        listed,
        vec![session.clone()],
        "a session must stay listed even once its tmux server is gone — \
         vanishing is exactly what this PR exists to prevent"
    );

    let err = client2
        .attach(&session.id, 80, 24)
        .await
        .expect_err("attach must fail: this entry's terminal did not survive the restart");
    assert!(
        err.to_string().contains("no terminal"),
        "error must name the missing terminal, got: {err:#}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a terminal-less attach must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound,
        "a vanished terminal is a not-found, not a bad request or server fault"
    );
}

/// The restart-gap decision is PER SESSION, not one answer applied to the
/// whole reloaded batch.
///
/// Two sessions exist; only one's tmux session is killed directly (the
/// other, and the private tmux server itself, are left untouched). An
/// implementation that probes `has_session` once and reuses the answer
/// for every row — or that otherwise conflates "the server is gone" with
/// "this one session is gone" — would either lose the live session too or
/// wrongly keep the dead one attachable; this test fails either way,
/// which is exactly the coverage gap a single-session restart-gap test
/// cannot close.
#[tokio::test]
async fn restart_gap_is_decided_per_session() {
    let h = harness().await;
    let (alive_session, _work1) = basic_session(&h).await;
    let (dead_session, _work2) = basic_session(&h).await;

    // Mirrors `create_session`'s own derivation (`service.rs`): the tmux
    // session name is `fh-` plus the FULL session id (not a truncated
    // prefix — see that call site for why a prefix is unsafe).
    let dead_tmux_name = format!("fh-{}", dead_session.id);
    let sock = h.state.path().join("tmux.sock");
    let killed = tmux_query(&sock, &["kill-session", "-t", &dead_tmux_name]).await;
    assert!(
        killed.status.success(),
        "test setup: kill-session must succeed, got: {}",
        String::from_utf8_lossy(&killed.stderr)
    );

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction after one session's tmux died");
    let client2 = connect_client(&sup2).await;

    let mut listed = client2
        .list_sessions()
        .await
        .expect("list after a partial restart gap");
    listed.sort_by(|a, b| a.id.cmp(&b.id));
    let mut expected = vec![alive_session.clone(), dead_session.clone()];
    expected.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(
        listed, expected,
        "both sessions must remain listed regardless of which one's terminal died"
    );

    let (chan, mut rx) = client2
        .attach(&alive_session.id, 80, 24)
        .await
        .expect("the untouched session must still attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    client2.send_input(chan, b"still-alive\r".to_vec());
    wait_for(&mut rx, &mut seen, "echo:", 10).await;
    wait_for(&mut rx, &mut seen, "still-alive", 5).await;

    let err = client2
        .attach(&dead_session.id, 80, 24)
        .await
        .expect_err("the killed session's attach must fail");
    assert!(
        err.to_string().contains("no terminal"),
        "error must name the missing terminal, got: {err:#}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a terminal-less attach must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound,
        "a vanished terminal is a not-found, not a bad request or server fault"
    );
}

// Not tested here: a DB write failure during `create_session` (the
// kill-the-just-created-tmux-session unwind path). Reproducing it needs
// fault injection into the SQLite connection or the filesystem beneath
// it, which M3 is expected to bring a seam for (PLAN.md's milestone
// ladder). A filesystem hack (a read-only database file, say) would buy
// little signal this far ahead of that seam existing, so it is skipped
// rather than improvised. The unwind logic itself — kill tmux, still
// return the DB error — is covered by code review and the ordinary
// create-path tests exercising the happy side of the same call.

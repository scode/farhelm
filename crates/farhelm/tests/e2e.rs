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

use farhelm_helm::{SupervisorClient, SupervisorError, TermEvent, TermStream};
use farhelm_proto::io::parse_control;
use farhelm_proto::{
    ControlMsg, ErrorKind, Frame, FrameKind, SessionInfo, SessionStatus, TerminalSelector,
};
use farhelm_supervisor::agent_kind::{CaptureWindow, CaptureWindowBounds};
use farhelm_supervisor::launch::{spec_path_for_launch, status_path_for_spec};
use farhelm_supervisor::service::{
    CaptureStoreFault, CreateCrashSeam, CreateStage, LIST_SESSION_CAP, SessionSnapshot, Supervisor,
    SupervisorSeams, SupervisorTimeouts, handle_connection,
};
use farhelm_supervisor::store::{
    LastOutcome, Reservation, ReservationOutcome, SessionStore, StoredSession,
};
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

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

/// The pane id (`%N`) tmux currently reports for `tmux_name`'s session —
/// used to independently confirm a test's own precondition about pane-id
/// reuse, rather than assuming it from the tmux server's fresh-start
/// behavior alone.
async fn pane_id_of(sock: &std::path::Path, tmux_name: &str) -> String {
    let out = tmux_query(
        sock,
        &["display-message", "-p", "-t", tmux_name, "#{pane_id}"],
    )
    .await;
    assert!(
        out.status.success(),
        "test setup: querying the pane id for {tmux_name} must succeed, got: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("pane id is UTF-8")
        .trim()
        .to_string()
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
///
/// That ordering rule extends to DESTRUCTURING a [`Harness`]: the fields
/// become plain locals dropped in reverse pattern order, so a pattern
/// naming the guard before `state` silently inverts the rule and leaks the
/// server (measured: three per full run, all from that one shape). The
/// guard is deliberately NOT folded into the tempdir type that would make
/// this unconstructible — one test drops the guard mid-run, on purpose, to
/// simulate the tmux server dying across a reboot while keeping the state
/// directory intact.
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

/// Attach an expected `status` to a cloned `SessionInfo` before an
/// equality assertion.
///
/// `list_sessions` computes `status` fresh from tmux on every call
/// (`service.rs`'s `ListSessions` handler) rather than trusting whatever a
/// caller last saw — so a listing assertion built from a `SessionInfo`
/// returned by `create_session` (always `Unknown`, a create-time
/// placeholder — see `service.rs`'s `create_session` doc comment) must
/// say explicitly what status that same row is expected to carry by the
/// time THIS call observes it, instead of silently reusing the
/// create-time value (which would make the assertion pass or fail on an
/// unrelated coincidence whenever the two happen to agree).
fn with_status(mut session: SessionInfo, status: SessionStatus) -> SessionInfo {
    session.status = status;
    session
}

/// Poll `list_sessions` until `session_id`'s status is no longer `Alive`,
/// returning the settled `SessionInfo`.
///
/// Status is computed fresh from tmux at LIST time, never pushed
/// (`service.rs`'s `ListSessions` handler) — so observing a transition
/// (an agent exiting on its own, a stop's kill sweep completing) needs a
/// bounded poll rather than a single read racing tmux's own
/// `pane_dead`/`pane_dead_status` bookkeeping.
async fn wait_for_non_alive_status(
    client: &SupervisorClient,
    session_id: &str,
    secs: u64,
) -> SessionInfo {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let listed = client
            .list_sessions()
            .await
            .expect("list while polling for a status transition");
        if let Some(found) = listed.sessions.iter().find(|s| s.id == session_id)
            && !matches!(found.status, SessionStatus::Alive)
        {
            return found.clone();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "session {session_id} never left Alive status within {secs}s"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Whether this host's tmux reliably records a dead pane's exit status.
///
/// tmux 3.4 (Ubuntu 24.04's package, so CI's) can PERMANENTLY report a
/// dead pane with an empty `#{pane_dead_status}` under parallel load —
/// measured directly against raw tmux, no farhelm involved: 40 concurrent
/// one-pane servers whose pane runs `sh -c 'sleep 0.3; exit 0'` left 6
/// panes dead with no status on 3.4, and 0 on 3.7b. The supervisor
/// already reports that honestly as `Exited { exit_code: None }`
/// (SPEC.md: exit code "when known"), so this is a fact about what the
/// exit-code-precision tests may assert per tmux version, not a product
/// gap to code around.
fn tmux_records_exit_codes_reliably() -> bool {
    let out = std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .expect("running tmux -V");
    let v = String::from_utf8_lossy(&out.stdout);
    // "tmux 3.4" / "tmux 3.7b" — compare (major, minor) numerically;
    // trailing letters do not matter at this boundary.
    let nums: Vec<u32> = v
        .split_whitespace()
        .nth(1)
        .unwrap_or("0.0")
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect();
    (
        nums.first().copied().unwrap_or(0),
        nums.get(1).copied().unwrap_or(0),
    ) > (3, 4)
}

/// Poll `list_sessions` until `session_id` reports `Exited` with the
/// expected code — or, on a tmux that loses exit codes under load
/// ([`tmux_records_exit_codes_reliably`]), with no code at all.
///
/// Two separate races justify the polling shape. First, tmux records
/// `pane_dead` and `pane_dead_status` in separate steps, so a poll can
/// land in the window where the pane is dead but its code is not yet
/// recorded — the list honestly reports `Exited { exit_code: None }` for
/// that instant, and asserting on the FIRST non-alive observation fails
/// on a loaded machine (this bit CI while every local run passed).
/// Second, tmux 3.4 can lose the code PERMANENTLY, so on such hosts an
/// `Exited { exit_code: None }` that persists to the deadline counts as
/// the accepted outcome rather than a failure — the precision assertion
/// only binds where tmux itself is trustworthy.
async fn wait_for_exit_code(
    client: &SupervisorClient,
    session_id: &str,
    expected: i32,
    secs: u64,
) -> SessionInfo {
    let tmux_reliable = tmux_records_exit_codes_reliably();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    let mut last_seen: Option<SessionInfo> = None;
    loop {
        let listed = client
            .list_sessions()
            .await
            .expect("list while polling for a status transition");
        if let Some(found) = listed.sessions.iter().find(|s| s.id == session_id) {
            match found.status {
                SessionStatus::Exited {
                    exit_code: Some(code),
                } if code == expected => return found.clone(),
                SessionStatus::Exited { exit_code } => {
                    assert!(
                        exit_code.is_none(),
                        "session {session_id} exited with {exit_code:?}, expected \
                         Some({expected})"
                    );
                }
                _ => {}
            }
            last_seen = Some(found.clone());
        }
        if tokio::time::Instant::now() >= deadline {
            if !tmux_reliable
                && let Some(found) = &last_seen
                && found.status == (SessionStatus::Exited { exit_code: None })
            {
                // This tmux is known to lose codes; a persistent None is
                // the documented accepted outcome, not a failure.
                return found.clone();
            }
            panic!(
                "session {session_id} never reached Exited {{ Some({expected}) }} within \
                 {secs}s (last observed: {last_seen:?})"
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Boot a supervisor on a throwaway state dir and connect a client to it.
async fn harness() -> Harness {
    harness_with_timeouts(SupervisorTimeouts::default()).await
}

/// Like [`harness`], but with the supervisor's gone-not-slow timeouts
/// shortened.
///
/// The seam exists because both production values are a minute — a
/// deliberate choice (a false detach is cheap, a missed one pins buffers),
/// but far longer than any test can afford to wait out. Injected through
/// the constructor rather than an environment variable: this repo's tests
/// never mutate the process environment, and a per-process knob would be
/// shared by every concurrently-running harness in this file anyway.
async fn harness_with_timeouts(timeouts: SupervisorTimeouts) -> Harness {
    harness_with_seams(timeouts, SupervisorSeams::default()).await
}

/// Like [`harness_with_timeouts`], but with the supervisor's injection
/// points supplied too — the conversation-capture tests' entry point,
/// since they need both a private agent home and a capture window short
/// enough to prove two sessions in one directory do NOT overlap without
/// waiting out a production minute.
async fn harness_with_seams(timeouts: SupervisorTimeouts, seams: SupervisorSeams) -> Harness {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let sup = Supervisor::new_with_seams(state.path(), farhelm_bin().into(), timeouts, seams)
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
async fn wait_for(rx: &mut TermStream, seen: &mut Vec<u8>, needle: &str, secs: u64) {
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

/// Like [`wait_for`], but ordered: first wait until `first` appears, then
/// keep reading until `then` appears strictly AFTER `first`'s position.
///
/// Exists for the snapshot replays whose fixture died still ON the
/// alternate screen: whether a dead pane's capture retains the app's last
/// frame or substitutes tmux's "Pane is dead" placeholder is
/// version-dependent (3.4 retains it, 3.7b substitutes — observed
/// directly on both), so the PREFILL may already contain the same marker
/// text the snapshot suffix carries. A plain `wait_for(marker)` then
/// returns before the divider/snapshot frames ever arrive and any
/// assertion about them races. Anchoring the content wait after the
/// divider's own position is version-proof: the divider exists only in
/// the suffix.
async fn wait_for_after(
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    first: &str,
    then: &str,
    secs: u64,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    let mut ended: Option<String> = None;
    loop {
        let text = String::from_utf8_lossy(seen).into_owned();
        if let Some(idx) = text.find(first)
            && text[idx + first.len()..].contains(then)
        {
            return;
        }
        if let Some(reason) = ended {
            panic!(
                "stream ended ({reason}) without {then:?} after {first:?}; transcript so far:\n\
                 {text}"
            );
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => seen.extend_from_slice(&bytes),
            Ok(Some(TermEvent::Detached(reason))) => {
                while let Ok(TermEvent::Data(bytes)) = rx.try_recv() {
                    seen.extend_from_slice(&bytes);
                }
                ended = Some(reason);
            }
            Ok(None) => ended = Some("closed".to_string()),
            Err(_) => {
                panic!("timed out waiting for {then:?} after {first:?}; transcript so far:\n{text}")
            }
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
async fn collect_counter_through(rx: &mut TermStream, target: u64) -> Vec<u8> {
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

    h.client.send_input(chan, b"hello-farhelm\r".to_vec()).await;
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
    h.client
        .send_input(chan, b"before-reattach\r".to_vec())
        .await;
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
        .send_input(chan2, b"live-after-reattach\r".to_vec())
        .await;
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
    h.client.send_input(c2, b"still-alive\r".to_vec()).await;
    wait_for(&mut rx2, &mut seen2, "still-alive", 10).await;
}

/// Wait for an attachment's `Detached` notice and return its reason.
///
/// Every takeover test needs this same wait, and the two failure modes it
/// distinguishes are exactly the ones a bug in the takeover path
/// produces: a stream that ends without any notice at all (the client was
/// torn down silently) versus one that simply never hears anything (the
/// incumbent was never kicked). Panicking with that distinction is the
/// point — an `Option` return would let a caller blur them.
async fn expect_detached(rx: &mut TermStream, secs: u64) -> String {
    tokio::time::timeout(Duration::from_secs(secs), async {
        while let Some(ev) = rx.recv().await {
            if let TermEvent::Detached(reason) = ev {
                return reason;
            }
        }
        panic!("attachment stream ended without a Detached notice");
    })
    .await
    .expect("timed out waiting for a Detached notice")
}

/// Two DISTINCT leases are two clients, so the second attach takes the
/// first over — SPEC.md's one-attached-client rule, now enforced by lease
/// identity rather than by "any second attach wins" (PLAN_M4.md item 3).
///
/// The loser must learn about it (the takeover reason on its own channel)
/// AND stop being able to type: a takeover that detached the stream but
/// left the input route live would leave a kicked client executing
/// commands in the winner's agent terminal. Both halves are asserted
/// because the lease check is what decides the first half and the
/// per-terminal cutover is what decides the second — a lease sweep that
/// forgot to remove the attachment would still send the notice.
#[tokio::test]
async fn an_attach_under_a_different_lease_takes_over_and_silences_the_loser() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (loser_chan, mut rx1) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "lease-one")
        .await
        .expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    let (winner_chan, mut rx2) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "lease-two")
        .await
        .expect("attach2");
    let reason = expect_detached(&mut rx1, 10).await;
    assert!(
        reason.contains("another client"),
        "the loser must be told it was taken over, got: {reason}"
    );

    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;
    // Ghost then marker on the SAME connection, so the supervisor has
    // decided the ghost's fate by the time the marker echoes back (the
    // ordering trick `kicked_client_cannot_still_send_input` uses).
    h.client
        .send_input(loser_chan, b"ghost-lease\r".to_vec())
        .await;
    h.client
        .send_input(winner_chan, b"marker-lease\r".to_vec())
        .await;
    wait_for(&mut rx2, &mut seen2, "marker-lease", 15).await;
    let transcript = String::from_utf8_lossy(&seen2);
    assert!(
        !transcript.contains("ghost-lease"),
        "input from a lease that lost the takeover reached the pane:\n{transcript}"
    );
}

/// The SAME lease reattaching to the SAME terminal is an ordinary
/// reconnect: the incumbent channel is still cut over, but it is told a
/// REPLACED reason rather than a takeover one.
///
/// Two failures in one test. The mechanism could plausibly go wrong in
/// the "helpful" direction — recognizing the incumbent as the same client
/// and leaving it in place would give one terminal two live forwarders,
/// the overlapping-control-client state the whole attach path exists to
/// avoid — so the cutover and its replay must still happen. And the
/// REASON must not be the takeover string: equal non-empty leases are one
/// client reconnecting (`ControlMsg::Attach`'s contract), so "another
/// client attached" would raise a takeover banner accusing a second user
/// who does not exist. A client that renders detach reasons verbatim
/// makes that difference visible to the user, which is why it is pinned
/// here rather than left to the supervisor's internal accounting.
#[tokio::test]
async fn a_same_lease_reattach_to_the_same_terminal_is_an_ordinary_cutover() {
    const LEASE: &str = "one-client-reconnecting";
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan1, mut rx1) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, LEASE)
        .await
        .expect("attach");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;
    h.client
        .send_input(chan1, b"before-same-lease\r".to_vec())
        .await;
    wait_for(&mut rx1, &mut seen1, "before-same-lease", 15).await;

    let (chan2, mut rx2) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, LEASE)
        .await
        .expect("reattach");
    let reason = expect_detached(&mut rx1, 10).await;
    assert!(
        reason.contains("replaced by a newer attachment"),
        "a same-lease reattach must tell the incumbent it was replaced, got: {reason}"
    );
    assert!(
        !reason.contains("another client"),
        "a client reconnecting under its own lease must never be told another client took \
         over, got: {reason}"
    );

    // Replay, then live: exactly what a reconnect promises.
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "before-same-lease", 20).await;
    h.client
        .send_input(chan2, b"after-same-lease\r".to_vec())
        .await;
    wait_for(&mut rx2, &mut seen2, "after-same-lease", 15).await;
}

/// The empty lease is not a lease: an un-leased attach takes over a
/// leased one, and a leased attach takes over an un-leased one.
///
/// Both directions in one test because they are one rule — the empty
/// lease matches nothing, not even another empty lease — and it is the
/// entire compatibility story for every pre-M4 client (and for the helm,
/// which sends no lease until PLAN_M4.md item 5). Get it wrong by
/// treating empty as a shared identity and two unrelated legacy clients
/// silently share a session; get it wrong in the other direction and a
/// legacy client can never reclaim a session a leased client holds.
#[tokio::test]
async fn the_empty_lease_takes_over_everything_and_is_taken_over_by_anything() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    // Leased incumbent, un-leased newcomer.
    let (_leased_chan, mut leased_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "held-lease")
        .await
        .expect("leased attach");
    let mut leased_seen = Vec::new();
    wait_for(&mut leased_rx, &mut leased_seen, "FAKE-AGENT READY", 20).await;

    let (_legacy_chan, mut legacy_rx) = h.client.attach(&session.id, 80, 24).await.expect("legacy");
    let reason = expect_detached(&mut leased_rx, 10).await;
    assert!(
        reason.contains("another client"),
        "an un-leased attach must take over a leased holder, got: {reason}"
    );
    let mut legacy_seen = Vec::new();
    wait_for(&mut legacy_rx, &mut legacy_seen, "FAKE-AGENT READY", 15).await;

    // Un-leased incumbent, leased newcomer.
    let (_new_chan, mut new_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "fresh-lease")
        .await
        .expect("leased reattach");
    let reason = expect_detached(&mut legacy_rx, 10).await;
    assert!(
        reason.contains("another client"),
        "a leased attach must take over an un-leased holder, got: {reason}"
    );
    let mut new_seen = Vec::new();
    wait_for(&mut new_rx, &mut new_seen, "FAKE-AGENT READY", 15).await;
}

/// An over-cap lease is refused as a bad REQUEST, and refused before the
/// attach has taken anything over.
///
/// The lease is retained for the life of every attachment made under it,
/// so an unbounded one is retained memory a client can mint from a single
/// oversized control frame — the reason the cap exists at all. Both
/// halves matter: the refusal itself, and its placement ahead of the
/// takeover, because a check that ran after the lease sweep would let any
/// client detach any other by sending garbage it knows will be rejected.
#[tokio::test]
async fn an_over_cap_lease_is_refused_without_disturbing_the_incumbent() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (holder_chan, mut holder_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            "holding-lease",
        )
        .await
        .expect("attach the agent terminal");
    let mut holder_seen = Vec::new();
    wait_for(&mut holder_rx, &mut holder_seen, "FAKE-AGENT READY", 20).await;

    // One byte over: the cap is 128, and a request that is refused must
    // be refused for its size alone, not for anything else about it.
    let over_cap = "x".repeat(129);
    let err = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, &over_cap)
        .await
        .expect_err("an over-cap lease must be refused");
    assert!(
        err.to_string().contains("lease"),
        "the error must say which field was too big, got: {err:#}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("an over-cap lease must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "an over-cap lease is a malformed request, not a not-found or a server fault"
    );

    // The incumbent is untouched: no detach notice, and still typing.
    let detached = drain_for(&mut holder_rx, &mut holder_seen, Duration::from_secs(1)).await;
    assert_eq!(
        detached, None,
        "a refused over-cap attach detached the session's live attachment"
    );
    h.client
        .send_input(holder_chan, b"survived-the-lease\r".to_vec())
        .await;
    wait_for(&mut holder_rx, &mut holder_seen, "survived-the-lease", 15).await;
}

/// The lease cap counts BYTES, not characters, and admits a lease that
/// sits exactly on it.
///
/// What is bounded is retained memory and frame content, both of which
/// are byte quantities — so a `chars().count()` cap would let a
/// multibyte lease carry several times the memory the cap names. The
/// exact-cap case is the other half of the same boundary: an off-by-one
/// that refused it would break any client that sizes its ids to the
/// documented limit.
#[tokio::test]
async fn the_lease_cap_counts_bytes_not_characters() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let exactly_at_cap = "x".repeat(128);
    let (_chan, mut rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            &exactly_at_cap,
        )
        .await
        .expect("a lease exactly at the cap must be accepted");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // 64 two-byte characters: 128 bytes, right on the cap.
    let multibyte_at_cap = "é".repeat(64);
    assert_eq!(multibyte_at_cap.len(), 128, "test fixture is 128 bytes");
    let (_chan2, mut rx2) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            &multibyte_at_cap,
        )
        .await
        .expect("a multibyte lease at the byte cap must be accepted");
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;

    // 65 of them: 65 characters — comfortably under any character-count
    // reading of the cap — but 130 bytes, which is over it.
    let multibyte_over_cap = "é".repeat(65);
    assert_eq!(multibyte_over_cap.chars().count(), 65);
    assert_eq!(multibyte_over_cap.len(), 130);
    let err = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            &multibyte_over_cap,
        )
        .await
        .expect_err("a lease over the BYTE cap must be refused even when few characters");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("an over-cap lease must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "the cap must be counted in bytes, not characters"
    );
}

/// Attaching a terminal tab is a `NotFound` that names the tab, because
/// no supervisor serves tabs yet (PLAN_M4.md item 2 is the next PR).
///
/// The alternative a selector-shaped attach path could drift into is
/// silently falling back to the agent terminal, which `TerminalSelector`
/// explicitly forbids: attaching the WRONG terminal is worse than
/// failing.
///
/// The refusal must also be free of SIDE EFFECTS, which the incumbent
/// under a different lease pins: terminal resolution happens before the
/// takeover, so an attach nobody can honor must never cost the session's
/// current client its attachment. Get that order wrong and any client
/// could detach any other by naming a tab that does not exist.
#[tokio::test]
async fn attaching_a_terminal_tab_is_a_not_found_that_names_the_tab() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (holder_chan, mut holder_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            "holding-lease",
        )
        .await
        .expect("attach the agent terminal");
    let mut holder_seen = Vec::new();
    wait_for(&mut holder_rx, &mut holder_seen, "FAKE-AGENT READY", 20).await;

    let err = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: "tab-does-not-exist".to_string(),
            },
            "intruding-lease",
        )
        .await
        .expect_err("attaching a tab must fail while no tabs exist");
    assert!(
        err.to_string().contains("tab-does-not-exist"),
        "the error must name the tab that could not be found, got: {err:#}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a tab attach must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound,
        "a terminal that does not exist is a not-found, not a bad request or a server fault"
    );

    // The incumbent is untouched: no detach notice, and still typing.
    let detached = drain_for(&mut holder_rx, &mut holder_seen, Duration::from_secs(1)).await;
    assert_eq!(
        detached, None,
        "a refused tab attach detached the session's live attachment"
    );
    h.client
        .send_input(holder_chan, b"survived-the-tab\r".to_vec())
        .await;
    wait_for(&mut holder_rx, &mut holder_seen, "survived-the-tab", 15).await;
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
            // Vocabulary only for now: this test predates tabs/leases
            // (PLAN_M4.md step 4) and is only exercising the channel-0/
            // channel-reuse rejection paths, so the agent terminal with
            // no lease — today's only meaning — is exactly what belongs
            // here.
            terminal: TerminalSelector::default(),
            lease: String::new(),
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
            terminal: TerminalSelector::default(),
            lease: String::new(),
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
            terminal: TerminalSelector::default(),
            lease: String::new(),
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

/// An unknown control-message tag tears down the whole connection — the
/// loop-level half of the contract whose parse-layer half lives in the
/// proto crate (`unknown_control_message_tag_fails_decode`). This is the
/// behavior that forced PLAN_M2_5.md's `PROTOCOL_VERSION` bump to 4: new
/// `ControlMsg` variants are not additive, so a peer speaking a newer
/// message set must be kept out by the version handshake, because once
/// past it a single unknown message kills the connection. Pinning the
/// teardown here means a later refactor that catches and swallows the
/// parse error inside the connection loop — silently converting
/// "connection-fatal" into "ignored", and with it invalidating the whole
/// version-bump rationale — fails a test instead of going unnoticed.
#[tokio::test]
async fn unknown_control_message_tears_down_the_connection() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};

    let h = harness().await;
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
        .write_frame(&Frame {
            kind: FrameKind::Control,
            channel: 0,
            body: br#"{"type":"message_from_the_future"}"#.to_vec(),
        })
        .await
        .unwrap();

    // The connection must die: the reader sees EOF or an error, never a
    // reply, and never a silently-continuing session. A tolerant loop
    // would leave the stream open and this read hanging, so the timeout
    // is the failure detector for that regression.
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            match reader.read_frame().await {
                Ok(Some(_)) => continue, // drain any in-flight frame
                Ok(None) => break,       // clean shutdown: connection torn down
                Err(_) => break,         // error shutdown: equally torn down
            }
        }
    })
    .await;
    outcome.expect("connection must be torn down after an unknown control message, not left open");
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
    assert!(h.client.list_sessions().await.unwrap().sessions.is_empty());

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction reading the same state dir");
    let client2 = connect_client(&sup2).await;
    assert!(
        client2.list_sessions().await.unwrap().sessions.is_empty(),
        "a rejected create must not have persisted a row visible to a fresh supervisor"
    );
}

/// A relative cwd must be refused at create time, not merely mis-resolved
/// later.
///
/// tmux resolves a relative working directory against the SUPERVISOR
/// DAEMON's own cwd, not the client's — so accepting one here would store
/// a path whose meaning depends on wherever the daemon happened to be
/// started, and would shift again on every daemon restart (manually
/// reproduced: a session created this way either fails to restart with
/// "working directory does not exist", or — if a same-named directory
/// happens to exist relative to the daemon's new cwd — silently
/// relaunches the agent in the wrong directory). Refusing it up front in
/// `ensure_cwd_usable`, shared by create and restart, closes the create
/// path and also makes a pre-existing stored relative cwd refuse to
/// restart with a clear error instead of mis-resolving.
#[tokio::test]
async fn create_with_relative_cwd_is_rejected() {
    let h = harness().await;
    let err = h
        .client
        .create_session("crates", "true", None, 80, 24)
        .await
        .expect_err("create should reject a relative cwd");
    let message = err.to_string();
    assert!(
        message.contains("crates") && message.contains("absolute"),
        "the refusal must name the offending path and explain the absoluteness requirement: {message}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a relative-cwd failure must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "a relative cwd is the caller's mistake, not a server fault"
    );
    assert!(
        h.client.list_sessions().await.unwrap().sessions.is_empty(),
        "a rejected create must not have created a session (in-memory or tmux)"
    );
    // The in-memory check above only proves the supervisor's own bookkeeping
    // saw nothing; a rejected create could in principle still have raced a
    // tmux `new-session` before validation ran. Probe the private socket
    // directly. No prior test in this harness created a tmux session, so
    // the server may not even be running yet — that absence itself proves
    // there is no session, the same shape `kill_tmux_server_and_wait` above
    // relies on.
    let probe = tmux_query(&h.state.path().join("tmux.sock"), &["list-sessions"]).await;
    if probe.status.success() {
        assert!(
            String::from_utf8_lossy(&probe.stdout).trim().is_empty(),
            "a rejected create must not have left a tmux session behind"
        );
    } else {
        assert!(
            String::from_utf8_lossy(&probe.stderr).contains("no server running"),
            "tmux list-sessions failed for a reason other than an absent server: {}",
            String::from_utf8_lossy(&probe.stderr)
        );
    }
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
    assert!(h.client.list_sessions().await.unwrap().sessions.is_empty());
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
    assert!(h.client.list_sessions().await.unwrap().sessions.is_empty());
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
    assert!(h.client.list_sessions().await.unwrap().sessions.is_empty());
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
        client.list_sessions().await.unwrap().sessions.is_empty(),
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
    h.client.send_input(chan, b"spam 80\r".to_vec()).await;
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

    h.client.resize(&session.id, chan, 100, 30).await;
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
        .send_input(channel, format!("{payload}\r").into_bytes())
        .await;
    wait_for(&mut first, &mut initial, &payload, 10).await;
    h.client.detach(channel).await;

    let (channel, mut second) = h
        .client
        .attach(&session.id, 40, 24)
        .await
        .expect("reattach");
    let mut replay = Vec::new();
    h.client
        .send_input(channel, b"geometry-barrier\r".to_vec())
        .await;
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
    winner.resize(&session.id, winner_chan, 100, 30).await;
    wait_for_geometry(&h, "100x30").await;

    h.client.resize(&session.id, loser_chan, 111, 33).await;
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

    h.client.resize(&session.id, live_chan, 100, 30).await;
    wait_for_geometry(&h, "100x30").await;

    h.client.resize(&session.id, stale_chan, 111, 33).await;
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
    h.client.send_input(chan, b"quit\r".to_vec()).await;

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

/// The adopted-server gap: tmux reads a `-f` config only when it STARTS a
/// server, so `ensure_server`'s adopt-a-surviving-server path (the
/// ordinary case across a supervisor restart or upgrade) never rereads
/// `TmuxDriver::config_body` at all — which is exactly why `focus-events`
/// is not in that config in the first place, and is instead reconciled by
/// an explicit, unconditional `set-option` every time `ensure_server` runs
/// (see that call's own doc for the full rationale, including what this
/// option does and does not actually change for us). A test that only
/// ever hits the fresh-start path would keep passing even if that
/// explicit reconciliation silently regressed back to "rely on the config
/// file", because fresh starts read the config regardless. This test
/// provokes adoption specifically: a server is started by hand, on this
/// state dir's socket, with focus-events deliberately off — standing in
/// for a survived server an upgraded supervisor binary reattaches to,
/// whose config predates this option (or simply had it off) — and only
/// THEN does a `Supervisor` get constructed against the same socket,
/// which `ensure_server` must adopt rather than start fresh.
///
/// `focus-events` is a SERVER option (`set -s`), so the live query below
/// uses `show-options -s` to match — a `-g` query would rely on tmux's
/// scope inference rather than pinning the same table the fix itself
/// names explicitly.
#[tokio::test]
async fn adopted_tmux_server_gets_focus_events_explicitly_not_just_from_config() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("state");
    let sock = state.path().join("tmux.sock");
    let _tmux = TmuxServerGuard(sock.clone());

    // Hand-roll a server on this socket BEFORE any farhelm code touches
    // this state dir, deliberately with the option off — this is the
    // "survived server" half of the adoption gap, so it must exist first.
    // `start-server` alone (rather than `new-session`) is enough to leave
    // a live, queryable server: `exit-empty off` keeps it up with no
    // sessions, so there is no need to spawn a pointless shell just to
    // give it something to hold open.
    let off_conf = state.path().join("pre-existing.conf");
    tokio::fs::write(
        &off_conf,
        "set -s exit-empty off\nset -s focus-events off\n",
    )
    .await
    .expect("write throwaway pre-existing config");
    let started = tokio::process::Command::new("tmux")
        .arg("-S")
        .arg(&sock)
        .arg("-f")
        .arg(&off_conf)
        .arg("start-server")
        .status()
        .await
        .expect("spawn scratch tmux");
    assert!(started.success(), "test setup: scratch tmux must start");

    // Now let the real code run: `ensure_server` (via `Supervisor::new_with_exe`)
    // finds this socket already live and must ADOPT it, not start a fresh
    // server whose config it would otherwise get to read.
    Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor construction must adopt the pre-existing server");

    let out = tmux_query(&sock, &["show-options", "-s", "focus-events"]).await;
    assert!(
        out.status.success(),
        "show-options -s focus-events failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "focus-events on",
        "adopting a pre-existing server must still bring focus-events on, not just \
         a fresh server's config"
    );
}

/// PLAN_M2.md's list-status contract: once an agent exits ON ITS OWN — no
/// stop or delete involved — the next `ListSessions` must reflect that as
/// `Exited` with the exact exit code tmux observed, not stay `Alive`
/// forever. `exited_agent_leaves_a_viewable_terminal` already proves the
/// terminal itself survives; this proves the status field tracks the same
/// event. The basic fake agent's own `quit` path exits 0, which is what
/// makes this an easy code to pin exactly (unlike a signal death).
#[tokio::test]
async fn exited_agent_lists_as_exited_with_its_exit_code() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, b"quit\r".to_vec()).await;

    // Exit-code precision, version-gated: see `wait_for_exit_code`.
    wait_for_exit_code(&h.client, &session.id, 0, 30).await;
}

/// A nonzero exit is reported precisely, not just "not alive" — the whole
/// point of carrying `exit_code` through instead of a boolean liveness
/// flag. A plain shell exit needs no fake-agent script at all: its code
/// is exactly what tmux's `#{pane_dead_status}` reports.
///
/// The half-second sleep before the exit is load-bearing, not padding: a
/// pane whose process dies while tmux is still setting the pane up can
/// lose the recorded exit status entirely (observed on loaded CI runners
/// as a permanent `Exited { exit_code: None }`; never reproduced locally,
/// where `exit 3` alone always raced in tmux's favor). An agent that
/// exits before its terminal even finishes materializing is not the
/// behavior this test pins, so the fixture deliberately outlives pane
/// setup instead.
#[tokio::test]
async fn nonzero_exit_lists_with_its_precise_code() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            "sh -c 'sleep 0.5; exit 3'",
            None,
            80,
            24,
        )
        .await
        .expect("create");

    // Exit-code precision, version-gated: see `wait_for_exit_code`.
    wait_for_exit_code(&h.client, &session.id, 3, 30).await;
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

    assert!(h.client.list_sessions().await.unwrap().sessions.is_empty());
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
            "session_id": format!("test-{name}"),
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
    h.client.send_input(c1, b"ghost-input\r".to_vec()).await;
    h.client.send_input(c2, b"marker-input\r".to_vec()).await;
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
    h.client
        .send_input(loser_chan, b"ghost-xconn\r".to_vec())
        .await;
    h.client
        .list_sessions()
        .await
        .expect("barrier after kicked input");
    winner
        .send_input(winner_chan, b"marker-xconn\r".to_vec())
        .await;
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
    h.client
        .send_input(chan, b"alive-after-loss\r".to_vec())
        .await;
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
///
/// M2.5 bounded the writer queue, which changed how this same peer
/// misbehaves and made the test's original shape unable to reach its own
/// half-close: once every admission permit is held by a handler parked on
/// a full queue, `handle_control` blocks the read loop too, so the flood
/// below backs up into the request direction. `WRITER_STALL_TIMEOUT` is
/// what breaks that — shortened here, since the production value is a
/// minute — and the request count is now sized to fit comfortably inside
/// the transport buffer either way, so the half-close is reachable
/// whether or not the read loop is still draining when it happens.
#[tokio::test]
async fn writer_never_reading_peer_does_not_hang_connection_shutdown() {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};

    let h = harness_with_timeouts(SupervisorTimeouts {
        writer_stall: Duration::from_secs(2),
        ..SupervisorTimeouts::default()
    })
    .await;

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

    // Enough cheap requests to fill the reply direction several times
    // over and leave a real backlog queued behind the parked writer, but
    // few enough (a `ListSessions` request is a few dozen bytes) that
    // they all fit in this 4 KiB transport buffer on their own. That
    // second property is what keeps the flood from blocking the TEST:
    // since M2.5's bounded writer queue, the supervisor's read loop can
    // itself stall behind a peer that never reads, so this must not
    // depend on the read loop keeping up.
    for req_id in 0..64u64 {
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
    // Named the way a real launch names its files — per session AND
    // generation (`launch::spec_path_for_launch`) — because the sweep
    // recognizes its own naming and leaves anything else alone.
    let orphan = spec_path_for_launch(state.path(), "orphan", 0);
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
    client
        .send_input(chan, b"through-the-proxy\r".to_vec())
        .await;
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
    h.client.send_input(chan, input).await;

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
            &[],
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
    assert_eq!(
        session.status,
        SessionStatus::Unknown,
        "SessionCreated's own reply must carry the create-time placeholder, not a fabricated \
         Alive — creation establishes only that the session and terminal exist, not that the \
         agent's exec succeeded (see ControlMsg::SessionCreated's own docs)"
    );

    let listed = h.client.list_sessions().await.expect("list");
    assert_eq!(
        listed.sessions,
        vec![with_status(session.clone(), SessionStatus::Alive)],
        "a session that has never been touched must list Alive once ListSessions computes \
         the real answer from tmux — even though the create-time reply itself said Unknown"
    );
    assert_eq!(listed.sessions[0].invocation, invocation);
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
            .sessions
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
    assert_eq!(
        listed.sessions,
        vec![with_status(
            session,
            SessionStatus::Exited { exit_code: None }
        )],
        "the stored tmux_name no longer resolves to a live pane, so this must list as \
         exited rather than fabricating liveness — the same honesty rule as the restart gap"
    );
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
    winner
        .send_input(winner_chan, b"survived-foreign-detach\r".to_vec())
        .await;
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
    assert!(listed.sessions.iter().any(|s| s.id == session.id));
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
    h.client
        .send_input(chan, b"a\x7fb\x1b[A\x03".to_vec())
        .await;
    // A plain printable byte with no special meaning to tmux or a
    // raw-mode pty, sent as a separate call. Its own hex line is the sync
    // point that proves the control-byte input above already made it
    // through, without depending on how `hexecho`'s read() calls happen
    // to chunk the payload into lines.
    h.client.send_input(chan, b"z".to_vec()).await;
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
        listed.sessions,
        vec![with_status(session.clone(), SessionStatus::Alive)],
        "session metadata must round-trip identically from SQLite, and a session whose \
         tmux server survived the restart must still list Alive"
    );

    let (chan, mut rx) = client2
        .attach(&session.id, 80, 24)
        .await
        .expect("attach must succeed: the tmux session is still alive");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    client2.send_input(chan, b"still-alive\r".to_vec()).await;
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
        listed.sessions,
        vec![with_status(
            session.clone(),
            SessionStatus::Exited { exit_code: None }
        )],
        "a session must stay listed even once its tmux server is gone — vanishing is \
         exactly what this PR exists to prevent — and the restart-gap entry (no terminal \
         at all) must list as exited with no exit code to fabricate, PLAN_M2.md's \
         restart-gap status contract"
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

/// A dead private tmux server must not take the whole session list down —
/// `TmuxDriver::pane_states`'s "no server running" tolerance, exercised
/// against a STILL-LIVE supervisor (no reconstruction, no restart-gap
/// reload, unlike the sibling `restart_gap_*` test above). `session` here
/// is tracked with a live `terminal: Some(..)` in this process's own map,
/// so this is exactly the case real-stack dogfooding found: the private
/// tmux server dying (crash, OOM, an operator killing it) while the
/// supervisor keeps running.
///
/// This PINS THE OPPOSITE of what this test asserted before this change:
/// an earlier version required `list_sessions` to fail here, reasoning
/// that reporting every tracked session `Exited` off a "fabricated" empty
/// pane-states map would be indistinguishable from an honestly observed
/// mass exit. That conflated two different things. An empty pane-states
/// MAP is not an empty session LISTING — `pane_states`'s return value
/// plays no part in WHICH rows `ListSessions` selects for its reply (the
/// session cap and byte budget decide that, independent of tmux
/// entirely); the map only ever feeds `session_status`'s per-entry
/// liveness lookup for whichever rows that selection already kept. And
/// `"no server running"` is not a guess: it is tmux's own DEFINITIVE
/// statement that no pane exists anywhere on this socket, so reporting
/// every terminal-bearing entry as gone is accurate reporting, not
/// fabrication — the same honest `Exited { exit_code: None }` a
/// restart-gap row already gets. The old behavior instead turned a dead
/// tmux server into a hard `ListSessions` failure: every session
/// unreachable THROUGH THE UI (which has no session ids left to act on,
/// including for delete, once the list that would supply them fails to
/// load) even though every one of them was intact in SQLite and
/// `DeleteSession`'s own handler was never itself refused. `TmuxDriver::
/// pane_states`'s own docs carry the full version of this reasoning.
///
/// The connection must also stay usable afterward: proven here by a
/// SECOND, genuinely different request (creating a fresh session)
/// succeeding right after the first request observed the dead server — a
/// repeat of the identical `list_sessions` call would only prove that one
/// request shape still works, not that the connection generally still
/// serves.
///
/// This does NOT attempt to restart or resurrect the vanished tmux server
/// — recovery is M3 (PLAN.md). Until then the session simply reports
/// `Exited`: a plain supervisor restart would reload its row
/// terminal-less (the ordinary restart-gap case), still `Exited`, not
/// "recovered" — there is no plain-restart path back to `Alive` for a
/// session whose tmux is actually gone.
#[tokio::test]
async fn list_sessions_survives_when_the_tmux_server_is_gone() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;

    let sock = h.state.path().join("tmux.sock");
    kill_tmux_server_and_wait(&sock).await;

    let expected = vec![with_status(
        session,
        SessionStatus::Exited { exit_code: None },
    )];
    let listed = h
        .client
        .list_sessions()
        .await
        .expect("list_sessions must succeed even once the private tmux server is gone");
    assert_eq!(
        listed.sessions, expected,
        "a session tracked with a live terminal must still be listed — never dropped — and \
         must report the same honest 'terminal gone' status a restart-gap row gets, since a \
         vanished tmux server makes that a definitive fact rather than a guess"
    );

    h.client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect(
            "the connection must stay usable for an unrelated request after one ListSessions \
             observed the dead tmux server",
        );
}

// No sibling test provokes a NON-"no server running" `pane_states`
// failure at this end-to-end layer: every other tmux failure this
// module's own tolerance list (and the one above it) does not cover
// would need something this harness has no honest way to arrange — a
// malformed or corrupted tmux invocation, a permissions failure on the
// socket, a tmux binary that emits an unrecognized diagnostic — without
// resorting to fault injection this test suite does not otherwise use
// (mocking or wrapping the tmux binary itself). Rather than invent a fake
// seam for it, that classification is pinned at the unit level instead:
// see `farhelm-supervisor`'s `tmux.rs`,
// `is_tolerated_list_panes_diagnostic_pins_all_three_tolerated_cases`
// (plus its path-embedding sibling), which exercise every diagnostic
// outcome directly against constructed stderr: the three tolerated ones
// (a running-but-empty server, an absent server, and a server caught
// mid-teardown) and the unclassified failure that must still propagate.

/// `session_status`'s pane-identity contract (`service.rs`): pane ids
/// reset to `%0` on a FRESH tmux server (verified empirically — killing
/// the server and creating a new session hands its first pane `%0`
/// again), so a stale, never-reloaded `SessionEntry` whose OLD pane
/// happened to be `%0` must not silently inherit a brand-new, unrelated
/// session's liveness just because the two share that recycled number.
///
/// Deliberately NOT the restart-gap case (the `restart_gap_*` tests):
/// the whole tmux server is killed and a SECOND session created on this
/// SAME live process, without ever reconstructing the `Supervisor`
/// (which would instead reload `terminal: None` for the dead row via
/// `has_session`). `old_session` is the very first session this harness
/// creates, so its pane is genuinely `%0`; killing the server and
/// creating `new_session` right after gives it the exact same number on
/// the freshly auto-started replacement server. Matching pane id alone
/// would let `old_session` read as `Alive` off of `new_session`'s real
/// liveness; `session_status`'s `session_name` cross-check
/// (`TmuxDriver::pane_states`'s `#{session_name}` field) is what tells
/// these two same-numbered panes apart.
#[tokio::test]
async fn stale_pane_id_after_server_restart_does_not_inherit_a_new_sessions_status() {
    let h = harness().await;
    let (old_session, _work1) = basic_session(&h).await;
    let sock = h.state.path().join("tmux.sock");
    let old_pane_id = pane_id_of(&sock, &format!("fh-{}", old_session.id)).await;

    kill_tmux_server_and_wait(&sock).await;

    // A brand-new session on the SAME live supervisor: tmux auto-starts a
    // fresh server for the socket (no `-N` flag anywhere in this
    // module — see `TmuxDriver::command`), whose pane-id counter starts
    // back at `%0`, the same number `old_session`'s terminal remembers.
    let (new_session, _work2) = basic_session(&h).await;
    let new_pane_id = pane_id_of(&sock, &format!("fh-{}", new_session.id)).await;
    assert_eq!(
        old_pane_id, new_pane_id,
        "test precondition: the old and new sessions must actually share the same recycled \
         pane id, or this test is not exercising the cross-check it claims to — if tmux's \
         pane-id-reset behavior ever changed, this assertion is what would catch it rather \
         than the test silently passing for an unrelated reason"
    );

    let listed = h.client.list_sessions().await.expect("list");
    let find = |id: &str| {
        listed
            .sessions
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .unwrap_or_else(|| panic!("session {id} missing from the list"))
    };
    assert_eq!(
        find(&old_session.id).status,
        SessionStatus::Exited { exit_code: None },
        "the old session's tmux is really gone; it must not inherit the new session's \
         liveness just because both happen to reuse pane %0"
    );
    assert_eq!(
        find(&new_session.id).status,
        SessionStatus::Alive,
        "the new session's own pane really is alive"
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
    listed.sessions.sort_by(|a, b| a.id.cmp(&b.id));
    let mut expected = vec![
        with_status(alive_session.clone(), SessionStatus::Alive),
        with_status(
            dead_session.clone(),
            SessionStatus::Exited { exit_code: None },
        ),
    ];
    expected.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(
        listed.sessions, expected,
        "both sessions must remain listed regardless of which one's terminal died, and \
         only the one whose tmux session actually died must list as exited"
    );

    let (chan, mut rx) = client2
        .attach(&alive_session.id, 80, 24)
        .await
        .expect("the untouched session must still attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    client2.send_input(chan, b"still-alive\r".to_vec()).await;
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

/// Pull the pid printed after `marker` (`"SELF-PID:"` or `"CHILD-PID:"`)
/// out of a fake-agent `spawner` transcript, panicking with the whole
/// transcript on any parse failure — a silent `0` or a wrong pid here
/// would make the process-tree-kill tests below pass or fail for reasons
/// unrelated to the code under test.
fn extract_pid(transcript: &[u8], marker: &str) -> u32 {
    let text = String::from_utf8_lossy(transcript);
    let after = text
        .find(marker)
        .unwrap_or_else(|| panic!("{marker} not found in transcript:\n{text}"))
        + marker.len();
    let digits: String = text[after..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("could not parse a pid after {marker} in:\n{text}"))
}

/// Whether `pid` is gone in the sense that matters for a tree-kill test:
/// no `/proc` entry at all, or a zombie (`Z`) still waiting on a parent
/// that may never call `wait()` on it (the fake-agent `spawner` script
/// deliberately never reaps its own child — see that script's docs).
/// Treating a zombie as "gone" is what keeps this test from depending on
/// exactly which ancestor ends up reaping an orphan once its own parent
/// dies to the same tree-kill.
fn process_is_gone(pid: u32) -> bool {
    let Ok(content) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return true;
    };
    let Some(after_comm) = content.rfind(')') else {
        return true;
    };
    content[after_comm + 1..]
        .split_whitespace()
        .next()
        .is_none_or(|state| state == "Z")
}

/// Poll until `pid` is gone (see `process_is_gone`), failing the test if
/// it never is. `kill_process_tree`'s SIGTERM/grace/SIGSTOP/SIGKILL
/// sequence is not instantaneous, so this is the only honest way to
/// observe its completion.
async fn wait_until_pid_gone(pid: u32, secs: u64) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        if process_is_gone(pid) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "pid {pid} was still alive after {secs}s"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Every currently-live pid whose parent is `parent`, read directly from
/// `/proc`. Used to discover a grandchild the fake-agent `spawner`
/// scripts never print themselves (only `SELF-PID`/`CHILD-PID`) — `sh -c
/// 'sleep 3600'` genuinely forks (verified empirically; see that
/// script's docs), so the process tree these tests kill is really three
/// levels deep, and a test that only checks the two printed pids cannot
/// tell "killed the tree" apart from "killed everything but the leaf".
fn children_of(parent: u32) -> Vec<u32> {
    let mut children = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return children;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        let Some(after_comm) = content.rfind(')') else {
            continue;
        };
        let Some(ppid) = content[after_comm + 1..]
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if ppid == parent {
            children.push(pid);
        }
    }
    children
}

/// Poll until `parent` has forked at least one child, returning its pid.
/// `spawner`'s child process is created via `std::process::Command::spawn`
/// (backed by `posix_spawn`), which returns once process creation has
/// SUCCEEDED — but that says nothing about how far the new process itself
/// has gotten. The race this closes is the child shell not yet having
/// forked its OWN `sleep` grandchild by the time we look, not anything
/// about when `spawn()` itself returns; this is the bounded wait for that,
/// rather than assuming the grandchild already exists the instant
/// `FAKE-AGENT READY` is seen.
async fn wait_for_child(parent: u32, secs: u64) -> u32 {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(&pid) = children_of(parent).first() {
            return pid;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "pid {parent} never forked a child within {secs}s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Poll until `path` exists, failing the test if it never does. Used
/// wherever a fake-agent script's own readiness cannot be observed
/// through the terminal (its child's stdio is disconnected — see
/// `spawner-stubborn`'s docs) and instead signals by creating a file in
/// the session's working directory.
async fn wait_for_file(path: &std::path::Path, secs: u64) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        if path.exists() {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{} never appeared within {secs}s",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Poll `path` until it contains a parseable pid, returning it. Used to
/// read the reparenting daemon's own self-reported pid out of
/// `reparented.pid` — see `spawner-reparent`'s docs for why a file is the
/// only way to learn it at all.
async fn wait_for_pid_file(path: &std::path::Path, secs: u64) -> u32 {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        if let Ok(contents) = std::fs::read_to_string(path)
            && let Ok(pid) = contents.trim().parse::<u32>()
        {
            return pid;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{} never contained a parseable pid within {secs}s",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The acceptance test for process-tree stop (PLAN_M2.md step 4 / M2
/// acceptance criterion 2): stopping a session must kill not just the
/// agent but every descendant it spawned, three levels deep — the
/// spawner process itself, the `sh` it forks, and the `sleep` `sh` forks
/// in turn. The `spawner` fixture exists exactly for this — a plain
/// script has nothing whose death would prove tree-kill rather than
/// single-process kill.
///
/// Also covers stop's other headline properties in the same run: the
/// session stays listed (both through this process's own client AND a
/// FRESH `Supervisor` on the same state dir, which is what actually
/// proves the DB row survived rather than merely this process's
/// in-memory map), and a fresh attach still works and replays the
/// pre-stop scrollback — stop leaves the terminal viewable, it does not
/// tear anything down.
#[tokio::test]
async fn stop_kills_the_whole_process_tree() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let child_pid = extract_pid(&seen, "CHILD-PID:");
    assert_ne!(
        self_pid, child_pid,
        "test fixture must report two distinct pids"
    );
    let grandchild_pid = wait_for_child(child_pid, 10).await;

    h.client.stop_session(&session.id).await.expect("stop");

    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(child_pid, 15).await;
    wait_until_pid_gone(grandchild_pid, 15).await;

    // Status is computed fresh from tmux at list time, not pushed, so the
    // pane's `pane_dead` flag flipping and this list call race each
    // other — this polls for the EVENTUAL exited classification rather
    // than asserting on a single read that might land before the flip.
    // What is under test is only that the session ends up classified
    // `Exited`, not which exact code it carries: PLAN_M2.md's status test
    // list, item (e), says "assert exited, don't over-pin the code" —
    // a SIGKILL death's `pane_dead_status` is not pinned to one value
    // across tmux versions, so the code is deliberately left unasserted.
    let found = wait_for_non_alive_status(&h.client, &session.id, 15).await;
    assert_eq!(found.id, session.id);
    assert_eq!(found.title, session.title);
    assert_eq!(found.cwd, session.cwd);
    assert_eq!(found.invocation, session.invocation);
    assert!(
        matches!(found.status, SessionStatus::Exited { .. }),
        "a stopped session must list as exited, got {:?}",
        found.status
    );

    // A fresh Supervisor on the same state dir is what actually proves
    // the row survived in SQLite, not just this process's own map — the
    // same reasoning `persisted_sessions_survive_a_supervisor_restart`
    // applies to create.
    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction on the same state dir");
    let client2 = connect_client(&sup2).await;
    let listed2 = client2
        .list_sessions()
        .await
        .expect("list from fresh supervisor");
    assert_eq!(
        listed2.sessions.len(),
        1,
        "a stopped session's row must survive a supervisor restart"
    );
    assert_eq!(listed2.sessions[0].id, session.id);
    assert!(
        matches!(listed2.sessions[0].status, SessionStatus::Exited { .. }),
        "the row's session is still dead after the restart too, got {:?}",
        listed2.sessions[0].status
    );

    h.client.detach(chan).await;
    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("a stopped session's terminal must still be attachable");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "SELF-PID", 15).await;
}

/// The SIGKILL half of `kill_process_tree`'s sequence: a child that traps
/// and discards SIGTERM must still die, because the sweep escalates to
/// SIGSTOP-quiesce and then SIGKILL rather than giving up once SIGTERM
/// alone fails. The `spawner-stubborn` fixture's child would survive
/// forever under a SIGTERM-only kill, so its death here is what pins the
/// escalation actually runs, not just that SIGTERM is sent.
///
/// Waits for `stubborn-ready` (written by the child itself, AFTER
/// installing the trap) before stopping — without that wait, a stop
/// racing the child's own startup could catch it before `trap ''` has
/// run, and SIGTERM would kill it the ordinary way, silently defeating
/// the point of this test.
#[tokio::test]
async fn stop_kills_a_child_that_ignores_sigterm() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-stubborn"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let child_pid = extract_pid(&seen, "CHILD-PID:");
    wait_for_file(&work.path().join("stubborn-ready"), 10).await;

    h.client.stop_session(&session.id).await.expect("stop");

    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(child_pid, 15).await;
}

/// The whole point of spawning `ListSessions`/`StopSession`/
/// `DeleteSession` (`service.rs`'s `handle_control`, per those arms' own
/// comments): a slow one in flight must not stall a cheap, unrelated
/// request on the SAME connection behind it. `stop_session` against a
/// `spawner` session is the slow one here — `kill_process_tree`'s grace
/// period alone is half a second, before quiesce and kill-confirmation
/// even start — and an unknown-session `attach` is about as cheap as a
/// request gets: one lock-guarded map lookup, no tmux call at all.
///
/// Reverting the handlers to plain inline `await`s would fail this: the
/// connection's single serial read loop would not even read the attach
/// request's frame off the wire until the stop request ahead of it had
/// been handled to completion, let alone reply to it first.
#[tokio::test]
async fn cheap_request_completes_before_a_slow_spawned_handler_in_flight() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // Kick off the slow stop without awaiting it yet.
    let stop_client = Arc::clone(&h.client);
    let stop_session_id = session.id.clone();
    let stop_done = Arc::new(AtomicBool::new(false));
    let stop_done_writer = Arc::clone(&stop_done);
    let stop_task = tokio::spawn(async move {
        stop_client
            .stop_session(&stop_session_id)
            .await
            .expect("stop");
        stop_done_writer.store(true, Ordering::SeqCst);
    });

    // Give the stop request time to actually be dispatched and its kill
    // sweep started (well inside its 500ms grace period) before firing
    // the cheap request — otherwise this could race the connection's own
    // read loop picking up the stop frame at all, rather than exercising
    // the "already in flight" scenario this test is about.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        !stop_done.load(Ordering::SeqCst),
        "test setup: the slow stop must still be in flight at this point"
    );

    let cheap_result = h.client.attach("definitely-not-a-session", 80, 24).await;
    assert!(
        cheap_result.is_err(),
        "an unknown-session attach must still fail fast"
    );
    assert!(
        !stop_done.load(Ordering::SeqCst),
        "the cheap request must complete WHILE the slow stop is still in flight"
    );

    stop_task.await.expect("stop task panicked");
}

/// Stop must be idempotent both in the ordinary sense (calling it twice on
/// a live session) and across the restart gap (a session whose terminal
/// never came back has nothing running, so "make sure nothing is running"
/// already holds).
#[tokio::test]
async fn stop_is_idempotent() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    h.client
        .stop_session(&session.id)
        .await
        .expect("first stop");
    h.client
        .stop_session(&session.id)
        .await
        .expect("second stop on an already-stopped session must also succeed");

    // A restart-gap (terminal-less) session, mirroring
    // `restart_gap_lists_sessions_without_a_terminal_and_attach_fails`.
    let (gap_session, _work2) = basic_session(&h).await;
    let gap_tmux_name = format!("fh-{}", gap_session.id);
    let sock = h.state.path().join("tmux.sock");
    let killed = tmux_query(&sock, &["kill-session", "-t", &gap_tmux_name]).await;
    assert!(
        killed.status.success(),
        "test setup: kill-session must succeed, got: {}",
        String::from_utf8_lossy(&killed.stderr)
    );
    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction after one session's tmux died");
    let client2 = connect_client(&sup2).await;
    client2
        .stop_session(&gap_session.id)
        .await
        .expect("stopping a terminal-less session must succeed: nothing can be running");
}

/// Unknown ids are the one failure mode stop and delete share, and both
/// must report it the same way `Attach` does.
#[tokio::test]
async fn stop_unknown_session_is_not_found() {
    let h = harness().await;
    let err = h
        .client
        .stop_session("does-not-exist")
        .await
        .expect_err("stop of an unknown session must fail");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound
    );
}

/// See `stop_unknown_session_is_not_found`.
#[tokio::test]
async fn delete_unknown_session_is_not_found() {
    let h = harness().await;
    let err = h
        .client
        .delete_session("does-not-exist")
        .await
        .expect_err("delete of an unknown session must fail");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound
    );
}

/// Delete must remove all three of a session's traces: the in-memory
/// entry, the tmux session backing its terminal, and the SQLite row —
/// the last checked through a SECOND, independent `Supervisor` on the same
/// state dir, exactly like `create_in_missing_directory_errors` does for
/// creation, since only that proves the row is really gone rather than
/// merely absent from this one process's map.
#[tokio::test]
async fn delete_removes_session_terminal_and_row() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let tmux_name = format!("fh-{}", session.id);

    h.client.delete_session(&session.id).await.expect("delete");

    assert!(
        h.client.list_sessions().await.unwrap().sessions.is_empty(),
        "a deleted session must not stay listed"
    );

    let sock = h.state.path().join("tmux.sock");
    let out = tmux_query(&sock, &["has-session", "-t", &format!("={tmux_name}")]).await;
    assert!(
        !out.status.success(),
        "the tmux session backing a deleted session's terminal must be gone"
    );

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction on the same state dir");
    let client2 = connect_client(&sup2).await;
    assert!(
        client2.list_sessions().await.unwrap().sessions.is_empty(),
        "the row must really be gone, not just absent from the original process's map"
    );
}

/// Deleting a session out from under an attached client must detach it
/// with an explicit notice rather than leaving its stream hanging —
/// mirroring how `second_attach_detaches_first` asserts a takeover's
/// `Detached` event.
#[tokio::test]
async fn delete_while_attached_detaches_the_client() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    let deleter = h.second_client().await;
    deleter
        .delete_session(&session.id)
        .await
        .expect("delete from a second client");

    let detached = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(ev) = rx.recv().await {
            if let TermEvent::Detached(reason) = ev {
                return reason;
            }
        }
        panic!("attached client's stream ended without a Detached event");
    })
    .await
    .expect("timed out waiting for Detached after delete");
    assert!(
        detached.contains("deleted"),
        "detach reason should say the session was deleted, got: {detached}"
    );
}

/// Delete must work on a restart-gap (terminal-less) session too — SPEC.md
/// promises delete "in any state" — mirroring the restart-gap setup in
/// `restart_gap_lists_sessions_without_a_terminal_and_attach_fails`.
#[tokio::test]
async fn delete_works_on_a_terminal_less_session() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let sock = h.state.path().join("tmux.sock");
    kill_tmux_server_and_wait(&sock).await;

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction after the tmux server died");
    let client2 = connect_client(&sup2).await;

    client2
        .delete_session(&session.id)
        .await
        .expect("delete on a terminal-less session must succeed");
    assert!(
        client2.list_sessions().await.unwrap().sessions.is_empty(),
        "a deleted session must not stay listed"
    );
}

/// Delete's process-tree reaping is the same `kill_process_tree` stop
/// uses, but exercised on its own path (delete's handler, not stop's) and
/// down to the same three-level chain — every discovered descendant
/// (agent, its `sh` child, that child's own `sleep`) must actually be
/// gone once delete returns, not merely the tmux session removed around
/// them.
#[tokio::test]
async fn delete_kills_the_whole_process_tree() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let child_pid = extract_pid(&seen, "CHILD-PID:");
    let grandchild_pid = wait_for_child(child_pid, 10).await;

    h.client.delete_session(&session.id).await.expect("delete");

    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(child_pid, 15).await;
    wait_until_pid_gone(grandchild_pid, 15).await;
}

/// Stop must leave an existing attachment exactly as it was: no
/// unexpected `Detached`, and the attachment stays a normal, kickable one
/// — a second client attaching afterwards must produce the ordinary
/// takeover notice on the first, proving stop did not itself already
/// tear the attachment down or leave it in some half-detached state.
#[tokio::test]
async fn stop_does_not_disturb_the_existing_attachment() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (_chan1, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    h.client.stop_session(&session.id).await.expect("stop");

    // Give stop a moment, then require that nothing unexpected arrived on
    // the existing attachment: no Detached (stop must not touch it) and
    // no closed stream. Trailing pre-stop output racing the agent's death
    // is fine and not itself asserted on.
    match tokio::time::timeout(Duration::from_millis(500), rx1.recv()).await {
        Err(_) => {}                       // nothing arrived — expected
        Ok(Some(TermEvent::Data(_))) => {} // trailing pre-death output
        Ok(Some(TermEvent::Detached(reason))) => {
            panic!("stop must not detach the existing attachment: {reason}")
        }
        Ok(None) => panic!("attachment stream closed unexpectedly after stop"),
    }

    // The attachment must still be live and kickable: a second attach
    // takes it over exactly like `second_attach_detaches_first`.
    let (chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("second attach");
    let detached = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(ev) = rx1.recv().await {
            if let TermEvent::Detached(reason) = ev {
                return reason;
            }
        }
        panic!("first attachment stream ended without a takeover Detached");
    })
    .await
    .expect("timed out waiting for the takeover Detached");
    assert!(
        detached.contains("another client"),
        "takeover reason changed unexpectedly: {detached}"
    );
    // The second attachment is otherwise ordinary — same session, same
    // (now-dead) pane, still attachable.
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;
    h.client.detach(chan2).await;
}

/// Stop followed by delete on the same live session must both succeed:
/// delete's own pane query must see the (by-then-dead) pane and skip
/// straight to tmux teardown rather than erroring on an already-stopped
/// agent.
#[tokio::test]
async fn stop_then_delete_both_succeed() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    h.client.stop_session(&session.id).await.expect("stop");
    h.client
        .delete_session(&session.id)
        .await
        .expect("delete after stop must succeed");
    assert!(
        h.client.list_sessions().await.unwrap().sessions.is_empty(),
        "a deleted session must not stay listed"
    );
}

/// Stopping an alt-screen agent must not silently discard its last frame.
///
/// A real alt-screen agent (claude, chiefly) restores the primary screen
/// on its way out of a SIGTERM, and tmux never records alternate-screen
/// content in history at all — without a pre-kill snapshot, the app's
/// final frame is unreachable forever the instant the kill lands, and a
/// reattach shows only a blank primary screen plus tmux's "pane is dead"
/// text. This pins the fix end to end: stop an `altscreen` fake-agent
/// session, attach fresh, and require BOTH the app's own marker text and
/// the "last screen before stop" divider that only the snapshot path
/// produces — the divider alone would not prove the CONTENT survived, and
/// the marker alone (with no divider) would not prove it came from the
/// snapshot path rather than some other replay quirk.
#[tokio::test]
async fn stop_replays_the_alt_screen_snapshot() {
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

    h.client.stop_session(&session.id).await.expect("stop");

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach after stop");
    let mut replay = Vec::new();
    // The needle is the CONTENT marker, not the divider: `send_alt_screen
    // _snapshot` (service.rs) now streams the divider and the snapshot
    // content as separate frames (chunked, matching the ordinary prefill's
    // own frame-per-piece shape), and `wait_for` returns the instant its
    // needle appears in the accumulated bytes — waiting on the divider
    // text would risk returning right after that FIRST frame, before the
    // content frame(s) sent immediately after it have necessarily been
    // drained yet. Waiting on content that can only ever arrive in a
    // LATER frame guarantees every frame before it, divider included, is
    // already in `replay` by the time this returns.
    wait_for(&mut rx2, &mut replay, "ALT-SCREEN APP", 15).await;
    let text = String::from_utf8_lossy(&replay);
    let divider_at = text.find("last screen before stop");
    assert!(
        divider_at.is_some(),
        "the app's content must be preceded by the snapshot divider: {text}"
    );
    // Pins `-e` on the underlying `capture-pane`: the fixture draws its
    // banner in reverse video (`\x1b[7m`), and a capture taken WITHOUT
    // escape sequences would carry the plain text with none of its
    // styling. Without this check, a regression that dropped `-e` would
    // still pass every other assertion here (the text marker survives
    // either way).
    assert!(
        text.contains("\x1b[7m"),
        "the snapshot must preserve the fixture's reverse-video SGR sequence, proving \
         capture-pane ran with -e: {text:?}"
    );
    // Pins `sanitize_snapshot_lines` (tmux.rs): `capture-pane -e` emits no
    // attribute reset at a line's end, so a line that ends while a
    // background/inverse attribute is still active leaves it running —
    // a real terminal's scroll/line-feed handling then fills every cell
    // from there onward with that still-active background
    // (background-color-erase), producing a highlight band the real
    // `claude` never showed. Only the SNAPSHOT segment is asserted here;
    // full xterm.js cell-attribute verification belongs to the
    // Playwright suite at a later stack layer.
    //
    // The segment must start AFTER the divider's own trailing `\r\n`, not
    // at the divider's own text: service.rs's divider line
    // (`"\r\n\x1b[2m-- last screen before stop --\x1b[0m\r\n"`) already
    // contains its own literal `\x1b[0m\r\n` — slicing from the divider's
    // text onward would make this assertion pass on the divider's OWN
    // bytes regardless of whether `sanitize_snapshot_lines` ever ran,
    // which is exactly the vacuous check a from-day-one review of this
    // test caught. Reuses `divider_at` (found once above) rather than
    // scanning for the divider text a second time.
    let after_divider = &text[divider_at.expect("checked above")..];
    let (_divider_line, snapshot_segment) = after_divider
        .split_once("\r\n")
        .expect("the divider line itself always ends in its own \\r\\n (service.rs)");
    assert!(
        snapshot_segment.contains("\x1b[0m\r\n"),
        "the snapshot segment (excluding the divider's own trailing reset) must carry an SGR \
         reset immediately before at least one line terminator: {snapshot_segment:?}"
    );
}

/// `-N` coverage: a styled background painted with erase-to-end-of-line
/// (`\x1b[K`, no literal trailing space characters — see `altscreen`'s
/// `STATUS BAR` row) must survive into the stored snapshot. Verified
/// empirically (scratch tmux session, not through this test) that such a
/// row captures as ~19 bytes without `-N` (trimmed right after the label)
/// versus padded out to the full 80-column pane width with it — so a
/// length threshold comfortably between those two shapes is what
/// discriminates "removing -N" from "keeping it" without depending on the
/// EXACT escape-sequence bytes tmux happens to re-serialize, which is not
/// this test's business to pin.
#[tokio::test]
async fn stop_snapshot_preserves_trailing_styled_padding_via_capture_n() {
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
    wait_for(&mut rx, &mut seen, "STATUS BAR", 20).await;
    h.client.detach(chan).await;

    h.client.stop_session(&session.id).await.expect("stop");

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach after stop");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "STATUS BAR", 15).await;
    let text = String::from_utf8_lossy(&replay);

    // Isolate the status-bar ROW: from the label up to the fixture's own
    // trailing CRLF, so padding from LATER lines (or the divider itself)
    // cannot inflate this measurement.
    let start = text
        .find("STATUS BAR")
        .expect("status bar label must be present");
    let row = &text[start..];
    let row = &row[..row.find("\r\n").unwrap_or(row.len())];
    assert!(
        row.len() > 40,
        "the status-bar row must carry its trailing erase-to-end-of-line padding, proving \
         capture-pane ran with -N — got a {}-byte row: {row:?}",
        row.len()
    );
    // `sanitize_snapshot_lines` (tmux.rs) must have closed this row off
    // with its own SGR reset immediately before the `\r\n` this slice was
    // cut at, regardless of the `-N` padding's own attribute bytes. The
    // risk this guards against is a real terminal's scroll/line-feed
    // handling filling cells with THIS row's still-active background
    // (background-color-erase) on replay — not that background leaking
    // into the divider row that follows: this fixture's divider happens
    // to differ enough in styling that `capture-pane -e` reserializes an
    // explicit `\x1b[49m` background reset at the divider's own start
    // regardless of what this test does. That reserialization is a
    // property of THIS SPECIFIC fixture content, not a general guarantee
    // — see `sanitize_snapshot_lines`'s own "why a bare boundary reset is
    // not enough" docs (tmux.rs) for the general case, where a following
    // row's UNCHANGED style is never re-stated at all and would be lost
    // without this transform's own restore.
    assert!(
        row.ends_with("\x1b[0m"),
        "the captured-and-sanitized status-bar row must end with an SGR reset before its line \
         terminator: {row:?}"
    );
}

/// The third replay state alongside the other two alt-screen tests here:
/// alive (ordinary reattach), dead-and-restored-to-primary (the divider
/// case, `stop_replays_the_alt_screen_snapshot`), and this one —
/// dead-but-STILL-on-the-alternate-screen. SIGKILLs the pane's own
/// process directly, bypassing the supervisor's `stop` path (and so its
/// SIGTERM-based restore handler AND its stop-time snapshot capture)
/// entirely, so the pane dies without ever leaving the alternate screen
/// and without `StopSession` ever having run to capture anything.
///
/// Pins the negative case only: the divider must NOT be appended. The
/// `Attach` handler's gate is snapshot EXISTENCE now (file or pending
/// map), not the pane's alternate-screen state — and no snapshot exists
/// here at all, because `StopSession` (the only thing that ever creates
/// one) never ran; `send_alt_screen_snapshot` finds neither source and
/// returns before ever touching `modes.alternate_on`. It does NOT assert
/// the app's own content survives, because (verified empirically,
/// scratch tmux session, not through this codebase) it does not: tmux
/// replaces a pane's LIVE grid with its own
/// "Pane is dead" placeholder the moment the process backing it exits,
/// whether or not that pane was on the alternate screen — capturing an
/// alt-screen pane that died this way shows only that placeholder, same
/// as capturing its (nonexistent) history would. That total loss is
/// exactly the failure this whole feature exists to prevent, but ONLY
/// for stops that went through `StopSession`'s own capture-before-kill
/// path; a pane killed some other way (an externally-issued SIGKILL, as
/// here) was never going to have a snapshot to fall back on in the first
/// place, and this test's job is only to confirm that absence does not
/// somehow manifest as a stray divider.
#[tokio::test]
async fn dead_pane_still_on_alt_screen_replays_without_a_divider() {
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

    let tmux_name = format!("fh-{}", session.id);
    let sock = h.state.path().join("tmux.sock");
    let pid_out = tmux_query(
        &sock,
        &["display-message", "-p", "-t", &tmux_name, "#{pane_pid}"],
    )
    .await;
    let pane_pid = String::from_utf8_lossy(&pid_out.stdout).trim().to_string();
    let killed = tokio::process::Command::new("kill")
        .arg("-9")
        .arg(&pane_pid)
        .status()
        .await
        .expect("running kill(1)");
    assert!(
        killed.success(),
        "SIGKILL of the pane's own process must succeed"
    );

    // Wait for tmux to actually mark the pane dead before attaching, so
    // the attach cannot race a not-yet-updated `pane_dead` flag.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let out = tmux_query(
            &sock,
            &["display-message", "-p", "-t", &tmux_name, "#{pane_dead}"],
        )
        .await;
        if String::from_utf8_lossy(&out.stdout).trim() == "1" {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "pane never went dead after SIGKILL"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach to a dead-on-alt-screen pane");
    let mut replay = Vec::new();
    // What the replay of a dead-on-alt-screen pane contains is
    // environment-dependent in a way that burned this test twice: some
    // environments retain the app's last frame in the capture (the
    // placeholder arriving, if at all, only via live output after
    // attach), others substitute tmux's "Pane is dead" placeholder into
    // the capture itself — and which one a given tmux 3.4 produces has
    // varied even between this repo's CI and a local install of the same
    // version. The assertion this test exists for is the NEGATIVE below
    // (no snapshot divider), so the anchor deliberately accepts either
    // marker: both prove the replay delivered the dead pane's content.
    let anchor_deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let text = String::from_utf8_lossy(&replay);
        if text.contains("Pane is dead") || text.contains("ALT-SCREEN APP") {
            break;
        }
        let remaining = anchor_deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx2.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => replay.extend_from_slice(&bytes),
            Ok(other) => panic!("attachment ended before any dead-pane content: {other:?}"),
            Err(_) => panic!(
                "timed out waiting for dead-pane replay content; transcript so far:\n{}",
                String::from_utf8_lossy(&replay)
            ),
        }
    }
    // Settle-then-drain, same pattern as the other negative-divider
    // assertions in this file: absence needs a short observation window,
    // not a single immediate check.
    tokio::time::sleep(Duration::from_millis(300)).await;
    while let Ok(TermEvent::Data(bytes)) = rx2.try_recv() {
        replay.extend_from_slice(&bytes);
    }
    let text = String::from_utf8_lossy(&replay);
    assert!(
        !text.contains("last screen before stop"),
        "a pane that died still on the alternate screen (never having gone through StopSession) \
         must never gain a snapshot divider: {text}"
    );
}

/// The positive counterpart to `dead_pane_still_on_alt_screen_replays_
/// without_a_divider`: here `StopSession` DOES run (and DOES capture,
/// since the pane is alive and on the alternate screen when `stop` is
/// called), and its own `kill_process_tree` is what finally kills the
/// app — via the `altscreen-ignores-term` fixture, which never restores
/// the primary screen because it never runs any code on SIGTERM at all
/// (`SIG_IGN`), so `kill_process_tree` must escalate through its full
/// grace/SIGSTOP-quiesce/SIGKILL sequence before the pane actually dies.
/// This is the exact scenario the alt-screen snapshot feature exists
/// for, and the one the earlier `dead && !alternate_on` gate silently
/// blanked: a dead pane still on the alternate screen, with a REAL
/// snapshot on disk this time. Requires both the divider and the app's
/// own marker to replay, AND the alt-exit escape (`\x1b[?1049l`) to
/// precede the divider — landing the snapshot on the primary screen's
/// scrollback rather than inside the scrollback-less alternate buffer the
/// ordinary mode-replay just re-entered.
#[tokio::test]
async fn stop_replays_the_alt_screen_snapshot_when_the_agent_ignores_term_and_never_restores() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen-ignores-term"),
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

    // Runs to completion: `kill_process_tree`'s own escalation (grace,
    // SIGSTOP-quiesce, SIGKILL, confirm) is what actually kills a process
    // that ignores SIGTERM outright, so this call does not return until
    // that whole sequence has finished.
    h.client
        .stop_session(&session.id)
        .await
        .expect("stop must still succeed against a SIGTERM-ignoring alt-screen app");

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach after stop");
    let mut replay = Vec::new();
    // Ordered wait, anchored on the divider: this fixture died still ON
    // the alternate screen, and whether the dead pane's PREFILL already
    // contains the app's frame is tmux-version-dependent (see
    // `wait_for_after`'s docs) — a plain content wait can return before
    // the snapshot suffix ever arrives.
    wait_for_after(
        &mut rx2,
        &mut replay,
        "last screen before stop",
        "ALT-SCREEN APP",
        15,
    )
    .await;
    let text = String::from_utf8_lossy(&replay);
    let exit_alt_screen = text.find("\x1b[?1049l").expect(
        "the alt-exit escape must precede the snapshot, since the pane died still on \
                 the alternate screen",
    );
    let divider = text.find("last screen before stop").expect("checked above");
    assert!(
        exit_alt_screen < divider,
        "the alt-exit escape must land the snapshot on the primary screen's scrollback, so it \
         must precede the divider, not follow it: {text:?}"
    );
}

/// The gap `Supervisor::pending_snapshots` (service.rs) exists to close:
/// an `Attach` landing AFTER the pane has gone dead but BEFORE
/// `StopSession` has finished (`kill_process_tree` can take a real
/// fraction of a second against an uncooperative tree) must still see
/// the snapshot, served from the in-memory pending map rather than a file
/// that has not been written yet.
///
/// Uses `altscreen-stubborn-child`: this process's own pid restores the
/// primary screen and exits within milliseconds of SIGTERM (so the pane
/// goes dead almost immediately), while its spawned child ignores
/// SIGTERM and forces `kill_process_tree` through its full SIGSTOP-
/// quiesce-then-SIGKILL escalation — several hundred milliseconds beyond
/// `KILL_GRACE` (500ms, service.rs) alone. A fixed delay comfortably
/// inside that window is what this test uses to land the concurrent
/// attach; see the delay's own comment for the honest limits of that
/// approach (best-effort, not a deterministic barrier — REJECTED per the
/// review round that requested this test).
#[tokio::test]
async fn attach_mid_stop_sees_the_pending_alt_screen_snapshot() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen-stubborn-child"),
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

    let stopper = h.second_client().await;
    let stop_session_id = session.id.clone();
    let stop_task = tokio::spawn(async move { stopper.stop_session(&stop_session_id).await });

    // The fixture's own root process restores the primary screen and
    // exits within single-digit milliseconds of receiving SIGTERM, but
    // its stubborn child forces `kill_process_tree` through its full
    // escalation. 250ms is comfortably inside the resulting window on any
    // reasonable machine: long enough that the pane is certainly already
    // dead, short enough that `stop_session` is, with very high
    // confidence but not a hard guarantee, still in flight — a faster-
    // than-expected sweep would simply mean this attach lands after
    // publish instead, reading the same content back from the file
    // rather than the pending map. Either way the assertions below still
    // hold; only the CODE PATH exercised would differ, which is the
    // honest limit of a fixed-delay approach to a race this narrow.
    tokio::time::sleep(Duration::from_millis(250)).await;

    let (chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach mid-stop");
    let mut replay = Vec::new();
    // Wait on the CONTENT marker, not the divider — same reasoning as
    // `stop_replays_the_alt_screen_snapshot`'s identical comment: the
    // divider and the snapshot content are separate, sequential frames,
    // and `wait_for` returns the instant its OWN needle appears, so only
    // a needle that can exclusively come from a LATER frame guarantees
    // everything before it (the divider included) has already arrived.
    wait_for(&mut rx2, &mut replay, "ALT-SCREEN APP", 10).await;
    let text = String::from_utf8_lossy(&replay);
    assert!(
        text.contains("last screen before stop"),
        "the app's content must be preceded by the snapshot divider: {text}"
    );
    h.client.detach(chan2).await;

    stop_task
        .await
        .expect("stop task must not panic")
        .expect("stop must still succeed despite the stubborn child");
}

/// The still-on-the-alternate-screen counterpart to `attach_mid_stop_
/// sees_the_pending_alt_screen_snapshot`: re-checks the pending-map
/// fallback against the CORRECTED replay rule (snapshot existence, not
/// `!alternate_on`), and specifically pins that the `\x1b[?1049l`
/// alt-exit escape composes correctly with a pending-map-served (not yet
/// written to disk) snapshot, not just a file-served one.
///
/// Uses `altscreen-stubborn-child-stays-alt`: this process's own pid dies
/// to the DEFAULT SIGTERM disposition within milliseconds (still on the
/// alternate screen, no restore — unlike `AltscreenStubbornChild`'s
/// restore-then-exit), while its spawned child ignores SIGTERM and forces
/// `kill_process_tree` through its full escalation regardless. Same
/// fixed-delay, best-effort timing approach as the sibling test above —
/// see its own comment for the honest limits of that.
#[tokio::test]
async fn attach_mid_stop_sees_the_pending_snapshot_while_still_on_the_alt_screen() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen-stubborn-child-stays-alt"),
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

    let stopper = h.second_client().await;
    let stop_session_id = session.id.clone();
    let stop_task = tokio::spawn(async move { stopper.stop_session(&stop_session_id).await });

    // See `attach_mid_stop_sees_the_pending_alt_screen_snapshot`'s
    // identical comment: 250ms lands comfortably inside the window
    // between the pane going dead (near-instant, default SIGTERM
    // disposition) and `kill_process_tree` finishing (bounded below by
    // `KILL_GRACE`, 500ms, plus escalating against the stubborn child).
    tokio::time::sleep(Duration::from_millis(250)).await;

    let (chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach mid-stop");
    let mut replay = Vec::new();
    // Ordered wait for the same tmux-version reason as the
    // ignores-term test above (`wait_for_after`'s docs): this fixture's
    // root died still on the alternate screen.
    wait_for_after(
        &mut rx2,
        &mut replay,
        "last screen before stop",
        "ALT-SCREEN APP",
        10,
    )
    .await;
    let text = String::from_utf8_lossy(&replay);
    let exit_alt_screen = text
        .find("\x1b[?1049l")
        .expect("the alt-exit escape must precede the pending-served snapshot too");
    let divider = text
        .find("last screen before stop")
        .expect("the app's content must be preceded by the snapshot divider");
    assert!(
        exit_alt_screen < divider,
        "the alt-exit escape must precede the divider even when the snapshot is served from \
         the pending map rather than the file: {text:?}"
    );
    h.client.detach(chan2).await;

    stop_task
        .await
        .expect("stop task must not panic")
        .expect("stop must still succeed despite the stubborn child");
}

/// A primary-screen agent's stop must never even capture a snapshot, let
/// alone replay one: its real scrollback already survives via ordinary
/// tmux history, so a synthetic "last screen" block would be clutter with
/// no lost content to recover. Pins the alt-screen-only gating in
/// `capture_alt_screen_before_stop` (fed by
/// `TmuxDriver::capture_alt_screen_if_active`'s own alternate-on check)
/// two ways: the snapshot FILE must not exist on disk at all (a
/// deterministic check on the actual artifact this feature writes, not a
/// proxy for it), and the replayed divider text must not appear either
/// (the user-visible consequence, kept as a second, independent
/// assertion).
#[tokio::test]
async fn stop_replay_has_no_snapshot_divider_for_a_primary_screen_agent() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.detach(chan).await;

    h.client.stop_session(&session.id).await.expect("stop");

    let snapshot_path = h.state.path().join("snapshots").join(&session.id);
    assert!(
        !snapshot_path.exists(),
        "a primary-screen stop must never write a snapshot file at all"
    );

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach after stop");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "FAKE-AGENT READY", 15).await;
    // A missing divider needs a settle window, not a single check: the
    // (incorrect) extra frames this test guards against would arrive
    // immediately after the prefill, same as everything else asserted on
    // above, so draining once more after a short wait is enough to catch
    // them without an open-ended sleep.
    tokio::time::sleep(Duration::from_millis(300)).await;
    while let Ok(TermEvent::Data(bytes)) = rx2.try_recv() {
        replay.extend_from_slice(&bytes);
    }
    let text = String::from_utf8_lossy(&replay);
    assert!(
        !text.contains("last screen before stop"),
        "a primary-screen stop replay must not gain the alt-screen divider: {text}"
    );
}

/// A snapshot file must never be consulted for a LIVE pane, no matter
/// what is sitting on disk at its path — the `Attach` handler gates the
/// whole feature on the pane being dead (see `send_alt_screen_snapshot`'s
/// call site), so a leftover or tampered-with file from some earlier
/// state must not leak into an otherwise-ordinary attach. Plants a
/// snapshot file directly (bypassing `stop` entirely) against a session
/// whose agent is still running, so this is a pure "was the file even
/// looked at" check, independent of whether stop's own capture logic
/// would ever have produced this content.
#[tokio::test]
async fn attach_ignores_a_stale_snapshot_file_for_a_live_pane() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let snapshot_dir = h.state.path().join("snapshots");
    std::fs::create_dir_all(&snapshot_dir).expect("create snapshots dir");
    std::fs::write(snapshot_dir.join(&session.id), b"stale content")
        .expect("plant a stale snapshot file");

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    // Same settle-then-drain pattern as the primary-screen negative test
    // above: absence needs a short observation window, not a single
    // immediate check.
    tokio::time::sleep(Duration::from_millis(300)).await;
    while let Ok(TermEvent::Data(bytes)) = rx.try_recv() {
        seen.extend_from_slice(&bytes);
    }
    let text = String::from_utf8_lossy(&seen);
    assert!(
        !text.contains("last screen before stop"),
        "a live pane's attach must never consult a stale snapshot file: {text}"
    );
}

/// Stop's contract is killing the process tree — a storage failure while
/// trying to capture or persist the alt-screen snapshot must never block
/// that. Pre-creates a regular FILE at the path the snapshots
/// subdirectory would occupy, so `ensure_private_dir` fails when
/// `publish_alt_screen_snapshot` tries to create it; `stop_session` must
/// still report success, and the pane must still actually be dead
/// afterwards, proving the kill ran to completion despite the storage
/// failure rather than merely "not erroring".
#[tokio::test]
async fn stop_still_kills_when_the_snapshots_directory_cannot_be_created() {
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

    std::fs::write(
        h.state.path().join("snapshots"),
        b"blocks directory creation",
    )
    .expect("plant a regular file where the snapshots directory belongs");

    h.client
        .stop_session(&session.id)
        .await
        .expect("stop must still succeed despite a storage failure");

    let tmux_name = format!("fh-{}", session.id);
    let sock = h.state.path().join("tmux.sock");
    let out = tmux_query(
        &sock,
        &["display-message", "-p", "-t", &tmux_name, "#{pane_dead}"],
    )
    .await;
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "1",
        "the process tree must still be killed even when the snapshot cannot be stored"
    );
}

/// A snapshot that cannot be READ must degrade the attach to the plain
/// prefill, not fail it — best-effort by design (see
/// `send_alt_screen_snapshot`'s docs). Plants a DIRECTORY at the snapshot
/// path for an already-dead-pane session (`tokio::fs::read` on a
/// directory fails, unlike the ordinary "file absent" case), and requires
/// the attach to still succeed and still replay the ordinary content —
/// just without the divider.
#[tokio::test]
async fn attach_degrades_to_plain_prefill_when_the_snapshot_path_is_unreadable() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.detach(chan).await;

    h.client.stop_session(&session.id).await.expect("stop");

    let snapshot_path = h.state.path().join("snapshots").join(&session.id);
    std::fs::create_dir_all(&snapshot_path).expect("plant a directory at the snapshot path");

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach must still succeed despite an unreadable snapshot");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "FAKE-AGENT READY", 15).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    while let Ok(TermEvent::Data(bytes)) = rx2.try_recv() {
        replay.extend_from_slice(&bytes);
    }
    let text = String::from_utf8_lossy(&replay);
    assert!(
        !text.contains("last screen before stop"),
        "an unreadable snapshot must degrade to the plain prefill, not appear: {text}"
    );
}

/// The chunked send path (`send_alt_screen_snapshot`) must deliver a
/// snapshot LARGER than one `REPLAY_CHUNK` (32 KiB) completely and in
/// order, across however many frames that takes — not just the
/// single-frame case every other snapshot test here happens to exercise
/// (the fixtures' own captured content is far smaller than 32 KiB).
/// Plants a snapshot with a head marker, a marker straddling the
/// (assumed, matching service.rs's own `REPLAY_CHUNK`) 32 KiB chunk
/// boundary, and a tail marker; requires all three to arrive, in that
/// relative order, in the reassembled replay.
#[tokio::test]
async fn dead_pane_snapshot_replay_delivers_a_multi_chunk_snapshot_intact() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.detach(chan).await;

    h.client.stop_session(&session.id).await.expect("stop");

    const ASSUMED_REPLAY_CHUNK: usize = 32 * 1024;
    let mut content = Vec::new();
    content.extend_from_slice(b"HEAD-MARKER");
    content.resize(ASSUMED_REPLAY_CHUNK - 5, b'x');
    content.extend_from_slice(b"BOUNDARY-MARKER");
    content.resize(ASSUMED_REPLAY_CHUNK + 4000, b'y');
    content.extend_from_slice(b"TAIL-MARKER");

    let snapshot_dir = h.state.path().join("snapshots");
    std::fs::create_dir_all(&snapshot_dir).expect("create snapshots dir");
    std::fs::write(snapshot_dir.join(&session.id), &content).expect("plant a multi-chunk snapshot");

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "TAIL-MARKER", 15).await;
    let text = String::from_utf8_lossy(&replay);
    let head = text.find("HEAD-MARKER").expect("head marker must arrive");
    let boundary = text
        .find("BOUNDARY-MARKER")
        .expect("chunk-boundary marker must arrive");
    let tail = text.find("TAIL-MARKER").expect("tail marker must arrive");
    assert!(
        head < boundary && boundary < tail,
        "markers must arrive in order across multiple chunks: {text:?}"
    );
}

/// Fail-closed cleanup applies to the alt-screen snapshot exactly like
/// the launch artifacts (`delete_fails_closed_when_a_launch_artifact_
/// cannot_be_removed`): an unremovable snapshot must fail the WHOLE
/// delete, row and map entry intact, rather than silently losing the last
/// handle on a file that may hold secrets. A non-empty DIRECTORY at the
/// snapshot path (rather than a permission trick) is what actually makes
/// `remove_file` fail here — `unlink` refuses any directory regardless of
/// permissions.
#[tokio::test]
async fn delete_fails_closed_when_the_alt_screen_snapshot_cannot_be_removed() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let snapshot_path = h.state.path().join("snapshots").join(&session.id);
    std::fs::create_dir_all(&snapshot_path).expect("plant a directory at the snapshot path");
    std::fs::write(snapshot_path.join("inner"), b"x").expect("make the directory non-empty");

    let result = h.client.delete_session(&session.id).await;
    let err = result.expect_err("delete must fail closed when the snapshot cannot be removed");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("must carry a SupervisorError")
            .kind,
        ErrorKind::Internal,
        "an unremovable snapshot is a server-side sweep problem, not a caller precondition"
    );

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction on the same state dir");
    let client2 = connect_client(&sup2).await;
    assert_eq!(
        client2
            .list_sessions()
            .await
            .expect("list from fresh supervisor")
            .sessions,
        // The failed delete already tore the tmux session down before the
        // snapshot removal refused, so the surviving row lists as exited
        // with no code — the same honest answer as any restart-gap row.
        vec![with_status(
            session.clone(),
            SessionStatus::Exited { exit_code: None }
        )],
        "a failed delete must leave the row in place for a retry"
    );
}

/// Snapshot files are plain session-id-keyed state under the
/// supervisor's own state dir, so they must survive exactly like the
/// SQLite row does across a supervisor restart (mirroring
/// `stop_kills_the_whole_process_tree`'s own restart check): stop an
/// alt-screen session, construct a SECOND, independent `Supervisor` on
/// the same state dir, and attach through IT — the divider and the app's
/// own marker must both replay, proving the snapshot was read from disk
/// rather than from any in-process state the first supervisor held.
#[tokio::test]
async fn alt_screen_snapshot_survives_a_supervisor_restart() {
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

    h.client.stop_session(&session.id).await.expect("stop");

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction on the same state dir");
    let client2 = connect_client(&sup2).await;

    let (_chan2, mut rx2) = client2
        .attach(&session.id, 80, 24)
        .await
        .expect("attach through a fresh supervisor on the same state dir");
    let mut replay = Vec::new();
    // Waits on the content marker, not the divider — see
    // `stop_replays_the_alt_screen_snapshot`'s identical comment for why:
    // the divider and the snapshot content are separate, sequential
    // frames, and `wait_for` returns as soon as ITS needle appears, so
    // only a needle that can exclusively come from a LATER frame
    // guarantees everything before it (divider included) already
    // arrived.
    wait_for(&mut rx2, &mut replay, "ALT-SCREEN APP", 15).await;
    let text = String::from_utf8_lossy(&replay);
    assert!(
        text.contains("last screen before stop"),
        "the snapshot must survive a supervisor restart and still replay behind its divider: \
         {text}"
    );
}

/// Delete must tolerate a tmux session that disappeared out from under a
/// LIVE `SessionEntry` — someone (or something) else killed it directly
/// on the private socket, distinct from the restart-gap case
/// (`delete_works_on_a_terminal_less_session`) where the whole tmux
/// server, not just one session, failed to survive. `pane_process`'s
/// tolerated-absence path (the same tmux diagnostics `has_session`/
/// `kill_session` already treat as "not there") is what makes this
/// succeed rather than fail-closed.
#[tokio::test]
async fn delete_after_externally_killed_tmux_session_succeeds() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let tmux_name = format!("fh-{}", session.id);
    let sock = h.state.path().join("tmux.sock");
    let killed = tmux_query(&sock, &["kill-session", "-t", &tmux_name]).await;
    assert!(
        killed.status.success(),
        "test setup: kill-session must succeed, got: {}",
        String::from_utf8_lossy(&killed.stderr)
    );

    // This process's own Supervisor still has a LIVE SessionEntry for
    // this session (entries are never demoted from Some to None within
    // one process's lifetime) — unlike the restart-gap tests, no second
    // Supervisor construction is involved here.
    h.client
        .delete_session(&session.id)
        .await
        .expect("delete must tolerate an externally killed tmux session");
    assert!(
        h.client.list_sessions().await.unwrap().sessions.is_empty(),
        "a deleted session must not stay listed"
    );
}

/// A reachable, deterministic fail-closed path for delete's teardown.
///
/// The obvious candidate — an absent pane — does NOT fail closed anymore:
/// per `kill_process_tree`'s `root_pid: None` handling, an absent or dead
/// pane runs the marker-only sweep and then SUCCEEDS
/// (`delete_after_externally_killed_tmux_session_succeeds` pins exactly
/// that). What DOES still fail closed is `pane_process`'s own session-
/// scoping check: renaming the underlying tmux session out from under a
/// live `SessionEntry` (verified empirically — `display-message -t
/// <pane>` happily resolves the renamed session and reports its NEW name)
/// makes the stored `tmux_name` mismatch what tmux now reports, which
/// `pane_process` treats as a hard error rather than "gone" — refusing to
/// guess which session a suspiciously-renamed pane now belongs to. Delete
/// must surface that as `Error`/`Internal` with the row and map entry
/// left in place, not a false `SessionDeleted`.
#[tokio::test]
async fn delete_after_renamed_tmux_session_fails_closed() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let tmux_name = format!("fh-{}", session.id);
    let sock = h.state.path().join("tmux.sock");
    let renamed = tmux_query(
        &sock,
        &["rename-session", "-t", &tmux_name, "renamed-out-from-under"],
    )
    .await;
    assert!(
        renamed.status.success(),
        "test setup: rename-session must succeed, got: {}",
        String::from_utf8_lossy(&renamed.stderr)
    );

    let err = h.client.delete_session(&session.id).await.expect_err(
        "delete must fail closed when the pane's session was renamed out from under it",
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a failed teardown must carry a SupervisorError")
            .kind,
        ErrorKind::Internal,
        "a teardown failure is a server-side sweep problem, not a caller precondition"
    );
    assert_eq!(
        h.client.list_sessions().await.unwrap().sessions,
        vec![with_status(
            session.clone(),
            SessionStatus::Exited { exit_code: None }
        )],
        "a failed delete must leave the row and map entry in place for a retry; \
         session_status requires BOTH the remembered pane id AND the remembered tmux \
         session name to match what tmux currently reports (see that function's own docs) \
         — the rename changes the session name tmux reports for this pane, so the identity \
         can no longer be positively confirmed, and the honest answer is Exited, not a \
         guess either way"
    );
}

/// The acceptance test for the environment-marker half of
/// `kill_process_tree` (lore/2026-07-27-m2-process-tree-stop.md): a
/// daemon that has fully reparented to init — no longer any descendant of
/// the pane's process at all — must still be killed, because only the
/// `FARHELM_SESSION_ID` marker (never a PPID walk) can find it. This must
/// fail if EITHER half of that mechanism is removed: marker injection at
/// launch (`launch.rs`'s `SESSION_ID_ENV_VAR`) or marker enumeration
/// during the sweep (`environ_has_marker`/`enumerate_tree`) — a bare PPID
/// closure from the pane root would never reach this pid at all.
#[tokio::test]
async fn stop_kills_a_reparented_marked_daemon() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-reparent"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let daemon_pid = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;
    assert_ne!(
        self_pid, daemon_pid,
        "the reparented daemon must be a genuinely different process"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(daemon_pid, 15).await;
}

/// The marker-only sweep must find and kill a REAL reparented survivor
/// even when there is no live pane process at all to walk ancestry
/// from — not a hypothetical, but the exact scenario `kill_process_tree`'s
/// `root_pid: None` handling exists for (see that function's docs). This
/// must fail if stop ever goes back to SKIPPING the sweep when the pane
/// looks dead or absent, which is what the first cut of this code did.
///
/// The pane is made dead by killing the agent process directly (not by
/// calling stop first, which would already reap the daemon via the live-
/// pane path and prove nothing about the dead-pane path specifically).
/// `remain-on-exit` keeps the pane around to report `pane_dead`, exactly
/// like `exited_agent_leaves_a_viewable_terminal` relies on elsewhere.
#[tokio::test]
async fn stop_kills_a_reparented_daemon_with_no_live_pane_to_walk_from() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-reparent"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let daemon_pid = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;

    // Kill the pane's own process directly: the pane goes dead
    // (remain-on-exit keeps the terminal), leaving no live pid for
    // kill_process_tree to walk ancestry from at all.
    // SAFETY: self_pid is a real, currently-live pid this test just
    // extracted from the fake agent's own output.
    unsafe {
        libc::kill(self_pid as libc::pid_t, libc::SIGKILL);
    }
    wait_until_pid_gone(self_pid, 10).await;

    h.client.stop_session(&session.id).await.expect("stop");
    wait_until_pid_gone(daemon_pid, 15).await;
}

/// Closure seeding, not just the marker scan alone: the reparented
/// daemon's own child has its `FARHELM_SESSION_ID` marker deliberately
/// stripped (`env -u`), so the marker scan alone would never find it —
/// only reaching it by walking the PPID closure FROM the daemon proves
/// that marker pids seed the closure before it expands, per
/// `enumerate_tree`'s docs. This must fail if that seeding is ever
/// demoted back to appending marker pids as closure LEAVES instead of
/// roots.
#[tokio::test]
async fn stop_kills_an_unmarked_child_of_a_reparented_daemon_via_closure_seeding() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-reparent"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let daemon_pid = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;
    let unmarked_child_pid = wait_for_pid_file(&work.path().join("unmarked-child.pid"), 10).await;
    assert!(
        marked_pids(&session.id).contains(&daemon_pid),
        "test setup: the daemon must actually carry the marker"
    );
    assert!(
        !marked_pids(&session.id).contains(&unmarked_child_pid),
        "test setup: the child must NOT carry the marker — that is the point"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    wait_until_pid_gone(daemon_pid, 15).await;
    wait_until_pid_gone(unmarked_child_pid, 15).await;
}

/// Whether the cgroup path can be exercised here — and, when it cannot,
/// say so LOUDLY before the caller returns.
///
/// One helper rather than a predicate plus a separate announce call,
/// because the two must never come apart: a test that checked availability
/// and forgot to announce would look like a pass while proving nothing,
/// which is the exact failure the loud skip exists to prevent.
///
/// Asked through the production probe rather than a hand-rolled
/// `which systemd-run`, so this answers the same question the supervisor
/// answers, by the same experiment. It is a SEPARATE probe, not a shared
/// verdict: a manager that dies between this call and the supervisor's own
/// could still leave the two disagreeing. That residual is the same one the
/// product has (`scope::ScopeManager`'s cached verdict), and its worst case
/// here is a test that runs the fallback while announcing the scope path —
/// which shows up as the test's own assertions failing, not as a false pass.
///
/// `#[ignore]` would be the obvious alternative and is the wrong one
/// (PLAN_M3.md item 10 says so explicitly): an ignored test is ignored
/// everywhere, including on the development hosts where the scope path is
/// the whole point. The message reaches CI's transcript because the test
/// step runs with `--show-output` (see `.github/workflows/ci.yml`).
async fn cgroup_path_available(test: &str) -> bool {
    if farhelm_supervisor::scope::ScopeManager::systemd()
        .available()
        .await
    {
        return true;
    }
    eprintln!(
        "SKIPPED {test}: this host has no usable systemd user manager, so the cgroup path \
         (PLAN_M3.md item 10) cannot be exercised here; the fallback path is what runs and \
         is proved by the rest of this suite"
    );
    false
}

/// SIGKILLs a pid on drop — failure-safe cleanup for the one fixture
/// `MarkerCleanupGuard` cannot reach.
///
/// The cloaked daemon (`Script::SpawnerCloaked`) carries no marker by
/// construction, so the marker sweep this file's other guard performs
/// would never find it; and on a host without a user manager it is
/// expected to SURVIVE the stop under test. Its own 120s self-expiry is
/// the backstop under this, not a substitute for it.
struct PidKillGuard {
    pid: u32,
    /// The pid's `/proc` start time when this guard was armed.
    ///
    /// Validated again before signaling, exactly as the production sweep
    /// does (`signal_validated` in service.rs). Not paranoia here: this
    /// guard's whole purpose is to clean up a pid the test EXPECTS to be
    /// killed by the code under test, so by the time `Drop` runs the number
    /// is usually free — and a test host busy enough to run this suite is
    /// exactly a host recycling pids. SIGKILLing an unrelated process
    /// because its number came up is not an acceptable cost of tidiness.
    starttime: Option<u64>,
}

impl PidKillGuard {
    fn arm(pid: u32) -> PidKillGuard {
        PidKillGuard {
            pid,
            starttime: proc_starttime(pid),
        }
    }
}

impl Drop for PidKillGuard {
    fn drop(&mut self) {
        if self.starttime.is_none() || proc_starttime(self.pid) != self.starttime {
            return;
        }
        // SAFETY: `libc::kill` validates the pid itself; one that is already
        // gone yields an ignorable errno.
        unsafe {
            libc::kill(self.pid as libc::pid_t, libc::SIGKILL);
        }
    }
}

/// A pid's `/proc` start time (field 22 of `stat`), or `None` if it cannot
/// be read.
///
/// The identity half of a `(pid, starttime)` pair: pids repeat, this does
/// not. Parsed from after the LAST `)` because `comm` can contain both
/// spaces and parentheses — the same rule the supervisor's own `parse_stat`
/// follows, restated here rather than shared because a test reaching into
/// private production internals to check production would prove less.
fn proc_starttime(pid: u32) -> Option<u64> {
    let raw = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let tail = &raw[raw.rfind(')')? + 1..];
    tail.split_whitespace().nth(19)?.parse().ok()
}

/// A marked process spawned OUTSIDE the session's cgroup, killed and
/// reaped on drop.
///
/// The instrument that makes "the backstop sweep still ran" observable at
/// all. Both of stop's mechanisms leave the same end state — nothing
/// running — so the only way to tell them apart is a process exactly one
/// of them can reach: this one is findable ONLY by the marker scan (it is
/// in no scope, and is a child of the test process rather than of the
/// pane), while the cloaked daemon is killable ONLY by the cgroup. A stop
/// that ends both provably ran both.
///
/// Owns the `Child` rather than handing back a bare pid so the zombie is
/// reaped: this IS a child of the test process, and the sweep under test
/// only kills it — somebody still has to `wait()`.
struct MarkedDecoy(std::process::Child);

impl MarkedDecoy {
    /// `Command::env` sets the CHILD's environment, never this process's,
    /// which is the repo rule this file lives under. `sleep 120` bounds the
    /// leak if the test dies before its own cleanup runs.
    fn spawn(session_id: &str) -> MarkedDecoy {
        MarkedDecoy(
            std::process::Command::new("sh")
                .arg("-c")
                .arg("sleep 120")
                .env("FARHELM_SESSION_ID", session_id)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("spawning the marked decoy"),
        )
    }

    fn pid(&self) -> u32 {
        self.0.id()
    }
}

impl Drop for MarkedDecoy {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The headline cgroup acceptance test (PLAN_M3.md item 10, acceptance
/// 10): on a host with a systemd user manager, stop must kill through the
/// launch's own scope AND still run the backstop sweep afterwards.
///
/// Both halves are asserted through processes only ONE mechanism can
/// reach, because the end state is otherwise identical:
///
/// - the cloaked daemon (double-forked to init, marker stripped) is
///   invisible to both halves of `kill_process_tree`, so its death can
///   only have come from the cgroup — this is the residual
///   lore/2026-07-27-m2-process-tree-stop.md accepted and this milestone
///   closes;
/// - the marked decoy is outside the scope entirely, so its death can only
///   have come from the marker sweep — which is SPEC_impl.md's
///   belt-and-suspenders rule made observable.
///
/// The recorded selection is checked too: a run where the manager was
/// present but the launch fell back would kill the decoy and leave the
/// daemon, and reading the column is what turns that into a clear failure
/// rather than a puzzling one.
#[tokio::test]
async fn a_scope_launched_stop_kills_through_the_cgroup_and_still_runs_the_sweep() {
    if !cgroup_path_available(
        "a_scope_launched_stop_kills_through_the_cgroup_and_still_runs_the_sweep",
    )
    .await
    {
        return;
    }
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-cloaked"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let cloaked_pid = wait_for_pid_file(&work.path().join("cloaked.pid"), 10).await;
    let _cloaked_cleanup = PidKillGuard::arm(cloaked_pid);
    assert!(
        !marked_pids(&session.id).contains(&cloaked_pid),
        "test setup: the cloaked daemon must NOT carry the marker — the whole point is that \
         only a cgroup can find it"
    );

    // The tree-shape audit, asserted rather than merely reasoned about:
    // `systemd-run --user --scope` must `exec` in place, so the pane's
    // process IS the agent, exactly as it is without the wrapper. Anything
    // that forked instead would leave the pane pointing at an intermediary
    // — and `pane_process` liveness, `pane_dead_status` exit codes, and the
    // sweep's PPID closure all read that pid.
    let pane_pid_out = tmux_query(
        &h.state.path().join("tmux.sock"),
        &[
            "display-message",
            "-p",
            "-t",
            &format!("fh-{}", session.id),
            "#{pane_pid}",
        ],
    )
    .await;
    assert_eq!(
        String::from_utf8_lossy(&pane_pid_out.stdout).trim(),
        self_pid.to_string(),
        "the scope wrapper must exec in place: the pane's process must still be the agent \
         itself, with nothing spliced in between"
    );

    let decoy = MarkedDecoy::spawn(&session.id);
    let decoy_pid = decoy.pid();

    assert_eq!(
        launch_scope_of(&h, &session.id).await,
        Some(format!("farhelm-{}-0.scope", session.id)),
        "a launch on a manager-equipped host must record its generation-scoped unit"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(cloaked_pid, 15).await;
    wait_until_pid_gone(decoy_pid, 15).await;
}

/// The recorded selection must survive a supervisor restart, and the
/// RESTARTED supervisor must still be able to kill through the scope
/// (PLAN_M3.md item 10's reload interplay, acceptance 10).
///
/// This is the case the durable column exists for: the restarted process
/// never ran the launch, never saw the probe that chose the scope, and
/// re-derives the unit name from the row's id and generation. If the
/// column were dropped — or the name derived from anything the restart
/// changes — the cloaked daemon would survive, since nothing else in the
/// system can reach it.
#[tokio::test]
async fn a_recorded_scope_survives_a_supervisor_restart_and_still_kills() {
    if !cgroup_path_available("a_recorded_scope_survives_a_supervisor_restart_and_still_kills")
        .await
    {
        return;
    }
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-cloaked"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let cloaked_pid = wait_for_pid_file(&work.path().join("cloaked.pid"), 10).await;
    let _cloaked_cleanup = PidKillGuard::arm(cloaked_pid);
    let scope_before = launch_scope_of(&h, &session.id).await;
    assert!(
        scope_before.is_some(),
        "test setup: this launch must be scoped"
    );

    // The predecessor is RELEASED before its replacement is built, and the
    // replacement's ownership is asserted rather than assumed. An
    // overlapping successor starts read-only (`Supervisor::owns_state_dir`)
    // and reconciles nothing, so a test that skipped this would exercise a
    // path production never takes — and, worse here, would prove nothing
    // about the restart at all: a read-only supervisor's stop is not the
    // stop under test. `_tmux` is bound AFTER `state` on purpose; see
    // `TmuxServerGuard`'s docs.
    let Harness {
        client,
        sup,
        state,
        _tmux,
        _slot,
    } = h;
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup) > 1 {
        assert!(tokio::time::Instant::now() < deadline, "connection drain");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);

    let restarted = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction on the same state dir");
    assert!(
        restarted.owns_state_dir(),
        "the predecessor must be gone, or this proves nothing about a restart"
    );
    let client2 = connect_client(&restarted).await;
    assert_eq!(
        stored_launch_scope(state.path(), &session.id).await,
        scope_before,
        "the recorded selection must be unchanged by a supervisor restart"
    );

    client2
        .stop_session(&session.id)
        .await
        .expect("stop through the restarted supervisor");
    wait_until_pid_gone(cloaked_pid, 15).await;
}

/// A launch whose cgroup WRAPPER failed must classify as error, not as a
/// plain exit — PLAN_M3.md item 10's one new failure mode, and the one gap
/// the wrapper opened in item 3's sentinel contract.
///
/// The gap: every other launch failure is reported by farhelm's own exec
/// shim, which writes a sentinel before dying. `systemd-run` runs BEFORE the
/// shim, so a wrapper that fails (the user manager died since the probe, the
/// unit was refused) exits the pane with no sentinel at all — leaving a
/// session that reports "your agent ran and finished" about an agent that
/// never started, and a launch spec holding its full command line on disk
/// with nothing left to consume it.
///
/// The shape is PLANTED rather than provoked, exactly as
/// `a_planted_malformed_spec_sentinel_classifies_error_with_its_detail`
/// plants its sentinel: making a real `systemd-run` fail from inside a test
/// would mean sabotaging the host's user manager. What is planted is only
/// the evidence — an unconsumed spec on a dead pane — while the scope
/// selection under it is the real one this host's real probe made.
#[tokio::test]
async fn a_failed_scope_wrapper_classifies_as_error_rather_than_a_plain_exit() {
    if !cgroup_path_available("a_failed_scope_wrapper_classifies_as_error_rather_than_a_plain_exit")
        .await
    {
        return;
    }
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    assert!(
        launch_scope_of(&h, &session.id).await.is_some(),
        "test setup: this launch must have selected a scope"
    );

    // Kill the agent outright so the pane is dead with no sentinel — the
    // state a failed wrapper leaves, reached the only way a test can.
    let sock = h.state.path().join("tmux.sock");
    let pid_out = tmux_query(
        &sock,
        &[
            "display-message",
            "-p",
            "-t",
            &format!("fh-{}", session.id),
            "#{pane_pid}",
        ],
    )
    .await;
    let pane_pid: u32 = String::from_utf8_lossy(&pid_out.stdout)
        .trim()
        .parse()
        .expect("a live pane must report a pid");
    // SAFETY: a real, currently-live pid this test just read from tmux.
    unsafe {
        libc::kill(pane_pid as libc::pid_t, libc::SIGKILL);
    }
    wait_until_pid_gone(pane_pid, 10).await;

    // The shim consumed and unlinked its own spec on the way past; putting
    // one back is what stands in for a wrapper that died before the shim
    // ever ran.
    let spec = spec_path_for_launch(h.state.path(), &session.id, 0);
    std::fs::write(&spec, b"{}").expect("plant an unconsumed launch spec");

    let found = wait_for_non_alive_status(&h.client, &session.id, 15).await;
    let SessionStatus::Error { detail } = &found.status else {
        panic!("a launch that never reached the shim must classify as error, got {found:?}");
    };
    assert!(
        detail.contains("never reached farhelm's exec shim"),
        "the error must say the agent never started, got {detail:?}"
    );
    assert!(
        !spec.exists(),
        "classifying the failure must also clean up the credential-bearing spec the wrapper \
         left behind"
    );
}

/// The fallback proof, run on EVERY host including the ones that have a
/// manager: a supervisor with no usable user manager records no scope and
/// stops exactly as M2 did (PLAN_M3.md item 10, acceptance 10's second
/// half).
///
/// CI proves this incidentally by having no manager at all; this test
/// makes it provable on a developer machine too, through the injected
/// `ScopeManager::disabled()`. Without it, the fallback would be exercised
/// only where nobody is looking — and the assertion that matters most
/// (`launch_scope` is NULL, so stop is sweep-only) would never run beside
/// the scope path it must stay distinguishable from.
///
/// The cloaked daemon is deliberately NOT part of this test: with no
/// cgroup, nothing can reach it, and asserting its survival would pin a
/// known gap as if it were a feature.
#[tokio::test]
async fn without_a_user_manager_a_launch_records_the_fallback_and_stops_like_m2() {
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            scopes: Arc::new(farhelm_supervisor::scope::ScopeManager::disabled()),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-reparent"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let daemon_pid = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;

    assert_eq!(
        launch_scope_of(&h, &session.id).await,
        None,
        "a launch with no usable user manager must durably record the fallback"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    // Exactly M2's guarantee, unchanged: the pane's own process and the
    // reparented marked daemon both die to the sweep alone.
    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(daemon_pid, 15).await;
}

/// Read one session's recorded cgroup SELECTION straight out of SQLite,
/// together with the unit name that selection derives.
///
/// Through the store rather than through any wire reply on purpose: the
/// selection is deliberately NOT wire vocabulary (PLAN_M3.md item 1 froze
/// M3's protocol before item 10 landed), so the durable column is the only
/// place it exists — and the durability is the property the tests are
/// actually about. The NAME is derived here exactly as the supervisor
/// derives it, never read back, because the database deliberately does not
/// store one (`store::StoredSession::launch_scoped`).
async fn launch_scope_of(h: &Harness, session_id: &str) -> Option<String> {
    stored_launch_scope(h.state.path(), session_id).await
}

/// [`launch_scope_of`] against a state directory rather than a live
/// harness, for the tests that dismantle their harness to release the
/// state dir before asking.
async fn stored_launch_scope(state_dir: &std::path::Path, session_id: &str) -> Option<String> {
    let store = SessionStore::open(&state_dir.join("supervisor.db"), false)
        .await
        .expect("opening the supervisor database read-only");
    let row = store
        .session(session_id)
        .await
        .expect("reading the session row")
        .expect("the session must still have a row");
    row.launch_scoped
        .then(|| farhelm_supervisor::scope::unit_name(session_id, row.generation))
        .flatten()
}

/// Delete must remove a session's launch artifacts, not just the row and
/// the terminal — `launch/<id>.json` can hold the agent's full command
/// line (credentials included, per launch.rs's own docs), and the shim
/// usually unlinks both files itself, but the ordinary case is exactly
/// what makes this easy to leave untested: this plants both files by hand
/// (standing in for a delete that outraces the shim, or a spec that was
/// never launched at all) so the removal path actually runs.
#[tokio::test]
async fn delete_removes_launch_artifacts() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    // Named per LAUNCH, not per session (`launch::spec_path_for_launch`):
    // this session has only ever launched once, so generation 0 is where
    // its files live.
    let spec_path = spec_path_for_launch(h.state.path(), &session.id, 0);
    let status_path = status_path_for_spec(&spec_path);
    wait_for_shim_to_consume_spec(&spec_path).await;
    std::fs::write(&spec_path, b"{}").expect("plant a launch spec");
    std::fs::write(&status_path, b"exec_failed").expect("plant a launch status file");

    h.client.delete_session(&session.id).await.expect("delete");

    assert!(
        !spec_path.exists(),
        "delete must remove the launch spec, which may hold credentials"
    );
    assert!(
        !status_path.exists(),
        "delete must remove the launch status file"
    );
}

/// Delete must remove a session's alt-screen stop snapshot — same
/// confidentiality class as the launch artifacts above (terminal content
/// can hold secrets an agent echoed), and delete is the last moment
/// anything comes back to clean it up. Stops an `altscreen` session first
/// so a snapshot genuinely exists, rather than asserting the absence of a
/// file that was never going to be there.
#[tokio::test]
async fn delete_removes_the_alt_screen_snapshot() {
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

    h.client.stop_session(&session.id).await.expect("stop");

    let snapshot_path = h.state.path().join("snapshots").join(&session.id);
    assert!(
        snapshot_path.exists(),
        "test setup: stopping an alt-screen session must write a snapshot"
    );

    h.client.delete_session(&session.id).await.expect("delete");

    assert!(
        !snapshot_path.exists(),
        "delete must remove the alt-screen snapshot, which may hold secrets an agent echoed"
    );
}

/// Wait for the shim to have consumed and unlinked the REAL launch spec
/// at `spec_path` before a test plants a fake one at the same path —
/// otherwise planting could race the shim's own read and hand it garbage
/// instead of the real spec it needs to exec the fake agent.
async fn wait_for_shim_to_consume_spec(spec_path: &std::path::Path) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while spec_path.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the real launch spec was never consumed by the shim"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Fail-closed artifact removal (SPEC.md/lore/2026-07-27-m2-process-tree-
/// stop.md): a launch spec that cannot be removed must fail the WHOLE
/// delete, row and map entry intact — never silently proceed and lose the
/// last handle on a file that may hold credentials. Removing WRITE
/// permission on the launch directory itself (not the file) is what
/// actually makes a file undeletable on POSIX: `unlink` needs write+exec
/// on the containing directory, not any particular mode on the file.
///
/// Skipped under euid 0: root bypasses directory permission checks
/// entirely, which would make this test pass trivially without
/// exercising the fail-closed path it exists to pin.
#[tokio::test]
async fn delete_fails_closed_when_a_launch_artifact_cannot_be_removed() {
    // SAFETY: geteuid takes no arguments and cannot fail.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!(
            "skipping: running as root, which bypasses the directory permission this test relies on"
        );
        return;
    }

    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let launch_dir = h.state.path().join("launch");
    let spec_path = launch_dir.join(format!("{}.json", session.id));
    wait_for_shim_to_consume_spec(&spec_path).await;
    std::fs::write(&spec_path, b"{}").expect("plant a launch spec");

    use std::os::unix::fs::PermissionsExt;
    let original_mode = std::fs::metadata(&launch_dir)
        .expect("stat launch dir")
        .permissions()
        .mode();
    std::fs::set_permissions(&launch_dir, std::fs::Permissions::from_mode(0o500))
        .expect("restrict launch dir to read+execute only");

    let result = h.client.delete_session(&session.id).await;

    // Restored FIRST and unconditionally, before any assertion that could
    // panic — a permission-broken state dir must not outlive this test
    // regardless of how it ends.
    std::fs::set_permissions(&launch_dir, std::fs::Permissions::from_mode(original_mode))
        .expect("restore launch dir permissions");

    let err = result.expect_err("delete must fail closed when a launch artifact cannot be removed");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("must carry a SupervisorError")
            .kind,
        ErrorKind::Internal,
        "an unremovable artifact is a server-side sweep problem, not a caller precondition"
    );

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction on the same state dir");
    let client2 = connect_client(&sup2).await;
    // Delete's process-tree sweep ran to completion (it happens before
    // the artifact removal that actually failed — see the handler's
    // ordering), so the agent is already dead by the time the row is
    // still-listed here — but it must be a genuinely EXITED row, not
    // `Alive`, even though the delete itself failed closed. Status is
    // computed fresh from tmux at list time, so this polls rather than a
    // single read (same reasoning as `wait_for_non_alive_status`'s docs).
    let found = wait_for_non_alive_status(&client2, &session.id, 15).await;
    assert_eq!(found.id, session.id);
    assert_eq!(found.title, session.title);
    assert_eq!(found.cwd, session.cwd);
    assert_eq!(found.invocation, session.invocation);
    assert!(
        matches!(found.status, SessionStatus::Exited { .. }),
        "a delete that already killed the process tree before failing closed must still \
         list the row as exited, not Alive, got {:?}",
        found.status
    );
    assert_eq!(
        client2
            .list_sessions()
            .await
            .expect("list from fresh supervisor")
            .sessions
            .len(),
        1,
        "a failed delete must leave the row in place for a retry, provable only through a \
         SEPARATE supervisor construction, not this process's own map"
    );

    let _ = std::fs::remove_file(&spec_path);
}

/// A best-effort race: attach from a second client, in a retry loop,
/// concurrently with a delete in flight. `DeleteSession`'s teardown sweep
/// deliberately runs BEFORE it takes the `attachments` lock (see that
/// handler's own comment), which is exactly what lets a concurrent Attach
/// install itself while the sweep is still running — the lock-held phase
/// then tears down WHATEVER attachment exists once it runs, new or old.
/// This test does not try to land in one specific interleaving; it
/// asserts that WHICHEVER one happens is internally consistent: an
/// attach either fails `NotFound` (delete's row/map removal already
/// happened) or succeeds and then receives a `Detached` (the lock-held
/// phase caught it) — never a hang, and never a "succeeded and stayed
/// attached forever" outcome once delete has actually finished. The
/// session must be gone by the end either way.
#[tokio::test]
async fn attach_during_delete_race_ends_in_a_consistent_state() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let deleter = Arc::clone(&h.client);
    let delete_session_id = session.id.clone();
    let delete_task = tokio::spawn(async move { deleter.delete_session(&delete_session_id).await });

    let second = h.second_client().await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "attach-during-delete race never reached a consistent outcome"
        );
        match second.attach(&session.id, 80, 24).await {
            Ok((_channel, mut rx)) => {
                // An attach that succeeded must be told WHY it no longer
                // holds the session, truthfully — a `Detached` naming
                // deletion, exactly as `DeleteSession`'s handler sends
                // once the row is confirmed gone (see its "notice after
                // commit" comment). A bare stream close (`None`) is NOT
                // an acceptable alternative: it would mean the client
                // learned the session vanished with no explanation, which
                // is the same silent-disappearance failure the handler's
                // ordering exists to prevent.
                let reason = tokio::time::timeout(Duration::from_secs(15), async {
                    loop {
                        match rx.recv().await {
                            Some(TermEvent::Detached(reason)) => return reason,
                            Some(TermEvent::Data(_)) => continue,
                            None => panic!(
                                "an attachment that raced a delete closed without a Detached \
                                 notice — the client learned nothing about why"
                            ),
                        }
                    }
                })
                .await
                .expect("an attachment that raced a delete must resolve to Detached");
                assert!(
                    reason.contains("delete"),
                    "Detached reason for a racer that saw a successful delete must name \
                     deletion, got: {reason:?}"
                );
                break;
            }
            Err(e) => {
                // `NotFound` is delete's row/map removal having already
                // landed. `Internal` is the OTHER legitimate shape of
                // this exact race: the entry is still in the map (not
                // removed yet) but delete's teardown already killed the
                // tmux session underneath it, so Attach's own tmux calls
                // fail with an ordinary (unclassified) tmux error. Both
                // are consistent outcomes of "delete got there first";
                // anything else is retried, bounded by the outer
                // deadline, since it may just be a transient blip rather
                // than the race settling.
                let expected_race_outcome = e
                    .downcast_ref::<SupervisorError>()
                    .is_some_and(|se| matches!(se.kind, ErrorKind::NotFound | ErrorKind::Internal));
                if expected_race_outcome {
                    break;
                }
            }
        }
    }

    delete_task
        .await
        .expect("delete task panicked")
        .expect("delete must succeed");
    assert!(
        h.client.list_sessions().await.unwrap().sessions.is_empty(),
        "the session must be gone once the race settles"
    );
}

/// Failure-safe cleanup for tests whose fixtures can leave marked
/// processes running when an assertion panics before `stop`/`delete` is
/// ever called (or before it completes). `Drop` runs even during a panic
/// unwind, so holding one of these for the duration of a test guarantees
/// a best-effort SIGKILL sweep of anything still carrying the session's
/// marker, regardless of where the test failed — the fixtures' own
/// self-expiry bounds (see `Script::SpawnerForkStorm`'s docs) are the
/// backstop under THIS, for the one case even a `Drop` cannot reach: the
/// whole test process being killed externally (a CI timeout, say) before
/// unwinding ever runs.
///
/// Deliberately synchronous (plain `libc::kill`, no starttime validation
/// like `kill_process_tree`'s production sweep): this is test cleanup for
/// a session id that is scoped to this one test and never reused, not a
/// production code path sharing a host with unrelated processes, so the
/// pid-reuse residual that motivates that validation elsewhere does not
/// apply here with any practical likelihood.
struct MarkerCleanupGuard {
    session_id: String,
}

impl MarkerCleanupGuard {
    fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
        }
    }
}

impl Drop for MarkerCleanupGuard {
    fn drop(&mut self) {
        for pid in marked_pids(&self.session_id) {
            // SAFETY: `libc::kill` validates `pid` itself; a pid that is
            // already gone or not ours simply yields an ignorable errno.
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
        }
    }
}

/// Every currently-live pid whose `/proc/<pid>/environ` carries an exact
/// `FARHELM_SESSION_ID=<id>` entry — the test-side mirror of the
/// supervisor's own marker scan (`environ_contains_marker` in
/// service.rs), used here to assert nothing survives a stop, independent
/// of ancestry.
fn marked_pids(session_id: &str) -> Vec<u32> {
    let marker = format!("FARHELM_SESSION_ID={session_id}");
    let mut pids = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return pids;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(environ) = std::fs::read(format!("/proc/{pid}/environ")) else {
            continue;
        };
        if environ
            .split(|&b| b == 0)
            .any(|entry| entry == marker.as_bytes())
        {
            pids.push(pid);
        }
    }
    pids
}

/// The acceptance test for `kill_process_tree`'s SIGSTOP-quiesce phase: a
/// child that continuously forks new marked grandchildren — each one
/// deliberately long-lived (`sleep 3600`, never exiting on its own) —
/// must leave NONE alive after stop, including ones that forked in the
/// narrow gap between SIGTERM and the sweep's later signals — the exact
/// race quiesce exists to close (see that function's docs and
/// lore/2026-07-27-m2-process-tree-stop.md).
///
/// Long-lived grandchildren are the point, not an incidental detail: a
/// SHORT-lived grandchild dies of natural causes within a few hundred
/// milliseconds regardless of whether the sweep ever reaches it, which
/// would let this test pass even with quiescing removed — the opposite
/// of what it exists to catch. Checked immediately after `stop_session`
/// returns, with no bounded retry: `kill_process_tree` already waits out
/// its own confirmation window (`confirm_gone`) before returning `Ok`, so
/// a survivor at this point is a survivor, not a straggler about to die
/// on its own.
///
/// This test's discriminating power was verified empirically while
/// writing it: temporarily disabling BOTH the post-grace SIGSTOP
/// re-enumeration and the `for _ in 0..MAX_QUIESCE_PASSES` fixpoint loop
/// in `kill_process_tree` (so the sweep goes straight from round one's
/// SIGTERM snapshot to a final SIGKILL of that same stale set, with no
/// re-enumeration at all) made this test fail reliably — a marked
/// grandchild that forked during the grace period, after round one's
/// snapshot, survived indefinitely, since nothing ever signaled it —
/// across repeated runs, while the real code passes just as reliably.
/// (An earlier attempt at this same verification, with the fork-storm
/// fixture's forking loop process left to die to a plain `SIGTERM`,
/// passed even with quiescing disabled: the loop's own death — and, it
/// turned out, a `SIGHUP` cascade to its whole foreground process group
/// once the pane's session-leader process died — stopped the storm
/// almost immediately either way, closing the race window before it
/// could matter. That is why the fixture ignores both `TERM` and `HUP`.)
/// The disabling change was reverted before this test was committed; it
/// must never be left disabled in the source.
#[tokio::test]
async fn stop_quiesce_survives_no_marked_process() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-fork-storm"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // Let the storm actually produce a few generations before stopping,
    // so there is something for the sweep to race against rather than a
    // trivially-empty tree.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !marked_pids(&session.id).is_empty(),
        "test setup: the fork storm must have produced at least one live marked process by now"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    let survivors = marked_pids(&session.id);
    assert!(
        survivors.is_empty(),
        "marked process(es) survived stop: {survivors:?} — the quiesce fixpoint let a fork \
         through"
    );
}

// ---------------------------------------------------------------------
// Terminal-path backpressure (PLAN_M2_5.md)
//
// Everything below drives the real pause/resume control messages against
// a real tmux with `pause-after` set, which is the only way to observe
// the two genuinely different catch-up regimes: a SHALLOW pause, lifted
// before tmux gives up, where delivery must be lossless and continuous;
// and a DEEP one, where tmux has cut the stream and the supervisor must
// recover by resetting the client's terminal and replaying history.
// ---------------------------------------------------------------------

/// Extract `FLOOD-NNNNNNNN` record numbers from a raw transcript, in
/// order.
///
/// Byte-oriented and deliberately tolerant of records the stream split
/// (at a notification boundary, or across the catch-up's reset): a
/// half-record simply is not a record. That tolerance is what lets the
/// assertions below be about ORDER — strictly increasing numbers prove
/// both no reordering and no duplicated replay — rather than about exact
/// framing, which no layer on this path promises.
fn flood_records(transcript: &[u8]) -> Vec<u64> {
    const PREFIX: &[u8] = b"FLOOD-";
    const DIGITS: usize = 8;

    transcript
        .windows(PREFIX.len() + DIGITS)
        .filter(|record| record.starts_with(PREFIX))
        .filter_map(|record| {
            std::str::from_utf8(&record[PREFIX.len()..])
                .ok()?
                .parse()
                .ok()
        })
        .collect()
}

/// Assert flood records are exactly consecutive, naming the offending
/// pair.
///
/// Consecutive rather than merely increasing, deliberately: "increasing"
/// is satisfied by a bug that drops every second record, which is exactly
/// the class of loss a flow-control change could introduce. Duplication
/// and reordering both show up as a step that is not +1 as well, so this
/// one predicate covers every way the byte stream could go wrong that a
/// numbered producer can express.
fn assert_records_consecutive(records: &[u64], what: &str, allowed_seams: usize) {
    let mut seams = 0;
    for pair in records.windows(2) {
        if pair[1] == pair[0] + 1 {
            continue;
        }
        // Exactly one record missing, at most `allowed_seams` times: a
        // record straddling a replay/live boundary is delivered as two
        // halves with the replay's own mode sequences between them, so
        // the scanner matches neither half. `counter_records` documents
        // the same effect for the attach cutover. Every OTHER shape —
        // a wider gap, a repeat, a step backwards — is real loss,
        // duplication, or reordering and fails immediately.
        if pair[1] == pair[0] + 2 && seams < allowed_seams {
            seams += 1;
            continue;
        }
        panic!(
            "{what}: record {} follows {} — output was lost, duplicated, or reordered",
            pair[1], pair[0]
        );
    }
}

/// Create a session running the `flood` script — the fast producer every
/// backpressure test needs. Returns the workdir for the caller to hold,
/// exactly like [`basic_session`].
async fn flood_session(h: &Harness) -> (SessionInfo, tempfile::TempDir) {
    let work = tempfile::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script flood"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    (session, work)
}

/// Drain an attachment into `seen` for `window`, returning any detach
/// reason that arrived.
///
/// Distinct from [`wait_for`] because these tests need to observe the
/// ABSENCE of something (no reset during a shallow pause) or to keep
/// reading through a period with no particular marker due — neither of
/// which a needle-driven wait can express.
async fn drain_for(rx: &mut TermStream, seen: &mut Vec<u8>, window: Duration) -> Option<String> {
    let deadline = tokio::time::Instant::now() + window;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => seen.extend_from_slice(&bytes),
            Ok(Some(TermEvent::Detached(reason))) => return Some(reason),
            Ok(None) => return Some("closed".to_string()),
            Err(_) => return None,
        }
    }
}

/// Read until `needle` appears in the transcript, scanning only what is
/// newly arrived.
///
/// [`wait_for`] cannot be used by these tests: it re-runs
/// `String::from_utf8_lossy(seen).contains(...)` over the WHOLE
/// transcript after every chunk, which allocates a fresh copy of it each
/// time and is quadratic in its length. That is fine for the kilobyte
/// transcripts the rest of this file works with and ruinous for the
/// multi-megabyte ones here — ruinous in a particularly misleading way,
/// too: the test itself becomes the slow consumer, which provokes the
/// very tmux-side pause it is trying to observe under controlled
/// conditions. This keeps a cursor instead, overlapping by `needle.len()
/// - 1` bytes so a needle straddling a chunk boundary is still found.
async fn wait_for_bytes(rx: &mut TermStream, seen: &mut Vec<u8>, needle: &[u8], secs: u64) {
    assert!(!needle.is_empty(), "an empty needle is always present");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    let mut scanned = 0;
    loop {
        if seen[scanned..]
            .windows(needle.len())
            .any(|window| window == needle)
        {
            return;
        }
        scanned = seen.len().saturating_sub(needle.len() - 1);
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => seen.extend_from_slice(&bytes),
            Ok(Some(TermEvent::Detached(reason))) => {
                panic!(
                    "stream ended ({reason}) without {needle:?} in {} bytes",
                    seen.len()
                )
            }
            Ok(None) => panic!("stream closed without {needle:?} in {} bytes", seen.len()),
            Err(_) => panic!(
                "timed out waiting for {needle:?}; {} bytes seen, last records: {:?}",
                seen.len(),
                flood_records(&seen[seen.len().saturating_sub(4096)..])
                    .into_iter()
                    .rev()
                    .take(3)
                    .collect::<Vec<_>>()
            ),
        }
    }
}

/// How many records the `flood` fake-agent script emits. Duplicated from
/// `fake_agent::FLOOD_RECORDS` because that module is private to the bin
/// crate. Only ever used to recognize a COMPLETE producer run, so drift
/// here weakens an assertion rather than causing a false failure.
const FLOOD_RECORDS: u64 = 800_000;

/// Force tmux to pause the SUPERVISOR's own output client for `pane`,
/// exactly as `pause-after` would after a stall — but immediately and
/// deterministically.
///
/// This is what makes the reset-then-replay catch-up testable at all.
/// Whether the delay-driven pause actually fires depends on how far tmux
/// happened to read ahead of the client before it stalled, and both
/// outcomes occur on every supported tmux generation (see the
/// either-behavior test below), so a test that waits for one is asserting
/// a race. tmux's documented on-demand form reaches the identical pane
/// state.
///
/// It needs no test-only seam in the supervisor, which is why it is done
/// this way: `refresh-client -A` acts on a NAMED client, and the
/// supervisor's two control clients are distinguishable from outside by
/// their flags. Input rides a client that keeps `no-output` set forever
/// (see `InputClient`), while the output client cleared it at its replay
/// cutover — so "the control client without `no-output`" is exactly the
/// attachment's own stream.
async fn force_tmux_pause(h: &Harness, pane: &str) {
    let sock = h.state.path().join("tmux.sock");
    let listed = tmux_query(
        &sock,
        &["list-clients", "-F", "#{client_name}\t#{client_flags}"],
    )
    .await;
    assert!(
        listed.status.success(),
        "listing tmux clients failed: {}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let listed = String::from_utf8_lossy(&listed.stdout).into_owned();
    let target = listed
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .find(|(_, flags)| !flags.split(',').any(|flag| flag == "no-output"))
        .map(|(name, _)| name)
        .unwrap_or_else(|| panic!("no output control client found among tmux clients:\n{listed}"));
    let paused = tmux_query(
        &sock,
        &[
            "refresh-client",
            "-t",
            target,
            "-A",
            &format!("{pane}:pause"),
        ],
    )
    .await;
    assert!(
        paused.status.success(),
        "forcing a tmux pane pause failed: {}",
        String::from_utf8_lossy(&paused.stderr)
    );
}

/// A forced tmux-side pause must be recovered THROUGH THE REAL
/// ATTACHMENT: terminal reset, history replayed, live output resuming.
///
/// The deterministic counterpart to the either-behavior test below, and
/// the only coverage that runs the FORWARDER's reset-then-replay send on
/// every CI run. The either-behavior test exercises this path only when
/// tmux happens to choose the read-ahead branch, which it did in 1 of 13
/// measured runs across 3.3a/3.4/3.7b — real coverage, but not coverage
/// anything may depend on. Here the pause is forced, so a regression in
/// the reset, the replay, or the continue cutover fails every time.
#[tokio::test]
async fn a_forced_tmux_pause_is_recovered_through_the_real_attachment() {
    let h = harness().await;
    // The counter fixture, not the flood: this test asserts that LIVE
    // output resumes after the replay, and a producer that can finish
    // makes that unfalsifiable — "no further records" would then be
    // correct rather than a pane left paused. `counter` runs until its
    // session is killed.
    let work = tempfile::tempdir().expect("workdir");
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
    let pane = pane_id_of(
        &h.state.path().join("tmux.sock"),
        &format!("fh-{}", session.id),
    )
    .await;

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    // Enough history accumulated that the replay below is unmistakably a
    // history replay rather than a screenful.
    wait_for_bytes(&mut rx, &mut seen, b"CUTOVER-00001200", 60).await;
    let before_pause = seen.len();

    force_tmux_pause(&h, &pane).await;

    // The reset proves the catch-up ran rather than the stream merely
    // continuing; without it the replay would land on top of content the
    // client still held.
    wait_for_bytes(&mut rx, &mut seen, b"\x1bc", 30).await;
    // `wait_for_bytes` returns on the reset itself, so the replay that
    // FOLLOWS it has not been read yet — keep draining before asserting.
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(10)).await;
    assert_eq!(detached, None, "the catch-up must not end the attachment");

    let reset_at = before_pause
        + seen[before_pause..]
            .windows(2)
            .position(|window| window == b"\x1bc")
            .expect("wait_for_bytes already proved the reset arrived");

    let replayed = counter_records(&seen[reset_at..]);
    assert!(
        replayed.len() > 1000,
        "the catch-up replayed only {} records; history was not replayed",
        replayed.len()
    );
    assert_records_consecutive(&replayed, "forced-pause catch-up replay", 1);

    // Live output must resume after the replay: a continue that returned
    // a snapshot but left the pane paused looks identical up to here and
    // leaves the terminal dead.
    let last_replayed = *replayed.last().expect("non-empty");
    let target = format!("CUTOVER-{:08}", last_replayed + 50);
    wait_for_bytes(&mut rx, &mut seen, target.as_bytes(), 60).await;
}

/// The same forced catch-up against an ALTERNATE-SCREEN pane, which
/// selects a different snapshot and a different mode-replay path.
///
/// PLAN_M2_5.md requires the catch-up to be correct on the alternate
/// screen as well as the normal one, and the two share only the command
/// group: alt-screen replay must select the VISIBLE snapshot (never the
/// normal screen's history, which would splice unrelated scrollback into
/// a full-screen app) and must re-enter the alternate buffer BEFORE the
/// content, since `\x1b[?1049h` clears the buffer it switches to. A
/// regression in either shows up here and in no normal-screen test.
#[tokio::test]
async fn a_forced_tmux_pause_recovers_an_alternate_screen_pane() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
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
    let pane = pane_id_of(
        &h.state.path().join("tmux.sock"),
        &format!("fh-{}", session.id),
    )
    .await;

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"FAKE-AGENT READY", 30).await;
    let before_pause = seen.len();

    force_tmux_pause(&h, &pane).await;
    wait_for_bytes(&mut rx, &mut seen, b"\x1bc", 30).await;
    // The replay follows the reset marker; drain it before asserting.
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(10)).await;
    assert_eq!(detached, None, "the catch-up must not end the attachment");

    let recovered = &seen[before_pause..];
    let reset_at = recovered
        .windows(2)
        .position(|window| window == b"\x1bc")
        .expect("wait_for_bytes already proved the reset arrived");
    let after_reset = String::from_utf8_lossy(&recovered[reset_at..]).into_owned();
    let enter_alt = after_reset
        .find("\x1b[?1049h")
        .expect("an alternate-screen pane must re-enter the alternate buffer after the reset");
    let content = after_reset
        .find("ALT-SCREEN APP")
        .expect("the catch-up must replay the alternate screen's own content");
    assert!(
        enter_alt < content,
        "the alternate-screen switch must precede the replayed content — it CLEARS the buffer \
         it switches to, so emitting it afterwards would wipe the replay"
    );
}

/// A forced catch-up must restore INPUT MODES and cursor state, not just
/// content.
///
/// PLAN_M2_5.md requires the catch-up to be a reattach in full, and mode
/// restoration is the half that fails silently: content looks right while
/// bracketed paste and application cursor keys are quietly off, which is
/// the audited silent-loss case SPEC_impl.md calls out. The ordinary
/// reattach path has covered this since M1; the CATCH-UP path reaches it
/// through a different caller, so a regression there (dropping the mode
/// replay, or emitting it before the content that overwrites it) would go
/// unnoticed by every content-only assertion.
#[tokio::test]
async fn a_forced_tmux_pause_restores_modes_and_cursor_state() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let pane = pane_id_of(
        &h.state.path().join("tmux.sock"),
        &format!("fh-{}", session.id),
    )
    .await;

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"FAKE-AGENT READY", 30).await;
    let before_pause = seen.len();

    force_tmux_pause(&h, &pane).await;
    wait_for_bytes(&mut rx, &mut seen, b"\x1bc", 30).await;
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(10)).await;
    assert_eq!(detached, None, "the catch-up must not end the attachment");

    let reset_at = before_pause
        + seen[before_pause..]
            .windows(2)
            .position(|window| window == b"\x1bc")
            .expect("wait_for_bytes already proved the reset arrived");
    let after_reset = String::from_utf8_lossy(&seen[reset_at..]).into_owned();

    // Cursor placement is re-synthesized on every replay, and must come
    // AFTER the content — writing the content moves the cursor, so a
    // position emitted first would be immediately wrong.
    let content = after_reset
        .find("FAKE-AGENT READY")
        .expect("the catch-up must replay the pane's content");
    let cursor = after_reset[content..]
        .find("\x1b[")
        .and_then(|offset| {
            after_reset[content + offset..]
                .find('H')
                .map(|end| content + offset + end)
        })
        .expect("the catch-up must re-synthesize a cursor position after the content");
    assert!(
        cursor > content,
        "cursor placement must follow the replayed content, not precede it"
    );

    // Bracketed paste is the mode a real agent most visibly loses. Only
    // assertable where tmux can report it (3.7+); below that the
    // supervisor degrades that one mode by design.
    if tmux_has_format(&h, "bracket_paste_flag").await {
        assert!(
            after_reset[content..].contains("\x1b[?2004h"),
            "the catch-up must restore bracketed paste — content alone passing here is exactly \
             the audited silent-loss case"
        );
    } else {
        eprintln!("tmux lacks bracket_paste_flag; skipping the mode-restoration assertion");
    }
}

/// A stall teardown that lands AFTER a takeover has installed a new
/// attachment must not detach the winner.
///
/// The dangerous shape is narrow and entirely invisible to ordinary
/// tests: a stalled forwarder hands its teardown to a separate task
/// (it must — forwarders may never take the attachments lock), so between
/// deciding to detach and actually detaching, a takeover can install a
/// DIFFERENT attachment for the same session. Since the winner is a
/// different connection using the same channel id — every helm numbers
/// channels from 1 — a teardown that checked only the channel, or checked
/// nothing, would tear down the innocent winner and send it a stall
/// notice it has no way to interpret.
///
/// Timing is swept rather than blocked on a barrier: the window is
/// between two lock acquisitions inside the supervisor and nothing
/// outside it can synchronize on that. Each iteration aims the takeover
/// at a slightly different offset around the stall deadline, so the sweep
/// covers before, during, and after. Any iteration that lands in the
/// window and gets this wrong fails the test.
#[tokio::test]
async fn a_stall_teardown_racing_a_takeover_never_detaches_the_winner() {
    let stall = Duration::from_millis(800);
    let h = harness_with_timeouts(SupervisorTimeouts {
        stall_detach: stall,
        ..SupervisorTimeouts::default()
    })
    .await;
    let work = tempfile::tempdir().expect("workdir");
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

    for offset_ms in [0i64, 20, 40, 60, 80, 120, 160, 200] {
        // A FRESH connection per iteration for each side: channel ids are
        // per-connection and never recycled, so reusing one client would
        // hand out 1, 2, 3... and the id collision this test depends on
        // would only happen on the first pass.
        let loser = h.second_client().await;
        let (loser_chan, mut loser_rx) = loser.attach(&session.id, 80, 24).await.expect("attach");
        let mut loser_seen = Vec::new();
        wait_for_bytes(&mut loser_rx, &mut loser_seen, b"CUTOVER-", 30).await;
        loser.pause_output(loser_chan).await;

        // Aim the takeover at the moment the stall teardown fires.
        let aim = stall + Duration::from_millis(offset_ms as u64);
        tokio::time::sleep(aim).await;

        let winner = h.second_client().await;
        let (winner_chan, mut winner_rx) = winner
            .attach(&session.id, 80, 24)
            .await
            .expect("takeover attach");
        assert_eq!(
            loser_chan, winner_chan,
            "test premise: both clients must use the same channel id, or the identity check \
             is not being exercised"
        );

        // The winner must survive and keep receiving. A stale teardown
        // detaching it would show up as either a Detached event or a
        // terminal that has gone silent.
        let mut winner_seen = Vec::new();
        wait_for_bytes(&mut winner_rx, &mut winner_seen, b"CUTOVER-", 30).await;
        let before = winner_seen.len();
        let detached = drain_for(&mut winner_rx, &mut winner_seen, Duration::from_secs(2)).await;
        assert_eq!(
            detached, None,
            "offset {offset_ms}ms: the winner was detached by the loser's stall teardown"
        );
        assert!(
            winner_seen.len() > before,
            "offset {offset_ms}ms: the winner stopped receiving output after the loser's \
             stall teardown"
        );
        winner.detach(winner_chan).await;
    }
}

/// Resident memory of the tmux server and the supervisor must stay FLAT
/// while a viewer is stalled against an unbounded producer.
///
/// This is the milestone's headline promise and the one nothing else
/// measures: every other test asserts the CONSEQUENCES of bounded queues
/// (a detach fires, delivery is lossless, order holds), all of which an
/// unbounded implementation satisfies perfectly right up until it
/// exhausts memory. The plan's own audit found an undrained control
/// client grew the tmux server at ~3.5 MB/s without `pause-after`; at that
/// rate a stall of a few seconds is unmistakable against the tolerance
/// below, and a regression that drops the flag or unbounds a queue shows
/// up here and nowhere else.
///
/// Two processes are sampled for two different claims. tmux is the one
/// the audit measured and the one `pause-after` protects. The supervisor
/// is this test process — the harness runs it in-process — so its number
/// carries libtest and the harness itself and is necessarily noisier;
/// it gets the looser bound, and is included because an unbounded
/// per-connection queue would grow it without limit while tmux stayed
/// flat.
///
/// Sampled across several windows rather than as a before/after pair:
/// a single pair cannot tell a leak from an allocator that grabbed one
/// chunk early, while a trend across a stall can.
#[tokio::test]
async fn memory_stays_flat_while_a_viewer_is_stalled() {
    /// Resident bytes of a process, from `/proc/<pid>/statm` (field 2 is
    /// resident pages).
    fn rss_bytes(pid: u32) -> Option<u64> {
        let statm = std::fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
        let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
        Some(pages * 4096)
    }

    let h = harness().await;
    let (session, _work) = flood_session(&h).await;
    let sock = h.state.path().join("tmux.sock");

    let tmux_pid: u32 = {
        let out = tmux_query(&sock, &["display-message", "-p", "#{pid}"]).await;
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .expect("tmux must report its server pid")
    };
    let own_pid = std::process::id();

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"FLOOD-000000", 30).await;

    h.client.pause_output(chan).await;
    // Let whatever was in flight settle before the baseline, so the
    // samples describe the STALL rather than the transition into it.
    drain_for(&mut rx, &mut seen, Duration::from_secs(2)).await;
    let tmux_baseline = rss_bytes(tmux_pid).expect("tmux rss");
    let own_baseline = rss_bytes(own_pid).expect("own rss");

    let mut tmux_peak = tmux_baseline;
    let mut own_peak = own_baseline;
    for _ in 0..6 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        tmux_peak = tmux_peak.max(rss_bytes(tmux_pid).expect("tmux rss"));
        own_peak = own_peak.max(rss_bytes(own_pid).expect("own rss"));
    }

    // Six seconds of stall. Unbounded, the audited growth rate would put
    // tmux ~21 MB over baseline; 8 MB is comfortably above ordinary
    // allocator noise and far below that.
    let tmux_growth = tmux_peak.saturating_sub(tmux_baseline);
    assert!(
        tmux_growth < 8 * 1024 * 1024,
        "the tmux server grew {tmux_growth} bytes during a stalled viewer — `pause-after` is \
         not bounding it"
    );
    // Looser, for the reason in this test's docs: this number is the
    // whole test process. Still far below what an unbounded per-connection
    // queue would reach against this producer.
    let own_growth = own_peak.saturating_sub(own_baseline);
    assert!(
        own_growth < 64 * 1024 * 1024,
        "the supervisor process grew {own_growth} bytes during a stalled viewer — a queue on \
         the terminal path is unbounded"
    );

    // The stall must not have been "flat" merely because everything died.
    h.client.resume_output(chan).await;
    let before = seen.len();
    drain_for(&mut rx, &mut seen, Duration::from_secs(5)).await;
    assert!(
        seen.len() > before,
        "no output resumed after the stall — the flat memory above proves nothing"
    );
}

/// A paused attachment must actually stop delivering output.
///
/// The assertion no other pause test makes, and the one a broken
/// implementation would most easily survive: a forwarder that kept
/// reading tmux and only ran the stall timer passes every
/// end-state-shaped test in this file, because the end state after a
/// resume looks the same either way. This observes the QUIET INTERVAL
/// itself — nothing new arrives while paused — and then that delivery
/// resumes.
///
/// The counter fixture rather than the flood: it paces itself, so
/// "nothing arrived" cannot be an artifact of the producer having
/// finished, and the in-flight backlog to drain first is small.
#[tokio::test]
async fn a_paused_attachment_stops_receiving_until_it_resumes() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
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

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"CUTOVER-", 30).await;

    h.client.pause_output(chan).await;
    // Drain whatever was already in flight when the pause landed: the
    // pause stops the supervisor PULLING from tmux, it does not retract
    // frames already queued toward this client.
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(2)).await;
    assert_eq!(
        detached, None,
        "a paused-but-live attachment must not be detached"
    );

    let quiet_from = seen.len();
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(3)).await;
    assert_eq!(detached, None, "a paused attachment must stay attached");
    assert_eq!(
        seen.len(),
        quiet_from,
        "output kept arriving while paused: the forwarder is still reading tmux and only the \
         stall timer is honoring the pause — {} bytes arrived",
        seen.len() - quiet_from
    );

    h.client.resume_output(chan).await;
    let resumed_from = seen.len();
    drain_for(&mut rx, &mut seen, Duration::from_secs(5)).await;
    assert!(
        seen.len() > resumed_from,
        "no output resumed after ResumeOutput"
    );
}

/// Repeated short pauses that add up to longer than the stall timeout
/// must NOT detach: the timeout is a hard maximum on ONE pause, not a
/// cumulative budget.
///
/// This is the test that fails an implementation keeping a single timer
/// across resumes — the obvious wrong simplification of "detach a pause
/// that lasts too long", and one every end-state test would otherwise
/// miss. It is the direct complement to the stall-detach test: together
/// they pin both halves of what "continuous" means.
#[tokio::test]
async fn repeated_short_pauses_never_accumulate_into_a_stall_detach() {
    let h = harness_with_timeouts(SupervisorTimeouts {
        stall_detach: Duration::from_secs(2),
        ..SupervisorTimeouts::default()
    })
    .await;
    let work = tempfile::tempdir().expect("workdir");
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

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"CUTOVER-", 30).await;

    // Five pauses of 1.2s each: every one comfortably inside the 2s
    // maximum, together three times over it.
    for cycle in 0..5 {
        h.client.pause_output(chan).await;
        let detached = drain_for(&mut rx, &mut seen, Duration::from_millis(1200)).await;
        assert_eq!(
            detached, None,
            "cycle {cycle}: a pause shorter than the stall timeout must never detach"
        );
        h.client.resume_output(chan).await;
        let detached = drain_for(&mut rx, &mut seen, Duration::from_millis(300)).await;
        assert_eq!(
            detached, None,
            "cycle {cycle}: a resumed attachment must stay attached"
        );
    }

    // Still live afterwards, not merely un-detached during the cycles.
    let before = seen.len();
    drain_for(&mut rx, &mut seen, Duration::from_secs(3)).await;
    assert!(
        seen.len() > before,
        "the attachment survived the pause cycles but stopped delivering output"
    );
}

/// A pause held across a large replay, with `PauseOutput` re-sent
/// repeatedly, must still detach relative to the FIRST pause.
///
/// Two failures in one test, both of which every other pause test
/// survives. First, the stall deadline must be ABSOLUTE: an
/// implementation that restarts its timer per chunk, per phase, or on
/// every observed pause message would keep this attachment alive forever
/// while a client sat paused, which is exactly the unbounded pin the
/// timeout exists to prevent. Second, the pause must gate the REPLAY
/// itself and not merely the live pump — pausing mid-replay is the case
/// where the forwarder has megabytes already in hand, so a version that
/// consulted the pause only between live events would push all of it at a
/// client that had said stop.
///
/// The spam is what makes the first failure observable: `PauseOutput`
/// repeated every 300ms is well inside the shortened timeout, so an
/// implementation that lets a repeat overwrite the stored pause start
/// never detaches at all.
#[tokio::test]
async fn a_paused_replay_detaches_relative_to_the_first_pause_despite_pause_spam() {
    let h = harness_with_timeouts(SupervisorTimeouts {
        stall_detach: Duration::from_secs(3),
        ..SupervisorTimeouts::default()
    })
    .await;
    // The flood fixture builds a full history quickly, so the reattach
    // below has a large replay for the pause to land in the middle of.
    let (session, _work) = flood_session(&h).await;
    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"FLOOD-000000", 30).await;
    h.client.detach(chan).await;

    // Reattach and pause immediately, while the replay is still being
    // written rather than after it has drained.
    let (chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach");
    let paused_at = tokio::time::Instant::now();
    h.client.pause_output(chan2).await;

    let mut replay = Vec::new();
    let mut reason = None;
    // Spam pause throughout, never resuming. Every repeat is inside the
    // 3s maximum, so only an absolute deadline detaches at all.
    for _ in 0..20 {
        h.client.pause_output(chan2).await;
        if let Some(seen_reason) =
            drain_for(&mut rx2, &mut replay, Duration::from_millis(300)).await
        {
            reason = Some(seen_reason);
            break;
        }
    }

    let reason = reason.expect(
        "a continuously paused attachment was never detached — repeated PauseOutput is \
         restarting the hard maximum instead of being ignored",
    );
    assert_eq!(
        reason,
        farhelm_proto::DETACH_REASON_STALLED,
        "the detach must be the stall detach"
    );
    let elapsed = paused_at.elapsed();
    assert!(
        elapsed < Duration::from_secs(9),
        "detached after {elapsed:?}, far past the 3s maximum measured from the first pause — \
         the deadline is being restarted rather than held absolute"
    );
}

/// A stall detaches exactly ONE attachment, leaving every other
/// attachment on the same CONNECTION alive (PLAN_M4.md item 3).
///
/// The stall bound is a property of one control-mode client — tmux's
/// `pause-after`/`%pause` are per client — so the teardown it triggers
/// must be scoped to that client's own attachment key. A teardown that
/// swept by connection, or by anything wider than the key, would let one
/// wedged view take down terminals the user is actively watching, which
/// is exactly the outcome the per-terminal design exists to avoid; a
/// genuinely wedged client converges on a whole-client detach anyway, one
/// stall bound at a time.
///
/// The two attachments are two SESSIONS rather than two terminals of one
/// session, because tabs do not exist yet — so this is a
/// connection-scoped over-detach guard, not a tab-isolation test. Their
/// leases are deliberately DISTINCT and deliberately irrelevant: takeover
/// is session-scoped, so two sessions never displace each other whatever
/// their leases say, and distinct leases keep this test from implying
/// otherwise. The same-session variant PLAN_M4.md acceptance item 5
/// describes lands with the tabs PR.
///
/// Both sessions run the QUIET fixture on purpose. The survivor sits
/// undrained for as long as the stall takes to fire, and a chatty fixture
/// would overflow the client's own per-terminal queue in that window —
/// which this client answers with a local stall detach of its own
/// (`SupervisorClient::dispatch`), indistinguishable here from the
/// supervisor-side over-detach under test. Liveness is asserted by typing
/// instead: an echo proves both the attachment and its input route
/// survived.
#[tokio::test]
async fn a_stall_detaches_only_its_own_attachment_not_the_connections_others() {
    let h = harness_with_timeouts(SupervisorTimeouts {
        stall_detach: Duration::from_secs(2),
        ..SupervisorTimeouts::default()
    })
    .await;
    let (stalling_session, _stalling_work) = basic_session(&h).await;
    let (live_session, _live_work) = basic_session(&h).await;

    // Two sessions on one connection, under two different leases: no
    // takeover is possible between them in either direction, so anything
    // that detaches the survivor came from the stall teardown.
    let (stalling_chan, mut stalling_rx) = h
        .client
        .attach_terminal(
            &stalling_session.id,
            80,
            24,
            TerminalSelector::Agent,
            "lease-of-the-stalled-view",
        )
        .await
        .expect("attach the terminal that will stall");
    let (live_chan, mut live_rx) = h
        .client
        .attach_terminal(
            &live_session.id,
            80,
            24,
            TerminalSelector::Agent,
            "lease-of-the-live-view",
        )
        .await
        .expect("attach the terminal that must survive");
    let mut stalling_seen = Vec::new();
    let mut live_seen = Vec::new();
    wait_for(&mut stalling_rx, &mut stalling_seen, "FAKE-AGENT READY", 20).await;
    wait_for(&mut live_rx, &mut live_seen, "FAKE-AGENT READY", 20).await;

    h.client.pause_output(stalling_chan).await;
    let reason = expect_detached(&mut stalling_rx, 15).await;
    assert_eq!(
        reason,
        farhelm_proto::DETACH_REASON_STALLED,
        "the paused attachment must take the stall detach"
    );

    // The connection's other attachment: no notice of its own, and still
    // authorized to type — the stall teardown must not have removed its
    // attachment or its input route along with the stalled one's.
    let live_detached = drain_for(&mut live_rx, &mut live_seen, Duration::from_secs(2)).await;
    assert_eq!(
        live_detached, None,
        "a stall on one attachment detached another attachment on the same connection"
    );
    h.client
        .send_input(live_chan, b"still-mine\r".to_vec())
        .await;
    wait_for(&mut live_rx, &mut live_seen, "still-mine", 15).await;
}

/// A pause from a client that LOST a takeover must not silence the
/// winner.
///
/// Pause carries only a channel id, and channel ids are unique only
/// within a connection — every browser tab rides the helm's single
/// supervisor connection, so two connections trivially collide on id 1.
/// Without both halves of the ownership check (owning connection AND
/// channel), a losing client's pause would silence a terminal it no
/// longer holds, which is a denial of service one tab can inflict on
/// another. This is the same trust boundary the input and resize arms
/// enforce, and it had no test.
#[tokio::test]
async fn a_pause_from_a_client_that_lost_a_takeover_cannot_silence_the_winner() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
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

    // The loser attaches first, on its own connection.
    let (loser_chan, mut loser_rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut loser_seen = Vec::new();
    wait_for_bytes(&mut loser_rx, &mut loser_seen, b"CUTOVER-", 30).await;

    // A second connection takes over. Its channel ids number from 1 too,
    // which is exactly the collision this test needs.
    let winner = h.second_client().await;
    let (winner_chan, mut winner_rx) = winner
        .attach(&session.id, 80, 24)
        .await
        .expect("takeover attach");
    assert_eq!(
        loser_chan, winner_chan,
        "test premise: both clients must use the same channel id, or the connection half of \
         the ownership check is not being exercised"
    );
    let mut winner_seen = Vec::new();
    wait_for_bytes(&mut winner_rx, &mut winner_seen, b"CUTOVER-", 30).await;

    // The loser, which has been detached, pauses "its" channel.
    h.client.pause_output(loser_chan).await;

    let before = winner_seen.len();
    let detached = drain_for(&mut winner_rx, &mut winner_seen, Duration::from_secs(3)).await;
    assert_eq!(
        detached, None,
        "the winner must not be detached by the loser's pause"
    );
    assert!(
        winner_seen.len() > before,
        "the loser's pause silenced the winner's terminal — the ownership check on \
         PauseOutput is not enforcing both channel and owning connection"
    );
}

/// The DEEP-pause contract: a client pause held well past tmux's
/// `pause-after` must still leave the terminal correct — under BOTH of
/// the flow-control behaviors tmux exhibits.
///
/// # Why this test has two branches
///
/// With `pause-after` set and a control client that stops reading, tmux
/// does one of two things, and which one is not something this code gets
/// to choose:
///
/// - **It throttles the pane.** tmux stops reading the PTY, the producer
///   blocks on `write`, and nothing is ever dropped. On resume, delivery
///   continues from exactly where it stopped. This is a genuine
///   end-to-end degrade-to-slow.
/// - **It reads ahead into history and pauses the client's stream.** The
///   producer free-runs, tmux fills its scrollback, and the bytes queued
///   for this client age past `pause-after`, at which point tmux cuts the
///   stream with `%pause` and discards what it had queued. Recovery is
///   then the supervisor's reset-then-replay catch-up, and history is
///   what makes it lossless within the replay floor.
///
/// Audited directly on 2026-07-29 (see SPEC_impl.md's backpressure
/// paragraph): tmux 3.7b took the read-ahead path in every trial, while
/// 3.4 took either path across repeated identical trials. The deciding
/// factor is how far tmux happens to have read ahead of the client at the
/// moment it stalls, which no test can pin down — so asserting one
/// behavior would be asserting a race. This follows the version-tolerant
/// precedent already in this file (see `wait_for_after`): detect which
/// happened, then assert that branch's FULL contract rather than
/// weakening both.
///
/// Both branches are real coverage, and the read-ahead branch is the only
/// end-to-end exercise of the forwarder's reset-then-replay path.
#[tokio::test]
async fn a_deep_pause_ends_correctly_under_either_tmux_flow_control_behavior() {
    let h = harness().await;
    let (session, _work) = flood_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"FLOOD-000000", 30).await;

    let paused_at = seen.len();
    let last_before_pause = flood_records(&seen)
        .last()
        .copied()
        .expect("test setup: records must have been delivered before the pause");
    h.client.pause_output(chan).await;

    // Hold the pause well past `pause-after` so tmux has to make its
    // choice. Draining throughout is deliberate and not a contradiction:
    // the pause stops the SUPERVISOR pulling from tmux, so what arrives
    // here is only what was already in flight — and NOT reading it would
    // instead trip the helm's own detach-not-block rule, ending the
    // attachment for an unrelated reason.
    let detached = drain_for(
        &mut rx,
        &mut seen,
        Duration::from_secs(farhelm_supervisor::tmux::TMUX_PAUSE_AFTER_SECS + 5),
    )
    .await;
    assert!(
        detached.is_none(),
        "a paused-but-live attachment must not be detached: {detached:?}"
    );

    h.client.resume_output(chan).await;
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(20)).await;
    assert_eq!(detached, None, "the attachment must survive the resume");

    // The catch-up reset is what tells the branches apart: it exists only
    // on the path where tmux cut the stream. Searched from the pause
    // point so an earlier attach-time byte pattern cannot be mistaken for
    // one, and taken as the LAST occurrence so a second stall during the
    // post-resume drain is analyzed rather than ignored.
    let reset_at = seen[paused_at..]
        .windows(2)
        .rposition(|window| window == b"\x1bc")
        .map(|offset| paused_at + offset);

    match reset_at {
        None => {
            eprintln!("deep-pause branch: tmux throttled the pane (lossless continuation)");
            // Nothing was dropped, so delivery must be exactly
            // consecutive ACROSS the pause boundary, not merely within
            // the suffix after it — a gap exactly at the boundary is the
            // failure this branch exists to rule out, and slicing at
            // `paused_at` would hide it. Including the last record
            // delivered before the pause is what tests the seam itself.
            let records = flood_records(&seen);
            assert!(
                records.len() > 500,
                "test setup: only {} records arrived, too few for the continuity assertion to \
                 mean anything",
                records.len()
            );
            let boundary = flood_records(&seen[..paused_at]).len().saturating_sub(1);
            assert_records_consecutive(
                &records[boundary..],
                "throttle-branch delivery across the pause boundary",
                1,
            );
        }
        Some(reset_at) => {
            eprintln!("deep-pause branch: tmux cut the stream (reset-then-replay catch-up)");
            // What the client had immediately before this reset. Compared
            // against the replay's first record below: the replay must
            // resume PAST it, never re-deliver content the client still
            // held, which is the "never replay into a populated terminal"
            // rule (PLAN_M2_5.md) observed from the outside.
            let last_before_reset = flood_records(&seen[..reset_at])
                .last()
                .copied()
                .unwrap_or(last_before_pause);
            let after_reset = flood_records(&seen[reset_at..]);
            let first_after = *after_reset
                .first()
                .expect("the catch-up replay must carry records");

            // Consecutive, not merely increasing: the replay is one
            // contiguous history capture followed by live output, so any
            // step other than +1 is loss, duplication, or reordering.
            // "Increasing" alone would pass a bug that dropped every
            // second record.
            assert_records_consecutive(&after_reset, "post-catch-up transcript", 1);
            // Deliberately NOT asserted: that `first_after` exceeds
            // `last_before_reset`. The replay is a fresh capture of
            // retained history, so it legitimately starts BEFORE the last
            // pre-pause record — resetting the terminal first is exactly
            // what makes re-delivering that overlap correct rather than
            // duplication (PLAN_M2_5.md's "never replay into a populated
            // terminal"). The reset is the assertion that matters, and it
            // is `reset_at`'s own existence.
            let _ = (first_after, last_before_reset);
            assert!(
                after_reset.len() > 1000,
                "the catch-up replayed only {} records; history was not replayed",
                after_reset.len()
            );

            // Delivery must actually be live again afterwards, not a
            // one-shot replay into a still-paused pane. Either the
            // producer had already finished during the stall — in which
            // case the replay carries its true tail, the strongest end
            // state available — or records keep arriving.
            let last_after_catch_up = *after_reset.last().expect("non-empty");
            drain_for(&mut rx, &mut seen, Duration::from_secs(10)).await;
            let finished = seen
                .windows(b"FLOOD-DONE".len())
                .any(|window| window == b"FLOOD-DONE");
            let latest = flood_records(&seen[reset_at..])
                .last()
                .copied()
                .expect("non-empty");
            if finished {
                assert_eq!(
                    latest,
                    FLOOD_RECORDS - 1,
                    "the producer finished, so the recovered terminal must hold its true tail"
                );
            } else {
                assert!(
                    latest > last_after_catch_up,
                    "no records arrived after the catch-up ({latest} still the last): the pane \
                     was replayed but never continued"
                );
            }
        }
    }
}

/// The SHALLOW-pause contract: a pause lifted before tmux's own
/// `pause-after` fires must be lossless and continuous — no reset, no
/// replay, delivery simply resuming with the very next record.
///
/// The complement to the deep-stall test, and the reason the supervisor
/// keys its catch-up on tmux's `%pause` notification rather than on "the
/// client was paused at some point". Recovering unconditionally would be
/// correct-looking but wasteful and visibly disruptive: every watermark
/// pause a busy terminal makes — which is the STEADY STATE this milestone
/// designs for — would clear and repaint the user's screen.
///
/// Scoped to a window around the pause rather than the producer's whole
/// run, for the same load-sensitivity reason as the deep-stall test
/// above, and for a sharper one: asserting "no reset ever" across a
/// multi-megabyte run would fail whenever unrelated load stalled the
/// pipeline past `pause-after`, which is correct behavior, not a bug.
#[tokio::test]
async fn shallow_pause_resumes_without_reset_or_replay() {
    let h = harness().await;
    let (session, _work) = flood_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"FLOOD-000000", 30).await;

    let paused_at = seen.len();
    h.client.pause_output(chan).await;
    // Comfortably inside tmux's own window, so it has no reason to cut
    // this client off.
    tokio::time::sleep(Duration::from_millis(500)).await;
    h.client.resume_output(chan).await;
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(10)).await;
    assert_eq!(
        detached, None,
        "a shallow pause must not end the attachment"
    );

    assert!(
        !seen[paused_at..]
            .windows(2)
            .any(|window| window == b"\x1bc"),
        "a pause lifted inside tmux's pause-after window must not trigger a catch-up reset"
    );
    let records = flood_records(&seen);
    assert!(
        records.len() > 1000,
        "test setup: too little output arrived ({} records) for the continuity assertion below \
         to mean anything",
        records.len()
    );
    // Lossless, not merely ordered — and asserted ACROSS the pause
    // boundary rather than only after it, since a gap exactly at the seam
    // is the failure this test exists to rule out. Including the last
    // record delivered before the pause is what tests the seam itself.
    let boundary = flood_records(&seen[..paused_at]).len().saturating_sub(1);
    assert_records_consecutive(
        &records[boundary..],
        "shallow-pause delivery across the pause",
        1,
    );
}

/// A pause that never ends must detach the attachment with the stall
/// reason, and must leave the session itself untouched and reattachable.
///
/// Both halves matter. The detach is what bounds memory when a viewer
/// wedges — every hop's buffers stay pinned for exactly as long as the
/// pause lasts, so "forever" is not an option. The session surviving is
/// what makes the detach an acceptable answer at all: SPEC.md promises a
/// stuck viewer never harms the agent, so the pane must still be running
/// and must still replay correctly to the next client.
#[tokio::test]
async fn a_pause_past_the_stall_timeout_detaches_and_leaves_the_session_healthy() {
    // Short enough to wait out, long enough that ordinary scheduling
    // jitter on a loaded CI runner cannot trip it early.
    let h = harness_with_timeouts(SupervisorTimeouts {
        stall_detach: Duration::from_secs(3),
        ..SupervisorTimeouts::default()
    })
    .await;
    // The counter fixture, NOT the flood: this test has to prove the
    // agent is producing again AFTER the detach, and a producer that can
    // finish during the stall makes that unfalsifiable — its tail would
    // then be present no matter how wedged the pane still was. `counter`
    // runs until its session is killed.
    let work = tempfile::tempdir().expect("workdir");
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

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"CUTOVER-", 30).await;

    h.client.pause_output(chan).await;
    let reason = drain_for(&mut rx, &mut seen, Duration::from_secs(30))
        .await
        .expect("a pause past the stall timeout must produce a detach");
    assert_eq!(
        reason,
        farhelm_proto::DETACH_REASON_STALLED,
        "the stall detach must use the reason both emitters share verbatim"
    );

    // The session is unharmed: still listed alive...
    let listed = h.client.list_sessions().await.expect("list after stall");
    let found = listed
        .sessions
        .iter()
        .find(|s| s.id == session.id)
        .expect("the stalled client's session must still exist");
    assert_eq!(
        found.status,
        SessionStatus::Alive,
        "a stalled viewer must not affect the agent"
    );

    // ...and, the part that actually matters, the AGENT IS RUNNING AGAIN.
    // Metadata saying `Alive` plus a replay of pre-stall bytes proves
    // neither: on the tmux behavior that throttles the pane, the agent's
    // writes were blocked for the whole stall, and a detach that failed to
    // release the pane would leave them blocked forever while every
    // assertion above still passed. Requiring records strictly PAST the
    // last one seen before the detach is what makes a still-wedged pane
    // fail.
    let last_before_detach = counter_records(&seen)
        .last()
        .copied()
        .expect("records must have been delivered before the stall");
    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach after a stall detach");
    let mut replay = Vec::new();
    let target = format!("CUTOVER-{:08}", last_before_detach + 50);
    wait_for_bytes(&mut rx2, &mut replay, target.as_bytes(), 60).await;
}

/// The cross-language invariant PLAN_M2_5.md's honesty argument rests on:
/// the browser's scrollback capacity must sit between SPEC.md's promised
/// floor and tmux's actual history floor, never outside either end.
///
/// Why the UPPER bound matters: after a deep stall the catch-up replays at
/// most `HISTORY_LIMIT` lines. If xterm.js could retain MORE than that, a
/// user would watch scrollback they already had get truncated by the
/// recovery — visible, unexplained loss. Holding the browser at or below
/// the floor is what makes the catch-up's end state observably equivalent
/// to lossless slow delivery instead.
///
/// Why the LOWER bound matters, and why it is pinned HERE rather than left
/// implicit: SPEC.md's own product promise is "at least the current screen
/// plus 10,000 lines of scrollback" — a real minimum, not merely "whatever
/// happens to be at most `HISTORY_LIMIT`". Before this bound was added,
/// `scrollback: 0` (or any value far below the promised floor) satisfied
/// the upper-bound check just as well as a correct value, silently
/// defeating the whole product guarantee this test exists to protect.
///
/// Asserted by reading the UI asset directly, because nothing else
/// connects these numbers: they live in different languages, in different
/// crates, with no shared build step. A test that pinned only the Rust
/// constants would go green while the JavaScript drifted, which is
/// precisely the failure this exists to catch.
#[test]
fn browser_scrollback_stays_within_the_product_floor_and_the_tmux_history_ceiling() {
    /// SPEC.md: "the terminal retains, and replay covers, at least the
    /// current screen plus 10,000 lines of scrollback."
    const SPEC_MINIMUM_SCROLLBACK: u32 = 10_000;

    let terminal_js =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../farhelm-ui/assets/terminal.js");
    let source = std::fs::read_to_string(&terminal_js)
        .unwrap_or_else(|e| panic!("reading {}: {e}", terminal_js.display()));
    let (_, after) = source
        .split_once("scrollback:")
        .expect("terminal.js must configure an explicit xterm.js scrollback");
    let scrollback: u32 = after
        .trim_start()
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .and_then(|digits| digits.parse().ok())
        .expect("terminal.js's scrollback must be a plain integer literal");
    assert!(
        scrollback <= farhelm_supervisor::tmux::HISTORY_LIMIT,
        "terminal.js keeps {scrollback} lines of scrollback but tmux only guarantees {} — a \
         post-stall catch-up would visibly truncate history the user already had",
        farhelm_supervisor::tmux::HISTORY_LIMIT
    );
    assert!(
        scrollback >= SPEC_MINIMUM_SCROLLBACK,
        "terminal.js keeps only {scrollback} lines of scrollback but SPEC.md promises at least \
         {SPEC_MINIMUM_SCROLLBACK} — this is a broken product promise, not merely a cosmetic gap"
    );
}

// ---------------------------------------------------------------------
// PLAN_M3.md item 2/4: boot-id classification, the durable last-known
// outcome, and the durable stop annotation.
//
// "Simulated reboot" throughout means two things together, because a real
// reboot does both: the boot id the supervisor reads changes (injected
// through `SupervisorSeams`), and the private tmux server is gone. Tests
// that changed only the boot id would leave live panes behind for the
// reload to find, which is not a reboot at all.
// ---------------------------------------------------------------------

/// A create-lifecycle seam that fails at exactly one stage and lets every
/// other one through (PLAN_M3.md items 2 and 6).
///
/// One stage at a time is the point: each of `CreateStage`'s boundaries
/// leaves durable state in a different shape, and a test that crashed at
/// several of them at once could not tell which shape its assertions were
/// actually about.
fn crash_at(stage: CreateStage) -> CreateCrashSeam {
    Arc::new(move |reached| {
        if reached == stage {
            anyhow::bail!("simulated crash at {stage:?}");
        }
        Ok(())
    })
}

/// A supervisor on an existing state dir whose boot-id source answers
/// `boot` — the stand-in for a machine that has (or has not) rebooted
/// since the last supervisor ran.
///
/// The three answers are deliberately distinct, because M3 treats them
/// differently: `Ok(Some(id))` is a positive identification, `Ok(None)` is
/// a host that publishes no boot id at all (permanently evidence-free),
/// and `Err` is a host that HAS one this read could not get — which must
/// not be allowed to produce the irreversible answers a successful read
/// would.
async fn supervisor_reading_boot(
    state: &std::path::Path,
    boot: Result<Option<&str>, &str>,
) -> Arc<Supervisor> {
    let boot = boot
        .map(|id| id.map(str::to_string))
        .map_err(str::to_string);
    Supervisor::new_with_seams(
        state,
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            boot_id: Arc::new(move || match &boot {
                Ok(id) => Ok(id.clone()),
                Err(e) => Err(anyhow::anyhow!("{e}")),
            }),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor construction on an existing state dir")
}

/// The common case of [`supervisor_reading_boot`]: a host that reports
/// exactly this boot id.
async fn supervisor_believing_boot(state: &std::path::Path, boot: Option<&str>) -> Arc<Supervisor> {
    supervisor_reading_boot(state, Ok(boot)).await
}

/// Like [`harness`], but the supervisor reads `boot` as the host's boot
/// id, so a later supervisor can be told a different one without the test
/// depending on whether this host publishes a real boot id at all.
async fn harness_believing_boot(boot: &str) -> Harness {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let sup = supervisor_believing_boot(state.path(), Some(boot)).await;
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

/// Wait until tmux itself reports `tmux_name`'s pane dead.
///
/// Deliberately asks tmux directly instead of polling `list_sessions`: the
/// same-boot test needs the agent to have exited WITHOUT the supervisor
/// ever observing it (that is what "with the supervisor down" means for an
/// in-process supervisor), and a list is exactly such an observation.
async fn wait_for_dead_pane(sock: &std::path::Path, tmux_name: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let out = tmux_query(
            sock,
            &["display-message", "-p", "-t", tmux_name, "#{pane_dead}"],
        )
        .await;
        if String::from_utf8_lossy(&out.stdout).trim() == "1" {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "pane of {tmux_name} never died"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// One session out of a listing, by id.
async fn listed(client: &SupervisorClient, id: &str) -> SessionInfo {
    client
        .list_sessions()
        .await
        .expect("list")
        .sessions
        .into_iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("session {id} must still be listed"))
}

/// M3 acceptance 2: on the SAME boot, classification is per session and
/// stays M2's live probing — nothing is interrupted, because nothing
/// happened to the host.
///
/// Three sessions, three different fates while no supervisor is watching:
/// one agent exits on its own (its pane survives holding the code), one
/// has its tmux session killed outright (nothing survives to ask), and one
/// is simply left alone. The reloading supervisor must report exactly
/// those three answers — including the true exit code from the surviving
/// dead pane, which is the "retained knowledge is not a guess" half of the
/// contract, and exited-UNKNOWN where nothing retained anything.
///
/// The exits deliberately happen without any intervening `list_sessions`:
/// a list is how this supervisor witnesses an exit, so listing first would
/// test the recording path rather than the reload path this test is about.
#[tokio::test]
async fn same_boot_classification_is_per_session_and_never_interrupted() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let work = tempfile::tempdir().expect("workdir");

    let exiting = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            "sh -c 'sleep 0.5; exit 3'",
            None,
            80,
            24,
        )
        .await
        .expect("create the self-exiting session");
    let (killed, _killed_work) = basic_session(&h).await;
    let (untouched, _untouched_work) = basic_session(&h).await;

    wait_for_dead_pane(&sock, &format!("fh-{}", exiting.id)).await;
    let out = tmux_query(
        &sock,
        &["kill-session", "-t", &format!("=fh-{}", killed.id)],
    )
    .await;
    assert!(
        out.status.success(),
        "test setup: killing one session's tmux session must succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor on the same boot");
    let client2 = connect_client(&sup2).await;

    // Asserted against what the dead pane ACTUALLY retains, not against a
    // version gate: on a tmux that records exit statuses reliably that is
    // `3`, and on one that loses them under load it is empty — either way
    // the supervisor must report exactly what tmux still holds and never
    // more (`tmux_records_exit_codes_reliably` documents the 3.4 behavior
    // this tolerates). A supervisor that fabricated a code, or dropped one
    // tmux still had, fails this on every host.
    let retained = tmux_query(
        &sock,
        &[
            "display-message",
            "-p",
            "-t",
            &format!("fh-{}", exiting.id),
            "#{pane_dead_status}",
        ],
    )
    .await;
    let retained: Option<i32> = String::from_utf8_lossy(&retained.stdout)
        .trim()
        .parse()
        .ok();
    if tmux_records_exit_codes_reliably() {
        assert_eq!(
            retained,
            Some(3),
            "a tmux this test trusts for codes must have kept this one"
        );
    }
    let exited = listed(&client2, &exiting.id).await;
    assert_eq!(
        exited.status,
        SessionStatus::Exited {
            exit_code: retained
        },
        "the supervisor must report exactly the code the surviving dead pane retains — \
         retained knowledge, never a guess and never a loss"
    );
    assert_eq!(
        listed(&client2, &killed.id).await.status,
        SessionStatus::Exited { exit_code: None },
        "nothing survived this session to hold a code, and none may be invented"
    );
    assert_eq!(
        listed(&client2, &untouched.id).await.status,
        SessionStatus::Alive,
        "an untouched session continues live across a supervisor restart"
    );
}

/// M3 acceptance 3 and 5: after a reboot, sessions that were live become
/// **interrupted** — an explicit lost-track state — while sessions that
/// had already ended keep their status, their codes, and their stop
/// annotations. Interrupted then persists: opening it (attach) fails
/// without changing anything, and further supervisor restarts on the same
/// boot leave it exactly as it was.
///
/// The stop annotation riding through this is the durable half of
/// PLAN_M3.md item 4: it was written when the user stopped the session,
/// and the tmux pane that stop happened in no longer exists by the time it
/// is read back here — so this proves the annotation comes from the
/// supervisor's own durable record and nowhere else.
#[tokio::test]
async fn a_reboot_interrupts_live_sessions_and_preserves_ended_ones() {
    let h = harness_believing_boot("boot-a").await;
    let (live, _live_work) = basic_session(&h).await;
    let (stopped, _stopped_work) = basic_session(&h).await;
    // A session that ends on its own AND is listed before the reboot: the
    // list is where its exit code is witnessed, so what survives below is
    // specifically the recording that list made — omit it and the code is
    // gone with the pane.
    let work = tempfile::tempdir().expect("workdir");
    let ended = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            "sh -c 'sleep 0.5; exit 3'",
            None,
            80,
            24,
        )
        .await
        .expect("create the self-exiting session");
    let ended_status = wait_for_exit_code(&h.client, &ended.id, 3, 30).await.status;

    h.client
        .stop_session(&stopped.id)
        .await
        .expect("stop the session the user ends deliberately");
    let before = listed(&h.client, &stopped.id).await;
    assert_eq!(
        before.annotation.as_deref(),
        Some("stopped by user"),
        "a user-initiated stop annotates the session immediately, not only after a restart"
    );

    // A plain supervisor restart first — same boot, tmux still up, so the
    // stopped session's pane is still there to be probed. This is the path
    // where a live probe and the durable record BOTH have something to
    // say, and the annotation has to come from the record even though the
    // status comes from tmux.
    let sup_restarted = supervisor_believing_boot(h.state.path(), Some("boot-a")).await;
    let client_restarted = connect_client(&sup_restarted).await;
    assert_eq!(
        listed(&client_restarted, &stopped.id)
            .await
            .annotation
            .as_deref(),
        Some("stopped by user"),
        "the annotation survives a supervisor restart, not merely a reboot"
    );
    assert_eq!(
        listed(&client_restarted, &live.id).await.status,
        SessionStatus::Alive,
        "no reboot happened yet, so the live session is untouched"
    );

    // The reboot: tmux dies with the host, and the next supervisor reads a
    // different boot id.
    kill_tmux_server_and_wait(&h.state.path().join("tmux.sock")).await;
    let sup2 = supervisor_believing_boot(h.state.path(), Some("boot-b")).await;
    let client2 = connect_client(&sup2).await;

    assert_eq!(
        listed(&client2, &live.id).await.status,
        SessionStatus::Interrupted,
        "a session that was running when the host rebooted lost its terminal to that \
         reboot; that is knowable, unlike how (or whether) its agent ended"
    );
    let after = listed(&client2, &stopped.id).await;
    assert!(
        matches!(after.status, SessionStatus::Exited { .. }),
        "an already-ended session keeps its status across a reboot: {after:?}"
    );
    assert_eq!(
        after.annotation.as_deref(),
        Some("stopped by user"),
        "the stop annotation is durable session metadata (SPEC.md), so it survives the \
         terminal it was recorded against"
    );

    let ended_after = listed(&client2, &ended.id).await;
    assert_eq!(
        ended_after.status, ended_status,
        "an exit observed by a list before the reboot keeps the code that list recorded, \
         even though the pane that held it is gone"
    );
    assert_eq!(
        ended_after.annotation, None,
        "an agent that ended on its own is never credited to the user"
    );

    // Attaching to an interrupted session fails, and — the part that
    // matters — leaves the classification exactly as it was: nothing
    // relaunches, and nothing gets downgraded to exited-unknown by the
    // attempt. This is SPEC.md's "opening and declining changes nothing"
    // as far as this PR can go; the resume OFFER itself is item 9.
    client2
        .attach(&live.id, 80, 24)
        .await
        .expect_err("an interrupted session has no terminal to attach to");
    assert_eq!(
        listed(&client2, &live.id).await.status,
        SessionStatus::Interrupted,
        "declining the offer changes nothing"
    );

    // A further restart on the SAME boot must not reclassify either.
    let sup3 = supervisor_believing_boot(h.state.path(), Some("boot-b")).await;
    let client3 = connect_client(&sup3).await;
    assert_eq!(
        listed(&client3, &live.id).await.status,
        SessionStatus::Interrupted,
        "interrupted is a durable outcome, not a per-startup inference"
    );
    assert_eq!(
        listed(&client3, &stopped.id).await.annotation.as_deref(),
        Some("stopped by user")
    );
}

/// M3 acceptance 3's pre-M3 clause: a database with no stored boot id
/// (every database written before this milestone) must NOT be read as a
/// reboot on its first M3 startup. There is no evidence either way, and
/// the no-guessing rule cuts both ways — so the same-boot path runs and a
/// still-live session keeps listing as alive.
///
/// Modelled by a first supervisor that reads no boot id at all, which
/// stores nothing, leaving the database in exactly the state a pre-M3 one
/// is in. The second half proves the id really is adopted from then on:
/// once a boot id HAS been stored, a later differing one does interrupt.
#[tokio::test]
async fn a_database_without_a_stored_boot_id_does_not_claim_a_reboot() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let sup1 = supervisor_believing_boot(state.path(), None).await;
    let client1 = connect_client(&sup1).await;
    let work = tempfile::tempdir().expect("workdir");
    let session = client1
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let sup2 = supervisor_believing_boot(state.path(), Some("boot-b")).await;
    let client2 = connect_client(&sup2).await;
    assert_eq!(
        listed(&client2, &session.id).await.status,
        SessionStatus::Alive,
        "with nothing stored to compare against, a differing boot id is not evidence of a \
         reboot — and the live tmux session proves the point independently"
    );

    // `boot-b` is stored now, so a THIRD boot id is a real reboot.
    kill_tmux_server_and_wait(&state.path().join("tmux.sock")).await;
    let sup3 = supervisor_believing_boot(state.path(), Some("boot-c")).await;
    let client3 = connect_client(&sup3).await;
    assert_eq!(
        listed(&client3, &session.id).await.status,
        SessionStatus::Interrupted,
        "once a boot id has been adopted, a change in it is the reboot evidence this \
         classification runs on"
    );
    drop(slot);
}

/// PLAN_M3.md item 2's ordering rule, pinned at the boundary it exists
/// for: the durable **launching** record must be committed BEFORE any
/// external side effect of the launch.
///
/// A crash injected immediately after that commit (the create seam skips
/// every cleanup path a graceful failure would run — a real crash gets no
/// cleanup either) must leave evidence that a launch was attempted. Under
/// M2's ordering there would be nothing at all: the row was written only
/// after tmux had the session, so this crash would have left silence.
///
/// What the next startup does with that evidence is the second half, and
/// it is deliberately NOT "exited": SPEC.md's exited means the agent RAN,
/// and a launch whose side effects were never found has not established
/// that. The row stays pending and lists as **unknown** — the honest
/// not-yet-classified answer — until PLAN_M3.md item 3's sentinel can call
/// it an error or item 6's reservation can retry it. Never alive (nothing
/// is running), never interrupted (no reboot happened), and never exited
/// (nothing was observed to run).
#[tokio::test]
async fn a_crash_after_the_launching_record_leaves_evidence_and_stays_pending() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let sup1 = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            create_crash: Some(crash_at(CreateStage::AfterRecord)),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let client1 = connect_client(&sup1).await;
    let work = tempfile::tempdir().expect("workdir");
    client1
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect_err("the injected crash must fail the create");

    // Nothing external happened: no tmux session was ever created.
    let sessions = tmux_query(&state.path().join("tmux.sock"), &["list-sessions"]).await;
    assert!(
        !String::from_utf8_lossy(&sessions.stdout).contains("fh-"),
        "the crash landed before the tmux side effect, so no session may exist: {}",
        String::from_utf8_lossy(&sessions.stdout)
    );

    let sup2 = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("the next startup");
    let client2 = connect_client(&sup2).await;
    let listing = client2.list_sessions().await.expect("list");
    assert_eq!(
        listing.sessions.len(),
        1,
        "the launching record is the evidence the crash left behind: {:?}",
        listing.sessions
    );
    assert_eq!(
        listing.sessions[0].status,
        SessionStatus::Unknown,
        "a launch whose side effects were never found has not been shown to have run, so \
         it stays pending rather than claiming an exit"
    );

    // And it stays pending across further restarts — nothing degrades it
    // into a fabricated exit later.
    let sup3 = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("a further startup");
    let client3 = connect_client(&sup3).await;
    assert_eq!(
        client3.list_sessions().await.expect("list").sessions[0].status,
        SessionStatus::Unknown
    );
    drop(slot);
}

/// PLAN_M3.md item 4 end to end: a stop's annotation is written where the
/// stop happens and read back by a supervisor that never saw it.
///
/// The two sessions are the contrast that gives the assertion meaning: one
/// ends because the user stopped it, the other because its tmux session
/// was killed out from under it (a stand-in for any ending the user had
/// nothing to do with). Both come back exited from the fresh supervisor —
/// only one of them says who did it. Without that contrast, an annotation
/// applied to every ended session would pass just as well.
///
/// The reconciliation of a stop INTERRUPTED mid-sweep is a different edge
/// and is pinned where it can be provoked deterministically, against the
/// reload itself (`service.rs`'s
/// `reload_reconciles_a_stop_intent_against_the_pane_it_left_behind`).
#[tokio::test]
async fn a_stop_annotation_is_written_where_it_happens_and_read_back_elsewhere() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let (stopped, _stopped_work) = basic_session(&h).await;
    let (killed, _killed_work) = basic_session(&h).await;

    h.client.stop_session(&stopped.id).await.expect("stop");
    let out = tmux_query(
        &sock,
        &["kill-session", "-t", &format!("=fh-{}", killed.id)],
    )
    .await;
    assert!(out.status.success(), "test setup: kill the other session");

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("the next startup");
    let client2 = connect_client(&sup2).await;

    let after = listed(&client2, &stopped.id).await;
    assert!(
        matches!(after.status, SessionStatus::Exited { .. }),
        "the stopped session ended: {after:?}"
    );
    assert_eq!(
        after.annotation.as_deref(),
        Some("stopped by user"),
        "the annotation is durable session metadata, not something the stopping process \
         merely held in memory"
    );

    let other = listed(&client2, &killed.id).await;
    assert!(
        matches!(other.status, SessionStatus::Exited { .. }),
        "the other session ended too: {other:?}"
    );
    assert_eq!(
        other.annotation, None,
        "an ending the user did not cause must never be credited to them"
    );
}

/// The list-versus-stop race, driven through the REAL handlers rather than
/// the store: a client polling `ListSessions` while a `StopSession` runs
/// must not end up with a session that lists as a plain exit.
///
/// This is the concrete loss seven review lenses converged on. The window
/// is not exotic — `kill_process_tree` spends seconds on SIGTERM, a grace
/// period, re-enumeration and SIGKILL, tmux marks the pane dead the
/// instant the process actually dies, and the UI lists every couple of
/// seconds — so a list observing that death mid-sweep is the ORDINARY
/// case, not a corner one. The poll loop below runs as fast as it can for
/// exactly that reason: it is trying to be the observer that gets there
/// first.
#[tokio::test]
async fn a_list_polling_through_a_stop_never_erases_the_annotation() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let poller = h.second_client().await;
    let id = session.id.clone();
    let polling = tokio::spawn(async move {
        for _ in 0..200 {
            if poller.list_sessions().await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
    h.client.stop_session(&session.id).await.expect("stop");
    polling.await.expect("the poller must not panic");

    let stopped = listed(&h.client, &id).await;
    assert!(
        matches!(stopped.status, SessionStatus::Exited { .. }),
        "the stop ended the session: {stopped:?}"
    );
    assert_eq!(
        stopped.annotation.as_deref(),
        Some("stopped by user"),
        "no amount of concurrent listing may erase who ended this session"
    );

    // And it is DURABLE, not just what this supervisor happens to hold.
    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("the next startup");
    let client2 = connect_client(&sup2).await;
    assert_eq!(
        listed(&client2, &id).await.annotation.as_deref(),
        Some("stopped by user")
    );
}

/// Stopping a session whose agent had ALREADY exited on its own must
/// record a plain exit, never the stop annotation: the user pressed stop,
/// but they did not end this run, and SPEC.md's annotation says who did.
///
/// Read back through a fresh supervisor because the claim is about the
/// durable record, not about what one process happens to be holding.
#[tokio::test]
async fn stopping_an_already_exited_session_records_no_annotation() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            "sh -c 'sleep 0.5; exit 5'",
            None,
            80,
            24,
        )
        .await
        .expect("create");
    wait_for_dead_pane(
        &h.state.path().join("tmux.sock"),
        &format!("fh-{}", session.id),
    )
    .await;

    h.client
        .stop_session(&session.id)
        .await
        .expect("stopping an already-dead session still succeeds");

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("the next startup");
    let client2 = connect_client(&sup2).await;
    let after = listed(&client2, &session.id).await;
    assert!(
        matches!(after.status, SessionStatus::Exited { .. }),
        "the agent had already ended: {after:?}"
    );
    assert_eq!(
        after.annotation, None,
        "the user did not end this run, so nothing may say they did"
    );
}

/// PLAN_M3.md item 2's read-failure clause: a boot id that cannot be READ
/// is not the same as a host that HAS none, and treating it as such would
/// let a transient `/proc` failure produce an irreversible answer — every
/// still-live session durably recorded as exited, on evidence that never
/// arrived.
///
/// The sequence pins both halves: the failed read must neither clear nor
/// replace the stored id (so nothing is reclassified on it), and the
/// LATER successful read of a different id must still see the original
/// stored value and interrupt exactly as it would have without the
/// failure in between.
#[tokio::test]
async fn an_unreadable_boot_id_defers_rather_than_deciding() {
    let h = harness_believing_boot("boot-a").await;
    let (session, _work) = basic_session(&h).await;

    // The reboot happens; the next supervisor cannot read the boot id.
    kill_tmux_server_and_wait(&h.state.path().join("tmux.sock")).await;
    let degraded = supervisor_reading_boot(h.state.path(), Err("/proc is unavailable")).await;
    let degraded_client = connect_client(&degraded).await;
    assert_eq!(
        listed(&degraded_client, &session.id).await.status,
        SessionStatus::Exited { exit_code: None },
        "with no boot id to compare, this pass can only report what it can see — and it \
         must not durably decide anything on that"
    );

    // The read succeeds on the next startup: the stored id is still
    // `boot-a`, so the reboot IS detected, and the session that the
    // degraded pass could have written off as a plain exit is correctly
    // interrupted instead.
    let recovered = supervisor_believing_boot(h.state.path(), Some("boot-b")).await;
    let recovered_client = connect_client(&recovered).await;
    assert_eq!(
        listed(&recovered_client, &session.id).await.status,
        SessionStatus::Interrupted,
        "the deferred classification must still be reachable once the read works"
    );
}

// ---------------------------------------------------------------------
// PLAN_M3.md item 3: error status via the launch shim's sentinel.
// ---------------------------------------------------------------------

/// M3 acceptance 3/4's core case: an invocation that cannot even `exec`
/// (argv0 names a file that simply does not exist, inside the session's
/// own throwaway tempdir) must list as **error**, carrying the shim's own
/// errno detail, and that classification must be DURABLE — surviving both
/// an ordinary supervisor restart and a simulated reboot, in the latter
/// case landing on error rather than the interrupted a plain reboot
/// conversion would otherwise produce.
///
/// The sentinel is witnessed here through an ordinary `list_sessions`
/// poll (`wait_for_non_alive_status`), which is the common case: most
/// exec failures WILL be listed at least once before anything restarts.
/// [`a_reboot_never_interrupts_a_row_a_sentinel_already_claims_as_error`]
/// below covers the harder case — a reboot landing before any list ever
/// consumed the sentinel, with the row still `Running` in the store.
#[tokio::test]
async fn unexecutable_invocation_lists_as_error_and_outranks_a_reboot() {
    let h = harness_believing_boot("boot-a").await;
    let sock = h.state.path().join("tmux.sock");
    let work = tempfile::tempdir().expect("workdir");
    let missing_binary = work.path().join("no-such-farhelm-agent");

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &missing_binary.to_string_lossy(),
            None,
            80,
            24,
        )
        .await
        .expect("create a session whose invocation cannot exec");

    // The shim's own process — which has already exec'd over the login
    // shell by the time it attempts the REAL agent's exec — dies the
    // moment that second exec fails, so the pane goes dead almost
    // immediately, well before any list has a chance to observe it.
    wait_for_dead_pane(&sock, &format!("fh-{}", session.id)).await;

    let before = wait_for_non_alive_status(&h.client, &session.id, 30).await;
    let detail = match before.status {
        SessionStatus::Error { detail } => detail,
        other => panic!("expected Error, an exec failure must never read as {other:?}"),
    };
    assert!(
        detail.contains("exec_failed") && detail.contains("errno="),
        "the shim's own errno detail must reach the wire verbatim: {detail}"
    );

    // Survives an ordinary supervisor restart on the SAME boot: by now the
    // sentinel FILE is gone (consumed once its Error outcome committed —
    // see `service.rs`'s reload/list sentinel-lifecycle comments), so this
    // proves the classification is durable store state, not a live
    // re-read of a file that no longer exists.
    let sup2 = supervisor_believing_boot(h.state.path(), Some("boot-a")).await;
    let client2 = connect_client(&sup2).await;
    assert_eq!(
        listed(&client2, &session.id).await.status,
        SessionStatus::Error {
            detail: detail.clone()
        },
        "a durable error outcome survives a supervisor restart"
    );

    // Survives a simulated reboot too, and the sharper claim: as ERROR,
    // not INTERRUPTED. By the time this reboot lands the row was ALREADY
    // terminal (`Error`), so `record_boot`'s blanket interrupt conversion
    // (which only touches launching/running/stop-requested rows) never
    // even considers it — the ordinary terminal-state stickiness already
    // proven by `a_reboot_interrupts_live_sessions_and_preserves_ended_ones`
    // is what protects it here.
    kill_tmux_server_and_wait(&sock).await;
    let sup3 = supervisor_believing_boot(h.state.path(), Some("boot-b")).await;
    let client3 = connect_client(&sup3).await;
    assert_eq!(
        listed(&client3, &session.id).await.status,
        SessionStatus::Error { detail },
        "an exec failure must classify error across a reboot, never interrupted"
    );
}

/// PLAN_M3.md item 3's hardest precedence case, and the reason
/// `SessionStore::record_boot` takes a `sentinel_overrides` map instead of
/// leaving this to an ordinary `Transition`: a launch sentinel that
/// exists while its row is STILL `Running` in the store — because nothing
/// ever listed this session before the reboot to let the sentinel
/// classify it first — must still win the race against the blanket
/// interrupt conversion that same reboot triggers. Get the ordering wrong
/// (convert to `Interrupted` first, check sentinels after) and this row
/// is already terminal and immune to reclassification by the time
/// anything looks for its sentinel — exactly the bug this test exists to
/// catch.
#[tokio::test]
async fn a_reboot_never_interrupts_a_row_a_sentinel_already_claims_as_error() {
    let h = harness_believing_boot("boot-a").await;
    let sock = h.state.path().join("tmux.sock");
    let work = tempfile::tempdir().expect("workdir");
    let missing_binary = work.path().join("no-such-farhelm-agent");

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &missing_binary.to_string_lossy(),
            None,
            80,
            24,
        )
        .await
        .expect("create a session whose invocation cannot exec");

    // Deliberately no `list_sessions` call here at all: this row must
    // still be `Running` in the store (`ConfirmRunning` committed at
    // create time, nothing has observed the exec failure since) when the
    // reboot below lands.
    wait_for_dead_pane(&sock, &format!("fh-{}", session.id)).await;

    kill_tmux_server_and_wait(&sock).await;
    let sup2 = supervisor_believing_boot(h.state.path(), Some("boot-b")).await;
    let client2 = connect_client(&sup2).await;
    let status = listed(&client2, &session.id).await.status;
    assert!(
        matches!(status, SessionStatus::Error { .. }),
        "a sentinel-bearing launch must classify error even though its row was still \
         `Running` at the exact moment a reboot was detected: {status:?}"
    );
}

/// SPEC.md's other half of the error/exited split, pinned directly: an
/// agent whose invocation DOES exec successfully, and then exits with 126
/// or 127 (the codes a shell conventionally uses for "found but not
/// executable" and "command not found" — easy to mistake for exec
/// failure since they LOOK like one), must classify exited, never error.
/// Exit codes alone never carry classification weight; only the
/// sentinel's presence does.
#[tokio::test]
async fn exec_that_succeeds_and_exits_126_or_127_is_exited_never_error() {
    let h = harness().await;
    for code in [126, 127] {
        let work = tempfile::tempdir().expect("workdir");
        let session = h
            .client
            .create_session(
                &work.path().to_string_lossy(),
                &format!("sh -c 'exit {code}'"),
                None,
                80,
                24,
            )
            .await
            .expect("create");
        let status = wait_for_exit_code(&h.client, &session.id, code, 30)
            .await
            .status;
        assert!(
            matches!(
                status,
                SessionStatus::Exited { exit_code: Some(c) } if c == code
            ) || status == (SessionStatus::Exited { exit_code: None }),
            "exit code {code} must classify exited, never error: {status:?}"
        );
    }
}

/// PLAN_M3.md item 5's other launch-failure class: a spec that never even
/// reached `exec` at all — missing or malformed, in the shim's own
/// vocabulary (pinned at the unit level in `launch.rs`'s
/// `exec_launch_spec_records_a_sentinel_for_a_malformed_spec` and
/// `..._for_a_missing_spec`) — must classify identically to a real exec
/// failure once the supervisor reads whatever sentinel resulted.
///
/// Planted directly at the sentinel's own derived path
/// (`spec_path_for_session`/`status_path_for_spec`, both public exactly so
/// a test can agree with the shim on where its output lives) rather than
/// raced out of a genuine shim run: `create_session` never itself hands
/// the shim a torn or missing spec (the write-then-launch ordering in
/// `service.rs`'s `create_session` guarantees a valid spec exists before
/// the tmux window that would read it is even created), so reaching this
/// failure class end-to-end would mean deliberately corrupting
/// supervisor-internal state anyway. Planting the sentinel directly tests
/// the piece this PR actually owns — the SUPERVISOR's reader and
/// classifier — independent of which shim code path produced the file,
/// and does so deterministically rather than racing a real tmux window.
///
/// The session's REAL agent is left running throughout: this also proves
/// a sentinel outranks even a genuinely alive pane (PLAN_M3.md item 3's
/// "outranks every inference"), which only a fresh `reload_sessions`
/// checks unconditionally — the list path only checks a dead-or-absent
/// pane's sentinel (`service.rs`'s `ListSessions` handler docs) — so a
/// supervisor restart is what exercises the stronger check.
#[tokio::test]
async fn a_planted_malformed_spec_sentinel_classifies_error_with_its_detail() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    assert_eq!(
        listed(&h.client, &session.id).await.status,
        SessionStatus::Alive,
        "the session's real agent must still be genuinely alive throughout this test"
    );

    let detail = format!(
        "launch spec at /state/launch/{}.json is malformed: EOF while parsing a value",
        session.id
    );
    let status_path = status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    std::fs::write(&status_path, &detail).expect("plant the sentinel");

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("restart to trigger reload's unconditional sentinel check");
    let client2 = connect_client(&sup2).await;
    assert_eq!(
        listed(&client2, &session.id).await.status,
        SessionStatus::Error {
            detail: detail.clone()
        },
        "a planted sentinel must classify error with its exact detail, even against a pane \
         that never stopped being genuinely alive"
    );
    assert!(
        !status_path.exists(),
        "a consumed sentinel is deleted once its Error outcome commits durably"
    );
}

/// Review-swarm fix batch item 5/19: a session `reload_sessions` classifies
/// `Error` via its sentinel must NOT lose its terminal in the process — the
/// bug this pins is that the sentinel branch used to `continue` before
/// ever recording the pane it had already found, leaving `Attach` refusing
/// a session whose dead pane genuinely still exists in tmux, and
/// `DeleteSession`'s kill sweep with nothing to act on at all (a leaked
/// tmux session). Attach must succeed (the dead pane is viewable, exactly
/// like any other exited session), and delete must actually tear the real
/// tmux session down, not merely drop the row.
#[tokio::test]
async fn a_reload_classified_error_session_keeps_its_terminal_for_attach_and_delete() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let work = tempfile::tempdir().expect("workdir");
    let missing_binary = work.path().join("no-such-farhelm-agent");

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &missing_binary.to_string_lossy(),
            None,
            80,
            24,
        )
        .await
        .expect("create a session whose invocation cannot exec");
    wait_for_dead_pane(&sock, &format!("fh-{}", session.id)).await;

    // Reload — not list — is what this test targets: a fresh supervisor's
    // `reload_sessions` reconciliation is where item 5's bug lived,
    // separately from `ListSessions`'s own (already-correct) sentinel
    // branch.
    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("reload onto the sentinel-classified row");
    let client2 = connect_client(&sup2).await;
    assert!(
        matches!(
            listed(&client2, &session.id).await.status,
            SessionStatus::Error { .. }
        ),
        "sanity: the reload really did classify this session as error"
    );

    let tmux_name = format!("fh-{}", session.id);
    let before = tmux_query(&sock, &["has-session", "-t", &format!("={tmux_name}")]).await;
    assert!(
        before.status.success(),
        "the real tmux session must still exist going into this test's real assertions"
    );

    // Attach succeeds: the dead pane genuinely exists, so this is exactly
    // like attaching to any other exited session, not a `NotFound`.
    client2
        .attach(&session.id, 80, 24)
        .await
        .expect("an error-classified session with a real dead pane must still be attachable");

    client2.delete_session(&session.id).await.expect("delete");
    let after = tmux_query(&sock, &["has-session", "-t", &format!("={tmux_name}")]).await;
    assert!(
        !after.status.success(),
        "delete must tear down the REAL tmux session, not merely drop the row — a session \
         reload never recorded a terminal for has nothing for the kill sweep to find"
    );
}

/// Review-swarm fix batch item 2: a launch sentinel this supervisor CANNOT
/// durably record (its boot-id read failed, so `may_record()` is false for
/// this instance's whole lifetime) must still surface as `error` in a
/// `ListSessions` reply — undurably — rather than silently reporting the
/// stale `Exited` a degraded pass used to fall back to by skipping the
/// sentinel read entirely. Once a LATER supervisor's boot id read succeeds,
/// the same classification lands durably, with the file consumed.
#[tokio::test]
async fn a_sentinel_survives_an_unreadable_boot_id_undurably_then_commits_once_readable() {
    let h = harness_believing_boot("boot-a").await;
    let sock = h.state.path().join("tmux.sock");
    let work = tempfile::tempdir().expect("workdir");
    let missing_binary = work.path().join("no-such-farhelm-agent");

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &missing_binary.to_string_lossy(),
            None,
            80,
            24,
        )
        .await
        .expect("create a session whose invocation cannot exec");
    wait_for_dead_pane(&sock, &format!("fh-{}", session.id)).await;

    let status_path = status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    assert!(
        status_path.exists(),
        "the shim must have left its sentinel behind by now"
    );

    // A supervisor whose boot-id read fails outright: `may_record()` is
    // false for its whole lifetime (no reboot-vs-same-boot decision could
    // even be made), yet this is exactly the "degraded boot-id" case item
    // 2 requires the list path to still READ the sentinel for.
    let degraded = supervisor_reading_boot(h.state.path(), Err("/proc is unavailable")).await;
    let degraded_client = connect_client(&degraded).await;
    let undurable = listed(&degraded_client, &session.id).await;
    assert!(
        matches!(undurable.status, SessionStatus::Error { .. }),
        "a degraded supervisor must still SURFACE a sentinel it read, even though it cannot \
         record it: {:?}",
        undurable.status
    );
    assert!(
        status_path.exists(),
        "an undurable classification must retain the sentinel file for a later pass to \
         commit against"
    );

    // A later supervisor whose boot-id read succeeds: the same
    // classification now lands durably, and the file is consumed.
    let recovered = supervisor_believing_boot(h.state.path(), Some("boot-a")).await;
    let recovered_client = connect_client(&recovered).await;
    assert!(
        matches!(
            listed(&recovered_client, &session.id).await.status,
            SessionStatus::Error { .. }
        ),
        "the classification must also land once recording becomes possible"
    );
    assert!(
        !status_path.exists(),
        "once durably committed, the sentinel file must finally be consumed"
    );
}

/// Review-swarm fix batch item 3(a): `StopSession`'s own dead/absent-pane
/// exit-recording boundary must check the sentinel FIRST — the failure
/// this pins is a stop committing a plain `ObservedExit` before anything
/// ever reads the sentinel, which terminal-stickiness would then protect
/// forever, permanently hiding an `Error` classification the file already
/// had evidence for. Stop is called BEFORE any list, deliberately: a list
/// is how this supervisor would ordinarily witness the sentinel first, and
/// this test is specifically about the row still being `Running` (no
/// intervening observer at all) when the stop lands.
#[tokio::test]
async fn stop_before_any_list_on_an_exec_failed_session_still_ends_error() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let work = tempfile::tempdir().expect("workdir");
    let missing_binary = work.path().join("no-such-farhelm-agent");

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &missing_binary.to_string_lossy(),
            None,
            80,
            24,
        )
        .await
        .expect("create a session whose invocation cannot exec");
    wait_for_dead_pane(&sock, &format!("fh-{}", session.id)).await;

    // No `list_sessions` call anywhere above: `stop_session` is the FIRST
    // and only observer this row ever gets before its own exit-recording
    // boundary runs.
    h.client
        .stop_session(&session.id)
        .await
        .expect("stop a session whose agent already never started");

    let status = listed(&h.client, &session.id).await.status;
    assert!(
        matches!(status, SessionStatus::Error { .. }),
        "a stop landing before any list must still end error, not a plain exit: {status:?}"
    );
}

/// Review-swarm fix batch item 1: a corrupt (invalid-UTF-8) sentinel must
/// fail the WHOLE `ListSessions` request rather than silently classifying
/// its row (or any other entry sharing the reply) from pane state alone.
/// Pinned against the list path specifically, since that is the site the
/// fix batch calls out as returning `Internal` for the request; the file
/// must survive the failed attempt, and a later pass with it repaired
/// (removed, here) must classify correctly.
#[tokio::test]
async fn a_corrupt_sentinel_fails_the_whole_list_request_and_survives() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    assert_eq!(
        listed(&h.client, &session.id).await.status,
        SessionStatus::Alive
    );

    // A genuinely alive pane never has its sentinel checked at all (the
    // dead-or-absent gate), so the pane is killed first — this is the
    // absent-terminal half of the gate, exercised deliberately rather than
    // the live half (covered by the dead-or-absent test elsewhere).
    let sock = h.state.path().join("tmux.sock");
    let out = tmux_query(
        &sock,
        &["kill-session", "-t", &format!("=fh-{}", session.id)],
    )
    .await;
    assert!(out.status.success(), "test setup: killing the tmux session");

    let status_path = status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    std::fs::create_dir_all(status_path.parent().unwrap()).unwrap();
    std::fs::write(&status_path, [0xff, 0xfe, 0xfd]).expect("plant a corrupt sentinel");

    let err = h
        .client
        .list_sessions()
        .await
        .expect_err("a corrupt sentinel must fail the whole list request");
    assert!(
        format!("{err:#}").contains("launch sentinel"),
        "the failure must name what went wrong: {err:#}"
    );
    assert!(
        status_path.exists(),
        "the corrupt file must survive a failed read for a later, repaired pass"
    );

    // Repaired (by removing the corrupt file outright): a later list must
    // classify normally rather than staying wedged on the earlier failure.
    std::fs::remove_file(&status_path).expect("remove the corrupt sentinel");
    let status = listed(&h.client, &session.id).await.status;
    assert_eq!(
        status,
        SessionStatus::Exited { exit_code: None },
        "once repaired, a later pass must classify normally again: {status:?}"
    );
}

/// Review-swarm fix batch item 13: the dead-or-absent gate itself, both
/// directions in one test. A genuinely ALIVE pane must never have its
/// sentinel even READ (`Alive` wins outright, and the planted file is left
/// untouched) — planting a sentinel behind a still-running agent is
/// exactly the scenario that must NOT retroactively classify it error.
/// Once the pane goes dead, the SAME planted file is read on the very next
/// list and classifies error.
#[tokio::test]
async fn the_dead_or_absent_gate_ignores_a_sentinel_behind_a_live_pane_until_the_pane_dies() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    assert_eq!(
        listed(&h.client, &session.id).await.status,
        SessionStatus::Alive
    );

    let detail = "exec_failed argv0=/nope errno=2".to_string();
    let status_path = status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    std::fs::write(&status_path, &detail).expect("plant a sentinel behind a live pane");

    assert_eq!(
        listed(&h.client, &session.id).await.status,
        SessionStatus::Alive,
        "a live pane must win outright; its sentinel is not even consulted"
    );
    assert!(
        status_path.exists(),
        "an unconsulted sentinel must be left completely untouched"
    );

    // Kill the real agent so the pane goes dead; the SAME file is what the
    // next list reads.
    let sock = h.state.path().join("tmux.sock");
    let out = tmux_query(
        &sock,
        &["kill-session", "-t", &format!("=fh-{}", session.id)],
    )
    .await;
    assert!(out.status.success(), "test setup: killing the tmux session");

    let status = listed(&h.client, &session.id).await.status;
    assert_eq!(
        status,
        SessionStatus::Error { detail },
        "once the pane is dead-or-absent, the same planted sentinel classifies error: {status:?}"
    );
}

// ---------------------------------------------------------------------
// PLAN_M3.md item 6 / acceptance 7: server-enforced create idempotency.
//
// "The reply was dropped" is simulated throughout by simply calling
// create again with the same key: from the supervisor's side a retried
// create and a create whose reply never arrived are the same request, and
// nothing about the guarantee depends on how the first answer was lost.
// The crash-stage tests below are the cases where that is NOT enough —
// there the first attempt has to genuinely die partway through, which is
// what `crash_at` provides.
//
// Every test that "restarts" a supervisor does so through
// `handoff_to_new_supervisor`, which releases the predecessor's state-dir
// claim first. That is not tidiness: a second supervisor constructed while
// the first still holds the claim runs READ-ONLY (no reconciliation, no
// settlement), so a handoff test that skipped it would exercise a path
// production never takes.
// ---------------------------------------------------------------------

/// Create with an intent key through the real client, for the tests that
/// only vary the key and the client.
async fn create_keyed(
    client: &SupervisorClient,
    work: &std::path::Path,
    key: &str,
) -> anyhow::Result<SessionInfo> {
    client
        .create_session_with_key(
            &work.to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            Some("keyed".to_string()),
            80,
            24,
            Some(key.to_string()),
        )
        .await
}

/// How many tmux sessions this state directory's private server holds —
/// the check that actually proves "no second process", since a duplicate
/// create leaves a duplicate agent behind whether or not anything lists it.
async fn tmux_session_count(sock: &std::path::Path) -> usize {
    tmux_session_names(sock).await.len()
}

/// The `fh-*` tmux session names this server holds.
async fn tmux_session_names(sock: &std::path::Path) -> Vec<String> {
    let out = tmux_query(sock, &["list-sessions", "-F", "#{session_name}"]).await;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|line| line.starts_with("fh-"))
        .map(str::to_string)
        .collect()
}

/// The supervisor's own view of a session's durable row, read through a
/// SECOND store handle on the same database.
///
/// Reading the database directly is the point: these tests assert on what
/// a crash left DURABLY, which the session list deliberately does not
/// expose (it reports classified status, not stored state), and doing it
/// through the real `SessionStore` keeps the assertions in the same
/// vocabulary the supervisor uses rather than in raw SQL. `may_migrate:
/// false` because the supervisor under test owns that right.
async fn stored_reservation(state: &std::path::Path, key: &str) -> Option<Reservation> {
    let store = SessionStore::open(&state.join("supervisor.db"), false)
        .await
        .expect("open the database a second time");
    store.reservation(key).await.expect("read the reservation")
}

/// Every session row the database holds, in id order.
async fn stored_sessions(state: &std::path::Path) -> Vec<StoredSession> {
    let store = SessionStore::open(&state.join("supervisor.db"), false)
        .await
        .expect("open the database a second time");
    let mut rows = store.load_all().await.expect("load");
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

/// Retire `sup` (and the client whose connection task holds its own
/// reference) and construct its replacement, which must genuinely OWN the
/// state directory.
///
/// The wait is the whole point: `StateDirOwnership` releases the `flock`
/// when the LAST `Supervisor` for the directory drops, and a connection
/// task holds an `Arc` of its own for as long as its pipe is open. Without
/// draining that first, the replacement is constructed alongside a live
/// predecessor and silently starts read-only — which would make every
/// assertion below about reload's reconciliation vacuous.
async fn handoff_to_new_supervisor(
    state: &std::path::Path,
    sup: Arc<Supervisor>,
    client: Arc<SupervisorClient>,
) -> Arc<Supervisor> {
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup) > 1 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the old supervisor's connection tasks never released it"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);
    let replacement = Supervisor::new_with_exe(state, farhelm_bin().into())
        .await
        .expect("the restarted supervisor");
    assert!(
        replacement.owns_state_dir(),
        "the replacement must hold the state directory's claim, or it reconciles nothing \
         and this test would pass for the wrong reason"
    );
    replacement
}

/// M3 acceptance 7's first clause: a create whose reply is lost AFTER the
/// session durably exists, retried with the same key, returns the SAME
/// session and starts no second process — including when the supervisor
/// restarts between the two attempts.
///
/// The restart half is not a bonus: it is the case a purely in-memory
/// dedup table would pass the first half of and fail here, and it is
/// exactly the shape of a real ambiguous failure (the supervisor being
/// restarted or crashing is one of the reasons a reply goes missing in the
/// first place).
#[tokio::test]
async fn a_retried_create_returns_the_same_session_across_a_supervisor_restart() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let work = tempfile::tempdir().expect("workdir");
    let sup1 = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    let client1 = connect_client(&sup1).await;

    let first = create_keyed(&client1, work.path(), "intent-1")
        .await
        .expect("create");
    let replay = create_keyed(&client1, work.path(), "intent-1")
        .await
        .expect("a retry of the same intent must succeed, not conflict with itself");
    assert_eq!(
        replay.id, first.id,
        "the retry must replay the session the first attempt created"
    );

    let sup2 = handoff_to_new_supervisor(state.path(), sup1, client1).await;
    let client2 = connect_client(&sup2).await;
    let after_restart = create_keyed(&client2, work.path(), "intent-1")
        .await
        .expect("the retry must still replay after a restart");
    assert_eq!(
        after_restart.id, first.id,
        "the reservation is durable, so a restart cannot forget which session an intent made"
    );
    // Dimensions are deliberately NOT part of the fingerprint — they shape
    // the attachment, not the session — so the same intent retried from a
    // differently-sized client is still the same intent.
    let resized = client2
        .create_session_with_key(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            Some("keyed".to_string()),
            132,
            50,
            Some("intent-1".to_string()),
        )
        .await
        .expect("a retry from a different-sized client is the same intent");
    assert_eq!(resized.id, first.id);

    let listing = client2.list_sessions().await.expect("list");
    assert_eq!(
        listing.sessions.len(),
        1,
        "four creates of one intent must leave exactly one session: {:?}",
        listing.sessions
    );
    assert_eq!(
        tmux_session_count(&state.path().join("tmux.sock")).await,
        1,
        "and exactly one agent process behind it"
    );
    drop(slot);
}

/// The other direction of the same key: reused for a DIFFERENT request, it
/// is refused rather than merged.
///
/// A reused key is a client bug — two distinct intents cannot share one
/// identity — and answering with the first intent's session would hand the
/// caller a session it never asked for while silently discarding the one it
/// did. `Conflict` is the classification the helm renders as a 409.
#[tokio::test]
async fn a_key_reused_for_a_different_request_is_refused() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let other = tempfile::tempdir().expect("other workdir");
    let first = create_keyed(&h.client, work.path(), "intent-2")
        .await
        .expect("create");

    let error = create_keyed(&h.client, other.path(), "intent-2")
        .await
        .expect_err("a different cwd under the same key must be refused");
    let downcast = error
        .chain()
        .find_map(|c| c.downcast_ref::<SupervisorError>())
        .expect("the refusal must carry a classified kind");
    assert_eq!(downcast.kind, ErrorKind::Conflict);
    assert!(
        downcast.message.contains("intent-2"),
        "the refusal must name the key: {}",
        downcast.message
    );

    let listing = h.client.list_sessions().await.expect("list");
    assert_eq!(
        listing.sessions.len(),
        1,
        "the refused request must not have created anything: {:?}",
        listing.sessions
    );
    assert_eq!(listing.sessions[0].id, first.id);
}

/// Concurrent creates sharing one intent key collapse to ONE launch, and
/// both callers get the same session.
///
/// The barrier is what makes this a real concurrency test rather than two
/// requests that happened to be issued close together: the first create is
/// HELD inside its launch (through the create-lifecycle seam) until the
/// second has demonstrably been issued, so the two genuinely overlap. A
/// plain `join!` proves nothing — the runtime is free to run them one after
/// the other, and usually does.
///
/// Two connections, not one: the supervisor handles each connection's
/// control messages in a single serial read loop, so two creates on one
/// connection would serialize before ever reaching the idempotency
/// machinery.
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_creates_under_one_intent_key_yield_one_session() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let work = tempfile::tempdir().expect("workdir");
    // Released by the test once the second create is in flight; the first
    // create parks inside its launch until then.
    let (release, held) = std::sync::mpsc::channel::<()>();
    let held = std::sync::Mutex::new(held);
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            create_crash: Some(Arc::new(move |stage| {
                if stage == CreateStage::DuringLaunch {
                    // Blocking, not awaiting: the seam is synchronous, and
                    // `block_in_place` is what keeps the rest of the
                    // runtime — including the second create — running.
                    tokio::task::block_in_place(|| {
                        let held = held.lock().expect("barrier mutex");
                        let _ = held.recv_timeout(Duration::from_secs(30));
                    });
                }
                Ok(())
            })),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let first_client = connect_client(&sup).await;
    let second_client = connect_client(&sup).await;

    let held_create = {
        let client = Arc::clone(&first_client);
        let work = work.path().to_path_buf();
        tokio::spawn(async move { create_keyed(&client, &work, "intent-3").await })
    };
    // The second create is issued while the first is parked in its launch.
    let racing_create = {
        let client = Arc::clone(&second_client);
        let work = work.path().to_path_buf();
        tokio::spawn(async move { create_keyed(&client, &work, "intent-3").await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !held_create.is_finished() && !racing_create.is_finished(),
        "test fixture: both creates must still be in flight when the barrier releases"
    );
    let _ = release.send(());

    let a = held_create.await.expect("join").expect("first create");
    let b = racing_create.await.expect("join").expect("second create");
    assert_eq!(
        a.id, b.id,
        "both callers must be answered with the same session"
    );

    let listing = first_client.list_sessions().await.expect("list");
    assert_eq!(
        listing.sessions.len(),
        1,
        "one intent, one session, however many requests raced: {:?}",
        listing.sessions
    );
    assert_eq!(
        tmux_session_count(&state.path().join("tmux.sock")).await,
        1,
        "and one agent process — a second launch is the failure this collapses"
    );
    drop(slot);
}

/// A replay whose session was since DELETED returns an explicit
/// gone-error: never a live-looking success carrying a dead id, and never
/// a fresh duplicate.
///
/// Both wrong answers are worth naming. Handing back the deleted id would
/// have the client attach to nothing; creating a second session would
/// resurrect work the user explicitly threw away, under a key they have no
/// reason to think is still live. The honest answer is that the key is
/// spent, and the message says which session it was spent on.
#[tokio::test]
async fn a_replay_for_a_deleted_session_reports_that_it_is_gone() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = create_keyed(&h.client, work.path(), "intent-4")
        .await
        .expect("create");
    h.client
        .delete_session(&session.id)
        .await
        .expect("delete the session the intent created");

    let error = create_keyed(&h.client, work.path(), "intent-4")
        .await
        .expect_err("a replay for a deleted session must not succeed");
    let downcast = error
        .chain()
        .find_map(|c| c.downcast_ref::<SupervisorError>())
        .expect("the gone-error must carry a classified kind");
    assert_eq!(
        downcast.kind,
        ErrorKind::Conflict,
        "nothing is missing — the KEY is spent, which is the same rule a reuse gets"
    );
    assert!(
        downcast.message.contains(&session.id) && downcast.message.contains("deleted"),
        "the error must name the session and what happened to it: {}",
        downcast.message
    );
    assert!(
        h.client
            .list_sessions()
            .await
            .expect("list")
            .sessions
            .is_empty(),
        "and it must not have created a replacement"
    );
}

/// The durable state a crash at `stage` leaves behind, as the next
/// supervisor's reload finds it.
struct CrashScene {
    /// The session row the interrupted attempt left, if any.
    row: Option<StoredSession>,
    /// tmux session names alive at the moment of inspection.
    tmux: Vec<String>,
    /// The reservation, after the replacement supervisor's reload has run
    /// but BEFORE any retry — so reload's own settlement pass is visible
    /// rather than masked by the retry's reconciliation.
    reservation_after_reload: Reservation,
}

/// Crash the first attempt at `stage`, inspect what it left, hand the
/// state directory to a fresh supervisor, inspect again, then retry — and
/// assert what every stage must equally produce: exactly one session,
/// exactly one live agent, and a usable answer.
///
/// The per-stage evidence is returned rather than asserted here because it
/// is the one thing that DIFFERS: the shared assertions below would pass
/// even if two stages left identical state, which would mean the seam was
/// not injecting where it claims to.
async fn retry_after_a_crash_at(stage: CreateStage) -> CrashScene {
    let state = tempfile::tempdir().expect("tempdir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let sock = state.path().join("tmux.sock");
    let work = tempfile::tempdir().expect("workdir");
    let sup1 = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            create_crash: Some(crash_at(stage)),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let client1 = connect_client(&sup1).await;
    create_keyed(&client1, work.path(), "intent-crash")
        .await
        .expect_err("the injected crash must fail the create");

    // What the crash left, before anything reconciles it.
    let rows = stored_sessions(state.path()).await;
    assert!(rows.len() <= 1, "one create can leave at most one row");
    let scene_row = rows.into_iter().next();
    let scene_tmux = tmux_session_names(&sock).await;

    // The retry arrives at a supervisor that never saw the first attempt
    // in memory — the only honest way to model a crash, and what makes
    // reload's own reconciliation load-bearing rather than incidental.
    let sup2 = handoff_to_new_supervisor(state.path(), sup1, client1).await;
    let reservation_after_reload = stored_reservation(state.path(), "intent-crash")
        .await
        .expect("the reservation outlives the crash");
    let client2 = connect_client(&sup2).await;

    let stranded: Vec<String> = client2
        .list_sessions()
        .await
        .expect("list what the crash left")
        .sessions
        .into_iter()
        .map(|s| s.id)
        .collect();
    let session = create_keyed(&client2, work.path(), "intent-crash")
        .await
        .expect("the retry must produce a session");
    assert_eq!(
        stranded,
        vec![session.id.clone()],
        "the retry must land on the identity the reservation already assigned — replaying it \
         or relaunching under it, never minting a second one beside it"
    );

    let listing = client2.list_sessions().await.expect("list");
    assert_eq!(
        listing.sessions.len(),
        1,
        "a crash at {stage:?} then a retry must leave exactly one session: {:?}",
        listing.sessions
    );
    assert_eq!(listing.sessions[0].id, session.id);
    assert_eq!(
        tmux_session_names(&sock).await,
        vec![format!("fh-{}", session.id)],
        "tmux must hold exactly the session the retry handed back"
    );
    assert_eq!(
        listed(&client2, &session.id).await.status,
        SessionStatus::Alive,
        "and the agent must actually be running in it"
    );
    // The retried session is a normal session: deleting it tears down
    // everything the retry built (the other half of item 6's
    // retry-versus-delete ordering — when the retry wins, the delete that
    // follows cleans up its work).
    client2
        .delete_session(&session.id)
        .await
        .expect("delete the retried session");
    assert!(
        tmux_session_names(&sock).await.is_empty(),
        "the delete must have torn down the relaunched agent too"
    );

    CrashScene {
        row: scene_row,
        tmux: scene_tmux,
        reservation_after_reload,
    }
}

/// Crash after the reservation and its launching row commit, before any
/// side effect: the retry PERFORMS the create, under the identities the
/// reservation already carries.
///
/// The distinguishing fact is that nothing was ever launched, so a retry
/// that replayed instead of launching would hand back a session id with no
/// agent behind it — which is why "absent side effects means redo" is a
/// real branch and not an optimization. The reservation must therefore
/// still be PENDING after reload: settling it as created would mean
/// claiming a session that never existed.
#[tokio::test]
async fn a_crash_after_the_reservation_lets_the_retry_perform_the_create() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let scene = retry_after_a_crash_at(CreateStage::AfterRecord).await;
    assert_eq!(
        scene
            .row
            .expect("the launching row is the evidence")
            .outcome,
        LastOutcome::Launching
    );
    assert!(
        scene.tmux.is_empty(),
        "the crash landed before any side effect, so tmux must hold nothing: {:?}",
        scene.tmux
    );
    assert_eq!(
        scene.reservation_after_reload.outcome,
        ReservationOutcome::Pending,
        "reload must not settle an intent whose launch left no trace"
    );
    drop(slot);
}

/// Crash between tmux having the session and the launch being confirmed
/// durably: the tmux session and its pane exist under a row that still
/// says `Launching`.
///
/// The retry must not launch a second one. What makes that work is the
/// unification item 6 asks for — reload rediscovers the pane and confirms
/// the row, and the reservation settles from that same verdict instead of
/// probing tmux a second time and risking a different answer. Settled by
/// RELOAD, before any retry arrives, which is what the reservation
/// assertion pins.
#[tokio::test]
async fn a_crash_during_the_launch_reconciles_to_the_session_that_started() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let scene = retry_after_a_crash_at(CreateStage::DuringLaunch).await;
    let row = scene.row.expect("the launching row survives");
    assert_eq!(row.outcome, LastOutcome::Launching);
    assert!(
        row.pane.is_empty(),
        "the confirmation never ran, so no pane was recorded"
    );
    assert_eq!(
        scene.tmux,
        vec![format!("fh-{}", row.id)],
        "but tmux really does hold the session"
    );
    assert_eq!(
        scene.reservation_after_reload.outcome,
        ReservationOutcome::Created,
        "reload's own pass must settle this intent, with no retry involved"
    );
    drop(slot);
}

/// Crash after the launch is confirmed and before the reservation's
/// outcome is recorded — acceptance 7's "the reply is dropped AFTER the
/// session durably exists", from the inside.
///
/// This is the window a reservation alone could never cover: the session
/// is complete and only the intent table does not know it, which is
/// exactly why the lifecycle is a state machine with a reconciling PENDING
/// state rather than one durable "this key is spent" flag.
#[tokio::test]
async fn a_crash_before_the_outcome_commit_still_replays_the_same_session() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let scene = retry_after_a_crash_at(CreateStage::BeforeOutcome).await;
    let row = scene.row.expect("the confirmed row survives");
    assert_eq!(
        row.outcome,
        LastOutcome::Running,
        "the launch was confirmed durably before the crash"
    );
    assert!(!row.pane.is_empty(), "with its pane recorded alongside");
    assert_eq!(scene.tmux, vec![format!("fh-{}", row.id)]);
    assert_eq!(
        scene.reservation_after_reload.outcome,
        ReservationOutcome::Created,
        "reload settles the intent whose session is demonstrably complete"
    );
    drop(slot);
}

/// PLAN_M3.md item 6 meeting item 2's reboot conversion: a create that
/// crashed BEFORE reaching tmux, then survived a reboot, must not be
/// replayed as though it had succeeded.
///
/// The reboot converts its launching row to `interrupted`, which looks
/// terminal — and settling a reservation from a terminal status alone
/// would record "created" for a session that never launched and can never
/// run, permanently. Provenance, not status, is what the settlement needs:
/// this row never had a pane, so nothing ever saw it in tmux.
///
/// The retry must therefore still perform the create, under the same
/// identity, exactly as it would have without the reboot.
#[tokio::test]
async fn a_reboot_does_not_turn_a_never_launched_intent_into_a_created_one() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let guard = TmuxServerGuard(state.path().join("tmux.sock"));
    let work = tempfile::tempdir().expect("workdir");
    let sup1 = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            boot_id: Arc::new(|| Ok(Some("boot-a".to_string()))),
            create_crash: Some(crash_at(CreateStage::AfterRecord)),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let client1 = connect_client(&sup1).await;
    create_keyed(&client1, work.path(), "intent-reboot")
        .await
        .expect_err("the injected crash must fail the create");

    // A reboot: a different boot id AND no surviving tmux server.
    drop(client1);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup1) > 1 {
        assert!(tokio::time::Instant::now() < deadline, "connection drain");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup1);
    // Dropping the guard kills the private tmux server, which is the other
    // half of a reboot: a boot-id change alone would leave live panes for
    // the reload to find, which is not a reboot at all. The replacement
    // guard covers the server the next supervisor starts.
    drop(guard);
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));

    let sup2 = supervisor_believing_boot(state.path(), Some("boot-b")).await;
    assert!(
        sup2.owns_state_dir(),
        "the replacement must own the state dir"
    );
    let client2 = connect_client(&sup2).await;
    assert_eq!(
        listed(&client2, &stored_sessions(state.path()).await[0].id)
            .await
            .status,
        SessionStatus::Interrupted,
        "test fixture: the reboot must have converted the stranded row"
    );
    assert_eq!(
        stored_reservation(state.path(), "intent-reboot")
            .await
            .expect("the reservation survives")
            .outcome,
        ReservationOutcome::Pending,
        "an interrupted row with no pane is no evidence a launch ever happened"
    );

    let session = create_keyed(&client2, work.path(), "intent-reboot")
        .await
        .expect("the retry must perform the create the crash never did");
    assert_eq!(
        listed(&client2, &session.id).await.status,
        SessionStatus::Alive,
        "and the session it hands back must be a real, running one — not the interrupted \
         placeholder the reboot left"
    );
    assert_eq!(
        stored_sessions(state.path()).await.len(),
        1,
        "still one session for one intent"
    );
    drop(slot);
}

// ---------------------------------------------------------------------
// Conversation-identity capture (PLAN_M3.md item 8, acceptance 8)
//
// Every test below drives a REAL supervisor against the record-writing
// fake agent, because the properties under test are about the interaction
// of three things a unit test cannot put in one room: when the supervisor
// confirms delivery of its first input, when the agent writes its record,
// and what the rescan concludes from the two. The per-kind parsing, the
// scan's completeness and budgets, and the pure correlation rules are
// unit-tested in farhelm-supervisor's `agent_kind` module; what is here is
// the wiring and the claim discipline built on top of it.
// ---------------------------------------------------------------------

/// The capture window every test in this section runs with.
///
/// Short deliberately, and each part matters. `AFTER` bounds how long
/// after first input a record may appear and still be attributed, so it is
/// also what two sessions in one directory must be spaced by to avoid
/// poisoning each other. `GRACE` is how long past the window's close the
/// supervisor waits before the one COMPLETE scan that may commit — every
/// test that expects a durable claim has to outlast it, so a production
/// value would put minutes on the clock. `BEFORE` only absorbs clock
/// granularity between the supervisor's reading and the agent's.
const TEST_CAPTURE_BEFORE: Duration = Duration::from_secs(1);
const TEST_CAPTURE_AFTER: Duration = Duration::from_secs(2);
const TEST_CAPTURE_GRACE: Duration = Duration::from_secs(1);

/// The bounds every capture harness injects.
fn test_capture_bounds() -> CaptureWindowBounds {
    CaptureWindowBounds::new(TEST_CAPTURE_BEFORE, TEST_CAPTURE_AFTER, TEST_CAPTURE_GRACE)
}

/// Everything a capture test needs beyond the harness itself: the private
/// agent home the supervisor observes and the fixture writes into, and a
/// directory of kind-named symlinks to the farhelm binary.
///
/// The symlinks are what let these tests exercise DERIVATION rather than
/// routing around it. A session launched as `farhelm internal fake-agent
/// ...` has basename `farhelm` and correctly classifies as generic, so
/// running the fixture through `<bin>/claude` is the only way to reach the
/// integrated path the way a real user does — and it simultaneously pins
/// PLAN_M3.md item 7's other promise, that the default resume template is
/// built from the ORIGINAL first token (this absolute path) rather than
/// from a bare command name. The binary is multi-call by SUBCOMMAND, not
/// by argv0, so it behaves identically under either name.
struct CaptureFixtures {
    home: tempfile::TempDir,
    bin: tempfile::TempDir,
}

/// A harness whose supervisor observes a private agent home, with the
/// short capture window above.
async fn capture_harness() -> (Harness, CaptureFixtures) {
    capture_harness_with_fault(None).await
}

/// [`capture_harness`] with a durable-write fault injected, for the
/// pending-durability tests.
async fn capture_harness_with_fault(
    fault: Option<CaptureStoreFault>,
) -> (Harness, CaptureFixtures) {
    let home = tempfile::tempdir().expect("agent home");
    let bin = tempfile::tempdir().expect("agent bin");
    for kind in ["claude", "codex"] {
        std::os::unix::fs::symlink(farhelm_bin(), bin.path().join(kind))
            .expect("symlink the farhelm binary under an agent's own name");
    }
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            agent_home: Some(home.path().to_path_buf()),
            capture_window: test_capture_bounds(),
            capture_store_fault: fault,
            scopes: Arc::new(farhelm_supervisor::scope::ScopeManager::disabled()),
            ..SupervisorSeams::default()
        },
    )
    .await;
    (h, CaptureFixtures { home, bin })
}

/// Create a session running the record-writing fake agent for `kind`
/// (`claude` or `codex`) in `cwd`, launched through the kind-named symlink
/// so the supervisor derives the integration itself.
async fn record_session(
    h: &Harness,
    fixtures: &CaptureFixtures,
    cwd: &std::path::Path,
    kind: &str,
) -> SessionInfo {
    let invocation = format!(
        "{} internal fake-agent --script {kind}-record --record-home {}",
        shell_words::quote(&fixtures.bin.path().join(kind).to_string_lossy()),
        shell_words::quote(&fixtures.home.path().to_string_lossy())
    );
    h.client
        .create_session(&cwd.to_string_lossy(), &invocation, None, 80, 24)
        .await
        .expect("create a record-writing session")
}

/// Attach, wait for the fixture to be listening, send one line, and wait
/// for the record it writes in response — the shape of "the agent's first
/// prompt", which is the only moment a record can appear.
///
/// Returns the conversation id the fixture reported, so a test can assert
/// the supervisor captured THAT id rather than merely some id.
async fn provoke_record(h: &Harness, session: &SessionInfo) -> (u32, TermStream, Vec<u8>, String) {
    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, b"first prompt\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-WRITTEN:", 20).await;
    let id = marker_value(&seen, "RECORD-WRITTEN:");
    (chan, rx, seen, id)
}

/// The value the fixture printed after `marker`, up to the line ending.
///
/// The fixture's markers are its own contract with these tests (the same
/// discipline `FAKE-AGENT READY` established), and reading the id back out
/// is what lets a test assert the supervisor captured the RIGHT
/// conversation rather than just any one — the property that separates
/// this feature working from it appearing to.
fn marker_value(transcript: &[u8], marker: &str) -> String {
    let text = String::from_utf8_lossy(transcript);
    let start = text
        .find(marker)
        .unwrap_or_else(|| panic!("no {marker} in transcript:\n{text}"))
        + marker.len();
    text[start..]
        .chars()
        .take_while(|c| !c.is_whitespace())
        .collect()
}

/// The value after the LAST occurrence of `marker`, for transcripts that
/// span a restart: a reattached client's replay carries the previous run's
/// markers too, so "the first one" is the wrong run's answer whenever a
/// terminal was reused.
fn last_marker_value(transcript: &[u8], marker: &str) -> String {
    let text = String::from_utf8_lossy(transcript);
    let start = text
        .rfind(marker)
        .unwrap_or_else(|| panic!("no {marker} in transcript:\n{text}"))
        + marker.len();
    text[start..]
        .chars()
        .take_while(|c| !c.is_whitespace())
        .collect()
}

/// This session's durable snapshot, as the supervisor would answer a
/// restart.
async fn snapshot_of(h: &Harness, session_id: &str) -> SessionSnapshot {
    h.sup
        .session_snapshot(session_id)
        .await
        .expect("reading the snapshot")
        .expect("the session exists")
}

/// Poll until this session's durable first-input time is recorded, and
/// return it.
///
/// Every window assertion below is arithmetic on THIS value rather than on
/// wall-clock sleeps, because the correlator is truncated to whole seconds:
/// a 3.5-second sleep can produce a 3-second separation, which would
/// silently break a disjointness premise a sleep-based test only *assumes*.
/// Waiting on the recorded value lets the premise be asserted instead.
async fn wait_for_first_input(h: &Harness, session_id: &str, secs: u64) -> i64 {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(at) = snapshot_of(h, session_id).await.first_input_at {
            return at;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "session {session_id} never recorded a durable first-input time within {secs}s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Sleep until a first input taken NOW would own a window disjoint from
/// the one anchored at `earlier`.
///
/// Disjointness is `t2 - before > t1 + after`, so this waits past
/// `t1 + after + before` with a whole second of margin for the truncation
/// on both readings. The caller still asserts the premise afterwards — this
/// only makes the assertion likely to hold rather than assuming it does.
async fn wait_until_window_disjoint_from(earlier: i64) {
    let target =
        earlier + TEST_CAPTURE_AFTER.as_secs() as i64 + TEST_CAPTURE_BEFORE.as_secs() as i64 + 1;
    while farhelm_supervisor::agent_kind::now_unix() <= target {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Assert that two sessions' capture windows really are disjoint, from the
/// DURABLE first-input times rather than from how long the test slept.
///
/// Without this the two-sessions-in-one-directory tests would silently
/// become vacuous the day a slow machine (or a second-boundary) shrank the
/// separation below the window: both sessions would bail, and "each
/// captured its own" would fail for a reason that looks like a capture bug.
fn assert_windows_disjoint(first: i64, second: i64) {
    let bounds = test_capture_bounds();
    let a = CaptureWindow::around(first, bounds);
    let b = CaptureWindow::around(second, bounds);
    assert!(
        !a.overlaps(&b),
        "this test's premise is that these windows do not overlap, but {a:?} meets {b:?}"
    );
}

/// Assert the opposite premise, for the ambiguity tests: the two windows
/// really do overlap, so a bail is the correct answer rather than an
/// accident of timing.
fn assert_windows_overlap(first: i64, second: i64) {
    let bounds = test_capture_bounds();
    let a = CaptureWindow::around(first, bounds);
    let b = CaptureWindow::around(second, bounds);
    assert!(
        a.overlaps(&b),
        "this test's premise is that these windows overlap, but {a:?} misses {b:?}"
    );
}

/// Poll until this session's stored snapshot reports a captured identity.
///
/// Polling because capture rides the list/reload cadence rather than an
/// event: nothing pushes, so a test must ask. `list_sessions` is what
/// drives the pass, so it is called each round rather than only reading the
/// store — a test that read the store alone would hang forever waiting for
/// a pass nothing triggered. The wait has to outlast the publication grace,
/// since nothing is committed until the horizon closes.
async fn wait_for_capture(h: &Harness, session_id: &str, secs: u64) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        h.client.list_sessions().await.expect("list drives capture");
        if let Some(conversation) = snapshot_of(h, session_id).await.captured_conversation {
            return conversation;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "session {session_id} never captured a conversation identity within {secs}s"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Drive list passes until every session's horizon has closed and then
/// some, for the tests asserting a capture must NOT happen.
///
/// Sleeping past the horizon is what makes this real negative evidence: a
/// session still inside its window is only ever `Provisional` anyway, so
/// asserting "nothing captured" before the horizon would pass on a broken
/// implementation too.
async fn settle_past_horizon(h: &Harness) {
    let deadline = tokio::time::Instant::now()
        + TEST_CAPTURE_AFTER
        + TEST_CAPTURE_GRACE
        + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        h.client.list_sessions().await.expect("list drives capture");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    // A few more passes with the clock already past every horizon, so the
    // final complete scan has certainly run.
    for _ in 0..3 {
        h.client.list_sessions().await.expect("list drives capture");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// SPEC.md's per-session resume promise, at its hardest: two sessions in
/// ONE working directory each capture their OWN conversation, and each
/// resumes exactly that one.
///
/// This is the case the whole correlation design exists for — "even when
/// several sessions share a working directory" is SPEC.md's own wording —
/// and it is where a naive implementation (take the newest record in the
/// project directory) silently hands both sessions the same conversation.
/// The inputs are spaced past the capture window so the two windows are
/// disjoint, and that premise is ASSERTED from the durable first-input
/// times rather than assumed from how long the test slept.
///
/// The filled resume argv is asserted, not just the id: SPEC.md's promise
/// is that restart resumes that conversation, and an id captured into a
/// template that never gets filled would satisfy the letter of a weaker
/// test while failing the actual promise.
#[tokio::test]
async fn two_claude_sessions_in_one_directory_each_capture_their_own_conversation() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");

    let first = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_chan_a, _rx_a, _seen_a, id_a) = provoke_record(&h, &first).await;
    let at_a = wait_for_first_input(&h, &first.id, 20).await;

    wait_until_window_disjoint_from(at_a).await;

    let second = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_chan_b, _rx_b, _seen_b, id_b) = provoke_record(&h, &second).await;
    let at_b = wait_for_first_input(&h, &second.id, 20).await;
    assert_ne!(id_a, id_b, "the fixture must mint distinct conversations");
    assert_windows_disjoint(at_a, at_b);

    assert_eq!(wait_for_capture(&h, &first.id, 30).await, id_a);
    assert_eq!(wait_for_capture(&h, &second.id, 30).await, id_b);

    for (session, conversation) in [(&first, &id_a), (&second, &id_b)] {
        let snapshot = snapshot_of(&h, &session.id).await;
        assert_eq!(snapshot.restart_offer, farhelm_proto::RestartOffer::Resume);
        assert_eq!(
            snapshot.resume_argv.as_deref().unwrap().last().unwrap(),
            conversation,
            "the resume template must be filled with THIS session's conversation"
        );
        assert_eq!(
            listed(&h.client, &session.id).await.restart_offer,
            farhelm_proto::RestartOffer::Resume,
            "the offer must reach the wire, not only the store"
        );
    }
}

/// SPEC.md requires BOTH integrations in v1, so Codex gets the same
/// shared-directory proof rather than being assumed to follow from
/// Claude's. It is not a formality: Codex's records live in a date-nested
/// tree that is NOT partitioned by working directory at all, so the
/// recorded-cwd filter carries all the weight here, and the scan cache is
/// keyed on a root every Codex session on the host shares.
#[tokio::test]
async fn two_codex_sessions_in_one_directory_each_capture_their_own_conversation() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");

    let first = record_session(&h, &fixtures, work.path(), "codex").await;
    let (_chan_a, _rx_a, _seen_a, id_a) = provoke_record(&h, &first).await;
    let at_a = wait_for_first_input(&h, &first.id, 20).await;

    wait_until_window_disjoint_from(at_a).await;

    let second = record_session(&h, &fixtures, work.path(), "codex").await;
    let (_chan_b, _rx_b, _seen_b, id_b) = provoke_record(&h, &second).await;
    let at_b = wait_for_first_input(&h, &second.id, 20).await;
    assert_ne!(id_a, id_b);
    assert_windows_disjoint(at_a, at_b);

    assert_eq!(wait_for_capture(&h, &first.id, 30).await, id_a);
    assert_eq!(wait_for_capture(&h, &second.id, 30).await, id_b);

    let snapshot = snapshot_of(&h, &first.id).await;
    let template = snapshot.resume_template.as_deref().unwrap();
    assert_eq!(
        snapshot.resume_argv.as_deref().unwrap(),
        // The audited codex shape: a subcommand, not a flag.
        [template[0].clone(), "resume".to_string(), id_a.clone()]
    );
}

/// Two sessions of DIFFERENT kinds in one working directory must not
/// poison each other, even with overlapping windows: a Claude record can
/// only ever be a Claude session's, so the ambiguity rule is scoped to the
/// kind as well as the directory. Without that scoping the natural
/// implementation (group by directory) would make a mixed pair — which is
/// an ordinary thing for a user to do — permanently uncapturable.
#[tokio::test]
async fn a_claude_and_a_codex_session_in_one_directory_do_not_poison_each_other() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");

    let claude = record_session(&h, &fixtures, work.path(), "claude").await;
    let codex = record_session(&h, &fixtures, work.path(), "codex").await;
    let (_c1, _r1, _s1, id_claude) = provoke_record(&h, &claude).await;
    let (_c2, _r2, _s2, id_codex) = provoke_record(&h, &codex).await;
    let at_claude = wait_for_first_input(&h, &claude.id, 20).await;
    let at_codex = wait_for_first_input(&h, &codex.id, 20).await;
    assert_windows_overlap(at_claude, at_codex);

    assert_eq!(wait_for_capture(&h, &claude.id, 30).await, id_claude);
    assert_eq!(wait_for_capture(&h, &codex.id, 30).await, id_codex);
}

/// The audited constraint that shapes the entire correlator: the record
/// appears at first PROMPT submission, not at launch, and the gap between
/// them is unbounded. So a session left sitting well past every window
/// constant in the code must still capture the moment its user finally
/// types — there is no deadline running from creation, and this test fails
/// loudly if one is ever introduced.
///
/// The idle period is longer than the window AND the publication grace
/// together, which is the whole span any timeout-shaped implementation
/// could plausibly have used; list passes run throughout, so such an
/// implementation would have settled the session `UncapturedFinal` before
/// the prompt ever arrived.
#[tokio::test]
async fn a_first_prompt_delayed_past_every_window_constant_still_captures() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;

    let idle =
        TEST_CAPTURE_BEFORE + TEST_CAPTURE_AFTER + TEST_CAPTURE_GRACE + Duration::from_secs(2);
    let deadline = tokio::time::Instant::now() + idle;
    while tokio::time::Instant::now() < deadline {
        h.client.list_sessions().await.expect("list");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly,
        "nothing may be claimed before the agent has written anything"
    );
    assert_eq!(
        snapshot_of(&h, &session.id).await.first_input_at,
        None,
        "and no correlator clock may have started either"
    );

    let (_chan, _rx, _seen, id) = provoke_record(&h, &session).await;
    assert_eq!(wait_for_capture(&h, &session.id, 30).await, id);
}

/// The munged-cwd collision, end to end through two real sessions.
///
/// `a.b` and `a-b` munge to the SAME Claude project directory, so both
/// sessions' records land side by side in one place. Only the recorded
/// `cwd` FIELD can tell them apart, which is exactly why SPEC_impl.md
/// records the munging as non-injective. Their first inputs are close
/// together on purpose: if directory membership were doing the work, the
/// two would look like a shared-directory collision and BOTH would bail —
/// so a passing test proves the field filter ran before the ambiguity rule
/// ever had anything to complain about.
#[tokio::test]
async fn two_directories_that_munge_alike_are_separated_by_the_recorded_cwd() {
    let (h, fixtures) = capture_harness().await;
    let parent = tempfile::tempdir().expect("workdir");
    let dotted = parent.path().join("a.b");
    let dashed = parent.path().join("a-b");
    std::fs::create_dir(&dotted).expect("mkdir a.b");
    std::fs::create_dir(&dashed).expect("mkdir a-b");
    assert_eq!(
        farhelm_supervisor::agent_kind::munge_cwd(
            &std::fs::canonicalize(&dotted).unwrap().to_string_lossy()
        ),
        farhelm_supervisor::agent_kind::munge_cwd(
            &std::fs::canonicalize(&dashed).unwrap().to_string_lossy()
        ),
        "the premise of this test is that these two collide"
    );

    let one = record_session(&h, &fixtures, &dotted, "claude").await;
    let two = record_session(&h, &fixtures, &dashed, "claude").await;
    let (_c1, _r1, _s1, id_one) = provoke_record(&h, &one).await;
    let (_c2, _r2, _s2, id_two) = provoke_record(&h, &two).await;

    assert_eq!(wait_for_capture(&h, &one.id, 30).await, id_one);
    assert_eq!(wait_for_capture(&h, &two.id, 30).await, id_two);
}

/// Correlation uses the CANONICAL working directory, not the spelling the
/// caller sent, because the agent records its own `getcwd()` — which the
/// kernel has already resolved. A session created through a symlink, or
/// with a dot component, or with a trailing slash, must therefore still
/// find its own records; without the resolution its munged directory name
/// and its recorded-cwd comparison would both miss, and capture would
/// simply never happen for anyone whose path was not already canonical.
#[tokio::test]
async fn a_symlinked_or_dotted_working_directory_still_correlates() {
    let (h, fixtures) = capture_harness().await;
    let parent = tempfile::tempdir().expect("workdir");
    let real = parent.path().join("real");
    std::fs::create_dir(&real).expect("mkdir");
    let link = parent.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    let canonical = std::fs::canonicalize(&real).expect("canonicalize");

    // Through the symlink, with a dot component and a trailing slash for
    // good measure — three different ways of naming the same directory.
    let spelled = link.join(".").join("");
    let session = record_session(&h, &fixtures, &spelled, "claude").await;
    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.canonical_cwd.as_deref(),
        Some(canonical.to_string_lossy().as_ref()),
        "the resolved spelling is what correlation must use"
    );

    let (_chan, _rx, _seen, id) = provoke_record(&h, &session).await;
    assert_eq!(wait_for_capture(&h, &session.id, 30).await, id);
}

/// Codex gets the same canonical-cwd proof as Claude, because the two
/// consume it differently: Claude uses it to build the project DIRECTORY
/// name, while Codex has no per-directory tree at all and uses it only for
/// the recorded-field comparison. A fix that resolved the path for one
/// path and not the other would pass a Claude-only test.
#[tokio::test]
async fn a_symlinked_working_directory_still_correlates_for_codex() {
    let (h, fixtures) = capture_harness().await;
    let parent = tempfile::tempdir().expect("workdir");
    let real = parent.path().join("real");
    std::fs::create_dir(&real).expect("mkdir");
    let link = parent.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");

    let session = record_session(&h, &fixtures, &link, "codex").await;
    let (_chan, _rx, _seen, id) = provoke_record(&h, &session).await;
    assert_eq!(wait_for_capture(&h, &session.id, 30).await, id);
}

/// The claim discipline's central rule: nothing is made durable while the
/// window is still open, so a rival record arriving LATE inside the window
/// flips a provisional match to ambiguous instead of finding an identity
/// already committed.
///
/// The rival is planted directly rather than launched as a second session,
/// which is the sharper test: a second session would ALSO be caught by the
/// overlapping-windows rule, so this would pass even with the record-level
/// rule removed. A bare file in the same project directory, carrying the
/// same recorded cwd and a timestamp inside the window, can only be caught
/// by re-deriving the verdict from scratch on every pass — which is
/// exactly what the provisional state exists to make happen.
#[tokio::test]
async fn a_rival_record_arriving_late_in_the_window_flips_a_provisional_claim() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_chan, _rx, _seen, _id) = provoke_record(&h, &session).await;
    let at = wait_for_first_input(&h, &session.id, 20).await;

    // One pass with only the real record present: the match exists, but it
    // is provisional, so nothing may be stored yet.
    h.client.list_sessions().await.expect("list drives capture");
    assert_eq!(
        snapshot_of(&h, &session.id).await.captured_conversation,
        None,
        "a match inside an open window must not be committed"
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly,
        "nor advertised: Resume promises a stored identity a restart can fill in"
    );

    // Now a second record for the same directory, timestamped inside the
    // same window — the shape another agent running here would produce.
    let canonical = std::fs::canonicalize(work.path()).expect("canonicalize");
    let project = fixtures.home.path().join(".claude").join("projects").join(
        farhelm_supervisor::agent_kind::munge_cwd(&canonical.to_string_lossy()),
    );
    let rival_line = serde_json::json!({
        "type": "user",
        "sessionId": "planted-rival-conversation",
        "cwd": canonical.to_string_lossy(),
        "timestamp": farhelm_supervisor::agent_kind::format_rfc3339(at),
    });
    std::fs::write(
        project.join("planted-rival.jsonl"),
        format!("{rival_line}\n"),
    )
    .expect("plant a rival record");

    settle_past_horizon(&h).await;
    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.captured_conversation, None,
        "the late rival makes the correlation ambiguous, so nothing is claimed"
    );
    assert!(
        snapshot.capture_ambiguous,
        "and the refusal is recorded durably, not merely inferred each pass"
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly
    );
}

/// A plain resume APPENDS to the existing record under the same id
/// (audited), so the watcher must treat an append as a confirmation rather
/// than as a new conversation — and an explicit fork, which writes a NEW
/// id, must not displace the identity already claimed.
///
/// Both halves are in one test because the second is only meaningful after
/// the first: the fork is written into the same directory the append just
/// touched, so a rescan that re-derived identity from "whatever is in this
/// directory now" would find two records and either bail or switch. The
/// captured identity must simply stay put — and the stored STAMP must
/// advance, which is what proves the re-verification actually re-read the
/// file rather than skipping it.
#[tokio::test]
async fn an_append_re_verifies_the_identity_and_a_fork_never_displaces_it() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;
    let (chan, mut rx, mut seen, id) = provoke_record(&h, &session).await;
    assert_eq!(wait_for_capture(&h, &session.id, 30).await, id);

    let canonical = std::fs::canonicalize(work.path()).expect("canonicalize");
    let record = fixtures
        .home
        .path()
        .join(".claude")
        .join("projects")
        .join(farhelm_supervisor::agent_kind::munge_cwd(
            &canonical.to_string_lossy(),
        ))
        .join(format!("{id}.jsonl"));
    let before = std::fs::metadata(&record).expect("the record exists").len();

    h.client.send_input(chan, b"append\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-APPENDED:", 20).await;
    // The append must be observable as a size change, or re-verification
    // is legitimately entitled to skip the re-read and this test would
    // assert nothing.
    let after = std::fs::metadata(&record).expect("the record exists").len();
    assert!(
        after > before,
        "the fixture's append must actually grow the record ({before} -> {after})"
    );

    for _ in 0..3 {
        h.client.list_sessions().await.expect("list drives capture");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        snapshot_of(&h, &session.id).await.captured_conversation,
        Some(id.clone()),
        "an append confirms the identity; it must not duplicate or replace it"
    );

    h.client.send_input(chan, b"fork\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-FORKED:", 20).await;
    let forked = marker_value(&seen, "RECORD-FORKED:");
    assert_ne!(forked, id, "a fork is a genuinely different conversation");
    settle_past_horizon(&h).await;
    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.captured_conversation.as_deref(),
        Some(id.as_str()),
        "the ORIGINAL conversation is this session's; the fork belongs to another"
    );
    assert_eq!(
        snapshot.resume_argv.as_deref().unwrap().last().unwrap(),
        &id
    );
}

/// The ambiguity bail, which is the mechanical form of SPEC.md's
/// never-silently-resume-the-wrong-conversation rule.
///
/// Two sessions launched near-simultaneously in one working directory have
/// overlapping windows — asserted from their durable first-input times, not
/// assumed from their ordering — so a record landing in the shared span
/// could honestly belong to either. Neither is captured, the refusal is
/// durable, and both keep offering the honest fresh launch.
///
/// The sticky half is what the second stage pins: DELETING the rival's
/// evidence entirely must not let the survivor change its mind. A pass
/// that re-derived the verdict from what is on disk right now would see
/// one clean candidate and claim it — on strictly worse evidence than the
/// pass that bailed.
#[tokio::test]
async fn two_near_simultaneous_sessions_in_one_directory_stay_uncaptured() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");

    let first = record_session(&h, &fixtures, work.path(), "claude").await;
    let second = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_c1, _r1, _s1, id_first) = provoke_record(&h, &first).await;
    let (_c2, _r2, _s2, _id_second) = provoke_record(&h, &second).await;
    let at_first = wait_for_first_input(&h, &first.id, 20).await;
    let at_second = wait_for_first_input(&h, &second.id, 20).await;
    assert_windows_overlap(at_first, at_second);

    settle_past_horizon(&h).await;
    for session in [&first, &second] {
        let snapshot = snapshot_of(&h, &session.id).await;
        assert_eq!(
            snapshot.captured_conversation, None,
            "an ambiguous correlation must claim nothing at all"
        );
        assert!(snapshot.capture_ambiguous, "and must record the refusal");
        assert_eq!(snapshot.resume_argv, None);
        assert_eq!(
            listed(&h.client, &session.id).await.restart_offer,
            farhelm_proto::RestartOffer::FreshOnly,
            "restart must offer the honest fallback, never a guessed resume"
        );
    }

    // Remove the SECOND session's record, leaving the first's alone in the
    // directory. A rescan that re-decided from present evidence would now
    // find exactly one candidate and claim it.
    let canonical = std::fs::canonicalize(work.path()).expect("canonicalize");
    let project = fixtures.home.path().join(".claude").join("projects").join(
        farhelm_supervisor::agent_kind::munge_cwd(&canonical.to_string_lossy()),
    );
    for entry in std::fs::read_dir(&project).expect("project dir") {
        let entry = entry.expect("dir entry");
        if !entry.file_name().to_string_lossy().contains(&id_first) {
            std::fs::remove_file(entry.path()).expect("remove the rival's record");
        }
    }
    settle_past_horizon(&h).await;
    assert_eq!(
        snapshot_of(&h, &first.id).await.captured_conversation,
        None,
        "an ambiguity does not become less ambiguous because its evidence was tidied away"
    );
}

/// The snapshot is immutable and the captured identity is durable, both
/// across a supervisor restart — which is the only reason capture is worth
/// doing at all, since SPEC.md's resume offer exists precisely for the
/// sessions that outlived their supervisor.
///
/// The capture is deliberately provoked at RELOAD rather than by a list
/// before the shutdown: nothing calls `list_sessions` on the first
/// supervisor, so the identity this test finds afterwards can only have
/// been claimed by the successor's own reload pass. That is the path a
/// real restart takes — a session whose agent wrote its record while the
/// supervisor was down — and it is the one a list-driven test would never
/// exercise. Only the DURABLE first-input time is polled for, because that
/// is the fact the successor needs to correlate at all.
#[tokio::test]
async fn a_capture_missed_while_the_supervisor_was_down_lands_on_reload() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_chan, _rx, _seen, id) = provoke_record(&h, &session).await;
    let at = wait_for_first_input(&h, &session.id, 20).await;

    // Past the horizon, so the successor's very first pass is allowed to
    // commit — but with no list on THIS supervisor, so nothing here can.
    while farhelm_supervisor::agent_kind::now_unix()
        <= at + (TEST_CAPTURE_AFTER + TEST_CAPTURE_GRACE).as_secs() as i64
    {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        snapshot_of(&h, &session.id).await.captured_conversation,
        None,
        "nothing drove a pass on this supervisor, so nothing can have been claimed"
    );

    // Release the first supervisor before constructing its replacement: an
    // overlapping successor starts read-only and reconciles nothing, so a
    // test that skipped this would exercise a path production never takes
    // (see `Supervisor::owns_state_dir`).
    // `_tmux` LAST, and that is not cosmetic: destructuring rebinds these
    // fields as ordinary locals, which drop in reverse declaration order —
    // so listing the guard before `state` would delete the state tempdir
    // (and with it the socket the guard kills through) before the guard
    // ever ran, leaking the tmux server. That leak was real and measured;
    // see `TmuxServerGuard`'s docs.
    let Harness {
        client,
        sup,
        state,
        _tmux,
        _slot,
    } = h;
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup) > 1 {
        assert!(tokio::time::Instant::now() < deadline, "connection drain");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);

    let restarted = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            agent_home: Some(fixtures.home.path().to_path_buf()),
            capture_window: test_capture_bounds(),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("restarted supervisor");
    assert!(
        restarted.owns_state_dir(),
        "the predecessor must be gone, or this proves nothing"
    );
    let after = restarted
        .session_snapshot(&session.id)
        .await
        .expect("snapshot")
        .expect("present");
    assert_eq!(
        after.captured_conversation.as_deref(),
        Some(id.as_str()),
        "the successor's own reload pass is what captured this"
    );
    assert_eq!(after.restart_offer, farhelm_proto::RestartOffer::Resume);
    assert_eq!(after.kind, farhelm_proto::AgentKind::Claude);
    assert_eq!(
        after.resume_argv.as_deref().unwrap().last().unwrap(),
        &id,
        "and the snapshot it fills is the immutable one from create"
    );
    drop(_slot);
}

/// An ambiguity verdict survives a restart, and that durability is
/// load-bearing rather than tidy: after a restart the rival's evidence may
/// be gone (its session deleted, its record cleaned up), so a successor
/// that re-derived the verdict from what is on disk would see one clean
/// candidate and claim it — resuming a conversation the first supervisor
/// had already established it could not attribute.
#[tokio::test]
async fn an_ambiguity_survives_a_restart_even_when_its_evidence_does_not() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let first = record_session(&h, &fixtures, work.path(), "claude").await;
    let second = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_c1, _r1, _s1, id_first) = provoke_record(&h, &first).await;
    let (_c2, _r2, _s2, _id_second) = provoke_record(&h, &second).await;
    settle_past_horizon(&h).await;
    assert!(snapshot_of(&h, &first.id).await.capture_ambiguous);

    // `_tmux` LAST, and that is not cosmetic: destructuring rebinds these
    // fields as ordinary locals, which drop in reverse declaration order —
    // so listing the guard before `state` would delete the state tempdir
    // (and with it the socket the guard kills through) before the guard
    // ever ran, leaking the tmux server. That leak was real and measured;
    // see `TmuxServerGuard`'s docs.
    let Harness {
        client,
        sup,
        state,
        _tmux,
        _slot,
    } = h;
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup) > 1 {
        assert!(tokio::time::Instant::now() < deadline, "connection drain");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);

    // The rival's record is gone by the time the successor looks.
    let canonical = std::fs::canonicalize(work.path()).expect("canonicalize");
    let project = fixtures.home.path().join(".claude").join("projects").join(
        farhelm_supervisor::agent_kind::munge_cwd(&canonical.to_string_lossy()),
    );
    for entry in std::fs::read_dir(&project).expect("project dir") {
        let entry = entry.expect("dir entry");
        if !entry.file_name().to_string_lossy().contains(&id_first) {
            std::fs::remove_file(entry.path()).expect("remove the rival's record");
        }
    }

    let restarted = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            agent_home: Some(fixtures.home.path().to_path_buf()),
            capture_window: test_capture_bounds(),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("restarted supervisor");
    let after = restarted
        .session_snapshot(&first.id)
        .await
        .expect("snapshot")
        .expect("present");
    assert!(after.capture_ambiguous, "the refusal survived");
    assert_eq!(
        after.captured_conversation, None,
        "and still refuses, even though only one candidate remains on disk"
    );
    assert_eq!(after.restart_offer, farhelm_proto::RestartOffer::FreshOnly);
    drop(_slot);
}

/// A durable write that FAILS must never yield a session that advertises
/// `Resume`: the offer promises a stored identity a restart can fill in,
/// and there is none. The retry then has to ride the polling cadence, not
/// the input path — so clearing the fault and polling again is what lands
/// the claim.
///
/// The same shape covers the first-input write, whose failure is quieter
/// and worse: correlation still works for this process, but a restart
/// would lose the anchor entirely, so the retry is the only thing that
/// makes capture survivable across the restart it exists for.
#[tokio::test]
async fn a_failed_durable_write_never_advertises_resume_and_is_retried() {
    let failing = Arc::new(AtomicBool::new(true));
    let armed = Arc::clone(&failing);
    let fault: CaptureStoreFault = Arc::new(move |_write, _session| {
        if armed.load(Ordering::SeqCst) {
            anyhow::bail!("injected capture-write failure")
        }
        Ok(())
    });
    let (h, fixtures) = capture_harness_with_fault(Some(fault)).await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_chan, _rx, _seen, id) = provoke_record(&h, &session).await;

    settle_past_horizon(&h).await;
    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.first_input_at, None,
        "the first-input write was refused, so nothing is stored"
    );
    assert_eq!(
        snapshot.captured_conversation, None,
        "and no identity may be committed either"
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly,
        "a claim this process holds but could not store must not advertise Resume"
    );

    // The retry rides the poll, not the input path: nothing more is typed.
    failing.store(false, Ordering::SeqCst);
    assert_eq!(wait_for_capture(&h, &session.id, 30).await, id);
    assert!(
        snapshot_of(&h, &session.id).await.first_input_at.is_some(),
        "the first-input write is retried on the same cadence"
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::Resume
    );
}

/// Input that never reaches the pane must not start the correlator's
/// clock. An empty data frame is the case that actually occurs (a client
/// flushing nothing), and starting the window on it would anchor the
/// session before its user has typed — narrowing, and possibly missing,
/// the window the real prompt lands in.
#[tokio::test]
async fn an_empty_input_frame_never_starts_the_correlator() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, Vec::new()).await;
    for _ in 0..5 {
        h.client.list_sessions().await.expect("list");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        snapshot_of(&h, &session.id).await.first_input_at,
        None,
        "nothing reached the pane, so nothing may have anchored the window"
    );

    // A real byte does anchor it, which is what makes the assertion above
    // about emptiness rather than about the hook never running.
    h.client.send_input(chan, b"first prompt\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-WRITTEN:", 20).await;
    wait_for_first_input(&h, &session.id, 20).await;
}

/// Capture iterates EVERY session, not the capped subset `ListSessions`
/// replies with: the ambiguity rule is a statement about all sessions
/// sharing a working directory, so a session beyond the reply cap must
/// still poison a window it occupies. Otherwise a busy host would turn a
/// bail into a wrong capture — the one outcome this design exists to
/// exclude — and would do it only under load, which is the worst possible
/// way to find out.
///
/// The extra sessions are inserted straight into the store and brought
/// into memory by a restart, because what is under test is the pass's
/// iteration over the session map, not five hundred tmux panes. The
/// poisoning rival is one of them: it is a Claude session in the same
/// canonical directory whose first input lands inside the real session's
/// window, and it exists only as a row.
#[tokio::test]
async fn capture_considers_sessions_beyond_the_list_reply_cap() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let canonical = std::fs::canonicalize(work.path())
        .expect("canonicalize")
        .to_string_lossy()
        .into_owned();

    let session = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_chan, _rx, _seen, _id) = provoke_record(&h, &session).await;
    let at = wait_for_first_input(&h, &session.id, 20).await;

    // `_tmux` LAST, and that is not cosmetic: destructuring rebinds these
    // fields as ordinary locals, which drop in reverse declaration order —
    // so listing the guard before `state` would delete the state tempdir
    // (and with it the socket the guard kills through) before the guard
    // ever ran, leaking the tmux server. That leak was real and measured;
    // see `TmuxServerGuard`'s docs.
    let Harness {
        client,
        sup,
        state,
        _tmux,
        _slot,
    } = h;
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup) > 1 {
        assert!(tokio::time::Instant::now() < deadline, "connection drain");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);

    let store = SessionStore::open(&state.path().join("supervisor.db"), false)
        .await
        .expect("open the store directly");
    for i in 0..=LIST_SESSION_CAP {
        // The last row is the rival: same kind, same canonical directory,
        // and a first input inside the real session's window.
        let rival = i == LIST_SESSION_CAP;
        store
            .insert_session(
                StoredSession {
                    id: format!("extra-{i}"),
                    title: format!("extra-{i}"),
                    cwd: work.path().to_string_lossy().into_owned(),
                    invocation: "agent".to_string(),
                    tmux_name: format!("fh-extra-{i}"),
                    pane: String::new(),
                    outcome: LastOutcome::Exited {
                        exit_code: Some(0),
                        annotation: None,
                    },
                    agent_kind: if rival {
                        farhelm_proto::AgentKind::Claude
                    } else {
                        farhelm_proto::AgentKind::Generic
                    },
                    resume_template: rival.then(|| {
                        vec![
                            "claude".to_string(),
                            "--resume".to_string(),
                            "{conversation}".to_string(),
                        ]
                    }),
                    canonical_cwd: rival.then(|| canonical.clone()),
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: rival.then_some(at),
                    generation: 0,
                    launch_scoped: false,
                },
                None,
            )
            .await
            .expect("insert an extra session row");
    }
    drop(store);

    let restarted = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            agent_home: Some(fixtures.home.path().to_path_buf()),
            capture_window: test_capture_bounds(),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("restarted supervisor");
    let client = connect_client(&restarted).await;
    let listing = client.list_sessions().await.expect("list");
    assert!(
        listing.total > LIST_SESSION_CAP as u64,
        "this test's premise is that there are more sessions than the reply cap"
    );
    for _ in 0..5 {
        client.list_sessions().await.expect("list drives capture");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        restarted
            .session_snapshot(&session.id)
            .await
            .expect("snapshot")
            .expect("present")
            .capture_ambiguous,
        "a rival beyond the reply cap must still poison this session's window"
    );
    drop(_slot);
}

/// A session whose kind basename recognition would miss (`env claude`, a
/// wrapper) still captures once the caller says what it is — the reason
/// PLAN_M3.md item 7 carries explicit overrides at all. And a
/// placeholder-free template on a NON-integrated kind is the fallback shape
/// SPEC.md describes, which must reach the wire as `FallbackTemplate`
/// rather than being flattened into a fresh launch.
///
/// All three are asserted here because they are the same override slot, and
/// because none has a UI caller — the API and these tests are the only
/// consumers until M5's profiles, so an untested override is an unexercised
/// one.
#[tokio::test]
async fn an_overridden_kind_captures_and_a_generic_fallback_template_is_offered() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");

    // `farhelm internal fake-agent ...`: basename `farhelm`, so derivation
    // says generic. The override is what makes it claude.
    let invocation = agent_cmd(&format!(
        "internal fake-agent --script claude-record --record-home {}",
        shell_words::quote(&fixtures.home.path().to_string_lossy())
    ));
    let overridden = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            &invocation,
            None,
            80,
            24,
            farhelm_helm::CreateExtras {
                agent_kind: Some(farhelm_proto::AgentKind::Claude),
                resume_template: Some(vec![
                    "my-wrapper".to_string(),
                    "--resume".to_string(),
                    "{conversation}".to_string(),
                ]),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create with overrides");
    let (_chan, _rx, _seen, id) = provoke_record(&h, &overridden).await;
    assert_eq!(wait_for_capture(&h, &overridden.id, 30).await, id);
    assert_eq!(
        snapshot_of(&h, &overridden.id)
            .await
            .resume_argv
            .as_deref()
            .unwrap(),
        ["my-wrapper", "--resume", &id]
    );

    // A generic session with a verbatim, placeholder-free resume
    // invocation: nothing to capture, but a real fallback to offer.
    let fallback = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
            farhelm_helm::CreateExtras {
                resume_template: Some(vec!["some-agent".to_string(), "--continue".to_string()]),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create with a fallback template");
    assert_eq!(
        fallback.restart_offer,
        farhelm_proto::RestartOffer::FallbackTemplate,
        "the create reply already knows this session has a fallback"
    );
    assert_eq!(
        listed(&h.client, &fallback.id).await.restart_offer,
        farhelm_proto::RestartOffer::FallbackTemplate
    );

    // ...and the invariant that keeps the promise honest: an INTEGRATED
    // kind may not carry a placeholder-free template, because once capture
    // succeeded such a template could only discard the identity.
    let refused = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
            farhelm_helm::CreateExtras {
                agent_kind: Some(farhelm_proto::AgentKind::Codex),
                resume_template: Some(vec!["codex".to_string(), "resume".to_string()]),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect_err("a placeholder-free template on an integrated kind is refused");
    assert!(
        format!("{refused:#}").contains("{conversation}"),
        "the refusal must name what is missing: {refused:#}"
    );
}

/// A keyed create REPLAYED after its session captured must report the
/// capture, not the create-time placeholder: the replay is "the same
/// answer to the same request", and the honest answer to "what would
/// restart do for this session" changes the moment an identity is claimed.
/// A replay frozen at create time would tell a retrying client `FreshOnly`
/// for a session that can in fact resume.
#[tokio::test]
async fn a_keyed_replay_after_capture_reports_the_resume_offer() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let invocation = format!(
        "{} internal fake-agent --script claude-record --record-home {}",
        shell_words::quote(&fixtures.bin.path().join("claude").to_string_lossy()),
        shell_words::quote(&fixtures.home.path().to_string_lossy())
    );
    let created = h
        .client
        .create_session_with_key(
            &work.path().to_string_lossy(),
            &invocation,
            None,
            80,
            24,
            Some("intent-capture-replay".to_string()),
        )
        .await
        .expect("create");
    assert_eq!(
        created.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly
    );

    let (_chan, _rx, _seen, id) = provoke_record(&h, &created).await;
    assert_eq!(wait_for_capture(&h, &created.id, 30).await, id);

    let replayed = h
        .client
        .create_session_with_key(
            &work.path().to_string_lossy(),
            &invocation,
            None,
            80,
            24,
            Some("intent-capture-replay".to_string()),
        )
        .await
        .expect("replay");
    assert_eq!(replayed.id, created.id, "still one session for one intent");
    assert_eq!(
        replayed.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "the replay reports what restart would do NOW, not at create time"
    );
}

/// PLAN_M3.md's recorded M3 limitation, pinned so it cannot be lost: a
/// session whose own invocation resumes an existing conversation appends to
/// a record whose header timestamp predates its window, so nothing is
/// captured and the honest fresh-launch fallback is offered.
///
/// The shape is reproduced by planting an OLD record for this working
/// directory and running an integrated session that writes none of its
/// own. That is exactly what `claude --resume <id>` looks like from the
/// outside, and pinning it here is what makes a future change that starts
/// correlating on appends a deliberate decision rather than an accident —
/// see the plan for why that correlation is not free.
#[tokio::test]
async fn a_session_resuming_an_old_conversation_is_not_captured() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let canonical = std::fs::canonicalize(work.path()).expect("canonicalize");
    let project = fixtures.home.path().join(".claude").join("projects").join(
        farhelm_supervisor::agent_kind::munge_cwd(&canonical.to_string_lossy()),
    );
    std::fs::create_dir_all(&project).expect("project dir");
    let old = serde_json::json!({
        "type": "user",
        "sessionId": "a-conversation-from-last-week",
        "cwd": canonical.to_string_lossy(),
        "timestamp": farhelm_supervisor::agent_kind::format_rfc3339(
            farhelm_supervisor::agent_kind::now_unix() - 7 * 24 * 3600,
        ),
    });
    std::fs::write(project.join("old.jsonl"), format!("{old}\n")).expect("plant an old record");

    // An integrated session that writes no record of its own — the `basic`
    // script under the `claude` name, which is what a resume looks like
    // from the supervisor's side.
    let invocation = format!(
        "{} internal fake-agent --script basic",
        shell_words::quote(&fixtures.bin.path().join("claude").to_string_lossy())
    );
    let session = h
        .client
        .create_session(&work.path().to_string_lossy(), &invocation, None, 80, 24)
        .await
        .expect("create");
    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, b"hello\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "echo:", 20).await;
    wait_for_first_input(&h, &session.id, 20).await;

    settle_past_horizon(&h).await;
    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.kind,
        farhelm_proto::AgentKind::Claude,
        "the session IS integrated; the limitation is about correlation, not derivation"
    );
    assert_eq!(
        snapshot.captured_conversation, None,
        "the record's header predates the window, so it is not a candidate"
    );
    assert!(
        !snapshot.capture_ambiguous,
        "and this is a clean miss, not an ambiguity"
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly
    );
}

// ---------------------------------------------------------------------
// Real-agent capture (PLAN_M3.md acceptance 8's second half)
//
// `#[ignore]`-marked, because they need vendor credentials and network
// access CI does not have and must never depend on. The fixture tests
// above are what keep the LOGIC honest in CI; these are what keep the
// AUDITED FACTS honest — that the record really does appear at first
// prompt submission, that its correlator fields really are named what
// this build reads, and that the resume template really does fill into a
// command the vendor accepts. Nothing but a real agent can tell us that a
// version bump changed one of them, and the failure mode if one did is
// silent: capture would simply stop happening.
//
// Run them individually and deliberately:
//
//     cargo test -p farhelm --test e2e -- --ignored --test-threads 1 \
//         real_claude_session_captures_its_conversation_identity
//
// Record the run and its result with the milestone; a green fixture suite
// is not a substitute.
// ---------------------------------------------------------------------

/// The shared body of the two real-agent tests: launch `agent` for real in
/// a scratch directory, submit one prompt, and require the supervisor to
/// have captured a conversation identity that fills the resume template.
///
/// Observes the user's REAL home rather than a fixture tree — that is the
/// point, since the whole question is where the vendor actually writes and
/// what it writes there — and therefore uses the production capture window
/// and publication grace: a shortened one would make a slow first response
/// look like a missing record and turn a real regression into a flake, or
/// vice versa. The poll deadline is correspondingly generous, since nothing
/// may be committed until a full minute past first input.
///
/// The prompt is chosen to be answerable without tools and cheap to serve;
/// nothing asserts anything about the ANSWER, only that submitting one
/// caused a record this build can correlate.
///
/// Both agents were run for real on 2026-07-31 and both passed; the run
/// records, and codex's upstream trust-dialog limitation, are in
/// PLAN_M3.md's testing-decisions section.
async fn real_agent_captures_its_conversation(
    ready_marker: &str,
    trust_dialog_markers: &[&str],
    // Given the scratch working directory, produce the home the supervisor
    // should observe, the agent command to launch, and any tempdir that must
    // outlive the run. Claude observes the user's real home directly; codex
    // needs a synthesized one (see its test for why), and this seam is what
    // lets one helper serve both without either knowing the other's needs.
    prepare: impl FnOnce(&std::path::Path) -> (std::path::PathBuf, String, Option<tempfile::TempDir>),
) {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let work = tempfile::tempdir().expect("workdir");
    let (agent_home, agent, _agent_home_guard) = prepare(work.path());
    let agent = agent.as_str();
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            agent_home: Some(agent_home),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let client = connect_client(&sup).await;

    let session = client
        .create_session(&work.path().to_string_lossy(), agent, None, 100, 30)
        .await
        .unwrap_or_else(|e| panic!("launching the real {agent}: {e:#}"));

    let (chan, mut rx) = client.attach(&session.id, 100, 30).await.expect("attach");
    let mut seen = Vec::new();
    // A fresh scratch directory is an UNTRUSTED workspace, and a modern
    // agent blocks on its own folder-trust dialog before it will accept a
    // prompt. Accepting it here is not a workaround: farhelm passes the
    // vendor's terminal through untouched and never configures an agent
    // (SPEC.md), so a real user meets this same dialog and presses enter.
    // The test simulates only that human half.
    //
    // Two orderings are load-bearing, both learned by running this for
    // real against Claude Code v2.1.220:
    //
    // 1. Dialog markers are checked BEFORE the ready marker, and a ready
    //    marker must never be a substring of any dialog text. The first
    //    real run matched "Claude Code" against the dialog's own body
    //    ("Claude Code'll be able to read, edit, and execute files here"),
    //    broke the wait, and typed the prompt into an unaccepted modal —
    //    no conversation was ever started and capture correctly found
    //    nothing. Hence "Claude Code v", which only the banner carries.
    // 2. Nothing slow may sit between accepting the dialog and submitting
    //    the prompt: that enter IS the session's first input byte, so it
    //    anchors the capture window. The slack for a human composing
    //    afterwards is exactly what `CAPTURE_WINDOW_AFTER` is sized for
    //    (see its docs, which name this dialog).
    //
    // Matching is against the RENDERED pane, not the raw stream: a TUI's
    // first paint arrives as cursor-positioned fragments that the raw
    // transcript shows as bare line endings.
    //
    // Accepted side effect: accepting trust writes the (soon-deleted)
    // scratch path into the user's real agent config. That is the vendor's
    // own write, and the same class of consequence this test already
    // embraces by observing the real HOME.
    let sock = state.path().join("tmux.sock");
    let tmux_name = format!("fh-{}", session.id);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        let pane = tmux_query(&sock, &["capture-pane", "-p", "-t", &tmux_name]).await;
        let text = String::from_utf8_lossy(&pane.stdout).to_string();
        // The deadline is checked FIRST, before either branch: a dialog
        // that never advances no matter how often it is answered is a real
        // failure mode (codex's does exactly that under tmux), and a
        // dialog branch that looped straight back to the top would press
        // enter at it forever instead of failing with the pane printed.
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {ready_marker:?}; rendered pane:\n{text}"
        );
        if trust_dialog_markers.iter().any(|m| text.contains(m)) {
            client.send_input(chan, b"\r".to_vec()).await;
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }
        if text.contains(ready_marker) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    // Drain whatever the attach streamed so far; nothing below asserts on it.
    while let Ok(TermEvent::Data(bytes)) = rx.try_recv() {
        seen.extend_from_slice(&bytes);
    }
    let _ = &seen;

    client
        // Deliberately digit-free: a numbered modal (the trust dialogs
        // above offer "1."/"2.") treats a stray digit as an option
        // selection, so a prompt containing one could pick an answer
        // rather than be typed if a dialog ever races this send.
        .send_input(chan, b"Reply with the single word ok.".to_vec())
        .await;
    // The submitting Enter is a SEPARATE keystroke, as a human's is. Sent
    // in the same burst as the text, codex intermittently reads the whole
    // thing as a paste and inserts the carriage return into the composer
    // instead of submitting — observed live as the prompt sitting unsent
    // on the "›" line until the poll deadline, on roughly half of runs,
    // while claude submitted the same burst every time. Splitting it costs
    // nothing (the capture window is anchored on the first input byte and
    // is a minute wide) and removes the whole class of flake.
    tokio::time::sleep(Duration::from_secs(1)).await;
    client.send_input(chan, b"\r".to_vec()).await;

    // The record appears at first prompt SUBMISSION, so this poll is
    // waiting on the agent's own bookkeeping — and then on the production
    // window plus publication grace to elapse before anything may commit.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
    let conversation = loop {
        client.list_sessions().await.expect("list drives capture");
        let snapshot = sup
            .session_snapshot(&session.id)
            .await
            .expect("snapshot")
            .expect("present");
        if let Some(conversation) = snapshot.captured_conversation {
            break conversation;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the real {agent} never produced a record this build could correlate; \
             transcript so far:\n{}",
            String::from_utf8_lossy(&seen)
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    };

    let snapshot = sup
        .session_snapshot(&session.id)
        .await
        .expect("snapshot")
        .expect("present");
    assert_eq!(snapshot.restart_offer, farhelm_proto::RestartOffer::Resume);
    let resume = snapshot
        .resume_argv
        .expect("a Resume offer has a filled argv");
    assert!(
        resume.iter().any(|element| element == &conversation),
        "the captured identity must land in the resume argv: {resume:?}"
    );
    assert!(
        !resume.iter().any(|element| element == "{conversation}"),
        "no placeholder may survive substitution: {resume:?}"
    );
    drop(slot);
}

/// Claude Code for real. Requires a working `claude` on PATH, already
/// authenticated (`claude` run once interactively, or a vendor credential
/// in the environment the login shell sources) and able to reach the API.
/// The prompt costs one short completion.
#[tokio::test]
#[ignore = "needs real Claude Code credentials and network; run deliberately"]
async fn real_claude_session_captures_its_conversation_identity() {
    // No flags and the user's real home: the plain invocation is the one
    // users type, and the one basename derivation must recognize. The
    // marker is "Claude Code v" (only the banner carries the version), not
    // "Claude Code" — the trust dialog's own body says "Claude Code'll be
    // able to read...", and matching that broke this test's first real run.
    real_agent_captures_its_conversation(
        "Claude Code v",
        &["Accessing workspace", "Do you trust"],
        |_work| {
            let home = std::env::var_os("HOME").expect("a real-agent run needs a real HOME");
            (std::path::PathBuf::from(home), "claude".to_string(), None)
        },
    )
    .await;
}

/// Codex for real. Requires an authenticated `codex` on PATH (its
/// `auth.json` is copied into the synthetic home below).
///
/// Unlike the claude test, this one runs codex against a SYNTHESIZED
/// `CODEX_HOME` rather than the user's real one, and that is not a
/// convenience — it is the only path that works. Codex v0.146.0's
/// folder-trust modal is input-dead under tmux: verified with strace, the
/// pane's `\r` reaches codex as a completed `read(0, "\r", 1024) = 1` and
/// is discarded, and the dialog never advances for ANY input tried (CR,
/// numeric option, arrows, kitty-protocol encodings, with and without a
/// rendering client attached). Codex's main TUI accepts input normally in
/// the same pane, so this is an upstream onboarding bug, not a farhelm
/// input-path problem — and it means a human sitting at the terminal is
/// equally stuck, so "have a person accept it" is not a fallback either.
///
/// The synthetic home sidesteps the modal the way codex itself intends:
/// trust is a recorded fact in its config, so a config that already trusts
/// the working directory means the modal never appears. Nothing here
/// configures the AGENT on the user's behalf in production terms — the
/// seam is `SupervisorSeams::agent_home`, which exists for exactly this,
/// and the user's real `~/.codex` is never written to. A `codex`-named
/// shim carries `CODEX_HOME` into the launch, which also keeps basename
/// derivation honest and makes the filled resume argv genuinely runnable.
///
/// No dialog markers are passed: with trust seeded the modal must not
/// appear, and pressing enter at a modal that ignores enter would only
/// burn the deadline two seconds at a time. If it ever does appear, the
/// wait fails with the rendered pane printed, which diagnoses itself.
#[tokio::test]
#[ignore = "needs real Codex credentials and network; run deliberately"]
async fn real_codex_session_captures_its_conversation_identity() {
    real_agent_captures_its_conversation("OpenAI Codex (v", &[], |work| {
        let real_home = std::env::var_os("HOME").expect("a real-agent run needs a real HOME");
        let real_auth = std::path::Path::new(&real_home).join(".codex/auth.json");
        let auth = std::fs::read(&real_auth).unwrap_or_else(|e| {
            panic!(
                "this test needs an authenticated codex ({}): {e}",
                real_auth.display()
            )
        });

        let synth = tempfile::tempdir().expect("synthetic codex home");
        let codex_home = synth.path().join(".codex");
        std::fs::create_dir_all(&codex_home).expect("codex home");
        let auth_path = codex_home.join("auth.json");
        std::fs::write(&auth_path, auth).expect("auth.json");
        std::fs::set_permissions(
            &auth_path,
            std::os::unix::fs::PermissionsExt::from_mode(0o600),
        )
        .expect("auth.json mode");
        // The trust key is the exact path the session is created with.
        std::fs::write(
            codex_home.join("config.toml"),
            format!(
                "[projects.\"{}\"]\ntrust_level = \"trusted\"\n",
                work.display()
            ),
        )
        .expect("config.toml");

        let real_codex = which_binary("codex").expect("codex on PATH");
        let bin = synth.path().join("bin");
        std::fs::create_dir_all(&bin).expect("shim dir");
        let shim = bin.join("codex");
        std::fs::write(
            &shim,
            format!(
                "#!/bin/sh\nexec env CODEX_HOME={} {} \"$@\"\n",
                shell_quote(&codex_home.to_string_lossy()),
                shell_quote(&real_codex.to_string_lossy()),
            ),
        )
        .expect("shim");
        std::fs::set_permissions(
            &shim,
            <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
        )
        .expect("shim mode");

        let agent = shim.to_string_lossy().into_owned();
        (synth.path().to_path_buf(), agent, Some(synth))
    })
    .await;
}

/// First `name` on `PATH` that is an executable regular file.
fn which_binary(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| {
            std::fs::metadata(candidate).is_ok_and(|m| {
                use std::os::unix::fs::PermissionsExt;
                m.is_file() && m.permissions().mode() & 0o111 != 0
            })
        })
}

/// Single-quote `s` for `/bin/sh`, closing and reopening around any quote.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ---------------------------------------------------------------------
// Restart with resume (PLAN_M3.md item 9; M3 acceptance 9, plus the
// restart clauses of acceptance 4 and 5)
//
// Every test below drives the real `RestartSession` handler through the
// real client, against a real tmux — the terminal-reuse behavior these
// pin (a respawned pane keeping the prior run above it) is tmux's, not
// this crate's, so a faked driver would prove nothing about it.
// ---------------------------------------------------------------------

/// Poll `list_sessions` until `session_id` reports `Alive`.
///
/// The mirror image of [`wait_for_non_alive_status`], and needed for the
/// same reason: a restart's reply says the pane exists, not that the agent
/// inside it has execed yet, so "the relaunch is running" is only
/// observable by asking tmux — which `ListSessions` does, freshly, on every
/// call.
async fn wait_for_alive_status(
    client: &SupervisorClient,
    session_id: &str,
    secs: u64,
) -> SessionInfo {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let listed = client.list_sessions().await.expect("list while polling");
        if let Some(found) = listed.sessions.iter().find(|s| s.id == session_id)
            && found.status == SessionStatus::Alive
        {
            return found.clone();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "session {session_id} never became Alive within {secs}s"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// The whole visible content of a session's pane, scrollback included —
/// asked of tmux directly rather than through an attachment.
///
/// The scrollback assertions below are about what the TERMINAL holds after
/// a respawn, which is precisely the thing an attachment's replay is
/// derived from; reading tmux itself keeps those assertions from passing
/// (or failing) for a reason that lives in the replay path instead.
async fn pane_capture(sock: &std::path::Path, tmux_name: &str) -> String {
    let out = tmux_query(sock, &["capture-pane", "-p", "-S", "-", "-t", tmux_name]).await;
    assert!(
        out.status.success(),
        "capture-pane for {tmux_name} must succeed, got: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// M3 acceptance 9, first clause: a restart on a LIVE session confirms
/// (`stop_if_running`), stops the whole process tree, and relaunches into
/// the SAME terminal.
///
/// The proof that the stop lifecycle really ran is the tree's death, not
/// the annotation: a successful restart deliberately CLEARS the annotation
/// with its new generation (PLAN_M3.md item 4), so a stopped-then-restarted
/// session must come back carrying none — which this asserts too, since a
/// stale "stopped by user" on a session that is running again is exactly
/// the bug that clearing exists to prevent.
///
/// The `spawner` fixture is used rather than `basic` because a
/// single-process agent cannot distinguish a tree kill from a plain one,
/// and "reaps the prior run before relaunching, never alongside" is the
/// clause under test.
#[tokio::test]
async fn restarting_a_live_session_stops_its_tree_and_reuses_the_terminal() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let tmux_name = format!("fh-{}", session.id);

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let child_pid = extract_pid(&seen, "CHILD-PID:");
    let grandchild_pid = wait_for_child(child_pid, 10).await;
    let pane_before = pane_id_of(&sock, &tmux_name).await;

    // Stopping first is what the user consented to; without that consent
    // the request is refused outright (see the next test).
    let restarted = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart with consent to stop the running agent");
    assert_eq!(restarted.id, session.id);
    assert_eq!(
        restarted.annotation, None,
        "the new generation clears the previous run's stop annotation"
    );

    // The whole PRIOR tree is gone — including the grandchild, which only
    // a tree sweep ever reaches. What this observes is the END STATE (no
    // survivors, a live new run), not the interleaving: proving "before,
    // never alongside" from the outside would need launch-time
    // instrumentation this harness does not have. The supervisor's own
    // ordering is asserted where it is decided instead — the sweep runs to
    // completion before `begin_relaunch` is called at all.
    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(child_pid, 15).await;
    wait_until_pid_gone(grandchild_pid, 15).await;

    let alive = wait_for_alive_status(&h.client, &session.id, 30).await;
    assert_eq!(
        alive.annotation, None,
        "a running session must never carry the previous run's annotation"
    );
    assert_eq!(
        pane_id_of(&sock, &tmux_name).await,
        pane_before,
        "SPEC.md: restart reuses the session's terminal when it still exists — same pane, \
         not a replacement one"
    );

    h.client.detach(chan).await;
}

/// The other half of the confirm contract: without `stop_if_running`, a
/// restart against an agent the supervisor finds ALIVE is refused with
/// `Conflict` and kills nothing at all.
///
/// This is the TOCTOU guard, not a redundancy: a client's cached status can
/// say "exited" while the agent is running (another client relaunched it,
/// or the status was simply stale), and the flag is what tells the
/// supervisor "the user was actually asked". So the assertion that matters
/// is the process still being alive afterwards, not just the error.
#[tokio::test]
async fn restarting_a_live_session_without_consent_is_refused_and_kills_nothing() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let child_pid = extract_pid(&seen, "CHILD-PID:");

    let err = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await
        .expect_err("a live agent may not be restarted without consent to stop it");
    let err = err
        .downcast_ref::<SupervisorError>()
        .expect("a refusal carries the supervisor's own classification");
    assert_eq!(err.kind, ErrorKind::Conflict);
    assert!(
        err.message.contains("still running"),
        "the refusal must say why, so a client can ask the user: {}",
        err.message
    );

    assert!(
        !process_is_gone(self_pid) && !process_is_gone(child_pid),
        "a refused restart must not have killed anything"
    );
    assert_eq!(
        listed(&h.client, &session.id).await.status,
        SessionStatus::Alive,
        "and must leave the session exactly as it was"
    );
}

/// M3 acceptance 9: relaunching into a RETAINED terminal keeps the prior
/// run in scrollback — "the previous run's output stays in scrollback"
/// (SPEC.md), with the new run's output below it.
///
/// The marker is produced by TYPING into the first run rather than by its
/// startup banner, because both runs print the same banner: an assertion
/// on text only the first run could have produced is what makes this about
/// retention rather than about the relaunch having printed something.
#[tokio::test]
async fn a_reused_terminal_keeps_the_prior_run_above_the_new_one() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let (session, _work) = basic_session(&h).await;
    let tmux_name = format!("fh-{}", session.id);

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client
        .send_input(chan, b"PRIOR-RUN-MARKER\r".to_vec())
        .await;
    wait_for(&mut rx, &mut seen, "echo:", 10).await;
    wait_for(&mut rx, &mut seen, "PRIOR-RUN-MARKER", 10).await;

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart");
    wait_for_alive_status(&h.client, &session.id, 30).await;

    // Read from tmux itself, and wait for the new run's own banner to
    // appear in the capture: the relaunched agent starts asynchronously,
    // so a single read can land before it has printed anything.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let capture = loop {
        let capture = pane_capture(&sock, &tmux_name).await;
        if let Some(marker) = capture.find("PRIOR-RUN-MARKER")
            && capture[marker..].contains("FAKE-AGENT READY")
        {
            break capture;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the new run never appeared below the prior run's output; capture:\n{capture}"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    assert!(
        capture.contains("PRIOR-RUN-MARKER"),
        "the prior run's output must survive the respawn: {capture}"
    );

    // And a client attaching after the restart sees the same thing, since
    // its replay is that scrollback.
    h.client.detach(chan).await;
    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach after restart");
    let mut replay = Vec::new();
    wait_for_after(
        &mut rx2,
        &mut replay,
        "PRIOR-RUN-MARKER",
        "FAKE-AGENT READY",
        20,
    )
    .await;
}

/// M3 acceptance 9: leftover descendants of a prior run are reaped BEFORE
/// the relaunch, never left running alongside it — including a daemon left
/// behind by an agent that exited on its own, which SPEC.md says only the
/// session's next restart (or teardown) goes hunting for.
///
/// The agent is killed directly rather than stopped, for the same reason
/// `stop_kills_a_reparented_daemon_with_no_live_pane_to_walk_from` does it:
/// a stop would already have reaped the daemon through the live-pane path,
/// proving nothing about the restart's own sweep. The daemon has fully
/// reparented to init by then, so only the environment-marker scan can
/// find it at all.
#[tokio::test]
async fn a_restart_reaps_a_daemon_left_by_a_self_exited_agent() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-reparent"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let daemon_pid = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;

    // SAFETY: `self_pid` is a real, currently-live pid this test just read
    // out of the fake agent's own output.
    unsafe {
        libc::kill(self_pid as libc::pid_t, libc::SIGKILL);
    }
    wait_until_pid_gone(self_pid, 10).await;
    wait_for_non_alive_status(&h.client, &session.id, 20).await;
    assert!(
        !process_is_gone(daemon_pid),
        "the daemon must outlive its parent, or this test proves nothing"
    );

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await
        .expect("an agent that already exited needs no stop consent");

    // Again the end state rather than the interleaving: the daemon is gone
    // and a new run is up. The "before, not alongside" ordering is a
    // property of the handler (the sweep completes before the generation is
    // opened), not something this vantage point can witness.
    wait_until_pid_gone(daemon_pid, 15).await;
    wait_for_alive_status(&h.client, &session.id, 30).await;
}

/// M3 acceptance 9: a vanished working directory fails the restart with an
/// error NAMING the directory, and the session survives untouched — its
/// stop annotation included, which is PLAN_M3.md item 4's "only a
/// SUCCESSFUL restart clears it".
///
/// The annotation is what makes this more than an error-message test: the
/// clear commits with the new launch generation, so a restart that never
/// gets a generation must leave the stopped outcome exactly as it was.
#[tokio::test]
async fn a_vanished_working_directory_refuses_the_restart_and_keeps_the_annotation() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let cwd = work.path().to_string_lossy().into_owned();
    let session = h
        .client
        .create_session(
            &cwd,
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    h.client.stop_session(&session.id).await.expect("stop");
    assert_eq!(
        listed(&h.client, &session.id).await.annotation.as_deref(),
        Some("stopped by user")
    );

    // The directory goes away under the session, exactly as a user
    // deleting a worktree would leave it.
    work.close().expect("remove the working directory");

    let err = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await
        .expect_err("a session whose working directory is gone cannot be relaunched");
    let err = err
        .downcast_ref::<SupervisorError>()
        .expect("a precondition failure carries its classification");
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert!(
        err.message.contains(&cwd),
        "the error must name the directory (SPEC.md): {}",
        err.message
    );

    let after = listed(&h.client, &session.id).await;
    assert!(
        matches!(after.status, SessionStatus::Exited { .. }),
        "the session itself survives a refused restart: {after:?}"
    );
    assert_eq!(
        after.annotation.as_deref(),
        Some("stopped by user"),
        "a restart that never opened a launch generation cannot have cleared the annotation"
    );
}

/// The staleness contract in the direction that actually happens
/// (`ControlMsg::RestartSession`'s docs): conversation capture upgrades a
/// session's offer from fresh-only to resumable AFTER a client read its
/// `SessionInfo`, so the mode that client picked is no longer the one the
/// supervisor will accept — and the refusal has to NAME the current offer,
/// because the client's next move is to re-present it rather than retry.
///
/// Driven through the ordinary client rather than a raw frame writer: the
/// staleness this exercises is a property of the SUPERVISOR's revalidation,
/// and reproducing it only needs the request to be sent with a mode that
/// was correct a moment earlier.
#[tokio::test]
async fn a_capture_that_lands_after_the_clients_read_makes_a_fresh_restart_conflict() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;
    // What a client that listed BEFORE the first prompt would have cached.
    assert_eq!(
        session.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly
    );

    let (_chan, _rx, _seen, conversation) = provoke_record(&h, &session).await;
    settle_past_horizon(&h).await;
    assert_eq!(
        snapshot_of(&h, &session.id)
            .await
            .captured_conversation
            .as_deref(),
        Some(conversation.as_str()),
        "the capture must have landed, or there is no staleness to test"
    );

    let err = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect_err("a fresh restart is not a legal answer to a resumable session");
    let err = err
        .downcast_ref::<SupervisorError>()
        .expect("a stale-offer refusal carries its classification");
    assert_eq!(err.kind, ErrorKind::Conflict);
    assert!(
        err.message.contains("resum"),
        "the refusal must name the CURRENT offer so the client can re-present it: {}",
        err.message
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "and the offer the client should re-present is the one it can now read"
    );
}

/// The environment variable the record-writing fixture reads a resumed
/// conversation id from (`fake_agent::RESUME_ENV_VAR`).
///
/// Duplicated rather than imported because this crate has no library
/// target — an integration test cannot reach `fake_agent`'s items at all
/// (the same duplication `FLOOD_RECORDS` accepts, for the same reason).
/// Drift is loud rather than silent: the fixture would report no resume at
/// all and the test below would fail waiting for its marker.
const FAKE_AGENT_RESUME_ENV: &str = "FARHELM_FAKE_AGENT_RESUME";

/// A resume template that runs the record-writing fixture and hands it the
/// substituted conversation id.
///
/// The `sh -c` wrapper exists for one mundane reason with a real payoff:
/// this binary's argument parser lives in `main.rs`, so the fixture cannot
/// grow a `--resume` flag from the test side — the wrapper moves the
/// substituted argv element into the environment variable the fixture reads
/// instead (`fake_agent::RESUME_ENV_VAR`). What it does NOT change is the
/// property under test: `{conversation}` is still its OWN argv element,
/// substituted slot-for-slot by the supervisor rather than spliced into any
/// string, which is exactly what keeps an id from ever becoming part of a
/// different command.
///
/// `argv0` must stay the kind-named symlink so the session still derives
/// its integration from its own invocation, as a real one would.
fn fixture_resume_template(
    argv0: &std::path::Path,
    kind: &str,
    record_home: &std::path::Path,
) -> Vec<String> {
    vec![
        "sh".to_string(),
        "-c".to_string(),
        format!(
            "{}=\"$2\" exec \"$0\" internal fake-agent --script {kind}-record --record-home \"$1\"",
            FAKE_AGENT_RESUME_ENV
        ),
        argv0.to_string_lossy().into_owned(),
        record_home.to_string_lossy().into_owned(),
        farhelm_supervisor::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
    ]
}

/// Where [`interrupted_session_resumes_its_conversation`]'s final assertion
/// finds the record the resumed run appended to, once it knows `kind` and
/// the conversation id.
///
/// Claude's tree is partitioned by working directory, so a listing of the
/// one project directory the fixture writes into is enough. Codex's is not
/// — it nests by CALENDAR DATE instead (see `record_path` in
/// `fake_agent.rs`) — so this walks the whole `.codex/sessions` tree rather
/// than duplicating that date math: a test-side reimplementation of the
/// fixture's own path formula would only prove the two agree with each
/// other, not that either matches what a real resumed Codex session does.
fn resumed_record_file(
    home: &std::path::Path,
    kind: &str,
    work: &std::path::Path,
    conversation: &str,
) -> std::path::PathBuf {
    match kind {
        "claude" => {
            let canonical = std::fs::canonicalize(work).expect("canonicalize the workdir");
            std::fs::read_dir(home.join(".claude").join("projects").join(
                farhelm_supervisor::agent_kind::munge_cwd(&canonical.to_string_lossy()),
            ))
            .expect("project dir")
            .map(|entry| entry.expect("dir entry").path())
            .find(|path| path.to_string_lossy().contains(conversation))
            .expect("the captured record still exists")
        }
        "codex" => {
            fn walk(dir: &std::path::Path, id: &str) -> Option<std::path::PathBuf> {
                for entry in std::fs::read_dir(dir).expect("read the sessions tree") {
                    let path = entry.expect("dir entry").path();
                    if path.is_dir() {
                        if let Some(found) = walk(&path, id) {
                            return Some(found);
                        }
                    } else if path.to_string_lossy().contains(id) {
                        return Some(path);
                    }
                }
                None
            }
            walk(&home.join(".codex").join("sessions"), conversation)
                .expect("the captured record still exists")
        }
        other => panic!("resumed_record_file: unknown kind {other}"),
    }
}

/// M3 acceptance 9 and 8 together: a session INTERRUPTED by a (simulated)
/// reboot restarts into a FRESH terminal — there is none left to reuse —
/// and `Resume` mode fills the snapshot's template with the identity that
/// was captured before the reboot, so the relaunched agent picks up the
/// same conversation.
///
/// Both halves are asserted from the fixture's own output rather than
/// inferred: it echoes the argv it was launched with (so the substituted id
/// is visible as a fact about what RAN), and it reports adopting the
/// existing record rather than starting a new one — which is what "resumes
/// exactly that conversation" means on disk.
///
/// Shared by both agent kinds ([`an_interrupted_session_resumes_its_conversation_in_a_fresh_terminal`]
/// and [`an_interrupted_codex_session_resumes_its_conversation_in_a_fresh_terminal`]):
/// the resume path is kind-agnostic once `fixture_resume_template` has
/// filled in the placeholder, and the only kind-specific step left is
/// finding where the record landed on disk ([`resumed_record_file`]).
async fn interrupted_session_resumes_its_conversation(kind: &str) {
    let home = tempfile::tempdir().expect("agent home");
    let bin = tempfile::tempdir().expect("agent bin");
    std::os::unix::fs::symlink(farhelm_bin(), bin.path().join(kind))
        .expect("symlink the farhelm binary under the agent's own name");
    let state = tempfile::tempdir().expect("state dir");
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));

    let seams = |boot: &str| SupervisorSeams {
        boot_id: {
            let boot = boot.to_string();
            Arc::new(move || Ok(Some(boot.clone())))
        },
        agent_home: Some(home.path().to_path_buf()),
        capture_window: test_capture_bounds(),
        ..SupervisorSeams::default()
    };

    let work = tempfile::tempdir().expect("workdir");
    let conversation = {
        let sup = Supervisor::new_with_seams(
            state.path(),
            farhelm_bin().into(),
            SupervisorTimeouts::default(),
            seams("boot-a"),
        )
        .await
        .expect("first supervisor");
        let client = connect_client(&sup).await;
        let session = client
            .create_session_with_extras(
                &work.path().to_string_lossy(),
                &format!(
                    "{} internal fake-agent --script {kind}-record --record-home {}",
                    shell_words::quote(&bin.path().join(kind).to_string_lossy()),
                    shell_words::quote(&home.path().to_string_lossy())
                ),
                None,
                80,
                24,
                farhelm_helm::CreateExtras {
                    resume_template: Some(fixture_resume_template(
                        &bin.path().join(kind),
                        kind,
                        home.path(),
                    )),
                    ..farhelm_helm::CreateExtras::default()
                },
            )
            .await
            .expect("create the record-writing session");

        let (chan, mut rx) = client.attach(&session.id, 80, 24).await.expect("attach");
        let mut seen = Vec::new();
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
        client.send_input(chan, b"first prompt\r".to_vec()).await;
        wait_for(&mut rx, &mut seen, "RECORD-WRITTEN:", 20).await;
        let conversation = marker_value(&seen, "RECORD-WRITTEN:");

        // Let the claim become durable before the reboot: an identity that
        // only ever existed in memory would prove nothing about a session
        // whose supervisor is about to be replaced.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            client.list_sessions().await.expect("list drives capture");
            let snapshot = sup
                .session_snapshot(&session.id)
                .await
                .expect("snapshot")
                .expect("present");
            if snapshot.captured_conversation.is_some() {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the fixture's identity was never captured"
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        drop(client);
        let drain = tokio::time::Instant::now() + Duration::from_secs(10);
        while Arc::strong_count(&sup) > 1 {
            assert!(tokio::time::Instant::now() < drain, "connection drain");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        drop(sup);
        (conversation, session)
    };
    let (conversation, session) = conversation;

    // The reboot: tmux dies with the host, and the next supervisor reads a
    // different boot id.
    kill_tmux_server_and_wait(&state.path().join("tmux.sock")).await;
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        seams("boot-b"),
    )
    .await
    .expect("post-reboot supervisor");
    assert!(sup.owns_state_dir(), "the predecessor must be gone");
    let client = connect_client(&sup).await;
    let interrupted = listed(&client, &session.id).await;
    assert_eq!(interrupted.status, SessionStatus::Interrupted);
    assert_eq!(
        interrupted.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "the captured identity survived the reboot, so opening this session offers a resume"
    );

    let restarted = client
        .restart_session(&session.id, farhelm_proto::RestartMode::Resume, false)
        .await
        .expect("an interrupted session has nothing running to consent about");
    assert_eq!(
        restarted.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "the identity is the conversation's, not the run's — it survives the relaunch too"
    );

    let (chan, mut rx) = client
        .attach(&session.id, 80, 24)
        .await
        .expect("the relaunch built a fresh terminal to attach to");
    let mut seen = Vec::new();
    wait_for(
        &mut rx,
        &mut seen,
        &format!("RECORD-RESUMED:{conversation}"),
        30,
    )
    .await;
    let argv_line = String::from_utf8_lossy(&seen);
    let argv_line = argv_line
        .split("FAKE-AGENT ARGV:")
        .nth(1)
        .expect("the fixture echoes its own argv")
        .lines()
        .next()
        .expect("a line");
    assert!(
        argv_line.contains("--record-home"),
        "the resume ran the TEMPLATE, not the launch invocation: {argv_line}"
    );
    // The substituted id itself is not visible in this argv, and that is a
    // property of the FIXTURE, not of the product: the template's
    // `{conversation}` element is consumed by the `sh -c` wrapper that
    // moves it into the environment variable the fixture reads (see
    // `fixture_resume_template`). What proves the id reached the relaunched
    // process is the `RECORD-RESUMED:<id>` marker waited on above, which
    // the fixture only prints for the exact id it was handed.

    // The resumed conversation genuinely continues: the fixture's
    // `append` command is its stand-in for a real agent writing more of
    // the SAME conversation (see `record_agent`'s docs), and it can only
    // do that because the relaunch handed it the id it was resuming.
    client.send_input(chan, b"append\r".to_vec()).await;
    wait_for(
        &mut rx,
        &mut seen,
        &format!("RECORD-APPENDED:{conversation}"),
        20,
    )
    .await;
    let record = String::from_utf8(
        std::fs::read(resumed_record_file(
            home.path(),
            kind,
            work.path(),
            &conversation,
        ))
        .expect("read the record"),
    )
    .expect("the fixture writes UTF-8");
    assert!(
        record.lines().count() >= 2,
        "the resumed run must append to the captured conversation, not replace it: {record}"
    );
    drop(slot);
}

/// Thin wrapper around [`interrupted_session_resumes_its_conversation`] for
/// the Claude-shaped fixture. Kept as its own `#[tokio::test]` (rather than
/// folded into a loop) so a failure names the agent kind directly in the
/// test binary's output.
#[tokio::test]
async fn an_interrupted_session_resumes_its_conversation_in_a_fresh_terminal() {
    interrupted_session_resumes_its_conversation("claude").await;
}

/// The Codex half of PLAN_M3.md acceptance 8: until this test existed, the
/// "both fixture pairs restart-resume their own conversation" claim was
/// only pinned for Codex up to offer-and-argv (`snapshot.resume_offer`,
/// `resume_argv`) — nothing actually EXECUTED a resume relaunch and
/// confirmed the SAME conversation record grew on disk afterward, the way
/// [`an_interrupted_session_resumes_its_conversation_in_a_fresh_terminal`]
/// already does for Claude. The resume machinery itself is kind-agnostic
/// (see `fixture_resume_template`'s docs), but only running it end to end
/// against Codex's differently-shaped, date-nested record tree
/// (`resumed_record_file`) rules out a Claude-only bug hiding behind a
/// kind-agnostic-looking code path.
#[tokio::test]
async fn an_interrupted_codex_session_resumes_its_conversation_in_a_fresh_terminal() {
    interrupted_session_resumes_its_conversation("codex").await;
}

/// SPEC.md's verbatim fallback resume, which only an explicitly configured
/// placeholder-free template can produce (PLAN_M3.md item 7): the session
/// offers `FallbackTemplate`, and restarting it runs that template rather
/// than the launch invocation.
///
/// The two commands are deliberately distinguishable in the terminal — the
/// launch prints one marker and the fallback another — because "ran the
/// right command" is the whole claim, and a template that silently fell
/// back to the launch invocation would otherwise look identical.
#[tokio::test]
async fn a_configured_fallback_template_is_what_a_restart_runs() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            "sh -c 'echo LAUNCH-INVOCATION; sleep 300'",
            None,
            80,
            24,
            farhelm_helm::CreateExtras {
                // Placeholder-free, on a session whose basename derives no
                // integration: SPEC.md's "the profile's resume invocation
                // verbatim".
                resume_template: Some(vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo FALLBACK-RESUME; sleep 300".to_string(),
                ]),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create with a configured fallback resume command");
    assert_eq!(
        session.restart_offer,
        farhelm_proto::RestartOffer::FallbackTemplate,
        "a configured placeholder-free template is an offer in its own right, not a fresh launch"
    );

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "LAUNCH-INVOCATION", 20).await;

    // The mode has to match the offer exactly — a `Fresh` restart of a
    // session with a configured fallback is refused, not silently honored.
    let refused = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect_err("fresh is not a legal mode for a fallback-template offer");
    assert_eq!(
        refused
            .downcast_ref::<SupervisorError>()
            .expect("classified")
            .kind,
        ErrorKind::Conflict
    );

    h.client
        .restart_session(
            &session.id,
            farhelm_proto::RestartMode::FallbackTemplate,
            true,
        )
        .await
        .expect("restart through the configured fallback");
    wait_for_alive_status(&h.client, &session.id, 30).await;

    h.client.detach(chan).await;
    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach after restart");
    let mut replay = Vec::new();
    wait_for_after(
        &mut rx2,
        &mut replay,
        "LAUNCH-INVOCATION",
        "FALLBACK-RESUME",
        20,
    )
    .await;
}

/// The variable the `env-echo` fixture reports (`fake_agent::RC_MARKER_VAR`),
/// duplicated for the same reason [`FAKE_AGENT_RESUME_ENV`] is: this crate
/// has no library target for a test to import from. Drift fails the test
/// rather than weakening it — the fixture would report an empty value and
/// the assertions below would not find the one they wait for.
const RC_MARKER_VAR: &str = "FARHELM_RC_MARKER";

/// Write rc files exporting [`RC_MARKER_VAR`] as `value` into a private
/// HOME, covering every shell family this launch chain might resolve to.
///
/// The launch shell is whatever the supervisor's own `$SHELL`/passwd entry
/// says (`launch::resolve_shell`), which no test may change — so instead of
/// guessing one, this writes the file each family reads for an INTERACTIVE
/// LOGIN shell (`-l -i`, the shape `window_command` uses): bash reads
/// `.bash_profile` (and `.bashrc` when a profile sources it, as this one
/// does), zsh reads `.zshenv`/`.zprofile`/`.zshrc` under `ZDOTDIR`, and a
/// POSIX `sh` reads `$ENV`. Whichever one the host uses, the value arrives
/// by the route a user's own rc file would take.
fn write_rc_files(home: &std::path::Path, value: &str) {
    let export = format!("export {RC_MARKER_VAR}={value}\n");
    std::fs::write(home.join(".bashrc"), &export).expect("write .bashrc");
    std::fs::write(
        home.join(".bash_profile"),
        format!(". \"$HOME/.bashrc\"\n{export}"),
    )
    .expect("write .bash_profile");
    for name in [".zshenv", ".zprofile", ".zshrc", ".profile", ".shinit"] {
        std::fs::write(home.join(name), &export).unwrap_or_else(|e| panic!("write {name}: {e}"));
    }
}

/// M3 acceptance 9's last clause, and SPEC.md's environment contract: "the
/// environment is evaluated at each launch: edit your rc files and the next
/// launch or restart sees the change".
///
/// The rc files live in a private HOME injected through
/// `SupervisorSeams::launch_env` — never by mutating this process's
/// environment, which this repo forbids and which every concurrently
/// running harness would share anyway.
///
/// If the host's login shell reads none of the files this test can write,
/// it says so loudly and stops rather than asserting something it cannot
/// observe: a silent pass would be worse than an honest skip, and a
/// failure would blame the product for the harness's blind spot.
#[tokio::test]
async fn an_rc_file_change_between_launches_reaches_the_relaunched_agent() {
    let home = tempfile::tempdir().expect("fixture home");
    write_rc_files(home.path(), "first");
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_env: vec![
                (
                    "HOME".to_string(),
                    home.path().to_string_lossy().into_owned(),
                ),
                (
                    "ZDOTDIR".to_string(),
                    home.path().to_string_lossy().into_owned(),
                ),
                (
                    "ENV".to_string(),
                    home.path().join(".shinit").to_string_lossy().into_owned(),
                ),
            ],
            ..SupervisorSeams::default()
        },
    )
    .await;
    let work = tempfile::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script env-echo"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, &format!("ENV:{RC_MARKER_VAR}="), 20).await;
    let observed = marker_value(&seen, &format!("ENV:{RC_MARKER_VAR}="));
    if observed != "first" {
        // Deterministic, not a shrug: the rc files this test writes cover
        // the shell families this launch chain can resolve to (see
        // `write_rc_files`), so for any of them the value MUST have
        // arrived. Anything else is a host whose login shell this harness
        // genuinely cannot reach, which is a skip — and one that names the
        // shell, so the gap is diagnosable rather than mysterious.
        let shell = farhelm_supervisor::launch::resolve_shell().await;
        let family = std::path::Path::new(&shell)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| shell.clone());
        assert!(
            !["bash", "zsh", "sh", "dash", "ksh"].contains(&family.as_str()),
            "the launch shell is {shell}, which sources one of the rc files this test writes, \
             so the relaunched agent should have seen the value; it reported {observed:?} \
             instead"
        );
        eprintln!(
            "SKIPPED an_rc_file_change_between_launches_reaches_the_relaunched_agent: this \
             host launches sessions through {shell}, which sources none of the rc files this \
             test knows how to write"
        );
        return;
    }

    // The edit a user would make between launches.
    write_rc_files(home.path(), "second");
    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart");
    wait_for_alive_status(&h.client, &session.id, 30).await;

    // A restart detaches whatever was attached to the previous run (the
    // supervisor's `detach_for_restart`), so the client reattaches — which
    // is also how it gets the reused terminal's scrollback replayed,
    // first run's line included.
    h.client.detach(chan).await;
    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach after restart");
    // Anchored AFTER the first run's own line, which is still in the
    // reused terminal's scrollback: an unanchored wait would match the
    // pre-restart value and pass without the relaunch having sourced
    // anything.
    let mut replay = Vec::new();
    wait_for_after(
        &mut rx2,
        &mut replay,
        &format!("ENV:{RC_MARKER_VAR}=first"),
        &format!("ENV:{RC_MARKER_VAR}=second"),
        30,
    )
    .await;
}

/// M3 acceptance 4's restart clause: after a successful restart, the
/// previous launch's `error` is gone — status, detail, and the sentinel
/// file that produced it.
///
/// The session is created with an invocation that cannot exec plus a
/// configured resume command that can, which is the only way (before M5's
/// profiles) to give one session both a failing launch and a working
/// relaunch. What that combination really exercises is the per-launch
/// sentinel lifecycle: the failed launch's sentinel sits at the very path
/// this relaunch's own would use, and a build that left it there would
/// classify a perfectly good agent as `error` forever.
#[tokio::test]
async fn a_restart_clears_a_previous_launch_error() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let work = tempfile::tempdir().expect("workdir");
    let missing_binary = work.path().join("no-such-farhelm-agent");
    let session = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            &missing_binary.to_string_lossy(),
            None,
            80,
            24,
            farhelm_helm::CreateExtras {
                resume_template: Some(vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo RELAUNCHED-OK; sleep 300".to_string(),
                ]),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create a session whose invocation cannot exec");

    wait_for_dead_pane(&sock, &format!("fh-{}", session.id)).await;
    let errored = wait_for_non_alive_status(&h.client, &session.id, 30).await;
    assert!(
        matches!(errored.status, SessionStatus::Error { .. }),
        "a launch that never execed is an error, not an exit: {errored:?}"
    );

    h.client
        .restart_session(
            &session.id,
            farhelm_proto::RestartMode::FallbackTemplate,
            false,
        )
        .await
        .expect("restart through the configured resume command");

    let alive = wait_for_alive_status(&h.client, &session.id, 30).await;
    assert!(
        !matches!(alive.status, SessionStatus::Error { .. }),
        "the previous launch's error describes a run this session no longer has"
    );
    // Sentinel paths are generation-scoped, so even a surviving gen-0 file
    // could never describe the relaunch's generation. What this pins is the
    // cleanup half: the consumed sentinel is removed rather than left as an
    // orphan for every future reload to re-read and re-classify.
    let sentinel = status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    assert!(
        !sentinel.exists(),
        "the failed launch's sentinel must not outlive the launch it described: {}",
        sentinel.display()
    );
}

// ---------------------------------------------------------------------
// Restart under concurrency, and the failure paths that must not lose
// durable metadata (PR8 review-swarm fix batch, items 1 and 4).
// ---------------------------------------------------------------------

/// The security case behind the per-session lifecycle claim: two restarts
/// of one session must never interleave into a kill nobody consented to.
///
/// Without serialization the sequence is entirely legal-looking and
/// entirely wrong: the first restart records its stop intent and starts a
/// kill sweep that takes seconds; the second restart, arriving mid-sweep,
/// probes the pane, finds it dead, concludes no consent is needed — and
/// then runs ITS marker sweep, which reaps the agent the first restart has
/// meanwhile launched. The user asked for a restart and got a stopped
/// session, with a live agent killed on the way.
///
/// The claim turns that into an ordinary serial pair, and this test pins
/// exactly that: the second restart runs AFTER the first has finished, so
/// it finds a LIVE agent and refuses without consent — and the session is
/// still running when both have returned.
///
/// `spawner-stubborn`'s SIGTERM-ignoring child is what makes the window
/// wide enough to aim at: it forces the first restart's sweep through the
/// full grace/quiesce/SIGKILL escalation rather than finishing instantly.
#[tokio::test]
async fn a_second_restart_cannot_reap_the_agent_the_first_one_just_launched() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-stubborn"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    wait_for_file(&work.path().join("stubborn-ready"), 10).await;

    // The first restart, with consent: it will spend seconds in the sweep.
    let first_client = Arc::clone(&h.client);
    let first_id = session.id.clone();
    let first = tokio::spawn(async move {
        first_client
            .restart_session(&first_id, farhelm_proto::RestartMode::Fresh, true)
            .await
    });
    // Long enough to be inside that sweep, short enough to be well before
    // it ends (`kill_process_tree`'s grace period alone is ~1s).
    tokio::time::sleep(Duration::from_millis(300)).await;
    let second = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await;

    first
        .await
        .expect("the first restart task")
        .expect("the first restart succeeds");
    let refusal = second.expect_err(
        "the second restart must see the FIRST one's relaunched agent, alive, and refuse \
         without consent — never sweep it away",
    );
    assert_eq!(
        refusal
            .downcast_ref::<SupervisorError>()
            .expect("classified")
            .kind,
        ErrorKind::Conflict
    );
    // The whole point: something is still running when the dust settles.
    wait_for_alive_status(&h.client, &session.id, 30).await;
}

/// Delete racing a restart resolves to exactly one winner, with the loser
/// getting an honest error — never a session torn half-down.
///
/// The delete is issued while the restart is inside its kill sweep, which
/// is precisely where an unserialized delete would kill the tmux session
/// the relaunch is about to respawn into. Whichever order the claim
/// imposes, the invariants below hold: the session is gone afterwards, and
/// nothing carrying its marker is still running.
#[tokio::test]
async fn a_delete_racing_a_restart_leaves_no_session_and_no_survivors() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-stubborn"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    wait_for_file(&work.path().join("stubborn-ready"), 10).await;

    let restart_client = Arc::clone(&h.client);
    let restart_id = session.id.clone();
    let restart = tokio::spawn(async move {
        restart_client
            .restart_session(&restart_id, farhelm_proto::RestartMode::Fresh, true)
            .await
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    h.client
        .delete_session(&session.id)
        .await
        .expect("the delete completes rather than colliding with the relaunch");
    // The restart either finished first (and its agent was then deleted)
    // or lost to the delete and said so; both are legitimate, and neither
    // may leave anything behind.
    let _ = restart.await.expect("the restart task");

    assert!(
        h.client
            .list_sessions()
            .await
            .expect("list")
            .sessions
            .iter()
            .all(|s| s.id != session.id),
        "the delete must win the session's existence outright"
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if marked_pids(&session.id).is_empty() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no process carrying this session's marker may outlive the delete"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// PLAN_M3.md item 4's binding contract, and the one this PR could most
/// easily have broken: a restart that FAILS leaves the stop annotation
/// exactly where it was.
///
/// The generation has to be opened before any side effect (item 2's
/// ordering rule), and opening it is what clears the annotation — so the
/// only way both promises hold is for a definitively-failed relaunch to put
/// the previous outcome back. This drives a real failure rather than a
/// simulated one: with the launch directory read-only, the spec write fails
/// and nothing external has happened, which is exactly the class of failure
/// the restore is defined for.
#[tokio::test]
async fn a_failed_restart_restores_the_stop_annotation_it_had_cleared() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    h.client.stop_session(&session.id).await.expect("stop");
    let stopped = listed(&h.client, &session.id).await;
    assert_eq!(stopped.annotation.as_deref(), Some("stopped by user"));

    // The launch directory becomes unwritable, so this restart's spec —
    // its first side effect — cannot land.
    let launch_dir = h.state.path().join("launch");
    let original = std::fs::metadata(&launch_dir)
        .expect("launch dir")
        .permissions();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&launch_dir, std::fs::Permissions::from_mode(0o500))
            .expect("make the launch dir read-only");
    }
    let refused = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await;
    std::fs::set_permissions(&launch_dir, original).expect("restore the launch dir");
    refused.expect_err("a launch spec that cannot be written fails the restart");

    let after = listed(&h.client, &session.id).await;
    assert!(
        matches!(after.status, SessionStatus::Exited { .. }),
        "the previous run's outcome is restored, not left as an unknown launching row: \
         {after:?}"
    );
    assert_eq!(
        after.annotation.as_deref(),
        Some("stopped by user"),
        "only a SUCCESSFUL restart clears the annotation (PLAN_M3.md item 4)"
    );

    // ...and the session is still restartable afterwards, which is what
    // makes the restore a recovery rather than a tidier failure.
    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await
        .expect("restart once the directory is writable again");
    wait_for_alive_status(&h.client, &session.id, 30).await;
}

/// A relaunch into a directory that no longer resolves where it did at
/// create time is refused, naming both paths (fix-batch item 21).
///
/// The threat is specific: a session's cwd is a path, and a path can be a
/// symlink somebody repoints between launches. Relaunching a permissive
/// agent into a directory an attacker chose is not a decision the user
/// made, and `ensure_cwd_usable`'s existence check cannot see it — the
/// directory is perfectly usable, it is simply a different one.
#[tokio::test]
async fn a_repointed_working_directory_refuses_the_restart() {
    let h = harness().await;
    let real = tempfile::tempdir().expect("real cwd");
    let decoy = tempfile::tempdir().expect("decoy cwd");
    let link = tempfile::tempdir().expect("link parent");
    let link = link.path().join("cwd");
    std::os::unix::fs::symlink(real.path(), &link).expect("symlink");

    let session = h
        .client
        .create_session(
            &link.to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create through the symlink");
    h.client.stop_session(&session.id).await.expect("stop");

    // The repoint.
    std::fs::remove_file(&link).expect("drop the old link");
    std::os::unix::fs::symlink(decoy.path(), &link).expect("repoint");

    let err = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await
        .expect_err("the session's directory is no longer the one it was created in");
    let err = err
        .downcast_ref::<SupervisorError>()
        .expect("a precondition failure carries its classification");
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    let canonical_decoy = std::fs::canonicalize(decoy.path()).expect("canonicalize");
    assert!(
        err.message
            .contains(&canonical_decoy.to_string_lossy().into_owned()),
        "the refusal must name where the path leads NOW: {}",
        err.message
    );
    assert_eq!(
        listed(&h.client, &session.id).await.annotation.as_deref(),
        Some("stopped by user"),
        "a refusal this early cannot have touched the session's durable state"
    );
}

/// A relaunch that is not resuming a captured identity opens a FRESH
/// capture window (fix-batch items 5 and 15): the previous run's ambiguity
/// verdict and first-input anchor are per-LAUNCH state, and carrying them
/// forward would deny the new run any capture at all.
///
/// Two fixture sessions in one directory make the first run's correlation
/// ambiguous — the durable refusal SPEC.md's no-wrong-conversation rule
/// depends on. Restarting one of them fresh must then let it capture its
/// OWN conversation on the new run, which is only possible if the verdict
/// and the anchor were both cleared.
#[tokio::test]
async fn a_fresh_relaunch_opens_a_new_capture_window_after_an_ambiguity() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let first = record_session(&h, &fixtures, work.path(), "claude").await;
    let second = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_c1, _r1, _s1, _id1) = provoke_record(&h, &first).await;
    let (_c2, _r2, _s2, _id2) = provoke_record(&h, &second).await;
    settle_past_horizon(&h).await;
    let ambiguous = snapshot_of(&h, &first.id).await;
    assert!(ambiguous.capture_ambiguous, "the setup must be ambiguous");
    assert_eq!(
        ambiguous.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly
    );

    // The rival is stopped first, so the new run's window has the
    // directory to itself — otherwise the ambiguity rule would (correctly)
    // refuse again and this test could not tell a cleared verdict from an
    // inherited one.
    h.client.stop_session(&second.id).await.expect("stop rival");
    h.client
        .restart_session(&first.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("fresh restart");
    wait_for_alive_status(&h.client, &first.id, 30).await;

    let after = snapshot_of(&h, &first.id).await;
    assert!(
        !after.capture_ambiguous,
        "the previous run's verdict describes a run this session no longer has"
    );
    assert_eq!(
        after.first_input_at, None,
        "and its first-input anchor points at a window that closed long ago"
    );

    // The new run captures its own conversation, which an inherited
    // ambiguity would have made impossible forever.
    let (chan, mut rx) = h.client.attach(&first.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    // Waited for by the ECHO of a prompt only this run has seen, and read
    // as the LAST marker: this attachment replays the reused terminal's
    // scrollback, which still holds the pre-restart run's own
    // `RECORD-WRITTEN:` line — waiting on the marker itself would return
    // on the OLD one, before the new run had written anything at all.
    h.client
        .send_input(chan, b"prompt-after-restart\r".to_vec())
        .await;
    // Anchored on the typed line's own echo, and read as the LAST marker:
    // this attachment replays the reused terminal's scrollback, so both an
    // earlier `RECORD-WRITTEN:` and an earlier `echo:` are already in the
    // transcript before the new run has produced anything at all.
    wait_for_after(
        &mut rx,
        &mut seen,
        "prompt-after-restart",
        "RECORD-WRITTEN:",
        20,
    )
    .await;
    let conversation = last_marker_value(&seen, "RECORD-WRITTEN:");
    settle_past_horizon(&h).await;
    let captured = snapshot_of(&h, &first.id).await;
    assert_eq!(
        captured.captured_conversation.as_deref(),
        Some(conversation.as_str()),
        "the fresh window captured the new run's own conversation"
    );
    assert_eq!(captured.restart_offer, farhelm_proto::RestartOffer::Resume);
}

/// Terminal reuse has to survive the case tmux's own respawn cannot carry
/// (fix-batch items 10 and 17): a TUI that dies on the ALTERNATE screen.
///
/// The shrink trick that preserves a primary-screen grid is powerless
/// here — an alternate-screen grid has no history to scroll into, which is
/// what the alternate screen IS — so the frame is captured before the kill
/// and re-emitted by the relaunched process itself
/// (`launch::LaunchSpec::preamble`). Without that, restarting an
/// agent-shaped TUI would silently discard everything the user was looking
/// at, which is exactly what SPEC.md's "the previous run's output stays in
/// scrollback" promises it will not.
///
/// The same restart also rotates the stored alt-screen snapshot: that file
/// describes the run being replaced, and leaving it behind would let the
/// NEXT natural death replay the OLD run's last screen as if it were the
/// new run's.
#[tokio::test]
async fn restarting_an_alt_screen_agent_carries_its_last_frame_into_the_new_run() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen-ignores-term"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let snapshot_path = h.state.path().join("snapshots").join(&session.id);

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "ALT-SCREEN APP", 20).await;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart a live alt-screen agent");
    wait_for_alive_status(&h.client, &session.id, 30).await;

    assert!(
        !snapshot_path.exists(),
        "the previous run's alt-screen snapshot must not outlive the run it describes, or a \
         later natural death would replay it as the NEW run's last screen"
    );

    // A fresh attachment sees the prior frame above the new run — the
    // whole promise, from the side a user actually experiences.
    h.client.detach(chan).await;
    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach after restart");
    let mut replay = Vec::new();
    wait_for_after(
        &mut rx2,
        &mut replay,
        "ALT-SCREEN APP",
        "FAKE-AGENT READY",
        30,
    )
    .await;
}

/// Pane ids are assigned by a server-wide counter that restarts at `%0`
/// with the tmux server, so a remembered `%N` can name a pane belonging to
/// a completely different session — and `respawn-pane` REPLACES the
/// process in whatever it names. Binding the target to the session as well
/// (`=<session>:.<pane>`) is what makes that unconstructible.
///
/// This drives the reuse path itself rather than the pairing in isolation:
/// two sessions whose pane ids come from the same counter, one restarted,
/// and the other's agent must be entirely undisturbed.
#[tokio::test]
async fn a_restart_respawns_only_its_own_pane() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let (restarted, _work_a) = basic_session(&h).await;
    let (bystander, _work_b) = basic_session(&h).await;

    let (chan, mut rx) = h
        .client
        .attach(&bystander.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client
        .send_input(chan, b"BYSTANDER-MARKER\r".to_vec())
        .await;
    wait_for(&mut rx, &mut seen, "BYSTANDER-MARKER", 10).await;
    let bystander_pane = pane_id_of(&sock, &format!("fh-{}", bystander.id)).await;

    h.client
        .restart_session(&restarted.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart");
    wait_for_alive_status(&h.client, &restarted.id, 30).await;

    assert_eq!(
        listed(&h.client, &bystander.id).await.status,
        SessionStatus::Alive,
        "the bystander's agent must be untouched by another session's respawn"
    );
    assert_eq!(
        pane_id_of(&sock, &format!("fh-{}", bystander.id)).await,
        bystander_pane,
        "and it must still be the same pane"
    );
    // Its terminal content is intact too — a respawn would have cleared
    // the visible grid even where it left the pane in place.
    let content = pane_capture(&sock, &format!("fh-{}", bystander.id)).await;
    assert!(
        content.contains("BYSTANDER-MARKER"),
        "the bystander's own output must survive: {content}"
    );
}

/// The reason sentinel paths are generation-scoped at all
/// (`spec_path_for_launch`/`status_path_for_spec`, both keyed on the
/// launch's generation number): a stale sentinel from an EARLIER
/// generation must never be able to paint a LATER, unrelated launch as
/// `error`, even if something failed to clean it up.
///
/// `a_restart_clears_a_previous_launch_error` already pins that a real
/// failed launch's own sentinel is deleted on a successful restart — this
/// test pins the complementary, previously-untested half: a gen-0
/// sentinel that SURVIVES (planted directly, standing in for whatever
/// cleanup bug might one day leave one behind) still cannot describe
/// gen-1, because nothing ever looks a generation-0 path up on behalf of
/// a generation-1 session. The session here never actually failed to
/// launch; the sentinel is a pure fabrication written straight to the
/// gen-0 path AFTER the restart has already moved the session to
/// generation 1, so real cleanup has nothing left to race against.
///
/// The sentinel gate (`sentinel_could_still_apply` combined with
/// `dead_or_absent` in `service.rs`) is only ever consulted for a pane
/// that is dead or gone — a live gen-1 pane never reaches it regardless of
/// what the gate would have said, which would make an assertion against a
/// still-alive gen-1 vacuous. So generation 1's agent is SIGKILLed WITHOUT
/// an annotation (its own pane pid, not `stop_session` — an annotated
/// exit is never sentinel-superseded per `sentinel_could_still_apply`'s
/// own docs, which would make the assertion vacuous the other way) before
/// either read: this is the exact state — an unannotated dead pane on the
/// current generation — that a wrongly-scoped sentinel lookup would flip
/// from `Exited` to `Error`.
///
/// Checked twice: once against the live supervisor's `ListSessions`, and
/// once again after a full supervisor handoff to a fresh process that
/// actually owns the state directory (`handoff_to_new_supervisor`; a
/// second supervisor started while the first still holds the directory
/// would come up read-only and reconcile nothing, per the comment on
/// `owns_state_dir` elsewhere in this file), which is `reload_sessions`'s
/// unconditional sentinel check — the stronger of the two reads and the
/// one most likely to re-surface a generation mismatch if the scoping
/// were ever accidentally loosened to "the session's latest sentinel"
/// instead of "this generation's".
#[tokio::test]
async fn a_stale_generation_zero_sentinel_cannot_taint_generation_one() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    assert_eq!(
        listed(&h.client, &session.id).await.status,
        SessionStatus::Alive,
        "the session must start out genuinely healthy"
    );

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart onto generation 1");
    wait_for_alive_status(&h.client, &session.id, 30).await;

    // Planted AFTER the restart above, so the restart's own cleanup of
    // generation 0's real launch files (which never failed) cannot
    // interfere with this fabricated one.
    let stale_sentinel =
        status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    std::fs::write(&stale_sentinel, "exec_failed argv0=/nope errno=2")
        .expect("plant a stale generation-0 sentinel");

    // `Alive` only means the pane hasn't died yet, not that the shim's own
    // `exec` chain has reached the real agent (`wait_for_alive_status`'s
    // own docs) — killing on that signal alone would race the shim itself
    // and reproduce the WRAPPER-failure shape
    // (`a_failed_scope_wrapper_classifies_as_error_rather_than_a_plain_exit`),
    // not the one under test here. The shim unlinks its own spec the
    // moment it has read it, strictly before exec'ing the real agent
    // (`exec_launch_spec_with_seam`'s docs), so generation 1's spec file
    // going away is the earliest reliable proof that the shim has handed
    // off and the real fake agent — not its wrapper — now owns the pane.
    // (An attach-and-wait-for-the-ready-banner alternative was tried and
    // rejected: this pane's tmux scrollback can still hold generation 0's
    // OWN ready banner from before the restart, so a naive text search
    // matches instantly against stale output rather than generation 1's.)
    let gen1_spec = spec_path_for_launch(h.state.path(), &session.id, 1);
    let shim_handoff_deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while gen1_spec.exists() {
        assert!(
            tokio::time::Instant::now() < shim_handoff_deadline,
            "generation 1's shim never consumed its own spec"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Kill generation 1's pane's OWN process directly, bypassing
    // `stop_session`, so the pane goes dead (tmux keeps a dead pane around
    // rather than tearing down the session) with no annotation — the only
    // shape that would ever even attempt a sentinel read. `kill-session`
    // would destroy the tmux session outright instead of leaving a dead
    // pane behind, which is not the state under test here.
    let sock = h.state.path().join("tmux.sock");
    let tmux_name = format!("fh-{}", session.id);
    let pid_out = tmux_query(
        &sock,
        &["display-message", "-p", "-t", &tmux_name, "#{pane_pid}"],
    )
    .await;
    let pane_pid: u32 = String::from_utf8_lossy(&pid_out.stdout)
        .trim()
        .parse()
        .expect("a live pane must report a pid");
    // SAFETY: a real, currently-live pid this test just read from tmux.
    unsafe {
        libc::kill(pane_pid as libc::pid_t, libc::SIGKILL);
    }
    wait_for_dead_pane(&sock, &tmux_name).await;

    let live_read = listed(&h.client, &session.id).await;
    assert!(
        matches!(live_read.status, SessionStatus::Exited { .. }),
        "an unannotated dead generation-1 pane must classify as Exited, not be swallowed by \
         the gen-0 sentinel: {live_read:?}"
    );

    // Hand off to a replacement supervisor that actually owns the state
    // directory, so `reload_sessions`'s unconditional sentinel check runs
    // for real rather than against a read-only reconciler.
    let Harness {
        client,
        sup,
        state,
        _tmux,
        _slot,
    } = h;
    let sup2 = handoff_to_new_supervisor(state.path(), sup, client).await;
    let client2 = connect_client(&sup2).await;
    let reloaded = listed(&client2, &session.id).await;
    assert!(
        matches!(reloaded.status, SessionStatus::Exited { .. }),
        "the stale generation-0 sentinel must still not taint generation 1 after a real, \
         owning reload: {reloaded:?}"
    );

    // `cleanup_launch_artifacts` only ever removes a launch's OWN files
    // once ITS OWN generation is classified `Error` (`service.rs`) —
    // generation 1 here is never classified `Error` and has no sentinel of
    // its own, so nothing in this path has any reason to touch generation
    // 0's leftover file. Confirmed empirically (not merely assumed) before
    // pinning: the real reload's cleanup does NOT sweep other generations'
    // files, so the plant survives untouched.
    assert!(
        stale_sentinel.exists(),
        "an unconsulted sentinel for the wrong generation is left untouched, not swept, by a \
         reload that never classified that generation as Error"
    );
}

/// PLAN_M3.md item 4 / acceptance 5's exact composition: stop, then
/// restart, then let the new run end on its own. The stop records
/// "stopped by user" on generation 0; the restart must clear that
/// annotation with the new generation (already pinned elsewhere); what is
/// untested until this is the THIRD leg — once generation 1 exits
/// NATURALLY, nothing must re-attach generation 0's annotation to it. The
/// real risk this guards against is not annotation storage (annotations
/// are intentionally kept on the session row and cleared whenever a new
/// generation opens) but a STALE-GENERATION OBSERVATION: generation 0's
/// exit or annotation being reported late and restored onto generation 1
/// despite the generation fence that is supposed to keep them apart. A bug
/// that let that happen would pass every other test here and only show up
/// in exactly this sequence.
///
/// The same invocation is used for both generations — `RestartMode::Fresh`
/// replays the session's original argv verbatim, so there is no
/// per-restart override to give the second run a different command. A
/// fixed sleep duration would race under load (generation 0 could exit
/// naturally before the stop lands, or generation 1 could exit before
/// `wait_for_alive_status` observes it), so both generations instead loop
/// on a marker FILE the test controls: `until [ -e released ]; do sleep
/// 0.2; done`, resolved against the session's own working directory.
/// Generation 0 is stopped while the marker provably does not exist yet,
/// so it cannot have exited on its own; generation 1 is left looping until
/// the test creates the marker, at which point it exits 0 naturally.
#[tokio::test]
async fn stop_then_restart_then_natural_exit_carries_no_stale_annotation() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let marker = work.path().join("released");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            "sh -c 'until [ -e released ]; do sleep 0.2; done'",
            None,
            80,
            24,
        )
        .await
        .expect("create");
    wait_for_alive_status(&h.client, &session.id, 30).await;

    h.client
        .stop_session(&session.id)
        .await
        .expect("stop the running agent");
    let stopped = wait_for_non_alive_status(&h.client, &session.id, 30).await;
    assert!(
        matches!(stopped.status, SessionStatus::Exited { .. }),
        "the stop must end generation 0: {stopped:?}"
    );
    assert_eq!(
        stopped.annotation.as_deref(),
        Some("stopped by user"),
        "the stop's annotation must be recorded where it happens, exactly like the other \
         stop-annotation tests"
    );
    assert!(
        !marker.exists(),
        "test setup: generation 0 must still be looping, not having exited on its own, when \
         it was stopped"
    );

    // The session already exited, so no live agent needs consent to stop
    // it first.
    let restarted = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await
        .expect("restart the already-exited session");
    assert_eq!(
        restarted.annotation, None,
        "a fresh generation must never inherit the previous generation's annotation"
    );
    wait_for_alive_status(&h.client, &session.id, 30).await;

    // Release generation 1's loop deliberately, rather than leaving it to
    // a timer — this is the moment, and only this moment, at which it may
    // exit on its own.
    std::fs::write(&marker, "").expect("release generation 1's loop");
    let exited = wait_for_exit_code(&h.client, &session.id, 0, 30).await;
    assert_eq!(
        exited.annotation, None,
        "a natural exit must carry no annotation at all, stale or otherwise — only a stop \
         records one"
    );
}

// ---------------------------------------------------------------------------
// Terminal tabs (PLAN_M4.md item 2)
//
// A tab is a tmux WINDOW on the session's tmux session running the user's
// login shell, rediscovered from a window marker rather than stored. The
// tests below are grouped by the promise each one pins: the launch
// contract, the refusals, close's reap, rediscovery, the marker split that
// keeps stop and restart off tabs, and the per-terminal properties that
// only become observable once a session has a second terminal at all.
// ---------------------------------------------------------------------------

/// A supervisor whose launches all run `shell` — the seam that makes a
/// tab's own launch drivable, since a tab has no invocation of its own.
///
/// Used two ways: to give the agent terminal a plain shell so the
/// conformance tests can drive BOTH terminals identically, and to give a
/// tab a shell that fails immediately so the dead-at-open-reply refusal is
/// reachable at all.
async fn harness_with_shell(shell: &str) -> Harness {
    harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_shell: Some(shell.to_string()),
            ..SupervisorSeams::default()
        },
    )
    .await
}

/// Wait for an attached SHELL terminal to be ready to accept a command.
///
/// A shell announces readiness with a prompt, whose text is the user's
/// business and not something a test may assume. So readiness is
/// established by round trip instead: send a command whose OUTPUT differs
/// from its own echo, and wait for the output. `printf 'X%sX\n' MARK`
/// echoes as its source text and prints `XMARKX`, so waiting on the latter
/// cannot be satisfied by the terminal merely echoing what was typed.
///
/// Retried rather than sent once: an interactive shell that has not
/// finished starting discards input, and there is no observable moment at
/// which it starts accepting it.
async fn wait_for_shell(
    client: &SupervisorClient,
    channel: u32,
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    marker: &str,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let command = format!("printf 'X%sX\\n' {marker}\r");
    let expected = format!("X{marker}X");
    loop {
        client
            .send_input(channel, command.clone().into_bytes())
            .await;
        let waited =
            tokio::time::timeout(Duration::from_secs(3), wait_for(rx, seen, &expected, 3)).await;
        if waited.is_ok() {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "shell on channel {channel} never answered a round trip; transcript so far:\n{}",
            String::from_utf8_lossy(seen)
        );
    }
}

/// Run `command` in an attached shell terminal and wait for `marker` in
/// its output. The caller is responsible for choosing a marker the
/// command's own echoed text does not contain (see [`wait_for_shell`]).
async fn run_in_shell(
    client: &SupervisorClient,
    channel: u32,
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    command: &str,
    marker: &str,
    secs: u64,
) {
    client
        .send_input(channel, format!("{command}\r").into_bytes())
        .await;
    wait_for(rx, seen, marker, secs).await;
}

/// Every pane on the harness's tmux server, as five `|`-separated fields:
/// `pane_id|session_name|window_id|tab_marker|agent_marker`.
///
/// The test-side mirror of the supervisor's own rediscovery query, used to
/// assert what tmux actually holds rather than trusting the supervisor's
/// report of it — which is the whole point for a feature whose only record
/// IS the tmux window marker. Both markers are included because both are
/// now READ by the supervisor: the tab marker is a tab's whole identity,
/// and the agent marker is what a pane-less reload prefers.
async fn window_rows(h: &Harness) -> Vec<String> {
    let out = tmux_query(
        &h.state.path().join("tmux.sock"),
        &[
            "list-panes",
            "-a",
            "-F",
            "#{pane_id}|#{session_name}|#{window_id}|#{@farhelm-tab}|#{@farhelm-agent}",
        ],
    )
    .await;
    assert!(
        out.status.success(),
        "listing panes failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

/// The tmux pane id backing one tab, read from tmux's own window markers.
async fn tab_pane(h: &Harness, session_id: &str, tab_id: &str) -> String {
    let want_session = format!("fh-{session_id}");
    let rows = window_rows(h).await;
    rows.iter()
        .find_map(|row| {
            let mut fields = row.split('|');
            let pane = fields.next()?;
            let session = fields.next()?;
            let _window = fields.next()?;
            let tab = fields.next()?;
            (session == want_session && tab == tab_id).then(|| pane.to_string())
        })
        .unwrap_or_else(|| panic!("no pane carries tab {tab_id}; rows:\n{}", rows.join("\n")))
}

/// The tab ids `list_sessions` currently reports for a session, in the
/// order it reports them.
async fn listed_tabs(client: &SupervisorClient, session_id: &str) -> Vec<String> {
    client
        .list_sessions()
        .await
        .expect("list")
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .unwrap_or_else(|| panic!("session {session_id} is not listed"))
        .tabs
        .iter()
        .map(|tab| tab.id.clone())
        .collect()
}

/// Write a shell script that daemonizes a long `sleep` and records its
/// pid, returning the script's path.
///
/// The `( … & )` subshell is a deliberate double fork: the intermediate
/// subshell exits immediately, so the `sleep` reparents to init and no
/// PPID closure from the tab's pane can reach it. `setsid` puts it in its
/// own session so the pty hangup that follows `kill-window` cannot reach
/// it either — without that, a survivor could die from the window's own
/// teardown and prove nothing about the reap that is supposed to have
/// killed it.
///
/// `scrub_env` additionally strips the environment (`env -i`), which
/// removes both farhelm markers and so hides the process from the marker
/// scan entirely — the accidental-daemonization shape only a cgroup can
/// reach (lore/2026-07-27-m2-process-tree-stop.md).
fn write_daemon_script(
    dir: &std::path::Path,
    name: &str,
    pid_file: &std::path::Path,
    scrub_env: bool,
) -> std::path::PathBuf {
    let path = dir.join(name);
    let scrub = if scrub_env { "env -i " } else { "" };
    std::fs::write(
        &path,
        format!(
            "( setsid {scrub}/bin/sh -c 'echo $$ > {pid}; exec sleep 120' \
             </dev/null >/dev/null 2>&1 & )\n",
            pid = pid_file.display()
        ),
    )
    .expect("writing the daemonizer script");
    path
}

/// A tab opened in a session runs the user's shell in the SESSION's
/// working directory, is attachable by the id the open returned, and
/// appears in that session's authoritative tab list (PLAN_M4.md
/// acceptance 1).
///
/// The working directory is checked by asking the shell rather than by
/// inspecting tmux: what SPEC.md promises is that a command typed in the
/// tab shows the session's cwd, and only running one proves the `-c` was
/// applied to the process rather than merely to the window's metadata.
#[tokio::test]
async fn a_tab_runs_a_shell_in_the_sessions_working_directory() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![tab.id.clone()],
        "an opened tab must appear in the session's own tab list, which is the one place \
         ordering is authoritative"
    );

    let (chan, mut rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "tab-lease",
        )
        .await
        .expect("attach the tab");
    let mut seen = Vec::new();
    wait_for_shell(&h.client, chan, &mut rx, &mut seen, "READY").await;

    // The session's cwd is a tempdir whose path may be a symlink (macOS
    // aside, `/tmp` is one on some Linux setups), so compare against what
    // the shell itself resolves rather than against the literal path.
    run_in_shell(
        &h.client,
        chan,
        &mut rx,
        &mut seen,
        "printf 'CW%s[%s]\\n' D \"$PWD\"",
        "CWD[",
        20,
    )
    .await;
    let transcript = String::from_utf8_lossy(&seen).into_owned();
    let reported = transcript
        .split("CWD[")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .expect("the tab shell must report its working directory")
        .trim()
        .to_string();
    let expected = std::fs::canonicalize(work.path()).expect("canonical workdir");
    assert_eq!(
        std::fs::canonicalize(&reported).expect("canonical reported cwd"),
        expected,
        "a tab must start in the session's working directory, not the supervisor's"
    );
}

/// Opening a tab when the session's working directory has vanished fails
/// with an error NAMING the directory, and leaves the session untouched
/// (PLAN_M4.md acceptance 4).
///
/// The same precondition — and deliberately the same error shape — restart
/// makes, reused unchanged rather than reworded: a user who has seen one
/// of these refusals should recognize the other.
#[tokio::test]
async fn opening_a_tab_after_the_working_directory_vanished_names_it() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let cwd = work.path().to_string_lossy().into_owned();
    let session = h
        .client
        .create_session(
            &cwd,
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    work.close()
        .expect("removing the session's working directory");

    let err = h
        .client
        .open_tab(&session.id)
        .await
        .expect_err("a vanished working directory must refuse the open");
    assert!(
        err.to_string().contains(&cwd),
        "the refusal must name the directory that vanished, got: {err:#}"
    );
    assert!(
        listed_tabs(&h.client, &session.id).await.is_empty(),
        "a refused open must leave no tab behind"
    );
    // The session itself is untouched: its agent is still answering.
    h.client.send_input(_chan, b"still-here\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "still-here", 15).await;
}

/// Opening a tab on a session whose tmux session no longer exists is
/// refused with advice to restart the session first (PLAN_M4.md
/// acceptance 4).
///
/// Building a tab-only tmux session for an agent-less session would be a
/// strange half-alive state this system deliberately does not have, and
/// SPEC.md already puts re-adding tabs after the user's own restart. The
/// refusal has to SAY that, because "no such terminal" alone leaves the
/// user with no next step.
#[tokio::test]
async fn opening_a_tab_without_a_tmux_session_says_to_restart_first() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let sock = h.state.path().join("tmux.sock");
    let killed = tmux_query(
        &sock,
        &["kill-session", "-t", &format!("fh-{}", session.id)],
    )
    .await;
    assert!(
        killed.status.success(),
        "test setup: killing the session's tmux session must succeed: {}",
        String::from_utf8_lossy(&killed.stderr)
    );

    let err = h
        .client
        .open_tab(&session.id)
        .await
        .expect_err("a session with no tmux session cannot gain a tab");
    let rendered = format!("{err:#}");
    assert!(
        rendered.contains("restart"),
        "the refusal must point at restarting the session, got: {rendered}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a refused open must carry a SupervisorError")
            .kind,
        ErrorKind::Conflict,
        "a session in the wrong state for this operation is a conflict, not a missing thing"
    );
}

/// A tab whose shell is already dead by the time the open would reply is
/// a REFUSED open carrying the pane's last words, with the window cleaned
/// up — never a silently "successful" tab holding a corpse (PLAN_M4.md
/// acceptance 4, SPEC.md's every-failed-operation rule).
///
/// The shell seam is what makes this reachable: a tab has no invocation of
/// its own, so the only way to drive its launch into failure is to choose
/// the shell. The fixture prints a recognizable line and exits, which is
/// exactly the shape a broken login shell has.
#[tokio::test]
async fn a_tab_whose_shell_is_dead_by_reply_time_is_refused_with_its_last_words() {
    let dying = tempfile::tempdir().unwrap();
    let shell = dying.path().join("dying-shell");
    std::fs::write(&shell, "#!/bin/sh\necho SHELL-REFUSED-TO-START\nexit 9\n")
        .expect("writing the failing shell fixture");
    std::fs::set_permissions(
        &shell,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .expect("making the failing shell executable");

    let h = harness_with_shell(&shell.to_string_lossy()).await;
    let work = tempfile::tempdir().unwrap();
    // The AGENT's own launch also runs through this shell, so it is given
    // a command it never reaches — this test is about the tab, and the
    // session only has to exist and hold a tmux session.
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
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let err = h
        .client
        .open_tab(&session.id)
        .await
        .expect_err("a shell that is already dead must refuse the open");
    let rendered = format!("{err:#}");
    assert!(
        rendered.contains("SHELL-REFUSED-TO-START"),
        "the refusal must carry the pane's last words as its detail, got: {rendered}"
    );
    assert!(
        listed_tabs(&h.client, &session.id).await.is_empty(),
        "a refused open must clean its window up rather than leave a dead tab listed"
    );
    let rows = window_rows(&h).await;
    assert!(
        !rows
            .iter()
            .any(|row| row.contains(&format!("fh-{}|", session.id))
                && row.split('|').nth(3).is_some_and(|tab| !tab.is_empty())),
        "no marked tab window may survive a refused open; rows:\n{}",
        rows.join("\n")
    );
}

/// Closing a tab kills its shell AND a deliberately daemonized child of
/// that shell, while the agent terminal and the session's OTHER tab are
/// untouched (PLAN_M4.md acceptance 3).
///
/// The daemonized child is the whole point: it has reparented to init, so
/// no PPID walk from the pane reaches it, and only the tab's own marker
/// scan can. The second tab and the agent are the other half — a close
/// that reaped by the SESSION's marker instead of the tab's would end all
/// three and still look like a pass without them.
#[tokio::test]
async fn closing_a_tab_kills_its_shell_and_daemonized_child_and_nothing_else() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent terminal");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;

    let doomed = h.client.open_tab(&session.id).await.expect("open the tab");
    let survivor = h
        .client
        .open_tab(&session.id)
        .await
        .expect("open a second tab");

    let (doomed_chan, mut doomed_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: doomed.id.clone(),
            },
            "one-client",
        )
        .await
        .expect("attach the doomed tab");
    let mut doomed_seen = Vec::new();
    wait_for_shell(
        &h.client,
        doomed_chan,
        &mut doomed_rx,
        &mut doomed_seen,
        "D",
    )
    .await;

    let (survivor_chan, mut survivor_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: survivor.id.clone(),
            },
            "one-client",
        )
        .await
        .expect("attach the surviving tab");
    let mut survivor_seen = Vec::new();
    wait_for_shell(
        &h.client,
        survivor_chan,
        &mut survivor_rx,
        &mut survivor_seen,
        "S",
    )
    .await;

    let doomed_pid_file = work.path().join("doomed-daemon.pid");
    let script = write_daemon_script(work.path(), "doomed.sh", &doomed_pid_file, false);
    run_in_shell(
        &h.client,
        doomed_chan,
        &mut doomed_rx,
        &mut doomed_seen,
        &format!("sh {} && printf 'SPAWN%sED\\n' N", script.display()),
        "SPAWNNED",
        20,
    )
    .await;
    let daemon_pid = wait_for_pid_file(&doomed_pid_file, 10).await;
    let _daemon_cleanup = PidKillGuard::arm(daemon_pid);
    let doomed_pane_pid = pane_pid_of(&h, &tab_pane(&h, &session.id, &doomed.id).await).await;

    h.client
        .close_tab(&session.id, &doomed.id)
        .await
        .expect("close the tab");

    wait_until_pid_gone(doomed_pane_pid, 15).await;
    wait_until_pid_gone(daemon_pid, 15).await;
    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![survivor.id.clone()],
        "closing one tab must leave the session's other tabs listed"
    );
    // The surviving tab and the agent both still answer, which is the
    // only proof their processes were never reaped.
    run_in_shell(
        &h.client,
        survivor_chan,
        &mut survivor_rx,
        &mut survivor_seen,
        "printf 'SURVIV%sR\\n' O",
        "SURVIVOR",
        20,
    )
    .await;
    h.client
        .send_input(agent_chan, b"agent-untouched\r".to_vec())
        .await;
    wait_for(&mut agent_rx, &mut agent_seen, "agent-untouched", 15).await;
}

/// The pid of the process tmux reports for `pane`.
async fn pane_pid_of(h: &Harness, pane: &str) -> u32 {
    let out = tmux_query(
        &h.state.path().join("tmux.sock"),
        &["display-message", "-p", "-t", pane, "#{pane_pid}"],
    )
    .await;
    assert!(
        out.status.success(),
        "querying a pane's pid failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .expect("tmux reports a numeric pane pid")
}

/// The cgroup half of a tab close (PLAN_M4.md item 2), loudly skipped
/// where no user manager exists — M3's pattern, for M3's reason.
///
/// The fixture is the one shape neither half of the marker sweep can
/// reach: double-forked to init (so no PPID walk from the tab's pane
/// finds it) AND environment-scrubbed (so the tab-marker scan cannot see
/// it either). Its death can therefore only have come from the tab's own
/// `systemd-run --scope`, which is the whole claim this test makes.
///
/// A tab's scope is not recorded anywhere — tabs have no durable row — so
/// this also pins that `close_tab` re-derives the same unit name the open
/// created, from the session id and the tab id alone.
#[tokio::test]
async fn closing_a_tab_kills_an_environment_scrubbed_double_fork_through_its_scope() {
    if !cgroup_path_available(
        "closing_a_tab_kills_an_environment_scrubbed_double_fork_through_its_scope",
    )
    .await
    {
        return;
    }
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (chan, mut rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "tab-lease",
        )
        .await
        .expect("attach the tab");
    let mut seen = Vec::new();
    wait_for_shell(&h.client, chan, &mut rx, &mut seen, "READY").await;

    let pid_file = work.path().join("cloaked-tab.pid");
    let script = write_daemon_script(work.path(), "cloak.sh", &pid_file, true);
    run_in_shell(
        &h.client,
        chan,
        &mut rx,
        &mut seen,
        &format!("sh {} && printf 'CLOAK%sD\\n' E", script.display()),
        "CLOAKED",
        20,
    )
    .await;
    let cloaked = wait_for_pid_file(&pid_file, 10).await;
    let _cloaked_cleanup = PidKillGuard::arm(cloaked);
    assert!(
        !marked_pids(&session.id).contains(&cloaked),
        "test setup: the cloaked daemon must NOT carry the session marker — the whole point \
         is that only a cgroup can find it"
    );

    h.client
        .close_tab(&session.id, &tab.id)
        .await
        .expect("close the tab");
    wait_until_pid_gone(cloaked, 15).await;
}

/// Tabs survive a supervisor restart by the same mechanism the agent
/// terminal does — tmux outliving the supervisor — and a window someone
/// conjured behind the supervisor's back is never reported as one
/// (PLAN_M4.md acceptance 2 and 4).
///
/// The unmarked window is not a hypothetical: a pane's own processes
/// inherit `TMUX` and can create windows on the private server, which is
/// exactly why rediscovery is marker-based rather than positional. Here it
/// is created directly against the same socket, which is the same thing
/// from the supervisor's point of view.
///
/// Both tabs are checked, in order, because ordering is the one thing
/// `SessionInfo::tabs` promises beyond identity — and a rediscovery that
/// rebuilt the list from a hash map would pass an identity-only assertion
/// while shuffling the user's tab strip on every poll.
#[tokio::test]
async fn tabs_are_rediscovered_across_a_supervisor_restart_and_unmarked_windows_are_ignored() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let work = tempfile::tempdir().expect("workdir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));

    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    let client = connect_client(&sup).await;
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
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let first = client.open_tab(&session.id).await.expect("open the first");
    let second = client.open_tab(&session.id).await.expect("open the second");

    // A window the supervisor never made, on its own private server.
    let sock = state.path().join("tmux.sock");
    let conjured = tmux_query(
        &sock,
        &[
            "new-window",
            "-d",
            "-P",
            "-F",
            "#{window_id}",
            "-t",
            &format!("=fh-{}:", session.id),
            "--",
            "sleep",
            "300",
        ],
    )
    .await;
    assert!(
        conjured.status.success(),
        "test setup: conjuring an unmarked window must succeed: {}",
        String::from_utf8_lossy(&conjured.stderr)
    );

    assert_eq!(
        listed_tabs(&client, &session.id).await,
        vec![first.id.clone(), second.id.clone()],
        "an unmarked window must never appear as a tab, and tabs list in creation order"
    );

    // Restart the supervisor over the same state directory, tmux and all
    // its windows untouched — the ordinary supervisor-restart shape.
    drop(client);
    drop(sup);
    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor");
    let client = connect_client(&sup).await;

    assert_eq!(
        listed_tabs(&client, &session.id).await,
        vec![first.id.clone(), second.id.clone()],
        "tabs must be rediscovered from their window markers across a supervisor restart, in \
         the same order"
    );

    // Attachable, not merely listed: the ids the rediscovery reported have
    // to be the ones the attach machinery resolves.
    let (chan, mut rx) = client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: second.id.clone(),
            },
            "after-restart",
        )
        .await
        .expect("attach a rediscovered tab");
    let mut seen = Vec::new();
    wait_for_shell(&client, chan, &mut rx, &mut seen, "REDISCOVERED").await;
    drop(slot);
}

/// Stopping the agent leaves a tab's shell AND its daemonized child
/// running (PLAN_M4.md acceptance 3, SPEC.md's "terminal tabs keep
/// running").
///
/// This is the marker split's whole reason to exist. Tab processes carry
/// the session marker the stop sweep is keyed on, so a stop that did not
/// subtract them would reap the very terminals SPEC.md promises survive
/// it. The daemonized child is what makes the assertion sharp: the tab's
/// shell might survive by luck of ancestry, but a reparented daemon is
/// reachable ONLY by the marker scan, so its survival is a statement about
/// the marker rule and nothing else.
///
/// The agent's own daemonized child is asserted dead in the same run, so a
/// stop that had simply stopped sweeping at all could not pass.
#[tokio::test]
async fn stopping_the_agent_leaves_a_tabs_shell_and_its_daemonized_child_running() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-reparent"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_agent_chan, mut agent_rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let agent_pid = extract_pid(&agent_seen, "SELF-PID:");
    let agent_daemon = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (tab_chan, mut tab_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "tab-lease",
        )
        .await
        .expect("attach the tab");
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    let tab_pid_file = work.path().join("tab-daemon.pid");
    let script = write_daemon_script(work.path(), "tab-daemon.sh", &tab_pid_file, false);
    run_in_shell(
        &h.client,
        tab_chan,
        &mut tab_rx,
        &mut tab_seen,
        &format!("sh {} && printf 'SPAWN%sED\\n' N", script.display()),
        "SPAWNNED",
        20,
    )
    .await;
    let tab_daemon = wait_for_pid_file(&tab_pid_file, 10).await;
    let _tab_daemon_cleanup = PidKillGuard::arm(tab_daemon);
    assert!(
        marked_pids(&session.id).contains(&tab_daemon),
        "test setup: the tab's daemon must carry the SESSION marker — surviving without it \
         would prove nothing about the exclusion rule"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    // The agent's side of the sweep still works.
    wait_until_pid_gone(agent_pid, 15).await;
    wait_until_pid_gone(agent_daemon, 15).await;

    // The tab's shell is still answering, and its daemon is still alive.
    run_in_shell(
        &h.client,
        tab_chan,
        &mut tab_rx,
        &mut tab_seen,
        "printf 'AFTER%sTOP\\n' S",
        "AFTERSTOP",
        20,
    )
    .await;
    assert!(
        !process_is_gone(tab_daemon),
        "stop must not reach a tab's daemonized child (pid {tab_daemon})"
    );
}

/// Restarting the agent touches the agent terminal alone: its attachment
/// is detached, while a tab stays attached, keeps answering, and keeps its
/// daemonized child (PLAN_M4.md acceptance 3, SPEC.md's "restart touches
/// the agent terminal only").
///
/// Two independent mechanisms have to hold for this and are asserted
/// together on purpose, because either one failing produces the same
/// user-visible complaint: the detach sweep is scoped to the agent's
/// attachment key, and the restart's pre-relaunch reap subtracts tab
/// processes.
#[tokio::test]
async fn restarting_the_agent_leaves_a_tab_attached_running_and_unswept() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
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
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (tab_chan, mut tab_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "one-client",
        )
        .await
        .expect("attach the tab");
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    let pid_file = work.path().join("tab-daemon.pid");
    let script = write_daemon_script(work.path(), "tab-daemon.sh", &pid_file, false);
    run_in_shell(
        &h.client,
        tab_chan,
        &mut tab_rx,
        &mut tab_seen,
        &format!("sh {} && printf 'SPAWN%sED\\n' N", script.display()),
        "SPAWNNED",
        20,
    )
    .await;
    let tab_daemon = wait_for_pid_file(&pid_file, 10).await;
    let _tab_daemon_cleanup = PidKillGuard::arm(tab_daemon);

    let restarted = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart");
    assert_eq!(
        restarted
            .tabs
            .iter()
            .map(|t| t.id.clone())
            .collect::<Vec<_>>(),
        vec![tab.id.clone()],
        "the restart's own reply must still report the tabs the restart did not touch"
    );

    // The AGENT's attachment is gone, and told why.
    let reason = expect_detached(&mut agent_rx, 15).await;
    assert!(
        reason.contains("restart"),
        "the agent's attachment must be detached for the restart, got: {reason:?}"
    );
    let _ = agent_chan;

    // The TAB's attachment survived: still delivering, on the same channel.
    run_in_shell(
        &h.client,
        tab_chan,
        &mut tab_rx,
        &mut tab_seen,
        "printf 'AFTER%sESTART\\n' R",
        "AFTERRESTART",
        20,
    )
    .await;
    assert!(
        !process_is_gone(tab_daemon),
        "a restart's pre-relaunch reap must not reach a tab's daemonized child (pid \
         {tab_daemon})"
    );
    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![tab.id.clone()],
        "a restart must leave the session's tabs listed"
    );
}

/// Deleting a session takes the agent, its tabs, and their daemonized
/// descendants (PLAN_M4.md acceptance 3).
///
/// The other side of the marker split: delete sweeps the session marker
/// INCLUSIVELY, so the exclusion that protects tabs from stop must not
/// leak into this path. A daemonized child of the tab is what proves it
/// reached past the tmux teardown, which would have killed the shell
/// regardless.
#[tokio::test]
async fn deleting_a_session_takes_its_tabs_and_their_daemonized_descendants() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (chan, mut rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "tab-lease",
        )
        .await
        .expect("attach the tab");
    let mut seen = Vec::new();
    wait_for_shell(&h.client, chan, &mut rx, &mut seen, "READY").await;

    let pid_file = work.path().join("tab-daemon.pid");
    let script = write_daemon_script(work.path(), "tab-daemon.sh", &pid_file, false);
    run_in_shell(
        &h.client,
        chan,
        &mut rx,
        &mut seen,
        &format!("sh {} && printf 'SPAWN%sED\\n' N", script.display()),
        "SPAWNNED",
        20,
    )
    .await;
    let tab_daemon = wait_for_pid_file(&pid_file, 10).await;
    let _tab_daemon_cleanup = PidKillGuard::arm(tab_daemon);
    let tab_pane_pid = pane_pid_of(&h, &tab_pane(&h, &session.id, &tab.id).await).await;

    h.client.delete_session(&session.id).await.expect("delete");

    wait_until_pid_gone(tab_pane_pid, 15).await;
    wait_until_pid_gone(tab_daemon, 15).await;
    assert!(
        marked_pids(&session.id).is_empty(),
        "delete must leave nothing carrying this session's marker"
    );
}

/// A session whose AGENT runs the same plain login shell a tab does, so
/// the conformance battery below can drive both terminals through one
/// code path instead of two.
///
/// The parameterization is the point: "a tab is the same terminal
/// machinery as the agent pane" is a claim, and the honest way to test a
/// claim of sameness is to run the same program through both and assert
/// the same properties, rather than to write a tab-shaped copy of each
/// agent-shaped test and hope they stayed equivalent.
async fn shell_session(h: &Harness) -> (SessionInfo, tempfile::TempDir) {
    let work = tempfile::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(&work.path().to_string_lossy(), "/bin/sh -i", None, 80, 24)
        .await
        .expect("create a shell session");
    (session, work)
}

/// The `COLSxROWS` tmux reports for the window containing `pane`.
async fn window_geometry(h: &Harness, pane: &str) -> String {
    let out = tmux_query(
        &h.state.path().join("tmux.sock"),
        &[
            "display-message",
            "-p",
            "-t",
            pane,
            "#{window_width}x#{window_height}",
        ],
    )
    .await;
    assert!(
        out.status.success(),
        "querying a window's geometry failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Every terminal-fidelity promise SPEC.md makes, asserted once for the
/// AGENT terminal and once for a TAB of the same session — replay of
/// scrollback across a reattach, alternate-screen selection, pane-mode
/// restoration, and binary-clean live output.
///
/// Parameterized rather than duplicated (PLAN_M4.md's testing decisions):
/// the property under test is that the two are the SAME machinery, and a
/// second tab-shaped copy of each assertion would be free to drift from
/// the agent-shaped one it was supposed to mirror.
///
/// "The same program in both" is made true rather than assumed: the agent
/// is created with `/bin/sh -i` as its invocation AND the tab's own shell
/// is pinned to `/bin/sh` through `launch_shell`, since a tab otherwise
/// launches whatever this host's `$SHELL` resolves to and the two
/// terminals would be running different programs while claiming to
/// demonstrate sameness.
///
/// The battery is deliberately the REPLAY/MODE/BINARY subset — scrollback
/// replay across a reattach, bracketed-paste restoration, alternate-screen
/// selection, and byte-clean live output. Per-window resize and stall
/// scoping are the two conformance properties that are only meaningful
/// BETWEEN terminals rather than within one, and they have tests of their
/// own for that reason.
#[tokio::test]
async fn terminal_conformance_holds_for_the_agent_and_for_a_tab() {
    let h = harness_with_shell("/bin/sh").await;
    let (session, _work) = shell_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let targets = [
        ("agent", TerminalSelector::Agent),
        ("tab", TerminalSelector::Tab { id: tab.id.clone() }),
    ];

    for (label, selector) in targets {
        let (chan, mut rx) = h
            .client
            .attach_terminal(&session.id, 80, 24, selector.clone(), "conformance")
            .await
            .unwrap_or_else(|e| panic!("{label}: attach failed: {e:#}"));
        let mut seen = Vec::new();
        wait_for_shell(&h.client, chan, &mut rx, &mut seen, "READY").await;

        // Binary-clean live output: invalid UTF-8 is legitimate terminal
        // content and must cross the control-mode stream byte-for-byte.
        // Anchored on an adjacent marker rather than on the byte itself,
        // since a lossy conversion would REPLACE the byte while leaving
        // everything around it intact — which is exactly the bug.
        run_in_shell(
            &h.client,
            chan,
            &mut rx,
            &mut seen,
            "printf 'BIN\\377ARY%s\\n' -END",
            "ARY-END",
            20,
        )
        .await;
        assert!(
            seen.contains(&0xff),
            "{label}: live output replaced or dropped an invalid UTF-8 byte"
        );

        // Enough output to push the earliest of it off an 80x24 screen,
        // so the reattach replay below has to come from tmux HISTORY
        // rather than from the visible grid.
        run_in_shell(
            &h.client,
            chan,
            &mut rx,
            &mut seen,
            "i=0; while [ $i -lt 60 ]; do printf 'SCROLL%s-%s\\n' ED $i; i=$((i+1)); done",
            "SCROLLED-59",
            20,
        )
        .await;
        // Bracketed paste, the audited silent-loss mode (SPEC_impl.md):
        // content replay alone passing this test would be the bug.
        h.client
            .send_input(chan, b"printf '\\033[?2004h'\r".to_vec())
            .await;
        run_in_shell(
            &h.client,
            chan,
            &mut rx,
            &mut seen,
            "printf 'MODE%s\\n' -SET",
            "MODE-SET",
            20,
        )
        .await;

        h.client.detach(chan).await;
        let (chan2, mut rx2) = h
            .client
            .attach_terminal(&session.id, 80, 24, selector.clone(), "conformance")
            .await
            .unwrap_or_else(|e| panic!("{label}: reattach failed: {e:#}"));
        let mut replay = Vec::new();
        wait_for(&mut rx2, &mut replay, "SCROLLED-0", 20).await;
        assert!(
            String::from_utf8_lossy(&replay).contains("SCROLLED-59"),
            "{label}: replay lost the tail of the pre-detach history"
        );
        if tmux_has_format(&h, "bracket_paste_flag").await {
            wait_for(&mut rx2, &mut replay, "\x1b[?2004h", 10).await;
        } else {
            eprintln!("tmux lacks bracket_paste_flag; skipping {label} mode restoration");
        }
        // Live after replay, not just replayed: a control-client overlap
        // renders the replay and then never updates, and only fresh
        // output tells a live terminal from a frozen one.
        run_in_shell(
            &h.client,
            chan2,
            &mut rx2,
            &mut replay,
            "printf 'LIVE%sREATTACH\\n' -AFTER-",
            "LIVE-AFTER-REATTACH",
            20,
        )
        .await;

        // Alternate screen: the replay must select the alternate buffer
        // BEFORE prefilling it (the switch clears what it switches to) and
        // must not mix the normal screen's history in.
        run_in_shell(
            &h.client,
            chan2,
            &mut rx2,
            &mut replay,
            "printf '\\033[?1049hALT%sSCREEN\\n' -",
            "ALT-SCREEN",
            20,
        )
        .await;
        h.client.detach(chan2).await;
        let (chan3, mut rx3) = h
            .client
            .attach_terminal(&session.id, 80, 24, selector.clone(), "conformance")
            .await
            .unwrap_or_else(|e| panic!("{label}: alt-screen reattach failed: {e:#}"));
        let mut alt = Vec::new();
        wait_for(&mut rx3, &mut alt, "ALT-SCREEN", 20).await;
        let alt_text = String::from_utf8_lossy(&alt).into_owned();
        assert!(
            alt_text.contains("\x1b[?1049h"),
            "{label}: an alternate-screen pane's replay must select that buffer first"
        );
        assert!(
            !alt_text.contains("SCROLLED-0"),
            "{label}: alternate-screen replay must not mix in the normal screen's history"
        );
        h.client
            .send_input(chan3, b"printf '\\033[?1049l'\r".to_vec())
            .await;
        h.client.detach(chan3).await;
    }
}

/// A resize reflows ONLY the window of the terminal whose channel carried
/// it (PLAN_M4.md item 3: resize goes per window).
///
/// Before tabs, `resize-window` was targeted at the tmux SESSION, which
/// resolves to whichever window tmux last made current — unambiguous with
/// one window and silently wrong with two. This is the test that would
/// have caught that: it resizes each terminal in turn and requires the
/// other's geometry to stay put.
#[tokio::test]
async fn a_resize_reflows_only_the_named_terminals_window() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let agent_pane = pane_id_of(
        &h.state.path().join("tmux.sock"),
        &format!("fh-{}", session.id),
    )
    .await;
    let tab_pane = tab_pane(&h, &session.id, &tab.id).await;

    let (agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let (tab_chan, mut tab_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "one-client",
        )
        .await
        .expect("attach the tab");
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    h.client.resize(&session.id, tab_chan, 111, 33).await;
    wait_for_pane_geometry(&h, &tab_pane, "111x33").await;
    assert_eq!(
        window_geometry(&h, &agent_pane).await,
        "80x24",
        "resizing a tab must not reflow the agent's window"
    );

    h.client.resize(&session.id, agent_chan, 90, 30).await;
    wait_for_pane_geometry(&h, &agent_pane, "90x30").await;
    assert_eq!(
        window_geometry(&h, &tab_pane).await,
        "111x33",
        "resizing the agent terminal must not reflow a tab's window"
    );
}

/// Poll one window's geometry until it reaches `expected`. Resize is
/// fire-and-forget, so polling is the only observation available — the
/// per-window counterpart of [`wait_for_geometry`].
async fn wait_for_pane_geometry(h: &Harness, pane: &str, expected: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let got = window_geometry(h, pane).await;
        if got == expected {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "window of pane {pane} never reached {expected} (last: {got})"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Input sent on one terminal's channel reaches ONLY that terminal's pane
/// (PLAN_M4.md item 3).
///
/// Both channels belong to one client and one connection, so nothing but
/// the attachment key distinguishes them — which is exactly the thing a
/// regression here would collapse. Each terminal's transcript is checked
/// for the OTHER's marker as well as for its own, because "it arrived"
/// and "it arrived only here" are different claims.
#[tokio::test]
async fn input_reaches_only_the_terminal_it_was_sent_to() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let (tab_chan, mut tab_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "one-client",
        )
        .await
        .expect("attach the tab");
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    h.client
        .send_input(agent_chan, b"for-the-agent-only\r".to_vec())
        .await;
    wait_for(&mut agent_rx, &mut agent_seen, "for-the-agent-only", 15).await;
    run_in_shell(
        &h.client,
        tab_chan,
        &mut tab_rx,
        &mut tab_seen,
        "printf 'FORTHE%sONLY\\n' TAB",
        "FORTHETABONLY",
        20,
    )
    .await;

    // Settle before asserting an ABSENCE: the other terminal's frames are
    // in flight over the same connection, and a single read could simply
    // have run ahead of them.
    let _ = drain_for(&mut agent_rx, &mut agent_seen, Duration::from_secs(1)).await;
    let _ = drain_for(&mut tab_rx, &mut tab_seen, Duration::from_secs(1)).await;
    assert!(
        !String::from_utf8_lossy(&agent_seen).contains("FORTHETABONLY"),
        "the tab's output leaked into the agent terminal's stream"
    );
    assert!(
        !String::from_utf8_lossy(&tab_seen).contains("for-the-agent-only"),
        "the agent terminal's output leaked into the tab's stream"
    );
}

/// A stalled viewer on one TAB pauses only that tab's stream: the agent
/// terminal keeps flowing throughout (PLAN_M4.md acceptance 5).
///
/// This is the reason per-terminal control clients exist rather than one
/// per session. tmux's `pause-after` flow control is a property of a
/// CONTROL CLIENT, so a client shared across a session's terminals would
/// let one wedged tab viewer pause the agent's stream — the terminal the
/// user is actually looking at.
///
/// The agent is round-tripped repeatedly for longer than
/// `TMUX_PAUSE_AFTER_SECS`, deliberately: the interesting window is the
/// one where tmux's own backstop fires on the stalled client, and a check
/// that finished before that would miss the very interaction under test.
///
/// Honest scope, because the property is not absolute. tmux answers a
/// lagging client in one of two ways and picks between them
/// nondeterministically (see `TMUX_PAUSE_AFTER_SECS`): it either cuts that
/// client with `%pause`, which is the fully isolated branch this test
/// usually observes, or it THROTTLES the pane, which can transiently slow
/// the session's other panes until the stall bound trips. So a pass here
/// says the supervisor's per-terminal machinery holds under the branch
/// tmux took on this run — not that both branches are isolation-free. The
/// residual is documented on `tmux::OutputStream` and is closed by the
/// session-sink design rather than by anything this test can assert.
#[tokio::test]
async fn a_stalled_tab_viewer_does_not_pause_the_agents_stream() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let (tab_chan, mut tab_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "one-client",
        )
        .await
        .expect("attach the tab");
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    // Stall the tab's viewer, then give it something enormous to be
    // behind on. Input still flows to a paused terminal — only its OUTPUT
    // is held — which is what lets the producer be started at all.
    h.client.pause_output(tab_chan).await;
    h.client
        .send_input(
            tab_chan,
            b"i=0; while [ $i -lt 200000 ]; do printf 'FLOOD-%s\\n' $i; i=$((i+1)); done\r"
                .to_vec(),
        )
        .await;

    let until = tokio::time::Instant::now() + Duration::from_secs(8);
    let mut round = 0;
    while tokio::time::Instant::now() < until {
        round += 1;
        let marker = format!("agent-alive-{round}");
        h.client
            .send_input(agent_chan, format!("{marker}\r").into_bytes())
            .await;
        wait_for(&mut agent_rx, &mut agent_seen, &marker, 10).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(
        round > 1,
        "the agent must have been round-tripped several times while the tab was stalled"
    );

    // And the stalled tab recovers on resume rather than staying dead —
    // the other half of the isolation claim.
    h.client.resume_output(tab_chan).await;
    wait_for(&mut tab_rx, &mut tab_seen, "FLOOD-", 30).await;
}

/// A tab's launch evaluates the environment at OPEN time, so an rc-file
/// change made between two opens is visible to the second (SPEC.md's
/// environment contract, extended to tabs by PLAN_M4.md item 2's
/// same-interactive-login-contract rule).
///
/// The agent-side version of this promise is already pinned
/// (`an_rc_file_change_between_launches_reaches_the_relaunched_agent`);
/// this is the half that would silently break if a tab ever resolved its
/// shell once and cached it, or launched through anything other than an
/// interactive login shell.
///
/// The rc files live in a private HOME injected through
/// `SupervisorSeams::launch_env` — never by mutating this process's
/// environment, which this repo forbids and which every concurrently
/// running harness would share anyway. A host whose login shell reads
/// none of the files [`write_rc_files`] knows how to write is an honest,
/// loud skip rather than a silent pass.
#[tokio::test]
async fn an_rc_file_change_between_two_tab_opens_reaches_the_second_tab() {
    let home = tempfile::tempdir().expect("fixture home");
    write_rc_files(home.path(), "first");
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_env: vec![
                (
                    "HOME".to_string(),
                    home.path().to_string_lossy().into_owned(),
                ),
                (
                    "ZDOTDIR".to_string(),
                    home.path().to_string_lossy().into_owned(),
                ),
                (
                    "ENV".to_string(),
                    home.path().join(".shinit").to_string_lossy().into_owned(),
                ),
            ],
            ..SupervisorSeams::default()
        },
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    /// Open a tab, attach it, and ask its shell what the marker holds.
    async fn tab_marker_value(h: &Harness, session_id: &str, ready: &str) -> String {
        let tab = h.client.open_tab(session_id).await.expect("open a tab");
        let (chan, mut rx) = h
            .client
            .attach_terminal(
                session_id,
                80,
                24,
                TerminalSelector::Tab { id: tab.id.clone() },
                "rc-lease",
            )
            .await
            .expect("attach the tab");
        let mut seen = Vec::new();
        wait_for_shell(&h.client, chan, &mut rx, &mut seen, ready).await;
        run_in_shell(
            &h.client,
            chan,
            &mut rx,
            &mut seen,
            &format!("printf 'EN%s[%s]\\n' V \"${RC_MARKER_VAR}\""),
            "ENV[",
            20,
        )
        .await;
        h.client.detach(chan).await;
        let text = String::from_utf8_lossy(&seen).into_owned();
        text.split("ENV[")
            .nth(1)
            .and_then(|rest| rest.split(']').next())
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    let first = tab_marker_value(&h, &session.id, "ONE").await;
    if first != "first" {
        // Deterministic, not a shrug — same reasoning as the agent-side
        // test: for every shell family `write_rc_files` covers, the value
        // MUST have arrived, so anything else is a host this harness
        // genuinely cannot reach, named so the gap is diagnosable.
        let shell = farhelm_supervisor::launch::resolve_shell().await;
        let family = std::path::Path::new(&shell)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| shell.clone());
        assert!(
            !["bash", "zsh", "sh", "dash", "ksh"].contains(&family.as_str()),
            "tabs launch through {shell}, which sources one of the rc files this test writes, \
             so the first tab should have seen the value; it reported {first:?} instead"
        );
        eprintln!(
            "SKIPPED an_rc_file_change_between_two_tab_opens_reaches_the_second_tab: this host \
             launches tabs through {shell}, which sources none of the rc files this test knows \
             how to write"
        );
        return;
    }

    write_rc_files(home.path(), "second");
    assert_eq!(
        tab_marker_value(&h, &session.id, "TWO").await,
        "second",
        "a tab opened after an rc-file edit must see the edit — the environment is evaluated \
         at each launch, not resolved once per supervisor"
    );
}

// ---------------------------------------------------------------------------
// The marker model (PLAN_M4.md item 2, positive selection)
// ---------------------------------------------------------------------------

/// An agent launched by a supervisor that is ITSELF running inside
/// somebody's farhelm tab must still be reaped by its own session's stop —
/// and a genuine tab of that same session must still survive it.
///
/// This is the dogfooding hole positive marking exists to close. A
/// supervisor started inside a tab inherits that tab's `FARHELM_TAB_ID`,
/// its tmux server inherits it, and every inner agent inherits it too;
/// under the exclusion-based design that came first, every one of those
/// inner agents looked like a tab process and escaped stop entirely.
/// `launch_env` reproduces exactly that inheritance — it injects into
/// tmux, which is the same route the real ambient value would take.
///
/// Both halves are asserted in one run on purpose: a stop that reaped the
/// agent by simply ignoring tab markers altogether would pass the first
/// assertion and fail the second.
#[tokio::test]
async fn an_agent_wearing_an_ambient_tab_marker_is_still_reaped_while_a_real_tab_survives() {
    // A plausible outer-tab id: minted-shaped, so it is the value a real
    // ambient marker would carry rather than something the parse would
    // reject for its shape alone.
    let ambient_tab = "0e5d9a11-0000-4000-8000-00000000abcd".to_string();
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_env: vec![("FARHELM_TAB_ID".to_string(), ambient_tab.clone())],
            ..SupervisorSeams::default()
        },
    )
    .await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-reparent"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_agent_chan, mut agent_rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let agent_pid = extract_pid(&agent_seen, "SELF-PID:");
    let agent_daemon = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (tab_chan, mut tab_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "tab-lease",
        )
        .await
        .expect("attach the tab");
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    h.client.stop_session(&session.id).await.expect("stop");

    // The inner agent and its reparented daemon are gone despite wearing
    // (or having inherited into their window) an outer tab's marker.
    wait_until_pid_gone(agent_pid, 15).await;
    wait_until_pid_gone(agent_daemon, 15).await;
    // The session's OWN tab is untouched.
    run_in_shell(
        &h.client,
        tab_chan,
        &mut tab_rx,
        &mut tab_seen,
        "printf 'STILL%sHERE\\n' -",
        "STILL-HERE",
        20,
    )
    .await;
}

/// A daemon left behind by a session launched BEFORE either kind marker
/// existed — session-marked and nothing else — must still be reaped by
/// stop.
///
/// The legacy bucket is the third root set `SweepTarget::AgentOnly`
/// claims, and it exists because a purely agent-marker-keyed stop would
/// silently stop reaching such processes on every host that upgraded.
/// `MarkedDecoy` is exactly that shape: the session marker, no kind
/// marker, and outside every cgroup this session owns.
#[tokio::test]
async fn a_session_marked_process_with_no_kind_marker_is_still_reaped_by_stop() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    let legacy = MarkedDecoy::spawn(&session.id);
    let legacy_pid = legacy.pid();
    h.client.stop_session(&session.id).await.expect("stop");
    wait_until_pid_gone(legacy_pid, 15).await;
}

/// The agent's window carries its marker after a create AND after a
/// restart that reuses the same pane (PLAN_M4.md item 2).
///
/// The marker became load-bearing when pane-less reload learned to prefer
/// it: without it, a session whose durable pane record is empty could
/// recover onto a TAB's pane and then reap that tab on its next stop. So
/// its presence is asserted directly against tmux rather than inferred,
/// and asserted again after a restart — `respawn-pane` keeps the window
/// and its options, and this is what would notice if a future change
/// swapped it for a fresh window instead.
#[tokio::test]
async fn the_agent_window_keeps_its_marker_across_a_restart_in_place() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let agent_marker = |rows: &[String]| -> Option<String> {
        rows.iter().find_map(|row| {
            let fields: Vec<&str> = row.split('|').collect();
            (fields.get(1) == Some(&format!("fh-{}", session.id).as_str())
                && fields.get(2) == Some(&"@0"))
            .then(|| fields.get(4).unwrap_or(&"").to_string())
        })
    };
    assert_eq!(
        agent_marker(&window_rows(&h).await).as_deref(),
        Some(session.id.as_str()),
        "a created session's agent window must carry its own session id as the agent marker"
    );

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart");
    assert_eq!(
        agent_marker(&window_rows(&h).await).as_deref(),
        Some(session.id.as_str()),
        "a restart that reuses the pane must leave the agent window's marker intact"
    );
}

// ---------------------------------------------------------------------------
// Tab lifecycle edges
// ---------------------------------------------------------------------------

/// Opening a tab on a RESTART-GAP session — one whose row survived a
/// supervisor restart but whose tmux did not — is the restart-first
/// conflict, and it must not build a tmux session as a side effect.
///
/// Distinct from the killed-tmux-session test: there the entry still
/// holds a terminal and tmux disagrees; here the entry has no terminal at
/// all, which is the branch that answers before tmux is consulted. Both
/// must produce the same advice, because they are the same fact for a
/// user.
#[tokio::test]
async fn opening_a_tab_on_a_restart_gap_session_is_a_restart_first_conflict() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let work = tempfile::tempdir().expect("workdir");
    let guard = TmuxServerGuard(state.path().join("tmux.sock"));

    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    let client = connect_client(&sup).await;
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
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    // Kill the tmux server and reload: the row comes back terminal-less,
    // which is the restart gap PLAN_M2.md names.
    drop(client);
    drop(sup);
    kill_tmux_server_and_wait(&state.path().join("tmux.sock")).await;
    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor");
    let client = connect_client(&sup).await;

    let err = client
        .open_tab(&session.id)
        .await
        .expect_err("a terminal-less session cannot gain a tab");
    assert!(
        format!("{err:#}").contains("restart"),
        "the refusal must point at restarting the session, got: {err:#}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a refused open must carry a SupervisorError")
            .kind,
        ErrorKind::Conflict
    );
    let sessions = tmux_query(&state.path().join("tmux.sock"), &["list-sessions"]).await;
    assert!(
        !sessions.status.success()
            || !String::from_utf8_lossy(&sessions.stdout).contains(&format!("fh-{}", session.id)),
        "a refused open must not have built a tmux session as a side effect"
    );
    drop(guard);
    drop(slot);
}

/// A tab whose shell EXITED on its own is not a closed tab: it stays
/// listed, stays attachable with its scrollback, and still closes
/// cleanly.
///
/// SPEC.md gives an established tab the same `remain-on-exit` contract the
/// agent terminal has — a dead pane is viewable, not gone — and the
/// dead-at-OPEN refusal is deliberately a different thing. This is the
/// test that keeps the two from being conflated into "a dead shell means
/// no tab".
#[tokio::test]
async fn a_tab_whose_shell_exited_stays_listed_replayable_and_closable() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (chan, mut rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "tab-lease",
        )
        .await
        .expect("attach the tab");
    let mut seen = Vec::new();
    wait_for_shell(&h.client, chan, &mut rx, &mut seen, "READY").await;
    run_in_shell(
        &h.client,
        chan,
        &mut rx,
        &mut seen,
        "printf 'BEFORE%sEXIT\\n' -",
        "BEFORE-EXIT",
        20,
    )
    .await;
    h.client.send_input(chan, b"exit\r".to_vec()).await;

    // The pane goes dead; the tab does not.
    let pane = tab_pane(&h, &session.id, &tab.id).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let out = tmux_query(
            &h.state.path().join("tmux.sock"),
            &["display-message", "-p", "-t", &pane, "#{pane_dead}"],
        )
        .await;
        if String::from_utf8_lossy(&out.stdout).trim() == "1" {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the tab's shell never exited"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![tab.id.clone()],
        "a tab whose shell exited is still a tab"
    );
    h.client.detach(chan).await;
    let (_chan2, mut rx2) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "tab-lease",
        )
        .await
        .expect("a dead tab pane must still be attachable");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "BEFORE-EXIT", 20).await;

    h.client
        .close_tab(&session.id, &tab.id)
        .await
        .expect("a tab whose shell already exited must still close");
    assert!(listed_tabs(&h.client, &session.id).await.is_empty());
}

/// Closing a tab id that is well-formed but unknown is `NotFound`, and
/// costs the session's real terminals nothing.
///
/// The shape matters: a valid-looking id exercises the lookup rather than
/// the syntax check, which is the path a client holding a selector from
/// before a reboot actually takes.
#[tokio::test]
async fn closing_an_unknown_but_well_formed_tab_id_is_not_found_and_harms_nothing() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;

    let err = h
        .client
        .close_tab(&session.id, "0e5d9a11-0000-4000-8000-00000000dead")
        .await
        .expect_err("an unknown tab id must be refused");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a refused close must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound,
    );
    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![tab.id.clone()],
        "a refused close must leave the session's real tabs alone"
    );
    h.client
        .send_input(agent_chan, b"agent-unharmed\r".to_vec())
        .await;
    wait_for(&mut agent_rx, &mut agent_seen, "agent-unharmed", 15).await;
}

/// A closed tab's attached client is told, on that tab's own channel.
///
/// A tab's forwarder holds a control client attached to the tmux SESSION,
/// so losing the tab's WINDOW does not end it — the stream would simply
/// go quiet forever. `detach_closed_tab` is the only thing that turns
/// that into a visible event, which is why it is asserted directly rather
/// than through "the terminal stopped updating".
#[tokio::test]
async fn a_closed_tabs_channel_receives_its_detached_notice() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (chan, mut rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "tab-lease",
        )
        .await
        .expect("attach the tab");
    let mut seen = Vec::new();
    wait_for_shell(&h.client, chan, &mut rx, &mut seen, "READY").await;

    h.client
        .close_tab(&session.id, &tab.id)
        .await
        .expect("close the tab");
    let reason = expect_detached(&mut rx, 15).await;
    assert!(
        reason.contains("tab"),
        "the notice must say the tab closed, got: {reason:?}"
    );
}

/// A tab open whose MARKING fails leaves nothing behind: no window, no
/// shell, no tab in the list — and the error says so rather than
/// claiming a clean removal it did not perform.
///
/// The marking is the one step whose failure strands something no
/// rediscovery can ever see again (an unmarked window is, by
/// construction, not a tab), so the unwind is worth proving rather than
/// reasoning about. The seam is the only way to reach that state: the
/// tmux call before it either works or leaves nothing.
#[tokio::test]
async fn a_tab_open_that_cannot_mark_its_window_leaves_nothing_behind() {
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            tab_open_fault: Some(Arc::new(|stage| {
                assert_eq!(
                    stage,
                    farhelm_supervisor::service::TabOpenStage::BeforeMarking
                );
                Err(anyhow::anyhow!("injected marking failure"))
            })),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let before = window_rows(&h).await.len();
    let err = h
        .client
        .open_tab(&session.id)
        .await
        .expect_err("an unmarkable window must fail the open");
    assert!(
        format!("{err:#}").contains("injected marking failure"),
        "the refusal must carry the cause, got: {err:#}"
    );
    assert!(
        listed_tabs(&h.client, &session.id).await.is_empty(),
        "a failed open must leave no tab"
    );
    let after = window_rows(&h).await;
    assert_eq!(
        after.len(),
        before,
        "a failed open must leave no window either; rows:\n{}",
        after.join("\n")
    );
}

/// A second lease attaching to EITHER of a session's terminals detaches
/// BOTH of the first client's channels, as one event — and leaves an
/// unrelated session's attachment alone (PLAN_M4.md acceptance 5).
///
/// SPEC.md's one-attached-client rule is per SESSION, not per terminal,
/// and only a session holding two terminals can show the difference: the
/// takeover has to sweep the whole lease and stop exactly there. The
/// unrelated session is the other half — a lease is never cross-session,
/// so one client may hold terminals in several sessions and taking one
/// over must not disturb the rest.
#[tokio::test]
async fn a_second_lease_takes_over_both_terminals_of_one_session_only() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (bystander, _bystander_work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let _bystander_cleanup = MarkerCleanupGuard::new(bystander.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (_agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "first-lease")
        .await
        .expect("attach the agent");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let (tab_chan, mut tab_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "first-lease",
        )
        .await
        .expect("attach the tab");
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;
    // The SAME client also holds the bystander session, under the same
    // lease: a lease groups a client's channels per session, never across
    // sessions.
    let (bystander_chan, mut bystander_rx) = h
        .client
        .attach_terminal(
            &bystander.id,
            80,
            24,
            TerminalSelector::Agent,
            "first-lease",
        )
        .await
        .expect("attach the bystander");
    let mut bystander_seen = Vec::new();
    wait_for(
        &mut bystander_rx,
        &mut bystander_seen,
        "FAKE-AGENT READY",
        20,
    )
    .await;

    // A different lease attaches to just ONE of the two terminals.
    let second = h.second_client().await;
    let (_winner_chan, mut winner_rx) = second
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "second-lease")
        .await
        .expect("take over");

    let agent_reason = expect_detached(&mut agent_rx, 15).await;
    let tab_reason = expect_detached(&mut tab_rx, 15).await;
    assert_eq!(
        agent_reason, tab_reason,
        "both channels of the losing lease must be told the SAME reason, which is what lets a \
         client coalesce them into one banner"
    );
    assert!(
        agent_reason.contains("another client"),
        "the reason must name a takeover, got: {agent_reason:?}"
    );

    // The bystander session's attachment, held by the SAME losing lease,
    // is untouched.
    let disturbed = drain_for(
        &mut bystander_rx,
        &mut bystander_seen,
        Duration::from_millis(500),
    )
    .await;
    assert_eq!(
        disturbed, None,
        "a takeover on one session must not detach the same client's terminals in another"
    );
    h.client
        .send_input(bystander_chan, b"bystander-alive\r".to_vec())
        .await;
    wait_for(
        &mut bystander_rx,
        &mut bystander_seen,
        "bystander-alive",
        15,
    )
    .await;
    // And the winner really has the terminal.
    let mut winner_seen = Vec::new();
    wait_for(&mut winner_rx, &mut winner_seen, "FAKE-AGENT READY", 20).await;
}

/// Deleting a session detaches EVERY channel it had — agent and tabs —
/// with the deletion notice, and reaps two environment-scrubbed tab
/// daemons through their own cgroups.
///
/// Two tabs rather than one because a delete names each tab's scope
/// separately (a cgroup kill reaches only what its own `systemd-run`
/// placed there), so a bug that named just one would pass with a single
/// tab. Loudly skipped where no user manager exists — the cloaked daemons
/// are unreachable by any marker scan by construction, which is the whole
/// point of the fixture.
#[tokio::test]
async fn deleting_a_session_detaches_every_channel_and_reaps_scrubbed_tab_daemons() {
    if !cgroup_path_available(
        "deleting_a_session_detaches_every_channel_and_reaps_scrubbed_tab_daemons",
    )
    .await
    {
        return;
    }
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;

    let mut tab_streams = Vec::new();
    let mut cloaked = Vec::new();
    let mut guards = Vec::new();
    for which in ["a", "b"] {
        let tab = h.client.open_tab(&session.id).await.expect("open a tab");
        let (chan, mut rx) = h
            .client
            .attach_terminal(
                &session.id,
                80,
                24,
                TerminalSelector::Tab { id: tab.id.clone() },
                "one-client",
            )
            .await
            .expect("attach the tab");
        let mut seen = Vec::new();
        wait_for_shell(&h.client, chan, &mut rx, &mut seen, "READY").await;
        let pid_file = work.path().join(format!("cloaked-{which}.pid"));
        let script =
            write_daemon_script(work.path(), &format!("cloak-{which}.sh"), &pid_file, true);
        run_in_shell(
            &h.client,
            chan,
            &mut rx,
            &mut seen,
            &format!("sh {} && printf 'CLOAK%sD\\n' E", script.display()),
            "CLOAKED",
            20,
        )
        .await;
        let pid = wait_for_pid_file(&pid_file, 10).await;
        guards.push(PidKillGuard::arm(pid));
        assert!(
            !marked_pids(&session.id).contains(&pid),
            "test setup: tab {which}'s cloaked daemon must carry no marker at all"
        );
        cloaked.push(pid);
        tab_streams.push(rx);
    }

    h.client.delete_session(&session.id).await.expect("delete");

    // Generous, because the failure this bounds is "it never died" and
    // the path to death is three D-Bus round trips per scope on a host
    // running several of these harnesses at once.
    for pid in cloaked {
        wait_until_pid_gone(pid, 30).await;
    }
    let agent_reason = expect_detached(&mut agent_rx, 15).await;
    assert!(
        agent_reason.contains("deleted"),
        "the agent's channel must be told the session was deleted, got: {agent_reason:?}"
    );
    for (index, mut rx) in tab_streams.into_iter().enumerate() {
        let reason = expect_detached(&mut rx, 15).await;
        assert_eq!(
            reason, agent_reason,
            "tab {index}'s channel must get the same deletion notice the agent's did"
        );
    }
}

/// Tabs survive a supervisor restart as the SAME shells, with their
/// scrollback (PLAN_M4.md acceptance 2).
///
/// The sibling rediscovery test pins that the ids and the ordering come
/// back. This pins the thing that actually matters to a user: the process
/// never noticed. Comparing pane PIDs across the restart is what
/// distinguishes "rediscovered the same shell" from "quietly started a
/// new one", and replaying content written before the restart is what
/// distinguishes a live reattachment from a fresh, empty terminal.
#[tokio::test]
async fn a_supervisor_restart_leaves_a_tabs_shell_and_scrollback_untouched() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let work = tempfile::tempdir().expect("workdir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));

    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    let client = connect_client(&sup).await;
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
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let tab = client.open_tab(&session.id).await.expect("open a tab");

    let (chan, mut rx) = client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "before-restart",
        )
        .await
        .expect("attach the tab");
    let mut seen = Vec::new();
    wait_for_shell(&client, chan, &mut rx, &mut seen, "READY").await;
    run_in_shell(
        &client,
        chan,
        &mut rx,
        &mut seen,
        "printf 'BEFORE%sRESTART\\n' -",
        "BEFORE-RESTART",
        20,
    )
    .await;

    let sock = state.path().join("tmux.sock");
    let pane_before =
        tmux_query(&sock, &["list-panes", "-a", "-F", "#{pane_id} #{pane_pid}"]).await;
    let pids_before = String::from_utf8_lossy(&pane_before.stdout).into_owned();

    drop(client);
    drop(sup);
    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor");
    let client = connect_client(&sup).await;

    let pane_after = tmux_query(&sock, &["list-panes", "-a", "-F", "#{pane_id} #{pane_pid}"]).await;
    assert_eq!(
        String::from_utf8_lossy(&pane_after.stdout),
        pids_before,
        "a supervisor restart must rediscover the SAME shells, not start new ones"
    );

    let (_chan2, mut rx2) = client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "after-restart",
        )
        .await
        .expect("attach the rediscovered tab");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "BEFORE-RESTART", 20).await;
    drop(slot);
}

/// A stalled TAB viewer takes the stall detach ALONE: the agent terminal
/// and a sibling tab stay usable throughout, and the stalled tab
/// reattaches as an ordinary reconnect with its scrollback (PLAN_M4.md
/// acceptance 5, item 3's deliberate per-terminal reading).
///
/// The stall timeout is shortened through the same seam every other
/// stall test uses, because the production value is a minute. What this
/// pins is the DETACH being scoped to one terminal — a client whose
/// background tab wedged must not lose the terminal it is looking at.
///
/// Honest scope: this exercises whichever branch tmux happened to take
/// for the stalled client (it answers a lagging client either by cutting
/// it with `%pause` or by throttling the pane, nondeterministically — see
/// `TMUX_PAUSE_AFTER_SECS`), so it pins the SUPERVISOR's per-terminal
/// teardown rather than both tmux paths. The throttling branch can
/// transiently slow the session's other panes until this detach fires;
/// that window is documented on `OutputStream` and closed by the
/// session-sink design rather than by this test.
#[tokio::test]
async fn a_stalled_tab_takes_the_stall_detach_alone_and_reattaches() {
    let stall = Duration::from_secs(3);
    let h = harness_with_timeouts(SupervisorTimeouts {
        stall_detach: stall,
        ..SupervisorTimeouts::default()
    })
    .await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let stalling = h.client.open_tab(&session.id).await.expect("open a tab");
    let sibling = h
        .client
        .open_tab(&session.id)
        .await
        .expect("open a second tab");

    let (agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let (sibling_chan, mut sibling_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: sibling.id.clone(),
            },
            "one-client",
        )
        .await
        .expect("attach the sibling tab");
    let mut sibling_seen = Vec::new();
    wait_for_shell(
        &h.client,
        sibling_chan,
        &mut sibling_rx,
        &mut sibling_seen,
        "SIB",
    )
    .await;
    let (stalling_chan, mut stalling_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: stalling.id.clone(),
            },
            "one-client",
        )
        .await
        .expect("attach the stalling tab");
    let mut stalling_seen = Vec::new();
    wait_for_shell(
        &h.client,
        stalling_chan,
        &mut stalling_rx,
        &mut stalling_seen,
        "STALL",
    )
    .await;
    run_in_shell(
        &h.client,
        stalling_chan,
        &mut stalling_rx,
        &mut stalling_seen,
        "printf 'BEFORE%sSTALL\\n' -",
        "BEFORE-STALL",
        20,
    )
    .await;

    // Wedge exactly one terminal and give it something to be behind on.
    // Deliberately a MODEST flood: the stall deadline is absolute from
    // the moment the pause is recorded, so volume is not what triggers
    // it, and a flood large enough to overrun `HISTORY_LIMIT` would push
    // the pre-stall marker out of the scrollback this test reattaches to
    // read.
    h.client.pause_output(stalling_chan).await;
    h.client
        .send_input(
            stalling_chan,
            b"i=0; while [ $i -lt 500 ]; do printf 'FLOOD-%s\\n' $i; i=$((i+1)); done\r".to_vec(),
        )
        .await;

    let reason = expect_detached(&mut stalling_rx, 60).await;
    assert!(
        reason.contains("stall"),
        "the stalled tab must be detached as stalled, got: {reason:?}"
    );

    // The agent and the sibling tab were never disturbed and still work.
    h.client
        .send_input(agent_chan, b"agent-untouched\r".to_vec())
        .await;
    wait_for(&mut agent_rx, &mut agent_seen, "agent-untouched", 20).await;
    run_in_shell(
        &h.client,
        sibling_chan,
        &mut sibling_rx,
        &mut sibling_seen,
        "printf 'SIBLING%sOK\\n' -",
        "SIBLING-OK",
        20,
    )
    .await;

    // And the stalled terminal reattaches like any reconnect, with the
    // scrollback it had before it wedged.
    let (_again, mut again_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: stalling.id.clone(),
            },
            "one-client",
        )
        .await
        .expect("a stall detach must leave the tab reattachable");
    let mut replay = Vec::new();
    wait_for(&mut again_rx, &mut replay, "BEFORE-STALL", 30).await;
}

/// A tab OPEN racing a session DELETE resolves to one coherent winner and
/// leaves no orphan, at any interleaving.
///
/// The session's lifecycle claim is what makes that true: without it a
/// delete could finish its process-tree sweep and an open could then start
/// a shell in the tmux session the delete is about to tear down, leaving
/// that shell's daemonized children alive with no row left to reap them
/// from. Serialized, both orders are correct — an open that wins is swept
/// by the delete behind it, and an open that loses finds no session at
/// all — so the assertion is on the OUTCOMES rather than on which one won.
///
/// Staggered offsets rather than one fixed timing, the same technique
/// `a_stall_teardown_racing_a_takeover_never_detaches_the_winner` uses:
/// the interesting interleavings are near the boundary and no single
/// delay reliably lands on them.
#[tokio::test]
async fn an_open_tab_racing_a_delete_leaves_one_coherent_winner() {
    for offset_ms in [0, 5, 20, 60] {
        let h = harness().await;
        let (session, _work) = basic_session(&h).await;
        let cleanup = MarkerCleanupGuard::new(session.id.clone());

        let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
        let mut seen = Vec::new();
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

        let opener = h.second_client().await;
        let open_id = session.id.clone();
        let opening =
            tokio::spawn(async move { opener.open_tab(&open_id).await.map(|tab| tab.id) });
        tokio::time::sleep(Duration::from_millis(offset_ms)).await;
        let deleted = h.client.delete_session(&session.id).await;
        let opened = opening.await.expect("the open task must not panic");

        assert!(
            deleted.is_ok(),
            "offset {offset_ms}: the delete must not be defeated by a concurrent open: {:#}",
            deleted.unwrap_err()
        );
        // Whichever way it went, nothing of the session may remain: no
        // row, no tmux session, and no marked process — including a shell
        // an open that WON would have started.
        assert!(
            h.client
                .list_sessions()
                .await
                .expect("list")
                .sessions
                .iter()
                .all(|listed| listed.id != session.id),
            "offset {offset_ms}: the deleted session must be gone from the list"
        );
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while !marked_pids(&session.id).is_empty() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "offset {offset_ms}: a delete racing an open (which {}) left marked processes \
                 behind: {:?}",
                if opened.is_ok() { "won" } else { "lost" },
                marked_pids(&session.id)
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        drop(cleanup);
    }
}

/// A tab CLOSE racing a session DELETE likewise resolves coherently.
///
/// The same claim serializes them, and the failure it prevents is uglier
/// than the open's: two sweeps and two teardowns racing over one window,
/// with either able to report success while the other was mid-reap. Both
/// orders are acceptable outcomes — a close that wins is followed by a
/// delete that finds one fewer tab, and a close that loses finds no
/// session — so this asserts the delete succeeds, the close's own answer
/// is one of those two shapes, and nothing survives.
#[tokio::test]
async fn a_close_tab_racing_a_delete_leaves_one_coherent_winner() {
    for offset_ms in [0, 10, 40] {
        let h = harness().await;
        let (session, _work) = basic_session(&h).await;
        let cleanup = MarkerCleanupGuard::new(session.id.clone());

        let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
        let mut seen = Vec::new();
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
        let tab = h.client.open_tab(&session.id).await.expect("open a tab");

        let closer = h.second_client().await;
        let close_session = session.id.clone();
        let close_tab_id = tab.id.clone();
        let closing =
            tokio::spawn(async move { closer.close_tab(&close_session, &close_tab_id).await });
        tokio::time::sleep(Duration::from_millis(offset_ms)).await;
        let deleted = h.client.delete_session(&session.id).await;
        let closed = closing.await.expect("the close task must not panic");

        assert!(
            deleted.is_ok(),
            "offset {offset_ms}: the delete must not be defeated by a concurrent close: {:#}",
            deleted.unwrap_err()
        );
        if let Err(e) = &closed {
            assert_eq!(
                e.downcast_ref::<SupervisorError>()
                    .expect("a refused close must carry a SupervisorError")
                    .kind,
                ErrorKind::NotFound,
                "offset {offset_ms}: a close that lost the race must report the session or tab \
                 as gone, not a teardown failure: {e:#}"
            );
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while !marked_pids(&session.id).is_empty() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "offset {offset_ms}: a delete racing a close left marked processes behind: {:?}",
                marked_pids(&session.id)
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        drop(cleanup);
    }
}

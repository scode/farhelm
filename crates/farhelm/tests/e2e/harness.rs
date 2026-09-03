//! Shared test infrastructure for every `e2e` module: connecting a client,
//! booting a `Supervisor` against a real private tmux, and polling for the
//! terminal/session-list state a test wants to assert on. This is the
//! broadly shared layer — everything the pre-split file's shared preamble
//! offered — plus the private helpers those exported items depend on
//! internally (`children_of`, `proc_starttime`), so a reader never has to
//! chase a definition into `session_lifecycle` to understand a harness
//! function's own behavior. Narrowly shared helpers stay section-owned:
//! a module that grew a helper another section later needed exports it
//! directly (`terminal_backpressure::drain_for` and friends), and the
//! consumer imports it by name from its owner.
//!
//! Re-exports below are `pub(crate) use` rather than plain `use` so a
//! sibling module can pull the whole set into scope with a single
//! `use crate::harness::*;`, matching what the pre-split file offered
//! implicitly through shared top-of-file imports.

pub(crate) use farhelm_helm::{SupervisorClient, SupervisorError, TermEvent, TermStream};
pub(crate) use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
pub(crate) use farhelm_proto::{
    ControlMsg, ErrorKind, Frame, FrameKind, LIST_SESSIONS_CAP, SessionInfo, SessionStatus,
    TerminalSelector, UPLOAD_ABORT_REASON_STALLED, UPLOAD_CHUNK_BYTES,
};
pub(crate) use farhelm_supervisor::agent_kind::{CaptureWindow, CaptureWindowBounds, now_unix};
pub(crate) use farhelm_supervisor::launch::{spec_path_for_launch, status_path_for_spec};
pub(crate) use farhelm_supervisor::service::{
    CaptureStoreFault, CreateCrashSeam, CreateStage, SessionSnapshot, Supervisor, SupervisorSeams,
    SupervisorTimeouts, handle_connection,
};
pub(crate) use farhelm_supervisor::store::{
    LastOutcome, Reservation, ReservationOutcome, SessionStore, StoredSession,
};
pub(crate) use std::io;
pub(crate) use std::pin::Pin;
pub(crate) use std::sync::Arc;
pub(crate) use std::sync::atomic::{AtomicBool, Ordering};
pub(crate) use std::task::{Context, Poll};
pub(crate) use std::time::Duration;
pub(crate) use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// The receive surface needed by the shared terminal waiters.
///
/// `TermStream` keeps its event queue private and cannot be constructed
/// outside `farhelm-helm`, so the e2e harness uses this crate-private trait to
/// run the same drain logic against scripted event sources in unit tests.
pub(crate) trait TermSource {
    /// Receive the next event, preserving the source's end-of-stream value.
    async fn recv(&mut self) -> Option<TermEvent>;

    /// Receive one event only when it is already queued for draining.
    fn try_recv(&mut self) -> Result<TermEvent, tokio::sync::mpsc::error::TryRecvError>;
}

impl TermSource for TermStream {
    async fn recv(&mut self) -> Option<TermEvent> {
        TermStream::recv(self).await
    }

    fn try_recv(&mut self) -> Result<TermEvent, tokio::sync::mpsc::error::TryRecvError> {
        TermStream::try_recv(self)
    }
}

/// Pane width used wherever a test reads the fake agent's argv marker.
///
/// Far wider than the suite's usual 80 because the marker carries two
/// absolute tempdir paths plus injected settings JSON. A pane narrower than
/// the line wraps it, and a replayed wrap comes back as a real newline;
/// callers therefore fail loudly through [`argv_marker`] if this bound stops
/// being sufficient.
pub(crate) const WIDE_COLS: u16 = 500;

/// Pane height; nothing here depends on it.
pub(crate) const ROWS: u16 = 24;

/// The marker the record-writing fixtures echo their own argv under.
pub(crate) const ARGV_MARKER: &str = "FAKE-AGENT ARGV:";

/// Extract the argv the fixture echoed on its most recent launch.
///
/// The LAST occurrence matters because a reattach after a relaunch replays
/// earlier generations too, and the first marker would answer for the wrong
/// generation. The width assertion is not paranoia: a wrapped line comes
/// back from replay as a genuine newline, so without it this would silently
/// return a prefix of the argv and every injection assertion would fail with
/// the flag simply missing. Failing on the width names the fix: raise
/// [`WIDE_COLS`].
///
/// The marker's own width counts toward the bound. The fixture prints it at
/// column zero and the argv follows on the same row, so measuring only the
/// argv would accept a line that had already wrapped by exactly the marker's
/// length.
///
/// The normalizer trims trailing blanks BEFORE the bound is applied. When
/// the fixture wins the race against the test's attach, the marker line
/// arrives through the attach snapshot rather than live, and a snapshot row
/// is padded with spaces out to the full pane width — a 484-character "line"
/// for a 245-character argv in a 500-column pane. That padding is not
/// wrapping: a row that really wrapped is full of argv characters to its last
/// column, so the trimmed length still trips the bound for the case it exists
/// to catch. The untrimmed check failed the 0.3.0-rc.1 release gate on exactly
/// this snapshot shape.
pub(crate) fn argv_marker(transcript: &[u8]) -> String {
    let text = normalize_pane_text(transcript);
    let start = text
        .rfind(ARGV_MARKER)
        .unwrap_or_else(|| panic!("no {ARGV_MARKER} in transcript:\n{text}"))
        + ARGV_MARKER.len();
    let line = text[start..]
        .lines()
        .next()
        .expect("a marker is followed by at least a line ending")
        .to_string();
    assert!(
        ARGV_MARKER.chars().count() + line.chars().count() < WIDE_COLS as usize,
        "the argv line filled the pane and may have wrapped; raise WIDE_COLS: {line}"
    );
    line
}

/// The built farhelm binary: fake agent + launch shim in one artifact,
/// exactly as production ships it.
pub(crate) fn farhelm_bin() -> &'static str {
    env!("CARGO_BIN_EXE_farhelm")
}

/// Run one tmux command against a private socket, asynchronously.
///
/// Async (tokio Command) rather than `std::process` because `#[tokio::test]`
/// runs a current-thread runtime: a blocking child-process wait in a test
/// body stalls the in-process supervisor and forwarder tasks on that same
/// runtime, distorting exactly the concurrent behavior under test.
pub(crate) async fn tmux_query(sock: &std::path::Path, args: &[&str]) -> std::process::Output {
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
pub(crate) async fn kill_tmux_server_and_wait(sock: &std::path::Path) {
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
pub(crate) async fn pane_id_of(sock: &std::path::Path, tmux_name: &str) -> String {
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
pub(crate) async fn tmux_has_format(h: &Harness, name: &str) -> bool {
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

/// Wait until a supervisor on `state_dir` can be DIALLED, for tests that
/// drive `serve()` directly (or run a supervisor as a real child process)
/// rather than using an in-process duplex pipe.
///
/// What a successful dial proves is exactly that a listening socket is
/// bound at that path — no more. It does not prove the supervisor has
/// accepted the connection (the kernel completes the handshake into the
/// listen backlog on its own), and it certainly does not prove startup
/// reconciliation has run, since `serve()` binds before it reconciles. A
/// test that needs either of those must still prove it with a completed
/// REQUEST; the attachment-sweep test does exactly that.
///
/// A dial is nonetheless the right thing to poll for, because the SOCKET
/// FILE is not. The file is wrong in both directions: `serve()` creates it
/// before it is listening on it, and — the case that made this a real bug
/// rather than a theoretical one — a `SIGKILL`ed supervisor leaves its
/// socket file behind, so a test waiting for a REPLACEMENT supervisor's
/// socket is satisfied instantly by the dead one's leftovers and proceeds
/// to race a process that has not bound anything yet. A dial against that
/// stale file is refused, which is the distinction the file cannot make.
///
/// The budget matches the rest of this suite's waits (20s) rather than the
/// 5s the file-existence version used: spawning a supervisor process,
/// opening its store, and binding is not a sub-second operation on a
/// loaded machine, and a wait four times tighter than every sibling is a
/// flake source of its own.
pub(crate) async fn wait_for_supervisor_ready(state_dir: &std::path::Path) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        match farhelm_supervisor::service::connect(state_dir).await {
            // Dropped immediately: this connection is the probe, not a
            // client. The supervisor sees a peer that hangs up before
            // saying hello, which it is already required to survive.
            Ok(_stream) => return,
            Err(e) => {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "nothing was listening on {}'s supervisor socket: {e:#}",
                    state_dir.display()
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

/// Quote a path for an agent invocation string, which the supervisor
/// parses with shell-words. Without this a checkout under a path with
/// spaces would fragment the argv and fail every test confusingly.
pub(crate) fn agent_cmd(args: &str) -> String {
    format!("{} {args}", shell_words::quote(farhelm_bin()))
}

/// Caps how many harnesses run at once.
///
/// Each one is a tmux server plus a login shell plus a fake agent, and
/// libtest runs every test in this binary concurrently. Unbounded, the
/// machine gets loaded enough that agent startup exceeds the waits and
/// tests fail for reasons that have nothing to do with the code — a
/// flakiness source worth removing rather than papering over with longer
/// timeouts.
pub(crate) static SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

/// Kills a private tmux server on drop.
///
/// Drop-based on purpose: a test that fails an assertion never reaches
/// an explicit teardown call, and leaked tmux servers accumulate across
/// runs until they visibly slow the machine down (this happened).
/// Synchronous because Drop cannot await. Every test that starts a tmux
/// server — via `harness()` or by hand — must hold one of these, ordered
/// BEFORE the state `TestDir` in its struct so the server dies before
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
pub(crate) struct TmuxServerGuard(pub(crate) std::path::PathBuf);

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

// ---------------------------------------------------------------
// The REAL-STACK fixtures: separate `farhelm` processes, a loopback
// port, and HTTP.
//
// Everything above this banner drives an in-process `Supervisor` over a
// duplex pipe, which is the right shape for almost everything: it is fast,
// it is deterministic, and it still exercises tmux for real. What it
// cannot show is that the SHIPPED binary assembles itself correctly —
// which flags it parses, what it prints, which pieces its startup wires
// together. The helpers below stand up the product's own processes for the
// handful of tests whose subject is exactly that, and they live here
// rather than in one of those modules because more than one now needs
// them.
// ---------------------------------------------------------------

/// How long a real-stack test waits for a multi-process stack to reach a
/// state.
///
/// Generous rather than tight: supervisors, tmux servers, an ssh handshake
/// and a helm all have to come up, and on a loaded runner that is genuinely
/// slow. The failure this bound reports — "it never got there" — is not
/// diagnosed any better by a shorter wait.
pub(crate) const REAL_STACK_SETTLE: Duration = Duration::from_secs(90);

/// One running supervisor process plus everything that must die with it.
///
/// Field order is drop order and is load-bearing, the same rule
/// [`TmuxServerGuard`]'s own docs set out: the process first, then its tmux
/// server, then the directory holding both their sockets.
///
/// The child needs no guard of its own — every process here is spawned with
/// `kill_on_drop(true)`, so dropping the `Child` is what kills it, and a
/// wrapper would only restate that. Which matters for the same reason the
/// order does: a test that fails an assertion never reaches an explicit
/// teardown, and a leaked helm holding a loopback port is exactly the
/// debris that makes the NEXT run fail for an unrelated reason.
pub(crate) struct SupervisorProcess {
    _child: tokio::process::Child,
    _tmux: TmuxServerGuard,
    pub(crate) state: farhelm_teststate::TestDir,
}

/// Start a real `farhelm supervisor run` on a fresh state directory and
/// wait for its socket to accept.
///
/// The socket wait is what makes the rest of a test deterministic: a helm
/// started against a directory with no socket yet is not wrong — it simply
/// retries — but it would turn every later assertion into a race with the
/// reconnect ladder.
pub(crate) async fn supervisor_process() -> SupervisorProcess {
    supervisor_process_with_env(std::iter::empty()).await
}

/// [`supervisor_process`], with `env` applied to the CHILD's environment
/// before spawn.
///
/// Every other real-stack fixture builds a [`SupervisorStartup`] directly
/// in Rust, which is the right shape for almost everything — but it never
/// exercises `main.rs`'s own `std::env::var` reads for
/// `FARHELM_AGENT_HOOKS`/`FARHELM_AGENT_INSTRUCTIONS` at all, since those
/// reads live in the CLI arm and a struct built by hand skips over them
/// entirely. `hook_identity`'s `FARHELM_AGENT_INSTRUCTIONS` tests need
/// exactly that arm exercised, env var and non-UTF-8 fallback included, so
/// this is the one place in the suite that sets an environment variable on
/// a spawned `farhelm` process rather than mutating this test process's
/// own — the two are never confused, because `Command::env` only ever
/// reaches the CHILD.
pub(crate) async fn supervisor_process_with_env(
    env: impl IntoIterator<Item = (&'static str, std::ffi::OsString)>,
) -> SupervisorProcess {
    let state = farhelm_teststate::tempdir().expect("supervisor state dir");
    supervisor_process_on_state(state, env).await
}

/// [`supervisor_process_with_env`], on a caller-prepared state directory.
///
/// Exists for tests that must plant on-disk state (an older build's
/// leftovers, say) BEFORE the supervisor's own startup sequence runs over
/// it — the in-process [`harness`] cannot serve them, because it wires its
/// client straight into `handle_connection` and never runs `serve`, which
/// is where the startup sweeps live. This spawns the real
/// `supervisor run` CLI, so what runs over the planted state is the
/// production startup path, byte for byte.
pub(crate) async fn supervisor_process_on_state(
    state: farhelm_teststate::TestDir,
    env: impl IntoIterator<Item = (&'static str, std::ffi::OsString)>,
) -> SupervisorProcess {
    let mut command = tokio::process::Command::new(farhelm_bin());
    command
        .args(["supervisor", "run", "--state-dir"])
        .arg(state.path());
    for (key, value) in env {
        command.env(key, value);
    }
    let child = command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn supervisor");
    let tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let socket = state.path().join("supervisor.sock");
    let deadline = tokio::time::Instant::now() + REAL_STACK_SETTLE;
    while !socket.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the supervisor at {} never bound its socket",
            state.path().display()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    SupervisorProcess {
        _child: child,
        _tmux: tmux,
        state,
    }
}

/// A running helm process and the loopback base URL it printed.
pub(crate) struct HelmProcess {
    _child: tokio::process::Child,
    pub(crate) base: String,
}

impl HelmProcess {
    /// The loopback address behind [`Self::base`], for the one client that
    /// cannot use an HTTP library: the WebSocket upgrade.
    pub(crate) fn addr(&self) -> std::net::SocketAddr {
        self.base
            .trim_start_matches("http://")
            .parse()
            .unwrap_or_else(|e| {
                panic!("the helm's base URL is not an address ({e}): {}", self.base)
            })
    }
}

/// Start a real `farhelm helm run` on an ephemeral port, against a state
/// directory somebody else owns, and read back the URL it prints.
///
/// `--port 0` plus parsing stdout, rather than picking a port and hoping:
/// this suite runs concurrently with itself and with whatever else is on
/// the machine, and a hardcoded port is a flake waiting for a second
/// worktree.
///
/// The state directory is a LOCAL supervisor's, deliberately. The local
/// row is reached through whatever listens in the helm's own state
/// directory, so sharing one directory is not a shortcut — it is the
/// production arrangement, the one where helm.db and `supervisor.sock` are
/// siblings and the local host needs no registering at all.
///
/// `ensure_hosts` is optional because only the fleet tests need a registry
/// seeded before serving begins; a single-host test passes `None` and gets
/// the local row alone.
pub(crate) async fn helm_process(
    state_dir: &std::path::Path,
    ensure_hosts: Option<&std::path::Path>,
) -> HelmProcess {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let mut command = tokio::process::Command::new(farhelm_bin());
    command
        .args(["helm", "run", "--port", "0", "--state-dir"])
        .arg(state_dir);
    if let Some(ensure) = ensure_hosts {
        command.arg("--ensure-hosts").arg(ensure);
    }
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn helm");
    let stdout = child.stdout.take().expect("piped stdout");
    let mut lines = BufReader::new(stdout).lines();
    let line = tokio::time::timeout(REAL_STACK_SETTLE, lines.next_line())
        .await
        .expect("the helm printed its URL within the settle budget")
        .expect("reading the helm's stdout")
        .expect("the helm printed a line before exiting");
    let base = line
        .split_once("http://")
        .map(|(_, url)| format!("http://{}", url.trim_end_matches('/')))
        .unwrap_or_else(|| panic!("the helm's first stdout line named no URL: {line:?}"));
    HelmProcess {
        _child: child,
        base,
    }
}

/// Exchange the shipped CLI's bootstrap token for the device secret every
/// later request in a real-stack test carries.
///
/// Returned as the raw secret rather than only as a configured client
/// because the WebSocket routes cannot use an HTTP client at all: they
/// carry the credential in `Sec-WebSocket-Protocol` instead of a header a
/// `reqwest::Client` default can supply.
pub(crate) async fn device_secret(state_dir: &std::path::Path, base: &str) -> String {
    let output = tokio::process::Command::new(farhelm_bin())
        .args(["helm", "token", "show", "--state-dir"])
        .arg(state_dir)
        .output()
        .await
        .expect("run token show");
    assert!(
        output.status.success(),
        "token show failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let token = String::from_utf8(output.stdout)
        .expect("the token is UTF-8")
        .trim()
        .to_string();
    assert!(!token.is_empty(), "token show must print a credential");

    let response = reqwest::Client::new()
        .post(format!("{base}/api/auth/token"))
        .json(&serde_json::json!({ "token": token }))
        .send()
        .await
        .expect("token exchange reached the helm");
    assert!(
        response.status().is_success(),
        "token exchange answered {}",
        response.status()
    );
    let exchange: serde_json::Value = response.json().await.expect("decode device exchange");
    exchange["device_secret"]
        .as_str()
        .expect("the exchange returns a device secret")
        .to_string()
}

/// An HTTP client carrying `secret` on every request.
pub(crate) fn client_with_secret(secret: &str) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("Bearer {secret}"))
            .expect("the device secret is an Authorization value"),
    );
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .expect("build authenticated client")
}

/// [`device_secret`] and [`client_with_secret`] together, for the callers
/// that never need the secret itself.
pub(crate) async fn authenticated_client(
    state_dir: &std::path::Path,
    base: &str,
) -> reqwest::Client {
    client_with_secret(&device_secret(state_dir, base).await)
}

/// GET a JSON body from the helm, failing the test on a non-2xx.
pub(crate) async fn get_json(client: &reqwest::Client, url: &str) -> serde_json::Value {
    let response = client.get(url).send().await.expect("GET reached the helm");
    let status = response.status();
    let body = response.text().await.expect("read body");
    assert!(status.is_success(), "GET {url} answered {status}: {body}");
    serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("GET {url} body is not JSON ({e}): {body}"))
}

/// POST a JSON body to the helm, returning the status and the body text —
/// both, because a refusal's body is prose and is half of what this stack
/// promises about refusals.
pub(crate) async fn post(
    client: &reqwest::Client,
    url: &str,
    body: serde_json::Value,
) -> (reqwest::StatusCode, String) {
    let response = client
        .post(url)
        .json(&body)
        .send()
        .await
        .expect("POST reached the helm");
    let status = response.status();
    (status, response.text().await.expect("read body"))
}

pub(crate) struct Harness {
    pub(crate) client: Arc<SupervisorClient>,
    /// Shared so tests can open additional connections to the same
    /// supervisor (`second_client`) — cross-connection enforcement is
    /// exactly what some SPEC.md rules are about.
    pub(crate) sup: Arc<Supervisor>,
    pub(crate) _tmux: TmuxServerGuard,
    // Read in tests (socket paths); ALSO held so the tempdir outlives
    // the tmux server — it must be declared after `_tmux` so the server
    // dies before its socket directory is deleted.
    //
    // A `TestDir` rather than a plain `TempDir`: drop-time removal is the
    // same (normal exit and panic unwinding alike), but the dir lives
    // under farhelm-teststate's shared /tmp scheme (sweepable `fh-it.`
    // container, held flock), so state killed too abruptly for
    // destructors to run (SIGKILL, abort) is reclaimed by a later run's
    // sweep once the protocol's grace passes, instead of orphaned
    // forever.
    pub(crate) state: farhelm_teststate::TestDir,
    // Released on drop, letting the next test start its stack.
    pub(crate) _slot: tokio::sync::SemaphorePermit<'static>,
}

impl Harness {
    /// Open a second, independent connection to this harness's
    /// supervisor — a stand-in for "another helm" or a restarted one.
    /// Its channel ids number from 1 like any client's, which is the
    /// collision the cross-connection tests rely on.
    pub(crate) async fn second_client(&self) -> Arc<SupervisorClient> {
        connect_client(&self.sup).await
    }
}

/// Connect one client to a supervisor over an in-process duplex pipe
/// (the local-transport shape, minus the socket file).
pub(crate) async fn connect_client(sup: &Arc<Supervisor>) -> Arc<SupervisorClient> {
    let (client_side, server_side) = tokio::io::duplex(1 << 20);
    let sup = Arc::clone(sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup, server_side).await;
    });
    let (r, w) = tokio::io::split(client_side);
    SupervisorClient::start(r, w).await.expect("handshake")
}

/// Create a basic-script fake-agent session in a fresh working
/// directory — the preamble almost every test in the suite shares.
/// Returns the workdir too: it must outlive the launch (it is the
/// session's cwd), so callers hold it as `_work`.
///
/// Returns as soon as `create` has REPLIED, which says a pane exists and
/// nothing more — the agent inside it may not have execed yet, and may
/// never. Prefer [`basic_session_ready`] wherever the test's assertions
/// are about a running agent; see [`wait_for_agent_ready`] for the flake
/// that distinction produces when it is skipped.
pub(crate) async fn basic_session(h: &Harness) -> (SessionInfo, farhelm_teststate::TestDir) {
    let work = farhelm_teststate::tempdir().expect("workdir");
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

/// The sentinel every fake-agent script prints exactly once, after the
/// `exec` that replaced the shim with the agent and after whatever setup
/// that script does first (`fake_agent.rs`'s "contract with tests").
///
/// Named here rather than spelled inline because [`wait_for_agent_ready`]
/// rests on WHERE in the launch chain it is written, not merely on the
/// text: nothing before the exec can print it. The converse is weaker —
/// its absence says only that readiness was never observed, not at which
/// link the chain died.
pub(crate) const AGENT_READY_MARKER: &str = "FAKE-AGENT READY";

/// How long [`wait_for_agent_ready`] gives a launch to reach the agent.
///
/// Matches the suite's other post-exec waits (`wait_for`'s 20–30s calls)
/// rather than being tuned down: this is a setup barrier on a machine that
/// may be running four harnesses plus a full workspace suite, so it has to
/// be generous enough that a merely SLOW launch never fails it. What keeps
/// the generosity honest is that a DEAD pane fails immediately rather than
/// waiting the budget out, and the panic reports the pane's own state.
const AGENT_READY_SECS: u64 = 30;

/// Wait until `session_id`'s agent has actually STARTED — i.e. has printed
/// [`AGENT_READY_MARKER`] into its pane on the private server at `sock` —
/// failing the test by name, with the pane's own last words, if it never
/// does or if the pane dies first.
///
/// # Why a post-exec marker and not a live pane
///
/// A launch is a chain: tmux runs a login shell, the shell runs the
/// transient cgroup scope wrapper (`systemd-run --user --scope`, wherever
/// a systemd user manager exists), the wrapper `exec`s farhelm's launch
/// shim, and the shim `exec`s the agent. Every link before the last one
/// holds a LIVE pane, so "the pane exists" and even "the session lists as
/// live" are satisfied by a launch that is about to die without ever
/// having run an agent. Only a post-exec write proves the chain completed.
///
/// # The bug history this exists for
///
/// On 2026-08-18 a full-suite run at libtest's default thread count failed
/// `boot_id_durable_outcome::a_list_polling_through_a_stop_never_erases_the_annotation`
/// on its `Exited` assertion, having gotten `Error { "the agent was never
/// started: the launch never reached farhelm's exec shim …" }` — the
/// supervisor's own never-started classifier
/// (`service::launch_artifacts::wrapper_failure_detail`). The session's
/// launch had died somewhere before the shim, so the stop under test
/// recorded a launch failure instead of the annotated exit, and the
/// assertion reported the WRONG thing as broken. Every `basic_session`
/// caller shares that exposure; this barrier converts it from a corrupted
/// assertion into a setup failure that names what actually went wrong.
///
/// # Contract
///
/// For a FRESH session — generation zero, no tabs yet, a quiet fixture.
/// The bare `fh-<id>` target resolves to the session's current pane, which
/// is the agent's only until a tab exists; the marker is searched in the
/// last 200 lines of history, which a noisy fixture could push it past;
/// and a retained marker from an earlier generation would satisfy a
/// relaunch falsely. Every caller today runs straight after `create`,
/// where all three hold. A caller that does not must not use this.
///
/// # Reading the failure
///
/// - a DEAD pane fails at once, and the captured text is what the chain
///   left on the pty — the scope wrapper's or the login shell's stderr when
///   the launch died before the shim, the agent's own output when it died
///   during its setup, or tmux's bare "Pane is dead" banner when nothing
///   was printed at all. The text is diagnostics, not attribution: it does
///   not say which link failed;
/// - a LIVE pane with no marker at the deadline is a launch that was
///   merely slow.
///
/// Every tmux call is bounded by the same deadline, because
/// `Command::output` has no timeout of its own and a tmux that stops
/// answering must surface as this setup failure rather than as a hung
/// test. Asks tmux directly rather than polling `list_sessions`, and that
/// is load-bearing for at least one caller: a list is how a supervisor
/// WITNESSES an exit, and
/// `same_boot_classification_is_per_session_and_never_interrupted` depends
/// on no list having happened before its reload.
pub(crate) async fn wait_for_agent_ready(sock: &std::path::Path, session_id: &str) {
    // No `=` exact-match prefix, unlike the `kill-session` targets
    // elsewhere in this suite: that prefix is a target-SESSION spelling,
    // and a pane target wearing it is refused outright ("can't find pane:
    // =fh-…") while `display-message` quietly expands every format to the
    // empty string — a failure mode that reads exactly like "the pane is
    // alive but silent", which is the one thing this helper must never
    // confuse. Bare `fh-<uuid>` is unambiguous anyway.
    let target = format!("fh-{session_id}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(AGENT_READY_SECS);
    let mut last_text = String::new();
    loop {
        // Scrollback as well as the visible grid: a dead pane's grid is
        // replaced by tmux's own "Pane is dead" banner, so the text worth
        // reporting only lives in history.
        let capture = tokio::time::timeout_at(
            deadline,
            tmux_query(sock, &["capture-pane", "-p", "-t", &target, "-S", "-200"]),
        )
        .await;
        let Ok(out) = capture else {
            panic!(
                "test setup: tmux stopped answering while waiting for session {session_id}'s \
                 agent to print {AGENT_READY_MARKER:?}; last pane text:\n{last_text}"
            );
        };
        last_text = String::from_utf8_lossy(&out.stdout).into_owned();
        if last_text.contains(AGENT_READY_MARKER) {
            return;
        }
        let dead = pane_format(sock, &target, "#{pane_dead}", deadline).await;
        let expired = tokio::time::Instant::now() >= deadline;
        if dead.as_deref() == Some("1") || expired {
            let status = pane_format(sock, &target, "#{pane_dead_status}", deadline).await;
            panic!(
                "test setup: session {session_id}'s agent never printed \
                 {AGENT_READY_MARKER:?} (pane_dead={dead:?}, pane_dead_status={status:?}, \
                 deadline expired={expired}), so this test's subject never started. A dead \
                 pane means the launch died before printing readiness — before farhelm's \
                 exec shim, or inside the agent's own setup — and the text below is whatever \
                 it left on the pty; a live pane means the launch was merely slow. Pane \
                 text:\n{last_text}"
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// One `display-message` format string evaluated against `target`, as a
/// trimmed string, or `None` when tmux refused to answer (the pane, its
/// window, or the whole server is gone) or did not answer by `deadline`.
///
/// Diagnostics only — every caller is already on a failure path, so a tmux
/// that cannot answer must not itself panic and replace the report with a
/// less informative one. The deadline is the caller's own, so a stuck
/// tmux cannot extend the wait it already lost.
async fn pane_format(
    sock: &std::path::Path,
    target: &str,
    format: &str,
    deadline: tokio::time::Instant,
) -> Option<String> {
    let out = tokio::time::timeout_at(
        deadline,
        tmux_query(sock, &["display-message", "-p", "-t", target, format]),
    )
    .await
    .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// [`basic_session`] plus [`wait_for_agent_ready`]: the shape to prefer
/// whenever a test's assertions are about an agent that is actually
/// RUNNING — which is to say almost all of them.
///
/// Kept separate from [`basic_session`] rather than folded into it so that
/// adopting the barrier stays a per-test decision: a handful of tests
/// deliberately act on a launch mid-flight, and a create that silently
/// waited for the agent would change what those pin. See
/// [`wait_for_agent_ready`] for the never-started failure this converts
/// from a corrupted assertion into a named setup failure.
pub(crate) async fn basic_session_ready(h: &Harness) -> (SessionInfo, farhelm_teststate::TestDir) {
    let (session, work) = basic_session(h).await;
    wait_for_agent_ready(&h.state.path().join("tmux.sock"), &session.id).await;
    (session, work)
}

/// Poll `list_sessions` until `session_id`'s status is no longer LIVE,
/// returning the settled `SessionInfo`.
///
/// Liveness is asked through `SessionStatus::is_live` rather than compared
/// against one variant: `PROTOCOL_VERSION` 10 split the single live status
/// into running/waiting/idle, and an equality here would silently start
/// treating a merely-idle agent as settled the moment the sampler lands.
///
/// Status is computed fresh from tmux at LIST time, never pushed
/// (`service.rs`'s `ListSessions` handler) — so observing a transition
/// (an agent exiting on its own, a stop's kill sweep completing) needs a
/// bounded poll rather than a single read racing tmux's own
/// `pane_dead`/`pane_dead_status` bookkeeping.
pub(crate) async fn wait_for_non_live_status(
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
            && !found.status.is_live()
        {
            return found.clone();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "session {session_id} never left a live status within {secs}s"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Poll `list_sessions` until one whole reply satisfies `settled`, and
/// return THAT reply's sessions.
///
/// Two separate reasons a single list is not enough, and this helper
/// answers both at once.
///
/// The first is the one [`wait_for_non_live_status`] documents: status is
/// computed fresh from tmux at LIST time rather than pushed, so any
/// transition is only observable by polling for it.
///
/// The second is that one list can be WRONG about a session that has not
/// transitioned at all. `pane_states` tolerates three tmux diagnostics by
/// degrading to an empty pane map (see `tmux.rs`'s `pane_states` and
/// `is_definitively_empty`), and an entry whose pane is missing from that
/// map honestly reports `Exited { exit_code: None }`. A loaded machine
/// that catches a list at such a moment turns a genuinely alive session
/// into an exited one, so a single-shot "assert it is live" fails on a
/// diagnostic the product is deliberately tolerant of. Waiting is not
/// weaker than asserting: a session that has really exited never becomes
/// live again, so the wait still fails — it just gets a bounded number of
/// chances to observe the truth first.
///
/// Returning the whole listing, rather than only the entry that satisfied
/// the predicate, is what lets a caller assert about SEVERAL rows of one
/// reply — "this one is alive AND that one is exited" is a claim about a
/// single observation of the world, and re-listing to check the second row
/// would reintroduce exactly the racing single-shot read this exists to
/// remove.
///
/// `what` names the condition for the timeout panic, which also reports
/// the last listing seen: "never settled" is not actionable without
/// knowing what it settled on instead.
pub(crate) async fn wait_for_listing(
    client: &SupervisorClient,
    secs: u64,
    what: &str,
    settled: impl Fn(&[SessionInfo]) -> bool,
) -> Vec<SessionInfo> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let listed = client
            .list_sessions()
            .await
            .expect("list while polling for a status");
        if settled(&listed.sessions) {
            return listed.sessions;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{what} did not hold within {secs}s (last listing: {:?})",
            listed.sessions
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Poll `list_sessions` until `session_id` reports a LIVE status, returning
/// the settled `SessionInfo`.
///
/// The overwhelmingly common shape of [`wait_for_listing`], and the only
/// status this suite ever waits for positively — every other settled state
/// is either reached through [`wait_for_non_live_status`] or asserted on
/// a listing that a wait already settled. See [`wait_for_listing`] for why
/// waiting rather than reading once is the right shape at all; reach for
/// it directly when the assertion spans more than one row of the reply.
///
/// A restart is the motivating case: its reply says the pane exists, not
/// that the agent inside it has execed yet, so "the relaunch is running"
/// is only ever observable by asking tmux again.
pub(crate) async fn wait_for_live_status(
    client: &SupervisorClient,
    session_id: &str,
    secs: u64,
) -> SessionInfo {
    let alive = |sessions: &[SessionInfo]| {
        sessions
            .iter()
            .any(|s| s.id == session_id && s.status.is_live())
    };
    wait_for_listing(
        client,
        secs,
        &format!("session {session_id} became live"),
        alive,
    )
    .await
    .into_iter()
    .find(|s| s.id == session_id)
    .expect("the predicate above matched this id")
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
pub(crate) fn tmux_records_exit_codes_reliably() -> bool {
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
pub(crate) async fn wait_for_exit_code(
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
pub(crate) async fn harness() -> Harness {
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
/// shared by every concurrently-running harness in the binary anyway.
pub(crate) async fn harness_with_timeouts(timeouts: SupervisorTimeouts) -> Harness {
    harness_with_seams(timeouts, SupervisorSeams::default()).await
}

/// Floor for the supervisor's tmux control-exchange budget
/// (`SupervisorTimeouts::tmux_exchange`) under every harness in this suite.
///
/// Production keeps `CONTROL_EXCHANGE_TIMEOUT` at 10s deliberately: it
/// bounds how long a wedged tmux can hold the supervisor-wide attachments
/// mutex, and that bound has to stay tight for a real deployment.
///
/// The 30s floor here is this repo's leading HYPOTHESIS for a class of
/// one-off CI failures, not a proven diagnosis — PLAN.md's M6.5 entry
/// records six distinct e2e tests each failing exactly once on a loaded
/// runner over one day, every one passing on rerun and in isolation, with
/// panic messages that line-map to these two control-mode budgets
/// expiring. That pattern is consistent with a busy-but-healthy tmux
/// occasionally taking longer than 10s/2s to answer and being read as
/// wedged, but nobody has reproduced the flake on demand to confirm it.
/// Loosening the budget here costs nothing if the hypothesis is wrong
/// (these tests do not otherwise depend on tmux answering slowly) and
/// removes a plausible cause if it is right, which is why it is worth
/// doing before the mechanism is settled.
const SUITE_TMUX_EXCHANGE_TIMEOUT: Duration = Duration::from_secs(30);

/// Floor for `SupervisorTimeouts::tmux_pane_list`; see
/// [`SUITE_TMUX_EXCHANGE_TIMEOUT`] for why this suite floors either budget
/// at all.
///
/// 10s, not some fraction of the 30s exchange floor: production's 2s and
/// 10s are not related by a ratio worth preserving (2:10 is a different
/// proportion from any floor pairing that also stays comfortably above
/// what a loaded runner needs), so there is no proportion here to keep.
/// The number is simply "generous enough for a busy pane-list call to
/// finish" — the same reasoning as the exchange floor's 30s, picked
/// independently. What IS preserved from production, deliberately, is the
/// STRUCTURAL relationship: this floor stays well below the exchange
/// floor (10s < 30s), so a slow pane listing still cannot eat the budget
/// the replay and cutover need afterward — see `PANE_LIST_TIMEOUT`'s own
/// docs for why that separation exists at all.
const SUITE_TMUX_PANE_LIST_TIMEOUT: Duration = Duration::from_secs(10);

/// Floor for `SupervisorTimeouts::sink_ready`; see
/// [`SUITE_TMUX_EXCHANGE_TIMEOUT`] for why this suite floors budgets at
/// all.
///
/// Production's 15s already comfortably exceeds a handful of sink respawn
/// retries (`SINK_RETRY_BASE` doubling up to `SINK_RETRY_MAX`), but a
/// sink's respawn attempt itself opens a fresh control-mode client through
/// [`SUITE_TMUX_EXCHANGE_TIMEOUT`]'s own budget — now 30s in this suite,
/// double the unfloored sink-ready wait. Left at 15s, a legitimately
/// answering-but-slow tmux could have its sink respawn attempt still
/// in flight when this budget gave up on it, failing the attach for the
/// same loaded-CI reason the tmux floors above exist to remove. 40s
/// clears one respawn attempt at the new exchange floor with room to
/// spare.
const SUITE_SINK_READY_TIMEOUT: Duration = Duration::from_secs(40);

/// `SupervisorTimeouts::default()` with this suite's loaded-CI floors
/// already applied to the three tmux-facing fields above.
///
/// For the handful of e2e sites that construct a `Supervisor` directly —
/// bypassing [`harness_with_seams`] entirely, typically to hold a
/// `Harness`'s pieces across a restart — but still go on to do real tmux
/// work (an attach, a stop that reaches the sink or the scope manager)
/// through the fresh supervisor. Prefer `harness()` /
/// `harness_with_seams()` whenever a `Harness` will do; reach for this only
/// where the test's own shape requires calling `Supervisor::new_with_exe`
/// or `Supervisor::new_with_seams` by hand. A caller that also wants to
/// override one of the OTHER fields (`stall_detach`, say) does so with
/// `SupervisorTimeouts { stall_detach: X, ..suite_timeouts() }`, the same
/// `..Default::default()` shape used everywhere else in this suite.
pub(crate) fn suite_timeouts() -> SupervisorTimeouts {
    floor_suite_timeouts(SupervisorTimeouts::default())
}

/// Raise `timeouts`' three tmux-facing fields to at least this suite's
/// floors, leaving every other field and any caller value ABOVE the floor
/// untouched.
///
/// A floor, not an assignment: an earlier version of
/// [`harness_with_seams`] unconditionally overwrote the two tmux budget
/// fields (`sink_ready` arrived floored from the start), which
/// would have silently shortened a larger value a future test supplied on
/// purpose (say, a test that deliberately wants an even slower simulated
/// tmux). `Duration::max` is what makes this a floor in both directions —
/// it raises a default or a too-small caller value up to the suite
/// minimum, and it leaves anything already at or above that minimum alone.
fn floor_suite_timeouts(mut timeouts: SupervisorTimeouts) -> SupervisorTimeouts {
    timeouts.tmux_exchange = timeouts.tmux_exchange.max(SUITE_TMUX_EXCHANGE_TIMEOUT);
    timeouts.tmux_pane_list = timeouts.tmux_pane_list.max(SUITE_TMUX_PANE_LIST_TIMEOUT);
    timeouts.sink_ready = timeouts.sink_ready.max(SUITE_SINK_READY_TIMEOUT);
    timeouts
}

/// Like [`harness_with_timeouts`], but with the supervisor's injection
/// points supplied too — the conversation-capture tests' entry point,
/// since they need both a private agent home and a capture window short
/// enough to prove two sessions in one directory do NOT overlap without
/// waiting out a production minute.
///
/// Every entry point above funnels through here, which is where the tmux
/// control-mode budgets are floored (see [`floor_suite_timeouts`]) —
/// unconditionally applied regardless of what `timeouts` carries for its
/// OTHER fields. A per-test override would have to be repeated at every
/// one of this suite's call sites (most of which only care about
/// `stall_detach` or an upload timeout and reach the tmux fields only
/// through `..Default::default()`), and a single missed site would
/// silently reintroduce the loaded-CI flake hypothesis this exists to
/// close off.
pub(crate) async fn harness_with_seams(
    timeouts: SupervisorTimeouts,
    seams: SupervisorSeams,
) -> Harness {
    let timeouts = floor_suite_timeouts(timeouts);
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("tempdir");
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

/// Drain terminal events until `pred` accepts the accumulated transcript.
///
/// The predicate rescans the whole buffer after every received event. A
/// `Detached` event ends the stream, but queued data behind it is drained and
/// the predicate gets one final chance because an agent's last output and
/// pane-death notice can race. The panic text preserves the waited-for label,
/// end reason, and lossy transcript for debugging.
pub(crate) async fn wait_until<S: TermSource>(
    rx: &mut S,
    seen: &mut Vec<u8>,
    secs: u64,
    what: &str,
    mut pred: impl FnMut(&[u8]) -> bool,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    let mut ended: Option<String> = None;
    loop {
        if pred(seen) {
            return;
        }
        if let Some(reason) = ended {
            panic!(
                "stream ended ({reason}) without {what}; transcript so far:\n{}",
                String::from_utf8_lossy(seen)
            );
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => seen.extend_from_slice(&bytes),
            // The replay-complete marker (PLAN_M5.md item 4) is
            // presentation metadata — it carries no bytes and this helper
            // is a plain text scan, so there is nothing for it to add to
            // `seen`. Its own ordering contract is pinned at the protocol
            // level in replay_marker.rs and, helm-side, in
            // farhelm-helm's client.rs/lib.rs; these supervisor-facing
            // tests deliberately do not re-assert it here.
            Ok(Some(TermEvent::ReplayComplete)) => {}
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
                "timed out waiting for {what}; transcript so far:\n{}",
                String::from_utf8_lossy(seen)
            ),
        }
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
/// the stream ends and only then reported missing. This now runs on the
/// shared [`wait_until`] drain core.
pub(crate) async fn wait_for(rx: &mut TermStream, seen: &mut Vec<u8>, needle: &str, secs: u64) {
    wait_for_inner(rx, seen, needle, secs).await;
}

/// Generic implementation for [`wait_for`], kept separate so the harness's
/// drain and predicate can be tested with a scripted source.
async fn wait_for_inner<S: TermSource>(rx: &mut S, seen: &mut Vec<u8>, needle: &str, secs: u64) {
    wait_until(rx, seen, secs, &format!("{needle:?}"), |seen| {
        String::from_utf8_lossy(seen).contains(needle)
    })
    .await;
}

/// Like [`wait_for`], but ordered: first wait until `first` appears, then
/// keep reading until `then` appears strictly AFTER `first`'s position.
///
/// Its job is the fixture-readiness barrier: `("FAKE-AGENT READY", "> ")`
/// waits for the fake agent's PROMPT rather than its ready marker, which
/// is what a test must do before it types if it later asserts on the
/// startup rows (see `reattach_replays_history_and_modes`). Anchoring
/// matters because `"> "` is not unique to startup, so only its position
/// after the marker identifies it. (It once also anchored assertions on a
/// dead pane's appended stop-time snapshot; that mechanism is gone.)
pub(crate) async fn wait_for_after(
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    first: &str,
    then: &str,
    secs: u64,
) {
    wait_for_after_inner(rx, seen, first, then, secs).await;
}

/// Generic implementation for [`wait_for_after`], kept separate so its
/// ordering predicate and diagnostics can be tested with a scripted source.
async fn wait_for_after_inner<S: TermSource>(
    rx: &mut S,
    seen: &mut Vec<u8>,
    first: &str,
    then: &str,
    secs: u64,
) {
    wait_until(
        rx,
        seen,
        secs,
        &format!("{then:?} after {first:?}"),
        |seen| {
            let text = String::from_utf8_lossy(seen);
            text.find(first)
                .is_some_and(|idx| text[idx + first.len()..].contains(then))
        },
    )
    .await;
}

const REPLAY_COMPLETE_RULE: &str = "this must be the first wait on a fresh attachment, because wait_for and wait_for_after consume the marker";

/// Return the byte offset where this attachment's live stream begins.
///
/// This must be the FIRST wait on a fresh attachment: `wait_for` and
/// `wait_for_after` swallow `ReplayComplete`, so calling this helper after
/// either one can only time out. It bounds this attachment's initial
/// catch-up and nothing later; a forced tmux pause can replay history into an
/// already-live attachment without another marker, so the three
/// `a_forced_tmux_pause_*` tests find that replay through its `ESC c` reset
/// instead. The supervisor emits this marker even when the pane has no
/// history, so a fresh attachment always has a boundary to receive.
pub(crate) async fn wait_for_replay_complete(
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    secs: u64,
) -> usize {
    wait_for_replay_complete_inner(rx, seen, secs).await
}

/// Generic implementation for [`wait_for_replay_complete`], kept separate so
/// the marker boundary can be tested with a scripted source.
async fn wait_for_replay_complete_inner<S: TermSource>(
    rx: &mut S,
    seen: &mut Vec<u8>,
    secs: u64,
) -> usize {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => seen.extend_from_slice(&bytes),
            Ok(Some(TermEvent::ReplayComplete)) => return seen.len(),
            Ok(Some(TermEvent::Detached(reason))) => {
                while let Ok(TermEvent::Data(bytes)) = rx.try_recv() {
                    seen.extend_from_slice(&bytes);
                }
                panic!(
                    "stream ended ({reason}) before ReplayComplete; {REPLAY_COMPLETE_RULE}; transcript so far:\n{}",
                    String::from_utf8_lossy(seen)
                );
            }
            Ok(None) => panic!(
                "stream ended (closed) before ReplayComplete; {REPLAY_COMPLETE_RULE}; transcript so far:\n{}",
                String::from_utf8_lossy(seen)
            ),
            Err(_) => panic!(
                "timed out waiting for ReplayComplete; {REPLAY_COMPLETE_RULE}; transcript so far:\n{}",
                String::from_utf8_lossy(seen)
            ),
        }
    }
}

/// Remove terminal presentation escapes and snapshot-only row padding, so a
/// test can read pane text without caring whether it arrived through the
/// attach snapshot or the live stream.
///
/// Lossy UTF-8 first, then every ECMA-48 escape sequence is removed whole.
/// The grammar covered: a CSI sequence (`ESC [`, then parameter bytes
/// `0x30..=0x3f` and intermediate bytes `0x20..=0x2f`, then one final byte
/// `0x40..=0x7e`) and a plain escape (`ESC`, zero or more intermediate bytes
/// `0x20..=0x2f`, then one final byte `0x30..=0x7e`), so `ESC(B` goes as a
/// unit, not as `ESC(` with a `B` left behind to glue onto a token. A
/// sequence the text cuts short (a lone trailing ESC, a CSI with no final
/// byte) is dropped up to the cut; an ESC followed by something that cannot
/// be a final byte (a non-ASCII character, say) drops only the ESC and keeps
/// the character intact. Then every line loses its trailing spaces and the
/// `\r` before its newline: that is the padding a snapshot row carries out
/// to the pane width, which is not part of what any fixture printed.
///
/// What it does NOT do: it does not touch invalid bytes, because they are
/// already gone by the time text exists (that is what the live boundary from
/// [`wait_for_replay_complete`] is for), and it does not parse string-type
/// sequences such as OSC, because tmux does not emit them around a pane
/// redraw and guessing at their terminators would risk swallowing real
/// output.
pub(crate) fn normalize_pane_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let is_intermediate = |b: u8| (0x20..=0x2f).contains(&b);
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_ref();
    while let Some(at) = rest.find('\x1b') {
        out.push_str(&rest[..at]);
        let after = &rest.as_bytes()[at + 1..];
        let mut end = 0;
        if after.first() == Some(&b'[') {
            end = 1;
            while after
                .get(end)
                .is_some_and(|b| (0x30..=0x3f).contains(b) || is_intermediate(*b))
            {
                end += 1;
            }
            if after.get(end).is_some_and(|b| (0x40..=0x7e).contains(b)) {
                end += 1;
            }
        } else {
            while after.get(end).is_some_and(|b| is_intermediate(*b)) {
                end += 1;
            }
            if after.get(end).is_some_and(|b| (0x30..=0x7e).contains(b)) {
                end += 1;
            }
        }
        // `end` only ever advanced over ASCII bytes, so it is a char boundary.
        rest = &rest[at + 1 + end..];
    }
    out.push_str(rest);
    out.split('\n')
        .map(|line| line.trim_end_matches([' ', '\r']))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Wait for `needle` in the normalized pane transcript.
///
/// This is opt-in because the default [`wait_for`] remains a raw matcher:
/// existing tests deliberately inspect escape sequences, including
/// bracketed-paste enablement and alternate-screen ordering. Keeping raw
/// matching as the default preserves those byte-level assertions while this
/// helper makes snapshot-shaped text indifferent to cursor addresses and
/// row padding.
// Opt-in primitive with no current caller, kept for the next test that reads
// a startup-printed row through the normalizer.
#[allow(dead_code)]
pub(crate) async fn wait_for_normalized(
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    needle: &str,
    secs: u64,
) {
    wait_for_normalized_inner(rx, seen, needle, secs).await;
}

/// Generic implementation for [`wait_for_normalized`], kept separate so
/// normalization-aware matching can share the same test seam as raw waits.
async fn wait_for_normalized_inner<S: TermSource>(
    rx: &mut S,
    seen: &mut Vec<u8>,
    needle: &str,
    secs: u64,
) {
    wait_until(rx, seen, secs, &format!("{needle:?}"), |seen| {
        normalize_pane_text(seen).contains(needle)
    })
    .await;
}

/// Extract complete records from the counter fixture without decoding
/// unrelated terminal bytes as UTF-8.
///
/// A record split exactly at replay cutover may have cursor-restoration
/// escapes inserted between its halves, in which case it is deliberately
/// absent here. The fixture flushes each short record as one PTY write so
/// that shape would itself be evidence that tmux split one write across
/// pane reads, not a parser artifact.
pub(crate) fn counter_records(transcript: &[u8]) -> Vec<u64> {
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

/// Wait for an attachment's `Detached` notice and return its reason.
///
/// Every takeover test needs this same wait, and the two failure modes it
/// distinguishes are exactly the ones a bug in the takeover path
/// produces: a stream that ends without any notice at all (the client was
/// torn down silently) versus one that simply never hears anything (the
/// incumbent was never kicked). Panicking with that distinction is the
/// point — an `Option` return would let a caller blur them.
pub(crate) async fn expect_detached(rx: &mut TermStream, secs: u64) -> String {
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

/// Pull the pid printed after `marker` (`"SELF-PID:"` or `"CHILD-PID:"`)
/// out of a fake-agent `spawner` transcript, panicking with the whole
/// transcript on any parse failure — a silent `0` or a wrong pid here
/// would make the process-tree-kill tests (in `session_lifecycle` and
/// the tab modules) pass or fail for reasons
/// unrelated to the code under test.
pub(crate) fn extract_pid(transcript: &[u8], marker: &str) -> u32 {
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
pub(crate) fn process_is_gone(pid: u32) -> bool {
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
pub(crate) async fn wait_until_pid_gone(pid: u32, secs: u64) {
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
pub(crate) async fn wait_for_child(parent: u32, secs: u64) -> u32 {
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
pub(crate) async fn wait_for_file(path: &std::path::Path, secs: u64) {
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
pub(crate) async fn wait_for_pid_file(path: &std::path::Path, secs: u64) -> u32 {
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

/// Probe once for a usable systemd user manager, and hand back the
/// ALREADY-PROBED manager for the caller to inject via
/// `SupervisorSeams::scopes` — or `None`, with the existing loud skip,
/// when this host has none.
///
/// This used to be `cgroup_path_available`, a bare predicate backed by its
/// own throwaway `ScopeManager`. That meant every scope-gated test ran TWO
/// independent probes of the same host property: this one, and a second
/// one the supervisor under test would lazily run on ITS OWN manager at
/// first launch (`core.rs`'s `scope_selected`, via
/// `scope::ScopeManager::available`). Each probe retries a real
/// `systemd-run`/`systemctl` round trip inside a shared 15s budget and then
/// caches whatever verdict it saw FOREVER in a `OnceCell` — cheap
/// insurance against repeating the experiment, but it also means two
/// probes racing a loaded user manager can durably disagree: one sees the
/// manager answer in time, the other times out and caches "unavailable"
/// forever. A test whose setup assertion required "this launch was
/// scoped" would then flake — not because scoping was broken, but because
/// the test's own precondition check and the supervisor's lazy probe asked
/// the same question twice and kept two different answers. (Flaked for
/// real in CI on 2026-08-03.)
///
/// Handing back the same `Arc<ScopeManager>` this function already probed,
/// for injection into a supervisor's seams, collapses the two probes into
/// one verdict: whichever supervisor is built with THIS `Arc` gets an
/// `OnceCell` that arrives pre-populated, so any of its calls that reach
/// the manager — `available` (which `scope_selected` reads at launch),
/// but equally `exists`/`kill` (which the stop path's `kill_scope` reads,
/// see sweep.rs) — can only ever agree with what this function saw, never
/// re-probe and never re-race. The one probe still runs through the
/// production entry point (`ScopeManager::systemd().available()`) rather
/// than a hand-rolled `which systemd-run`, so it keeps asking the same
/// question the supervisor asks, by the same experiment.
///
/// The guarantee is per-`Arc`, not per-test: a test that builds a SECOND
/// supervisor (a restart) and does not thread this same `Arc` into ITS
/// seams gets a fresh, unprobed `ScopeManager` for that supervisor, and
/// the two-probe race reopens there instead — `exists`/`kill` trigger
/// their own first-touch probe exactly as `available` does, independent
/// of whether some OTHER `ScopeManager` already answered the same
/// question. Callers that restart a supervisor mid-test must reuse the
/// probed manager on every construction, not just the first.
///
/// The residual this does NOT close: a manager that dies AFTER this probe
/// succeeds but before the launch's own `systemd-run` runs. That is the
/// product's own documented residual (`scope::ScopeManager`'s doc comment
/// on the cached verdict), not a test-harness gap — production has the
/// identical exposure between a supervisor's first launch and its Nth, and
/// engineering the test around it would mean proving a stronger guarantee
/// here than the code under test actually makes.
///
/// `#[ignore]` would be the obvious alternative and is the wrong one
/// (PLAN_M3.md item 10 says so explicitly): an ignored test is ignored
/// everywhere, including on the development hosts where the scope path is
/// the whole point. The message reaches CI's transcript because the test
/// step runs with `--show-output` (see `.github/workflows/ci.yml`).
pub(crate) async fn probed_scope_manager(
    test: &str,
) -> Option<Arc<farhelm_supervisor::scope::ScopeManager>> {
    let scopes = Arc::new(farhelm_supervisor::scope::ScopeManager::systemd());
    if scopes.available().await {
        return Some(scopes);
    }
    eprintln!(
        "SKIPPED {test}: this host has no usable systemd user manager, so the cgroup path \
         (PLAN_M3.md item 10) cannot be exercised here; the fallback path is what runs and \
         is proved by the rest of this suite"
    );
    None
}

/// [`probed_scope_manager`] plus the harness built from its verdict — the
/// entry point every scope-gated test wants EXCEPT one that goes on to
/// build a second supervisor (a restart) of its own, which needs the raw
/// `Arc` back to inject into that construction too. Bundling the common
/// case here (rather than making every caller repeat the probe-then-seam
/// plumbing) keeps that plumbing in one place while still handing back the
/// `Arc` for the one caller that needs it twice.
///
/// Returns `None`, after the existing loud skip, on a host with no usable
/// systemd user manager.
pub(crate) async fn scope_gated_harness(
    test: &str,
) -> Option<(Harness, Arc<farhelm_supervisor::scope::ScopeManager>)> {
    let scopes = probed_scope_manager(test).await?;
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            scopes: scopes.clone(),
            ..SupervisorSeams::default()
        },
    )
    .await;
    Some((h, scopes))
}

/// SIGKILLs a pid on drop — failure-safe cleanup for the one fixture
/// `MarkerCleanupGuard` cannot reach.
///
/// The cloaked daemon (`Script::SpawnerCloaked`) carries no marker by
/// construction, so the marker sweep this file's other guard performs
/// would never find it; and on a host without a user manager it is
/// expected to SURVIVE the stop under test. Its own 120s self-expiry is
/// the backstop under this, not a substitute for it.
pub(crate) struct PidKillGuard {
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
    pub(crate) fn arm(pid: u32) -> PidKillGuard {
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
pub(crate) struct MarkedDecoy(std::process::Child);

impl MarkedDecoy {
    /// `Command::env` sets the CHILD's environment, never this process's,
    /// which is the repo rule this file lives under. `sleep 120` bounds the
    /// leak if the test dies before its own cleanup runs.
    ///
    /// The kind markers are scrubbed because the test runner itself may be
    /// running inside a Farhelm session — the ordinary state of an agent
    /// developing farhelm on a farhelm-supervised box — where
    /// `FARHELM_AGENT_ID` (or, from a tab, `FARHELM_TAB_ID`) sits in the
    /// runner's own environment. A decoy inheriting either marker is no
    /// longer the legacy shape it exists to embody (session marker, no
    /// kind marker): the sweep's any-agent/any-tab rules read the
    /// inherited marker as "already claimed by someone else's launch" and
    /// the legacy bucket correctly refuses it, timing out both
    /// sweep-backstop tests. (The spawn-CLI tests take the same child-only
    /// precaution for the different trio of variables they care about —
    /// session id, token, supervisor socket.)
    ///
    /// Split from [`MarkedDecoy::spawn`] so a test can assert the
    /// CONFIGURED environment operations via `Command::get_envs` — the
    /// scrub only changes a child's actual environment on a host whose
    /// runner carries the markers, so inspecting a live child would prove
    /// nothing on clean CI, while the builder's op list is the same
    /// everywhere.
    pub(crate) fn command(session_id: &str) -> std::process::Command {
        let mut command = std::process::Command::new("sh");
        command
            .arg("-c")
            .arg("sleep 120")
            .env("FARHELM_SESSION_ID", session_id)
            .env_remove("FARHELM_AGENT_ID")
            .env_remove("FARHELM_TAB_ID")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        command
    }

    /// Launch [`MarkedDecoy::command`]'s spec, owning the `Child` so the
    /// zombie is reaped on drop (see the struct docs).
    pub(crate) fn spawn(session_id: &str) -> MarkedDecoy {
        MarkedDecoy(
            Self::command(session_id)
                .spawn()
                .expect("spawning the marked decoy"),
        )
    }

    pub(crate) fn pid(&self) -> u32 {
        self.0.id()
    }
}

impl Drop for MarkedDecoy {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
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
pub(crate) struct MarkerCleanupGuard {
    session_id: String,
}

impl MarkerCleanupGuard {
    pub(crate) fn new(session_id: impl Into<String>) -> Self {
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
pub(crate) fn marked_pids(session_id: &str) -> Vec<u32> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// A deterministic event source for testing wait behavior without a
    /// production `TermStream`, which has no public constructor.
    struct ScriptedSource {
        events: VecDeque<TermEvent>,
        pending: bool,
    }

    impl ScriptedSource {
        /// Build a source that returns the supplied events in order.
        fn new(events: impl IntoIterator<Item = TermEvent>) -> Self {
            Self {
                events: events.into_iter().collect(),
                pending: false,
            }
        }

        /// Build a source whose receive operation remains pending until the
        /// caller's timeout expires.
        fn pending() -> Self {
            Self::then_pending([])
        }

        /// Build a source that delivers `events` in order and then stays
        /// pending, the shape of a live attachment that has gone quiet:
        /// what a wait sees when the thing it waits for never comes.
        fn then_pending(events: impl IntoIterator<Item = TermEvent>) -> Self {
            Self {
                events: events.into_iter().collect(),
                pending: true,
            }
        }
    }

    impl TermSource for ScriptedSource {
        async fn recv(&mut self) -> Option<TermEvent> {
            match self.events.pop_front() {
                Some(event) => Some(event),
                None if self.pending => std::future::pending().await,
                None => None,
            }
        }

        fn try_recv(&mut self) -> Result<TermEvent, tokio::sync::mpsc::error::TryRecvError> {
            self.events
                .pop_front()
                .ok_or(tokio::sync::mpsc::error::TryRecvError::Empty)
        }
    }

    /// The normalizer must remove complete CSI and two-byte escapes without
    /// leaving a final byte glued to the first meaningful token.
    #[test]
    fn normalize_pane_text_removes_whole_sequences_and_nothing_else() {
        assert_eq!(normalize_pane_text(b"\x1b(B61 7f"), "61 7f");
        assert_eq!(normalize_pane_text(b"\x1b[1A61"), "61");
        assert_eq!(normalize_pane_text(b"\x1b[?25f61 \x1b[0m7a"), "61 7a");
        assert_eq!(normalize_pane_text(b"\x1b=61"), "61");
        assert_eq!(normalize_pane_text(b"61\x1b"), "61");
        assert_eq!(normalize_pane_text(b"61\x1b[2;1"), "61");
        assert_eq!(normalize_pane_text("\x1bé 61".as_bytes()), "é 61");
        assert_eq!(normalize_pane_text(b"plain 61 7f"), "plain 61 7f");
    }

    /// Snapshot rows contain terminal positioning and width padding; the
    /// normalized form must expose only the text a test means to read.
    #[test]
    fn normalize_pane_text_trims_snapshot_row_padding() {
        assert_eq!(
            normalize_pane_text(b"\x1b[2;1HREADY       \r\nPROMPT   \r\n"),
            "READY\nPROMPT\n"
        );
    }

    /// The returned replay boundary must point exactly between the initial
    /// catch-up bytes and the first live event.
    #[tokio::test]
    async fn wait_for_replay_complete_returns_pre_marker_offset() {
        let mut source = ScriptedSource::new([
            TermEvent::Data(b"snapshot".to_vec()),
            TermEvent::ReplayComplete,
            TermEvent::Data(b"live".to_vec()),
        ]);
        let mut seen = Vec::new();
        let live_from = wait_for_replay_complete_inner(&mut source, &mut seen, 1).await;
        assert_eq!(live_from, b"snapshot".len());
        assert_eq!(seen, b"snapshot");
    }

    /// A detach before the marker must explain that the helper was required
    /// to be the first wait on this fresh attachment.
    #[tokio::test]
    #[should_panic(expected = "this must be the first wait on a fresh attachment")]
    async fn wait_for_replay_complete_reports_missing_marker_rule() {
        let mut source = ScriptedSource::new([
            TermEvent::Data(b"snapshot".to_vec()),
            TermEvent::Detached("taken over".to_string()),
        ]);
        let mut seen = Vec::new();
        let _ = wait_for_replay_complete_inner(&mut source, &mut seen, 1).await;
    }

    /// Raw waits must match a needle split across independently delivered
    /// terminal chunks.
    #[tokio::test]
    async fn wait_for_matches_across_chunk_boundaries() {
        let mut source = ScriptedSource::new([
            TermEvent::Data(b"nee".to_vec()),
            TermEvent::Data(b"dle".to_vec()),
        ]);
        let mut seen = Vec::new();
        wait_for_inner(&mut source, &mut seen, "needle", 1).await;
        assert_eq!(seen, b"needle");
    }

    /// An ordinary wait timeout must name the raw needle that was missing.
    #[tokio::test]
    #[should_panic(expected = "timed out waiting for \"needle\"")]
    async fn wait_for_timeout_names_needle() {
        let mut source = ScriptedSource::pending();
        let mut seen = Vec::new();
        wait_for_inner(&mut source, &mut seen, "needle", 0).await;
    }

    /// Ordered waits must preserve the requirement that the second marker
    /// occurs after the first, even when both arrive in separate chunks.
    #[tokio::test]
    async fn wait_for_after_matches_across_chunk_boundaries() {
        let mut source = ScriptedSource::new([
            TermEvent::Data(b"first".to_vec()),
            TermEvent::Data(b"then".to_vec()),
        ]);
        let mut seen = Vec::new();
        wait_for_after_inner(&mut source, &mut seen, "first", "then", 1).await;
        assert_eq!(seen, b"firstthen");
    }

    /// An ordered wait timeout must retain both marker labels in its
    /// diagnostic so the missing ordering edge is identifiable.
    #[tokio::test]
    #[should_panic(expected = "timed out waiting for \"then\" after \"first\"")]
    async fn wait_for_after_timeout_names_markers() {
        let mut source = ScriptedSource::pending();
        let mut seen = Vec::new();
        wait_for_after_inner(&mut source, &mut seen, "first", "then", 0).await;
    }

    /// The normalized wait must match a needle that exists ONLY after
    /// escape removal and row-padding trim, across chunk boundaries, while
    /// leaving the raw bytes in `seen` for any later byte-level assertion.
    /// A raw `contains` implementation, or one that normalizes only the
    /// newest chunk, fails this.
    #[tokio::test]
    async fn wait_for_normalized_matches_only_after_normalization() {
        let mut source = ScriptedSource::new([
            TermEvent::Data(b"\x1b[2;1HREA".to_vec()),
            TermEvent::Data(b"DY     \r\n".to_vec()),
        ]);
        let mut seen = Vec::new();
        wait_for_normalized_inner(&mut source, &mut seen, "READY\n", 1).await;
        assert_eq!(seen, b"\x1b[2;1HREADY     \r\n");
        assert!(!String::from_utf8_lossy(&seen).contains("READY\n"));
    }

    /// The first-wait rule, exercised the way it is actually broken: a
    /// `wait_for` consumes the marker, and the boundary wait after it can
    /// only time out. Its timeout must name the rule, because a bare
    /// "timed out waiting for ReplayComplete" reads like a supervisor bug.
    #[tokio::test]
    #[should_panic(expected = "timed out waiting for ReplayComplete; this must be the first wait")]
    async fn wait_for_replay_complete_after_a_consuming_wait_names_the_rule() {
        let mut source = ScriptedSource::then_pending([
            TermEvent::Data(b"snapshot".to_vec()),
            TermEvent::ReplayComplete,
            TermEvent::Data(b"FAKE-AGENT READY".to_vec()),
        ]);
        let mut seen = Vec::new();
        wait_for_inner(&mut source, &mut seen, "READY", 1).await;
        let _ = wait_for_replay_complete_inner(&mut source, &mut seen, 0).await;
    }

    /// A `Detached` ends the stream but is not itself a failure: data still
    /// queued behind it is drained and the predicate gets one last chance,
    /// because an agent's final output and the pane-death notice race.
    /// Dropping either the drain or the recheck would revive that race.
    #[tokio::test]
    async fn wait_for_finds_a_needle_queued_behind_detached() {
        let mut source = ScriptedSource::new([
            TermEvent::Data(b"bye".to_vec()),
            TermEvent::Detached("pane died".to_string()),
            TermEvent::Data(b" needle".to_vec()),
        ]);
        let mut seen = Vec::new();
        wait_for_inner(&mut source, &mut seen, "needle", 1).await;
        assert_eq!(seen, b"bye needle");
    }
}

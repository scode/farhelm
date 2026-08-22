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
    ControlMsg, ErrorKind, Frame, FrameKind, SessionInfo, SessionStatus, TerminalSelector,
    UPLOAD_ABORT_REASON_STALLED, UPLOAD_CHUNK_BYTES,
};
pub(crate) use farhelm_supervisor::agent_kind::{CaptureWindow, CaptureWindowBounds, now_unix};
pub(crate) use farhelm_supervisor::launch::{spec_path_for_launch, status_path_for_spec};
pub(crate) use farhelm_supervisor::service::{
    CaptureStoreFault, CreateCrashSeam, CreateStage, LIST_SESSION_CAP, SessionSnapshot, Supervisor,
    SupervisorSeams, SupervisorTimeouts, handle_connection,
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

/// Drain terminal events until `needle` has appeared in the accumulated
/// output, failing the test after `secs`. Everything received is
/// appended to `seen`, so callers can make further assertions on the
/// transcript after the call returns.
///
/// A `Detached` event ends the stream but is not itself a failure: when
/// an agent exits, its last output and the pane-death notice race, and
/// the bytes may already be in hand. So the needle is re-checked after
/// the stream ends and only then reported missing.
pub(crate) async fn wait_for(rx: &mut TermStream, seen: &mut Vec<u8>, needle: &str, secs: u64) {
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
///
/// Its second job is the fixture-readiness barrier: `("FAKE-AGENT READY",
/// "> ")` waits for the fake agent's PROMPT rather than its ready marker,
/// which is what a test must do before it types if it later asserts on the
/// startup rows (see `reattach_replays_history_and_modes`). Anchoring
/// matters there for a different reason than above — `"> "` is not unique
/// to startup, so only its position after the marker identifies it.
pub(crate) async fn wait_for_after(
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
            // See `wait_for`'s twin arm just above: presentation-only, not
            // asserted on by this transcript scan.
            Ok(Some(TermEvent::ReplayComplete)) => {}
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

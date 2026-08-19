//! The helm connection manager against REAL transports (PLAN_M6.md item
//! 4's transport truth).
//!
//! `farhelm-helm`'s own `manager` tests cover the whole state machine
//! against scripted peers under a paused clock — every cadence, refusal,
//! and identity decision — and they are the right place for that, because
//! none of it needs a process. What they cannot cover is the part that is
//! only real when it is real: that a registry row actually reaches a
//! supervisor over the transport its `kind` implies, that the hello
//! identity crossing that transport is the supervisor's own, and that a
//! genuine session on the far side lands in helm.db's cache.
//!
//! Two transports, one test each:
//!
//! - An ssh row over the user's own `ssh` to `localhost`, running
//!   `farhelm internal stdio` against an ISOLATED remote state dir (the
//!   registry field that replaces M1's `--remote-state-dir`), so the
//!   "remote" supervisor on this same machine never touches the user's
//!   real state. Skipped loudly where passwordless `ssh localhost` is
//!   unavailable, exactly like the cgroup tests skip without a systemd
//!   user manager; CI provisions self-ssh so the skip never hides the path
//!   there.
//! - The reserved local row over the real unix socket in the helm's own
//!   state directory — the production arrangement, where helm.db and
//!   `supervisor.sock` are siblings.
//!
//! The missing-socket half of the local row's story (nothing listening →
//! unreachable with the manual-start hint) is pinned in `farhelm-helm`'s
//! own tests, where it needs no supervisor at all and therefore no slot,
//! no tmux, and no skip.

use crate::harness::*;
use farhelm_helm::manager::{
    Cadence, ConnectionManager, HostState, RefreshHealth, SystemTransport,
};
use farhelm_helm::store::{HelmStore, HostId};

/// Cadences for the real-transport tests.
///
/// The state machine's timing is pinned on the virtual clock in
/// `farhelm-helm`'s own tests; here a real supervisor, a real ssh child
/// and a real scheduler are in the loop, so the virtual clock is
/// unavailable and the production forty-five-second re-probe would only
/// be time spent sleeping. Short values make the whole test a matter of
/// seconds.
///
/// A ladder is kept rather than emptied, but it is INSURANCE, not coverage:
/// these fixtures wait for the supervisor's socket to accept before the
/// manager is started, so the startup race a ladder would rescue is
/// foreclosed here by construction. What it actually buys is that a
/// loaded runner hiccuping mid-handshake retries instead of failing the
/// test. The ladder's real coverage — that retries happen on schedule, and
/// only in the active window — is on the virtual clock in `farhelm-helm`.
fn brisk_cadence() -> Cadence {
    Cadence {
        connect_backoff: vec![
            std::time::Duration::from_millis(100),
            std::time::Duration::from_millis(250),
            std::time::Duration::from_millis(500),
        ],
        reprobe: std::time::Duration::from_millis(500),
        refresh: std::time::Duration::from_millis(200),
        // The two DEADLINES keep their production values. They bound a
        // real ssh handshake and a real supervisor's list walk here, and a
        // shortened one would turn a slow CI runner into a spurious
        // timeout — the exact failure this file's generous
        // [`SETTLE_TIMEOUT`] exists to avoid.
        ..Cadence::default()
    }
}

/// How long a real-transport test waits for a host to reach a state.
///
/// Generous rather than tight: an ssh handshake plus a supervisor's own
/// startup on a loaded CI runner is the slow path here, and the failure
/// this bound exists to report ("it never got there") is not one a shorter
/// wait would diagnose any better.
const SETTLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Whether `ssh localhost` works without a password or a prompt.
///
/// `BatchMode=yes` is what makes the probe honest: it turns every
/// interactive fallback (password, passphrase, host-key confirmation)
/// into an immediate failure instead of a hang, so a "yes" here really
/// does mean the transport under test can be opened unattended.
///
/// `StrictHostKeyChecking=yes`, deliberately NOT `accept-new`: a test
/// suite must not write to the developer's `~/.ssh/known_hosts`, and
/// `accept-new` does exactly that — silently trusting whatever key answers
/// on localhost and leaving the entry behind after the run. Strict checking
/// means the probe simply fails where localhost is not ALREADY trusted, and
/// the test skips loudly, which is the correct outcome for a precondition
/// the suite is not entitled to create. CI seeds the key explicitly with
/// `ssh-keyscan` (see `.github/workflows/ci.yml`), so the ssh path is still
/// genuinely exercised there rather than skipped.
pub(crate) async fn self_ssh_available() -> bool {
    tokio::process::Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "ConnectTimeout=10",
            "localhost",
            "true",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false)
}

/// A real supervisor serving on its own state directory's unix socket,
/// plus a direct client to it.
///
/// The direct client is the test's CONTROL channel, deliberately separate
/// from anything the manager does: it creates the session the manager is
/// then expected to discover, and its own `peer_hello()` is the
/// independent source of truth for what identity the supervisor reports —
/// so the assertion "the helm recorded the right identity" compares two
/// observations rather than one value against itself.
struct RemoteSupervisor {
    control: std::sync::Arc<SupervisorClient>,
    identity: String,
    /// Declared FIRST so it is dropped first: the accept loop must stop
    /// before the tmux server it may talk to and the state directory it
    /// serves from disappear underneath it.
    _serving: ServeGuard,
    _tmux: TmuxServerGuard,
    state: farhelm_teststate::TestDir,
    _work: farhelm_teststate::TestDir,
    session: SessionInfo,
    _slot: tokio::sync::SemaphorePermit<'static>,
}

/// A detached `serve()` accept loop, stopped when this guard drops.
///
/// `serve()` never returns on its own, so a spawned one outlives the test
/// that started it — for the whole test BINARY, holding its state directory
/// and its supervisor alive while every later test runs. Drop-based rather
/// than an explicit teardown call because a test that fails an assertion
/// never reaches an explicit anything, which is exactly when a leaked
/// accept loop does the most damage (this is the same reasoning
/// [`TmuxServerGuard`] is built on).
///
/// Aborting rather than joining: the loop has no shutdown signal to ask it
/// to finish, and its work is all cancel-safe — the supervisor's durable
/// state is transactional, so an aborted accept loop can lose an in-flight
/// request and nothing else.
struct ServeGuard(tokio::task::JoinHandle<()>);

impl Drop for ServeGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Where a detached `serve()` reports the error that ended it.
///
/// `serve()` can fail for reasons that have nothing to do with the test —
/// the state directory is already owned by another supervisor, the socket
/// cannot be bound — and a spawned task's error goes nowhere. Without this,
/// such a failure surfaces only as the socket wait timing out sixty seconds
/// later, reported as "the supervisor never started accepting connections",
/// which describes the symptom and hides the cause.
type ServeFailure = std::sync::Arc<std::sync::Mutex<Option<String>>>;

/// Boot a real supervisor, start it serving on its socket, and create one
/// fake-agent session on it.
///
/// The session matters: a host that connects but has nothing to list would
/// let a broken refresh pass as "cached zero sessions, correctly".
async fn remote_supervisor() -> RemoteSupervisor {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("state dir");
    let sup =
        Supervisor::new_with_exe_and_timeouts(state.path(), farhelm_bin().into(), suite_timeouts())
            .await
            .expect("supervisor");
    let tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    // `serve` never returns on its own; it owns the accept loop until the
    // guard below aborts it at teardown. Its error — if it ends early — is
    // parked where the socket wait can report it instead of timing out
    // against a supervisor that already failed.
    //
    // `sup` is MOVED into the task rather than cloned: the accept loop is
    // the only thing this fixture needs the supervisor value for (every
    // later interaction goes through the socket, like a real client's
    // would), and a retained clone would keep the supervisor alive past the
    // aborted loop for no purpose.
    let failure: ServeFailure = std::sync::Arc::new(std::sync::Mutex::new(None));
    let reported = std::sync::Arc::clone(&failure);
    let serve_task = ServeGuard(tokio::spawn(async move {
        if let Err(error) = sup.serve().await {
            *reported.lock().expect("serve failure mutex") = Some(format!("{error:#}"));
        }
    }));

    // Dial the socket the same way the helm's local transport does, so a
    // failure here is a failure of the supervisor's own serving path
    // rather than of a test-only shortcut.
    let control = wait_for_socket_client(state.path(), &failure).await;
    let identity = control
        .peer_hello()
        .host_identity
        .clone()
        .expect("a served supervisor always reports its minted identity");

    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = control
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            Some("connection-manager fixture".to_string()),
            80,
            24,
        )
        .await
        .expect("create the session the manager must discover");

    RemoteSupervisor {
        control,
        identity,
        _serving: serve_task,
        _tmux: tmux,
        state,
        _work: work,
        session,
        _slot: slot,
    }
}

/// Connect to a supervisor's unix socket, retrying until its accept loop
/// is actually up.
///
/// `Supervisor::serve` binds asynchronously in a spawned task, so a dial
/// issued immediately after the spawn legitimately races the bind. Polling
/// with a bound turns that into a clear failure if it never comes up,
/// instead of a sleep long enough to be flaky and short enough to
/// occasionally not be.
///
/// `failure` is checked on every pass so a serve that ENDED — rather than
/// one that is merely still starting — is reported with its own error
/// immediately, instead of being waited out and then blamed on slowness
/// (see [`ServeFailure`]).
async fn wait_for_socket_client(
    state_dir: &std::path::Path,
    failure: &ServeFailure,
) -> std::sync::Arc<SupervisorClient> {
    let deadline = tokio::time::Instant::now() + SETTLE_TIMEOUT;
    loop {
        if let Some(error) = failure.lock().expect("serve failure mutex").as_deref() {
            panic!(
                "the supervisor at {} stopped serving: {error}",
                state_dir.display()
            );
        }
        if let Ok(stream) = farhelm_supervisor::service::connect(state_dir).await {
            let (r, w) = tokio::io::split(stream);
            if let Ok(client) = SupervisorClient::start(r, w).await {
                return client;
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the supervisor at {} never started accepting connections",
            state_dir.display()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Wait for `host` to be connected with a completed refresh behind it —
/// the state every assertion below is made against.
async fn await_refreshed(manager: &ConnectionManager, host: HostId) -> HostState {
    tokio::time::timeout(
        SETTLE_TIMEOUT,
        manager.wait_for_state(host, |state| {
            matches!(
                state,
                HostState::Connected {
                    last_refresh: RefreshHealth::Ok { .. },
                    ..
                }
            )
        }),
    )
    .await
    .unwrap_or_else(|_| {
        panic!(
            "host {host} never reached a refreshed connected state; last seen {:?}",
            manager.state(host)
        )
    })
    .expect("actor is running")
}

/// One registry row's recorded identity, read straight from helm.db.
async fn recorded_identity(store: &HelmStore, host: HostId) -> Option<String> {
    store
        .list_hosts()
        .await
        .expect("list hosts")
        .into_iter()
        .find(|row| row.id == host)
        .expect("host row")
        .host_identity
}

/// An ssh registry row must reach a real supervisor through real ssh,
/// record the identity that supervisor actually reports, and drain its
/// real session list into helm.db.
///
/// This is the transport truth the scripted-peer tests deliberately cannot
/// give: `ssh_stdio_args`' quoting, the remote `farhelm internal stdio` proxy,
/// the per-row `remote_farhelm`/`remote_state_dir` fields that replaced
/// M1's argv flags, the handshake over an exec channel, and the identity
/// crossing it — every one of which is invisible to an in-memory duplex.
///
/// The remote state dir is the fixture's own tempdir, which is what keeps
/// a test that says "remote" on the SAME machine from ever touching the
/// developer's real `~/.local/state/farhelm`.
///
/// Skipped loudly (never silently) where passwordless `ssh localhost` is
/// unavailable — the repo's established pattern for a test whose
/// precondition is a property of the host, not of the code.
#[tokio::test]
async fn an_ssh_row_reaches_a_real_supervisor_and_caches_its_sessions() {
    if !self_ssh_available().await {
        eprintln!(
            "SKIPPED an_ssh_row_reaches_a_real_supervisor_and_caches_its_sessions: passwordless \
             `ssh localhost` is unavailable on this host"
        );
        return;
    }
    let remote = remote_supervisor().await;

    // The helm's own state dir, separate from the "remote" supervisor's:
    // it holds helm.db and the ssh ControlMaster sockets, and nothing else.
    let helm_state = farhelm_teststate::tempdir().expect("helm state dir");
    let store = HelmStore::open(&helm_state.path().join("helm.db"))
        .await
        .expect("open helm.db");
    let host = store
        .add_ssh_host(
            "localhost",
            Some(farhelm_bin()),
            Some(&remote.state.path().to_string_lossy()),
        )
        .await
        .expect("register the ssh host");

    let manager = ConnectionManager::start(
        store.clone(),
        std::sync::Arc::new(SystemTransport::new(helm_state.path())),
        brisk_cadence(),
    )
    .await
    .expect("start manager");

    let state = await_refreshed(&manager, host).await;
    let HostState::Connected {
        identity,
        build_version,
        ..
    } = &state
    else {
        unreachable!("filtered on a refreshed Connected state");
    };
    assert_eq!(
        identity.as_deref(),
        Some(remote.identity.as_str()),
        "the identity the helm learned must be the one the supervisor itself reports"
    );
    assert_eq!(
        build_version,
        farhelm_proto::BUILD_VERSION,
        "both sides are this same build, over a real transport"
    );
    assert_eq!(
        recorded_identity(&store, host).await.as_deref(),
        Some(remote.identity.as_str()),
        "first contact over ssh must reach helm.db, not just the in-memory state"
    );

    let cached: Vec<String> = store
        .cached_sessions(host)
        .await
        .expect("cached sessions")
        .into_iter()
        .map(|info| info.id)
        .collect();
    assert_eq!(
        cached,
        vec![remote.session.id.clone()],
        "the real session on the far side must have been drained into the cache"
    );

    // `remote` is deliberately still alive here, and stays so until this
    // function returns: the control client keeps the supervisor's socket in
    // use, and its state dir must outlive the ssh child running against it.
    // An explicit `drop` would say nothing the scope does not already.
}

/// The reserved local row must reach a supervisor on THIS machine over its
/// unix socket, in the production arrangement where helm.db and
/// `supervisor.sock` share one state directory.
///
/// The local row exists precisely so the helm's own machine is a host like
/// any other — identity learned the same way, sessions cached the same way
/// — which is what lets a stopped local supervisor's sessions serve stale
/// exactly as a down remote's do. Pinning it against a real socket is what
/// proves `SystemTransport` dispatches on `HostKind` correctly; a scripted
/// transport would answer the same for both kinds and prove nothing.
#[tokio::test]
async fn the_local_row_reaches_a_real_supervisor_over_its_unix_socket() {
    let remote = remote_supervisor().await;
    // Deliberately the SAME directory the supervisor serves from: that is
    // where a real helm keeps helm.db, and it is what makes the local
    // row's socket lookup a production path rather than a test-only
    // arrangement.
    let store = HelmStore::open(&remote.state.path().join("helm.db"))
        .await
        .expect("open helm.db");
    let local = store.list_hosts().await.expect("list hosts")[0].id;

    let manager = ConnectionManager::start(
        store.clone(),
        std::sync::Arc::new(SystemTransport::new(remote.state.path())),
        brisk_cadence(),
    )
    .await
    .expect("start manager");

    let state = await_refreshed(&manager, local).await;
    let HostState::Connected { identity, .. } = &state else {
        unreachable!("filtered on a refreshed Connected state");
    };
    assert_eq!(
        identity.as_deref(),
        Some(remote.identity.as_str()),
        "the local host learns its identity exactly like any other host"
    );
    assert_eq!(
        recorded_identity(&store, local).await.as_deref(),
        Some(remote.identity.as_str()),
        "the reserved local row is a cacheable, identifiable host row, not a synthetic entry"
    );

    let cached: Vec<String> = store
        .cached_sessions(local)
        .await
        .expect("cached sessions")
        .into_iter()
        .map(|info| info.id)
        .collect();
    assert_eq!(
        cached,
        vec![remote.session.id.clone()],
        "the local host's sessions must be cached like any other host's — the property that lets \
         a stopped local supervisor still serve a stale list"
    );

    // The control client proves the supervisor is still the same live
    // process the manager was talking to, rather than one that died and
    // left a cache behind.
    assert_eq!(remote.control.list_sessions().await.expect("list").total, 1);
}

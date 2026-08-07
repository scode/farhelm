//! A scripted fleet the REST tests stand the REAL serving path up on.
//!
//! Everything under test in `sessions.rs`, `uploads.rs`, `terminal.rs`,
//! `aggregate.rs`, and `hosts.rs` is
//! about what happens between a request and a host: the merged view, the
//! cursor, owner-lookup routing, the refusals. None of that is exercised by
//! calling a handler with a hand-made value, and all of it is exercised by
//! a real [`ConnectionManager`] over a real [`HelmStore`] with scripted
//! supervisors on the far end — which is what this module builds.
//!
//! ## The splice, and why it exists
//!
//! Two kinds of scripted host live here, and the second is the interesting
//! one.
//!
//! A **standalone** host answers its own handshake and serves a fixed
//! session list. That is enough for every test about the LIST: merging,
//! ordering, staleness, pagination, host management.
//!
//! A **spliced** host relays every message to a peer the test wrote itself,
//! EXCEPT `ListSessions`, which this module answers on that peer's behalf.
//! That exception is the whole trick. A connection manager refreshes its
//! cache on a cadence, so a test peer scripted to expect (say) a
//! `StopSession` would instead be handed the manager's housekeeping list
//! request and fall over — and every one of this crate's several dozen
//! REST tests scripts exactly that kind of one-shot exchange. Answering
//! `ListSessions` here keeps those peers written the way they read best
//! ("expect one request, reply once") while the manager underneath them
//! behaves exactly as it does in production. It also seeds the cache the
//! owner lookup routes through, which is what makes a spliced test's
//! session id routable at all.
//!
//! A spliced peer's EXIT is deliberately not propagated: this module keeps
//! the manager-side connection open and goes on answering list requests
//! after the test's peer has finished its one exchange and returned. The
//! alternative — closing the manager's connection when the peer returns —
//! would flip the host out of `Connected` mid-test and turn every later
//! assertion in that test into a routing refusal.

use crate::AppState;
use crate::manager::{
    Cadence, ConnectionManager, HostState, HostTransport, RefreshHealth, TransportPair,
};
use crate::store::{HelmStore, HostId, HostRow};
use farhelm_proto::io::{FrameReader, FrameWriter, parse_control};
use farhelm_proto::{ControlMsg, PROTOCOL_VERSION, RestartOffer, SessionInfo, SessionStatus};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream};
use tokio::sync::{broadcast, mpsc, watch};

/// How long a harness waits for a host to reach the state a test needs.
///
/// Generous rather than tight: these run alongside the rest of the crate's
/// suite on shared runners, and the failure this bound reports ("it never
/// got there") is not diagnosed any better by a shorter wait.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Cadences for a REST test.
///
/// Two deliberate settings. The connect ladder is nearly instant, because
/// no test here is about retry timing — that is pinned on the virtual clock
/// in `manager.rs` — and a test that has to wait a real second for its
/// first connection is a test nobody runs. The refresh interval is an HOUR,
/// which given that the first refresh happens immediately on connect means
/// each connection refreshes exactly ONCE: the cache a test asserts against
/// is then the one its script produced, not whichever pass happened to land
/// last.
fn test_cadence() -> Cadence {
    Cadence {
        connect_backoff: vec![Duration::from_millis(10), Duration::from_millis(20)],
        reprobe: Duration::from_secs(3600),
        refresh: Duration::from_secs(3600),
        ..Cadence::default()
    }
}

/// A minimal session that round-trips — the filler every list-shaped test
/// needs and no test is about.
pub(crate) fn session(id: &str, created_at: i64) -> SessionInfo {
    SessionInfo {
        creation_seq: None,
        parent: None,
        archived: false,
        id: id.to_string(),
        title: id.to_string(),
        created_at,
        cwd: format!("/{id}"),
        invocation: "agent".to_string(),
        status: SessionStatus::Running,
        annotation: None,
        restart_offer: RestartOffer::default(),
        tabs: Vec::new(),
        source_profile: None,
    }
}

/// What one scripted host does when dialed.
pub(crate) struct HostScript {
    /// `false` fails the DIAL itself — the host is down and no peer runs.
    pub(crate) reachable: bool,
    pub(crate) protocol: u32,
    pub(crate) build: String,
    /// The identity the hello reports. `None` models a supervisor with
    /// none, which connects but caches nothing.
    pub(crate) identity: Option<String>,
    /// The session list this host serves, in one page.
    pub(crate) sessions: Vec<SessionInfo>,
    /// A test-owned peer to splice this host's connection to (module
    /// docs). Taken on the first dial; a reconnect after that falls back to
    /// standalone behavior, which is the honest thing to do — the test's
    /// scripted exchange is spent.
    pub(crate) peer: Option<DuplexStream>,
    /// The TRANSPORT panics when this host is dialed — the stand-in for any
    /// bug that takes an actor's task down, which is what leaves an entry
    /// `retired`. There is no honest smaller stand-in, since a panic is
    /// exactly what an unexpected bug is.
    pub(crate) panic_on_dial: bool,
}

impl Default for HostScript {
    fn default() -> HostScript {
        HostScript {
            reachable: true,
            protocol: PROTOCOL_VERSION,
            build: "peer-build".to_string(),
            identity: None,
            sessions: Vec::new(),
            peer: None,
            panic_on_dial: false,
        }
    }
}

/// The scripted transport: one [`HostScript`] per registry row.
pub(crate) struct ScriptedFleet {
    scripts: Mutex<HashMap<HostId, HostScript>>,
    /// Every dial, whole — which host, and the registry row it was pointed
    /// at.
    ///
    /// The ROW rather than a count, because several properties are only
    /// observable in what was dialed: that a retarget reached the transport
    /// rather than merely the registry, and that a host's
    /// `remote_farhelm`/`remote_state_dir` are handed to the thing that
    /// opens the connection rather than just recorded beside it. Both look
    /// identical in every API response if they are wrong.
    dials: Mutex<Vec<HostRow>>,
    /// One kill channel PER HOST, not one for the whole fleet.
    ///
    /// Per-host because taking one host down must be observable as one host
    /// going down: a fleet-wide kill would bounce every other connection
    /// too, and a test asserting "only that host's rows went stale" would
    /// then be racing the others' reconnects rather than testing the merge.
    kills: Mutex<HashMap<HostId, broadcast::Sender<()>>>,
    /// Whether a host with no script of its own is DOWN rather than the
    /// ordinary reachable default — see [`Self::take_down_next_host`].
    unknown_hosts_are_down: std::sync::atomic::AtomicBool,
    /// Shared with every running peer: the list barrier and the request
    /// counter (see [`ListGate`]).
    gate: Arc<ListGate>,
}

/// What a test can do to a scripted host's LIST replies from outside.
///
/// Separate from [`ScriptedFleet`] and `Arc`-shared because the peers need
/// it: the transport hands out `&self`, so a spawned peer cannot borrow the
/// fleet, and cloning one small shared thing is cleaner than teaching the
/// transport to hand out a `Weak`.
struct ListGate {
    /// A gate the NEXT list reply for a host must wait on — see
    /// [`ScriptedFleet::hold_next_list`].
    holds: Mutex<HashMap<HostId, tokio::sync::oneshot::Receiver<()>>>,
    /// How many `ListSessions` requests every host has received, so a test
    /// can wait for a refresh to have STARTED rather than sleep and hope.
    requests: watch::Sender<usize>,
    /// The same count, PER HOST — what a test needs to reason about one
    /// host's refresh loop while other hosts are also connected. The
    /// fleet-wide counter above cannot distinguish "this host refreshed
    /// again" from "some other host did", which is the difference between a
    /// deterministic barrier and a coincidence.
    per_host: watch::Sender<HashMap<HostId, usize>>,
    /// What each host's list currently contains, mirrored out of the
    /// scripts so a running peer can read it.
    ///
    /// LIVE rather than a snapshot taken when the connection was dialed: a
    /// host whose sessions never changed for the life of a connection
    /// cannot model the ordinary case where one appears, so a test that
    /// creates a session and then expects the host to report it had no way
    /// to say so. Each reply is built when its REQUEST arrives, which is
    /// what lets a held reply be genuinely stale relative to whatever
    /// happened while it was held.
    lists: Mutex<HashMap<HostId, Arc<Vec<SessionInfo>>>>,
}

impl ScriptedFleet {
    fn new() -> Arc<ScriptedFleet> {
        Arc::new(ScriptedFleet {
            scripts: Mutex::new(HashMap::new()),
            dials: Mutex::new(Vec::new()),
            kills: Mutex::new(HashMap::new()),
            unknown_hosts_are_down: std::sync::atomic::AtomicBool::new(false),
            gate: Arc::new(ListGate {
                holds: Mutex::new(HashMap::new()),
                requests: watch::Sender::new(0),
                per_host: watch::Sender::new(HashMap::new()),
                lists: Mutex::new(HashMap::new()),
            }),
        })
    }

    /// This host's kill channel, created on first use — which happens
    /// either when it is dialed or when a test kills it, whichever comes
    /// first.
    fn kill_channel(&self, host: HostId) -> broadcast::Sender<()> {
        self.kills
            .lock()
            .expect("kill mutex")
            .entry(host)
            .or_insert_with(|| broadcast::channel(8).0)
            .clone()
    }

    /// Make this host's NEXT list reply wait until the returned sender is
    /// used (or dropped).
    ///
    /// The barrier a refresh-versus-create race needs: it lets a test hold a
    /// drain that has already STARTED — so the reply it eventually gives
    /// genuinely predates whatever happened in between — instead of
    /// approximating that with a sleep, which is how a race test becomes a
    /// flake.
    pub(crate) fn hold_next_list(&self, host: HostId) -> tokio::sync::oneshot::Sender<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.gate.holds.lock().expect("hold mutex").insert(host, rx);
        tx
    }

    /// Wait until at least `n` list requests have been received across the
    /// fleet.
    pub(crate) async fn await_list_requests(&self, n: usize) {
        let mut rx = self.gate.requests.subscribe();
        let _ = rx.wait_for(|seen| *seen >= n).await;
    }

    /// How many `ListSessions` requests THIS host has received so far.
    ///
    /// Per-host rather than fleet-wide, and that is what makes it usable as a
    /// BARRIER: a test asserting that something did not happen during one
    /// host's refresh has to be able to tell that host's next request from
    /// some other host's, which the fleet-wide counter above cannot do. See
    /// [`Harness::refresh_to_completion`].
    fn host_requests(&self, host: HostId) -> usize {
        self.gate.per_host.borrow().get(&host).copied().unwrap_or(0)
    }

    /// Wait until `host` has received at least `n` list requests.
    async fn await_host_requests(&self, host: HostId, n: usize) {
        let mut rx = self.gate.per_host.subscribe();
        let _ = rx
            .wait_for(|seen| seen.get(&host).copied().unwrap_or(0) >= n)
            .await;
    }

    /// How many times `host` has been dialed.
    pub(crate) fn dials(&self, host: HostId) -> usize {
        self.dialed_rows(host).len()
    }

    /// Every registry row `host` was dialed with, in order.
    pub(crate) fn dialed_rows(&self, host: HostId) -> Vec<HostRow> {
        self.dials
            .lock()
            .expect("dial log mutex")
            .iter()
            .filter(|row| row.id == host)
            .cloned()
            .collect()
    }

    /// Change a host's script in place — what the far side "becoming"
    /// something else looks like: a supervisor upgraded past this helm's
    /// protocol, a machine reinstalled under a new identity, a host
    /// switched off.
    ///
    /// Takes effect on the NEXT connection, since a hello is exchanged
    /// once; pair it with [`Self::kill_connection`] to make the change
    /// land now.
    pub(crate) fn edit(&self, host: HostId, edit: impl FnOnce(&mut HostScript)) {
        let mut scripts = self.scripts.lock().expect("script mutex");
        let script = scripts.entry(host).or_default();
        edit(script);
        // Mirrored out so running peers see it: a script edit that only
        // took effect on the NEXT connection could not model a session
        // appearing on a host that is already up.
        self.gate
            .lists
            .lock()
            .expect("list mutex")
            .insert(host, Arc::new(script.sessions.clone()));
    }

    /// Drop this host's live connection — the helm sees EOF, exactly as
    /// when a supervisor is killed or an ssh channel dies. The next dial
    /// picks up whatever [`Self::edit`] has since changed.
    pub(crate) fn kill_connection(&self, host: HostId) {
        let _ = self.kill_channel(host).send(());
    }

    /// Make `host` unreachable and drop its connection, so it enters the
    /// reconnect path and stays there — the "someone switched that machine
    /// off" move.
    pub(crate) fn take_down(&self, host: HostId) {
        self.edit(host, |script| script.reachable = false);
        self.kill_connection(host);
    }

    /// Script the NEXT host to be dialed as unreachable, before its id
    /// exists.
    ///
    /// A test that registers a host through `POST /api/hosts` cannot script
    /// it first: the id is minted by the request. Taking it down afterwards
    /// is a different scenario — the host connected and then went away —
    /// and specifically not the one "add always registers" is about, which
    /// is a machine that is switched off at the moment it is added. Any id
    /// with no script yet answers from `HostScript::default()`, so this
    /// simply makes that default unreachable for ids not yet seen.
    pub(crate) fn take_down_next_host(&self) {
        self.unknown_hosts_are_down
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl HostTransport for ScriptedFleet {
    fn connect<'a>(
        &'a self,
        host: &'a HostRow,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<TransportPair>> + Send + 'a>> {
        let id = host.id;
        Box::pin(async move {
            self.dials
                .lock()
                .expect("dial log mutex")
                .push(host.clone());
            let (behavior, peer) = {
                let mut scripts = self.scripts.lock().expect("script mutex");
                let unknown_is_down = self
                    .unknown_hosts_are_down
                    .load(std::sync::atomic::Ordering::Relaxed);
                let script = scripts.entry(id).or_insert_with(|| HostScript {
                    reachable: !unknown_is_down,
                    ..HostScript::default()
                });
                // Panicked with NO lock held below: the guard is dropped
                // with this block, so a scripted panic cannot poison a mutex
                // the rest of the fixture still needs.
                let panic_on_dial = script.panic_on_dial;
                if panic_on_dial {
                    drop(scripts);
                    panic!("scripted panic dialing host {id}");
                }
                if !script.reachable {
                    anyhow::bail!("scripted host {id} is down");
                }
                self.gate
                    .lists
                    .lock()
                    .expect("list mutex")
                    .entry(id)
                    .or_insert_with(|| Arc::new(script.sessions.clone()));
                (
                    PeerBehavior {
                        protocol: script.protocol,
                        build: script.build.clone(),
                        identity: script.identity.clone(),
                    },
                    script.peer.take(),
                )
            };
            let (ours, theirs) = tokio::io::duplex(256 * 1024);
            let kill = self.kill_channel(id).subscribe();
            let gate = Arc::clone(&self.gate);
            // Standalone, or spliced to the test's own peer — see the
            // module docs for what the splice buys.
            tokio::spawn(async move {
                match peer {
                    None => run_standalone(theirs, behavior, gate, id, kill).await,
                    Some(peer) => run_spliced(theirs, behavior, peer, gate, id, kill).await,
                }
            });
            let (r, w) = tokio::io::split(ours);
            Ok((
                Box::new(r) as Box<dyn AsyncRead + Send + Unpin>,
                Box::new(w) as Box<dyn AsyncWrite + Send + Unpin>,
            ))
        })
    }
}

/// The parts of a [`HostScript`] a running peer needs — everything except
/// the spliced stream, which is moved separately because it is not
/// clonable.
#[derive(Clone)]
struct PeerBehavior {
    protocol: u32,
    build: String,
    identity: Option<String>,
}

impl PeerBehavior {
    fn hello(&self) -> ControlMsg {
        ControlMsg::Hello {
            protocol_version: self.protocol,
            build_version: self.build.clone(),
            role: "supervisor".to_string(),
            host_identity: self.identity.clone(),
            auth: None,
        }
    }

    /// This host's whole session list as one page. Pagination of the WIRE
    /// list is pinned in `manager.rs` against a peer that really does
    /// chunk; nothing at the REST layer depends on how many pages a host's
    /// drain took, so serving one page here keeps these tests about the
    /// merge rather than about the drain.
    fn list_reply(&self, gate: &ListGate, host: HostId, req_id: u64) -> ControlMsg {
        let sessions = gate
            .lists
            .lock()
            .expect("list mutex")
            .get(&host)
            .map(|list| list.as_ref().clone())
            .unwrap_or_default();
        ControlMsg::SessionList {
            req_id,
            total: sessions.len() as u64,
            sessions,
            next_cursor: None,
        }
    }
}

/// A whole scripted supervisor: hello, then a fixed list forever.
///
/// Writes its hello WITHOUT waiting for the helm's, matching the protocol's
/// crossing-hellos rule — a peer that waited would deadlock against a helm
/// that is also waiting.
impl ListGate {
    /// Record that a list request arrived, and hand back this host's
    /// barrier if a test armed one.
    ///
    /// Returns the gate rather than awaiting it, because the caller decides
    /// where the waiting may safely happen. In the spliced peer it may not
    /// happen inline: that task is the only reader of the helm-to-peer
    /// direction, so a held list reply would also hold up the CREATE the
    /// test is about to make, and the barrier would deadlock the very race
    /// it exists to stage.
    fn admit(&self, host: HostId) -> Option<tokio::sync::oneshot::Receiver<()>> {
        self.requests.send_modify(|seen| *seen += 1);
        self.per_host
            .send_modify(|seen| *seen.entry(host).or_insert(0) += 1);
        self.holds.lock().expect("hold mutex").remove(&host)
    }
}

async fn run_standalone(
    io: DuplexStream,
    behavior: PeerBehavior,
    gate: Arc<ListGate>,
    host: HostId,
    mut kill: broadcast::Receiver<()>,
) {
    let (r, w) = tokio::io::split(io);
    let mut reader = FrameReader::new(r);
    let mut writer = FrameWriter::new(w);
    if writer.write_control(&behavior.hello()).await.is_err() {
        return;
    }
    loop {
        let frame = tokio::select! {
            _ = kill.recv() => return,
            frame = reader.read_frame() => frame,
        };
        let Ok(Some(frame)) = frame else { return };
        let Ok(ControlMsg::ListSessions { req_id, .. }) = parse_control(&frame) else {
            // Everything else is ignored rather than refused: a standalone
            // host exists to be listed, and a test that needs a session
            // operation answered splices its own peer instead.
            continue;
        };
        if let Some(held) = gate.admit(host) {
            // Standalone peers serve nothing but lists, so waiting inline
            // holds up nothing else.
            let _ = held.await;
        }
        if writer
            .write_control(&behavior.list_reply(&gate, host, req_id))
            .await
            .is_err()
        {
            return;
        }
    }
}

/// Relay the helm's connection to a test's peer, answering `ListSessions`
/// here instead of forwarding it (module docs).
///
/// The manager-side writer is owned by ONE task fed through a channel:
/// both the list answers and the peer's own replies have to reach it, and
/// two writers on one half would interleave partial frames.
async fn run_spliced(
    io: DuplexStream,
    behavior: PeerBehavior,
    peer: DuplexStream,
    gate: Arc<ListGate>,
    host: HostId,
    mut kill: broadcast::Receiver<()>,
) {
    let (helm_r, helm_w) = tokio::io::split(io);
    let (peer_r, peer_w) = tokio::io::split(peer);
    let (to_helm, mut outbound) = mpsc::channel::<farhelm_proto::Frame>(64);

    let writer_task = tokio::spawn(async move {
        let mut writer = FrameWriter::new(helm_w);
        while let Some(frame) = outbound.recv().await {
            if writer.write_frame(&frame).await.is_err() {
                return;
            }
        }
    });

    // Peer → helm, verbatim except for the hello's identity (see
    // `identified_hello`). Ends when the test's peer finishes its exchange;
    // the manager-side connection deliberately stays open (module docs), so
    // this task simply returns and the one below carries on.
    let upstream = tokio::spawn({
        let to_helm = to_helm.clone();
        let behavior = behavior.clone();
        async move {
            let mut reader = FrameReader::new(peer_r);
            while let Ok(Some(frame)) = reader.read_frame().await {
                let frame = identified_hello(frame, &behavior);
                if to_helm.send(frame).await.is_err() {
                    return;
                }
            }
        }
    });

    // Helm → peer, minus the list requests this harness answers itself.
    let downstream = async {
        let mut reader = FrameReader::new(helm_r);
        let mut writer = FrameWriter::new(peer_w);
        while let Ok(Some(frame)) = reader.read_frame().await {
            if let Ok(ControlMsg::ListSessions { req_id, .. }) = parse_control(&frame) {
                // Built when the REQUEST arrives, not when the reply is
                // sent: that is what makes a held reply genuinely describe
                // the world as it was before the barrier was released.
                let reply =
                    farhelm_proto::Frame::control(&behavior.list_reply(&gate, host, req_id));
                match gate.admit(host) {
                    // Held: answered from a task of its own so this relay
                    // keeps carrying everything else (see `ListGate::admit`).
                    Some(held) => {
                        let to_helm = to_helm.clone();
                        tokio::spawn(async move {
                            let _ = held.await;
                            let _ = to_helm.send(reply).await;
                        });
                    }
                    None if to_helm.send(reply).await.is_err() => return,
                    None => {}
                }
                continue;
            }
            if writer.write_frame(&frame).await.is_err() {
                return;
            }
        }
    };

    tokio::select! {
        _ = kill.recv() => {}
        _ = downstream => {}
    }
    upstream.abort();
    writer_task.abort();
}

/// Give a spliced peer's hello the host identity it did not report.
///
/// The one place this harness rewrites what a test's peer said, and it
/// earns the exception. Test peers complete their handshake with
/// `io::handshake`, which has no identity parameter at all, so every one of
/// them reports `None` — and a supervisor reporting no identity connects
/// but writes NO cache, since the cache's identity binding has nothing to
/// bind to. That is correct production behavior and exactly wrong for a
/// fixture: the session those tests operate on would never reach the cache,
/// so owner-lookup routing would 404 every single one of them. A real M6
/// supervisor always mints an identity, so injecting one here makes the
/// spliced peer MORE faithful, not less.
///
/// Anything that is not a hello, and a hello that already carries an
/// identity, passes through untouched.
fn identified_hello(frame: farhelm_proto::Frame, behavior: &PeerBehavior) -> farhelm_proto::Frame {
    let Some(identity) = behavior.identity.as_ref() else {
        return frame;
    };
    match parse_control(&frame) {
        Ok(ControlMsg::Hello {
            protocol_version,
            build_version,
            role,
            host_identity: None,
            auth: None,
        }) => farhelm_proto::Frame::control(&ControlMsg::Hello {
            protocol_version,
            build_version,
            role,
            host_identity: Some(identity.clone()),
            auth: None,
        }),
        _ => frame,
    }
}

/// A running scripted fleet plus the real router over it.
///
/// FIELD ORDER IS DROP ORDER, and it is load-bearing here: struct fields
/// drop in DECLARATION order, so the temp directory has to come LAST or it
/// is deleted out from under a store and a manager that are still using it
/// (an earlier version declared it first, with a comment claiming the
/// opposite). `served` precedes the rest for the same reason in the other
/// direction: the listeners it holds must stop before the state they serve
/// goes away.
pub(crate) struct Harness {
    /// Guards for every loopback server [`Self::serve`] started, aborted
    /// when this harness drops.
    served: Vec<ServerGuard>,
    pub(crate) store: HelmStore,
    pub(crate) manager: Arc<ConnectionManager>,
    pub(crate) fleet: Arc<ScriptedFleet>,
    pub(crate) state: Arc<AppState>,
    /// A real device credential persisted in this harness's helm.db.
    /// [`Self::router`] and [`Self::serve`] apply it by default so the
    /// existing suite continues to exercise handlers behind auth, while the
    /// explicit unauthenticated variants expose the enforcement boundary.
    device_secret: String,
    port: u16,
    /// Declared LAST so it is dropped last — see this struct's own docs.
    _dir: tempfile::TempDir,
}

/// An axum server aborted when dropped.
///
/// `axum::serve` never returns on its own, so a spawned one outlives the
/// test that started it — holding a loopback port and, through the router,
/// a whole fleet of connection actors — for the rest of the test BINARY.
/// Drop-based rather than an explicit teardown because a test that fails an
/// assertion never reaches an explicit anything, which is exactly when a
/// leaked server does the most damage.
struct ServerGuard(tokio::task::JoinHandle<()>);

impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Apply the harness's real persisted device credential to every request.
///
/// This is deliberately outside the production router. Tests still traverse
/// the real auth middleware and storage lookup, but the hundreds of fixtures
/// whose subject is a handler do not each repeat exchange ceremony. Both
/// explicit transports are supplied because a raw WebSocket upgrade does not
/// use the REST authorization header.
fn authenticated_router(router: axum::Router, secret: String) -> axum::Router {
    let authorization = axum::http::HeaderValue::from_str(&format!("Bearer {secret}"))
        .expect("the harness secret is an Authorization value");
    let protocols = axum::http::HeaderValue::from_str(&format!(
        "{}, farhelm-device-{secret}",
        crate::auth::WS_PROTOCOL
    ))
    .expect("the harness secret is a WebSocket protocol value");
    router.layer(axum::middleware::from_fn(
        move |mut request: axum::extract::Request, next: axum::middleware::Next| {
            let authorization = authorization.clone();
            let protocols = protocols.clone();
            async move {
                request
                    .headers_mut()
                    .insert(axum::http::header::AUTHORIZATION, authorization);
                request
                    .headers_mut()
                    .insert(axum::http::header::SEC_WEBSOCKET_PROTOCOL, protocols);
                next.run(request).await
            }
        },
    ))
}

impl Harness {
    /// The real router, middleware and all — what `tower::ServiceExt::oneshot`
    /// drives.
    pub(crate) fn router(&self) -> axum::Router {
        authenticated_router(
            crate::build_router(Arc::clone(&self.state), None, self.port),
            self.device_secret.clone(),
        )
    }

    /// The real router without the harness's synthetic device secret.
    pub(crate) fn unauthenticated_router(&self) -> axum::Router {
        crate::build_router(Arc::clone(&self.state), None, self.port)
    }

    /// Serve the real router on a loopback port, for the tests that need a
    /// genuine byte stream (every WebSocket one).
    ///
    /// Takes `&mut self` so the server's guard is RETAINED by the harness:
    /// the returned address is only useful while the harness is alive, and
    /// tying the two together is what stops a test from leaking a listener
    /// and its actors past its own end.
    pub(crate) async fn serve(&mut self) -> std::net::SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let app = authenticated_router(
            crate::build_router(Arc::clone(&self.state), None, addr.port()),
            self.device_secret.clone(),
        );
        self.served.push(ServerGuard(tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        })));
        addr
    }

    /// Serve without supplying a device secret, for tests of pre-upgrade
    /// WebSocket refusal and the public exchange route.
    pub(crate) async fn serve_unauthenticated(&mut self) -> std::net::SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let app = crate::build_router(Arc::clone(&self.state), None, addr.port());
        self.served.push(ServerGuard(tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        })));
        addr
    }

    /// Wait until `host` is connected with a completed refresh behind it —
    /// the state every list and routing assertion is made against, since a
    /// host whose first refresh has not landed has nothing in the cache to
    /// route by.
    pub(crate) async fn await_refreshed(&self, host: HostId) {
        self.await_state(host, |state| {
            matches!(
                state,
                HostState::Connected {
                    last_refresh: RefreshHealth::Ok { .. },
                    ..
                }
            )
        })
        .await;
    }

    /// Stand a NEW helm up over the same helm.db, after letting `rescript`
    /// change what the far side does.
    ///
    /// A real restart, not a reset: the store is reopened from the same
    /// file, and the manager, its actors, and the router are all built from
    /// scratch — which is what makes an assertion about what survived
    /// actually about the DATABASE rather than about in-memory state that
    /// happened to stay put. The old harness is consumed, so nothing can
    /// accidentally keep serving from it.
    ///
    /// The scripted fleet is shared between the two, deliberately: the
    /// "hosts" are the far side of the network and do not restart just
    /// because the helm did.
    pub(crate) async fn restart_with(self, rescript: impl FnOnce(&ScriptedFleet)) -> Harness {
        rescript(&self.fleet);
        let Harness {
            served,
            store,
            manager,
            fleet,
            state,
            device_secret,
            port,
            _dir,
        } = self;
        // Everything the old helm owned goes first, in order: its servers,
        // its router state, then its actors. Only the temp directory (and
        // therefore helm.db) survives into the new one.
        drop(served);
        drop(state);
        manager.shutdown();
        drop(manager);
        drop(store);

        let store = HelmStore::open(&_dir.path().join("helm.db"))
            .await
            .expect("reopen helm.db");
        let manager = ConnectionManager::start(
            store.clone(),
            Arc::clone(&fleet) as Arc<dyn HostTransport>,
            test_cadence(),
        )
        .await
        .expect("start the restarted manager");
        Harness {
            served: Vec::new(),
            store: store.clone(),
            state: Arc::new(AppState::new(Arc::clone(&manager), store)),
            device_secret,
            manager,
            fleet,
            port,
            _dir,
        }
    }

    /// Wait until `host` is connected UNDER `identity` with a refresh of
    /// exactly `sessions` entries behind it.
    ///
    /// The precise version of [`Self::await_refreshed`], and the one a test
    /// that re-scripts a host must use. A host registered through
    /// `POST /api/hosts` connects immediately under whatever script it had
    /// at the time — by default no identity and no sessions — and that
    /// refresh already satisfies "connected and refreshed". A test that
    /// then edits the script, retries, and waits for the loose condition can
    /// therefore observe the INITIAL refresh and assert against an empty
    /// list (observed failing a full-crate run with `total == 0`). Naming
    /// the identity and the count makes the wait mean the state the test
    /// actually set up.
    pub(crate) async fn await_refreshed_as(&self, host: HostId, identity: &str, sessions: usize) {
        self.await_state(host, |state| {
            matches!(
                state,
                HostState::Connected {
                    identity: Some(seen),
                    last_refresh: RefreshHealth::Ok { sessions: count },
                    ..
                } if seen == identity && *count == sessions
            )
        })
        .await;
    }

    /// Drive one refresh of `host` all the way to COMPLETION — its cache
    /// write committed and its status published — and return.
    ///
    /// The barrier a NEGATIVE assertion needs, and the reason it cannot be a
    /// timeout. "A no-op refresh publishes nothing" is only provable against
    /// a refresh that has actually finished: synchronizing on the list
    /// REQUEST proves the drain started, and a test that then waits a couple
    /// of hundred milliseconds for silence passes just as happily when the
    /// refresh was slow and its bump lands afterwards — the exact false pass
    /// a negative test exists to rule out.
    ///
    /// The happens-before is the actor's own loop, not a sleep: one actor
    /// serves one host sequentially, and it does not ask for another session
    /// list until the previous refresh has committed and published. So the
    /// SECOND request arriving is proof that the FIRST refresh is done. That
    /// is why this drives two.
    ///
    /// The second refresh is still in flight when this returns, deliberately
    /// — waiting for a third would only move the same one-refresh tail — so
    /// a caller asserting silence is asserting it about a completed no-op
    /// plus an identical in-flight one, neither of which may publish
    /// anything. A caller expecting a CHANGE should wait for the change
    /// instead; this is the barrier for expecting nothing.
    pub(crate) async fn refresh_to_completion(&self, host: HostId) {
        let before = self.fleet.host_requests(host);
        self.manager.refresh_now(host);
        self.fleet.await_host_requests(host, before + 1).await;
        self.manager.refresh_now(host);
        self.fleet.await_host_requests(host, before + 2).await;
    }

    /// Wait until `host`'s state satisfies `predicate`, failing the test
    /// with the last seen state rather than hanging.
    pub(crate) async fn await_state(
        &self,
        host: HostId,
        predicate: impl FnMut(&HostState) -> bool,
    ) -> HostState {
        tokio::time::timeout(SETTLE_TIMEOUT, self.manager.wait_for_state(host, predicate))
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "host {host} never reached the expected state; last seen {:?}",
                    self.manager.state(host)
                )
            })
            .expect("an actor is running for this host")
    }
}

/// Builds a [`Harness`]: register hosts and their scripts, then start.
///
/// Split from `Harness` because every row must exist BEFORE the manager
/// starts — an actor already dialing while a test is still writing rows
/// would race its own fixture — and a builder makes that ordering
/// impossible to get wrong.
pub(crate) struct FleetBuilder {
    dir: tempfile::TempDir,
    store: HelmStore,
    fleet: Arc<ScriptedFleet>,
    port: u16,
    /// Overrides [`test_cadence`]'s hour-long refresh interval for the one
    /// kind of test that needs refreshes to actually recur — the ones about
    /// a refresh RACING something else.
    refresh: Option<Duration>,
    /// Overrides `/api/events`'s subscriber bound, so a test can exhaust it
    /// with a handful of real sockets instead of sixty-four.
    event_subscriber_cap: Option<usize>,
}

impl FleetBuilder {
    /// A fresh helm.db with only its reserved local row, and a local host
    /// scripted UNREACHABLE.
    ///
    /// Unreachable by default because the local row exists in every
    /// registry and therefore gets an actor in every test, while most tests
    /// are about something else — a local host that connected would
    /// interleave its own hello and refresh into whatever is being watched,
    /// and would put its own (empty) list into the merged view. A test that
    /// is about the local row scripts it explicitly.
    pub(crate) async fn new() -> FleetBuilder {
        let dir = tempfile::tempdir().expect("temp state dir");
        let store = HelmStore::open(&dir.path().join("helm.db"))
            .await
            .expect("open helm.db");
        let fleet = ScriptedFleet::new();
        let local = local_id(&store).await;
        fleet.scripts.lock().expect("script mutex").insert(
            local,
            HostScript {
                reachable: false,
                ..HostScript::default()
            },
        );
        FleetBuilder {
            dir,
            store,
            fleet,
            // The default port the pre-M6 tests' `Host: 127.0.0.1:7433`
            // headers were written against; the origin guard compares
            // against it, so it is part of what those tests exercise.
            port: 7433,
            refresh: None,
            event_subscriber_cap: None,
        }
    }

    /// Admit at most `cap` event subscriptions, so the endpoint's refusal is
    /// reachable through real sockets.
    ///
    /// The bound this replaces is sized for a browser population rather than
    /// for a test (`crate::events::MAX_SUBSCRIBERS`), and reaching it by
    /// opening that many connections would say more about the test's patience
    /// than about the endpoint.
    pub(crate) fn event_subscriber_cap(mut self, cap: usize) -> FleetBuilder {
        self.event_subscriber_cap = Some(cap);
        self
    }

    /// Refresh this fleet on `interval` instead of once an hour.
    ///
    /// Only for tests whose subject IS the refresh loop's interleaving with
    /// something else; every other test wants exactly one refresh per
    /// connection, so that what it asserts against is the list its script
    /// produced rather than whichever pass landed last.
    pub(crate) fn refresh_every(mut self, interval: Duration) -> FleetBuilder {
        self.refresh = Some(interval);
        self
    }

    /// Script the reserved local row.
    pub(crate) async fn local(self, script: HostScript) -> FleetBuilder {
        let id = local_id(&self.store).await;
        self.script(id, script);
        self
    }

    /// Register an ssh host at `destination` and script it. Returns the
    /// builder and the new host's id.
    pub(crate) async fn ssh(self, destination: &str, script: HostScript) -> (FleetBuilder, HostId) {
        let id = self
            .store
            .add_ssh_host(destination, None, None)
            .await
            .expect("register ssh host");
        self.script(id, script);
        (self, id)
    }

    fn script(&self, host: HostId, script: HostScript) {
        self.fleet
            .gate
            .lists
            .lock()
            .expect("list mutex")
            .insert(host, Arc::new(script.sessions.clone()));
        self.fleet
            .scripts
            .lock()
            .expect("script mutex")
            .insert(host, script);
    }

    /// Start the manager and assemble the shared state the router reads.
    pub(crate) async fn start(self) -> Harness {
        let cadence = Cadence {
            refresh: self.refresh.unwrap_or(test_cadence().refresh),
            ..test_cadence()
        };
        let manager = ConnectionManager::start(
            self.store.clone(),
            Arc::clone(&self.fleet) as Arc<dyn HostTransport>,
            cadence,
        )
        .await
        .expect("start manager");
        let state = Arc::new(AppState {
            // The only override any fixture makes, and it exists so the
            // feed's own bound can be exhausted through real sockets — see
            // `AppState::event_subscriber_cap`.
            event_subscriber_cap: self
                .event_subscriber_cap
                .unwrap_or(crate::events::MAX_SUBSCRIBERS),
            ..AppState::new(Arc::clone(&manager), self.store)
        });
        let device_secret = state
            .auth
            .mint_device()
            .await
            .expect("mint the harness device session");
        Harness {
            served: Vec::new(),
            store: state.store.clone(),
            state,
            device_secret,
            manager,
            fleet: self.fleet,
            port: self.port,
            _dir: self.dir,
        }
    }
}

/// The reserved local row's id, which `HelmStore::open` mints first.
pub(crate) async fn local_id(store: &HelmStore) -> HostId {
    store.list_hosts().await.expect("list hosts")[0].id
}

/// The single-host arrangement almost every pre-M6 REST test wants: the
/// local host connected, its connection spliced to the test's own scripted
/// peer, and one session (`sess-1`) in its list.
///
/// `sess-1` is what those tests' URLs already name, so seeding it is what
/// makes owner-lookup routing find them a host without every test having to
/// describe a fleet. `sess-missing` and friends are deliberately NOT
/// seeded: a 404 from the merged view before any host is contacted is the
/// contract for an id nobody owns.
///
/// Returns once the host is connected and its first refresh has landed, so
/// a caller can issue requests immediately.
pub(crate) async fn spliced_helm(peer: DuplexStream) -> Harness {
    spliced_helm_listing(peer, vec![session("sess-1", 1_700_000_000)]).await
}

/// A helm whose one connected host answers its handshake and its list and
/// nothing else — for the tests whose requests never reach a handler at
/// all.
///
/// The CORS preflight is answered by the route itself and a refused origin
/// never gets past the middleware, so scripting a supervisor exchange for
/// either would be scenery. What is still needed is a REAL fleet behind the
/// router, because the routes those tests name are session routes and a
/// router with no hosts would answer differently for the wrong reason.
pub(crate) async fn idle_helm() -> Harness {
    helm_listing(vec![session("sess-1", 1_700_000_000)]).await
}

/// [`idle_helm`] with an explicit session list — for the tests where the
/// list IS the fixture and no supervisor exchange is scripted at all.
pub(crate) async fn helm_listing(sessions: Vec<SessionInfo>) -> Harness {
    let harness = FleetBuilder::new()
        .await
        .local(HostScript {
            identity: Some("local-identity".to_string()),
            sessions,
            ..HostScript::default()
        })
        .await
        .start()
        .await;
    let local = local_id(&harness.store).await;
    harness.await_refreshed(local).await;
    harness
}

/// [`spliced_helm`] with an explicit session list — for the tests that
/// assert on what the LIST route reports, where the list is the fixture.
pub(crate) async fn spliced_helm_listing(
    peer: DuplexStream,
    sessions: Vec<SessionInfo>,
) -> Harness {
    let harness = FleetBuilder::new()
        .await
        .local(HostScript {
            identity: Some("local-identity".to_string()),
            sessions,
            peer: Some(peer),
            ..HostScript::default()
        })
        .await
        .start()
        .await;
    let local = local_id(&harness.store).await;
    harness.await_refreshed(local).await;
    harness
}

// The two peer/client doubles below are here for the same reason the fleet
// is: more than one test module needs them. `silent_supervisor` is the far
// end of every "the helm refused this by itself" assertion, and
// `WsTestClient` is the near end of every terminal-socket test — and the
// session-routing tests reach for both.

/// A scripted supervisor that must be asked NOTHING.
///
/// The far side of every "the helm refused this by itself" assertion:
/// it completes the handshake, so its host really is connected and
/// routable, and then fails if any request arrives at all. Bounded
/// silence rather than an EOF, because the harness holds the connection
/// open for the whole test (`rest_harness`'s module docs) — so
/// "nothing was forwarded" is only observable as nothing arriving. The
/// window is generous on purpose: a leak would send its frame
/// immediately, so a longer wait only costs time on the failing path.
pub(crate) async fn silent_supervisor(peer_side: tokio::io::DuplexStream) {
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};

    let (r, w) = tokio::io::split(peer_side);
    let mut reader = FrameReader::new(r);
    let mut writer = FrameWriter::new(w);
    handshake(&mut reader, &mut writer, "supervisor")
        .await
        .unwrap();
    // Three outcomes, and only one of them is a failure. A timeout is
    // the ordinary pass (nothing was sent). A clean EOF is also a pass:
    // the harness tears connections down deliberately — a host taken
    // down, a retry, a reconnect — and a closed stream is not a request.
    // Only a FRAME means something reached a supervisor that must not
    // have been asked anything.
    let leaked = tokio::time::timeout(Duration::from_secs(2), reader.read_frame()).await;
    assert!(
        !matches!(leaked, Ok(Ok(Some(_)))),
        "no request may reach this supervisor, but one arrived: {leaked:?}"
    );
}

/// A deliberately minimal WebSocket client: enough to complete the
/// upgrade, send text/binary frames, and — crucially — to STOP
/// READING whenever a test wants to model a wedged browser.
///
/// Hand-rolled rather than pulled from a crate because no WebSocket
/// client is a dependency of this workspace, and the two behaviors
/// these tests need are precisely the ones a real client library goes
/// out of its way to hide: never draining the socket, and observing
/// the server's close. Only the subset actually used is implemented —
/// client frames are always masked and always short, which is all a
/// pause/resume message or a keystroke ever is.
/// The mask every client frame this suite sends is XORed with.
///
/// Fixed rather than random: masking is a proxy-cache defence, and a server
/// that unmasks correctly does so for any key. A constant one keeps a failing
/// frame reproducible byte for byte.
const MASK: [u8; 4] = [0x37, 0xfa, 0x21, 0x3d];

/// One client frame's header, in whichever of the protocol's three length
/// forms `declared` needs, with the mask bit set.
///
/// Split from the payload so a test can send a header whose declared length
/// is a LIE about what follows — see [`WsTestClient::send_frame_header`],
/// which is the only way to observe a frame-size bound rejecting on the
/// declaration rather than on bytes it has already buffered.
fn frame_header(fin: bool, opcode: u8, declared: usize) -> Vec<u8> {
    let mut header = vec![if fin { 0x80 | opcode } else { opcode }];
    match declared {
        len if len < 126 => header.push(0x80 | len as u8),
        len if len <= u16::MAX as usize => {
            header.push(0x80 | 126);
            header.extend_from_slice(&(len as u16).to_be_bytes());
        }
        len => {
            header.push(0x80 | 127);
            header.extend_from_slice(&(len as u64).to_be_bytes());
        }
    }
    header.extend_from_slice(&MASK);
    header
}

pub(crate) struct WsTestClient {
    stream: tokio::net::TcpStream,
    /// Bytes read from the socket but not yet parsed into a frame.
    buffered: Vec<u8>,
}

impl WsTestClient {
    pub(crate) async fn connect(addr: std::net::SocketAddr, path: &str) -> WsTestClient {
        WsTestClient::try_connect(addr, path)
            .await
            .unwrap_or_else(|status| panic!("WebSocket upgrade refused with HTTP {status}"))
    }

    /// Attempt the upgrade, returning the HTTP status line's code instead of
    /// panicking when the server refuses.
    ///
    /// Exists for the tests whose subject IS the refusal — a bound reached
    /// BEFORE the upgrade answers an ordinary HTTP status, and the difference
    /// between that and a socket that opens and immediately closes is the
    /// whole point of admitting early.
    pub(crate) async fn try_connect(
        addr: std::net::SocketAddr,
        path: &str,
    ) -> Result<WsTestClient, u16> {
        let mut stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
        // A fixed key is fine: nothing here verifies the accept hash,
        // which exists to defend against caching proxies, not tests.
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: Upgrade\r\n\
             Upgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
             Sec-WebSocket-Key: AAAAAAAAAAAAAAAAAAAAAA==\r\n\r\n",
            addr.port()
        );
        stream
            .write_all(request.as_bytes())
            .await
            .expect("send upgrade");
        // Read exactly up to the end of the response headers, leaving
        // any frame bytes that followed them in the buffer.
        let mut buffered = Vec::new();
        let mut byte = [0u8; 1];
        while !buffered.ends_with(b"\r\n\r\n") {
            let n = stream.read(&mut byte).await.expect("read upgrade response");
            assert!(n > 0, "connection closed during the WebSocket upgrade");
            buffered.push(byte[0]);
        }
        let response = String::from_utf8_lossy(&buffered).into_owned();
        let code: u16 = response
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .unwrap_or_else(|| panic!("unreadable HTTP status line: {response}"));
        if code != 101 {
            return Err(code);
        }
        Ok(WsTestClient {
            stream,
            buffered: Vec::new(),
        })
    }

    /// Send one masked client frame. `opcode` is 1 for text, 2 for
    /// binary — the only two this suite sends.
    pub(crate) async fn send(&mut self, opcode: u8, payload: &[u8]) {
        assert!(
            payload.len() < 126,
            "test frames stay in the short-length form"
        );
        self.send_frame(true, opcode, payload).await;
    }

    /// Send one masked client frame in whatever length form its payload
    /// needs, with `fin` under the caller's control.
    ///
    /// The general form [`Self::send`] is the short case of, and it exists
    /// for exactly one kind of test: the ones about a server's INBOUND
    /// bounds. Those need what a well-behaved client library goes out of its
    /// way not to produce — a single frame far larger than any limit, and a
    /// run of individually-legal fragments that assemble into an oversized
    /// message. `fin: false` is what makes the second of those constructible
    /// at all (a continuation frame carries opcode 0).
    pub(crate) async fn send_frame(&mut self, fin: bool, opcode: u8, payload: &[u8]) {
        let mask = MASK;
        let mut frame = frame_header(fin, opcode, payload.len());
        frame.extend(
            payload
                .iter()
                .enumerate()
                .map(|(i, byte)| byte ^ mask[i % 4]),
        );
        // Ignored rather than unwrapped: a server that refuses an oversized
        // frame may close before this whole write lands, and that close IS
        // the behaviour under test.
        let _ = self.stream.write_all(&frame).await;
    }

    /// Send only a frame's HEADER, declaring `declared` payload bytes that
    /// never arrive.
    ///
    /// The one shape that can tell a server's FRAME bound from its MESSAGE
    /// bound. A server enforcing the frame bound refuses on the declared
    /// length alone, before a single payload byte exists; one enforcing only
    /// the message bound has nothing to measure yet and waits — which is
    /// exactly the buffering the frame bound exists to prevent, and is
    /// indistinguishable from correct behaviour if the test sends the body
    /// too.
    ///
    /// No real client library will ever emit this, which is the point: it is
    /// what a hostile or broken peer emits.
    pub(crate) async fn send_frame_header(&mut self, opcode: u8, declared: usize) {
        let header = frame_header(true, opcode, declared);
        let _ = self.stream.write_all(&header).await;
    }

    pub(crate) async fn send_text(&mut self, text: &str) {
        self.send(1, text.as_bytes()).await;
    }

    /// Read one server frame's (opcode, payload), or `None` once the
    /// server closes. Server frames are never masked.
    pub(crate) async fn recv(&mut self) -> Option<(u8, Vec<u8>)> {
        loop {
            if let Some(frame) = self.take_buffered_frame() {
                return Some(frame);
            }
            let mut chunk = [0u8; 8192];
            let n = self.stream.read(&mut chunk).await.ok()?;
            if n == 0 {
                return None;
            }
            self.buffered.extend_from_slice(&chunk[..n]);
        }
    }

    /// Decode one complete frame out of `buffered`, if there is one.
    /// Handles the two length forms axum actually emits for the sizes
    /// these tests produce.
    fn take_buffered_frame(&mut self) -> Option<(u8, Vec<u8>)> {
        if self.buffered.len() < 2 {
            return None;
        }
        let opcode = self.buffered[0] & 0x0f;
        let short = (self.buffered[1] & 0x7f) as usize;
        let (len, header) = match short {
            126 => {
                if self.buffered.len() < 4 {
                    return None;
                }
                (
                    u16::from_be_bytes([self.buffered[2], self.buffered[3]]) as usize,
                    4,
                )
            }
            127 => {
                if self.buffered.len() < 10 {
                    return None;
                }
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&self.buffered[2..10]);
                (u64::from_be_bytes(bytes) as usize, 10)
            }
            other => (other, 2),
        };
        if self.buffered.len() < header + len {
            return None;
        }
        let payload = self.buffered[header..header + len].to_vec();
        self.buffered.drain(..header + len);
        Some((opcode, payload))
    }
}

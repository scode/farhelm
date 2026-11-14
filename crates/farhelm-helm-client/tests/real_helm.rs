//! The client against a real helm, run in process.
//!
//! Each test starts its own helm the way the desktop app does (`run_embedded` on a thread, port 0, a fresh state
//! directory) and talks to it through the client's real WebSocket code. These are the tests that catch the
//! client and the helm disagreeing about the wire, which the channel's scripted unit tests cannot.
//!
//! The helm has no hosts here, so nothing reaches a supervisor; the collections are whatever an empty helm serves.
//! Every wait is bounded by [`STEP`] so a regression fails rather than hangs. The one sleep is an observation window
//! for something that must not happen (an automatic retry), and says so.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "an integration test: its helpers panic to fail the test, which `allow-*-in-tests` covers only inside `#[test]` functions"
)]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use farhelm_helm_client::collections::Collections;
use farhelm_helm_client::{
    ApiPath, ChannelConfig, ChannelEvent, ChannelEvents, ChannelState, CommandFuture,
    HelmConnection, HelmEndpoint, Method, PushedState,
};
use farhelm_proto::ui_channel::{Collection, HostView, Preferences};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// The most any one step may take. Generous because the machine is shared with other agents' builds and tests.
const STEP: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------------------
// A real helm
// ---------------------------------------------------------------------------

/// An embedded helm on its own thread, stopped and joined on drop.
struct Helm {
    addr: SocketAddr,
    secret: String,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<anyhow::Result<()>>>,
    _state: tempfile::TempDir,
}

impl Helm {
    fn start() -> Helm {
        let state = tempfile::tempdir().unwrap();
        let args = farhelm_helm::HelmArgs {
            port: 0,
            state_dir: Some(state.path().join("state")),
            ensure_hosts: None,
            payload_dir: None,
            release_base_url: None,
            backstop_refresh_secs: None,
            test_credential_file: None,
        };
        let (ready_tx, ready) = std::sync::mpsc::channel();
        let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            farhelm_helm::run_embedded(args, None, None, ready_tx, shutdown_rx)
        });
        let ready = ready
            .recv_timeout(STEP)
            .expect("the embedded helm became ready");
        Helm {
            addr: ready.addr,
            secret: ready.native_device_secret,
            shutdown: Some(shutdown),
            thread: Some(thread),
            _state: state,
        }
    }

    fn endpoint(&self) -> HelmEndpoint {
        HelmEndpoint::new(self.addr, self.secret.as_str())
    }
}

impl Drop for Helm {
    fn drop(&mut self) {
        let _ = self.shutdown.take().map(|shutdown| shutdown.send(()));
        if let Some(thread) = self.thread.take() {
            let result = thread.join();
            if !std::thread::panicking() {
                result
                    .expect("the helm thread did not panic")
                    .expect("the helm shut down cleanly");
            }
        }
    }
}

/// Run a test body on a throwaway executor. The client's own runtime is separate, which is also how the GUI uses it:
/// nothing the client hands out may depend on being polled from its runtime.
fn block_on<T>(body: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(body)
}

// ---------------------------------------------------------------------------
// The GUI side
// ---------------------------------------------------------------------------

/// What a GUI does with the channel: apply every event to its collections, in order.
struct Gui {
    events: ChannelEvents,
    collections: Collections,
    states: Vec<ChannelState>,
}

impl Gui {
    fn new(events: ChannelEvents) -> Gui {
        Gui {
            events,
            collections: Collections::default(),
            states: Vec::new(),
        }
    }

    async fn apply_next(&mut self) -> ChannelEvent {
        let event = tokio::time::timeout(STEP, self.events.recv())
            .await
            .expect("an event arrived in time")
            .expect("the channel is alive");
        match &event {
            ChannelEvent::State(state) => self.states.push(state.clone()),
            ChannelEvent::Snapshot(snapshot) => self.collections.install(snapshot),
            ChannelEvent::Changes(changes) => self.collections.apply(changes),
        }
        event
    }

    async fn until_state(&mut self, wanted: ChannelState) {
        while self.apply_next().await != ChannelEvent::State(wanted.clone()) {}
    }

    /// The next snapshot, not yet installed, so a test can compare it with what the incremental apply holds.
    async fn next_snapshot(&mut self) -> Vec<PushedState> {
        loop {
            let event = tokio::time::timeout(STEP, self.events.recv())
                .await
                .unwrap()
                .unwrap();
            match event {
                ChannelEvent::Snapshot(snapshot) => return snapshot,
                ChannelEvent::State(state) => self.states.push(state),
                ChannelEvent::Changes(changes) => self.collections.apply(&changes),
            }
        }
    }

    /// Await a command while applying events, as the GUI's event loop would.
    async fn run(&mut self, mut command: CommandFuture) -> farhelm_proto::ui_channel::Reply {
        tokio::time::timeout(STEP, async {
            loop {
                tokio::select! {
                    reply = &mut command => return reply.expect("the command was answered"),
                    _ = self.apply_next() => {}
                }
            }
        })
        .await
        .expect("the command was answered in time")
    }

    fn preferences(&self) -> Preferences {
        self.collections
            .get::<Preferences>("preferences")
            .expect("the preferences entry is held")
            .clone()
    }
}

fn open(endpoint: HelmEndpoint) -> (HelmConnection, Gui) {
    let (connection, events) = HelmConnection::open(endpoint, ChannelConfig::default()).unwrap();
    (connection, Gui::new(events))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The GUI's whole model of the helm starts from the snapshot and is kept current by changes the helm pushes before
/// each reply. This pins both against the real helm: all eight pushed collections arrive on open, whatever entries a
/// fresh helm holds in them decode as the client's types (a snapshot entry that does not decode breaks the channel
/// before `Connected`), and a command's effect is already in the GUI's collections when its reply resolves, with no
/// refetch. A fresh helm has no templates, notices or provisioning runs, so their entry shapes are pinned by the
/// helm's `every_pushed_collection_round_trips_through_its_proto_type`, not here.
#[test]
fn a_command_s_changes_are_applied_before_its_reply_resolves() {
    let helm = Helm::start();
    let (connection, mut gui) = open(helm.endpoint());
    block_on(async {
        gui.until_state(ChannelState::Connected).await;
        assert_eq!(
            gui.states,
            [ChannelState::Connecting, ChannelState::Connected]
        );
        for collection in [
            Collection::Sessions,
            Collection::Hosts,
            Collection::Preferences,
            Collection::Approvals,
            Collection::Templates,
            Collection::Notices,
            Collection::Announcements,
            Collection::HostProgress,
        ] {
            assert!(
                gui.collections.meta(collection).is_some(),
                "{collection:?} was not in the snapshot"
            );
        }
        assert_ne!(
            gui.preferences().compact,
            Some(true),
            "the premise: compact starts off"
        );

        let path = ApiPath::api(&["preferences"]).unwrap();
        let put = connection.channel().command(
            Method::Put,
            path,
            Some(serde_json::json!({"compact": true})),
        );
        let reply = gui.run(put).await;
        assert!((200..300).contains(&reply.status), "{reply:?}");
        assert_eq!(gui.preferences().compact, Some(true));
    });
}

/// One GUI at a time: a second channel takes the seat and the first stops in `Displaced` instead of fighting back,
/// until its user takes the seat back, which displaces the other in turn.
#[test]
fn a_second_channel_displaces_the_first_until_it_takes_back() {
    let helm = Helm::start();
    let (first, mut first_gui) = open(helm.endpoint());
    block_on(async {
        first_gui.until_state(ChannelState::Connected).await;
        let (_second, mut second_gui) = open(helm.endpoint());
        second_gui.until_state(ChannelState::Connected).await;
        first_gui.until_state(ChannelState::Displaced).await;

        first.channel().take_back();
        first_gui.until_state(ChannelState::Connected).await;
        second_gui.until_state(ChannelState::Displaced).await;
    });
}

/// A connection the helm side drops (here, a proxy cutting it) comes back by itself, and the snapshot it comes back
/// with matches what the GUI's incremental apply held, so a reconnect never silently corrects or corrupts the list.
#[test]
fn a_dropped_connection_reconnects_with_a_consistent_snapshot() {
    let helm = Helm::start();
    block_on(async {
        let proxy = Proxy::start(helm.addr).await;
        let (connection, mut gui) = open(HelmEndpoint::new(proxy.addr, helm.secret.as_str()));
        gui.until_state(ChannelState::Connected).await;
        let path = ApiPath::api(&["preferences"]).unwrap();
        let put = connection.channel().command(
            Method::Put,
            path,
            Some(serde_json::json!({"compact": true})),
        );
        gui.run(put).await;

        proxy.cut();
        gui.until_state(ChannelState::Reconnecting).await;
        let snapshot = gui.next_snapshot().await;
        // The local host row is the one entry that changes by itself here: no supervisor runs, so the helm keeps
        // retrying that connection and its attempt count moves on during the outage, where no change can reach the
        // GUI. The fresh snapshot is legitimately newer there, so host rows are compared with their connection state
        // set aside; everything else must match exactly.
        let differences: Vec<String> = gui
            .collections
            .check_against(&snapshot)
            .into_iter()
            .filter(|difference| !difference.starts_with("Hosts: entry "))
            .collect();
        assert_eq!(differences, Vec::<String>::new());
        let mut fresh = Collections::default();
        fresh.install(&snapshot);
        let held_hosts = gui.collections.ordered::<HostView>();
        assert_eq!(held_hosts.len(), fresh.ordered::<HostView>().len());
        for held in held_hosts {
            let fresh_host = fresh
                // Hosts are keyed by their registry id on the wire.
                .get::<HostView>(&held.id.to_string())
                .expect("every held host is in the fresh snapshot");
            let mut held = held.clone();
            held.state = fresh_host.state.clone();
            assert_eq!(&held, fresh_host);
        }
        gui.collections.install(&snapshot);
        gui.until_state(ChannelState::Connected).await;
        assert!(
            proxy.connections() >= 2,
            "the reconnect went through the proxy"
        );
    });
}

/// A refused credential cannot fix itself, so the client waits for the user's retry instead of retrying on a timer.
/// Pinned against the real helm's 401, through a proxy that counts connections. The backoffs are shrunk to a
/// millisecond, so a client that retried on its own would make dozens of attempts inside the observation window.
#[test]
fn a_wrong_credential_is_not_retried_until_the_user_asks() {
    let helm = Helm::start();
    block_on(async {
        let proxy = Proxy::start(helm.addr).await;
        let config = ChannelConfig {
            reconnect_initial: Duration::from_millis(1),
            reconnect_max: Duration::from_millis(1),
            ..ChannelConfig::default()
        };
        let (connection, events) = HelmConnection::open(
            HelmEndpoint::new(proxy.addr, "AAAAAAAAAAAAAAAAAAAAAA"),
            config,
        )
        .unwrap();
        let mut gui = Gui::new(events);
        gui.until_state(ChannelState::CannotConnect { status: 401 })
            .await;
        assert_eq!(proxy.connections(), 1, "the premise: one refused attempt");

        // sleep-ok: observation window for the absence of automatic retries; at 1 ms backoffs a retrying client would make dozens of attempts in it
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(proxy.connections(), 1, "the client retried on its own");

        connection.channel().retry();
        gui.until_state(ChannelState::Reconnecting).await;
        gui.until_state(ChannelState::CannotConnect { status: 401 })
            .await;
        assert_eq!(proxy.connections(), 2);
    });
}

// ---------------------------------------------------------------------------
// A cuttable proxy
// ---------------------------------------------------------------------------

/// A TCP proxy in front of the helm that the test can cut, to drop connections from the helm's side of the client.
///
/// The helm's loopback guard requires `Host: 127.0.0.1:<its port>`, and the client names the proxy's port, so the
/// proxy rewrites the Host header of each connection's first request (the WebSocket upgrade) before relaying bytes.
struct Proxy {
    addr: SocketAddr,
    live: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
    accepted: Arc<Mutex<usize>>,
}

impl Proxy {
    async fn start(helm: SocketAddr) -> Proxy {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let live = Arc::new(Mutex::new(Vec::new()));
        let accepted = Arc::new(Mutex::new(0));
        let (live_for_task, accepted_for_task) = (live.clone(), accepted.clone());
        tokio::spawn(async move {
            while let Ok((client, _)) = listener.accept().await {
                *accepted_for_task.lock().unwrap() += 1;
                let relay = tokio::spawn(relay(client, helm, addr));
                live_for_task.lock().unwrap().push(relay.abort_handle());
            }
        });
        Proxy {
            addr,
            live,
            accepted,
        }
    }

    /// Drop every connection relayed so far. Aborting closes both sockets, so each end sees the connection go.
    fn cut(&self) {
        for relay in self.live.lock().unwrap().drain(..) {
            relay.abort();
        }
    }

    fn connections(&self) -> usize {
        *self.accepted.lock().unwrap()
    }
}

async fn relay(mut client: TcpStream, helm: SocketAddr, proxy: SocketAddr) {
    let Ok(mut upstream) = TcpStream::connect(helm).await else {
        return;
    };
    // Read the request head, which the client sends whole before waiting for an answer.
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match client.read(&mut byte).await {
            Ok(1) => head.push(byte[0]),
            _ => return,
        }
    }
    let head =
        String::from_utf8_lossy(&head).replace(&format!("Host: {proxy}"), &format!("Host: {helm}"));
    let head = head.replace(&format!("host: {proxy}"), &format!("host: {helm}"));
    if upstream.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
}

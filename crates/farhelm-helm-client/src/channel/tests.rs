//! The channel's state machine against a scripted connector, on tokio's paused clock.
//!
//! Every test runs with `start_paused`, so tokio jumps the clock to the next timer whenever every task is idle. That
//! makes timing assertions exact (the watchdog fires at precisely 30 s of virtual time) without a wall-clock read or
//! a real wait. Elapsed times are measured with `tokio::time::Instant` for the same reason.

use super::*;
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState,
};
use futures_util::FutureExt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// A connector whose attempts the test answers one by one, recording when each attempt started.
#[derive(Clone)]
struct Script {
    results: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<Result<Link, ConnectError>>>>,
    attempts: Arc<Mutex<Vec<Instant>>>,
}

impl Connector for Script {
    fn connect(&self) -> BoxFuture<'static, Result<Link, ConnectError>> {
        let script = self.clone();
        Box::pin(async move {
            script.attempts.lock().unwrap().push(Instant::now());
            match script.results.lock().await.recv().await {
                Some(result) => result,
                // An unanswered attempt hangs, which is how a test says "no more connections".
                None => std::future::pending().await,
            }
        })
    }
}

impl Script {
    fn attempts(&self) -> usize {
        self.attempts.lock().unwrap().len()
    }
}

/// The helm's end of a scripted link, served by a task that answers pings (unless told not to) and forwards
/// commands to the test. Dropping it hangs up: the task holds a sender of its own (for pongs), so the drop aborts the
/// task too, or the client would never see the link end.
struct Peer {
    to_client: mpsc::UnboundedSender<ServerMessage>,
    commands: mpsc::UnboundedReceiver<Command>,
    task: tokio::task::AbortHandle,
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Peer {
    fn send(&self, message: ServerMessage) {
        self.to_client.send(message).unwrap();
    }

    async fn command(&mut self) -> Command {
        self.commands.recv().await.expect("the link is open")
    }

    fn reply(&self, id: u64) {
        self.send(ServerMessage::Reply(Reply {
            id,
            status: 200,
            body: None,
        }));
    }
}

fn peer(answer_pings: bool) -> (Link, Peer) {
    let (tx, mut from_client) = mpsc::unbounded_channel();
    let (to_client, rx) = mpsc::unbounded_channel();
    let (commands_tx, commands) = mpsc::unbounded_channel();
    let pong = to_client.clone();
    let answer = Arc::new(AtomicBool::new(answer_pings));
    let task = tokio::spawn(async move {
        while let Some(message) = from_client.recv().await {
            match message {
                ClientMessage::Ping { n } if answer.load(Ordering::Relaxed) => {
                    let _ = pong.send(ServerMessage::Pong { n });
                }
                ClientMessage::Ping { .. } => {}
                ClientMessage::Command(command) => {
                    let _ = commands_tx.send(command);
                }
            }
        }
    });
    (
        Link { tx, rx },
        Peer {
            to_client,
            commands,
            task: task.abort_handle(),
        },
    )
}

struct Harness {
    script: Script,
    results: mpsc::UnboundedSender<Result<Link, ConnectError>>,
    handle: ChannelHandle,
    events: ChannelEvents,
}

fn start() -> Harness {
    let (results, results_rx) = mpsc::unbounded_channel();
    let script = Script {
        results: Arc::new(tokio::sync::Mutex::new(results_rx)),
        attempts: Arc::default(),
    };
    let (handle, events) = spawn_channel(Arc::new(script.clone()), ChannelConfig::default());
    Harness {
        script,
        results,
        handle,
        events,
    }
}

impl Harness {
    /// Answer the next attempt with a link whose peer has already sent its snapshot, and wait for `Connected`.
    async fn connect(&mut self, answer_pings: bool) -> Peer {
        let peer = self.open(answer_pings);
        peer.send(snapshot());
        self.until_state(ChannelState::Connected).await;
        peer
    }

    /// Answer the next attempt with a link that has sent nothing yet.
    fn open(&self, answer_pings: bool) -> Peer {
        let (link, peer) = peer(answer_pings);
        self.results.send(Ok(link)).unwrap();
        peer
    }

    /// Await a command while draining events, as the GUI must: completions fire only as the feed is read.
    async fn run(&mut self, mut command: CommandFuture) -> Result<Reply, CommandError> {
        loop {
            tokio::select! {
                result = &mut command => return result,
                event = self.events.recv() => {
                    event.expect("the channel is alive");
                }
            }
        }
    }

    async fn until_state(&mut self, wanted: ChannelState) {
        loop {
            match self.events.recv().await.expect("the channel is alive") {
                ChannelEvent::State(state) if state == wanted => return,
                _ => {}
            }
        }
    }
}

/// A snapshot holding one empty collection (approvals), which is the one [`changes`] targets.
fn snapshot() -> ServerMessage {
    ServerMessage::Snapshot {
        collections: vec![CollectionState {
            collection: Collection::Approvals,
            meta: CollectionMeta {
                availability: Availability::Available,
                order: vec![],
                counts: None,
            },
            entries: vec![],
        }],
    }
}

fn changes(order: &[&str]) -> ServerMessage {
    ServerMessage::Changes(CollectionChanges {
        collection: Collection::Approvals,
        meta: CollectionMeta {
            availability: Availability::Available,
            order: order.iter().map(|key| key.to_string()).collect(),
            counts: None,
        },
        upsert: vec![],
        remove: vec![],
    })
}

fn path() -> ApiPath {
    ApiPath::api(&["preferences"]).unwrap()
}

// ---------------------------------------------------------------------------
// Ordering
// ---------------------------------------------------------------------------

/// The helm pushes a command's effects before its reply so the GUI can read the new state when the reply lands. Two
/// separate channels (one for events, one for the reply) would lose that order, so this pins that a reply resolves
/// only once the event feed has handed out everything sent before it.
#[tokio::test(start_paused = true)]
async fn a_reply_resolves_only_after_the_changes_sent_before_it() {
    let mut harness = start();
    let mut peer = harness.connect(true).await;
    let mut reply = harness.handle.command(Method::Put, path(), None);
    let command = peer.command().await;
    assert_eq!(
        (command.method.as_str(), command.path.as_str()),
        ("PUT", "/api/preferences")
    );

    peer.send(changes(&["first"]));
    peer.reply(command.id);
    peer.send(changes(&["after"]));

    assert!(
        matches!(harness.events.recv().await, Some(ChannelEvent::Changes(c)) if c.meta.order == ["first"])
    );
    assert!(
        (&mut reply).now_or_never().is_none(),
        "the reply resolved before its changes were read"
    );
    assert!(
        matches!(harness.events.recv().await, Some(ChannelEvent::Changes(c)) if c.meta.order == ["after"])
    );
    assert_eq!(reply.now_or_never().unwrap().unwrap().status, 200);
}

// ---------------------------------------------------------------------------
// The watchdog
// ---------------------------------------------------------------------------

/// The helm runs commands one at a time, so a command queued behind a slow one has not started. Counting its time
/// from the send would break a healthy channel whenever two slow commands queue up; this pins that the clock starts
/// at admission (the previous reply), and that a break settles both kinds of pending command correctly.
#[tokio::test(start_paused = true)]
async fn the_watchdog_counts_a_queued_command_from_its_admission() {
    let mut harness = start();
    let mut peer = harness.connect(true).await;
    let started = Instant::now();
    let first = harness.handle.command(Method::Post, path(), None);
    let second = harness.handle.command(Method::Post, path(), None);
    let first_id = peer.command().await.id;
    peer.command().await;

    // sleep-ok: paused clock: the first reply arrives 20 s after admission, the stimulus under test
    tokio::time::sleep(Duration::from_secs(20)).await;
    peer.reply(first_id);
    harness.until_state(ChannelState::Reconnecting).await;

    assert_eq!(started.elapsed(), Duration::from_secs(50));
    // The state came first; the lost command's failure fires only as the feed is read past it.
    assert_eq!(harness.run(first).await.unwrap().id, first_id);
    assert_eq!(harness.run(second).await, Err(CommandError::Reset));
    // A connection that had been up comes back at once, not after a backoff.
    // sleep-ok: paused clock: lets the channel task make its immediate reconnect attempt
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!(harness.script.attempts(), 2);
}

/// Pings are what notice a dead helm while the GUI sends nothing, so an unanswered ping alone must break the
/// channel, one watchdog period after it was sent.
#[tokio::test(start_paused = true)]
async fn an_unanswered_ping_breaks_the_channel() {
    let mut harness = start();
    let _peer = harness.connect(false).await;
    let started = Instant::now();
    harness.until_state(ChannelState::Reconnecting).await;
    assert_eq!(started.elapsed(), Duration::from_secs(5 + 30));
}

/// A channel whose helm answers its pings stays up indefinitely; the heartbeat must not break a healthy idle channel.
#[tokio::test(start_paused = true)]
async fn an_idle_healthy_channel_stays_up() {
    let mut harness = start();
    let _peer = harness.connect(true).await;
    // sleep-ok: paused clock: an observation window long past every backoff and watchdog period
    tokio::time::sleep(Duration::from_secs(600)).await;
    assert_eq!(harness.events.try_recv(), None);
    assert_eq!(harness.script.attempts(), 1);
}

// ---------------------------------------------------------------------------
// Terminal states and reconnecting
// ---------------------------------------------------------------------------

/// Reconnecting after a displacement would take the seat back from the UI that just took it, so the two would steal
/// it from each other forever. Only the user's explicit take-back reconnects.
#[tokio::test(start_paused = true)]
async fn a_displaced_channel_reconnects_only_on_take_back() {
    let mut harness = start();
    let mut peer = harness.connect(true).await;
    let pending = harness.handle.command(Method::Post, path(), None);
    peer.command().await;
    peer.send(ServerMessage::Displaced);
    harness.until_state(ChannelState::Displaced).await;
    assert_eq!(harness.run(pending).await, Err(CommandError::Reset));

    harness.handle.retry();
    // sleep-ok: paused clock: an observation window long past every backoff and watchdog period
    tokio::time::sleep(Duration::from_secs(600)).await;
    assert_eq!(harness.script.attempts(), 1);
    let refused = harness.handle.command(Method::Get, path(), None);
    assert_eq!(harness.run(refused).await, Err(CommandError::Reset));

    harness.handle.take_back();
    harness.until_state(ChannelState::Reconnecting).await;
    harness.connect(true).await;
    assert_eq!(harness.script.attempts(), 2);
}

/// The permission to reconnect at once belongs to the connection that earned it by answering a heartbeat, not to the
/// channel. Before the shared reconnect policy, a channel-wide flag set by an earlier healthy connection survived a
/// displacement and take-back, so a later connection that dropped before ever answering a heartbeat reconnected at
/// once on stale evidence: the hot loop the policy exists to prevent. This pins that such a drop waits the initial
/// backoff.
#[tokio::test(start_paused = true)]
async fn a_take_back_connection_that_drops_unproven_waits_the_initial_backoff() {
    let mut harness = start();
    let first = harness.connect(true).await;
    // sleep-ok: paused clock: one ping interval, so the first connection answers a heartbeat (proves itself)
    tokio::time::sleep(Duration::from_secs(6)).await;
    first.send(ServerMessage::Displaced);
    harness.until_state(ChannelState::Displaced).await;
    drop(first);

    harness.handle.take_back();
    harness.until_state(ChannelState::Reconnecting).await;
    // The take-back connection delivers its snapshot and drops before its first heartbeat.
    let second = harness.connect(true).await;
    assert_eq!(
        harness.script.attempts(),
        2,
        "premise: the take-back made exactly one attempt"
    );
    let dropped_at = Instant::now();
    drop(second);
    harness.until_state(ChannelState::Reconnecting).await;
    let _third = harness.connect(true).await;
    let third_at = harness.script.attempts.lock().unwrap()[2];
    assert_eq!(
        third_at - dropped_at,
        ChannelConfig::default().reconnect_initial,
        "an unproven connection's drop waits the initial backoff, whatever an earlier connection proved"
    );
}

/// A refused credential cannot fix itself, so retrying on a timer would only hammer the helm; the channel waits for
/// the user's retry.
#[tokio::test(start_paused = true)]
async fn a_refused_credential_waits_for_retry() {
    let mut harness = start();
    harness
        .results
        .send(Err(ConnectError::Refused { status: 401 }))
        .unwrap();
    harness
        .until_state(ChannelState::CannotConnect { status: 401 })
        .await;
    // sleep-ok: paused clock: an observation window long past every backoff and watchdog period
    tokio::time::sleep(Duration::from_secs(600)).await;
    assert_eq!(harness.script.attempts(), 1);

    harness.handle.retry();
    harness.until_state(ChannelState::Reconnecting).await;
    harness.connect(true).await;
    assert_eq!(harness.script.attempts(), 2);
}

/// Failed attempts are serial and back off, doubling up to the cap, so a helm that is down is not hammered and two
/// attempts never race. Commands meanwhile fail at once rather than waiting for a connection, and the user's retry
/// cannot hurry an automatic reconnect (it exists only for a refused credential).
#[tokio::test(start_paused = true)]
async fn failed_attempts_back_off_one_at_a_time() {
    let mut harness = start();
    let started = Instant::now();
    for _ in 0..6 {
        harness
            .results
            .send(Err(ConnectError::Failed("refused".into())))
            .unwrap();
    }
    let refused = harness.handle.command(Method::Get, path(), None);
    assert_eq!(harness.run(refused).await, Err(CommandError::Reset));
    harness.handle.retry();
    let _peer = {
        // sleep-ok: paused clock: lets the scripted failures and their backoffs play out
        tokio::time::sleep(Duration::from_secs(30)).await;
        harness.connect(true).await
    };
    let offsets: Vec<u64> = harness
        .script
        .attempts
        .lock()
        .unwrap()
        .iter()
        .map(|at| (*at - started).as_secs())
        .collect();
    assert_eq!(offsets, [0, 1, 3, 7, 15, 23, 31]);
}

/// The embedder's report that the helm exited is final: no reconnect can reach a helm that is gone, and commands
/// fail at once from then on.
#[tokio::test(start_paused = true)]
async fn helm_ended_is_final() {
    let mut harness = start();
    let _peer = harness.connect(true).await;
    harness.handle.helm_ended();
    harness.until_state(ChannelState::HelmEnded).await;
    let refused = harness.handle.command(Method::Get, path(), None);
    assert_eq!(harness.run(refused).await, Err(CommandError::Reset));
    // sleep-ok: paused clock: an observation window long past every backoff and watchdog period
    tokio::time::sleep(Duration::from_secs(600)).await;
    assert_eq!(harness.script.attempts(), 1);
}

/// A helm that drops every connection right after its snapshot must not get a hot loop of snapshots and resets. Only
/// a connection that proved itself (answered a heartbeat) earns an immediate reconnect; one that merely delivered a
/// snapshot falls back to the doubling backoff from its second break on.
#[tokio::test(start_paused = true)]
async fn a_helm_that_drops_each_connection_after_its_snapshot_gets_a_backoff() {
    let mut harness = start();
    let started = Instant::now();
    // The first connection proves itself, so its break reconnects at once.
    let first = harness.connect(true).await;
    // sleep-ok: paused clock: one ping interval, so the first connection answers a heartbeat before it drops
    tokio::time::sleep(Duration::from_secs(6)).await;
    drop(first);
    harness.until_state(ChannelState::Reconnecting).await;
    for _ in 0..4 {
        // Each later connection delivers its snapshot and drops before any heartbeat.
        let peer = harness.open(true);
        peer.send(snapshot());
        harness.until_state(ChannelState::Connected).await;
        drop(peer);
        harness.until_state(ChannelState::Reconnecting).await;
    }
    let _last = harness.connect(true).await;
    let offsets: Vec<u64> = harness
        .script
        .attempts
        .lock()
        .unwrap()
        .iter()
        .map(|at| (*at - started).as_secs())
        .collect();
    assert_eq!(offsets, [0, 6, 7, 9, 13, 21]);
}

// ---------------------------------------------------------------------------
// Protocol violations
// ---------------------------------------------------------------------------

/// The snapshot is what makes a connection usable, so a helm that opens the socket and never sends one is as broken
/// as one that does not answer: the watchdog covers the snapshot too.
#[tokio::test(start_paused = true)]
async fn a_missing_snapshot_trips_the_watchdog() {
    let mut harness = start();
    let started = Instant::now();
    let _peer = harness.open(true);
    harness.until_state(ChannelState::Reconnecting).await;
    assert_eq!(started.elapsed(), Duration::from_secs(30));
}

/// Replies are matched to commands by order, so a reply out of order means the two sides disagree about which command
/// it answers. Resolving the wrong future would be worse than any break, so the channel breaks, and at once: the
/// watchdog would also break it 30 s later, so only zero elapsed time shows the reply itself was the cause.
#[tokio::test(start_paused = true)]
async fn an_out_of_order_reply_breaks_the_channel() {
    let mut harness = start();
    let mut peer = harness.connect(true).await;
    let first = harness.handle.command(Method::Post, path(), None);
    let second = harness.handle.command(Method::Post, path(), None);
    peer.command().await;
    let second_id = peer.command().await.id;
    let started = Instant::now();
    peer.reply(second_id);
    harness.until_state(ChannelState::Reconnecting).await;
    assert_eq!(
        started.elapsed(),
        Duration::ZERO,
        "the out-of-order reply did not break the channel itself"
    );
    assert_eq!(harness.run(first).await, Err(CommandError::Reset));
    assert_eq!(harness.run(second).await, Err(CommandError::Reset));
}

/// The GUI's copy of pushed state is only right if every change lands on a collection a snapshot installed and every
/// entry decodes. Each of those failures is a disagreement about the protocol, and the one recovery for it is the
/// reset: break, reconnect, rebuild from a fresh snapshot. None of them may reach the GUI as an event. Each must break
/// the channel at once: a message dropped silently would still end in a break later (the snapshot watchdog, or a
/// command left unanswered), so the zero elapsed time is what shows the message itself was refused.
#[tokio::test(start_paused = true)]
async fn changes_the_gui_cannot_apply_break_the_channel() {
    let undecodable = ServerMessage::Changes(CollectionChanges {
        upsert: vec![farhelm_proto::ui_channel::KeyedEntry {
            key: "a".into(),
            value: serde_json::json!({"not": "an approval"}),
        }],
        ..match changes(&["a"]) {
            ServerMessage::Changes(c) => c,
            _ => unreachable!(),
        }
    });
    let not_installed = ServerMessage::Changes(CollectionChanges {
        collection: Collection::Hosts,
        ..match changes(&[]) {
            ServerMessage::Changes(c) => c,
            _ => unreachable!(),
        }
    });
    let mut harness = start();
    for (label, before_snapshot, message) in [
        ("changes before the snapshot", true, changes(&[])),
        (
            "changes to a collection the snapshot lacks",
            false,
            not_installed,
        ),
        ("an undecodable entry", false, undecodable),
    ] {
        let peer = harness.open(true);
        if !before_snapshot {
            peer.send(snapshot());
            harness.until_state(ChannelState::Connected).await;
        }
        let started = Instant::now();
        peer.send(message);
        loop {
            match harness.events.recv().await.expect("the channel is alive") {
                ChannelEvent::State(ChannelState::Reconnecting) => break,
                ChannelEvent::Changes(changes) => panic!("{label}: reached the GUI as {changes:?}"),
                _ => {}
            }
        }
        assert_eq!(
            started.elapsed(),
            Duration::ZERO,
            "{label}: the message did not break the channel itself"
        );
    }
}

// ---------------------------------------------------------------------------
// The reset
// ---------------------------------------------------------------------------

/// SPEC_impl.md, "One failure path: the reset": a completion queued across a reset must not reach the new UI
/// session. The GUI drops its UI-session owner when it reads the new state, so the channel must announce the state
/// before any lost command fails, and must fail them all before the next connection's snapshot can arrive. The command
/// also fails with the one opaque error, never with advice about resending.
#[tokio::test(start_paused = true)]
async fn a_completion_queued_across_a_reset_fires_after_the_state_and_before_the_new_snapshot() {
    let mut harness = start();
    let mut peer = harness.connect(true).await;
    let mut lost = harness.handle.command(Method::Post, path(), None);
    peer.command().await;
    drop(peer);

    assert_eq!(
        harness.events.recv().await,
        Some(ChannelEvent::State(ChannelState::Reconnecting))
    );
    assert!(
        (&mut lost).now_or_never().is_none(),
        "the lost command failed before the GUI read the new state"
    );
    let next = harness.open(true);
    // The next connection's snapshot is the next event; reading up to it fires the queued failure first.
    next.send(snapshot());
    assert!(matches!(
        harness.events.recv().await,
        Some(ChannelEvent::Snapshot(_))
    ));
    assert_eq!(lost.now_or_never(), Some(Err(CommandError::Reset)));
}

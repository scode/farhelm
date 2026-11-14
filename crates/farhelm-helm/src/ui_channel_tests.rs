//! Tests of the UI channel, in two layers.
//!
//! The ordering contract ([`run`]) is tested against a small router of test
//! routes, driven through plain channels: that is the only way to hold a
//! command at a chosen point and look at what the channel does meanwhile
//! without racing a real supervisor. The endpoint, the seat and the real
//! collections are then tested against a real helm over a real socket.

use super::*;
use crate::rest_harness::{self, WsTestClient};
use axum::extract::State as AxState;
use axum::routing::get;
use farhelm_proto::ui_channel::{Collection, KeyedEntry, SessionRow};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// How long a test waits for something that should be immediate. Generous,
/// because a shorter wait would not diagnose "nothing arrived" any better.
const NOTICE: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------
// The diff
// ---------------------------------------------------------------------------

fn entry(id: &str, title: &str) -> (String, Value) {
    (
        id.to_string(),
        serde_json::json!({ "id": id, "title": title }),
    )
}

/// The same entry as the wire carries it: under its key.
fn keyed(id: &str, title: &str) -> KeyedEntry {
    let (key, value) = entry(id, title);
    KeyedEntry { key, value }
}

fn projected(entries: Vec<(String, Value)>) -> Projected {
    Projected {
        entries,
        counts: None,
    }
}

fn empty_sent() -> Sent {
    Sent {
        meta: CollectionMeta {
            availability: Availability::Available,
            order: Vec::new(),
            counts: None,
        },
        entries: BTreeMap::new(),
    }
}

/// The generic diff is what every collection's pushes come from, so its
/// three outcomes are pinned directly: an added and a changed entry are sent
/// whole under the key the diff found them by (the one key the GUI files them
/// under), a vanished one is named for removal, and an identical read sends
/// nothing at all (a feed bump that changed nothing must not wake the GUI).
#[farhelm_testtrace::test]
fn the_diff_sends_added_and_replaced_entries_whole_and_names_removed_ones() {
    let (first, sent) = diff(
        Collection::Approvals,
        &empty_sent(),
        Ok(projected(vec![entry("a", "one"), entry("b", "two")])),
    );
    let first = first.expect("a first read is a change");
    assert_eq!(first.upsert.len(), 2);
    assert_eq!(first.meta.order, ["a", "b"]);

    let (second, sent) = diff(
        Collection::Approvals,
        &sent,
        Ok(projected(vec![
            entry("b", "two, renamed"),
            entry("c", "three"),
        ])),
    );
    let second = second.expect("a different read is a change");
    assert_eq!(
        second.upsert,
        vec![keyed("b", "two, renamed"), keyed("c", "three")]
    );
    assert_eq!(second.remove, ["a"]);
    assert_eq!(second.meta.order, ["b", "c"]);

    let (third, _) = diff(
        Collection::Approvals,
        &sent,
        Ok(projected(vec![
            entry("b", "two, renamed"),
            entry("c", "three"),
        ])),
    );
    assert_eq!(third, None, "an identical read sends nothing");
}

/// A failed read is never evidence that anything was deleted (SPEC_impl.md,
/// "What is pushed"). It changes only the availability; the entries, order
/// and counts the GUI holds stay. When reads recover, the diff is taken
/// against what the GUI still holds, so an entry that really did go during
/// the outage is removed then, and only then.
#[farhelm_testtrace::test]
fn a_failed_read_marks_the_collection_unavailable_without_removing_anything() {
    let (_, sent) = diff(
        Collection::Approvals,
        &empty_sent(),
        Ok(projected(vec![entry("a", "one"), entry("b", "two")])),
    );

    let (failed, during) = diff(Collection::Approvals, &sent, Err("boom".to_string()));
    let failed = failed.expect("losing availability is a change");
    assert!(failed.upsert.is_empty() && failed.remove.is_empty());
    assert_eq!(
        failed.meta.availability,
        Availability::Unavailable {
            error: "boom".into()
        }
    );
    assert_eq!(failed.meta.order, ["a", "b"], "the last order stays");
    assert_eq!(during.entries.len(), 2, "the held entries stay");

    let (recovered, _) = diff(
        Collection::Approvals,
        &during,
        Ok(projected(vec![entry("b", "two")])),
    );
    let recovered = recovered.expect("recovering is a change");
    assert_eq!(recovered.meta.availability, Availability::Available);
    assert_eq!(recovered.remove, ["a"]);
    assert!(
        recovered.upsert.is_empty(),
        "an unchanged entry is not resent"
    );
}

// ---------------------------------------------------------------------------
// The seat
// ---------------------------------------------------------------------------

/// One UI per helm: taking the seat tells the holder it was displaced, the
/// displaced channel's ending (dropping its receiver afterwards) must not free
/// the seat its successor now holds, and the successor's ending does.
#[farhelm_testtrace::test]
async fn taking_the_seat_displaces_the_holder_and_its_release_keeps_the_successor() {
    let seat = UiSeat::default();
    assert!(!seat.is_held());
    let mut first = seat.take();
    assert!(seat.is_held());
    assert!(first.try_recv().is_err(), "not displaced yet");

    let second = seat.take();
    first
        .try_recv()
        .expect("the first holder is told it was displaced");
    drop(first);
    assert!(
        seat.is_held(),
        "the displaced channel's ending keeps the successor"
    );
    drop(second);
    assert!(!seat.is_held(), "the holder's ending frees the seat");
}

// ---------------------------------------------------------------------------
// Ordering, against test routes
// ---------------------------------------------------------------------------

/// The state behind the test routes: a value a slow save writes and a read
/// returns, a list the pushed test collection reads, and switches to hold the
/// save or fail the list.
#[derive(Default)]
struct TestRoutes {
    value: Mutex<Option<String>>,
    items: Mutex<Vec<Value>>,
    save_gate: Mutex<Option<oneshot::Receiver<()>>>,
    fail_items: AtomicBool,
    /// Requests that matched no test route. The router's fallback counts them, so a test can show a command never
    /// reached the router at all, not merely that it missed every real route.
    unrouted_hits: AtomicUsize,
}

/// A router of test routes, standing in for the helm's: a save that waits on
/// a gate the test opens, a read of what it saved, an add to the pushed list,
/// and the pushed list itself.
fn test_router(routes: Arc<TestRoutes>) -> axum::Router {
    async fn save(AxState(routes): AxState<Arc<TestRoutes>>, body: String) -> StatusCode {
        let gate = routes.save_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            let _ = gate.await;
        }
        *routes.value.lock().unwrap() = Some(body);
        StatusCode::NO_CONTENT
    }
    async fn read(AxState(routes): AxState<Arc<TestRoutes>>) -> String {
        routes.value.lock().unwrap().clone().unwrap_or_default()
    }
    async fn add(AxState(routes): AxState<Arc<TestRoutes>>, body: String) -> StatusCode {
        routes
            .items
            .lock()
            .unwrap()
            .push(serde_json::json!({ "id": body }));
        StatusCode::CREATED
    }
    async fn items(AxState(routes): AxState<Arc<TestRoutes>>) -> axum::response::Response {
        if routes.fail_items.load(Ordering::SeqCst) {
            return (StatusCode::SERVICE_UNAVAILABLE, "items are down").into_response();
        }
        let items = routes.items.lock().unwrap().clone();
        axum::Json(serde_json::json!({ "items": items })).into_response()
    }
    async fn unrouted(AxState(routes): AxState<Arc<TestRoutes>>) -> StatusCode {
        routes.unrouted_hits.fetch_add(1, Ordering::SeqCst);
        StatusCode::NOT_FOUND
    }
    axum::Router::new()
        .route("/api/test/value", get(read).put(save))
        .route("/api/test/items", get(items).post(add))
        .fallback(unrouted)
        .with_state(routes)
}

/// The one collection the ordering tests push: the test routes' list.
fn test_collection() -> CollectionSpec {
    CollectionSpec {
        collection: Collection::Approvals,
        path: |_| "/api/test/items".to_string(),
        split: |body| split_keyed_array(body, "items", "id"),
    }
}

/// A running channel over the test routes, and its ends.
struct Driven {
    to_channel: mpsc::Sender<ClientMessage>,
    from_channel: mpsc::Receiver<ServerMessage>,
    displace: Option<oneshot::Sender<()>>,
    feed: watch::Sender<u64>,
    task: tokio::task::JoinHandle<()>,
}

/// Start [`run`] over the test routes, with its snapshot already read.
async fn drive(routes: Arc<TestRoutes>) -> (Driven, ServerMessage) {
    let channel = Channel::new(
        test_router(routes),
        HeaderValue::from_static("Bearer test"),
        vec![test_collection()],
    );
    let (to_channel, incoming) = mpsc::channel(16);
    let (outgoing, from_channel) = mpsc::channel(16);
    let (displace, displaced) = oneshot::channel();
    let (feed, feed_rx) = watch::channel(0);
    let task = tokio::spawn(run(channel, incoming, outgoing, displaced, feed_rx));
    let mut driven = Driven {
        to_channel,
        from_channel,
        displace: Some(displace),
        feed,
        task,
    };
    let snapshot = next(&mut driven).await;
    (driven, snapshot)
}

/// The channel's next message, which must arrive promptly.
async fn next(driven: &mut Driven) -> ServerMessage {
    tokio::time::timeout(NOTICE, driven.from_channel.recv())
        .await
        .expect("the channel should send promptly")
        .expect("the channel should still be open")
}

fn command(id: u64, method: &str, path: &str, body: Option<Value>) -> ClientMessage {
    ClientMessage::Command(Command {
        id,
        method: method.to_string(),
        // Through deserialization, the helm's only way in: there is no public string constructor (paths are a trust
        // boundary), and a test that needs a non-canonical path sends raw JSON instead.
        path: serde_json::from_value(Value::String(path.to_string()))
            .expect("a test command's path is canonical"),
        body,
    })
}

/// Serial admission is what makes the stream ordered (SPEC_impl.md, "The UI
/// channel"). A save held mid-flight, followed at once by a read that depends
/// on it, must not let the read run first: the read's reply shows the saved
/// value, and replies come in command order. The heartbeat sent while the
/// save is held is answered at once, so a slow command never makes a live
/// channel look dead to the GUI's watchdog.
#[farhelm_testtrace::test]
async fn a_command_waits_for_the_one_before_it_while_heartbeats_are_answered() {
    let routes = Arc::new(TestRoutes::default());
    let (release, gate) = oneshot::channel();
    *routes.save_gate.lock().unwrap() = Some(gate);
    let (mut driven, _snapshot) = drive(Arc::clone(&routes)).await;

    driven
        .to_channel
        .send(command(
            1,
            "PUT",
            "/api/test/value",
            Some(Value::String("saved".into())),
        ))
        .await
        .unwrap();
    driven
        .to_channel
        .send(command(2, "GET", "/api/test/value", None))
        .await
        .unwrap();
    driven
        .to_channel
        .send(ClientMessage::Ping { n: 7 })
        .await
        .unwrap();
    assert_eq!(
        next(&mut driven).await,
        ServerMessage::Pong { n: 7 },
        "the heartbeat is answered while the save is still held"
    );
    assert!(
        routes.value.lock().unwrap().is_none(),
        "premise: the save is still held when the heartbeat is answered"
    );

    release.send(()).unwrap();
    let ServerMessage::Reply(first) = next(&mut driven).await else {
        panic!("expected the save's reply first");
    };
    assert_eq!((first.id, first.status), (1, 204));
    let ServerMessage::Reply(second) = next(&mut driven).await else {
        panic!("expected the read's reply second");
    };
    assert_eq!(second.id, 2);
    assert_eq!(
        second.body,
        Some(serde_json::json!("saved")),
        "the read ran after the save it depends on"
    );
}

/// A GUI that sees "done" already shows the result (SPEC_impl.md): every
/// change a command causes is pushed before that command's reply, without
/// waiting for a feed bump.
#[farhelm_testtrace::test]
async fn the_changes_a_command_causes_arrive_before_its_reply() {
    let routes = Arc::new(TestRoutes::default());
    let (mut driven, snapshot) = drive(Arc::clone(&routes)).await;
    let ServerMessage::Snapshot { collections } = snapshot else {
        panic!("the first message is the snapshot");
    };
    assert!(
        collections[0].entries.is_empty(),
        "premise: nothing listed yet"
    );

    driven
        .to_channel
        .send(command(
            1,
            "POST",
            "/api/test/items",
            Some(Value::String("x".into())),
        ))
        .await
        .unwrap();
    let ServerMessage::Changes(changes) = next(&mut driven).await else {
        panic!("the change must come before the reply");
    };
    assert_eq!(
        changes.upsert,
        vec![KeyedEntry {
            key: "\"x\"".into(),
            value: serde_json::json!({ "id": "\"x\"" }),
        }]
    );
    let ServerMessage::Reply(reply) = next(&mut driven).await else {
        panic!("then the reply");
    };
    assert_eq!((reply.id, reply.status), (1, 201));
}

/// A feed bump while idle re-reads the collections; a read that fails then
/// is pushed as unavailable, and nothing the GUI holds is removed.
#[farhelm_testtrace::test]
async fn a_feed_bump_pushes_changes_and_a_failed_read_removes_nothing() {
    let routes = Arc::new(TestRoutes::default());
    routes
        .items
        .lock()
        .unwrap()
        .push(serde_json::json!({ "id": "kept" }));
    let (mut driven, _snapshot) = drive(Arc::clone(&routes)).await;

    routes.fail_items.store(true, Ordering::SeqCst);
    driven.feed.send_modify(|revision| *revision += 1);
    let ServerMessage::Changes(changes) = next(&mut driven).await else {
        panic!("a failed read is pushed as a change of availability");
    };
    assert!(matches!(
        changes.meta.availability,
        Availability::Unavailable { .. }
    ));
    assert!(changes.remove.is_empty(), "a failed read removes nothing");
    assert_eq!(changes.meta.order, ["kept"]);
}

/// A canonical path can still hold an id with an encoded slash, which a
/// route would decode into two segments. The dispatcher refuses it with an
/// ordinary reply, never routes it, and the channel carries on with the next
/// command: an odd host id costs that one action, not a reset.
#[farhelm_testtrace::test]
async fn an_encoded_separator_is_refused_without_being_routed() {
    let routes = Arc::new(TestRoutes::default());
    let (mut driven, _snapshot) = drive(Arc::clone(&routes)).await;
    driven
        .to_channel
        .send(command(
            1,
            "PUT",
            "/api/test/x%2F..%2Fvalue",
            Some(Value::String("no".into())),
        ))
        .await
        .unwrap();
    let ServerMessage::Reply(reply) = next(&mut driven).await else {
        panic!("expected the refusal");
    };
    assert_eq!((reply.id, reply.status), (1, 400));
    assert!(routes.value.lock().unwrap().is_none(), "nothing was routed");

    driven
        .to_channel
        .send(command(2, "GET", "/api/test/value", None))
        .await
        .unwrap();
    let ServerMessage::Reply(reply) = next(&mut driven).await else {
        panic!("the next command still runs");
    };
    assert_eq!((reply.id, reply.status), (2, 200));
}

/// Commands run one at a time, so a GUI that keeps sending while one is stuck
/// would grow the queue without bound. The channel holds up to
/// `MAX_QUEUED_COMMANDS` behind the one in flight and closes on the next: a
/// close is the failure the GUI already handles, by resetting.
#[farhelm_testtrace::test]
async fn a_gui_that_floods_commands_behind_a_stuck_one_is_disconnected() {
    let routes = Arc::new(TestRoutes::default());
    let (_release, gate) = oneshot::channel();
    *routes.save_gate.lock().unwrap() = Some(gate);
    let (mut driven, _snapshot) = drive(Arc::clone(&routes)).await;
    driven
        .to_channel
        .send(command(
            0,
            "PUT",
            "/api/test/value",
            Some(Value::String("held".into())),
        ))
        .await
        .unwrap();
    for id in 1..=MAX_QUEUED_COMMANDS as u64 {
        driven
            .to_channel
            .send(command(id, "GET", "/api/test/value", None))
            .await
            .unwrap();
    }
    // Messages are handled in order, so a heartbeat answered now proves the
    // full queue was accepted without closing.
    driven
        .to_channel
        .send(ClientMessage::Ping { n: 1 })
        .await
        .unwrap();
    assert_eq!(next(&mut driven).await, ServerMessage::Pong { n: 1 });

    driven
        .to_channel
        .send(command(999, "GET", "/api/test/value", None))
        .await
        .unwrap();
    tokio::time::timeout(NOTICE, driven.task)
        .await
        .expect("one command past the bound closes the channel")
        .unwrap();
    assert!(
        routes.value.lock().unwrap().is_none(),
        "premise: the first command was still held when the queue overflowed"
    );
}

/// Only the helm's word counts as a takeover (SPEC.md): a displaced channel
/// is sent `displaced`, and then the channel ends.
#[farhelm_testtrace::test]
async fn a_displaced_channel_says_so_and_ends() {
    let (mut driven, _snapshot) = drive(Arc::new(TestRoutes::default())).await;
    driven.displace.take().unwrap().send(()).unwrap();
    assert_eq!(next(&mut driven).await, ServerMessage::Displaced);
    tokio::time::timeout(NOTICE, driven.task)
        .await
        .expect("a displaced channel ends")
        .unwrap();
}

// ---------------------------------------------------------------------------
// The socket bridge, against test routes
// ---------------------------------------------------------------------------

/// How the test's stand-in for the socket's write half treats what the helm
/// writes: hand it to the test, fail at once (a GUI that went away), or never
/// finish (a GUI that stopped reading).
#[derive(Clone, Copy)]
enum Writes {
    Accept,
    Fail,
    Hang,
}

/// A [`serve_socket`] running over the test routes, with an in-memory socket:
/// frames the test sends in, frames the helm wrote out.
struct Bridged {
    to_helm: mpsc::UnboundedSender<Result<ws::Message, axum::Error>>,
    from_helm: mpsc::UnboundedReceiver<ws::Message>,
    /// Held so the feed stays open: a closed feed would end the channel on
    /// its own and hide what the test looks at.
    _feed: watch::Sender<u64>,
    task: tokio::task::JoinHandle<()>,
}

/// Bridge a channel over the test routes to an in-memory socket whose writes
/// behave as `writes` says. `displaced` is the seat receiver the channel holds.
fn bridge(routes: Arc<TestRoutes>, writes: Writes, displaced: oneshot::Receiver<()>) -> Bridged {
    let channel = Channel::new(
        test_router(routes),
        HeaderValue::from_static("Bearer test"),
        vec![test_collection()],
    );
    let (to_helm, mut from_gui) = mpsc::unbounded_channel();
    let rx = futures_util::stream::poll_fn(move |cx| from_gui.poll_recv(cx));
    let (written, from_helm) = mpsc::unbounded_channel();
    let tx = futures_util::sink::unfold(written, move |written, message: ws::Message| async move {
        match writes {
            Writes::Accept => {
                let _ = written.send(message);
                Ok(written)
            }
            Writes::Fail => Err(axum::Error::new("the GUI went away")),
            Writes::Hang => std::future::pending().await,
        }
    });
    let (feed, feed_rx) = watch::channel(0);
    let task = tokio::spawn(serve_socket(Box::pin(tx), rx, channel, displaced, feed_rx));
    Bridged {
        to_helm,
        from_helm,
        _feed: feed,
        task,
    }
}

impl Bridged {
    /// Send one text frame, as the GUI's socket would deliver it.
    fn send_text(&self, text: &str) {
        let _ = self
            .to_helm
            .send(Ok(ws::Message::Text(text.to_string().into())));
    }

    /// The next message the helm wrote, which must arrive promptly.
    async fn next(&mut self) -> ServerMessage {
        let frame = tokio::time::timeout(NOTICE, self.from_helm.recv())
            .await
            .expect("the helm should write promptly")
            .expect("the bridge should still be writing");
        match frame {
            ws::Message::Text(text) => {
                serde_json::from_str(text.as_str()).expect("a server message")
            }
            other => panic!("expected a text frame, got {other:?}"),
        }
    }
}

/// A command whose path is not canonical does not decode, so it is never
/// routed: the channel closes as for any message it has no word for, which is
/// the reset a correct GUI (whose path type cannot build such a path) never
/// sees. The canonical command before it is routed, so the refusal is the
/// path's doing, not a dead channel.
#[farhelm_testtrace::test]
async fn a_command_whose_path_is_not_canonical_ends_the_channel_unrouted() {
    let routes = Arc::new(TestRoutes::default());
    let seat = UiSeat::default();
    let mut bridged = bridge(Arc::clone(&routes), Writes::Accept, seat.take());
    assert!(matches!(
        bridged.next().await,
        ServerMessage::Snapshot { .. }
    ));

    bridged.send_text(
        r#"{"type":"command","id":1,"method":"PUT","path":"/api/test/value","body":"kept"}"#,
    );
    let ServerMessage::Reply(reply) = bridged.next().await else {
        panic!("premise: a canonical command is routed and answered");
    };
    assert_eq!((reply.id, reply.status), (1, 204));

    bridged.send_text(
        r#"{"type":"command","id":2,"method":"PUT","path":"/api/test/%2e%2e/test/value","body":"overwritten"}"#,
    );
    tokio::time::timeout(NOTICE, bridged.task)
        .await
        .expect("a non-canonical path ends the channel")
        .unwrap();
    assert_eq!(
        routes.value.lock().unwrap().as_deref(),
        Some("\"kept\""),
        "the non-canonical command reached no real route"
    );
    // A router does not normalize `%2e%2e`, so that path would miss every real route anyway; only the fallback's
    // count shows the command never reached the router at all.
    assert_eq!(
        routes.unrouted_hits.load(Ordering::SeqCst),
        0,
        "the non-canonical command was never routed"
    );
    assert!(!seat.is_held(), "the ended channel gave the seat up");
}

/// A write that fails must end the channel and free the seat: before the
/// writer owned its queue, the channel kept running with nobody draining it,
/// held the seat for good, and agent approvals kept waiting on a window that
/// would never show their card. The GUI's side stays open throughout, so only
/// the failed write can be what ended it.
#[farhelm_testtrace::test]
async fn a_failed_write_ends_the_channel_and_gives_up_the_seat() {
    let seat = UiSeat::default();
    let bridged = bridge(Arc::new(TestRoutes::default()), Writes::Fail, seat.take());
    assert!(seat.is_held(), "premise: the channel holds the seat");
    tokio::time::timeout(NOTICE, bridged.task)
        .await
        .expect("a failed write ends the channel")
        .unwrap();
    assert!(!seat.is_held());
    // `bridged.to_helm` is still held here, so the GUI side never closed or sent anything, and the silence close is a
    // minute away: the reader cannot be what ended the channel.
}

/// A GUI that stops reading must not hold the channel forever either: a
/// write that never completes ends it at the write deadline, well before the
/// silence close could be what ended it. The paused clock makes both exact.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_write_that_never_completes_ends_the_channel_at_the_write_deadline() {
    let seat = UiSeat::default();
    let started = tokio::time::Instant::now();
    let bridged = bridge(Arc::new(TestRoutes::default()), Writes::Hang, seat.take());
    bridged.task.await.unwrap();
    let elapsed = started.elapsed();
    assert!(
        elapsed >= WRITE_DEADLINE && elapsed < SILENCE_CLOSE,
        "ended after {elapsed:?}, not at the write deadline"
    );
    assert!(!seat.is_held());
}

/// The helm sends no keepalive; it closes a channel whose GUI sent nothing
/// for a minute, which gives up the seat of a GUI that vanished without
/// closing. A GUI that heartbeats (the client does every 5 seconds) is never
/// closed, however long it stays otherwise quiet. The helm writes nothing
/// during the silence, which is what tells this design from the idle ping it
/// replaced (that one also closed after a minute, but pinged first).
#[farhelm_testtrace::test(start_paused = true)]
async fn a_silent_gui_is_closed_after_a_minute_and_a_heartbeat_keeps_it_open() {
    let seat = UiSeat::default();
    let mut bridged = bridge(Arc::new(TestRoutes::default()), Writes::Accept, seat.take());
    assert!(matches!(
        bridged.next().await,
        ServerMessage::Snapshot { .. }
    ));

    // Heartbeats for twice the silence bound: every one is answered, and the
    // channel stays open past the point where silence would have closed it.
    for n in 0..24 {
        // sleep-ok: the paused clock's stand-in for the client's 5-second heartbeat interval.
        tokio::time::sleep(Duration::from_secs(5)).await;
        bridged.send_text(&format!(r#"{{"type":"ping","n":{n}}}"#));
        assert_eq!(bridged.next().await, ServerMessage::Pong { n });
    }
    assert!(
        !bridged.task.is_finished(),
        "a heartbeating GUI stays connected"
    );
    assert!(seat.is_held());

    let quiet_from = tokio::time::Instant::now();
    bridged.task.await.unwrap();
    let silent_for = quiet_from.elapsed();
    assert!(
        silent_for >= SILENCE_CLOSE && silent_for < SILENCE_CLOSE + Duration::from_secs(1),
        "closed after {silent_for:?} of silence"
    );
    assert!(!seat.is_held());
    while let Ok(frame) = bridged.from_helm.try_recv() {
        assert!(
            matches!(frame, ws::Message::Close(_)),
            "the helm wrote {frame:?} to a silent GUI; it must send nothing until it closes"
        );
    }
}

// ---------------------------------------------------------------------------
// The reply bound
// ---------------------------------------------------------------------------

/// A slow host must not hold every later GUI command behind it: past
/// `HOST_REPLY_BOUND` the route answers that the host timed out (504), and
/// the work itself is not abandoned. The paused clock makes the bound exact.
#[farhelm_testtrace::test(start_paused = true)]
async fn the_reply_bound_answers_a_slow_host_with_a_timeout_and_keeps_the_work() {
    let (release, held) = oneshot::channel::<()>();
    let (finished_tx, finished) = oneshot::channel();
    let response = crate::sessions::within_host_reply_bound(async move {
        let _ = held.await;
        let _ = finished_tx.send(());
        StatusCode::OK.into_response()
    })
    .await;
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    release
        .send(())
        .expect("the work is still running after the reply");
    finished.await.expect("the work runs to completion");
}

/// The bound is on the real routes, not only on the helper: a rename and an
/// ordinary Delete against a host that never answers (the scripted fixture
/// host answers neither) reply 504 once `HOST_REPLY_BOUND` passes. Removing
/// the bound from either route makes this hang until nextest's timeout.
#[farhelm_testtrace::test(start_paused = true)]
async fn rename_and_delete_answer_504_when_the_host_never_answers() {
    let harness = rest_harness::helm_listing(vec![rest_harness::session("s1", 100)]).await;
    let full = harness.local_session_ref("s1");
    let rename = http(
        &harness,
        "POST",
        &format!("/api/sessions/{full}/rename"),
        serde_json::json!({ "title": "renamed" }),
    )
    .await;
    assert_eq!(rename, StatusCode::GATEWAY_TIMEOUT);
    let request = Request::builder()
        .method("DELETE")
        .uri(format!("/api/sessions/{full}"))
        .header("host", "127.0.0.1:7433")
        .body(Body::empty())
        .unwrap();
    let delete = harness.router().oneshot(request).await.unwrap().status();
    assert_eq!(delete, StatusCode::GATEWAY_TIMEOUT);
}

// ---------------------------------------------------------------------------
// The endpoint, against a real helm
// ---------------------------------------------------------------------------

/// The next server message off a real channel, skipping transport frames.
async fn recv_message(ws: &mut WsTestClient) -> ServerMessage {
    loop {
        let (opcode, payload) = tokio::time::timeout(NOTICE, ws.recv())
            .await
            .expect("the channel should send promptly")
            .expect("the channel should still be open");
        match opcode {
            1 => return serde_json::from_slice(&payload).expect("a server message"),
            9 | 10 => continue,
            other => panic!("unexpected frame opcode {other}"),
        }
    }
}

async fn send_message(ws: &mut WsTestClient, message: &ClientMessage) {
    let text = serde_json::to_vec(message).unwrap();
    ws.send_frame(true, 1, &text).await;
}

/// Read until the next message for `collection` (skipping others).
async fn changes_of(ws: &mut WsTestClient, collection: Collection) -> CollectionChanges {
    loop {
        if let ServerMessage::Changes(changes) = recv_message(ws).await
            && changes.collection == collection
        {
            return changes;
        }
    }
}

/// The snapshot sent on open holds every pushed collection as the GET
/// routes serve it: the session rows decode as the proto type under their
/// served ids, hosts are keyed by registry id, preferences and the
/// announcement queue are one entry each, and the empty lists (approvals,
/// templates, notices, host progress) are available, empty collections.
#[farhelm_testtrace::test]
async fn the_snapshot_on_open_holds_every_collection_as_served() {
    let mut harness = rest_harness::helm_listing(vec![
        rest_harness::session("s1", 100),
        rest_harness::session("s2", 200),
    ])
    .await;
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let ServerMessage::Snapshot { collections } = recv_message(&mut ws).await else {
        panic!("the first message is the snapshot");
    };
    let names: Vec<Collection> = collections.iter().map(|c| c.collection).collect();
    assert_eq!(
        names,
        [
            Collection::Sessions,
            Collection::Hosts,
            Collection::Preferences,
            Collection::Approvals,
            Collection::Templates,
            Collection::Notices,
            Collection::Announcements,
            Collection::HostProgress,
        ]
    );
    let sessions = &collections[0];
    assert_eq!(sessions.meta.availability, Availability::Available);
    let rows: Vec<SessionRow> = sessions
        .entries
        .iter()
        .map(|entry| serde_json::from_value(entry.value.clone()).expect("a session row"))
        .collect();
    let ids: Vec<String> = rows.iter().map(|row| row.info.id.clone()).collect();
    let keys: Vec<&str> = sessions
        .entries
        .iter()
        .map(|entry| entry.key.as_str())
        .collect();
    assert_eq!(keys, ids, "each row travels under its served id");
    // The list's default order is creation time, newest first.
    assert_eq!(
        ids,
        [
            harness.local_session_ref("s2"),
            harness.local_session_ref("s1")
        ]
    );
    assert_eq!(sessions.meta.order, ids);
    assert_eq!(sessions.meta.counts.map(|c| c.total), Some(2));
    let local = rest_harness::local_id(&harness.store).await;
    assert_eq!(collections[1].meta.order, [local.to_string()]);
    assert_eq!(collections[2].meta.order, [PREFERENCES_KEY]);
    for empty in [3, 4, 5, 7] {
        assert_eq!(
            collections[empty].meta.availability,
            Availability::Available,
            "{:?}",
            collections[empty].collection
        );
        assert!(collections[empty].entries.is_empty());
    }
    assert_eq!(collections[6].meta.order, [ANNOUNCEMENTS_KEY]);
}

const UI_CHANNEL: &str = farhelm_proto::ui_channel::UI_CHANNEL_PATH;

/// One HTTP request to the real router, as a client other than the channel.
async fn http(
    harness: &rest_harness::Harness,
    method: &str,
    path: &str,
    body: Value,
) -> StatusCode {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    harness.router().oneshot(request).await.unwrap().status()
}

/// An ordinary preference write used not to bump the feed, so a connected GUI
/// never learned of it. Made outside the channel while the channel is idle,
/// it can only reach the GUI through the feed bump, so this fails without it
/// (a command over the channel would not: the channel re-reads after every
/// command anyway).
#[farhelm_testtrace::test]
async fn a_preference_write_outside_the_channel_reaches_an_idle_connected_ui() {
    let mut harness = rest_harness::idle_helm().await;
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let _snapshot = recv_message(&mut ws).await;
    let status = http(
        &harness,
        "PUT",
        "/api/preferences",
        serde_json::json!({ "compact": true }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let changes = changes_of(&mut ws, Collection::Preferences).await;
    let preferences: farhelm_proto::ui_channel::Preferences =
        serde_json::from_value(changes.upsert[0].value.clone()).expect("preferences decode");
    assert_eq!(preferences.compact, Some(true));
}

/// Dismissing announcements used not to bump the feed, so a connected GUI
/// kept showing a dismissed announcement as pending. Made outside the channel
/// while it is idle, the dismissal can only reach the GUI through the feed
/// bump, and it arrives as the whole queue with the entry moved to history.
#[farhelm_testtrace::test]
async fn dismissing_an_announcement_outside_the_channel_moves_it_to_history_on_the_ui() {
    use farhelm_proto::announcements::{Announcement, AnnouncementQueue};
    let mut harness = rest_harness::idle_helm().await;
    harness
        .store
        .enqueue_announcements(vec![Announcement {
            id: "news-1".into(),
            title: "News".into(),
            body: "Something new.".into(),
            detail: None,
            link: None,
        }])
        .await
        .unwrap();
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let ServerMessage::Snapshot { collections } = recv_message(&mut ws).await else {
        panic!("the first message is the snapshot");
    };
    let queue: AnnouncementQueue =
        serde_json::from_value(collections[6].entries[0].value.clone()).expect("a queue");
    assert!(
        queue.pending.iter().any(|entry| entry.id == "news-1"),
        "premise: the announcement is pending"
    );
    let status = http(
        &harness,
        "POST",
        "/api/announcements/shown",
        serde_json::json!({ "ids": ["news-1"] }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let changes = changes_of(&mut ws, Collection::Announcements).await;
    assert_eq!(changes.upsert[0].key, ANNOUNCEMENTS_KEY);
    let queue: AnnouncementQueue =
        serde_json::from_value(changes.upsert[0].value.clone()).expect("a queue");
    assert!(queue.pending.iter().all(|entry| entry.id != "news-1"));
    assert!(queue.history.iter().any(|entry| entry.id == "news-1"));
}

/// A template saved outside the channel (another client's HTTP write) reaches
/// an idle connected UI through the feed bump, keyed by the template's name.
/// With the channel idle, nothing but the bump in `templates::store_template`
/// (shared with the agent relay's writes) can deliver it.
#[farhelm_testtrace::test]
async fn a_template_saved_outside_the_channel_reaches_an_idle_connected_ui() {
    let mut harness = rest_harness::idle_helm().await;
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let _snapshot = recv_message(&mut ws).await;
    let status = http(
        &harness,
        "PUT",
        "/api/templates/review",
        serde_json::json!({ "prompt": "review this" }),
    )
    .await;
    assert!(status.is_success(), "{status}");
    let changes = changes_of(&mut ws, Collection::Templates).await;
    assert_eq!(changes.upsert[0].key, "review");
    let template: farhelm_proto::launcher::LaunchTemplate =
        serde_json::from_value(changes.upsert[0].value.clone()).expect("a template");
    assert_eq!(template.fields.prompt.as_deref(), Some("review this"));
}

/// An agent's `template create`, taken through the whole relay path (the verb
/// handler, its card on the connected UI, the user's Allow), ends with the
/// template on that UI, keyed by name. An agent's write is a template change
/// the GUI did not make, so the push is the only way the GUI learns of it.
///
/// NOTE: this does not isolate the write's own feed bump. Answering the card
/// removes it and bumps the feed, and the re-read that follows can already
/// include the template. The write's bump is in `templates::store_template`,
/// which the relay and the GUI's PUT share; the idle-channel PUT test above
/// is the one that fails without it.
#[farhelm_testtrace::test]
async fn an_agents_approved_template_create_reaches_the_connected_ui() {
    use crate::agent_requests::HelmAgentRequests;
    use farhelm_proto::{AgentOutcome, AgentReply, AgentVerb};
    let mut harness = rest_harness::idle_helm().await;
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let _snapshot = recv_message(&mut ws).await;
    let origin = crate::approvals::tests::local_origin(&harness).await;
    let handler = HelmAgentRequests::for_state(&harness.state);
    let asking = tokio::spawn(async move {
        handler
            .handle(
                origin,
                "sess-1",
                AgentVerb::TemplateCreate {
                    name: "agent-made".into(),
                    fields: serde_json::from_value(
                        serde_json::json!({ "prompt": "from an agent" }),
                    )
                    .unwrap(),
                    host: None,
                    shortcuts: Default::default(),
                },
            )
            .await
    });
    // A template write always waits for a visible Allow, and the card that
    // asks for it reaches the UI as a pushed approval.
    let card = changes_of(&mut ws, Collection::Approvals).await;
    let id = card.upsert[0].key.clone();
    let status = http(
        &harness,
        "POST",
        &format!("/api/approvals/{id}"),
        serde_json::json!({ "answer": "allow" }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let outcome = tokio::time::timeout(NOTICE, asking)
        .await
        .expect("the request ends once allowed")
        .unwrap();
    assert!(
        matches!(
            outcome,
            AgentOutcome::Ok {
                reply: AgentReply::TemplateWritten { .. }
            }
        ),
        "{outcome:?}"
    );
    let changes = changes_of(&mut ws, Collection::Templates).await;
    assert_eq!(changes.upsert[0].key, "agent-made");
    let template: farhelm_proto::launcher::LaunchTemplate =
        serde_json::from_value(changes.upsert[0].value.clone()).expect("a template");
    assert_eq!(template.fields.prompt.as_deref(), Some("from an agent"));
}

/// A trash-listing claim on the harness's local host. `notices`' own claim
/// helper models an identity-less host, but `helm_listing` scripts the local
/// supervisor with `local-identity`, and the recording transaction refuses a
/// claim whose identity differs from the host's stored binding.
async fn local_trash_claim(harness: &rest_harness::Harness) -> crate::manager::SessionClaim {
    let mut claim = crate::notices::tests::claim(&harness.store).await;
    claim.identity = Some("local-identity".to_string());
    claim
}

/// Marking a notice read outside the channel reaches an idle connected UI as
/// the replaced notice. (A notice the supervisor's trash listing raises bumps
/// the feed at its own boundary; `notices`' forwarded-listing test proves that
/// bump.)
#[farhelm_testtrace::test]
async fn marking_a_notice_read_outside_the_channel_reaches_an_idle_connected_ui() {
    let mut harness = rest_harness::idle_helm().await;
    let claim = local_trash_claim(&harness).await;
    assert!(crate::notices::tests::record(
        &harness.store,
        &claim,
        &crate::notices::tests::stuck("first")
    ));
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let ServerMessage::Snapshot { collections } = recv_message(&mut ws).await else {
        panic!("the first message is the snapshot");
    };
    let notice: farhelm_proto::notices::Notice =
        serde_json::from_value(collections[5].entries[0].value.clone()).expect("a notice");
    assert!(!notice.read, "premise: the notice is unread");
    assert_eq!(collections[5].entries[0].key, notice.id.to_string());
    let status = http(
        &harness,
        "POST",
        "/api/notices/marks",
        serde_json::json!({ "action": "read", "ids": [notice.id] }),
    )
    .await;
    assert!(status.is_success(), "{status}");
    let changes = changes_of(&mut ws, Collection::Notices).await;
    let marked: farhelm_proto::notices::Notice =
        serde_json::from_value(changes.upsert[0].value.clone()).expect("a notice");
    assert_eq!((marked.id, marked.read), (notice.id, true));
}

/// A provisioning run's progress reaches a connected UI as its steps move:
/// the step transition (`set_step`, the same call a running plan makes)
/// bumps the feed, and the host's run arrives keyed by host id.
#[farhelm_testtrace::test]
async fn a_provisioning_step_transition_reaches_the_connected_ui() {
    use farhelm_proto::provisioning::{
        ProvisioningOperation, ProvisioningView, RunStatus, StepStatus, StepView,
    };
    let mut harness = rest_harness::idle_helm().await;
    let host = rest_harness::local_id(&harness.store).await;
    harness
        .state
        .provisioning
        .insert_run_for_test(ProvisioningView {
            host_id: host,
            run_id: Some("run-1".into()),
            operation: Some(ProvisioningOperation::Update),
            status: RunStatus::Running,
            steps: vec![StepView {
                step: "install".into(),
                name: "Install the supervisor".into(),
                paths: vec![],
                explanation: None,
                status: StepStatus::Pending,
                message: None,
            }],
            message: None,
        })
        .await;
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let ServerMessage::Snapshot { collections } = recv_message(&mut ws).await else {
        panic!("the first message is the snapshot");
    };
    assert_eq!(collections[7].meta.order, [host.to_string()], "premise");
    harness
        .state
        .provisioning
        .set_step_for_test(host, 0, StepStatus::Running, Some("copying".into()))
        .await;
    let changes = changes_of(&mut ws, Collection::HostProgress).await;
    assert_eq!(changes.upsert[0].key, host.to_string());
    let run: ProvisioningView =
        serde_json::from_value(changes.upsert[0].value.clone()).expect("a run");
    assert_eq!(run.steps[0].status, StepStatus::Running);
    assert_eq!(run.steps[0].message.as_deref(), Some("copying"));
}

/// A preference write sent as a command comes back as a change to the
/// preferences collection, before the write's reply, through the real route.
#[farhelm_testtrace::test]
async fn an_ordinary_preference_patch_reaches_the_connected_ui_before_its_reply() {
    let mut harness = rest_harness::idle_helm().await;
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let _snapshot = recv_message(&mut ws).await;
    send_message(
        &mut ws,
        &command(
            1,
            "PUT",
            "/api/preferences",
            Some(serde_json::json!({ "compact": true })),
        ),
    )
    .await;
    let changes = changes_of(&mut ws, Collection::Preferences).await;
    let preferences: farhelm_proto::ui_channel::Preferences =
        serde_json::from_value(changes.upsert[0].value.clone()).expect("preferences decode");
    assert_eq!(preferences.compact, Some(true));
    let ServerMessage::Reply(reply) = recv_message(&mut ws).await else {
        panic!("the reply follows the change");
    };
    assert_eq!((reply.id, reply.status), (1, 204));
}

/// Changes made outside the GUI reach it too. An agent's approved host
/// edit goes through the relay's shared mutation path
/// (`hosts::edit_identity_checked`), which bumps the feed, and the connected
/// UI receives the changed host without asking for it.
#[farhelm_testtrace::test]
async fn a_change_through_the_agent_relays_mutation_path_reaches_the_connected_ui() {
    use farhelm_proto::host_appearance::HostColor;
    let mut harness = rest_harness::idle_helm().await;
    let local = rest_harness::local_id(&harness.store).await;
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let ServerMessage::Snapshot { collections } = recv_message(&mut ws).await else {
        panic!("the first message is the snapshot");
    };
    let before: farhelm_proto::ui_channel::HostView =
        serde_json::from_value(collections[1].entries[0].value.clone()).expect("a host");
    assert_ne!(before.color, HostColor::Teal, "premise: not teal yet");

    crate::hosts::edit_identity_checked(
        &harness.state,
        local,
        crate::hosts::IdentityEdit {
            alias: None,
            icon: None,
            color: Some(HostColor::Teal),
        },
        |_| Ok(()),
    )
    .await
    .expect("the edit succeeds");
    let changes = changes_of(&mut ws, Collection::Hosts).await;
    let host: farhelm_proto::ui_channel::HostView =
        serde_json::from_value(changes.upsert[0].value.clone()).expect("a host");
    assert_eq!((host.id, host.color), (local, HostColor::Teal));
    assert_eq!(changes.upsert[0].key, local.to_string());
}

/// Wait until the seat's state is `held`, polling briefly: a disconnect is
/// noticed by the serving task on its own schedule.
async fn await_seat(harness: &rest_harness::Harness, held: bool) {
    tokio::time::timeout(NOTICE, async {
        while harness.state.manager.ui_seat().is_held() != held {
            // sleep-ok: polling interval for the serving task's own ending after a disconnect.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("the seat should become held={held}"));
}

/// Agent approvals ask the seat whether a window is open (SPEC.md, "One GUI
/// at a time"): a connected UI counts; one that disconnected does not; and a
/// takeover moves the seat to the new UI, whose old holder is told it was
/// displaced. The old `/api/events` subscription no longer counts.
#[farhelm_testtrace::test]
async fn approvals_follow_the_seat_through_connect_disconnect_and_takeover() {
    let mut harness = rest_harness::idle_helm().await;
    let addr = harness.serve().await;
    assert!(!crate::approvals::gui_connected(&harness.state));

    let events = WsTestClient::connect(addr, "/api/events").await;
    assert!(
        !crate::approvals::gui_connected(&harness.state),
        "an events subscriber is not a window"
    );
    drop(events);

    let mut first = WsTestClient::connect(addr, UI_CHANNEL).await;
    let _snapshot = recv_message(&mut first).await;
    assert!(crate::approvals::gui_connected(&harness.state));

    drop(first);
    await_seat(&harness, false).await;
    assert!(!crate::approvals::gui_connected(&harness.state));

    let mut older = WsTestClient::connect(addr, UI_CHANNEL).await;
    let _snapshot = recv_message(&mut older).await;
    let mut newer = WsTestClient::connect(addr, UI_CHANNEL).await;
    let _snapshot = recv_message(&mut newer).await;
    assert_eq!(recv_message(&mut older).await, ServerMessage::Displaced);
    assert!(
        crate::approvals::gui_connected(&harness.state),
        "the newer UI holds the seat"
    );
    drop(older);
    // The displaced channel's ending must not free the newer UI's seat.
    send_message(&mut newer, &ClientMessage::Ping { n: 1 }).await;
    assert_eq!(recv_message(&mut newer).await, ServerMessage::Pong { n: 1 });
    assert!(crate::approvals::gui_connected(&harness.state));
}

/// The session-list query is this channel's state: setting it re-serves the
/// list filtered, and a word the list route refuses is refused here too,
/// leaving the old query in force.
#[farhelm_testtrace::test]
async fn the_session_query_filters_the_pushed_list_and_refusals_keep_the_old_one() {
    let mut harness = rest_harness::helm_listing(vec![
        rest_harness::session("alpha", 100),
        rest_harness::session("beta", 200),
    ])
    .await;
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let _snapshot = recv_message(&mut ws).await;

    send_message(
        &mut ws,
        &command(
            1,
            "PUT",
            SESSION_QUERY_PATH,
            Some(serde_json::json!({ "title": "alp" })),
        ),
    )
    .await;
    let changes = changes_of(&mut ws, Collection::Sessions).await;
    assert_eq!(changes.remove, [harness.local_session_ref("beta")]);
    assert_eq!(changes.meta.counts.and_then(|c| c.matching), Some(1));
    let ServerMessage::Reply(reply) = recv_message(&mut ws).await else {
        panic!("the reply follows");
    };
    assert_eq!((reply.id, reply.status), (1, 204));

    send_message(
        &mut ws,
        &command(
            2,
            "PUT",
            SESSION_QUERY_PATH,
            Some(serde_json::json!({ "sort": "sideways" })),
        ),
    )
    .await;
    let ServerMessage::Reply(reply) = recv_message(&mut ws).await else {
        panic!("a refused query sends no changes");
    };
    assert_eq!((reply.id, reply.status), (2, 400));
}

// ---------------------------------------------------------------------------
// Wire-type contracts
// ---------------------------------------------------------------------------

/// GET the route through the real router and return its JSON.
async fn served(harness: &rest_harness::Harness, path: &str) -> Value {
    let request = Request::builder()
        .uri(path)
        .header("host", "127.0.0.1:7433")
        .body(Body::empty())
        .unwrap();
    let response = harness.router().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{path}");
    serde_json::from_slice(
        &to_bytes(response.into_body(), MAX_REPLY_BODY)
            .await
            .unwrap(),
    )
    .unwrap()
}

/// Decode `json` as `T` and encode it again: the two must agree, or the GUI
/// would read a different shape than the helm served.
fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned>(json: &Value) -> T {
    let decoded: T = serde_json::from_value(json.clone()).expect("the served entry decodes");
    assert_eq!(
        &serde_json::to_value(&decoded).unwrap(),
        json,
        "the entry re-encodes to what was served"
    );
    decoded
}

/// The helm serves the same proto types the GUI decodes. Each pushed
/// collection's served JSON must decode and re-encode unchanged. (The key an
/// entry is filed under needs no check here: it travels on the wire beside
/// the entry, so the GUI files it under the key the helm diffed it by.)
#[farhelm_testtrace::test]
async fn every_pushed_collection_round_trips_through_its_proto_type() {
    let harness = rest_harness::helm_listing(vec![rest_harness::session("s1", 100)]).await;

    let sessions = served(&harness, "/api/sessions").await;
    let row = &sessions["sessions"][0];
    let _: SessionRow = round_trip(row);
    let list: farhelm_proto::ui_channel::SessionList =
        serde_json::from_value(sessions.clone()).expect("the list decodes");
    assert_eq!(serde_json::to_value(&list).unwrap(), sessions);

    let patch = Request::builder()
        .method("PUT")
        .uri("/api/preferences")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"compact":true,"list_sort":"title"}"#))
        .unwrap();
    let written = harness.router().oneshot(patch).await.unwrap();
    assert_eq!(written.status(), StatusCode::NO_CONTENT);
    let preferences = served(&harness, "/api/preferences").await;
    let decoded: farhelm_proto::ui_channel::Preferences = round_trip(&preferences);
    assert_eq!(decoded.list_sort.as_deref(), Some("title"));

    let hosts = served(&harness, "/api/hosts").await;
    let host = &hosts["hosts"][0];
    let _: farhelm_proto::ui_channel::HostView = round_trip(host);

    // A card needs an open window; the waiting request is aborted at the end.
    let _gui = harness.state.manager.ui_seat().hold();
    let origin = crate::approvals::tests::local_origin(&harness).await;
    let (asking, id) = crate::approvals::tests::waiting(&harness, origin, "asker").await;
    let approvals = served(&harness, "/api/approvals").await;
    let entry = &approvals["approvals"][0];
    let decoded: farhelm_proto::approvals::PendingApproval = round_trip(entry);
    assert_eq!(decoded.id, id);
    assert_eq!(entry["id"].as_str(), Some(id.as_str()));
    asking.abort();

    let put = Request::builder()
        .method("PUT")
        .uri("/api/templates/review")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"prompt":"review this"}"#))
        .unwrap();
    assert!(
        harness
            .router()
            .oneshot(put)
            .await
            .unwrap()
            .status()
            .is_success()
    );
    let templates = served(&harness, "/api/templates").await;
    let _: farhelm_proto::launcher::LaunchTemplate = round_trip(&templates["templates"][0]);

    let claim = local_trash_claim(&harness).await;
    assert!(crate::notices::tests::record(
        &harness.store,
        &claim,
        &crate::notices::tests::stuck("first")
    ));
    let notices = served(&harness, "/api/notices").await;
    let _: farhelm_proto::notices::Notice = round_trip(&notices["notices"][0]);

    harness
        .store
        .enqueue_announcements(vec![farhelm_proto::announcements::Announcement {
            id: "news-1".into(),
            title: "News".into(),
            body: "Something new.".into(),
            detail: Some("More.".into()),
            link: Some(farhelm_proto::announcements::AnnouncementLink {
                label: "Read".into(),
                url: "https://example.com/news".into(),
            }),
        }])
        .await
        .unwrap();
    let announcements = served(&harness, "/api/announcements").await;
    let queue: farhelm_proto::announcements::AnnouncementQueue = round_trip(&announcements);
    assert!(queue.pending.iter().any(|entry| entry.id == "news-1"));

    let host = rest_harness::local_id(&harness.store).await;
    harness
        .state
        .provisioning
        .insert_run_for_test(farhelm_proto::provisioning::ProvisioningView {
            host_id: host,
            run_id: Some("run-1".into()),
            operation: Some(farhelm_proto::provisioning::ProvisioningOperation::Add),
            status: farhelm_proto::provisioning::RunStatus::Running,
            steps: vec![farhelm_proto::provisioning::StepView {
                step: "install".into(),
                name: "Install the supervisor".into(),
                paths: vec![farhelm_proto::provisioning::PlanStepPath {
                    full: "/home/user/.local/bin/farhelm".into(),
                    short: "~/.local/bin/farhelm".into(),
                }],
                explanation: Some("Copies the binary.".into()),
                status: farhelm_proto::provisioning::StepStatus::Pending,
                message: None,
            }],
            message: None,
        })
        .await;
    let runs = served(&harness, "/api/provisioning").await;
    let run: farhelm_proto::provisioning::ProvisioningView = round_trip(&runs["runs"][0]);
    assert_eq!(run.host_id, host);
}

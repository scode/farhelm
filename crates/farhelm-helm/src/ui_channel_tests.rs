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
use farhelm_proto::SessionStatus;
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
        session_groups: Vec::new(),
        selected_outside_query: None,
        selected_cut: false,
    }
}

fn empty_sent() -> Sent {
    Sent {
        meta: CollectionMeta {
            availability: Availability::Available,
            order: Vec::new(),
            counts: None,
            session_groups: Vec::new(),
            selected_outside_query: None,
            selected_cut: false,
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

/// The channel reads the selected session only for a key that names one: an
/// empty key (which the list route would read as no `id` at all, serving the
/// whole fleet on every push) selects nothing, like an absent one.
#[farhelm_testtrace::test]
fn an_empty_selected_key_selects_nothing() {
    let query = |selected: Option<&str>| SessionQuery {
        selected: selected.map(str::to_owned),
        ..SessionQuery::default()
    };
    assert_eq!(selected_key(&query(Some("entry:fh-1"))), Some("entry:fh-1"));
    assert_eq!(selected_key(&query(Some(""))), None);
    assert_eq!(selected_key(&query(None)), None);
}

/// The entry carried only because the query selected it must stay out of the
/// list the GUI draws while a lookup by key still finds it, so the diff keeps
/// its key out of the order and names it in the metadata instead. When the
/// query starts to match it, the entry is unchanged, so only the metadata moves
/// (it joins the order and loses the mark), and nothing is resent or removed.
#[farhelm_testtrace::test]
fn the_diff_keeps_the_entry_carried_outside_the_query_out_of_the_order() {
    let mut carried = projected(vec![entry("a", "one"), entry("open", "two")]);
    carried.selected_outside_query = Some("open".into());
    let (first, sent) = diff(Collection::Sessions, &empty_sent(), Ok(carried));
    let first = first.expect("a first read is a change");
    assert_eq!(first.meta.order, ["a"]);
    assert_eq!(first.meta.selected_outside_query.as_deref(), Some("open"));
    assert_eq!(
        first.upsert,
        vec![keyed("a", "one"), keyed("open", "two")],
        "the carried entry is sent like any other"
    );

    let (listed, _) = diff(
        Collection::Sessions,
        &sent,
        Ok(projected(vec![entry("a", "one"), entry("open", "two")])),
    );
    let listed = listed.expect("the mark moving is a change");
    assert!(listed.upsert.is_empty() && listed.remove.is_empty());
    assert_eq!(listed.meta.order, ["a", "open"]);
    assert_eq!(listed.meta.selected_outside_query, None);
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
/// the pushed list itself, and a refusal marked with helm-only headers
/// ([`marked`]).
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
        .route("/api/test/marked/{which}", axum::routing::post(marked))
        .fallback(unrouted)
        .with_state(routes)
}

/// `POST /api/test/marked/{which}`: a 409 refusal whose headers are chosen by
/// `which`, standing in for a route that marks its refusal. `none` sets no
/// marker header; a number sets exactly the header of that marker in
/// [`HelmMarker::ALL`]; `all` sets every one; `foreign` sets every marker's
/// header with a value no marker has, plus a helm header that is not a marker.
/// The route is the only place a header is chosen: the channel under test is
/// told nothing about it.
async fn marked(
    axum::extract::Path(which): axum::extract::Path<String>,
) -> axum::response::Response {
    let markers: Vec<HelmMarker> = match which.as_str() {
        "none" | "foreign" => Vec::new(),
        "all" => HelmMarker::ALL.to_vec(),
        index => vec![HelmMarker::ALL[index.parse::<usize>().expect("a marker's index")]],
    };
    let mut response = (StatusCode::CONFLICT, "refused").into_response();
    let headers = response.headers_mut();
    for marker in markers {
        let (name, value) = marker.header();
        headers.insert(name, HeaderValue::from_static(value));
    }
    if which == "foreign" {
        for marker in HelmMarker::ALL {
            headers.insert(
                marker.header().0,
                HeaderValue::from_static("something-else"),
            );
        }
        headers.insert(
            farhelm_proto::http::BUILD_STAMP_HEADER,
            HeaderValue::from_static("0.0.0-test"),
        );
    }
    response
}

/// The one collection the ordering tests push: the test routes' list.
fn test_collection() -> CollectionSpec {
    CollectionSpec {
        collection: Collection::Approvals,
        path: |_| "/api/test/items".to_string(),
        split: |body| split_keyed_array(body, "items", "id"),
        selected: None,
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
    drive_channel(channel).await
}

/// Exercise command timing without a network socket on the GUI side.
async fn drive_channel(channel: Channel) -> (Driven, ServerMessage) {
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
        method: serde_json::from_value(Value::String(method.to_string()))
            .expect("a test command's method is one the wire knows"),
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

/// The reply to the next command, skipping any changes pushed before it.
async fn next_reply(driven: &mut Driven) -> Reply {
    loop {
        if let ServerMessage::Reply(reply) = next(driven).await {
            return reply;
        }
    }
}

/// The GUI branches on facts only the helm's own code states, in headers
/// (the launcher's YOLO question, a launch waiting out a busy host, the
/// environment Delete's escalation), and a reply that dropped them left it
/// matching the refusal's words, which can quote a supervisor. The channel
/// must carry them with one lookup over the closed set of helm-only headers,
/// whatever route set them. Specified against a stand-in route the channel
/// knows nothing about: a refusal with exactly one marker's header carries
/// exactly that marker, for every marker; one with all of them carries all,
/// in `HelmMarker::ALL`'s order; one with every marker header holding a
/// foreign value, plus a helm header that is no marker, carries none; one
/// with none carries none; and the status and body travel unchanged
/// throughout.
#[farhelm_testtrace::test]
async fn each_helm_marker_header_reaches_the_reply_and_nothing_else_does() {
    let routes = Arc::new(TestRoutes::default());
    let (mut driven, _snapshot) = drive(routes).await;
    let mut id = 0;
    for (index, marker) in HelmMarker::ALL.into_iter().enumerate() {
        id += 1;
        let markers = marked_reply(&mut driven, id, &index.to_string()).await;
        assert_eq!(markers, [marker], "{marker:?} alone");
    }
    let markers = marked_reply(&mut driven, id + 1, "all").await;
    assert_eq!(markers, HelmMarker::ALL, "every marker, in order");
    let markers = marked_reply(&mut driven, id + 2, "foreign").await;
    assert_eq!(
        markers,
        Vec::<HelmMarker>::new(),
        "a foreign value is no marker"
    );
    let markers = marked_reply(&mut driven, id + 3, "none").await;
    assert_eq!(markers, Vec::<HelmMarker>::new(), "no header, no marker");
}

/// Send command `id` to the [`marked`] route for `which`, check that its
/// reply relays the route's status and body, and return the reply's markers.
async fn marked_reply(driven: &mut Driven, id: u64, which: &str) -> Vec<HelmMarker> {
    let path = format!("/api/test/marked/{which}");
    driven
        .to_channel
        .send(command(id, "POST", &path, None))
        .await
        .unwrap();
    let reply = next_reply(driven).await;
    assert_eq!(reply.id, id, "{which}");
    assert_eq!(
        reply.status, 409,
        "the status travels beside the markers: {which}"
    );
    assert_eq!(reply.body, Some(Value::String("refused".into())), "{which}");
    reply.markers
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

/// A command's method is a closed set on the wire, so one outside it
/// (`OPTIONS`, or `get` in the wrong case) does not decode, and the channel
/// closes unrouted as for any message it cannot read, like a non-canonical
/// path. Before the method was an enum it was a string the helm matched
/// itself, answering a 400 and carrying on; this pins that the closed set is
/// enforced where commands are decoded. Each method gets a fresh channel,
/// whose canonical command before it is routed, so the close is the method's
/// doing, not a dead channel.
#[farhelm_testtrace::test]
async fn a_command_whose_method_is_not_on_the_wire_ends_the_channel_unrouted() {
    for method in ["OPTIONS", "get"] {
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

        bridged.send_text(&format!(
            r#"{{"type":"command","id":2,"method":"{method}","path":"/api/test/value","body":"overwritten"}}"#
        ));
        tokio::time::timeout(NOTICE, bridged.task)
            .await
            .unwrap_or_else(|_| panic!("a command with the method {method:?} ends the channel"))
            .unwrap();
        assert_eq!(
            routes.value.lock().unwrap().as_deref(),
            Some("\"kept\""),
            "the {method:?} command reached no route"
        );
        assert_eq!(
            routes.unrouted_hits.load(Ordering::SeqCst),
            0,
            "the {method:?} command was never routed"
        );
        assert!(!seat.is_held(), "the ended channel gave the seat up");
    }
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

/// A supervisor that holds a tab reply must leave the channel responsive.
/// Each request reaches the peer, the channel replies before the desktop's
/// 30-second watchdog, and later commands can still use the same channel.
#[farhelm_testtrace::test(start_paused = true)]
async fn stalled_tab_requests_reply_before_the_ui_watchdog() {
    use farhelm_proto::ControlMsg;
    let (client, supervisor) = tokio::io::duplex(64 * 1024);
    let (harness, mut peer) = tokio::join!(
        rest_harness::spliced_helm(client),
        rest_harness::FakeSupervisor::accept(supervisor)
    );
    let secret = harness.state.auth.mint_device().unwrap();
    let channel = Channel::new(
        crate::api_router(Arc::clone(&harness.state)),
        HeaderValue::from_str(&format!("Bearer {secret}")).unwrap(),
        vec![],
    );
    let (mut driven, _) = drive_channel(channel).await;
    let session = harness.local_session_ref("sess-1");
    for (id, method, tab) in [(1, "POST", None), (2, "DELETE", Some("tab-1"))] {
        let watchdog = tokio::time::Instant::now() + Duration::from_secs(30);
        let mut segments = vec!["sessions", &session, "tabs"];
        segments.extend(tab);
        let path = farhelm_proto::ui_channel::ApiPath::api(&segments).unwrap();
        let request = command(id, method, path.as_str(), None);
        driven.to_channel.try_send(request).unwrap();
        let request = tokio::time::timeout_at(watchdog, peer.recv())
            .await
            .expect("the request reaches the supervisor");
        assert!(matches!(
            (id, request),
            (1, ControlMsg::OpenTab { .. }) | (2, ControlMsg::CloseTab { .. })
        ));
        let Some(ServerMessage::Reply(reply)) =
            tokio::time::timeout_at(watchdog, driven.from_channel.recv())
                .await
                .expect("the tab command replies before the watchdog")
        else {
            panic!("expected the tab command's reply");
        };
        assert!(tokio::time::Instant::now() < watchdog);
        assert_eq!((reply.id, reply.status), (id, 504));
        let body = reply.body.as_ref().and_then(Value::as_str).unwrap();
        assert!(body.contains("still being carried out"));
    }
}

/// The bound is on the real routes, not only on the helper: a rename, an
/// ordinary Delete, a Stop, a Restart and a Replace against a host that never
/// answers (the scripted fixture host answers none of them) reply 504 once
/// `HOST_REPLY_BOUND` passes. Removing the bound from any of these routes
/// makes this hang until nextest's timeout.
#[farhelm_testtrace::test(start_paused = true)]
async fn lifecycle_routes_answer_504_when_the_host_never_answers() {
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
    for (action, body) in [
        ("stop", serde_json::json!({})),
        ("restart", serde_json::json!({ "stop_if_running": false })),
        ("replace", serde_json::json!({})),
    ] {
        let status = http(
            &harness,
            "POST",
            &format!("/api/sessions/{full}/{action}"),
            body,
        )
        .await;
        assert_eq!(status, StatusCode::GATEWAY_TIMEOUT, "{action}");
    }
}

/// The launcher's create and its host reads are bounded too, so a launch or a
/// folder check against a host that never answers cannot hold the GUI's
/// ordered channel: a create, a Browse, an automatic folder check
/// (`membership_only`) and a checkout preview reply 504 once
/// `HOST_REPLY_BOUND` passes, and a repository search still answers, its
/// suggestions marked incomplete and its scan error naming the timeout rather
/// than telling the user to check Git (SPEC.md "Managed checkouts": an
/// incomplete scan never blocks a typed repository). The fixture host
/// answers none of these, and the checkout folder is configured so the
/// preview and the search reach the host instead of refusing first. Removing
/// a bound makes this hang until nextest's timeout.
#[farhelm_testtrace::test(start_paused = true)]
async fn launcher_routes_answer_within_the_bound_when_the_host_never_answers() {
    let harness = rest_harness::helm_listing(vec![rest_harness::session("s1", 100)]).await;
    let host = rest_harness::local_id(&harness.store).await;
    harness
        .store
        .set_checkout_root(None, "/checkouts")
        .await
        .expect("a valid checkout root");
    let create = http(
        &harness,
        "POST",
        "/api/sessions",
        serde_json::json!({
            "host": host,
            "cwd": "/tmp",
            "command": { "command": "true", "yolo": false, "agent": null, "resume": null },
        }),
    )
    .await;
    assert_eq!(create, StatusCode::GATEWAY_TIMEOUT, "create");
    for (route, body) in [
        (
            "/api/browse-directory",
            serde_json::json!({ "host": host, "cwd": "/tmp" }),
        ),
        (
            "/api/browse-directory",
            serde_json::json!({ "host": host, "cwd": "/tmp", "membership_only": true }),
        ),
        (
            "/api/github-checkout-preview",
            serde_json::json!({ "host": host, "repo": "owner/repo" }),
        ),
    ] {
        let status = http(&harness, "POST", route, body.clone()).await;
        assert_eq!(status, StatusCode::GATEWAY_TIMEOUT, "{route} {body}");
    }
    let request = Request::builder()
        .method("POST")
        .uri("/api/github-repositories")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "host": host, "query": "" }).to_string(),
        ))
        .unwrap();
    let response = harness.router().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "repository search");
    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["truncated"], true, "{body}");
    let scan_error = body["scan_error"].as_str().unwrap_or_default();
    assert!(
        scan_error.contains("did not answer"),
        "the scan error names the timeout: {body}"
    );
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
/// templates, notices, host progress, recipes and their places) are
/// available, empty collections.
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
            Collection::SshQuestions,
            Collection::Templates,
            Collection::Notices,
            Collection::Announcements,
            Collection::HostProgress,
            Collection::Recipes,
            Collection::RecipePlaces,
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
    // A channel opens in most recent activity order; with both sessions
    // running and no work burst recorded, that is creation time, newest first.
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
    for empty in [
        Collection::Approvals,
        Collection::SshQuestions,
        Collection::Templates,
        Collection::Notices,
        Collection::HostProgress,
        Collection::Recipes,
        Collection::RecipePlaces,
    ] {
        let collection = collections
            .iter()
            .find(|state| state.collection == empty)
            .unwrap();
        assert_eq!(
            collection.meta.availability,
            Availability::Available,
            "{:?}",
            empty
        );
        assert!(collection.entries.is_empty());
    }
    let announcements = collections
        .iter()
        .find(|state| state.collection == Collection::Announcements)
        .unwrap();
    assert_eq!(announcements.meta.order, [ANNOUNCEMENTS_KEY]);
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
    let queue = entry_of::<AnnouncementQueue>(
        snapshot_entries(&collections, Collection::Announcements),
        ANNOUNCEMENTS_KEY,
    );
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

/// A template saved outside the channel (an HTTP write) reaches an idle
/// connected UI through the feed bump, keyed by the template's name. With the
/// channel idle, nothing but the bump in `templates::store_template` (shared
/// with the agent relay's writes) can deliver it.
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
    let entry = &snapshot_entries(&collections, Collection::Notices)[0];
    let notice: farhelm_proto::notices::Notice =
        serde_json::from_value(entry.value.clone()).expect("a notice");
    assert!(!notice.read, "premise: the notice is unread");
    assert_eq!(entry.key, notice.id.to_string());
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
            outcome_unknown: false,
        })
        .await;
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let ServerMessage::Snapshot { collections } = recv_message(&mut ws).await else {
        panic!("the first message is the snapshot");
    };
    let entries = snapshot_entries(&collections, Collection::HostProgress);
    let before = entry_of::<ProvisioningView>(entries, &host.to_string());
    assert_eq!(before.steps[0].status, StepStatus::Pending, "premise");
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

/// Stand-in commands for the helm's Podman check: the check's first command
/// (`uname -s`) waits until the test releases it, then reports a host that is
/// not Linux, which ends the check refused with a known reason and no further
/// command. Each command is recorded rather than asserted here: a panic would
/// end the helm's spawned check task, and the test would fail far from the cause.
struct HeldPlaceCommands {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    seen: std::sync::Mutex<Vec<Vec<String>>>,
}

#[async_trait::async_trait]
impl crate::environment::podman_probe::PlaceCommands for HeldPlaceCommands {
    async fn run(
        &self,
        _route: &crate::access::BaseRoute,
        argv: &[String],
    ) -> anyhow::Result<String> {
        self.seen.lock().unwrap().push(argv.to_vec());
        self.entered.notify_one();
        self.release.notified().await;
        Ok("Darwin\n".to_string())
    }
}

/// The `container_check` of host `id`'s entry among `entries`, decoded as the
/// GUI decodes a hosts entry.
fn container_check_of(
    entries: &[KeyedEntry],
    id: crate::store::HostId,
) -> Option<farhelm_proto::ui_channel::ContainerCheckView> {
    let entry = entries
        .iter()
        .find(|entry| entry.key == farhelm_proto::ui_channel::HostView::key(id))
        .expect("the host's entry is among the hosts sent");
    let host: farhelm_proto::ui_channel::HostView =
        serde_json::from_value(entry.value.clone()).expect("a host entry");
    host.container_check
}

/// Settings → Containers shows each host's Podman check, and the check used to
/// change only the helm's private cache, so a connected GUI saw it finish only
/// by re-reading on a timer. Its state now rides the host's entry in the hosts
/// collection, and both of its changes bump the feed.
///
/// Specified, with the check started outside the channel (so only a feed bump
/// can bring the change) and the local host's actor settled first (so no
/// connection change bumps the feed meanwhile): the snapshot carries an
/// unchecked `container_check`; starting a check pushes the entry with
/// `refreshing` on and no result yet; the check's end pushes it again with
/// `refreshing` off and the stored result: refused, with the check's reason and
/// its time. Each bump is announced only once the state it announces is in
/// place: seen from inside each bump, `refreshing` reads on, then off. That
/// order is invisible to the channel (no await point separates storing the
/// result from clearing the flag), so the bump observer is what pins it.
#[farhelm_testtrace::test]
async fn a_container_check_reaches_the_connected_ui_when_it_starts_and_when_it_ends() {
    use farhelm_proto::ui_channel::ContainerCheckView;
    let commands = Arc::new(HeldPlaceCommands {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
        seen: std::sync::Mutex::new(Vec::new()),
    });
    let mut harness = rest_harness::FleetBuilder::new()
        .await
        .place_commands(commands.clone())
        .start()
        .await;
    let local = rest_harness::local_id(&harness.store).await;
    harness
        .await_state(local, |state| state.phase() == "unreachable-reprobing")
        .await;
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let ServerMessage::Snapshot { collections } = recv_message(&mut ws).await else {
        panic!("the first message is the snapshot");
    };
    assert_eq!(collections[1].collection, Collection::Hosts, "premise");
    assert_eq!(
        container_check_of(&collections[1].entries, local),
        Some(ContainerCheckView {
            checked_at: None,
            refreshing: false,
            usable: false,
            reason: None,
        }),
        "premise: the local host can hold containers and was never checked"
    );

    // What a re-read would see at the instant of each bump. The publisher holds
    // no cache lock while bumping, so the observer may take it.
    let row = harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == local)
        .unwrap();
    let places = Arc::clone(&harness.state.podman_places);
    let at_bumps = Arc::new(std::sync::Mutex::new(Vec::new()));
    let record = Arc::clone(&at_bumps);
    harness.manager.events().observe_bumps(move || {
        record
            .lock()
            .unwrap()
            .push(places.view(&row).unwrap().refreshing);
    });

    let status = http(
        &harness,
        "POST",
        &format!("/api/hosts/{local}/podman-probe"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    tokio::time::timeout(NOTICE, commands.entered.notified())
        .await
        .expect("the check runs its first command, where it is held");
    let started = changes_of(&mut ws, Collection::Hosts).await;
    assert_eq!(
        container_check_of(&started.upsert, local),
        Some(ContainerCheckView {
            checked_at: None,
            refreshing: true,
            usable: false,
            reason: None,
        })
    );

    commands.release.notify_one();
    let ended = changes_of(&mut ws, Collection::Hosts).await;
    let check = container_check_of(&ended.upsert, local).expect("the entry keeps its check");
    assert!(!check.refreshing, "{check:?}");
    assert!(!check.usable, "{check:?}");
    assert_eq!(
        check.reason.as_deref(),
        Some("containers require a Linux host")
    );
    assert!(check.checked_at.is_some(), "{check:?}");
    assert_eq!(
        *at_bumps.lock().unwrap(),
        [true, false],
        "each bump shows the state it announces"
    );
    assert_eq!(
        *commands.seen.lock().unwrap(),
        [["uname", "-s"]],
        "the check ran only its first command"
    );
}

/// Stand-in commands for the helm's Podman check that record every command
/// and refuse it, so a test can show that nothing it did started a check.
#[derive(Default)]
struct RecordedPlaceCommands {
    seen: std::sync::Mutex<Vec<Vec<String>>>,
}

#[async_trait::async_trait]
impl crate::environment::podman_probe::PlaceCommands for RecordedPlaceCommands {
    async fn run(
        &self,
        _route: &crate::access::BaseRoute,
        argv: &[String],
    ) -> anyhow::Result<String> {
        self.seen.lock().unwrap().push(argv.to_vec());
        anyhow::bail!("this test runs no Podman check")
    }
}

/// A helm as production assembles its recipe places (the production Podman
/// provider over the helm's own check cache, here with recorded commands),
/// holding recipe `dev`, with the local host's actor settled first so no
/// connection change bumps the feed meanwhile, and a GUI channel past its
/// snapshot, which is returned with the rest.
async fn recipes_helm() -> (
    rest_harness::Harness,
    Arc<RecordedPlaceCommands>,
    crate::store::HostId,
    WsTestClient,
    Vec<CollectionState>,
) {
    let commands = Arc::new(RecordedPlaceCommands::default());
    let mut harness = rest_harness::FleetBuilder::new()
        .await
        .place_commands(commands.clone())
        .production_podman()
        .start()
        .await;
    crate::tensorlake::recipes::tests_support::create(&harness.store, "dev").await;
    let local = rest_harness::local_id(&harness.store).await;
    harness
        .await_state(local, |state| state.phase() == "unreachable-reprobing")
        .await;
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let ServerMessage::Snapshot { collections } = recv_message(&mut ws).await else {
        panic!("the first message is the snapshot");
    };
    (harness, commands, local, ws, collections)
}

/// The entry filed under `key` among `entries`, decoded as the GUI decodes it.
fn entry_of<T: serde::de::DeserializeOwned>(entries: &[KeyedEntry], key: &str) -> T {
    let entry = entries
        .iter()
        .find(|entry| entry.key == key)
        .unwrap_or_else(|| panic!("{key} is among the entries sent: {entries:?}"));
    serde_json::from_value(entry.value.clone()).expect("the entry decodes as its proto type")
}

/// The snapshot's entries of `collection`.
fn snapshot_entries(collections: &[CollectionState], collection: Collection) -> &[KeyedEntry] {
    &collections
        .iter()
        .find(|state| state.collection == collection)
        .unwrap_or_else(|| panic!("{collection:?} is in the snapshot"))
        .entries
}

/// Settings → Recipes follows each place's build, which takes minutes, and
/// the recipe places list used to be a query, so a connected GUI saw a step
/// only when it read the list again. The list is now pushed, and a place job
/// bumps the feed at each change its entry shows.
///
/// Specified, with the job started outside the channel through the shared
/// build registry (so only feed bumps can bring its changes) and its work
/// held between changes: the snapshot lists recipe `dev` at the local host,
/// not building; admitting the job pushes that entry building with no step;
/// the work's step (published as a real image build publishes it) pushes the
/// step; the job's end pushes the entry neither building nor with a step.
/// Each bump comes once the state it announces is in place: seen from inside
/// each bump, the job is admitted at the first two and gone at the last. The
/// channel cannot see that order (no await point separates removing the job
/// from announcing it), so the bump observer is what pins it.
#[farhelm_testtrace::test]
async fn a_place_jobs_admission_step_and_end_reach_the_connected_ui() {
    use farhelm_proto::{Place, RecipePlaceView};
    let (harness, _commands, local, mut ws, collections) = recipes_helm().await;
    let key = RecipePlaceView::key("dev", Place::PodmanHost(local));
    let before: RecipePlaceView = entry_of(
        snapshot_entries(&collections, Collection::RecipePlaces),
        &key,
    );
    assert!(
        !before.building && before.step.is_none(),
        "premise: {before:?}"
    );

    let builds = Arc::clone(harness.state.recipe_builds());
    let events = Arc::clone(harness.manager.events());
    let at_bumps = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (observed, record) = (Arc::clone(&builds), Arc::clone(&at_bumps));
    events.observe_bumps(move || {
        let admitted = observed.podman_job_active("dev", local);
        record.lock().unwrap().push(admitted);
    });
    let (step, finish) = (
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
    );
    let (step_gate, finish_gate, steps) = (step.clone(), finish.clone(), events.clone());
    builds
        .start_podman("dev", local, true, events, move |progress, _| async move {
            step_gate.notified().await;
            crate::environment::podman::enter_build_step(&progress, &steps, "Installing packages");
            finish_gate.notified().await;
            anyhow::bail!("the test ends the job here")
        })
        .expect("the job is admitted");

    let admitted: RecipePlaceView = entry_of(
        &changes_of(&mut ws, Collection::RecipePlaces).await.upsert,
        &key,
    );
    assert!(admitted.building && admitted.step.is_none(), "{admitted:?}");
    step.notify_one();
    let stepped: RecipePlaceView = entry_of(
        &changes_of(&mut ws, Collection::RecipePlaces).await.upsert,
        &key,
    );
    assert!(stepped.building, "{stepped:?}");
    assert_eq!(stepped.step.as_deref(), Some("Installing packages"));
    finish.notify_one();
    let ended: RecipePlaceView = entry_of(
        &changes_of(&mut ws, Collection::RecipePlaces).await.upsert,
        &key,
    );
    assert!(!ended.building && ended.step.is_none(), "{ended:?}");
    assert_eq!(
        *at_bumps.lock().unwrap(),
        [true, true, false],
        "each bump shows the state it announces"
    );
}

/// The recipe catalog is pushed too, so an edit made outside the channel
/// reaches a connected GUI through the save's feed bump. And re-reading
/// either recipe collection must start nothing: it runs after every bump,
/// so a read that started a Podman check would start one on every bump.
///
/// Specified: the snapshot holds `dev` with its two packages; a save outside
/// the channel pushes `dev` with the saved package list; and after the
/// snapshot's and that push's reads, the local host's Podman check is neither
/// running nor recorded, no check command ran, and no build or place job is
/// admitted for the recipe.
#[farhelm_testtrace::test]
async fn a_recipe_edit_reaches_the_connected_ui_and_the_reads_start_nothing() {
    use farhelm_proto::recipes::RecipeView;
    let (harness, commands, local, mut ws, collections) = recipes_helm().await;
    let before: RecipeView = entry_of(snapshot_entries(&collections, Collection::Recipes), "dev");
    assert_eq!(before.packages, ["git", "ripgrep"], "premise");

    let mut save = crate::tensorlake::recipes::tests_support::save();
    save.packages.push("jq".into());
    let body = serde_json::to_value(&save).unwrap();
    let status = http(&harness, "PUT", "/api/tensorlake/recipes/dev", body).await;
    assert_eq!(status, StatusCode::OK);
    let edited: RecipeView = entry_of(
        &changes_of(&mut ws, Collection::Recipes).await.upsert,
        "dev",
    );
    assert_eq!(edited.packages, ["git", "ripgrep", "jq"]);

    let hosts = harness.store.list_hosts().await.unwrap();
    let row = hosts.iter().find(|row| row.id == local).unwrap();
    let check = harness.state.podman_places.view(row).unwrap();
    assert!(!check.refreshing && check.checked_at.is_none(), "{check:?}");
    assert!(
        commands.seen.lock().unwrap().is_empty(),
        "no check command ran"
    );
    let builds = harness.state.recipe_builds();
    assert!(!builds.is_building("dev") && !builds.podman_job_active("dev", local));
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

/// The answer crosses the same routed UI command path used in the app and reaches its waiting attempt exactly once.
/// A malformed answer and a late duplicate also cross that path. No UI message or tracing field can echo the answer.
#[farhelm_testtrace::test]
async fn ssh_answers_reach_only_the_waiting_attempt() {
    use crate::askpass::Answer;
    use farhelm_proto::ui_channel::{SshConnectionPurpose, SshQuestion};

    let mut harness = rest_harness::idle_helm().await;
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let ServerMessage::Snapshot { collections } = recv_message(&mut ws).await else {
        panic!("the UI must be connected before the question appears");
    };
    assert!(snapshot_entries(&collections, Collection::SshQuestions).is_empty());

    let questions = Arc::clone(&harness.state.ssh_questions);
    questions.publish_to(Arc::clone(harness.state.manager.events()));
    let question = SshQuestion {
        id: "test-ssh-question".into(),
        host: 42,
        purpose: SshConnectionPurpose::Sessions,
        startup: false,
        prompt: "Verification code:".into(),
        notice: false,
        expires_at: crate::manager::unix_millis() + 120_000,
    };
    let mut pending = questions.open(
        question.clone(),
        tokio::time::Instant::now() + Duration::from_secs(120),
    );
    assert_eq!(
        entry_of::<SshQuestion>(
            &changes_of(&mut ws, Collection::SshQuestions).await.upsert,
            &question.id
        ),
        question
    );

    const ANSWER: &str = "ssh-test-answer-only-7fc21";
    let path = format!("/api/ssh-questions/{}", question.id);
    let attempt = |id, answer| {
        command(
            id,
            "POST",
            &path,
            Some(serde_json::json!({"action": "send", "answer": answer})),
        )
    };
    send_message(&mut ws, &attempt(1, serde_json::json!([ANSWER]))).await;
    let ServerMessage::Reply(refused) = recv_message(&mut ws).await else {
        panic!("a malformed answer should be refused without changing the list");
    };
    assert_eq!((refused.id, refused.status, refused.body), (1, 422, None));

    send_message(&mut ws, &attempt(2, serde_json::json!(ANSWER))).await;
    let removed = changes_of(&mut ws, Collection::SshQuestions).await;
    assert_eq!(removed.remove, [question.id]);
    assert!(removed.upsert.is_empty());
    let ServerMessage::Reply(accepted) = recv_message(&mut ws).await else {
        panic!("a delivered answer has an empty receipt");
    };
    assert_eq!(
        (accepted.id, accepted.status, accepted.body),
        (2, 204, None)
    );
    let delivered = tokio::time::timeout(NOTICE, &mut pending.reply)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(delivered, Answer::Send(ref answer) if answer == ANSWER));

    send_message(&mut ws, &attempt(3, serde_json::json!(ANSWER))).await;
    let ServerMessage::Reply(stale) = recv_message(&mut ws).await else {
        panic!("a stale answer should be refused");
    };
    assert_eq!((stale.id, stale.status, stale.body), (3, 404, None));
    assert!(
        crate::test_capture::current()
            .matching_events(|event| {
                event
                    .fields
                    .values()
                    .chain(event.span_fields.values())
                    .any(|text| text.contains(ANSWER))
            })
            .unwrap()
            .is_empty(),
        "an SSH answer appeared in the UI channel's traces"
    );
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
/// displaced.
#[farhelm_testtrace::test]
async fn approvals_follow_the_seat_through_connect_disconnect_and_takeover() {
    let mut harness = rest_harness::idle_helm().await;
    let addr = harness.serve().await;
    assert!(!crate::approvals::gui_connected(&harness.state));

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

/// Collapse can change the sidebar even when a client has no rows in the
/// group. The roster must reach that client as a metadata-only push.
#[farhelm_testtrace::test]
async fn a_session_group_collapse_pushes_metadata_to_a_client_that_filtered_it_out() {
    use farhelm_proto::session_requests::UpdateSessionGroup;

    let mut harness = rest_harness::helm_listing(vec![
        rest_harness::session("alpha", 100),
        rest_harness::session("beta", 200),
    ])
    .await;
    let host = rest_harness::local_id(&harness.store).await;
    let group = harness
        .store
        .create_session_group(
            "Beta".into(),
            vec![crate::store::SessionKey::new(host, "beta")],
        )
        .await
        .unwrap();
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let _snapshot = recv_message(&mut ws).await;
    let changes = set_query(&mut ws, 1, serde_json::json!({ "title": "alpha" })).await;
    assert_eq!(changes.meta.order, [harness.local_session_ref("alpha")]);
    assert_eq!(changes.meta.session_groups.len(), 1);
    assert_eq!(changes.meta.session_groups[0].id, group);
    assert!(!changes.meta.session_groups[0].collapsed);

    assert!(
        harness
            .store
            .update_session_group(
                group,
                UpdateSessionGroup {
                    collapsed: Some(true),
                    ..Default::default()
                }
            )
            .await
            .unwrap()
    );
    harness.manager.events().bump();
    let push = changes_of(&mut ws, Collection::Sessions).await;
    assert!(push.upsert.is_empty());
    assert!(push.remove.is_empty());
    assert_eq!(push.meta.order, changes.meta.order);
    assert_eq!(push.meta.session_groups[0].id, group);
    assert!(push.meta.session_groups[0].collapsed);
}

/// The markers come off the real router's responses, middleware and all, not
/// only off stand-in routes: an ask-first YOLO launch sent over a real channel
/// is refused 409 carrying the YOLO marker and the never-accepted marker the
/// helm sets beside it. That pair is what lets the launcher ask its YOLO
/// question, and know the launch did not start, instead of reading the
/// refusal's words.
#[farhelm_testtrace::test]
async fn a_yolo_launch_refused_over_a_real_channel_carries_its_markers() {
    let mut harness = rest_harness::idle_helm().await;
    let local = rest_harness::local_id(&harness.store).await;
    let host = harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|host| host.id == local)
        .expect("the local host is registered");
    assert!(
        !host.yolo_without_asking,
        "premise: the local host asks before YOLO launches"
    );
    let addr = harness.serve().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let _snapshot = recv_message(&mut ws).await;
    send_message(
        &mut ws,
        &command(
            1,
            "POST",
            "/api/sessions",
            Some(serde_json::json!({
                "cwd": "/project",
                "command": { "command": "sleep 300", "yolo": true },
            })),
        ),
    )
    .await;
    let reply = loop {
        if let ServerMessage::Reply(reply) = recv_message(&mut ws).await {
            break reply;
        }
    };
    assert_eq!((reply.id, reply.status), (1, 409), "{:?}", reply.body);
    assert_eq!(
        reply.markers,
        [HelmMarker::CreateUnaccepted, HelmMarker::YoloConfirmation]
    );
}

/// Send `query` as this channel's session query and return the session list's
/// changes it pushed, checking that the reply that follows accepts it.
///
/// Every caller expects the query to change the list, and the channel pushes
/// a command's changes before its reply, so a reply that arrives first fails
/// at once, naming that cause, rather than leaving the test to time out
/// waiting for a push that is never coming.
async fn set_query(ws: &mut WsTestClient, id: u64, query: Value) -> CollectionChanges {
    send_message(ws, &command(id, "PUT", SESSION_QUERY_PATH, Some(query))).await;
    let changes = loop {
        match recv_message(ws).await {
            ServerMessage::Changes(changes) if changes.collection == Collection::Sessions => {
                break changes;
            }
            ServerMessage::Reply(reply) => panic!(
                "the query's reply ({}, status {}) came with no session-list change",
                reply.id, reply.status
            ),
            _ => continue,
        }
    };
    let ServerMessage::Reply(reply) = recv_message(ws).await else {
        panic!("the reply follows the changes");
    };
    assert_eq!((reply.id, reply.status), (id, 204), "the query is accepted");
    changes
}

/// The open session's pane, header and Rename read its row from the pushed
/// session list, so the list must carry that session while a filter leaves it
/// out (SPEC.md keeps the selection then), and carry it outside the list: out
/// of the order, marked in the metadata, and in none of the counts. Specifies
/// it through every way the carried entry moves: the query selecting a
/// filtered-out session; the filter matching it, which lists it once and
/// unmarked; the selection moving and clearing; a selected key that names
/// nothing, which carries nothing and leaves the list available; and the
/// session's deletion, which removes it.
#[farhelm_testtrace::test]
async fn the_open_session_rides_outside_a_filter_that_leaves_it_out() {
    let mut harness = rest_harness::helm_listing(vec![
        rest_harness::session("alpha", 100),
        rest_harness::session("beta", 200),
    ])
    .await;
    let addr = harness.serve().await;
    let alpha = harness.local_session_ref("alpha");
    let beta = harness.local_session_ref("beta");
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let _snapshot = recv_message(&mut ws).await;
    let narrowed = Some(ListCounts {
        total: 2,
        matching: Some(1),
        truncated: false,
    });

    // The filter leaves the open session out: it stays, carried and marked.
    let changes = set_query(
        &mut ws,
        1,
        serde_json::json!({ "title": "alp", "selected": beta }),
    )
    .await;
    assert_eq!(changes.meta.order, std::slice::from_ref(&alpha));
    assert_eq!(changes.meta.selected_outside_query, Some(beta.clone()));
    assert_eq!(changes.meta.counts, narrowed, "the counts are the filter's");
    assert!(changes.remove.is_empty(), "the open session is not removed");

    // The filter matches it: listed once, unmarked.
    let changes = set_query(
        &mut ws,
        2,
        serde_json::json!({ "title": "bet", "selected": beta }),
    )
    .await;
    assert_eq!(changes.meta.order, std::slice::from_ref(&beta));
    assert_eq!(changes.meta.selected_outside_query, None);
    assert_eq!(changes.meta.counts, narrowed);
    assert_eq!(changes.remove, std::slice::from_ref(&alpha));

    // The selection moves to a session the filter leaves out.
    let changes = set_query(
        &mut ws,
        3,
        serde_json::json!({ "title": "bet", "selected": alpha }),
    )
    .await;
    assert_eq!(changes.meta.order, std::slice::from_ref(&beta));
    assert_eq!(changes.meta.selected_outside_query, Some(alpha.clone()));
    let upserted: Vec<&str> = changes.upsert.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(upserted, [alpha.as_str()]);

    // The selection clears: the carried session goes.
    let changes = set_query(&mut ws, 4, serde_json::json!({ "title": "bet" })).await;
    assert_eq!(changes.meta.selected_outside_query, None);
    assert_eq!(changes.remove, std::slice::from_ref(&alpha));

    // A key that names nothing is accepted and carries nothing.
    send_message(
        &mut ws,
        &command(
            5,
            "PUT",
            SESSION_QUERY_PATH,
            Some(serde_json::json!({ "title": "bet", "selected": "not a session" })),
        ),
    )
    .await;
    let ServerMessage::Reply(reply) = recv_message(&mut ws).await else {
        panic!("an unchanged list sends no changes, only the reply");
    };
    assert_eq!((reply.id, reply.status), (5, 204));

    // The carried session is deleted: it goes, and the list stays available.
    let changes = set_query(
        &mut ws,
        6,
        serde_json::json!({ "title": "bet", "selected": alpha }),
    )
    .await;
    assert_eq!(changes.meta.selected_outside_query, Some(alpha.clone()));
    let local = rest_harness::local_id(&harness.store).await;
    harness.fleet.edit(local, |script| {
        script.sessions.retain(|session| session.id != "alpha");
    });
    harness.refresh_to_completion(local).await;
    let changes = changes_of(&mut ws, Collection::Sessions).await;
    assert_eq!(changes.remove, std::slice::from_ref(&alpha));
    assert_eq!(changes.meta.availability, Availability::Available);
    assert_eq!(changes.meta.selected_outside_query, None);
    assert_eq!(changes.meta.order, std::slice::from_ref(&beta));
}

/// The listing cap can leave the open session out as surely as a filter can
/// (under creation order a fleet past the cap loses its oldest sessions), and
/// the pane needs it just the same. With one session more than the cap, the
/// open one being the oldest, the list is cut and carries it outside the
/// order, and the counts still describe the cut list alone. The fleet is two
/// hosts because one host may list at most the cap itself.
#[farhelm_testtrace::test]
async fn the_open_session_rides_outside_the_listing_cap() {
    let cap = farhelm_proto::LIST_SESSIONS_CAP;
    let filler = (1..=cap)
        .map(|n| rest_harness::session(&format!("s{n:04}"), 100 + n as i64))
        .collect();
    let (builder, other) = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("local-identity".to_string()),
            sessions: filler,
            ..rest_harness::HostScript::default()
        })
        .await
        .ssh(
            "oldest.example",
            rest_harness::HostScript {
                identity: Some("oldest-identity".to_string()),
                sessions: vec![rest_harness::session("oldest", 50)],
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let mut harness = builder.start().await;
    let local = rest_harness::local_id(&harness.store).await;
    harness.await_refreshed(local).await;
    harness.await_refreshed(other).await;
    let addr = harness.serve().await;
    let oldest = harness.session_ref(other, "oldest");
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let ServerMessage::Snapshot { collections } = recv_message(&mut ws).await else {
        panic!("the first message is the snapshot");
    };
    let sessions = collections
        .iter()
        .find(|collection| collection.collection == Collection::Sessions)
        .expect("the snapshot holds the session list");
    assert!(
        sessions.entries.iter().all(|entry| entry.key != oldest),
        "premise: the cap cut the oldest session from the opening list"
    );
    assert_eq!(
        sessions.meta.counts.map(|counts| counts.truncated),
        Some(true),
        "premise: the opening list is truncated"
    );

    let changes = set_query(
        &mut ws,
        1,
        serde_json::json!({ "sort": "created", "selected": oldest }),
    )
    .await;
    assert_eq!(changes.meta.order.len(), cap);
    assert!(!changes.meta.order.contains(&oldest), "the cap cut it");
    assert_eq!(changes.meta.selected_outside_query, Some(oldest.clone()));
    assert!(
        changes.upsert.iter().any(|entry| entry.key == oldest),
        "the carried session's row is sent"
    );
    assert_eq!(
        changes.meta.counts,
        Some(ListCounts {
            total: cap as u64 + 1,
            matching: None,
            truncated: true,
        })
    );
}

/// Each supervisor lists only its newest sessions (the cap), so an old open
/// session on a host past its cap can exist while no read of that host
/// carries it. Under a host filter naming ANOTHER host, the query's own list
/// is read whole, so without a word about the open session the GUI, which
/// takes a missing open session as deleted only from an uncut listing, would
/// read a cut-away session as deleted. The word must not be the list's
/// `truncated`: that list was read to the end, and the GUI says "could not
/// read the list to the end" from it. Specified, with host A uncut and host B
/// past its cap: the open session cut from B's list is marked `selected_cut`
/// while the host-A counts stay uncut; an open session B does list is
/// carried and not marked, although its own read was cut; and an open
/// session missing from uncut A is not marked although B is past its cap, so
/// a deletion there is still provable.
#[farhelm_testtrace::test]
async fn an_open_session_its_capped_host_cut_away_is_marked_cut_and_the_list_is_not() {
    let (builder, capped) = rest_harness::FleetBuilder::new()
        .await
        .local(rest_harness::HostScript {
            identity: Some("local-identity".to_string()),
            sessions: vec![rest_harness::session("a1", 300)],
            ..rest_harness::HostScript::default()
        })
        .await
        .ssh(
            "capped.example",
            rest_harness::HostScript {
                identity: Some("capped-identity".to_string()),
                sessions: vec![rest_harness::session("b-new", 200)],
                truncated: true,
                ..rest_harness::HostScript::default()
            },
        )
        .await;
    let mut harness = builder.start().await;
    let local = rest_harness::local_id(&harness.store).await;
    harness.await_refreshed(local).await;
    harness.await_refreshed(capped).await;
    let addr = harness.serve().await;
    let a1 = harness.local_session_ref("a1");
    let cut_away = harness.session_ref(capped, "b-old");
    let only_local = SessionQuery {
        host: Some(local),
        ..SessionQuery::default()
    };

    // Premises: host A's own listing is whole, and the cut-away session's
    // own read finds nothing and says it was cut.
    let listing = served(&harness, &session_list_path(&only_local)).await;
    assert_eq!(listing["truncated"], false, "premise: {listing}");
    let carried = served(&harness, &selected_session_path(&cut_away)).await;
    assert_eq!(
        carried["sessions"],
        serde_json::json!([]),
        "premise: {carried}"
    );
    assert_eq!(carried["truncated"], true, "premise: {carried}");
    // The one-session read is scoped to its own host: B's cap does not cut a
    // read of a session on A (`store::SessionFilter::host_scope`).
    let gone_from_a = harness.local_session_ref("gone");
    let gone = served(&harness, &selected_session_path(&gone_from_a)).await;
    assert_eq!(gone["truncated"], false, "{gone}");

    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    let _snapshot = recv_message(&mut ws).await;
    let host_a_counts = Some(ListCounts {
        total: 2,
        matching: Some(1),
        truncated: false,
    });

    let changes = set_query(
        &mut ws,
        1,
        serde_json::json!({ "host": local, "selected": cut_away }),
    )
    .await;
    assert_eq!(changes.meta.order, std::slice::from_ref(&a1));
    assert_eq!(
        changes.meta.selected_outside_query, None,
        "nothing to carry"
    );
    assert!(
        changes.meta.selected_cut,
        "a cut-away open session is no evidence of a deletion"
    );
    assert_eq!(
        changes.meta.counts, host_a_counts,
        "host A's list was read to the end"
    );

    let listed_on_b = harness.session_ref(capped, "b-new");
    let changes = set_query(
        &mut ws,
        2,
        serde_json::json!({ "host": local, "selected": listed_on_b }),
    )
    .await;
    assert_eq!(changes.meta.selected_outside_query, Some(listed_on_b));
    assert!(!changes.meta.selected_cut, "a carried session is not cut");
    assert_eq!(changes.meta.counts, host_a_counts);

    let changes = set_query(
        &mut ws,
        3,
        serde_json::json!({ "host": local, "selected": gone_from_a }),
    )
    .await;
    assert_eq!(changes.meta.selected_outside_query, None);
    assert!(
        !changes.meta.selected_cut,
        "another host's cap must not make a deletion on host A unprovable"
    );
    assert_eq!(changes.meta.counts, host_a_counts);
}

/// The served ids of a snapshot's session list, in served order.
fn snapshot_session_ids(message: ServerMessage) -> (Availability, Vec<String>) {
    let ServerMessage::Snapshot { collections } = message else {
        panic!("the first message is the snapshot");
    };
    let sessions = collections
        .into_iter()
        .find(|collection| collection.collection == Collection::Sessions)
        .expect("the snapshot holds the session list");
    let ids = sessions
        .entries
        .into_iter()
        .map(|entry| entry.key)
        .collect();
    (sessions.meta.availability, ids)
}

/// The three list orders the UI channel's tests tell apart, as served ids.
struct ThreeOrders {
    activity: Vec<String>,
    title: Vec<String>,
    created: Vec<String>,
}

/// A served helm whose three list orders all disagree: `beta` is the oldest
/// but the only running session (activity puts it first), `gamma` is the
/// newest (creation order puts it first), and `alpha` sorts first by title.
async fn three_orders_helm() -> (rest_harness::Harness, std::net::SocketAddr, ThreeOrders) {
    let running = rest_harness::session("beta", 100);
    let mut alpha = rest_harness::session("alpha", 200);
    alpha.status = SessionStatus::Idle;
    let mut gamma = rest_harness::session("gamma", 300);
    gamma.status = SessionStatus::Idle;
    let mut harness = rest_harness::helm_listing(vec![running, alpha, gamma]).await;
    let addr = harness.serve().await;
    let ids = |names: [&str; 3]| -> Vec<String> {
        names
            .iter()
            .map(|name| harness.local_session_ref(name))
            .collect()
    };
    let orders = ThreeOrders {
        activity: ids(["beta", "gamma", "alpha"]),
        title: ids(["alpha", "beta", "gamma"]),
        created: ids(["gamma", "alpha", "beta"]),
    };
    assert_ne!(
        orders.activity, orders.created,
        "premise: activity order differs from creation order"
    );
    (harness, addr, orders)
}

/// Store `list_sort` through the preferences route, as any HTTP caller
/// would: the route's write is what bumps the fleet feed.
async fn store_list_sort(harness: &rest_harness::Harness, word: &str) {
    let request = Request::builder()
        .method("PUT")
        .uri("/api/preferences")
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "list_sort": word }).to_string(),
        ))
        .unwrap();
    let status = harness.router().oneshot(request).await.unwrap().status();
    assert_eq!(status, StatusCode::NO_CONTENT, "the preference was stored");
}

/// A channel opens in the list order the helm has stored, so the GUI's first
/// snapshot, and the one after every reset, is already in the order the user
/// picked; before this, every channel opened in creation order and only a
/// GUI-sent query could correct it, which made the list reorder visibly after
/// each connect. With nothing stored, and with a stored word this build does
/// not serve (a row can outlive the build that validated it), the channel
/// opens in most recent activity order, the client default SPEC.md names, and
/// the list stays available rather than failing on the stored word.
#[farhelm_testtrace::test]
async fn a_channel_opens_in_the_stored_list_order_or_most_recent_activity() {
    let (harness, addr, orders) = three_orders_helm().await;
    let stored = |word: &str| -> crate::store::PreferencePatch {
        serde_json::from_value(serde_json::json!({ "list_sort": word })).unwrap()
    };
    let opened = || async {
        let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
        snapshot_session_ids(recv_message(&mut ws).await)
    };

    assert_eq!(
        opened().await,
        (Availability::Available, orders.activity.clone()),
        "nothing stored: most recent activity"
    );

    harness
        .store
        .update_preferences(stored("title"))
        .await
        .unwrap();
    assert_eq!(
        opened().await,
        (Availability::Available, orders.title),
        "a stored order is the one a new channel opens in"
    );

    // The store takes any word; only the route refuses unknown ones, so this
    // plants what a newer build could have left behind.
    harness
        .store
        .update_preferences(stored("sideways"))
        .await
        .unwrap();
    assert_eq!(
        opened().await,
        (Availability::Available, orders.activity),
        "a word this build does not serve: most recent activity, list available"
    );
}

/// SPEC.md ("Session list"): a change to the shared preferences, by an agent
/// or the command line included, reaches the open window as it happens, and
/// the list order is one of them. The channel therefore follows the stored
/// order whenever the GUI's current query names no sort; while it does, that
/// sort is in force and the preference does not move the list.
/// Specified: with a channel open in activity order, storing `title` through
/// the preferences route pushes the sessions in title order; after the GUI's
/// query names `created` (pushed in creation order), storing `activity`
/// pushes no change to the sessions; a later query with no sort pushes the
/// stored `activity` order again, and storing `title` then moves the list.
#[farhelm_testtrace::test]
async fn an_open_channel_follows_the_stored_order_until_the_gui_names_one() {
    let (harness, addr, orders) = three_orders_helm().await;
    let mut ws = WsTestClient::connect(addr, UI_CHANNEL).await;
    assert_eq!(
        snapshot_session_ids(recv_message(&mut ws).await),
        (Availability::Available, orders.activity.clone()),
        "premise: the channel opened in activity order"
    );

    store_list_sort(&harness, "title").await;
    let changes = changes_of(&mut ws, Collection::Sessions).await;
    assert_eq!(
        changes.meta.order,
        orders.title.clone(),
        "the open list follows"
    );

    send_message(
        &mut ws,
        &command(
            1,
            "PUT",
            SESSION_QUERY_PATH,
            Some(serde_json::json!({ "sort": "created" })),
        ),
    )
    .await;
    let changes = changes_of(&mut ws, Collection::Sessions).await;
    assert_eq!(changes.meta.order, orders.created, "the GUI's sort applies");

    store_list_sort(&harness, "activity").await;
    // One re-projection pushes the write's effects, sessions before
    // preferences (the collections' order), so a reorder would arrive before
    // the preferences change that carries the new word.
    loop {
        match recv_message(&mut ws).await {
            ServerMessage::Changes(changes) if changes.collection == Collection::Preferences => {
                break;
            }
            ServerMessage::Changes(changes) => assert_ne!(
                changes.collection,
                Collection::Sessions,
                "the GUI's sort stays in force"
            ),
            ServerMessage::Reply(_) => {}
            other => panic!("unexpected message {other:?}"),
        }
    }

    // A query that names no sort hands the order back to the preference,
    // rather than leaving the list in the list route's default (creation)
    // order, which the GUI's last sort happened to match.
    send_message(
        &mut ws,
        &command(2, "PUT", SESSION_QUERY_PATH, Some(serde_json::json!({}))),
    )
    .await;
    let changes = changes_of(&mut ws, Collection::Sessions).await;
    assert_eq!(
        changes.meta.order, orders.activity,
        "a query with no sort follows the stored order again"
    );
    store_list_sort(&harness, "title").await;
    let changes = changes_of(&mut ws, Collection::Sessions).await;
    assert_eq!(
        changes.meta.order, orders.title,
        "and the preference moves it"
    );
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
        // The schema-85 fields too: they reach the GUI only through this
        // pushed type, so their wire names and number types must survive it.
        .body(Body::from(
            r#"{"compact":true,"list_sort":"title","appearance":"light","sidebar_width":512,
                "host_rail_open":true,"sound_waiting":false,"sound_approval":true,
                "sound_turn_finished":true,"terminal_text_size":16}"#,
        ))
        .unwrap();
    let written = harness.router().oneshot(patch).await.unwrap();
    assert_eq!(written.status(), StatusCode::NO_CONTENT);
    let preferences = served(&harness, "/api/preferences").await;
    let decoded: farhelm_proto::ui_channel::Preferences = round_trip(&preferences);
    assert_eq!(decoded.list_sort.as_deref(), Some("title"));
    assert_eq!(decoded.appearance.as_deref(), Some("light"));
    assert_eq!(decoded.sidebar_width, Some(512));
    assert_eq!(decoded.host_rail_open, Some(true));
    assert_eq!(decoded.sound_waiting, Some(false));
    assert_eq!(decoded.sound_approval, Some(true));
    assert_eq!(decoded.sound_turn_finished, Some(true));
    assert_eq!(decoded.terminal_text_size, Some(16));

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
            outcome_unknown: false,
        })
        .await;
    let runs = served(&harness, "/api/provisioning").await;
    let run: farhelm_proto::provisioning::ProvisioningView = round_trip(&runs["runs"][0]);
    assert_eq!(run.host_id, host);

    // This helm has only the Tensorlake provider, so `dev`'s one place is the
    // Tensorlake account, unusable without a key: its reason is set.
    crate::tensorlake::recipes::tests_support::create(&harness.store, "dev").await;
    let recipes = served(&harness, "/api/tensorlake/recipes").await;
    let recipe: farhelm_proto::recipes::RecipeView = round_trip(&recipes["recipes"][0]);
    assert_eq!(recipe.name, "dev");
    let places = served(&harness, "/api/recipe-places").await;
    let place: farhelm_proto::RecipePlaceView = round_trip(&places[0]);
    assert!(place.reason.is_some(), "{place:?}");
}

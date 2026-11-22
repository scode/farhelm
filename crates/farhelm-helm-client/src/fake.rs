//! A stand-in helm for GUI tests, in process and without a runtime.
//!
//! The GUI is written against [`ChannelHandle`] and [`ChannelEvents`]; a [`FakeHelm`] serves the same pair directly,
//! so a GUI test drives the real GUI code with no sockets, threads or timers. Everything is synchronous and
//! test-driven: the test reads the commands the GUI sent, answers them, pushes changes, and stages a break, a
//! displacement or a refused credential exactly when it wants. Nothing happens unless the test does it, which is
//! what makes the fake safe under GPUI's deterministic test executor.
//!
//! Ordered commands answer in order. The test can answer an unordered command whenever it chooses. Events and
//! replies share one queue, so a test that pushes changes and then
//! replies gets the GUI to see the changes before the reply, as the helm guarantees; and a break announces the new
//! state before the commands it lost fail. Pushed state is decoded the way the channel decodes it, so a test that
//! pushes an entry the GUI could not decode fails in the fake rather than handing the GUI something the real channel
//! never would.
//!
//! NOTE: The fake does not model the watchdog, heartbeats or reconnect backoff; those belong to the real channel's
//! own tests. A "break" here is the outcome of all that: the state the GUI would see and the commands it would lose.
//!
//! [`terminal::FakeTerminal`] does the same for one terminal stream, and [`transfers::FakeTransfers`] for file
//! transfers.

pub mod terminal;
pub mod transfers;

use std::collections::{BTreeMap, VecDeque};

use farhelm_proto::ui_channel::{CollectionChanges, CollectionState, HelmMarker, Reply};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::channel::{
    ChannelEvent, ChannelEvents, ChannelHandle, ChannelState, CommandError, Completion, Item, Lane,
    Method, Request,
};
use crate::collections::{decode_changes, decode_state};

/// A command the GUI sent, as the fake received it.
#[derive(Debug, Clone, PartialEq)]
pub struct FakeCommand {
    pub id: u64,
    pub method: Method,
    pub path: String,
    pub body: Option<Value>,
    /// The effective lane the helm would use for this command.
    pub lane: Lane,
}

/// The user decisions the GUI forwarded, in order. Tests assert on these to check the GUI's buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeDecision {
    TakeBack,
    Retry,
    HelmEnded,
}

pub struct FakeHelm {
    requests: mpsc::UnboundedReceiver<Request>,
    items: mpsc::UnboundedSender<Item>,
    connected: bool,
    next_id: u64,
    /// Commands the test has read and not answered, with the same reply rules as the real channel.
    ordered: VecDeque<(u64, Completion)>,
    unordered: BTreeMap<u64, Completion>,
    decisions: Vec<FakeDecision>,
}

impl FakeHelm {
    /// A fake whose channel starts `Connecting`, with the handle and feed the GUI under test should get.
    pub fn new() -> (FakeHelm, ChannelHandle, ChannelEvents) {
        let (requests_tx, requests) = mpsc::unbounded_channel();
        let (items, items_rx) = mpsc::unbounded_channel();
        let fake = FakeHelm {
            requests,
            items,
            connected: false,
            next_id: 1,
            ordered: VecDeque::new(),
            unordered: BTreeMap::new(),
            decisions: Vec::new(),
        };
        fake.state(ChannelState::Connecting);
        (
            fake,
            ChannelHandle::new(requests_tx),
            ChannelEvents::new(items_rx),
        )
    }

    /// Open (or reopen) the channel with this snapshot: the GUI sees the snapshot, then `Connected`. Panics when an
    /// entry does not decode, which is a broken test, not a GUI behavior to test.
    pub fn connect(&mut self, snapshot: Vec<CollectionState>) {
        self.drain();
        #[expect(
            clippy::expect_used,
            reason = "the fake is a test double: a panic here is how it reports a broken test, never a GUI behavior"
        )]
        let decoded = snapshot
            .into_iter()
            .map(decode_state)
            .collect::<Result<Vec<_>, _>>()
            .expect("the fake helm's snapshot decodes");
        self.event(ChannelEvent::Snapshot(decoded));
        self.state(ChannelState::Connected);
        self.connected = true;
    }

    /// Push one collection's changes. Panics, like [`FakeHelm::connect`], when an entry does not decode, and when the
    /// channel is not connected: the real channel never delivers changes outside a connection.
    pub fn push(&mut self, changes: CollectionChanges) {
        assert!(
            self.connected,
            "the fake helm pushes changes only while connected"
        );
        #[expect(
            clippy::expect_used,
            reason = "the fake is a test double: a panic here is how it reports a broken test, never a GUI behavior"
        )]
        let decoded = decode_changes(changes).expect("the fake helm's changes decode");
        self.event(ChannelEvent::Changes(decoded));
    }

    /// The commands the GUI has sent since the last call, in order. Commands sent while the channel is not
    /// connected never show up here; they fail with [`CommandError::Reset`], as on the real channel.
    pub fn commands(&mut self) -> Vec<FakeCommand> {
        self.drain();
        let mut taken = Vec::new();
        while let Ok(request) = self.requests.try_recv() {
            self.receive(request, &mut taken);
        }
        taken
    }

    /// Answer a command returned by [`FakeHelm::commands`]. Ordered replies must follow the order in their lane.
    /// Use [`FakeHelm::reply_marked`] to add helm markers.
    pub fn reply(&mut self, id: u64, status: u16, body: Option<Value>) {
        self.reply_marked(id, status, Vec::new(), body);
    }

    /// Answer a command as [`FakeHelm::reply`] does, with `markers` among the reply's helm markers: what the real
    /// helm sends when the routed response set the helm-only headers they stand for (a 409 asking to confirm a YOLO
    /// launch, say). Like the real helm, the fake does not check that the status and markers belong together.
    pub fn reply_marked(
        &mut self,
        id: u64,
        status: u16,
        markers: Vec<HelmMarker>,
        body: Option<Value>,
    ) {
        let done = if let Some((_, done)) = self.ordered.pop_front_if(|(front, _)| *front == id) {
            done
        } else if let Some(done) = self.unordered.remove(&id) {
            done
        } else {
            panic!("the fake helm cannot answer unknown or out-of-order command {id}");
        };
        let reply = Reply {
            id,
            status,
            markers,
            body,
        };
        let _ = self.items.send(Item::Complete(done, Ok(reply)));
    }

    /// The connection broke: the GUI sees `Reconnecting`, then its unanswered commands fail with
    /// [`CommandError::Reset`].
    /// Follow with [`FakeHelm::connect`] to model the automatic reconnect.
    pub fn break_channel(&mut self) {
        self.disconnect(ChannelState::Reconnecting);
    }

    /// Another UI took the seat. The GUI sees `Displaced`; its take-back shows up in [`FakeHelm::decisions`].
    pub fn displace(&mut self) {
        self.disconnect(ChannelState::Displaced);
    }

    /// The helm refused the credential. The GUI sees `CannotConnect`; its retry shows up in the decisions.
    pub fn refuse_credential(&mut self) {
        self.disconnect(ChannelState::CannotConnect { status: 401 });
    }

    /// The user decisions forwarded so far.
    pub fn decisions(&mut self) -> Vec<FakeDecision> {
        self.drain();
        self.decisions.clone()
    }

    fn disconnect(&mut self, state: ChannelState) {
        self.drain();
        self.connected = false;
        self.state(state);
        for done in self
            .ordered
            .drain(..)
            .map(|(_, done)| done)
            .chain(std::mem::take(&mut self.unordered).into_values())
        {
            let _ = self
                .items
                .send(Item::Complete(done, Err(CommandError::Reset)));
        }
    }

    /// Settle requests that arrived while not connected, so they fail as of the state they were sent in. Commands
    /// sent while connected stay queued for [`FakeHelm::commands`].
    fn drain(&mut self) {
        if self.connected {
            return;
        }
        let mut ignored = Vec::new();
        while let Ok(request) = self.requests.try_recv() {
            self.receive(request, &mut ignored);
        }
    }

    fn receive(&mut self, request: Request, taken: &mut Vec<FakeCommand>) {
        match request {
            Request::Command {
                method,
                path,
                body,
                lane,
                done,
            } => {
                if !self.connected {
                    let _ = self
                        .items
                        .send(Item::Complete(done, Err(CommandError::Reset)));
                    return;
                }
                let id = self.next_id;
                self.next_id += 1;
                match lane {
                    Lane::Ordered => self.ordered.push_back((id, done)),
                    Lane::Unordered => {
                        self.unordered.insert(id, done);
                    }
                }
                taken.push(FakeCommand {
                    id,
                    method,
                    path: path.as_str().to_owned(),
                    body,
                    lane,
                });
            }
            Request::TakeBack => self.decisions.push(FakeDecision::TakeBack),
            Request::Retry => self.decisions.push(FakeDecision::Retry),
            Request::HelmEnded => self.decisions.push(FakeDecision::HelmEnded),
        }
    }

    fn state(&self, state: ChannelState) {
        self.event(ChannelEvent::State(state));
    }

    fn event(&self, event: ChannelEvent) {
        let _ = self.items.send(Item::Event(event));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApiPath;
    use futures_util::FutureExt;

    /// The fake is only useful if it keeps the real channel's ordering and failure semantics, since GUI tests trust
    /// it in place of the helm: changes before a reply arrive first, a break announces its state before the commands
    /// it lost fail, commands sent while down fail at once, and user decisions are recorded. All of it runs with no
    /// runtime.
    #[test]
    fn the_fake_keeps_the_channels_ordering_and_failure_semantics() {
        let (mut fake, handle, mut events) = FakeHelm::new();
        let mut early = handle.command(Method::Get, ApiPath::api(&["hosts"]).unwrap(), None);
        fake.connect(vec![]);
        assert_eq!(
            events.try_recv(),
            Some(ChannelEvent::State(ChannelState::Connecting))
        );
        assert_eq!(events.try_recv(), Some(ChannelEvent::Snapshot(vec![])));
        assert_eq!((&mut early).now_or_never(), Some(Err(CommandError::Reset)));

        let mut put = handle.command(Method::Put, ApiPath::api(&["preferences"]).unwrap(), None);
        let sent = fake.commands();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].path, "/api/preferences");
        fake.push(CollectionChanges {
            collection: farhelm_proto::ui_channel::Collection::Preferences,
            meta: farhelm_proto::ui_channel::CollectionMeta {
                availability: farhelm_proto::ui_channel::Availability::Available,
                order: vec![],
                counts: None,
                selected_outside_query: None,
                selected_cut: false,
                session_groups: Vec::new(),
            },
            upsert: vec![],
            remove: vec![],
        });
        fake.reply(sent[0].id, 200, None);
        assert_eq!(
            events.try_recv(),
            Some(ChannelEvent::State(ChannelState::Connected))
        );
        assert!(matches!(events.try_recv(), Some(ChannelEvent::Changes(_))));
        assert!(
            (&mut put).now_or_never().is_none(),
            "the reply resolved before its changes were read"
        );
        assert_eq!(events.try_recv(), None);
        assert_eq!(put.now_or_never().unwrap().unwrap().status, 200);

        let mut lost = handle.command(Method::Post, ApiPath::api(&["sessions"]).unwrap(), None);
        assert_eq!(fake.commands().len(), 1);
        fake.displace();
        assert_eq!(
            events.try_recv(),
            Some(ChannelEvent::State(ChannelState::Displaced))
        );
        assert!(
            (&mut lost).now_or_never().is_none(),
            "the lost command failed before the GUI read the new state"
        );
        assert_eq!(events.try_recv(), None);
        assert_eq!(lost.now_or_never(), Some(Err(CommandError::Reset)));
        handle.take_back();
        assert_eq!(fake.decisions(), [FakeDecision::TakeBack]);
    }

    /// A GUI test can stage either unordered reply first, even with ordered work outstanding. The recorded lane
    /// also lets callers pin the scheduling choice at the GUI boundary.
    #[test]
    fn unordered_replies_can_pass_earlier_commands_in_the_fake() {
        let (mut fake, handle, mut events) = FakeHelm::new();
        fake.connect(vec![]);
        let ordered =
            handle.command_in_lane(Lane::Unordered, Method::Put, ApiPath::session_query(), None);
        let path = ApiPath::api(&["hosts"]).unwrap();
        let first = handle.command_in_lane(Lane::Unordered, Method::Get, path.clone(), None);
        let last = handle.command_in_lane(Lane::Unordered, Method::Get, path, None);
        let sent = fake.commands();
        let [query, before, after] = &sent[..] else {
            panic!("expected three commands");
        };
        assert_eq!(
            [query.lane, before.lane, after.lane],
            [Lane::Ordered, Lane::Unordered, Lane::Unordered]
        );
        fake.reply(after.id, 201, None);
        fake.reply(before.id, 202, None);
        fake.reply(query.id, 200, None);
        while events.try_recv().is_some() {}
        for (completion, sent, status) in [
            (last, after, 201),
            (first, before, 202),
            (ordered, query, 200),
        ] {
            let reply = completion.now_or_never().unwrap().unwrap();
            assert_eq!((reply.id, reply.status), (sent.id, status));
        }
    }

    /// An unordered command between two ordered commands does not let the fake accept the second ordered reply first.
    /// Otherwise a GUI test could stage a sequence of events the real app would never receive.
    #[test]
    #[should_panic(expected = "the fake helm cannot answer unknown or out-of-order command")]
    fn the_fake_rejects_an_ordered_reply_that_passes_another() {
        let (mut fake, handle, _events) = FakeHelm::new();
        fake.connect(vec![]);
        let path = ApiPath::api(&["hosts"]).unwrap();
        let _first = handle.command(Method::Get, path.clone(), None);
        let _unordered = handle.command_in_lane(Lane::Unordered, Method::Get, path.clone(), None);
        let _last = handle.command(Method::Get, path, None);
        let sent = fake.commands();
        let [first, unordered, last] = &sent[..] else {
            panic!("expected three commands");
        };
        assert_eq!(
            [first.lane, unordered.lane, last.lane],
            [Lane::Ordered, Lane::Unordered, Lane::Ordered]
        );
        fake.reply(last.id, 200, None);
    }
}

//! A stand-in helm for GUI tests: in process, ordered, and without a runtime.
//!
//! The GUI is written against [`ChannelHandle`] and [`ChannelEvents`]; a [`FakeHelm`] serves the same pair directly,
//! so a GUI test drives the real GUI code with no sockets, threads or timers. Everything is synchronous and
//! test-driven: the test reads the commands the GUI sent, answers them, pushes changes, and stages a break, a
//! displacement or a refused credential exactly when it wants. Nothing happens unless the test does it, which is
//! what makes the fake safe under GPUI's deterministic test executor.
//!
//! Ordering matches the real channel: events and replies share one queue, so a test that pushes changes and then
//! replies gets the GUI to see the changes before the reply, as the helm guarantees; and a break announces the new
//! state before the commands it lost fail. Pushed state is decoded the way the channel decodes it, so a test that
//! pushes an entry the GUI could not decode fails in the fake rather than handing the GUI something the real channel
//! never would.
//!
//! NOTE: The fake does not model the watchdog, heartbeats or reconnect backoff; those belong to the real channel's
//! own tests. A "break" here is the outcome of all that: the state the GUI would see and the commands it would lose.

use std::collections::VecDeque;

use farhelm_proto::ui_channel::{CollectionChanges, CollectionState, Reply};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::channel::{
    ChannelEvent, ChannelEvents, ChannelHandle, ChannelState, CommandError, Completion, Item,
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
    /// Commands the test has read and not answered.
    unanswered: VecDeque<(u64, Completion)>,
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
            unanswered: VecDeque::new(),
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

    /// Answer a command returned by [`FakeHelm::commands`]. Answers must follow command order, as the helm's do.
    pub fn reply(&mut self, id: u64, status: u16, body: Option<Value>) {
        let position = self.unanswered.front().map(|(front, _)| *front);
        assert_eq!(
            position,
            Some(id),
            "the fake helm answers commands in order"
        );
        let (_, done) = self.unanswered.pop_front().unwrap();
        let _ = self
            .items
            .send(Item::Complete(done, Ok(Reply { id, status, body })));
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
        for (_, done) in std::mem::take(&mut self.unanswered) {
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
                self.unanswered.push_back((id, done));
                taken.push(FakeCommand {
                    id,
                    method,
                    path: path.as_str().to_owned(),
                    body,
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
}

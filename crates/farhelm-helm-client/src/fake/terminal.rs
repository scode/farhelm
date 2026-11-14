//! A stand-in terminal stream for GUI tests: the helm's side of one attached terminal, driven by the test.
//!
//! The GUI is written against [`TerminalHandle`] and [`TerminalEvents`]; a [`FakeTerminal`] hands out the same pair,
//! so the GUI's terminal code runs unchanged with no socket, runtime or timer. The test plays the helm: it opens the
//! attachment's socket, sends grid output and the end of the replay, detaches it with any code, drops the link,
//! refuses the attach, and reads back what the GUI sent (input, resizes, take-control and reconnect-now requests,
//! close). Nothing happens unless the test does it, which keeps the fake safe under GPUI's deterministic test executor:
//! the GUI only ever polls channels the test fills, and never parks on anything outside GPUI's dispatcher.
//!
//! The states the GUI sees follow the real stream's rules ([`crate::terminal`]): `Attaching` first; `Attached` only
//! ahead of an attachment's first output or end of replay, not when its socket opens; `Reconnecting` once per outage
//! however many attempts follow; a detach that stops the stream (decided by the same function the real stream uses, so
//! the two cannot disagree) waits in `Detached`; `Closed` on close from every state; and input sent while not attached
//! is dropped rather than delivered later. Everything but `detach_stops` is restated here rather than shared, so the
//! real stream's parity test (`terminal/tests/parity.rs`) plays one script against both and fails when they part.
//! Change a rule in either place and that test says whether the other still agrees.
//!
//! NOTE: Like the channel fake, this models outcomes, not mechanics: there is no ping, watchdog or backoff timer here.
//! A dropped link or an undecodable grid message is the state the GUI would see next (`Reconnecting`), and the test
//! decides when the reconnect's socket opens by calling [`FakeTerminal::attach`] again. The real stream's own tests
//! cover the timing. The feed is not bounded the way the real one is: a slow GUI slowing the stream is the real
//! stream's behavior to test, and a bound here would stall a GUI test until the test next called the fake.

use farhelm_proto::DetachCode;
use farhelm_proto::grid::{GridInput, GridOutput};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::{TryRecvError, TrySendError};

use crate::terminal::{
    Request, StreamState, TerminalAttach, TerminalEvent, TerminalEvents, TerminalHandle,
    detach_stops, stream_channels,
};

/// The fake feed's capacity: far more events than any test sends, so sending never waits on the GUI. Tokio allocates
/// a bounded channel's storage as it fills, so the number costs nothing up front.
const FEED_CAPACITY: usize = 1 << 20;

/// Something the GUI sent through its [`TerminalHandle`], as the fake received it.
#[derive(Debug, Clone, PartialEq)]
pub enum FakeTerminalRequest {
    /// Input, and whether the stream was attached to carry it. Undelivered input is what the real stream drops.
    Input { input: GridInput, delivered: bool },
    /// A new size for the terminal. Recording it does not mean the helm got it yet. The real stream sends a resize on
    /// the socket at once only while one is open (`Open` or `Attached`). While an attach is connecting, it sends the
    /// size as the socket's first message once that socket opens ([`FakeAttach::resize_on_open`]). At any other time
    /// (waiting for the user, between reconnect attempts) it only stores the size for the next attach's query.
    Resize { cols: u16, rows: u16 },
    /// The user's "Take control". It attaches again only from `Detached` or `Refused`; anywhere else the real stream
    /// ignores it.
    TakeControl,
    /// A request to end a reconnect's backoff early. The fake has no backoff, so the test decides what it means by
    /// when it next calls [`FakeTerminal::attach`].
    ReconnectNow,
    /// The GUI closed the stream; the fake is `Closed` from here on.
    Close,
}

/// What one attach asked the helm for: the route (displacing or not) and the size, as the real stream would have put
/// them in the attach request, and the resize it sends first on the socket, if any.
///
/// The size of a displacing attach is the one the stream held when it entered `Attaching` (at open or take-control),
/// because the real stream builds that request at once; a resize while it connects comes as `resize_on_open`. A
/// reconnect reports the latest size and no resize: the real one is built when its backoff ends, which the fake has no
/// moment for, and resizes during the backoff do apply to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FakeAttach {
    /// The displacing route: only the first attempt after open or after a take-control. Every other attach, the
    /// retry after that attempt failed included, is the non-displacing reconnect.
    pub take_control: bool,
    /// The terminal width, in columns, that the attach's query asked for.
    pub cols: u16,
    /// The terminal height, in rows, that the attach's query asked for.
    pub rows: u16,
    /// The size (columns, rows) the stream sends as the socket's first message, because the GUI resized after the
    /// query was built. `None` when the query's size is still the latest, including a resize there and back.
    pub resize_on_open: Option<(u16, u16)>,
}

/// Where the stream is, from the test's side. Only some of these are states the GUI sees: `Open` shows nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakePhase {
    /// Connecting for the first time, or again after a take-control; the next attach displaces.
    Attaching,
    /// An attach's socket is open but nothing has arrived on it: the GUI still sees the previous state and its input
    /// is dropped. The first output or end of replay makes it `Attached`.
    Open,
    /// The attachment is live; input is delivered.
    Attached,
    /// The link is gone and the stream is coming back by itself; the next attach does not displace.
    Reconnecting,
    /// Stopped by a detach that waits for the user; only take-control attaches again.
    Detached,
    /// The helm refused the attach; only take-control tries again.
    Refused,
    /// Closed, by the GUI or by its dropping every handle or its feed. Final.
    Closed,
}

/// The test's side of one fake terminal stream: it plays the helm and reads back what the GUI sent.
///
/// Every method first acts on what the GUI has sent so far, in order, so a take-control or close the GUI sent counts
/// before the test's next step. Methods that model a helm event panic when the stream is in a state where the helm
/// could not produce it (output before any socket opens, say): that is a broken test, not GUI behavior to test.
pub struct FakeTerminal {
    requests: mpsc::UnboundedReceiver<Request>,
    events: mpsc::Sender<TerminalEvent>,
    phase: FakePhase,
    /// Whether `Reconnecting` is the last state the GUI saw, so one outage is announced once. Cleared, as in the real
    /// stream, when an attachment goes live or the stream stops, not when a socket merely opens.
    reconnecting_shown: bool,
    /// Whether the next attach takes the displacing route: true from open and after a take-control, false once one
    /// attempt has been made, whether its socket opened or the link was lost first. The real stream clears it after
    /// every connect outcome except a refusal, which waits for take-control and sets it again.
    take_control_next: bool,
    /// The size held when the stream last entered `Attaching`, which a displacing attach asks for.
    attaching_size: (u16, u16),
    /// The latest size the GUI asked for.
    size: (u16, u16),
    received: Vec<FakeTerminalRequest>,
}

impl FakeTerminal {
    /// A stream for `attach` that has just started attaching, with the handle and feed the GUI under test should get.
    pub fn open(attach: &TerminalAttach) -> (FakeTerminal, TerminalHandle, TerminalEvents) {
        let (handle, feed, requests, events) = stream_channels(FEED_CAPACITY);
        let size = (attach.cols, attach.rows);
        let mut fake = FakeTerminal {
            requests,
            events,
            phase: FakePhase::Attaching,
            reconnecting_shown: false,
            take_control_next: true,
            attaching_size: size,
            size,
            received: Vec::new(),
        };
        fake.emit(TerminalEvent::State(StreamState::Attaching));
        (fake, handle, feed)
    }

    /// The attach's socket opens. The GUI sees nothing yet: `Attached` comes with the first [`FakeTerminal::output`]
    /// or [`FakeTerminal::replay_complete`], as a real attachment goes live with its replay. Returns what this attach
    /// asked for. Panics unless the stream is attaching or reconnecting.
    pub fn attach(&mut self) -> FakeAttach {
        self.settle();
        assert!(
            matches!(self.phase, FakePhase::Attaching | FakePhase::Reconnecting),
            "the fake terminal attaches only while attaching or reconnecting, not {:?}",
            self.phase
        );
        let (cols, rows) = if self.take_control_next {
            self.attaching_size
        } else {
            self.size
        };
        let request = FakeAttach {
            take_control: self.take_control_next,
            cols,
            rows,
            resize_on_open: (self.size != (cols, rows)).then_some(self.size),
        };
        self.take_control_next = false;
        self.phase = FakePhase::Open;
        request
    }

    /// One grid output message from the supervisor (replay or live). Panics unless a socket is open.
    pub fn output(&mut self, output: GridOutput) {
        self.require_socket();
        self.go_live();
        self.emit(TerminalEvent::Output(output));
    }

    /// The end of the attachment's replay; what follows is live. Panics unless a socket is open.
    pub fn replay_complete(&mut self) {
        self.require_socket();
        self.go_live();
        self.emit(TerminalEvent::ReplayComplete);
    }

    /// The helm detached this viewer. `taken_over`, `tab_closed` and `stalled` stop the stream in `Detached` until the
    /// GUI takes control; `replaced`, `other` and a missing code reconnect, so the GUI sees `Reconnecting`. Panics
    /// unless a socket is open.
    pub fn detach(&mut self, code: Option<DetachCode>, reason: &str) {
        self.require_socket();
        if detach_stops(code) {
            self.phase = FakePhase::Detached;
            self.reconnecting_shown = false;
            self.emit(TerminalEvent::State(StreamState::Detached {
                code,
                reason: reason.to_owned(),
            }));
        } else {
            self.lose_link();
        }
    }

    /// The attach failed, or the socket dropped or its watchdog fired: the GUI sees `Reconnecting` (once per outage).
    /// Call [`FakeTerminal::attach`] when the reconnect's socket should open. A failed displacing attach is not tried
    /// again as one: the retry is a non-displacing reconnect at the latest size, as in the real stream.
    pub fn drop_link(&mut self) {
        self.settle();
        assert!(
            matches!(
                self.phase,
                FakePhase::Attaching
                    | FakePhase::Open
                    | FakePhase::Attached
                    | FakePhase::Reconnecting
            ),
            "the fake terminal drops its link only while connecting or connected, not {:?}",
            self.phase
        );
        self.lose_link();
    }

    /// An undecodable grid message. The real stream treats it as a lost link that never earns an immediate
    /// reconnect; to the GUI it looks the same as [`FakeTerminal::drop_link`]. Panics unless a socket is open.
    pub fn protocol_fault(&mut self) {
        self.require_socket();
        self.lose_link();
    }

    /// The helm refused the upgrade with `status` (401 credential, 404 session gone, ...). The stream waits in
    /// `Refused` until the GUI takes control. Panics unless the stream is attaching or reconnecting.
    pub fn refuse(&mut self, status: u16) {
        self.settle();
        assert!(
            matches!(self.phase, FakePhase::Attaching | FakePhase::Reconnecting),
            "the fake terminal is refused only while attaching or reconnecting, not {:?}",
            self.phase
        );
        self.phase = FakePhase::Refused;
        self.reconnecting_shown = false;
        self.emit(TerminalEvent::State(StreamState::Refused { status }));
    }

    /// Everything the GUI sent since the last call, in order.
    pub fn received(&mut self) -> Vec<FakeTerminalRequest> {
        self.settle();
        std::mem::take(&mut self.received)
    }

    /// The stream's phase after acting on what the GUI has sent so far.
    pub fn phase(&mut self) -> FakePhase {
        self.settle();
        self.phase
    }

    /// Catch up with the GUI: notice a dropped feed, then act on its requests in order.
    fn settle(&mut self) {
        // The real stream watches its feed in every wait and ends, silently, once the GUI drops it.
        if self.events.is_closed() {
            self.phase = FakePhase::Closed;
        }
        loop {
            match self.requests.try_recv() {
                Ok(request) => self.act(request),
                Err(TryRecvError::Empty) => break,
                // Every handle is gone, which closes the real stream.
                Err(TryRecvError::Disconnected) => {
                    self.close();
                    break;
                }
            }
        }
    }

    fn act(&mut self, request: Request) {
        let received = match request {
            Request::Input(input) => FakeTerminalRequest::Input {
                input,
                delivered: self.phase == FakePhase::Attached,
            },
            Request::Resize { cols, rows } => {
                self.size = (cols, rows);
                FakeTerminalRequest::Resize { cols, rows }
            }
            Request::TakeControl => {
                if matches!(self.phase, FakePhase::Detached | FakePhase::Refused) {
                    self.phase = FakePhase::Attaching;
                    self.take_control_next = true;
                    self.attaching_size = self.size;
                    self.emit(TerminalEvent::State(StreamState::Attaching));
                }
                FakeTerminalRequest::TakeControl
            }
            Request::ReconnectNow => FakeTerminalRequest::ReconnectNow,
            Request::Close => {
                self.close();
                FakeTerminalRequest::Close
            }
        };
        self.received.push(received);
    }

    fn close(&mut self) {
        if self.phase != FakePhase::Closed {
            self.phase = FakePhase::Closed;
            self.emit(TerminalEvent::State(StreamState::Closed));
        }
    }

    /// Show `Attached` ahead of an open socket's first event, as the real stream's `mark_attached` does.
    fn go_live(&mut self) {
        if self.phase == FakePhase::Open {
            self.phase = FakePhase::Attached;
            self.reconnecting_shown = false;
            self.emit(TerminalEvent::State(StreamState::Attached));
        }
    }

    fn lose_link(&mut self) {
        self.phase = FakePhase::Reconnecting;
        // The real stream drops the displacing route after any connect outcome but a refusal. `attach` already cleared
        // it when the socket opened; this covers an attach that failed before it did.
        self.take_control_next = false;
        if !self.reconnecting_shown {
            self.reconnecting_shown = true;
            self.emit(TerminalEvent::State(StreamState::Reconnecting));
        }
    }

    /// Panics unless an attach's socket is open (live or not), the only time the helm can send on it.
    fn require_socket(&mut self) {
        self.settle();
        assert!(
            matches!(self.phase, FakePhase::Open | FakePhase::Attached),
            "the fake terminal needs an open socket for this, not {:?}",
            self.phase
        );
    }

    fn emit(&mut self, event: TerminalEvent) {
        match self.events.try_send(event) {
            Ok(()) => {}
            // The GUI dropped its feed: the real stream ends, and nothing will read this.
            Err(TrySendError::Closed(_)) => self.phase = FakePhase::Closed,
            Err(TrySendError::Full(_)) => {
                panic!(
                    "the fake terminal's feed holds {FEED_CAPACITY} events; the test sent more unread"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::AttachmentLease;

    fn attach() -> TerminalAttach {
        TerminalAttach {
            session: "s1".into(),
            tab: None,
            lease: AttachmentLease::new(),
            cols: 80,
            rows: 24,
        }
    }

    fn state(event: Option<TerminalEvent>) -> StreamState {
        match event {
            Some(TerminalEvent::State(state)) => state,
            other => panic!("expected a state event, got {other:?}"),
        }
    }

    fn text(data: &str) -> GridInput {
        GridInput::Text { data: data.into() }
    }

    /// GUI tests trust this fake in place of a real terminal stream, so it must keep the stream's contract: the state
    /// sequence of an attachment, a reconnect and a takeover; the displacing route only for the first attach and after
    /// take-control; and `Closed` on close. All of it with no runtime.
    #[test]
    fn the_fake_keeps_the_streams_states_and_routes() {
        let (mut fake, handle, mut events) = FakeTerminal::open(&attach());
        assert_eq!(state(events.try_recv()), StreamState::Attaching);

        assert!(fake.attach().take_control);
        fake.output(GridOutput::HistoryReset);
        fake.replay_complete();
        assert_eq!(state(events.try_recv()), StreamState::Attached);
        assert_eq!(
            events.try_recv(),
            Some(TerminalEvent::Output(GridOutput::HistoryReset))
        );
        assert_eq!(events.try_recv(), Some(TerminalEvent::ReplayComplete));

        // One outage is announced once, however many attempts it takes, and the reconnect is non-displacing.
        fake.drop_link();
        fake.drop_link();
        assert_eq!(state(events.try_recv()), StreamState::Reconnecting);
        assert_eq!(events.try_recv(), None);
        assert!(!fake.attach().take_control);
        fake.replay_complete();
        assert_eq!(state(events.try_recv()), StreamState::Attached);
        assert_eq!(events.try_recv(), Some(TerminalEvent::ReplayComplete));

        // A takeover waits for the user; take-control attaches through the displacing route again.
        fake.detach(Some(DetachCode::TakenOver), "taken over");
        assert_eq!(
            state(events.try_recv()),
            StreamState::Detached {
                code: Some(DetachCode::TakenOver),
                reason: "taken over".into()
            }
        );
        handle.take_control();
        assert_eq!(fake.phase(), FakePhase::Attaching);
        assert_eq!(state(events.try_recv()), StreamState::Attaching);
        assert!(fake.attach().take_control);

        handle.close();
        assert_eq!(fake.phase(), FakePhase::Closed);
        assert_eq!(state(events.try_recv()), StreamState::Closed);
    }

    /// The real stream builds a displacing attach the moment it starts attaching, so a GUI that opens at one size and
    /// fits while the attach connects asks (and gets its replay started) at the first size, and the fitted size
    /// follows as the socket's first resize. A fake that reported the fitted size in the query would let a GUI test
    /// pass whose replay the real app draws at another size; one that reported no resize would hide that the helm
    /// does learn the fitted size, which is what the GUI relies on instead of resending it. A reconnect asks for the
    /// latest size in its query and sends no resize.
    #[test]
    fn a_displacing_attach_asks_for_the_size_it_started_with() {
        let (mut fake, handle, _events) = FakeTerminal::open(&attach());
        handle.resize(100, 30);
        assert_eq!(
            fake.attach(),
            FakeAttach {
                take_control: true,
                cols: 80,
                rows: 24,
                resize_on_open: Some((100, 30)),
            }
        );
        fake.replay_complete();

        fake.drop_link();
        assert_eq!(
            fake.attach(),
            FakeAttach {
                take_control: false,
                cols: 100,
                rows: 30,
                resize_on_open: None,
            }
        );
        fake.replay_complete();

        // The size at take-control goes in the query; one sent after it follows on the socket.
        fake.detach(Some(DetachCode::TakenOver), "taken over");
        handle.resize(120, 40);
        handle.take_control();
        handle.resize(90, 20);
        assert_eq!(
            fake.attach(),
            FakeAttach {
                take_control: true,
                cols: 120,
                rows: 40,
                resize_on_open: Some((90, 20)),
            }
        );
        fake.replay_complete();

        // A resize there and back while connecting leaves the query's size current, so nothing follows it.
        fake.detach(Some(DetachCode::TakenOver), "taken over");
        handle.take_control();
        handle.resize(70, 20);
        handle.resize(90, 20);
        assert_eq!(
            fake.attach(),
            FakeAttach {
                take_control: true,
                cols: 90,
                rows: 20,
                resize_on_open: None,
            }
        );
    }

    /// A displacing attach that fails before its socket opens is retried by the real stream as an ordinary reconnect
    /// at the latest size, never as a second takeover. A fake that kept the displacing route armed would tell a GUI
    /// test that the retry displaces the other viewer, at the size from before the GUI's resize, when the app does
    /// neither. Pins both the first attach after open and the one after a take-control.
    #[test]
    fn a_failed_displacing_attach_retries_without_displacing() {
        let (mut fake, handle, mut events) = FakeTerminal::open(&attach());
        assert_eq!(state(events.try_recv()), StreamState::Attaching);
        handle.resize(100, 30);
        fake.drop_link();
        assert_eq!(state(events.try_recv()), StreamState::Reconnecting);
        assert_eq!(
            fake.attach(),
            FakeAttach {
                take_control: false,
                cols: 100,
                rows: 30,
                resize_on_open: None,
            }
        );
        fake.replay_complete();

        // The same after take-control: its first attempt fails, and the retry does not displace.
        fake.detach(Some(DetachCode::TakenOver), "taken over");
        handle.take_control();
        assert_eq!(fake.phase(), FakePhase::Attaching);
        handle.resize(120, 40);
        fake.drop_link();
        assert_eq!(
            fake.attach(),
            FakeAttach {
                take_control: false,
                cols: 120,
                rows: 40,
                resize_on_open: None,
            }
        );
    }

    /// An attachment goes live on its first output or end of replay, not when its socket opens, and the GUI's
    /// keystrokes in between are dropped by the real stream (its own test pins that window on the wire). A fake that
    /// went live on open would deliver that input, show `Attached` for an attach that never went live, and end an
    /// outage the real stream still shows.
    #[test]
    fn an_attachment_goes_live_on_its_first_output() {
        let (mut fake, handle, mut events) = FakeTerminal::open(&attach());
        assert_eq!(state(events.try_recv()), StreamState::Attaching);

        fake.attach();
        handle.input(text("too soon"));
        assert_eq!(fake.phase(), FakePhase::Open);
        assert_eq!(events.try_recv(), None);

        // An attempt that opened but never went live leaves the outage announced once.
        fake.drop_link();
        assert_eq!(state(events.try_recv()), StreamState::Reconnecting);
        fake.attach();
        fake.drop_link();
        assert_eq!(events.try_recv(), None);

        fake.attach();
        fake.replay_complete();
        assert_eq!(state(events.try_recv()), StreamState::Attached);
        assert_eq!(events.try_recv(), Some(TerminalEvent::ReplayComplete));
        handle.input(text("ls\r"));
        assert_eq!(
            fake.received(),
            [
                FakeTerminalRequest::Input {
                    input: text("too soon"),
                    delivered: false
                },
                FakeTerminalRequest::Input {
                    input: text("ls\r"),
                    delivered: true
                },
            ]
        );
    }

    /// The fake decides which detach codes stop the stream by calling the real stream's own `detach_stops`, so the
    /// two cannot disagree on a code. This test pins one stopping and one reconnecting code, and that the fake acts on
    /// both answers: a stopping code waits in `Detached`, any other reconnects.
    #[test]
    fn a_detach_stops_or_reconnects_by_the_real_streams_rule() {
        for (code, stops) in [
            (Some(DetachCode::TakenOver), true),
            (Some(DetachCode::Replaced), false),
        ] {
            let (mut fake, _handle, mut events) = FakeTerminal::open(&attach());
            fake.attach();
            fake.replay_complete();
            let _ = [events.try_recv(), events.try_recv(), events.try_recv()];
            fake.detach(code, "why");
            let expected = if stops {
                StreamState::Detached {
                    code,
                    reason: "why".into(),
                }
            } else {
                StreamState::Reconnecting
            };
            assert_eq!(state(events.try_recv()), expected, "detach code {code:?}");
        }
    }

    /// A refusal (a revoked credential, a session that is gone) is the one state besides a detach that only the user
    /// ends, so the GUI's "Take control" action must work from it: input meanwhile is dropped, take-control attaches
    /// through the displacing route, and closing still reports `Closed`.
    #[test]
    fn a_refusal_waits_for_take_control() {
        let (mut fake, handle, mut events) = FakeTerminal::open(&attach());
        assert_eq!(state(events.try_recv()), StreamState::Attaching);
        fake.refuse(401);
        assert_eq!(
            state(events.try_recv()),
            StreamState::Refused { status: 401 }
        );
        handle.input(text("x"));
        handle.take_control();
        assert_eq!(fake.phase(), FakePhase::Attaching);
        assert_eq!(state(events.try_recv()), StreamState::Attaching);
        assert!(fake.attach().take_control);

        // A reconnect can be refused too.
        fake.drop_link();
        fake.refuse(404);
        assert_eq!(state(events.try_recv()), StreamState::Reconnecting);
        assert_eq!(
            state(events.try_recv()),
            StreamState::Refused { status: 404 }
        );
        handle.close();
        assert_eq!(fake.phase(), FakePhase::Closed);
        assert_eq!(state(events.try_recv()), StreamState::Closed);
        assert_eq!(
            fake.received()[0],
            FakeTerminalRequest::Input {
                input: text("x"),
                delivered: false
            }
        );
    }

    /// An undecodable grid message is a lost link to the GUI: `Reconnecting`, dropped input, and a reconnect that
    /// never displaces anyone.
    #[test]
    fn a_protocol_fault_reconnects_without_displacing() {
        let (mut fake, handle, mut events) = FakeTerminal::open(&attach());
        fake.attach();
        fake.replay_complete();
        let _ = [events.try_recv(), events.try_recv(), events.try_recv()];
        fake.protocol_fault();
        assert_eq!(state(events.try_recv()), StreamState::Reconnecting);
        handle.input(text("x"));
        assert_eq!(
            fake.received(),
            [FakeTerminalRequest::Input {
                input: text("x"),
                delivered: false
            }]
        );
        assert!(!fake.attach().take_control);
    }

    /// The real stream ends when every handle is dropped; the fake must show `Closed` too, or a GUI test could keep
    /// driving a terminal the app had let go of.
    #[test]
    fn dropping_every_handle_closes_the_stream() {
        let (mut fake, handle, mut events) = FakeTerminal::open(&attach());
        drop(handle);
        assert_eq!(fake.phase(), FakePhase::Closed);
        assert_eq!(state(events.try_recv()), StreamState::Attaching);
        assert_eq!(state(events.try_recv()), StreamState::Closed);
    }

    /// The real stream also ends when the GUI drops its feed, with nothing left to tell; the fake must notice even when
    /// it has nothing to send, or a test could keep feeding a terminal view that is gone.
    #[test]
    fn dropping_the_event_feed_ends_the_stream() {
        let (mut fake, _handle, events) = FakeTerminal::open(&attach());
        fake.attach();
        fake.replay_complete();
        drop(events);
        assert_eq!(fake.phase(), FakePhase::Closed);
    }
}

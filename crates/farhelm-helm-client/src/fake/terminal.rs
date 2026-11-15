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
//! ahead of an attachment's first output or end of replay, not when its socket opens; `Reconnecting` when each
//! reconnect wait begins and again when its attempt starts, numbered on the default ladder; a detach that stops the
//! stream waits in `Detached`; `Closed` on close from every state; each `Attached` carries the next stamp, counted
//! from 1; and input sent while not attached, or stamped for another attachment than the live one, is dropped rather
//! than delivered later. Two pieces are the real stream's own code, so the two cannot disagree on them: which detach
//! codes stop the stream (`detach_stops`) and the ladder's numbers (`crate::reconnect::Ladder`). Everything else is
//! restated here rather than shared, so the real stream's parity test (`terminal/tests/parity.rs`) plays one script
//! against both and fails when they part. Change a rule in either place and that test says whether the other still
//! agrees.
//!
//! NOTE: Like the channel fake, this models outcomes, not mechanics: there is no ping, watchdog or timer here. A
//! dropped link or an undecodable grid message shows the GUI the next reconnect wait, the test ends that wait when it
//! likes ([`FakeTerminal::end_wait`], or [`FakeTerminal::attach`] for an attempt whose socket opens), and the GUI's
//! "Reconnect now" ends it as in the real stream. A link that proved itself (it answered a ping), which the real stream
//! reconnects without a wait, is the test's to declare with [`FakeTerminal::drop_proven_link`]. The real stream's own
//! tests cover the timing. The feed is not bounded the way the real one is: a slow GUI slowing the stream is the real
//! stream's behavior to test, and a bound here would stall a GUI test until the test next called the fake.

use farhelm_proto::DetachCode;
use farhelm_proto::grid::{GridInput, GridOutput};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::{TryRecvError, TrySendError};

use crate::reconnect::Ladder;
use crate::terminal::{
    Attachment, ReconnectStatus, Request, StreamState, TerminalAttach, TerminalEvent,
    TerminalEvents, TerminalHandle, TerminalLadder, detach_stops, retried_refusal, stream_channels,
};

/// The fake feed's capacity: far more events than any test sends, so sending never waits on the GUI. Tokio allocates
/// a bounded channel's storage as it fills, so the number costs nothing up front.
const FEED_CAPACITY: usize = 1 << 20;

/// Something the GUI sent through its [`TerminalHandle`], as the fake received it.
#[derive(Debug, Clone, PartialEq)]
pub enum FakeTerminalRequest {
    /// Input, and whether the stream was attached to carry it, to the attachment it was stamped for. Undelivered input
    /// is what the real stream drops. A group sent as one request is recorded as one entry per part, in order, all
    /// delivered or none.
    Input { input: GridInput, delivered: bool },
    /// A new size for the terminal. Recording it does not mean the helm got it yet. The real stream sends a resize on
    /// the socket at once only while one is open (`Open` or `Attached`). While an attach is connecting, it sends the
    /// size as the socket's first message once that socket opens ([`FakeAttach::resize_on_open`]). At any other time
    /// (waiting for the user, between reconnect attempts) it only stores the size for the next attach's query.
    Resize { cols: u16, rows: u16 },
    /// The user's "Take control". It attaches again only from `Detached` or `Refused`; anywhere else the real stream
    /// ignores it.
    TakeControl,
    /// A request to end a reconnect's wait early. During a wait the fake starts the attempt, as the real stream does;
    /// anywhere else it is ignored.
    ReconnectNow,
    /// The GUI closed the stream; the fake is `Closed` from here on.
    Close,
}

/// What one attach asked the helm for: the route (displacing or not) and the size, as the real stream would have put
/// them in the attach request, and the resize it sends first on the socket, if any.
///
/// The size is the one the stream held when the attempt started, because the real stream builds the request then: on
/// entering `Attaching` (at open or take-control), when a reconnect's wait ended ([`FakeTerminal::end_wait`], a
/// "Reconnect now", or [`FakeTerminal::attach`] ending it), or when a proven link dropped
/// ([`FakeTerminal::drop_proven_link`]), whose attempt starts at once. A resize after that, while the attempt
/// connects, comes as `resize_on_open`; resizes during a wait apply to the attempt that follows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FakeAttach {
    /// The displacing route: only the first attempt after open (when [`TerminalAttach::take_control`] asked for it)
    /// or after a take-control. Every other attach, the retry after that attempt failed included, is the
    /// non-displacing reconnect.
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
    /// Connecting for the first time, or again after a take-control; the next attach displaces (the first one only
    /// when [`TerminalAttach::take_control`] asked for it).
    Attaching,
    /// An attach's socket is open but nothing has arrived on it: the GUI still sees the previous state and its input
    /// is dropped. The first output or end of replay makes it `Attached`.
    Open,
    /// The attachment is live; input is delivered.
    Attached,
    /// The link is gone and the stream is coming back by itself, waiting for its next attempt or with one in flight;
    /// the next attach does not displace.
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
    /// The outage's position on the ladder, reset as the real stream resets it: when an attachment that went live
    /// ends, and at take-control.
    ladder: Ladder,
    /// The wait the GUI was last shown, while it runs: `Some` from a break or failed attempt until the attempt starts.
    waiting: Option<ReconnectStatus>,
    /// Whether the next attach takes the displacing route: from open as [`TerminalAttach::take_control`] says, true
    /// after a take-control, false once one attempt has been made, whether its socket opened or the link was lost
    /// first. The real stream clears it after every connect outcome except a refusal it does not retry, which waits
    /// for take-control and sets it again.
    take_control_next: bool,
    /// The size held when the current attempt started (entering `Attaching`, a reconnect's wait ending, or a proven
    /// link's drop, whose attempt starts at once), which its attach asks for.
    attempt_size: (u16, u16),
    /// The latest size the GUI asked for.
    size: (u16, u16),
    /// How many attachments have gone live: the live one's stamp, 0 before the first.
    attachments: u64,
    received: Vec<FakeTerminalRequest>,
}

impl FakeTerminal {
    /// A stream for `attach` that has just started attaching, with the handle and feed the GUI under test should get.
    /// Its reconnects are numbered on the app's default ladder.
    pub fn open(attach: &TerminalAttach) -> (FakeTerminal, TerminalHandle, TerminalEvents) {
        FakeTerminal::open_on(attach, TerminalLadder::default())
    }

    /// [`FakeTerminal::open`] on another ladder, for the parity test, whose real stream climbs a short one.
    #[cfg(test)]
    pub(crate) fn open_with_ladder(
        attach: &TerminalAttach,
        ladder: TerminalLadder,
    ) -> (FakeTerminal, TerminalHandle, TerminalEvents) {
        FakeTerminal::open_on(attach, ladder)
    }

    /// The one constructor behind both: the stream starts `Attaching`, with its reconnects numbered on `ladder`.
    fn open_on(
        attach: &TerminalAttach,
        ladder: TerminalLadder,
    ) -> (FakeTerminal, TerminalHandle, TerminalEvents) {
        let (handle, feed, requests, events) = stream_channels(FEED_CAPACITY);
        let size = (attach.cols, attach.rows);
        let mut fake = FakeTerminal {
            requests,
            events,
            phase: FakePhase::Attaching,
            ladder: Ladder::new(ladder),
            waiting: None,
            take_control_next: attach.take_control,
            attempt_size: size,
            size,
            attachments: 0,
            received: Vec::new(),
        };
        fake.emit(TerminalEvent::State(StreamState::Attaching));
        (fake, handle, feed)
    }

    /// The attach's socket opens. The GUI sees nothing yet: `Attached` comes with the first [`FakeTerminal::output`]
    /// or [`FakeTerminal::replay_complete`], as a real attachment goes live with its replay. Returns what this attach
    /// asked for. During a reconnect wait the wait ends first, so the GUI sees the attempt start. Panics unless the
    /// stream is attaching or reconnecting.
    pub fn attach(&mut self) -> FakeAttach {
        self.settle();
        assert!(
            matches!(self.phase, FakePhase::Attaching | FakePhase::Reconnecting),
            "the fake terminal attaches only while attaching or reconnecting, not {:?}",
            self.phase
        );
        self.start_attempt();
        let (cols, rows) = self.attempt_size;
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
            self.emit(TerminalEvent::State(StreamState::Detached {
                code,
                reason: reason.to_owned(),
            }));
        } else {
            self.lose_link();
        }
    }

    /// The attach failed, or the socket dropped or its watchdog fired: the GUI sees the next reconnect wait, a
    /// `Reconnecting` numbered on the ladder (which starts over when the socket had gone live). During a wait, the
    /// attempt starts and fails, so the GUI sees it start and then the next wait. Call [`FakeTerminal::end_wait`] or
    /// [`FakeTerminal::attach`] to move on. A failed displacing attach is not tried again as one: the retry is a
    /// non-displacing reconnect, as in the real stream.
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

    /// A live attachment whose link had proven itself (it answered a ping) drops: the real stream reconnects at once,
    /// so the GUI sees attempt 1 in flight with no wait shown before it, the common outage of a healthy link that
    /// blips or a helm that restarts. If that attempt fails ([`FakeTerminal::drop_link`]), attempt 2 waits its rung.
    /// Panics unless the attachment is live, since only a live attachment can answer a ping.
    pub fn drop_proven_link(&mut self) {
        self.settle();
        assert_eq!(
            self.phase,
            FakePhase::Attached,
            "only a live attachment proves its link"
        );
        self.ladder.reset();
        self.phase = FakePhase::Reconnecting;
        self.take_control_next = false;
        self.attempt_size = self.size;
        let status = self.ladder.schedule(true);
        self.emit(TerminalEvent::State(StreamState::Reconnecting(status)));
    }

    /// An undecodable grid message. The real stream treats it as a lost link that never earns an immediate
    /// reconnect; to the GUI it looks the same as [`FakeTerminal::drop_link`]. Panics unless a socket is open.
    pub fn protocol_fault(&mut self) {
        self.require_socket();
        self.lose_link();
    }

    /// The helm refused the upgrade with `status`. A 5xx is a failed attempt, retried on the ladder like
    /// [`FakeTerminal::drop_link`]. Any other status (401 credential, 404 session gone, ...) stops the stream in
    /// `Refused` until the GUI takes control. During a wait, the attempt it refuses starts first. Panics unless the
    /// stream is attaching or reconnecting.
    pub fn refuse(&mut self, status: u16) {
        self.settle();
        assert!(
            matches!(self.phase, FakePhase::Attaching | FakePhase::Reconnecting),
            "the fake terminal is refused only while attaching or reconnecting, not {:?}",
            self.phase
        );
        self.start_attempt();
        if retried_refusal(status) {
            self.lose_link();
            return;
        }
        self.phase = FakePhase::Refused;
        self.emit(TerminalEvent::State(StreamState::Refused { status }));
    }

    /// The reconnect wait the GUI was shown runs out: the GUI sees its attempt start (`Reconnecting` with no wait),
    /// and the attach that follows asks for the size held now. Panics unless a wait is running.
    pub fn end_wait(&mut self) {
        self.settle();
        assert!(
            self.waiting.is_some(),
            "the fake terminal ends a wait only while one runs, not in {:?}",
            self.phase
        );
        self.start_attempt();
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
            Request::Input { attachment, group } => {
                let delivered =
                    self.phase == FakePhase::Attached && attachment == Attachment(self.attachments);
                self.received.extend(
                    group
                        .into_iter()
                        .map(|input| FakeTerminalRequest::Input { input, delivered }),
                );
                return;
            }
            Request::Resize { cols, rows } => {
                self.size = (cols, rows);
                FakeTerminalRequest::Resize { cols, rows }
            }
            Request::TakeControl => {
                if matches!(self.phase, FakePhase::Detached | FakePhase::Refused) {
                    self.phase = FakePhase::Attaching;
                    self.take_control_next = true;
                    self.attempt_size = self.size;
                    self.ladder.reset();
                    self.emit(TerminalEvent::State(StreamState::Attaching));
                }
                FakeTerminalRequest::TakeControl
            }
            Request::ReconnectNow => {
                self.start_attempt();
                FakeTerminalRequest::ReconnectNow
            }
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
            self.waiting = None;
            self.emit(TerminalEvent::State(StreamState::Closed));
        }
    }

    /// Show `Attached` ahead of an open socket's first event, as the real stream's `mark_attached` does.
    fn go_live(&mut self) {
        if self.phase == FakePhase::Open {
            self.phase = FakePhase::Attached;
            self.attachments += 1;
            self.emit(TerminalEvent::State(StreamState::Attached(Attachment(
                self.attachments,
            ))));
        }
    }

    /// The link, or the attempt in flight, is gone: schedule the next attempt and show its wait. A live attachment
    /// ending starts the ladder over, as the real stream's does. This path never proves a link, so its attempt always
    /// waits; [`FakeTerminal::drop_proven_link`] is the one that reconnects at once.
    fn lose_link(&mut self) {
        self.start_attempt();
        if self.phase == FakePhase::Attached {
            self.ladder.reset();
        }
        self.phase = FakePhase::Reconnecting;
        // The real stream drops the displacing route after any connect outcome but a refusal it does not retry.
        // `attach` already cleared it when the socket opened; this covers an attach that failed before it did.
        self.take_control_next = false;
        let status = self.ladder.schedule(false);
        self.waiting = Some(status);
        self.emit(TerminalEvent::State(StreamState::Reconnecting(status)));
    }

    /// End the running wait, if any, as the real stream does just before it dials: the GUI sees the attempt start,
    /// and the attempt's request takes the size held now.
    fn start_attempt(&mut self) {
        if let Some(status) = self.waiting.take() {
            self.attempt_size = self.size;
            let in_flight = ReconnectStatus {
                next_attempt_in: None,
                ..status
            };
            self.emit(TerminalEvent::State(StreamState::Reconnecting(in_flight)));
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
    use crate::terminal::{AttachmentLease, ReconnectPhase};
    use ReconnectPhase::{Probing, Retrying};

    fn attach() -> TerminalAttach {
        TerminalAttach {
            session: "s1".into(),
            tab: None,
            lease: AttachmentLease::new(),
            cols: 80,
            rows: 24,
            take_control: true,
        }
    }

    /// The `Reconnecting` state for `attempt` of the default ladder's six retries, waiting `ms` (`None`: in flight).
    fn reconnecting(attempt: u32, phase: ReconnectPhase, ms: Option<u64>) -> StreamState {
        StreamState::Reconnecting(ReconnectStatus {
            attempt,
            of: 6,
            phase,
            next_attempt_in: ms.map(std::time::Duration::from_millis),
        })
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
        assert_eq!(
            state(events.try_recv()),
            StreamState::Attached(Attachment(1))
        );
        assert_eq!(
            events.try_recv(),
            Some(TerminalEvent::Output(GridOutput::HistoryReset))
        );
        assert_eq!(events.try_recv(), Some(TerminalEvent::ReplayComplete));

        // Each wait and each attempt is shown, numbered on the ladder, and the reconnect is non-displacing.
        fake.drop_link();
        fake.drop_link();
        assert_eq!(
            state(events.try_recv()),
            reconnecting(1, Retrying, Some(500))
        );
        assert_eq!(state(events.try_recv()), reconnecting(1, Retrying, None));
        assert_eq!(
            state(events.try_recv()),
            reconnecting(2, Retrying, Some(1_000))
        );
        assert_eq!(events.try_recv(), None);
        assert!(!fake.attach().take_control);
        assert_eq!(state(events.try_recv()), reconnecting(2, Retrying, None));
        fake.replay_complete();
        assert_eq!(
            state(events.try_recv()),
            StreamState::Attached(Attachment(2))
        );
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
        assert_eq!(
            state(events.try_recv()),
            reconnecting(1, Retrying, Some(500))
        );
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
        handle.input(Attachment(0), vec![text("too soon")]);
        assert_eq!(fake.phase(), FakePhase::Open);
        assert_eq!(events.try_recv(), None);

        // An attempt that opened but never went live is a failed attempt: the ladder climbs on, not over.
        fake.drop_link();
        assert_eq!(
            state(events.try_recv()),
            reconnecting(1, Retrying, Some(500))
        );
        fake.attach();
        assert_eq!(state(events.try_recv()), reconnecting(1, Retrying, None));
        fake.drop_link();
        assert_eq!(
            state(events.try_recv()),
            reconnecting(2, Retrying, Some(1_000))
        );

        fake.attach();
        fake.replay_complete();
        assert_eq!(state(events.try_recv()), reconnecting(2, Retrying, None));
        assert_eq!(
            state(events.try_recv()),
            StreamState::Attached(Attachment(1))
        );
        assert_eq!(events.try_recv(), Some(TerminalEvent::ReplayComplete));
        handle.input(Attachment(1), vec![text("ls\r")]);
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
                reconnecting(1, Retrying, Some(500))
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
        handle.input(Attachment(0), vec![text("x")]);
        handle.take_control();
        assert_eq!(fake.phase(), FakePhase::Attaching);
        assert_eq!(state(events.try_recv()), StreamState::Attaching);
        assert!(fake.attach().take_control);

        // A reconnect can be refused too: the refusal answers the attempt, which starts first.
        fake.drop_link();
        fake.refuse(404);
        assert_eq!(
            state(events.try_recv()),
            reconnecting(1, Retrying, Some(500))
        );
        assert_eq!(state(events.try_recv()), reconnecting(1, Retrying, None));
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
        assert_eq!(
            state(events.try_recv()),
            reconnecting(1, Retrying, Some(500))
        );
        handle.input(Attachment(1), vec![text("x")]);
        assert_eq!(
            fake.received(),
            [FakeTerminalRequest::Input {
                input: text("x"),
                delivered: false
            }]
        );
        assert!(!fake.attach().take_control);
    }

    /// A group typed for an attachment that has since been replaced is dropped whole, even though the stream is live
    /// again by the time it reads it, while a group stamped for the live attachment arrives whole and in order. This
    /// is the fence GUI tests rely on to show that a keystroke typed at one screen never reaches the next one.
    #[test]
    fn input_reaches_only_the_attachment_it_was_stamped_for() {
        let (mut fake, handle, mut events) = FakeTerminal::open(&attach());
        fake.attach();
        fake.replay_complete();
        let _ = [events.try_recv(), events.try_recv(), events.try_recv()];
        fake.drop_link();
        fake.attach();
        fake.replay_complete();
        assert_eq!(
            state(events.try_recv()),
            reconnecting(1, Retrying, Some(500))
        );
        assert_eq!(state(events.try_recv()), reconnecting(1, Retrying, None));
        assert_eq!(
            state(events.try_recv()),
            StreamState::Attached(Attachment(2))
        );
        handle.input(Attachment(1), vec![text("a"), text("b")]);
        handle.input(Attachment(2), vec![text("c"), text("d")]);
        handle.input(Attachment(2), Vec::new());
        let input = |data, delivered| FakeTerminalRequest::Input {
            input: text(data),
            delivered,
        };
        assert_eq!(
            fake.received(),
            [
                input("a", false),
                input("b", false),
                input("c", true),
                input("d", true)
            ]
        );
    }

    /// GUI tests drive the terminal's outage card through this fake, so it must show every phase the real stream
    /// shows and number it the same way: each wait and each attempt start, the six retries and then the probes that
    /// keep counting, a 5xx refusal retried like any failed attempt, "Reconnect now" starting the waiting attempt
    /// (probing included) with its number kept, and an attachment that went live starting the next outage at the
    /// bottom of the ladder. A wait the test ends with `end_wait` asks for the size held then.
    #[test]
    fn the_fake_shows_each_wait_and_attempt_on_the_ladder() {
        let (mut fake, handle, mut events) = FakeTerminal::open(&attach());
        fake.attach();
        fake.replay_complete();
        let _ = [events.try_recv(), events.try_recv(), events.try_recv()];

        // Six retries, the third answered with a 5xx, then the first probe.
        let waits = [500, 1_000, 2_000, 4_000, 8_000, 15_000];
        fake.drop_link();
        for (attempt, wait) in (1..).zip(waits) {
            assert_eq!(
                state(events.try_recv()),
                reconnecting(attempt, Retrying, Some(wait))
            );
            fake.end_wait();
            assert_eq!(
                state(events.try_recv()),
                reconnecting(attempt, Retrying, None)
            );
            if attempt == 3 {
                fake.refuse(503);
            } else {
                fake.drop_link();
            }
        }
        assert_eq!(
            state(events.try_recv()),
            reconnecting(7, Probing, Some(30_000))
        );

        // Reconnect now starts the probe at once, keeping its number; its request takes the size held then.
        handle.resize(100, 30);
        handle.reconnect_now();
        assert_eq!(fake.phase(), FakePhase::Reconnecting);
        assert_eq!(state(events.try_recv()), reconnecting(7, Probing, None));
        handle.resize(120, 40);
        assert_eq!(fake.attach().resize_on_open, Some((120, 40)));
        fake.drop_link();
        assert_eq!(
            state(events.try_recv()),
            reconnecting(8, Probing, Some(30_000))
        );

        // An attachment that goes live ends the outage: the next one starts at the first rung.
        fake.attach();
        fake.replay_complete();
        assert_eq!(state(events.try_recv()), reconnecting(8, Probing, None));
        assert_eq!(
            state(events.try_recv()),
            StreamState::Attached(Attachment(2))
        );
        assert_eq!(events.try_recv(), Some(TerminalEvent::ReplayComplete));
        fake.drop_link();
        assert_eq!(
            state(events.try_recv()),
            reconnecting(1, Retrying, Some(500))
        );
    }

    /// A healthy link that blips is the common outage, and the card shows it in an order only the fake can give a GUI
    /// test: no wait before the first attempt, then, if it fails, the ladder's second rung, as the real stream does
    /// after a link that answered a ping. On the real side,
    /// `only_an_attachment_that_answered_a_ping_reconnects_at_once` pins the first half (attempt 1 in flight with no
    /// wait, non-displacing); the second rung after it follows from the shared `Ladder::schedule`. The proven link
    /// drops after an earlier outage, whose count going live does not clear, so the drop itself must start the ladder
    /// over. The attempt is non-displacing and asks for the size held when the link dropped.
    #[test]
    fn a_proven_link_reconnects_at_once_and_then_climbs_from_the_second_rung() {
        let (mut fake, handle, mut events) = FakeTerminal::open(&attach());
        fake.attach();
        fake.replay_complete();
        // One ordinary outage first, which leaves the ladder's count at attempt 1 once the attachment is live again.
        fake.drop_link();
        fake.attach();
        fake.replay_complete();
        assert_eq!(fake.phase(), FakePhase::Attached);
        while events.try_recv().is_some() {}
        handle.resize(100, 30);
        fake.drop_proven_link();
        assert_eq!(state(events.try_recv()), reconnecting(1, Retrying, None));
        assert_eq!(events.try_recv(), None);
        assert_eq!(
            fake.attach(),
            FakeAttach {
                take_control: false,
                cols: 100,
                rows: 30,
                resize_on_open: None,
            }
        );
        fake.drop_link();
        assert_eq!(
            state(events.try_recv()),
            reconnecting(2, Retrying, Some(1_000))
        );
    }

    /// A terminal opened without the intent to take it (a tab a recovering view opens, say) must not displace another
    /// viewer with its first attach, and a GUI test must be able to see that: the fake reports the non-displacing
    /// route for that attach, at the size the stream opened with, while a take-control after it still displaces.
    #[test]
    fn a_first_attach_that_does_not_take_control_does_not_displace() {
        let unowned = TerminalAttach {
            take_control: false,
            ..attach()
        };
        let (mut fake, handle, mut events) = FakeTerminal::open(&unowned);
        assert_eq!(state(events.try_recv()), StreamState::Attaching);
        assert_eq!(
            fake.attach(),
            FakeAttach {
                take_control: false,
                cols: 80,
                rows: 24,
                resize_on_open: None,
            }
        );
        fake.detach(Some(DetachCode::TakenOver), "another client attached");
        handle.take_control();
        assert!(fake.attach().take_control);
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

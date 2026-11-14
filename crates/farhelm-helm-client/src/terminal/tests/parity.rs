//! The fake terminal stream (`crate::fake::terminal`) checked against the real one, step for step.
//!
//! GUI tests drive a `FakeTerminal` in place of a real stream, because GPUI's deterministic test executor panics on
//! anything that waits on tokio. The fake therefore restates the real stream's rules rather than running them: its
//! states, one announcement per outage, which attach displaces and at what size, and which requests each state
//! ignores. Only `detach_stops` is shared code. A rule changed in `terminal.rs` and not in the fake would leave every
//! GUI test built on the fake passing against the old rule, so this test plays one script against both and requires
//! them to agree, step by step, on what the GUI was shown and on what reached the helm.
//!
//! NOTE: This compares the GUI's event feed, the route and size of every accepted attach and the resize sent as its
//! socket opens, and which input reached the helm. It does not compare timing (pings, the watchdog, the backoff),
//! which the fake does not model and the real stream's own tests pin, nor the route of an attach that failed or was
//! refused, which the fake does not report. One script cannot reach every state, so some of the fake's rules are
//! pinned only by its own tests: see the test's docstring for which.

use super::*;
use crate::fake::terminal::{FakeAttach, FakePhase, FakeTerminal, FakeTerminalRequest};

/// How long the real stream gets for any one step. Generous: every step completes in milliseconds, and a stream
/// that diverged from the fake waits for something that never comes, so this only bounds how long a failure takes.
const STEP_WAIT: Duration = Duration::from_secs(10);

/// One step of the script: something the helm does, or something the GUI asks through its handle.
#[derive(Debug, Clone, Copy)]
enum Step {
    /// The helm accepts the attach that is connecting.
    Accept,
    /// The attach that is connecting fails before its socket opens (the connection drops during the handshake).
    Fail,
    /// The helm refuses the attach that is connecting with this HTTP status.
    Refuse(u16),
    /// One grid output message on the open socket.
    Output,
    /// The end of the open socket's replay.
    ReplayComplete,
    /// A detach notice with this code, then the socket closes.
    Detach(Option<DetachCode>),
    /// The open socket drops.
    DropLink,
    /// A binary message on the open socket that is not a grid message.
    Undecodable,
    /// Input stamped for the newest attachment the GUI was shown.
    Input(&'static str),
    /// Input stamped for the attachment before the newest one: what the GUI sends when it typed for an attachment the
    /// stream has since replaced.
    StaleInput(&'static str),
    Resize(u16, u16),
    TakeControl,
    ReconnectNow,
    Close,
}

/// What one side showed at one step: the parts both the fake and the real stream can report.
#[derive(Debug, Default, PartialEq)]
struct Observed {
    /// The events the GUI's feed got, in order.
    events: Vec<TerminalEvent>,
    /// For an accepted attach: its route, the size its query asked for, and the resize sent as its socket opened.
    attach: Option<FakeAttach>,
    /// For input: what reached the helm, if anything did.
    delivered: Option<GridInput>,
}

/// The script both sides play, after the stream opens at 80x24.
///
/// On the real side every GUI step lands where the stream's next move does not depend on timing: before a connect the
/// test has not answered yet, while the stream waits for the user, or while a socket is open and the test can see the
/// request handled on it (see `run_real`). Never during a backoff, where a resize might or might not beat the
/// reconnect.
fn script() -> Vec<Step> {
    use Step::*;
    let mut steps = vec![
        // A resize while the first, displacing attach connects reaches only the retry after that attach fails, and the
        // retry does not displace.
        Resize(100, 30),
        Fail,
        Accept,
        // A socket that opened but never went live leaves the outage announced once and drops input; the first output
        // makes it live.
        DropLink,
        Accept,
        Input("before live"),
        Output,
        ReplayComplete,
        // Live: input reaches the helm, take-control is ignored, and a resize is kept for the next attach.
        Input("live"),
        TakeControl,
        Resize(110, 35),
        DropLink,
        Accept,
        ReplayComplete,
        // An undecodable message is a lost link.
        Undecodable,
        Accept,
        ReplayComplete,
        // A takeover waits for the user. Input is dropped and reconnect-now ignored; take-control attaches through the
        // displacing route at the size held when it was asked, and a resize while that attach connects is sent as
        // soon as its socket opens.
        Detach(Some(DetachCode::TakenOver)),
        Input("while detached"),
        ReconnectNow,
        Resize(120, 40),
        TakeControl,
        Resize(90, 20),
        Accept,
        ReplayComplete,
        Input("live again"),
        // Typed for the attachment before the takeover: dropped, although the stream is live.
        StaleInput("for the old screen"),
        // A refused reconnect waits for the user as well and ends the outage it was part of: after take-control, a
        // failed attempt announces a new one, and the retry does not displace.
        DropLink,
        Refuse(401),
        Input("while refused"),
        TakeControl,
        Fail,
        Accept,
        ReplayComplete,
        Input("after the refusal"),
    ];
    // Each remaining detach code, followed by a take-control that only a stopped stream acts on.
    for code in [
        Some(DetachCode::Replaced),
        Some(DetachCode::Other),
        None,
        Some(DetachCode::TabClosed),
        Some(DetachCode::Stalled),
    ] {
        steps.extend([Detach(code), TakeControl, Accept, ReplayComplete]);
    }
    // A close while the stream waits for the user still ends in `Closed`.
    steps.extend([Detach(Some(DetachCode::TakenOver)), Close]);
    steps
}

fn text(data: &str) -> GridInput {
    GridInput::Text { data: data.into() }
}

/// The stamp of the newest attachment `events` announced, or 0 (no attachment) before the first.
fn newest(events: &[TerminalEvent], stamp: &mut u64) {
    for event in events {
        if let TerminalEvent::State(StreamState::Attached(Attachment(n))) = event {
            *stamp = *n;
        }
    }
}

/// Play the script against the fake. Index 0 is what the GUI saw on open; index `i` is step `i - 1`. Beside each
/// step's observations goes the fake's phase after it, which tells `run_real` whether a socket is open and live.
fn run_fake(steps: &[Step]) -> (Vec<Observed>, Vec<FakePhase>) {
    let (mut fake, handle, mut events) = FakeTerminal::open(&attach("s1"));
    let (mut observed, mut phases) = (Vec::new(), Vec::new());
    let mut stamp = 0;
    for step in std::iter::once(None).chain(steps.iter().copied().map(Some)) {
        let mut seen = Observed::default();
        match step {
            None => {}
            Some(Step::Accept) => seen.attach = Some(fake.attach()),
            Some(Step::Fail | Step::DropLink) => fake.drop_link(),
            Some(Step::Refuse(status)) => fake.refuse(status),
            Some(Step::Output) => fake.output(GridOutput::HistoryReset),
            Some(Step::ReplayComplete) => fake.replay_complete(),
            Some(Step::Detach(code)) => fake.detach(code, "why"),
            Some(Step::Undecodable) => fake.protocol_fault(),
            Some(step @ (Step::Input(data) | Step::StaleInput(data))) => {
                let stamp = if matches!(step, Step::StaleInput(_)) {
                    stamp - 1
                } else {
                    stamp
                };
                handle.input(Attachment(stamp), vec![text(data)]);
                seen.delivered = fake
                    .received()
                    .into_iter()
                    .find_map(|request| match request {
                        FakeTerminalRequest::Input {
                            input,
                            delivered: true,
                        } => Some(input),
                        _ => None,
                    });
            }
            Some(Step::Resize(cols, rows)) => handle.resize(cols, rows),
            Some(Step::TakeControl) => handle.take_control(),
            Some(Step::ReconnectNow) => handle.reconnect_now(),
            Some(Step::Close) => handle.close(),
        }
        // The fake acts on the GUI's requests when the test next calls it; settle them before reading the feed.
        phases.push(fake.phase());
        while let Some(event) = events.try_recv() {
            seen.events.push(event);
        }
        newest(&seen.events, &mut stamp);
        observed.push(seen);
    }
    assert_eq!(events.try_recv(), None);
    (observed, phases)
}

/// Play the script against the real stream over real sockets, the test answering each attach as the step says.
///
/// The fake's run says how many events to wait for at each step and whether an input should be read off the socket:
/// the real stream gives no other sign of being done with a step. Waiting on the fake's count is not taking its word
/// for it. Before each step acts, nothing may be waiting in the feed, so a real stream that showed an event early (or
/// an extra one) fails at the start of the next step, and one that shows fewer fails the step on [`STEP_WAIT`]. An
/// extra event shown later, after a step's events were read, is caught only at a later step or at the end, where the
/// feed must be over.
///
/// An accepted socket is read up to its first ping, which the stream sends only once it serves the socket, after any
/// resize owed from the connect: that reads the opening resize (or shows there was none) and shows the stream is past
/// connecting before the next step. A resize made while connecting may be handled by the stream either inside the
/// connect or as the socket's first request; both put the same resize before that ping, so the step does not race.
///
/// A request the GUI sends while a socket is open races the helm's next frame on it, so there the real side waits
/// for proof that the stream handled the request before the next step: a live resize is read off the socket, and
/// input on a socket that is open but not live is followed by a resize to the size already held, which the stream
/// sends only after it has handled that input.
async fn run_real(steps: &[Step], expected: &[Observed], phases: &[FakePhase]) -> Vec<Observed> {
    let (addr, mut connections) = listen().await;
    // The watchdog never fires and only the first ping on each socket is sent (none is answered), so only the script
    // ends a socket; the short backoff keeps reconnects prompt.
    let config = ChannelConfig {
        ping_interval: Duration::from_millis(20),
        watchdog: Duration::from_secs(3600),
        reconnect_initial: Duration::from_millis(10),
        reconnect_max: Duration::from_millis(10),
    };
    let (handle, mut events) =
        open_terminal(HelmEndpoint::new(addr, "test-secret"), attach("s1"), config);
    let mut socket: Option<WebSocketStream<TcpStream>> = None;
    // The latest size the GUI asked for, which the resize after early input repeats.
    let mut size = (80, 24);
    let mut observed = Vec::new();
    let mut stamp = 0;
    for (index, step) in std::iter::once(None)
        .chain(steps.iter().copied().map(Some))
        .enumerate()
    {
        let mut seen = Observed::default();
        if index > 0
            && let Some(early) = events.try_recv()
        {
            panic!(
                "step {index} ({step:?}): the real stream showed {early:?} before the step acted"
            );
        }
        match step {
            None => {}
            Some(Step::Accept) => {
                let mut accepted = accept_upgrade(next_connection(&mut connections).await).await;
                seen.attach = Some(FakeAttach {
                    resize_on_open: opening_resize(&mut accepted.socket).await,
                    ..attach_of(&accepted.uri)
                });
                socket = Some(accepted.socket);
            }
            Some(Step::Fail) => drop(next_connection(&mut connections).await),
            Some(Step::Refuse(status)) => {
                refuse_upgrade(next_connection(&mut connections).await, status).await;
            }
            Some(Step::Output) => {
                let frame = Message::binary(GridOutput::HistoryReset.encode().unwrap());
                socket.as_mut().unwrap().send(frame).await.unwrap();
            }
            Some(Step::ReplayComplete) => {
                let frame = Message::text(TerminalServerMessage::ReplayComplete.to_text());
                socket.as_mut().unwrap().send(frame).await.unwrap();
            }
            Some(Step::Detach(code)) => {
                let notice = TerminalServerMessage::Detached {
                    reason: "why".into(),
                    code,
                };
                let mut detached = socket.take().unwrap();
                detached
                    .send(Message::text(notice.to_text()))
                    .await
                    .unwrap();
            }
            Some(Step::DropLink) => socket = None,
            Some(Step::Undecodable) => {
                let frame = Message::binary(vec![0xff]);
                socket.as_mut().unwrap().send(frame).await.unwrap();
            }
            Some(Step::Input(data)) if phases[index] == FakePhase::Open => {
                let open = socket.as_mut().unwrap();
                handle.input(Attachment(stamp), vec![text(data)]);
                handle.resize(size.0, size.1);
                seen.delivered = next_resize(open).await;
            }
            // On a live socket, like input on one that is not live yet: the resize after it shows it was handled.
            Some(Step::StaleInput(data)) => {
                let open = socket.as_mut().unwrap();
                handle.input(Attachment(stamp - 1), vec![text(data)]);
                handle.resize(size.0, size.1);
                seen.delivered = next_resize(open).await;
            }
            Some(Step::Input(data)) => {
                handle.input(Attachment(stamp), vec![text(data)]);
                if expected[index].delivered.is_some() {
                    let Some(open) = socket.as_mut() else {
                        panic!("step {index}: the fake delivered input while no socket was open")
                    };
                    seen.delivered = Some(next_input(open).await);
                }
            }
            Some(Step::Resize(cols, rows)) => {
                size = (cols, rows);
                handle.resize(cols, rows);
                if phases[index] == FakePhase::Attached {
                    seen.delivered = next_resize(socket.as_mut().unwrap()).await;
                }
            }
            Some(Step::TakeControl) => handle.take_control(),
            Some(Step::ReconnectNow) => handle.reconnect_now(),
            Some(Step::Close) => handle.close(),
        }
        for _ in 0..expected[index].events.len() {
            match timeout(STEP_WAIT, events.recv()).await {
                Ok(Some(event)) => seen.events.push(event),
                _ => panic!(
                    "step {index} ({step:?}): the real stream showed {:?} where the fake showed {:?}",
                    seen.events, expected[index].events
                ),
            }
        }
        newest(&seen.events, &mut stamp);
        observed.push(seen);
    }
    // `Closed` is final: the feed ends after it.
    let after = timeout(STEP_WAIT, events.recv()).await;
    assert!(
        matches!(after, Ok(None)),
        "the real stream went on after the script: {after:?}"
    );
    observed
}

/// The stream's next connection attempt, before its handshake.
async fn next_connection(connections: &mut mpsc::UnboundedReceiver<TcpStream>) -> TcpStream {
    timeout(STEP_WAIT, connections.recv())
        .await
        .expect("the stream connected")
        .expect("the listener is alive")
}

/// Read a newly accepted `socket` up to the stream's first ping, which it sends only once it serves the socket, and
/// return the size of the last resize before it. Only the first ping on a socket is ever sent (the script answers
/// none), so nothing later waits for another. Panics on input, which no step sends before the socket is served.
async fn opening_resize(socket: &mut WebSocketStream<TcpStream>) -> Option<(u16, u16)> {
    let mut resize = None;
    loop {
        match timeout(STEP_WAIT, socket.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                match serde_json::from_str::<TerminalClientMessage>(&text) {
                    Ok(TerminalClientMessage::Ping) => return resize,
                    Ok(TerminalClientMessage::Resize { cols, rows }) => resize = Some((cols, rows)),
                    other => panic!("an unexpected control frame before the first ping: {other:?}"),
                }
            }
            other => panic!("no ping reached the helm: {other:?}"),
        }
    }
}

/// Read `socket` up to the next resize frame, and return the input (if any) that reached the helm before it.
async fn next_resize(socket: &mut WebSocketStream<TcpStream>) -> Option<GridInput> {
    let mut input = None;
    loop {
        match timeout(STEP_WAIT, socket.next()).await {
            Ok(Some(Ok(Message::Text(text)))) if text.contains(r#""type":"resize""#) => {
                return input;
            }
            Ok(Some(Ok(Message::Binary(bytes)))) => {
                input.get_or_insert(GridInput::decode(&bytes).unwrap());
            }
            Ok(Some(Ok(_))) => {}
            other => panic!("no resize reached the helm: {other:?}"),
        }
    }
}

/// The next input frame the helm got, skipping control frames.
async fn next_input(socket: &mut WebSocketStream<TcpStream>) -> GridInput {
    loop {
        match timeout(STEP_WAIT, socket.next()).await {
            Ok(Some(Ok(Message::Binary(bytes)))) => return GridInput::decode(&bytes).unwrap(),
            Ok(Some(Ok(_))) => {}
            other => panic!("no input reached the helm: {other:?}"),
        }
    }
}

/// The route and size an attach asked for, read off its path and query the way the fake reports them. The opening
/// resize is not in the request, so it is left `None` for the caller to fill in from the socket.
fn attach_of(uri: &str) -> FakeAttach {
    let (path, query) = uri.split_once('?').unwrap();
    let param = |name: &str| -> u16 {
        query
            .split('&')
            .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| panic!("no {name} in {uri}"))
    };
    FakeAttach {
        take_control: !path.ends_with("/unowned"),
        cols: param("cols"),
        rows: param("rows"),
        resize_on_open: None,
    }
}

/// The fake terminal must show the GUI what the real stream shows and send the helm what the real stream sends, or GUI
/// tests built on it pass against rules the app does not follow. One script covers most of the rules the fake restates:
/// the state sequence (`Attached` on the first output rather than on open, `Reconnecting` once per outage and again
/// after a refusal, `Closed` last), which detach codes stop the stream, an undecodable message as a lost link, that
/// only take-control from a stopped or refused stream displaces and that a failed displacing attach retries without
/// displacing, the size each attach asks for (including a resize made while live), the resize that follows an attach
/// whose query a resize overtook while it connected, input dropped while no attachment is live (with a socket open or
/// not), and input stamped for an earlier attachment dropped while a later one is live. Each step's observations must
/// be equal on both sides.
///
/// Not compared here: a close from any state but waiting for the user, and the stream ending when the GUI drops every
/// handle or its feed. The fake's own tests and the real stream's tests pin those separately.
#[tokio::test]
async fn the_fake_terminal_and_the_real_stream_agree_on_one_script() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let steps = script();
        let (fake, phases) = run_fake(&steps);
        let real = run_real(&steps, &fake, &phases).await;
        assert_eq!(real.len(), fake.len());
        for (index, (real, fake)) in real.iter().zip(&fake).enumerate() {
            let step = index.checked_sub(1).map(|step| steps[step]);
            assert_eq!(
                real, fake,
                "step {index} ({step:?}, None is the open): the real stream (left) and the fake (right) disagree"
            );
        }
    })
    .await
    .expect("the parity test finished in time");
}

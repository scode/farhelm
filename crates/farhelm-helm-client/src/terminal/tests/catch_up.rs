//! The catch-up's idle bound ("The catch-up's idle bound" in `terminal.rs`) against the scripted helm, on real time.
//!
//! The bound is shrunk to a few hundred milliseconds. Each test waits for the event the bound produces rather than for
//! the bound's length, so a loaded machine only makes them slower; where a test has to pace the helm's side under the
//! bound, the pacing is a fraction of it, and the comment there says how much slack that leaves.

use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ReconnectPhase::Retrying;

/// The `n`th grid message of a scripted replay: numbered, so the feed shows their order and that none is missing.
fn numbered(n: usize) -> GridOutput {
    GridOutput::Hello {
        cols: 80,
        rows: 24,
        history_cap: u32::try_from(n).unwrap(),
    }
}

/// Send the helm's `n`th grid message ([`numbered`]) on `socket`.
async fn send_numbered(socket: &mut WebSocketStream<TcpStream>, n: usize) {
    let frame = numbered(n).encode().unwrap();
    socket.send(Message::binary(frame)).await.unwrap();
}

/// Play a helm whose supervisor answered the attach and then sent nothing: every ping is answered, with a control
/// message this client does not know and a WebSocket ping beside it, and no grid message ever goes out. Returns how
/// many pings were answered.
fn answer_pings_only(mut socket: WebSocketStream<TcpStream>) -> Arc<AtomicUsize> {
    let answered = Arc::new(AtomicUsize::new(0));
    let count = answered.clone();
    tokio::spawn(async move {
        while let Some(Ok(message)) = socket.next().await {
            if matches!(&message, Message::Text(text) if text.as_str() == r#"{"type":"ping"}"#) {
                let replies = [
                    Message::text(PONG),
                    Message::text(r#"{"type":"a_later_notice"}"#),
                    Message::Ping(Vec::new().into()),
                ];
                for reply in replies {
                    if socket.send(reply).await.is_err() {
                        return;
                    }
                }
                count.fetch_add(1, Ordering::SeqCst);
            }
        }
    });
    answered
}

/// Why: the helm answers pings once the supervisor has answered the attach, so a reattach that then sends no replay
/// keeps a healthy-looking link that the watchdog never ends, and the terminal would sit behind its reconnect card
/// until the connection dropped. The old app's catch-up watchdog counted such a reconnect as a failed attempt.
///
/// Spec: after a working attachment drops, a reattach whose helm answers every ping (with an unknown control message
/// and a WebSocket ping beside each pong) but sends no grid message is ended at the bound as a failed attempt. The
/// stream shows no `Attached` for it, shows the ladder's second rung rather than starting over at the first, and
/// attaches again.
#[tokio::test]
async fn a_reattach_that_sends_no_grid_message_climbs_the_ladder() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let config = ChannelConfig {
            ping_interval: Duration::from_millis(20),
            terminal_ladder: ladder(&[10, 20], 30),
            replay_idle: Duration::from_millis(300),
            ..ChannelConfig::default()
        };
        let (_handle, mut events, mut accepted, first) = attached(config).await;
        drop(first);
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(1, 2, Retrying, Some(10))
        );
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(1, 2, Retrying, None)
        );
        let silent = accepted.recv().await.unwrap();
        let answered = answer_pings_only(silent.socket);
        // Far past the bound and well inside the test's own, so a stream that is not bounded fails here, saying so.
        let ended = tokio::time::timeout(Duration::from_secs(10), next_event(&mut events))
            .await
            .expect("nothing ended a reattach that answered pings and sent no grid message");
        assert_eq!(ended, reconnecting(2, 2, Retrying, Some(20)));
        assert!(
            answered.load(Ordering::SeqCst) > 0,
            "the premise: the silent helm answered pings while the bound ran"
        );
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(2, 2, Retrying, None)
        );
        accepted.recv().await.unwrap();
    })
    .await
    .expect("the stream test finished in time");
}

/// Why: a replay that stops partway would otherwise keep the terminal hidden for as long as its link holds, while a
/// slow replay that keeps arriving must never be cut off. The old app's watchdog measured silence for that reason and,
/// once it ran out, revealed what had arrived as a live terminal.
///
/// Spec: grid messages spaced under the bound, for longer than the bound in all, all arrive with no end of replay
/// among them; once they stop, the stream ends the catch-up with `ReplayIdle`, not the supervisor's `ReplayComplete`.
/// The attachment is live from there (its input reaches the helm), and the supervisor's own marker arriving later is
/// dropped, so the caller sees one end of replay and the output after it.
#[tokio::test]
async fn a_replay_that_goes_quiet_partway_ends_its_catch_up_on_what_arrived() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let (addr, mut accepted) = server().await;
        let config = ChannelConfig {
            ping_interval: Duration::from_secs(3600),
            replay_idle: Duration::from_millis(1500),
            ..ChannelConfig::default()
        };
        let (handle, mut events) =
            open_terminal(HelmEndpoint::new(addr, "test-secret"), attach("s1"), config);
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attaching)
        );
        let mut socket = accepted.recv().await.unwrap().socket;
        const SENT: usize = 7;
        send_numbered(&mut socket, 0).await;
        for n in 1..SENT {
            // The pacing is a fifth of the bound, for 1.8 s in all against the 1.5 s bound. A bound that measured
            // time since the socket opened ends the catch-up before the last messages; a stall of over a second on
            // this machine between two sends would end it too, which is the slack this pacing leaves.
            // sleep-ok: the helm's pacing of its replay, under the bound and for longer than it in all
            tokio::time::sleep(Duration::from_millis(300)).await;
            send_numbered(&mut socket, n).await;
        }
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attached(Attachment(1)))
        );
        for n in 0..SENT {
            assert_eq!(
                next_event(&mut events).await,
                TerminalEvent::Output(numbered(n)),
                "message {n}"
            );
        }
        assert_eq!(next_event(&mut events).await, TerminalEvent::ReplayIdle);

        let typed = GridInput::Text {
            data: "typed".into(),
        };
        handle.input(Attachment(1), vec![typed.clone()]);
        let Some(Ok(Message::Binary(bytes))) = socket.next().await else {
            panic!("the input did not reach the helm")
        };
        assert_eq!(GridInput::decode(&bytes).unwrap(), typed);

        socket
            .send(Message::text(r#"{"type":"replay_complete"}"#))
            .await
            .unwrap();
        send_numbered(&mut socket, 99).await;
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::Output(numbered(99))
        );
    })
    .await
    .expect("the stream test finished in time");
}

/// Why: while the caller is behind, the stream reads nothing from its socket, so the silence it would see then is its
/// own and not the peer's; and once the caller catches up, the peer may take a moment to resume what it held back.
/// Charging either to the peer would end a large replay's catch-up early under a slow GUI.
///
/// Spec: a replay that fills the caller's queue, held there for three bounds, then resumed a third of a bound after the
/// caller has read everything, reaches the caller whole and in order, with the end of the catch-up after its last
/// message and nowhere before.
#[tokio::test]
async fn the_bound_waits_while_the_caller_is_behind() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let (addr, mut accepted) = server().await;
        let config = ChannelConfig {
            ping_interval: Duration::from_secs(3600),
            replay_idle: Duration::from_millis(600),
            ..ChannelConfig::default()
        };
        let (_handle, mut events) =
            open_terminal(HelmEndpoint::new(addr, "test-secret"), attach("s1"), config);
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attaching)
        );
        let mut socket = accepted.recv().await.unwrap().socket;
        // `Attached` and all but the last of these fill the queue; the stream holds the last one for the caller.
        for n in 0..EVENT_QUEUE {
            send_numbered(&mut socket, n).await;
        }
        while events.events.len() < EVENT_QUEUE {
            tokio::task::yield_now().await;
        }
        // sleep-ok: the caller's deliberate stall, three bounds long; only real time passing shows the bound waited.
        tokio::time::sleep(Duration::from_millis(1800)).await;
        assert_eq!(
            events.events.len(),
            EVENT_QUEUE,
            "the premise: the queue is still full"
        );

        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attached(Attachment(1)))
        );
        for n in 0..EVENT_QUEUE {
            assert_eq!(
                next_event(&mut events).await,
                TerminalEvent::Output(numbered(n)),
                "message {n}"
            );
        }
        // The bound starts again as the caller begins reading, so the reads and this pause share the slack of 400 ms.
        // sleep-ok: the helm resuming a third of a bound after the caller caught up, which the bound must allow it
        tokio::time::sleep(Duration::from_millis(200)).await;
        send_numbered(&mut socket, EVENT_QUEUE).await;
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::Output(numbered(EVENT_QUEUE))
        );
        assert_eq!(next_event(&mut events).await, TerminalEvent::ReplayIdle);
    })
    .await
    .expect("the stream test finished in time");
}

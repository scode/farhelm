//! The stream's reconnect ladder in time, on tokio's paused clock, against a scripted dial instead of sockets.
//!
//! Like the channel's tests (`channel/tests.rs`), these run with `start_paused`: tokio jumps the clock to the next
//! timer whenever every task is idle, so the waits are asserted exactly (attempt 7 starts at precisely 30.5 s) with no
//! real wait. Real sockets cannot run under that clock, which would jump past their timeouts while the kernel is still
//! answering, so each attempt here is answered from a script of connect failures. Only failures can be scripted (a
//! socket needs a real connection), which is all a climb up the ladder is; what an attachment and a proven link do to
//! the ladder is pinned on real sockets in `terminal/tests.rs`.

use super::*;
use std::sync::{Arc, Mutex};

use ReconnectPhase::{Probing, Retrying};

/// The attempts a scripted stream made: when each started, on the paused clock, and the path it asked for.
type Attempts = Arc<Mutex<Vec<(Duration, String)>>>;

/// Open a stream for `attach` whose attempts fail with `script`, in order, and then never finish. Returns the
/// stream's handle and feed and the record of its attempts.
fn scripted(
    attach: TerminalAttach,
    script: Vec<ConnectError>,
) -> (TerminalHandle, TerminalEvents, Attempts) {
    let started = Instant::now();
    let attempts = Attempts::default();
    let script = Arc::new(Mutex::new(VecDeque::from(script)));
    let record = attempts.clone();
    let dial: Dial = Box::new(move |path: ApiPath| {
        record
            .lock()
            .unwrap()
            .push((started.elapsed(), path.as_str().to_owned()));
        let next = script.lock().unwrap().pop_front();
        Box::pin(async move {
            match next {
                Some(error) => Err(error),
                None => std::future::pending().await,
            }
        })
    });
    let (handle, events) = spawn_stream(attach, ChannelConfig::default(), dial);
    (handle, events, attempts)
}

/// The next `n` events, each bounded so a stream that stopped short fails instead of hanging (on the paused clock the
/// bound is virtual time, far past every wait on the ladder).
async fn events_now(events: &mut TerminalEvents, n: usize) -> Vec<TerminalEvent> {
    let mut seen = Vec::new();
    for _ in 0..n {
        match timeout(Duration::from_secs(3600), events.recv()).await {
            Ok(Some(event)) => seen.push(event),
            other => panic!("the stream stopped after {seen:?}: {other:?}"),
        }
    }
    seen
}

/// When each attempt started, in milliseconds since the stream opened.
fn offsets(attempts: &Attempts) -> Vec<u128> {
    let attempts = attempts.lock().unwrap();
    attempts.iter().map(|(at, _)| at.as_millis()).collect()
}

/// SPEC.md's terminal recovery is bounded retries, then periodic re-probing, visible in the terminal itself: a terminal
/// whose helm stays unreachable must climb the approved ladder (0.5, 1, 2, 4, 8 and 15 s), then probe every 30 s
/// without ever giving up, and tell its card about every wait and every attempt as it goes. This pins the attempts'
/// start times exactly, the full feed of `Reconnecting` states (each wait with its delay, each attempt in flight, the
/// count running past six into probing), and that only the first attach takes the displacing route. Without the
/// terminal's own ladder the attempts would follow the UI channel's 1, 2, 4, 8 s doubling; announced once per outage,
/// the feed would hold one state.
#[tokio::test(start_paused = true)]
async fn an_unreachable_terminal_climbs_the_ladder_then_probes_every_30_s() {
    let failures = (0..10)
        .map(|_| ConnectError::Failed("refused".into()))
        .collect();
    let (_handle, mut events, attempts) = scripted(attach("s1"), failures);
    let mut expected = vec![TerminalEvent::State(StreamState::Attaching)];
    let waits = [
        500, 1_000, 2_000, 4_000, 8_000, 15_000, 30_000, 30_000, 30_000,
    ];
    for (attempt, wait) in (1..).zip(waits) {
        let phase = if attempt <= 6 { Retrying } else { Probing };
        expected.push(reconnecting(attempt, 6, phase, Some(wait)));
        expected.push(reconnecting(attempt, 6, phase, None));
    }
    // The tenth failure's wait: shown only after its attempt was made, so the record below is complete.
    expected.push(reconnecting(10, 6, Probing, Some(30_000)));
    assert_eq!(events_now(&mut events, expected.len()).await, expected);
    assert_eq!(
        offsets(&attempts),
        [
            0, 500, 1_500, 3_500, 7_500, 15_500, 30_500, 60_500, 90_500, 120_500
        ]
    );
    let attempts = attempts.lock().unwrap();
    assert!(
        attempts[0].1.starts_with("/api/sessions/s1/term?"),
        "{:?}",
        attempts[0]
    );
    for (_, path) in &attempts[1..] {
        assert!(path.starts_with("/api/sessions/s1/term/unowned?"), "{path}");
    }
}

/// A 5xx answer to the upgrade is usually a helm restarting, so it is retried on the ladder like a connection that
/// never opened (decided by the migration's orchestrator on 2026-11-15); parking it as a refusal would leave the
/// terminal dead behind a Take control button after every helm restart. Every other status is the route's answer
/// about this attach (a revoked credential, a session that is gone) and still waits for the user, with no attempt
/// made by itself. Pinned at the edges of the 5xx range and for the statuses a helm gives, with the clock run far past
/// the ladder's first wait.
#[tokio::test(start_paused = true)]
async fn a_5xx_upgrade_answer_is_retried_and_any_other_refusal_waits_for_the_user() {
    for status in [500, 503, 599] {
        let script = vec![
            ConnectError::Refused { status },
            ConnectError::Refused { status },
        ];
        let (_handle, mut events, attempts) = scripted(attach("s1"), script);
        assert_eq!(
            events_now(&mut events, 4).await,
            [
                TerminalEvent::State(StreamState::Attaching),
                reconnecting(1, 6, Retrying, Some(500)),
                reconnecting(1, 6, Retrying, None),
                reconnecting(2, 6, Retrying, Some(1_000)),
            ],
            "{status}"
        );
        assert_eq!(offsets(&attempts), [0, 500], "{status}");
    }
    for status in [302, 400, 401, 403, 404, 409, 499, 600] {
        let (_handle, mut events, attempts) =
            scripted(attach("s1"), vec![ConnectError::Refused { status }]);
        assert_eq!(
            events_now(&mut events, 2).await,
            [
                TerminalEvent::State(StreamState::Attaching),
                TerminalEvent::State(StreamState::Refused { status }),
            ],
            "{status}"
        );
        // sleep-ok: paused clock: an observation window far past every wait on the ladder
        tokio::time::sleep(Duration::from_secs(600)).await;
        assert_eq!(offsets(&attempts), [0], "{status}: retried by itself");
    }
}

/// A stream opened without the intent to take its terminal (a tab a recovering view opens while it may have missed a
/// takeover) must not displace whoever holds it, so its first attach takes the non-displacing route; that the helm
/// then refuses it with the ordinary takeover notice is the helm's own contract. Only a later take-control displaces.
#[tokio::test(start_paused = true)]
async fn a_stream_that_does_not_take_control_first_attaches_without_displacing() {
    let unowned = TerminalAttach {
        take_control: false,
        ..attach("s1")
    };
    let script = vec![
        ConnectError::Refused { status: 404 },
        ConnectError::Failed("refused".into()),
    ];
    let (handle, mut events, attempts) = scripted(unowned, script);
    assert_eq!(
        events_now(&mut events, 2).await,
        [
            TerminalEvent::State(StreamState::Attaching),
            TerminalEvent::State(StreamState::Refused { status: 404 }),
        ]
    );
    handle.take_control();
    // `Attaching`, then the failed attempt's wait, which comes only once that attempt was made.
    assert_eq!(
        events_now(&mut events, 2).await,
        [
            TerminalEvent::State(StreamState::Attaching),
            reconnecting(1, 6, Retrying, Some(500)),
        ]
    );
    let paths: Vec<String> = attempts
        .lock()
        .unwrap()
        .iter()
        .map(|(_, path)| path.clone())
        .collect();
    assert_eq!(paths.len(), 2);
    assert!(
        paths[0].starts_with("/api/sessions/s1/term/unowned?"),
        "{}",
        paths[0]
    );
    assert!(
        paths[1].starts_with("/api/sessions/s1/term?"),
        "{}",
        paths[1]
    );
}

//! The reconnect policies: how long the UI channel and each terminal stream wait after a connection ends.
//!
//! Both kinds of connection share one rule about evidence: after a break, reconnect at once only if the connection that
//! just ended had proven the link worked by answering a heartbeat; otherwise wait. A helm or host that drops every
//! connection as soon as it opens therefore gets a wait, never a hot loop, while a link that was healthy and blipped
//! comes back immediately. The evidence is per connection: each new connection has to answer a heartbeat itself before
//! its own break may skip the wait. What counts as progress that starts the waits over is each caller's call: for the
//! UI channel, an answered heartbeat and the user's retry or take-back; for a terminal, an attachment and the user's
//! take-control. A terminal's "Reconnect now" is not one: it only ends the current wait, and the attempt keeps its
//! number.
//!
//! The waits themselves differ. The UI channel doubles from `reconnect_initial` to `reconnect_max` ([`Backoff`]).
//! A terminal stream climbs its own [`TerminalLadder`] (0.5, 1, 2, 4, 8 and 15 s, then a probe every 30 s forever),
//! the old app's schedule, which SPEC.md's "bounded retries, then periodic re-probing" describes and the terminal's
//! outage card counts out to the user ("attempt 3 of 6"). The terminal area asked for it, and the migration's
//! orchestrator approved it on 2026-11-15. A terminal never gives up: there is no failure state past the ladder, so a
//! terminal whose network comes back overnight is there again by itself.

use std::time::Duration;

use crate::ChannelConfig;

/// The UI channel's wait before the next connection attempt, doubling per failure between `reconnect_initial` and
/// `reconnect_max` from [`ChannelConfig`]. Terminal streams do not use it; they climb a [`Ladder`].
pub(crate) struct Backoff {
    initial: Duration,
    max: Duration,
    next: Duration,
}

impl Backoff {
    pub(crate) fn new(config: &ChannelConfig) -> Self {
        Backoff {
            initial: config.reconnect_initial,
            max: config.reconnect_max,
            next: config.reconnect_initial,
        }
    }

    /// The connection made progress, or the user asked again: the next wait starts small.
    pub(crate) fn reset(&mut self) {
        self.next = self.initial;
    }

    /// What to do after a connection ended. `proven` says whether that connection answered a heartbeat: then it
    /// reconnects at once (`None`) and the wait starts over. Otherwise the answer is the wait before the next attempt,
    /// and the wait after that doubles, up to the cap.
    pub(crate) fn after_break(&mut self, proven: bool) -> Option<Duration> {
        if proven {
            self.reset();
            return None;
        }
        let wait = self.next;
        self.next = (self.next * 2).min(self.max);
        Some(wait)
    }
}

/// A terminal stream's reconnect schedule: a bounded run of active retries, then probes at one interval forever.
///
/// Attempt `n` of an outage waits `retries[n - 1]`, or `probe_every` once the retries are spent. The default is the
/// approved schedule; tests shrink it through [`ChannelConfig::terminal_ladder`] so real sockets reconnect promptly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLadder {
    /// The wait before each active retry, in order. Its length is the bound the outage card counts against.
    pub retries: Vec<Duration>,
    /// The wait before every attempt after the last retry.
    pub probe_every: Duration,
}

impl Default for TerminalLadder {
    fn default() -> TerminalLadder {
        TerminalLadder {
            retries: [500, 1_000, 2_000, 4_000, 8_000, 15_000]
                .into_iter()
                .map(Duration::from_millis)
                .collect(),
            probe_every: Duration::from_secs(30),
        }
    }
}

/// Which regime a terminal's recovery is in: the bounded active retries, or the background probes that follow them
/// and never end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectPhase {
    /// One of the ladder's active retries: the card counts them ("attempt 3 of 6").
    Retrying,
    /// Past the retries: one probe per `probe_every`, forever, which the card shows as a cadence, not a count.
    Probing,
}

/// Where a terminal's automatic recovery stands: what its outage card shows.
///
/// A stream reports one of these when it starts waiting for an attempt (`next_attempt_in` set) and again when the
/// attempt starts (`None`), so a card always shows the attempt it is on and never counts a stale wait down. An
/// attempt the stream makes at once (after a link that had proven itself) is reported only as in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectStatus {
    /// The attempt this is, counted from 1 per outage. It keeps counting while probing, past `of`; the old card
    /// stopped showing it there, because a rising number reads as escalating failure rather than patience.
    pub attempt: u32,
    /// How many active retries the ladder has before probing: 6 with the default ladder.
    pub of: u32,
    pub phase: ReconnectPhase,
    /// The wait before this attempt starts, as it was scheduled (it is not reported again as it counts down).
    /// `None` while the attempt is in flight.
    pub next_attempt_in: Option<Duration>,
}

/// One stream's position on its [`TerminalLadder`] during an outage.
///
/// Pure bookkeeping, shared by the real stream and the fake (`crate::fake::terminal`) so the numbers a GUI test sees
/// are the app's. When the stream consults it is each one's own code, which the parity test compares.
pub(crate) struct Ladder {
    config: TerminalLadder,
    /// Attempts scheduled in this outage. The stream's first attach (and the one after a take-control) is not one:
    /// it is `Attaching`, and only its failure starts the count.
    made: u32,
}

impl Ladder {
    /// A ladder with no attempt made yet: the first break schedules attempt 1.
    pub(crate) fn new(config: TerminalLadder) -> Ladder {
        Ladder { config, made: 0 }
    }

    /// The outage is over (an attachment went live) or the user started over (take-control): the next break starts
    /// at the bottom of the ladder.
    pub(crate) fn reset(&mut self) {
        self.made = 0;
    }

    /// Schedule the next attempt, after a break or a failed attempt. `at_once` says the connection that just ended
    /// proved the link (it answered a heartbeat): the attempt then goes without its wait. Proof needs an attachment,
    /// which resets the ladder, so it only ever skips the first rung; the next failure waits the second.
    pub(crate) fn schedule(&mut self, at_once: bool) -> ReconnectStatus {
        self.made = self.made.saturating_add(1);
        let rung = usize::try_from(self.made - 1)
            .ok()
            .and_then(|index| self.config.retries.get(index));
        let (phase, wait) = match rung {
            Some(wait) => (ReconnectPhase::Retrying, *wait),
            None => (ReconnectPhase::Probing, self.config.probe_every),
        };
        ReconnectStatus {
            attempt: self.made,
            of: u32::try_from(self.config.retries.len()).unwrap_or(u32::MAX),
            phase,
            next_attempt_in: (!at_once).then_some(wait),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backoff() -> Backoff {
        Backoff::new(&ChannelConfig {
            reconnect_initial: Duration::from_secs(1),
            reconnect_max: Duration::from_secs(8),
            ..ChannelConfig::default()
        })
    }

    /// A peer that keeps failing must be retried less and less often, up to the cap and no further, or a dead helm or
    /// host is hammered.
    #[test]
    fn unproven_breaks_double_the_wait_up_to_the_cap() {
        let mut b = backoff();
        let waits: Vec<u64> = (0..6)
            .map(|_| b.after_break(false).map_or(0, |w| w.as_secs()))
            .collect();
        assert_eq!(waits, [1, 2, 4, 8, 8, 8]);
    }

    /// A connection that answered a heartbeat proved the link worked, so its break reconnects at once and the wait
    /// starts over: a healthy link that blipped comes back promptly, and the next failure waits only the initial step.
    #[test]
    fn a_proven_break_reconnects_at_once_and_starts_the_wait_over() {
        let mut b = backoff();
        assert_eq!(b.after_break(false), Some(Duration::from_secs(1)));
        assert_eq!(b.after_break(false), Some(Duration::from_secs(2)));
        assert_eq!(b.after_break(true), None);
        assert_eq!(b.after_break(false), Some(Duration::from_secs(1)));
    }

    /// Progress the caller recognizes (a terminal attaching, a user's retry) resets the wait without skipping it.
    #[test]
    fn a_reset_starts_the_wait_over_without_skipping_it() {
        let mut b = backoff();
        b.after_break(false);
        b.after_break(false);
        b.reset();
        assert_eq!(b.after_break(false), Some(Duration::from_secs(1)));
    }

    /// `(attempt, phase, wait in ms)` of one status, for comparing a schedule against literals.
    fn rung(status: ReconnectStatus) -> (u32, ReconnectPhase, Option<u128>) {
        assert_eq!(status.of, 6, "the default ladder has six retries");
        (
            status.attempt,
            status.phase,
            status.next_attempt_in.map(|wait| wait.as_millis()),
        )
    }

    /// The terminal's schedule is a user-visible cadence settled by a decision (SPEC.md's bounded retries, then
    /// periodic re-probing; the old app's 0.5 to 15 s ladder and 30 s probe), so it is written out as literals: a test
    /// that recomputed it from the same constant would pass for any rung someone quietly retuned. Specifies six
    /// retries numbered 1 to 6 of 6, then probes every 30 s that keep counting and never stop.
    #[test]
    fn the_terminal_ladder_retries_six_times_then_probes_every_30_s() {
        use ReconnectPhase::{Probing, Retrying};
        let mut ladder = Ladder::new(TerminalLadder::default());
        let schedule: Vec<_> = (0..9).map(|_| rung(ladder.schedule(false))).collect();
        assert_eq!(
            schedule,
            [
                (1, Retrying, Some(500)),
                (2, Retrying, Some(1_000)),
                (3, Retrying, Some(2_000)),
                (4, Retrying, Some(4_000)),
                (5, Retrying, Some(8_000)),
                (6, Retrying, Some(15_000)),
                (7, Probing, Some(30_000)),
                (8, Probing, Some(30_000)),
                (9, Probing, Some(30_000)),
            ]
        );
    }

    /// A link that proved itself skips only its first wait, so a healthy terminal that blips is back at once, while a
    /// path that keeps failing after that still climbs the ladder from its second rung; and a reset (an attachment, a
    /// take-control) starts the next outage at the bottom.
    #[test]
    fn a_proven_link_skips_the_first_wait_and_a_reset_starts_over() {
        use ReconnectPhase::Retrying;
        let mut ladder = Ladder::new(TerminalLadder::default());
        assert_eq!(rung(ladder.schedule(true)), (1, Retrying, None));
        assert_eq!(rung(ladder.schedule(false)), (2, Retrying, Some(1_000)));
        ladder.reset();
        assert_eq!(rung(ladder.schedule(false)), (1, Retrying, Some(500)));
    }
}

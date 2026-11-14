//! The reconnect policy the UI channel and every terminal stream share: how long to wait after a connection ends.
//!
//! Both kinds of connection follow one rule, which they used to implement separately (and had already started to
//! differ in): after a break, reconnect at once only if the connection that just ended had proven the link worked by
//! answering a heartbeat; otherwise wait, and double the wait while attempts keep failing, up to a cap. A helm or host
//! that drops every connection as soon as it opens therefore gets a backoff, never a hot loop, while a link that was
//! healthy and blipped comes back immediately.
//!
//! The evidence is per connection: each new connection has to answer a heartbeat itself before its own break may skip
//! the wait. What counts as progress that resets the wait to its start is the caller's call (the UI channel's
//! answered heartbeat, a terminal's attachment, a user's explicit retry) through [`Backoff::reset`].

use std::time::Duration;

use crate::ChannelConfig;

/// The wait before the next connection attempt, doubling per failure between `reconnect_initial` and
/// `reconnect_max` from [`ChannelConfig`].
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
}

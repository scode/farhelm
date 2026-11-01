//! The synchronized-output (mode 2026) deadline, kept on a clock the caller
//! supplies.
//!
//! While a program holds a synchronized update open, vte buffers its output
//! and draws nothing until the update ends. If the end never comes, vte's
//! safety rule is to give up after 150 ms and draw what it has, but vte only
//! records that deadline: nothing wakes it. alacritty's own event loop polls
//! the deadline; the supervisor has no such loop, so the emulator exposes the
//! deadline and its owner wakes on it (`Emulator::sync_deadline`,
//! `Emulator::flush_sync_if_due`).
//!
//! vte's stock timer, `StdSyncHandler`, reads `Instant::now()` itself when an
//! update starts, which would make the deadline untestable without real
//! waiting. [`SyncClock`] instead notes only that a timeout was armed and for
//! how long, and the emulator turns that into a deadline from the `now` its
//! caller passed to the feed that armed it. Every byte of one feed is treated
//! as arriving at that `now`, which is what a feed is: one read from the pty.

use std::cell::Cell;
use std::time::{Duration, Instant};

use alacritty_terminal::vte::ansi::Timeout;

/// vte's [`Timeout`] for a synchronized update, with the deadline resolved by
/// the emulator from the caller's clock rather than from `Instant::now()`.
///
/// Invariant between feeds: `armed` is `None` (the emulator resolves it right
/// after every call into the parser), and `deadline` is `Some` exactly when
/// `pending` is true.
///
/// The two time fields are `Cell`s because vte hands out only a shared
/// reference to its timer (`Processor::sync_timeout`), and resolving happens
/// after vte returns.
#[derive(Debug, Default)]
pub struct SyncClock {
    /// Whether vte is inside a synchronized update, buffering its bytes.
    pending: bool,
    /// A timeout vte armed (or re-armed, on a new update start inside the
    /// buffer) during the current call, not yet turned into a deadline.
    armed: Cell<Option<Duration>>,
    /// When the open update must be drawn even if it never ends.
    deadline: Cell<Option<Instant>>,
}

impl SyncClock {
    /// Turn a timeout armed during the call that just ended into a deadline
    /// measured from `now`, the time the caller attributed to that call.
    pub(crate) fn resolve(&self, now: Instant) {
        if let Some(duration) = self.armed.take() {
            self.deadline.set(Some(now + duration));
        }
    }

    /// When the open synchronized update must be flushed, if one is open.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.deadline.get()
    }
}

impl Timeout for SyncClock {
    fn set_timeout(&mut self, duration: Duration) {
        self.pending = true;
        self.armed.set(Some(duration));
    }

    fn clear_timeout(&mut self) {
        self.pending = false;
        self.armed.set(None);
        self.deadline.set(None);
    }

    fn pending_timeout(&self) -> bool {
        self.pending
    }
}

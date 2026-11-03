//! The supervisor's terminal emulator: for each terminal that runs in a
//! terminal host rather than tmux, the one place its output is interpreted.
//!
//! Farhelm is replacing tmux and xterm.js for new terminals. The terminal
//! host keeps the pty and journals raw output; the supervisor feeds that
//! output to one [`Emulator`] per terminal, which answers the program's
//! terminal queries, tracks the modes that input encoding needs, gives agent
//! status detection the screen text and title it reads from tmux today, and
//! is snapshotted so that a restarted supervisor can rebuild it. It is
//! `alacritty_terminal` 0.26.0 with Farhelm's small patches
//! (`vendor/README.md`), and this crate is the policy around it.
//!
//! - [`Emulator`] (`emulator`): feeding output, resizing, the
//!   synchronized-output deadline, the clean-boundary check, and the read
//!   accessors.
//! - [`FeedMode`], [`Effects`], [`Notice`] (`effects`): which side effects a
//!   feed may have, depending on whether its bytes are live or replayed.
//! - `queries`: the answers to terminal queries, and how they depart from
//!   alacritty's own.
//! - `theme`: Farhelm's terminal colors, for color query answers.
//! - [`Modes`] (`modes`): bracketed paste, mouse, focus, cursor-key and keypad
//!   modes, alternate screen, cursor visibility.
//! - `screen`: screen text shaped like tmux's capture.
//! - `sync`: the synchronized-output deadline on the caller's clock.
//! - [`snapshot_split_candidate`] (`split`): where in a read to try a
//!   snapshot, as a hint the emulator then confirms.
//!
//! Hostile output is bounded per piece of state by the vendored caps
//! (combining characters per cell, OSC payload, hyperlink length, title
//! length) and by vte's own 2 MiB limit on a synchronized update;
//! `tests/hostile.rs` checks them through this crate. A bound on a whole
//! terminal's memory, including how many distinct hyperlinks it may hold,
//! is not here yet: it belongs with the snapshot format and the per-terminal
//! memory budget measured against it.
//!
//! The crate depends on no other Farhelm crate. It re-exports
//! `alacritty_terminal` so that callers reading [`Emulator::term`] name the
//! same version.

mod effects;
mod emulator;
mod modes;
mod queries;
mod screen;
mod split;
mod sync;
mod theme;

pub use alacritty_terminal;
pub use effects::{ClipboardTarget, Collected, Effects, FeedMode, Listener, Notice};
pub use emulator::{Emulator, Size};
pub use modes::{Modes, MouseEncoding, MouseReporting};
pub use split::snapshot_split_candidate;

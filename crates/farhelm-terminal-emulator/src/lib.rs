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
//! - [`Snapshot`], [`Restored`] (`snapshot`): an emulator's state as a blob
//!   for the terminal host to keep, restoring an emulator from one, and the
//!   Restart seeds. Every format version ever shipped stays readable; the
//!   module docs say how, and the format itself is in `snapshot::v1`.
//!
//! Hostile output is bounded per piece of state by the vendored caps
//! (combining characters per cell, OSC payload, hyperlink length, title
//! length), for all of a terminal's cell extras together by its extra
//! storage, and by vte's own 2 MiB limit on a synchronized update;
//! `tests/hostile.rs` checks them through this crate.
//!
//! # Memory budget
//!
//! One emulator at Farhelm's full scrollback (12,000 lines of history and 60
//! on the screen, 200 columns) is budgeted at **80 MiB for ordinary output**,
//! and hostile output is **bounded at about 190 MiB**. So a supervisor holds at
//! least 12 terminals per GiB of memory doing ordinary work, and at least 5
//! per GiB if every one of them is hostile at once. Measured by
//! `tests/memory.rs` (ignored tests, run in release under nextest; Linux,
//! 2026-11):
//!
//! | Output | Resident | Extra storage charged |
//! | --- | --- | --- |
//! | ordinary text with some colors and wide characters | 56 MiB | 0 |
//! | sixteen combining marks and its own underline color on every cell | 120 MiB | 64 MiB |
//! | a new maximal hyperlink before every character | 119 MiB | 63.5 MiB |
//! | a saved cursor with a link restored and changed before every character | 88 MiB | 64 MiB |
//! | a full title stack of maximal titles and an OSC left open | 94 MiB | 0 |
//! | a wide character's head erased on every other cell (a snapshot of fixups) | 56 MiB | 0 |
//!
//! The bound is the sum of what each cap allows:
//!
//! - The grid: 24 bytes a cell, 55 MiB at that size (56 MiB measured,
//!   with each row's own allocation). Ordinary output costs about this.
//! - Cell extras, the allocations that hold a cell's combining marks,
//!   underline color and hyperlink when they vary from cell to cell: at most
//!   64 MiB, the vendored extra storage limit (`MAX_EXTRA_STORAGE_BYTES`),
//!   which no path can exceed. It charges each extra 160 bytes, at least
//!   what it takes. Without it, the combining-marks case takes 387 MiB and
//!   the hyperlink case about 6 GB; with it, past the limit combining marks
//!   are dropped, underline colors fall back to the default and links end.
//!   64 MiB holds an extra for about 400,000 cells, a sixth of the full
//!   scrollback, which real text stays well within.
//! - Outside the grid, 38 MiB measured: the title stack (4096 titles of up
//!   to 4096 bytes, 16 MiB), an OSC never ended (buffered up to 10 MiB, in a
//!   buffer whose capacity can reach 16 MiB), and vte's 2 MiB buffer for a
//!   synchronized update. That is 34 MiB of buffers; the rest is the
//!   allocator's overhead on 4096 separate titles.
//! - A snapshot while it is taken: about twice the blob, so about 32 MiB at
//!   the host's 16 MiB limit (32.3 MiB measured for a blob of fixups). Sizes
//!   are found by measuring first, so a hostile state never paints its whole
//!   stream at once; then the stream and the cell fixups are painted into
//!   buffers reserved to the sizes measured, and laid out into the blob,
//!   which is reserved to its size too. Ordinary output's takes about 7 MiB.
//!
//! 56 + 64 + 38 + 32 is 190 MiB. Each terminal host process is bounded on
//! its own, by its journal cap and `MAX_SNAPSHOT`
//! (`crates/terminal-host/SPEC.md`).
//!
//! The crate depends on no other Farhelm crate. It re-exports
//! `alacritty_terminal` so that callers reading [`Emulator::term`] name the
//! same version.

mod effects;
mod emulator;
mod modes;
mod queries;
mod screen;
mod snapshot;
mod split;
mod sync;
mod theme;

pub use alacritty_terminal;
pub use effects::{ClipboardTarget, Collected, Effects, FeedMode, Listener, Notice};
pub use emulator::{Emulator, Size};
pub use modes::{Modes, MouseEncoding, MouseReporting};
pub use snapshot::{
    FORMAT_VERSION, MAX_RESTORE_CELLS, MAX_SIDE, MIN_SNAPSHOT_LIMIT, NotClean, Restored, Snapshot,
    SnapshotError, WidthTable, history_seed_from_lines,
};
pub use split::snapshot_split_candidate;

/// The answer to XTVERSION (`CSI > q`): `DCS > | Farhelm <version> ST`, the
/// version being the workspace's, which is `0.0.0-unreleased` outside a
/// release build. See `queries` for why the emulator answers as Farhelm.
pub const VERSION_REPORT: &str = concat!("\x1bP>|Farhelm ", env!("CARGO_PKG_VERSION"), "\x1b\\");

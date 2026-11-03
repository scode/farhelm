//! Proof that the terminal emulator can stand in for tmux where the
//! supervisor reads a terminal: screen text and title for agent status
//! detection, and the answers agents get to their terminal queries.
//!
//! Status detection reads each agent's screen with `tmux capture-pane -p`
//! and its title with `#{pane_title}`, and its per-agent readers were written
//! against that text. Terminals in a terminal host have no tmux; their text,
//! title and query answers come from `farhelm-terminal-emulator` instead.
//! These tests feed the same bytes to the pinned tmux 3.7c, configured as the
//! supervisor configures its own servers, and to the emulator, and compare
//! the two at many cut points (`harness` explains how, and which documented
//! differences it allows):
//!
//! - `generated`: hand-picked and seeded random output covering wide and
//!   combining characters, emoji, wraps, trailing blanks, colors, the
//!   alternate screen, scroll regions, erase operations, titles and
//!   synchronized updates.
//! - `fixtures`: the frozen captured agent screens, repainted, with each
//!   screen's reader run on the emulator's text.
//! - `queries`: the startup probes recorded agents send, and every query in
//!   the emulator's answer table.
//! - `recordings`: real agent sessions replayed, with the readers run on
//!   both sides at every cut. Manual (`#[ignore]`), because recordings stay
//!   out of the repository.
//!
//! Every test needs the pinned tmux first on `PATH` and says `SKIPPED`
//! without it. Each starts one private tmux server in a directory of its
//! own and stops only that server.

mod fixtures;
mod generated;
mod harness;
mod queries;
mod recordings;

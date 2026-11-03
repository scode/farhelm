//! A terminal host: one long-lived process that owns a pty and the program in
//! it, journals the program's output in memory, and serves the journal,
//! input and resizes to one client at a time. `SPEC.md` beside this crate is
//! the contract; this crate's public API is the whole boundary.
//!
//! - [`main_entry`]: run a host from a command line (the `terminal-host`
//!   binary, or an embedding program's subcommand).
//! - [`protocol`]: the wire format, pure, shared by hosts and clients.
//!
//! `journal` is public only so its model tests (`tests/journal_model.rs`)
//! can drive it; it is the host's internals, not part of the boundary, and
//! no client may depend on it.

// Coverage measurement (nightly, `cargo llvm-cov`) cannot see code that runs
// between a fork and the exec or `_exit` after it; that code is excluded with
// `coverage(off)` and exercised by the real-pty tests instead.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod host;
#[doc(hidden)]
pub mod journal;
pub mod protocol;
mod spawn;

pub use spawn::main_entry;

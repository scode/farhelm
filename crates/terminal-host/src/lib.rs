//! A terminal host: one long-lived process that owns a pty and the program in
//! it, journals the program's output in memory, and serves the journal,
//! input and resizes to one client at a time. `SPEC.md` beside this crate is
//! the contract; this crate's public API is the whole boundary.
//!
//! - [`protocol`]: the wire format, pure, shared by hosts and clients.
//!
//! `journal` is public only so its model tests (`tests/journal_model.rs`)
//! can drive it; it is the host's internals, not part of the boundary, and
//! no client may depend on it.
//!
//! The process itself (its start, the pty, the event loop) is not here yet.

#[doc(hidden)]
pub mod journal;
pub mod protocol;

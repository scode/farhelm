//! Read-only preparation for `farhelm uninstall`.
//!
//! The command wiring deliberately comes later. Keeping ownership discovery
//! separate means the eventual prompt, runtime checks, and remover all start
//! from one bounded plan instead of independently deciding which paths belong
//! to an installation.

#[allow(dead_code)] // The CLI is wired in the following uninstall change.
pub(crate) mod ownership;

//! Launch templates and everything that launches from them (SPEC.md, "Creation": Launch templates, Quick launch,
//! Template number shortcuts, Quick-launch sequences).
//!
//! The session launcher owns its draft, its dialog and its one submit path (`crate::launcher`); this module owns what a
//! template does to that draft and everything that starts from a template. Built in pieces. So far:
//!
//! - [`apply`]: applying one or more templates to a launcher draft, all or nothing, through the draft's own hand-edit
//!   methods, plus the launcher snapshot that Save as template reads too. No GPUI types.
//!
//! Templates, the ⚡ mark and the number keys are read from the store the helm pushes, never with a request per use.
//! SPEC.md says the shortcuts read "the mark and templates fresh on each use, so an agent's or the command line's
//! change takes effect at once"; the old app needed a request for that because it had no pushed copy. Here every
//! change reaches the store as the helm sees it, and a change this window makes is pushed before its reply, so the
//! store is that fresh read.

pub mod apply;

#[cfg(test)]
mod apply_tests;

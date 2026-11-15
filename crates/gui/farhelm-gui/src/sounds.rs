//! Status sounds: which sound an observation of the fleet calls for, and the sounds themselves, as plain Rust.
//!
//! SPEC.md, "Status" (the sounds paragraph). Each open client plays a sound when a session starts waiting, when a new
//! agent approval request arrives, and when a session goes from running to idle, subject to three switches and a few
//! silencing rules. This module holds the two framework-free halves of that, so both are tested without a window:
//!
//! - [`rules`]: given the previous and the current observation, at most one [`Sound`] to play.
//! - [`synth`]: each [`Sound`] rendered once into a small mono WAV in memory, for a playback seam to hand to the
//!   platform.
//!
//! Ported from the old Dioxus UI at the reference commit: `crates/farhelm-ui/src/sounds.rs` (`History`,
//! `status_words`, `StatusScope`) and `crates/farhelm-ui/assets/sounds.js` (`detectEvents`, `chooseSound`, the `NOTES`
//! table and the envelopes in `createPlayer`). The old app split the rules between Rust and JavaScript and played
//! through Web Audio's oscillators; here the rules are one Rust type and the oscillators are replaced by rendering the
//! same notes into PCM samples.
//!
//! NOTE: Nothing here observes the store, reads the switches from the helm's preferences, or plays anything. Building an
//! observation from pushed state, deciding which session is quiet, and handing the WAV bytes to the platform belong to
//! the GPUI owner that uses these halves. Nothing here may depend on `gpui` or GPUI Kit either: that is what keeps
//! the rules and the synthesizer testable as plain functions.

pub mod rules;
pub mod synth;

/// One of the three status sounds, declared from most to least urgent.
///
/// The order is the priority SPEC.md gives ("approval, waiting, then turn finished") and the old app's `PRIORITY`
/// list: when one observation calls for several, only the most urgent enabled, non-quiet one plays
/// ([`rules::SoundRules::observe`]). [`Sound::PRIORITY`] is that order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    /// A new agent approval request arrived: the old app's two-note ding-dong.
    Approval,
    /// A session started waiting for the user's answer: the old app's rising two-note bell.
    Waiting,
    /// A session went from running to idle: the old app's single soft pluck. Off by default.
    TurnFinished,
}

impl Sound {
    /// Every sound, most urgent first: the order the rules try them in.
    pub const PRIORITY: [Sound; 3] = [Sound::Approval, Sound::Waiting, Sound::TurnFinished];
}

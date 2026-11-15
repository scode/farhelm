//! Status sounds: which sound an observation of the fleet calls for, the sounds themselves, the owner that plays them,
//! and the seam a platform player plugs into.
//!
//! SPEC.md, "Status" (the sounds paragraph). Each open client plays a sound when a session starts waiting, when a new
//! agent approval request arrives, and when a session goes from running to idle, subject to three switches and a few
//! silencing rules. The parts:
//!
//! - [`rules`]: given the previous and the current observation, at most one [`Sound`] to play. Plain Rust.
//! - [`synth`]: each [`Sound`] rendered once into a small mono WAV in memory. Plain Rust.
//! - [`StatusSounds`] ([`owner`]): the one owner that decides when a sound plays (SPEC_impl.md, "Notices: the notice
//!   center", leaves the three sounds to it). It builds an observation from the current UI session's store on every
//!   change, runs the rules and hands the chosen sound to the player. The main view keeps it for the window's life.
//! - The player seam ([`Player`], [`install_player`]): what actually makes the sound. The bootstrap installs the player
//!   the host hands it (`bootstrap::Host::player`), and a host without one plays nothing; tests install a fake that
//!   records what it was asked to play.
//!
//! Ported from the old Dioxus UI at the reference commit: `crates/farhelm-ui/src/sounds.rs` (`History`,
//! `status_words`, `StatusScope`, `SessionSounds`) and `crates/farhelm-ui/assets/sounds.js` (`detectEvents`,
//! `chooseSound`, the `NOTES` table and the envelopes in `createPlayer`). The old app split the rules between Rust and
//! JavaScript and played through Web Audio's oscillators; here the rules are one Rust type and the oscillators are
//! replaced by rendering the same notes into PCM samples.
//!
//! The switches are the helm's shared preferences (`sound_waiting`, `sound_approval`, `sound_turn_finished`), which
//! the old app kept per device: the maintainer decided on 2026-11-13 that every setting the old app kept per device
//! becomes a helm preference. Settings → Notifications draws them (`crate::settings`), and [`rules::Switches`] reads
//! them, unset fields included, for both.
//!
//! [`rules`] and [`synth`] must not depend on `gpui` or GPUI Kit: that is what keeps them testable as plain functions.

use std::rc::Rc;

use gpui::{App, Global};

pub mod owner;
pub mod rules;
pub mod synth;

pub use owner::StatusSounds;

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

// ---- The player seam ------------------------------------------------------------------------------------------------

/// Makes a sound on the platform: the one place sound leaves the app.
///
/// [`StatusSounds`] calls it at most once per observation, with the sound and its rendered WAV file ([`synth::wav`],
/// rendered once per process). Playback is best effort and fire-and-forget, as the old Web Audio player was: an
/// implementation must return at once, must not report failure back (a sound that could not play is lost, never
/// queued or retried), and is never asked whether the previous sound has finished.
pub trait Player {
    /// Play `sound`, whose complete mono 16-bit PCM WAV file is `wav`.
    fn play(&self, sound: Sound, wav: &'static [u8]);
}

/// The installed player, if any.
struct InstalledPlayer(Rc<dyn Player>);

impl Global for InstalledPlayer {}

/// Make `player` the app's player from now on, replacing any installed before.
///
/// Optional by design: the bootstrap installs one only when the host passes one, and it is looked up directly rather
/// than through `bootstrap::installed`, which would fail a debug build without one: with no player installed the owner
/// still decides every sound and plays nothing. The desktop app hands its platform player to the bootstrap, which
/// installs it; tests install a fake.
pub fn install_player(player: Rc<dyn Player>, cx: &mut App) {
    cx.set_global(InstalledPlayer(player));
}

/// The installed player, for the bootstrap's test that the host's player is the one installed. It only reads.
#[cfg(test)]
pub(crate) fn installed_player(cx: &App) -> Option<Rc<dyn Player>> {
    cx.try_global::<InstalledPlayer>()
        .map(|InstalledPlayer(player)| player.clone())
}

/// Hand `sound` to the installed player, rendering its WAV file the first time. Nothing without a player, which also
/// skips the rendering.
fn play(sound: Sound, cx: &App) {
    if let Some(InstalledPlayer(player)) = cx.try_global::<InstalledPlayer>() {
        player.play(sound, synth::wav(sound));
    }
}

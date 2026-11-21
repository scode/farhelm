//! Values the installed app hands the window for the upgrade highlights dialog.
//!
//! The desktop owns eligibility and the fetch. The window receives display text and only sends a
//! shared preference write after an explicit skip or dismissal.

use std::future::Future;
use std::pin::Pin;

use serde::Deserialize;

/// An installed stable app can check the notes independently of the updater's automatic setting.
pub struct UpgradeNotesHost {
    /// The version that a dismissal records, regardless of the range the site served.
    pub running: String,
    /// Start a check with the saved baseline and whether the user wants highlights.
    pub check: UpgradeNotesCheck,
}

/// A window can await the desktop's result without joining the updater's worker or owning an HTTP client.
pub type UpgradeNotesCheck = Box<dyn Fn(Option<String>, bool) -> UpgradeNotesAnswer>;

/// Desktop checks settle on the window's executor even though the bounded read is on a worker.
pub type UpgradeNotesAnswer = Pin<Box<dyn Future<Output = anyhow::Result<UpgradeNotes>>>>;

/// The three successful decisions the window can act on. A fetch or decode failure returns an error instead.
pub enum UpgradeNotes {
    /// The user already saw or skipped the running version.
    Current,
    /// The user turned highlights off, so this upgrade counts as skipped.
    Skipped,
    /// Newest first. The list includes the running release.
    Show(Vec<ReleaseHighlights>),
}

/// Finalized highlights of one release, in the same order as the website.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseHighlights {
    pub version: String,
    pub date: String,
    pub highlights: Vec<HighlightCategory>,
    pub additional_changes: u64,
}

/// One changelog category from the published release section.
#[derive(Clone, Debug, Deserialize)]
pub struct HighlightCategory {
    pub name: String,
    pub entries: Vec<Highlight>,
}

/// The app renders text runs and one optional reference to the website's screenshot disclosure.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Highlight {
    pub pieces: Vec<HighlightPiece>,
    pub screenshot_url: Option<String>,
}

/// One piece of display text. Styling cannot carry markup or choose a link destination.
#[derive(Clone, Debug, Deserialize)]
pub struct HighlightPiece {
    pub style: HighlightStyle,
    pub text: String,
}

/// Future styles still render their text in older app builds.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HighlightStyle {
    Bold,
    Code,
    #[serde(other)]
    Plain,
}

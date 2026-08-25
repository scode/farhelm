//! The `farhelm-desktop` binary: a native window onto a helm that runs
//! inside it.
//!
//! ## Why this crate is a shell and nothing else
//!
//! D6 ships the Mac GUI as two bare binaries — `farhelm` (CLI, helm,
//! supervisor) and `farhelm-desktop` (this one) — installed side by side in
//! `~/.local/bin`, with no `.app` bundle around either. There is no code here
//! because there is nothing this binary does that the `farhelm-ui` bin does
//! not also have to do: both call [`farhelm_ui::desktop::run`].
//!
//! That shared body is what makes the Xvfb smoke worth running. It exercises
//! the same desktop implementation, but not the same artifact: the smoke
//! builds `farhelm-ui` for Linux, while the release builds THIS package for
//! macOS. What each side still owes its own validation is the thin part —
//! this wrapper, the packaging, and the platform — which is why
//! `docs/manual-mac-checklist.md` exists.
//!
//! It is a SEPARATE crate rather than a second `[[bin]]` in `farhelm-ui`
//! because a package can turn on a dependency's feature and a bin target
//! cannot: `farhelm-ui`'s desktop renderer is a feature, and only a package
//! depending on `farhelm-ui` with `features = ["desktop"]` gets it
//! unconditionally. Keeping that package out of the workspace's
//! `default-members` is then what keeps WebKit out of every ordinary build.

fn main() -> anyhow::Result<()> {
    farhelm_ui::desktop::run()
}

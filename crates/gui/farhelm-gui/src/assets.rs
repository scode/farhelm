//! The app's bundled assets: interface fonts and icon SVGs, compiled into the binary.
//!
//! Everything the GUI draws from a file comes from here, never from the filesystem at run time, so a build carries its
//! whole look and a headless test renders the same pixels on every machine. The maintainer decided on 2026-11-13 that
//! the fonts the shipped app uses stay embedded in the binary this way (fonts needed only by tests are another matter).
//! Fonts are registered with the text system by the bootstrap ([`crate::bootstrap::install`]); icons are served to
//! GPUI's `svg()` element through [`Assets`].
//!
//! Inter is the old app's interface face (`--font-ui` in its stylesheet), and JetBrains Mono Nerd Font its terminal
//! face. The old app shipped only the terminal's regular and bold faces and let the browser slant the regular one for
//! italic text. GPUI does not synthesize an oblique, so the Italic and Bold Italic faces are bundled as well (the
//! migration's orchestrator decided on 2026-11-13 to bundle them so terminal italics stay slanted, at about 5 MB of
//! binary size). The Inter faces and the terminal's regular and bold faces are lossless decompressions of the WOFF2
//! files the old app shipped at the reference commit; the two italic faces are the TTFs of the same Nerd Fonts release
//! those came from, unchanged. `THIRD_PARTY_NOTICES.md` has their provenance and licenses ("Inter interface font",
//! "JetBrains Mono terminal font").
//!
//! The Close and Delete icons are the old app's own action glyphs, copied from its icon module at the reference commit
//! (14-unit box, 1.2 stroke, round caps and joins). The More icon is a new drawing: the old app wrote the "⋯" character
//! in text. The four status shapes are the old stylesheet's `.status-dot` shapes (an 8-pixel box: a filled circle, a
//! triangle cut with `clip-path`, a diamond, and a ring with a 1.5-pixel border), drawn as SVG because GPUI has no
//! clip path; the view tints them with the status color, as the old dot took `currentColor`. The Spinner is the old
//! stylesheet's `.delete-spinner`, a 12-pixel ring whose top quarter is solid and the rest 30% of the same color (its
//! border in `color-mix(in srgb, var(--accent) 30%, transparent)` with `border-top-color: var(--accent)`); the 30% is
//! the SVG's stroke opacity, so the view still tints it with one color.

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

/// The interface font family name, as the bundled faces declare it.
pub const UI_FONT: &str = "Inter";

/// The interface faces the GUI uses: regular text and the semibold used for dialog titles and row titles.
pub fn ui_fonts() -> Vec<Cow<'static, [u8]>> {
    vec![
        Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Regular.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Inter-SemiBold.ttf")),
    ]
}

/// The terminal font family name, as the bundled faces declare it. Bold and italic requests must use this family so
/// the text system picks the matching bundled face instead of substituting another font.
pub const TERMINAL_FONT: &str = "JetBrainsMono Nerd Font";

/// The terminal faces: regular, bold, italic and bold italic, one for every combination of the bold and italic
/// attributes a cell can carry. All four share the same advance and vertical metrics, so a cell is the same size
/// whatever its style. Dropping a face does not fail anything at run time: the text system quietly draws its style in
/// the nearest face that is left (italic upright, say).
pub fn terminal_fonts() -> Vec<Cow<'static, [u8]>> {
    vec![
        Cow::Borrowed(include_bytes!(
            "../assets/fonts/JetBrainsMonoNerdFont-Regular.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../assets/fonts/JetBrainsMonoNerdFont-Bold.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../assets/fonts/JetBrainsMonoNerdFont-Italic.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../assets/fonts/JetBrainsMonoNerdFont-BoldItalic.ttf"
        )),
    ]
}

/// An icon the component layer can draw. Each names one SVG under `assets/icons/`.
///
/// A closed enum rather than free-form paths: a typo in an icon name is then a compile error instead of a blank button,
/// and the set of icons the app ships is visible in one place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// A cross: close a dialog or dismiss a notice (the old app's Close glyph).
    Close,
    /// Three dots: open a menu of more actions.
    More,
    /// A bin: delete (the old app's Delete glyph).
    Trash,
    /// A pencil: rename (the old app's Edit glyph, on its Rename command).
    Edit,
    /// A filled circle: the running status.
    StatusRunning,
    /// A triangle: the waiting status, the one pointed shape, because it asks for a person.
    StatusWaiting,
    /// A filled diamond: idle with output nobody has looked at.
    StatusUnseen,
    /// A ring: idle, with nothing new in it.
    StatusIdle,
    /// A ring with one solid quarter: work under way that the user is waiting for (a Delete). Drawn turned a step at a
    /// time, never still-shaped like a status mark.
    Spinner,
}

impl Icon {
    /// Every icon, for the asset listing and the test that each one is bundled.
    ///
    /// A slice, one entry per line, so that a new icon only adds a line: the extension point in
    /// `docs/gpui/foundation-ownership.md` is append-only, and a length in the type would change with every addition.
    pub const ALL: &[Icon] = &[
        Icon::Close,
        Icon::More,
        Icon::Trash,
        Icon::Edit,
        Icon::StatusRunning,
        Icon::StatusWaiting,
        Icon::StatusUnseen,
        Icon::StatusIdle,
        Icon::Spinner,
    ];
}

impl Icon {
    /// The asset path GPUI's `svg()` element asks [`Assets`] for.
    pub fn path(self) -> &'static str {
        match self {
            Icon::Close => "icons/close.svg",
            Icon::More => "icons/more.svg",
            Icon::Trash => "icons/trash.svg",
            Icon::Edit => "icons/edit.svg",
            Icon::StatusRunning => "icons/status-running.svg",
            Icon::StatusWaiting => "icons/status-waiting.svg",
            Icon::StatusUnseen => "icons/status-unseen.svg",
            Icon::StatusIdle => "icons/status-idle.svg",
            Icon::Spinner => "icons/spinner.svg",
        }
    }
}

/// The asset source the app and its headless tests install, serving the bundled icons.
pub struct Assets;

impl Assets {
    /// The SVG bytes for one asset path, or `None` for a path the app does not bundle.
    fn bytes(path: &str) -> Option<&'static [u8]> {
        match path {
            "icons/close.svg" => Some(include_bytes!("../assets/icons/close.svg")),
            "icons/more.svg" => Some(include_bytes!("../assets/icons/more.svg")),
            "icons/trash.svg" => Some(include_bytes!("../assets/icons/trash.svg")),
            "icons/edit.svg" => Some(include_bytes!("../assets/icons/edit.svg")),
            "icons/status-running.svg" => {
                Some(include_bytes!("../assets/icons/status-running.svg"))
            }
            "icons/status-waiting.svg" => {
                Some(include_bytes!("../assets/icons/status-waiting.svg"))
            }
            "icons/status-unseen.svg" => Some(include_bytes!("../assets/icons/status-unseen.svg")),
            "icons/status-idle.svg" => Some(include_bytes!("../assets/icons/status-idle.svg")),
            "icons/spinner.svg" => Some(include_bytes!("../assets/icons/spinner.svg")),
            _ => None,
        }
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::bytes(path).map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Icon::ALL
            .iter()
            .copied()
            .map(Icon::path)
            .filter(|asset| asset.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every icon the enum names is bundled, so no button can draw a missing glyph.
    #[test]
    fn every_icon_resolves_to_bundled_svg_bytes() {
        for icon in Icon::ALL {
            let bytes = Assets.load(icon.path()).unwrap().unwrap();
            assert!(bytes.starts_with(b"<svg"), "{icon:?} is not an SVG");
        }
        assert!(Assets.load("icons/missing.svg").unwrap().is_none());
    }
}

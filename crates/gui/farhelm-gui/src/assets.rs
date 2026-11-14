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
//! The Close, Delete and ChevronDown icons are the old app's own action glyphs, copied from its icon module at the
//! reference commit (14-unit box, 1.2 stroke, round caps and joins). The More icon is a new drawing: the old app wrote
//! the "⋯" character in text. The four status shapes are the old stylesheet's `.status-dot` shapes (an 8-pixel box: a
//! filled circle, a triangle cut with `clip-path`, a diamond, and a ring with a 1.5-pixel border), drawn as SVG because
//! GPUI has no clip path; the view tints them with the status color, as the old dot took `currentColor`. The Spinner is
//! the old stylesheet's `.delete-spinner`, a 12-pixel ring whose top quarter is solid and the rest 30% of the same
//! color (its border in `color-mix(in srgb, var(--accent) 30%, transparent)` with `border-top-color: var(--accent)`);
//! the 30% is the SVG's stroke opacity, so the view still tints it with one color.
//!
//! The sixteen host marks (`Host*`, the shapes a user picks for a host: cloud, house, flask and so on) are the old
//! app's `HostMark` drawings, copied from its icon module at the reference commit into one file each: Farhelm's own
//! set, in a 16-unit box with a 1.3 stroke and round caps and joins, unfilled, in the current color, so the view tints
//! each one with the host's chosen color.
//!
//! The Settings gear and the Sessions and Hosts section glyphs are the old app's own (`SettingsIcon`, and
//! `ActionGlyph::Sessions` and `ActionGlyph::Hosts` in its icon module at the reference commit): the gear in its 16-unit
//! box, the two section glyphs in the 14-unit action box, all with the 1.2 stroke and round caps and joins.

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
    /// A ring with one solid quarter: work under way that the user is waiting for (a Delete, a Stop, a Replace that has
    /// waited a while). Drawn turned a step at a time, never still-shaped like a status mark.
    Spinner,
    /// A tick: the mark in a checked checkbox (drawn for this app, in the shape of the platform checkbox's tick).
    Check,
    /// A rounded square: stop the agent (the old app's Stop glyph, on its Stop command).
    Stop,
    /// Two opposed arrows: start the session over under the same settings (the old app's Replace glyph). Deliberately
    /// not a circular arrow, which reads as resuming, so Replace and a later Restart never look alike beside each other.
    Replace,
    /// A small rocket: start a session (the old app's Launch glyph, on the session launcher's header tile).
    Launch,
    /// The first of the host marks, one per `HostIcon` a user can choose for a host
    /// (`farhelm_proto::host_appearance`), in that enum's order. The cloud is a remote host's default.
    HostCloud,
    HostHouse,
    HostFlask,
    HostDatabase,
    HostChip,
    HostRocket,
    HostGear,
    HostGem,
    HostHexagon,
    HostTriangle,
    HostRing,
    HostSquare,
    HostBug,
    HostFactory,
    HostCastle,
    /// The laptop, the local host's default (drawn in red).
    HostLaptop,
    /// A downward chevron: the closed select's sign that it opens a list (the old app's ChevronDown glyph, which its
    /// split buttons and sort menu drew; the native select it stands in for drew the platform's own chevron).
    ChevronDown,
    /// An eye: mark a session's latest output as seen, or as unseen again (the old app's mark-seen glyph, on its Mark
    /// read and Mark unread command).
    MarkSeen,
    /// A gear: Settings (the old app's Settings glyph, on its rail button and in the Settings dialog's header tile).
    Settings,
    /// Two stacked windows: the Settings dialog's Sessions section (the old app's Sessions glyph).
    Sessions,
    /// Three boxes joined as a tree: the Settings dialog's Hosts section (the old app's Hosts glyph).
    Hosts,
}

impl Icon {
    /// Every icon, for the asset listing and the test that each one is bundled.
    ///
    /// A slice, one entry per line, so that a new icon only adds a line and leaves every existing one alone: a length
    /// in the type would change with every addition (`docs/gpui/foundation-ownership.md` says why that matters).
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
        Icon::Check,
        Icon::Stop,
        Icon::Replace,
        Icon::Launch,
        Icon::HostCloud,
        Icon::HostHouse,
        Icon::HostFlask,
        Icon::HostDatabase,
        Icon::HostChip,
        Icon::HostRocket,
        Icon::HostGear,
        Icon::HostGem,
        Icon::HostHexagon,
        Icon::HostTriangle,
        Icon::HostRing,
        Icon::HostSquare,
        Icon::HostBug,
        Icon::HostFactory,
        Icon::HostCastle,
        Icon::HostLaptop,
        Icon::ChevronDown,
        Icon::MarkSeen,
        Icon::Settings,
        Icon::Sessions,
        Icon::Hosts,
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
            Icon::Check => "icons/check.svg",
            Icon::Stop => "icons/stop.svg",
            Icon::Replace => "icons/replace.svg",
            Icon::Launch => "icons/launch.svg",
            Icon::HostCloud => "icons/host-cloud.svg",
            Icon::HostHouse => "icons/host-house.svg",
            Icon::HostFlask => "icons/host-flask.svg",
            Icon::HostDatabase => "icons/host-database.svg",
            Icon::HostChip => "icons/host-chip.svg",
            Icon::HostRocket => "icons/host-rocket.svg",
            Icon::HostGear => "icons/host-gear.svg",
            Icon::HostGem => "icons/host-gem.svg",
            Icon::HostHexagon => "icons/host-hexagon.svg",
            Icon::HostTriangle => "icons/host-triangle.svg",
            Icon::HostRing => "icons/host-ring.svg",
            Icon::HostSquare => "icons/host-square.svg",
            Icon::HostBug => "icons/host-bug.svg",
            Icon::HostFactory => "icons/host-factory.svg",
            Icon::HostCastle => "icons/host-castle.svg",
            Icon::HostLaptop => "icons/host-laptop.svg",
            Icon::ChevronDown => "icons/chevron-down.svg",
            Icon::MarkSeen => "icons/mark-seen.svg",
            Icon::Settings => "icons/settings.svg",
            Icon::Sessions => "icons/sessions.svg",
            Icon::Hosts => "icons/hosts.svg",
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
            "icons/check.svg" => Some(include_bytes!("../assets/icons/check.svg")),
            "icons/stop.svg" => Some(include_bytes!("../assets/icons/stop.svg")),
            "icons/replace.svg" => Some(include_bytes!("../assets/icons/replace.svg")),
            "icons/launch.svg" => Some(include_bytes!("../assets/icons/launch.svg")),
            "icons/host-cloud.svg" => Some(include_bytes!("../assets/icons/host-cloud.svg")),
            "icons/host-house.svg" => Some(include_bytes!("../assets/icons/host-house.svg")),
            "icons/host-flask.svg" => Some(include_bytes!("../assets/icons/host-flask.svg")),
            "icons/host-database.svg" => Some(include_bytes!("../assets/icons/host-database.svg")),
            "icons/host-chip.svg" => Some(include_bytes!("../assets/icons/host-chip.svg")),
            "icons/host-rocket.svg" => Some(include_bytes!("../assets/icons/host-rocket.svg")),
            "icons/host-gear.svg" => Some(include_bytes!("../assets/icons/host-gear.svg")),
            "icons/host-gem.svg" => Some(include_bytes!("../assets/icons/host-gem.svg")),
            "icons/host-hexagon.svg" => Some(include_bytes!("../assets/icons/host-hexagon.svg")),
            "icons/host-triangle.svg" => Some(include_bytes!("../assets/icons/host-triangle.svg")),
            "icons/host-ring.svg" => Some(include_bytes!("../assets/icons/host-ring.svg")),
            "icons/host-square.svg" => Some(include_bytes!("../assets/icons/host-square.svg")),
            "icons/host-bug.svg" => Some(include_bytes!("../assets/icons/host-bug.svg")),
            "icons/host-factory.svg" => Some(include_bytes!("../assets/icons/host-factory.svg")),
            "icons/host-castle.svg" => Some(include_bytes!("../assets/icons/host-castle.svg")),
            "icons/host-laptop.svg" => Some(include_bytes!("../assets/icons/host-laptop.svg")),
            "icons/chevron-down.svg" => Some(include_bytes!("../assets/icons/chevron-down.svg")),
            "icons/mark-seen.svg" => Some(include_bytes!("../assets/icons/mark-seen.svg")),
            "icons/settings.svg" => Some(include_bytes!("../assets/icons/settings.svg")),
            "icons/sessions.svg" => Some(include_bytes!("../assets/icons/sessions.svg")),
            "icons/hosts.svg" => Some(include_bytes!("../assets/icons/hosts.svg")),
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

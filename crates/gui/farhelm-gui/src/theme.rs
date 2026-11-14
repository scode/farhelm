//! Theme tokens and the user's appearance choice: the only place in the GUI that turns numbers into colors.
//!
//! The port copies the old app's look rather than redesigning it (SPEC_impl.md, "GUI architecture"), so every token
//! here is one of the old stylesheet's custom properties (`crates/farhelm-ui/assets/app.css` at the reference commit
//! `24bc0580`), named by role, with the CSS variable it came from in its doc. Views never hold a color: they ask
//! [`palette`] for the tokens while they draw, and turn them into GPUI colors at the last moment through [`color`],
//! which sits under the crate's one exemption from the raw-color bans in `crates/gui/clippy.toml`. A [`Token`] can only
//! be made inside this module, so `color` cannot be handed a literal from anywhere else: the palette tables below are
//! the only place a color value is written down.
//!
//! Appearance follows SPEC.md: the user chooses System, Dark or Light, and System follows the operating system live.
//! A change goes through [`set_preference`] or [`follow_system`], which re-project GPUI Kit's theme from the same
//! tokens (Kit's text inputs read Kit's theme) and refresh every window. A refresh redraws cached views too, so no view
//! needs a theme setter. Persisting the choice is the local preference store's job, which is not built yet; until then
//! the choice lives for one run.

use std::rc::Rc;

use gpui::{App, Global, Pixels, Subscription, Window, WindowAppearance, px};
use gpui_component::{Theme, ThemeConfig, ThemeMode};

/// A color from the theme's tables. The constructors are private, so every color the GUI draws with is one of the
/// tokens defined in this module; [`color`] turns one into a GPUI color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    /// `0xRRGGBB`, or `0xRRGGBBAA` when `alpha` is set.
    value: u32,
    alpha: bool,
}

/// An opaque token, `0xRRGGBB`.
const fn rgb_token(value: u32) -> Token {
    Token {
        value,
        alpha: false,
    }
}

/// A token that carries alpha, `0xRRGGBBAA`.
const fn rgba_token(value: u32) -> Token {
    Token { value, alpha: true }
}

/// Fully transparent: the edge of a button tier that has none, kept so every tier is the same size.
pub const TRANSPARENT: Token = rgba_token(0);

/// The color roles the GUI paints with.
///
/// Roles, not colors: a floating surface asks for [`Palette::bg_2`], never "the lighter grey". Each doc names the CSS
/// custom property the value was copied from at the reference commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Whether this is the dark appearance; selects Kit's mode.
    pub dark: bool,
    /// `--bg-0`: the ground behind everything.
    pub bg_0: Token,
    /// `--well`: recessed fields such as text inputs.
    pub well: Token,
    /// `--bg-1`: raised chrome: the sidebar column, dialog footers.
    pub bg_1: Token,
    /// `--bg-2`: floating surfaces: menus, dialogs, tooltips.
    pub bg_2: Token,
    /// `--control-hover-bg`: the hover fill of a control; also a dialog header's band.
    pub control_hover: Token,
    /// `--fg-0`: primary text.
    pub fg_0: Token,
    /// `--fg-dim`: explanatory prose on a notice surface.
    pub fg_dim: Token,
    /// `--fg-1`: secondary text: labels, metadata, quiet status words.
    pub fg_1: Token,
    /// `--fg-2`: quiet prose, and the text of an unavailable filled action.
    pub fg_2: Token,
    /// `--on-fill`: text and shapes on a solid selection, primary or danger fill.
    pub on_fill: Token,
    /// `--accent`: focus rings on neutral surfaces.
    pub accent: Token,
    /// `--accent-fill`: solid selection and ordinary primary actions.
    pub accent_fill: Token,
    /// `--accent-fill-hover`: hover inside selection and on a primary action.
    pub accent_fill_hover: Token,
    /// `--accent-edge`: the edge of a filled primary action.
    pub accent_edge: Token,
    /// `--border`: the standard control outline.
    pub border: Token,
    /// `--border-dim`: quiet rules, such as between a dialog's header, body and footer.
    pub border_dim: Token,
    /// `--border-sidebar-edge`: the sidebar's edge against the main pane.
    pub border_sidebar_edge: Token,
    /// `--border-row`: the hairline between stacked session rows.
    pub border_row: Token,
    /// `--border-dialog`: a modal panel's own edge.
    pub border_dialog: Token,
    /// `--ok`: running, connected.
    pub ok: Token,
    /// `--warn`: the amber label tone.
    pub warn: Token,
    /// `--danger`: the red text tone of destructive actions and waiting.
    pub danger: Token,
    /// `--danger-strong`: failure prose on ordinary surfaces.
    pub danger_strong: Token,
    /// `--danger-soft`: the outline of a red fill.
    pub danger_soft: Token,
    /// `--danger-bg`: the solid red fill of a destructive confirmation.
    pub danger_bg: Token,
    /// `--danger-fill-hover`: hover on that fill.
    pub danger_fill_hover: Token,
    /// `--info`: idle with output nobody has looked at.
    pub info: Token,
    /// `--on-accent-dim`: secondary text on a selection fill.
    pub on_accent_dim: Token,
    /// `--scrim-dialog`, as `0xRRGGBBAA`: the veil behind a modal dialog.
    pub scrim_dialog: Token,
    /// `--shadow-menu`'s color, as `0xRRGGBBAA`: under menus and tooltips.
    pub shadow_menu: Token,
    /// `--shadow-dialog`'s color, as `0xRRGGBBAA`: under modal dialogs.
    pub shadow_dialog: Token,
    /// `--host-icon-*`, in the host color order lavender, orchid, teal, steel, sand, copper, red.
    pub host: [Token; 7],
}

/// The old app's "Steel" dark appearance.
pub const DARK: Palette = Palette {
    dark: true,
    bg_0: rgb_token(0x181c24),
    well: rgb_token(0x1a2029),
    bg_1: rgb_token(0x1d222c),
    bg_2: rgb_token(0x282e3a),
    control_hover: rgb_token(0x333b49),
    fg_0: rgb_token(0xe2e6f0),
    fg_dim: rgb_token(0xbbc3d2),
    fg_1: rgb_token(0xa2aabb),
    fg_2: rgb_token(0x9ba5b8),
    on_fill: rgb_token(0xffffff),
    accent: rgb_token(0x8ea2f5),
    accent_fill: rgb_token(0x5268d0),
    accent_fill_hover: rgb_token(0x495fc7),
    accent_edge: rgb_token(0x5268d0),
    border: rgb_token(0x485367),
    border_dim: rgb_token(0x303846),
    border_sidebar_edge: rgb_token(0x3c4657),
    border_row: rgb_token(0x303846),
    border_dialog: rgb_token(0x59667c),
    ok: rgb_token(0x7fcf9a),
    warn: rgb_token(0xe5bb6e),
    danger: rgb_token(0xee8a8a),
    danger_strong: rgb_token(0xf09595),
    danger_soft: rgb_token(0xf2c0c4),
    danger_bg: rgb_token(0xa83e4d),
    danger_fill_hover: rgb_token(0x983644),
    info: rgb_token(0x8ea2f5),
    on_accent_dim: rgb_token(0xf4f6ff),
    // rgb(8 11 17 / 62%)
    scrim_dialog: rgba_token(0x080b119e),
    // rgba(0, 0, 0, 0.35)
    shadow_menu: rgba_token(0x00000059),
    // rgb(0 0 0 / 40%)
    shadow_dialog: rgba_token(0x00000066),
    host: [
        rgb_token(0xb8a4e8),
        rgb_token(0xdfa0d2),
        rgb_token(0x76c6bf),
        rgb_token(0xa1b8cd),
        rgb_token(0xcdbb99),
        rgb_token(0xd7a180),
        rgb_token(0xe69c9c),
    ],
};

/// The old app's "Steel" light appearance. Layout and type are shared with dark; only roles change.
pub const LIGHT: Palette = Palette {
    dark: false,
    bg_0: rgb_token(0xf4f6fa),
    well: rgb_token(0xf1f3f8),
    bg_1: rgb_token(0xe9edf4),
    bg_2: rgb_token(0xffffff),
    control_hover: rgb_token(0xdde3ee),
    fg_0: rgb_token(0x1b2130),
    fg_dim: rgb_token(0x414b5f),
    fg_1: rgb_token(0x4f5a70),
    fg_2: rgb_token(0x5a6579),
    on_fill: rgb_token(0xffffff),
    accent: rgb_token(0x445acf),
    accent_fill: rgb_token(0x4a63d8),
    accent_fill_hover: rgb_token(0x4059c7),
    accent_edge: rgb_token(0x4a63d8),
    border: rgb_token(0xb8c2d3),
    border_dim: rgb_token(0xd8deea),
    border_sidebar_edge: rgb_token(0xcbd3e0),
    border_row: rgb_token(0xd8deea),
    border_dialog: rgb_token(0xb8c2d3),
    ok: rgb_token(0x276a43),
    warn: rgb_token(0x79531b),
    danger: rgb_token(0xa63e4d),
    danger_strong: rgb_token(0xa13443),
    danger_soft: rgb_token(0xf2c0c4),
    danger_bg: rgb_token(0xa83e4d),
    danger_fill_hover: rgb_token(0x983644),
    info: rgb_token(0x445acf),
    on_accent_dim: rgb_token(0xf4f6ff),
    // rgb(27 33 48 / 30%)
    scrim_dialog: rgba_token(0x1b21304d),
    // rgba(27, 33, 48, 0.15)
    shadow_menu: rgba_token(0x1b213026),
    // rgb(27 33 48 / 20%)
    shadow_dialog: rgba_token(0x1b213033),
    host: [
        rgb_token(0x68529c),
        rgb_token(0x8c447d),
        rgb_token(0x216b67),
        rgb_token(0x4e657d),
        rgb_token(0x75603b),
        rgb_token(0x8b5233),
        rgb_token(0xa14646),
    ],
};

/// Sizes the slice's surfaces use, copied from the old stylesheet. Shared by both appearances.
pub mod metrics {
    use super::{Pixels, px};

    /// `--radius-sm`: controls and inputs.
    pub const RADIUS_SM: Pixels = px(3.);
    /// `--radius-md`: panels, cards, dialogs, tooltips.
    pub const RADIUS_MD: Pixels = px(4.);
    /// The common control text size (`.btn`, `.menu-item`, tooltips, dialog summaries).
    pub const TEXT_SM: Pixels = px(12.);
    /// Row titles and settings choices.
    pub const TEXT_MD: Pixels = px(13.);
    /// `.dialog-title`.
    pub const TEXT_TITLE: Pixels = px(18.);
    /// `.btn` padding, vertical then horizontal.
    pub const BUTTON_PAD: (Pixels, Pixels) = (px(6.), px(10.));
    /// `.banner` padding, vertical then horizontal: the red band across the top of a surface.
    pub const BANNER_PAD: (Pixels, Pixels) = (px(4.), px(10.));
    /// `.banner-reclaim` padding, vertical then horizontal: a button inside that band.
    pub const BANNER_BUTTON_PAD: (Pixels, Pixels) = (px(1.), px(8.));
    /// `.banner-reclaim`'s `margin-left`: the gap between the banner's text and its button.
    pub const BANNER_BUTTON_GAP: Pixels = px(10.);
    /// `.dialog-header`/`.dialog-body` padding, vertical then horizontal.
    pub const DIALOG_PAD: (Pixels, Pixels) = (px(20.), px(24.));
    /// `.dialog-footer` padding, vertical then horizontal.
    pub const DIALOG_FOOTER_PAD: (Pixels, Pixels) = (px(14.), px(24.));
    /// The dialog panel's width (`width: min(480px, 100%)`).
    pub const DIALOG_WIDTH: Pixels = px(480.);
    /// `--dialog-vertical-margin`'s floor: space kept above and below a dialog.
    pub const DIALOG_VERTICAL_MARGIN: Pixels = px(24.);
    /// `.farhelm-tooltip`'s max width (`min(280px, 100vw - 8px)`; the layer manager applies the viewport half).
    pub const TOOLTIP_MAX_WIDTH: Pixels = px(280.);
    /// `.farhelm-tooltip`'s padding, vertical then horizontal.
    pub const TOOLTIP_PAD: (Pixels, Pixels) = (px(4.), px(8.));
    /// `.farhelm-tooltip`'s line height: `line-height: 1.35` at the 12-pixel control size.
    pub const TOOLTIP_LINE_HEIGHT: Pixels = px(16.2);
    /// `.farhelm-tooltip`'s `-webkit-line-clamp`: longer help is cut at this many lines.
    pub const TOOLTIP_MAX_LINES: usize = 8;
}

/// The user's appearance choice. System follows the operating system live; the explicit choices ignore it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preference {
    #[default]
    System,
    Dark,
    Light,
}

/// Explicit choices win; System maps GPUI's four appearances onto the two palettes.
pub fn resolve(preference: Preference, system: WindowAppearance) -> &'static Palette {
    match preference {
        Preference::Dark => &DARK,
        Preference::Light => &LIGHT,
        Preference::System => match system {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => &DARK,
            WindowAppearance::Light | WindowAppearance::VibrantLight => &LIGHT,
        },
    }
}

/// App-global theme state. Only this module writes it; views read it through [`palette`].
struct AppTheme {
    preference: Preference,
    /// The last operating-system appearance reported to [`follow_system`] (or read at install).
    system: WindowAppearance,
    /// The palette Kit and the windows were last told about. `None` forces the first [`apply`] to configure Kit.
    applied: Option<&'static Palette>,
}
impl Global for AppTheme {}

/// Install the theme. The bootstrap calls this after Kit's own init and before any window opens, so the first frame
/// already uses the resolved palette and Kit agrees with it (Kit's init picks Light on its own).
pub fn install(preference: Preference, system: WindowAppearance, cx: &mut App) {
    cx.set_global(AppTheme {
        preference,
        system,
        applied: None,
    });
    apply(cx);
}

/// The palette every view paints with. Before [`install`] (a bare fixture) this is dark, the old app's default look.
pub fn palette(cx: &App) -> &'static Palette {
    cx.try_global::<AppTheme>()
        .and_then(|theme| theme.applied)
        .unwrap_or(&DARK)
}

/// Commit a user choice and repaint. Choosing the current value again changes nothing.
pub fn set_preference(preference: Preference, cx: &mut App) {
    let Some(theme) = cx.try_global::<AppTheme>() else {
        return;
    };
    if theme.preference == preference {
        return;
    }
    cx.global_mut::<AppTheme>().preference = preference;
    apply(cx);
    // The control's selected state changes even when the palette does not (System to the mode it already resolved to).
    cx.refresh_windows();
}

/// The operating-system seam: production reaches it through [`observe_window`] and the bootstrap; tests call it with
/// the appearance a platform would report.
pub fn follow_system(system: WindowAppearance, cx: &mut App) {
    if !cx.has_global::<AppTheme>() {
        return;
    }
    cx.global_mut::<AppTheme>().system = system;
    apply(cx);
}

/// Follow this window's appearance on every operating-system change. The bootstrap reads the window's initial
/// appearance itself, once the window exists (work done inside `open_window`'s builder is a cookbook trap).
///
/// The window's appearance, not the app-level one, because on Linux the app-level value can lag the desktop portal.
/// The returned subscription must live as long as the window; dropping it silently stops the following.
pub fn observe_window(window: &Window) -> Subscription {
    window.observe_window_appearance(|window, cx| follow_system(window.appearance(), cx))
}

/// Make Kit and every window agree with the resolved palette. Silent when nothing changed: Wayland can report the
/// same appearance again, and an explicit choice ignores system changes.
fn apply(cx: &mut App) {
    let theme = cx.global::<AppTheme>();
    let next = resolve(theme.preference, theme.system);
    let first = theme.applied.is_none();
    if theme.applied == Some(next) {
        return;
    }
    cx.global_mut::<AppTheme>().applied = Some(next);
    if cx.has_global::<Theme>() {
        if first {
            let kit = Theme::global_mut(cx);
            kit.dark_theme = Rc::new(kit_config(&DARK));
            kit.light_theme = Rc::new(kit_config(&LIGHT));
        }
        let mode = if next.dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        Theme::change(mode, None, cx);
    }
    cx.refresh_windows();
}

/// Project one palette onto Kit's theme schema, so the Kit text input draws with the same tokens as Farhelm's own
/// chrome. Kit keys this map omits fall back to Kit's defaults, which no Farhelm surface shows. The config also names
/// Inter as Kit's font; Kit's default system face would otherwise differ from the rest of the window.
fn kit_config(palette: &Palette) -> ThemeConfig {
    let hex = |token: Token| format!("#{:06x}", token.value);
    let colors = serde_json::json!({
        "background": hex(palette.well),
        "foreground": hex(palette.fg_0),
        "border": hex(palette.border),
        "input.border": hex(palette.border),
        "ring": hex(palette.accent),
        "caret": hex(palette.fg_0),
        "muted.foreground": hex(palette.fg_1),
        "selection.background": hex(palette.accent_fill),
        "scrollbar.background": format!("#{:06x}00", palette.bg_1.value),
        "scrollbar.thumb.background": hex(palette.border),
        "scrollbar.thumb.hover.background": hex(palette.fg_1),
    });
    let config = serde_json::json!({
        "name": if palette.dark { "Farhelm Steel Dark" } else { "Farhelm Steel Light" },
        "mode": if palette.dark { "dark" } else { "light" },
        "font.family": crate::assets::UI_FONT,
        // The old stylesheet's `--radius-sm` and `--radius-md`, and its flat controls (Kit adds drop shadows otherwise).
        "radius": 3,
        "radius.lg": 4,
        "shadow": false,
        "colors": colors,
    });
    // The JSON above is a constant shape this module controls; a schema mismatch is a programming error that the theme
    // test catches. Falling back to Kit's default config keeps a release build drawing rather than aborting.
    serde_json::from_value(config).unwrap_or_else(|error| {
        tracing::error!("Kit theme projection rejected: {error}");
        ThemeConfig::default()
    })
}

pub use raw::color;

/// The crate's one exemption from the raw-color bans: everything that draws turns a [`Token`] into a GPUI color through
/// [`color`], and only this module can make a token, so a color that is not in the palette tables cannot reach the
/// screen without a second exemption that review would see.
#[allow(
    clippy::disallowed_methods,
    reason = "the theme module is the one place raw colors are built"
)]
mod raw {
    use gpui::Hsla;

    use super::Token;

    /// A theme token as a GPUI color.
    pub fn color(token: Token) -> Hsla {
        if token.alpha {
            gpui::rgba(token.value).into()
        } else {
            gpui::rgb(token.value).into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The projection of both palettes is accepted by Kit's schema; a rejected projection would silently fall back to
    /// Kit's own colors in a release build, so this is the test that catches it.
    #[test]
    fn both_palettes_project_onto_kits_schema() {
        for palette in [&DARK, &LIGHT] {
            let config = serde_json::to_value(kit_config(palette)).unwrap();
            assert_eq!(config["font.family"], crate::assets::UI_FONT);
            assert_eq!(config["mode"], if palette.dark { "dark" } else { "light" });
        }
    }

    /// SPEC.md's appearance rule: an explicit choice ignores the system, System follows it.
    #[test]
    fn explicit_choices_win_and_system_follows_the_os() {
        assert_eq!(resolve(Preference::Dark, WindowAppearance::Light), &DARK);
        assert_eq!(resolve(Preference::Light, WindowAppearance::Dark), &LIGHT);
        assert_eq!(
            resolve(Preference::System, WindowAppearance::VibrantLight),
            &LIGHT
        );
        assert_eq!(
            resolve(Preference::System, WindowAppearance::VibrantDark),
            &DARK
        );
    }

    /// A system change repaints only while the choice is System, and an explicit choice switches the palette at once.
    #[gpui::test]
    fn the_palette_follows_choice_and_system(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            install(Preference::System, WindowAppearance::Dark, cx);
            assert!(palette(cx).dark);
            follow_system(WindowAppearance::Light, cx);
            assert!(!palette(cx).dark);
            set_preference(Preference::Dark, cx);
            assert!(palette(cx).dark);
            follow_system(WindowAppearance::Light, cx);
            assert!(palette(cx).dark, "an explicit choice ignores the system");
            assert_eq!(
                Theme::global(cx).mode,
                ThemeMode::Dark,
                "Kit agrees with the palette"
            );
        });
    }
}

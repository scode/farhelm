//! Theme tokens and the user's appearance choice: the only place in the GUI that turns numbers into colors.
//!
//! The port copies the old app's look rather than redesigning it (SPEC_impl.md, "GUI architecture"), so every token
//! here is one of the old stylesheet's custom properties (`crates/farhelm-ui/assets/app.css` at the reference commit
//! `24bc0580`), named by role, with the CSS variable it came from in its doc. Views never hold a color: they ask
//! [`palette`] for the tokens while they draw, and turn them into GPUI colors at the last moment through [`color`],
//! which sits under the crate's one exemption from the raw-color bans in `crates/gui/clippy.toml`. A [`Token`] can only
//! be made inside this module, so `color` cannot be handed a literal from anywhere else: the palette tables below are
//! the only place the GUI's own colors are written down.
//!
//! The terminal is the one place colors come from outside the tables: a program can name any truecolor, and the
//! 256-color index's cube and gray ramp are computed rather than listed. The migration's orchestrator decided on
//! 2026-11-13 that the terminal's palette lives inside the theme under its exemption, with no second one. Those wire
//! values become GPUI colors here too, through [`TerminalColor::hsla`] under the same exemption, so no view builds a
//! color either way.
//!
//! The terminal's palette values stay here, in [`TERMINAL`], but the arithmetic that resolves a cell's style against
//! them (the 256-color index, bold-as-bright, inverse, dim and the program's OSC 10/11 defaults) lives in the grid
//! model's `colors` module, which has no GPUI and no palette of its own. [`TERMINAL_GRID`] hands it these values:
//! the terminal view creates its grid model with it, and [`terminal_color`] and the terminal's style resolution call
//! the same arithmetic, so the colors the model reasons about are the colors the screen paints.
//!
//! Appearance follows SPEC.md: the user chooses System, Dark or Light, and System follows the operating system live.
//! A change goes through [`set_preference`] or [`follow_system`], which re-project GPUI Kit's theme from the same
//! tokens (Kit's text inputs read Kit's theme) and refresh every window. A refresh redraws cached views too, so no view
//! needs a theme setter. The choice is a helm preference, not a device's (the maintainer decided on 2026-11-13 that
//! appearance is a helm preference like every other setting): every launch starts at System, and the Settings area
//! hands the stored choice to [`set_preference`], through the main view's subscription, when the first snapshot
//! arrives and on every change.

use std::rc::Rc;

use gpui::{App, Global, Hsla, Pixels, Subscription, Window, WindowAppearance, px};
use gpui_component::{Theme, ThemeConfig, ThemeMode};

use crate::bootstrap::{installed, installed_mut};

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

/// The recording cues keep the sample's colors in either app appearance.
#[cfg(feature = "agent")]
pub(crate) mod video {
    use super::{Token, rgb_token};

    pub(crate) const ACCENT: Token = rgb_token(0xff5fa2);
    pub(crate) const INK: Token = rgb_token(0xffffff);
    pub(crate) const BOX: Token = rgb_token(0x111119);
    pub(crate) const SHADE: Token = rgb_token(0x000000);
    pub(crate) const CARD: Token = rgb_token(0x080a0e);
    pub(crate) const SUBTITLE: Token = rgb_token(0xc9d1dc);
}

/// The color roles the GUI paints with.
///
/// Roles, not colors: a floating surface asks for [`Palette::bg_2`], never "the lighter grey". Each doc names the CSS
/// custom property where the color comes from the old stylesheet.
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
    /// Settings content sits on a separate ground from the navigation and its cards.
    pub settings_page: Token,
    /// Individually grouped content in Settings and the session launcher.
    pub card: Token,
    /// `--control-hover-bg`: the hover fill of a control; also a dialog header's band.
    pub control_hover: Token,
    /// Translucent fill for neutral buttons and chips. It follows the surface underneath, including dialog bodies.
    pub button_rest: Token,
    /// Hover tint for neutral buttons, chips and unchosen segments.
    pub button_hover: Token,
    /// Tint while a neutral button or chip is held down with the pointer.
    pub button_held: Token,
    /// Quieter tint for filled actions that are unavailable.
    pub button_retired: Token,
    /// Hover fill for ordinary primary buttons. Open menus and other selections use `accent_fill_hover`.
    pub primary_button_hover: Token,
    /// Hover fill for ordinary destructive buttons. Banner actions use `danger_fill_hover`.
    pub danger_button_hover: Token,
    /// `--fg-0`: primary text.
    pub fg_0: Token,
    /// `--fg-bright`: emphasis on an ordinary neutral surface, such as a link's host in its destination display.
    pub fg_bright: Token,
    /// `--tunnel-fg`: the open-tunnels pill's text, the one distinctively colored control in the session's top row.
    pub tunnel_fg: Token,
    /// `--tunnel-bg`: the open-tunnels pill's fill.
    pub tunnel_bg: Token,
    /// `--tunnel-border`: the open-tunnels pill's edge.
    pub tunnel_border: Token,
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
    /// `--accent-fill-hover`: hover inside a selection, open menus, and primary buttons while held.
    pub accent_fill_hover: Token,
    /// `--accent-edge`: the edge of a filled primary action.
    pub accent_edge: Token,
    /// `--accent-fill` over `--bg-1`: the fill of a marked session row that is not the selected one. The old stylesheet
    /// had no variable for it; its `.session-row[data-session-marked="true"]` rule mixed one in place, `color-mix(in
    /// srgb, var(--accent-fill) 14%, var(--bg-1))`. Both are opaque, so the mix is premixed here from this palette's
    /// own `accent_fill` and `bg_1`, 14 and 86 parts per channel, rounded.
    pub marked_bg: Token,
    /// `--border`: the standard control outline.
    pub border: Token,
    /// `--border-dim`: quiet rules, such as between a dialog's header, body and footer.
    pub border_dim: Token,
    /// `--border-nested`: the rule down the left of an indented section that belongs to the row above it, such as a
    /// host's setup steps under its row.
    pub border_nested: Token,
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
    /// `--warn-bright`: amber prose, which needs more lift than a label does; a question's consequence lines.
    pub warn_bright: Token,
    /// `--warn-bg`: the filled amber band behind the truncated-list warning.
    pub warn_bg: Token,
    /// `--warn-bg-dim`: the quieter amber band, behind the launcher's refused-launch alert.
    pub warn_bg_dim: Token,
    /// `--warn-border`: the edge of an amber band, around the launcher's refused-launch alert.
    pub warn_border: Token,
    /// `color-mix(in srgb, var(--warn) 14%, transparent)`: the agent approval card's "Agent request" badge fill
    /// (`.approval-card-badge`), `--warn` at 14% over whatever it sits on.
    pub warn_badge_bg: Token,
    /// `color-mix(in srgb, var(--warn) 12%, transparent)`: a warning chip's fill on an approval card
    /// (`.approval-permission-warning`), `--warn` at 12%.
    pub warn_chip_bg: Token,
    /// `--danger`: the red text tone of destructive actions and waiting.
    pub danger: Token,
    /// `--danger-strong`: failure prose on ordinary surfaces.
    pub danger_strong: Token,
    /// `--danger-soft`: the outline of a red fill.
    pub danger_soft: Token,
    /// `--danger-bg`: the solid red fill of a destructive confirmation.
    pub danger_bg: Token,
    /// `--danger-fill-hover`: banner button hover and destructive buttons while held.
    pub danger_fill_hover: Token,
    /// `--danger-bg-dim`: a subdued red fill, the hover of an outlined danger button (a permanent answer).
    pub danger_bg_dim: Token,
    /// `color-mix(in srgb, var(--danger) 14%, transparent)`, as `0xRRGGBBAA`: the red wash behind a loud question,
    /// the old YOLO confirmation's `.yolo-confirmation` background.
    pub danger_wash: Token,
    /// The pending-update pill's pointer tint, between its resting and reminder fills.
    pub danger_wash_hover: Token,
    /// The reminder's strongest fill holds under the pointer. Light mode uses a solid fill to distinguish the pulse
    /// from the hover wash.
    pub danger_wash_pulse: Token,
    /// The pending-update pill's text on its hover wash.
    pub update_ready_hover_ink: Token,
    /// The pending-update pill's text on the reminder fill.
    pub update_ready_pulse_ink: Token,
    /// The pending-update pill's warning mark on its hover wash: darker amber in light mode, where the resting amber
    /// would fall below 4.5:1 on the deeper wash.
    pub update_ready_hover_warn: Token,
    /// The pending-update pill's warning mark on the reminder fill: white in light mode, where amber cannot be read
    /// on solid red.
    pub update_ready_pulse_warn: Token,
    /// `color-mix(in srgb, var(--danger) 20%, var(--bg-2))`, worked out: the launcher's permission-escalation banner,
    /// the old `.permission-escalation-banner` background.
    pub danger_tint: Token,
    /// `--info`: idle with output nobody has looked at.
    pub info: Token,
    /// `--on-accent-dim`: secondary text on a selection fill.
    pub on_accent_dim: Token,
    /// `--link`: a link and a button drawn as one (`.inline-link`), underlined as well, because this blue alone does
    /// not stand out enough from the grey prose around it.
    pub link: Token,
    /// `--status-pulse-min`, in percent: the running pulse's dimmest opacity on an ordinary row. Shallow, never off, so
    /// it reads as a heartbeat rather than a blink; the light appearance's is higher because its dark green fades into
    /// a light hover sooner.
    pub status_pulse_min: u8,
    /// `--selected-status-pulse-min`, in percent: the same on the solid selection, where a deeper dip loses the
    /// mark's contrast against the fill.
    pub selected_status_pulse_min: u8,
    /// `--scrim`, as `0xRRGGBBAA`: the veil over a terminal that a Delete is taking away. Black in both appearances,
    /// because it lies over the terminal's own dark ground and must be darker than it; a lighter veil would lift the
    /// terminal rather than dim it.
    pub scrim: Token,
    /// `--scrim-dialog`, as `0xRRGGBBAA`: the veil behind a modal dialog.
    pub scrim_dialog: Token,
    /// `--shadow-menu`'s color, as `0xRRGGBBAA`: under menus and tooltips.
    pub shadow_menu: Token,
    /// `--shadow-dialog`'s color, as `0xRRGGBBAA`: under modal dialogs.
    pub shadow_dialog: Token,
    /// `--host-icon-*`, in the host color order lavender, orchid, teal, steel, sand, copper, red.
    pub host: [Token; 7],
    /// `--harness-*`, the agent marks' identity colors, ordered claude, muse, cursor, pi, omp, grok, opencode,
    /// antigravity and custom (`docs/harness-marks.md`, "Colors"). Codex, Goose and the Terminal mark have none: they
    /// keep the agent track's `--fg-1`, as their owners' terms or Farhelm's own choice ask.
    pub harness: [Token; 9],
}

impl Palette {
    /// Blue marks the general Settings sections. Controls keep their usual accent in all groups.
    pub fn settings_general(&self) -> Token {
        self.accent
    }

    /// The Settings group for places sessions run shares teal with the host palette.
    pub fn settings_placement(&self) -> Token {
        self.host[2]
    }

    /// The Settings group for agent configuration shares orchid with the host palette.
    pub fn settings_agents(&self) -> Token {
        self.host[1]
    }
}

/// The old app's "Steel" dark appearance.
pub const DARK: Palette = Palette {
    dark: true,
    bg_0: rgb_token(0x181c24),
    well: rgb_token(0x1a2029),
    bg_1: rgb_token(0x1d222c),
    bg_2: rgb_token(0x282e3a),
    settings_page: rgb_token(0x282e3a),
    card: rgb_token(0x20252f),
    control_hover: rgb_token(0x333b49),
    // White at 6.5%, 11%, 8.5%, and 4%, rounded to the nearest alpha byte.
    button_rest: rgba_token(0xffffff11),
    button_hover: rgba_token(0xffffff1c),
    button_held: rgba_token(0xffffff16),
    button_retired: rgba_token(0xffffff0a),
    primary_button_hover: rgb_token(0x5c72da),
    danger_button_hover: rgb_token(0xb44757),
    fg_0: rgb_token(0xe2e6f0),
    fg_bright: rgb_token(0xf2f5ff),
    tunnel_fg: rgb_token(0xa0d8cf),
    tunnel_bg: rgb_token(0x213b3b),
    tunnel_border: rgb_token(0x436b68),
    fg_dim: rgb_token(0xbbc3d2),
    fg_1: rgb_token(0xa2aabb),
    fg_2: rgb_token(0x9ba5b8),
    on_fill: rgb_token(0xffffff),
    accent: rgb_token(0x8ea2f5),
    accent_fill: rgb_token(0x5268d0),
    accent_fill_hover: rgb_token(0x495fc7),
    accent_edge: rgb_token(0x5268d0),
    marked_bg: rgb_token(0x242c43),
    border: rgb_token(0x485367),
    border_dim: rgb_token(0x303846),
    border_nested: rgb_token(0x485367),
    border_sidebar_edge: rgb_token(0x3c4657),
    border_row: rgb_token(0x303846),
    border_dialog: rgb_token(0x59667c),
    ok: rgb_token(0x7fcf9a),
    warn: rgb_token(0xe5bb6e),
    warn_bright: rgb_token(0xead09b),
    warn_bg: rgb_token(0x3d3424),
    warn_bg_dim: rgb_token(0x302b23),
    warn_border: rgb_token(0x6a5533),
    warn_badge_bg: rgba_token(0xe5bb6e24),
    warn_chip_bg: rgba_token(0xe5bb6e1f),
    danger: rgb_token(0xee8a8a),
    danger_strong: rgb_token(0xf09595),
    danger_soft: rgb_token(0xf2c0c4),
    danger_bg: rgb_token(0xa83e4d),
    danger_fill_hover: rgb_token(0x983644),
    danger_bg_dim: rgb_token(0x3a292f),
    // `--danger` at 14% alpha (0x24 is 14% of 255, rounded).
    danger_wash: rgba_token(0xee8a8a24),
    danger_wash_hover: rgba_token(0xee8a8a30),
    danger_wash_pulse: rgba_token(0xee8a8a39),
    update_ready_hover_ink: rgb_token(0xf09595),
    update_ready_pulse_ink: rgb_token(0xf09595),
    update_ready_hover_warn: rgb_token(0xe5bb6e),
    update_ready_pulse_warn: rgb_token(0xe5bb6e),
    danger_tint: rgb_token(0x50404a),
    info: rgb_token(0x8ea2f5),
    on_accent_dim: rgb_token(0xf4f6ff),
    link: rgb_token(0x9cb7ff),
    status_pulse_min: 60,
    selected_status_pulse_min: 85,
    // rgba(0, 0, 0, 0.82)
    scrim: rgba_token(0x000000d1),
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
    harness: [
        rgb_token(0xd97757),
        rgb_token(0x456bff),
        rgb_token(0x2fbfaf),
        rgb_token(0x4d9abf),
        rgb_token(0x9b4dff),
        rgb_token(0xd8b839),
        rgb_token(0x83b94b),
        rgb_token(0xe85d9a),
        rgb_token(0xd2b79a),
    ],
};

/// The old app's "Steel" light appearance. Layout and type are shared with dark; only roles change.
pub const LIGHT: Palette = Palette {
    dark: false,
    bg_0: rgb_token(0xf4f6fa),
    well: rgb_token(0xf1f3f8),
    bg_1: rgb_token(0xe9edf4),
    bg_2: rgb_token(0xffffff),
    settings_page: rgb_token(0xf4f6fa),
    card: rgb_token(0xffffff),
    control_hover: rgb_token(0xdde3ee),
    // Ink at 6%, 10.5%, 14%, and 3.5%, rounded to the nearest alpha byte.
    button_rest: rgba_token(0x1b21300f),
    button_hover: rgba_token(0x1b21301b),
    button_held: rgba_token(0x1b213024),
    button_retired: rgba_token(0x1b213009),
    primary_button_hover: rgb_token(0x4059c7),
    danger_button_hover: rgb_token(0x983644),
    fg_0: rgb_token(0x1b2130),
    fg_bright: rgb_token(0x111827),
    tunnel_fg: rgb_token(0x1c6865),
    tunnel_bg: rgb_token(0xd4eeeb),
    tunnel_border: rgb_token(0x81bdb4),
    fg_dim: rgb_token(0x414b5f),
    fg_1: rgb_token(0x4f5a70),
    fg_2: rgb_token(0x5a6579),
    on_fill: rgb_token(0xffffff),
    accent: rgb_token(0x445acf),
    accent_fill: rgb_token(0x4a63d8),
    accent_fill_hover: rgb_token(0x4059c7),
    accent_edge: rgb_token(0x4a63d8),
    marked_bg: rgb_token(0xd3daf0),
    border: rgb_token(0xb8c2d3),
    border_dim: rgb_token(0xd8deea),
    border_nested: rgb_token(0xb8c2d3),
    border_sidebar_edge: rgb_token(0xcbd3e0),
    border_row: rgb_token(0xd8deea),
    border_dialog: rgb_token(0xb8c2d3),
    ok: rgb_token(0x276a43),
    warn: rgb_token(0x79531b),
    warn_bright: rgb_token(0x694719),
    warn_bg: rgb_token(0xfff0d2),
    warn_bg_dim: rgb_token(0xfaf2e3),
    warn_border: rgb_token(0xd6bc87),
    warn_badge_bg: rgba_token(0x79531b24),
    warn_chip_bg: rgba_token(0x79531b1f),
    danger: rgb_token(0xa63e4d),
    danger_strong: rgb_token(0xa13443),
    danger_soft: rgb_token(0xf2c0c4),
    danger_bg: rgb_token(0xa83e4d),
    danger_fill_hover: rgb_token(0x983644),
    danger_bg_dim: rgb_token(0xfae9ec),
    // `--danger` at 14% alpha (0x24 is 14% of 255, rounded).
    danger_wash: rgba_token(0xa63e4d24),
    danger_wash_hover: rgba_token(0xa63e4d38),
    danger_wash_pulse: rgb_token(0xa83e4d),
    update_ready_hover_ink: rgb_token(0x8b2837),
    update_ready_pulse_ink: rgb_token(0xffffff),
    update_ready_hover_warn: rgb_token(0x694719),
    update_ready_pulse_warn: rgb_token(0xffffff),
    danger_tint: rgb_token(0xedd8db),
    info: rgb_token(0x445acf),
    on_accent_dim: rgb_token(0xf4f6ff),
    link: rgb_token(0x3158bd),
    status_pulse_min: 73,
    selected_status_pulse_min: 85,
    // rgba(0, 0, 0, 0.82)
    scrim: rgba_token(0x000000d1),
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
    harness: [
        rgb_token(0xd25e39),
        rgb_token(0x456bff),
        rgb_token(0x249084),
        rgb_token(0x3e89ad),
        rgb_token(0x9b4dff),
        rgb_token(0x987f1e),
        rgb_token(0x638d37),
        rgb_token(0xc73c7a),
        rgb_token(0x886349),
    ],
};

/// The terminal's colors: Ghostty's default theme, which the old app's terminal used (`terminal-theme.js` at the
/// reference commit, itself read from Ghostty's `Config.zig` and `color.zig`).
///
/// The terminal surface stays dark in both appearances, as in the old app, so this is one palette, not a pair. A
/// program can still name any color: the 256-color index, truecolor and the OSC 10/11 defaults resolve against these
/// values in the grid model ([`TERMINAL_GRID`] is their form there), and [`TerminalColor::hsla`] is the one place
/// the results become GPUI colors, under this module's exemption.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalPalette {
    /// `foreground`: the default text color.
    pub foreground: Token,
    /// `background`: the default cell and pane ground.
    pub background: Token,
    /// `cursor`: the cursor fill and outline.
    pub cursor: Token,
    /// `cursorAccent`: the glyph under a filled block cursor.
    pub cursor_accent: Token,
    /// The sixteen ANSI colors, `black` through `brightWhite`, in index order.
    pub ansi: [Token; 16],
}

/// Ghostty's default terminal theme, as the old app's `terminal-theme.js` held it.
pub const TERMINAL: TerminalPalette = TerminalPalette {
    foreground: rgb_token(0xffffff),
    background: rgb_token(0x282c34),
    cursor: rgb_token(0xffffff),
    cursor_accent: rgb_token(0x282c34),
    ansi: [
        rgb_token(0x1d1f21),
        rgb_token(0xcc6666),
        rgb_token(0xb5bd68),
        rgb_token(0xf0c674),
        rgb_token(0x81a2be),
        rgb_token(0xb294bb),
        rgb_token(0x8abeb7),
        rgb_token(0xc5c8c6),
        rgb_token(0x666666),
        rgb_token(0xd54e53),
        rgb_token(0xb9ca4a),
        rgb_token(0xe7c547),
        rgb_token(0x7aa6da),
        rgb_token(0xc397d8),
        rgb_token(0x70c0b1),
        rgb_token(0xeaeaea),
    ],
};

/// What a terminal cell color resolves against: the role's default when the program named none, and whether bold
/// promotes the eight base ANSI colors to their bright variants (the old app's `grid-style.js` rule).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalRole {
    /// The color used for [`farhelm_grid_model::Color::Default`].
    pub default: TerminalColor,
    /// Bold text promotes ANSI 0-7 to 8-15.
    pub bright: bool,
}

/// A resolved terminal color: an RGB value from the palette or the program, kept as plain data so styles can be
/// compared and cached; [`TerminalColor::hsla`] turns it into a GPUI color when the terminal paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalColor(u32);

impl TerminalColor {
    /// A palette token's color. Terminal tokens are opaque; an alpha token is never a terminal role.
    pub fn of(token: Token) -> Self {
        Self(token.value)
    }

    /// The color as GPUI paints it, with `alpha` (the terminal's dim attribute draws at half alpha).
    pub fn hsla(self, alpha: f32) -> Hsla {
        raw::rgb_value(self.0).alpha(alpha)
    }

    /// The same color in the grid model's form, for handing a resolved color back to its arithmetic.
    pub fn rgb(self) -> farhelm_grid_model::Rgb {
        farhelm_grid_model::Rgb::from_hex(self.0)
    }
}

/// A color the grid model resolved. This is the same door [`terminal_color`] already opens for a program's truecolor:
/// the value is still plain data until [`TerminalColor::hsla`] paints it under this module's exemption.
impl From<farhelm_grid_model::Rgb> for TerminalColor {
    fn from(rgb: farhelm_grid_model::Rgb) -> Self {
        Self(rgb.hex())
    }
}

/// [`TERMINAL`]'s colors in the grid model's form, which is what a terminal's [`farhelm_grid_model::GridModel`] is
/// created with and what [`terminal_color`] and the terminal's style resolution resolve against.
///
/// This is a projection, not a second copy: the values are read from [`TERMINAL`] at compile time, so the two cannot
/// drift, and a constant means every cell's resolution borrows it rather than rebuilding it. The cursor colors are
/// left out because only the renderer draws a cursor.
pub const TERMINAL_GRID: farhelm_grid_model::TerminalPalette = terminal_palette();

/// Build [`TERMINAL_GRID`]; only that constant calls this.
const fn terminal_palette() -> farhelm_grid_model::TerminalPalette {
    use farhelm_grid_model::Rgb;
    let mut ansi = [Rgb { r: 0, g: 0, b: 0 }; 16];
    // A `while` loop because `const fn` cannot call `array::map` or iterate yet.
    let mut i = 0;
    while i < ansi.len() {
        ansi[i] = Rgb::from_hex(TERMINAL.ansi[i].value);
        i += 1;
    }
    farhelm_grid_model::TerminalPalette {
        foreground: Rgb::from_hex(TERMINAL.foreground.value),
        background: Rgb::from_hex(TERMINAL.background.value),
        ansi,
    }
}

/// Resolve a wire color for one role: the role's default, a palette index (0-15 from [`TERMINAL`], 16-231 the xterm
/// color cube, 232-255 the gray ramp) or a program's truecolor.
///
/// The arithmetic is the grid model's ([`farhelm_grid_model::colors::color`]), so the model and the screen agree on
/// every color; only the values come from here.
pub fn terminal_color(color: farhelm_grid_model::Color, role: TerminalRole) -> TerminalColor {
    farhelm_grid_model::colors::color(
        &TERMINAL_GRID,
        color,
        farhelm_grid_model::colors::ColorRole {
            default: role.default.rgb(),
            bright: role.bright,
        },
    )
    .into()
}

/// Sizes the slice's surfaces use, copied from the old stylesheet. Shared by both appearances.
pub mod metrics {
    use super::{Pixels, px};

    /// Standard corners on buttons, fields and surfaces that share their shape.
    pub const RADIUS_SM: Pixels = px(5.);
    /// Tighter corners for the small checkbox and key cap affordances.
    pub const RADIUS_CAP: Pixels = px(3.);
    /// The well around a segmented control allows room for its 5-pixel segments.
    pub const RADIUS_SEGMENTS: Pixels = px(7.);
    /// Corners on panels, cards, dialogs and tooltips that keep the old panel shape.
    pub const RADIUS_MD: Pixels = px(4.);
    /// `--radius-lg`: pill-ish shapes (the Settings switch's track).
    pub const RADIUS_LG: Pixels = px(8.);
    /// The common control text size (`.btn`, `.menu-item`, tooltips, dialog summaries).
    pub const TEXT_SM: Pixels = px(12.);
    /// Row titles and settings choices.
    pub const TEXT_MD: Pixels = px(13.);
    /// `.dialog-title`.
    pub const TEXT_TITLE: Pixels = px(18.);
    /// `.rename-input`'s line height: `line-height: 1.3` at the 13-pixel text size. With Kit's 8-pixel vertical padding
    /// and its border, a one-row field is 35 pixels high, as the old Rename field was.
    pub const RENAME_LINE_HEIGHT: Pixels = px(16.9);
    /// `.templates-field textarea`'s line height: `line-height: 1.45` at the 13-pixel text size, the old multi-line
    /// boxes' line, which a growing text box's rows are measured in.
    pub const GROWING_LINE_HEIGHT: Pixels = px(18.85);
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
    /// `--dialog-vertical-margin`'s floor: space kept above and below a dialog is 5% of the window's height, never
    /// less than this (`max(24px, 5dvh)`, `app.css` at the reference commit, line 128). The dialog frame applies the 5%.
    pub const DIALOG_VERTICAL_MARGIN: Pixels = px(24.);
    /// The old dialog backdrops' side padding: space kept beside a dialog, so a dialog never runs past either side of a
    /// narrow window. `app.css` at the reference commit gives `padding: var(--dialog-vertical-margin) 20px` to
    /// `.host-settings-backdrop` (line 3480), `.rename-dialog-backdrop` (line 4541) and `.launch-composer-backdrop`
    /// (line 6531); a dialog's `width: min(Npx, 100%)` resolved inside that padding.
    pub const DIALOG_SIDE_MARGIN: Pixels = px(20.);
    /// `.farhelm-tooltip`'s max width (`min(280px, 100vw - 8px)`; the layer manager applies the viewport half).
    pub const TOOLTIP_MAX_WIDTH: Pixels = px(280.);
    /// `.farhelm-tooltip`'s padding, vertical then horizontal.
    pub const TOOLTIP_PAD: (Pixels, Pixels) = (px(4.), px(8.));
    /// `.farhelm-tooltip`'s line height: `line-height: 1.35` at the 12-pixel control size.
    pub const TOOLTIP_LINE_HEIGHT: Pixels = px(16.2);
    /// `.farhelm-tooltip`'s `-webkit-line-clamp`: longer help is cut at this many lines. Paragraph help has none.
    pub const TOOLTIP_MAX_LINES: usize = 8;
    /// `.farhelm-tooltip[data-paragraphs="true"]`'s max width (`min(440px, 100vw - 8px)`; the layer manager applies
    /// the viewport half): paragraph help's wider card.
    pub const TOOLTIP_PARAGRAPHS_MAX_WIDTH: Pixels = px(440.);
    /// The gap between two paragraphs of paragraph help: one empty line at the tooltip's line height, which is what the
    /// blank line between paragraphs left under the old card's `white-space: pre-line`.
    pub const TOOLTIP_PARAGRAPH_GAP: Pixels = TOOLTIP_LINE_HEIGHT;
    /// `--sidebar-width`: the sidebar column's default width, which the screenshot goldens draw the session list at.
    /// It holds the 56-pixel host rail as well as the session list beside it, as the old width did. The app's sidebar
    /// draws the width the helm stores instead, defaulting to the same 396 pixels (`crate::sidebar`).
    pub const SIDEBAR_WIDTH: Pixels = px(396.);
    /// `--row-slot`: the width of a session row's status and locality columns.
    pub const ROW_SLOT: Pixels = px(12.);
    /// `--row-slot-gap`: the gap after each of those columns.
    pub const ROW_SLOT_GAP: Pixels = px(6.);
    /// `.session-row-open`'s padding: (top and bottom, left). The right edge has none.
    pub const ROW_PAD: (Pixels, Pixels) = (px(6.), px(12.));
    /// `.session-title`'s font size.
    pub const TEXT_ROW_TITLE: Pixels = px(14.);
    /// `.status-dot`: the status mark's box.
    pub const STATUS_DOT: Pixels = px(8.);
    /// `.session-heading`'s padding: (top and bottom, left and right).
    pub const LIST_HEADING_PAD: (Pixels, Pixels) = (px(4.), px(8.));
    /// `.status`'s padding (2em at its 14px): the list's own one-line states, such as "No sessions".
    pub const LIST_STATUS_PAD: Pixels = px(28.);
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

/// The palette every view paints with. Without [`install`] (a bootstrap bug, which fails a debug build) this is dark,
/// the old app's default look.
pub fn palette(cx: &App) -> &'static Palette {
    installed::<AppTheme>(cx)
        .and_then(|theme| theme.applied)
        .unwrap_or(&DARK)
}

/// Commit a user choice and repaint. Choosing the current value again changes nothing.
pub fn set_preference(preference: Preference, cx: &mut App) {
    let Some(theme) = installed_mut::<AppTheme>(cx) else {
        return;
    };
    if theme.preference == preference {
        return;
    }
    theme.preference = preference;
    apply(cx);
    // The control's selected state changes even when the palette does not (System to the mode it already resolved to).
    cx.refresh_windows();
}

/// The operating-system seam: production reaches it through [`observe_window`] and the bootstrap; tests call it with
/// the appearance a platform would report.
pub fn follow_system(system: WindowAppearance, cx: &mut App) {
    let Some(theme) = installed_mut::<AppTheme>(cx) else {
        return;
    };
    theme.system = system;
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
    // Kit's theme, which Kit's own init installs ahead of this one in the bootstrap.
    if installed::<Theme>(cx).is_some() {
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
/// [`color`], and only this module can make a token. The terminal's resolved colors take the other way through here,
/// `rgb_value`, which is how a program's truecolor and the 256-color cube reach the screen (the orchestrator's
/// decision of 2026-11-13 in the module doc). Any other color that is not in the palette tables would need a second
/// exemption that review would see.
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

    /// An opaque `0xRRGGBB` as a GPUI color, for the terminal colors [`super::TerminalColor`] resolved inside this
    /// module (a palette token or a program's own color, which no token can name).
    pub(super) fn rgb_value(value: u32) -> Hsla {
        gpui::rgb(value).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read the three sRGB channels before compositing a translucent wash over the sidebar.
    fn srgb(token: Token) -> [f64; 3] {
        let rgb = if token.alpha {
            token.value >> 8
        } else {
            token.value
        };
        [16, 8, 0].map(|shift| f64::from((rgb >> shift) & 0xff) / 255.)
    }

    /// Relative luminance uses linear channels. Washes are composited in sRGB before this conversion.
    fn luminance(rgb: [f64; 3]) -> f64 {
        let linear = rgb.map(|channel| {
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
    }

    /// The small readout text and warning stay above 4.5:1 over the pill as drawn on the sidebar. The stronger
    /// hover wash would reduce both below that floor with their resting inks.
    #[test]
    fn light_update_pill_text_has_contrast_on_every_fill() {
        assert_eq!(LIGHT.danger_wash, rgba_token(0xa63e4d24));
        assert_eq!(LIGHT.danger_strong, rgb_token(0xa13443));
        assert_eq!(LIGHT.danger_wash_hover, rgba_token(0xa63e4d38));
        assert_eq!(LIGHT.update_ready_hover_ink, rgb_token(0x8b2837));
        assert_eq!(LIGHT.danger_wash_pulse, LIGHT.danger_bg);
        assert_eq!(LIGHT.update_ready_pulse_ink, LIGHT.on_fill);
        assert_eq!(LIGHT.update_ready_hover_warn, LIGHT.warn_bright);
        assert_eq!(LIGHT.update_ready_pulse_warn, LIGHT.on_fill);
        assert_eq!(DARK.update_ready_hover_warn, DARK.warn);
        assert_eq!(DARK.update_ready_pulse_warn, DARK.warn);

        let backdrop = srgb(LIGHT.bg_1);
        for (fill, inks) in [
            (LIGHT.danger_wash, [LIGHT.danger_strong, LIGHT.warn]),
            (
                LIGHT.danger_wash_hover,
                [LIGHT.update_ready_hover_ink, LIGHT.update_ready_hover_warn],
            ),
            (
                LIGHT.danger_wash_pulse,
                [LIGHT.update_ready_pulse_ink, LIGHT.update_ready_pulse_warn],
            ),
        ] {
            let alpha = if fill.alpha {
                f64::from(fill.value & 0xff) / 255.
            } else {
                1.
            };
            let wash = srgb(fill);
            let background = luminance(std::array::from_fn(|i| {
                alpha * wash[i] + (1. - alpha) * backdrop[i]
            }));
            for ink in inks {
                let foreground = luminance(srgb(ink));
                let contrast =
                    (background.max(foreground) + 0.05) / (background.min(foreground) + 0.05);
                assert!(
                    contrast >= 4.5,
                    "{ink:?} on {fill:?} has {contrast:.2}:1 contrast"
                );
            }
        }
    }

    /// A terminal link's warning marks the part of the destination that differs from what the link text says as an
    /// inverted chip, `danger_bg` text on an `on_fill` ground (`terminal::disclosure`'s `mark_style`). The mark is the
    /// part the user most needs to read, so it must stay legible in both appearances. Spec: both tokens are opaque,
    /// and their WCAG contrast is at least 4.5:1 in the dark and the light palette.
    #[test]
    fn a_link_warnings_marked_difference_is_readable_in_both_appearances() {
        for palette in [&DARK, &LIGHT] {
            assert!(
                !palette.danger_bg.alpha && !palette.on_fill.alpha,
                "the premise: the chip is drawn in opaque colors, dark: {}",
                palette.dark
            );
            let [ink, ground] =
                [palette.danger_bg, palette.on_fill].map(|token| luminance(srgb(token)));
            let contrast = (ink.max(ground) + 0.05) / (ink.min(ground) + 0.05);
            assert!(
                contrast >= 4.5,
                "the mark has {contrast:.2}:1 contrast, dark: {}",
                palette.dark
            );
        }
    }

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

    /// SPEC_impl.md's rule for a missing global: a debug build fails at once, naming it, rather than quietly carrying
    /// on without it. The palette stands in for every foundation's global, since they all go through the same
    /// bootstrap lookup. Without this test, that lookup could go back to a silent fallback, the regression the rule
    /// exists to prevent, and every other test would still pass. A release build only logs, so the test exists only
    /// where the assertion does.
    #[cfg(debug_assertions)]
    #[gpui::test]
    #[should_panic(expected = "AppTheme is missing")]
    fn reading_the_palette_before_the_bootstrap_fails_a_debug_build(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            palette(cx);
        });
    }
}

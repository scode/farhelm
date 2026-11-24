//! The arithmetic that turns a cell's wire style into the colors the terminal draws it with.
//!
//! NOTE: This module holds no palette of its own. The values (Ghostty's default theme) live in the desktop app's theme
//! (`crates/gui/farhelm-gui/src/theme.rs`), which the migration's orchestrator decided on 2026-11-13 is the only place
//! the desktop app writes the terminal palette down (the supervisor's emulator keeps its own copy, in
//! `crates/farhelm-terminal-emulator/src/theme.rs`, for answering color queries). The owner hands a [`TerminalPalette`] to [`crate::GridModel::new`], and the
//! GUI's renderer resolves through these same functions, so what the model reasons about and what the screen shows come
//! from one set of rules applied to one set of values.
//!
//! The rules are the old app's `grid-style.js` at the reference commit, as the GPUI port carried them: inverse swaps
//! the roles before bold promotes ANSI 0-7 to their bright variants, bold never touches the background, dim halves the
//! foreground's alpha but never the background's, and the program's OSC 10/11 defaults resolve without bright
//! promotion. Underline color, strikethrough and the decoration shapes are paint, not ink, and stay with the renderer.
//!
//! This is a model of what the renderer asks GPUI to draw. It does not know about fonts, antialiasing or the
//! compositor, and nothing here measures pixels.

use farhelm_proto::grid::{Color, Style, TerminalState, style_flags as flags};

/// An opaque sRGB color, eight bits per channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// The color written as `0xRRGGBB`, the way the theme's tables and CSS hex write it. Bits above the low 24 are
    /// ignored.
    pub const fn from_hex(value: u32) -> Self {
        Self {
            r: (value >> 16) as u8,
            g: (value >> 8) as u8,
            b: value as u8,
        }
    }

    /// The color as `0xRRGGBB`; the inverse of [`Rgb::from_hex`] for any 24-bit value.
    pub const fn hex(self) -> u32 {
        (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }
}

/// The colors a terminal resolves against before the program changes anything: the default foreground and
/// background, and the sixteen ANSI colors (`black` through `brightWhite`, in index order).
///
/// Indices 16-255 are not part of a palette: the xterm color cube and gray ramp are computed by [`indexed`]. A program
/// that redefines an entry with OSC 4 sends the new value as a truecolor on the wire, so it never reaches this table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalPalette {
    pub foreground: Rgb,
    pub background: Rgb,
    pub ansi: [Rgb; 16],
}

/// What a wire color resolves against in one position (foreground, background or underline).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorRole {
    /// The color [`Color::Default`] means in this position, after any inverse swap and the program's OSC 10/11.
    pub default: Rgb,
    /// Bold text promotes ANSI 0-7 to 8-15. Only the foreground and an explicit underline color ever set this.
    pub bright: bool,
}

/// One run's glyph color, its alpha and the background behind it.
///
/// `fg_alpha` is 0.5 for dim text and 1 otherwise. It is kept apart from `fg` because the renderer applies it at paint
/// time; blending it into `fg` here would lose the information that the background never dims.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ink {
    pub fg: Rgb,
    pub fg_alpha: f32,
    pub bg: Rgb,
}

/// One of the 256 xterm indices: 0-15 from the palette, 16-231 the 6x6x6 color cube, 232-255 the gray ramp.
pub fn indexed(palette: &TerminalPalette, index: u8) -> Rgb {
    match index {
        0..=15 => palette.ansi[usize::from(index)],
        16..=231 => {
            let i = index - 16;
            let levels = [0u8, 95, 135, 175, 215, 255];
            Rgb {
                r: levels[usize::from(i / 36)],
                g: levels[usize::from(i / 6 % 6)],
                b: levels[usize::from(i % 6)],
            }
        }
        232..=255 => {
            let v = 8 + (index - 232) * 10;
            Rgb { r: v, g: v, b: v }
        }
    }
}

/// Resolve a wire color for one role: the role's default, a palette index (bold-promoted when the role asks and the
/// index is below 8), or the program's truecolor as sent.
pub fn color(palette: &TerminalPalette, color: Color, role: ColorRole) -> Rgb {
    match color {
        Color::Default => role.default,
        Color::Palette(index) => {
            let index = if role.bright && index < 8 {
                index + 8
            } else {
                index
            };
            indexed(palette, index)
        }
        Color::Rgb { r, g, b } => Rgb { r, g, b },
    }
}

/// The terminal's default foreground and background, after the program's OSC 10/11 overrides.
///
/// These color uncovered cells and the pane ground as well as cells in the default style. An override naming a
/// palette index resolves without bright promotion: there is no bold attribute on a default.
pub fn defaults(palette: &TerminalPalette, state: Option<TerminalState>) -> (Rgb, Rgb) {
    let role = |default| ColorRole {
        default,
        bright: false,
    };
    (
        color(
            palette,
            state.and_then(|s| s.default_fg).unwrap_or(Color::Default),
            role(palette.foreground),
        ),
        color(
            palette,
            state.and_then(|s| s.default_bg).unwrap_or(Color::Default),
            role(palette.background),
        ),
    )
}

/// Resolve one run's style against the palette and the terminal's current defaults.
///
/// Inverse swaps both the wire colors and their defaults first, so bold brightens whichever color ends up in the
/// foreground, and the background is never promoted. Conceal (`HIDDEN`) does not change the ink: concealed text keeps
/// its colors, and it is the renderer that leaves its glyphs out.
pub fn ink(palette: &TerminalPalette, style: Style, state: Option<TerminalState>) -> Ink {
    let (default_fg, default_bg) = defaults(palette, state);
    let bold = style.flags & flags::BOLD != 0;
    let (fg, bg, fg_default, bg_default) = if style.flags & flags::INVERSE != 0 {
        (style.bg, style.fg, default_bg, default_fg)
    } else {
        (style.fg, style.bg, default_fg, default_bg)
    };
    Ink {
        fg: color(
            palette,
            fg,
            ColorRole {
                default: fg_default,
                bright: bold,
            },
        ),
        fg_alpha: if style.flags & flags::DIM != 0 {
            0.5
        } else {
            1.0
        },
        bg: color(
            palette,
            bg,
            ColorRole {
                default: bg_default,
                bright: false,
            },
        ),
    }
}

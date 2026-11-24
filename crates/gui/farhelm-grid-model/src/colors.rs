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
//!
//! The second half of the module judges that ink: [`unseen`] says whether a run can be read at all, so that link
//! detection can treat text a program drew in (nearly) its background color the way it treats concealed text. A plain
//! click on a printed URL opens it without showing it first, which is only safe while the whole address is something
//! the user can actually see on the screen.

use std::sync::LazyLock;

use farhelm_proto::grid::{Color, Style, TerminalState, style_flags as flags};
use regex::Regex;

use crate::{GridRun, constant_regex, links::JS_SPACE};

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

// ---------------------------------------------------------------------------------------------------------------------
// Whether a run can be seen
// ---------------------------------------------------------------------------------------------------------------------

/// The glyph-to-background contrast below which a run counts as hidden: too close to its background to read.
///
/// The threshold is absolute, a maintainer decision: it does not depend on how faint the text around the run is. With
/// the desktop app's theme and the tests' xterm fixture alike, the dimmest ordinary styles stay above it (bright black
/// at about 2.4:1 and 4.3:1, dim bright black at about 1.55:1 and 2.05:1), while ANSI black on the default background
/// (about 1.2:1 in both) and any color on itself fall below.
pub const HIDDEN_CONTRAST: f64 = 1.35;

/// The WCAG 2 contrast ratio of two opaque colors, from 1 (the same luminance) to 21 (black on white).
///
/// Symmetric in its arguments. Relative luminance uses the sRGB transfer function's own 0.04045 knee, which WCAG 2.2
/// notes as the correct value. No 8-bit channel value lies between it and the older 0.03928, so the two agree here.
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// WCAG relative luminance of an sRGB color, in 0..=1.
fn luminance(color: Rgb) -> f64 {
    let linear = |channel: u8| {
        let value = f64::from(channel) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

/// The opaque color a run's glyphs come out as: the foreground composited over the background at its alpha.
///
/// The blend is in sRGB, on the encoded channel values, rounded to eight bits. That is what GPUI's frame buffer does on
/// both platforms: the wgpu renderer (Linux) picks a non-sRGB surface (`Bgra8Unorm`, or the first non-sRGB format the
/// adapter offers), and the Metal renderer (macOS, `gpui-pre-apple`'s `metal_renderer.rs`) sets its layer to
/// `MTLPixelFormat::BGRA8Unorm`, so the blend unit mixes encoded values without linearizing them. Only dim text has an
/// alpha below 1.
pub fn glyph(ink: Ink) -> Rgb {
    let alpha = f64::from(ink.fg_alpha).clamp(0.0, 1.0);
    let mix = |fg: u8, bg: u8| {
        (f64::from(fg) * alpha + f64::from(bg) * (1.0 - alpha))
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Rgb {
        r: mix(ink.fg.r, ink.bg.r),
        g: mix(ink.fg.g, ink.bg.g),
        b: mix(ink.fg.b, ink.bg.b),
    }
}

/// Text that draws nothing a reader could see whatever its color: whitespace (JavaScript's, plus the braille blank
/// and the Hangul fillers, which draw nothing either), format characters and combining marks.
///
/// A run made only of these is never hidden by its color, because there is nothing to hide. That matters for the
/// common case of a program clearing a region with spaces in its background color: those cells must keep reading as
/// spaces, not as something unseen.
static DRAWS_NOTHING: LazyLock<Regex> = LazyLock::new(|| {
    constant_regex(
        &r"^[%s\u{115F}\u{1160}\u{3164}\u{FFA0}\u{2800}\p{Cf}\p{M}]*$".replace("%s", JS_SPACE),
    )
});

/// Whether a run is **unseen**: concealed, or **hidden** (it has a character other than whitespace, a format
/// character or a combining mark, and its glyph-to-background contrast is below [`HIDDEN_CONTRAST`]).
///
/// The judgment is per run. A run has one style and so one ink, which is why a run is unseen or not as a whole and
/// no cell is ever judged apart from its run. `state` supplies the program's current OSC 10/11 defaults, so a run in
/// the default foreground becomes hidden when the program sets that default to its background, and stops being
/// hidden when it changes it back. Conceal wins regardless of color: a concealed run's glyphs are never drawn.
///
/// Callers that blank unseen runs must not assume the text is gone from the screen in every other sense: an unseen
/// run's cells are still selectable and copyable, and a program hyperlink still owns them.
pub fn unseen(palette: &TerminalPalette, run: &GridRun, state: Option<TerminalState>) -> bool {
    if run.style.flags & flags::HIDDEN != 0 {
        return true;
    }
    let ink = ink(palette, run.style, state);
    // The arithmetic is cheaper than the regex, and most runs are readable, so the text is only looked at for a run
    // whose color already hides it.
    contrast(glyph(ink), ink.bg) < HIDDEN_CONTRAST && !DRAWS_NOTHING.is_match(&run.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small palette for these tests: xterm's black and bright black on a dark ground, with a light default
    /// foreground. Only the entries the tests name matter; the rest are filler.
    fn palette() -> TerminalPalette {
        let mut ansi = [Rgb::from_hex(0x808080); 16];
        ansi[0] = Rgb::from_hex(0x000000);
        ansi[8] = Rgb::from_hex(0x7f7f7f);
        TerminalPalette {
            foreground: Rgb::from_hex(0xd0d0d0),
            background: Rgb::from_hex(0x1c1c1c),
            ansi,
        }
    }

    fn run(text: &str, flags: u16, fg: Color, bg: Color) -> GridRun {
        GridRun {
            col: 0,
            width: u16::try_from(text.chars().count()).unwrap_or(u16::MAX),
            text: text.to_owned(),
            style: Style {
                flags,
                fg,
                bg,
                underline_color: None,
            },
            link: None,
        }
    }

    /// The contrast formula is WCAG's, so its fixed points must come out exactly: the ends of the scale, symmetry, and
    /// a published mid-scale value. A wrong luminance weight or transfer function would move the 1.35 threshold to a
    /// different set of colors without any test of `unseen` noticing.
    #[test]
    fn contrast_matches_wcag_reference_values() {
        let black = Rgb::from_hex(0x000000);
        let white = Rgb::from_hex(0xffffff);
        assert!((contrast(black, white) - 21.0).abs() < 1e-9);
        assert!((contrast(white, black) - 21.0).abs() < 1e-9);
        assert!((contrast(white, white) - 1.0).abs() < 1e-9);
        // #777777 on white is the textbook "just fails AA" gray, 4.48:1.
        let gray = contrast(Rgb::from_hex(0x777777), white);
        assert!((gray - 4.48).abs() < 0.01, "{gray}");
    }

    /// Dim text is drawn at half alpha over its own background, and the background itself never dims. The blend is
    /// on encoded sRGB values, as the renderer's non-sRGB frame buffer does it.
    #[test]
    fn a_dim_glyph_is_the_srgb_midpoint_of_its_colors() {
        let ink = Ink {
            fg: Rgb::from_hex(0xffffff),
            fg_alpha: 0.5,
            bg: Rgb::from_hex(0x000000),
        };
        assert_eq!(glyph(ink), Rgb::from_hex(0x808080));
        let opaque = Ink {
            fg_alpha: 1.0,
            ..ink
        };
        assert_eq!(glyph(opaque), Rgb::from_hex(0xffffff));
    }

    /// Both ways a run can be unseen: concealed text is unseen whatever its colors, and colored text is hidden only
    /// when it both falls below the threshold and has something to draw. Blanks, format characters and combining marks
    /// in the background color stay seen, because a program clearing a region with background-colored spaces is
    /// ordinary.
    #[test]
    fn unseen_is_concealed_or_low_contrast_with_something_to_draw() {
        let palette = palette();
        let readable = run("x", 0, Color::Default, Color::Default);
        assert!(!unseen(&palette, &readable, None));
        assert!(unseen(
            &palette,
            &run("x", flags::HIDDEN, Color::Default, Color::Default),
            None
        ));
        let black = |text| run(text, 0, Color::Palette(0), Color::Default);
        assert!(contrast(Rgb::from_hex(0x000000), palette.background) < HIDDEN_CONTRAST);
        assert!(unseen(&palette, &black("x"), None));
        for blank in [
            " ", "\u{a0}", "\u{2800}", "\u{3164}", "\u{200b}", "\u{feff}", " \u{301}",
        ] {
            assert!(!unseen(&palette, &black(blank), None), "{blank:?}");
        }
        assert!(unseen(&palette, &black(" x "), None));
    }
}

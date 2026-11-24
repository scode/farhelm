//! How a grid cell's wire style becomes paint: colors, alpha and decorations.
//!
//! Ported from the GPUI prototype's terminal, whose rules follow the old app's `grid-style.js` at the reference
//! commit: inverse swaps the roles before bold promotes ANSI 0-7 to their bright variants, dim halves the foreground's
//! alpha but never the background's, an explicit underline color applies only to underlined text and keeps full alpha
//! (strikethrough shares it then), and when several underline bits are set the double one wins.
//!
//! The ink (the glyph color, its dim alpha and the background) is the grid model's [`colors::ink`], so the rules
//! the model judges text by and the colors the screen paints are one implementation; the palette values come from
//! [`theme::TERMINAL_GRID`]. What stays here is paint only the renderer needs: underline color and shape,
//! strikethrough, and the face attributes. Nothing here builds a color: the underline's goes through
//! [`theme::terminal_color`].

use farhelm_grid_model::{Color, Style, TerminalState, colors, style_flags as flags};

use crate::theme::{self, TerminalColor, TerminalRole};

/// The five decoration shapes a cell can carry; underline and strikethrough share one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Underline {
    Solid,
    Double,
    Wavy,
    Dotted,
    Dashed,
}

/// One run's resolved paint. Alphas are separate from the colors so the dim attribute never reaches the background.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedStyle {
    pub fg: TerminalColor,
    pub bg: TerminalColor,
    /// The foreground's alpha: 0.5 when dim, otherwise 1.
    pub alpha: f32,
    pub underline_color: TerminalColor,
    pub underline_alpha: f32,
    pub underline: Option<Underline>,
    pub bold: bool,
    pub italic: bool,
    /// Hidden text keeps its background and decorations; only the glyphs are not drawn.
    pub hidden: bool,
    pub strike: bool,
}

/// The terminal's default foreground and background, after the program's OSC 10/11 overrides. They color uncovered
/// cells and the pane ground as well as cells in the default style.
pub fn defaults(state: Option<TerminalState>) -> (TerminalColor, TerminalColor) {
    let (fg, bg) = colors::defaults(&theme::TERMINAL_GRID, state);
    (fg.into(), bg.into())
}

/// Resolve one wire style against the terminal's current defaults.
pub fn resolve(style: Style, state: Option<TerminalState>) -> ResolvedStyle {
    let ink = colors::ink(&theme::TERMINAL_GRID, style, state);
    let fg = TerminalColor::from(ink.fg);
    let alpha = ink.fg_alpha;
    let bold = style.flags & flags::BOLD != 0;
    let underline = if style.flags & flags::DOUBLE_UNDERLINE != 0 {
        Some(Underline::Double)
    } else if style.flags & flags::UNDERCURL != 0 {
        Some(Underline::Wavy)
    } else if style.flags & flags::DOTTED_UNDERLINE != 0 {
        Some(Underline::Dotted)
    } else if style.flags & flags::DASHED_UNDERLINE != 0 {
        Some(Underline::Dashed)
    } else if style.flags & flags::UNDERLINE != 0 {
        Some(Underline::Solid)
    } else {
        None
    };
    // An explicit underline color colors the decorations only while the text is underlined, as the old app's styleCss
    // set `text-decoration-color` only then: a strikethrough on text that is not underlined keeps the text's color and
    // its dim alpha.
    let explicit = style
        .underline_color
        .filter(|c| *c != Color::Default && underline.is_some());
    ResolvedStyle {
        fg,
        bg: ink.bg.into(),
        alpha,
        underline_color: explicit.map_or(fg, |c| {
            theme::terminal_color(
                c,
                TerminalRole {
                    default: fg,
                    bright: bold,
                },
            )
        }),
        underline_alpha: if explicit.is_some() { 1.0 } else { alpha },
        underline,
        bold,
        italic: style.flags & flags::ITALIC != 0,
        hidden: style.flags & flags::HIDDEN != 0,
        strike: style.flags & flags::STRIKETHROUGH != 0,
    }
}

//! How a grid cell's wire style becomes paint: colors, alpha and decorations.
//!
//! Ported from the GPUI prototype's terminal, whose rules follow the old app's `grid-style.js` at the reference
//! commit: inverse swaps the roles before bold promotes ANSI 0-7 to their bright variants, dim halves the foreground's
//! alpha but never the background's, an explicit underline color applies only to underlined text and keeps full alpha
//! (strikethrough shares it then), and when several underline bits are set the double one wins. The colors themselves
//! come from [`crate::theme::terminal_color`]; nothing here builds one.

use farhelm_grid_model::{Color, Style, TerminalState, style_flags as flags};

use crate::theme::{self, TERMINAL, TerminalColor, TerminalRole};

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
    let role = |default| TerminalRole {
        default: TerminalColor::of(default),
        bright: false,
    };
    (
        theme::terminal_color(
            state.and_then(|s| s.default_fg).unwrap_or(Color::Default),
            role(TERMINAL.foreground),
        ),
        theme::terminal_color(
            state.and_then(|s| s.default_bg).unwrap_or(Color::Default),
            role(TERMINAL.background),
        ),
    )
}

/// Resolve one wire style against the terminal's current defaults.
pub fn resolve(style: Style, state: Option<TerminalState>) -> ResolvedStyle {
    let (default_fg, default_bg) = defaults(state);
    let bold = style.flags & flags::BOLD != 0;
    let inverse = style.flags & flags::INVERSE != 0;
    let (fg, bg, fg_default, bg_default) = if inverse {
        (style.bg, style.fg, default_bg, default_fg)
    } else {
        (style.fg, style.bg, default_fg, default_bg)
    };
    let fg = theme::terminal_color(
        fg,
        TerminalRole {
            default: fg_default,
            bright: bold,
        },
    );
    let alpha = if style.flags & flags::DIM != 0 {
        0.5
    } else {
        1.0
    };
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
        bg: theme::terminal_color(
            bg,
            TerminalRole {
                default: bg_default,
                bright: false,
            },
        ),
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

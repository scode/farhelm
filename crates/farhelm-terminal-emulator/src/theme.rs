//! Farhelm's terminal colors, as answers to a program's color queries.
//!
//! Programs ask the terminal for its colors (OSC 4 for a palette entry, OSC
//! 10, 11 and 12 for the foreground, background and cursor), most often to
//! decide whether to draw for a light or a dark background. The answers must
//! be the colors the user actually sees, which are Farhelm's terminal theme:
//! Ghostty's default palette, kept for the browser in
//! `crates/farhelm-ui/assets/terminal-theme.js`. A test in this module reads
//! that file and fails if the two drift apart.
//!
//! The theme names only the 16 ANSI colors. Entries 16 to 255 are the
//! standard xterm 256-color extension (a 6x6x6 color cube, then a 24-step gray
//! ramp), which is what xterm.js, the browser terminal, derives for them too.

use alacritty_terminal::vte::ansi::{NamedColor, Rgb};

/// The 16 ANSI colors, black to bright white.
const ANSI: [Rgb; 16] = [
    rgb(0x1D, 0x1F, 0x21),
    rgb(0xCC, 0x66, 0x66),
    rgb(0xB5, 0xBD, 0x68),
    rgb(0xF0, 0xC6, 0x74),
    rgb(0x81, 0xA2, 0xBE),
    rgb(0xB2, 0x94, 0xBB),
    rgb(0x8A, 0xBE, 0xB7),
    rgb(0xC5, 0xC8, 0xC6),
    rgb(0x66, 0x66, 0x66),
    rgb(0xD5, 0x4E, 0x53),
    rgb(0xB9, 0xCA, 0x4A),
    rgb(0xE7, 0xC5, 0x47),
    rgb(0x7A, 0xA6, 0xDA),
    rgb(0xC3, 0x97, 0xD8),
    rgb(0x70, 0xC0, 0xB1),
    rgb(0xEA, 0xEA, 0xEA),
];

/// The default foreground (OSC 10).
const FOREGROUND: Rgb = rgb(0xFF, 0xFF, 0xFF);
/// The default background (OSC 11).
const BACKGROUND: Rgb = rgb(0x28, 0x2C, 0x34);
/// The cursor color (OSC 12).
const CURSOR: Rgb = rgb(0xFF, 0xFF, 0xFF);

const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

/// The theme's color for one of alacritty's color slots: palette entries 0 to
/// 255, then the foreground, background and cursor ([`NamedColor`]'s values).
/// `None` for the slots no query can name (alacritty's dim and bright
/// foreground variants).
pub fn color(index: usize) -> Option<Rgb> {
    const CUBE_LEVELS: [u8; 6] = [0x00, 0x5F, 0x87, 0xAF, 0xD7, 0xFF];
    match index {
        0..=15 => Some(ANSI[index]),
        16..=231 => {
            let cube = index - 16;
            Some(rgb(
                CUBE_LEVELS[cube / 36],
                CUBE_LEVELS[cube / 6 % 6],
                CUBE_LEVELS[cube % 6],
            ))
        }
        232..=255 => {
            let level = 8 + 10 * (index - 232) as u8;
            Some(rgb(level, level, level))
        }
        _ if index == NamedColor::Foreground as usize => Some(FOREGROUND),
        _ if index == NamedColor::Background as usize => Some(BACKGROUND),
        _ if index == NamedColor::Cursor as usize => Some(CURSOR),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The browser's theme file, which this module must agree with.
    const THEME_JS: &str = include_str!("../../farhelm-ui/assets/terminal-theme.js");

    /// The `#RRGGBB` value the theme file gives `key`.
    fn theme_js(key: &str) -> Rgb {
        let needle = format!("{key}: \"#");
        let start = THEME_JS
            .find(&needle)
            .unwrap_or_else(|| panic!("terminal-theme.js has no {key}"))
            + needle.len();
        let hex = &THEME_JS[start..start + 6];
        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex digits");
        rgb(channel(0), channel(2), channel(4))
    }

    /// Every color the theme file names is the one this crate answers with.
    ///
    /// Why: programs choose light or dark output from these answers, so an
    /// answer that differs from what the browser paints makes them choose
    /// wrong. The two lists are in two languages and nothing else ties them
    /// together.
    #[test]
    fn answers_match_the_browser_theme() {
        let names = [
            "black",
            "red",
            "green",
            "yellow",
            "blue",
            "magenta",
            "cyan",
            "white",
            "brightBlack",
            "brightRed",
            "brightGreen",
            "brightYellow",
            "brightBlue",
            "brightMagenta",
            "brightCyan",
            "brightWhite",
        ];
        for (index, name) in names.iter().enumerate() {
            assert_eq!(
                color(index),
                Some(theme_js(name)),
                "palette entry {index} ({name})"
            );
        }
        assert_eq!(
            color(NamedColor::Foreground as usize),
            Some(theme_js("foreground"))
        );
        assert_eq!(
            color(NamedColor::Background as usize),
            Some(theme_js("background"))
        );
        assert_eq!(color(NamedColor::Cursor as usize), Some(theme_js("cursor")));
    }

    /// The extended palette follows the xterm formula at its corners and
    /// steps.
    ///
    /// Why: it is computed rather than listed, and xterm.js computes the
    /// same values for the browser; a slip in the arithmetic would go
    /// unnoticed until a program asked.
    #[test]
    fn extended_palette_follows_the_xterm_formula() {
        assert_eq!(color(16), Some(rgb(0, 0, 0)));
        assert_eq!(color(17), Some(rgb(0, 0, 0x5F)));
        assert_eq!(color(21), Some(rgb(0, 0, 0xFF)));
        assert_eq!(color(22), Some(rgb(0, 0x5F, 0)));
        assert_eq!(color(52), Some(rgb(0x5F, 0, 0)));
        assert_eq!(color(196), Some(rgb(0xFF, 0, 0)));
        assert_eq!(color(231), Some(rgb(0xFF, 0xFF, 0xFF)));
        assert_eq!(color(232), Some(rgb(8, 8, 8)));
        assert_eq!(color(255), Some(rgb(0xEE, 0xEE, 0xEE)));
        assert_eq!(color(NamedColor::DimBlack as usize), None);
    }
}

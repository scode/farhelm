//! Helpers every grid-model integration test binary shares. Each binary includes this with `mod common;`.

use farhelm_grid_model::{Rgb, TerminalPalette};

/// The palette every test model is created with.
///
/// It is deliberately not the desktop app's theme. The grid model has no palette of its own and takes whatever its
/// owner draws with, so tests that depend on a color say so through this fixture instead of reaching for production
/// values, and a change to the app's look cannot quietly change what these tests exercise. The values are xterm's
/// classic sixteen on a dark ground, which keeps the same broad shape as the real terminal (light text on a dark
/// background, ANSI black close to that background) without copying it.
pub fn palette() -> TerminalPalette {
    TerminalPalette {
        foreground: Rgb::from_hex(0xd0d0d0),
        background: Rgb::from_hex(0x1c1c1c),
        ansi: [
            0x000000, 0xcd0000, 0x00cd00, 0xcdcd00, 0x0000ee, 0xcd00cd, 0x00cdcd, 0xe5e5e5,
            0x7f7f7f, 0xff0000, 0x00ff00, 0xffff00, 0x5c5cff, 0xff00ff, 0x00ffff, 0xffffff,
        ]
        .map(Rgb::from_hex),
    }
}

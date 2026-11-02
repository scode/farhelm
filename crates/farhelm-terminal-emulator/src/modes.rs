//! The terminal modes a program sets that change how the terminal's input
//! must be encoded, or how its screen is read.
//!
//! The emulator only reports them; encoding keys, mouse events, focus changes
//! and pastes from them is the input side's job. Keyboard modes are legacy
//! xterm ones only: alacritty's kitty keyboard protocol stays off, as in the
//! browser terminal today, so a program's request to turn it on is ignored and
//! never shows up here.

use alacritty_terminal::term::TermMode;

/// Which mouse events the program asked to receive. The modes exclude each
/// other: setting one replaces the others, and resetting any of them (or
/// 1001, highlight tracking, which is otherwise ignored) turns reporting
/// off, as in tmux 3.7c.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseReporting {
    /// No mouse reporting; the viewer handles the mouse itself.
    Off,
    /// Button presses and releases (mode 1000, "normal" tracking).
    Clicks,
    /// Presses, releases, and motion while a button is held (mode 1002,
    /// "button-event" tracking).
    Drag,
    /// Presses, releases, and all motion (mode 1003, "any-event" tracking).
    Motion,
}

/// How mouse events are encoded when reported: SGR while the program has
/// mode 1006 set, else UTF-8 coordinates while it has 1005 set, else the
/// default encoding.
///
/// 1005 and 1006 are separate modes a program can hold at once (the
/// vendored patch, as in tmux), and SGR wins whichever was set first, as in
/// tmux and the browser terminal today (which does not implement 1005 at
/// all). xterm, and upstream alacritty, keep only the one set last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEncoding {
    /// The original X10-style encoding, `CSI M` and three bytes, which cannot
    /// express a coordinate past 223.
    Default,
    /// The same with coordinates as UTF-8 characters (mode 1005).
    Utf8,
    /// `CSI < b ; x ; y M` or `m` (mode 1006).
    Sgr,
}

/// A program's terminal modes at one moment, read from the emulator with
/// [`crate::Emulator::modes`].
///
/// X10 compatibility mouse mode (9) is absent because alacritty does not
/// implement it; tmux does not either, and answers DECRQM 9 as not
/// recognized. Today the browser terminal does honor it, since it sees the
/// program's raw output, so the input encoder loses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modes {
    /// Bracketed paste (mode 2004): pasted text must be wrapped in
    /// `CSI 200 ~` and `CSI 201 ~`.
    pub bracketed_paste: bool,
    /// Which mouse events to report.
    pub mouse_reporting: MouseReporting,
    /// How to encode them.
    pub mouse_encoding: MouseEncoding,
    /// Focus reporting (mode 1004): send `CSI I` and `CSI O` when the
    /// terminal gains and loses focus.
    pub focus_reporting: bool,
    /// Application cursor keys (DECCKM, mode 1): arrows send `ESC O A`
    /// rather than `CSI A`.
    pub application_cursor: bool,
    /// Application keypad (DECKPAM, `ESC =`): the keypad sends `ESC O`
    /// sequences rather than digits.
    pub application_keypad: bool,
    /// The alternate screen is showing (mode 1049, 47 or 1047): the screen
    /// holds a full-screen program's display, and scrollback belongs to the
    /// primary screen underneath.
    pub alternate_screen: bool,
    /// The cursor is visible (DECTCEM, mode 25).
    pub cursor_visible: bool,
}

impl Modes {
    /// Read the modes out of alacritty's mode flags.
    pub(crate) fn from_term_mode(mode: TermMode) -> Modes {
        let mouse_reporting = if mode.contains(TermMode::MOUSE_MOTION) {
            MouseReporting::Motion
        } else if mode.contains(TermMode::MOUSE_DRAG) {
            MouseReporting::Drag
        } else if mode.contains(TermMode::MOUSE_REPORT_CLICK) {
            MouseReporting::Clicks
        } else {
            MouseReporting::Off
        };
        let mouse_encoding = if mode.contains(TermMode::SGR_MOUSE) {
            MouseEncoding::Sgr
        } else if mode.contains(TermMode::UTF8_MOUSE) {
            MouseEncoding::Utf8
        } else {
            MouseEncoding::Default
        };
        Modes {
            bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
            mouse_reporting,
            mouse_encoding,
            focus_reporting: mode.contains(TermMode::FOCUS_IN_OUT),
            application_cursor: mode.contains(TermMode::APP_CURSOR),
            application_keypad: mode.contains(TermMode::APP_KEYPAD),
            alternate_screen: mode.contains(TermMode::ALT_SCREEN),
            cursor_visible: mode.contains(TermMode::SHOW_CURSOR),
        }
    }
}

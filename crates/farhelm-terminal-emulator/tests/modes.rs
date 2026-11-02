//! The mode accessors that input encoding and prompt sending will read.
//!
//! Prompt sending checks bracketed paste before it pastes into an agent, and
//! the input encoder (a later change) turns keys, mouse events and focus
//! changes into bytes according to these modes. Each must follow the
//! program's own set and reset sequences.

mod common;

use common::{emulator, feed};
use farhelm_terminal_emulator::{Modes, MouseEncoding, MouseReporting};

/// A fresh terminal's modes: everything off, cursor visible.
///
/// Why: a program that sets nothing must get plain encoding: no paste
/// brackets, no mouse reports, normal cursor keys.
#[test]
fn a_fresh_terminal_has_default_modes() {
    assert_eq!(
        emulator(80, 24).modes(),
        Modes {
            bracketed_paste: false,
            mouse_reporting: MouseReporting::Off,
            mouse_encoding: MouseEncoding::Default,
            focus_reporting: false,
            application_cursor: false,
            application_keypad: false,
            alternate_screen: false,
            cursor_visible: true,
        }
    );
}

/// Each mode follows its set and reset sequence.
///
/// Why: a mode that does not reset leaves, say, paste brackets around every
/// prompt sent to a shell after the agent exits.
#[test]
fn each_mode_follows_its_sequences() {
    type Check = fn(&Modes) -> bool;
    let cases: &[(&str, &str, &str, Check)] = &[
        ("bracketed paste", "\x1b[?2004h", "\x1b[?2004l", |m| {
            m.bracketed_paste
        }),
        ("focus reporting", "\x1b[?1004h", "\x1b[?1004l", |m| {
            m.focus_reporting
        }),
        ("application cursor keys", "\x1b[?1h", "\x1b[?1l", |m| {
            m.application_cursor
        }),
        ("application keypad", "\x1b=", "\x1b>", |m| {
            m.application_keypad
        }),
        ("alternate screen", "\x1b[?1049h", "\x1b[?1049l", |m| {
            m.alternate_screen
        }),
        ("hidden cursor", "\x1b[?25l", "\x1b[?25h", |m| {
            !m.cursor_visible
        }),
    ];
    for (name, set, reset, is_set) in cases {
        let mut emulator = emulator(80, 24);
        feed(&mut emulator, set.as_bytes());
        assert!(is_set(&emulator.modes()), "{name} after {set:?}");
        feed(&mut emulator, reset.as_bytes());
        assert!(!is_set(&emulator.modes()), "{name} after {reset:?}");
    }
}

/// Mouse reporting modes replace one another, as do mouse encodings, and
/// turning off the active one leaves reporting off.
///
/// Why: the encoder must report exactly the events the program asked for
/// last, in the encoding it asked for last.
#[test]
fn mouse_modes_replace_one_another() {
    let mut emulator = emulator(80, 24);
    let mut expect = |output: &str, reporting, encoding| {
        feed(&mut emulator, output.as_bytes());
        let modes = emulator.modes();
        assert_eq!(
            (modes.mouse_reporting, modes.mouse_encoding),
            (reporting, encoding),
            "after {output:?}"
        );
    };
    expect(
        "\x1b[?1000h",
        MouseReporting::Clicks,
        MouseEncoding::Default,
    );
    expect("\x1b[?1002h", MouseReporting::Drag, MouseEncoding::Default);
    expect(
        "\x1b[?1003h",
        MouseReporting::Motion,
        MouseEncoding::Default,
    );
    expect("\x1b[?1006h", MouseReporting::Motion, MouseEncoding::Sgr);
    expect("\x1b[?1005h", MouseReporting::Motion, MouseEncoding::Utf8);
    expect(
        "\x1b[?1006h\x1b[?1000h",
        MouseReporting::Clicks,
        MouseEncoding::Sgr,
    );
    expect(
        "\x1b[?1000l\x1b[?1006l",
        MouseReporting::Off,
        MouseEncoding::Default,
    );
    // X10 compatibility mode is not implemented; it changes nothing.
    expect("\x1b[?9h", MouseReporting::Off, MouseEncoding::Default);
}

/// Resetting any mouse tracking mode turns mouse reporting off, whichever
/// mode is on, as in tmux 3.7c (probed) and xterm.js; 1001, which nothing
/// implements, counts too. The encoding is left alone.
///
/// Why: alacritty clears only the mode named. A program that turned on
/// any-event tracking and resets click tracking on its way out would
/// otherwise leave reporting on, and the shell after it would receive mouse
/// reports as typed garbage. Spec: `MouseReporting`, `src/queries.rs`
/// ("Beyond queries").
#[test]
fn resetting_any_tracking_mode_ends_mouse_reporting() {
    for (set, reset) in [
        ("\x1b[?1003h", "\x1b[?1000l"),
        ("\x1b[?1002h", "\x1b[?1003l"),
        ("\x1b[?1000h", "\x1b[?1002l"),
        ("\x1b[?1000h", "\x1b[?1001l"),
    ] {
        let mut emulator = emulator(80, 24);
        feed(&mut emulator, format!("\x1b[?1006h{set}").as_bytes());
        feed(&mut emulator, reset.as_bytes());
        let modes = emulator.modes();
        assert_eq!(
            (modes.mouse_reporting, modes.mouse_encoding),
            (MouseReporting::Off, MouseEncoding::Sgr),
            "{set:?} then {reset:?}"
        );
    }
}

/// Modes 47 and 1047 show the alternate screen, as in tmux.
///
/// Why: alacritty ignores them, so a program using them would draw over the
/// shell's screen, and status detection would read the wrong screen.
/// Spec: `src/queries.rs` ("Beyond queries").
#[test]
fn modes_47_and_1047_show_the_alternate_screen() {
    for mode in ["47", "1047"] {
        let mut emulator = emulator(80, 24);
        feed(&mut emulator, format!("\x1b[?{mode}h").as_bytes());
        assert!(emulator.modes().alternate_screen, "{mode} set");
        feed(&mut emulator, format!("\x1b[?{mode}l").as_bytes());
        assert!(!emulator.modes().alternate_screen, "{mode} reset");
    }
}

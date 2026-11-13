//! Farhelm's screen-text format and titles, on hand-built screens.
//!
//! Agent status detection recognizes prompts and questions by their shape in
//! this text, so every expected value below is part of the format's contract
//! (`src/screen.rs`), on a terminal of the size given (20 by 6 unless named).
//! The values were first taken from tmux 3.7c's capture of the same output,
//! since the status readers were first written against tmux's captures (the
//! format's module docs say why it keeps that shape); they are pinned here as
//! Farhelm's own. The
//! supervisor's tests, where the status readers are, run every reader on
//! this emulator's repaint of the captured agent screens.

mod common;

use common::{emulator, feed};
use farhelm_terminal_emulator::trim_screen_tail;

/// The capture of `output` on a fresh 20x6 terminal.
fn capture(output: &str) -> String {
    capture_at(20, 6, output)
}

/// The capture of `output` on a fresh terminal of `columns` by `lines`.
fn capture_at(columns: u16, lines: u16, output: &str) -> String {
    let mut emulator = emulator(columns, lines);
    feed(&mut emulator, output.as_bytes());
    emulator.screen_text()
}

/// Compare captures of `(columns, lines, output, expected)` cases and report
/// every mismatch at once.
fn check_captures(cases: &[(u16, u16, &str, &str)]) {
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|&(columns, lines, output, expected)| {
            let got = capture_at(columns, lines, output);
            (got != expected).then(|| {
                format!("{output:?} at {columns}x{lines}: got {got:?}, expected {expected:?}")
            })
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Each screen reads exactly as the screen-text format says.
///
/// Why: the screen readers' rules depend on the format's details (a blank
/// draft row reading as nothing, a wrapped row as two lines), so any change
/// can turn a recognized prompt into an unrecognized one. Spec:
/// `src/screen.rs`, module docs.
#[test]
fn screens_read_in_the_screen_text_format() {
    let cases: &[(&str, &str, &str)] = &[
        (
            "tab over blank cells",
            "a\tb\r\n12345678\t9",
            "a\tb\n12345678\t9\n\n\n\n\n",
        ),
        (
            "tab over written cells",
            "abcdefghijk\rx\ty",
            "xbcdefghyjk\n\n\n\n\n\n",
        ),
        ("tab at a row's start", "a\tb\r\n\tc", "a\tb\n\tc\n\n\n\n\n"),
        (
            "tab in the last column",
            "abc\x1b[1;20H\t\r\n\x1b[2;20H\t\tz",
            "abc\n                   z\n\n\n\n\n",
        ),
        ("wide characters", "中文x\r\n😀y", "中文x\n😀y\n\n\n\n\n"),
        (
            "combining characters",
            "e\u{301}x\r\nn\u{303}\u{301}",
            "e\u{301}x\nn\u{303}\u{301}\n\n\n\n\n",
        ),
        (
            "variation selector",
            "\u{2764}\u{fe0f}z",
            "\u{2764}\u{fe0f}z\n\n\n\n\n\n",
        ),
        (
            "wrapped row",
            "abcdefghijklmnopqrstuvwxyz0123",
            "abcdefghijklmnopqrst\nuvwxyz0123\n\n\n\n\n",
        ),
        (
            "wide character at the margin",
            "abcdefghijklmnopqrs中z",
            "abcdefghijklmnopqrs\n中z\n\n\n\n\n",
        ),
        (
            "trailing spaces, written and colored",
            "abc   \r\ndef\x1b[41m   \x1b[0m\r\n   \r\nghi",
            "abc\ndef\n\nghi\n\n\n",
        ),
        ("blank rows", "top\r\n\r\n\r\nmid\r\n", "top\n\n\nmid\n\n\n"),
        (
            "erased characters",
            "abcdef\x1b[3G\x1b[2X",
            "ab  ef\n\n\n\n\n\n",
        ),
        (
            "alternate screen",
            "primary\r\n\x1b[?1049h\x1b[Halt one\r\nalt two",
            "alt one\nalt two\n\n\n\n\n",
        ),
        (
            "back from the alternate screen",
            "primary\r\n\x1b[?1049halt\x1b[?1049l",
            "primary\n\n\n\n\n\n",
        ),
        (
            "scrolled off the top",
            "1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8",
            "3\n4\n5\n6\n7\n8\n",
        ),
        (
            "wide character half overwritten",
            "ab中cd\r\x1b[2Cx",
            "abx cd\n\n\n\n\n\n",
        ),
    ];
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|(name, output, expected)| {
            let got = capture(output);
            (got != *expected).then(|| format!("{name}: got {got:?}, expected {expected:?}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The screen tail is the screen text with trailing whitespace trimmed and
/// cut to the last `max_bytes` on a character boundary, the same through
/// the emulator and through `trim_screen_tail` on its text.
///
/// Why: the sampler compares tails between samples to notice output, and
/// readers look for prompts at the bottom; the cut must be one rule
/// wherever a tail is taken, and never split a character. Spec:
/// `trim_screen_tail`.
#[test]
fn the_tail_trims_and_cuts_like_the_sampler() {
    let mut emulator = emulator(20, 6);
    feed(&mut emulator, "one\r\ntwo 中\r\n\r\n".as_bytes());
    assert_eq!(emulator.screen_tail(4096), "one\ntwo 中");
    // "中" is three bytes: a cut inside it moves forward past it.
    assert_eq!(emulator.screen_tail(4), " 中");
    assert_eq!(emulator.screen_tail(2), "");
    assert_eq!(emulator.screen_tail(0), "");
}

/// `trim_screen_tail` on text that did not come from an emulator: trailing
/// whitespace of any kind (spaces, tabs, blank lines) goes before the cut, a
/// cut that lands inside a character moves forward past it, and a budget
/// exactly the trimmed length keeps all of it.
///
/// Why: it is public so that callers holding screen text from elsewhere (a
/// host terminal's published view, captured fixture screens) cut it by the
/// sampler's rule; going through an emulator would only test the
/// delegation. Spec: `trim_screen_tail`.
#[test]
fn trim_screen_tail_cuts_any_screen_text_by_the_samplers_rule() {
    assert_eq!(trim_screen_tail("prompt> \t\n\n  \n", 4096), "prompt>");
    assert_eq!(trim_screen_tail("abc\ndef\n\n", 7), "abc\ndef");
    assert_eq!(trim_screen_tail("abc\ndef\n\n", 6), "bc\ndef");
    // "é" is two bytes: a cut after its first byte moves past it.
    assert_eq!(trim_screen_tail("xé€", 4), "€");
    assert_eq!(trim_screen_tail("xé€", 5), "é€");
    assert_eq!(trim_screen_tail(" \n\t\n", 4096), "");
}

/// Titles read as the program set them, with surrounding whitespace
/// trimmed, nothing before any is set, and nothing after a full reset.
///
/// Why: Codex's status is read from title prefixes (a spinner, "[ ! ]
/// Action Required"), so a title must reach the reader as the program set
/// it. Spec: `Emulator::title`.
#[test]
fn titles_read_as_the_program_set_them() {
    let title = |output: &str| {
        let mut emulator = emulator(20, 6);
        feed(&mut emulator, output.as_bytes());
        emulator.title().to_owned()
    };
    assert_eq!(title(""), "");
    assert_eq!(title("\x1b]2;a;b;c\x07"), "a;b;c");
    assert_eq!(title("\x1b]0;\u{280b} working\x07"), "\u{280b} working");
    assert_eq!(
        title("\x1b]2;[ ! ] Action Required\x1b\\"),
        "[ ! ] Action Required"
    );
    assert_eq!(title("\x1b]2;\x07"), "");
    assert_eq!(title("\x1b]2;  spaced title  \x07"), "spaced title");
    assert_eq!(
        title("\x1b]0;#(evil) x\x07"),
        "#(evil) x",
        "kept as set, `#(` included"
    );
    assert_eq!(title("\x1b]2;hello\x07\x1bc"), "", "RIS clears the title");
}

/// Tabs read as the format says: one tab character for a tab over blank
/// cells, spaces for a tab whose cells were written over since, nothing for
/// a tab sent from the last column, and a tab after a pending wrap starting
/// the next row.
///
/// Why: Codex's prompt rules look at a row's first characters and
/// indentation, where a stray tab or missing spaces changes the reading.
/// Spec: `src/screen.rs`, module docs.
#[test]
fn tabs_read_as_their_spans() {
    check_captures(&[
        (10, 3, "abcdefghi\t", "abcdefghi\n\n\n"),
        (10, 3, "abcdefghi\tX", "abcdefghiX\n\n\n"),
        (10, 3, "abcdefgh\tZ", "abcdefgh\tZ\n\n\n"),
        (10, 3, "abcdefghij\tX", "abcdefghij\nX\n\n"),
        (20, 3, "abcdefghi\t", "abcdefghi\t\n\n\n"),
        (20, 6, "a\tb\r\x1b[5GX", "a   X   b\n\n\n\n\n\n"),
    ]);
}

/// A tab whose span was partly erased, whose crossed tab stops changed, or
/// that was shifted by inserted characters reads by the span inferred from
/// the cells and tab stops as they are now, not the span it first covered.
///
/// Why: alacritty keeps no record of a tab's span, so the format infers it
/// (`src/screen.rs`, the paragraph on tabs). This pins what the emulator
/// does in exactly the cases where that inference is not what the program
/// drew, so a change is deliberate; only the tab-versus-spaces spelling is
/// at stake, never a visible character.
#[test]
fn tab_spans_after_edits_read_as_inferred() {
    // (output, its first row)
    let cases = [
        ("a\tb\r\x1b[5G\x1b[2X", "a\tb"),
        ("a\t\x1b[11Gb\r\x1b[3G\x1bH", "a\t        b"),
        ("a\tb\r\x1b[2@", "  a\t  b"),
    ];
    for (output, first_row) in cases {
        assert_eq!(
            capture(output),
            format!("{first_row}\n\n\n\n\n\n"),
            "{output:?}"
        );
    }
}

/// Modes 47 and 1047 capture the alternate screen, and leaving keeps the
/// cursor where the program left it and the saved cursor where it was;
/// leaving with 1049 after entering with 47 puts the cursor back where it
/// was on the primary screen.
///
/// Why: status detection reads whichever screen shows, and a program using
/// these older modes must not have its text land on the shell's screen.
/// Spec: `src/queries.rs` ("Beyond queries").
#[test]
fn modes_47_and_1047_read_the_alternate_screen() {
    check_captures(&[
        (10, 3, "primary\r\n\x1b[?47hALT", "\nALT\n\n"),
        (
            10,
            3,
            "primary\x1b[?47h\r\n\r\nALT\x1b[?47lX",
            "primary\n\n   X\n",
        ),
        (
            10,
            3,
            "primary\x1b[?1047h\r\n\r\nALT\x1b[?1047lX",
            "primary\n\n   X\n",
        ),
        (10, 3, "ab\x1b7\r\n\x1b[?47hz\x1b[?47l\x1b8X", "abX\n\n\n"),
        (
            10,
            3,
            "\x1b[?47hOLD\x1b[?47l\x1b[?47h\x1b[2;1HNEW",
            "\nNEW\n\n",
        ),
    ]);

    // Entering with 47 and leaving with 1049: alacritty keeps a cursor per
    // screen, so the primary screen's cursor is where it was before the
    // switch. (output, its first row)
    let mixed = [
        ("p\x1b[?47hA\x1b[?1049hB\x1b[?1049lC", "pC"),
        ("p\x1b[?47hA\x1b[?1049lC", "pC"),
    ];
    for (output, first_row) in mixed {
        assert_eq!(
            capture_at(10, 3, output),
            format!("{first_row}\n\n\n"),
            "{output:?}"
        );
    }
}

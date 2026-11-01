//! Screen text and title shaped like tmux's capture, on hand-built screens.
//!
//! Agent status detection recognizes prompts and questions in the text tmux
//! captures, so the emulator's text has to come out the same shape. Every
//! expected value below is what `tmux capture-pane -p` (tmux 3.7c, a pane
//! of the size given, 20 by 6 unless named) printed for the same output, with
//! the title as `#{pane_title}` returned it; where the emulator knowingly
//! differs, the test names both. The comparison against a live tmux, on
//! generated output and recorded agent sessions, is the supervisor's
//! `emulator_parity`, where the status readers are.

mod common;

use common::{emulator, feed};

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

/// Each screen captures exactly as tmux 3.7c captured it.
///
/// Why: the screen readers' rules are written against tmux's shape (a blank
/// draft row capturing as nothing, a wrapped row as two lines), so any
/// difference can turn a recognized prompt into an unrecognized one. Spec:
/// `src/screen.rs`, module docs.
#[test]
fn screens_capture_like_tmux() {
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
            (got != *expected).then(|| format!("{name}: got {got:?}, tmux {expected:?}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The screen tail is the capture with trailing whitespace trimmed and cut
/// to the last `max_bytes` on a character boundary, as the status sampler
/// trims tmux's.
///
/// Why: the sampler compares tails between samples to notice output; the
/// cut must be on the same rule, and never split a character.
/// Spec: `retain_pane_tail` in the supervisor's tmux driver.
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

/// Titles come out as tmux reports them, except for the two documented
/// differences: before any title is set the emulator reports nothing where
/// tmux reports the host name, and surrounding whitespace is trimmed.
///
/// Why: Codex's status is read from title prefixes (a spinner, "[ ! ]
/// Action Required"), so a title must reach the reader as the program set
/// it. Spec: `Emulator::title`.
#[test]
fn titles_read_like_pane_title() {
    let title = |output: &str| {
        let mut emulator = emulator(20, 6);
        feed(&mut emulator, output.as_bytes());
        emulator.title().to_owned()
    };
    assert_eq!(title(""), "", "tmux: the host name");
    assert_eq!(title("\x1b]2;a;b;c\x07"), "a;b;c");
    assert_eq!(title("\x1b]0;\u{280b} working\x07"), "\u{280b} working");
    assert_eq!(
        title("\x1b]2;[ ! ] Action Required\x1b\\"),
        "[ ! ] Action Required"
    );
    assert_eq!(title("\x1b]2;\x07"), "");
    assert_eq!(
        title("\x1b]2;  spaced title  \x07"),
        "spaced title",
        "tmux keeps the spaces"
    );
    assert_eq!(
        title("\x1b]0;#(evil) x\x07"),
        "#(evil) x",
        "tmux: _(evil) x"
    );
    assert_eq!(
        title("\x1b]2;hello\x07\x1bc"),
        "",
        "tmux keeps the title on RIS"
    );
}

/// Tabs capture as tmux captures them: one tab character for a tab over
/// blank cells, spaces for a tab whose cells were written over since,
/// nothing for a tab sent from the last column, and a tab after a pending
/// wrap starting the next row.
///
/// Why: Codex's prompt rules look at a row's first characters and
/// indentation, where a stray tab or missing spaces changes the reading.
/// Spec: `src/screen.rs`, module docs.
#[test]
fn tabs_capture_like_tmux() {
    check_captures(&[
        (10, 3, "abcdefghi\t", "abcdefghi\n\n\n"),
        (10, 3, "abcdefghi\tX", "abcdefghiX\n\n\n"),
        (10, 3, "abcdefgh\tZ", "abcdefgh\tZ\n\n\n"),
        (10, 3, "abcdefghij\tX", "abcdefghij\nX\n\n"),
        (20, 3, "abcdefghi\t", "abcdefghi\t\n\n\n"),
        (20, 6, "a\tb\r\x1b[5GX", "a   X   b\n\n\n\n\n\n"),
    ]);
}

/// Known differences from tmux: a tab whose span was partly erased, whose
/// crossed tab stops changed, or that was shifted by inserted characters.
///
/// Why: tmux tracks each tab's span through edits; the emulator infers it
/// from the cells and the current tab stops (`src/screen.rs`). This pins
/// what the emulator does in exactly the cases where that inference fails,
/// so a change is deliberate; the visible cell positions agree with tmux,
/// only the tab-versus-spaces spelling differs.
#[test]
fn known_tab_differences_from_tmux() {
    // (output, this emulator's first row, tmux's first row)
    let cases = [
        ("a\tb\r\x1b[5G\x1b[2X", "a\tb", "a\t  b"),
        ("a\t\x1b[11Gb\r\x1b[3G\x1bH", "a\t        b", "a\t  b"),
        ("a\tb\r\x1b[2@", "  a\t  b", "  a\tb"),
    ];
    for (output, ours, tmux) in cases {
        assert_eq!(
            capture(output),
            format!("{ours}\n\n\n\n\n\n"),
            "{output:?} (tmux: {tmux:?})"
        );
    }
}

/// Modes 47 and 1047 capture the alternate screen, and leaving keeps the
/// cursor where the program left it and the saved cursor where it was, as
/// in tmux; mixing them with 1049 knowingly differs.
///
/// Why: status detection reads whichever screen shows, and a program using
/// these older modes must not have its text land on the shell's screen.
/// Spec: `src/queries.rs` ("Beyond queries").
#[test]
fn modes_47_and_1047_capture_like_tmux() {
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

    // Known differences: entering with 47 and leaving with 1049. alacritty
    // keeps a cursor per screen, tmux one cursor.
    // (output, this emulator's first row, tmux's first row)
    let mixed = [
        ("p\x1b[?47hA\x1b[?1049hB\x1b[?1049lC", "pC", "p  C"),
        ("p\x1b[?47hA\x1b[?1049lC", "pC", "p C"),
    ];
    for (output, ours, tmux) in mixed {
        assert_eq!(
            capture_at(10, 3, output),
            format!("{ours}\n\n\n"),
            "{output:?} (tmux: {tmux:?})"
        );
    }
}

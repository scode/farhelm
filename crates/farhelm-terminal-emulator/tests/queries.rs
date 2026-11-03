//! The query-answer table, pinned through the public API.
//!
//! Each row names what tmux 3.7c answered when probed under Farhelm's server
//! configuration (every row's `tmux` value comes from a probe, the same
//! bytes written in one write), beside what this emulator answers, so that a
//! row where the two differ is visibly deliberate (`src/queries.rs` gives
//! each reason).
//! The exhaustive replay against a live tmux is a separate test that follows
//! this crate; these tests are the table itself.

mod common;

use common::{answer, answer_from, emulator, feed, resize};

/// One row: a query, this emulator's answer, and tmux 3.7c's.
struct Row {
    query: &'static str,
    ours: &'static str,
    tmux: &'static str,
}

const fn row(query: &'static str, ours: &'static str, tmux: &'static str) -> Row {
    Row { query, ours, tmux }
}

/// Check every row on a fresh 80x24 emulator, and report all mismatches at
/// once.
fn check(rows: &[Row]) {
    let failures: Vec<String> = rows
        .iter()
        .filter_map(|row| {
            let got = answer(row.query);
            (got != row.ours).then(|| {
                format!(
                    "{:?}: answered {:?}, table says {:?} (tmux said {:?})",
                    row.query, got, row.ours, row.tmux
                )
            })
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Device attributes, device status, version and window queries answer as
/// the table says.
///
/// Why: programs decide what the terminal can do from these answers, and
/// they have been tested against tmux's. DA1 in particular must not be
/// alacritty's VT102 answer (`CSI ? 6 c`), but what tmux and the browser
/// terminal both say. Spec: `src/queries.rs`, the table.
#[test]
fn identification_status_and_window_queries() {
    check(&[
        row("\x1b[c", "\x1b[?1;2c", "\x1b[?1;2c"),
        row("\x1b[0c", "\x1b[?1;2c", "\x1b[?1;2c"),
        // vte reports DECID as DA1; tmux ignores it.
        row("\x1bZ", "\x1b[?1;2c", ""),
        // alacritty's own DA2 rather than tmux's claim to be tmux.
        row("\x1b[>c", "\x1b[>0;2600;1c", "\x1b[>84;0;0c"),
        row("\x1b[=c", "", ""),
        // vte does not parse XTVERSION.
        row("\x1b[>q", "", "\x1bP>|tmux 3.7c\x1b\\"),
        row("\x1b[5n", "\x1b[0n", "\x1b[0n"),
        row("\x1b[3;5H\x1b[6n", "\x1b[3;5R", "\x1b[3;5R"),
        row("\x1b[?6n", "", ""),
        // vte does not parse the color scheme query.
        row("\x1b[?996n", "", "\x1b[?997;1n"),
        row("\x1b[11t", "", ""),
        row("\x1b[13t", "", ""),
        // tmux's default cell size of 16x32 pixels, at 80x24.
        row("\x1b[14t", "\x1b[4;768;1280t", "\x1b[4;768;1280t"),
        row("\x1b[14;2t", "\x1b[4;768;1280t", "\x1b[4;768;1280t"),
        // vte parses only 14, 18, 22 and 23 of the window operations.
        row("\x1b[15t", "", "\x1b[5;768;1280t"),
        row("\x1b[16t", "", "\x1b[6;32;16t"),
        row("\x1b[18t", "\x1b[8;24;80t", "\x1b[8;24;80t"),
        row("\x1b[19t", "", "\x1b[9;24;80t"),
        row("\x1b[21t", "", ""),
        // Kitty keyboard mode is off, so its query goes unanswered.
        row("\x1b[?u", "", ""),
        row("\x1b[?4m", "", ""),
        row("\x05", "", ""),
        // vte ignores every DCS: DECRQSS and XTGETTCAP.
        row("\x1bP$qm\x1b\\", "", "\x1bP0$r\x1b\\"),
        row("\x1bP$q q\x1b\\", "", "\x1bP1$r q0 q\x1b\\"),
        row("\x1bP+q524742\x1b\\", "", ""),
    ]);
}

/// Color queries answer with Farhelm's theme, in tmux's format, with the
/// query's own terminator; clipboard reads are never answered.
///
/// Why: agents pick light or dark output from the background color, so the
/// answer must be what the user sees. OSC 12 and OSC 4 are answered today by
/// the browser terminal (tmux passes them through), from the same theme.
/// Spec: `src/queries.rs`, the table; `src/theme.rs`.
#[test]
fn color_and_clipboard_queries() {
    check(&[
        row(
            "\x1b]10;?\x07",
            "\x1b]10;rgb:ffff/ffff/ffff\x07",
            "\x1b]10;rgb:ffff/ffff/ffff\x07",
        ),
        row(
            "\x1b]10;?\x1b\\",
            "\x1b]10;rgb:ffff/ffff/ffff\x1b\\",
            "\x1b]10;rgb:ffff/ffff/ffff\x1b\\",
        ),
        row(
            "\x1b]11;?\x07",
            "\x1b]11;rgb:2828/2c2c/3434\x07",
            "\x1b]11;rgb:2828/2c2c/3434\x07",
        ),
        row(
            "\x1b]11;?\x1b\\",
            "\x1b]11;rgb:2828/2c2c/3434\x1b\\",
            "\x1b]11;rgb:2828/2c2c/3434\x1b\\",
        ),
        row("\x1b]12;?\x07", "\x1b]12;rgb:ffff/ffff/ffff\x07", ""),
        // Two dynamic colors in one query, as xterm allows.
        row(
            "\x1b]10;?;?\x07",
            "\x1b]10;rgb:ffff/ffff/ffff\x07\x1b]11;rgb:2828/2c2c/3434\x07",
            "",
        ),
        row("\x1b]4;1;?\x07", "\x1b]4;1;rgb:cccc/6666/6666\x07", ""),
        // 200 is cube entry 184: red level 5, green 0, blue 4.
        row(
            "\x1b]4;200;?\x1b\\",
            "\x1b]4;200;rgb:ffff/0000/d7d7\x1b\\",
            "",
        ),
        row("\x1b]52;c;?\x07", "", ""),
    ]);
}

/// A color the program set is the one its query reports, and a query sent
/// before a change in the same write reports the color before the change.
///
/// Why: tmux reports the pane's own colors once a program sets them, and
/// answers each query when it is parsed. Answering only after the whole
/// write (as alacritty's `ColorRequest` event would) gets the second case
/// wrong. Spec: `QueryHandler::dynamic_color_sequence`.
#[test]
fn color_queries_report_program_colors_at_the_time_of_asking() {
    let mut emulator = emulator(80, 24);
    assert_eq!(
        answer_from(
            &mut emulator,
            "\x1b]11;?\x07\x1b]11;#102030\x07\x1b]11;?\x07"
        ),
        "\x1b]11;rgb:2828/2c2c/3434\x07\x1b]11;rgb:1010/2020/3030\x07"
    );
    assert_eq!(
        answer_from(&mut emulator, "\x1b]4;3;rgb:01/02/03\x07\x1b]4;3;?\x07"),
        "\x1b]4;3;rgb:0101/0202/0303\x07"
    );
    assert_eq!(
        answer_from(&mut emulator, "\x1b]111\x07\x1b]11;?\x07"),
        "\x1b]11;rgb:2828/2c2c/3434\x07",
        "resetting the background goes back to the theme"
    );
}

/// DECRQM on a fresh terminal answers each mode as the table says.
///
/// Why: programs probe support for features (synchronized output, bracketed
/// paste, mouse encodings) this way, and "not recognized" (0) versus "reset"
/// (2) changes what they do. Spec: `src/queries.rs`, "DECRQM".
#[test]
fn mode_reports_on_a_fresh_terminal() {
    check(&[
        row("\x1b[4$p", "\x1b[4;2$y", "\x1b[4;2$y"),
        row("\x1b[2$p", "\x1b[2;0$y", "\x1b[2;0$y"),
        row("\x1b[12$p", "\x1b[12;0$y", "\x1b[12;0$y"),
        // alacritty implements newline mode; tmux does not.
        row("\x1b[20$p", "\x1b[20;2$y", "\x1b[20;0$y"),
        row("\x1b[34$p", "\x1b[34;0$y", "\x1b[34;0$y"),
        row("\x1b[$p", "", ""),
        row("\x1b[?1$p", "\x1b[?1;2$y", "\x1b[?1;2$y"),
        row("\x1b[?3$p", "\x1b[?3;4$y", "\x1b[?3;4$y"),
        row("\x1b[?5$p", "\x1b[?5;0$y", "\x1b[?5;0$y"),
        row("\x1b[?6$p", "\x1b[?6;2$y", "\x1b[?6;2$y"),
        row("\x1b[?7$p", "\x1b[?7;1$y", "\x1b[?7;1$y"),
        row("\x1b[?8$p", "\x1b[?8;0$y", "\x1b[?8;0$y"),
        row("\x1b[?9$p", "\x1b[?9;0$y", "\x1b[?9;0$y"),
        row("\x1b[?12$p", "\x1b[?12;2$y", "\x1b[?12;2$y"),
        row("\x1b[?25$p", "\x1b[?25;1$y", "\x1b[?25;1$y"),
        row("\x1b[?45$p", "\x1b[?45;0$y", "\x1b[?45;0$y"),
        row("\x1b[?47$p", "\x1b[?47;2$y", "\x1b[?47;2$y"),
        row("\x1b[?66$p", "\x1b[?66;0$y", "\x1b[?66;0$y"),
        row("\x1b[?67$p", "\x1b[?67;0$y", "\x1b[?67;0$y"),
        row("\x1b[?69$p", "\x1b[?69;0$y", "\x1b[?69;0$y"),
        row("\x1b[?1000$p", "\x1b[?1000;2$y", "\x1b[?1000;2$y"),
        row("\x1b[?1001$p", "\x1b[?1001;0$y", "\x1b[?1001;0$y"),
        row("\x1b[?1002$p", "\x1b[?1002;2$y", "\x1b[?1002;2$y"),
        row("\x1b[?1003$p", "\x1b[?1003;2$y", "\x1b[?1003;2$y"),
        row("\x1b[?1004$p", "\x1b[?1004;2$y", "\x1b[?1004;2$y"),
        row("\x1b[?1005$p", "\x1b[?1005;2$y", "\x1b[?1005;2$y"),
        row("\x1b[?1006$p", "\x1b[?1006;2$y", "\x1b[?1006;2$y"),
        // alacritty records these, but nothing in Farhelm acts on them.
        row("\x1b[?1007$p", "\x1b[?1007;0$y", "\x1b[?1007;0$y"),
        row("\x1b[?1015$p", "\x1b[?1015;0$y", "\x1b[?1015;0$y"),
        row("\x1b[?1016$p", "\x1b[?1016;0$y", "\x1b[?1016;0$y"),
        row("\x1b[?1036$p", "\x1b[?1036;0$y", "\x1b[?1036;0$y"),
        row("\x1b[?1042$p", "\x1b[?1042;0$y", "\x1b[?1042;0$y"),
        row("\x1b[?1047$p", "\x1b[?1047;2$y", "\x1b[?1047;2$y"),
        row("\x1b[?1048$p", "\x1b[?1048;0$y", "\x1b[?1048;0$y"),
        row("\x1b[?1049$p", "\x1b[?1049;2$y", "\x1b[?1049;2$y"),
        row("\x1b[?2004$p", "\x1b[?2004;2$y", "\x1b[?2004;2$y"),
        row("\x1b[?2026$p", "\x1b[?2026;2$y", "\x1b[?2026;2$y"),
        row("\x1b[?2027$p", "\x1b[?2027;0$y", "\x1b[?2027;0$y"),
        // This emulator sends no color scheme notifications.
        row("\x1b[?2031$p", "\x1b[?2031;0$y", "\x1b[?2031;2$y"),
        row("\x1b[?2048$p", "\x1b[?2048;0$y", "\x1b[?2048;0$y"),
        row("\x1b[?$p", "", ""),
    ]);
}

/// DECRQM follows the program's mode changes, including an open
/// synchronized update, which only a mode report from inside the update can
/// see.
///
/// Why: a report that never says "set" for a mode the program just set
/// would tell it the mode does not stick. For 2026, alacritty alone would say
/// reset even inside an update; tmux says set. Spec: `src/queries.rs`,
/// "DECRQM".
#[test]
fn mode_reports_follow_mode_changes() {
    check(&[
        row(
            "\x1b[?2026h\x1b[?2026$p\x1b[?2026l",
            "\x1b[?2026;1$y",
            "\x1b[?2026;1$y",
        ),
        row(
            "\x1b[?2004h\x1b[?2004$p\x1b[?2004l",
            "\x1b[?2004;1$y",
            "\x1b[?2004;1$y",
        ),
        row("\x1b[1 q\x1b[?12$p\x1b[0 q", "\x1b[?12;1$y", "\x1b[?12;1$y"),
        row(
            "\x1b[?1003h\x1b[?1003$p\x1b[?1002$p\x1b[?1000$p\x1b[?1003l",
            "\x1b[?1003;1$y\x1b[?1002;2$y\x1b[?1000;2$y",
            "\x1b[?1003;1$y\x1b[?1002;2$y\x1b[?1000;2$y",
        ),
        row(
            "\x1b[?1049h\x1b[?1049$p\x1b[?1049l",
            "\x1b[?1049;1$y",
            "\x1b[?1049;1$y",
        ),
        row("\x1b[4h\x1b[4$p\x1b[4l", "\x1b[4;1$y", "\x1b[4;1$y"),
        // Modes 47 and 1047 are the alternate screen, as in tmux.
        row(
            "\x1b[?47h\x1b[?47$p\x1b[?1047$p\x1b[?1049$p\x1b[?47l",
            "\x1b[?47;1$y\x1b[?1047;1$y\x1b[?1049;1$y",
            "\x1b[?47;1$y\x1b[?1047;1$y\x1b[?1049;1$y",
        ),
        row(
            "\x1b[?1047h\x1b[?47$p\x1b[?1047$p\x1b[?1049$p\x1b[?1047l\x1b[?1047$p",
            "\x1b[?47;1$y\x1b[?1047;1$y\x1b[?1049;1$y\x1b[?1047;2$y",
            "\x1b[?47;1$y\x1b[?1047;1$y\x1b[?1049;1$y\x1b[?1047;2$y",
        ),
        // Resetting any tracking mode ends mouse reporting.
        row(
            "\x1b[?1003h\x1b[?1000l\x1b[?1003$p\x1b[?1000$p",
            "\x1b[?1003;2$y\x1b[?1000;2$y",
            "\x1b[?1003;2$y\x1b[?1000;2$y",
        ),
    ]);

    // The update has ended by the time its report is read, and a later
    // report says so.
    let mut emulator = emulator(80, 24);
    assert_eq!(
        answer_from(
            &mut emulator,
            "\x1b[?2026h\x1b[?2026$p\x1b[?2026l\x1b[?2026$p"
        ),
        "\x1b[?2026;1$y\x1b[?2026;2$y"
    );
}

/// Answers come back in the order the program asked, whether alacritty or
/// this crate produced them.
///
/// Why: a program that sends several queries in one write matches answers
/// to questions by order (a DA1 sent last as a sentinel, most commonly). DA1
/// is answered by this crate and the cursor position by alacritty, through
/// one shared queue. Spec: `src/effects.rs`, module docs.
#[test]
fn answers_keep_the_order_of_the_questions() {
    assert_eq!(
        answer("\x1b[6n\x1b[c\x1b[5n\x1b]11;?\x07\x1b[18t"),
        "\x1b[1;1R\x1b[?1;2c\x1b[0n\x1b]11;rgb:2828/2c2c/3434\x07\x1b[8;24;80t"
    );
}

/// Size queries follow a resize.
///
/// Why: a program that asks after the viewer resized the terminal must get
/// the new size, in cells and in (assumed) pixels.
#[test]
fn size_queries_follow_resizes() {
    let mut emulator = emulator(80, 24);
    resize(&mut emulator, 100, 30);
    assert_eq!(
        answer_from(&mut emulator, "\x1b[18t\x1b[14t"),
        "\x1b[8;30;100t\x1b[4;960;1600t"
    );
}

/// The kitty keyboard protocol stays off even when a program asks for it:
/// the push is ignored, the query unanswered, and no keyboard mode recorded.
///
/// Why: today's browser terminal has no kitty support, so agents send and
/// expect legacy key encoding; turning kitty on would change how Shift+Enter
/// and Escape reach them. Spec: `Emulator::new`, `src/modes.rs`.
#[test]
fn kitty_keyboard_requests_are_ignored() {
    let mut emulator = emulator(80, 24);
    let effects = feed(&mut emulator, b"\x1b[>1u\x1b[=3;1u\x1b[?u");
    assert!(effects.replies.is_empty(), "{:?}", effects.replies);
    let (active, inactive) = emulator.term().keyboard_mode_stacks();
    assert!(active.is_empty() && inactive.is_empty());
}

/// The open-update flag behind DECRQM 2026 survives between feeds: an update
/// opened in one feed and queried in the next reports set.
///
/// Why: the flag lives in the emulator rather than in one call precisely
/// because vte replays an update's bytes, queries included, in the call that
/// ends it; a flag reset per call would answer reset here. Spec:
/// `QueryHandler::sync_open` in `src/queries.rs`.
#[test]
fn the_open_update_flag_survives_between_feeds() {
    let mut emulator = emulator(80, 24);
    assert_eq!(feed(&mut emulator, b"\x1b[?2026h").replies, b"");
    assert_eq!(feed(&mut emulator, b"\x1b[?2026$p").replies, b"");
    assert_eq!(
        feed(&mut emulator, b"\x1b[?2026l\x1b[?2026$p").replies,
        b"\x1b[?2026;1$y\x1b[?2026;2$y"
    );
}

/// Known difference from tmux: a query inside an open update is answered
/// when the update ends (or is flushed at its deadline), not at once.
///
/// Why: tmux answers at once (probed: a cursor position query after an
/// update start, in its own write, got its answer immediately). This pins
/// the emulator's behavior so that a change to it is deliberate; the tmux
/// comparison on recorded agent sessions checks that agents do not wait on
/// such a query. Spec: `src/queries.rs`, the paragraph on synchronized
/// updates.
#[test]
fn a_query_inside_an_open_update_is_answered_when_it_ends() {
    let mut emulator = emulator(80, 24);
    let asked = feed(&mut emulator, b"\x1b[?2026h\x1b[3;4H\x1b[6n");
    assert_eq!(asked.replies, b"", "tmux answers \\x1b[3;4R here");
    assert_eq!(feed(&mut emulator, b"\x1b[?2026l").replies, b"\x1b[3;4R");
}

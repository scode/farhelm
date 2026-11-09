//! Tests of the grid output encoding: cells to runs (styles, colors, wide
//! and combining characters, links), the screen diff and its shift, history
//! numbering across appends, eviction and relayouts, message packing within
//! the format's bounds, the state message, and clipboard parts. Each feeds
//! an emulator directly; the sender's own tests drive these through a host
//! terminal.

use super::test_client::{TestClient, held_history, row_text};
use super::*;
use farhelm_proto::grid::{GRID_MAX_LINK_URI_BYTES, GridStreamCheck};
use farhelm_terminal_emulator::{Collected, FeedMode, Size};

// ---- Helpers ----------------------------------------------------------------

/// An emulator of `cols` by `rows` keeping `scrollback` lines of history,
/// fed `output` live.
fn emulator(cols: u16, rows: u16, scrollback: usize, output: &[u8]) -> Emulator {
    let mut emulator = Emulator::new(
        Size {
            columns: cols,
            lines: rows,
        },
        scrollback,
    );
    feed(&mut emulator, output);
    emulator
}

fn feed(emulator: &mut Emulator, output: &[u8]) {
    emulator.feed(
        output,
        std::time::Instant::now(),
        FeedMode::Live,
        &mut Collected::default(),
    );
}

/// `count` numbered lines, `line 0000` and on, each ended with CR LF.
fn numbered(from: usize, count: usize) -> Vec<u8> {
    (from..from + count)
        .flat_map(|n| format!("line {n:04}\r\n").into_bytes())
        .collect()
}

/// Encode every message, as a sender does before sending it, and apply it
/// to `client`: each must be one a client accepts.
fn deliver(client: &mut TestClient, messages: &[GridOutput]) {
    for message in messages {
        let bytes = message.encode().expect("the sender's messages encode");
        client.apply(&bytes);
    }
}

/// A client that has had `hello` and `state` for a screen of `cols` by
/// `rows`.
fn started_client(cols: u16, rows: u16) -> TestClient {
    let mut client = TestClient::new();
    deliver(
        &mut client,
        &[
            GridOutput::Hello {
                cols,
                rows,
                history_cap: 1000,
            },
            GridOutput::State(TerminalState {
                mouse: MouseMode::Off,
                focus: false,
                alt_screen: false,
                default_fg: None,
                default_bg: None,
                history_trimmed: false,
                exit: None,
            }),
        ],
    );
    client
}

fn plain(fg: Color) -> Style {
    Style { fg, ..Style::PLAIN }
}

// ---- Cells to runs ----------------------------------------------------------

/// Cells of one style share a run, a style change starts another, and blank
/// cells in the default style are not sent.
///
/// Why: runs are what keeps a row small on the wire, and the format reads a
/// column no run covers as blank, so sending default blanks costs bytes for
/// nothing. Spec: `ab` in bold red, a space, `cd` in the default style and
/// trailing blanks become exactly two runs, `ab` at column 0 with bold and
/// palette color 1, and `cd` at column 3 in the default style; the row does
/// not wrap.
#[test]
fn cells_of_one_style_share_a_run_and_default_blanks_are_left_out() {
    let emulator = emulator(20, 3, 100, b"\x1b[1;31mab\x1b[m cd   ");
    let row = &screen(&emulator).lines[0];
    assert_eq!(
        row,
        &CellRow {
            wraps: false,
            runs: vec![
                CellRun {
                    col: 0,
                    width: 2,
                    text: "ab".into(),
                    style: Style {
                        flags: style_flags::BOLD,
                        fg: Color::Palette(1),
                        ..Style::PLAIN
                    },
                    link: None,
                },
                CellRun {
                    col: 3,
                    width: 2,
                    text: "cd".into(),
                    style: Style::PLAIN,
                    link: None,
                },
            ],
        }
    );
}

/// A space with a background color is sent; the same space in the default
/// style is not.
///
/// Why: a colored bar of spaces (a status line, a selection highlight) is
/// visible content, and dropping it as "blank" would draw it in the
/// default background. Spec: three spaces on a blue background become one
/// run of three spaces with background palette 4.
#[test]
fn spaces_with_a_background_are_content() {
    let emulator = emulator(20, 3, 100, b"\x1b[44m   \x1b[m");
    assert_eq!(
        screen(&emulator).lines[0].runs,
        vec![CellRun {
            col: 0,
            width: 3,
            text: "   ".into(),
            style: Style {
                bg: Color::Palette(4),
                ..Style::PLAIN
            },
            link: None,
        }]
    );
}

/// A wide character, a character with a combining mark, and any other
/// non-ASCII character are each a run of one cell's content.
///
/// Why: only the emulator knows how characters group into cells, so the
/// format asks the client to lay out ASCII alone; a wide character's spacer
/// cell must not be sent, or the client would draw something in it. Spec:
/// `a界e\u{301}b` becomes `a` at 0, `界` at 1 with width 2, `e\u{301}` at 3
/// with width 1, and `b` at 4.
#[test]
fn non_ascii_cells_are_runs_of_their_own() {
    let emulator = emulator(20, 3, 100, "a界e\u{301}b".as_bytes());
    let screen = screen(&emulator);
    let runs: Vec<(u16, u16, &str)> = screen.lines[0]
        .runs
        .iter()
        .map(|run| (run.col, run.width, run.text.as_str()))
        .collect();
    assert_eq!(
        runs,
        vec![(0, 1, "a"), (1, 2, "界"), (3, 1, "e\u{301}"), (4, 1, "b")]
    );
}

/// A row that wrapped says so, and a tab reads as blank.
///
/// Why: the client joins wrapped rows without a newline when it copies a
/// selection; and alacritty stores a tab's first cell as a tab character,
/// which is not printable ASCII and would otherwise become a run of its
/// own. Spec: 25 characters on a 20-column screen wrap the first row only;
/// `a\tb` reads `a`, seven blanks and `b`, as one run with no tab in it.
#[test]
fn wraps_are_marked_and_tabs_are_blank() {
    let emulator = emulator(20, 4, 100, b"0123456789abcdefghijKLMNO\r\na\tb");
    let screen = screen(&emulator);
    assert!(screen.lines[0].wraps);
    assert!(!screen.lines[1].wraps);
    assert_eq!(row_text(&screen.lines[2]), "a       b");
    assert_eq!(screen.lines[2].runs.len(), 1);
}

/// A palette entry the program redefined is sent as the color it now is,
/// and the default colors it redefined travel in `state`.
///
/// Why: the client maps palette indices through its own theme and never
/// learns the program's palette, so an index the program changed would
/// draw in the theme's color instead of the program's. Spec: after OSC 4
/// sets entry 1 to #ff8000, text in color 1 has foreground #ff8000 as a
/// direct color, text in color 2 keeps palette 2, and a 256-color index
/// stays an index; after OSC 10 and 11, `state` carries the new default
/// foreground and background.
#[test]
fn redefined_colors_travel_as_direct_colors() {
    let emulator = emulator(
        20,
        3,
        100,
        b"\x1b]4;1;rgb:ff/80/00\x1b\\\x1b[31ma\x1b[32mb\x1b[38;5;200mc\x1b[m\
          \x1b]10;rgb:11/22/33\x1b\\\x1b]11;rgb:44/55/66\x1b\\",
    );
    let fgs: Vec<Color> = screen(&emulator).lines[0]
        .runs
        .iter()
        .map(|run| run.style.fg)
        .collect();
    assert_eq!(
        fgs,
        vec![
            Color::Rgb {
                r: 0xff,
                g: 0x80,
                b: 0
            },
            Color::Palette(2),
            Color::Palette(200),
        ]
    );
    let state = terminal_state(&emulator);
    assert_eq!(
        state.default_fg,
        Some(Color::Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33
        })
    );
    assert_eq!(
        state.default_bg,
        Some(Color::Rgb {
            r: 0x44,
            g: 0x55,
            b: 0x66
        })
    );
}

/// An OSC 8 link reaches the run with its id and target, and runs of one
/// link share one entry of the message's link table.
///
/// Why: the hover and the mismatch warning compare a link's target with
/// its text across rows of one link, which needs every run of it to name
/// the same entry. Spec: a link with id `x` over `abc` on one row and `de`
/// on the next is one table entry `[x, https://example.com/]` that both
/// rows' runs point at.
#[test]
fn links_share_one_table_entry() {
    let emulator = emulator(
        20,
        3,
        100,
        b"\x1b]8;id=x;https://example.com/\x1b\\abc\r\nde\x1b]8;;\x1b\\",
    );
    let screen = screen(&emulator);
    let rows: Vec<(u16, &CellRow)> = vec![(0, &screen.lines[0]), (1, &screen.lines[1])];
    let messages = rows_messages(&rows);
    assert_eq!(messages.len(), 1);
    let GridOutput::Rows { rows, links } = &messages[0] else {
        panic!("a rows message, got {messages:?}");
    };
    assert_eq!(
        links,
        &vec![Link {
            id: "x".into(),
            uri: "https://example.com/".into()
        }]
    );
    assert!(
        rows.iter()
            .all(|row| row.row.runs.iter().all(|run| run.link == Some(0)))
    );
}

/// Anonymous OSC restarts keep distinct numeric emulator ids on the wire.
///
/// The grid client's continuation rule recognizes this spelling because
/// the protocol has no explicit/generated flag. Pin the literal user's
/// reproduction here so a vendor upgrade cannot silently defeat that rule.
#[test]
fn anonymous_link_restarts_carry_generated_ids() {
    let emulator = emulator(
        30,
        2,
        100,
        b"\x1b]8;;https://apnews.com/x\x1b\\https://apn\x1b]8;;https://apnews.com/x\x1b\\ews.com/x\x1b]8;;\x1b\\\n",
    );
    let screen = screen(&emulator);
    let messages = rows_messages(&[(0, &screen.lines[0])]);
    let GridOutput::Rows { rows, links } = &messages[0] else {
        panic!("a rows message, got {messages:?}");
    };
    assert_eq!(links.len(), 2);
    assert_ne!(links[0].id, links[1].id);
    for link in links {
        let digits = link
            .id
            .strip_suffix("_alacritty")
            .expect("generated id suffix");
        assert!(!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()));
        assert_eq!(link.uri, "https://apnews.com/x");
    }
    assert_eq!(rows[0].row.runs.len(), 2);
    assert_eq!(rows[0].row.runs[0].text, "https://apn");
    assert_eq!(rows[0].row.runs[0].link, Some(0));
    assert_eq!(rows[0].row.runs[1].text, "ews.com/x");
    assert_eq!(rows[0].row.runs[1].link, Some(1));
}

/// A row with more distinct links than one message can carry keeps its
/// text and styles and loses the links that do not fit.
///
/// Why: a row cannot be split across messages, and a hostile program can
/// put a new maximal link on every cell; the format's rule is to drop links
/// rather than the row. Spec: 300 cells, each its own link of the longest
/// target the emulator keeps, on a 300-column row, pack into messages that
/// all encode, the row's text intact, at most 256 links in the message.
#[test]
fn a_row_with_too_many_links_drops_links_and_keeps_its_text() {
    let target = "x".repeat(GRID_MAX_LINK_URI_BYTES - "https://".len());
    let mut output = Vec::new();
    for n in 0..300 {
        output.extend(format!("\x1b]8;id={n};https://{target}\x1b\\a").into_bytes());
    }
    output.extend(b"\x1b]8;;\x1b\\");
    let emulator = emulator(300, 2, 10, &output);
    let screen = screen(&emulator);
    let messages = rows_messages(&[(0, &screen.lines[0])]);
    assert_eq!(messages.len(), 1);
    let encoded = messages[0].encode().expect("the message encodes");
    assert!(encoded.len() <= GRID_MAX_MESSAGE);
    let GridOutput::Rows { rows, links } = &messages[0] else {
        panic!("a rows message");
    };
    assert!(links.len() <= GRID_MAX_LINKS, "{} links", links.len());
    assert!(!links.is_empty(), "some links fit");
    let text: String = rows[0]
        .row
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect();
    assert_eq!(text, "a".repeat(300));
}

/// Rows of hostile text (quotes and backslashes, combining floods, wide
/// characters, every style) encode into messages a client accepts.
///
/// Why: the sender encodes every message before sending it, and a message
/// that breaks a bound ends the attachment; text a program controls must
/// never be able to make the sender produce one. Spec: a screen and history
/// of such text, packed into `rows` and `history` messages, all encode and
/// apply to a client, which then shows exactly the emulator's screen and
/// history.
#[test]
fn hostile_text_packs_into_messages_a_client_accepts() {
    let mut output = Vec::new();
    for n in 0..80 {
        output.extend(b"\"\\\"\\");
        output.extend(format!("\x1b[{};38;2;1;2;{}m", 1 + n % 9, n).into_bytes());
        output.extend("e\u{301}\u{302}\u{303}界😀".as_bytes());
        output.extend(b"a");
        for _ in 0..20 {
            output.extend("\u{20dd}".as_bytes());
        }
        output.extend(b"\x1b[m\r\n");
    }
    let emulator = emulator(40, 10, 1000, &output);
    let screen = screen(&emulator);
    let mut client = started_client(screen.cols, screen.rows);
    let history = HistoryCursor::new().read(&emulator, 0, usize::MAX);
    deliver(
        &mut client,
        &history_messages(history.first, history.cols, &history.lines),
    );
    let update = SentScreen::after_hello(screen.cols, screen.rows)
        .update(&screen, 0)
        .expect("a first frame");
    deliver(&mut client, &update);
    assert!(client.shows(&emulator));
}

// ---- The screen ---------------------------------------------------------------

/// An update sends only the rows that changed, nothing at all when nothing
/// changed, and the cursor alone when only it moved.
///
/// Why: diffing against what the client was last sent is what keeps a
/// keystroke's echo one row on the wire, however large the screen. Spec:
/// after the first frame, an unchanged screen needs no update; one changed
/// row is one `rows` entry and a frame; a cursor move alone is a frame
/// with no rows.
#[test]
fn an_update_sends_only_what_changed() {
    let mut emulator = emulator(20, 5, 100, b"one\r\ntwo\r\n");
    let mut sent = SentScreen::after_hello(20, 5);
    let mut client = started_client(20, 5);
    deliver(&mut client, &sent.update(&screen(&emulator), 0).unwrap());
    assert!(sent.update(&screen(&emulator), 0).is_none());

    feed(&mut emulator, b"three");
    let update = sent.update(&screen(&emulator), 0).unwrap();
    let GridOutput::Rows { rows, .. } = &update[0] else {
        panic!("rows first: {update:?}");
    };
    assert_eq!(rows.iter().map(|row| row.y).collect::<Vec<_>>(), vec![2]);
    deliver(&mut client, &update);

    feed(&mut emulator, b"\x1b[1;1H");
    let update = sent.update(&screen(&emulator), 0).unwrap();
    assert!(matches!(update.as_slice(), [GridOutput::Frame { .. }]));
    deliver(&mut client, &update);
    assert!(client.shows(&emulator));
}

/// Output that scrolls the screen is sent as a shift and the new rows,
/// not the whole screen again.
///
/// Why: a program printing line after line moves every row of the screen;
/// without the shift each new line would resend the screen. Spec: a full
/// screen of distinct lines and two more lines on top, with the two lines
/// pushed into history sent first and the caller allowing a shift of 2, is
/// a frame shifting by 2 with at most three rows (the two new lines, and
/// slack for the cursor's row), after which the client shows the
/// emulator's screen and history.
#[test]
fn scrolling_output_is_a_shift() {
    let mut emulator = emulator(20, 6, 100, &numbered(0, 5));
    let mut sent = SentScreen::after_hello(20, 6);
    let mut client = started_client(20, 6);
    let mut cursor = HistoryCursor::new();
    let first = cursor.read(&emulator, 0, usize::MAX);
    assert!(first.lines.is_empty(), "premise: nothing in history yet");
    deliver(&mut client, &sent.update(&screen(&emulator), 0).unwrap());
    feed(&mut emulator, &numbered(5, 2));
    // The lines the shift pushes off go to the client first, as the sender
    // sends them.
    let pushed = cursor.read(&emulator, 0, usize::MAX);
    assert_eq!(pushed.lines.len(), 2, "premise: two lines scrolled");
    deliver(
        &mut client,
        &history_messages(pushed.first, pushed.cols, &pushed.lines),
    );
    let update = sent.update(&screen(&emulator), 2).unwrap();
    let Some(GridOutput::Frame { shift, .. }) = update.last() else {
        panic!("a frame last: {update:?}");
    };
    assert_eq!(*shift, 2);
    let sent_rows: usize = update
        .iter()
        .map(|message| match message {
            GridOutput::Rows { rows, .. } => rows.len(),
            _ => 0,
        })
        .sum();
    assert!(sent_rows <= 3, "{sent_rows} rows sent for a scroll of 2");
    deliver(&mut client, &update);
    assert!(client.shows(&emulator));
}

/// A frame that resizes the screen sends every row that is not blank.
///
/// Why: the client blanks every row a resizing frame does not update, so a
/// row left out because it did not change would vanish. Spec: after a
/// resize from 20x5 to 30x5, the update carries every non-blank row and a
/// frame of the new size, and the client shows the emulator's screen.
#[test]
fn a_resizing_frame_sends_every_row() {
    let mut emulator = emulator(20, 5, 100, b"one\r\ntwo\r\nthree");
    let mut sent = SentScreen::after_hello(20, 5);
    let mut client = started_client(20, 5);
    deliver(&mut client, &sent.update(&screen(&emulator), 0).unwrap());
    emulator.resize(
        Size {
            columns: 30,
            lines: 5,
        },
        FeedMode::Live,
        &mut Collected::default(),
    );
    let update = sent.update(&screen(&emulator), 0).unwrap();
    let rows: usize = update
        .iter()
        .map(|message| match message {
            GridOutput::Rows { rows, .. } => rows.len(),
            _ => 0,
        })
        .sum();
    assert_eq!(rows, 3);
    deliver(&mut client, &update);
    assert!(client.shows(&emulator));
}

/// The cursor blinks unless the program chose a steady style, and hides
/// when the program hides it.
///
/// Why: today's terminal blinks the cursor by default, where alacritty's
/// default is steady; a program's choice of shape and blink is kept.
/// Spec: by default a visible blinking block; after `CSI 4 q` a steady
/// underline; after `CSI 5 q` a blinking bar; after `CSI ? 25 l` hidden.
#[test]
fn the_cursor_follows_the_program_and_blinks_by_default() {
    let mut emulator = emulator(20, 3, 100, b"");
    let cursor = |emulator: &Emulator| screen(emulator).cursor;
    let shown = cursor(&emulator);
    assert_eq!(
        (shown.shape, shown.blink, shown.visible),
        (CursorShape::Block, true, true)
    );
    feed(&mut emulator, b"\x1b[4 q");
    let shown = cursor(&emulator);
    assert_eq!((shown.shape, shown.blink), (CursorShape::Underline, false));
    feed(&mut emulator, b"\x1b[5 q");
    let shown = cursor(&emulator);
    assert_eq!((shown.shape, shown.blink), (CursorShape::Bar, true));
    feed(&mut emulator, b"\x1b[?25l");
    assert!(!cursor(&emulator).visible);
}

// ---- History ------------------------------------------------------------------

/// History is numbered once and later reads append from where the last
/// one ended.
///
/// Why: appended ranges are what let a viewer that missed screen frames
/// still receive every line that scrolled into history. Spec: after 10
/// lines on a 5-row screen, the first read returns the 6 lines in history;
/// after 4 more lines, the next read returns exactly the 4 new history
/// lines, numbered on from the first, with no reset.
#[test]
fn history_reads_append_where_the_last_ended() {
    let mut emulator = emulator(20, 5, 100, &numbered(0, 10));
    let mut cursor = HistoryCursor::new();
    let first = cursor.read(&emulator, 0, usize::MAX);
    assert_eq!(first.reset, None);
    assert_eq!(first.lines.len(), 6);
    assert_eq!(row_text(&first.lines[0]), "line 0000");
    feed(&mut emulator, &numbered(10, 4));
    let next = cursor.read(&emulator, 0, usize::MAX);
    assert_eq!(next.reset, None);
    assert_eq!(next.first, first.first + 6);
    let texts: Vec<String> = next.lines.iter().map(row_text).collect();
    assert_eq!(
        texts,
        vec!["line 0006", "line 0007", "line 0008", "line 0009"]
    );
    assert_eq!(next.owed(), 0);
}

/// A read limited to fewer lines than are owed leaves the rest owed, and
/// the next read continues with them.
///
/// Why: the sender reads history in bounded batches so as not to hold the
/// emulator's lock for long; batches must still add up to every line.
/// Spec: with 20 lines in history, reads of 8, 8 and 8 return 8, 8 and 4
/// lines in order, owing 12, 4 and then nothing.
#[test]
fn bounded_reads_add_up_to_every_line() {
    // 24 lines and the cursor's row on a 5-row screen: 20 in history.
    let emulator = emulator(20, 5, 100, &numbered(0, 24));
    let mut cursor = HistoryCursor::new();
    let mut texts = Vec::new();
    let mut owed = Vec::new();
    for _ in 0..3 {
        let batch = cursor.read(&emulator, 0, 8);
        texts.extend(batch.lines.iter().map(row_text));
        owed.push(batch.owed());
    }
    assert_eq!(owed, vec![12, 4, 0]);
    let expected: Vec<String> = (0..20).map(|n| format!("line {n:04}")).collect();
    assert_eq!(texts, expected);
}

/// A read bounded by bytes stops once its lines come to the bound, takes
/// at least one line, and successive reads still add up to every line.
///
/// Why: a live cycle sends its history ahead of its screen frame, and lines
/// of a colored scrollback encode at kilobytes each, so a cycle bounded by
/// lines alone could hold the frame back behind megabytes; a bound that
/// could return nothing would stall the history. Spec: with 20 short lines
/// in history (about 93 bytes each by the estimate), reads bounded at 200
/// bytes return 3 lines each until the last, which returns the 2 left, in
/// order; a read bounded at 1 byte returns one line.
#[test]
fn reads_bounded_by_bytes_add_up_to_every_line() {
    let emulator = emulator(20, 5, 100, &numbered(0, 24));
    let mut cursor = HistoryCursor::new();
    let mut texts = Vec::new();
    let mut sizes = Vec::new();
    loop {
        let batch = cursor.read_within(&emulator, 0, usize::MAX, 200);
        sizes.push(batch.lines.len());
        texts.extend(batch.lines.iter().map(row_text));
        if batch.owed() == 0 {
            break;
        }
    }
    assert_eq!(sizes, vec![3, 3, 3, 3, 3, 3, 2]);
    let expected: Vec<String> = (0..20).map(|n| format!("line {n:04}")).collect();
    assert_eq!(texts, expected);

    let mut cursor = HistoryCursor::new();
    assert_eq!(
        cursor.read_within(&emulator, 0, usize::MAX, 1).lines.len(),
        1
    );
}

/// A read bounded by bytes stays within its bound plus one row as encoded,
/// links included.
///
/// Why: a link's target can be kilobytes behind one visible character, and
/// goes out in the message's link table; a bound that counted only the
/// text would let one live cycle send megabytes of history ahead of its
/// screen frame. Spec: 41 lines in history, each with 16 cells that are
/// links of their own to targets of 2,000 bytes, read with a bound of
/// 64 KiB: every row keeps its 16 links, and the history messages the
/// lines encode to take at most the bound plus the largest single row's
/// encoding plus 256 bytes of envelope per message.
#[test]
fn a_read_bounded_by_bytes_counts_links() {
    let target = "x".repeat(2000);
    let mut output = Vec::new();
    for line in 0..45 {
        for cell in 0..16 {
            output.extend_from_slice(
                format!("\x1b]8;id=l{line}c{cell};https://example.com/{cell}/{target}\x1b\\")
                    .as_bytes(),
            );
            output.push(b'a' + cell as u8);
            output.extend_from_slice(b"\x1b]8;;\x1b\\");
        }
        output.extend_from_slice(b"\r\n");
    }
    let emulator = emulator(80, 5, 100, &output);
    let bound = 64 * 1024;
    let batch = HistoryCursor::new().read_within(&emulator, 0, usize::MAX, bound);
    assert!(
        !batch.lines.is_empty() && batch.owed() > 0,
        "premise: bounded"
    );
    assert!(
        batch
            .lines
            .iter()
            .all(|row| row.runs.iter().filter(|run| run.link.is_some()).count() == 16),
        "premise: every row keeps its links"
    );
    let encoded_len = |messages: &[GridOutput]| -> usize {
        messages
            .iter()
            .map(|message| message.encode().expect("a message").len())
            .sum()
    };
    let messages = history_messages(batch.first, batch.cols, &batch.lines);
    let largest_row = batch
        .lines
        .iter()
        .map(|row| encoded_len(&history_messages(0, batch.cols, std::slice::from_ref(row))))
        .max()
        .unwrap();
    let encoded = encoded_len(&messages);
    assert!(
        encoded <= bound + largest_row + 256 * messages.len(),
        "{encoded} bytes for a bound of {bound} ({} lines, largest row {largest_row})",
        batch.lines.len()
    );
}

/// Lines owed that scrolled off the far end before they were read make
/// the next read a reset to the oldest line held.
///
/// Why: an append cannot skip lines, and the client would refuse a gap; the
/// lines it never got are gone from the emulator too, so starting over from
/// what is held leaves it with the emulator's history. Spec: with 10 lines
/// of scrollback, after 4 lines read and 30 more scrolled in, the next read
/// is an eviction reset whose lines are the 10 the emulator holds.
#[test]
fn owed_lines_evicted_before_a_read_reset_history() {
    let mut emulator = emulator(20, 5, 10, &numbered(0, 8));
    let mut cursor = HistoryCursor::new();
    cursor.read(&emulator, 0, usize::MAX);
    feed(&mut emulator, &numbered(8, 30));
    let batch = cursor.read(&emulator, 0, usize::MAX);
    assert_eq!(batch.reset, Some(HistoryReset::Evicted));
    assert_eq!(batch.lines, held_history(&emulator));
    assert_eq!(batch.lines.len(), 10);
}

/// An erased history, and a relayout the runtime counted, make the next
/// read a relayout reset.
///
/// Why: after `CSI 3 J` or a resize, line numbers sent before name other
/// lines or none; the client has to rebuild its history rather than append
/// to it. Spec: after `CSI 3 J` the next read is a relayout reset with no
/// lines; after more output and a relayout count that moved, the next read
/// is again a relayout reset with the history held.
#[test]
fn erasure_and_relayouts_reset_history() {
    let mut emulator = emulator(20, 5, 100, &numbered(0, 10));
    let mut cursor = HistoryCursor::new();
    cursor.read(&emulator, 0, usize::MAX);
    feed(&mut emulator, b"\x1b[3J");
    let batch = cursor.read(&emulator, 0, usize::MAX);
    assert_eq!(batch.reset, Some(HistoryReset::Relayout));
    assert!(batch.lines.is_empty());
    feed(&mut emulator, &numbered(10, 10));
    assert_eq!(cursor.read(&emulator, 0, usize::MAX).reset, None);
    let batch = cursor.read(&emulator, 1, usize::MAX);
    assert_eq!(batch.reset, Some(HistoryReset::Relayout));
    assert_eq!(batch.lines, held_history(&emulator));
}

/// History lines carry their own width, and an alternate screen does not
/// move the primary screen's history.
///
/// Why: the client keeps each history line's width for joining wrapped
/// lines; and a full-screen program's redraws must not be read as history.
/// Spec: history lines read from a 30-column emulator say 30 columns;
/// output on the alternate screen leaves nothing owed.
#[test]
fn history_has_its_width_and_ignores_the_alternate_screen() {
    let mut emulator = emulator(30, 5, 100, &numbered(0, 10));
    let mut cursor = HistoryCursor::new();
    let batch = cursor.read(&emulator, 0, usize::MAX);
    assert_eq!(batch.cols, 30);
    feed(&mut emulator, b"\x1b[?1049h");
    feed(&mut emulator, &numbered(10, 20));
    let batch = cursor.read(&emulator, 0, usize::MAX);
    assert_eq!((batch.reset, batch.lines.len()), (None, 0));
}

// ---- Packing ------------------------------------------------------------------

/// History packs into messages of about [`MESSAGE_TARGET`] bytes, numbered
/// on from one message to the next.
///
/// Why: message size, not count, is what bounds the bytes one attachment
/// has queued on the connection, and a client refuses a gap between
/// appends. Spec: 2,000 lines of 100 characters pack into several messages,
/// none over the target, each starting where the previous ended, and
/// a client applying them holds every line.
#[test]
fn history_packs_into_bounded_contiguous_messages() {
    let line: String = "x".repeat(100);
    let lines: Vec<CellRow> = (0..2000)
        .map(|_| CellRow {
            wraps: false,
            runs: vec![CellRun {
                col: 0,
                width: 100,
                text: line.clone(),
                style: plain(Color::Palette(3)),
                link: None,
            }],
        })
        .collect();
    let messages = history_messages(7, 100, &lines);
    assert!(messages.len() > 1);
    let mut check = GridStreamCheck::new();
    check
        .accept(&GridOutput::Hello {
            cols: 100,
            rows: 5,
            history_cap: 5000,
        })
        .unwrap();
    check
        .accept(&GridOutput::State(terminal_state(&emulator(1, 1, 0, b""))))
        .unwrap();
    for message in &messages {
        let bytes = message.encode().expect("encodes");
        assert!(bytes.len() <= MESSAGE_TARGET, "{} bytes", bytes.len());
        check.accept(message).expect("contiguous");
    }
    let mut client = started_client(100, 5);
    client.history_cap = 5000;
    deliver(&mut client, &messages);
    assert_eq!(client.history.len(), 2000);
    assert_eq!(client.history.front().map(|line| line.0), Some(7));
}

/// Binary packing uses the full target without charging JSON escaping or
/// repeatedly charging a shared link table. Screen and history prefixes
/// differ, so both paths must stop at the actual next-row byte boundary.
#[test]
fn binary_packing_counts_exact_envelopes_rows_and_shared_links() {
    let row = CellRow {
        wraps: true,
        runs: vec![CellRun {
            col: 0,
            width: 1000,
            text: "\"".repeat(1000),
            style: plain(Color::Palette(3)),
            link: Some(Link {
                id: "shared".into(),
                uri: "https://example.invalid/\"\\".into(),
            }),
        }],
    };
    let lines = vec![row; 100];
    let screen: Vec<_> = lines
        .iter()
        .enumerate()
        .map(|(y, row)| (y as u16, row))
        .collect();
    for messages in [rows_messages(&screen), history_messages(17, 1000, &lines)] {
        assert!(messages.len() > 1, "premise: several full packs");
        for (at, message) in messages.iter().enumerate() {
            let bytes = message.encode().unwrap();
            assert!(bytes.len() <= MESSAGE_TARGET);
            let (envelope, prefix, row_bytes, links) = match message {
                GridOutput::Rows { rows, links } => (
                    8,
                    6,
                    rows.iter().map(|row| row.row.binary_len()).sum::<usize>(),
                    links,
                ),
                GridOutput::History { rows, links, .. } => (
                    18,
                    4,
                    rows.iter().map(Row::binary_len).sum::<usize>(),
                    links,
                ),
                _ => panic!("row-bearing message"),
            };
            assert_eq!(links.len(), 1, "shared links are charged once");
            let count = match message {
                GridOutput::Rows { rows, .. } => rows.len(),
                GridOutput::History { rows, .. } => rows.len(),
                _ => unreachable!(),
            };
            assert_eq!(
                bytes.len(),
                envelope + prefix * count + row_bytes + links[0].binary_len()
            );
            if at + 1 < messages.len() {
                let next_row = cell_row_binary_len(&lines[0]) + prefix;
                assert!(
                    bytes.len() + next_row > MESSAGE_TARGET,
                    "pack stopped before its exact binary boundary"
                );
            }
            assert_eq!(GridOutput::decode(&bytes).unwrap(), *message);
        }
    }
}

// ---- Clipboard ----------------------------------------------------------------

/// A clipboard write is cut into parts within both the text bound and the
/// message bound, at character boundaries, and a write over the format's
/// bound is dropped.
///
/// Why: a part must fit one message once JSON escapes it, and control
/// characters escape to six bytes each; cutting inside a character would
/// make invalid UTF-8. Spec: 1 MiB of `é` and one more is three parts of
/// at most 512 KiB that rejoin to the write; 512 KiB of control characters is cut so
/// every part encodes; an empty write is one empty last part; 8 MiB and one
/// byte is nothing.
#[test]
fn clipboard_writes_split_into_parts_that_fit() {
    let text = "é".repeat(512 * 1024 + 1);
    let parts = clipboard_messages(ClipboardTarget::Clipboard, &text);
    assert_eq!(parts.len(), 3);
    let mut joined = String::new();
    for (at, part) in parts.iter().enumerate() {
        let GridOutput::Clipboard { text, last, target } = part else {
            panic!("a clipboard part");
        };
        assert_eq!(*target, ClipboardTarget::Clipboard);
        assert_eq!(*last, at + 1 == parts.len());
        assert!(text.len() <= GRID_MAX_CLIPBOARD_PART_BYTES);
        part.encode().expect("the part encodes");
        joined.push_str(text);
    }
    assert_eq!(joined, text);

    let controls = "\u{1}".repeat(512 * 1024);
    let parts = clipboard_messages(ClipboardTarget::Primary, &controls);
    assert!(parts.len() > 1);
    for part in &parts {
        part.encode().expect("every part fits a message");
    }

    assert_eq!(
        clipboard_messages(ClipboardTarget::Clipboard, ""),
        vec![GridOutput::Clipboard {
            target: ClipboardTarget::Clipboard,
            text: String::new(),
            last: true
        }]
    );
    let too_large = "a".repeat(GRID_MAX_CLIPBOARD_BYTES + 1);
    assert!(clipboard_messages(ClipboardTarget::Clipboard, &too_large).is_empty());
}

// ---- Round-two cases: shift-only frames and palette rewrites --------------------

/// A scroll that leaves no row to send is still an update: the frame's
/// shift moves the client's rows.
///
/// Why: when the only change is that the screen moved up (a newline at the
/// bottom row), the rows after a shift all match what the client has, and
/// an update that saw no changed row would leave the client's screen where
/// it was while the line went to history. Spec: on a 20x3 screen showing
/// `a`, `b` and a blank cursor row, one more newline with a shift of 1
/// allowed is a frame shifting by 1 with no rows, after which the client
/// shows `b` and two blank rows.
#[test]
fn a_shift_only_update_is_sent() {
    let mut emulator = emulator(20, 3, 100, b"a\r\nb\r\n");
    let mut sent = SentScreen::after_hello(20, 3);
    let mut client = started_client(20, 3);
    deliver(&mut client, &sent.update(&screen(&emulator), 0).unwrap());
    feed(&mut emulator, b"\r\n");
    let mut cursor = HistoryCursor::new();
    let pushed = cursor.read(&emulator, 0, usize::MAX);
    deliver(
        &mut client,
        &history_messages(pushed.first, pushed.cols, &pushed.lines),
    );
    let update = sent
        .update(&screen(&emulator), 1)
        .expect("a shift is an update");
    assert!(
        matches!(update.as_slice(), [GridOutput::Frame { shift: 1, .. }]),
        "{update:?}"
    );
    deliver(&mut client, &update);
    assert!(client.shows(&emulator));
    assert_eq!(row_text(&client.screen[0]), "b");
}

/// Redefining a palette entry after history colored with it was sent
/// resets the client's history, which is then sent in the new color.
///
/// Why: a redefined palette entry travels as a direct color, so history
/// rows sent before the change carry the old color; the client would show
/// them in it while the supervisor's own rows changed. Spec: with entry 1
/// redefined to #ff0000 and a line in color 1 sent into history, redefining
/// it to #00ff00 makes the next read a relayout reset whose rows carry
/// #00ff00.
#[test]
fn a_palette_rewrite_resets_delivered_history() {
    let mut emulator = emulator(
        20,
        3,
        100,
        b"\x1b]4;1;rgb:ff/00/00\x1b\\\x1b[31mred\x1b[m\r\n\r\n\r\n",
    );
    let mut cursor = HistoryCursor::new();
    let first = cursor.read(&emulator, 0, usize::MAX);
    let colored = |batch: &HistoryBatch| {
        batch
            .lines
            .iter()
            .flat_map(|line| line.runs.iter())
            .find(|run| run.text == "red")
            .map(|run| run.style.fg)
    };
    assert_eq!(
        colored(&first),
        Some(Color::Rgb {
            r: 0xff,
            g: 0,
            b: 0
        }),
        "premise: the line went to history in the redefined color"
    );
    feed(&mut emulator, b"\x1b]4;1;rgb:00/ff/00\x1b\\");
    let next = cursor.read(&emulator, 0, usize::MAX);
    assert_eq!(next.reset, Some(HistoryReset::Relayout));
    assert_eq!(
        colored(&next),
        Some(Color::Rgb {
            r: 0,
            g: 0xff,
            b: 0
        })
    );
}

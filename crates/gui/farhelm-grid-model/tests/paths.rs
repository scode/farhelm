//! Printed-path recognition, ported from the old web client's `terminal-files.test.js` at reference commit 24bc0580,
//! whose detection cases are the oracle here. Each test names the old case it replaces; the old cases about hovers,
//! transfers and notices are not detection and are ported with those features. The old client also made a program's
//! `file://` hyperlink a file link; here no hyperlink names a file, and a hyperlink's cells are never part of a
//! printed path, which the tests at the end pin.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test-only crate; crates/gui/clippy.toml's test allowance covers #[test] functions but not these helpers"
)]
use farhelm_grid_model::{
    GridModel, GridOutput, LinkKind, Position, Selection, SelectionMode, TerminalLink, style_flags,
    web_target,
};
use serde_json::{Value, json};

const PLAIN: [i64; 3] = [0, -1, -1];

/// A model whose lines are `rows`, each `(width, row)`, numbered from 0 like the old fixtures' link lines. Every row
/// is a history line of its own width, because the old fixtures gave each row its own length and a cut's fit rule
/// reads it; a blank one-row screen follows them. Messages pass the real encoder and the stream checker.
fn lines(rows: Vec<(u16, Value)>) -> GridModel {
    linked_lines(rows, Vec::new())
}

/// [`lines`] with an OSC 8 link table, `[id, uri]` entries, that every row's runs may index.
fn linked_lines(rows: Vec<(u16, Value)>, links: Vec<Value>) -> GridModel {
    let cols = rows.iter().map(|(width, _)| *width).max().unwrap_or(1);
    let mut messages = vec![
        json!({"t":"hello","cols":cols,"rows":1,"history_cap":10000}),
        json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,"default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
        json!({"t":"frame","rev":1,"cols":cols,"rows":1,"cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":false}}),
    ];
    for (first, (width, row)) in rows.into_iter().enumerate() {
        messages.push(json!({"t":"history","first":first,"cols":width,"rows":[row],"links":links}));
    }
    let mut model = GridModel::new();
    for value in messages {
        let message: GridOutput = serde_json::from_value(value).unwrap();
        let _ = model.apply(&message.encode().unwrap()).unwrap();
    }
    model
}

/// A row of runs `(col, width, text)`; `wraps` says it continues on the next row.
fn row(wraps: bool, runs: &[(u16, u16, &str)]) -> Value {
    let runs: Vec<Value> = runs
        .iter()
        .map(|&(col, width, text)| json!([col, width, text, PLAIN]))
        .collect();
    json!({"w":wraps,"r":runs})
}

/// A row of ASCII text, one byte per cell, as wide as its text unless `width` is larger (blank padding).
fn ascii(text: &str, width: usize, wraps: bool) -> (u16, Value) {
    assert!(text.is_ascii());
    let runs: &[(u16, u16, &str)] = if text.is_empty() {
        &[]
    } else {
        &[(0, text.len() as u16, text)]
    };
    (width.max(text.len()).max(1) as u16, row(wraps, runs))
}

/// `text` soft-wrapped over rows `cols` wide.
fn wrapped(text: &str, cols: usize) -> Vec<(u16, Value)> {
    let pieces: Vec<&str> = (0..text.len())
        .step_by(cols)
        .map(|at| &text[at..(at + cols).min(text.len())])
        .collect();
    let last = pieces.len() - 1;
    pieces
        .into_iter()
        .enumerate()
        .map(|(index, piece)| ascii(piece, cols, index != last))
        .collect()
}

/// The paths on one printed row: its ASCII as plain cells, CJK characters as two-cell runs.
fn printed(text: &str) -> GridModel {
    let mut runs = Vec::new();
    let mut col = 0;
    for (at, ch) in text.char_indices() {
        let width = if ('\u{2e80}'..='\u{9fff}').contains(&ch) {
            2
        } else {
            1
        };
        runs.push((col, width, &text[at..at + ch.len_utf8()]));
        col += width;
    }
    lines(vec![(col.max(1), row(false, &runs))])
}

/// One recognized path as the old tests compared it: the path, and its first and last cell boundary as
/// `(line, col)`.
type Found = (String, (u64, u16), (u64, u16));

/// What the old tests compared: each path touching `line` with its endpoints.
fn found(model: &GridModel, line: u64) -> Vec<Found> {
    model
        .path_links(line)
        .into_iter()
        .map(|link| {
            assert_eq!(link.kind, LinkKind::Path);
            let (start, end) = (link.ranges[0].start, link.ranges.last().unwrap().end);
            (link.uri, (start.line, start.col), (end.line, end.col))
        })
        .collect()
}

fn paths(model: &GridModel, line: u64) -> Vec<String> {
    found(model, line)
        .into_iter()
        .map(|(path, _, _)| path)
        .collect()
}

/// Conceal removes a program hyperlink's hit range without giving the cells back to printed paths: otherwise hidden
/// text a program tagged with one target could be offered as a download of another file. The same concealed text
/// without an OSC 8 tag still follows the ordinary file-path rule. Specified: concealed untagged `src/example.rs` is a
/// path; tagged, it is neither a hyperlink hit nor a path, from the recognizer or the hit test.
#[test]
fn a_concealed_program_link_keeps_ownership_over_a_printed_path() {
    let label = "src/example.rs";
    let width = label.len() as u16;
    let concealed = [i64::from(style_flags::HIDDEN), -1, -1];
    let position = Position { line: 0, col: 2 };
    let plain = lines(vec![(
        width,
        json!({"w":false,"r":[[0,width,label,concealed]]}),
    )]);
    assert_eq!(plain.path_link_at(position).unwrap().uri, label);

    let tagged = linked_lines(
        vec![(width, json!({"w":false,"r":[[0,width,label,concealed,0]]}))],
        vec![json!(["tag", "file:///work/other.rs"])],
    );
    assert!(
        paths(&tagged, 0).is_empty(),
        "a hyperlink's cells are never a printed path"
    );
    assert!(
        tagged.link_at(position).is_none(),
        "conceal removes the OSC hit range"
    );
    assert!(
        tagged.path_link_at(position).is_none(),
        "the raw tag prevents fallback"
    );
}

/// Replaces "paths exclude diagnostic locations, prose punctuation, URLs and versions". The table pins SPEC.md's
/// recognition boundaries: a `:line:column` suffix and sentence punctuation stay outside the link, and URLs, other
/// schemes and versions never become paths, including a scheme-bearing chunk that brackets could otherwise split
/// into a file-looking piece (`document.domain`).
#[test]
fn paths_exclude_diagnostic_locations_prose_punctuation_urls_and_versions() {
    for (input, path) in [
        ("/srv/out/report.csv", "/srv/out/report.csv"),
        ("~/out.csv", "~/out.csv"),
        ("src/main.rs:42:7", "src/main.rs"),
        ("./build/app", "./build/app"),
        ("notes.txt", "notes.txt"),
        ("notes.txt:42:7).", "notes.txt"),
        ("(report.pdf).", "report.pdf"),
        ("\"src/main.rs:42\",", "src/main.rs"),
        ("路径/报告.csv!", "路径/报告.csv"),
    ] {
        assert_eq!(paths(&printed(input), 0), [path], "{input}");
    }
    for input in [
        "hello",
        "1.2.3",
        "v1.2.3",
        "//host/out.csv",
        "https://host/out.csv",
        "ftp://host/out.csv",
        "file:///tmp/out.csv",
        "data:report.pdf",
        "mailto:a@host.example",
        "javascript:alert/report.pdf",
        "https:report.pdf",
        "javascript:alert(document.domain)",
        "ftp://host/(src/main.rs)",
        "(javascript:alert(document.domain))",
    ] {
        assert!(paths(&printed(input), 0).is_empty(), "{input}");
    }
}

/// Replaces "bare digit-leading extensions retain numeric-token exclusions". A digit-leading extension is still file
/// evidence (`backup.7z`), but numbers and quantities like `1.5x` are not, unless a slash makes them an explicit path.
#[test]
fn bare_digit_leading_extensions_keep_the_numeric_token_exclusions() {
    for input in [
        "backup.7z",
        "backup.001",
        "archive.1part",
        "123.txt",
        "./1.5x",
    ] {
        assert_eq!(paths(&printed(input), 0), [input], "{input}");
    }
    for input in [
        "1.5x",
        "-1.5x",
        "+1.5x",
        "v1.5x",
        "1.2rc1",
        "1.2.3",
        "v1.2.3",
        "1.2.3-rc1",
        "1.2+build",
    ] {
        assert!(paths(&printed(input), 0).is_empty(), "{input}");
    }
}

/// Replaces "a long numeric-looking filename completes recognition". A file stem made of many numeric components
/// nearly matches the version guard and must still be a path. The old test bounded the time in a subprocess because
/// a JavaScript regex can backtrack; the regex crate matches in linear time, so only the result is pinned.
#[test]
fn a_long_numeric_looking_file_name_is_a_path() {
    let input = format!("1{}.x", ".1111111111".repeat(8));
    assert_eq!(paths(&printed(&input), 0), [input]);
}

/// Replaces "the numeric-version guard keeps version spellings out and answers near-matches fast". Release-version
/// spellings with pre-release and build suffixes are not paths. The old near-match (`1.1`, thousands of hyphens, a
/// stray `@`) that cost the browser 50 ms a call is read through a whole soft-wrapped line at the 8192-unit bound,
/// and so is the same spelling ending in `x`, which has a file-extension shape and is kept out only by the version
/// guard. The old timing assertion is dropped, since the regex crate cannot backtrack.
#[test]
fn version_spellings_stay_out_including_a_long_near_match() {
    for input in [
        "1.2-rc.3",
        "v1.5x",
        "2.0.1+build.5",
        "1.2.3-beta-2",
        "1.2.3-rc.1+build.5",
        "1.0.0-beta+exp.sha.5114f85",
        "1.2-",
    ] {
        assert!(paths(&printed(input), 0).is_empty(), "{input}");
    }
    for end in ['@', 'x'] {
        let near = format!("1.1{}{end}", "-".repeat(8188));
        assert_eq!(near.len(), 8192);
        let model = lines(wrapped(&near, 512));
        for line in 0..16 {
            assert!(paths(&model, line).is_empty(), "ending {end}, line {line}");
        }
    }
    // The premise that makes the `x` case the guard's alone: with a slash in front the spelling is no longer
    // version-shaped, and it links.
    assert_eq!(paths(&printed("./1.1---x"), 0), ["./1.1---x"]);
}

/// A program hyperlink over one cell to `uri`, as the grid model finds it.
fn hyperlink(uri: &str) -> TerminalLink {
    let model = linked_lines(
        vec![(1, json!({"w":false,"r":[[0,1,"x",PLAIN,0]]}))],
        vec![json!(["id", uri])],
    );
    let link = model.link_at(Position { line: 0, col: 0 }).unwrap();
    assert!(matches!(link.kind, LinkKind::Osc { .. }), "{uri}");
    link
}

/// Replaces "OSC allows web unchanged and decoded file paths only", whose file half SPEC.md withdrew: a program's
/// hyperlink that is not a web address is inert, `file://` included, so its target never reaches a file check or an
/// opener. Specified: a web target stays web and names no file; every other target, the `file://` ones the old client
/// decoded to a path among them, names no file and is no web target either. Each target is a real hyperlink's, read
/// through [`TerminalLink::file_path`].
#[test]
fn program_hyperlinks_name_no_file_whatever_their_scheme() {
    assert!(web_target("https://example.test/x", false).is_some());
    assert_eq!(hyperlink("https://example.test/x").file_path(), None);
    for uri in [
        "file://another-host/tmp/my%20report.pdf",
        "file:///work/link/../report.pdf",
        "file:///work/report.pdf",
        "file://localhost/work/report.pdf",
        "javascript:alert(1)",
        "data:text/html,<script>",
        "ftp://host/a",
        "ssh://build-box/work",
        "file://host?next=/tmp/report.pdf",
        "file:///a%00b",
        "file:///bad%",
    ] {
        let link = hyperlink(uri);
        assert_eq!(link.file_path(), None, "{uri}");
        assert!(web_target(&link.uri, false).is_none(), "{uri}");
    }
}

/// Replaces "path mapping handles wide, combining and wrapped path cells". A wide character and a combining mark
/// before a path, and a soft wrap through it, must not shift its cells: offsets are mapped back to the cells that
/// own them, never counted one column per character. Asked from either row, it is the same link.
#[test]
fn path_cells_map_through_wide_combining_and_wrapped_cells() {
    let model = lines(vec![
        (
            11,
            row(
                true,
                &[(0, 2, "界"), (2, 1, "e\u{301}"), (3, 8, " src/mai")],
            ),
        ),
        ascii("n.rs:42!", 11, false),
    ]);
    for line in [0, 1] {
        assert_eq!(
            found(&model, line),
            [("src/main.rs".to_owned(), (0, 4), (1, 4))]
        );
    }
}

/// Printed paths can wrap before a wide character. Coloring the wrap padding must leave the recognized path intact,
/// while a real space in the same column splits the token.
#[test]
fn path_recognition_joins_over_styled_padding_only() {
    for padding in [false, true] {
        let model = lines(vec![
            (
                9,
                json!({"w":true,"p":padding,"r":[[0,8,"src/img/",[0,-1,-1]],[8,1," ",[0,-1,2]]]}),
            ),
            (9, row(false, &[(0, 2, "界"), (2, 3, ".rs")])),
        ]);
        let path = "src/img/界.rs".to_owned();
        for line in [0, 1] {
            assert_eq!(paths(&model, line).contains(&path), padding);
        }
        assert!(model.path_link_at(Position { line: 0, col: 8 }).is_none());
    }
}

/// Replaces "paths cut near the right edge join through soft wraps and program breaks". The same text links whole
/// through a terminal wrap and through a program's own line break, which TUIs pad after the head and indent before
/// the tail; neither padding nor indentation belongs in the path, and both shapes give the same endpoints from
/// either row. The program break's link covers only the path's own cells on each row, never the padding or the
/// indentation, where the old link was one span that covered both (a deviation the parity file records).
#[test]
fn a_path_cut_near_the_right_edge_joins_through_a_soft_wrap_and_a_program_break() {
    let head = "Failure screenshot (/tmp/farhelm-";
    let tail = "tmp/run-id/failure.jpg)";
    for wraps in [true, false] {
        let (padding, indentation) = if wraps { ("", "") } else { ("   ", "  ") };
        let model = lines(vec![
            ascii(&format!("{head}{padding}"), 0, wraps),
            ascii(&format!("{indentation}{tail}"), 0, false),
        ]);
        let end = (indentation.len() + tail.len() - 1) as u16;
        for line in [0, 1] {
            assert_eq!(
                found(&model, line),
                [(
                    "/tmp/farhelm-tmp/run-id/failure.jpg".to_owned(),
                    (0, head.find('/').unwrap() as u16),
                    (1, end)
                )],
                "wraps={wraps}, line {line}"
            );
        }
        if !wraps {
            let link = model.path_links(0).remove(0);
            let range = |line, from, to| {
                Selection::new(
                    Position { line, col: from },
                    Position { line, col: to },
                    SelectionMode::Linear,
                )
            };
            assert_eq!(link.ranges, [range(0, 20, 33), range(1, 2, 24)]);
            assert!(link.covers(Position { line: 0, col: 32 }));
            assert!(!link.covers(Position { line: 0, col: 33 }), "padding");
            assert!(!link.covers(Position { line: 1, col: 1 }), "indentation");
            assert!(link.covers(Position { line: 1, col: 2 }));
        }
    }
}

/// Replaces "cut paths keep delimiters, column limits and the two-row boundary". The cut exception must not join
/// unrelated rows (a tail that fits), quoted or bracketed tokens, URI fragments, or a chain of three hard-broken rows.
/// Its fit rule counts terminal columns, not characters: exactly four spare columns can be a TUI's margin, one more
/// means the tail would have fit, and a wide or combined glyph counts by its cells.
#[test]
fn cut_paths_keep_delimiters_column_limits_and_the_two_row_boundary() {
    // Each refused case still has its single-row fragments, so a detector that found nothing cannot pass.
    let head = || ascii("/tmp/head-", 0, false);
    let tail = || ascii("tail/file.txt", 0, false);
    for (rows, expected) in [
        (
            vec![ascii("/tmp/head-", 80, false), tail()],
            vec![vec!["/tmp/head-"], vec!["tail/file.txt"]],
        ),
        (
            vec![ascii("\"/tmp/head-\"", 0, false), tail()],
            vec![vec!["/tmp/head-"], vec!["tail/file.txt"]],
        ),
        (
            vec![ascii("(/tmp/head-)", 0, false), tail()],
            vec![vec!["/tmp/head-"], vec!["tail/file.txt"]],
        ),
        (
            vec![head(), ascii("\"tail/file.txt\"", 0, false)],
            vec![vec!["/tmp/head-"], vec!["tail/file.txt"]],
        ),
        (
            vec![ascii("https://example.test/", 0, false), tail()],
            vec![vec![], vec!["tail/file.txt"]],
        ),
        (
            vec![ascii("file:///tmp/head-", 0, false), tail()],
            vec![vec![], vec!["tail/file.txt"]],
        ),
        (
            vec![
                head(),
                ascii("tail/", 0, false),
                ascii("file.txt", 0, false),
            ],
            vec![vec!["/tmp/head-"], vec!["tail/"], vec!["file.txt"]],
        ),
    ] {
        let model = lines(rows);
        for (line, expected) in expected.into_iter().enumerate() {
            assert_eq!(paths(&model, line as u64), expected, "line {line}");
        }
    }
    // Neither cut row may be part of a soft wrap: not a head that continues the row above, nor a tail that wraps
    // into a row that exists. The same rows without the wrap join, so each refusal is the wrap's.
    for continued in [false, true] {
        let model = lines(vec![
            ascii("abc", 0, continued),
            head(),
            ascii("x.rs", 0, false),
        ]);
        let joined = paths(&model, 2).contains(&"/tmp/head-x.rs".to_owned());
        assert_eq!(joined, !continued, "head continues a wrap: {continued}");
        if continued {
            assert_eq!(paths(&model, 2), ["x.rs"]);
        }
    }
    for wraps in [false, true] {
        let model = lines(vec![
            head(),
            ascii("x.rs", 0, wraps),
            // Blank, so the tail is not itself the head of another cut, which would make a chain.
            ascii("", 4, false),
        ]);
        let joined = paths(&model, 0).contains(&"/tmp/head-x.rs".to_owned());
        assert_eq!(joined, !wraps, "tail wraps on: {wraps}");
        if wraps {
            assert_eq!(paths(&model, 0), ["/tmp/head-"]);
        }
    }
    for (space, joins) in [(7, true), (8, false)] {
        let model = lines(vec![
            ascii("/tmp/head-", 10 + space, false),
            ascii("x.rs", 0, false),
        ]);
        assert_eq!(
            paths(&model, 0).contains(&"/tmp/head-x.rs".to_owned()),
            joins,
            "{space} spare columns"
        );
    }
    let wide = lines(vec![
        (17, row(false, &[(0, 2, "界"), (2, 11, " /tmp/head-")])),
        (
            12,
            row(
                false,
                &[(1, 1, "e\u{301}"), (2, 2, "界"), (4, 8, ".txt:12!")],
            ),
        ),
    ]);
    assert_eq!(
        found(&wide, 1),
        [("/tmp/head-e\u{301}界.txt".to_owned(), (0, 3), (1, 8))]
    );
}

/// Replaces "plain words cannot donate a head to a cut path". An older, narrower history row can sit above a new
/// screen so that the fit rule holds; joining `four` to `/work/real.txt` would make the hover ask for the wrong file
/// and hide a valid single-row path. The head's last token must itself be a path.
#[test]
fn a_plain_word_cannot_donate_a_head_to_a_cut_path() {
    for head in ["four", "1.2.3", "https://host/"] {
        let model = lines(vec![
            ascii(head, 20, false),
            ascii("/work/real.txt /work/gone.txt", 40, false),
        ]);
        assert_eq!(
            found(&model, 1),
            [
                ("/work/real.txt".to_owned(), (1, 0), (1, 14)),
                ("/work/gone.txt".to_owned(), (1, 15), (1, 29)),
            ],
            "{head}"
        );
    }
}

/// Replaces the detection half of "a wrapped or program-broken printed path cannot grow beyond its cached endpoint"
/// (its transfer half belongs to the file hover). A redraw can append to a path while every cell of the old link
/// keeps its text; recognition must find the whole new token, and the old link must no longer match it by
/// [`TerminalLink::same_range`], or a click could fetch the previous file name.
#[test]
fn a_printed_path_extended_on_its_next_row_is_a_different_link() {
    for wraps in [false, true] {
        let model =
            |tail: &str| lines(vec![ascii("long/repor", 10, wraps), ascii(tail, 10, false)]);
        let before = model("t.txt").path_links(0);
        assert_eq!(found(&model("t.txt"), 0)[0].2, (1, 5));
        assert_eq!(before[0].uri, "long/report.txt");
        let after = model("t.txt.bak").path_links(0);
        assert_eq!(after[0].uri, "long/report.txt.bak", "wraps={wraps}");
        assert!(!after[0].same_range(&before[0]));
        let back = model("t.txt").path_links(0);
        assert!(back[0].same_range(&before[0]));
        assert_eq!(back, before);
    }
}

/// Replaces the comparison inside "different paths, cell endpoints and unknown ranges start fresh checks" (the
/// check-cancelling half belongs to the file hover). A file hover keeps its check only for the same endpoints: any
/// moved start or end, on either axis, is another hover, and a link without ranges never matches, so identity fails
/// closed.
#[test]
fn same_range_compares_both_endpoints_and_fails_closed() {
    let model = printed("x report.pdf y");
    let link = model.path_links(0).remove(0);
    let moved = |start: (u64, u16), end: (u64, u16)| {
        let mut other = link.clone();
        other.ranges[0].start = Position {
            line: start.0,
            col: start.1,
        };
        other.ranges[0].end = Position {
            line: end.0,
            col: end.1,
        };
        other
    };
    assert!(link.same_range(&moved((0, 2), (0, 12))));
    for (start, end) in [((0, 1), (0, 12)), ((1, 2), (1, 12)), ((0, 2), (0, 13))] {
        assert!(!link.same_range(&moved(start, end)), "{start:?}..{end:?}");
    }
    let mut longer = link.clone();
    longer.ranges.push(moved((1, 0), (1, 3)).ranges[0]);
    assert!(!link.same_range(&longer));
    let empty = TerminalLink {
        ranges: Vec::new(),
        ..link.clone()
    };
    assert!(!empty.same_range(&empty) && !link.same_range(&empty));
}

/// Pins the bound grid-terminal.spec.ts's "a late link answer is dropped when a row of a long printed URL beyond the
/// path detector's reach changes" relies on, the old `softPathLinks` limits: a soft-wrapped logical line has path
/// links through 129 rows and 8192 UTF-16 units, and none at all past either, from any row, rather than a truncated
/// token whose start would change what counts as a path.
#[test]
fn the_detector_reaches_129_rows_and_8192_units_of_a_wrapped_line() {
    for (rows, reached) in [(129, true), (130, false)] {
        let text = format!("/{}", "a".repeat(rows * 10 - 1));
        let model = lines(wrapped(&text, 10));
        for line in [0, 64, rows as u64 - 1] {
            assert_eq!(
                !paths(&model, line).is_empty(),
                reached,
                "{rows} rows, line {line}"
            );
        }
    }
    for (combining, reached) in [(false, true), (true, false)] {
        let mut rows = vec![ascii("/tmp/x.txt", 512, true)];
        rows.extend((1..15).map(|_| ascii("", 512, true)));
        // A combining mark adds a second UTF-16 unit in one cell: 8193 units in 16 rows of 512 columns.
        let last: &[(u16, u16, &str)] = if combining {
            &[(0, 1, "e\u{301}")]
        } else {
            &[]
        };
        rows.push((512, row(false, last)));
        assert_eq!(
            paths(&lines(rows), 0) == ["/tmp/x.txt"],
            reached,
            "combining={combining}"
        );
    }
}

/// The old client asked its links in order, program hyperlinks, printed web addresses, then paths, and the first one
/// covering a cell won: a path never replaces a program's explicit target with its visible label. A path link's
/// target is the printed path and never a URL, so it cannot reach the browser opener, and a `file://` hyperlink names
/// no file through the same [`TerminalLink::file_path`]. The existing lookups return no paths, and the path lookup
/// returns none over the hyperlink's cells.
#[test]
fn hyperlinks_and_web_addresses_own_their_cells_and_a_path_never_opens_a_browser() {
    let text = "/tmp/x.txt https://h.test/a.txt /srv/y.txt";
    let model = linked_lines(
        vec![(
            text.len() as u16,
            json!({"w":false,"r":[[0,10,"/tmp/x.txt",PLAIN,0],[10,32,&text[10..],PLAIN]]}),
        )],
        vec![json!(["id", "file:///tmp/x.txt"])],
    );
    let hyperlink = model.link_at(Position { line: 0, col: 2 }).unwrap();
    assert!(matches!(hyperlink.kind, LinkKind::Osc { .. }));
    assert_eq!(
        hyperlink.file_path(),
        None,
        "an inert hyperlink names no file"
    );
    assert_eq!(model.path_link_at(Position { line: 0, col: 2 }), None);
    assert_eq!(model.path_link_at(Position { line: 0, col: 15 }), None);
    let path = model.path_link_at(Position { line: 0, col: 35 }).unwrap();
    assert_eq!(path.file_path().as_deref(), Some("/srv/y.txt"));
    assert!(web_target(&path.uri, false).is_none() && web_target(&path.uri, true).is_none());
    let web = model.link_at(Position { line: 0, col: 15 }).unwrap();
    assert_eq!(web.file_path(), None);
    assert!(
        model
            .row_links(0)
            .iter()
            .all(|link| link.kind != LinkKind::Path)
    );
    assert_eq!(paths(&model, 0), ["/srv/y.txt"]);
}

/// A row of runs `(col, width, text, hyperlinked)`, the hyperlinked ones all hyperlink `0` of their model's table.
fn tagged_row(wraps: bool, runs: &[(u16, u16, &str, bool)]) -> Value {
    let runs: Vec<Value> = runs
        .iter()
        .map(|&(col, width, text, tagged)| {
            if tagged {
                json!([col, width, text, PLAIN, 0])
            } else {
                json!([col, width, text, PLAIN])
            }
        })
        .collect();
    json!({"w":wraps,"r":runs})
}

/// The paths touching each of `rows` lines.
fn paths_by_line(model: &GridModel, rows: u64) -> Vec<Vec<String>> {
    (0..rows).map(|line| paths(model, line)).collect()
}

/// SPEC.md: a printed path never includes a cell a program's hyperlink owns. Plain text right before an inert
/// hyperlink must not borrow its label: `/tmp/` printed before a `file://` hyperlink labelled `report.pdf` would
/// otherwise offer `/tmp/report.pdf`, a file the program named only through the hyperlink it is no longer allowed to
/// act on. Specified: with the label on the same row, across a soft wrap, or as a cut path's continuation, no path
/// includes the label, from any cell; without the tag (the premise), the same text is that one path. A cut that would
/// take a hyperlink's cells is refused before cuts are weighed against fragments, so the plain head row stays the
/// path it is on its own.
#[test]
fn plain_text_before_a_hyperlink_never_borrows_its_label_as_a_path() {
    let link = || vec![json!(["id", "file:///tmp/report.pdf"])];
    for tagged in [false, true] {
        let same_row = linked_lines(
            vec![(
                15,
                tagged_row(
                    false,
                    &[(0, 5, "/tmp/", false), (5, 10, "report.pdf", tagged)],
                ),
            )],
            link(),
        );
        let soft = linked_lines(
            vec![
                (5, tagged_row(true, &[(0, 5, "/tmp/", false)])),
                (10, tagged_row(false, &[(0, 10, "report.pdf", tagged)])),
            ],
            link(),
        );
        if tagged {
            assert!(paths(&same_row, 0).is_empty(), "same row");
            assert!(
                paths_by_line(&soft, 2).iter().all(Vec::is_empty),
                "soft wrap"
            );
            for col in 0..15 {
                assert_eq!(
                    same_row.path_link_at(Position { line: 0, col }),
                    None,
                    "col {col}"
                );
            }
        } else {
            assert_eq!(paths(&same_row, 0), ["/tmp/report.pdf"], "the premise");
            assert_eq!(
                paths_by_line(&soft, 2),
                [["/tmp/report.pdf"]; 2],
                "the premise"
            );
        }
    }
    // A program break: the head fits the cut rule (four spare columns, a four-column tail, so the tail could not have
    // fit with a margin to spare), which the untagged premise shows by joining.
    for tagged in [false, true] {
        let cut = linked_lines(
            vec![
                ascii("/tmp/head-", 14, false),
                (4, tagged_row(false, &[(0, 4, "x.rs", tagged)])),
            ],
            link(),
        );
        let expected: [&[&str]; 2] = if tagged {
            [&["/tmp/head-"], &[]]
        } else {
            [&["/tmp/head-x.rs"], &["/tmp/head-x.rs"]]
        };
        assert_eq!(paths_by_line(&cut, 2), expected, "tagged={tagged}");
    }
}

/// A cut path is joined across the padding after its head and the indentation before its tail, which are blanks a
/// TUI left. A hyperlink's space is the program's text, not such a blank (SPEC.md: a hyperlink's cell never counts as
/// padding when a cut path is joined), so a join that skipped it would build a path across a hyperlink. Specified:
/// a head `/tmp/head-` with four blank columns after it joins its tail `x.rs` (the premise), and with those four
/// columns a hyperlink's spaces it does not; a tail indented by two blank columns joins, and indented by a
/// hyperlink's two spaces it does not. Each refusal leaves both rows' own paths.
#[test]
fn a_hyperlinks_space_is_never_padding_or_indentation_for_a_cut() {
    let link = || vec![json!(["id", "file:///tmp/head-x.rs"])];
    for tagged in [false, true] {
        let padded = linked_lines(
            vec![
                (
                    14,
                    tagged_row(
                        false,
                        &[(0, 10, "/tmp/head-", false), (10, 4, "    ", tagged)],
                    ),
                ),
                ascii("x.rs", 0, false),
            ],
            link(),
        );
        let indented = linked_lines(
            vec![
                ascii("/tmp/head-", 16, false),
                (
                    6,
                    tagged_row(false, &[(0, 2, "  ", tagged), (2, 4, "x.rs", false)]),
                ),
            ],
            link(),
        );
        for (name, model) in [("padding", padded), ("indentation", indented)] {
            let expected: [&[&str]; 2] = if tagged {
                [&["/tmp/head-"], &["x.rs"]]
            } else {
                [&["/tmp/head-x.rs"], &["/tmp/head-x.rs"]]
            };
            assert_eq!(
                paths_by_line(&model, 2),
                expected,
                "{name}, tagged={tagged}"
            );
        }
    }
}

/// The program-cut exception joins one isolated pair of rows, never a chain: of three rows that could each be cut,
/// a two-row join would offer a piece of a longer path, resolved against the session's directory. Refusing a cut
/// because a hyperlink owns its cells must not make the chain look shorter. Specified: with rows `/srv/`, `data/` and
/// `file.txt`, each pair meeting the cut rule, nothing is joined, and each row keeps only its own path, whether or not
/// `/srv/` is a hyperlink; with it a hyperlink, `/srv/` itself is no path and `data/file.txt` is offered from no row.
#[test]
fn a_cut_refused_for_a_hyperlink_still_makes_its_rows_a_chain() {
    for tagged in [false, true] {
        let model = linked_lines(
            vec![
                (9, tagged_row(false, &[(0, 5, "/srv/", tagged)])),
                ascii("data/", 9, false),
                ascii("file.txt", 0, false),
            ],
            vec![json!(["id", "file:///srv/data/file.txt"])],
        );
        let head: &[&str] = if tagged { &[] } else { &["/srv/"] };
        let expected: [&[&str]; 3] = [head, &["data/"], &["file.txt"]];
        assert_eq!(paths_by_line(&model, 3), expected, "tagged={tagged}");
    }
}

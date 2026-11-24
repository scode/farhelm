//! Recognition stays on validated cell data; browser dispatch is an allowlist.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test-only crate; crates/gui/clippy.toml's test allowance covers #[test] functions but not these helpers"
)]
mod common;

use farhelm_grid_model::{
    GridModel, GridOutput, LinkKind, LinkWarning, Position, SelectionMode, WarningReason, colors,
    style_flags, tunnel_port, web_target,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Explicit cell rows pass the actual wire encoder/checker before lookup. This
/// fixture has no terminal emulator or UI whose URL detection could mask ours.
fn model(cols: u16, rows: Vec<Value>, links: Vec<Value>) -> GridModel {
    let count = rows.len();
    let mut model = GridModel::new(common::palette());
    for value in [
        json!({"t":"hello","cols":cols,"rows":count,"history_cap":10000}),
        json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,"default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
        json!({"t":"rows","rows":rows.into_iter().enumerate().map(|(index,row)|json!([index,row])).collect::<Vec<_>>(),"links":links}),
        json!({"t":"frame","rev":1,"cols":cols,"rows":count,"cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":false}}),
    ] {
        apply_message(&mut model, value);
    }
    model
}

/// Apply the encoded message even for an update, so a conceal toggle has the same frame boundary as live rendering.
fn apply_message(model: &mut GridModel, value: Value) {
    let message: GridOutput = serde_json::from_value(value).unwrap();
    let _ = model.apply(&message.encode().unwrap()).unwrap();
}

/// ASCII has one byte per wire cell. Unicode cases supply their widths below
/// instead of letting a test's width heuristic decide the expected ownership.
fn ascii(text: &str, wraps: bool, link: Option<usize>) -> Value {
    assert!(text.is_ascii());
    let run = if let Some(link) = link {
        json!([0, text.len(), text, [0, -1, -1], link])
    } else {
        json!([0, text.len(), text, [0, -1, -1]])
    };
    json!({"w":wraps,"r":[run]})
}

/// The hover's warning for the link at `(line, col)`, which must be a program hyperlink with a web destination.
fn warning(grid: &GridModel, line: u64, col: u16) -> Option<LinkWarning> {
    let link = grid.link_at(Position { line, col }).unwrap();
    assert!(matches!(link.kind, LinkKind::Osc { .. }), "{link:?}");
    grid.link_disclosure(&link).unwrap().warning
}

/// The reasons of a warning, or none for the ordinary display: the shape most assertions here compare.
fn reasons(warning: &Option<LinkWarning>) -> Vec<WarningReason> {
    warning
        .as_ref()
        .map_or_else(Vec::new, |warning| warning.reasons.clone())
}

use WarningReason::{Credentials, Disguised, Hidden, Incomplete, Mismatch};

/// A web address anywhere in a label is a claim about where the link goes, also when the label wraps: prose before
/// it, a ticket number glued to it or a scheme-less host don't make it descriptive text (SPEC.md's terminal links
/// paragraph). This replaces the old rule that a label was an address only when the whole of it looked like one, under
/// which a spoof could hide behind one word. Spec: each label, soft-wrapped over two rows of one explicit hyperlink,
/// warns from either row against another site, quoting the address it mentions and the whole label with the wrap
/// joined; against the address it names, it is quiet.
#[test]
fn a_wrapped_program_label_warns_on_an_address_anywhere_in_it() {
    let columns = 20;
    for (label, address, own) in [
        (
            "ticket-42(https://example.test/x)",
            "https://example.test/x",
            "https://example.test/x",
        ),
        (
            "ticket-42(example.test/tickets/42)",
            "example.test/tickets/42",
            "https://example.test/tickets/42",
        ),
        (
            "https://example.test/tickets/42",
            "https://example.test/tickets/42",
            "https://example.test/tickets/42",
        ),
    ] {
        let (first, last) = label.split_at(usize::from(columns));
        for (target, warns) in [("https://other.test/", true), (own, false)] {
            let grid = model(
                columns,
                vec![ascii(first, true, Some(0)), ascii(last, false, Some(0))],
                vec![json!(["example", target])],
            );
            let start = grid.link_at(Position { line: 0, col: 0 }).unwrap();
            let end = grid.link_at(Position { line: 1, col: 0 }).unwrap();
            assert_eq!(start.label, first);
            assert_eq!(end.label, last);
            assert_eq!(start.kind, end.kind, "both rows share the explicit tag");
            for link in [start, end] {
                let disclosure = grid.link_disclosure(&link).unwrap();
                let warning = disclosure.warning;
                if warns {
                    let warning = warning.unwrap();
                    assert_eq!(warning.reasons, [Mismatch], "{label}");
                    assert_eq!(warning.address.as_deref(), Some(address), "{label}");
                    assert_eq!(warning.label, label, "the soft wrap joins");
                } else {
                    assert_eq!(warning, None, "{label} -> {target}");
                }
            }
        }
    }
}

/// A bracketed or quoted address is still a claim about the destination, and so is one inside Markdown or after
/// prose. The warning quotes the address without its wrapping, and keeps the whole label, punctuation and words
/// included, so the user can compare what they saw with the actual target. Spec: each one-row label warns against
/// another site, naming `https://example.test/` as the address and the trimmed label as the label; against
/// `https://example.test/` it is quiet.
#[test]
fn quoting_an_address_keeps_the_original_label_in_mismatch_warnings() {
    for label in [
        "<https://example.test/>",
        "\"https://example.test/\"",
        "'https://example.test/'",
        "(https://example.test/)",
        "[https://example.test/]",
        " ( \"<https://example.test/>\" ) ",
        "See (https://example.test/)",
        "ticket-42(https://example.test/)",
        "[docs](https://example.test/)",
        "[https://example.test/] [docs]",
        "<https://example.test/)",
    ] {
        for (target, differs) in [
            ("https://example.test/", false),
            ("https://other.test/", true),
        ] {
            let grid = model(
                48,
                vec![ascii(label, false, Some(0))],
                vec![json!(["tag", target])],
            );
            let link = grid.link_at(Position { line: 0, col: 0 }).unwrap();
            let disclosure = grid.link_disclosure(&link).unwrap();
            assert_eq!(disclosure.target, target);
            if differs {
                let warning = disclosure.warning.unwrap();
                assert_eq!(warning.reasons, [Mismatch], "{label:?}");
                assert_eq!(
                    warning.address.as_deref(),
                    Some("https://example.test/"),
                    "{label:?}"
                );
                assert_eq!(warning.label, label.trim(), "{label:?}");
            } else {
                assert_eq!(disclosure.warning, None, "{label:?}");
            }
        }
    }
}

/// Ported from the fix for a bare address below a bracketed one (#4748), whose two-row case the label reader now
/// covers on every row: a bracketed row joined to a bare one doesn't look like one address, so judging the join alone
/// would show no warning while the bare row names another place. Spec: `<https://example.test/>` above
/// `https://other.test/`, one explicit hyperlink to `https://example.test/` on two hard rows, warns from both rows,
/// naming `https://other.test/`. The old answer for the first row, quiet, came from judging that row alone; the label is
/// now judged whole from every row (SPEC.md), so both rows show the same warning.
#[test]
fn a_bare_address_row_beside_a_bracketed_one_keeps_its_claim() {
    let bracketed = "<https://example.test/>";
    let bare = "https://other.test/";
    let target = "https://example.test/";
    let grid = model(
        40,
        vec![
            ascii(bracketed, false, Some(0)),
            ascii(bare, false, Some(0)),
        ],
        vec![json!(["tag", target])],
    );
    let first = grid.link_at(Position { line: 0, col: 0 }).unwrap();
    let second = grid.link_at(Position { line: 1, col: 0 }).unwrap();
    assert_eq!(first.kind, second.kind);
    let shown = warning(&grid, 0, 0);
    assert_eq!(shown, warning(&grid, 1, 0), "both rows show the same hover");
    let shown = shown.unwrap();
    assert_eq!(shown.reasons, [Mismatch]);
    assert_eq!(shown.address.as_deref(), Some(bare));
    assert_eq!(shown.label, format!("{bracketed} {bare}"));
}

/// Honest output can print one address several times under one hyperlink, quoted differently on each row. If the
/// repeat were not recognized, the hover would warn about an address that names the real target. Spec: the two rows
/// each start with an address after punctuation only, so the seam separates and each address is judged alone; both
/// name the destination, and both rows are quiet.
#[test]
fn repeated_quoted_addresses_each_match_the_destination() {
    let first = "<https://example.test/>";
    let second = "<('https://example.test/')>";
    let grid = model(
        40,
        vec![ascii(first, false, Some(0)), ascii(second, false, Some(0))],
        vec![json!(["tag", "https://example.test/"])],
    );
    for (line, label) in [(0, first), (1, second)] {
        let link = grid.link_at(Position { line, col: 0 }).unwrap();
        assert_eq!(link.label, label);
        let disclosure = grid.link_disclosure(&link).unwrap();
        assert_eq!(disclosure.target, "https://example.test/");
        assert_eq!(disclosure.warning, None);
    }
}

/// A row that shows a multi-row hyperlink in two separate stretches can't be put back into one text, so the label
/// can't be checked whole: a gap is how a program would keep part of an address out of the comparison. Spec: with
/// `<https://example.test/>`, a gap and `more` on row 0 and `continuation` on row 1, all one hyperlink to the address
/// row 0 names, the first stretch's hover is incomplete (it mentions an address), shows every piece as the partial
/// label (the break before `continuation` joins, as in a complete label), and quotes no address. Updated from a warning that quoted the stretch as a mismatching claim: incomplete is
/// now its own reason, and an address that matches is no mismatch.
#[test]
fn a_quoted_address_fragment_warns_when_the_full_label_is_ambiguous() {
    let address = "<https://example.test/>";
    let gap = address.len();
    let target = "https://example.test/";
    let grid = model(
        48,
        vec![
            json!({"w":false,"r":[
                [0,gap,address,[0,-1,-1],0],
                [gap+1,4,"more",[0,-1,-1],0]
            ]}),
            ascii("continuation", false, Some(0)),
        ],
        vec![json!(["tag", target])],
    );
    let fragments = grid.row_links(0);
    assert_eq!(fragments.len(), 2);
    assert_eq!(fragments[0].kind, fragments[1].kind);
    assert_eq!(fragments[0].label, address);
    let disclosure = grid.link_disclosure(&fragments[0]).unwrap();
    assert_eq!(disclosure.target, target);
    let warning = disclosure.warning.unwrap();
    assert_eq!(warning.reasons, [Incomplete]);
    assert_eq!(warning.label, format!("{address} morecontinuation"));
    assert_eq!(warning.address, None);
}

/// A URL can continue over soft wraps; underlining follows cell ranges, while
/// evidence includes the neighboring boundary whose edit could extend it.
#[test]
fn plain_urls_join_wraps_and_keep_unicode_cell_offsets() {
    let wrapped = model(
        14,
        vec![
            ascii("https://exampl", true, None),
            ascii("e.test/path", false, None),
        ],
        vec![],
    );
    let link = wrapped.link_at(Position { line: 1, col: 5 }).unwrap();
    assert_eq!(link.uri, "https://example.test/path");
    assert_eq!(link.kind, LinkKind::Text);
    assert_eq!(link.ranges.len(), 2);
    assert_eq!(link.ranges[0].start, Position { line: 0, col: 0 });
    assert_eq!(link.ranges[1].end, Position { line: 1, col: 11 });
    assert!(link.evidence.contains(&2));
    let wide = model(
        40,
        vec![
            json!({"w":false,"r":[[0,2,"😀",[0,-1,-1]],[3,22,"https://example.test/x",[0,-1,-1]]]}),
        ],
        vec![],
    );
    let link = wide.link_at(Position { line: 0, col: 3 }).unwrap();
    assert_eq!(link.ranges[0].start.col, 3);
    assert!(!link.covers(Position { line: 0, col: 2 }));
}

/// A styled pad can paint the final column without breaking a printed URL or becoming part of its hit range.
/// A styled real space in the same column still ends the URL.
#[test]
fn a_styled_padding_run_is_outside_the_printed_url() {
    for padding in [false, true] {
        let model = model(
            16,
            vec![
                json!({"w":true,"p":padding,"r":[[0,15,"https://a.test/",[0,-1,-1]],[15,1," ",[0,-1,2]]]}),
                json!({"w":false,"r":[[0,2,"界",[0,-1,-1]]]}),
            ],
            vec![],
        );
        let link = model.link_at(Position { line: 0, col: 1 }).unwrap();
        assert_eq!(
            link.uri,
            if padding {
                "https://a.test/界"
            } else {
                "https://a.test/"
            }
        );
        assert_eq!(link.ranges[0].end.col, 15);
        assert_eq!(link.ranges.len(), if padding { 2 } else { 1 });
        assert!(model.link_at(Position { line: 0, col: 15 }).is_none());
    }
}

/// URL lookup must cross a padded middle row both when searching forward for the end and backward for the start.
/// The same cells with a real space stop the URL at that row.
#[test]
fn the_url_search_crosses_a_padded_middle_row() {
    for padding in [false, true] {
        let model = model(
            21,
            vec![
                ascii("https://example.test/", true, None),
                json!({"w":true,"p":padding,"r":[
                    [0,2,"界",[0,-1,-1]],[2,18,"abcdefghijklmnopqr",[0,-1,-1]],[20,1," ",[0,-1,2]]
                ]}),
                json!({"w":false,"r":[[0,2,"界",[0,-1,-1]],[2,4,"more",[0,-1,-1]]]}),
            ],
            vec![],
        );
        let start = model.link_at(Position { line: 0, col: 1 }).unwrap();
        let end = model.link_at(Position { line: 2, col: 3 });
        if padding {
            assert_eq!(start.uri, "https://example.test/界abcdefghijklmnopqr界more");
            assert_eq!(start.ranges.len(), 3);
            assert_eq!(start.ranges[1].end.col, 20);
            assert_eq!(end, Some(start));
        } else {
            assert_eq!(start.uri, "https://example.test/界abcdefghijklmnopqr");
            assert_eq!(start.ranges.len(), 2);
            assert!(end.is_none());
        }
    }
}

/// The previous row's padding stays outside copy, words, and links even when an erase shortens the next glyph.
/// Generated OSC identities continue from the last text run so the link keeps its whole visible label.
#[test]
fn osc_continuation_looks_past_the_styled_padding_run() {
    for (next, width) in [
        (json!({"w":false,"r":[[0,2,"界",[0,-1,2],1]]}), 2),
        (
            json!({"w":false,"r":[[0,1,"界",[0,-1,2],1],[1,1," ",[0,-1,1]]]}),
            1,
        ),
    ] {
        let model = model(
            4,
            vec![
                json!({"w":true,"p":true,"r":[[0,3,"abc",[0,-1,2],0],[3,1," ",[0,-1,2]]]}),
                next,
            ],
            vec![
                json!(["1_alacritty", "https://example.test"]),
                json!(["2_alacritty", "https://example.test"]),
            ],
        );
        let above = model.link_at(Position { line: 0, col: 2 }).unwrap();
        let below = model.link_at(Position { line: 1, col: 0 }).unwrap();
        assert_eq!(
            above.kind,
            LinkKind::Osc {
                id: "1_alacritty".into()
            }
        );
        assert_eq!(below.kind, above.kind);
        assert_eq!(above.ranges[0].end.col, 3);
        assert_eq!(
            below.ranges[0].end,
            Position {
                line: 1,
                col: width
            }
        );
        assert!(model.link_at(Position { line: 0, col: 3 }).is_none());
        assert!(
            model
                .link_at(Position {
                    line: 1,
                    col: width
                })
                .is_none()
        );
        let word = model.word_range(Position { line: 0, col: 2 }).unwrap();
        assert_eq!(
            word.end,
            Position {
                line: 1,
                col: width
            }
        );
        assert_eq!(
            model.selection_text(word.start, word.end, word.mode),
            "abc界"
        );
    }
}

/// The wire allows a link run to cover the last column. Its padded cell must not reappear as a false mismatch
/// warning when disclosure reconstructs the visible label across the wrap, and the wide-wrap pad must not stop the seam
/// from being soft. Spec: `https://a.test/` plus a padded cell, soft-wrapped into `界`, under a generated hyperlink to
/// `https://a.test/界` is quiet from both rows; to another site it warns, quoting the joined `https://a.test/界`.
#[test]
fn osc_disclosure_excludes_padding_inside_a_link_run() {
    for (target, warns) in [("https://a.test/界", false), ("https://b.test/", true)] {
        let model = model(
            16,
            vec![
                json!({"w":true,"p":true,"r":[[0,16,"https://a.test/ ",[0,-1,-1],0]]}),
                json!({"w":false,"r":[[0,2,"界",[0,-1,-1],1]]}),
            ],
            vec![
                json!(["1_alacritty", target]),
                json!(["2_alacritty", target]),
            ],
        );
        let first = model.link_at(Position { line: 0, col: 1 }).unwrap();
        let last = model.link_at(Position { line: 1, col: 0 }).unwrap();
        assert_eq!(first.kind, last.kind);
        assert_eq!(first.label, "https://a.test/");
        assert!(model.link_at(Position { line: 0, col: 15 }).is_none());
        let shown = warning(&model, 0, 1);
        assert_eq!(shown, warning(&model, 1, 0));
        if warns {
            let shown = shown.unwrap();
            assert_eq!(shown.label, "https://a.test/界", "the soft seam joins");
            assert_eq!(shown.address.as_deref(), Some("https://a.test/界"));
        } else {
            assert_eq!(shown, None);
        }
    }
}

/// A hidden suffix cannot extend the address opened by a click on the visible prefix.
#[test]
fn a_printed_link_ends_before_a_concealed_suffix() {
    let model = model(
        32,
        vec![json!({"w":false,"r":[
            [0,20,"https://example.test",[0,-1,-1]],
            [20,7,".hidden",[style_flags::HIDDEN,-1,-1]]
        ]})],
        vec![],
    );
    let link = model.link_at(Position { line: 0, col: 8 }).unwrap();
    assert_eq!(link.kind, LinkKind::Text);
    assert_eq!(link.uri, "https://example.test");
    assert_eq!(link.ranges[0].end, Position { line: 0, col: 20 });
    assert!(model.link_at(Position { line: 0, col: 20 }).is_none());
}

/// A concealed run across a soft wrap breaks the link without changing what copying that range yields.
#[test]
fn concealed_text_breaks_a_wrapped_link_but_remains_in_selection() {
    let model = model(
        20,
        vec![
            ascii("https://example.test", true, None),
            json!({"w":false,"r":[[0,5,"/mask",[style_flags::HIDDEN,-1,-1]],[5,5,"/tail",[0,-1,-1]]]}),
        ],
        vec![],
    );
    let word = model.word_range(Position { line: 0, col: 8 }).unwrap();
    assert_eq!(word.mode, SelectionMode::Linear);
    assert_eq!(
        model.selection_text(word.start, word.end, word.mode),
        "https://example.test/mask/tail",
    );
    let link = model.link_at(Position { line: 0, col: 8 }).unwrap();
    assert_eq!(link.uri, "https://example.test");
    assert_eq!(link.ranges.len(), 1);
    assert!(model.link_at(Position { line: 1, col: 5 }).is_none());
}

/// The wire value of a direct (truecolor) color: `0x1000000 + 0xRRGGBB` (`farhelm_proto::grid::Color`).
fn direct(rgb: u32) -> i64 {
    0x100_0000 + i64::from(rgb)
}

/// A program can hide text without the conceal attribute by drawing it too close to its background to read.
/// A plain click opens a printed URL without showing it first, so hidden text must end the URL exactly as concealed
/// text does: a background-colored `.hidden` after the readable host must not quietly become part of it. The dimmest
/// ordinary styles are the other side of the same rule. Bright black (SGR 90), dim default text and dim bright black
/// are how programs print secondary text, and a URL in them must keep linking whole.
#[test]
fn a_printed_link_ends_before_hidden_text() {
    let background = common::palette().background.hex();
    for (name, style, hidden) in [
        (
            "ANSI black on the default background",
            json!([0, 0, -1]),
            true,
        ),
        (
            "text in its own explicit background color",
            json!([0, direct(0x336699), direct(0x336699)]),
            true,
        ),
        (
            "truecolor equal to the default background",
            json!([0, direct(background), -1]),
            true,
        ),
        ("bright black (SGR 90)", json!([0, 8, -1]), false),
        ("dim default text", json!([style_flags::DIM, -1, -1]), false),
        ("dim bright black", json!([style_flags::DIM, 8, -1]), false),
    ] {
        let model = model(
            32,
            vec![json!({"w":false,"r":[
                [0,20,"https://example.test",[0,-1,-1]],
                [20,7,".hidden",style]
            ]})],
            vec![],
        );
        // Premise: the fixture palette puts each style on the intended side of the threshold.
        let suffix = &model.screen()[0].runs[1];
        assert_eq!(
            colors::unseen(model.palette(), suffix, model.state()),
            hidden,
            "{name}"
        );
        let link = model.link_at(Position { line: 0, col: 8 }).unwrap();
        assert_eq!(link.kind, LinkKind::Text, "{name}");
        if hidden {
            assert_eq!(link.uri, "https://example.test", "{name}");
            assert_eq!(link.ranges[0].end, Position { line: 0, col: 20 }, "{name}");
            assert!(
                model.link_at(Position { line: 0, col: 20 }).is_none(),
                "{name}"
            );
        } else {
            assert_eq!(link.uri, "https://example.test.hidden", "{name}");
            assert_eq!(
                model.link_at(Position { line: 0, col: 22 }),
                Some(link),
                "{name}"
            );
        }
    }
}

/// Hidden text at the start of a soft-wrapped continuation ends the URL at the seam, as concealed text does
/// (`concealed_text_breaks_a_wrapped_link_but_remains_in_selection`). Only link detection treats it as blank:
/// selecting and copying the word still yields everything the program wrote, hidden part included.
#[test]
fn hidden_text_breaks_a_wrapped_link_but_remains_in_selection() {
    let model = model(
        20,
        vec![
            ascii("https://example.test", true, None),
            json!({"w":false,"r":[[0,5,"/mask",[0,0,-1]],[5,5,"/tail",[0,-1,-1]]]}),
        ],
        vec![],
    );
    let mask = &model.screen()[1].runs[0];
    assert!(colors::unseen(model.palette(), mask, model.state()));
    let word = model.word_range(Position { line: 0, col: 8 }).unwrap();
    assert_eq!(
        model.selection_text(word.start, word.end, word.mode),
        "https://example.test/mask/tail",
    );
    let link = model.link_at(Position { line: 0, col: 8 }).unwrap();
    assert_eq!(link.uri, "https://example.test");
    assert_eq!(link.ranges.len(), 1);
    assert!(model.link_at(Position { line: 1, col: 5 }).is_none());
}

/// Hidden is judged against the program's current colors, not the theme's. A program that sets its default foreground
/// (OSC 10) to the background color hides every default-colored cell while the text it colors explicitly stays
/// readable, so a URL whose visible prefix is explicitly colored ends where that prefix does. Setting the default
/// back makes the whole URL readable, and linked, again.
#[test]
fn a_default_foreground_set_to_the_background_hides_default_colored_text() {
    let mut model = model(
        32,
        vec![json!({"w":false,"r":[
            [0,20,"https://example.test",[0,7,-1]],
            [20,7,".hidden",[0,-1,-1]]
        ]})],
        vec![],
    );
    let state = |default_fg: Option<i64>| json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,"default_fg":default_fg,"default_bg":null,"history_trimmed":false,"exit":null});
    let at = Position { line: 0, col: 8 };
    assert_eq!(
        model.link_at(at).unwrap().uri,
        "https://example.test.hidden"
    );

    let background = direct(model.palette().background.hex());
    apply_message(&mut model, state(Some(background)));
    // Premise: the program's new default hides the default-colored suffix and leaves the explicit prefix readable.
    let runs = &model.screen()[0].runs;
    assert!(!colors::unseen(model.palette(), &runs[0], model.state()));
    assert!(colors::unseen(model.palette(), &runs[1], model.state()));
    assert_eq!(model.link_at(at).unwrap().uri, "https://example.test");
    assert!(model.link_at(Position { line: 0, col: 20 }).is_none());

    apply_message(&mut model, state(None));
    assert_eq!(
        model.link_at(at).unwrap().uri,
        "https://example.test.hidden"
    );
}

/// The two ways a program puts text out of sight, as wire styles: the conceal attribute, and ANSI black on the default
/// background, which the fixture palette draws below the hidden threshold. A program hyperlink treats them the same, so
/// each conceal test below runs with both; `assert_out_of_sight` checks the second one's premise.
fn out_of_sight() -> [(&'static str, Value); 2] {
    [
        ("concealed", json!([style_flags::HIDDEN, -1, -1])),
        ("background-colored", json!([0, 0, -1])),
    ]
}

/// Premise for a test using [`out_of_sight`]: the run at `(line, index)` is unseen under the fixture palette.
fn assert_out_of_sight(grid: &GridModel, line: usize, index: usize, way: &str) {
    let run = &grid.screen()[line].runs[index];
    assert!(
        colors::unseen(grid.palette(), run, grid.state()),
        "{way}: {run:?}"
    );
}

/// A style-only update changes which label a user sees. It can expose a real destination mismatch or remove a false
/// one, and the text it hides is itself a reason to doubt the link. The link under the pointer changes identity and its
/// hover range ends at the visible label. Spec: for each way of putting text out of sight, `https://example.test` with
/// `.other.test` linked to the joined address is quiet; with the suffix out of sight it mismatches on what's left and
/// warns hidden. `menu` with `://example.test/x` linked to another site warns; with the suffix out of sight the label
/// `menu` mentions no address, and only hidden remains. Updated from conceal clearing the second warning: hidden text is
/// now a reason of its own.
#[test]
fn concealing_a_program_link_suffix_rechecks_the_visible_label() {
    for (way, style) in out_of_sight() {
        for (prefix, suffix, target, before_reasons, after_reasons) in [
            (
                "https://example.test",
                ".other.test",
                "https://example.test.other.test",
                vec![],
                vec![Mismatch, Hidden],
            ),
            (
                "menu",
                "://example.test/x",
                "https://other.test/x",
                vec![Mismatch],
                vec![Hidden],
            ),
        ] {
            let split = prefix.len() as u16;
            let row = |style: &Value| {
                json!({"w":false,"r":[
                    [0,split,prefix,[0,-1,-1],0],
                    [split,suffix.len(),suffix,style,0]
                ]})
            };
            let links = vec![json!(["tag", target])];
            let mut grid = model(48, vec![row(&json!([0, -1, -1]))], links.clone());
            let start = Position { line: 0, col: 0 };
            let boundary = Position {
                line: 0,
                col: split,
            };
            let before = grid.link_at(start).unwrap();
            let full_label = format!("{prefix}{suffix}");
            assert_eq!(before.label, full_label);
            assert_eq!(grid.link_at(boundary), Some(before.clone()));
            assert_eq!(reasons(&warning(&grid, 0, 0)), before_reasons, "{way}");

            apply_message(
                &mut grid,
                json!({"t":"rows","rows":[[0,row(&style)]],"links":links}),
            );
            apply_message(
                &mut grid,
                json!({"t":"frame","rev":2,"cols":48,"rows":1,"cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":false}}),
            );
            assert_out_of_sight(&grid, 0, 1, way);
            let after = grid.link_at(start).unwrap();
            assert_eq!(after.kind, before.kind);
            assert_eq!(after.uri, before.uri);
            assert_eq!(after.label, prefix);
            assert_eq!(after.ranges[0].end, boundary);
            assert_ne!(after, before);
            assert_eq!(grid.link_at(boundary), None, "{way}");
            let shown = warning(&grid, 0, 0).unwrap();
            assert_eq!(shown.reasons, after_reasons, "{way}");
            assert_eq!(shown.label, prefix, "{way}");
        }
    }
}

/// A row that shows nothing of the hyperlink ends the visible label walk, and it is no evidence of hidden text: that is
/// how conceal always worked, and text too faint to read works the same (an accepted gap, SPEC.md). It stays in the
/// evidence boundary so revealing it can extend the link, and another visible row carrying the same tag beyond it stays
/// outside the comparison. Spec: for each way of putting text out of sight, `https://example.test/x` above an
/// out-of-sight row and `unrelated`, one explicit hyperlink to that address, is quiet from row 0.
#[test]
fn a_concealed_only_row_breaks_a_program_link_label() {
    for (way, style) in out_of_sight() {
        let grid = model(
            42,
            vec![
                ascii("https://example.test/x", false, Some(0)),
                json!({"w":false,"r":[[0,6,"hidden",style,0]]}),
                ascii("unrelated", false, Some(0)),
            ],
            vec![json!(["tag", "https://example.test/x"])],
        );
        assert_out_of_sight(&grid, 1, 0, way);
        let first = grid.link_at(Position { line: 0, col: 0 }).unwrap();
        let other = grid.link_at(Position { line: 2, col: 0 }).unwrap();
        assert_eq!(first.kind, other.kind);
        assert!(grid.row_links(1).is_empty(), "{way}");
        assert!(first.evidence.contains(&1));
        assert!(!first.evidence.contains(&2));
        assert_eq!(warning(&grid, 0, 0), None, "{way}");
    }
}

/// An out-of-sight gap splits the hit ranges even when the runs have one explicit tag. With a continuation on the next
/// row, the row's two stretches can't be put back into one text, so a visible stretch that mentions an address makes
/// the label incomplete. Spec: `https://example.test/x`, a two-cell gap and `more` on row 0, `continuation` on row 1,
/// one hyperlink to `https://example.test/x`: the two stretches split at the gap, and the first one's hover is
/// incomplete. A concealed gap of spaces hides nothing, so that is all; a gap with text in it, concealed or in the
/// background color, adds hidden. Updated from a warning that quoted the stretch as a mismatching claim.
#[test]
fn a_concealed_gap_splits_the_hit_ranges_of_an_explicit_program_link() {
    let address = "https://example.test/x";
    let split = address.len();
    let [(_, concealed), (_, faded)] = out_of_sight();
    for (gap, style, expected) in [
        ("  ", concealed.clone(), vec![Incomplete]),
        ("xx", concealed, vec![Incomplete, Hidden]),
        ("xx", faded, vec![Incomplete, Hidden]),
    ] {
        let grid = model(
            48,
            vec![
                json!({"w":false,"r":[
                    [0,split,address,[0,-1,-1],0],
                    [split,2,gap,style,0],
                    [split+2,4,"more",[0,-1,-1],0]
                ]}),
                ascii("continuation", false, Some(0)),
            ],
            vec![json!(["tag", address])],
        );
        assert_out_of_sight(&grid, 0, 1, gap);
        let links = grid.row_links(0);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].kind, links[1].kind);
        assert_eq!(links[0].label, address);
        assert_eq!(links[0].ranges[0].end.col, split as u16);
        assert_eq!(links[1].label, "more");
        assert_eq!(links[1].ranges[0].start.col, (split + 2) as u16);
        assert_eq!(
            grid.link_at(Position {
                line: 0,
                col: split as u16
            }),
            None
        );
        let shown = grid.link_disclosure(&links[0]).unwrap().warning.unwrap();
        assert_eq!(shown.reasons, expected, "{gap:?}");
        assert_eq!(shown.label, format!("{address} morecontinuation"));
    }
}

/// Revealing a separate stretch of the same tag can change the hovered row's warning while leaving the hovered stretch
/// itself untouched. When that happens, a display for the old link must be refreshed. Spec: for each way of putting text
/// out of sight, `https://example.test/`, a gap and an out-of-sight `more` on row 0, and `x` on row 1, one hyperlink to
/// `https://example.test/x`: the hidden `more` makes the hover warn hidden, and the gap it leaves keeps the rows from
/// joining into the destination, so it mismatches too. Once `more` is drawn normally, the hovered link compares
/// unequal to the one before, and its hover is incomplete instead: row 0 now shows the hyperlink in two stretches.
#[test]
fn revealing_a_second_stretch_invalidates_a_wrapped_link_disclosure() {
    for (way, style) in out_of_sight() {
        let prefix = "https://example.test/";
        let links = vec![json!(["tag", "https://example.test/x"])];
        let separated = prefix.len() + 1;
        let row = |style: &Value| {
            json!({"w":false,"r":[
                [0,prefix.len(),prefix,[0,-1,-1],0],
                [separated,4,"more",style,0]
            ]})
        };
        let mut grid = model(
            48,
            vec![row(&style), ascii("x", false, Some(0))],
            links.clone(),
        );
        assert_out_of_sight(&grid, 0, 1, way);
        let hovered = Position { line: 0, col: 0 };
        let before = grid.link_at(hovered).unwrap();
        assert_eq!(before.label, prefix);
        let shown = warning(&grid, 0, 0).unwrap();
        assert_eq!(shown.reasons, [Mismatch, Hidden], "{way}");
        assert_eq!(shown.label, format!("{prefix} x"), "{way}");

        apply_message(
            &mut grid,
            json!({"t":"rows","rows":[[0,row(&json!([0,-1,-1]))]],"links":links}),
        );
        apply_message(
            &mut grid,
            json!({"t":"frame","rev":2,"cols":48,"rows":2,"cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":false}}),
        );
        let after = grid.link_at(hovered).unwrap();
        assert_eq!(grid.row_links(0).len(), 2, "the second stretch is visible");
        assert_eq!(after.label, before.label);
        assert_eq!(after.ranges, before.ranges);
        assert_eq!(after.evidence, before.evidence);
        assert_eq!(after.uri, before.uri);
        assert_eq!(after.kind, before.kind);
        assert_ne!(after, before, "the display must refresh");
        let shown = warning(&grid, 0, 0).unwrap();
        assert_eq!(shown.reasons, [Incomplete], "{way}");
        assert_eq!(shown.label, format!("{prefix} morex"), "{way}");
    }
}

/// Anonymous tags sharing a destination only inherit each other's identity across visible cells. Putting either side
/// of a soft-wrap seam out of sight, by conceal or by color, keeps the second row's visible text from joining the
/// first.
#[test]
fn concealed_cells_break_generated_link_continuation_on_either_side_of_a_wrap() {
    for (way, hidden_style) in out_of_sight() {
        for hide_above in [false, true] {
            let style = |hide| {
                if hide {
                    hidden_style.clone()
                } else {
                    json!([0, -1, -1])
                }
            };
            let grid = model(
                8,
                vec![
                    json!({"w":true,"r":[[0,4,"head",style(false),0],[4,4,"seam",style(hide_above),1]]}),
                    json!({"w":false,"r":[[0,4,"seam",style(!hide_above),2],[4,4,"tail",style(false),3]]}),
                ],
                (1..=4)
                    .map(|id| json!([format!("{id}_alacritty"), "https://example.test/x"]))
                    .collect(),
            );
            let head = grid.link_at(Position { line: 0, col: 0 }).unwrap();
            let tail = grid.link_at(Position { line: 1, col: 4 }).unwrap();
            assert!(matches!(head.kind, LinkKind::Osc { ref id } if id == "1_alacritty"));
            let expected = if hide_above {
                "3_alacritty"
            } else {
                "4_alacritty"
            };
            assert!(
                matches!(tail.kind, LinkKind::Osc { ref id } if id == expected),
                "{way}"
            );
            let hidden = if hide_above {
                Position { line: 0, col: 4 }
            } else {
                Position { line: 1, col: 0 }
            };
            assert_eq!(grid.link_at(hidden), None, "{way}");
        }
    }
}

/// A printed address under a program's tag must never become a fallback target, including when the whole tagged label
/// is out of sight, by conceal or by color. The same visible address without a tag is a printed link.
#[test]
fn a_concealed_program_link_over_a_printed_address_has_no_fallback_link() {
    let label = "https://example.test/x";
    let position = Position { line: 0, col: 0 };
    let printed = model(26, vec![ascii(label, false, None)], vec![]);
    assert_eq!(printed.link_at(position).unwrap().kind, LinkKind::Text);
    for (way, style) in out_of_sight() {
        let tagged = model(
            26,
            vec![json!({"w":false,"r":[[0,label.len(),label,style,0]]})],
            vec![json!(["tag", "https://other.test/x"])],
        );
        assert_out_of_sight(&tagged, 0, 0, way);
        assert!(tagged.row_links(0).is_empty(), "{way}");
        assert_eq!(tagged.link_at(position), None, "{way}");
    }
}

/// Explicit OSC ownership defeats prose recognition even for an inert scheme;
/// otherwise a displayed URL could silently override the program's real target.
#[test]
fn osc_precedence_and_generated_continuation_identity() {
    let explicit = model(
        40,
        vec![ascii("https://example.test/x", false, Some(0))],
        vec![json!(["id", "file:///etc/passwd"])],
    );
    let links = explicit.row_links(0);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].uri, "file:///etc/passwd");
    assert!(web_target(&links[0].uri, false).is_none());
    let generated = model(
        8,
        vec![
            json!({"w":true,"r":[[0,4,"abcd",[0,-1,-1],0],[4,4,"efgh",[0,-1,-1],1]]}),
            ascii("ijkl", false, Some(2)),
            ascii("mnop", false, Some(2)),
        ],
        vec![
            json!(["1_alacritty", "https://example.test"]),
            json!(["2_alacritty", "https://example.test"]),
            json!(["3_alacritty", "https://example.test"]),
        ],
    );
    assert_eq!(generated.row_links(0).len(), 1);
    assert_eq!(
        generated.row_links(1)[0].kind,
        LinkKind::Osc {
            id: "1_alacritty".into()
        }
    );
    assert_eq!(
        generated.row_links(2)[0].kind,
        LinkKind::Osc {
            id: "3_alacritty".into()
        }
    );
}

/// The detector preserves the addon's case and punctuation boundaries rather
/// than treating every parser-accepted or address-looking string as a link.
#[test]
fn prose_detection_and_native_browser_scheme_boundary() {
    for (text, expected) in [
        ("HTTPS://EXAMPLE.TEST/x.", Some("HTTPS://EXAMPLE.TEST/x")),
        ("Https://example.test/x", None),
        ("http://127.1/x", None),
        ("https://example.test/x)", Some("https://example.test/x")),
        ("file:///tmp/x", None),
        ("https://example.test/😀", Some("https://example.test/😀")),
    ] {
        let width = text
            .chars()
            .map(|ch| if ch == '😀' { 2 } else { 1 })
            .sum::<u16>();
        let row = if text.ends_with('😀') {
            json!({"w":false,"r":[[0,21,"https://example.test/",[0,-1,-1]],[21,2,"😀",[0,-1,-1]]]})
        } else {
            ascii(text, false, None)
        };
        let model = model(width.max(40), vec![row], vec![]);
        assert_eq!(
            model.row_links(0).first().map(|link| link.uri.as_str()),
            expected,
            "{text}"
        );
    }
    for uri in [
        "javascript:alert(1)",
        "file:///tmp/x",
        "mailto:x@example.test",
        "//example.test",
        "https://",
        "https://exa\nmple.test/x",
    ] {
        assert!(web_target(uri, true).is_none(), "{uri}");
    }
    assert_eq!(
        web_target("https://example.test/", true)
            .unwrap()
            .host_str(),
        Some("example.test")
    );
}

/// The `grid` exceptions to the oracle's safety check (`tests/fixtures/links-oracle-exceptions.json`), keyed by grid
/// index, line and result index, after checking the whole file: every entry has a known kind, a key and a rule, and no
/// key appears twice. The `policy` entries are the link disclosure unit test's.
fn grid_exceptions() -> BTreeSet<(usize, u64, usize)> {
    let file: Value =
        serde_json::from_str(include_str!("fixtures/links-oracle-exceptions.json")).unwrap();
    let mut seen = BTreeSet::new();
    let mut grids = BTreeSet::new();
    for entry in file["exceptions"].as_array().unwrap() {
        let kind = entry["kind"].as_str().unwrap();
        assert!(
            matches!(kind, "policy" | "grid"),
            "unknown exception kind: {entry}"
        );
        assert!(
            entry["rule"].as_str().is_some_and(|rule| !rule.is_empty()),
            "an exception names the behavior that explains it: {entry}"
        );
        assert!(
            seen.insert((kind.to_owned(), entry["key"].to_string())),
            "duplicate exception: {entry}"
        );
        if kind == "grid" {
            let key = &entry["key"];
            grids.insert((
                key["grid"].as_u64().unwrap() as usize,
                key["line"].as_u64().unwrap(),
                key["result"].as_u64().unwrap() as usize,
            ));
        }
    }
    grids
}

/// Recognition and label joining are evaluated together against the old web client's recorded answers. Which cells
/// form which link (kind, target, ranges, evidence, identity) and the destination a hover names still bind exactly.
/// The warning binds in the safety direction only: where the old client warned, the hover must still warn, and where it
/// warned on a complete label for a mismatch, the label the hover shows, escaped, must be the text the old client
/// quoted. That last check keeps row joining honest: a reader that joined rows differently would show a different
/// label. Hard row breaks, repeated tags, ambiguous runs and budget exhaustion must not turn a forged address into a
/// quiet disclosure. A case where the answer legitimately differs is a named exception in the fixture.
#[test]
fn pinned_cell_links_and_label_join_oracle() {
    let oracle: Value = serde_json::from_str(include_str!("fixtures/links-oracle.json")).unwrap();
    let exceptions = grid_exceptions();
    let mut used = BTreeSet::new();
    let mut failures = Vec::new();
    for (index, case) in oracle["grids"].as_array().unwrap().iter().enumerate() {
        let model = model(
            case["cols"].as_u64().unwrap() as u16,
            case["rows"].as_array().unwrap().clone(),
            case["links"].as_array().unwrap().clone(),
        );
        for question in case["questions"].as_array().unwrap() {
            let line = question["line"].as_u64().unwrap();
            let links = model.row_links(line);
            let expected = question["expected"].as_array().unwrap();
            assert_eq!(links.len(), expected.len(), "grid {index}, row {line}");
            for (result, (link, expected)) in links.iter().zip(expected).enumerate() {
                let disclosure = model.link_disclosure(link);
                let mut actual = json!({
                    "kind": match &link.kind { LinkKind::Osc { .. } => "osc", LinkKind::Text => "text", LinkKind::Path => "path" },
                    "uri": link.uri,
                    "ranges": link.ranges.iter().map(|range| json!({
                        "start": {"abs": range.start.line, "col": range.start.col},
                        "end": {"abs": range.end.line, "col": range.end.col},
                    })).collect::<Vec<_>>(),
                    "evidence": link.evidence,
                    "disclosure": disclosure.as_ref().map(|display| json!({
                        "host": display.host, "target": display.target,
                    })),
                });
                if let LinkKind::Osc { id } = &link.kind {
                    actual["id"] = json!(id);
                }
                let mut exact = expected.clone();
                let claimed = exact["disclosure"]
                    .as_object_mut()
                    .and_then(|display| display.remove("claimed"));
                assert_eq!(actual, exact, "grid {index}, row {line}, result {result}");
                let Some(claimed) = claimed.as_ref().and_then(Value::as_str) else {
                    continue;
                };
                let warning = disclosure.and_then(|display| display.warning);
                let holds = warning.as_ref().is_some_and(|warning| {
                    !warning.reasons.contains(&Mismatch)
                        || warning.reasons.contains(&Incomplete)
                        || farhelm_proto::text::escaped(&warning.label) == claimed
                });
                let key = (index, line, result);
                if exceptions.contains(&key) {
                    assert!(
                        !holds,
                        "an exception for a case that holds: grid {index}, row {line}, result {result}"
                    );
                    used.insert(key);
                } else if !holds {
                    failures.push((key, claimed.to_owned(), warning));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    let unused: Vec<_> = exceptions.difference(&used).collect();
    assert!(unused.is_empty(), "unused grid exceptions: {unused:?}");
}

/// A tunnel is offered only for a remote host's own loopback, so the offer must neither miss what a local dev server
/// prints (`localhost`, `127.1`, a server bound to all addresses) nor catch a LAN address, which the browser on the Mac
/// can reach by itself and which would then be tunneled to the wrong place. Specified by the old app's table
/// (`terminal-tunnels.test.js` at reference commit 24bc0580): these hosts give the link's port, the scheme's default
/// port when none is written, and everything else, including malformed links and port 0, gives none.
#[test]
fn tunnel_ports_are_offered_only_for_normalized_loopback_and_wildcard_hosts() {
    for host in [
        "localhost",
        "sub.localhost.",
        "127.0.0.1",
        "127.255.1.2",
        "127.1",
        "0x7f000001",
        "0.0.0.0",
        "[::1]",
        "[::]",
    ] {
        assert_eq!(
            tunnel_port(&format!("http://{host}:5173/path?q=1#part")),
            Some(5173),
            "{host}"
        );
    }
    assert_eq!(tunnel_port("HTTPS://LOCALHOST/path"), Some(443));
    assert_eq!(tunnel_port("http://localhost/path"), Some(80));
    for host in [
        "192.168.1.1",
        "10.0.0.1",
        "0.1.2.3",
        "server",
        "localhost.example.test",
        "example.local",
        "[::ffff:127.0.0.1]",
        "[fe80::1]",
        "example.test",
    ] {
        assert_eq!(tunnel_port(&format!("http://{host}:5173/")), None, "{host}");
    }
    let long = format!("http://localhost/{}", "x".repeat(4096));
    for uri in [
        "http://localhost:0/",
        "http://localhost:000/",
        "http://localhost:65536/",
        "http://localhost:bad/",
        "http://",
        "//localhost/",
        "http:localhost",
        "file://localhost/x",
        "ftp://localhost/",
        " http://localhost/",
        "http://localhost/\npath",
        "http://localhost/\u{feff}",
        "http://localhost/\u{7f}",
        long.as_str(),
    ] {
        assert_eq!(tunnel_port(uri), None, "{uri:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Labels over several rows
// ---------------------------------------------------------------------------------------------------------------------

/// Rows for one explicit hyperlink (link 0) whose text is `lines`: each line starts a new row with a hard break before
/// it, and a line longer than `cols` soft-wraps over as many full rows as it needs.
fn logical_lines(cols: u16, lines: &[&str]) -> Vec<Value> {
    let width = usize::from(cols);
    let mut rows = Vec::new();
    for line in lines {
        let pieces: Vec<&str> = line
            .as_bytes()
            .chunks(width)
            .map(|chunk| std::str::from_utf8(chunk).unwrap())
            .collect();
        for (index, piece) in pieces.iter().enumerate() {
            rows.push(ascii(piece, index + 1 < pieces.len(), Some(0)));
        }
    }
    rows
}

/// Every row's hover for a hyperlink whose text starts at column 0 of each of the model's rows, after asserting they
/// are all the same: the label is read whole from whichever row the pointer rests on, so a user can't get a quieter
/// answer by moving to another row.
fn same_hover_on_every_row(grid: &GridModel, rows: usize) -> Option<LinkWarning> {
    let first = warning(grid, 0, 0);
    for line in 1..rows as u64 {
        assert_eq!(warning(grid, line, 0), first, "row {line}");
    }
    first
}

/// How a hyperlink continued over rows is read, row by row of SPEC.md's examples: a soft wrap always joins; a row that
/// starts with an address (after punctuation only) is a new item, so the break before it separates like a space; any
/// other break joins, so a host or path a program broke over rows is judged whole. A label that mismatches read that
/// way warns only if it also mismatches with every break joined. These are the cases that decided the rule: each one
/// is either a spoof that must warn or an honest layout that must not, and the past regressions in this area came from
/// getting exactly these joins wrong. Spec: each case, hovered on every row, shows the same hover; the quiet ones show
/// none, and the loud ones warn of a mismatch naming the given address and showing the given label.
#[test]
fn labels_over_several_rows_join_and_separate_at_their_seams() {
    let evil = "https://evil.test/";
    for (cols, lines, target, expected) in [
        (
            40,
            &["https://github.com/a", "/b"][..],
            "https://github.com/a/b",
            None,
        ),
        (
            40,
            &["github.", "com/login"],
            evil,
            Some(("github.com/login", "github.com/login")),
        ),
        (
            40,
            &["Visit", "https://github.com/"],
            evil,
            Some(("https://github.com/", "Visit https://github.com/")),
        ),
        // `https://` fills its row, so the address soft-wraps onto the next ones.
        (
            8,
            &["Visit", "https://github.com/"],
            "https://github.com/",
            None,
        ),
        (
            40,
            &["Preview", "https://github.com/"],
            "https://github.com/",
            None,
        ),
        (
            40,
            &["Preview", "1.https://github.com/"],
            "https://github.com/",
            None,
        ),
        (
            40,
            &["[https://evil.test/]", "https://github.com/"],
            evil,
            Some((
                "https://github.com/",
                "[https://evil.test/] https://github.com/",
            )),
        ),
        (
            40,
            &["<https://example.test/>", "\"https://other.test/\""],
            "https://example.test/",
            Some((
                "https://other.test/",
                "<https://example.test/> \"https://other.test/\"",
            )),
        ),
        (
            40,
            &["https://github.com/", "https://github.com/"],
            "https://github.com/",
            None,
        ),
        (
            40,
            &[
                "Read the guide at",
                "https://docs.example.com/a/long/pa",
                "th/here",
            ],
            "https://docs.example.com/a/long/path/here",
            None,
        ),
        // A host that soft-wraps after a separating break is still one address.
        (
            10,
            &["See", "example.test/x"],
            "https://example.test/x",
            None,
        ),
        (
            10,
            &["See", "example.test/x"],
            evil,
            Some(("example.test/x", "See example.test/x")),
        ),
    ] {
        let rows = logical_lines(cols, lines);
        let count = rows.len();
        let grid = model(cols, rows, vec![json!(["tag", target])]);
        let shown = same_hover_on_every_row(&grid, count);
        match expected {
            None => assert_eq!(shown, None, "{lines:?} -> {target}"),
            Some((address, label)) => {
                let shown = shown.unwrap_or_else(|| panic!("{lines:?} -> {target} warns"));
                assert_eq!(shown.reasons, [Mismatch], "{lines:?}");
                assert_eq!(shown.address.as_deref(), Some(address), "{lines:?}");
                assert_eq!(shown.label, label, "{lines:?}");
            }
        }
    }
}

/// The ordinary hover for a label too long to read whole: incomplete, with no text, so no part of an unread label is
/// ever presented as if it were the whole.
fn over_budget() -> Option<LinkWarning> {
    Some(LinkWarning {
        reasons: vec![Incomplete],
        label: String::new(),
        address: None,
        address_parsed: None,
        more: 0,
    })
}

/// A label longer than the budget can't be checked, and must say so from every row: if it were read whole from one end
/// and in part from the other, the quieter row would be the one to click. Spec: one explicit hyperlink over 129 rows
/// of honest text is incomplete with no label when hovered at its first, middle and last row; over 128 rows, the
/// budget's edge, it is read whole and quiet.
#[test]
fn an_explicit_label_over_the_row_budget_is_incomplete_from_every_row() {
    for (count, expected) in [(129, over_budget()), (128, None)] {
        let rows = (0..count).map(|_| ascii("abcd", false, Some(0))).collect();
        let grid = model(8, rows, vec![json!(["tag", "https://example.test/"])]);
        for line in [0, count / 2, count - 1] {
            assert_eq!(
                warning(&grid, line, 0),
                expected,
                "{count} rows, row {line}"
            );
        }
    }
}

/// A program that opens a fresh anonymous hyperlink on every row, each continuing the last across a soft wrap, makes
/// one long link out of many generated ids. Identity is resolved at most 129 rows back, so from the last row the root
/// of a chain longer than that is a different id than from the first: the label reader resolves the hovered row first
/// and keeps one lookup for the whole walk, which reaches past the budget from wherever the pointer is. Spec: a chain of
/// 140 such rows is incomplete with no label hovered at its first, middle and last row (whose identities differ), and a
/// chain of 128 rows is one link and quiet from its first and last row.
#[test]
fn a_generated_chain_over_the_row_budget_is_incomplete_from_every_row() {
    let chain = |count: u64| {
        let rows = (0..count)
            .map(|index| json!({"w": index + 1 < count, "r": [[0, 4, "abcd", [0, -1, -1], index]]}))
            .collect();
        let links = (0..count)
            .map(|index| json!([format!("{}_alacritty", index + 1), "https://example.test/"]))
            .collect();
        model(4, rows, links)
    };
    let grid = chain(140);
    let first = grid.link_at(Position { line: 0, col: 0 }).unwrap();
    let last = grid.link_at(Position { line: 139, col: 0 }).unwrap();
    assert_ne!(
        first.kind, last.kind,
        "the premise: the chain is longer than one lookup"
    );
    for line in [0, 70, 139] {
        assert_eq!(warning(&grid, line, 0), over_budget(), "row {line}");
    }
    let grid = chain(128);
    let first = grid.link_at(Position { line: 0, col: 0 }).unwrap();
    let last = grid.link_at(Position { line: 127, col: 0 }).unwrap();
    assert_eq!(first.kind, last.kind, "the premise: one link");
    for line in [0, 127] {
        assert_eq!(warning(&grid, line, 0), None, "row {line}");
    }
}

/// A row that shows a multi-row hyperlink in more than one stretch can't be put back into one text, so the stretches
/// are scanned one by one and any address among them makes the whole label incomplete, even one that names the
/// destination: the gap is where the rest of an address could hide. Spec: `heading`, then `one` and `two` with a gap
/// between them, then `https://example.test/`, one hyperlink to that address, hovered on each of its four stretches,
/// shows the same incomplete hover with every piece in the label (the break before `one` joins and the one before the
/// address separates, as in a complete label); the same rows without the address are quiet.
#[test]
fn a_row_with_several_stretches_makes_a_multi_row_label_incomplete() {
    let rows = |last: &str| {
        vec![
            ascii("heading", false, Some(0)),
            json!({"w":false,"r":[[0,3,"one",[0,-1,-1],0],[4,3,"two",[0,-1,-1],0]]}),
            ascii(last, false, Some(0)),
        ]
    };
    let target = vec![json!(["tag", "https://example.test/"])];
    let grid = model(30, rows("https://example.test/"), target.clone());
    let shown = warning(&grid, 0, 0);
    for (line, col) in [(1, 0), (1, 4), (2, 0)] {
        assert_eq!(warning(&grid, line, col), shown, "row {line}, column {col}");
    }
    let shown = shown.unwrap();
    assert_eq!(shown.reasons, [Incomplete]);
    assert_eq!(shown.label, "headingone two https://example.test/");
    let grid = model(30, rows("the end"), target);
    assert_eq!(warning(&grid, 1, 4), None);
}

/// A hyperlink on one row only, shown in several stretches, is several labels, as the old client had it: a program
/// listing two links to one place under one tag isn't hiding anything between them. Spec: `https://a.test/`, a gap and
/// `https://b.test/` on one row, one hyperlink to `https://a.test/`: the first stretch is quiet and the second warns of a
/// mismatch, naming `https://b.test/` and showing only its own text.
#[test]
fn stretches_of_a_one_row_label_are_judged_on_their_own() {
    let grid = model(
        40,
        vec![json!({"w":false,"r":[
            [0,15,"https://a.test/",[0,-1,-1],0],
            [16,15,"https://b.test/",[0,-1,-1],0]
        ]})],
        vec![json!(["tag", "https://a.test/"])],
    );
    assert_eq!(warning(&grid, 0, 0), None);
    let shown = warning(&grid, 0, 16).unwrap();
    assert_eq!(shown.reasons, [Mismatch]);
    assert_eq!(shown.address.as_deref(), Some("https://b.test/"));
    assert_eq!(shown.label, "https://b.test/");
}

/// The hover names why an address doesn't match, because look-alike characters and login details before a host need
/// a different explanation than a plain other site. Spec: to `https://example.com/x`, `https://other.test/x` is a
/// mismatch, `https://ｅxample.com/x` (a fullwidth letter) is disguised and `example.com@evil.test/x` is credentials,
/// each quoting its address as written and its parsed form; `https://exa<U+FEFF>mple.com/x` is disguised, and the
/// invisible character stays in the quoted address for the display to escape, though the URL parser drops it.
#[test]
fn each_address_reason_reaches_the_hover() {
    // Wire runs: printable ASCII flows one byte per cell; any other character is a cell of its own, with a zero-width
    // character riding on the cell before it.
    let plain = |label: &str| json!([[0, label.len(), label, [0, -1, -1], 0]]);
    for (label, runs, reason, parsed) in [
        (
            "https://other.test/x",
            plain("https://other.test/x"),
            Mismatch,
            Some("https://other.test/x"),
        ),
        (
            "https://ｅxample.com/x",
            json!([
                [0, 8, "https://", [0, -1, -1], 0],
                [8, 2, "ｅ", [0, -1, -1], 0],
                [10, 12, "xample.com/x", [0, -1, -1], 0]
            ]),
            Disguised,
            Some("https://example.com/x"),
        ),
        (
            "example.com@evil.test/x",
            plain("example.com@evil.test/x"),
            Credentials,
            Some("https://example.com@evil.test/x"),
        ),
        (
            "https://exa\u{feff}mple.com/x",
            json!([
                [0, 10, "https://ex", [0, -1, -1], 0],
                [10, 1, "a\u{feff}", [0, -1, -1], 0],
                [11, 10, "mple.com/x", [0, -1, -1], 0]
            ]),
            Disguised,
            Some("https://example.com/x"),
        ),
    ] {
        let grid = model(
            40,
            vec![json!({"w":false,"r":runs})],
            vec![json!(["tag", "https://example.com/x"])],
        );
        let shown = warning(&grid, 0, 0).unwrap();
        assert_eq!(shown.reasons, [reason], "{label}");
        assert_eq!(shown.address.as_deref(), Some(label), "{label}");
        assert_eq!(shown.address_parsed.as_deref(), parsed, "{label}");
        assert_eq!(shown.more, 0);
    }
}

/// The destination is what the user decides on, and a long one can hide the part that matters anywhere in it, the
/// omitted middle included: shortening it, as the old client did past 300 characters, let a spoof put the real place
/// where nobody could read it. Spec: a hyperlink to a destination of almost 2,000 characters discloses all of it, and
/// its host whole, both in the ordinary display (labelled `click here`) and in a warning (labelled with another site).
#[test]
fn the_hover_discloses_a_long_destination_in_full() {
    let target = format!("https://example.test/{}", "a".repeat(1900));
    for (label, warns) in [("click here", false), ("https://other.test/", true)] {
        let grid = model(
            40,
            vec![ascii(label, false, Some(0))],
            vec![json!(["tag", target])],
        );
        let link = grid.link_at(Position { line: 0, col: 0 }).unwrap();
        let disclosure = grid.link_disclosure(&link).unwrap();
        assert_eq!(disclosure.target, target, "{label}");
        assert_eq!(disclosure.host, "example.test", "{label}");
        assert_eq!(disclosure.warning.is_some(), warns, "{label}");
    }
}

/// The hover marks where the destination differs from the address the text mentions, so the user sees at a glance
/// which part of the real destination the text lied about. The mark only makes sense for a plain other address: a
/// disguised one reads as the destination's own place once folded, and login details put a host the parsed form
/// doesn't name as its host, so marking those would point at the wrong part. Spec: against its destination, a label
/// mentioning another host marks that host in the destination; another path marks the path's differing middle; a
/// disguised address, login details before a host, an address that doesn't parse, an address longer than the
/// destination by a path, and the ordinary display mark nothing.
#[test]
fn only_a_plain_mismatch_marks_where_the_destination_differs() {
    let plain = |label: &str| json!([[0, label.len(), label, [0, -1, -1], 0]]);
    for (runs, target, reason, marked) in [
        (
            plain("https://github.com/login"),
            "https://evil.test/login",
            Some(Mismatch),
            Some("evil.test"),
        ),
        (
            plain("https://example.test/docs/guide"),
            "https://example.test/docs/admin",
            Some(Mismatch),
            Some("admin"),
        ),
        (
            json!([
                [0, 8, "https://", [0, -1, -1], 0],
                [8, 2, "ｅ", [0, -1, -1], 0],
                [10, 12, "xample.com/x", [0, -1, -1], 0]
            ]),
            "https://evil.test/x",
            Some(Disguised),
            None,
        ),
        (
            plain("example.com@evil.test/x"),
            "https://evil.test/x",
            Some(Credentials),
            None,
        ),
        (
            plain("https://example.test:bogus/"),
            "https://example.test/",
            Some(Mismatch),
            None,
        ),
        (
            plain("https://example.test/a/b"),
            "https://example.test/a",
            Some(Mismatch),
            None,
        ),
        (plain("click here"), "https://example.test/", None, None),
    ] {
        let grid = model(
            40,
            vec![json!({"w":false,"r":runs})],
            vec![json!(["tag", target])],
        );
        let link = grid.link_at(Position { line: 0, col: 0 }).unwrap();
        let disclosure = grid.link_disclosure(&link).unwrap();
        assert_eq!(
            reasons(&disclosure.warning),
            Vec::from_iter(reason),
            "the premise: {runs} warns as expected"
        );
        assert_eq!(
            disclosure
                .difference()
                .map(|range| &disclosure.target[range]),
            marked,
            "{runs} against {target}"
        );
    }
}

/// A seam is soft only where the hyperlink's own text meets edge to edge: its piece reaches the row's last text column
/// (a wide-wrap pad column is not text) and the next row's piece starts at column 0. Anything else is a
/// program's own break, which separates before an address. Getting this wrong either glues a word onto an address
/// (`Visithttps://`, a false alarm) or splits a wrapped address in two. Spec: with `Visit` above `https://github.com/`
/// under one explicit hyperlink to another site, the label reads `Visithttps://github.com/` when `Visit` ends the
/// wrapped row (after unlinked text, or before a wide-wrap pad cell), and `Visit https://github.com/` when unlinked
/// cells fill the rest of the wrapped row, or a real styled cell (not a pad) takes its last column.
#[test]
fn a_seam_is_soft_only_where_the_hyperlink_itself_wraps() {
    let link = |col: usize| json!([col, 5, "Visit", [0, -1, -1], 0]);
    let filler = |col: usize, width: usize| json!([col, width, " ".repeat(width), [0, -1, -1]]);
    for (name, first, joined) in [
        (
            "unlinked cells fill the wrapped row",
            json!({"w":true,"r":[link(0), filler(5, 15)]}),
            false,
        ),
        (
            "the hyperlink ends the wrapped row",
            json!({"w":true,"r":[[0,15,"x".repeat(15),[0,-1,-1]], link(15)]}),
            true,
        ),
        (
            "a wide-wrap pad cell follows the hyperlink",
            json!({"w":true,"p":true,"r":[[0,14,"x".repeat(14),[0,-1,-1]], link(14), [19,1," ",[0,-1,2]]]}),
            true,
        ),
        (
            "a real styled cell follows the hyperlink",
            json!({"w":true,"p":false,"r":[[0,14,"x".repeat(14),[0,-1,-1]], link(14), [19,1," ",[0,-1,2]]]}),
            false,
        ),
    ] {
        let grid = model(
            20,
            vec![first, ascii("https://github.com/", false, Some(0))],
            vec![json!(["tag", "https://evil.test/"])],
        );
        let col = grid.row_links(0)[0].ranges[0].start.col;
        let shown = warning(&grid, 0, col).unwrap();
        assert_eq!(shown, warning(&grid, 1, 0).unwrap(), "{name}");
        let label = if joined {
            "Visithttps://github.com/"
        } else {
            "Visit https://github.com/"
        };
        assert_eq!(shown.label, label, "{name}");
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Unseen text inside a hyperlink
// ---------------------------------------------------------------------------------------------------------------------

/// Text drawn in its background color is out of sight like concealed text: it leaves the label, and its presence warns
/// that part of the label is hidden, since a program has no honest reason to hide part of a link's text. Spec:
/// `https://example.test/a` followed by a background-colored `b` in the same hyperlink: the link's label and range end
/// at `a`; to `https://example.test/a` the hover warns hidden, showing `https://example.test/a`; to
/// `https://example.test/ab` it also mismatches on what's visible.
#[test]
fn a_hidden_suffix_leaves_the_label_and_warns_hidden() {
    for (target, expected) in [
        ("https://example.test/a", vec![Hidden]),
        ("https://example.test/ab", vec![Mismatch, Hidden]),
    ] {
        let grid = model(
            30,
            vec![json!({"w":false,"r":[
                [0,22,"https://example.test/a",[0,-1,-1],0],
                [22,1,"b",[0,0,-1],0]
            ]})],
            vec![json!(["tag", target])],
        );
        assert_out_of_sight(&grid, 0, 1, "background-colored");
        let link = grid.link_at(Position { line: 0, col: 0 }).unwrap();
        assert_eq!(link.label, "https://example.test/a");
        assert_eq!(link.ranges[0].end.col, 22);
        assert!(link.unseen);
        let shown = warning(&grid, 0, 0).unwrap();
        assert_eq!(shown.reasons, expected, "{target}");
        assert_eq!(shown.label, "https://example.test/a");
    }
}

/// Unseen text between two pieces of one anonymous hyperlink splits it into two links, and each of them must still warn
/// that something with its destination is hidden between them. Spec: `abc`, an out-of-sight `XYZ` and `def` on one row,
/// three generated ids to one destination: the visible pieces are separate links, and each one's hover warns hidden.
#[test]
fn a_hidden_middle_piece_warns_from_either_visible_piece() {
    for (way, style) in out_of_sight() {
        let grid = model(
            12,
            vec![json!({"w":false,"r":[
                [0,3,"abc",[0,-1,-1],0],
                [3,3,"XYZ",style,1],
                [6,3,"def",[0,-1,-1],2]
            ]})],
            (1..=3)
                .map(|id| json!([format!("{id}_alacritty"), "https://example.test/"]))
                .collect(),
        );
        assert_out_of_sight(&grid, 0, 1, way);
        let links = grid.row_links(0);
        assert_eq!(links.len(), 2, "{way}");
        assert_ne!(links[0].kind, links[1].kind, "{way}");
        for col in [0, 6] {
            assert_eq!(
                reasons(&warning(&grid, 0, col)),
                [Hidden],
                "{way}, column {col}"
            );
        }
    }
}

/// A program can fill a row with the start of a host and move to the next row itself (CR LF at the edge, or cursor
/// positioning) instead of letting the terminal wrap. On screen that is indistinguishable from a wrap, and the user reads
/// the two pieces as one host, so the hover must too: separating there would judge only the second piece, which names
/// the destination, and a spoof the old client caught would go quiet. Spec: under one explicit hyperlink without the
/// wrap flag, `git` ending row 0 at its last column above `hub.com/login`, `pay` above `pal.com/login` and
/// `accounts.goo` above `gle.com/x`, each to the second row's site, warn of a mismatch from both rows, naming and
/// showing the joined address.
#[test]
fn a_hyperlink_that_fills_its_row_joins_the_next_row_without_a_wrap() {
    for (head, tail, target, joined) in [
        (
            "git",
            "hub.com/login",
            "https://hub.com/login",
            "github.com/login",
        ),
        (
            "pay",
            "pal.com/login",
            "https://pal.com/login",
            "paypal.com/login",
        ),
        (
            "accounts.goo",
            "gle.com/x",
            "https://gle.com/x",
            "accounts.google.com/x",
        ),
    ] {
        // Unlinked filler before the head lets the head end exactly at the last column of a row wide enough for the
        // tail.
        let cols = head.len().max(tail.len());
        let filler = cols - head.len();
        let mut first = Vec::new();
        if filler > 0 {
            first.push(json!([0, filler, "x".repeat(filler), [0, -1, -1]]));
        }
        first.push(json!([filler, head.len(), head, [0, -1, -1], 0]));
        let grid = model(
            cols as u16,
            vec![json!({"w":false,"r":first}), ascii(tail, false, Some(0))],
            vec![json!(["tag", target])],
        );
        let shown = warning(&grid, 0, filler as u16);
        assert_eq!(shown, warning(&grid, 1, 0), "{joined}: both rows");
        let shown = shown.unwrap_or_else(|| panic!("{joined} warns"));
        assert_eq!(shown.reasons, [Mismatch], "{joined}");
        assert_eq!(shown.address.as_deref(), Some(joined));
        assert_eq!(shown.label, joined);
    }
}

/// Programs print secondary text in gray (SGR 90) and dim text all the time, and an honest link in them must not warn:
/// the hidden threshold sits below the dimmest ordinary styles. Spec: `https://example.test/a` followed by `b` in each
/// of SGR 90, dim default and dim SGR 90, all one hyperlink to `https://example.test/ab`, is quiet, its label whole.
#[test]
fn gray_and_dim_text_in_a_hyperlink_is_not_hidden() {
    for style in [
        json!([0, 8, -1]),
        json!([style_flags::DIM, -1, -1]),
        json!([style_flags::DIM, 8, -1]),
    ] {
        let grid = model(
            30,
            vec![json!({"w":false,"r":[
                [0,22,"https://example.test/a",[0,-1,-1],0],
                [22,1,"b",style,0]
            ]})],
            vec![json!(["tag", "https://example.test/ab"])],
        );
        let link = grid.link_at(Position { line: 0, col: 0 }).unwrap();
        assert_eq!(link.label, "https://example.test/ab", "{style}");
        assert!(!link.unseen);
        assert_eq!(warning(&grid, 0, 0), None, "{style}");
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// When a resting hover must be judged again
// ---------------------------------------------------------------------------------------------------------------------

/// Replace row `y` with `row` and commit it as a frame at revision `rev`, the way live output arrives.
fn redraw_row(grid: &mut GridModel, y: usize, row: Value, links: &[Value], rev: u64) {
    let (cols, rows) = grid.size();
    apply_message(grid, json!({"t":"rows","rows":[[y,row]],"links":links}));
    apply_message(
        grid,
        json!({"t":"frame","rev":rev,"cols":cols,"rows":rows,"cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":false}}),
    );
}

/// A resting hover keeps its answer for as long as the link under the pointer compares equal, so a change that adds or
/// drops the hidden reason must change the hovered link, even when its own text and range stay the same. Spec: row 0
/// holds `click here` and, after a gap, `x`, one hyperlink. Recoloring `x` to the background makes the link at
/// `click here` compare unequal and its hover warn hidden; recoloring it back restores the original link. Recoloring
/// `x` to green, which stays readable, leaves the link equal. A same-target hyperlink on another row turning hidden
/// leaves it equal too: only the hovered row is watched (SPEC.md).
#[test]
fn a_run_turning_hidden_on_the_hovered_row_changes_the_link() {
    let links = [
        json!(["tag", "https://example.test/"]),
        json!(["other", "https://example.test/"]),
    ];
    let row = |x_style: Value| json!({"w":false,"r":[[0,10,"click here",[0,-1,-1],0],[11,1,"x",x_style,0]]});
    let other = |style: Value| json!({"w":false,"r":[[0,1,"y",style,1]]});
    let mut grid = model(
        20,
        vec![row(json!([0, -1, -1])), other(json!([0, -1, -1]))],
        links.to_vec(),
    );
    let at = Position { line: 0, col: 2 };
    let original = grid.link_at(at).unwrap();
    assert!(!original.unseen);
    assert_eq!(warning(&grid, 0, 2), None);

    redraw_row(&mut grid, 0, row(json!([0, 0, -1])), &links, 2);
    assert_out_of_sight(&grid, 0, 1, "background-colored");
    let hidden = grid.link_at(at).unwrap();
    assert_eq!(
        (&hidden.label, &hidden.ranges),
        (&original.label, &original.ranges)
    );
    assert_ne!(hidden, original, "turning hidden changes the link");
    assert_eq!(reasons(&warning(&grid, 0, 2)), [Hidden]);

    redraw_row(&mut grid, 0, row(json!([0, -1, -1])), &links, 3);
    assert_eq!(
        grid.link_at(at).unwrap(),
        original,
        "and turning visible again restores it"
    );

    redraw_row(&mut grid, 0, row(json!([0, 2, -1])), &links, 4);
    assert_eq!(
        grid.link_at(at).unwrap(),
        original,
        "a readable color changes nothing"
    );

    redraw_row(&mut grid, 1, other(json!([0, 0, -1])), &links, 5);
    assert_out_of_sight(&grid, 1, 0, "background-colored");
    assert_eq!(
        grid.link_at(at).unwrap(),
        original,
        "another row is not watched"
    );
}

/// The program's own default colors decide what is hidden too, so an OSC 10 that sets the default foreground to the
/// background hides every default-colored run, and must change the hovered link the same way a recolor does. Spec:
/// `click here` in explicit white and `x` in the default foreground, one hyperlink; setting the default foreground to
/// the background makes the link compare unequal and warn hidden, and setting it back restores it.
#[test]
fn a_default_color_change_that_hides_a_run_changes_the_link() {
    let mut grid = model(
        20,
        vec![json!({"w":false,"r":[[0,10,"click here",[0,7,-1],0],[11,1,"x",[0,-1,-1],0]]})],
        vec![json!(["tag", "https://example.test/"])],
    );
    let state = |default_fg: Option<i64>| json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,"default_fg":default_fg,"default_bg":null,"history_trimmed":false,"exit":null});
    let at = Position { line: 0, col: 2 };
    let original = grid.link_at(at).unwrap();
    assert_eq!(warning(&grid, 0, 2), None);

    let background = direct(grid.palette().background.hex());
    apply_message(&mut grid, state(Some(background)));
    assert_out_of_sight(&grid, 0, 1, "default foreground on the background");
    assert_ne!(grid.link_at(at).unwrap(), original);
    assert_eq!(reasons(&warning(&grid, 0, 2)), [Hidden]);

    apply_message(&mut grid, state(None));
    assert_eq!(grid.link_at(at).unwrap(), original);
}

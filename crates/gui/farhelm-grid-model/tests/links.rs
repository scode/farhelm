//! Recognition stays on validated cell data; browser dispatch is an allowlist.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test-only crate; crates/gui/clippy.toml's test allowance covers #[test] functions but not these helpers"
)]
use farhelm_grid_model::{
    GridModel, GridOutput, LinkKind, Position, SelectionMode, style_flags, tunnel_port, web_target,
};
use serde_json::{Value, json};

/// Explicit cell rows pass the actual wire encoder/checker before lookup. This
/// fixture has no terminal emulator or UI whose URL detection could mask ours.
fn model(cols: u16, rows: Vec<Value>, links: Vec<Value>) -> GridModel {
    let count = rows.len();
    let mut model = GridModel::new();
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

/// Disclosure reads both pieces of a wrapped program hyperlink. A wrapped label can put an address immediately
/// after its own text, and still receives an ordinary target disclosure. A wrapped address alone still warns.
#[test]
fn a_wrapped_program_label_can_contain_an_address_without_claiming_it() {
    let target = "https://other.test/";
    let columns = 20;
    for (label, warns) in [
        ("ticket-42(https://example.test/x)", false),
        ("ticket-42(example.test/tickets/42)", false),
        ("https://example.test/tickets/42", true),
    ] {
        let (first, last) = label.split_at(usize::from(columns));
        let model = model(
            columns,
            vec![ascii(first, true, Some(0)), ascii(last, false, Some(0))],
            vec![json!(["example", target])],
        );
        let start = model.link_at(Position { line: 0, col: 0 }).unwrap();
        let end = model.link_at(Position { line: 1, col: 0 }).unwrap();
        assert_eq!(start.label, first);
        assert_eq!(end.label, last);
        assert_eq!(start.kind, end.kind, "both rows share the explicit tag");
        for link in [start, end] {
            let disclosure = model.link_disclosure(&link).unwrap();
            assert_eq!(disclosure.host, "other.test");
            assert_eq!(
                disclosure.claimed_text.as_deref(),
                warns.then_some(label),
                "{label}"
            );
        }
    }
}

/// A bracketed or quoted address is still a claim about the destination. The warning retains
/// the wrapping punctuation so the user can compare what they saw with the actual target.
#[test]
fn quoting_an_address_keeps_the_original_label_in_mismatch_warnings() {
    for (label, claims_address) in [
        ("<https://example.test/>", true),
        ("\"https://example.test/\"", true),
        ("'https://example.test/'", true),
        ("(https://example.test/)", true),
        ("[https://example.test/]", true),
        (" ( \"<https://example.test/>\" ) ", true),
        ("See (https://example.test/)", false),
        ("ticket-42(https://example.test/)", false),
        ("[docs](https://example.test/)", false),
        ("[https://example.test/] [docs]", false),
        ("<https://example.test/)", false),
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
            let expected = (claims_address && differs).then_some(label.trim());
            assert_eq!(disclosure.claimed_text.as_deref(), expected, "{label:?}");
            assert_eq!(disclosure.claimed.as_deref(), expected, "{label:?}");
        }
    }
}

/// Distinct outer delimiters can make a concatenation look like prose even when both rows
/// present complete addresses. Hovering each row must still compare that row to the target.
#[test]
fn differently_wrapped_rows_keep_their_individual_address_claims() {
    let honest = "<https://example.test/>";
    let other = "\"https://other.test/\"";
    let target = "https://example.test/";
    let grid = model(
        40,
        vec![ascii(honest, false, Some(0)), ascii(other, false, Some(0))],
        vec![json!(["tag", target])],
    );
    let first = grid.link_at(Position { line: 0, col: 0 }).unwrap();
    let second = grid.link_at(Position { line: 1, col: 0 }).unwrap();
    assert_eq!(first.kind, second.kind);
    assert_eq!(first.label, honest);
    assert_eq!(second.label, other);
    assert_eq!(grid.link_disclosure(&first).unwrap().claimed_text, None);
    assert_eq!(
        grid.link_disclosure(&second)
            .unwrap()
            .claimed_text
            .as_deref(),
        Some(other)
    );
}

/// Two rows with the same tag can each spell out the same target. The second address
/// must be recognized as a repeat even when the rows quote it differently.
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
        assert_eq!(disclosure.claimed_text, None);
    }
}

/// An ambiguous continuation still warns on a quoted address-shaped fragment. Even when the
/// fragment names the destination, the other stretches keep it from proving a complete match.
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
    assert_eq!(disclosure.claimed_text.as_deref(), Some(address));
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
/// warning when disclosure reconstructs the visible label across the wrap.
#[test]
fn osc_disclosure_excludes_padding_inside_a_link_run() {
    let model = model(
        16,
        vec![
            json!({"w":true,"p":true,"r":[[0,16,"https://a.test/ ",[0,-1,-1],0]]}),
            json!({"w":false,"r":[[0,2,"界",[0,-1,-1],1]]}),
        ],
        vec![
            json!(["1_alacritty", "https://a.test/界"]),
            json!(["2_alacritty", "https://a.test/界"]),
        ],
    );
    let first = model.link_at(Position { line: 0, col: 1 }).unwrap();
    let last = model.link_at(Position { line: 1, col: 0 }).unwrap();
    assert_eq!(first.kind, last.kind);
    assert_eq!(first.label, "https://a.test/");
    assert!(model.link_at(Position { line: 0, col: 15 }).is_none());
    assert!(
        model
            .link_disclosure(&first)
            .unwrap()
            .claimed_text
            .is_none()
    );
    assert!(model.link_disclosure(&last).unwrap().claimed_text.is_none());
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

/// A style-only update changes which label a user sees. It can expose a real destination mismatch or remove
/// a false warning. The link under the pointer changes identity and its hover range ends at the visible label.
#[test]
fn concealing_a_program_link_suffix_rechecks_the_visible_label() {
    for (prefix, suffix, target, warns_before) in [
        (
            "https://example.test",
            ".other.test",
            "https://example.test.other.test",
            false,
        ),
        ("menu", "://example.test/x", "https://other.test/x", true),
    ] {
        let split = prefix.len() as u16;
        let row = |flags| {
            json!({"w":false,"r":[
                [0,split,prefix,[0,-1,-1],0],
                [split,suffix.len(),suffix,[flags,-1,-1],0]
            ]})
        };
        let links = vec![json!(["tag", target])];
        let mut grid = model(48, vec![row(0)], links.clone());
        let start = Position { line: 0, col: 0 };
        let boundary = Position {
            line: 0,
            col: split,
        };
        let before = grid.link_at(start).unwrap();
        let full_label = format!("{prefix}{suffix}");
        assert_eq!(before.label, full_label);
        assert_eq!(grid.link_at(boundary), Some(before.clone()));
        assert_eq!(
            grid.link_disclosure(&before)
                .unwrap()
                .claimed_text
                .as_deref(),
            warns_before.then_some(full_label.as_str())
        );

        apply_message(
            &mut grid,
            json!({"t":"rows","rows":[[0,row(style_flags::HIDDEN)]],"links":links}),
        );
        apply_message(
            &mut grid,
            json!({"t":"frame","rev":2,"cols":48,"rows":1,"cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":false}}),
        );
        let after = grid.link_at(start).unwrap();
        assert_eq!(after.kind, before.kind);
        assert_eq!(after.uri, before.uri);
        assert_eq!(after.label, prefix);
        assert_eq!(after.ranges[0].end, boundary);
        assert_ne!(after, before);
        assert_eq!(grid.link_at(boundary), None);
        assert_eq!(
            grid.link_disclosure(&after)
                .unwrap()
                .claimed_text
                .as_deref(),
            (!warns_before).then_some(prefix)
        );
    }
}

/// A concealed-only row ends the visible label walk. It stays in the evidence boundary so revealing it can
/// extend the link, and another visible row carrying the same tag beyond it stays outside the comparison.
#[test]
fn a_concealed_only_row_breaks_a_program_link_label() {
    let grid = model(
        42,
        vec![
            ascii("https://example.test/x", false, Some(0)),
            json!({"w":false,"r":[[0,6,"hidden",[style_flags::HIDDEN,-1,-1],0]]}),
            ascii("unrelated", false, Some(0)),
        ],
        vec![json!(["tag", "https://example.test/x"])],
    );
    let first = grid.link_at(Position { line: 0, col: 0 }).unwrap();
    let other = grid.link_at(Position { line: 2, col: 0 }).unwrap();
    assert_eq!(first.kind, other.kind);
    assert!(grid.row_links(1).is_empty());
    assert!(first.evidence.contains(&1));
    assert!(!first.evidence.contains(&2));
    assert!(grid.link_disclosure(&first).unwrap().claimed_text.is_none());
}

/// A concealed gap splits the hit ranges even when the runs have one explicit tag. An adjacent continuation
/// makes the address ambiguous, so the visible address-shaped fragment still warns.
#[test]
fn a_concealed_gap_splits_the_hit_ranges_of_an_explicit_program_link() {
    let address = "https://example.test/x";
    let split = address.len();
    let grid = model(
        48,
        vec![
            json!({"w":false,"r":[
                [0,split,address,[0,-1,-1],0],
                [split,2,"  ",[style_flags::HIDDEN,-1,-1],0],
                [split+2,4,"more",[0,-1,-1],0]
            ]}),
            ascii("continuation", false, Some(0)),
        ],
        vec![json!(["tag", address])],
    );
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
    assert_eq!(
        grid.link_disclosure(&links[0])
            .unwrap()
            .claimed_text
            .as_deref(),
        Some(address)
    );
}

/// Revealing a separate stretch of the same tag can change the hovered row's warning while leaving the
/// hovered stretch itself untouched. When that happens, a display for the old link must be refreshed.
#[test]
fn revealing_a_second_stretch_invalidates_a_wrapped_link_disclosure() {
    let prefix = "https://example.test/";
    let links = vec![json!(["tag", "https://example.test/x"])];
    let separated = prefix.len() + 1;
    let row = |flags| {
        json!({"w":false,"r":[
            [0,prefix.len(),prefix,[0,-1,-1],0],
            [separated,4,"more",[flags,-1,-1],0]
        ]})
    };
    let mut grid = model(
        48,
        vec![row(style_flags::HIDDEN), ascii("x", false, Some(0))],
        links.clone(),
    );
    let hovered = Position { line: 0, col: 0 };
    let before = grid.link_at(hovered).unwrap();
    assert_eq!(before.label, prefix);
    assert!(
        grid.link_disclosure(&before)
            .unwrap()
            .claimed_text
            .is_none()
    );

    apply_message(
        &mut grid,
        json!({"t":"rows","rows":[[0,row(0)]],"links":links}),
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
    assert_eq!(
        grid.link_disclosure(&after)
            .unwrap()
            .claimed_text
            .as_deref(),
        Some(prefix)
    );
}

/// Anonymous tags sharing a destination only inherit each other's identity across visible cells. Concealing
/// either side of a soft-wrap seam keeps the second row's visible text from joining the first.
#[test]
fn concealed_cells_break_generated_link_continuation_on_either_side_of_a_wrap() {
    for hide_above in [false, true] {
        let style = |hide| {
            [
                if hide {
                    i64::from(style_flags::HIDDEN)
                } else {
                    0
                },
                -1,
                -1,
            ]
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
        assert!(matches!(tail.kind, LinkKind::Osc { ref id } if id == expected));
        let hidden = if hide_above {
            Position { line: 0, col: 4 }
        } else {
            Position { line: 1, col: 0 }
        };
        assert_eq!(grid.link_at(hidden), None);
    }
}

/// A printed address under a program's tag must never become a fallback target, including when the whole
/// tagged label is concealed. The same visible address without a tag is a printed link.
#[test]
fn a_concealed_program_link_over_a_printed_address_has_no_fallback_link() {
    let label = "https://example.test/x";
    let position = Position { line: 0, col: 0 };
    let printed = model(26, vec![ascii(label, false, None)], vec![]);
    assert_eq!(printed.link_at(position).unwrap().kind, LinkKind::Text);
    let tagged = model(
        26,
        vec![json!({"w":false,"r":[[0,label.len(),label,[style_flags::HIDDEN,-1,-1],0]]})],
        vec![json!(["tag", "https://other.test/x"])],
    );
    assert!(tagged.row_links(0).is_empty());
    assert_eq!(tagged.link_at(position), None);
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

/// Recognition and label joining are evaluated together against the actual
/// pinned providers. Hard row breaks, repeated tags, ambiguous runs and budget
/// exhaustion must not turn a forged address into a quiet disclosure.
#[test]
fn pinned_cell_links_and_label_join_oracle() {
    let oracle: Value = serde_json::from_str(include_str!("fixtures/links-oracle.json")).unwrap();
    for (index, case) in oracle["grids"].as_array().unwrap().iter().enumerate() {
        let model = model(
            case["cols"].as_u64().unwrap() as u16,
            case["rows"].as_array().unwrap().clone(),
            case["links"].as_array().unwrap().clone(),
        );
        for question in case["questions"].as_array().unwrap() {
            let line = question["line"].as_u64().unwrap();
            let actual: Vec<_> = model.row_links(line).iter().map(|link| {
                let mut value = json!({
                    "kind": match &link.kind { LinkKind::Osc { .. } => "osc", LinkKind::Text => "text", LinkKind::Path => "path" },
                    "uri": link.uri,
                    "ranges": link.ranges.iter().map(|range| json!({
                        "start": {"abs": range.start.line, "col": range.start.col},
                        "end": {"abs": range.end.line, "col": range.end.col},
                    })).collect::<Vec<_>>(),
                    "evidence": link.evidence,
                    "disclosure": model.link_disclosure(link).map(|display| json!({
                        "host": display.host, "target": display.target, "claimed": display.claimed,
                    })),
                });
                if let LinkKind::Osc { id } = &link.kind { value["id"] = json!(id); }
                value
            }).collect();
            assert_eq!(
                json!(actual),
                question["expected"],
                "grid {index}, row {line}"
            );
        }
    }
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

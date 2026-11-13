//! Recognition stays on validated cell data; browser dispatch is an allowlist.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test-only crate; crates/gui/clippy.toml's test allowance covers #[test] functions but not these helpers"
)]
use farhelm_grid_model::{GridModel, GridOutput, LinkKind, Position, web_target};
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
        let message: GridOutput = serde_json::from_value(value).unwrap();
        let _ = model.apply(&message.encode().unwrap()).unwrap();
    }
    model
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
                    "kind": match &link.kind { LinkKind::Osc { .. } => "osc", LinkKind::Text => "text" },
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

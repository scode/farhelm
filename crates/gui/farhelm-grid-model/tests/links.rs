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

/// Generated OSC identities continue from the last text run, even when a separate color run paints the padding.
/// A separate identity on the next row would split its label across what is really one linked stretch.
#[test]
fn osc_continuation_looks_past_the_styled_padding_run() {
    let model = model(
        4,
        vec![
            json!({"w":true,"p":true,"r":[[0,3,"abc",[0,-1,-1],0],[3,1," ",[0,-1,2]]]}),
            json!({"w":false,"r":[[0,2,"界",[0,-1,-1],1]]}),
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
    assert!(model.link_at(Position { line: 0, col: 3 }).is_none());
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

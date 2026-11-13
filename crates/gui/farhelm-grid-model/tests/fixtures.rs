//! Replay the protocol's golden output corpus and the old web client's recorded streams through the model.
//!
//! Each standalone output fixture receives only the hello/state context it needs, so a fixture does not fail because
//! its unrelated predecessor was missing. Recorded JSONL streams are explicitly encoded to current wire form.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test-only crate; crates/gui/clippy.toml's test allowance covers #[test] functions but not these helpers"
)]
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use farhelm_grid_model::{
    Color, Cursor, CursorShape, ExitState, GridModel, GridOutput, GridRow, GridRun, Link,
    MouseMode, TerminalState,
};
use farhelm_proto::grid::{GRID_MAX_DIMENSION, GRID_MAX_HISTORY_CAP};

/// The protocol's own golden corpus, read in place from `farhelm-proto` so this model is always checked against the
/// fixtures the encoder and supervisor are checked against, never against a copy that could fall behind.
fn proto_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../farhelm-proto/grid-fixtures")
}

/// The old web client's recorded streams, copied here because their original home goes with the old GUI.
fn recorded_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recorded")
}

/// Enumerate every file, so newly copied cases cannot silently go untested.
fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_files(&path, files);
        } else {
            files.push(path);
        }
    }
}

/// Encode semantic helpers through production code, including binary rows.
/// This corpus checks model contents; dedicated OSC 52 tests consume effects.
fn send(model: &mut GridModel, message: &GridOutput) {
    let _ = model.apply(&message.encode().unwrap()).unwrap();
}

/// Give independent output cases a valid attachment prefix at the largest size.
fn prefix(message: &GridOutput) -> GridModel {
    let mut model = GridModel::new();
    if !matches!(message, GridOutput::Hello { .. }) {
        send(
            &mut model,
            &GridOutput::Hello {
                cols: GRID_MAX_DIMENSION,
                rows: GRID_MAX_DIMENSION,
                history_cap: GRID_MAX_HISTORY_CAP,
            },
        );
        let bytes = fs::read(proto_root().join("output/valid/state-running.json")).unwrap();
        let _ = model.apply(&bytes).unwrap();
    }
    model
}

/// Compare the owned representation with the fixture's received row, resolving
/// only its own link table. This catches style/text/link loss in the model.
fn owned(row: &farhelm_proto::grid::Row, links: &[Link]) -> GridRow {
    GridRow {
        wraps: row.wraps,
        runs: row
            .runs
            .iter()
            .map(|run| GridRun {
                col: run.col,
                width: run.width,
                text: run.text.clone(),
                style: run.style,
                link: run
                    .link
                    .map(|index| Arc::new(links[usize::from(index)].clone())),
            })
            .collect(),
    }
}

/// Exercise every accepted output fixture through the ordering checker and
/// owned model, then check that its content survived the client transition.
fn replay_output(message: &GridOutput, bytes: &[u8], name: &str) {
    let mut model = prefix(message);
    let _ = model
        .apply(bytes)
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    match message {
        GridOutput::Hello {
            cols,
            rows,
            history_cap,
        } => {
            assert_eq!(model.size(), (*cols, *rows), "{name}");
            assert_eq!(model.history_cap(), *history_cap as usize, "{name}");
        }
        GridOutput::Rows { rows, links } => {
            assert!(
                model.screen().iter().all(|row| row.runs.is_empty()),
                "{name}: uncommitted rows leaked"
            );
            let frame: GridOutput = serde_json::from_value(serde_json::json!({
                "t": "frame", "rev": 1, "cols": 1000, "rows": 1000,
                "cursor": {"x": 0, "y": 0, "visible": true, "shape": "block", "blink": true}
            }))
            .unwrap();
            send(&mut model, &frame);
            let expected: BTreeMap<_, _> = rows
                .iter()
                .map(|entry| (entry.y, owned(&entry.row, links)))
                .collect();
            for (y, row) in expected {
                assert_eq!(&model.screen()[usize::from(y)], &row, "{name}");
            }
        }
        GridOutput::Frame {
            rev,
            cols,
            rows,
            cursor,
            ..
        } => {
            assert_eq!(model.size(), (*cols, *rows), "{name}");
            assert_eq!(model.revision(), Some(*rev), "{name}");
            assert_eq!(model.cursor(), Some(*cursor), "{name}");
        }
        GridOutput::History {
            first,
            cols,
            rows,
            links,
        } => {
            assert_eq!(model.history().len(), rows.len(), "{name}");
            assert_eq!(model.screen_base(), first + rows.len() as u64, "{name}");
            for (offset, (line, row)) in model.history().iter().zip(rows).enumerate() {
                assert_eq!(line.number, first + offset as u64, "{name}");
                assert_eq!(line.cols, *cols, "{name}");
                assert_eq!(line.row, owned(row, links), "{name}");
            }
        }
        GridOutput::State(state) => assert_eq!(model.state(), Some(*state), "{name}"),
        GridOutput::HistoryReset => assert!(model.take_damage().history_reset, "{name}"),
        GridOutput::Clipboard { .. } => {}
    }
}

/// Every accepted output fixture in the protocol's golden corpus survives the model: the corpus is read in place from
/// `farhelm-proto`, each standalone message gets the hello/state prefix it needs, and its rows, history, frame, state
/// or reset must show up in the model unchanged.
///
/// The corpus's own properties are `farhelm-proto`'s to test, and its `grid_tests.rs` does: the fixture inventory,
/// input fixtures, every invalid fixture refused for its stated reason, and the binary encodings. This test checks
/// only what the model adds, so it skips those categories, but it fails on any file outside the known categories:
/// a new kind of fixture must be classified here before it can silently go untested. Frame sequencing (rows waiting
/// for their frame, damage, history shifts) is covered by the recorded streams below and by `model.rs`, not by this
/// per-fixture replay.
#[test]
fn every_accepted_output_fixture_survives_the_model() {
    let root = proto_root();
    let mut paths = Vec::new();
    collect_files(&root, &mut paths);
    paths.sort();
    let mut replayed = 0;
    for path in &paths {
        let relative = path.strip_prefix(&root).unwrap().to_str().unwrap();
        if relative.starts_with("input/") || relative.starts_with("output/invalid/") {
            continue;
        }
        let binary = relative.starts_with("output/binary/");
        if binary && relative.starts_with("output/binary/invalid/") {
            continue;
        }
        assert!(
            relative.starts_with("output/valid/")
                || relative.starts_with("output/tolerated/")
                || relative.starts_with("output/binary/valid/"),
            "unclassified fixture {relative}: decide whether the model must replay it"
        );
        if binary && path.extension().unwrap() == "json" {
            // The description of a `.bin` beside it; the pair is replayed through the `.bin`.
            continue;
        }
        let bytes = fs::read(path).unwrap();
        let message =
            GridOutput::decode(&bytes).unwrap_or_else(|error| panic!("{relative}: {error}"));
        replay_output(&message, &bytes, relative);
        replayed += 1;
    }
    assert!(
        replayed > 0,
        "no output fixtures found under {}",
        root.display()
    );
}

/// Farhelm's JS client recordings retain their semantic form in the fixture
/// tree. Encode each entry to the actual binary/JSON protocol before applying
/// it, then pin final screen, history, styles, cursor, and terminal state.
#[test]
fn recorded_attach_and_scroll_streams() {
    for name in ["attach", "scroll"] {
        let mut model = GridModel::new();
        for line in fs::read_to_string(recorded_root().join(format!("{name}.jsonl")))
            .unwrap()
            .lines()
        {
            let message: GridOutput = serde_json::from_str(line).unwrap();
            send(&mut model, &message);
        }
        assert!(!model.is_visible());
        model.mark_replay_complete().unwrap();
        assert!(model.is_visible());
        if name == "attach" {
            assert_eq!(model.terminal_text(), "$ echo\nhello 中文 web\n\n");
            assert_eq!(
                model
                    .history()
                    .iter()
                    .map(|line| line.number)
                    .collect::<Vec<_>>(),
                [41, 42, 43]
            );
            assert_eq!(
                model.screen()[1].runs[3].link.as_ref().unwrap().uri,
                "https://example.com/"
            );
            assert_eq!(model.screen()[1].runs[0].style.flags, 1);
            assert_eq!(model.screen_base(), 44);
            assert_eq!(
                model.cursor(),
                Some(Cursor {
                    x: 2,
                    y: 2,
                    visible: true,
                    shape: CursorShape::Block,
                    blink: true,
                })
            );
            assert_eq!(
                model.state(),
                Some(TerminalState {
                    mouse: MouseMode::Off,
                    focus: false,
                    alt_screen: false,
                    default_fg: None,
                    default_bg: None,
                    history_trimmed: false,
                    exit: None,
                })
            );
        } else {
            assert_eq!(model.terminal_text(), "l4\nl5\nl6");
            assert_eq!(
                model
                    .history()
                    .iter()
                    .map(|line| line.row.text(0, line.cols).trim().to_owned())
                    .collect::<Vec<_>>(),
                ["l1", "l2", "l3"]
            );
            assert_eq!(model.revision(), Some(5));
            assert_eq!(
                model.cursor(),
                Some(Cursor {
                    x: 2,
                    y: 2,
                    visible: false,
                    shape: CursorShape::Underline,
                    blink: true,
                })
            );
            assert_eq!(
                model.state(),
                Some(TerminalState {
                    mouse: MouseMode::Drag,
                    focus: true,
                    alt_screen: true,
                    default_fg: Color::from_wire(17834032),
                    default_bg: None,
                    history_trimmed: true,
                    exit: Some(ExitState::Code(3)),
                })
            );
        }
    }
}

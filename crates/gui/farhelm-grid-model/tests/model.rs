//! State-transition tests that complement individual protocol fixtures.
//!
//! These encode semantic test messages through the real encoder. No test sends
//! JSON rows as wire data, or relies on a terminal emulator or a UI runtime.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test-only crate; crates/gui/clippy.toml's test allowance covers #[test] functions but not these helpers"
)]
use std::sync::Arc;

use farhelm_grid_model::{
    ClipboardTarget, Color, GridModel, GridOutput, Link, Position, Selection, SelectionMode,
    ViewRow, Viewport, display_text, position_of,
};
use farhelm_proto::grid::{
    Cursor, CursorShape, GRID_MAX_LINE_NUMBER, MouseMode, Row, Run, ScreenRow, Style, TerminalState,
};

/// Build a row with explicit cell widths; a test never guesses Unicode width.
fn row(wraps: bool, runs: &[(u16, u16, &str)]) -> Row {
    Row {
        wraps,
        runs: runs
            .iter()
            .map(|&(col, width, text)| Run {
                col,
                width,
                text: text.into(),
                style: Style::PLAIN,
                link: None,
            })
            .collect(),
    }
}

/// Use the same binary encoding as production before reaching the model.
/// Screen/history tests deliberately discard effects; clipboard tests inspect
/// the returned write directly rather than using this helper.
fn send(model: &mut GridModel, message: GridOutput) {
    let _ = model.apply(&message.encode().unwrap()).unwrap();
}

/// Project real accumulated damage instead of manufacturing reset flags in tests.
fn sync_view(view: &mut Viewport, model: &mut GridModel) -> bool {
    let damage = model.take_damage();
    view.synchronize(model, &damage)
}

/// The number of the line at a scrolled view's top; a test that expects the notice line there asserts it directly.
fn top_line(view: &Viewport, model: &GridModel) -> u64 {
    match view.row_at(model, view.top(model)) {
        ViewRow::Line(number) => number,
        ViewRow::Notice => panic!("the view's top is the notice line"),
    }
}

/// Establish the mandatory hello/state prefix without completing replay.
fn opened(cols: u16, rows: u16, history_cap: u32) -> GridModel {
    let mut model = GridModel::new();
    send(
        &mut model,
        GridOutput::Hello {
            cols,
            rows,
            history_cap,
        },
    );
    send(
        &mut model,
        GridOutput::State(TerminalState {
            mouse: MouseMode::Off,
            focus: false,
            alt_screen: false,
            default_fg: None,
            default_bg: None,
            history_trimmed: false,
            exit: None,
        }),
    );
    model
}

/// Output changes history indices, but a scrolled view must keep its numbered
/// line until eviction forces it forward. Explicit bottom navigation follows again.
#[test]
fn viewport_anchors_survive_output_and_clamp_after_eviction() {
    let mut model = opened(8, 3, 4);
    send(&mut model, frame(1, 8, 3, 0));
    send(
        &mut model,
        history(
            100,
            8,
            vec![
                row(false, &[(0, 1, "A")]),
                row(false, &[(0, 1, "B")]),
                row(false, &[(0, 1, "C")]),
            ],
        ),
    );
    let mut view = Viewport::default();
    sync_view(&mut view, &mut model);
    assert!(view.is_following());
    assert!(view.scroll_lines(&model, -2));
    assert_eq!(view.top(&model), position_of(101));
    send(
        &mut model,
        history(103, 8, vec![row(false, &[(0, 1, "D")])]),
    );
    sync_view(&mut view, &mut model);
    assert_eq!(view.top(&model), position_of(101));
    assert_eq!(
        model.line(top_line(&view, &model)).unwrap().1.text(0, 1),
        "B"
    );
    send(
        &mut model,
        history(
            104,
            8,
            vec![row(false, &[(0, 1, "E")]), row(false, &[(0, 1, "F")])],
        ),
    );
    sync_view(&mut view, &mut model);
    assert_eq!(view.top(&model), position_of(102));
    assert!(!view.is_following());
    assert!(!view.scroll_lines(&model, 0));
    assert_eq!(
        model.line(top_line(&view, &model)).unwrap().1.text(0, 1),
        "C"
    );
    assert!(view.scroll_lines(&model, i64::MAX));
    assert!(view.is_following());
    assert_eq!(view.top(&model), position_of(model.screen_base()));
    assert!(view.scroll_lines(&model, i64::MIN));
    assert_eq!(view.top(&model), view.first(&model));
    assert!(view.follow());
    assert!(!view.follow());
}

/// Relayout and history reset can reuse numbers for different text. Alternate
/// screens retain primary history internally but must not offer it for navigation.
#[test]
fn viewport_resets_on_relayout_and_hides_primary_history_on_alt_screen() {
    let mut model = opened(8, 3, 4);
    send(&mut model, frame(1, 8, 3, 0));
    send(
        &mut model,
        history(
            100,
            8,
            vec![
                row(false, &[(0, 1, "A")]),
                row(false, &[(0, 1, "B")]),
                row(false, &[(0, 1, "C")]),
            ],
        ),
    );
    let mut view = Viewport::default();
    sync_view(&mut view, &mut model);
    view.scroll_lines(&model, -2);
    send(&mut model, GridOutput::HistoryReset);
    assert!(sync_view(&mut view, &mut model));
    assert!(view.is_following());
    send(
        &mut model,
        history(
            900,
            8,
            vec![
                row(false, &[(0, 1, "X")]),
                row(false, &[(0, 1, "Y")]),
                row(false, &[(0, 1, "Z")]),
            ],
        ),
    );
    sync_view(&mut view, &mut model);
    view.scroll_to(&model, position_of(901));
    let mut state = model.state().unwrap();
    state.alt_screen = true;
    send(&mut model, GridOutput::State(state));
    assert!(sync_view(&mut view, &mut model));
    assert!(view.is_following());
    assert_eq!(view.first(&model), view.bottom(&model));
    assert!(!view.scroll_lines(&model, -1));
    state.alt_screen = false;
    send(&mut model, GridOutput::State(state));
    sync_view(&mut view, &mut model);
    view.scroll_lines(&model, -1);
    send(&mut model, frame(2, 9, 4, 0));
    assert!(sync_view(&mut view, &mut model));
    assert!(view.is_following());
}

/// After a recovery the supervisor numbers the rebuilt history from line 0 and marks it trimmed. The notice line
/// saying earlier output was lost must sit above line 0 rather than on it: numbered as a line, it once took line 0's
/// place and hid the oldest recovered line for good. SPEC.md asks for the notice above the scrollback.
#[test]
fn the_notice_line_sits_above_history_that_starts_at_line_zero() {
    let mut model = opened(8, 3, 4);
    send(&mut model, frame(1, 8, 3, 0));
    let mut state = model.state().unwrap();
    state.history_trimmed = true;
    send(&mut model, GridOutput::State(state));
    send(
        &mut model,
        history(
            0,
            8,
            vec![row(false, &[(0, 1, "A")]), row(false, &[(0, 1, "B")])],
        ),
    );
    let mut view = Viewport::default();
    sync_view(&mut view, &mut model);
    assert_eq!(view.notice(&model), Some(view.first(&model)));
    assert!(view.scroll_lines(&model, i64::MIN));
    assert_eq!(view.row_at(&model, view.top(&model)), ViewRow::Notice);
    assert_eq!(view.row_at(&model, view.top(&model) + 1), ViewRow::Line(0));
    assert_eq!(view.row_at(&model, view.top(&model) + 2), ViewRow::Line(1));
    assert_eq!(view.top(&model) + 3, view.bottom(&model));
    assert!(view.scroll_lines(&model, 1));
    assert_eq!(top_line(&view, &model), 0);
    assert_eq!(model.line(0).unwrap().1.text(0, 1), "A");
    // The alternate screen offers no history, and so no notice.
    state.alt_screen = true;
    send(&mut model, GridOutput::State(state));
    sync_view(&mut view, &mut model);
    assert_eq!(view.notice(&model), None);
    assert_eq!(view.first(&model), view.bottom(&model));
}

/// A recovery that kept no history at all still has its notice, one row above the screen's first line (numbered 0),
/// which a notice numbered as a line would have landed on, losing the notice behind the screen row.
#[test]
fn the_notice_line_sits_above_a_screen_with_no_history() {
    let mut model = opened(8, 3, 4);
    send(&mut model, frame(1, 8, 3, 0));
    let mut state = model.state().unwrap();
    state.history_trimmed = true;
    send(&mut model, GridOutput::State(state));
    let mut view = Viewport::default();
    sync_view(&mut view, &mut model);
    assert_eq!(model.screen_base(), 0);
    assert!(view.scroll_lines(&model, -1));
    assert_eq!(view.row_at(&model, view.top(&model)), ViewRow::Notice);
    assert_eq!(view.top(&model) + 1, view.bottom(&model));
    assert!(!view.scroll_lines(&model, -1));
}

/// Wire history uses safe-integer line numbers. Navigation must saturate rather than wrap to
/// unrelated retained text, even for deltas larger than the entire buffer.
#[test]
fn viewport_navigation_saturates_at_large_absolute_line_numbers() {
    let mut model = opened(8, 2, 4);
    send(&mut model, frame(1, 8, 2, 0));
    send(
        &mut model,
        history(
            GRID_MAX_LINE_NUMBER - 2,
            8,
            vec![row(false, &[(0, 1, "A")]), row(false, &[(0, 1, "B")])],
        ),
    );
    let mut view = Viewport::default();
    sync_view(&mut view, &mut model);
    view.scroll_lines(&model, i64::MIN);
    assert_eq!(view.top(&model), position_of(GRID_MAX_LINE_NUMBER - 2));
    view.scroll_lines(&model, i64::MAX);
    assert!(view.is_following());
    assert_eq!(view.top(&model), position_of(GRID_MAX_LINE_NUMBER));
}

/// Commit a frame with a visible cursor safely inside every test's size.
fn frame(rev: u64, cols: u16, rows: u16, shift: u16) -> GridOutput {
    GridOutput::Frame {
        rev,
        cols,
        rows,
        shift,
        cursor: Cursor {
            x: 0,
            y: 0,
            visible: true,
            shape: CursorShape::Block,
            blink: true,
        },
    }
}

/// Stage a whole update; the final occurrence of a row index must win.
fn rows(entries: Vec<(u16, Row)>) -> GridOutput {
    GridOutput::Rows {
        rows: entries
            .into_iter()
            .map(|(y, row)| ScreenRow { y, row })
            .collect(),
        links: Vec::new(),
    }
}

/// Append history at its own width, independently of the current screen.
fn history(first: u64, cols: u16, rows: Vec<Row>) -> GridOutput {
    GridOutput::History {
        first,
        cols,
        rows,
        links: Vec::new(),
    }
}

/// Catch-up is hidden until both the first screen and the separate relay
/// completion notice arrive. Even an empty rows message opens an update, so a
/// completion notice between it and its frame cannot release the placeholder.
#[test]
fn replay_visibility_and_atomic_screen_commits() {
    let mut model = GridModel::new();
    assert!(model.mark_replay_complete().is_err());
    model = opened(4, 2, 10);
    send(&mut model, rows(vec![(0, row(false, &[(0, 3, "old")]))]));
    assert_eq!(model.terminal_text(), "\n");
    assert!(model.mark_replay_complete().is_err());
    send(&mut model, rows(vec![(0, row(false, &[(0, 3, "new")]))]));
    send(&mut model, frame(1, 4, 2, 0));
    assert_eq!(model.terminal_text(), "new\n");
    assert!(!model.is_visible());
    model.mark_replay_complete().unwrap();
    assert!(model.is_visible());
    assert!(model.take_damage().visibility);
    send(&mut model, rows(Vec::new()));
    assert!(model.mark_replay_complete().is_err());
}

/// Context-free fixtures cannot prove cross-message ordering. The shared
/// checker must reject a missing prefix, repeated hello, missing initial state,
/// and a state update inserted between staged rows and their frame.
#[test]
fn stream_ordering_is_enforced_before_model_mutation() {
    let hello = || GridOutput::Hello {
        cols: 4,
        rows: 2,
        history_cap: 10,
    };
    let mut model = GridModel::new();
    assert!(model.apply(&frame(1, 4, 2, 0).encode().unwrap()).is_err());
    send(&mut model, hello());
    assert!(model.apply(&frame(1, 4, 2, 0).encode().unwrap()).is_err());
    assert!(model.apply(&hello().encode().unwrap()).is_err());

    let mut model = opened(4, 2, 10);
    let old_state = model.state().unwrap();
    send(&mut model, rows(vec![(0, row(false, &[(0, 3, "new")]))]));
    let mut new_state = old_state;
    new_state.focus = true;
    assert!(
        model
            .apply(&GridOutput::State(new_state).encode().unwrap())
            .is_err()
    );
    assert_eq!(model.state(), Some(old_state));
    assert_eq!(model.revision(), None);
}

/// The GPUI app's helm client hands the model messages it already decoded, so `apply_output` is the path the app uses.
/// It must refuse exactly what `apply` refuses: skipping the ordering checker because decoding happened elsewhere would
/// let a misordered stream mutate the screen. Spec: a frame before the hello, and a second hello, are refused without
/// mutation; in-order decoded messages build the same screen as encoded ones.
#[test]
fn already_decoded_output_gets_the_same_ordering_checks() {
    let hello = || GridOutput::Hello {
        cols: 4,
        rows: 2,
        history_cap: 10,
    };
    let mut model = GridModel::new();
    assert!(model.apply_output(frame(1, 4, 2, 0)).is_err());
    let _ = model.apply_output(hello()).unwrap();
    assert!(model.apply_output(hello()).is_err());

    let mut decoded = opened(4, 2, 10);
    let mut encoded = opened(4, 2, 10);
    let update = || rows(vec![(0, row(false, &[(0, 3, "new")]))]);
    let _ = decoded.apply_output(update()).unwrap();
    let _ = decoded.apply_output(frame(1, 4, 2, 0)).unwrap();
    send(&mut encoded, update());
    send(&mut encoded, frame(1, 4, 2, 0));
    assert_eq!(decoded.screen(), encoded.screen());
    assert_eq!(decoded.revision(), Some(1));
}

/// A refused frame cannot expose buffered rows or change the committed size,
/// cursor, revision, or repaint record. The upstream checker also retains its
/// pending bounds until a valid committing frame arrives.
#[test]
fn invalid_frame_leaves_committed_and_pending_state_unchanged() {
    let mut model = opened(4, 2, 10);
    send(&mut model, frame(1, 4, 2, 0));
    model.take_damage();
    send(&mut model, rows(vec![(1, row(false, &[(0, 4, "next")]))]));
    let bad = frame(2, 3, 2, 0).encode().unwrap();
    assert!(model.apply(&bad).is_err());
    assert_eq!(model.size(), (4, 2));
    assert_eq!(model.revision(), Some(1));
    assert_eq!(model.terminal_text(), "\n");
    assert_eq!(model.take_damage(), Default::default());
    send(&mut model, frame(2, 4, 2, 0));
    assert_eq!(model.terminal_text(), "\nnext");
    assert!(model.apply(&frame(2, 4, 2, 0).encode().unwrap()).is_err());
}

/// A scroll commits by moving old rows before replacing new rows. Damage from
/// several unpainted frames must move with those rows; resize clears every old
/// row rather than letting stale cells survive at the new dimensions.
#[test]
fn shifts_resize_and_coalesced_damage() {
    let mut model = opened(4, 3, 10);
    send(
        &mut model,
        rows(vec![
            (0, row(false, &[(0, 1, "a")])),
            (1, row(false, &[(0, 1, "b")])),
            (2, row(false, &[(0, 1, "c")])),
        ]),
    );
    send(&mut model, frame(1, 4, 3, 0));
    model.take_damage();
    send(&mut model, rows(vec![(0, row(false, &[(0, 1, "x")]))]));
    send(&mut model, frame(2, 4, 3, 0));
    send(&mut model, history(0, 4, vec![row(false, &[(0, 1, "x")])]));
    send(&mut model, frame(3, 4, 3, 1));
    send(&mut model, history(1, 4, vec![row(false, &[(0, 1, "b")])]));
    send(&mut model, frame(4, 4, 3, 1));
    assert_eq!(model.terminal_text(), "c\n\n");
    let damage = model.take_damage();
    assert_eq!(damage.shift, 2);
    assert_eq!(damage.rows.into_iter().collect::<Vec<_>>(), [1, 2]);
    assert_eq!(model.line(1).unwrap().1.text(0, 4), "b   ");
    send(&mut model, rows(vec![(1, row(false, &[(0, 2, "ok")]))]));
    send(&mut model, frame(5, 2, 2, 1));
    assert_eq!(model.terminal_text(), "\nok");
    let damage = model.take_damage();
    assert!(damage.size && damage.all);
}

/// Absolute line numbers and original history widths survive cap eviction;
/// reset deliberately invalidates them and permits rebuilding at a new base.
/// Empty appends and cap-zero attachments still establish screen numbering.
#[test]
fn history_anchors_caps_empty_appends_and_reset() {
    let mut model = opened(4, 2, 2);
    send(
        &mut model,
        history(
            40,
            8,
            vec![
                row(false, &[(0, 5, "first")]),
                row(false, &[(0, 4, "kept")]),
            ],
        ),
    );
    send(
        &mut model,
        history(42, 3, vec![row(false, &[(0, 3, "new")])]),
    );
    assert!(model.line(40).is_none());
    assert_eq!(model.line(41).unwrap().0, 8);
    assert_eq!(model.line(41).unwrap().1.text(0, 8), "kept    ");
    assert_eq!(model.screen_base(), 43);
    assert!(
        model
            .apply(&history(99, 4, Vec::new()).encode().unwrap())
            .is_err()
    );
    assert_eq!(model.screen_base(), 43);
    model.take_damage();
    send(&mut model, GridOutput::HistoryReset);
    assert!(model.take_damage().history_reset);
    assert!(model.history().is_empty());
    send(&mut model, history(7, 9, Vec::new()));
    assert_eq!(model.screen_base(), 7);
    assert!(model.line(41).is_none());

    let mut zero = opened(1, 1, 0);
    send(&mut zero, history(100, 1, vec![row(false, &[]); 3]));
    assert!(zero.history().is_empty());
    assert_eq!(zero.screen_base(), 103);
}

/// History row counts are bounded by encoded bytes, not by an invented cap
/// per message. A valid append larger than the retention cap is accepted and
/// retains precisely its newest lines, including every line number.
#[test]
fn large_valid_history_append_is_not_rejected_by_a_client_only_count_limit() {
    let mut model = opened(1, 1, 2);
    send(&mut model, history(500, 1, vec![row(false, &[]); 100_001]));
    assert_eq!(
        model
            .history()
            .iter()
            .map(|line| line.number)
            .collect::<Vec<_>>(),
        [100_499, 100_500]
    );
    assert_eq!(model.screen_base(), 100_501);
}

/// Long OSC 8 targets must be allocated once per message table entry rather
/// than once per cell. Sharing survives across rows and history cap eviction;
/// unused or skipped rows must not keep extra references to the shared target.
#[test]
fn osc8_targets_are_shared_across_runs_and_rows() {
    let mut model = opened(10, 3, 2);
    let mut linked = row(false, &[]);
    linked.runs = (0..10)
        .map(|col| Run {
            col,
            width: 1,
            text: "x".into(),
            style: Style::PLAIN,
            link: Some(0),
        })
        .collect();
    let target = Link {
        id: "i".repeat(250),
        uri: format!("https://example.test/{}", "x".repeat(2000)),
    };
    send(
        &mut model,
        GridOutput::Rows {
            rows: (0..3)
                .map(|y| ScreenRow {
                    y,
                    row: linked.clone(),
                })
                .collect(),
            links: vec![target.clone()],
        },
    );
    send(&mut model, frame(1, 10, 3, 0));
    let first = model.screen()[0].runs[0].link.as_ref().unwrap();
    for row in model.screen() {
        for run in &row.runs {
            assert!(Arc::ptr_eq(first, run.link.as_ref().unwrap()));
        }
    }
    assert_eq!(Arc::strong_count(first), 30);
    assert_eq!(first.as_ref(), &target);

    send(
        &mut model,
        GridOutput::History {
            first: 40,
            cols: 10,
            rows: vec![linked; 3],
            links: vec![target.clone()],
        },
    );
    let first = model.history()[0].row.runs[0].link.as_ref().unwrap();
    for line in model.history() {
        for run in &line.row.runs {
            assert!(Arc::ptr_eq(first, run.link.as_ref().unwrap()));
        }
    }
    assert_eq!(Arc::strong_count(first), 20);
    assert_eq!(first.as_ref(), &target);
    assert!(!Arc::ptr_eq(
        first,
        model.screen()[0].runs[0].link.as_ref().unwrap()
    ));
}

/// Selection endpoints may lie anywhere in u64, while valid retained lines
/// can start at the protocol's last history number and screen rows follow it.
/// Clamping must neither underflow on an unopened model nor loop over a huge
/// absent prefix/suffix, and screen numbering must agree with `line()`.
#[test]
fn selection_clamps_to_retained_lines_at_numeric_boundaries() {
    let pos = |line, col| Position { line, col };
    assert_eq!(
        GridModel::new().selection_text(pos(0, 0), pos(u64::MAX, 1), SelectionMode::Linear),
        ""
    );
    let mut model = opened(1, 2, 1);
    let last_history = farhelm_proto::grid::GRID_MAX_LINE_NUMBER;
    send(
        &mut model,
        history(last_history, 1, vec![row(false, &[(0, 1, "h")])]),
    );
    send(
        &mut model,
        rows(vec![
            (0, row(false, &[(0, 1, "a")])),
            (1, row(false, &[(0, 1, "b")])),
        ]),
    );
    send(&mut model, frame(1, 1, 2, 0));
    assert_eq!(model.screen_base(), last_history + 1);
    assert_eq!(model.line(last_history + 2).unwrap().1.text(0, 1), "b");
    assert_eq!(
        model.selection_text(pos(0, 0), pos(u64::MAX, 1), SelectionMode::Linear),
        "h\na\nb"
    );
    assert_eq!(
        model.selection_text(
            pos(last_history + 1, 0),
            pos(last_history + 1, 1),
            SelectionMode::Linear
        ),
        "a"
    );
    assert_eq!(
        model.selection_text(pos(u64::MAX, 0), pos(u64::MAX, 1), SelectionMode::Linear),
        ""
    );
}

/// OSC 52 effects are delivered only when complete, without retaining a queue
/// of completed writes. A primary-selection write must never overwrite the
/// user's clipboard, but its completed-write effect suppresses misleading copy
/// guidance. Interrupted writes fail the same checker as screen data.
#[test]
fn clipboard_parts_are_atomic_and_primary_is_ignored() {
    let mut model = opened(1, 1, 0);
    let part = |target, text: &str, last| {
        GridOutput::Clipboard {
            target,
            text: text.into(),
            last,
        }
        .encode()
        .unwrap()
    };
    let partial = model
        .apply(&part(ClipboardTarget::Clipboard, "one ", false))
        .unwrap();
    assert_eq!(partial.clipboard_text, None);
    assert!(!partial.clipboard_written);
    assert!(model.apply(&frame(1, 1, 1, 0).encode().unwrap()).is_err());
    assert_eq!(
        model
            .apply(&part(ClipboardTarget::Clipboard, "中", true))
            .unwrap()
            .clipboard_text
            .as_deref(),
        Some("one 中")
    );
    let primary = model
        .apply(&part(ClipboardTarget::Primary, "ignored", true))
        .unwrap();
    assert_eq!(primary.clipboard_text, None);
    assert!(primary.clipboard_written);
    assert_eq!(
        model
            .apply(&part(ClipboardTarget::Clipboard, "next", true))
            .unwrap()
            .clipboard_text
            .as_deref(),
        Some("next")
    );
}

/// Copying joins soft wraps at each line's original width, skips the unused
/// cell before a wide wrap, keeps combining marks, and excludes a wide glyph
/// whose first cell lies before the selection. Rectangles never join wraps.
#[test]
fn selection_uses_cells_original_widths_and_farhelm_trimming() {
    let mut model = opened(4, 2, 10);
    send(
        &mut model,
        history(
            40,
            5,
            vec![
                row(true, &[(0, 4, "abcd")]),
                row(false, &[(0, 2, "中"), (2, 1, "e\u{301}")]),
            ],
        ),
    );
    send(
        &mut model,
        rows(vec![
            (0, row(true, &[(0, 1, "a"), (2, 1, "b")])),
            (1, row(false, &[(0, 1, "c"), (1, 1, "\u{a0}"), (2, 1, "d")])),
        ]),
    );
    send(&mut model, frame(1, 4, 2, 0));
    let pos = |line, col| Position { line, col };
    assert_eq!(
        model.selection_text(pos(40, 0), pos(41, 5), SelectionMode::Linear),
        "abcd中e\u{301}"
    );
    assert_eq!(
        model.selection_text(pos(41, 1), pos(41, 3), SelectionMode::Linear),
        "e\u{301}"
    );
    assert_eq!(
        model.selection_text(pos(41, 0), pos(41, 1), SelectionMode::Linear),
        "中"
    );
    assert_eq!(
        model.selection_text(pos(42, 0), pos(43, 4), SelectionMode::Linear),
        "a b c d"
    );
    assert_eq!(
        model.selection_text(pos(43, 4), pos(42, 0), SelectionMode::Linear),
        "a b c d"
    );
    assert_eq!(
        model.selection_text(pos(42, 0), pos(43, 3), SelectionMode::Column),
        "a b\nc d"
    );
    assert_eq!(
        model.selection_text(pos(0, 0), pos(u64::MAX, 4), SelectionMode::Column),
        "abcd\n中e\u{301}\na b\nc d"
    );
}

/// Double/triple-click ranges follow cells across soft wraps, including the
/// unusual clicked-separator rule and the padding before a wrapped wide glyph.
#[test]
fn selection_word_and_line_ranges_match_cell_owned_text() {
    let mut model = opened(8, 2, 10);
    send(&mut model, frame(1, 8, 2, 0));
    send(
        &mut model,
        history(
            100,
            8,
            vec![
                row(true, &[(0, 8, "(foo)  x")]),
                row(false, &[(0, 4, "yz a")]),
                row(true, &[(0, 7, "abcdefg")]),
                row(false, &[(0, 2, "中"), (2, 1, "z")]),
            ],
        ),
    );
    let pos = |line, col| Position { line, col };
    for (at, start, end, text) in [
        (pos(100, 0), pos(100, 0), pos(100, 4), "(foo"),
        (pos(100, 2), pos(100, 1), pos(100, 4), "foo"),
        (pos(100, 5), pos(100, 5), pos(100, 7), ""),
        (pos(100, 7), pos(100, 7), pos(101, 2), "xyz"),
        (pos(103, 1), pos(102, 0), pos(103, 3), "abcdefg中z"),
    ] {
        let range = model.word_range(at).unwrap();
        assert_eq!(range, Selection::new(start, end, SelectionMode::Linear));
        assert_eq!(
            model.selection_text(range.start, range.end, range.mode),
            text
        );
    }
    assert_eq!(
        model.line_range(pos(100, 3)),
        Some(Selection::new(
            pos(100, 0),
            pos(101, 8),
            SelectionMode::Linear
        ))
    );
    assert!(model.word_range(pos(99, 0)).is_none());
    let rectangle = Selection::new(pos(102, 6), pos(100, 2), SelectionMode::Column);
    assert_eq!(rectangle.columns(101, 8), 2..6);
    assert_eq!(rectangle.trim_before(101).unwrap().columns(101, 8), 2..6);
    let linear = Selection::new(pos(100, 6), pos(102, 2), SelectionMode::Linear);
    assert_eq!(linear.trim_before(101).unwrap().start, pos(101, 0));
    assert!(linear.trim_before(103).is_none());

    // The JS space search stops at the combining mark inside the next cell,
    // while its returned range still includes that whole cell. Treating a
    // whole fragment as merely "space" would incorrectly include column two.
    let mut marked = opened(4, 1, 0);
    send(
        &mut marked,
        rows(vec![(
            0,
            row(
                false,
                &[(0, 1, " "), (1, 1, " \u{301}"), (2, 1, " "), (3, 1, "x")],
            ),
        )]),
    );
    send(&mut marked, frame(1, 4, 1, 0));
    assert_eq!(
        marked.word_range(pos(0, 0)),
        Some(Selection::new(pos(0, 0), pos(0, 2), SelectionMode::Linear))
    );
}

/// A single soft-wrapped program line can fill the history cap. Word lookup
/// must retain its cell budget, and logical-line lookup its independent row cap;
/// alternate-screen lookup must not wander into the retained primary buffer.
#[test]
fn selection_searches_bound_long_wraps_and_hide_primary_history_on_alt() {
    let mut model = opened(8, 2, 4000);
    send(&mut model, frame(1, 8, 2, 0));
    for chunk in 0..12 {
        send(
            &mut model,
            history(
                100 + chunk * 250,
                8,
                vec![row(true, &[(0, 8, "abcdefgh")]); 250],
            ),
        );
    }
    let pos = |line, col| Position { line, col };
    assert_eq!(
        model.word_range(pos(1600, 3)),
        Some(Selection::new(
            pos(1344, 0),
            pos(1856, 8),
            SelectionMode::Linear
        ))
    );
    assert_eq!(
        model.line_range(pos(1600, 3)),
        Some(Selection::new(
            pos(600, 0),
            pos(2600, 8),
            SelectionMode::Linear
        ))
    );
    let mut state = model.state().unwrap();
    state.alt_screen = true;
    send(&mut model, GridOutput::State(state));
    assert!(model.word_range(pos(1600, 3)).is_none());
    assert_eq!(
        model
            .line_range(pos(model.screen_base(), 0))
            .unwrap()
            .start
            .line,
        model.screen_base()
    );
}

/// Style/default-color state remains typed and lossless; only drawing strips
/// bidi controls. Copying preserves received text, since display sanitization
/// must not silently change the model's evidence for a link or selection.
#[test]
fn display_sanitization_does_not_mutate_received_text() {
    let mut model = opened(3, 1, 0);
    send(
        &mut model,
        rows(vec![(0, row(false, &[(0, 1, "x\u{202e}"), (1, 1, "y")]))]),
    );
    send(&mut model, frame(1, 3, 1, 0));
    assert_eq!(display_text(&model.screen()[0].runs[0].text), "x");
    assert_eq!(model.terminal_text(), "x\u{202e}y");
    let mut state = model.state().unwrap();
    state.default_fg = Some(Color::Rgb { r: 1, g: 2, b: 3 });
    state.focus = true;
    state.mouse = MouseMode::Motion;
    send(&mut model, GridOutput::State(state));
    assert_eq!(model.state(), Some(state));
}

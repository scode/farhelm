//! The renderer's contract with the grid protocol: how wire styles become paint, how damage maps onto the row cache,
//! and that every change the screen shows redraws the view, and nothing else does.
//!
//! The fixtures are the GPUI prototype's recordings in the grid protocol's JSON form (`tests/fixtures/terminal/`, with
//! their provenance in its README); tests encode them through the real encoder before the model sees them, so a fixture
//! cannot bypass the protocol's width, style and ordering checks. Pixel output is the screenshot goldens' job
//! (`goldens.rs`); these tests pin geometry and redraws, which the fake text system can show. The one test that needs
//! real font metrics, the cell's size, measures the bundled font in a headless app (`test_support::headless`), which
//! needs no renderer until something is drawn.

use std::rc::Rc;

use farhelm_grid_model::{ClipboardTarget, Color, GridOutput, Style, style_flags as flags};
use gpui::{AppContext as _, Entity, TestAppContext};

use super::*;
use crate::render_count;
use crate::test_support;
use crate::theme::{self, TerminalRole};

const MATRIX: &str = include_str!("../../tests/fixtures/terminal/matrix.jsonl");
const ATTACH: &str = include_str!("../../tests/fixtures/terminal/attach.jsonl");

/// A fixture's messages, each round-tripped through the binary encoder so the model sees exactly what a live
/// attachment would hand it.
fn messages(recording: &str) -> Vec<GridOutput> {
    recording
        .lines()
        .map(|json| {
            let message: GridOutput = serde_json::from_str(json).unwrap();
            GridOutput::decode(&message.encode().unwrap()).unwrap()
        })
        .collect()
}

/// Fixed metrics for tests that need no real font.
fn metrics() -> CellMetrics {
    CellMetrics::new(10., 22., 16.).unwrap()
}

/// A view fed `recording` without a GPUI context, through the same private path `apply` uses.
fn fed(recording: &str, replay_complete: bool) -> TerminalView {
    let mut view = TerminalView::new(metrics());
    for message in messages(recording) {
        let _ = view.model.apply_output(message).unwrap();
    }
    if replay_complete {
        view.model.mark_replay_complete().unwrap();
    }
    let damage = view.model.take_damage();
    view.update_grid(damage);
    view
}

/// The palette and the 256-color index are the old app's (Ghostty's defaults and the xterm cube); bold brightens only
/// ANSI 0-7 and only the foreground and underline; dim halves the foreground's alpha but never the background's or an
/// explicit underline's; inverse swaps the roles before bold promotes, so bold brightens the color that ends up in the
/// foreground; an explicit underline color reaches a strikethrough only on underlined text; the five underline bits map
/// to their shapes. A drift here recolors every program's output.
#[test]
fn styles_resolve_with_the_old_apps_precedence() {
    let rgb = |index: u8| {
        theme::terminal_color(
            Color::Palette(index),
            TerminalRole {
                default: TerminalColor::of(TERMINAL.foreground),
                bright: false,
            },
        )
    };
    assert_eq!(rgb(1), TerminalColor::of(TERMINAL.ansi[1]));
    assert_eq!(format!("{:?}", rgb(16)), "TerminalColor(0)");
    assert_eq!(format!("{:?}", rgb(231)), "TerminalColor(16777215)");
    assert_eq!(format!("{:?}", rgb(232)), "TerminalColor(526344)");
    assert_eq!(format!("{:?}", rgb(255)), "TerminalColor(15658734)");
    for index in 0..8 {
        let s = style::resolve(
            Style {
                flags: flags::BOLD | flags::DIM | flags::UNDERLINE,
                fg: Color::Palette(index),
                bg: Color::Palette(index),
                underline_color: Some(Color::Palette(index)),
            },
            None,
        );
        assert_eq!(s.fg, rgb(index + 8));
        assert_eq!(s.bg, rgb(index));
        assert_eq!(s.underline_color, rgb(index + 8));
        assert_eq!((s.alpha, s.underline_alpha), (0.5, 1.0));
    }
    // Bold leaves the bright colors and the 256-color index alone.
    for index in [9, 15, 16, 196] {
        let s = style::resolve(
            Style {
                flags: flags::BOLD,
                fg: Color::Palette(index),
                ..Style::PLAIN
            },
            None,
        );
        assert_eq!(s.fg, rgb(index), "bold on index {index}");
    }
    // Inverse first: the background's ANSI 2 moves to the foreground and bold brightens it; the foreground's ANSI 1
    // becomes the background unbrightened. Bold before the swap would give fg 2 and bg 9.
    let swapped = style::resolve(
        Style {
            flags: flags::INVERSE | flags::BOLD,
            fg: Color::Palette(1),
            bg: Color::Palette(2),
            underline_color: None,
        },
        None,
    );
    assert_eq!((swapped.fg, swapped.bg), (rgb(10), rgb(1)));
    // SGR 58 without SGR 4: the strike keeps the dim text's color and alpha. Underlined, the explicit color applies at
    // full alpha.
    let colored = |extra| {
        style::resolve(
            Style {
                flags: flags::STRIKETHROUGH | flags::DIM | extra,
                fg: Color::Palette(1),
                bg: Color::Default,
                underline_color: Some(Color::Palette(4)),
            },
            None,
        )
    };
    let struck = colored(0);
    assert_eq!(
        (struck.underline_color, struck.underline_alpha),
        (rgb(1), 0.5)
    );
    let underlined = colored(flags::UNDERLINE);
    assert_eq!(
        (underlined.underline_color, underlined.underline_alpha),
        (rgb(4), 1.0)
    );
    let inverse = style::resolve(
        Style {
            flags: flags::INVERSE,
            ..Style::PLAIN
        },
        None,
    );
    assert_eq!(inverse.fg, TerminalColor::of(TERMINAL.background));
    assert_eq!(inverse.bg, TerminalColor::of(TERMINAL.foreground));
    for (flag, expected) in [
        (flags::UNDERLINE, Underline::Solid),
        (flags::DOUBLE_UNDERLINE, Underline::Double),
        (flags::UNDERCURL, Underline::Wavy),
        (flags::DOTTED_UNDERLINE, Underline::Dotted),
        (flags::DASHED_UNDERLINE, Underline::Dashed),
    ] {
        let resolved = style::resolve(
            Style {
                flags: flag,
                ..Style::PLAIN
            },
            None,
        );
        assert_eq!(resolved.underline, Some(expected));
    }
}

/// A program's OSC 10/11 colors replace the defaults for default-styled cells and the pane ground; inverse then swaps
/// them. The matrix fixture sets both.
#[test]
fn program_default_colors_override_the_palettes() {
    let view = fed(MATRIX, true);
    let (fg, bg) = style::defaults(view.state);
    assert_ne!(fg, TerminalColor::of(TERMINAL.foreground));
    assert_ne!(bg, TerminalColor::of(TERMINAL.background));
    let inverse = style::resolve(
        Style {
            flags: flags::INVERSE,
            ..Style::PLAIN
        },
        view.state,
    );
    assert_eq!((inverse.fg, inverse.bg), (bg, fg));
}

/// The row cache keeps every run at the column the wire gave it (the markers after one- and two-cell CJK, emoji,
/// variation and combining sequences), and automation reads the received text. Nothing is shown before the replay
/// completes, so a half-replayed screen never reaches the bridge either. Painting places a run at its cached column
/// times the cell width; that this keeps fallback glyphs from moving later cells is the goldens' job to show.
#[test]
fn the_cache_keeps_wire_columns_and_the_placeholder_hides_the_replay() {
    let behind = fed(MATRIX, false);
    assert!(behind.terminal_text().is_none());
    let view = fed(MATRIX, true);
    let text = view.terminal_text().unwrap();
    let markers: Vec<_> = view.rows[11]
        .row
        .runs
        .iter()
        .filter(|r| r.text == "|")
        .map(|r| r.col)
        .collect();
    assert_eq!(markers, [2, 5, 10, 13, 18, 21, 25, 30, 34, 38]);
    assert!(text.contains("e\u{301}"));
}

/// A cursor on a wide character's spacer cell covers the whole two-cell box; on plain ASCII it covers one cell.
#[test]
fn a_cursor_covers_a_wide_characters_whole_box() {
    let view = fed(MATRIX, true);
    let element = TerminalElement {
        rows: view.rows.clone(),
        cols: view.cols,
        cursor: view.cursor,
        visible: view.visible,
        state: view.state,
        metrics: view.metrics,
        phase: view.phase,
        cell: Size::default(),
    };
    let mut cursor = view.cursor.unwrap();
    (cursor.x, cursor.y) = (1, 11);
    assert_eq!(element.cursor_cell(cursor), (0, 2));
    (cursor.x, cursor.y) = (7, 14);
    assert_eq!(element.cursor_cell(cursor), (7, 1));
}

/// Damage is final-coordinate: a frame that shifts the screen by one keeps the shifted row's cache entry (its shaped
/// glyphs survive a scroll), replaces the changed row, and an equal rewrite keeps the old entry and needs no redraw.
#[test]
fn a_scroll_keeps_shaped_rows_and_equal_rewrites_change_nothing() {
    let mut view = fed(ATTACH, true);
    let retained = view.rows[1].clone();
    let changed: GridOutput = serde_json::from_value(serde_json::json!({
        "t":"rows", "rows":[[2,{"w":false,"r":[[0,7,"changed",[0,-1,-1]]]}]]
    }))
    .unwrap();
    let frame: GridOutput = serde_json::from_value(serde_json::json!({
        "t":"frame", "rev":2,"cols":20,"rows":4,"shift":1,
        "cursor":{"x":2,"y":2,"visible":true,"shape":"bar","blink":false}
    }))
    .unwrap();
    let _ = view.model.apply_output(changed).unwrap();
    let _ = view.model.apply_output(frame).unwrap();
    let damage = view.model.take_damage();
    assert!(view.update_grid(damage));
    assert!(Rc::ptr_eq(&retained, &view.rows[0]));
    assert_eq!(view.rows[2].row.text(0, 20).trim_end(), "changed");
    let current = view.rows[0].clone();
    let mut damage = Damage::default();
    damage.rows.insert(0);
    assert!(!view.update_grid(damage));
    assert!(Rc::ptr_eq(&current, &view.rows[0]));
}

/// Only whole sizes from 9 to 28 points reach the text system (the glyph atlas never evicts, so the size set bounds
/// what zooming adds), and unusable geometry is refused instead of asserting inside a platform callback.
#[test]
fn text_sizes_are_whole_points_in_range_and_bad_geometry_is_refused() {
    assert_eq!(TextSize::new(3).points(), TextSize::MIN);
    assert_eq!(TextSize::new(200).points(), TextSize::MAX);
    assert_eq!(TextSize::DEFAULT.points(), 14);
    assert!(CellMetrics::new(0., 22., 16.).is_none());
    assert!(CellMetrics::new(10., f32::NAN, 16.).is_none());
    assert!(CellMetrics::new(10., 22., -1.).is_none());
}

/// The cell is the old app's: the bundled font's advance by its ascent plus descent, rounded as the old app's
/// `cellFor` did. A taller or wider cell fits fewer rows and columns than the old app did and leaves gaps between box-
/// drawing and block glyphs, which Claude Code and Codex draw their borders with.
///
/// Spec: JetBrains Mono Nerd Font's advance is 600 and its ascent plus descent 1320 per 1000 units (hhea 1020/-300), so
/// at every supported size the measurement is 0.6 and 1.32 times the size, whatever sign the platform gives the
/// descent; at 14 points an 80 by 24 grid draws 8.4 by 19 cells at scale 1 and 8.4 by 18.5 at scale 2.
#[test]
fn the_cell_is_the_bundled_fonts_advance_by_its_ascent_plus_descent() {
    let mut cx = test_support::headless().unwrap();
    for points in TextSize::MIN..=TextSize::MAX {
        let m = cx
            .update(|cx| CellMetrics::measure(TextSize::new(points), cx))
            .unwrap();
        let size = f32::from(points);
        assert!(
            (m.char_width - 0.6 * size).abs() < 1e-3 && (m.char_height - 1.32 * size).abs() < 1e-3,
            "{points} points measured {m:?}"
        );
        assert_eq!(m.font_size, size);
    }
    let m = cx
        .update(|cx| CellMetrics::measure(TextSize::DEFAULT, cx))
        .unwrap();
    let rounded = |cell: Size<f32>| ((cell.width * 1e3).round(), (cell.height * 1e3).round());
    assert_eq!(rounded(m.cell_size(80, 24, 1.)), (8400., 19000.));
    assert_eq!(rounded(m.cell_size(80, 24, 2.)), (8400., 18500.));
    assert_eq!(m.cell_size(0, 0, 2.), Size::default());
}

/// The device-pixel baseline GPUI's `paint_line` and `paint_glyph` give a line handed origin `y`: the line height
/// minus ascent and descent, halved, plus the ascent, scaled and rounded half toward zero (GPUI keeps one vertical
/// subpixel variant, so every glyph sits on a whole device pixel). Restated here because the seam it causes is the
/// point of the test below; if GPUI changes this arithmetic, the goldens' stacked blocks are the check that notices.
fn gpui_baseline(y: f32, cell_height: f32, ascent: f32, descent: f32, scale: f32) -> i32 {
    let device = (y + (cell_height - ascent - descent) / 2. + ascent) * scale;
    (device.abs() - 0.5).ceil().copysign(device) as i32
}

/// Stacked rows of full blocks and vertical box-drawing lines must not be split by baseline rounding (blocks still
/// stripe at most text sizes for another reason: `Baselines`' NOTE). The old app's cell is usually a fraction of a
/// device pixel off a whole one, and GPUI rounds each glyph row's baseline on its own, so rows placed at their cell's
/// top step 37 or 38 device pixels while the glyph is 37 tall: every few rows a stripe of background splits a border
/// that Claude Code or Codex drew. The styles golden shows one such pair; this pins the rule for every row.
///
/// Spec: in the styles golden's geometry (a 9-row grid at 14 points on a 2x display, its top at 16.5 logical pixels:
/// cells of 167/9 logical pixels, ascent 14.28 and descent 4.2), rows placed at their cell's top step by 38 somewhere
/// (the premise), while rows placed by `Baselines` step by exactly 37 device pixels everywhere, the first row lands
/// where GPUI alone puts it, and no row is more than a device pixel from where GPUI alone puts it. At scale 1, where
/// the cell is a whole number of device pixels, `Baselines` changes nothing.
#[test]
fn row_baselines_step_by_whole_device_rows() {
    let (ascent, descent) = (14.28, 4.2);
    let metrics = CellMetrics::new(8.4, ascent + descent, 14.).unwrap();
    for (scale, top, fractional) in [(2., 16.5, true), (1., 16., false)] {
        let cell = metrics.cell_size(36, 9, scale);
        let baselines = Baselines {
            top,
            cell_height: cell.height,
            device_row: metrics.device_row(scale),
            scale,
        };
        let natural: Vec<i32> = (0..9)
            .map(|row| {
                gpui_baseline(
                    top + row as f32 * cell.height,
                    cell.height,
                    ascent,
                    descent,
                    scale,
                )
            })
            .collect();
        let snapped: Vec<i32> = (0..9)
            .map(|row| {
                gpui_baseline(
                    baselines.origin_y(row, ascent, descent),
                    cell.height,
                    ascent,
                    descent,
                    scale,
                )
            })
            .collect();
        let steps = |b: &[i32]| b.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();
        let row = metrics.device_row(scale) as i32;
        if fractional {
            assert_eq!(row, 37);
            assert!(
                steps(&natural).contains(&38),
                "premise: unsnapped rows should leave a seam here, got steps {:?}",
                steps(&natural)
            );
        } else {
            assert_eq!(snapped, natural, "a whole-pixel cell must not move any row");
        }
        assert!(
            steps(&snapped).iter().all(|&s| s == row),
            "scale {scale}: steps {:?}",
            steps(&snapped)
        );
        assert_eq!(snapped[0], natural[0]);
        for (s, n) in snapped.iter().zip(&natural) {
            assert!(
                (s - n).abs() <= 1,
                "scale {scale}: {snapped:?} against {natural:?}"
            );
        }
    }
}

/// A terminal window with one view, the render counter reset after its first frame.
fn terminal_window(cx: &mut TestAppContext) -> (Entity<TerminalView>, gpui::AnyWindowHandle) {
    test_support::install(cx);
    let mut view = None;
    let window = cx.add_window(|_, cx| {
        let terminal = cx.new(|_| TerminalView::new(metrics()));
        view = Some(terminal.clone());
        Host(terminal)
    });
    cx.run_until_parked();
    cx.update(render_count::reset);
    (view.unwrap(), window.into())
}

/// The window's root: draws the terminal and nothing else, so the counter sees only the terminal's renders.
struct Host(Entity<TerminalView>);

impl gpui::Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        self.0.clone()
    }
}

/// The terminal notifies itself: a delivery that changes what it shows redraws it, and a delivery it does not show
/// (history the screen does not include) redraws nothing. A terminal that missed a notification would show a stale
/// screen; one that redrew for output it does not show would spend frames on every history append during a flood.
/// (GPUI folds every notify within one update into one frame, so this cannot tell one notify per batch from one per
/// message.)
#[gpui::test]
fn shown_output_redraws_and_hidden_output_does_not(cx: &mut TestAppContext) {
    let (terminal, _window) = terminal_window(cx);
    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply(messages(ATTACH), cx);
        applied.unwrap();
        view.mark_replay_complete(cx).unwrap();
    });
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| render_count::count("terminal", cx)), 1);

    cx.update(render_count::reset);
    let history: GridOutput = serde_json::from_value(serde_json::json!({
        "t":"history","first":44,"rows":[{"w":false,"r":[[0,4,"five",[0,-1,-1]]]}],"cols":20
    }))
    .unwrap();
    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply([history], cx);
        applied.unwrap();
    });
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| render_count::count("terminal", cx)),
        0,
        "history the screen does not show redraws nothing"
    );

    let rows: GridOutput = serde_json::from_value(serde_json::json!({
        "t":"rows", "rows":[[3,{"w":false,"r":[[0,3,"new",[0,-1,-1]]]}]]
    }))
    .unwrap();
    let frame: GridOutput = serde_json::from_value(serde_json::json!({
        "t":"frame", "rev":2,"cols":20,"rows":4,
        "cursor":{"x":3,"y":3,"visible":true,"shape":"block","blink":true}
    }))
    .unwrap();
    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply([rows, frame], cx);
        applied.unwrap();
    });
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| render_count::count("terminal", cx)), 1);
    assert!(
        terminal
            .read_with(cx, |view, _| view.terminal_text())
            .unwrap()
            .contains("new")
    );
}

/// A refused message ends the batch with its error and changes nothing itself, but what came before it in the batch
/// is shown and its effects are delivered: the owner ends the attachment on the error, the screen it leaves is the
/// last valid one, and a clipboard write that completed before the refusal still reaches the owner, because the model
/// hands out a completed write only once and dropping it would lose the copy the program made.
///
/// Spec: the fixture's output, a completed OSC 52 clipboard write, then a second hello: the hello is refused, the
/// returned effects carry the write's text, and the fixture's screen is shown.
#[gpui::test]
fn a_refused_message_keeps_the_valid_output_and_effects_before_it(cx: &mut TestAppContext) {
    let (terminal, _window) = terminal_window(cx);
    let mut batch = messages(ATTACH);
    let hello_again = batch[0].clone();
    batch.push(GridOutput::Clipboard {
        target: ClipboardTarget::Clipboard,
        text: "copied".into(),
        last: true,
    });
    batch.push(hello_again);
    let (effects, refused) = terminal.update(cx, |view, cx| {
        let (effects, result) = view.apply(batch, cx);
        view.mark_replay_complete(cx).unwrap();
        (effects, result)
    });
    assert!(refused.is_err(), "a second hello is refused");
    assert!(effects.clipboard_written);
    assert_eq!(effects.clipboard_text.as_deref(), Some("copied"));
    assert!(
        terminal
            .read_with(cx, |view, _| view.terminal_text())
            .unwrap()
            .contains("hello")
    );
}

/// A cursor phase change that changes nothing redraws nothing; a real change redraws once. Blink drives this twice a
/// second, so a redraw on an unchanged phase would be a steady cost on an idle terminal.
#[gpui::test]
fn an_unchanged_phase_redraws_nothing(cx: &mut TestAppContext) {
    let (terminal, _window) = terminal_window(cx);
    terminal.update(cx, |view, cx| {
        view.set_cursor_phase(CursorPhase::default(), cx);
    });
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| render_count::count("terminal", cx)), 0);
    terminal.update(cx, |view, cx| {
        view.set_cursor_phase(
            CursorPhase {
                focused: true,
                on: false,
            },
            cx,
        );
    });
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| render_count::count("terminal", cx)), 1);
}

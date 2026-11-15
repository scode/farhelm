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
use crate::components::ScrollbarLines;
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
    view.update_shown();
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
        rows: view.shown.clone(),
        cursor_row: None,
        selected: Vec::new(),
        underlined: Vec::new(),
        cols: view.cols,
        cursor: view.cursor,
        visible: view.visible,
        state: view.state,
        metrics: view.metrics,
        phase: view.phase,
        cell: Size::default(),
        notice: None,
    };
    let mut cursor = view.cursor.unwrap();
    (cursor.x, cursor.y) = (1, 11);
    assert_eq!(element.cursor_cell(cursor, 11), (0, 2));
    (cursor.x, cursor.y) = (7, 14);
    assert_eq!(element.cursor_cell(cursor, 14), (7, 1));
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

/// Every combination of bold and italic draws in a bundled face of its own. GPUI never slants or emboldens a face
/// itself, so a style whose request lands on another face draws wrong without any error: italic text drew upright
/// until the Italic and Bold Italic faces were bundled. The styles golden shows the result as pixels; this names the
/// face selection itself, so a missing face fails here by name.
///
/// Spec: the faces `terminal_face` asks for with neither attribute, bold, italic, and both resolve to four different
/// fonts, and each is a JetBrains Mono face, not the interface font: its advance at 14 points is 0.6 times the size.
#[test]
fn each_bold_and_italic_combination_resolves_to_its_own_bundled_face() {
    let mut cx = test_support::headless().unwrap();
    let faces = cx.update(|cx| {
        [0, flags::BOLD, flags::ITALIC, flags::BOLD | flags::ITALIC].map(|bits| {
            let resolved = style::resolve(
                Style {
                    flags: bits,
                    ..Style::PLAIN
                },
                None,
            );
            let id = cx.text_system().resolve_font(&terminal_face(resolved));
            let advance = cx.text_system().advance(id, px(14.), 'W').unwrap().width;
            (bits, id, f32::from(advance))
        })
    });
    for (i, (bits, id, advance)) in faces.iter().enumerate() {
        assert!(
            (advance - 0.6 * 14.).abs() < 1e-3,
            "flags {bits:#x} resolved to a face with advance {advance}, not JetBrains Mono"
        );
        for (other_bits, other_id, _) in &faces[i + 1..] {
            assert_ne!(
                id, other_id,
                "flags {bits:#x} and {other_bits:#x} resolved to the same face"
            );
        }
    }
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

// ---- Scrollback ------------------------------------------------------------------------------------------------------

/// The view's first lines of text, for the scrollback tests.
fn shown_lines(terminal: &Entity<TerminalView>, cx: &mut TestAppContext) -> Vec<String> {
    terminal
        .read_with(cx, |view, _| view.terminal_text())
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// A terminal attached with the ATTACH fixture: a 4-row screen with three history lines kept (an empty one,
/// "three", "four"; "one" fell off the 3-line cap).
fn attached_with_history(cx: &mut TestAppContext) -> Entity<TerminalView> {
    let (terminal, _window) = terminal_window(cx);
    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply(messages(ATTACH), cx);
        applied.unwrap();
        view.mark_replay_complete(cx).unwrap();
    });
    cx.run_until_parked();
    terminal
}

/// Scrollback is a window of the screen's height onto history then the screen (SPEC.md, "Terminal experience":
/// scrollback stays viewable): scrolling back shows the history lines above the screen, it stops at the oldest line
/// kept, and following again shows the live screen. Scrolled back, new history does not move the view off the text
/// it shows (it is anchored to the line, as in the old app), even when the oldest line falls off the cap.
#[gpui::test]
fn the_view_scrolls_into_history_and_stays_on_its_lines(cx: &mut TestAppContext) {
    let terminal = attached_with_history(cx);
    let screen = shown_lines(&terminal, cx);
    assert_eq!(
        screen[0], "$ echo",
        "the premise: following shows the screen"
    );

    terminal.update(cx, |view, cx| view.scroll_lines(-2, cx));
    cx.run_until_parked();
    let lines = shown_lines(&terminal, cx);
    assert_eq!(&lines[..3], ["three", "four", "$ echo"]);
    assert!(!terminal.read_with(cx, |view, _| view.is_following()));

    terminal.update(cx, |view, cx| view.scroll_lines(-10, cx));
    cx.run_until_parked();
    assert_eq!(
        &shown_lines(&terminal, cx)[..3],
        ["", "three", "four"],
        "stops at the oldest line kept"
    );

    terminal.update(cx, |view, cx| view.scroll_lines(1, cx));
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
        shown_lines(&terminal, cx)[0],
        "three",
        "the view stays on its line while history grows and the oldest falls off"
    );

    terminal.update(cx, |view, cx| view.follow(cx));
    cx.run_until_parked();
    assert_eq!(shown_lines(&terminal, cx)[0], "$ echo");
    assert!(terminal.read_with(cx, |view, _| view.is_following()));
}

/// When recovery lost some history, the notice "Some earlier output could not be recovered" is the line above the
/// oldest history line (SPEC.md, "Terminal experience"; the old app's view), reached by scrolling to the top, and
/// never on the alternate screen, which shows no history.
#[gpui::test]
fn trimmed_history_shows_its_notice_above_the_oldest_line(cx: &mut TestAppContext) {
    let terminal = attached_with_history(cx);
    let trimmed = |alt: bool| -> GridOutput {
        serde_json::from_value(
            serde_json::json!({"t":"state","mouse":"off","focus":false,"alt_screen":alt,
            "default_fg":null,"default_bg":null,"history_trimmed":true,"exit":null}),
        )
        .unwrap()
    };
    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply([trimmed(false)], cx);
        applied.unwrap();
        view.scroll_lines(-10, cx);
    });
    cx.run_until_parked();
    assert_eq!(&shown_lines(&terminal, cx)[..2], [TRIMMED_TEXT, ""]);

    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply([trimmed(true)], cx);
        applied.unwrap();
        view.scroll_lines(-10, cx);
    });
    cx.run_until_parked();
    assert!(
        terminal.read_with(cx, |view, _| view.is_following()),
        "the alternate screen has nothing to scroll to"
    );
    assert!(!shown_lines(&terminal, cx).contains(&TRIMMED_TEXT.to_owned()));
}

/// The wheel moves the view by whole lines and keeps the part of a line it moved past, so a trackpad's small steps
/// add up instead of being lost, as the old app's wheel did; a step at another cell height starts afresh.
#[gpui::test]
fn wheel_pixels_add_up_to_whole_lines(cx: &mut TestAppContext) {
    let terminal = attached_with_history(cx);
    let cell = 20.;
    terminal.update(cx, |view, cx| view.scroll_pixels(8., cell, cx));
    cx.run_until_parked();
    assert!(
        terminal.read_with(cx, |view, _| view.is_following()),
        "under half a line moves nothing"
    );
    terminal.update(cx, |view, cx| view.scroll_pixels(8., cell, cx));
    cx.run_until_parked();
    assert_eq!(
        shown_lines(&terminal, cx)[0],
        "four",
        "two steps of 8 pixels make a line"
    );
    terminal.update(cx, |view, cx| view.scroll_pixels(-40., cell, cx));
    cx.run_until_parked();
    assert!(terminal.read_with(cx, |view, _| view.is_following()));
}

/// Scrolled back, the cursor is drawn on its screen row where that row now shows, lower by the lines scrolled, and not
/// at all once its row has left the view; drawn at its screen position instead, it would sit on a history line the
/// program cannot write to.
#[gpui::test]
fn the_cursor_moves_down_with_its_row_when_scrolled_back(cx: &mut TestAppContext) {
    let terminal = attached_with_history(cx);
    let cursor_row = |cx: &mut TestAppContext| terminal.read_with(cx, |view, _| view.cursor_row());
    assert_eq!(
        cursor_row(cx),
        Some(2),
        "the premise: the fixture's cursor is on screen row 2 of 4"
    );
    terminal.update(cx, |view, cx| view.scroll_lines(-1, cx));
    assert_eq!(cursor_row(cx), Some(3));
    terminal.update(cx, |view, cx| view.scroll_lines(-1, cx));
    assert_eq!(cursor_row(cx), None, "its row is below the view");
}

/// Each way the view moves redraws it, and so does output while it is scrolled back, since new history moves the
/// screen down under a view anchored to its line; output it does not show while it follows still redraws nothing.
/// The other scrollback tests read the view's rows, which it rebuilds whether or not it asks for a frame, so without
/// this a lost notify would leave the window showing stale rows and every test passing.
#[gpui::test]
fn scrolling_and_output_while_scrolled_back_redraw(cx: &mut TestAppContext) {
    let terminal = attached_with_history(cx);
    let history = |first: u64| -> GridOutput {
        serde_json::from_value(serde_json::json!({
            "t":"history","first":first,"rows":[{"w":false,"r":[[0,4,"more",[0,-1,-1]]]}],"cols":20
        }))
        .unwrap()
    };
    let renders = |cx: &mut TestAppContext| {
        cx.run_until_parked();
        let count = cx.update(|cx| render_count::count("terminal", cx));
        cx.update(render_count::reset);
        count
    };
    renders(cx);

    terminal.update(cx, |view, cx| view.scroll_lines(-1, cx));
    assert_eq!(renders(cx), 1, "a line scroll redraws");
    terminal.update(cx, |view, cx| view.scroll_pixels(20., 20., cx));
    assert_eq!(renders(cx), 1, "a wheel scroll redraws");
    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply([history(44)], cx);
        applied.unwrap();
    });
    assert_eq!(
        renders(cx),
        1,
        "history arriving while scrolled back redraws"
    );
    terminal.update(cx, |view, cx| view.follow(cx));
    assert_eq!(renders(cx), 1, "following again redraws");
    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply([history(45)], cx);
        applied.unwrap();
    });
    assert_eq!(
        renders(cx),
        0,
        "history the following view does not show redraws nothing"
    );
}

/// The scrollbar's numbers count every line navigation can reach: the history, the notice line when recovery lost
/// some, and the screen, with the view's top among them; the alternate screen and the catch-up placeholder offer
/// nothing to scroll. A bar that counted otherwise would sit in the wrong place, or offer a scroll the view cannot make.
#[gpui::test]
fn the_scroll_position_counts_what_navigation_can_reach(cx: &mut TestAppContext) {
    let placeholder = fed(ATTACH, false);
    assert!(
        !placeholder.visible && placeholder.model.history().len() == 3,
        "the premise: hidden, with the fixture's history already in the model"
    );
    assert!(
        !placeholder.scroll_position().scrollable(),
        "behind the catch-up placeholder"
    );

    let terminal = attached_with_history(cx);
    let position =
        |cx: &mut TestAppContext| terminal.read_with(cx, |view, _| view.scroll_position());
    // The fixture: a 4-row screen under three history lines.
    assert_eq!(
        position(cx),
        ScrollbarLines {
            visible: 4,
            total: 7,
            top: 3
        }
    );
    let state = |alt: bool| -> GridOutput {
        serde_json::from_value(
            serde_json::json!({"t":"state","mouse":"off","focus":false,"alt_screen":alt,
            "default_fg":null,"default_bg":null,"history_trimmed":true,"exit":null}),
        )
        .unwrap()
    };
    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply([state(false)], cx);
        applied.unwrap();
        view.scroll_lines(-1, cx);
    });
    assert_eq!(
        position(cx),
        ScrollbarLines {
            visible: 4,
            total: 8,
            top: 3
        },
        "the notice line counts, and the view is a line up"
    );
    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply([state(true)], cx);
        applied.unwrap();
    });
    assert!(
        !position(cx).scrollable(),
        "the alternate screen has no history"
    );
}

// ---- Selection ---------------------------------------------------------------------------------------------------

/// A history line arriving at `first`, which drops the oldest when the fixture's 3-line cap is full.
fn history_line(first: u64, text: &str) -> GridOutput {
    serde_json::from_value(serde_json::json!({
        "t":"history","first":first,"rows":[{"w":false,"r":[[0,text.len(),text,[0,-1,-1]]]}],"cols":20
    }))
    .unwrap()
}

/// The selection is held in line numbers, as the old app held it: the view's rows name their lines (the notice line
/// standing for the line below it), so a selection made scrolled back is the same text when the view follows again,
/// and the element draws a box only on the rows the selection covers. Selecting all covers the history and the
/// screen.
#[gpui::test]
fn a_selection_names_lines_and_select_all_covers_history_and_screen(cx: &mut TestAppContext) {
    let terminal = attached_with_history(cx);
    // The fixture: history lines 41 (empty), 42 "three" and 43 "four" above a screen whose first line is 44.
    let read =
        |cx: &mut TestAppContext, row: u16| terminal.read_with(cx, |view, _| view.line_at_row(row));
    assert_eq!(read(cx, 0), Some(44));
    terminal.update(cx, |view, cx| view.scroll_lines(-3, cx));
    assert_eq!(read(cx, 0), Some(41));
    assert_eq!(read(cx, 3), Some(44));

    let three = Selection::new(
        Position { line: 42, col: 0 },
        Position { line: 42, col: 5 },
        SelectionMode::Linear,
    );
    terminal.update(cx, |view, cx| view.set_selection(Some(three), cx));
    assert_eq!(
        terminal.read_with(cx, |view, _| view.selected_rows()),
        [(1, 0..5)],
        "drawn on the row showing line 42 only"
    );
    terminal.update(cx, |view, cx| view.follow(cx));
    assert_eq!(
        terminal.read_with(cx, |view, _| view.selected_text()),
        Some("three".into())
    );
    assert_eq!(
        terminal.read_with(cx, |view, _| view.selected_rows()),
        [],
        "not drawn while its line is out of view"
    );

    terminal.update(cx, |view, cx| view.select_all(cx));
    let all = terminal
        .read_with(cx, |view, _| view.selected_text())
        .unwrap();
    assert!(all.contains("three\nfour\n$ echo\nhello"), "{all:?}");
}

/// What makes the selection's line numbers stop naming its text ends it: here the alternate screen switching, one of
/// the viewport's resets (a resize and a history reset are the others, which reset the viewport the same way; its own
/// tests pin those). History dropped at the cap trims it to what is left, rather than letting it name lines that are
/// gone. An empty selection (a click) has no text, so it never reaches the clipboard.
#[gpui::test]
fn a_selection_is_trimmed_by_dropped_history_and_cleared_by_a_reset(cx: &mut TestAppContext) {
    let terminal = attached_with_history(cx);
    let selection = |cx: &mut TestAppContext| terminal.read_with(cx, |view, _| view.selection());
    let from_41 = Selection::new(
        Position { line: 41, col: 0 },
        Position { line: 42, col: 5 },
        SelectionMode::Linear,
    );
    terminal.update(cx, |view, cx| view.set_selection(Some(from_41), cx));
    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply([history_line(44, "five")], cx);
        applied.unwrap();
    });
    assert_eq!(
        selection(cx).map(|selection| selection.start),
        Some(Position { line: 42, col: 0 }),
        "line 41 was dropped; the selection keeps the rest"
    );

    let alternate: GridOutput =
        serde_json::from_value(serde_json::json!({"t":"state","mouse":"off","focus":false,
        "alt_screen":true,"default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}))
        .unwrap();
    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply([alternate], cx);
        applied.unwrap();
    });
    assert_eq!(
        selection(cx),
        None,
        "the alternate screen switching clears it"
    );

    let click = Selection::new(
        Position { line: 44, col: 2 },
        Position { line: 44, col: 2 },
        SelectionMode::Linear,
    );
    terminal.update(cx, |view, cx| view.set_selection(Some(click), cx));
    assert_eq!(terminal.read_with(cx, |view, _| view.selected_text()), None);
}

/// A hover rests on the text it read: the link under the pointer is underlined on the rows it covers; when the view
/// scrolls under the still pointer, what is under it now is looked up, and when the program rewrites those rows the
/// link is looked up again, so a target that moved away or is gone is no longer underlined (or clickable). The
/// fixture's second screen row (line 45) carries a program's hyperlink, "web", at columns 11 to 13.
#[gpui::test]
fn a_hovered_link_is_underlined_and_dropped_when_its_text_changes(cx: &mut TestAppContext) {
    let terminal = attached_with_history(cx);
    terminal.update(cx, |view, cx| view.set_hover(Some((12, 1)), cx));
    let hovered = terminal.read_with(cx, |view, _| {
        view.hovered_link()
            .map(|link| (link.uri.clone(), link.kind.clone()))
    });
    assert_eq!(
        hovered.as_ref().map(|(uri, _)| uri.as_str()),
        Some("https://example.com/"),
        "the premise: the fixture's hyperlink is under the pointer"
    );
    assert_eq!(
        terminal.read_with(cx, |view, _| view.underlined_rows()),
        [(1, 11..14)]
    );

    // Scrolling under the still pointer brings other text under it: the hover follows the cell, not the link.
    terminal.update(cx, |view, cx| view.scroll_lines(-1, cx));
    assert_eq!(
        terminal.read_with(cx, |view, _| view.hovered_link().cloned()),
        None,
        "scrolled off the link"
    );
    assert_eq!(terminal.read_with(cx, |view, _| view.underlined_rows()), []);
    terminal.update(cx, |view, cx| view.follow(cx));
    assert!(
        terminal.read_with(cx, |view, _| view.hovered_link().is_some()),
        "and back on it"
    );

    let rewritten: GridOutput = serde_json::from_value(serde_json::json!({
        "t":"rows", "rows":[[1,{"w":false,"r":[[0,5,"plain",[0,-1,-1]]]}]]
    }))
    .unwrap();
    let frame: GridOutput = serde_json::from_value(serde_json::json!({
        "t":"frame", "rev":2,"cols":20,"rows":4,
        "cursor":{"x":2,"y":2,"visible":true,"shape":"block","blink":true}
    }))
    .unwrap();
    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply([rewritten, frame], cx);
        applied.unwrap();
    });
    assert_eq!(
        terminal.read_with(cx, |view, _| view.hovered_link().cloned()),
        None
    );
    assert_eq!(terminal.read_with(cx, |view, _| view.underlined_rows()), []);
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

// ---- Drawing ---------------------------------------------------------------------------------------------------------

/// A terminal window showing one complete screen: a hello at `cols` by `rows`, plain state, the screen rows `screen`
/// (the protocol's JSON form) and one frame with `cursor`, round-tripped through the encoder as [`messages`] does and
/// drawn once, so each shown row's cache holds what the element shaped for it.
fn drawn(
    cols: u16,
    rows: u16,
    screen: serde_json::Value,
    cursor: serde_json::Value,
    cx: &mut TestAppContext,
) -> (Entity<TerminalView>, gpui::AnyWindowHandle) {
    let recording = [
        serde_json::json!({"t":"hello","cols":cols,"rows":rows,"history_cap":0}),
        serde_json::json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
            "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
        serde_json::json!({"t":"rows","rows":screen}),
        serde_json::json!({"t":"frame","rev":1,"cols":cols,"rows":rows,"cursor":cursor}),
    ]
    .map(|message| message.to_string())
    .join("\n");
    let (terminal, window) = terminal_window(cx);
    terminal.update(cx, |view, cx| {
        let (_, applied) = view.apply(messages(&recording), cx);
        applied.unwrap();
        view.mark_replay_complete(cx).unwrap();
    });
    cx.run_until_parked();
    (terminal, window)
}

/// One shaped run as `paint_rows` places it: the run's wire column and width (its background box), and each glyph
/// piece's column within the run, width and text. A piece is drawn at the run's column plus its own, times the cell
/// width, and clipped to its width in cells.
type ShapedRun = (u16, u16, Vec<(u16, u16, String)>);

/// What the last frame shaped for screen row `y`. Panics when the row was never drawn.
fn shaped(terminal: &Entity<TerminalView>, y: usize, cx: &mut TestAppContext) -> Vec<ShapedRun> {
    terminal.read_with(cx, |view, _| {
        let shaped = view.rows[y].shaped.borrow();
        let runs = shaped.as_ref().expect("the premise: the row was drawn");
        runs.iter()
            .map(|run| {
                let pieces = run.pieces.iter();
                let pieces = pieces.map(|p| (p.col, p.width, p.line.text.to_string()));
                (run.col, run.width, pieces.collect())
            })
            .collect()
    })
}

/// A run of `text` shaped as one piece covering its whole box.
fn whole(col: u16, width: u16, text: &str) -> ShapedRun {
    (col, width, vec![(0, width, text.to_owned())])
}

/// A printable-ASCII run of `text` shaped one piece per cell.
fn per_cell(col: u16, text: &str) -> ShapedRun {
    let pieces = text.chars().enumerate();
    let pieces = pieces.map(|(i, c)| (i as u16, 1, c.to_string())).collect();
    (col, text.len() as u16, pieces)
}

/// A glyph the font draws wider or narrower than its cells must never move the cells after it: the old app's version
/// of this test caught a block element drawn 1.5 cells wide, and Arabic cells that shape narrower side by side, each
/// pushing every later letter off its column. GPUI's rule differs from the old app's flow-or-box choice: printable
/// ASCII is split one piece per cell, and every other run is one piece as wide as the wire says, placed at the wire's
/// column and clipped there, so no font measurement takes part. A bidi control prints nothing, but its cell is still
/// a box at its column.
///
/// Specified: on a row of box drawing, ASCII pairs, a light shade, a left-to-right mark alone in a cell and an accented
/// Latin letter, each ASCII pair is a run of two one-cell pieces at its wire column and every other cell is a run of
/// one whole piece at its own column; the mark's piece is empty; on the next row two Arabic cells are whole pieces at
/// columns 0 and 1, and the `X` after them is at column 2.
#[gpui::test]
fn non_ascii_runs_draw_as_one_box_at_their_wire_column(cx: &mut TestAppContext) {
    let plain = [0, -1, -1];
    let screen = serde_json::json!([
        [0, {"w": false, "r": [
            [0, 1, "\u{2500}", plain], [1, 1, "\u{253c}", plain], [2, 1, "\u{2500}", plain],
            [3, 2, "ab", plain], [5, 1, "\u{2591}", plain], [6, 2, "cd", plain],
            [8, 1, "\u{200e}", plain], [9, 2, "ef", plain], [11, 1, "\u{e9}", plain],
            [12, 2, "gh", plain],
        ]}],
        [1, {"w": false, "r": [[0, 1, "\u{6fb}", plain], [1, 1, "\u{6fb}", plain], [2, 1, "X", plain]]}],
    ]);
    let cursor = serde_json::json!({"x":0,"y":0,"visible":false,"shape":"block","blink":false});
    let (terminal, _window) = drawn(40, 2, screen, cursor, cx);
    assert_eq!(
        shaped(&terminal, 0, cx),
        [
            whole(0, 1, "\u{2500}"),
            whole(1, 1, "\u{253c}"),
            whole(2, 1, "\u{2500}"),
            per_cell(3, "ab"),
            whole(5, 1, "\u{2591}"),
            per_cell(6, "cd"),
            whole(8, 1, ""),
            per_cell(9, "ef"),
            whole(11, 1, "\u{e9}"),
            per_cell(12, "gh"),
        ]
    );
    assert_eq!(
        shaped(&terminal, 1, cx),
        [
            whole(0, 1, "\u{6fb}"),
            whole(1, 1, "\u{6fb}"),
            per_cell(2, "X")
        ]
    );
}

/// Cells arrive in visual order, so nothing the program prints may reorder them: a right-to-left override (zero-width,
/// so the emulator keeps it with the cell before it) or right-to-left letters let a bidi-aware shaper move text across
/// cells, which the old app's test caught in the browser. Each non-ASCII cell is its own run and the override is
/// stripped before shaping, while copy and automation keep the received text.
///
/// Specified: `ab`, `c` with U+202E in one cell, `def`, then three Hebrew letters in their own cells draw at columns
/// 0-1, 2, 3-5, 6, 7 and 8 in that order, the override's cell drawing only `c`; the view's text still holds U+202E.
#[gpui::test]
fn right_to_left_text_and_an_override_keep_their_wire_columns(cx: &mut TestAppContext) {
    let plain = [0, -1, -1];
    let screen = serde_json::json!([[0, {"w": false, "r": [
        [0, 2, "ab", plain], [2, 1, "c\u{202e}", plain], [3, 3, "def", plain],
        [6, 1, "\u{5e9}", plain], [7, 1, "\u{5dc}", plain], [8, 1, "\u{5d5}", plain],
    ]}]]);
    let cursor = serde_json::json!({"x":0,"y":0,"visible":false,"shape":"block","blink":false});
    let (terminal, _window) = drawn(60, 2, screen, cursor, cx);
    assert_eq!(
        shaped(&terminal, 0, cx),
        [
            per_cell(0, "ab"),
            whole(2, 1, "c"),
            per_cell(3, "def"),
            whole(6, 1, "\u{5e9}"),
            whole(7, 1, "\u{5dc}"),
            whole(8, 1, "\u{5d5}"),
        ]
    );
    let text = terminal
        .read_with(cx, |view, _| view.terminal_text())
        .unwrap();
    assert!(
        text.starts_with("abc\u{202e}def\u{5e9}\u{5dc}\u{5d5}"),
        "{text:?}"
    );
}

/// A program hides the cursor while it redraws (`CSI ? 25 l`) and shows it again after, and both must reach the
/// screen: a cursor left showing flickers across every redraw, and one that never comes back leaves the user typing
/// blind. The old app's test fed the frames directly, as this one does.
///
/// Specified: on a drawn screen with an unfocused cursor at (0, 0), a frame with `visible:true` paints the cursor's
/// outline, one with `visible:false` paints no quad in the cursor's color, and one with `visible:true` again paints
/// the same outline as the first.
#[gpui::test]
fn a_hidden_cursor_is_not_drawn_and_comes_back_when_shown(cx: &mut TestAppContext) {
    let at = |rev: u64, visible: bool| -> serde_json::Value {
        serde_json::json!({"t":"frame","rev":rev,"cols":20,"rows":4,
            "cursor":{"x":0,"y":0,"visible":visible,"shape":"bar","blink":false}})
    };
    let screen = serde_json::json!([[0, {"w": false, "r": [[0, 2, "$ ", [0, -1, -1]]]}]]);
    let (terminal, window) = drawn(20, 4, screen, at(1, true)["cursor"].clone(), cx);
    let cursor_color = gpui::Background::from(TerminalColor::of(TERMINAL.cursor).hsla(1.));
    let drawn_cursor = |rev: u64, visible: bool, cx: &mut TestAppContext| {
        let frame: GridOutput = serde_json::from_value(at(rev, visible)).unwrap();
        terminal.update(cx, |view, cx| view.apply([frame], cx).1.unwrap());
        cx.run_until_parked();
        let quads = cx.update_window(window, |_, window, _| window.painted_quads());
        let quads = quads.unwrap().into_iter();
        quads
            .filter(|quad| quad.background == cursor_color)
            .map(|quad| quad.bounds)
            .collect::<Vec<_>>()
    };
    let shown = drawn_cursor(2, true, cx);
    assert!(!shown.is_empty(), "the premise: a shown cursor is drawn");
    assert_eq!(drawn_cursor(3, false, cx), []);
    assert_eq!(drawn_cursor(4, true, cx), shown);
}

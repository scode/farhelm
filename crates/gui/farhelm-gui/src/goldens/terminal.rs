//! Golden scenes for the terminal: its styles, wide glyphs and cursor shapes, an attached terminal's status line,
//! exit line and reconnect card, a file link's hover display, and the scrim a pending Delete puts over it. The scene
//! list itself, `SCENES` with each scene's name and size, stays in `mod.rs`.

use crate::session;
use crate::session_list::SessionList;
use crate::terminal::attached::{AttachedTerminal, Hold, OpenStream, TerminalTarget};
use crate::terminal::set::TerminalSet;
use crate::terminal::{CellMetrics, CursorPhase, TerminalView, TextSize};
use crate::theme;
use farhelm_grid_model::GridOutput;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::FakeTerminal;
use farhelm_helm_client::terminal::AttachmentLease;
use gpui::{
    AnyElement, App, AppContext as _, Entity, IntoElement, ParentElement, Styled, Window, div, px,
};
use std::cell::RefCell;
use std::rc::Rc;

use super::{SceneState, defer_after, session_list_scene_opening};

/// One terminal's whole output for a golden scene, in the grid protocol's JSON description, decoded the way a live
/// attachment's frames are: a hello at `cols` by `rows`, plain state, the given rows, and one frame with `cursor`.
/// Panics on a message that does not decode: dropping it would draw a partial terminal, which `--update` would then
/// bake into the reference.
fn grid_output(
    cols: u16,
    rows: u16,
    screen: serde_json::Value,
    cursor: serde_json::Value,
) -> Vec<GridOutput> {
    let json = [
        serde_json::json!({"t":"hello","cols":cols,"rows":rows,"history_cap":0}),
        serde_json::json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
            "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
        serde_json::json!({"t":"rows","rows":screen}),
        serde_json::json!({"t":"frame","rev":1,"cols":cols,"rows":rows,"cursor":cursor}),
    ];
    json.into_iter()
        .map(|value| serde_json::from_value(value).unwrap())
        .collect()
}

/// One run in the protocol's JSON form: `[col, width, text, [flags, fg, bg]]`, colors as wire values (-1 the default,
/// 0-255 a palette index, `0x1000000 | rgb` a truecolor).
fn run(col: u16, width: u16, text: &str, flags: u16, fg: i64, bg: i64) -> serde_json::Value {
    serde_json::json!([col, width, text, [flags, fg, bg]])
}

/// One screen row of runs.
fn screen_row(y: u16, runs: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!([y, {"w": false, "r": runs}])
}

/// A terminal view showing `output` behind no placeholder, with the cursor in `phase`, at the default text size.
/// Panics when the font cannot be measured or the output is refused, for the same reason as [`grid_output`]: an empty
/// terminal must fail the golden, never become its reference.
fn terminal_view(
    output: Vec<GridOutput>,
    phase: CursorPhase,
    cx: &mut App,
) -> Entity<TerminalView> {
    let metrics = CellMetrics::measure(TextSize::DEFAULT, cx).unwrap();
    let view = cx.new(|_| TerminalView::new(metrics));
    view.update(cx, |view, cx| {
        let (_, applied) = view.apply(output, cx);
        applied.unwrap();
        view.mark_replay_complete(cx).unwrap();
        view.set_cursor_phase(phase, cx);
    });
    view
}

/// The terminal's styles in the bundled font: the default colors, the sixteen palette colors as backgrounds, bold
/// brightening, dim, italic (in the bundled Italic face, so slanted), inverse, the five underline shapes,
/// strikethrough, hidden text that keeps its underline, a truecolor foreground, samples of the 256-color cube and gray
/// ramp, and box drawing. ASCII and box drawing only: no fallback font is bundled for goldens, because the migration's
/// orchestrator decided on 2026-11-13 that M1 has no fallback-font goldens.
///
/// The box drawing is two rows, a box beside shades and a pair of full blocks, so the picture shows whether lines and
/// blocks meet across cells both ways: programs draw their borders with these characters, and a cell that does not
/// match the glyph box leaves gaps between them. The two rows are the scene's last, rows 7 and 8: in this scene's
/// geometry they are the pair whose baselines GPUI would round a device pixel further apart than a glyph row is tall
/// (`Baselines::origin_y` in the terminal module), so the box's vertical lines show the seam if row baselines stop
/// stepping by whole device rows. The blocks and shades are drawn as rectangles of their cells (the terminal's
/// `blocks` module), so they meet at any size; the shades show as flat fills.
pub(super) fn terminal_styles_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use farhelm_grid_model::style_flags as f;
    let palette: Vec<_> = (0..16)
        .map(|i| run(i * 2, 2, "  ", 0, -1, i64::from(i)))
        .collect();
    let bold: Vec<_> = (0..8)
        .map(|i| run(i * 2, 1, "B", f::BOLD, i64::from(i), -1))
        .collect();
    let cube: Vec<_> = [16, 52, 88, 124, 160, 196, 46, 21, 232, 244, 255]
        .into_iter()
        .enumerate()
        .map(|(i, index)| run(i as u16 * 2, 2, "  ", 0, -1, index))
        .collect();
    let screen = serde_json::json!([
        screen_row(0, vec![run(0, 12, "default text", 0, -1, -1)]),
        screen_row(1, palette),
        screen_row(2, bold),
        screen_row(
            3,
            vec![
                run(0, 3, "dim", f::DIM, -1, -1),
                run(4, 6, "italic", f::ITALIC, -1, -1),
                run(11, 7, "inverse", f::INVERSE, -1, -1),
                run(19, 8, "truecolr", 0, 0x1ff_8800, -1),
            ]
        ),
        screen_row(
            4,
            vec![
                run(0, 5, "solid", f::UNDERLINE, -1, -1),
                run(6, 6, "double", f::DOUBLE_UNDERLINE, -1, -1),
                run(13, 4, "wavy", f::UNDERCURL, -1, -1),
                run(18, 6, "dotted", f::DOTTED_UNDERLINE, -1, -1),
                run(25, 6, "dashed", f::DASHED_UNDERLINE, -1, -1),
            ]
        ),
        screen_row(
            5,
            vec![
                run(0, 6, "strike", f::STRIKETHROUGH, -1, -1),
                run(7, 6, "hidden", f::HIDDEN | f::UNDERLINE, -1, -1),
            ]
        ),
        screen_row(6, cube),
        screen_row(7, box_row(["┌", "─", "─", "┐"])),
        screen_row(8, box_row(["└", "─", "─", "┘"])),
    ]);
    let cursor = serde_json::json!({"x":0,"y":7,"visible":false,"shape":"block","blink":false});
    let view = terminal_view(
        grid_output(36, 9, screen, cursor),
        CursorPhase {
            focused: true,
            on: true,
        },
        cx,
    );
    state.terminals.borrow_mut().push(view);
}

/// The last column of the first colored row is a separate padding run. Its background must meet the grid's edge.
/// The following row starts with a two-cell lightning glyph from the bundled font, so no fallback font is needed.
/// The last two rows show the same glyph after erasing its right half. A new color clips the glyph at one column.
/// When the erase leaves the same color, the glyph still has both columns to draw in.
pub(super) fn terminal_wide_glyphs_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    const HEAD: &str = "wide character wraps here";
    const GREEN: i64 = 0x12a_6f62;
    const RED: i64 = 0x197_4640;
    let width: u16 = HEAD.len().try_into().unwrap();
    let screen = serde_json::json!([
        [0, {"w": true, "p": true, "r": [
            run(0, width, HEAD, 0, -1, GREEN),
            run(width, 1, " ", 0, -1, GREEN),
        ]}],
        screen_row(1, vec![
            run(0, 2, "⚡", 0, -1, GREEN),
            run(2, 9, "continued", 0, -1, GREEN),
        ]),
        screen_row(2, vec![
            run(0, 1, "⚡", 0, -1, GREEN),
            run(1, 1, " ", 0, -1, RED),
            run(3, 20, "different background", 0, -1, -1),
        ]),
        screen_row(3, vec![
            run(0, 2, "⚡", 0, -1, GREEN),
            run(3, 15, "same background", 0, -1, -1),
        ]),
    ]);
    let cursor = serde_json::json!({"x":0,"y":0,"visible":false,"shape":"block","blink":false});
    let view = terminal_view(
        grid_output(width + 1, 4, screen, cursor),
        CursorPhase {
            focused: false,
            on: false,
        },
        cx,
    );
    state.terminals.borrow_mut().push(view);
}

/// One row of the styles scene's box drawing: a box edge in columns 0-3, the three shades from full to light in 5-7,
/// and two full blocks side by side in 9-10.
fn box_row(edge: [&str; 4]) -> Vec<serde_json::Value> {
    let shades = ["█", "▓", "░", "", "█", "█"];
    let edge = edge.into_iter().enumerate();
    let shades = shades
        .into_iter()
        .enumerate()
        .map(|(i, text)| (i + 5, text));
    edge.chain(shades)
        .filter(|(_, text)| !text.is_empty())
        .map(|(col, text)| run(col as u16, 1, text, 0, -1, -1))
        .collect()
}

/// The cursor's shapes side by side, each its own small terminal: a focused block re-painting its glyph in the accent
/// color, a bar, an underline, an unfocused outline, a block on a wide character's spacer cell covering the whole
/// two-cell box, and a block on a left half block, which re-paints the half in the accent color as it would a glyph.
/// The wide run's glyph has no bundled face; the golden pins the cursor's box, not that glyph.
pub(super) fn terminal_cursors_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let cases = [
        (0, "block", true),
        (0, "bar", true),
        (0, "underline", true),
        (0, "block", false),
        (3, "block", true),
    ];
    for (x, shape, focused) in cases {
        let screen = serde_json::json!([screen_row(
            0,
            vec![
                run(0, 2, "ab", 0, -1, -1),
                run(2, 2, "\u{4e2d}", 0, -1, -1),
                run(4, 2, "cd", 0, -1, -1),
            ]
        )]);
        let cursor = serde_json::json!({"x":x,"y":0,"visible":true,"shape":shape,"blink":false});
        let view = terminal_view(
            grid_output(6, 2, screen, cursor),
            CursorPhase { focused, on: true },
            cx,
        );
        state.terminals.borrow_mut().push(view);
    }
    let screen = serde_json::json!([screen_row(
        0,
        vec![
            run(0, 1, "a", 0, -1, -1),
            run(1, 1, "\u{258c}", 0, -1, -1),
            run(2, 1, "b", 0, -1, -1),
        ]
    )]);
    let cursor = serde_json::json!({"x":1,"y":0,"visible":true,"shape":"block","blink":false});
    let view = terminal_view(
        grid_output(6, 2, screen, cursor),
        CursorPhase {
            focused: true,
            on: true,
        },
        cx,
    );
    state.terminals.borrow_mut().push(view);
}

/// Two attached terminals the helm detached as stalled viewers, each showing the red status line with the helm's
/// reason and Reconnect above its kept screen (the old terminal's `.banner` and its button). The first one's
/// program exited, so it also has the old app's exit line (`.grid-terminal-exit`) below its last row, in the terminal's
/// reversed defaults; the second's program still runs. In both the program's last lines sit at the bottom of its screen,
/// where a shell leaves them, and the status line takes room the fit leaves to the grid, so the screen is lifted by
/// whole rows: its top rows go out of view and the last ones stay.
///
/// The owners need a UI session, so the setup connects a fake helm and makes them once its snapshot arrives; each
/// owner's opener plays its whole stream at once (attach, a replay at the fitted size, its end, the stall), and the
/// settle loop draws until the picture stops changing.
pub(super) fn terminal_status_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    attached_setup(
        state,
        cx,
        Vec::new(),
        |keep| {
            vec![
                stalled_stream(Some(2), keep.clone()),
                stalled_stream(None, keep),
            ]
        },
        |_| {},
    );
}

/// What a scene keeps alive for as long as it draws (`SceneState`'s `keep`): fake helms and streams, subscriptions.
type Kept = Rc<RefCell<Vec<Box<dyn std::any::Any>>>>;

/// Connect a fake helm and, once its snapshot arrives, make one attached terminal for each opener `openers` makes,
/// handing them whatever they must keep alive. Fixtures can stage an outcome on each owner before its first frame.
fn attached_setup(
    state: &SceneState,
    cx: &mut App,
    snapshot: Vec<farhelm_proto::ui_channel::CollectionState>,
    openers: fn(Kept) -> Vec<OpenStream>,
    stage: fn(&mut AttachedTerminal),
) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(snapshot);
    // The subscription is kept in `keep` too, so the closure holds it only weakly: a strong handle would be a cycle that
    // outlives the app and fails its leak check.
    let (attached, keep) = (state.attached.clone(), Rc::downgrade(&state.keep));
    let made = cx.observe(&connection, move |connection, cx| {
        let Some(session) = connection.read(cx).session().cloned() else {
            return;
        };
        if !attached.borrow().is_empty() {
            return;
        }
        let Some(keep) = keep.upgrade() else {
            return;
        };
        for open in openers(keep) {
            let target = TerminalTarget {
                session: "golden".into(),
                tab: None,
                lease: AttachmentLease::new(),
            };
            let metrics = CellMetrics::measure(TextSize::DEFAULT, cx).unwrap();
            let owner = cx.new(|cx| {
                let mut owner = AttachedTerminal::new(&session, target, metrics, open, cx);
                stage(&mut owner);
                owner
            });
            attached.borrow_mut().push(owner);
        }
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(made));
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
}

/// An opener whose fake stream replays a short session at the bottom of the fitted screen, with the program exited
/// with status `exit` if given, and is then detached as a stalled viewer, with the helm's reason. The fake is kept in
/// `keep`.
fn stalled_stream(exit: Option<i32>, keep: Kept) -> OpenStream {
    replayed_stream(exit, keep, |fake| {
        fake.detach(
            Some(farhelm_proto::DetachCode::Stalled),
            farhelm_proto::DETACH_REASON_STALLED,
        );
    })
}

/// An opener whose fake stream replays a short session at the bottom of the fitted screen, with the program exited
/// with status `exit` if given, and then does `then` to it. The fake is kept in `keep`.
pub(super) fn replayed_stream(
    exit: Option<i32>,
    keep: Kept,
    then: fn(&mut FakeTerminal),
) -> OpenStream {
    Box::new(move |attach| {
        let (mut fake, handle, events) = FakeTerminal::open(&attach);
        let asked = fake.attach();
        let last = asked.rows.saturating_sub(1);
        let (ending, cursor_x) = match exit {
            Some(_) => ("$ exit", 6),
            None => ("$ ", 2),
        };
        let screen = serde_json::json!([
            screen_row(last - 2, vec![run(0, 6, "$ make", 0, -1, -1)]),
            screen_row(last - 1, vec![run(0, 17, "error: tests fail", 0, 1, -1)]),
            screen_row(last, vec![run(0, ending.len() as u16, ending, 0, -1, -1)]),
        ]);
        let cursor =
            serde_json::json!({"x":cursor_x,"y":last,"visible":true,"shape":"block","blink":false});
        let mut output = grid_output(asked.cols, asked.rows, screen, cursor);
        if let Some(code) = exit {
            let exited = serde_json::json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
                "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":{"code":code}});
            output.push(serde_json::from_value(exited).unwrap());
        }
        for message in output {
            fake.output(message);
        }
        fake.replay_complete();
        then(&mut fake);
        keep.borrow_mut().push(Box::new(fake));
        (handle, events)
    })
}

/// Render the session's shell both before and after the capped list omits its row.
pub(super) fn terminal_cut_row_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use crate::session_list::tests::{change, listing_meta, session as fixture};
    use farhelm_proto::ui_channel::SessionRow;

    session_list_scene_opening(state, &[fixture("a").tabs(1), fixture("b")], Some("a"), cx);
    let connection = session::connection(cx).unwrap();
    let keep = Rc::downgrade(&state.keep);
    connection.update(cx, |connection, _| {
        connection.serve_terminals(move |attach| {
            replayed_stream(None, keep.upgrade().unwrap(), |_| {})(attach)
        });
    });
    state.after.set(Some(|_, state, window, cx| {
        let connection = session::connection(cx).unwrap();
        let session = connection.read(cx).session().unwrap().clone();
        let metrics = CellMetrics::measure(TextSize::DEFAULT, cx).unwrap();
        let hold = Rc::new(|_: &App| Hold::Nothing);
        let set = cx.new(|cx| TerminalSet::new(&session, "a".into(), metrics, hold, cx));
        let mark = crate::focus::Mark::take(cx);
        set.update(cx, |set, cx| {
            set.choose(Some("tab-0".into()), &mark, window, cx)
        });
        state.keep.borrow_mut().push(Box::new(set));
        state.after.set(Some(cut_row));
    }));

    /// Hold staging through the shell's replay and the store's removal of the row.
    fn cut_row(_: &Entity<SessionList>, state: &SceneState, _: &mut Window, cx: &mut App) {
        let set = cut_row_terminal(state).unwrap();
        let set = set.read(cx);
        assert_eq!(set.chosen(), Some("tab-0"));
        if !set.shown().unwrap().read(cx).is_live() {
            state.after.set(Some(cut_row));
            return;
        }
        let mut keep = state.keep.borrow_mut();
        let helm = keep
            .iter_mut()
            .find_map(|entry| entry.downcast_mut::<FakeHelm>())
            .unwrap();
        helm.push(change(listing_meta(&["b"], 502, true), &[], &["a"]));
        // The next staging frame drains the fake's push before drawing.
        state.after.set(Some(|_, state, _, cx| {
            let store = session::current_store(cx).unwrap();
            let sessions = store.read(cx).collections();
            assert!(sessions.get::<SessionRow>("a").is_none());
            let set = cut_row_terminal(state).unwrap();
            assert_eq!(set.read(cx).chosen(), Some("tab-0"));
        }));
    }
}

/// The pane belongs to the open session for the whole scene, even when its sidebar row disappears.
fn cut_row_terminal(state: &SceneState) -> Option<Entity<TerminalSet>> {
    state
        .keep
        .borrow()
        .iter()
        .find_map(|entry| entry.downcast_ref::<Entity<TerminalSet>>().cloned())
}

/// The still-open terminal pane and the still-selected shell after the session list omits its row.
pub(super) fn terminal_cut_row(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    defer_after(state, window, cx);
    let ground = div().size_full().flex().flex_col();
    let Some(set) = cut_row_terminal(state) else {
        return ground.into_any_element();
    };
    ground
        .child(TerminalSet::strip(&set, window, cx))
        .child(div().flex_1().min_h_0().child(set))
        .into_any_element()
}

/// The attached terminals a scene made, one above the other, each in a pane of a fixed size.
pub(super) fn attached_terminals(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .children(
            state
                .attached
                .borrow()
                .iter()
                .map(|owner| div().w(px(420.)).h(px(150.)).child(owner.clone())),
        )
        .into_any_element()
}

/// Put a real approval card in front of an attached terminal with the outcome of a covered link click. The
/// interaction tests drive the click itself. This scene pins the status line's position over the card.
pub(super) fn terminal_covered_link_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    use crate::approvals::region_tests::{stop, waiting};
    crate::approvals::region::install(window, cx);
    attached_setup(
        state,
        cx,
        vec![waiting(&[stop("golden-covered-link", "build-box")])],
        |keep| vec![replayed_stream(None, keep, |_| {})],
        AttachedTerminal::stage_covered_refusal,
    );
}

/// Match the approval region's placement relative to the terminal pane, as the session view does.
pub(super) fn terminal_covered_link(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let pane = crate::layers::Anchor::for_element("golden-covered-link-pane", window, cx);
    crate::approvals::region::set_pane(pane.clone(), cx);
    div()
        .relative()
        .w(px(760.))
        .h(px(420.))
        .child(pane.track())
        .children(state.attached.borrow().iter().cloned())
        .into_any_element()
}

/// A file link's hover display in each of its states, stacked (`crate::terminal::files`): checking, the three size
/// tiers, the limit refusal in red, a folder and a failed check. Drawn on their own rather than over a terminal, since
/// a display's look does not depend on what it hovers over.
pub(super) fn terminal_file_disclosures(
    _: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    crate::terminal::files::golden_displays(window, cx)
}

/// A shell's last lines at the bottom of a small terminal, for the Delete scrim to lie over: enough text that the
/// picture shows how far the veil dims it.
pub(super) fn terminal_delete_scrim_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let screen = serde_json::json!([
        screen_row(4, vec![run(0, 9, "$ cargo t", 0, -1, -1)]),
        screen_row(5, vec![run(0, 21, "test result: ok. 41 p", 0, 2, -1)]),
        screen_row(6, vec![run(0, 2, "$ ", 0, -1, -1)]),
    ]);
    let cursor = serde_json::json!({"x":2,"y":6,"visible":true,"shape":"block","blink":false});
    let view = terminal_view(
        grid_output(46, 7, screen, cursor),
        CursorPhase {
            focused: true,
            on: true,
        },
        cx,
    );
    state.terminals.borrow_mut().push(view);
}

/// The scene's terminal in a pane of a fixed size, on the terminal's ground as an attached terminal's pane is, under the
/// scrim the session pane draws over the open session's terminal while its Delete stops the agent (the old
/// `.terminal-delete-overlay`), its spinner on its first step.
pub(super) fn terminal_delete_scrim(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) -> AnyElement {
    div()
        .relative()
        .w(px(420.))
        .h(px(150.))
        .overflow_hidden()
        .bg(theme::color(theme::TERMINAL.background))
        .children(state.terminals.borrow().iter().cloned())
        .child(crate::session_pane::delete_scrim("Stopping agent…", 0, cx))
        .into_any_element()
}

/// The terminals a terminal scene's setup made, in a row.
pub(super) fn terminals(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .flex()
        .gap(px(12.))
        .children(state.terminals.borrow().iter().cloned())
        .into_any_element()
}

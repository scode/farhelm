//! Tests of a list row's open control (`components/list_row.rs`): the keyboard focus ring it paints inside its edge, in
//! the accent on an ordinary row and the on-fill ink on the selected one, never for a click, and kept through pointer
//! movement until a press.

use gpui::{
    Hsla, IntoElement as _, Modifiers, ParentElement as _, Styled as _, TestAppContext,
    VisualTestContext, div, point, px,
};

use crate::components::{HoverHelp, ListRow, Role};
use crate::test_support::{bounds, click, focused_id, keys, open, redraw};
use crate::{focus, theme};

use super::{tooltip, wait_hover_delay};

/// The color of the ring painted over the row control drawn under `id` in a fresh frame, if any ([`painted_ring`]).
fn ring(cx: &mut VisualTestContext, id: &'static str) -> Option<Hsla> {
    redraw(cx);
    painted_ring(cx, id)
}

/// The color of the ring over the row control drawn under `id` in the frame on screen, drawing none of its own: what
/// the step's own events left drawn, whichever of them asked for the frame. It is the painted quad that covers the
/// control's bounds exactly with a 2-pixel border. The control's own fill is a quad over the same bounds with no
/// border, so the border is what tells the ring apart.
fn painted_ring(cx: &mut VisualTestContext, id: &'static str) -> Option<Hsla> {
    let row = bounds(cx, id);
    let (scale, quads) = cx.update(|window, _| (window.scale_factor(), window.painted_quads()));
    let (row, width) = (row.scale(scale), px(2.).scale(scale));
    quads
        .into_iter()
        .find(|quad| quad.bounds == row && quad.border_widths.top == width)
        .map(|quad| quad.border_color)
}

/// GPUI's own "last input was the keyboard" flag for the window under test, which app code never reads
/// (`focus::last_input_was_keyboard` answers instead). Only a test premise reads it: that a pointer move cleared GPUI's
/// flag, so a ring that outlasts the move shows the coordinator's flag at work rather than GPUI's.
#[allow(
    clippy::disallowed_methods,
    reason = "a test premise that GPUI's flag and the coordinator's differ; no app code reads GPUI's"
)]
fn gpui_keyboard_flag(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, _| window.last_input_was_keyboard())
}

/// Why this matters: a keyboard user needs to see which row has focus, and the selected row wears the accent fill, so
/// an accent ring there would barely show; the old app drew it in the on-fill ink on the selection
/// (`.session-row.selected :focus-visible`). This is the old chrome spec's "keyboard focus uses accent on ordinary
/// controls and inverse on selection; a mouse click does not", for the rows.
///
/// Specified, over an ordinary row and a selected one: Tab rings the ordinary row in the accent, and Tab on rings the
/// selected row in the on-fill ink. A click on the ordinary row under Linux's click rule focuses it with no ring. Under
/// the macOS click rule, a click on the keyboard-focused selected row leaves focus there and takes the ring away.
#[gpui::test]
fn keyboard_focus_rings_a_row_in_the_accent_and_the_selected_row_in_on_fill_and_a_click_does_not(
    cx: &mut TestAppContext,
) {
    let cx = open(cx, |_, _| {
        let row = |id: &'static str, selected: bool| {
            ListRow::new(
                id,
                id,
                HoverHelp::tooltip("Open this session"),
                Role::Button,
            )
            .selected(selected)
            .line(id)
        };
        div()
            .w(px(300.))
            .flex()
            .flex_col()
            .child(row("plain", false))
            .child(row("picked", true))
            .into_any_element()
    });
    let palette = cx.update(|_, cx| theme::palette(cx));
    let (accent, on_fill) = (theme::color(palette.accent), theme::color(palette.on_fill));
    assert_ne!(accent, on_fill, "the premise: the two inks differ");

    keys(cx, "tab");
    assert_eq!(ring(cx, "plain"), Some(accent), "the ordinary row's ring");
    keys(cx, "tab");
    assert_eq!(ring(cx, "picked"), Some(on_fill), "the selected row's ring");

    click(cx, "plain");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("plain"),
        "the premise: Linux's click rule focused the row"
    );
    assert_eq!(ring(cx, "plain"), None, "a click paints no ring");

    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    keys(cx, "tab");
    assert_eq!(ring(cx, "picked"), Some(on_fill), "Tab ringed it");
    click(cx, "picked");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("picked"),
        "the premise: under the macOS rule the click left focus where it was"
    );
    assert_eq!(ring(cx, "picked"), None, "the click took the ring away");
}

/// Why this matters: the ring is how a keyboard user sees where they are, and the old app's (`:focus-visible`) stayed
/// through pointer movement and went only with a press. GPUI's own "last input was the keyboard" turns false on any
/// pointer move, so with the ring and focus help tied to it a keyboard user who nudged the mouse lost the ring at once,
/// and focus landing after the nudge (a move the app makes, or the frame of a Tab drawn after the nudge) showed
/// neither ring nor help. The focus coordinator keeps its own flag for both (`focus::last_input_was_keyboard`); this
/// pins its three edges, and that the ring and the tooltip's landing check both read it.
///
/// Specified, on two rows with the macOS click rule (a click leaves focus where it is, so only the flag changes): after
/// Tab lands on the first row and its help shows, the pointer moving to empty space beside the rows keeps both the
/// ring and the help. Focus the app then moves to the second row, with no key or press since the move, shows that
/// row's ring and, after the hover delay, its help. A click on the second row leaves focus there, and the frame on
/// screen once the click is handled shows no ring and no help. A key press after that, which moves no focus, brings
/// the ring back.
#[gpui::test]
fn the_ring_and_help_outlast_pointer_movement_and_a_press_ends_the_ring_until_a_key(
    cx: &mut TestAppContext,
) {
    let cx = open(cx, |_, _| {
        let row = |id: &'static str, help: &'static str| {
            ListRow::new(id, id, HoverHelp::tooltip(help), Role::Button).line(id)
        };
        div()
            .w(px(300.))
            .flex()
            .flex_col()
            .child(row("first", "Open the first session"))
            .child(row("second", "Open the second session"))
            .into_any_element()
    });
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    let accent = cx.update(|_, cx| theme::color(theme::palette(cx).accent));

    // The second row's handle lives in its element state, so take it from where a Tab leaves focus.
    keys(cx, "tab tab");
    let second = cx.update(|window, cx| window.focused(cx));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("second"),
        "the premise: two Tabs reached the second row"
    );
    let Some(second) = second else {
        panic!("the premise: the second row holds focus");
    };
    keys(cx, "shift-tab");
    wait_hover_delay(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("first"),
        "the premise: Shift+Tab focused the first row"
    );
    assert_eq!(
        ring(cx, "first"),
        Some(accent),
        "the premise: the keyboard ringed the first row"
    );
    assert_eq!(
        tooltip(cx).as_deref(),
        Some("Open the first session"),
        "the premise: the keyboard landing showed the first row's help"
    );

    let first = bounds(cx, "first");
    let beside = point(first.right() + px(40.), first.center().y);
    cx.simulate_mouse_move(beside, None, Modifiers::none());
    redraw(cx);
    assert!(
        !gpui_keyboard_flag(cx),
        "the premise: the move cleared GPUI's own flag"
    );
    assert_eq!(
        ring(cx, "first"),
        Some(accent),
        "the pointer moving kept the ring"
    );
    assert_eq!(
        tooltip(cx).as_deref(),
        Some("Open the first session"),
        "and kept the help"
    );

    cx.update(|window, cx| {
        focus::focus_control(&second, &focus::Mark::take(cx), window, cx);
    });
    redraw(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("second"),
        "the premise: the app moved focus to the second row"
    );
    assert_eq!(
        ring(cx, "second"),
        Some(accent),
        "focus landing after the pointer move shows the ring"
    );
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx).as_deref(),
        Some("Open the second session"),
        "and the landed row's help"
    );

    click(cx, "second");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("second"),
        "the premise: under the macOS rule the click left focus on the row"
    );
    assert_eq!(
        painted_ring(cx, "second"),
        None,
        "the click took the ring away"
    );
    assert_eq!(tooltip(cx), None, "and put the help away");

    keys(cx, "a");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("second"),
        "the premise: the key moved no focus"
    );
    assert_eq!(
        painted_ring(cx, "second"),
        Some(accent),
        "the key brought the ring back"
    );
}

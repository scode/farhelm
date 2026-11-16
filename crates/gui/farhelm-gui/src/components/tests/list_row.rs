//! Tests of a list row's open control (`components/list_row.rs`): the keyboard focus ring it paints inside its edge, in
//! the accent on an ordinary row and the on-fill ink on the selected one, and never for a click.

use gpui::{
    Hsla, IntoElement as _, ParentElement as _, Styled as _, TestAppContext, VisualTestContext,
    div, px,
};

use crate::components::{HoverHelp, ListRow, Role};
use crate::test_support::{bounds, click, focused_id, keys, open, redraw};
use crate::{focus, theme};

/// The color of the ring painted over the row control drawn under `id` in a fresh frame, if any: the painted quad that
/// covers the control's bounds exactly with a 2-pixel border. The control's own fill is a quad over the same bounds
/// with no border, so the border is what tells the ring apart.
fn ring(cx: &mut VisualTestContext, id: &'static str) -> Option<Hsla> {
    redraw(cx);
    let row = bounds(cx, id);
    let (scale, quads) = cx.update(|window, _| (window.scale_factor(), window.painted_quads()));
    let (row, width) = (row.scale(scale), px(2.).scale(scale));
    quads
        .into_iter()
        .find(|quad| quad.bounds == row && quad.border_widths.top == width)
        .map(|quad| quad.border_color)
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

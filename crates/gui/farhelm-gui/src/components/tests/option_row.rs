//! Tests of the option row: it activates on a click and never takes focus from the field that drives its list, and
//! its selected state draws the accent fill with the fill's ink.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    Background, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _,
    TestAppContext, VisualTestContext, canvas, div, px,
};

use crate::components::{Button, HoverHelp, OptionRow, Role, TextInput, TextInputState};
use crate::test_support::{bounds, click, frame, keys, open};
use crate::theme;

/// Why this matters: the quick switcher keeps focus in its search field while the user picks a row, as the old one
/// did with `tabindex="-1"` rows, so typing never stops going to the field. A row that took focus on a click, or that
/// Tab could reach, would pull the keyboard out of the field.
///
/// Specification: with a focused field, an option row and a button, a click on the row runs its handler once and
/// leaves focus in the field; Tab from the field goes to the button, past the row; the row reports its role and label.
#[gpui::test]
fn an_option_row_activates_on_a_click_and_never_takes_focus(cx: &mut TestAppContext) {
    let clicks = Rc::new(Cell::new(0));
    let made: Rc<RefCell<Option<TextInputState>>> = Rc::new(RefCell::new(None));
    let button = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (counted, kept, focus) = (clicks.clone(), made.clone(), button.clone());
    let cx = open(cx, move |window, cx| {
        let state = kept
            .borrow_mut()
            .get_or_insert_with(|| TextInputState::new(window, cx))
            .clone();
        let counted = counted.clone();
        div()
            .flex()
            .flex_col()
            .w(px(240.))
            .child(
                div()
                    .debug_selector(|| "field-box".into())
                    .child(TextInput::new(
                        "field",
                        &state,
                        "Search",
                        HoverHelp::tooltip("A test field"),
                    )),
            )
            .child(
                OptionRow::new(
                    "option",
                    "First result",
                    HoverHelp::tooltip("Open it"),
                    Role::ListBoxOption,
                )
                .selected(true)
                .child(div().child("First result"))
                .on_click(move |_, _, _| counted.set(counted.get() + 1)),
            )
            .child(
                Button::new(
                    "after",
                    "After",
                    HoverHelp::tooltip("A test control"),
                    Role::Button,
                )
                .focus(&focus),
            )
            .into_any_element()
    });
    let state = made.borrow().clone().expect("the field was drawn");
    let field_focused = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| state.focus_handle(cx).is_focused(window))
    };
    click(cx, "field-box");
    assert!(field_focused(cx), "the premise: the field has focus");

    click(cx, "option");
    assert_eq!(clicks.get(), 1, "the click activated the row");
    assert!(field_focused(cx), "and focus stayed in the field");

    keys(cx, "tab");
    assert!(
        cx.update(|window, _| button.is_focused(window)),
        "Tab went past the row to the button"
    );

    let row = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("option"))
        .map(|node| (node.role, node.label));
    assert_eq!(
        row,
        Some((Some(Role::ListBoxOption), Some("First result".into())))
    );
}

/// Why this matters: the selected row's ink used to be set by each list that draws option rows (the quick switcher,
/// the launcher's model and host lists), three copies of one rule, and a list that forgot it would draw its words in
/// the resting ink on the accent fill, which in the light theme is dark text on a saturated blue. The old sheet tied
/// the two together on the row itself, and so does the row now.
///
/// Specification: of two rows drawn one above the other with plain content, the selected one paints the accent fill
/// over its whole box and gives its content the fill's ink (`on_fill`); the other paints no accent fill and gives its
/// content the resting ink (`fg_0`), whatever ink surrounds it.
#[gpui::test]
fn the_selected_option_row_draws_the_fill_with_its_ink(cx: &mut TestAppContext) {
    // What each row's content inherited as its text color, read while it paints, which is the ink its text draws in.
    let inks: Rc<RefCell<[Option<Hsla>; 2]>> = Rc::new(RefCell::new([None; 2]));
    let seen = inks.clone();
    let cx = open(cx, move |_, cx| {
        let row = |index: usize, id: &'static str, selected: bool| {
            let seen = seen.clone();
            let probe = canvas(
                |_, _, _| (),
                move |_, (), window, _| seen.borrow_mut()[index] = Some(window.text_style().color),
            )
            .size(px(4.));
            OptionRow::new(
                id,
                id,
                HoverHelp::tooltip("A test row"),
                Role::ListBoxOption,
            )
            .selected(selected)
            .child(div().child("A result"))
            .child(probe)
        };
        div()
            .flex()
            .flex_col()
            .w(px(240.))
            // An ambient ink other than the resting one, so the resting row's ink is shown to be its own rather than
            // inherited: the harness's root already draws in `fg_0`.
            .text_color(theme::color(theme::palette(cx).fg_1))
            .child(row(0, "picked", true))
            .child(row(1, "other", false))
            .into_any_element()
    });
    let palette = cx.update(|_, cx| theme::palette(cx));
    assert_ne!(
        palette.on_fill, palette.fg_0,
        "the premise: the two inks differ, so the probe tells them apart"
    );
    assert_ne!(
        palette.fg_1, palette.fg_0,
        "the premise: the ambient ink differs from the resting ink"
    );

    let picked = bounds(cx, "picked");
    let (scale, quads) = cx.update(|window, _| (window.scale_factor(), window.painted_quads()));
    let accent = Background::from(theme::color(palette.accent_fill));
    let filled: Vec<_> = quads
        .iter()
        .filter(|quad| quad.background == accent)
        .map(|quad| quad.bounds)
        .collect();
    assert_eq!(
        filled,
        [picked.scale(scale)],
        "only the selected row paints the accent fill, over its own box"
    );
    assert_eq!(
        *inks.borrow(),
        [
            Some(theme::color(palette.on_fill)),
            Some(theme::color(palette.fg_0))
        ],
        "the selected row's content takes the fill's ink, the other's the resting ink"
    );
}

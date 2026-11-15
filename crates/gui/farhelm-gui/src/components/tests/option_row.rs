//! Tests of the option row: it activates on a click and never takes focus from the field that drives its list.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    InteractiveElement as _, IntoElement, ParentElement as _, Styled as _, TestAppContext,
    VisualTestContext, div, px,
};

use crate::components::{Button, HoverHelp, OptionRow, Role, TextInput, TextInputState};
use crate::test_support::{click, frame, keys, open};

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

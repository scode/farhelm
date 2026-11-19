//! Tests of the copy button: it copies the text it was given and keeps focus.

use gpui::{IntoElement, ParentElement as _, TestAppContext, div};

use crate::components::{HoverHelp, copy_button};
use crate::test_support::{click, keys, open};

/// Copy leaves focus on the button so the keyboard user can continue from it. With the button focused by Tab,
/// clicking copies the supplied text exactly, including line breaks, and leaves the button focused.
#[gpui::test]
fn a_copy_button_copies_its_text_and_keeps_focus(cx: &mut TestAppContext) {
    let focus = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let handle = focus.clone();
    let cx = open(cx, move |_, _| {
        div()
            .child(
                copy_button(
                    "copy",
                    "Copy plan",
                    HoverHelp::tooltip("Copy the complete host plan"),
                    "first line\nsecond line\n",
                )
                .focus(&handle),
            )
            .into_any_element()
    });
    keys(cx, "tab");
    assert!(
        cx.update(|window, _| focus.is_focused(window)),
        "the premise: the button has focus"
    );
    assert_eq!(
        cx.read_from_clipboard(),
        None,
        "the premise: nothing copied yet"
    );
    click(cx, "copy");
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some("first line\nsecond line\n")
    );
    assert!(
        cx.update(|window, _| focus.is_focused(window)),
        "focus stayed on the button"
    );
}

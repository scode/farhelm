//! Tests of the copy button: it copies the text it was given and keeps focus.

use gpui::{IntoElement, ParentElement as _, TestAppContext, div};

use crate::components::{HoverHelp, copy_button};
use crate::test_support::{click, keys, open};

/// The copy control copies the text it was given, the text the user was shown, and stays where it is: the old copy
/// buttons (Copy plan, SSH help's commands) left focus on the button so a keyboard user could go on from it. Specified:
/// with the copy button focused (Tab, so under either platform's click rule), a click puts its text on the clipboard
/// exactly, line breaks and all, and focus is still on the button.
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

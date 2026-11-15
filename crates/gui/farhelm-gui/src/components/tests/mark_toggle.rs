//! Tests of the mark toggle: it takes its click from the control around it and takes no focus.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    InteractiveElement as _, IntoElement, Modifiers, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, TestAppContext, div, point, px,
};

use crate::components::{Button, HoverHelp, Role};
use crate::test_support::{bounds, center, check, click, frame, keys, open};

use super::{tooltip, wait_hover_delay};

/// A mark toggle sits inside a control with a click of its own (a session's status dot inside the row that opens the
/// session), so a click on the mark that also reached the control around it would do two things at once: toggle the
/// session's seen state and open it. It takes no focus, since the action has a keyboard path elsewhere (the row's
/// menu), and a press that moved focus would pull it off whatever the user was typing into. An unavailable mark must
/// not become a hole through which the click opens the row after all.
///
/// Specification: inside a parent that counts its clicks, with a button focused, a click on an enabled mark runs its
/// handler once, the parent's never, and focus stays on the button (on Linux, where a click focuses the control it
/// lands on); a click on a mark without a handler runs nothing and still reaches no parent; a click on the parent
/// beside the marks reaches it; the mark reports its role and label, is not focusable, and shows its help on hover.
#[gpui::test]
fn a_mark_toggle_takes_its_click_from_the_control_around_it_and_no_focus(cx: &mut TestAppContext) {
    let (toggles, outer) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let focus = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (toggled, opened, button) = (toggles.clone(), outer.clone(), focus.clone());
    let cx = open(cx, move |_, _| {
        let (toggled, opened) = (toggled.clone(), opened.clone());
        let mark = || div().w(px(8.)).h(px(8.));
        div()
            .flex()
            .gap(px(8.))
            .child(
                div()
                    .id("outer")
                    .debug_selector(|| "outer".into())
                    .flex()
                    .gap(px(8.))
                    .p(px(20.))
                    .on_click(move |_, _, _| opened.set(opened.get() + 1))
                    .child(
                        crate::components::MarkToggle::new(
                            "enabled",
                            "Mark read",
                            HoverHelp::tooltip("Idle — new output — Mark read"),
                            Role::Button,
                        )
                        .mark(mark())
                        .on_click(move |_, _, _| toggled.set(toggled.get() + 1)),
                    )
                    .child(
                        crate::components::MarkToggle::new(
                            "unavailable",
                            "Mark read",
                            HoverHelp::tooltip("Idle — new output — Mark read"),
                            Role::Button,
                        )
                        .mark(mark()),
                    ),
            )
            .child(
                Button::new(
                    "after",
                    "After",
                    HoverHelp::tooltip("A test control"),
                    Role::Button,
                )
                .focus(&button),
            )
            .into_any_element()
    });
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(true, cx));
    keys(cx, "tab");
    assert!(
        cx.update(|window, _| focus.is_focused(window)),
        "the premise: the button has focus"
    );

    click(cx, "enabled");
    assert_eq!((toggles.get(), outer.get()), (1, 0), "only the mark ran");
    assert!(
        cx.update(|window, _| focus.is_focused(window)),
        "focus stayed on the button"
    );
    click(cx, "unavailable");
    assert_eq!((toggles.get(), outer.get()), (1, 0), "nothing ran");
    let corner = bounds(cx, "outer").origin;
    cx.simulate_click(
        point(corner.x + px(2.), corner.y + px(2.)),
        Modifiers::none(),
    );
    check(cx);
    assert_eq!(
        outer.get(),
        1,
        "the premise: a click beside the marks reaches the parent"
    );

    let nodes = frame(cx);
    let toggle = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("enabled"))
        .expect("the mark was drawn");
    assert_eq!(toggle.role, Some(Role::Button));
    assert_eq!(toggle.label.as_deref(), Some("Mark read"));
    assert!(!toggle.flags.focusable, "it takes no focus");

    let at = center(cx, "enabled");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx).as_deref(),
        Some("Idle — new output — Mark read")
    );
}

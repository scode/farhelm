//! Tests of a toggled menu's key handlers: Down and Up on the closed toggle ask for the menu on its first or last item,
//! and nothing else does; Home and End in the open menu move focus to its first or last item, and nothing else does.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, TestAppContext, Window, div,
};

use crate::components::{Button, HoverHelp, MenuStart, Role, home_end_keys, toggle_arrow_keys};
use crate::focus::Mark;
use crate::layers::{self, Anchor, Popover};
use crate::test_support::{anchor_element, button, focused_id, keys, open};

/// The owners of menus opened from a toggle (the session list's order title, the session header's carets) rely on
/// this handler for the old app's way of opening a closed menu from the keyboard, so which keys open it, and on which
/// item, is pinned here once rather than per owner. A held key must open nothing: a key still held from closing the
/// menu would otherwise open it again. Specified: with the toggle focused, Down asks for the first item and Up for the
/// last; Shift+Down, a held Down and a plain letter ask for nothing.
#[gpui::test]
fn down_and_up_on_a_toggle_ask_for_its_first_and_last_item(cx: &mut TestAppContext) {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let focus = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (seen, handle) = (asked.clone(), focus.clone());
    let cx = open(cx, move |_, _| {
        let seen = seen.clone();
        div()
            .child(
                Button::new(
                    "toggle",
                    "Toggle",
                    HoverHelp::tooltip("Opens a menu"),
                    Role::Button,
                )
                .focus(&handle)
                .on_key_down(toggle_arrow_keys(move |start, _, _| {
                    seen.borrow_mut().push(start)
                })),
            )
            .into_any_element()
    });
    keys(cx, "tab");
    assert!(
        cx.update(|window, _| focus.is_focused(window)),
        "the premise: the toggle has focus"
    );
    keys(cx, "down up shift-down a");
    cx.simulate_event(gpui::KeyDownEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse("down").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    assert_eq!(*asked.borrow(), [MenuStart::First, MenuStart::Last]);
}

/// An open menu with two items, `first` and `last`, whose panel takes Home and End through [`home_end_keys`].
struct TwoItems {
    first: FocusHandle,
    last: FocusHandle,
}

impl Render for TwoItems {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        layers::panel(cx)
            .on_key_down(home_end_keys(self.first.clone(), self.last.clone()))
            .child(button("first", &self.first))
            .child(button("last", &self.last))
    }
}

/// The layer manager's menu context binds only Up and Down, so Home and End in a menu opened from a toggle are this
/// handler's alone: without it they would do nothing, and a long menu would have no keyboard way to its ends, as the
/// old menus had. A modified Home must not move: Shift+Home is not the menu's to take. Specified: in an open menu of
/// two items with focus on the last, Home moves focus to the first, End back to the last, and Shift+Home leaves it on
/// the last.
#[gpui::test]
fn home_and_end_in_an_open_menu_move_to_its_first_and_last_item(cx: &mut TestAppContext) {
    let anchor: Rc<RefCell<Option<Anchor>>> = Rc::default();
    let out = anchor.clone();
    let cx = open(cx, move |window, cx| {
        anchor_element("toggle", &out, window, cx).into_any_element()
    });
    let anchor = anchor.borrow().clone().expect("the anchor was drawn");
    cx.update(|window, cx| {
        let (first, last) = (
            cx.focus_handle().tab_stop(true),
            cx.focus_handle().tab_stop(true),
        );
        let start = last.clone();
        let view = cx.new(|_| TwoItems { first, last });
        let mark = Mark::take(cx);
        layers::open_popover(
            Popover::menu(view, cx.focus_handle(), anchor, start),
            &mark,
            window,
            cx,
        )
        .expect("the menu opened");
    });
    cx.run_until_parked();
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("last"),
        "the premise: focus is on the last item"
    );
    keys(cx, "home");
    assert_eq!(focused_id(cx).as_deref(), Some("first"), "Home");
    keys(cx, "end");
    assert_eq!(focused_id(cx).as_deref(), Some("last"), "End");
    keys(cx, "shift-home");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("last"),
        "Shift+Home moves nothing"
    );
}

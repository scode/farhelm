//! Tests of the focus stop (`components::focus_stop`, the layer's rule under every scroll stop): a handle its caller
//! owns can hold focus before the stop is first drawn, the stop is in Tab order with its role and name and shows its
//! ring only for the keyboard, and it takes none of a scroll stop's keys.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AnyElement, App, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _,
    Styled as _, TestAppContext, Window, div, px,
};

use crate::components::{Button, HoverHelp, Ring, Role, focus_stop};
use crate::focus::{self, TabReach};
use crate::test_support::{focused, focused_id, follow, keys, open, redraw, settle};

use super::role_and_name;

/// Why this matters: the session pane makes its interrupted card home (SPEC.md, "Where focus rests") as it learns the
/// card is to show, before the card's first frame. A stop whose handle lived in its element state, as a scroll stop's
/// does, would have no handle to make home then, which is why the focus stop takes its caller's. The ring belongs to
/// the keyboard (the old `:focus-visible`), so focus that came to rest there without a key press draws none.
///
/// Specified: a caller's handle made home while the stop is not drawn holds focus; once the stop is drawn with that
/// handle, the stop is the focused element, with no ring, since no key was pressed.
#[gpui::test]
fn a_callers_handle_holds_focus_before_the_stop_is_first_drawn(cx: &mut TestAppContext) {
    let home = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let shown = Rc::new(Cell::new(false));
    let (drawn, show) = (home.clone(), shown.clone());
    let cx = open(cx, move |window, cx| {
        if show.get() {
            card(&drawn, window, cx)
        } else {
            div().into_any_element()
        }
    });
    cx.update(|window, cx| focus::set_home(Some(&home), window, cx));
    settle(cx);
    assert!(
        cx.debug_bounds("card").is_none(),
        "the premise: the stop is not drawn yet"
    );
    assert!(focused(cx, &home), "the caller's handle holds focus");

    shown.set(true);
    redraw(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("card"),
        "once drawn, the stop is what holds focus"
    );
    assert!(
        cx.debug_bounds("card-ring").is_none(),
        "no ring without the keyboard"
    );
}

/// Why this matters: a focus stop is reached with Tab like any stop, and on macOS with Keyboard navigation off, the
/// system default, plain Tab skips buttons; a stop counted as one would be out of plain Tab's reach there. Assistive
/// technology reads its role and name where focus rests.
///
/// Specified: with Tab reaching text fields and pop-ups only, plain Tab passes over a button before the stop and puts
/// focus on the stop, which shows its ring and reports the role and label it was given.
#[gpui::test]
fn plain_tab_reaches_the_stop_which_shows_its_ring_role_and_name(cx: &mut TestAppContext) {
    let (before, stop) = cx.update(|cx| {
        (
            cx.focus_handle().tab_stop(true),
            cx.focus_handle().tab_stop(true),
        )
    });
    let (drawn_before, drawn_stop) = (before.clone(), stop.clone());
    let cx = open(cx, move |window, cx| {
        let help = HoverHelp::tooltip("A button");
        div()
            .flex()
            .flex_col()
            .child(Button::new("before", "Before", help, Role::Button).focus(&drawn_before))
            .child(card(&drawn_stop, window, cx))
            .into_any_element()
    });
    let reach = Rc::new(Cell::new(TabReach::TextFieldsAndPopups));
    follow(cx, &reach);

    keys(cx, "tab");
    assert!(!focused(cx, &before), "plain Tab passed over the button");
    assert!(focused(cx, &stop), "and reached the stop");
    assert!(
        cx.debug_bounds("card-ring").is_some(),
        "the stop shows its ring"
    );
    assert_eq!(
        role_and_name(cx, "card"),
        (Some(Role::Group), Some("Session ended".into())),
        "the stop has its role and label"
    );
}

/// Why this matters: the interrupted card does not scroll, and the keys a scroll stop takes (Space, the arrows, Home,
/// End, the page keys) belong to whatever holds the card, as for any element that takes no keys. A focus stop that
/// stopped them would swallow them for nothing.
///
/// Specified: with the stop focused, each of the scroll stop's keys goes on to a key listener around it.
#[gpui::test]
fn the_stop_takes_none_of_the_scroll_keys(cx: &mut TestAppContext) {
    let stop = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let seen: Rc<RefCell<Vec<String>>> = Rc::default();
    let (drawn, heard) = (stop.clone(), seen.clone());
    let cx = open(cx, move |window, cx| {
        let heard = heard.clone();
        div()
            .on_key_down(move |event, _, _| heard.borrow_mut().push(event.keystroke.key.clone()))
            .child(card(&drawn, window, cx))
            .into_any_element()
    });
    keys(cx, "tab");
    assert!(focused(cx, &stop), "the premise: the stop holds focus");

    keys(cx, "down up pagedown pageup space home end");
    assert_eq!(
        *seen.borrow(),
        ["down", "up", "pagedown", "pageup", "space", "home", "end"],
        "every key went on past the stop"
    );
}

/// A focus stop shaped like the interrupted card (a group named by its words, with a ring along its edge), drawn under
/// `card` and focused through `focus`, the caller's handle.
fn card(focus: &FocusHandle, window: &mut Window, cx: &mut App) -> AnyElement {
    let card = div()
        .id("card")
        .debug_selector(|| "card".into())
        .w(px(300.))
        .h(px(80.))
        .child("Session ended");
    let ring = Ring {
        inset_x: px(0.),
        inset_y: px(0.),
        radius: px(4.),
    };
    let label = "Session ended".into();
    focus_stop(card, "card", focus, Role::Group, label, ring, window, cx).into_any_element()
}

//! Tests of the scrollbar: the slider's arithmetic, a press and drag on the track, a drag ending with the bar, and a
//! bar with nothing to scroll not being there.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    FocusHandle, InteractiveElement as _, IntoElement, Modifiers, MouseButton, ParentElement as _,
    Styled as _, TestAppContext, VisualTestContext, div, point, px,
};

use crate::components::{
    Button, HoverHelp, Role, Scrollbar, ScrollbarEvent, ScrollbarLines, scrollbar,
};
use crate::focus;
use crate::test_support::{bounds, click, open, redraw};

/// The slider's size and place follow the old bar's arithmetic: its height is the visible share of the track, never
/// under 20 pixels, rounded to whole pixels; it sits at the top for the oldest line and at the bottom for the live
/// screen. A drag inverts that, so the slider's top at either end of its travel names the oldest line and the screen,
/// and a drag past either end stays there. A bar that disagreed with the view about where it is would jump under the
/// pointer at the first drag.
#[test]
fn the_slider_is_the_visible_share_of_its_track_and_a_drag_inverts_it() {
    let lines = |top| ScrollbarLines {
        visible: 10,
        total: 100,
        top,
    };
    assert_eq!(scrollbar::slider(lines(0), 200.), (0., 20.));
    assert_eq!(scrollbar::slider(lines(90), 200.), (180., 20.));
    assert_eq!(scrollbar::slider(lines(45), 200.), (90., 20.));
    let few = ScrollbarLines {
        visible: 10,
        total: 1000,
        top: 0,
    };
    assert_eq!(
        scrollbar::slider(few, 200.).1,
        scrollbar::SLIDER_MIN_HEIGHT,
        "never shorter than the minimum"
    );
    let half = ScrollbarLines {
        visible: 10,
        total: 20,
        top: 0,
    };
    assert_eq!(
        scrollbar::slider(half, 201.).1,
        101.,
        "rounded to whole pixels"
    );
    assert_eq!(scrollbar::top_at(lines(0), 200., 0.), 0.);
    assert_eq!(scrollbar::top_at(lines(0), 200., 180.), 90.);
    assert_eq!(scrollbar::top_at(lines(0), 200., 90.), 45.);
    assert_eq!(scrollbar::top_at(lines(0), 200., -50.), 0., "past the top");
    assert_eq!(
        scrollbar::top_at(lines(0), 200., 500.),
        90.,
        "past the bottom"
    );
    assert!(
        !ScrollbarLines {
            visible: 10,
            total: 10,
            top: 0
        }
        .scrollable(),
        "nothing to scroll"
    );
}

/// What a scrollbar test's bar asked of its owner, in order.
type Events = Rc<RefCell<Vec<ScrollbarEvent>>>;

/// A bar of `total` lines, 10 visible, scrolled to the live screen, 14 by 200 pixels inside an area that a press
/// focuses (as the terminal's grid is), beside a button that holds focus at the start. Returns the bar's events, in
/// order, and the area's and the button's focus handles.
fn scrollbar_scene(
    cx: &mut TestAppContext,
    total: usize,
) -> (&mut VisualTestContext, Events, (FocusHandle, FocusHandle)) {
    let area = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();
    let tracked = area.clone();
    let cx = open(cx, move |_, cx| {
        let seen = seen.clone();
        let bar = Scrollbar::new(
            "bar",
            "History",
            HoverHelp::tooltip("Drag to scroll"),
            Role::ScrollBar,
            ScrollbarLines {
                visible: 10,
                total,
                top: total.saturating_sub(10),
            },
            crate::theme::color(crate::theme::palette(cx).fg_dim),
        )
        .shown(true)
        .on_scroll(move |event, _, _| seen.borrow_mut().push(event));
        let grid = div()
            .id("grid")
            .debug_selector(|| "grid".into())
            .flex()
            .w(px(100.))
            .h(px(200.))
            .child(div().flex_1())
            .child(div().w(px(14.)).h(px(200.)).child(bar));
        div()
            .flex()
            .gap(px(8.))
            .child(focus::track(grid, &tracked, focus::OnPress::Focus))
            .child(Button::new(
                "other",
                "Other",
                HoverHelp::tooltip("Other"),
                Role::Button,
            ))
            .into_any_element()
    });
    click(cx, "other");
    let other = cx
        .update(|window, cx| window.focused(cx))
        .expect("the premise: a click focuses the button");
    (cx, events, (area, other))
}

/// A press on the track centers the slider on the pointer and scrolls there at once, then the slider follows the
/// pointer until the release, also once the pointer has left the bar's few pixels (the old bar's document-level drag);
/// the release tells the owner the drag ended. A press on the slider grabs it where it was pressed and moves nothing
/// until the pointer does. A drag whose release the window never saw ends at the next move without the button held.
/// Only the primary button scrolls, and no press on the bar moves focus: the old bar prevented the press's default, so
/// the terminal kept its focus and the program heard nothing.
#[gpui::test]
fn a_press_on_the_track_jumps_there_and_a_drag_follows_the_pointer(cx: &mut TestAppContext) {
    let (cx, events, (area, other)) = scrollbar_scene(cx, 100);
    let none = Modifiers::none();
    // Points relative to the bar's top left, wherever the harness drew it.
    let origin = bounds(cx, "bar").origin;
    let at = |x: f32, y: f32| origin + point(px(x), px(y));
    let take =
        |events: &Rc<RefCell<Vec<ScrollbarEvent>>>| std::mem::take(&mut *events.borrow_mut());

    // The slider (20 pixels) is at the bottom; a press near the top centers it there, clamped to the oldest line.
    cx.simulate_mouse_down(at(7., 5.), MouseButton::Left, none);
    assert_eq!(take(&events), [ScrollbarEvent::Scroll(0.)]);
    // Halfway down the track, and well to the right of the bar: the slider's top is the pointer less half its height.
    cx.simulate_mouse_move(at(300., 100.), MouseButton::Left, none);
    assert_eq!(take(&events), [ScrollbarEvent::Scroll(45.)]);
    cx.simulate_mouse_up(at(300., 100.), MouseButton::Left, none);
    assert_eq!(take(&events), [ScrollbarEvent::Released]);
    cx.simulate_mouse_move(at(7., 150.), None, none);
    assert_eq!(take(&events), [], "no drag after the release");

    // A press on the slider (at the bottom, 180 to 200) moves nothing; dragging it up 90 pixels moves the top 45 lines.
    cx.simulate_mouse_down(at(7., 190.), MouseButton::Left, none);
    assert_eq!(take(&events), [], "grabbing the slider moves nothing");
    cx.simulate_mouse_move(at(7., 100.), MouseButton::Left, none);
    assert_eq!(take(&events), [ScrollbarEvent::Scroll(45.)]);
    cx.simulate_mouse_up(at(7., 100.), MouseButton::Left, none);
    take(&events);

    // A release the window never saw (the button let go outside it): the next move, with no button held, ends the
    // drag rather than leaving the bar grabbed.
    cx.simulate_mouse_down(at(7., 5.), MouseButton::Left, none);
    take(&events);
    cx.simulate_mouse_move(at(7., 100.), None, none);
    assert_eq!(take(&events), [ScrollbarEvent::Released]);
    cx.simulate_mouse_move(at(7., 150.), MouseButton::Left, none);
    assert_eq!(take(&events), [], "the drag is over");

    cx.simulate_mouse_down(at(7., 5.), MouseButton::Right, none);
    cx.simulate_mouse_up(at(7., 5.), MouseButton::Right, none);
    assert_eq!(take(&events), [], "the secondary button does not scroll");
    assert!(
        cx.update(|window, _| other.is_focused(window)),
        "no press on the bar moved focus"
    );
    // The premise of that: a press on the area beside the bar does focus the area.
    cx.simulate_click(at(-50., 100.), none);
    assert!(cx.update(|window, _| area.is_focused(window)));
}

/// A drag cut short because the bar went away (the history stopped being scrollable, as when a program switches to
/// the alternate screen) is over: when history comes back, a move with the button still held scrolls nothing, rather
/// than a stale drag taking the view along with a pointer the user is using for something else.
#[gpui::test]
fn a_drag_ends_when_the_bar_goes_away(cx: &mut TestAppContext) {
    let total = Rc::new(Cell::new(100usize));
    let events: Events = Rc::default();
    let (shown_total, seen) = (total.clone(), events.clone());
    let cx = open(cx, move |_, cx| {
        let seen = seen.clone();
        let total = shown_total.get();
        div()
            .w(px(14.))
            .h(px(200.))
            .child(
                Scrollbar::new(
                    "bar",
                    "History",
                    HoverHelp::tooltip("Drag to scroll"),
                    Role::ScrollBar,
                    ScrollbarLines {
                        visible: 10,
                        total,
                        top: total.saturating_sub(10),
                    },
                    crate::theme::color(crate::theme::palette(cx).fg_dim),
                )
                .shown(true)
                .on_scroll(move |event, _, _| seen.borrow_mut().push(event)),
            )
            .into_any_element()
    });
    let origin = bounds(cx, "bar").origin;
    let at = |y: f32| origin + point(px(7.), px(y));
    cx.simulate_mouse_down(at(5.), MouseButton::Left, Modifiers::none());
    assert_eq!(
        events.borrow().len(),
        1,
        "the premise: the press started a drag"
    );
    total.set(10);
    redraw(cx);
    total.set(100);
    redraw(cx);
    events.borrow_mut().clear();
    cx.simulate_mouse_move(at(100.), MouseButton::Left, Modifiers::none());
    assert_eq!(
        events.borrow().as_slice(),
        [],
        "the drag ended with the bar"
    );
}

/// With nothing to scroll the bar is not there at all, as the old one was `display: none`: nothing is drawn under its
/// id (so no hover help and no scrollbar for assistive technology), and a press where it would be goes on to what is
/// under it, here the area that a press focuses, as the terminal's grid is.
#[gpui::test]
fn a_bar_with_nothing_to_scroll_is_not_there(cx: &mut TestAppContext) {
    let (cx, events, (area, other)) = scrollbar_scene(cx, 10);
    assert!(
        cx.debug_bounds("bar").is_none(),
        "nothing drawn under the bar's id"
    );
    assert!(
        cx.update(|window, _| other.is_focused(window)),
        "the premise"
    );
    let at = bounds(cx, "grid").top_right() + point(px(-7.), px(5.));
    cx.simulate_click(at, Modifiers::none());
    assert_eq!(events.borrow().as_slice(), []);
    assert!(
        cx.update(|window, _| area.is_focused(window)),
        "the press reached the area under the bar"
    );
}

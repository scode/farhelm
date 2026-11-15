//! Tests of the divider: the drag, double-click and arrow steps it reports to the owner of the width, and its grip
//! keeping the drag from what lies under it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    InteractiveElement as _, IntoElement, Modifiers, ParentElement as _, Styled as _,
    TestAppContext, VisualTestContext, div, point, px,
};

use crate::components::{HoverHelp, Role};
use crate::test_support::{center, check, focused_id, frame, keys, open};

/// The divider owns no width, so everything its owner does rests on it reporting each gesture exactly once and in
/// order: a second `End`, or a `Move` after the drag ended, would make the owner write twice or keep following the
/// pointer. And its grip lies over whatever the divider reaches over, which must never see the drag (a terminal would
/// report it to its program). Specified, with the grip drawn after a probe it overlaps and the Mac's click rule (a
/// click focuses no button), since a separator takes focus on every press anyway: a press reports `Start` at the
/// pointer and focuses the strip, a move with the button held reports `Move` there, and a
/// move with the button no longer held (a release the window never saw) reports one `End`, after which a held move
/// reports nothing; the probe under the grip hears neither the press nor the drag's moves but does hear the move after
/// it; a press and release report `Start` and `End`; the second press of a double-click reports the double-click and no
/// new `Start`; on the focused strip Left and Right report a step back and forward and a Shift-modified arrow nothing;
/// and the strip's role, name and value reach assistive technology.
#[gpui::test]
fn a_divider_reports_its_drag_double_click_and_arrows(cx: &mut TestAppContext) {
    use crate::components::{Divider, DividerDrag, DividerStep};
    use gpui::{MouseButton, MouseDownEvent, MouseUpEvent};

    let events: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let probe = Rc::new(Cell::new((0, 0)));
    let strip_focus = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (seen, heard, handle) = (events.clone(), probe.clone(), strip_focus.clone());
    let cx = open(cx, move |_, _| {
        let divider = || {
            let (dragged, stepped, doubled) = (seen.clone(), seen.clone(), seen.clone());
            Divider::new(
                "divider",
                "Pane width",
                HoverHelp::tooltip("Drag to resize"),
                Role::Splitter,
                &handle,
                400,
                300..=500,
            )
            .on_drag(move |event, _, _| {
                dragged.borrow_mut().push(match event {
                    DividerDrag::Start(x) => format!("start {}", f32::from(x)),
                    DividerDrag::Move(x) => format!("move {}", f32::from(x)),
                    DividerDrag::End => "end".into(),
                })
            })
            .on_double_click(move |_, _| doubled.borrow_mut().push("double".into()))
            .on_step(move |step, _, _| {
                stepped.borrow_mut().push(match step {
                    DividerStep::Back => "back".into(),
                    DividerStep::Forward => "forward".into(),
                })
            })
        };
        let (pressed, moved) = (heard.clone(), heard.clone());
        // The probe starts under the strip's box and runs on past it, drawn between the strip and its grip, as a pane
        // is drawn between the sidebar's strip and the grip the main view draws after it.
        let probe = div()
            .id("probe")
            .absolute()
            .left(px(0.))
            .top_0()
            .w(px(100.))
            .h(px(200.))
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                pressed.set((pressed.get().0 + 1, pressed.get().1))
            })
            .on_mouse_move(move |_, _, _| moved.set((moved.get().0, moved.get().1 + 1)));
        div()
            .relative()
            .w(px(200.))
            .h(px(200.))
            .child(div().w(px(5.)).h(px(200.)).child(divider()))
            .child(probe)
            .child(
                div()
                    .absolute()
                    .left(px(0.))
                    .top_0()
                    .w(px(5.))
                    .h(px(200.))
                    .child(divider().grip()),
            )
            .into_any_element()
    });
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    let at = center(cx, "divider");
    let x = f32::from(at.x);
    let down = |cx: &mut VisualTestContext, click_count| {
        cx.simulate_event(MouseDownEvent {
            position: at,
            modifiers: Modifiers::none(),
            button: MouseButton::Left,
            click_count,
            first_mouse: false,
        });
        check(cx);
    };
    let up = |cx: &mut VisualTestContext| {
        cx.simulate_event(MouseUpEvent {
            position: at,
            modifiers: Modifiers::none(),
            button: MouseButton::Left,
            click_count: 1,
        });
        check(cx);
    };
    let take = |events: &Rc<RefCell<Vec<String>>>| std::mem::take(&mut *events.borrow_mut());
    let held = |cx: &mut VisualTestContext, dx: f32, button: Option<MouseButton>| {
        cx.simulate_mouse_move(point(at.x + px(dx), at.y), button, Modifiers::none());
        check(cx);
    };

    down(cx, 1);
    assert!(
        cx.update(|window, _| strip_focus.is_focused(window)),
        "the press focused the strip"
    );
    held(cx, 30., Some(MouseButton::Left));
    held(cx, 40., None);
    assert_eq!(
        probe.get(),
        (0, 0),
        "nothing under the grip saw the press or the drag"
    );
    held(cx, 50., Some(MouseButton::Left));
    assert_eq!(
        take(&events),
        [
            format!("start {x}"),
            format!("move {}", x + 30.),
            "end".into()
        ]
    );
    assert_eq!(
        probe.get(),
        (0, 1),
        "a move after the drag reaches what is under it"
    );

    down(cx, 1);
    up(cx);
    down(cx, 2);
    up(cx);
    assert_eq!(
        take(&events),
        [format!("start {x}"), "end".into(), "double".into()]
    );

    assert_eq!(focused_id(cx).as_deref(), Some("divider"));
    keys(cx, "left right shift-right");
    assert_eq!(take(&events), ["back", "forward"]);

    let nodes = frame(cx);
    let divider = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("divider"))
        .expect("the divider was drawn");
    assert_eq!(
        (
            divider.role,
            divider.label.as_deref(),
            divider.value.as_deref()
        ),
        (Some(Role::Splitter), Some("Pane width"), Some("400"))
    );
}

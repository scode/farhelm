//! Tests of the dialog frame: a long body scrolls with the answers in view, a frame too tall for the window scrolls as
//! a whole, the answers wrap in a narrow window, the header's Close comes first in Tab's order, and a body given a
//! scroll stop has it whenever it holds nothing to focus, and scrolls by keyboard through it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    FocusHandle, IntoElement, Modifiers, ParentElement as _, ScrollDelta, ScrollEvent, Styled as _,
    TestAppContext, VisualTestContext, div, point, px, size,
};

use crate::components::{Button, DialogFrame, HoverHelp, Role, TextInput, TextInputState, Tier};
use crate::test_support::{bounds, check, click, focused_id, keys, open, redraw};

use super::{each_scroll_key_scrolls, lines, role_and_name};

/// In a short window a long dialog body scrolls while the answers stay inside the visible frame. The old app lost
/// answers below the window edge more than once before its frame guaranteed this; the frame now does it for every
/// dialog.
#[gpui::test]
fn a_long_dialog_body_scrolls_and_the_answers_stay_visible(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        DialogFrame::new(
            "long",
            "A long dialog",
            div().children((0..60).map(|line| div().child(format!("line {line}")))),
            vec![
                Button::new(
                    "cancel",
                    "Cancel",
                    HoverHelp::tooltip("Close"),
                    Role::Button,
                )
                .tier(Tier::Neutral),
                Button::new("ok", "Continue", HoverHelp::tooltip("Go on"), Role::Button)
                    .tier(Tier::Primary),
            ],
        )
        .into_any_element()
    });
    cx.simulate_resize(size(px(720.), px(400.)));
    cx.run_until_parked();
    check(cx);

    let frame = bounds(cx, "long");
    let answers = bounds(cx, "long-answers");
    let body = bounds(cx, "long-body");
    assert!(
        frame.size.height <= px(400. - 48.),
        "the frame keeps its margin to the window edges: {frame:?}"
    );
    assert!(
        frame.bottom() >= answers.bottom(),
        "the answers are inside the frame: {answers:?} in {frame:?}"
    );
    assert!(
        body.size.height < px(60. * 12.),
        "the body is clipped to scroll, not grown to its content: {body:?}"
    );
}

/// When even the title and the answers do not fit a very short window, the whole frame scrolls, so the answers stay
/// reachable instead of being cut off (the old `.host-dialog-framed`). The body keeps its three-line floor rather than
/// collapsing to nothing.
#[gpui::test]
fn a_frame_too_tall_for_the_window_scrolls_as_a_whole(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        DialogFrame::new(
            "short",
            "A dialog in a very short window",
            div().children((0..20).map(|line| div().child(format!("line {line}")))),
            vec![
                Button::new("ok", "Continue", HoverHelp::tooltip("Go on"), Role::Button)
                    .tier(Tier::Primary),
            ],
        )
        .summary("with a summary line under the title")
        .into_any_element()
    });
    cx.simulate_resize(size(px(720.), px(160.)));
    cx.run_until_parked();
    check(cx);

    let frame = bounds(cx, "short");
    let answers = bounds(cx, "short-answers");
    assert!(
        frame.size.height <= px(160. - 48.),
        "the frame keeps its margin: {frame:?}"
    );
    assert!(
        answers.bottom() > frame.bottom(),
        "the premise: the answers do not fit the frame before scrolling: {answers:?} in {frame:?}"
    );
    assert!(
        bounds(cx, "short-body").size.height >= px(36.),
        "the body keeps its floor"
    );

    cx.simulate_event(ScrollEvent {
        position: frame.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-1000.))),
        modifiers: Modifiers::none(),
        ..Default::default()
    });
    cx.run_until_parked();
    check(cx);
    let frame = bounds(cx, "short");
    let answer = bounds(cx, "ok");
    // The answer button, not the whole footer: GPUI's scroll extent leaves the frame's own border width of the footer's
    // bottom padding below the fold, which hides nothing the user needs.
    assert!(
        answer.top() >= frame.top() && answer.bottom() <= frame.bottom(),
        "scrolling the frame brought the answer into view: {answer:?} in {frame:?}"
    );
}

/// In a narrow window a dialog's answers wrap onto more lines instead of running past the frame's start edge, in each
/// shape the footer has: answers alone, beside a note, and beside a lead. Why this matters: an answers row that cannot
/// wrap is cut at the frame's edge, and the first answer cut off is Cancel, so a question with extra answers (the YOLO
/// question's four) would lose its way out. An earlier version of the footer lead kept the answers from shrinking,
/// which did exactly that to every dialog. Each frame's buttons have ids of their own, since a test finds an element
/// by its id and two elements drawn under one id are one to it.
#[gpui::test]
fn a_dialogs_answers_wrap_in_a_narrow_window_in_every_footer_shape(cx: &mut TestAppContext) {
    const FRAMES: [(&str, [&str; 4]); 3] = [
        ("plain", ["plain-1", "plain-2", "plain-3", "plain-4"]),
        ("noted", ["noted-1", "noted-2", "noted-3", "noted-4"]),
        ("led", ["led-1", "led-2", "led-3", "led-4"]),
    ];
    let answers = |ids: [&'static str; 4]| {
        ids.into_iter()
            .map(|id| {
                Button::new(
                    id,
                    "A rather long answer",
                    HoverHelp::tooltip("Answer"),
                    Role::Button,
                )
                .tier(Tier::Neutral)
            })
            .collect::<Vec<_>>()
    };
    let cx = open(cx, move |_, _| {
        // As wide as the window, as the layer manager's overlay is, so the frame's `max_w_full` holds it to the window.
        div()
            .w_full()
            .flex()
            .flex_col()
            .child(DialogFrame::new(
                "plain",
                "Plain",
                div().child("Body"),
                answers(FRAMES[0].1),
            ))
            .child(
                DialogFrame::new("noted", "Noted", div().child("Body"), answers(FRAMES[1].1))
                    .note("Most choices save immediately"),
            )
            .child(
                DialogFrame::new("led", "Led", div().child("Body"), answers(FRAMES[2].1))
                    .footer_lead(div().child("What the main action would start")),
            )
            .into_any_element()
    });
    cx.simulate_resize(size(px(360.), px(1400.)));
    cx.run_until_parked();
    check(cx);

    for (frame_id, ids) in FRAMES {
        let frame = bounds(cx, frame_id);
        let (first, last) = (bounds(cx, ids[0]), bounds(cx, ids[3]));
        assert!(
            frame.size.width <= px(360.),
            "the premise: the frame is held to the narrow window: {frame:?}"
        );
        assert!(
            first.left() >= frame.left() && last.right() <= frame.right(),
            "{frame_id}: the answers stay inside the frame: {first:?} .. {last:?} in {frame:?}"
        );
        assert!(
            last.top() > first.top(),
            "{frame_id}: the answers wrapped onto another line: {first:?} .. {last:?}"
        );
    }
}

/// Why this matters: SPEC.md gives editors such as Rename "a shared title and Close header" and has Tab follow "Close,
/// the title field, Cancel, Save". The Close must be reachable from the keyboard and come first, or a keyboard user
/// tabbing out of the field would never reach it, and it must do what its dialog says when clicked or pressed.
///
/// Specification: in a frame with a Close, a field and Cancel and Save answers, Tab from nothing focused goes to Close,
/// then the field, then Cancel, then Save, then back to Close; Enter on the focused Close and a click on it each run
/// its handler once. A disabled Close, as Rename draws it while saving (SPEC.md: Close follows Cancel's busy rules),
/// ignores a click.
#[gpui::test]
fn the_dialog_close_comes_first_in_tab_order_and_runs_its_handler(cx: &mut TestAppContext) {
    let closed = Rc::new(Cell::new(0));
    let busy = Rc::new(Cell::new(false));
    let drawn_busy = busy.clone();
    let field: Rc<RefCell<Option<TextInputState>>> = Rc::new(RefCell::new(None));
    let handles: Rc<RefCell<Option<(FocusHandle, FocusHandle)>>> = Rc::new(RefCell::new(None));
    let (count, kept, answers) = (closed.clone(), field.clone(), handles.clone());
    let cx = open(cx, move |window, cx| {
        let state = kept
            .borrow_mut()
            .get_or_insert_with(|| TextInputState::new(window, cx))
            .clone();
        let (cancel, save) = answers
            .borrow_mut()
            .get_or_insert_with(|| {
                (
                    cx.focus_handle().tab_stop(true),
                    cx.focus_handle().tab_stop(true),
                )
            })
            .clone();
        let count = count.clone();
        DialogFrame::new(
            "rename",
            "Rename session",
            TextInput::new("title", &state, "Title", HoverHelp::tooltip("The title")),
            vec![
                Button::new(
                    "cancel",
                    "Cancel",
                    HoverHelp::tooltip("Close"),
                    Role::Button,
                )
                .tier(Tier::Neutral)
                .focus(&cancel),
                Button::new("save", "Save", HoverHelp::tooltip("Save"), Role::Button)
                    .tier(Tier::Primary)
                    .focus(&save),
            ],
        )
        .close(
            "Close rename",
            HoverHelp::tooltip("Close rename; keep the current title"),
            drawn_busy.get(),
            move |_, _, _| count.set(count.get() + 1),
        )
        .into_any_element()
    });
    let (cancel, save) = handles.borrow().clone().expect("the answers were drawn");
    let state = field.borrow().clone().expect("the field was drawn");
    let focused = |cx: &mut VisualTestContext, handle: &FocusHandle| {
        cx.update(|window, _| handle.is_focused(window))
    };

    keys(cx, "tab");
    keys(cx, "enter");
    assert_eq!(
        closed.get(),
        1,
        "the first Tab reached Close, and Enter pressed it"
    );
    keys(cx, "tab x");
    assert_eq!(
        cx.update(|_, cx| state.text(cx)).as_ref(),
        "x",
        "the next Tab reached the field"
    );
    keys(cx, "tab");
    assert!(focused(cx, &cancel), "then Cancel");
    keys(cx, "tab");
    assert!(focused(cx, &save), "then Save");
    keys(cx, "tab enter");
    assert_eq!(closed.get(), 2, "and around to Close again");

    click(cx, "rename-close");
    assert_eq!(closed.get(), 3, "a click on Close ran it");

    busy.set(true);
    redraw(cx);
    click(cx, "rename-close");
    assert_eq!(closed.get(), 3, "a disabled Close ignored the click");
}

/// An announcement-like dialog under `news`, its body `count` lines (`line-0` on) under a link-tier button `link` while
/// `link` holds (the body's only control), with a Close answer focused through `close`, and its body given the stop
/// "Announcement content".
fn announcement(count: usize, link: bool, close: &FocusHandle) -> DialogFrame {
    let body = div()
        .children(link.then(|| {
            Button::new(
                "link",
                "Read more",
                HoverHelp::tooltip("Open the link"),
                Role::Link,
            )
            .tier(Tier::Link)
        }))
        .child(lines(count, "Announcement line"));
    let close = Button::new("close", "Close", HoverHelp::tooltip("Close"), Role::Button)
        .tier(Tier::Neutral)
        .focus(close);
    DialogFrame::new("news", "Announcements", body, vec![close]).scroll_stop("Announcement content")
}

/// Whether the body's stop is drawn: an element with the region role named "Announcement content".
fn body_stop_drawn(cx: &mut VisualTestContext) -> bool {
    role_and_name(cx, "news-body-stop") == (Some(Role::Region), Some("Announcement content".into()))
}

/// Why this matters: SPEC.md, "Reaching everything by keyboard", gives a region with nothing focusable in it its own
/// named tab stop. A long announcement without a link, or a list whose every action is disabled, has no other way in
/// for the keyboard; a body with a control is reached through it, so a stop there would only cost a Tab press. As in
/// the old app, the stop does not wait for the body to overflow. Whether the body holds a control is read from its last
/// drawn frame, so when the control goes, nothing but the stop's own request would draw the frame that shows the stop.
///
/// Specified, in a dialog whose short body (it fits) has the stop and a Close answer: while a link is in the body there
/// is no stop and Tab reaches the link; once the link is gone, with focus on Close (one redraw, the stop asking for the
/// frame after it), the body's stop is a region named by its label, and Tab reaches it.
#[gpui::test]
fn a_dialog_body_has_its_own_stop_whenever_it_holds_nothing_to_focus(cx: &mut TestAppContext) {
    let link = Rc::new(Cell::new(true));
    let close = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let drawn_link = link.clone();
    let cx = open(cx, move |_, _| {
        announcement(3, drawn_link.get(), &close).into_any_element()
    });
    cx.simulate_resize(size(px(720.), px(400.)));
    cx.run_until_parked();
    check(cx);
    assert!(
        bounds(cx, "line-2").bottom() < bounds(cx, "news-body").bottom(),
        "the premise: the short body fits"
    );
    assert!(!body_stop_drawn(cx), "a body holding a link has no stop");
    keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("link"),
        "Tab reaches the link"
    );
    // Off the link before it goes: a focused control going away redraws for its own reasons (the focus coordinator's
    // recovery), which would hide a stop that did not ask for the frame that shows it.
    keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("close"),
        "the premise: Close holds focus"
    );

    link.set(false);
    redraw(cx);
    // Tab before anything else draws: the role check below draws a frame of its own, which would show a stop that
    // never asked for one.
    keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("news-body-stop"),
        "Tab reached the body's stop"
    );
    assert!(body_stop_drawn(cx), "a region named by its label");
}

/// Why this matters: a stop the keys do not scroll leaves the body as unreadable as no stop at all; GPUI scrolls nothing
/// from the keyboard by itself. The keys are the scroll box's (one function draws both stops), and this pins that the
/// dialog's stop is wired to the body's own scroll rather than to some other container.
///
/// Specified: with focus on the stop of a body that overflows by several pages, every scroll key moves the body's
/// content (each_scroll_key_scrolls).
#[gpui::test]
fn the_scroll_keys_scroll_a_dialog_body_through_its_stop(cx: &mut TestAppContext) {
    let close = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let drawn = close.clone();
    let cx = open(cx, move |_, _| {
        announcement(80, false, &drawn).into_any_element()
    });
    cx.simulate_resize(size(px(720.), px(400.)));
    cx.run_until_parked();
    check(cx);
    keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("news-body-stop"),
        "the premise: Tab put focus on the body's stop"
    );
    each_scroll_key_scrolls(cx, "line-0");
}

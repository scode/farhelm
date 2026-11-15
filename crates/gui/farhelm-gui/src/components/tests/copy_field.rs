//! Tests of the copy field: what a click or Enter copies, how long it confirms and what it is named meanwhile, and when
//! its clipboard glyph shows. The hidden-characters answer it leaves for its owner is tested through the details bar,
//! which draws the warning (`session_pane/details_tests.rs`).
//!
//! Each test draws one field over a [`CopyFeedback`] the test holds, as an owner would, and reads GPUI's test clipboard,
//! which stands in for the system's (headless Linux has none).

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AppContext as _, Entity, IntoElement, Modifiers, ParentElement as _, SharedString, Styled as _,
    TestAppContext, VisualTestContext, div, point, px,
};

use super::{painted_under, role_and_name};
use crate::components::{CONFIRMATION, COPIED, CopyFeedback, CopyField, HoverHelp, Role};
use crate::host_text::HostText;
use crate::test_support::{advance, center, click, frame, keys, open};

/// The field's id in every test.
const FIELD: &str = "copy";

/// Open a window drawing one copy field that shows `~/src` and copies `copied`, over a [`CopyFeedback`] the test holds
/// as the field's owner would. Returns the window, that feedback, and a cell holding what the field copies, which the
/// test may change (the field reads it on every frame).
fn field<'a>(
    cx: &'a mut TestAppContext,
    copied: &str,
) -> (
    &'a mut VisualTestContext,
    Entity<CopyFeedback>,
    Rc<RefCell<String>>,
) {
    let feedback = cx.update(|cx| cx.new(|_| CopyFeedback::default()));
    let copies = Rc::new(RefCell::new(copied.to_owned()));
    let (drawn, held) = (feedback.clone(), copies.clone());
    let cx = open(cx, move |_, _| {
        div()
            .child(CopyField::new(
                FIELD,
                "~/src",
                HoverHelp::tooltip("/home/user/src — click to copy"),
                &HostText::from_host("~/src"),
                SharedString::from(held.borrow().clone()),
                &drawn,
            ))
            .into_any_element()
    });
    (cx, feedback, copies)
}

/// What the test clipboard holds, as text.
fn clipboard(cx: &mut VisualTestContext) -> Option<String> {
    cx.read_from_clipboard().and_then(|item| item.text())
}

/// What the field painted, in paint order.
fn painted(cx: &mut VisualTestContext) -> Vec<String> {
    painted_under(&frame(cx), FIELD).0
}

/// SPEC.md, "Terminal experience": "a click confirms locally for about 1.5 seconds", and the old bar's per-click record
/// existed because a plain flag let the first of two quick clicks clear the flag the second had just set, cutting its
/// confirmation short. And a copy that differed from the recorded value would be useless, so the click copies the raw
/// bytes whatever the field shows. Specified: the field shows `~/src` and is named so; a click puts `/home/user/src` on
/// the clipboard, and the field then shows and is named `✓ Copied`; one second on it still does, and a second click
/// then starts the time over: a further second (two since the first click) still shows `✓ Copied`, and only past 1.5
/// seconds after the second click does it show and name `~/src` again. The old field was a button, so Enter copies
/// too: with the field focused (Tab) and copying `/home/user/other`, Enter puts that on the clipboard and confirms.
#[gpui::test]
fn a_click_copies_the_raw_bytes_and_confirms_for_a_moment_per_click(cx: &mut TestAppContext) {
    let (cx, feedback, copies) = field(cx, "/home/user/src");
    assert_eq!(painted(cx), ["~/src"], "the premise: the value shows");
    assert_eq!(clipboard(cx), None, "the premise: nothing copied yet");
    assert_eq!(
        role_and_name(cx, FIELD),
        (Some(Role::Button), Some("~/src".into()))
    );

    click(cx, FIELD);
    assert_eq!(clipboard(cx).as_deref(), Some("/home/user/src"));
    assert_eq!(painted(cx), [COPIED]);
    assert_eq!(role_and_name(cx, FIELD).1.as_deref(), Some(COPIED));
    assert!(cx.update(|_, cx| feedback.read(cx).confirming()));

    advance(cx, Duration::from_secs(1));
    assert_eq!(painted(cx), [COPIED], "still confirming after a second");
    click(cx, FIELD);
    advance(cx, Duration::from_secs(1));
    assert_eq!(
        painted(cx),
        [COPIED],
        "the second click's confirmation is not cut short by the first's deadline"
    );
    advance(cx, CONFIRMATION - Duration::from_secs(1));
    assert_eq!(painted(cx), ["~/src"], "the confirmation is over");
    assert_eq!(role_and_name(cx, FIELD).1.as_deref(), Some("~/src"));
    assert!(!cx.update(|_, cx| feedback.read(cx).confirming()));

    *copies.borrow_mut() = "/home/user/other".to_owned();
    keys(cx, "tab enter");
    assert_eq!(clipboard(cx).as_deref(), Some("/home/user/other"));
    assert_eq!(painted(cx), [COPIED], "Enter copies and confirms");
}

/// What the field painted in the frame the app last drew, without asking for a frame of its own: a test of what
/// redraws the field must not supply the redraw itself. The driver record must be on already ([`frame`] turns it on).
fn drawn(cx: &mut VisualTestContext) -> (Vec<String>, usize) {
    cx.run_until_parked();
    painted_under(
        &cx.update(|window, _| window.driver_nodes().to_vec()),
        FIELD,
    )
}

/// The old field's clipboard glyph was `display: none` at rest and shown under the pointer and keyboard focus
/// (`:hover`, `:focus-visible`), so the value starts at the field's edge until the user reaches for it, and a keyboard
/// user still sees that the field is a copy control. The pointer's part follows GPUI's own hover, which a key press
/// ends while the pointer rests where it was (typing into the terminal with the pointer parked on the bar): a glyph
/// that stayed then would show without the hover fill it belongs with. Each pointer step is read from the frame the
/// app drew by itself, so the test also shows that the pointer's coming and going redraws the field. Specified: at
/// rest no glyph; with the pointer over the field, one glyph before the value; after a key press, with the pointer
/// still there, none; once the pointer moves within the field, one again; with the pointer gone, none; under keyboard
/// focus (Tab), one.
#[gpui::test]
fn the_glyph_follows_the_pointers_hover_and_keyboard_focus(cx: &mut TestAppContext) {
    let (cx, _, _) = field(cx, "/home/user/src");
    assert_eq!(
        painted_under(&frame(cx), FIELD),
        (vec!["~/src".to_owned()], 0),
        "none at rest"
    );
    let at = center(cx, FIELD);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    assert_eq!(
        drawn(cx),
        (vec!["~/src".to_owned()], 1),
        "under the pointer"
    );
    keys(cx, "a");
    assert_eq!(drawn(cx).1, 0, "a key press ends the hover");
    cx.simulate_mouse_move(at + point(px(1.), px(0.)), None, Modifiers::none());
    assert_eq!(drawn(cx).1, 1, "the pointer moving again brings it back");
    cx.simulate_mouse_move(at + point(px(0.), px(200.)), None, Modifiers::none());
    assert_eq!(drawn(cx).1, 0, "the pointer left");
    keys(cx, "tab");
    assert_eq!(drawn(cx).1, 1, "under keyboard focus");
}

/// A field can move out from under a resting pointer with no pointer event at all (the bar's age or title changing
/// width, say). The glyph must go then as the hover fill does, rather than stay on until something unrelated redraws
/// the window: the hover has to be checked again as the moved field is drawn, and the change has to bring a frame of
/// its own. Specified: with the pointer resting on the field and the glyph drawn, the
/// field moves 150 pixels to the right with no pointer event and the window redraws once; once the app is idle, the
/// frame it drew by itself shows no glyph.
#[gpui::test]
fn the_glyph_goes_when_the_field_moves_from_under_a_resting_pointer(cx: &mut TestAppContext) {
    let feedback = cx.update(|cx| cx.new(|_| CopyFeedback::default()));
    let offset = Rc::new(std::cell::Cell::new(0.));
    let (drawn_feedback, drawn_offset) = (feedback.clone(), offset.clone());
    let cx = open(cx, move |_, _| {
        div()
            .flex()
            .pl(px(drawn_offset.get()))
            .child(CopyField::new(
                FIELD,
                "~/src",
                HoverHelp::tooltip("/home/user/src — click to copy"),
                &HostText::from_host("~/src"),
                "/home/user/src",
                &drawn_feedback,
            ))
            .into_any_element()
    });
    let _ = frame(cx);
    let at = center(cx, FIELD);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    assert_eq!(drawn(cx).1, 1, "the premise: the glyph under the pointer");
    offset.set(150.);
    cx.update(|window, _| window.refresh());
    assert_eq!(
        drawn(cx).1,
        0,
        "the field asked for the frame that drops it"
    );
}

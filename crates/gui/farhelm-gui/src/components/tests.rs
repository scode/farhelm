//! Behavior tests for the components, on GPUI's simulated platform.
//!
//! These cover what a component promises beyond its looks (the goldens pin those): activation, the disabled state,
//! hover help reaching the layer manager and leaving with its control, the dialog frame keeping its answers reachable,
//! the text input's events, with the line breaks only the field that keeps them keeps, the arrows a field hands to its
//! owner, the option row that never takes focus, the mark toggle that keeps its click from the control around it, and
//! the select's list, keys and focus (with the focus coordinator's rule for a press on a dialog's empty space while a
//! select's list is open). Each test opens its window through the shared harness ([`crate::test_support::open`]), and
//! each step ends with its invariant checks (the focus coordinator's and the layer manager's rules).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AppContext as _, Context, FocusHandle, InteractiveElement, IntoElement, Modifiers, MouseButton,
    ParentElement, Render, ScrollDelta, ScrollEvent, Styled, Subscription, TestAppContext,
    VisualTestContext, Window, div, point, px, size,
};

use gpui::StatefulInteractiveElement as _;
use gpui::prelude::FluentBuilder as _;

use super::{
    Button, Checkbox, DialogFrame, HoverHelp, OptionRow, RadioGroup, RadioOption, Role, Scrollbar,
    ScrollbarEvent, ScrollbarLines, TextInput, TextInputEvent, TextInputState, Tier, Vertical,
    scrollbar,
};
use super::{Select, SelectOption};
use crate::assets::Icon;
use crate::focus;
use crate::focus::{Mark, TabReach};
use crate::layers;
use crate::layers::Dialog;
use crate::test_support::{
    advance, bounds, center, check, click, focused_id, frame, keys, open, redraw,
};
use gpui::SharedString;

/// A click activates an enabled button, and a disabled one ignores it: the old app's disabled buttons were inert, and a
/// dimmed button that still acted would send a command the user was told was unavailable.
#[gpui::test]
fn a_click_activates_an_enabled_button_and_not_a_disabled_one(cx: &mut TestAppContext) {
    let clicks = Rc::new(Cell::new((0, 0)));
    let counted = clicks.clone();
    let cx = open(cx, move |_, _| {
        let (on, off) = (counted.clone(), counted.clone());
        div()
            .flex()
            .gap(px(8.))
            .child(
                Button::new("on", "Save", HoverHelp::tooltip("Save it"), Role::Button)
                    .tier(Tier::Primary)
                    .on_click(move |_, _, _| on.set((on.get().0 + 1, on.get().1))),
            )
            .child(
                Button::new("off", "Save", HoverHelp::tooltip("Save it"), Role::Button)
                    .tier(Tier::Primary)
                    .disabled(true)
                    .on_click(move |_, _, _| off.set((off.get().0, off.get().1 + 1))),
            )
            .into_any_element()
    });

    click(cx, "on");
    click(cx, "off");
    assert_eq!(
        clicks.get(),
        (1, 0),
        "the enabled button ran once, the disabled one never"
    );
}

/// Enter or Space on a focused button runs it once. GPUI also turns the release of an Enter or Space pressed on a
/// focused element into a click, so a button that ran on the press and ran again on that click would act twice on one
/// key press whenever focus stays on it (a toggle, a step button, a Save whose dialog stays open), sending a second
/// command; and a held Enter that answered one question, repeating onto the next question's focused answer, would answer
/// that one too when released (SPEC.md: "Repeated or composing Enter cannot answer the new question"). In-process tests
/// that send only the press never saw it. Specified: with a button focused, Enter pressed and released runs it once,
/// and so does Space; a held Enter's repeat and its release run nothing; a click still runs it once.
#[gpui::test]
fn enter_or_space_on_a_focused_button_runs_it_once(cx: &mut TestAppContext) {
    let runs = Rc::new(Cell::new(0));
    let focus = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (counted, handle) = (runs.clone(), focus.clone());
    let cx = open(cx, move |_, _| {
        let counted = counted.clone();
        Button::new(
            "step",
            "Step",
            HoverHelp::tooltip("Step once"),
            Role::Button,
        )
        .focus(&handle)
        .on_click(move |_, _, _| counted.set(counted.get() + 1))
        .into_any_element()
    });
    keys(cx, "tab");
    assert!(
        cx.update(|window, _| focus.is_focused(window)),
        "the premise: the button has focus"
    );
    for key in ["enter", "space"] {
        let before = runs.get();
        cx.simulate_event(gpui::KeyDownEvent {
            native_key: None,
            keystroke: gpui::Keystroke::parse(key).unwrap(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(gpui::KeyUpEvent {
            native_key: None,
            keystroke: gpui::Keystroke::parse(key).unwrap(),
        });
        cx.run_until_parked();
        check(cx);
        assert_eq!(
            runs.get(),
            before + 1,
            "{key}, pressed and released, ran it once"
        );
    }
    // A key held down from an earlier press (the Enter that answered the question before this one) repeats onto the
    // button and is released there: neither the repeat nor the release runs it.
    let before = runs.get();
    cx.simulate_event(gpui::KeyDownEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    cx.simulate_event(gpui::KeyUpEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
    check(cx);
    assert_eq!(
        runs.get(),
        before,
        "a held Enter's repeat and release ran nothing"
    );
    let before = runs.get();
    click(cx, "step");
    assert_eq!(runs.get(), before + 1, "a click ran it once");
}

/// A busy button, whose action is in flight, keeps focus and its place in Tab's order but does nothing when pressed:
/// the old app's `aria-disabled` Save. Disabling it instead would drop focus out of the dialog's controls for the
/// length of the save. Specified: Tab reaches the busy button; a click, Enter and Space all leave it focused and run
/// nothing, and Enter and Space stop at it rather than reaching what holds it; once it is no longer busy, a click runs
/// it.
#[gpui::test]
fn a_busy_button_keeps_focus_and_ignores_presses(cx: &mut TestAppContext) {
    let runs = Rc::new(Cell::new(0));
    let busy = Rc::new(Cell::new(true));
    // Enter and Space that went past the button to the element holding it.
    let passed = Rc::new(Cell::new(0));
    let focus = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (counted, drawn, handle, heard) =
        (runs.clone(), busy.clone(), focus.clone(), passed.clone());
    let cx = open(cx, move |_, _| {
        let (counted, heard) = (counted.clone(), heard.clone());
        div()
            .on_key_down(move |event: &gpui::KeyDownEvent, _, _| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    heard.set(heard.get() + 1);
                }
            })
            .child(
                Button::new("save", "Save", HoverHelp::tooltip("Save it"), Role::Button)
                    .tier(Tier::Primary)
                    .busy(drawn.get())
                    .focus(&handle)
                    .on_click(move |_, _, _| counted.set(counted.get() + 1)),
            )
            .into_any_element()
    });
    keys(cx, "tab");
    assert!(
        cx.update(|window, _| focus.is_focused(window)),
        "Tab reached the busy button"
    );
    click(cx, "save");
    keys(cx, "enter");
    keys(cx, "space");
    assert_eq!(runs.get(), 0, "no press ran it while busy");
    assert_eq!(
        passed.get(),
        0,
        "Enter and Space stopped at the busy button"
    );
    assert!(
        cx.update(|window, _| focus.is_focused(window)),
        "and it kept focus"
    );
    busy.set(false);
    redraw(cx);
    click(cx, "save");
    assert_eq!(runs.get(), 1, "once not busy, a click ran it");
}

/// The text of the tooltip showing now, if any.
fn tooltip(cx: &mut VisualTestContext) -> Option<gpui::SharedString> {
    cx.update(|window, cx| layers::shown_tooltip(window, cx))
        .map(|shown| shown.text)
}

/// Wait out the hover delay on the executor's clock, the way a pointer resting on a control does, and check.
fn wait_hover_delay(cx: &mut VisualTestContext) {
    advance(cx, layers::HOVER_DELAY);
}

/// Hovering a control shows its help through the layer manager once the pointer has rested for the old app's hover
/// delay, and leaving it takes the help down. This is the old app's "every control has hover help" rule, now carried
/// by the constructor instead of a page-walking test; the delay is what keeps a pointer crossing the window from
/// flashing a tooltip on every control it passes.
#[gpui::test]
fn hovering_shows_the_controls_help_after_the_hover_delay_and_leaving_hides_it(
    cx: &mut TestAppContext,
) {
    let cx = open(cx, |_, _| {
        Button::new(
            "save",
            "Save",
            HoverHelp::tooltip("Save the title"),
            Role::Button,
        )
        .into_any_element()
    });

    let save = center(cx, "save");
    cx.simulate_mouse_move(save, None, Modifiers::none());
    check(cx);
    assert_eq!(tooltip(cx), None, "nothing shows before the hover delay");
    advance(
        cx,
        layers::HOVER_DELAY - std::time::Duration::from_millis(1),
    );
    assert_eq!(
        tooltip(cx),
        None,
        "still nothing just before the delay ends"
    );
    wait_hover_delay(cx);
    assert_eq!(tooltip(cx), Some("Save the title".into()));

    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    check(cx);
    assert_eq!(tooltip(cx), None, "leaving to empty space hid it");
}

/// Moving straight from one control onto its neighbor shows the neighbor's help at once, whichever hover event GPUI
/// delivers first: a user scanning a row of controls does not wait the hover delay at each (the old app's rule). If the
/// first control's leave could take down any tooltip, the neighbor's help would vanish the moment it appeared, and it
/// would also do so for two controls that merely share an id in different parts of the window.
#[gpui::test]
fn one_controls_leave_never_hides_anothers_tooltip(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        div()
            .flex()
            .child(Button::new(
                "same",
                "Save",
                HoverHelp::tooltip("Save the title"),
                Role::Button,
            ))
            // A second "same" in another part of the window: an id is unique only among siblings, so it may repeat
            // under a different parent. (Under the same element path GPUI would treat the two as one element.)
            .child(
                div()
                    .id("panel")
                    .debug_selector(|| "panel".into())
                    .child(Button::new(
                        "same",
                        "Rename",
                        HoverHelp::tooltip("Give it a new title"),
                        Role::Button,
                    )),
            )
            .into_any_element()
    });
    // Two controls share the selector "same", so locate them through the panel that holds the second: the first sits
    // immediately left of it, with no gap.
    let panel = bounds(cx, "panel");
    let first = point(panel.left() - px(4.), panel.center().y);
    let second = point(panel.left() + px(4.), panel.center().y);
    cx.simulate_mouse_move(first, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx),
        Some("Save the title".into()),
        "the premise: the first control's help shows"
    );

    cx.simulate_mouse_move(second, None, Modifiers::none());
    check(cx);
    assert_eq!(
        tooltip(cx),
        Some("Give it a new title".into()),
        "the neighbor's help shows at once, and the first control's leave did not take it down"
    );
}

/// A tooltip closes when its control is gone, though no hover-leave ever fires for a control that simply stops being
/// drawn (a dialog closing on Enter, a deleted row). SPEC_impl.md: an overlay closes when its subject is gone.
#[gpui::test]
fn a_tooltip_closes_when_its_control_is_no_longer_drawn(cx: &mut TestAppContext) {
    let shown = Rc::new(Cell::new(true));
    let drawn = shown.clone();
    let cx = open(cx, move |_, _| {
        div()
            .size(px(200.))
            .when(drawn.get(), |area| {
                area.child(Button::new(
                    "gone",
                    "Delete",
                    HoverHelp::tooltip("Delete it"),
                    Role::Button,
                ))
            })
            .into_any_element()
    });
    let at = center(cx, "gone");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx),
        Some("Delete it".into()),
        "the premise: its help shows"
    );

    // The harness draws from a plain cell that notifies nothing, so redraw it the way a view's own change would.
    shown.set(false);
    redraw(cx);
    assert!(
        cx.debug_bounds("gone").is_none(),
        "the premise: the control is no longer drawn"
    );
    assert_eq!(tooltip(cx), None, "its tooltip went with it");
}

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

// ---- The text input's events and the dialog's Close ---------------------------------------------------------------

/// How a test makes a field's state: [`TextInputState::new`], [`TextInputState::new_keeping_line_breaks`] or a growing
/// box ([`TextInputState::new_growing`]).
type MakeState = fn(&mut Window, &mut gpui::App) -> TextInputState;

/// A view that owns a text input's state, as an editor does, and records every event the input reports. It is drawn
/// 200 pixels wide under `selector`.
struct Field {
    state: TextInputState,
    events: Rc<RefCell<Vec<TextInputEvent>>>,
    selector: &'static str,
    _events: Subscription,
}

impl Field {
    fn new(
        events: &Rc<RefCell<Vec<TextInputEvent>>>,
        window: &mut Window,
        cx: &mut Context<Field>,
    ) -> Field {
        Field::made(TextInputState::new, "field-box", events, window, cx)
    }

    /// A field whose state `make` makes, drawn under `selector`.
    fn made(
        make: MakeState,
        selector: &'static str,
        events: &Rc<RefCell<Vec<TextInputEvent>>>,
        window: &mut Window,
        cx: &mut Context<Field>,
    ) -> Field {
        let state = make(window, cx);
        let subscription = state.subscribe(window, cx, |field: &mut Field, event, _, _| {
            field.events.borrow_mut().push(event)
        });
        Field {
            state,
            events: events.clone(),
            selector,
            _events: subscription,
        }
    }
}

impl Render for Field {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let selector = self.selector;
        div()
            .debug_selector(move || selector.into())
            .w(px(200.))
            .child(TextInput::new(
                "field",
                &self.state,
                "Title",
                HoverHelp::tooltip("The title"),
            ))
    }
}

/// Why this matters: an editor keeps its draft in step with the field from the field's change events, and saves on
/// Enter; with neither reported, the Rename editor could not mark its draft touched or submit from the keyboard
/// (SPEC_impl.md, "Drafts"). A value the editor itself puts into the field (a rename pushed by another client) must
/// not count as the user's edit, or following a push would mark the draft touched and stop it following.
///
/// Specification: each typed character reports one `Changed`; `set_text` reports nothing, also for a value holding a
/// line break the field strips; Enter reports `Submitted` and nothing else, and leaves the text as it was (Kit's own
/// change report for Enter, which changes nothing, is not passed on).
#[gpui::test]
fn a_text_input_reports_edits_and_enter_and_not_a_value_set_by_its_owner(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let made: Rc<RefCell<Option<gpui::Entity<Field>>>> = Rc::new(RefCell::new(None));
    let (recorded, kept) = (events.clone(), made.clone());
    let cx = open(cx, move |window, cx| {
        let field = kept
            .borrow_mut()
            .get_or_insert_with(|| cx.new(|cx| Field::new(&recorded, window, cx)))
            .clone();
        field.into_any_element()
    });
    click(cx, "field-box");
    keys(cx, "a b");
    assert_eq!(
        *events.borrow(),
        [TextInputEvent::Changed, TextInputEvent::Changed]
    );

    let field = made.borrow().clone().expect("the field was drawn");
    events.borrow_mut().clear();
    cx.update(|window, cx| {
        let state = field.read(cx).state.clone();
        // With a line break, which the single-line field strips: what Enter's no-op edit is compared against below
        // must be the text the field holds, not the text it was given.
        state.set_text("pushed\n", window, cx);
    });
    cx.run_until_parked();
    check(cx);
    assert_eq!(
        *events.borrow(),
        [],
        "a value set by the owner is not an edit"
    );

    keys(cx, "enter");
    assert_eq!(*events.borrow(), [TextInputEvent::Submitted]);
    let text = cx.update(|_, cx| field.read(cx).state.text(cx));
    assert_eq!(
        text.as_ref(),
        "pushed",
        "Enter submitted and inserted nothing"
    );
}

/// A growing box three rows tall, for the tests below.
fn growing_box(window: &mut Window, cx: &mut gpui::App) -> TextInputState {
    TextInputState::new_growing(3, window, cx)
}

/// The growing box holds multi-line text (a launch's first message, a template's prompt), so Enter must type a line
/// break rather than submit, as the old `<textarea>` did, and a tab in the value must stay in it (SPEC.md: "Newline and
/// tab remain editable"); Tab itself moves focus on, as it did out of the old textarea, or a keyboard user could not
/// leave the box. Specified: typing a letter, Enter and a letter gives a two-line value reported as three edits and no
/// submission; a value set with a tab keeps it, and Tab then leaves it unchanged and moves focus to the next field.
#[gpui::test]
fn a_growing_box_takes_line_breaks_keeps_tabs_and_lets_tab_move_on(cx: &mut TestAppContext) {
    let (cx, fields) = open_fields(cx, &[("box", growing_box), ("next", TextInputState::new)]);
    let (grown, next) = (&fields[0], &fields[1]);
    click(cx, "box");
    keys(cx, "a enter b");
    assert_eq!(
        take(cx, grown),
        ("a\nb".into(), vec![TextInputEvent::Changed; 3]),
        "Enter typed a line break, reported as an edit and never as a submission"
    );
    cx.update(|window, cx| {
        let state = grown.read(cx).state.clone();
        state.set_text("one\ttwo", window, cx);
    });
    cx.run_until_parked();
    keys(cx, "tab x");
    assert_eq!(
        take(cx, grown).0,
        "one\ttwo",
        "the tab stayed in the value, and Tab typed nothing into it"
    );
    assert_eq!(take(cx, next).0, "x", "Tab moved focus to the next field");
}

/// SPEC.md's multi-line boxes have no resize corner any more (decided by the maintainer on 2026-11-13): they grow with
/// their text up to a cap and then scroll, so a long prompt neither hides behind a fixed box nor pushes a dialog's
/// answers off the window. Specified: the box starts at its rows; five lines make it taller; it is still growing one
/// line short of the cap and stops at the cap's twelve lines, and thirty lines leave it at that height. (Line breaks
/// set the rows here, not wrapping, so the test context's stand-in text system measures the same as the real one; the
/// golden shows real wrapping.)
#[gpui::test]
fn a_growing_box_grows_with_its_lines_up_to_its_cap(cx: &mut TestAppContext) {
    let (cx, fields) = open_fields(cx, &[("box", growing_box)]);
    let grown = fields[0].clone();
    let mut height_with = |lines: usize| {
        let text: Vec<String> = (1..=lines).map(|n| format!("line {n}")).collect();
        cx.update(|window, cx| {
            let state = grown.read(cx).state.clone();
            state.set_text(text.join("\n"), window, cx);
        });
        redraw(cx);
        bounds(cx, "box").size.height
    };
    let max = super::text_input::GROWING_MAX_ROWS;
    let (one, three, five) = (height_with(1), height_with(3), height_with(5));
    let (below, cap, past) = (height_with(max - 1), height_with(max), height_with(30));
    assert_eq!(one, three, "the box starts at its three rows");
    assert!(
        five > three,
        "five lines made it taller: {three:?} then {five:?}"
    );
    assert!(
        cap > below,
        "it grew up to the cap's {max} lines: {below:?} then {cap:?}"
    );
    assert_eq!(past, cap, "past the cap it stays as tall as at the cap");
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

// ---- The field that keeps line breaks -------------------------------------------------------------------------------

/// Paste, as the platform's shortcut does in a text field.
const PASTE: &str = if cfg!(target_os = "macos") {
    "cmd-v"
} else {
    "ctrl-v"
};

/// Select the whole text, as the platform's shortcut does in a text field.
const SELECT_ALL: &str = if cfg!(target_os = "macos") {
    "cmd-a"
} else {
    "ctrl-a"
};

/// Enter with the platform modifier, which Kit binds to its secondary Enter.
const PLATFORM_ENTER: &str = if cfg!(target_os = "macos") {
    "cmd-enter"
} else {
    "ctrl-enter"
};

/// Open a window drawing one [`Field`] per `(selector, make)` pair, in a column, and hand back the fields once drawn.
fn open_fields<'a>(
    cx: &'a mut TestAppContext,
    fields: &'static [(&'static str, MakeState)],
) -> (&'a mut VisualTestContext, Vec<gpui::Entity<Field>>) {
    let made: Rc<RefCell<Vec<gpui::Entity<Field>>>> = Rc::new(RefCell::new(Vec::new()));
    let kept = made.clone();
    let cx = open(cx, move |window, cx| {
        let mut kept = kept.borrow_mut();
        if kept.is_empty() {
            for (selector, make) in fields {
                let events = Rc::new(RefCell::new(Vec::new()));
                kept.push(cx.new(|cx| Field::made(*make, selector, &events, window, cx)));
            }
        }
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(kept.iter().cloned())
            .into_any_element()
    });
    let fields = made.borrow().clone();
    (cx, fields)
}

/// The text a field holds and the events it reported since the last call, which clears them.
fn take(cx: &mut VisualTestContext, field: &gpui::Entity<Field>) -> (String, Vec<TextInputEvent>) {
    cx.update(|_, cx| {
        let field = field.read(cx);
        let events = field.events.borrow_mut().drain(..).collect();
        (field.state.text(cx).to_string(), events)
    })
}

/// Why this matters: SPEC.md refuses a supplied session title that holds a newline, and the refusal belongs to the
/// supervisor. A field that quietly dropped a pasted line break would save "fixbug" for a pasted "fix\nbug": a title
/// the user never typed, saved without a word, which is what the old Rename field's textarea existed to prevent. The
/// old field also stayed one row tall on purpose, rather than growing to show the pasted lines.
///
/// Specification: a paste of "fix\nbug" into the field from `new_keeping_line_breaks` leaves exactly that text and
/// reports one `Changed`, and the field stays as tall as before; the same paste into the single-line field leaves
/// "fixbug" (the stripping this variant exists to avoid). `set_text` with a line break keeps it and reports nothing.
#[gpui::test]
fn a_pasted_line_break_is_kept_in_a_field_that_stays_one_row(cx: &mut TestAppContext) {
    let (cx, fields) = open_fields(
        cx,
        &[
            ("kept", TextInputState::new_keeping_line_breaks),
            ("stripped", TextInputState::new),
        ],
    );
    let (kept, stripped) = (&fields[0], &fields[1]);
    let one_row = bounds(cx, "kept").size.height;
    cx.update(|_, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string("fix\nbug".into())));

    click(cx, "kept");
    keys(cx, PASTE);
    assert_eq!(
        take(cx, kept),
        ("fix\nbug".into(), vec![TextInputEvent::Changed]),
        "the paste kept its line break and was reported once"
    );
    assert_eq!(
        bounds(cx, "kept").size.height,
        one_row,
        "the field stayed one row"
    );

    click(cx, "stripped");
    keys(cx, PASTE);
    assert_eq!(
        take(cx, stripped).0,
        "fixbug",
        "the single-line field strips the same paste"
    );

    cx.update(|window, cx| {
        let state = kept.read(cx).state.clone();
        state.set_text("pushed\nvalue", window, cx);
    });
    cx.run_until_parked();
    check(cx);
    assert_eq!(
        take(cx, kept),
        ("pushed\nvalue".into(), vec![]),
        "a value set by the owner keeps its line break and is not an edit"
    );
}

/// Why this matters: the field keeps line breaks only so that a paste reaches the supervisor intact; typing must not
/// be able to make one, or the Rename editor would turn a user's Enter (meant to save) into a title the supervisor
/// refuses. Kit's text area breaks the line on Shift+Enter even when Enter submits, and GPUI types Enter's character
/// into the field after Kit submits, so both had to be taken away.
///
/// Specification: in the field from `new_keeping_line_breaks` holding typed text, Enter, Shift+Enter and the platform
/// modifier's Enter each report `Submitted` and nothing else and leave the text as it was, also with the whole text
/// selected; Tab moves focus on to the next field rather than typing a tab.
#[gpui::test]
fn typing_enter_in_the_field_that_keeps_line_breaks_submits_and_inserts_nothing(
    cx: &mut TestAppContext,
) {
    let (cx, fields) = open_fields(
        cx,
        &[
            ("kept", TextInputState::new_keeping_line_breaks),
            ("next", TextInputState::new),
        ],
    );
    let kept = &fields[0];
    click(cx, "kept");
    keys(cx, "a b");
    assert_eq!(
        take(cx, kept),
        (
            "ab".into(),
            vec![TextInputEvent::Changed, TextInputEvent::Changed]
        ),
        "the premise: typing reaches the field"
    );

    for chord in ["enter", "shift-enter", PLATFORM_ENTER] {
        keys(cx, chord);
        assert_eq!(
            take(cx, kept),
            ("ab".into(), vec![TextInputEvent::Submitted]),
            "{chord} submitted and inserted nothing"
        );
        keys(cx, SELECT_ALL);
        keys(cx, chord);
        assert_eq!(
            take(cx, kept),
            ("ab".into(), vec![TextInputEvent::Submitted]),
            "{chord} with the text selected submitted and replaced nothing"
        );
        keys(cx, "end");
    }

    keys(cx, "tab x");
    assert_eq!(take(cx, kept).0, "ab", "Tab typed nothing into the field");
    assert_eq!(
        take(cx, &fields[1]).0,
        "x",
        "Tab moved focus on to the next field"
    );
}

// ---- Fields that drive a list: the arrows handed over, and the rows that never take focus --------------------------

/// Why this matters: GPUI Kit's single-line field takes Up and Down for caret moves it then does nothing with, so
/// without the hand-over a search field could never move a selection in the list beside it (the quick switcher's
/// arrows). Taking the keys must not cost the field its text or its focus.
///
/// Specification: in a field drawn with `on_vertical`, after typing `ab`, Down, Up and Down reach the handler once
/// each, in order, and the field still holds `ab` and focus.
#[gpui::test]
fn up_and_down_reach_the_fields_owner_and_leave_the_field_alone(cx: &mut TestAppContext) {
    let arrows = Rc::new(RefCell::new(Vec::new()));
    let made: Rc<RefCell<Option<TextInputState>>> = Rc::new(RefCell::new(None));
    let (recorded, kept) = (arrows.clone(), made.clone());
    let cx = open(cx, move |window, cx| {
        let state = kept
            .borrow_mut()
            .get_or_insert_with(|| TextInputState::new(window, cx))
            .clone();
        let recorded = recorded.clone();
        div()
            .debug_selector(|| "field-box".into())
            .w(px(200.))
            .child(
                TextInput::new(
                    "field",
                    &state,
                    "Search",
                    HoverHelp::tooltip("A test field"),
                )
                .on_vertical(move |direction, _, _| recorded.borrow_mut().push(direction)),
            )
            .into_any_element()
    });
    click(cx, "field-box");
    keys(cx, "a b down up down");
    assert_eq!(
        *arrows.borrow(),
        [Vertical::Down, Vertical::Up, Vertical::Down]
    );
    let state = made.borrow().clone().expect("the field was drawn");
    assert_eq!(cx.update(|_, cx| state.text(cx)).as_ref(), "ab");
    assert!(
        cx.update(|window, cx| state.focus_handle(cx).is_focused(window)),
        "the field kept focus"
    );
}

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

// ---- The checkbox and the switch ----------------------------------------------------------------------------------

/// Open a window drawing one checkbox under `resume` whose checked state lives in `checked`, toggled by its handler,
/// inside a parent that counts the Enter presses reaching it, with the switch look when `switch` and unavailable while
/// `disabled` holds. Hands back the Enter count.
fn open_checkbox<'a>(
    cx: &'a mut TestAppContext,
    checked: &Rc<Cell<bool>>,
    disabled: &Rc<Cell<bool>>,
    switch: bool,
) -> (&'a mut VisualTestContext, Rc<Cell<usize>>) {
    let enters = Rc::new(Cell::new(0));
    let (state, off, heard) = (checked.clone(), disabled.clone(), enters.clone());
    let cx = open(cx, move |_, _| {
        let (state, heard) = (state.clone(), heard.clone());
        let role = if switch { Role::Switch } else { Role::CheckBox };
        let checkbox = Checkbox::new("resume", "Resume", HoverHelp::tooltip("Resume it"), role)
            .checked(state.get())
            .disabled(off.get())
            .on_toggle(move |next, _, _| state.set(next));
        let checkbox = if switch { checkbox.switch() } else { checkbox };
        div()
            .on_key_down(move |event: &gpui::KeyDownEvent, _, _| {
                if event.keystroke.key == "enter" {
                    heard.set(heard.get() + 1);
                }
            })
            .child(checkbox)
            .into_any_element()
    });
    (cx, enters)
}

/// The old app's checkboxes and Settings switches were native checkboxes, and a setting that toggled on Enter, or did
/// not toggle on a click on its words, would surprise anyone who used the old one; an unavailable one must not change
/// a setting at all. Specified, for both looks: a click on the control and a click on its words each toggle it,
/// handing the owner the new value; with focus on it, a press of Space toggles it once (its release and a held repeat
/// do not toggle it again) and Enter, pressed and released, does not (it goes on to what holds it); while unavailable,
/// neither a click nor Space toggles it.
#[gpui::test]
fn a_checkbox_toggles_on_a_click_or_space_and_not_on_enter(cx: &mut TestAppContext) {
    for switch in [false, true] {
        let look = if switch { "switch" } else { "checkbox" };
        let checked = Rc::new(Cell::new(false));
        let disabled = Rc::new(Cell::new(false));
        let (cx, enters) = open_checkbox(cx, &checked, &disabled, switch);
        click(cx, "resume");
        assert!(checked.get(), "{look}: a click turned it on");
        let words = bounds(cx, "resume");
        let near_end = if switch {
            words.left() + px(4.)
        } else {
            words.right() - px(4.)
        };
        cx.simulate_click(gpui::point(near_end, words.center().y), Modifiers::none());
        check(cx);
        assert!(!checked.get(), "{look}: a click on its words turned it off");

        // The tests run under Linux's rule, where a click focuses a control: the clicks above left focus on it.
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("resume"),
            "{look}: the premise: it has focus"
        );
        press(cx, "space", false);
        assert!(checked.get(), "{look}: Space turned it on");
        // Draw the new state before the release, so a toggle on the release would turn it off again rather than set
        // the value it already has.
        redraw(cx);
        release(cx, "space");
        assert!(
            checked.get(),
            "{look}: releasing Space did not toggle it back"
        );
        press(cx, "space", true);
        assert!(
            checked.get(),
            "{look}: a held Space did not toggle it again"
        );
        release(cx, "space");
        press(cx, "enter", false);
        release(cx, "enter");
        assert!(
            checked.get(),
            "{look}: Enter, pressed and released, did not toggle it"
        );
        assert_eq!(enters.get(), 1, "{look}: Enter went on to what holds it");

        disabled.set(true);
        redraw(cx);
        click(cx, "resume");
        press(cx, "space", false);
        release(cx, "space");
        assert!(
            checked.get(),
            "{look}: unavailable, neither a click nor Space toggled it"
        );
    }
}

/// Press `chord` (with `held`, as an auto-repeat of a key already down), as the platform reports a key-down.
fn press(cx: &mut VisualTestContext, chord: &'static str, held: bool) {
    cx.simulate_event(gpui::KeyDownEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse(chord).unwrap(),
        is_held: held,
        prefer_character_input: false,
    });
    cx.run_until_parked();
    check(cx);
}

/// Release `chord`. GPUI turns the release of an Enter or Space pressed on a focused element into a click, which only a
/// test that sends the release can see.
fn release(cx: &mut VisualTestContext, chord: &'static str) {
    cx.simulate_event(gpui::KeyUpEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse(chord).unwrap(),
    });
    cx.run_until_parked();
    check(cx);
}

/// On macOS with Keyboard navigation off, Tab reaches text fields and pop-ups only, and WebKit treated a checkbox as a
/// button there, so Tab skipped it (SPEC.md, "Keyboard focus"); with every control reached, as on Linux, Tab lands on it.
/// A checkbox given the pop-up rule instead would be a Tab stop a Mac user never had. Specified: with Tab reaching text
/// fields and pop-ups only, Tab from nothing focused does not focus the checkbox; with every control reached, it does.
#[gpui::test]
fn tab_reaches_a_checkbox_only_where_it_reaches_every_control(cx: &mut TestAppContext) {
    let checked = Rc::new(Cell::new(false));
    let disabled = Rc::new(Cell::new(false));
    let (cx, _) = open_checkbox(cx, &checked, &disabled, false);
    let reach = Rc::new(Cell::new(crate::focus::TabReach::TextFieldsAndPopups));
    crate::test_support::follow(cx, &reach);
    assert_eq!(focused_id(cx), None, "the premise: nothing has focus yet");
    keys(cx, "tab");
    assert_ne!(focused_id(cx).as_deref(), Some("resume"), "Tab skipped it");
    reach.set(crate::focus::TabReach::EveryControl);
    keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("resume"),
        "with every control reached, Tab focused it"
    );
}

/// Assistive technology and the test bridge read a checkbox's state from its role and its checked state; a control
/// drawn checked but reported unchecked would tell a screen reader the opposite of the screen. Specified: the checkbox
/// reports the check box role and the switch the switch role, each with its label as its name, and each reports
/// checked or not checked as drawn.
#[gpui::test]
fn a_checkbox_reports_its_role_and_whether_it_is_checked(cx: &mut TestAppContext) {
    for (switch, role) in [(false, Role::CheckBox), (true, Role::Switch)] {
        let checked = Rc::new(Cell::new(false));
        let disabled = Rc::new(Cell::new(false));
        let (cx, _) = open_checkbox(cx, &checked, &disabled, switch);
        for on in [false, true] {
            checked.set(on);
            let node = frame(cx)
                .into_iter()
                .find(|node| node.element_id.as_deref() == Some("resume"))
                .map(|node| (node.role, node.label, node.toggled));
            let toggled = if on {
                gpui::Toggled::True
            } else {
                gpui::Toggled::False
            };
            assert_eq!(
                node,
                Some((Some(role), Some("Resume".into()), Some(toggled))),
                "switch: {switch}, checked: {on}"
            );
        }
    }
}

/// A button that is one of a set of choices (the host rail's entries) must tell assistive technology which choice is
/// in force, as the old `aria-pressed` did, and a button showing content in place of its words (a host's mark) must
/// still be named by its label, since a mark says nothing to a screen reader. Specified: a pressed button reports the
/// toggled state on, one not pressed reports it off, and a plain button reports none; the pressed one, given content,
/// is named by its label and paints its content's text, never the label's.
#[gpui::test]
fn a_pressed_button_reports_its_state_and_content_keeps_the_label_as_its_name(
    cx: &mut TestAppContext,
) {
    let cx = open(cx, |_, _| {
        div()
            .flex()
            .gap(px(8.))
            .child(
                Button::new(
                    "on",
                    "All hosts",
                    HoverHelp::tooltip("Show every host"),
                    Role::Button,
                )
                .tier(Tier::Rail)
                .pressed(true)
                .content(div().child("All")),
            )
            .child(
                Button::new(
                    "off",
                    "Local",
                    HoverHelp::tooltip("Show one host"),
                    Role::Button,
                )
                .tier(Tier::Rail)
                .pressed(false),
            )
            .child(Button::new(
                "plain",
                "Save",
                HoverHelp::tooltip("Save it"),
                Role::Button,
            ))
            .into_any_element()
    });
    let nodes = frame(cx);
    let find = |id: &str| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("{id} was drawn"))
    };
    assert_eq!(find("on").toggled, Some(gpui::Toggled::True));
    assert_eq!(find("off").toggled, Some(gpui::Toggled::False));
    assert_eq!(find("plain").toggled, None);
    assert_eq!(find("on").label.as_deref(), Some("All hosts"));
    let painted: Vec<String> = nodes
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect();
    assert!(painted.iter().any(|text| text == "All"), "{painted:?}");
    assert!(
        !painted.iter().any(|text| text == "All hosts"),
        "{painted:?}"
    );
}

/// A status mark is information, not a control: the old app's `role="status"` span with its label and tooltip. A user
/// who sees only its symbol needs its words on hover, a screen reader needs them as its name, and a keyboard user must
/// not stop on it. Specified: a status mark showing "?" is drawn with the status role and its label as its name, paints
/// "?", takes no focus from Tab (which goes on to the button after it) and has no click handler, and resting the
/// pointer on it shows its hover help.
#[gpui::test]
fn a_status_mark_is_named_and_helps_on_hover_but_is_no_tab_stop(cx: &mut TestAppContext) {
    let after = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let focus = after.clone();
    let cx = open(cx, move |_, _| {
        div()
            .flex()
            .gap(px(8.))
            .child(super::StatusMark::new(
                "unloaded",
                "The list could not be loaded",
                HoverHelp::tooltip("The list could not be loaded"),
                Role::Status,
                "?",
            ))
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
    keys(cx, "tab");
    assert!(
        cx.update(|window, _| after.is_focused(window)),
        "Tab went past the mark to the button"
    );
    let nodes = frame(cx);
    let mark = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("unloaded"))
        .expect("the mark was drawn");
    assert_eq!(mark.role, Some(Role::Status));
    assert_eq!(mark.label.as_deref(), Some("The list could not be loaded"));
    assert!(!mark.flags.focusable && !mark.flags.click, "not a control");
    assert!(
        nodes
            .iter()
            .any(|node| node.texts.iter().any(|text| text.text.as_ref() == "?")),
        "it paints its text"
    );

    let at = center(cx, "unloaded");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(tooltip(cx).as_deref(), Some("The list could not be loaded"));
}

// ---- The scrollbar --------------------------------------------------------------------------------------------------

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

// ---- The radio group and the segmented control ------------------------------------------------------------------

/// What the owner of a test's radio group sees: the value chosen now (its state, which the group's choices set), how
/// many choices it heard, how often its main action ran and with which value chosen at the time, and how many Enter
/// presses went past the group to the element holding it. Setting `disabled` makes the whole group unavailable at the
/// next redraw.
#[derive(Clone, Default)]
struct Owner {
    chosen: Rc<Cell<Option<&'static str>>>,
    choices: Rc<Cell<usize>>,
    launched_with: Rc<RefCell<Vec<Option<&'static str>>>>,
    enters_passed: Rc<Cell<usize>>,
    disabled: Rc<Cell<bool>>,
}

/// How a test draws its group: its role, its options as (label, unavailable), each option's value being its label,
/// what starts chosen, its look (`look`, applied to the group as built: the segments it starts with, plain radios or
/// chips; one field, so a spec cannot ask for two), whether it is given the owner's main action as its Enter, and what
/// else each option is given (`dress`, called with each option's label and the option as built from it).
#[derive(Clone, Copy)]
struct GroupSpec {
    role: Role,
    options: &'static [(&'static str, bool)],
    chosen: Option<&'static str>,
    look: fn(RadioGroup<&'static str>) -> RadioGroup<&'static str>,
    with_enter: bool,
    dress: fn(&'static str, RadioOption<&'static str>) -> RadioOption<&'static str>,
}

impl GroupSpec {
    /// A radio group (native radios) of `options` with nothing more set.
    fn radio(options: &'static [(&'static str, bool)], chosen: Option<&'static str>) -> Self {
        GroupSpec {
            role: Role::RadioGroup,
            options,
            chosen,
            look: |group| group,
            with_enter: false,
            dress: |_, option| option,
        }
    }
}

/// Open a window drawing, in a row, a button `before`, the group `choice` (labelled "Theme", with the help "Pick a
/// theme"; option `X`'s id is `choice-X` and its help "Use X"), and a button `after`, inside a parent that counts the
/// Enter presses reaching it. Hands back what its owner sees.
fn open_group(cx: &mut TestAppContext, spec: GroupSpec) -> (&mut VisualTestContext, Owner) {
    let owner = Owner::default();
    owner.chosen.set(spec.chosen);
    let kept = owner.clone();
    let cx = open(cx, move |_, _| {
        let (chose, launched, heard) = (kept.clone(), kept.clone(), kept.enters_passed.clone());
        let mut group = RadioGroup::new(
            "choice",
            "Theme",
            HoverHelp::tooltip("Pick a theme"),
            spec.role,
        )
        .chosen(kept.chosen.get())
        .disabled(kept.disabled.get())
        .on_change(move |value, _, _| {
            chose.chosen.set(Some(*value));
            chose.choices.set(chose.choices.get() + 1);
        });
        for &(label, unavailable) in spec.options {
            let help = HoverHelp::tooltip(format!("Use {label}"));
            let option = RadioOption::new(label, help, label).disabled(unavailable);
            group = group.option((spec.dress)(label, option));
        }
        group = (spec.look)(group);
        if spec.with_enter {
            group = group.on_enter(move |_, _| {
                launched
                    .launched_with
                    .borrow_mut()
                    .push(launched.chosen.get());
            });
        }
        let help = || HoverHelp::tooltip("A test control");
        div()
            .on_key_down(move |event: &gpui::KeyDownEvent, _, _| {
                if event.keystroke.key == "enter" {
                    heard.set(heard.get() + 1);
                }
            })
            .flex()
            .items_center()
            .gap(px(8.))
            .child(Button::new("before", "Before", help(), Role::Button))
            .child(group)
            .child(Button::new("after", "After", help(), Role::Button))
            .into_any_element()
    });
    (cx, owner)
}

/// Press `keystrokes` and draw the owner's new state, as an owner that notifies on a change would.
fn press_and_draw(cx: &mut VisualTestContext, keystrokes: &str) {
    keys(cx, keystrokes);
    redraw(cx);
}

/// Native radios move the choice and focus together with the arrows, and the old app's radio groups (Appearance, the
/// session status filter) relied on that; WebKit's radios wrap at the ends and skip unavailable options. A group that
/// stopped at an end, landed on an unavailable option, or moved the choice without focus would leave a keyboard user
/// somewhere the screen does not show. Specified, from the chosen A in A, unavailable B, C, D: Down chooses and focuses
/// C, Right D, Down wraps to A, Up wraps to D and Left gives C; Shift+Down changes nothing.
#[gpui::test]
fn the_arrows_move_a_radio_groups_choice_and_focus_wrapping_past_unavailable_options(
    cx: &mut TestAppContext,
) {
    let options = &[("A", false), ("B", true), ("C", false), ("D", false)];
    let (cx, owner) = open_group(cx, GroupSpec::radio(options, Some("A")));
    press_and_draw(cx, "tab");
    press_and_draw(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("choice-A"),
        "the premise: Tab entered at the chosen A"
    );
    for (key, expected) in [
        ("down", "C"),
        ("right", "D"),
        ("down", "A"),
        ("up", "D"),
        ("left", "C"),
    ] {
        press_and_draw(cx, key);
        assert_eq!(owner.chosen.get(), Some(expected), "{key} chose {expected}");
        let id = format!("choice-{expected}");
        assert_eq!(
            focused_id(cx).as_deref(),
            Some(id.as_str()),
            "{key} took focus to {expected}"
        );
    }
    press_and_draw(cx, "shift-down");
    assert_eq!(
        owner.chosen.get(),
        Some("C"),
        "a modified arrow changed nothing"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("choice-C"));
}

/// A native radio group is one tab stop, so Tab passes a group of many choices in one press, and it enters where the
/// choice is, so the first key a user presses there acts from the chosen option. A group that made every option a stop
/// would make every form with a radio group slower to cross; one that entered at the first option would hide where the
/// choice is. Specified: with B chosen among A, B and C, Tab from the button before enters at B and the next Tab
/// reaches the button after, and Shift+Tab back enters at B and leaves for the button before; with nothing chosen,
/// Tab enters at A and Shift+Tab at C (WebKit's rule), each leaving in one more press.
#[gpui::test]
fn tab_enters_a_radio_group_at_its_choice_and_leaves_it_in_one_press(cx: &mut TestAppContext) {
    let options = &[("A", false), ("B", false), ("C", false)];
    for (chosen, forward, backward) in [
        (Some("B"), "choice-B", "choice-B"),
        (None, "choice-A", "choice-C"),
    ] {
        let (cx, _) = open_group(cx, GroupSpec::radio(options, chosen));
        let mut expect = |keystrokes: &str, id: &str| {
            press_and_draw(cx, keystrokes);
            assert_eq!(
                focused_id(cx).as_deref(),
                Some(id),
                "chosen {chosen:?}: {keystrokes} reached {id}"
            );
        };
        expect("tab", "before");
        expect("tab", forward);
        expect("tab", "after");
        expect("shift-tab", backward);
        expect("shift-tab", "before");
    }
}

/// Space chooses a focused native radio, while Enter does not: in a form, Enter on a radio goes on to the form, and the
/// old launcher made it launch with the values shown, unchanged (SPEC.md: "Enter on ... YOLO radios ... launches with
/// the values shown"). A radio that chose on Enter would change a required answer the user had not given. Specified,
/// with nothing chosen and focus on A: without the owner's action, Enter (its press and its release, which GPUI turns
/// into a click on the focused element) changes nothing and reaches what holds the group, and Space chooses A,
/// reported once, and again reports nothing; with the owner's action, Enter runs it with nothing chosen and does not
/// reach what holds the group, and a held Enter does nothing at all.
#[gpui::test]
fn space_chooses_a_focused_radio_and_enter_goes_on_to_the_owner(cx: &mut TestAppContext) {
    let options = &[("A", false), ("B", false)];
    {
        let (cx, owner) = open_group(cx, GroupSpec::radio(options, None));
        press_and_draw(cx, "tab");
        press_and_draw(cx, "tab");
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("choice-A"),
            "the premise: focus is on A"
        );
        press_and_draw(cx, "enter");
        key_up(cx, "enter");
        redraw(cx);
        assert_eq!(
            owner.chosen.get(),
            None,
            "Enter, pressed and released, chose nothing"
        );
        assert_eq!(
            owner.enters_passed.get(),
            1,
            "Enter went on to what holds the group"
        );
        press_and_draw(cx, "space");
        assert_eq!(owner.chosen.get(), Some("A"), "Space chose A");
        press_and_draw(cx, "space");
        assert_eq!(
            owner.choices.get(),
            1,
            "choosing the chosen radio again reported nothing"
        );
    }

    let spec = GroupSpec {
        with_enter: true,
        ..GroupSpec::radio(options, None)
    };
    let (cx, owner) = open_group(cx, spec);
    press_and_draw(cx, "tab");
    press_and_draw(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("choice-A"),
        "the premise: focus is on A"
    );
    press_and_draw(cx, "enter");
    assert_eq!(
        *owner.launched_with.borrow(),
        vec![None],
        "Enter ran the owner's action, nothing chosen"
    );
    assert_eq!(owner.enters_passed.get(), 0, "and stopped at the group");
    held_key(cx, "enter");
    assert_eq!(
        owner.launched_with.borrow().len(),
        1,
        "a held Enter did not run it again"
    );
    assert_eq!(
        owner.enters_passed.get(),
        0,
        "nor reached what holds the group"
    );
}

/// Press `chord` as an auto-repeat of a key already down, and check.
fn held_key(cx: &mut VisualTestContext, chord: &'static str) {
    cx.simulate_event(gpui::KeyDownEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse(chord).unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    cx.run_until_parked();
    check(cx);
}

/// Release `chord`, which GPUI turns into a click on the focused element when it released an Enter or Space pressed
/// there, and check.
fn key_up(cx: &mut VisualTestContext, chord: &'static str) {
    cx.simulate_event(gpui::KeyUpEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse(chord).unwrap(),
    });
    cx.run_until_parked();
    check(cx);
}

/// The old launcher's segmented choices (effort, permission mode, destination type) were buttons with `aria-pressed`:
/// each a tab stop, chosen with Enter or Space, and Enter on one applied it and then launched. Giving them the radio
/// keyboard would change how a user steps through the launcher. Specified, in a group of A (chosen), B and C: Tab
/// reaches A, then B; Enter chooses B; Tab reaches C and Space chooses it; Down neither chooses nor moves; Enter on the
/// chosen C reports it again. With the owner's action, Enter on B chooses B and then runs the action with B chosen.
#[gpui::test]
fn a_button_group_makes_each_option_a_tab_stop_chosen_with_enter_or_space(cx: &mut TestAppContext) {
    let options = &[("A", false), ("B", false), ("C", false)];
    let buttons = GroupSpec {
        role: Role::Group,
        ..GroupSpec::radio(options, Some("A"))
    };
    {
        let (cx, owner) = open_group(cx, buttons);
        for id in ["before", "choice-A", "choice-B"] {
            press_and_draw(cx, "tab");
            assert_eq!(focused_id(cx).as_deref(), Some(id), "Tab reached {id}");
        }
        press_and_draw(cx, "enter");
        assert_eq!(owner.chosen.get(), Some("B"), "Enter chose B");
        press_and_draw(cx, "tab");
        press_and_draw(cx, "space");
        assert_eq!(owner.chosen.get(), Some("C"), "Space chose C");
        press_and_draw(cx, "down");
        assert_eq!(owner.chosen.get(), Some("C"), "Down chose nothing");
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("choice-C"),
            "and moved nothing"
        );
        press_and_draw(cx, "enter");
        assert_eq!(
            owner.choices.get(),
            3,
            "Enter on the chosen C reported it again"
        );
    }

    let (cx, owner) = open_group(
        cx,
        GroupSpec {
            with_enter: true,
            ..buttons
        },
    );
    for _ in 0..3 {
        press_and_draw(cx, "tab");
    }
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("choice-B"),
        "the premise: focus is on B"
    );
    press_and_draw(cx, "enter");
    assert_eq!(
        *owner.launched_with.borrow(),
        vec![Some("B")],
        "Enter chose B, then ran the action"
    );
}

/// An unavailable segment of a button group was a disabled `<button>` in the old app, which Tab never reached; a
/// segment that stayed a tab stop would give a keyboard user a stop where nothing can be done. Specified, in a button
/// group of A, unavailable B and C: Tab from the button before reaches A, then C, then the button after.
#[gpui::test]
fn tab_skips_an_unavailable_option_in_a_button_group(cx: &mut TestAppContext) {
    let spec = GroupSpec {
        role: Role::Group,
        ..GroupSpec::radio(&[("A", false), ("B", true), ("C", false)], Some("A"))
    };
    let (cx, _) = open_group(cx, spec);
    for id in ["before", "choice-A", "choice-C", "after"] {
        press_and_draw(cx, "tab");
        assert_eq!(focused_id(cx).as_deref(), Some(id), "Tab reached {id}");
    }
}

/// A click chooses, as on the old segments and radios, and an unavailable option or group must not change a setting
/// the user was shown they cannot change. Specified, with A chosen among A, unavailable B and C: a click on C chooses
/// it and a click on B changes nothing; with the whole group unavailable, a click on A changes nothing and Tab goes
/// from the button before straight to the button after.
#[gpui::test]
fn a_click_chooses_and_unavailable_options_and_groups_ignore_it(cx: &mut TestAppContext) {
    let options = &[("A", false), ("B", true), ("C", false)];
    let (cx, owner) = open_group(cx, GroupSpec::radio(options, Some("A")));
    click(cx, "choice-C");
    assert_eq!(owner.chosen.get(), Some("C"), "a click chose C");
    click(cx, "choice-B");
    assert_eq!(
        owner.chosen.get(),
        Some("C"),
        "a click on the unavailable B did nothing"
    );
    owner.disabled.set(true);
    redraw(cx);
    click(cx, "choice-A");
    assert_eq!(
        owner.chosen.get(),
        Some("C"),
        "a click in the unavailable group did nothing"
    );
    click(cx, "before");
    if focused_id(cx).as_deref() != Some("before") {
        // Where a click does not focus a control (macOS), Tab from nothing reaches the first button.
        press_and_draw(cx, "tab");
    }
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("before"),
        "the premise: the button before has focus"
    );
    press_and_draw(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("after"),
        "Tab passed the unavailable group"
    );
}

/// Assistive technology and the test bridge tell native radios from pressed buttons by role, and read the choice from
/// each option's checked or pressed state; a choice drawn but not reported would leave a screen reader saying nothing
/// is chosen. Specified, with B chosen among A, B and C: a radio group reports the radio group role and its label, and
/// options with the radio button role, their labels, and B alone checked; a button group reports the group role, and
/// buttons with B alone pressed.
#[gpui::test]
fn a_radio_group_reports_its_role_and_which_option_is_chosen(cx: &mut TestAppContext) {
    let options = &[("A", false), ("B", false), ("C", false)];
    for (role, option_role) in [
        (Role::RadioGroup, Role::RadioButton),
        (Role::Group, Role::Button),
    ] {
        let spec = GroupSpec {
            role,
            ..GroupSpec::radio(options, Some("B"))
        };
        let (cx, _) = open_group(cx, spec);
        let nodes = frame(cx);
        let node = |id: &str| {
            nodes
                .iter()
                .find(|node| node.element_id.as_deref() == Some(id))
                .map(|node| (node.role, node.label.clone(), node.toggled))
        };
        assert_eq!(
            node("choice"),
            Some((Some(role), Some("Theme".into()), None))
        );
        for (label, on) in [("A", false), ("B", true), ("C", false)] {
            let toggled = if on {
                gpui::Toggled::True
            } else {
                gpui::Toggled::False
            };
            assert_eq!(
                node(&format!("choice-{label}")),
                Some((Some(option_role), Some(label.into()), Some(toggled))),
                "{role:?}: option {label}"
            );
        }
    }
}

/// Every option has help of its own, and so does the group; the group's must never cover an option's, because GPUI
/// tells the group about a pointer move after its options, so help wired to the group itself would replace the
/// option's on every move straight onto it. Specified: a pointer moving from empty space straight onto B shows B's help
/// after the hover delay; resting on the group's edge, outside every option, shows the group's; moving on from there
/// onto A shows A's at once.
#[gpui::test]
fn an_options_help_wins_over_the_groups_which_shows_at_its_edge(cx: &mut TestAppContext) {
    let options = &[("A", false), ("B", false)];
    let (cx, _) = open_group(cx, GroupSpec::radio(options, Some("A")));
    let on_b = center(cx, "choice-B");
    cx.simulate_mouse_move(on_b, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx),
        Some("Use B".into()),
        "B's own help shows over B"
    );
    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    check(cx);
    let group = bounds(cx, "choice");
    // Inside the well's 2-pixel padding, past its 1-pixel edge and short of the first option.
    cx.simulate_mouse_move(
        point(group.left() + px(2.), group.center().y),
        None,
        Modifiers::none(),
    );
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx),
        Some("Pick a theme".into()),
        "the group's help shows on its edge"
    );
    let on_a = center(cx, "choice-A");
    cx.simulate_mouse_move(on_a, None, Modifiers::none());
    check(cx);
    assert_eq!(
        tooltip(cx),
        Some("Use A".into()),
        "A's help replaced it at once"
    );
}

/// Plain radio circles promise the radio keyboard; drawn over a button group, they would take Tab at every option and
/// ignore the arrows while looking like radios. Specified: asking for the plain look on a group with the button role
/// fails in debug builds, which the tests run in.
#[test]
#[should_panic(expected = "plain radios behave as radios")]
fn plain_radios_refuse_the_button_group_role() {
    let _ = RadioGroup::<&str>::new(
        "yolo",
        "Runs in YOLO mode",
        HoverHelp::tooltip("Say"),
        Role::Group,
    )
    .radios();
}

/// What the element drawn under `id` painted in `nodes`, the driver record of one frame: the strings its subtree
/// painted, and how many SVG glyphs that subtree holds. The record has no ink colors, so a glyph's or a word's color
/// is the goldens' to pin.
fn painted_under(nodes: &[gpui::DriverNode], id: &str) -> (Vec<String>, usize) {
    let root = nodes
        .iter()
        .position(|node| node.element_id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("{id} was drawn"));
    let within = |mut index: usize| loop {
        if index == root {
            return true;
        }
        match nodes[index].parent {
            Some(parent) => index = parent,
            None => return false,
        }
    };
    let under: Vec<&gpui::DriverNode> = (0..nodes.len())
        .filter(|&index| within(index))
        .map(|index| &nodes[index])
        .collect();
    let texts = under
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect();
    let glyphs = under
        .iter()
        .filter(|node| node.type_name.ends_with("Svg"))
        .count();
    (texts, glyphs)
}

/// The role, accessible name and pressed or checked state the element drawn under `id` reports in `nodes`.
fn reported(
    nodes: &[gpui::DriverNode],
    id: &str,
) -> Option<(
    Option<Role>,
    Option<gpui::SharedString>,
    Option<gpui::Toggled>,
)> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| (node.role, node.label.clone(), node.toggled))
}

/// The launcher's destination type drew a glyph before each choice's words, and a glyph says nothing to a screen
/// reader, so the option must stay named by its label however it is drawn; an option drawn with only its glyph that
/// lost its name would be an unlabelled button. Specified, in a button group with A drawn with a glyph and its words
/// and B (chosen) with its glyph alone: A paints one glyph and its words, B one glyph and no words, and both report the
/// button role, their labels as their names, and B alone pressed.
#[gpui::test]
fn a_glyph_option_draws_its_glyph_and_keeps_its_label_as_its_name(cx: &mut TestAppContext) {
    let spec = GroupSpec {
        role: Role::Group,
        dress: |label, option| match label {
            "A" => option.icon(Icon::HostHouse),
            _ => option.icon_only(Icon::HostCloud),
        },
        ..GroupSpec::radio(&[("A", false), ("B", false)], Some("B"))
    };
    let (cx, _) = open_group(cx, spec);
    let nodes = frame(cx);
    assert_eq!(
        painted_under(&nodes, "choice-A"),
        (vec!["A".to_string()], 1),
        "A drew its glyph and its words"
    );
    assert_eq!(
        painted_under(&nodes, "choice-B"),
        (Vec::new(), 1),
        "B drew its glyph alone"
    );
    for (label, on) in [("A", gpui::Toggled::False), ("B", gpui::Toggled::True)] {
        assert_eq!(
            reported(&nodes, &format!("choice-{label}")),
            Some((Some(Role::Button), Some(label.into()), Some(on))),
            "option {label}"
        );
    }
}

/// A glyph or the danger ink changes how an option looks, never how it is reached or chosen: the old YOLO segment and
/// the destination type's glyph segments were the same pressed buttons as their neighbours, and a decorated option
/// that dropped out of Tab or ignored Enter would hide exactly the choices that matter most. Specified, in a button
/// group with the owner's action, of A with a glyph, B with its glyph alone and C in the danger ink, A chosen: Tab
/// reaches A, B and C in turn; Enter on C chooses it and then runs the action with C chosen; Shift+Tab back to B and
/// Space chooses B.
#[gpui::test]
fn glyph_and_danger_options_keep_the_button_groups_keys(cx: &mut TestAppContext) {
    let spec = GroupSpec {
        role: Role::Group,
        with_enter: true,
        dress: |label, option| match label {
            "A" => option.icon(Icon::HostHouse),
            "B" => option.icon_only(Icon::HostCloud),
            _ => option.danger(),
        },
        ..GroupSpec::radio(&[("A", false), ("B", false), ("C", false)], Some("A"))
    };
    let (cx, owner) = open_group(cx, spec);
    for id in ["before", "choice-A", "choice-B", "choice-C"] {
        press_and_draw(cx, "tab");
        assert_eq!(focused_id(cx).as_deref(), Some(id), "Tab reached {id}");
    }
    press_and_draw(cx, "enter");
    assert_eq!(
        *owner.launched_with.borrow(),
        vec![Some("C")],
        "Enter on the danger option chose it, then ran the action"
    );
    press_and_draw(cx, "shift-tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("choice-B"),
        "the premise: Shift+Tab reached B"
    );
    press_and_draw(cx, "space");
    assert_eq!(
        owner.chosen.get(),
        Some("B"),
        "Space chose the glyph-only B"
    );
}

/// Settings' terminal link choices showed what each choice does under its label, because the choices have
/// consequences a short label cannot carry, and stacked them so every line stays readable. Specified, in a radio group
/// of A (chosen, "Opens once") and B ("Passes it on"): each option paints its label, then its description below it;
/// B sits below A at the same left edge; both report the radio button role, their labels (not their descriptions) as
/// their names, and A alone checked; resting the pointer on B shows B's hover help. The accessible description is not
/// in the driver record, so that the visible line is also B's description is the module docs' promise, not checked
/// here.
#[gpui::test]
fn a_described_option_draws_its_line_under_the_label_and_the_group_stacks(cx: &mut TestAppContext) {
    let spec = GroupSpec {
        dress: |label, option| match label {
            "A" => option.description("Opens once"),
            _ => option.description("Passes it on"),
        },
        ..GroupSpec::radio(&[("A", false), ("B", false)], Some("A"))
    };
    let (cx, _) = open_group(cx, spec);
    let nodes = frame(cx);
    for (label, description, on) in [
        ("A", "Opens once", gpui::Toggled::True),
        ("B", "Passes it on", gpui::Toggled::False),
    ] {
        let id = format!("choice-{label}");
        assert_eq!(
            painted_under(&nodes, &id).0,
            vec![label.to_string(), description.to_string()],
            "{label} painted its label, then its description"
        );
        let lines: Vec<_> = nodes
            .iter()
            .flat_map(|node| node.texts.iter())
            .filter(|text| text.text == label || text.text == description)
            .map(|text| text.bounds)
            .collect();
        assert!(
            lines[1].top() >= lines[0].bottom(),
            "{label}'s description is under its label: {lines:?}"
        );
        assert_eq!(
            reported(&nodes, &id),
            Some((Some(Role::RadioButton), Some(label.into()), Some(on))),
            "option {label}"
        );
    }
    let (a, b) = (bounds(cx, "choice-A"), bounds(cx, "choice-B"));
    assert!(
        b.top() >= a.bottom() && b.left() == a.left(),
        "B is stacked under A: {a:?}, {b:?}"
    );
    let on_b = center(cx, "choice-B");
    cx.simulate_mouse_move(on_b, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(tooltip(cx), Some("Use B".into()), "B's hover help shows");
}

/// The launcher's harness row was a plain row of pressed buttons: each its own tab stop, Enter applying the harness
/// and launching, and no group around them for assistive technology. The chips look and the generic container role
/// together port it; a row that took the radio keyboard would let the arrows change the harness, and one that
/// announced a group would add an announced, named group the old row never had. Specified, with B chosen among A, B
/// and C drawn as chips in a generic container, with the owner's action: the container reports no role and no name
/// (the name half holds by construction: the driver record reads a name only from an element with a role); each chip
/// reports the button role, its label and B alone pressed, is 36 pixels high and sits 8 pixels after the chip before
/// it; Tab reaches A, B and C in turn; Down neither chooses nor moves; Enter on C chooses it and then runs the action.
#[gpui::test]
fn chips_in_a_generic_container_are_pressed_buttons_in_no_group(cx: &mut TestAppContext) {
    let spec = GroupSpec {
        role: Role::GenericContainer,
        look: RadioGroup::chips,
        with_enter: true,
        ..GroupSpec::radio(&[("A", false), ("B", false), ("C", false)], Some("B"))
    };
    let (cx, owner) = open_group(cx, spec);
    let nodes = frame(cx);
    assert_eq!(
        reported(&nodes, "choice"),
        Some((None, None, None)),
        "the row is no group: no role, no name"
    );
    for (label, on) in [
        ("A", gpui::Toggled::False),
        ("B", gpui::Toggled::True),
        ("C", gpui::Toggled::False),
    ] {
        assert_eq!(
            reported(&nodes, &format!("choice-{label}")),
            Some((Some(Role::Button), Some(label.into()), Some(on))),
            "chip {label}"
        );
    }
    let chips = [
        bounds(cx, "choice-A"),
        bounds(cx, "choice-B"),
        bounds(cx, "choice-C"),
    ];
    for chip in &chips {
        assert!(
            (chip.size.height.as_f32() - 36.).abs() < 0.5,
            "a chip is 36 pixels high: {chip:?}"
        );
    }
    for pair in chips.windows(2) {
        assert!(
            ((pair[1].left() - pair[0].right()).as_f32() - 8.).abs() < 0.5,
            "chips are 8 pixels apart: {pair:?}"
        );
    }
    for id in ["before", "choice-A", "choice-B", "choice-C"] {
        press_and_draw(cx, "tab");
        assert_eq!(focused_id(cx).as_deref(), Some(id), "Tab reached {id}");
    }
    press_and_draw(cx, "down");
    assert_eq!(owner.chosen.get(), Some("B"), "Down chose nothing");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("choice-C"),
        "and moved nothing"
    );
    press_and_draw(cx, "enter");
    assert_eq!(
        *owner.launched_with.borrow(),
        vec![Some("C")],
        "Enter chose C, then ran the action"
    );
}

// ---- The select ---------------------------------------------------------------------------------------------------

/// What a select test's owner holds: the chosen value, which the select is drawn from, every choice it heard, the
/// Enter presses that reached the element around the select, and two things the owner can change while the list is
/// open: whether the select is unavailable, and whether Low is ("No longer offered").
struct Picker {
    chosen: Rc<RefCell<Option<SharedString>>>,
    heard: Rc<RefCell<Vec<SharedString>>>,
    enters: Rc<Cell<usize>>,
    disabled: Rc<Cell<bool>>,
    low_gone: Rc<Cell<bool>>,
}

impl Picker {
    /// An owner with `initial` chosen, nothing heard, and the select and every option available.
    fn new(initial: Option<&'static str>) -> Self {
        Picker {
            chosen: Rc::new(RefCell::new(initial.map(SharedString::from))),
            heard: Rc::new(RefCell::new(Vec::new())),
            enters: Rc::new(Cell::new(0)),
            disabled: Rc::new(Cell::new(false)),
            low_gone: Rc::new(Cell::new(false)),
        }
    }

    /// The same owner, for the draw function to hold while the test keeps the original.
    fn share(&self) -> Self {
        Picker {
            chosen: self.chosen.clone(),
            heard: self.heard.clone(),
            enters: self.enters.clone(),
            disabled: self.disabled.clone(),
            low_gone: self.low_gone.clone(),
        }
    }
}

/// The test's effort select, `effort`, chosen as `chosen` says and choosing into it: Low, Medium and High, and Max,
/// which cannot be chosen ("Not offered by this model"); unavailable while the owner says so, and Low too once the
/// owner says it is gone. `extra` adds what a test needs on top (a retained label, a placeholder).
fn effort_select(picker: &Picker, extra: fn(Select) -> Select) -> Select {
    let (chosen, heard) = (picker.chosen.clone(), picker.heard.clone());
    let help = |words: &'static str| HoverHelp::tooltip(words);
    let low = SelectOption::new("low", "Low", help("Less reasoning, faster replies"));
    let low = if picker.low_gone.get() {
        low.unavailable("No longer offered")
    } else {
        low
    };
    let select = Select::new(
        "effort",
        "Effort",
        help("Effort: how hard the agent thinks"),
        Role::ComboBox,
    )
    .options([
        low,
        SelectOption::new("medium", "Medium", help("The usual reasoning")),
        SelectOption::new("high", "High", help("More reasoning, slower replies")),
        SelectOption::new("max", "Max", help("The most reasoning"))
            .unavailable("Not offered by this model"),
    ])
    .on_choose(move |value, _, _| {
        chosen.replace(Some(value.clone()));
        heard.borrow_mut().push(value.clone());
    })
    .disabled(picker.disabled.get());
    let select = match picker.chosen.borrow().clone() {
        Some(value) => select.chosen(value),
        None => select,
    };
    extra(select)
}

/// Open a window drawing a button, `before` (whose handle the test gets back), then [`effort_select`] 320 pixels wide
/// inside an element that counts the Enter presses reaching it, the select starting with `initial` chosen.
fn open_picker<'a>(
    cx: &'a mut TestAppContext,
    initial: Option<&'static str>,
    extra: fn(Select) -> Select,
) -> (&'a mut VisualTestContext, Picker, FocusHandle) {
    let picker = Picker::new(initial);
    let before = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let drawn = picker.share();
    let drawn_before = before.clone();
    let cx = open(cx, move |_, _| {
        let enters = drawn.enters.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                Button::new(
                    "before",
                    "Before",
                    HoverHelp::tooltip("A test control"),
                    Role::Button,
                )
                .focus(&drawn_before),
            )
            .child(
                // Wide enough for the unavailable option's whole line in the test context's stand-in text system.
                div()
                    .w(px(320.))
                    .on_key_down(move |event: &gpui::KeyDownEvent, _, _| {
                        if event.keystroke.key == "enter" {
                            enters.set(enters.get() + 1);
                        }
                    })
                    .child(effort_select(&drawn, extra)),
            )
            .into_any_element()
    });
    (cx, picker, before)
}

/// No additions to the test's select.
fn plain(select: Select) -> Select {
    select
}

/// Whether the select's list is drawn in a fresh frame.
fn list_open(cx: &mut VisualTestContext) -> bool {
    frame(cx)
        .iter()
        .any(|node| node.element_id.as_deref() == Some("effort-list"))
}

/// The selected state each of the list's options reports, in order, from a fresh frame.
fn option_states(cx: &mut VisualTestContext) -> Vec<(String, Option<bool>)> {
    frame(cx)
        .into_iter()
        .filter(|node| node.role == Some(Role::MenuListOption))
        .map(|node| {
            (
                node.element_id.unwrap_or_default().to_string(),
                node.selected,
            )
        })
        .collect()
}

/// Why this matters: a select's list is how the user sees the choices and which one holds now. The old native pop-up
/// marked the chosen option and started its highlight there, so the arrow keys move from the current value; and the
/// list here lines up with its control at exactly its width (decided by the migration's orchestrator for the
/// launcher's floating lists), so a list sized by its content would stick out past the form's column.
///
/// Specified: a click on the closed control opens the list just below it (four pixels down), left edges aligned, as
/// wide as the control; the list holds the options in order, Medium (the chosen one) reported selected and the rest
/// not, and Medium holds focus; the closed control reports the chosen option's label as its value.
#[gpui::test]
fn a_select_list_opens_under_its_control_at_its_width_on_the_chosen_option(
    cx: &mut TestAppContext,
) {
    let (cx, _, _) = open_picker(cx, Some("medium"), plain);
    let control = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("effort"))
        .map(|node| (node.role, node.label, node.value));
    assert_eq!(
        control,
        Some((
            Some(Role::ComboBox),
            Some("Effort".into()),
            Some("Medium".into())
        )),
        "the closed control reports its role, name and the chosen option"
    );
    assert!(!list_open(cx), "the premise: the list starts closed");

    click(cx, "effort");
    assert!(list_open(cx), "a click opened the list");
    let (list, control) = (bounds(cx, "effort-list"), bounds(cx, "effort"));
    assert_eq!(
        (list.left(), list.top(), list.size.width),
        (
            control.left(),
            control.bottom() + px(4.),
            control.size.width
        ),
        "the list sits under the control at exactly its width: {list:?} under {control:?}"
    );
    assert_eq!(
        option_states(cx),
        [
            ("effort-option-low".to_string(), Some(false)),
            ("effort-option-medium".to_string(), Some(true)),
            ("effort-option-high".to_string(), Some(false)),
            ("effort-option-max".to_string(), Some(false)),
        ],
        "the options in order, the chosen one marked"
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("effort-option-medium"),
        "focus starts on the chosen option"
    );
}

/// Why this matters: the closed control's keys are the native pop-up's as WebKit had them on macOS, and inside the
/// list the keys must choose, move and back out the way a menu does; Enter on the closed control must go on to the
/// dialog around it (the launcher's Enter runs Launch from there) and open nothing, including through GPUI's keyboard
/// click on the key's release; and a choice's key release must not reopen the list it just closed.
///
/// Specified: Tab focuses the closed control; Space opens the list on the chosen option; Down moves to the next; Enter,
/// pressed and released, chooses it and closes the list, reports the new value once and puts focus back on the closed
/// control, without reaching the element around the select or reopening the list. Down opens the list on the new
/// choice and Up moves back; Space, pressed and released, chooses and closes the same way. Alt+Down opens it; Escape
/// closes it without a choice, focus back on the control; Up opens it too. Enter on the closed control, pressed and
/// released, opens nothing and goes on to the element around it.
#[gpui::test]
fn the_select_keys_open_move_choose_and_close_as_a_native_pop_up(cx: &mut TestAppContext) {
    let (cx, picker, _) = open_picker(cx, Some("medium"), plain);
    keys(cx, "tab tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("effort"),
        "the premise: Tab reached the closed control"
    );

    keys(cx, "space");
    assert!(list_open(cx), "Space opened the list");
    assert_eq!(focused_id(cx).as_deref(), Some("effort-option-medium"));
    keys(cx, "down");
    assert_eq!(focused_id(cx).as_deref(), Some("effort-option-high"));
    press(cx, "enter", false);
    release(cx, "enter");
    assert!(!list_open(cx), "Enter chose and closed the list");
    assert_eq!(
        *picker.heard.borrow(),
        ["high"],
        "the owner heard High once"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("effort"), "focus is back");
    assert_eq!(
        picker.enters.get(),
        0,
        "the choosing Enter stopped at the list"
    );

    keys(cx, "down");
    assert!(list_open(cx), "Down opened the list");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("effort-option-high"),
        "on the new choice"
    );
    keys(cx, "up");
    assert_eq!(focused_id(cx).as_deref(), Some("effort-option-medium"));
    press(cx, "space", false);
    release(cx, "space");
    assert!(!list_open(cx), "Space chose and closed the list");
    assert_eq!(*picker.heard.borrow(), ["high", "medium"]);
    assert_eq!(focused_id(cx).as_deref(), Some("effort"), "focus is back");

    keys(cx, "alt-down");
    assert!(list_open(cx), "Alt+Down opened the list");
    keys(cx, "down escape");
    assert!(!list_open(cx), "Escape closed the list");
    assert_eq!(
        *picker.heard.borrow(),
        ["high", "medium"],
        "and chose nothing"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("effort"), "focus is back");
    keys(cx, "up");
    assert!(list_open(cx), "Up opened the list");
    keys(cx, "escape");

    press(cx, "enter", false);
    release(cx, "enter");
    assert!(!list_open(cx), "Enter on the closed control opened nothing");
    assert_eq!(
        picker.enters.get(),
        1,
        "Enter went on to what holds the select"
    );
}

/// Why this matters: a pointer user picks from the list by clicking, and an option the old app showed disabled ("Max ·
/// Not offered by this model") must not be choosable however it is reached, or the launcher would submit a choice the
/// harness refuses; its reason must be on screen, since that is the only place the user learns it.
///
/// Specified: a click on an option chooses it and closes the list, focus back on the closed control; a click on the
/// unavailable option leaves the list open and chooses nothing, and the option's line reads "Max · Not offered by this
/// model"; from the first option Up wraps to the last that can be chosen, passing over the unavailable one; a click on
/// the closed control while its list is open closes the list.
#[gpui::test]
fn a_click_chooses_an_option_and_an_unavailable_one_cannot_be_chosen(cx: &mut TestAppContext) {
    let (cx, picker, _) = open_picker(cx, Some("medium"), plain);
    click(cx, "effort");
    click(cx, "effort-option-low");
    assert!(!list_open(cx), "the click chose and closed the list");
    assert_eq!(*picker.heard.borrow(), ["low"]);
    assert_eq!(focused_id(cx).as_deref(), Some("effort"), "focus is back");

    click(cx, "effort");
    assert!(list_open(cx), "the premise: the list is open again");
    click(cx, "effort-option-max");
    assert!(
        list_open(cx),
        "a click on the unavailable option left it open"
    );
    assert_eq!(*picker.heard.borrow(), ["low"], "and chose nothing");
    let painted: Vec<String> = frame(cx)
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect();
    assert!(
        painted
            .iter()
            .any(|text| text == "Max · Not offered by this model"),
        "the unavailable option shows its reason; painted: {painted:?}"
    );

    keys(cx, "escape");
    click(cx, "effort");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("effort-option-low"),
        "the premise: the list opened on Low"
    );
    keys(cx, "up");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("effort-option-high"),
        "Up wrapped to High, past Max"
    );
    click(cx, "effort");
    assert!(!list_open(cx), "a click on the control closed its list");
}

/// Why this matters: on macOS with Keyboard navigation off (the system default) Tab reaches text fields and pop-ups
/// only, and a select is a pop-up: WebKit kept it in Tab's order there while skipping buttons (SPEC.md, "Keyboard
/// focus"). A select given the button rule would be unreachable from the keyboard for most Mac users.
///
/// Specified: with Tab reaching text fields and pop-ups only, Tab from nothing focused passes the button before the
/// select and lands on the select.
#[gpui::test]
fn tab_reaches_a_select_where_it_reaches_only_text_fields_and_pop_ups(cx: &mut TestAppContext) {
    let (cx, _, before) = open_picker(cx, Some("medium"), plain);
    let reach = Rc::new(Cell::new(TabReach::TextFieldsAndPopups));
    crate::test_support::follow(cx, &reach);
    assert_eq!(focused_id(cx), None, "the premise: nothing has focus yet");
    keys(cx, "tab");
    assert!(
        !cx.update(|window, _| before.is_focused(window)),
        "Tab skipped the button"
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("effort"),
        "and landed on the select"
    );
}

/// The test select with a retained value's label.
fn retaining(select: Select) -> Select {
    select.retained(
        "Unavailable effort (retained)",
        HoverHelp::tooltip("The stored effort, no longer offered"),
    )
}

/// The test select with a placeholder.
fn with_placeholder(select: Select) -> Select {
    select.placeholder("Choose an effort")
}

/// The value the closed control reports, from a fresh frame.
fn shown_value(cx: &mut VisualTestContext) -> Option<SharedString> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("effort"))
        .and_then(|node| node.value)
}

/// Why this matters: the launcher's environment selects keep showing a stored value that is no longer offered until
/// the user picks another, rather than silently showing (and later submitting) the first option, as the old ones did
/// with "Unavailable recipe (retained)".
///
/// Specified: with "ultra" chosen, which no option is, the closed control shows the retained label; its list starts
/// with that entry, chosen and focused, then the four options; choosing Low reports "low".
#[gpui::test]
fn a_select_keeps_showing_a_retained_value_until_another_is_chosen(cx: &mut TestAppContext) {
    let (cx, picker, _) = open_picker(cx, Some("ultra"), retaining);
    assert_eq!(
        shown_value(cx).as_deref(),
        Some("Unavailable effort (retained)")
    );
    click(cx, "effort");
    assert_eq!(
        option_states(cx).first(),
        Some(&("effort-option-ultra".to_string(), Some(true))),
        "the retained value comes first, chosen"
    );
    assert_eq!(option_states(cx).len(), 5, "then the four options");
    assert_eq!(focused_id(cx).as_deref(), Some("effort-option-ultra"));
    click(cx, "effort-option-low");
    assert_eq!(*picker.heard.borrow(), ["low"]);
}

/// Why this matters: a select with nothing chosen must say so rather than look chosen, as the old "Choose a host" did;
/// a list that marked or started on some option would suggest a value the owner does not hold.
///
/// Specified: with nothing chosen and a placeholder, the closed control shows the placeholder, and its list marks no
/// option and starts on the first that can be chosen.
#[gpui::test]
fn a_select_with_nothing_chosen_shows_its_placeholder(cx: &mut TestAppContext) {
    let (cx, _, _) = open_picker(cx, None, with_placeholder);
    assert_eq!(shown_value(cx).as_deref(), Some("Choose an effort"));
    click(cx, "effort");
    assert!(
        option_states(cx)
            .iter()
            .all(|(_, selected)| *selected == Some(false)),
        "no option is marked"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("effort-option-low"));
}

/// Why this matters: a native select fired no change when the user picked the value it already held, and an owner
/// that saves on every choice (a Settings select) would otherwise send a write that changes nothing each time the user
/// opens the list and closes it on the current value.
///
/// Specified: with Medium chosen, a click on Medium in the list closes the list without telling the owner, and so does
/// Enter on Medium after Space opened the list; focus is back on the closed control both times.
#[gpui::test]
fn choosing_the_chosen_option_closes_the_list_and_tells_the_owner_nothing(cx: &mut TestAppContext) {
    let (cx, picker, _) = open_picker(cx, Some("medium"), plain);
    click(cx, "effort");
    assert!(list_open(cx), "the premise: the list is open");
    click(cx, "effort-option-medium");
    assert!(!list_open(cx), "the click closed the list");
    assert!(
        picker.heard.borrow().is_empty(),
        "and told the owner nothing"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("effort"));

    keys(cx, "space");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("effort-option-medium"),
        "the premise: the list opened on Medium"
    );
    press(cx, "enter", false);
    release(cx, "enter");
    assert!(!list_open(cx), "Enter closed the list");
    assert!(
        picker.heard.borrow().is_empty(),
        "and told the owner nothing"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("effort"));
}

/// Why this matters: the list is a copy of what the owner drew when it opened, and the owner's state can change while
/// it is open (a harness's efforts arriving, a value chosen by another control); a list that kept the old copy would
/// mark a value the owner no longer holds and let the user choose an option the owner has withdrawn.
///
/// Specified: with the list open on Medium, the owner choosing High and withdrawing Low ("No longer offered") shows in
/// the open list on the next frame: High is marked and Medium not, Low reads "Low · No longer offered", and a click on
/// Low leaves the list open and tells the owner nothing.
#[gpui::test]
fn the_open_list_follows_its_owner(cx: &mut TestAppContext) {
    let (cx, picker, _) = open_picker(cx, Some("medium"), plain);
    click(cx, "effort");
    assert_eq!(
        option_states(cx)[1],
        ("effort-option-medium".to_string(), Some(true)),
        "the premise: the list opened with Medium marked"
    );
    picker.chosen.replace(Some("high".into()));
    picker.low_gone.set(true);
    redraw(cx);
    let states = option_states(cx);
    assert_eq!(
        (states[1].1, states[2].1),
        (Some(false), Some(true)),
        "High is marked now, not Medium: {states:?}"
    );
    let painted: Vec<String> = frame(cx)
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect();
    assert!(
        painted.iter().any(|text| text == "Low · No longer offered"),
        "Low shows why it is withdrawn; painted: {painted:?}"
    );
    click(cx, "effort-option-low");
    assert!(
        list_open(cx),
        "a click on the withdrawn Low left the list open"
    );
    assert!(
        picker.heard.borrow().is_empty(),
        "and told the owner nothing"
    );
}

/// Why this matters: an owner disables its select while work runs (the old launcher's Effort select was `disabled:
/// busy`), and that can happen while the list is open. A list left open would let the user choose on a control the
/// owner said is unavailable; closing it must not leave focus on a control that no longer takes focus, where keys would
/// reach nothing.
///
/// Specified: with the list open from the keyboard, the owner drawing the select unavailable closes the list on the
/// next frame; focus is then on a drawn element, not on the select nor on an option, and the harness's focus invariant
/// (something holds focus) holds throughout.
#[gpui::test]
fn a_select_made_unavailable_while_its_list_is_open_closes_the_list(cx: &mut TestAppContext) {
    let (cx, picker, _) = open_picker(cx, Some("medium"), plain);
    keys(cx, "tab tab space");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("effort-option-medium"),
        "the premise: the list is open, with focus in it"
    );
    picker.disabled.set(true);
    redraw(cx);
    assert!(!list_open(cx), "the list closed");
    let nodes = frame(cx);
    let focused: Vec<_> = nodes.iter().filter(|node| node.flags.focused).collect();
    assert!(
        !focused.is_empty(),
        "focus is on an element drawn in the frame"
    );
    assert!(
        focused.iter().all(|node| node
            .element_id
            .as_deref()
            .is_none_or(|id| !id.starts_with("effort"))),
        "not on the select or an option: {:?}",
        focused
            .iter()
            .map(|node| &node.element_id)
            .collect::<Vec<_>>()
    );
}

/// Why this matters: a press on the closed control focuses it, taking focus out of the open list. If the list closed
/// only on the click, a press dragged off the control before its release (no click) would leave the list open with
/// focus outside it, and the arrow keys would reach nothing in it.
///
/// Specified: with the list open, a press on the closed control released elsewhere closes the list; the next click on
/// the control opens it again.
#[gpui::test]
fn a_press_on_the_closed_control_closes_its_open_list(cx: &mut TestAppContext) {
    let (cx, _, _) = open_picker(cx, Some("medium"), plain);
    click(cx, "effort");
    assert!(list_open(cx), "the premise: the list is open");
    let control = bounds(cx, "effort");
    let elsewhere = bounds(cx, "before").center();
    cx.simulate_mouse_down(control.center(), MouseButton::Left, Modifiers::none());
    check(cx);
    cx.simulate_mouse_up(elsewhere, MouseButton::Left, Modifiers::none());
    check(cx);
    assert!(!list_open(cx), "the press closed the list");
    click(cx, "effort");
    assert!(list_open(cx), "the next click opened it again");
}

/// A dialog's content for the empty-space test: a select drawn 240 pixels wide with room around it, under
/// `picker-panel`, whose focus handle the dialog starts on.
struct PickerPanel {
    focus: FocusHandle,
    picker: Picker,
}

impl Render for PickerPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "picker-panel".into())
            .w(px(320.))
            .h(px(240.))
            .p(px(40.))
            .child(effort_select(&self.picker, plain).focus(&self.focus))
    }
}

/// Why this matters: SPEC.md has a click on the empty space of a dialog that does not keep focus on the control in use
/// (Rename, say) focus the dialog itself, so Enter then presses its main action. With a select's list open inside the
/// dialog, the same click first closes the list, which hands focus back to the select; the dialog then saw focus had
/// moved during the press, took that for the click landing in a control, and left focus on the select, so the click
/// acted as a click on the select. A press on empty space must mean the same with or without a list open.
///
/// Specified: in a dialog whose empty space takes focus, a click on its empty space with nothing open focuses the
/// dialog (the premise); with the select's list opened by a click on the select, a click on the same spot closes the
/// list and focuses the dialog, not the select.
#[gpui::test]
fn a_click_on_a_dialogs_empty_space_with_a_select_list_open_focuses_the_dialog(
    cx: &mut TestAppContext,
) {
    let cx = open(cx, |_, _| div().into_any_element());
    let select = cx.update(|_, cx| cx.focus_handle().tab_stop(true));
    let container = cx.update(|_, cx| cx.focus_handle());
    let (start, kept) = (select.clone(), container.clone());
    let opened = cx.update(move |window, cx| {
        let picker = Picker::new(Some("medium"));
        let focus = start.clone();
        let view = cx.new(|_| PickerPanel { focus, picker });
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container: kept,
                start,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: false,
                returns_to: crate::focus::ReturnTo::Opener,
                on_dismiss: None,
                placement: layers::DialogPlacement::Centered,
                dismiss_on_outside_press: false,
            },
            &mark,
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    assert!(opened.is_some(), "the premise: the dialog opened");
    let empty = bounds(cx, "picker-panel").origin + gpui::point(px(6.), px(6.));
    let focused = |cx: &mut VisualTestContext, handle: &FocusHandle| {
        cx.update(|window, _| handle.is_focused(window))
    };
    assert!(
        focused(cx, &select),
        "the premise: the dialog starts on the select"
    );
    cx.simulate_click(empty, Modifiers::none());
    check(cx);
    assert!(
        focused(cx, &container),
        "the premise: with nothing open, a click on the empty space focuses the dialog"
    );

    click(cx, "effort");
    assert!(
        list_open(cx),
        "the premise: the click opened the select's list"
    );
    cx.simulate_click(empty, Modifiers::none());
    check(cx);
    assert!(!list_open(cx), "the click outside the list closed it");
    assert!(
        focused(cx, &container),
        "and focused the dialog, as with nothing open"
    );
    assert!(!focused(cx, &select), "not the select");
}

// ---- The mark toggle ----------------------------------------------------------------------------------------------

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
                        super::MarkToggle::new(
                            "enabled",
                            "Mark read",
                            HoverHelp::tooltip("Idle — new output — Mark read"),
                            Role::Button,
                        )
                        .mark(mark())
                        .on_click(move |_, _, _| toggled.set(toggled.get() + 1)),
                    )
                    .child(
                        super::MarkToggle::new(
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

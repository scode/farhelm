//! Behavior tests for the components, on GPUI's simulated platform.
//!
//! These cover what a component promises beyond its looks (the goldens pin those): activation, the disabled state,
//! hover help reaching the layer manager and leaving with its control, the dialog frame keeping its answers reachable,
//! and the text input's events, with the line breaks only the field that keeps them keeps. Each test opens its window
//! through the shared harness ([`crate::test_support::open`]), and each step ends with its invariant checks (the focus
//! coordinator's and the layer manager's rules).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AppContext as _, Context, FocusHandle, InteractiveElement, IntoElement, Modifiers,
    ParentElement, Render, ScrollDelta, ScrollEvent, Styled, Subscription, TestAppContext,
    VisualTestContext, Window, div, point, px, size,
};

use gpui::prelude::FluentBuilder as _;

use super::{
    Button, DialogFrame, HoverHelp, Role, TextInput, TextInputEvent, TextInputState, Tier,
};
use crate::layers;
use crate::test_support::{advance, bounds, center, check, click, keys, open, redraw};

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

/// How a test makes a field's state: [`TextInputState::new`] or [`TextInputState::new_keeping_line_breaks`].
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

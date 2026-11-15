//! Tests of the text input: the events it reports to its owner, the growing box, the field that keeps line breaks,
//! whether an Enter was a held repeat, and the Up and Down keys it hands to the owner of the list beside it.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext as _, Context, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Styled as _, Subscription, TestAppContext, VisualTestContext, Window, div, px,
};

use crate::components::{HoverHelp, TextInput, TextInputEvent, TextInputState, Vertical};
use crate::test_support::{bounds, check, click, frame, keys, open, redraw};

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
    assert_eq!(
        *events.borrow(),
        [TextInputEvent::Submitted { held: false }]
    );
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

/// The masked field stands in for the old `type="password"` inputs (the ntfy access token): its owner must get the text
/// as typed, or a save would send something else, while the window must not show it. Specified: "abc" typed into a
/// plain field and "xyz" into a masked one each report three edits and hold what was typed; the painted frame shows
/// "abc" (the premise that a field's text is painted at all) and nowhere "xyz".
#[gpui::test]
fn a_masked_input_holds_what_was_typed_and_paints_none_of_it(cx: &mut TestAppContext) {
    let (cx, fields) = open_fields(
        cx,
        &[
            ("plain", TextInputState::new),
            ("secret", TextInputState::new_masked),
        ],
    );
    click(cx, "plain");
    keys(cx, "a b c");
    click(cx, "secret");
    keys(cx, "x y z");
    let edits = vec![TextInputEvent::Changed; 3];
    assert_eq!(take(cx, &fields[0]), ("abc".into(), edits.clone()));
    assert_eq!(take(cx, &fields[1]), ("xyz".into(), edits));
    let painted: String = frame(cx)
        .into_iter()
        .flat_map(|node| node.texts.into_iter().map(|line| line.text.to_string()))
        .collect();
    assert!(painted.contains("abc"), "the premise: {painted:?}");
    assert!(
        !painted.contains("xyz"),
        "the secret is masked: {painted:?}"
    );
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
    let max = crate::components::text_input::GROWING_MAX_ROWS;
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
            ("ab".into(), vec![TextInputEvent::Submitted { held: false }]),
            "{chord} submitted and inserted nothing"
        );
        keys(cx, SELECT_ALL);
        keys(cx, chord);
        assert_eq!(
            take(cx, kept),
            ("ab".into(), vec![TextInputEvent::Submitted { held: false }]),
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

// ---- A held Enter ---------------------------------------------------------------------------------------------------

/// Why this matters: Kit's field submits on every auto-repeat of a held Enter, and an owner whose main action must run
/// once per press (SPEC.md for the launcher: "A held Enter never launches") can tell a repeat from a press only by what
/// the event says. A flag read from the wrong key-down would launch on a repeat or drop a real press.
///
/// Specification: in the single-line field and in the field that keeps line breaks, a fresh Enter reports
/// `Submitted { held: false }`, its auto-repeat `Submitted { held: true }`, and the next fresh Enter
/// `Submitted { held: false }` again, each with nothing else, and the text stays as typed.
#[gpui::test]
fn an_enter_says_whether_it_was_a_held_repeat(cx: &mut TestAppContext) {
    let (cx, fields) = open_fields(
        cx,
        &[
            ("line", TextInputState::new),
            ("kept", TextInputState::new_keeping_line_breaks),
        ],
    );
    for (selector, field) in ["line", "kept"].into_iter().zip(&fields) {
        click(cx, selector);
        keys(cx, "a");
        assert_eq!(
            take(cx, field),
            ("a".into(), vec![TextInputEvent::Changed]),
            "the premise: {selector} has focus"
        );
        for held in [false, true, false] {
            super::press(cx, "enter", held);
            assert_eq!(
                take(cx, field),
                ("a".into(), vec![TextInputEvent::Submitted { held }]),
                "{selector}: an Enter with held {held}"
            );
        }
    }
}

/// Why this matters: Enter pressed elsewhere (on one field, or on a control that moves focus) can go on repeating into
/// a field once focus is there. Were that first repeat reported as a fresh press, an owner's once-per-press action
/// would run on a key the user pressed for something else. The checkbox, the radio group and the select read the
/// platform's held flag the same way, so no control disagrees about which Enter is held.
///
/// Specification: after a fresh Enter in one field and a click on a second, the held repeat that arrives in the second
/// reports `Submitted { held: true }` there, and the first field reports nothing more.
#[gpui::test]
fn a_held_enter_first_pressed_elsewhere_is_held_in_the_field(cx: &mut TestAppContext) {
    let (cx, fields) = open_fields(
        cx,
        &[
            ("first", TextInputState::new),
            ("second", TextInputState::new),
        ],
    );
    click(cx, "first");
    super::press(cx, "enter", false);
    assert_eq!(
        take(cx, &fields[0]).1,
        [TextInputEvent::Submitted { held: false }],
        "the premise: the first press was the first field's"
    );
    click(cx, "second");
    // Asserted rather than shown by typing, which would put a keystroke between the press and its repeat.
    assert!(
        cx.update(|window, cx| fields[1].read(cx).state.focus_handle(cx).is_focused(window)),
        "the premise: the second field has focus"
    );
    super::press(cx, "enter", true);
    assert_eq!(
        take(cx, &fields[1]).1,
        [TextInputEvent::Submitted { held: true }]
    );
    assert_eq!(take(cx, &fields[0]).1, [], "the first field saw no repeat");
}

/// Why this matters: an Enter chord Kit has no binding for (Alt+Enter) passes the field's key-down listener and submits
/// nothing, so its held flag stays recorded. The Shift+Enter that the field keeping line breaks rewrites at the action
/// never passes that listener; were it to read the leftover flag, a fresh press would be reported as held, and an owner
/// that skips held Enters would drop it.
///
/// Specification: in the field that keeps line breaks, a held Alt+Enter reports nothing and leaves the text, and a
/// fresh Shift+Enter after it reports `Submitted { held: false }`.
#[gpui::test]
fn a_shift_enter_never_reads_an_earlier_enter_that_submitted_nothing(cx: &mut TestAppContext) {
    let (cx, fields) = open_fields(cx, &[("kept", TextInputState::new_keeping_line_breaks)]);
    click(cx, "kept");
    keys(cx, "a");
    assert_eq!(
        take(cx, &fields[0]),
        ("a".into(), vec![TextInputEvent::Changed]),
        "the premise: kept has focus"
    );
    super::press(cx, "alt-enter", true);
    assert_eq!(
        take(cx, &fields[0]),
        ("a".into(), vec![]),
        "the premise: a held Alt+Enter submits nothing"
    );
    super::press(cx, "shift-enter", false);
    assert_eq!(
        take(cx, &fields[0]),
        ("a".into(), vec![TextInputEvent::Submitted { held: false }])
    );
}

// ---- A field that drives a list: the arrows handed over -------------------------------------------------------------

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

// ---- One subscriber per state ---------------------------------------------------------------------------------------

/// Why this matters: a state's subscriptions share the record of the last reported text, so a second subscriber never
/// hears of an edit, and nothing on screen shows it. Both Credentials editors once watched a field their draft already
/// watched, and an error and a pending file read that an edit should have cleared stayed; two reviews found it only by
/// reading the code. The debug build the tests run in has to fail instead.
///
/// Specified: two subscriptions to one state made in one update, the second through a clone, as an editor and its draft
/// made theirs, fail a debug build once GPUI activates them, saying that a state takes one subscriber.
#[cfg(debug_assertions)]
#[gpui::test]
#[should_panic(
    expected = "the field already has a subscription, and a state takes one subscriber, since a second never hears of \
                an edit"
)]
fn a_second_subscriber_while_the_first_is_kept_fails_a_debug_build(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let events = Rc::new(RefCell::new(Vec::new()));
    // Both are handed out of the update, so they are still kept when it returns and GPUI activates them.
    let _kept = cx.update(|window, cx| {
        let mut second = None;
        let field = cx.new(|cx| {
            // `Field::new` subscribes and keeps that first subscription in the field.
            let field = Field::new(&events, window, cx);
            second = Some(
                field
                    .state
                    .clone()
                    .subscribe(window, cx, |_: &mut Field, _, _, _| {}),
            );
            field
        });
        (field, second)
    });
}

/// Replace `field`'s subscription by plain assignment, as a view rebuilding its watch would, with one that records into
/// `into` instead of the field's own record.
fn resubscribe(
    cx: &mut VisualTestContext,
    field: &gpui::Entity<Field>,
    into: &Rc<RefCell<Vec<TextInputEvent>>>,
) {
    let into = into.clone();
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field._events = field
                .state
                .subscribe(window, cx, move |_: &mut Field, event, _, _| {
                    into.borrow_mut().push(event)
                });
        })
    });
    check(cx);
}

/// Why this matters: the check above must catch only the case that starves a subscriber. Replacing a subscription by
/// assignment is the ordinary way to rebuild a watch, and the old one is dropped before GPUI activates the new one, so
/// the two never both receive an event; failing it would forbid that.
///
/// Specified: assigning a new subscription over a field's kept one does not panic, twice in a row; after each, the next
/// edit reaches the newest one once and the earlier ones not at all.
#[gpui::test]
fn replacing_a_subscription_by_assignment_is_allowed_and_the_replacement_hears_edits(
    cx: &mut TestAppContext,
) {
    let (cx, fields) = open_fields(cx, &[("line", TextInputState::new)]);
    let field = &fields[0];
    click(cx, "line");
    keys(cx, "a");
    assert_eq!(
        take(cx, field),
        ("a".into(), vec![TextInputEvent::Changed]),
        "the premise: the first subscription reports edits"
    );

    // Twice, so that a second replacement, which the next test relies on, is shown not to panic or starve either.
    let first = Rc::new(RefCell::new(Vec::new()));
    resubscribe(cx, field, &first);
    keys(cx, "b");
    assert_eq!(
        take(cx, field),
        ("ab".into(), vec![]),
        "the original subscription heard nothing"
    );
    assert_eq!(
        first.borrow_mut().drain(..).collect::<Vec<_>>(),
        [TextInputEvent::Changed],
        "the first replacement reports the edit"
    );

    let second = Rc::new(RefCell::new(Vec::new()));
    resubscribe(cx, field, &second);
    keys(cx, "c");
    assert_eq!(
        take(cx, field),
        ("abc".into(), vec![]),
        "the original subscription still heard nothing"
    );
    assert_eq!(
        *first.borrow(),
        [],
        "the first replacement heard nothing once replaced"
    );
    assert_eq!(
        *second.borrow(),
        [TextInputEvent::Changed],
        "the second replacement reports the edit"
    );
}

/// Why this matters: a replacement drops the old subscription while the new one is already counted, and bookkeeping
/// that lost track there would let the next second subscriber starve unnoticed. The panic also has to name the call
/// that subscribed twice, not the text input's own code, or finding the culprit takes a backtrace.
///
/// Specified: after two replacements by assignment, a further subscription kept beside the field's fails a debug build,
/// and the panic names that call's site in this file.
#[cfg(debug_assertions)]
#[gpui::test]
#[should_panic(
    expected = "TextInputState::subscribe at crates/gui/farhelm-gui/src/components/tests/text_input.rs:"
)]
fn a_second_subscriber_after_replacements_still_fails_and_names_its_caller(
    cx: &mut TestAppContext,
) {
    let (cx, fields) = open_fields(cx, &[("line", TextInputState::new)]);
    let field = &fields[0];
    let replacement = Rc::new(RefCell::new(Vec::new()));
    // Twice, because a count that went wrong in one replacement could show only at the next; the test above shows
    // that both replacements pass, so the panic expected here can only come from the subscription below.
    resubscribe(cx, field, &replacement);
    resubscribe(cx, field, &replacement);
    // Handed out of the update, so it is still kept beside the field's when GPUI activates it.
    let _second = cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field
                .state
                .clone()
                .subscribe(window, cx, |_: &mut Field, _, _, _| {})
        })
    });
}

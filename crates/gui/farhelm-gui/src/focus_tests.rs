//! Behavior tests for the focus coordinator, on GPUI's simulated platform: Tab and the dialog trap, restoring focus
//! when a layer closes, home, choosing a terminal, which controls Tab reaches and which press focuses one, and what
//! counts as the user moving on.
//!
//! Each test opens its window through the shared harness ([`crate::test_support::open`]: GPUI Kit's `Root` around a
//! root view wired as the app's), drives it through simulated keys and clicks, so key bindings resolve as they do in
//! the app, and runs the harness's invariant checks after every step. The dialogs, menus and questions these tests trap
//! focus in are the harness's shared fixtures, apart from the placed dialog below, whose controls sit at fixed
//! positions; the layer manager's and the prompt's own rules are tested in `layers_tests.rs` and `prompt_tests.rs`.
//! The rules come from SPEC.md, "Keyboard focus", and SPEC_impl.md, "Focus".

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, ParentElement, Render,
    ScrollDelta, ScrollEvent, ScrollHandle, StatefulInteractiveElement as _, Styled,
    TestAppContext, VisualTestContext, WeakFocusHandle, Window, div, point, px, size,
};

use crate::components::{
    Button, DialogFrame, HoverHelp, Select, SelectOption, TextInput, TextInputState,
};
use crate::focus::{self, Mark, OnPress};
use crate::layers::{self, Anchor, Dialog};
use crate::prompt::{self, Presentation};
use crate::test_support::{
    Panel, advance, anchor_element, bounds, button, center, check, click, focused, follow, handle,
    is_open, keys, open, open_dialog, open_dialog_returning, open_menu, question, redraw, settle,
};

// ---- Helpers on top of the shared harness ----------------------------------------------------------------------------

/// A control in a [`Placed`] dialog: a button with a focus handle the test holds, a text field, or an element a press
/// focuses whose handle is not a tab stop, so the coordinator records no position for it (as for a control that tracks
/// its own focus and records none).
enum PlacedKind {
    Button(FocusHandle),
    Field,
    Pressable(FocusHandle),
}

/// A dialog's content view whose controls sit at fixed horizontal positions, in the order given, which is their tab
/// order. That lets a test put the control nearest to another somewhere other than next to it in tab order. The last
/// control can be taken away. The field's state is made on first draw and kept, so what it holds can be read back.
struct Placed {
    controls: Vec<(&'static str, f32, PlacedKind)>,
    shown: Rc<Cell<bool>>,
    field: FieldState,
}

impl Render for Placed {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = self.shown.get();
        let count = self.controls.len();
        let mut panel = div()
            .debug_selector(|| "panel".into())
            .w(px(600.))
            .h(px(60.));
        for (index, (id, x, kind)) in self.controls.iter().enumerate() {
            if !shown && index + 1 == count {
                continue;
            }
            // Buttons carry their own selector (their id); the field's wrapper carries the field's.
            let placed = div().absolute().left(px(*x)).top(px(10.));
            panel = panel.child(match kind {
                PlacedKind::Button(focus) => placed.child(button(id, focus)),
                PlacedKind::Pressable(focus) => {
                    let selector = *id;
                    focus::track(
                        placed.debug_selector(move || selector.into()).size(px(30.)),
                        focus,
                        OnPress::Focus,
                    )
                }
                PlacedKind::Field => {
                    let selector = *id;
                    let placed = placed.debug_selector(move || selector.into());
                    let state = self
                        .field
                        .borrow_mut()
                        .get_or_insert_with(|| TextInputState::new(window, cx))
                        .clone();
                    placed.w(px(80.)).child(TextInput::new(
                        *id,
                        &state,
                        "Field",
                        HoverHelp::tooltip("A test field"),
                    ))
                }
            });
        }
        panel
    }
}

/// A [`Placed`] field's state, filled in when the field is first drawn.
type FieldState = Rc<RefCell<Option<TextInputState>>>;

/// Open a [`Placed`] dialog starting on `start`; returns the dialog's container, whether its last control is shown,
/// and its field's state once drawn.
fn open_placed_dialog(
    cx: &mut VisualTestContext,
    controls: Vec<(&'static str, f32, PlacedKind)>,
    start: &FocusHandle,
) -> (FocusHandle, Rc<Cell<bool>>, FieldState) {
    open_placed_dialog_with(cx, controls, start, true)
}

/// [`open_placed_dialog`] with the dialog's empty-space rule given: `keeps` is
/// [`Dialog::keeps_focus_on_empty_click`].
fn open_placed_dialog_with(
    cx: &mut VisualTestContext,
    controls: Vec<(&'static str, f32, PlacedKind)>,
    start: &FocusHandle,
    keeps: bool,
) -> (FocusHandle, Rc<Cell<bool>>, FieldState) {
    let shown = Rc::new(Cell::new(true));
    let field = Rc::new(RefCell::new(None));
    let container = handle(cx);
    let (start, kept, view_shown, view_field) = (
        start.clone(),
        container.clone(),
        shown.clone(),
        field.clone(),
    );
    let id = cx.update(move |window, cx| {
        let view = cx.new(|_| Placed {
            controls,
            shown: view_shown,
            field: view_field,
        });
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container: kept,
                start,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: keeps,
                returns_to: focus::ReturnTo::Opener,
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
    assert!(id.is_some(), "the placed dialog did not open");
    (container, shown, field)
}

// ---- Focus: Tab, the trap, restore, home ----------------------------------------------------------------------------

/// Tab and Shift+Tab move through the controls in order, and Farhelm's coordinator handles them rather than GPUI Kit's
/// `Root`, which binds the same keys one context up. If Kit's handler ran instead, the dialog trap below would never
/// apply, because Kit knows nothing of Farhelm's layers.
#[gpui::test]
fn tab_and_shift_tab_move_through_the_controls_in_order(cx: &mut TestAppContext) {
    let handles = Rc::new(RefCell::new(Vec::new()));
    let drawn = handles.clone();
    let cx = open(cx, move |_, _| {
        div()
            .flex()
            .gap(px(8.))
            .children(
                drawn
                    .borrow()
                    .iter()
                    .zip(["one", "two", "three"])
                    .map(|(focus, id)| button(id, focus)),
            )
            .into_any_element()
    });
    let made: Vec<FocusHandle> = (0..3).map(|_| handle(cx)).collect();
    handles.replace(made.clone());
    redraw(cx);

    keys(cx, "tab");
    assert!(
        focused(cx, &made[0]),
        "the first Tab reaches the first control"
    );
    keys(cx, "tab");
    assert!(focused(cx, &made[1]));
    keys(cx, "shift-tab");
    assert!(focused(cx, &made[0]), "Shift+Tab goes back");
}

/// SPEC.md: "Every modal dialog keeps Tab and Shift+Tab within its visible, enabled controls", and the page behind it
/// cannot receive focus. Tab from the dialog's last control wraps to its first, never out to the page.
#[gpui::test]
fn tab_stays_inside_the_topmost_dialog(cx: &mut TestAppContext) {
    let page = Rc::new(RefCell::new(None));
    let drawn = page.clone();
    let cx = open(cx, move |_, _| {
        div()
            .flex()
            .children(drawn.borrow().clone().map(|focus| button("page", &focus)))
            .into_any_element()
    });
    let behind = handle(cx);
    page.replace(Some(behind.clone()));
    redraw(cx);
    let (first, second) = (handle(cx), handle(cx));
    open_dialog(
        cx,
        vec![("first", first.clone()), ("second", second.clone())],
        &first,
    );
    assert!(
        focused(cx, &first),
        "the premise: focus starts on the dialog's first control"
    );

    for _ in 0..3 {
        keys(cx, "tab");
        assert!(
            !focused(cx, &behind),
            "Tab never reaches the page behind the dialog"
        );
    }
    assert!(
        focused(cx, &second),
        "three Tabs over two controls land on the second"
    );
    keys(cx, "tab");
    assert!(
        focused(cx, &first),
        "Tab from the last control wraps to the first"
    );
    keys(cx, "shift-tab");
    assert!(
        focused(cx, &second),
        "Shift+Tab from the first wraps to the last"
    );
}

/// Closing something the user opened returns focus to where it was before it opened (SPEC.md, "Opening and closing"),
/// and Escape closes only the topmost layer.
#[gpui::test]
fn escape_closes_the_dialog_and_focus_returns_to_its_opener(cx: &mut TestAppContext) {
    let page = Rc::new(RefCell::new(None));
    let drawn = page.clone();
    let cx = open(cx, move |_, _| {
        div()
            .children(drawn.borrow().clone().map(|focus| button("opener", &focus)))
            .into_any_element()
    });
    let opener = handle(cx);
    page.replace(Some(opener.clone()));
    redraw(cx);
    keys(cx, "tab");
    assert!(focused(cx, &opener), "the premise: the opener has focus");

    let inside = handle(cx);
    let (outer, _, _) = open_dialog(cx, vec![("inside", inside.clone())], &inside);
    let nested = handle(cx);
    let (inner, _, _) = open_dialog(cx, vec![("nested", nested.clone())], &nested);
    assert!(focused(cx, &nested));

    keys(cx, "escape");
    assert!(!is_open(cx, inner), "Escape closed the topmost dialog");
    assert!(is_open(cx, outer), "and only that one");
    assert!(
        focused(cx, &inside),
        "focus went back to what held it when the top dialog opened"
    );

    keys(cx, "escape");
    assert!(!is_open(cx, outer));
    assert!(focused(cx, &opener), "and then back to the page's opener");
}

/// SPEC.md, "Opening and closing": "When the focused control goes away, focus moves to the closest remaining control
/// in the same dialog or menu, never out to the page or a terminal behind it." Tab order here is `near`, `far`, `going`,
/// but `near` sits right beside `going` and `far` across the dialog, so the control that takes focus when `going`
/// vanishes tells closest-by-position apart from "the previous tab stop" (`far`), and from the dialog itself (where a
/// plain Enter would press the dialog's main action).
#[gpui::test]
fn a_control_vanishing_inside_a_dialog_moves_focus_to_the_closest_remaining_one(
    cx: &mut TestAppContext,
) {
    let cx = open(cx, |_, _| div().into_any_element());
    let (far, near, going) = (handle(cx), handle(cx), handle(cx));
    let (container, shown, _) = open_placed_dialog(
        cx,
        vec![
            ("near", 380., PlacedKind::Button(near.clone())),
            ("far", 10., PlacedKind::Button(far.clone())),
            ("going", 480., PlacedKind::Button(going.clone())),
        ],
        &going,
    );
    assert!(
        focused(cx, &going),
        "the premise: the control about to vanish has focus"
    );
    let (far_at, near_at, going_at) = (center(cx, "far"), center(cx, "near"), center(cx, "going"));
    assert!(
        (going_at.x - near_at.x).abs() < (going_at.x - far_at.x).abs(),
        "the premise: `near` is the closer one"
    );
    keys(cx, "shift-tab");
    assert!(
        focused(cx, &far),
        "the premise: `far`, not `near`, is the tab stop before `going`"
    );
    keys(cx, "tab");
    assert!(focused(cx, &going), "the premise: back on `going`");

    shown.set(false);
    redraw(cx);
    assert!(
        cx.debug_bounds("going").is_none(),
        "the premise: the control is gone"
    );
    assert!(
        focused(cx, &near),
        "focus moved to the closest remaining control"
    );
    assert!(
        !focused(cx, &container),
        "not to the dialog itself, which only takes focus when no control is left"
    );
}

/// The same rule with a text field as the closest control: GPUI Kit tracks a field's focus itself, so the coordinator
/// knows where it is only because the field records its position. When the focused button beside the field vanishes,
/// typing lands in the field, not in the farther button.
#[gpui::test]
fn a_text_field_beside_a_vanishing_control_takes_focus(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let (far, going) = (handle(cx), handle(cx));
    let (_, shown, field) = open_placed_dialog(
        cx,
        vec![
            ("far", 10., PlacedKind::Button(far.clone())),
            ("field", 380., PlacedKind::Field),
            ("going", 480., PlacedKind::Button(going.clone())),
        ],
        &going,
    );
    assert!(
        focused(cx, &going),
        "the premise: the control about to vanish has focus"
    );
    let (far_at, field_at, going_at) =
        (center(cx, "far"), center(cx, "field"), center(cx, "going"));
    assert!(
        (going_at.x - field_at.x).abs() < (going_at.x - far_at.x).abs(),
        "the premise: the field is the closer one"
    );

    shown.set(false);
    redraw(cx);
    assert!(
        cx.debug_bounds("going").is_none(),
        "the premise: the control is gone"
    );
    assert!(!focused(cx, &far), "focus did not go to the farther button");
    keys(cx, "z");
    let text = {
        let state = field.borrow().clone();
        cx.update(|_, cx| state.map(|state| state.text(cx).to_string()))
    };
    assert_eq!(
        text.as_deref(),
        Some("z"),
        "the field took focus: typing landed in it"
    );
}

/// SPEC.md: "Every modal dialog keeps Tab and Shift+Tab within its visible, enabled controls." A disabled button is not
/// a tab stop, and a click on it does not focus it on a platform where a click focuses controls. In a question whose
/// confirming answer is disabled, Tab from Cancel stays on Cancel, so a following Enter cancels instead of landing on
/// a dead button.
#[gpui::test]
fn a_disabled_control_takes_no_focus(cx: &mut TestAppContext) {
    let handles = Rc::new(RefCell::new(Vec::new()));
    let drawn = handles.clone();
    let cx = open(cx, move |_, _| {
        let drawn = drawn.borrow();
        div()
            .flex()
            .gap(px(8.))
            .children(
                drawn
                    .iter()
                    .zip(["first", "off", "last"])
                    .map(|(focus, id)| button(id, focus).disabled(id == "off")),
            )
            .into_any_element()
    });
    let made: Vec<FocusHandle> = (0..3).map(|_| handle(cx)).collect();
    handles.replace(made.clone());
    redraw(cx);
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));

    keys(cx, "tab");
    assert!(
        focused(cx, &made[0]),
        "the premise: the first control has focus"
    );
    keys(cx, "tab");
    assert!(focused(cx, &made[2]), "Tab skipped the disabled control");
    click(cx, "off");
    assert!(
        focused(cx, &made[2]),
        "a click on the disabled control left focus where it was"
    );

    let sent = Rc::new(Cell::new(false));
    let record = sent.clone();
    let asked = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask_for_test(
            Presentation::Centered,
            question("alpha").confirm_enabled(false),
            None,
            &mark,
            move |_, _, _| record.set(true),
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    let asked = asked.expect("the question opened");
    keys(cx, "tab");
    keys(cx, "enter");
    assert!(!sent.get(), "nothing was confirmed");
    assert!(
        !is_open(cx, asked.layer),
        "Tab stayed on Cancel, so Enter answered Cancel and closed the question"
    );
}

/// The selected session's terminal is home: whenever nothing holds focus and nothing is open, focus goes there, but
/// home never takes focus from a control that holds it (SPEC.md, "Where focus rests"). A click on the page's empty
/// space leaves nothing focused, as on a web page, so focus goes home then too. There is no terminal yet, so a plain
/// focusable element tracked the way a terminal will be stands in for it.
#[gpui::test]
fn focus_goes_home_when_nothing_holds_it_and_never_takes_it_from_a_control(
    cx: &mut TestAppContext,
) {
    let state = Rc::new(RefCell::new(None::<(FocusHandle, FocusHandle)>));
    let control_shown = Rc::new(Cell::new(true));
    let (drawn, shown) = (state.clone(), control_shown.clone());
    let cx = open(cx, move |_, _| {
        let Some((home, control)) = drawn.borrow().clone() else {
            return div().into_any_element();
        };
        div()
            .flex()
            .gap(px(8.))
            .child(focus::track(
                div()
                    .relative()
                    .debug_selector(|| "home".into())
                    .size(px(40.)),
                &home,
                OnPress::Focus,
            ))
            .children(shown.get().then(|| button("control", &control)))
            .into_any_element()
    });
    let (home, control) = (handle(cx), handle(cx));
    state.replace(Some((home.clone(), control.clone())));
    redraw(cx);

    keys(cx, "tab");
    keys(cx, "tab");
    assert!(focused(cx, &control), "the premise: a control holds focus");
    cx.update(|window, cx| focus::set_home(Some(&home), window, cx));
    cx.run_until_parked();
    check(cx);
    assert!(
        focused(cx, &control),
        "setting home does not take focus from the control"
    );

    cx.simulate_click(point(px(600.), px(500.)), Modifiers::none());
    check(cx);
    assert!(
        focused(cx, &home),
        "a click on the page's empty space sent focus home"
    );
    keys(cx, "tab");
    assert!(
        focused(cx, &control),
        "the premise: Tab from home reached the control again"
    );

    control_shown.set(false);
    redraw(cx);
    assert!(
        focused(cx, &home),
        "the control went away and focus went home"
    );
}

/// A button whose own click removes it (the connection-lost banner's Dismiss) hands focus home in a frame of its own,
/// with nothing else redrawing. GPUI reports the lost focus while it finishes a frame and draws no new one for a focus
/// move made then, so without the coordinator asking for one the window kept drawing "nothing focused" until some
/// unrelated redraw: no focus events, and the test bridge reported no focus after Dismiss in the running app. The
/// focus-in event on home is what tells the drawn frame caught up from the focus merely being recorded.
#[gpui::test]
fn a_control_that_removes_itself_on_click_hands_focus_home_in_a_drawn_frame(
    cx: &mut TestAppContext,
) {
    let state = Rc::new(RefCell::new(None::<(FocusHandle, FocusHandle)>));
    let control_shown = Rc::new(Cell::new(true));
    let (drawn, shown) = (state.clone(), control_shown.clone());
    let cx = open(cx, move |_, _| {
        let Some((home, control)) = drawn.borrow().clone() else {
            return div().into_any_element();
        };
        let hide = shown.clone();
        div()
            .flex()
            .gap(px(8.))
            .child(focus::track(
                div()
                    .relative()
                    .debug_selector(|| "home".into())
                    .size(px(40.)),
                &home,
                OnPress::Focus,
            ))
            .children(shown.get().then(|| {
                button("dismiss", &control).on_click(move |_, window, _| {
                    hide.set(false);
                    window.refresh();
                })
            }))
            .into_any_element()
    });
    let (home, control) = (handle(cx), handle(cx));
    state.replace(Some((home.clone(), control.clone())));
    redraw(cx);
    cx.update(|window, cx| focus::set_home(Some(&home), window, cx));
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    redraw(cx);
    let entered = Rc::new(Cell::new(0));
    let counted = entered.clone();
    let _entered = cx.update(|window, cx| {
        window.on_focus_in(&home, cx, move |_, _| counted.set(counted.get() + 1))
    });
    keys(cx, "tab");
    assert!(focused(cx, &control), "the premise: the button holds focus");
    entered.set(0);

    click(cx, "dismiss");
    cx.run_until_parked();
    assert!(
        !control_shown.get(),
        "the premise: the click removed the button"
    );
    assert!(focused(cx, &home), "focus went home");
    assert_eq!(
        entered.get(),
        1,
        "home's focus-in arrived, so a frame was drawn with home focused"
    );
}

/// Why this matters: a dialog's empty-space rule runs after the controls under the press have handled it, and it used
/// to take focus for the dialog from a text field that had just focused itself for the click, so a click into the
/// Rename title left it unfocused and typing went nowhere. The rule now asks what the press did to focus
/// ([`OnPress::Focus`]), which covers every control, not only GPUI Kit's field, while a press on empty space still
/// focuses the dialog.
///
/// Specification: in a dialog without the rule that keeps focus on an empty-space click, with focus on a button, a
/// click into the field and then typing lands in the field, and so does a second click into the field that already
/// has focus. A click on empty space focuses the dialog. A click on a button leaves it focused where a click focuses
/// controls (Linux), and focuses the dialog where the button takes no focus (macOS, as in WebKit), even when the
/// button already had focus. A click on an element that focuses itself on a press keeps it focused whether or not
/// the coordinator knows where it is drawn.
#[gpui::test]
fn a_click_into_a_dialogs_text_field_leaves_the_field_focused(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let control = handle(cx);
    let pressable = cx.update(|_, cx| cx.focus_handle());
    let (container, _, field) = open_placed_dialog_with(
        cx,
        vec![
            ("control", 10., PlacedKind::Button(control.clone())),
            ("field", 200., PlacedKind::Field),
            ("pressable", 400., PlacedKind::Pressable(pressable.clone())),
        ],
        &control,
        false,
    );
    assert!(focused(cx, &control), "the premise: the button has focus");

    click(cx, "field");
    keys(cx, "a");
    assert_eq!(
        field_text(cx, &field).as_deref(),
        Some("a"),
        "the click focused the field and typing landed in it"
    );
    click(cx, "field");
    keys(cx, "b");
    assert_eq!(
        field_text(cx, &field).as_deref(),
        Some("ab"),
        "a second click into the focused field kept focus there"
    );

    let panel = bounds(cx, "panel");
    cx.simulate_click(
        point(panel.right() - px(4.), panel.bottom() - px(4.)),
        Modifiers::none(),
    );
    check(cx);
    assert!(
        focused(cx, &container),
        "a click on empty space still focused the dialog itself"
    );

    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    click(cx, "control");
    assert!(
        focused(cx, &control),
        "the Linux rule: the click focused the button, and the dialog left it there"
    );
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    click(cx, "control");
    assert!(
        focused(cx, &container),
        "the macOS rule: the button took no focus from the click, so the dialog did"
    );

    click(cx, "pressable");
    assert!(
        focused(cx, &pressable),
        "an element that focused itself on the press, with no position recorded, kept focus"
    );
}

// ---- Focus: choosing a terminal, and a tab's exit --------------------------------------------------------------------

/// One thing a terminal test draws: a terminal stand-in (tracked as a terminal is, so a press focuses it) or a button,
/// drawn while `shown` is set.
struct Drawn {
    id: &'static str,
    focus: FocusHandle,
    terminal: bool,
    shown: Rc<Cell<bool>>,
}

/// Open a window drawing `controls` (buttons) and then `terminals` (terminal stand-ins), in that order, which is also
/// their tab order. Nothing holds focus but the root, and no home is set.
fn open_terminals<'a>(
    cx: &'a mut TestAppContext,
    controls: &[&'static str],
    terminals: &[&'static str],
) -> (&'a mut VisualTestContext, Rc<RefCell<Vec<Drawn>>>) {
    let drawn = Rc::new(RefCell::new(Vec::<Drawn>::new()));
    let reading = drawn.clone();
    let cx = open(cx, move |_, _| {
        div()
            .flex()
            .gap(px(8.))
            .children(
                reading
                    .borrow()
                    .iter()
                    .filter(|item| item.shown.get())
                    .map(|item| {
                        let id = item.id;
                        if item.terminal {
                            focus::track(
                                div()
                                    .relative()
                                    .debug_selector(move || id.into())
                                    .size(px(40.)),
                                &item.focus,
                                OnPress::Focus,
                            )
                            .into_any_element()
                        } else {
                            button(id, &item.focus).into_any_element()
                        }
                    }),
            )
            .into_any_element()
    });
    let kinds = controls
        .iter()
        .map(|id| (*id, false))
        .chain(terminals.iter().map(|id| (*id, true)));
    let mut made = Vec::new();
    for (id, terminal) in kinds {
        made.push(Drawn {
            id,
            focus: handle(cx),
            terminal,
            shown: Rc::new(Cell::new(true)),
        });
    }
    drawn.replace(made);
    redraw(cx);
    (cx, drawn)
}

/// The focus handle and the shown flag of what [`open_terminals`] drew under `id`.
fn drawn(items: &RefCell<Vec<Drawn>>, id: &str) -> (FocusHandle, Rc<Cell<bool>>) {
    match items.borrow().iter().find(|item| item.id == id) {
        Some(item) => (item.focus.clone(), item.shown.clone()),
        None => panic!("nothing was drawn under {id}"),
    }
}

/// The user chooses the session whose terminal is `terminal`, now: the mark is taken in the same step, as a click or
/// key handler that chooses takes it.
fn choose_now(cx: &mut VisualTestContext, terminal: &FocusHandle) {
    let terminal = terminal.clone();
    cx.update(move |window, cx| {
        let mark = Mark::take(cx);
        focus::choose_terminal(&terminal, &mark, window, cx);
    });
    cx.run_until_parked();
    check(cx);
}

/// SPEC.md, "Where focus rests": "Choosing a session, including the one already selected, focuses its terminal." The
/// choice takes focus from whatever control held it (here the session's row), and the terminal is home from then on,
/// so a click on the page's empty space sends focus back to it. Without the public move, a view could only set home,
/// which never takes focus from a control.
#[gpui::test]
fn choosing_a_session_focuses_its_terminal_and_makes_it_home(cx: &mut TestAppContext) {
    let (cx, items) = open_terminals(cx, &["row"], &["terminal"]);
    let ((row, _), (terminal, _)) = (drawn(&items, "row"), drawn(&items, "terminal"));
    keys(cx, "tab");
    assert!(
        focused(cx, &row),
        "the premise: the session's row holds focus"
    );

    choose_now(cx, &terminal);
    assert!(
        focused(cx, &terminal),
        "choosing the session focused its terminal"
    );
    choose_now(cx, &terminal);
    assert!(
        focused(cx, &terminal),
        "choosing the session already selected keeps its terminal focused"
    );

    keys(cx, "shift-tab");
    assert!(
        focused(cx, &row),
        "the premise: Shift+Tab went back to the row"
    );
    cx.simulate_click(point(px(600.), px(500.)), Modifiers::none());
    check(cx);
    assert!(
        focused(cx, &terminal),
        "the terminal is home: a click on empty space sent focus there"
    );
}

/// A right or middle press on the page's empty space sends focus home, as a primary one does. GPUI focuses the window's
/// root on a press of any mouse button that reaches it, so with the empty-space rule on the primary button alone, a
/// right press on the list's title in the running app moved focus off the terminal to the root, and the keys typed next
/// reached nothing. A dialog's veil takes such a press before the page does, so focus stays in the dialog.
///
/// Specified, with the terminal home: with the terminal focused, a right and then a middle press on empty space leave
/// it focused; with a button focused, a right press on empty space sends focus to the terminal; and with a dialog
/// open, a right press on the page behind it leaves focus on the dialog's control.
#[gpui::test]
fn a_right_or_middle_press_on_the_pages_empty_space_sends_focus_home(cx: &mut TestAppContext) {
    let (cx, items) = open_terminals(cx, &["row"], &["terminal"]);
    let ((row, _), (terminal, _)) = (drawn(&items, "row"), drawn(&items, "terminal"));
    choose_now(cx, &terminal);
    assert!(
        focused(cx, &terminal),
        "the premise: the terminal is home and focused"
    );
    let empty = point(px(600.), px(500.));
    // Over the terminal stand-in, the press would be the terminal's own and test nothing of the page's rule.
    assert!(
        !bounds(cx, "terminal").contains(&empty) && !bounds(cx, "row").contains(&empty),
        "the premise: the press lands on empty space"
    );

    for button in [MouseButton::Right, MouseButton::Middle] {
        cx.simulate_mouse_down(empty, button, Modifiers::none());
        cx.simulate_mouse_up(empty, button, Modifiers::none());
        check(cx);
        assert!(
            focused(cx, &terminal),
            "a {button:?} press on empty space kept focus on the terminal"
        );
    }

    keys(cx, "shift-tab");
    assert!(focused(cx, &row), "the premise: Shift+Tab went to the row");
    cx.simulate_mouse_down(empty, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(empty, MouseButton::Right, Modifiers::none());
    check(cx);
    assert!(
        focused(cx, &terminal),
        "a right press on empty space sent focus home"
    );

    let only = handle(cx);
    open_dialog(cx, vec![("only", only.clone())], &only);
    assert!(
        focused(cx, &only),
        "the premise: the dialog's control holds focus"
    );
    cx.simulate_mouse_down(empty, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(empty, MouseButton::Right, Modifiers::none());
    check(cx);
    assert!(
        focused(cx, &only),
        "a right press behind the dialog left focus on its control"
    );
}

/// A press on a disabled control is that control's, though it takes no focus, so it is not a press on the page's empty
/// space and leaves focus where it was, whichever mouse button made it. The empty-space rule runs on every button, so a
/// disabled control that marked only its primary press as its own let a right press on it (the header's dimmed
/// Restart, say) move focus off the control in use, where a primary press on it moved nothing.
///
/// Specified: with an enabled button focused and a disabled one beside it, a right and then a middle press on the
/// disabled button leave the enabled one focused.
#[gpui::test]
fn a_right_or_middle_press_on_a_disabled_button_keeps_focus_where_it_was(cx: &mut TestAppContext) {
    let handles = Rc::new(RefCell::new(None::<(FocusHandle, FocusHandle)>));
    let drawn = handles.clone();
    let cx = open(cx, move |_, _| {
        let Some((on, off)) = drawn.borrow().clone() else {
            return div().into_any_element();
        };
        div()
            .flex()
            .gap(px(8.))
            .child(button("on", &on))
            .child(button("off", &off).disabled(true))
            .into_any_element()
    });
    let (on, off) = (handle(cx), handle(cx));
    handles.replace(Some((on.clone(), off)));
    redraw(cx);
    keys(cx, "tab");
    assert!(
        focused(cx, &on),
        "the premise: the enabled button holds focus"
    );

    for button in [MouseButton::Right, MouseButton::Middle] {
        press_with(cx, "off", button);
        assert!(
            focused(cx, &on),
            "a {button:?} press on the disabled button left focus where it was"
        );
    }
}

/// SPEC.md's settled case: "The user selects a session and, before its terminal finishes loading, types in the session
/// filter. The filter keeps focus when the terminal arrives." A choice that completes later carries the mark from when
/// the user chose; once they moved on, the terminal only becomes home, and takes focus when nothing else holds it.
#[gpui::test]
fn a_chosen_terminal_that_arrives_after_the_user_moved_on_only_becomes_home(
    cx: &mut TestAppContext,
) {
    let (cx, items) = open_terminals(cx, &["filter"], &["terminal"]);
    let ((filter, filter_shown), (terminal, _)) =
        (drawn(&items, "filter"), drawn(&items, "terminal"));
    let chosen = cx.update(|_, cx| Mark::take(cx));
    keys(cx, "tab");
    assert!(
        focused(cx, &filter),
        "the premise: the user went on into the filter"
    );
    assert!(
        cx.update(|_, cx| focus::has_moved_on(&chosen, cx)),
        "the premise: that counts as moving on"
    );

    let arrived = terminal.clone();
    cx.update(move |window, cx| focus::choose_terminal(&arrived, &chosen, window, cx));
    cx.run_until_parked();
    check(cx);
    assert!(
        focused(cx, &filter),
        "the filter kept focus when the terminal arrived"
    );

    filter_shown.set(false);
    redraw(cx);
    assert!(
        focused(cx, &terminal),
        "the terminal is home: once the filter went away, focus went there"
    );
}

/// A control the app returns focus to after the user's own action (the host rail's All once a filter clears, the
/// session header's Escape) must follow the same rules as choosing a session, without becoming home: it may take focus
/// only while the user has not moved on and no layer is open, or it would pull focus from where the user went next or
/// out from under a dialog, and a button that became home would take focus whenever nothing holds it. Specified: with
/// a terminal as home and a row focused, moving focus to a button takes it there, and a click on empty space then sends
/// focus home to the terminal, not to the button; with a mark taken before the user clicked the row again, it moves
/// nothing; under an open dialog it moves nothing.
#[gpui::test]
fn focus_control_moves_focus_unless_the_user_moved_on_or_a_layer_is_open(cx: &mut TestAppContext) {
    let (cx, items) = open_terminals(cx, &["row", "target"], &["terminal"]);
    let ((row, _), (target, _), (terminal, _)) = (
        drawn(&items, "row"),
        drawn(&items, "target"),
        drawn(&items, "terminal"),
    );
    let home = terminal.clone();
    cx.update(move |window, cx| focus::set_home(Some(&home), window, cx));
    let move_to = |cx: &mut VisualTestContext, mark: Mark| {
        let target = target.clone();
        cx.update(move |window, cx| focus::focus_control(&target, &mark, window, cx));
        cx.run_until_parked();
        check(cx);
    };

    click(cx, "row");
    assert!(focused(cx, &row), "the premise: the row holds focus");
    let now = cx.update(|_, cx| Mark::take(cx));
    move_to(cx, now);
    assert!(focused(cx, &target), "the move took focus to the button");
    cx.simulate_click(point(px(600.), px(500.)), Modifiers::none());
    check(cx);
    assert!(
        focused(cx, &terminal),
        "home is still the terminal: a click on empty space sent focus there, not to the button"
    );

    let earlier = cx.update(|_, cx| Mark::take(cx));
    click(cx, "row");
    assert!(
        focused(cx, &row) && cx.update(|_, cx| focus::has_moved_on(&earlier, cx)),
        "the premise: the user clicked the row, which counts as moving on"
    );
    move_to(cx, earlier);
    assert!(focused(cx, &row), "after the user moved on, nothing moved");

    let inside = handle(cx);
    open_dialog(cx, vec![("inside", inside.clone())], &inside);
    assert!(focused(cx, &inside), "the premise: the dialog holds focus");
    let now = cx.update(|_, cx| Mark::take(cx));
    move_to(cx, now);
    assert!(focused(cx, &inside), "under an open dialog, nothing moved");
}

/// Focus never leaves an open dialog for the page behind it (SPEC.md, "Opening and closing"), not even for a session
/// the user just chose. The terminal becomes home; closing the dialog returns focus to where it was before it opened,
/// which is why a dialog that chooses a session closes itself before choosing.
#[gpui::test]
fn choosing_a_terminal_while_a_dialog_is_open_leaves_focus_in_the_dialog(cx: &mut TestAppContext) {
    let (cx, items) = open_terminals(cx, &["opener"], &["terminal"]);
    let ((opener, opener_shown), (terminal, _)) =
        (drawn(&items, "opener"), drawn(&items, "terminal"));
    keys(cx, "tab");
    assert!(focused(cx, &opener), "the premise: the opener holds focus");
    let inside = handle(cx);
    let (dialog, _, _) = open_dialog(cx, vec![("inside", inside.clone())], &inside);
    assert!(focused(cx, &inside), "the premise: the dialog holds focus");

    choose_now(cx, &terminal);
    assert!(focused(cx, &inside), "focus stayed inside the dialog");

    keys(cx, "escape");
    assert!(
        !is_open(cx, dialog),
        "the premise: Escape closed the dialog"
    );
    assert!(
        focused(cx, &opener),
        "closing the dialog returned focus to its opener, not to the terminal"
    );
    opener_shown.set(false);
    redraw(cx);
    assert!(focused(cx, &terminal), "the chosen terminal became home");
}

/// Why this matters: SPEC.md, "Desktop window chrome", sends focus back to the terminal when the Rename editor opened
/// with its shortcut closes, and the shortcut may be pressed while a row reached with Tab holds focus, where returning
/// to the opener would land on the row. The dialog's opener names home instead; every other dialog keeps returning to
/// its opener, which the tests above cover.
///
/// Specification: with the terminal home and a row focused, a dialog opened returning home gives focus to the terminal
/// when it closes. Opened over another dialog, it gives focus back to that dialog, since focus never leaves an open
/// dialog for the page, and that dialog in turn returns it to the row.
#[gpui::test]
fn a_dialog_whose_opener_names_home_returns_focus_to_the_terminal(cx: &mut TestAppContext) {
    let (cx, items) = open_terminals(cx, &["row"], &["terminal"]);
    let ((row, _), (terminal, _)) = (drawn(&items, "row"), drawn(&items, "terminal"));
    choose_now(cx, &terminal);
    keys(cx, "shift-tab");
    assert!(
        focused(cx, &row),
        "the premise: a row reached with Tab holds focus"
    );

    let inside = handle(cx);
    open_dialog_returning(
        cx,
        vec![("inside", inside.clone())],
        &inside,
        focus::ReturnTo::Home,
    );
    keys(cx, "escape");
    assert!(
        focused(cx, &terminal),
        "closing it returned focus home, to the terminal"
    );

    keys(cx, "shift-tab");
    assert!(focused(cx, &row), "the premise: the row has focus again");
    let below = handle(cx);
    let (outer, _, _) = open_dialog(cx, vec![("below", below.clone())], &below);
    let above = handle(cx);
    open_dialog_returning(
        cx,
        vec![("above", above.clone())],
        &above,
        focus::ReturnTo::Home,
    );
    keys(cx, "escape");
    assert!(
        focused(cx, &below),
        "with a dialog still open below it, focus went back into that dialog"
    );
    keys(cx, "escape");
    assert!(!is_open(cx, outer), "the premise: the dialog below closed");
    assert!(
        focused(cx, &row),
        "and that dialog returned focus to its own opener, the row"
    );
}

/// Why this matters: a dialog opened to return home may close when there is no home (no session is selected), and
/// focus must then still land somewhere a key reaches, rather than on the window's root.
///
/// Specification: with no home set, a dialog opened returning home gives focus back to its opener when it closes.
#[gpui::test]
fn a_dialog_returning_home_falls_back_to_its_opener_without_a_home(cx: &mut TestAppContext) {
    let (cx, items) = open_terminals(cx, &["row"], &[]);
    let (row, _) = drawn(&items, "row");
    keys(cx, "tab");
    assert!(focused(cx, &row), "the premise: the row holds focus");
    let inside = handle(cx);
    open_dialog_returning(
        cx,
        vec![("inside", inside.clone())],
        &inside,
        focus::ReturnTo::Home,
    );
    keys(cx, "escape");
    assert!(
        focused(cx, &row),
        "with no home, closing it returned focus to its opener"
    );
}

/// A window with a row button, an element a menu can be anchored to, and a terminal stand-in below them, for the
/// menu-over-terminal tests. Returns the row's and the terminal's focus handles and the menu's anchor.
fn open_menu_over_terminal(
    cx: &mut TestAppContext,
) -> (&mut VisualTestContext, FocusHandle, FocusHandle, Anchor) {
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let row_slot: Rc<RefCell<Option<FocusHandle>>> = Rc::new(RefCell::new(None));
    let terminal_slot: Rc<RefCell<Option<FocusHandle>>> = Rc::new(RefCell::new(None));
    let (out, rows, terminals) = (anchor_out.clone(), row_slot.clone(), terminal_slot.clone());
    let cx = open(cx, move |window, cx| {
        let row = rows
            .borrow_mut()
            .get_or_insert_with(|| cx.focus_handle().tab_stop(true))
            .clone();
        let terminal = terminals
            .borrow_mut()
            .get_or_insert_with(|| cx.focus_handle().tab_stop(true))
            .clone();
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(button("row", &row))
            .child(anchor_element("opener", &out, window, cx))
            // Room for the menu, so that it never covers the terminal.
            .child(div().h(px(120.)))
            .child(focus::track(
                div()
                    .relative()
                    .debug_selector(|| "terminal".into())
                    .size(px(40.)),
                &terminal,
                OnPress::Focus,
            ))
            .into_any_element()
    });
    let row = row_slot.borrow().clone().unwrap();
    let terminal = terminal_slot.borrow().clone().unwrap();
    let anchor = anchor_out.borrow().clone().unwrap();
    (cx, row, terminal, anchor)
}

/// Why this matters: a click into the terminal types into it (SPEC.md, "Where focus rests"), also when it closes an
/// open menu. The menu's press-outside close returns focus to the menu's opener while the same press is still being
/// handled, before the terminal sees it; a terminal that counted any focus move during the press as a press taken by
/// something inside it left focus on the opener, where Space or Enter reopened the menu.
///
/// Specification, Linux click rule: with the row's menu open after a click on the row (so the row held focus), a click
/// into the terminal closes the menu and focuses the terminal.
#[gpui::test]
fn a_click_into_the_terminal_with_a_menu_open_focuses_it_under_the_linux_rule(
    cx: &mut TestAppContext,
) {
    let (cx, row, terminal, anchor) = open_menu_over_terminal(cx);
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    click(cx, "row");
    assert!(
        focused(cx, &row),
        "the premise: the clicked row holds focus"
    );
    let item = handle(cx);
    let menu = open_menu(cx, anchor, vec![("item", item.clone())]);
    assert!(focused(cx, &item), "the premise: the menu's item has focus");
    click(cx, "terminal");
    assert!(!is_open(cx, menu), "the press outside closed the menu");
    assert!(
        focused(cx, &terminal),
        "the click into the terminal focused the terminal"
    );
}

/// The macOS half of the test above: there a click does not focus the row, so the opener the menu returns focus to is
/// whatever held focus before (here the row, reached with Tab).
///
/// Specification, macOS click rule: with the row's menu open after Tab reached the row, a click into the terminal
/// closes the menu and focuses the terminal.
#[gpui::test]
fn a_click_into_the_terminal_with_a_menu_open_focuses_it_under_the_macos_rule(
    cx: &mut TestAppContext,
) {
    let (cx, row, terminal, anchor) = open_menu_over_terminal(cx);
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    keys(cx, "tab");
    assert!(
        focused(cx, &row),
        "the premise: the row reached with Tab holds focus"
    );
    let item = handle(cx);
    let menu = open_menu(cx, anchor, vec![("item", item.clone())]);
    assert!(focused(cx, &item), "the premise: the menu's item has focus");
    click(cx, "terminal");
    assert!(!is_open(cx, menu), "the press outside closed the menu");
    assert!(
        focused(cx, &terminal),
        "the click into the terminal focused the terminal"
    );
}

// ---- Which controls Tab reaches -------------------------------------------------------------------------------------

/// What the placed dialog's text field holds: typing after a focus step shows whether the field (and not the dialog
/// itself, the other place focus can land) took the step.
fn field_text(cx: &mut VisualTestContext, field: &FieldState) -> Option<String> {
    let state = field.borrow().clone();
    cx.update(|_, cx| state.map(|state| state.text(cx).to_string()))
}

/// On macOS with Keyboard navigation off (the system default), Tab reaches text fields and pop-ups only,
/// skipping buttons, and Option-Tab reaches every control; with it on, and on Linux, Tab reaches every control. A
/// keyboard-only Mac user who never turned the setting on must still get through a form with Tab, and one who turned it
/// on must reach the buttons; getting either wrong is what this guards.
#[gpui::test]
fn tab_skips_buttons_unless_every_control_is_reached_and_option_tab_reaches_them(
    cx: &mut TestAppContext,
) {
    let cx = open(cx, |_, _| div().into_any_element());
    let reach = Rc::new(Cell::new(focus::TabReach::TextFieldsAndPopups));
    follow(cx, &reach);
    let (first, last) = (handle(cx), handle(cx));
    let (_, _, field) = open_placed_dialog(
        cx,
        vec![
            ("first", 10., PlacedKind::Button(first.clone())),
            ("field", 200., PlacedKind::Field),
            ("last", 400., PlacedKind::Button(last.clone())),
        ],
        &first,
    );
    assert!(
        focused(cx, &first),
        "the premise: the first button has focus"
    );

    keys(cx, "tab");
    keys(cx, "q");
    assert_eq!(
        field_text(cx, &field).as_deref(),
        Some("q"),
        "Tab left the button for the field: typing landed in it"
    );

    keys(cx, "tab");
    keys(cx, "r");
    assert_eq!(
        field_text(cx, &field).as_deref(),
        Some("qr"),
        "with only one field to reach, Tab stayed on it and skipped both buttons"
    );

    keys(cx, "alt-tab");
    assert!(focused(cx, &last), "Option-Tab reached the next button");
    keys(cx, "alt-shift-tab");
    keys(cx, "s");
    assert_eq!(
        field_text(cx, &field).as_deref(),
        Some("qrs"),
        "Option-Shift-Tab went back to the field"
    );
    keys(cx, "alt-shift-tab");
    assert!(
        focused(cx, &first),
        "Option-Shift-Tab from the field reached the button before it"
    );

    keys(cx, "alt-tab");
    reach.set(focus::TabReach::EveryControl);
    keys(cx, "tab");
    assert!(
        focused(cx, &last),
        "with every control reached, Tab reached the button after the field"
    );
}

/// In a dialog of buttons only, with Tab reaching text fields only, Tab finds nothing to move to and focus stays where
/// it was. Moving it to the dialog itself instead would let a following Enter press the dialog's main action from a
/// Cancel the user had chosen.
#[gpui::test]
fn tab_in_a_dialog_of_buttons_stays_put_where_tab_skips_buttons(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let reach = Rc::new(Cell::new(focus::TabReach::TextFieldsAndPopups));
    follow(cx, &reach);
    let (cancel, confirm) = (handle(cx), handle(cx));
    open_dialog(
        cx,
        vec![("cancel", cancel.clone()), ("confirm", confirm.clone())],
        &cancel,
    );
    assert!(focused(cx, &cancel), "the premise: Cancel has focus");

    keys(cx, "tab");
    assert!(
        focused(cx, &cancel),
        "Tab found no field and left focus on Cancel"
    );
    keys(cx, "shift-tab");
    assert!(focused(cx, &cancel), "Shift+Tab too");
    keys(cx, "alt-tab");
    assert!(focused(cx, &confirm), "Option-Tab reached the other button");
}

/// The Keyboard navigation setting can change while the app runs, without the app ever losing focus (macOS flips it
/// with Control-F7), and the old app's WebKit followed it at once. The coordinator reads the setting on every Tab
/// press, so the next press after a change follows it, with no reread step in between.
#[gpui::test]
fn the_tab_setting_is_read_on_every_press(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let reach = Rc::new(Cell::new(focus::TabReach::TextFieldsAndPopups));
    follow(cx, &reach);
    let (cancel, confirm) = (handle(cx), handle(cx));
    open_dialog(
        cx,
        vec![("cancel", cancel.clone()), ("confirm", confirm.clone())],
        &cancel,
    );
    keys(cx, "tab");
    assert!(
        focused(cx, &cancel),
        "the premise: with Tab skipping buttons, Tab found nothing to move to"
    );

    reach.set(focus::TabReach::EveryControl);
    keys(cx, "tab");
    assert!(
        focused(cx, &confirm),
        "the next press after the setting changed reached the button"
    );
}

/// Whether a click focuses a button is the coordinator's platform rule: it does on Linux, not on macOS. Both rules
/// are applied here, whatever platform runs the suite, so the macOS rule (which also proves GPUI's own focus-on-press
/// is suppressed) is tested on Linux too. A click activates the button either way.
#[gpui::test]
fn a_click_focuses_a_button_only_where_the_platform_rule_says(cx: &mut TestAppContext) {
    let presses = Rc::new(Cell::new(0));
    let target = Rc::new(RefCell::new(None));
    let (counted, drawn) = (presses.clone(), target.clone());
    let cx = open(cx, move |_, _| {
        let counted = counted.clone();
        div()
            .children(drawn.borrow().clone().map(|focus| {
                button("press", &focus).on_click(move |_, _, _| counted.set(counted.get() + 1))
            }))
            .into_any_element()
    });
    let press = handle(cx);
    target.replace(Some(press.clone()));
    redraw(cx);

    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    click(cx, "press");
    assert!(
        !focused(cx, &press),
        "the macOS rule: a click does not focus it"
    );
    assert_eq!(presses.get(), 1, "the click activated it");

    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    click(cx, "press");
    assert!(focused(cx, &press), "the Linux rule: a click focuses it");
    assert_eq!(presses.get(), 2);
}

/// Press and release `button` at the center of what is drawn under `selector`, and check.
fn press_with(cx: &mut VisualTestContext, selector: &'static str, button: MouseButton) {
    let at = center(cx, selector);
    cx.simulate_mouse_down(at, button, Modifiers::none());
    cx.simulate_mouse_up(at, button, Modifiers::none());
    check(cx);
}

/// A terminal takes focus from a press of any button, as the old one did on every mousedown, while a button takes it
/// from a primary press only. The terminal half matters because under mouse reporting a right or middle click into a
/// full-screen program is the program's, and the keys typed next must reach it too, not the control that held focus.
/// The button half is the old app's rule (its buttons took no focus from a right click), and also proves GPUI's own
/// focus-on-press is suppressed for every button: it ignores which button went down, and once focused a button on a
/// right press under either platform rule. The Linux click rule is applied, where a primary press does focus a button,
/// so the right press not focusing it is the button rule and not the macOS one.
#[gpui::test]
fn a_right_or_middle_press_focuses_a_terminal_but_not_a_button(cx: &mut TestAppContext) {
    let (cx, items) = open_terminals(cx, &["button"], &["terminal"]);
    let ((button, _), (terminal, _)) = (drawn(&items, "button"), drawn(&items, "terminal"));
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    assert!(
        !focused(cx, &button) && !focused(cx, &terminal),
        "the premise: only the root holds focus"
    );

    press_with(cx, "button", MouseButton::Right);
    assert!(
        !focused(cx, &button),
        "a right press does not focus a button"
    );
    press_with(cx, "terminal", MouseButton::Right);
    assert!(focused(cx, &terminal), "a right press focuses the terminal");
    press_with(cx, "button", MouseButton::Right);
    assert!(
        focused(cx, &terminal),
        "a right press on a button leaves focus where it was"
    );

    click(cx, "button");
    assert!(
        focused(cx, &button),
        "a primary press still focuses a button under the Linux rule"
    );
    press_with(cx, "terminal", MouseButton::Middle);
    assert!(
        focused(cx, &terminal),
        "a middle press focuses the terminal"
    );
    click(cx, "button");
    click(cx, "terminal");
    assert!(
        focused(cx, &terminal),
        "a primary press still focuses the terminal"
    );
}

/// A right press on a button inside a dialog that takes focus from its empty space passes through to the dialog, as a
/// primary press on a macOS button does: no button but the primary one focuses a control, so the press is not the
/// button's. Without this the outcome would hang on whether the button already held focus (a right press on the focused
/// one would count as the button keeping focus, on any other as the dialog's), under the Linux rule where a primary
/// press is the button's.
///
/// Specification: in such a dialog under the Linux click rule, with focus on a button, a right press on that button
/// focuses the dialog.
#[gpui::test]
fn a_right_press_on_a_focused_button_in_a_dialog_focuses_the_dialog(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let control = handle(cx);
    let (container, _, _) = open_placed_dialog_with(
        cx,
        vec![("control", 10., PlacedKind::Button(control.clone()))],
        &control,
        false,
    );
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    assert!(focused(cx, &control), "the premise: the button has focus");

    press_with(cx, "control", MouseButton::Right);
    assert!(
        focused(cx, &container),
        "the right press was not the button's, so the dialog took focus"
    );
}

/// Enter and Space activate a focused button, but a held key does not repeat it: SPEC.md, "Repeated or composing Enter
/// cannot answer the new question". The ring shows only after keyboard input.
#[gpui::test]
fn enter_and_space_activate_a_focused_button_and_a_held_key_does_not(cx: &mut TestAppContext) {
    let presses = Rc::new(Cell::new(0));
    let target = Rc::new(RefCell::new(None));
    let (counted, drawn) = (presses.clone(), target.clone());
    let cx = open(cx, move |_, _| {
        let counted = counted.clone();
        div()
            .children(drawn.borrow().clone().map(|focus| {
                button("act", &focus).on_click(move |_, _, _| counted.set(counted.get() + 1))
            }))
            .into_any_element()
    });
    let act = handle(cx);
    target.replace(Some(act.clone()));
    redraw(cx);
    keys(cx, "tab");
    assert!(focused(cx, &act), "the premise: the button has focus");
    assert!(
        cx.update(|window, _| focus::shows_ring(&act, window)),
        "keyboard focus shows the ring"
    );

    keys(cx, "enter");
    keys(cx, "space");
    assert_eq!(presses.get(), 2, "Enter and Space each activated it once");
    cx.simulate_event(KeyDownEvent {
        native_key: None,
        keystroke: Keystroke::parse("enter").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    check(cx);
    assert_eq!(presses.get(), 2, "a held Enter did not");
}

// ---- "The user moved on" ---------------------------------------------------------------------------------------------

/// The moved-on counter is what lets work that finishes later decide whether it may move focus. A key press or click
/// counts; a held key, the same key again on the same control, a double click's second click, scrolling and pointer
/// movement do not (SPEC.md, "Who may move focus": "any key press or any click inside Farhelm").
///
/// "Any key press" includes the keys a binding handles, which GPUI never shows an ordinary key listener: Farhelm's own
/// Tab, Shift+Tab and Escape, and the editing keys GPUI Kit's text input binds (Backspace, the arrows). A user who
/// Tabs on, closes a dialog or edits a field after starting a launch has moved on, and the launch's result must not
/// take focus from them.
#[gpui::test]
fn what_counts_as_the_user_moving_on(cx: &mut TestAppContext) {
    let input: Rc<RefCell<Option<TextInputState>>> = Rc::new(RefCell::new(None));
    let page = Rc::new(RefCell::new(None::<FocusHandle>));
    let (drawn_input, drawn_page) = (input.clone(), page.clone());
    let cx = open(cx, move |window, cx| {
        let state = drawn_input
            .borrow_mut()
            .get_or_insert_with(|| TextInputState::new(window, cx))
            .clone();
        div()
            .size(px(300.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .w(px(200.))
                    .debug_selector(|| "field".into())
                    .child(TextInput::new(
                        "field",
                        &state,
                        "Field",
                        HoverHelp::tooltip("A test field"),
                    )),
            )
            .children(
                drawn_page
                    .borrow()
                    .clone()
                    .map(|focus| button("control", &focus)),
            )
            .into_any_element()
    });
    let control = handle(cx);
    page.replace(Some(control.clone()));
    redraw(cx);
    let moved =
        |cx: &mut VisualTestContext, mark: &Mark| cx.update(|_, cx| focus::has_moved_on(mark, cx));
    let mark = |cx: &mut VisualTestContext| cx.update(|_, cx| Mark::take(cx));

    let before = mark(cx);
    cx.simulate_mouse_move(point(px(50.), px(50.)), None, Modifiers::none());
    cx.simulate_event(ScrollEvent {
        position: point(px(50.), px(50.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-20.))),
        ..Default::default()
    });
    assert!(
        !moved(cx, &before),
        "pointer movement and scrolling are not moving on"
    );

    keys(cx, "a");
    assert!(moved(cx, &before), "a key press is");
    let after_key = mark(cx);
    keys(cx, "a");
    assert!(
        !moved(cx, &after_key),
        "the same key again on the same control is a repeat"
    );
    cx.simulate_event(KeyDownEvent {
        native_key: None,
        keystroke: Keystroke::parse("b").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    assert!(!moved(cx, &after_key), "a held key is a repeat");

    let before_click = mark(cx);
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(50.), px(50.)),
        click_count: 2,
        ..Default::default()
    });
    assert!(
        !moved(cx, &before_click),
        "a double click's second click is a repeat"
    );
    cx.simulate_click(point(px(50.), px(50.)), Modifiers::none());
    assert!(moved(cx, &before_click), "a click is moving on");

    // Keys Farhelm's own bindings handle.
    let before_tab = mark(cx);
    let focus_before_tab = cx.update(|window, cx| window.focused(cx));
    keys(cx, "tab");
    assert_ne!(
        cx.update(|window, cx| window.focused(cx)),
        focus_before_tab,
        "the premise: Tab moved focus"
    );
    assert!(moved(cx, &before_tab), "Tab is moving on");
    let before_back = mark(cx);
    keys(cx, "shift-tab");
    assert!(moved(cx, &before_back), "Shift+Tab is moving on");
    let only = handle(cx);
    let (dialog, _, _) = open_dialog(cx, vec![("only", only.clone())], &only);
    let before_escape = mark(cx);
    keys(cx, "escape");
    assert!(
        !is_open(cx, dialog),
        "the premise: Escape closed the dialog"
    );
    assert!(moved(cx, &before_escape), "Escape is moving on");

    // Keys GPUI Kit's text input binds for itself.
    click(cx, "field");
    keys(cx, "x y");
    let text = |cx: &mut VisualTestContext| {
        let state = input.borrow().clone();
        cx.update(|_, cx| state.map(|state| state.text(cx).to_string()))
    };
    assert_eq!(
        text(cx).as_deref(),
        Some("xy"),
        "the premise: the field took the typing"
    );
    let before_edit = mark(cx);
    keys(cx, "backspace");
    assert_eq!(
        text(cx).as_deref(),
        Some("x"),
        "the premise: Backspace edited the field"
    );
    assert!(
        moved(cx, &before_edit),
        "Backspace in a text field is moving on"
    );
    let before_arrow = mark(cx);
    keys(cx, "left");
    assert!(
        moved(cx, &before_arrow),
        "an arrow key in a text field is moving on"
    );
}

// ---- Repeats of a key press that closed a layer ----------------------------------------------------------------------

/// Every key-down that reached [`open_recording_terminal`]'s terminal stand-in, in order: its key, with " held" after
/// a held repeat.
type Typed = Rc<RefCell<Vec<String>>>;

/// Open a window drawing one terminal stand-in (tracked as a terminal is) that records every key-down reaching it, and
/// choose it, so it holds focus and is home. A layer opened now returns focus to it when it closes, as the launcher
/// returns focus to the terminal it was opened from.
fn open_recording_terminal(
    cx: &mut TestAppContext,
) -> (&mut VisualTestContext, FocusHandle, Typed) {
    let slot = Rc::new(RefCell::new(None::<FocusHandle>));
    let typed: Typed = Rc::default();
    let (drawn, record) = (slot.clone(), typed.clone());
    let cx = open(cx, move |_, _| {
        let record = record.clone();
        div()
            .children(drawn.borrow().clone().map(|terminal| {
                focus::track(
                    div()
                        .debug_selector(|| "terminal".into())
                        .size(px(40.))
                        .on_key_down(move |event: &KeyDownEvent, _, _| {
                            let held = if event.is_held { " held" } else { "" };
                            record
                                .borrow_mut()
                                .push(format!("{}{held}", event.keystroke.key));
                        }),
                    &terminal,
                    OnPress::Focus,
                )
            }))
            .into_any_element()
    });
    let terminal = handle(cx);
    slot.replace(Some(terminal.clone()));
    redraw(cx);
    choose_now(cx, &terminal);
    assert!(
        focused(cx, &terminal),
        "the premise: the terminal holds focus"
    );
    (cx, terminal, typed)
}

/// Send an auto-repeat of `key`, held down since its last press: a key-down with `is_held` set, spelled as a press
/// of the key is ([`keys`] adds the text a key types), and check.
fn held(cx: &mut VisualTestContext, key: &'static str) {
    let keystroke = match Keystroke::parse(key) {
        Ok(keystroke) => keystroke.with_simulated_ime(),
        Err(error) => panic!("{key} is not a keystroke: {error}"),
    };
    cx.simulate_event(KeyDownEvent {
        native_key: None,
        keystroke,
        is_held: true,
        prefer_character_input: false,
    });
    check(cx);
}

/// Whether the user has moved on since `mark`.
fn moved_on(cx: &mut VisualTestContext, mark: &Mark) -> bool {
    cx.update(|_, cx| focus::has_moved_on(mark, cx))
}

/// Ask a question over the page and answer it with Enter, which closes it and returns focus to what held it before.
/// Returns the mark taken right after the answer, as work that the answer starts takes its mark (a launch).
fn answer_with_enter(cx: &mut VisualTestContext) -> Mark {
    let layer = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask(question("alpha"), None, &mark, |_, _, _| {}, window, cx)
    });
    settle(cx);
    let layer = layer.expect("the question opened");
    keys(cx, "enter");
    assert!(
        !is_open(cx, layer),
        "the premise: Enter answered the question and closed it"
    );
    cx.update(|_, cx| Mark::take(cx))
}

/// Why this matters: SPEC.md ("Creation", "Who may move focus") says an accidental repeated Enter on the launcher's own
/// submit does not count as moving on. Submitting closes the launcher's dialog and returns focus to its opener, often
/// the terminal, so without this the second Enter counted (the launch would not open its session) and reached the
/// terminal's program as a second Enter. The old app hid the launcher and left focus on the page's body, where the
/// second press went nowhere and, within its 500 ms submit-repeat interval, did not count (`nonblocking-launch.spec.ts`
/// at `24bc0580`: "New opens after a repeated Enter submit retargets to the released page").
///
/// Specified: once Enter closes a layer and focus is back on the terminal, a held repeat of that Enter, a fresh Enter
/// at once, a fresh Enter exactly 500 ms after it (the old app ended a repeat only once more than 500 ms had passed),
/// and a held repeat after that are neither counted (a mark taken after the answer still holds) nor delivered to the
/// terminal. A fresh Enter 501 ms after the closing one reaches the terminal and counts as usual, and so do its own
/// held repeats.
#[gpui::test]
fn a_repeat_of_the_enter_that_closed_a_layer_neither_counts_nor_reaches_the_terminal(
    cx: &mut TestAppContext,
) {
    let (cx, terminal, typed) = open_recording_terminal(cx);
    let answered = answer_with_enter(cx);
    assert!(
        focused(cx, &terminal),
        "the premise: closing the question returned focus to the terminal"
    );
    assert!(
        typed.borrow().is_empty(),
        "the premise: the answering Enter itself reached only the question"
    );

    held(cx, "enter");
    keys(cx, "enter");
    advance(cx, Duration::from_millis(500));
    keys(cx, "enter");
    held(cx, "enter");
    assert!(
        !moved_on(cx, &answered),
        "no repeat of the closing Enter counted as moving on"
    );
    assert!(
        typed.borrow().is_empty(),
        "no repeat of the closing Enter reached the terminal: {:?}",
        typed.borrow()
    );

    advance(cx, Duration::from_millis(1));
    keys(cx, "enter");
    held(cx, "enter");
    assert_eq!(
        *typed.borrow(),
        ["enter", "enter held"],
        "a fresh Enter after the interval, and its own repeat, reached the terminal"
    );
    assert!(moved_on(cx, &answered), "and that Enter counted");
}

/// Why this matters: only the closing key's own repeats are swallowed, so typing goes on as usual the moment a layer
/// closes. A different key is a new gesture, as in the old app, where any other press ended the submit's repeat.
///
/// Specified: after Enter closes a layer, a different key pressed at once reaches the terminal and counts as moving
/// on, and an Enter pressed after it reaches the terminal too, although it falls within 500 ms of the closing Enter.
#[gpui::test]
fn a_different_key_after_the_enter_that_closed_a_layer_is_delivered_and_ends_its_repeats(
    cx: &mut TestAppContext,
) {
    let (cx, _, typed) = open_recording_terminal(cx);
    let answered = answer_with_enter(cx);

    keys(cx, "a");
    assert_eq!(*typed.borrow(), ["a"], "the other key reached the terminal");
    assert!(moved_on(cx, &answered), "and counted as moving on");
    keys(cx, "enter");
    assert_eq!(
        *typed.borrow(),
        ["a", "enter"],
        "an Enter after the other key is no longer a repeat of the closing one"
    );
}

/// Why this matters: the rule covers every layer a key closes, and Escape closes the most. A held Escape must close one
/// layer, not every layer below it and then reach the terminal's program as Escapes (SPEC.md: Escape closes the
/// topmost layer). But Escape pressed twice in quick succession is how a user backs out of two layers, so a dismissal
/// keeps its fresh presses.
///
/// Specified: with a dialog open over another over the terminal, Escape closes the top one, and a held repeat of it
/// leaves the lower one open; a fresh Escape at once closes that one, returning focus to the terminal, and a held
/// repeat of it does not reach the terminal, while a fresh Escape at once does (a dismissal keeps only its held
/// repeats, even when it closes the last layer).
#[gpui::test]
fn a_held_escape_closes_one_layer_and_a_second_press_the_next(cx: &mut TestAppContext) {
    let (cx, terminal, typed) = open_recording_terminal(cx);
    let (outer, inner) = (handle(cx), handle(cx));
    let (lower, _, _) = open_dialog(cx, vec![("outer", outer.clone())], &outer);
    let (upper, _, _) = open_dialog(cx, vec![("inner", inner.clone())], &inner);

    keys(cx, "escape");
    assert!(
        !is_open(cx, upper) && is_open(cx, lower),
        "the premise: Escape closed the top dialog only"
    );
    held(cx, "escape");
    assert!(
        is_open(cx, lower),
        "a held Escape did not close the dialog below"
    );
    assert!(focused(cx, &outer), "focus stayed in the dialog below");

    keys(cx, "escape");
    assert!(
        !is_open(cx, lower),
        "a second press of Escape closed the dialog below"
    );
    assert!(
        focused(cx, &terminal),
        "and focus went back to the terminal"
    );
    held(cx, "escape");
    assert!(
        typed.borrow().is_empty(),
        "no held Escape reached the terminal: {:?}",
        typed.borrow()
    );
    keys(cx, "escape");
    assert_eq!(
        *typed.borrow(),
        ["escape"],
        "a fresh Escape after a dismissal reached the terminal, although within 500 ms"
    );
}

/// Why this matters: a key that closes one layer often opens the next, as a row's menu item opens its question, and
/// that layer starts on the answer the user asked for, so a new Enter goes ahead (SPEC.md, "Opening and closing": from
/// a row's menu, Delete then Enter deletes). Swallowing that Enter as a repeat would undo the start rule; a held one
/// still must not answer ("Repeated or composing Enter cannot answer the new question").
///
/// Specified: Enter closes a question and another opens at once; a held repeat of that Enter leaves the new question
/// unanswered, and a fresh Enter at once, within 500 ms of the first, answers it.
#[gpui::test]
fn a_fresh_enter_answers_a_layer_that_opened_after_the_enter_that_closed_one(
    cx: &mut TestAppContext,
) {
    let (cx, _, typed) = open_recording_terminal(cx);
    answer_with_enter(cx);
    let answered = Rc::new(Cell::new(false));
    let record = answered.clone();
    let next = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask(
            question("beta"),
            None,
            &mark,
            move |_, _, _| record.set(true),
            window,
            cx,
        )
    });
    settle(cx);
    let next = next.expect("the premise: the next question opened");

    held(cx, "enter");
    assert!(
        is_open(cx, next) && !answered.get(),
        "a held Enter did not answer the new question"
    );
    keys(cx, "enter");
    assert!(
        answered.get() && !is_open(cx, next),
        "a fresh Enter answered it"
    );
    assert!(typed.borrow().is_empty(), "no Enter reached the terminal");
}

/// A dialog's content standing in for the launcher's form: a select whose closed control runs the dialog's main action
/// on Enter (as the launcher's selects launch), and a button. Each counts what reached it.
struct Composer {
    pick: FocusHandle,
    act: FocusHandle,
    chosen: Rc<RefCell<Option<gpui::SharedString>>>,
    launched: Rc<Cell<usize>>,
    acted: Rc<Cell<usize>>,
}

impl Render for Composer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (chosen, launched, acted) = (
            self.chosen.clone(),
            self.launched.clone(),
            self.acted.clone(),
        );
        let help = |words: &'static str| HoverHelp::tooltip(words);
        let select = Select::new("pick", "Pick", help("Pick one"), gpui::Role::ComboBox)
            .options([
                SelectOption::new("a", "A", help("The first")),
                SelectOption::new("b", "B", help("The second")),
            ])
            .on_choose(move |value, _, _| {
                chosen.replace(Some(value.clone()));
            })
            .on_enter(move |_, _| launched.set(launched.get() + 1))
            .focus(&self.pick);
        let select = match self.chosen.borrow().clone() {
            Some(value) => select.chosen(value),
            None => select,
        };
        div()
            .w(px(300.))
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(select)
            .child(button("act", &self.act).on_click(move |_, _, _| acted.set(acted.get() + 1)))
    }
}

/// What a test reads back from [`open_composer`].
struct Composed {
    pick: FocusHandle,
    act: FocusHandle,
    chosen: Rc<RefCell<Option<gpui::SharedString>>>,
    launched: Rc<Cell<usize>>,
    acted: Rc<Cell<usize>>,
}

/// Open a [`Composer`] dialog over the page, starting on its select, for a mark taken now.
fn open_composer(cx: &mut VisualTestContext) -> Composed {
    let composed = Composed {
        pick: handle(cx),
        act: handle(cx),
        chosen: Rc::default(),
        launched: Rc::default(),
        acted: Rc::default(),
    };
    let view = Composer {
        pick: composed.pick.clone(),
        act: composed.act.clone(),
        chosen: composed.chosen.clone(),
        launched: composed.launched.clone(),
        acted: composed.acted.clone(),
    };
    let (container, start) = (handle(cx), composed.pick.clone());
    let opened = cx.update(move |window, cx| {
        let view = cx.new(|_| view);
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container,
                start,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: true,
                returns_to: focus::ReturnTo::Opener,
                on_dismiss: None,
                placement: layers::DialogPlacement::Centered,
                dismiss_on_outside_press: false,
            },
            &mark,
            window,
            cx,
        )
    });
    settle(cx);
    assert!(opened.is_some(), "the premise: the dialog opened");
    composed
}

/// Why this matters: inside the launcher, Enter chooses in a select's list, which closes the list and returns focus to
/// the select, and Enter on the closed select launches (SPEC.md, "Creation"). Typed at normal speed, that second Enter
/// falls within 500 ms of the first; swallowing it would lose a deliberate launch without a sign. Choosing is not a
/// submit, and the old app's native select delivered that Return to the form. The same goes for a question answered over
/// a dialog: focus returns to a control the user is still working in.
///
/// Specified: in a dialog, Space opens a select's list and Enter chooses and closes it; a fresh Enter at once reaches
/// the select's main action. Then, with focus on a button of the dialog, a question asked over it is answered with
/// Enter, and a fresh Enter at once presses the button.
#[gpui::test]
fn a_fresh_enter_after_a_close_that_leaves_a_dialog_open_reaches_the_dialog(
    cx: &mut TestAppContext,
) {
    let (cx, _, typed) = open_recording_terminal(cx);
    let composed = open_composer(cx);
    assert!(
        focused(cx, &composed.pick),
        "the premise: the dialog starts on its select"
    );

    keys(cx, "space");
    assert!(
        !focused(cx, &composed.pick),
        "the premise: Space opened the list, which took focus"
    );
    keys(cx, "enter");
    assert_eq!(
        composed.chosen.borrow().as_deref(),
        Some("a"),
        "the premise: Enter chose the first option"
    );
    assert!(
        focused(cx, &composed.pick),
        "the premise: the list closed and focus went back to the select"
    );
    keys(cx, "enter");
    assert_eq!(
        composed.launched.get(),
        1,
        "the Enter right after choosing ran the main action"
    );

    keys(cx, "tab");
    assert!(
        focused(cx, &composed.act),
        "the premise: Tab reached the button"
    );
    let layer = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask(question("alpha"), None, &mark, |_, _, _| {}, window, cx)
    });
    settle(cx);
    let layer = layer.expect("the premise: the question opened");
    keys(cx, "enter");
    assert!(
        !is_open(cx, layer) && focused(cx, &composed.act),
        "the premise: Enter answered the question and focus went back to the button"
    );
    keys(cx, "enter");
    assert_eq!(
        composed.acted.get(),
        1,
        "the Enter right after the answer pressed the button"
    );
    assert!(typed.borrow().is_empty(), "nothing reached the terminal");
}

/// Why this matters: a click is a new gesture, so it ends a closing press's repeats ("Any other fresh key press or a
/// click ends it"), and a layer that a click closes was not closed by a key, so the key pressed before the click must
/// not be taken as the closing press.
///
/// Specified: after Enter closes a layer, a click on the terminal and then Enter at once delivers that Enter. With a
/// question open, a key pressed in it and then a click on its Cancel close it; the same key pressed at once reaches the
/// terminal.
#[gpui::test]
fn a_click_ends_a_closing_press_and_a_layer_a_click_closes_swallows_nothing(
    cx: &mut TestAppContext,
) {
    let (cx, terminal, typed) = open_recording_terminal(cx);
    answer_with_enter(cx);
    click(cx, "terminal");
    keys(cx, "enter");
    assert_eq!(
        *typed.borrow(),
        ["enter"],
        "the Enter after the click reached the terminal"
    );

    let layer = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask(question("alpha"), None, &mark, |_, _, _| {}, window, cx)
    });
    settle(cx);
    let layer = layer.expect("the premise: the question opened");
    keys(cx, "x");
    click(cx, "prompt-cancel");
    assert!(
        !is_open(cx, layer) && focused(cx, &terminal),
        "the premise: the click closed the question and focus went back to the terminal"
    );
    keys(cx, "x");
    assert_eq!(
        *typed.borrow(),
        ["enter", "x"],
        "the key after the click reached the terminal"
    );
}

/// Why this matters: a layer that closes on its own (a reply, its subject going) is taken as closed by the last key
/// press, because GPUI says nothing about when a key's handling ends. The coordinator's docs bound what that costs: the
/// window runs from the press, so it swallows only a key still held from that press. This pins the bound.
///
/// Specified: a key pressed in a dialog over the terminal, then the dialog closed by its opener's code 501 ms later; a
/// held repeat of that key does not reach the terminal, and a fresh press of it does.
#[gpui::test]
fn a_layer_closed_without_a_key_swallows_only_a_key_still_held(cx: &mut TestAppContext) {
    let (cx, terminal, typed) = open_recording_terminal(cx);
    let only = handle(cx);
    let (layer, _, _) = open_dialog(cx, vec![("only", only.clone())], &only);
    keys(cx, "x");
    advance(cx, Duration::from_millis(501));
    cx.update(|window, cx| layers::close(layer, window, cx));
    settle(cx);
    assert!(
        focused(cx, &terminal),
        "the premise: closing the dialog returned focus to the terminal"
    );

    held(cx, "x");
    assert!(
        typed.borrow().is_empty(),
        "a key still held from the last press did not reach the terminal"
    );
    keys(cx, "x");
    assert_eq!(
        *typed.borrow(),
        ["x"],
        "a fresh press after the window reached the terminal"
    );
}

/// Why this matters: the key capture's handler runs inside the keystroke interceptor, before GPUI matches any binding
/// (a physical-key row's action, by contrast, is queued and runs once the key's handling is over). A layer the handler
/// closes must be taken as closed by the press being handled, not the one before it, or its repeats would leak into
/// whatever focus went back to.
///
/// Specified: with a dialog open over the terminal and a capture armed that closes the dialog on X, X closes it; a held
/// repeat of X and a fresh X at once do not reach the terminal.
#[gpui::test]
fn a_layer_the_key_capture_closes_is_taken_as_closed_by_that_press(cx: &mut TestAppContext) {
    let (cx, terminal, typed) = open_recording_terminal(cx);
    let only = handle(cx);
    let (layer, _, _) = open_dialog(cx, vec![("only", only.clone())], &only);
    let armed = cx.update(move |_, cx| {
        crate::actions::arm_key_capture(
            move |down, window, cx| {
                if down.keystroke.key != "x" {
                    return crate::actions::Captured::Cancel;
                }
                crate::actions::disarm_key_capture(cx);
                layers::close(layer, window, cx);
                crate::actions::Captured::Consumed
            },
            |_| {},
            cx,
        )
    });
    assert!(armed, "the premise: the capture armed");

    keys(cx, "x");
    assert!(
        !is_open(cx, layer) && focused(cx, &terminal),
        "the premise: the capture closed the dialog and focus went back to the terminal"
    );
    held(cx, "x");
    keys(cx, "x");
    assert!(
        typed.borrow().is_empty(),
        "no repeat of X reached the terminal: {:?}",
        typed.borrow()
    );
}

// ---- Scrolling the focused control into view -------------------------------------------------------------------------

/// A dialog frame drawn on the page, as the component tests draw one, with `body` as its body and no answers. In a
/// window 400 pixels tall the frame keeps to 352, so a body much taller than that scrolls inside "tall-body".
fn tall_dialog(body: impl IntoElement) -> AnyElement {
    DialogFrame::new("tall", "A tall dialog", body, Vec::new()).into_any_element()
}

/// Whether the element drawn under `selector` lies wholly inside the element drawn under `region`, as last drawn.
fn shows_whole(cx: &mut VisualTestContext, selector: &'static str, region: &'static str) -> bool {
    let (inner, outer) = (bounds(cx, selector), bounds(cx, region));
    inner.top() >= outer.top() && inner.bottom() <= outer.bottom()
}

/// How far the element drawn under `selector` sits inside the edge of the element drawn under `region`: `top` from its
/// top edge, otherwise from its bottom edge. A reveal by the least distance leaves the control within a few pixels of
/// the edge (about the reveal margin, which the coordinator measures from inside the control's border).
fn gap_to_edge(
    cx: &mut VisualTestContext,
    selector: &'static str,
    region: &'static str,
    top: bool,
) -> f32 {
    let (inner, outer) = (bounds(cx, selector), bounds(cx, region));
    f32::from(if top {
        inner.top() - outer.top()
    } else {
        outer.bottom() - inner.bottom()
    })
}

/// Whether a gap from [`gap_to_edge`] is what a least-distance reveal leaves: the control inside the edge by four pixels
/// at most, a little more than the reveal margin.
fn at_edge(gap: f32) -> bool {
    (0. ..=4.).contains(&gap)
}

/// Tab onto controls below the fold of a dialog's body scrolls each into view, ending on a text field, and Shift+Tab
/// back scrolls up again. Without this, keyboard focus goes where the user cannot see it, and SPEC.md's "Reaching
/// everything by keyboard" holds only for a user who also scrolls with the pointer. Going back up onto a control above
/// the view that is not the body's first, the body scrolls by the least distance: the control lands at the top edge,
/// not at the bottom, where an algorithm that always aligns one way would put it. The field is a GPUI Kit input, which
/// tracks its own focus and only records its position, so this also covers that path.
#[gpui::test]
fn tab_to_a_field_below_the_fold_scrolls_it_into_view_and_shift_tab_scrolls_back(
    cx: &mut TestAppContext,
) {
    let stops = Rc::new(RefCell::new(None::<(FocusHandle, FocusHandle)>));
    let field: FieldState = Rc::default();
    let (drawn, drawn_field) = (stops.clone(), field.clone());
    let cx = open(cx, move |window, cx| {
        let Some((top, middle)) = drawn.borrow().clone() else {
            return div().into_any_element();
        };
        let state = drawn_field
            .borrow_mut()
            .get_or_insert_with(|| TextInputState::new(window, cx))
            .clone();
        let help = HoverHelp::tooltip("A test field");
        tall_dialog(
            div()
                .child(button("top", &top))
                .child(div().h(px(600.)))
                .child(button("middle", &middle))
                .child(div().h(px(600.)))
                .child(
                    div()
                        .debug_selector(|| "field".into())
                        .w(px(200.))
                        .child(TextInput::new("field-input", &state, "Field", help)),
                ),
        )
    });
    cx.simulate_resize(size(px(720.), px(400.)));
    let (top, middle) = (handle(cx), handle(cx));
    let field_focused = move |cx: &mut VisualTestContext| {
        let state = field.borrow().clone();
        cx.update(|window, cx| state.is_some_and(|state| state.focus_handle(cx).is_focused(window)))
    };
    stops.replace(Some((top.clone(), middle.clone())));
    redraw(cx);
    assert!(
        shows_whole(cx, "top", "tall-body")
            && bounds(cx, "middle").top() >= bounds(cx, "tall-body").bottom(),
        "the premise: the top control shows and the next one is below the fold"
    );

    keys(cx, "tab tab");
    assert!(
        focused(cx, &middle),
        "the premise: Tab reached the middle control"
    );
    assert!(
        shows_whole(cx, "middle", "tall-body"),
        "the body scrolled it into view"
    );
    keys(cx, "tab");
    assert!(field_focused(cx), "the premise: Tab reached the field");
    assert!(
        shows_whole(cx, "field", "tall-body"),
        "the body scrolled the whole field into view"
    );
    assert!(
        bounds(cx, "middle").bottom() <= bounds(cx, "tall-body").top(),
        "the premise of going back: the middle control is above the view"
    );

    keys(cx, "shift-tab");
    assert!(
        focused(cx, &middle),
        "the premise: Shift+Tab went back to the middle control"
    );
    let gap = gap_to_edge(cx, "middle", "tall-body", true);
    assert!(
        at_edge(gap),
        "the body scrolled up by the least distance: {gap} below the top edge"
    );
    keys(cx, "shift-tab");
    assert!(
        focused(cx, &top),
        "the premise: Shift+Tab went back to the top control"
    );
    assert!(
        shows_whole(cx, "top", "tall-body"),
        "the body scrolled back to show it"
    );
}

/// Focus moving down onto a control below the view scrolls it by the least distance, to the bottom edge, and focus
/// moving onto a control that already shows whole scrolls nothing, as in a browser. The body jumping on every Tab, to
/// put each control at one edge, would lose the user's place. The second half runs with the body already scrolled
/// partway, where a scroll in either direction could show.
#[gpui::test]
fn tab_scrolls_the_least_distance_and_not_at_all_for_a_control_already_in_view(
    cx: &mut TestAppContext,
) {
    let stops = Rc::new(RefCell::new(
        None::<(FocusHandle, FocusHandle, FocusHandle)>,
    ));
    let drawn = stops.clone();
    let cx = open(cx, move |_, _| {
        let Some((top, first, second)) = drawn.borrow().clone() else {
            return div().into_any_element();
        };
        tall_dialog(
            div()
                .child(button("top", &top))
                .child(div().h(px(600.)))
                .child(button("first", &first))
                .child(button("second", &second))
                .child(div().h(px(600.))),
        )
    });
    cx.simulate_resize(size(px(720.), px(400.)));
    let (top, first, second) = (handle(cx), handle(cx), handle(cx));
    stops.replace(Some((top.clone(), first.clone(), second.clone())));
    redraw(cx);
    keys(cx, "tab");
    assert!(
        focused(cx, &top),
        "the premise: the first Tab reaches the top control"
    );
    assert!(
        bounds(cx, "first").top() >= bounds(cx, "tall-body").bottom(),
        "the premise: the next control is below the fold"
    );

    keys(cx, "tab");
    assert!(
        focused(cx, &first),
        "the premise: Tab reached the first control below"
    );
    let gap = gap_to_edge(cx, "first", "tall-body", false);
    assert!(
        at_edge(gap),
        "the body scrolled down by the least distance: {gap} above the bottom edge"
    );
    keys(cx, "tab");
    assert!(
        focused(cx, &second),
        "the premise: Tab reached the control right under it"
    );
    let before = bounds(cx, "first");
    assert!(
        shows_whole(cx, "first", "tall-body") && before.top() > bounds(cx, "tall-body").top(),
        "the premise: the body is scrolled partway and the first control shows whole, below the top edge"
    );

    keys(cx, "shift-tab");
    assert!(
        focused(cx, &first),
        "the premise: Shift+Tab went back to it"
    );
    assert_eq!(bounds(cx, "first"), before, "the body did not scroll");
}

/// In a window so short that the dialog's whole card scrolls (the frame's mode for when even the title and answers do
/// not fit), the body sits at its floor and partly outside the card's visible box. Tab onto a control below the body's
/// fold must scroll both: the body to the control, then the card to the body's part that holds it. Scrolling only the
/// nearest region leaves the control cut off by the card.
///
/// Then two of the app's focus moves before a frame: the first, to the answer below the card's fold, scrolls the card,
/// which moves the body with it; the second, back to the body's control, must measure the body's box where the card's
/// scroll put it. Measured where the frame on screen drew it, the body would look misplaced by the card's scroll and
/// scroll the control out of its own view.
#[gpui::test]
fn tab_in_a_dialog_whose_card_scrolls_scrolls_body_and_card_until_the_control_shows(
    cx: &mut TestAppContext,
) {
    let stops = Rc::new(RefCell::new(
        None::<(FocusHandle, FocusHandle, FocusHandle)>,
    ));
    let drawn = stops.clone();
    let cx = open(cx, move |_, _| {
        let Some((top, deep, answer)) = drawn.borrow().clone() else {
            return div().into_any_element();
        };
        let body = div()
            .child(button("top", &top))
            .child(div().h(px(300.)))
            .child(button("deep", &deep));
        let answers = vec![button("answer", &answer)];
        DialogFrame::new("short", "A dialog in a very short window", body, answers)
            .summary("with a summary line under the title")
            .into_any_element()
    });
    cx.simulate_resize(size(px(720.), px(160.)));
    let (top, deep, answer) = (handle(cx), handle(cx), handle(cx));
    stops.replace(Some((top.clone(), deep.clone(), answer.clone())));
    redraw(cx);
    assert!(
        bounds(cx, "short-body").bottom() > bounds(cx, "short").bottom()
            && !shows_whole(cx, "deep", "short-body"),
        "the premise: the card scrolls, the body reaches below its visible box, and the control is below the body's fold"
    );

    keys(cx, "tab");
    assert!(
        focused(cx, &top),
        "the premise: the first Tab reaches the top control"
    );
    keys(cx, "tab");
    assert!(
        focused(cx, &deep),
        "the premise: Tab reached the control below the fold"
    );
    assert!(
        shows_whole(cx, "deep", "short-body"),
        "the body scrolled the control into its view"
    );
    assert!(
        shows_whole(cx, "deep", "short"),
        "the card scrolled so the control shows in the window too"
    );

    assert!(
        !shows_whole(cx, "answer", "short"),
        "the premise of the second half: the answer is below the card's fold, so moving to it scrolls the card"
    );
    let now = cx.update(|_, cx| Mark::take(cx));
    let (to_answer, to_deep) = (answer.clone(), deep.clone());
    cx.update(move |window, cx| {
        focus::focus_control(&to_answer, &now, window, cx);
        focus::focus_control(&to_deep, &now, window, cx);
    });
    settle(cx);
    assert!(
        focused(cx, &deep),
        "the premise: the second move took focus back to the control"
    );
    assert!(
        shows_whole(cx, "deep", "short-body") && shows_whole(cx, "deep", "short"),
        "the control still shows whole in the body and the card"
    );
}

/// A pointer press that focuses a partly hidden control leaves the scroll where it is: browsers do the same, and
/// scrolling would move the very control under the pointer while the user clicks it. A focus move the app then makes
/// onto that control, which already holds focus, does scroll it into view, in a frame of its own: GPUI draws nothing
/// for a focus move onto the control that holds focus, so the scroll has to ask for that frame. The region is a plain
/// page region opted in through `scroll_region`, as the sidebar's host list is.
#[gpui::test]
fn a_press_leaves_a_partly_hidden_control_where_it_is_and_the_apps_focus_move_reveals_it(
    cx: &mut TestAppContext,
) {
    let scroll = ScrollHandle::new();
    let stops = Rc::new(RefCell::new(None::<(FocusHandle, FocusHandle)>));
    let (drawn, region) = (stops.clone(), scroll.clone());
    let cx = open(cx, move |_, _| {
        let Some((first, partial)) = drawn.borrow().clone() else {
            return div().into_any_element();
        };
        // 100 pixels tall, with "partial" 80 pixels down: the region's bottom edge cuts through it.
        let content = div()
            .id("region")
            .debug_selector(|| "region".into())
            .w(px(200.))
            .h(px(100.))
            .overflow_y_scroll()
            .child(div().h(px(80.)).child(button("first", &first)))
            .child(button("partial", &partial))
            .child(div().h(px(300.)));
        focus::scroll_region(content, &region).into_any_element()
    });
    let (first, partial) = (handle(cx), handle(cx));
    stops.replace(Some((first, partial.clone())));
    redraw(cx);
    let (before, view) = (bounds(cx, "partial"), bounds(cx, "region"));
    assert!(
        before.top() < view.bottom() - px(10.) && before.bottom() > view.bottom(),
        "the premise: the region's edge cuts through the control: {before:?} in {view:?}"
    );

    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    cx.simulate_click(
        point(before.center().x, before.top() + px(5.)),
        Modifiers::none(),
    );
    check(cx);
    assert!(
        focused(cx, &partial),
        "the premise: the click focused the control"
    );
    assert_eq!(bounds(cx, "partial"), before, "the press scrolled nothing");

    let now = cx.update(|_, cx| Mark::take(cx));
    let target = partial.clone();
    cx.update(move |window, cx| focus::focus_control(&target, &now, window, cx));
    settle(cx);
    assert!(
        focused(cx, &partial),
        "the premise: focus stayed on the control"
    );
    assert!(
        shows_whole(cx, "partial", "region"),
        "the app's focus move scrolled the whole control into view"
    );
}

/// Two focus moves made before a frame is drawn measure the second control where it is, not where the frame on screen
/// shows it. A reveal scrolls without drawing, so after the first move scrolled the region down, the control at its top
/// is drawn as in view but no longer is; measuring it there would leave focus out of sight. Key presses handled back to
/// back during a slow frame do this in the app.
#[gpui::test]
fn a_second_focus_move_before_a_frame_measures_the_scroll_the_first_made(cx: &mut TestAppContext) {
    let scroll = ScrollHandle::new();
    let stops = Rc::new(RefCell::new(None::<(FocusHandle, FocusHandle)>));
    let (drawn, region) = (stops.clone(), scroll.clone());
    let cx = open(cx, move |_, _| {
        let Some((first, far)) = drawn.borrow().clone() else {
            return div().into_any_element();
        };
        let content = div()
            .id("region")
            .debug_selector(|| "region".into())
            .w(px(200.))
            .h(px(100.))
            .overflow_y_scroll()
            .child(button("first", &first))
            .child(div().h(px(300.)))
            .child(button("far", &far));
        focus::scroll_region(content, &region).into_any_element()
    });
    let (first, far) = (handle(cx), handle(cx));
    stops.replace(Some((first.clone(), far.clone())));
    redraw(cx);
    assert!(
        shows_whole(cx, "first", "region")
            && bounds(cx, "far").top() >= bounds(cx, "region").bottom(),
        "the premise: the first control shows and the far one is below the fold"
    );

    let now = cx.update(|_, cx| Mark::take(cx));
    let (to_far, to_first) = (far.clone(), first.clone());
    cx.update(move |window, cx| {
        focus::focus_control(&to_far, &now, window, cx);
        focus::focus_control(&to_first, &now, window, cx);
    });
    settle(cx);
    assert!(
        focused(cx, &first),
        "the premise: the second move took focus back to the first control"
    );
    assert!(
        shows_whole(cx, "first", "region"),
        "the region scrolled back up: the second move saw the first one's scroll"
    );
}

/// A dialog's content for the [`focus::focus_in_layer`] tests: a tall body that starts on `start`, at its top, with
/// `near` beside it and `target` far below the fold. `near` is drawn while `near_shown` holds and `target` while
/// `target_shown` does; neither cell notifies, so a test changes one and then redraws, which makes that frame the
/// first (or the last) to draw the control.
struct LayerContent {
    start: FocusHandle,
    near: FocusHandle,
    target: FocusHandle,
    near_shown: Rc<Cell<bool>>,
    target_shown: Rc<Cell<bool>>,
}

impl Render for LayerContent {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        tall_dialog(
            div()
                .child(button("start", &self.start))
                .children(self.near_shown.get().then(|| button("near", &self.near)))
                .child(div().h(px(600.)))
                .children(
                    self.target_shown
                        .get()
                        .then(|| button("target", &self.target)),
                ),
        )
    }
}

/// An open [`LayerContent`] dialog: its layer, its controls' handles, the page button behind it, the cells that show
/// `near` and `target`, and the slot the page draws its button from (empty, the page draws no button).
struct LayerDialog {
    id: layers::LayerId,
    start: FocusHandle,
    near: FocusHandle,
    target: FocusHandle,
    page: FocusHandle,
    near_shown: Rc<Cell<bool>>,
    target_shown: Rc<Cell<bool>>,
    page_slot: Rc<RefCell<Option<FocusHandle>>>,
}

/// Open a [`LayerContent`] dialog on a 400-pixel page holding a "page" button, starting on `start`, and check that
/// `target` is drawn below the body's fold. Returns the dialog, `start`, `target` and the page button.
fn open_layer_dialog(
    cx: &mut TestAppContext,
) -> (
    &mut VisualTestContext,
    layers::LayerId,
    FocusHandle,
    FocusHandle,
    FocusHandle,
) {
    let (cx, dialog) = open_layer_dialog_with(cx, true);
    assert!(
        bounds(cx, "target").top() >= bounds(cx, "tall-body").bottom(),
        "the premise: the target is drawn below the body's fold"
    );
    (cx, dialog.id, dialog.start, dialog.target, dialog.page)
}

/// [`open_layer_dialog`] with `target` drawn only if `target_shown`; `near` is drawn. Checks that the dialog holds
/// focus on its start and that `target` is drawn exactly when asked.
fn open_layer_dialog_with(
    cx: &mut TestAppContext,
    target_shown: bool,
) -> (&mut VisualTestContext, LayerDialog) {
    let page = Rc::new(RefCell::new(None::<FocusHandle>));
    let drawn = page.clone();
    let cx = open(cx, move |_, _| {
        div()
            .children(drawn.borrow().clone().map(|focus| button("page", &focus)))
            .into_any_element()
    });
    cx.simulate_resize(size(px(720.), px(400.)));
    let (start, near, target, container, on_page) =
        (handle(cx), handle(cx), handle(cx), handle(cx), handle(cx));
    page.replace(Some(on_page.clone()));
    redraw(cx);
    let (near_shown, shown) = (Rc::new(Cell::new(true)), Rc::new(Cell::new(target_shown)));
    let content = LayerContent {
        start: start.clone(),
        near: near.clone(),
        target: target.clone(),
        near_shown: near_shown.clone(),
        target_shown: shown.clone(),
    };
    let first = start.clone();
    let opened = cx.update(move |window, cx| {
        let view = cx.new(|_| content);
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container,
                start: first,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: true,
                returns_to: focus::ReturnTo::Opener,
                on_dismiss: None,
                placement: layers::DialogPlacement::Centered,
                dismiss_on_outside_press: false,
            },
            &mark,
            window,
            cx,
        )
    });
    settle(cx);
    let Some(id) = opened else {
        panic!("the dialog did not open");
    };
    assert!(
        focused(cx, &start) && cx.debug_bounds("target").is_some() == target_shown,
        "the premise: the dialog holds focus on its start, and the target is drawn only if asked"
    );
    (
        cx,
        LayerDialog {
            id,
            start,
            near,
            target,
            page: on_page,
            near_shown,
            target_shown: shown,
            page_slot: page,
        },
    )
}

/// Move focus to `target` with [`focus::focus_in_layer`] for `mark`, then settle.
fn move_in_layer(cx: &mut VisualTestContext, target: &FocusHandle, mark: Mark) {
    let target = target.clone();
    cx.update(move |window, cx| focus::focus_in_layer(&target, &mark, window, cx));
    settle(cx);
}

/// A result that arrives later moves focus to a control already drawn in the open dialog, and scrolls it into view:
/// Settings opening agent trust from a trust hint, then moving focus to the agent's switch once its read answers
/// (SPEC.md: "open agent trust at the relevant switch and scroll its block into view"). [`focus::focus_control`] does
/// nothing while a dialog is open, so this is the call for it.
#[gpui::test]
fn focus_in_layer_moves_focus_inside_the_dialog_and_scrolls_the_control_into_view(
    cx: &mut TestAppContext,
) {
    let (cx, _, _, target, _) = open_layer_dialog(cx);
    let mark = cx.update(|_, cx| Mark::take(cx));
    move_in_layer(cx, &target, mark);
    assert!(focused(cx, &target), "focus moved to the control");
    assert!(
        shows_whole(cx, "target", "tall-body"),
        "the body scrolled the whole control into view"
    );
}

/// Once the user has pressed a key or clicked since the action whose result arrives, the result moves no focus
/// (SPEC.md, "Who may move focus": "Once the user has moved on, the result moves no focus"). Focus staying on the same
/// control does not show the user has not moved on: both the key and the click here leave it where it was.
#[gpui::test]
fn focus_in_layer_does_nothing_after_the_user_moved_on(cx: &mut TestAppContext) {
    let (cx, _, start, target, _) = open_layer_dialog(cx);
    let before = bounds(cx, "target");

    let mark = cx.update(|_, cx| Mark::take(cx));
    keys(cx, "x");
    assert!(
        focused(cx, &start),
        "the premise: the key left focus on the start control"
    );
    move_in_layer(cx, &target, mark);
    assert!(focused(cx, &start), "after a key press, nothing moved");

    let mark = cx.update(|_, cx| Mark::take(cx));
    let header = bounds(cx, "tall");
    cx.simulate_click(
        point(header.center().x, header.top() + px(10.)),
        Modifiers::none(),
    );
    check(cx);
    assert!(
        focused(cx, &start),
        "the premise: the click on the dialog's header left focus on the start control"
    );
    move_in_layer(cx, &target, mark);
    assert!(focused(cx, &start), "after a click, nothing moved");
    assert_eq!(bounds(cx, "target"), before, "and nothing scrolled");
}

/// A later result never takes focus out of where it now is: not into a dialog that another layer now covers, and not
/// out to the page while a dialog is open. Each would pull focus away from what the user is looking at, which
/// [`focus::focus_in_layer`]'s check of the topmost layer prevents. With no dialog open it moves no focus at all, not
/// even to a control drawn on the page: that is [`focus::focus_control`]'s job, which keeps the page's own rules.
#[gpui::test]
fn focus_in_layer_refuses_a_target_outside_the_topmost_dialog(cx: &mut TestAppContext) {
    let (cx, dialog, start, target, page) = open_layer_dialog(cx);

    let mark = cx.update(|_, cx| Mark::take(cx));
    move_in_layer(cx, &page, mark);
    assert!(
        focused(cx, &start),
        "a control on the page behind the dialog is refused"
    );

    let above = handle(cx);
    let (covering, _, _) = open_dialog(cx, vec![("above", above.clone())], &above);
    assert!(
        focused(cx, &above),
        "the premise: another dialog opened above and holds focus"
    );
    let mark = cx.update(|_, cx| Mark::take(cx));
    move_in_layer(cx, &target, mark);
    assert!(
        focused(cx, &above),
        "a control in the covered dialog is refused"
    );

    cx.update(move |window, cx| {
        layers::close(covering, window, cx);
        layers::close(dialog, window, cx);
    });
    settle(cx);
    assert!(
        !is_open(cx, dialog) && !focused(cx, &page) && cx.debug_bounds("page").is_some(),
        "the premise: both dialogs closed, and the page's control is drawn and does not hold focus"
    );
    let mark = cx.update(|_, cx| Mark::take(cx));
    move_in_layer(cx, &page, mark);
    assert!(
        !focused(cx, &page),
        "with no dialog open, nothing moved, not even to a drawn control on the page"
    );
}

/// A dialog's content in the shape of the launcher's Custom choice: an "other" button, then a "custom" button whose
/// click shows a "flags" control far below the body's fold. The click takes the mark of the user's action (`chosen`),
/// makes the flags appear and calls [`focus::focus_in_layer`] for them right there, in the action, as area code does:
/// the flags are not drawn yet, and the coordinator waits for the frame that draws them. "other" is somewhere else for
/// focus to be when the click on "custom" leaves focus where it was (macOS).
struct CustomContent {
    other: FocusHandle,
    custom: FocusHandle,
    flags: FocusHandle,
    chosen: Rc<Cell<Option<Mark>>>,
}

impl Render for CustomContent {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = self.chosen.get().is_some();
        let (view, chosen, flags) = (cx.entity(), self.chosen.clone(), self.flags.clone());
        let custom = button("custom", &self.custom).on_click(move |_, window, cx| {
            let mark = Mark::take(cx);
            chosen.set(Some(mark));
            view.update(cx, |_, cx| cx.notify());
            focus::focus_in_layer(&flags, &mark, window, cx);
        });
        tall_dialog(
            div()
                .child(button("other", &self.other))
                .child(custom)
                .child(div().h(px(600.)))
                .children(shown.then(|| button("flags", &self.flags))),
        )
    }
}

/// An open [`CustomContent`] dialog: its controls' handles and the cell that receives the click's mark.
struct CustomDialog {
    other: FocusHandle,
    custom: FocusHandle,
    flags: FocusHandle,
    chosen: Rc<Cell<Option<Mark>>>,
}

/// Open a [`CustomContent`] dialog on a 400-pixel page, starting on "custom"; returns the flags control's handle and the
/// cell that receives the click's mark.
fn open_custom_dialog(
    cx: &mut TestAppContext,
) -> (&mut VisualTestContext, FocusHandle, Rc<Cell<Option<Mark>>>) {
    let (cx, dialog) = open_custom_dialog_on(cx, false);
    (cx, dialog.flags, dialog.chosen)
}

/// [`open_custom_dialog`], starting on "other" when `on_other` and on "custom" otherwise, and checking that the
/// dialog holds focus there with the flags control not drawn.
fn open_custom_dialog_on(
    cx: &mut TestAppContext,
    on_other: bool,
) -> (&mut VisualTestContext, CustomDialog) {
    let cx = open(cx, |_, _| div().into_any_element());
    cx.simulate_resize(size(px(720.), px(400.)));
    settle(cx);
    let (other, custom, flags, container) = (handle(cx), handle(cx), handle(cx), handle(cx));
    let chosen = Rc::new(Cell::new(None));
    let content = CustomContent {
        other: other.clone(),
        custom: custom.clone(),
        flags: flags.clone(),
        chosen: chosen.clone(),
    };
    let start = if on_other {
        other.clone()
    } else {
        custom.clone()
    };
    let opened = cx.update(move |window, cx| {
        let view = cx.new(|_| content);
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container,
                start,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: true,
                returns_to: focus::ReturnTo::Opener,
                on_dismiss: None,
                placement: layers::DialogPlacement::Centered,
                dismiss_on_outside_press: false,
            },
            &mark,
            window,
            cx,
        )
    });
    settle(cx);
    let started = if on_other { &other } else { &custom };
    assert!(
        opened.is_some() && focused(cx, started) && cx.debug_bounds("flags").is_none(),
        "the premise: the dialog opened on its start, and the flags control is not drawn"
    );
    (
        cx,
        CustomDialog {
            other,
            custom,
            flags,
            chosen,
        },
    )
}

/// A user's own action that makes a new control appear and moves focus into it, as choosing Custom in the launcher
/// moves focus into its flags field (SPEC.md: accepting Custom "moves focus to its flags editor"). The target is not
/// drawn while the click is handled, and the owner calls [`focus::focus_in_layer`] from the click all the same: the
/// coordinator moves focus once the frame that draws it is on screen, and the control, below the body's fold, scrolls
/// into view. On macOS a click does not focus the button, so no move that carries focus from the button would do.
#[gpui::test]
fn focus_in_layer_from_the_action_moves_to_a_control_the_action_showed(cx: &mut TestAppContext) {
    let (cx, flags, chosen) = open_custom_dialog(cx);
    click(cx, "custom");
    assert!(
        chosen.get().is_some(),
        "the premise: the click chose Custom"
    );
    assert!(
        focused(cx, &flags),
        "focus moved to the control the click showed"
    );
    assert!(
        shows_whole(cx, "flags", "tall-body"),
        "the body scrolled the whole control into view"
    );
    assert!(
        !shows_whole(cx, "custom", "tall-body"),
        "and the body really scrolled: the clicked button left the view"
    );
}

/// The macOS shape of the Custom choice, and of host settings' Edit: a click leaves the clicked button unfocused, so
/// focus stays on whatever other control of the dialog held it, and the click makes a control appear that focus must
/// move to. Nothing about the button's own focus can carry focus there; only the move the click asks for, which waits
/// for the frame that draws its target.
///
/// Specified: under macOS's click rule, with focus on another control of the dialog and the flags control not drawn,
/// a click on the custom button leaves the button unfocused, and focus then moves to the flags control the click
/// showed, which the body scrolls into view whole.
#[gpui::test]
fn focus_in_layer_from_a_click_that_leaves_its_button_unfocused_moves_to_the_control_it_showed(
    cx: &mut TestAppContext,
) {
    let (cx, dialog) = open_custom_dialog_on(cx, true);
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    assert!(
        focused(cx, &dialog.other) && !focused(cx, &dialog.custom),
        "the premise: focus is on another control of the dialog, not the button"
    );
    click(cx, "custom");
    assert!(
        dialog.chosen.get().is_some(),
        "the premise: the click chose Custom"
    );
    assert!(
        !focused(cx, &dialog.custom),
        "the premise: the click left the button unfocused"
    );
    assert!(
        focused(cx, &dialog.flags),
        "focus moved to the control the click showed"
    );
    assert!(
        shows_whole(cx, "flags", "tall-body"),
        "and the body scrolled the whole control into view"
    );
}

/// A move to a control that is not drawn yet must still land once it is drawn, without its caller scheduling a frame
/// of its own: that obligation on every caller is what made area code pick the wrong move or queue `window.defer`
/// from its render. The coordinator waits for the first frame that draws the target inside the layer.
///
/// Specified: with the target not drawn, [`focus::focus_in_layer`] for a mark taken now leaves focus on the start
/// control, and so does a frame that draws the dialog without the target; the first frame that draws the target moves
/// focus to it and scrolls the body to show it whole.
#[gpui::test]
fn focus_in_layer_waits_for_the_frame_that_draws_its_target(cx: &mut TestAppContext) {
    let (cx, dialog) = open_layer_dialog_with(cx, false);
    let mark = cx.update(|_, cx| Mark::take(cx));
    move_in_layer(cx, &dialog.target, mark);
    assert!(
        focused(cx, &dialog.start),
        "nothing moved while the target is not drawn"
    );
    redraw(cx);
    assert!(
        focused(cx, &dialog.start),
        "a frame that does not draw the target moved nothing"
    );
    dialog.target_shown.set(true);
    redraw(cx);
    assert!(
        focused(cx, &dialog.target),
        "the frame that drew the target moved focus to it"
    );
    assert!(
        shows_whole(cx, "target", "tall-body"),
        "and the body scrolled the whole control into view"
    );
}

/// The user moving on cancels a waiting move as it cancels an immediate one (SPEC.md, "Who may move focus"): a key
/// pressed while the target is still on its way must not be followed by focus jumping away from where the user is.
///
/// Specified: after [`focus::focus_in_layer`] waits for an undrawn target, a key press that leaves focus on the start
/// control, then the frame that draws the target, leave focus on the start control and scroll nothing to the target.
#[gpui::test]
fn a_waiting_focus_in_layer_is_dropped_once_the_user_moved_on(cx: &mut TestAppContext) {
    let (cx, dialog) = open_layer_dialog_with(cx, false);
    let mark = cx.update(|_, cx| Mark::take(cx));
    move_in_layer(cx, &dialog.target, mark);
    keys(cx, "x");
    assert!(
        focused(cx, &dialog.start),
        "the premise: the key left focus on the start control"
    );
    dialog.target_shown.set(true);
    redraw(cx);
    assert!(
        focused(cx, &dialog.start),
        "the target drawn after the user moved on took no focus"
    );
    assert!(
        !shows_whole(cx, "target", "tall-body"),
        "and nothing scrolled to it"
    );
}

/// The window has one pending move, and a newer move wins over it: a result that moved focus elsewhere after the
/// action that is still waiting must not be undone when the older target finally draws.
///
/// Specified: with [`focus::focus_in_layer`] waiting for an undrawn target, a second one to a drawn control moves focus
/// there at once, and the frame that then draws the first target leaves focus on the second control.
#[gpui::test]
fn a_newer_focus_move_replaces_a_waiting_one(cx: &mut TestAppContext) {
    let (cx, dialog) = open_layer_dialog_with(cx, false);
    let mark = cx.update(|_, cx| Mark::take(cx));
    move_in_layer(cx, &dialog.target, mark);
    move_in_layer(cx, &dialog.near, mark);
    assert!(
        focused(cx, &dialog.near),
        "the premise: the newer move, to a drawn control, moved focus at once"
    );
    dialog.target_shown.set(true);
    redraw(cx);
    assert!(
        focused(cx, &dialog.near),
        "the older target, drawn later, took no focus"
    );
}

/// A waiting move belongs to the layer it was asked in, while focus stays there: once focus has left it (another
/// dialog opened above, here a result that needs no user input), the move is stale even after focus comes back, since
/// what the user sees has changed in between. Checking the layer only when the target draws would miss this.
///
/// Specified: with [`focus::focus_in_layer`] waiting for an undrawn target, another dialog opens above and closes
/// again, returning focus to the start control; the frame that then draws the target leaves focus on the start
/// control. No key or click happens, so only focus leaving the layer drops the move.
#[gpui::test]
fn a_waiting_focus_in_layer_is_dropped_once_focus_left_the_layer(cx: &mut TestAppContext) {
    let (cx, dialog) = open_layer_dialog_with(cx, false);
    let mark = cx.update(|_, cx| Mark::take(cx));
    move_in_layer(cx, &dialog.target, mark);
    let above = handle(cx);
    let (covering, _, _) = open_dialog(cx, vec![("above", above.clone())], &above);
    assert!(
        focused(cx, &above),
        "the premise: another dialog opened above and holds focus"
    );
    cx.update(move |window, cx| layers::close(covering, window, cx));
    settle(cx);
    assert!(
        focused(cx, &dialog.start),
        "the premise: closing it returned focus to the start control"
    );
    dialog.target_shown.set(true);
    redraw(cx);
    assert!(
        focused(cx, &dialog.start),
        "the target drawn after focus came back took no focus"
    );
}

/// An action may replace the control it was pressed on with the control focus should go to, as an inline question
/// takes the place of the button that asked it. In the frame that does that, the focused control vanishes and the
/// coordinator recovers focus to the closest control left before the waiting move runs; that recovery is the
/// coordinator keeping focus in place, not a newer move, and must not cancel it.
///
/// Specified: with focus on the near control and [`focus::focus_in_layer`] waiting for the undrawn target, a frame
/// that stops drawing the near control and draws the target leaves focus on the target.
#[gpui::test]
fn a_waiting_focus_in_layer_survives_recovery_from_a_control_that_vanished(
    cx: &mut TestAppContext,
) {
    let (cx, dialog) = open_layer_dialog_with(cx, false);
    let mark = cx.update(|_, cx| Mark::take(cx));
    move_in_layer(cx, &dialog.near, mark);
    assert!(
        focused(cx, &dialog.near),
        "the premise: the near control holds focus"
    );
    let mark = cx.update(|_, cx| Mark::take(cx));
    move_in_layer(cx, &dialog.target, mark);
    dialog.near_shown.set(false);
    dialog.target_shown.set(true);
    redraw(cx);
    assert!(
        cx.debug_bounds("near").is_none(),
        "the premise: the near control is no longer drawn"
    );
    assert!(
        focused(cx, &dialog.target),
        "focus moved to the target after the recovery from the vanished control"
    );
}

// ---- A dialog naming its return target -------------------------------------------------------------------------------

/// Open a dialog of one "named-start" button that starts there and names `control` as where closing returns focus
/// ([`focus::ReturnTo::Control`]), for a mark taken now.
fn open_naming(
    control: &FocusHandle,
    start: &FocusHandle,
    window: &mut Window,
    cx: &mut gpui::App,
) -> Option<layers::LayerId> {
    let view = cx.new(|_| Panel {
        controls: vec![("named-start", start.clone())],
        shown: Rc::new(Cell::new(true)),
    });
    let mark = Mark::take(cx);
    layers::open_dialog(
        Dialog {
            view: view.into(),
            container: cx.focus_handle(),
            start: start.clone(),
            subject: None,
            main_action: None,
            keeps_focus_on_empty_click: true,
            returns_to: focus::ReturnTo::Control(control.clone()),
            on_dismiss: None,
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        &mark,
        window,
        cx,
    )
}

/// A button `id`, focusing `control`, whose click opens [`open_naming`]'s dialog naming it, starting on `start`: an
/// opener that, as the app's openers now do, does not focus itself before opening.
fn naming_button(id: &'static str, control: &FocusHandle, start: &FocusHandle) -> Button {
    let (named, first) = (control.clone(), start.clone());
    button(id, control).on_click(move |_, window, cx| {
        open_naming(&named, &first, window, cx);
    })
}

/// A dialog's content holding its start control and a [`naming_button`] beside it, for a dialog opened from inside a
/// dialog.
struct InnerOpener {
    start: FocusHandle,
    inner: FocusHandle,
    inner_start: FocusHandle,
}

impl Render for InnerOpener {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .gap(px(8.))
            .child(button("outer-start", &self.start))
            .child(naming_button("inner", &self.inner, &self.inner_start))
    }
}

/// On macOS a click leaves a button unfocused, so a dialog a button opens would give focus back to whatever held it
/// before the click; SPEC.md returns it to the button ("Escape and either close control return focus to the gear"),
/// as the old app did by naming it. Openers used to focus the button first, which cost a whole-window refresh and was
/// refused inside another dialog, where the coordinator moves no focus on the page's behalf. The opener names the
/// button instead.
///
/// Specified, under macOS's click rule: with focus on another page control, a click on a page button that opens a
/// dialog naming it leaves focus away from the button until the dialog opens, and Escape returns focus to the button.
/// Inside an open dialog, a click on a button that opens a second dialog naming it leaves focus on the first dialog's
/// start, and Escape on the second returns focus to that button. A dialog that names a page control while another
/// dialog stays open below it gives focus back to what held it when it opened, inside the dialog below.
#[gpui::test]
fn a_dialog_returns_focus_to_the_control_its_opener_names(cx: &mut TestAppContext) {
    let elsewhere = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let opener = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let start = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (drawn_elsewhere, drawn_opener, drawn_start) =
        (elsewhere.clone(), opener.clone(), start.clone());
    let cx = open(cx, move |_, _| {
        div()
            .flex()
            .gap(px(8.))
            .child(button("elsewhere", &drawn_elsewhere))
            .child(naming_button("opener", &drawn_opener, &drawn_start))
            .into_any_element()
    });
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    keys(cx, "tab");
    assert!(
        focused(cx, &elsewhere),
        "the premise: focus is on a page control away from the opener"
    );
    click(cx, "opener");
    assert!(
        focused(cx, &start) && !focused(cx, &opener),
        "the premise: the click opened the dialog with focus on its start"
    );
    keys(cx, "escape");
    assert!(
        focused(cx, &opener),
        "closing it returned focus to the button that opened it, which never held focus"
    );

    let (outer_start, inner, inner_start, container) =
        (handle(cx), handle(cx), handle(cx), handle(cx));
    let content = InnerOpener {
        start: outer_start.clone(),
        inner: inner.clone(),
        inner_start: inner_start.clone(),
    };
    let first = outer_start.clone();
    let outer = cx.update(move |window, cx| {
        let view = cx.new(|_| content);
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container,
                start: first,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: true,
                returns_to: focus::ReturnTo::Opener,
                on_dismiss: None,
                placement: layers::DialogPlacement::Centered,
                dismiss_on_outside_press: false,
            },
            &mark,
            window,
            cx,
        )
    });
    settle(cx);
    assert!(
        outer.is_some() && focused(cx, &outer_start),
        "the premise: the outer dialog opened on its start"
    );
    click(cx, "inner");
    assert!(
        focused(cx, &inner_start),
        "the premise: the inner button's click opened the second dialog"
    );
    keys(cx, "escape");
    assert!(
        focused(cx, &inner),
        "closing the second dialog returned focus to the button inside the first that opened it"
    );

    let named_page = opener.clone();
    let fallback = cx.update(move |window, cx| open_naming(&named_page, &start, window, cx));
    settle(cx);
    assert!(
        fallback.is_some(),
        "the premise: a dialog naming the page's button opened over the outer dialog"
    );
    keys(cx, "escape");
    assert!(
        focused(cx, &inner),
        "with the outer dialog still open, focus went back to its opener there, not to the page"
    );
}

/// A feature may open a dialog and, in the same update, move focus to a control in it other than its start: Settings
/// opened at agent trust when the agent's read has already answered. Nothing of the dialog is drawn yet, focus
/// included, so judging the move then would refuse it, and whether it worked would hang on where the dialog starts.
///
/// Specified: a dialog of two buttons opened on the first, with [`focus::focus_in_layer`] asked for the second in the
/// same update for a mark taken there, has focus on the second once the first frame is drawn.
#[gpui::test]
fn focus_in_layer_asked_as_its_dialog_opens_moves_after_the_first_frame(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let (first, second, container) = (handle(cx), handle(cx), handle(cx));
    let (start, other) = (first.clone(), second.clone());
    let opened = cx.update(move |window, cx| {
        let view = cx.new(|_| Panel {
            controls: vec![("first", start.clone()), ("second", other.clone())],
            shown: Rc::new(Cell::new(true)),
        });
        let mark = Mark::take(cx);
        let id = layers::open_dialog(
            Dialog {
                view: view.into(),
                container,
                start,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: true,
                returns_to: focus::ReturnTo::Opener,
                on_dismiss: None,
                placement: layers::DialogPlacement::Centered,
                dismiss_on_outside_press: false,
            },
            &mark,
            window,
            cx,
        );
        focus::focus_in_layer(&other, &mark, window, cx);
        id
    });
    settle(cx);
    assert!(opened.is_some(), "the premise: the dialog opened");
    assert!(
        focused(cx, &second),
        "focus moved to the second control once the dialog was drawn"
    );
    assert!(!focused(cx, &first), "and left the dialog's start");
}

/// A waiting move must not land anywhere but inside its layer: if the control it waits for first appears outside the
/// dialog (on the page behind it), moving there would pull focus out of the dialog the user is in. The immediate
/// refusal is tested above; this is the same rule for a move kept for later.
///
/// Specified: with the page's button not drawn, [`focus::focus_in_layer`] for it moves nothing; the frame that then
/// draws it on the page behind the dialog leaves focus on the dialog's start, and so does a later frame. The test does
/// not by itself show that the move waited rather than being refused at once: a mutant that moves on the waiting
/// path's refusal shows that it reaches that path.
#[gpui::test]
fn a_waiting_focus_in_layer_whose_target_draws_outside_the_layer_is_refused(
    cx: &mut TestAppContext,
) {
    let (cx, dialog) = open_layer_dialog_with(cx, true);
    let page = dialog.page_slot.take();
    redraw(cx);
    assert!(
        page.is_some() && cx.debug_bounds("page").is_none(),
        "the premise: the page's button is no longer drawn"
    );
    let mark = cx.update(|_, cx| Mark::take(cx));
    move_in_layer(cx, &dialog.page, mark);
    assert!(
        focused(cx, &dialog.start),
        "the premise: nothing moved while the button is not drawn"
    );
    dialog.page_slot.replace(page);
    redraw(cx);
    assert!(
        cx.debug_bounds("page").is_some(),
        "the premise: the button is drawn on the page again"
    );
    assert!(
        focused(cx, &dialog.start),
        "the button drawn outside the dialog took no focus"
    );
    redraw(cx);
    assert!(focused(cx, &dialog.start), "nor in a later frame");
}

/// A named return target can go while its dialog is open (the gear no longer drawn): focusing it then would lose focus
/// on the next frame and send it home, where SPEC.md returns focus "to where it was before it opened".
///
/// Specified: with focus on a page control, a dialog opened naming another page control, which then stops being drawn,
/// gives focus back to the first control when Escape closes it.
#[gpui::test]
fn a_dialog_naming_a_control_no_longer_drawn_returns_focus_to_its_opener(cx: &mut TestAppContext) {
    let elsewhere = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let named = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let shown = Rc::new(Cell::new(true));
    let (drawn_elsewhere, drawn_named, drawn_shown) =
        (elsewhere.clone(), named.clone(), shown.clone());
    let cx = open(cx, move |_, _| {
        div()
            .flex()
            .gap(px(8.))
            .child(button("elsewhere", &drawn_elsewhere))
            .children(drawn_shown.get().then(|| button("named", &drawn_named)))
            .into_any_element()
    });
    keys(cx, "tab");
    assert!(
        focused(cx, &elsewhere),
        "the premise: focus is on the page control the dialog opens over"
    );
    let start = handle(cx);
    let (target, first) = (named.clone(), start.clone());
    let opened = cx.update(move |window, cx| open_naming(&target, &first, window, cx));
    settle(cx);
    assert!(
        opened.is_some() && focused(cx, &start),
        "the premise: the dialog opened on its start"
    );
    shown.set(false);
    redraw(cx);
    assert!(
        cx.debug_bounds("named").is_none(),
        "the premise: the named control is no longer drawn"
    );
    keys(cx, "escape");
    assert!(
        focused(cx, &elsewhere),
        "closing returned focus to what held it before the dialog opened"
    );
}

// ---- A menu naming its return target ---------------------------------------------------------------------------------

/// A menu's content: its start, "menu-first", and "menu-close", whose click closes the menu as a chosen command does.
struct ClosingMenu {
    first: FocusHandle,
    close: FocusHandle,
    /// The menu's own layer, set once it opened.
    layer: Rc<Cell<Option<layers::LayerId>>>,
}

impl Render for ClosingMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layer = self.layer.clone();
        layers::panel(cx)
            .debug_selector(|| "closing-menu".into())
            .w(px(160.))
            .child(button("menu-first", &self.first))
            .child(
                button("menu-close", &self.close).on_click(move |_, window, cx| {
                    if let Some(id) = layer.get() {
                        layers::close(id, window, cx);
                    }
                }),
            )
    }
}

/// A page of a button "elsewhere" and a menu toggle "toggle", whose click opens a [`ClosingMenu`] below it, as a
/// session row's ⋯ does, and the handles a test checks focus against.
struct ToggleScene<'a> {
    cx: &'a mut VisualTestContext,
    elsewhere: FocusHandle,
    /// The toggle's focus handle as last drawn, held weakly. Like a real toggle's, the handle is element state
    /// ([`focus::control_handle`]) that goes when the toggle is no longer drawn, so a test holding it strongly would
    /// keep alive what the app lets die, and hide what happens when only the menu's return target holds it.
    toggle: Rc<RefCell<Option<WeakFocusHandle>>>,
    first: FocusHandle,
    /// Whether the toggle is drawn. The anchor around it stays drawn without it, so its open menu stays open.
    shown: Rc<Cell<bool>>,
}

/// Open a [`ToggleScene`] whose menu returns focus to the toggle when `names_toggle` ([`focus::ReturnTo::Control`]),
/// and otherwise to what held focus when it opened ([`focus::ReturnTo::Opener`]), under macOS's click rule, where a
/// click leaves the toggle unfocused. The toggle does not focus itself before opening, as the app's openers do not.
fn toggle_scene(cx: &mut TestAppContext, names_toggle: bool) -> ToggleScene<'_> {
    let elsewhere = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let toggle: Rc<RefCell<Option<WeakFocusHandle>>> = Rc::new(RefCell::new(None));
    let first = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let close = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let shown = Rc::new(Cell::new(true));
    let (drawn_elsewhere, drawn_toggle, drawn_shown) =
        (elsewhere.clone(), toggle.clone(), shown.clone());
    let (menu_first, menu_close) = (first.clone(), close.clone());
    let cx = open(cx, move |window, cx| {
        let anchor = Anchor::for_element("toggle-anchor", window, cx);
        // Made only while the toggle is drawn, so its element state, and with it the handle, goes when it is not.
        let toggle_focus = drawn_shown
            .get()
            .then(|| focus::control_handle("toggle-focus", window, cx));
        drawn_toggle.replace(toggle_focus.as_ref().map(FocusHandle::downgrade));
        let toggle = toggle_focus.map(|toggle_focus| {
            let opens = anchor.clone();
            let (named, first, close) =
                (toggle_focus.clone(), menu_first.clone(), menu_close.clone());
            button("toggle", &toggle_focus).on_click(move |_, window, cx| {
                let layer = Rc::new(Cell::new(None));
                let view = cx.new(|_| ClosingMenu {
                    first: first.clone(),
                    close: close.clone(),
                    layer: layer.clone(),
                });
                let mark = Mark::take(cx);
                let returns_to = if names_toggle {
                    focus::ReturnTo::Control(named.clone())
                } else {
                    focus::ReturnTo::Opener
                };
                layer.set(layers::open_popover(
                    layers::Popover::menu(view, cx.focus_handle(), opens.clone(), first.clone())
                        .returns_to(returns_to),
                    &mark,
                    window,
                    cx,
                ));
            })
        });
        div()
            .flex()
            .gap(px(8.))
            .child(button("elsewhere", &drawn_elsewhere))
            .child(
                div()
                    .relative()
                    .w(px(80.))
                    .h(px(24.))
                    .children(toggle)
                    .child(anchor.track()),
            )
            .into_any_element()
    });
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    keys(cx, "tab");
    assert!(
        focused(cx, &elsewhere),
        "the premise: focus is on a page control away from the toggle"
    );
    ToggleScene {
        cx,
        elsewhere,
        toggle,
        first,
        shown,
    }
}

/// Whether the scene's toggle holds focus. False once the toggle's handle is gone, which nothing focused can be.
fn toggle_focused(scene: &mut ToggleScene<'_>) -> bool {
    let toggle = scene
        .toggle
        .borrow()
        .as_ref()
        .and_then(WeakFocusHandle::upgrade);
    toggle.is_some_and(|toggle| focused(scene.cx, &toggle))
}

/// Whether the scene's menu was drawn in the last frame.
fn menu_drawn(scene: &mut ToggleScene<'_>) -> bool {
    scene.cx.debug_bounds("closing-menu").is_some()
}

/// Click the scene's toggle and check the premise: the menu opened on its first item, and the toggle never took focus.
fn open_from_toggle(scene: &mut ToggleScene<'_>) {
    click(scene.cx, "toggle");
    assert!(
        focused(scene.cx, &scene.first) && !toggle_focused(scene),
        "the premise: the click opened the menu with focus on its first item"
    );
}

/// SPEC.md: closing something the user opened returns focus to where it was before it opened, on every platform. On
/// macOS the click on a menu's toggle focuses nothing, so what held focus before the menu opened is whatever held it
/// before the click, not the toggle; the menu names its toggle instead, as a dialog names its button. Openers used to
/// focus the toggle first, which costs a whole-window refresh.
///
/// Specified, under macOS's click rule: with focus on another page control, a click on a toggle that opens a menu
/// naming it leaves the toggle unfocused, and Escape returns focus to the toggle. From the same start, a click on the
/// menu's item that closes it returns focus to the toggle too.
#[gpui::test]
fn a_menu_returns_focus_to_the_toggle_its_opener_names(cx: &mut TestAppContext) {
    let mut scene = toggle_scene(cx, true);
    open_from_toggle(&mut scene);
    keys(scene.cx, "escape");
    assert!(
        toggle_focused(&mut scene),
        "Escape returned focus to the toggle, which never held focus"
    );

    keys(scene.cx, "shift-tab");
    assert!(
        focused(scene.cx, &scene.elsewhere),
        "the premise: focus is back on the page control away from the toggle"
    );
    open_from_toggle(&mut scene);
    click(scene.cx, "menu-close");
    assert!(
        !menu_drawn(&mut scene),
        "the premise: the item closed the menu"
    );
    assert!(
        toggle_focused(&mut scene),
        "the item that closed the menu returned focus to the toggle"
    );
}

/// A named toggle can go while its menu is open (the toggle hidden, its anchor still drawn): focusing it would lose
/// focus on the next frame and send it home, where SPEC.md returns it "to where it was before it opened".
///
/// Specified, under macOS's click rule: with focus on a page control, a menu opened from a toggle that names it, whose
/// toggle then stops being drawn while the menu stays open, gives focus back to that page control on Escape.
#[gpui::test]
fn a_menu_naming_a_toggle_no_longer_drawn_returns_focus_to_its_opener(cx: &mut TestAppContext) {
    let mut scene = toggle_scene(cx, true);
    open_from_toggle(&mut scene);
    scene.shown.set(false);
    redraw(scene.cx);
    assert!(
        scene.cx.debug_bounds("toggle").is_none() && menu_drawn(&mut scene),
        "the premise: the toggle is no longer drawn and its menu is still open"
    );
    keys(scene.cx, "escape");
    assert!(
        focused(scene.cx, &scene.elsewhere),
        "closing returned focus to what held it before the menu opened"
    );
}

/// The popovers that keep returning to their opener (a select's list, the template editor's key menu) must keep doing
/// so: only a menu that names a control returns there.
///
/// Specified, under macOS's click rule: with focus on a page control, a menu opened by a click on a toggle, returning
/// to its opener, gives focus back to that page control on Escape, not to the toggle.
#[gpui::test]
fn a_menu_returning_to_its_opener_returns_to_what_held_focus_before_the_click(
    cx: &mut TestAppContext,
) {
    let mut scene = toggle_scene(cx, false);
    open_from_toggle(&mut scene);
    keys(scene.cx, "escape");
    assert!(
        focused(scene.cx, &scene.elsewhere) && !toggle_focused(&mut scene),
        "Escape returned focus to what held it before the click"
    );
}

/// When the toggle that opened a menu held focus as it opened (reached with Tab, or a click on Linux, which focuses
/// it) and is gone by the time the menu closes (its row deleted), the fallback to the opener names the same vanished
/// control. The name's strong handle keeps that opener alive, so focus would land on a control nobody draws. GPUI
/// reports focus lost only on a frame that goes from drawing the focused element to not drawing it, and when the frame
/// on screen at the close already lacks the control (a row's removal closes its menu after the frame that drew
/// neither), nothing reports it: the handle then dies and the window is left with nothing focused. Here the frame on
/// screen still drew the toggle, so the next frame would report the loss and hide the wrong move; the check is made
/// right after the close, before any frame, to see the move itself.
///
/// Specified: a menu opened with Enter on a toggle reached with Tab, naming that toggle, whose toggle then stops being
/// drawn while the menu stays open, leaves focus, as Escape's dismissal closes it, on something still alive (home, or
/// the root when there is none, as in this scene), not on the toggle.
#[gpui::test]
fn a_menu_naming_a_vanished_toggle_that_was_its_opener_recovers_focus(cx: &mut TestAppContext) {
    let mut scene = toggle_scene(cx, true);
    keys(scene.cx, "tab");
    assert!(
        toggle_focused(&mut scene),
        "the premise: Tab moved focus to the toggle"
    );
    keys(scene.cx, "enter");
    assert!(
        focused(scene.cx, &scene.first),
        "the premise: Enter on the toggle opened its menu on its first item"
    );
    let toggle = scene.toggle.borrow().clone();
    scene.shown.set(false);
    redraw(scene.cx);
    assert!(
        scene.cx.debug_bounds("toggle").is_none() && menu_drawn(&mut scene),
        "the premise: the toggle is no longer drawn and its menu is still open"
    );
    let recovered = scene.cx.update(move |window, cx| {
        assert!(
            layers::dismiss_top(window, cx),
            "the premise: Escape took the menu"
        );
        let toggle = toggle.as_ref().and_then(WeakFocusHandle::upgrade);
        window
            .focused(cx)
            .is_some_and(|holder| Some(holder) != toggle)
    });
    assert!(
        recovered,
        "focus recovered rather than landing on the toggle nobody draws"
    );
    check(scene.cx);
}

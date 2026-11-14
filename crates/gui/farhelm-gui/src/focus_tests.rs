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

use gpui::{
    AnyElement, AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, ParentElement, Render,
    ScrollDelta, ScrollEvent, ScrollHandle, StatefulInteractiveElement as _, Styled,
    TestAppContext, VisualTestContext, Window, div, point, px, size,
};

use crate::components::{DialogFrame, HoverHelp, TextInput, TextInputState};
use crate::focus::{self, Mark, OnPress};
use crate::layers::{self, Anchor, Dialog};
use crate::prompt::{self, Presentation};
use crate::test_support::{
    anchor_element, bounds, button, center, check, click, focused, follow, handle, is_open, keys,
    open, open_dialog, open_dialog_returning, open_menu, question, redraw, settle,
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
/// page region opted in through `scroll_region`, as the sidebar's host list will be once the sidebar draws it through
/// that.
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

//! Behavior tests for the focus coordinator, the layer manager and the prompt, on GPUI's simulated platform.
//!
//! Each test builds a window the way the app does (GPUI Kit's `Root` around a root view wired with [`focus::root`] and
//! [`layers::attach`]), drives it through simulated keys and clicks, so key bindings resolve as they do in the app, and
//! runs the harness's invariant checks after every step. The rules come from SPEC.md, "Keyboard focus", and
//! SPEC_impl.md, "Focus", "Overlays" and "Questions".

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, ParentElement,
    Pixels, Point, Render, ScrollDelta, ScrollEvent, Styled, TestAppContext, VisualTestContext,
    Window, div, point, px,
};
use gpui_component::Root;

use crate::cache::{self, Cached};
use crate::components::{Button, HoverHelp, Role, TextInput, TextInputState, Tier};
use crate::focus::{self, Mark, OnPress};
use crate::host_text::{HostSentence, HostText};
use crate::layers::{self, Anchor, Dialog, LayerId, Popover, Subject};
use crate::prompt::{self, Answer, Question};
use crate::{render_count, test_support};

// ---- Harness ---------------------------------------------------------------------------------------------------------

/// What a test draws in its window, given the window's root view state.
type Draw = dyn Fn(&mut Window, &mut App) -> AnyElement;

/// A root view wired like the main view: Farhelm's key context and focus actions, the layer stack, the coordinator.
struct Scene {
    draw: Box<Draw>,
    _layers: layers::Attachment,
    focus: focus::Attachment,
}

impl Render for Scene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("scene", cx);
        focus::root(div(), &self.focus)
            .size_full()
            .flex()
            .flex_col()
            .items_start()
            .p(px(20.))
            .gap(px(8.))
            .child((self.draw)(window, cx))
            .children(layers::overlay(window, cx))
    }
}

/// Open a bootstrapped window drawing `draw` inside Kit's `Root`, as the app does.
fn open(
    cx: &mut TestAppContext,
    draw: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
) -> &mut VisualTestContext {
    test_support::install(cx);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let scene = cx.new(|cx| Scene {
            draw: Box::new(draw),
            _layers: layers::attach(window, cx),
            focus: focus::attach(window, cx),
        });
        Root::new(scene, window, cx)
    });
    // Simulated windows start inactive, and an inactive window has no focus path for keys to follow.
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    check(cx);
    cx
}

/// Run the harness's per-step checks.
fn check(cx: &mut VisualTestContext) {
    cx.update(test_support::check_invariants);
}

/// Redraw after a change the scene's draw function reads from a plain cell, which notifies nothing.
fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    check(cx);
}

/// Press keys, as the platform would deliver them, and check.
fn keys(cx: &mut VisualTestContext, keystrokes: &str) {
    cx.simulate_keystrokes(keystrokes);
    check(cx);
}

/// Click the center of the element drawn under `selector`, and check.
fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = center(cx, selector);
    cx.simulate_click(at, Modifiers::none());
    check(cx);
}

fn center(cx: &mut VisualTestContext, selector: &'static str) -> Point<Pixels> {
    match cx.debug_bounds(selector) {
        Some(bounds) => bounds.center(),
        None => panic!("{selector} was not drawn"),
    }
}

/// A tab-stop focus handle the test owns, to give a control and then ask whether it is focused.
fn handle(cx: &mut VisualTestContext) -> FocusHandle {
    cx.update(|_, cx| cx.focus_handle().tab_stop(true))
}

fn focused(cx: &mut VisualTestContext, handle: &FocusHandle) -> bool {
    cx.update(|window, _| handle.is_focused(window))
}

/// A button the test can find (`id` is its selector) and whose focus handle it holds.
fn button(id: &'static str, focus: &FocusHandle) -> Button {
    Button::new(id, id, HoverHelp::tooltip("A test control"), Role::Button).focus(focus)
}

/// A dialog's content view: some buttons, drawn under its own selector.
struct Panel {
    controls: Vec<(&'static str, FocusHandle)>,
    shown: Rc<Cell<bool>>,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let shown = self.shown.get();
        div()
            .debug_selector(|| "panel".into())
            .w(px(300.))
            .p(px(20.))
            .flex()
            .gap(px(8.))
            .children(
                self.controls
                    .iter()
                    // The last control can be taken away, to test a focused control disappearing.
                    .enumerate()
                    .filter(|(index, _)| shown || *index + 1 < self.controls.len())
                    .map(|(_, (id, focus))| button(id, focus)),
            )
    }
}

/// Open a dialog of `controls`, starting on `start`, for a mark taken now.
fn open_dialog(
    cx: &mut VisualTestContext,
    controls: Vec<(&'static str, FocusHandle)>,
    start: &FocusHandle,
) -> (LayerId, FocusHandle, Rc<Cell<bool>>) {
    let shown = Rc::new(Cell::new(true));
    let container = handle(cx);
    let (start, kept, panel_shown) = (start.clone(), container.clone(), shown.clone());
    let id = cx.update(move |window, cx| {
        let view = cx.new(|_| Panel {
            controls,
            shown: panel_shown,
        });
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container: kept,
                start,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: true,
            },
            &mark,
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    match id {
        Some(id) => (id, container, shown),
        None => panic!("the dialog did not open"),
    }
}

/// A control in a [`Placed`] dialog: a button with a focus handle the test holds, or a text field.
enum PlacedKind {
    Button(FocusHandle),
    Field,
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
                keeps_focus_on_empty_click: true,
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

fn is_open(cx: &mut VisualTestContext, id: LayerId) -> bool {
    cx.update(|window, cx| layers::is_open(id, window, cx))
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

/// A dialog is modal: a click on the page behind it does nothing, because the veil takes it. And in a dialog SPEC.md
/// lists as keeping focus (the launcher, the Stop, Restart, Replace and Delete questions), a click on its empty space
/// keeps focus on the control in use (SPEC.md, "Opening and closing").
#[gpui::test]
fn the_page_behind_a_dialog_takes_no_clicks_and_empty_space_keeps_focus(cx: &mut TestAppContext) {
    let clicks = Rc::new(Cell::new(0));
    let counted = clicks.clone();
    let cx = open(cx, move |_, _| {
        let counted = counted.clone();
        Button::new(
            "behind",
            "Behind",
            HoverHelp::tooltip("A page control"),
            Role::Button,
        )
        .on_click(move |_, _, _| counted.set(counted.get() + 1))
        .into_any_element()
    });
    let behind = center(cx, "behind");
    let only = handle(cx);
    open_dialog(cx, vec![("only", only.clone())], &only);

    cx.simulate_click(behind, Modifiers::none());
    check(cx);
    assert_eq!(
        clicks.get(),
        0,
        "the click behind the dialog reached nothing"
    );

    let panel = match cx.debug_bounds("panel") {
        Some(bounds) => bounds,
        None => panic!("the dialog's panel was not drawn"),
    };
    cx.simulate_click(
        point(panel.left() + px(4.), panel.top() + px(4.)),
        Modifiers::none(),
    );
    check(cx);
    assert!(
        focused(cx, &only),
        "a click on the dialog's empty space kept focus on its control"
    );
}

/// A dialog SPEC.md does not list as keeping focus on an empty-space click (Rename, say) moves focus to the dialog
/// itself on such a click, as a click on a web dialog's background did, and Enter then presses its main action
/// (SPEC.md, "Opening and closing"; label text counts as empty space, since labels are not selectable). The rule is per
/// dialog: the test above covers a dialog that keeps focus, this one a dialog that does not.
#[gpui::test]
fn a_dialog_without_the_rule_takes_focus_on_an_empty_space_click(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let only = handle(cx);
    let container = handle(cx);
    let pressed = Rc::new(Cell::new(0));
    let (start, kept, record) = (only.clone(), container.clone(), pressed.clone());
    let opened = cx.update(move |window, cx| {
        let view = cx.new(|_| Panel {
            controls: vec![("only", start.clone())],
            shown: Rc::new(Cell::new(true)),
        });
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container: kept,
                start,
                subject: None,
                main_action: Some(Rc::new(move |_, _| record.set(record.get() + 1))),
                keeps_focus_on_empty_click: false,
            },
            &mark,
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    assert!(opened.is_some(), "the premise: the dialog opened");
    assert!(focused(cx, &only), "the premise: its control has focus");

    let panel = match cx.debug_bounds("panel") {
        Some(bounds) => bounds,
        None => panic!("the dialog's panel was not drawn"),
    };
    cx.simulate_click(
        point(panel.left() + px(4.), panel.top() + px(4.)),
        Modifiers::none(),
    );
    check(cx);
    assert!(
        focused(cx, &container),
        "a click on the empty space of a dialog without the rule focused the dialog itself"
    );
    keys(cx, "enter");
    assert_eq!(
        pressed.get(),
        1,
        "Enter on the focused dialog pressed its main action"
    );
}

// ---- Which controls Tab reaches -------------------------------------------------------------------------------------

/// Make the coordinator follow `reach` for which controls Tab reaches, as the bootstrap does with the host's answer.
fn follow(cx: &mut VisualTestContext, reach: &Rc<Cell<focus::TabReach>>) {
    let reach = reach.clone();
    cx.update(move |_, cx| focus::follow_tab_reach(Rc::new(move || reach.get()), cx));
}

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

/// Things the user did not start never open over what they are doing: a layer asked for with a mark the user has moved
/// on from does not open.
#[gpui::test]
fn a_layer_does_not_open_once_the_user_moved_on(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let mark = cx.update(|_, cx| Mark::take(cx));
    keys(cx, "x");
    let start = handle(cx);
    let opened = cx.update(|window, cx| {
        let view = cx.new(|_| Panel {
            controls: Vec::new(),
            shown: Rc::new(Cell::new(true)),
        });
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container: start.clone(),
                start,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: true,
            },
            &mark,
            window,
            cx,
        )
    });
    assert_eq!(opened, None, "the user moved on, so nothing opened");
    assert!(!cx.update(|window, cx| layers::any_open(window, cx)));
}

// ---- Layers: subjects, anchors, popovers -----------------------------------------------------------------------------

/// What a test dialog or question is about.
struct Thing {
    name: String,
}

/// A dialog closes when the thing it is about is gone, even though nobody closed it (SPEC.md: each goes away "when the
/// thing it is about is gone"), and focus leaves with it.
#[gpui::test]
fn a_dialog_closes_when_its_subject_is_gone(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let thing = cx.update(|_, cx| {
        cx.new(|_| Thing {
            name: "alpha".into(),
        })
    });
    let start = handle(cx);
    let subject = Subject::of(&thing);
    let id = cx.update(|window, cx| {
        let view = cx.new(|_| Panel {
            controls: vec![("only", start.clone())],
            shown: Rc::new(Cell::new(true)),
        });
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container: cx.focus_handle(),
                start,
                subject: Some(subject),
                main_action: None,
                keeps_focus_on_empty_click: true,
            },
            &mark,
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    let Some(id) = id else {
        panic!("the dialog did not open")
    };

    drop(thing);
    cx.run_until_parked();
    check(cx);
    assert!(!is_open(cx, id), "the dialog closed with its subject");
}

/// A popover menu's own content: its items.
struct Menu {
    items: Vec<(&'static str, FocusHandle)>,
}

impl Render for Menu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        layers::panel(cx)
            .debug_selector(|| "menu".into())
            .w(px(160.))
            .children(self.items.iter().map(|(id, focus)| button(id, focus)))
    }
}

/// Draw an anchor element under `selector` and hand its anchor out through `out`, as a row's menu button will.
fn anchor_element(
    selector: &'static str,
    out: &Rc<RefCell<Option<Anchor>>>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Div {
    let anchor = Anchor::for_element(format!("{selector}-anchor"), window, cx);
    out.replace(Some(anchor.clone()));
    div()
        .relative()
        .debug_selector(move || selector.into())
        .w(px(80.))
        .child(div().h(px(24.)))
        .child(anchor.track())
}

/// Open a menu of `items` below `anchor`, starting on the first, for a mark taken now.
fn open_menu(
    cx: &mut VisualTestContext,
    anchor: Anchor,
    items: Vec<(&'static str, FocusHandle)>,
) -> LayerId {
    let start = items[0].1.clone();
    let id = cx.update(move |window, cx| {
        let view = cx.new(|_| Menu { items });
        let mark = Mark::take(cx);
        layers::open_popover(
            Popover {
                view: view.into(),
                container: cx.focus_handle(),
                start,
                anchor,
            },
            &mark,
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    match id {
        Some(id) => id,
        None => panic!("the menu did not open"),
    }
}

/// A popover is anchored to an element, not to coordinates: when the element moves, the popover is drawn below it in
/// the same frame; when the element is no longer drawn, the popover closes (SPEC_impl.md, "Overlays"). A press outside
/// closes it, but a press on its own anchor is left to the anchor, so a toggle cannot close and reopen it in one click.
#[gpui::test]
fn a_popover_follows_its_anchor_and_closes_when_the_anchor_is_gone(cx: &mut TestAppContext) {
    let pushed_down = Rc::new(Cell::new(false));
    let anchor_shown = Rc::new(Cell::new(true));
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let (down, shown, out) = (
        pushed_down.clone(),
        anchor_shown.clone(),
        anchor_out.clone(),
    );
    let cx = open(cx, move |window, cx| {
        let mut column = div().flex().flex_col();
        // Above the anchor, so the menu below the anchor never covers it.
        column = column.child(div().debug_selector(|| "elsewhere".into()).size(px(40.)));
        if down.get() {
            column = column.child(div().h(px(60.)));
        }
        if shown.get() {
            let anchor = Anchor::for_element("opener-anchor", window, cx);
            out.replace(Some(anchor.clone()));
            // The anchor has content before its tracker, as a real one does (a button), so a tracker that is not
            // pinned to the anchor's corner would record the wrong place.
            column = column.child(
                div()
                    .relative()
                    .debug_selector(|| "opener".into())
                    .w(px(80.))
                    .child(div().h(px(24.)))
                    .child(anchor.track()),
            );
        }
        column.into_any_element()
    });
    let item = handle(cx);
    let anchor = || anchor_out.borrow().clone().unwrap();
    let id = open_menu(cx, anchor(), vec![("item", item.clone())]);
    let first = cx.debug_bounds("menu").unwrap();
    let opener = cx.debug_bounds("opener").unwrap();
    assert_eq!(
        (first.top(), first.left()),
        (opener.bottom() + px(4.), opener.left()),
        "the menu sits just below its anchor, aligned with it: {first:?} under {opener:?}"
    );

    pushed_down.set(true);
    redraw(cx);
    let moved = cx.debug_bounds("menu").unwrap();
    let opener = cx.debug_bounds("opener").unwrap();
    assert!(is_open(cx, id), "moving the anchor does not close the menu");
    assert_eq!(
        moved.top() - first.top(),
        px(60.),
        "the menu moved with its anchor in the same frame"
    );
    assert!(moved.top() >= opener.bottom());

    click(cx, "opener");
    assert!(
        is_open(cx, id),
        "a press on the anchor itself is left to the anchor"
    );
    click(cx, "elsewhere");
    assert!(!is_open(cx, id), "a press outside closed the menu");

    let id = open_menu(cx, anchor(), vec![("item", item.clone())]);
    anchor_shown.set(false);
    redraw(cx);
    assert!(
        !is_open(cx, id),
        "the menu closed when its anchor was no longer drawn"
    );
}

/// Escape closes only the topmost layer: a menu open over a dialog closes, and the dialog stays.
#[gpui::test]
fn escape_closes_a_menu_over_a_dialog_and_leaves_the_dialog(cx: &mut TestAppContext) {
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let out = anchor_out.clone();
    let cx = open(cx, move |window, cx| {
        anchor_element("opener", &out, window, cx).into_any_element()
    });
    let only = handle(cx);
    let (dialog, _, _) = open_dialog(cx, vec![("only", only.clone())], &only);
    let item = handle(cx);
    let anchor = anchor_out.borrow().clone().unwrap();
    let menu = open_menu(cx, anchor, vec![("item", item)]);
    keys(cx, "escape");
    assert!(!is_open(cx, menu), "Escape closed the menu");
    assert!(is_open(cx, dialog), "and left the dialog under it");
    assert!(focused(cx, &only), "focus went back into the dialog");
}

/// SPEC.md: "Inside a dialog or menu, focus stays inside." With a menu open over the page, Tab and Shift+Tab move
/// between the menu's items and never out to the page behind it, which would leave the menu open with nothing of it
/// focused.
#[gpui::test]
fn tab_stays_inside_an_open_menu(cx: &mut TestAppContext) {
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let page = Rc::new(RefCell::new(None::<FocusHandle>));
    let (out, drawn) = (anchor_out.clone(), page.clone());
    let cx = open(cx, move |window, cx| {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(drawn.borrow().clone().map(|focus| button("page", &focus)))
            .child(anchor_element("opener", &out, window, cx))
            .into_any_element()
    });
    let behind = handle(cx);
    page.replace(Some(behind.clone()));
    redraw(cx);
    let (first, second) = (handle(cx), handle(cx));
    let anchor = anchor_out.borrow().clone().unwrap();
    open_menu(
        cx,
        anchor,
        vec![("first", first.clone()), ("second", second.clone())],
    );
    assert!(
        focused(cx, &first),
        "the premise: the menu starts on its first item"
    );

    for _ in 0..3 {
        keys(cx, "tab");
        assert!(
            !focused(cx, &behind),
            "Tab never reaches the page behind the menu"
        );
    }
    assert!(
        focused(cx, &second),
        "three Tabs over two items land on the second"
    );
    keys(cx, "shift-tab");
    assert!(focused(cx, &first), "Shift+Tab moves back inside the menu");
}

/// The input a cached view draws: a stand-in for a store entry.
struct Source(String);

/// Opening and closing a layer redraws the root that draws the overlay, never a cached view under it: the layer stack
/// notifies only the root (SPEC_impl.md, "The store, and how views redraw"). A cached view on the page is the witness: a
/// whole-window refresh would re-render it.
///
/// A focus move refreshes the whole window in GPUI whatever the layer manager does (SPEC_impl.md's cookbook), so the
/// dialog here starts on the page control that already has focus: opening and closing it then moves no focus, and any
/// redraw of the cached view could only come from the layer stack. (Focus outside an open dialog breaks the focus
/// invariant, so this one step is not checked; the invariant holds again once the dialog is closed.)
#[gpui::test]
fn opening_and_closing_a_layer_redraws_only_the_root(cx: &mut TestAppContext) {
    type Row = Cached<Source, String>;
    let row: Rc<RefCell<Option<Entity<Row>>>> = Rc::new(RefCell::new(None));
    let page = Rc::new(RefCell::new(None::<FocusHandle>));
    let (drawn_row, drawn_page) = (row.clone(), page.clone());
    let cx = open(cx, move |_, _| {
        let mut size = gpui::StyleRefinement::default();
        size.size.width = Some(px(200.).into());
        size.size.height = Some(px(20.).into());
        div()
            .flex()
            .flex_col()
            .children(
                drawn_page
                    .borrow()
                    .clone()
                    .map(|focus| button("page", &focus)),
            )
            .children(
                drawn_row
                    .borrow()
                    .as_ref()
                    .map(|row| cache::element(row, size)),
            )
            .into_any_element()
    });
    let made = cx.update(|_, cx| {
        let source = cx.new(|_| Source("a row".into()));
        cx.new(|cx| {
            Cached::new(
                "row",
                source,
                |source: &Source| source.0.clone(),
                |text, _, _| div().child(text.clone()).into_any_element(),
                cx,
            )
        })
    });
    row.replace(Some(made));
    let on_page = handle(cx);
    page.replace(Some(on_page.clone()));
    redraw(cx);
    keys(cx, "tab");
    assert!(
        focused(cx, &on_page),
        "the premise: the page control has focus"
    );

    cx.update(|_, cx| render_count::reset(cx));
    let start = on_page.clone();
    let id = cx.update(move |window, cx| {
        let view = cx.new(|_| Panel {
            controls: Vec::new(),
            shown: Rc::new(Cell::new(true)),
        });
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container: cx.focus_handle(),
                start,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: true,
            },
            &mark,
            window,
            cx,
        )
    });
    cx.run_until_parked();
    let Some(id) = id else {
        panic!("the dialog did not open")
    };
    assert!(focused(cx, &on_page), "the premise: opening moved no focus");
    assert!(
        cx.update(|_, cx| render_count::count("scene", cx)) > 0,
        "the premise: the root redrew to draw the dialog"
    );
    cx.update(|window, cx| layers::close(id, window, cx));
    cx.run_until_parked();
    check(cx);
    assert!(!is_open(cx, id));
    assert!(focused(cx, &on_page), "the premise: closing moved no focus");
    assert_eq!(
        cx.update(|_, cx| render_count::count("row", cx)),
        0,
        "opening and closing the dialog redrew no cached view"
    );
}

// ---- The prompt ------------------------------------------------------------------------------------------------------

/// A question about a thing, frozen at `name`.
fn question(name: &str) -> Question<String> {
    Question::new(
        "Delete it?",
        Answer {
            label: "Delete".into(),
            help: HoverHelp::tooltip("Delete it for good"),
            tier: Tier::Danger,
        },
        name.to_string(),
    )
    .summary(name.to_string())
    .line("Its history is removed.")
}

/// SPEC_impl.md's generic prompt test, for the one prompt kind there is: change the confirmation's subject after it
/// opens, then send Cancel and Confirm with no draw between them, and check that any command sent names the frozen
/// target. Whatever runs acts on what the user was shown. The first answer wins, and Confirm runs at most once.
///
/// Here the frozen value is a plain copy, so the "subject changes" step cannot make the prompt act on something else and
/// cannot fail: the prompt never sees the live subject. That holds by convention, not by the type: `Question<T>` would
/// as happily carry a live handle (an entity, an `Rc<RefCell<_>>`), and a feature's `on_confirm` could capture live
/// state and ignore its argument. The step gains teeth when each real prompt kind runs it with its own subject and
/// command (SPEC_impl.md, "Questions: the prompt"); this test's teeth are the answer ordering (without the
/// first-answer-wins guard, Cancel then Confirm would send).
#[gpui::test]
fn a_confirmation_acts_only_on_what_it_showed(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let thing = cx.update(|_, cx| {
        cx.new(|_| Thing {
            name: "alpha".into(),
        })
    });
    let sent = Rc::new(RefCell::new(Vec::<String>::new()));

    for order in [
        ["cancel", "confirm"],
        ["confirm", "cancel"],
        ["confirm", "confirm"],
    ] {
        thing.update(cx, |thing, _| thing.name = "alpha".into());
        sent.borrow_mut().clear();
        let shown = thing.read_with(cx, |thing, _| thing.name.clone());
        let record = sent.clone();
        let asked = cx.update(|window, cx| {
            let mark = Mark::take(cx);
            prompt::ask_for_test(
                question(&shown),
                Some(Subject::of(&thing)),
                &mark,
                move |frozen, _, _| record.borrow_mut().push(frozen),
                window,
                cx,
            )
        });
        cx.run_until_parked();
        check(cx);
        let asked = asked.expect("the question opened");

        // The subject changes after the question opened.
        thing.update(cx, |thing, _| thing.name = "beta".into());
        // Both answers, with no draw between them.
        cx.update(|window, cx| {
            for answer in order {
                match answer {
                    "cancel" => asked.cancel(window, cx),
                    _ => asked.confirm(window, cx),
                }
            }
        });
        cx.run_until_parked();
        check(cx);
        let expected: Vec<String> = if order[0] == "confirm" {
            vec!["alpha".into()]
        } else {
            Vec::new()
        };
        assert_eq!(
            *sent.borrow(),
            expected,
            "answers {order:?}: only the first ran, on the frozen target"
        );
        assert!(!is_open(cx, asked.layer), "the question closed");
    }
}

/// One start-rule case: its name, how the question is adjusted, and whether focus starts on the confirming answer.
type StartCase = (&'static str, fn(Question<String>) -> Question<String>, bool);

/// Where a question starts (SPEC.md, "Opening and closing"): on its confirming button when the user just asked for it,
/// on Cancel when that button is disabled, and when the action is risky or the question arrived later (one builder,
/// [`Question::starts_on_cancel`], covers both). Enter then
/// answers what has focus, so a wrong start is a wrong answer.
#[gpui::test]
fn a_question_starts_on_the_answer_the_rules_pick(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let cases: [StartCase; 3] = [
        ("asked for", |q| q, true),
        ("risky or arrived later", Question::starts_on_cancel, false),
        ("disabled", |q| q.confirm_enabled(false), false),
    ];
    for (case, adjust, starts_on_confirm) in cases {
        let sent = Rc::new(Cell::new(0));
        let record = sent.clone();
        let asked = cx.update(|window, cx| {
            let mark = Mark::take(cx);
            prompt::ask_for_test(
                adjust(question("alpha")),
                None,
                &mark,
                move |_, _, _| record.set(record.get() + 1),
                window,
                cx,
            )
        });
        cx.run_until_parked();
        check(cx);
        let asked = asked.expect("the question opened");
        keys(cx, "enter");
        assert_eq!(
            sent.get(),
            usize::from(starts_on_confirm),
            "{case}: Enter answered the start"
        );
        assert!(
            !is_open(cx, asked.layer),
            "{case}: Enter answered and closed it"
        );
    }
}

/// Which questions keep focus on the answer in use when their empty space is clicked is each question's own rule
/// ([`Question::keeps_focus_on_empty_click`]): SPEC.md gives it to the Stop, Restart, Replace and Delete questions.
/// Without it a click on the question's text focuses the question itself, and a following Enter presses Confirm; with
/// it, focus stays on Cancel (where a risky question starts) and Enter cancels. The opt-in's plumbing through the
/// question to its dialog is what this guards: lose it and those four questions confirm on a stray click and Enter.
#[gpui::test]
fn a_question_that_opts_in_keeps_focus_on_an_empty_space_click(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    for (case, keeps, sent_after_enter) in [("opted in", true, 0), ("not opted in", false, 1)] {
        let sent = Rc::new(Cell::new(0));
        let record = sent.clone();
        let asked = cx.update(|window, cx| {
            let mark = Mark::take(cx);
            let question = question("alpha").starts_on_cancel();
            let question = if keeps {
                question.keeps_focus_on_empty_click()
            } else {
                question
            };
            prompt::ask_for_test(
                question,
                None,
                &mark,
                move |_, _, _| record.set(record.get() + 1),
                window,
                cx,
            )
        });
        cx.run_until_parked();
        check(cx);
        let asked = asked.expect("the question opened");
        let body = match cx.debug_bounds("prompt-body") {
            Some(bounds) => bounds,
            None => panic!("{case}: the question's body was not drawn"),
        };
        cx.simulate_click(
            point(body.left() + px(4.), body.top() + px(4.)),
            Modifiers::none(),
        );
        check(cx);
        assert!(
            is_open(cx, asked.layer),
            "{case}: the click on its empty space left the question open"
        );
        keys(cx, "enter");
        assert_eq!(
            sent.get(),
            sent_after_enter,
            "{case}: Enter after the click answered what kept focus"
        );
    }
}

/// Escape cancels a question and returns focus to where it was; nothing is sent.
#[gpui::test]
fn escape_cancels_a_question_and_returns_focus(cx: &mut TestAppContext) {
    let target = Rc::new(RefCell::new(None));
    let drawn = target.clone();
    let cx = open(cx, move |_, _| {
        div()
            .children(
                drawn
                    .borrow()
                    .clone()
                    .map(|focus| button("row-delete", &focus)),
            )
            .into_any_element()
    });
    let opener = handle(cx);
    target.replace(Some(opener.clone()));
    redraw(cx);
    keys(cx, "tab");
    let sent = Rc::new(Cell::new(false));
    let record = sent.clone();
    let layer = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask(
            question("alpha"),
            None,
            &mark,
            move |_, _, _| record.set(true),
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    keys(cx, "escape");
    assert!(!sent.get(), "Escape sent nothing");
    assert!(!is_open(cx, layer.unwrap()), "Escape closed the question");
    assert!(
        focused(cx, &opener),
        "focus returned to the control that opened it"
    );
}

/// Enter while the dialog itself, rather than one of its controls, has focus presses its enabled main action (SPEC.md,
/// "Opening and closing"); a held Enter does not.
#[gpui::test]
fn enter_on_the_dialog_itself_presses_its_main_action(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let pressed = Rc::new(Cell::new(0));
    let record = pressed.clone();
    let container = handle(cx);
    let kept = container.clone();
    cx.update(move |window, cx| {
        let view = cx.new(|_| Panel {
            controls: Vec::new(),
            shown: Rc::new(Cell::new(true)),
        });
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container: kept.clone(),
                start: kept,
                subject: None,
                main_action: Some(Rc::new(move |_, _| record.set(record.get() + 1))),
                keeps_focus_on_empty_click: true,
            },
            &mark,
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    assert!(
        focused(cx, &container),
        "the premise: the dialog itself has focus"
    );
    cx.simulate_event(KeyDownEvent {
        native_key: None,
        keystroke: Keystroke::parse("enter").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    assert_eq!(pressed.get(), 0, "a held Enter did not press it");
    keys(cx, "enter");
    assert_eq!(pressed.get(), 1, "Enter pressed the main action");
}

/// A question about a thing closes when the thing is gone (a session deleted from another window while its delete
/// confirmation was open), and nothing is sent.
#[gpui::test]
fn a_question_closes_when_its_subject_is_gone(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let thing: Entity<Thing> = cx.update(|_, cx| {
        cx.new(|_| Thing {
            name: "alpha".into(),
        })
    });
    let sent = Rc::new(Cell::new(false));
    let record = sent.clone();
    let subject = Subject::of(&thing);
    let layer = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask(
            question("alpha"),
            Some(subject),
            &mark,
            move |_, _, _| record.set(true),
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    drop(thing);
    cx.run_until_parked();
    check(cx);
    assert!(
        !is_open(cx, layer.unwrap()),
        "the question closed with its subject"
    );
    assert!(!sent.get());
}

/// Ask `question` with no subject, draw it with GPUI's driver record on, and hand back that frame's record: every node
/// drawn and the text each painted. The question stays open; the caller answers it.
fn ask_and_record(
    cx: &mut VisualTestContext,
    question: Question<String>,
) -> (prompt::Asked<String>, Vec<gpui::DriverNode>) {
    cx.update(|window, _| window.set_driver_enabled(true));
    let asked = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        prompt::ask_for_test(question, None, &mark, |_, _, _| {}, window, cx)
    });
    let Some(asked) = asked else {
        panic!("the question did not open")
    };
    redraw(cx);
    let nodes = cx.update(|window, _| window.driver_nodes().to_vec());
    (asked, nodes)
}

/// Why this matters: a prompt's summary is where a confirmation names its target, and the first real one (a session's
/// delete confirmation) names a session whose title a host wrote. When the summary took only a string, the one way to
/// put that title there was its raw value, so an override character in it would reverse the summary on screen and a
/// zero-width one would make two sessions' confirmations look identical, to a screen reader too (SPEC.md: "including
/// in accessible names"). A long title must wrap, not run past the dialog's edge: while a sentence's parts could not
/// shrink below their one-line width, a 108-character title was clipped at the border, so two sessions whose titles
/// share a long start got confirmations that looked the same, the very problem escaping exists to prevent. The
/// plain-string case guards the other direction: every existing question passes app wording as a string, and it must
/// draw as it did before.
///
/// Specification: a question whose summary is a [`HostSentence`] paints the host value in its escaped presentation,
/// and no text the frame paints holds a presentation-unsafe character; inside the dialog the sentence is one `Label`
/// node under its id, named with the value escaped; the dialog's own accessible name is still the question's title.
/// With a summary naming a title too long for one line and a sentence line of app words too long for one line, every
/// text painted inside the dialog lies within the dialog's left and right edges and is not clipped. A question whose
/// summary is a plain string paints that string as one line, exactly as given.
#[gpui::test]
fn a_summary_that_names_a_host_value_is_drawn_and_named_escaped(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    // Inside the dialog drawn under `prompt`, at any depth.
    let inside_dialog = |nodes: &[gpui::DriverNode], mut index: usize| {
        while let Some(parent) = nodes[index].parent {
            if nodes[parent].element_id.as_deref() == Some("prompt") {
                return true;
            }
            index = parent;
        }
        false
    };
    // The accessible name of the dialog drawn under `prompt`, if it was recorded as a dialog.
    let dialog_label = |nodes: &[gpui::DriverNode]| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some("prompt"))
            .filter(|node| node.role == Some(Role::Dialog))
            .and_then(|node| node.label.clone())
    };
    // Every line of text the frame painted, in record order.
    let painted = |nodes: &[gpui::DriverNode]| -> Vec<String> {
        nodes
            .iter()
            .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
            .collect()
    };

    // A host value with a right-to-left override and a zero-width space, in the summary.
    let title = HostText::from_host("invoice\u{202E}fdp\u{200B}.exe");
    let summary = HostSentence::new("prompt-summary")
        .text("Delete ")
        .host(&title)
        .text(" on this machine?");
    let (asked, nodes) = ask_and_record(cx, question("alpha").summary(summary));
    let texts = painted(&nodes);
    assert!(
        texts
            .iter()
            .any(|text| text == "invoice<U+202E>fdp<U+200B>.exe"),
        "the host value is painted escaped; painted: {texts:?}"
    );
    assert!(
        !texts.iter().any(|text| text
            .chars()
            .any(farhelm_proto::text::is_presentation_unsafe)),
        "nothing painted holds a hidden character; painted: {texts:?}"
    );
    let Some(sentence) = nodes
        .iter()
        .position(|node| node.element_id.as_deref() == Some("prompt-summary"))
    else {
        let ids: Vec<_> = nodes.iter().map(|node| &node.element_id).collect();
        panic!("the summary sentence was not recorded; recorded ids: {ids:?}");
    };
    assert!(
        inside_dialog(&nodes, sentence),
        "the sentence is drawn inside the dialog"
    );
    assert_eq!(nodes[sentence].role, Some(Role::Label));
    assert_eq!(
        nodes[sentence].label.as_deref(),
        Some("Delete invoice<U+202E>fdp<U+200B>.exe on this machine?")
    );
    assert_eq!(dialog_label(&nodes).as_deref(), Some("Delete it?"));
    cx.update(|window, cx| asked.cancel(window, cx));
    cx.run_until_parked();
    check(cx);

    // App wording as a string, as every question before host sentences passed it.
    let (asked, nodes) = ask_and_record(cx, question("api-refactor, on this machine"));
    let texts = painted(&nodes);
    assert!(
        texts
            .iter()
            .any(|text| text == "api-refactor, on this machine"),
        "the string summary is painted as given; painted: {texts:?}"
    );
    assert_eq!(dialog_label(&nodes).as_deref(), Some("Delete it?"));
    cx.update(|window, cx| asked.cancel(window, cx));
    cx.run_until_parked();
    check(cx);

    // A title with no spaces, far wider than the dialog, and a line of app words that is too.
    let long = "a-very-long-session-title-that-a-host-wrote-without-any-spaces-\
                at-all-so-it-cannot-wrap-at-a-word-boundary";
    let words = "The session's terminal and its history are removed, and the working directory stays where it is on \
                 the host.";
    let summary = HostSentence::new("prompt-summary")
        .text("Delete ")
        .host(&HostText::from_host(long))
        .text(" on this machine?");
    let line = HostSentence::new("prompt-line").text(words);
    let (asked, nodes) = ask_and_record(cx, question("alpha").summary(summary).line(line));
    let Some(dialog) = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("prompt"))
    else {
        panic!("the dialog was not recorded")
    };
    let inside: Vec<_> = (0..nodes.len())
        .filter(|&index| inside_dialog(&nodes, index))
        .flat_map(|index| nodes[index].texts.iter())
        .collect();
    for expected in [long, words] {
        assert!(
            inside.iter().any(|text| text.text.as_ref() == expected),
            "the premise: {expected:?} is painted inside the dialog"
        );
    }
    // Text bounds are floats from layout, so allow half a pixel of rounding at each edge.
    let slack = px(0.5);
    for text in &inside {
        assert!(
            text.bounds.left() >= dialog.bounds.left() - slack
                && text.bounds.right() <= dialog.bounds.right() + slack,
            "{:?} is painted within the dialog's edges: text {:?}, dialog {:?}",
            text.text,
            text.bounds,
            dialog.bounds
        );
        assert!(
            text.visible_bounds.size.width >= text.bounds.size.width - slack,
            "{:?} is not clipped: painted {:?}, visible {:?}",
            text.text,
            text.bounds,
            text.visible_bounds
        );
    }
    cx.update(|window, cx| asked.cancel(window, cx));
    cx.run_until_parked();
    check(cx);
}

//! Behavior tests for the layer manager, on GPUI's simulated platform: the modal dialog (its veil, its empty-space
//! rule, its main action and its dismissal hook), the refusal to open a layer once the user has moved on, subjects
//! and anchors, popovers and menus, the pill and the approval region drawn above every layer without being one (and a
//! layer raised above the region), and that the layer stack redraws only the root. How a tooltip lays out help in
//! paragraphs is tested beside the tooltip, in `layers/tooltip_tests.rs`.
//!
//! Each test opens its window through the shared harness ([`crate::test_support::open`]: GPUI Kit's `Root` around a
//! root view wired as the app's), drives it through simulated keys and clicks, so key bindings resolve as they do in
//! the app, and runs the harness's invariant checks after every step. The test of a session as a subject also serves
//! the helm client's fake, so the listings that do or do not prove the session gone are the ones the test pushes. The
//! rules come from SPEC.md, "Keyboard focus", and SPEC_impl.md, "Overlays".

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, Keystroke, Modifiers, MouseButton, ParentElement, Render,
    StatefulInteractiveElement as _, Styled, TestAppContext, VisualTestContext, Window, div, point,
    prelude::FluentBuilder as _, px,
};

use crate::cache::{self, Cached};
use crate::components::{Button, HoverHelp, Role, TextInput, TextInputState, Tier};
use crate::focus::{self, Mark};
use crate::layers::{
    self, Anchor, Dialog, DialogPlacement, Dismissal, LayerId, Pill, PillId, PillPlacement,
    Popover, Subject,
};
use crate::prompt;
use crate::render_count;
use crate::test_support::{
    Panel, Thing, anchor_element, bounds, button, center, check, click, focused, follow, frame,
    handle, is_open, keys, open, open_dialog, open_menu, question, redraw, settle,
};

// ---- Layers: a dialog is modal --------------------------------------------------------------------------------------

/// A dialog is modal: a click on the page behind it does nothing, because the veil takes it. And in a dialog SPEC.md
/// lists as keeping focus (the launcher, the Stop, Restart, Replace and Delete questions), a click on its empty space
/// keeps focus on the control in use (SPEC.md, "Opening and closing"). So does a right press there: GPUI's own
/// focus-on-press takes a press of any mouse button, and once focused the dialog's container on a right press.
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

    let empty = point(panel.left() + px(4.), panel.top() + px(4.));
    cx.simulate_mouse_down(empty, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(empty, MouseButton::Right, Modifiers::none());
    check(cx);
    assert!(
        focused(cx, &only),
        "a right press on the dialog's empty space kept focus on its control"
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

/// Why this matters: an editor whose save is in flight must not be dismissed, or the draft and the helm's refusal are
/// lost (the Rename editor: while a save is in flight, Escape does nothing). The editor's own view sees Escape only
/// while focus is inside it, not while the dialog itself holds focus after a click on its empty space, so the hook
/// belongs to the layer manager, which handles Escape there.
///
/// Specification: while a dialog's dismissal hook answers Stay, Escape leaves it open and focus where it was, both from
/// a control inside it and from the dialog itself holding focus, and the layer manager reports Escape as taken, so it
/// is not passed on; once the hook answers Close, Escape closes it.
#[gpui::test]
fn a_dialog_that_may_not_be_dismissed_ignores_escape(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    let busy = Rc::new(Cell::new(true));
    let (only, container) = (handle(cx), handle(cx));
    let (start, kept, guard) = (only.clone(), container.clone(), busy.clone());
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
                main_action: None,
                keeps_focus_on_empty_click: false,
                returns_to: focus::ReturnTo::Opener,
                on_dismiss: Some(Rc::new(move |_, _| {
                    if guard.get() {
                        Dismissal::Stay
                    } else {
                        Dismissal::Close
                    }
                })),
                placement: DialogPlacement::Centered,
                dismiss_on_outside_press: false,
            },
            &mark,
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    let Some(dialog) = opened else {
        panic!("the premise: the dialog opened");
    };
    assert!(focused(cx, &only), "the premise: its control has focus");
    keys(cx, "escape");
    assert!(is_open(cx, dialog), "Escape from its control left it open");
    assert!(focused(cx, &only), "and focus where it was");

    let panel = bounds(cx, "panel");
    cx.simulate_click(
        point(panel.left() + px(4.), panel.top() + px(4.)),
        Modifiers::none(),
    );
    check(cx);
    assert!(
        focused(cx, &container),
        "the premise: the dialog itself holds focus"
    );
    keys(cx, "escape");
    assert!(
        is_open(cx, dialog),
        "Escape from the dialog itself left it open"
    );
    assert!(
        cx.update(layers::dismiss_top),
        "the guarded dialog took Escape, so it goes on to nothing behind it"
    );

    busy.set(false);
    keys(cx, "escape");
    assert!(
        !is_open(cx, dialog),
        "once it may be dismissed, Escape closed it"
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

// ---- "The user moved on" ---------------------------------------------------------------------------------------------

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
    assert_eq!(opened, None, "the user moved on, so nothing opened");
    assert!(!cx.update(|window, cx| layers::any_open(window, cx)));
}

// ---- Layers: subjects, anchors, popovers -----------------------------------------------------------------------------

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
    let Some(id) = id else {
        panic!("the dialog did not open")
    };

    drop(thing);
    cx.run_until_parked();
    check(cx);
    assert!(!is_open(cx, id), "the dialog closed with its subject");
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

// ---- Layers: a dialog's placement and an outside press ---------------------------------------------------------------

/// A dialog's content taller than the window that, like a dialog with a scrolling list, can shrink to the room it is
/// given (`flex_1` and `min_h_0`): what the top placement's height cap is for.
struct TallPanel {
    control: FocusHandle,
}

impl Render for TallPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "panel".into())
            .w(px(300.))
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(button("only", &self.control))
            .child(div().flex_none().h(px(4000.)))
    }
}

/// What a test of a dialog's options holds on to: the window, the dialog's layer, the page opener's and the dialog
/// control's focus handles, how many presses the page's corner button got, and an anchor on the page for a menu.
struct OptionDialog<'a> {
    cx: &'a mut VisualTestContext,
    dialog: LayerId,
    opener: FocusHandle,
    only: FocusHandle,
    corner_presses: Rc<Cell<usize>>,
    menu_anchor: Anchor,
}

/// Which content a test dialog draws.
#[derive(Clone, Copy)]
enum Tall {
    /// One control.
    No,
    /// Taller than the window, but able to shrink to the room it gets ([`TallPanel`]).
    Shrinking,
}

/// A dialog's placement and whether an outside press dismisses it ([`Dialog::placement`] and
/// [`Dialog::dismiss_on_outside_press`]), as a test opens it.
type Options = (DialogPlacement, bool);

/// Every dialog's ordinary form: centered, and an outside press does nothing.
const ORDINARY: Options = (DialogPlacement::Centered, false);

/// The quick switcher's form: near the top, and an outside press dismisses it.
const SWITCHER: Options = (DialogPlacement::Top, true);

/// Open a dialog over a page whose opener button holds focus, with `options`, its one control `only` starting focused,
/// and `guard` as its dismissal hook; `tall` makes its content taller than the window. The page also has a button in its
/// bottom-left corner that counts presses (`page-corner`) and an anchor for a menu at its top left.
fn open_option_dialog(
    cx: &mut TestAppContext,
    options: Options,
    guard: Option<layers::DismissHook>,
    tall: Tall,
) -> OptionDialog<'_> {
    let page: Rc<RefCell<Option<FocusHandle>>> = Rc::new(RefCell::new(None));
    let presses = Rc::new(Cell::new(0));
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let (drawn, counted, out) = (page.clone(), presses.clone(), anchor_out.clone());
    let cx = open(cx, move |window, cx| {
        let counted = counted.clone();
        div()
            .size_full()
            .child(anchor_element("menu-anchor", &out, window, cx))
            .children(drawn.borrow().clone().map(|focus| button("opener", &focus)))
            .child(
                div().absolute().bottom_0().left_0().child(
                    Button::new(
                        "page-corner",
                        "Corner",
                        HoverHelp::tooltip("A page control"),
                        Role::Button,
                    )
                    .on_click(move |_, _, _| counted.set(counted.get() + 1)),
                ),
            )
            .into_any_element()
    });
    let opener = handle(cx);
    page.replace(Some(opener.clone()));
    redraw(cx);
    keys(cx, "tab");
    assert!(focused(cx, &opener), "the premise: the opener holds focus");
    let (only, container) = (handle(cx), handle(cx));
    let (start, kept) = (only.clone(), container.clone());
    let opened = cx.update(move |window, cx| {
        let view: gpui::AnyView = match tall {
            Tall::Shrinking => cx
                .new(|_| TallPanel {
                    control: start.clone(),
                })
                .into(),
            Tall::No => cx
                .new(|_| Panel {
                    controls: vec![("only", start.clone())],
                    shown: Rc::new(Cell::new(true)),
                })
                .into(),
        };
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view,
                container: kept,
                start,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: false,
                returns_to: focus::ReturnTo::Opener,
                on_dismiss: guard,
                placement: options.0,
                dismiss_on_outside_press: options.1,
            },
            &mark,
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    let Some(dialog) = opened else {
        panic!("the premise: the dialog opened");
    };
    let menu_anchor = anchor_out
        .borrow()
        .clone()
        .expect("the page drew its menu anchor");
    OptionDialog {
        cx,
        dialog,
        opener,
        only,
        corner_presses: presses,
        menu_anchor,
    }
}

/// The old quick switcher sat near the top of the window rather than centered, so it did not jump as its result list
/// grew, and never grew past three quarters of the window (`.quick-switcher-backdrop` at the reference commit).
/// Specified: a dialog opened with the top placement is centered across, its top `min(12% of the window height, 80 px)`
/// from the window's top (80 px in a tall window, 12% in a short one), and a content taller than the window is held to
/// 75% of the window's height.
#[gpui::test]
fn a_dialog_placed_at_the_top_sits_near_the_top_and_is_capped(cx: &mut TestAppContext) {
    let cx = open_option_dialog(cx, SWITCHER, None, Tall::Shrinking).cx;
    let viewport = cx.update(|window, _| window.viewport_size());
    let panel = bounds(cx, "panel");
    assert!(
        viewport.height * 0.12 > px(80.),
        "the premise: a tall window, where the 80 px bound applies"
    );
    assert_eq!(panel.top(), px(80.), "its top: {panel:?} in {viewport:?}");
    assert_eq!(
        panel.left(),
        viewport.width - panel.right(),
        "centered across: {panel:?}"
    );
    assert!(
        panel.size.height <= viewport.height * 0.75 && panel.size.height > px(100.),
        "its height is held to 75% of the window's: {panel:?} in {viewport:?}"
    );
    cx.simulate_resize(gpui::size(px(800.), px(400.)));
    crate::test_support::settle(cx);
    let panel = bounds(cx, "panel");
    assert_eq!(
        panel.top(),
        px(48.),
        "in a 400 px window, 12% of its height: {panel:?}"
    );
    assert!(
        panel.size.height <= px(300.),
        "and still at most 75% of it: {panel:?}"
    );
}

/// The same dialog centered, for contrast: the default placement is unchanged. Specified: its top and bottom margins
/// are equal.
#[gpui::test]
fn a_dialog_placed_by_default_is_centered(cx: &mut TestAppContext) {
    let cx = open_option_dialog(cx, ORDINARY, None, Tall::No).cx;
    let viewport = cx.update(|window, _| window.viewport_size());
    let panel = bounds(cx, "panel");
    assert_eq!(
        panel.top(),
        viewport.height - panel.bottom(),
        "centered top to bottom: {panel:?}"
    );
}

/// The old quick switcher closed on a press outside it. Specified: with the option, a press on the veil outside the
/// dialog closes it, as Escape would, returning focus to its opener, and the page control under the press never sees
/// it (the veil takes it); a press inside the dialog leaves it open.
#[gpui::test]
fn a_press_outside_a_dialog_that_asks_for_it_dismisses_it(cx: &mut TestAppContext) {
    let OptionDialog {
        cx,
        dialog,
        opener,
        only,
        corner_presses,
        ..
    } = open_option_dialog(cx, SWITCHER, None, Tall::No);
    assert!(
        focused(cx, &only),
        "the premise: focus starts in the dialog"
    );
    click(cx, "panel");
    assert!(
        is_open(cx, dialog),
        "a press inside the dialog left it open"
    );
    click(cx, "page-corner");
    assert!(!is_open(cx, dialog), "a press outside closed it");
    assert!(
        focused(cx, &opener),
        "focus went back to the opener, as for Escape"
    );
    assert_eq!(
        corner_presses.get(),
        0,
        "the page control under the press never saw it"
    );
}

/// A press inside a menu opened over such a dialog is outside the dialog too, but it is the menu's: dismissing from
/// the dialog's handler would close the top layer, the menu, before its item could take the press. Specified: with a
/// menu open over a dialog that closes on an outside press, a press on the menu's item, drawn outside the dialog,
/// leaves both open.
#[gpui::test]
fn a_press_in_a_menu_over_the_dialog_is_the_menus(cx: &mut TestAppContext) {
    let OptionDialog {
        cx,
        dialog,
        menu_anchor,
        ..
    } = open_option_dialog(cx, SWITCHER, None, Tall::No);
    let item = handle(cx);
    let menu = open_menu(cx, menu_anchor, vec![("item", item)]);
    let (item_box, panel) = (bounds(cx, "item"), bounds(cx, "panel"));
    assert!(
        !panel.intersects(&item_box),
        "the premise: the item is drawn outside the dialog"
    );
    click(cx, "item");
    assert!(is_open(cx, menu), "the menu stayed open");
    assert!(is_open(cx, dialog), "and so did the dialog");
}

/// The switcher's outside press is one its veil takes, read in the bubble phase, after a menu opened over it has already
/// closed itself in the capture phase on the same press; whether the switcher was the top layer is therefore noted when
/// the press began. Specified: with a menu open over a dialog that closes on an outside press, a press on the veil
/// beside both closes the menu and leaves the dialog open.
#[gpui::test]
fn a_press_on_the_veil_beside_a_menu_over_the_dialog_closes_only_the_menu(cx: &mut TestAppContext) {
    let OptionDialog {
        cx,
        dialog,
        menu_anchor,
        ..
    } = open_option_dialog(cx, SWITCHER, None, Tall::No);
    let item = handle(cx);
    let menu = open_menu(cx, menu_anchor, vec![("item", item)]);
    assert!(
        !bounds(cx, "panel").intersects(&bounds(cx, "page-corner")),
        "the premise: the press lands on the veil, outside the switcher"
    );
    click(cx, "page-corner");
    assert!(!is_open(cx, menu), "the press closed the menu");
    assert!(is_open(cx, dialog), "and only the menu");
}

/// Without the option nothing changes: an outside press is the veil's, and the dialog stays.
#[gpui::test]
fn a_press_outside_an_ordinary_dialog_leaves_it_open(cx: &mut TestAppContext) {
    let OptionDialog {
        cx,
        dialog,
        corner_presses,
        ..
    } = open_option_dialog(cx, ORDINARY, None, Tall::No);
    click(cx, "page-corner");
    assert!(is_open(cx, dialog), "the dialog stayed open");
    assert_eq!(corner_presses.get(), 0, "and the veil took the press");
}

/// An outside press dismisses through Escape's path, so a dialog that may not be dismissed now (an editor whose save is
/// in flight) refuses it as it refuses Escape. Specified: while its hook answers Stay, an outside press leaves the
/// dialog open and focus inside it; once the hook answers Close, the next outside press closes it.
#[gpui::test]
fn a_guarded_dialog_refuses_an_outside_press(cx: &mut TestAppContext) {
    let busy = Rc::new(Cell::new(true));
    let held = busy.clone();
    let guard: layers::DismissHook = Rc::new(move |_, _| {
        if held.get() {
            Dismissal::Stay
        } else {
            Dismissal::Close
        }
    });
    let OptionDialog {
        cx, dialog, only, ..
    } = open_option_dialog(cx, SWITCHER, Some(guard), Tall::No);
    click(cx, "page-corner");
    assert!(is_open(cx, dialog), "the guarded dialog stayed open");
    assert!(focused(cx, &only), "and focus stayed in it");
    busy.set(false);
    click(cx, "page-corner");
    assert!(
        !is_open(cx, dialog),
        "once allowed, the outside press closed it"
    );
}

/// SPEC.md asks a question when the user leaves an editor with an unsaved draft ("Leaving an unsaved draft asks Keep
/// editing, Discard, or Save in that reading order, with initial focus still on Save"), so a dismissal must be able to
/// keep the editor open and ask, then leave the closing to the answer. A yes-or-no guard cannot say that, and an
/// editor asking from its own key handler would miss Escape while the dialog itself holds focus.
///
/// Specified, for a dialog whose hook asks a three-answer question and answers Stay: Escape, and a dismissal the way a
/// header Close makes it ([`layers::dismiss`]), leave the dialog open under the open question; then Keep editing
/// (Escape on the question) closes only the question and focus is back in the dialog, Discard closes both without
/// saving and focus goes back to the page, and Save (Enter, where the question starts) saves once and closes both.
#[gpui::test]
fn a_dialog_can_ask_before_it_closes_and_each_answer_does_its_part(cx: &mut TestAppContext) {
    for (answer, by_close, keystrokes, stays, saves) in [
        ("Keep editing", false, "escape", true, 0),
        ("Discard", false, "shift-tab enter", false, 0),
        ("Save", false, "enter", false, 1),
        ("Save, after Close", true, "enter", false, 1),
    ] {
        let saved = Rc::new(Cell::new(0));
        let own: Rc<Cell<Option<LayerId>>> = Rc::new(Cell::new(None));
        let asked: Rc<Cell<Option<LayerId>>> = Rc::new(Cell::new(None));
        let (counted, closes, opened) = (saved.clone(), own.clone(), asked.clone());
        let hook: layers::DismissHook = Rc::new(move |window, cx| {
            let (counted, discards, saves) = (counted.clone(), closes.clone(), closes.clone());
            let leave = prompt::Question::new(
                "Save these changes before leaving?",
                prompt::Answer {
                    label: "Save".into(),
                    help: HoverHelp::tooltip("Save the draft, then leave"),
                    tier: Tier::Primary,
                },
                (),
            )
            .cancel_answer(prompt::Answer {
                label: "Keep editing".into(),
                help: HoverHelp::tooltip("Go back to the draft"),
                tier: Tier::Neutral,
            })
            .alternative(
                prompt::Answer {
                    label: "Discard".into(),
                    help: HoverHelp::tooltip("Leave without saving"),
                    tier: Tier::Neutral,
                },
                move |(), window, cx| {
                    if let Some(id) = discards.get() {
                        layers::close(id, window, cx);
                    }
                },
            );
            let mark = Mark::take(cx);
            let question = prompt::ask(
                leave,
                None,
                &mark,
                move |(), window, cx| {
                    counted.set(counted.get() + 1);
                    if let Some(id) = saves.get() {
                        layers::close(id, window, cx);
                    }
                },
                window,
                cx,
            );
            opened.set(question);
            Dismissal::Stay
        });
        let OptionDialog {
            cx,
            dialog,
            opener,
            only,
            ..
        } = open_option_dialog(cx, ORDINARY, Some(hook), Tall::No);
        own.set(Some(dialog));
        if by_close {
            // What a dialog's header Close does: dismiss its own layer.
            cx.update(|window, cx| layers::dismiss(dialog, window, cx));
            cx.run_until_parked();
            check(cx);
        } else {
            keys(cx, "escape");
        }
        assert!(
            is_open(cx, dialog),
            "{answer}: the dismissal left the dialog open"
        );
        let Some(question) = asked.get() else {
            panic!("{answer}: the dismissal asked the question");
        };
        assert!(
            is_open(cx, question) && !focused(cx, &only),
            "{answer}: the question is open over the dialog and has focus"
        );
        keys(cx, keystrokes);
        assert_eq!(saved.get(), saves, "{answer}: what was saved");
        assert_eq!(
            is_open(cx, dialog),
            stays,
            "{answer}: whether the dialog stayed"
        );
        let home = if stays { &only } else { &opener };
        assert!(focused(cx, home), "{answer}: where focus went");
    }
}

// ---- Layers: a popover's presets -------------------------------------------------------------------------------------

/// Why this matters: openers build popovers through the presets rather than struct literals, so a preset's defaults
/// are what every menu and floating list gets, and a field added later takes its value there instead of in every
/// opener's file. What each default does is shown by tests that open through the presets: a menu starting on its item
/// and keeping Tab inside ([`tab_stays_inside_an_open_menu`]), a floating list leaving focus and Tab with the field
/// ([`a_popover_that_never_takes_focus_leaves_focus_and_tab_with_the_field`]), and the tunnel list's tests for the two
/// builders. This one pins the values, so a preset that drifts fails under its own name.
///
/// What: [`Popover::menu`] starts on the handle it was given, returns focus to its opener and traps Tab;
/// [`Popover::floating`] has no start; [`Popover::returns_to`] changes only where focus returns, and
/// [`Popover::tab_through`] only turns the trap off.
#[gpui::test]
fn the_popover_presets_carry_their_documented_defaults(cx: &mut TestAppContext) {
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let out = anchor_out.clone();
    let cx = open(cx, move |window, cx| {
        anchor_element("opener", &out, window, cx).into_any_element()
    });
    let anchor = anchor_out
        .borrow()
        .clone()
        .expect("the premise: the anchor drew");
    let (start, toggle) = (handle(cx), handle(cx));
    cx.update(|_, cx| {
        let view = cx.new(|_| FloatingList);
        let menu = Popover::menu(
            view.clone(),
            cx.focus_handle(),
            anchor.clone(),
            start.clone(),
        );
        assert!(
            menu.start.as_ref() == Some(&start),
            "a menu starts on the handle it was given"
        );
        assert_eq!(
            menu.returns_to,
            focus::ReturnTo::Opener,
            "a menu returns focus to its opener"
        );
        assert!(menu.traps_tab, "a menu keeps Tab inside it");

        let named = menu.returns_to(focus::ReturnTo::Control(toggle.clone()));
        assert_eq!(
            named.returns_to,
            focus::ReturnTo::Control(toggle.clone()),
            "returns_to names where focus returns"
        );
        assert!(
            named.traps_tab && named.start.as_ref() == Some(&start),
            "returns_to leaves the trap and the start alone"
        );

        let through = named.tab_through();
        assert!(!through.traps_tab, "tab_through turns the trap off");
        assert!(
            through.returns_to == focus::ReturnTo::Control(toggle.clone())
                && through.start.as_ref() == Some(&start),
            "tab_through leaves where focus starts and returns alone"
        );

        let floating = Popover::floating(view, cx.focus_handle(), anchor.clone());
        assert!(
            floating.start.is_none(),
            "a floating list never takes focus"
        );
    });
}

// ---- Layers: a popover placed beside its anchor ----------------------------------------------------------------------

/// Open a window whose only content is an anchor element under the selector `opener`, placed by `place` (a div that
/// positions its child), and return the anchor's handle once it has been drawn.
fn open_placed_anchor(
    cx: &mut TestAppContext,
    place: fn(gpui::Div) -> gpui::Div,
) -> (&mut VisualTestContext, Rc<RefCell<Option<Anchor>>>) {
    let out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let drawn = out.clone();
    let cx = open(cx, move |window, cx| {
        place(div().size_full().relative())
            .child(anchor_element("opener", &drawn, window, cx))
            .into_any_element()
    });
    (cx, out)
}

/// The old session row menu flew out beside the sidebar rather than below its button (`session_menu_placement_style`
/// at the reference commit: left at the toggle's right edge plus 10, top at its top minus 4). Specified: a popover
/// opened on [`Anchor::beside`] sits at exactly that offset from its anchor, and the overlay invariants hold.
#[gpui::test]
fn a_popover_beside_its_anchor_sits_right_of_it_and_a_little_above(cx: &mut TestAppContext) {
    let (cx, out) = open_placed_anchor(cx, |place| place.pt(px(100.)).pl(px(40.)));
    let item = handle(cx);
    let anchor = out.borrow().clone().unwrap();
    open_menu(cx, anchor.beside(), vec![("item", item)]);
    let menu = cx.debug_bounds("menu").unwrap();
    let opener = cx.debug_bounds("opener").unwrap();
    let viewport = cx.update(|window, _| window.viewport_size());
    assert!(
        menu.right() < viewport.width && menu.bottom() < viewport.height,
        "the premise: the window has room, so no clamp moved the menu ({menu:?} in {viewport:?})"
    );
    assert_eq!(
        (menu.left(), menu.top()),
        (opener.right() + px(10.), opener.top() - px(4.)),
        "the menu starts 10 px right of its anchor and 4 px above it: {menu:?} beside {opener:?}"
    );
}

/// Port, not redesign: like the old flyout, the beside placement never moves to the other side of its anchor when the
/// window is too narrow; it is only shifted back inside the window, as every popover is. Specified: near the right and
/// bottom edges the popover's right and bottom edges sit on the window's, it is not flipped to the anchor's left, and
/// the overlay invariants still hold (checked as it opens).
#[gpui::test]
fn a_popover_beside_an_anchor_near_the_edges_is_kept_inside_and_never_flips(
    cx: &mut TestAppContext,
) {
    let (cx, out) = open_placed_anchor(cx, |place| place.flex().justify_end().items_end());
    // Three items, so the menu is taller than the room below its anchor's top.
    let items = vec![
        ("first", handle(cx)),
        ("second", handle(cx)),
        ("third", handle(cx)),
    ];
    let anchor = out.borrow().clone().unwrap();
    open_menu(cx, anchor.beside(), items);
    let menu = cx.debug_bounds("menu").unwrap();
    let opener = cx.debug_bounds("opener").unwrap();
    let viewport = cx.update(|window, _| window.viewport_size());
    assert!(
        opener.right() + px(10.) + menu.size.width > viewport.width
            && opener.top() - px(4.) + menu.size.height > viewport.height,
        "the premise: unclamped, the menu would run past the right and bottom edges ({opener:?}, {viewport:?})"
    );
    assert_eq!(
        (menu.right(), menu.bottom()),
        (viewport.width, viewport.height),
        "the menu is shifted back inside the window: {menu:?} in {viewport:?}"
    );
    assert!(
        menu.left() > opener.left() - menu.size.width,
        "the menu was not flipped to the anchor's left: {menu:?} beside {opener:?}"
    );
}

// ---- Layers: a popover as wide as its anchor, and one that never takes focus -----------------------------------------

/// A floating list's content: a surface with a height but no width of its own, so it is as wide as its layer makes it,
/// and with nothing in it that takes focus, as a list that leaves focus in its field must be.
struct FloatingList;

impl Render for FloatingList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        layers::surface(cx)
            .debug_selector(|| "list".into())
            .h(px(40.))
    }
}

/// Open a [`FloatingList`] on `anchor` as a popover that never takes focus, for a mark taken now, and check.
fn open_floating_list(cx: &mut VisualTestContext, anchor: Anchor) -> LayerId {
    let id = cx.update(move |window, cx| {
        let view = cx.new(|_| FloatingList);
        let mark = Mark::take(cx);
        let list = Popover::floating(view, cx.focus_handle(), anchor);
        layers::open_popover(list, &mark, window, cx)
    });
    cx.run_until_parked();
    check(cx);
    match id {
        Some(id) => id,
        None => panic!("the premise: the list opened"),
    }
}

/// Why this matters: a select's list lines up with its control, and the old launcher's folder picker and composer
/// search floated at exactly their field's width (`left: 0; right: 0` under the field at the reference commit), however
/// long their entries. The width has to be the anchor's in this frame: the window's layout runs before the anchor
/// records where it is drawn, so a width taken then lags a resize, and stays wrong if no other frame follows.
///
/// Specified: a popover opened on [`Anchor::matching_width`] is exactly as wide as its anchor element, left edges
/// aligned, just below it; after the anchor's width changes, the frame that draws the new width draws the popover at
/// it.
#[gpui::test]
fn a_popover_matching_its_anchor_is_as_wide_as_it_in_the_same_frame(cx: &mut TestAppContext) {
    let width = Rc::new(Cell::new(px(200.)));
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let (drawn_width, out) = (width.clone(), anchor_out.clone());
    let cx = open(cx, move |window, cx| {
        let anchor = Anchor::for_element("field-anchor", window, cx);
        out.replace(Some(anchor.clone()));
        div()
            .relative()
            .debug_selector(|| "field".into())
            .w(drawn_width.get())
            .h(px(24.))
            .child(anchor.track())
            .into_any_element()
    });
    let anchor = anchor_out.borrow().clone().unwrap();
    open_floating_list(cx, anchor.matching_width());
    for expected in [px(200.), px(260.)] {
        width.set(expected);
        redraw(cx);
        let (list, field) = (bounds(cx, "list"), bounds(cx, "field"));
        assert_eq!(
            field.size.width, expected,
            "the premise: the field is drawn at its width"
        );
        assert_eq!(
            (list.left(), list.top(), list.size.width),
            (field.left(), field.bottom() + px(4.), expected),
            "the list sits under the field at exactly its width: {list:?} under {field:?}"
        );
    }
}

/// Why this matters: the old launcher's floating lists kept focus in the field the user was typing in (their options at
/// `tabindex: -1`, a press on the list prevented), and the terminal's overlays on a zero-size anchor must not take the
/// keys from the terminal. A popover that took focus on opening, trapped Tab or took focus from a press on it would
/// leave the user typing into nothing.
///
/// Specified: a popover opened without a start while a text field holds focus leaves focus in the field; a press on
/// the popover leaves focus in the field and the popover open; Escape in the field closes it, as the topmost layer,
/// with focus still in the field; reopened, Tab goes on from the field to the next control on the page, as with nothing
/// open, and the popover stays open. The harness's invariants, including that such a popover never holds focus, are
/// checked after every step.
#[gpui::test]
fn a_popover_that_never_takes_focus_leaves_focus_and_tab_with_the_field(cx: &mut TestAppContext) {
    let field: Rc<RefCell<Option<TextInputState>>> = Rc::new(RefCell::new(None));
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let next = Rc::new(RefCell::new(None::<FocusHandle>));
    let (kept, out, drawn_next) = (field.clone(), anchor_out.clone(), next.clone());
    let cx = open(cx, move |window, cx| {
        let state = kept
            .borrow_mut()
            .get_or_insert_with(|| TextInputState::new(window, cx))
            .clone();
        let anchor = Anchor::for_element("field-anchor", window, cx);
        out.replace(Some(anchor.clone()));
        let help = HoverHelp::tooltip("A test field");
        div()
            .flex()
            .flex_col()
            .gap(px(80.))
            .child(
                div()
                    .relative()
                    .debug_selector(|| "field-box".into())
                    .w(px(160.))
                    .child(TextInput::new("field", &state, "Folder", help))
                    .child(anchor.track()),
            )
            .children(
                drawn_next
                    .borrow()
                    .clone()
                    .map(|focus| button("next", &focus)),
            )
            .into_any_element()
    });
    let after = handle(cx);
    next.replace(Some(after.clone()));
    redraw(cx);
    let state = field.borrow().clone().unwrap();
    let in_field = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| state.focus_handle(cx).is_focused(window))
    };
    let anchor = || anchor_out.borrow().clone().unwrap().matching_width();
    click(cx, "field-box");
    assert!(in_field(cx), "the premise: the field holds focus");

    let list = open_floating_list(cx, anchor());
    assert!(in_field(cx), "opening the list left focus in the field");
    click(cx, "list");
    assert!(is_open(cx, list), "a press on the list is not outside it");
    assert!(in_field(cx), "a press on the list left focus in the field");
    keys(cx, "escape");
    assert!(!is_open(cx, list), "Escape closed the list");
    assert!(in_field(cx), "and focus stayed in the field");

    let list = open_floating_list(cx, anchor());
    keys(cx, "tab");
    assert!(
        focused(cx, &after),
        "Tab went on from the field to the next control, not into the list"
    );
    assert!(is_open(cx, list), "Tab does not close the list");
}

/// Why this matters: the focus coordinator's moves made on the user's behalf (`focus::focus_control`) hold back while
/// a dialog or menu is open, because focus never leaves the topmost layer for the page behind it. A popover that never
/// takes focus is not such a layer: focus is on the page already, so holding back under it would freeze those moves for
/// as long as a floating list shows.
///
/// Specified: with only a popover that never takes focus open, a move to another page control, for a mark taken now,
/// takes focus there and leaves the popover open; with a menu open (a popover that holds focus), the same move does
/// nothing and focus stays in the menu.
#[gpui::test]
fn focus_moves_go_on_under_a_popover_that_never_takes_focus(cx: &mut TestAppContext) {
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let controls = Rc::new(RefCell::new(None::<(FocusHandle, FocusHandle)>));
    let (out, drawn) = (anchor_out.clone(), controls.clone());
    let cx = open(cx, move |window, cx| {
        let anchor = Anchor::for_element("first-anchor", window, cx);
        out.replace(Some(anchor.clone()));
        let page = div().flex().flex_col().gap(px(80.));
        match drawn.borrow().clone() {
            Some((first, second)) => page
                .child(
                    div()
                        .relative()
                        .w(px(160.))
                        .child(button("first", &first))
                        .child(anchor.track()),
                )
                .child(button("second", &second)),
            None => page,
        }
        .into_any_element()
    });
    let (first, second) = (handle(cx), handle(cx));
    controls.replace(Some((first.clone(), second.clone())));
    redraw(cx);
    let anchor = anchor_out.borrow().clone().unwrap();
    let move_to = |target: &FocusHandle, cx: &mut VisualTestContext| {
        let target = target.clone();
        cx.update(move |window, cx| {
            let mark = Mark::take(cx);
            focus::focus_control(&target, &mark, window, cx)
        });
        cx.run_until_parked();
        check(cx);
    };
    click(cx, "first");
    assert!(
        focused(cx, &first),
        "the premise: the first control holds focus"
    );

    let list = open_floating_list(cx, anchor.clone());
    move_to(&second, cx);
    assert!(
        focused(cx, &second),
        "under the floating list, the move took focus"
    );
    assert!(is_open(cx, list), "and the list stayed open");

    let item = handle(cx);
    let menu = open_menu(cx, anchor, vec![("item", item.clone())]);
    assert!(focused(cx, &item), "the premise: the menu took focus");
    move_to(&first, cx);
    assert!(focused(cx, &item), "under a menu, the move did nothing");
    assert!(is_open(cx, menu), "and the menu stayed open");
}

// ---- Layers: a popover that lets Tab through -------------------------------------------------------------------------

/// The controls the Tab-through tests draw, as the terminal's top row draws the tunnel pill: a control before the
/// pill, the pill (a button that is also the list's anchor), room for the list to open into, and a control after the
/// pill unless the scene ends at it ([`PillScene`]).
fn pill_controls(page: &PillPage, with_after: bool, pill_anchor: &Anchor) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(button("before", &page.before))
        .child(
            div()
                .relative()
                .w(px(80.))
                .child(button("pill", &page.pill))
                .child(pill_anchor.track()),
        )
        // Room for the list under the pill, so it covers nothing a test clicks.
        .child(div().h(px(120.)))
        .when(with_after, |column| {
            column.child(button("after", &page.after))
        })
}

/// Where the Tab-through tests draw the pill and its neighbours.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PillScene {
    /// On the page, with a control after the pill.
    Page,
    /// On the page, the pill its last control, so a walk out of the list forward has to wrap past the page's end.
    PageEndingAtPill,
    /// Inside a dialog on the page, as a list opened from a dialog's control would be: the dialog is then the trap
    /// below the list.
    Dialog,
}

/// The dialog of [`PillScene::Dialog`]: the pill's controls, in a dialog of their own.
struct PillDialog {
    page: PillPage,
}

impl Render for PillDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pill_anchor = Anchor::for_element("pill-anchor", window, cx);
        self.page.anchor.replace(Some(pill_anchor.clone()));
        div()
            .debug_selector(|| "pill-dialog".into())
            .w(px(300.))
            .p(px(20.))
            .child(pill_controls(&self.page, true, &pill_anchor))
    }
}

/// What the Tab-through tests hold of their scene: the controls before, at and after the pill, and the pill's anchor
/// as last drawn. Clones share the anchor.
#[derive(Clone)]
struct PillPage {
    before: FocusHandle,
    pill: FocusHandle,
    after: FocusHandle,
    anchor: Rc<RefCell<Option<Anchor>>>,
}

impl PillPage {
    /// Open the page of [`PillScene::Page`].
    fn open(cx: &mut TestAppContext) -> (&mut VisualTestContext, PillPage) {
        PillPage::open_scene(cx, PillScene::Page)
    }

    /// Open a window drawing `scene`, and a spot of empty page under "elsewhere". Nothing holds focus but the root
    /// until a test moves it, except in [`PillScene::Dialog`], whose dialog starts on the control before the pill.
    fn open_scene(cx: &mut TestAppContext, scene: PillScene) -> (&mut VisualTestContext, PillPage) {
        let drawn: Rc<RefCell<Option<PillPage>>> = Rc::new(RefCell::new(None));
        let on_page = drawn.clone();
        let cx = open(cx, move |window, cx| {
            let mut column = div().flex().flex_col();
            if scene != PillScene::Dialog
                && let Some(page) = on_page.borrow().clone()
            {
                let pill_anchor = Anchor::for_element("pill-anchor", window, cx);
                page.anchor.replace(Some(pill_anchor.clone()));
                column = column.child(pill_controls(&page, scene == PillScene::Page, &pill_anchor));
            }
            column
                .child(div().debug_selector(|| "elsewhere".into()).size(px(40.)))
                .into_any_element()
        });
        let page = PillPage {
            before: handle(cx),
            pill: handle(cx),
            after: handle(cx),
            anchor: Rc::new(RefCell::new(None)),
        };
        drawn.replace(Some(page.clone()));
        redraw(cx);
        if scene == PillScene::Dialog {
            let (in_dialog, start, container) = (page.clone(), page.before.clone(), handle(cx));
            let id = cx.update(move |window, cx| {
                let view = cx.new(|_| PillDialog { page: in_dialog });
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
                        placement: DialogPlacement::Centered,
                        dismiss_on_outside_press: false,
                    },
                    &mark,
                    window,
                    cx,
                )
            });
            settle(cx);
            assert!(id.is_some(), "the premise: the pill's dialog opened");
        }
        (cx, page)
    }

    /// Open the tunnel list's stand-in below the pill, starting on the first of `controls`, letting Tab through and
    /// returning focus to the pill, for a mark taken now. Returns its id and the switch that takes its last control
    /// away ([`Panel`]).
    fn open_list(
        &self,
        cx: &mut VisualTestContext,
        controls: Vec<(&'static str, FocusHandle)>,
    ) -> (LayerId, Rc<Cell<bool>>) {
        let start = controls[0].1.clone();
        let anchor = self.anchor.borrow().clone().unwrap();
        let pill = self.pill.clone();
        let shown = Rc::new(Cell::new(true));
        let panel_shown = shown.clone();
        let id = cx.update(move |window, cx| {
            let view = cx.new(|_| Panel {
                controls,
                shown: panel_shown,
            });
            let mark = Mark::take(cx);
            layers::open_popover(
                Popover::menu(view, cx.focus_handle(), anchor, start)
                    .returns_to(focus::ReturnTo::Control(pill))
                    .tab_through(),
                &mark,
                window,
                cx,
            )
        });
        settle(cx);
        match id {
            Some(id) => (id, shown),
            None => panic!("the premise: the list opened"),
        }
    }
}

/// Why this matters: SPEC.md's tunnel list ("Tab follows its ordinary controls and leaving the list closes it without
/// reclaiming focus") is a list of ordinary controls in the page's Tab order, not a menu that keeps Tab in. GPUI orders
/// Tab by paint order and paints every overlay after the page, so without the coordinator's splice Tab from the list's
/// last control would wrap to the window's first control and Shift+Tab from its first would land on the page's last,
/// nowhere near the pill. And a trap would leave the controls after the pill unreachable while the list is open.
///
/// Specified: the list takes focus on its first control when it opens, holds the keyboard (`focus_layer_open`) and
/// traps nothing (`trap`). Tab walks its controls, and from the last goes to the control after the pill, closing the
/// list. Shift+Tab from its first control reaches the pill, which leaves the list open (the pill is not outside it, as
/// in the old app, where the list lived inside the pill's element); Tab from the pill enters the list's first control;
/// Shift+Tab from the pill reaches the control before it and closes the list.
#[gpui::test]
fn tab_and_shift_tab_walk_a_list_that_lets_tab_through_in_page_order(cx: &mut TestAppContext) {
    let (cx, page) = PillPage::open(cx);
    let (one, two) = (handle(cx), handle(cx));
    let items = vec![("one", one.clone()), ("two", two.clone())];

    let (list, _) = page.open_list(cx, items.clone());
    assert!(
        focused(cx, &one),
        "the premise: the list starts on its first control"
    );
    assert!(
        cx.update(
            |window, cx| layers::focus_layer_open(window, cx) && layers::trap(window, cx).is_none()
        ),
        "the list holds the keyboard but traps no Tab"
    );
    keys(cx, "tab");
    assert!(focused(cx, &two), "Tab walks the list's controls");
    keys(cx, "tab");
    settle(cx);
    assert!(
        focused(cx, &page.after),
        "Tab from the list's last control goes on to the control after the pill"
    );
    assert!(!is_open(cx, list), "and focus leaving the list closed it");

    let (list, _) = page.open_list(cx, items.clone());
    keys(cx, "shift-tab");
    settle(cx);
    assert!(
        focused(cx, &page.pill),
        "Shift+Tab from the list's first control reaches the pill"
    );
    assert!(
        is_open(cx, list),
        "focus on the pill does not close the list"
    );
    keys(cx, "tab");
    assert!(
        focused(cx, &one),
        "Tab from the pill enters the list, which follows it in Tab order"
    );
    keys(cx, "shift-tab shift-tab");
    settle(cx);
    assert!(
        focused(cx, &page.before),
        "Shift+Tab from the pill reaches the control before it"
    );
    assert!(!is_open(cx, list), "and focus leaving the list closed it");
}

/// Why this matters: SPEC.md, "Escape closes the list and returns focus to the pill", and on macOS the click that
/// opened the list left the pill unfocused, so only the name the opener gave brings focus back there. A guard rather
/// than a test of the Tab-through change: Escape's return to a named control predates it, and this keeps the close
/// that returns no focus (focus leaving the list) from taking Escape's return with it.
///
/// Specified: from any control of a list that lets Tab through, Escape closes it and focus goes to the pill its opener
/// named.
#[gpui::test]
fn escape_closes_a_list_that_lets_tab_through_and_returns_focus_to_the_pill(
    cx: &mut TestAppContext,
) {
    let (cx, page) = PillPage::open(cx);
    let (one, two) = (handle(cx), handle(cx));
    let (list, _) = page.open_list(cx, vec![("one", one.clone()), ("two", two.clone())]);
    keys(cx, "tab");
    assert!(focused(cx, &two), "the premise: focus is inside the list");
    keys(cx, "escape");
    assert!(!is_open(cx, list), "Escape closed the list");
    assert!(focused(cx, &page.pill), "and returned focus to the pill");
}

/// Why this matters: SPEC.md's "leaving the list closes it without reclaiming focus" covers the pointer too. A menu
/// closed by an outside press returns focus to its opener; this list must not, or a click on another control (or, on
/// macOS, where a click focuses no button, a click anywhere) would put focus back on the pill instead of where the user
/// went.
///
/// Specified: a press on the page's empty space, or on a control after the pill (with either platform's click rule),
/// closes the list and leaves focus off the pill: on the control where a click focuses it, and home where the press
/// focuses nothing (the empty page, or a button where clicks focus none). A press on the pill itself, which is the
/// list's anchor, leaves it open for the pill's own toggle to decide, and the pill taking focus from it (Linux) is not
/// leaving.
#[gpui::test]
fn a_press_elsewhere_closes_a_list_that_lets_tab_through_without_reclaiming_focus(
    cx: &mut TestAppContext,
) {
    let (cx, page) = PillPage::open(cx);
    let one = handle(cx);
    let items = vec![("one", one.clone())];
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    // A home other than the pill, standing in for the terminal, so a press that focuses nothing shows where focus
    // rests after it.
    let home = page.before.clone();
    cx.update(move |window, cx| focus::set_home(Some(&home), window, cx));
    settle(cx);

    let (list, _) = page.open_list(cx, items.clone());
    click(cx, "elsewhere");
    settle(cx);
    assert!(
        !is_open(cx, list),
        "a press on the empty page closed the list"
    );
    assert!(
        focused(cx, &page.before),
        "and focus went home, not back to the pill"
    );

    let (list, _) = page.open_list(cx, items.clone());
    click(cx, "after");
    settle(cx);
    assert!(
        !is_open(cx, list),
        "a press on another control closed the list"
    );
    assert!(
        focused(cx, &page.after),
        "and focus is on that control, where a click focuses controls"
    );

    let (list, _) = page.open_list(cx, items.clone());
    click(cx, "pill");
    settle(cx);
    assert!(
        focused(cx, &page.pill),
        "the premise: the press focused the pill"
    );
    assert!(
        is_open(cx, list),
        "a press on the pill is the pill's, and focus there is not leaving"
    );
    keys(cx, "escape");
    assert!(!is_open(cx, list), "the premise: Escape closed the list");

    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    let (list, _) = page.open_list(cx, items);
    click(cx, "after");
    settle(cx);
    assert!(
        !is_open(cx, list),
        "a press on another control closed the list where clicks focus no button"
    );
    assert!(
        focused(cx, &page.before),
        "and focus, left on a control no longer drawn, recovered home rather than going back to the pill"
    );
}

/// Why this matters: closing one tunnel takes its row, and the close button that held focus, out of the list. SPEC.md:
/// "When the focused control goes away, focus moves to the closest remaining control". The focus coordinator's recovery
/// does that for any layer that holds focus, but GPUI reports the lost focus as focus leaving the list, so the rule that
/// closes the list on leaving must see where recovery put focus rather than the report.
///
/// Specified: when the focused control of a list that lets Tab through goes away, focus moves to the list's remaining
/// control and the list stays open.
#[gpui::test]
fn a_list_that_lets_tab_through_keeps_focus_when_its_focused_control_goes(cx: &mut TestAppContext) {
    let (cx, page) = PillPage::open(cx);
    let (one, two) = (handle(cx), handle(cx));
    let (list, last_shown) = page.open_list(cx, vec![("one", one.clone()), ("two", two.clone())]);
    keys(cx, "tab");
    assert!(
        focused(cx, &two),
        "the premise: the last control holds focus"
    );
    last_shown.set(false);
    redraw(cx);
    settle(cx);
    assert!(
        focused(cx, &one),
        "focus moved to the list's remaining control"
    );
    assert!(is_open(cx, list), "and the list stayed open");
}

/// Why this matters: a list opened from a control inside a dialog has the dialog as the trap below it, and the layer
/// manager draws each layer apart, so the dialog's container never contains the list's controls. A walk that let only
/// stops inside the trap qualify would pass over every control of the list and leave it, so Tab could never reach the
/// list and it would close at once.
///
/// Specified: with the pill inside a dialog, Tab walks the list's controls and from the last reaches the dialog's
/// control after the pill, closing the list and leaving the dialog open; Shift+Tab from the list's first control
/// reaches the pill, and Tab from the pill enters the list.
#[gpui::test]
fn tab_walks_a_list_that_lets_tab_through_inside_a_dialog(cx: &mut TestAppContext) {
    let (cx, page) = PillPage::open_scene(cx, PillScene::Dialog);
    assert!(
        focused(cx, &page.before),
        "the premise: the dialog starts on the control before the pill"
    );
    let (one, two) = (handle(cx), handle(cx));
    let items = vec![("one", one.clone()), ("two", two.clone())];

    let (list, _) = page.open_list(cx, items.clone());
    keys(cx, "tab");
    assert!(focused(cx, &two), "Tab walks the list's controls");
    keys(cx, "tab");
    settle(cx);
    assert!(
        focused(cx, &page.after),
        "Tab from the list's last control reaches the dialog's control after the pill"
    );
    assert!(!is_open(cx, list), "and focus leaving the list closed it");
    assert!(
        cx.update(|window, cx| layers::trap(window, cx).is_some()),
        "while the dialog stays open, holding Tab"
    );

    let (list, _) = page.open_list(cx, items);
    keys(cx, "shift-tab");
    settle(cx);
    assert!(
        focused(cx, &page.pill),
        "Shift+Tab from the list's first control reaches the pill in the dialog"
    );
    keys(cx, "tab");
    assert!(focused(cx, &one), "Tab from the pill enters the list");
    assert!(is_open(cx, list), "and the list stayed open throughout");
}

/// Send an auto-repeat of `key`, held down since its last press (a key-down with `is_held` set), and check.
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

/// Why this matters: a held Escape closes one layer and then reaches nothing (the focus coordinator swallows the
/// repeats of a dismissal). With focus on the pill while its list is open, focus is outside the layer Escape closes, a
/// state no other layer has; unless that dismissal is noted like one from inside the list, the repeats go on to close
/// the dialog the pill sits in.
///
/// Specified: with the pill inside a dialog and focus on it while its list is open, Escape closes the list and leaves
/// focus on the pill, and a held repeat of that Escape leaves the dialog open with focus still on the pill.
#[gpui::test]
fn a_held_escape_on_the_pill_closes_only_its_list(cx: &mut TestAppContext) {
    let (cx, page) = PillPage::open_scene(cx, PillScene::Dialog);
    let one = handle(cx);
    let (list, _) = page.open_list(cx, vec![("one", one.clone())]);
    keys(cx, "shift-tab");
    settle(cx);
    assert!(
        focused(cx, &page.pill) && is_open(cx, list),
        "the premise: focus is on the pill, with the list open"
    );
    keys(cx, "escape");
    settle(cx);
    assert!(!is_open(cx, list), "Escape on the pill closed the list");
    assert!(focused(cx, &page.pill), "and left focus on the pill");
    held(cx, "escape");
    settle(cx);
    assert!(
        cx.update(|window, cx| layers::trap(window, cx).is_some()),
        "the held repeat left the pill's dialog open"
    );
    assert!(focused(cx, &page.pill), "with focus still on the pill");
}

/// Why this matters: Escape on the pill counts the pill as inside its list so that the dismissal is noted (the test
/// above), but only Escape may: the pill's own toggle closes its list with an ordinary close while focus is on the
/// pill, and if that close were noted too, the same key pressed again within the repeat interval, a deliberate press
/// to reopen the list, would be swallowed.
///
/// Specified: with focus on the pill while its list is open, a key press followed by an ordinary close of the list
/// returns no focus; once focus has moved elsewhere without a key press, a fresh press of the same key still counts as
/// the user moving on.
#[gpui::test]
fn an_ordinary_close_from_the_pill_keeps_a_fresh_press(cx: &mut TestAppContext) {
    let (cx, page) = PillPage::open(cx);
    let one = handle(cx);
    let (list, _) = page.open_list(cx, vec![("one", one.clone())]);
    keys(cx, "shift-tab");
    settle(cx);
    assert!(
        focused(cx, &page.pill) && is_open(cx, list),
        "the premise: the pill holds focus with its list open"
    );
    keys(cx, "x");
    cx.update(|window, cx| layers::close(list, window, cx));
    settle(cx);
    assert!(
        !is_open(cx, list),
        "the premise: the ordinary close closed the list"
    );
    let before = page.before.clone();
    cx.update(move |window, cx| {
        let mark = Mark::take(cx);
        focus::focus_control(&before, &mark, window, cx)
    });
    settle(cx);
    assert!(
        focused(cx, &page.before),
        "the premise: focus moved elsewhere without a key press"
    );
    let mark = cx.update(|_, cx| Mark::take(cx));
    keys(cx, "x");
    assert!(
        cx.update(|_, cx| focus::has_moved_on(&mark, cx)),
        "a fresh press of the key pressed before the ordinary close still counts"
    );
}

/// Why this matters: GPUI orders the list's controls after the whole page, so a walk out of the list that has to wrap
/// past the page's end passes them again on its way round. Counted as stops already seen, they would end the walk as if
/// it had come full circle, and focus would stay in the list.
///
/// Specified: with the pill the page's last control, Tab from the list's last control wraps to the page's first
/// control and closes the list.
#[gpui::test]
fn tab_out_of_a_list_that_lets_tab_through_wraps_past_the_pages_end(cx: &mut TestAppContext) {
    let (cx, page) = PillPage::open_scene(cx, PillScene::PageEndingAtPill);
    let (one, two) = (handle(cx), handle(cx));
    let (list, _) = page.open_list(cx, vec![("one", one.clone()), ("two", two.clone())]);
    keys(cx, "tab");
    assert!(
        focused(cx, &two),
        "the premise: the list's last control holds focus"
    );
    keys(cx, "tab");
    settle(cx);
    assert!(
        focused(cx, &page.before),
        "Tab wrapped past the pill, the page's last control, to its first"
    );
    assert!(!is_open(cx, list), "and focus leaving the list closed it");
}

/// Why this matters: a control of the list may ask something in a layer of its own, which takes focus out of the list
/// without the user leaving it. Closing the list then would also close the question opened over it, as a layer that
/// takes focus closes what was opened over it.
///
/// Specified: a dialog opened from a control of a list that lets Tab through takes focus and leaves the list open;
/// when the dialog is dismissed, focus is back on that control and the list is still open.
#[gpui::test]
fn a_layer_opened_above_a_list_that_lets_tab_through_leaves_it_open(cx: &mut TestAppContext) {
    let (cx, page) = PillPage::open(cx);
    let one = handle(cx);
    let (list, _) = page.open_list(cx, vec![("one", one.clone())]);
    assert!(
        focused(cx, &one),
        "the premise: focus is on the list's control"
    );
    let asked = handle(cx);
    let (question, _, _) = open_dialog(cx, vec![("asked", asked.clone())], &asked);
    settle(cx);
    assert!(focused(cx, &asked), "the premise: the dialog took focus");
    assert!(
        is_open(cx, list),
        "focus in a layer above the list is not leaving it"
    );
    keys(cx, "escape");
    settle(cx);
    assert!(
        !is_open(cx, question),
        "the premise: Escape closed the dialog"
    );
    assert!(
        focused(cx, &one) && is_open(cx, list),
        "focus went back to the list's control, and the list is still open"
    );
}

/// Why this matters: GPUI reports focus leaving every focused element when the window stops being active, though no
/// focus moved. The old list stayed open when the user switched apps (the browser's active element did not change),
/// and closing it then would lose the user's place.
///
/// Specified: with focus in a list that lets Tab through, the window going inactive leaves the list open and focus
/// where it was.
#[gpui::test]
fn switching_apps_leaves_a_list_that_lets_tab_through_open(cx: &mut TestAppContext) {
    let (cx, page) = PillPage::open(cx);
    let (one, two) = (handle(cx), handle(cx));
    let (list, _) = page.open_list(cx, vec![("one", one.clone()), ("two", two.clone())]);
    keys(cx, "tab");
    assert!(focused(cx, &two), "the premise: focus is inside the list");
    cx.deactivate_window();
    settle(cx);
    assert!(
        cx.update(|window, _| !window.is_window_active()),
        "the premise: the window is inactive"
    );
    assert!(
        is_open(cx, list),
        "the window going inactive left the list open"
    );
    assert!(focused(cx, &two), "and focus where it was");
    cx.update(|window, _| window.activate_window());
    settle(cx);
    assert!(is_open(cx, list), "and coming back left it open too");
}

// ---- Layers: what a close takes with it ------------------------------------------------------------------------------

/// What the close tests hold of their scene: the page control the user is in when something opens (the terminal, or a
/// field), the anchor element a popover sits on, and the switch that stops the anchor being drawn.
struct CloseScene {
    page: FocusHandle,
    anchor: Rc<RefCell<Option<Anchor>>>,
    anchor_shown: Rc<Cell<bool>>,
}

impl CloseScene {
    /// Open a window drawing a page control above an anchor element, with focus on the page control.
    fn open(cx: &mut TestAppContext) -> (&mut VisualTestContext, CloseScene) {
        let anchor: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
        let anchor_shown = Rc::new(Cell::new(true));
        let page = Rc::new(RefCell::new(None::<FocusHandle>));
        let (out, shown, drawn_page) = (anchor.clone(), anchor_shown.clone(), page.clone());
        let cx = open(cx, move |window, cx| {
            let column = div().flex().flex_col().gap(px(8.)).children(
                drawn_page
                    .borrow()
                    .clone()
                    .map(|focus| button("page", &focus)),
            );
            if shown.get() {
                column.child(anchor_element("field", &out, window, cx))
            } else {
                column
            }
            .into_any_element()
        });
        let focus = handle(cx);
        page.replace(Some(focus.clone()));
        redraw(cx);
        click(cx, "page");
        assert!(
            focused(cx, &focus),
            "the premise: the page control holds focus"
        );
        let scene = CloseScene {
            page: focus,
            anchor,
            anchor_shown,
        };
        (cx, scene)
    }

    /// The anchor as last drawn. Panics if it was never drawn, since every caller opens a layer on it.
    fn anchor(&self) -> Anchor {
        match self.anchor.borrow().clone() {
            Some(anchor) => anchor.matching_width(),
            None => panic!("the premise: the anchor was drawn"),
        }
    }

    /// A floating list on the anchor (a popover that never takes focus, standing in for the terminal's link
    /// disclosure), then a dialog over it, opened from the page control as a shortcut opens Rename there. Checks that
    /// both are open and focus is on the dialog's control.
    fn list_under_dialog(&self, cx: &mut VisualTestContext) -> (LayerId, LayerId, FocusHandle) {
        let list = open_floating_list(cx, self.anchor());
        assert!(
            focused(cx, &self.page),
            "the premise: the list left focus on the page"
        );
        let only = handle(cx);
        let (dialog, _, _) = open_dialog(cx, vec![("only", only.clone())], &only);
        assert!(
            is_open(cx, list) && is_open(cx, dialog),
            "the premise: the list and the dialog over it are open"
        );
        assert!(focused(cx, &only), "the premise: focus is in the dialog");
        (list, dialog, only)
    }
}

/// Why this matters: a popover that never takes focus (the terminal's link destination display, a floating list) shows
/// while the user keeps typing or pointing on the page, so a dialog the user opens then (the quick switcher, Rename,
/// Delete) lands above it. The popover goes on its own schedule: its owner closes it when the pointer leaves the link,
/// a press outside it closes it, and so does its anchor no longer being drawn. Had that close taken every layer above
/// the popover with it, as a menu's close does, the dialog the user is working in would vanish under them.
///
/// Specified: with a dialog opened over such a popover, closing the popover by its id, by its anchor going and by a
/// press outside it (on the dialog's own control) each close the popover only: the dialog stays open and focus stays on
/// its control. The dialog's own close still returns focus to the page control it was opened from.
#[gpui::test]
fn a_dialog_over_a_popover_that_never_takes_focus_outlives_its_close(cx: &mut TestAppContext) {
    let (cx, scene) = CloseScene::open(cx);

    let (list, dialog, only) = scene.list_under_dialog(cx);
    cx.update(|window, cx| layers::close(list, window, cx));
    settle(cx);
    assert!(!is_open(cx, list), "closing the list closed it");
    assert!(is_open(cx, dialog), "and left the dialog over it open");
    assert!(focused(cx, &only), "with focus still in the dialog");
    keys(cx, "escape");
    assert!(
        !is_open(cx, dialog),
        "the premise: Escape closed the dialog"
    );
    assert!(
        focused(cx, &scene.page),
        "the dialog's close returned focus to the page control it was opened from"
    );

    let (list, dialog, only) = scene.list_under_dialog(cx);
    scene.anchor_shown.set(false);
    redraw(cx);
    assert!(
        !is_open(cx, list),
        "the list closed when its anchor was no longer drawn"
    );
    assert!(is_open(cx, dialog), "and left the dialog over it open");
    assert!(focused(cx, &only), "with focus still in the dialog");
    keys(cx, "escape");
    assert!(
        !is_open(cx, dialog),
        "the premise: Escape closed the dialog"
    );
    scene.anchor_shown.set(true);
    redraw(cx);

    let (list, dialog, only) = scene.list_under_dialog(cx);
    click(cx, "only");
    assert!(!is_open(cx, list), "a press outside the list closed it");
    assert!(
        is_open(cx, dialog),
        "and left the dialog the press was in open"
    );
    assert!(focused(cx, &only), "with focus still in the dialog");
}

/// Why this matters: a layer that takes focus owns what was opened over it, since focus stays inside the topmost such
/// layer and whatever opened over it was opened from there. A dialog opened from a menu's item must go with the menu,
/// or it would be left over a page whose menu is gone. This is the rule the popover that never takes focus is the
/// exception to, kept as it was.
///
/// Specified: closing a menu with a dialog opened over it closes the dialog too, and focus returns to the page control
/// that held it when the menu opened.
#[gpui::test]
fn closing_a_layer_that_takes_focus_closes_what_was_opened_over_it(cx: &mut TestAppContext) {
    let (cx, scene) = CloseScene::open(cx);
    let item = handle(cx);
    let menu = open_menu(cx, scene.anchor(), vec![("item", item.clone())]);
    assert!(focused(cx, &item), "the premise: the menu took focus");
    let only = handle(cx);
    let (dialog, _, _) = open_dialog(cx, vec![("only", only.clone())], &only);
    assert!(focused(cx, &only), "the premise: focus is in the dialog");

    cx.update(|window, cx| layers::close(menu, window, cx));
    settle(cx);
    assert!(!is_open(cx, menu), "closing the menu closed it");
    assert!(!is_open(cx, dialog), "and the dialog opened over it");
    assert!(
        focused(cx, &scene.page),
        "focus returned to the page control that held it when the menu opened"
    );
}

/// Why this matters: a reset of the connection to the helm closes every layer (SPEC.md, "Losing the connection to the
/// helm"), and a dialog left behind would keep its veil over the inert window. The reset must not reach the layers
/// through a close of the bottom one: when that is a popover that never takes focus, it closes alone.
///
/// Specified: a reset with a dialog open over a popover that never takes focus closes both, and focus returns to the
/// page control the dialog was opened from.
#[gpui::test]
fn a_reset_closes_a_dialog_over_a_popover_that_never_takes_focus(cx: &mut TestAppContext) {
    let (cx, scene) = CloseScene::open(cx);
    let (list, dialog, _) = scene.list_under_dialog(cx);
    // Deferred to after this update, as the reset runs from the channel's events: `close_all` updates each window to
    // close its layers, and a window cannot be updated from inside its own update.
    cx.update(|_, cx| cx.defer(layers::close_all));
    settle(cx);
    assert!(
        !is_open(cx, list) && !is_open(cx, dialog),
        "the reset closed the list and the dialog over it"
    );
    assert!(
        focused(cx, &scene.page),
        "focus returned to the page control the dialog was opened from"
    );
}

// ---- Layers: a session as a subject ----------------------------------------------------------------------------------

/// Why this matters: a Delete question about a session must close when that session is deleted from another window or
/// by an agent (SPEC.md: a layer goes away "when the thing it is about is gone"), but a session missing from one pushed
/// listing is not always gone: the helm cuts long lists at its cap, a filter hides rows, and a failed read lists
/// nothing. A question that closed on those would vanish under the user's pointer on a passing read error. The rule is
/// the one the selected session's survivor uses, so the two can never disagree about the same session.
///
/// Specification: a question with `Subject::session("b")` stays open while another session goes, and while listings
/// that are truncated, filtered or unavailable lack `b`; it closes once a complete, available, unfiltered listing lacks
/// it. A question about a session such a listing already lacks closes as soon as it opens.
#[gpui::test]
fn a_question_about_a_session_closes_only_when_a_complete_listing_proves_it_gone(
    cx: &mut TestAppContext,
) {
    use farhelm_helm_client::fake::FakeHelm;
    use farhelm_proto::ui_channel::{
        Availability, Collection, CollectionChanges, CollectionMeta, ListCounts, SessionRow,
    };

    use crate::test_support::{meta, sessions};

    let cx = open(cx, |_, _| div().into_any_element());
    let (mut fake, handle, events) = FakeHelm::new();
    let _connection = cx.update(|_, cx| crate::session::start(handle, events, None, cx));
    fake.connect(vec![sessions(&["a", "b"])]);
    cx.run_until_parked();
    let listed = cx.update(|_, cx| {
        crate::session::current_store(cx).map(|store| {
            let collections = store.read(cx).collections();
            ["a", "b"].map(|key| collections.get::<SessionRow>(key).is_some())
        })
    });
    assert_eq!(
        listed,
        Some([true, true]),
        "the premise: the store holds both sessions"
    );
    /// Ask about `key`, with the session as the question's subject; `None` if nothing opened.
    fn ask(cx: &mut VisualTestContext, key: &'static str) -> Option<LayerId> {
        let layer = cx.update(|window, cx| {
            let mark = Mark::take(cx);
            let Some(subject) = Subject::session(key, cx) else {
                panic!("the premise: a UI session is live, so a session subject can be made")
            };
            prompt::ask(
                question(key),
                Some(subject),
                &mark,
                |_, _, _| {},
                window,
                cx,
            )
        });
        cx.run_until_parked();
        check(cx);
        layer
    }
    fn push(
        fake: &mut FakeHelm,
        cx: &mut VisualTestContext,
        meta: CollectionMeta,
        remove: &[&str],
    ) {
        fake.push(CollectionChanges {
            collection: Collection::Sessions,
            meta,
            upsert: vec![],
            remove: remove.iter().map(|key| key.to_string()).collect(),
        });
        cx.run_until_parked();
        check(cx);
    }
    fn counts(truncated: bool, matching: Option<u64>) -> Option<ListCounts> {
        Some(ListCounts {
            total: 2,
            matching,
            truncated,
        })
    }

    let Some(layer) = ask(cx, "b") else {
        panic!("the premise: the question about b opened")
    };
    push(&mut fake, cx, meta(&["b"]), &["a"]);
    assert!(is_open(cx, layer), "another session going leaves it open");
    let mut truncated = meta(&[]);
    truncated.counts = counts(true, None);
    push(&mut fake, cx, truncated, &["b"]);
    assert!(is_open(cx, layer), "a truncated listing proves nothing");
    let mut filtered = meta(&[]);
    filtered.counts = counts(false, Some(0));
    push(&mut fake, cx, filtered, &[]);
    assert!(is_open(cx, layer), "a filtered listing proves nothing");
    let mut unavailable = meta(&[]);
    unavailable.availability = Availability::Unavailable {
        error: "the session store could not be read".into(),
    };
    push(&mut fake, cx, unavailable, &[]);
    assert!(is_open(cx, layer), "an unavailable listing proves nothing");
    push(&mut fake, cx, meta(&[]), &[]);
    assert!(
        !is_open(cx, layer),
        "a complete listing without b closed it"
    );

    let Some(gone) = ask(cx, "b") else {
        panic!("the premise: a question about a session already proven gone still opens")
    };
    assert!(!is_open(cx, gone), "and closes at once");
}

// ---- Menus: arrows and the pointer -----------------------------------------------------------------------------------

/// A menu whose items take the highlight when the pointer enters them, as the session row's menu items will.
struct HoverMenu {
    items: Vec<(&'static str, FocusHandle)>,
}

impl Render for HoverMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        layers::panel(cx)
            .debug_selector(|| "menu".into())
            .w(px(160.))
            .children(self.items.iter().map(|(id, focus)| hover_item(id, focus)))
    }
}

/// `button(id, focus)`, taking the highlight when the pointer enters it.
fn hover_item(id: &'static str, focus: &FocusHandle) -> impl IntoElement + use<> {
    let highlight = focus.clone();
    div()
        .id(gpui::ElementId::Name(format!("{id}-hover").into()))
        .on_hover(move |hovered, window, cx| {
            if *hovered {
                focus::highlight(&highlight, window, cx);
            }
        })
        .child(button(id, focus))
}

/// Why this matters: SPEC.md has menus use their own arrow keys and "A menu has one highlight. Whichever moved last,
/// the pointer or the keys, sets it". The old menus walked every item with the arrows and wrapped at both ends
/// (`sidebar.spec.ts` at reference commit `24bc0580`). Tab is no substitute: on a Mac with Keyboard navigation off it
/// skips buttons, and every menu item is one.
///
/// Specification: with Tab reaching text fields only, Down and Up in a menu move through every item and wrap at both
/// ends, never reaching the page behind. Moving the pointer onto an item moves the highlight there, and the arrows go
/// on from it. The pointer over the page behind an open menu takes nothing out of the menu.
#[gpui::test]
fn arrows_walk_every_menu_item_and_wrap_and_the_pointer_moves_the_same_highlight(
    cx: &mut TestAppContext,
) {
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let page = Rc::new(RefCell::new(None::<FocusHandle>));
    let (out, drawn) = (anchor_out.clone(), page.clone());
    let cx = open(cx, move |window, cx| {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(
                drawn
                    .borrow()
                    .clone()
                    .map(|focus| hover_item("page", &focus)),
            )
            .child(anchor_element("opener", &out, window, cx))
            .into_any_element()
    });
    let reach = Rc::new(Cell::new(focus::TabReach::TextFieldsAndPopups));
    follow(cx, &reach);
    let behind = handle(cx);
    page.replace(Some(behind.clone()));
    redraw(cx);
    let items = [handle(cx), handle(cx), handle(cx)];
    let anchor = anchor_out.borrow().clone().unwrap();
    let start = items[0].clone();
    let view_items = vec![
        ("first", items[0].clone()),
        ("second", items[1].clone()),
        ("third", items[2].clone()),
    ];
    let id = cx.update(move |window, cx| {
        let view = cx.new(|_| HoverMenu { items: view_items });
        let mark = Mark::take(cx);
        layers::open_popover(
            Popover::menu(view, cx.focus_handle(), anchor, start),
            &mark,
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    assert!(id.is_some(), "the premise: the menu opened");
    let at = |cx: &mut VisualTestContext| items.iter().position(|item| focused(cx, item));
    assert_eq!(
        at(cx),
        Some(0),
        "the premise: the menu starts on its first item"
    );

    for expected in [1, 2, 0] {
        keys(cx, "down");
        assert_eq!(at(cx), Some(expected), "Down reaches every item and wraps");
    }
    for expected in [2, 1, 0, 2] {
        keys(cx, "up");
        assert_eq!(at(cx), Some(expected), "Up reaches every item and wraps");
    }

    let second = center(cx, "second");
    cx.simulate_mouse_move(second, None, Modifiers::none());
    check(cx);
    assert_eq!(
        at(cx),
        Some(1),
        "the pointer moved the highlight to the item under it"
    );
    keys(cx, "down");
    assert_eq!(
        at(cx),
        Some(2),
        "the keys go on from where the pointer left it"
    );

    let page_button = center(cx, "page");
    cx.simulate_mouse_move(page_button, None, Modifiers::none());
    check(cx);
    assert!(
        !focused(cx, &behind),
        "the page behind the menu took nothing"
    );
    assert_eq!(at(cx), Some(2), "the highlight stayed in the menu");
}

// ---- The pill --------------------------------------------------------------------------------------------------------

/// A pill's content: a fixed-size box under its own selector, with nothing in it that asks for the pointer or focus, as
/// [`layers::Pill`] requires of its view.
struct PillBox {
    selector: &'static str,
}

impl Render for PillBox {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let selector = self.selector;
        div()
            .debug_selector(move || selector.into())
            .w(px(120.))
            .h(px(30.))
    }
}

/// Show `view` as a pill on `anchor`, placed as `placement` says, and settle. Returns what [`layers::show_pill`]
/// returned: the pill's id, or `None` when it does not show.
fn show_pill_view(
    cx: &mut VisualTestContext,
    view: gpui::AnyView,
    anchor: Anchor,
    placement: PillPlacement,
) -> Option<PillId> {
    let id = cx.update(move |window, cx| {
        let pill = Pill {
            view,
            anchor,
            placement,
        };
        layers::show_pill(pill, window, cx)
    });
    settle(cx);
    id
}

/// Show a [`PillBox`] drawn under `selector` on `anchor`; see [`show_pill_view`].
fn show_pill(
    cx: &mut VisualTestContext,
    anchor: Anchor,
    selector: &'static str,
    placement: PillPlacement,
) -> Option<PillId> {
    let view = cx.update(|_, cx| cx.new(|_| PillBox { selector }));
    show_pill_view(cx, view.into(), anchor, placement)
}

/// [`show_pill`], for a step whose premise is that the pill shows: panics when it does not.
fn shown(
    cx: &mut VisualTestContext,
    anchor: Anchor,
    selector: &'static str,
    placement: PillPlacement,
) -> PillId {
    match show_pill(cx, anchor, selector, placement) {
        Some(id) => id,
        None => panic!("the premise: the pill {selector} shows"),
    }
}

/// Whether the pill `id` shows in the test's window.
fn pill_shown(cx: &mut VisualTestContext, id: PillId) -> bool {
    cx.update(|window, cx| layers::pill_shown(id, window, cx))
}

/// Why this matters: SPEC.md puts the quick-launch sequence pill centered over its area, and at the bottom of the main
/// area while the launcher is open, so it does not cover the launcher. The feature moves it between the two by showing
/// it again, so showing must replace rather than add a second pill. The area it sits over resizes with the sidebar
/// and the window, and the pill must stay centered in the frame that draws the new size, not a frame later. And like a
/// popover it must not outlive the element it is placed over, where it would be pinned to a place that is gone.
///
/// Specified: a pill shown [`PillPlacement::Centered`] on an anchor element has the element's center, and has it again
/// in the one redraw after the element changes size; one then shown [`PillPlacement::Bottom`] replaces it, centered
/// across the element with its bottom edge on the element's; once the element is no longer drawn the pill goes, and a
/// pill shown on that gone element does not show.
#[gpui::test]
fn the_pill_sits_centered_over_its_anchor_or_on_its_bottom_edge(cx: &mut TestAppContext) {
    let area_shown = Rc::new(Cell::new(true));
    let area_size = Rc::new(Cell::new(gpui::size(px(400.), px(240.))));
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let (shown_cell, size, out) = (area_shown.clone(), area_size.clone(), anchor_out.clone());
    let cx = open(cx, move |window, cx| {
        // Away from the window's corner on both axes, so a pill placed against the window rather than the element is
        // caught.
        let page = div().pl(px(30.)).pt(px(10.));
        if !shown_cell.get() {
            return page.into_any_element();
        }
        let anchor = Anchor::for_element("area-anchor", window, cx);
        out.replace(Some(anchor.clone()));
        page.child(
            div()
                .relative()
                .debug_selector(|| "area".into())
                .w(size.get().width)
                .h(size.get().height)
                .child(anchor.track()),
        )
        .into_any_element()
    });
    let anchor = || anchor_out.borrow().clone().unwrap();

    let first = shown(cx, anchor(), "pill-a", PillPlacement::Centered);
    let (pill, area) = (bounds(cx, "pill-a"), bounds(cx, "area"));
    assert_eq!(
        pill.size,
        gpui::size(px(120.), px(30.)),
        "the premise: the pill is drawn at its own size"
    );
    assert_eq!(
        pill.center(),
        area.center(),
        "the pill is centered over its anchor: {pill:?} over {area:?}"
    );

    area_size.set(gpui::size(px(300.), px(180.)));
    redraw(cx);
    let (pill, area) = (bounds(cx, "pill-a"), bounds(cx, "area"));
    assert_eq!(
        area.size,
        gpui::size(px(300.), px(180.)),
        "the premise: the anchor is drawn at its new size"
    );
    assert_eq!(
        pill.center(),
        area.center(),
        "in the frame that resized its anchor, the pill is centered over the new size: {pill:?} over {area:?}"
    );

    let second = shown(cx, anchor(), "pill-b", PillPlacement::Bottom);
    assert!(
        cx.debug_bounds("pill-a").is_none() && !pill_shown(cx, first),
        "showing a pill replaced the one showing"
    );
    let (pill, area) = (bounds(cx, "pill-b"), bounds(cx, "area"));
    assert_eq!(
        (pill.center().x, pill.bottom()),
        (area.center().x, area.bottom()),
        "the pill is centered across its anchor, on its bottom edge: {pill:?} over {area:?}"
    );

    // One redraw is enough: the frame that no longer draws the anchor releases its state as it ends, and the pill's
    // removal notifies the stack, whose next frame the same settle draws.
    let gone = anchor();
    area_shown.set(false);
    redraw(cx);
    assert!(!pill_shown(cx, second), "the pill went with its anchor");
    assert!(
        cx.debug_bounds("pill-b").is_none(),
        "and is no longer drawn"
    );
    assert!(
        show_pill(cx, gone, "pill-c", PillPlacement::Centered).is_none(),
        "a pill on an anchor that is gone does not show"
    );
}

/// Why this matters: SPEC.md says neither the sequence pill nor its list takes focus or pointer input, and the old pill
/// was `pointer-events: none` over the terminal: a click on it must reach the terminal or control under it (where the
/// sequence's own rules cancel it), and the keys must stay where the user is. A pill that counted as a layer would
/// also stop the shortcuts that refuse while anything is open, trap Tab, or swallow Escape.
///
/// Specified: showing a pill moves no focus, and it is not an open layer (`any_open`, `focus_layer_open` and `trap`
/// all say nothing is open); a press on it clicks the control under it, which takes focus as it would without the
/// pill; Escape leaves the pill showing. With a dialog opened over the page, the pill still shows, a press on it is the
/// veil's (the page control gets nothing and the dialog stays), and Escape closes the dialog, not the pill, with focus
/// back on its opener. The harness's focus and overlay invariants are checked after every step.
#[gpui::test]
fn a_press_on_the_pill_reaches_what_it_covers_and_the_pill_is_not_a_layer(cx: &mut TestAppContext) {
    let clicks = Rc::new(Cell::new(0));
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let controls = Rc::new(RefCell::new(None::<(FocusHandle, FocusHandle)>));
    let (counted, out, drawn) = (clicks.clone(), anchor_out.clone(), controls.clone());
    let cx = open(cx, move |window, cx| {
        let anchor = Anchor::for_element("under-anchor", window, cx);
        out.replace(Some(anchor.clone()));
        let Some((page, under)) = drawn.borrow().clone() else {
            return div().into_any_element();
        };
        let counted = counted.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(40.))
            .child(button("page", &page))
            // The control the pill is placed over, big enough that the pill lies inside it.
            .child(
                div()
                    .relative()
                    .debug_selector(|| "under-box".into())
                    .w(px(200.))
                    .h(px(80.))
                    .flex()
                    .justify_center()
                    .child(
                        button("under", &under)
                            .on_click(move |_, _, _| counted.set(counted.get() + 1)),
                    )
                    .child(anchor.track()),
            )
            .into_any_element()
    });
    let (page, under) = (handle(cx), handle(cx));
    controls.replace(Some((page.clone(), under.clone())));
    redraw(cx);
    let nothing_open = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            !layers::any_open(window, cx)
                && !layers::focus_layer_open(window, cx)
                && layers::trap(window, cx).is_none()
        })
    };
    click(cx, "page");
    assert!(
        focused(cx, &page),
        "the premise: the page control holds focus"
    );

    let anchor = anchor_out.borrow().clone().unwrap();
    let id = shown(cx, anchor, "pill", PillPlacement::Centered);
    assert!(focused(cx, &page), "showing the pill moved no focus");
    assert!(nothing_open(cx), "the pill is not an open layer");
    let (pill, control) = (bounds(cx, "pill"), bounds(cx, "under"));
    assert!(
        control.contains(&pill.center()),
        "the premise: the pill covers the control ({pill:?} over {control:?})"
    );

    cx.simulate_click(pill.center(), Modifiers::none());
    check(cx);
    assert_eq!(
        clicks.get(),
        1,
        "the press on the pill clicked the control under it"
    );
    assert!(
        focused(cx, &under),
        "and that control took focus, as without the pill"
    );
    keys(cx, "escape");
    assert!(
        pill_shown(cx, id),
        "Escape takes no pill down: it is not a layer"
    );

    let only = handle(cx);
    let (dialog, _, _) = open_dialog(cx, vec![("only", only.clone())], &only);
    assert!(focused(cx, &only), "the premise: the dialog took focus");
    assert_eq!(
        bounds(cx, "pill"),
        pill,
        "the pill still shows, in its place, with a dialog open"
    );
    cx.simulate_click(pill.center(), Modifiers::none());
    check(cx);
    assert_eq!(
        clicks.get(),
        1,
        "under a dialog, a press on the pill is the veil's"
    );
    assert!(is_open(cx, dialog), "and the dialog stays");
    keys(cx, "escape");
    assert!(!is_open(cx, dialog), "Escape closed the dialog");
    assert!(pill_shown(cx, id), "and not the pill");
    assert!(
        focused(cx, &under),
        "focus went back to the dialog's opener"
    );
    assert!(nothing_open(cx));
}

/// A pill that breaks the [`layers::Pill`] rule on purpose: it blocks and counts every press on it, so a test can tell
/// whether it is drawn above what it covers. A rule-abiding pill takes no press, so its draw order cannot be seen by
/// pressing it.
///
/// `id` is both its element id and its selector, so a test can tell two probes apart, in the frame and by place.
struct ProbePill {
    presses: Rc<Cell<u32>>,
    id: &'static str,
}

impl Render for ProbePill {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (presses, id) = (self.presses.clone(), self.id);
        div()
            .id(id)
            .debug_selector(move || id.into())
            .w(px(120.))
            .h(px(30.))
            .occlude()
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                presses.set(presses.get() + 1)
            })
    }
}

/// Why this matters: the pill exists so the sequence pill stays in view above the launcher's veil (the old one at
/// z-index 50 over the launcher's backdrop at 40); drawn under the veil it would be dimmed and, for SPEC.md, missing.
/// The approval cards sit under it, as the old ones did at z-index 45, and the launch pill under them, as the old launch
/// pills sat at 40 over the launcher's backdrop (the old z-index ladder at `.header-confirm`): drawn under a dialog's
/// veil the progress pill would vanish, over the cards it would cover a waiting agent's answer buttons, and over the
/// sequence pill it would cover what the user is typing. The tooltip goes above the pill, as in the old app: the two
/// were level at z-index 50 and the tooltip's card came later in the page (rung 8 of the layer manager's z-order
/// ladder). A rule-abiding pill shows no order to a press, so this test uses probes that take presses, and reads the
/// order the frame drew in for the tooltip, whose card takes no press either.
///
/// Specified: with a dialog open, a press on a probe pill over the page, and one on a probe launch pill, each reaches
/// its probe, not the dialog's veil; and in the drawn frame the dialog's layer comes before the launch pill, the launch
/// pill before an approval card, the card before the pill, and the pill before a tooltip shown over it.
#[gpui::test]
fn the_pills_are_drawn_above_every_layer_in_the_old_order_and_under_the_tooltip(
    cx: &mut TestAppContext,
) {
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let out = anchor_out.clone();
    let cx = open(cx, move |window, cx| {
        anchor_element("area", &out, window, cx).into_any_element()
    });
    let only = handle(cx);
    let (dialog, _, _) = open_dialog(cx, vec![("only", only.clone())], &only);
    install_card(cx, top_right());
    let probe = |cx: &mut VisualTestContext, id: &'static str| {
        let presses = Rc::new(Cell::new(0));
        let counted = presses.clone();
        let view = cx.update(move |_, cx| {
            cx.new(|_| ProbePill {
                presses: counted,
                id,
            })
        });
        (presses, gpui::AnyView::from(view))
    };
    let (presses, pill) = probe(cx, "probe-pill");
    let (launch_presses, launch) = probe(cx, "launch-probe");
    let anchor = anchor_out.borrow().clone().unwrap();
    if show_pill_view(cx, pill, anchor, PillPlacement::Centered).is_none() {
        panic!("the premise: the probe pill shows");
    }
    if show_launch_view(cx, launch).is_none() {
        panic!("the premise: the probe launch pill shows");
    }

    let at = center(cx, "probe-pill");
    cx.simulate_click(at, Modifiers::none());
    check(cx);
    assert_eq!(
        presses.get(),
        1,
        "the press reached the probe pill, drawn above the dialog's veil"
    );
    let launch_at = center(cx, "launch-probe");
    cx.simulate_click(launch_at, Modifiers::none());
    check(cx);
    assert_eq!(
        launch_presses.get(),
        1,
        "the press reached the probe launch pill, drawn above the dialog's veil"
    );
    assert!(is_open(cx, dialog), "the premise: the dialog is still open");

    // The tooltip shows after the dialog opened, since a layer opening puts it away.
    let owner = cx.update(|_, cx| cx.new(|_| layers::TooltipOwner));
    cx.update(|window, cx| layers::show_tooltip_at(&owner, "Probe help".into(), at, window, cx));
    settle(cx);
    // The driver record is filled as elements are prepared, and GPUI prepares deferred draws in priority order, the
    // same order it paints and hit-tests them, so a lower index is drawn under a higher one.
    let nodes = frame(cx);
    let position = |what: &str, found: &dyn Fn(&gpui::DriverNode) -> bool| match nodes
        .iter()
        .position(found)
    {
        Some(index) => index,
        None => panic!("the premise: the frame draws {what}"),
    };
    let element =
        |id: &'static str| move |node: &gpui::DriverNode| node.element_id.as_deref() == Some(id);
    let layer = position("the dialog's layer", &element("layer-0"));
    let launch = position("the probe launch pill", &element("launch-probe"));
    let card = position("the approval card", &element("card"));
    let pill = position("the probe pill", &element("probe-pill"));
    let tooltip = position("the tooltip's text", &|node| {
        node.texts
            .iter()
            .any(|text| text.text.as_ref() == "Probe help")
    });
    assert!(
        layer < launch && launch < card && card < pill && pill < tooltip,
        "drawn bottom to top: the dialog ({layer}), the launch pill ({launch}), the card ({card}), the pill ({pill}), \
         the tooltip ({tooltip})"
    );
}

/// Why this matters: the pill's feature takes it down when the sequence ends, and a reset retires the UI session that
/// feature lives in, so nothing would be left to take it down: SPEC.md has everything from before a reset start over,
/// and a pill left over the inert window would show a sequence that no longer exists. And a feature hiding its own
/// pill must not take down a pill shown after it, as no opener closes a layer it did not open.
///
/// Specified: [`layers::hide_pill`] takes its pill down, and does nothing to a pill shown after it; so does
/// [`layers::close_all`], which the UI-session owner calls on every reset (its own tests check that call), with or
/// without a dialog open.
#[gpui::test]
fn hiding_the_pill_or_a_reset_takes_it_down(cx: &mut TestAppContext) {
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let out = anchor_out.clone();
    let cx = open(cx, move |window, cx| {
        anchor_element("area", &out, window, cx).into_any_element()
    });
    let anchor = || anchor_out.borrow().clone().unwrap();
    let hide = |cx: &mut VisualTestContext, id: PillId| {
        cx.update(move |window, cx| layers::hide_pill(id, window, cx));
        settle(cx);
    };

    let first = shown(cx, anchor(), "pill", PillPlacement::Centered);
    hide(cx, first);
    assert!(!pill_shown(cx, first), "hiding took the pill down");
    assert!(
        cx.debug_bounds("pill").is_none(),
        "and it is no longer drawn"
    );

    let replaced = shown(cx, anchor(), "pill-old", PillPlacement::Centered);
    let current = shown(cx, anchor(), "pill", PillPlacement::Centered);
    hide(cx, replaced);
    assert!(
        pill_shown(cx, current),
        "hiding a replaced pill left the pill shown after it"
    );
    hide(cx, current);

    for with_dialog in [false, true] {
        let id = shown(cx, anchor(), "pill", PillPlacement::Centered);
        let dialog = with_dialog.then(|| {
            let only = handle(cx);
            open_dialog(cx, vec![("only", only.clone())], &only).0
        });
        assert!(
            pill_shown(cx, id),
            "the premise: the pill shows (with_dialog: {with_dialog})"
        );
        // Deferred to after this update, as the reset runs from the channel's events: `close_all` updates each
        // window to close its layers, and a window cannot be updated from inside its own update.
        cx.update(|_, cx| cx.defer(layers::close_all));
        settle(cx);
        assert!(!pill_shown(cx, id), "a reset took the pill down");
        assert!(
            cx.debug_bounds("pill").is_none(),
            "and it is no longer drawn"
        );
        if let Some(dialog) = dialog {
            assert!(
                !is_open(cx, dialog),
                "the premise: the reset closed the dialog too"
            );
        }
    }
}

// ---- The launch pill -------------------------------------------------------------------------------------------------

/// Show `view` as the window's launch pill and settle. Returns what [`layers::show_launch_pill`] returned.
fn show_launch_view(cx: &mut VisualTestContext, view: gpui::AnyView) -> Option<PillId> {
    let id = cx.update(move |window, cx| layers::show_launch_pill(view, window, cx));
    settle(cx);
    id
}

/// Show a [`PillBox`] drawn under `selector` as the launch pill, for a step whose premise is that it shows.
fn shown_launch(cx: &mut VisualTestContext, selector: &'static str) -> PillId {
    let view = cx.update(|_, cx| cx.new(|_| PillBox { selector }));
    match show_launch_view(cx, view.into()) {
        Some(id) => id,
        None => panic!("the premise: the launch pill {selector} shows"),
    }
}

/// Why this matters: SPEC.md's launch progress pill (and the "Started" report after it) and the quick-launch sequence
/// pill can be up at the same time, and each feature takes down only its own. With one slot, a sequence armed while a
/// launch pill shows (the "Started" report, say) would wipe it.
///
/// Specified: a launch pill and a pill show together; showing a launch pill again replaces only the launch pill, and
/// showing a pill again replaces only the pill; hiding either leaves the other drawn, and hiding a replaced launch
/// pill's id does nothing; a reset takes both down.
#[gpui::test]
fn the_launch_pill_shows_beside_the_pill_and_each_hides_alone(cx: &mut TestAppContext) {
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let out = anchor_out.clone();
    let cx = open(cx, move |window, cx| {
        anchor_element("area", &out, window, cx).into_any_element()
    });
    let anchor = || anchor_out.borrow().clone().unwrap();
    let hide = |cx: &mut VisualTestContext, id: PillId| {
        cx.update(move |window, cx| layers::hide_pill(id, window, cx));
        settle(cx);
    };
    let drawn =
        |cx: &mut VisualTestContext, selector: &'static str| cx.debug_bounds(selector).is_some();

    let pill = shown(cx, anchor(), "pill", PillPlacement::Centered);
    let launch = shown_launch(cx, "launch");
    assert!(
        pill_shown(cx, pill) && pill_shown(cx, launch),
        "both pills show"
    );
    assert!(
        drawn(cx, "pill") && drawn(cx, "launch"),
        "and both are drawn"
    );

    let launch_b = shown_launch(cx, "launch-b");
    assert!(
        !pill_shown(cx, launch) && !drawn(cx, "launch"),
        "showing a launch pill replaced the launch pill"
    );
    assert!(
        pill_shown(cx, pill) && drawn(cx, "pill"),
        "and left the pill showing"
    );

    let pill_b = shown(cx, anchor(), "pill-b", PillPlacement::Centered);
    assert!(
        !pill_shown(cx, pill) && !drawn(cx, "pill"),
        "showing a pill replaced the pill"
    );
    assert!(
        pill_shown(cx, launch_b) && drawn(cx, "launch-b"),
        "and left the launch pill showing"
    );

    hide(cx, launch);
    assert!(
        pill_shown(cx, launch_b),
        "hiding a replaced launch pill left the one shown after it"
    );
    hide(cx, launch_b);
    assert!(
        !pill_shown(cx, launch_b) && !drawn(cx, "launch-b"),
        "hiding took the launch pill down"
    );
    assert!(
        pill_shown(cx, pill_b) && drawn(cx, "pill-b"),
        "and left the pill drawn"
    );

    let launch_c = shown_launch(cx, "launch-c");
    hide(cx, pill_b);
    assert!(
        !pill_shown(cx, pill_b) && !drawn(cx, "pill-b"),
        "hiding took the pill down"
    );
    assert!(
        pill_shown(cx, launch_c) && drawn(cx, "launch-c"),
        "and left the launch pill drawn"
    );

    let pill_c = shown(cx, anchor(), "pill-c", PillPlacement::Centered);
    // Deferred, as the reset runs from the channel's events (see `hiding_the_pill_or_a_reset_takes_it_down`).
    cx.update(|_, cx| cx.defer(layers::close_all));
    settle(cx);
    assert!(
        !pill_shown(cx, launch_c) && !pill_shown(cx, pill_c),
        "a reset took both pills down"
    );
    assert!(
        !drawn(cx, "launch-c") && !drawn(cx, "pill-c"),
        "and neither is drawn"
    );
}

/// Why this matters: the old launch progress pill was fixed to the window, `bottom: 20px` and centered across it with
/// `max-width: calc(100vw - 40px)`, and `pointer-events: none`: the user goes on working while a launch is pending, so
/// a press on the pill must reach what it covers, and the pill must neither take focus nor count as a layer (which
/// would stop the shortcuts that refuse while one is open, trap Tab or swallow Escape).
///
/// Specified: a launch pill is centered across the window with its bottom edge 20 pixels above the window's; a view
/// wider than the window keeps 20 pixels clear at each side; showing it moves no focus and opens no layer; a press on it
/// reaches the element under it.
#[gpui::test]
fn the_launch_pill_sits_at_the_windows_bottom_and_a_press_passes_through(cx: &mut TestAppContext) {
    let presses = Rc::new(Cell::new(0));
    let page = Rc::new(RefCell::new(None::<FocusHandle>));
    let (counted, drawn) = (presses.clone(), page.clone());
    let cx = open(cx, move |window, _| {
        let Some(page) = drawn.borrow().clone() else {
            return div().into_any_element();
        };
        let counted = counted.clone();
        // A floor under the whole window, so whatever the pill covers is the floor.
        div()
            .relative()
            .w(window.viewport_size().width)
            .h(window.viewport_size().height)
            .child(button("page", &page))
            .child(
                div()
                    .id("floor")
                    .absolute()
                    .top(px(40.))
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .on_mouse_down(MouseButton::Left, move |_, _, _| {
                        counted.set(counted.get() + 1)
                    }),
            )
            .into_any_element()
    });
    let focus = handle(cx);
    page.replace(Some(focus.clone()));
    redraw(cx);
    click(cx, "page");
    assert!(
        focused(cx, &focus),
        "the premise: the page control holds focus"
    );

    shown_launch(cx, "launch");
    let viewport = cx.update(|window, _| window.viewport_size());
    let pill = bounds(cx, "launch");
    assert_eq!(
        pill.size,
        gpui::size(px(120.), px(30.)),
        "the premise: the launch pill is drawn at its own size"
    );
    assert_eq!(
        (pill.center().x, pill.bottom()),
        (viewport.width / 2., viewport.height - px(20.)),
        "the launch pill is centered across the window, 20 pixels above its bottom: {pill:?} in {viewport:?}"
    );
    assert!(
        focused(cx, &focus),
        "showing the launch pill moved no focus"
    );
    assert!(
        cx.update(|window, cx| !layers::any_open(window, cx)
            && !layers::focus_layer_open(window, cx)
            && layers::trap(window, cx).is_none()),
        "the launch pill is not an open layer"
    );

    cx.simulate_click(pill.center(), Modifiers::none());
    check(cx);
    assert_eq!(
        presses.get(),
        1,
        "the press on the launch pill reached the floor under it"
    );

    // A wrapped line far wider than the window: the cap holds it inside the side insets.
    let wide = cx.update(|_, cx| cx.new(|_| WideText));
    if show_launch_view(cx, wide.into()).is_none() {
        panic!("the premise: the wide launch pill shows");
    }
    let wide = bounds(cx, "wide");
    assert!(
        wide.left() >= px(20.) && wide.right() <= viewport.width - px(20.),
        "a wide launch pill keeps 20 pixels clear at each side: {wide:?} in {viewport:?}"
    );
}

/// A launch pill's content far wider than any test window, as one wrapping line: [`layers::show_launch_pill`]'s cap
/// must hold it inside the window's side insets.
struct WideText;

impl Render for WideText {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "wide".into())
            .child("Launching a session with a long name ".repeat(40))
    }
}

// ---- The approval region ---------------------------------------------------------------------------------------------

/// Where a [`Card`] puts its top-left corner in the frame being drawn, or `None` for nowhere (the card draws nothing).
type Place = Rc<dyn Fn(&Window) -> Option<gpui::Point<gpui::Pixels>>>;

/// The approval region's stand-in for an agent approval card: one button, Allow, on a card placed where `place` says,
/// or nothing while `shown` is off. It keeps the region's rule ([`layers::install_approval_region`]): the card occludes,
/// and nothing with a size or a listener is drawn while it is hidden. A press on Allow, or Enter on it, counts an
/// answer; the card stays until the test hides it, as a real card goes once its answer is accepted.
struct Card {
    allow: FocusHandle,
    shown: Rc<Cell<bool>>,
    answers: Rc<Cell<u32>>,
    place: Place,
}

impl Render for Card {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let Some(at) = (self.place)(window).filter(|_| self.shown.get()) else {
            return div().into_any_element();
        };
        let answers = self.answers.clone();
        div()
            .id("card")
            .debug_selector(|| "card".into())
            .absolute()
            .left(at.x)
            .top(at.y)
            .w(px(180.))
            .p(px(10.))
            .occlude()
            .child(
                button("card-allow", &self.allow)
                    .on_click(move |_, _, _| answers.set(answers.get() + 1)),
            )
            .into_any_element()
    }
}

/// What a test holds of the [`Card`] it installed: Allow's focus handle, the switch that hides the card, and its count
/// of answers.
struct Installed {
    allow: FocusHandle,
    shown: Rc<Cell<bool>>,
    answers: Rc<Cell<u32>>,
}

/// Install a shown [`Card`] placed by `place` as the window's approval region, as the approvals feature does at window
/// start, and settle.
fn install_card(cx: &mut VisualTestContext, place: Place) -> Installed {
    let (allow, shown, answers) = (handle(cx), Rc::new(Cell::new(true)), Rc::new(Cell::new(0)));
    let card = Card {
        allow: allow.clone(),
        shown: shown.clone(),
        answers: answers.clone(),
        place,
    };
    cx.update(move |window, cx| {
        let view = cx.new(|_| card);
        layers::install_approval_region(view.into(), window, cx);
    });
    settle(cx);
    Installed {
        allow,
        shown,
        answers,
    }
}

/// A card in the window's top right corner, clear of a centered or top-placed dialog and of the page's top-left
/// controls.
fn top_right() -> Place {
    Rc::new(|window: &Window| Some(point(window.viewport_size().width - px(200.), px(10.))))
}

/// Why this matters: an agent that asks while a dialog is open waits on the card, not the dialog, and the old app kept
/// the card answerable over the quick switcher, the one dialog that closes on a press outside it ("approval remains
/// answerable while the quick switcher is open"). A press on the card is outside the switcher by geometry, so the
/// switcher's close must come from its own veil instead.
///
/// Specified: with the switcher open and its control focused, a press on the card's Allow answers it and the switcher
/// stays open.
#[gpui::test]
fn a_press_on_an_approval_card_answers_it_and_leaves_the_quick_switcher_open(
    cx: &mut TestAppContext,
) {
    let OptionDialog {
        cx, dialog, only, ..
    } = open_option_dialog(cx, SWITCHER, None, Tall::No);
    let card = install_card(cx, top_right());
    assert!(
        focused(cx, &only),
        "the premise: the switcher's control has focus"
    );
    assert!(
        !bounds(cx, "card").intersects(&bounds(cx, "panel")),
        "the premise: the card is drawn outside the switcher"
    );
    click(cx, "card-allow");
    assert_eq!(card.answers.get(), 1, "the press answered the card");
    assert!(is_open(cx, dialog), "and the switcher stayed open");
}

/// Why this matters: the old "Rename wraps Close through Save while a real approval remains usable". A card stays
/// usable over an ordinary dialog, and a key meant for the card must not act on the dialog behind it: Escape there
/// would close a Rename and lose its text. The dialog's own rules (its veil, its Escape and focus return) are the
/// existing dialog tests'; only Tab is checked here, since the card's controls must stay out of the dialog's Tab.
///
/// Specified, where a click focuses controls: with a card shown, Tab in a dialog of Save and Close wraps between the
/// two; a press on Allow answers it and leaves focus there, Enter answers again and Escape does nothing, the dialog
/// staying open throughout; Tab from Allow goes back into the dialog; with focus back on Allow, the card going sends
/// focus into the dialog.
#[gpui::test]
fn an_approval_card_stays_usable_over_a_dialog(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| div().into_any_element());
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    let card = install_card(cx, top_right());
    let (save, close) = (handle(cx), handle(cx));
    let (dialog, container, _) = open_dialog(
        cx,
        vec![("save", save.clone()), ("close", close.clone())],
        &save,
    );
    assert!(focused(cx, &save), "the premise: the dialog starts on Save");
    keys(cx, "tab tab");
    assert!(
        focused(cx, &save),
        "Tab wrapped through Close to Save, past the card"
    );

    click(cx, "card-allow");
    assert_eq!(card.answers.get(), 1, "the press answered the card");
    assert!(
        focused(cx, &card.allow),
        "and focused Allow, with the dialog open"
    );
    keys(cx, "enter");
    assert_eq!(card.answers.get(), 2, "Enter on Allow answered it");
    keys(cx, "escape");
    assert!(is_open(cx, dialog), "Escape on Allow left the dialog open");
    assert!(focused(cx, &card.allow), "and focus on Allow");
    keys(cx, "tab");
    let in_dialog =
        |cx: &mut VisualTestContext| cx.update(|window, cx| container.contains_focused(window, cx));
    assert!(in_dialog(cx), "Tab from Allow went back into the dialog");

    click(cx, "card-allow");
    assert!(
        focused(cx, &card.allow),
        "the premise: Allow has focus again"
    );
    card.shown.set(false);
    redraw(cx);
    assert!(in_dialog(cx), "the card going sent focus into the dialog");
}

/// Why this matters: the old "host retirement releases before removal and preserves approval focus" and "SSH command
/// arrival preserves an independent approval owner". A user answering a card must not have focus pulled off it by the
/// dialog behind finishing on its own; and a card going with no dialog open must leave focus where keys still work.
///
/// A dialog whose subject goes (the retired host's) closes through the same close as one its opener closes, so the
/// opener's close stands for both.
///
/// Specified, where a click focuses controls: with focus put on Allow while a dialog is open, a late move the dialog
/// makes inside itself does nothing, and the dialog closing leaves focus on Allow; with no dialog open, the card going
/// sends focus home.
#[gpui::test]
fn focus_on_an_approval_card_outlives_the_dialog_and_goes_home_with_the_card(
    cx: &mut TestAppContext,
) {
    let home = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let drawn = home.clone();
    let cx = open(cx, move |_, _| button("home", &drawn).into_any_element());
    cx.update(|window, cx| focus::set_home(Some(&home), window, cx));
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    let card = install_card(cx, top_right());
    assert!(focused(cx, &home), "the premise: focus is home");

    let save = handle(cx);
    let (dialog, _, _) = open_dialog(cx, vec![("save", save.clone())], &save);
    click(cx, "card-allow");
    assert!(
        focused(cx, &card.allow),
        "the premise: Allow has focus, with the dialog open"
    );
    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        focus::focus_in_layer(&save, &mark, window, cx);
    });
    settle(cx);
    assert!(
        focused(cx, &card.allow),
        "the dialog's late move left focus on Allow"
    );
    cx.update(|window, cx| layers::close(dialog, window, cx));
    settle(cx);
    assert!(!is_open(cx, dialog), "the premise: the dialog closed");
    assert!(focused(cx, &card.allow), "and focus stayed on Allow");

    card.shown.set(false);
    redraw(cx);
    assert!(
        focused(cx, &home),
        "with no dialog open, the card going sent focus home"
    );
}

/// Why this matters: the region is installed for the window's whole life and draws nothing most of the time. If the
/// empty region, or the box its view is laid out in, took presses, the page under the card's place would stop
/// working, and a press on the page's empty space would no longer send focus home. And a region counted as a layer
/// would stop every shortcut that refuses while something is open.
///
/// Specified, where a click focuses controls: with the card hidden, a press where it would be clicks the page control
/// drawn there, and a press on the page's empty space inside the card's place sends focus home; with the card shown, no
/// layer is open (`any_open`, `focus_layer_open` and `trap` all say so). And since the approvals feature installs the
/// region once, a reset ([`layers::close_all`]) must leave it: after one, the card is still drawn and still answers a
/// press.
#[gpui::test]
fn an_empty_approval_region_covers_nothing_and_no_region_is_a_layer(cx: &mut TestAppContext) {
    let presses = Rc::new(Cell::new(0));
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let (home, under) = (
        cx.update(|cx| cx.focus_handle().tab_stop(true)),
        cx.update(|cx| cx.focus_handle().tab_stop(true)),
    );
    let (counted, out, drawn_home, drawn_under) = (
        presses.clone(),
        anchor_out.clone(),
        home.clone(),
        under.clone(),
    );
    let cx = open(cx, move |window, cx| {
        let anchor = Anchor::for_element("under-anchor", window, cx);
        out.replace(Some(anchor.clone()));
        let counted = counted.clone();
        div()
            .child(button("home", &drawn_home))
            .child(
                div()
                    .relative()
                    .child(
                        button("under", &drawn_under)
                            .on_click(move |_, _, _| counted.set(counted.get() + 1)),
                    )
                    .child(anchor.track()),
            )
            .into_any_element()
    });
    cx.update(|window, cx| focus::set_home(Some(&home), window, cx));
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    // The card's place is the page control's top-left corner: it would cover the control and the space beside it.
    let anchor = anchor_out.borrow().clone();
    let card = install_card(
        cx,
        Rc::new(move |_: &Window| anchor.as_ref()?.drawn_bounds().map(|at| at.origin)),
    );
    let place = bounds(cx, "card");
    card.shown.set(false);
    redraw(cx);
    assert!(
        cx.debug_bounds("card").is_none(),
        "the premise: the hidden card draws nothing"
    );

    let control = bounds(cx, "under");
    assert!(
        place.contains(&control.center()),
        "the premise: the card's place covers the control"
    );
    click(cx, "under");
    assert_eq!(
        presses.get(),
        1,
        "a press where the card would be clicked the control"
    );
    assert!(focused(cx, &under), "and focused it");
    let beside = point(control.right() + px(20.), control.center().y);
    assert!(
        place.contains(&beside),
        "the premise: the card's place covers the space beside the control"
    );
    cx.simulate_click(beside, Modifiers::none());
    check(cx);
    assert!(
        focused(cx, &home),
        "a press on the empty space there sent focus home"
    );

    card.shown.set(true);
    redraw(cx);
    assert!(
        cx.debug_bounds("card").is_some(),
        "the premise: the card shows"
    );
    let open_layer = cx.update(|window, cx| {
        layers::any_open(window, cx)
            || layers::focus_layer_open(window, cx)
            || layers::trap(window, cx).is_some()
    });
    assert!(!open_layer, "the region is not an open layer");

    // Deferred to after this update, as the reset runs from the channel's events (see the pill's reset test).
    cx.update(|_, cx| cx.defer(layers::close_all));
    settle(cx);
    assert!(
        cx.debug_bounds("card").is_some(),
        "after a reset the card is still drawn"
    );
    click(cx, "card-allow");
    assert_eq!(card.answers.get(), 1, "and still answers a press");
}

/// Why this matters: the cards sit below the session's tabs, which move with the user-dragged sidebar width and the
/// bands above them, so the region's view places its card from where the session view draws an element. It must do so
/// in the frame that moves the element: a card a frame behind would stay misplaced after a single change (a session
/// opening) until something else redrew.
///
/// Specified: after a frame, an anchor's `drawn_bounds` is where its element was drawn, and a card placed at its
/// bottom-left corner sits there; after the element grows, one redraw has both at the new bounds; once the element is
/// no longer drawn, `drawn_bounds` is `None`.
#[gpui::test]
fn an_anchors_drawn_bounds_follow_its_element_in_the_frame_that_moves_it(cx: &mut TestAppContext) {
    let area_shown = Rc::new(Cell::new(true));
    let area_size = Rc::new(Cell::new(gpui::size(px(400.), px(60.))));
    let anchor_out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let (shown_cell, size, out) = (area_shown.clone(), area_size.clone(), anchor_out.clone());
    let cx = open(cx, move |window, cx| {
        let page = div().pl(px(30.)).pt(px(10.));
        if !shown_cell.get() {
            return page.into_any_element();
        }
        let anchor = Anchor::for_element("tabs-anchor", window, cx);
        out.replace(Some(anchor.clone()));
        page.child(
            div()
                .relative()
                .debug_selector(|| "tabs".into())
                .w(size.get().width)
                .h(size.get().height)
                .child(anchor.track()),
        )
        .into_any_element()
    });
    let anchor = anchor_out.borrow().clone().unwrap();
    let read = anchor.clone();
    install_card(
        cx,
        Rc::new(move |_: &Window| read.drawn_bounds().map(|at| at.bottom_left())),
    );
    let drawn = |cx: &mut VisualTestContext| cx.update(|_, _| anchor.drawn_bounds());

    let tabs = bounds(cx, "tabs");
    assert_eq!(
        drawn(cx),
        Some(tabs),
        "drawn_bounds is where the element was drawn"
    );
    assert_eq!(
        bounds(cx, "card").origin,
        tabs.bottom_left(),
        "and the card sits below it"
    );

    area_size.set(gpui::size(px(500.), px(140.)));
    redraw(cx);
    let tabs = bounds(cx, "tabs");
    assert_eq!(
        tabs.size,
        gpui::size(px(500.), px(140.)),
        "the premise: the element is drawn at its new size"
    );
    assert_eq!(drawn(cx), Some(tabs), "drawn_bounds followed the element");
    assert_eq!(
        bounds(cx, "card").origin,
        tabs.bottom_left(),
        "and in the frame that grew it, the card sits below its new bounds"
    );

    area_shown.set(false);
    redraw(cx);
    assert_eq!(drawn(cx), None, "an element no longer drawn has no bounds");
}

/// Open a dialog of Other and Save, starting on Other, and give back its id and Save's handle: two dialogs opened this
/// way draw Save at the same place, so a card placed over one's Save covers the other's.
fn other_and_save(cx: &mut VisualTestContext) -> (LayerId, FocusHandle) {
    let (other, save) = (handle(cx), handle(cx));
    let (id, _, _) = open_dialog(
        cx,
        vec![("other", other.clone()), ("save", save.clone())],
        &other,
    );
    (id, save)
}

/// Why this matters: SPEC.md's tunnel warning can be asked from an approval card (its Tunnel and open), and the old app
/// drew the warning above the cards whatever opened it (z-index 50 over their 45); drawn under them, the card that
/// asked can cover the warning answering it. Every other dialog stays under the cards, which "remain usable" over it,
/// and the raise must go with the layer that asked for it.
///
/// Specified, where a click focuses controls: with a card drawn over a dialog's Save, a click there answers the card;
/// with a dialog of the same controls raised above the approval region in its place, the same click answers nothing
/// and focuses Save, and the frame draws the card, then the raised dialog's box (the second layer opened: ids are not
/// reused), then a tooltip; once that dialog closes, the same click over a third, ordinary one answers the card again.
#[gpui::test]
fn a_raised_layers_box_is_drawn_above_the_cards_and_an_ordinary_dialog_under_them(
    cx: &mut TestAppContext,
) {
    let cx = open(cx, |_, _| div().into_any_element());
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    let (ordinary, _) = other_and_save(cx);
    let over = center(cx, "save");
    let card = install_card(
        cx,
        Rc::new(move |_: &Window| Some(over - point(px(10.), px(10.)))),
    );
    assert!(
        bounds(cx, "card").contains(&over),
        "the premise: the card covers Save"
    );
    cx.simulate_click(over, Modifiers::none());
    check(cx);
    assert_eq!(
        card.answers.get(),
        1,
        "over an ordinary dialog, the press on Save's place was the card's"
    );

    cx.update(|window, cx| layers::close(ordinary, window, cx));
    let (raised, save) = other_and_save(cx);
    cx.update(|window, cx| layers::above_approval_region(raised, window, cx));
    settle(cx);
    assert_eq!(
        center(cx, "save"),
        over,
        "the premise: the raised dialog's Save is where the first one's was"
    );
    cx.simulate_click(over, Modifiers::none());
    check(cx);
    assert_eq!(
        card.answers.get(),
        1,
        "over the raised dialog's box, the press answered no card"
    );
    assert!(focused(cx, &save), "and was Save's");

    let owner = cx.update(|_, cx| cx.new(|_| layers::TooltipOwner));
    cx.update(|window, cx| layers::show_tooltip_at(&owner, "Probe help".into(), over, window, cx));
    settle(cx);
    let nodes = frame(cx);
    let at = |found: &dyn Fn(&gpui::DriverNode) -> bool| nodes.iter().position(found);
    let element =
        |id: &'static str| move |node: &gpui::DriverNode| node.element_id.as_deref() == Some(id);
    let (card_at, layer_at) = (at(&element("card")), at(&element("layer-1")));
    let tooltip_at = at(&|node| {
        node.texts
            .iter()
            .any(|text| text.text.as_ref() == "Probe help")
    });
    assert!(
        matches!((card_at, layer_at, tooltip_at), (Some(card), Some(layer), Some(tip)) if card < layer && layer < tip),
        "drawn bottom to top: the card ({card_at:?}), the raised dialog ({layer_at:?}), the tooltip ({tooltip_at:?})"
    );

    cx.update(|window, cx| layers::close(raised, window, cx));
    other_and_save(cx);
    cx.simulate_click(over, Modifiers::none());
    check(cx);
    assert_eq!(
        card.answers.get(),
        2,
        "over the next, ordinary dialog, the press was the card's again"
    );
}

/// Why this matters: SPEC.md has independent agent approval cards "remain usable" while any modal dialog is open, the
/// raised tunnel warning included (the foundations owner's decision of 2026-11-15, over the old warning's backdrop,
/// which blocked them). Only the warning's box goes above the cards; were its veil raised with it, every other agent's
/// card would wait on the warning. The veil is still drawn, on its own under the cards, and is all that keeps the page
/// behind the security warning from taking presses.
///
/// Specified, where a click focuses controls: with a dialog raised above the approval region, a press on a page button
/// clear of its box and of a card reaches nothing and leaves focus in the dialog; a press on the card's Allow, clear of
/// the box, answers the card and focuses Allow, and the dialog stays open.
#[gpui::test]
fn cards_beside_a_raised_dialog_stay_usable(cx: &mut TestAppContext) {
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
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    let behind = center(cx, "behind");
    let card = install_card(cx, top_right());
    let save = handle(cx);
    let (raised, _, _) = open_dialog(cx, vec![("save", save.clone())], &save);
    cx.update(|window, cx| layers::above_approval_region(raised, window, cx));
    settle(cx);
    assert!(
        !bounds(cx, "card").intersects(&bounds(cx, "layer-0")),
        "the premise: the card is clear of the dialog's box"
    );
    assert!(
        !bounds(cx, "layer-0").contains(&behind) && !bounds(cx, "card").contains(&behind),
        "the premise: the page button is clear of the dialog's box and the card"
    );
    cx.simulate_click(behind, Modifiers::none());
    check(cx);
    assert_eq!(
        clicks.get(),
        0,
        "the page behind the raised dialog took no press: its veil did"
    );
    assert!(focused(cx, &save), "and focus stayed in the dialog");
    click(cx, "card-allow");
    assert_eq!(
        card.answers.get(),
        1,
        "the press beside the raised dialog answered the card"
    );
    assert!(focused(cx, &card.allow), "and focused Allow");
    assert!(is_open(cx, raised), "with the dialog still open");
}

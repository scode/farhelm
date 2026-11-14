//! Behavior tests for the layer manager, on GPUI's simulated platform: the modal dialog (its veil, its empty-space
//! rule, its main action and its dismissal hook), the refusal to open a layer once the user has moved on, subjects
//! and anchors, popovers and menus, and that the layer stack redraws only the root.
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
    KeyDownEvent, Keystroke, Modifiers, ParentElement, Render, StatefulInteractiveElement as _,
    Styled, TestAppContext, VisualTestContext, Window, div, point, px,
};

use crate::cache::{self, Cached};
use crate::components::{Button, HoverHelp, Role, Tier};
use crate::focus::{self, Mark};
use crate::layers::{self, Anchor, Dialog, DialogPlacement, Dismissal, LayerId, Popover, Subject};
use crate::prompt;
use crate::render_count;
use crate::test_support::{
    Panel, Thing, anchor_element, bounds, button, center, check, click, focused, follow, handle,
    is_open, keys, open, open_dialog, open_menu, question, redraw,
};

// ---- Layers: a dialog is modal --------------------------------------------------------------------------------------

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

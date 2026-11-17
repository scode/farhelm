//! Tests of the select: its list's place, keys and clicks, unavailable and retained values, the placeholder, the open
//! list following its owner, Enter on the closed control running the owner's main action, and the focus coordinator's
//! rule for a press on a dialog's empty space while the list is open.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement, Modifiers,
    MouseButton, ParentElement as _, Render, SharedString, Styled as _, TestAppContext,
    VisualTestContext, Window, div, px,
};

use crate::components::{Button, HoverHelp, Role, Select, SelectOption};
use crate::focus::{Mark, TabReach};
use crate::layers::{self, Dialog};
use crate::test_support::{bounds, check, click, focused_id, frame, keys, open, redraw};

use super::{press, release, stroke};

/// What a select test's owner holds: the chosen value, which the select is drawn from, every choice it heard, the
/// Enter presses that reached the element around the select, and two things the owner can change while the list is
/// open: whether the select is unavailable, and whether Low is ("No longer offered").
struct Picker {
    chosen: Rc<RefCell<Option<SharedString>>>,
    heard: Rc<RefCell<Vec<SharedString>>>,
    enters: Rc<Cell<usize>>,
    disabled: Rc<Cell<bool>>,
    low_gone: Rc<Cell<bool>>,
    /// Whether the select gets a main action ([`Select::on_enter`]), and the chosen value it ran with each time.
    main_action: Rc<Cell<bool>>,
    launched: Rc<RefCell<Vec<Option<SharedString>>>>,
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
            main_action: Rc::new(Cell::new(false)),
            launched: Rc::new(RefCell::new(Vec::new())),
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
            main_action: self.main_action.clone(),
            launched: self.launched.clone(),
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
    let select = if picker.main_action.get() {
        let (chosen, launched) = (picker.chosen.clone(), picker.launched.clone());
        select.on_enter(move |_, _| launched.borrow_mut().push(chosen.borrow().clone()))
    } else {
        select
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

/// A window drawing a 320-pixel select, `recipe`, of two options, `cube` (marked with the sandbox mark) and `bare`
/// (unmarked), its owner keeping whichever was chosen last, `cube` first.
fn open_marked(cx: &mut TestAppContext) -> &mut VisualTestContext {
    let chosen = Rc::new(RefCell::new(SharedString::from("cube")));
    open(cx, move |_, _| {
        let help = |words: &'static str| HoverHelp::tooltip(words);
        let choose = chosen.clone();
        div()
            .w(px(320.))
            .child(
                Select::new(
                    "recipe",
                    "Recipe",
                    help("Recipe: what to start from"),
                    Role::ComboBox,
                )
                .options([
                    SelectOption::new("cube", "Cube", help("A marked option"))
                        .mark(crate::assets::Icon::EnvironmentSandbox),
                    SelectOption::new("bare", "Bare", help("An unmarked option")),
                ])
                .chosen(chosen.borrow().clone())
                .on_choose(move |value, _, _| {
                    choose.replace(value.clone());
                }),
            )
            .into_any_element()
    })
}

/// Why this matters: SPEC.md's New sandbox "lists recipes with a cube mark", and the closed control shows the chosen
/// option, so a mark drawn only in the list would vanish once chosen, while one that stayed after choosing an unmarked
/// option would mislabel it; an unmarked option must stay unmarked, or every option would look like a sandbox's.
///
/// Specified: with `cube` chosen, the closed control draws its mark and keeps its label as its value; the open list draws
/// the mark on `cube` and none on `bare`; once `bare` is chosen, the closed control draws no mark.
#[gpui::test]
fn a_marked_option_draws_its_mark_in_the_list_and_while_chosen(cx: &mut TestAppContext) {
    let cx = open_marked(cx);
    assert!(
        cx.debug_bounds("recipe-mark").is_some(),
        "the closed control's mark"
    );
    let value = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("recipe"))
        .and_then(|node| node.value);
    assert_eq!(
        value.as_deref(),
        Some("Cube"),
        "the mark is no part of the value"
    );
    click(cx, "recipe");
    assert!(cx.debug_bounds("recipe-option-cube-mark").is_some());
    assert!(cx.debug_bounds("recipe-option-bare-mark").is_none());
    click(cx, "recipe-option-bare");
    redraw(cx);
    assert!(
        cx.debug_bounds("recipe-mark").is_none(),
        "the mark follows the choice"
    );
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
/// A right press makes no click at all, and focuses the control as any press does, so it must close the list too, or
/// the list stays open with focus outside it; the next click must still open the list.
///
/// Specified: with the list open, a press on the closed control released elsewhere closes the list; the next click on
/// the control opens it again. With the list open, a right press on the control closes it with focus on the control;
/// the next click opens it again.
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

    cx.simulate_mouse_down(control.center(), MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(control.center(), MouseButton::Right, Modifiers::none());
    check(cx);
    assert!(!list_open(cx), "a right press closed the list");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("effort"),
        "and focus is on the closed control, not lost with the list"
    );
    click(cx, "effort");
    assert!(list_open(cx), "the next click after it opened the list");
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

/// Why this matters: Enter on the launcher's closed effort, agent-type and host selects launched with the value shown
/// (SPEC.md: "Enter on the closed host picker, agent-type or effort dropdown ... launches with the values shown"),
/// while a choice made in the open list only chose, as the native pop-up's own Enter did. A list whose choosing Enter
/// also launched would start a session before the user finished choosing.
///
/// Specified, with Medium chosen and a main action: with focus on the closed control, Enter pressed and released runs
/// the action once with Medium chosen, opens nothing, reports no choice and stops there; in the open list, Down and
/// Enter choose High and close the list without running the action.
#[gpui::test]
fn enter_on_a_closed_select_with_a_main_action_runs_it_with_the_value_shown(
    cx: &mut TestAppContext,
) {
    let (cx, picker, _) = open_picker(cx, Some("medium"), plain);
    picker.main_action.set(true);
    redraw(cx);
    keys(cx, "tab tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("effort"),
        "the premise: Tab reached the closed control"
    );

    stroke(cx, "enter", false);
    assert_eq!(
        *picker.launched.borrow(),
        vec![Some(SharedString::from("medium"))],
        "Enter ran the action once, with Medium chosen"
    );
    assert!(!list_open(cx), "and opened nothing");
    assert!(picker.heard.borrow().is_empty(), "and reported no choice");
    assert_eq!(picker.enters.get(), 0, "and stopped there");

    keys(cx, "space");
    assert!(list_open(cx), "the premise: Space opened the list");
    keys(cx, "down");
    stroke(cx, "enter", false);
    assert!(!list_open(cx), "Enter in the list chose and closed it");
    assert_eq!(*picker.heard.borrow(), ["high"], "choosing High");
    assert_eq!(
        picker.launched.borrow().len(),
        1,
        "without running the action"
    );
}

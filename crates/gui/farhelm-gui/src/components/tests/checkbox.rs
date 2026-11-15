//! Tests of the checkbox and its switch look: what toggles it and what does not, where Tab reaches it, the role and
//! checked state it reports, the inline switch's words, and Enter running the owner's main action without toggling.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    InteractiveElement as _, IntoElement, Modifiers, ParentElement as _, Styled as _,
    TestAppContext, VisualTestContext, div, px,
};

use crate::components::{Checkbox, HoverHelp, Role};
use crate::test_support::{bounds, check, click, focused_id, frame, keys, open, redraw};

use super::{press, release, stroke};

/// Open a window drawing one checkbox under `resume` whose checked state lives in `checked`, toggled by its handler,
/// inside a parent that counts the Enter presses reaching it, with the switch look when `switch` and unavailable while
/// `disabled` holds. With `launched`, the checkbox gets a main action ([`Checkbox::on_enter`]) that records the checked
/// state it ran with there. Hands back the Enter count.
fn open_checkbox<'a>(
    cx: &'a mut TestAppContext,
    checked: &Rc<Cell<bool>>,
    disabled: &Rc<Cell<bool>>,
    switch: bool,
    launched: Option<&Rc<RefCell<Vec<bool>>>>,
) -> (&'a mut VisualTestContext, Rc<Cell<usize>>) {
    let enters = Rc::new(Cell::new(0));
    let (state, off, heard) = (checked.clone(), disabled.clone(), enters.clone());
    let launched = launched.cloned();
    let cx = open(cx, move |_, _| {
        let (state, heard) = (state.clone(), heard.clone());
        let role = if switch { Role::Switch } else { Role::CheckBox };
        let toggled = state.clone();
        let checkbox = Checkbox::new("resume", "Resume", HoverHelp::tooltip("Resume it"), role)
            .checked(state.get())
            .disabled(off.get())
            .on_toggle(move |next, _, _| toggled.set(next));
        let checkbox = if switch { checkbox.switch() } else { checkbox };
        let checkbox = match launched.clone() {
            Some(launched) => {
                checkbox.on_enter(move |_, _| launched.borrow_mut().push(state.get()))
            }
            None => checkbox,
        };
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
        let (cx, enters) = open_checkbox(cx, &checked, &disabled, switch, None);
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

/// On macOS with Keyboard navigation off, Tab reaches text fields and pop-ups only, and WebKit treated a checkbox as a
/// button there, so Tab skipped it (SPEC.md, "Keyboard focus"); with every control reached, as on Linux, Tab lands on
/// it. A checkbox given the pop-up rule instead would be a Tab stop a Mac user never had. Specified: with Tab reaching
/// text fields and pop-ups only, Tab from nothing focused does not focus the checkbox; with every control reached, it
/// does.
#[gpui::test]
fn tab_reaches_a_checkbox_only_where_it_reaches_every_control(cx: &mut TestAppContext) {
    let checked = Rc::new(Cell::new(false));
    let disabled = Rc::new(Cell::new(false));
    let (cx, _) = open_checkbox(cx, &checked, &disabled, false, None);
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
        let (cx, _) = open_checkbox(cx, &checked, &disabled, switch, None);
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

/// The Hosts heading's Details control is a switch drawn before its words, where a Settings row draws its words first
/// and the switch at the row's far end; drawing it the Settings way would move the words and stretch the control across
/// the heading. Specified: an inline switch paints its words to the right of where the control starts plus the switch's
/// 34 pixels, a Settings switch paints them at the control's start, and a click on an inline switch's words still
/// toggles it.
#[gpui::test]
fn an_inline_switch_draws_its_words_after_it_and_still_toggles(cx: &mut TestAppContext) {
    let checked = Rc::new(Cell::new(false));
    let state = checked.clone();
    let cx = open(cx, move |_, _| {
        let state = state.clone();
        div()
            .w(px(300.))
            .child(
                Checkbox::new(
                    "inline",
                    "Details",
                    HoverHelp::tooltip("Show details"),
                    Role::Switch,
                )
                .switch()
                .inline()
                .checked(state.get())
                .on_toggle(move |next, _, _| state.set(next)),
            )
            .child(
                Checkbox::new(
                    "row",
                    "Sounds",
                    HoverHelp::tooltip("Play sounds"),
                    Role::Switch,
                )
                .switch(),
            )
            .into_any_element()
    });
    let nodes = frame(cx);
    let words_left = |id: &str, word: &str| {
        let control = nodes
            .iter()
            .position(|node| node.element_id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("{id} was drawn"));
        let text = nodes
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                let mut at = Some(*index);
                while let Some(here) = at {
                    if here == control {
                        return true;
                    }
                    at = nodes[here].parent;
                }
                false
            })
            .flat_map(|(_, node)| node.texts.iter())
            .find(|text| text.text.as_ref() == word)
            .unwrap_or_else(|| panic!("{id} painted {word}"));
        (nodes[control].bounds.left(), text.bounds.left())
    };
    let (inline_start, inline_words) = words_left("inline", "Details");
    assert!(
        inline_words >= inline_start + px(34.),
        "the words follow the switch: {inline_start:?} {inline_words:?}"
    );
    let (row_start, row_words) = words_left("row", "Sounds");
    assert_eq!(row_words, row_start, "a Settings switch's words come first");

    let words = nodes
        .iter()
        .flat_map(|node| node.texts.iter())
        .find(|text| text.text.as_ref() == "Details")
        .map(|text| text.bounds.center())
        .expect("the words were painted");
    cx.simulate_click(words, Modifiers::none());
    check(cx);
    assert!(checked.get(), "a click on the words toggles it");
}

/// Why this matters: Enter on the launcher's Resume checkbox launched with the box as shown and did not toggle it (the
/// old launcher's `enter_choice` applied nothing there; SPEC.md: "Enter on the ... Resume checkbox launches with the
/// values shown, without changing them"). A box that toggled on Enter would launch with the opposite of what the user
/// saw. Specified, with a main action: with focus on the checked box, Enter pressed and released runs the action once
/// with it checked, leaves it checked and stops there.
#[gpui::test]
fn enter_on_a_checkbox_with_a_main_action_runs_it_without_toggling(cx: &mut TestAppContext) {
    let checked = Rc::new(Cell::new(false));
    let disabled = Rc::new(Cell::new(false));
    let launched = Rc::new(RefCell::new(Vec::new()));
    let (cx, enters) = open_checkbox(cx, &checked, &disabled, false, Some(&launched));
    // The tests run under Linux's rule, where a click focuses a control.
    click(cx, "resume");
    redraw(cx);
    assert!(checked.get(), "the premise: a click turned it on");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("resume"),
        "the premise: it has focus"
    );
    stroke(cx, "enter", false);
    assert_eq!(
        *launched.borrow(),
        vec![true],
        "Enter ran the action once, checked"
    );
    assert!(checked.get(), "and did not toggle it");
    assert_eq!(enters.get(), 0, "and stopped there");
}

//! Tests of the radio group and its segmented, button group and chip looks: the keys and clicks that choose, where Tab
//! enters and leaves, Enter and the owner's main action, the roles and chosen state reported, whose hover help wins,
//! and how glyph, danger and described options are drawn without changing how they are reached.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    InteractiveElement as _, IntoElement, Modifiers, ParentElement as _, Styled as _,
    TestAppContext, VisualTestContext, div, point, px,
};

use crate::assets::Icon;
use crate::components::{Button, HoverHelp, RadioGroup, RadioOption, Role};
use crate::test_support::{bounds, center, check, click, focused_id, frame, open, redraw};

use super::{held_key, painted_under, press_and_draw, stroke, tooltip, wait_hover_delay};

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
/// reported once, and again reports nothing; with the owner's action, Enter, pressed and released, runs it once with
/// nothing chosen and does not reach what holds the group, and a held Enter does nothing at all.
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
    stroke(cx, "enter", false);
    assert_eq!(
        *owner.launched_with.borrow(),
        vec![None],
        "Enter ran the owner's action once, nothing chosen"
    );
    assert_eq!(owner.enters_passed.get(), 0, "and stopped at the group");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("choice-A"),
        "the premise: focus is still on A"
    );
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
/// chosen C reports it again. With the owner's action, Enter on B, pressed and released, chooses B once and then runs
/// the action once with B chosen, and stops at the group.
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
    stroke(cx, "enter", false);
    assert_eq!(
        *owner.launched_with.borrow(),
        vec![Some("B")],
        "Enter chose B, then ran the action"
    );
    assert_eq!(owner.choices.get(), 1, "its release chose nothing more");
    assert_eq!(owner.enters_passed.get(), 0, "and it stopped at the group");
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

/// Every option has help of its own, and so does the group around them; the group's must never cover an option's.
/// GPUI tells the group about a pointer move after its options, so with the last news winning, the group's help would
/// replace the option's on every move straight onto it. Specified: a pointer moving from empty space straight onto B
/// shows B's help after the hover delay; resting on the group's edge, outside every option, shows the group's; moving
/// on from there onto A shows A's at once, and moving from A back onto the edge shows the group's again at once.
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
    cx.simulate_mouse_move(
        point(group.left() + px(2.), group.center().y),
        None,
        Modifiers::none(),
    );
    check(cx);
    assert_eq!(
        tooltip(cx),
        Some("Pick a theme".into()),
        "back on the edge, the group's help came back at once"
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
/// reports the button role, its label and B alone pressed, is 26 pixels high and sits 6 pixels after the chip before
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
            (chip.size.height.as_f32() - 26.).abs() < 0.5,
            "a chip is 26 pixels high: {chip:?}"
        );
    }
    for pair in chips.windows(2) {
        assert!(
            ((pair[1].left() - pair[0].right()).as_f32() - 6.).abs() < 0.5,
            "chips are 6 pixels apart: {pair:?}"
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

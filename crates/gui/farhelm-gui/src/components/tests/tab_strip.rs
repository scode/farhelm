//! Tests of the tab strip: its keys, Enter choosing and then running the owner's main action, and the roles it reports.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    IntoElement, ParentElement as _, Styled as _, TestAppContext, VisualTestContext, div, px,
};

use crate::components::{Button, HoverHelp, Role};
use crate::test_support::{focused_id, frame, open, redraw};

use super::{held_key, press_and_draw};

/// What the owner of a test's tab strip sees: the tab chosen now (its state, which the strip's choices set) and how
/// often its main action ran.
#[derive(Clone, Default)]
struct Strip {
    chosen: Rc<Cell<&'static str>>,
    launched: Rc<Cell<usize>>,
}

/// Open a window drawing a button `before`, a tab strip `strip` of tabs A, B and C (tab `X`'s id is `strip-X`) with A
/// chosen and the owner's main action as its Enter, and a button `after`.
fn open_strip(cx: &mut TestAppContext) -> (&mut VisualTestContext, Strip) {
    let owner = Strip::default();
    owner.chosen.set("A");
    let kept = owner.clone();
    let cx = open(cx, move |_, _| {
        let (chose, launched) = (kept.chosen.clone(), kept.launched.clone());
        let mut strip = crate::components::TabStrip::new(
            "strip",
            "Launch kind",
            HoverHelp::tooltip("Pick a kind"),
        )
        .chosen(kept.chosen.get())
        .on_choose(move |value, _, _| chose.set(*value))
        .on_enter(move |_, _| launched.set(launched.get() + 1));
        for label in ["A", "B", "C"] {
            strip = strip.tab(crate::components::TabOption::new(
                label,
                HoverHelp::tooltip(format!("Use {label}")),
                label,
            ));
        }
        let help = || HoverHelp::tooltip("A test control");
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(Button::new("before", "Before", help(), Role::Button))
            .child(strip)
            .child(Button::new("after", "After", help(), Role::Button))
            .into_any_element()
    });
    (cx, owner)
}

/// The tab strip's keys, as the old launcher's Agent and Command tabs had them: one tab stop, at the chosen tab;
/// Left and Right choose the previous and next tab and wrap at the ends, Home and End the first and last, and focus
/// goes with the choice; Space chooses; Enter chooses and then runs the owner's main action, once, and a held Enter
/// does nothing. Why this matters: the strip is the launcher's way between its two tabs, SPEC.md has Enter on a
/// launch-kind tab apply it and launch, and a held Enter that launched again would start a second session.
#[gpui::test]
fn the_tab_strip_moves_with_the_arrows_and_enter_chooses_then_launches(cx: &mut TestAppContext) {
    let (cx, owner) = open_strip(cx);
    press_and_draw(cx, "tab");
    press_and_draw(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("strip-A"),
        "the premise: Tab entered at the chosen A"
    );
    for (key, expected) in [
        ("right", "B"),
        ("end", "C"),
        ("right", "A"),
        ("left", "C"),
        ("home", "A"),
    ] {
        press_and_draw(cx, key);
        assert_eq!(owner.chosen.get(), expected, "{key} chose {expected}");
        let id = format!("strip-{expected}");
        assert_eq!(
            focused_id(cx).as_deref(),
            Some(id.as_str()),
            "{key} took focus to {expected}"
        );
    }
    press_and_draw(cx, "shift-right");
    assert_eq!(owner.chosen.get(), "A", "a modified arrow changed nothing");

    press_and_draw(cx, "right");
    owner.chosen.set("A");
    redraw(cx);
    press_and_draw(cx, "space");
    assert_eq!(owner.chosen.get(), "B", "Space chose the focused B");
    assert_eq!(owner.launched.get(), 0, "and launched nothing");

    owner.chosen.set("A");
    redraw(cx);
    press_and_draw(cx, "enter");
    assert_eq!(owner.chosen.get(), "B", "Enter chose the focused B");
    assert_eq!(owner.launched.get(), 1, "and then ran the main action");
    held_key(cx, "enter");
    assert_eq!(owner.launched.get(), 1, "a held Enter did nothing");

    press_and_draw(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("after"),
        "the strip is one tab stop: the next Tab left it"
    );
}

/// The strip reports the tab list role with its label, and each tab the tab role, its words as its name and whether it
/// is selected, so a screen reader can say which kind of launch is chosen.
#[gpui::test]
fn the_tab_strip_reports_its_roles_and_which_tab_is_selected(cx: &mut TestAppContext) {
    let (cx, _) = open_strip(cx);
    let nodes = frame(cx);
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .map(|node| (node.role, node.label.clone(), node.selected))
    };
    assert_eq!(
        node("strip").map(|(role, label, _)| (role, label)),
        Some((Some(Role::TabList), Some("Launch kind".into())))
    );
    for (label, selected) in [("A", true), ("B", false), ("C", false)] {
        assert_eq!(
            node(&format!("strip-{label}")),
            Some((Some(Role::Tab), Some(label.into()), Some(selected))),
            "tab {label}"
        );
    }
}

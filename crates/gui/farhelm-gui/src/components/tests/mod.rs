//! Behavior tests for the components, on GPUI's simulated platform.
//!
//! These cover what a component promises beyond its looks (the goldens pin those): activation, the disabled state,
//! hover help reaching the layer manager and leaving with its control, the dialog frame keeping its answers reachable,
//! the text input's events, with the line breaks only the field that keeps them keeps, the arrows a field hands to its
//! owner, the option row that never takes focus, the mark toggle that keeps its click from the control around it, and
//! the select's list, keys and focus (with the focus coordinator's rule for a press on a dialog's empty space while a
//! select's list is open). Each test opens its window through the shared harness ([`crate::test_support::open`]), and
//! each step ends with its invariant checks (the focus coordinator's and the layer manager's rules).
//!
//! Each component's tests live in a file named after the component's own file under `components/` (`button.rs` for
//! `components/button.rs`, and so on), so a change to one control touches one test file rather than a file every
//! control's PR edits. Two files test the layer's shared rules from `components/mod.rs` instead of a control:
//! `hover_help.rs` (hover help and its tooltip) and `enter_choice.rs` (the choice controls' Enter rule). `transition.rs`
//! tests the one-shot entrance (`components/transition.rs`), which draws no control. A new control's tests go in a new
//! file here, declared below.
//!
//! This file holds only the step helpers that tests of more than one component use: reading the tooltip and waiting
//! out the hover delay, sending a key's press and release separately (GPUI's keyboard click happens on the release),
//! sending keys and then drawing a frame, holding a key down for its repeats, reading what an element's subtree
//! painted, and pressing every key a scroll stop takes; and the two fixtures the scroll box's and the dialog body's
//! scroll stop tests share (numbered lines of content, and an element's role and name). A fixture that one
//! component's tests use stays in that component's file.

mod button;
mod checkbox;
mod copy_button;
mod custom_flags;
mod dialog;
mod divider;
mod enter_choice;
mod hover_help;
mod mark_toggle;
mod menu_item;
mod option_row;
mod radio_group;
mod scroll_box;
mod scrollbar;
mod select;
mod sequence_pill;
mod status_mark;
mod tab_strip;
mod text_input;
mod transition;

use gpui::{
    Div, InteractiveElement as _, ParentElement as _, Pixels, SharedString, VisualTestContext, div,
    px,
};

use crate::components::Role;

use crate::layers;
use crate::test_support::{advance, bounds, check, frame, keys, redraw};

/// The text of the tooltip showing now, if any.
fn tooltip(cx: &mut VisualTestContext) -> Option<gpui::SharedString> {
    cx.update(|window, cx| layers::shown_tooltip(window, cx))
        .map(|shown| shown.text)
}

/// Wait out the hover delay on the executor's clock, the way a pointer resting on a control does, and check.
fn wait_hover_delay(cx: &mut VisualTestContext) {
    advance(cx, layers::HOVER_DELAY);
}

/// Press `chord` (with `held`, as an auto-repeat of a key already down), as the platform reports a key-down.
fn press(cx: &mut VisualTestContext, chord: &'static str, held: bool) {
    cx.simulate_event(gpui::KeyDownEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse(chord).unwrap(),
        is_held: held,
        prefer_character_input: false,
    });
    cx.run_until_parked();
    check(cx);
}

/// Release `chord`. GPUI turns the release of an Enter or Space pressed on a focused element into a click, which only a
/// test that sends the release can see.
fn release(cx: &mut VisualTestContext, chord: &'static str) {
    cx.simulate_event(gpui::KeyUpEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse(chord).unwrap(),
    });
    cx.run_until_parked();
    check(cx);
}

/// [`press`] and then [`release`] `chord`, as a real key does, drawing the owner's new state after each half, so a
/// control that acted again on the release (GPUI's keyboard click) shows it.
fn stroke(cx: &mut VisualTestContext, chord: &'static str, held: bool) {
    press(cx, chord, held);
    redraw(cx);
    release(cx, chord);
    redraw(cx);
}

/// Press `keystrokes` and draw the owner's new state, as an owner that notifies on a change would.
fn press_and_draw(cx: &mut VisualTestContext, keystrokes: &str) {
    keys(cx, keystrokes);
    redraw(cx);
}

/// Press `chord` as an auto-repeat of a key already down, and check.
fn held_key(cx: &mut VisualTestContext, chord: &'static str) {
    cx.simulate_event(gpui::KeyDownEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse(chord).unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    cx.run_until_parked();
    check(cx);
}

/// What the element drawn under `id` painted in `nodes`, the driver record of one frame: the strings its subtree
/// painted, and how many SVG glyphs that subtree holds. The record has no ink colors, so a glyph's or a word's color
/// is the goldens' to pin.
pub(super) fn painted_under(nodes: &[gpui::DriverNode], id: &str) -> (Vec<String>, usize) {
    let root = nodes
        .iter()
        .position(|node| node.element_id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("{id} was drawn"));
    let within = |mut index: usize| loop {
        if index == root {
            return true;
        }
        match nodes[index].parent {
            Some(parent) => index = parent,
            None => return false,
        }
    };
    let under: Vec<&gpui::DriverNode> = (0..nodes.len())
        .filter(|&index| within(index))
        .map(|index| &nodes[index])
        .collect();
    let texts = under
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect();
    let glyphs = under
        .iter()
        .filter(|node| node.type_name.ends_with("Svg"))
        .count();
    (texts, glyphs)
}

/// Press each key a focused scroll stop takes, checking how far each one scrolls the element drawn under `line` at the
/// top of the scrolling content: Down a little and Up back to the start, Page Down further than Down and Page Up back,
/// Space as far as Page Down, End further still, and Home back to the start. The stop must hold focus with its content
/// at the top, and the content must run several pages past the region, or End would land where Page Down does.
///
/// Every key is checked through the drawn position of the content, after only the frame the key itself asks for, so a
/// key that scrolled without redrawing fails too.
fn each_scroll_key_scrolls(cx: &mut VisualTestContext, line: &'static str) {
    let start = bounds(cx, line).top();
    let moved = |cx: &mut VisualTestContext, key: &str| -> Pixels {
        keys(cx, key);
        start - bounds(cx, line).top()
    };
    let down = moved(cx, "down");
    assert!(down > px(0.), "Down scrolled the content up: {down:?}");
    assert_eq!(moved(cx, "up"), px(0.), "Up scrolled back to the start");
    let page = moved(cx, "pagedown");
    assert!(
        page > down,
        "Page Down scrolled further than Down: {page:?}"
    );
    assert_eq!(moved(cx, "pageup"), px(0.), "Page Up scrolled back");
    assert_eq!(
        moved(cx, "space"),
        page,
        "Space scrolled a page, as Page Down"
    );
    let end = moved(cx, "end");
    assert!(end > page, "End scrolled furthest: {end:?}");
    assert_eq!(moved(cx, "home"), px(0.), "Home scrolled back to the start");
}

/// `count` lines of text, each `text` followed by its number from 1, drawn under `line-0`, `line-1` and so on.
fn lines(count: usize, text: &'static str) -> Div {
    div().children((0..count).map(move |index| {
        div()
            .debug_selector(move || format!("line-{index}"))
            .child(format!("{text} {}", index + 1))
    }))
}

/// The role and accessible name drawn under element id `id` in a fresh frame.
fn role_and_name(cx: &mut VisualTestContext, id: &str) -> (Option<Role>, Option<SharedString>) {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| (node.role, node.label))
        .unwrap_or_default()
}

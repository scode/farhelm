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
//! `hover_help.rs` (hover help and its tooltip) and `enter_choice.rs` (the choice controls' Enter rule). A new
//! control's tests go in a new file here, declared below.
//!
//! This file holds only the step helpers that tests of more than one component use: reading the tooltip and waiting
//! out the hover delay, sending a key's press and release separately (GPUI's keyboard click happens on the release),
//! sending keys and then drawing a frame, and holding a key down for its repeats. A fixture that one component's tests use stays in that component's file.

mod button;
mod checkbox;
mod copy_button;
mod dialog;
mod divider;
mod enter_choice;
mod hover_help;
mod mark_toggle;
mod option_row;
mod radio_group;
mod scrollbar;
mod select;
mod status_mark;
mod tab_strip;
mod text_input;

use gpui::VisualTestContext;

use crate::layers;
use crate::test_support::{advance, check, keys, redraw};

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

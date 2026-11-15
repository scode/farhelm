//! Tests of the button: a click and Enter or Space run it once, a disabled one is inert, a busy one keeps focus, its
//! help and the retired look while ignoring presses, and a pressed one reports its state with its label as its name.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    InteractiveElement as _, IntoElement, Modifiers, ParentElement as _, Styled as _,
    TestAppContext, div, px,
};

use crate::components::{Button, HoverHelp, Role, Tier};
use crate::test_support::{center, check, click, frame, keys, open, redraw};

use super::{tooltip, wait_hover_delay};

/// A click activates an enabled button, and a disabled one ignores it: the old app's disabled buttons were inert, and a
/// dimmed button that still acted would send a command the user was told was unavailable.
#[gpui::test]
fn a_click_activates_an_enabled_button_and_not_a_disabled_one(cx: &mut TestAppContext) {
    let clicks = Rc::new(Cell::new((0, 0)));
    let counted = clicks.clone();
    let cx = open(cx, move |_, _| {
        let (on, off) = (counted.clone(), counted.clone());
        div()
            .flex()
            .gap(px(8.))
            .child(
                Button::new("on", "Save", HoverHelp::tooltip("Save it"), Role::Button)
                    .tier(Tier::Primary)
                    .on_click(move |_, _, _| on.set((on.get().0 + 1, on.get().1))),
            )
            .child(
                Button::new("off", "Save", HoverHelp::tooltip("Save it"), Role::Button)
                    .tier(Tier::Primary)
                    .disabled(true)
                    .on_click(move |_, _, _| off.set((off.get().0, off.get().1 + 1))),
            )
            .into_any_element()
    });

    click(cx, "on");
    click(cx, "off");
    assert_eq!(
        clicks.get(),
        (1, 0),
        "the enabled button ran once, the disabled one never"
    );
}

/// Enter or Space on a focused button runs it once. GPUI also turns the release of an Enter or Space pressed on a
/// focused element into a click, so a button that ran on the press and ran again on that click would act twice on one
/// key press whenever focus stays on it (a toggle, a step button, a Save whose dialog stays open), sending a second
/// command; and a held Enter that answered one question, repeating onto the next question's focused answer, would
/// answer that one too when released (SPEC.md: "Repeated or composing Enter cannot answer the new question").
/// In-process tests that send only the press never saw it. Specified: with a button focused, Enter pressed and released
/// runs it once, and so does Space; a held Enter's repeat and its release run nothing; a click still runs it once.
#[gpui::test]
fn enter_or_space_on_a_focused_button_runs_it_once(cx: &mut TestAppContext) {
    let runs = Rc::new(Cell::new(0));
    let focus = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (counted, handle) = (runs.clone(), focus.clone());
    let cx = open(cx, move |_, _| {
        let counted = counted.clone();
        Button::new(
            "step",
            "Step",
            HoverHelp::tooltip("Step once"),
            Role::Button,
        )
        .focus(&handle)
        .on_click(move |_, _, _| counted.set(counted.get() + 1))
        .into_any_element()
    });
    keys(cx, "tab");
    assert!(
        cx.update(|window, _| focus.is_focused(window)),
        "the premise: the button has focus"
    );
    for key in ["enter", "space"] {
        let before = runs.get();
        cx.simulate_event(gpui::KeyDownEvent {
            native_key: None,
            keystroke: gpui::Keystroke::parse(key).unwrap(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(gpui::KeyUpEvent {
            native_key: None,
            keystroke: gpui::Keystroke::parse(key).unwrap(),
        });
        cx.run_until_parked();
        check(cx);
        assert_eq!(
            runs.get(),
            before + 1,
            "{key}, pressed and released, ran it once"
        );
    }
    // A key held down from an earlier press (the Enter that answered the question before this one) repeats onto the
    // button and is released there: neither the repeat nor the release runs it.
    let before = runs.get();
    cx.simulate_event(gpui::KeyDownEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    cx.simulate_event(gpui::KeyUpEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
    check(cx);
    assert_eq!(
        runs.get(),
        before,
        "a held Enter's repeat and release ran nothing"
    );
    let before = runs.get();
    click(cx, "step");
    assert_eq!(runs.get(), before + 1, "a click ran it once");
}

/// A busy button, whose action is in flight, keeps focus and its place in Tab's order but does nothing when pressed:
/// the old app's `aria-disabled` Save. Disabling it instead would drop focus out of the dialog's controls for the
/// length of the save. Specified: Tab reaches the busy button; a click, Enter and Space all leave it focused and run
/// nothing, and Enter and Space stop at it rather than reaching what holds it; once it is no longer busy, a click runs
/// it.
#[gpui::test]
fn a_busy_button_keeps_focus_and_ignores_presses(cx: &mut TestAppContext) {
    let runs = Rc::new(Cell::new(0));
    let busy = Rc::new(Cell::new(true));
    // Enter and Space that went past the button to the element holding it.
    let passed = Rc::new(Cell::new(0));
    let focus = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (counted, drawn, handle, heard) =
        (runs.clone(), busy.clone(), focus.clone(), passed.clone());
    let cx = open(cx, move |_, _| {
        let (counted, heard) = (counted.clone(), heard.clone());
        div()
            .on_key_down(move |event: &gpui::KeyDownEvent, _, _| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    heard.set(heard.get() + 1);
                }
            })
            .child(
                Button::new("save", "Save", HoverHelp::tooltip("Save it"), Role::Button)
                    .tier(Tier::Primary)
                    .busy(drawn.get())
                    .focus(&handle)
                    .on_click(move |_, _, _| counted.set(counted.get() + 1)),
            )
            .into_any_element()
    });
    keys(cx, "tab");
    assert!(
        cx.update(|window, _| focus.is_focused(window)),
        "Tab reached the busy button"
    );
    click(cx, "save");
    keys(cx, "enter");
    keys(cx, "space");
    assert_eq!(runs.get(), 0, "no press ran it while busy");
    assert_eq!(
        passed.get(),
        0,
        "Enter and Space stopped at the busy button"
    );
    assert!(
        cx.update(|window, _| focus.is_focused(window)),
        "and it kept focus"
    );
    busy.set(false);
    redraw(cx);
    click(cx, "save");
    assert_eq!(runs.get(), 1, "once not busy, a click ran it");
}

/// A busy button is also the old `aria-disabled` control whose reason is its hover help (the session header's Restart
/// for a session that cannot resume): unlike a disabled one, it must stay reachable, or the reason could never be read
/// from the keyboard, and its tooltip must still show. Its accessible name may say more than its label. Specified: Tab
/// reaches a busy neutral button; a click, Enter and Space run nothing and leave it focused, Enter and Space stopping
/// at it; resting the pointer on it shows its help; the frame names it by its accessible name while painting its label.
#[gpui::test]
fn a_busy_button_keeps_its_help_and_may_be_named_for_its_reason(cx: &mut TestAppContext) {
    let runs = Rc::new(Cell::new(0));
    let passed = Rc::new(Cell::new(0));
    let focus = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (counted, handle, heard) = (runs.clone(), focus.clone(), passed.clone());
    let cx = open(cx, move |_, _| {
        let (counted, heard) = (counted.clone(), heard.clone());
        div()
            .on_key_down(move |event: &gpui::KeyDownEvent, _, _| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    heard.set(heard.get() + 1);
                }
            })
            .child(
                Button::new(
                    "restart",
                    "Restart",
                    HoverHelp::tooltip("Restart unavailable — the reason"),
                    Role::Button,
                )
                .tier(Tier::Neutral)
                .compact()
                .busy(true)
                .accessible_name("Restart unavailable")
                .focus(&handle)
                .on_click(move |_, _, _| counted.set(counted.get() + 1)),
            )
            .into_any_element()
    });
    keys(cx, "tab");
    assert!(
        cx.update(|window, _| focus.is_focused(window)),
        "Tab reached the busy button"
    );
    click(cx, "restart");
    keys(cx, "enter");
    keys(cx, "space");
    assert_eq!(runs.get(), 0, "no press ran it");
    assert_eq!(passed.get(), 0, "Enter and Space stopped at it");
    assert!(
        cx.update(|window, _| focus.is_focused(window)),
        "and it kept focus"
    );
    let nodes = frame(cx);
    let node = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("restart"))
        .expect("the button was drawn");
    assert_eq!(node.label.as_deref(), Some("Restart unavailable"));
    assert!(
        nodes
            .iter()
            .flat_map(|node| &node.texts)
            .any(|text| text.text.as_ref() == "Restart"),
        "the label is what it paints"
    );
    let at = center(cx, "restart");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(
        tooltip(cx).as_deref(),
        Some("Restart unavailable — the reason")
    );
}

/// A busy button must look unavailable in every filled tier, or a greyed Restart would read as one that works; the
/// old sheet drew every `aria-disabled` filled button with `--bg-2`, `--border-dim` and `--fg-2`. The unfilled tiers
/// have no fill to retire and keep their look. Specified, in both palettes: busy neutral, chip, primary and danger
/// buttons take exactly the retired colors; a busy ghost button keeps the ghost's colors; a neutral one that is not busy
/// keeps the neutral fill.
#[test]
fn a_busy_filled_button_takes_the_retired_look_and_an_unfilled_one_keeps_its_own() {
    let button = |tier: Tier, busy: bool| {
        Button::new("b", "B", HoverHelp::tooltip("A button"), Role::Button)
            .tier(tier)
            .busy(busy)
    };
    for palette in [&crate::theme::DARK, &crate::theme::LIGHT] {
        let retired = (
            Some(palette.bg_2),
            palette.bg_2,
            Some(palette.border_dim),
            palette.fg_2,
        );
        for tier in [Tier::Neutral, Tier::Chip, Tier::Primary, Tier::Danger] {
            assert_eq!(button(tier, true).colors(palette), retired, "{tier:?}");
        }
        assert_eq!(
            button(Tier::Ghost, true).colors(palette),
            button(Tier::Ghost, false).colors(palette),
            "a ghost keeps its look"
        );
        assert_ne!(
            button(Tier::Neutral, false).colors(palette),
            retired,
            "a neutral button that is not busy keeps its fill"
        );
    }
}

/// A button that is one of a set of choices (the host rail's entries) must tell assistive technology which choice is
/// in force, as the old `aria-pressed` did, and a button showing content in place of its words (a host's mark) must
/// still be named by its label, since a mark says nothing to a screen reader. Specified: a pressed button reports the
/// toggled state on, one not pressed reports it off, and a plain button reports none; the pressed one, given content,
/// is named by its label and paints its content's text, never the label's.
#[gpui::test]
fn a_pressed_button_reports_its_state_and_content_keeps_the_label_as_its_name(
    cx: &mut TestAppContext,
) {
    let cx = open(cx, |_, _| {
        div()
            .flex()
            .gap(px(8.))
            .child(
                Button::new(
                    "on",
                    "All hosts",
                    HoverHelp::tooltip("Show every host"),
                    Role::Button,
                )
                .tier(Tier::Rail)
                .pressed(true)
                .content(div().child("All")),
            )
            .child(
                Button::new(
                    "off",
                    "Local",
                    HoverHelp::tooltip("Show one host"),
                    Role::Button,
                )
                .tier(Tier::Rail)
                .pressed(false),
            )
            .child(Button::new(
                "plain",
                "Save",
                HoverHelp::tooltip("Save it"),
                Role::Button,
            ))
            .into_any_element()
    });
    let nodes = frame(cx);
    let find = |id: &str| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("{id} was drawn"))
    };
    assert_eq!(find("on").toggled, Some(gpui::Toggled::True));
    assert_eq!(find("off").toggled, Some(gpui::Toggled::False));
    assert_eq!(find("plain").toggled, None);
    assert_eq!(find("on").label.as_deref(), Some("All hosts"));
    let painted: Vec<String> = nodes
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect();
    assert!(painted.iter().any(|text| text == "All"), "{painted:?}");
    assert!(
        !painted.iter().any(|text| text == "All hosts"),
        "{painted:?}"
    );
}

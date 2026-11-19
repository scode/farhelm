//! Tests of the scroll box: plain Tab reaches it where Tab skips buttons, it reports its region role and name, every
//! scroll key scrolls it, a press focuses it, modified keys and a control inside it keep their keys, and the old app's
//! case of an approval's long initial prompt reached by Shift+Tab from Allow.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    IntoElement, Modifiers, ParentElement as _, Styled as _, TestAppContext, div, point, px,
};

use crate::components::{Button, HoverHelp, Role, ScrollBox};
use crate::focus::TabReach;
use crate::test_support::{bounds, click, focused, focused_id, follow, keys, open, settle};

use super::{each_scroll_key_scrolls, lines, role_and_name};

/// When Tab reaches a box whose pointer help is already visible, the card centers even with the pointer still
/// off-center. The focus handoff schedules its own frame, so this observes the card before any forced redraw.
#[gpui::test]
fn keyboard_focus_centers_existing_scroll_box_help(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        div()
            .w(px(240.))
            .ml(px(100.))
            .child(
                ScrollBox::new(
                    "command",
                    "Command",
                    HoverHelp::tooltip("Read the rest of the command"),
                    Role::Region,
                    lines(40, "argument"),
                )
                .max_height(px(180.)),
            )
            .into_any_element()
    });
    let target = bounds(cx, "command");
    assert!(
        bounds(cx, "line-39").bottom() > target.bottom(),
        "the box really scrolls"
    );
    let at = point(target.center().x, target.bottom() - px(35.));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    settle(cx);
    super::wait_hover_delay(cx);
    assert!((bounds(cx, "tooltip-card").center().y - at.y).abs() <= px(1.));

    keys(cx, "tab");
    settle(cx);
    let centered = bounds(cx, "tooltip-card").center().y;
    assert_eq!(focused_id(cx).as_deref(), Some("command"));
    assert!((centered - target.center().y).abs() <= px(1.));
}

/// Why this matters: SPEC.md, "Reaching everything by keyboard", gives a region whose content holds nothing focusable
/// "its own tab stop with an accessible name", in every engine Farhelm ships on. An approval card's command and prompt
/// are such regions, and GPUI scrolls nothing from the keyboard by itself, so without the box's stop and keys a long
/// command could be read only with the pointer. On macOS with Keyboard navigation off, the system default, plain Tab
/// skips buttons; a stop counted as a button would leave those users no way in.
///
/// Specified: with Tab reaching text fields and pop-ups only, plain Tab passes over a button before the box and puts
/// focus on the box, which reports the region role and its label as its name; there, every scroll key moves the
/// content (each_scroll_key_scrolls).
#[gpui::test]
fn a_labelled_box_is_a_tab_stop_with_its_role_and_every_scroll_key_scrolls_it(
    cx: &mut TestAppContext,
) {
    let before = handle_for(cx);
    let drawn = before.clone();
    let cx = open(cx, move |_, _| {
        let help = HoverHelp::tooltip("The command the agent asks to run");
        div()
            .w(px(400.))
            .flex()
            .flex_col()
            .child(
                Button::new(
                    "before",
                    "Before",
                    HoverHelp::tooltip("A button"),
                    Role::Button,
                )
                .focus(&drawn),
            )
            .child(
                ScrollBox::new(
                    "command",
                    "Command",
                    help,
                    Role::Region,
                    lines(40, "argument"),
                )
                .max_height(px(100.)),
            )
            .into_any_element()
    });
    let reach = Rc::new(Cell::new(TabReach::TextFieldsAndPopups));
    follow(cx, &reach);
    assert!(
        bounds(cx, "line-39").bottom() > bounds(cx, "command").bottom(),
        "the premise: the content overflows the box"
    );

    keys(cx, "tab");
    assert!(!focused(cx, &before), "plain Tab passed over the button");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("command"),
        "and reached the box"
    );
    assert_eq!(
        role_and_name(cx, "command"),
        (Some(Role::Region), Some("Command".into())),
        "the box is a region named by its label"
    );
    each_scroll_key_scrolls(cx, "line-0");
}

/// Why this matters: a port of the old app's browser case "an approval's complete initial prompt is
/// keyboard-scrollable" (`keyboard-scroll.spec.ts` at reference commit `24bc0580`). The prompt is what the user
/// authorizes, so all of it must be readable from the keyboard, and the first control after it (Allow) is where a
/// keyboard user arrives from. The old case also checked the focus ring, since a stop the user cannot see is no way in;
/// its region role, name and Page Down are the first test's.
///
/// Specified: in a card whose 100-line prompt overflows its box, with Allow after the box, Shift+Tab from a focused
/// Allow puts focus on the box, which shows its focus ring.
#[gpui::test]
fn an_approvals_complete_initial_prompt_is_keyboard_scrollable(cx: &mut TestAppContext) {
    let allow = handle_for(cx);
    let drawn = allow.clone();
    let cx = open(cx, move |_, _| {
        let prompt = lines(
            100,
            "Prompt line: inspect the full request before allowing it.",
        );
        div()
            .w(px(520.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                ScrollBox::new(
                    "prompt",
                    "Initial prompt",
                    HoverHelp::tooltip("The prompt the new session starts with"),
                    Role::Region,
                    prompt,
                )
                .max_height(px(160.)),
            )
            .child(
                Button::new(
                    "allow",
                    "Allow",
                    HoverHelp::tooltip("Allow it"),
                    Role::Button,
                )
                .focus(&drawn),
            )
            .into_any_element()
    });
    assert!(
        bounds(cx, "line-99").bottom() > bounds(cx, "prompt").bottom(),
        "the premise: the prompt overflows its box"
    );
    keys(cx, "tab tab");
    assert!(focused(cx, &allow), "the premise: Allow holds focus");

    keys(cx, "shift-tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt"),
        "Shift+Tab reached the prompt"
    );
    assert!(
        cx.debug_bounds("prompt-ring").is_some(),
        "the box shows its focus ring"
    );
}

/// Why this matters: a press on the box focuses it, as a press on the old focusable box did in WebKit, so a pointer
/// user can go on with the keys. The keys are the box's only while the box itself holds focus: GPUI hands a key to
/// every element on the focused element's path, so without that rule Down on a link inside a box would scroll the box,
/// not reach the link's owner. And a modified key (Shift+Space, which paged up in browsers) is left to whatever holds
/// the box rather than read as its plain key.
///
/// Specified, in a box whose content, a button and then 40 lines, overflows it: a click on a line focuses the box;
/// there Shift+Space and Shift+Down move nothing; Tab then puts focus on the button inside, where Down moves nothing
/// and Space presses the button once, moving nothing either.
#[gpui::test]
fn a_press_focuses_the_box_and_neither_modified_keys_nor_its_controls_scroll_it(
    cx: &mut TestAppContext,
) {
    let inner = handle_for(cx);
    let pressed = Rc::new(Cell::new(0));
    let (drawn, count) = (inner.clone(), pressed.clone());
    let cx = open(cx, move |_, _| {
        let count = count.clone();
        let button = Button::new("inner", "Copy", HoverHelp::tooltip("Copy it"), Role::Button)
            .focus(&drawn)
            .on_click(move |_, _, _| count.set(count.get() + 1));
        let content = div().child(button).child(lines(40, "note"));
        let help = HoverHelp::tooltip("Notes");
        div()
            .w(px(400.))
            .child(
                ScrollBox::new("notes", "Notes", help, Role::Region, content).max_height(px(100.)),
            )
            .into_any_element()
    });
    assert!(
        bounds(cx, "line-39").bottom() > bounds(cx, "notes").bottom(),
        "the premise: the content overflows the box"
    );
    let top = bounds(cx, "line-0").top();
    assert_ne!(
        focused_id(cx).as_deref(),
        Some("notes"),
        "the premise: the box is not focused yet"
    );

    click(cx, "line-1");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("notes"),
        "the press focused the box"
    );
    keys(cx, "shift-space shift-down");
    assert_eq!(
        bounds(cx, "line-0").top(),
        top,
        "modified keys did not scroll it"
    );

    keys(cx, "tab");
    assert!(
        focused(cx, &inner),
        "the premise: the button inside holds focus"
    );
    keys(cx, "down space");
    assert_eq!(
        bounds(cx, "line-0").top(),
        top,
        "the button's keys did not scroll the box"
    );
    assert_eq!(pressed.get(), 1, "Space pressed the button");
}

/// A tab-stop focus handle made before the window opens, for a control the test draws and then asks about.
fn handle_for(cx: &mut TestAppContext) -> gpui::FocusHandle {
    cx.update(|cx| cx.focus_handle().tab_stop(true))
}

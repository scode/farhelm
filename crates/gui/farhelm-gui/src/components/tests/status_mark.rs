//! Tests of the status mark: it is named by its label and shows its help on hover, also when it draws other content,
//! and is never a control.

use gpui::{IntoElement, Modifiers, ParentElement as _, Styled as _, TestAppContext, div, px};

use crate::components::{Button, HoverHelp, Role};
use crate::layers::{self, TooltipSide};
use crate::test_support::{center, frame, keys, open};

use super::{tooltip, wait_hover_delay};

/// The notch still identifies a status mark when its help sits outside a surface. Even help configured with a
/// control's rounded ring keeps the ring off when a status mark owns it.
#[gpui::test]
fn a_status_mark_keeps_beside_help_without_a_ring(cx: &mut TestAppContext) {
    let cx = open(cx, |window, cx| {
        div()
            .relative()
            .w(px(220.))
            .child(layers::beside_edge("status-help-edge", window, cx))
            .child(crate::components::StatusMark::new(
                "phase",
                "Connected",
                HoverHelp::tooltip("Host is connected").ring_radius(px(4.)),
                Role::Status,
                "Connected",
            ))
            .into_any_element()
    });
    let at = center(cx, "phase");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    wait_hover_delay(cx);
    cx.update(|window, cx| {
        let shown = layers::shown_tooltip(window, cx).expect("the mark's own help");
        assert_eq!(shown.text.as_ref(), "Host is connected");
        assert_eq!(shown.side(), TooltipSide::Beside);
        assert!(shown.ring().is_none());
    });
    assert!(cx.debug_bounds("tooltip-notch").is_some());
}

/// A status mark is information, not a control: the old app's `role="status"` span with its label and tooltip. A user
/// who sees only its symbol needs its words on hover, a screen reader needs them as its name, and a keyboard user must
/// not stop on it. Specified: a status mark showing "?" is drawn with the status role and its label as its name, paints
/// "?", takes no focus from Tab (which goes on to the button after it) and has no click handler, and resting the
/// pointer on it shows its hover help.
#[gpui::test]
fn a_status_mark_is_named_and_helps_on_hover_but_is_no_tab_stop(cx: &mut TestAppContext) {
    let after = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let focus = after.clone();
    let cx = open(cx, move |_, _| {
        div()
            .flex()
            .gap(px(8.))
            .child(crate::components::StatusMark::new(
                "unloaded",
                "The list could not be loaded",
                HoverHelp::tooltip("The list could not be loaded"),
                Role::Status,
                "?",
            ))
            .child(
                Button::new(
                    "after",
                    "After",
                    HoverHelp::tooltip("A test control"),
                    Role::Button,
                )
                .focus(&focus),
            )
            .into_any_element()
    });
    keys(cx, "tab");
    assert!(
        cx.update(|window, _| after.is_focused(window)),
        "Tab went past the mark to the button"
    );
    let nodes = frame(cx);
    let mark = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("unloaded"))
        .expect("the mark was drawn");
    assert_eq!(mark.role, Some(Role::Status));
    assert_eq!(mark.label.as_deref(), Some("The list could not be loaded"));
    assert!(!mark.flags.focusable && !mark.flags.click, "not a control");
    assert!(
        nodes
            .iter()
            .any(|node| node.texts.iter().any(|text| text.text.as_ref() == "?")),
        "it paints its text"
    );

    let at = center(cx, "unloaded");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(tooltip(cx).as_deref(), Some("The list could not be loaded"));
}

/// A status that shows more than a word (the Hosts panel's phase dot with its words, a host's identity mark) is still a
/// status mark: its label must stay its name, since what it draws may say nothing to a screen reader, and its hover help
/// must still show. Specified: a status mark given content is named by its label, paints the content's text and not its
/// own, is no control, and shows its help on hover (here with the help not also its description).
#[gpui::test]
fn a_status_mark_with_content_keeps_its_name_and_help(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        div()
            .child(
                crate::components::StatusMark::new(
                    "phase",
                    "Connected",
                    HoverHelp::tooltip("Connected"),
                    Role::Status,
                    "unused",
                )
                .described(false)
                .content(div().flex().child("●").child("Connected")),
            )
            .into_any_element()
    });
    let nodes = frame(cx);
    let mark = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("phase"))
        .expect("the mark was drawn");
    assert_eq!(mark.role, Some(Role::Status));
    assert_eq!(mark.label.as_deref(), Some("Connected"));
    assert!(!mark.flags.focusable && !mark.flags.click, "not a control");
    let painted: Vec<String> = nodes
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect();
    assert!(painted.iter().any(|text| text == "●"), "{painted:?}");
    assert!(!painted.iter().any(|text| text == "unused"), "{painted:?}");

    let at = center(cx, "phase");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    wait_hover_delay(cx);
    assert_eq!(tooltip(cx).as_deref(), Some("Connected"));
}

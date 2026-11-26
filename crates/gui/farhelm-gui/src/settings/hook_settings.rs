//! One agent's all-hosts hook setup switch and the shared preference that restores its prompt and hint.

use gpui::{App, Div, FocusHandle, ParentElement as _, Styled as _, Window, div, px};

use super::{card, choices, configured};
use crate::components::{Checkbox, HoverHelp, Role};
use crate::hook_setup::HookAgent;
use crate::prompt::ActionFrame;

pub(super) const LABEL: &str = "Set up conversation hooks on all hosts";
pub(super) const TOOLTIP: &str = "Let Farhelm install its conversation hooks in this agent's configuration whenever it starts the agent";

/// The switch keeps its focus target stable while the radio group, read, or save changes.
pub(super) struct HookSwitch {
    pub(super) state: configured::Configured,
    pub(super) focus: FocusHandle,
    pub(super) read_started: bool,
}

pub(super) fn row(
    agent: HookAgent,
    switch: &HookSwitch,
    refused: &super::Refused,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let checked = switch.state.shown();
    let available = switch.state.available();
    // A refusal can reverse the switch during a keyboard redraw. Only a shown state can authorize all-hosts setup.
    let frame = window.use_keyed_state(("hook-consent", agent.index()), cx, |_, _| {
        ActionFrame::default()
    });
    let toggle = frame.update(cx, |frame, _| frame.capture(!checked, available, window));
    let contents = div()
        .flex()
        .items_start()
        .gap(px(8.))
        .child(card::agent_mark(agent.harness(), cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(card::switch_copy(LABEL, "", cx))
                .child(card::help("hook-setup-consent", agent.consent(), cx))
                .child(card::help("hook-setup-removal", agent.removal(), cx)),
        );
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            Checkbox::new(agent.id(), LABEL, HoverHelp::tooltip(TOOLTIP), Role::Switch)
                .switch()
                .content(contents)
                .accessible_description(format!("{} {}", agent.consent(), agent.removal()))
                .checked(checked)
                .value_unknown(!available)
                .focus(&switch.focus)
                .on_toggle(move |_, window, cx| {
                    if let Some(&enabled) = toggle.shown() {
                        on_toggle(enabled, window, cx);
                    }
                }),
        )
        .children(configured::failure(
            agent.error_id(),
            "Could not save or read hook setup: ".into(),
            &switch.state,
            cx,
        ))
        .children(choices::busy_line(refused, agent.id(), cx))
}

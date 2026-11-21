//! One agent's all-hosts hook setup switch and the shared preference that restores its prompt and hint.

use gpui::{App, Div, FocusHandle, ParentElement as _, Styled as _, Window, div, px};

use super::{card, choices, configured};
use crate::components::{Checkbox, HoverHelp, Role};
use crate::hook_setup::HookAgent;

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
    cx: &App,
) -> Div {
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
                .child(card::help(agent.consent(), cx))
                .child(card::help(agent.removal(), cx)),
        );
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            Checkbox::new(agent.id(), LABEL, HoverHelp::tooltip(TOOLTIP), Role::Switch)
                .switch()
                .content(contents)
                .checked(switch.state.shown())
                .disabled(!switch.state.available())
                .focus(&switch.focus)
                .on_toggle(on_toggle),
        )
        .children(configured::failure(
            agent.error_id(),
            "Could not save or read hook setup: ".into(),
            &switch.state,
            cx,
        ))
        .children(choices::busy_line(refused, agent.id(), cx))
}

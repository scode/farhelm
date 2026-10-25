//! Managed-checkout trust controls whose mechanisms are command-line flags.
//!
//! Claude keeps its separate file-editing explanation and legacy endpoint.
//! These controls never answer the question currently on screen: their saved
//! choice applies when the helm next composes a managed-checkout launch.

use crate::api::{self, PreferenceValue};
use crate::{ApiBase, LaunchHarness, Session};
use dioxus::prelude::*;
use std::{cell::Cell, collections::HashSet, rc::Rc};

/// The three flag-based settings share save and dismissal behavior, but their
/// wording and observable question support remain explicit per agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TrustAgent {
    Codex,
    Muse,
    Pi,
}

impl TrustAgent {
    /// Stable endpoint and DOM suffix, independent of the displayed name.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Muse => "muse",
            Self::Pi => "pi",
        }
    }

    /// The vendor name used by both Settings and the discovery band.
    fn name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Muse => "Muse",
            Self::Pi => "Pi",
        }
    }

    /// Keep sparse preference writes agent-specific.
    pub(crate) fn dismissal(self, value: bool) -> PreferenceValue {
        match self {
            Self::Codex => PreferenceValue::CodexTrustHintDismissed(value),
            Self::Muse => PreferenceValue::MuseTrustHintDismissed(value),
            Self::Pi => PreferenceValue::PiTrustHintDismissed(value),
        }
    }

    /// Pi approves project resources rather than a recognized folder question.
    /// Its switch remains available without promising a discovery band.
    fn has_hint(self) -> bool {
        match self {
            Self::Codex | Self::Muse => true,
            Self::Pi => false,
        }
    }

    /// Labels describe the agent's actual trust mechanism rather than claiming
    /// that every agent asks the same folder-trust question.
    fn choice_label(self) -> &'static str {
        match self {
            Self::Codex => "answer Codex's folder-trust question for managed checkouts",
            Self::Muse => "answer Muse's workspace-trust question for managed checkouts",
            Self::Pi => "approve Pi project resources for managed checkouts",
        }
    }

    /// The command-line promise excludes vendor-file edits. Plain Restart and
    /// Replace keep the stored launch; only composing a launch reads this choice.
    fn help(self) -> &'static str {
        match self {
            Self::Codex => {
                "For folders Farhelm clones, including agent-requested checkouts, Farhelm passes a command-line trust override to Codex. Folders outside managed checkouts keep Codex's own trust behavior. Codex's saved configuration is unchanged."
            }
            Self::Muse => {
                "For folders Farhelm clones, including agent-requested checkouts, Farhelm passes Muse's trust option for that launch. Folders outside managed checkouts keep Muse's own trust behavior. Muse's saved configuration is unchanged."
            }
            Self::Pi => {
                "For folders Farhelm clones, including agent-requested checkouts, Farhelm passes Pi's option to approve project settings and extensions for that launch. Folders outside managed checkouts keep Pi's normal resource approval. Pi's saved configuration is unchanged."
            }
        }
    }

    /// Resolve only structured launches. Muse shares a generic integration kind,
    /// so neither the integration kind nor terminal text can identify its setting.
    #[warn(clippy::wildcard_enum_match_arm)]
    fn for_session(session: &Session) -> Option<Self> {
        match session.agent_selection()?.harness {
            LaunchHarness::Codex => Some(Self::Codex),
            LaunchHarness::Muse => Some(Self::Muse),
            LaunchHarness::Pi => Some(Self::Pi),
            LaunchHarness::Claude
            | LaunchHarness::Goose
            | LaunchHarness::Grok
            | LaunchHarness::Omp
            | LaunchHarness::Cursor
            | LaunchHarness::OpenCode => None,
        }
    }
}

/// Page-owned closes survive keyed views and transient read failures. An accepted
/// observation clears only an ended question, never navigation or a missing row.
/// Raw routing references identify occurrences; presentation spelling must not
/// merge or separate the page's remembered dismissals.
#[derive(Clone, Copy)]
pub(crate) struct OccurrenceDismissals(pub Signal<HashSet<(String, TrustAgent)>>);

impl OccurrenceDismissals {
    /// Reconcile only accepted detail observations, without subscribing the
    /// detail reader to local close clicks and causing redundant fetches.
    pub(crate) fn observe(mut self, session: &Session) {
        let agent = TrustAgent::for_session(session);
        let should_remove = self.0.peek().iter().any(|(id, closed_agent)| {
            id == session.id.raw()
                && (!session.workspace_trust_prompt || Some(*closed_agent) != agent)
        });
        if should_remove {
            self.0.write().retain(|(id, closed_agent)| {
                id != session.id.raw()
                    || (session.workspace_trust_prompt && Some(*closed_agent) == agent)
            });
        }
    }
}

/// Unknown or failed configuration reads must not advertise a safety-sensitive
/// opt-in as off. Only the recognized agent's own managed terminal qualifies.
fn show_hint(
    prompt: bool,
    managed: bool,
    agent_visible: bool,
    enabled: Option<bool>,
    dismissed: bool,
    occurrence_closed: bool,
) -> bool {
    prompt && managed && agent_visible && enabled == Some(false) && !dismissed && !occurrence_closed
}

/// Save one explicit opt-in with visible failures. The pending write survives
/// dialog dismissal, while its late completion cannot update an unmounted view.
///
/// Click errors stay in this scope. Each resource independently retires copied
/// pending dismissals, so a reopened view cannot retain a failed choice after
/// the original click waiter was destroyed. The preference writer continues.
#[component]
pub(crate) fn WorkspaceTrustSettings(agent: TrustAgent) -> Element {
    let base = use_context::<ApiBase>().0;
    let read_base = base.clone();
    let mut state = use_resource(move || {
        let base = read_base.clone();
        async move { api::fetch_agent_trust(&base, agent).await }
    });
    crate::claude_trust::use_hint_read_reconciliation(state);
    let mut saving = use_signal(|| false);
    let mut pending_choice = use_signal(|| None::<bool>);
    let mut error = use_signal(|| None::<String>);
    // Retain identity with the error so another entry point's newer choice
    // also withdraws a failure whose completion was already consumed.
    let mut hint_error = use_signal(|| None::<api::PreferenceWrite>);
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let on_drop = mounted.clone();
    use_drop(move || on_drop.set(false));
    let value = state
        .read()
        .as_ref()
        .and_then(|value| value.as_ref().ok())
        .cloned();
    let load_error = state
        .read()
        .as_ref()
        .and_then(|value| value.as_ref().err())
        .cloned();
    let enabled =
        pending_choice().unwrap_or_else(|| value.as_ref().is_some_and(|value| value.enabled));
    let dismissed = value.as_ref().is_some_and(|value| value.hint_dismissed);
    let locked = value.is_none() || saving();
    let preference_base = base.clone();
    let key = agent.key();
    let name = agent.name();
    // A fast refusal may leave the native box ahead of its virtual value.
    // Follow resource changes as well as errors so pending overlays and the
    // authoritative reread both reconcile the rendered property.
    use_effect(move || {
        let _ = state.read();
        let _ = hint_error();
        crate::settings::sync_preference_checks();
    });

    rsx! {
        div { id: "checkout-{key}-trust", class: "checkout-claude-trust", tabindex: "-1",
            label { class: "app-settings-choice", "data-tooltip": agent.help(),
                span { "{agent.choice_label()}" }
                input {
                    r#type: "checkbox", class: "host-settings-switch", checked: enabled, disabled: locked,
                    aria_describedby: "{key}-trust-help {key}-trust-risk",
                    onchange: move |event| {
                        let choice = event.checked();
                        let completion = api::save_agent_trust(&base, agent, choice);
                        pending_choice.set(Some(choice));
                        saving.set(true);
                        error.set(None);
                        let mounted = mounted.clone();
                        dioxus::core::spawn_forever(async move {
                            let result = completion.await;
                            if mounted.get() {
                                if result.is_ok() && let Some(Ok(value)) = state.write().as_mut() {
                                    value.enabled = choice;
                                }
                                // Native checkbox state changes before this event.
                                // On refusal, restore the acknowledged choice.
                                pending_choice.set(None);
                                saving.set(false);
                                error.set(result.err());
                                state.restart();
                            }
                        });
                    },
                }
            }
            p { id: "{key}-trust-help", class: "host-settings-help",
                "{agent.help()} New, Clone, Restart with and Replace with use this setting. Restart and Replace keep the session's saved launch."
            }
            p { id: "{key}-trust-risk", class: "host-settings-help",
                "As when you approve it yourself, repository hooks, settings and commands are then trusted. This is why the switch is off by default."
            }
            if agent.has_hint() {
                label { class: "app-settings-choice",
                    "data-tooltip": "show a note above the terminal when {name} asks about trust in a managed checkout",
                    span { "show the hint when {name} asks about a managed checkout" }
                    input { r#type: "checkbox", class: "host-settings-switch", checked: !dismissed, disabled: value.is_none(),
                        "data-preference-checked": (!dismissed).to_string(),
                        onchange: move |event| {
                            let dismissed = !event.checked();
                            let (write, completion) = api::observe_preference(
                                &preference_base,
                                agent.dismissal(dismissed),
                            );
                            // Keep native synchronization on the clicked value
                            // without waiting for a pending-overlay GET.
                            if let Some(Ok(value)) = state.write().as_mut() {
                                value.hint_dismissed = dismissed;
                                value.pending_dismissal = Some(write);
                            }
                            hint_error.set(None);
                            state.restart();
                            spawn(async move {
                                let saved = completion.await;
                                // Settings and the band share this field, so a
                                // component-local click counter is insufficient.
                                if !write.is_current() {
                                    return;
                                }
                                hint_error.set((!saved).then_some(write));
                            });
                        },
                    }
                }
                if hint_error().is_some_and(|write| write.is_current()) {
                    p { class: "app-settings-error", role: "alert",
                        "Could not save whether to show the {name} trust hint. Try again."
                    }
                }
            }
            if let Some(message) = error().or(load_error) {
                p { class: "app-settings-error", role: "alert", "Could not save or read {name} trust settings: {message}" }
            }
        }
    }
}

/// Select the band from structured launch identity; an unrecognized, legacy or
/// command launch cannot accidentally borrow another agent's opt-in or dismissal.
#[component]
pub(crate) fn WorkspaceTrustBand(session: ReadSignal<Session>, agent_visible: bool) -> Element {
    let agent = TrustAgent::for_session(&session.read()).filter(|agent| agent.has_hint());
    rsx! {
        if let Some(agent) = agent {
            AgentTrustBand { key: "{agent.key()}", session, agent_visible, agent }
        }
    }
}

/// Keep the nonmodal band mounted while configuration loads. Its reads follow
/// accepted session refreshes rather than adding subscriptions or polling.
#[component]
fn AgentTrustBand(session: ReadSignal<Session>, agent_visible: bool, agent: TrustAgent) -> Element {
    let base = use_context::<ApiBase>().0;
    let read_base = base.clone();
    let mut state = use_resource(move || {
        let _ = session.read();
        let base = read_base.clone();
        async move { api::fetch_agent_trust(&base, agent).await }
    });
    crate::claude_trust::use_hint_read_reconciliation(state);
    let mut occurrences = use_context::<OccurrenceDismissals>();
    let mut settings_open = use_signal(|| false);
    let settings_dialog = crate::modal_dialog::DialogHandle::new(crate::settings::DIALOG_SELECTOR);
    let session = session.read();
    let value = state
        .read()
        .as_ref()
        .and_then(|value| value.as_ref().ok())
        .cloned();
    let occurrence = (session.id.raw().to_owned(), agent);
    let visible = show_hint(
        session.workspace_trust_prompt,
        session.working_copy.is_some(),
        agent_visible,
        value.as_ref().map(|value| value.enabled),
        value.as_ref().is_some_and(|value| value.hint_dismissed),
        occurrences.0.read().contains(&occurrence),
    );
    let key = agent.key();
    let name = agent.name();
    rsx! {
        div { class: "workspace-trust-surface", style: "display: contents", "data-agent": key,
            "data-enabled": value.as_ref().map(|value| value.enabled.to_string()).unwrap_or_default(),
            "data-hint-dismissed": value.as_ref().map(|value| value.hint_dismissed.to_string()).unwrap_or_default(),
            if visible {
                div { class: "claude-trust-band workspace-trust-band", role: "status", aria_live: "polite",
                    div { class: "claude-trust-band-copy",
                        strong { "{name} asks about trust for this checkout. " }
                        "Farhelm can answer it for managed checkouts through a command-line flag. It's off by default because repository hooks, settings and commands are then trusted."
                    }
                    div { class: "claude-trust-band-actions",
                        button { r#type: "button", class: "btn btn-primary",
                            "data-tooltip": "open Settings at the {name} managed-checkout trust switch",
                            onclick: move |_| settings_open.set(true), "Settings" }
                        button { r#type: "button", class: "btn btn-neutral",
                            "data-tooltip": "stop showing this {name} note for every session; Settings can turn it back on",
                            onclick: move |_| {
                                let (write, _) = api::observe_preference(&base, agent.dismissal(true));
                                if let Some(Ok(value)) = state.write().as_mut() {
                                    value.hint_dismissed = true;
                                    value.pending_dismissal = Some(write);
                                }
                                state.restart();
                            }, "Don't show again" }
                        button { r#type: "button", class: "btn btn-neutral", aria_label: "hide this trust hint for this occurrence",
                            "data-tooltip": "hide this note for this question only",
                            onclick: move |_| { occurrences.0.write().insert(occurrence.clone()); }, "×" }
                    }
                }
            }
            if settings_open() {
                crate::settings::SettingsDialog {
                    focus_workspace_trust: Some(agent),
                    on_close: move |_| {
                        settings_open.set(false);
                        state.restart();
                        settings_dialog.close(crate::settings::RETURN_FOCUS);
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::show_hint;

    /// A failed read cannot imply consent is disabled, and discovery belongs
    /// only to a current question in the visible managed-checkout agent pane.
    #[farhelm_testtrace::test]
    fn hint_requires_all_current_question_gates() {
        assert!(show_hint(true, true, true, Some(false), false, false));
        assert!(!show_hint(false, true, true, Some(false), false, false));
        assert!(!show_hint(true, false, true, Some(false), false, false));
        assert!(!show_hint(true, true, false, Some(false), false, false));
        assert!(!show_hint(true, true, true, None, false, false));
        assert!(!show_hint(true, true, true, Some(true), false, false));
        assert!(!show_hint(true, true, true, Some(false), true, false));
        assert!(!show_hint(true, true, true, Some(false), false, true));
    }
}

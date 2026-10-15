//! Opt-in checkout trust and its nonmodal discovery hint.
//!
//! No terminal input is sent by this UI. The question remains Claude's to
//! answer until a future managed checkout uses the explicitly saved setting.

use crate::api::{self, PreferenceValue};
use crate::{ApiBase, Session};
use dioxus::prelude::*;
use std::{cell::Cell, rc::Rc};

/// Page-local occurrence dismissals survive keyed session views and temporary
/// stale surfaces. Only accepted observations of an ended question clear them;
/// navigation, a filtered list, and a failed read are not that evidence.
#[derive(Clone, Copy)]
pub(crate) struct OccurrenceDismissals(pub Signal<std::collections::HashSet<String>>);

impl OccurrenceDismissals {
    /// Apply a current session observation without subscribing its reader to
    /// dismissal changes (which would otherwise cause redundant fetches).
    pub(crate) fn observe(mut self, session: &Session) {
        if !session.claude_trust_prompt && self.0.peek().contains(&session.id) {
            self.0.write().remove(&session.id);
        }
    }
}

/// Display the hint only for a current question in a managed checkout's agent
/// terminal. Read failure is not evidence that the safety-sensitive opt-in is off.
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

/// Save an explicit opt-in and put its mechanism caveats beside the switch.
/// Like other configuration controls, saves outlive dismissal and failures
/// are visible. Mounted checks keep late completions out of a closed dialog.
#[component]
pub(crate) fn ClaudeTrustSettings() -> Element {
    let base = use_context::<ApiBase>().0;
    let read_base = base.clone();
    let mut state = use_resource(move || {
        let base = read_base.clone();
        async move { api::fetch_claude_trust(&base).await }
    });
    let mut saving = use_signal(|| false);
    let mut pending_choice = use_signal(|| None::<bool>);
    let mut error = use_signal(|| None::<String>);
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let on_drop = mounted.clone();
    use_drop(move || on_drop.set(false));
    let value = state
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned();
    let enabled =
        pending_choice().unwrap_or_else(|| value.as_ref().is_some_and(|value| value.enabled));
    let dismissed = value.as_ref().is_some_and(|value| value.hint_dismissed);
    let locked = value.is_none() || saving();
    let preference_base = base.clone();
    let load_error = state
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .cloned();

    rsx! {
        div { id: "checkout-claude-trust", class: "checkout-claude-trust", tabindex: "-1",
            label { class: "app-settings-choice",
                "data-tooltip": "mark each new managed checkout trusted in Claude's own settings file before the agent starts; see the caveats below",
                span { "answer Claude's \"trust this folder?\" for new checkouts" }
                input {
                    r#type: "checkbox", class: "host-settings-switch", checked: enabled, disabled: locked,
                    aria_describedby: "claude-trust-help claude-trust-caveats",
                    onchange: move |event| {
                        let choice = event.checked();
                        let completion = api::save_claude_trust(&base, choice);
                        pending_choice.set(Some(choice));
                        saving.set(true);
                        error.set(None);
                        let mounted = mounted.clone();
                        dioxus::core::spawn_forever(async move {
                            let result = completion.await;
                            if mounted.get() {
                                if result.is_ok() && let Some(Ok(value)) = state.write().as_mut() { value.enabled = choice; }
                                // Render the attempted choice while pending, then
                                // the saved value on refusal, including native DOM
                                // checkboxes whose state changed before the event.
                                pending_choice.set(None);
                                saving.set(false);
                                error.set(result.err());
                                state.restart();
                            }
                        });
                    },
                }
            }
            p { id: "claude-trust-help", class: "host-settings-help",
                "For folders Farhelm clones, including agent-requested checkouts, Farhelm marks the folder trusted before the agent starts. Folders you choose yourself still get Claude's question. On a remote host, the edit is made to that host's own Claude settings. Changes apply to future checkouts, whatever agent they start with."
            }
            div { id: "claude-trust-caveats", class: "claude-trust-caution",
                strong { "How this works, and why it's off by default" }
                p { "Claude has no supported way to do this. Farhelm edits Claude's own settings file directly (normally ~/.claude.json on the session's host), copying how Claude writes it. That file also holds sign-in, settings and per-folder state. Farhelm changes only the folder's trust key and replaces the file in one step, so it is never left half-written." }
                p { "A Claude session exiting at the same instant can rarely undo Farhelm's edit or lose one of its own updates. Any Claude update can change the file or its locking without notice: the edit may stop working, so Claude asks again, or collide with Claude's writes. Farhelm cannot detect every such change. This is best effort; Farhelm cannot promise it stays safe across Claude versions." }
                p { "As when you answer Yes yourself, the repository's Claude hooks and commands then run without Claude's warning. When Farhelm archives the checkout, it removes that folder's trust key again, also best effort." }
            }
            label { class: "app-settings-choice",
                "data-tooltip": "show a note above the terminal when Claude asks whether to trust a new managed checkout",
                span { "show the hint when Claude asks about a managed checkout" }
                input { r#type: "checkbox", class: "host-settings-switch", checked: !dismissed, disabled: value.is_none(),
                    onchange: move |event| {
                        api::store_preference(&preference_base, PreferenceValue::ClaudeTrustHintDismissed(!event.checked()));
                        state.restart();
                    },
                }
            }
            if let Some(message) = error().or(load_error) {
                p { class: "app-settings-error", role: "alert", "Could not save or read Claude trust settings: {message}" }
            }
        }
    }
}

/// Announce discoverability without covering the question or taking focus.
/// The page owns occurrence dismissal, so switching sessions or temporarily
/// withdrawing a stale terminal cannot revive the same unanswered question.
#[component]
pub(crate) fn ClaudeTrustBand(session: ReadSignal<Session>, agent_visible: bool) -> Element {
    let base = use_context::<ApiBase>().0;
    let read_base = base.clone();
    let mut state = use_resource(move || {
        // Every accepted detail read writes this signal, including the
        // existing feed-outage fallback. Share that refresh boundary instead
        // of adding another subscription or polling loop for the hint.
        let _ = session.read();
        let base = read_base.clone();
        async move { api::fetch_claude_trust(&base).await }
    });
    let mut occurrences = use_context::<OccurrenceDismissals>();
    let mut settings_open = use_signal(|| false);
    let session = session.read();
    let value = state
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned();
    let visible = show_hint(
        session.claude_trust_prompt,
        session.working_copy.is_some(),
        agent_visible,
        value.as_ref().map(|value| value.enabled),
        value.as_ref().is_some_and(|value| value.hint_dismissed),
        occurrences.0.read().contains(&session.id),
    );
    let id = session.id.clone();
    rsx! {
        // A mounted band may be hidden either by a saved choice or by an
        // unanswered settings read. Keep that distinction observable without
        // adding a box to the terminal's flow or changing focus.
        div { class: "claude-trust-surface", style: "display: contents",
            "data-enabled": value.as_ref().map(|value| value.enabled.to_string()).unwrap_or_default(),
            "data-hint-dismissed": value.as_ref().map(|value| value.hint_dismissed.to_string()).unwrap_or_default(),
        if visible {
            div { class: "claude-trust-band", role: "status", aria_live: "polite",
                div { class: "claude-trust-band-copy",
                    strong { "Claude asks this for every new checkout. " }
                    "Farhelm can answer it for you, but it's off by default: there's no supported way to do it, so Farhelm would have to edit Claude's own settings file, which comes with caveats."
                }
                div { class: "claude-trust-band-actions",
                    button { r#type: "button", class: "btn btn-primary",
                        "data-tooltip": "open Settings at the option to answer this question for new checkouts, with its caveats",
                        onclick: move |_| settings_open.set(true), "Settings and caveats" }
                    button { r#type: "button", class: "btn btn-neutral",
                        "data-tooltip": "stop showing this note for every session; Settings can turn it back on",
                        onclick: move |_| {
                        api::store_preference(&base, PreferenceValue::ClaudeTrustHintDismissed(true));
                        if let Some(Ok(value)) = state.write().as_mut() { value.hint_dismissed = true; }
                        state.restart();
                    }, "Don't show again" }
                    button { r#type: "button", class: "btn btn-neutral", aria_label: "hide this trust hint for this occurrence",
                        "data-tooltip": "hide this note for this question only",
                        onclick: move |_| { occurrences.0.write().insert(id.clone()); }, "×" }
                }
            }
        }
        if settings_open() {
            crate::settings::SettingsDialog {
                focus_claude_trust: true,
                on_close: move |_| {
                    settings_open.set(false);
                    state.restart();
                    crate::settings::return_focus_to_gear();
                },
            }
        }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::show_hint;

    /// Discovery must never advertise trust for a user-selected folder or
    /// hide Claude behind a modal. Each gate independently withdraws the band.
    #[test]
    fn hint_requires_current_managed_question_and_saved_opt_outs() {
        assert!(show_hint(true, true, true, Some(false), false, false));
        assert!(!show_hint(false, true, true, Some(false), false, false));
        assert!(!show_hint(true, false, true, Some(false), false, false));
        assert!(!show_hint(true, true, false, Some(false), false, false));
        assert!(!show_hint(true, true, true, Some(true), false, false));
        assert!(!show_hint(true, true, true, None, false, false));
        assert!(!show_hint(true, true, true, Some(false), true, false));
        assert!(!show_hint(true, true, true, Some(false), false, true));
    }
}

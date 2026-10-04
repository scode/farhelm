//! Helm-wide choices that have no natural home in the session or host rows.
//!
//! These are the same preferences the host dialogs write when the user answers
//! permanently. Update the shared client copy before queuing the sparse write:
//! the next add or removal in this window must see the answer immediately.
//! Other clients keep their loaded preferences until they reload, as usual.

use dioxus::prelude::*;

use crate::api::{PreferenceValue, store_preference};
use crate::hosts::settings_dialog::install_dialog_with_selector;
use crate::list::SharedPreferences;
use crate::{ApiBase, modal_isolation};

const DIALOG_SELECTOR: &str = r#".app-settings-dialog[role="dialog"]"#;

/// Undo modal isolation before focusing the gear, which is inert while open.
/// The frame lets the closing render remove the dialog before focus returns.
pub(crate) fn return_focus_to_gear() {
    document::eval(&format!(
        "{} requestAnimationFrame(() => document.querySelector('.app-settings-toggle')?.focus({{ preventScroll: true }}))",
        modal_isolation::release_js(DIALOG_SELECTOR),
    ));
}

/// Expose the two permanent host answers without introducing another save step.
///
/// Writes use the ordinary preference queue and its silent failure behavior.
/// Controls stay enabled: preferences have no host-operation lock or outcome
/// surface, and closing the dialog must not cancel a queued preference write.
#[component]
pub(crate) fn SettingsDialog(on_close: EventHandler<()>) -> Element {
    let base = use_context::<ApiBase>().0;
    let setup_base = base.clone();
    let mut preferences = use_context::<SharedPreferences>();
    let setup = preferences.0.read().skip_host_setup_confirmation == Some(true);
    let remove = preferences.0.read().skip_host_remove_confirmation == Some(true);

    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            div {
                class: "app-settings-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "settings",
                tabindex: "-1",
                onmounted: move |_| install_dialog_with_selector(
                    DIALOG_SELECTOR, "input", Some(".app-settings-close"),
                ),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape && !event.is_composing() {
                        on_close.call(());
                    }
                },
                h2 { class: "host-settings-title", "settings" }
                p { class: "host-settings-help",
                    "These choices apply to every host on this helm. Other open clients pick them up when they reload."
                }
                div {
                    label { class: "app-settings-choice",
                        "data-tooltip": "set up a new host as soon as it has been checked, without showing the plan first",
                        input {
                            r#type: "checkbox",
                            checked: setup,
                            aria_describedby: "host-setup-preference-help",
                            onchange: move |event| {
                                preferences.0.write().skip_host_setup_confirmation = Some(event.checked());
                                store_preference(&setup_base, PreferenceValue::HostSetupConfirmation(event.checked()));
                            },
                        }
                        "set up new hosts without asking"
                    }
                    p { id: "host-setup-preference-help", class: "host-settings-help",
                        if setup {
                            "Farhelm sets up a new host as soon as it has checked it, without asking."
                        } else {
                            "Farhelm shows what setup will change on a new host and asks before doing it."
                        }
                    }
                }
                div {
                    label { class: "app-settings-choice",
                        "data-tooltip": "forget a host as soon as you choose remove, without asking; its sessions keep running",
                        input {
                            r#type: "checkbox",
                            checked: remove,
                            aria_describedby: "host-remove-preference-help",
                            onchange: move |event| {
                                preferences.0.write().skip_host_remove_confirmation = Some(event.checked());
                                store_preference(&base, PreferenceValue::HostRemoveConfirmation(event.checked()));
                            },
                        }
                        "remove hosts without asking"
                    }
                    p { id: "host-remove-preference-help", class: "host-settings-help",
                        if remove {
                            "Farhelm forgets a host as soon as you choose remove. Its sessions keep running."
                        } else {
                            "Farhelm asks before forgetting a host. Its sessions keep running."
                        }
                    }
                }
                div { class: "host-settings-actions",
                    button {
                        r#type: "button",
                        class: "btn btn-neutral app-settings-close",
                        "data-tooltip": "close: changes are already saved",
                        onclick: move |_| on_close.call(()),
                        "close"
                    }
                }
            }
        }
    }
}

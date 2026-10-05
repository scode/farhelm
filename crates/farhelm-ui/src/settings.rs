//! Helm-wide choices that have no natural home in the session or host rows.
//!
//! These are the same preferences the host dialogs write when the user answers
//! permanently. Update the shared client copy before queuing the sparse write:
//! the next add or removal in this window must see the answer immediately.
//! Other clients keep their loaded preferences until they reload, as usual.
//!
//! A desktop app whose updater runs adds one choice that is not a helm
//! preference: whether to install updates automatically. It belongs to that
//! app installation (SPEC.md "Session list"), so it is read from and written
//! to the app's own state file through the updater, never the helm.

use dioxus::prelude::*;

use crate::api::{PreferenceValue, store_preference};
use crate::app_updater::use_app_updater;
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

/// Put the automatic-updates checkbox back to the setting in force after a
/// failed save.
///
/// The browser has already flipped the box when `onchange` runs, and when
/// the setting in force equals what the dialog last rendered, the signal
/// does not change and no render rewrites `checked`; the box would keep
/// showing the choice that was not saved. So the property is set directly.
fn restore_automatic_checkbox(in_force: bool) {
    document::eval(&format!(
        "(() => {{ const box = document.querySelector('{DIALOG_SELECTOR} input[aria-describedby=\"automatic-updates-help\"]'); if (box) box.checked = {in_force}; }})();"
    ));
}

/// Expose the two permanent host answers, and in a desktop app whose updater
/// runs the automatic-updates choice, without introducing another save step.
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
    let updater = use_app_updater();
    // Read once when the dialog opens and then kept locally: the file is
    // this app's own, so nothing else changes it while the dialog is up.
    let mut automatic = use_signal(|| {
        updater
            .as_ref()
            .is_some_and(|updater| updater.automatic_updates())
    });
    // Why the last change of that choice was not saved, until the next try.
    let mut automatic_error = use_signal(|| None::<String>);

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
                if let Some(updater) = updater {
                    p { class: "host-settings-help",
                        "This one belongs to this Farhelm app on this Mac."
                    }
                    div {
                        label { class: "app-settings-choice",
                            "data-tooltip": "install updates automatically: check for a new Farhelm release when the app starts and about once a day, and install it in the background",
                            input {
                                r#type: "checkbox",
                                checked: automatic(),
                                aria_describedby: "automatic-updates-help",
                                // Only a saved choice is shown as the choice: on
                                // a failed write the box returns to the setting
                                // still in force, with the reason beside it.
                                onchange: move |event| {
                                    match updater.set_automatic_updates(event.checked()) {
                                        Ok(()) => {
                                            automatic.set(event.checked());
                                            automatic_error.set(None);
                                        }
                                        Err(reason) => {
                                            let in_force = updater.automatic_updates();
                                            automatic.set(in_force);
                                            automatic_error.set(Some(reason));
                                            restore_automatic_checkbox(in_force);
                                        }
                                    }
                                },
                            }
                            "install updates automatically"
                        }
                        p { id: "automatic-updates-help", class: "host-settings-help",
                            if automatic() {
                                "Farhelm checks for a new release when it starts and about once a day, and installs it in the background. Restarting Farhelm finishes the update."
                            } else {
                                "Farhelm installs a new release only when you check for updates from the ? menu or this machine's update item, or run the installer yourself."
                            }
                        }
                        if let Some(reason) = automatic_error() {
                            p { class: "host-settings-help app-settings-error", role: "alert", "{reason}" }
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

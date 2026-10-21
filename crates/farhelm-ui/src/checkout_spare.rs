//! A saved future-create choice, independent of path previews and agent trust.
//!
//! The read must succeed before the switch becomes editable. Saves outlive this
//! component, and the shared API fence orders a reopened read after them.

use crate::{ApiBase, api};
use dioxus::prelude::*;
use std::{cell::Cell, rc::Rc};

/// Explain the earlier-clone tradeoff beside the explicit all-hosts switch.
/// A failed save restores the known persisted choice, including the native
/// checkbox state. Unmounted completions finish their write without touching UI.
#[component]
pub(crate) fn CheckoutSpareSettings() -> Element {
    let base = use_context::<ApiBase>().0;
    let read_base = base.clone();
    let mut state = use_resource(move || {
        let base = read_base.clone();
        async move { api::fetch_checkout_spare(&base).await }
    });
    let mut saving = use_signal(|| false);
    let mut pending_choice = use_signal(|| None::<bool>);
    let mut error = use_signal(|| None::<String>);
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let on_drop = mounted.clone();
    use_drop(move || on_drop.set(false));
    let saved = state
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|value| value.enabled);
    let enabled = pending_choice().or(saved).unwrap_or(false);
    let locked = saved.is_none() || saving();
    let load_error = state
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .cloned();

    rsx! {
        div { id: "checkout-spare", class: "checkout-spare-setting",
            label { class: "app-settings-choice",
                "data-tooltip": "prepare one extra checkout per repository and host so the next launch can update it instead of cloning again",
                span { "keep a checkout ready for faster launches" }
                input { r#type: "checkbox", class: "host-settings-switch", checked: enabled, disabled: locked,
                    aria_describedby: "checkout-spare-help",
                    onchange: move |event| {
                        let choice = event.checked();
                        // Register the ordering ticket before spawning, so a
                        // remounted read cannot overtake this explicit choice.
                        let completion = api::save_checkout_spare(&base, choice);
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
                                pending_choice.set(None);
                                saving.set(false);
                                error.set(result.err());
                                state.restart();
                            }
                        });
                    },
                }
            }
            p { id: "checkout-spare-help", class: "host-settings-help",
                "On by default. Keeps one extra checkout per repository and host, using extra disk space. Farhelm prepares it through your login shell without prompting for credentials. The next launch updates that earlier clone: unchanged files keep older modification times, and Git hooks and checkout filters run ahead of time, then again only for the update. Turning this off affects future creates; each repository's spare is discarded at its next create on that host, or after 30 days unused. "
                a { href: "https://farhelm.io/docs/using/start-a-session/#prepared-checkouts",
                    target: "_blank", rel: "noopener",
                    "data-tooltip": "read how prepared checkouts differ from a fresh clone",
                    onclick: move |event| {
                        event.prevent_default();
                        crate::app_bar::open_external_page("https://farhelm.io/docs/using/start-a-session/#prepared-checkouts");
                    },
                    "How prepared checkouts differ from a fresh clone"
                }
            }
            if let Some(message) = error().or(load_error) {
                p { class: "app-settings-error", role: "alert", "Could not save or read prepared checkout settings: {message}" }
            }
        }
    }
}

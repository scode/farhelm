//! Explicitly saved checkout folders for Settings, the launcher, and host settings.
//! A host's override and the inherited default stay separate: displaying the
//! default never copies it into host configuration. The caller owns any reads
//! that successful setup should refresh or enable afterwards.

use crate::api::{fetch_checkout_root, save_checkout_root};
use crate::{ApiBase, HostId};
use dioxus::prelude::*;
use std::{cell::Cell, rc::Rc};

/// Keep refused drafts editable and distinguish a typed value from a saved one.
/// Save writes only an edited draft, trimmed: the draft is read once, so an
/// untouched Save would put back that old value over another writer's change
/// while the field was open (SPEC.md, "One GUI at a time"). An empty result
/// clears this scope; a host then inherits the default at its next launch.
/// Use default is an explicit clear even when the draft has not been edited.
///
/// Loading or saving cannot submit a surrounding launcher form. The field's
/// own save disables semantically so it keeps keyboard focus in the modal.
/// A surrounding host edit instead disables natively, allowing that dialog's
/// existing scroll-region recovery when all its controls become unavailable.
/// A submitted save survives dismissal; only a still-mounted field
/// receives its outcome or callback. Callers must remount when `host` changes,
/// because a draft and its initial snapshot belong to one configuration scope.
#[component]
pub(crate) fn CheckoutRootField(
    note: String,
    on_saved: EventHandler<()>,
    /// None edits the global default; Some edits only that host's override.
    #[props(default)]
    host: Option<HostId>,
    /// Honor a surrounding dialog's existing write/edit gate without sharing drafts.
    #[props(default)]
    disabled: bool,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let read_base = base.clone();
    let mut root = use_resource(move || {
        let base = read_base.clone();
        async move { fetch_checkout_root(&base, host).await }
    });
    let mut initialized = use_signal(|| false);
    let mut draft = use_signal(String::new);
    let mut edited = use_signal(|| false);
    let mut has_override = use_signal(|| false);
    let mut saving = use_signal(|| false);
    let mut outcome = use_signal(|| None::<Result<(), String>>);
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let drop_mounted = mounted.clone();
    use_drop(move || drop_mounted.set(false));
    use_effect(move || {
        if !initialized()
            && let Some(Ok(value)) = root.read().as_ref()
        {
            draft.set(value.root.clone().unwrap_or_default());
            has_override.set(value.root.is_some());
            initialized.set(true);
        }
    });
    let load_error = root
        .read()
        .as_ref()
        .and_then(|value| value.as_ref().err())
        .cloned();
    let locked = !initialized() || saving() || disabled;
    let label = if host.is_some() {
        "Checkout folder"
    } else {
        "Default checkout folder"
    };
    let placeholder = if host.is_some() {
        root.read()
            .as_ref()
            .and_then(|value| value.as_ref().ok())
            .and_then(|value| value.default.as_deref())
            .map_or_else(
                || "No default set".to_owned(),
                |value| format!("Default: {value}"),
            )
    } else {
        String::new()
    };
    let save_tooltip = if host.is_some() {
        "Save the checkout folder for this host"
    } else {
        "Save the default checkout folder for hosts without their own"
    };
    let retry_tooltip = if host.is_some() {
        "Retry loading this host's checkout folder and the default"
    } else {
        "Retry loading the default checkout folder"
    };

    // Both explicit actions declare their save before spawning it. A reopened
    // dialog must read after this write even if this field is dismissed before
    // the task is first polled. A refused clear must preserve a different draft.
    let submit = EventHandler::new(move |use_default: bool| {
        if !initialized() || saving() || disabled {
            return;
        }
        if use_default {
            if host.is_none() || !has_override() {
                return;
            }
        } else if !edited() {
            return;
        }
        saving.set(true);
        outcome.set(None);
        let value = if use_default {
            String::new()
        } else {
            let value = draft().trim().to_string();
            draft.set(value.clone());
            value
        };
        let value = (!value.is_empty()).then_some(value);
        let stored_override = value.is_some();
        let saving_request = save_checkout_root(&base, host, value);
        let mounted = mounted.clone();
        // Closing a modal retires its handles, not the user's explicit save.
        dioxus::core::spawn_forever(async move {
            let result = saving_request.await;
            if !mounted.get() {
                return;
            }
            saving.set(false);
            let saved = result.is_ok();
            outcome.set(Some(result));
            if saved {
                if use_default {
                    draft.set(String::new());
                }
                has_override.set(stored_override);
                edited.set(false);
                root.restart();
                on_saved.call(());
            }
        });
    });

    rsx! {
        div { class: "checkout-root-field",
            label { class: "checkout-root-label",
                span { "{label}" }
                input {
                    r#type: "text", value: "{draft}", placeholder, readonly: locked, disabled,
                    aria_disabled: locked, autocomplete: "off", spellcheck: "false",
                    // A path's spelling and case identify the target folder.
                    autocorrect: "off", autocapitalize: "none",
                    oninput: move |event| {
                        if initialized() && !saving() && !disabled { draft.set(event.value()); edited.set(true); outcome.set(None); }
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        // Configuration Save and launcher submit are separate acts.
                        if event.key() == Key::Enter {
                            event.prevent_default(); event.stop_propagation();
                        }
                    },
                }
            }
            p { class: "launch-composer-repository-note", "{note}" }
            if let Some(error) = load_error {
                p { class: "checkout-root-error", role: "alert", "{error}" }
                button {
                    r#type: "button", class: "btn btn-neutral", "data-tooltip": retry_tooltip,
                    disabled, aria_disabled: disabled || saving(),
                    onclick: move |_| { if !disabled && !saving() { root.restart(); } }, "Retry"
                }
            } else {
                button {
                    r#type: "button", class: "btn btn-primary", disabled, aria_disabled: locked || !edited(),
                    "data-tooltip": save_tooltip,
                    onclick: move |_| submit.call(false),
                    if saving() { "Saving…" } else { "Save" }
                }
                if host.is_some() && has_override() {
                    button {
                        r#type: "button", class: "btn btn-neutral", disabled, aria_disabled: locked,
                        "data-tooltip": "Use the default checkout folder from Settings → Checkouts",
                        onclick: move |_| submit.call(true), "Use default"
                    }
                }
            }
            if let Some(result) = outcome() {
                match result {
                    Ok(()) => rsx! { p { class: "checkout-root-success", role: "status", "Checkout folder saved" } },
                    Err(error) => rsx! { p { class: "checkout-root-error", role: "alert", "{error}" } },
                }
            }
        }
    }
}

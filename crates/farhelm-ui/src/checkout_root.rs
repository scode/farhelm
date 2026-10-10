//! One explicitly saved all-hosts folder field, shared by Settings and setup
//! in the session launcher. It owns only its draft and visible write outcome;
//! the caller owns what successful setup should refresh or enable afterwards.

use crate::ApiBase;
use crate::api::{fetch_checkout_root, save_checkout_root};
use dioxus::prelude::*;
use std::{cell::Cell, rc::Rc};

/// Keep refused drafts editable and distinguish a typed value from a saved one.
/// Saving trims surrounding whitespace, and an empty result clears the global
/// folder: a pasted path's stray trailing space is invisible in the preview,
/// and first-use creation would otherwise make a folder whose name ends in it. Loading or saving cannot submit a
/// surrounding launcher form, and disabling is semantic rather than native so
/// a write does not drop keyboard focus into the page behind a modal.
/// A submitted save survives dismissal; only a still-mounted field receives
/// its outcome or callback, since those handles belong to the component.
#[component]
pub(crate) fn CheckoutRootField(note: String, on_saved: EventHandler<()>) -> Element {
    let base = use_context::<ApiBase>().0;
    let read_base = base.clone();
    let mut root = use_resource(move || {
        let base = read_base.clone();
        async move { fetch_checkout_root(&base).await }
    });
    let mut initialized = use_signal(|| false);
    let mut draft = use_signal(String::new);
    let mut saving = use_signal(|| false);
    let mut outcome = use_signal(|| None::<Result<(), String>>);
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let drop_mounted = mounted.clone();
    use_drop(move || drop_mounted.set(false));
    use_effect(move || {
        if !initialized()
            && let Some(Ok(value)) = root.read().as_ref()
        {
            draft.set(value.clone().unwrap_or_default());
            initialized.set(true);
        }
    });
    let load_error = root
        .read()
        .as_ref()
        .and_then(|value| value.as_ref().err())
        .cloned();
    let locked = !initialized() || saving();

    rsx! {
        div { class: "checkout-root-field",
            label { class: "checkout-root-label",
                span { "checkout folder" }
                input {
                    r#type: "text", value: "{draft}", readonly: locked,
                    aria_disabled: locked, autocomplete: "off", spellcheck: "false",
                    oninput: move |event| {
                        if initialized() && !saving() { draft.set(event.value()); outcome.set(None); }
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        // Enter must not fall through to the launcher's form:
                        // saving configuration and launching are separate acts.
                        if event.key() == Key::Enter {
                            event.prevent_default(); event.stop_propagation();
                        }
                    },
                }
            }
            p { class: "launch-composer-repository-note", "{note}" }
            if let Some(error) = load_error {
                p { class: "checkout-root-error", role: "alert", "{error}" }
                button { r#type: "button", onclick: move |_| root.restart(), "retry" }
            } else {
                button {
                    r#type: "button", aria_disabled: locked,
                    onclick: move |_| {
                        if !initialized() || saving() { return; }
                        saving.set(true);
                        outcome.set(None);
                        let base = base.clone();
                        let value = draft().trim().to_string();
                        draft.set(value.clone());
                        let mounted = mounted.clone();
                        // Closing a modal must not retract an explicit Save.
                        // The request is app-owned; all local handles remain
                        // component-owned and must be checked after the await.
                        dioxus::core::spawn_forever(async move {
                            let result = save_checkout_root(&base,
                                if value.is_empty() { None } else { Some(value) }).await;
                            if !mounted.get() { return; }
                            saving.set(false);
                            let saved = result.is_ok();
                            outcome.set(Some(result));
                            if saved {
                                root.restart();
                                on_saved.call(());
                            }
                        });
                    },
                    if saving() { "saving…" } else { "save" }
                }
            }
            if let Some(result) = outcome() {
                match result {
                    Ok(()) => rsx! { p { class: "checkout-root-success", role: "status", "checkout folder saved" } },
                    Err(error) => rsx! { p { class: "checkout-root-error", role: "alert", "{error}" } },
                }
            }
        }
    }
}

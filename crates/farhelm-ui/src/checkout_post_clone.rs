//! The explicitly saved command for future managed checkouts on every host.
//! Its draft is local to Settings; the helm owns configuration and accepted
//! preparations keep the command that was frozen when they were accepted.

use crate::ApiBase;
use crate::api::{fetch_checkout_post_clone, save_checkout_post_clone};
use dioxus::prelude::*;
use std::{cell::Cell, rc::Rc};

/// Preserve executable text until the user explicitly saves it.
/// Only an all-whitespace draft clears the setting; otherwise whitespace and
/// newlines are part of the command. Reading an explicit disable displays an
/// empty box without rewriting it. Saves survive dismissal, while completion
/// touches component-owned handles only if this field is still mounted.
/// A stored carriage return makes nonblank saves unsafe: textarea input can
/// normalize it before the draft reaches us. Clearing remains available, and
/// commands requiring that character can still be changed through the CLI.
#[component]
pub(crate) fn CheckoutPostCloneField() -> Element {
    let base = use_context::<ApiBase>().0;
    let read_base = base.clone();
    let mut command = use_resource(move || {
        let base = read_base.clone();
        async move { fetch_checkout_post_clone(&base).await }
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
            && let Some(Ok(value)) = command.read().as_ref()
        {
            draft.set(value.clone().unwrap_or_default());
            initialized.set(true);
        }
    });
    let load_error = command
        .read()
        .as_ref()
        .and_then(|value| value.as_ref().err())
        .cloned();
    let locked = !initialized() || saving();
    let stored_carriage_return = command
        .read()
        .as_ref()
        .and_then(|value| value.as_ref().ok())
        .and_then(|value| value.as_deref())
        .is_some_and(|value| value.contains('\r'));

    rsx! {
        div { class: "checkout-post-clone-field",
            label { class: "checkout-root-label",
                span { "post-clone command" }
                textarea {
                    rows: "4", value: "{draft}", readonly: locked,
                    aria_disabled: locked,
                    // Commands are executable text: browser corrections and
                    // remembered prose must not change what the user writes.
                    autocomplete: "off", autocorrect: "off",
                    autocapitalize: "none", spellcheck: "false",
                    oninput: move |event| {
                        if initialized() && !saving() {
                            draft.set(event.value());
                            outcome.set(None);
                        }
                    },
                }
            }
            p { class: "launch-composer-repository-note",
                "Runs in each new managed checkout, on its host, after cloning and before the agent starts. Changes affect future checkouts only. A host given its own command from the command line keeps it. Save an empty field to remove the all-hosts command."
            }
            if let Some(error) = load_error {
                p { class: "checkout-root-error", role: "alert", "{error}" }
                button {
                    r#type: "button",
                    "data-tooltip": "retry loading the all-hosts post-clone command",
                    onclick: move |_| command.restart(), "retry"
                }
            } else {
                button {
                    r#type: "button", aria_disabled: locked,
                    "data-tooltip": "save the post-clone command for future managed checkouts",
                    onclick: move |_| {
                        if !initialized() || saving() { return; }
                        let value = draft();
                        // Judge the loaded command, not the browser's normalized
                        // draft; after a successful clear, the refetch lifts this refusal.
                        if stored_carriage_return && !value.trim().is_empty() {
                            outcome.set(Some(Err("the saved command contains a carriage return this field would turn into a newline; change it with `farhelm helm checkout-config set-post-clone` on the helm's machine, or save an empty field to clear it".into())));
                            return;
                        }
                        saving.set(true);
                        outcome.set(None);
                        let base = base.clone();
                        let value = if value.trim().is_empty() {
                            // A blank save clears the command; show it cleared
                            // rather than leaving whitespace beside "saved".
                            draft.set(String::new());
                            None
                        } else {
                            Some(value)
                        };
                        let mounted = mounted.clone();
                        // Declare the save in the click handler so a reopened
                        // Settings read sees it even before this task is polled.
                        let saving_request = save_checkout_post_clone(&base, value);
                        // Explicit Save belongs to the app's request lifetime;
                        // dismissing Settings only retires its local view.
                        dioxus::core::spawn_forever(async move {
                            let result = saving_request.await;
                            if !mounted.get() { return; }
                            saving.set(false);
                            let saved = result.is_ok();
                            outcome.set(Some(result));
                            if saved { command.restart(); }
                        });
                    },
                    if saving() { "saving…" } else { "save" }
                }
            }
            if let Some(result) = outcome() {
                match result {
                    Ok(()) => rsx! { p { class: "checkout-root-success", role: "status", "post-clone command saved" } },
                    Err(error) => rsx! { p { class: "checkout-root-error", role: "alert", "{error}" } },
                }
            }
        }
    }
}

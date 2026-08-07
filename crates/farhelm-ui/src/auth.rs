//! The browser's one-time device authentication surface.
//!
//! A same-build 401 carrying the authentication middleware's marker raises
//! this page-wide prompt. It is an ordinary in-page form (never a browser
//! dialog), exchanges the bootstrap token for an origin-scoped device secret,
//! and then unmounts itself so the active surface and feed mount afresh and
//! re-read through the authenticated session.

use crate::{ApiBase, api};
use dioxus::prelude::*;

/// localStorage key for the device secret returned by the helm.
///
/// localStorage is scoped to the complete origin, including its port. That is
/// the security property a host-scoped cookie cannot provide when unrelated
/// loopback services share `127.0.0.1`.
#[cfg(target_arch = "wasm32")]
pub(crate) const DEVICE_SECRET_KEY: &str = "farhelm.device-secret";

/// Read the device secret attached to every protected browser request.
#[cfg(target_arch = "wasm32")]
pub(crate) fn device_secret() -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()
        .flatten()?
        .get_item(DEVICE_SECRET_KEY)
        .ok()
        .flatten()
}

/// Native bootstrap arrives with the embedded-app work; this item only builds
/// the browser's origin-scoped storage path.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn device_secret() -> Option<String> {
    None
}

/// Persist one exchanged device secret in the browser origin that received
/// it. Failure stays visible on the token form rather than pretending a
/// credential the next request cannot read was installed successfully.
#[cfg(target_arch = "wasm32")]
fn store_device_secret(secret: &str) -> Result<(), String> {
    let storage = web_sys::window()
        .ok_or_else(|| "browser storage is unavailable".to_string())?
        .local_storage()
        .map_err(|_| "browser storage is unavailable".to_string())?
        .ok_or_else(|| "browser storage is unavailable".to_string())?;
    storage
        .set_item(DEVICE_SECRET_KEY, secret)
        .map_err(|_| "the browser refused to persist this device session".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn store_device_secret(_secret: &str) -> Result<(), String> {
    Err("device authentication is not available in this renderer yet".to_string())
}

/// Whether the next render must replace the application with the token form.
pub(crate) static TOKEN_REQUIRED: GlobalSignal<bool> = Signal::global(|| false);

/// Raise the token surface once. Repeated 401s do not dirty the signal again.
pub(crate) fn require_token() {
    if !*TOKEN_REQUIRED.peek() {
        *TOKEN_REQUIRED.write() = true;
    }
}

/// Exchange the user's pasted bootstrap token and remount the authenticated
/// application on success.
#[component]
pub(crate) fn TokenPrompt() -> Element {
    let base = use_context::<ApiBase>();
    let mut token = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    rsx! {
        main { class: "auth-page",
            form {
                class: "auth-card",
                onsubmit: move |event| {
                    event.prevent_default();
                    if *busy.peek() {
                        return;
                    }
                    let submitted = token.peek().trim().to_string();
                    if submitted.is_empty() {
                        error.set(Some("enter the token printed by `farhelm helm token show`".to_string()));
                        return;
                    }
                    busy.set(true);
                    error.set(None);
                    let base = base.0.clone();
                    spawn(async move {
                        match api::exchange_token(&base, &submitted).await {
                            Ok(device_secret) => {
                                if let Err(detail) = store_device_secret(&device_secret) {
                                    error.set(Some(detail));
                                    busy.set(false);
                                    return;
                                }
                                // The active surface and feed were unmounted
                                // while this form occupied the page. Clearing
                                // the gate mounts them from scratch, preserving
                                // list or open-session navigation while forcing
                                // the authenticated re-read the exchange owes.
                                *TOKEN_REQUIRED.write() = false;
                            }
                            Err(detail) => {
                                error.set(Some(detail));
                                busy.set(false);
                            }
                        }
                    });
                },
                h1 { "Authenticate this device" }
                p {
                    "Run "
                    code { "farhelm helm token show" }
                    " on the helm's machine, then paste the token here."
                }
                label { r#for: "farhelm-token", "Web token" }
                input {
                    id: "farhelm-token",
                    class: "auth-token-input",
                    r#type: "password",
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    autofocus: true,
                    value: "{token}",
                    disabled: *busy.read(),
                    oninput: move |event| token.set(event.value()),
                }
                button {
                    class: "btn auth-submit",
                    r#type: "submit",
                    disabled: *busy.read(),
                    if *busy.read() { "Checking…" } else { "Continue" }
                }
                if let Some(detail) = error.read().as_ref() {
                    p { class: "auth-error", role: "alert", "{detail}" }
                }
            }
        }
    }
}

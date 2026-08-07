//! Device authentication surfaces for the browser and native desktop app.
//!
//! The browser raises an ordinary in-page token prompt after a recognized
//! 401. Desktop bootstrap instead performs two exchanges: native REST owns a
//! process-scoped credential, while the webview receives a separate one over
//! IPC for localStorage and WebSocket subprotocols. Every successful exchange
//! remounts the active surface and feed so no reader retains revoked state.

use crate::{ApiBase, api};
use dioxus::prelude::*;

#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
#[derive(Debug, serde::Deserialize)]
struct DesktopExchange {
    secret: Option<String>,
    error: Option<String>,
    #[serde(default)]
    retry_token: bool,
    #[serde(default)]
    ready: bool,
}

/// Incremented when native REST observes a revoked desktop credential. The
/// gate reacts by cancelling its completed authentication future and running
/// a new validation/exchange inside the webview context, replacing that
/// client's independently revoked WebSocket credential too.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) static DESKTOP_AUTH_GENERATION: GlobalSignal<u64> = Signal::global(|| 0);

#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) fn require_desktop_webview_reauth() {
    *DESKTOP_AUTH_GENERATION.write() += 1;
}

/// Hold the component tree behind the webview's own authenticated exchange.
///
/// Native bootstrap has already authenticated reqwest. This second exchange
/// runs inside the webview so its localStorage and WebSocket subprotocol carry
/// a separately minted device credential. Values enter JavaScript through the
/// eval IPC channel, never through a URL or rendered DOM. A validation 401 is
/// the only state that mints: transport errors, capacity refusals, and failed
/// WebSocket greetings return visibly over IPC and leave the device table
/// alone. The generation signal explicitly restarts this future after token
/// rotation; component-key remount behavior is not part of the contract.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
#[component]
pub(crate) fn DesktopBootstrapGate() -> Element {
    let config = use_context::<crate::desktop::WebviewBootstrap>();
    let mut ready = use_signal(|| false);
    let mut failure = use_signal(|| None::<String>);
    let generation = *DESKTOP_AUTH_GENERATION.read();
    let mut active_generation = use_signal(|| generation);

    let mut authentication = use_future(move || {
        let config = config.clone();
        async move {
            let token = match crate::desktop::current_token().await {
                Ok(token) => token,
                Err(error) => {
                    failure.set(Some(format!(
                        "reading the desktop bootstrap token: {error:#}"
                    )));
                    return;
                }
            };
            // The asset expression returns the authentication promise. Await
            // it here so Dioxus keeps the eval channel alive through every
            // recv/send pair; firing it and returning would close IPC while
            // the webview was still validating its credential.
            let mut eval =
                document::eval(concat!("await ", include_str!("../assets/desktop-auth.js")));
            if let Err(error) = eval.send(serde_json::json!({
                "base": config.base,
                "token": token,
                "persisted": config.persisted_secret,
            })) {
                failure.set(Some(format!(
                    "sending desktop authentication over IPC: {error}"
                )));
                return;
            }
            let mut exchange = match eval.recv::<DesktopExchange>().await {
                Ok(exchange) => exchange,
                Err(error) => {
                    failure.set(Some(format!("desktop authentication IPC failed: {error}")));
                    return;
                }
            };
            if exchange.retry_token {
                let token = match crate::desktop::current_token().await {
                    Ok(token) => token,
                    Err(error) => {
                        failure.set(Some(format!(
                            "re-reading the desktop bootstrap token: {error:#}"
                        )));
                        return;
                    }
                };
                if let Err(error) = eval.send(serde_json::json!({ "token": token })) {
                    failure.set(Some(format!(
                        "retrying desktop authentication over IPC: {error}"
                    )));
                    return;
                }
                exchange = match eval.recv::<DesktopExchange>().await {
                    Ok(exchange) => exchange,
                    Err(error) => {
                        failure.set(Some(format!(
                            "desktop authentication retry IPC failed: {error}"
                        )));
                        return;
                    }
                };
            }
            if let Some(error) = exchange.error {
                failure.set(Some(format!("desktop authentication failed: {error}")));
                return;
            }
            let Some(secret) = exchange.secret else {
                failure.set(Some(
                    "desktop authentication IPC returned neither a credential nor an error"
                        .to_string(),
                ));
                return;
            };
            if let Err(error) = crate::desktop::persist_webview_secret(secret) {
                let _ = eval.send(serde_json::json!({ "persisted": false }));
                failure.set(Some(format!(
                    "persisting webview device session: {error:#}"
                )));
                return;
            }
            if let Err(error) = eval.send(serde_json::json!({ "persisted": true })) {
                failure.set(Some(format!(
                    "acknowledging persisted webview session over IPC: {error}"
                )));
                return;
            }
            let committed = match eval.recv::<DesktopExchange>().await {
                Ok(committed) => committed,
                Err(error) => {
                    failure.set(Some(format!(
                        "desktop authentication commit IPC failed: {error}"
                    )));
                    return;
                }
            };
            if let Some(error) = committed.error {
                failure.set(Some(format!("desktop authentication failed: {error}")));
                return;
            }
            if !committed.ready {
                failure.set(Some(
                    "desktop authentication IPC did not confirm browser persistence".to_string(),
                ));
                return;
            }
            *TOKEN_REQUIRED.write() = false;
            ready.set(true);
        }
    });
    use_effect(use_reactive((&generation,), move |(generation,)| {
        if generation != *active_generation.peek() {
            active_generation.set(generation);
            ready.set(false);
            failure.set(None);
            authentication.restart();
        }
    }));

    if *ready.read() {
        rsx! { crate::AppBody {} }
    } else if let Some(detail) = failure.read().as_ref() {
        rsx! { main { class: "auth-page", p { class: "auth-error", role: "alert", "{detail}" } } }
    } else {
        rsx! { main { class: "auth-page", p { "Starting Farhelm…" } } }
    }
}

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

/// Read the process-scoped credential installed by native desktop bootstrap.
///
/// Native HTTP and the webview deliberately hold separate device sessions;
/// this accessor never exposes the native secret to JavaScript.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn device_secret() -> Option<String> {
    NATIVE_DEVICE_SECRET
        .get()
        .and_then(|secret| secret.read().ok()?.clone())
}

/// The native reqwest stack's explicit device credential.
///
/// Desktop bootstrap installs this before Dioxus launches. It is process
/// state rather than a cookie jar because the helm accepts one explicit
/// credential on every edge and the native client must not acquire ambient
/// browser behavior by accident.
#[cfg(not(target_arch = "wasm32"))]
static NATIVE_DEVICE_SECRET: std::sync::OnceLock<std::sync::RwLock<Option<String>>> =
    std::sync::OnceLock::new();

/// Replace the credential read by subsequent native REST requests.
#[cfg(all(not(target_arch = "wasm32"), feature = "desktop"))]
pub(crate) fn install_native_device_secret(secret: String) {
    let slot = NATIVE_DEVICE_SECRET.get_or_init(|| std::sync::RwLock::new(None));
    *slot
        .write()
        .expect("native device credential lock poisoned") = Some(secret);
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
#[cfg_attr(
    all(feature = "desktop", not(target_arch = "wasm32")),
    allow(dead_code)
)]
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

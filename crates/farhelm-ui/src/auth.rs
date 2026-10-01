//! Device authentication surfaces for the browser and native desktop app.
//!
//! The browser raises an ordinary in-page token prompt after a recognized
//! 401. Desktop bootstrap instead performs two exchanges: native REST owns a
//! process-scoped credential, while the webview receives a separate one over
//! IPC for localStorage and WebSocket subprotocols. In the browser, every
//! successful exchange remounts the active surface and feed so no reader
//! retains revoked state. A desktop re-authentication instead runs underneath
//! the live app (see [`DesktopBootstrapGate`]): the sockets that rotation
//! closed reconnect on their own, reading the replacement secret from
//! localStorage at each attempt.

use crate::{ApiBase, api};
use dioxus::prelude::*;

#[cfg(native_desktop)]
#[derive(Debug, serde::Deserialize)]
struct DesktopExchange {
    secret: Option<String>,
    error: Option<String>,
    /// The page has no usable device secret and asks native to mint one.
    #[serde(default)]
    need_secret: bool,
    #[serde(default)]
    ready: bool,
}

/// Incremented when native REST observes a revoked desktop credential. The
/// gate reacts by restarting its completed authentication future, running a
/// new validation/exchange inside the webview context and replacing that
/// client's independently revoked WebSocket credential too. The bump does not
/// unmount the app: the native request that triggered it is still in flight
/// and must be able to report its outcome (SPEC.md "Signing in again").
#[cfg(native_desktop)]
pub(crate) static DESKTOP_AUTH_GENERATION: GlobalSignal<u64> = Signal::global(|| 0);

#[cfg(native_desktop)]
pub(crate) fn require_desktop_webview_reauth() {
    *DESKTOP_AUTH_GENERATION.write() += 1;
}

/// Mint the webview's device secret with the web token, re-reading the token
/// once if the helm refuses it: the token file is a rotation boundary, and a
/// rotation between reading and using it is the one refusal a retry fixes. A
/// second refusal is reported rather than retried without bound.
#[cfg(native_desktop)]
async fn mint_webview_secret(base: &str) -> Result<String, String> {
    for _ in 0..2 {
        let token = crate::desktop::current_token()
            .await
            .map_err(|error| format!("reading the desktop bootstrap token: {error:#}"))?;
        let deadline = tokio::time::Instant::now() + api::WEBVIEW_EXCHANGE_TIMEOUT;
        if let Some(secret) = api::mint_webview_device_secret(base, &token, deadline).await? {
            return Ok(secret);
        }
    }
    Err("webview device exchange failed with 401 Unauthorized".to_string())
}

/// Hold the component tree behind the webview's own authenticated exchange.
///
/// Native bootstrap has already authenticated reqwest. This second exchange
/// runs inside the webview so its localStorage and WebSocket subprotocol carry
/// a separately minted device credential. Values enter JavaScript through the
/// eval IPC channel, never through a URL or rendered DOM, and the web token
/// never enters it at all: native mints the webview's device secret itself
/// (`mint_webview_secret`) and hands over only that. A validation 401 (or no
/// stored secret) is the only state that mints: transport errors, capacity refusals, and failed
/// WebSocket greetings return visibly over IPC and leave the device table
/// alone. The generation signal explicitly restarts this future after token
/// rotation; component-key remount behavior is not part of the contract.
///
/// Only the first authentication holds the tree back. Once the app has
/// mounted it stays mounted through every later re-authentication, so an
/// action that ran into the rotation can still report its outcome; only a
/// failure replaces it, with a Retry button that starts another run
/// (SPEC.md "Signing in again").
#[cfg(native_desktop)]
#[component]
pub(crate) fn DesktopBootstrapGate() -> Element {
    let config = use_context::<crate::desktop::WebviewBootstrap>();
    let mut state = use_signal(GateState::default);
    let generation = *DESKTOP_AUTH_GENERATION.read();
    let mut active_generation = use_signal(|| generation);

    let mut authentication = use_future(move || {
        let config = config.clone();
        async move {
            // Stop the console shim from SPENDING the outgoing credential
            // before its replacement exists: this future restarting is
            // exactly the reauthentication window in which the old device
            // secret may already be revoked, and a flush timer firing mid-
            // window would drain queued entries into 401s. Capture keeps
            // running; only sending pauses, until the success path below
            // re-arms with the fresh secret. Harmless on the first run
            // (an unarmed shim, or one not yet loaded).
            document::eval(
                "if (window.__farhelmClientLog) { window.__farhelmClientLog.disarm(); }",
            );
            // The asset expression returns the authentication promise. Await
            // it here so Dioxus keeps the eval channel alive through every
            // recv/send pair; firing it and returning would close IPC while
            // the webview was still validating its credential.
            let mut eval =
                document::eval(concat!("await ", include_str!("../assets/desktop-auth.js")));
            // No web token goes to the page at all. If its stored device
            // secret is missing or refused it asks (`need_secret`), and native
            // mints one with the token itself, so the helm's root credential
            // never enters JavaScript that later-shown content could have
            // tampered with (SPEC.md "Client hardening").
            if let Err(error) = eval.send(serde_json::json!({
                "base": config.base,
                "persisted": config.persisted_secret,
            })) {
                state
                    .write()
                    .fail(format!("sending desktop authentication over IPC: {error}"));
                return;
            }
            let mut exchange = match eval.recv::<DesktopExchange>().await {
                Ok(exchange) => exchange,
                Err(error) => {
                    state
                        .write()
                        .fail(format!("desktop authentication IPC failed: {error}"));
                    return;
                }
            };
            if exchange.need_secret {
                let minted = mint_webview_secret(&config.base).await;
                let reply = match &minted {
                    Ok(secret) => serde_json::json!({ "secret": secret }),
                    Err(error) => serde_json::json!({ "error": error }),
                };
                if let Err(error) = eval.send(reply) {
                    state.write().fail(format!(
                        "sending the minted webview credential over IPC: {error}"
                    ));
                    return;
                }
                exchange = match eval.recv::<DesktopExchange>().await {
                    Ok(exchange) => exchange,
                    Err(error) => {
                        state.write().fail(format!(
                            "desktop authentication IPC failed after minting: {error}"
                        ));
                        return;
                    }
                };
            }
            if let Some(error) = exchange.error {
                state
                    .write()
                    .fail(format!("desktop authentication failed: {error}"));
                return;
            }
            let Some(secret) = exchange.secret else {
                state.write().fail(
                    "desktop authentication IPC returned neither a credential nor an error"
                        .to_string(),
                );
                return;
            };
            if let Err(error) = crate::desktop::persist_webview_secret(secret.clone()) {
                let _ = eval.send(serde_json::json!({ "persisted": false }));
                state
                    .write()
                    .fail(format!("persisting webview device session: {error:#}"));
                return;
            }
            if let Err(error) = eval.send(serde_json::json!({ "persisted": true })) {
                state.write().fail(format!(
                    "acknowledging persisted webview session over IPC: {error}"
                ));
                return;
            }
            let committed = match eval.recv::<DesktopExchange>().await {
                Ok(committed) => committed,
                Err(error) => {
                    state
                        .write()
                        .fail(format!("desktop authentication commit IPC failed: {error}"));
                    return;
                }
            };
            if let Some(error) = committed.error {
                state
                    .write()
                    .fail(format!("desktop authentication failed: {error}"));
                return;
            }
            if !committed.ready {
                state.write().fail(
                    "desktop authentication IPC did not confirm browser bootstrap completion"
                        .to_string(),
                );
                return;
            }
            // Arm the console shim now that a device session genuinely
            // exists (committed to disk, not merely exchanged) — see
            // `arm_client_log_shim`'s docs for why this exact point in the
            // flow is "success" for the shim's purposes too, including on a
            // REauthentication after `DESKTOP_AUTH_GENERATION` bumps and this
            // whole future restarts.
            arm_client_log_shim(
                &config.base,
                &secret,
                config.smoke_client_log_marker.as_deref(),
            );
            // Same arming point, same reasoning, same re-arm on every
            // reauthentication — see `arm_native_clipboard`'s docs.
            arm_native_clipboard(&config.base, &secret);
            *TOKEN_REQUIRED.write() = false;
            state.write().succeed();
        }
    });
    use_effect(use_reactive((&generation,), move |(generation,)| {
        if generation != *active_generation.peek() {
            active_generation.set(generation);
            state.write().restart();
            authentication.restart();
        }
    }));

    match state.read().view() {
        GateView::Starting => rsx! { main { class: "auth-page", p { "Starting Farhelm…" } } },
        GateView::App => rsx! { crate::AppBody {} },
        // Every failure offers Retry, with no attempt to sort transient
        // causes from permanent ones: a retry of a permanent failure just
        // shows the same error again, while a failure with no way out left
        // the window dead until the app was relaunched. Retry goes through
        // the same generation bump a native 401 uses, so there is one
        // restart path; the run it starts shows "Starting Farhelm…" rather
        // than the app until it succeeds (`GateState::held`).
        GateView::Failed(detail) => rsx! {
            main { class: "auth-page",
                div { class: "auth-card",
                    p { class: "auth-error", role: "alert", "{detail}" }
                    button {
                        class: "btn btn-primary auth-submit",
                        r#type: "button",
                        onclick: move |_| require_desktop_webview_reauth(),
                        "Retry"
                    }
                }
            }
        },
    }
}

/// What [`DesktopBootstrapGate`] has learned about the webview's
/// authentication, kept apart from the component so the rule a regression
/// would break is testable without a webview.
///
/// That rule is that `authenticated` never goes back to false. Restarting
/// for a re-authentication used to reset it, which unmounted `AppBody` and
/// dropped every component-scoped task with it, including the very one
/// awaiting the native retry that bumped the generation: the Delete, Stop
/// or rename that ran into a token rotation ended with no outcome on screen.
/// The page's own sockets need no remount to recover. Rotation closes them,
/// and they reconnect with whatever secret localStorage holds at each
/// attempt, which the gate's future replaces before it reports success.
/// They reconnect on their own retry ladders, though, not the moment the
/// new secret lands, so a terminal can show "reconnecting" for a few
/// seconds longer than the old remount took, or until the next background
/// probe if its ladder was already spent. Nothing here nudges them: the
/// user can always reconnect by hand, and SPEC.md "Signing in again" puts
/// little weight on preserving UI state across a sign-in.
#[cfg(native_desktop)]
#[derive(Debug, Default)]
struct GateState {
    /// Whether this window has authenticated at least once.
    authenticated: bool,
    /// Why the latest authentication run failed, until another run starts.
    failure: Option<String>,
    /// Whether the app stays off screen until the current run succeeds,
    /// even though the window authenticated before. Set when a run starts
    /// from the failure page (the Retry button): the webview credential
    /// that just failed is still the one in place, and an app put back on
    /// screen during that run would let the user start an action that a
    /// repeat failure then cuts off with no outcome shown.
    held: bool,
}

#[cfg(native_desktop)]
impl GateState {
    /// Start another authentication run. Clears a failure, and leaves the
    /// app mounted if it already was: a run that starts from the app (a
    /// native 401) keeps it, a run that starts from the failure page keeps
    /// it off screen until it succeeds (see `held`).
    fn restart(&mut self) {
        if self.failure.take().is_some() {
            self.held = true;
        }
    }

    fn succeed(&mut self) {
        self.authenticated = true;
        self.held = false;
        self.failure = None;
    }

    fn fail(&mut self, detail: String) {
        self.failure = Some(detail);
    }

    /// A failure wins even over an app that is already mounted: the
    /// webview's own credential is unusable then, so its terminals and event
    /// feed cannot reconnect, and `.auth-page` deliberately replaces rather
    /// than floats over controls that cannot succeed. The native retry that
    /// asked for the re-authentication has settled by then
    /// (`api::retry_desktop_request` asks only afterwards), so at most the
    /// caller's short read of an already-received reply can be cut off.
    fn view(&self) -> GateView<'_> {
        match (&self.failure, self.authenticated && !self.held) {
            (Some(detail), _) => GateView::Failed(detail),
            (None, true) => GateView::App,
            (None, false) => GateView::Starting,
        }
    }
}

/// The three things the desktop gate can show.
#[cfg(native_desktop)]
#[derive(Debug, PartialEq, Eq)]
enum GateView<'a> {
    /// The first authentication is still running, or a run started from
    /// the failure page is.
    Starting,
    /// The app, mounted for the rest of the window's life unless a later
    /// authentication run fails.
    App,
    /// The latest authentication run failed, with its detail.
    Failed(&'a str),
}

/// Hand the webview console shim (`assets/client-log-shim.js`) the loopback
/// origin and device secret it needs to start flushing its buffered
/// captures, mirroring how the exchange above hands the shim's OWN
/// credential across the same IPC boundary.
///
/// A fire-and-forget one-shot eval, exactly like `feed.rs`'s subscription
/// cleanup and `session_view.rs`'s island sync snippet: nothing here needs
/// to observe a result. The script BOTH stores the configuration in the
/// shim's pending global AND arms directly when the shim already installed —
/// script REGISTRATION order is not execution order for Dioxus-injected
/// assets, so either side may win the load race, and the pending global is
/// what makes both orders arrive at an armed shim (the shim consumes it on
/// install; see its module header).
///
/// The credential crosses in this JSON payload, lives in the shim's armed
/// state for as long as it is current, and is transmitted only in the
/// `Authorization` header the shim sends — never through `tracing`, never
/// through a `console.log`, exactly like the desktop device secret
/// exchanged just above it in this same function.
///
/// `marker` carries `WebviewBootstrap::smoke_client_log_marker` through
/// unchanged: `None` in every real run, `Some` only under
/// `scripts/desktop-smoke.sh`, which is what lets the shim prove its own
/// pipeline by echoing the marker back through `console.error` once armed.
#[cfg(native_desktop)]
fn arm_client_log_shim(base: &str, secret: &str, marker: Option<&str>) {
    document::eval(&arm_client_log_script(base, secret, marker));
}

/// Build `arm_client_log_shim`'s script. Split out so the one property that
/// makes it safe — every value crossing through `serde_json` rather than
/// string interpolation, this crate's rule for building JavaScript that
/// touches the one origin able to reach the helm's API — is pinned by a
/// unit test with hostile punctuation, and a later edit cannot quietly
/// regress to interpolation without that test noticing.
#[cfg(native_desktop)]
fn arm_client_log_script(base: &str, secret: &str, marker: Option<&str>) -> String {
    // A `None` marker serializes as `"smokeMarker": null`, which the shim's
    // `if (config.smokeMarker)` guard treats identically to an absent key —
    // one construction path for all three values, no representation branch
    // to test or drift.
    let payload = serde_json::to_string(
        &serde_json::json!({ "base": base, "secret": secret, "smokeMarker": marker }),
    )
    .expect("an object of strings is always serializable");
    format!(
        "window.__farhelmClientLogPending = {payload}; \
         if (window.__farhelmClientLog) {{ \
           window.__farhelmClientLog.arm(window.__farhelmClientLogPending); \
         }}"
    )
}

/// Install `window.__farhelmNativeClipboardWrite`: the desktop webview's
/// working route to the system clipboard.
///
/// The webview cannot write the clipboard itself — WKWebView does not treat
/// the `dioxus://` page as a secure context, so `navigator.clipboard` is
/// simply absent there (farhelm-helm's clipboard.rs module docs carry the
/// full 2026-09 diagnosis) — so terminal.js's two copy paths prefer this
/// global when it exists and POST the text to the embedded helm's
/// `POST /api/clipboard`, where the native sink desktop.rs registered
/// performs the real write. A browser build never has the global installed
/// and keeps the web clipboard API path unchanged.
///
/// Armed at the same success point as the client-log shim, for the same
/// reason: only a COMMITTED device session may be spent, including on every
/// reauthentication. The installed function reads its base and secret from
/// `window.__farhelmNativeClipboardConfig` at CALL time rather than by
/// closure capture, so a re-arm after credential rotation refreshes even a
/// function object something captured earlier.
///
/// The write is fire-and-forget with every failure swallowed — the same
/// silent best-effort contract SPEC.md sets for every clipboard operation
/// and terminal.js's own provider documents; a 401 during the
/// reauthentication window is lost exactly like any other refused write.
#[cfg(native_desktop)]
fn arm_native_clipboard(base: &str, secret: &str) {
    document::eval(&arm_native_clipboard_script(base, secret));
}

/// Build `arm_native_clipboard`'s script. Split out for the same pinned
/// property as [`arm_client_log_script`]: every value crosses through
/// `serde_json`, never string interpolation, and the unit test below feeds
/// it hostile punctuation so a regression cannot land quietly.
#[cfg(native_desktop)]
fn arm_native_clipboard_script(base: &str, secret: &str) -> String {
    let payload = serde_json::to_string(&serde_json::json!({ "base": base, "secret": secret }))
        .expect("an object of strings is always serializable");
    format!(
        "window.__farhelmNativeClipboardConfig = {payload}; \
         window.__farhelmNativeClipboardWrite = function (text) {{ \
           try {{ \
             var config = window.__farhelmNativeClipboardConfig; \
             fetch(config.base + \"/api/clipboard\", {{ \
               method: \"POST\", \
               headers: {{ \
                 \"content-type\": \"application/json\", \
                 \"authorization\": \"Bearer \" + config.secret \
               }}, \
               body: JSON.stringify({{ text: String(text) }}) \
             }}).catch(function () {{}}); \
           }} catch (error) {{}} \
         }};"
    )
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
#[cfg(native_desktop)]
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
#[cfg_attr(native_desktop, allow(dead_code))]
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
                    class: "btn btn-primary auth-submit",
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

#[cfg(all(test, native_desktop))]
mod arm_script_tests {
    use super::arm_client_log_script;

    /// The arming script must keep hostile punctuation in EVERY field as
    /// JSON data, never as script syntax — the property that stops a base,
    /// secret, or smoke marker containing quotes, backslashes, or
    /// `</script>`-shaped text from changing what the eval executes. Pins
    /// the serde_json path so a later edit cannot quietly regress to string
    /// interpolation. Covers the marker field alongside base/secret because
    /// it is built the same way (`serde_json::Value::from`) and deserves
    /// the identical proof, not a weaker one just because it is optional.
    #[farhelm_testtrace::test]
    fn hostile_punctuation_stays_json_data_in_the_arming_script() {
        let script = arm_client_log_script(
            r#"http://127.0.0.1:7433/"; window.pwned = 1; ""#,
            r#"se"cr\et
with `newline` and ${interpolation}"#,
            Some(r#"mark"er\with `newline`"#),
        );
        assert!(
            script.starts_with("window.__farhelmClientLogPending = {"),
            "the payload must be assigned as one JSON object literal: {script}"
        );
        assert!(
            script.contains(r#"\"; window.pwned = 1; \""#),
            "quotes in the base must arrive escaped, not as live syntax: {script}"
        );
        assert!(
            script.contains(r#"se\"cr\\et\nwith"#),
            "quotes, backslashes, and newlines in the secret must be JSON-escaped: {script}"
        );
        assert!(
            !script.contains("se\"cr\\et\nwith"),
            "the raw secret text must not appear unescaped anywhere in the script"
        );
        assert!(
            script.contains(r#"mark\"er\\with `newline`"#),
            "quotes and backslashes in the smoke marker must be JSON-escaped too: {script}"
        );
        assert!(
            !script.contains("mark\"er\\with `newline`"),
            "the raw smoke marker text must not appear unescaped anywhere in the script"
        );
    }

    /// The clipboard arming script carries the same two credentials over the
    /// same eval boundary, and must hold the same property: hostile
    /// punctuation in the base or secret stays JSON data, never script
    /// syntax. A separate pin rather than trusting the sibling test because
    /// the two builders are separate functions that can regress separately.
    #[farhelm_testtrace::test]
    fn hostile_punctuation_stays_json_data_in_the_clipboard_arming_script() {
        let script = super::arm_native_clipboard_script(
            r#"http://127.0.0.1:7433/"; window.pwned = 1; ""#,
            r#"se"cr\et
with `newline` and ${interpolation}"#,
        );
        assert!(
            script.starts_with("window.__farhelmNativeClipboardConfig = {"),
            "the config must be assigned as one JSON object literal: {script}"
        );
        assert!(
            script.contains(r#"\"; window.pwned = 1; \""#),
            "quotes in the base must arrive escaped, not as live syntax: {script}"
        );
        assert!(
            script.contains(r#"se\"cr\\et\nwith"#),
            "quotes, backslashes, and newlines in the secret must be JSON-escaped: {script}"
        );
        assert!(
            !script.contains("se\"cr\\et\nwith"),
            "the raw secret text must not appear unescaped anywhere in the script"
        );
        assert!(
            script.contains("window.__farhelmNativeClipboardWrite = function"),
            "the script must install the writer terminal.js prefers: {script}"
        );
    }
}

#[cfg(all(test, native_desktop))]
mod gate_tests {
    /// A desktop re-authentication must not unmount the app. The native
    /// retry that bumps the generation runs inside a component-scoped task
    /// (a Delete, a Stop, a rename), and unmounting `AppBody` dropped that
    /// task, so the action that ran into a token rotation finished with no
    /// outcome on screen (SPEC.md "Signing in again"). Only the first run
    /// shows "Starting Farhelm…".
    #[farhelm_testtrace::test]
    fn reauthentication_keeps_the_app_mounted() {
        let mut state = super::GateState::default();
        assert_eq!(state.view(), super::GateView::Starting);
        state.succeed();
        assert_eq!(state.view(), super::GateView::App);
        state.restart();
        assert_eq!(
            state.view(),
            super::GateView::App,
            "a running re-authentication must leave the app mounted"
        );
        state.succeed();
        assert_eq!(state.view(), super::GateView::App);
    }

    /// A failed authentication run replaces the app, before or after the
    /// first success, and the next run clears it: the webview credential is
    /// unusable while it stands, and a stale failure must not outlive the
    /// run that replaces it.
    #[farhelm_testtrace::test]
    fn failure_replaces_the_app_until_the_next_run() {
        let mut state = super::GateState::default();
        state.fail("first".to_string());
        assert_eq!(state.view(), super::GateView::Failed("first"));
        state.restart();
        assert_eq!(state.view(), super::GateView::Starting);
        state.succeed();
        state.restart();
        state.fail("later".to_string());
        assert_eq!(state.view(), super::GateView::Failed("later"));
    }

    /// A run started from the failure page (Retry) keeps the app off screen
    /// until it succeeds, even when the window authenticated before. The
    /// webview credential that just failed is still in place during that
    /// run, and an app shown meanwhile would let the user start a Delete or
    /// Stop that a repeat failure then cuts off with no outcome (SPEC.md
    /// "Signing in again"). A run started from the app (a native 401) is
    /// the other case and keeps the app mounted; see
    /// `reauthentication_keeps_the_app_mounted`.
    #[farhelm_testtrace::test]
    fn retry_from_a_failure_holds_the_app_until_success() {
        let mut state = super::GateState::default();
        state.succeed();
        state.restart();
        state.fail("rotated".to_string());
        state.restart();
        assert_eq!(state.view(), super::GateView::Starting);
        state.fail("again".to_string());
        assert_eq!(state.view(), super::GateView::Failed("again"));
        state.restart();
        assert_eq!(state.view(), super::GateView::Starting);
        state.succeed();
        assert_eq!(state.view(), super::GateView::App);
        state.restart();
        assert_eq!(
            state.view(),
            super::GateView::App,
            "after a success, a native 401 restarts with the app mounted again"
        );
    }
}

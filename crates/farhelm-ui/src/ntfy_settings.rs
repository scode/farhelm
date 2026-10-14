//! ntfy configuration has an explicit-save lifecycle, unlike replayed shared
//! preferences. The form never receives the saved token and never sends alerts.

use crate::{
    ApiBase,
    api::{fetch_ntfy_settings, save_ntfy_settings, test_ntfy_settings},
};
use dioxus::prelude::*;
use farhelm_proto::ntfy::{Priority, SaveSettings, Settings};
use std::{cell::Cell, rc::Rc};

/// An empty replacement preserves the existing secret; clearing is a separate
/// deliberate choice. This keeps ordinary edits from accidentally deleting it.
fn save_request(draft: Settings, token: String, clear: bool) -> SaveSettings {
    SaveSettings {
        enabled: draft.enabled,
        server_url: draft.server_url,
        topic: draft.topic,
        priority: draft.priority,
        token: if clear {
            Some(String::new())
        } else if token.is_empty() {
            None
        } else {
            Some(token)
        },
    }
}

/// Explicit Save and Send test report their own outcome inline. Requests survive
/// modal dismissal, but component state is touched only while mounted; the saved
/// token is never seeded into a password field or preference queue.
#[component]
pub(crate) fn NtfySettings() -> Element {
    let base = use_context::<ApiBase>().0;
    let read_base = base.clone();
    let mut load = use_resource(move || {
        let base = read_base.clone();
        async move { fetch_ntfy_settings(&base).await }
    });
    let mut initialized = use_signal(|| false);
    let mut draft = use_signal(Settings::default);
    let mut token = use_signal(String::new);
    let mut clear = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut outcome = use_signal(|| None::<Result<String, String>>);
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let drop_mounted = mounted.clone();
    use_drop(move || drop_mounted.set(false));
    use_effect(move || {
        if !initialized()
            && let Some(Ok(settings)) = load.read().as_ref()
        {
            draft.set(settings.clone());
            initialized.set(true);
        }
    });
    let error = load
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .cloned();
    let locked = !initialized() || busy();
    let testing_base = base.clone();
    let testing_mounted = mounted.clone();

    rsx! {
        section { class: "host-settings-section ntfy-settings", aria_label: "ntfy alerts",
            h3 { class: "host-settings-section-title", "ntfy alerts" }
            p { class: "host-settings-help",
                "Send a session's title and host when it waits for you. Applies to every window using this helm. In the desktop app, keep the window open."
            }
            label { class: "app-settings-choice",
                span { "send ntfy alerts" }
                input { r#type: "checkbox", class: "host-settings-switch", checked: draft.read().enabled, disabled: locked,
                    onchange: move |event| { draft.write().enabled = event.checked(); outcome.set(None); } }
            }
            label { class: "checkout-root-label", span { "server URL" }
                input { r#type: "url", value: "{draft.read().server_url}", readonly: locked,
                    autocomplete: "off", spellcheck: "false",
                    oninput: move |event| { draft.write().server_url = event.value(); outcome.set(None); } }
            }
            label { class: "checkout-root-label", span { "topic" }
                input { r#type: "text", value: "{draft.read().topic}", readonly: locked,
                    autocomplete: "off", spellcheck: "false", autocapitalize: "none",
                    oninput: move |event| { draft.write().topic = event.value(); outcome.set(None); } }
            }
            p { class: "host-settings-help", "Anyone who knows an ntfy.sh topic can read its alerts. Choose a hard-to-guess topic." }
            label { class: "checkout-root-label", span { "access token (optional)" }
                input { r#type: "password", value: "{token}", readonly: locked || clear(),
                    autocomplete: "new-password", spellcheck: "false",
                    placeholder: if draft.read().token_set { "token saved; leave blank to keep it" } else { "no token saved" },
                    oninput: move |event| { token.set(event.value()); outcome.set(None); } }
            }
            label { class: "app-settings-choice",
                span { "clear saved token" }
                input { r#type: "checkbox", class: "host-settings-switch", checked: clear(), disabled: locked,
                    onchange: move |event| { clear.set(event.checked()); outcome.set(None); } }
            }
            label { class: "checkout-root-label", span { "priority" }
                select { value: draft.read().priority.header(), disabled: locked,
                    onchange: move |event| {
                        draft.write().priority = match event.value().as_str() {
                            "min" => Priority::Min, "low" => Priority::Low, "high" => Priority::High,
                            "urgent" => Priority::Urgent, _ => Priority::Default,
                        }; outcome.set(None);
                    },
                    option { value: "min", "min" } option { value: "low", "low" }
                    option { value: "default", "default" } option { value: "high", "high" }
                    option { value: "urgent", "urgent" }
                }
            }
            if let Some(error) = error {
                p { class: "checkout-root-error", role: "alert", "{error}" }
                button { r#type: "button", onclick: move |_| load.restart(), "retry loading" }
            }
            button { r#type: "button", disabled: locked,
                onclick: move |_| {
                    if locked { return; }
                    busy.set(true); outcome.set(None);
                    let request = save_request(draft(), token(), clear());
                    let base = base.clone(); let mounted = mounted.clone();
                    dioxus::core::spawn_forever(async move {
                        let result = save_ntfy_settings(&base, request).await;
                        if !mounted.get() { return; }
                        busy.set(false);
                        match result {
                            Ok(settings) => { draft.set(settings); token.set(String::new()); clear.set(false);
                                outcome.set(Some(Ok("ntfy alert settings saved".into()))); }
                            Err(error) => outcome.set(Some(Err(error))),
                        }
                    });
                }, "save"
            }
            button { r#type: "button", disabled: locked,
                "data-tooltip": "send one alert using the saved settings, including while alerts are switched off",
                onclick: move |_| {
                    if locked { return; }
                    busy.set(true); outcome.set(None);
                    let base = testing_base.clone(); let mounted = testing_mounted.clone();
                    dioxus::core::spawn_forever(async move {
                        let result = test_ntfy_settings(&base).await;
                        if !mounted.get() { return; }
                        busy.set(false); outcome.set(Some(result.map(|()| "test alert sent".into())));
                    });
                }, "send test"
            }
            p { class: "host-settings-help", "Save before testing. Send test uses the saved settings, not this form's unsaved changes." }
            if busy() { p { role: "status", "working…" } }
            if let Some(result) = outcome() {
                match result {
                    Ok(message) => rsx! { p { class: "checkout-root-success", role: "status", "{message}" } },
                    Err(error) => rsx! { p { class: "checkout-root-error", role: "alert", "{error}" } },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unrelated saves preserve a token the form never received; explicit
    /// replacement and clearing cannot be confused with that preservation.
    #[test]
    fn secret_updates_are_explicit() {
        assert!(
            save_request(Settings::default(), String::new(), false)
                .token
                .is_none()
        );
        assert_eq!(
            save_request(Settings::default(), "replacement".into(), false)
                .token
                .as_deref(),
            Some("replacement")
        );
        assert_eq!(
            save_request(Settings::default(), "ignored".into(), true)
                .token
                .as_deref(),
            Some("")
        );
    }
}

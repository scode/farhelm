//! ntfy configuration has an explicit-save lifecycle, unlike the fire-and-forget
//! shared preferences. The form never receives the saved token and never sends alerts.

use crate::{
    ApiBase,
    api::{fetch_ntfy_settings, save_ntfy_settings, test_ntfy_settings},
};
use dioxus::prelude::*;
use farhelm_proto::ntfy::{Priority, SaveSettings, Settings};
use std::{cell::Cell, rc::Rc};

/// A blank replacement leaves the helm to apply its origin-bound preservation
/// rule. The form never knows the secret and cannot safely carry it between
/// servers; explicit replacement or clearing still wins over that rule.
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

/// Match the helm's URL-origin comparison for the pre-save explanation, including
/// default-port normalization. Invalid drafts cannot be saved and have no new
/// origin to compare; the helm remains authoritative for stale forms and writes.
fn server_origin_changed(saved: &str, draft: &str) -> bool {
    let parsed = |value: &str| {
        url::Url::parse(value)
            .ok()
            .filter(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
    };
    match (parsed(saved), parsed(draft)) {
        (Some(saved), Some(draft)) => saved.origin() != draft.origin(),
        _ => false,
    }
}

/// Explicit Save and Send test report their own outcome inline. Requests survive
/// modal dismissal, but component state is touched only while mounted; the saved
/// token is never seeded into a password field or preference queue.
/// Priority segments edit this same draft; selecting one never saves or sends.
/// Its heading is subordinate to the surrounding Notifications panel.
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
    // The editable draft cannot also be the origin baseline: edits must keep
    // comparing against the last successful load/save, not the last keystroke.
    let mut saved_server = use_signal(String::new);
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
            saved_server.set(settings.server_url.clone());
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
    let origin_changed = server_origin_changed(&saved_server(), &draft.read().server_url);
    let token_will_clear = draft.read().token_set && (clear() || origin_changed);
    let testing_base = base.clone();
    let testing_mounted = mounted.clone();

    rsx! {
        section { class: "host-settings-section ntfy-settings", aria_label: "ntfy alerts",
            h4 { class: "host-settings-section-title", "ntfy alerts" }
            p { class: "host-settings-help",
                "Send a session's title and host when it waits for you. Applies to every window using this helm. In the desktop app, keep the window open."
            }
            label { class: "app-settings-choice setting-row",
                span { "send ntfy alerts" }
                input { r#type: "checkbox", class: "control-switch", checked: draft.read().enabled, disabled: locked,
                    "data-tooltip": "send alerts when a session waits for you; Save applies this to every window using this helm",
                    onchange: move |event| { draft.write().enabled = event.checked(); outcome.set(None); } }
            }
            label { class: "checkout-root-label", span { "server URL" }
                input { r#type: "url", class: "font-mono", value: "{draft.read().server_url}", readonly: locked,
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    "data-tooltip": "ntfy server that receives session alerts",
                    oninput: move |event| { draft.write().server_url = event.value(); outcome.set(None); } }
            }
            label { class: "checkout-root-label", span { "topic" }
                input { r#type: "text", value: "{draft.read().topic}", readonly: locked,
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    "data-tooltip": "topic to publish alerts to; subscribe to the same topic in your ntfy client",
                    oninput: move |event| { draft.write().topic = event.value(); outcome.set(None); } }
            }
            p { class: "host-settings-help", "Anyone who knows an ntfy.sh topic can read its alerts. Choose a hard-to-guess topic." }
            label { class: "checkout-root-label", span { "access token (optional)" }
                // `new-password`, not `off`: browsers ignore `off` when they
                // fill saved logins into a password field, and the helm's own
                // web token is typically saved for this same origin. Filled
                // in here and saved, it would be sent to the ntfy server as
                // the bearer token on every alert.
                input { r#type: "password", class: "ntfy-token", value: "{token}", readonly: locked || clear(),
                    autocomplete: "new-password", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    "data-tooltip": if clear() {
                        "the saved access token will be cleared when you save"
                    } else if origin_changed && draft.read().token_set {
                        "optional token for this server; leaving it blank clears the previous server's saved token"
                    } else { "optional server access token; leave blank to preserve the saved token" },
                    placeholder: if token_will_clear { "saved token will be cleared" }
                        else if draft.read().token_set { "token saved; leave blank to keep it" } else { "no token saved" },
                    oninput: move |event| { token.set(event.value()); outcome.set(None); } }
            }
            if draft.read().token_set && origin_changed && token().is_empty() && !clear() {
                p { class: "host-settings-help ntfy-token-origin-hint", "changing the server clears the saved token when you save" }
            }
            label { class: "app-settings-choice setting-row",
                span { "clear saved token" }
                input { r#type: "checkbox", class: "control-switch", checked: clear(), disabled: locked,
                    "data-tooltip": "remove the saved access token when you Save",
                    onchange: move |event| { clear.set(event.checked()); outcome.set(None); } }
            }
            div { class: "checkout-root-label", span { "priority" }
                div { class: "segmented-control", role: "group", aria_label: "priority",
                    for priority in [Priority::Min, Priority::Low, Priority::Default, Priority::High, Priority::Urgent] {
                        button { r#type: "button", value: priority.header(), disabled: locked,
                            aria_pressed: draft.read().priority == priority,
                            "data-tooltip": "priority used for session alerts; Save applies the selected priority",
                            onclick: move |_| { draft.write().priority = priority; outcome.set(None); },
                            "{priority.header()}"
                        }
                    }
                }
            }
            if let Some(error) = error {
                p { class: "checkout-root-error", role: "alert", "{error}" }
                button { r#type: "button", class: "btn btn-neutral", "data-tooltip": "load the saved ntfy alert settings again", onclick: move |_| load.restart(), "retry loading" }
            }
            // These actions share presentation, not a submission: Send test
            // uses saved settings and must never apply this draft implicitly.
            crate::modal_dialog::DialogActions {
                button { r#type: "button", class: "btn btn-neutral", disabled: locked,
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
                button { r#type: "button", class: "btn btn-primary", disabled: locked,
                    "data-tooltip": "save ntfy alert settings for every window using this helm",
                    onclick: move |_| {
                        if locked { return; }
                        busy.set(true); outcome.set(None);
                        let request = save_request(draft(), token(), clear());
                        let base = base.clone(); let mounted = mounted.clone();
                        let saving_request = save_ntfy_settings(&base, request);
                        dioxus::core::spawn_forever(async move {
                            let result = saving_request.await;
                            if !mounted.get() { return; }
                            busy.set(false);
                            match result {
                                Ok(settings) => { saved_server.set(settings.server_url.clone()); draft.set(settings); token.set(String::new()); clear.set(false);
                                    outcome.set(Some(Ok("ntfy alert settings saved".into()))); }
                                Err(error) => outcome.set(Some(Err(error))),
                            }
                        });
                    }, "save"
                }
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

    /// The warning follows server authority, not topic or URL spelling. Match
    /// the backend's default-port treatment so equivalent addresses do not
    /// falsely promise token removal, and incomplete edits have no origin yet.
    #[test]
    fn token_warning_compares_normalized_server_origins() {
        for equivalent in [
            "https://example.test",
            "https://example.test:443/prefix",
            "https://EXAMPLE.test/",
        ] {
            assert!(!server_origin_changed("https://example.test", equivalent));
        }
        for different in [
            "http://example.test",
            "https://other.test",
            "https://example.test:8443",
        ] {
            assert!(server_origin_changed("https://example.test", different));
        }
        assert!(!server_origin_changed("https://example.test", "https://"));
    }
}

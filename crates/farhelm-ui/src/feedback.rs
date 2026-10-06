//! The "Send feedback" dialog the sidebar bar's help menu opens (SPEC.md
//! "Feedback").
//!
//! A deliberately small surface: a message, an optional way to reach the
//! user, and a plain display of everything else that goes with them. What
//! the dialog shows is the whole submission, so every attached field is
//! computed once, displayed, and sent from the same value: the version the
//! sidebar shows, desktop app or web UI, and the operating system the
//! browser or webview reports, each fitted to its cap first
//! ([`fit_machine_field`]) so a field the user cannot edit can never block
//! sending.
//!
//! Nothing leaves the machine until Send: the dialog posts to its own helm
//! (`api::send_feedback`), which forwards to the project's endpoint. A
//! failure keeps the typed text and says sending failed; nothing is queued
//! or retried. Only success changes shared contact memory, through a separate
//! best-effort preference write. Each dialog seeds from this client's
//! authenticated snapshot, so another client's changes need a reload.
//! Agents have no way to open this dialog or send through it.

use dioxus::prelude::*;
use farhelm_proto::feedback::{
    FEEDBACK_OS_MAX_CHARS, FEEDBACK_VERSION_MAX_CHARS, FeedbackSubmission, FeedbackSurface,
    fit_machine_field, is_blank,
};

use crate::api::{PreferenceValue, store_preference};
use crate::hosts::settings_dialog::install_dialog_with_selector;
use crate::list::SharedPreferences;
use crate::reader::sleep_ms;
use crate::{ApiBase, modal_isolation};

const DIALOG_SELECTOR: &str = r#".feedback-dialog[role="dialog"]"#;

/// What a successful send says, in the dialog and in the bar's status
/// region that announces it (`app_bar::AppBar`).
pub(crate) const THANKS: &str = "Thanks. Your feedback is on its way to Farhelm's maintainer.";

/// How long the thanks stays on screen before the dialog closes itself.
const THANKS_MS: u64 = 1_500;

/// Which client this is, decided at build time: `native_desktop` is the
/// desktop renderer on a non-wasm target (`build.rs`). The `desktop`
/// feature alone is not enough, since both renderer features can be on at
/// once and a wasm build then takes the web path.
fn surface() -> FeedbackSurface {
    if cfg!(native_desktop) {
        FeedbackSurface::Desktop
    } else {
        FeedbackSurface::Web
    }
}

/// How the dialog names the surface to the user.
fn surface_label(surface: FeedbackSurface) -> &'static str {
    match surface {
        FeedbackSurface::Desktop => "desktop app",
        FeedbackSurface::Web => "web UI",
    }
}

/// The operating system's common name from what the browser reports
/// (`navigator.userAgentData.platform` where it exists, else
/// `navigator.platform`), which is often a hardware string such as
/// `MacIntel` or `Linux x86_64`. Anything unrecognized is kept as reported;
/// nothing more specific (versions, device models) is read on purpose.
pub(crate) fn os_name(reported: &str) -> String {
    let lower = reported.to_ascii_lowercase();
    let name = if lower.contains("iphone") || lower.contains("ipad") {
        "iOS"
    } else if lower.contains("mac") {
        "macOS"
    } else if lower.contains("win") {
        "Windows"
    } else if lower.contains("android") {
        "Android"
    } else if lower.contains("linux") || lower.contains("x11") {
        "Linux"
    } else {
        return reported.to_string();
    };
    name.to_string()
}

/// Read the platform the page reports. A renderer that cannot answer leaves
/// an empty string, which [`fit_machine_field`] shows as `unknown`.
const PLATFORM_JS: &str = "return (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || '';";

/// Undo the dialog's modal isolation and give focus back to the help menu's
/// toggle, which opened it. The frame lets the closing render remove the
/// dialog before focus returns.
pub(crate) fn return_focus_to_help() {
    document::eval(&format!(
        "{} requestAnimationFrame(() => document.querySelector('.app-help-toggle')?.focus({{ preventScroll: true }}))",
        modal_isolation::release_js(DIALOG_SELECTOR),
    ));
}

/// Move focus to a control inside the dialog once the render that changed
/// it has landed. Focus is only taken back from the page body or the dialog
/// container (where the modal's safety net parks it), never from something
/// the user chose meanwhile.
fn refocus(selector: &str) {
    let selector = serde_json::to_string(&format!(".feedback-dialog {selector}"))
        .expect("a string always serializes");
    document::eval(&format!(
        "requestAnimationFrame(() => {{
            const active = document.activeElement;
            const dialog = document.querySelector('.feedback-dialog');
            if (active && active !== document.body && active !== dialog) return;
            document.querySelector({selector})?.focus({{ preventScroll: true }});
        }});"
    ));
}

/// Where one send stands.
#[derive(Clone, Debug, PartialEq, Eq)]
enum SendState {
    Editing,
    Sending,
    Sent,
    /// The "Couldn't send feedback: ..." sentence to show, from the helm or
    /// the transport (`api::send_feedback`).
    Failed(String),
}

/// Whether the dialog may close now. Not while a send is in flight: closing
/// would drop the send with no word on whether it arrived and lose the typed
/// text, which a failed send must keep.
///
/// Every user close path asks this of the LIVE state when it runs, not of the
/// render it was drawn in: Send sets the state synchronously, and a Cancel
/// click or Escape queued right behind it in the same event burst still runs
/// before the render that disables Cancel. The modal's own Escape fallback
/// only clicks Cancel, so Cancel's handler covers it.
fn may_close(state: &SendState) -> bool {
    *state != SendState::Sending
}

/// The contact to remember after a successful send, using the exact submission.
/// An unchecked choice or a blank field forgets the previous contact. Callers
/// must apply this only after success; editing, cancel and failure preserve it.
fn contact_after_success(sent: Option<String>, reuse: bool) -> Option<String> {
    sent.filter(|contact| reuse && !is_blank(contact))
}

/// The dialog. `version` is the version the sidebar shows, which is what
/// is sent. `on_sent` fires on success, so the parent's always-mounted
/// status region can announce it; `on_close` closes the dialog and the
/// parent returns focus. Contact starts from the shared authenticated snapshot;
/// editing and failed sends leave memory unchanged, and success writes the
/// captured submission and reuse choice through the preference queue.
#[component]
pub(crate) fn FeedbackDialog(
    version: String,
    on_sent: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut message = use_signal(String::new);
    let mut preferences = use_context::<SharedPreferences>();
    let mut contact = use_signal(move || {
        preferences
            .0
            .peek()
            .feedback_contact
            .clone()
            .unwrap_or_default()
    });
    let mut remember = use_signal(|| true);
    let mut os = use_signal(|| fit_machine_field("", FEEDBACK_OS_MAX_CHARS));
    let mut state = use_signal(|| SendState::Editing);
    let version = fit_machine_field(&version, FEEDBACK_VERSION_MAX_CHARS);
    let surface = surface();

    use_hook(move || {
        spawn(async move {
            let reported = document::eval(PLATFORM_JS)
                .join::<String>()
                .await
                .unwrap_or_default();
            os.set(fit_machine_field(
                &os_name(&reported),
                FEEDBACK_OS_MAX_CHARS,
            ));
        });
    });

    let submission = {
        let version = version.clone();
        move || FeedbackSubmission {
            message: message(),
            contact: (!is_blank(&contact.read())).then(|| contact.read().clone()),
            version: version.clone(),
            surface,
            os: os(),
        }
    };
    let validity = submission().validate();
    let sending = state() == SendState::Sending;
    // An empty message just disables Send; a message that is too long (or a
    // contact) says why, since nothing else on screen would.
    let size_problem = match &validity {
        Err(reason) if !is_blank(&message()) => Some(reason.clone()),
        _ => None,
    };
    let can_send = validity.is_ok() && !sending;

    let send = move |_| {
        // `can_send` is this render's view; a second activation handled
        // before the next render (most plausible across the desktop app's
        // bridge) must not start a second send and file a second issue.
        if !can_send || *state.peek() == SendState::Sending {
            return;
        }
        let base = base.clone();
        let outgoing = submission();
        let reuse = remember();
        state.set(SendState::Sending);
        spawn(async move {
            match crate::api::send_feedback(&base, &outgoing).await {
                Ok(()) => {
                    let remembered = contact_after_success(outgoing.contact.clone(), reuse);
                    preferences.0.write().feedback_contact = remembered.clone();
                    // Always write, even when this client's seed looks unchanged:
                    // another client may have replaced the helm's value meanwhile.
                    store_preference(&base, PreferenceValue::FeedbackContact(remembered));
                    state.set(SendState::Sent);
                    on_sent.call(());
                    // Send is gone and the fields were disabled while
                    // sending, so focus fell to the page; put it on close.
                    refocus(".feedback-cancel");
                    sleep_ms(THANKS_MS).await;
                    on_close.call(());
                }
                Err(reason) => {
                    state.set(SendState::Failed(reason));
                    // The disabled message field dropped focus; give it
                    // back so the user can edit or retry from the keyboard.
                    refocus(".feedback-message");
                }
            }
        });
    };

    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            div {
                class: "app-settings-dialog feedback-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "send feedback",
                tabindex: "-1",
                onmounted: move |_| install_dialog_with_selector(
                    DIALOG_SELECTOR, "textarea", Some(".feedback-cancel"),
                ),
                // Closing while a send is in flight would cancel it with no
                // word on whether it arrived and lose the text, so Escape
                // and cancel wait for the send (bounded by the helm's
                // timeout), reading the live state (`may_close`). Cancel is
                // also natively disabled meanwhile, once that render lands.
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape && !event.is_composing() && may_close(&state.peek()) {
                        on_close.call(());
                    }
                },
                h2 { class: "host-settings-title", "send feedback" }
                if state() == SendState::Sent {
                    p { class: "feedback-thanks", "{THANKS}" }
                } else {
                    p { class: "host-settings-help",
                        "This goes privately to Farhelm's maintainer, not to a public issue tracker. Nothing is sent until you press send."
                    }
                    label { class: "feedback-field",
                        "message"
                        textarea {
                            class: "feedback-message",
                            rows: "5",
                            required: true,
                            disabled: sending,
                            value: "{message}",
                            oninput: move |event| message.set(event.value()),
                        }
                    }
                    label { class: "feedback-field",
                        "how to reach you (optional)"
                        input {
                            r#type: "text",
                            class: "feedback-contact",
                            autocomplete: "email",
                            disabled: sending,
                            value: "{contact}",
                            oninput: move |event| contact.set(event.value()),
                        }
                    }
                    if !is_blank(&contact.read()) {
                        label {
                            class: "app-settings-choice",
                            "data-tooltip": "reuse: remember this contact on the helm after a successful send, for the desktop app and web UI",
                            input {
                                r#type: "checkbox",
                                class: "feedback-reuse",
                                checked: remember(),
                                disabled: sending,
                                onchange: move |event| remember.set(event.checked()),
                            }
                            "Re-use for future feedback"
                        }
                    }
                    div { class: "feedback-attached",
                        p { class: "host-settings-help", "Sent with it, and nothing else:" }
                        dl {
                            dt { "version" }
                            dd { class: "feedback-version peer-value", dir: "ltr", "{version}" }
                            dt { "app" }
                            dd { class: "feedback-surface", "{surface_label(surface)}" }
                            dt { "operating system" }
                            dd { class: "feedback-os peer-value", dir: "ltr", "{os}" }
                        }
                    }
                    if let Some(problem) = size_problem {
                        p { class: "feedback-error", role: "alert", "{problem}" }
                    }
                    if let SendState::Failed(reason) = state() {
                        p { class: "feedback-error", role: "alert",
                            "{reason} Your text is still here, so you can try again or copy it."
                        }
                    }
                }
                div { class: "host-settings-actions",
                    if state() != SendState::Sent {
                        button {
                            r#type: "button",
                            class: "btn btn-primary feedback-send",
                            "data-tooltip": "send: deliver this message privately to the maintainer",
                            disabled: !can_send,
                            onclick: send,
                            if sending { "sending…" } else { "send" }
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-neutral feedback-cancel",
                        "data-tooltip": if state() == SendState::Sent { "close: your message was sent" } else { "cancel: close without sending" },
                        disabled: sending,
                        onclick: move |_| {
                            if may_close(&state.peek()) {
                                on_close.call(());
                            }
                        },
                        if state() == SendState::Sent { "close" } else { "cancel" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reuse preserves exactly the sent contact; opting out or emptying the
    /// field forgets it. The send handler applies this only on success, whose
    /// integration boundary is covered by the feedback browser scenarios.
    #[farhelm_testtrace::test]
    fn successful_send_remembers_only_an_opted_in_nonblank_contact() {
        let sent = Some("  contact@example.test  ".to_string());
        assert_eq!(contact_after_success(sent.clone(), true), sent);
        assert_eq!(contact_after_success(sent, false), None);
        assert_eq!(contact_after_success(None, true), None);
        assert_eq!(
            contact_after_success(Some(" \u{2003} ".to_string()), true),
            None
        );
    }

    /// The operating system the dialog shows (and sends) is a common name,
    /// not the raw hardware string browsers report, and nothing beyond the
    /// name is read; an unrecognized value is kept as the browser said it.
    #[farhelm_testtrace::test]
    fn os_name_maps_reported_platforms_to_common_names() {
        assert_eq!(os_name("MacIntel"), "macOS");
        assert_eq!(os_name("macOS"), "macOS");
        assert_eq!(os_name("Linux x86_64"), "Linux");
        assert_eq!(os_name("Win32"), "Windows");
        assert_eq!(os_name("iPad"), "iOS");
        assert_eq!(os_name("FreeBSD amd64"), "FreeBSD amd64");
        assert_eq!(os_name(""), "");
    }

    /// Spec (`may_close`): the dialog may close in every state but an
    /// unresolved send. Why: every close path (Cancel, Escape, and the
    /// modal's Escape fallback, which clicks Cancel) asks this of the live
    /// state, so a close queued right behind Send cannot drop the send and
    /// the typed text; this pins the rule they share. The browser spec
    /// "a close queued behind send waits for the send" drives the paths.
    #[farhelm_testtrace::test]
    fn only_an_unresolved_send_keeps_the_dialog_open() {
        assert!(may_close(&SendState::Editing), "closing before Send works");
        assert!(!may_close(&SendState::Sending));
        assert!(may_close(&SendState::Sent));
        assert!(may_close(&SendState::Failed(
            "Couldn't send feedback: no.".to_string()
        )));
    }
}

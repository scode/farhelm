//! Helm-wide choices that have no natural home in the session or host rows.
//!
//! These are the same preferences the host dialogs write when the user answers
//! permanently. Update the shared client copy before queuing the sparse write:
//! the next add or removal in this window must see the answer immediately.
//! Other clients keep their loaded preferences until they reload, as usual.
//!
//! A desktop app whose updater runs adds one choice that is not a helm
//! preference: whether to install updates automatically. It belongs to that
//! app installation (SPEC.md "Session list"), so it is read from and written
//! to the app's own state file through the updater, never the helm.
//! Sound switches belong to this device's webview storage and remain live
//! even if a storage write fails; the sound asset owns those controls.
//! The checkout folder and post-clone command are helm configuration, not
//! preferences: each explicitly saves validated text and shows its outcome.
//! Ntfy alerts follow that explicit-save contract too, with a secret that is
//! never part of the shared client snapshot or replayed preference writes.
//! Terminal frame coloring is a helm preference too: an open session sees the
//! new choice through the shared signal, while other clients reload to get it.
//! Native tunnel consent also requires an acknowledged save; its controller
//! owns the shared warning and revokes listeners when the switch turns off.
//! The desktop download-opening list uses an acknowledged save too: the helm
//! decides what opens, while this dialog warns about listed types it does not
//! recognize as safe.

use dioxus::prelude::*;

use crate::api::{
    PreferenceValue, TerminalLinkClick, restore_session_confirmations, store_preference,
};
use crate::app_updater::use_app_updater;
use crate::hosts::settings_dialog::{HostDialogHeader, install_dialog_with_selector};
use crate::list::SharedPreferences;
use crate::{ApiBase, modal_isolation};

const DIALOG_SELECTOR: &str = r#".app-settings-dialog[role="dialog"]"#;

/// Undo modal isolation before focusing the gear, which is inert while open.
/// The frame lets the closing render remove the dialog before focus returns.
pub(crate) fn return_focus_to_gear() {
    document::eval(&format!(
        "{} requestAnimationFrame(() => document.querySelector('.app-settings-toggle')?.focus({{ preventScroll: true }}))",
        modal_isolation::release_js(DIALOG_SELECTOR),
    ));
}

/// Put the automatic-updates switch back to the setting in force after a
/// failed save.
///
/// The browser has already flipped the box when `onchange` runs, and when
/// the setting in force equals what the dialog last rendered, the signal
/// does not change and no render rewrites `checked`; the box would keep
/// showing the choice that was not saved. So the property is set directly.
fn restore_automatic_checkbox(in_force: bool) {
    document::eval(&format!(
        "(() => {{ const box = document.querySelector('{DIALOG_SELECTOR} input[aria-describedby=\"automatic-updates-help\"]'); if (box) box.checked = {in_force}; }})();"
    ));
}

/// Expose session/host choices, terminal link ownership, sounds and checkout settings.
///
/// Writes use the ordinary preference queue. Restoring session confirmations
/// reports a failed save because a reload could otherwise silently restore the
/// opt-out; other switch preferences keep their silent failure behavior.
/// Controls stay enabled, and closing the dialog must not cancel a queued write.
/// The checkout and ntfy fields are exceptions: their explicit saves expose validation
/// refusals rather than treating arbitrary text like a switch preference.
/// Tunnel consent is another exception: it is never optimistic or silently retried.
/// The desktop file-opening list also waits for an acknowledged save before
/// updating the shared preference; its warnings follow the unsaved draft.
/// Only the grouped body scrolls; both dismissal paths stay outside it and
/// share the caller's close callback and focus-return lifecycle.
#[component]
pub(crate) fn SettingsDialog(
    on_close: EventHandler<()>,
    #[props(default)] focus_claude_trust: bool,
    #[props(default)] focus_workspace_trust: Option<crate::workspace_trust::TrustAgent>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let setup_base = base.clone();
    let session_base = base.clone();
    let naming_base = base.clone();
    let frame_base = base.clone();
    let mut preferences = use_context::<SharedPreferences>();
    let setup = preferences.0.read().skip_host_setup_confirmation == Some(true);
    let remove = preferences.0.read().skip_host_remove_confirmation == Some(true);
    let skip_sessions = preferences.0.read().skip_session_confirmations == Some(true);
    // The positive naming preference defaults on for upgraded helms too;
    // treating an absent value like the confirmation opt-outs would disable it.
    let name_quick_launches = preferences.0.read().name_session_on_quick_launch != Some(false);
    let color_frames = preferences.0.read().color_terminal_frames == Some(true);
    // Superseded writes also report false. Only the latest click may explain
    // its failure, including when an intervening click opted out again.
    let mut session_generation = use_signal(|| 0_u64);
    let mut session_error = use_signal(|| false);
    let updater = use_app_updater();
    // Read once when the dialog opens and then kept locally: the file is
    // this app's own, so nothing else changes it while the dialog is up.
    let mut automatic = use_signal(|| {
        updater
            .as_ref()
            .is_some_and(|updater| updater.automatic_updates())
    });
    // Why the last change of that choice was not saved, until the next try.
    let mut automatic_error = use_signal(|| None::<String>);
    // Retain Claude's existing entry point while each flag-based band lands on
    // its own switch. The selector is built only from the closed agent enum.
    let trust_focus = if focus_claude_trust {
        Some("#checkout-claude-trust".to_string())
    } else {
        focus_workspace_trust.map(|agent| format!("#checkout-{}-trust", agent.key()))
    };

    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            div {
                class: "app-settings-dialog host-dialog-framed",
                role: "dialog",
                aria_modal: "true",
                aria_label: "settings",
                tabindex: "-1",
                onmounted: move |_| {
                    install_dialog_with_selector(
                        DIALOG_SELECTOR,
                        trust_focus.as_deref().unwrap_or("input"),
                        Some(".app-settings-close"),
                        None,
                    );
                    if let Some(selector) = &trust_focus {
                        // The shared dialog focus helper deliberately prevents
                        // scrolling. A band's entry point promises its setting
                        // and explanation, so bring that section into view.
                        document::eval(&format!("requestAnimationFrame(() => document.querySelector('{selector}')?.scrollIntoView({{block: 'start'}}))"));
                    }
                },
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape && !event.is_composing() {
                        on_close.call(());
                    }
                },
                HostDialogHeader {
                    title: rsx! { "settings" },
                    tile: rsx! {
                        crate::icons::SettingsIcon {}
                    },
                    summary: "for every host on this helm, and for this device",
                    dismiss_label: "close settings",
                    dismiss_tooltip: "close settings; unsaved fields are discarded",
                    on_dismiss: move |_| on_close.call(()),
                }
                div { class: "host-settings-body",
                    section { class: "host-settings-section", aria_label: "sessions",
                        h3 { class: "host-settings-section-title", "sessions" }
                        label {
                            class: "app-settings-choice",
                            "data-tooltip": "delete, replace, restart and close terminal tabs on the first click",
                            span { "skip session confirmations" }
                            input {
                                r#type: "checkbox",
                                class: "host-settings-switch",
                                checked: skip_sessions,
                                aria_describedby: "session-confirmations-help",
                                onchange: move |event| {
                                    let skip = event.checked();
                                    preferences.0.write().skip_session_confirmations = Some(skip);
                                    let generation = session_generation() + 1;
                                    session_generation.set(generation);
                                    session_error.set(false);
                                    if skip {
                                        store_preference(&session_base, PreferenceValue::SessionConfirmations(true));
                                    } else {
                                        let completion = restore_session_confirmations(&session_base);
                                        spawn(async move {
                                            let saved = completion.await;
                                            if session_generation() == generation {
                                                session_error.set(!saved);
                                            }
                                        });
                                    }
                                },
                            }
                        }
                        p { id: "session-confirmations-help", class: "host-settings-help",
                            "Delete, replace, restart and close terminal tabs on the first click. YOLO, host and trash confirmations still follow their own rules. This choice applies to this helm; other open clients pick it up when they reload."
                        }
                        if session_error() {
                            p { class: "host-settings-help app-settings-error", role: "alert",
                                "Confirmations may still be off for this helm. Try again."
                            }
                        }
                        label {
                            class: "app-settings-choice",
                            "data-tooltip": "offer to name a quick-launched session while its agent starts",
                            span { "name sessions on quick launch" }
                            input {
                                r#type: "checkbox",
                                class: "host-settings-switch",
                                checked: name_quick_launches,
                                aria_describedby: "quick-launch-naming-help",
                                onchange: move |event| {
                                    preferences.0.write().name_session_on_quick_launch = Some(event.checked());
                                    store_preference(
                                        &naming_base,
                                        PreferenceValue::NameSessionOnQuickLaunch(event.checked()),
                                    );
                                },
                            }
                        }
                        p { id: "quick-launch-naming-help", class: "host-settings-help",
                            "Offer to name a quick-launched session while its agent starts. Enter keeps its default name. This choice applies to this helm; other open clients pick it up when they reload."
                        }
                        label {
                            class: "app-settings-choice",
                            "data-tooltip": "color each terminal's frame to match its host icon",
                            span { "color terminal frames by host" }
                            input {
                                r#type: "checkbox",
                                class: "host-settings-switch",
                                checked: color_frames,
                                aria_describedby: "terminal-frame-color-help",
                                onchange: move |event| {
                                    let color = event.checked();
                                    preferences.0.write().color_terminal_frames = Some(color);
                                    store_preference(&frame_base, PreferenceValue::ColorTerminalFrames(color));
                                },
                            }
                        }
                        p { id: "terminal-frame-color-help", class: "host-settings-help",
                            "Match the thin frame around every session's terminal to its host icon color. The terminal background stays the same. This choice applies to this helm; other open clients pick it up when they reload."
                        }
                    }
                    section { class: "host-settings-section", aria_label: "hosts",
                        h3 { class: "host-settings-section-title", "hosts" }
                        p { class: "host-settings-help",
                            "The host choices apply to every host on this helm. Other open clients pick them up when they reload."
                        }
                        div {
                            label {
                                class: "app-settings-choice",
                                "data-tooltip": "set up a new host as soon as it has been checked, without showing the plan first",
                                span { "set up new hosts without asking" }
                                input {
                                    r#type: "checkbox",
                                    class: "host-settings-switch",
                                    checked: setup,
                                    aria_describedby: "host-setup-preference-help",
                                    onchange: move |event| {
                                        preferences.0.write().skip_host_setup_confirmation = Some(event.checked());
                                        store_preference(
                                            &setup_base,
                                            PreferenceValue::HostSetupConfirmation(event.checked()),
                                        );
                                    },
                                }
                            }
                            p {
                                id: "host-setup-preference-help",
                                class: "host-settings-help",
                                if setup {
                                    "Farhelm sets up a new host as soon as it has checked it, without asking."
                                } else {
                                    "Farhelm shows what setup will change on a new host and asks before doing it."
                                }
                            }
                        }
                        div {
                            label {
                                class: "app-settings-choice",
                                "data-tooltip": "forget a host as soon as you choose remove, without asking; its sessions keep running",
                                span { "remove hosts without asking" }
                                input {
                                    r#type: "checkbox",
                                    class: "host-settings-switch",
                                    checked: remove,
                                    aria_describedby: "host-remove-preference-help",
                                    onchange: move |event| {
                                        preferences.0.write().skip_host_remove_confirmation = Some(event.checked());
                                        store_preference(
                                            &base,
                                            PreferenceValue::HostRemoveConfirmation(event.checked()),
                                        );
                                    },
                                }
                            }
                            p {
                                id: "host-remove-preference-help",
                                class: "host-settings-help",
                                if remove {
                                    "Farhelm forgets a host as soon as you choose remove. Its sessions keep running."
                                } else {
                                    "Farhelm asks before forgetting a host. Its sessions keep running."
                                }
                            }
                        }
                    }
                    section {
                        class: "host-settings-section",
                        aria_label: "terminal links",
                        h3 { class: "host-settings-section-title", "terminal links" }
                        p { class: "host-settings-help",
                            "Clicking a web link in a terminal: this choice applies to every client of this helm. Other open clients pick it up when they reload. This web-link choice does not change file downloads."
                        }
                        TerminalLinkChoice {
                            value: TerminalLinkClick::Intercept,
                            label: "open in Farhelm, don't pass the click on",
                            help: "Farhelm opens the link once. The program does not receive that click or a drag starting there.",
                        }
                        TerminalLinkChoice {
                            value: TerminalLinkClick::Both,
                            label: "open in Farhelm and also pass the click on",
                            help: "Use this when the program needs link clicks. A program that opens links itself may open a second tab.",
                        }
                        TerminalLinkChoice {
                            value: TerminalLinkClick::Program,
                            label: "pass the click on only",
                            help: "Let the program handle links. Farhelm does not open them; without mouse reporting, clicking a link does nothing.",
                        }
                        NativeTunnelSettings {}
                        NativeOpenExtensionsSettings {}
                    }
                    section {
                        class: "host-settings-section",
                        aria_label: "managed checkouts",
                        h3 { class: "host-settings-section-title", "managed checkouts" }
                        crate::checkout_root::CheckoutRootField {
                            note: "Managed checkouts are cloned into this folder on every host. ~ means each host's own home folder. A missing folder is created on first use. A host given its own folder from the command line keeps that folder. Save an empty field to clear it.",
                            on_saved: move |_| {},
                        }
                        crate::checkout_post_clone::CheckoutPostCloneField {}
                        crate::claude_trust::ClaudeTrustSettings {}
                        crate::workspace_trust::WorkspaceTrustSettings { agent: crate::workspace_trust::TrustAgent::Codex }
                        crate::workspace_trust::WorkspaceTrustSettings { agent: crate::workspace_trust::TrustAgent::Muse }
                        crate::workspace_trust::WorkspaceTrustSettings { agent: crate::workspace_trust::TrustAgent::Pi }
                    }
                    crate::sounds::SoundSettings {}
                    crate::ntfy_settings::NtfySettings {}
                    if let Some(updater) = updater {
                        section {
                            class: "host-settings-section",
                            aria_label: "updates",
                            h3 { class: "host-settings-section-title", "updates" }
                            p { class: "host-settings-help",
                                "This one belongs to this Farhelm app on this Mac."
                            }
                            div {
                                label {
                                    class: "app-settings-choice",
                                    "data-tooltip": "install updates automatically: check for a new Farhelm release when the app starts and about once a day, and install it in the background",
                                    span { "install updates automatically" }
                                    input {
                                        r#type: "checkbox",
                                        class: "host-settings-switch",
                                        checked: automatic(),
                                        aria_describedby: "automatic-updates-help",
                                        // Only a saved choice is shown as the choice: on
                                        // a failed write the box returns to the setting
                                        // still in force, with the reason beside it.
                                        onchange: move |event| {
                                            match updater.set_automatic_updates(event.checked()) {
                                                Ok(()) => {
                                                    automatic.set(event.checked());
                                                    automatic_error.set(None);
                                                }
                                                Err(reason) => {
                                                    let in_force = updater.automatic_updates();
                                                    automatic.set(in_force);
                                                    automatic_error.set(Some(reason));
                                                    restore_automatic_checkbox(in_force);
                                                }
                                            }
                                        },
                                    }
                                }
                                p {
                                    id: "automatic-updates-help",
                                    class: "host-settings-help",
                                    if automatic() {
                                        "Farhelm checks for a new release when it starts and about once a day, and installs it in the background. Restarting Farhelm finishes the update."
                                    } else {
                                        "Farhelm installs a new release only when you check for updates from the ? menu or this machine's update item, or run the installer yourself."
                                    }
                                }
                                if let Some(reason) = automatic_error() {
                                    p {
                                        class: "host-settings-help app-settings-error",
                                        role: "alert",
                                        "{reason}"
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "host-settings-actions host-settings-footer",
                    // Only the desktop app shows the file-types field, so only
                    // its footer names it.
                    span {
                        if cfg!(native_desktop) {
                            "Most choices save immediately. Ntfy alerts, checkout fields and file types use save."
                        } else {
                            "Most choices save immediately. Ntfy alerts and checkout fields use save."
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-neutral app-settings-close",
                        "data-tooltip": "close settings; unsaved fields are discarded",
                        onclick: move |_| on_close.call(()),
                        "close"
                    }
                }
            }
        }
    }
}

/// The web renderer neither advertises tunnels nor contacts their native routes.
#[component]
fn NativeTunnelSettings() -> Element {
    #[cfg(native_desktop)]
    {
        rsx! { crate::tunnels::TunnelSettings {} }
    }
    #[cfg(not(native_desktop))]
    {
        rsx! {}
    }
}

/// Keep the desktop-only opener setting out of browsers, which cannot open a saved file.
#[component]
fn NativeOpenExtensionsSettings() -> Element {
    #[cfg(native_desktop)]
    {
        rsx! { OpenExtensionsSettings {} }
    }
    #[cfg(not(native_desktop))]
    {
        rsx! {}
    }
}

/// Warnings follow recognizable draft entries even when another entry is invalid.
///
/// A malformed full draft cannot be saved, but hiding a warning merely because
/// a neighboring token is malformed would make the risky type disappear while
/// the user is correcting the list. The parser still defines each token.
#[cfg(native_desktop)]
fn open_extension_warnings(
    raw: &str,
) -> Vec<(String, farhelm_proto::open_extensions::WarningKind)> {
    use farhelm_proto::open_extensions::{OpenExtensions, WarningKind};

    let mut warnings = Vec::new();
    for token in raw.split(',') {
        if let Ok(list) = OpenExtensions::parse(token) {
            for (extension, kind) in list.entries() {
                if kind != WarningKind::Safe && !warnings.iter().any(|(seen, _)| seen == extension)
                {
                    warnings.push((extension.to_string(), kind));
                    // A valid policy has at most 32 distinct entries. Bound
                    // the display for an invalid, arbitrarily long draft.
                    if warnings.len() == 32 {
                        return warnings;
                    }
                }
            }
        }
    }
    warnings
}

/// Edit the helm-wide opener policy with visible validation and live risk warnings.
///
/// The save survives dialog dismissal. The component's feedback handles do not:
/// after an awaited write, they are used only while the dialog is still mounted.
#[cfg(native_desktop)]
#[component]
fn OpenExtensionsSettings() -> Element {
    use std::{cell::Cell, rc::Rc};

    use farhelm_proto::open_extensions::{DEFAULT, OpenExtensions, WarningKind};

    let base = use_context::<ApiBase>().0;
    let mut preferences = use_context::<SharedPreferences>();
    let mut draft = use_signal(|| {
        preferences
            .0
            .read()
            .open_extensions
            .clone()
            .unwrap_or_else(|| DEFAULT.to_string())
    });
    let mut saving = use_signal(|| false);
    let mut outcome = use_signal(|| None::<Result<(), String>>);
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let drop_mounted = mounted.clone();
    use_drop(move || drop_mounted.set(false));
    let warnings = open_extension_warnings(&draft());
    let modifier = if cfg!(target_os = "macos") {
        "Cmd"
    } else {
        "Ctrl"
    };

    rsx! {
        div { class: "open-extensions-field",
            label { r#for: "open-extensions-input", class: "open-extensions-label",
                "file types to open with {modifier}-click"
            }
            input {
                id: "open-extensions-input", r#type: "text", value: "{draft}",
                readonly: saving(), aria_disabled: saving(),
                autocomplete: "off", spellcheck: "false", autocorrect: "off", autocapitalize: "none",
                aria_describedby: "open-extensions-help",
                oninput: move |event| {
                    if !saving() {
                        draft.set(event.value());
                        outcome.set(None);
                    }
                },
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Enter {
                        event.prevent_default();
                        event.stop_propagation();
                    }
                },
            }
            p { id: "open-extensions-help", class: "host-settings-help",
                "Comma-separated final extensions. A leading dot is optional. Save an empty field to stop opening downloaded files; downloads still save. This choice applies to this helm."
            }
            for (extension, kind) in warnings {
                div { class: "open-extensions-warning", role: "alert", key: "{extension}",
                    p { class: "open-extensions-warning-heading",
                        span { aria_hidden: "true", "⚠" }
                        " {extension}: "
                        if kind == WarningKind::Html { "an agent-written page may run in your browser" }
                        else { "no protection from the default app" }
                    }
                    p {
                        if kind == WarningKind::Html {
                            "An agent-written page can run scripts in your browser and open web addresses outside Farhelm's link permissions. It can send cross-site requests to services on your computer, including localhost on other ports, and those requests may carry the service's cookies even though the page cannot read them directly."
                        } else {
                            "Farhelm does not restrict what your default app does with this file. An agent can write this type of file, and opening it may run code on your computer."
                        }
                    }
                }
            }
            button {
                r#type: "button", class: "btn btn-neutral", aria_disabled: saving(),
                "data-tooltip": "save the file types this helm may open",
                onclick: move |_| {
                    if saving() { return; }
                    let value = match OpenExtensions::parse(&draft()) {
                        Ok(list) => list.normalized(),
                        Err(reason) => {
                            outcome.set(Some(Err(reason)));
                            return;
                        }
                    };
                    saving.set(true);
                    outcome.set(None);
                    let base = base.clone();
                    let mounted = mounted.clone();
                    // Explicit Save belongs to the app, even if the dialog closes.
                    dioxus::core::spawn_forever(async move {
                        let result = crate::api::write_open_extensions(&base, &value).await;
                        if result.is_ok() {
                            // The next dialog reads this shared mirror even
                            // when this one closed during the write.
                            preferences.0.write().open_extensions = Some(value.clone());
                        }
                        if !mounted.get() { return; }
                        saving.set(false);
                        if result.is_ok() {
                            draft.set(value);
                        }
                        outcome.set(Some(result));
                    });
                },
                if saving() { "saving…" } else { "save file types" }
            }
            if let Some(result) = outcome() {
                match result {
                    Ok(()) => rsx! { p { class: "checkout-root-success", role: "status", "file types saved" } },
                    Err(reason) => rsx! { p { class: "checkout-root-error", role: "alert", "{reason}" } },
                }
            }
        }
    }
}

#[cfg(all(test, native_desktop))]
mod open_extensions_tests {
    use super::open_extension_warnings;
    use farhelm_proto::open_extensions::WarningKind;

    /// Recognized document and log types need no risk message beside the field.
    #[test]
    fn safe_draft_has_no_warnings() {
        assert!(open_extension_warnings("md, txt, json, log").is_empty());
    }

    /// Each unrecognized type gets its own warning, even in an invalid draft.
    #[test]
    fn unsafe_draft_warns_per_type() {
        assert_eq!(
            open_extension_warnings("command, tar.gz, shell, COMMAND"),
            vec![
                ("command".into(), WarningKind::Other),
                ("shell".into(), WarningKind::Other)
            ]
        );
    }

    /// HTML-like final types share the browser-specific warning vocabulary.
    #[test]
    fn html_family_gets_specific_warning() {
        assert_eq!(
            open_extension_warnings("html, htm, xhtml, svg"),
            vec![
                ("html".into(), WarningKind::Html),
                ("htm".into(), WarningKind::Html),
                ("xhtml".into(), WarningKind::Html),
                ("svg".into(), WarningKind::Html),
            ]
        );
    }

    /// Repeated and empty tokens do not consume the policy's distinct-type cap.
    /// The warning scan must still reach each risky type after those tokens.
    #[test]
    fn duplicates_before_unsafe_types_do_not_hide_warnings() {
        let draft = format!("{}html, command", "md,".repeat(40));
        assert_eq!(
            open_extension_warnings(&draft),
            vec![
                ("html".into(), WarningKind::Html),
                ("command".into(), WarningKind::Other),
            ]
        );
    }
}

/// One radio choice writes only terminal link ownership, through the shared queue.
/// Its mounted terminals see the optimistic choice immediately; closing settings
/// cannot cancel the write or leave only the selected terminal using the new rule.
#[component]
fn TerminalLinkChoice(
    value: TerminalLinkClick,
    label: &'static str,
    help: &'static str,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut preferences = use_context::<SharedPreferences>();
    let selected = TerminalLinkClick::from_key(preferences.0.read().terminal_link_click.as_deref());
    let help_id = format!("terminal-link-{}-help", value.key());
    rsx! {
        div {
            label {
                class: "app-settings-choice",
                "data-tooltip": help,
                span { "{label}" }
                input {
                    r#type: "radio",
                    name: "terminal-link-click",
                    value: value.key(),
                    checked: selected == value,
                    aria_describedby: help_id.clone(),
                    onchange: move |_| {
                        preferences.0.write().terminal_link_click = Some(value.key().into());
                        store_preference(&base, PreferenceValue::TerminalLinkClick(value));
                    },
                }
            }
            p { id: help_id, class: "host-settings-help", "{help}" }
        }
    }
}

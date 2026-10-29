//! Group helm-wide and device choices without changing their owners.
//!
//! The helm-wide preferences include the host dialogs' permanent answers.
//! Update the shared client copy before queuing the sparse write:
//! the next add or removal in this window must see the answer immediately.
//! Other clients keep their loaded preferences until they reload, as usual.
//!
//! A desktop app whose updater runs adds one choice that is not a helm
//! preference: whether to install updates automatically. It belongs to that
//! app installation (SPEC.md "Session list"), so it is read from and written
//! to the app's own state file through the updater, never the helm.
//! Sound switches belong to this device's webview storage and remain live
//! even if a storage write fails; the sound asset owns those controls.
//! Appearance follows the same device boundary: its head asset owns the live
//! palette and native radios, including changes while Settings is closed.
//! The checkout folder and post-clone command are helm configuration, not
//! preferences: each explicitly saves validated text and shows its outcome.
//! Ntfy alerts follow that explicit-save contract too, with a secret that is
//! never part of the shared client snapshot or the queued preference writes.
//! Terminal frame coloring is a helm preference too: an open session sees the
//! new choice through the shared signal, while other clients reload to get it.
//! Automatic remote-host updates are a separate shared opt-in: only a committed
//! helm preference authorizes background work; the optimistic checkbox does not.
//! Native tunnel consent also requires an acknowledged save; its controller
//! owns the shared warning and revokes listeners when the switch turns off.
//! The desktop download-opening list uses an acknowledged save too: the helm
//! decides what opens, while this dialog warns about listed types it does not
//! recognize as safe.
//!
//! Section changes hide mounted panels rather than replacing their component
//! trees. Explicit-save fields keep their drafts and pending work, and device
//! assets initialize their inputs before their panels are visible.
//! Label/help groups keep each explanation with its own choice; they do not
//! combine preference writers or create a dialog-wide save lifecycle.

use dioxus::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

use crate::ApiBase;
use crate::api::{
    self, PreferenceValue, PreferenceWrite, TerminalLinkClick, restore_session_confirmations,
    store_preference,
};
use crate::app_updater::use_app_updater;
use crate::hosts::settings_dialog::HostDialogHeader;
use crate::list::SharedPreferences;
use crate::modal_dialog::{DialogFocus, DialogHandle, ModalDialog};

/// The app bar and trust bands close the same modal through its surviving owner.
pub(crate) const DIALOG_SELECTOR: &str = r#".app-settings-dialog[role="dialog"]"#;
/// Every Settings entry point returns to the gear after shared release and render.
pub(crate) const RETURN_FOCUS: &str =
    "document.querySelector('.app-settings-toggle')?.focus({ preventScroll: true });";

/// Stable panel keys preserve draft and focus ownership; labels are presentation.
/// The App entry still follows updater availability rather than platform alone.
const SETTINGS_SECTIONS: [(&str, &str); 8] = [
    ("sessions", "Sessions"),
    ("appearance", "Appearance"),
    ("hosts", "Hosts"),
    ("terminal", "Terminal"),
    ("checkouts", "Checkouts"),
    ("agent-trust", "Agent trust"),
    ("notifications", "Notifications"),
    ("app", "App"),
];

/// Reuse the shared decorative marks without giving navigation another focus stop.
/// Section keys remain the existing closed list; neither icons nor visible names
/// choose which mounted panel owns a draft or an outstanding save.
fn settings_section_icon(section: &str) -> Element {
    use crate::icons::{ActionGlyph, ActionIcon, BellIcon, FolderIcon};
    match section {
        "sessions" => rsx! { ActionIcon { glyph: ActionGlyph::Sessions } },
        "appearance" => rsx! { ActionIcon { glyph: ActionGlyph::Appearance } },
        "hosts" => rsx! { ActionIcon { glyph: ActionGlyph::Hosts } },
        "terminal" => rsx! { ActionIcon { glyph: ActionGlyph::Terminal } },
        "checkouts" => rsx! { FolderIcon {} },
        "agent-trust" => rsx! { ActionIcon { glyph: ActionGlyph::AgentTrust } },
        "notifications" => rsx! { BellIcon { loud: false } },
        "app" => rsx! { ActionIcon { glyph: ActionGlyph::App } },
        _ => unreachable!("Settings icons use only the declared section keys"),
    }
}

/// Hand focus to the newly revealed content after a section or narrow back click.
///
/// The narrow layout hides the activating button, so leaving focus there would
/// strand keyboard users in hidden content. Headings are programmatic targets,
/// not extra Tab stops; the list uses the same ordinary buttons on every engine.
fn focus_settings_content(section_list: bool) {
    let target = if section_list {
        ".app-settings-navigation button[aria-pressed=\"true\"]"
    } else {
        ".app-settings-panel:not([hidden]) > h3"
    };
    let target = serde_json::to_string(target).expect("a string always serializes");
    document::eval(&format!(
        r#"requestAnimationFrame(() => {{
            const dialog = document.querySelector('{DIALOG_SELECTOR}');
            if (!dialog) return;
            dialog.querySelector('.host-settings-body').scrollTop = 0;
            dialog.querySelector({target})?.focus({{ preventScroll: true }});
        }});"#
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

/// Repair native check state even when a refusal beats the optimistic render.
///
/// Dioxus does not rewrite an unchanged `checked` value. A checkbox or radio
/// that the browser changed before its event can therefore retain an unsaved
/// choice when the next render equals the previous one. Read the current DOM
/// snapshot when this eval runs, rather than capturing an older choice that
/// could overwrite another click while the renderer is catching up.
pub(crate) fn sync_preference_checks() {
    document::eval(&format!(
        "document.querySelectorAll('{DIALOG_SELECTOR} input[data-preference-checked]').forEach(input => {{ input.checked = input.dataset.preferenceChecked === 'true'; }});"
    ));
}

/// Change only the surviving shared field whose save this Settings control owns.
///
/// Rollback and corrective reads must not replace the whole preferences row:
/// unrelated controls may have changed while this request was outstanding.
fn apply_settings_preference(preferences: &mut api::Preferences, value: PreferenceValue) {
    match value {
        PreferenceValue::AutoUpdateRemoteHosts(value) => {
            preferences.auto_update_remote_hosts = Some(value);
        }
        PreferenceValue::NameSessionOnQuickLaunch(value) => {
            preferences.name_session_on_quick_launch = Some(value);
        }
        PreferenceValue::ColorTerminalFrames(value) => {
            preferences.color_terminal_frames = Some(value);
        }
        PreferenceValue::TerminalLinkClick(value) => {
            preferences.terminal_link_click = Some(value.key().into());
        }
        _ => unreachable!("only shared Settings choices use this rollback"),
    }
}

/// Keep optimistic shared settings honest even after their dialog closes.
///
/// The queue retains the last observed or acknowledged value, rather than the
/// previous click: that click may itself be an unsaved choice. Restore that
/// fallback immediately on failure, then refine it with one fresh field read
/// because a lost reply can follow a committed save. Neither path retries a PUT.
/// Only the alert belongs to Settings. The shared signal belongs to the longer
/// lived authenticated tree; a destroyed tree is left alone through try_write.
fn save_settings_preference(
    base: &str,
    mut preferences: SharedPreferences,
    value: PreferenceValue,
    mut error: Signal<Option<PreferenceWrite>>,
    mounted: Rc<Cell<bool>>,
) {
    let (write, completion) = api::observe_preference(base, value.clone());
    apply_settings_preference(&mut preferences.0.write(), value);
    error.set(None);
    let base = base.to_owned();
    dioxus::core::spawn_forever(async move {
        if completion.await || !write.is_current() {
            return;
        }
        let Some(fallback) = write.rollback_value() else {
            return;
        };
        {
            let Ok(mut shared) = preferences.0.try_write() else {
                return;
            };
            apply_settings_preference(&mut shared, fallback);
        }
        if mounted.get() {
            error.set(Some(write));
        }
        if let Ok(fetched) = api::fetch_preferences(&base).await
            && let Ok(mut shared) = preferences.0.try_write()
            && let Some(value) = write.reconcile(&fetched)
        {
            apply_settings_preference(&mut shared, value);
        }
    });
}

/// Group settings without giving a section change any save or discard semantics.
///
/// Writes use the ordinary preference queue. Restoring session confirmations
/// reports a failed save because a reload could otherwise silently restore the
/// opt-out. Naming, frame colors, remote updates and link clicks report and roll back a failed
/// newest choice; only their alert is dropped when the dialog closes. The host
/// confirmation choices retain their documented silent failure behavior.
/// Controls stay enabled, and closing the dialog must not cancel a queued write.
/// The checkout and ntfy fields are exceptions: their explicit saves expose validation
/// refusals rather than treating arbitrary text like a switch preference.
/// Tunnel consent is another exception: it is never optimistic or silently retried.
/// The desktop file-opening list also waits for an acknowledged save before
/// updating the shared preference; its warnings follow the unsaved draft.
/// Only the grouped body scrolls; both dismissal paths stay outside it and
/// share the caller's close callback and focus-return lifecycle.
/// Every mount starts in sessions, except the existing trust-band entry points.
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
    let auto_update_base = base.clone();
    let link_base = base.clone();
    let mut preferences = use_context::<SharedPreferences>();
    let setup = preferences.0.read().skip_host_setup_confirmation == Some(true);
    let remove = preferences.0.read().skip_host_remove_confirmation == Some(true);
    let skip_sessions = preferences.0.read().skip_session_confirmations == Some(true);
    // The positive naming preference defaults on for upgraded helms too;
    // treating an absent value like the confirmation opt-outs would disable it.
    let name_quick_launches = preferences.0.read().name_session_on_quick_launch != Some(false);
    let color_frames = preferences.0.read().color_terminal_frames == Some(true);
    let auto_update_hosts = preferences.0.read().auto_update_remote_hosts == Some(true);
    // Superseded writes also report false. Only the latest click may explain
    // its failure, including when an intervening click opted out again.
    let mut session_generation = use_signal(|| 0_u64);
    let mut session_error = use_signal(|| false);
    let naming_error = use_signal(|| None::<PreferenceWrite>);
    let frame_error = use_signal(|| None::<PreferenceWrite>);
    let auto_update_error = use_signal(|| None::<PreferenceWrite>);
    let link_error = use_signal(|| None::<PreferenceWrite>);
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let on_drop = mounted.clone();
    use_drop(move || on_drop.set(false));
    let naming_mounted = mounted.clone();
    let frame_mounted = mounted.clone();
    let auto_update_mounted = mounted.clone();
    // The radio components share a field and one adjacent alert. Registering in
    // their parent also gives every radio the same surviving rollback lifetime.
    let link_change = use_callback(move |value: TerminalLinkClick| {
        save_settings_preference(
            &link_base,
            preferences,
            PreferenceValue::TerminalLinkClick(value),
            link_error,
            mounted.clone(),
        );
    });
    use_effect(move || {
        let _ = preferences.0.read();
        sync_preference_checks();
    });
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
    let opens_trust = trust_focus.is_some();
    let mut selected = use_signal(move || {
        if opens_trust {
            "agent-trust"
        } else {
            "sessions"
        }
    });
    // Narrow windows start on the selected pane, not the section picker. This
    // affects presentation only: neither back nor selection unmounts a panel.
    let mut sections_open = use_signal(|| false);
    let has_updater = updater.is_some();

    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            ModalDialog {
                handle: DialogHandle::new(DIALOG_SELECTOR),
                focus: DialogFocus::remembered(
                    trust_focus.as_deref().unwrap_or("#settings-sessions input"),
                    Some(".app-settings-close"), false, None,
                ),
                class: "app-settings-dialog host-dialog-framed",
                role: "dialog",
                aria_modal: "true",
                aria_label: "Settings",
                tabindex: "-1",
                onmounted: move |_| {
                    if let Some(selector) = &trust_focus {
                        // The shared dialog focus helper deliberately prevents
                        // scrolling. A band's entry point promises its setting
                        // and explanation, so bring that section into view.
                        document::eval(
                            &format!(
                                "requestAnimationFrame(() => document.querySelector('{selector}')?.scrollIntoView({{block: 'start'}}))",
                            ),
                        );
                    }
                },
                on_escape: move |event: KeyboardEvent| {
                    if !event.is_composing() {
                        on_close.call(());
                    }
                },
                HostDialogHeader {
                    title: rsx! { "Settings" },
                    tile: rsx! {
                        crate::icons::SettingsIcon {}
                    },
                    summary: "For every host on this helm, and for this device",
                    dismiss_label: "Close settings",
                    dismiss_tooltip: "Close settings; unsaved fields are discarded",
                    on_dismiss: move |_| on_close.call(()),
                }
                div { class: if sections_open() { "dialog-body host-settings-body app-settings-sections-open" } else { "dialog-body host-settings-body" },
                    nav {
                        class: "app-settings-navigation",
                        aria_label: "Settings sections",
                        for (key, label) in SETTINGS_SECTIONS.iter().copied().filter(|(key, _)| *key != "app" || has_updater) {
                            button {
                                key: "{key}",
                                r#type: "button",
                                aria_pressed: selected() == key,
                                aria_controls: "settings-{key}",
                                "data-tooltip": "Show {label} settings",
                                onclick: move |_| {
                                    selected.set(key);
                                    sections_open.set(false);
                                    focus_settings_content(false);
                                },
                                span { class: "settings-section-icon", aria_hidden: "true",
                                    {settings_section_icon(key)}
                                }
                                span { "{label}" }
                            }
                        }
                    }
                    div { class: "app-settings-content",
                        button {
                            r#type: "button",
                            class: "app-settings-back",
                            "data-tooltip": "Choose a settings section",
                            onclick: move |_| {
                                sections_open.set(true);
                                focus_settings_content(true);
                            },
                            "‹ Settings"
                        }
                        section {
                            id: "settings-sessions",
                            class: "host-settings-section app-settings-panel",
                            aria_label: "Sessions",
                            hidden: selected() != "sessions",
                            h3 {
                                class: "host-settings-section-title",
                                tabindex: "-1",
                                "Sessions"
                            }
                            div { class: "setting-group",
                                label {
                                    class: "app-settings-choice setting-row",
                                    "data-tooltip": "Delete, replace, restart and close terminal tabs on the first click",
                                    span { "Skip session confirmations" }
                                    input {
                                        r#type: "checkbox",
                                        class: "control-switch",
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
                                p {
                                    id: "session-confirmations-help",
                                    class: "host-settings-help",
                                    "Delete, replace, restart and close terminal tabs on the first click. YOLO, host and trash confirmations still follow their own rules. This choice applies to this helm; other open clients pick it up when they reload."
                                }
                                if session_error() {
                                    p {
                                        class: "host-settings-help app-settings-error",
                                        role: "alert",
                                        "Confirmations may still be off for this helm. Try again."
                                    }
                                }
                            }
                            div { class: "setting-group",
                                label {
                                    class: "app-settings-choice setting-row",
                                    "data-tooltip": "Offer to name a quick-launched session while its agent starts",
                                    span { "Name sessions on quick launch" }
                                    input {
                                        r#type: "checkbox",
                                        class: "control-switch",
                                        checked: name_quick_launches,
                                        "data-preference-checked": name_quick_launches.to_string(),
                                        aria_describedby: "quick-launch-naming-help",
                                        onchange: move |event| {
                                            save_settings_preference(
                                                &naming_base,
                                                preferences,
                                                PreferenceValue::NameSessionOnQuickLaunch(event.checked()),
                                                naming_error,
                                                naming_mounted.clone(),
                                            );
                                        },
                                    }
                                }
                                p {
                                    id: "quick-launch-naming-help",
                                    class: "host-settings-help",
                                    "Offer to name a quick-launched session while its agent starts. Enter keeps its default name. This choice applies to this helm; other open clients pick it up when they reload."
                                }
                                if naming_error().is_some_and(|write| write.is_current()) {
                                    p { class: "host-settings-help app-settings-error", role: "alert",
                                        "Could not save whether to name quick-launched sessions. Try again."
                                    }
                                }
                            }
                            div { class: "setting-group",
                                label {
                                    class: "app-settings-choice setting-row",
                                    "data-tooltip": "Color each terminal's frame to match its host icon",
                                    span { "Color terminal frames by host" }
                                    input {
                                        r#type: "checkbox",
                                        class: "control-switch",
                                        checked: color_frames,
                                        "data-preference-checked": color_frames.to_string(),
                                        aria_describedby: "terminal-frame-color-help",
                                        onchange: move |event| {
                                            let color = event.checked();
                                            save_settings_preference(&frame_base, preferences,
                                                PreferenceValue::ColorTerminalFrames(color), frame_error,
                                                frame_mounted.clone());
                                        },
                                    }
                                }
                                p {
                                    id: "terminal-frame-color-help",
                                    class: "host-settings-help",
                                    "Match the thin frame around every session's terminal to its host icon color. The terminal background stays the same. This choice applies to this helm; other open clients pick it up when they reload."
                                }
                                if frame_error().is_some_and(|write| write.is_current()) {
                                    p { class: "host-settings-help app-settings-error", role: "alert",
                                        "Could not save whether to color terminal frames. Try again."
                                    }
                                }
                            }
                        }
                        section {
                            id: "settings-appearance",
                            class: "host-settings-section app-settings-panel",
                            aria_label: "Appearance",
                            hidden: selected() != "appearance",
                            h3 {
                                class: "host-settings-section-title",
                                tabindex: "-1",
                                "Appearance"
                            }
                            crate::appearance::AppearanceSettings {}
                        }
                        section {
                            id: "settings-hosts",
                            class: "host-settings-section app-settings-panel",
                            aria_label: "Hosts",
                            hidden: selected() != "hosts",
                            h3 {
                                class: "host-settings-section-title",
                                tabindex: "-1",
                                "Hosts"
                            }
                            div { class: "setting-group",
                                label {
                                    class: "app-settings-choice setting-row",
                                    "data-tooltip": "Update remote hosts as soon as an update is available, even with no window open; never update the helm's own machine",
                                    span { "Update remote hosts automatically" }
                                    input {
                                        r#type: "checkbox",
                                        class: "control-switch",
                                        checked: auto_update_hosts,
                                        "data-preference-checked": auto_update_hosts.to_string(),
                                        aria_describedby: "remote-host-auto-update-help",
                                        onchange: move |event| {
                                            save_settings_preference(&auto_update_base, preferences,
                                                PreferenceValue::AutoUpdateRemoteHosts(event.checked()), auto_update_error,
                                                auto_update_mounted.clone());
                                        },
                                    }
                                }
                                p { id: "remote-host-auto-update-help", class: "host-settings-help",
                                    "Update remote hosts as soon as an update is available, even with no window open. This never updates the helm's own machine. Other open clients pick up this choice when they reload."
                                }
                                if auto_update_error().is_some_and(|write| write.is_current()) {
                                    p { class: "host-settings-help app-settings-error", role: "alert",
                                        "Could not save whether to update remote hosts automatically. Try again."
                                    }
                                }
                            }
                            p { class: "host-settings-help",
                                "The host choices apply to every host on this helm. Other open clients pick them up when they reload."
                            }
                            div { class: "setting-group",
                                label {
                                    class: "app-settings-choice setting-row",
                                    "data-tooltip": "Set up a new host as soon as it has been checked, without showing the plan first",
                                    span { "Set up new hosts without asking" }
                                    input {
                                        r#type: "checkbox",
                                        class: "control-switch",
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
                            div { class: "setting-group",
                                label {
                                    class: "app-settings-choice setting-row",
                                    "data-tooltip": "Forget a host as soon as you choose remove, without asking; its sessions keep running",
                                    span { "Remove hosts without asking" }
                                    input {
                                        r#type: "checkbox",
                                        class: "control-switch",
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
                            id: "settings-terminal",
                            class: "host-settings-section app-settings-panel",
                            aria_label: "Terminal",
                            hidden: selected() != "terminal",
                            h3 {
                                class: "host-settings-section-title",
                                tabindex: "-1",
                                "Terminal"
                            }
                            p { class: "host-settings-help",
                                "Clicking a web link in a terminal: this choice applies to every client of this helm. Other open clients pick it up when they reload. This web-link choice does not change file downloads."
                            }
                            div { class: "segmented-control terminal-link-choices", role: "radiogroup", aria_label: "Terminal web links",
                                TerminalLinkChoice {
                                    value: TerminalLinkClick::Intercept,
                                    label: "Open in Farhelm, don't pass the click on",
                                    help: "Farhelm opens the link once. The program does not receive that click or a drag starting there.",
                                    on_change: link_change,
                                }
                                TerminalLinkChoice {
                                    value: TerminalLinkClick::Both,
                                    label: "Open in Farhelm and also pass the click on",
                                    help: "Use this when the program needs link clicks. A program that opens links itself may open a second tab.",
                                    on_change: link_change,
                                }
                                TerminalLinkChoice {
                                    value: TerminalLinkClick::Program,
                                    label: "Pass the click on only",
                                    help: "Let the program handle links. Farhelm does not open them; without mouse reporting, clicking a link does nothing.",
                                    on_change: link_change,
                                }
                            }
                            if link_error().is_some_and(|write| write.is_current()) {
                                p { class: "host-settings-help app-settings-error", role: "alert",
                                    "Could not save how terminal links are clicked. Try again."
                                }
                            }
                            NativeTunnelSettings {}
                            NativeOpenExtensionsSettings {}
                        }
                        section {
                            id: "settings-checkouts",
                            class: "host-settings-section app-settings-panel",
                            aria_label: "Checkouts",
                            hidden: selected() != "checkouts",
                            h3 {
                                class: "host-settings-section-title",
                                tabindex: "-1",
                                "Checkouts"
                            }
                            crate::checkout_root::CheckoutRootField {
                                note: "Managed checkouts are cloned into this folder on every host. ~ means each host's own home folder. A missing folder is created on first use. A host given its own folder from the command line keeps that folder. Save an empty field to clear it.",
                                on_saved: move |_| {},
                            }
                            crate::checkout_post_clone::CheckoutPostCloneField {}
                            crate::checkout_spare::CheckoutSpareSettings {}
                        }
                        section {
                            id: "settings-agent-trust",
                            class: "host-settings-section app-settings-panel",
                            aria_label: "Agent trust",
                            hidden: selected() != "agent-trust",
                            h3 {
                                class: "host-settings-section-title",
                                tabindex: "-1",
                                "Agent trust"
                            }
                            crate::claude_trust::ClaudeTrustSettings {}
                            crate::workspace_trust::WorkspaceTrustSettings { agent: crate::workspace_trust::TrustAgent::Codex }
                            crate::workspace_trust::WorkspaceTrustSettings { agent: crate::workspace_trust::TrustAgent::Muse }
                            crate::workspace_trust::WorkspaceTrustSettings { agent: crate::workspace_trust::TrustAgent::Pi }
                        }
                        section {
                            id: "settings-notifications",
                            class: "host-settings-section app-settings-panel",
                            aria_label: "Notifications",
                            hidden: selected() != "notifications",
                            h3 {
                                class: "host-settings-section-title",
                                tabindex: "-1",
                                "Notifications"
                            }
                            crate::sounds::SoundSettings {}
                            crate::ntfy_settings::NtfySettings {}
                        }
                        if let Some(updater) = updater {
                            section {
                                id: "settings-app",
                                class: "host-settings-section app-settings-panel",
                                aria_label: "App",
                                hidden: selected() != "app",
                                h3 {
                                    class: "host-settings-section-title",
                                    tabindex: "-1",
                                    "App"
                                }
                                p { class: "host-settings-help",
                                    "This one belongs to this Farhelm app on this Mac."
                                }
                                div { class: "setting-group",
                                    label {
                                        class: "app-settings-choice setting-row",
                                        "data-tooltip": "Install updates automatically: check for a new Farhelm release when the app starts and about once a day, and install it in the background",
                                        span { "Install updates automatically" }
                                        input {
                                            r#type: "checkbox",
                                            class: "control-switch",
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
                }
                crate::modal_dialog::DialogFooter { class: "host-settings-actions",
                    // Only the desktop app shows the file-types field, so only
                    // its footer names it.
                    span { class: "dialog-footer-hint",
                        if cfg!(native_desktop) {
                            "Most choices save immediately; ntfy alerts, checkout fields and file types use Save."
                        } else {
                            "Most choices save immediately; ntfy alerts and checkout fields use Save."
                        }
                    }
                    crate::modal_dialog::DialogActions {
                        button {
                            r#type: "button",
                            class: "btn btn-neutral app-settings-close",
                            "data-tooltip": "Close settings; unsaved fields are discarded",
                            onclick: move |_| on_close.call(()),
                            "Close"
                        }
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
        rsx! {
            crate::tunnels::TunnelSettings {}
        }
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
        rsx! {
            OpenExtensionsSettings {}
        }
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
            label {
                r#for: "open-extensions-input",
                class: "open-extensions-label",
                "File types to open with {modifier}-click"
            }
            input {
                id: "open-extensions-input",
                r#type: "text",
                value: "{draft}",
                readonly: saving(),
                aria_disabled: saving(),
                autocomplete: "off",
                spellcheck: "false",
                autocorrect: "off",
                autocapitalize: "none",
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
                div {
                    class: "open-extensions-warning",
                    role: "alert",
                    key: "{extension}",
                    p { class: "open-extensions-warning-heading",
                        span { aria_hidden: "true", "⚠" }
                        " {extension}: "
                        if kind == WarningKind::Html {
                            "An agent-written page may run in your browser"
                        } else {
                            "No protection from the default app"
                        }
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
                r#type: "button",
                class: "btn btn-neutral",
                aria_disabled: saving(),
                "data-tooltip": "Save the file types this helm may open",
                onclick: move |_| {
                    if saving() {
                        return;
                    }
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
                        if !mounted.get() {
                            return;
                        }
                        saving.set(false);
                        if result.is_ok() {
                            draft.set(value);
                        }
                        outcome.set(Some(result));
                    });
                },
                if saving() {
                    "Saving…"
                } else {
                    "Save file types"
                }
            }
            if let Some(result) = outcome() {
                match result {
                    Ok(()) => rsx! {
                        p { class: "checkout-root-success", role: "status", "File types saved" }
                    },
                    Err(reason) => rsx! {
                        p { class: "checkout-root-error", role: "alert", "{reason}" }
                    },
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
/// The shared segment paints the native input's checked/focus state; its visible
/// explanation remains the input's description, separate from its stable name.
#[component]
fn TerminalLinkChoice(
    value: TerminalLinkClick,
    label: &'static str,
    help: &'static str,
    on_change: EventHandler<TerminalLinkClick>,
) -> Element {
    let preferences = use_context::<SharedPreferences>();
    let selected = TerminalLinkClick::from_key(preferences.0.read().terminal_link_click.as_deref());
    let help_id = format!("terminal-link-{}-help", value.key());
    rsx! {
        label { class: "app-settings-choice segment-radio", "data-tooltip": help,
            input {
                r#type: "radio",
                name: "terminal-link-click",
                value: value.key(),
                checked: selected == value,
                "data-preference-checked": (selected == value).to_string(),
                aria_label: label,
                aria_describedby: help_id.clone(),
                onchange: move |_| {
                    on_change.call(value);
                },
            }
            span {
                span { "{label}" }
                span { id: help_id, class: "terminal-link-description", "{help}" }
            }
        }
    }
}

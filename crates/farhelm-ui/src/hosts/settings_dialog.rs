//! The host settings dialog: connection details, host identity appearance,
//! and permission choices, in a modal over the whole page.
//!
//! It replaced a panel that expanded inline inside the host's sidebar row.
//! The sidebar is too narrow for that: the destination was cut off, the edit
//! buttons crowded the values, and the YOLO explanation wrapped awkwardly.
//! The dialog shows every value in full and edits one field at a time, in
//! place, saving each change on its own (there is no dialog-wide Save).
//! A host-specific header identifies the peer and its reported connection
//! state, while Connection, Appearance and Permissions group
//! the controls. The refresh has its own CSS class because the base dialog styles also serve
//! unrelated modals. The switches remain native checkboxes, including their
//! write-disable, refused-write reset and keyboard contracts.
//! Each permission's label, explanation and outcome form one spacing group,
//! so adjacent independent grants do not look like one setting.
//!
//! What this module owns is presentation and focus. The state it renders
//! (which host is open, which field is being edited, the draft, the outcome
//! of the last write) belongs to `HostsPanel`, which also performs every
//! write through its ordinary `run` path. The dialog is rendered by the panel
//! rather than by the row for the reason `HostDestinationForm` records: a
//! host mutation landing rebuilds the rows, and anything a row owns goes
//! with it.
//!
//! Focus follows the app's other modals. On open, focus moves to the dialog's
//! first control and `modal_isolation` makes the rest of the page inert, so
//! nothing behind the backdrop (a session's terminal in particular) can be
//! focused or typed into. Escape cancels an open field edit, or otherwise
//! closes the dialog; clicking the backdrop does nothing, as in the rename
//! dialog. On close, focus returns to the host row's "⋯"
//! toggle the dialog was opened from.
//!
//! Pending writes can temporarily remove the current focus target:
//!
//! - A write natively disables the control that started it (the field, its
//!   save button, the YOLO checkbox) for as long as the request holds the
//!   page's operation token, and a disabled element drops focus to `body`.
//!   The session launcher transfers focus to its surviving primary action
//!   before disabling fields. Here the controls are the hosts panel's shared
//!   ones, so instead the isolation's Escape goes to the field's own cancel
//!   button when a field is open (natively disabled mid-save, so Escape then
//!   does nothing, as the dialog's own handler would), and focus is put back
//!   on the field or checkbox once the write ends (`refocus_after_write`).
//!
//! The shared modal boundary wraps Tab among enabled, visible controls and
//! keyboard scroll regions. A body with no usable control gains its own entry
//! stop; write recovery later yields to explicit navigation or another usable
//! owner before restoring a control.

use dioxus::prelude::*;

use super::{EditField, HostDestinationForm, gui_host_name, phase_class, phase_display_label};
use crate::icons::HostMark;
use crate::modal_dialog::{
    DialogFocus, DialogHandle, FocusTarget, ModalDialog, RecoveryDelay, RecoveryFocus, ScrollRegion,
};
use crate::ops::ConfirmToken;
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::{Host, HostId, HostKind, HostPhase};

/// The selector the isolation and the focus scripts find the dialog by.
const DIALOG_SELECTOR: &str = r#".host-settings-dialog[role="dialog"]"#;
/// Preserve the existing generic release selector for every host row close path.
pub(crate) const DIALOG: DialogHandle = DialogHandle::new(DIALOG_SELECTOR);
/// Add form and row setup share a selector while retaining different mount focus.
pub(crate) const ADD_DIALOG_SELECTOR: &str = r#".host-add-dialog[role="dialog"]"#;
/// The panel and nested SSH help share the mounted Add form's boundary.
pub(crate) const ADD_DIALOG: DialogHandle = DialogHandle::new(ADD_DIALOG_SELECTOR);
/// Uninstall's root installs independently of the generic host close handle.
pub(crate) const UNINSTALL_DIALOG_SELECTOR: &str = r#".host-uninstall-dialog[role="dialog"]"#;

/// One setting the dialog can change, as the key its last write's outcome is
/// filed under. A superset of [`EditField`]: checkboxes and appearance write through
/// the same `run` path but have no text editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsField {
    Destination,
    Alias,
    YoloWithoutAsking,
    CommandsWithoutAsking,
    LinksWithoutAsking,
    LocalLinksWithoutAsking,
    Appearance,
}

/// One explicit appearance choice; the panel supplies the other current word.
/// Carrying a complete render-time pair would let a queued click restore a
/// value from before the preceding write's confirmed response.
#[derive(Clone, Copy)]
pub(super) enum AppearanceChoice {
    Icon(farhelm_proto::host_appearance::HostIcon),
    Color(farhelm_proto::host_appearance::HostColor),
}

impl From<EditField> for SettingsField {
    fn from(field: EditField) -> Self {
        match field {
            EditField::Destination => SettingsField::Destination,
            EditField::Alias => SettingsField::Alias,
        }
    }
}

/// What the dialog's last write left behind, shown under the setting it
/// changed. The same two outcomes the row distinguishes (see
/// `HostRowActivity`): a refusal, or a change that committed but whose reply
/// this build could not decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FieldOutcome {
    Error(String),
    Warning(String),
}

/// Place only a settings write's outcome under the field that produced it.
/// Menu action refusals have their own dialogs, so a host-level line with no
/// `last_write` no longer has a meaningful place in Settings.
pub(super) fn field_outcome(
    last_write: Option<SettingsField>,
    error: Option<String>,
    warning: Option<String>,
) -> Option<(SettingsField, FieldOutcome)> {
    let owner = last_write?;
    let outcome = error
        .map(FieldOutcome::Error)
        .or(warning.map(FieldOutcome::Warning))?;
    Some((owner, outcome))
}

/// Describe the originating row without owning release or return scheduling.
///
/// A removed host has no toggle. Shared focus treats that missing destination
/// as a no-op; it must not choose another row or the terminal instead.
pub(crate) fn row_focus(id: HostId) -> FocusTarget {
    FocusTarget::new(
        format!(r#"[data-host-id="{id}"] .host-row-menu"#),
        None,
        false,
    )
}

/// A refused Remove leaves its confirmation mounted. Focus its safe exit
/// after the busy controls re-enable so the refusal can be dismissed with
/// Enter or Escape without reaching the page behind the dialog.
pub(crate) fn focus_remove_refusal() {
    document::eval(
        "requestAnimationFrame(() => document.querySelector('.host-remove-dialog .host-settings-close')?.focus({ preventScroll: true }))",
    );
}

/// Put focus back inside the dialog after a write ends, if the write lost it.
///
/// A control that is disabled while a write runs drops focus to `body` (see
/// the module doc); that is usually the control that started the write, but
/// any page-wide operation disables the dialog's controls the same way. When
/// it ends, focus goes back to the control that last had it, if that is
/// still in the dialog and usable; otherwise to the open field (a refused
/// save, so the user can correct the draft), then the YOLO checkbox. It runs
/// two frames late so that `focus_edit_button`, which handles a field that
/// closed on success, gets there first; a usable current owner, including an
/// approval answer outside this dialog, keeps focus.
/// The busy body's temporary tab stop is not a usable return target after
/// saving ends, even if the engine has not blurred it yet. Test both current
/// and remembered focus against the shared usable-owner contract, including
/// deliberate programmatic headings and independent approval answers.
///
/// `reset_checkbox` is set after a refused checkbox toggle. That leaves the
/// box showing the user's click while the setting never changed, and since
/// the rendered value did not change either, nothing would redraw it; the
/// reset puts both boxes back to the settings the page currently renders (see
/// [`reset_checkboxes_js`]).
fn refocus_after_write(reset_checkbox: bool) {
    let dialog = serde_json::to_string(DIALOG_SELECTOR).expect("a string always serializes");
    let reset = if reset_checkbox {
        reset_checkboxes_js()
    } else {
        String::new()
    };
    let recovery = settings_recovery().script();
    document::eval(&format!(
        r#"requestAnimationFrame(() => requestAnimationFrame(() => {{
            const dialog = document.querySelector({dialog});
            if (!dialog) return;
            {reset}
            {recovery}
        }}))"#
    ));
}

/// Let explicit field navigation win before the existing remembered fallback.
fn settings_recovery() -> RecoveryFocus {
    RecoveryFocus::new(
        ".host-destination-input",
        Some(".host-yolo-without-asking-toggle"),
        true,
        RecoveryDelay::TwoFrames,
    )
}

/// Put focus back on a field's edit button once its editor has closed.
///
/// The editor replaces that button while it is open, so when it closes (a
/// cancel, or a save the helm accepted) the element that had focus is gone
/// and focus would fall to `body`. The isolation would then swallow the next
/// keystroke to recover it; landing on the button the user started from
/// avoids that and keeps keyboard use continuous.
fn focus_edit_button(field: EditField) {
    let selector = match field {
        EditField::Destination => ".host-settings-dialog .host-edit",
        EditField::Alias => ".host-settings-dialog .host-alias",
    };
    let selector = serde_json::to_string(selector).expect("a string always serializes");
    document::eval(&format!(
        "requestAnimationFrame(() => document.querySelector({selector})?.focus({{ preventScroll: true }}))"
    ));
}

/// JavaScript, run with `dialog` in scope, that sets all permission checkboxes' shown
/// state back to the settings the page currently renders.
///
/// Each value is read from its input's `data-` attribute when the script
/// runs, not baked in when it is built: a host-list refresh landing in
/// between (another client toggling the same host) must win over a reset that
/// was queued against the older value. All boxes are reset whichever one was
/// refused, because resetting one that already shows its setting is a no-op.
fn reset_checkboxes_js() -> String {
    "const yolo = dialog.querySelector('.host-yolo-without-asking-toggle'); \
     if (yolo) yolo.checked = yolo.dataset.yoloWithoutAsking === 'true'; \
     const commands = dialog.querySelector('.host-commands-without-asking-toggle'); \
     if (commands) commands.checked = commands.dataset.commandsWithoutAsking === 'true'; \
     for (const input of dialog.querySelectorAll('[data-browser-permission]')) \
         input.checked = input.dataset.browserPermission === 'true';"
        .to_string()
}

/// Restore the shown settings after a refused checkbox write or start.
///
/// The hosts panel refuses to start a write while another operation holds
/// the page's lock; the click has already flipped the box by then, and with
/// no write there is no end of one to trigger `refocus_after_write`. A fast
/// failed write can also settle before that effect ever observes busy, so
/// its result calls this directly rather than relying on an intermediate render.
pub(super) fn reset_checkboxes() {
    let dialog = serde_json::to_string(DIALOG_SELECTOR).expect("a string always serializes");
    document::eval(&format!(
        "(() => {{ const dialog = document.querySelector({dialog}); if (!dialog) return; {} }})()",
        reset_checkboxes_js()
    ));
}

/// The explanation under the YOLO checkbox, for its current state.
///
/// Both sentences start by saying what YOLO means, because this is where a
/// user decides about it and cannot be assumed to know the term.
fn yolo_help(yolo_without_asking: bool) -> &'static str {
    if yolo_without_asking {
        "YOLO bypasses an agent's safety checks. Unclassified launches on the agent's own setting count as YOLO too. \
         These sessions start on this host without asking you to confirm."
    } else {
        "YOLO bypasses an agent's safety checks. Unclassified launches on the agent's own setting count as YOLO too. \
         Farhelm asks you to confirm each of these launches on this host."
    }
}

/// The explanation under the "run farhelm commands" checkbox, for its current
/// state.
///
/// Says what the commands do (an agent in one of this host's sessions acting on
/// the fleet) because the label alone does not, and the "on" text names what
/// turning it on gives away. Host changes remain a per-request decision in
/// either state; this setting cannot grant an agent control over hosts.
fn commands_help(commands_without_asking: bool) -> &'static str {
    if commands_without_asking {
        "Agents in this host's sessions start, stop, restart, rename, clone and delete sessions on any \
         host, and edit and delete launch templates, without asking you. Changes to hosts always need \
         your approval for each request."
    } else {
        "When an agent in one of this host's sessions asks Farhelm to start, stop, restart, rename, \
         clone or delete a session, or to edit or delete a launch template, Farhelm asks you first. \
         Changes to hosts always need your approval for each request."
    }
}

/// Identify the host and the question without owning any dialog lifecycle.
///
/// Registered hosts retain their own mark, status and reported version. An
/// add flow has no host yet, so it uses the default host mark. A supplied
/// tile replaces that mark for an application-wide dialog; it changes only
/// the presentation, leaving host identity and closing behavior to the caller.
/// A supplied summary replaces the status line, with or without a host;
/// callers reporting a refusal can put that summary in the danger tone.
/// Titles remain caller-rendered markup so peer values keep
/// their escaping and direction isolation. Dismiss runs the caller's cancel
/// path; this header must not decide when an operation permits cancellation.
/// The shared DialogHeader receives only presentation elements: host facts
/// and the caller-owned close control remain this adapter's responsibility.
/// An unknown host kind with no mark contributes no empty icon tile.
#[component]
pub(crate) fn HostDialogHeader(
    host: Option<Host>,
    tile: Option<Element>,
    title: Element,
    summary: Option<String>,
    summary_danger: Option<bool>,
    dismiss_label: String,
    dismiss_tooltip: String,
    on_dismiss: EventHandler<()>,
) -> Element {
    // Only a peer's actual report can supply a version. Executable paths and
    // the helm's own version are not evidence of what this host is running.
    let reported_version = host.as_ref().and_then(|host| match &host.state {
        HostPhase::Connected { build_version, .. } => Some(build_version.clone()),
        HostPhase::VersionSkew { peer_build, .. } => Some(peer_build.clone()),
        _ => None,
    });
    let tile = tile.or_else(|| match host.as_ref() {
        Some(host) => match host.kind {
            HostKind::Local => {
                Some(rsx! { HostMark { icon: host.icon, color: host.color, local: true } })
            }
            HostKind::Ssh => Some(rsx! { HostMark { icon: host.icon, color: host.color } }),
            HostKind::Unrecognized => None,
        },
        None => Some(rsx! { HostMark {} }),
    });
    // Only this adapter knows host phases and reported versions. The shared
    // frame receives their already separated, escaped presentation elements.
    let summary = summary.map(|summary| rsx! { "{summary}" }).or_else(|| {
        host.map(|host| {
            rsx! {
                span { class: "host-status {phase_class(&host.state)}",
                    span { class: "status-dot", "aria-hidden": "true" }
                    "{phase_display_label(&host.state)}"
                }
                if let Some(version) = reported_version {
                    span { class: "host-settings-version",
                        "Farhelm "
                        span { class: "peer-value", dir: "ltr", "{version.escaped()}" }
                    }
                }
            }
        })
    });
    rsx! {
        crate::modal_dialog::DialogHeader {
            title,
            tile,
            summary,
            summary_class: if summary_danger.unwrap_or(false) { "host-settings-summary-danger" } else { "" },
            close: rsx! { button {
                r#type: "button",
                class: "dialog-close host-settings-dismiss",
                aria_label: "{dismiss_label}",
                "data-tooltip": "{dismiss_tooltip}",
                onclick: move |_| on_dismiss.call(()),
                "×"
            } },
        }
    }
}

/// Edit one host's settings without giving a refresh ownership of its draft.
///
/// The panel owns every value and write handler so a host-list refresh can
/// update the header and controls without discarding an editor or its outcome.
/// Grouping and switch styling do not introduce a dialog-wide save: each
/// change still uses the panel's operation token and existing focus recovery.
#[component]
pub(super) fn HostSettingsDialog(
    host: Host,
    /// Whether writes are disabled right now: this host's own write or
    /// provisioning run, or any other operation holding the page's token.
    busy: bool,
    /// The field whose editor is open, if any.
    editing: Option<EditField>,
    /// This host's last settings-write outcome, attributed to its field.
    outcome: Option<(SettingsField, FieldOutcome)>,
    /// The open editor's draft, owned by the panel so a refresh cannot drop it.
    draft: Signal<String>,
    on_edit_start: EventHandler<(HostId, EditField, String)>,
    on_edit_submit: EventHandler<(HostId, EditField, String)>,
    on_edit_cancel: EventHandler<()>,
    on_yolo_without_asking: EventHandler<(HostId, bool)>,
    on_commands_without_asking: EventHandler<(HostId, bool)>,
    /// Category and value travel together; the handler attributes failures to that exact toggle.
    on_browser_permission: EventHandler<(HostId, bool, bool)>,
    on_appearance: EventHandler<(HostId, AppearanceChoice)>,
    on_close: EventHandler<()>,
) -> Element {
    let id = host.id;
    let shown_name = gui_host_name(host.name.raw(), host.kind.is_this_machine());
    // Same gates the row's panel used: a destination exists only for a kind
    // the registry manages, and the alias editor only against a helm that
    // sent the `alias` key at all (see `Host.alias`), whatever the kind.
    let manageable = host.kind.is_manageable();
    let alias_supported = host.alias.is_some();
    let destination = host.destination.clone().unwrap_or_default();
    let alias = host.alias.clone().flatten();

    // Recover focus a write's disabled control dropped; see
    // `refocus_after_write`.
    let mut last_busy = use_signal(|| busy);
    let checkbox_refused = matches!(
        &outcome,
        Some((
            SettingsField::YoloWithoutAsking
                | SettingsField::CommandsWithoutAsking
                | SettingsField::LinksWithoutAsking
                | SettingsField::LocalLinksWithoutAsking,
            FieldOutcome::Error(_)
        ))
    );
    use_effect(use_reactive((&busy,), move |(busy,)| {
        let previous = *last_busy.peek();
        last_busy.set(busy);
        if previous && !busy {
            refocus_after_write(checkbox_refused);
        }
    }));

    // Refocus the edit button of a field whose editor just closed; see
    // `focus_edit_button`.
    let mut last_editing = use_signal(|| editing);
    use_effect(use_reactive((&editing,), move |(editing,)| {
        let previous = *last_editing.peek();
        if editing.is_none()
            && let Some(field) = previous
        {
            focus_edit_button(field);
        }
        last_editing.set(editing);
    }));

    let outcome_for = move |field: SettingsField| -> Element {
        match &outcome {
            Some((owner, FieldOutcome::Error(text))) if *owner == field => rsx! {
                PeerLine {
                    class: "action-error host-error host-settings-outcome",
                    parts: vec![DetailPart::peer(text.clone())],
                }
            },
            Some((owner, FieldOutcome::Warning(text))) if *owner == field => rsx! {
                PeerLine {
                    class: "host-warning host-settings-outcome",
                    parts: vec![DetailPart::peer(text.clone())],
                }
            },
            _ => rsx! {},
        }
    };

    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            ModalDialog {
                handle: DIALOG,
                // The earlier quiet header Close is not the first editable
                // setting. Keep the original target order and focus memory.
                focus: DialogFocus::remembered(
                    ".host-edit, .host-alias, .host-yolo-without-asking-toggle, .host-settings-close",
                    Some(".host-cancel-edit, .host-settings-close"), false, None,
                ),
                class: "host-settings-dialog host-settings-refreshed",
                role: "dialog",
                aria_modal: "true",
                aria_label: "host settings · {shown_name}",
                // Programmatically focusable only, for the isolation's
                // keydown net and the shared initial-focus policy's no-control
                // fallback; `-1` keeps it out of the Tab order.
                tabindex: "-1",
                on_escape: move |evt: KeyboardEvent| {
                    // An IME uses Escape to dismiss its candidates; treating
                    // that as cancel would throw away the draft mid-word.
                    if evt.is_composing() {
                        return;
                    }
                    if editing.is_some() {
                        // Matches the editor's own cancel button, which is
                        // disabled while its save is in flight.
                        if !busy {
                            on_edit_cancel.call(());
                        }
                    } else {
                        on_close.call(());
                    }
                },
                HostDialogHeader {
                    host: host.clone(),
                    title: rsx! { span { class: "peer-value", dir: "ltr", "{shown_name}" } },
                    dismiss_label: "close host settings",
                    dismiss_tooltip: "close: done with this host's settings",
                    on_dismiss: move |_| on_close.call(()),
                }
                ScrollRegion { class: "dialog-body host-settings-body",
                handle: DIALOG, selector: ".host-settings-body", name: "host settings",
                recovery: settings_recovery(),
                if manageable || alias_supported {
                section { class: "host-settings-section", aria_label: "Connection",
                    h3 { class: "host-settings-section-title", "Connection" }
                if manageable {
                    div { class: "host-settings-row", "data-setting": "destination",
                        span { class: "host-settings-label", "destination" }
                        if editing == Some(EditField::Destination) {
                            HostDestinationForm {
                                draft,
                                busy,
                                alias: false,
                                on_submit: move |value| on_edit_submit.call((id, EditField::Destination, value)),
                                on_cancel: move |_| on_edit_cancel.call(()),
                            }
                        } else {
                            span { class: "host-settings-value peer-value", dir: "ltr",
                                "{destination.escaped()}"
                            }
                            button {
                                r#type: "button",
                                class: "btn btn-neutral host-edit",
                                aria_label: "edit destination",
                                "data-tooltip": "edit: change the ssh destination Farhelm uses for this host",
                                disabled: busy || editing.is_some(),
                                onclick: {
                                    // These are authenticated registry settings, not host advertisements.
                                    // Keep the existing raw editor seed so saving untouched data round-trips.
                                    let destination = destination.raw().to_owned();
                                    move |_| on_edit_start.call((id, EditField::Destination, destination.clone()))
                                },
                                "edit"
                            }
                        }
                        {outcome_for(SettingsField::Destination)}
                    }
                }
                if alias_supported {
                    div { class: "host-settings-row", "data-setting": "alias",
                        span { class: "host-settings-label", "alias" }
                        if editing == Some(EditField::Alias) {
                            HostDestinationForm {
                                draft,
                                busy,
                                alias: true,
                                on_submit: move |value| on_edit_submit.call((id, EditField::Alias, value)),
                                on_cancel: move |_| on_edit_cancel.call(()),
                            }
                        } else {
                            span { class: "host-settings-value peer-value", dir: "ltr",
                                "{alias.as_ref().map_or_else(|| display_peer(\"none\"), |alias| alias.escaped())}"
                            }
                            button {
                                r#type: "button",
                                class: "btn btn-neutral host-alias",
                                aria_label: "edit alias",
                                "data-tooltip": "edit: change the name this host is shown under",
                                disabled: busy || editing.is_some(),
                                onclick: {
                                    // As with destination, the editor owns the user's exact stored bytes.
                                    let alias = alias.as_ref().map(|alias| alias.raw().to_owned()).unwrap_or_default();
                                    move |_| on_edit_start.call((id, EditField::Alias, alias.clone()))
                                },
                                "edit"
                            }
                        }
                        {outcome_for(SettingsField::Alias)}
                    }
                }
                }
                }
                if host.kind.supports_appearance() {
                    section { class: "host-settings-section host-settings-appearance", aria_label: "Appearance",
                        h3 { class: "host-settings-section-title", "Appearance" }
                        span { class: "host-settings-label", "icon" }
                        div { class: "host-appearance-icons", role: "group", aria_label: "icon",
                            for icon in farhelm_proto::host_appearance::HostIcon::ALL {
                                button {
                                    r#type: "button", class: "host-icon-choice",
                                    aria_label: icon.as_str(), aria_pressed: host.icon == icon,
                                    "data-tooltip": "icon: {icon.as_str()}",
                                    disabled: busy || editing.is_some(),
                                    onclick: move |_| on_appearance.call((id, AppearanceChoice::Icon(icon))),
                                    HostMark { icon, color: host.color, local: host.kind.is_this_machine() }
                                }
                            }
                        }
                        span { class: "host-settings-label", "color" }
                        div { class: "host-appearance-colors", role: "group", aria_label: "color",
                            for color in farhelm_proto::host_appearance::HostColor::ALL {
                                button {
                                    r#type: "button", class: "host-color-choice",
                                    aria_label: color.as_str(), aria_pressed: host.color == color,
                                    "data-tooltip": "color: {color.as_str()}",
                                    "data-host-color": color.as_str(),
                                    disabled: busy || editing.is_some(),
                                    onclick: move |_| on_appearance.call((id, AppearanceChoice::Color(color))),
                                    span { "aria-hidden": "true" }
                                }
                            }
                            span { class: "host-color-name", "{host.color.as_str()}" }
                        }
                        div { class: "host-appearance-preview",
                            span { class: "host-settings-help", "in the session list" }
                            div { class: "host-appearance-preview-row",
                                span { class: "status-dot", "aria-hidden": "true" }
                                span { class: "session-locality-slot", HostMark { icon: host.icon, color: host.color, local: host.kind.is_this_machine() } }
                                span { "session" }
                            }
                        }
                        {outcome_for(SettingsField::Appearance)}
                    }
                }
                section { class: "host-settings-section", aria_label: "Permissions",
                    h3 { class: "host-settings-section-title", "Permissions" }
                div { class: "host-settings-yolo setting-group",
                    label { class: "host-yolo-without-asking setting-row",
                        "data-tooltip": "start YOLO sessions on this host without asking first",
                        span { "Start YOLO sessions without asking" }
                        input {
                            r#type: "checkbox",
                            class: "host-yolo-without-asking-toggle control-switch",
                            checked: host.yolo_without_asking,
                            "data-yolo-without-asking": "{host.yolo_without_asking}",
                            // One setting at a time: a toggle while a field
                            // is open would replace that field's refusal with
                            // its own outcome while the draft stays open.
                            disabled: busy || editing.is_some(),
                            onchange: move |event| on_yolo_without_asking.call((id, event.checked())),
                        }
                    }
                    p { class: "host-settings-help", "{yolo_help(host.yolo_without_asking)}" }
                    {outcome_for(SettingsField::YoloWithoutAsking)}
                }
                div { class: "host-settings-commands",
                    div { class: "setting-group",
                        label { class: "host-commands-without-asking setting-row",
                            "data-tooltip": "let agents in this host's sessions start, stop, change and delete sessions and templates without asking first",
                            span { "Run farhelm commands without asking" }
                            input {
                                r#type: "checkbox",
                                class: "host-commands-without-asking-toggle control-switch",
                                checked: host.commands_without_asking,
                                "data-commands-without-asking": "{host.commands_without_asking}",
                                // One setting at a time, as for the YOLO box above.
                                disabled: busy || editing.is_some(),
                                onchange: move |event| on_commands_without_asking.call((id, event.checked())),
                            }
                        }
                        p { class: "host-settings-help", "{commands_help(host.commands_without_asking)}" }
                        {outcome_for(SettingsField::CommandsWithoutAsking)}
                    }
                    for (local_network, allowed, label, help, field) in [
                        (false, host.links_without_asking, "Open links without asking",
                         "Agents on this host may open public web addresses. Local-network addresses still need their own permission.",
                         SettingsField::LinksWithoutAsking),
                        (true, host.local_links_without_asking, "Open local-network links without asking",
                         "Agents on this host may make your browser load addresses on your own machine or private network without an approval question.",
                         SettingsField::LocalLinksWithoutAsking),
                    ] {
                        div { class: "setting-group",
                            label {
                                class: "host-settings-choice setting-row",
                                "data-tooltip": help,
                                span { "{label}" }
                                input {
                                    r#type: "checkbox",
                                    class: if local_network { "host-local-links-without-asking-toggle control-switch" }
                                        else { "host-links-without-asking-toggle control-switch" },
                                    checked: allowed,
                                    "data-browser-permission": "{allowed}",
                                    disabled: busy || editing.is_some(),
                                    onchange: move |event| on_browser_permission.call((id, local_network, event.checked())),
                                }
                            }
                            p { class: "host-settings-help", "{help}" }
                            {outcome_for(field)}
                        }
                    }
                }
                }
                }
                crate::modal_dialog::DialogFooter { class: "host-settings-actions",
                    span { class: "dialog-footer-hint", "changes save as you make them" }
                    crate::modal_dialog::DialogActions {
                        button {
                            r#type: "button",
                            class: "btn btn-neutral host-settings-close",
                            "data-tooltip": "close: done with this host's settings",
                            onclick: move |_| on_close.call(()),
                            "close"
                        }
                    }
                }
            }
        }
    }
}

/// Ask before forgetting a host, with its identity and answers always visible.
///
/// Removing the row leaves the supervisor and sessions running. The header
/// and Cancel therefore share the same safe dismissal and row-focus return.
/// Once removal starts, neither may release modal isolation before the helm
/// answers; the panel checks its live request state before releasing focus
/// and closing. Rendered busy props alone cannot guard same-turn input.
/// The dialog owns any refusal from that request. The permanent
/// answer names the app-wide gear so users can restore this confirmation.
///
/// `shown` is the opening of the panel's removal question that the panel's
/// render read. The dialog draws the host from it and hands that same token
/// back with every answer: `on_remove` with whether to stop asking, and
/// `on_cancel` from Cancel, the header's dismiss and Escape alike. That lets
/// the panel refuse an answer drawn for a showing that is no longer the open
/// one, and otherwise remove exactly the entry this dialog named.
#[component]
pub(super) fn HostRemoveDialog(
    shown: ConfirmToken<Host>,
    busy: bool,
    error: Option<String>,
    on_remove: EventHandler<(ConfirmToken<Host>, bool)>,
    on_cancel: EventHandler<ConfirmToken<Host>>,
) -> Element {
    let host = shown.key().clone();
    let shown_name = gui_host_name(host.name.raw(), host.kind.is_this_machine());
    // The shortened heading must leave the complete destination readable.
    let destination = host.destination.as_ref().unwrap_or(&host.name).escaped();
    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            ModalDialog {
                handle: DIALOG,
                focus: DialogFocus::remembered(
                    ".host-remove-confirm:enabled", Some(".host-cancel-edit, .host-settings-close"),
                    true, Some(".host-remove-confirm"),
                ),
                class: "host-settings-dialog host-remove-dialog host-dialog-framed",
                role: "dialog",
                aria_modal: "true",
                aria_label: "remove host · {shown_name}",
                tabindex: "-1",
                on_escape: {
                    let shown = shown.clone();
                    move |evt: KeyboardEvent| {
                        if !evt.is_composing() && !busy {
                            on_cancel.call(shown.clone());
                        }
                    }
                },
                HostDialogHeader {
                    host: Some(host.clone()),
                    title: rsx! { "remove host · " span { class: "peer-value", dir: "ltr", "{shown_name}" } },
                    summary: error.as_ref().map(|_| "Farhelm refused this action".to_string()),
                    summary_danger: error.is_some(),
                    dismiss_label: "cancel host removal",
                    dismiss_tooltip: "cancel: keep this host",
                    on_dismiss: {
                        let shown = shown.clone();
                        move |_| {
                            if !busy {
                                on_cancel.call(shown.clone());
                            }
                        }
                    },
                }
                ScrollRegion { class: "dialog-body host-settings-body",
                    handle: DIALOG, selector: ".host-settings-body", name: "host removal details",
                    recovery: RecoveryFocus::new(".host-settings-close", None, true, RecoveryDelay::OneFrame),
                    p { class: "host-settings-value peer-value host-dialog-destination", dir: "ltr", "{destination}" }
                    p { class: "host-remove-explanation",
                        "Farhelm will forget this host. Its supervisor and sessions keep running, and adding the destination again finds them."
                    }
                    if let Some(reason) = error {
                        PeerLine {
                            class: "host-refusal-reason",
                            parts: vec![DetailPart::peer(reason)],
                        }
                    }
                }
                crate::modal_dialog::DialogFooter { class: "host-dialog-footer",
                    p { id: "host-remove-permanent-hint", class: "dialog-footer-hint host-dialog-permanent-hint",
                        "undo \"don't ask again\" with the gear at the top of the sidebar"
                    }
                    crate::modal_dialog::DialogActions { class: "host-remove-actions",
                        button {
                            r#type: "button",
                            class: "btn btn-neutral host-settings-close",
                            "data-tooltip": "cancel: keep this host",
                            disabled: busy,
                            autofocus: busy,
                            onclick: {
                                let shown = shown.clone();
                                move |_| {
                                    if !busy {
                                        on_cancel.call(shown.clone())
                                    }
                                }
                            },
                            "cancel"
                        }
                        button {
                            r#type: "button",
                            class: "btn btn-danger btn-outline",
                            "data-tooltip": "remove it, and remove hosts without asking from now on",
                            aria_describedby: "host-remove-permanent-hint",
                            disabled: busy,
                            onkeydown: crate::guard_confirmation_enter,
                            onclick: {
                                let shown = shown.clone();
                                move |_| on_remove.call((shown.clone(), true))
                            },
                            "remove, don't ask again"
                        }
                        button {
                            r#type: "button",
                            class: "btn btn-danger host-remove-confirm",
                            "data-tooltip": "remove: forget this host; its supervisor and sessions keep running",
                            disabled: busy,
                            autofocus: !busy,
                            onkeydown: crate::guard_confirmation_enter,
                            onclick: move |_| on_remove.call((shown.clone(), false)),
                            "remove"
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host's line must show in the dialog while the dialog hides the row
    /// that would otherwise carry it, and under the right setting: a leftover
    /// host-action refusal shown under a setting the user never touched
    /// would blame the wrong thing; those actions use their own dialogs.
    /// Specifies: nothing to show gives nothing; a line the dialog's own write
    /// produced goes under that setting; a host action with no settings
    /// field never appears here. An unreadable accepted reply is a warning.
    #[test]
    fn field_outcome_places_the_line_under_the_setting_that_produced_it() {
        let error = || Some("refused".to_string());
        assert_eq!(
            field_outcome(Some(SettingsField::Alias), None, None),
            None,
            "nothing to show"
        );
        assert_eq!(
            field_outcome(None, error(), None),
            None,
            "a host action's line has no settings field"
        );
        assert_eq!(
            field_outcome(Some(SettingsField::Alias), error(), None),
            Some((
                SettingsField::Alias,
                FieldOutcome::Error("refused".to_string())
            ))
        );
        assert_eq!(
            field_outcome(
                Some(SettingsField::YoloWithoutAsking),
                None,
                Some("unreadable reply".to_string())
            ),
            Some((
                SettingsField::YoloWithoutAsking,
                FieldOutcome::Warning("unreadable reply".to_string())
            )),
            "a committed-but-unreadable reply is shown as a warning, not an error"
        );
    }
}

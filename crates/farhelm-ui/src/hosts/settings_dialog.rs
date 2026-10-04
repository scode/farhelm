//! The host settings dialog: one host's destination, alias, and whether YOLO
//! launches on it need a confirmation, in a modal over the whole page.
//!
//! It replaced a panel that expanded inline inside the host's sidebar row.
//! The sidebar is too narrow for that: the destination was cut off, the edit
//! buttons crowded the values, and the YOLO explanation wrapped awkwardly.
//! The dialog shows every value in full and edits one field at a time, in
//! place, saving each change on its own (there is no dialog-wide Save).
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
//! and restart-with dialogs. On close, focus returns to the host row's "⋯"
//! toggle the dialog was opened from.
//!
//! Two focus hazards are handled rather than designed away:
//!
//! - A write natively disables the control that started it (the field, its
//!   save button, the YOLO checkbox) for as long as the request holds the
//!   page's operation token, and a disabled element drops focus to `body`.
//!   The restart-with dialog avoids that with `aria-disabled` everywhere
//!   (see its module doc); here the controls are the hosts panel's shared
//!   ones, so instead the isolation's Escape goes to the field's own cancel
//!   button when a field is open (natively disabled mid-save, so Escape then
//!   does nothing, as the dialog's own handler would), and focus is put back
//!   on the field or checkbox once the write ends (`refocus_after_write`).
//! - There is no Tab trap, unlike restart-with's. The isolation makes
//!   everything outside the dialog inert, so Tab past the last control goes
//!   to the browser's own chrome rather than into the page; coming back
//!   lands in the dialog, and any stray key is caught by the isolation's net.
//!   That was judged enough for a dialog of a few controls; add a trap if
//!   leaving to the browser chrome turns out to confuse people.

use dioxus::prelude::*;

use super::{EditField, HostDestinationForm, gui_host_name};
use crate::modal_isolation;
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::{Host, HostId};

/// The selector the isolation and the focus scripts find the dialog by.
const DIALOG_SELECTOR: &str = r#".host-settings-dialog[role="dialog"]"#;
const ADD_DIALOG_SELECTOR: &str = r#".host-add-dialog[role="dialog"]"#;
const UNINSTALL_DIALOG_SELECTOR: &str = r#".host-uninstall-dialog[role="dialog"]"#;

/// One setting the dialog can change, as the key its last write's outcome is
/// filed under. A superset of [`EditField`]: the two checkboxes write through
/// the same `run` path but have no text editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsField {
    Destination,
    Alias,
    YoloWithoutAsking,
    CommandsWithoutAsking,
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

/// Place this host's current error or warning line in the dialog: under the
/// setting the dialog's own last write changed, or, with `None`, at the top.
///
/// `HostsPanel` keeps outcomes per host, not per field, and a host's line can
/// come from something the dialog did not do (a retry or adopt refusal). Such
/// a line still shows, at the top, rather than nowhere: the row that would
/// otherwise carry it is behind the backdrop, and a refusal that sent the user
/// to these settings (fix the destination) is exactly the one to keep in
/// view. `last_write` is the setting the dialog last wrote for THIS host, if
/// its write is what produced the current line.
pub(super) fn field_outcome(
    last_write: Option<SettingsField>,
    error: Option<String>,
    warning: Option<String>,
) -> Option<(Option<SettingsField>, FieldOutcome)> {
    let outcome = error
        .map(FieldOutcome::Error)
        .or(warning.map(FieldOutcome::Warning))?;
    Some((last_write, outcome))
}

/// Release the dialog's isolation, then focus host `id`'s "⋯" toggle.
///
/// Called by the close path before it unmounts the dialog. The order is the
/// point: the toggle sits outside the dialog and stays inert until the
/// isolation is released, and `focus()` on an inert element silently does
/// nothing. The focus waits a frame so it lands after the dialog is gone.
/// A host that was removed meanwhile has no toggle, and the lookup just
/// finds nothing.
pub(crate) fn return_focus_to_row(id: HostId) {
    let row = serde_json::to_string(&format!(r#"[data-host-id="{id}"] .host-row-menu"#))
        .expect("a string always serializes");
    document::eval(&format!(
        "{} requestAnimationFrame(() => document.querySelector({row})?.focus({{ preventScroll: true }}))",
        modal_isolation::release_js(DIALOG_SELECTOR),
    ));
}

/// Move focus into the freshly mounted dialog, then isolate it.
///
/// Focus goes first because `modal_isolation::install_js` requires it (making
/// the previously focused element's ancestors inert would blur it). The first
/// control is an edit button, or the YOLO checkbox on a host with neither a
/// destination nor an alias to edit; the container itself is the fallback.
/// It also starts remembering the last control inside the dialog that had
/// focus, for `refocus_after_write`.
fn install_dialog() {
    install_dialog_with_initial_focus("button, input");
}

/// Install modal isolation while putting the destructive dialog's safe escape
/// hatch first in the focus order. The shared settings dialog focuses its
/// first editor, but removal must start on Cancel so an accidental Enter
/// cannot immediately forget the host.
fn install_remove_dialog() {
    install_dialog_with_initial_focus(".host-settings-close");
}

/// Focus and isolate the add-host dialog, whose first control is the SSH
/// destination rather than a destructive action.
pub(crate) fn install_add_dialog() {
    install_dialog_with_selector(
        ADD_DIALOG_SELECTOR,
        ".add-host-ssh",
        Some(".add-host-cancel"),
    );
}

/// Focus and isolate a row's uninstall confirmation with Cancel first, as
/// the removal dialog does: the confirm button removes Farhelm from a host,
/// so a stray Enter must land on the safe answer.
pub(crate) fn install_uninstall_dialog() {
    install_dialog_with_selector(
        UNINSTALL_DIALOG_SELECTOR,
        ".uninstall-cancel",
        Some(".uninstall-cancel"),
    );
}

/// Release the add-host modal and return focus to the heading's add button.
pub(crate) fn return_focus_to_add_button() {
    let selector = serde_json::to_string(".add-host-button").expect("a string always serializes");
    document::eval(&format!(
        "{} requestAnimationFrame(() => document.querySelector({selector})?.focus({{ preventScroll: true }}))",
        modal_isolation::release_js(ADD_DIALOG_SELECTOR),
    ));
}

/// Put the safe cancel action first when probing changes into a setup offer.
pub(crate) fn focus_add_cancel() {
    document::eval("document.querySelector('.add-host-cancel')?.focus({ preventScroll: true });");
}

/// Apply the shared modal setup and choose the first control explicitly.
fn install_dialog_with_initial_focus(initial_focus: &str) {
    install_dialog_with_selector(
        DIALOG_SELECTOR,
        initial_focus,
        Some(".host-cancel-edit, .host-settings-close"),
    );
}

/// Install focus isolation for a dialog and name its safe Escape action.
///
/// The add-host dialog shares this helper but has a different cancel control;
/// keeping the selector beside the dialog setup prevents the document-level
/// Escape fallback from silently swallowing Escape when focus has escaped.
pub(crate) fn install_dialog_with_selector(
    dialog_selector: &str,
    initial_focus: &str,
    escape_selector: Option<&str>,
) {
    let dialog = serde_json::to_string(dialog_selector).expect("a string always serializes");
    let initial_focus = serde_json::to_string(initial_focus).expect("a string always serializes");
    document::eval(&format!(
        r#"(() => {{
            const dialog = document.querySelector({dialog});
            if (!dialog) return;
            if (!dialog.__farhelmFocusMemory) {{
                dialog.__farhelmFocusMemory = true;
                dialog.addEventListener('focusin', (event) => {{
                    if (event.target !== dialog) dialog.__farhelmLastFocus = event.target;
                }});
            }}
            (dialog.querySelector({initial_focus}) ?? dialog).focus({{ preventScroll: true }});
        }})(); {}"#,
        // The caller supplies the dialog's safe cancel action. Settings use
        // the field-cancel selector before their dialog close control; the
        // add flow supplies its own cancel button.
        modal_isolation::install_js(dialog_selector, escape_selector,),
    ));
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
/// closed on success, gets there first; if focus is already on one of the
/// dialog's controls by then, this leaves it alone.
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
    document::eval(&format!(
        r#"requestAnimationFrame(() => requestAnimationFrame(() => {{
            const dialog = document.querySelector({dialog});
            if (!dialog) return;
            {reset}
            const active = document.activeElement;
            if (active && active !== dialog && dialog.contains(active)) return;
            const last = dialog.__farhelmLastFocus;
            const usable = last instanceof Element && last.isConnected && dialog.contains(last) && !last.disabled;
            const target = usable
                ? last
                : (dialog.querySelector('.host-destination-input')
                    ?? dialog.querySelector('.host-yolo-without-asking-toggle')
                    ?? dialog);
            target.focus({{ preventScroll: true }});
        }}))"#
    ));
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

/// JavaScript, run with `dialog` in scope, that sets both checkboxes' shown
/// state back to the settings the page currently renders.
///
/// Each value is read from its input's `data-` attribute when the script
/// runs, not baked in when it is built: a host-list refresh landing in
/// between (another client toggling the same host) must win over a reset that
/// was queued against the older value. Both boxes are reset whichever one was
/// refused, because resetting one that already shows its setting is a no-op.
fn reset_checkboxes_js() -> String {
    "const yolo = dialog.querySelector('.host-yolo-without-asking-toggle'); \
     if (yolo) yolo.checked = yolo.dataset.yoloWithoutAsking === 'true'; \
     const commands = dialog.querySelector('.host-commands-without-asking-toggle'); \
     if (commands) commands.checked = commands.dataset.commandsWithoutAsking === 'true';"
        .to_string()
}

/// Undo a checkbox click whose write never started.
///
/// The hosts panel refuses to start a write while another operation holds
/// the page's lock; the click has already flipped the box by then, and with
/// no write there is no end of one to trigger the reset in
/// `refocus_after_write`.
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
        "YOLO means an agent runs with no approval prompts: any command, any file, without asking you. \
         YOLO sessions start on this host without asking you to confirm."
    } else {
        "YOLO means an agent runs with no approval prompts: any command, any file, without asking you. \
         Farhelm asks you to confirm each YOLO launch on this host."
    }
}

/// The explanation under the "run farhelm commands" checkbox, for its current
/// state.
///
/// Says what the commands do (an agent in one of this host's sessions acting on
/// the fleet) because the label alone does not, and the "on" text names what
/// turning it on gives away.
fn commands_help(commands_without_asking: bool) -> &'static str {
    if commands_without_asking {
        "Agents in this host's sessions start, stop, restart, rename and clone sessions on any host, \
         and edit launch templates, without asking you."
    } else {
        "When an agent in one of this host's sessions asks Farhelm to start, stop, restart, rename or \
         clone a session, or to edit a launch template, Farhelm asks you first."
    }
}

/// The dialog itself. Every prop is state or a handler owned by `HostsPanel`;
/// see this module's doc for the division of labor.
#[component]
pub(super) fn HostSettingsDialog(
    host: Host,
    /// Whether writes are disabled right now: this host's own write or
    /// provisioning run, or any other operation holding the page's token.
    busy: bool,
    /// The field whose editor is open, if any.
    editing: Option<EditField>,
    /// This host's error or warning line, under the setting the dialog's own
    /// write changed, or (`None`) at the top; see [`field_outcome`].
    outcome: Option<(Option<SettingsField>, FieldOutcome)>,
    /// The open editor's draft, owned by the panel so a refresh cannot drop it.
    draft: Signal<String>,
    on_edit_start: EventHandler<(HostId, EditField, String)>,
    on_edit_submit: EventHandler<(HostId, EditField, String)>,
    on_edit_cancel: EventHandler<()>,
    on_yolo_without_asking: EventHandler<(HostId, bool)>,
    on_commands_without_asking: EventHandler<(HostId, bool)>,
    on_close: EventHandler<()>,
) -> Element {
    let id = host.id;
    let shown_name = gui_host_name(&host.name, host.kind.is_this_machine());
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
            Some(SettingsField::YoloWithoutAsking | SettingsField::CommandsWithoutAsking),
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

    let outcome_for = move |field: Option<SettingsField>| -> Element {
        match &outcome {
            Some((owner, FieldOutcome::Error(text))) if *owner == field => rsx! {
                PeerLine {
                    class: "action-error host-error host-settings-outcome",
                    parts: vec![DetailPart::Peer(text.clone())],
                }
            },
            Some((owner, FieldOutcome::Warning(text))) if *owner == field => rsx! {
                PeerLine {
                    class: "host-warning host-settings-outcome",
                    parts: vec![DetailPart::Peer(text.clone())],
                }
            },
            _ => rsx! {},
        }
    };

    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            div {
                class: "host-settings-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "host settings · {shown_name}",
                // Programmatically focusable only, for the isolation's
                // keydown net and the no-control fallback in
                // `install_dialog`; `-1` keeps it out of the Tab order.
                tabindex: "-1",
                onmounted: move |_| install_dialog(),
                onkeydown: move |evt: KeyboardEvent| {
                    // An IME uses Escape to dismiss its candidates; treating
                    // that as cancel would throw away the draft mid-word.
                    if evt.key() != Key::Escape || evt.is_composing() {
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
                h2 { class: "host-settings-title",
                    "host settings · "
                    span { class: "peer-value", dir: "ltr", "{shown_name}" }
                }
                {outcome_for(None)}
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
                                "{display_peer(&destination)}"
                            }
                            button {
                                r#type: "button",
                                class: "btn btn-neutral host-edit",
                                aria_label: "edit destination",
                                "data-tooltip": "edit: change the ssh destination Farhelm uses for this host",
                                disabled: busy || editing.is_some(),
                                onclick: {
                                    let destination = destination.clone();
                                    move |_| on_edit_start.call((id, EditField::Destination, destination.clone()))
                                },
                                "edit"
                            }
                        }
                        {outcome_for(Some(SettingsField::Destination))}
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
                                "{display_peer(alias.as_deref().unwrap_or(\"none\"))}"
                            }
                            button {
                                r#type: "button",
                                class: "btn btn-neutral host-alias",
                                aria_label: "edit alias",
                                "data-tooltip": "edit: change the name this host is shown under",
                                disabled: busy || editing.is_some(),
                                onclick: {
                                    let alias = alias.clone().unwrap_or_default();
                                    move |_| on_edit_start.call((id, EditField::Alias, alias.clone()))
                                },
                                "edit"
                            }
                        }
                        {outcome_for(Some(SettingsField::Alias))}
                    }
                }
                div { class: "host-settings-yolo",
                    label { class: "host-yolo-without-asking",
                        "data-tooltip": "start YOLO sessions on this host without asking first",
                        input {
                            r#type: "checkbox",
                            class: "host-yolo-without-asking-toggle",
                            checked: host.yolo_without_asking,
                            "data-yolo-without-asking": "{host.yolo_without_asking}",
                            // One setting at a time: a toggle while a field
                            // is open would replace that field's refusal with
                            // its own outcome while the draft stays open.
                            disabled: busy || editing.is_some(),
                            onchange: move |event| on_yolo_without_asking.call((id, event.checked())),
                        }
                        span { "start YOLO sessions here without asking" }
                    }
                    p { class: "host-settings-help", "{yolo_help(host.yolo_without_asking)}" }
                    {outcome_for(Some(SettingsField::YoloWithoutAsking))}
                }
                div { class: "host-settings-commands",
                    label { class: "host-commands-without-asking",
                        "data-tooltip": "let agents in this host's sessions start, stop and change sessions and templates without asking first",
                        input {
                            r#type: "checkbox",
                            class: "host-commands-without-asking-toggle",
                            checked: host.commands_without_asking,
                            "data-commands-without-asking": "{host.commands_without_asking}",
                            // One setting at a time, as for the YOLO box above.
                            disabled: busy || editing.is_some(),
                            onchange: move |event| on_commands_without_asking.call((id, event.checked())),
                        }
                        span { "run farhelm commands from this host without asking" }
                    }
                    p { class: "host-settings-help", "{commands_help(host.commands_without_asking)}" }
                    {outcome_for(Some(SettingsField::CommandsWithoutAsking))}
                }
                div { class: "host-settings-actions",
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

/// Ask before forgetting a host, keeping the explanation and the safe default
/// in the same modal surface as host settings. The permanent answer names the
/// app-wide gear so users can find how to restore this confirmation.
#[component]
pub(super) fn HostRemoveDialog(
    host: Host,
    busy: bool,
    on_remove: EventHandler<bool>,
    on_cancel: EventHandler<()>,
) -> Element {
    let shown_name = gui_host_name(&host.name, host.kind.is_this_machine());
    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            div {
                class: "host-settings-dialog host-remove-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "remove host · {shown_name}",
                tabindex: "-1",
                onmounted: move |_| install_remove_dialog(),
                onkeydown: move |evt: KeyboardEvent| {
                    if evt.key() == Key::Escape && !evt.is_composing() && !busy {
                        return_focus_to_row(host.id);
                        on_cancel.call(());
                    }
                },
                h2 { class: "host-settings-title",
                    "remove host · "
                    span { class: "peer-value", dir: "ltr", "{shown_name}" }
                }
                p { class: "host-remove-explanation",
                    "Farhelm will forget this host. Its supervisor and sessions keep running, and adding the destination again finds them."
                }
                div { class: "host-settings-actions host-remove-actions",
                    button {
                        r#type: "button",
                        class: "btn btn-danger",
                        "data-tooltip": "remove: forget this host; its supervisor and sessions keep running",
                        disabled: busy,
                        onclick: move |_| on_remove.call(false),
                        "remove"
                    }
                    div { class: "host-permanent-answer",
                        button {
                            r#type: "button",
                            class: "btn btn-danger btn-outline",
                            "data-tooltip": "remove it, and remove hosts without asking from now on",
                            aria_describedby: "host-remove-permanent-hint",
                            disabled: busy,
                            onclick: move |_| on_remove.call(true),
                            "remove, and don't ask again"
                        }
                        p { id: "host-remove-permanent-hint", class: "host-settings-help",
                            "You can turn this back on with the gear at the top of the sidebar."
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-neutral host-settings-close",
                        "data-tooltip": "cancel: keep this host",
                        autofocus: true,
                        onclick: move |_| {
                            return_focus_to_row(host.id);
                            on_cancel.call(())
                        },
                        "cancel"
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
    /// retry or adopt refusal shown under a setting the user never touched
    /// would blame the wrong thing, and one shown nowhere would be lost.
    /// Specifies: nothing to show gives nothing; a line the dialog's own write
    /// produced goes under that setting; any other line goes to the top
    /// (`None`); a committed-but-unreadable reply is a warning, not an error.
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
            Some((None, FieldOutcome::Error("refused".to_string()))),
            "a line the dialog did not produce goes to the top"
        );
        assert_eq!(
            field_outcome(Some(SettingsField::Alias), error(), None),
            Some((
                Some(SettingsField::Alias),
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
                Some(SettingsField::YoloWithoutAsking),
                FieldOutcome::Warning("unreadable reply".to_string())
            )),
            "a committed-but-unreadable reply is shown as a warning, not an error"
        );
    }
}

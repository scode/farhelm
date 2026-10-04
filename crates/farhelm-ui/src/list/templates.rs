//! The Templates panel: create, edit and delete the helm's launch templates
//! (SPEC.md, Launch templates: "Templates are created, edited, and deleted
//! in a Templates panel whose control sits beside New; its form offers
//! every launcher field, each optional").
//!
//! A modal dialog rather than a popover anchored to its button: the dialog
//! needs no placement or focus-out state machine, and the page behind it
//! (the session list, an open terminal) is inert while it is up, exactly as
//! for the settings dialog it is built like.
//!
//! The form is one [`TemplateForm`] of strings, one per launcher field, and
//! the only translation to and from what the helm stores is
//! [`fields_from_form`] and [`form_from_fields`]. Every field has a "leave as
//! is" state (the template does not set it); the agent-launch choices with a
//! default and the resume command also have a "reset to default" state,
//! which the stored JSON keeps as `null`.

use dioxus::prelude::*;
use farhelm_proto::launcher::{LauncherKind, TemplateDestination, TemplateFields};
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission};

use super::shared::HostOption;
use crate::ApiBase;
use crate::hosts::settings_dialog::install_dialog_with_selector;
use crate::peer::display_peer;

const DIALOG_SELECTOR: &str = r#".templates-dialog[role="dialog"]"#;

/// A count bumped every time the Templates dialog closes, provided by the
/// list view so the launcher re-reads the helm's templates after they may
/// have changed. The launcher can be open behind the dialog, and without
/// this it would go on applying a template's old version, offering none it
/// lacks, and still applying one that was deleted.
#[derive(Clone, Copy)]
pub(super) struct TemplatesRevision(pub(super) Signal<u64>);

/// Release the dialog's focus lock, then hand focus back to the button
/// that opened it. The release comes first because that button is inert
/// until then, and `focus()` on an inert element silently does nothing
/// (`modal_isolation`'s close-path rule).
pub(super) fn return_focus_to_templates_button() {
    document::eval(&format!(
        "{} requestAnimationFrame(() => document.querySelector('.templates-button')?.focus({{ preventScroll: true }}))",
        crate::modal_isolation::release_js(DIALOG_SELECTOR),
    ));
}

/// A select's value for "the template does not set this field".
const LEAVE: &str = "";
/// A select's value for "the template resets this choice to its default".
const RESET: &str = "default";
/// A select's value for "the template sets this text field to the text
/// beside it" (model, resume command, destination).
const SET: &str = "set";

/// The Templates form as the user edits it: each field's select state and
/// text, before it becomes [`TemplateFields`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TemplateForm {
    pub(super) name: String,
    pub(super) kind: String,
    pub(super) agent: String,
    pub(super) model_mode: String,
    pub(super) model: String,
    pub(super) effort: String,
    pub(super) permissions: String,
    pub(super) workspace_trust: String,
    pub(super) command: String,
    pub(super) yolo: String,
    pub(super) resume_mode: String,
    pub(super) resume: String,
    pub(super) host: String,
    pub(super) destination_mode: String,
    pub(super) destination: String,
    pub(super) session_name: String,
}

/// A wire enum's word, the value its select option carries.
fn word<T: serde::Serialize>(value: T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(word)) => word,
        _ => String::new(),
    }
}

/// Read a wire enum back from its select value.
fn from_word<T: serde::de::DeserializeOwned>(field: &str, value: &str) -> Result<T, String> {
    serde_json::from_value(serde_json::Value::String(value.to_string()))
        .map_err(|_| format!("{field} {value:?} is not a value this field takes"))
}

/// A choice with a default: leave, reset to default, or a value.
fn tri_state<T: serde::de::DeserializeOwned>(
    field: &str,
    value: &str,
) -> Result<Option<Option<T>>, String> {
    match value {
        LEAVE => Ok(None),
        RESET => Ok(Some(None)),
        other => from_word(field, other).map(|value| Some(Some(value))),
    }
}

/// The fields a form describes, or the reason it describes none. An empty
/// text field means the template leaves that field alone; nothing is
/// trimmed, because a command's spacing is the user's.
pub(super) fn fields_from_form(form: &TemplateForm) -> Result<TemplateFields, String> {
    let nonempty = |text: &str| (!text.is_empty()).then(|| text.to_string());
    Ok(TemplateFields {
        kind: match form.kind.as_str() {
            LEAVE => None,
            other => Some(from_word::<LauncherKind>("launch kind", other)?),
        },
        agent: match form.agent.as_str() {
            LEAVE => None,
            other => Some(from_word::<LaunchHarness>("agent type", other)?),
        },
        model: match form.model_mode.as_str() {
            LEAVE => None,
            RESET => Some(None),
            _ if form.model.is_empty() => {
                return Err("type the model to set, or leave it as is".to_string());
            }
            _ => Some(Some(form.model.clone())),
        },
        effort: tri_state::<LaunchEffort>("effort", &form.effort)?,
        permissions: tri_state::<LaunchPermission>("permissions", &form.permissions)?,
        workspace_trust: match form.workspace_trust.as_str() {
            LEAVE => None,
            RESET => Some(None),
            "true" => Some(Some(true)),
            "false" => Some(Some(false)),
            other => {
                return Err(format!(
                    "workspace trust {other:?} is not a value this field takes"
                ));
            }
        },
        command: nonempty(&form.command),
        yolo: match form.yolo.as_str() {
            LEAVE => None,
            "true" => Some(true),
            "false" => Some(false),
            other => {
                return Err(format!(
                    "YOLO answer {other:?} is not a value this field takes"
                ));
            }
        },
        resume_command: match form.resume_mode.as_str() {
            LEAVE => None,
            RESET => Some(None),
            _ if form.resume.is_empty() => {
                return Err("type the resume command to set, or leave it as is".to_string());
            }
            _ => Some(Some(form.resume.clone())),
        },
        host: nonempty(&form.host),
        destination: match form.destination_mode.as_str() {
            LEAVE => None,
            _ if form.destination.is_empty() => {
                return Err("type the folder or repository to set, or leave it as is".to_string());
            }
            "github" => Some(TemplateDestination::Github(form.destination.clone())),
            _ => Some(TemplateDestination::Folder(form.destination.clone())),
        },
        name: nonempty(&form.session_name),
    })
}

/// The form showing a stored template, for editing it.
pub(super) fn form_from_fields(name: &str, fields: &TemplateFields) -> TemplateForm {
    let tri = |set: Option<String>, is_set: bool| match (is_set, set) {
        (false, _) => LEAVE.to_string(),
        (true, None) => RESET.to_string(),
        (true, Some(word)) => word,
    };
    TemplateForm {
        name: name.to_string(),
        kind: fields.kind.map(word).unwrap_or_default(),
        agent: fields.agent.map(word).unwrap_or_default(),
        model_mode: match &fields.model {
            None => LEAVE.to_string(),
            Some(None) => RESET.to_string(),
            Some(Some(_)) => SET.to_string(),
        },
        model: fields.model.clone().flatten().unwrap_or_default(),
        effort: tri(fields.effort.flatten().map(word), fields.effort.is_some()),
        permissions: tri(
            fields.permissions.flatten().map(word),
            fields.permissions.is_some(),
        ),
        workspace_trust: tri(
            fields
                .workspace_trust
                .flatten()
                .map(|trust| trust.to_string()),
            fields.workspace_trust.is_some(),
        ),
        command: fields.command.clone().unwrap_or_default(),
        yolo: fields.yolo.map(|yolo| yolo.to_string()).unwrap_or_default(),
        resume_mode: match &fields.resume_command {
            None => LEAVE.to_string(),
            Some(None) => RESET.to_string(),
            Some(Some(_)) => SET.to_string(),
        },
        resume: fields.resume_command.clone().flatten().unwrap_or_default(),
        host: fields.host.clone().unwrap_or_default(),
        destination_mode: match &fields.destination {
            None => LEAVE.to_string(),
            Some(TemplateDestination::Folder(_)) => "folder".to_string(),
            Some(TemplateDestination::Github(_)) => "github".to_string(),
        },
        destination: match &fields.destination {
            Some(TemplateDestination::Folder(path) | TemplateDestination::Github(path)) => {
                path.clone()
            }
            None => String::new(),
        },
        session_name: fields.name.clone().unwrap_or_default(),
    }
}

/// A one-line description of what a template sets, for its row in the
/// list: every field it sets, a reset shown as such. The command and resume
/// text are left out (they can be long, and the edit form shows them).
fn template_summary(fields: &TemplateFields) -> String {
    let choice = |name: &str, value: Option<String>| match value {
        Some(value) => format!("{name} {value}"),
        None => format!("{name} reset"),
    };
    let mut parts = Vec::new();
    if let Some(kind) = fields.kind {
        parts.push(format!("{} launch", word(kind)));
    }
    if let Some(agent) = fields.agent {
        parts.push(crate::launch_composer::harness_label(agent).to_string());
    }
    if let Some(model) = &fields.model {
        parts.push(match model {
            Some(model) => model.clone(),
            None => "model reset".to_string(),
        });
    }
    if let Some(effort) = fields.effort {
        parts.push(choice("effort", effort.map(word)));
    }
    if let Some(permissions) = fields.permissions {
        parts.push(choice("permissions", permissions.map(word)));
    }
    if let Some(trust) = fields.workspace_trust {
        parts.push(choice(
            "trust",
            trust.map(|trust| if trust { "on" } else { "off" }.to_string()),
        ));
    }
    if fields.command.is_some() {
        parts.push("command".to_string());
    }
    if let Some(yolo) = fields.yolo {
        parts.push(if yolo { "YOLO" } else { "not YOLO" }.to_string());
    }
    if let Some(resume) = &fields.resume_command {
        parts.push(
            if resume.is_some() {
                "resume command"
            } else {
                "resume reset"
            }
            .to_string(),
        );
    }
    if fields.host.is_some() {
        parts.push("host".to_string());
    }
    if fields.destination.is_some() {
        parts.push("destination".to_string());
    }
    if let Some(name) = &fields.name {
        parts.push(format!("name {}", display_peer(name)));
    }
    if parts.is_empty() {
        "sets nothing yet".to_string()
    } else {
        parts.join(" · ")
    }
}

/// One labelled select whose options are `(value, label)` pairs.
#[component]
fn Choice(
    label: String,
    value: String,
    options: Vec<(String, String)>,
    disabled: bool,
    on_change: EventHandler<String>,
) -> Element {
    rsx! {
        label { class: "templates-field",
            span { "{label}" }
            select {
                disabled,
                onchange: move |evt| on_change.call(evt.value()),
                for (option_value, option_label) in options {
                    option { value: "{option_value}", selected: option_value == value, "{option_label}" }
                }
            }
        }
    }
}

/// The Templates dialog. `hosts` is the registry snapshot the launcher
/// uses, for the host field (named by install identity, so a host with no
/// recorded identity cannot be offered).
#[component]
pub(super) fn TemplatesDialog(hosts: Vec<HostOption>, on_close: EventHandler<()>) -> Element {
    let base = use_context::<ApiBase>().0;
    let list_base = base.clone();
    let mut templates = use_resource(move || {
        let base = list_base.clone();
        async move { crate::api::fetch_templates(&base).await }
    });
    let mut form = use_signal(TemplateForm::default);
    // The name of the template the form edits, or `None` for a new one, so
    // a rename deletes the old name after saving the new one.
    let mut editing = use_signal(|| None::<String>);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let leave = || (LEAVE.to_string(), "leave as is".to_string());
    let reset = || (RESET.to_string(), "reset to default".to_string());
    let kinds = vec![
        leave(),
        ("agent".to_string(), "agent".to_string()),
        ("command".to_string(), "command".to_string()),
    ];
    let mut agents = vec![leave()];
    agents.extend(LaunchHarness::ALL.iter().map(|harness| {
        (
            word(*harness),
            crate::launch_composer::harness_label(*harness).to_string(),
        )
    }));
    let mut efforts = vec![leave(), reset()];
    efforts.extend(
        farhelm_proto::launcher::EFFORT_ORDER
            .iter()
            .map(|effort| (word(*effort), word(*effort))),
    );
    let mut permissions = vec![leave(), reset()];
    permissions.extend(
        [
            LaunchPermission::Yolo,
            LaunchPermission::Approve,
            LaunchPermission::SmartApprove,
            LaunchPermission::Chat,
        ]
        .map(|permission| (word(permission), permission.wire_word().replace('_', " "))),
    );
    let trust = vec![
        leave(),
        reset(),
        ("true".to_string(), "trusted".to_string()),
        ("false".to_string(), "not trusted".to_string()),
    ];
    let yolo = vec![
        leave(),
        ("true".to_string(), "yes (YOLO)".to_string()),
        ("false".to_string(), "no".to_string()),
    ];
    let text_modes = vec![leave(), reset(), (SET.to_string(), "set to".to_string())];
    let destination_modes = vec![
        leave(),
        ("folder".to_string(), "folder".to_string()),
        ("github".to_string(), "fresh GitHub checkout".to_string()),
    ];
    let mut host_choices = vec![leave()];
    host_choices.extend(hosts.iter().filter_map(|host| {
        host.identity
            .clone()
            .map(|identity| (identity, display_peer(&host.name)))
    }));
    let current = form();
    // A stored host no current row has (removed, or not yet contacted) is
    // still the template's; show it rather than a select that reads "leave
    // as is" while saving would keep the identity.
    if !current.host.is_empty() && !host_choices.iter().any(|(value, _)| *value == current.host) {
        host_choices.push((
            current.host.clone(),
            "an install no host here has".to_string(),
        ));
    }
    let disabled = busy();

    let save_base = base.clone();
    let save = move |_| {
        if busy() {
            return;
        }
        let draft = form();
        let fields = match fields_from_form(&draft) {
            Ok(fields) => fields,
            Err(message) => {
                error.set(Some(message));
                return;
            }
        };
        let base = save_base.clone();
        let previous = editing();
        // Saving a NEW template, or renaming one, under a name another
        // template already has would replace that other template, which the
        // user never opened. Last-write-wins (SPEC.md) is about two clients
        // editing the same template, not this form clobbering a different one.
        let taken = templates
            .peek()
            .as_ref()
            .and_then(|listed| listed.as_ref().ok())
            .is_some_and(|listed| listed.iter().any(|template| template.name == draft.name));
        if previous.as_deref() != Some(draft.name.as_str()) && taken {
            error.set(Some(format!(
                "a template named {:?} already exists; edit that one, or choose another name",
                draft.name
            )));
            return;
        }
        busy.set(true);
        error.set(None);
        spawn(async move {
            let outcome = crate::api::put_template(&base, &draft.name, &fields).await;
            // A rename stores the new name first, so a failure in between
            // leaves both rather than neither.
            let outcome = match (outcome, previous) {
                (Ok(()), Some(old)) if old != draft.name => {
                    crate::api::delete_template(&base, &old).await
                }
                (outcome, _) => outcome,
            };
            busy.set(false);
            match outcome {
                Ok(()) => {
                    form.set(TemplateForm::default());
                    editing.set(None);
                    templates.restart();
                }
                Err(message) => error.set(Some(message)),
            }
        });
    };

    let listed = templates.read().clone();
    let delete_base = base.clone();

    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            div {
                class: "templates-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "templates",
                tabindex: "-1",
                onmounted: move |_| install_dialog_with_selector(
                    DIALOG_SELECTOR, "input", Some(".templates-close"),
                ),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape && !event.is_composing() && !busy() {
                        on_close.call(());
                    }
                },
                h2 { class: "host-settings-title", "templates" }
                p { class: "host-settings-help",
                    "A template is a named set of launcher edits. In the launcher, type tl: and a name to apply one; it changes only the fields it sets."
                }
                match listed {
                    None => rsx! { p { class: "host-settings-help", "loading templates" } },
                    Some(Err(message)) => rsx! { p { class: "templates-error", role: "status", "templates unavailable: {message}" } },
                    Some(Ok(list)) if list.is_empty() => rsx! { p { class: "host-settings-help", "no templates yet" } },
                    Some(Ok(list)) => rsx! {
                        ul { class: "templates-list",
                            for template in list {
                                li { class: "templates-row", key: "{template.name}",
                                    span { class: "templates-row-name peer-value", dir: "ltr", "{display_peer(&template.name)}" }
                                    span { class: "templates-row-summary", "{template_summary(&template.fields)}" }
                                    button {
                                        r#type: "button",
                                        class: "btn btn-neutral templates-edit",
                                        disabled,
                                        onclick: {
                                            let template = template.clone();
                                            move |_| {
                                                form.set(form_from_fields(&template.name, &template.fields));
                                                editing.set(Some(template.name.clone()));
                                                error.set(None);
                                            }
                                        },
                                        "edit"
                                    }
                                    button {
                                        r#type: "button",
                                        class: "btn btn-danger templates-delete",
                                        disabled,
                                        onclick: {
                                            let name = template.name.clone();
                                            let base = delete_base.clone();
                                            move |_| {
                                                // `disabled` lands a render late, so a fast
                                                // second click still reaches this handler.
                                                if busy() {
                                                    return;
                                                }
                                                let name = name.clone();
                                                let base = base.clone();
                                                busy.set(true);
                                                spawn(async move {
                                                    let outcome = crate::api::delete_template(&base, &name).await;
                                                    busy.set(false);
                                                    match outcome {
                                                        Ok(()) => {
                                                            if editing() == Some(name.clone()) {
                                                                editing.set(None);
                                                                form.set(TemplateForm::default());
                                                            }
                                                            templates.restart();
                                                        }
                                                        Err(message) => error.set(Some(message)),
                                                    }
                                                });
                                            }
                                        },
                                        "delete"
                                    }
                                }
                            }
                        }
                    },
                }
                h3 { class: "templates-form-title",
                    if editing().is_some() { "edit template" } else { "new template" }
                }
                div { class: "templates-form",
                    label { class: "templates-field",
                        span { "name" }
                        input {
                            r#type: "text",
                            class: "templates-name",
                            value: "{current.name}",
                            disabled,
                            oninput: move |evt| form.with_mut(|form| form.name = evt.value()),
                        }
                    }
                    Choice { label: "launch kind", value: current.kind.clone(), options: kinds, disabled, on_change: move |value| form.with_mut(|form| form.kind = value) }
                    Choice { label: "agent type", value: current.agent.clone(), options: agents, disabled, on_change: move |value| form.with_mut(|form| form.agent = value) }
                    Choice { label: "model", value: current.model_mode.clone(), options: text_modes.clone(), disabled, on_change: move |value| form.with_mut(|form| form.model_mode = value) }
                    if current.model_mode == SET {
                        input {
                            r#type: "text",
                            class: "templates-model",
                            aria_label: "model to set",
                            value: "{current.model}",
                            disabled,
                            oninput: move |evt| form.with_mut(|form| form.model = evt.value()),
                        }
                    }
                    Choice { label: "effort", value: current.effort.clone(), options: efforts, disabled, on_change: move |value| form.with_mut(|form| form.effort = value) }
                    Choice { label: "permissions", value: current.permissions.clone(), options: permissions, disabled, on_change: move |value| form.with_mut(|form| form.permissions = value) }
                    Choice { label: "workspace trust", value: current.workspace_trust.clone(), options: trust, disabled, on_change: move |value| form.with_mut(|form| form.workspace_trust = value) }
                    label { class: "templates-field",
                        span { "command" }
                        input {
                            r#type: "text",
                            class: "templates-command",
                            placeholder: "leave as is",
                            dir: "ltr",
                            value: "{current.command}",
                            disabled,
                            oninput: move |evt| form.with_mut(|form| form.command = evt.value()),
                        }
                    }
                    Choice { label: "runs without approval prompts", value: current.yolo.clone(), options: yolo, disabled, on_change: move |value| form.with_mut(|form| form.yolo = value) }
                    Choice { label: "resume command", value: current.resume_mode.clone(), options: text_modes, disabled, on_change: move |value| form.with_mut(|form| form.resume_mode = value) }
                    if current.resume_mode == SET {
                        input {
                            r#type: "text",
                            class: "templates-resume",
                            aria_label: "resume command to set",
                            dir: "ltr",
                            value: "{current.resume}",
                            disabled,
                            oninput: move |evt| form.with_mut(|form| form.resume = evt.value()),
                        }
                    }
                    Choice { label: "host", value: current.host.clone(), options: host_choices, disabled, on_change: move |value| form.with_mut(|form| form.host = value) }
                    Choice { label: "destination", value: current.destination_mode.clone(), options: destination_modes, disabled, on_change: move |value| form.with_mut(|form| form.destination_mode = value) }
                    if current.destination_mode != LEAVE {
                        input {
                            r#type: "text",
                            class: "templates-destination",
                            aria_label: if current.destination_mode == "github" { "repository (owner/name)" } else { "folder" },
                            dir: "ltr",
                            value: "{current.destination}",
                            disabled,
                            oninput: move |evt| form.with_mut(|form| form.destination = evt.value()),
                        }
                    }
                    label { class: "templates-field",
                        span { "session name" }
                        input {
                            r#type: "text",
                            class: "templates-session-name",
                            placeholder: "leave as is",
                            value: "{current.session_name}",
                            disabled,
                            oninput: move |evt| form.with_mut(|form| form.session_name = evt.value()),
                        }
                    }
                }
                if let Some(message) = error() {
                    p { class: "templates-error", role: "status", "{display_peer(&message)}" }
                }
                div { class: "host-settings-actions",
                    if editing().is_some() {
                        button {
                            r#type: "button",
                            class: "btn btn-neutral templates-new",
                            disabled,
                            onclick: move |_| {
                                form.set(TemplateForm::default());
                                editing.set(None);
                                error.set(None);
                            },
                            "new template"
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-primary templates-save",
                        disabled,
                        onclick: save,
                        "save"
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-neutral templates-close",
                        disabled,
                        onclick: move |_| on_close.call(()),
                        "close"
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: the form and the stored fields translate both ways without
    /// loss: "leave as is" is an absent field, "reset to default" is `null`,
    /// and every value survives a save and a reload into the edit form.
    ///
    /// Why: SPEC.md has a template set only the fields it contains, so a
    /// form that turned "leave" into a value (or lost a reset) would make a
    /// template quietly do more or less than its author chose.
    #[test]
    fn the_template_form_round_trips_leave_reset_and_values() {
        let form = TemplateForm {
            name: "my-codex".into(),
            kind: "agent".into(),
            agent: "codex".into(),
            model_mode: SET.into(),
            model: "gpt-6-luna".into(),
            effort: RESET.into(),
            permissions: "yolo".into(),
            workspace_trust: "false".into(),
            resume_mode: RESET.into(),
            destination_mode: "github".into(),
            destination: "acme/web".into(),
            session_name: "web".into(),
            ..TemplateForm::default()
        };
        let fields = fields_from_form(&form).expect("a complete form");
        assert_eq!(fields.kind, Some(LauncherKind::Agent));
        assert_eq!(fields.agent, Some(LaunchHarness::Codex));
        assert_eq!(fields.model, Some(Some("gpt-6-luna".into())));
        assert_eq!(fields.effort, Some(None));
        assert_eq!(fields.permissions, Some(Some(LaunchPermission::Yolo)));
        assert_eq!(fields.workspace_trust, Some(Some(false)));
        assert_eq!(fields.command, None, "an empty command is left as is");
        assert_eq!(fields.yolo, None);
        assert_eq!(fields.resume_command, Some(None));
        assert_eq!(
            fields.destination,
            Some(TemplateDestination::Github("acme/web".into()))
        );
        assert_eq!(fields.name.as_deref(), Some("web"));
        assert_eq!(form_from_fields("my-codex", &fields), form);
        assert_eq!(
            fields_from_form(&TemplateForm::default()),
            Ok(TemplateFields::default())
        );
    }

    /// Spec: a field set to "set to" with no text is refused rather than
    /// stored as a value nobody typed.
    ///
    /// Why: an empty model or resume command set on purpose would launch
    /// with an empty argument; asking is the only safe reading.
    #[test]
    fn a_set_field_without_text_is_refused() {
        for form in [
            TemplateForm {
                model_mode: SET.into(),
                ..TemplateForm::default()
            },
            TemplateForm {
                resume_mode: SET.into(),
                ..TemplateForm::default()
            },
            TemplateForm {
                destination_mode: "folder".into(),
                ..TemplateForm::default()
            },
        ] {
            assert!(fields_from_form(&form).is_err(), "{form:?}");
        }
    }
}

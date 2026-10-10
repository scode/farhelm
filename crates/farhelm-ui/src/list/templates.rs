//! Edit launch templates without inventing values for fields they leave alone.
//!
//! The dialog keeps one explicit draft and a saved baseline. Stored kindless
//! templates are interpreted only in the draft: that difference is unsaved
//! until the user chooses save. API writes keep the existing last-write-wins
//! contract; name checks prevent an ordinary new/rename/undo from replacing a
//! template the user did not open.

use super::shared::HostOption;
use crate::ApiBase;
use crate::hosts::settings_dialog::install_dialog_with_selector;
use crate::peer::display_peer;
use dioxus::prelude::*;
use farhelm_proto::launcher::{
    LaunchCatalogModel, LaunchTemplate, LauncherKind, TEMPLATE_NAME_CAP, TemplateDestination,
    TemplateFields, check_template_shape,
};
use farhelm_proto::{LaunchHarness, LaunchPermission};

const DIALOG_SELECTOR: &str = r#".templates-dialog[role="dialog"]"#;
const DEFAULT: &str = "default";

/// Tell an already-open launcher to reload templates when the editor closes.
/// A deletion or edit otherwise leaves its old catalog usable behind the modal.
#[derive(Clone, Copy)]
pub(super) struct TemplatesRevision(pub(super) Signal<u64>);

/// Release modal isolation before returning focus; the opener is inert until
/// the lock is released, so focusing it in the opposite order does nothing.
pub(super) fn return_focus_to_templates_button() {
    document::eval(&format!(
        "{} requestAnimationFrame(() => document.querySelector('.templates-button')?.focus({{ preventScroll: true }}))",
        crate::modal_isolation::release_js(DIALOG_SELECTOR),
    ));
}

/// Move focus after responsive navigation has mounted its destination. A phone
/// row or Back button disappears when activated, so retaining focus there would
/// send keyboard users back to the beginning of the dialog.
fn focus_template_control(selector: &str) {
    let selector = serde_json::to_string(selector).expect("a string always serializes");
    document::eval(&format!(
        "requestAnimationFrame(() => document.querySelector('{DIALOG_SELECTOR}')?.querySelector({selector})?.focus())"
    ));
}

/// Recover focus lost to a disabled or removed control after a write or reload.
/// Reuse the dialog installer's focus memory, and leave a usable focused control
/// alone. Two frames let explicit navigation/prompt focus win before fallback.
fn restore_template_focus() {
    document::eval(&format!(
        r#"requestAnimationFrame(() => requestAnimationFrame(() => {{
            const dialog = document.querySelector('{DIALOG_SELECTOR}');
            if (!dialog) return;
            const usable = el => el instanceof HTMLElement && el.isConnected
                && dialog.contains(el) && !el.disabled && el.getClientRects().length;
            if (usable(document.activeElement)) return;
            const last = dialog.__farhelmLastFocus;
            const name = dialog.querySelector('.templates-name');
            (usable(last) ? last : usable(name) ? name
                : dialog.querySelector('.templates-new') ?? dialog).focus({{preventScroll: true}});
        }}))"#,
    ));
}

/// The same wire words serve as select values on both renderers.
pub(super) fn word<T: serde::Serialize>(value: T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(word)) => word,
        _ => String::new(),
    }
}

/// Controls offer only known enum words; a stale/unknown word remains a refusal.
fn from_word<T: serde::de::DeserializeOwned>(value: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(value.to_string())).ok()
}

/// Editor field identities separate absence from an empty text being edited.
/// Adding a field writes a present value immediately; removing it is the only
/// way to make its launcher edit absent again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Field {
    Agent,
    Model,
    Effort,
    Permissions,
    Trust,
    Command,
    Yolo,
    Resume,
    Host,
    Destination,
    Name,
}

impl Field {
    // Command-only fields lead when present; their absence keeps the agent
    // editor in its usual agent/model/effort order without a second ordering table.
    pub(super) const ALL: [Self; 11] = [
        Self::Command,
        Self::Yolo,
        Self::Agent,
        Self::Model,
        Self::Effort,
        Self::Permissions,
        Self::Trust,
        Self::Resume,
        Self::Host,
        Self::Destination,
        Self::Name,
    ];

    /// Both stored approval fields have one user-facing name, but remain
    /// separate edits so switching kind cannot silently reinterpret either.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Agent => "agent type",
            Self::Model => "model",
            Self::Effort => "effort",
            Self::Permissions | Self::Yolo => "approvals",
            Self::Trust => "workspace trust",
            Self::Command => "command",
            Self::Resume => "resume command",
            Self::Host => "host",
            Self::Destination => "destination",
            Self::Name => "session name",
        }
    }

    /// Stable field keys keep two different approvals controls distinguishable
    /// in the DOM and in tests even though their labels deliberately match.
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Model => "model",
            Self::Effort => "effort",
            Self::Permissions => "permissions",
            Self::Yolo => "yolo",
            Self::Trust => "trust",
            Self::Command => "command",
            Self::Resume => "resume",
            Self::Host => "host",
            Self::Destination => "destination",
            Self::Name => "name",
        }
    }

    /// Presence, including a reset or an empty draft text, means this field
    /// will be applied; value truthiness must never decide whether it exists.
    pub(super) fn is_set(self, f: &TemplateFields) -> bool {
        match self {
            Self::Agent => f.agent.is_some(),
            Self::Model => f.model.is_some(),
            Self::Effort => f.effort.is_some(),
            Self::Permissions => f.permissions.is_some(),
            Self::Trust => f.workspace_trust.is_some(),
            Self::Command => f.command.is_some(),
            Self::Yolo => f.yolo.is_some(),
            Self::Resume => f.resume_command.is_some(),
            Self::Host => f.host.is_some(),
            Self::Destination => f.destination.is_some(),
            Self::Name => f.name.is_some(),
        }
    }

    /// Add an editable value without choosing another field on the user's
    /// behalf. In particular, model/effort/trust do not require an agent type.
    fn add(self, f: &mut TemplateFields) {
        match self {
            Self::Agent => f.agent = Some(LaunchHarness::ALL[0]),
            Self::Model => f.model = Some(None),
            Self::Effort => f.effort = Some(None),
            Self::Permissions => f.permissions = Some(None),
            Self::Trust => f.workspace_trust = Some(None),
            Self::Command => f.command = Some(String::new()),
            Self::Yolo => f.yolo = Some(false),
            Self::Resume => f.resume_command = Some(None),
            Self::Host => f.host = Some(String::new()),
            Self::Destination => f.destination = Some(TemplateDestination::Folder(String::new())),
            Self::Name => f.name = Some(String::new()),
        }
    }

    /// Removal leaves the field alone when the template is applied; it does
    /// not reset the launcher's current value.
    pub(super) fn remove(self, f: &mut TemplateFields) {
        match self {
            Self::Agent => f.agent = None,
            Self::Model => f.model = None,
            Self::Effort => f.effort = None,
            Self::Permissions => f.permissions = None,
            Self::Trust => f.workspace_trust = None,
            Self::Command => f.command = None,
            Self::Yolo => f.yolo = None,
            Self::Resume => f.resume_command = None,
            Self::Host => f.host = None,
            Self::Destination => f.destination = None,
            Self::Name => f.name = None,
        }
    }

    /// Decide which edits can be added under this switch, using harness-owned
    /// capability answers rather than a second list of per-agent facts.
    pub(super) fn offered(self, f: &TemplateFields) -> bool {
        match self {
            Self::Host | Self::Destination | Self::Name => true,
            Self::Agent => f.kind.is_some(),
            Self::Command | Self::Yolo | Self::Resume => f.kind == Some(LauncherKind::Command),
            Self::Model => {
                f.kind == Some(LauncherKind::Agent)
                    && f.agent.is_none_or(LaunchHarness::offers_model)
            }
            Self::Effort => {
                f.kind == Some(LauncherKind::Agent)
                    && f.agent.is_none_or(LaunchHarness::offers_effort)
            }
            Self::Permissions => {
                f.kind == Some(LauncherKind::Agent) && f.agent.is_none_or(|a| !a.offers_only_yolo())
            }
            Self::Trust => {
                f.kind == Some(LauncherKind::Agent)
                    && f.agent.is_none_or(LaunchHarness::offers_workspace_trust)
            }
        }
    }

    /// Keep incompatible choices visible and require an explicit correction.
    /// Arbitrary model text is accepted even when the catalog does not know it;
    /// final launcher application retains its own stricter rules.
    fn refusal(self, f: &TemplateFields, catalog: Option<&[LaunchCatalogModel]>) -> Option<String> {
        let fits = if self == Self::Model {
            f.kind == Some(LauncherKind::Agent)
        } else {
            self.offered(f)
        };
        if !fits {
            return Some(
                "does not fit this launcher switch or agent type; remove it to save".into(),
            );
        }
        if self == Self::Permissions
            && let Some(Some(value)) = f.permissions
            && !permission_options(f.agent)
                .iter()
                .any(|(key, _)| *key == word(value))
        {
            return Some(
                "this agent type does not offer these approvals; change or remove them".into(),
            );
        }
        if self == Self::Effort
            && let Some(Some(value)) = f.effort
            && !effort_options(f, catalog)
                .iter()
                .any(|(key, _)| *key == word(value))
        {
            return Some(
                "this agent type or model does not offer this effort; change or remove it".into(),
            );
        }
        None
    }
}

/// Default remains a value inside the control. Its label explains whether the
/// harness's omission asks for approval or runs without prompts.
fn permission_options(agent: Option<LaunchHarness>) -> Vec<(String, String)> {
    let mut options = vec![(
        DEFAULT.into(),
        if agent.is_none() {
            "default (the agent's own)"
        } else if agent.and_then(LaunchHarness::omitted_permission) == Some(LaunchPermission::Yolo)
        {
            "default (YOLO)"
        } else {
            "default (asks)"
        }
        .into(),
    )];
    options.extend(
        LaunchPermission::ALL
            .iter()
            .copied()
            .filter(|permission| {
                agent.map_or(*permission == LaunchPermission::Yolo, |a| {
                    a.offers_permission(*permission)
                })
            })
            .map(|p| {
                (
                    word(p),
                    if p == LaunchPermission::Yolo {
                        "YOLO".into()
                    } else {
                        p.wire_word().replace('_', " ")
                    },
                )
            }),
    );
    options
}

/// Known catalogs narrow effort choices using the same helper as the launcher.
/// Before loading one, no catalog-based refusal is justified; the capability
/// check still rejects an agent type that has no effort choice at all.
fn effort_options(
    f: &TemplateFields,
    catalog: Option<&[LaunchCatalogModel]>,
) -> Vec<(String, String)> {
    let efforts = match (f.agent, catalog) {
        (Some(agent), Some(catalog)) => farhelm_proto::launcher::compatible_efforts(
            agent,
            f.model.as_ref().and_then(|m| m.as_deref()),
            catalog,
        ),
        _ => farhelm_proto::launcher::EFFORT_ORDER.to_vec(),
    };
    let mut options = vec![(DEFAULT.into(), "default".into())];
    options.extend(efforts.into_iter().map(|v| (word(v), word(v))));
    options
}

/// The editor's draft uses the wire fields directly so absence, false, and
/// explicit null resets survive editing without a second translation model.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Draft {
    name: String,
    fields: TemplateFields,
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            name: String::new(),
            fields: TemplateFields {
                kind: Some(LauncherKind::Agent),
                ..Default::default()
            },
        }
    }
}

impl Draft {
    /// Infer only in the editor. Comparing with the original baseline marks a
    /// legacy switch as unsaved instead of migrating stored records on read.
    fn from_template(t: &LaunchTemplate) -> Self {
        let mut fields = t.fields.clone();
        fields.kind = fields.kind.or(fields.implied_kind());
        Self {
            name: t.name.clone(),
            fields,
        }
    }

    /// Refuse incompatible or unfinished fields without requiring a complete
    /// launch. Another stacked template may supply the missing agent or command.
    fn refusal(&self, catalog: Option<&[LaunchCatalogModel]>) -> Option<String> {
        for field in Field::ALL.into_iter().filter(|k| k.is_set(&self.fields)) {
            if let Some(message) = field.refusal(&self.fields, catalog) {
                return Some(format!("{}: {message}", field.label()));
            }
        }
        let f = &self.fields;
        for (label, value) in [
            ("command", f.command.as_deref()),
            ("model", f.model.as_ref().and_then(|v| v.as_deref())),
            (
                "resume command",
                f.resume_command.as_ref().and_then(|v| v.as_deref()),
            ),
            ("host", f.host.as_deref()),
            ("session name", f.name.as_deref()),
            (
                "destination",
                f.destination.as_ref().map(|v| match v {
                    TemplateDestination::Folder(s) | TemplateDestination::Github(s) => s.as_str(),
                }),
            ),
        ] {
            if value == Some("") {
                let verb = if label == "host" { "choose" } else { "type" };
                return Some(format!("{verb} the {label}, or remove its field"));
            }
        }
        None
    }
}

/// What the panel knows about the helm's template names at the moment of a
/// save.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ListedNames<'a> {
    /// The list is being read: the first load, or a reload after a save, a
    /// delete or a retry. A reload keeps showing the previous list until it
    /// finishes, but that list may already be out of date (it lacks the
    /// template just saved), so it proves nothing about a name.
    Loading,
    /// The last read failed.
    Failed,
    /// The list as last read, with no read in progress.
    Loaded(Vec<&'a str>),
}

/// Why saving the form under `name` must not go ahead, or `None` when it may.
///
/// `previous` is the name of the template the form was opened from (`None`
/// for a new one), and `listed` is what the panel knows about the names.
///
/// Saving under the template's own name replaces that template, which is
/// the point of editing it (last write wins, SPEC.md). Any other save, a new
/// template or a rename, would silently replace whichever template already
/// has `name`, which SPEC_impl.md ("Templates") says the panel must refuse.
/// So it needs a freshly loaded list to show that the name is free: a list
/// being read, or one that failed to load, proves nothing, and treating it
/// as "not taken" is how a slow or failed read used to let a save overwrite
/// another template.
fn save_name_refusal(previous: Option<&str>, name: &str, listed: &ListedNames) -> Option<String> {
    if previous == Some(name) {
        return None;
    }
    match listed {
        ListedNames::Loading => Some(
            "the template list is still loading, so Farhelm cannot check that no other template has this name; \
             wait for it to load, then save again"
                .to_string(),
        ),
        ListedNames::Failed => Some(
            "the template list could not be loaded, so Farhelm cannot check that no other template has this name; \
             retry loading it above, then save again"
                .to_string(),
        ),
        ListedNames::Loaded(names) => names.contains(&name).then(|| {
            format!("a template named {name:?} already exists; edit that one, or choose another name")
        }),
    }
}

/// Give the launcher the editor's compatibility and unfinished-field rules.
/// Saving a partial setup need not make a complete launch, but must not store
/// a field combination the same editor would refuse.
pub(super) fn saved_fields_refusal(
    template: &LaunchTemplate,
    catalog: Option<&[LaunchCatalogModel]>,
) -> Option<String> {
    Draft {
        name: template.name.clone(),
        fields: template.fields.clone(),
    }
    .refusal(catalog)
}

/// Check a new launcher save against the freshly read catalog of names.
/// A failed read cannot prove a name is free, and must never enable a write.
pub(super) fn new_name_refusal(name: &str, templates: Option<&[LaunchTemplate]>) -> Option<String> {
    let names = match templates {
        Some(templates) => ListedNames::Loaded(templates.iter().map(|t| t.name.as_str()).collect()),
        None => ListedNames::Failed,
    };
    save_name_refusal(None, name, &names)
}

/// A one-line description of what a template sets, for its row in the
/// list: every field it sets, a reset shown as such. The command and resume
/// text are left out (they can be long, and the edit form shows them).
pub(super) fn template_summary(fields: &TemplateFields) -> String {
    let choice = |name: &str, value: Option<String>| match value {
        Some(value) => format!("{name} {value}"),
        None => format!("{name} default"),
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
            None => "model default".to_string(),
        });
    }
    if let Some(effort) = fields.effort {
        parts.push(choice("effort", effort.map(word)));
    }
    if let Some(permissions) = fields.permissions {
        parts.push(choice("approvals", permissions.map(word)));
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
                "resume default"
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

/// One native select, including an older incompatible value until the user
/// corrects it. Dropping an unknown option would visually claim a different
/// choice while the draft still held the old one. Its explicit accessible name
/// stays independent of the option text nested inside the visible label.
/// Options also mark the saved selection: a select's initial value can be applied
/// before its options mount, which otherwise leaves the first option displayed.
#[component]
fn Choice(
    label: String,
    value: String,
    mut options: Vec<(String, String)>,
    disabled: bool,
    on_change: EventHandler<String>,
) -> Element {
    if !options.iter().any(|(key, _)| key == &value) {
        options.push((value.clone(), format!("{value} (not offered)")));
    }
    rsx! {
        label { class: "templates-field",
            span { "{label}" }
            select { disabled, value: "{value}", aria_label: "{label}", "data-tooltip": "{label}: the value this template applies",
                onchange: move |event| on_change.call(event.value()),
                for (key, text) in options { option { value: "{key}", selected: key == value, "{text}" } }
            }
        }
    }
}

/// A present field's controls, independent of list navigation and save state.
/// Changes only update the draft; they never clear another field to make it fit.
#[component]
fn FieldEditor(
    field: Field,
    mut draft: Signal<Draft>,
    hosts: Vec<HostOption>,
    catalog: Option<Vec<LaunchCatalogModel>>,
    disabled: bool,
) -> Element {
    let current = draft().fields;
    let refusal = field.refusal(&current, catalog.as_deref());
    let label = field.label();
    let controls = match field {
        Field::Agent => rsx! {
            Choice { label, value: current.agent.map(word).unwrap_or_default(),
                options: LaunchHarness::ALL.iter().map(|a| (word(*a), crate::launch_composer::harness_label(*a).to_string())).collect(), disabled,
                on_change: move |v: String| draft.with_mut(|d| d.fields.agent = from_word(&v)),
            }
        },
        Field::Model => {
            let value = current.model.clone().flatten().unwrap_or_default();
            // Show alternatives even for a saved or custom model. Filtering by
            // the current value would hide the catalog unless the user cleared it.
            let suggestions: Vec<_> = current
                .agent
                .map(|a| {
                    crate::launch_composer::model_options(
                        catalog.as_deref().unwrap_or_default(),
                        Some(a),
                        "",
                        false,
                    )
                    .into_iter()
                    .filter_map(|option| match option {
                        crate::launch_composer::ModelOption::Model { id, .. } => Some(id),
                        _ => None,
                    })
                    .collect()
                })
                .unwrap_or_default();
            rsx! {
                label { class: "templates-field",
                    span { "model" }
                    input { class: "templates-model", r#type: "text", value: "{value}", disabled, placeholder: "default, or type any model",
                        "data-tooltip": "model: type any model; suggestions follow the agent type this template sets",
                        oninput: move |event| draft.with_mut(|d| d.fields.model = Some((!event.value().is_empty()).then(|| event.value()))),
                    }
                }
                if !suggestions.is_empty() {
                    div { class: "templates-model-suggestions", role: "group", aria_label: "model suggestions",
                        for id in suggestions {
                            button { r#type: "button", class: "templates-suggestion", disabled,
                                "data-tooltip": "set model to {id}", onclick: move |_| draft.with_mut(|d| d.fields.model = Some(Some(id.clone()))), "{id}"
                            }
                        }
                    }
                }
                span { class: "templates-hint", "An empty input sets the agent's default model. Any other model text is kept." }
            }
        }
        Field::Effort => rsx! {
            Choice { label, value: current.effort.flatten().map(word).unwrap_or_else(|| DEFAULT.into()), options: effort_options(&current, catalog.as_deref()), disabled,
                on_change: move |v: String| draft.with_mut(|d| d.fields.effort = Some(from_word(&v))),
            }
        },
        Field::Permissions => rsx! {
            Choice { label, value: current.permissions.flatten().map(word).unwrap_or_else(|| DEFAULT.into()), options: permission_options(current.agent), disabled,
                on_change: move |v: String| draft.with_mut(|d| d.fields.permissions = Some(from_word(&v))),
            }
        },
        Field::Trust => rsx! {
            Choice { label, value: current.workspace_trust.flatten().map(|v| v.to_string()).unwrap_or_else(|| DEFAULT.into()),
                options: vec![(DEFAULT.into(), "default".into()), ("true".into(), "trusted".into()), ("false".into(), "not trusted".into())], disabled,
                on_change: move |v: String| draft.with_mut(|d| d.fields.workspace_trust = Some(v.parse().ok())),
            }
        },
        Field::Yolo => rsx! {
            Choice { label, value: current.yolo.map(|v| v.to_string()).unwrap_or_default(),
                options: vec![("false".into(), "asks for approval".into()), ("true".into(), "runs without prompts (YOLO)".into())], disabled,
                on_change: move |v: String| draft.with_mut(|d| d.fields.yolo = v.parse().ok()),
            }
            span { class: "templates-hint", "Your statement about this command; Farhelm does not inspect it." }
        },
        Field::Command => rsx! {
            label { class: "templates-field",
                span { "command" }
                input { class: "templates-command templates-code", value: current.command.unwrap_or_default(), disabled, placeholder: "agent-tool {{farhelm_args}}",
                    "data-tooltip": "command: the command this template puts in the launcher", oninput: move |e| draft.with_mut(|d| d.fields.command = Some(e.value())),
                }
            }
            span { class: "templates-hint", "Placeholders: {{cwd}} for the folder and {{farhelm_args}} for Farhelm's integration arguments; each must be a whole argument." }
        },
        Field::Resume => rsx! {
            label { class: "templates-field",
                span { "resume command" }
                input { class: "templates-resume templates-code", value: current.resume_command.flatten().unwrap_or_default(), disabled, placeholder: "default, or agent-tool --resume {{conversation}} {{farhelm_args}}",
                    "data-tooltip": "resume command: an empty input resets it to the default", oninput: move |e| draft.with_mut(|d| d.fields.resume_command = Some((!e.value().is_empty()).then(|| e.value()))),
                }
            }
            span { class: "templates-hint", "An empty input sets the default. Placeholders: {{cwd}}, {{farhelm_args}}, {{conversation}}; each must be a whole argument." }
        },
        Field::Host => {
            let value = current.host.unwrap_or_default();
            let mut options: Vec<_> = hosts
                .into_iter()
                .filter_map(|h| h.identity.map(|id| (id, display_peer(&h.name))))
                .collect();
            if value.is_empty() {
                options.insert(0, (String::new(), "choose a host".into()));
            } else if !options.iter().any(|(id, _)| id == &value) {
                options.push((value.clone(), "an install no host here has".into()));
            }
            rsx! { Choice { label, value, options, disabled, on_change: move |v: String| draft.with_mut(|d| d.fields.host = Some(v)) } }
        }
        Field::Name => rsx! {
            label { class: "templates-field",
                span { "session name" }
                input { class: "templates-session-name", value: current.name.unwrap_or_default(), disabled, placeholder: "review",
                    "data-tooltip": "session name: the name this template puts in the launcher", oninput: move |e| draft.with_mut(|d| d.fields.name = Some(e.value())),
                }
            }
        },
        Field::Destination => {
            let (github, value) = match current.destination {
                Some(TemplateDestination::Github(v)) => (true, v),
                Some(TemplateDestination::Folder(v)) => (false, v),
                None => (false, String::new()),
            };
            rsx! {
                div { class: "templates-field",
                    span { "destination" }
                    div { class: "templates-segments", role: "group", aria_label: "destination",
                        for (is_github,text) in [(false,"folder"),(true,"managed checkout")] {
                            button { r#type: "button", disabled, aria_pressed: is_github == github, "data-tooltip": "destination: {text}",
                                onclick: move |_| draft.with_mut(|d| { let value=match d.fields.destination.take() { Some(TemplateDestination::Folder(v) | TemplateDestination::Github(v)) => v, None => String::new() }; d.fields.destination=Some(if is_github { TemplateDestination::Github(value) } else { TemplateDestination::Folder(value) }); }), "{text}"
                            }
                        }
                    }
                    label {
                        span { if github { "repository" } else { "folder path" } }
                        input { class: "templates-destination", value, disabled, placeholder: if github { "owner/name" } else { "/path/to/project" },
                            "data-tooltip": if github { "repository: each session gets a managed checkout" } else { "folder path: the working folder on the selected host" },
                            oninput: move |e| draft.with_mut(|d| d.fields.destination=Some(if github { TemplateDestination::Github(e.value()) } else { TemplateDestination::Folder(e.value()) })),
                        }
                    }
                }
                if github { span { class: "templates-hint", "Each session gets a managed checkout of this repository." } }
            }
        }
    };
    rsx! {
        div { class: if refusal.is_some() { "templates-set-field templates-incompatible" } else { "templates-set-field" }, "data-field": field.key(),
            div { class: "templates-field-controls", {controls}
                if let Some(ref message)=refusal { p { class: "templates-error", "{message}" } }
            }
            button { class: "templates-remove", r#type: "button", disabled, aria_label: "remove {label}", "data-tooltip": "remove {label}: this template will leave it as is",
                onclick: move |_| draft.with_mut(|d| field.remove(&mut d.fields)), "✕"
            }
        }
    }
}

/// All editor departure paths share the same guard, including mobile Back and
/// Escape. A pending action carries its destination rather than losing it when
/// the user saves from the inline prompt.
#[derive(Clone, PartialEq)]
enum Departure {
    Open(LaunchTemplate),
    New,
    Duplicate,
    Back,
    Close,
}

/// A free copy name is only a draft suggestion; the save still checks the
/// loaded list. Truncate at UTF-8 boundaries to respect the API's byte limit.
fn duplicate_name(name: &str, names: &[String]) -> String {
    for number in 1.. {
        let suffix = if number == 1 {
            " copy".to_string()
        } else {
            format!(" copy {number}")
        };
        let mut end = name.len().min(TEMPLATE_NAME_CAP - suffix.len());
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        let candidate = format!("{}{suffix}", &name[..end]);
        if !names.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

/// Apply an approved departure only after the dirty draft has been handled.
/// Duplicate copies the saved baseline, so discard cannot sneak unsaved edits
/// into a new template. The caller refreshes that baseline before save-and-leave.
#[allow(clippy::too_many_arguments)]
fn leave_editor(
    action: Departure,
    mut draft: Signal<Draft>,
    mut baseline: Signal<Draft>,
    mut previous: Signal<Option<String>>,
    mut opened: Signal<bool>,
    mut pending: Signal<Option<Departure>>,
    mut error: Signal<Option<String>>,
    names: Vec<String>,
    on_close: EventHandler<()>,
) {
    let focus = match &action {
        Departure::Back => Some(".templates-new"),
        Departure::Close => None,
        _ => Some(".templates-name"),
    };
    pending.set(None);
    error.set(None);
    match action {
        Departure::Open(template) => {
            baseline.set(Draft {
                name: template.name.clone(),
                fields: template.fields.clone(),
            });
            draft.set(Draft::from_template(&template));
            previous.set(Some(template.name));
            opened.set(true);
        }
        Departure::New => {
            draft.set(Draft::default());
            baseline.set(Draft::default());
            previous.set(None);
            opened.set(true);
        }
        Departure::Duplicate => {
            let stored = baseline();
            let mut copy = Draft::from_template(&LaunchTemplate {
                name: stored.name,
                fields: stored.fields,
            });
            copy.name = duplicate_name(&copy.name, &names);
            // The source is already stored; a new copy must be saved explicitly
            // even if none of its values has been edited yet.
            baseline.set(Draft::default());
            draft.set(copy);
            previous.set(None);
            opened.set(true);
        }
        Departure::Back => {
            opened.set(false);
            draft.set(Draft::default());
            baseline.set(Draft::default());
            previous.set(None);
        }
        Departure::Close => on_close.call(()),
    }
    if let Some(selector) = focus {
        focus_template_control(selector);
    }
}

/// A deleted template lives only in this mounted dialog. The generation keeps
/// an older expiry timer from clearing a later deletion's undo notice.
#[derive(Clone, PartialEq)]
struct Undo {
    template: LaunchTemplate,
    generation: u64,
}

/// Consult the resource's pending state before its retained value: a reload
/// still displays an old list, which cannot establish that a new name is free.
fn name_refusal(
    templates: Resource<Result<Vec<LaunchTemplate>, String>>,
    previous: Option<&str>,
    name: &str,
) -> Option<String> {
    let listed = templates.peek();
    let names = match listed.as_ref() {
        _ if templates.pending() => ListedNames::Loading,
        None => ListedNames::Loading,
        Some(Err(_)) => ListedNames::Failed,
        Some(Ok(list)) => ListedNames::Loaded(list.iter().map(|t| t.name.as_str()).collect()),
    };
    save_name_refusal(previous, name, &names)
}

/// Manage the same modal lifecycle as the launcher expects, with one local
/// draft and one serialized write at a time. Async save/delete/undo work keeps
/// navigation disabled until its outcome is known; failed writes keep the draft.
#[component]
pub(super) fn TemplatesDialog(
    hosts: Vec<HostOption>,
    on_close: EventHandler<()>,
    /// A successful launcher save opens its actual returned fields directly;
    /// the asynchronous list read is not needed to rediscover the new record.
    initial_template: Option<LaunchTemplate>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let list_base = base.clone();
    let mut templates = use_resource(move || {
        let base = list_base.clone();
        async move { crate::api::fetch_templates(&base).await }
    });
    let catalog_base = base.clone();
    let catalog = use_resource(move || {
        let base = catalog_base.clone();
        async move { crate::api::fetch_launch_catalog(&base).await }
    });
    let initial_draft = initial_template
        .as_ref()
        .map(Draft::from_template)
        .unwrap_or_default();
    let initial_baseline = initial_draft.clone();
    let mut draft = use_signal(move || initial_draft);
    let mut baseline = use_signal(move || initial_baseline);
    let mut previous = use_signal(|| initial_template.as_ref().map(|t| t.name.clone()));
    let opened = use_signal(|| initial_template.is_some());
    let mut pending = use_signal(|| None::<Departure>);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let mut add_open = use_signal(|| false);
    let mut undo = use_signal(|| None::<Undo>);
    let mut undo_generation = use_signal(|| 0_u64);
    use_effect(move || {
        let writing = busy();
        let loading = templates.pending();
        if !writing && !loading {
            restore_template_focus();
        }
    });
    let current = draft();
    let dirty = opened() && current != baseline();
    let catalog_rows = catalog
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let list = templates.read().clone();
    let names: Vec<String> = list
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|rows| rows.iter().map(|t| t.name.clone()).collect())
        .unwrap_or_default();
    let departure_names = names.clone();
    let depart: EventHandler<Departure> = EventHandler::new(move |action| {
        if busy() {
            return;
        }
        // Rows retain a stored snapshot while an editor holds newer values.
        // Reselecting the source must not discard edits or reopen a deleted name.
        if let Departure::Open(template) = &action
            && previous().as_deref() == Some(template.name.as_str())
        {
            return;
        }
        add_open.set(false);
        if opened() && draft() != baseline() {
            pending.set(Some(action));
        } else {
            leave_editor(
                action,
                draft,
                baseline,
                previous,
                opened,
                pending,
                error,
                departure_names.clone(),
                on_close,
            );
        }
    });

    let save_base = base.clone();
    let save: EventHandler<Option<Departure>> = EventHandler::new(move |after| {
        if busy() {
            return;
        }
        let value = draft();
        let catalog_value = catalog.peek();
        let catalog_rows = catalog_value
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(Vec::as_slice);
        // Match the API's shape refusals locally, especially the empty name:
        // its PUT path cannot otherwise reach the template handler at all.
        let shape_refusal = check_template_shape(&LaunchTemplate {
            name: value.name.clone(),
            fields: value.fields.clone(),
        })
        .err();
        if let Some(message) = shape_refusal
            .or_else(|| value.refusal(catalog_rows))
            .or_else(|| name_refusal(templates, previous().as_deref(), &value.name))
        {
            error.set(Some(message));
            return;
        }
        let old = previous();
        let base = save_base.clone();
        busy.set(true);
        error.set(None);
        spawn(async move {
            let result = crate::api::put_template(&base, &value.name, &value.fields).await;
            let result = match (result, old) {
                (Ok(()), Some(old)) if old != value.name => {
                    crate::api::delete_template(&base, &old).await
                }
                (result, _) => result,
            };
            busy.set(false);
            match result {
                Ok(()) => {
                    baseline.set(value.clone());
                    previous.set(Some(value.name.clone()));
                    let mut names = templates
                        .peek()
                        .as_ref()
                        .and_then(|r| r.as_ref().ok())
                        .map(|rows| rows.iter().map(|t| t.name.clone()).collect::<Vec<_>>())
                        .unwrap_or_default();
                    if !names.contains(&value.name) {
                        names.push(value.name.clone());
                    }
                    templates.restart();
                    if let Some(action) = after {
                        leave_editor(
                            action, draft, baseline, previous, opened, pending, error, names,
                            on_close,
                        );
                    } else {
                        pending.set(None);
                    }
                }
                Err(message) => error.set(Some(message)),
            }
        });
    });

    let delete_base = base.clone();
    let delete = move |_| {
        if busy() {
            return;
        }
        let Some(name) = previous() else {
            return;
        };
        let Some(template) = templates
            .peek()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .and_then(|rows| rows.iter().find(|t| t.name == name))
            .cloned()
        else {
            error.set(Some(
                "reload the template list before deleting this template".into(),
            ));
            return;
        };
        let base = delete_base.clone();
        busy.set(true);
        error.set(None);
        spawn(async move {
            match crate::api::delete_template(&base, &name).await {
                Ok(()) => {
                    undo_generation += 1;
                    let generation = undo_generation();
                    undo.set(Some(Undo {
                        template,
                        generation,
                    }));
                    leave_editor(
                        Departure::Back,
                        draft,
                        baseline,
                        previous,
                        opened,
                        pending,
                        error,
                        Vec::new(),
                        on_close,
                    );
                    templates.restart();
                    // This is the user-visible undo window, not a readiness
                    // wait. Dioxus cancels the task when this dialog unmounts.
                    spawn(async move {
                        crate::reader::sleep_ms(10_000).await;
                        if undo
                            .peek()
                            .as_ref()
                            .is_some_and(|v| v.generation == generation)
                        {
                            undo.set(None);
                        }
                    });
                }
                Err(message) => error.set(Some(message)),
            }
            busy.set(false);
        });
    };
    let undo_base = base.clone();
    let restore = move |_| {
        if busy() {
            return;
        }
        let Some(value) = undo() else {
            return;
        };
        // Re-read names at undo time; the list after deletion alone cannot
        // see a name another client took during the ten-second window.
        let base = undo_base.clone();
        busy.set(true);
        error.set(None);
        spawn(async move {
            let result = match crate::api::fetch_templates(&base).await {
                Ok(rows) => {
                    let names = ListedNames::Loaded(rows.iter().map(|t| t.name.as_str()).collect());
                    match save_name_refusal(None, &value.template.name, &names) {
                        Some(message) => Err(message),
                        None => {
                            crate::api::put_template(
                                &base,
                                &value.template.name,
                                &value.template.fields,
                            )
                            .await
                        }
                    }
                }
                Err(message) => Err(format!(
                    "cannot check the deleted template's name: {message}"
                )),
            };
            busy.set(false);
            match result {
                Ok(()) => {
                    undo.set(None);
                    templates.restart();
                }
                Err(message) => error.set(Some(message)),
            }
        });
    };

    let disabled = busy();
    let prompt = pending();
    let inferred =
        previous().is_some() && baseline().fields.kind.is_none() && current.fields.kind.is_some();
    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            div { class: if opened() { "templates-dialog templates-editor-open" } else { "templates-dialog" }, role: "dialog", aria_modal: "true", aria_label: "templates", tabindex: "-1",
                onmounted: move |_| install_dialog_with_selector(DIALOG_SELECTOR,".templates-new",Some(".templates-close")),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key()==Key::Escape && !event.is_composing() {
                        event.stop_propagation();
                        if add_open() { add_open.set(false); focus_template_control(".templates-add"); }
                        else { depart.call(Departure::Close); }
                    }
                },
                div { class: "templates-header",
                    h2 { class: "host-settings-title", "templates" }
                    button { r#type: "button", class: "btn btn-neutral templates-close", disabled, "data-tooltip": "close templates; unsaved changes ask before leaving", onclick: move |_| depart.call(Departure::Close), "close" }
                }
                p { class: "host-settings-help templates-help",
                    "A template is a named set of launcher edits. In New session, type tl:name to apply one; it changes only the fields it sets. Templates stack, the later one winning: tl:my-claude then tl:webbuilder starts Claude in a fresh acme/web checkout on build-box."
                }
                div { class: "templates-panes",
                    aside { class: "templates-sidebar", aria_label: "template list", aria_busy: templates.pending(),
                        button { r#type: "button", class: "btn btn-neutral templates-new", disabled, "data-tooltip": "start a new template; unsaved changes ask before leaving", onclick: move |_| depart.call(Departure::New), "+ new template" }
                        match list.clone() {
                            None => rsx! { p { class: "templates-hint", "loading templates" } },
                            Some(Err(_)) => rsx! {},
                            Some(Ok(rows)) => rsx! {
                                ul { class: "templates-list",
                                    for template in rows {
                                        li { key: "{template.name}",
                                            button { r#type: "button", class: "templates-row", disabled: disabled || templates.pending(), aria_pressed: previous().as_ref()==Some(&template.name), "data-tooltip": "edit template {display_peer(&template.name)}", onclick: move |_| depart.call(Departure::Open(template.clone())),
                                                span { class: "templates-row-name peer-value", dir: "ltr", "{display_peer(&template.name)}" }
                                                span { class: "templates-row-summary", "{template_summary(&template.fields)}" }
                                            }
                                        }
                                    }
                                }
                            },
                        }
                        // A failed or absent read is unknown, not an empty list.
                        if let Some(Ok(rows)) = &list {
                            p { class: "templates-count", "{rows.len()} " if rows.len() == 1 { "template" } else { "templates" } }
                        }
                    }
                    section { class: "templates-editor", aria_label: "template editor",
                        if opened() {
                            button { r#type: "button", class: "templates-back", disabled, "data-tooltip": "back to templates; unsaved changes ask before leaving", onclick: move |_| depart.call(Departure::Back), "‹ templates" }
                            div { class: "templates-editor-heading",
                                h3 { if previous().is_some() { "edit template" } else { "new template" } }
                                if dirty { span { class: "templates-unsaved", "unsaved changes" } }
                            }
                            label { class: "templates-field",
                                span { "name" }
                                input { class: "templates-name", value: "{current.name}", disabled, placeholder: "my-claude", "data-tooltip": "template name: use tl:name to apply it in New session", oninput: move |e| draft.with_mut(|d| d.name=e.value()) }
                            }
                            div { class: "templates-field",
                                span { "switches launcher to" }
                                div { class: "templates-segments", role: "group", aria_label: "switches launcher to",
                                    for (kind,text) in [(Some(LauncherKind::Agent),"agent"),(Some(LauncherKind::Command),"command"),(None,"don't switch")] {
                                        button { r#type: "button", disabled, aria_pressed: current.fields.kind==kind, "data-tooltip": "switches launcher to: {text}; incompatible fields stay visible", onclick: move |_| { draft.with_mut(|d| d.fields.kind=kind); add_open.set(false); }, "{text}" }
                                    }
                                }
                            }
                            if inferred { p { class: "templates-hint templates-inferred", "This stored template does not switch the launcher yet. Saving will store the switch inferred from its fields." } }
                            h4 { class: "templates-sets-heading", "sets" }
                            for field in Field::ALL.into_iter().filter(|f| f.is_set(&current.fields)) {
                                FieldEditor { key: "{field.key()}", field, draft, hosts: hosts.clone(), catalog: catalog_rows.clone(), disabled }
                            }
                            div { class: "templates-add-area",
                                button { r#type: "button", class: "btn btn-neutral templates-add", disabled, aria_expanded: add_open(), "data-tooltip": "add a field this template will set; fields not shown are left as is", onclick: move |_| add_open.toggle(), "+ add field" }
                                if add_open() {
                                    div { class: "templates-add-menu", role: "group", aria_label: "add field",
                                        for field in Field::ALL.into_iter().filter(|f| !f.is_set(&current.fields) && f.offered(&current.fields)) {
                                            button { r#type: "button", disabled, "data-tooltip": "add {field.label()} to the template", onclick: move |_| { draft.with_mut(|d| field.add(&mut d.fields)); add_open.set(false); }, "{field.label()}" }
                                        }
                                    }
                                }
                            }
                            p { class: "templates-hint", "Fields not shown are left as is. Another template can supply choices this one leaves out." }
                            footer { class: "templates-editor-footer",
                                if let Some(action)=prompt.clone() {
                                    div { class: "templates-departure", role: "status",
                                        onmounted: move |_| { document::eval("requestAnimationFrame(() => { const prompt = document.querySelector('.templates-departure'); prompt?.scrollIntoView({block: 'nearest'}); prompt?.querySelector('.templates-save-leave')?.focus({preventScroll: true}); })"); },
                                        p { "Save these changes before leaving?" }
                                        div { class: "templates-actions",
                                            button { r#type: "button", class: "btn btn-primary templates-save-leave", disabled, "data-tooltip": "save these changes, then continue", onclick: { let action=action.clone(); move |_| save.call(Some(action.clone())) }, "save" }
                                            button { r#type: "button", class: "btn btn-neutral templates-discard", disabled, "data-tooltip": "discard these changes, then continue", onclick: { let names=names.clone(); move |_| leave_editor(action.clone(),draft,baseline,previous,opened,pending,error,names.clone(),on_close) }, "discard" }
                                            button { r#type: "button", class: "btn btn-neutral templates-keep-editing", disabled, "data-tooltip": "stay here with the unsaved draft", onclick: move |_| { pending.set(None); focus_template_control(".templates-name"); }, "keep editing" }
                                        }
                                    }
                                } else {
                                    div { class: "templates-actions",
                                        if previous().is_some() {
                                            button { r#type: "button", class: "btn btn-neutral templates-duplicate", disabled: disabled || templates.pending(), "data-tooltip": "open a new unsaved copy with a free name", onclick: move |_| depart.call(Departure::Duplicate), "duplicate" }
                                            button { r#type: "button", class: "btn btn-danger templates-delete", disabled: disabled || templates.pending(), "data-tooltip": "delete this template now; undo is available here for about ten seconds", onclick: delete, "delete" }
                                        }
                                        button { r#type: "button", class: "btn btn-primary templates-save", disabled, "data-tooltip": "save the template's name, switch and shown fields", onclick: move |_| save.call(None), "save" }
                                    }
                                }
                            }
                        } else { p { class: "templates-empty", "Choose a template to edit, or make a new one." } }
                    }
                }
                // Recovery belongs to both responsive views: the sidebar is
                // hidden while editing on a phone, but its name guard still applies.
                if let Some(Err(message)) = list {
                    div { class: "templates-list-error",
                        p { class: "templates-error", "templates unavailable: {message}" }
                        button { r#type: "button", class: "btn btn-neutral templates-retry", disabled, "data-tooltip": "load the template list again without discarding this draft", onclick: move |_| templates.restart(), "retry" }
                    }
                }
                if let Some(message)=error() { p { class: "templates-error", role: "status", "{message}" } }
                if let Some(value)=undo() {
                    div { class: "templates-undo-notice", role: "status",
                        span { "Deleted {display_peer(&value.template.name)}." }
                        button { r#type: "button", class: "btn btn-neutral templates-undo", disabled, "data-tooltip": "restore the deleted template if its old name is still free", onclick: restore, "undo" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opening a kindless record changes only the draft. It must be visibly
    /// unsaved so simply inspecting a template cannot migrate its behavior.
    #[test]
    fn legacy_kind_is_inferred_without_changing_the_stored_template() {
        for (fields, kind) in [
            (
                TemplateFields {
                    agent: Some(LaunchHarness::Claude),
                    ..Default::default()
                },
                Some(LauncherKind::Agent),
            ),
            (
                TemplateFields {
                    yolo: Some(false),
                    ..Default::default()
                },
                Some(LauncherKind::Command),
            ),
            (
                TemplateFields {
                    resume_command: Some(None),
                    ..Default::default()
                },
                Some(LauncherKind::Command),
            ),
            (
                TemplateFields {
                    name: Some("review".into()),
                    ..Default::default()
                },
                None,
            ),
        ] {
            let stored = LaunchTemplate {
                name: "legacy".into(),
                fields: fields.clone(),
            };
            let draft = Draft::from_template(&stored);
            assert_eq!(stored.fields, fields, "opening is not a write");
            assert_eq!(draft.fields, TemplateFields { kind, ..fields });
            assert_eq!(
                draft
                    != Draft {
                        name: stored.name,
                        fields: stored.fields
                    },
                kind.is_some()
            );
        }
    }

    /// A don't-switch template cannot hide launch-specific edits inside a
    /// placement template, and a switch change preserves the old value until
    /// its author removes it rather than silently making a different template.
    #[test]
    fn switches_offer_their_fields_and_flag_retained_incompatible_values() {
        let placement = TemplateFields::default();
        let offered: Vec<_> = Field::ALL
            .into_iter()
            .filter(|f| f.offered(&placement))
            .collect();
        assert_eq!(offered, vec![Field::Host, Field::Destination, Field::Name]);
        let mut draft = Draft {
            name: "command".into(),
            fields: TemplateFields {
                kind: Some(LauncherKind::Command),
                command: Some("tool {farhelm_args}".into()),
                yolo: Some(false),
                ..Default::default()
            },
        };
        assert!(Field::Resume.offered(&draft.fields));
        assert!(!Field::Effort.offered(&draft.fields));
        assert_eq!(
            draft.refusal(None),
            None,
            "a template need not name an agent"
        );
        draft.fields.kind = Some(LauncherKind::Agent);
        assert!(Field::Model.offered(&draft.fields));
        assert!(draft.refusal(None).unwrap().contains("command"));
        assert_eq!(draft.fields.command.as_deref(), Some("tool {farhelm_args}"));
        Field::Command.remove(&mut draft.fields);
        Field::Yolo.remove(&mut draft.fields);
        assert_eq!(draft.refusal(None), None);
    }

    /// Agent-specific approval vocabularies must not leak into another agent's
    /// template. Model text is intentionally exempt: the editor accepts custom
    /// ids while the launcher retains its own application rules.
    #[test]
    fn agent_changes_flag_approvals_but_keep_arbitrary_models() {
        let mut draft = Draft {
            name: "agent".into(),
            fields: TemplateFields {
                kind: Some(LauncherKind::Agent),
                agent: Some(LaunchHarness::Goose),
                permissions: Some(Some(LaunchPermission::SmartApprove)),
                model: Some(Some("any future model".into())),
                ..Default::default()
            },
        };
        assert_eq!(draft.refusal(None), None);
        draft.fields.agent = Some(LaunchHarness::Claude);
        assert!(draft.refusal(None).unwrap().contains("approvals"));
        assert_eq!(
            draft.fields.permissions,
            Some(Some(LaunchPermission::SmartApprove))
        );
        draft.fields.permissions = Some(None);
        assert_eq!(draft.refusal(None), None);
        assert_eq!(
            permission_options(Some(LaunchHarness::Claude)),
            vec![
                ("default".into(), "default (asks)".into()),
                ("yolo".into(), "YOLO".into())
            ]
        );
        assert!(
            permission_options(Some(LaunchHarness::Omp))
                .iter()
                .any(|(key, _)| key == "approve")
        );
        assert!(
            !permission_options(Some(LaunchHarness::Omp))
                .iter()
                .any(|(key, _)| key == "smart_approve")
        );
        draft.fields.agent = Some(LaunchHarness::Pi);
        assert!(!Field::Permissions.offered(&draft.fields));
        assert!(draft.refusal(None).is_some());
        Field::Permissions.remove(&mut draft.fields);
        assert_eq!(draft.refusal(None), None);
    }

    /// Presence must survive editing for null resets and false approvals alike.
    /// Removing a field makes it absent; adding it again does not recover an
    /// older incompatible value or require any other launcher field.
    #[test]
    fn field_presence_preserves_resets_false_and_values() {
        let fields = TemplateFields {
            kind: Some(LauncherKind::Command),
            yolo: Some(false),
            resume_command: Some(None),
            destination: Some(TemplateDestination::Github("acme/web".into())),
            ..Default::default()
        };
        let template = LaunchTemplate {
            name: "roundtrip".into(),
            fields: fields.clone(),
        };
        let mut draft = Draft::from_template(&template);
        assert_eq!(draft.fields, fields);
        assert!(Field::Yolo.is_set(&draft.fields));
        assert!(Field::Resume.is_set(&draft.fields));
        Field::Resume.remove(&mut draft.fields);
        assert!(!Field::Resume.is_set(&draft.fields));
        Field::Resume.add(&mut draft.fields);
        assert_eq!(draft.fields.resume_command, Some(None));
        assert_eq!(draft.refusal(None), None);
        assert_eq!(draft.fields.kind, Some(LauncherKind::Command));
    }

    /// Copy names fit the API's byte limit even for multibyte names. They do
    /// not replace an existing copy, and remain mere suggestions until save.
    #[test]
    fn duplicate_names_are_free_and_fit_the_name_limit() {
        assert_eq!(
            duplicate_name("build", &["build copy".into(), "build copy 2".into()]),
            "build copy 3"
        );
        let copy = duplicate_name(&"界".repeat(42), &[]);
        assert!(copy.len() <= 128);
        assert!(copy.ends_with(" copy"));
    }
    /// Spec: a new template, or a rename, is saved only once the template
    /// list has loaded with no read in progress and shows the name is free;
    /// while the list is loading (the first load or a reload) or failed to
    /// load, the save is refused saying so. Saving the template already open
    /// under its own name is never refused.
    ///
    /// Why: the panel used to read an unloaded list as "no template has this
    /// name", so saving a new template while the list was slow or failing
    /// replaced an existing template the user never opened (SPEC_impl.md
    /// "Templates" requires refusing a name another template has). A reload
    /// counts as loading because the list it still shows can lack the
    /// template just saved; the save handler maps a reload to `Loading`.
    #[test]
    fn a_new_or_renamed_template_needs_the_loaded_list() {
        let loaded = ListedNames::Loaded(vec!["build", "review"]);
        let taken =
            |refusal: Option<String>| refusal.is_some_and(|text| text.contains("already exists"));
        let loading =
            |refusal: Option<String>| refusal.is_some_and(|text| text.contains("still loading"));
        let failed = |refusal: Option<String>| {
            refusal.is_some_and(|text| text.contains("could not be loaded"))
        };

        // A new template: a taken name is refused as taken, a free one is
        // allowed, and either is refused while the list is not loaded.
        assert!(taken(save_name_refusal(None, "build", &loaded)));
        assert_eq!(save_name_refusal(None, "deploy", &loaded), None);
        for name in ["build", "deploy"] {
            assert!(
                loading(save_name_refusal(None, name, &ListedNames::Loading)),
                "{name}"
            );
            assert!(
                failed(save_name_refusal(None, name, &ListedNames::Failed)),
                "{name}"
            );
        }

        // A rename is a new name too.
        assert!(taken(save_name_refusal(Some("build"), "review", &loaded)));
        assert_eq!(save_name_refusal(Some("build"), "deploy", &loaded), None);
        assert!(loading(save_name_refusal(
            Some("build"),
            "deploy",
            &ListedNames::Loading
        )));
        assert!(failed(save_name_refusal(
            Some("build"),
            "deploy",
            &ListedNames::Failed
        )));

        // The template open under its own name saves whatever the list.
        for listed in [loaded.clone(), ListedNames::Loading, ListedNames::Failed] {
            assert_eq!(
                save_name_refusal(Some("build"), "build", &listed),
                None,
                "{listed:?}"
            );
        }
    }
}

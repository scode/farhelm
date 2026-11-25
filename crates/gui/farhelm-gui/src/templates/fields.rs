//! The template editor's model: which fields a template sets, how each is added and removed, which ones fit the
//! template's switch and agent type, and why a draft cannot be saved. No GPUI types.
//!
//! SPEC.md, "Launch templates": "The editor shows only fields the template sets, with labeled controls and explicit
//! add/remove actions"; its "switches launcher to" choice decides which fields are offered; "Incompatible fields stay
//! visible and prevent saving until corrected or removed"; "Fields need not form a complete launch, because another
//! stacked template can supply what they omit" (SPEC_impl.md). Ported from the old app's `Field`, `Draft`,
//! `permission_options`, `effort_options`, `save_name_refusal` and `duplicate_name` (`list/templates.rs` at reference
//! commit `24bc0580`).
//!
//! Here's the TLDR of the shape:
//!
//! - The draft holds the wire fields directly ([`TemplateFields`]), so a field's presence, an explicit `null` reset
//!   and a false YOLO answer survive editing without a second translation model. A field is "set" when it is present,
//!   whatever its value: an empty text is a present field the user has not filled in yet, which refuses saving.
//! - [`Field`] lists the editable fields in the old editor's order, each with its label, what adding it puts in the
//!   draft, what removing it takes out (a session name takes its kind; Approvals takes the custom words), whether the
//!   template's switch and agent type offer it, and why a set one does not fit.
//! - [`Draft`] is one editor's template: its name, its fields, and the wrapper prefix's raw text while it does not
//!   parse (kept so a half-typed quote is never lost). [`Draft::refusal`] is the one reason saving is refused, in the
//!   old order.
//!
//! What this does not decide: whether a template applies to a given launcher (that is
//! `farhelm_proto::launcher::apply_template`, at application), and the helm's own shape check, which every save runs
//! too (`check_template_shape`).

use farhelm_proto::launcher::{
    LaunchCatalogModel, LaunchTemplate, LauncherKind, TEMPLATE_NAME_CAP, TemplateDestination,
    TemplateFields, compatible_efforts,
};
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission};

use super::summary::permission_label;
use crate::components::capitalize_validation;
use crate::host_text::{HostSentence, HostText};
use crate::permission::words::effort_label;

/// A field the editor can show. The order is the old editor's, which is the order the "Sets" list and the add menu
/// use: the command launch's fields first when present, then the agent launch's, then placement and name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    Command,
    /// The command's YOLO assertion. Labelled "Approvals" like [`Field::Permissions`]: the two are one idea for the
    /// user, but separate stored fields, so switching kind can never reinterpret one as the other.
    Yolo,
    Agent,
    Model,
    Effort,
    Permissions,
    Wrapper,
    /// A retired containerized-session choice from an older template: shown so it can be removed, never offered.
    Container,
    Resume,
    Prompt,
    Host,
    Destination,
    Name,
}

impl Field {
    /// Every field, in the editor's order.
    pub const ALL: [Field; 13] = [
        Field::Command,
        Field::Yolo,
        Field::Agent,
        Field::Model,
        Field::Effort,
        Field::Permissions,
        Field::Wrapper,
        Field::Container,
        Field::Resume,
        Field::Prompt,
        Field::Host,
        Field::Destination,
        Field::Name,
    ];

    /// The field's label, the old editor's words.
    pub const fn label(self) -> &'static str {
        match self {
            Field::Prompt => "Initial prompt",
            Field::Agent => "Agent type",
            Field::Model => "Model",
            Field::Effort => "Effort",
            Field::Permissions | Field::Yolo => "Approvals",
            Field::Wrapper => "Wrapper prefix",
            Field::Container => "Containerized",
            Field::Command => "Command",
            Field::Resume => "Resume command",
            Field::Host => "Host",
            Field::Destination => "Destination",
            Field::Name => "Session name",
        }
    }

    /// A stable key for element ids, which tells the two "Approvals" fields apart.
    pub const fn key(self) -> &'static str {
        match self {
            Field::Prompt => "prompt",
            Field::Agent => "agent",
            Field::Model => "model",
            Field::Effort => "effort",
            Field::Permissions => "permissions",
            Field::Wrapper => "wrapper",
            Field::Container => "container",
            Field::Yolo => "yolo",
            Field::Command => "command",
            Field::Resume => "resume",
            Field::Host => "host",
            Field::Destination => "destination",
            Field::Name => "name",
        }
    }

    /// Whether `fields` sets this field. Presence decides, never the value: a reset, an empty text and a false YOLO
    /// answer are all set.
    pub fn is_set(self, fields: &TemplateFields) -> bool {
        match self {
            Field::Prompt => fields.prompt.is_some(),
            Field::Agent => fields.agent.is_some() || fields.custom_agent.is_some(),
            Field::Model => fields.model.is_some(),
            Field::Effort => fields.effort.is_some(),
            Field::Permissions => fields.permissions.is_some(),
            Field::Wrapper => fields.wrapper_prefix.is_some(),
            Field::Container => fields.container.is_some(),
            Field::Command => fields.command.is_some(),
            Field::Yolo => fields.yolo.is_some(),
            Field::Resume => fields.resume_command.is_some(),
            Field::Host => fields.host.is_some() || fields.launch_target().is_some(),
            Field::Destination => fields.destination.is_some(),
            Field::Name => fields.name.is_some(),
        }
    }

    /// Add the field with a starting value, choosing nothing else on the user's behalf (a model or an effort does not
    /// add an agent type). A text starts empty, which refuses saving until it is filled in; a session name starts as a
    /// base name, by having no kind (SPEC.md: "Adding the name starts on base").
    pub fn add(self, fields: &mut TemplateFields) {
        match self {
            Field::Prompt => fields.prompt = Some(String::new()),
            Field::Agent => fields.agent = Some(LaunchHarness::ALL[0]),
            Field::Model => fields.model = Some(None),
            Field::Effort => fields.effort = Some(None),
            Field::Permissions => fields.permissions = Some(None),
            Field::Wrapper => fields.wrapper_prefix = Some(None),
            Field::Container => fields.container = Some(None),
            Field::Command => fields.command = Some(String::new()),
            Field::Yolo => fields.yolo = Some(false),
            Field::Resume => fields.resume_command = Some(None),
            Field::Host => fields.host = Some(String::new()),
            Field::Destination => {
                fields.destination = Some(TemplateDestination::Folder(String::new()));
            }
            Field::Name => {
                fields.name = Some(String::new());
                fields.name_kind = None;
            }
        }
    }

    /// Remove the field: the template then leaves the launcher's value alone, which is not the same as resetting it.
    /// A removed name takes its kind with it (SPEC.md: "removing it removes its kind"), and removed Approvals take the
    /// custom words that belong to them.
    pub fn remove(self, fields: &mut TemplateFields) {
        match self {
            Field::Prompt => fields.prompt = None,
            Field::Agent => {
                fields.agent = None;
                fields.custom_agent = None;
            }
            Field::Model => fields.model = None,
            Field::Effort => fields.effort = None,
            Field::Permissions => {
                fields.permissions = None;
                fields.permission_flags = None;
            }
            Field::Wrapper => fields.wrapper_prefix = None,
            Field::Container => fields.container = None,
            Field::Command => fields.command = None,
            Field::Yolo => fields.yolo = None,
            Field::Resume => fields.resume_command = None,
            Field::Host => {
                fields.host = None;
                fields.recipe = None;
                fields.target = None;
            }
            Field::Destination => fields.destination = None,
            Field::Name => {
                fields.name = None;
                fields.name_kind = None;
            }
        }
    }

    /// Whether the template's switch and agent type offer the field, for the add menu. "Don't switch" (no kind) offers
    /// only host, destination and session name (SPEC.md); the agent's own capabilities narrow the rest, and with no
    /// agent type every agent-launch choice is offered, since another template may supply the agent. A retired
    /// container field is never offered.
    pub fn offered(self, fields: &TemplateFields) -> bool {
        let agent_kind = fields.kind == Some(LauncherKind::Agent);
        match self {
            Field::Host | Field::Destination | Field::Name => true,
            Field::Agent => fields.kind.is_some(),
            Field::Command | Field::Yolo | Field::Resume => {
                fields.kind == Some(LauncherKind::Command)
            }
            Field::Prompt => {
                agent_kind
                    && fields
                        .agent
                        .is_none_or(|agent| agent.supports_initial_prompt(false))
            }
            Field::Model => agent_kind && fields.agent.is_none_or(LaunchHarness::offers_model),
            Field::Effort => agent_kind && fields.agent.is_none_or(LaunchHarness::offers_effort),
            Field::Permissions => {
                agent_kind
                    && fields
                        .agent
                        .is_none_or(|agent| !agent.is_shell() && !agent.offers_only_yolo())
            }
            Field::Wrapper => agent_kind,
            Field::Container => false,
        }
    }

    /// Why a set field does not fit, or `None` when it does. A field that no longer fits the switch or the agent type
    /// stays visible and refuses saving until it is corrected or removed (SPEC.md). A model of any text fits an agent
    /// launch: the editor never refuses a model the catalog does not list; application and the helm judge it later.
    pub fn refusal(
        self,
        fields: &TemplateFields,
        catalog: Option<&[LaunchCatalogModel]>,
    ) -> Option<String> {
        if self == Field::Container {
            return Some(farhelm_proto::RETIRED_CONTAINER_REASON.to_owned());
        }
        let shell = fields.agent.is_some_and(LaunchHarness::is_shell);
        if self == Field::Agent && shell && fields.kind == Some(LauncherKind::Command) {
            return Some(
                "Bash and Zsh are shell session types. Choose one on the Agent tab.".to_owned(),
            );
        }
        let custom = fields.agent.is_some_and(LaunchHarness::uses_custom_catalog);
        // Switching tabs keeps the selected agent visible. Generic Custom only describes a Command launch.
        if self == Field::Agent
            && custom
            && fields.kind == Some(LauncherKind::Agent)
            && fields.custom_agent.as_ref().is_none_or(Option::is_none)
        {
            return Some("Generic Custom is only for Command templates. Choose a registered or built-in agent.".into());
        }
        if custom {
            let custom_value = match self {
                Field::Model => fields.model.as_ref().and_then(Option::as_deref),
                Field::Effort => fields
                    .effort
                    .as_ref()
                    .and_then(Option::as_ref)
                    .map(|value| value.as_cli_arg()),
                _ => None,
            };
            if let Some(value) = custom_value
                && let Err(reason) = farhelm_proto::custom_agents::validate_model_or_effort(value)
            {
                return Some(reason.to_owned());
            }
        }
        let fits = if self == Field::Model {
            fields.kind == Some(LauncherKind::Agent) && !shell
        } else {
            self.offered(fields)
        };
        if !fits {
            return Some(
                "does not fit this launcher switch or agent type; remove it to save".to_owned(),
            );
        }
        if self == Field::Permissions
            && let Some(Some(chosen)) = fields.permissions
            && !permission_options(fields.agent)
                .iter()
                .any(|(value, _)| *value == Some(chosen))
        {
            return Some(
                "this agent type does not offer these approvals; change or remove them".to_owned(),
            );
        }
        if self == Field::Effort
            && !custom
            && let Some(Some(chosen)) = &fields.effort
            && !chosen.known().is_some_and(|chosen| {
                effort_options(fields, catalog)
                    .iter()
                    .any(|(value, _)| *value == Some(chosen))
            })
        {
            return Some(
                "this agent type or model does not offer this effort; change or remove it"
                    .to_owned(),
            );
        }
        None
    }

    /// Whether the field's stored text holds characters the editor's single-line inputs cannot show or keep, so it is
    /// shown escaped and read-only, its raw value kept through unrelated saves until the field is removed and added
    /// again (SPEC.md for the initial prompt: "Invisible formatting is shown escaped and read-only, retaining its raw
    /// value through unrelated saves; removing and adding the field replaces it"). An initial prompt keeps newline and
    /// tab editable; anything else unsafe makes it read-only too.
    pub fn read_only(self, fields: &TemplateFields) -> bool {
        let unsafe_text = |text: &str, multiline: bool| {
            text.chars().any(|ch| {
                !(multiline && matches!(ch, '\n' | '\t'))
                    && farhelm_proto::text::is_presentation_unsafe(ch)
            })
        };
        match self {
            Field::Name => fields
                .name
                .as_deref()
                .is_some_and(|text| unsafe_text(text, false)),
            Field::Prompt => fields
                .prompt
                .as_deref()
                .is_some_and(|text| unsafe_text(text, true)),
            Field::Command => fields
                .command
                .as_deref()
                .is_some_and(|text| unsafe_text(text, false)),
            Field::Resume => fields
                .resume_command
                .as_ref()
                .and_then(Option::as_deref)
                .is_some_and(|text| unsafe_text(text, false)),
            Field::Model => fields
                .model
                .as_ref()
                .and_then(Option::as_deref)
                .is_some_and(|text| unsafe_text(text, false)),
            Field::Wrapper => fields
                .wrapper_prefix
                .as_ref()
                .and_then(Option::as_ref)
                .is_some_and(|words| unsafe_text(&words.join(" "), false)),
            Field::Destination => match &fields.destination {
                Some(TemplateDestination::Folder(text) | TemplateDestination::Github(text)) => {
                    unsafe_text(text, false)
                }
                None => false,
            },
            Field::Agent
            | Field::Effort
            | Field::Permissions
            | Field::Container
            | Field::Yolo
            | Field::Host => false,
        }
    }
}

/// The Approvals choices for `agent`: the reset first, labelled with what it means for this agent ("Guarded · Claude
/// auto mode (reset)"), then every permission the agent offers, or all of them with no agent type, since another
/// template may supply the agent. An explicit choice equal to the agent's default stays its own choice: hiding it would
/// make otherwise valid stored templates unsaveable.
pub fn permission_options(agent: Option<LaunchHarness>) -> Vec<(Option<LaunchPermission>, String)> {
    if agent.is_some_and(LaunchHarness::is_shell) {
        return Vec::new();
    }
    let mut options = vec![(None, format!("{} (reset)", permission_label(agent, None)))];
    options.extend(
        LaunchPermission::ALL
            .iter()
            .copied()
            .filter(|permission| agent.is_none_or(|agent| agent.offers_permission(*permission)))
            .map(|permission| (Some(permission), permission_label(agent, Some(permission)))),
    );
    options
}

/// The Effort choices: "Default" (the reset) first, then the efforts the agent type and model offer in a catalog that
/// was read, or every effort when the agent type or the catalog is unknown (no catalog-based refusal is justified
/// before one is read).
pub fn effort_options(
    fields: &TemplateFields,
    catalog: Option<&[LaunchCatalogModel]>,
) -> Vec<(Option<LaunchEffort>, String)> {
    let efforts = match (fields.agent, catalog) {
        (Some(agent), Some(catalog)) => compatible_efforts(
            agent,
            fields.model.as_ref().and_then(|model| model.as_deref()),
            catalog,
        ),
        _ => farhelm_proto::launcher::EFFORT_ORDER.to_vec(),
    };
    let mut options = vec![(None, "Default".to_owned())];
    options.extend(
        efforts
            .into_iter()
            .map(|effort| (Some(effort), effort_label(Some(effort)).to_owned())),
    );
    options
}

/// One editor's template: its name and its fields as they will be saved, plus the wrapper prefix's raw text while it
/// does not parse. See the module docs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub name: String,
    pub fields: TemplateFields,
    /// The wrapper editor's text while it does not split into words (an open quote): kept visible and refusing saving,
    /// without replacing the last valid word list in `fields`.
    pub wrapper_input: Option<String>,
}

impl Default for Draft {
    /// A new template: no name, the Agent switch, no fields (the old editor's new draft).
    fn default() -> Draft {
        Draft {
            name: String::new(),
            fields: TemplateFields {
                kind: Some(LauncherKind::Agent),
                ..TemplateFields::default()
            },
            wrapper_input: None,
        }
    }
}

impl Draft {
    /// A stored template as the editor opens it. A template from before the switch existed gets the switch its fields
    /// imply, in the draft only (SPEC.md: "opening one infers its switch in an unsaved draft, and saving stores that
    /// switch"), and a workspace-trust field from an older template is dropped, since Settings owns trust now and every
    /// save drops it. Compared with the stored template, either makes the draft unsaved; opening never rewrites the
    /// stored template itself.
    pub fn from_template(template: &LaunchTemplate) -> Draft {
        let mut fields = template.fields.clone();
        fields.workspace_trust = None;
        fields.kind = fields.kind.or(fields.implied_kind());
        Draft {
            name: template.name.clone(),
            fields,
            wrapper_input: None,
        }
    }

    /// The draft as the template a save writes.
    pub fn template(&self) -> LaunchTemplate {
        LaunchTemplate {
            name: self.name.clone(),
            fields: self.fields.clone(),
        }
    }

    /// Why this draft cannot be saved, or `None`. The first reason in the old editor's order: a wrapper prefix that
    /// does not parse, custom words that do not fit their choice, a field that does not fit, then a present text left
    /// empty. Fields need not form a complete launch. The helm's shape check and the name rules come after this, at the
    /// save.
    pub fn refusal(&self, catalog: Option<&[LaunchCatalogModel]>) -> Option<String> {
        if let Some(text) = &self.wrapper_input {
            return Some(match farhelm_proto::split_wrapper_prefix(text) {
                Err(reason) => capitalize_validation(reason),
                // The text parses now but has not reached the fields yet; the editor writes it there on every edit,
                // so this is a race the user can resolve by looking again.
                Ok(_) => "Wrapper prefix changed while saving; review it and try again".to_owned(),
            });
        }
        if let Err(reason) = farhelm_proto::check_permission_flags(
            self.fields.permissions.flatten(),
            self.fields.permission_flags.as_deref(),
        ) {
            return Some(capitalize_validation(reason));
        }
        for field in Field::ALL
            .into_iter()
            .filter(|field| field.is_set(&self.fields))
        {
            if let Some(reason) = field.refusal(&self.fields, catalog) {
                return Some(format!("{}: {reason}", field.label()));
            }
        }
        let fields = &self.fields;
        let destination = fields
            .destination
            .as_ref()
            .map(|destination| match destination {
                TemplateDestination::Folder(text) | TemplateDestination::Github(text) => {
                    text.as_str()
                }
            });
        for (label, value) in [
            ("command", fields.command.as_deref()),
            (
                "model",
                fields.model.as_ref().and_then(|model| model.as_deref()),
            ),
            (
                "resume command",
                fields
                    .resume_command
                    .as_ref()
                    .and_then(|resume| resume.as_deref()),
            ),
            ("host", fields.host.as_deref()),
            ("session name", fields.name.as_deref()),
            ("destination", destination),
        ] {
            if value == Some("") {
                let verb = if label == "host" { "Choose" } else { "Type" };
                return Some(format!("{verb} the {label}, or remove its field"));
            }
        }
        None
    }
}

/// Why saving under `name` is refused because of the name alone, or `None`. `previous` is the stored template the
/// editor opened (`None` for a new template or a copy), `names` the templates the store holds, or `None` while the
/// helm reports it could not read them (or before its first snapshot arrived).
///
/// Saving under the template's own name replaces it, which is the point of editing (last write wins, SPEC.md), and is
/// never checked: a stored spaced name keeps saving under itself. Any other save, a new template or a rename, must use
/// a name without whitespace (SPEC.md: "Creating a template or renaming one refuses any whitespace in its name") that
/// no template has ([`taken_refusal`]).
pub fn name_refusal(
    previous: Option<&str>,
    name: &str,
    names: Option<&[&str]>,
) -> Option<HostSentence> {
    if previous == Some(name) {
        return None;
    }
    if let Err(reason) = farhelm_proto::launcher::check_new_template_name(name) {
        return Some(
            HostSentence::new("template-name-refusal").text(capitalize_validation(reason)),
        );
    }
    taken_refusal(name, names)
}

/// Why writing a template under `name` would replace another template, or `None` when the list shows the name is
/// free. A list the helm could not read proves nothing, so it refuses too (SPEC.md: "A taken template name is refused
/// without overwriting").
///
/// This is the whole name check for a delete's Undo, which restores the dialog's loaded copy under its old name: a
/// stored spaced name predates the whitespace rule and keeps its Undo (SPEC.md: existing names with whitespace "retain
/// delete undo"), so Undo must not run [`name_refusal`]'s new-name rule. The sentence names the conflicting template
/// as its own host value so the name cannot reorder the words around it. Undo supplies its own wording.
pub fn taken_refusal(name: &str, names: Option<&[&str]>) -> Option<HostSentence> {
    let sentence = HostSentence::new("template-name-refusal");
    match names {
        None => Some(sentence.text(
            "The template list could not be read, so Farhelm cannot check that no other template has this name; \
             save again once it is back",
        )),
        Some(names) => names.contains(&name).then(|| {
            sentence
                .text("A template named \"")
                .host(&HostText::from_host(name))
                .text("\" already exists; edit that one, or choose another name")
        }),
    }
}

/// The name Duplicate suggests for a copy of `name`: whitespace replaced by `-`, then `-copy`, `-copy-2` and so on
/// until a name no template in `names` has, cut at a character boundary to fit the helm's name limit (SPEC.md:
/// "Duplicate opens an unsaved copy named `<name>-copy`, then `<name>-copy-2` and so on until free, replacing
/// whitespace in the source name with `-`").
pub fn duplicate_name(name: &str, names: &[&str]) -> String {
    let name: String = name
        .chars()
        .map(|ch| if ch.is_whitespace() { '-' } else { ch })
        .collect();
    let mut number = 1usize;
    loop {
        let suffix = if number == 1 {
            "-copy".to_owned()
        } else {
            format!("-copy-{number}")
        };
        let mut end = name
            .len()
            .min(TEMPLATE_NAME_CAP.saturating_sub(suffix.len()));
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        let candidate = format!("{}{suffix}", &name[..end]);
        if !names.contains(&candidate.as_str()) {
            return candidate;
        }
        number += 1;
    }
}

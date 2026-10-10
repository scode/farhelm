//! The launcher's rules that both the browser and the helm apply: the model
//! catalog's row type, how picking an agent type reconciles the choices made
//! for another one, and launch templates (SPEC.md, Launch templates).
//!
//! These live in the wire crate rather than in either side because they must
//! agree exactly. The browser applies a template through the launcher's own
//! setters; the helm applies the same template for `farhelm agent create
//! --template` and for spawn's template resolution. A template that applied
//! one way in the GUI and another way from the CLI would make "applying a
//! template is exactly making its edits by hand" false on one of them.
//!
//! Nothing here decides whether a launch is valid as a whole; that is the
//! helm's compiler for an agent launch and [`crate::CommandLaunch::validate`]
//! for a command launch. A template is only ever checked for whether each of
//! its fields APPLIES to the launcher it is applied to.

use serde::{Deserialize, Serialize};

use crate::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection};

// ---------------------------------------------------------------------------
// The model catalog and agent-type reconciliation
// ---------------------------------------------------------------------------

/// One model the helm's release catalog offers, with the efforts it accepts.
///
/// What `GET /api/launch-catalog` answers with, one row per model. Custom
/// model ids are not rows: they are whatever a user types, owned by the
/// agent type they were entered for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchCatalogModel {
    pub id: String,
    pub harness: LaunchHarness,
    pub efforts: Vec<LaunchEffort>,
}

/// Every effort in the order the launcher lists them.
pub const EFFORT_ORDER: &[LaunchEffort] = &[
    LaunchEffort::Off,
    LaunchEffort::Minimal,
    LaunchEffort::Low,
    LaunchEffort::Medium,
    LaunchEffort::High,
    LaunchEffort::Xhigh,
    LaunchEffort::Max,
    LaunchEffort::Ultra,
];

/// The efforts an agent type offers for `model`: that model's own list when
/// the catalog knows it, otherwise every effort some catalog model of that
/// agent type offers.
pub fn compatible_efforts(
    harness: LaunchHarness,
    model: Option<&str>,
    catalog: &[LaunchCatalogModel],
) -> Vec<LaunchEffort> {
    if let Some(model) = model.map(|model| harness.catalog_model_id(model))
        && let Some(entry) = catalog
            .iter()
            .find(|entry| entry.harness == harness && entry.id == model)
    {
        return entry.efforts.clone();
    }

    EFFORT_ORDER
        .iter()
        .copied()
        .filter(|effort| {
            catalog
                .iter()
                .any(|entry| entry.harness == harness && entry.efforts.contains(effort))
        })
        .collect()
}

/// Move an agent launch's choices to another agent type without keeping
/// values the new type cannot use (SPEC.md: "Replacing a harness clears
/// incompatible choices and a YOLO permission that was the previous
/// harness's default").
///
/// A model is owned either by the release catalog or, for a custom id, by the
/// agent type on which it was entered; the caller passes that ownership so a
/// click, a search result and a template make exactly the same
/// reconciliation. Effort is independent when the new type still offers it;
/// only an effort the new model or type cannot accept is cleared.
/// Unsupported permissions fall back to the destination's default, and a
/// source's default YOLO never becomes an explicit preference for another
/// type. Callers must keep the source agent type in `selection`.
///
/// Returns the reconciled selection and who owns its model now.
pub fn reconcile_harness_selection(
    mut selection: LaunchSelection,
    model_owner: Option<LaunchHarness>,
    harness: LaunchHarness,
    catalog: &[LaunchCatalogModel],
) -> (LaunchSelection, Option<LaunchHarness>) {
    let source = selection.harness;
    // A harness's default YOLO does not express an approval preference for a
    // different harness. Keep explicit approval choices, and explicit YOLO
    // from harnesses whose omitted mode is non-YOLO.
    if selection.harness != harness
        && selection.permissions == selection.harness.omitted_permission()
    {
        selection.permissions = None;
    }
    selection.harness = harness;
    if !harness.offers_model() {
        // Grok exposes no model, effort or workspace-trust choice. Clear retained values at the harness
        // boundary so a recent setup or prior selection cannot manufacture a
        // launch shape that the helm must reject later.
        selection.model = None;
        selection.effort = None;
        selection.workspace_trust = None;
        selection.permissions = harness.effective_permission(selection.permissions);
        return (selection, None);
    }
    // Ownership is read in both harnesses' catalog spellings
    // (`LaunchHarness::catalog_model_id`). The destination's spelling keeps
    // a bare `gpt-6-luna` moved to OpenCode as OpenCode's model; the
    // source's spelling still recognizes a model the source harness owns,
    // so Claude's `claude-fable-5` moved to OpenCode (where it reads as an
    // unknown `opencode/claude-fable-5`) is cleared rather than kept as a
    // custom OpenCode id.
    let known_owners = selection.model.as_ref().map(|model| {
        let in_destination = harness.catalog_model_id(model);
        let in_source = source.catalog_model_id(model);
        catalog
            .iter()
            .filter(|candidate| {
                candidate.id == in_destination
                    || (candidate.harness == source && candidate.id == in_source)
            })
            .map(|candidate| candidate.harness)
            .collect::<Vec<_>>()
    });
    let known_is_owned = known_owners
        .as_ref()
        .is_some_and(|owners| owners.contains(&harness));
    let owner = known_is_owned.then_some(harness).or(model_owner);
    let retained_owner = known_is_owned
        .then_some(harness)
        .or((model_owner == Some(harness)).then_some(harness));
    if (known_owners
        .as_ref()
        .is_some_and(|owners| !owners.is_empty())
        && !known_is_owned)
        || (known_owners.as_ref().is_none_or(|owners| owners.is_empty())
            && owner.is_some()
            && retained_owner.is_none())
    {
        selection.model = None;
    }
    if selection.effort.is_some_and(|effort| {
        !compatible_efforts(harness, selection.model.as_deref(), catalog).contains(&effort)
    }) {
        selection.effort = None;
    }
    selection.permissions = harness.effective_permission(selection.permissions);
    if !harness.offers_workspace_trust() {
        selection.workspace_trust = None;
    }
    (selection, retained_owner)
}

// ---------------------------------------------------------------------------
// Launch templates
// ---------------------------------------------------------------------------

/// The launcher's two launch kinds, as a template names them.
///
/// Separate from [`crate::LaunchKind`], which also names the legacy kind a
/// launcher can never produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LauncherKind {
    Agent,
    Command,
}

/// Where a launch runs: an existing folder, or a fresh checkout of a GitHub
/// repository (`owner/name`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TemplateDestination {
    Folder(String),
    Github(String),
}

/// Deserialize a field where absent and `null` mean different things:
/// absent leaves the launcher's value alone, `null` resets it to the agent
/// type's default. Used with `#[serde(default)]`, which supplies the
/// absent case.
fn present<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// What a template sets: every launcher field, each optional (SPEC.md: "A
/// template may set any launcher field").
///
/// A field left out keeps whatever the launcher already holds, which is what
/// lets templates stack. The agent-launch choices that have a default
/// (model, effort, permissions, workspace trust) and the resume command
/// distinguish absent from `null`: `null` explicitly resets the choice, the
/// way choosing "harness default" by hand does. The host is named by its
/// recorded install identity, never by registry id, so a row retargeted to
/// another install makes the field inapplicable rather than aiming it at the
/// successor. Unknown fields are refused, so a misspelled field is an error
/// rather than a silently ignored edit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<LauncherKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<LaunchHarness>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub model: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub effort: Option<Option<LaunchEffort>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub permissions: Option<Option<LaunchPermission>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub workspace_trust: Option<Option<bool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yolo: Option<bool>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub resume_command: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<TemplateDestination>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl TemplateFields {
    /// Whether the template sets a command-launch choice, including a false
    /// YOLO assertion or an explicit null reset of the resume command.
    ///
    /// The agent type is shared by both launch kinds: beside these fields it
    /// declares the command's agent, rather than requiring an agent launch.
    pub fn sets_command_fields(&self) -> bool {
        self.command.is_some() || self.yolo.is_some() || self.resume_command.is_some()
    }

    /// Whether the template sets a choice available only to agent launches.
    /// Explicit null resets still count as choices.
    ///
    /// This deliberately excludes the agent type, which a command launch may
    /// declare too. Including it would reject valid agent-plus-command templates
    /// when the helm checks for mixed launch kinds.
    pub fn sets_agent_only_fields(&self) -> bool {
        self.model.is_some()
            || self.effort.is_some()
            || self.permissions.is_some()
            || self.workspace_trust.is_some()
    }

    /// Infer the launcher tab from choices alone, without consulting `kind`.
    ///
    /// Command choices take precedence because an accompanying agent type
    /// declares the command's agent. Otherwise an agent type or agent-only
    /// choice implies an agent launch; placement alone implies neither. Callers
    /// keep explicit kinds and decide whether mixed choices should be refused.
    pub fn implied_kind(&self) -> Option<LauncherKind> {
        if self.sets_command_fields() {
            Some(LauncherKind::Command)
        } else if self.agent.is_some() || self.sets_agent_only_fields() {
            Some(LauncherKind::Agent)
        } else {
            None
        }
    }
}

/// The longest template name the helm stores, in bytes.
pub const TEMPLATE_NAME_CAP: usize = 128;

/// The most a template's fields may hold, as JSON, matching the 64 KiB a
/// create's fields are held to (SPEC.md, Creation): a template is applied
/// into exactly those fields.
pub const TEMPLATE_FIELDS_CAP: usize = 64 * 1024;

/// A named, partial set of launcher edits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchTemplate {
    pub name: String,
    pub fields: TemplateFields,
}

/// Refuse templates whose name, commands or size cannot be safely edited.
/// Commands cannot contain controls or invisible formatting characters: the
/// single-line editor cannot show or preserve them. Every save path shares
/// this check; applying an already stored template does not run it, so legacy
/// commands remain usable until the user edits the template (SPEC.md).
/// Names must also survive use as a URL segment: `.` and `..` navigate instead
/// of naming a template, even when percent-encoded.
/// Whether fields apply is decided against the launcher when applied.
pub fn check_template_shape(template: &LaunchTemplate) -> Result<(), String> {
    let name = &template.name;
    if name.is_empty() {
        return Err("a template needs a name".to_string());
    }
    // Template names travel as the last URL segment. Both spellings are
    // navigation even when percent-encoded, so neither can identify a template.
    if name == "." || name == ".." {
        return Err("a template name cannot be . or ..".to_string());
    }
    if name.len() > TEMPLATE_NAME_CAP {
        return Err(format!(
            "a template name is at most {TEMPLATE_NAME_CAP} bytes"
        ));
    }
    if name.trim() != name {
        return Err("a template name cannot start or end with spaces".to_string());
    }
    if name.chars().any(char::is_control) {
        return Err("a template name cannot contain control characters".to_string());
    }
    for (field, command) in [
        ("launch command", template.fields.command.as_deref()),
        (
            "resume command",
            template
                .fields
                .resume_command
                .as_ref()
                .and_then(|value| value.as_deref()),
        ),
    ] {
        if command.is_some_and(|command| command.chars().any(crate::text::is_presentation_unsafe)) {
            return Err(format!(
                "a template's {field} cannot contain control or invisible formatting characters"
            ));
        }
    }
    let bytes = serde_json::to_string(&template.fields).map_or(usize::MAX, |json| json.len());
    if bytes > TEMPLATE_FIELDS_CAP {
        return Err(format!(
            "a template's fields are {bytes} bytes, exceeding the {TEMPLATE_FIELDS_CAP}-byte limit"
        ));
    }
    Ok(())
}

/// Everything the launcher holds, as templates read and edit it.
///
/// The agent type is the one field the two launch kinds share (SPEC.md,
/// Concepts): `harness` is the agent launch's, `command_agent` the command
/// launch's declared type, and a template's `agent` field sets whichever the
/// active kind uses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LauncherState {
    pub kind: Option<LauncherKind>,
    pub harness: Option<LaunchHarness>,
    pub model: Option<String>,
    /// Which agent type `model` was entered for when it is a custom id (the
    /// launcher's own record of the same fact), so a later change of agent
    /// type clears it exactly as it does by hand
    /// ([`reconcile_harness_selection`]).
    pub model_owner: Option<LaunchHarness>,
    pub effort: Option<LaunchEffort>,
    pub permissions: Option<LaunchPermission>,
    pub workspace_trust: Option<bool>,
    pub command: String,
    pub yolo: Option<bool>,
    pub command_agent: Option<LaunchHarness>,
    pub resume_command: Option<String>,
    pub host: Option<String>,
    pub destination: Option<TemplateDestination>,
    pub name: Option<String>,
}

/// What a template is applied against besides the launcher itself.
#[derive(Debug, Clone, Copy)]
pub struct TemplateContext<'a> {
    /// The release catalog, for which models and efforts an agent type
    /// offers. An empty catalog (not read yet) checks only what the agent
    /// type itself declares.
    pub catalog: &'a [LaunchCatalogModel],
    /// The install identities of the hosts the launcher can target.
    pub known_hosts: &'a [String],
    /// The dialog holds the host fixed (Replace with), so a template that
    /// sets one is refused.
    pub host_fixed: bool,
}

/// Why a template was not applied, naming the field (SPEC.md: "refused with
/// a message naming the field, and nothing from it is applied").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRefusal {
    pub template: String,
    pub field: &'static str,
    pub reason: String,
}

impl std::fmt::Display for TemplateRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "template {:?} was not applied: its {} {}",
            self.template, self.field, self.reason
        )
    }
}

/// An agent type's protocol word (`claude`, `open_code`), as refusals name
/// it: the spelling a template and the CLI's `--agent` use.
fn harness_word(harness: LaunchHarness) -> String {
    match serde_json::to_value(harness) {
        Ok(serde_json::Value::String(word)) => word,
        _ => format!("{harness:?}"),
    }
}

/// Apply one template to the launcher, all or nothing, in SPEC.md's fixed
/// order: launch kind first, then agent type, then every other field.
///
/// Choosing an agent type on the agent launch kind reconciles the choices
/// made for another type exactly as picking it by hand does
/// ([`reconcile_harness_selection`]). Agent-launch fields and
/// command-launch fields apply to the launch kind active once the template's
/// own kind, if any, is set; one that does not apply is refused, as is a
/// model, effort, permission or trust choice the agent type does not offer,
/// a host the dialog holds fixed, and a host no known install has. The
/// result is a launcher state, not a launch: a field the launch needs and no
/// template set is still the user's (or the CLI flags') to fill.
pub fn apply_template(
    state: &LauncherState,
    template: &LaunchTemplate,
    context: &TemplateContext<'_>,
) -> Result<LauncherState, TemplateRefusal> {
    let fields = &template.fields;
    let refuse = |field: &'static str, reason: String| TemplateRefusal {
        template: template.name.clone(),
        field,
        reason,
    };
    let mut next = state.clone();

    // 1. The launch kind.
    if let Some(kind) = fields.kind {
        next.kind = Some(kind);
    }
    let kind = next.kind.unwrap_or(LauncherKind::Agent);

    // 2. The agent type, which each kind keeps in its own field.
    if let Some(agent) = fields.agent {
        match kind {
            LauncherKind::Agent => {
                let selection = LaunchSelection {
                    harness: next.harness.unwrap_or(agent),
                    model: next.model.clone(),
                    effort: next.effort,
                    permissions: next.permissions,
                    workspace_trust: next.workspace_trust,
                };
                let (selection, owner) = reconcile_harness_selection(
                    selection,
                    next.model_owner,
                    agent,
                    context.catalog,
                );
                next.model_owner = owner;
                next.harness = Some(selection.harness);
                next.model = selection.model;
                next.effort = selection.effort;
                next.permissions = selection.permissions;
                next.workspace_trust = selection.workspace_trust;
            }
            LauncherKind::Command => next.command_agent = Some(agent),
        }
    }

    // 3. The agent launch's choices.
    let agent_fields: [(&'static str, bool); 4] = [
        ("model", fields.model.is_some()),
        ("effort", fields.effort.is_some()),
        ("permissions", fields.permissions.is_some()),
        ("workspace trust", fields.workspace_trust.is_some()),
    ];
    if let Some((field, _)) = agent_fields.iter().find(|(_, set)| *set) {
        if kind == LauncherKind::Command {
            return Err(refuse(
                field,
                "applies to an agent launch, and the launcher is on the command launch kind"
                    .to_string(),
            ));
        }
        // With no agent type chosen, match the catalog spelling exactly, as
        // typing the model by hand does (SPEC.md: "a bare `gpt-6-luna` picks
        // Codex"). OpenCode's bare-name shorthand applies only after choosing
        // OpenCode; counting it here would make that Codex model ambiguous.
        if next.harness.is_none()
            && let Some(Some(model)) = &fields.model
        {
            let owners: Vec<LaunchHarness> = context
                .catalog
                .iter()
                .filter(|entry| entry.id == model.as_str())
                .map(|entry| entry.harness)
                .collect();
            if let [owner] = owners.as_slice() {
                next.harness = Some(*owner);
            }
        }
        let Some(harness) = next.harness else {
            return Err(refuse(
                field,
                "needs an agent type, and none is chosen".to_string(),
            ));
        };
        let label = harness_word(harness);
        if let Some(model) = &fields.model {
            match model {
                Some(model) => {
                    if !harness.offers_model() {
                        return Err(refuse("model", format!("is not offered by {label}")));
                    }
                    // Compared in this agent type's own spelling only, as the
                    // launcher and the helm compare it: a bare OpenCode name
                    // another type also lists is OpenCode's
                    // (`LaunchHarness::catalog_model_id`).
                    let spelled = harness.catalog_model_id(model);
                    let owners: Vec<LaunchHarness> = context
                        .catalog
                        .iter()
                        .filter(|entry| entry.id == spelled)
                        .map(|entry| entry.harness)
                        .collect();
                    if !owners.is_empty() && !owners.contains(&harness) {
                        return Err(refuse(
                            "model",
                            format!("{model:?} is not offered by {label}"),
                        ));
                    }
                    // The same reconciliation choosing the model by hand
                    // runs: an effort the new model does not offer goes.
                    let (selection, owner) = reconcile_harness_selection(
                        LaunchSelection {
                            harness,
                            model: Some(model.clone()),
                            effort: next.effort,
                            permissions: next.permissions,
                            workspace_trust: next.workspace_trust,
                        },
                        Some(harness),
                        harness,
                        context.catalog,
                    );
                    next.model = selection.model;
                    next.model_owner = owner;
                    next.effort = selection.effort;
                    next.permissions = selection.permissions;
                    next.workspace_trust = selection.workspace_trust;
                }
                None => {
                    next.model = None;
                    next.model_owner = None;
                }
            }
        }
        if let Some(effort) = fields.effort {
            if let Some(effort) = effort
                && (!harness.offers_effort()
                    || (!context.catalog.is_empty()
                        && !compatible_efforts(harness, next.model.as_deref(), context.catalog)
                            .contains(&effort)))
            {
                return Err(refuse(
                    "effort",
                    format!(
                        "{} is not offered by {label} for this model",
                        effort.as_cli_arg()
                    ),
                ));
            }
            next.effort = effort;
        }
        if let Some(permissions) = fields.permissions {
            if let Some(permission) = permissions
                && !harness.offers_permission(permission)
            {
                return Err(refuse(
                    "permissions",
                    format!("{} is not offered by {label}", permission.wire_word()),
                ));
            }
            next.permissions = permissions;
        }
        if let Some(trust) = fields.workspace_trust {
            if trust.is_some() && !harness.offers_workspace_trust() {
                return Err(refuse(
                    "workspace trust",
                    format!("is not offered by {label}"),
                ));
            }
            next.workspace_trust = trust;
        }
    }

    // 4. The command launch's fields.
    let command_fields: [(&'static str, bool); 3] = [
        ("command", fields.command.is_some()),
        ("YOLO answer", fields.yolo.is_some()),
        ("resume command", fields.resume_command.is_some()),
    ];
    if let Some((field, _)) = command_fields.iter().find(|(_, set)| *set) {
        if kind == LauncherKind::Agent {
            return Err(refuse(
                field,
                "applies to a command launch, and the launcher is on the agent launch kind"
                    .to_string(),
            ));
        }
        if let Some(command) = &fields.command {
            next.command = command.clone();
        }
        if let Some(yolo) = fields.yolo {
            next.yolo = Some(yolo);
        }
        if let Some(resume) = &fields.resume_command {
            next.resume_command = resume.clone();
        }
    }

    // 5. The destination and the session's name.
    if let Some(host) = &fields.host {
        if context.host_fixed {
            return Err(refuse(
                "host",
                "cannot change here: this dialog keeps the session's host".to_string(),
            ));
        }
        if !context.known_hosts.contains(host) {
            return Err(refuse(
                "host",
                "names an install no host here has (it may have been replaced or removed)"
                    .to_string(),
            ));
        }
        next.host = Some(host.clone());
    }
    if let Some(destination) = &fields.destination {
        next.destination = Some(destination.clone());
    }
    if let Some(name) = &fields.name {
        next.name = Some(name.clone());
    }
    Ok(next)
}

/// Apply templates one after another, each to the result of the last, so
/// later ones win where they overlap (SPEC.md: templates stack). The first
/// refusal stops the whole sequence with nothing applied.
pub fn apply_templates<'t>(
    state: &LauncherState,
    templates: impl IntoIterator<Item = &'t LaunchTemplate>,
    context: &TemplateContext<'_>,
) -> Result<LauncherState, TemplateRefusal> {
    let mut next = state.clone();
    for template in templates {
        next = apply_template(&next, template, context)?;
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every launch-specific field determines a legacy template's tab, while
    /// placement fields do not. False assertions and null resets are choices,
    /// and a declared command agent must never turn a command into an agent
    /// launch. Pinning the wire inputs keeps the helm and editor's shared rule
    /// from losing fields or conflating absence with a reset.
    #[test]
    fn template_fields_imply_kind_by_field() {
        use serde_json::json;

        for (fields, expected) in [
            (json!({}), None),
            (
                json!({"command": "echo hello"}),
                Some(LauncherKind::Command),
            ),
            (json!({"yolo": true}), Some(LauncherKind::Command)),
            (json!({"yolo": false}), Some(LauncherKind::Command)),
            (
                json!({"resume_command": "resume"}),
                Some(LauncherKind::Command),
            ),
            (json!({"resume_command": null}), Some(LauncherKind::Command)),
            (json!({"agent": "claude"}), Some(LauncherKind::Agent)),
            (json!({"model": "custom"}), Some(LauncherKind::Agent)),
            (json!({"model": null}), Some(LauncherKind::Agent)),
            (json!({"effort": "high"}), Some(LauncherKind::Agent)),
            (json!({"effort": null}), Some(LauncherKind::Agent)),
            (
                json!({"permissions": "smart_approve"}),
                Some(LauncherKind::Agent),
            ),
            (json!({"permissions": null}), Some(LauncherKind::Agent)),
            (json!({"workspace_trust": true}), Some(LauncherKind::Agent)),
            (json!({"workspace_trust": false}), Some(LauncherKind::Agent)),
            (json!({"workspace_trust": null}), Some(LauncherKind::Agent)),
            (json!({"host": "installation"}), None),
            (json!({"destination": {"folder": "project"}}), None),
            (json!({"destination": {"github": "owner/repo"}}), None),
            (json!({"name": "review"}), None),
            (json!({"kind": "command"}), None),
            (
                json!({"kind": "command", "agent": "claude"}),
                Some(LauncherKind::Agent),
            ),
            (
                json!({"kind": "agent", "yolo": false}),
                Some(LauncherKind::Command),
            ),
        ] {
            let parsed: TemplateFields = serde_json::from_value(fields.clone()).unwrap();
            assert_eq!(parsed.implied_kind(), expected, "fields: {fields}");
        }

        for fields in [
            json!({"agent": "claude", "command": "claude {farhelm_args}"}),
            json!({"agent": "claude", "yolo": false}),
            json!({"agent": "claude", "resume_command": "resume"}),
            json!({"agent": "claude", "resume_command": null}),
        ] {
            let parsed: TemplateFields = serde_json::from_value(fields.clone()).unwrap();
            assert_eq!(
                parsed.implied_kind(),
                Some(LauncherKind::Command),
                "fields: {fields}"
            );
            assert!(
                !parsed.sets_agent_only_fields(),
                "a declared command agent is not agent-only: {fields}"
            );
        }
    }

    fn catalog() -> Vec<LaunchCatalogModel> {
        vec![
            LaunchCatalogModel {
                id: "claude-fable-5".into(),
                harness: LaunchHarness::Claude,
                efforts: vec![LaunchEffort::High],
            },
            LaunchCatalogModel {
                id: "gpt-6-luna".into(),
                harness: LaunchHarness::Codex,
                efforts: vec![LaunchEffort::Low, LaunchEffort::High],
            },
        ]
    }

    fn template(name: &str, fields: TemplateFields) -> LaunchTemplate {
        LaunchTemplate {
            name: name.into(),
            fields,
        }
    }

    fn context<'a>(
        catalog: &'a [LaunchCatalogModel],
        hosts: &'a [String],
        host_fixed: bool,
    ) -> TemplateContext<'a> {
        TemplateContext {
            catalog,
            known_hosts: hosts,
            host_fixed,
        }
    }

    /// Spec: a template applies in SPEC.md's fixed order (launch kind, then
    /// agent type, then the rest), and choosing an agent type on the agent
    /// kind clears the choices the new type cannot use, exactly as picking
    /// it by hand does; on the command kind the agent type is the declared
    /// one.
    ///
    /// Why: the order is what makes a template that sets both a kind and an
    /// agent type mean the same thing however its fields are listed, and
    /// "exactly as by hand" is SPEC.md's whole definition of applying one.
    #[test]
    fn a_template_applies_kind_then_agent_type_then_the_rest() {
        let catalog = catalog();
        let state = LauncherState {
            harness: Some(LaunchHarness::Claude),
            model: Some("claude-fable-5".into()),
            effort: Some(LaunchEffort::High),
            ..LauncherState::default()
        };
        let codex = template(
            "codex",
            TemplateFields {
                agent: Some(LaunchHarness::Codex),
                effort: Some(Some(LaunchEffort::Low)),
                ..TemplateFields::default()
            },
        );
        let applied = apply_template(&state, &codex, &context(&catalog, &[], false)).unwrap();
        assert_eq!(applied.harness, Some(LaunchHarness::Codex));
        assert_eq!(applied.model, None, "Claude's model does not move to Codex");
        assert_eq!(applied.effort, Some(LaunchEffort::Low));

        let command = template(
            "as-command",
            TemplateFields {
                kind: Some(LauncherKind::Command),
                agent: Some(LaunchHarness::Codex),
                command: Some("codex {farhelm_args}".into()),
                ..TemplateFields::default()
            },
        );
        let applied = apply_template(&state, &command, &context(&catalog, &[], false)).unwrap();
        assert_eq!(applied.kind, Some(LauncherKind::Command));
        assert_eq!(applied.command_agent, Some(LaunchHarness::Codex));
        assert_eq!(
            applied.harness,
            Some(LaunchHarness::Claude),
            "the agent tab's draft is untouched"
        );
        assert_eq!(applied.command, "codex {farhelm_args}");
    }

    /// Spec: templates stack, the later winning where they overlap, and a
    /// field a template leaves out keeps the launcher's value, while `null`
    /// resets a choice to the agent type's default.
    ///
    /// Why: stacking (`my-codex` then `myproject-webbuilder`) is the use
    /// SPEC.md names, and absent versus `null` is the only way a template
    /// can either leave a model alone or reset it.
    #[test]
    fn templates_stack_and_null_resets_a_choice() {
        let catalog = catalog();
        let base = template(
            "my-codex",
            TemplateFields {
                agent: Some(LaunchHarness::Codex),
                model: Some(Some("gpt-6-luna".into())),
                effort: Some(Some(LaunchEffort::High)),
                ..TemplateFields::default()
            },
        );
        let project = template(
            "myproject",
            TemplateFields {
                effort: Some(Some(LaunchEffort::Low)),
                name: Some("webbuilder".into()),
                ..TemplateFields::default()
            },
        );
        let stacked = apply_templates(
            &LauncherState::default(),
            [&base, &project],
            &context(&catalog, &[], false),
        )
        .unwrap();
        assert_eq!(stacked.model.as_deref(), Some("gpt-6-luna"));
        assert_eq!(stacked.effort, Some(LaunchEffort::Low));
        assert_eq!(stacked.name.as_deref(), Some("webbuilder"));

        let reset = template(
            "reset-model",
            TemplateFields {
                model: Some(None),
                ..TemplateFields::default()
            },
        );
        let applied = apply_template(&stacked, &reset, &context(&catalog, &[], false)).unwrap();
        assert_eq!(applied.model, None);
        assert_eq!(applied.effort, Some(LaunchEffort::Low), "left out, kept");
    }

    /// Stacking a Grok choice after a trusted harness must clear unsupported
    /// trust, model and effort choices. The shared launcher also serves CLI
    /// templates, whose retained trust used to make the resulting launch fail.
    #[test]
    fn a_grok_template_clears_retained_workspace_trust() {
        let catalog = catalog();
        for trust in [false, true] {
            let trusted = template(
                "trusted",
                TemplateFields {
                    agent: Some(LaunchHarness::Codex),
                    model: Some(Some("gpt-6-luna".into())),
                    effort: Some(Some(LaunchEffort::High)),
                    workspace_trust: Some(Some(trust)),
                    ..TemplateFields::default()
                },
            );
            let grok = template(
                "grok",
                TemplateFields {
                    agent: Some(LaunchHarness::Grok),
                    ..TemplateFields::default()
                },
            );
            let seeded = apply_template(
                &LauncherState::default(),
                &trusted,
                &context(&catalog, &[], false),
            )
            .unwrap();
            assert_eq!(seeded.workspace_trust, Some(trust));
            let applied = apply_template(&seeded, &grok, &context(&catalog, &[], false)).unwrap();
            assert_eq!(applied.harness, Some(LaunchHarness::Grok));
            assert_eq!(applied.workspace_trust, None);
            assert_eq!(applied.model, None);
            assert_eq!(applied.effort, None);
        }
    }

    /// Spec: a field that does not apply refuses the whole template, naming
    /// the field: a command field on the agent kind, an agent choice on the
    /// command kind or with no agent type, a model another agent type owns,
    /// an effort or permission the type does not offer, a host the dialog
    /// holds fixed, and a host no known install has.
    ///
    /// Why: SPEC.md makes application all or nothing, so the caller keeps
    /// its launcher exactly as it was on any refusal; and naming the field is
    /// how the user knows which part of the template to fix.
    #[test]
    fn a_field_that_does_not_apply_refuses_the_template_naming_it() {
        let catalog = catalog();
        let hosts = vec!["install-a".to_string()];
        let codex = LauncherState {
            kind: Some(LauncherKind::Agent),
            harness: Some(LaunchHarness::Codex),
            ..LauncherState::default()
        };
        let cases: [(LauncherState, TemplateFields, bool, &str); 8] = [
            (
                codex.clone(),
                TemplateFields {
                    command: Some("sh".into()),
                    ..TemplateFields::default()
                },
                false,
                "command",
            ),
            (
                LauncherState {
                    kind: Some(LauncherKind::Command),
                    ..LauncherState::default()
                },
                TemplateFields {
                    model: Some(Some("gpt-6-luna".into())),
                    ..TemplateFields::default()
                },
                false,
                "model",
            ),
            (
                LauncherState::default(),
                TemplateFields {
                    effort: Some(Some(LaunchEffort::High)),
                    ..TemplateFields::default()
                },
                false,
                "effort",
            ),
            (
                codex.clone(),
                TemplateFields {
                    model: Some(Some("claude-fable-5".into())),
                    ..TemplateFields::default()
                },
                false,
                "model",
            ),
            (
                codex.clone(),
                TemplateFields {
                    effort: Some(Some(LaunchEffort::Ultra)),
                    ..TemplateFields::default()
                },
                false,
                "effort",
            ),
            (
                codex.clone(),
                TemplateFields {
                    permissions: Some(Some(LaunchPermission::SmartApprove)),
                    ..TemplateFields::default()
                },
                false,
                "permissions",
            ),
            (
                codex.clone(),
                TemplateFields {
                    host: Some("install-a".into()),
                    ..TemplateFields::default()
                },
                true,
                "host",
            ),
            (
                codex.clone(),
                TemplateFields {
                    host: Some("install-b".into()),
                    ..TemplateFields::default()
                },
                false,
                "host",
            ),
        ];
        for (state, fields, host_fixed, field) in cases {
            let refusal = apply_template(
                &state,
                &template("t", fields.clone()),
                &context(&catalog, &hosts, host_fixed),
            )
            .expect_err(&format!("{fields:?} must be refused"));
            assert_eq!(refusal.field, field, "{fields:?}");
            assert!(refusal.to_string().contains("\"t\""), "{refusal}");
        }
        let applied = apply_template(
            &codex,
            &template(
                "t",
                TemplateFields {
                    host: Some("install-a".into()),
                    ..TemplateFields::default()
                },
            ),
            &context(&catalog, &hosts, false),
        )
        .unwrap();
        assert_eq!(applied.host.as_deref(), Some("install-a"));
    }

    /// Spec: setting a model reconciles like choosing it by hand: an effort
    /// the new model does not offer is cleared; a custom model stays owned
    /// by the agent type it was set on, so a later template changing the
    /// agent type clears it; a bare name OpenCode reads in its own spelling
    /// is OpenCode's even when another type lists that name; and with no
    /// agent type chosen, the exact catalog spelling picks its owner even
    /// when another type accepts that spelling as a shorthand.
    ///
    /// Why: the helm applies templates with this function for the agent CLI,
    /// and SPEC.md requires a template to have exactly the effects of the
    /// same edits by hand, so every reconciliation the launcher runs on those
    /// edits must run here too, or the CLI would build a launch the GUI never
    /// could.
    #[test]
    fn setting_a_model_reconciles_as_choosing_it_by_hand() {
        let mut catalog = catalog();
        catalog.push(LaunchCatalogModel {
            id: "gpt-6-astra".into(),
            harness: LaunchHarness::Codex,
            efforts: vec![LaunchEffort::High],
        });
        // Keep the overlapping name: inferring an agent type must use the
        // catalog spelling, while an already chosen OpenCode accepts the bare
        // name too. A Codex-only fixture cannot distinguish those contracts.
        catalog.push(LaunchCatalogModel {
            id: "opencode/gpt-6-luna".into(),
            harness: LaunchHarness::OpenCode,
            efforts: vec![],
        });
        let ctx = context(&catalog, &[], false);
        let codex_low = LauncherState {
            harness: Some(LaunchHarness::Codex),
            effort: Some(LaunchEffort::Low),
            ..LauncherState::default()
        };
        let astra = template(
            "astra",
            TemplateFields {
                model: Some(Some("gpt-6-astra".into())),
                ..TemplateFields::default()
            },
        );
        let applied = apply_template(&codex_low, &astra, &ctx).unwrap();
        assert_eq!(applied.model.as_deref(), Some("gpt-6-astra"));
        assert_eq!(applied.effort, None, "astra offers no low effort");

        let custom = template(
            "custom",
            TemplateFields {
                agent: Some(LaunchHarness::Claude),
                model: Some(Some("my-custom".into())),
                ..TemplateFields::default()
            },
        );
        let to_codex = template(
            "to-codex",
            TemplateFields {
                agent: Some(LaunchHarness::Codex),
                ..TemplateFields::default()
            },
        );
        let applied =
            apply_templates(&LauncherState::default(), [&custom, &to_codex], &ctx).unwrap();
        assert_eq!(applied.harness, Some(LaunchHarness::Codex));
        assert_eq!(
            applied.model, None,
            "a custom Claude model does not follow to Codex"
        );

        let opencode = template(
            "zen",
            TemplateFields {
                agent: Some(LaunchHarness::OpenCode),
                model: Some(Some("claude-fable-5".into())),
                ..TemplateFields::default()
            },
        );
        let applied = apply_template(&LauncherState::default(), &opencode, &ctx).unwrap();
        assert_eq!(applied.harness, Some(LaunchHarness::OpenCode));
        assert_eq!(applied.model.as_deref(), Some("claude-fable-5"));

        let bare = template(
            "bare",
            TemplateFields {
                model: Some(Some("gpt-6-luna".into())),
                ..TemplateFields::default()
            },
        );
        let applied = apply_template(&LauncherState::default(), &bare, &ctx).unwrap();
        assert_eq!(
            applied.harness,
            Some(LaunchHarness::Codex),
            "the exact catalog spelling belongs to Codex"
        );
        assert_eq!(applied.model.as_deref(), Some("gpt-6-luna"));

        let qualified = template(
            "qualified",
            TemplateFields {
                model: Some(Some("opencode/gpt-6-luna".into())),
                ..TemplateFields::default()
            },
        );
        let applied = apply_template(&LauncherState::default(), &qualified, &ctx).unwrap();
        assert_eq!(applied.harness, Some(LaunchHarness::OpenCode));
        assert_eq!(applied.model.as_deref(), Some("opencode/gpt-6-luna"));

        let chosen_opencode = LauncherState {
            harness: Some(LaunchHarness::OpenCode),
            ..LauncherState::default()
        };
        let applied = apply_template(&chosen_opencode, &bare, &ctx).unwrap();
        assert_eq!(applied.harness, Some(LaunchHarness::OpenCode));
        assert_eq!(applied.model.as_deref(), Some("gpt-6-luna"));
    }

    /// Spec: a template's stored JSON distinguishes a field left out from a
    /// field set to `null`, and an unknown field is refused.
    ///
    /// Why: the helm stores templates as this JSON, so the distinction must
    /// survive storage; and a misspelled field silently ignored would apply a
    /// template that does less than its author wrote.
    #[test]
    fn template_json_keeps_absent_and_null_apart_and_refuses_unknown_fields() {
        let left_out: TemplateFields = serde_json::from_str("{}").unwrap();
        assert_eq!(left_out.model, None);
        let reset: TemplateFields = serde_json::from_str(r#"{"model":null}"#).unwrap();
        assert_eq!(reset.model, Some(None));
        assert_eq!(serde_json::to_string(&reset).unwrap(), r#"{"model":null}"#);
        let set: TemplateFields = serde_json::from_str(
            r#"{"kind":"command","command":"sh","destination":{"github":"o/r"}}"#,
        )
        .unwrap();
        assert_eq!(set.kind, Some(LauncherKind::Command));
        assert_eq!(
            set.destination,
            Some(TemplateDestination::Github("o/r".into()))
        );
        assert!(serde_json::from_str::<TemplateFields>(r#"{"modle":"x"}"#).is_err());
    }

    /// Spec: the helm stores a template only with a usable name (non-empty,
    /// bounded, no surrounding spaces or control characters) and bounded
    /// fields.
    ///
    /// Why: names are what `tl:name` and `--template` match exactly, so one
    /// with an invisible difference could never be typed; and the fields are
    /// applied into a create, which is held to 64 KiB.
    #[test]
    fn a_template_needs_a_usable_name_and_bounded_fields() {
        let named = |name: &str| template(name, TemplateFields::default());
        assert!(check_template_shape(&named("my-codex")).is_ok());
        for bad in [
            "",
            " padded",
            "line\nbreak",
            &"x".repeat(TEMPLATE_NAME_CAP + 1),
        ] {
            assert!(check_template_shape(&named(bad)).is_err(), "{bad:?}");
        }
        let huge = template(
            "huge",
            TemplateFields {
                command: Some("x".repeat(TEMPLATE_FIELDS_CAP)),
                ..TemplateFields::default()
            },
        );
        assert!(check_template_shape(&huge).is_err());
    }

    /// Dot path segments cannot survive template request URL normalization.
    /// Refuse those exact names at save validation; three dots is an ordinary
    /// name and must not be rejected by an overbroad dot-only rule.
    #[test]
    fn template_names_refuse_url_navigation_segments() {
        for name in [".", ".."] {
            assert_eq!(
                check_template_shape(&template(name, TemplateFields::default())),
                Err("a template name cannot be . or ..".into())
            );
        }
        for name in ["...", "a.b", ".hidden"] {
            assert!(check_template_shape(&template(name, TemplateFields::default())).is_ok());
        }
    }

    /// Single-line editors cannot preserve hidden command characters. Every
    /// save uses this refusal, naming the affected field; absent and reset
    /// commands, ordinary Unicode and quoted shell arguments remain valid.
    #[test]
    fn template_commands_refuse_controls_and_invisible_formatting() {
        for bad in [
            "echo a\nb",
            "echo\targument",
            "echo \u{200B}hidden",
            "echo \u{202E}reverse",
        ] {
            for (field, fields) in [
                (
                    "launch command",
                    TemplateFields {
                        command: Some(bad.into()),
                        ..Default::default()
                    },
                ),
                (
                    "resume command",
                    TemplateFields {
                        resume_command: Some(Some(bad.into())),
                        ..Default::default()
                    },
                ),
            ] {
                let refusal = check_template_shape(&template("unsafe", fields)).unwrap_err();
                assert!(refusal.contains(field), "{refusal}");
                assert!(
                    refusal.contains("control or invisible formatting"),
                    "{refusal}"
                );
            }
        }
        for fields in [
            TemplateFields::default(),
            TemplateFields {
                resume_command: Some(None),
                ..Default::default()
            },
            TemplateFields {
                command: Some("echo '世界 ❤️'".into()),
                resume_command: Some(Some("tool --resume '{conversation}'".into())),
                ..Default::default()
            },
        ] {
            check_template_shape(&template("ordinary", fields)).unwrap();
        }
    }

    /// The new save rule must not retroactively disable stored templates.
    /// Applying a legacy multiline command preserves its bytes; attempting
    /// to save any edit still refuses until that command is repaired.
    #[test]
    fn legacy_template_commands_still_apply_but_cannot_be_saved() {
        let legacy = template(
            "legacy",
            TemplateFields {
                kind: Some(LauncherKind::Command),
                command: Some("echo 'first\nsecond'".into()),
                ..Default::default()
            },
        );
        assert!(check_template_shape(&legacy).is_err());
        let applied = apply_template(
            &LauncherState::default(),
            &legacy,
            &context(&[], &[], false),
        )
        .unwrap();
        assert_eq!(applied.command, "echo 'first\nsecond'");
    }
}

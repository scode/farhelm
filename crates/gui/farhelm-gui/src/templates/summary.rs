//! A template's one-line summary: what it sets, in a fixed order, as the Templates list shows it under each name.
//!
//! Based on the old app's `template_summary_parts` (`list/templates.rs` at reference commit `24bc0580`). The quick
//! switcher's template section and Cmd+J list show the same summary. An unavailable permission leads the summary
//! because the list can run out of space before it reaches the usual permissions field.
//!
//! The summary names the fields a template sets and says as little as it can about their values: a command or resume
//! command is named, never shown, because it can be long; a model, a session name, custom permission words and a
//! wrapper prefix are shown, since they are short and are what tells two templates apart. Those values are written by
//! whoever saved the template, an agent included, so they are host-supplied text: the summary hands them back as
//! [`Part::Host`] or [`Part::Mono`], so the line can escape them and isolate their direction from the app's words.
//!
//! SPEC.md says the list summary "mentions" an initial prompt. The old app's word for it was "Initial prompt", which is
//! what this keeps.
//!
//! The drawing is shared too ([`line`], and the ⚡ chord's text size, [`CHORD_TEXT`]), so that the lists show one
//! summary the same way rather than each keeping a copy.

use farhelm_proto::launcher::{LauncherKind, TemplateFields};
use farhelm_proto::{LaunchEffortValue, LaunchHarness, LaunchPermission};

use gpui::{Div, ElementId, ParentElement as _, Styled as _, div};

use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::harness_label;
use crate::permission::words::{effort_label, guarded_mode_name, permission_display};

/// One run of a summary. Runs are drawn one after another on one line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    /// The app's own words.
    Words(String),
    /// A value from the template, drawn as host text in the interface face.
    Host(String),
    /// A value from the template that is shell words (custom permission flags, a wrapper prefix), drawn as host text
    /// in the monospace face.
    Mono(String),
}

/// What separates two fields' parts.
const SEPARATOR: &str = " · ";

/// `.templates-quick-chord`: the text size of the ⚡ chord where a list names it beside a template (the Templates
/// list's marked row, the quick switcher's Quick launch row), smaller than the 12-pixel summary.
pub(crate) const CHORD_TEXT: f32 = 11.;

/// Draw the summary as one line so an ellipsis can fall in any run, including a monospace value.
/// The caller supplies a row-specific id. Assistive technology reads the whole escaped summary at that id even
/// when the row only has room for its start. The caller still sets the text size and ink.
pub(crate) fn line(id: impl Into<ElementId>, parts: &[Part]) -> Div {
    let sentence = parts
        .iter()
        .fold(HostSentence::new(id), |sentence, part| match part {
            Part::Words(words) => sentence.text(words.clone()),
            Part::Host(value) => sentence.host(&HostText::from_host(value.as_str())),
            Part::Mono(value) => sentence.mono(&HostText::from_host(value.as_str())),
        });
    div().min_w_0().truncate().child(sentence)
}

/// The summary of `fields`, in the old app's order: initial prompt, launch kind, agent type, model, effort,
/// permissions, custom words, wrapper prefix, command, retired container, YOLO, resume command, placement, destination,
/// session name. An unavailable permission comes first so the row can show it even when the rest is clipped. A
/// template that sets nothing says "Sets nothing yet". Applying a template ignores retired workspace trust.
pub fn parts(fields: &TemplateFields) -> Vec<Part> {
    let mut summary = Summary::default();
    let unavailable = unavailable_permission_label(fields.agent, fields.permissions.flatten());
    if let Some(label) = &unavailable {
        summary.words(label);
    }
    if fields.prompt.is_some() {
        summary.words("Initial prompt");
    }
    if let Some(kind) = fields.kind {
        summary.words(match kind {
            LauncherKind::Agent => "Agent launch",
            LauncherKind::Command => "Command launch",
        });
    }
    if let Some(Some(id)) = &fields.custom_agent {
        summary.field(vec![Part::Host(id.clone())]);
    } else if let Some(agent) = fields.agent {
        summary.words(harness_label(agent));
    }
    match &fields.model {
        Some(Some(model)) => summary.field(vec![Part::Host(model.clone())]),
        Some(None) => summary.words("Model default"),
        None => {}
    }
    match &fields.effort {
        Some(Some(LaunchEffortValue::Known(effort))) => {
            summary.words(&format!("Effort {}", effort_label(Some(*effort))));
        }
        Some(Some(LaunchEffortValue::Custom(effort))) => summary.field(vec![
            Part::Words("Effort ".into()),
            Part::Host(effort.clone()),
        ]),
        Some(None) => summary.words("Effort default"),
        None => {}
    }
    if let Some(permissions) = fields.permissions
        && unavailable.is_none()
    {
        summary.words(&format!(
            "Approvals {}",
            permission_label(fields.agent, permissions)
        ));
    }
    if let Some(words) = &fields.permission_flags {
        summary.field(vec![Part::Mono(farhelm_proto::join_permission_flags(
            words,
        ))]);
    }
    match &fields.wrapper_prefix {
        Some(Some(words)) => summary.field(vec![
            Part::Words("Wrapper prefix ".to_owned()),
            Part::Mono(words.join(" ")),
        ]),
        Some(None) => summary.words("Wrapper prefix none"),
        None => {}
    }
    if fields.command.is_some() {
        summary.words("Command");
    }
    match &fields.container {
        Some(Some(_)) => summary.words("Retired containerized setting"),
        Some(None) => summary.words("Retired containerized reset"),
        None => {}
    }
    if let Some(yolo) = fields.yolo {
        summary.words(if yolo { "YOLO" } else { "Not YOLO" });
    }
    match &fields.resume_command {
        Some(Some(_)) => summary.words("Resume command"),
        Some(None) => summary.words("Resume default"),
        None => {}
    }
    // A recipe-and-place target reads as the environment it makes ("New web container on host 1", "New dev sandbox"),
    // in the launcher's words for it. The old summary passed no hosts, so a container's place is named by its id rather
    // than the host's name, and a summary never changes with the host list.
    if let Some(target) = &fields.target {
        summary.field(vec![
            Part::Words("New ".to_owned()),
            Part::Host(crate::environment_ui::target_label(target, None)),
        ]);
    }
    if let Some(recipe) = &fields.recipe {
        summary.field(vec![
            Part::Words("New sandbox · ".to_owned()),
            Part::Host(recipe.clone()),
        ]);
    } else if fields.host.is_some() {
        summary.words("Host");
    }
    if fields.destination.is_some() {
        summary.words("Destination");
    }
    if let Some(name) = &fields.name {
        summary.field(vec![
            Part::Words("Name ".to_owned()),
            Part::Host(name.clone()),
        ]);
    }
    summary.finish()
}

/// The summary as plain text, values unescaped, for tests to compare whole summaries. Never draw this; the list draws
/// [`parts`] so the values go through host text.
#[cfg(test)]
pub fn plain(fields: &TemplateFields) -> String {
    parts(fields)
        .into_iter()
        .map(|part| match part {
            Part::Words(text) | Part::Host(text) | Part::Mono(text) => text,
        })
        .collect()
}

/// Describe the permission the template stores so a list or approval card can identify the actual choice. An
/// explicit mode the agent cannot take keeps its name and says it is unavailable. Only a reset resolves to the
/// agent's default. With no agent type a reset stays agent-dependent, and a shell has no default permission.
pub(crate) fn permission_label(
    agent: Option<LaunchHarness>,
    permission: Option<LaunchPermission>,
) -> String {
    let Some(harness) = agent else {
        return match permission {
            Some(_) => permission_display(permission).to_owned(),
            None => "Agent-dependent mode".to_owned(),
        };
    };
    if harness.is_shell() {
        return permission
            .map_or("None (shell)", |mode| permission_display(Some(mode)))
            .to_owned();
    }
    if let Some(label) = unavailable_permission_label(agent, permission) {
        return label;
    }
    match permission.or_else(|| harness.omitted_permission()) {
        Some(LaunchPermission::Guarded) => guarded_mode_name(harness)
            .unwrap_or_else(|| permission_display(Some(LaunchPermission::Guarded)))
            .to_owned(),
        Some(chosen) => permission_display(Some(chosen)).to_owned(),
        None => permission_display(None).to_owned(),
    }
}

/// A named non-shell agent can reject an explicit mode. Use one label for the list and the approval card.
fn unavailable_permission_label(
    agent: Option<LaunchHarness>,
    permission: Option<LaunchPermission>,
) -> Option<String> {
    let (agent, mode) = (agent?, permission?);
    (!agent.is_shell() && !agent.offers_permission(mode))
        .then(|| format!("{} (not offered)", permission_display(Some(mode))))
}

/// The parts collected so far, with the separator put in between fields.
#[derive(Default)]
struct Summary {
    parts: Vec<Part>,
}

impl Summary {
    fn words(&mut self, words: &str) {
        self.field(vec![Part::Words(words.to_owned())]);
    }

    fn field(&mut self, runs: Vec<Part>) {
        if !self.parts.is_empty() {
            self.parts.push(Part::Words(SEPARATOR.to_owned()));
        }
        self.parts.extend(runs);
    }

    fn finish(self) -> Vec<Part> {
        if self.parts.is_empty() {
            vec![Part::Words("Sets nothing yet".to_owned())]
        } else {
            self.parts
        }
    }
}

//! Save as template's model: what the launcher offers to keep, and what is checked when the panel opens.
//!
//! SPEC.md, "Launch templates" (the Save as template sentences): an inline checklist "capture[s] the current tab's
//! choices, host, destination, and session name", with some values checked and others offered unchecked, and "The saved
//! template contains exactly the checked fields and always the active launch kind". SPEC_impl.md, "Launch templates":
//! the panel "shares the snapshot reader used for application" ([`super::apply::snapshot`]), and "A pure mapping offers
//! only the active tab's values and marks explicit choices separately from remembered defaults and Clone's inherited
//! placement". This module is that mapping, with no GPUI types; the panel drawing it is [`super::save_panel`].
//!
//! Ported from the old app's `Candidate` and `choice_text` (`list/save_template.rs` at reference commit `24bc0580`).
//! Where the old launcher kept its own flags for what the user chose by hand, this reads the draft's own record of it:
//! the folder's and the name's touched marks, and the draft's `host_chosen`, `permissions_explicit` and
//! `wrapper_explicit`, all set by its hand-edit methods, which template application goes through too.
//!
//! The destination offered is the snapshot's: the folder, or a managed checkout's chosen repository, which is always
//! checked (SPEC.md: "A GitHub repository is always checked"). While a checkout is chosen the set-aside folder is not
//! offered, and a repository field holding only typed text offers no destination.

use farhelm_proto::launcher::{
    LauncherKind, TemplateDestination, TemplateFields, TemplateNameKind,
};
use farhelm_proto::ui_channel::HostView;

use super::fields::Field;
use super::summary::permission_label;
use crate::launcher::draft::{LaunchDraft, harness_label};
use crate::permission::words::launch_effort_label;

/// The values the panel offers and which of them start checked, fixed when the panel opens: the old panel's snapshot
/// "stays fixed while this inline panel is open, even if the surrounding launcher changes".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// Every value offered, as the template would store it. A field the launcher has no value for is absent, and so not
    /// offered.
    pub fields: TemplateFields,
    /// The offered fields checked when the panel opens, in the editor's field order.
    pub checked: Vec<Field>,
}

impl Candidate {
    /// What the launcher's `draft` offers, read through the one launcher snapshot, with `hosts` naming the draft's host
    /// by its install identity (a host without a usable one is not offered).
    ///
    /// Only the active tab's values are read: the other tab's draft is kept for a later switch, never saved. Agent type,
    /// model, effort, the initial prompt, the command, its YOLO answer and an opted-in resume command start checked
    /// whenever they are set, Clone's launch choices included. Permissions and the wrapper prefix start checked only
    /// after an explicit choice, the host only after a hand pick, the folder only after typing or picking it, and the
    /// session name only after typing it or applying a template that sets it; until then they are offered unchecked. A
    /// permission or prompt the chosen agent type cannot take is not offered at all, even when a Clone's or a recent
    /// setup's choice marks it explicit, so the default checklist always saves a template that fits.
    pub fn from_draft(draft: &LaunchDraft, hosts: &[HostView]) -> Candidate {
        let state = super::apply::snapshot(draft, hosts);
        // A template's base name comes through the snapshot as its original text; a typed name has no kind, and is
        // offered as a base name, whose kind is the field's absence.
        let name = state.name.clone().filter(|name| !name.is_empty());
        let name_kind = name.as_ref().and(
            state
                .name_kind
                .filter(|kind| *kind == TemplateNameKind::Fixed),
        );
        let mut fields = TemplateFields {
            kind: state.kind,
            host: state.host.clone(),
            destination: state.destination.clone(),
            name,
            name_kind,
            ..TemplateFields::default()
        };
        let explicit_permissions = draft.agent.permissions_explicit;
        match state.kind {
            Some(LauncherKind::Agent) => {
                fields.prompt = state.prompt.clone();
                fields.agent = state.harness;
                fields.model = state.model.clone().map(Some);
                fields.effort = state.effort.map(Some);
                // An explicit choice of the agent's default is saved as a reset (present, null); a passive one is not
                // offered.
                fields.permissions = (state.permissions.is_some() || explicit_permissions)
                    .then_some(state.permissions);
                fields.permission_flags = state.permission_flags.clone();
                // Offered even when empty ("None"), so an explicitly cleared prefix saves as a reset.
                fields.wrapper_prefix = Some(state.wrapper_prefix.clone());
                for field in [Field::Permissions, Field::Prompt] {
                    if !field.offered(&fields) {
                        field.remove(&mut fields);
                    }
                }
            }
            Some(LauncherKind::Command) => {
                fields.agent = state.command_agent;
                fields.command = (!state.command.is_empty()).then(|| state.command.clone());
                fields.yolo = state.yolo;
                fields.resume_command = state.resume_command.clone().map(Some);
            }
            None => {}
        }
        let checked = Field::ALL
            .into_iter()
            .filter(|field| {
                field.is_set(&fields)
                    && match field {
                        Field::Permissions => explicit_permissions,
                        Field::Wrapper => draft.agent.wrapper_explicit,
                        Field::Host => draft.host_chosen,
                        Field::Destination => {
                            draft.folder.is_touched()
                                || matches!(
                                    fields.destination,
                                    Some(TemplateDestination::Github(_))
                                )
                        }
                        Field::Name => draft.name.is_touched(),
                        _ => true,
                    }
            })
            .collect();
        Candidate { fields, checked }
    }

    /// The fields to save with `checked` checked and the session name as `name_kind` (`None` is a base name): the active
    /// launch kind always, and every unchecked field removed. Unchecking the name drops its kind too, even when Fixed was
    /// chosen before.
    pub fn selected(
        &self,
        checked: &[Field],
        name_kind: Option<TemplateNameKind>,
    ) -> TemplateFields {
        let mut fields = self.fields.clone();
        for field in Field::ALL {
            if !checked.contains(&field) {
                field.remove(&mut fields);
            }
        }
        fields.name_kind = fields.name.as_ref().and(name_kind);
        fields
    }
}

/// How the checklist shows `field`'s offered value, as raw text for the host-text element to draw escaped (template
/// text may hold characters an agent or the command line wrote). The old panel's `choice_text`: an initial prompt by its
/// first line, with an ellipsis when more lines follow; approvals and effort in the launcher's words; an empty wrapper
/// prefix as "None"; an unset value as "Default". The host is the launcher's own label for it, which the panel draws
/// from the host row instead, so it is `None` here.
pub fn choice_text(field: Field, fields: &TemplateFields) -> Option<String> {
    let value = match field {
        Field::Prompt => fields
            .prompt
            .as_ref()
            .map(|prompt| match prompt.split_once('\n') {
                Some((first, _)) => format!("{first}…"),
                None => prompt.clone(),
            }),
        Field::Agent => fields.agent.map(|agent| harness_label(agent).to_owned()),
        Field::Model => fields.model.clone().flatten(),
        Field::Effort => fields
            .effort
            .as_ref()
            .and_then(Option::as_ref)
            .map(|effort| launch_effort_label(Some(effort)).to_owned()),
        Field::Permissions => Some(format!(
            "{}{}",
            permission_label(fields.agent, fields.permissions.flatten()),
            fields
                .permission_flags
                .as_deref()
                .map(|words| format!(" · {}", farhelm_proto::join_permission_flags(words)))
                .unwrap_or_default(),
        )),
        Field::Wrapper => Some(
            fields
                .wrapper_prefix
                .clone()
                .flatten()
                .map(|words| words.join(" "))
                .unwrap_or_else(|| "None".to_owned()),
        ),
        Field::Command => fields.command.clone(),
        Field::Yolo => fields
            .yolo
            .map(|yolo| if yolo { "YOLO" } else { "Keeps safety checks" }.to_owned()),
        Field::Resume => fields.resume_command.clone().flatten(),
        Field::Destination => fields
            .destination
            .as_ref()
            .map(|destination| match destination {
                TemplateDestination::Folder(text) => text.clone(),
                TemplateDestination::Github(text) => format!("gh:{text}"),
            }),
        Field::Name => fields.name.clone(),
        Field::Host | Field::Container => return None,
    };
    Some(value.unwrap_or_else(|| "Default".to_owned()))
}

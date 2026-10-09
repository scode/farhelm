//! Save a reviewed launcher snapshot as partial edits, never as a session.
//!
//! Available values and deliberate choices are separate: a remembered default
//! may be offered without being checked. The snapshot stays fixed while this
//! inline panel is open, even if the surrounding launcher changes.

use super::templates::{Field, new_name_refusal, saved_fields_refusal, word};
use crate::{ApiBase, api, peer::display_peer};
use dioxus::prelude::*;
use farhelm_proto::launcher::{
    LaunchCatalogModel, LaunchTemplate, LauncherKind, LauncherState, TemplateDestination,
    TemplateFields, check_template_shape,
};

/// Provenance needed only for the launcher's pre-checked save choices.
/// Agent/model/effort and command choices have no passive seeds; the remaining
/// fields distinguish an explicit edit from defaults and Clone's placement.
#[derive(Default)]
pub(super) struct ExplicitChoices {
    pub host: bool,
    pub folder: bool,
    pub name: bool,
    pub permissions: bool,
    pub trust: bool,
}

/// All values offered when the panel opens, plus the default selection.
/// Filtering uses the editor's field identities so unchecked means absent,
/// including for false values and explicit default resets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Candidate {
    pub fields: TemplateFields,
    pub checked: Vec<Field>,
}

impl Candidate {
    /// Read only the active tab; dormant drafts must never become saved edits.
    /// An explicit default approval/trust choice is a present null reset.
    pub(super) fn from_state(state: &LauncherState, explicit: ExplicitChoices) -> Self {
        let mut fields = TemplateFields {
            kind: state.kind,
            host: state.host.clone(),
            destination: state.destination.clone(),
            name: state.name.clone().filter(|s| !s.is_empty()),
            ..Default::default()
        };
        match state.kind {
            Some(LauncherKind::Agent) => {
                fields.agent = state.harness;
                fields.model = state.model.clone().map(Some);
                fields.effort = state.effort.map(Some);
                fields.permissions = (state.permissions.is_some() || explicit.permissions)
                    .then_some(state.permissions);
                fields.workspace_trust = (state.workspace_trust.is_some() || explicit.trust)
                    .then_some(state.workspace_trust);
                // Clone and recents mark their launch choices explicit even
                // when this agent has no corresponding control. A forced mode
                // or unsupported reset is not a choice the launcher can save.
                for field in [Field::Permissions, Field::Trust] {
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
                        Field::Permissions => explicit.permissions,
                        Field::Trust => explicit.trust,
                        Field::Host => explicit.host,
                        Field::Destination => {
                            explicit.folder
                                || matches!(
                                    fields.destination,
                                    Some(TemplateDestination::Github(_))
                                )
                        }
                        Field::Name => explicit.name,
                        _ => true,
                    }
            })
            .collect();
        Self { fields, checked }
    }

    /// Keep the active tab unconditionally and remove each unchecked edit.
    /// Saving never infers a different kind from the remaining checklist.
    fn selected(&self, checked: &[Field]) -> TemplateFields {
        let mut fields = self.fields.clone();
        for field in Field::ALL {
            if !checked.contains(&field) {
                field.remove(&mut fields);
            }
        }
        fields
    }
}

/// Render one offered choice without displaying peer control characters.
/// Boolean approval answers use the same words as the launcher's controls;
/// the host's snapshot label identifies the persisted binding to the user.
fn choice_text(field: Field, fields: &TemplateFields, host_label: &str) -> String {
    let value = match field {
        Field::Agent => fields.agent.map(word),
        Field::Model => fields.model.clone().flatten(),
        Field::Effort => fields.effort.flatten().map(word),
        Field::Permissions => fields
            .permissions
            .flatten()
            .map(|v| v.wire_word().replace('_', " ")),
        Field::Trust => fields
            .workspace_trust
            .flatten()
            .map(|v| if v { "trust" } else { "ask" }.into()),
        Field::Command => fields.command.clone(),
        Field::Yolo => fields.yolo.map(|v| if v { "YOLO" } else { "asks" }.into()),
        Field::Resume => fields.resume_command.clone().flatten(),
        // The identity is the persisted binding, but the launcher label is
        // what lets a user recognize the host they are choosing to keep.
        Field::Host => Some(host_label.to_owned()),
        Field::Destination => fields.destination.as_ref().map(|v| match v {
            TemplateDestination::Folder(s) => s.clone(),
            TemplateDestination::Github(s) => format!("gh:{s}"),
        }),
        Field::Name => fields.name.clone(),
    };
    display_peer(value.as_deref().unwrap_or("default"))
}

/// An inline save surface with its own keys and guarded asynchronous write.
/// The parent holds `saving` too, preventing an accepted save from losing its
/// future to launcher cancellation or a competing launch.
#[component]
pub(super) fn SaveTemplatePanel(
    candidate: Candidate,
    host_label: String,
    catalog: Option<Vec<LaunchCatalogModel>>,
    mut saving: Signal<bool>,
    on_cancel: EventHandler<()>,
    on_saved: EventHandler<LaunchTemplate>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut name = use_signal(String::new);
    let mut checked = use_signal(|| candidate.checked.clone());
    let mut error = use_signal(|| None::<String>);
    let save_candidate = candidate.clone();
    let save = use_callback(move |()| {
        if saving() {
            return;
        }
        let template = LaunchTemplate {
            name: name(),
            fields: save_candidate.selected(&checked()),
        };
        let refusal = check_template_shape(&template)
            .err()
            .map(|e| e.to_string())
            .or_else(|| saved_fields_refusal(&template, catalog.as_deref()));
        if let Some(refusal) = refusal {
            error.set(Some(refusal));
            return;
        }
        saving.set(true);
        error.set(None);
        let base = base.clone();
        spawn(async move {
            let listed = api::fetch_templates(&base).await;
            if let Some(refusal) = new_name_refusal(&template.name, listed.as_deref().ok()) {
                error.set(Some(refusal));
                saving.set(false);
                return;
            }
            match api::put_template(&base, &template.name, &template.fields).await {
                Ok(()) => {
                    saving.set(false);
                    on_saved.call(template);
                }
                Err(refusal) => {
                    error.set(Some(refusal));
                    saving.set(false);
                }
            }
        });
    });
    rsx! {
        section { class: "save-template-panel", aria_label: "save as template",
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    event.prevent_default(); event.stop_propagation();
                    if !saving() { on_cancel.call(()); }
                }
            },
            label { "template name"
                input { class: "save-template-name", r#type: "text", value: "{name}",
                    aria_label: "template name", "data-tooltip": "template name: choose a new name for this setup",
                    disabled: saving(), oninput: move |event| name.set(event.value()),
                    onkeydown: move |event| {
                        if event.key() == Key::Enter {
                            event.prevent_default(); event.stop_propagation(); save.call(());
                        }
                    },
                    onmounted: move |_| {
                        document::eval("requestAnimationFrame(() => document.querySelector('.save-template-name')?.focus())");
                    },
                }
            }
            p { "choose which parts of this setup the template keeps" }
            for field in Field::ALL.into_iter().filter(|f| f.is_set(&candidate.fields)) {
                label { class: "save-template-choice", "data-tooltip": "keep this {field.label()} in the template",
                    input { r#type: "checkbox", "data-field": field.key(), checked: checked().contains(&field),
                        disabled: saving(), onchange: move |event| {
                            checked.with_mut(|fields| {
                                fields.retain(|f| *f != field);
                                if event.checked() { fields.push(field); }
                            });
                        },
                    }
                    span { "{field.label()}: "
                        span { class: "peer-value", dir: "ltr", "{choice_text(field, &candidate.fields, &host_label)}" }
                    }
                }
            }
            if candidate.fields.host.is_none() {
                p { "this host has not been identified, so it cannot be saved in a template" }
            }
            if let Some(message) = error() { p { class: "save-template-error", role: "alert", "{message}" } }
            div { class: "save-template-actions",
                button { r#type: "button", class: "btn btn-primary save-template-save", disabled: saving(),
                    "data-tooltip": "save: keep the checked choices and open the new template",
                    onclick: move |_| save.call(()), if saving() { "saving…" } else { "save" }
                }
                button { r#type: "button", class: "btn btn-neutral save-template-cancel", disabled: saving(),
                    "data-tooltip": "cancel: return to the launcher without saving a template",
                    onclick: move |_| { if !saving() { on_cancel.call(()); } }, "cancel"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::launcher::{TemplateContext, apply_template};
    use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission};

    /// Clone/recent provenance and a prior agent's edits cannot manufacture
    /// unavailable controls. The default checklist must save a valid setup
    /// without requiring the user to uncheck unsupported or forced modes.
    #[test]
    fn unsupported_controls_are_absent_despite_explicit_provenance() {
        for (harness, permissions, trust) in [
            (LaunchHarness::Claude, Some(None), None),
            (LaunchHarness::Pi, None, Some(None)),
            (LaunchHarness::OpenCode, None, None),
            (LaunchHarness::Codex, Some(None), Some(None)),
        ] {
            let state = LauncherState {
                kind: Some(LauncherKind::Agent),
                harness: Some(harness),
                permissions: harness.offers_only_yolo().then_some(LaunchPermission::Yolo),
                // Both Clone seeding and a Codex-to-Claude transition can
                // leave explicit trust provenance after its value is cleared.
                workspace_trust: None,
                ..Default::default()
            };
            let candidate = Candidate::from_state(
                &state,
                ExplicitChoices {
                    permissions: true,
                    trust: true,
                    ..Default::default()
                },
            );
            assert_eq!(candidate.fields.workspace_trust, trust, "{harness:?}");
            assert_eq!(candidate.checked.contains(&Field::Trust), trust.is_some());
            assert_eq!(candidate.fields.permissions, permissions, "{harness:?}");
            assert_eq!(
                candidate.checked.contains(&Field::Permissions),
                permissions.is_some()
            );
            let template = LaunchTemplate {
                name: "supported choices".into(),
                fields: candidate.selected(&candidate.checked),
            };
            assert_eq!(saved_fields_refusal(&template, Some(&[])), None);
        }
    }

    /// Passive defaults and Clone's placement stay offered but unchecked;
    /// deliberate edits, including a false trust answer, must be retained.
    #[test]
    fn prechecks_follow_explicitness_without_hiding_available_values() {
        let state = LauncherState {
            kind: Some(LauncherKind::Agent),
            harness: Some(LaunchHarness::Codex),
            model: Some("custom-model".into()),
            effort: Some(LaunchEffort::High),
            permissions: Some(LaunchPermission::Yolo),
            workspace_trust: Some(false),
            host: Some("host-install".into()),
            destination: Some(TemplateDestination::Folder("~".into())),
            name: Some("copied name".into()),
            ..Default::default()
        };
        let passive = Candidate::from_state(&state, ExplicitChoices::default());
        assert_eq!(
            passive.checked,
            vec![Field::Agent, Field::Model, Field::Effort]
        );
        for field in [
            Field::Host,
            Field::Destination,
            Field::Name,
            Field::Permissions,
            Field::Trust,
        ] {
            assert!(
                field.is_set(&passive.fields),
                "{} remains offered",
                field.label()
            );
        }
        let deliberate = Candidate::from_state(
            &state,
            ExplicitChoices {
                host: true,
                folder: true,
                name: true,
                permissions: true,
                trust: true,
            },
        );
        assert_eq!(deliberate.checked.len(), 8);
        assert_eq!(
            deliberate.selected(&deliberate.checked).workspace_trust,
            Some(Some(false))
        );
        let reset = Candidate::from_state(
            &LauncherState {
                permissions: None,
                ..state
            },
            ExplicitChoices {
                permissions: true,
                ..Default::default()
            },
        );
        assert!(reset.checked.contains(&Field::Permissions));
        assert_eq!(reset.fields.permissions, Some(None));
    }

    /// The active tab selects one draft. Command resume off must be absent,
    /// while a repository remains deliberate even when Clone supplied it.
    #[test]
    fn command_snapshot_omits_dormant_agent_fields_and_resume_off() {
        let state = LauncherState {
            kind: Some(LauncherKind::Command),
            command: "printf hello".into(),
            yolo: Some(false),
            command_agent: Some(LaunchHarness::Claude),
            harness: Some(LaunchHarness::Codex),
            model: Some("dormant-model".into()),
            permissions: Some(LaunchPermission::Yolo),
            destination: Some(TemplateDestination::Github("owner/repo".into())),
            ..Default::default()
        };
        let candidate = Candidate::from_state(&state, ExplicitChoices::default());
        assert_eq!(
            candidate.checked,
            vec![
                Field::Command,
                Field::Yolo,
                Field::Agent,
                Field::Destination
            ]
        );
        assert_eq!(candidate.fields.model, None);
        assert_eq!(candidate.fields.permissions, None);
        assert_eq!(candidate.fields.resume_command, None);
        assert_eq!(candidate.fields.yolo, Some(false));
        let selected = candidate.selected(&[Field::Command]);
        assert_eq!(selected.kind, Some(LauncherKind::Command));
        assert_eq!(selected.agent, None);
        assert_eq!(selected.yolo, None);
    }

    /// Applying a saved selection to a fresh launcher must reproduce the
    /// selected choices; unchecked placement cannot leak through the save.
    #[test]
    fn saved_agent_and_command_choices_round_trip_through_shared_application() {
        let context = TemplateContext {
            catalog: &[],
            known_hosts: &["host-install".into()],
            host_fixed: false,
        };
        for state in [
            LauncherState {
                kind: Some(LauncherKind::Agent),
                harness: Some(LaunchHarness::Codex),
                model: Some("custom-model".into()),
                effort: Some(LaunchEffort::High),
                permissions: Some(LaunchPermission::Yolo),
                workspace_trust: Some(true),
                host: Some("host-install".into()),
                destination: Some(TemplateDestination::Folder("/work".into())),
                name: Some("review".into()),
                ..Default::default()
            },
            LauncherState {
                kind: Some(LauncherKind::Command),
                command_agent: Some(LaunchHarness::Claude),
                command: "printf 'raw\\ttext'".into(),
                yolo: Some(false),
                resume_command: Some("printf resume".into()),
                destination: Some(TemplateDestination::Github("owner/repo".into())),
                ..Default::default()
            },
        ] {
            let candidate = Candidate::from_state(
                &state,
                ExplicitChoices {
                    permissions: true,
                    trust: true,
                    ..Default::default()
                },
            );
            let template = LaunchTemplate {
                name: "saved".into(),
                fields: candidate.selected(&candidate.checked),
            };
            check_template_shape(&template).expect("fixture template fits the wire contract");
            let applied = apply_template(&LauncherState::default(), &template, &context)
                .expect("saved choices apply");
            assert_eq!(applied.kind, state.kind);
            if state.kind == Some(LauncherKind::Agent) {
                assert_eq!(applied.harness, state.harness);
                assert_eq!(applied.model, state.model);
                assert_eq!(applied.effort, state.effort);
                assert_eq!(applied.permissions, state.permissions);
                assert_eq!(applied.workspace_trust, state.workspace_trust);
                assert_eq!(applied.destination, None);
            } else {
                assert_eq!(applied.command, state.command);
                assert_eq!(applied.command_agent, state.command_agent);
                assert_eq!(applied.yolo, state.yolo);
                assert_eq!(applied.resume_command, state.resume_command);
                assert_eq!(applied.destination, state.destination);
            }
            assert_eq!(applied.host, None);
            assert_eq!(applied.name, None);
        }
    }
}

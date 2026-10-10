//! Turning an agent's create request into a launch (SPEC.md, Agent-spawned
//! sessions): the templates `farhelm agent create` and `farhelm spawn` name,
//! then their flags, applied to an empty launcher exactly as the GUI applies
//! templates, and the result validated like a GUI launch.
//!
//! The helm does this rather than the CLI or the supervisor because it owns
//! both inputs that are not on the command line: the templates
//! (`templates.rs`) and the release catalog an agent launch is composed
//! from (`launches.rs`). The application itself is the wire crate's
//! `farhelm_proto::launcher::apply_templates`, the same function the
//! launcher's `tl:` search runs, so the CLI and the GUI cannot disagree
//! about what a template means.
//!
//! What is deliberately absent: the remembered GUI defaults. An empty
//! launcher here is empty, so a field no template or flag set is a refusal
//! naming the flag, never a value borrowed from the user's last launch
//! (SPEC.md: "the same templates can launch differently from the CLI than
//! from a GUI that preselected a remembered permission").
//!
//! Which host the session lands on is decided by the caller
//! (`agent_requests::create_for_agent`), because it is the one that knows
//! whether this is a spawn and how a `--host` name resolves; this module
//! reports only the host identity a template named.

use farhelm_proto::launcher::{
    LaunchCatalogModel, LaunchTemplate, LauncherKind, LauncherState, TemplateContext,
    TemplateDestination, TemplateFields, TemplateRefusal, apply_template, apply_templates,
};
use farhelm_proto::{CommandLaunch, ErrorKind, LaunchRequest, LaunchSelection, SessionLaunch};

use crate::hosts::{HostStateView, HostView};

/// What an agent's create resolved to, before a host is chosen. Resolved
/// afresh on every attempt; a keyed retry is matched on its host by the
/// request, not by this (`agent_requests::AgentRequestDigest`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolution {
    pub(crate) cwd: String,
    pub(crate) launch: SessionLaunch,
    pub(crate) title: Option<String>,
    /// The install identity a template set as the host, if one did. The
    /// caller uses it only when no `--host` was given.
    pub(crate) template_host: Option<String>,
}

use crate::sessions::invalid_request as invalid;

/// The release catalog in the shape template application reads.
fn catalog() -> Vec<LaunchCatalogModel> {
    crate::launches::catalog()
        .iter()
        .map(|model| LaunchCatalogModel {
            id: model.id.to_string(),
            harness: model.harness,
            efforts: model.efforts.to_vec(),
        })
        .collect()
}

/// The install identities a template's `host` may name: every registered
/// host with a recorded identity that is not currently reporting another
/// one. A host in identity mismatch is excluded for the reason the GUI
/// excludes it (`create_form::template_host`): its recorded identity is
/// the install it used to be, not the one now answering.
pub(crate) fn template_host_identities(views: &[HostView]) -> Vec<String> {
    views
        .iter()
        .filter(|view| !matches!(view.state, HostStateView::IdentityMismatch { .. }))
        .filter_map(|view| view.identity.clone())
        .collect()
}

/// The host row a template's install identity names, by the rule
/// [`template_host_identities`] states.
pub(crate) fn template_host_row<'v>(views: &'v [HostView], identity: &str) -> Option<&'v HostView> {
    views.iter().find(|view| {
        view.identity.as_deref() == Some(identity)
            && !matches!(view.state, HostStateView::IdentityMismatch { .. })
    })
}

/// The flag a launcher field is set by, for refusals of the flags
/// themselves: the field words are `apply_template`'s.
fn flag_for(field: &str) -> &'static str {
    match field {
        "model" => "--model",
        "effort" => "--effort",
        "permissions" => "--permissions",
        "workspace trust" => "--trust",
        "command" => "--command",
        "YOLO answer" => "--yolo/--no-yolo",
        "resume command" => "--resume-command",
        _ => "a flag",
    }
}

/// A refusal of the flags, which are applied as one more template but are
/// not one: name the flag rather than an unnamed template.
fn flag_refusal(refusal: TemplateRefusal) -> anyhow::Error {
    invalid(format!("{} {}", flag_for(refusal.field), refusal.reason))
}

/// Resolve `templates` (by exact name, in order) and then `edits` into a
/// launch, a folder and a title.
///
/// `stored` is the helm's template catalog, `known_hosts` the identities a
/// template's host may name, and `spawn` whether the create is a spawn,
/// which keeps its own host and so refuses a template that sets one.
///
/// `host_given` says the request named a host with `--host`. The flag wins
/// over a template's host (SPEC.md: "an explicit flag wins over a
/// template"), so the templates' host fields are dropped before they are
/// applied: a template pinned to a host that was since reinstalled or
/// removed stays usable with an explicit `--host`, rather than being
/// refused for a field the flag would have replaced anyway.
pub(crate) fn resolve(
    stored: &[LaunchTemplate],
    templates: &[String],
    edits: &TemplateFields,
    known_hosts: &[String],
    spawn: bool,
    host_given: bool,
) -> anyhow::Result<Resolution> {
    let named: Vec<LaunchTemplate> = templates
        .iter()
        .map(|name| {
            stored
                .iter()
                .find(|template| &template.name == name)
                .map(|template| {
                    let mut template = template.clone();
                    if host_given && !spawn {
                        template.fields.host = None;
                    }
                    template
                })
                .ok_or_else(|| {
                    anyhow::Error::new(crate::SupervisorError {
                        origin: crate::client::ErrorOrigin::Helm,
                        kind: ErrorKind::NotFound,
                        message: format!(
                            "no template is named {name:?}; farhelm agent templates lists them"
                        ),
                    })
                })
        })
        .collect::<anyhow::Result<_>>()?;
    let catalog = catalog();
    let context = TemplateContext {
        catalog: &catalog,
        known_hosts,
        host_fixed: spawn,
    };
    let state =
        apply_templates(&LauncherState::default(), &named, &context).map_err(|refusal| {
            if spawn && refusal.field == "host" {
                invalid(format!(
                    "template {:?} sets a host, and farhelm spawn always creates on its own host; \
                 use farhelm agent create --host to create elsewhere",
                    refusal.template
                ))
            } else {
                invalid(refusal.to_string())
            }
        })?;
    let flags = LaunchTemplate {
        name: String::new(),
        fields: edits.clone(),
    };
    let state = apply_template(&state, &flags, &context).map_err(flag_refusal)?;

    let cwd = match state.destination {
        Some(TemplateDestination::Folder(folder)) if !folder.is_empty() => folder,
        Some(TemplateDestination::Github(repo)) => {
            return Err(invalid(format!(
                "a template sets a managed checkout of {repo} as the destination, and the CLI \
                 does not create checkouts; pass --cwd with an existing folder"
            )));
        }
        _ => {
            return Err(invalid(
                "--cwd is required unless a template sets the folder".to_string(),
            ));
        }
    };
    // The launch is validated exactly as the REST create validates the same
    // request (`sessions::resolve_launch_request`); only the refusals for a
    // field nothing set are this path's own, because only here can a
    // template have been expected to set it.
    let request = match state.kind.unwrap_or(LauncherKind::Agent) {
        LauncherKind::Agent => {
            let Some(harness) = state.harness else {
                return Err(invalid(
                    "--agent is required for an agent launch unless a template sets the agent \
                     type; pass --command for a command launch"
                        .to_string(),
                ));
            };
            LaunchRequest::Agent {
                selection: LaunchSelection {
                    harness,
                    model: state.model,
                    effort: state.effort,
                    permissions: state.permissions,
                    workspace_trust: state.workspace_trust,
                },
            }
        }
        LauncherKind::Command => {
            if state.command.is_empty() {
                return Err(invalid(
                    "a command launch needs --command unless a template sets the command"
                        .to_string(),
                ));
            }
            let Some(yolo) = state.yolo else {
                return Err(invalid(
                    "a command launch needs --yolo or --no-yolo unless a template sets its YOLO \
                     answer"
                        .to_string(),
                ));
            };
            LaunchRequest::Command(CommandLaunch {
                command: state.command,
                yolo,
                agent: state.command_agent,
                resume: state.resume_command,
            })
        }
    };
    let launch = crate::sessions::resolve_launch_request(request)?;
    Ok(Resolution {
        cwd,
        launch,
        title: state.name,
        template_host: state.host,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::{LaunchEffort, LaunchHarness};

    fn template(name: &str, fields: TemplateFields) -> LaunchTemplate {
        LaunchTemplate {
            name: name.to_string(),
            fields,
        }
    }

    fn folder(path: &str) -> Option<TemplateDestination> {
        Some(TemplateDestination::Folder(path.to_string()))
    }

    fn message(error: anyhow::Error) -> String {
        format!("{error:#}")
    }

    /// Spec: templates apply first, in order, and the flags then act as
    /// further edits, so `--template my-codex --model gpt-6-luna` is
    /// `my-codex` with a different model; a template's folder and name
    /// satisfy `--cwd` and `--title`, and an explicit flag wins over them.
    ///
    /// Why: this is SPEC.md's definition of how the CLI combines templates
    /// and flags, and the reason required flags may be omitted at all.
    #[farhelm_testtrace::test]
    fn flags_edit_the_templates_result_and_win_over_it() {
        let stored = [template(
            "my-codex",
            TemplateFields {
                agent: Some(LaunchHarness::Codex),
                model: Some(Some("gpt-6.1-sol".to_string())),
                effort: Some(Some(LaunchEffort::High)),
                destination: folder("/srv/a"),
                name: Some("from-template".to_string()),
                ..Default::default()
            },
        )];
        let resolved = resolve(
            &stored,
            &["my-codex".to_string()],
            &TemplateFields {
                model: Some(Some("gpt-6-luna".to_string())),
                ..Default::default()
            },
            &[],
            false,
            false,
        )
        .unwrap();
        assert_eq!(resolved.cwd, "/srv/a");
        assert_eq!(resolved.title.as_deref(), Some("from-template"));
        let SessionLaunch::Agent { selection, .. } = &resolved.launch else {
            panic!("an agent launch: {:?}", resolved.launch);
        };
        assert_eq!(selection.harness, LaunchHarness::Codex);
        assert_eq!(selection.model.as_deref(), Some("gpt-6-luna"));
        assert_eq!(selection.effort, Some(LaunchEffort::High));

        let resolved = resolve(
            &stored,
            &["my-codex".to_string()],
            &TemplateFields {
                destination: folder("/srv/b"),
                name: Some("explicit".to_string()),
                ..Default::default()
            },
            &[],
            false,
            false,
        )
        .unwrap();
        assert_eq!(resolved.cwd, "/srv/b", "an explicit --cwd wins");
        assert_eq!(resolved.title.as_deref(), Some("explicit"));
    }

    /// Spec: with no template, `--command` and its YOLO answer make a
    /// command launch with the declared agent type and resume command, and
    /// a missing folder, agent type or YOLO answer is refused naming the
    /// flag, with no remembered default filling it.
    ///
    /// Why: an agent's create must say everything the launch needs; a gap
    /// filled from the user's GUI defaults would launch something the agent
    /// never chose, which SPEC.md rules out for the CLI.
    #[farhelm_testtrace::test]
    fn a_launch_without_its_required_fields_is_refused_naming_the_flag() {
        let resolved = resolve(
            &[],
            &[],
            &TemplateFields {
                kind: Some(LauncherKind::Command),
                agent: Some(LaunchHarness::Claude),
                command: Some("claude {farhelm_args}".to_string()),
                yolo: Some(false),
                resume_command: Some(Some(
                    "claude --resume {conversation} {farhelm_args}".to_string(),
                )),
                destination: folder("/w"),
                ..Default::default()
            },
            &[],
            false,
            false,
        )
        .unwrap();
        assert_eq!(
            resolved.launch,
            SessionLaunch::Command(CommandLaunch {
                command: "claude {farhelm_args}".to_string(),
                yolo: false,
                agent: Some(LaunchHarness::Claude),
                resume: Some("claude --resume {conversation} {farhelm_args}".to_string()),
            })
        );

        let no_folder = TemplateFields {
            agent: Some(LaunchHarness::Claude),
            ..Default::default()
        };
        assert!(
            message(resolve(&[], &[], &no_folder, &[], false, false).unwrap_err())
                .contains("--cwd")
        );
        let no_agent = TemplateFields {
            destination: folder("/w"),
            ..Default::default()
        };
        assert!(
            message(resolve(&[], &[], &no_agent, &[], false, false).unwrap_err())
                .contains("--agent")
        );
        let no_yolo = TemplateFields {
            kind: Some(LauncherKind::Command),
            command: Some("sh".to_string()),
            destination: folder("/w"),
            ..Default::default()
        };
        assert!(
            message(resolve(&[], &[], &no_yolo, &[], false, false).unwrap_err())
                .contains("--no-yolo")
        );
    }

    /// Spec: an unknown template name is refused naming it; a template that
    /// does not apply is refused naming the template and the field; a flag
    /// that does not apply is refused naming the flag; a template whose
    /// destination is a fresh GitHub checkout is refused, as is a template
    /// that sets a host on a spawn.
    ///
    /// Why: these are SPEC.md's CLI refusals. Each names what to change,
    /// because the caller is an agent that can only act on the message.
    #[farhelm_testtrace::test]
    fn refusals_name_the_template_or_flag_at_fault() {
        let stored = [
            template(
                "runner",
                TemplateFields {
                    kind: Some(LauncherKind::Command),
                    command: Some("sh".to_string()),
                    yolo: Some(false),
                    ..Default::default()
                },
            ),
            template(
                "checkout",
                TemplateFields {
                    agent: Some(LaunchHarness::Claude),
                    destination: Some(TemplateDestination::Github("o/r".to_string())),
                    ..Default::default()
                },
            ),
            template(
                "elsewhere",
                TemplateFields {
                    host: Some("install-2".to_string()),
                    ..Default::default()
                },
            ),
        ];
        let unknown = resolve(
            &stored,
            &["nope".to_string()],
            &TemplateFields::default(),
            &[],
            false,
            false,
        );
        assert!(message(unknown.unwrap_err()).contains("no template is named \"nope\""));

        let flag = resolve(
            &stored,
            &["runner".to_string()],
            &TemplateFields {
                model: Some(Some("x".to_string())),
                destination: folder("/w"),
                ..Default::default()
            },
            &[],
            false,
            false,
        );
        let flag = message(flag.unwrap_err());
        assert!(
            flag.starts_with("--model applies to an agent launch"),
            "{flag}"
        );

        let checkout = resolve(
            &stored,
            &["checkout".to_string()],
            &TemplateFields::default(),
            &[],
            false,
            false,
        );
        assert!(message(checkout.unwrap_err()).contains("does not create checkouts"));
        let overridden = resolve(
            &stored,
            &["checkout".to_string()],
            &TemplateFields {
                destination: folder("/w"),
                ..Default::default()
            },
            &[],
            false,
            false,
        );
        assert!(
            overridden.is_ok(),
            "an explicit --cwd replaces the checkout"
        );

        let hosts = ["install-2".to_string()];
        let spawned = resolve(
            &stored,
            &["elsewhere".to_string()],
            &TemplateFields::default(),
            &hosts,
            true,
            false,
        );
        assert!(
            message(spawned.unwrap_err()).contains("farhelm spawn always creates on its own host")
        );
        let created = resolve(
            &stored,
            &["elsewhere".to_string(), "runner".to_string()],
            &TemplateFields {
                destination: folder("/w"),
                ..Default::default()
            },
            &hosts,
            false,
            false,
        )
        .unwrap();
        assert_eq!(created.template_host.as_deref(), Some("install-2"));
    }
}

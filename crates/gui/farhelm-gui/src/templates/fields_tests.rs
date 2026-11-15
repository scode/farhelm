//! Tests of the template editor's model, without a window: which fields a switch offers, adding and removing them,
//! why a draft refuses saving, the name rules, and Duplicate's suggested name.
//!
//! The oracle is SPEC.md ("Launch templates") and the old editor's model tests in `list/templates.rs` at reference
//! commit `24bc0580`. The editor saves its whole draft over the stored template (last write wins), so a field that
//! silently changes meaning between loading and saving would be written back wrong; these rules are pinned here rather
//! than only through the view.

use farhelm_proto::launcher::{
    EFFORT_ORDER, LaunchCatalogModel, LaunchTemplate, LauncherKind, TemplateDestination,
    TemplateFields, TemplateNameKind,
};
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission};

use super::fields::{
    Draft, Field, duplicate_name, effort_options, name_refusal, permission_options, taken_refusal,
};

fn agent_fields(agent: Option<LaunchHarness>) -> TemplateFields {
    TemplateFields {
        kind: Some(LauncherKind::Agent),
        agent,
        ..TemplateFields::default()
    }
}

/// Pi is the fixture for an agent type that takes no initial prompt and offers only YOLO, so no Approvals field. Tests
/// that lean on that call this first, so a change to Pi's capabilities fails here as a stale premise rather than as a
/// behavior regression.
fn assert_pi_premise() {
    assert!(
        !LaunchHarness::Pi.supports_initial_prompt(false) && LaunchHarness::Pi.offers_only_yolo(),
        "premise: Pi takes no initial prompt and offers only YOLO"
    );
}

fn draft(fields: TemplateFields) -> Draft {
    Draft {
        name: "t".into(),
        fields,
        wrapper_input: None,
    }
}

/// Presence is what a template applies, whatever the value: a reset, an empty text and a false YOLO answer are all
/// set fields, and adding one never chooses another field on the user's behalf (a model adds no agent type).
#[test]
fn presence_decides_and_adding_chooses_nothing_else() {
    let mut fields = agent_fields(None);
    for field in [
        Field::Model,
        Field::Effort,
        Field::Permissions,
        Field::Wrapper,
    ] {
        assert!(!field.is_set(&fields));
        field.add(&mut fields);
        assert!(field.is_set(&fields), "{field:?} is set once added");
    }
    assert_eq!(fields.model, Some(None), "a model starts as the reset");
    assert_eq!(fields.agent, None, "adding a model chose no agent type");
    let mut command = TemplateFields {
        kind: Some(LauncherKind::Command),
        ..TemplateFields::default()
    };
    Field::Yolo.add(&mut command);
    assert_eq!(command.yolo, Some(false));
    assert!(
        Field::Yolo.is_set(&command),
        "a false answer is a set field"
    );
}

/// A session name owns its kind: adding it starts on base (no stored kind), and removing it takes a fixed kind with
/// it, so adding it back cannot revive a kind the user no longer sees (SPEC.md: "Adding the name starts on base;
/// removing it removes its kind").
#[test]
fn a_session_name_owns_its_kind() {
    let mut fields = TemplateFields {
        name: Some("review".into()),
        name_kind: Some(TemplateNameKind::Fixed),
        ..TemplateFields::default()
    };
    Field::Name.remove(&mut fields);
    assert_eq!((fields.name.as_deref(), fields.name_kind), (None, None));
    Field::Name.add(&mut fields);
    assert_eq!((fields.name.as_deref(), fields.name_kind), (Some(""), None));
}

/// Removing Approvals takes the custom words that belong to them, so a later Custom choice cannot pick up stale words.
#[test]
fn removing_approvals_takes_their_custom_words() {
    let mut fields = TemplateFields {
        permissions: Some(Some(LaunchPermission::Custom)),
        permission_flags: Some(vec!["--sandbox".into()]),
        ..agent_fields(Some(LaunchHarness::Codex))
    };
    Field::Permissions.remove(&mut fields);
    assert_eq!((fields.permissions, fields.permission_flags), (None, None));
}

/// SPEC.md: "don't switch offers only host, destination and session name"; Command offers the command's fields; Agent
/// offers the agent's, narrowed by the agent type's capabilities (Pi takes no initial prompt and offers only YOLO, so
/// no Approvals), and every agent-launch choice with no agent type. A retired container field is never offered.
#[test]
fn the_switch_and_agent_decide_what_is_offered() {
    let offered = |fields: &TemplateFields| -> Vec<Field> {
        Field::ALL
            .into_iter()
            .filter(|field| field.offered(fields))
            .collect()
    };
    assert_eq!(
        offered(&TemplateFields::default()),
        vec![Field::Host, Field::Destination, Field::Name]
    );
    assert_eq!(
        offered(&TemplateFields {
            kind: Some(LauncherKind::Command),
            ..TemplateFields::default()
        }),
        vec![
            Field::Command,
            Field::Yolo,
            Field::Agent,
            Field::Resume,
            Field::Host,
            Field::Destination,
            Field::Name
        ]
    );
    let any_agent = offered(&agent_fields(None));
    for field in [
        Field::Model,
        Field::Effort,
        Field::Permissions,
        Field::Wrapper,
        Field::Prompt,
    ] {
        assert!(any_agent.contains(&field), "{field:?} with no agent type");
    }
    assert_pi_premise();
    let pi = offered(&agent_fields(Some(LaunchHarness::Pi)));
    assert!(
        !pi.contains(&Field::Prompt) && !pi.contains(&Field::Permissions),
        "{pi:?}"
    );
    assert!(!any_agent.contains(&Field::Container));
}

/// An incompatible field stays visible and refuses saving until corrected or removed (SPEC.md), naming the field, while
/// a model of any text fits an agent launch: the editor never refuses a model the catalog does not list.
#[test]
fn incompatible_fields_refuse_saving_but_any_model_text_fits() {
    let switched = draft(TemplateFields {
        kind: Some(LauncherKind::Command),
        command: Some("make".into()),
        effort: Some(Some(LaunchEffort::High)),
        ..TemplateFields::default()
    });
    assert_eq!(
        switched.refusal(None).as_deref(),
        Some("Effort: does not fit this launcher switch or agent type; remove it to save")
    );
    let custom_model = draft(TemplateFields {
        model: Some(Some("not-in-any-catalog".into())),
        ..agent_fields(Some(LaunchHarness::Claude))
    });
    assert_eq!(custom_model.refusal(Some(&[])), None);
    assert!(
        !LaunchHarness::Grok.offers_model(),
        "premise: Grok has no model choice"
    );
    let grok_model = draft(TemplateFields {
        model: Some(Some("grok-anything".into())),
        ..agent_fields(Some(LaunchHarness::Grok))
    });
    assert!(
        !Field::Model.offered(&grok_model.fields),
        "not offered for adding"
    );
    assert_eq!(
        grok_model.refusal(None),
        None,
        "but a set model fits any agent launch; application judges it"
    );
    assert_pi_premise();
    let pi_yolo = draft(TemplateFields {
        permissions: Some(Some(LaunchPermission::Guarded)),
        ..agent_fields(Some(LaunchHarness::Pi))
    });
    assert!(
        pi_yolo
            .refusal(None)
            .is_some_and(|reason| reason.starts_with("Approvals: ")),
        "Pi offers no Approvals field at all"
    );
}

/// An explicit permission the agent offers stays saveable even when it is that agent's default, and with no agent type
/// every permission is offered, since another template may supply the agent.
#[test]
fn explicit_permissions_stay_saveable_and_partial_templates_offer_all() {
    assert!(
        LaunchHarness::Codex.offers_permission(LaunchPermission::Guarded),
        "premise: Codex offers Guarded"
    );
    let codex = permission_options(Some(LaunchHarness::Codex));
    assert!(
        codex
            .iter()
            .any(|(value, _)| *value == Some(LaunchPermission::Guarded))
    );
    assert_eq!(codex[0].0, None, "the reset comes first");
    assert!(codex[0].1.ends_with("(reset)"), "{:?}", codex[0]);
    let any = permission_options(None);
    assert_eq!(any.len(), LaunchPermission::ALL.len() + 1);
    let explicit = draft(TemplateFields {
        permissions: Some(Some(LaunchPermission::Guarded)),
        ..agent_fields(Some(LaunchHarness::Codex))
    });
    assert_eq!(explicit.refusal(None), None);
}

/// Effort choices follow the agent type and model in a catalog that was read; before one is read, or with no agent
/// type, every effort is offered, so nothing is refused for lack of a catalog.
#[test]
fn effort_choices_follow_a_read_catalog_only() {
    let catalog = vec![LaunchCatalogModel {
        id: "gpt-6-luna".into(),
        harness: LaunchHarness::Codex,
        efforts: vec![LaunchEffort::Low],
    }];
    let fields = TemplateFields {
        model: Some(Some("gpt-6-luna".into())),
        effort: Some(Some(LaunchEffort::High)),
        ..agent_fields(Some(LaunchHarness::Codex))
    };
    let read = effort_options(&fields, Some(&catalog));
    assert_eq!(
        read.iter().map(|(value, _)| *value).collect::<Vec<_>>(),
        vec![None, Some(LaunchEffort::Low)]
    );
    assert_eq!(effort_options(&fields, None).len(), 1 + EFFORT_ORDER.len());
    assert!(
        draft(fields.clone()).refusal(Some(&catalog)).is_some_and(
            |reason| reason.starts_with("Effort: this agent type or model does not offer")
        )
    );
    assert_eq!(
        draft(fields).refusal(None),
        None,
        "no catalog, no catalog-based refusal"
    );
}

/// A present text left empty refuses saving with the old wording, and a wrapper prefix that does not parse refuses
/// with the parser's own reason; neither is silently dropped from the saved template.
#[test]
fn unfinished_text_refuses_saving() {
    let empty_command = draft(TemplateFields {
        kind: Some(LauncherKind::Command),
        command: Some(String::new()),
        ..TemplateFields::default()
    });
    assert_eq!(
        empty_command.refusal(None).as_deref(),
        Some("Type the command, or remove its field")
    );
    let empty_host = draft(TemplateFields {
        host: Some(String::new()),
        ..TemplateFields::default()
    });
    assert_eq!(
        empty_host.refusal(None).as_deref(),
        Some("Choose the host, or remove its field")
    );
    let mut quote = draft(agent_fields(Some(LaunchHarness::Codex)));
    quote.fields.wrapper_prefix = Some(None);
    quote.wrapper_input = Some("env 'open".into());
    let reason = quote.refusal(None).unwrap();
    assert!(
        reason.chars().next().is_some_and(char::is_uppercase),
        "{reason}"
    );
}

/// A template from before the switch existed opens with the switch its fields imply, in the draft only, and an older
/// template's workspace-trust field is dropped; either makes the draft differ from the stored template, so saving
/// stores the switch and drops the trust (SPEC.md).
#[test]
fn opening_infers_the_switch_and_drops_trust_in_the_draft() {
    let stored = LaunchTemplate {
        name: "legacy".into(),
        fields: TemplateFields {
            command: Some("make dev".into()),
            workspace_trust: Some(Some(true)),
            ..TemplateFields::default()
        },
    };
    let opened = Draft::from_template(&stored);
    assert_eq!(opened.fields.kind, Some(LauncherKind::Command));
    assert_eq!(opened.fields.workspace_trust, None);
    assert_ne!(
        opened.template(),
        stored,
        "the draft is unsaved against the stored template"
    );
    let placement_only = LaunchTemplate {
        name: "place".into(),
        fields: TemplateFields {
            destination: Some(TemplateDestination::Folder("/src".into())),
            ..TemplateFields::default()
        },
    };
    assert_eq!(
        Draft::from_template(&placement_only).fields.kind,
        None,
        "placement alone implies no switch"
    );
}

/// Text the single-line inputs cannot show or keep makes the field read-only, shown escaped; an initial prompt keeps
/// newline and tab editable, while invisible formatting makes it read-only too.
#[test]
fn unsafe_text_makes_a_field_read_only() {
    let fields = TemplateFields {
        name: Some("a\u{200B}b".into()),
        prompt: Some("line one\n\tline two".into()),
        model: Some(Some("m\u{7}".into())),
        ..agent_fields(None)
    };
    assert!(Field::Name.read_only(&fields));
    assert!(
        !Field::Prompt.read_only(&fields),
        "newline and tab stay editable"
    );
    assert!(Field::Model.read_only(&fields));
    let formatted = TemplateFields {
        prompt: Some("hidden\u{202E}text".into()),
        ..fields
    };
    assert!(Field::Prompt.read_only(&formatted));
}

/// A retired containerized field is shown so it can be removed, refuses saving with the retired reason, and is never
/// offered for adding.
#[test]
fn a_retired_container_field_refuses_until_removed() {
    let fields = TemplateFields {
        container: Some(None),
        ..agent_fields(Some(LaunchHarness::Claude))
    };
    assert!(Field::Container.is_set(&fields));
    assert!(!Field::Container.offered(&fields));
    assert_eq!(
        draft(fields).refusal(None),
        Some(format!(
            "Containerized: {}",
            farhelm_proto::RETIRED_CONTAINER_REASON
        ))
    );
}

/// Saving under the template's own name never checks the name (a stored spaced name keeps saving); a new or renamed
/// name refuses whitespace and a name another template has, and cannot be proven free while the helm could not read
/// its templates.
#[test]
fn new_and_renamed_names_must_be_free_and_unspaced() {
    let names = ["my codex", "review"];
    assert_eq!(
        name_refusal(Some("my codex"), "my codex", Some(&names)),
        None
    );
    assert!(
        name_refusal(Some("review"), "my codex", Some(&names))
            .is_some_and(|reason| reason.contains("whitespace"))
    );
    assert_eq!(
        name_refusal(None, "review", Some(&names)).as_deref(),
        Some("A template named \"review\" already exists; edit that one, or choose another name")
    );
    assert_eq!(name_refusal(None, "fresh", Some(&names)), None);
    assert!(
        name_refusal(None, "fresh", None)
            .is_some_and(|reason| reason.contains("could not be read"))
    );
}

/// Duplicate suggests `<name>-copy`, then `-copy-2` and so on until free, with whitespace replaced by `-`, cut to fit the
/// helm's 128-byte name limit at a character boundary.
#[test]
fn duplicate_names_are_free_unspaced_and_fit_the_limit() {
    assert_eq!(duplicate_name("review", &["review"]), "review-copy");
    assert_eq!(
        duplicate_name("review", &["review", "review-copy"]),
        "review-copy-2"
    );
    assert_eq!(duplicate_name("my codex", &[]), "my-codex-copy");
    let long = "é".repeat(80);
    let copy = duplicate_name(&long, &[]);
    assert!(
        copy.len() <= farhelm_proto::launcher::TEMPLATE_NAME_CAP,
        "{}",
        copy.len()
    );
    assert!(copy.ends_with("-copy"));
}

/// Approvals are narrowed to what the agent type offers: changing the agent keeps the stored choice, visible, and
/// refuses saving until it is reset or removed, so one agent's approval vocabulary never leaks into another agent's
/// template (SPEC.md: "Incompatible fields stay visible and prevent saving until corrected or removed"). Ported from
/// the old `agent_changes_flag_approvals_but_keep_arbitrary_models`.
#[test]
fn changing_the_agent_flags_approvals_it_does_not_offer() {
    assert!(
        LaunchHarness::Goose.offers_permission(LaunchPermission::SmartApprove)
            && !LaunchHarness::Claude.offers_permission(LaunchPermission::SmartApprove),
        "premise: Goose offers SmartApprove and Claude does not"
    );
    let mut goose = draft(TemplateFields {
        permissions: Some(Some(LaunchPermission::SmartApprove)),
        ..agent_fields(Some(LaunchHarness::Goose))
    });
    assert_eq!(goose.refusal(None), None);
    goose.fields.agent = Some(LaunchHarness::Claude);
    assert_eq!(
        goose.refusal(None).as_deref(),
        Some("Approvals: this agent type does not offer these approvals; change or remove them")
    );
    assert_eq!(
        goose.fields.permissions,
        Some(Some(LaunchPermission::SmartApprove)),
        "the stored choice stays until the user changes it"
    );
    goose.fields.permissions = Some(None);
    assert_eq!(goose.refusal(None), None, "the reset fits every agent");
    assert_eq!(
        permission_options(Some(LaunchHarness::Claude)),
        vec![
            (None, "Guarded · Claude auto mode (reset)".to_owned()),
            (
                Some(LaunchPermission::Guarded),
                "Guarded · Claude auto mode".to_owned()
            ),
            (Some(LaunchPermission::Yolo), "YOLO".to_owned()),
            (Some(LaunchPermission::Custom), "Custom".to_owned()),
        ]
    );
    let omp: Vec<_> = permission_options(Some(LaunchHarness::Omp))
        .into_iter()
        .map(|(value, _)| value)
        .collect();
    assert!(omp.contains(&Some(LaunchPermission::Approve)));
    assert!(!omp.contains(&Some(LaunchPermission::SmartApprove)));
}

/// Custom words belong to the Custom choice: Custom without words, or words under another choice, refuse saving with
/// the shared rule's reason, the one the helm applies at every admission path.
#[test]
fn custom_words_must_match_their_choice() {
    assert!(
        LaunchHarness::Codex.offers_permission(LaunchPermission::Custom),
        "premise: Codex offers Custom"
    );
    let custom_without_words = draft(TemplateFields {
        permissions: Some(Some(LaunchPermission::Custom)),
        ..agent_fields(Some(LaunchHarness::Codex))
    });
    assert_eq!(
        custom_without_words.refusal(None).as_deref(),
        Some("Custom permissions need permission flags")
    );
    let stray_words = draft(TemplateFields {
        permissions: Some(Some(LaunchPermission::Guarded)),
        permission_flags: Some(vec!["--sandbox".into()]),
        ..agent_fields(Some(LaunchHarness::Codex))
    });
    assert_eq!(
        stray_words.refusal(None).as_deref(),
        Some("Permission flags are only allowed with custom permissions")
    );
    let custom = draft(TemplateFields {
        permissions: Some(Some(LaunchPermission::Custom)),
        permission_flags: Some(vec!["--sandbox".into()]),
        ..agent_fields(Some(LaunchHarness::Codex))
    });
    assert_eq!(custom.refusal(None), None);
}

/// A wrapper text that parses but has not reached the fields yet refuses with its own wording rather than saving the
/// stale word list.
#[test]
fn a_wrapper_text_not_yet_written_refuses() {
    let mut pending = draft(agent_fields(Some(LaunchHarness::Codex)));
    pending.fields.wrapper_prefix = Some(None);
    pending.wrapper_input = Some("env A=1".into());
    assert_eq!(
        pending.refusal(None).as_deref(),
        Some("Wrapper prefix changed while saving; review it and try again")
    );
}

/// Every single-line text field shares one read-only boundary, so a model, folder, repository, command or session name
/// cannot hide characters the inputs would show wrongly or lose; a line break counts in all of them. Ordinary Unicode,
/// the literal escape text, an empty value, a reset and an absent field stay editable. Ported from the old
/// `unsafe_editor_values_require_read_only_escaped_display`, without its escaped strings, which the view draws.
#[test]
fn single_line_fields_share_the_read_only_boundary() {
    let single_line = [
        Field::Name,
        Field::Model,
        Field::Destination,
        Field::Command,
        Field::Resume,
        Field::Wrapper,
    ];
    let with = |raw: &str, destination: fn(String) -> TemplateDestination| TemplateFields {
        name: Some(raw.into()),
        model: Some(Some(raw.into())),
        destination: Some(destination(raw.into())),
        command: Some(raw.into()),
        resume_command: Some(Some(raw.into())),
        wrapper_prefix: Some(Some(vec![raw.into()])),
        ..TemplateFields::default()
    };
    for raw in [
        "model\u{200b}",
        "/tmp/\u{202e}project",
        "first\r\nsecond",
        "a\nb",
    ] {
        for destination in [TemplateDestination::Folder, TemplateDestination::Github] {
            let fields = with(raw, destination);
            for field in single_line {
                assert!(field.read_only(&fields), "{field:?} with {raw:?}");
            }
        }
    }
    for raw in ["", "ordinary model", "/tmp/项目", "literal<U+200B>"] {
        let fields = with(raw, TemplateDestination::Folder);
        for field in single_line {
            assert!(!field.read_only(&fields), "{field:?} with {raw:?}");
        }
    }
    let reset = TemplateFields {
        model: Some(None),
        resume_command: Some(None),
        wrapper_prefix: Some(None),
        ..TemplateFields::default()
    };
    for field in single_line {
        assert!(
            !field.read_only(&TemplateFields::default()),
            "{field:?} absent"
        );
        assert!(!field.read_only(&reset), "{field:?} reset");
    }
}

/// The Host field covers an older environment placement too, so a template carrying only a recipe shows a Host field
/// the user can remove, and removing it takes the recipe and target with it rather than leaving a placement the editor
/// no longer shows.
#[test]
fn the_host_field_covers_and_removes_a_placement() {
    let mut recipe_only = TemplateFields {
        recipe: Some("rust".into()),
        ..TemplateFields::default()
    };
    assert!(Field::Host.is_set(&recipe_only));
    Field::Host.remove(&mut recipe_only);
    assert_eq!(
        (recipe_only.host, recipe_only.recipe, recipe_only.target),
        (None, None, None)
    );
    let mut placed = TemplateFields {
        host: Some("build".into()),
        target: Some(farhelm_proto::LaunchTarget::Environment {
            recipe: "rust".into(),
            place: farhelm_proto::Place::TensorlakeAccount,
        }),
        ..TemplateFields::default()
    };
    Field::Host.remove(&mut placed);
    assert_eq!((placed.host, placed.target), (None, None));
}

/// A retired containerized field refuses saving only while present: removing it, the one thing the editor offers, makes
/// the draft saveable again.
#[test]
fn removing_a_retired_container_field_makes_the_draft_saveable() {
    let mut retired = draft(TemplateFields {
        container: Some(None),
        ..agent_fields(Some(LaunchHarness::Claude))
    });
    assert!(
        retired.refusal(None).is_some(),
        "premise: it refuses while present"
    );
    Field::Container.remove(&mut retired.fields);
    assert_eq!(retired.refusal(None), None);
}

/// A new template starts on the Agent switch with nothing set and no name, and that empty set of fields is saveable:
/// a template need not form a complete launch, and the name rules run separately at the save.
#[test]
fn a_new_template_starts_on_agent_with_nothing_set() {
    let new = Draft::default();
    assert_eq!(new.name, "");
    assert_eq!(new.fields.kind, Some(LauncherKind::Agent));
    assert_eq!(
        new.fields,
        TemplateFields {
            kind: Some(LauncherKind::Agent),
            ..TemplateFields::default()
        },
        "nothing else is set, a session-name kind included"
    );
    assert_eq!(new.refusal(None), None);
}

/// Opening a template from before the switch existed infers the switch from its fields, in the draft only, so merely
/// looking at a template never rewrites it; the draft reads as unsaved exactly when a switch was inferred. An agent type
/// implies Agent, a YOLO answer or a resume command implies Command, and a name alone implies no switch. Ported from the
/// old `legacy_kind_is_inferred_without_changing_the_stored_template`.
#[test]
fn opening_infers_the_switch_from_each_field_kind() {
    for (fields, kind) in [
        (
            TemplateFields {
                agent: Some(LaunchHarness::Claude),
                ..TemplateFields::default()
            },
            Some(LauncherKind::Agent),
        ),
        (
            TemplateFields {
                yolo: Some(false),
                ..TemplateFields::default()
            },
            Some(LauncherKind::Command),
        ),
        (
            TemplateFields {
                resume_command: Some(None),
                ..TemplateFields::default()
            },
            Some(LauncherKind::Command),
        ),
        (
            TemplateFields {
                name: Some("review".into()),
                ..TemplateFields::default()
            },
            None,
        ),
    ] {
        let stored = LaunchTemplate {
            name: "legacy".into(),
            fields: fields.clone(),
        };
        let opened = Draft::from_template(&stored);
        assert_eq!(stored.fields, fields, "opening is not a write");
        assert_eq!(
            opened.fields,
            TemplateFields {
                kind,
                ..fields.clone()
            }
        );
        assert_eq!(opened.template() != stored, kind.is_some(), "{fields:?}");
    }
}

/// Saving the open template under its own name is never refused, not even while the template list could not be read:
/// the save replaces the template the user is editing, which is the point (last write wins). A rename onto another
/// template's name is refused like a new name.
#[test]
fn the_open_name_always_saves_and_a_rename_must_be_free() {
    let names = ["build", "review"];
    for listed in [Some(&names[..]), Some(&[][..]), None] {
        assert_eq!(
            name_refusal(Some("review"), "review", listed),
            None,
            "{listed:?}"
        );
    }
    assert_eq!(
        name_refusal(Some("build"), "review", Some(&names)).as_deref(),
        Some("A template named \"review\" already exists; edit that one, or choose another name")
    );
}

/// Names from before the whitespace rule keep their delete Undo: Undo restores the loaded copy under its old name and
/// checks only that no other template has taken it, never the new-name rule (SPEC.md: existing names with whitespace
/// "retain delete undo"). Ported from the old `historical_spaced_names_keep_save_and_delete_undo`; if the new-name rule
/// crept into this check, deleting an old template would silently lose its restoration.
#[test]
fn spaced_historical_names_keep_their_undo() {
    for name in ["old template", "old\u{a0}template"] {
        assert!(
            farhelm_proto::launcher::check_new_template_name(name).is_err(),
            "premise: {name:?} is refused as a new name"
        );
        assert_eq!(taken_refusal(name, Some(&[])), None);
        assert_eq!(
            name_refusal(Some(name), name, None),
            None,
            "and keeps saving"
        );
        assert!(taken_refusal(name, Some(&[name])).is_some(), "taken again");
        assert!(taken_refusal(name, None).is_some(), "unproven while unread");
    }
}

/// Duplicate's whitespace mapping covers Unicode whitespace too, so a copy of an old spaced name is always a valid new
/// name: the new-name rule it must pass refuses "any whitespace in its name, including Unicode whitespace" (SPEC.md).
#[test]
fn duplicate_replaces_unicode_whitespace() {
    let copy = duplicate_name("my\u{a0}project name", &[]);
    assert_eq!(copy, "my-project-name-copy");
    assert!(farhelm_proto::launcher::check_new_template_name(&copy).is_ok());
}

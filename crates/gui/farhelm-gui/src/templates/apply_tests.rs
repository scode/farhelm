//! Tests of applying launch templates to a launcher draft, without a window.
//!
//! The oracle is SPEC.md ("Launch templates", "Restart with"), SPEC_impl.md ("Launch templates") and the old app's
//! template tests at reference commit `24bc0580` (`list/create_form.rs` and `list/create_form/draft.rs`), where they
//! tested behavior rather than the old form's wiring. Each test states the rule it pins, because the replay's whole
//! point is that a template behaves exactly like the same edits by hand, and a shortcut taken in the replay would only
//! show up as a subtle difference in some side effect.

use std::collections::BTreeMap;

use farhelm_proto::launcher::{
    LaunchCatalogModel, LaunchTemplate, LauncherKind, TemplateDestination, TemplateFields,
    TemplateNameKind,
};
use farhelm_proto::session_requests::RestartSessionBody;
use farhelm_proto::ui_channel::{HostStateView, HostView, Preferences, SessionRow};
use farhelm_proto::{
    CommandLaunch, LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, LaunchTarget,
    Place, RestartOffer, SessionInfo, SessionLaunch,
};

use super::apply::{Applied, Context, apply, install_identity, snapshot};
use crate::hosts::offers::HostKind;
use crate::launcher::draft::{LaunchDraft, Tab};
use crate::launcher::request::{Account, Preview, Recipes, Target};

// ---------------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------------

/// Two hosts from the helm's pinned host list: 1 with install identity `install-one`, 2 with `install-two`. Both are
/// connected; the tests that need a mismatched or session-owned row change one.
fn hosts() -> Vec<HostView> {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../farhelm-helm/http-contract/host-list.json"
    ))
    .unwrap();
    let mut one: HostView = serde_json::from_value(fixture["hosts"][0].clone()).unwrap();
    one.id = 1;
    one.identity = Some("install-one".into());
    let mut two = one.clone();
    two.id = 2;
    two.identity = Some("install-two".into());
    // The premise the host tests rely on: both rows offer a usable identity, on a host no environment owns. The
    // placement tests rely on both being container hosts, so each holds a container place.
    assert!(matches!(one.state, HostStateView::Connected { .. }));
    assert!(!one.is_session_owned);
    assert!(HostKind::of(&one).holds_containers());
    vec![one, two]
}

/// A catalog with one Codex model offering two efforts and one Claude model offering one.
fn catalog() -> Vec<LaunchCatalogModel> {
    vec![
        LaunchCatalogModel {
            id: "gpt-6-luna".into(),
            family: "luna".into(),
            latest: true,
            harness: LaunchHarness::Codex,
            efforts: vec![LaunchEffort::Low, LaunchEffort::High],
        },
        LaunchCatalogModel {
            id: "claude-fable-5".into(),
            family: "fable".into(),
            latest: true,
            harness: LaunchHarness::Claude,
            efforts: vec![LaunchEffort::High],
        },
    ]
}

/// The folder New would seed for a host: `/home/<id>`, so a test can tell which host seeded it.
fn default_folder(host: i64) -> String {
    format!("/home/{host}")
}

/// Everything a test applies against. Held in one place so each test can borrow a [`Context`] from it. The recipe
/// catalog lists `web` and `api`; the places a placement may name follow from the hosts (both are the helm's own kind,
/// which can hold containers).
struct Fixture {
    hosts: Vec<HostView>,
    catalog: Vec<LaunchCatalogModel>,
    preferences: Preferences,
    recipes: Vec<String>,
}

impl Fixture {
    fn new() -> Fixture {
        Fixture {
            hosts: hosts(),
            catalog: catalog(),
            preferences: Preferences {
                remembered_wrapper_prefixes: BTreeMap::from([(
                    LaunchHarness::Codex,
                    vec!["env".into(), "FOO='a b'".into()],
                )]),
                ..Preferences::default()
            },
            recipes: vec!["web".into(), "api".into()],
        }
    }

    fn context(&self) -> Context<'_> {
        Context {
            hosts: &self.hosts,
            catalog: Some(&self.catalog),
            remembered: &self.preferences,
            default_folder: &default_folder,
            known_recipes: &self.recipes,
        }
    }

    fn apply(
        &self,
        draft: &mut LaunchDraft,
        templates: &[LaunchTemplate],
    ) -> Result<Applied, String> {
        apply(draft, templates, &self.context())
    }
}

fn template(name: &str, fields: TemplateFields) -> LaunchTemplate {
    LaunchTemplate {
        name: name.into(),
        fields,
    }
}

fn new_draft() -> LaunchDraft {
    LaunchDraft::new(Some(1), "~".into(), None)
}

/// Saving from the launcher and replaying the template must preserve both the registered ID and an effort
/// outside the built-in model catalog. A template never captures the selector for one named start.
#[test]
fn custom_agent_template_replays_the_saved_registration_and_effort() {
    let fixture = Fixture::new();
    let agent = farhelm_proto::custom_agents::CustomAgent {
        id: "acme-code".into(),
        display_name: "Acme Code".into(),
        command: "acme-code".into(),
        supports_resumption: true,
        supports_reporting: false,
    };
    let mut source = new_draft();
    source.choose_custom_agent(&agent, Some(&fixture.catalog), None);
    source.agent.model = Some("deep-v2".into());
    source.agent.custom_effort = "extra-deep".into();
    source.edit_conversation("one-start".into());
    let expected = source.agent_selection().unwrap();
    let candidate = super::save::Candidate::from_draft(&source, &fixture.hosts);
    assert_eq!(candidate.fields.custom_agent, Some(Some(agent.id)));
    let mut draft = new_draft();
    fixture
        .apply(&mut draft, &[template("custom", candidate.fields)])
        .unwrap();
    assert_eq!(draft.agent_selection(), Some(expected));
    assert!(draft.conversation.is_empty());

    fixture
        .apply(
            &mut draft,
            &[template(
                "other",
                TemplateFields {
                    agent: Some(LaunchHarness::Custom),
                    custom_agent: Some(Some("other-code".into())),
                    ..TemplateFields::default()
                },
            )],
        )
        .unwrap();
    let changed = draft.agent_selection().unwrap();
    assert_eq!(changed.custom_agent.as_deref(), Some("other-code"));
    assert_eq!((changed.model, changed.effort), (None, None));
}

/// A template that names a registered custom agent loads that registration's remembered wrapper prefix, replacing the
/// previous agent's the way picking the agent by hand does (`each_custom_agent_loads_its_own_remembered_wrapper` covers
/// the hand pick); a wrapper the template names itself, or its explicit clear, overrides the remembered one.
///
/// Why: templates replay hand edits, and the wrapper memory is now per registration. Loading the shared slot, or
/// nothing, would let one custom agent's wrapper ride along into another's launch.
#[test]
fn a_templates_custom_agent_loads_that_registrations_wrapper() {
    let mut fixture = Fixture::new();
    fixture.preferences.remembered_custom_wrapper_prefixes = BTreeMap::from([
        ("acme".to_owned(), vec!["acme-env".to_owned()]),
        ("beta".to_owned(), vec!["beta-env".to_owned()]),
    ]);
    let pick = |id: &str, wrapper_prefix: Option<Option<Vec<String>>>| {
        template(
            id,
            TemplateFields {
                agent: Some(LaunchHarness::Custom),
                custom_agent: Some(Some(id.into())),
                wrapper_prefix,
                ..TemplateFields::default()
            },
        )
    };
    let mut draft = new_draft();
    fixture.apply(&mut draft, &[pick("acme", None)]).unwrap();
    assert_eq!(draft.agent.wrapper, "acme-env");
    draft.edit_wrapper("typed".into());
    fixture.apply(&mut draft, &[pick("beta", None)]).unwrap();
    assert_eq!(
        draft.agent.wrapper, "beta-env",
        "another registration replaces the typed prefix"
    );
    assert!(!draft.agent.wrapper_explicit);

    fixture
        .apply(
            &mut draft,
            &[pick("acme", Some(Some(vec!["template-wrap".into()])))],
        )
        .unwrap();
    assert_eq!(
        draft.agent.wrapper, "template-wrap",
        "the template's own prefix wins"
    );
    fixture
        .apply(&mut draft, &[pick("beta", Some(None))])
        .unwrap();
    assert_eq!(
        draft.agent.wrapper, "",
        "the template's explicit clear wins"
    );
}

/// A template switch uses the effort the custom field shows. A word Claude can use stays, while a custom-only
/// word clears with the same notice as a hand switch so the desktop and CLI replay the same choice.
#[test]
fn template_switch_to_builtin_reconciles_the_custom_effort() {
    let fixture = Fixture::new();
    for (word, model, catalog_loaded, expected) in [
        (
            "high",
            Some(Some("claude-fable-5")),
            true,
            Some(LaunchEffort::High),
        ),
        ("extra-deep", Some(Some("claude-fable-5")), true, None),
        ("extra-deep", Some(None), true, None),
        ("ultra", None, false, None),
    ] {
        let next = template(
            "Claude",
            TemplateFields {
                agent: Some(LaunchHarness::Claude),
                model: model.map(|id| id.map(str::to_owned)),
                ..TemplateFields::default()
            },
        );
        let mut draft = new_draft();
        draft.choose_harness(LaunchHarness::Custom, None, None);
        draft.agent.custom_agent = Some("acme-code".into());
        draft.agent.custom_effort = word.into();
        let mut context = fixture.context();
        if !catalog_loaded {
            context.catalog = None;
        }
        let applied = apply(&mut draft, &[next], &context).unwrap();
        let selected = draft
            .agent_selection()
            .expect("the template selects Claude");
        assert_eq!(
            (selected.harness, selected.effort),
            (LaunchHarness::Claude, expected.map(Into::into))
        );
        let cleared = applied
            .notice
            .flatten()
            .is_some_and(|notice| notice.contains("selected effort"));
        assert_eq!(cleared, expected.is_none(), "{word}");
    }
}

/// Generic Custom is a deliberate declaration with its own command. Its template clears a previously
/// selected registration and retains the same compulsory YOLO policy.
#[test]
fn generic_custom_command_template_clears_an_old_registration() {
    let fixture = Fixture::new();
    let generic = |draft: &mut LaunchDraft| {
        draft.choose_tab(Tab::Command);
        draft.choose_command_agent(Some(LaunchHarness::Custom));
        draft.edit_command("my-agent {farhelm_args}".into());
    };
    let mut source = new_draft();
    generic(&mut source);
    let candidate = super::save::Candidate::from_draft(&source, &fixture.hosts);
    assert_eq!(candidate.fields.custom_agent, Some(None));
    assert_eq!(
        candidate.fields.yolo, None,
        "a declaration supplies an effective answer only"
    );
    let mut draft = new_draft();
    draft.choose_command_yolo(false);
    generic(&mut draft);
    let captured = super::save::Candidate::from_draft(&draft, &fixture.hosts);
    assert_eq!(captured.fields.yolo, None, "the earlier No is dormant");
    draft.command.custom_agent = Some("old-id".into());
    let non_yolo = template(
        "guarded",
        TemplateFields {
            yolo: Some(false),
            ..TemplateFields::default()
        },
    );
    assert!(fixture.apply(&mut draft, &[non_yolo]).is_err());
    assert_eq!(draft.command.custom_agent.as_deref(), Some("old-id"));
    fixture
        .apply(&mut draft, &[template("generic", candidate.fields)])
        .unwrap();
    assert_eq!(draft.command.custom_agent, None);
    assert_eq!(draft.command.effective_yolo(), Some(true));
    let codex = template(
        "codex",
        TemplateFields {
            kind: Some(LauncherKind::Command),
            agent: Some(LaunchHarness::Codex),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[codex]).unwrap();
    assert_eq!(
        draft.command.effective_yolo(),
        Some(false),
        "the built-in keeps its explicit answer"
    );
}

/// A session as the helm serves it (its pinned list fixture), in `/src/project`, titled `source`, with `launch` as its
/// stored launch. A small copy of the launcher tests' own fixture, which is private to that module.
fn session(launch: SessionLaunch) -> SessionInfo {
    let row: SessionRow =
        serde_json::from_value(crate::test_support::session_entry("s1").value).unwrap();
    let mut info = row.info;
    info.cwd = "/src/project".into();
    info.title = "source".into();
    info.launch = launch;
    info
}

/// A Codex agent session stored with `model` and Guarded, a resume command and a captured conversation, as Restart with
/// opens it.
fn restart_session(model: Option<&str>) -> SessionInfo {
    let selection = LaunchSelection {
        custom_agent: None,
        harness: LaunchHarness::Codex,
        model: model.map(str::to_owned),
        effort: None,
        permissions: Some(LaunchPermission::Guarded),
        permission_flags: None,
        workspace_trust: None,
        wrapper_prefix: None,
        container: None,
    };
    let mut session = session(SessionLaunch::Agent {
        custom_reporting: None,
        selection,
        start: vec!["codex".into(), "{farhelm_args}".into()],
        resume: Some(vec![
            "codex".into(),
            "{conversation}".into(),
            "{farhelm_args}".into(),
        ]),
    });
    session.restart_offer = RestartOffer::Resume;
    session
}

/// [`restart_session`] on `gpt-6-luna`, opened in Restart with.
fn restart_draft() -> LaunchDraft {
    LaunchDraft::restart_with("s1", &restart_session(Some("gpt-6-luna")), Some(1)).unwrap()
}

/// The restart a press of Restart with's main action would send for `draft`, checked against the fixture's hosts and
/// catalog with `live` as the session's stored launch; `None` while the launcher would refuse the press.
fn restart_body(
    fixture: &Fixture,
    draft: &LaunchDraft,
    live: &SessionLaunch,
) -> Option<RestartSessionBody> {
    let target = Target {
        hosts: &fixture.hosts,
        catalog: Some(&fixture.catalog),
        preview: Preview::Waiting,
        recipes: &Recipes::Loading,
        account: &Account::Reading,
    };
    draft.restart_body(target, Some(live), false)
}

// ---------------------------------------------------------------------------------------------------------------------
// Stacking and order
// ---------------------------------------------------------------------------------------------------------------------

/// SPEC.md's own example: applying `my-codex` and then `myproject-webbuilder` applies both, and fields the later one
/// leaves out keep what the earlier one set. This is what makes templates composable, so the batch fold and the
/// replay must both carry every earlier edit forward.
#[test]
fn templates_stack_with_the_later_winning_where_they_overlap() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    let my_codex = template(
        "my-codex",
        TemplateFields {
            agent: Some(LaunchHarness::Codex),
            model: Some(Some("gpt-6-luna".into())),
            effort: Some(Some(LaunchEffort::High.into())),
            name: Some("codex-work".into()),
            ..TemplateFields::default()
        },
    );
    let webbuilder = template(
        "myproject-webbuilder",
        TemplateFields {
            host: Some("install-two".into()),
            destination: Some(TemplateDestination::Folder("/src/webbuilder".into())),
            name: Some("webbuilder".into()),
            name_kind: Some(TemplateNameKind::Fixed),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[my_codex, webbuilder]).unwrap();
    assert_eq!(draft.agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(draft.agent.model.as_deref(), Some("gpt-6-luna"));
    assert_eq!(draft.agent.effort, Some(LaunchEffort::High));
    assert_eq!(draft.host, Some(2));
    assert_eq!(draft.folder.value(), "/src/webbuilder");
    assert_eq!(draft.name.value(), "webbuilder", "the later name wins");
    assert_eq!(
        draft.name_kind,
        Some(TemplateNameKind::Fixed),
        "and so does its kind"
    );
}

/// The launch kind applies before the agent type (SPEC.md: "launch kind first, then agent type"), so a template that
/// switches to Command and names an agent declares that agent for the command instead of choosing it on the Agent tab.
/// The Agent tab's own choices are left alone.
#[test]
fn the_launch_kind_applies_before_the_agent_type() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Claude, Some(&fixture.catalog), None);
    let declared = template(
        "declared",
        TemplateFields {
            kind: Some(LauncherKind::Command),
            agent: Some(LaunchHarness::Codex),
            command: Some("codex {farhelm_args}".into()),
            yolo: Some(false),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[declared]).unwrap();
    assert_eq!(draft.tab, Tab::Command);
    assert_eq!(draft.command.agent, Some(LaunchHarness::Codex));
    assert_eq!(draft.command.command, "codex {farhelm_args}");
    assert_eq!(draft.command.yolo, Some(false));
    assert_eq!(
        draft.agent.harness,
        Some(LaunchHarness::Claude),
        "the Agent tab keeps its draft"
    );
}

/// A template's agent type is a hand pick of that agent: it loads that agent's remembered wrapper prefix, exactly as
/// choosing the agent by hand does (SPEC.md: "Switching agent types loads the new type's remembered prefix").
#[test]
fn a_templates_agent_loads_its_remembered_wrapper_prefix() {
    let fixture = Fixture::new();
    let mut by_hand = new_draft();
    by_hand.choose_harness(
        LaunchHarness::Codex,
        Some(&fixture.catalog),
        fixture
            .preferences
            .remembered_wrapper(LaunchHarness::Codex, None),
    );
    let mut by_template = new_draft();
    let codex = template(
        "codex",
        TemplateFields {
            agent: Some(LaunchHarness::Codex),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut by_template, &[codex]).unwrap();
    assert_eq!(by_template.agent.wrapper, "env FOO='a b'");
    assert_eq!(
        by_template, by_hand,
        "the template is exactly the hand edit"
    );
}

/// The wrapper prefix has three states (SPEC.md): absent leaves the current prefix, including a remembered one;
/// present with words replaces it; present with no prefix clears it.
#[test]
fn a_wrapper_prefix_field_has_three_states() {
    let fixture = Fixture::new();
    let codex = |wrapper_prefix| {
        template(
            "codex",
            TemplateFields {
                agent: Some(LaunchHarness::Codex),
                wrapper_prefix,
                ..TemplateFields::default()
            },
        )
    };

    let mut absent = new_draft();
    fixture.apply(&mut absent, &[codex(None)]).unwrap();
    assert_eq!(
        absent.agent.wrapper, "env FOO='a b'",
        "absent keeps the remembered prefix"
    );

    let mut replaced = new_draft();
    fixture
        .apply(
            &mut replaced,
            &[codex(Some(Some(vec!["nice".into(), "-n5".into()])))],
        )
        .unwrap();
    assert_eq!(replaced.agent.wrapper, "nice -n5");

    let mut cleared = new_draft();
    fixture.apply(&mut cleared, &[codex(Some(None))]).unwrap();
    assert_eq!(
        cleared.agent.wrapper, "",
        "an explicit null overrides the remembered prefix"
    );
}

/// Custom flags travel with a template's Custom choice, and a later template's other permission clears them (SPEC.md:
/// "a later template setting another permission clears them").
#[test]
fn custom_words_are_cleared_by_a_later_templates_permission() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    let custom = template(
        "custom",
        TemplateFields {
            agent: Some(LaunchHarness::Codex),
            permissions: Some(Some(LaunchPermission::Custom)),
            permission_flags: Some(vec!["--sandbox".into(), "read only".into()]),
            ..TemplateFields::default()
        },
    );
    fixture
        .apply(&mut draft, std::slice::from_ref(&custom))
        .unwrap();
    assert_eq!(draft.agent.permissions, Some(LaunchPermission::Custom));
    assert_eq!(draft.agent.permission_flags, "--sandbox 'read only'");
    assert!(draft.agent.permissions_explicit);

    let guarded = template(
        "guarded",
        TemplateFields {
            permissions: Some(Some(LaunchPermission::Guarded)),
            ..TemplateFields::default()
        },
    );
    let mut stacked = new_draft();
    fixture.apply(&mut stacked, &[custom, guarded]).unwrap();
    assert_eq!(stacked.agent.permissions, Some(LaunchPermission::Guarded));
    assert_eq!(stacked.agent.permission_flags, "");
}

/// A model only one agent type offers picks that type when none is chosen, as typing it by hand does (SPEC.md: "a
/// bare `gpt-6-luna` picks Codex"); a model id the catalog does not list is kept as the chosen agent's custom model,
/// so a later agent switch clears it as it would a hand-typed one.
#[test]
fn a_model_picks_its_only_agent_and_a_custom_id_keeps_its_owner() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    let luna = template(
        "luna",
        TemplateFields {
            model: Some(Some("gpt-6-luna".into())),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[luna]).unwrap();
    assert_eq!(draft.agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(draft.agent.model.as_deref(), Some("gpt-6-luna"));
    let mut by_hand = new_draft();
    by_hand.choose_catalog_model(
        LaunchHarness::Codex,
        "gpt-6-luna",
        Some(&fixture.catalog),
        fixture
            .preferences
            .remembered_wrapper(LaunchHarness::Codex, None),
    );
    assert_eq!(
        draft, by_hand,
        "the template is the same pick from the model list"
    );

    let custom = template(
        "custom-model",
        TemplateFields {
            agent: Some(LaunchHarness::Claude),
            model: Some(Some("claude-private-preview".into())),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[custom]).unwrap();
    assert_eq!(draft.agent.model.as_deref(), Some("claude-private-preview"));
    assert_eq!(draft.agent.model_owner, Some(LaunchHarness::Claude));
}

/// A model-only template for an older known ID sets both choices in the visible draft. Replay must use the
/// inferred agent's catalog edit so it retains the original model and its supported effort.
#[test]
fn an_older_model_template_applies_its_inferred_agent_to_the_draft() {
    let mut fixture = Fixture::new();
    for (id, family, harness, effort) in [
        (
            "gpt-6-astra",
            "astra",
            LaunchHarness::Codex,
            Some(LaunchEffort::Low),
        ),
        ("opencode/gpt-6.1-sol", "sol", LaunchHarness::OpenCode, None),
    ] {
        fixture.catalog.push(LaunchCatalogModel {
            id: id.into(),
            family: family.into(),
            latest: false,
            harness,
            efforts: effort.into_iter().collect(),
        });
        let mut draft = new_draft();
        assert_eq!(draft.agent.harness, None);
        let saved = template(
            "older model",
            TemplateFields {
                model: Some(Some(id.into())),
                effort: effort.map(|value| Some(value.into())),
                ..TemplateFields::default()
            },
        );
        fixture.apply(&mut draft, &[saved]).unwrap();
        assert_eq!(draft.agent.harness, Some(harness), "{id}");
        assert_eq!(draft.agent.model.as_deref(), Some(id));
        assert_eq!(draft.agent.model_owner, Some(harness));
        assert_eq!(draft.agent.effort, effort);
    }
}

/// Applying a template for a program that cannot receive the initial prompt leaves it dormant, as a hand edit does.
/// An explicit prompt edit still replaces it, and switching back makes the text eligible for the next request.
#[test]
fn the_prompt_follows_the_shared_fold() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, Some(&fixture.catalog), None);
    draft.edit_prompt("draft message".into());
    let message = template(
        "message",
        TemplateFields {
            prompt: Some("from the template".into()),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[message]).unwrap();
    assert_eq!(draft.prompt, "from the template");

    for harness in [LaunchHarness::Bash, LaunchHarness::Zsh, LaunchHarness::Pi] {
        let unsupported = template(
            "switch",
            TemplateFields {
                agent: Some(harness),
                ..TemplateFields::default()
            },
        );
        fixture.apply(&mut draft, &[unsupported]).unwrap();
        assert_eq!(draft.agent.harness, Some(harness));
        assert_eq!(draft.prompt, "from the template");
        assert_eq!(draft.active_prompt(), None);
        draft.choose_harness(LaunchHarness::Codex, Some(&fixture.catalog), None);
        assert_eq!(draft.active_prompt(), Some("from the template"));
    }
}

/// SPEC.md: an initial prompt "survives unrelated template applications", and a template that sets it, to the empty
/// text included, replaces it (the old "initial prompt launcher template stacking" case). A dormant prompt, kept while
/// the Command tab is chosen or while the agent type takes none, must not be dropped by a template that never names the
/// prompt, or a name-only template would silently lose the message. Specified: a Codex draft's prompt is cleared by a
/// template setting `prompt: ""`; set again, it survives a name-only template applied on the Command tab and another
/// applied with Goose chosen by hand, and is still the draft's once Codex is chosen again.
#[test]
fn an_unrelated_template_leaves_a_dormant_prompt_and_an_empty_one_clears_it() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, Some(&fixture.catalog), None);
    draft.edit_prompt("draft message".into());
    let clear = template(
        "clear",
        TemplateFields {
            prompt: Some(String::new()),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[clear]).unwrap();
    assert_eq!(draft.prompt, "", "the empty prompt cleared it");

    draft.edit_prompt("draft message".into());
    let name_only = template(
        "name-only",
        TemplateFields {
            name: Some("unrelated edit".into()),
            ..TemplateFields::default()
        },
    );
    draft.choose_tab(Tab::Command);
    fixture
        .apply(&mut draft, std::slice::from_ref(&name_only))
        .unwrap();
    assert_eq!(draft.prompt, "draft message", "kept on the Command tab");
    draft.choose_tab(Tab::Agent);
    assert!(
        !LaunchHarness::Goose.supports_initial_prompt(false),
        "the premise: Goose takes no initial prompt"
    );
    draft.choose_harness(LaunchHarness::Goose, Some(&fixture.catalog), None);
    fixture.apply(&mut draft, &[name_only]).unwrap();
    draft.choose_harness(LaunchHarness::Codex, Some(&fixture.catalog), None);
    assert_eq!(
        draft.prompt, "draft message",
        "kept with Goose, and the draft's again with Codex"
    );
}

/// SPEC.md: a command template may carry a resume command, and "`null` explicitly resets the choice, the way choosing
/// harness default by hand does" (the old "a template fills the command tab and the folder, and a reset clears a
/// choice"). Specified: a command template declaring Claude, with a resume command, turns Resume on with that
/// text; on a Codex draft, a template with effort High sets it, and a later template with `effort: null` resets it to
/// the default.
#[test]
fn a_template_fills_the_resume_command_and_a_null_effort_resets_the_choice() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    let command = template(
        "command",
        TemplateFields {
            kind: Some(LauncherKind::Command),
            agent: Some(LaunchHarness::Claude),
            command: Some("claude {farhelm_args}".into()),
            resume_command: Some(Some("claude --resume {conversation} {farhelm_args}".into())),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[command]).unwrap();
    assert_eq!(draft.tab, Tab::Command);
    assert!(draft.command.resume_on, "Resume is on");
    assert_eq!(
        draft.command.resume,
        "claude --resume {conversation} {farhelm_args}"
    );

    let mut draft = new_draft();
    let high = template(
        "high",
        TemplateFields {
            agent: Some(LaunchHarness::Codex),
            model: Some(Some("gpt-6-luna".into())),
            effort: Some(Some(LaunchEffort::High.into())),
            ..TemplateFields::default()
        },
    );
    let reset = template(
        "reset",
        TemplateFields {
            effort: Some(None),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[high]).unwrap();
    assert_eq!(draft.agent.effort, Some(LaunchEffort::High), "the premise");
    fixture.apply(&mut draft, &[reset]).unwrap();
    assert_eq!(draft.agent.effort, None, "the reset chose the default");
}

/// SPEC.md: a template's workspace-trust field, from before trust left the launcher, "is ignored when applied".
/// Refusing it would make every such old template unusable. Ignoring it is structural beyond that: the draft has no
/// trust to set and the launch selection carries none, so the test pins only that the field is not refused. Specified:
/// Codex templates with `workspace_trust` true, false and null each apply without refusal.
#[test]
fn an_old_templates_workspace_trust_is_ignored_when_applied() {
    let fixture = Fixture::new();
    for trust in [Some(true), Some(false), None] {
        let mut draft = new_draft();
        let old = template(
            "old",
            TemplateFields {
                kind: Some(LauncherKind::Agent),
                agent: Some(LaunchHarness::Codex),
                workspace_trust: Some(trust),
                ..TemplateFields::default()
            },
        );
        fixture
            .apply(&mut draft, &[old])
            .unwrap_or_else(|refusal| panic!("{trust:?}: {refusal}"));
    }
}

/// A retired container choice in a stored template is shown so it can be removed, but applying it is refused (SPEC.md:
/// "a retired containerized field is refused"), naming the field, with nothing applied; a template that applied part of
/// it would start a session the container settings no longer describe. Specified: a Codex template carrying a container
/// object is refused with a reason naming the container, and the draft keeps its agent type unchosen.
#[test]
fn a_retired_container_template_is_refused_and_changes_nothing() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    let before = draft.clone();
    let retired = template(
        "retired",
        TemplateFields {
            agent: Some(LaunchHarness::Codex),
            container: Some(Some(farhelm_proto::ContainerConfig {
                engine: farhelm_proto::ContainerEngine::Podman,
                image: "localhost/agent:latest".into(),
                api_proxy_port: 42000,
                github_token_file: None,
            })),
            ..TemplateFields::default()
        },
    );
    let refusal = fixture.apply(&mut draft, &[retired]).unwrap_err();
    assert!(refusal.to_lowercase().contains("container"), "{refusal}");
    assert_eq!(draft, before, "nothing applied");
}

// ---------------------------------------------------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------------------------------------------------

/// A template whose field does not apply is refused naming the field, and nothing from it is applied (SPEC.md). Within
/// a batch, a later refusal takes back nothing because nothing was applied yet: one press, one pick or one completed
/// sequence is one refusal unit.
#[test]
fn a_refusal_anywhere_in_the_batch_leaves_the_draft_unchanged() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    draft.choose_tab(Tab::Command);
    let before = draft.clone();
    let model_on_command = template(
        "model-on-command",
        TemplateFields {
            model: Some(Some("gpt-6-luna".into())),
            ..TemplateFields::default()
        },
    );
    let refusal = fixture
        .apply(&mut draft, std::slice::from_ref(&model_on_command))
        .unwrap_err();
    assert_eq!(
        refusal,
        "Template \"model-on-command\" was not applied: its model applies to an agent launch, and the launcher is \
         on the command launch kind"
    );
    assert_eq!(draft, before);

    let fine = template(
        "fine",
        TemplateFields {
            command: Some("make dev".into()),
            ..TemplateFields::default()
        },
    );
    assert!(
        fixture
            .apply(&mut draft, &[fine, model_on_command])
            .is_err()
    );
    assert_eq!(
        draft, before,
        "the first template's edits did not land either"
    );
}

/// A template names a host by its recorded install identity, so a row in the identity-mismatch phase (whose recorded
/// identity belongs to the install it used to reach) and a session-owned environment host never match. The template
/// is refused rather than aimed at the successor.
#[test]
fn a_host_is_matched_only_by_a_usable_install_identity() {
    let mut fixture = Fixture::new();
    fixture.hosts[1].state = HostStateView::IdentityMismatch {
        recorded: "install-two".into(),
        reported: "install-three".into(),
    };
    assert_eq!(install_identity(&fixture.hosts[1]), None);
    let mut draft = new_draft();
    let before = draft.clone();
    let two = template(
        "two",
        TemplateFields {
            host: Some("install-two".into()),
            ..TemplateFields::default()
        },
    );
    let refusal = fixture
        .apply(&mut draft, std::slice::from_ref(&two))
        .unwrap_err();
    assert!(
        refusal.contains("its host names an install no host here has"),
        "{refusal}"
    );
    assert_eq!(draft, before);

    let mut owned = Fixture::new();
    owned.hosts[1].is_session_owned = true;
    let refusal = owned.apply(&mut draft, &[two]).unwrap_err();
    assert!(
        refusal.contains("its host names an install no host here has"),
        "{refusal}"
    );
}

/// A host change by template re-seeds a folder the user has not typed or picked, through the same hand edit as
/// choosing the host, and leaves a typed folder alone (SPEC.md, "Working directory").
#[test]
fn a_templates_host_reseeds_only_an_untouched_folder() {
    let fixture = Fixture::new();
    let two = template(
        "two",
        TemplateFields {
            host: Some("install-two".into()),
            ..TemplateFields::default()
        },
    );
    let mut untouched = new_draft();
    fixture
        .apply(&mut untouched, std::slice::from_ref(&two))
        .unwrap();
    assert_eq!(
        (untouched.host, untouched.folder.value().as_str()),
        (Some(2), "/home/2")
    );

    let mut typed = new_draft();
    typed.edit_folder("/mine".into());
    fixture.apply(&mut typed, &[two]).unwrap();
    assert_eq!(
        (typed.host, typed.folder.value().as_str()),
        (Some(2), "/mine")
    );
}

/// SPEC.md, "Launch templates": applying a template makes its edits "in the same way and with the same effects as making
/// them by hand". A repository destination is the repository field's choice, so it goes through
/// `LaunchDraft::choose_repository`, which also asks the helm for a new checkout preview; a folder destination is the
/// folder edit, which also returns a checkout draft to that folder. Specified: a repository template makes the
/// destination a managed checkout of that repository, chosen once, with the set-aside folder as it was; a later folder
/// template returns the draft to that folder.
#[test]
fn a_repository_destination_chooses_that_repository() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    let folder_before = draft.folder.clone();
    let repo = template(
        "repo",
        TemplateFields {
            destination: Some(TemplateDestination::Github("Example/Project".into())),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[repo]).unwrap();
    let checkout = draft.checkout.clone().expect("a managed checkout");
    let chosen = checkout.repo.expect("a chosen repository");
    assert_eq!(
        (chosen.owner.as_str(), chosen.name.as_str()),
        ("example", "project")
    );
    assert_eq!(checkout.chosen, 1, "chosen as a hand pick is, once");
    assert_eq!(
        draft.folder, folder_before,
        "the folder is set aside, not changed"
    );

    let folder = template(
        "folder",
        TemplateFields {
            destination: Some(TemplateDestination::Folder("/work".into())),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[folder]).unwrap();
    assert_eq!(
        (draft.checkout.clone(), draft.folder.value().as_str()),
        (None, "/work")
    );
}

/// A repository the shared fold takes as text must still be one the launcher can choose, or applying it would leave the
/// destination half-set. The old app refused it in its own two sentences: one for text that is no `owner/repo` pair at
/// all, one for a pair the rules refuse (a URL, a branch suffix, a bad name). Specified: `not a repo` and `a/b/c` are
/// each refused with the whole old sentence, and the draft is unchanged.
#[test]
fn a_destination_that_is_not_a_repository_is_refused() {
    let fixture = Fixture::new();
    for (text, reason) in [
        ("not a repo", "Enter a GitHub repository as owner/repo"),
        (
            "a/b/c",
            "Enter a valid GitHub owner/repo pair without a URL or branch suffix",
        ),
    ] {
        let mut draft = new_draft();
        let before = draft.clone();
        let bad = template(
            "bad",
            TemplateFields {
                destination: Some(TemplateDestination::Github(text.into())),
                ..TemplateFields::default()
            },
        );
        assert_eq!(
            fixture.apply(&mut draft, &[bad]).unwrap_err(),
            format!(
                "Template \"bad\" was not applied: its destination {text:?} is not a GitHub repository ({reason})"
            )
        );
        assert_eq!(draft, before);
    }
}

/// The snapshot is what Save as template and the held-value checks read, so it must say what the launcher would launch
/// into: the chosen repository while a checkout is chosen, never the set-aside folder, and nothing for typed repository
/// text no one chose (the old `launcher_snapshot`: an unselected query "is an editor draft, not a reusable
/// destination"). Specified: a typed folder, then a typed repository query, then a chosen repository, read as the
/// folder, no destination, and the repository; applying the snapshot's destination to a fresh draft chooses it again.
#[test]
fn the_snapshot_reads_a_chosen_repository_as_the_destination() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    draft.edit_folder("/typed".into());
    assert_eq!(
        snapshot(&draft, &fixture.hosts).destination,
        Some(TemplateDestination::Folder("/typed".into()))
    );
    draft.edit_repository_query("example/proj".into());
    assert_eq!(snapshot(&draft, &fixture.hosts).destination, None);
    draft.choose_repository(farhelm_proto::parse_github_repo("example/project").unwrap());
    let destination = snapshot(&draft, &fixture.hosts).destination;
    assert_eq!(
        destination,
        Some(TemplateDestination::Github("example/project".into()))
    );

    let mut fresh = new_draft();
    let again = template(
        "again",
        TemplateFields {
            destination,
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut fresh, &[again]).unwrap();
    assert_eq!(
        fresh.checkout.and_then(|checkout| checkout.repo),
        draft.checkout.and_then(|checkout| checkout.repo)
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Held values: Replace with and Restart with
// ---------------------------------------------------------------------------------------------------------------------

/// Replace with holds the host. A template naming the same host applies its other fields with the host left alone; one
/// naming another host is refused (SPEC.md: a template "that changes a field the current launcher mode holds fixed,
/// such as Replace with's host or folder").
#[test]
fn replace_with_drops_an_equal_host_and_refuses_another() {
    let fixture = Fixture::new();
    let session = session(SessionLaunch::Command(CommandLaunch {
        custom_agent: None,
        command: "make dev".into(),
        yolo: false,
        agent: None,
        resume: None,
    }));
    let mut draft = LaunchDraft::replace_with("s1", &session, Some(1));
    let same_host = template(
        "same-host",
        TemplateFields {
            host: Some("install-one".into()),
            command: Some("make test".into()),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[same_host]).unwrap();
    assert_eq!(draft.command.command, "make test");
    assert_eq!(draft.host, Some(1));
    assert_eq!(
        draft.folder.value(),
        "/src/project",
        "an equal host is no host change, so no re-seed"
    );

    let before = draft.clone();
    let other_host = template(
        "other-host",
        TemplateFields {
            host: Some("install-two".into()),
            ..TemplateFields::default()
        },
    );
    let refusal = fixture.apply(&mut draft, &[other_host]).unwrap_err();
    assert!(refusal.contains("its host cannot change here"), "{refusal}");
    assert_eq!(draft, before);
}

/// Replace with holds the source's folder too (SPEC.md, "Replace with": the destination stays the source's). A template
/// repeating that folder, trailing slash and all, applies its other fields with the folder left alone; one naming
/// another folder, or any repository, is refused whole. Why: applying the rest while dropping the destination would
/// launch somewhere other than the template says, and taking it would move the replacement out of the source's folder
/// or checkout.
#[test]
fn replace_with_drops_an_equal_folder_and_refuses_another_destination() {
    let fixture = Fixture::new();
    let session = session(SessionLaunch::Command(CommandLaunch {
        custom_agent: None,
        command: "make dev".into(),
        yolo: false,
        agent: None,
        resume: None,
    }));
    let mut draft = LaunchDraft::replace_with("s1", &session, Some(1));
    assert_eq!(
        draft.folder.value(),
        "/src/project",
        "the premise: the source's folder"
    );
    let same_folder = template(
        "same-folder",
        TemplateFields {
            destination: Some(TemplateDestination::Folder("/src/project/".into())),
            command: Some("make test".into()),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[same_folder]).unwrap();
    assert_eq!(draft.command.command, "make test");
    assert_eq!(draft.folder.value(), "/src/project");

    for (name, destination) in [
        (
            "other-folder",
            TemplateDestination::Folder("/srv/elsewhere".into()),
        ),
        (
            "repository",
            TemplateDestination::Github("acme/widget".into()),
        ),
    ] {
        let before = draft.clone();
        let moving = template(
            name,
            TemplateFields {
                destination: Some(destination),
                command: Some("make lint".into()),
                ..TemplateFields::default()
            },
        );
        let refusal = fixture.apply(&mut draft, &[moving]).unwrap_err();
        assert!(
            refusal.contains(
                "its destination cannot change here: this dialog keeps the session's folder"
            ),
            "{name}: {refusal}"
        );
        assert_eq!(draft, before, "{name}: nothing from it is applied");
    }
}

/// Restart with holds the launch kind, the agent type, the host, the folder and the name. A template that repeats them
/// exactly (the folder with a trailing slash, the title byte for byte) applies silently as nothing for those fields,
/// and its other edits land. Without the removal, every one of those fields would be refused as a change to a held
/// field, so the template applying at all is what this pins.
#[test]
fn restart_with_applies_equal_held_values_as_nothing() {
    let fixture = Fixture::new();
    let mut draft = restart_draft();
    let repeat = template(
        "repeat",
        TemplateFields {
            kind: Some(LauncherKind::Agent),
            agent: Some(LaunchHarness::Codex),
            host: Some("install-one".into()),
            destination: Some(TemplateDestination::Folder("/src/project/".into())),
            name: Some("source".into()),
            name_kind: Some(TemplateNameKind::Fixed),
            permissions: Some(Some(LaunchPermission::Yolo)),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[repeat]).unwrap();
    assert_eq!(draft.agent.permissions, Some(LaunchPermission::Yolo));
    assert_eq!(
        draft.name_kind, None,
        "a repeated name brings no template kind with it"
    );
    assert_eq!(draft.folder.value(), "/src/project");
}

/// Restart with keeps the launch kind, agent type, host, folder and session name fixed and never sends an initial
/// prompt, so a template that would change one of them must be refused whole, naming the field, or a partly applied
/// batch would restart with edits the user never saw as one unit (the old `restart-with.spec.ts` case "restart with
/// refuses a template's held ${field} atomically", over the same six fields). Specified: beside a Low effort, which
/// alone applies, a template changing the launch kind, the agent type, the host (another install, not a placement), the
/// folder, the session name or the initial prompt is refused in the old words naming that field, and the draft is
/// unchanged, the effort included.
#[test]
fn restart_with_refuses_a_change_to_a_held_field() {
    // A plain function pointer per field, so the different closures can share one array.
    type Change = fn(&mut TemplateFields);
    let fixture = Fixture::new();
    let low = TemplateFields {
        effort: Some(Some(LaunchEffort::Low.into())),
        ..TemplateFields::default()
    };
    fixture
        .apply(&mut restart_draft(), &[template("low", low.clone())])
        .expect("the premise: the effort alone applies");
    let changes: [(&str, Change); 6] = [
        ("launch kind", |f| f.kind = Some(LauncherKind::Command)),
        ("agent type", |f| f.agent = Some(LaunchHarness::Claude)),
        ("host", |f| f.host = Some("install-two".into())),
        ("destination", |f| {
            f.destination = Some(TemplateDestination::Folder("/other-folder".into()))
        }),
        ("session name", |f| f.name = Some("source-other".into())),
        ("initial prompt", |f| f.prompt = Some("hello".into())),
    ];
    for (field, change) in changes {
        let mut fields = low.clone();
        change(&mut fields);
        let mut draft = restart_draft();
        let before = draft.clone();
        assert_eq!(
            fixture
                .apply(&mut draft, &[template("held", fields)])
                .unwrap_err(),
            format!(
                "Template \"held\" was not applied: its {field} cannot change here: this dialog keeps {field} fixed \
                 for restart"
            )
        );
        assert_eq!(draft, before, "{field}: nothing applied");
    }
}

/// In Restart with, a template's explicit permission reset asks for the agent's guarded default rather than the
/// historical unset value the stored command may carry, so it reads as the change the helm would make.
#[test]
fn restart_with_resets_permissions_to_todays_default() {
    let fixture = Fixture::new();
    let mut draft = restart_draft();
    draft.choose_permissions(Some(LaunchPermission::Yolo));
    let reset = template(
        "reset",
        TemplateFields {
            permissions: Some(None),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[reset]).unwrap();
    assert_eq!(draft.agent.permissions, Some(LaunchPermission::Guarded));
}

/// A partial agent template is how a user keeps resume settings, so in Restart with it must change what a restart may
/// change and reach the restart it sends; the tests above pin only what is dropped or refused (the old
/// `restart-with.spec.ts` case "restart with applies an allowed partial agent template"). Specified: on a Codex session
/// stored with no model and no permission, a template choosing `gpt-6-luna`, Low effort, a permission reset and the
/// wrapper prefix `env RESUME_MODE=1` keeps Codex, and the restart sends exactly that selection, the reset as Guarded,
/// and no command edit.
#[test]
fn restart_with_applies_a_partial_agent_template_and_sends_its_choices() {
    let fixture = Fixture::new();
    let mut session = restart_session(None);
    // No stored permission, as the old case's session had, so the reset's Guarded is a change the body must carry.
    if let SessionLaunch::Agent { selection, .. } = &mut session.launch {
        selection.permissions = None;
    }
    let mut draft = LaunchDraft::restart_with("s1", &session, Some(1)).unwrap();
    let settings = template(
        "resume-settings",
        TemplateFields {
            model: Some(Some("gpt-6-luna".into())),
            effort: Some(Some(LaunchEffort::Low.into())),
            permissions: Some(None),
            wrapper_prefix: Some(Some(vec!["env".into(), "RESUME_MODE=1".into()])),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[settings]).unwrap();
    assert_eq!(draft.agent.harness, Some(LaunchHarness::Codex));
    let body = restart_body(&fixture, &draft, &session.launch).expect("the template made an edit");
    assert_eq!(
        body.with,
        Some(LaunchSelection {
            custom_agent: None,
            harness: LaunchHarness::Codex,
            model: Some("gpt-6-luna".into()),
            effort: Some(LaunchEffort::Low.into()),
            permissions: Some(LaunchPermission::Guarded),
            permission_flags: None,
            workspace_trust: None,
            wrapper_prefix: Some(vec!["env".into(), "RESUME_MODE=1".into()]),
            container: None,
        })
    );
    assert_eq!(body.with_command, None);
}

/// A command template edits only a command launch's own fields, so it needs no launch kind or agent type to apply in
/// Restart with, and the session keeps its declared agent and Resume (the old `restart-with.spec.ts` case "restart with
/// applies an allowed partial command template"). Specified: on a Claude command session with a resume command, a
/// template setting only a new command, a new resume command and YOLO keeps the Command tab with Resume on, and the
/// restart sends that command launch for Claude, and no agent edit.
#[test]
fn restart_with_applies_a_partial_command_template_and_sends_it() {
    let fixture = Fixture::new();
    let mut session = session(SessionLaunch::Command(CommandLaunch {
        custom_agent: None,
        command: "claude {farhelm_args}".into(),
        yolo: false,
        agent: Some(LaunchHarness::Claude),
        resume: Some("claude --resume {conversation} {farhelm_args}".into()),
    }));
    session.restart_offer = RestartOffer::Resume;
    let mut draft = LaunchDraft::restart_with("s1", &session, Some(1)).unwrap();
    let command = "claude --model sonnet {farhelm_args}";
    let resume = "claude --model sonnet --resume {conversation} {farhelm_args}";
    let edit = template(
        "resume-command",
        TemplateFields {
            command: Some(command.into()),
            resume_command: Some(Some(resume.into())),
            yolo: Some(true),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[edit]).unwrap();
    assert_eq!(draft.tab, Tab::Command);
    assert!(draft.command.resume_on, "Resume stays on");
    let body = restart_body(&fixture, &draft, &session.launch).expect("the template made an edit");
    assert_eq!(
        body.with_command,
        Some(CommandLaunch {
            custom_agent: None,
            command: command.into(),
            yolo: true,
            agent: Some(LaunchHarness::Claude),
            resume: Some(resume.into()),
        })
    );
    assert_eq!(body.with, None);
}

// ---------------------------------------------------------------------------------------------------------------------
// The escalation banner and the session name's kind
// ---------------------------------------------------------------------------------------------------------------------

/// The banner is judged once per batch, against the draft from before it: two templates that switch Codex to Claude
/// and then to Pi raise one banner from Codex's Guarded to Pi's YOLO, whose Undo is the draft before the press. A
/// switching template that chooses the permission itself raises none, since a direct choice never does.
#[test]
fn escalation_is_judged_once_for_the_batch() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, Some(&fixture.catalog), None);
    let before = draft.clone();
    let agent = |name: &str, harness| {
        template(
            name,
            TemplateFields {
                agent: Some(harness),
                ..TemplateFields::default()
            },
        )
    };
    let edit = fixture
        .apply(
            &mut draft,
            &[
                agent("claude", LaunchHarness::Claude),
                agent("pi", LaunchHarness::Pi),
            ],
        )
        .unwrap();
    assert!(edit.switched_agent);
    let escalation = edit.escalation.expect("Guarded to YOLO is an escalation");
    assert_eq!(escalation.before, Some(LaunchPermission::Guarded));
    assert_eq!(escalation.after, Some(LaunchPermission::Yolo));
    assert_eq!(*escalation.undo, before);

    let mut chosen = before.clone();
    let pi_yolo = template(
        "pi-yolo",
        TemplateFields {
            agent: Some(LaunchHarness::Pi),
            permissions: Some(Some(LaunchPermission::Yolo)),
            ..TemplateFields::default()
        },
    );
    let edit = fixture.apply(&mut chosen, &[pi_yolo]).unwrap();
    assert!(edit.switched_agent);
    assert_eq!(edit.escalation, None);
}

/// A template's name is a base name unless it says fixed; typing in Name afterwards makes it the user's exact name and
/// drops the kind (SPEC.md: "Typing in Name or accepting `name:` makes it the user's exact name").
#[test]
fn a_templates_name_kind_lasts_until_the_name_is_typed() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    let named = template(
        "named",
        TemplateFields {
            name: Some("webbuilder".into()),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[named]).unwrap();
    assert_eq!(
        draft.name_kind,
        Some(TemplateNameKind::Base),
        "absent kind means base"
    );
    assert_eq!(
        snapshot(&draft, &fixture.hosts).name_kind,
        Some(TemplateNameKind::Base)
    );
    draft.edit_name("webbuilder".into());
    assert_eq!(draft.name_kind, None);
}

/// The snapshot reads the host by its usable install identity and leaves out text that does not parse, so Save as
/// template never captures a half-typed wrapper or a mismatched host's predecessor identity.
#[test]
fn the_snapshot_reads_identities_and_skips_unparsed_text() {
    let mut fixture = Fixture::new();
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, Some(&fixture.catalog), None);
    draft.edit_wrapper("env 'unterminated".into());
    let state = snapshot(&draft, &fixture.hosts);
    assert_eq!(state.host.as_deref(), Some("install-one"));
    assert_eq!(state.wrapper_prefix, None);
    assert_eq!(state.kind, Some(LauncherKind::Agent));

    fixture.hosts[0].state = HostStateView::IdentityMismatch {
        recorded: "install-one".into(),
        reported: "install-other".into(),
    };
    assert_eq!(snapshot(&draft, &fixture.hosts).host, None);
}

/// In a Command restart, a template's explicit reset of the resume command would switch Resume off, which Restart with
/// does not allow (SPEC.md: "clearing a command's resume choice remain refused").
#[test]
fn restart_with_refuses_clearing_a_commands_resume() {
    let fixture = Fixture::new();
    let mut session = session(SessionLaunch::Command(CommandLaunch {
        custom_agent: None,
        command: "codex {farhelm_args}".into(),
        yolo: false,
        agent: Some(LaunchHarness::Codex),
        resume: Some("codex resume {conversation} {farhelm_args}".into()),
    }));
    session.restart_offer = RestartOffer::Resume;
    let mut draft = LaunchDraft::restart_with("s1", &session, Some(1)).unwrap();
    assert_eq!(
        draft.tab,
        Tab::Command,
        "the premise: a command launch restarts on the Command tab"
    );
    let before = draft.clone();
    let reset = template(
        "reset",
        TemplateFields {
            resume_command: Some(None),
            ..TemplateFields::default()
        },
    );
    let refusal = fixture.apply(&mut draft, &[reset]).unwrap_err();
    assert!(
        refusal.contains("its resume choice cannot change here"),
        "{refusal}"
    );
    assert_eq!(draft, before);
}

/// A permission chosen before the batch's last agent switch does not hide the banner: on OMP a template choosing
/// Approve and then one switching to Pi, which offers only YOLO, still warns, because the switch is what changed the
/// permission. Only the switching template's own choice hides it (`escalation_is_judged_once_for_the_batch`).
#[test]
fn a_permission_chosen_before_the_last_switch_still_raises_the_banner() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Omp, Some(&fixture.catalog), None);
    let approve = template(
        "approve",
        TemplateFields {
            permissions: Some(Some(LaunchPermission::Approve)),
            ..TemplateFields::default()
        },
    );
    let pi = template(
        "pi",
        TemplateFields {
            agent: Some(LaunchHarness::Pi),
            ..TemplateFields::default()
        },
    );
    let applied = fixture.apply(&mut draft, &[approve, pi]).unwrap();
    assert_eq!(draft.agent.permissions, Some(LaunchPermission::Yolo));
    assert!(
        applied.escalation.is_some(),
        "the switch to Pi made the permission more dangerous"
    );
}

/// A batch that switches agents and back still reports a switch, so the view takes down a banner an earlier switch
/// raised (its Undo would take back this batch too). A batch that makes no edit to the agent controls leaves their
/// notice alone rather than clearing it.
#[test]
fn a_batch_reports_any_switch_and_leaves_an_untouched_notice() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, Some(&fixture.catalog), None);
    let agent = |name: &str, harness| {
        template(
            name,
            TemplateFields {
                agent: Some(harness),
                ..TemplateFields::default()
            },
        )
    };
    let there_and_back = fixture
        .apply(
            &mut draft,
            &[
                agent("claude", LaunchHarness::Claude),
                agent("codex", LaunchHarness::Codex),
            ],
        )
        .unwrap();
    assert_eq!(draft.agent.harness, Some(LaunchHarness::Codex));
    assert!(there_and_back.switched_agent);

    let folder = template(
        "folder",
        TemplateFields {
            destination: Some(TemplateDestination::Folder("/src/other".into())),
            ..TemplateFields::default()
        },
    );
    let applied = fixture.apply(&mut draft, &[folder]).unwrap();
    assert_eq!(
        applied,
        Applied {
            template: Some("folder".into()),
            ..Applied::default()
        },
        "the form records the applied template and leaves the agent notice alone"
    );
}

/// A permission a later template chooses after the batch's last agent switch is a direct choice, not the switch's
/// doing: stacking `claude` and then a YOLO template on Codex's Guarded raises no banner, as the old app raised none.
#[test]
fn a_permission_chosen_after_the_last_switch_raises_no_banner() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, Some(&fixture.catalog), None);
    assert_eq!(
        draft.agent.permissions,
        Some(LaunchPermission::Guarded),
        "the premise: Codex starts below YOLO, so a banner could be raised"
    );
    let claude = template(
        "claude",
        TemplateFields {
            agent: Some(LaunchHarness::Claude),
            ..TemplateFields::default()
        },
    );
    let yolo = template(
        "yolo",
        TemplateFields {
            permissions: Some(Some(LaunchPermission::Yolo)),
            ..TemplateFields::default()
        },
    );
    let applied = fixture.apply(&mut draft, &[claude, yolo]).unwrap();
    assert_eq!(draft.agent.permissions, Some(LaunchPermission::Yolo));
    assert!(applied.switched_agent);
    assert_eq!(applied.escalation, None);
}

/// An edit a later template makes after the switch leaves the switch's banner up, as the old app did: there, only an
/// agent switch replaced the banner, so stacking `pi` and then a template that switches to the Command tab on Codex's
/// Guarded still warns that Pi moved Guarded to YOLO, and its Undo goes back to before the press.
#[test]
fn a_later_edit_after_the_switch_keeps_the_switchs_banner() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, Some(&fixture.catalog), None);
    let before = draft.clone();
    let pi = template(
        "pi",
        TemplateFields {
            agent: Some(LaunchHarness::Pi),
            ..TemplateFields::default()
        },
    );
    let command = template(
        "command",
        TemplateFields {
            kind: Some(LauncherKind::Command),
            ..TemplateFields::default()
        },
    );
    let applied = fixture.apply(&mut draft, &[pi, command]).unwrap();
    assert_eq!(draft.tab, Tab::Command);
    let escalation = applied
        .escalation
        .expect("the switch to Pi raised the banner before the later edit");
    assert_eq!(escalation.after, Some(LaunchPermission::Yolo));
    assert_eq!(*escalation.undo, before);
}

// ---------------------------------------------------------------------------------------------------------------------
// Environment placements
// ---------------------------------------------------------------------------------------------------------------------

/// The target a container template of `recipe` on host `host` stores.
fn container(recipe: &str, host: i64) -> LaunchTarget {
    LaunchTarget::Environment {
        recipe: recipe.into(),
        place: Place::PodmanHost(host),
    }
}

/// A template named `name` that places sessions at `target` and sets nothing else.
fn placed(name: &str, target: LaunchTarget) -> LaunchTemplate {
    template(
        name,
        TemplateFields {
            target: Some(target),
            ..TemplateFields::default()
        },
    )
}

/// SPEC.md, "Creation": templates "carry the recipe and place together", and applying one is the same as choosing it by
/// hand, so a container template must land as New container does: the target set, the folder at the environment's home
/// whatever folder the draft had, and the Host row's host kept underneath for when the user leaves the environment. The
/// snapshot, which the fold and Save as template read, then reports the target and not that host, which the launch
/// never uses. A container host whose Podman check has no result still counts as a known place: whether it can take a
/// container now is the launcher's refusal, not the template's. Specified: a fresh New on host 1 in `/home/1`, given a
/// template of `web` on host 2, whose check has no result, holds that target, `~` as its folder, and still host 1, and
/// its snapshot names the target and no host.
#[test]
fn a_container_template_enters_a_new_container_at_its_home() {
    let mut fixture = Fixture::new();
    fixture.hosts[1].container_check = None;
    let mut draft = LaunchDraft::new(Some(1), "/home/1".into(), None);
    fixture
        .apply(&mut draft, &[placed("web-box", container("web", 2))])
        .unwrap();
    assert_eq!(draft.target, Some(container("web", 2)));
    assert_eq!(draft.folder.value(), "~", "the environment's home");
    assert_eq!(
        draft.host,
        Some(1),
        "the Host row's host is kept underneath"
    );
    let state = snapshot(&draft, &fixture.hosts);
    assert_eq!(
        (state.host, state.target),
        (None, Some(container("web", 2)))
    );
}

/// The Runs on switch ignores a press of the segment already chosen (SPEC.md: "An active segment keeps its explicit
/// choices"), so a template for that segment must change its recipe and host as choosing them in their own fields
/// does, or it would apply as nothing; and like those fields it must change nothing else. Specified: a
/// draft already in a container of `web` on host 1, with a typed name, given a template of `api` on host 2, differs
/// from before in its target alone.
#[test]
fn a_template_for_the_chosen_segment_changes_only_its_recipe_and_host() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    fixture
        .apply(&mut draft, &[placed("web-box", container("web", 1))])
        .unwrap();
    draft.edit_name("review".into());
    let before = draft.clone();
    assert_eq!(
        before.target,
        Some(container("web", 1)),
        "the premise: in a container"
    );
    fixture
        .apply(&mut draft, &[placed("api-box", container("api", 2))])
        .unwrap();
    let expected = LaunchDraft {
        target: Some(container("api", 2)),
        ..before
    };
    assert_eq!(draft, expected);
}

/// Templates saved before targets existed store a recipe alone, and the proto reads it as that recipe on the Tensorlake
/// account; such a template must keep launching where it always did. Specified: a template with only `recipe: web`
/// leaves the draft in a new sandbox of `web`.
#[test]
fn a_legacy_recipe_template_becomes_a_sandbox_target() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    let legacy = template(
        "legacy",
        TemplateFields {
            recipe: Some("web".into()),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[legacy]).unwrap();
    assert_eq!(
        draft.target,
        Some(LaunchTarget::Environment {
            recipe: "web".into(),
            place: Place::TensorlakeAccount,
        })
    );
}

/// SPEC.md, "Creation": "Missing recipes or removed place hosts make that placement inapplicable", and a template that
/// does not apply changes nothing; a placement on a host that is gone, or that is an environment's own, must not launch
/// somewhere else either. Specified: a target naming the unknown recipe `gone`, one on host 7 (no such host) and one on
/// host 2 once it is a container's own host are each refused, naming the recipe or the place, with the draft as before.
#[test]
fn a_missing_recipe_or_removed_place_host_is_refused_with_nothing_applied() {
    let mut fixture = Fixture::new();
    // An environment's own host can hold no containers, whatever else its row says.
    fixture.hosts[1].kind = "podman".into();
    let mut draft = new_draft();
    let before = draft.clone();
    for (target, words) in [
        (
            container("gone", 1),
            "names a recipe that is unavailable here",
        ),
        (
            container("web", 7),
            "names a place that is unavailable here",
        ),
        (
            container("web", 2),
            "names a place that is unavailable here",
        ),
    ] {
        let refusal = fixture
            .apply(&mut draft, &[placed("box", target)])
            .unwrap_err();
        assert!(refusal.contains(words), "{refusal}");
        assert_eq!(draft, before, "nothing applied");
    }
}

/// Placement is one choice: an installed host or a new environment (SPEC.md, "Sandbox sessions": "The two choices are
/// mutually exclusive"). A later template's host must therefore take the launch out of the environment, as picking a
/// host by hand does, or it would launch into the environment the user stacked a host over. Specified: a batch of a
/// container template and then a template of host 2 leaves no target and host 2 chosen.
#[test]
fn a_template_host_leaves_the_environment() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    let host_two = template(
        "host-two",
        TemplateFields {
            host: Some("install-two".into()),
            ..TemplateFields::default()
        },
    );
    fixture
        .apply(
            &mut draft,
            &[placed("web-box", container("web", 1)), host_two],
        )
        .unwrap();
    assert_eq!(draft.target, None);
    assert_eq!(draft.host, Some(2));
}

/// Restart with keeps the session where it runs (SPEC.md, "Restart with"), so a template that would move it into a new
/// environment must be refused rather than ignored, and, as the old app did, under the host it holds. Specified: a
/// container template in Restart with is refused naming the host as kept fixed, and the draft is as before.
#[test]
fn restart_with_refuses_a_placement() {
    let fixture = Fixture::new();
    let mut draft = restart_draft();
    let before = draft.clone();
    let refusal = fixture
        .apply(&mut draft, &[placed("web-box", container("web", 1))])
        .unwrap_err();
    assert!(refusal.contains("its host cannot change here"), "{refusal}");
    assert_eq!(draft, before, "nothing applied");
}

/// SPEC.md, "Sandbox sessions": "a recipe with a non-home Folder is inapplicable", and a new environment's folder is
/// fixed to its home, so a template's other folder cannot apply there; applied in part (the replay's folder edit is
/// ignored under a target), the template would break the all-or-nothing rule. Specified: a draft already in a container
/// of `web`, given a template with folder `/x`, and a fresh draft given the batch of that container template and then
/// the folder template, are each refused naming the folder, with the draft as before.
#[test]
fn a_template_folder_is_refused_in_a_new_environment() {
    let fixture = Fixture::new();
    let folder = template(
        "folder",
        TemplateFields {
            destination: Some(TemplateDestination::Folder("/x".into())),
            ..TemplateFields::default()
        },
    );
    let mut draft = new_draft();
    fixture
        .apply(&mut draft, &[placed("web-box", container("web", 1))])
        .unwrap();
    let before = draft.clone();
    assert_eq!(
        before.target,
        Some(container("web", 1)),
        "the premise: in a container"
    );
    let refusal = fixture
        .apply(&mut draft, std::slice::from_ref(&folder))
        .unwrap_err();
    assert!(refusal.contains("its folder"), "{refusal}");
    assert_eq!(draft, before, "nothing applied");

    let mut draft = new_draft();
    let before = draft.clone();
    let refusal = fixture
        .apply(
            &mut draft,
            &[placed("web-box", container("web", 1)), folder],
        )
        .unwrap_err();
    assert!(refusal.contains("its folder"), "{refusal}");
    assert_eq!(draft, before, "nothing applied");
}

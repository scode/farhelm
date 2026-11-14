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
use farhelm_proto::ui_channel::{HostStateView, HostView, SessionRow};
use farhelm_proto::{
    CommandLaunch, LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, RestartOffer,
    SessionInfo, SessionLaunch,
};

use super::apply::{Applied, Context, apply, install_identity, snapshot};
use crate::launcher::draft::{LaunchDraft, Tab};

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
    // The premise the host tests rely on: both rows offer a usable identity.
    assert!(matches!(one.state, HostStateView::Connected { .. }));
    vec![one, two]
}

/// A catalog with one Codex model offering two efforts and one Claude model offering one.
fn catalog() -> Vec<LaunchCatalogModel> {
    vec![
        LaunchCatalogModel {
            id: "gpt-6-luna".into(),
            harness: LaunchHarness::Codex,
            efforts: vec![LaunchEffort::Low, LaunchEffort::High],
        },
        LaunchCatalogModel {
            id: "claude-fable-5".into(),
            harness: LaunchHarness::Claude,
            efforts: vec![LaunchEffort::High],
        },
    ]
}

/// The folder New would seed for a host: `/home/<id>`, so a test can tell which host seeded it.
fn default_folder(host: i64) -> String {
    format!("/home/{host}")
}

/// Everything a test applies against. Held in one place so each test can borrow a [`Context`] from it.
struct Fixture {
    hosts: Vec<HostView>,
    catalog: Vec<LaunchCatalogModel>,
    wrappers: BTreeMap<LaunchHarness, Vec<String>>,
}

impl Fixture {
    fn new() -> Fixture {
        Fixture {
            hosts: hosts(),
            catalog: catalog(),
            wrappers: BTreeMap::from([(
                LaunchHarness::Codex,
                vec!["env".into(), "FOO='a b'".into()],
            )]),
        }
    }

    fn context(&self) -> Context<'_> {
        Context {
            hosts: &self.hosts,
            catalog: Some(&self.catalog),
            remembered_wrappers: &self.wrappers,
            default_folder: &default_folder,
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

/// A Codex agent session with a stored resume command and a captured conversation, as Restart with opens it.
fn restart_draft() -> LaunchDraft {
    let selection = LaunchSelection {
        harness: LaunchHarness::Codex,
        model: Some("gpt-6-luna".into()),
        effort: None,
        permissions: Some(LaunchPermission::Guarded),
        permission_flags: None,
        workspace_trust: None,
        wrapper_prefix: None,
        container: None,
    };
    let mut session = session(SessionLaunch::Agent {
        selection,
        start: vec!["codex".into(), "{farhelm_args}".into()],
        resume: Some(vec![
            "codex".into(),
            "{conversation}".into(),
            "{farhelm_args}".into(),
        ]),
    });
    session.restart_offer = RestartOffer::Resume;
    LaunchDraft::restart_with("s1", &session, Some(1)).unwrap()
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
            effort: Some(Some(LaunchEffort::High)),
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
            .wrappers
            .get(&LaunchHarness::Codex)
            .map(Vec::as_slice),
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
            .wrappers
            .get(&LaunchHarness::Codex)
            .map(Vec::as_slice),
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

/// A template that switches to an agent that cannot take an initial prompt clears the dormant prompt, as the shared
/// fold says, while one that sets its own prompt replaces it. This is the one place the replay writes the fold's
/// result directly, so both directions are pinned.
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

    let pi = template(
        "pi",
        TemplateFields {
            agent: Some(LaunchHarness::Pi),
            ..TemplateFields::default()
        },
    );
    fixture.apply(&mut draft, &[pi]).unwrap();
    assert_eq!(
        draft.prompt, "",
        "Pi takes no initial prompt, so the template's switch drops it"
    );
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

/// A GitHub destination cannot be applied until the launcher can hold a managed checkout; until then it is refused
/// with a sentence that says why, rather than silently dropped (which would launch into the wrong folder).
#[test]
fn a_github_destination_is_refused_until_managed_checkouts_exist() {
    let fixture = Fixture::new();
    let mut draft = new_draft();
    let before = draft.clone();
    let repo = template(
        "repo",
        TemplateFields {
            destination: Some(TemplateDestination::Github("example/project".into())),
            ..TemplateFields::default()
        },
    );
    assert_eq!(
        fixture.apply(&mut draft, &[repo]).unwrap_err(),
        "Template \"repo\" was not applied: its destination is a GitHub checkout, which this launcher cannot open yet"
    );
    assert_eq!(draft, before);
}

// ---------------------------------------------------------------------------------------------------------------------
// Held values: Replace with and Restart with
// ---------------------------------------------------------------------------------------------------------------------

/// Replace with holds the host. A template naming the same host applies its other fields with the host left alone; one
/// naming another host is refused (SPEC.md: a template "that changes a field the current launcher mode holds fixed,
/// such as the host in Replace with").
#[test]
fn replace_with_drops_an_equal_host_and_refuses_another() {
    let fixture = Fixture::new();
    let session = session(SessionLaunch::Command(CommandLaunch {
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

/// A held field a template would actually change is refused in template order, naming the field, and nothing
/// applies. Initial prompts are always refused in Restart with.
#[test]
fn restart_with_refuses_a_change_to_a_held_field() {
    let fixture = Fixture::new();
    let mut draft = restart_draft();
    let before = draft.clone();
    let claude = template(
        "claude",
        TemplateFields {
            agent: Some(LaunchHarness::Claude),
            ..TemplateFields::default()
        },
    );
    assert_eq!(
        fixture.apply(&mut draft, &[claude]).unwrap_err(),
        "Template \"claude\" was not applied: its agent type cannot change here: this dialog keeps agent type fixed \
         for restart"
    );
    let prompt = template(
        "prompt",
        TemplateFields {
            prompt: Some("hello".into()),
            ..TemplateFields::default()
        },
    );
    assert!(
        fixture
            .apply(&mut draft, &[prompt])
            .unwrap_err()
            .contains("its initial prompt cannot change here")
    );
    assert_eq!(draft, before);
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
        Applied::default(),
        "nothing to show, and the notice is left as it is"
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

//! Tests of the launcher's draft model, without a window.
//!
//! The oracle is SPEC.md ("Creation", "Lifecycle operations") and the old app's own draft tests at reference commit
//! `24bc0580` (`create_form/draft.rs`, `launch_composer.rs`), where they tested behavior rather than the old form's
//! wiring.

use farhelm_proto::launcher::LaunchCatalogModel;
use farhelm_proto::ui_channel::SessionRow;
use farhelm_proto::{
    CommandLaunch, LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, RestartOffer,
    SessionInfo, SessionLaunch,
};

use super::draft::{
    DestinationKind, Field, FolderDefault, Install, LaunchDraft, Mode, RestartBaseline, Tab,
    remembered_permission,
};

/// A small catalog: one Codex model with two efforts, one Claude model with one.
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

fn new_draft() -> LaunchDraft {
    LaunchDraft::new(Some(1), "~".into(), None)
}

/// A session as the helm serves it (its pinned list fixture), with `launch` as its stored launch. Shared with the
/// request tests.
pub(super) fn session(launch: SessionLaunch) -> SessionInfo {
    let row: SessionRow =
        serde_json::from_value(crate::test_support::session_entry("s1").value).unwrap();
    let mut info = row.info;
    info.cwd = "/src/project".into();
    info.title = "source".into();
    info.invocation = "source-command --flag".into();
    info.launch = launch;
    // An ordinary folder session: the helm fixture's row is in a managed checkout, which a Clone would start a new
    // checkout of (the managed-checkout tests give their sources one of their own).
    info.working_copy = None;
    info.github_repo = None;
    info
}

/// An agent launch of `selection`, with a stored resume command when `resume`. Shared with the request tests.
pub(super) fn agent_launch(selection: LaunchSelection, resume: bool) -> SessionLaunch {
    SessionLaunch::Agent {
        selection,
        start: vec!["agent".into(), "{farhelm_args}".into()],
        resume: resume.then(|| {
            vec![
                "agent".into(),
                "{conversation}".into(),
                "{farhelm_args}".into(),
            ]
        }),
    }
}

fn selection(harness: LaunchHarness) -> LaunchSelection {
    LaunchSelection {
        harness,
        model: None,
        effort: None,
        permissions: None,
        permission_flags: None,
        workspace_trust: None,
        wrapper_prefix: None,
        container: None,
    }
}

fn command(agent: Option<LaunchHarness>, resume: Option<&str>) -> CommandLaunch {
    CommandLaunch {
        command: "my-agent {farhelm_args}".into(),
        yolo: false,
        agent,
        resume: resume.map(str::to_owned),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Tabs and New
// ---------------------------------------------------------------------------------------------------------------------

/// SPEC.md: "switching tabs keeps each tab's draft". Each tab's fields survive a visit to the other, and neither is
/// copied into the other (old `tabs_retain_independent_requests_and_one_time_inputs`).
#[test]
fn each_tab_keeps_its_own_draft_across_switches() {
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, None, None);
    draft.edit_prompt("hello".into());
    draft.choose_tab(Tab::Command);
    draft.edit_command("run-it".into());
    draft.choose_command_yolo(true);
    draft.choose_tab(Tab::Agent);
    assert_eq!(draft.agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(draft.prompt, "hello");
    assert_eq!(draft.command.command, "run-it");
    assert_eq!(draft.command.yolo, Some(true));
}

/// New opens on the Agent tab with no agent type, and the remembered permission is a passive default, never a
/// deliberate choice (SPEC.md: "New normally preselects no harness or model"). Custom is never remembered.
#[test]
fn new_starts_with_no_agent_and_a_passive_remembered_permission() {
    let draft = LaunchDraft::new(Some(1), "~".into(), remembered_permission(Some("yolo")));
    assert_eq!(draft.mode, Mode::New);
    assert_eq!(draft.tab, Tab::Agent);
    assert_eq!(draft.agent.harness, None);
    assert_eq!(draft.agent.permissions, Some(LaunchPermission::Yolo));
    assert!(!draft.agent.permissions_explicit);
    assert_eq!(remembered_permission(Some("custom")), None);
    assert_eq!(
        remembered_permission(Some("smart_approve")),
        Some(LaunchPermission::SmartApprove)
    );
    assert_eq!(
        remembered_permission(Some("a-word-from-a-newer-helm")),
        None
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Agent switches and the escalation banner
// ---------------------------------------------------------------------------------------------------------------------

/// The red banner (SPEC.md, "Creation") appears when an agent switch makes the shown permission more dangerous:
/// Codex's Guarded becomes Pi's only mode, YOLO. It names both modes and the agent, and its Undo is the whole draft
/// from before the switch.
#[test]
fn switching_to_an_agent_without_the_shown_mode_raises_the_banner() {
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, None, None);
    let before = draft.clone();
    let edit = draft.choose_harness(LaunchHarness::Pi, None, None);
    let escalation = edit.escalation.expect("Guarded to YOLO is an escalation");
    assert_eq!(escalation.before, Some(LaunchPermission::Guarded));
    assert_eq!(escalation.after, Some(LaunchPermission::Yolo));
    assert_eq!(
        escalation.message(),
        "Permission changed: Guarded → YOLO, because Pi does not support Guarded."
    );
    assert_eq!(*escalation.undo, before);
}

/// Only a move to a later safety group raises the banner: between two agents that both default to Guarded nothing
/// is raised, but the edit still says it switched agents, which is what tells the view to take down a banner a
/// previous switch raised. Choosing the same agent again is no switch.
#[test]
fn an_equally_safe_switch_raises_no_banner_but_reports_the_switch() {
    let mut draft = new_draft();
    assert!(
        draft
            .choose_harness(LaunchHarness::Codex, None, None)
            .switched_agent
    );
    let edit = draft.choose_harness(LaunchHarness::Claude, None, None);
    assert_eq!(edit.escalation, None);
    assert!(edit.switched_agent);
    assert!(
        !draft
            .choose_harness(LaunchHarness::Claude, None, None)
            .switched_agent
    );
}

/// A model of another agent, picked from the full list, switches agents with the same banner check as a direct
/// switch: Codex's Guarded becomes YOLO under Goose, whose only default is YOLO.
#[test]
fn a_cross_agent_model_pick_can_raise_the_banner() {
    let catalog = vec![LaunchCatalogModel {
        id: "z-ai/glm-5.3".into(),
        harness: LaunchHarness::Goose,
        efforts: vec![],
    }];
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, Some(&catalog), None);
    let edit =
        draft.choose_catalog_model(LaunchHarness::Goose, "z-ai/glm-5.3", Some(&catalog), None);
    assert_eq!(draft.agent.harness, Some(LaunchHarness::Goose));
    assert_eq!(draft.agent.model.as_deref(), Some("z-ai/glm-5.3"));
    assert_eq!(
        edit.escalation.map(|escalation| escalation.after),
        Some(Some(LaunchPermission::Yolo))
    );
}

/// Choosing the first agent raises the banner only over a permission chosen deliberately before it: a remembered
/// value alone does not (SPEC.md: "a displayed default or a remembered permission alone does not").
#[test]
fn the_first_agent_escalates_only_a_deliberate_choice() {
    let mut remembered = LaunchDraft::new(Some(1), "~".into(), Some(LaunchPermission::Guarded));
    let edit = remembered.choose_harness(LaunchHarness::Cursor, None, None);
    assert_eq!(edit.escalation, None);
    assert_eq!(
        edit.notice, None,
        "a remembered permission is no deliberate choice to report as cleared"
    );

    let mut chosen = new_draft();
    chosen.choose_permissions(Some(LaunchPermission::Guarded));
    let edit = chosen.choose_harness(LaunchHarness::Cursor, None, None);
    let escalation = edit
        .escalation
        .expect("a deliberate Guarded becomes Cursor's own setting");
    assert_eq!(escalation.after, None, "Cursor's own setting");
    assert_eq!(
        edit.notice.as_deref(),
        Some("The selected permission is unavailable for this harness, so it was cleared"),
        "the first agent still says what it did to a deliberate choice"
    );
}

/// Undo of the first agent's warning brings back the deliberate choice itself, not only its look: no agent and Guarded
/// chosen, so choosing the agent again warns again. With nothing chosen, the displayed default is no choice, and the
/// first agent warns about nothing. Why: the old `permission-switch` spec switched a second time after Undo because an
/// Undo that restored only the shown mode would let the next switch reach YOLO silently, and it checked the passive
/// default beside the passive remembered mode that `the_first_agent_escalates_only_a_deliberate_choice` covers.
#[test]
fn undo_of_the_first_agents_warning_keeps_the_choice_deliberate() {
    let guarded_to_yolo = Some((
        Some(LaunchPermission::Guarded),
        Some(LaunchPermission::Yolo),
    ));
    let mut draft = new_draft();
    draft.choose_permissions(Some(LaunchPermission::Guarded));
    let escalation = draft
        .choose_harness(LaunchHarness::OpenCode, None, None)
        .escalation
        .expect("a deliberate Guarded becomes OpenCode's YOLO");
    assert_eq!(Some((escalation.before, escalation.after)), guarded_to_yolo);
    let mut undone = *escalation.undo;
    assert_eq!(
        (undone.agent.harness, undone.agent.permissions),
        (None, Some(LaunchPermission::Guarded))
    );
    let again = undone.choose_harness(LaunchHarness::OpenCode, None, None);
    assert_eq!(
        again
            .escalation
            .map(|escalation| (escalation.before, escalation.after)),
        guarded_to_yolo,
        "the second switch warns again"
    );

    let mut passive = new_draft();
    let edit = passive.choose_harness(LaunchHarness::OpenCode, None, None);
    assert_eq!(
        (passive.agent.harness, passive.agent.permissions),
        (Some(LaunchHarness::OpenCode), Some(LaunchPermission::Yolo)),
        "the premise: the switch happened and shows YOLO"
    );
    assert_eq!(edit.escalation, None, "a displayed default is no choice");
}

/// An agent chosen while the Command tab is showing switches to the Agent tab, and raises no banner: the user was not
/// looking at the permission it changed.
#[test]
fn a_switch_from_the_command_tab_raises_no_banner() {
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, None, None);
    draft.choose_tab(Tab::Command);
    let edit = draft.choose_harness(LaunchHarness::Pi, None, None);
    assert_eq!(draft.tab, Tab::Agent);
    assert_eq!(edit.escalation, None);
}

/// An agent switch clears the model the new agent does not offer and says so, as the old reconciliation notice did;
/// without a catalog read, an effort the new agent can take is kept (a missing catalog never makes a choice
/// incompatible).
#[test]
fn a_switch_clears_what_the_new_agent_cannot_use_and_says_so() {
    let catalog = catalog();
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, Some(&catalog), None);
    draft.choose_catalog_model(LaunchHarness::Codex, "gpt-6-luna", Some(&catalog), None);
    let edit = draft.choose_harness(LaunchHarness::Claude, Some(&catalog), None);
    assert_eq!(draft.agent.model, None);
    assert_eq!(
        edit.notice.as_deref(),
        Some("The selected model is not in this harness's Farhelm offering, so it was cleared")
    );

    let mut unread = new_draft();
    unread.choose_harness(LaunchHarness::Codex, None, None);
    unread.choose_effort(Some(LaunchEffort::Low));
    unread.choose_harness(LaunchHarness::Claude, None, None);
    assert_eq!(unread.agent.effort, Some(LaunchEffort::Low));
}

/// The permission an agent switch lands on, and what it says about it: a mode the new agent lacks becomes its default,
/// said only when the user chose that mode (the old spec: "normalizing a passive remembered mode must stay silent"),
/// and a model picked from another agent's list switches agents the same way; a model id both Goose and Pi list stays
/// the one picked and takes Pi's efforts, which stay across Pi's models that both offer them. Why: the old app's
/// `goose-pi-composer` spec drove these transitions in the rendered launcher because its handlers once applied the
/// reconciled model and effort but dropped the reconciled permission, so the launcher showed one mode and sent another.
#[test]
fn an_agent_switch_lands_on_a_permission_the_new_agent_offers() {
    use LaunchHarness::{Goose, Pi};
    use LaunchPermission::{Approve, Chat, SmartApprove, Yolo};
    const REPLACED: &str = "The selected permission is unavailable for this harness, so it was replaced by YOLO, this harness's default";
    let pi_model = |id: &str| LaunchCatalogModel {
        id: id.into(),
        harness: Pi,
        efforts: vec![LaunchEffort::Minimal, LaunchEffort::Xhigh],
    };
    // Goose lists the same ids first, with an effort Pi's rows lack, so only a lookup by the owning agent finds Pi's.
    let goose_model = |id: &str| LaunchCatalogModel {
        id: id.into(),
        harness: Goose,
        efforts: vec![LaunchEffort::High],
    };
    let catalog = vec![
        goose_model("x-ai/grok-4.6"),
        goose_model("z-ai/glm-5.3-flash"),
        pi_model("x-ai/grok-4.6"),
        pi_model("z-ai/glm-5.3-flash"),
    ];
    // A mode Goose offers and Pi lacks, chosen on Goose, becomes Pi's YOLO and is said, whether Pi is chosen by its
    // button or by its model picked from the full list.
    for (chosen, by_model) in [(SmartApprove, false), (Chat, false), (Approve, true)] {
        let mut draft = new_draft();
        draft.choose_harness(Goose, Some(&catalog), None);
        draft.choose_permissions(Some(chosen));
        let edit = if by_model {
            draft.choose_catalog_model(Pi, "x-ai/grok-4.6", Some(&catalog), None)
        } else {
            draft.choose_harness(Pi, Some(&catalog), None)
        };
        let case = format!("{chosen:?}, by model: {by_model}");
        assert_eq!(
            (draft.agent.harness, draft.agent.permissions),
            (Some(Pi), Some(Yolo)),
            "{case}"
        );
        assert_eq!(edit.notice.as_deref(), Some(REPLACED), "{case}");
        if by_model {
            assert_eq!(
                draft.agent.model.as_deref(),
                Some("x-ai/grok-4.6"),
                "the id both list stays the one picked, as Pi's"
            );
        }
    }

    // A remembered mode is no choice of the user's, so Pi replaces it silently.
    let mut remembered = LaunchDraft::new(Some(1), "~".into(), Some(SmartApprove));
    let edit = remembered.choose_harness(Pi, Some(&catalog), None);
    assert_eq!(remembered.agent.permissions, Some(Yolo));
    assert_eq!(edit.notice, None);

    // An effort both Pi models offer stays when the other one is picked: Pi's efforts, not Goose's for the same id.
    for effort in [LaunchEffort::Minimal, LaunchEffort::Xhigh] {
        let mut pi = new_draft();
        pi.choose_harness(Pi, Some(&catalog), None);
        pi.choose_catalog_model(Pi, "x-ai/grok-4.6", Some(&catalog), None);
        pi.choose_effort(Some(effort));
        let edit = pi.choose_catalog_model(Pi, "z-ai/glm-5.3-flash", Some(&catalog), None);
        assert_eq!(pi.agent.effort, Some(effort));
        assert_eq!(edit.notice, None);
    }
}

/// An agent's default YOLO is the agent's, not the user's: switching from Pi, OpenCode or Goose to Codex, by its button
/// or by a Codex model picked from the full list, shows Codex's own default, Guarded, and says nothing. Why: the old
/// app tested both paths (`remembered-permissions`, "default YOLO does not spill into the next harness", and the
/// sidebar spec's "model picks discard another harness's default YOLO"), since carrying the YOLO over would start the
/// next agent unguarded without the user choosing that.
#[test]
fn a_default_yolo_does_not_follow_to_the_next_agent() {
    let catalog = catalog();
    for first in [
        LaunchHarness::Pi,
        LaunchHarness::OpenCode,
        LaunchHarness::Goose,
    ] {
        for by_model in [false, true] {
            let mut draft = new_draft();
            draft.choose_harness(first, Some(&catalog), None);
            assert_eq!(
                draft.agent.permissions,
                Some(LaunchPermission::Yolo),
                "the premise: {first:?} shows its default YOLO"
            );
            let edit = if by_model {
                draft.choose_catalog_model(LaunchHarness::Codex, "gpt-6-luna", Some(&catalog), None)
            } else {
                draft.choose_harness(LaunchHarness::Codex, Some(&catalog), None)
            };
            assert_eq!(
                (draft.agent.harness, draft.agent.permissions),
                (Some(LaunchHarness::Codex), Some(LaunchPermission::Guarded)),
                "from {first:?}, by model: {by_model}"
            );
            assert_eq!(edit.notice, None, "from {first:?}, by model: {by_model}");
            if by_model {
                assert_eq!(
                    draft.agent.model.as_deref(),
                    Some("gpt-6-luna"),
                    "the pick is kept"
                );
            }
        }
    }
}

/// Switching agent types loads the new type's remembered wrapper prefix; choosing the same type again keeps whatever
/// the user typed (SPEC.md: "Switching agent types loads the new type's remembered prefix").
#[test]
fn an_agent_switch_loads_its_remembered_wrapper() {
    let remembered = vec!["nice".to_owned(), "-n".to_owned(), "5".to_owned()];
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, None, Some(&remembered));
    assert_eq!(draft.agent.wrapper, "nice -n 5");
    draft.edit_wrapper("typed".into());
    draft.choose_harness(LaunchHarness::Codex, None, Some(&remembered));
    assert_eq!(draft.agent.wrapper, "typed");
    draft.choose_harness(LaunchHarness::Claude, None, None);
    assert_eq!(draft.agent.wrapper, "");
}

/// Custom flags belong to the agent they were written for: switching agents drops them, and so does choosing another
/// permission, while their text is kept as typed even when it does not parse (SPEC.md: "Changing agent type clears
/// custom and its words").
#[test]
fn custom_flags_stay_with_their_agent_and_their_choice() {
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, None, None);
    draft.choose_permissions(Some(LaunchPermission::Custom));
    draft.edit_permission_flags("--sandbox 'unclosed".into());
    assert_eq!(draft.agent.permission_flags, "--sandbox 'unclosed");
    draft.choose_harness(LaunchHarness::Claude, None, None);
    assert_ne!(draft.agent.permissions, Some(LaunchPermission::Custom));
    assert_eq!(draft.agent.permission_flags, "");

    draft.choose_permissions(Some(LaunchPermission::Custom));
    draft.edit_permission_flags("--yes".into());
    draft.choose_permissions(Some(LaunchPermission::Guarded));
    assert_eq!(draft.agent.permission_flags, "");
}

/// Within one agent type a catalog model clears an effort it does not offer; a model of another type, picked from the
/// full list, switches the type on purpose (SPEC.md). A typed custom id never switches the type and is owned by it, so
/// a later switch clears it.
#[test]
fn model_choices_follow_their_owning_agent() {
    let catalog = catalog();
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, Some(&catalog), None);
    draft.choose_catalog_model(LaunchHarness::Codex, "gpt-6-luna", Some(&catalog), None);
    draft.choose_effort(Some(LaunchEffort::Low));
    let edit = draft.choose_catalog_model(
        LaunchHarness::Claude,
        "claude-fable-5",
        Some(&catalog),
        None,
    );
    assert_eq!(draft.agent.harness, Some(LaunchHarness::Claude));
    assert_eq!(draft.agent.model.as_deref(), Some("claude-fable-5"));
    assert_eq!(draft.agent.effort, None, "Claude's model offers only High");
    assert_eq!(
        edit.notice.as_deref(),
        Some(
            "The selected effort is not in Farhelm's offering for that model or harness, so it was cleared"
        ),
        "the picked model is kept, so only the effort is reported"
    );

    let same = draft.choose_catalog_model(
        LaunchHarness::Claude,
        "claude-fable-5",
        Some(&catalog),
        None,
    );
    assert!(!same.switched_agent);
    let mut codex = new_draft();
    codex.choose_harness(LaunchHarness::Codex, Some(&catalog), None);
    codex.choose_effort(Some(LaunchEffort::Medium));
    let pick = codex.choose_catalog_model(LaunchHarness::Codex, "gpt-6-luna", Some(&catalog), None);
    assert_eq!(
        codex.agent.effort, None,
        "gpt-6-luna offers only Low and High"
    );
    assert_eq!(
        pick.notice.as_deref(),
        Some("The selected effort is not in Farhelm's offering for that model, so it was cleared")
    );
    draft.choose_effort(Some(LaunchEffort::High));
    draft.choose_default_model();
    assert_eq!(
        draft.agent.effort,
        Some(LaunchEffort::High),
        "the default model keeps the effort"
    );

    draft.choose_custom_model("my-own-model", Some(&catalog));
    assert_eq!(draft.agent.harness, Some(LaunchHarness::Claude));
    assert_eq!(draft.agent.model_owner, Some(LaunchHarness::Claude));
    draft.choose_harness(LaunchHarness::Codex, Some(&catalog), None);
    assert_eq!(
        draft.agent.model, None,
        "a custom id belongs to the agent it was typed for"
    );

    let mut no_agent = new_draft();
    no_agent.choose_custom_model("my-own-model", Some(&catalog));
    assert_eq!(
        no_agent.agent.model, None,
        "a custom id needs an agent to own it"
    );
}

/// Reset choices returns New's agent choices with the remembered permission, keeping the command tab, the one-time
/// inputs and the destination.
#[test]
fn reset_choices_returns_to_the_remembered_permission() {
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, None, None);
    draft.choose_permissions(Some(LaunchPermission::Yolo));
    draft.edit_wrapper("nice".into());
    draft.edit_prompt("hello".into());
    draft.edit_command("run-it".into());
    draft.reset_choices(Some(LaunchPermission::Guarded));
    assert_eq!(draft.agent.harness, None);
    assert_eq!(draft.agent.permissions, Some(LaunchPermission::Guarded));
    assert!(!draft.agent.permissions_explicit);
    assert_eq!(draft.agent.wrapper, "");
    assert_eq!(draft.prompt, "hello");
    assert_eq!(draft.command.command, "run-it");
}

// ---------------------------------------------------------------------------------------------------------------------
// Destination
// ---------------------------------------------------------------------------------------------------------------------

/// SPEC.md, "Working directory": "Changing the host re-seeds a folder the user has not typed or picked in that draft".
#[test]
fn a_host_change_reseeds_only_an_untouched_folder() {
    let mut draft = new_draft();
    draft.choose_host(2, "/home/on-two".into());
    assert_eq!(draft.host, Some(2));
    assert_eq!(draft.folder.value(), "/home/on-two");

    draft.edit_folder("/picked".into());
    draft.choose_host(3, "/home/on-three".into());
    assert_eq!(draft.folder.value(), "/picked");

    let mut typed = new_draft();
    typed.edit_folder("/typed".into());
    typed.choose_host(2, "/home/on-two".into());
    assert_eq!(typed.folder.value(), "/typed");

    // A Clone's folder is the source's, untouched; choosing the host it already has must not replace it.
    let mut clone = LaunchDraft::clone_of(
        &session(agent_launch(selection(LaunchHarness::Codex), true)),
        Some(4),
    );
    clone.choose_host(4, "~".into());
    assert_eq!(clone.folder.value(), "/src/project");
    clone.seed_folder("~/later".into());
    assert_eq!(
        clone.folder.value(),
        "~/later",
        "a later default seeds an untouched folder"
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Seeding from a session, and held fields
// ---------------------------------------------------------------------------------------------------------------------

/// Clone carries the source's stored launch verbatim, on the Agent tab for an agent launch, as deliberate choices,
/// with its folder and title untouched and the prompt and conversation empty (SPEC.md, "Clone").
#[test]
fn clone_seeds_an_agent_launch_verbatim() {
    let mut stored = selection(LaunchHarness::Codex);
    stored.model = Some("gpt-6-luna".into());
    stored.effort = Some(LaunchEffort::High);
    stored.permissions = Some(LaunchPermission::Custom);
    stored.permission_flags = Some(vec!["--a".into(), "b c".into()]);
    stored.wrapper_prefix = Some(vec!["nice".into(), "-n".into(), "5".into()]);
    let draft = LaunchDraft::clone_of(&session(agent_launch(stored, true)), Some(4));
    assert_eq!(draft.mode, Mode::Clone);
    assert_eq!(draft.tab, Tab::Agent);
    assert_eq!(draft.host, Some(4));
    assert_eq!(draft.agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(draft.agent.model.as_deref(), Some("gpt-6-luna"));
    assert_eq!(draft.agent.effort, Some(LaunchEffort::High));
    assert_eq!(draft.agent.permissions, Some(LaunchPermission::Custom));
    assert!(draft.agent.permissions_explicit);
    assert_eq!(
        draft.agent.permission_flags, "--a 'b c'",
        "literal words quoted back"
    );
    assert_eq!(draft.agent.wrapper, "nice -n 5");
    assert_eq!(draft.folder.value(), "/src/project");
    assert!(!draft.folder.is_touched());
    assert_eq!(draft.name.value(), "source");
    assert!(draft.prompt.is_empty() && draft.conversation.is_empty());
    assert_eq!(
        draft.command.command, "source-command --flag",
        "the Command tab holds the session's command line"
    );
    assert_eq!(draft.command.yolo, None);
}

/// Replace with seeds like Clone, from the source's folder, title and launch, on the source's host.
#[test]
fn replace_with_seeds_from_its_source() {
    let draft = LaunchDraft::replace_with(
        "s1",
        &session(agent_launch(selection(LaunchHarness::Codex), true)),
        Some(4),
    );
    assert_eq!(
        draft.mode,
        Mode::ReplaceWith {
            source: "s1".into()
        }
    );
    assert_eq!(draft.host, Some(4));
    assert_eq!(draft.agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(
        draft.agent.permissions,
        Some(LaunchPermission::Guarded),
        "Codex's omitted permission, resolved"
    );
    assert_eq!(draft.folder.value(), "/src/project");
}

/// Restart with holds the agent type and the launch kind: a switch to another agent, directly or through another
/// agent's model, is refused with the old app's words and changes nothing.
#[test]
fn restart_with_refuses_to_switch_agents() {
    let mut stored = selection(LaunchHarness::Codex);
    stored.model = Some("my-own-model".into());
    let source = session(agent_launch(stored, true));
    let mut draft = LaunchDraft::restart_with("s1", &source, Some(1)).unwrap();
    let before = draft.clone();
    let edit = draft.choose_harness(LaunchHarness::Claude, None, None);
    assert_eq!(
        edit.notice.as_deref(),
        Some("Agent type cannot change here: this dialog keeps the session's agent type")
    );
    assert!(!edit.switched_agent);
    let catalog = catalog();
    draft.choose_catalog_model(
        LaunchHarness::Claude,
        "claude-fable-5",
        Some(&catalog),
        None,
    );
    assert_eq!(
        draft, before,
        "a refused pick leaves even the model's owner as it was"
    );
    draft.choose_tab(Tab::Command);
    assert_eq!(draft.tab, Tab::Agent, "Restart with holds the launch kind");
}

/// The banner's Undo restores the draft exactly as it was before the switch, the custom model's owner included, so a
/// later agent switch clears that model as it would have without the detour (SPEC.md: "Undo restores the draft before
/// the switch").
#[test]
fn undo_after_a_cross_agent_model_pick_restores_the_draft_exactly() {
    let catalog = vec![LaunchCatalogModel {
        id: "z-ai/glm-5.3".into(),
        harness: LaunchHarness::Goose,
        efforts: vec![],
    }];
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, Some(&catalog), None);
    draft.choose_custom_model("my-codex-model", Some(&catalog));
    let before = draft.clone();
    let escalation = draft
        .choose_catalog_model(LaunchHarness::Goose, "z-ai/glm-5.3", Some(&catalog), None)
        .escalation
        .expect("Guarded to YOLO");
    assert_eq!(*escalation.undo, before);
    let mut undone = *escalation.undo;
    undone.choose_harness(LaunchHarness::Claude, Some(&catalog), None);
    assert_eq!(
        undone.agent.model, None,
        "the Codex custom model does not follow to Claude"
    );
}

/// A command launch seeds the Command tab with its command, its YOLO answer, its declared agent and its resume
/// command; a launch from before launch kinds carries only its stored command, with the YOLO question unanswered.
#[test]
fn clone_seeds_command_and_legacy_launches_on_the_command_tab() {
    let draft = LaunchDraft::clone_of(
        &session(SessionLaunch::Command(command(
            Some(LaunchHarness::Claude),
            Some("resume {conversation}"),
        ))),
        Some(1),
    );
    assert_eq!(draft.tab, Tab::Command);
    assert_eq!(draft.command.command, "my-agent {farhelm_args}");
    assert_eq!(draft.command.yolo, Some(false));
    assert_eq!(draft.agent.harness, None, "the Agent tab stays empty");
    assert!(
        draft.agent.permissions_explicit,
        "nothing passive to escalate from on its Agent tab"
    );
    assert_eq!(draft.command.agent, Some(LaunchHarness::Claude));
    assert!(draft.command.resume_on);
    assert_eq!(draft.command.resume, "resume {conversation}");

    let legacy = LaunchDraft::clone_of(
        &session(SessionLaunch::Legacy {
            invocation: "old-agent --flag".into(),
            agent_kind: farhelm_proto::AgentKind::Generic,
            resume_template: None,
        }),
        Some(1),
    );
    assert_eq!(legacy.tab, Tab::Command);
    assert_eq!(legacy.command.command, "old-agent --flag");
    assert_eq!(legacy.command.yolo, None);
    assert!(legacy.agent.permissions_explicit);
}

/// Which fields each mode holds fixed (SPEC.md, "Restart with" and "Replace with"): New and Clone none, Replace with
/// the host, Restart with launch kind, agent type, host, destination, name and prompt.
#[test]
fn each_mode_holds_its_own_fields() {
    let source = session(agent_launch(selection(LaunchHarness::Codex), true));
    let fields = [
        Field::LaunchKind,
        Field::AgentType,
        Field::Host,
        Field::Destination,
        Field::Name,
        Field::InitialPrompt,
    ];
    let clone = LaunchDraft::clone_of(&source, Some(1));
    assert!(fields.iter().all(|field| !clone.holds(*field)));
    assert!(fields.iter().all(|field| !new_draft().holds(*field)));
    let replace = LaunchDraft::replace_with("s1", &source, Some(1));
    assert_eq!(
        fields
            .iter()
            .filter(|field| replace.holds(**field))
            .collect::<Vec<_>>(),
        [&Field::Host]
    );
    let restart = LaunchDraft::restart_with("s1", &source, Some(1))
        .expect("an agent launch with a resume command");
    assert!(fields.iter().all(|field| restart.holds(*field)));
}

/// Restart with is offered for an agent launch with a stored resume command, and for a command launch that declared an
/// agent type and opted into Resume; not for Cursor or OpenCode (no stored resume command), a command launch without
/// Resume, or a launch from before launch kinds (SPEC.md, "Restart with").
#[test]
fn restart_with_is_offered_only_for_a_resumable_launch() {
    assert!(RestartBaseline::of(&agent_launch(selection(LaunchHarness::Codex), true)).is_some());
    assert!(RestartBaseline::of(&agent_launch(selection(LaunchHarness::Cursor), false)).is_none());
    assert!(
        RestartBaseline::of(&SessionLaunch::Command(command(
            Some(LaunchHarness::Claude),
            Some("r")
        )))
        .is_some()
    );
    assert!(
        RestartBaseline::of(&SessionLaunch::Command(command(
            Some(LaunchHarness::Claude),
            None
        )))
        .is_none()
    );
    assert!(RestartBaseline::of(&SessionLaunch::Command(command(None, Some("r")))).is_none());
    assert!(
        RestartBaseline::of(&SessionLaunch::Legacy {
            invocation: "x".into(),
            agent_kind: farhelm_proto::AgentKind::Generic,
            resume_template: Some(vec!["x".into()]),
        })
        .is_none()
    );
}

/// Restart with shows the permission the stored command was built with, so a Claude session stored before guarded
/// flags stays "Agent's own setting" rather than today's Guarded default; a session with no captured conversation
/// must name one; Reset choices returns to the source's own launch.
#[test]
fn restart_with_shows_the_stored_permission_and_resets_to_it() {
    let mut source = session(agent_launch(selection(LaunchHarness::Claude), true));
    source.restart_offer = RestartOffer::default();
    let mut draft = LaunchDraft::restart_with("s1", &source, Some(1)).unwrap();
    assert_eq!(
        draft.agent.permissions, None,
        "the agent's own setting, as stored"
    );
    assert!(matches!(&draft.mode, Mode::RestartWith(restart) if restart.conversation_required));

    draft.choose_permissions(Some(LaunchPermission::Yolo));
    draft.edit_conversation("abc".into());
    draft.reset_choices(Some(LaunchPermission::Guarded));
    assert_eq!(draft.agent.permissions, None);
    assert_eq!(draft.conversation, "");

    source.restart_offer = RestartOffer::Resume;
    let captured = LaunchDraft::restart_with("s1", &source, Some(1)).unwrap();
    assert!(matches!(&captured.mode, Mode::RestartWith(restart) if !restart.conversation_required));
}

/// A recent setup's catalog model is the catalog's, and any other model is a custom one its agent owns, as if typed;
/// with no catalog yet, every model counts as custom. The setup's choices land on the Agent tab as deliberate ones.
/// Why: a later agent switch clears a custom model by its owner (the shared reconciliation), so a catalog model marked
/// custom would be cleared wrongly, and a custom one left unowned would cross to the next agent (old
/// `custom_model_owner`).
#[test]
fn a_recent_setups_model_is_owned_only_when_custom() {
    let mut catalog_model = selection(LaunchHarness::Codex);
    catalog_model.model = Some("gpt-6-luna".into());
    catalog_model.permissions = Some(LaunchPermission::Yolo);
    let mut draft = new_draft();
    draft.choose_tab(Tab::Command);
    draft.restore_recent(&catalog_model, Some(&catalog()));
    assert_eq!(draft.tab, Tab::Agent);
    assert_eq!(draft.agent.model.as_deref(), Some("gpt-6-luna"));
    assert_eq!(
        draft.agent.model_owner, None,
        "a catalog model is the catalog's"
    );
    assert!(draft.agent.permissions_explicit);

    let mut custom = selection(LaunchHarness::Codex);
    custom.model = Some("my-finetune".into());
    let mut draft = new_draft();
    draft.restore_recent(&custom, Some(&catalog()));
    assert_eq!(
        draft.agent.model_owner,
        Some(LaunchHarness::Codex),
        "a custom model is its agent's"
    );

    let mut draft = new_draft();
    draft.restore_recent(&catalog_model, None);
    assert_eq!(
        draft.agent.model_owner,
        Some(LaunchHarness::Codex),
        "with no catalog yet, every model counts as custom"
    );
}

// ---- New's folder default ------------------------------------------------------------------------------------------

/// The installation the folder-default tests decide for.
fn install() -> Install {
    Install {
        host: 1,
        identity: Some("identity1".into()),
    }
}

/// New as its own opening makes it without the open session's folder: `~`, waiting for the host's default
/// (`entry::new_draft`; its tests show when New opens this way).
fn pending_new() -> LaunchDraft {
    let mut draft = new_draft();
    draft.folder_default = FolderDefault::Pending;
    draft
}

/// New's folder default is decided once: New starts waiting for it with its untouched folder, the first settlement
/// seeds the folder and ends the wait, and a later one changes nothing. A folder the user typed waits for nothing and
/// is kept when the default settles. A read that failed settles on `~`, marked unread. Why: SPEC.md, "Working
/// directory": New seeds the host's last plain folder, and "a refresh of the recent setups never rewrites an open
/// draft's folder" (old `folder_default_follows_precedence_once_per_opening_or_host_change`).
#[test]
fn new_waits_for_its_folder_default_once() {
    let mut draft = pending_new();
    assert_eq!(draft.folder_default, FolderDefault::Pending);
    assert!(draft.folder_default_waiting());
    draft.settle_folder_default(Some("/srv/work".into()), install());
    assert_eq!(draft.folder.value(), "/srv/work");
    assert_eq!(draft.folder_default, FolderDefault::Settled);
    assert!(!draft.folder_default_waiting());
    assert!(
        !draft.folder.is_touched(),
        "a seed is not the user's choice"
    );
    draft.settle_folder_default(Some("/srv/later".into()), install());
    assert_eq!(
        draft.folder.value(),
        "/srv/work",
        "a later answer moves nothing"
    );

    let mut typed = pending_new();
    typed.edit_folder("/mine".into());
    assert!(
        !typed.folder_default_waiting(),
        "a typed folder waits for nothing"
    );
    typed.settle_folder_default(Some("/srv/work".into()), install());
    assert_eq!(typed.folder.value(), "/mine");
    assert_eq!(typed.folder_default, FolderDefault::Settled);

    let mut unread = pending_new();
    unread.settle_folder_default(None, install());
    assert_eq!(unread.folder.value(), "~");
    assert_eq!(unread.folder_default, FolderDefault::Unread);
    assert!(!unread.folder_default_waiting());
}

/// A host change makes the new host's default pending again; picking the host already chosen does not. A Clone
/// starts settled on its source's folder and waits only once the user moves it to another host; Replace with and
/// Restart with never wait. Why: SPEC.md, "Changing the host re-seeds a folder the user has not typed or picked", and
/// Restart with "never uses or updates the per-host folder default" (old `folder_default_follows_precedence_...`'s
/// Clone and source rows).
#[test]
fn a_host_change_rearms_the_folder_default_where_the_destination_is_free() {
    let mut draft = pending_new();
    draft.settle_folder_default(Some("/srv/work".into()), install());
    let host = draft.host.expect("New has a host");
    draft.choose_host(host, "~".into());
    assert_eq!(
        draft.folder_default,
        FolderDefault::Settled,
        "the same host"
    );
    draft.choose_host(host + 1, "~".into());
    assert_eq!(draft.folder_default, FolderDefault::Pending);
    assert_eq!(draft.folder.value(), "~", "the stand-in while it waits");

    let source = session(agent_launch(selection(LaunchHarness::Codex), true));
    let mut clone = LaunchDraft::clone_of(&source, Some(1));
    assert_eq!(clone.folder_default, FolderDefault::Settled);
    assert!(!clone.folder_default_waiting());
    clone.choose_host(2, "~".into());
    assert_eq!(clone.folder_default, FolderDefault::Pending);

    let mut replace = LaunchDraft::replace_with("a", &source, Some(1));
    assert_eq!(replace.folder_default, FolderDefault::Settled);
    replace.choose_host(2, "~".into());
    assert_eq!(
        replace.folder_default,
        FolderDefault::Settled,
        "Replace with holds its host"
    );
    let mut resumable = source.clone();
    resumable.restart_offer = RestartOffer::Resume;
    let mut restart = LaunchDraft::restart_with("a", &resumable, Some(1)).expect("offered");
    restart.choose_host(2, "~".into());
    assert!(
        !restart.folder_default_waiting(),
        "Restart with holds its destination"
    );
}

/// A managed checkout does not wait for the folder default, and a default that settles meanwhile leaves the folder
/// kept aside behind it as it was. Why: the default is a folder's; the old launcher spent it without seeding a folder
/// the managed checkout had put aside.
#[test]
fn a_managed_checkout_neither_waits_nor_is_seeded() {
    let mut draft = pending_new();
    let before = draft.folder.value().clone();
    draft.choose_destination(DestinationKind::Checkout);
    assert!(!draft.folder_default_waiting());
    draft.settle_folder_default(Some("/srv/work".into()), install());
    assert_eq!(draft.folder_default, FolderDefault::Settled);
    draft.choose_destination(DestinationKind::Folder);
    assert_eq!(draft.folder.value(), &before);
}

// ---- A folder's installation ---------------------------------------------------------------------------------------

/// Host row 1 recording installation `identity`, as the registry lists it.
fn host_one(identity: &str) -> farhelm_proto::ui_channel::HostView {
    let mut host = crate::hosts::tests::host(1, "local", crate::hosts::tests::connected());
    host.identity = Some(identity.to_owned());
    host
}

/// A folder taken from host 1's history while it named installation `a` refuses Launch once the row names `b`, or is
/// gone, and an explicit choice of the host, a typed folder, or a managed checkout ends the refusal. Why: SPEC.md,
/// "Working directory": a path on two installations can hold different work, so a folder taken on one must not launch
/// on the other (old `remembered_destination_survives_only_same_install_reconnections` and
/// `destination-authority.spec.ts`, "applied … retains installation authority through Launch").
#[test]
fn a_history_folder_stays_on_its_installation() {
    let mut draft = new_draft();
    draft.take_history_folder("/srv/work".into(), Install::of(&host_one("a")));
    assert_eq!(draft.folder.value(), "/srv/work");
    assert!(
        !draft.folder_installation_moved(&[host_one("a")]),
        "same install"
    );
    assert!(
        draft.folder_installation_moved(&[host_one("b")]),
        "retargeted"
    );
    assert!(draft.folder_installation_moved(&[]), "row gone");

    let mut chosen = draft.clone();
    chosen.choose_host(1, "~".into());
    assert!(
        !chosen.folder_installation_moved(&[host_one("b")]),
        "the host chosen again"
    );
    let mut typed = draft.clone();
    typed.edit_folder("/srv/work".into());
    assert!(
        !typed.folder_installation_moved(&[host_one("b")]),
        "the folder typed"
    );
    let mut checkout = draft.clone();
    checkout.choose_destination(DestinationKind::Checkout);
    assert!(
        !checkout.folder_installation_moved(&[host_one("b")]),
        "a managed checkout"
    );
}

/// New's default decided for installation `a` is decided again once the row names `b`, while the folder is still that
/// untouched default, even after the same host was chosen again; a folder the user typed meanwhile is kept and not
/// re-decided. Why: the default came from the old
/// installation's history and says nothing about the new one (old `folder_default_step`'s target change).
#[test]
fn a_default_from_a_retargeted_installation_is_decided_again() {
    let mut draft = pending_new();
    draft.settle_folder_default(Some("/srv/work".into()), Install::of(&host_one("a")));
    assert!(
        !draft.redecide_moved_default(&[host_one("a")]),
        "same install"
    );
    assert!(
        !draft.folder_installation_moved(&[host_one("b")]),
        "a default is decided again, not refused"
    );
    let mut typed = draft.clone();
    typed.edit_folder("/mine".into());
    assert!(
        !typed.redecide_moved_default(&[host_one("b")]),
        "a typed folder stays"
    );
    // Choosing the same host again, or putting the folder aside behind a managed checkout and back, keeps an untouched
    // default's installation, so a later retarget still decides it.
    draft.choose_host(1, "~".into());
    draft.choose_destination(DestinationKind::Checkout);
    draft.choose_repository(farhelm_proto::GithubRepo {
        owner: "acme".into(),
        name: "bar".into(),
    });
    draft.choose_destination(DestinationKind::Folder);
    assert!(draft.redecide_moved_default(&[host_one("b")]));
    assert_eq!(draft.folder_default, FolderDefault::Pending);
    assert!(draft.folder_default_waiting());
}

// ---------------------------------------------------------------------------------------------------------------------
// The initial prompt across prefills
// ---------------------------------------------------------------------------------------------------------------------

/// SPEC.md keeps the initial prompt a one-time message: recent setups do not carry it, so taking one must leave the
/// message the user typed alone (the old templates spec's "initial prompt survives a recent but clears on Clone and
/// Replace prefills"). Losing the message on a recent would discard typing. The case's other half, a prompt not leaking
/// into the next Clone or Replace with, needs no test here: the old form was one persistent draft that each prefill
/// had to clear, while every GPUI opening builds a new draft from a session, which carries no prompt. Specified: a
/// Codex draft's typed prompt survives `restore_recent` of a Claude setup.
#[test]
fn a_recent_setup_keeps_the_typed_prompt() {
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, Some(&catalog()), None);
    draft.edit_prompt("one-time message".into());
    draft.restore_recent(&selection(LaunchHarness::Claude), Some(&catalog()));
    assert_eq!(
        draft.agent.harness,
        Some(LaunchHarness::Claude),
        "the premise: the recent setup was taken"
    );
    assert_eq!(
        draft.prompt, "one-time message",
        "the recent kept the message"
    );
}

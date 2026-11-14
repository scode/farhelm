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

use super::draft::{Field, LaunchDraft, Mode, RestartBaseline, Tab, remembered_permission};

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

//! Tests of a draft's launch check and request bodies, without a window.
//!
//! The wording oracle is the old app's at reference commit `24bc0580` (`LaunchPrerequisites::refusal` and the
//! submit-time checks in `create_form.rs`); the body oracle is its `create_body` and `restart_request_body` (`api.rs`),
//! less the retry keys the GUI no longer sends.

use farhelm_proto::launcher::LaunchCatalogModel;
use farhelm_proto::ui_channel::{HostStateView, HostView};
use farhelm_proto::{
    CommandLaunch, ContainerConfig, ContainerEngine, DeleteGuard, LaunchEffort, LaunchHarness,
    LaunchPermission, LaunchSelection, RETIRED_CONTAINER_REASON, RestartOffer, SessionInfo,
    SessionLaunch,
};

use super::draft::{LaunchDraft, Tab};
use super::request::Target;

/// Host 1 connected with connection 4, as the helm's pinned host list serves it; host 2 the same row never
/// connected (connection 0) and not connected now.
fn hosts() -> Vec<HostView> {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../farhelm-helm/http-contract/host-list.json"
    ))
    .unwrap();
    let connected: HostView = serde_json::from_value(fixture["hosts"][0].clone()).unwrap();
    let mut never = connected.clone();
    never.id = 2;
    never.incarnation = 0;
    never.state = serde_json::from_value(serde_json::json!({
        "phase": "unreachable-reprobing", "cause": "transport-failure", "last_error": "down"
    }))
    .unwrap();
    // The premise every test here relies on: the fixture's first host is connected on connection 4.
    assert!(matches!(connected.state, HostStateView::Connected { .. }));
    assert_eq!((connected.id, connected.incarnation), (1, 4));
    vec![connected, never]
}

fn catalog() -> Vec<LaunchCatalogModel> {
    vec![LaunchCatalogModel {
        id: "gpt-6-luna".into(),
        harness: LaunchHarness::Codex,
        efforts: vec![LaunchEffort::Low],
    }]
}

fn target<'a>(hosts: &'a [HostView], catalog: Option<&'a [LaunchCatalogModel]>) -> Target<'a> {
    Target { hosts, catalog }
}

fn draft() -> LaunchDraft {
    LaunchDraft::new(Some(1), "~/src".into(), None)
}

fn session(launch: SessionLaunch, offer: RestartOffer) -> SessionInfo {
    let mut info = super::draft_tests::session(launch);
    info.restart_offer = offer;
    info
}

fn agent_launch(selection: LaunchSelection) -> SessionLaunch {
    super::draft_tests::agent_launch(selection, true)
}

fn codex() -> LaunchSelection {
    LaunchSelection {
        harness: LaunchHarness::Codex,
        model: None,
        effort: None,
        permissions: Some(LaunchPermission::Guarded),
        permission_flags: None,
        workspace_trust: None,
        wrapper_prefix: None,
        container: None,
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Why Launch is refused
// ---------------------------------------------------------------------------------------------------------------------

/// The reasons come one at a time, in the old app's order: a connected host first, then the tab's own fields (SPEC.md:
/// "an incomplete attempt says why beside Launch"). A host the store lists but is not connected refuses as one that
/// is gone.
#[test]
fn refusals_name_the_first_thing_to_fix() {
    let hosts = hosts();
    let mut draft = draft();
    draft.host = Some(2);
    assert_eq!(
        draft.refusal(target(&hosts, None), None).as_deref(),
        Some("Choose a connected host before launching")
    );
    draft.host = Some(1);
    assert_eq!(
        draft.refusal(target(&hosts, None), None).as_deref(),
        Some("Choose a structured harness before launching")
    );
    draft.choose_harness(LaunchHarness::Codex, None, None);
    draft.choose_permissions(Some(LaunchPermission::Custom));
    draft.edit_permission_flags("'unclosed".into());
    assert_eq!(
        draft.refusal(target(&hosts, None), None).as_deref(),
        Some("Enter valid custom permission flags before launching")
    );
    draft.choose_permissions(Some(LaunchPermission::Guarded));
    draft.edit_wrapper("'unclosed".into());
    assert_eq!(
        draft.refusal(target(&hosts, None), None).as_deref(),
        Some("Correct the wrapper prefix before launching")
    );
    draft.edit_wrapper(String::new());
    assert_eq!(draft.refusal(target(&hosts, None), None), None);
}

/// The Command tab asks for a command and for the YOLO answer, which has no default (SPEC.md, "Command launch": "a
/// required YOLO assertion"); a placeholder the shared command rule refuses is refused here too, capitalized. A command
/// left on the Command tab does not refuse an agent launch.
#[test]
fn the_command_tab_needs_a_command_and_a_yolo_answer() {
    let hosts = hosts();
    let mut draft = draft();
    draft.choose_tab(Tab::Command);
    assert_eq!(
        draft.refusal(target(&hosts, None), None).as_deref(),
        Some("Enter an agent command before launching")
    );
    draft.edit_command("run '{cwd}'".into());
    assert_eq!(
        draft.refusal(target(&hosts, None), None).as_deref(),
        Some("Say whether this command runs in YOLO mode (YOLO) before launching")
    );
    draft.choose_command_yolo(false);
    let rule = draft
        .command_launch()
        .unwrap()
        .validate()
        .expect_err("a quoted placeholder breaks the shared rule");
    let shown = draft
        .refusal(target(&hosts, None), None)
        .expect("a quoted placeholder is refused");
    assert_eq!(shown.to_lowercase(), rule.to_lowercase());
    assert!(shown.starts_with(char::is_uppercase), "{shown}");

    draft.choose_harness(LaunchHarness::Codex, None, None);
    assert_eq!(draft.tab, Tab::Agent);
    assert_eq!(draft.refusal(target(&hosts, None), None), None);
}

/// A model the catalog says belongs to another agent cannot launch once the catalog has been read; without a catalog
/// read the same choice is not refused, because a slow read says nothing about whether it still fits.
#[test]
fn a_catalog_misfit_refuses_only_against_a_read_catalog() {
    let hosts = hosts();
    let catalog = catalog();
    let mut draft = draft();
    draft.choose_harness(LaunchHarness::Claude, None, None);
    draft.agent.model = Some("gpt-6-luna".into());
    assert_eq!(draft.refusal(target(&hosts, None), None), None);
    assert_eq!(
        draft
            .refusal(target(&hosts, Some(&catalog)), None)
            .as_deref(),
        Some(
            "This saved choice is no longer supported by the current catalog; choose a compatible model or effort"
        )
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Create and Replace with bodies
// ---------------------------------------------------------------------------------------------------------------------

/// An agent launch's create carries the host, the connection the launcher saw, the folder as typed, the choices with
/// an omitted permission resolved to the agent's default, and the prompt and conversation where the agent takes them;
/// a blank name is left out so the helm generates one.
#[test]
fn an_agent_create_body_carries_the_draft() {
    let hosts = hosts();
    let mut draft = draft();
    draft.choose_harness(LaunchHarness::Claude, None, Some(&["nice".to_owned()]));
    draft.choose_effort(Some(LaunchEffort::High));
    draft.edit_prompt("first\nmessage".into());
    draft.edit_conversation("0123".into());
    draft.edit_name("   ".into());
    let body = draft
        .create_body(target(&hosts, None))
        .expect("a complete draft");
    assert_eq!(body.host, Some(1));
    assert_eq!(body.expected_incarnation, Some(4));
    assert_eq!(body.cwd, "~/src");
    assert_eq!(body.title, None);
    assert!(body.command.is_none());
    let launch = body.launch.expect("an agent launch");
    assert_eq!(launch.harness, LaunchHarness::Claude);
    assert_eq!(launch.effort, Some(LaunchEffort::High));
    assert_eq!(
        launch.permissions,
        Some(LaunchPermission::Guarded),
        "Claude's default, resolved"
    );
    assert_eq!(launch.wrapper_prefix, Some(vec!["nice".to_owned()]));
    assert_eq!(body.prompt.as_deref(), Some("first\nmessage"));
    assert_eq!(body.conversation.as_deref(), Some("0123"));
    assert!(!body.confirm_yolo);
}

/// The initial prompt goes only where it can be sent: Goose offers no box, so its draft's dormant prompt stays in the
/// draft and out of the request (SPEC.md: "Goose, Pi and command launches offer no box").
#[test]
fn a_prompt_the_agent_cannot_take_stays_out_of_the_body() {
    let hosts = hosts();
    let mut draft = draft();
    draft.edit_prompt("hello".into());
    draft.choose_harness(LaunchHarness::Goose, None, None);
    let body = draft.create_body(target(&hosts, None)).unwrap();
    assert_eq!(body.prompt, None);
    assert_eq!(draft.prompt, "hello");
}

/// A command launch's create sends the command as written with its YOLO answer and declared agent; the resume command
/// only while Resume is on, and no prompt or conversation. A name is sent byte for byte.
#[test]
fn a_command_create_body_sends_the_command_as_written() {
    let hosts = hosts();
    let mut draft = draft();
    draft.edit_prompt("not for commands".into());
    draft.choose_tab(Tab::Command);
    draft.edit_command("claude {farhelm_args}".into());
    draft.choose_command_yolo(true);
    draft.choose_command_agent(Some(LaunchHarness::Claude));
    draft.edit_command_resume("claude --resume {conversation} {farhelm_args}".into());
    draft.edit_name(" my session ".into());
    let body = draft.create_body(target(&hosts, None)).unwrap();
    assert_eq!(
        body.command,
        Some(CommandLaunch {
            command: "claude {farhelm_args}".into(),
            yolo: true,
            agent: Some(LaunchHarness::Claude),
            resume: None,
        })
    );
    assert_eq!(body.prompt, None);
    assert_eq!(body.title.as_deref(), Some(" my session "));
    draft.choose_command_resume(true);
    let resumed = draft.create_body(target(&hosts, None)).unwrap();
    assert_eq!(
        resumed
            .command
            .and_then(|command| command.resume)
            .as_deref(),
        Some("claude --resume {conversation} {farhelm_args}")
    );
}

/// Replace with sends this draft's create and the delete precondition matching what the launcher showed about the
/// source (SPEC.md, "Replace with"); an incomplete draft sends nothing.
#[test]
fn a_replace_with_body_carries_the_create_and_the_shown_guard() {
    let hosts = hosts();
    let source = session(agent_launch(codex()), RestartOffer::Resume);
    let draft = LaunchDraft::replace_with("s1", &source, Some(1));
    let body = draft
        .replace_body(target(&hosts, None), DeleteGuard::AgentEnded)
        .unwrap();
    assert!(body.only_if_agent_ended && !body.only_if_nothing_alive);
    let nothing_alive = draft
        .replace_body(target(&hosts, None), DeleteGuard::NothingAlive)
        .unwrap();
    assert!(nothing_alive.only_if_nothing_alive && !nothing_alive.only_if_agent_ended);
    assert_eq!(
        body.with
            .and_then(|with| with.launch)
            .map(|launch| launch.harness),
        Some(LaunchHarness::Codex)
    );
    let incomplete = LaunchDraft::new(Some(1), "~".into(), None);
    assert!(
        incomplete
            .replace_body(target(&hosts, None), DeleteGuard::NothingAlive)
            .is_none()
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Restart with
// ---------------------------------------------------------------------------------------------------------------------

/// Restart with's main action stays inactive until a setting changes or a conversation is named, and its request is
/// the edited launch in the baseline's shape (SPEC.md, "Restart with"). Custom flags with spaces round-trip through
/// their editor, so an untouched Custom launch is not taken for an edit.
#[test]
fn restart_with_needs_an_edit_and_sends_it() {
    let hosts = hosts();
    let mut stored = codex();
    stored.permissions = Some(LaunchPermission::Custom);
    stored.permission_flags = Some(vec!["--sandbox".into(), "danger full".into()]);
    let launch = agent_launch(stored);
    let mut draft = LaunchDraft::restart_with(
        "s1",
        &session(launch.clone(), RestartOffer::Resume),
        Some(1),
    )
    .unwrap();
    assert_eq!(
        draft
            .refusal(target(&hosts, None), Some(&launch))
            .as_deref(),
        Some("Change a setting before restarting")
    );
    draft.choose_effort(Some(LaunchEffort::Low));
    let body = draft
        .restart_body(target(&hosts, None), Some(&launch), true)
        .expect("an edit");
    let with = body.with.expect("an agent edit");
    assert_eq!(with.effort, Some(LaunchEffort::Low));
    assert_eq!(
        with.permission_flags,
        Some(vec!["--sandbox".to_owned(), "danger full".to_owned()])
    );
    assert!(body.stop_if_running && body.with_command.is_none());
}

/// A session with no captured conversation must name one to restart; a stored launch that changed since the launcher
/// opened refuses rather than restarting with edits made against the old one.
#[test]
fn restart_with_refuses_a_missing_conversation_and_a_changed_launch() {
    let hosts = hosts();
    let launch = agent_launch(codex());
    let mut draft = LaunchDraft::restart_with(
        "s1",
        &session(launch.clone(), RestartOffer::default()),
        Some(1),
    )
    .unwrap();
    draft.choose_effort(Some(LaunchEffort::Low));
    assert_eq!(
        draft
            .refusal(target(&hosts, None), Some(&launch))
            .as_deref(),
        Some("Enter a conversation to restart")
    );
    draft.edit_conversation("0123".into());
    assert_eq!(draft.refusal(target(&hosts, None), Some(&launch)), None);

    let mut changed = codex();
    changed.model = Some("gpt-6-luna".into());
    assert_eq!(
        draft
            .refusal(target(&hosts, None), Some(&agent_launch(changed)))
            .as_deref(),
        Some("This session's launch settings changed; close and reopen restart with")
    );
}

/// Restart with refuses an edited command the shared rule refuses, but only once something was edited; and its host
/// reasons name the held source host rather than asking for another (the old app's words).
#[test]
fn restart_with_names_its_command_and_host_problems() {
    let hosts = hosts();
    let stored = CommandLaunch {
        command: "claude {farhelm_args}".into(),
        yolo: false,
        agent: Some(LaunchHarness::Claude),
        resume: Some("claude --resume {conversation} {farhelm_args}".into()),
    };
    let launch = SessionLaunch::Command(stored);
    let mut draft = LaunchDraft::restart_with(
        "s1",
        &session(launch.clone(), RestartOffer::Resume),
        Some(1),
    )
    .unwrap();
    assert_eq!(
        draft
            .refusal(target(&hosts, None), Some(&launch))
            .as_deref(),
        Some("Change a setting before restarting")
    );
    draft.edit_command_resume("claude --resume '{conversation}' {farhelm_args}".into());
    assert_eq!(
        draft
            .refusal(target(&hosts, None), Some(&launch))
            .as_deref(),
        Some("Correct the command before restarting")
    );

    draft.host = Some(2);
    assert_eq!(
        draft
            .refusal(target(&hosts, None), Some(&launch))
            .as_deref(),
        Some("The source host is unavailable; wait for it to reconnect before restarting")
    );
    draft.host = Some(99);
    assert_eq!(
        draft
            .refusal(target(&hosts, None), Some(&launch))
            .as_deref(),
        Some("The source host installation cannot be confirmed; close and reopen restart with")
    );
}

/// The initial prompt is checked by the shared rule (control characters other than newline and tab are refused), and
/// an agent that cannot combine a prompt with a named conversation (Muse) refuses the pair rather than dropping one.
#[test]
fn the_initial_prompt_is_checked_before_launch() {
    let hosts = hosts();
    let mut draft = draft();
    draft.choose_harness(LaunchHarness::Muse, None, None);
    draft.edit_prompt("bell\u{7}".into());
    let rule = farhelm_proto::launcher::check_initial_prompt("bell\u{7}").unwrap_err();
    let shown = draft
        .refusal(target(&hosts, None), None)
        .expect("a bell is refused");
    assert_eq!(shown.to_lowercase(), rule.to_lowercase());
    assert!(shown.starts_with(char::is_uppercase), "{shown}");
    draft.edit_prompt("hello".into());
    draft.edit_conversation("0123".into());
    assert_eq!(
        draft.refusal(target(&hosts, None), None).as_deref(),
        Some("This agent cannot combine an initial prompt with a named conversation")
    );
}

/// A session from before containerized sessions were retired supports Delete only (SPEC.md, "Retired containerized
/// sessions"): its Clone keeps the container choice and refuses to launch rather than quietly starting an ordinary
/// session.
#[test]
fn a_retired_container_launch_is_refused() {
    let hosts = hosts();
    let mut stored = codex();
    stored.container = Some(ContainerConfig {
        engine: ContainerEngine::Podman,
        image: "ubuntu".into(),
        api_proxy_port: 8080,
        github_token_file: None,
    });
    let clone = LaunchDraft::clone_of(
        &session(agent_launch(stored), RestartOffer::Resume),
        Some(1),
    );
    assert_eq!(
        clone.refusal(target(&hosts, None), None).as_deref(),
        Some(RETIRED_CONTAINER_REASON)
    );
}

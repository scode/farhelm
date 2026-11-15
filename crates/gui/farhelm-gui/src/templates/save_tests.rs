//! Tests of Save as template: what the launcher offers and checks ([`super::save::Candidate`], without a window), and
//! the panel in the real launcher over the helm client's fake (its footer button, Enter and Escape, the refusals, a
//! refused save, and a saved template opening in the Templates editor).
//!
//! The oracle is SPEC.md, "Launch templates" (the Save as template sentences), the old app's `list/save_template.rs`
//! tests and the Save as template cases of `e2e/tests/templates.spec.ts` at reference commit `24bc0580`.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::launcher::{
    LaunchTemplate, LauncherKind, TemplateDestination, TemplateFields, TemplateNameKind,
};
use farhelm_proto::ui_channel::{Availability, HostView, SessionRow};
use farhelm_proto::{
    LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, SessionLaunch,
};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::dialog::Opened;
use super::dialog_tests::{painted, push, template, templates_state};
use super::fields::Field;
use super::save::{Candidate, choice_text};
use crate::components::EditCommand;
use crate::focus::Mark;
use crate::launcher::draft::{LaunchDraft, Tab};
use crate::main_view::MainView;
use crate::session_list::tests::{listing, preferences, session};
use crate::test_support::{self, click, keys, settle};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The helm's pinned host fixture's first host (id 1, identity `identity1`), and the same row with no recorded
/// identity, as id 2.
fn hosts() -> Vec<HostView> {
    let state = crate::launcher::view_tests::fixture_hosts(&[0]);
    let known: HostView = serde_json::from_value(state.entries[0].value.clone()).unwrap();
    let mut unknown = known.clone();
    unknown.id = 2;
    unknown.identity = None;
    vec![known, unknown]
}

/// New on host 1 in `~`, with no agent type and a passive remembered permission.
fn new_draft() -> LaunchDraft {
    LaunchDraft::new(Some(1), "~".into(), Some(LaunchPermission::Yolo))
}

/// A Clone of a Codex session on host 1, its launch carrying a model, an effort, a permission and a wrapper prefix.
fn clone_draft() -> LaunchDraft {
    let row: SessionRow = serde_json::from_value(test_support::session_entry("s1").value).unwrap();
    let mut info = row.info;
    info.cwd = "/src/project".into();
    info.title = "source".into();
    info.launch = SessionLaunch::Agent {
        selection: LaunchSelection {
            harness: LaunchHarness::Codex,
            model: Some("gpt-6-luna".into()),
            effort: Some(LaunchEffort::High),
            permissions: Some(LaunchPermission::Guarded),
            permission_flags: None,
            workspace_trust: None,
            wrapper_prefix: Some(vec!["with-endpoint".into(), "--".into()]),
            container: None,
        },
        start: vec!["codex".into(), "{farhelm_args}".into()],
        resume: None,
    };
    LaunchDraft::clone_of(&info, Some(1))
}

/// The real main view over the fake helm holding `templates`, with the launcher opened on `draft` in a window tall
/// enough for the whole panel, and the launcher's catalog read answered with an empty catalog. Every command sent so far
/// is taken, so a test's later commands are its own.
fn launcher_on<'a>(
    cx: &'a mut TestAppContext,
    draft: LaunchDraft,
    templates: &[LaunchTemplate],
) -> (FakeHelm, &'a mut VisualTestContext) {
    launcher_with_hosts(cx, draft, templates, &[0])
}

/// [`launcher_on`] with the host fixture's hosts at `hosts` registered (host11, the eleventh, is an environment's).
fn launcher_with_hosts<'a>(
    cx: &'a mut TestAppContext,
    draft: LaunchDraft,
    templates: &[LaunchTemplate],
    hosts: &[usize],
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut helm, _connection) = test_support::started(cx);
    helm.connect(vec![
        listing(&["a"], &[session("a")]),
        crate::launcher::view_tests::fixture_hosts(hosts),
        preferences(&[], None),
        templates_state(templates, Availability::Available),
    ]);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, MainView::new);
    window.simulate_resize(gpui::size(gpui::px(1600.), gpui::px(1600.)));
    window.update(|window, cx| {
        let mark = Mark::take(cx);
        assert!(crate::launcher::view::open(draft, &mark, window, cx).is_some());
    });
    settle(window);
    // The fake answers in order, so the main view's own start-up commands are answered too, plainly; the launcher's
    // catalog read gets an empty catalog.
    for sent in helm.commands() {
        let body = (sent.path == "/api/launch-catalog").then(|| json!([]));
        helm.reply(sent.id, 200, body);
    }
    settle(window);
    (helm, window)
}

/// New with Codex chosen by hand.
fn codex_draft() -> LaunchDraft {
    let mut draft = new_draft();
    draft.choose_harness(LaunchHarness::Codex, None, None);
    draft
}

/// Whether the launcher shows Codex as its chosen agent type: the launcher's draft, as the user sees it.
fn codex_chosen(cx: &mut VisualTestContext) -> bool {
    test_support::frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("launcher-harness-Codex"))
        .is_some_and(|node| node.toggled == Some(gpui::Toggled::True))
}

/// Type `name` into the open panel's name field and press Enter, checking that it sent one PUT under that name; returns
/// the save's command id and body.
fn save_as(
    helm: &mut FakeHelm,
    name: &str,
    cx: &mut VisualTestContext,
) -> (u64, serde_json::Value) {
    click(cx, "save-template-name");
    cx.simulate_input(name);
    keys(cx, "enter");
    let (id, method, path, body) = one(helm);
    assert_eq!(
        (method, path),
        (Method::Put, format!("/api/templates/{name}"))
    );
    (id, body)
}

/// Whether anything is drawn under the debug selector `selector`.
fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_some()
}

/// The one command sent since the last call: its method, path and body.
fn one(helm: &mut FakeHelm) -> (u64, Method, String, serde_json::Value) {
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "one command: {sent:?}");
    let sent = sent.into_iter().next().unwrap();
    (
        sent.id,
        sent.method,
        sent.path,
        sent.body.unwrap_or_default(),
    )
}

// ---- What is offered and checked ----------------------------------------------------------------------------------

/// SPEC.md: agent type, model and effort are checked whenever set, while "Remembered defaults ... are offered unchecked"
/// and the host, folder and name only after the user's own choice. A passive value that started checked would quietly
/// become a template edit. Specified: New with Codex, a model and an effort chosen offers its passive permission, its
/// prefilled host and folder and the empty wrapper prefix unchecked, and checks only agent type, model and effort.
#[test]
fn new_checks_its_choices_and_offers_passive_values_unchecked() {
    let mut draft = codex_draft();
    draft.choose_custom_model("gpt-6-luna", None);
    draft.choose_effort(Some(LaunchEffort::High));
    let candidate = Candidate::from_draft(&draft, &hosts());
    assert_eq!(
        candidate.checked,
        vec![Field::Agent, Field::Model, Field::Effort]
    );
    let fields = &candidate.fields;
    assert_eq!(fields.permissions, Some(Some(LaunchPermission::Yolo)));
    assert_eq!(fields.host.as_deref(), Some("identity1"));
    assert_eq!(
        fields.destination,
        Some(TemplateDestination::Folder("~".into()))
    );
    assert_eq!(fields.wrapper_prefix, Some(None));
    assert_eq!(fields.name, None, "an empty name is not offered");
    assert_eq!(choice_text(Field::Wrapper, fields).as_deref(), Some("None"));
}

/// SPEC.md: permissions and a wrapper prefix are checked "only after an explicit choice; host only after a hand pick; a
/// folder only after typing or picking it; session name after typing it". These read the draft's own marks, set by its
/// hand-edit methods. Specified: after each of those edits every one of them is checked; a typed name is a base name;
/// a hand-cleared wrapper prefix is saved as a reset.
#[test]
fn hand_edits_check_permissions_wrapper_host_folder_and_name() {
    let mut draft = codex_draft();
    draft.choose_permissions(Some(LaunchPermission::Guarded));
    draft.edit_wrapper(String::new());
    draft.choose_host(1, "~".into());
    draft.edit_folder("/work".into());
    draft.edit_name("review".into());
    let candidate = Candidate::from_draft(&draft, &hosts());
    for field in [
        Field::Permissions,
        Field::Wrapper,
        Field::Host,
        Field::Destination,
        Field::Name,
    ] {
        assert!(candidate.checked.contains(&field), "{field:?}");
    }
    let saved = candidate.selected(&candidate.checked, None);
    assert_eq!(saved.wrapper_prefix, Some(None));
    assert_eq!(saved.name.as_deref(), Some("review"));
    assert_eq!(saved.name_kind, None, "a typed name is a base name");
}

/// SPEC_impl.md: "a passive remembered value does not become a template edit". A remembered wrapper prefix loads on an
/// agent switch and must stay unchecked until edited. Specified: picking Codex with a remembered prefix offers it, shown
/// as its words, unchecked; editing it checks it.
#[test]
fn a_remembered_wrapper_prefix_is_offered_unchecked_until_edited() {
    let mut draft = new_draft();
    let remembered = vec!["with-endpoint".to_owned(), "--".to_owned()];
    draft.choose_harness(LaunchHarness::Codex, None, Some(&remembered));
    let passive = Candidate::from_draft(&draft, &hosts());
    assert!(!passive.checked.contains(&Field::Wrapper));
    assert_eq!(
        choice_text(Field::Wrapper, &passive.fields).as_deref(),
        Some("with-endpoint --")
    );
    draft.edit_wrapper("with-endpoint --profile work --".into());
    assert!(
        Candidate::from_draft(&draft, &hosts())
            .checked
            .contains(&Field::Wrapper)
    );
}

/// SPEC.md: Clone's launch choices are checked, while "Clone's inherited host and folder are offered unchecked". The old
/// launcher counted a restored launch as the user's own choices and Clone's placement as inherited. Specified: a Clone
/// checks agent type, model, effort, permissions and wrapper prefix, and offers host, folder and name unchecked.
#[test]
fn clone_checks_its_launch_choices_and_offers_its_placement_unchecked() {
    let candidate = Candidate::from_draft(&clone_draft(), &hosts());
    assert_eq!(
        candidate.checked,
        vec![
            Field::Agent,
            Field::Model,
            Field::Effort,
            Field::Permissions,
            Field::Wrapper
        ]
    );
    let fields = &candidate.fields;
    assert_eq!(fields.host.as_deref(), Some("identity1"));
    assert_eq!(
        fields.destination,
        Some(TemplateDestination::Folder("/src/project".into()))
    );
    assert_eq!(fields.name.as_deref(), Some("source"));
}

/// SPEC.md: a nonempty initial prompt is checked, "shown by its escaped first line with an ellipsis when more lines
/// follow", and only the active tab's values are offered. Specified: the prompt is checked and shows its first line and
/// an ellipsis, while the full text is saved; for an agent that cannot take a prompt, or on the Command tab, it is not
/// offered.
#[test]
fn the_initial_prompt_is_checked_by_its_first_line_where_it_can_be_sent() {
    let mut draft = codex_draft();
    draft.edit_prompt("first\nsecond".into());
    let candidate = Candidate::from_draft(&draft, &hosts());
    assert!(candidate.checked.contains(&Field::Prompt));
    assert_eq!(
        choice_text(Field::Prompt, &candidate.fields).as_deref(),
        Some("first…")
    );
    assert_eq!(
        candidate
            .selected(&candidate.checked, None)
            .prompt
            .as_deref(),
        Some("first\nsecond")
    );
    draft.choose_harness(LaunchHarness::Goose, None, None);
    assert_eq!(Candidate::from_draft(&draft, &hosts()).fields.prompt, None);
    draft.choose_tab(Tab::Command);
    assert_eq!(Candidate::from_draft(&draft, &hosts()).fields.prompt, None);
}

/// SPEC.md: "Command, YOLO and opted-in resume are checked when set", and the saved template "always" has the active
/// launch kind. The Command tab must not save the dormant Agent tab's choices. Specified: on the Command tab with an agent
/// type chosen on the Agent tab and Resume off, Command, YOLO and the declared agent are checked and nothing of the agent
/// tab is offered; saving only the command keeps the command kind; switching Resume on offers it checked.
#[test]
fn the_command_tab_offers_only_its_own_fields() {
    let mut draft = codex_draft();
    draft.choose_tab(Tab::Command);
    draft.edit_command("printf hello".into());
    draft.choose_command_yolo(false);
    draft.choose_command_agent(Some(LaunchHarness::Claude));
    let candidate = Candidate::from_draft(&draft, &hosts());
    assert_eq!(
        candidate.checked,
        vec![Field::Command, Field::Yolo, Field::Agent]
    );
    assert_eq!(candidate.fields.agent, Some(LaunchHarness::Claude));
    assert_eq!(candidate.fields.permissions, None);
    assert_eq!(candidate.fields.resume_command, None);
    let only_command = candidate.selected(&[Field::Command], None);
    assert_eq!(only_command.kind, Some(LauncherKind::Command));
    assert_eq!((only_command.agent, only_command.yolo), (None, None));
    draft.choose_command_resume(true);
    draft.edit_command_resume("printf resume".into());
    assert!(
        Candidate::from_draft(&draft, &hosts())
            .checked
            .contains(&Field::Resume)
    );
}

/// SPEC.md: "A GitHub repository is always checked", and a managed checkout's set-aside folder is not where the session
/// would run, so offering it would save a destination the user never chose. Specified: with a folder typed and then a
/// repository chosen, the destination offered is the repository, shown as `gh:owner/repo`, checked without any hand mark,
/// and saved in the template format as `{"github": "owner/repo"}`; with only typed repository text, no destination is
/// offered at all.
#[test]
fn a_chosen_repository_is_offered_checked_and_the_folder_is_not() {
    let mut draft = codex_draft();
    draft.edit_folder("/typed".into());
    draft.edit_repository_query("example/proj".into());
    let typed = Candidate::from_draft(&draft, &hosts());
    assert_eq!(
        typed.fields.destination, None,
        "typed repository text is no destination"
    );
    draft.choose_repository(farhelm_proto::parse_github_repo("example/project").unwrap());
    let candidate = Candidate::from_draft(&draft, &hosts());
    assert_eq!(
        candidate.fields.destination,
        Some(TemplateDestination::Github("example/project".into()))
    );
    assert!(candidate.checked.contains(&Field::Destination));
    assert_eq!(
        choice_text(Field::Destination, &candidate.fields).as_deref(),
        Some("gh:example/project")
    );
    let saved = serde_json::to_value(candidate.selected(&candidate.checked, None)).unwrap();
    assert_eq!(saved["destination"], json!({"github": "example/project"}));
}

/// A Clone into a managed checkout ignores the copied title it has not been told to keep (SPEC.md, "Managed checkouts":
/// "Clone into a different repository ... ignore an untouched copied title: the session is unnamed"), so Save as
/// template must not offer that title as the session name. Specified: a Clone with a repository chosen offers no name;
/// once a name is typed, that name is offered, checked.
#[test]
fn a_clone_into_a_checkout_offers_no_copied_title() {
    let mut draft = clone_draft();
    assert_eq!(
        Candidate::from_draft(&draft, &hosts())
            .fields
            .name
            .as_deref(),
        Some("source"),
        "the premise: a Clone in a folder offers its copied title"
    );
    draft.choose_repository(farhelm_proto::parse_github_repo("example/project").unwrap());
    assert_eq!(Candidate::from_draft(&draft, &hosts()).fields.name, None);
    draft.edit_name("review".into());
    let candidate = Candidate::from_draft(&draft, &hosts());
    assert_eq!(candidate.fields.name.as_deref(), Some("review"));
    assert!(candidate.checked.contains(&Field::Name));
}

/// SPEC.md: the session-name row defaults "to base for a typed name and retain[s] a template-supplied kind", and "A host
/// without a usable recorded install identity is omitted". Specified: a name a template set as Fixed is checked and
/// offered as Fixed, and unchecking the name drops its kind; a host with no recorded identity is not offered.
#[test]
fn a_template_name_keeps_its_kind_and_an_unidentified_host_is_omitted() {
    let mut draft = new_draft();
    draft.edit_name("app-review".into());
    draft.set_template_name_kind(TemplateNameKind::Fixed);
    draft.host = Some(2);
    let candidate = Candidate::from_draft(&draft, &hosts());
    assert!(candidate.checked.contains(&Field::Name));
    assert_eq!(candidate.fields.name_kind, Some(TemplateNameKind::Fixed));
    assert_eq!(
        candidate
            .selected(&[], Some(TemplateNameKind::Fixed))
            .name_kind,
        None
    );
    assert_eq!(candidate.fields.host, None);
}

// ---- The panel in the launcher -------------------------------------------------------------------------------------

/// SPEC.md: "Cancel comes first in the action row, then Reset choices where offered, Save as template where offered, and
/// the main action last", and "Replace with does not offer this action". Specified: New's footer has the four in that
/// order.
#[gpui::test]
fn the_footer_offers_save_as_template_before_the_main_action(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, codex_draft(), &[]);
    let order: Vec<String> = test_support::frame(cx)
        .into_iter()
        .filter_map(|node| node.element_id.map(|id| id.to_string()))
        .filter(|id| {
            [
                "launcher-cancel",
                "launcher-reset",
                "launcher-save-template",
                "launcher-launch",
            ]
            .contains(&id.as_str())
        })
        .collect();
    assert_eq!(
        order,
        [
            "launcher-cancel",
            "launcher-reset",
            "launcher-save-template",
            "launcher-launch"
        ]
    );
}

/// SPEC.md: "Enter in the name field saves without launching", the saved template "contains exactly the checked fields
/// and always the active launch kind", and "Successful saving closes the launcher and opens the new template in the
/// Templates editor". Specified: the panel opens with focus in its name field and holds Launch; typing a name and Enter
/// sends one PUT of the checked fields and no create; once the helm stores it, the launcher is gone and Templates shows
/// the new template in its editor.
#[gpui::test]
fn enter_saves_the_checked_fields_and_opens_the_new_template(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, codex_draft(), &[]);
    click(cx, "launcher-save-template");
    assert!(painted(cx).contains("Choose which parts of this setup the template keeps"));
    // Typed before any click: focus went to the name field when the panel opened.
    cx.simulate_input("my-codex");
    click(cx, "launcher-launch");
    assert!(
        helm.commands().is_empty(),
        "Launch waits while the panel is open"
    );
    click(cx, "save-template-name");
    keys(cx, "enter");
    let (id, method, path, body) = one(&mut helm);
    assert_eq!(
        (method, path.as_str()),
        (Method::Put, "/api/templates/my-codex")
    );
    assert_eq!(body, json!({"kind": "agent", "agent": "codex"}));
    let saved = template(
        "my-codex",
        TemplateFields {
            kind: Some(LauncherKind::Agent),
            agent: Some(LaunchHarness::Codex),
            ..TemplateFields::default()
        },
    );
    push(&mut helm, &[saved], Availability::Available, cx);
    helm.reply(id, 200, None);
    settle(cx);
    assert!(!drawn(cx, "launcher"), "the launcher closed");
    let editor_name = cx.update(|_, cx| {
        let dialog = cx.global::<Opened>().0.upgrade().expect("Templates opened");
        dialog
            .read(cx)
            .editor
            .as_ref()
            .map(|editor| editor.draft().name.clone())
    });
    assert_eq!(editor_name.as_deref(), Some("my-codex"));
}

/// SPEC.md: "A taken template name is refused without overwriting", and Save as template always applies the new-name
/// rule. Specified: a name with whitespace and then a taken name are each refused in the panel with nothing sent.
#[gpui::test]
fn a_spaced_or_taken_name_is_refused_without_a_write(cx: &mut TestAppContext) {
    let taken = template("taken", TemplateFields::default());
    let (mut helm, cx) = launcher_on(cx, codex_draft(), &[taken]);
    click(cx, "launcher-save-template");
    click(cx, "save-template-name");
    cx.simulate_input("bad name");
    keys(cx, "enter");
    assert!(
        painted(cx).contains("cannot contain whitespace"),
        "{}",
        painted(cx)
    );
    cx.update(|window, cx| window.dispatch_action(EditCommand::SelectAll.action(), cx));
    cx.simulate_input("taken");
    click(cx, "save-template-save");
    assert!(painted(cx).contains("already exists"), "{}", painted(cx));
    assert!(helm.commands().is_empty());
}

/// SPEC.md: "errors preserve the launcher draft", and a failed save is reported and never retried ("One GUI at a time").
/// While the save is out the launcher holds still, so its refusal has somewhere to be said. Specified: while the PUT is
/// in flight Cancel, the header Close and Escape do nothing; the helm's refusal shows in the panel, the launcher stays
/// with Codex still chosen, and nothing more is sent.
#[gpui::test]
fn a_refused_save_is_reported_and_keeps_the_launcher(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, codex_draft(), &[]);
    click(cx, "launcher-save-template");
    click(cx, "save-template-name");
    cx.simulate_input("my-codex");
    keys(cx, "enter");
    let (id, ..) = one(&mut helm);
    click(cx, "launcher-cancel");
    click(cx, "launcher-close");
    keys(cx, "escape");
    assert!(
        drawn(cx, "launcher") && drawn(cx, "save-template-panel"),
        "Cancel, Close and Escape wait for the save"
    );
    helm.reply(id, 409, Some(json!("fixture template save refused")));
    settle(cx);
    assert!(painted(cx).contains("fixture template save refused"));
    assert!(drawn(cx, "launcher") && drawn(cx, "save-template-panel"));
    assert!(codex_chosen(cx), "the launcher's draft stayed");
    assert!(helm.commands().is_empty(), "nothing was retried");
}

/// SPEC.md: "Escape returns to the launcher", and the old panel took the launcher's Escape first. Specified: Escape with
/// the panel open closes only the panel, the launcher and its Codex choice stay, and the next Escape closes the
/// launcher.
#[gpui::test]
fn escape_closes_the_panel_and_keeps_the_launcher(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, codex_draft(), &[]);
    click(cx, "launcher-save-template");
    assert!(
        drawn(cx, "save-template-panel"),
        "the premise: the panel is open"
    );
    keys(cx, "escape");
    assert!(!drawn(cx, "save-template-panel"));
    assert!(drawn(cx, "launcher") && codex_chosen(cx));
    keys(cx, "escape");
    assert!(!drawn(cx, "launcher"));
}

/// SPEC.md: "Errors that are not a refused launch, such as "Save as template" refusing a wrapper, keep the plain line".
/// Specified: with a wrapper prefix that does not parse, Save as template opens no panel and says why, without "Not
/// launched.".
#[gpui::test]
fn a_wrapper_that_does_not_parse_refuses_on_a_plain_line(cx: &mut TestAppContext) {
    let mut draft = codex_draft();
    draft.edit_wrapper("with-endpoint 'unclosed".into());
    let (_helm, cx) = launcher_on(cx, draft, &[]);
    click(cx, "launcher-save-template");
    assert!(!drawn(cx, "save-template-panel"));
    assert!(drawn(cx, "launcher-save-template-refusal"));
    assert!(!painted(cx).contains("Not launched."));
}

/// SPEC.md: "Replace with does not offer this action". Specified: Replace with's footer has no Save as template.
#[gpui::test]
fn replace_with_offers_no_save_as_template(cx: &mut TestAppContext) {
    let row: SessionRow = serde_json::from_value(test_support::session_entry("s1").value).unwrap();
    let replace = LaunchDraft::replace_with("s1", &row.info, Some(1));
    let (_helm, window) = launcher_on(cx, replace, &[]);
    assert!(
        drawn(window, "launcher"),
        "the premise: the launcher is open"
    );
    assert!(!drawn(window, "launcher-save-template"));
}

/// The old footer left Save as template out on a host a disposable environment owns, whose identity names the
/// environment rather than a place a later launch can go. Specified: a launcher on host11, an environment's host, has
/// no Save as template.
#[gpui::test]
fn an_environments_host_offers_no_save_as_template(cx: &mut TestAppContext) {
    let draft = LaunchDraft::new(Some(11), "~".into(), None);
    let (_helm, window) = launcher_with_hosts(cx, draft, &[], &[0, 10]);
    assert!(
        drawn(window, "launcher"),
        "the premise: the launcher is open"
    );
    assert!(!drawn(window, "launcher-save-template"));
}

/// The panel's own controls decide what is saved: SPEC.md, "The saved template contains exactly the checked fields",
/// and the session-name row "offers base or fixed name". Specified: with Codex and a typed name, unchecking Agent type
/// and choosing Fixed name saves the launch kind and the name as fixed, without the agent type.
#[gpui::test]
fn unchecking_a_field_and_choosing_fixed_change_what_is_saved(cx: &mut TestAppContext) {
    let mut draft = codex_draft();
    draft.edit_name("review".into());
    let (mut helm, cx) = launcher_on(cx, draft, &[]);
    click(cx, "launcher-save-template");
    click(cx, "save-template-field-agent");
    click(cx, "save-template-name-kind-Fixed name");
    let (_, body) = save_as(&mut helm, "kept", cx);
    assert_eq!(
        body,
        json!({"kind": "agent", "name": "review", "name_kind": "fixed"})
    );
}

/// The panel opens at the end of the launcher's body, below the fold in the app's default window; the name field it
/// focuses must be on screen to type into. Specified: at 1100 by 760, opening the panel scrolls the body so the name
/// field shows above the pinned footer, with focus in it. (The old app's WebKit `focus()` centered the field and so
/// showed part of the checklist too; the parity file records the difference.)
#[gpui::test]
fn opening_the_panel_brings_its_name_field_into_view(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, codex_draft(), &[]);
    cx.simulate_resize(gpui::size(gpui::px(1100.), gpui::px(760.)));
    settle(cx);
    let before = test_support::bounds(cx, "launcher-folder-row");
    click(cx, "launcher-save-template");
    settle(cx);
    assert!(
        test_support::bounds(cx, "launcher-folder-row").top() < before.top(),
        "the premise: the panel opened below the fold, so the body scrolled"
    );
    let footer = test_support::bounds(cx, "launcher-cancel");
    let field = test_support::bounds(cx, "save-template-name");
    assert!(
        field.bottom() <= footer.top() && field.top() >= gpui::px(0.),
        "{field:?} above {footer:?}"
    );
    cx.simulate_input("typed");
    assert!(painted(cx).contains("typed"), "focus is in the name field");
}

/// SPEC.md, "Who may move focus": "Once the user has moved on, the result moves no focus, opens no dialog", closing its
/// own subject being the one thing a completion still does. The old launcher opened Templates whatever the user did
/// meanwhile; here a press while the save is out keeps Templates closed. Specified: after a click during the save, its
/// success closes the launcher and opens no dialog.
#[gpui::test]
fn a_save_the_user_moved_on_from_opens_no_dialog(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, codex_draft(), &[]);
    click(cx, "launcher-save-template");
    let (id, _) = save_as(&mut helm, "my-codex", cx);
    // A click inside Farhelm while the save is out: the user has moved on.
    click(cx, "save-template-name");
    let saved = template("my-codex", TemplateFields::default());
    push(&mut helm, &[saved], Availability::Available, cx);
    helm.reply(id, 200, None);
    settle(cx);
    assert!(!drawn(cx, "launcher"), "the launcher closed");
    assert!(!test_support::any_open(cx), "no Templates dialog opened");
}

/// Cancel and the header Close do nothing while the save is out, and they must look it, as the old launcher disabled
/// Cancel then: a live-looking button that silently refuses misleads. Specified: before the save both take a click;
/// while the PUT is in flight neither does, Cancel staying focusable as a busy button does (the frame draws its Close
/// unavailable instead).
#[gpui::test]
fn cancel_and_close_are_drawn_busy_during_the_save(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, codex_draft(), &[]);
    click(cx, "launcher-save-template");
    let node = |cx: &mut VisualTestContext, id: &str| {
        test_support::frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("{id} was not drawn"))
    };
    for id in ["launcher-cancel", "launcher-close"] {
        assert!(node(cx, id).flags.click, "the premise: {id} takes a click");
    }
    let (id, _) = save_as(&mut helm, "my-codex", cx);
    let cancel = node(cx, "launcher-cancel");
    assert!(
        cancel.flags.focusable && !cancel.flags.click,
        "Cancel is busy"
    );
    assert!(
        !node(cx, "launcher-close").flags.click,
        "Close is unavailable"
    );
    helm.reply(id, 409, Some(json!("fixture template save refused")));
    settle(cx);
}

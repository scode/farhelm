//! Tests of the launcher's managed-checkout destination: the draft's destination edits and the copied-title rule, the
//! launch check and create body for a checkout, the suggestions' rules, and the dialog in the real main view over the
//! helm client's fake (the Where row, the repository field and its list, and the preview with what invalidates it).
//!
//! The oracle is SPEC.md, "Managed checkouts", and the old launcher's behavior at reference commit `24bc0580`
//! (`github_checkout.rs`, the destination parts of `list/create_form.rs`, and `github-checkout-composer.spec.ts`).
//! In the dialog tests the launcher opens on New on host1 (connected on connection 4, installation `identity1`) in
//! `/tmp/retained-folder`, a Command tab draft so that a press can launch; the fake answers only when told.

use std::time::Duration;

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{Collection, CollectionChanges, HostView, KeyedEntry};
use farhelm_proto::{AcceptedGithubPreview, CheckoutPreviewBinding, GithubRepo};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::draft::{DestinationKind, LaunchDraft, Mode, Tab};
use super::request::{
    CONVERSATION_IN_CHECKOUT, Preview, SELECT_REPOSITORY, Target, UNVERIFIED_HOST, WAIT_FOR_PREVIEW,
};
use super::view_tests::{Opened, answer_catalog, draft, launcher_on, name_of, read, shown_refusal};
use crate::test_support::{self, settle};

fn repo(owner: &str, name: &str) -> GithubRepo {
    GithubRepo {
        owner: owner.into(),
        name: name.into(),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The draft
// ---------------------------------------------------------------------------------------------------------------------

/// SPEC.md: "The folder segment deliberately leaves checkout mode and restores the prior editable path", and
/// "Empty, invalid or unselected repository input cannot launch using a previous folder". Why: the folder must survive
/// a detour through Managed checkout, while nothing typed into the repository field may stand for a choice. Specified:
/// Managed checkout starts empty with the folder kept aside; typing a valid pair chooses nothing; choosing one sets it
/// and counts the choice; typing again drops it; Folder brings back the folder untouched; choosing the kind already
/// chosen keeps a chosen repository.
#[test]
fn the_destination_switch_keeps_the_folder_and_typed_text_never_chooses() {
    let mut draft = LaunchDraft::new(Some(1), "/tmp/retained-folder".into(), None);
    draft.edit_folder("/tmp/typed".into());
    draft.choose_destination(DestinationKind::Checkout);
    let checkout = draft.checkout.clone().expect("a checkout is chosen");
    assert_eq!((checkout.query.as_str(), checkout.repo), ("", None));
    draft.edit_repository_query("acme/bar".into());
    assert_eq!(
        draft.checkout.as_ref().and_then(|c| c.repo.clone()),
        None,
        "a typed pair is not a choice"
    );
    draft.choose_repository(repo("acme", "bar"));
    let checkout = draft.checkout.clone().unwrap();
    assert_eq!(checkout.repo, Some(repo("acme", "bar")));
    assert_eq!(checkout.chosen, 1);
    assert_eq!(checkout.field_text(), "acme/bar");
    draft.choose_destination(DestinationKind::Checkout);
    assert_eq!(
        draft.checkout.as_ref().and_then(|c| c.repo.clone()),
        Some(repo("acme", "bar")),
        "choosing the kind already chosen keeps the repository"
    );
    draft.edit_repository_query("acme/ba".into());
    let checkout = draft.checkout.clone().unwrap();
    assert_eq!(
        (checkout.repo, checkout.chosen),
        (None, 1),
        "typing drops the choice"
    );
    draft.choose_destination(DestinationKind::Folder);
    assert_eq!(draft.checkout, None);
    assert_eq!(
        draft.folder.value(),
        "/tmp/typed",
        "the folder comes back as it was"
    );
}

/// SPEC.md: "Selecting a folder or editing the ordinary directory returns to an existing-directory launch without
/// changing the agent choice", and choosing a repository again "obtains a new preview" even for the same one. Specified:
/// a folder picked while a checkout is chosen ends checkout mode with the agent tab untouched; choosing the same
/// repository twice counts twice.
#[test]
fn a_picked_folder_ends_checkout_mode_and_each_choice_counts() {
    let mut draft = LaunchDraft::new(Some(1), "~".into(), None);
    draft.choose_tab(Tab::Command);
    draft.choose_repository(repo("acme", "bar"));
    draft.choose_repository(repo("acme", "bar"));
    assert_eq!(draft.checkout.as_ref().map(|c| c.chosen), Some(2));
    draft.edit_folder("/srv".into());
    assert_eq!(draft.destination_kind(), DestinationKind::Folder);
    assert_eq!(draft.folder.value(), "/srv");
    assert_eq!(draft.tab, Tab::Command, "the launch kind stays");
}

/// Restart with holds its destination (SPEC.md, "Restart with"), so nothing may turn it into a checkout, not even the
/// composer search's `gh:` choice. Specified: every destination edit leaves Restart with's draft as it was.
#[test]
fn restart_with_holds_its_destination() {
    let session = super::draft_tests::session(farhelm_proto::SessionLaunch::Command(
        farhelm_proto::CommandLaunch {
            command: "codex".into(),
            yolo: false,
            agent: Some(farhelm_proto::LaunchHarness::Codex),
            resume: Some("codex resume".into()),
        },
    ));
    let mut draft = LaunchDraft::restart_with("a", &session, Some(1)).expect("offered");
    let before = draft.clone();
    draft.choose_destination(DestinationKind::Checkout);
    draft.edit_repository_query("acme/bar".into());
    draft.choose_repository(repo("acme", "bar"));
    draft.edit_folder("/elsewhere".into());
    assert_eq!(draft, before);
}

/// SPEC.md: "Replace with into a managed checkout, Clone into a different repository, and Clone of a source with no
/// repository ignore an untouched copied title: the session is unnamed", and choosing Folder again "restores the
/// source's original directory and copied title". Why: a copied title would otherwise name the new checkout's folder
/// after another session without the user asking. Specified: for Clone and Replace with, an untouched copied title gives
/// an empty checkout name and the field shows it empty, the folder destination shows the title again, and a name the
/// user types is kept; New's typed name is always kept.
#[test]
fn a_checkout_ignores_an_untouched_copied_title() {
    let session = super::draft_tests::session(farhelm_proto::SessionLaunch::Command(
        farhelm_proto::CommandLaunch {
            command: "true".into(),
            yolo: false,
            agent: None,
            resume: None,
        },
    ));
    for mut draft in [
        LaunchDraft::clone_of(&session, Some(1)),
        LaunchDraft::replace_with("a", &session, Some(1)),
    ] {
        assert!(matches!(draft.mode, Mode::Clone | Mode::ReplaceWith { .. }));
        assert!(!session.title.is_empty(), "the premise: a title to copy");
        draft.choose_repository(repo("acme", "bar"));
        assert_eq!(draft.checkout_title(), "", "the copied title is ignored");
        assert_eq!(draft.shown_name(), "");
        draft.choose_destination(DestinationKind::Folder);
        assert_eq!(draft.shown_name(), session.title, "Folder shows it again");
        draft.choose_destination(DestinationKind::Checkout);
        draft.edit_name("Fix parser".into());
        assert_eq!(draft.checkout_title(), "Fix parser", "a typed name is kept");
    }
    let mut new = LaunchDraft::new(Some(1), "~".into(), None);
    new.edit_name("mine".into());
    new.choose_repository(repo("acme", "bar"));
    assert_eq!(new.checkout_title(), "mine");
}

// ---------------------------------------------------------------------------------------------------------------------
// Suggestions
// ---------------------------------------------------------------------------------------------------------------------

/// SPEC.md: "Incomplete discovery is visible and does not prevent selecting a valid manually entered pair", and the old
/// `repository_choices`: the typed pair (lowercased) first, then discovered repositories containing the text, each once,
/// at most 100. Specified: a valid pair is offered with nothing discovered; invalid text offers none; filtering and
/// deduplication; the cap with the typed pair kept first.
#[test]
fn a_valid_typed_pair_is_always_the_first_suggestion() {
    use super::view::repository_choices_for_tests as choices;
    assert_eq!(choices("Acme/Bar", &[]), vec![repo("acme", "bar")]);
    assert!(choices("acme/bar@main", &[]).is_empty());
    let found = [repo("acme", "bar"), repo("acme", "baz"), repo("other", "x")];
    // `acme/` is not a pair, so only the discovered repositories containing it are offered.
    assert_eq!(
        choices("acme/", &found),
        vec![repo("acme", "bar"), repo("acme", "baz")]
    );
    // `acme/ba` is a valid pair itself, so it comes first, before the discovered ones it filters.
    assert_eq!(
        choices("acme/ba", &found),
        vec![repo("acme", "ba"), repo("acme", "bar"), repo("acme", "baz")]
    );
    assert_eq!(
        choices("acme/bar", &[repo("acme", "bar"), repo("acme", "bar")]),
        vec![repo("acme", "bar")]
    );
    let many: Vec<GithubRepo> = (0..120)
        .map(|n| repo("acme", &format!("bar-{n}")))
        .collect();
    let capped = choices("acme/bar", &many);
    assert_eq!(capped.len(), 100);
    assert_eq!(capped[0], repo("acme", "bar"));
}

// ---------------------------------------------------------------------------------------------------------------------
// The launch check and the create body
// ---------------------------------------------------------------------------------------------------------------------

/// host1 connected on connection 4 with installation `identity1`, and the same row with no identity (never reached).
fn hosts() -> Vec<HostView> {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../farhelm-helm/http-contract/host-list.json"
    ))
    .unwrap();
    let host: HostView = serde_json::from_value(fixture["hosts"][0].clone()).unwrap();
    assert_eq!(
        (host.id, host.incarnation, host.identity.as_deref()),
        (1, 4, Some("identity1")),
        "the premise: the fixture's first host"
    );
    let mut unidentified = host.clone();
    unidentified.id = 2;
    unidentified.identity = None;
    vec![host, unidentified]
}

fn preview(cwd: &str, basename: &str) -> AcceptedGithubPreview {
    AcceptedGithubPreview {
        binding: CheckoutPreviewBinding {
            canonical_root: "/work".into(),
            basename: basename.into(),
            cwd: cwd.into(),
            config_revision: 3,
        },
        host: "1".into(),
        incarnation: 4,
        installation_identity: "identity1".into(),
    }
}

/// A complete Command tab draft on `host` in `/tmp/retained-folder`.
fn command_draft(host: i64) -> LaunchDraft {
    let mut draft = LaunchDraft::new(Some(host), "/tmp/retained-folder".into(), None);
    draft.choose_tab(Tab::Command);
    draft.edit_command("true".into());
    draft.choose_command_yolo(false);
    draft
}

/// The old launcher's order and words (`LaunchPrerequisites::refusal`, and its press-time check of a named
/// conversation): a checkout without a chosen repository asks for one, before the launch kind's own reasons; a named
/// conversation cannot start in a new clone, which comes before the preview wait; a connected host that reports no
/// installation identity cannot have a checkout; and a chosen repository waits for a current preview, a failed one
/// included. Why: launching a checkout without the exact folder shown would break SPEC.md's "Before Launch becomes
/// available, the composer shows the exact host and path". Specified: each of those, and a ready preview launching.
#[test]
fn a_checkout_launches_only_with_a_repository_and_a_current_preview() {
    let hosts = hosts();
    let ready = preview("/work/bar-1", "bar-1");
    let target = |preview| Target {
        hosts: &hosts,
        catalog: None,
        preview,
    };
    let mut draft = LaunchDraft::new(Some(1), "~".into(), None);
    assert_eq!(
        draft.refusal(target(Preview::Waiting), None).as_deref(),
        Some("Choose a structured harness before launching"),
        "the premise: the agent tab has a reason of its own"
    );
    draft.choose_destination(DestinationKind::Checkout);
    assert_eq!(
        draft
            .refusal(target(Preview::Ready(&ready)), None)
            .as_deref(),
        Some(SELECT_REPOSITORY),
        "before the agent tab's own reason"
    );
    let mut draft = command_draft(1);
    draft.choose_repository(repo("acme", "bar"));
    for waiting in [Preview::Waiting, Preview::Failed] {
        assert_eq!(
            draft.refusal(target(waiting), None).as_deref(),
            Some(WAIT_FOR_PREVIEW)
        );
    }
    assert_eq!(draft.refusal(target(Preview::Ready(&ready)), None), None);
    let mut codex = LaunchDraft::new(Some(1), "~".into(), None);
    let _ = codex.choose_harness(farhelm_proto::LaunchHarness::Codex, None, None);
    codex.edit_conversation("my-thread".into());
    codex.choose_repository(repo("acme", "bar"));
    assert_eq!(
        codex.refusal(target(Preview::Waiting), None).as_deref(),
        Some(CONVERSATION_IN_CHECKOUT),
        "before the preview wait"
    );
    let mut unverified = command_draft(2);
    unverified.choose_repository(repo("acme", "bar"));
    assert_eq!(
        unverified
            .refusal(target(Preview::Ready(&ready)), None)
            .as_deref(),
        Some(UNVERIFIED_HOST)
    );
}

/// The create a checkout sends (the old `create_body` with its `github_checkout`): the previewed folder as `cwd`, the
/// preview's connection, and the repository, name and preview exactly as previewed, so the helm can refuse it if the
/// host's checkout folder changed since. An unnamed checkout sends no title and an empty checkout name, which the helm
/// names `<repo>-N`; a named one sends the name in both places, as the helm requires them to agree. A folder
/// destination sends no checkout.
#[test]
fn a_checkout_create_carries_its_preview() {
    let hosts = hosts();
    let ready = preview("/work/bar-1", "bar-1");
    let target = Target {
        hosts: &hosts,
        catalog: None,
        preview: Preview::Ready(&ready),
    };
    let mut draft = command_draft(1);
    draft.choose_repository(repo("acme", "bar"));
    let body = draft.create_body(target).expect("a complete checkout");
    assert_eq!(body.cwd, "/work/bar-1");
    assert_eq!(body.expected_incarnation, Some(4));
    assert_eq!(body.title, None);
    let checkout = body.github_checkout.expect("a checkout is requested");
    assert_eq!(checkout.repo, "acme/bar");
    assert_eq!(checkout.title.as_deref(), Some(""));
    assert_eq!(checkout.preview.as_ref(), Some(&ready));
    draft.edit_name("Fix parser".into());
    let body = draft.create_body(target).unwrap();
    assert_eq!(body.title.as_deref(), Some("Fix parser"));
    assert_eq!(
        body.github_checkout.and_then(|c| c.title).as_deref(),
        Some("Fix parser")
    );
    draft.choose_destination(DestinationKind::Folder);
    let body = draft.create_body(target).unwrap();
    assert_eq!(body.cwd, "/tmp/retained-folder");
    assert_eq!(body.github_checkout, None);
}

// ---------------------------------------------------------------------------------------------------------------------
// The dialog
// ---------------------------------------------------------------------------------------------------------------------

/// Every command the window sent since the last call (the fake hands each out once).
fn sent(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    cx.run_until_parked();
    helm.commands()
}

/// The commands among `commands` sent to `path`.
fn to(commands: &[FakeCommand], path: &str) -> Vec<FakeCommand> {
    commands
        .iter()
        .filter(|command| command.path == path)
        .cloned()
        .collect()
}

const REPOSITORIES: &str = "/api/github-repositories";
const PREVIEW: &str = "/api/github-checkout-preview";

/// A suggestions reply from host1's installation.
fn repositories(repos: &[(&str, &str)], truncated: bool, scan_error: Option<&str>) -> Value {
    json!({
        "host": "1", "incarnation": 4, "installation_identity": "identity1",
        "repos": repos.iter().map(|(owner, name)| json!({"owner": owner, "name": name})).collect::<Vec<_>>(),
        "truncated": truncated, "scan_error": scan_error, "needs_checkout_root": false,
    })
}

/// A preview reply from host1's installation.
fn preview_reply(cwd: &str, basename: &str, revision: i64) -> Value {
    json!({
        "canonical_root": "/work", "basename": basename, "cwd": cwd, "config_revision": revision,
        "host": "1", "incarnation": 4, "installation_identity": "identity1",
    })
}

/// Edit the open launcher's draft as `edit` does, then let it redraw.
fn edit(cx: &mut VisualTestContext, edit: impl FnOnce(&mut LaunchDraft)) {
    cx.update(|_, cx| {
        let launcher = cx
            .try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade())
            .expect("a launcher is open");
        launcher.update(cx, |launcher, cx| {
            edit(&mut launcher.draft);
            cx.notify();
        });
    });
    settle(cx);
}

/// What the read-only folder field shows while a checkout is chosen.
fn checkout_folder(cx: &mut VisualTestContext) -> String {
    read(cx, |launcher, _, cx| launcher.checkout_path_text(cx))
}

/// The launcher on a complete Command tab draft, with its catalog read answered and Managed checkout chosen from the
/// Where row.
fn checkout_open(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    let (mut helm, cx) = launcher_on(cx, command_draft(1));
    answer_catalog(&mut helm, cx, Some(json!([])));
    test_support::click(cx, "launcher-destination-Managed checkout");
    assert_eq!(
        draft(cx).destination_kind(),
        DestinationKind::Checkout,
        "the premise: the click chose a managed checkout"
    );
    (helm, cx)
}

/// SPEC.md: "Selecting managed checkout reveals a repository field and explains the directory's lifetime", the folder
/// field shows the preview read-only, and empty repository input cannot launch; Folder restores the prior path. Why:
/// the old app's case "destination type keeps unselected repositories separate from the folder" guarded against a
/// launch quietly using the old folder. Specified: after Managed checkout, the repository field and the lifetime
/// sentence are drawn, the folder field reads "Select a repository" and no folder check goes out; a press says
/// "Select a GitHub repository before launching" and sends nothing; Folder brings back the editable folder field
/// holding `/tmp/retained-folder`, and Launch then creates there.
#[gpui::test]
fn managed_checkout_hides_the_folder_until_a_repository_is_chosen(cx: &mut TestAppContext) {
    let (mut helm, cx) = checkout_open(cx);
    assert!(cx.debug_bounds("launcher-repository-row").is_some());
    test_support::advance(cx, Duration::from_millis(200));
    let commands = sent(&mut helm, cx);
    assert!(
        to(&commands, "/api/browse-directory").is_empty(),
        "no folder check for a checkout"
    );
    assert_eq!(checkout_folder(cx), "");
    assert!(
        read(cx, |launcher, _, _| launcher.checkout_path_placeholder()) == "Select a repository"
    );
    test_support::click(cx, "launcher-launch");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some(format!("Not launched. {SELECT_REPOSITORY}").as_str())
    );
    assert!(to(&sent(&mut helm, cx), "/api/sessions").is_empty());
    test_support::click(cx, "launcher-destination-Folder");
    assert!(cx.debug_bounds("launcher-repository-row").is_none());
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.folder.text(cx).to_string()),
        "/tmp/retained-folder"
    );
    test_support::click(cx, "launcher-launch");
    let creates = to(&sent(&mut helm, cx), "/api/sessions");
    let [create] = &creates[..] else {
        panic!("one create, not {}", creates.len());
    };
    let body = create.body.clone().unwrap();
    assert_eq!(body["cwd"], json!("/tmp/retained-folder"));
    assert_eq!(body["github_checkout"], Value::Null);
}

/// SPEC_impl.md: repository suggestions "debounce for 150 ms and discard stale host/query generations", and SPEC.md:
/// "Incomplete discovery is visible and does not prevent selecting a valid manually entered pair". Specified: typing
/// asks nothing before 150 ms; then one read for host1 on connection 4 with the typed text; a truncated reply with a
/// scan error lists its matching repositories and says the error under the field; Down and Enter pick the highlighted
/// row, which Enter never launches; the field then shows the choice and the note goes. A click into the field opens
/// the list by itself, as the old field's focus did.
#[gpui::test]
fn suggestions_are_read_after_typing_and_enter_picks_one(cx: &mut TestAppContext) {
    let (mut helm, cx) = checkout_open(cx);
    test_support::click(cx, "launcher-repository-row");
    assert!(
        read(cx, |launcher, window, cx| launcher
            .repository_focused(window, cx)),
        "the premise: the click focused the repository field"
    );
    assert!(
        cx.debug_bounds("launcher-repository-list").is_some(),
        "focusing the field opened its list"
    );
    // Not a valid pair itself, so the list holds only what the host found.
    test_support::keys(cx, "a c m e /");
    test_support::advance(cx, Duration::from_millis(100));
    assert!(
        to(&sent(&mut helm, cx), REPOSITORIES).is_empty(),
        "nothing before 150 ms"
    );
    test_support::advance(cx, Duration::from_millis(60));
    let reads = to(&sent(&mut helm, cx), REPOSITORIES);
    let [read_] = &reads[..] else {
        panic!("one read, not {}", reads.len());
    };
    assert_eq!(
        read_.body,
        Some(json!({"host": 1, "expected_incarnation": 4, "query": "acme/"}))
    );
    helm.reply(
        read_.id,
        200,
        Some(repositories(
            &[("acme", "bar"), ("acme", "baz"), ("other", "x")],
            true,
            Some("repository discovery is unavailable on this host; verify Git and the configured checkout root"),
        )),
    );
    settle(cx);
    assert!(cx.debug_bounds("launcher-repository-list").is_some());
    assert!(
        name_of(cx, "launcher-repository-option-acme/bar").is_some()
            && name_of(cx, "launcher-repository-option-acme/baz").is_some()
            && name_of(cx, "launcher-repository-option-other/x").is_none(),
        "the matching repositories are listed"
    );
    assert_eq!(
        name_of(cx, "launcher-repository-note").as_deref(),
        Some(
            "repository discovery is unavailable on this host; verify Git and the configured checkout root"
        )
    );
    test_support::keys(cx, "down down enter");
    assert_eq!(
        draft(cx).checkout.and_then(|checkout| checkout.repo),
        Some(repo("acme", "baz"))
    );
    assert!(
        cx.debug_bounds("launcher-repository-list").is_none(),
        "picking closes the list"
    );
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.repository_field_text(cx)),
        "acme/baz"
    );
    assert!(name_of(cx, "launcher-repository-note").is_none());
    assert!(
        to(&sent(&mut helm, cx), "/api/sessions").is_empty(),
        "Enter chose; it launched nothing"
    );
}

/// A valid typed pair stays selectable when discovery fails (SPEC.md, as above). Specified: with the read refused, the
/// list still offers the typed pair, the failure is said under the field, and picking it chooses it.
#[gpui::test]
fn a_typed_pair_is_chosen_when_discovery_fails(cx: &mut TestAppContext) {
    let (mut helm, cx) = checkout_open(cx);
    test_support::click(cx, "launcher-repository-row");
    test_support::keys(cx, "a c m e / q u x");
    test_support::advance(cx, Duration::from_millis(160));
    let reads = to(&sent(&mut helm, cx), REPOSITORIES);
    assert_eq!(reads.len(), 1, "the premise: the read is out");
    helm.reply(reads[0].id, 504, Some(json!("the host timed out")));
    settle(cx);
    assert!(name_of(cx, "launcher-repository-note").is_some());
    test_support::click(cx, "launcher-repository-option-acme/qux");
    assert_eq!(
        draft(cx).checkout.and_then(|checkout| checkout.repo),
        Some(repo("acme", "qux"))
    );
}

/// The preview: asked as soon as a repository is chosen, with the checkout's name (empty for an unnamed one); the
/// folder field waits, then shows the exact path; the name field offers the `repo-N` the checkout gets; and Launch
/// sends the create with the preview. SPEC.md: "Before Launch becomes available, the composer shows the exact host and
/// path", and "Whenever a checkout launch is unnamed, the name field shows the `repo-N` it will get as placeholder
/// text". Specified: one preview read for host1 on connection 4 of `acme/bar` with title ""; "Waiting for checkout
/// preview" and a refused press until it answers; then the path, the `bar-1` placeholder, and a create to
/// `/work/bar-1` carrying the preview.
#[gpui::test]
fn a_chosen_repository_is_previewed_before_it_launches(cx: &mut TestAppContext) {
    let (mut helm, cx) = checkout_open(cx);
    edit(cx, |draft| draft.choose_repository(repo("acme", "bar")));
    let previews = to(&sent(&mut helm, cx), PREVIEW);
    let [asked] = &previews[..] else {
        panic!("one preview, not {}", previews.len());
    };
    assert_eq!(
        asked.body,
        Some(json!({"host": 1, "expected_incarnation": 4, "repo": "acme/bar", "title": ""}))
    );
    assert_eq!(
        read(cx, |launcher, _, _| launcher.checkout_path_placeholder()),
        "Waiting for checkout preview"
    );
    test_support::click(cx, "launcher-launch");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some(format!("Not launched. {WAIT_FOR_PREVIEW}").as_str())
    );
    helm.reply(
        asked.id,
        200,
        Some(preview_reply("/work/bar-1", "bar-1", 3)),
    );
    settle(cx);
    assert_eq!(checkout_folder(cx), "/work/bar-1");
    assert_eq!(
        read(cx, |launcher, _, _| launcher
            .checkout_name_placeholder_shown()),
        "bar-1"
    );
    test_support::click(cx, "launcher-launch");
    let creates = to(&sent(&mut helm, cx), "/api/sessions");
    let [create] = &creates[..] else {
        panic!("one create, not {}", creates.len());
    };
    let body = create.body.clone().unwrap();
    assert_eq!(body["cwd"], json!("/work/bar-1"));
    assert_eq!(body["github_checkout"]["repo"], json!("acme/bar"));
    assert_eq!(
        body["github_checkout"]["preview"]["cwd"],
        json!("/work/bar-1")
    );
}

/// A refused checkout launch brings the launcher back on its draft, with focus in the search box as for any launcher
/// (SPEC.md: "Ordinary launches still start focus on search"), which is drawn whatever the destination. Why: starting
/// on a field that is not drawn (the editable folder field, while a checkout is chosen) would leave focus wherever the
/// focus coordinator's recovery puts it. Specified: with the preview ready, a press sends one create; the helm's
/// refusal brings the launcher back still on the checkout of `acme/bar`, its words shown, and the search box focused.
#[gpui::test]
fn a_refused_checkout_launch_comes_back_with_focus_in_the_search_box(cx: &mut TestAppContext) {
    let (mut helm, cx) = checkout_open(cx);
    edit(cx, |draft| draft.choose_repository(repo("acme", "bar")));
    let previews = to(&sent(&mut helm, cx), PREVIEW);
    assert_eq!(previews.len(), 1, "the premise: the preview is out");
    helm.reply(
        previews[0].id,
        200,
        Some(preview_reply("/work/bar-1", "bar-1", 3)),
    );
    settle(cx);
    test_support::click(cx, "launcher-launch");
    let creates = to(&sent(&mut helm, cx), "/api/sessions");
    let [create] = &creates[..] else {
        panic!("one create, not {}", creates.len());
    };
    helm.reply(create.id, 409, Some(json!("host1 is busy")));
    settle(cx);
    assert_eq!(
        draft(cx).checkout.and_then(|checkout| checkout.repo),
        Some(repo("acme", "bar")),
        "the premise: back on the checkout draft"
    );
    assert!(shown_refusal(cx).is_some_and(|words| words.contains("host1 is busy")));
    assert!(read(cx, |launcher, window, cx| launcher
        .search
        .focus_handle(cx)
        .is_focused(window)));
}

/// A refused preview leaves no path and says why: the old "Checkout preview unavailable" in the folder field and the
/// helm's words beside the preview, with Launch waiting. Specified with the helm's missing-folder refusal.
#[gpui::test]
fn a_refused_preview_says_why_and_shows_no_path(cx: &mut TestAppContext) {
    let (mut helm, cx) = checkout_open(cx);
    edit(cx, |draft| draft.choose_repository(repo("acme", "bar")));
    let previews = to(&sent(&mut helm, cx), PREVIEW);
    assert_eq!(previews.len(), 1, "the premise: the preview is out");
    let words = "no checkout folder is configured; set one in this host's settings or a default in Settings → Checkouts";
    helm.reply(previews[0].id, 400, Some(json!(words)));
    settle(cx);
    assert_eq!(checkout_folder(cx), "");
    assert_eq!(
        read(cx, |launcher, _, _| launcher.checkout_path_placeholder()),
        "Checkout preview unavailable"
    );
    assert_eq!(
        name_of(cx, "launcher-preview-error").as_deref(),
        Some(words)
    );
    test_support::click(cx, "launcher-launch");
    assert!(to(&sent(&mut helm, cx), "/api/sessions").is_empty());
}

/// SPEC.md: "Host, installation, destination, title, agent choice and observed configuration changes invalidate an
/// undispatched preview. Late responses cannot restore its authority or steal focus." Specified, one preview out at a
/// time throughout: a name typed while the first preview is out sends nothing until it answers; that late answer is
/// not shown, and the preview for the new name goes out at once; an agent-choice edit after the second answers asks
/// again; a revision observed higher than the answer's asks again, and an answer older than it is refused as stale; the
/// host reconnecting while a preview is out makes its answer stale and asks on the new connection; and leaving for
/// Folder while a preview is out keeps the folder destination, with its folder, when it answers. The focus half: the
/// late answer leaves focus in the name field the user is typing in.
#[gpui::test]
fn what_a_preview_answers_for_asks_again_and_late_answers_never_apply(cx: &mut TestAppContext) {
    let (mut helm, cx) = checkout_open(cx);
    edit(cx, |draft| draft.choose_repository(repo("acme", "bar")));
    let first = to(&sent(&mut helm, cx), PREVIEW);
    assert_eq!(first.len(), 1, "the premise: the first preview is out");
    test_support::click(cx, "launcher-name-row");
    let name_focused = |cx: &mut VisualTestContext| {
        read(cx, |launcher, window, cx| {
            launcher.name.focus_handle(cx).is_focused(window)
        })
    };
    assert!(
        name_focused(cx),
        "the premise: the user is in the name field"
    );
    edit(cx, |draft| draft.edit_name("fix".into()));
    assert!(
        to(&sent(&mut helm, cx), PREVIEW).is_empty(),
        "one preview at a time"
    );
    helm.reply(
        first[0].id,
        200,
        Some(preview_reply("/work/bar-1", "bar-1", 3)),
    );
    settle(cx);
    assert_eq!(checkout_folder(cx), "", "the late answer is not shown");
    assert!(name_focused(cx), "nor does it move focus");
    let second = to(&sent(&mut helm, cx), PREVIEW);
    let [second] = &second[..] else {
        panic!("one preview for the new name");
    };
    assert_eq!(second.body.as_ref().unwrap()["title"], json!("fix"));
    helm.reply(
        second.id,
        200,
        Some(preview_reply("/work/bar-fix", "bar-fix", 3)),
    );
    settle(cx);
    assert_eq!(checkout_folder(cx), "/work/bar-fix");

    edit(cx, |draft| draft.edit_command("false".into()));
    let third = to(&sent(&mut helm, cx), PREVIEW);
    assert_eq!(third.len(), 1, "an agent-choice change asks again");
    helm.reply(
        third[0].id,
        200,
        Some(preview_reply("/work/bar-fix", "bar-fix", 3)),
    );
    settle(cx);
    assert_eq!(checkout_folder(cx), "/work/bar-fix");

    cx.update(|_, cx| {
        let launcher = cx
            .try_global::<Opened>()
            .and_then(|o| o.0.upgrade())
            .unwrap();
        launcher.update(cx, |launcher, cx| {
            launcher.observe_checkout_config_revision(5, cx)
        });
    });
    settle(cx);
    let fourth = to(&sent(&mut helm, cx), PREVIEW);
    assert_eq!(fourth.len(), 1, "a newer revision asks again");
    helm.reply(
        fourth[0].id,
        200,
        Some(preview_reply("/work/bar-fix", "bar-fix", 4)),
    );
    settle(cx);
    assert_eq!(
        checkout_folder(cx),
        "",
        "an answer older than the revision is not one"
    );
    assert!(name_of(cx, "launcher-preview-error").is_some());

    edit(cx, |draft| draft.choose_repository(repo("acme", "bar")));
    let fifth = to(&sent(&mut helm, cx), PREVIEW);
    assert_eq!(fifth.len(), 1, "choosing again asks again");
    reconnect(&mut helm, cx, 5);
    helm.reply(
        fifth[0].id,
        200,
        Some(preview_reply("/work/bar-fix", "bar-fix", 5)),
    );
    settle(cx);
    assert_eq!(
        checkout_folder(cx),
        "",
        "an answer about the old connection is not one"
    );
    let after = sent(&mut helm, cx);
    answer_reconnect_reads(&mut helm, cx, &after);
    let sixth = to(&after, PREVIEW);
    let [sixth] = &sixth[..] else {
        panic!("one preview for the new connection");
    };
    assert_eq!(
        sixth.body.as_ref().unwrap()["expected_incarnation"],
        json!(5)
    );

    test_support::click(cx, "launcher-destination-Folder");
    let mut reply = preview_reply("/work/bar-fix", "bar-fix", 5);
    reply["incarnation"] = json!(5);
    helm.reply(sixth.id, 200, Some(reply));
    settle(cx);
    assert_eq!(
        draft(cx).destination_kind(),
        DestinationKind::Folder,
        "a late answer does not bring the checkout back"
    );
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.folder.text(cx).to_string()),
        "/tmp/retained-folder"
    );
}

/// Push host1 as connected on connection `incarnation`, as the helm does after the host reconnects. The app's Trash
/// then reads that host's checkout trash and the launcher rereads its launch history, which the caller answers in their
/// places in the fake's order (see [`answer_reconnect_reads`]).
fn reconnect(helm: &mut FakeHelm, cx: &mut VisualTestContext, incarnation: u64) {
    let mut host = hosts().remove(0);
    host.incarnation = incarnation;
    helm.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: test_support::meta(&["1"]),
        upsert: vec![KeyedEntry {
            key: "1".into(),
            value: serde_json::to_value(host).unwrap(),
        }],
        remove: Vec::new(),
    });
    settle(cx);
}

/// Answer the reads a reconnect sends among `commands`: the one checkout-trash read with a timeout, which the Trash
/// shows as its own failure, and the launcher's reread of the host's launch history with no body, so the launcher
/// offers no history and observes no new configuration revision. The fake answers commands in the order they were
/// sent, so both are answered in that order, before anything sent after them.
fn answer_reconnect_reads(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    commands: &[FakeCommand],
) {
    let reads = to(commands, "/api/checkout-trash/list");
    assert_eq!(
        reads.len(),
        1,
        "one trash read after the reconnect, not {}",
        reads.len()
    );
    let mut answered: Vec<&FakeCommand> = commands
        .iter()
        .filter(|command| {
            command.path == "/api/checkout-trash/list"
                || command.path.starts_with("/api/launch-history")
        })
        .collect();
    answered.sort_by_key(|command| command.id);
    for command in answered {
        if command.path.starts_with("/api/launch-history") {
            helm.reply(command.id, 200, None);
        } else {
            helm.reply(command.id, 504, Some(json!("the host timed out")));
        }
    }
    settle(cx);
}

/// An answer from another installation of the host than the one the launcher asked about is not an answer about it
/// (the old `PreviewAuthority::accepts`): it shows the old "stale or belongs to a different host installation" words
/// and Launch keeps waiting.
#[gpui::test]
fn a_preview_from_another_installation_is_refused(cx: &mut TestAppContext) {
    let (mut helm, cx) = checkout_open(cx);
    edit(cx, |draft| draft.choose_repository(repo("acme", "bar")));
    let previews = to(&sent(&mut helm, cx), PREVIEW);
    assert_eq!(previews.len(), 1, "the premise: the preview is out");
    let mut reply = preview_reply("/work/bar-1", "bar-1", 3);
    reply["installation_identity"] = json!("identity9");
    helm.reply(previews[0].id, 200, Some(reply));
    settle(cx);
    assert_eq!(checkout_folder(cx), "");
    assert_eq!(
        name_of(cx, "launcher-preview-error").as_deref(),
        Some(
            "The preview is stale or belongs to a different host installation; select the repository again"
        )
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Naming, Clone of a managed checkout
// ---------------------------------------------------------------------------------------------------------------------

/// A command-launch session titled "source" in `/src/project`, in a managed checkout of `current` (its working copy)
/// and launched as a checkout of `launched` (its provenance).
fn checkout_session(
    current: Option<GithubRepo>,
    launched: Option<GithubRepo>,
) -> farhelm_proto::SessionInfo {
    let mut info = super::draft_tests::session(farhelm_proto::SessionLaunch::Command(
        farhelm_proto::CommandLaunch {
            command: "true".into(),
            yolo: false,
            agent: None,
            resume: None,
        },
    ));
    info.working_copy = current.map(|repo| farhelm_proto::WorkingCopyInfo {
        id: "copy-1".into(),
        repo,
        canonical_path: "/work/bar-1".into(),
        origin_session_id: "s0".into(),
    });
    info.github_repo = launched;
    info
}

/// SPEC.md, "Managed checkouts": "Clone of a session in a managed checkout starts on a managed checkout of that
/// repository, preferring its current checkout association over its launch provenance", named `<title>-clone`; "the name
/// field shows the effective name"; "Choosing an existing folder again restores the source's original directory and
/// copied title; choosing the source repository again restores Clone's default name"; Clone into a different
/// repository is unnamed; "editing the name makes it explicit". Specified: each of those on one Clone, and that a
/// session with only launch provenance still clones onto its repository.
#[test]
fn a_clone_of_a_managed_checkout_starts_on_its_repository_named_clone() {
    let session = checkout_session(Some(repo("acme", "bar")), Some(repo("acme", "old")));
    let mut draft = LaunchDraft::clone_of(&session, Some(1));
    let checkout = draft
        .checkout
        .clone()
        .expect("a Clone of a checkout starts on one");
    assert_eq!(
        checkout.repo,
        Some(repo("acme", "bar")),
        "the current association wins"
    );
    assert!(!draft.name.is_touched(), "seeded, not chosen");
    assert_eq!(draft.checkout_title(), "source-clone");
    assert_eq!(draft.shown_name(), "source-clone");
    draft.choose_destination(DestinationKind::Folder);
    assert_eq!(
        (draft.folder.value().as_str(), draft.shown_name().as_str()),
        ("/src/project", "source"),
        "Folder brings back the source's folder and title"
    );
    draft.choose_repository(repo("acme", "other"));
    assert_eq!(draft.checkout_title(), "", "another repository is unnamed");
    draft.choose_repository(repo("acme", "bar"));
    assert_eq!(
        draft.checkout_title(),
        "source-clone",
        "the source's repository again"
    );
    draft.edit_name("mine".into());
    assert_eq!(draft.checkout_title(), "mine", "a typed name is exact");

    let provenance = checkout_session(None, Some(repo("acme", "old")));
    let draft = LaunchDraft::clone_of(&provenance, Some(1));
    assert_eq!(
        draft.checkout.and_then(|checkout| checkout.repo),
        Some(repo("acme", "old"))
    );
    let plain = checkout_session(None, None);
    assert_eq!(LaunchDraft::clone_of(&plain, Some(1)).checkout, None);
}

/// A repository identity the shared parser would not produce (a capital, an invisible character) did not come from the
/// helm, so a Clone does not carry it into the editable field; the old launcher showed such text escaped behind a
/// "Clear repository" button instead. The Clone still opens on a managed checkout, with nothing chosen, so one press of
/// Launch cannot start it inside the source's own checkout (SPEC.md: "Empty, invalid or unselected repository input
/// cannot launch using a previous folder"). Specified: for a capitalized and a formatting-character owner, the Clone is
/// a managed checkout with no repository chosen and an empty field.
#[test]
fn a_clone_carries_only_a_repository_the_shared_parser_reads_back() {
    for owner in ["Acme", "ac\u{202e}me"] {
        let session = checkout_session(Some(repo(owner, "bar")), None);
        let draft = LaunchDraft::clone_of(&session, Some(1));
        let checkout = draft.checkout.expect("still a managed checkout");
        assert_eq!(
            (checkout.field_text(), checkout.repo),
            (String::new(), None),
            "{owner:?}"
        );
        assert_eq!(draft.clone_source, None);
    }
}

/// SPEC.md: Clone's default and a template base name use "the lowest free `<title>-clone-N`" / "its first free
/// candidate, from the base itself through `-2` to `-50`", each advanced only by a preview that finds the name taken;
/// "Changing hosts or repositories restarts the suffix search"; "A typed name or a template's fixed name remains a
/// conflict rather than gaining a suffix"; and the cap "leaves the conflict visible". Specified: a taken `source-clone`
/// moves to `-2` and `-3`; an answer about a name the draft no longer holds moves nothing; a host change starts over; a
/// template base name numbers the same way; a fixed or typed name never moves; at `-50` it stops.
#[test]
fn a_taken_default_name_moves_on_and_an_exact_one_stays() {
    let session = checkout_session(Some(repo("acme", "bar")), None);
    let mut draft = LaunchDraft::clone_of(&session, Some(1));
    assert!(draft.advance_checkout_name("source-clone"));
    assert_eq!(draft.checkout_title(), "source-clone-2");
    assert!(
        !draft.advance_checkout_name("source-clone"),
        "a late answer about the old name"
    );
    assert!(draft.advance_checkout_name("source-clone-2"));
    assert_eq!(draft.checkout_title(), "source-clone-3");
    draft.choose_host(2, "~".into());
    assert_eq!(
        draft.checkout_title(),
        "source-clone",
        "another host starts over"
    );
    for _ in 2..=50 {
        let title = draft.checkout_title();
        assert!(draft.advance_checkout_name(&title));
    }
    assert_eq!(draft.checkout_title(), "source-clone-50");
    assert!(!draft.advance_checkout_name("source-clone-50"), "the cap");

    let mut template = LaunchDraft::new(Some(1), "~".into(), None);
    template.edit_name("review".into());
    template.set_template_name_kind(farhelm_proto::launcher::TemplateNameKind::Base);
    template.choose_repository(repo("acme", "bar"));
    assert!(template.advance_checkout_name("review"));
    assert_eq!(template.checkout_title(), "review-2");
    template.set_template_name_kind(farhelm_proto::launcher::TemplateNameKind::Fixed);
    assert_eq!(template.checkout_title(), "review", "a fixed name is exact");
    assert!(!template.advance_checkout_name("review"));
    template.edit_name("typed".into());
    assert!(
        !template.advance_checkout_name("typed"),
        "a typed name is exact"
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Taken names, lost races, the inline setup and the guarded answer, in the dialog
// ---------------------------------------------------------------------------------------------------------------------

/// The supervisor's refusal of a taken checkout name, as the helm passes it on.
const TAKEN: &str = "a directory with that checkout name already exists; choose another title";

/// Answer what the window sent on opening, in the order sent (the fake answers in order): the catalog read with no
/// models, everything else before the first checkout preview with an empty answer, as the shared `answer_catalog`
/// does. Returns the commands from that preview on, still unanswered: a draft that opens on a chosen repository asks for
/// its preview at once, which `answer_catalog` would answer empty.
fn answer_catalog_keeping(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    let mut sent = sent(helm, cx);
    sent.sort_by_key(|command| command.id);
    let first_preview = sent
        .iter()
        .position(|command| command.path == PREVIEW)
        .expect("the premise: the opening draft asked for its preview");
    let rest = sent.split_off(first_preview);
    let mut read = false;
    for command in sent {
        if command.path.ends_with("launch-catalog") {
            read = true;
            helm.reply(command.id, 200, Some(json!([])));
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    assert!(
        read,
        "the premise: the launcher read the catalog before the preview"
    );
    settle(cx);
    rest
}

/// Answer the one preview among `commands` (which start with it, unanswered, in the order sent) with `status` and
/// `body`, then the commands after it as `answer_catalog_keeping` answers them, returning what the preview asked.
fn answer_preview_in(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    commands: &[FakeCommand],
    status: u16,
    body: Value,
) -> Value {
    let previews = to(commands, PREVIEW);
    let [preview] = &previews[..] else {
        panic!("one preview, not {}", previews.len());
    };
    helm.reply(preview.id, status, Some(body));
    for command in commands.iter().filter(|command| command.id > preview.id) {
        if command.path.ends_with("launch-catalog") {
            helm.reply(command.id, 200, Some(json!([])));
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    settle(cx);
    preview.body.clone().unwrap()
}

/// Answer the one preview among the commands sent since the last call with `status` and `body`, returning what it
/// asked.
fn answer_preview(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    status: u16,
    body: Value,
) -> Value {
    let previews = to(&sent(helm, cx), PREVIEW);
    let [preview] = &previews[..] else {
        panic!("one preview, not {}", previews.len());
    };
    helm.reply(preview.id, status, Some(body));
    settle(cx);
    preview.body.clone().unwrap()
}

/// SPEC.md: Clone's default name "uses ... the lowest free `<source title>-clone-N`" and "preview and launch use it
/// alike". Specified, on a Clone opened on a session in a checkout of `acme/bar`: the first preview asks for
/// `source-clone`; the helm says it is taken; the next asks for `source-clone-2`, which the name field then shows, and
/// whose ready answer is the folder shown.
#[gpui::test]
fn a_taken_clone_name_is_previewed_again_with_the_next_suffix(cx: &mut TestAppContext) {
    let mut draft =
        LaunchDraft::clone_of(&checkout_session(Some(repo("acme", "bar")), None), Some(1));
    draft.choose_tab(Tab::Command);
    draft.edit_command("true".into());
    draft.choose_command_yolo(false);
    let (mut helm, cx) = launcher_on(cx, draft);
    let first = answer_catalog_keeping(&mut helm, cx);
    let asked = answer_preview_in(&mut helm, cx, &first, 409, json!(TAKEN));
    assert_eq!(asked["title"], json!("source-clone"));
    let asked = answer_preview(
        &mut helm,
        cx,
        200,
        preview_reply("/work/bar-source-clone-2", "bar-source-clone-2", 3),
    );
    assert_eq!(asked["title"], json!("source-clone-2"));
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.name.text(cx).to_string()),
        "source-clone-2"
    );
    assert_eq!(checkout_folder(cx), "/work/bar-source-clone-2");
}

/// SPEC.md: "If another create wins the displayed path, Launch reports the conflict and obtains a new preview; it
/// never submits automatically". Specified: with a ready preview, the create the helm refuses with its proof that
/// nothing was accepted comes back saying "... nothing was accepted; review the refreshed preview and press Launch
/// again", a new preview goes out, and no second create is sent.
#[gpui::test]
fn a_lost_race_takes_a_new_preview_and_sends_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx) = checkout_open(cx);
    edit(cx, |draft| draft.choose_repository(repo("acme", "bar")));
    answer_preview(&mut helm, cx, 200, preview_reply("/work/bar-1", "bar-1", 3));
    test_support::click(cx, "launcher-launch");
    let creates = to(&sent(&mut helm, cx), "/api/sessions");
    let [create] = &creates[..] else {
        panic!("one create, not {}", creates.len());
    };
    let words =
        "the checkout preview is no longer current; obtain a new preview before resubmitting";
    helm.reply_marked(
        create.id,
        409,
        vec![farhelm_proto::ui_channel::HelmMarker::CreateUnaccepted],
        Some(json!(words)),
    );
    settle(cx);
    assert_eq!(
        shown_refusal(cx),
        Some(format!(
            "Not launched. {words}; nothing was accepted; review the refreshed preview and press Launch again"
        ))
    );
    let after = sent(&mut helm, cx);
    assert_eq!(to(&after, PREVIEW).len(), 1, "a new preview");
    assert!(to(&after, "/api/sessions").is_empty(), "nothing sent again");
}

/// SPEC.md: "When the selected host has no checkout folder in effect, the launcher's managed-checkout destination
/// offers the same explicit-save field ... success refreshes repository suggestions and the checkout preview without
/// reopening the launcher", and "Selecting a repository before setup does not hide the field". Specified: a suggestions
/// answer saying the host needs a folder shows the field, which reads the default folder; choosing a repository keeps
/// it; saving `~/checkouts` sends `PUT /api/checkout-root`, and once done both the suggestions and the preview are
/// asked for again; when the suggestions then say the host needs no setup, the field goes and focus moves on to the
/// repository field rather than wherever focus recovery would put it.
#[gpui::test]
fn a_host_without_a_checkout_folder_offers_the_setup_inline(cx: &mut TestAppContext) {
    let (mut helm, cx) = checkout_open(cx);
    test_support::advance(cx, Duration::from_millis(160));
    let reads = to(&sent(&mut helm, cx), REPOSITORIES);
    assert_eq!(reads.len(), 1, "the premise: the suggestions read is out");
    let mut reply = repositories(&[], false, Some("no checkout folder is configured"));
    reply["needs_checkout_root"] = json!(true);
    helm.reply(reads[0].id, 200, Some(reply));
    settle(cx);
    assert!(
        cx.debug_bounds("launcher-checkout-setup").is_some(),
        "the field shows"
    );
    let field_read = sent(&mut helm, cx);
    let [get] = &to(&field_read, "/api/checkout-root")[..] else {
        panic!("the field reads the default folder");
    };
    helm.reply(get.id, 200, Some(json!({"root": null})));
    settle(cx);
    edit(cx, |draft| draft.choose_repository(repo("acme", "bar")));
    answer_preview(
        &mut helm,
        cx,
        400,
        json!("no checkout folder is configured"),
    );
    assert!(
        cx.debug_bounds("launcher-checkout-setup").is_some(),
        "choosing a repository keeps it"
    );
    test_support::click(cx, "checkout-folder");
    test_support::keys(cx, "~ / c h e c k o u t s");
    test_support::click(cx, "checkout-folder-save");
    let put = sent(&mut helm, cx);
    let [save] = &to(&put, "/api/checkout-root")[..] else {
        panic!("one save");
    };
    assert_eq!(save.body, Some(json!({"root": "~/checkouts"})));
    helm.reply(save.id, 200, Some(json!({"root": "~/checkouts"})));
    settle(cx);
    test_support::advance(cx, Duration::from_millis(160));
    let after = sent(&mut helm, cx);
    let previews = to(&after, PREVIEW);
    assert_eq!(previews.len(), 1, "the preview is asked again");
    let reads = to(&after, REPOSITORIES);
    assert_eq!(reads.len(), 1, "and the suggestions");
    // Answered in the order sent, as the fake requires; the field reads its route again after a done save.
    let mut ordered = after.clone();
    ordered.sort_by_key(|command| command.id);
    for command in ordered {
        let body = match command.path.as_str() {
            PREVIEW => preview_reply("/home/user/checkouts/bar-1", "bar-1", 3),
            REPOSITORIES => repositories(&[], false, None),
            "/api/checkout-root" => json!({"root": "~/checkouts"}),
            other => panic!("unexpected command after the save: {other}"),
        };
        helm.reply(command.id, 200, Some(body));
    }
    settle(cx);
    assert!(
        cx.debug_bounds("launcher-checkout-setup").is_none(),
        "the host needs no setup now"
    );
    assert!(
        read(cx, |launcher, window, cx| launcher
            .repository_focused(window, cx)),
        "focus moved on to the repository field"
    );
}

/// A Codex launch with YOLO chosen, into a managed checkout of `acme/bar` previewed ready, launched, refused for YOLO by
/// a host that asks first, and answered "Start guarded session instead": the moment the guarded launch waits for its
/// new preview. Returns that preview's command, unanswered, once it is checked that no create went out with it.
fn guarded_waiting(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext, FakeCommand) {
    let mut draft = LaunchDraft::new(
        Some(1),
        "~".into(),
        Some(farhelm_proto::LaunchPermission::Yolo),
    );
    let _ = draft.choose_harness(farhelm_proto::LaunchHarness::Codex, None, None);
    draft.choose_permissions(Some(farhelm_proto::LaunchPermission::Yolo));
    draft.choose_repository(repo("acme", "bar"));
    let (mut helm, cx) = launcher_on(cx, draft);
    let first = answer_catalog_keeping(&mut helm, cx);
    answer_preview_in(
        &mut helm,
        cx,
        &first,
        200,
        preview_reply("/work/bar-1", "bar-1", 3),
    );
    test_support::click(cx, "launcher-launch");
    let creates = to(&sent(&mut helm, cx), "/api/sessions");
    let [create] = &creates[..] else {
        panic!("one create, not {}", creates.len());
    };
    helm.reply_marked(
        create.id,
        409,
        vec![farhelm_proto::ui_channel::HelmMarker::YoloConfirmation],
        Some(json!("YOLO launches on host1 need --yolo")),
    );
    settle(cx);
    assert_eq!(
        name_of(cx, "prompt-alternative-0").as_deref(),
        Some("Start guarded session instead"),
        "the premise: the question offers the guarded answer"
    );
    test_support::click(cx, "prompt-alternative-0");
    let after = sent(&mut helm, cx);
    assert!(
        to(&after, "/api/sessions").is_empty(),
        "no create before the preview"
    );
    let [preview] = &to(&after, PREVIEW)[..] else {
        panic!("the new preview");
    };
    let preview = preview.clone();
    (helm, cx, preview)
}

/// SPEC.md: the YOLO question's guarded answer starts "the same launch guarded instead"; switching to Guarded is an
/// agent-choice change, which "invalidate[s] an undispatched preview". The old launcher waited for the new preview and
/// then submitted once (its `GuardedSubmitStamp`); refusing instead would turn the answer into a dead end. Specified:
/// once the new preview answers ready, one create goes out, Guarded and with that preview, without consent.
#[gpui::test]
fn the_guarded_answer_waits_for_the_new_preview_then_launches_once(cx: &mut TestAppContext) {
    let (mut helm, cx, preview) = guarded_waiting(cx);
    helm.reply(
        preview.id,
        200,
        Some(preview_reply("/work/bar-1", "bar-1", 3)),
    );
    settle(cx);
    let creates = to(&sent(&mut helm, cx), "/api/sessions");
    let [create] = &creates[..] else {
        panic!("one create once ready, not {}", creates.len());
    };
    let body = create.body.as_ref().unwrap();
    assert_eq!(body["launch"]["permissions"], json!("guarded"));
    assert_eq!(body["confirm_yolo"], json!(false));
    assert_eq!(
        body["github_checkout"]["preview"]["cwd"],
        json!("/work/bar-1")
    );
}

/// The old cases "guarded checkout answer ... is cancelled by edit" and "... by close": the waiting launch answers for
/// the draft as it was answered, so an edit (here the initial prompt, which no preview depends on, so only the edit's
/// own rule can cancel it) or closing the launcher cancels it. Specified: for each, the ready preview that follows sends
/// no create.
#[gpui::test]
fn an_edit_cancels_the_guarded_answers_waiting_launch(cx: &mut TestAppContext) {
    let (mut helm, cx, preview) = guarded_waiting(cx);
    test_support::click(cx, "launcher-prompt-row");
    test_support::keys(cx, "h i");
    helm.reply(
        preview.id,
        200,
        Some(preview_reply("/work/bar-1", "bar-1", 3)),
    );
    settle(cx);
    assert!(to(&sent(&mut helm, cx), "/api/sessions").is_empty());
}

/// See [`an_edit_cancels_the_guarded_answers_waiting_launch`]: closing the launcher cancels it too.
#[gpui::test]
fn closing_the_launcher_cancels_the_guarded_answers_waiting_launch(cx: &mut TestAppContext) {
    let (mut helm, cx, preview) = guarded_waiting(cx);
    test_support::click(cx, "launcher-cancel");
    assert!(
        !test_support::any_open(cx),
        "the premise: the launcher closed"
    );
    helm.reply(
        preview.id,
        200,
        Some(preview_reply("/work/bar-1", "bar-1", 3)),
    );
    settle(cx);
    assert!(to(&sent(&mut helm, cx), "/api/sessions").is_empty());
}

/// SPEC.md: Launch "never submits automatically or silently chooses another directory". The waiting launch is for the
/// preview the guarded answer made due; something else asking for a new one meanwhile (here a newer configuration
/// revision, which may name another folder) means the folder it would start in is one the user has not seen.
/// Specified: after the revision rises, neither the old preview's answer nor the new preview's ready answer sends a
/// create.
#[gpui::test]
fn another_preview_cancels_the_guarded_answers_waiting_launch(cx: &mut TestAppContext) {
    let (mut helm, cx, preview) = guarded_waiting(cx);
    cx.update(|_, cx| {
        let launcher = cx
            .try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade())
            .expect("a launcher is open");
        launcher.update(cx, |launcher, cx| {
            launcher.observe_checkout_config_revision(9, cx)
        });
    });
    settle(cx);
    helm.reply(
        preview.id,
        200,
        Some(preview_reply("/work/bar-1", "bar-1", 9)),
    );
    settle(cx);
    let after = sent(&mut helm, cx);
    assert!(
        to(&after, "/api/sessions").is_empty(),
        "not on the old answer"
    );
    let [next] = &to(&after, PREVIEW)[..] else {
        panic!("the preview for the new revision");
    };
    helm.reply(next.id, 200, Some(preview_reply("/work/bar-1", "bar-1", 9)));
    settle(cx);
    assert!(
        to(&sent(&mut helm, cx), "/api/sessions").is_empty(),
        "nor on the new one"
    );
}

/// The old case "Escape dismisses repository suggestions before closing the composer": the list is a layer of its own
/// above the launcher, so the first Escape closes only it and the second the launcher, and nothing is created. Why: an
/// Escape meant for the list must not discard the whole draft. Specified: with the list open over the repository
/// field, Escape closes the list with the launcher still open and the field still focused; a second Escape closes the
/// launcher; no create was sent.
#[gpui::test]
fn escape_closes_the_suggestions_before_the_launcher(cx: &mut TestAppContext) {
    let (mut helm, cx) = checkout_open(cx);
    test_support::click(cx, "launcher-repository-row");
    assert!(
        cx.debug_bounds("launcher-repository-list").is_some(),
        "the premise: the list is open"
    );
    test_support::keys(cx, "escape");
    assert!(
        cx.debug_bounds("launcher-repository-list").is_none(),
        "the list closed"
    );
    assert!(test_support::any_open(cx), "the launcher stayed");
    assert!(read(cx, |launcher, window, cx| launcher
        .repository_focused(window, cx)));
    test_support::keys(cx, "escape");
    assert!(
        !test_support::any_open(cx),
        "the second Escape closed the launcher"
    );
    assert!(to(&sent(&mut helm, cx), "/api/sessions").is_empty());
}

// ---------------------------------------------------------------------------------------------------------------------
// A quick launch into a managed checkout
// ---------------------------------------------------------------------------------------------------------------------

/// The real main view over the fake helm with session `a`, host1 and the preferences, and a quick launch prepared out
/// of sight (`prepare_unseen`, what a template's shortcut starts) whose templates make a complete Command tab launch
/// into a managed checkout of `acme/bar`. Answers the window's first reads and the catalog read in the order sent, and
/// returns the commands from then on, still unanswered.
fn quick_launch_into_a_checkout(
    cx: &mut TestAppContext,
) -> (FakeHelm, &mut VisualTestContext, Vec<FakeCommand>) {
    use crate::session_list::tests::{listing, preferences, session};
    let (mut helm, _connection) = test_support::started(cx);
    helm.connect(vec![
        listing(&["a"], &[session("a")]),
        super::view_tests::fixture_hosts(&[0]),
        preferences(&[], None),
    ]);
    cx.run_until_parked();
    let (_, cx) = test_support::open_view(cx, crate::main_view::MainView::new);
    cx.update(|window, cx| {
        let finish = |draft: &mut LaunchDraft, _: super::draft::Catalog<'_>, _: &gpui::App| {
            draft.choose_tab(Tab::Command);
            draft.edit_command("true".into());
            draft.choose_command_yolo(false);
            draft.choose_repository(repo("acme", "bar"));
            super::view::Prepared::Launch { notice: None }
        };
        let draft = LaunchDraft::new(Some(1), "~".into(), None);
        assert!(super::view::prepare_unseen(
            draft,
            crate::focus::Mark::take(cx),
            finish,
            window,
            cx
        ));
    });
    settle(cx);
    let mut first = sent(&mut helm, cx);
    first.sort_by_key(|command| command.id);
    let mut read = false;
    for command in &first {
        if command.path.ends_with("launch-catalog") {
            read = true;
            helm.reply(command.id, 200, Some(json!([])));
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    assert!(read, "the premise: the quick launch read the catalog");
    settle(cx);
    let rest = sent(&mut helm, cx);
    (helm, cx, rest)
}

/// SPEC.md, "Managed checkouts": a template's shortcut "obtains the same exact preview and launches without showing the
/// path first", and the old app's case "quick launch waits for a managed preview and starts directly". Why: a repository
/// template's shortcut must start its session, not open New saying it waits for a preview. Specified: once the templates
/// are applied, a preview of `acme/bar` goes out and no create; nothing is shown; the ready answer sends one create into
/// the previewed folder, with the preview, and the launcher is still never shown.
#[gpui::test]
fn a_quick_launch_into_a_checkout_waits_for_its_preview_out_of_sight(cx: &mut TestAppContext) {
    let (mut helm, cx, sent_after) = quick_launch_into_a_checkout(cx);
    assert!(
        to(&sent_after, "/api/sessions").is_empty(),
        "no create before the preview"
    );
    let [preview] = &to(&sent_after, PREVIEW)[..] else {
        panic!("the preview goes out at once");
    };
    assert!(
        !test_support::any_open(cx),
        "nothing is shown while it waits"
    );
    helm.reply(
        preview.id,
        200,
        Some(preview_reply("/work/bar-1", "bar-1", 3)),
    );
    settle(cx);
    let creates = to(&sent(&mut helm, cx), "/api/sessions");
    let [create] = &creates[..] else {
        panic!("one create once ready, not {}", creates.len());
    };
    let body = create.body.as_ref().unwrap();
    assert_eq!(body["cwd"], json!("/work/bar-1"));
    assert_eq!(
        body["github_checkout"]["preview"]["cwd"],
        json!("/work/bar-1")
    );
    assert!(!test_support::any_open(cx), "the launcher was never shown");
}

/// A quick launch that cannot start shows New with the same draft and what stopped it (SPEC.md, "Quick launch": its
/// refusal "reveals New with the same draft, error or confirmation"). Specified: the preview refused for a host with no
/// checkout folder sends no create and shows the launcher, its preview saying the helm's words.
#[gpui::test]
fn a_quick_launch_whose_preview_fails_is_shown(cx: &mut TestAppContext) {
    let (mut helm, cx, sent_after) = quick_launch_into_a_checkout(cx);
    let [preview] = &to(&sent_after, PREVIEW)[..] else {
        panic!("the preview goes out at once");
    };
    let words = "no checkout folder is configured; set one in this host's settings or a default in Settings → Checkouts";
    helm.reply(preview.id, 400, Some(json!(words)));
    settle(cx);
    assert!(
        to(&sent(&mut helm, cx), "/api/sessions").is_empty(),
        "nothing started"
    );
    assert!(test_support::any_open(cx), "the launcher is shown");
    assert_eq!(
        name_of(cx, "launcher-preview-error").as_deref(),
        Some(words)
    );
}

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

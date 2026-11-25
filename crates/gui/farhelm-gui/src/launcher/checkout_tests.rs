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
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, HostStateView, HostView, KeyedEntry,
};
use farhelm_proto::{AcceptedGithubPreview, CheckoutPreviewBinding, GithubRepo};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::draft::{DestinationKind, LaunchDraft, Mode, Tab};
use super::request::{
    Account, CONVERSATION_IN_CHECKOUT, Preview, Recipes, SELECT_REPOSITORY, Target,
    UNVERIFIED_HOST, WAIT_FOR_PREVIEW,
};
use super::view_tests::{
    answer_catalog, click_in_body, draft, fixture_hosts, launcher_on, launcher_with_hosts, name_of,
    read, shown_refusal,
};
use crate::layers;
use crate::test_support::{self, settle};

/// The recipe catalog these tests check against: none read yet, which only a new environment's launch looks at.
static NO_CATALOG: Recipes = Recipes::Loading;

/// The Tensorlake account these tests check against: still being read, which only a new sandbox's launch looks at.
static NO_ACCOUNT: Account = Account::Reading;

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
            custom_agent: None,
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
            custom_agent: None,
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
        recipes: &NO_CATALOG,
        account: &NO_ACCOUNT,
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
        recipes: &NO_CATALOG,
        account: &NO_ACCOUNT,
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

/// The global list carries repository identities only, in recency order.
fn repositories(repos: &[(&str, &str)]) -> Value {
    json!(
        repos
            .iter()
            .map(|(owner, name)| json!({"owner": owner, "name": name}))
            .collect::<Vec<_>>()
    )
}

/// A pending repository reply also holds the commands the launcher sent after it on the ordered channel.
struct PendingRepositoryHistory {
    request: FakeCommand,
    following: Vec<FakeCommand>,
}

impl PendingRepositoryHistory {
    /// Answer the held history first, then release the later commands in their original order.
    fn answer(self, helm: &mut FakeHelm, cx: &mut VisualTestContext, status: u16, body: Value) {
        helm.reply(self.request.id, status, Some(body));
        for command in self.following.into_iter().chain(sent(helm, cx)) {
            assert_ne!(command.path, REPOSITORIES, "one global read per launcher");
            reply_checkout_launcher_read(helm, command, false);
        }
        settle(cx);
    }
}

/// Answer the ordinary fixture reads that accompany a repository or checkout-folder read.
fn reply_checkout_launcher_read(helm: &mut FakeHelm, command: FakeCommand, needs_root: bool) {
    let body = match command.path.as_str() {
        "/api/launch-catalog" | REPOSITORIES => Some(json!([])),
        path if path.starts_with("/api/launch-history?host=") => {
            Some(checkout_host_history(needs_root, 3))
        }
        _ => None,
    };
    helm.reply(command.id, 200, body);
}

/// Hold the first repository read so the test can observe loading, failure or fresh suggestions.
fn hold_repositories(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> PendingRepositoryHistory {
    let mut reads = sent(helm, cx).into_iter();
    let request = loop {
        let command = reads
            .next()
            .expect("the launcher reads global repository history");
        if command.path == REPOSITORIES {
            break command;
        }
        reply_checkout_launcher_read(helm, command, false);
    };
    assert_eq!(request.method, farhelm_helm_client::Method::Get);
    assert_eq!(request.body, None, "the read does not name a host or query");
    PendingRepositoryHistory {
        request,
        following: reads.collect(),
    }
}

/// Open directly on Managed checkout while its first global history answer is still pending.
fn checkout_open_with_history_pending(
    cx: &mut TestAppContext,
) -> (FakeHelm, &mut VisualTestContext, PendingRepositoryHistory) {
    let (mut helm, cx) = launcher_on(cx, command_draft(1));
    let request = hold_repositories(&mut helm, cx);
    test_support::click(cx, "launcher-destination-Managed checkout");
    assert_eq!(draft(cx).destination_kind(), DestinationKind::Checkout);
    (helm, cx, request)
}

/// A connected host's configuration comes with its launch history, independently of repository history.
fn checkout_host_history(needs_checkout_root: bool, revision: i64) -> Value {
    json!({
        "launches": [], "folders": [],
        "checkout_config_revision": revision, "needs_checkout_root": needs_checkout_root,
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
        let launcher =
            layers::last_opened::<super::view::Launcher>(cx).expect("a launcher is open");
        launcher.update(cx, |launcher, cx| {
            edit(&mut launcher.draft);
            cx.notify();
        });
    });
    settle(cx);
}

/// Read the actual Folder line's path. A pending or refused preview must leave no accessible path node behind.
fn checkout_folder(cx: &mut VisualTestContext) -> String {
    name_of(cx, "launcher-preview-path").unwrap_or_default()
}

/// Each Folder state must be distinguishable while keeping the same accessible status region. A stale state could
/// show an obsolete path or tell a screen reader that the launcher is ready before it actually is.
fn assert_folder_status(cx: &mut VisualTestContext, selector: &'static str, id: &str, words: &str) {
    let nodes = test_support::frame(cx);
    let status = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("launcher-preview"))
        .expect("the Folder line has a status region");
    assert_eq!(status.role, Some(gpui::Role::Status));
    assert_eq!(status.label.as_deref(), Some("Folder"));
    assert!(
        cx.debug_bounds(selector).is_some(),
        "Folder state {selector} is drawn"
    );
    let detail = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .expect("the Folder status words have an accessible name");
    assert_eq!(detail.label.as_deref(), Some(words));
    assert_eq!(detail.value.as_deref(), Some(words));
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
/// line shows the preview, and empty repository input cannot launch. Folder restores the prior path. Why:
/// the old app's case "destination type keeps unselected repositories separate from the folder" guarded against a
/// launch quietly using the old folder. Specified: after Managed checkout, the repository field and the lifetime
/// sentence are drawn, the Folder line reads "Select a repository" and no folder check goes out; a press says
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
    assert_folder_status(
        cx,
        "launcher-preview-unselected",
        "launcher-preview-message",
        "Select a repository",
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

/// Repository shares Host's address border. The Folder status spans the card beneath the address and Name,
/// so a long destination gets the full available width and the preview cannot repeat the path elsewhere.
#[gpui::test]
fn managed_checkout_shares_the_address_and_places_the_folder_below_it(cx: &mut TestAppContext) {
    let (_helm, cx) = checkout_open(cx);
    let address = cx
        .debug_bounds("launcher-address")
        .expect("the shared address");
    let host = cx
        .debug_bounds("launcher-host-row")
        .expect("the address's Host segment");
    let repository = cx
        .debug_bounds("launcher-repository-row")
        .expect("the address's Repository segment");
    let name = cx
        .debug_bounds("launcher-name-row")
        .expect("the optional Name row");
    let folder = cx
        .debug_bounds("launcher-folder-row")
        .expect("the full-width Folder line");
    assert!(address.contains(&host.origin) && address.contains(&repository.origin));
    assert_eq!(host.right(), repository.left());
    assert!(repository.right() <= address.right());
    assert!(folder.top() >= address.bottom().max(name.bottom()));
    assert_eq!(folder.left(), address.left());
    assert_eq!(folder.right(), name.right().max(address.right()));
    assert!(name_of(cx, "launcher-preview-heading").is_none());
    assert!(name_of(cx, "launcher-checkout-folder").is_none());
}

/// Repository history is read on opening. Typing filters locally without another command; Enter selects
/// the highlighted repository, closes the field's list and leaves Launch for a separate press.
#[gpui::test]
fn repository_history_loads_on_open_and_enter_picks_a_filtered_match(cx: &mut TestAppContext) {
    let (mut helm, cx, request) = checkout_open_with_history_pending(cx);
    click_in_body(cx, "launcher-repository");
    assert!(
        read(cx, |launcher, window, cx| launcher
            .repository_focused(window, cx)),
        "the premise: the click focused the repository field"
    );
    assert!(
        cx.debug_bounds("launcher-repository-list").is_some(),
        "focusing the field opened its list"
    );
    let repository = cx
        .debug_bounds("launcher-repository-row")
        .expect("the repository segment");
    let list = cx
        .debug_bounds("launcher-repository-list")
        .expect("the open suggestions list");
    assert_eq!(list.left(), repository.left());
    assert_eq!(list.size.width, repository.size.width);
    test_support::keys(cx, "a c m e /");
    request.answer(
        &mut helm,
        cx,
        200,
        repositories(&[("acme", "bar"), ("acme", "baz"), ("other", "x")]),
    );
    assert!(cx.debug_bounds("launcher-repository-list").is_some());
    assert!(
        name_of(cx, "launcher-repository-option-acme/bar").is_some()
            && name_of(cx, "launcher-repository-option-acme/baz").is_some()
            && name_of(cx, "launcher-repository-option-other/x").is_none(),
        "the matching repositories are listed"
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
    assert!(
        to(&sent(&mut helm, cx), "/api/sessions").is_empty(),
        "Enter chose; it launched nothing"
    );
}

/// If the helm cannot read its history, a valid typed pair remains selectable immediately.
#[gpui::test]
fn a_typed_pair_is_chosen_when_repository_history_fails(cx: &mut TestAppContext) {
    let (mut helm, cx, request) = checkout_open_with_history_pending(cx);
    click_in_body(cx, "launcher-repository");
    test_support::keys(cx, "a c m e / q u x");
    request.answer(&mut helm, cx, 504, json!("repository history read failed"));
    test_support::click(cx, "launcher-repository-option-acme/qux");
    assert_eq!(
        draft(cx).checkout.and_then(|checkout| checkout.repo),
        Some(repo("acme", "qux"))
    );
}

/// The repository row Enter would choose must remain visible while arrows wrap through an overflowing list. Checking
/// selection alone misses a clipped highlight, which gives no usable feedback while focus stays in the field.
#[gpui::test]
fn repository_arrows_keep_the_highlight_visible_through_wrapping(cx: &mut TestAppContext) {
    let (mut helm, cx, request) = checkout_open_with_history_pending(cx);
    click_in_body(cx, "launcher-repository");
    assert!(read(cx, |launcher, window, cx| launcher
        .repository_focused(window, cx)));
    let names: Vec<_> = (0..24).map(|index| format!("repo-{index:02}")).collect();
    let repos: Vec<_> = names.iter().map(|name| ("acme", name.as_str())).collect();
    request.answer(&mut helm, cx, 200, repositories(&repos));
    let row = |cx: &mut VisualTestContext, index| {
        let id = format!("launcher-repository-option-acme/repo-{index:02}");
        test_support::frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(id.as_str()))
            .unwrap_or_else(|| panic!("the list has no row for {id}"))
    };
    let viewport = test_support::bounds(cx, "launcher-repository-list");
    assert!(
        row(cx, 23).bounds.top() >= viewport.bottom(),
        "the premise: the last row starts below the viewport"
    );
    // The GPUI list has no highlight when it opens. Down establishes the first highlight before testing wraps.
    for (key, presses, expected) in [
        ("down", 1, 0),
        ("up", 1, 23),
        ("down", 1, 0),
        ("down", 12, 12),
    ] {
        for _ in 0..presses {
            test_support::keys(cx, key);
        }
        let row = row(cx, expected);
        assert_eq!(row.selected, Some(true), "row {expected} is highlighted");
        assert_eq!(
            row.visible_bounds, row.bounds,
            "all of row {expected} is visible"
        );
        assert!(
            read(cx, |launcher, window, cx| launcher
                .repository_focused(window, cx)),
            "arrows leave focus in the field"
        );
    }
}

/// Editing the name refreshes the preview for the chosen repository. The existing history still applies.
#[gpui::test]
fn editing_the_checkout_name_reads_no_repository_suggestions(cx: &mut TestAppContext) {
    let (mut helm, cx, request) = checkout_open_with_history_pending(cx);
    click_in_body(cx, "launcher-repository");
    request.answer(&mut helm, cx, 200, repositories(&[("acme", "bar")]));
    test_support::click(cx, "launcher-repository-option-acme/bar");
    let previews = to(&sent(&mut helm, cx), PREVIEW);
    assert_eq!(
        previews.len(),
        1,
        "the premise: the choice requested its preview"
    );
    helm.reply(
        previews[0].id,
        200,
        Some(preview_reply("/work/bar-1", "bar-1", 3)),
    );
    settle(cx);
    assert_eq!(
        checkout_folder(cx),
        "/work/bar-1",
        "the premise: the chosen repository has a ready preview"
    );

    click_in_body(cx, "launcher-name");
    assert!(read(cx, |launcher, window, cx| launcher
        .name
        .focus_handle(cx)
        .is_focused(window)));
    test_support::keys(cx, "x");
    test_support::advance(cx, Duration::from_millis(160));
    assert_eq!(draft(cx).shown_name(), "x");
    let commands = sent(&mut helm, cx);
    assert!(
        to(&commands, REPOSITORIES).is_empty(),
        "the name edit sends no repository history read"
    );
    let [preview] = &to(&commands, PREVIEW)[..] else {
        panic!("the name edit sends one preview");
    };
    assert_eq!(preview.body.as_ref().unwrap()["title"], json!("x"));
    helm.reply(
        preview.id,
        200,
        Some(preview_reply("/work/bar-x", "bar-x", 3)),
    );
    settle(cx);
    assert_eq!(checkout_folder(cx), "/work/bar-x");
    test_support::advance(cx, Duration::from_millis(160));
    assert!(
        to(&sent(&mut helm, cx), REPOSITORIES).is_empty(),
        "accepting the next preview sends no repository history read"
    );
}

/// Focusing Repository closes the command results. Enter on a query with no matching history must
/// leave the repository unselected, including after the field's suggestion list closes and reopens.
#[gpui::test]
fn repository_focus_closes_search_and_enter_on_no_match_chooses_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx, request) = checkout_open_with_history_pending(cx);
    test_support::click(cx, "launcher-search-row");
    assert!(
        super::view_tests::search_focused(cx),
        "the premise: the search box has focus"
    );
    test_support::keys(cx, "h o s t : m i s s i n g");
    assert_eq!(
        name_of(cx, "launcher-search-message").as_deref(),
        Some("No host matches")
    );
    click_in_body(cx, "launcher-repository");
    assert!(!super::view_tests::has_node(cx, "launcher-search-results"));
    assert!(read(cx, |launcher, window, cx| launcher
        .repository_focused(window, cx)));
    test_support::keys(cx, "i n v a l i d");
    assert_eq!(
        name_of(cx, "launcher-repository-empty").as_deref(),
        Some("Loading repositories…")
    );
    request.answer(&mut helm, cx, 200, repositories(&[]));
    let words = Some("No repository matches; enter owner/repo");
    assert_eq!(name_of(cx, "launcher-repository-empty").as_deref(), words);
    test_support::keys(cx, "enter");
    assert_eq!(name_of(cx, "launcher-repository-empty").as_deref(), words);
    test_support::keys(cx, "escape");
    assert!(!super::view_tests::has_node(
        cx,
        "launcher-repository-empty"
    ));
    test_support::keys(cx, "enter");
    assert_eq!(name_of(cx, "launcher-repository-empty").as_deref(), words);
    assert_eq!(draft(cx).checkout.and_then(|checkout| checkout.repo), None);
    assert_eq!(shown_refusal(cx), None);
    let commands = sent(&mut helm, cx);
    assert!(to(&commands, PREVIEW).is_empty());
    assert!(to(&commands, "/api/sessions").is_empty());
}

/// The command field's `gh:` query uses the history even on a Folder draft, including a valid typed
/// repository ahead of partial matches. It never issues a new request for each edit.
#[gpui::test]
fn gh_search_offers_recent_repositories_and_a_valid_typed_pair(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, command_draft(1));
    let request = hold_repositories(&mut helm, cx);
    assert_eq!(
        draft(cx).destination_kind(),
        DestinationKind::Folder,
        "the premise: a folder destination"
    );
    test_support::click(cx, "launcher-search-row");
    assert!(
        super::view_tests::search_focused(cx),
        "the premise: the search box has focus"
    );
    test_support::keys(cx, "g h : w i d g");
    assert_eq!(
        name_of(cx, "launcher-search-message").as_deref(),
        Some("Loading repositories…")
    );
    request.answer(
        &mut helm,
        cx,
        200,
        repositories(&[("acme", "widgets"), ("other", "x")]),
    );
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Managed checkout: acme/widgets")
    );
    assert!(
        name_of(cx, "launcher-search-option-1").is_none(),
        "only the match"
    );
    assert_eq!(
        name_of(cx, "launcher-search-group-GitHub repositories").as_deref(),
        Some("GitHub repositories")
    );

    test_support::keys(cx, "/ n e w");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Managed checkout: widg/new"),
        "the typed pair is offered before any answer about it"
    );
    test_support::keys(cx, "enter");
    assert_eq!(draft(cx).destination_kind(), DestinationKind::Checkout);
    assert_eq!(
        draft(cx).checkout.and_then(|checkout| checkout.repo),
        Some(repo("widg", "new"))
    );
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.search.text(cx).to_string()),
        ""
    );
}

/// An invalid `gh:` query must hold Launch even on an otherwise complete Folder draft.
#[gpui::test]
fn a_gh_query_holds_launch_until_a_repository_is_chosen(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, command_draft(1));
    let request = hold_repositories(&mut helm, cx);
    test_support::click(cx, "launcher-search-row");
    assert!(
        super::view_tests::search_focused(cx),
        "the premise: the search box has focus"
    );
    test_support::keys(cx, "g h : x");
    request.answer(&mut helm, cx, 200, repositories(&[]));
    assert!(cx.debug_bounds("launcher-repository-row").is_none());
    assert_eq!(
        name_of(cx, "launcher-search-message").as_deref(),
        Some("No repository matches; enter owner/repo")
    );
    test_support::keys(cx, "escape");
    test_support::click(cx, "launcher-launch");
    assert_eq!(
        shown_refusal(cx),
        Some(format!("Not launched. {SELECT_REPOSITORY}"))
    );
    assert!(
        to(&sent(&mut helm, cx), "/api/sessions").is_empty(),
        "no create"
    );

    test_support::click(cx, "launcher-destination-Managed checkout");
    click_in_body(cx, "launcher-repository");
    test_support::keys(cx, "a c m e / b a r");
    test_support::click(cx, "launcher-repository-option-acme/bar");
    assert_eq!(
        draft(cx).checkout.and_then(|checkout| checkout.repo),
        Some(repo("acme", "bar"))
    );
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.search.text(cx).to_string()),
        "",
        "the pick answered the gh: query"
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
    assert_folder_status(
        cx,
        "launcher-preview-pending",
        "launcher-preview-message",
        "Waiting for a current checkout preview",
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
    assert_folder_status(
        cx,
        "launcher-preview-ready",
        "launcher-preview-path",
        "/work/bar-1",
    );
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

/// A refused preview leaves no path and puts the helm's words in the same Folder status. The missing-folder
/// refusal must remain visible, and Launch still waits for an accepted preview.
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
    assert_folder_status(
        cx,
        "launcher-preview-failed",
        "launcher-preview-error",
        words,
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
    click_in_body(cx, "launcher-name");
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
        let launcher = layers::last_opened::<super::view::Launcher>(cx).unwrap();
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

/// A preview for the original host cannot authorize a checkout on the new host. Check after the original reply and
/// while the replacement is still held, so the replacement cannot mask stale output. The new host must also reveal
/// its missing checkout folder through launch history even though the repository stays selected.
#[gpui::test]
fn switching_hosts_drops_the_old_preview_and_checks_the_new_hosts_setup(cx: &mut TestAppContext) {
    // The fixture's host10 is a second connected host with its own installation identity.
    let fleet = super::view_tests::fixture_hosts(&[0, 9]);
    let (mut helm, cx) = super::view_tests::launcher_with_hosts(cx, command_draft(1), fleet);
    let reads = sent(&mut helm, cx);
    assert_eq!(to(&reads, "/api/launch-history?host=1").len(), 1);
    for command in reads {
        reply_checkout_launcher_read(&mut helm, command, false);
    }
    settle(cx);
    test_support::click(cx, "launcher-destination-Managed checkout");
    assert!(cx.debug_bounds("launcher-checkout-setup").is_none());
    edit(cx, |draft| draft.choose_repository(repo("acme", "bar")));
    let previews = to(&sent(&mut helm, cx), PREVIEW);
    let [first] = &previews[..] else {
        panic!("the premise: host1's preview is held");
    };
    assert_eq!(first.body.as_ref().unwrap()["host"], json!(1));

    test_support::click(cx, "launcher-host");
    test_support::click(cx, "launcher-host-option-10");
    assert_eq!(draft(cx).host, Some(10), "the picker changed hosts");
    assert_eq!(
        draft(cx).checkout.and_then(|checkout| checkout.repo),
        Some(repo("acme", "bar"))
    );
    test_support::advance(cx, Duration::from_millis(160));
    let pending = sent(&mut helm, cx);
    assert!(to(&pending, PREVIEW).is_empty(), "a preview is already out");
    assert!(to(&pending, REPOSITORIES).is_empty());
    assert_eq!(
        to(&pending, "/api/launch-history?host=10").len(),
        1,
        "the new host is asked about its checkout folder despite the repository staying selected"
    );

    helm.reply(first.id, 200, Some(preview_reply("/work/old", "old", 3)));
    settle(cx);
    assert_eq!(
        checkout_folder(cx),
        "",
        "host1's late success has no authority"
    );
    assert!(
        cx.debug_bounds("launcher-checkout-setup").is_none(),
        "the new host's launch history is still held"
    );
    // These reads were queued while the first preview was out. The replacement preview is now behind them.
    for command in pending {
        let reply = if command.path == "/api/launch-history?host=10" {
            Some(checkout_host_history(true, 3))
        } else {
            None
        };
        helm.reply(command.id, 200, reply);
    }
    settle(cx);
    let commands = sent(&mut helm, cx);
    let previews = to(&commands, PREVIEW);
    let [preview] = &previews[..] else {
        panic!("the new host's preview is now out");
    };
    let setup_reads = to(&commands, "/api/checkout-root");
    let [setup_read] = &setup_reads[..] else {
        panic!("the visible setup field reads the default checkout folder");
    };
    assert_eq!(preview.body.as_ref().unwrap()["host"], json!(10));
    assert_eq!(preview.body.as_ref().unwrap()["repo"], json!("acme/bar"));
    assert_eq!(
        checkout_folder(cx),
        "",
        "the replacement preview is still held"
    );
    assert!(
        cx.debug_bounds("launcher-checkout-setup").is_some(),
        "the new host's history offered checkout-folder setup"
    );
    test_support::click(cx, "launcher-launch");
    assert!(to(&sent(&mut helm, cx), "/api/sessions").is_empty());
    let mut reply = preview_reply("/work/new", "new", 3);
    reply["host"] = json!("10");
    reply["installation_identity"] = json!("identity10");
    helm.reply(preview.id, 200, Some(reply));
    helm.reply(setup_read.id, 200, Some(json!({"root": null})));
    settle(cx);
    assert_eq!(checkout_folder(cx), "/work/new");
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
/// offers no history and observes no new configuration revision. Settle both reads before the test proceeds.
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
            custom_agent: None,
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
/// helm, so a Clone does not carry it into the editable field; the old launcher carried it, showing one with an
/// invisible character escaped behind a "Clear repository" button. The Clone still opens on a managed checkout, with
/// nothing chosen, so one press of Launch cannot start it inside the source's own checkout (SPEC.md: "Empty, invalid or
/// unselected repository input cannot launch using a previous folder"). Specified: for a capitalized and a
/// formatting-character owner, the Clone is a managed checkout with no repository chosen and an empty field.
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

/// Answer what the window sent on opening in send order. Give the catalog no models and the other commands before
/// the first checkout preview an empty answer, as `answer_catalog` does. Leave that preview and everything after it
/// unanswered. A draft that opens on a chosen repository asks for a preview at once.
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

/// Submit the inline setup with a repository already chosen and its preview refused for lack of a folder.
/// This is the state the launcher has to recover from after either a save or a CLI change.
fn submit_inline_setup(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext, u64) {
    let (helm, cx) = launcher_on(cx, command_draft(1));
    submit_inline_setup_in(helm, cx)
}

/// Use the open fixture's host roster to submit the same initial setup on host1.
fn submit_inline_setup_in(
    mut helm: FakeHelm,
    cx: &mut VisualTestContext,
) -> (FakeHelm, &mut VisualTestContext, u64) {
    let reads = sent(&mut helm, cx);
    assert_eq!(to(&reads, "/api/launch-history?host=1").len(), 1);
    for command in reads {
        reply_checkout_launcher_read(&mut helm, command, true);
    }
    test_support::click(cx, "launcher-destination-Managed checkout");
    settle(cx);
    assert!(
        cx.debug_bounds("launcher-checkout-setup").is_some(),
        "the field shows"
    );
    let field_read = sent(&mut helm, cx);
    let [get] = &to(&field_read, "/api/checkout-root")[..] else {
        panic!("the field reads the default folder");
    };
    helm.reply(
        get.id,
        200,
        Some(json!({"root": null, "compare_on_save": true})),
    );
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
    assert_eq!(
        save.body,
        Some(json!({"root": "~/checkouts", "expected": null}))
    );
    (helm, cx, save.id)
}

/// Settle the held save and the reads it starts. The focus assertions depend on the setup field having been removed,
/// so both paths wait for the host to confirm its new checkout folder.
fn finish_inline_setup(helm: &mut FakeHelm, cx: &mut VisualTestContext, save: u64) {
    helm.reply(save, 204, None);
    settle(cx);
    let after = sent(helm, cx);
    assert!(to(&after, PREVIEW).is_empty(), "wait for the new revision");
    assert_eq!(
        to(&after, "/api/launch-history?host=1").len(),
        1,
        "setup is checked again"
    );
    assert!(
        to(&after, REPOSITORIES).is_empty(),
        "the global repository history is still current"
    );
    // Answered in the order sent, as the fake requires; the field reads its route again after a done save.
    for command in after {
        let body = match command.path.as_str() {
            "/api/launch-history?host=1" => checkout_host_history(false, 4),
            "/api/checkout-root" => json!({"root": "~/checkouts", "compare_on_save": true}),
            other => panic!("unexpected command after the save: {other}"),
        };
        helm.reply(command.id, 200, Some(body));
    }
    settle(cx);
    assert_single_refreshed_preview(helm, cx, "/home/user/checkouts/bar-1");
    assert!(
        cx.debug_bounds("launcher-checkout-setup").is_none(),
        "the host needs no setup now"
    );
}

/// A new configuration revision needs one preview. Once it answers the folder is ready without another RPC.
fn assert_single_refreshed_preview(helm: &mut FakeHelm, cx: &mut VisualTestContext, cwd: &str) {
    let commands = sent(helm, cx);
    let [preview] = commands.as_slice() else {
        panic!("one refreshed preview: {commands:?}");
    };
    assert_eq!(preview.path, PREVIEW);
    helm.reply(preview.id, 200, Some(preview_reply(cwd, "bar-1", 4)));
    settle(cx);
    assert_eq!(checkout_folder(cx), cwd);
    assert!(to(&sent(helm, cx), PREVIEW).is_empty());
}

/// This one launcher must be usable as soon as its setup succeeds. Saving must check the preview and
/// checkout-folder answer again, then move focus to the repository field if it finds setup can go away.
#[gpui::test]
fn a_host_without_a_checkout_folder_offers_the_setup_inline(cx: &mut TestAppContext) {
    let (mut helm, cx, save) = submit_inline_setup(cx);
    finish_inline_setup(&mut helm, cx, save);
    assert!(
        read(cx, |launcher, window, cx| launcher
            .repository_focused(window, cx)),
        "focus moved on to the repository field"
    );
}

/// SPEC.md, "Who may move focus": a save that finishes after another click can refresh the launcher without pulling
/// focus away from the field the user chose while the save was still pending.
#[gpui::test]
fn a_late_inline_checkout_save_keeps_focus_in_the_new_field(cx: &mut TestAppContext) {
    let (mut helm, cx, save) = submit_inline_setup(cx);
    click_in_body(cx, "launcher-name");
    let name_focused = |cx: &mut VisualTestContext| {
        read(cx, |launcher, window, cx| {
            launcher.name.focus_handle(cx).is_focused(window)
        })
    };
    assert!(name_focused(cx), "the user moved to the name during Save");
    finish_inline_setup(&mut helm, cx, save);
    assert!(name_focused(cx), "the completed save left focus alone");
}

/// If the launch-history recheck fails, the supervisor can still preview the newly saved checkout folder.
#[gpui::test]
fn a_failed_checkout_folder_recheck_still_retries_the_preview(cx: &mut TestAppContext) {
    let (mut helm, cx, save) = submit_inline_setup(cx);
    helm.reply(save, 204, None);
    settle(cx);
    let after = sent(&mut helm, cx);
    assert_eq!(to(&after, "/api/launch-history?host=1").len(), 1);
    assert!(
        to(&after, PREVIEW).is_empty(),
        "the local read is still pending"
    );
    for command in after {
        match command.path.as_str() {
            "/api/launch-history?host=1" => helm.reply(command.id, 500, None),
            "/api/checkout-root" => helm.reply(
                command.id,
                200,
                Some(json!({"root": "~/checkouts", "compare_on_save": true})),
            ),
            other => panic!("unexpected read after the save: {other}"),
        }
    }
    settle(cx);
    assert_single_refreshed_preview(&mut helm, cx, "/home/user/checkouts/bar-1");
}

/// Changing hosts while setup is rechecked still needs the saved revision. The first host's reply
/// is obsolete and cannot release the preview for the new destination.
#[gpui::test]
fn switching_hosts_during_checkout_setup_waits_for_the_new_hosts_revision(cx: &mut TestAppContext) {
    let fleet = super::view_tests::fixture_hosts(&[0, 9]);
    let (helm, cx) = super::view_tests::launcher_with_hosts(cx, command_draft(1), fleet);
    let (mut helm, cx, save) = submit_inline_setup_in(helm, cx);
    helm.reply(save, 204, None);
    settle(cx);
    let first = sent(&mut helm, cx);
    assert_eq!(to(&first, "/api/launch-history?host=1").len(), 1);
    assert!(to(&first, PREVIEW).is_empty());

    test_support::click(cx, "launcher-host");
    test_support::click(cx, "launcher-host-option-10");
    assert_eq!(draft(cx).host, Some(10));
    let second = sent(&mut helm, cx);
    let [history] = second.as_slice() else {
        panic!("wait only for the new host's history: {second:?}");
    };
    assert_eq!(history.path, "/api/launch-history?host=10");

    for command in first {
        let body = match command.path.as_str() {
            "/api/launch-history?host=1" => checkout_host_history(false, 4),
            "/api/checkout-root" => json!({"root": "~/checkouts", "compare_on_save": true}),
            other => panic!("unexpected read after the save: {other}"),
        };
        helm.reply(command.id, 200, Some(body));
    }
    settle(cx);
    assert!(
        to(&sent(&mut helm, cx), PREVIEW).is_empty(),
        "host1 cannot release host10's preview"
    );
    helm.reply(history.id, 200, Some(checkout_host_history(false, 4)));
    settle(cx);
    let commands = sent(&mut helm, cx);
    let [preview] = commands.as_slice() else {
        panic!("one preview on the new host: {commands:?}");
    };
    assert_eq!(preview.path, PREVIEW);
    assert_eq!(preview.body.as_ref().unwrap()["host"], json!(10));
    let cwd = "/work/on-host10";
    let mut reply = preview_reply(cwd, "on-host10", 4);
    reply["host"] = json!("10");
    reply["installation_identity"] = json!("identity10");
    helm.reply(preview.id, 200, Some(reply));
    settle(cx);
    assert_eq!(checkout_folder(cx), cwd);
    assert!(to(&sent(&mut helm, cx), PREVIEW).is_empty());
}

/// The launcher's pending folder recheck cannot follow a host while it is hidden for Resume.
/// The wake uses its new connection's preview and completes the original press.
#[gpui::test]
fn a_checkout_wake_retires_a_pending_folder_recheck(cx: &mut TestAppContext) {
    checkout_wake_during_folder_save(cx, true);
}

/// A Save reply that arrives after Resume has been pressed must not add a visible-only recheck to the held launch.
#[gpui::test]
fn a_checkout_wake_ignores_a_late_folder_save_reply(cx: &mut TestAppContext) {
    checkout_wake_during_folder_save(cx, false);
}

/// Hold a save or its follow-up history read through the same wake. Both must preserve the original press.
fn checkout_wake_during_folder_save(cx: &mut TestAppContext, save_before_wake: bool) {
    let fleet = fixture_hosts(&[0, 11]);
    let mut host: HostView = serde_json::from_value(fleet.entries[1].value.clone()).unwrap();
    assert_eq!(host.id, 12);
    assert!(matches!(host.state, HostStateView::Suspended { .. }));
    let (helm, cx) = launcher_with_hosts(cx, command_draft(1), fleet);
    let (mut helm, cx, save) = submit_inline_setup_in(helm, cx);
    if save_before_wake {
        helm.reply(save, 204, None);
        settle(cx);
    }
    let old_reads = sent(&mut helm, cx);
    assert_eq!(
        to(&old_reads, "/api/launch-history?host=1").len(),
        usize::from(save_before_wake)
    );
    assert!(to(&old_reads, PREVIEW).is_empty());

    test_support::click(cx, "launcher-host");
    test_support::click(cx, "launcher-host-option-12");
    assert_eq!(draft(cx).host, Some(host.id));
    assert_eq!(
        draft(cx).checkout.and_then(|checkout| checkout.repo),
        Some(repo("acme", "bar"))
    );
    let resume = press_checkout_wake(&mut helm, cx);

    if !save_before_wake {
        helm.reply(save, 204, None);
    }
    for command in old_reads {
        let reply = match command.path.as_str() {
            "/api/launch-history?host=1" => checkout_host_history(false, 4),
            "/api/checkout-root" => json!({"root": "~/checkouts", "compare_on_save": true}),
            other => panic!("unexpected read after the save: {other}"),
        };
        helm.reply(command.id, 200, Some(reply));
    }
    helm.reply(resume.id, 200, None);
    settle(cx);
    // The footer's editor can reload its own saved value. The hidden launcher sends no setup recheck of its own.
    for command in sent(&mut helm, cx) {
        assert_eq!(command.path, "/api/checkout-root");
        helm.reply(
            command.id,
            200,
            Some(json!({"root": "~/checkouts", "compare_on_save": true})),
        );
    }
    connect_checkout_host(&mut helm, cx, &mut host);
    let preview = hold_woken_preview(&mut helm, cx, &host, "");
    let mut reply = woken_checkout_reply(&host, "bar-1");
    reply["config_revision"] = json!(4);
    preview.reply(&mut helm, 200, reply);
    settle(cx);
    let commands = sent(&mut helm, cx);
    let [create] = commands.as_slice() else {
        panic!("the original press should send one create: {commands:?}");
    };
    assert_eq!(create.path, "/api/sessions");
    let body = create.body.as_ref().unwrap();
    assert_eq!(body["expected_incarnation"], host.incarnation);
    assert_eq!(body["github_checkout"]["preview"]["config_revision"], 4);
    assert!(!test_support::any_open(cx));
}

/// A new sandbox uses the same repository history even when the currently selected host needs a checkout folder.
/// Its Folder line offers that choice before it can say where the clone will go.
#[gpui::test]
fn new_sandbox_uses_global_repositories_without_the_source_hosts_checkout_folder(
    cx: &mut TestAppContext,
) {
    use super::environment_launch_tests::{plain_preferences, window_with};
    use super::view_tests::{fixture_hosts, pick_host_menu_row};
    use farhelm_proto::tensorlake::SshKeyStatus;

    let (mut helm, cx) = window_with(
        cx,
        command_draft(1),
        fixture_hosts(&[0, 1, 2]),
        plain_preferences(),
    );
    let reads = sent(&mut helm, cx);
    assert_eq!(to(&reads, REPOSITORIES).len(), 1);
    assert_eq!(to(&reads, "/api/launch-history?host=1").len(), 1);
    for read in reads {
        match read.path.as_str() {
            REPOSITORIES => helm.reply(read.id, 200, Some(repositories(&[("acme", "bar")]))),
            "/api/tensorlake" => helm.reply(
                read.id,
                200,
                Some(json!(super::environment_tests::account(
                    true,
                    SshKeyStatus::NotCreated
                ))),
            ),
            _ => reply_checkout_launcher_read(&mut helm, read, true),
        }
    }
    settle(cx);
    test_support::click(cx, "launcher-destination-Managed checkout");
    assert!(
        cx.debug_bounds("launcher-checkout-setup").is_some(),
        "the original host has no folder configured"
    );
    pick_host_menu_row(cx, "launcher-host-option-New sandbox");
    assert!(draft(cx).target.is_some());
    assert_eq!(draft(cx).destination_kind(), DestinationKind::Checkout);
    assert_folder_status(
        cx,
        "launcher-preview-unselected",
        "launcher-preview-message",
        "Select a repository",
    );
    click_in_body(cx, "launcher-repository");
    assert!(read(cx, |launcher, window, cx| launcher
        .repository_focused(window, cx)));
    assert!(cx.debug_bounds("launcher-checkout-setup").is_none());
    assert!(name_of(cx, "launcher-repository-option-acme/bar").is_some());
    assert!(
        to(&sent(&mut helm, cx), REPOSITORIES).is_empty(),
        "switching to New sandbox does not read again"
    );
    test_support::click(cx, "launcher-repository-option-acme/bar");
    assert_folder_status(
        cx,
        "launcher-preview-environment",
        "launcher-preview-message",
        "Cloned inside the new environment once it starts",
    );
}

/// A CLI change can resolve a failed preview while the launcher is open. The stale save must be refused,
/// but the folder discovered by the refetch still lets the launcher refresh that preview and dismiss setup.
#[gpui::test]
fn a_stale_inline_checkout_save_refreshes_the_preview_and_keeps_the_refusal(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, save) = submit_inline_setup(cx);
    let reason = "This checkout setting changed elsewhere.";
    helm.reply(save, 409, Some(json!(reason)));
    settle(cx);
    let reads = sent(&mut helm, cx);
    let [get] = reads.as_slice() else {
        panic!("one refetch should follow the refusal: {reads:?}");
    };
    assert_eq!(get.path, "/api/checkout-root");
    helm.reply(
        get.id,
        200,
        Some(json!({"root": "~/cli", "compare_on_save": true})),
    );
    settle(cx);
    let refreshed = sent(&mut helm, cx);
    let [history] = refreshed.as_slice() else {
        panic!("only checkout setup is rechecked before the preview: {refreshed:?}");
    };
    assert_eq!(history.path, "/api/launch-history?host=1");
    helm.reply(history.id, 200, Some(checkout_host_history(false, 4)));
    settle(cx);
    assert_single_refreshed_preview(&mut helm, cx, "/home/user/cli/bar-1");
    assert!(cx.debug_bounds("launcher-checkout-setup").is_none());
    assert_eq!(
        test_support::label(cx, "launcher-checkout-setup-refused").as_deref(),
        Some(reason)
    );
}

/// A Codex launch with YOLO chosen, into a managed checkout of `acme/bar` previewed ready, launched, refused for YOLO by
/// a host that asks first, and answered "Start guarded session instead": the moment the guarded launch waits for its
/// new preview. Returns that preview's command, unanswered, once it is checked that no create went out with it.
fn guarded_waiting(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext, FakeCommand) {
    guarded_waiting_named(cx, None, "/api/sessions")
}

/// Wait at the guarded answer's fresh preview, for a New or Replace request, optionally with a template base name.
fn guarded_waiting_named<'a>(
    cx: &'a mut TestAppContext,
    base: Option<&str>,
    route: &str,
) -> (FakeHelm, &'a mut VisualTestContext, FakeCommand) {
    let mut draft = LaunchDraft::new(
        Some(1),
        "~".into(),
        Some(farhelm_proto::LaunchPermission::Yolo),
    );
    let _ = draft.choose_harness(farhelm_proto::LaunchHarness::Codex, None, None);
    draft.choose_permissions(Some(farhelm_proto::LaunchPermission::Yolo));
    if route == REPLACE_A {
        draft.mode = Mode::ReplaceWith { source: "a".into() };
    }
    draft.choose_repository(repo("acme", "bar"));
    if let Some(base) = base {
        draft.edit_name(base.into());
        draft.set_template_name_kind(farhelm_proto::launcher::TemplateNameKind::Base);
    }
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
    let creates = to(&sent(&mut helm, cx), route);
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
    if route == REPLACE_A {
        show_source(&mut helm, cx, json!({"state": "exited", "exit_code": 0}));
        assert_eq!(name_of(cx, "launcher-source-warning"), None);
    }
    assert_eq!(
        name_of(cx, "prompt-alternative-0").as_deref(),
        Some("Start guarded session instead"),
        "the premise: the question offers the guarded answer"
    );
    test_support::click(cx, "prompt-alternative-0");
    let after = sent(&mut helm, cx);
    assert!(to(&after, route).is_empty(), "no launch before the preview");
    let [preview] = &to(&after, PREVIEW)[..] else {
        panic!("the new preview");
    };
    let preview = preview.clone();
    (helm, cx, preview)
}

const REPLACE_A: &str = "/api/sessions/a/replace";

/// Push the source independently of checkout preview settlement, as the helm does when its agent exits or restarts.
fn show_source(helm: &mut FakeHelm, cx: &mut VisualTestContext, status: Value) {
    use crate::session_list::tests::{change, listing_meta, session};
    helm.push(change(
        listing_meta(&["a"], 1, false),
        &[session("a").status(status)],
        &[],
    ));
    settle(cx);
}

/// SPEC.md: the YOLO question's guarded answer starts "the same launch guarded instead"; switching to Guarded is an
/// agent-choice change, which "invalidate[s] an undispatched preview". The old launcher waited for the new preview and
/// then submitted once (its `GuardedSubmitStamp`); refusing instead would turn the answer into a dead end. Specified:
/// once the new preview answers ready, one Replace goes out with that preview, guarded and without YOLO consent.
/// The restarted source must still be protected by the ended-and-tabless warning shown when the user answered.
#[gpui::test]
fn the_guarded_answer_waits_for_the_new_preview_then_launches_once(cx: &mut TestAppContext) {
    let (mut helm, cx, preview) = guarded_waiting_named(cx, None, REPLACE_A);
    show_source(&mut helm, cx, json!({"state": "running"}));
    assert!(name_of(cx, "launcher-source-warning").is_some());
    helm.reply(
        preview.id,
        200,
        Some(preview_reply("/work/bar-1", "bar-1", 3)),
    );
    settle(cx);
    let creates = to(&sent(&mut helm, cx), REPLACE_A);
    let [create] = &creates[..] else {
        panic!("one create once ready, not {}", creates.len());
    };
    let body = create.body.as_ref().unwrap();
    assert_eq!(body["only_if_nothing_alive"], json!(true));
    assert_eq!(body["with"]["launch"]["permissions"], json!("guarded"));
    assert_eq!(body["confirm_yolo"], json!(false));
    assert_eq!(
        body["with"]["github_checkout"]["preview"]["cwd"],
        json!("/work/bar-1")
    );
}

/// SPEC.md: Launch "never submits automatically or silently chooses another directory". A hidden quick launch follows
/// its own base name's numbering, but the guarded answer's wait is in a launcher the user sees, whose name changing is
/// for the user to look at before pressing Launch again. Specified: a guarded wait on a template base name `review`,
/// whose new preview is answered taken and the next candidate's ready, sends no create.
#[gpui::test]
fn a_guarded_wait_does_not_follow_a_taken_base_name(cx: &mut TestAppContext) {
    let (mut helm, cx, preview) = guarded_waiting_named(cx, Some("review"), "/api/sessions");
    assert_eq!(
        preview.body.as_ref().unwrap()["title"],
        json!("review"),
        "the premise: the guarded wait previews the base name"
    );
    helm.reply(preview.id, 409, Some(json!(TAKEN)));
    settle(cx);
    let asked = answer_preview(
        &mut helm,
        cx,
        200,
        preview_reply("/work/bar-review-2", "bar-review-2", 3),
    );
    assert_eq!(
        asked["title"],
        json!("review-2"),
        "the premise: the name moved on"
    );
    assert!(
        to(&sent(&mut helm, cx), "/api/sessions").is_empty(),
        "no create"
    );
}

/// The old cases "guarded checkout answer ... is cancelled by edit" and "... by close": the waiting launch answers for
/// the draft as it was answered, so an edit (here the initial prompt, which no preview depends on, so only the edit's
/// own rule can cancel it) or closing the launcher cancels it. Specified: for each, the ready preview that follows sends
/// no create.
#[gpui::test]
fn an_edit_cancels_the_guarded_answers_waiting_launch(cx: &mut TestAppContext) {
    let (mut helm, cx, preview) = guarded_waiting(cx);
    click_in_body(cx, "launcher-prompt");
    test_support::keys(cx, "h i");
    assert_eq!(
        draft(cx).prompt,
        "hi",
        "the premise: the edit reached the draft"
    );
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
        let launcher =
            layers::last_opened::<super::view::Launcher>(cx).expect("a launcher is open");
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
    click_in_body(cx, "launcher-repository");
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
    quick_launch_into_a_checkout_from(cx, false)
}

/// [`quick_launch_into_a_checkout`], from New's own prefill when `prefill` is set: with no session listed, so the
/// prefill's folder default is the host's own and still pending, as a template shortcut's is with no session open.
fn quick_launch_into_a_checkout_from(
    cx: &mut TestAppContext,
    prefill: bool,
) -> (FakeHelm, &mut VisualTestContext, Vec<FakeCommand>) {
    use crate::session_list::tests::{listing, preferences, session};
    let (mut helm, _connection) = test_support::started(cx);
    let sessions = if prefill {
        listing(&[], &[])
    } else {
        listing(&["a"], &[session("a")])
    };
    helm.connect(vec![
        sessions,
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
        let draft = if prefill {
            let draft = super::entry::new_prefill(cx).expect("New's prefill");
            assert_eq!(
                draft.folder_default,
                super::draft::FolderDefault::Pending,
                "the premise: the prefill's folder default is pending"
            );
            draft
        } else {
            LaunchDraft::new(Some(1), "~".into(), None)
        };
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

/// A repository template's quick launch from New's own prefill, whose folder default is still pending, reads no launch
/// history: the folder does not wait for it in a managed checkout, and the answer's configuration revision would make
/// the preview it waits for stale, so it would show New instead of starting. Specified: no history read goes out, the
/// preview does, and its ready answer starts the one create with nothing shown.
#[gpui::test]
fn a_repository_quick_launch_from_news_prefill_reads_no_history(cx: &mut TestAppContext) {
    let (mut helm, cx, sent_after) = quick_launch_into_a_checkout_from(cx, true);
    assert!(
        sent_after
            .iter()
            .all(|command| !command.path.starts_with("/api/launch-history")),
        "no history read"
    );
    let [preview] = &to(&sent_after, PREVIEW)[..] else {
        panic!("the preview goes out at once");
    };
    // Were a history read out, its answer, with a newer configuration revision, would come first and make the
    // preview stale; answering it so here keeps the outcome below meaningful should one ever go out again.
    for read in sent_after
        .iter()
        .filter(|command| command.path.starts_with("/api/launch-history"))
    {
        helm.reply(
            read.id,
            200,
            Some(json!({ "launches": [], "folders": [], "checkout_config_revision": 9 })),
        );
    }
    helm.reply(
        preview.id,
        200,
        Some(preview_reply("/work/bar-1", "bar-1", 3)),
    );
    settle(cx);
    let creates = to(&sent(&mut helm, cx), "/api/sessions");
    assert_eq!(creates.len(), 1, "one create once ready");
    assert!(!test_support::any_open(cx), "the launcher was never shown");
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

// ---- Waking the existing checkout host ----------------------------------------------------------------------------

const WAKE_RESUME: &str = "/api/hosts/12/resume";

/// A complete form on a suspended sandbox. The repository is already chosen, so a single Launch press can
/// authorize the allocation. A typed title is kept exact, unlike a template's numbered base.
fn suspended_checkout<'a>(
    cx: &'a mut TestAppContext,
    title: Option<&str>,
) -> (FakeHelm, &'a mut VisualTestContext, HostView) {
    let hosts = fixture_hosts(&[0, 11]);
    let host: HostView = serde_json::from_value(hosts.entries[1].value.clone()).unwrap();
    assert_eq!(host.id, 12);
    assert!(host.identity.is_some());
    assert!(matches!(host.state, HostStateView::Suspended { .. }));
    let mut draft = command_draft(host.id);
    draft.choose_repository(repo("acme", "bar"));
    if let Some(title) = title {
        draft.edit_name(title.into());
    }
    let (mut helm, cx) = launcher_with_hosts(cx, draft, hosts);
    pre_wake_traffic(&mut helm, cx, title);
    (helm, cx, host)
}

/// The same suspended sandbox with its recorded identity missing. Both surfaces must explain why it cannot allocate
/// a checkout, even though a plain-folder launch can still try to wake it.
pub(crate) fn unverified_checkout_hosts() -> farhelm_proto::ui_channel::CollectionState {
    let mut hosts = fixture_hosts(&[0, 11]);
    assert_eq!(hosts.entries[1].key, "12");
    assert_eq!(hosts.entries[1].value["state"]["phase"], "suspended");
    hosts.entries[1].value["identity"] = Value::Null;
    hosts
}

/// A repository choice on an unverified suspended host never authorizes Resume, preview or create.
#[gpui::test]
fn an_unverified_suspended_host_cannot_allocate_a_checkout(cx: &mut TestAppContext) {
    let mut draft = command_draft(12);
    draft.choose_repository(repo("acme", "bar"));
    let (mut helm, cx) = launcher_with_hosts(cx, draft, unverified_checkout_hosts());
    let detail = "launcher-preview-message";
    assert_folder_status(cx, "launcher-preview-unverified", detail, UNVERIFIED_HOST);
    test_support::click(cx, "launcher-launch");
    assert_eq!(
        shown_refusal(cx).unwrap(),
        format!("Not launched. {UNVERIFIED_HOST}")
    );
    assert!(sent(&mut helm, cx).iter().all(|command| {
        !matches!(
            command.path.as_str(),
            WAKE_RESUME | PREVIEW | "/api/sessions"
        )
    }));
}

/// Verify the actual form is fully resolved up to the host connection. It names its allocation policy and
/// supplies no old folder or preview. Answer only unrelated reads before the press.
fn pre_wake_traffic(helm: &mut FakeHelm, cx: &mut VisualTestContext, title: Option<&str>) {
    let expected = if title.is_some() {
        "The folder is named after the session when the host is ready"
    } else {
        "The folder is picked automatically when the host is ready"
    };
    assert_folder_status(
        cx,
        "launcher-preview-wake",
        "launcher-preview-message",
        expected,
    );
    assert!(checkout_folder(cx).is_empty());
    assert!(name_of(cx, "launcher-checkout-folder").is_none());
    assert!(name_of(cx, "launcher-preview-path").is_none());
    let unexpected = super::environment_launch_tests::answer_except(helm, cx, |command| {
        matches!(
            command.path.as_str(),
            WAKE_RESUME | PREVIEW | "/api/sessions"
        )
    });
    assert!(
        unexpected.is_empty(),
        "no launch traffic before the press: {unexpected:?}"
    );
}

/// A checkout press sends only Resume. The held form keeps New and lifecycle changes out through the preview.
fn press_checkout_wake(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> FakeCommand {
    test_support::click(cx, "launcher-launch");
    one_checkout_wake(helm, cx)
}

/// Verify that the press reached the suspended host and retained the create's ownership before it has a path.
fn one_checkout_wake(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> FakeCommand {
    let commands = sent(helm, cx);
    let [resume] = &commands[..] else {
        panic!("only one Resume expected, got {commands:?}");
    };
    assert_eq!(resume.path, WAKE_RESUME);
    assert_checkout_preparing(cx);
    resume.clone()
}

/// The ordinary window remains usable while the checkout owns its destination and the launcher slot.
fn assert_checkout_preparing(cx: &mut VisualTestContext) {
    assert!(!test_support::any_open(cx));
    assert!(name_of(cx, "launch-wake").is_some());
    assert!(name_of(cx, "launch-progress").is_none());
    cx.update(|_, cx| {
        assert!(super::launch::busy(cx));
        assert!(super::launch::host_lifecycle_refusal(12, cx).is_some());
    });
}

/// The exact preview belongs to the new connection. Holding it proves the wake cannot submit with its old path.
pub(crate) struct HeldCheckoutPreview {
    command: FakeCommand,
    trash_reads: Vec<FakeCommand>,
}

impl HeldCheckoutPreview {
    /// Release the preview first, then any trash reads sent while the window processed the host change.
    pub(crate) fn reply(self, helm: &mut FakeHelm, status: u16, body: Value) {
        helm.reply(self.command.id, status, Some(body));
        for command in self.trash_reads {
            reply_checkout_trash(helm, &command);
        }
    }
}

/// Match the window's unrelated reconnect traffic without letting the fixture consume a create or a Retry.
fn reply_checkout_trash(helm: &mut FakeHelm, command: &FakeCommand) {
    assert_eq!(command.path, "/api/checkout-trash/list");
    helm.reply(
        command.id,
        200,
        Some(crate::trash::tests::listing(&[], false)),
    );
}

/// Hold the sole fresh preview. Either side of it can contain an independent trash-list read.
pub(crate) fn hold_woken_preview(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    host: &HostView,
    title: &str,
) -> HeldCheckoutPreview {
    let commands = sent(helm, cx);
    let previews = to(&commands, PREVIEW);
    let [preview] = &previews[..] else {
        panic!("one fresh preview of {title:?} expected, got {commands:?}");
    };
    let mut trash_reads = Vec::new();
    for command in commands {
        if command.id < preview.id {
            reply_checkout_trash(helm, &command);
        } else if command.id > preview.id {
            assert_eq!(command.path, "/api/checkout-trash/list");
            trash_reads.push(command);
        }
    }
    settle(cx);
    let body = preview.body.as_ref().unwrap();
    assert_eq!(body["host"], host.id);
    assert_eq!(body["expected_incarnation"], host.incarnation);
    assert_eq!(body["title"], title);
    assert_checkout_preparing(cx);
    HeldCheckoutPreview {
        command: preview.clone(),
        trash_reads,
    }
}

/// Match the same installed host and connection that the outgoing preview names.
pub(crate) fn woken_checkout_reply(host: &HostView, basename: &str) -> Value {
    let HostStateView::Connected { identity, .. } = &host.state else {
        panic!("the host must have a live connection to answer a preview");
    };
    assert!(identity.is_some(), "the fixture verifies the installation");
    assert_eq!(identity, &host.identity);
    let cwd = format!("/work/{basename}");
    let mut reply = preview_reply(&cwd, basename, 3);
    reply["host"] = json!(host.id.to_string());
    reply["incarnation"] = json!(host.incarnation);
    reply["installation_identity"] = json!(identity);
    reply
}

/// Release the exact preview and check the complete request. An expired or replaced wake must never get here.
pub(crate) fn create_woken_checkout(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    host: &HostView,
    preview: HeldCheckoutPreview,
    basename: &str,
) -> FakeCommand {
    let expected = woken_checkout_reply(host, basename);
    preview.reply(helm, 200, expected.clone());
    settle(cx);
    let commands = sent(helm, cx);
    let [create] = &commands[..] else {
        panic!("only one create expected, got {commands:?}");
    };
    assert_eq!(create.path, "/api/sessions");
    let body = create.body.as_ref().unwrap();
    assert_eq!(body["expected_incarnation"], host.incarnation);
    assert_eq!(body["cwd"], expected["cwd"]);
    assert_eq!(
        body["github_checkout"]["preview"]["incarnation"],
        host.incarnation
    );
    assert_eq!(body["github_checkout"]["preview"]["cwd"], expected["cwd"]);
    assert!(!test_support::any_open(cx));
    create.clone()
}

/// Connect to the same installation on a fresh incarnation, which is the only legal connection for the preview.
pub(crate) fn connect_checkout_host(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    host: &mut HostView,
) {
    let identity = host.identity.as_deref().expect("verified fixture host");
    let connection = crate::hosts::tests::connected_with("0.40.0", identity, 1);
    super::wake_tests::push(helm, cx, host, connection);
}

/// A revealed form asks for its own preview. Answer every command in wire order, checking that the lost or
/// expired press sent neither a create nor a second Resume.
fn answer_reopened_checkout_preview(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    host: &HostView,
    basename: &str,
) {
    let commands = sent(helm, cx);
    assert_eq!(to(&commands, PREVIEW).len(), 1, "{commands:?}");
    for command in commands {
        let body = match command.path.as_str() {
            PREVIEW => Some(woken_checkout_reply(host, basename)),
            "/api/checkout-trash/list" => Some(crate::trash::tests::listing(&[], false)),
            "/api/sessions" | WAKE_RESUME => panic!("the spent press sent {command:?}"),
            _ => None,
        };
        helm.reply(command.id, 200, body);
    }
    settle(cx);
}

/// The approved checkout uses the single wake deadline to reach its connection. A preview held past that time
/// still belongs to the accepted press, so it can create without showing the form.
#[gpui::test]
fn a_woken_checkout_can_preview_after_the_connection_deadline(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = suspended_checkout(cx, None);
    let resume = press_checkout_wake(&mut helm, cx);
    helm.reply(resume.id, 200, None);
    settle(cx);
    test_support::advance(cx, Duration::from_secs(59));
    assert!(sent(&mut helm, cx).is_empty());
    connect_checkout_host(&mut helm, cx, &mut host);
    let preview = hold_woken_preview(&mut helm, cx, &host, "");
    test_support::advance(cx, Duration::from_secs(2));
    assert_checkout_preparing(cx);
    assert!(sent(&mut helm, cx).is_empty());
    let create = create_woken_checkout(&mut helm, cx, &host, preview, "bar-1");
    helm.reply(create.id, 409, Some(json!("The test refused the create")));
    settle(cx);
}

/// A late connection belongs to the host, not the expired press. The reopened form may preview for a new
/// press, but will not spend the old one.
#[gpui::test]
fn a_late_checkout_connection_cannot_spend_the_expired_press(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = suspended_checkout(cx, None);
    let resume = press_checkout_wake(&mut helm, cx);
    helm.reply(resume.id, 200, None);
    settle(cx);
    test_support::advance(cx, Duration::from_secs(59));
    assert_checkout_preparing(cx);
    test_support::advance(cx, Duration::from_secs(1));
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some(
            "Not launched. The host resumed, but this window did not see it connect within 60 seconds. The session was not launched; try again."
        ),
    );
    assert!(name_of(cx, "launch-wake").is_none());
    assert!(
        super::environment_launch_tests::answer_except(&mut helm, cx, |command| {
            matches!(
                command.path.as_str(),
                PREVIEW | "/api/sessions" | WAKE_RESUME
            )
        })
        .is_empty()
    );
    connect_checkout_host(&mut helm, cx, &mut host);
    answer_reopened_checkout_preview(&mut helm, cx, &host, "bar-1");
    assert!(to(&sent(&mut helm, cx), "/api/sessions").is_empty());
    assert!(name_of(cx, "launcher-launch").is_some());
}

/// A typed name authorizes exactly its matching folder. When the new connection finds that name taken,
/// re-show the form with the refusal so the user can choose a different title.
#[gpui::test]
fn a_woken_checkout_with_a_taken_typed_name_returns_to_the_form(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = suspended_checkout(cx, Some("taken"));
    let resume = press_checkout_wake(&mut helm, cx);
    helm.reply(resume.id, 200, None);
    settle(cx);
    connect_checkout_host(&mut helm, cx, &mut host);
    let preview = hold_woken_preview(&mut helm, cx, &host, "taken");
    preview.reply(&mut helm, 409, json!(TAKEN));
    settle(cx);
    assert_eq!(shown_refusal(cx).unwrap(), format!("Not launched. {TAKEN}"));
    assert_eq!(
        name_of(cx, "launcher-preview-error").as_deref(),
        Some(TAKEN)
    );
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.name.text(cx).to_string()),
        "taken"
    );
    assert!(name_of(cx, "launch-wake").is_none());
    let after = sent(&mut helm, cx);
    assert!(to(&after, PREVIEW).is_empty());
    assert!(to(&after, "/api/sessions").is_empty());
}

/// A race that takes the freshly previewed path spends the old press. The user sees the new path and decides
/// whether to send another create.
#[gpui::test]
fn a_woken_checkout_that_loses_its_path_previews_again_without_creating(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = suspended_checkout(cx, None);
    let resume = press_checkout_wake(&mut helm, cx);
    helm.reply(resume.id, 200, None);
    settle(cx);
    connect_checkout_host(&mut helm, cx, &mut host);
    let preview = hold_woken_preview(&mut helm, cx, &host, "");
    let create = create_woken_checkout(&mut helm, cx, &host, preview, "bar-1");
    let words =
        "the checkout preview is no longer current; obtain a new preview before resubmitting";
    helm.reply_marked(
        create.id,
        409,
        vec![farhelm_proto::ui_channel::HelmMarker::CreateUnaccepted],
        Some(json!(words)),
    );
    settle(cx);
    assert!(shown_refusal(cx).unwrap().contains("nothing was accepted"));
    answer_reopened_checkout_preview(&mut helm, cx, &host, "bar-2");
    assert_eq!(checkout_folder(cx), "/work/bar-2");
    assert!(to(&sent(&mut helm, cx), "/api/sessions").is_empty());
}

/// Read two setups while connected and establish that the recorded installation can still be identified when the
/// host suspends. Only the managed setup is useful without the connection that supplied its directory history.
fn suspend_with_recents(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext, HostView) {
    use super::recents_view_tests::{answer_history, chosen, history, launched};
    use farhelm_proto::{LaunchHarness, LaunchPermission};

    let mut hosts = fixture_hosts(&[0, 11]);
    let mut host: HostView = serde_json::from_value(hosts.entries[1].value.clone()).unwrap();
    let suspended = host.state.clone();
    assert!(matches!(suspended, HostStateView::Suspended { .. }));
    host.state =
        crate::hosts::tests::connected_with("0.40.0", host.identity.as_deref().unwrap(), 1);
    hosts.entries[1].value = serde_json::to_value(&host).unwrap();
    let open_draft = LaunchDraft::new(Some(host.id), "/work".into(), None);
    let (mut helm, cx) = launcher_with_hosts(cx, open_draft, hosts);
    let codex = chosen(
        LaunchHarness::Codex,
        None,
        None,
        Some(LaunchPermission::Guarded),
    );
    let mut checkout = launched("/work/old-checkout", &codex);
    checkout["host"] = json!(host.id);
    checkout["github_repo"] = json!({ "owner": "acme", "name": "bar" });
    let mut folder = launched("/work", &codex);
    folder["host"] = json!(host.id);
    answer_history(&mut helm, cx, &history(vec![checkout, folder]));
    assert!(
        name_of(cx, "launcher-recent-1").is_some(),
        "both rows were read"
    );

    super::wake_tests::push(
        &mut helm,
        cx,
        &mut host,
        crate::session_list::environment_tests::changing(
            farhelm_proto::tensorlake::LifecycleDirection::Suspending,
        ),
    );
    assert!(
        name_of(cx, "launcher-recent-0").is_none(),
        "the host is still changing"
    );
    super::wake_tests::push(&mut helm, cx, &mut host, suspended);
    assert!(
        name_of(cx, "launcher-recent-0").is_some_and(|name| name.contains("acme/bar")),
        "the same installation has the repository choice"
    );
    assert!(name_of(cx, "launcher-recent-1").is_none());
    assert!(
        draft(cx).checkout.is_none(),
        "the recent setup has not been applied"
    );
    let outstanding = super::environment_launch_tests::answer_except(&mut helm, cx, |command| {
        matches!(
            command.path.as_str(),
            WAKE_RESUME | PREVIEW | "/api/launch-history?host=12"
        )
    });
    assert!(
        outstanding.is_empty(),
        "suspension alone sends no Resume or checkout traffic"
    );
    (helm, cx, host)
}

/// The empty command pane applies and launches a recent checkout in the same Enter. The setup supplies its
/// repository choice while suspended, and only the new connection can supply its folder.
#[gpui::test]
fn recent_checkout_enter_wakes_the_verified_host_before_previewing(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = suspend_with_recents(cx);
    test_support::keys(cx, "down");
    assert!(
        crate::session_list::host_rail::tests::node(&test_support::frame(cx), "launcher-recent-0")
            .is_some_and(|node| node.selected == Some(true))
    );
    test_support::keys(cx, "enter");
    let resume = one_checkout_wake(&mut helm, cx);
    helm.reply(resume.id, 200, None);
    settle(cx);
    connect_checkout_host(&mut helm, cx, &mut host);
    let preview = hold_woken_preview(&mut helm, cx, &host, "");
    let create = create_woken_checkout(&mut helm, cx, &host, preview, "bar-2");
    helm.reply(create.id, 409, Some(json!("The test refused the create")));
    settle(cx);
}

/// Retargeting a suspended row must withdraw the previous installation's repository setup immediately, even
/// though neither row has a connected history key.
#[gpui::test]
fn a_changed_installation_loses_its_retained_checkout_setup(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = suspend_with_recents(cx);
    host.identity = Some("another-installation".into());
    let state = host.state.clone();
    super::wake_tests::push(&mut helm, cx, &mut host, state);
    assert!(name_of(cx, "launcher-recent-0").is_none());
    assert!(
        sent(&mut helm, cx)
            .iter()
            .all(|command| command.path != WAKE_RESUME)
    );
}

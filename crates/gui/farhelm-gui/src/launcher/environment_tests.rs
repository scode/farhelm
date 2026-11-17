//! Tests of a launch into a new environment on the draft and its request, with no window: the Runs on edits and their
//! effects, the folder rules a recipe target skips, the reasons a new container or sandbox cannot launch, the body it
//! sends, New sandbox's registration note, and when "Create a sandbox for this session?" is asked and in what words.
//! New sandbox's segment, recipe listing, press and question in the launcher are `sandbox_view_tests`'.
//!
//! SPEC.md, "Creation" (the Runs on bullet), "Container sessions" and "Sandbox sessions"; the old app's
//! `create_form/draft.rs` (`select_target`, `choose_target`), `sandbox_reason` in `create_form.rs` and
//! `recipe_refusal` in `sandbox_sessions.rs` at reference commit `24bc0580`.

use farhelm_proto::launcher::TemplateNameKind;
use farhelm_proto::recipes::RecipeView;
use farhelm_proto::tensorlake::{Settings, SshKeyStatus};
use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{
    CommandLaunch, LaunchTarget, Place, ProviderKind, SessionInfo, SessionLaunch, parse_github_repo,
};
use serde_json::json;

use super::draft::{
    DestinationKind, ENVIRONMENT_HOME, FolderDefault, Install, LaunchDraft, Mode, RunsOn, Tab,
};
use super::request::{
    Account, CHOOSE_RECIPE, CONTAINER_HOST_UNAVAILABLE, LOADING_ACCOUNT, LOADING_RECIPES, Preview,
    Recipes, SELECT_REPOSITORY, Target,
};
use crate::environment_ui::ProviderUi;

/// The helm's pinned host-list fixture's first three hosts (`view_tests::fixture_hosts`): host1 (connected, Podman
/// usable), host2 (Podman not installed) and host3 (an SSH host carrying no Podman check, as one the helm cannot route
/// a check to does).
fn fixture_hosts() -> Vec<HostView> {
    super::view_tests::fixture_hosts(&[0, 1, 2])
        .entries
        .into_iter()
        .map(|entry| serde_json::from_value(entry.value).unwrap())
        .collect()
}

/// A recipe as the pushed catalog carries it: `name` on `base` (the wire's `kind`, as `{"kind": "tensorlake_ubuntu"}`
/// for managed Ubuntu), with no snapshot built.
pub(super) fn recipe_json(name: &str, base: serde_json::Value) -> serde_json::Value {
    json!({
        "name": name,
        "size": { "cpus": 2, "memory_gb": 4, "disk_gb": 20 },
        "base": base,
        "packages": [],
        "claude": null, "codex": null,
        "claude_login_credential": null, "codex_login_credential": null, "github_token_credential": null,
        "tunnels": [],
        "snapshot": null,
        "build": { "state": "idle" },
        "prepared": null,
        "preparation": { "state": "idle" },
        "cleanup_pending": false,
    })
}

/// [`recipe_json`] decoded.
fn recipe(name: &str, base: serde_json::Value) -> RecipeView {
    serde_json::from_value(recipe_json(name, base)).unwrap()
}

/// The catalog the reason tests check against: `web` on managed Ubuntu, which a container runs, and `tl-image` on a
/// Tensorlake image, which it cannot.
fn catalog() -> Recipes {
    Recipes::Listed(vec![
        recipe("web", json!({ "kind": "tensorlake_ubuntu" })),
        recipe("tl-image", json!({ "kind": "image", "image": "ubuntu" })),
    ])
}

/// A new container of `recipe` on host `host`.
fn container(recipe: &str, host: i64) -> LaunchTarget {
    LaunchTarget::Environment {
        recipe: recipe.into(),
        place: Place::PodmanHost(host),
    }
}

/// A complete Command tab draft on host1 in `~`.
fn complete() -> LaunchDraft {
    let mut draft = LaunchDraft::new(Some(1), "~".into(), None);
    draft.choose_tab(Tab::Command);
    draft.edit_command("true".into());
    draft.choose_command_yolo(false);
    draft
}

/// [`complete`] in a new container of `web` on host1.
fn in_container() -> LaunchDraft {
    let mut draft = complete();
    draft.choose_runs_on(RunsOn::New(ProviderKind::Podman), Some(container("web", 1)));
    draft
}

/// The Tensorlake account while its read is out: what every container check here runs against, since a container
/// must not wait for an account it does not use.
static READING: Account = Account::Reading;

/// What the draft is checked against: `hosts`, no catalog read, `recipes`, and the account still being read.
fn target<'a>(hosts: &'a [HostView], recipes: &'a Recipes) -> Target<'a> {
    target_with(hosts, recipes, &READING)
}

/// [`target`], with the Tensorlake account as `account`.
fn target_with<'a>(
    hosts: &'a [HostView],
    recipes: &'a Recipes,
    account: &'a Account,
) -> Target<'a> {
    Target {
        hosts,
        catalog: None,
        preview: Preview::Waiting,
        recipes,
        account,
    }
}

// ---- The Runs on edits ----------------------------------------------------------------------------------------------

/// SPEC.md, "Container sessions": "Folder destinations use its home, without Browse or recent folders". A folder from
/// the host would name a place that does not exist in the new container. Specified: entering New container over a
/// typed folder taken from host1's history sets the target and makes the folder `~` again, untouched and bound to no
/// installation; the destination stays a folder, and the host stays as it was.
#[test]
fn entering_a_container_sets_the_home_folder() {
    let mut draft = complete();
    draft.take_history_folder(
        "/srv/app".into(),
        Install {
            host: 1,
            identity: None,
        },
    );
    assert!(
        draft.folder_install.is_some() && draft.folder.is_touched(),
        "the premise: a folder the user took, bound to host1's installation"
    );
    draft.choose_runs_on(RunsOn::New(ProviderKind::Podman), Some(container("web", 1)));
    assert_eq!(draft.target, Some(container("web", 1)));
    assert_eq!(draft.folder.value(), ENVIRONMENT_HOME);
    assert!(!draft.folder.is_touched(), "the home folder is untouched");
    assert_eq!(draft.folder_install, None, "and bound to no installation");
    assert_eq!(draft.destination_kind(), DestinationKind::Folder);
    assert_eq!(
        draft.host,
        Some(1),
        "the Host row's choice is kept for later"
    );
}

/// SPEC.md, "Container sessions": "The managed checkout is cloned inside it", and "Sandbox sessions": "Managed checkout
/// works normally". The old launcher kept "repository intent" across placement changes, so a repository chosen before
/// or after entering a new environment must survive both ways. Specified: a managed checkout of `owner/repo` stays
/// chosen through entering New container and leaving it again; inside the container the repository field takes typing
/// (which drops the choice, as on a host), another repository can be chosen, and Folder chosen, as on a host.
#[test]
fn a_managed_checkout_survives_entering_and_leaving() {
    let repo = parse_github_repo("owner/repo").unwrap();
    let mut draft = complete();
    draft.choose_repository(repo.clone());
    assert_eq!(
        draft
            .checkout
            .as_ref()
            .and_then(|checkout| checkout.repo.clone()),
        Some(repo.clone()),
        "the premise: owner/repo is chosen"
    );
    draft.choose_runs_on(RunsOn::New(ProviderKind::Podman), Some(container("web", 1)));
    assert_eq!(
        draft
            .checkout
            .as_ref()
            .and_then(|checkout| checkout.repo.clone()),
        Some(repo.clone()),
        "entering kept it"
    );
    draft.edit_repository_query("owner/x".into());
    assert_eq!(draft.destination_kind(), DestinationKind::Checkout);
    assert_eq!(
        draft
            .checkout
            .as_ref()
            .map(|checkout| (checkout.field_text(), checkout.repo.clone())),
        Some(("owner/x".to_owned(), None)),
        "typing inside the container drops the choice and keeps the text"
    );
    let other = parse_github_repo("owner/other").unwrap();
    draft.choose_repository(other.clone());
    assert_eq!(
        draft
            .checkout
            .as_ref()
            .and_then(|checkout| checkout.repo.clone()),
        Some(other.clone()),
        "a repository is chosen inside the container as on a host"
    );
    draft.choose_runs_on(RunsOn::ExistingHost, None);
    assert_eq!(
        draft
            .checkout
            .as_ref()
            .and_then(|checkout| checkout.repo.clone()),
        Some(other),
        "leaving kept it"
    );
    draft.choose_runs_on(RunsOn::New(ProviderKind::Podman), Some(container("web", 1)));
    draft.choose_destination(DestinationKind::Folder);
    assert_eq!(draft.destination_kind(), DestinationKind::Folder);
    assert_eq!(draft.folder.value(), ENVIRONMENT_HOME, "Folder is the home");
}

/// SPEC.md, "Creation": "An active segment keeps its explicit choices", and Existing host "shows the ordinary Host
/// picker". A second press of the chosen segment must not throw away the recipe and place the user picked. Specified:
/// with `web` on host2 chosen by hand, choosing New container again with another default changes nothing; Existing host
/// leaves the environment with the draft's host as it was.
#[test]
fn the_chosen_segment_keeps_its_choices_and_existing_host_gives_the_host_back() {
    let mut draft = in_container();
    draft.choose_container_host(2);
    draft.choose_recipe("api".into());
    draft.choose_runs_on(RunsOn::New(ProviderKind::Podman), Some(container("web", 1)));
    assert_eq!(
        draft.target,
        Some(container("api", 2)),
        "the explicit choices stay"
    );
    draft.choose_runs_on(RunsOn::ExistingHost, None);
    assert_eq!(draft.target, None);
    assert_eq!(draft.host, Some(1), "the Host row is back on host1");
}

/// The recipe and container host edits act only on the environment chosen, so a stray edit (a template, the search)
/// cannot put a draft in a new environment by the back door. Specified: without a target both do nothing; a segment
/// entered with a target of another provider, or none, does nothing either.
#[test]
fn recipe_and_host_edits_need_an_environment() {
    let mut draft = complete();
    draft.choose_recipe("web".into());
    draft.choose_container_host(1);
    assert_eq!(draft.target, None);
    draft.choose_runs_on(
        RunsOn::New(ProviderKind::Podman),
        Some(LaunchTarget::Environment {
            recipe: "web".into(),
            place: Place::TensorlakeAccount,
        }),
    );
    assert_eq!(
        draft.target, None,
        "a target of another provider is not entered"
    );
    draft.choose_runs_on(RunsOn::New(ProviderKind::Podman), None);
    assert_eq!(draft.target, None, "nor is a segment with no usable place");
}

/// SPEC.md, "Sandbox sessions": "Restart, Restart with, Replace and Replace with stay in the existing sandbox", and
/// "Container sessions" follows that. Both modes hold the host, which [`LaunchDraft::holds`] answers. Specified: in
/// Replace with and Restart with, entering New container changes nothing.
#[test]
fn restart_with_and_replace_with_keep_the_existing_environment() {
    let session = fixture_session();
    let mut replace = LaunchDraft::replace_with("a", &session, Some(1));
    assert!(
        matches!(replace.mode, Mode::ReplaceWith { .. }),
        "the premise: a Replace with draft"
    );
    replace.choose_runs_on(RunsOn::New(ProviderKind::Podman), Some(container("web", 1)));
    assert_eq!(replace.target, None, "Replace with");
    let mut restart = LaunchDraft::restart_with("a", &session, Some(1))
        .expect("the premise: the fixture's Codex launch offers Restart with");
    restart.choose_runs_on(RunsOn::New(ProviderKind::Podman), Some(container("web", 1)));
    assert_eq!(restart.target, None, "Restart with");
}

/// The helm's pinned session-list fixture's first session: a Codex launch in `~/src/widgets` on host2.
fn fixture_session() -> SessionInfo {
    serde_json::from_value(
        serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../../farhelm-helm/http-contract/session-list.json"
        ))
        .unwrap()["sessions"][0]
            .clone(),
    )
    .unwrap()
}

/// SPEC.md, "Sandbox sessions": Clone "targets the recipe for a fresh sandbox", and New beside a sandbox session uses
/// "the new sandbox's home Folder default, never the disposable host or its old folder". Both come in through one seed,
/// which must leave exactly what entering the segment by hand leaves, or the two ways into an environment would launch
/// differently. Specified: a Clone seeded with `web` on host1 holds that target and the folder `~` in place of the
/// source's, untouched and bound to no installation, with no host marked chosen, and Existing host then gives back the
/// host it held; a New seeded while its folder default is pending no longer waits for it; Replace with's seed changes
/// nothing.
#[test]
fn a_seeded_environment_has_the_segments_effects_and_chooses_nothing() {
    let session = fixture_session();
    let mut clone = LaunchDraft::clone_of(&session, Some(1));
    assert_ne!(
        clone.folder.value(),
        ENVIRONMENT_HOME,
        "the premise: the source's folder"
    );
    clone.seed_target(container("web", 1));
    assert_eq!(clone.target, Some(container("web", 1)));
    assert_eq!(clone.folder.value(), ENVIRONMENT_HOME);
    assert!(!clone.folder.is_touched() && clone.folder_install.is_none());
    assert!(!clone.host_chosen, "a seed is no hand pick");
    clone.choose_runs_on(RunsOn::ExistingHost, None);
    assert_eq!((clone.target, clone.host), (None, Some(1)));

    let mut new = LaunchDraft::new(Some(1), "~".into(), None);
    new.folder_default = FolderDefault::Pending;
    assert!(new.folder_default_waiting(), "the premise: New waits");
    new.seed_target(container("web", 1));
    assert!(
        !new.folder_default_waiting(),
        "a new environment's home waits for no host's history"
    );

    let mut replace = LaunchDraft::replace_with("a", &session, Some(1));
    let before = replace.clone();
    replace.seed_target(container("web", 1));
    assert_eq!(
        replace, before,
        "Replace with keeps the existing environment"
    );
}

/// A host is Existing host's choice: a host picked from the search or a template's host must leave a new environment,
/// or the draft would launch somewhere the Host row does not show. Specified: choosing host2 while in a container
/// clears the target and chooses host2.
#[test]
fn choosing_a_host_leaves_the_environment() {
    let mut draft = in_container();
    draft.choose_host(2, "~".into());
    assert_eq!(draft.target, None);
    assert_eq!(draft.host, Some(2));
}

/// SPEC.md: a new environment's folder destination is its home "without Browse or recent folders". Specified: while
/// in a container, a typed or picked folder and a history folder change nothing.
#[test]
fn the_home_folder_is_fixed_while_a_recipe_is_chosen() {
    let mut draft = in_container();
    let before = draft.clone();
    draft.edit_folder("/srv/app".into());
    draft.take_history_folder(
        "/srv/app".into(),
        Install {
            host: 1,
            identity: None,
        },
    );
    assert_eq!(draft, before);
}

/// A recipe target must neither wait for a host's folder default nor be refused for a folder's installation (the
/// launcher's handoff, after New's folder default and a folder's installation binding landed): the environment's home
/// is no host's folder. Specified: New's pending default with a container chosen does not wait; its later settling
/// seeds nothing into the home folder, and the draft launches. (No folder taken from an installation survives entering,
/// so the installation check has nothing to refuse.)
#[test]
fn a_recipe_target_skips_the_folder_default_and_installation() {
    let mut draft = complete();
    draft.folder_default = FolderDefault::Pending;
    draft.choose_runs_on(RunsOn::New(ProviderKind::Podman), Some(container("web", 1)));
    assert!(
        !draft.folder_default_waiting(),
        "nothing waits for host1's history"
    );
    draft.settle_folder_default(
        Some("/srv/app".into()),
        Install {
            host: 1,
            identity: None,
        },
    );
    assert_eq!(
        draft.folder.value(),
        ENVIRONMENT_HOME,
        "the history seeded nothing"
    );
    let hosts = fixture_hosts();
    let recipes = catalog();
    assert_eq!(
        draft.refusal(target(&hosts, &recipes), None),
        None,
        "and the draft launches"
    );
}

// ---- Reasons and the body -------------------------------------------------------------------------------------------

/// SPEC.md, "Creation": an incomplete launch "says why beside Launch". The old launcher's reasons for a new
/// environment, in its order: the catalog loading or unreadable (its own words), the recipe missing or one the provider
/// cannot run, then the container host gone or unusable with its reason (one the helm cannot route a check to cannot
/// run containers). Specified: each of those states gives its reason, and a recipe the container runs on a usable host
/// gives none.
#[test]
fn a_new_container_is_refused_with_the_old_reasons() {
    let hosts = fixture_hosts();
    let recipes = catalog();
    let listed = &recipes;
    let (loading, unreadable) = (
        Recipes::Loading,
        Recipes::Unreadable("the catalog is broken".into()),
    );
    let with = |draft: LaunchDraft, recipes| draft.refusal(target(&hosts, recipes), None);
    assert_eq!(
        with(in_container(), &loading).as_deref(),
        Some(LOADING_RECIPES)
    );
    assert_eq!(
        with(in_container(), &unreadable).as_deref(),
        Some("the catalog is broken")
    );
    let mut missing = in_container();
    missing.choose_recipe("gone".into());
    assert_eq!(
        with(missing, listed).as_deref(),
        Some("Recipe \"gone\" is unavailable")
    );
    let mut unchosen = in_container();
    unchosen.choose_recipe(String::new());
    assert_eq!(with(unchosen, listed).as_deref(), Some(CHOOSE_RECIPE));
    let mut unsupported = in_container();
    unsupported.choose_recipe("tl-image".into());
    assert_eq!(
        with(unsupported, listed).as_deref(),
        Some("This recipe's base cannot run as a container")
    );
    let mut unusable = in_container();
    unusable.choose_container_host(2);
    assert_eq!(
        with(unusable, listed).as_deref(),
        Some("Podman is not installed on this host")
    );
    let mut unroutable = in_container();
    unroutable.choose_container_host(3);
    assert_eq!(
        with(unroutable, listed).as_deref(),
        Some("this host cannot run containers"),
        "host3 carries no Podman check"
    );
    let mut gone = in_container();
    gone.choose_container_host(99);
    assert_eq!(
        with(gone, listed).as_deref(),
        Some(CONTAINER_HOST_UNAVAILABLE)
    );
    assert_eq!(with(in_container(), listed), None);
}

/// A new environment has no host yet, so the Host row's reasons must not refuse it: the draft's kept host may be
/// disconnected or gone. Specified: a container draft whose kept host is host2 (connecting) or none launches.
#[test]
fn a_new_environment_needs_no_connected_host() {
    let hosts = fixture_hosts();
    let recipes = catalog();
    for host in [Some(2), None] {
        let mut draft = in_container();
        draft.host = host;
        assert_eq!(
            draft.refusal(target(&hosts, &recipes), None),
            None,
            "{host:?}"
        );
    }
}

/// SPEC_impl.md, "Tensorlake sandbox sessions": a recipe launch has its own route and "must never fall through to
/// ordinary creation"; the helm's environment create takes the recipe and place beside a create that names no host.
/// Specified: the ordinary create body is none for a container draft; its environment body names `web` on host1's
/// place, unconfirmed, with a create in `~` that carries no host or connection, the command launch and no checkout.
#[test]
fn a_new_container_sends_its_own_body() {
    let hosts = fixture_hosts();
    let recipes = catalog();
    let draft = in_container();
    let target = target(&hosts, &recipes);
    assert_eq!(draft.create_body(target), None, "never the ordinary create");
    let body = draft
        .environment_body(target)
        .expect("a complete draft has a body");
    assert_eq!(body.recipe, "web");
    assert_eq!(body.place, Place::PodmanHost(1));
    assert!(!body.confirmed);
    assert_eq!(body.request.host, None);
    assert_eq!(body.request.expected_incarnation, None);
    assert_eq!(body.request.cwd, ENVIRONMENT_HOME);
    assert_eq!(body.request.github_checkout, None);
    assert_eq!(
        body.request
            .command
            .as_ref()
            .map(|command| command.command.as_str()),
        Some("true")
    );
    assert_eq!(
        complete().environment_body(target),
        None,
        "an existing host has none"
    );
}

/// SPEC.md, "Managed checkouts": "A recipe-target environment launch is another exception. The user accepts its
/// recipe, place, repository and naming policy before the environment exists", so no installation is verified and no
/// folder previewed before it launches. Specified: in a container, a managed checkout with no repository chosen is
/// refused as on a host; with one chosen it launches with no preview and with no host chosen, so with no installation
/// identity to verify.
#[test]
fn a_checkout_in_a_new_environment_needs_only_a_repository() {
    let hosts = fixture_hosts();
    let recipes = catalog();
    let mut draft = in_container();
    draft.choose_destination(DestinationKind::Checkout);
    assert_eq!(
        draft.refusal(target(&hosts, &recipes), None).as_deref(),
        Some(SELECT_REPOSITORY)
    );
    draft.choose_repository(parse_github_repo("owner/repo").unwrap());
    draft.host = None;
    assert_eq!(
        draft.refusal(target(&hosts, &recipes), None),
        None,
        "no identity and no preview are asked for"
    );
}

/// SPEC_impl.md, "Tensorlake sandbox sessions": "For managed checkouts, carry repository and naming intent until the
/// new supervisor connects"; the old launcher sent the checkout with no preview and an empty folder. Specified: a
/// container draft with `owner/repo` chosen sends `github_checkout` naming it, with the name its checkout gets and no
/// preview, and an empty `cwd`.
#[test]
fn a_new_environment_carries_its_checkout_in_the_body() {
    let hosts = fixture_hosts();
    let recipes = catalog();
    let mut draft = in_container();
    draft.choose_repository(parse_github_repo("owner/repo").unwrap());
    draft.edit_name("work".into());
    let body = draft
        .environment_body(target(&hosts, &recipes))
        .expect("a complete draft has a body");
    let checkout = body
        .request
        .github_checkout
        .expect("the checkout is carried");
    assert_eq!(checkout.repo, "owner/repo");
    assert_eq!(checkout.title.as_deref(), Some("work"));
    assert_eq!(checkout.preview, None);
    assert_eq!(body.request.cwd, "");
    assert_eq!(body.request.title.as_deref(), Some("work"));
}

/// The name a managed checkout's body sends in a new environment, as the checkout's own title and the session's, and
/// the kind it is sent as.
fn sent_name(draft: &LaunchDraft) -> (Option<String>, Option<String>, Option<TemplateNameKind>) {
    let hosts = fixture_hosts();
    let recipes = catalog();
    let body = draft
        .environment_body(target(&hosts, &recipes))
        .expect("a complete draft has a body");
    let checkout_title = body
        .request
        .github_checkout
        .as_ref()
        .and_then(|checkout| checkout.title.clone());
    (checkout_title, body.request.title, body.name_kind)
}

/// SPEC.md, "Managed checkouts": "Changing hosts or repositories restarts the suffix search". A suffix the existing
/// host's previews moved a numbered name to says nothing about a new environment, where the helm numbers the base once
/// it can see what is taken; carried in, it would name a fresh container `review-2` where `review` is free (the old app
/// sent the template's raw name). Specified: a template base name `review`, previewed as taken on host1 and so moved to
/// `review-2`, is `review` again once New container is entered, and the body sends `review` as a base.
#[test]
fn a_numbered_name_starts_over_in_a_new_environment() {
    let mut draft = complete();
    draft.edit_name("review".into());
    draft.set_template_name_kind(TemplateNameKind::Base);
    draft.choose_repository(parse_github_repo("owner/repo").unwrap());
    assert!(draft.advance_checkout_name("review"));
    assert_eq!(
        draft.checkout_title(),
        "review-2",
        "the premise: host1's preview moved the name on"
    );
    draft.choose_runs_on(RunsOn::New(ProviderKind::Podman), Some(container("web", 1)));
    assert_eq!(draft.checkout_title(), "review");
    assert_eq!(
        sent_name(&draft),
        (
            Some("review".to_owned()),
            Some("review".to_owned()),
            Some(TemplateNameKind::Base)
        )
    );
}

/// The helm numbers a managed checkout's name in a new environment only when the body calls it a base (`name_kind`);
/// any other name is the user's exact one, which fails as a conflict where that folder exists. Clone's default
/// `<title>-clone` carries no template name kind of its own, so the body must call it a base, as the old app did for a
/// Clone source. Specified: a Clone of a session in a managed checkout of `owner/repo`, in a new container, sends
/// `source-clone` as a base; with another repository chosen it sends no session name and no kind (the copied title is
/// ignored there); with a typed name, that name and no kind.
#[test]
fn a_clones_default_name_goes_to_a_new_environment_as_a_base() {
    let repo = parse_github_repo("owner/repo").unwrap();
    let mut source = super::draft_tests::session(SessionLaunch::Command(CommandLaunch {
        command: "true".into(),
        yolo: false,
        agent: None,
        resume: None,
    }));
    source.github_repo = Some(repo.clone());
    let mut draft = LaunchDraft::clone_of(&source, Some(1));
    draft.choose_runs_on(RunsOn::New(ProviderKind::Podman), Some(container("web", 1)));
    assert_eq!(
        (draft.clone_source.as_ref(), draft.name_kind),
        (Some(&repo), None),
        "the premise: a Clone into its source's repository, with no template name kind"
    );
    assert_eq!(
        sent_name(&draft),
        (
            Some("source-clone".to_owned()),
            Some("source-clone".to_owned()),
            Some(TemplateNameKind::Base)
        )
    );

    draft.choose_repository(parse_github_repo("owner/other").unwrap());
    assert_eq!(
        sent_name(&draft),
        (Some(String::new()), None, None),
        "another repository"
    );

    draft.choose_repository(repo);
    draft.edit_name("mine".into());
    assert_eq!(
        sent_name(&draft),
        (Some("mine".to_owned()), Some("mine".to_owned()), None),
        "a typed name"
    );
}

// ---- New sandbox: the account's prerequisites -----------------------------------------------------------------------

/// [`recipe_json`] on managed Ubuntu with a snapshot built, as a sandbox starts from.
pub(crate) fn built_json(name: &str) -> serde_json::Value {
    let mut view = recipe_json(name, json!({ "kind": "tensorlake_ubuntu" }));
    view["snapshot"] = json!({
        "id": "snap", "built_at": 1, "base": { "kind": "tensorlake_ubuntu" }, "disk_gb": 20, "packages": [],
        "claude_version": null, "codex_version": null,
    });
    view
}

/// [`built_json`] decoded.
fn built(name: &str) -> RecipeView {
    serde_json::from_value(built_json(name)).unwrap()
}

/// A Tensorlake account as the helm answers it: an API key saved or not, and Farhelm's SSH key as `ssh_key`.
pub(crate) fn account(api_key_set: bool, ssh_key: SshKeyStatus) -> Settings {
    Settings {
        api_key_set,
        create_without_asking: false,
        helm_tag: "tag".into(),
        ssh_key,
    }
}

/// Farhelm's SSH key, made, with `command` as its registration command.
pub(crate) fn ready_key(command: &str) -> SshKeyStatus {
    SshKeyStatus::Ready {
        public_key: "ssh-ed25519 AAAA fixture".into(),
        public_key_path: "/state/tensorlake.pub".into(),
        name: "fixture".into(),
        register_command: command.into(),
    }
}

/// A new sandbox of `recipe`, on the Tensorlake account.
pub(super) fn sandbox(recipe: &str) -> LaunchTarget {
    LaunchTarget::Environment {
        recipe: recipe.into(),
        place: Place::TensorlakeAccount,
    }
}

/// [`complete`] in a new sandbox of `recipe`.
fn in_sandbox(recipe: &str) -> LaunchDraft {
    let mut draft = complete();
    draft.choose_runs_on(RunsOn::New(ProviderKind::Tensorlake), Some(sandbox(recipe)));
    draft
}

/// SPEC.md, "Sandbox sessions": "A recipe without a built snapshot is disabled with No snapshot yet. A missing API key
/// or absent or unreadable local Farhelm SSH key also disables it." A sandbox launched without them only fails minutes
/// later at the helm, so each is said before the press, and one at a time in the old launcher's order, so the user
/// fixes the first thing first. Specified: a sandbox of a built recipe says "Loading Tensorlake settings…" while the
/// account read is out, the read's own words once it failed, the API key before the SSH key when both are missing, then
/// "Create Farhelm's SSH key in Settings → Tensorlake", then the helm's reason for an unreadable key written escaped
/// (it is the helm's text); with the key ready, an unbuilt recipe says "No snapshot yet", a base Tensorlake cannot
/// run says so before any account reason, and a built recipe launches.
#[test]
fn a_new_sandbox_is_refused_for_each_missing_prerequisite_in_the_old_order() {
    let hosts = fixture_hosts();
    let recipes = Recipes::Listed(vec![
        built("ready"),
        recipe("unbuilt", json!({ "kind": "tensorlake_ubuntu" })),
        recipe(
            "boxed",
            json!({ "kind": "container_image", "reference": "ubuntu", "user": "dev" }),
        ),
    ]);
    let check = |draft: &LaunchDraft, account: Account| {
        draft.refusal(target_with(&hosts, &recipes, &account), None)
    };
    let ready = in_sandbox("ready");
    assert_eq!(
        ready.target,
        Some(sandbox("ready")),
        "the premise: a sandbox draft"
    );
    assert_eq!(
        check(&ready, Account::Reading).as_deref(),
        Some(LOADING_ACCOUNT)
    );
    assert_eq!(
        check(&ready, Account::Unreadable("the helm is gone".into())).as_deref(),
        Some("the helm is gone")
    );
    assert_eq!(
        check(
            &ready,
            Account::Read(account(false, SshKeyStatus::NotCreated))
        )
        .as_deref(),
        Some("Set a Tensorlake API key in Settings"),
        "the API key first"
    );
    assert_eq!(
        check(
            &ready,
            Account::Read(account(true, SshKeyStatus::NotCreated))
        )
        .as_deref(),
        Some("Create Farhelm's SSH key in Settings → Tensorlake")
    );
    let reason = "disk \u{202e}full";
    let unavailable = SshKeyStatus::Unavailable {
        reason: reason.into(),
    };
    let escaped = farhelm_proto::text::escaped(reason);
    assert_ne!(
        escaped, reason,
        "the premise: the reason holds a character shown escaped"
    );
    assert_eq!(
        check(&ready, Account::Read(account(true, unavailable))),
        Some(format!("Farhelm's local SSH key is unavailable: {escaped}"))
    );
    let usable = || Account::Read(account(true, ready_key("tl sbx ssh keys add")));
    assert_eq!(
        check(&in_sandbox("unbuilt"), usable()).as_deref(),
        Some("No snapshot yet")
    );
    assert_eq!(
        check(&in_sandbox("boxed"), Account::Reading).as_deref(),
        Some("This recipe's base cannot run as a sandbox"),
        "a base it cannot run, before any account reason"
    );
    assert_eq!(check(&ready, usable()), None);
}

/// SPEC.md, "Sandbox sessions": the registration note shows "the existing copyable SSH-key registration command", so
/// it needs a key to register; a note naming a command that does not exist would send the user to a dead end. The note
/// with a made key, and none before the key is created, are `sandbox_view_tests`'. Specified: with Farhelm's key
/// unreadable, or no account read, there is no note, and a container has none even with a made key.
#[test]
fn the_registration_note_needs_a_readable_key() {
    let tensorlake = ProviderUi(ProviderKind::Tensorlake);
    let unreadable = account(
        true,
        SshKeyStatus::Unavailable {
            reason: "unreadable".into(),
        },
    );
    assert_eq!(tensorlake.registration_note(Some(&unreadable)), None);
    assert_eq!(tensorlake.registration_note(None), None);
    let made = account(true, ready_key("tl sbx ssh keys add"));
    assert_eq!(
        ProviderUi(ProviderKind::Podman).registration_note(Some(&made)),
        None
    );
}

/// SPEC.md, "Sandbox sessions": launching a sandbox asks first, "Only Create Tensorlake sandboxes without asking skips
/// this question", and a container, which bills nothing per create, is never asked about. Asking for an account the
/// launcher could not read keeps a billed create from going out on a setting nobody saw. Specified: a Tensorlake
/// recipe of 2 CPUs, 4 GB memory and 20 GB disk is asked about, sized "2 CPU · 4 GB memory · 20 GB disk" and linking
/// the pricing page, with the account's setting off and with no account read; with the setting on it is not; a
/// container is never asked about, whatever the account says; and, with the setting off, exactly the providers whose
/// shared `asks_before_creating` holds (the helm's own check, which refuses their unconfirmed creates) are asked about,
/// so a provider the helm gates can never launch with no question to answer.
#[test]
fn only_a_sandbox_asks_before_creating_unless_its_setting_skips_it() {
    let tensorlake = ProviderUi(ProviderKind::Tensorlake);
    let ready = built("ready");
    let asking = account(true, ready_key("tl sbx ssh keys add"));
    let mut skipping = asking.clone();
    skipping.create_without_asking = true;
    let question = tensorlake
        .creation_question(&ready, Ok(&asking))
        .expect("the setting off asks");
    assert_eq!(question.title, "Create a sandbox for this session?");
    assert_eq!(question.size, "2 CPU · 4 GB memory · 20 GB disk");
    assert_eq!(question.link_url, "https://www.tensorlake.ai/pricing");
    assert_eq!(question.confirm_label, "Create and launch");
    assert_eq!(
        tensorlake.creation_question(&ready, Err(LOADING_ACCOUNT)),
        Some(question),
        "an account not read asks"
    );
    assert_eq!(tensorlake.creation_question(&ready, Ok(&skipping)), None);
    let podman = ProviderUi(ProviderKind::Podman);
    assert_eq!(podman.creation_question(&ready, Ok(&asking)), None);
    assert_eq!(podman.creation_question(&ready, Ok(&skipping)), None);
    for provider in crate::environment_ui::RUNS_ON {
        assert_eq!(
            ProviderUi(provider)
                .creation_question(&ready, Ok(&asking))
                .is_some(),
            provider.asks_before_creating(),
            "{provider:?} asks exactly when the helm requires consent"
        );
    }
}

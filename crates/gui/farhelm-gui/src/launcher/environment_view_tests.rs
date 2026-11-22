//! Tests of the launcher's Host menu and a new container's rows in the real main view over the helm client's
//! fake: which offers show, what New container chooses and shows (its note on the recipe's build there included), the
//! folder field on the way back to Existing host, the composer search rows New container refuses, and the Podman checks
//! the launcher asks for as it opens. SPEC.md, "Creation" (the Runs on bullet) and "Container sessions".
//!
//! The fleet is the helm's pinned fixture's first three hosts: host1 (connected, Podman usable), host2 (Podman not
//! installed) and host3 (an SSH host with no Podman check yet); the catalog holds `web`, on managed Ubuntu. The window
//! and its answering helper are the launch tests' (`environment_launch_tests`).

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionState, HostView, KeyedEntry, Preferences,
};
use farhelm_proto::{LaunchHarness, LaunchTarget, Place, RecipePlaceView};
use gpui::TestAppContext;

use super::environment_launch_tests::{
    answer_except, plain_preferences, window_with, window_with_more,
};
use super::launch_tests::command_draft;
use super::recents_view_tests::{answer_history, chosen, history, launched};
use super::view_tests::{click_in_body, draft, fixture_hosts, name_of, pick_host_menu_row, read};
use crate::test_support::{self, keys, meta, settle};

/// [`window_with`] on [`command_draft`], host1 to host3 and [`plain_preferences`].
fn window(
    cx: &mut TestAppContext,
) -> (
    farhelm_helm_client::fake::FakeHelm,
    &mut gpui::VisualTestContext,
) {
    window_with(
        cx,
        command_draft(),
        fixture_hosts(&[0, 1, 2]),
        plain_preferences(),
    )
}

/// A new-environment offer needs a usable place. With host1's Podman usable, the picker shows New container after
/// existing hosts, and omits New sandbox while the account read is still out.
#[gpui::test]
fn new_environments_without_a_usable_place_are_hidden(cx: &mut TestAppContext) {
    let (_helm, cx) = window(cx);
    click_in_body(cx, "launcher-host");
    assert_eq!(
        name_of(cx, "launcher-host-existing").as_deref(),
        Some("Existing host")
    );
    assert_eq!(
        name_of(cx, "launcher-host-new").as_deref(),
        Some("New environment")
    );
    assert_eq!(
        name_of(cx, "launcher-host-option-New container").as_deref(),
        Some("New container · experimental")
    );
    assert_eq!(name_of(cx, "launcher-host-option-New sandbox"), None);
}

/// A fleet without a usable container host or account offers only its existing hosts. An empty New environment group
/// would misleadingly suggest that a launch could provision something there.
#[gpui::test]
fn the_environment_group_is_hidden_when_only_existing_hosts_remain(cx: &mut TestAppContext) {
    let hosts = fixture_hosts(&[1]);
    let (_helm, cx) = window_with(cx, command_draft(), hosts, plain_preferences());
    assert!(
        name_of(cx, "launcher-destination-Folder").is_some(),
        "the premise: the launcher is drawn"
    );
    click_in_body(cx, "launcher-host");
    assert!(
        name_of(cx, "launcher-host-option-2").is_some(),
        "existing hosts are offered"
    );
    assert_eq!(name_of(cx, "launcher-host-new"), None);
    assert_eq!(name_of(cx, "launcher-host-option-New container"), None);
}

/// SPEC.md, "Creation": New container "shows Recipe and Container host, defaulting to the last container host used";
/// "Hosts with unusable Podman are disabled with their reason, and the note says whether the recipe is ready there or
/// builds on first launch"; "Container sessions": "Folder destinations use its home, without Browse or recent folders",
/// and "The launch action's accessible name names the new container and its host". Specified: with host2 the last container host but unusable,
/// New container chooses `web` on host1, the first usable host. The picker names the environment and Folder shows `~` as text, with no
/// field; host2's option says why and cannot be chosen; the note says the build status is being checked while the
/// helm has pushed nothing about `web` there; the footer names "new web container on host1". host3, an SSH host
/// carrying no Podman check, is listed too, disabled with the helm's words for it. What the note says once the recipe
/// place arrives is [`the_note_follows_the_recipe_place`]'s.
#[gpui::test]
fn new_container_shows_its_rows_with_unusable_hosts_disabled(cx: &mut TestAppContext) {
    let preferences = Preferences {
        welcome_seen: Some(true),
        last_container_host: Some(2),
        ..Preferences::default()
    };
    let (mut helm, cx) = window_with(cx, command_draft(), fixture_hosts(&[0, 1, 2]), preferences);
    answer_except(&mut helm, cx, |_| false);
    pick_host_menu_row(cx, "launcher-host-option-New container");
    assert_eq!(
        read(cx, |launcher, _, _| launcher.draft.target.clone()),
        Some(LaunchTarget::Environment {
            recipe: "web".into(),
            place: Place::PodmanHost(1),
        }),
        "the last container host is unusable, so the first usable one"
    );
    assert_eq!(
        name_of(cx, "launcher-host").as_deref(),
        Some("Host: New container")
    );
    assert_eq!(name_of(cx, "launcher-folder"), None, "no folder field");
    assert_eq!(
        name_of(cx, "launcher-folder-held").as_deref(),
        Some("~"),
        "Folder shows the environment's home as text"
    );
    assert!(name_of(cx, "launcher-recipe").is_some());
    test_support::click(cx, "launcher-container-host");
    let painted = crate::session_list::tests::painted(&test_support::frame(cx));
    assert!(
        painted
            .iter()
            .any(|line| line.contains("Podman is not installed on this host")),
        "host2 says why: {painted:?}"
    );
    assert!(
        painted
            .iter()
            .any(|line| line.contains("this host cannot run containers")),
        "host3, which carries no check, is listed and says so: {painted:?}"
    );
    test_support::click(cx, "launcher-container-host-option-2");
    assert_eq!(
        read(cx, |launcher, _, _| launcher
            .draft
            .target
            .clone()
            .map(|target| target.place())),
        Some(Place::PodmanHost(1)),
        "host2 cannot be chosen"
    );
    keys(cx, "escape");
    assert_eq!(
        name_of(cx, "launcher-place-note").as_deref(),
        Some("Checking build status…"),
        "before the recipe's place is pushed"
    );
    assert!(
        name_of(cx, "launcher-launch")
            .is_some_and(|name| name.starts_with("Launch Command · new web container on host1")),
        "{:?}",
        name_of(cx, "launcher-launch")
    );
}

/// The last container host used is New container's default while it is usable (SPEC.md, "Creation"), so a user who
/// launches on one machine keeps landing there. Specified: with host2 usable and the last container host, New container
/// chooses host2.
#[gpui::test]
fn new_container_defaults_to_the_last_container_host(cx: &mut TestAppContext) {
    let mut hosts: Vec<HostView> = (0..2)
        .map(|index| {
            let state = fixture_hosts(&[index]);
            serde_json::from_value(state.entries[0].value.clone()).unwrap()
        })
        .collect();
    if let Some(check) = hosts[1].container_check.as_mut() {
        check.usable = true;
        check.reason = None;
    }
    let preferences = Preferences {
        welcome_seen: Some(true),
        last_container_host: Some(2),
        ..Preferences::default()
    };
    let (mut helm, cx) = window_with(
        cx,
        command_draft(),
        crate::hosts::tests::hosts(&hosts),
        preferences,
    );
    answer_except(&mut helm, cx, |_| false);
    pick_host_menu_row(cx, "launcher-host-option-New container");
    assert_eq!(
        read(cx, |launcher, _, _| launcher
            .draft
            .target
            .clone()
            .map(|target| target.place())),
        Some(Place::PodmanHost(2))
    );
}

/// SPEC.md, "Container sessions": "Opening the launcher refreshes probes in the background", so a host checked long
/// ago does not keep a stale reason, and one never checked can become a container host from here. Specified: opening
/// the launcher asks for a Podman check of host1, host2 and host3, every host that can hold containers.
#[gpui::test]
fn opening_the_launcher_checks_the_container_hosts(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    cx.run_until_parked();
    // Looked at, not answered: the fake answers in order, and the catalog read before them waits too.
    let probes: Vec<String> = helm
        .commands()
        .into_iter()
        .map(|command| command.path)
        .filter(|path| path.ends_with("/podman-probe"))
        .collect();
    assert_eq!(
        probes,
        [
            "/api/hosts/1/podman-probe",
            "/api/hosts/2/podman-probe",
            "/api/hosts/3/podman-probe"
        ]
    );
}

/// The shared Podman check's contract: a caller sends nothing while the hosts list is unavailable, because that list
/// still names the last hosts it had, and a check asked for one that is gone would only be refused. Specified: with the
/// pushed hosts collection unavailable (still listing host1 to host3), opening the launcher asks for no Podman check.
#[gpui::test]
fn opening_the_launcher_checks_no_host_while_the_hosts_are_unavailable(cx: &mut TestAppContext) {
    let mut hosts = fixture_hosts(&[0, 1, 2]);
    hosts.meta.availability = Availability::Unavailable {
        error: "hosts unreadable".into(),
    };
    let (mut helm, cx) = window_with(cx, command_draft(), hosts, plain_preferences());
    cx.run_until_parked();
    assert!(
        name_of(cx, "launcher-destination-Folder").is_some(),
        "the premise: the launcher opened"
    );
    let probes: Vec<String> = helm
        .commands()
        .into_iter()
        .map(|command| command.path)
        .filter(|path| path.ends_with("/podman-probe"))
        .collect();
    assert_eq!(probes, Vec::<String>::new());
}

/// SPEC.md, "Container sessions": "The managed checkout is cloned inside it. Folder destinations use its home". The
/// The destination switch stays available once a recipe is chosen. The Host segment names the environment and can
/// take a different target. Folder shows `~` with no field. Managed checkout shows a separate Repository field and
/// explains the folder's lifetime inside the environment without asking the source host for a preview.
#[gpui::test]
fn a_new_container_keeps_the_where_row(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    answer_except(&mut helm, cx, |_| false);
    pick_host_menu_row(cx, "launcher-host-option-New container");
    assert!(
        name_of(cx, "launcher-destination-Folder").is_some(),
        "the Where row shows"
    );
    assert_eq!(
        name_of(cx, "launcher-host").as_deref(),
        Some("Host: New container")
    );
    assert_eq!(
        name_of(cx, "launcher-folder"),
        None,
        "Folder is the fixed home"
    );
    test_support::click(cx, "launcher-destination-Managed checkout");
    assert!(
        cx.debug_bounds("launcher-repository-row").is_some(),
        "the repository field shows"
    );
    assert_eq!(
        read(cx, |launcher, _, _| launcher.draft.destination_kind()),
        crate::launcher::draft::DestinationKind::Checkout
    );
    // A repository chosen as the suggestions would choose it, through the draft's own edit.
    cx.update(|_, cx| {
        let launcher =
            crate::layers::last_opened::<super::view::Launcher>(cx).expect("the launcher is open");
        launcher.update(cx, |launcher, cx| {
            launcher
                .draft
                .choose_repository(farhelm_proto::parse_github_repo("owner/repo").unwrap());
            cx.notify();
        });
    });
    test_support::settle(cx);
    let previews = answer_except(&mut helm, cx, |command| {
        command.path.ends_with("checkout-preview")
    });
    assert_eq!(previews.len(), 0, "no preview is asked of the host");
    assert!(
        cx.debug_bounds("launcher-preview-environment").is_some(),
        "the checkout is said to be cloned inside the new environment"
    );
    assert_eq!(
        name_of(cx, "launcher-preview-message").as_deref(),
        Some("Cloned inside the new environment once it starts"),
    );
    let lifetime = "New folder named after the session, inside the new environment; deleting its last session removes it permanently.";
    assert_eq!(name_of(cx, lifetime).as_deref(), Some(lifetime));
    assert!(name_of(cx, "New folder named after the session; it moves to the trash when its last session is deleted.").is_none());
}

/// The folder field is its own text input, which follows the draft only when an edit syncs it; a Runs on edit that did
/// not sync it left Existing host showing the folder typed before New container while the footer and the launch used
/// `~`, so the user saw one folder and launched in another. Specified: with a folder typed on Existing host, entering
/// New container and leaving it again for Existing host leaves the folder field showing the draft's folder, `~` (the
/// Runs on edit leaves the folder at the environment's home, `LaunchDraft::choose_runs_on`).
#[gpui::test]
fn leaving_new_container_shows_the_drafts_folder_in_the_field(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    answer_except(&mut helm, cx, |_| false);
    click_in_body(cx, "launcher-folder");
    keys(cx, "/ s r c");
    let typed = draft(cx).folder.value().clone();
    assert_ne!(
        typed, "~",
        "the premise: a folder other than home was typed"
    );
    assert_eq!(folder_field(cx), typed, "the premise: the field shows it");
    pick_host_menu_row(cx, "launcher-host-option-New container");
    assert!(
        draft(cx).target.is_some(),
        "the premise: New container is chosen"
    );
    pick_host_menu_row(cx, "launcher-host-option-1");
    let left = draft(cx);
    assert_eq!(left.target, None, "the premise: back on Existing host");
    assert_eq!(left.folder.value(), "~");
    assert_eq!(folder_field(cx), "~", "the field shows the draft's folder");
}

/// The text the folder field holds.
fn folder_field(cx: &mut gpui::VisualTestContext) -> String {
    read(cx, |launcher, _, cx| launcher.folder.text(cx).to_string())
}

/// `web` on host1's recipe place as the helm pushes it, `ready` there or not, building nothing, with no reason.
fn push_web_place(helm: &mut FakeHelm, cx: &mut gpui::VisualTestContext, ready: bool) {
    let view = RecipePlaceView {
        recipe: "web".into(),
        place: Place::PodmanHost(1),
        ready,
        building: false,
        reason: None,
        built_at: None,
        prepared_at: None,
        step: None,
        cleanup_pending: false,
    };
    let key = RecipePlaceView::key(&view.recipe, view.place);
    helm.push(CollectionChanges {
        collection: Collection::RecipePlaces,
        meta: meta(&[key.as_str()]),
        upsert: vec![KeyedEntry {
            key,
            value: serde_json::to_value(view).unwrap(),
        }],
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// SPEC.md, "Creation": for New container "the note says whether the recipe is ready there or builds on first launch",
/// which is what tells a user whether Launch takes seconds or minutes. Specified: with `web` chosen on host1, the note
/// says "Checking build status…" until the helm pushes `web`'s place there, then "Builds on first launch here (a few
/// minutes)" for a place with nothing built and nothing building, then "Ready on host1" once it is ready.
#[gpui::test]
fn the_note_follows_the_recipe_place(cx: &mut TestAppContext) {
    let places = CollectionState {
        collection: Collection::RecipePlaces,
        meta: meta(&[]),
        entries: Vec::new(),
    };
    let (mut helm, cx) = window_with_more(
        cx,
        command_draft(),
        fixture_hosts(&[0, 1, 2]),
        plain_preferences(),
        vec![places],
    );
    answer_except(&mut helm, cx, |_| false);
    pick_host_menu_row(cx, "launcher-host-option-New container");
    assert_eq!(
        name_of(cx, "launcher-place-note").as_deref(),
        Some("Checking build status…"),
        "the premise: nothing pushed about web on host1 yet"
    );
    push_web_place(&mut helm, cx, false);
    assert_eq!(
        name_of(cx, "launcher-place-note").as_deref(),
        Some("Builds on first launch here (a few minutes)")
    );
    push_web_place(&mut helm, cx, true);
    assert_eq!(
        name_of(cx, "launcher-place-note").as_deref(),
        Some("Ready on host1")
    );
}

/// A note about a host that is gone would say "Checking build status…" with nothing left to check; the old
/// `PlacementPicker` drew none without a listed place. Specified: with New container on host1, once the registry stops
/// listing host1 the Container host select stays, keeping host1 as the chosen place, and no build note is drawn.
/// Launch says why it cannot start. Pressing the retained New container segment keeps that refusal visible, because
/// the shared picker sends no edit for a press on a chosen segment with nowhere to enter, so nothing reaches the
/// launcher's edit handler, which clears the refusal.
#[gpui::test]
fn no_note_once_the_container_host_is_gone(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    answer_except(&mut helm, cx, |_| false);
    pick_host_menu_row(cx, "launcher-host-option-New container");
    assert!(
        name_of(cx, "launcher-place-note").is_some(),
        "the premise: the note is drawn while host1 is listed"
    );
    helm.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&["2", "3"]),
        upsert: Vec::new(),
        remove: vec!["1".into()],
    });
    test_support::settle(cx);
    assert_eq!(
        draft(cx).target.map(|target| target.place()),
        Some(Place::PodmanHost(1)),
        "the premise: host1 is still the chosen place"
    );
    assert!(
        name_of(cx, "launcher-container-host").is_some(),
        "the premise: the Container host select is still drawn"
    );
    assert_eq!(name_of(cx, "launcher-place-note"), None);

    test_support::click(cx, "launcher-launch");
    let refusal = name_of(cx, "launcher-refusal").expect("the missing host prevents launch");
    pick_host_menu_row(cx, "launcher-host-option-New container");
    assert_eq!(name_of(cx, "launcher-refusal"), Some(refusal));
}

/// Why the last row taken from the composer search changed nothing, as the line above the box says it.
fn refusal(cx: &mut gpui::VisualTestContext) -> Option<String> {
    name_of(cx, "launcher-search-refusal")
}

/// SPEC.md, "Container sessions": a new container's Folder destinations "use its home, without Browse or recent
/// folders"; and "Creation": a recent setup carries its place. The composer search's folder rows come from the existing
/// host's history, so taking one in New container would move the folder off the home or browse a machine that does not
/// exist yet, while a recent setup filled in place would launch in the container without its folder. Specified: in New
/// container, a saved folder, a typed path and Browse taken from the search each change nothing and say why above the
/// box in the old search's words; a recent setup taken from it goes back to Existing host with the setup's folder.
#[gpui::test]
fn new_container_refuses_folder_rows_and_takes_recent_setups_back_to_their_host(
    cx: &mut TestAppContext,
) {
    let mut answer = history(vec![launched(
        "/tmp",
        &chosen(LaunchHarness::Codex, None, None, None),
    )]);
    answer["folders"] = serde_json::json!([{
        "host": 1,
        "canonical_cwd": "/srv/project",
        "canonical_proven": true,
        "display_cwd": "/srv/project",
        "created_at": 1,
        "creation_seq": null,
    }]);
    let (mut helm, cx) = window(cx);
    answer_history(&mut helm, cx, &answer);
    pick_host_menu_row(cx, "launcher-host-option-New container");
    let before = draft(cx);
    assert!(
        before.target.is_some(),
        "the premise: New container is chosen"
    );
    let fixed = "A new sandbox's Folder is fixed to its home directory";

    test_support::click(cx, "launcher-search-row");
    cx.simulate_input("folder:project");
    settle(cx);
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Use this path: /srv/project"),
        "the premise: the saved folder is offered"
    );
    keys(cx, "enter");
    assert_eq!(refusal(cx).as_deref(), Some(fixed));
    assert_eq!(draft(cx), before, "the saved folder changed nothing");

    for _ in 0.."folder:project".len() {
        keys(cx, "backspace");
    }
    cx.simulate_input("folder:/srv/other");
    settle(cx);
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Use this path: /srv/other"),
        "the premise: the typed path is offered first"
    );
    assert_eq!(
        name_of(cx, "launcher-search-option-1").as_deref(),
        Some("Browse this path: /srv/other"),
        "the premise: and its Browse after it"
    );
    keys(cx, "enter");
    assert_eq!(refusal(cx).as_deref(), Some(fixed));
    assert_eq!(draft(cx), before, "the typed path changed nothing");
    // Enter reopens the closed results, and Down moves to Browse.
    keys(cx, "enter down enter");
    assert_eq!(
        refusal(cx).as_deref(),
        Some("A new sandbox cannot be browsed before it exists")
    );
    assert_eq!(draft(cx), before, "Browse changed nothing");

    for _ in 0.."folder:/srv/other".len() {
        keys(cx, "backspace");
    }
    cx.simulate_input("recent:");
    settle(cx);
    assert!(
        name_of(cx, "launcher-search-option-0")
            .is_some_and(|name| name.starts_with("Recent setup: /tmp")),
        "the premise: the existing host's setup is offered"
    );
    keys(cx, "enter");
    let taken = draft(cx);
    assert_eq!(taken.target, None, "the setup went back to Existing host");
    assert_eq!(taken.folder.value(), "/tmp", "with the setup's folder");
}

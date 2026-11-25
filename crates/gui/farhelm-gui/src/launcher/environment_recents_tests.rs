//! Tests of recent setups that made a new environment, in the real launcher over the helm client's fake: read from the
//! helm's own history of them, offered beside a host's setups and while a recipe is chosen, naming their own recipe and
//! place, restoring that placement when taken, and refused in Replace with. Which setups rank, and the merge of the two
//! histories, are `recents_tests`'.
//!
//! Ported from the old `e2e/tests/podman-environments.spec.ts` at reference commit `24bc0580`: "environment recents
//! hold focused intent until deliberate promotion" (its search half; the rest, re-reading on every activity-feed notice
//! while a focused row keeps its setup, is superseded: the launcher reads the history once as it opens) and
//! "environment recents stay independent of a held ordinary folder default".

use std::cell::Cell;

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{Collection, CollectionState, KeyedEntry};
use farhelm_proto::{LaunchTarget, Place, ProviderKind};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::draft::{LaunchDraft, RunsOn};
use super::entry::open_new;
use super::environment_launch_tests::{
    answer_reads, plain_preferences, window_with, window_with_more,
};
use super::environment_tests::recipe_json;
use super::folder_default_tests::edit_draft;
use super::search_view_tests::type_in;
use super::view_tests::{
    draft, fixture_hosts, launcher_with_hosts, name_of, retarget_history_host, search_focused,
    search_text, selected_state,
};
use crate::test_support::{self, keys, meta, settle};

/// Replace with's refusal of a new environment's setup, the old launcher's words.
const KEEPS_ENVIRONMENT: &str = "Restart and Replace keep the existing environment";

/// A new container of `recipe` on host1.
fn container(recipe: &str) -> LaunchTarget {
    LaunchTarget::Environment {
        recipe: recipe.into(),
        place: Place::PodmanHost(1),
    }
}

/// One Codex launch into a new [`container`] of `recipe`, at `created_at`, into a managed checkout of `repo` if one is
/// named, as the helm's history of launches into new environments lists it.
fn environment(recipe: &str, repo: Option<&str>, created_at: i64) -> Value {
    json!({
        "host": 7,
        "target": serde_json::to_value(container(recipe)).unwrap(),
        "github_repo": repo.map(|name| json!({ "owner": "acme", "name": name })),
        "canonical_cwd": null,
        "cwd": "~",
        "selection": { "harness": "codex" },
        "created_at": created_at,
        "creation_seq": created_at,
    })
}

/// Answer every command the window sent ([`answer_reads`]), the environments' history with `entries`; the creates are
/// kept, unanswered, and returned.
fn answer(helm: &mut FakeHelm, cx: &mut VisualTestContext, entries: &[Value]) -> Vec<FakeCommand> {
    let history = Value::from(entries.to_vec());
    answer_reads(helm, cx, None, Some(&history), |command| {
        command.body.is_some() && command.path.ends_with("sessions")
    })
}

/// The accessible value of the element drawn under `id`, as a select reports the option it shows.
fn value_of(cx: &mut VisualTestContext, id: &str) -> Option<String> {
    test_support::frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .and_then(|node| node.value.map(|value| value.to_string()))
}

/// The id of the strip row whose name holds `words`.
fn row_naming(cx: &mut VisualTestContext, words: &str) -> Option<&'static str> {
    [
        "launcher-recent-0",
        "launcher-recent-1",
        "launcher-recent-2",
    ]
    .into_iter()
    .find(|id| name_of(cx, id).is_some_and(|name| name.contains(words)))
}

/// SPEC.md, "Creation": recent setups restore "recipe and place together", and the environments' history is the helm's
/// own, kept after the disposable host is gone; the old strip stayed up with a recipe chosen and needed no host
/// connected. Specified: a draft in a container of `web`, with host2 (connecting, so no host's history is read) behind
/// it, shows the setup that made an `api` container as "~ · api container on host1 · Codex"; `recent:` in search offers
/// it under those words too, and Enter there makes `api` on host1 the placement, the folder staying `~`.
#[gpui::test]
fn an_environment_setup_shows_by_a_chosen_recipe_with_no_host_connected(cx: &mut TestAppContext) {
    let mut opened = LaunchDraft::new(Some(2), "~".into(), None);
    opened.choose_runs_on(RunsOn::New(ProviderKind::Podman), Some(container("web")));
    let hosts = fixture_hosts(&[0, 1, 2]);
    let (mut helm, cx) = window_with(cx, opened, hosts, plain_preferences());
    let host_reads = Cell::new(0);
    let history = json!([environment("api", None, 2)]);
    answer_reads(&mut helm, cx, None, Some(&history), |command| {
        if command.path.starts_with("/api/launch-history") {
            host_reads.set(host_reads.get() + 1);
        }
        false
    });
    assert_eq!(
        host_reads.get(),
        0,
        "the premise: no host's history was read"
    );
    let name = name_of(cx, "launcher-recent-0").unwrap_or_default();
    assert!(
        name.starts_with("~ · api container on host1 · Codex"),
        "{name}"
    );
    test_support::click(cx, "launcher-search-row");
    cx.simulate_input("recent:");
    settle(cx);
    let option = name_of(cx, "launcher-search-option-0").unwrap_or_default();
    assert!(
        option.starts_with("Recent setup: ~ · api container on host1"),
        "{option}"
    );
    keys(cx, "enter");
    let taken = draft(cx);
    assert_eq!(
        (taken.target, taken.folder.value().as_str()),
        (Some(container("api")), "~")
    );
}

/// A past model from helm-wide history remains an explicit choice while the selected host's history is re-read.
#[gpui::test]
fn a_past_model_from_environment_history_survives_host_retargeting(cx: &mut TestAppContext) {
    let opened = LaunchDraft::new(Some(1), "/tmp".into(), None);
    let hosts = fixture_hosts(&[0]);
    let (mut helm, cx) = launcher_with_hosts(cx, opened, hosts);
    let mut recent = environment("api", None, 2);
    recent["selection"]["model"] = json!("env-model-newer");
    let mut earlier = environment("api", None, 1);
    earlier["selection"]["model"] = json!("env-model-earlier");
    assert!(answer(&mut helm, cx, &[recent, earlier]).is_empty());
    assert!(search_focused(cx));
    type_in(cx, "model:env-model");
    let default = "launcher-search-option-0";
    let row = "launcher-search-option-1";
    let text = Some("Model: env-model-earlier (Codex, past launch)");
    // The nondefault row distinguishes retaining the explicit choice from silently restoring the default.
    assert_eq!(
        name_of(cx, default).as_deref(),
        Some("Model: env-model-newer (Codex, past launch)")
    );
    assert_eq!(selected_state(cx, default), Some(true));
    assert_eq!(name_of(cx, row).as_deref(), text);
    keys(cx, "down");
    assert_eq!(selected_state(cx, row), Some(true));

    retarget_history_host(&mut helm, cx);
    assert_eq!(name_of(cx, row).as_deref(), text);
    assert_eq!(selected_state(cx, row), Some(true));
    assert!(search_focused(cx));
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.model.as_deref(), Some("env-model-earlier"));
    assert_eq!(search_text(cx), "");
    assert_eq!(name_of(cx, "launcher-search-refusal"), None);
}

/// What the launcher shows after a recent setup restores its placement must be what Launch sends, and never an ordinary
/// create on the host behind it. Specified: on New on host1 in `/tmp`, with `alpha` listed before `web`, a click on the
/// `web` container setup chooses it, the Recipe select shows `web`, the footer names "new web container on host1" and
/// the Host row goes; Launch then sends one environment create of `web` at host1's place in `~` with the setup's Codex,
/// and no ordinary create. The container host is checked through the footer and the body alone: the fixture offers one
/// usable container host, so its select could not tell a restored host from the default.
#[gpui::test]
fn a_taken_environment_setup_shows_and_sends_its_placement(cx: &mut TestAppContext) {
    let recipes = CollectionState {
        collection: Collection::Recipes,
        meta: meta(&["alpha", "web"]),
        entries: ["alpha", "web"]
            .map(|name| KeyedEntry {
                key: name.into(),
                value: recipe_json(name, json!({ "kind": "tensorlake_ubuntu" })),
            })
            .into(),
    };
    let opened = LaunchDraft::new(Some(1), "/tmp".into(), None);
    let hosts = fixture_hosts(&[0, 1, 2]);
    let (mut helm, cx) = window_with_more(cx, opened, hosts, plain_preferences(), vec![recipes]);
    answer(&mut helm, cx, &[environment("web", None, 2)]);
    test_support::click(cx, "launcher-recent-0");
    assert_eq!(draft(cx).target, Some(container("web")));
    assert!(value_of(cx, "launcher-recipe").is_some_and(|shown| shown.starts_with("web")));
    let action = name_of(cx, "launcher-launch").unwrap_or_default();
    assert!(action.contains("new web container on host1"), "{action}");
    assert!(super::view_tests::search_focused(cx));
    assert_eq!(
        name_of(cx, "launcher-host").as_deref(),
        Some("Host: New container")
    );
    test_support::click(cx, "launcher-launch");
    let sent = answer(&mut helm, cx, &[environment("web", None, 2)]);
    let [create] = &sent[..] else {
        panic!("one create, not {sent:?}");
    };
    assert_eq!(create.path, "/api/environment-sessions");
    let body = create.body.as_ref().expect("a create carries a body");
    assert_eq!(body["recipe"], json!("web"));
    assert_eq!(body["place"], json!({ "kind": "podman_host", "host": 1 }));
    assert_eq!(body["request"]["cwd"], json!("~"));
    assert_eq!(body["request"]["launch"]["harness"], json!("codex"));
}

/// A template mounts both placement selects on an ordinary launch. Put its recipe and host after the defaults
/// so the selects and the create can distinguish the saved placement from both first options.
#[gpui::test]
fn a_container_template_shows_the_recipe_and_host_it_sends(cx: &mut TestAppContext) {
    use farhelm_proto::launcher::{LaunchTemplate, TemplateFields};

    let mut hosts = fixture_hosts(&[0, 8]);
    hosts.entries[1].value["container_check"] = hosts.entries[0].value["container_check"].clone();
    let recipes = CollectionState {
        collection: Collection::Recipes,
        meta: meta(&["web", "restored"]),
        entries: ["web", "restored"]
            .map(|name| KeyedEntry {
                key: name.into(),
                value: recipe_json(name, json!({ "kind": "tensorlake_ubuntu" })),
            })
            .into(),
    };
    let target = LaunchTarget::Environment {
        recipe: "restored".into(),
        place: Place::PodmanHost(9),
    };
    let template = LaunchTemplate {
        name: "saved-box".into(),
        fields: TemplateFields {
            target: Some(target.clone()),
            ..TemplateFields::default()
        },
    };
    let templates = CollectionState {
        collection: Collection::Templates,
        meta: meta(&["saved-box"]),
        entries: vec![KeyedEntry {
            key: "saved-box".into(),
            value: serde_json::to_value(template).unwrap(),
        }],
    };
    let (mut helm, cx) = window_with_more(
        cx,
        super::launch_tests::command_draft(),
        hosts,
        plain_preferences(),
        vec![recipes, templates],
    );
    // Taking a template waits for the launch catalog, even when the template changes only placement.
    assert!(answer(&mut helm, cx, &[]).is_empty());
    assert!(draft(cx).target.is_none());
    assert!(!super::view_tests::has_node(cx, "launcher-recipe"));
    assert!(!super::view_tests::has_node(cx, "launcher-container-host"));
    assert!(super::view_tests::search_focused(cx));
    super::search_view_tests::type_in(cx, "tl:saved-box");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Template: saved-box")
    );
    keys(cx, "enter");
    assert_eq!(
        draft(cx).target,
        Some(target),
        "template application said {:?}",
        name_of(cx, "launcher-search-refusal")
    );
    assert!(value_of(cx, "launcher-recipe").is_some_and(|value| value.starts_with("restored")));
    assert_eq!(
        value_of(cx, "launcher-container-host").as_deref(),
        Some("host9")
    );
    assert!(
        answer(&mut helm, cx, &[]).is_empty(),
        "a template fills without launching"
    );

    test_support::click(cx, "launcher-launch");
    let sent = answer(&mut helm, cx, &[]);
    let [create] = sent.as_slice() else {
        panic!("expected one environment create: {sent:?}");
    };
    assert_eq!(create.path, "/api/environment-sessions");
    let body = create.body.as_ref().expect("the create carries its draft");
    assert_eq!(body["recipe"], json!("restored"));
    assert_eq!(body["place"], json!({ "kind": "podman_host", "host": 9 }));
    assert_eq!(body["request"]["cwd"], json!("~"));
}

/// The old "environment recents stay independent of a held ordinary folder default": a host's history still out must
/// not take the environments' setups away, and an environment's `~` must never become that host's remembered folder.
/// Specified: New on host1, then host9 chosen, its history held: the `web` container setup still shows while the folder
/// waits; host9's history then settles the folder on its own `/ordinary-remembered`, the setup still shows, and a click
/// on it chooses `web` on host1.
#[gpui::test]
fn environment_setups_stay_apart_from_a_hosts_held_folder_default(cx: &mut TestAppContext) {
    let (mut helm, cx) = super::entry_tests::window_with(cx, None);
    assert!(cx.update(open_new), "the premise: New opened");
    settle(cx);
    answer(&mut helm, cx, &[environment("web", None, 2)]);
    edit_draft(cx, |draft| draft.choose_host(9, "~".into()));
    cx.run_until_parked();
    let held = helm.commands();
    assert!(
        held.iter()
            .any(|command| command.path.starts_with("/api/launch-history")),
        "the premise: host9's history was asked for"
    );
    assert!(
        row_naming(cx, "web container on host1").is_some(),
        "shown while host9's read is out"
    );
    assert!(
        draft(cx).folder_default_waiting(),
        "and the folder still waits for host9's"
    );
    for command in held {
        let body = command.path.starts_with("/api/launch-history").then(|| {
            json!({
                "launches": [{
                    "host": 9, "github_repo": null, "canonical_cwd": "/ordinary-remembered",
                    "cwd": "/ordinary-remembered", "selection": { "harness": "codex" },
                    "created_at": 1, "creation_seq": null,
                }],
                "folders": [],
                "checkout_config_revision": 1,
            })
        });
        helm.reply(command.id, 200, body);
    }
    settle(cx);
    assert_eq!(draft(cx).folder.value(), "/ordinary-remembered");
    let row = row_naming(cx, "web container on host1").expect("still shown");
    test_support::click(cx, row);
    assert_eq!(draft(cx).target, Some(container("web")));
}

/// SPEC.md, "Sandbox sessions": "Restart, Restart with, Replace and Replace with stay in the existing sandbox"; the
/// launcher area decided on 2026-11-16 that Replace with shows a new environment's setup and refuses it in the old
/// launcher's words rather than leaving it out. In Replace with on host1, the `web` container setup has a descriptive
/// name and explanatory help. The disabled row offers no Fill or Replace action, and Enter on the query result stays
/// refused with the same reason, so changing presentation cannot change the target.
#[gpui::test]
fn replace_with_refuses_an_environment_setup(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(super::view_tests::codex(None, None, None));
    let opened = LaunchDraft::replace_with("a", &source, Some(1));
    let (mut helm, cx) = launcher_with_hosts(cx, opened, fixture_hosts(&[0]));
    answer(&mut helm, cx, &[environment("web", None, 2)]);
    let name = name_of(cx, "launcher-recent-0").unwrap_or_default();
    assert!(name.starts_with("~ · web container on host1"), "{name}");
    let before = draft(cx);
    let row = test_support::frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("launcher-recent-0"))
        .expect("the held setup remains listed");
    assert!(!row.flags.click);
    let help = super::recents_view_tests::recent_help(cx, "launcher-recent-0")
        .expect("a disabled row still explains itself on hover");
    assert!(help.starts_with(KEEPS_ENVIRONMENT), "{help}");
    assert!(help.contains("~ · web container on host1"), "{help}");
    test_support::click(cx, "launcher-recent-0");
    assert_eq!(draft(cx), before, "nothing changed");
    test_support::click(cx, "launcher-search-row");
    keys(cx, "down");
    assert!(super::view_tests::search_focused(cx));
    assert_eq!(
        super::view_tests::selected_state(cx, "launcher-recent-0"),
        Some(true)
    );
    keys(cx, "enter enter");
    assert_eq!(
        draft(cx),
        before,
        "repeated Enter did not replace the source"
    );
    assert!(
        helm.commands()
            .iter()
            .all(|command| !command.path.ends_with("/replace"))
    );
    cx.simulate_input("recent:");
    settle(cx);
    assert_eq!(name_of(cx, "launcher-recent-enter"), None);
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some(KEEPS_ENVIRONMENT)
    );
    assert_eq!(draft(cx), before, "nothing changed");
    test_support::click(cx, "launcher-launch");
    assert!(
        helm.commands()
            .iter()
            .any(|command| command.path.ends_with("/replace")),
        "the underlying form can replace when its own action is chosen"
    );
}

/// A recent setup's destination comes with it: one that made a new environment in a managed checkout chooses that
/// repository there, and one in the environment's home leaves a managed checkout chosen before for Folder, at the home
/// the placement set. Why: otherwise the new container would get a checkout of a repository its setup never named, and
/// the fixed home must not be edited as if it were a host's folder (the old `accept_recent` took the entry's
/// destination whole). Specified: on New on host1 in `/tmp`, taking from `recent:` the `api` container setup in
/// `acme/baz` chooses `api` with a checkout of `acme/baz`; taking the `web` container setup then chooses `web`, no
/// checkout, and the untouched folder `~`.
#[gpui::test]
fn an_environment_setup_brings_its_own_destination(cx: &mut TestAppContext) {
    let opened = LaunchDraft::new(Some(1), "/tmp".into(), None);
    let (mut helm, cx) = window_with(cx, opened, fixture_hosts(&[0]), plain_preferences());
    let history = [
        environment("api", Some("baz"), 3),
        environment("web", None, 2),
    ];
    answer(&mut helm, cx, &history);
    test_support::click(cx, "launcher-search-row");
    let take = |cx: &mut VisualTestContext, keys_to_row: &str, recipe: &str| {
        cx.simulate_input("recent:");
        settle(cx);
        keys(cx, keys_to_row);
        assert_eq!(draft(cx).target, Some(container(recipe)));
    };
    take(cx, "enter", "api");
    let repo = draft(cx).checkout.and_then(|checkout| checkout.repo);
    assert_eq!(repo.map(|repo| repo.name), Some("baz".to_owned()));
    take(cx, "down enter", "web");
    let taken = draft(cx);
    assert!(taken.checkout.is_none(), "Folder again");
    assert_eq!(taken.folder.value(), "~");
    assert!(!taken.folder.is_touched());
}

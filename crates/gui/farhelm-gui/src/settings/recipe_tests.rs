//! Tests of Settings → Recipes: the row's words from a recipe's view alone (its summary and its snapshot status, ported
//! from the old list's own unit tests), and, over the helm client's fake, the rows drawn from the pushed catalog and
//! redrawn by its pushes, the one read a showing still sends (the account's), a catalog the helm could not read and
//! its Retry loading, and the snapshot build with its accepted reply, its refusal and its API-key gate.
//!
//! The dialog is opened from the gear as in the dialog's own tests (`super::tests`), whose helpers these share. Every
//! step ends in one of the shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::recipes::{
    BuildStep, BuildView, PrepareView, RecipeBase, RecipeSize, RecipeView, SnapshotView, Tunnel,
};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, KeyedEntry,
    Preferences,
};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::Section;
use super::recipe_place_tests::places_state;
use super::recipes::{BuildAction, Tone, line_texts, snapshot_status, summary};
use super::tests::{
    available, drawn, label_of, open_rail, open_rail_with, open_settings, painted, section_button,
    sent,
};
use crate::host_text::tests::painted_words;
use crate::operations::{OpKey, Work};
use crate::session;
use crate::test_support::{advance, click, frame, meta, settle};

// ---- A row's words, from its view alone ---------------------------------------------------------------------------

/// A recipe on `base` with `tunnels` tunnels and nothing built, so the list's wording can be checked without a window.
pub(super) fn view(base: RecipeBase, tunnels: usize) -> RecipeView {
    RecipeView {
        name: "rust-dev".into(),
        size: RecipeSize {
            cpus: 4,
            memory_gb: 8,
            disk_gb: 50,
        },
        base,
        packages: Vec::new(),
        claude: None,
        codex: None,
        claude_login_credential: None,
        codex_login_credential: None,
        github_token_credential: None,
        tunnels: (0..tunnels)
            .map(|n| Tunnel {
                sandbox_port: 8000 + n as u16,
                host: "lb.internal".into(),
                port: 8000,
            })
            .collect(),
        snapshot: None,
        build: BuildView::Idle,
        prepared: None,
        preparation: PrepareView::Idle,
        cleanup_pending: false,
    }
}

/// A managed Ubuntu recipe whose packages match [`built`]'s, with `snapshot` and `build`: individual tests change one
/// input to make the snapshot stale.
fn managed(snapshot: Option<SnapshotView>, build: BuildView) -> RecipeView {
    RecipeView {
        snapshot,
        build,
        packages: vec!["git".into(), "ripgrep".into()],
        ..view(RecipeBase::ManagedUbuntu, 0)
    }
}

/// A snapshot built 2026-11-03 14:02:05 UTC with `packages` on 50 GB.
fn built(packages: &[&str]) -> SnapshotView {
    SnapshotView {
        id: "snap1".into(),
        built_at: 1_793_714_525,
        base: RecipeBase::ManagedUbuntu,
        disk_gb: 50,
        packages: packages.iter().map(|p| p.to_string()).collect(),
        claude_version: Some("2.0.1 (Claude Code)".into()),
        codex_version: Some("codex-cli 0.40.0".into()),
    }
}

/// An ordinary line, for comparing with [`line_texts`].
fn plain(text: &str) -> (Tone, String) {
    (Tone::Plain, text.into())
}

/// This one line is how the list tells recipes apart at a glance, and the old app's mockup wording is the contract its
/// browser spec checked too. Specified (the old `summaries_follow_the_mockup`): the summary reads size, base and
/// package count, then the tunnel count only when there are tunnels, with singular and plural counts, and each base
/// named as the old list named it.
#[test]
fn summaries_follow_the_mockup() {
    let packages = |n: usize| RecipeView {
        packages: (0..n).map(|n| format!("pkg{n}")).collect(),
        ..view(RecipeBase::ManagedUbuntu, 1)
    };
    assert_eq!(
        summary(&packages(12)),
        "4 CPU · 8 GB · 50 GB · base: Tensorlake Ubuntu · 12 packages · 1 tunnel"
    );
    let mut one_package = packages(1);
    one_package.tunnels = view(RecipeBase::ManagedUbuntu, 2).tunnels;
    assert_eq!(
        summary(&one_package),
        "4 CPU · 8 GB · 50 GB · base: Tensorlake Ubuntu · 1 package · 2 tunnels"
    );
    assert_eq!(
        summary(&view(
            RecipeBase::TensorlakeImage {
                image: "team-base".into()
            },
            0
        )),
        "4 CPU · 8 GB · 50 GB · base: your image team-base · 0 packages"
    );
    assert_eq!(
        summary(&view(
            RecipeBase::TensorlakeSnapshot {
                snapshot_id: "abc123".into()
            },
            0
        )),
        "4 CPU · 8 GB · 50 GB · base: your snapshot abc123 · 0 packages"
    );
}

/// A container base belongs to another provider, so offering Build here would start Tensorlake work for a recipe
/// Tensorlake cannot run, and the helm would refuse it. Specified (the old
/// `container_bases_explain_their_provider_and_offer_no_tensorlake_build`): the summary names the image and its login
/// account as Podman only; the status offers no build and warns that the base cannot run on Tensorlake.
#[test]
fn container_bases_explain_their_provider_and_offer_no_tensorlake_build() {
    let recipe = view(
        RecipeBase::ContainerImage {
            reference: "example.test/team/dev:latest".into(),
            user: "dev".into(),
        },
        0,
    );
    assert!(
        summary(&recipe)
            .contains("container image example.test/team/dev:latest as dev (Podman only)")
    );
    let status = snapshot_status(&recipe);
    assert_eq!(status.action, None);
    assert!(
        line_texts(&status).contains(&(
            Tone::Warning,
            "This recipe's base cannot run on Tensorlake".into()
        )),
        "{:?}",
        line_texts(&status)
    );
}

/// This is the whole status vocabulary of the list, settled with the user in the old app: each state must offer
/// exactly the build the helm accepts, and say what the user would otherwise have to find out from a bill. Specified
/// (the old `snapshot_status_follows_the_build_state`): with no snapshot, "No snapshot yet" and Build; built, when and
/// with which agents, and Rebuild; a running build keeps the built line, adds its step as "step n of 9", pulses, and
/// offers nothing; a failed one names its step and the reason, with Retry; an interrupted one offers Retry; changed
/// packages (in any order counts as unchanged) and a smaller disk warn, and pending cleanup adds a muted line; image
/// and snapshot bases offer the same build as managed Ubuntu.
#[test]
fn snapshot_status_follows_the_build_state() {
    let idle = snapshot_status(&managed(None, BuildView::Idle));
    assert_eq!(
        (line_texts(&idle), idle.running(), idle.action),
        (
            vec![plain("No snapshot yet")],
            false,
            Some(BuildAction::Build)
        )
    );

    let built_line = "Snapshot built 2026-11-03 14:02 UTC · 2.0.1 (Claude Code) · codex-cli 0.40.0";
    let rebuilt = snapshot_status(&managed(Some(built(&["ripgrep", "git"])), BuildView::Idle));
    assert_eq!(
        (line_texts(&rebuilt), rebuilt.action),
        (vec![plain(built_line)], Some(BuildAction::Rebuild))
    );

    let building = snapshot_status(&managed(
        Some(built(&["git", "ripgrep"])),
        BuildView::Building {
            step: BuildStep::Packages,
            started_at: 1,
        },
    ));
    assert_eq!(
        (line_texts(&building), building.running(), building.action),
        (
            vec![
                plain(built_line),
                plain("Building snapshot: installing packages (step 5 of 9)")
            ],
            true,
            None
        )
    );

    let failed = snapshot_status(&managed(
        None,
        BuildView::Failed {
            step: BuildStep::Packages,
            message: "E: Unable to locate package nope".into(),
            at: 1,
        },
    ));
    assert_eq!(
        (line_texts(&failed), failed.action),
        (
            vec![(
                Tone::Error,
                "Snapshot build failed while installing packages (step 5 of 9): E: Unable to locate package nope"
                    .into()
            )],
            Some(BuildAction::Retry)
        )
    );

    let interrupted = snapshot_status(&managed(None, BuildView::Interrupted { started_at: 1 }));
    assert_eq!(interrupted.action, Some(BuildAction::Retry));

    let mut stale = managed(Some(built(&["git"])), BuildView::Idle);
    stale.size.disk_gb = 40;
    stale.cleanup_pending = true;
    let stale = line_texts(&snapshot_status(&stale));
    assert!(
        stale[1].0 == Tone::Warning && stale[1].1.starts_with("Packages changed"),
        "{stale:?}"
    );
    assert!(
        stale[2].0 == Tone::Warning
            && stale[2]
                .1
                .starts_with("The disk is now smaller than this snapshot's 50 GB"),
        "{stale:?}"
    );
    assert_eq!(stale[3].0, Tone::Muted);

    for base in [
        RecipeBase::TensorlakeImage {
            image: "team-base".into(),
        },
        RecipeBase::TensorlakeSnapshot {
            snapshot_id: "abc".into(),
        },
    ] {
        let status = snapshot_status(&view(base, 0));
        assert_eq!(
            (line_texts(&status), status.action),
            (vec![plain("No snapshot yet")], Some(BuildAction::Build))
        );
    }
}

/// A base edit keeps the previous snapshot and its build activity, so the warning must tell a changed base from changed
/// packages, and an image or snapshot base cannot be compared by its kind alone. Specified (the old
/// `base_changes_keep_the_snapshot_and_build_actions`): for each Tensorlake base, matching build inputs show only the
/// built line with Rebuild; another identifier of the same kind warns "Base changed" with Rebuild; a running build
/// offers nothing and ends on its step; failed and interrupted builds offer Retry, the latter with the muted cleanup
/// line last.
#[test]
fn base_changes_keep_the_snapshot_and_build_actions() {
    for base in [
        RecipeBase::ManagedUbuntu,
        RecipeBase::TensorlakeImage {
            image: "team-base".into(),
        },
        RecipeBase::TensorlakeSnapshot {
            snapshot_id: "snapshot-1".into(),
        },
    ] {
        let mut recipe = managed(Some(built(&["git", "ripgrep"])), BuildView::Idle);
        recipe.base = base.clone();
        if let Some(snapshot) = recipe.snapshot.as_mut() {
            snapshot.base = base.clone();
        }
        let status = snapshot_status(&recipe);
        assert_eq!(status.action, Some(BuildAction::Rebuild));
        assert_eq!(
            status.lines.len(),
            1,
            "matching build inputs need no warning"
        );

        recipe.base = match &base {
            RecipeBase::ManagedUbuntu | RecipeBase::TensorlakeImage { .. } => {
                RecipeBase::TensorlakeImage {
                    image: "another-base".into(),
                }
            }
            RecipeBase::TensorlakeSnapshot { .. } => RecipeBase::TensorlakeSnapshot {
                snapshot_id: "another-snapshot".into(),
            },
            RecipeBase::ContainerImage { .. } => RecipeBase::ContainerImage {
                reference: "example.test/another-base:latest".into(),
                user: "dev".into(),
            },
        };
        let lines = line_texts(&snapshot_status(&recipe));
        assert!(lines[0].1.starts_with("Snapshot built"), "{lines:?}");
        assert!(
            lines[1].0 == Tone::Warning && lines[1].1.starts_with("Base changed"),
            "{lines:?}"
        );
        assert_eq!(snapshot_status(&recipe).action, Some(BuildAction::Rebuild));

        recipe.build = BuildView::Building {
            step: BuildStep::Codex,
            started_at: 1,
        };
        let status = snapshot_status(&recipe);
        assert!(status.running());
        assert_eq!(status.action, None);
        assert_eq!(
            line_texts(&status).last(),
            Some(&plain("Building snapshot: installing Codex (step 7 of 9)"))
        );

        recipe.build = BuildView::Failed {
            step: BuildStep::Codex,
            message: "installer failed".into(),
            at: 1,
        };
        assert_eq!(snapshot_status(&recipe).action, Some(BuildAction::Retry));
        recipe.build = BuildView::Interrupted { started_at: 1 };
        recipe.cleanup_pending = true;
        let status = snapshot_status(&recipe);
        assert_eq!(status.action, Some(BuildAction::Retry));
        assert_eq!(
            line_texts(&status).last().map(|line| line.0),
            Some(Tone::Muted)
        );
    }
}

/// The pulsing dot says which line is the running build; on any other line it would claim that a pending cleanup is
/// what runs. The old row drew it before the last line, which a pending cleanup or a base warning can follow, so this
/// port marks the build's own line instead. Specified: a build at its third step with cleanup pending has exactly one
/// running line, "Building snapshot: waiting for the build sandbox to start (step 3 of 9)", and the muted cleanup line
/// after it does not run.
#[test]
fn only_the_running_builds_line_carries_the_pulse() {
    let mut recipe = managed(
        None,
        BuildView::Building {
            step: BuildStep::Starting,
            started_at: 1,
        },
    );
    recipe.cleanup_pending = true;
    let status = snapshot_status(&recipe);
    let running: Vec<String> = status
        .lines
        .iter()
        .filter(|line| line.running)
        .map(|line| line.text())
        .collect();
    assert_eq!(
        running,
        ["Building snapshot: waiting for the build sandbox to start (step 3 of 9)"]
    );
    assert_eq!(
        status.lines.last().map(|line| (line.tone, line.running)),
        Some((Tone::Muted, false))
    );
}

// ---- The section, over the fake helm -------------------------------------------------------------------------------

/// A recipe as `GET /api/tensorlake/recipes` lists it, and so as the pushed catalog carries it: `name` on managed
/// Ubuntu with 2 CPUs, 4 GB, 20 GB and the package `jq`, with `build` as its build state and `snapshot` as its built
/// snapshot.
pub(super) fn entry(name: &str, build: Value, snapshot: Value) -> Value {
    json!({
        "name": name,
        "size": { "cpus": 2, "memory_gb": 4, "disk_gb": 20 },
        "base": { "kind": "tensorlake_ubuntu" },
        "packages": ["jq"],
        "claude": null, "codex": null,
        "claude_login_credential": null, "codex_login_credential": null, "github_token_credential": null,
        "tunnels": [],
        "snapshot": snapshot,
        "build": build,
        "prepared": null,
        "preparation": { "state": "idle" },
        "cleanup_pending": false,
    })
}

/// `recipes` keyed by name, as the helm files the catalog's entries.
fn keyed(recipes: &[Value]) -> (Vec<String>, Vec<KeyedEntry>) {
    let names: Vec<String> = recipes
        .iter()
        .map(|recipe| recipe["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    let entries = names
        .iter()
        .zip(recipes)
        .map(|(key, value)| KeyedEntry {
            key: key.clone(),
            value: value.clone(),
        })
        .collect();
    (names, entries)
}

/// The pushed catalog holding `recipes`, in that order, for the helm's first snapshot.
pub(super) fn catalog(recipes: Vec<Value>) -> CollectionState {
    let (names, entries) = keyed(&recipes);
    let order: Vec<&str> = names.iter().map(String::as_str).collect();
    CollectionState {
        collection: Collection::Recipes,
        meta: meta(&order),
        entries,
    }
}

/// Push `recipes` as the whole catalog, in that order: each entry replaced whole, and nothing removed, since no test
/// here drops a recipe.
pub(super) fn push(helm: &mut FakeHelm, recipes: Vec<Value>) {
    let (names, upsert) = keyed(&recipes);
    let order: Vec<&str> = names.iter().map(String::as_str).collect();
    helm.push(CollectionChanges {
        collection: Collection::Recipes,
        meta: meta(&order),
        upsert,
        remove: Vec::new(),
    });
}

/// Push the helm's failed read of the catalog: `error` as its reason, the last order `kept` and no entry changed, as
/// the helm's diff sends a failed read.
fn push_unreadable(helm: &mut FakeHelm, error: &str, kept: &[&str]) {
    helm.push(CollectionChanges {
        collection: Collection::Recipes,
        meta: CollectionMeta {
            availability: Availability::Unavailable {
                error: error.into(),
            },
            ..meta(kept)
        },
        upsert: Vec::new(),
        remove: Vec::new(),
    });
}

/// The Tensorlake account as `GET /api/tensorlake` answers it, with or without a saved API key.
pub(super) fn account(key_set: bool) -> Value {
    json!({ "api_key_set": key_set, "create_without_asking": false, "helm_tag": "ab12",
            "ssh_key": { "state": "not_created" } })
}

/// The one command sent since the last call, checked to be the account read, which is all a showing of the section
/// reads: the catalog comes from the push.
fn one_account_read(helm: &mut FakeHelm) -> FakeCommand {
    let mut reads = sent(helm);
    assert_eq!(
        reads
            .iter()
            .map(|read| (read.method, read.path.as_str()))
            .collect::<Vec<_>>(),
        [(Method::Get, "/api/tensorlake")],
        "only the account is read"
    );
    reads.remove(0)
}

/// Answer the account read the section sends next, with or without a saved key.
pub(super) fn answer_account(helm: &mut FakeHelm, cx: &mut VisualTestContext, key_set: bool) {
    let read = one_account_read(helm);
    helm.reply(read.id, 200, Some(account(key_set)));
    settle(cx);
}

/// The gear's rail over a helm whose first snapshot pushes `recipes` as the catalog, and no recipe places (the
/// per-place lines have their own tests, `super::recipe_place_tests`).
fn open_with(cx: &mut TestAppContext, recipes: Vec<Value>) -> (FakeHelm, &mut VisualTestContext) {
    let (helm, cx, _gear) = open_rail_with(
        cx,
        &Preferences::default(),
        vec![catalog(recipes), places_state(Vec::new())],
    );
    (helm, cx)
}

/// Open Settings, show Recipes, and answer its one read, the account's, with `key_set`.
fn show_recipes(helm: &mut FakeHelm, cx: &mut VisualTestContext, key_set: bool) {
    open_settings(cx);
    assert_eq!(sent(helm), [], "the premise: Sessions reads nothing");
    click(cx, section_button(Section::Recipes));
    answer_account(helm, cx, key_set);
}

/// Show Sessions and then Recipes again, answering the account read that brings with `key_set`.
fn show_again(helm: &mut FakeHelm, cx: &mut VisualTestContext, key_set: bool) {
    click(cx, section_button(Section::Sessions));
    click(cx, section_button(Section::Recipes));
    answer_account(helm, cx, key_set);
}

/// The window's painted text without the isolates around host values, for looking up sentences that name one.
pub(super) fn words(cx: &mut VisualTestContext) -> String {
    painted_words(&painted(cx))
}

/// The work in flight on the recipe `name`.
fn recipe_work(cx: &mut VisualTestContext, name: &str) -> Option<Work> {
    cx.update(|_, cx| {
        session::current_operations(cx)
            .and_then(|operations| operations.read(cx).in_flight(&OpKey::recipe(name)))
    })
}

/// Whether the control under `id` acts when pressed; `None` when it is not drawn. A busy button stays focusable (so
/// [`available`] still says yes) but takes no press, which is what tells it from an available one.
pub(super) fn pressable(cx: &mut VisualTestContext, id: &str) -> Option<bool> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| node.flags.click)
}

/// The one command sent since the last call, checked to be `name`'s build: `POST
/// /api/tensorlake/recipes/<name>/build` with no body.
fn one_build(helm: &mut FakeHelm, name: &str) -> FakeCommand {
    let mut sent = sent(helm);
    assert_eq!(sent.len(), 1, "one command: {sent:?}");
    let build = sent.remove(0);
    assert_eq!(
        (build.method, build.path.clone(), build.body.clone()),
        (
            Method::Post,
            format!("/api/tensorlake/recipes/{name}/build"),
            None
        ),
        "the build's POST"
    );
    build
}

/// The ticker's count of delivered ticks.
pub(super) fn ticks(cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| crate::ticker::ticks(cx))
}

/// A build at `step`, as the catalog reports a running one.
fn building_at(step: &str) -> Value {
    json!({ "state": "building", "step": step, "started_at": 1_793_714_525 })
}

/// A build at its fourth step.
pub(super) fn building() -> Value {
    building_at("connecting")
}

/// No build running.
pub(super) fn idle() -> Value {
    json!({ "state": "idle" })
}

/// A built snapshot as the list reports it, with both agents' versions.
fn snapshot() -> Value {
    json!({ "id": "snap-1", "built_at": 1_793_714_525, "base": { "kind": "tensorlake_ubuntu" }, "disk_gb": 20,
            "packages": ["jq"], "claude_version": "2.0.1 (Claude Code)", "codex_version": "codex-cli 0.40.0" })
}

/// The catalog is pushed, so the rows come from the store and a showing of the section must not read it, while whether
/// an API key is saved is not pushed and must still be read each time it shows (a key saved elsewhere since must make
/// Build available). SPEC.md: "The list shows each recipe's name, … base, package count, tunnel count and build
/// status". Specified: showing Recipes sends exactly one command, `GET /api/tensorlake`, and no catalog read; the rows
/// draw at once from the pushed catalog (one built, with its agents' versions and Rebuild; one never built, with
/// Build) in the Recipes list, with Build unavailable and no no-key note until the account answers, and available once
/// it says a key is saved; showing Sessions and Recipes again reads the account once more and nothing else.
#[gpui::test]
fn recipes_draws_the_pushed_catalog_and_reads_only_the_account_when_shown(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_with(
        cx,
        vec![
            entry("rust-dev", idle(), snapshot()),
            entry("py-data", idle(), Value::Null),
        ],
    );
    open_settings(cx);
    click(cx, section_button(Section::Recipes));
    let read = one_account_read(&mut helm);

    assert!(!drawn(cx, "recipes-loading"));
    assert_eq!(label_of(cx, "recipes-list").as_deref(), Some("Recipes"));
    let text = words(cx);
    assert!(
        text.contains(
            "rust-dev 2 CPU · 4 GB · 20 GB · base: Tensorlake Ubuntu · 1 package Snapshot built 2026-11-03 14:02 UTC · \
             2.0.1 (Claude Code) · codex-cli 0.40.0 Not prepared yet; the first new host or session prepares it \
             (about a minute). Rebuild snapshot"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "py-data 2 CPU · 4 GB · 20 GB · base: Tensorlake Ubuntu · 1 package No snapshot yet Not prepared yet; \
             the first new host or session prepares it (about a minute). Build snapshot"
        ),
        "{text}"
    );
    assert_eq!(
        label_of(cx, "recipe-build-py-data").as_deref(),
        Some("Build snapshot for py-data"),
        "the button names its recipe"
    );
    assert_eq!(
        available(cx, "recipe-build-py-data"),
        Some(false),
        "no build before the key is known"
    );
    assert!(
        !text.contains("Save an API key"),
        "nor a claim that no key is saved"
    );

    helm.reply(read.id, 200, Some(account(true)));
    settle(cx);
    assert_eq!(available(cx, "recipe-build-py-data"), Some(true));

    show_again(&mut helm, cx, true);
}

/// The reason the catalog is pushed: a build takes minutes, and the section must follow it while it stays shown, as
/// the old list did by reading again on every change the helm announced. Specified: with Recipes shown, a push that
/// adds a recipe draws its row, and a push that moves an existing recipe's build draws the new step, each with no
/// command sent.
#[gpui::test]
fn a_pushed_change_redraws_the_rows_with_no_read(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_with(cx, vec![entry("rust-dev", idle(), Value::Null)]);
    show_recipes(&mut helm, cx, true);
    assert!(!drawn(cx, "recipe-e2e-new"), "the premise: not listed yet");

    push(
        &mut helm,
        vec![
            entry("rust-dev", building_at("packages"), Value::Null),
            entry("e2e-new", idle(), Value::Null),
        ],
    );
    settle(cx);
    assert!(drawn(cx, "recipe-e2e-new"), "the pushed recipe is listed");
    assert!(
        words(cx).contains("Building snapshot: installing packages (step 5 of 9)"),
        "{}",
        words(cx)
    );
    assert_eq!(sent(&mut helm), [], "a push is not answered with a read");
}

/// Until the store has the catalog there is nothing to list, and "No recipes yet." would be a false claim. The helm
/// pushes the catalog with its first snapshot, so this shows only against a snapshot without it. Specified: with no
/// catalog in the snapshot, Recipes shows "Loading recipes…" and neither the empty line nor a list.
#[gpui::test]
fn a_catalog_not_pushed_yet_reads_as_loading(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_recipes(&mut helm, cx, true);
    assert!(drawn(cx, "recipes-loading"));
    assert!(!drawn(cx, "recipes-empty"));
    assert!(!drawn(cx, "recipes-list"));
}

/// A failed read must not look like an empty catalog, or the user would believe there are no recipes. Specified: an
/// empty pushed catalog says "No recipes yet."; once the helm pushes that it could not read the catalog, with no rows to
/// keep, the helm's words show alone as an alert with Retry loading, and "No recipes yet." goes; Retry loading sends the
/// account read, after which the helm re-reads its pushed collections, and its push of the readable empty catalog drops
/// the alert and says "No recipes yet." again.
#[gpui::test]
fn an_empty_catalog_says_so_and_an_unreadable_one_shows_only_its_reason(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_with(cx, Vec::new());
    show_recipes(&mut helm, cx, true);
    assert!(words(cx).contains("No recipes yet."));

    push_unreadable(&mut helm, "could not read Tensorlake recipes", &[]);
    settle(cx);
    assert_eq!(
        label_of(cx, "recipes-load-error").as_deref(),
        Some("could not read Tensorlake recipes")
    );
    assert!(
        !drawn(cx, "recipes-empty"),
        "a failed read is not an empty list"
    );
    assert!(!drawn(cx, "recipes-loading"));

    click(cx, "recipes-retry");
    answer_account(&mut helm, cx, true);
    push(&mut helm, Vec::new());
    settle(cx);
    assert!(!drawn(cx, "recipes-load-error"));
    assert!(drawn(cx, "recipes-empty"));
}

/// SPEC.md: "A list refresh that fails keeps the rows shown, says they may be out of date, and offers **Retry
/// loading**." Dropping the rows would hide every recipe over a passing failure; keeping them silently would leave a
/// stale list looking current. The helm's failed read keeps the entries it pushed before, so the store still has them.
/// Specified: with a recipe listed, the helm's push that it could not read the catalog keeps the row and says "Could not
/// refresh the recipes, so this list may be out of date: " with the helm's words; Retry loading sends exactly the
/// account read; the helm's next push of a readable catalog lists the new recipe and the alert goes.
#[gpui::test]
fn an_unreadable_catalog_keeps_the_rows_and_retry_loading_asks_again(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_with(cx, vec![entry("rust-dev", idle(), Value::Null)]);
    show_recipes(&mut helm, cx, true);
    push_unreadable(&mut helm, "recipe store unavailable", &["rust-dev"]);
    settle(cx);
    assert!(drawn(cx, "recipe-rust-dev"), "the row stays");
    assert_eq!(
        label_of(cx, "recipes-load-error").as_deref(),
        Some(
            "Could not refresh the recipes, so this list may be out of date: recipe store unavailable"
        )
    );

    click(cx, "recipes-retry");
    answer_account(&mut helm, cx, true);
    push(
        &mut helm,
        vec![
            entry("rust-dev", idle(), Value::Null),
            entry("e2e-refresh", idle(), Value::Null),
        ],
    );
    settle(cx);
    assert!(drawn(cx, "recipe-e2e-refresh"), "the new recipe is listed");
    assert!(!drawn(cx, "recipes-load-error"));
}

/// A build takes minutes and bills, so the helm accepts it (202) and runs it on its own task (SPEC_impl.md, "The UI
/// channel": recipe builds take the accepted form), and pushes the recipe building before its reply; the row must then
/// follow the build's steps to its end without reading anything. It goes through the store under the recipe's key, so
/// a second press while the first is out sends nothing. Specified: Build snapshot sends `POST
/// /api/tensorlake/recipes/<name>/build` with no body, as Save work on the recipe's key, and the button is busy while
/// it is out; the helm's push of the build at its fourth step and then the 202 settle the key, show "Building
/// snapshot: connecting and checking the base (step 4 of 9)" and no build button, and send nothing; a push of the
/// fifth step shows it; a push of the built snapshot shows "Snapshot built" with Rebuild snapshot; and nothing is read
/// throughout.
#[gpui::test]
fn build_is_accepted_under_the_recipes_key_and_the_push_follows_it_to_its_end(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = open_with(cx, vec![entry("e2e-build", idle(), Value::Null)]);
    show_recipes(&mut helm, cx, true);
    click(cx, "recipe-build-e2e-build");
    let build = one_build(&mut helm, "e2e-build");
    assert_eq!(recipe_work(cx, "e2e-build"), Some(Work::Save));
    assert_eq!(
        (
            available(cx, "recipe-build-e2e-build"),
            pressable(cx, "recipe-build-e2e-build")
        ),
        (Some(true), Some(false)),
        "busy while its request is out: still focusable, taking no press"
    );
    click(cx, "recipe-build-e2e-build");
    assert_eq!(
        sent(&mut helm),
        [],
        "a second press while the first is out sends nothing"
    );

    // The helm pushes what a command changed before its reply.
    push(&mut helm, vec![entry("e2e-build", building(), Value::Null)]);
    helm.reply(
        build.id,
        202,
        Some(entry("e2e-build", building(), Value::Null)),
    );
    settle(cx);
    assert_eq!(
        recipe_work(cx, "e2e-build"),
        None,
        "accepted settles the key"
    );
    assert_eq!(sent(&mut helm), [], "nothing is read after the build");
    assert!(
        words(cx).contains("Building snapshot: connecting and checking the base (step 4 of 9)"),
        "{}",
        words(cx)
    );
    assert!(
        !drawn(cx, "recipe-build-e2e-build"),
        "a running build offers no build"
    );

    push(
        &mut helm,
        vec![entry("e2e-build", building_at("packages"), Value::Null)],
    );
    settle(cx);
    assert!(
        words(cx).contains("Building snapshot: installing packages (step 5 of 9)"),
        "{}",
        words(cx)
    );
    push(&mut helm, vec![entry("e2e-build", idle(), snapshot())]);
    settle(cx);
    assert!(words(cx).contains("Snapshot built"), "{}", words(cx));
    assert_eq!(
        label_of(cx, "recipe-build-e2e-build").as_deref(),
        Some("Rebuild snapshot for e2e-build")
    );
    assert_eq!(sent(&mut helm), [], "and nothing is read throughout");
}

/// Every build needs a saved Tensorlake API key, and the old list said so instead of offering a request the helm would
/// refuse. Specified: with no key saved, Build snapshot is drawn unavailable with "Save an API key in Settings →
/// Credentials to build snapshots." beside it, and a click sends nothing.
#[gpui::test]
fn without_an_api_key_build_is_unavailable_and_says_why(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_with(cx, vec![entry("rust-dev", idle(), Value::Null)]);
    show_recipes(&mut helm, cx, false);
    assert_eq!(available(cx, "recipe-build-rust-dev"), Some(false));
    assert!(
        words(cx).contains(
            "Build snapshot Save an API key in Settings → Credentials to build snapshots."
        )
    );
    click(cx, "recipe-build-rust-dev");
    assert_eq!(sent(&mut helm), []);
}

/// A refused build must say why where the user pressed it, in the helm's words, or the press looks lost; and it must
/// not linger once the user tries again. Specified: a build refused with 400 "create Farhelm's SSH key in Settings →
/// Tensorlake and register it with Tensorlake first" answering the build's POST shows those words as an alert under the
/// row, sends nothing more, and the button is ready again; the next press sends the build's POST again and clears the
/// line while it is out.
#[gpui::test]
fn a_refused_build_shows_the_helms_words_under_its_row_until_the_next_build(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = open_with(cx, vec![entry("rust-dev", idle(), Value::Null)]);
    show_recipes(&mut helm, cx, true);
    click(cx, "recipe-build-rust-dev");
    let build = one_build(&mut helm, "rust-dev");
    let reason =
        "create Farhelm's SSH key in Settings → Tensorlake and register it with Tensorlake first";
    helm.reply(build.id, 400, Some(json!(reason)));
    settle(cx);
    assert_eq!(
        label_of(cx, "recipe-rust-dev-build-error").as_deref(),
        Some(reason)
    );
    assert_eq!(sent(&mut helm), [], "a refusal sends nothing more");
    assert_eq!(available(cx, "recipe-build-rust-dev"), Some(true));

    click(cx, "recipe-build-rust-dev");
    one_build(&mut helm, "rust-dev");
    assert!(
        !drawn(cx, "recipe-rust-dev-build-error"),
        "the next build clears it"
    );
}

/// A refusal is about a build that did not start; once the pushed catalog shows the recipe building (started here or
/// by another client since), the refusal says nothing true any more, and kept on the key it would come back under the
/// row once that build ends. The old row's rule: a refusal stays "until the next click or a re-read shows the build
/// running". Only a push that shows the recipe building may drop it: the helm pushes on every change it announces,
/// most of them about something else, and the refusal must outlive those. Specified: after a refused build, a push
/// that leaves the recipe idle keeps the refusal's line; a push of the recipe building drops it; and a push of the
/// recipe built does not bring it back.
#[gpui::test]
fn a_push_showing_the_build_running_drops_an_older_refusal_for_good(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_with(cx, vec![entry("rust-dev", idle(), Value::Null)]);
    show_recipes(&mut helm, cx, true);
    click(cx, "recipe-build-rust-dev");
    let build = one_build(&mut helm, "rust-dev");
    helm.reply(
        build.id,
        400,
        Some(json!("save a Tensorlake API key in Settings first")),
    );
    settle(cx);
    assert!(
        drawn(cx, "recipe-rust-dev-build-error"),
        "the premise: the refusal shows"
    );

    let mut pushed = |build: Value, snapshot: Value, cx: &mut VisualTestContext| {
        push(
            &mut helm,
            vec![
                entry("rust-dev", build, snapshot),
                entry("other", idle(), Value::Null),
            ],
        );
        settle(cx);
    };
    pushed(idle(), Value::Null, cx);
    assert!(drawn(cx, "recipe-other"), "the premise: the push arrived");
    assert!(
        drawn(cx, "recipe-rust-dev-build-error"),
        "a push that shows no build keeps it"
    );

    pushed(building(), Value::Null, cx);
    assert!(
        words(cx).contains("Building snapshot"),
        "the premise: the push shows it building"
    );
    assert!(
        !drawn(cx, "recipe-rust-dev-build-error"),
        "a running build drops it"
    );

    pushed(idle(), snapshot(), cx);
    assert!(
        words(cx).contains("Snapshot built"),
        "the premise: the build ended"
    );
    assert!(
        !drawn(cx, "recipe-rust-dev-build-error"),
        "and it does not come back"
    );
}

/// The ticker's rule: no view holds the running pulse while it draws no pulsing mark, since a held pulse keeps the
/// tick loop waking ten times a second. Settings keeps the Recipes section, undrawn, while another section shows, and a
/// row pushed as building stays building until the helm pushes otherwise. Specified: with a build running and Recipes
/// shown, the ticker ticks; once Sessions shows, half a second delivers no tick; showing Recipes again, still
/// building, ticks again.
#[gpui::test]
fn hiding_recipes_lets_the_running_pulse_go(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_with(cx, vec![entry("py-data", building(), Value::Null)]);
    show_recipes(&mut helm, cx, true);
    let before = ticks(cx);
    advance(cx, std::time::Duration::from_millis(500));
    assert!(ticks(cx) > before, "the premise: the build's pulse ticks");

    click(cx, section_button(Section::Sessions));
    let hidden = ticks(cx);
    advance(cx, std::time::Duration::from_millis(500));
    assert_eq!(ticks(cx), hidden, "nothing ticks for a section nobody sees");

    click(cx, section_button(Section::Recipes));
    answer_account(&mut helm, cx, true);
    let shown = ticks(cx);
    advance(cx, std::time::Duration::from_millis(500));
    assert!(ticks(cx) > shown, "shown again, it pulses again");
}

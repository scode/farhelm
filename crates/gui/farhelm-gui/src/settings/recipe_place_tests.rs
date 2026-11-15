//! Tests of Settings → Recipes' prepared-snapshot line and per-place lines: the prepared line's words from a recipe's
//! view alone (ported from the old list's own unit tests), and, over the helm client's fake, the lines the pushed
//! places draw with the hosts' names, the per-place Rebuild with its accepted reply and its refusal, the Podman checks
//! asked once per opening, and the clock and pulse the section holds only while shown.
//!
//! The dialog is opened from the gear as in the dialog's own tests (`super::tests`), and the section is shown as in the
//! recipe list's tests (`super::recipe_tests`), whose helpers these share.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::RecipePlaceView;
use farhelm_proto::recipes::{
    PrepareStep, PrepareView, PreparedSnapshotView, PreparedStaleReason, RecipeBase, RecipeView,
};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState,
    ContainerCheckView, HostView, KeyedEntry, Preferences,
};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::Section;
use super::recipe_tests::{
    account, answer_account, building, catalog, entry, idle, pressable, push, ticks, view, words,
};
use super::recipes::Tone;
use super::recipes::prepared::{age, prepared_status};
use super::tests::{
    Opened, available, drawn, label_of, open_rail_with, open_settings, section_button, sent,
};
use crate::hosts::tests::{aliased, connected, host, hosts};
use crate::operations::{OpKey, Work};
use crate::session;
use crate::test_support::{advance, click, meta, settle};

// ---- The prepared line, from a view alone --------------------------------------------------------------------------

/// A managed Ubuntu recipe with a ready prepared snapshot made at second 100 for Farhelm 0.38.0. Its IDs are opaque on
/// purpose: none belongs in the line, which needs only age, version and state.
fn prepared_recipe() -> RecipeView {
    let mut recipe = view(RecipeBase::ManagedUbuntu, 0);
    recipe.prepared = Some(PreparedSnapshotView {
        id: "private-prepared-id".into(),
        prepared_at: 100,
        farhelm_version: "0.38.0".into(),
        source_snapshot: "private-built-id".into(),
        stale_reason: None,
    });
    recipe
}

/// The line `recipe` shows at `now`: its tone, its words and whether it runs.
fn prepared(recipe: &RecipeView, now: Option<i64>) -> (Tone, String, bool) {
    let line = prepared_status(recipe, now);
    (line.tone, line.text(), line.running)
}

/// Lazy preparation must explain both first use and reuse without claiming that an idle client's read is a current
/// freshness decision: the viewer's clock moves the age on, past expiry too, but only the helm marks a snapshot stale.
/// Specified (the old `prepared_status_keeps_readiness_last_checked_and_ages_display_only`): a recipe never prepared
/// says the first new host or session prepares it, muted; a ready one is positive, "when last checked", with its age
/// in seconds, minutes or hours (never negative), its Farhelm version and the 24-hour rule, or its UTC time when the
/// viewer's clock cannot say; and ages saturate at the extremes.
#[test]
fn prepared_status_keeps_readiness_last_checked_and_ages_display_only() {
    assert_eq!(
        prepared(&view(RecipeBase::ManagedUbuntu, 0), Some(100)),
        (
            Tone::Muted,
            "Not prepared yet; the first new host or session prepares it (about a minute).".into(),
            false
        )
    );
    let recipe = prepared_recipe();
    for (now, when) in [
        (Some(99), "0 s ago"),
        (Some(141), "41 s ago"),
        (Some(160), "1 min ago"),
        (Some(7300), "2 h ago"),
        (Some(86_500), "24 h ago"),
        (Some(86_501), "24 h ago"),
        (None, "1970-01-01 00:01 UTC"),
    ] {
        assert_eq!(
            prepared(&recipe, now),
            (
                Tone::Positive,
                format!(
                    "Ready for new hosts and sessions when last checked · prepared {when} for Farhelm 0.38.0 · \
                     expires after 24 h; checked before use"
                ),
                false
            )
        );
    }
    assert_eq!(age(i64::MIN, i64::MAX), "0 s");
    assert_eq!(age(i64::MAX, i64::MIN), format!("{} h", i64::MAX / 3600));
}

/// Every stale reason the helm reports tells the user when the work will happen, and no client time can override it.
/// Specified (the old `prepared_status_explains_each_helm_stale_reason`): each reason is a warning naming it, with the
/// next-new-host-or-session explanation, whatever the viewer's time.
#[test]
fn prepared_status_explains_each_helm_stale_reason() {
    for (reason, phrase) in [
        (PreparedStaleReason::Expired, "it expired"),
        (PreparedStaleReason::FarhelmUpdated, "Farhelm updated"),
        (
            PreparedStaleReason::SnapshotRebuilt,
            "the snapshot was rebuilt",
        ),
    ] {
        let mut recipe = prepared_recipe();
        if let Some(prepared) = recipe.prepared.as_mut() {
            prepared.stale_reason = Some(reason);
        }
        for now in [Some(0), Some(101), None] {
            assert_eq!(
                prepared(&recipe, now),
                (
                    Tone::Warning,
                    format!(
                        "Not prepared since {phrase}; the next new host or session prepares it (about a minute)."
                    ),
                    false
                )
            );
        }
    }
}

/// A retained snapshot must not hide replacement work that runs or failed. Specified (the old
/// `prepared_status_reports_all_phases_and_lazy_recovery`): each of the five phases shows as "Preparing for new hosts
/// and sessions: <phase> (step n of 5)" with its elapsed time (never negative, left out without a clock), and is the
/// running line; each failed phase is an error naming it with the reason and the retry explanation; an interrupted
/// preparation warns with the same explanation.
#[test]
fn prepared_status_reports_all_phases_and_lazy_recovery() {
    for (index, (step, label)) in [
        (PrepareStep::Creating, "creating the preparer sandbox"),
        (PrepareStep::Starting, "starting the preparer sandbox"),
        (PrepareStep::Installing, "installing Farhelm"),
        (PrepareStep::Snapshotting, "taking the prepared snapshot"),
        (PrepareStep::Finishing, "finishing up"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut recipe = prepared_recipe();
        recipe.preparation = PrepareView::Preparing {
            step,
            started_at: 100,
        };
        for (now, elapsed) in [
            (Some(99), " · 0 s"),
            (Some(141), " · 41 s"),
            (Some(160), " · 1 min"),
            (Some(3700), " · 1 h"),
            (None, ""),
        ] {
            assert_eq!(
                prepared(&recipe, now),
                (
                    Tone::Plain,
                    format!(
                        "Preparing for new hosts and sessions: {label} (step {} of 5){elapsed}",
                        index + 1
                    ),
                    true
                )
            );
        }
        recipe.preparation = PrepareView::Failed {
            step,
            message: "gateway refused".into(),
            at: 200,
        };
        assert_eq!(
            prepared(&recipe, Some(201)),
            (
                Tone::Error,
                format!(
                    "Preparing failed while {label} (step {} of 5): gateway refused. The next new host or session \
                     tries again.",
                    index + 1
                ),
                false
            )
        );
    }
    let mut recipe = prepared_recipe();
    recipe.preparation = PrepareView::Interrupted { started_at: 100 };
    assert_eq!(
        prepared(&recipe, Some(201)),
        (
            Tone::Warning,
            "Preparation stopped when the helm stopped. The next new host or session tries again."
                .into(),
            false
        )
    );
}

// ---- The section, over the fake helm -------------------------------------------------------------------------------

/// One recipe place as `GET /api/recipe-places` lists it, and so as the pushed places carry it: `recipe` at the
/// container host `host` (`None` for the Tensorlake account), with every flag off and nothing recorded unless `fields`
/// sets it.
fn place(recipe: &str, host: Option<i64>, fields: Value) -> Value {
    let mut place = json!({
        "recipe": recipe,
        "place": match host {
            Some(host) => json!({ "kind": "podman_host", "host": host }),
            None => json!({ "kind": "tensorlake_account" }),
        },
        "ready": false, "building": false, "reason": null, "built_at": null, "prepared_at": null, "step": null,
        "cleanup_pending": false,
    });
    if let (Some(place), Some(fields)) = (place.as_object_mut(), fields.as_object()) {
        place.extend(fields.clone());
    }
    place
}

/// `places` keyed as the helm files them (`RecipePlaceView::key`), with that order.
fn keyed_places(places: Vec<Value>) -> (CollectionMeta, Vec<KeyedEntry>) {
    let entries: Vec<KeyedEntry> = places
        .into_iter()
        .map(|value| {
            let view: RecipePlaceView =
                serde_json::from_value(value.clone()).expect("the test's place decodes");
            KeyedEntry {
                key: RecipePlaceView::key(&view.recipe, view.place),
                value,
            }
        })
        .collect();
    let order: Vec<&str> = entries.iter().map(|entry| entry.key.as_str()).collect();
    (meta(&order), entries)
}

/// The pushed places holding `places`, for the helm's first snapshot.
pub(super) fn places_state(places: Vec<Value>) -> CollectionState {
    let (meta, entries) = keyed_places(places);
    CollectionState {
        collection: Collection::RecipePlaces,
        meta,
        entries,
    }
}

/// Push `places` as every recipe's places, each entry replaced whole and nothing removed.
fn push_places(helm: &mut FakeHelm, places: Vec<Value>) {
    let (meta, upsert) = keyed_places(places);
    helm.push(CollectionChanges {
        collection: Collection::RecipePlaces,
        meta,
        upsert,
        remove: Vec::new(),
    });
}

/// `build-box` (registry id 7, a connected ssh host) as the pushed hosts collection lists it, where the places' names
/// come from.
fn build_box() -> CollectionState {
    hosts(&[aliased(
        host(7, "ssh", connected()),
        "build-box",
        "dev@build-box.invalid",
    )])
}

/// The app over the fake helm with `build-box` in the hosts, `recipes` as the catalog and `places` as the places in its
/// first snapshot, Settings open at Recipes and the account read answered with a saved key.
fn show_recipes_with_places(
    cx: &mut TestAppContext,
    recipes: Vec<Value>,
    places: Vec<Value>,
) -> (FakeHelm, &mut VisualTestContext) {
    let (mut helm, cx, _gear) = open_rail_with(
        cx,
        &Preferences::default(),
        vec![build_box(), catalog(recipes), places_state(places)],
    );
    open_settings(cx);
    click(cx, section_button(Section::Recipes));
    answer_account(&mut helm, cx, true);
    (helm, cx)
}

/// The one command sent since the last call, checked to be `name`'s rebuild at container host `host`: `POST
/// /api/recipes/<name>/rebuild` with the place in its body.
fn one_rebuild(helm: &mut FakeHelm, name: &str, host: i64) -> FakeCommand {
    let mut sent = sent(helm);
    assert_eq!(sent.len(), 1, "one command: {sent:?}");
    let rebuild = sent.remove(0);
    assert_eq!(
        (rebuild.method, rebuild.path.clone(), rebuild.body.clone()),
        (
            Method::Post,
            format!("/api/recipes/{name}/rebuild"),
            Some(json!({ "place": { "kind": "podman_host", "host": host } }))
        ),
        "the rebuild's POST"
    );
    rebuild
}

/// SPEC.md: "The list shows each recipe's … build status per place, with Rebuild per place". The places are pushed, so
/// the lines come from the store with nothing read, and the account place is the row's built-snapshot line's, so
/// drawing it here too would say the same thing twice. Specified: showing Recipes reads only the account; the row has
/// one line per container place from the pushed places, none for the Tensorlake account: a registered host by its name
/// ("build-box · Ready · Built <UTC>" with a pending cleanup), and a host the registry no longer lists as "Unavailable
/// place", building with the helm's step and its Rebuild unavailable; a push that moves the second place's step draws
/// the new step with nothing sent.
#[gpui::test]
fn the_pushed_places_draw_a_line_per_container_place_with_its_hosts_name(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_recipes_with_places(
        cx,
        vec![entry("rust-dev", idle(), Value::Null)],
        vec![
            place("rust-dev", None, json!({ "ready": true })),
            place(
                "rust-dev",
                Some(7),
                json!({ "ready": true, "built_at": 1_793_714_525, "cleanup_pending": true }),
            ),
            place(
                "rust-dev",
                Some(9),
                json!({ "building": true, "step": "installing packages" }),
            ),
        ],
    );
    assert!(!drawn(cx, "recipe-rust-dev-places-reading"));
    assert!(
        !drawn(cx, "recipe-rust-dev-place-account"),
        "the account is the snapshot line's"
    );
    assert_eq!(
        label_of(cx, "recipe-rust-dev-place-7").as_deref(),
        Some(
            "build-box · Ready · Built 2026-11-03 14:02:05 UTC · Previous image cleanup is unfinished"
        )
    );
    assert_eq!(
        label_of(cx, "recipe-rust-dev-place-9").as_deref(),
        Some("Unavailable place · Preparing… · installing packages")
    );
    assert_eq!(available(cx, "recipe-rebuild-rust-dev-7"), Some(true));
    assert_eq!(
        available(cx, "recipe-rebuild-rust-dev-9"),
        Some(false),
        "a place that builds offers no Rebuild"
    );
    assert_eq!(
        label_of(cx, "recipe-rebuild-rust-dev-7").as_deref(),
        Some("Rebuild rust-dev at build-box"),
        "the button names its recipe and place"
    );

    push_places(
        &mut helm,
        vec![place(
            "rust-dev",
            Some(9),
            json!({ "building": true, "step": "building the image" }),
        )],
    );
    settle(cx);
    assert_eq!(
        label_of(cx, "recipe-rust-dev-place-9").as_deref(),
        Some("Unavailable place · Preparing… · building the image")
    );
    assert_eq!(sent(&mut helm), [], "a push is not answered with a read");
}

/// Until the store has the places there is nothing to draw, and drawing no line would claim the recipe has no places.
/// The helm pushes them with its first snapshot, so this shows only against a snapshot without them. Specified: with
/// a catalog but no places in the snapshot, the row says "Reading per-place builds…".
#[gpui::test]
fn places_not_pushed_yet_read_as_reading(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail_with(
        cx,
        &Preferences::default(),
        vec![catalog(vec![entry("rust-dev", idle(), Value::Null)])],
    );
    open_settings(cx);
    click(cx, section_button(Section::Recipes));
    answer_account(&mut helm, cx, true);
    assert_eq!(
        label_of(cx, "recipe-rust-dev-places-reading").as_deref(),
        Some("Reading per-place builds…")
    );
}

/// Places the helm could not read must not look like a recipe with no places. Specified: once the helm pushes that it
/// could not read the places, the row shows its words as an alert, as the old rows did for a failed read, and draws no
/// place line.
#[gpui::test]
fn unreadable_places_show_their_reason_in_the_row(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_recipes_with_places(
        cx,
        vec![entry("rust-dev", idle(), Value::Null)],
        vec![place("rust-dev", Some(7), json!({ "ready": true }))],
    );
    assert!(drawn(cx, "recipe-rust-dev-place-7"), "the premise");
    let (kept, _) = keyed_places(vec![place("rust-dev", Some(7), json!({ "ready": true }))]);
    helm.push(CollectionChanges {
        collection: Collection::RecipePlaces,
        meta: CollectionMeta {
            availability: Availability::Unavailable {
                error: "could not read recipe places".into(),
            },
            ..kept
        },
        upsert: Vec::new(),
        remove: Vec::new(),
    });
    settle(cx);
    assert_eq!(
        label_of(cx, "recipe-rust-dev-places-error").as_deref(),
        Some("could not read recipe places")
    );
    assert!(!drawn(cx, "recipe-rust-dev-place-7"));
}

/// A place's Rebuild starts or joins work that can take minutes, so the helm accepts it (202) and pushes the place
/// building before that reply; it shares the recipe's key with the snapshot build, so one request at a time per
/// recipe. Specified: Rebuild sends `POST /api/recipes/<name>/rebuild` with the place, as Save work on the recipe's key,
/// and while it is out every Rebuild and the snapshot build of the recipe are busy; the helm's push of the place
/// building and then the 202 settle the key and show the place building with no Rebuild offered, with nothing read.
#[gpui::test]
fn a_places_rebuild_is_accepted_under_the_recipes_key_and_the_push_shows_it_building(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = show_recipes_with_places(
        cx,
        vec![entry("rust-dev", idle(), Value::Null)],
        vec![place("rust-dev", Some(7), json!({ "ready": true }))],
    );
    click(cx, "recipe-rebuild-rust-dev-7");
    let rebuild = one_rebuild(&mut helm, "rust-dev", 7);
    let work = cx.update(|_, cx| {
        session::current_operations(cx)
            .and_then(|operations| operations.read(cx).in_flight(&OpKey::recipe("rust-dev")))
    });
    assert_eq!(work, Some(Work::Save));
    assert_eq!(
        pressable(cx, "recipe-rebuild-rust-dev-7"),
        Some(false),
        "busy while out"
    );
    assert_eq!(
        pressable(cx, "recipe-build-rust-dev"),
        Some(false),
        "the snapshot build shares the key"
    );

    // The helm pushes what a command changed before its reply.
    push_places(
        &mut helm,
        vec![place(
            "rust-dev",
            Some(7),
            json!({ "building": true, "step": "building the image" }),
        )],
    );
    helm.reply(rebuild.id, 202, None);
    settle(cx);
    assert_eq!(sent(&mut helm), [], "nothing is read after the rebuild");
    assert_eq!(
        label_of(cx, "recipe-rust-dev-place-7").as_deref(),
        Some("build-box · Preparing… · building the image")
    );
    assert_eq!(available(cx, "recipe-rebuild-rust-dev-7"), Some(false));
}

/// A refused Rebuild must say why, in the helm's words, where the recipe's refusals show; and a push that shows the
/// recipe building there settles it, as one showing its snapshot building does. Specified: a Rebuild refused with 409
/// shows the helm's words under the row and sends nothing more; a push of that place building drops the line.
#[gpui::test]
fn a_refused_rebuild_shows_under_the_row_until_a_push_shows_it_building(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_recipes_with_places(
        cx,
        vec![entry("rust-dev", idle(), Value::Null)],
        vec![place("rust-dev", Some(7), json!({ "ready": true }))],
    );
    click(cx, "recipe-rebuild-rust-dev-7");
    let rebuild = one_rebuild(&mut helm, "rust-dev", 7);
    helm.reply(rebuild.id, 409, Some(json!("the recipe is being deleted")));
    settle(cx);
    assert_eq!(
        label_of(cx, "recipe-rust-dev-build-error").as_deref(),
        Some("the recipe is being deleted")
    );
    assert_eq!(sent(&mut helm), [], "a refusal sends nothing more");

    push_places(
        &mut helm,
        vec![place("rust-dev", Some(7), json!({ "building": true }))],
    );
    settle(cx);
    assert!(
        !drawn(cx, "recipe-rust-dev-build-error"),
        "a place building drops it"
    );
}

/// Only a place that has a line can settle a refusal. The Tensorlake account's place has none, and the helm reports it
/// building while a preparation runs, which a launch elsewhere starts and which says nothing about a refused Rebuild or
/// Build snapshot; settling from it would drop a refusal the user never acted on. Specified: after a refused Rebuild, a
/// push of the places holding the Tensorlake account's place building (and the container place ready) keeps the
/// refusal's line.
#[gpui::test]
fn a_place_without_a_line_building_keeps_a_refusal(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_recipes_with_places(
        cx,
        vec![entry("rust-dev", idle(), Value::Null)],
        vec![place("rust-dev", Some(7), json!({ "ready": true }))],
    );
    click(cx, "recipe-rebuild-rust-dev-7");
    let rebuild = one_rebuild(&mut helm, "rust-dev", 7);
    helm.reply(rebuild.id, 409, Some(json!("the recipe is being deleted")));
    settle(cx);
    assert!(
        drawn(cx, "recipe-rust-dev-build-error"),
        "the premise: the refusal shows"
    );

    push_places(
        &mut helm,
        vec![
            place("rust-dev", None, json!({ "building": true })),
            place("rust-dev", Some(7), json!({ "ready": true })),
        ],
    );
    settle(cx);
    assert!(
        drawn(cx, "recipe-rust-dev-build-error"),
        "the account's place building, which draws no line, keeps it"
    );
}

/// Refusals are settled on every push of anything, and a build announces each of its steps, so settling on a recipe
/// being shown building, rather than on its starting to build, would drop a Rebuild refused while another place of the
/// same recipe already builds at the next unrelated push, before anyone could read it. Specified: with place 9 of
/// `rust-dev` already pushed building, a Rebuild at place 7 refused with 409 shows the helm's words; a push of the
/// catalog adding another recipe, which changes nothing about `rust-dev`, keeps them.
#[gpui::test]
fn a_refusal_stays_while_another_place_of_its_recipe_already_builds(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_recipes_with_places(
        cx,
        vec![entry("rust-dev", idle(), Value::Null)],
        vec![
            place("rust-dev", Some(7), json!({ "ready": true })),
            place("rust-dev", Some(9), json!({ "building": true })),
        ],
    );
    assert!(
        place_9_builds(cx),
        "the premise: place 9 has a line, drawn building"
    );
    refuse_rebuild_at_7(&mut helm, cx);

    push(
        &mut helm,
        vec![
            entry("rust-dev", idle(), Value::Null),
            entry("other", idle(), Value::Null),
        ],
    );
    settle(cx);
    assert!(drawn(cx, "recipe-other"), "the premise: the push landed");
    assert_eq!(
        label_of(cx, "recipe-rust-dev-build-error").as_deref(),
        Some("the recipe is being deleted"),
        "an unrelated push keeps it"
    );
}

/// Whether place 9's line is drawn, building: the premise of the tests that need another place of the recipe building.
fn place_9_builds(cx: &mut VisualTestContext) -> bool {
    label_of(cx, "recipe-rust-dev-place-9").is_some_and(|words| words.contains("Preparing…"))
}

/// Press Rebuild at place 7 and have the helm refuse it, checking the refusal's line shows.
fn refuse_rebuild_at_7(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    click(cx, "recipe-rebuild-rust-dev-7");
    let rebuild = one_rebuild(helm, "rust-dev", 7);
    helm.reply(rebuild.id, 409, Some(json!("the recipe is being deleted")));
    settle(cx);
    assert!(
        drawn(cx, "recipe-rust-dev-build-error"),
        "the premise: the refusal shows"
    );
}

/// Each place, and the snapshot, counts as a source of its own: the refusal is about a build that has been asked for
/// since once any source starts, even while another of the recipe's sources was already building (the old row's rule,
/// "until … a re-read shows the build running"). A memo keyed by recipe alone would keep it, stale, after the new build
/// ends. Specified: with place 9 building and a Rebuild at place 7 refused, a push showing the recipe's snapshot
/// building drops the refusal.
#[gpui::test]
fn a_snapshot_starting_while_a_place_builds_drops_the_refusal(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_recipes_with_places(
        cx,
        vec![entry("rust-dev", idle(), Value::Null)],
        vec![
            place("rust-dev", Some(7), json!({ "ready": true })),
            place("rust-dev", Some(9), json!({ "building": true })),
        ],
    );
    assert!(place_9_builds(cx), "the premise: place 9 builds");
    refuse_rebuild_at_7(&mut helm, cx);

    push(&mut helm, vec![entry("rust-dev", building(), Value::Null)]);
    settle(cx);
    assert!(
        !drawn(cx, "recipe-rust-dev-build-error"),
        "the snapshot starting drops it"
    );
}

/// A places read the helm could not make says nothing about which places build, so it must not count as every place
/// stopping; otherwise the push that makes the places readable again would count a place that kept building as
/// starting, and drop a refusal that landed while it built. Specified: with place 9 building and a Rebuild at place 7
/// refused, a push of the places unreadable and then one of them readable again, place 9 still building, keep the
/// refusal.
#[gpui::test]
fn places_read_again_after_a_failure_do_not_count_as_starting(cx: &mut TestAppContext) {
    let both = || {
        vec![
            place("rust-dev", Some(7), json!({ "ready": true })),
            place("rust-dev", Some(9), json!({ "building": true })),
        ]
    };
    let (mut helm, cx) =
        show_recipes_with_places(cx, vec![entry("rust-dev", idle(), Value::Null)], both());
    assert!(place_9_builds(cx), "the premise: place 9 builds");
    refuse_rebuild_at_7(&mut helm, cx);

    let (kept, _) = keyed_places(both());
    helm.push(CollectionChanges {
        collection: Collection::RecipePlaces,
        meta: CollectionMeta {
            availability: Availability::Unavailable {
                error: "could not read recipe places".into(),
            },
            ..kept
        },
        upsert: Vec::new(),
        remove: Vec::new(),
    });
    settle(cx);
    assert!(
        drawn(cx, "recipe-rust-dev-places-error"),
        "the premise: the places are unreadable"
    );
    push_places(&mut helm, both());
    settle(cx);
    assert!(place_9_builds(cx), "the premise: readable again");
    assert_eq!(
        label_of(cx, "recipe-rust-dev-build-error").as_deref(),
        Some("the recipe is being deleted"),
        "the refusal stays"
    );
}

/// The open Recipes section, through the handle the dialog's opening records for tests.
fn recipes_section(cx: &mut VisualTestContext) -> gpui::Entity<super::recipes::Recipes> {
    cx.update(|_, cx| {
        let dialog = cx.global::<Opened>().0.upgrade().expect("Settings is open");
        dialog.read(cx).recipes.clone().expect("Recipes was shown")
    })
}

/// Whether the open Recipes section holds the running pulse and the ticker's clock.
fn holds_time(cx: &mut VisualTestContext) -> (bool, bool) {
    let section = recipes_section(cx);
    cx.update(|_, cx| section.read(cx).holds_time())
}

/// The seconds since the epoch on the test's clock now.
fn epoch_now(cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| crate::ticker::wall_now(cx))
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

/// SPEC.md: the prepared line shows "**Ready for new hosts and sessions** with age", and "age display can advance
/// without a new helm read", which takes the ticker's 30-second clock; the clock must not be held while the section is
/// not drawn (the ticker's rule). A ready snapshot runs no pulse, and the pushed catalog does not follow the clock, so
/// only the clock can move its age. Specified: with a ready prepared snapshot made now and Recipes shown, the line says
/// "prepared 0 s ago", the section holds the clock and no pulse; thirty seconds on it says "prepared 30 s ago" with
/// nothing sent; once Sessions shows, it holds neither.
#[gpui::test]
fn a_ready_snapshots_age_moves_on_the_clock_which_goes_when_hidden(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_recipes_with_places(cx, Vec::new(), Vec::new());
    let made = epoch_now(cx);
    let mut recipe = entry("rust-dev", idle(), Value::Null);
    if let Some(recipe) = recipe.as_object_mut() {
        recipe.insert(
            "prepared".into(),
            json!({ "id": "prepared-1", "prepared_at": made, "farhelm_version": "0.38.0",
                    "source_snapshot": "snap-1", "stale_reason": null }),
        );
    }
    push(&mut helm, vec![recipe]);
    settle(cx);
    assert!(
        words(cx).contains("prepared 0 s ago for Farhelm 0.38.0"),
        "{}",
        words(cx)
    );
    assert_eq!(holds_time(cx), (false, true), "the clock, and no pulse");

    advance(cx, std::time::Duration::from_secs(30));
    assert!(
        words(cx).contains("prepared 30 s ago for Farhelm 0.38.0"),
        "{}",
        words(cx)
    );
    assert_eq!(sent(&mut helm), [], "the age moved with nothing sent");

    click(cx, section_button(Section::Sessions));
    assert_eq!(holds_time(cx), (false, false), "neither once hidden");
}

/// A running preparation pulses like a build (SPEC.md: "a pulsing current phase and step count while preparing"), and
/// the pulse must go when the section is hidden, or the tick loop keeps waking for nothing. Specified: with one recipe
/// pushed preparing and Recipes shown, the line names its phase and step, the ticker ticks and the section holds the
/// pulse and the clock (the line shows the elapsed time); once Sessions shows, it holds neither and half a second
/// delivers no tick.
#[gpui::test]
fn a_running_preparation_pulses_until_the_section_is_hidden(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_recipes_with_places(cx, Vec::new(), Vec::new());
    let started = epoch_now(cx);
    let mut recipe = entry("rust-dev", idle(), Value::Null);
    if let Some(recipe) = recipe.as_object_mut() {
        recipe.insert(
            "preparation".into(),
            json!({ "state": "preparing", "step": "installing", "started_at": started }),
        );
    }
    push(&mut helm, vec![recipe]);
    settle(cx);
    assert!(
        words(cx)
            .contains("Preparing for new hosts and sessions: installing Farhelm (step 3 of 5)"),
        "{}",
        words(cx)
    );
    assert_eq!(holds_time(cx), (true, true), "the pulse and the clock");
    let before = ticks(cx);
    advance(cx, std::time::Duration::from_millis(500));
    assert!(ticks(cx) > before, "the preparation's pulse ticks");

    click(cx, section_button(Section::Sessions));
    assert_eq!(holds_time(cx), (false, false), "neither once hidden");
    let hidden = ticks(cx);
    advance(cx, std::time::Duration::from_millis(500));
    assert_eq!(ticks(cx), hidden, "nothing ticks for a section nobody sees");
}

/// A place's words follow its host's Podman check, and the old section asked for a check of every container host when
/// Settings opened; a section that never asked would say "this host needs a Podman check" for good. The check's
/// outcome reaches the place through the helm's push of the places, which it re-reads when the check ends, so the
/// section must neither read the places nor ask again when a host's check moves. Specified: with `build-box` able to
/// hold containers and never checked, showing Recipes asks `POST /api/hosts/7/podman-probe` once beside the account
/// read; showing it again in the same opening asks no second check; pushes of the hosts with the check running and
/// then finished send nothing; and the helm's push of the place with the finished check's reason draws that reason.
#[gpui::test]
fn container_hosts_are_checked_once_and_the_push_brings_the_checks_outcome(
    cx: &mut TestAppContext,
) {
    let with_check = |checked_at: Option<u64>, refreshing: bool| {
        let mut build_box = aliased(
            host(7, "ssh", connected()),
            "build-box",
            "dev@build-box.invalid",
        );
        build_box.container_check = Some(ContainerCheckView {
            checked_at,
            refreshing,
            usable: false,
            reason: checked_at.map(|_| "Podman is not installed".to_owned()),
        });
        hosts(&[build_box])
    };
    let needs_check = place(
        "rust-dev",
        Some(7),
        json!({ "reason": "this host needs a Podman check" }),
    );
    let (mut helm, cx, _gear) = open_rail_with(
        cx,
        &Preferences::default(),
        vec![
            with_check(None, false),
            catalog(vec![entry("rust-dev", idle(), Value::Null)]),
            places_state(vec![needs_check]),
        ],
    );
    open_settings(cx);
    click(cx, section_button(Section::Recipes));
    let (checks, reads): (Vec<FakeCommand>, Vec<FakeCommand>) = sent(&mut helm)
        .into_iter()
        .partition(|command| command.method == Method::Post);
    assert_eq!(
        checks
            .iter()
            .map(|check| check.path.as_str())
            .collect::<Vec<_>>(),
        ["/api/hosts/7/podman-probe"],
        "one check, of the one container host"
    );
    assert_eq!(
        reads
            .iter()
            .map(|read| read.path.as_str())
            .collect::<Vec<_>>(),
        ["/api/tensorlake"],
        "and only the account read"
    );
    for command in reads.into_iter().chain(checks) {
        let body = (command.method == Method::Get).then(|| account(true));
        helm.reply(command.id, if body.is_some() { 200 } else { 202 }, body);
    }
    settle(cx);

    click(cx, section_button(Section::Sessions));
    click(cx, section_button(Section::Recipes));
    answer_account(&mut helm, cx, true);

    let push_hosts = |helm: &mut FakeHelm, cx: &mut VisualTestContext, state: CollectionState| {
        helm.push(CollectionChanges {
            collection: state.collection,
            meta: state.meta,
            upsert: state.entries,
            remove: Vec::new(),
        });
        settle(cx);
    };
    // What the store holds as `build-box`'s check, to show each hosts push landed before nothing sent is believed.
    let stored_check = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            session::current_store(cx).and_then(|store| {
                store
                    .read(cx)
                    .collections()
                    .get::<HostView>(&HostView::key(7))
                    .and_then(|host| host.container_check.clone())
            })
        })
    };
    push_hosts(&mut helm, cx, with_check(None, true));
    assert!(
        stored_check(cx).is_some_and(|check| check.refreshing),
        "the premise: the push of the running check landed"
    );
    push_hosts(&mut helm, cx, with_check(Some(1_793_714_525), false));
    assert!(
        stored_check(cx).is_some_and(|check| check.checked_at.is_some() && !check.refreshing),
        "the premise: the push of the finished check landed"
    );
    assert_eq!(sent(&mut helm), [], "a check moving sends nothing");
    assert_eq!(
        label_of(cx, "recipe-rust-dev-place-7").as_deref(),
        Some("build-box · this host needs a Podman check"),
        "the premise: the place's words are the helm's until it pushes the place"
    );

    push_places(
        &mut helm,
        vec![place(
            "rust-dev",
            Some(7),
            json!({ "reason": "Podman is not installed" }),
        )],
    );
    settle(cx);
    assert_eq!(
        label_of(cx, "recipe-rust-dev-place-7").as_deref(),
        Some("build-box · Podman is not installed")
    );
    assert_eq!(sent(&mut helm), []);
}

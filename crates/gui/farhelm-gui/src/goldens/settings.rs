//! Golden scenes for the Settings dialog, one scene per section, a few states of its Recipes section, and Tensorlake
//! with a key saved and its list of sandboxes not on this helm. Agent trust has a second viewport for its last
//! block. The scene list itself, `SCENES` with each scene's name and size, stays in `mod.rs`.

use crate::focus::Mark;
use crate::{session, test_support};
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{Collection, CollectionState, KeyedEntry, PREFERENCES_KEY};
use gpui::{AnyElement, App, IntoElement as _, Window, div, point, px};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{Pending, SceneState, take_pending};

/// The Settings dialog over the fake helm's preference row, opened at `section` as the gear opens it (references
/// `settings-sessions`, `settings-appearance`, `settings-hosts`, `settings-recipes`, `settings-tensorlake`,
/// `settings-containers`, `settings-credentials`, `settings-terminal`, `settings-checkouts`, `settings-agent-trust`,
/// `settings-notifications` and `settings-app`): focus on the section's first switch (on the dialog itself at
/// Appearance, Recipes, Tensorlake, Containers, Credentials, Terminal, Checkouts, Agent trust and Notifications),
/// naming and automatic remote-host updates on because the row stores no choice for either. Agent trust's reads of
/// each agent's trust, sent as it opens, are answered at once with the trust off and the hint not dismissed,
/// Checkouts' reads as the reference stack had them: no default folder, no post-clone command, and a checkout kept
/// ready, Notifications' read of the ntfy settings with the helm's defaults
/// (off, `https://ntfy.sh`, no topic, Default priority, no token), Credentials' and Tensorlake's reads of the account
/// with no key saved and sandboxes created only after asking, Credentials' read with an empty catalog, Recipes' read of
/// the Tensorlake account with a key saved (so the rows offer their builds) over an empty pushed catalog and no pushed
/// places, and Containers' reads with the reference's three container hosts, all ready, and no leftovers; nothing else
/// is answered, so the checks Containers then asks for stay unanswered and the rows keep the first read's state. The
/// opener is the scene's ring handle, which stands in for the gear. The fake and the connection are kept for the
/// scene's life, like the Rename editor's.
fn settings_scene(
    state: &SceneState,
    section: crate::settings::Section,
    window: &mut Window,
    cx: &mut App,
) {
    settings_scene_with(state, section, SettingsFixture::default(), window, cx);
}

/// What a Settings scene's fake helm holds beyond [`settings_scene`]'s defaults.
#[derive(Default)]
struct SettingsFixture {
    /// The pushed recipe catalog's entries, as `GET /api/tensorlake/recipes` lists them; no recipes when empty.
    recipes: Vec<serde_json::Value>,
    /// The pushed recipe places' entries, as `GET /api/recipe-places` lists them; no places when empty.
    places: Vec<serde_json::Value>,
    /// The registered hosts, in the first snapshot's hosts collection (where Recipes' place names come from); the
    /// snapshot carries no hosts collection when empty.
    hosts: Vec<farhelm_proto::ui_channel::HostView>,
    /// The platform the Terminal section decides its tunnel switch for, instead of this machine's, through the
    /// dialog's test-only setter (`settings::decide_tunnels_for_opened`) applied as it opens.
    tunnels: Option<farhelm_input_mapping::Platform>,
    /// Open a new recipe's editor as the dialog opens (`settings::open_new_recipe_for_opened`), as New recipe does; the
    /// section must be Recipes.
    recipe_editor: bool,
    /// Answer the Tensorlake account read with a key saved whatever the section, as Recipes always does; Tensorlake
    /// then reads its list of sandboxes not on this helm, which [`settings_tensorlake_sandboxes`] answers.
    tensorlake_key: bool,
    /// The query entered after opening, for results and empty search scenes.
    search: Option<&'static str>,
}

/// `entries` as a pushed collection's first snapshot, each filed under the key `key` gives it, in their order.
fn pushed_state(
    collection: Collection,
    entries: Vec<serde_json::Value>,
    key: impl Fn(&serde_json::Value) -> String,
) -> CollectionState {
    let entries: Vec<KeyedEntry> = entries
        .into_iter()
        .map(|value| KeyedEntry {
            key: key(&value),
            value,
        })
        .collect();
    let order: Vec<&str> = entries.iter().map(|entry| entry.key.as_str()).collect();
    CollectionState {
        collection,
        meta: test_support::meta(&order),
        entries,
    }
}

/// [`settings_scene`], with the fake helm holding `fixture`.
fn settings_scene_with(
    state: &SceneState,
    section: crate::settings::Section,
    fixture: SettingsFixture,
    window: &mut Window,
    cx: &mut App,
) {
    let SettingsFixture {
        recipes,
        places,
        hosts,
        tunnels,
        recipe_editor,
        tensorlake_key,
        search,
    } = fixture;
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let mut snapshot = vec![
        CollectionState {
            collection: Collection::Preferences,
            meta: test_support::meta(&[PREFERENCES_KEY]),
            entries: vec![KeyedEntry {
                key: PREFERENCES_KEY.into(),
                value: serde_json::json!({}),
            }],
        },
        pushed_state(Collection::Recipes, recipes, |recipe| {
            recipe["name"].as_str().unwrap_or_default().to_owned()
        }),
        // A place that does not decode gets no key, and the fake helm's snapshot then fails the scene loudly.
        pushed_state(Collection::RecipePlaces, places, |place| {
            serde_json::from_value::<farhelm_proto::RecipePlaceView>(place.clone())
                .map(|view| farhelm_proto::RecipePlaceView::key(&view.recipe, view.place))
                .unwrap_or_default()
        }),
    ];
    if !hosts.is_empty() {
        snapshot.push(crate::hosts::tests::hosts(&hosts));
    }
    helm.connect(snapshot);
    // The dialog reads the row from the store, which the snapshot reaches after this setup returns: open once the
    // connection has a UI session, and only once.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    let opened = std::cell::Cell::new(false);
    let helm = Rc::new(RefCell::new(helm));
    let answering = helm.clone();
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::settings::open_at(section, &opener, &mark, window, cx);
            if recipe_editor {
                crate::settings::open_new_recipe_for_opened(window, cx);
            }
            if let Some(query) = search {
                crate::settings::search_for_opened(query, window, cx);
            }
        });
        if let Some(platform) = tunnels {
            crate::settings::decide_tunnels_for_opened(platform, cx);
        }
        let mut helm = answering.borrow_mut();
        for read in helm.commands() {
            let trust = read.path.starts_with("/api/checkout-claude-trust")
                || read.path.starts_with("/api/checkout-agent-trust/");
            let answer = if trust {
                serde_json::json!({ "enabled": false, "hint_dismissed": false })
            } else {
                match read.path.as_str() {
                    "/api/checkout-root" => serde_json::json!({ "root": null }),
                    "/api/checkout-post-clone" => serde_json::json!({ "command": null }),
                    "/api/checkout-spare" => serde_json::json!({ "enabled": true }),
                    "/api/credentials" => serde_json::json!([]),
                    "/api/container-orphans" => serde_json::json!([]),
                    // Credentials' and Tensorlake's key forms picture no key saved, as their references do; Recipes
                    // pictures rows that can build, which takes a saved key, and so does Tensorlake's list of
                    // sandboxes not on this helm.
                    "/api/tensorlake" => serde_json::json!({
                        "api_key_set": section == crate::settings::Section::Recipes || tensorlake_key,
                        "create_without_asking": false,
                        "helm_tag": "fixture",
                        "ssh_key": { "state": "not_created" },
                    }),
                    "/api/ntfy-alerts" => {
                        match serde_json::to_value(farhelm_proto::ntfy::Settings::default()) {
                            Ok(defaults) => defaults,
                            Err(_) => continue,
                        }
                    }
                    _ => continue,
                }
            };
            helm.reply(read.id, 200, Some(answer));
        }
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// Settings as the gear opens it, at Sessions.
pub(super) fn settings_sessions_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Sessions, window, cx);
}

/// Settings at Appearance, System chosen because the row stores no appearance. Focus starts on the dialog itself
/// (`Section::first`), so no ring shows, as in the reference, where the section was reached by a click.
pub(super) fn settings_appearance_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Appearance, window, cx);
}

/// Settings at Hosts.
pub(super) fn settings_hosts_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Hosts, window, cx);
}

/// Settings at Recipes with no recipes and New recipe under "No recipes yet.", as in the reference. Focus starts on the
/// dialog itself, since the section's controls wait for its reads.
pub(super) fn settings_recipes_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Recipes, window, cx);
}

/// Settings at Recipes with a new recipe's editor open, as New recipe opens it, against the reference
/// `settings-recipes`, whose set has no editor scene: the form from its top with focus on Back, Managed Ubuntu chosen
/// among the Software cards, and the editor's Cancel and Save pinned above the Close row, ruled off from it. The
/// dialog's body is shorter than the form, so the lower fields are below the fold, as a user first sees them.
pub(super) fn settings_recipe_editor_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene_with(
        state,
        crate::settings::Section::Recipes,
        SettingsFixture {
            recipe_editor: true,
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// Settings at Recipes with recipes in the states a row draws differently, against the reference `settings-recipes`,
/// whose stack had none: one built with both agents' versions and changed packages, so it warns, and rebuilding at its
/// fifth step, its line drawn at the pulse's full brightness and no action offered; one failed at its fourth step with
/// the build sandbox's reason and Retry; and one on a container image, which Tensorlake cannot run, so it warns and
/// offers nothing. Three rows, so the last one's warning fits above the footer unscrolled. The window is 70 pixels
/// taller than the other Settings scenes' for that: the Tensorlake lines start with the place's name, which wraps two
/// of them, and at 830 pixels the last warning fell behind the footer.
pub(super) fn settings_recipes_rows_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let recipe = |name: &str,
                  base: serde_json::Value,
                  build: serde_json::Value,
                  snapshot: serde_json::Value| {
        serde_json::json!({
            "name": name,
            "size": { "cpus": 2, "memory_gb": 4, "disk_gb": 20 },
            "base": base,
            "packages": ["jq", "ripgrep"],
            "claude": null, "codex": null,
            "claude_login_credential": null, "codex_login_credential": null, "github_token_credential": null,
            "tunnels": [{ "sandbox_port": 8080, "host": "127.0.0.1", "port": 8080 }],
            "snapshot": snapshot,
            "build": build,
            "prepared": null,
            "preparation": { "state": "idle" },
            "cleanup_pending": false,
        })
    };
    let ubuntu = serde_json::json!({ "kind": "tensorlake_ubuntu" });
    let idle = serde_json::json!({ "state": "idle" });
    let built = serde_json::json!({
        "id": "snap-1", "built_at": 1_793_714_525, "base": ubuntu, "disk_gb": 20, "packages": ["jq"],
        "claude_version": "2.0.1 (Claude Code)", "codex_version": "codex-cli 0.40.0",
    });
    let recipes = vec![
        recipe(
            "rust-dev",
            ubuntu.clone(),
            serde_json::json!({ "state": "building", "step": "packages", "started_at": 1_793_714_525 }),
            built,
        ),
        recipe(
            "team-image",
            serde_json::json!({ "kind": "image", "image": "team/devbox-base" }),
            serde_json::json!({ "state": "failed", "step": "connecting", "at": 1_793_714_525,
                "message": "the sandbox user cannot use sudo without a password" }),
            serde_json::Value::Null,
        ),
        recipe(
            "podman-dev",
            serde_json::json!({ "kind": "container_image", "reference": "example.test/team/dev:latest", "user": "dev" }),
            idle,
            serde_json::Value::Null,
        ),
    ];
    settings_scene_with(
        state,
        crate::settings::Section::Recipes,
        SettingsFixture {
            recipes,
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// Settings at Recipes with one recipe's per-place lines and a stale prepared snapshot, against the reference
/// `settings-recipes`, whose stack had no recipe: `rust-dev`, built, whose prepared snapshot the helm reports expired;
/// ready at `build-box` (a registered host, named from the hosts collection) with an older image awaiting cleanup; and
/// building at a host the registry no longer lists ("Unavailable place"), whose Rebuild is unavailable. The Tensorlake
/// account's place is pushed too and draws no line. No line shows an age, which the viewer's clock would move.
pub(super) fn settings_recipes_places_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let recipe = serde_json::json!({
        "name": "rust-dev",
        "size": { "cpus": 2, "memory_gb": 4, "disk_gb": 20 },
        "base": { "kind": "tensorlake_ubuntu" },
        "packages": ["jq"],
        "claude": null, "codex": null,
        "claude_login_credential": null, "codex_login_credential": null, "github_token_credential": null,
        "tunnels": [],
        "snapshot": {
            "id": "snap-1", "built_at": 1_793_714_525, "base": { "kind": "tensorlake_ubuntu" }, "disk_gb": 20,
            "packages": ["jq"], "claude_version": "2.0.1 (Claude Code)", "codex_version": "codex-cli 0.40.0",
        },
        "build": { "state": "idle" },
        "prepared": {
            "id": "prepared-1", "prepared_at": 1_793_714_525, "farhelm_version": "0.38.0",
            "source_snapshot": "snap-1", "stale_reason": "expired",
        },
        "preparation": { "state": "idle" },
        "cleanup_pending": false,
    });
    let place = |place: serde_json::Value, fields: serde_json::Value| {
        let mut entry = serde_json::json!({
            "recipe": "rust-dev", "place": place, "ready": false, "building": false, "reason": null,
            "built_at": null, "prepared_at": null, "step": null, "cleanup_pending": false,
        });
        if let (Some(entry), Some(fields)) = (entry.as_object_mut(), fields.as_object()) {
            entry.extend(fields.clone());
        }
        entry
    };
    let places = vec![
        place(
            serde_json::json!({ "kind": "tensorlake_account" }),
            serde_json::json!({ "ready": true }),
        ),
        place(
            serde_json::json!({ "kind": "podman_host", "host": 7 }),
            serde_json::json!({ "ready": true, "built_at": 1_793_714_525, "cleanup_pending": true }),
        ),
        place(
            serde_json::json!({ "kind": "podman_host", "host": 9 }),
            serde_json::json!({ "building": true, "step": "installing packages" }),
        ),
    ];
    use crate::hosts::tests::{aliased, connected, host};
    settings_scene_with(
        state,
        crate::settings::Section::Recipes,
        SettingsFixture {
            recipes: vec![recipe],
            places,
            hosts: vec![aliased(
                host(7, "ssh", connected()),
                "build-box",
                "dev@build-box.invalid",
            )],
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// Settings at Tensorlake, as in the reference: the experimental warning, the intro and the key form with no key saved,
/// and below the fold the create-without-asking switch off. Focus starts on the dialog itself, since every control
/// waits for the account read.
pub(super) fn settings_tensorlake_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Tensorlake, window, cx);
}

/// Whether [`settings_tensorlake_sandboxes`] has answered the list's read yet: kept across the scene's frames, since
/// the read goes out only once the account read's answer says a key is saved, after the opening's own answers.
struct SandboxListAnswered(std::cell::Cell<bool>);

/// The list's read, which [`settings_tensorlake_sandboxes`] finds by its path.
const SANDBOX_LIST_PATH: &str = "/api/tensorlake/orphans";

/// Settings at Tensorlake with a key saved, against the reference `settings-tensorlake`, whose stack had none: once the
/// list of sandboxes not on this helm has read, its rows as [`leftover_sandboxes`] lists them, with the body scrolled
/// just far enough to bring the list's heading to its top, so the heading, Refresh and the rows show. The read is
/// answered over the fake helm, so no Tensorlake account is reached; the rows' creation time reads in UTC, the zone a
/// golden's app has. The window records its frames for the scroll to measure the heading by.
pub(super) fn settings_tensorlake_sandboxes_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    settings_scene_with(
        state,
        crate::settings::Section::Tensorlake,
        SettingsFixture {
            tensorlake_key: true,
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
    window.set_driver_enabled(true);
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(SandboxListAnswered(std::cell::Cell::new(false))));
    keep.push(Box::new(super::Pending(std::cell::Cell::new(true))));
}

/// The list's answer: four leftovers in the states a row draws differently. One of this helm's, running, with its
/// creation time; one from another helm; one whose snapshot cleanup failed, with its reason and Retry cleanup; and one
/// whose cleanup is running, with Cleaning up….
fn leftover_sandboxes() -> serde_json::Value {
    let row = |id: &str, name: &str| {
        serde_json::json!({
            "id": id, "name": name, "created_at": null, "state": "running", "from_another_helm": false,
        })
    };
    let mut own = row("sb-1", "farhelm-host-fixture-old-gpubox");
    own["created_at"] = serde_json::json!(1_794_647_100_000_i64);
    let mut other = row("sb-2", "farhelm-host-zzzz-other-devbox");
    other["from_another_helm"] = serde_json::json!(true);
    let mut failed = row("sb-3", "farhelm-build-fixture-rust-dev");
    failed["cleanup"] =
        serde_json::json!({ "phase": "failed", "reason": "snapshot cleanup blocked" });
    let mut cleaning = row("sb-4", "farhelm-sess-fixture-run-k7q2");
    cleaning["cleanup"] = serde_json::json!({ "phase": "running" });
    cleaning["state"] = serde_json::json!("Cleanup in progress");
    serde_json::json!([own, other, failed, cleaning])
}

/// Draws nothing; on each frame, it moves [`settings_tensorlake_sandboxes_setup`]'s scene one step on. Until the list's
/// read has been answered, it answers every command that has reached the fake helm, in the order sent, as the fake
/// requires: the list's read, found by its path, with [`leftover_sandboxes`], and anything else as done with no body.
/// Once a frame has drawn after that, it scrolls the body, once, by how far that frame drew the list's heading below
/// the body's top (less a margin). A step a frame, since the read goes out only after the account read's answer
/// arrives, and the rows only after the list's.
pub(super) fn settings_tensorlake_sandboxes(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    use gpui::{IntoElement as _, point, px};
    let keep = state.keep.borrow();
    let helm = keep
        .iter()
        .find_map(|kept| kept.downcast_ref::<Rc<RefCell<FakeHelm>>>());
    let answered = keep
        .iter()
        .find_map(|kept| kept.downcast_ref::<SandboxListAnswered>());
    if let (Some(helm), Some(answered)) = (helm, answered) {
        if !answered.0.get() {
            let mut helm = helm.borrow_mut();
            for command in helm.commands() {
                let body = (command.path == SANDBOX_LIST_PATH).then(leftover_sandboxes);
                answered.0.set(answered.0.get() || body.is_some());
                helm.reply(command.id, 200, body);
            }
        } else if super::take_pending(state) {
            // After this frame is drawn, so the record holds where it put the heading.
            window.defer(cx, |window, cx| {
                let top = |id: &str| {
                    window
                        .driver_nodes()
                        .iter()
                        .find(|node| node.element_id.as_deref() == Some(id))
                        .map(|node| node.bounds.top())
                };
                let (Some(heading), Some(body)) =
                    (top("tensorlake-sandboxes-heading"), top("settings-body"))
                else {
                    return;
                };
                window.dispatch_event(
                    gpui::PlatformInput::Scroll(gpui::ScrollEvent {
                        // Inside the section's pane, right of the section list, whatever the header's height.
                        position: point(px(500.), px(415.)),
                        delta: gpui::ScrollDelta::Pixels(point(px(0.), body + px(8.) - heading)),
                        ..Default::default()
                    }),
                    cx,
                );
            });
        }
    }
    gpui::div().into_any_element()
}

/// Settings at Credentials, as in the reference: nothing saved in any list and no Tensorlake key saved. Focus starts
/// on the dialog itself, since every control waits for the section's reads.
pub(super) fn settings_credentials_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Credentials, window, cx);
}

/// Settings at Terminal, the default link choice chosen because the row stores none, and, decided for the Mac on every
/// machine, the tunnel switch below it off because the row stores no consent. Focus starts on the dialog itself
/// (`Section::first`), so no ring shows, as in the reference, where the section was reached by a click. The reference is
/// the browser interface, which had no tunnel switch; the old Mac app drew it under the link choices.
pub(super) fn settings_terminal_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene_with(
        state,
        crate::settings::Section::Terminal,
        SettingsFixture {
            tunnels: Some(farhelm_input_mapping::Platform::Mac),
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// Settings at Containers: the reference's three container hosts, each pushed with a passed Podman check (Ready), and
/// no containers left behind, as in the reference. Focus starts on the dialog itself (`Section::first`), so no ring
/// shows.
pub(super) fn settings_containers_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let mut hosts = crate::hosts::tests::reference_hosts();
    for host in &mut hosts {
        host.container_check = Some(farhelm_proto::ui_channel::ContainerCheckView {
            checked_at: Some(1_800_000_000),
            refreshing: false,
            usable: true,
            reason: None,
        });
    }
    let fixture = SettingsFixture {
        hosts,
        ..SettingsFixture::default()
    };
    settings_scene_with(
        state,
        crate::settings::Section::Containers,
        fixture,
        window,
        cx,
    );
}

/// Settings at Checkouts, its fields empty and the spare switch on, as in the reference. Focus starts on the dialog
/// itself (`Section::first`), so no ring shows.
pub(super) fn settings_checkouts_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Checkouts, window, cx);
}

/// Agent trust at the top of the section, the view paired with the old app's reference.
pub(super) fn settings_agent_trust_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::AgentTrust, window, cx);
}

/// Start at the same section and stage a scroll to show Antigravity's switch, caveats and hint below the fold.
pub(super) fn settings_agent_trust_end_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    settings_agent_trust_setup(state, window, cx);
    state
        .keep
        .borrow_mut()
        .push(Box::new(Pending(Cell::new(true))));
}

/// Wait for the dialog to have a drawn body before sending the wheel event it needs to receive.
pub(super) fn settings_agent_trust_end(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    if crate::layers::any_open(window, cx) && take_pending(state) {
        window.defer(cx, |window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::Scroll(gpui::ScrollEvent {
                    position: point(px(500.), px(415.)),
                    delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-4000.))),
                    ..Default::default()
                }),
                cx,
            );
        });
    }
    div().into_any_element()
}

/// Settings at Notifications, as in the reference: the Sounds sub-heading with its switches at their defaults (the row
/// stores no choice for them: waiting and approval on, turn finished off), and below it the ntfy alerts form over the
/// helm's defaults. Focus starts on the dialog itself (`Section::first`), so no ring shows.
pub(super) fn settings_notifications_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Notifications, window, cx);
}

/// Settings at App (reference `settings-app`), over a fake desktop updater kept as the app's
/// (`desktop::updates::fake`), so the section is listed after Notifications and shows the running version, its check
/// button, and both automatic updates and upgrade highlights on. Focus starts on the automatic-updates switch,
/// whose ring shows only under keyboard focus.
pub(super) fn settings_app_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let mut host = crate::whats_new::tests::test_host(Box::new(|_, _| {
        Box::pin(async { Ok(crate::desktop::updates::notes::UpgradeNotes::Current) })
    }));
    host.automatic_updates = Box::new(|| true);
    crate::desktop::updates::keep(Some(host), cx);
    settings_scene(state, crate::settings::Section::App, window, cx);
}

fn keep_updater(cx: &mut App) {
    use crate::desktop::updates::fake::{Feed, Seen, fake_host};
    let (feed, seen) = (Rc::new(Feed::default()), Rc::new(Seen::default()));
    seen.automatic.set(true);
    crate::desktop::updates::keep(Some(fake_host(&feed, &seen, false)), cx);
}

/// Match host and app updates in one result pane, with the first result selected.
pub(super) fn settings_search_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    keep_updater(cx);
    settings_scene_with(
        state,
        crate::settings::Section::Sessions,
        SettingsFixture {
            search: Some("update"),
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// The search field and result pane after a query that matches nothing.
pub(super) fn settings_search_empty_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    keep_updater(cx);
    settings_scene_with(
        state,
        crate::settings::Section::Sessions,
        SettingsFixture {
            search: Some("wallpaper"),
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

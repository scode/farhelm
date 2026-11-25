//! Golden scenes for the Settings dialog. The agent pages cover Claude's caution, Antigravity's directory-trust
//! controls and an agent with no controls. `SCENES` in `mod.rs` names each scene and sets its viewport.

use crate::focus::Mark;
use crate::settings::SavedRecipeQuestion;
use crate::{session, test_support};
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionState, KeyedEntry, PREFERENCES_KEY,
};
use gpui::{AnyElement, App, IntoElement as _, Window, div, point, px};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::SceneState;

/// The recipe editor's footer differs for an unsaved recipe, a saved recipe and an open delete question.
#[derive(Clone, Copy)]
enum RecipeEditorFixture {
    New,
    Saved {
        question: Option<SavedRecipeQuestion>,
    },
}

/// The Settings dialog over the fake helm's preference row, opened at `section` as the gear opens it (references
/// `settings-sessions`, `settings-appearance`, `settings-hosts`, `settings-recipes`, `settings-tensorlake`,
/// `settings-containers`, `settings-credentials`, `settings-terminal`, `settings-checkouts`, `settings-claude`,
/// `settings-notifications` and `settings-app`): focus on the section's first switch (on the dialog itself at
/// Appearance, Recipes, Tensorlake, Containers, Credentials, Terminal, Checkouts, agent pages and Notifications),
/// naming and automatic remote-host updates on because the row stores no choice for either. The displayed agent's
/// trust read is answered at once with the trust off and the hint shown,
/// Checkouts' reads as the reference stack had them: no default folder, no post-clone command, and a checkout kept
/// ready, Notifications' read of the ntfy settings with the helm's defaults
/// (off, `https://ntfy.sh`, no topic, Default priority, no token), Credentials' and Tensorlake's reads of the account
/// with no key saved and sandboxes created only after asking, Credentials' catalog with one GitHub token, Recipes' read of
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
    /// Open an editor in the Recipes section with the corresponding footer actions.
    recipe_editor: Option<RecipeEditorFixture>,
    /// Show the new subscription login editor and both agent choices.
    credential_editor: bool,
    /// Give Containers one leftover to confirm deleting. Its deletion is never sent.
    container_leftover: bool,
    /// Answer the Tensorlake account read with a key saved whatever the section, as Recipes always does; Tensorlake
    /// then reads its list of sandboxes not on this helm, which [`settings_tensorlake_sandboxes`] answers.
    tensorlake_key: bool,
    /// The query entered after opening, for results and empty search scenes.
    search: Option<&'static str>,
    /// An available catalog can be empty after deletion. Omitting it represents a snapshot without the collection.
    custom_agents: Option<CollectionState>,
    /// Stage the busy refusal once after the saved registration is open.
    custom_agent_save_refusal: bool,
    /// Fill the Add form before measuring the five dirty fields.
    custom_agent_dirty_add: bool,
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
        credential_editor,
        container_leftover,
        tensorlake_key,
        search,
        custom_agents,
        custom_agent_save_refusal,
        custom_agent_dirty_add,
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
    snapshot.extend(custom_agents);
    helm.connect(snapshot);
    // The dialog reads the row from the store, which the snapshot reaches after this setup returns: open once the
    // connection has a UI session, and only once.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    let helm = Rc::new(RefCell::new(helm));
    let answering = helm.clone();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let opened = section.clone();
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::settings::open_at(section.clone(), &opener, &mark, window, cx);
            match recipe_editor {
                Some(RecipeEditorFixture::New) => {
                    crate::settings::open_new_recipe_for_opened(window, cx);
                }
                Some(RecipeEditorFixture::Saved { question }) => {
                    crate::settings::open_saved_recipe_for_opened(question, window, cx);
                }
                None => {}
            }
            if credential_editor {
                crate::settings::open_agent_credential_for_opened(window, cx);
            }
            if custom_agent_save_refusal {
                crate::settings::stage_custom_agent_save_refusal(window, cx);
            }
            if custom_agent_dirty_add {
                crate::settings::stage_custom_agent_draft(window, cx);
            }
            if let Some(query) = search {
                crate::settings::search_for_opened(query, window, cx);
            }
        });
        if let Some(platform) = tunnels {
            crate::settings::decide_tunnels_for_opened(platform, cx);
        }
        answer_settings_reads(
            &mut answering.borrow_mut(),
            opened,
            tensorlake_key,
            container_leftover,
        );
    });
}

/// Scroll the saved registration to its two capabilities and command previews. The pinned heading keeps its ID.
pub(super) fn settings_custom_agent_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    open_custom_agent(state, false, window, cx);
    stage_scroll_end(state, false);
}

/// Draw the blank Add page that Settings shows after deleting its last custom agent.
/// The lifecycle test drives the delete itself and verifies that the old draft is absent.
pub(super) fn settings_custom_agent_empty_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    open_add_custom_agent(state, Availability::Available, window, cx);
}

/// On the first failed read, Add remains visible alongside the reason the existing registrations could not load.
pub(super) fn settings_custom_agent_catalog_unavailable_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    open_add_custom_agent(
        state,
        Availability::Unavailable {
            error: "The registration database is locked. Try again.".into(),
        },
        window,
        cx,
    );
}

/// Keep the Add scene's row count at zero whether the helm read an empty list or the first read failed.
fn open_add_custom_agent(
    state: &SceneState,
    availability: Availability,
    window: &mut Window,
    cx: &mut App,
) {
    let mut meta = test_support::meta(&[]);
    meta.availability = availability;
    settings_scene_with(
        state,
        crate::settings::Section::AddCustomAgent,
        SettingsFixture {
            custom_agents: Some(CollectionState {
                collection: Collection::CustomAgents,
                meta,
                entries: Vec::new(),
            }),
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// Exercise all five field markers together, including the ID that can only be edited on Add.
pub(super) fn settings_custom_agent_dirty_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    settings_scene_with(
        state,
        crate::settings::Section::AddCustomAgent,
        SettingsFixture {
            custom_agent_dirty_add: true,
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// A long saved name still shares one title row with the fixed scope.
pub(super) fn settings_custom_agent_long_name_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let mut agents = super::custom_agent_state();
    agents.entries[0].value["display_name"] =
        "Acme Code for Multi-Repository Release Engineering and Internal Tools".into();
    settings_scene_with(
        state,
        crate::settings::Section::CustomAgent("acme-code".into()),
        SettingsFixture {
            custom_agents: Some(agents),
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// Use the bundled-font spelling in screenshots. The Settings test checks the actual right-to-left text isolates.
pub(super) fn settings_custom_agent_nav_tooltip_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    open_custom_agent(state, false, window, cx);
    window.set_driver_enabled(true);
    state.hover.set(Some("settings-section-custom-acme-code"));
    state
        .keep
        .borrow_mut()
        .push(Box::new(super::Pending(Cell::new(true))));
}

/// Show the refusal where the user can retry the same draft.
pub(super) fn settings_custom_agent_save_refused_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    open_custom_agent(state, true, window, cx);
}

/// Use the same saved registration for the editor, sidebar and refusal scenes.
fn open_custom_agent(state: &SceneState, save_refusal: bool, window: &mut Window, cx: &mut App) {
    settings_scene_with(
        state,
        crate::settings::Section::CustomAgent("acme-code".into()),
        SettingsFixture {
            custom_agents: Some(super::custom_agent_state()),
            custom_agent_save_refusal: save_refusal,
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// Custom agents are below the built-in sections. Scroll the sidebar before the shared hover step rests on the row.
pub(super) fn settings_custom_agent_nav_tooltip(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    if let Some(nav) = window.driver_nodes().iter().find(|node| {
        node.element_id.as_deref() == Some("settings-sections") && !node.visible_bounds.is_empty()
    }) && super::take_pending(state)
    {
        let position = nav.visible_bounds.center();
        window.defer(cx, move |window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::Scroll(gpui::ScrollEvent {
                    position,
                    delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
                    ..Default::default()
                }),
                cx,
            );
        });
    }
    div().into_any_element()
}

/// A later section in the same opening receives the same fake configuration as if it were the first.
fn answer_settings_reads(
    helm: &mut FakeHelm,
    section: crate::settings::Section,
    tensorlake_key: bool,
    container_leftover: bool,
) {
    for read in helm.commands() {
        let trust = read.path.starts_with("/api/checkout-claude-trust")
            || read.path.starts_with("/api/checkout-agent-trust/");
        let answer = if trust {
            serde_json::json!({ "enabled": false, "hint_dismissed": false })
        } else {
            match read.path.as_str() {
                "/api/agent-hook-setup/grok" | "/api/agent-hook-setup/antigravity" => {
                    serde_json::json!({ "enabled": false })
                }
                "/api/checkout-root" => serde_json::json!({ "root": null }),
                "/api/checkout-post-clone" => serde_json::json!({ "command": null }),
                "/api/checkout-spare" => serde_json::json!({ "enabled": true }),
                "/api/credentials" if section == crate::settings::Section::Credentials => {
                    serde_json::json!([{
                        "name": "team-github",
                        "kind": "github_token",
                        "created_at": 1_793_714_525_i64,
                        "updated_at": 1_793_714_525_i64,
                        "revision": 1,
                        "used_by": ["rust-dev"],
                    }])
                }
                "/api/credentials" => serde_json::json!([]),
                "/api/container-orphans" if container_leftover => serde_json::json!([{
                    "id": "a".repeat(64),
                    "name": "build-check",
                    "place": { "kind": "podman_host", "host": 2 },
                    "role": "session",
                    "host": null,
                }]),
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

/// Stage an edited name so the golden shows the draft note beside the recipe editor's pinned answers.
pub(super) fn settings_recipe_editor_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    recipe_editor_scene(state, RecipeEditorFixture::New, window, cx);
    stage_draft(state, crate::settings::Section::Recipes, DraftStep::Edit);
}

fn recipe_editor_scene(
    state: &SceneState,
    editor: RecipeEditorFixture,
    window: &mut Window,
    cx: &mut App,
) {
    settings_scene_with(
        state,
        crate::settings::Section::Recipes,
        SettingsFixture {
            recipe_editor: Some(editor),
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// Keep the editor's final groups and its footer in the same picture.
pub(super) fn settings_recipe_editor_end_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    recipe_editor_scene(
        state,
        RecipeEditorFixture::Saved { question: None },
        window,
        cx,
    );
    stage_scroll_end(state, true);
}

/// Show the Software choices and the following groups between the recipe editor's top and end views.
pub(super) fn settings_recipe_editor_middle_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    recipe_editor_scene(state, RecipeEditorFixture::New, window, cx);
    stage_scroll(state, true, 0.46);
}

/// A saved recipe's inline delete question replaces the ordinary footer controls before any deletion is sent.
pub(super) fn settings_recipe_editor_delete_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    recipe_editor_scene(
        state,
        RecipeEditorFixture::Saved {
            question: Some(SavedRecipeQuestion::Delete),
        },
        window,
        cx,
    );
}

/// The second Delete question shows the risk of forgetting a capture in the
/// same footer. Begin with keyboard input to make Cancel's default focus
/// visible in the capture.
pub(super) fn settings_recipe_editor_delete_anyway_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    window.activate_window();
    super::press_tab(window, cx);
    recipe_editor_scene(
        state,
        RecipeEditorFixture::Saved {
            question: Some(SavedRecipeQuestion::DeleteAnyway),
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
    tensorlake_with_sandboxes(state, window, cx);
    state
        .keep
        .borrow_mut()
        .push(Box::new(super::Pending(Cell::new(true))));
}

/// The middle viewport covers the API key actions and SSH key between the top and the leftovers.
pub(super) fn settings_tensorlake_keys_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    tensorlake_with_sandboxes(state, window, cx);
    stage_scroll(state, false, 0.34);
}

/// Keep the Creating sandboxes card and its Usage Credits note visible below the leftovers.
pub(super) fn settings_tensorlake_end_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    tensorlake_with_sandboxes(state, window, cx);
    stage_scroll_end(state, false);
}

fn tensorlake_with_sandboxes(state: &SceneState, window: &mut Window, cx: &mut App) {
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
    state
        .keep
        .borrow_mut()
        .push(Box::new(SandboxListAnswered(Cell::new(false))));
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
    serde_json::json!({
        "discovery": { "status": "ok", "sandboxes": [own, other] },
        "cleanups": [failed, cleaning],
    })
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
        } else if keep.iter().any(|item| item.is::<ScrollEnd>()) {
            // Measure only when the rendered viewport contains the list's rows, which changed its scroll range.
            if window
                .driver_nodes()
                .iter()
                .any(|node| node.element_id.as_deref() == Some("tensorlake-sandbox-0-delete"))
            {
                drop(keep);
                return settings_scroll_end(state, window, cx);
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

/// One saved GitHub token exercises its Edit row. The other credential lists are empty and no Tensorlake key is set.
/// Focus starts on the dialog itself until the section's reads settle.
pub(super) fn settings_credentials_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Credentials, window, cx);
}

/// Picture the agent login editor with both agent marks in the segmented choice.
pub(super) fn settings_credential_editor_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    settings_scene_with(
        state,
        crate::settings::Section::Credentials,
        SettingsFixture {
            credential_editor: true,
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
    stage_draft(
        state,
        crate::settings::Section::Credentials,
        DraftStep::Edit,
    );
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
    containers_scene(state, false, window, cx);
}

fn containers_scene(
    state: &SceneState,
    container_leftover: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let mut hosts = crate::hosts::tests::reference_hosts();
    use farhelm_proto::host_appearance::HostColor;
    for (host, color) in
        hosts
            .iter_mut()
            .zip([HostColor::Copper, HostColor::Teal, HostColor::Orchid])
    {
        host.color = color;
        host.container_check = Some(farhelm_proto::ui_channel::ContainerCheckView {
            checked_at: Some(1_800_000_000),
            refreshing: false,
            usable: true,
            reason: None,
        });
    }
    let fixture = SettingsFixture {
        hosts,
        container_leftover,
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

/// Wait for the fake leftover read before showing its row, a later refresh error and the inline Delete question.
struct ContainerQuestion(Rc<Cell<bool>>);

pub(super) fn settings_containers_question_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    containers_scene(state, true, window, cx);
    state
        .keep
        .borrow_mut()
        .push(Box::new(ContainerQuestion(Rc::new(Cell::new(false)))));
}

pub(super) fn settings_containers_question(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let keep = state.keep.borrow();
    if let Some(ContainerQuestion(asked)) = keep
        .iter()
        .find_map(|item| item.downcast_ref::<ContainerQuestion>())
        && !asked.get()
    {
        let asked = asked.clone();
        window.defer(cx, move |window, cx| {
            asked.set(crate::settings::ask_first_container_for_opened(window, cx));
        });
    }
    div().into_any_element()
}

/// Settings at Checkouts, its fields empty and the spare switch on, as in the reference. Focus starts on the dialog
/// itself (`Section::first`), so no ring shows.
pub(super) fn settings_checkouts_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Checkouts, window, cx);
}

/// At the application's minimum width and height, the page end and footer still have room.
pub(super) fn settings_checkouts_capped_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    settings_checkouts_edited_setup(state, window, cx);
    stage_scroll_end(state, false);
}

/// Show Save and Discard after the edited field has scrolled away at the minimum supported window size.
pub(super) fn settings_checkouts_capped(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    settings_edited(state, window, cx);
    settings_scroll_end(state, window, cx)
}

/// Claude's trust switch, its caution and its hint switch belong to the same page.
pub(super) fn settings_claude_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    use farhelm_proto::LaunchHarness;
    settings_scene(
        state,
        crate::settings::Section::Agent(LaunchHarness::Claude),
        window,
        cx,
    );
}

/// Codex's trust and hint switches show how the opt-in applies to any folder.
pub(super) fn settings_codex_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(
        state,
        crate::settings::Section::Agent(farhelm_proto::LaunchHarness::Codex),
        window,
        cx,
    );
}

/// Muse's page explains its per-launch flag and its own hint switch.
pub(super) fn settings_muse_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(
        state,
        crate::settings::Section::Agent(farhelm_proto::LaunchHarness::Muse),
        window,
        cx,
    );
}

/// Pi's setting approves project resources in any folder and has no hint switch.
pub(super) fn settings_pi_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(
        state,
        crate::settings::Section::Agent(farhelm_proto::LaunchHarness::Pi),
        window,
        cx,
    );
}

/// Antigravity's trust applies to ordinary directories, which its help and hint switch explain.
pub(super) fn settings_antigravity_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    use farhelm_proto::LaunchHarness;
    settings_scene(
        state,
        crate::settings::Section::Agent(LaunchHarness::Antigravity),
        window,
        cx,
    );
}

/// Grok's page shows which files the all-hosts switch lets Farhelm write and how to undo those writes.
pub(super) fn settings_grok_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(
        state,
        crate::settings::Section::Agent(farhelm_proto::LaunchHarness::Grok),
        window,
        cx,
    );
}

/// An agent with no settings still has a page so someone looking for it can find that answer.
pub(super) fn settings_cursor_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    use farhelm_proto::LaunchHarness;
    settings_scene(
        state,
        crate::settings::Section::Agent(LaunchHarness::Cursor),
        window,
        cx,
    );
}

/// A pending focus reveal can move the viewport back to the top. Finish only after the laid-out body reaches its end.
struct ScrollEnd {
    wait_for_recipe_back: bool,
    fraction: f32,
    done: Cell<bool>,
}

fn stage_scroll_end(state: &SceneState, wait_for_recipe_back: bool) {
    stage_scroll(state, wait_for_recipe_back, 1.);
}

/// A scroll target is relative to the laid-out content so the middle and end views survive spacing changes.
fn stage_scroll(state: &SceneState, wait_for_recipe_back: bool, fraction: f32) {
    state.keep.borrow_mut().push(Box::new(ScrollEnd {
        wait_for_recipe_back,
        fraction,
        done: Cell::new(false),
    }));
}

/// A scene only settles once its reported offset reaches the requested part of the measured viewport.
pub(super) fn settings_scroll_end(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let keep = state.keep.borrow();
    if let Some(end) = keep
        .iter()
        .find_map(|item| item.downcast_ref::<ScrollEnd>())
        && !end.done.get()
        && let Some(scroll) =
            crate::settings::page_scroll_for_opened(end.wait_for_recipe_back, window, cx)
        && scroll.max_offset().y > px(0.)
    {
        let target = -scroll.max_offset().y * end.fraction;
        if (scroll.offset().y - target).abs() <= px(1.) {
            end.done.set(true);
        } else {
            window.defer(cx, move |window, _| {
                scroll.set_offset(point(px(0.), target));
                window.refresh();
            });
        }
    }
    div().into_any_element()
}

/// Settings at Notifications, as in the reference: the Sounds sub-heading with its switches at their defaults (the row
/// stores no choice for them: waiting and approval on, turn finished off), and below it the ntfy alerts form over the
/// helm's defaults. Focus starts on the dialog itself (`Section::first`), so no ring shows.
pub(super) fn settings_notifications_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Notifications, window, cx);
}

/// Edited screens use the real form subscription after the initial reads settle, so the footer and field dots
/// come from the same draft state as an edit typed in the app.
struct DraftScene {
    section: crate::settings::Section,
    step: Cell<DraftStep>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DraftStep {
    Edit,
    VerifyDraft,
    OpenNotifications,
    EditNotifications,
    AskToClose,
    Done,
}

fn stage_draft(state: &SceneState, section: crate::settings::Section, first: DraftStep) {
    state.keep.borrow_mut().push(Box::new(DraftScene {
        section,
        step: Cell::new(first),
    }));
}

/// A stable frame can precede the second form's read. Do not finish a draft screenshot during that gap.
pub(super) fn settings_scene_pending(state: &SceneState) -> bool {
    state.keep.borrow().iter().any(|item| {
        item.downcast_ref::<DraftScene>()
            .is_some_and(|draft| draft.step.get() != DraftStep::Done)
            || item
                .downcast_ref::<ContainerQuestion>()
                .is_some_and(|question| !question.0.get())
            || item
                .downcast_ref::<ScrollEnd>()
                .is_some_and(|end| !end.done.get())
    })
}

pub(super) fn settings_terminal_edited_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    use crate::settings::Section;
    settings_terminal_setup(state, window, cx);
    stage_draft(state, Section::Terminal, DraftStep::Edit);
}

pub(super) fn settings_notifications_edited_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    use crate::settings::Section;
    settings_notifications_setup(state, window, cx);
    stage_draft(state, Section::Notifications, DraftStep::Edit);
}

pub(super) fn settings_checkouts_edited_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    use crate::settings::Section;
    settings_checkouts_setup(state, window, cx);
    stage_draft(state, Section::Checkouts, DraftStep::Edit);
}

pub(super) fn settings_unsaved_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    use crate::settings::Section;
    settings_checkouts_setup(state, window, cx);
    stage_draft(state, Section::Checkouts, DraftStep::OpenNotifications);
}

/// Advance the edited scene once per frame. The close question follows two actual draft changes in one dialog.
pub(super) fn settings_edited(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    use crate::settings::{self, Section};

    let keep = state.keep.borrow();
    // The renderer draws once before setup so an anchored scene can measure its opener.
    let Some(draft) = keep
        .iter()
        .find_map(|item| item.downcast_ref::<DraftScene>())
    else {
        return div().into_any_element();
    };
    let step = draft.step.get();
    let section = match step {
        DraftStep::Edit | DraftStep::OpenNotifications => draft.section.clone(),
        DraftStep::EditNotifications => Section::Notifications,
        DraftStep::VerifyDraft => {
            if settings::draft_changed_for_opened(draft.section.clone(), cx) {
                draft.step.set(DraftStep::Done);
            }
            return div().into_any_element();
        }
        DraftStep::AskToClose => {
            if settings::draft_changed_for_opened(Section::Checkouts, cx)
                && settings::draft_changed_for_opened(Section::Notifications, cx)
            {
                draft.step.set(DraftStep::Done);
                window.defer(cx, |window, cx| {
                    let settings = crate::layers::top_container(window, cx);
                    assert!(settings.is_some(), "Settings must be open with both drafts");
                    assert!(crate::layers::dismiss_top(window, cx));
                    let question = crate::layers::top_container(window, cx);
                    assert!(
                        question.is_some() && question != settings,
                        "the close question opened"
                    );
                });
            }
            return div().into_any_element();
        }
        DraftStep::Done => return div().into_any_element(),
    };
    if step == DraftStep::EditNotifications {
        let helm = keep
            .iter()
            .find_map(|item| item.downcast_ref::<Rc<RefCell<FakeHelm>>>())
            .expect("the second page reads from the scene's helm");
        answer_settings_reads(&mut helm.borrow_mut(), section.clone(), false, false);
    }
    if let Some(input) = settings::draft_input_for_opened(section.clone(), cx) {
        let text = match section {
            Section::Terminal => "md, txt, json, png, jpg, html",
            Section::Checkouts => "~/work/managed",
            Section::Notifications => "farhelm-lab",
            Section::Recipes => "rust-tools",
            Section::Credentials => "preview-only-token",
            _ => unreachable!("the scene only edits its staged fields"),
        };
        let next = match step {
            DraftStep::OpenNotifications => DraftStep::EditNotifications,
            DraftStep::EditNotifications => DraftStep::AskToClose,
            DraftStep::Edit => DraftStep::VerifyDraft,
            DraftStep::VerifyDraft | DraftStep::AskToClose | DraftStep::Done => {
                unreachable!("these steps do not edit a field")
            }
        };
        draft.step.set(next);
        window.defer(cx, move |window, cx| {
            input.edit_for_golden(text, window, cx);
            if step == DraftStep::OpenNotifications {
                settings::show_for_opened(Section::Notifications, window, cx);
            } else if matches!(section, Section::Terminal | Section::Notifications) {
                // Keep the edited field, its explanatory copy, and the pinned actions in view together.
                window.dispatch_event(
                    gpui::PlatformInput::Scroll(gpui::ScrollEvent {
                        position: point(px(500.), px(415.)),
                        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
                        ..Default::default()
                    }),
                    cx,
                );
            }
        });
    }
    div().into_any_element()
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

/// Find the registered agent and the Add form without loading either page.
pub(super) fn settings_search_custom_agents_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let mut agents = super::custom_agent_state();
    agents.entries[0].value["display_name"] = "Acme Custom Code".into();
    settings_scene_with(
        state,
        crate::settings::Section::Sessions,
        SettingsFixture {
            search: Some("custom"),
            custom_agents: Some(agents),
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

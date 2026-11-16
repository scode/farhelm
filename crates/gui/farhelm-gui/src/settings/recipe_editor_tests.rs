//! Tests of the recipe editor in Settings → Recipes over the helm client's fake: New recipe and Edit opening it in place
//! of the list, Back, the base cards and their fields, tunnel rows, local refusals, Save as a create or an update under
//! the recipe's key with the credential choices it loaded, a refused save, and the answers Settings pins while Recipes
//! shows the editor.
//!
//! The dialog is opened from the gear as in the dialog's own tests (`super::tests`), whose helpers these share, over a
//! helm that pushes the catalog as the section's own tests have it (`super::recipe_tests`). Every step ends in one
//! of the shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::Preferences;
use gpui::{Entity, TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::Refused;
use super::Section;
use super::recipe_place_tests::places_state;
use super::recipe_tests::{answer_account, catalog, entry, idle, push};
use super::recipes::Editor;
use super::recipes::draft::Draft;
use super::tests::{
    Opened, available, drawn, fill, label_of, open_rail_with, open_settings, painted, reach,
    section_button, sent, switch_on,
};
use crate::operations::{OpKey, Work};
use crate::session;
use crate::test_support::{bounds, click, focused_id, keys, settle};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The gear's rail over a helm whose first snapshot pushes `recipes` as the catalog and no places, with Settings open
/// at Recipes and its one read, the account's, answered with a saved API key.
pub(super) fn show(
    cx: &mut TestAppContext,
    recipes: Vec<Value>,
) -> (FakeHelm, &mut VisualTestContext) {
    let (mut helm, cx, _gear) = open_rail_with(
        cx,
        &Preferences::default(),
        vec![catalog(recipes), places_state(Vec::new())],
    );
    open_settings(cx);
    click(cx, section_button(Section::Recipes));
    answer_account(&mut helm, cx, true);
    (helm, cx)
}

/// The one command sent since the last call.
pub(super) fn one_command(helm: &mut FakeHelm) -> FakeCommand {
    let mut sent = sent(helm);
    assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
    sent.remove(0)
}

/// Press `id` (New recipe or a row's Edit) and answer the one read the editor sends as it opens, the credential
/// catalog's (`GET /api/credentials`), with nothing saved.
fn open_editor(helm: &mut FakeHelm, cx: &mut VisualTestContext, id: &'static str) {
    open_editor_with(helm, cx, id, json!([]));
}

/// [`open_editor`], with `catalog` as the credential catalog's answer.
pub(super) fn open_editor_with(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    id: &'static str,
    catalog: Value,
) {
    click(cx, id);
    let read = one_command(helm);
    assert_eq!(
        (read.method, read.path.as_str()),
        (Method::Get, "/api/credentials"),
        "the editor reads the credential catalog as it opens"
    );
    helm.reply(read.id, 200, Some(catalog));
    settle(cx);
}

/// The open recipe editor.
pub(super) fn editor(cx: &mut VisualTestContext) -> Entity<Editor> {
    cx.update(|_, cx| {
        let dialog = cx.global::<Opened>().0.upgrade().expect("Settings is open");
        let recipes = dialog.read(cx).recipes.clone().expect("Recipes was shown");
        recipes.read(cx).editor().expect("an editor is open")
    })
}

/// The open editor's draft, as Save would read it.
pub(super) fn draft(cx: &mut VisualTestContext) -> Draft {
    let editor = editor(cx);
    cx.update(|_, cx| editor.read(cx).draft(cx))
}

/// Type `text` into the editor's field `id` (its element id) the way a keyboard user does: Tab until the field holds
/// focus, which scrolls it into the dialog's view, select what it holds, and type. The form is taller than the dialog's
/// body, so a click cannot reach its lower fields, and GPUI Kit's fields carry no element id of their own for
/// [`reach`] to find, so the editor's field is asked whether it holds focus.
pub(super) fn type_into(cx: &mut VisualTestContext, id: &str, text: &str) {
    let editor = editor(cx);
    for _ in 0..40 {
        let focused = cx.update(|window, cx| {
            editor
                .read(cx)
                .field(id)
                .is_some_and(|field| field.focus_handle(cx).is_focused(window))
        });
        if focused {
            keys(cx, super::tests::SELECT_ALL);
            cx.simulate_input(text);
            crate::test_support::check(cx);
            return;
        }
        keys(cx, "tab");
    }
    panic!("Tab never reached the field {id}");
}

/// Choose the Software card `id` the way a keyboard user does: Tab to it and Space. The second row of cards can lie
/// below the dialog's view.
pub(super) fn choose(cx: &mut VisualTestContext, id: &str) {
    reach(cx, id);
    keys(cx, "space");
}

/// The work in flight on the recipe `name`, and the refusal its key records.
fn recipe_key(cx: &mut VisualTestContext, name: &str) -> (Option<Work>, Option<String>) {
    cx.update(|_, cx| {
        session::current_operations(cx)
            .map(|operations| {
                let operations = operations.read(cx);
                let key = OpKey::recipe(name);
                (
                    operations.in_flight(&key),
                    operations
                        .failure(&key)
                        .map(|failure| failure.message.raw().to_owned()),
                )
            })
            .unwrap_or_default()
    })
}

/// A saved recipe as the catalog lists it, with a Claude Code endpoint naming a token, a Codex subscription login, a
/// GitHub token and one tunnel: a credential choice in every place the draft holds one, so a save shows whether the
/// editor loaded and kept each of them.
pub(super) fn with_credentials(name: &str) -> Value {
    json!({
        "name": name,
        "size": { "cpus": 2, "memory_gb": 4, "disk_gb": 20 },
        "base": { "kind": "tensorlake_ubuntu" },
        "packages": ["git", "jq"],
        "claude": { "base_url": "http://127.0.0.1:8080", "token_credential": "work-endpoint" },
        "codex": null,
        "claude_login_credential": null,
        "codex_login_credential": "codex-work",
        "github_token_credential": "ci-token",
        "tunnels": [{ "sandbox_port": 8080, "host": "127.0.0.1", "port": 8080 }],
        "snapshot": null,
        "build": { "state": "idle" },
        "prepared": null,
        "preparation": { "state": "idle" },
        "cleanup_pending": false,
    })
}

/// An idle `rust-dev` with nothing built, the catalog most tests start from.
fn one_recipe() -> Vec<Value> {
    vec![entry("rust-dev", idle(), Value::Null)]
}

// ---- Opening and leaving -------------------------------------------------------------------------------------------

/// SPEC.md: "The editor replaces the list in place", it "opens at its top, including Back", and its answers "stay
/// visible at the bottom of the scrolling panel, above the unchanged Close footer". The answers are pinned in the
/// dialog's footer region, not in the scrolling body, or a long form would push Save out of view. Specified: the list
/// offers New recipe and an Edit per row; New recipe replaces the list with the editor titled "New recipe", focus on
/// Back; opening sends the credential catalog's read and nothing else; Cancel and Save are drawn inside the dialog's
/// pinned notices, with the Close row still drawn; Back with a typed name, then Discard in the question it asks,
/// sends nothing (no save, and no read of the recipes: they are pushed), closes the editor, shows the list and puts
/// focus on New recipe.
#[gpui::test]
fn new_recipe_opens_the_editor_in_place_and_back_returns_to_the_list(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, one_recipe());
    assert!(drawn(cx, "recipe-new"));
    assert_eq!(
        label_of(cx, "recipe-edit-rust-dev").as_deref(),
        Some("Edit rust-dev")
    );

    open_editor(&mut helm, cx, "recipe-new");
    assert!(drawn(cx, "recipe-editor"));
    assert!(!drawn(cx, "recipes-list"), "the editor replaces the list");
    assert_eq!(
        label_of(cx, "recipe-editor-title").as_deref(),
        Some("New recipe")
    );
    assert_eq!(focused_id(cx).as_deref(), Some("recipe-back"));
    let (notices, save, cancel) = (
        bounds(cx, "settings-notices"),
        bounds(cx, "recipe-save"),
        bounds(cx, "recipe-cancel"),
    );
    assert!(
        notices.contains(&save.center()) && notices.contains(&cancel.center()),
        "Cancel and Save are pinned in the dialog's notices: {notices:?} {save:?} {cancel:?}"
    );
    assert!(drawn(cx, "settings-footer-close"), "the Close row stays");
    assert_eq!(
        sent(&mut helm),
        [],
        "opening sends nothing beyond the credentials read"
    );

    fill(cx, "recipe-name", "scratch");
    click(cx, "recipe-back");
    click(cx, "prompt-alternative-0");
    assert_eq!(sent(&mut helm), [], "closing sends nothing");
    assert!(!drawn(cx, "recipe-editor"));
    assert!(drawn(cx, "recipes-list"));
    assert!(!drawn(cx, "recipe-save"), "the answers go with the editor");
    assert_eq!(focused_id(cx).as_deref(), Some("recipe-new"));
}

/// A new recipe's name is its identity for good, so Save must create it under that name and that name's key, never
/// overwrite (the helm refuses an existing name); and a done save must show the new row, which the helm's push of the
/// catalog brings before its answer, with nothing read. Specified: New recipe, a name and Save send
/// `POST /api/tensorlake/recipes` with the name and the new recipe's defaults (2 CPUs, 4 GB, 20 GB, managed Ubuntu, no
/// packages, credentials or tunnels), as save work on the name's key; meanwhile Save reads "Saving…" and takes no press
/// and Back and Cancel are unavailable; the helm's push of the new recipe and then its done answer close the editor,
/// show the new row, send nothing and put focus on New recipe.
#[gpui::test]
fn a_new_recipe_is_created_under_its_key_and_its_pushed_row_shows(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, Vec::new());
    open_editor(&mut helm, cx, "recipe-new");
    fill(cx, "recipe-name", "rust-dev");
    click(cx, "recipe-save");
    let save = one_command(&mut helm);
    assert_eq!(
        (save.method, save.path.as_str()),
        (Method::Post, "/api/tensorlake/recipes")
    );
    assert_eq!(
        save.body,
        Some(json!({ "name": "rust-dev", "recipe": {
            "size": { "cpus": 2, "memory_gb": 4, "disk_gb": 20 },
            "base": { "kind": "tensorlake_ubuntu" },
            "packages": [],
            "claude": null, "codex": null,
            "claude_login_credential": null, "codex_login_credential": null, "github_token_credential": null,
            "tunnels": [],
        }}))
    );
    assert_eq!(recipe_key(cx, "rust-dev").0, Some(Work::Save));
    assert!(painted(cx).contains("Saving…"));
    assert_eq!(
        super::recipe_tests::pressable(cx, "recipe-save"),
        Some(false)
    );
    assert_eq!(available(cx, "recipe-back"), Some(false));
    assert_eq!(available(cx, "recipe-cancel"), Some(false));

    // The helm pushes what a command changed before its reply.
    push(&mut helm, one_recipe());
    helm.reply(
        save.id,
        201,
        Some(entry("rust-dev", json!({ "state": "idle" }), Value::Null)),
    );
    settle(cx);
    assert_eq!(sent(&mut helm), [], "nothing is read after the save");
    assert!(!drawn(cx, "recipe-editor"), "a done save closes the editor");
    assert!(drawn(cx, "recipe-rust-dev"), "the new row shows");
    assert_eq!(focused_id(cx).as_deref(), Some("recipe-new"));
}

/// The old editor's `readonly: locked`: while the save is out, nothing may change the draft being sent. An edit then
/// would also hide the helm's refusal the moment it arrived, since a refusal shows only while the draft is still the
/// one it answered. Specified: with a new recipe's save out (Claude on API endpoint with a base URL), typing into CPUs
/// and into Claude's base URL leaves the draft's values as sent, and every Software card, Add tunnel, the GitHub picker
/// and Claude's mode choice are unavailable; after the helm refuses the save, CPUs takes an edit again and the cards
/// are available.
#[gpui::test]
fn while_a_save_is_out_the_form_takes_no_edit(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, Vec::new());
    open_editor(&mut helm, cx, "recipe-new");
    fill(cx, "recipe-name", "rust-dev");
    reach(cx, "recipe-claude-mode-None");
    keys(cx, "right");
    type_into(cx, "recipe-claude-url", "http://127.0.0.1:8080");
    click(cx, "recipe-save");
    let save = one_command(&mut helm);
    assert_eq!(
        recipe_key(cx, "rust-dev").0,
        Some(Work::Save),
        "the premise: the save is out"
    );

    fill(cx, "recipe-cpus", "3");
    assert_eq!(draft(cx).cpus, "2", "CPUs takes no edit");
    type_into(cx, "recipe-claude-url", "http://127.0.0.1:9");
    assert_eq!(
        draft(cx).claude.base_url,
        "http://127.0.0.1:8080",
        "the base URL takes no edit"
    );
    for card in [
        "recipe-base-Managed Ubuntu",
        "recipe-base-Your image",
        "recipe-base-Your snapshot",
        "recipe-base-Container image",
        "recipe-add-tunnel",
        "recipe-github",
        "recipe-claude-mode-None",
    ] {
        assert_eq!(available(cx, card), Some(false), "{card} is unavailable");
    }

    helm.reply(save.id, 400, Some(json!("the helm refused it")));
    settle(cx);
    fill(cx, "recipe-cpus", "3");
    assert_eq!(draft(cx).cpus, "3", "editable again once the save settled");
    assert_eq!(available(cx, "recipe-base-Your image"), Some(true));
}

/// SPEC.md: "In Settings the editor opens at its top, including Back". The editor replaces a list the user may have
/// scrolled, and the dialog's body keeps its scroll offset across that swap, so only the editor's own opening can bring
/// its top back into view; with a short list the rule holds trivially, which is why the old spec seeded enough recipes
/// to scroll first. Specified: with six recipes, the body scrolled until New recipe shows (the section's heading then
/// lies above the body's top), New recipe opens the editor with Back wholly inside the body; Back again, the body
/// scrolled to the last row's Edit, and Edit opens that recipe's editor with Back wholly inside the body too.
#[gpui::test]
fn the_editor_opens_at_its_top_from_a_scrolled_list(cx: &mut TestAppContext) {
    let names = ["dev-1", "dev-2", "dev-3", "dev-4", "dev-5", "dev-6"];
    let (_helm, cx) = show(
        cx,
        names
            .iter()
            .map(|name| entry(name, idle(), Value::Null))
            .collect(),
    );
    let back_shows_whole = |cx: &mut VisualTestContext| {
        let (body, back) = (bounds(cx, "settings-body"), bounds(cx, "recipe-back"));
        body.top() <= back.top() && back.bottom() <= body.bottom()
    };

    reach(cx, "recipe-new");
    assert!(
        bounds(cx, "settings-heading").top() < bounds(cx, "settings-body").top(),
        "the premise: the list is scrolled"
    );
    keys(cx, "space");
    assert!(drawn(cx, "recipe-editor"));
    assert!(back_shows_whole(cx), "New recipe opens at the editor's top");

    click(cx, "recipe-back");
    reach(cx, "recipe-edit-dev-6");
    assert!(
        bounds(cx, "settings-heading").top() < bounds(cx, "settings-body").top(),
        "the premise: the list is scrolled"
    );
    keys(cx, "space");
    assert_eq!(
        label_of(cx, "recipe-editor-title").as_deref(),
        Some("Recipe: dev-6")
    );
    assert!(back_shows_whole(cx), "Edit opens at the editor's top");
}

// ---- Saving a recipe that exists -----------------------------------------------------------------------------------

/// Save writes the whole recipe, so a draft that dropped or blanked a choice the user never touched would silently
/// stop authenticating a recipe's agents and GitHub on every Edit, and an empty catalog makes every one of those
/// choices unavailable, which must not change what is sent either. Specified, with the catalog read answering empty:
/// Edit on a recipe with a Claude endpoint naming a token, a Codex login, a GitHub token and a tunnel, a new disk size
/// and Save send `PUT /api/tensorlake/recipes/<name>` with the new size and every other field as the catalog listed it,
/// the credential names included.
#[gpui::test]
fn edit_then_save_sends_the_recipes_credential_choices_back_unchanged(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, vec![with_credentials("rust-dev")]);
    open_editor(&mut helm, cx, "recipe-edit-rust-dev");
    assert_eq!(
        label_of(cx, "recipe-editor-title").as_deref(),
        Some("Recipe: rust-dev")
    );
    fill(cx, "recipe-disk", "30");
    click(cx, "recipe-save");
    let save = one_command(&mut helm);
    assert_eq!(
        (save.method, save.path.as_str()),
        (Method::Put, "/api/tensorlake/recipes/rust-dev")
    );
    assert_eq!(
        save.body,
        Some(json!({
            "size": { "cpus": 2, "memory_gb": 4, "disk_gb": 30 },
            "base": { "kind": "tensorlake_ubuntu" },
            "packages": ["git", "jq"],
            "claude": { "base_url": "http://127.0.0.1:8080", "token_credential": "work-endpoint" },
            "codex": null,
            "claude_login_credential": null,
            "codex_login_credential": "codex-work",
            "github_token_credential": "ci-token",
            "tunnels": [{ "sandbox_port": 8080, "host": "127.0.0.1", "port": 8080 }],
        }))
    );
}

/// SPEC.md: a recipe's name "never changes: hosts refer to their recipe by name, and renaming means creating a new
/// recipe". A Name field that took edits would show a name the update ignores, or a draft that never matches its
/// baseline. Specified: Edit shows the saved name; typing into Name changes neither the field nor the draft, also after
/// a refused save has unlocked the form again; Save goes to the saved name's route.
#[gpui::test]
fn a_saved_recipes_name_is_fixed(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, one_recipe());
    open_editor(&mut helm, cx, "recipe-edit-rust-dev");
    assert_eq!(draft(cx).name, "rust-dev");
    fill(cx, "recipe-name", "renamed");
    assert_eq!(draft(cx).name, "rust-dev", "Name takes no edit");
    assert!(!painted(cx).contains("renamed"));

    click(cx, "recipe-save");
    let save = one_command(&mut helm);
    assert_eq!(save.path, "/api/tensorlake/recipes/rust-dev");
    helm.reply(save.id, 400, Some(json!("the helm refused it")));
    settle(cx);
    fill(cx, "recipe-name", "renamed-again");
    assert_eq!(
        draft(cx).name,
        "rust-dev",
        "still fixed once the form is editable again"
    );
}

/// SPEC.md: a failed save keeps the draft and shows the reason, here beside the pinned answers. The refusal shares the
/// recipe's operation key with the row's build, whose refusal the row draws under its build button; one the editor
/// already showed must leave the key, or it would come back under the row as a build refusal once the editor closed.
/// Specified: a refused update shows the helm's words as the editor's error and keeps the edited disk size; the key
/// records no refusal afterwards; Back, then Discard, shows the list with no refusal under the row.
#[gpui::test]
fn a_refused_save_keeps_the_draft_and_leaves_no_refusal_under_the_row(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, one_recipe());
    open_editor(&mut helm, cx, "recipe-edit-rust-dev");
    fill(cx, "recipe-disk", "30");
    click(cx, "recipe-save");
    let save = one_command(&mut helm);
    let reason = "the recipe rust-dev is being deleted";
    helm.reply(save.id, 409, Some(json!(reason)));
    settle(cx);

    assert_eq!(label_of(cx, "recipe-editor-error").as_deref(), Some(reason));
    assert_eq!(draft(cx).disk_gb, "30", "the draft is kept");
    assert_eq!(
        recipe_key(cx, "rust-dev"),
        (None, None),
        "the refusal the editor showed left the key"
    );

    click(cx, "recipe-back");
    click(cx, "prompt-alternative-0");
    assert_eq!(sent(&mut helm), [], "discarding sends nothing");
    assert!(drawn(cx, "recipe-rust-dev"));
    assert!(
        !drawn(cx, "recipe-rust-dev-build-error"),
        "the row shows no refusal of its own"
    );
}

/// A press the key cannot take must say why where it was pressed, not look lost: the recipe's key admits one request
/// at a time, and a build of the same recipe holds it. Specified: with the recipe's build out, Save sends nothing and
/// the line under the answers reads [`Refused::WORDS`]; the editor stays editable.
#[gpui::test]
fn a_save_while_the_recipes_build_is_out_says_why_and_sends_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, one_recipe());
    click(cx, "recipe-build-rust-dev");
    let build = one_command(&mut helm);
    assert_eq!(build.path, "/api/tensorlake/recipes/rust-dev/build");
    // The editor's credential read waits behind the build, since the helm answers in order; Save needs no answer to it.
    click(cx, "recipe-edit-rust-dev");
    assert_eq!(
        one_command(&mut helm).path,
        "/api/credentials",
        "the premise: the editor opened and read the catalog"
    );
    click(cx, "recipe-save");
    assert_eq!(sent(&mut helm), [], "nothing more went out");
    assert_eq!(
        label_of(cx, "recipe-save-busy").as_deref(),
        Some(Refused::WORDS)
    );
    assert_eq!(available(cx, "recipe-back"), Some(true));
}

// ---- Checks before sending -----------------------------------------------------------------------------------------

/// The old spec's "refuses an invalid name locally": a name outside the grammar can only be refused, and saying so
/// before a round trip keeps the user's typing in place; the reason must go with the next edit, as the old `oninput`
/// cleared it, or it would blame a name already fixed. Specified: "Bad Name" and Save send nothing and show the shared
/// rule's reason, capitalized, as the editor's error; an edit of Name drops it; a size that is not a whole number is
/// refused the same way, naming the field.
#[gpui::test]
fn a_bad_name_or_size_is_refused_locally_until_the_next_edit(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, Vec::new());
    open_editor(&mut helm, cx, "recipe-new");
    fill(cx, "recipe-name", "Bad Name");
    click(cx, "recipe-save");
    assert_eq!(sent(&mut helm), [], "a local refusal sends nothing");
    assert_eq!(
        label_of(cx, "recipe-editor-error").as_deref(),
        Some(
            "A recipe name uses lowercase letters, digits and hyphens, starts with a letter and does not end with a \
             hyphen"
        )
    );
    fill(cx, "recipe-name", "good-name");
    assert!(
        !drawn(cx, "recipe-editor-error"),
        "an edit drops the reason"
    );

    fill(cx, "recipe-cpus", "two");
    click(cx, "recipe-save");
    assert_eq!(sent(&mut helm), []);
    assert_eq!(
        label_of(cx, "recipe-editor-error").as_deref(),
        Some("CPUs must be a whole number")
    );
}

// ---- The form ------------------------------------------------------------------------------------------------------

/// The old spec's "saves a custom base": each card must show its own fields and the custom-base requirements, explain
/// what its build leaves without implying a billed snapshot for a container image, and never lose another base's text
/// or the packages, which belong to every base. Specified: a new recipe starts on Managed Ubuntu, pressed, with no
/// requirements note and the both-providers build help; Your image shows Image, the note and Tensorlake's help;
/// Container image shows its reference and user and the container help; back on Your image the typed image is kept;
/// Save sends the image base with the typed packages.
#[gpui::test]
fn each_base_card_shows_its_fields_and_keeps_every_bases_text(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, Vec::new());
    open_editor(&mut helm, cx, "recipe-new");
    assert_eq!(switch_on(cx, "recipe-base-Managed Ubuntu"), Some(true));
    assert!(!drawn(cx, "recipe-base-requirements"));
    assert!(!drawn(cx, "recipe-image"));
    assert!(
        painted(cx)
            .contains("On Tensorlake it takes a snapshot; on a container host it saves an image")
    );

    fill(cx, "recipe-name", "custom");
    type_into(cx, "recipe-packages", "jq\nripgrep");
    choose(cx, "recipe-base-Your image");
    assert_eq!(switch_on(cx, "recipe-base-Your image"), Some(true));
    assert!(drawn(cx, "recipe-image"));
    assert!(drawn(cx, "recipe-base-requirements"));
    assert!(painted(cx).contains("official installers and takes a snapshot."));
    type_into(cx, "recipe-image", "team-base");

    choose(cx, "recipe-base-Container image");
    assert!(drawn(cx, "recipe-container-reference") && drawn(cx, "recipe-container-user"));
    assert!(!drawn(cx, "recipe-image"));
    assert!(painted(cx).contains("saves an image on the container host for launches there."));
    choose(cx, "recipe-base-Your image");
    assert_eq!(draft(cx).image, "team-base", "switching kept the image");
    assert_eq!(draft(cx).packages, "jq\nripgrep", "and the packages");

    click(cx, "recipe-save");
    let save = one_command(&mut helm);
    let body = save.body.unwrap_or_default();
    assert_eq!(
        body["recipe"]["base"],
        json!({ "kind": "image", "image": "team-base" })
    );
    assert_eq!(body["recipe"]["packages"], json!(["jq", "ripgrep"]));
}

/// Tunnel rows come and go by the user's explicit Add and Remove, and a removed row must take its own text with it, not
/// its neighbor's (the rows are keyed by themselves, not their position). An empty row cannot be saved, and saying why
/// before a round trip keeps the rows in place. Specified: Edit on a recipe with one tunnel shows "Tunnel 1"; Add tunnel
/// adds a second, empty row; removing the first leaves the new row, now named "Tunnel 1", and the draft holds only it;
/// Save then sends nothing and says the ports must be from 1 to 65535.
#[gpui::test]
fn tunnel_rows_are_added_and_removed_by_row(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, vec![with_credentials("rust-dev")]);
    open_editor(&mut helm, cx, "recipe-edit-rust-dev");
    assert_eq!(label_of(cx, "recipe-tunnel-0").as_deref(), Some("Tunnel 1"));
    reach(cx, "recipe-add-tunnel");
    keys(cx, "space");
    assert_eq!(label_of(cx, "recipe-tunnel-1").as_deref(), Some("Tunnel 2"));
    reach(cx, "recipe-tunnel-0-remove");
    keys(cx, "space");
    assert!(!drawn(cx, "recipe-tunnel-0"));
    assert_eq!(label_of(cx, "recipe-tunnel-1").as_deref(), Some("Tunnel 1"));
    assert_eq!(
        draft(cx).tunnels,
        [Default::default()],
        "only the new, empty row"
    );

    click(cx, "recipe-save");
    assert_eq!(sent(&mut helm), []);
    assert_eq!(
        label_of(cx, "recipe-editor-error").as_deref(),
        Some("Tunnel ports must be from 1 to 65535")
    );
}

// ---- Switching sections --------------------------------------------------------------------------------------------

/// SPEC.md: "Switching Settings sections hides those actions with the editor", and switching sections preserves
/// unsaved fields. Pinned in the dialog's footer, the answers would otherwise stay under another section, where Save
/// would save a form nobody sees. Specified: with an editor open and a name typed, showing Sessions draws neither Save
/// nor Cancel and keeps the Close row; showing Recipes again reads the account, as every showing does, and draws the
/// editor with its answers and the typed name.
#[gpui::test]
fn switching_sections_hides_the_answers_and_keeps_the_open_editor(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, Vec::new());
    open_editor(&mut helm, cx, "recipe-new");
    fill(cx, "recipe-name", "keep-me");
    assert!(drawn(cx, "recipe-save"));

    click(cx, section_button(Section::Sessions));
    assert!(!drawn(cx, "recipe-save") && !drawn(cx, "recipe-cancel"));
    assert!(!drawn(cx, "settings-notices"), "no pinned region at all");
    assert!(drawn(cx, "settings-footer-close"));

    click(cx, section_button(Section::Recipes));
    answer_account(&mut helm, cx, true);
    assert!(drawn(cx, "recipe-editor") && drawn(cx, "recipe-save"));
    assert_eq!(draft(cx).name, "keep-me");
}

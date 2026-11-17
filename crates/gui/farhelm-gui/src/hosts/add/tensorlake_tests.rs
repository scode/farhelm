//! Tests of Add host's host kind choice and its Tensorlake sandbox body ([`super::tensorlake`]), over the helm client's
//! fake, opened from the same stand-in rail as the SSH tests (`hosts/add/tests.rs`, whose helpers these share): the
//! kind choice and the account read, the recipe editor opened from Recipe, and Continue's plan with the creation
//! question it opens on the prompt (`hosts/setup.rs`), its answers, and where a refused creation shows.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{Collection, CollectionChanges, CollectionState, KeyedEntry};
use gpui::{App, TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::tests::{destination, open_dialog, open_rail_with, type_text};
use crate::hosts::tests::start_work;
use crate::operations::{OpKey, Work};
use crate::test_support::{
    any_open, click, focused_id, frame, keys, label, meta, one_command, settle,
};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The Tensorlake account as `GET /api/tensorlake` answers it, with or without a saved API key.
fn account(key_set: bool) -> Value {
    json!({ "api_key_set": key_set, "create_without_asking": false, "helm_tag": "fixture",
            "ssh_key": { "state": "not_created" } })
}

/// A recipe as the pushed catalog lists it: 2 CPUs, 4 GB memory and 20 GB disk, on the image `base`, with one package.
fn recipe(name: &str, base: &str) -> Value {
    json!({
        "name": name,
        "size": { "cpus": 2, "memory_gb": 4, "disk_gb": 20 },
        "base": { "kind": "image", "image": base },
        "packages": ["jq"],
        "claude": null, "codex": null,
        "claude_login_credential": null, "codex_login_credential": null, "github_token_credential": null,
        "tunnels": [],
    })
}

/// `recipes` keyed by name, in order, as the catalog's entries.
fn keyed(recipes: &[Value]) -> (Vec<String>, Vec<KeyedEntry>) {
    let names: Vec<String> = recipes
        .iter()
        .map(|recipe| recipe["name"].as_str().unwrap().to_owned())
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

/// The pushed recipe catalog holding `recipes`, in that order, for the helm's first snapshot.
fn catalog(recipes: &[Value]) -> CollectionState {
    let (names, entries) = keyed(recipes);
    CollectionState {
        collection: Collection::Recipes,
        meta: meta(&names.iter().map(String::as_str).collect::<Vec<_>>()),
        entries,
    }
}

/// Push `recipes` as the whole catalog, removing `gone`, as the helm's diff after a recipe was deleted.
fn push_catalog(helm: &mut FakeHelm, recipes: &[Value], gone: &[&str]) {
    let (names, upsert) = keyed(recipes);
    helm.push(CollectionChanges {
        collection: Collection::Recipes,
        meta: meta(&names.iter().map(String::as_str).collect::<Vec<_>>()),
        upsert,
        remove: gone.iter().map(|name| (*name).to_owned()).collect(),
    });
}

/// The kind choice's two tabs, by the ids the tab strip gives them (its id and the tab's label).
const SSH_TAB: &str = "add-host-kind-SSH";
const SANDBOX_TAB: &str = "add-host-kind-Tensorlake sandbox";

/// Choose the Tensorlake sandbox kind by clicking its tab, and take the one account read that sends: its envelope id.
fn choose_tensorlake(cx: &mut VisualTestContext, helm: &mut FakeHelm) -> u64 {
    click(cx, SANDBOX_TAB);
    one_command(helm, Method::Get, "/api/tensorlake").0
}

/// The open dialog's sandbox form; panics unless the Tensorlake sandbox kind is shown.
fn sandbox_form(cx: &App) -> gpui::Entity<super::TensorlakeForm> {
    let dialog = crate::layers::last_opened::<super::AddHostDialog>(cx).expect("a dialog is open");
    match &dialog.read(cx).body {
        super::Body::Tensorlake(form) => form.clone(),
        super::Body::Ssh => panic!("the sandbox kind is shown"),
    }
}

/// The open dialog's sandbox name as typed.
fn sandbox_name(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| sandbox_form(cx).read(cx).name_typed())
}

/// Whether the control `id` is drawn, from a fresh frame.
fn drawn(cx: &mut VisualTestContext, id: &str) -> bool {
    frame(cx)
        .iter()
        .any(|node| node.element_id.as_deref() == Some(id))
}

/// Whether the control `id` can be pressed, from a fresh frame, or `None` when it is not drawn.
fn pressable(cx: &mut VisualTestContext, id: &str) -> Option<bool> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| node.flags.click)
}

/// Whether `words` are drawn somewhere: every text run of a fresh frame joined, whitespace collapsed, so a line that
/// wraps still reads as written.
fn shows(cx: &mut VisualTestContext, words: &str) -> bool {
    let drawn = frame(cx)
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect::<Vec<_>>()
        .join(" ");
    let drawn = drawn.split_whitespace().collect::<Vec<_>>().join(" ");
    drawn.contains(words)
}

/// Whether focus is inside the control `id`, from a fresh frame: the focused element or one around it has that id.
/// GPUI Kit's text inputs share one inner focus id (`input-state`), so [`focused_id`] cannot tell one field from
/// another; the field's own id is on an element around that one.
fn focus_in(cx: &mut VisualTestContext, id: &str) -> bool {
    let nodes = frame(cx);
    nodes.iter().filter(|node| node.flags.focused).any(|node| {
        let mut at = Some(node);
        while let Some(node) = at {
            if node.element_id.as_deref() == Some(id) {
                return true;
            }
            at = node.parent.map(|parent| &nodes[parent]);
        }
        false
    })
}

/// The value the closed Recipe select shows, from a fresh frame.
fn recipe_shown(cx: &mut VisualTestContext) -> Option<String> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("add-tensorlake-recipe"))
        .and_then(|node| node.value)
        .map(|value| value.to_string())
}

// ---- Tests ---------------------------------------------------------------------------------------------------------

/// Why this matters: Add host is where both kinds of host are added. Switching kinds must keep the SSH draft (it lives on
/// the dialog), must not start the Tensorlake account read before the user asked for a sandbox (an SSH add never needs
/// it), and starts the sandbox's form afresh each time, as the old dialog's remount did.
///
/// Specified: the dialog opens on SSH with no read sent; choosing Tensorlake sandbox sends one `GET /api/tensorlake`,
/// says "Loading Tensorlake settings…" until it answers, swaps the header's summary and puts an unavailable Continue
/// in Add's place; choosing SSH again shows the destination as typed and Add; choosing Tensorlake sandbox again reads
/// the account again and shows an empty Name.
#[gpui::test]
fn the_kind_choice_swaps_bodies_keeps_the_ssh_draft_and_starts_the_sandbox_afresh(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _) = open_rail_with(cx, Vec::new());
    open_dialog(cx);
    type_text(cx, "dev@box");
    assert!(
        helm.commands().is_empty(),
        "the premise: an SSH add reads nothing"
    );

    let read = choose_tensorlake(cx, &mut helm);
    assert!(shows(cx, "Loading Tensorlake settings…"));
    assert!(shows(cx, "A Tensorlake sandbox created from a recipe"));
    assert_eq!(pressable(cx, "add-tensorlake-continue"), Some(false));
    assert!(!drawn(cx, "add-host-submit"), "Continue takes Add's place");
    helm.reply(read, 200, Some(account(true)));
    cx.run_until_parked();
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(SANDBOX_TAB),
        "the premise: the click left focus on the chosen tab, before Name"
    );
    keys(cx, "tab");
    type_text(cx, "gpubox");
    assert_eq!(
        sandbox_name(cx),
        "gpubox",
        "the premise: the first form holds the typed name"
    );

    click(cx, SSH_TAB);
    assert_eq!(destination(cx), "dev@box", "the SSH draft is as it was");
    assert!(drawn(cx, "add-host-submit") && !drawn(cx, "add-tensorlake-continue"));
    assert!(shows(cx, "An SSH destination this helm can reach"));

    let again = choose_tensorlake(cx, &mut helm);
    helm.reply(again, 200, Some(account(true)));
    cx.run_until_parked();
    assert!(drawn(cx, "add-tensorlake-name"));
    assert_eq!(sandbox_name(cx), "", "the sandbox's form started afresh");
}

/// Why this matters: without an API key nothing can be created, and the way out is Settings → Tensorlake; the old
/// dialog read the account again when Settings closed, so a key saved there brings up the form without reopening Add
/// host.
///
/// Specified: with no key saved the body says "Save a Tensorlake API key in Settings → Tensorlake before adding a
/// sandbox." and offers "Open Settings → Tensorlake" and no Name; the button opens Settings over Add host; closing
/// Settings with Escape returns focus to the button and reads the account again, and an account with a key then shows
/// Name and Recipe, with focus in Name. Tab reaching the button without a trip to Settings reads nothing.
#[gpui::test]
fn without_a_key_the_body_opens_settings_and_reads_the_account_again_when_it_closes(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _) = open_rail_with(cx, Vec::new());
    open_dialog(cx);
    let read = choose_tensorlake(cx, &mut helm);
    helm.reply(read, 200, Some(account(false)));
    cx.run_until_parked();
    assert!(shows(
        cx,
        "Save a Tensorlake API key in Settings → Tensorlake before adding a sandbox."
    ));
    assert!(!drawn(cx, "add-tensorlake-name"));
    // Focus reaching the button without a trip to Settings reads nothing.
    keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("add-tensorlake-open-settings")
    );
    assert!(
        helm.commands().is_empty(),
        "only a return from Settings reads the account again"
    );

    click(cx, "add-tensorlake-open-settings");
    assert!(
        shows(cx, "For every host on this helm"),
        "Settings opened over Add host"
    );
    // Settings reads what its Tensorlake section needs; answer each in order, the account still without a key.
    for command in helm.commands() {
        let body = (command.path == "/api/tensorlake").then(|| account(false));
        helm.reply(command.id, 200, body);
    }
    cx.run_until_parked();

    keys(cx, "escape");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("add-tensorlake-open-settings"),
        "closing Settings returns focus to the button"
    );
    let (again, _) = one_command(&mut helm, Method::Get, "/api/tensorlake");
    helm.reply(again, 200, Some(account(true)));
    cx.run_until_parked();
    assert!(drawn(cx, "add-tensorlake-name") && drawn(cx, "add-tensorlake-recipe"));
    assert!(any_open(cx), "Add host is still open");
    assert!(
        focus_in(cx, "add-tensorlake-name"),
        "the key saved in Settings put focus in Name"
    );
}

/// Why this matters: a failed account read must say why and let the user try again, rather than leave the body
/// loading for good or show a form whose key state is unknown.
///
/// Specified: a refused read shows the helm's words and Retry and no Name; Retry sends the read again and shows the
/// loading line until it answers.
#[gpui::test]
fn a_failed_account_read_shows_the_helms_words_and_retry_reads_again(cx: &mut TestAppContext) {
    let (mut helm, cx, _) = open_rail_with(cx, Vec::new());
    open_dialog(cx);
    let read = choose_tensorlake(cx, &mut helm);
    helm.reply(
        read,
        503,
        Some(json!({ "error": "the helm's store is busy" })),
    );
    cx.run_until_parked();
    assert!(
        label(cx, "add-tensorlake-account-error")
            .is_some_and(|line| line.contains("the helm's store is busy")),
        "{:?}",
        label(cx, "add-tensorlake-account-error")
    );
    assert!(!drawn(cx, "add-tensorlake-name"));

    click(cx, "add-tensorlake-account-retry");
    let (again, _) = one_command(&mut helm, Method::Get, "/api/tensorlake");
    assert!(shows(cx, "Loading Tensorlake settings…"));
    helm.reply(again, 200, Some(account(true)));
    cx.run_until_parked();
    assert!(drawn(cx, "add-tensorlake-name"));
}

/// Why this matters: the recipe is what the sandbox is created from and what it bills for, so the choice must offer
/// exactly the catalog's recipes, describe the chosen one the way Settings → Recipes does, and never silently swap a
/// chosen recipe that went away for another.
///
/// Specified: Recipe starts on "Choose a recipe" and offers the catalog's recipes in order, then "New recipe…";
/// choosing one shows it and its summary (size, base and packages, with the image's name as written); when a push takes
/// it out of the catalog, the select shows "<name> (unavailable)" and no summary.
#[gpui::test]
fn the_recipe_choice_offers_the_catalog_shows_the_summary_and_keeps_a_vanished_choice(
    cx: &mut TestAppContext,
) {
    let recipes = [recipe("rust-dev", "team/devbox"), recipe("web", "team/web")];
    let (mut helm, cx, _) = open_rail_with(cx, vec![catalog(&recipes)]);
    open_dialog(cx);
    let read = choose_tensorlake(cx, &mut helm);
    helm.reply(read, 200, Some(account(true)));
    cx.run_until_parked();
    assert_eq!(recipe_shown(cx).as_deref(), Some("Choose a recipe"));

    click(cx, "add-tensorlake-recipe");
    let offered: Vec<String> = frame(cx)
        .into_iter()
        .filter(|node| node.role == Some(gpui::Role::MenuListOption))
        .filter_map(|node| node.element_id.map(|id| id.to_string()))
        .collect();
    assert_eq!(
        offered,
        [
            "add-tensorlake-recipe-option-rust-dev",
            "add-tensorlake-recipe-option-web",
            "add-tensorlake-recipe-option-__new__"
        ],
        "the catalog's recipes in order, then New recipe…"
    );
    click(cx, "add-tensorlake-recipe-option-rust-dev");
    assert_eq!(recipe_shown(cx).as_deref(), Some("rust-dev"));
    assert!(shows(
        cx,
        "2 CPU · 4 GB · 20 GB · base: your image team/devbox · 1 package"
    ));

    push_catalog(&mut helm, &[recipe("web", "team/web")], &["rust-dev"]);
    cx.run_until_parked();
    assert_eq!(
        recipe_shown(cx).as_deref(),
        Some("rust-dev (unavailable)"),
        "a vanished choice stays chosen, marked"
    );
    assert!(!shows(
        cx,
        "2 CPU · 4 GB · 20 GB · base: your image team/devbox · 1 package"
    ));
}

/// Why this matters: the sandbox body hides the SSH fields but keeps their draft, and Enter on the dialog itself runs
/// the dialog's main action; if that were still Add, Enter would send the hidden destination to the helm, which then
/// dials it, for a draft the user cannot see.
///
/// Specified: with a destination typed and the Tensorlake sandbox kind shown, Enter while the dialog itself holds focus
/// sends nothing beyond the account read the choice sent.
#[gpui::test]
fn enter_on_the_dialog_with_the_sandbox_shown_never_probes_the_hidden_destination(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _) = open_rail_with(cx, Vec::new());
    open_dialog(cx);
    type_text(cx, "dev@box");
    let read = choose_tensorlake(cx, &mut helm);
    helm.reply(read, 200, Some(account(true)));
    cx.run_until_parked();
    // Focus on the dialog itself, as a click on its empty space gives it in dialogs without Add host's keep-focus rule.
    cx.update(|window, cx| {
        let dialog = crate::layers::top_container(window, cx).expect("Add host is open");
        let mark = crate::focus::Mark::take(cx);
        crate::focus::focus_in_layer(&dialog, &mark, window, cx);
    });
    crate::test_support::settle(cx);
    let on_dialog = cx.update(|window, cx| {
        crate::layers::top_container(window, cx).is_some_and(|dialog| dialog.is_focused(window))
    });
    assert!(on_dialog, "the premise: the dialog itself has focus");
    keys(cx, "enter");
    assert!(
        helm.commands().is_empty(),
        "Enter sent nothing: {:?}",
        helm.commands()
    );
    assert!(any_open(cx));
}

/// Why this matters: while Add's probe is out the fields being checked must stay on screen, as the old dialog made the
/// other kind unavailable then; a switch would hide them and start the sandbox's account read beside the probe.
///
/// Specified: with the SSH probe out, choosing the Tensorlake sandbox kind keeps the SSH fields and Add's "Checking…"
/// on screen and sends nothing.
#[gpui::test]
fn a_kind_switch_while_the_probe_is_out_does_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx, _) = open_rail_with(cx, Vec::new());
    open_dialog(cx);
    type_text(cx, "dev@box");
    click(cx, "add-host-submit");
    let _ = super::tests::one_probe(&mut helm);
    click(cx, SANDBOX_TAB);
    assert!(
        helm.commands().is_empty(),
        "no account read beside the probe"
    );
    assert!(drawn(cx, "add-host-destination") && !drawn(cx, "add-tensorlake-continue"));
    assert_eq!(label(cx, "add-host-submit").as_deref(), Some("Checking…"));
}

// ---- The recipe editor in Add host ---------------------------------------------------------------------------------

/// Open Add host on the Tensorlake sandbox kind with a key saved and `recipes` in the catalog, then choose "New
/// recipe…" and answer the one read the editor sends as it opens, the credential catalog's, with nothing saved.
fn open_new_recipe<'a>(
    cx: &'a mut TestAppContext,
    recipes: &[Value],
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut helm, cx, _) = open_rail_with(cx, vec![catalog(recipes)]);
    open_dialog(cx);
    let read = choose_tensorlake(cx, &mut helm);
    helm.reply(read, 200, Some(account(true)));
    cx.run_until_parked();
    click(cx, "add-tensorlake-recipe");
    click(cx, "add-tensorlake-recipe-option-__new__");
    let (credentials, _) = one_command(&mut helm, Method::Get, "/api/credentials");
    helm.reply(credentials, 200, Some(json!([])));
    cx.run_until_parked();
    assert!(
        focus_in(cx, "recipe-name"),
        "the premise: the editor opened with focus in its Name"
    );
    (helm, cx)
}

/// Why this matters: a recipe made from Add host must not cost the user the dialog they are in, and its Save must stay
/// reachable: the editor is taller than the dialog, and answers that scroll with it are lost below the fold (the old
/// suite's "Add host pins inline recipe answers and focuses Name").
///
/// Specified: "New recipe…" opens the editor in place of the fields with focus in its Name, without Back; the kind
/// choice, Cancel and Continue give way to the editor's Cancel and Save. The editor is taller than the dialog's body,
/// and Cancel and Save sit in the footer, below the scrolling body and inside the frame; Tab from the editor's fields
/// reaches Cancel, then Save.
#[gpui::test]
fn new_recipe_opens_the_editor_with_focus_in_name_and_pins_its_answers(cx: &mut TestAppContext) {
    let (_helm, cx) = open_new_recipe(cx, &[recipe("rust-dev", "team/devbox")]);
    assert!(drawn(cx, "recipe-editor") && !drawn(cx, "add-tensorlake-name"));
    assert!(!drawn(cx, "recipe-back"), "Add host's editor has no Back");
    assert!(
        !drawn(cx, SANDBOX_TAB),
        "the kind choice is hidden while the editor is open"
    );
    assert!(!drawn(cx, "add-host-cancel") && !drawn(cx, "add-tensorlake-continue"));
    assert!(drawn(cx, "recipe-cancel") && drawn(cx, "recipe-save"));
    let (frame_box, body, last, save) = (
        crate::test_support::bounds(cx, "add-host"),
        crate::test_support::bounds(cx, "add-host-body"),
        crate::test_support::bounds(cx, "recipe-add-tunnel"),
        crate::test_support::bounds(cx, "recipe-save"),
    );
    assert!(
        last.top() > body.bottom(),
        "the premise: the editor runs on below the body's view: {last:?} under {body:?}"
    );
    assert!(
        save.bottom() <= frame_box.bottom() && save.top() >= body.bottom(),
        "Save is pinned below the scrolling body, inside the dialog: {save:?} under {body:?} in {frame_box:?}"
    );
    // Tab walks the editor's fields and then reaches the pinned answers, Cancel first.
    for _ in 0..60 {
        if focused_id(cx).as_deref() == Some("recipe-cancel") {
            break;
        }
        keys(cx, "tab");
    }
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("recipe-cancel"),
        "Tab reached Cancel"
    );
    keys(cx, "tab");
    assert_eq!(focused_id(cx).as_deref(), Some("recipe-save"), "then Save");
}

/// Why this matters: the inline editor replaces Add host's fields and the user reads it from its top, its title naming
/// what is being made; an editor that opened scrolled would also hide whether the pinned answers stand where they do on
/// their own or only because the body scrolled (the old suite's "Add host pins inline recipe answers and focuses Name",
/// whose premise was that the body had not scrolled).
///
/// Specified: right after "New recipe…", with the editor taller than the body, its title and its Name field lie wholly
/// inside the body's view.
#[gpui::test]
fn the_inline_editor_opens_at_its_top(cx: &mut TestAppContext) {
    let (_helm, cx) = open_new_recipe(cx, &[recipe("rust-dev", "team/devbox")]);
    let body = crate::test_support::bounds(cx, "add-host-body");
    assert!(
        crate::test_support::bounds(cx, "recipe-add-tunnel").top() > body.bottom(),
        "the premise: the editor runs on below the body's view"
    );
    for id in ["recipe-editor-title", "recipe-name"] {
        let shown = crate::test_support::bounds(cx, id);
        assert!(
            body.top() <= shown.top() && shown.bottom() <= body.bottom(),
            "{id} shows whole at the body's top: {shown:?} in {body:?}"
        );
    }
}

/// Why this matters: the point of making a recipe from Add host is to create the sandbox from it, so a done Save must
/// come back to the form with that recipe chosen, and with focus where the user goes on from.
///
/// Specified: Save sends `POST /api/tensorlake/recipes` for the typed name; once it is done (the catalog pushed with
/// the new recipe first, as the helm does), the editor closes, Recipe shows the new recipe with its summary, and focus
/// is on Recipe.
#[gpui::test]
fn a_done_save_chooses_the_new_recipe_and_returns_focus_to_recipe(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_new_recipe(cx, &[recipe("rust-dev", "team/devbox")]);
    type_text(cx, "newbox");
    click(cx, "recipe-save");
    let (save, body) = one_command(&mut helm, Method::Post, "/api/tensorlake/recipes");
    assert_eq!(body["name"], "newbox", "{body}");
    push_catalog(
        &mut helm,
        &[
            recipe("rust-dev", "team/devbox"),
            recipe("newbox", "team/new"),
        ],
        &[],
    );
    helm.reply(save, 200, Some(json!({})));
    cx.run_until_parked();
    assert!(!drawn(cx, "recipe-editor") && drawn(cx, "add-tensorlake-name"));
    assert_eq!(recipe_shown(cx).as_deref(), Some("newbox"));
    assert!(shows(cx, "base: your image team/new"));
    assert_eq!(focused_id(cx).as_deref(), Some("add-tensorlake-recipe"));
    assert!(drawn(cx, SANDBOX_TAB), "the kind choice is back");
}

/// Why this matters: closing Add host must not silently drop a recipe being written, and the editor's own Cancel must
/// return to the form rather than close Add host.
///
/// Specified: with a typed name, the editor's Cancel asks "Save these changes before leaving?", and Discard returns
/// to the form with Add host open and focus on Recipe; opened again and typed into, Escape asks the same question with
/// Add host still open, and Discard then closes Add host. Nothing is sent to the helm.
#[gpui::test]
fn leaving_with_unsaved_changes_asks_and_discard_goes_where_it_was_asked(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_new_recipe(cx, &[]);
    type_text(cx, "draft");
    click(cx, "recipe-cancel");
    assert!(shows(cx, "Save these changes before leaving?"));
    click(cx, "prompt-alternative-0");
    assert!(
        any_open(cx) && drawn(cx, "add-tensorlake-name"),
        "Discard went back to the form"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("add-tensorlake-recipe"));

    click(cx, "add-tensorlake-recipe");
    click(cx, "add-tensorlake-recipe-option-__new__");
    let (credentials, _) = one_command(&mut helm, Method::Get, "/api/credentials");
    helm.reply(credentials, 200, Some(json!([])));
    cx.run_until_parked();
    assert!(focus_in(cx, "recipe-name"), "the premise: focus in Name");
    type_text(cx, "draft");
    keys(cx, "escape");
    assert!(shows(cx, "Save these changes before leaving?"));
    assert!(any_open(cx), "Escape asked rather than closing Add host");
    click(cx, "prompt-alternative-0");
    assert!(!any_open(cx), "Discard closed Add host");
    assert!(helm.commands().is_empty(), "nothing was saved");
}

// ---- Continue and the creation confirmation ------------------------------------------------------------------------

/// The helm's Tensorlake creation plan as `POST /api/hosts/probe` answers it, with the one-use id `probe_id`.
fn plan(probe_id: &str) -> Value {
    json!({
        "result": "tensorlake-provisionable",
        "probe_id": probe_id,
        "sandbox": "farhelm-host-fixture-gpubox",
        "destination": "farhelm-host-fixture-gpubox",
        "recipe": "rust-dev",
        "resources": { "cpus": 2, "memory_gb": 4, "disk_gb": 20 },
        "confirmation": "Create a Tensorlake sandbox\n1. Create the sandbox\n",
        "steps": {
            "lead": "Create a Tensorlake sandbox",
            "kept": null,
            "steps": [{ "id": "create-sandbox", "name": "Create the sandbox", "paths": [], "explanation": null }],
        },
    })
}

/// The helm's refusal of a creation whose recipe went away while the question was open.
const RECIPE_GONE: &str =
    "the recipe was deleted while confirmation was open; choose a recipe again";

/// Open Add host on the Tensorlake sandbox kind with a key saved (and `create_without_asking` as given), then fill
/// Name with `gpubox` and choose `rust-dev`, as a user does. The catalog must hold `rust-dev`.
fn fill_form(cx: &mut VisualTestContext, helm: &mut FakeHelm, create_without_asking: bool) {
    open_dialog(cx);
    let read = choose_tensorlake(cx, helm);
    let mut settings = account(true);
    settings["create_without_asking"] = json!(create_without_asking);
    helm.reply(read, 200, Some(settings));
    cx.run_until_parked();
    keys(cx, "tab");
    type_text(cx, "gpubox");
    click(cx, "add-tensorlake-recipe");
    click(cx, "add-tensorlake-recipe-option-rust-dev");
}

/// The rail with `rust-dev` in the catalog (and `more` in the helm's first snapshot), and Add host filled by
/// [`fill_form`].
fn filled_form(
    cx: &mut TestAppContext,
    create_without_asking: bool,
    more: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext) {
    let mut snapshot = vec![catalog(&[recipe("rust-dev", "team/devbox")])];
    snapshot.extend(more);
    let (mut helm, cx, _) = open_rail_with(cx, snapshot);
    fill_form(cx, &mut helm, create_without_asking);
    (helm, cx)
}

/// Press Continue and take the probe it sends: its envelope id and body.
fn continue_to_probe(cx: &mut VisualTestContext, helm: &mut FakeHelm) -> (u64, Value) {
    click(cx, "add-tensorlake-continue");
    one_command(helm, Method::Post, "/api/hosts/probe")
}

/// Press Continue and answer its probe with a plan under `probe_id`: the creation confirmation opens, unless the form
/// creates without asking.
fn confirm_plan(cx: &mut VisualTestContext, helm: &mut FakeHelm, probe_id: &str) {
    let (probe, _) = continue_to_probe(cx, helm);
    helm.reply(probe, 200, Some(plan(probe_id)));
    settle(cx);
}

/// The open question's title, which the prompt draws under its own id.
fn question_title(cx: &mut VisualTestContext) -> Option<String> {
    label(cx, "prompt")
}

/// Whether Add host is open on the Tensorlake sandbox's fields, as a reopened Add host starts.
fn sandbox_fields_open(cx: &mut VisualTestContext) -> bool {
    any_open(cx) && drawn(cx, "add-tensorlake-name")
}

/// Why this matters: Continue is where the user asks what creating a sandbox would cost, and nothing may be created
/// before they confirm; the plan they confirm must be the one for the name and recipe they typed, and its question is
/// asked as every other question is, on the prompt, with the old dialog's answers in its order.
///
/// Specified: Continue is unavailable until Name and Recipe are both set; it then sends one `POST /api/hosts/probe`
/// for the Tensorlake target with the name and recipe, reading "Checking…" meanwhile. The plan closes Add host and
/// opens "Create Tensorlake sandbox · gpubox" with the size it bills for ("2 CPUs · 4 GB memory · 20 GB disk"), the
/// helm's steps, Copy plan and the billing callout with its pricing link; the answers read Cancel, Back and Create
/// sandbox in that order, focus is on Create sandbox, and nothing else is sent.
#[gpui::test]
fn continue_asks_for_the_plan_and_the_question_shows_its_size_steps_and_billing(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _) = open_rail_with(cx, vec![catalog(&[recipe("rust-dev", "team/devbox")])]);
    open_dialog(cx);
    let read = choose_tensorlake(cx, &mut helm);
    helm.reply(read, 200, Some(account(true)));
    cx.run_until_parked();
    assert_eq!(
        pressable(cx, "add-tensorlake-continue"),
        Some(false),
        "nothing typed yet"
    );
    keys(cx, "tab");
    type_text(cx, "gpubox");
    assert_eq!(
        pressable(cx, "add-tensorlake-continue"),
        Some(false),
        "no recipe yet"
    );
    click(cx, "add-tensorlake-recipe");
    click(cx, "add-tensorlake-recipe-option-rust-dev");
    assert_eq!(pressable(cx, "add-tensorlake-continue"), Some(true));

    let (probe, body) = continue_to_probe(cx, &mut helm);
    assert_eq!(
        body,
        json!({ "target": { "kind": "tensorlake", "name": "gpubox", "recipe": "rust-dev" } })
    );
    assert_eq!(
        label(cx, "add-tensorlake-continue").as_deref(),
        Some("Checking…")
    );
    helm.reply(probe, 200, Some(plan("probe-1")));
    settle(cx);
    assert_eq!(
        question_title(cx).as_deref(),
        Some("Create Tensorlake sandbox · gpubox")
    );
    assert!(!drawn(cx, "add-tensorlake-name"), "Add host closed");
    for words in [
        "2 CPUs · 4 GB memory · 20 GB disk",
        "Create a Tensorlake sandbox",
        "Create the sandbox",
        "Copy plan",
        "This sandbox bills while running.",
        "Tensorlake pricing",
        "Farhelm never resumes it on its own.",
    ] {
        assert!(shows(cx, words), "{words:?} is drawn");
    }
    let answers: Vec<String> = frame(cx)
        .into_iter()
        .filter(|node| {
            node.element_id.as_deref().is_some_and(|id| {
                matches!(
                    id,
                    "prompt-cancel" | "prompt-alternative-0" | "prompt-confirm"
                )
            })
        })
        .filter_map(|node| node.label.map(|label| label.to_string()))
        .collect();
    assert_eq!(answers, ["Cancel", "Back", "Create sandbox"]);
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    assert!(
        helm.commands().is_empty(),
        "nothing is created before Create sandbox"
    );
}

/// Why this matters: the plan is for the name and recipe Continue sent, and Back or a refusal reopens Add host with the
/// fields as they are when the plan arrives, so the fields must hold still while the probe is out, as Add's do.
///
/// Specified: with Continue's probe out (sent by Enter in Name, which keeps focus there), Recipe cannot be pressed and
/// typing in Name changes nothing.
#[gpui::test]
fn the_fields_hold_still_while_the_probe_is_out(cx: &mut TestAppContext) {
    let (mut helm, cx) = filled_form(cx, false, Vec::new());
    keys(cx, "shift-tab");
    assert!(
        focus_in(cx, "add-tensorlake-name"),
        "the premise: focus back in Name"
    );
    keys(cx, "enter");
    let _ = one_command(&mut helm, Method::Post, "/api/hosts/probe");
    assert!(
        focus_in(cx, "add-tensorlake-name"),
        "the premise: focus still in Name"
    );
    assert_eq!(pressable(cx, "add-tensorlake-recipe"), Some(false));
    type_text(cx, "x");
    assert_eq!(sandbox_name(cx), "gpubox", "Name is read-only meanwhile");
}

/// Why this matters: the pricing link is how the user checks what a sandbox will cost before Create sandbox, so it
/// must open Tensorlake's pricing page, the old callout's link.
///
/// Specified: pressing "Tensorlake pricing" in the question opens `https://www.tensorlake.ai/pricing`.
#[gpui::test]
fn the_pricing_link_opens_tensorlakes_pricing_page(cx: &mut TestAppContext) {
    let (mut helm, cx) = filled_form(cx, false, Vec::new());
    confirm_plan(cx, &mut helm, "probe-1");
    click(cx, "sandbox-pricing");
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("https://www.tensorlake.ai/pricing")
    );
}

/// Why this matters: the billing callout is the one warning between the user and a sandbox that bills by the hour,
/// and the old suite checked its look on purpose: a warning wash and edge make it read as a callout rather than one
/// more line of the plan.
///
/// Specified: over the callout's own bounds the palette's `warn_bg_dim` fill is painted, and an edge in `warn` that is
/// three pixels on the left and nothing elsewhere. (GPUI paints a box's fill and its edges as two quads.)
#[gpui::test]
fn the_billing_callout_has_the_warning_wash_and_edge(cx: &mut TestAppContext) {
    let (mut helm, cx) = filled_form(cx, false, Vec::new());
    confirm_plan(cx, &mut helm, "probe-1");
    let billing = crate::test_support::bounds(cx, "sandbox-billing");
    let palette = cx.update(|_, cx| crate::theme::palette(cx));
    let (scale, quads) = cx.update(|window, _| (window.scale_factor(), window.painted_quads()));
    let wash = gpui::Background::from(crate::theme::color(palette.warn_bg_dim));
    let own: Vec<_> = quads
        .iter()
        .filter(|quad| quad.bounds == billing.scale(scale))
        .collect();
    assert!(
        own.iter().any(|quad| quad.background == wash),
        "the callout's box paints the warning wash"
    );
    let quad = own
        .iter()
        .find(|quad| quad.border_color == crate::theme::color(palette.warn))
        .expect("the callout's box paints an edge in the warning color");
    assert_eq!(quad.border_widths.left, gpui::px(3.).scale(scale));
    let none = gpui::px(0.).scale(scale);
    assert!(
        [
            quad.border_widths.top,
            quad.border_widths.right,
            quad.border_widths.bottom
        ] == [none; 3],
        "only the left edge"
    );
}

/// Why this matters: the callout's last sentence is the one that says Tensorlake suspends an idle sandbox and Farhelm
/// never resumes it, and the old callout wrapped it inside its box. The `host-add-tensorlake-creation` golden caught
/// this port drawing it as one unbreakable item of the row that holds the pricing link, so it ran off the dialog's
/// right edge mid-word, led by a stray space.
///
/// Specified: in the creation question, the Usage Credits sentence is painted as one text whose box lies wholly inside
/// the callout's bounds and is taller than one 20.8-pixel line (it wrapped), and the text is exactly the sentence, with
/// no leading space. GPUI's driver record keeps one entry per painted paragraph, its box the wrapped text's whole size,
/// so the box's width and height are what tell a wrapped sentence from one running out of the callout.
#[gpui::test]
fn the_usage_credits_sentence_wraps_inside_the_billing_callout(cx: &mut TestAppContext) {
    let (mut helm, cx) = filled_form(cx, false, Vec::new());
    confirm_plan(cx, &mut helm, "probe-1");
    let billing = crate::test_support::bounds(cx, "sandbox-billing");
    let note = crate::hosts::setup::USAGE_CREDITS_NOTE;
    let painted: Vec<gpui::DriverText> = frame(cx)
        .into_iter()
        .flat_map(|node| node.texts)
        .filter(|text| text.text.contains("Usage Credits"))
        .collect();
    let [sentence] = painted.as_slice() else {
        panic!("the sentence is painted once: {painted:?}");
    };
    assert_eq!(
        sentence.text.as_ref(),
        note,
        "the sentence alone, with no leading space"
    );
    let shown = sentence.bounds;
    assert!(
        billing.left() <= shown.left()
            && shown.right() <= billing.right()
            && billing.top() <= shown.top()
            && shown.bottom() <= billing.bottom(),
        "the sentence lies inside the callout: {shown:?} in {billing:?}"
    );
    assert!(
        shown.size.height > gpui::px(21.),
        "the sentence wraps onto more than one line: {shown:?}"
    );
}

/// Why this matters: Back must return to what was typed, never lose it, and leave Name editable: a plan is for the
/// name and recipe it was made for, so going back to change the name must lead to a plan for the new one (the old
/// suite's Back test edited the name before Continue for exactly this).
///
/// Specified: Back sends nothing and reopens Add host on the Tensorlake sandbox kind with `gpubox` and `rust-dev` kept
/// and focus in Name, without reading the account again; typing there edits the name, and Continue's probe and the
/// question that follows name `gpubox2`.
#[gpui::test]
fn back_reopens_add_host_with_the_fields_kept_and_continue_asks_for_the_edited_name(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = filled_form(cx, false, Vec::new());
    confirm_plan(cx, &mut helm, "probe-1");
    click(cx, "prompt-alternative-0");
    settle(cx);
    assert!(helm.commands().is_empty(), "Back sends nothing");
    assert!(sandbox_fields_open(cx), "Add host is back on the sandbox");
    assert_eq!(question_title(cx), None, "the question closed");
    assert_eq!(sandbox_name(cx), "gpubox");
    assert_eq!(recipe_shown(cx).as_deref(), Some("rust-dev"));
    assert!(focus_in(cx, "add-tensorlake-name"), "focus is back in Name");
    type_text(cx, "2");
    assert_eq!(sandbox_name(cx), "gpubox2", "Name is editable again");
    let (probe, body) = continue_to_probe(cx, &mut helm);
    assert_eq!(body["target"]["name"], "gpubox2", "Continue asks again");
    helm.reply(probe, 200, Some(plan("probe-2")));
    settle(cx);
    assert_eq!(
        question_title(cx).as_deref(),
        Some("Create Tensorlake sandbox · gpubox2")
    );
}

/// Why this matters: Create sandbox is the one press that bills, so it must send exactly the plan shown, once, under
/// Add host's key, where a second creation from the same opening is refused; the run's progress then shows on the new
/// host's row.
///
/// Specified: Create sandbox closes the question and sends `POST /api/hosts/provision` with the plan's id under the
/// key Continue's probe ran under; the helm's acceptance leaves nothing open.
#[gpui::test]
fn create_sandbox_submits_the_plan_under_add_hosts_key(cx: &mut TestAppContext) {
    let (mut helm, cx) = filled_form(cx, false, Vec::new());
    let key = super::tests::dialog_key(cx);
    confirm_plan(cx, &mut helm, "probe-1");
    click(cx, "prompt-confirm");
    let (create, body) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    assert_eq!(body, json!({ "probe_id": "probe-1" }));
    assert_eq!(question_title(cx), None, "the question closed");
    assert!(
        cx.update(|_, cx| crate::session::current_operations(cx)
            .is_some_and(|operations| operations.read(cx).in_flight(&key).is_some())),
        "the creation runs under Add host's key"
    );
    helm.reply(
        create,
        202,
        Some(json!({ "host_id": 7, "run_id": "run-1" })),
    );
    settle(cx);
    assert!(!any_open(cx), "nothing reopened once the helm accepted it");
}

/// Why this matters: a creation is a host registry change, which the migration keeps out of the way of a session
/// launch, and the question can stay open while a launch starts, so checking only at Continue would let Create
/// sandbox put a new host beside the launch. The refusal takes the path a refused SSH setup takes, so the user can try
/// again with the fields as they were.
///
/// Specified: with the question open and a launcher `Launch` in flight, Create sandbox sends no provision request and
/// reopens Add host on the sandbox with `gpubox` and `rust-dev`, focus in Name and its error line reading "Add host:
/// not started, a session launch is in progress; try again when it finishes".
#[gpui::test]
fn create_sandbox_during_a_launch_sends_nothing_and_reopens_add_host_saying_why(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = filled_form(cx, false, Vec::new());
    confirm_plan(cx, &mut helm, "probe-1");
    assert!(
        question_title(cx).is_some(),
        "the premise: the question is open"
    );
    start_work(&mut helm, cx, Work::Launch, OpKey::new_launch());
    click(cx, "prompt-confirm");
    settle(cx);
    assert!(helm.commands().is_empty(), "no creation was sent");
    assert!(sandbox_fields_open(cx));
    assert_eq!(sandbox_name(cx), "gpubox");
    assert_eq!(recipe_shown(cx).as_deref(), Some("rust-dev"));
    assert!(focus_in(cx, "add-tensorlake-name"));
    assert_eq!(
        super::tests::error_line(cx),
        Some(format!("Add host: {}", crate::hosts::LAUNCH_IN_FLIGHT))
    );
}

/// Why this matters: without asking, nobody presses Create sandbox, so the launch check at the submission is the only
/// one between a launch that started while the probe was out and a billed creation beside it.
///
/// Specified: with the account's `create_without_asking` on and a `Launch` started while the probe is out, the plan's
/// arrival sends no provision request and reopens Add host on the sandbox with the fields and the launch's reason.
#[gpui::test]
fn without_asking_a_plan_during_a_launch_is_refused_as_create_sandbox_is(cx: &mut TestAppContext) {
    let (mut helm, cx) = filled_form(cx, true, Vec::new());
    let (probe, _) = continue_to_probe(cx, &mut helm);
    start_work(&mut helm, cx, Work::Launch, OpKey::new_launch());
    helm.reply(probe, 200, Some(plan("probe-1")));
    settle(cx);
    assert!(helm.commands().is_empty(), "no creation was sent");
    assert!(sandbox_fields_open(cx));
    assert_eq!(sandbox_name(cx), "gpubox");
    assert_eq!(
        super::tests::error_line(cx),
        Some(format!("Add host: {}", crate::hosts::LAUNCH_IN_FLIGHT))
    );
}

/// Why this matters: what Continue refuses without asking the helm must say why with the fields kept, and Enter in
/// Name reaches the same refusals the button's unavailability hides.
///
/// Specified: Enter in an empty Name says "Enter a name for the host"; with a name and no recipe, "Choose a recipe";
/// with both and a `Launch` in flight, Continue says "Add host: not started, …". None of them sends a probe.
#[gpui::test]
fn continue_refuses_a_missing_field_or_a_launch_with_the_fields_kept(cx: &mut TestAppContext) {
    let (mut helm, cx, _) = open_rail_with(cx, vec![catalog(&[recipe("rust-dev", "team/devbox")])]);
    open_dialog(cx);
    let read = choose_tensorlake(cx, &mut helm);
    helm.reply(read, 200, Some(account(true)));
    cx.run_until_parked();
    keys(cx, "tab");
    assert!(
        focus_in(cx, "add-tensorlake-name"),
        "the premise: focus in Name"
    );
    keys(cx, "enter");
    assert_eq!(
        super::tests::error_line(cx).as_deref(),
        Some("Enter a name for the host")
    );
    type_text(cx, "gpubox");
    keys(cx, "enter");
    assert_eq!(
        super::tests::error_line(cx).as_deref(),
        Some("Choose a recipe")
    );
    click(cx, "add-tensorlake-recipe");
    click(cx, "add-tensorlake-recipe-option-rust-dev");
    start_work(&mut helm, cx, Work::Launch, OpKey::new_launch());
    click(cx, "add-tensorlake-continue");
    assert_eq!(
        super::tests::error_line(cx),
        Some(format!("Add host: {}", crate::hosts::LAUNCH_IN_FLIGHT))
    );
    assert!(helm.commands().is_empty(), "no probe was sent");
    assert_eq!(sandbox_name(cx), "gpubox");
}

/// Why this matters: a probe the helm refuses, or answers with something other than a Tensorlake plan, must say so
/// rather than open a question about nothing, and leave the fields for another try.
///
/// Specified: a 409 to the probe shows the helm's words in Add host's error line; an SSH plan in reply shows "The
/// helm did not return a Tensorlake creation plan"; Add host stays open with the name kept and no question opens.
#[gpui::test]
fn a_refused_probe_or_a_reply_that_is_no_sandbox_plan_says_why(cx: &mut TestAppContext) {
    let (mut helm, cx) = filled_form(cx, false, Vec::new());
    let (probe, _) = continue_to_probe(cx, &mut helm);
    helm.reply(
        probe,
        409,
        Some(json!("a host named gpubox already exists")),
    );
    settle(cx);
    assert_eq!(
        super::tests::error_line(cx).as_deref(),
        Some("a host named gpubox already exists")
    );
    let (probe, _) = continue_to_probe(cx, &mut helm);
    helm.reply(
        probe,
        200,
        Some(json!({
            "result": "provisionable", "probe_id": "p-1", "plan": {},
            "confirmation": "", "steps": { "lead": "", "kept": null, "steps": [] },
        })),
    );
    settle(cx);
    assert_eq!(
        super::tests::error_line(cx).as_deref(),
        Some("The helm did not return a Tensorlake creation plan")
    );
    assert!(sandbox_fields_open(cx) && question_title(cx).is_none());
    assert_eq!(sandbox_name(cx), "gpubox");
}

/// Why this matters: the helm can still refuse the plan (its recipe deleted while the question was open, a name taken
/// meanwhile), and the user must see why with what they typed intact.
///
/// Specified: a 409 to Create sandbox reopens Add host on the sandbox with `gpubox` and `rust-dev` kept, the helm's
/// words in its error line and focus in Name, and reads nothing more.
#[gpui::test]
fn a_refused_creation_reopens_add_host_with_the_fields_and_the_helms_words(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = filled_form(cx, false, Vec::new());
    confirm_plan(cx, &mut helm, "probe-1");
    click(cx, "prompt-confirm");
    let (create, _) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    helm.reply(create, 409, Some(json!(RECIPE_GONE)));
    settle(cx);
    assert!(sandbox_fields_open(cx), "back at the fields");
    assert_eq!(sandbox_name(cx), "gpubox");
    assert_eq!(recipe_shown(cx).as_deref(), Some("rust-dev"));
    assert!(focus_in(cx, "add-tensorlake-name"));
    assert_eq!(super::tests::error_line(cx).as_deref(), Some(RECIPE_GONE));
    assert!(helm.commands().is_empty(), "the account is not read again");
}

/// Why this matters: SPEC.md, "Keyboard focus": once the user has moved on, a result opens no dialog, and a refused
/// new host has no row, so the Hosts panel says it, as it does for a refused SSH setup.
///
/// Specified: after Create sandbox, a key press, then a 409, nothing opens and the panel's line reads "Add host: "
/// and the helm's words.
#[gpui::test]
fn a_creation_refused_after_the_user_moved_on_shows_on_the_hosts_panel(cx: &mut TestAppContext) {
    let (mut helm, cx) = filled_form(cx, false, Vec::new());
    confirm_plan(cx, &mut helm, "probe-1");
    click(cx, "prompt-confirm");
    let (create, _) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    keys(cx, "tab");
    helm.reply(create, 409, Some(json!(RECIPE_GONE)));
    settle(cx);
    assert!(!any_open(cx), "nothing opened");
    assert_eq!(
        label(cx, "hosts-add-error"),
        Some(format!("Add host: {RECIPE_GONE}"))
    );
}

/// Why this matters: a question the user did not just ask for must not open once they have moved on (the SSH setup
/// confirmation's rule); the dialog still open says to ask again.
///
/// Specified: Shift+Tab while the probe is out, then a plan, opens no question and sends nothing; Add host stays on
/// the sandbox saying to press Continue again.
#[gpui::test]
fn a_plan_after_the_user_moved_on_opens_no_question(cx: &mut TestAppContext) {
    let (mut helm, cx) = filled_form(cx, false, Vec::new());
    let (probe, _) = continue_to_probe(cx, &mut helm);
    keys(cx, "shift-tab");
    helm.reply(probe, 200, Some(plan("probe-1")));
    settle(cx);
    assert_eq!(question_title(cx), None, "no question");
    assert!(sandbox_fields_open(cx));
    assert_eq!(
        super::tests::error_line(cx).as_deref(),
        Some(super::tensorlake::MOVED_ON)
    );
    assert!(helm.commands().is_empty());
}

/// Why this matters: without asking, the plan goes out on the strength of the Continue, as an SSH plan does under "Set
/// up new hosts without asking"; whether the user moved on decides only where a refusal shows, never whether the
/// creation goes.
///
/// Specified: with the account's `create_without_asking` on, Shift+Tab while the probe is out, then a plan, still
/// sends the plan's `POST /api/hosts/provision` with Add host closed and no question.
#[gpui::test]
fn without_asking_the_plan_goes_out_after_the_user_moved_on(cx: &mut TestAppContext) {
    let (mut helm, cx) = filled_form(cx, true, Vec::new());
    let (probe, _) = continue_to_probe(cx, &mut helm);
    keys(cx, "shift-tab");
    helm.reply(probe, 200, Some(plan("probe-1")));
    settle(cx);
    let (_, body) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    assert_eq!(body, json!({ "probe_id": "probe-1" }));
    assert!(!any_open(cx), "Add host closed and no question opened");
}

/// Why this matters: "Create Tensorlake sandboxes without asking" is the one setting that lets a plan bill without a
/// question, and it is Tensorlake's own: the SSH setup preference must never stand in for it.
///
/// Specified: with the account's `create_without_asking` on, the plan is sent as it arrives, with Add host closed and
/// no question.
#[gpui::test]
fn only_the_tensorlake_setting_sends_the_plan_without_asking(cx: &mut TestAppContext) {
    let (mut helm, cx) = filled_form(cx, true, Vec::new());
    confirm_plan(cx, &mut helm, "probe-1");
    let (_, body) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    assert_eq!(body, json!({ "probe_id": "probe-1" }));
    assert!(!any_open(cx), "Add host closed and no question opened");
}

/// The SSH setup preference on, the Tensorlake setting off: [`only_the_tensorlake_setting_sends_the_plan_without_asking`]'s
/// other half, kept apart since it needs the preference row in the helm's first snapshot.
///
/// Specified: with "Set up new hosts without asking" on (asserted as the premise), the plan opens the question and
/// sends nothing.
#[gpui::test]
fn the_ssh_setup_preference_never_sends_a_sandbox_plan(cx: &mut TestAppContext) {
    let preferences = CollectionState {
        collection: Collection::Preferences,
        meta: meta(&[farhelm_proto::ui_channel::PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: farhelm_proto::ui_channel::PREFERENCES_KEY.into(),
            value: json!({ "skip_host_setup_confirmation": true }),
        }],
    };
    let (mut helm, cx) = filled_form(cx, false, vec![preferences]);
    assert!(
        cx.update(|_, cx| !crate::hosts::setup::asks_first(cx)),
        "the premise: the SSH setup preference is on"
    );
    confirm_plan(cx, &mut helm, "probe-1");
    assert!(
        helm.commands().is_empty(),
        "the plan waits for Create sandbox"
    );
    assert!(question_title(cx).is_some(), "the question is open");
}

/// Why this matters: Cancel and Escape must retire the plan for good, so that nothing bills from a question the user
/// dismissed (the old suite's Back, Cancel and Escape checks, and its "cancel retires a Tensorlake creation before a
/// queued answer").
///
/// Specified: Cancel on the question closes it with nothing open and nothing sent; so does Escape, on a second plan.
#[gpui::test]
fn cancel_and_escape_on_the_question_create_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx) = filled_form(cx, false, Vec::new());
    confirm_plan(cx, &mut helm, "probe-1");
    click(cx, "prompt-cancel");
    settle(cx);
    assert!(!any_open(cx));
    assert!(helm.commands().is_empty(), "Cancel created nothing");

    fill_form(cx, &mut helm, false);
    confirm_plan(cx, &mut helm, "probe-2");
    assert!(
        question_title(cx).is_some(),
        "the premise: the question is open"
    );
    keys(cx, "escape");
    settle(cx);
    assert!(!any_open(cx));
    assert!(helm.commands().is_empty(), "Escape created nothing");
}

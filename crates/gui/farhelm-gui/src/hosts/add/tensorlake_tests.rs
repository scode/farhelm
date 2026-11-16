//! Tests of Add host's host kind choice and its Tensorlake sandbox body ([`super::tensorlake`]), over the helm client's
//! fake, opened from the same stand-in rail as the SSH tests (`hosts/add/tests.rs`, whose helpers these share).

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{Collection, CollectionChanges, CollectionState, KeyedEntry};
use gpui::{App, TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::tests::{destination, open_dialog, open_rail_with, type_text};
use crate::test_support::{any_open, click, focused_id, frame, keys, label, meta, one_command};

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

/// Whether the sandbox body's Name holds focus.
fn name_focused(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| sandbox_form(cx).read(cx).name_focused(window, cx))
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
        name_focused(cx),
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
/// Specified: Recipe starts on "Choose a recipe" and offers the catalog's recipes in order; choosing one shows it and
/// its summary (size, base and packages, with the image's name as written); when a push takes it out of the catalog,
/// the select shows "<name> (unavailable)" and no summary.
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
            "add-tensorlake-recipe-option-web"
        ],
        "the catalog's recipes in order"
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

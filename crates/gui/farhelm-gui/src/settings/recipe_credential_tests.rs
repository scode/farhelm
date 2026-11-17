//! Tests of the recipe editor's credential choices over the helm client's fake: the catalog's read as the editor opens
//! and its Retry, the GitHub token and each agent's mode, endpoint and login pickers, a missing choice kept visible,
//! and Save sending the names chosen.
//!
//! The editor is opened as in its own tests (`super::recipe_editor_tests`), whose helpers these share. The pickers sit
//! below the dialog's fold, so they are reached the way a keyboard user reaches them: Tab, which scrolls each into view,
//! then Space to open a select's list, and a click on the option the list shows. Every step ends in one of the shared
//! harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::recipe_editor_tests::{
    draft, one_command, open_editor_with, show, type_into, with_credentials,
};
use super::recipe_tests::{entry, idle};
use super::recipes::draft::AgentMode;
use super::tests::{available, drawn, fill, label_of, reach, sent};
use crate::test_support::{click, frame, keys, settle};

/// A catalog entry as `GET /api/credentials` lists it: `name` of `kind`, used by no recipe.
fn credential(name: &str, kind: &str) -> Value {
    json!({ "name": name, "kind": kind, "created_at": 1_794_647_100, "updated_at": 1_794_647_100,
            "revision": 1, "used_by": [] })
}

/// A catalog with one credential of every kind, and a second Claude login, so each picker's options show which kinds
/// it offers.
fn catalog() -> Value {
    json!([
        credential("ci-token", "github_token"),
        credential("work-endpoint", "endpoint_token"),
        credential("claude-work", "claude_login"),
        credential("claude-home", "claude_login"),
        credential("codex-work", "codex_login"),
    ])
}

/// Tab to the select `id` and open its list.
fn open_list(cx: &mut VisualTestContext, id: &str) {
    reach(cx, id);
    keys(cx, "space");
}

/// The element ids of the options the open list shows, in order.
fn options(cx: &mut VisualTestContext) -> Vec<String> {
    frame(cx)
        .into_iter()
        .filter(|node| node.role == Some(gpui::Role::MenuListOption))
        .filter_map(|node| node.element_id.map(|id| id.to_string()))
        .collect()
}

/// What the closed select `id` shows as its value.
fn shown(cx: &mut VisualTestContext, id: &str) -> Option<String> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .and_then(|node| node.value.map(|value| value.to_string()))
}

/// Press Save and return the one command it sent, which must be the recipe's save: its body.
fn save(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Value {
    click(cx, "recipe-save");
    let save = one_command(helm);
    assert!(
        matches!(save.method, Method::Post | Method::Put),
        "a save: {save:?}"
    );
    save.body.unwrap_or_default()
}

/// The body's recipe: the body itself for an update, its `recipe` for a creation.
fn recipe(body: &Value) -> &Value {
    body.get("recipe").unwrap_or(body)
}

/// SPEC.md: "Recipes select API endpoint tokens, GitHub tokens and Claude or Codex subscription logins by name", and
/// "Subscription requires a named login for that agent", so each picker must offer exactly its kind; the old spec
/// checked the options ("None", then the names) and that a login picker admits only its own agent's logins. Specified:
/// with the catalog read, the GitHub picker offers None and `ci-token`; choosing it, Claude's API endpoint with a base
/// URL and `work-endpoint`, and Codex's Subscription, whose login picker offers None and `codex-work` only, then Save
/// sends those names in the recipe, each agent's other mode empty.
#[gpui::test]
fn each_picker_offers_its_kind_and_save_sends_the_names_chosen(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, Vec::new());
    open_editor_with(&mut helm, cx, "recipe-new", catalog());
    fill(cx, "recipe-name", "rust-dev");

    open_list(cx, "recipe-github");
    assert_eq!(
        options(cx),
        ["recipe-github-option-", "recipe-github-option-ci-token"],
        "None, then the GitHub tokens"
    );
    click(cx, "recipe-github-option-ci-token");

    reach(cx, "recipe-claude-mode-None");
    keys(cx, "right");
    assert!(
        drawn(cx, "recipe-claude-endpoint"),
        "API endpoint shows its fields"
    );
    type_into(cx, "recipe-claude-url", "http://127.0.0.1:8080");
    open_list(cx, "recipe-claude-token");
    assert_eq!(
        options(cx),
        [
            "recipe-claude-token-option-",
            "recipe-claude-token-option-work-endpoint"
        ]
    );
    click(cx, "recipe-claude-token-option-work-endpoint");

    reach(cx, "recipe-codex-mode-None");
    keys(cx, "right right");
    open_list(cx, "recipe-codex-login");
    assert_eq!(
        options(cx),
        [
            "recipe-codex-login-option-",
            "recipe-codex-login-option-codex-work"
        ],
        "Codex's logins only"
    );
    click(cx, "recipe-codex-login-option-codex-work");

    let body = save(&mut helm, cx);
    let recipe = recipe(&body);
    assert_eq!(recipe["github_token_credential"], json!("ci-token"));
    assert_eq!(
        recipe["claude"],
        json!({ "base_url": "http://127.0.0.1:8080", "token_credential": "work-endpoint" })
    );
    assert_eq!(recipe["claude_login_credential"], Value::Null);
    assert_eq!(recipe["codex"], Value::Null);
    assert_eq!(recipe["codex_login_credential"], json!("codex-work"));
}

/// SPEC.md: "Switching modes retains each draft but saves only the selected choice", so a user who looks at another
/// mode and comes back finds what they typed; Subscription without a login is refused before anything is sent; and
/// "Subscription requires a named login for that agent", which the old spec checked for Claude as for Codex. Specified:
/// Edit on a recipe whose Claude endpoint names a token; Subscription hides the endpoint's fields and shows the login
/// picker, which offers None and both Claude logins and nothing else; Save then sends nothing and says to choose a
/// Claude login; back on API endpoint the base URL and the token are as loaded, and the draft holds both modes' fields
/// throughout.
#[gpui::test]
fn switching_an_agents_mode_keeps_each_modes_draft(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, vec![with_credentials("rust-dev")]);
    open_editor_with(&mut helm, cx, "recipe-edit-rust-dev", catalog());
    assert_eq!(draft(cx).claude_mode, AgentMode::Endpoint, "the premise");

    reach(cx, "recipe-claude-mode-API endpoint");
    keys(cx, "right");
    assert!(!drawn(cx, "recipe-claude-endpoint"));
    open_list(cx, "recipe-claude-login");
    assert_eq!(
        options(cx),
        [
            "recipe-claude-login-option-",
            "recipe-claude-login-option-claude-work",
            "recipe-claude-login-option-claude-home"
        ],
        "Claude's logins only"
    );
    keys(cx, "escape");
    assert!(options(cx).is_empty(), "Escape closed the list");
    click(cx, "recipe-save");
    assert_eq!(sent(&mut helm), [], "a local refusal sends nothing");
    assert_eq!(
        label_of(cx, "recipe-editor-error").as_deref(),
        Some("Choose a Claude login in Subscription mode")
    );
    assert_eq!(draft(cx).claude.base_url, "http://127.0.0.1:8080", "kept");

    reach(cx, "recipe-claude-mode-Subscription");
    keys(cx, "left");
    assert!(drawn(cx, "recipe-claude-endpoint"));
    assert_eq!(
        shown(cx, "recipe-claude-token").as_deref(),
        Some("work-endpoint")
    );
    assert_eq!(draft(cx).claude.token_credential, "work-endpoint");
}

/// SPEC.md: "An unavailable draft choice stays visible until the user chooses another or None; it cannot be saved as a
/// missing reference" — showing None instead would hide that the saved recipe names something gone, and quietly
/// submitting the old name would be refused without the user seeing why. Specified: Edit on a recipe whose GitHub token
/// the catalog lacks shows it as "<name> (unavailable)" with the old line saying to choose another or None; choosing
/// None drops the line, and Save then sends no GitHub token.
#[gpui::test]
fn a_missing_choice_stays_visible_until_another_is_chosen(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, vec![with_credentials("rust-dev")]);
    open_editor_with(&mut helm, cx, "recipe-edit-rust-dev", json!([]));
    assert_eq!(
        shown(cx, "recipe-github").as_deref(),
        Some("ci-token (unavailable)")
    );
    assert_eq!(
        label_of(cx, "recipe-github-missing").as_deref(),
        Some("The selected GitHub credential is unavailable. Choose another or None.")
    );

    open_list(cx, "recipe-github");
    click(cx, "recipe-github-option-");
    assert!(!drawn(cx, "recipe-github-missing"));
    assert_eq!(
        recipe(&save(&mut helm, cx))["github_token_credential"],
        Value::Null
    );
}

/// The catalog is not pushed, so the editor reads it once as it opens; until it answers the pickers cannot be
/// trusted to offer every name, and a failed read must not look like an empty catalog. The old editor said
/// "Loading credentials…", showed the reason with Retry loading credentials, and kept its pickers unavailable until a
/// read answered. Specified: before the read answers, "Loading credentials…" shows and the GitHub picker is unavailable;
/// a refused read shows the helm's words and Retry loading credentials, the picker still unavailable; Retry sends the
/// read once more, and its answer drops the reason and makes the picker available.
#[gpui::test]
fn the_catalog_is_read_as_the_editor_opens_and_retry_reads_it_again(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, vec![entry("rust-dev", idle(), Value::Null)]);
    click(cx, "recipe-edit-rust-dev");
    let read = one_command(&mut helm);
    assert_eq!(read.path, "/api/credentials");
    assert!(drawn(cx, "recipe-credentials-loading"));
    assert_eq!(available(cx, "recipe-github"), Some(false));

    helm.reply(read.id, 500, Some(json!("could not read credentials")));
    settle(cx);
    assert_eq!(
        label_of(cx, "recipe-credentials-error").as_deref(),
        Some("could not read credentials")
    );
    assert!(!drawn(cx, "recipe-credentials-loading"));
    assert_eq!(available(cx, "recipe-github"), Some(false));

    reach(cx, "recipe-credentials-retry");
    keys(cx, "space");
    let again = one_command(&mut helm);
    assert_eq!(
        (again.method, again.path.as_str()),
        (Method::Get, "/api/credentials")
    );
    helm.reply(again.id, 200, Some(catalog()));
    settle(cx);
    assert!(!drawn(cx, "recipe-credentials-error"));
    assert_eq!(available(cx, "recipe-github"), Some(true));
}

/// Why this matters: the pickers list only credentials already saved, so the editor must say where to add or replace
/// one, or a user whose token is not listed has no way on from the form; and it must say that the recipe keeps names,
/// not values, since the secret itself never comes back to the GUI. The old spec `tensorlake-recipes.spec.ts`, "a
/// managed recipe is created, edited and deleted without exposing its token", checked the endpoint tokens' line beside
/// the picker. Specified: with a recipe whose Claude endpoint names a token open in the editor, the painted text holds
/// the GitHub block's "Add or replace tokens in Settings → Credentials." and the Agents block's "Add or replace
/// endpoint tokens in Settings → Credentials. This recipe saves their names, not their values."
#[gpui::test]
fn the_editor_says_where_credentials_are_added_and_that_it_keeps_names(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, vec![with_credentials("rust-dev")]);
    open_editor_with(&mut helm, cx, "recipe-edit-rust-dev", catalog());
    assert!(
        drawn(cx, "recipe-claude-endpoint"),
        "the premise: Claude's endpoint shows"
    );
    let text = super::tests::painted(cx);
    assert!(
        text.contains("Add or replace tokens in Settings → Credentials."),
        "{text}"
    );
    assert!(
        text.contains(
            "Add or replace endpoint tokens in Settings → Credentials. This recipe saves their names, not their values."
        ),
        "{text}"
    );
}

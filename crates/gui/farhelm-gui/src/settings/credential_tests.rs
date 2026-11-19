//! Tests of Settings → Credentials over the helm client's fake: the catalog's read and its three lists, the in-place
//! editor's saves, refusals, checks and Delete, delivery failures after a done replacement, the Codex login's agent
//! choice and file, the storage warnings, and the Tensorlake API key form.
//!
//! The dialog is opened from the gear as in the dialog's own tests (`super::tests`), whose helpers these share. Every
//! step ends in one of the shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::Preferences;
use gpui::{Entity, TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::Section;
use super::credentials::Editor;
use super::tests::{
    SELECT_ALL, available, drawn, fill, label_of, open_rail, open_settings, painted, reach,
    section_button, sent,
};
use crate::layers;
use crate::local_time;
use crate::operations::{OpKey, Work};
use crate::session;
use crate::test_support::{click, focused_id, keys, settle};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// A catalog entry as `GET /api/credentials` lists it: `name` of `kind`, saved once (`revision` 1) or replaced, at
/// 2026-11-14 09:05:00 UTC, used by `recipes`.
fn entry(name: &str, kind: &str, revision: u64, recipes: &[&str]) -> Value {
    json!({ "name": name, "kind": kind, "created_at": 1_794_647_100, "updated_at": 1_794_647_100,
            "revision": revision, "used_by": recipes })
}

/// The helm's Tensorlake settings as `GET /api/tensorlake` answers them, with or without a saved key.
fn account(key_set: bool) -> Value {
    json!({ "api_key_set": key_set, "create_without_asking": false, "helm_tag": "fixture",
            "ssh_key": { "state": "not_created" } })
}

/// The reads showing Credentials for the first time in an opening sends, by path, sorted: the catalog, and the account
/// the Tensorlake API key form shows.
const SHOWN_READS: &[&str] = &["/api/credentials", "/api/tensorlake"];

/// Answer `reads`, the section's reads, by path: the catalog with `catalog`, the Tensorlake account with no key saved.
fn answer(helm: &mut FakeHelm, reads: Vec<FakeCommand>, catalog: &Value) {
    for read in reads {
        let answer = match read.path.as_str() {
            "/api/credentials" => catalog.clone(),
            "/api/tensorlake" => account(false),
            other => panic!("an unexpected read of {other}"),
        };
        helm.reply(read.id, 200, Some(answer));
    }
}

/// Open Settings, show Credentials, and answer its reads with `catalog` (see [`answer`]). Checks that showing the
/// section sends exactly [`SHOWN_READS`].
fn show_credentials(helm: &mut FakeHelm, cx: &mut VisualTestContext, catalog: Value) {
    open_settings(cx);
    assert_eq!(sent(helm), [], "the premise: Sessions reads nothing");
    click(cx, section_button(Section::Credentials));
    let reads = sent(helm);
    let mut paths: Vec<&str> = reads.iter().map(|read| read.path.as_str()).collect();
    paths.sort();
    assert_eq!(paths, SHOWN_READS, "each read once");
    assert!(
        reads.iter().all(|read| read.method == Method::Get),
        "only reads"
    );
    answer(helm, reads, &catalog);
    settle(cx);
}

/// The one command sent since the last call: its id, method, path and body.
fn one_command(helm: &mut FakeHelm) -> (u64, Method, String, Value) {
    let sent = sent(helm);
    assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
    let command = sent.into_iter().next().unwrap();
    (
        command.id,
        command.method,
        command.path,
        command.body.unwrap_or_default(),
    )
}

/// Press the control with element id `id` the way a keyboard user does: Tab to it, which scrolls it into the dialog's
/// view, and Space. The section is taller than the dialog's body, so a click cannot reach its lower controls.
fn press(cx: &mut VisualTestContext, id: &str) {
    reach(cx, id);
    keys(cx, "space");
}

/// Choose Codex in a new agent login's agent choice the way a keyboard user does: Tab to the chosen option, Claude Code,
/// which is the group's one tab stop, and Right, which moves the choice with focus, as native radios do.
fn choose_codex(cx: &mut VisualTestContext) {
    reach(cx, "credential-agent-Claude Code");
    keys(cx, "right");
}

/// Whether focus is where the open editor starts it: on Name for a new credential, on the secret for a saved one.
/// GPUI Kit's fields carry no element id of their own, so the editor's handle is asked.
fn editor_start_focused(cx: &mut VisualTestContext) -> bool {
    let editor = editor(cx);
    cx.update(|window, cx| editor.read(cx).start(cx).is_focused(window))
}

/// The open credential editor.
fn editor(cx: &mut VisualTestContext) -> Entity<Editor> {
    cx.update(|_, cx| {
        let dialog = layers::last_opened::<super::SettingsDialog>(cx).expect("Settings is open");
        let credentials = dialog
            .read(cx)
            .credentials
            .clone()
            .expect("Credentials was shown");
        credentials.read(cx).editor().expect("an editor is open")
    })
}

/// The key the Tensorlake API key form's draft holds. The field is masked, so what it paints cannot show it.
fn key_typed(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| {
        let dialog = layers::last_opened::<super::SettingsDialog>(cx).expect("Settings is open");
        let credentials = dialog
            .read(cx)
            .credentials
            .clone()
            .expect("Credentials was shown");
        let key = credentials.read(cx).key();
        key.read(cx).typed()
    })
}

/// The open editor's secret: the paste field's text and the value the editor would save.
fn secret(cx: &mut VisualTestContext) -> (String, String) {
    let editor = editor(cx);
    cx.update(|_, cx| editor.read(cx).secret(cx))
}

/// The work in flight on the credential `name`.
fn credential_work(cx: &mut VisualTestContext, name: &str) -> Option<Work> {
    cx.update(|_, cx| {
        session::current_operations(cx)
            .and_then(|operations| operations.read(cx).in_flight(&OpKey::credential(name)))
    })
}

// ---- The overview --------------------------------------------------------------------------------------------------

/// The catalog is helm state the helm does not push, so the section reads it each time it shows (coming back from
/// another section must show current recipe use), and must not offer Add before it knows the names already taken (the
/// old Add was unavailable until the list loaded). SPEC.md: "Lists show the name, recipe use and saved or replaced
/// time", and agent logins of either agent share one list. Specified: showing Credentials sends the section's reads,
/// one catalog read among them; before the catalog answers, Add GitHub token is unavailable and "Loading credentials…"
/// shows; once answered, each entry is drawn as a row in its kind's list with its use and its time to the minute in the
/// viewer's local time (the app's zone pinned five and a half hours east of UTC here, with no zone named), Saved for a
/// first revision and Replaced for a later one; a list with no entries says "None saved."; showing Sessions and
/// Credentials again reads the catalog once more and nothing else.
#[gpui::test]
fn credentials_reads_its_catalog_whenever_shown_and_lists_each_entry_in_its_kinds_list(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    cx.update(|_, cx| local_time::pin(5 * 3_600 + 1_800, cx));
    open_settings(cx);
    click(cx, section_button(Section::Credentials));
    let reads = sent(&mut helm);
    assert_eq!(
        reads.len(),
        SHOWN_READS.len(),
        "the section's reads: {reads:?}"
    );
    assert_eq!(
        available(cx, "credentials-add-github"),
        Some(false),
        "Add waits for the list"
    );
    assert!(drawn(cx, "credentials-loading"));
    let catalog = json!([
        entry("ci-token", "github_token", 1, &["build"]),
        entry("claude-login", "claude_login", 2, &[]),
        entry("codex-login", "codex_login", 1, &[]),
    ]);
    answer(&mut helm, reads, &catalog);
    settle(cx);

    assert_eq!(available(cx, "credentials-add-github"), Some(true));
    assert!(!drawn(cx, "credentials-loading"));
    let text = painted(cx);
    assert!(
        text.contains("ci-token Used by recipes: build Saved 2026-11-14, 14:35"),
        "{text}"
    );
    assert!(
        text.contains("claude-login No recipes use this credential. Replaced 2026-11-14, 14:35"),
        "{text}"
    );
    assert!(
        text.contains("codex-login"),
        "both agents in one list: {text}"
    );
    assert!(
        text.contains("API endpoint tokens Add endpoint token None saved."),
        "{text}"
    );
    assert!(
        text.find("ci-token").unwrap() < text.find("Agent subscription logins").unwrap(),
        "the GitHub token is in its own list: {text}"
    );

    click(cx, section_button(Section::Sessions));
    click(cx, section_button(Section::Credentials));
    let again: Vec<(Method, String)> = sent(&mut helm)
        .into_iter()
        .map(|read| (read.method, read.path))
        .collect();
    assert_eq!(
        again,
        [(Method::Get, "/api/credentials".to_owned())],
        "the catalog alone, read again"
    );
}

/// SPEC.md: "The Credentials overview and every credential editor each show one prominent danger-tinted Not encrypted
/// on disk warning", and "The overview's warning names the Tensorlake API key, so the key form there shows none of its
/// own". A second warning, or one that forgot the key, would misstate what is stored in plain text. Specified: the
/// overview draws one warning, naming the Tensorlake API key; the editor draws one, without it, and the overview's is
/// gone while the editor shows.
#[gpui::test]
fn the_overview_and_the_editor_each_show_one_storage_warning(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_credentials(&mut helm, cx, json!([]));
    let overview = label_of(cx, "credentials-storage-warning").expect("the overview's warning");
    assert!(overview.starts_with("Not encrypted on disk."), "{overview}");
    assert!(
        overview.contains("including the Tensorlake API key"),
        "{overview}"
    );
    assert_eq!(painted(cx).matches("Not encrypted on disk").count(), 1);

    press(cx, "credentials-add-github");
    assert!(!drawn(cx, "credentials-storage-warning"));
    let editor = label_of(cx, "credential-storage-warning").expect("the editor's warning");
    assert!(!editor.contains("Tensorlake"), "{editor}");
    assert_eq!(painted(cx).matches("Not encrypted on disk").count(), 1);
}

/// A failed read must not look like an empty catalog, or the user would believe nothing is saved and try to add a name
/// that exists (the old "Keep a failed read distinct from an empty catalog so the editor can offer Retry"). Specified:
/// a refused catalog read shows the helm's words as an alert with Retry loading, and no list says "None saved."; Retry
/// sends the read again, and its answer lists the rows and drops the alert.
#[gpui::test]
fn a_failed_catalog_read_shows_its_reason_and_retry_reads_again(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, section_button(Section::Credentials));
    for read in sent(&mut helm) {
        if read.path == "/api/credentials" {
            helm.reply(read.id, 500, Some(json!("could not read credentials")));
        } else {
            answer(&mut helm, vec![read], &json!([]));
        }
    }
    settle(cx);
    assert_eq!(
        label_of(cx, "credentials-load-error").as_deref(),
        Some("could not read credentials")
    );
    assert!(!painted(cx).contains("None saved."));

    press(cx, "credentials-retry");
    let (id, method, path, _) = one_command(&mut helm);
    assert_eq!((method, path.as_str()), (Method::Get, "/api/credentials"));
    helm.reply(
        id,
        200,
        Some(json!([entry("ci-token", "github_token", 1, &[])])),
    );
    settle(cx);
    assert!(!drawn(cx, "credentials-load-error"));
    assert!(painted(cx).contains("ci-token"));
}

// ---- The editor ----------------------------------------------------------------------------------------------------

/// Creation fixes a credential's name and kind, so Save must send all three in one creation, under the key of the
/// credential it creates, and a done save must show the new entry, which only a fresh read can (the catalog is not
/// pushed). Specified: Add GitHub token opens "New GitHub token" with focus on Name; Save is unavailable until a token
/// is typed; Save sends `POST /api/credentials` with the name, `github_token` and the token, as work on that name's key,
/// and shows "Saving…"; the helm's done answer closes the editor, puts focus back on Add GitHub token and reads the
/// catalog again, whose answer lists the new name.
#[gpui::test]
fn a_new_credential_saves_its_name_kind_and_secret_and_the_list_reads_again(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_credentials(&mut helm, cx, json!([]));
    press(cx, "credentials-add-github");
    assert_eq!(
        label_of(cx, "credential-editor-title").as_deref(),
        Some("New GitHub token")
    );
    assert!(editor_start_focused(cx), "focus starts on Name");
    fill(cx, "credential-name", "ci-token");
    assert_eq!(
        available(cx, "credential-save"),
        Some(false),
        "nothing to save yet"
    );
    fill(cx, "credential-secret", "fictional-token");

    reach(cx, "credential-save");
    keys(cx, "space");
    let (id, method, path, body) = one_command(&mut helm);
    assert_eq!((method, path.as_str()), (Method::Post, "/api/credentials"));
    assert_eq!(
        body,
        json!({ "name": "ci-token", "kind": "github_token", "secret": "fictional-token" })
    );
    assert_eq!(
        credential_work(cx, "ci-token"),
        Some(Work::Save),
        "under the credential's own key"
    );
    assert!(drawn(cx, "credential-saving"));

    helm.reply(id, 201, Some(entry("ci-token", "github_token", 1, &[])));
    settle(cx);
    assert!(
        !drawn(cx, "credential-editor"),
        "a done save closes the editor"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("credentials-add-github"));
    let (id, method, path, _) = one_command(&mut helm);
    assert_eq!(
        (method, path.as_str()),
        (Method::Get, "/api/credentials"),
        "read again"
    );
    helm.reply(
        id,
        200,
        Some(json!([entry("ci-token", "github_token", 1, &[])])),
    );
    settle(cx);
    assert!(painted(cx).contains("ci-token"));
}

/// SPEC.md: "Settings never returns a saved secret", the name and kind cannot change, "An empty replacement keeps the
/// saved value", and "A successful save with failed deliveries closes and clears the secret editor, and shows the
/// delivery failures on the Credentials overview". Specified: Edit shows the name read-only and an empty secret; Save
/// sends `PUT /api/credentials/ci-token` with only the typed secret; an answer listing a host it did not reach closes
/// the editor and shows "Saved; not delivered to Example host: could not install the agent settings"; a second
/// replacement left blank sends an empty secret (the helm keeps the saved value), and its answer with no failures
/// clears the line; with the line shown again, Edit and Back clear it too.
#[gpui::test]
fn a_replacement_sends_only_the_secret_and_shows_the_hosts_it_did_not_reach(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let saved = entry("ci-token", "github_token", 1, &[]);
    show_credentials(&mut helm, cx, json!([saved.clone()]));
    press(cx, "credential-edit-ci-token");
    assert_eq!(
        label_of(cx, "credential-editor-title").as_deref(),
        Some("GitHub token: ci-token")
    );
    assert!(
        editor_start_focused(cx),
        "the name is fixed, so focus is on the secret"
    );
    assert_eq!(
        secret(cx),
        (String::new(), String::new()),
        "the saved secret is never shown"
    );
    fill(cx, "credential-name", "renamed");
    assert!(!painted(cx).contains("renamed"), "the name is read-only");

    fill(cx, "credential-secret", "fictional-replacement");
    reach(cx, "credential-save");
    keys(cx, "space");
    let (id, method, path, body) = one_command(&mut helm);
    assert_eq!(
        (method, path.as_str()),
        (Method::Put, "/api/credentials/ci-token")
    );
    assert_eq!(body, json!({ "secret": "fictional-replacement" }));
    let replaced = entry("ci-token", "github_token", 2, &[]);
    helm.reply(
        id,
        200,
        Some(json!({ "credential": replaced, "delivery_failures": [
            { "host": "Example host", "reason": "could not install the agent settings" }] })),
    );
    settle(cx);
    assert!(!drawn(cx, "credential-editor"));
    assert_eq!(
        label_of(cx, "credentials-undelivered-0").as_deref(),
        Some("Saved; not delivered to Example host: could not install the agent settings")
    );
    let (id, _, _, _) = one_command(&mut helm);
    helm.reply(id, 200, Some(json!([replaced.clone()])));
    settle(cx);

    press(cx, "credential-edit-ci-token");
    reach(cx, "credential-save");
    keys(cx, "space");
    let (id, _, _, body) = one_command(&mut helm);
    assert_eq!(
        body,
        json!({ "secret": "" }),
        "a blank replacement keeps the saved value"
    );
    helm.reply(
        id,
        200,
        Some(json!({ "credential": replaced, "delivery_failures": [] })),
    );
    settle(cx);
    assert!(
        !drawn(cx, "credentials-undelivered-0"),
        "a delivered save clears the line"
    );
    let (id, _, _, _) = one_command(&mut helm);
    helm.reply(id, 200, Some(json!([replaced.clone()])));
    settle(cx);

    // Lines shown again, then Edit and Back: closing an editor clears them, as the old app's Back did.
    press(cx, "credential-edit-ci-token");
    reach(cx, "credential-save");
    keys(cx, "space");
    let (id, _, _, _) = one_command(&mut helm);
    helm.reply(
        id,
        200,
        Some(
            json!({ "credential": replaced.clone(), "delivery_failures": [
            { "host": "Example host", "reason": "could not install the agent settings" }] }),
        ),
    );
    settle(cx);
    assert!(
        drawn(cx, "credentials-undelivered-0"),
        "the premise: the line shows again"
    );
    let (id, _, _, _) = one_command(&mut helm);
    helm.reply(id, 200, Some(json!([replaced.clone()])));
    settle(cx);
    press(cx, "credential-edit-ci-token");
    press(cx, "credential-back");
    let (id, _, _, _) = one_command(&mut helm);
    helm.reply(id, 200, Some(json!([replaced])));
    settle(cx);
    assert!(
        !drawn(cx, "credentials-undelivered-0"),
        "Back cleared the line"
    );
}

/// SPEC.md: "a failed save keeps the draft and shows its reason", and the old editor showed the helm's words in place.
/// A refused save treated as saved would lose a secret the user cannot read back. Specified: a creation the helm refuses
/// (409, the name taken) leaves the editor open with the helm's words as an alert and the typed secret in the field and
/// the draft, nothing read again; Save sends the same body again.
#[gpui::test]
fn a_refused_save_keeps_the_draft_with_the_helms_words(cx: &mut TestAppContext) {
    let refusal = "a credential named ci-token already exists";
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_credentials(&mut helm, cx, json!([]));
    press(cx, "credentials-add-github");
    fill(cx, "credential-name", "ci-token");
    fill(cx, "credential-secret", "fictional-token");
    reach(cx, "credential-save");
    keys(cx, "space");
    let (id, _, _, body) = one_command(&mut helm);
    helm.reply(id, 409, Some(json!(refusal)));
    settle(cx);

    assert!(drawn(cx, "credential-editor"), "the editor stays");
    assert_eq!(label_of(cx, "credential-error").as_deref(), Some(refusal));
    assert_eq!(
        secret(cx),
        ("fictional-token".to_owned(), "fictional-token".to_owned())
    );
    assert_eq!(sent(&mut helm), [], "nothing read again");
    keys(cx, "space");
    let (_, _, _, again) = one_command(&mut helm);
    assert_eq!(again, body, "the same draft is sent again");
}

/// The old editor checked the name's grammar and the secret's shape before sending, and SPEC.md has Codex accept "a
/// JSON object of at most 64 KiB" only. Sending a draft the helm must refuse would waste a request and, for a secret,
/// put it on the wire for nothing. Specified: a name with capitals sends nothing and says the grammar's rule; a valid
/// name with a Codex login that is not a JSON object sends nothing and says what a Codex login must be; an edit clears
/// the line.
#[gpui::test]
fn the_editor_checks_the_name_and_the_secret_before_sending(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_credentials(&mut helm, cx, json!([]));
    press(cx, "credentials-add-agent-login");
    choose_codex(cx);
    fill(cx, "credential-name", "Bad_Name");
    fill(cx, "credential-secret", "[1, 2]");
    reach(cx, "credential-save");
    keys(cx, "space");
    assert_eq!(sent(&mut helm), [], "a malformed name is not sent");
    assert!(
        label_of(cx, "credential-error")
            .unwrap()
            .starts_with("Use lowercase letters")
    );

    // Save's reach scrolled Name out of view, so Shift+Tab back to it, which scrolls it in, and retype it.
    for _ in 0..10 {
        if editor_start_focused(cx) {
            break;
        }
        keys(cx, "shift-tab");
    }
    assert!(editor_start_focused(cx), "the premise: Name holds focus");
    keys(cx, SELECT_ALL);
    cx.simulate_input("codex-login");
    assert!(!drawn(cx, "credential-error"), "an edit clears the line");
    reach(cx, "credential-save");
    keys(cx, "space");
    assert_eq!(sent(&mut helm), [], "a malformed login is not sent");
    assert_eq!(
        label_of(cx, "credential-error").as_deref(),
        Some("a Codex login must be a JSON object of at most 64 KiB")
    );
}

/// SPEC.md: "Delete is unavailable while a recipe uses a credential, with the recipe names shown", and a deleted
/// credential must leave the list, which only a fresh read shows. Specified: the editor of a credential a recipe uses
/// names the recipe, says why Delete is unavailable, and its Delete sends nothing; the editor of an unused one sends
/// `DELETE /api/credentials/<name>` under its key, and the done answer closes the editor and reads the catalog again;
/// a refused Delete keeps the editor with the helm's words.
#[gpui::test]
fn delete_is_unavailable_while_a_recipe_uses_the_credential(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let catalog = json!([
        entry("used", "endpoint_token", 1, &["inference"]),
        entry("free", "endpoint_token", 1, &[])
    ]);
    show_credentials(&mut helm, cx, catalog.clone());
    press(cx, "credential-edit-used");
    // Tab to Cancel scrolls the editor's foot, where the use and the reason show, into view.
    reach(cx, "credential-cancel");
    assert!(painted(cx).contains("Used by recipes: inference"));
    assert!(painted(cx).contains("Delete is unavailable while a recipe uses this credential."));
    assert_eq!(available(cx, "credential-delete"), Some(false));
    press(cx, "credential-back");
    let reads = sent(&mut helm);
    answer(&mut helm, reads, &catalog);
    settle(cx);

    press(cx, "credential-edit-free");
    reach(cx, "credential-delete");
    keys(cx, "space");
    let (id, method, path, _) = one_command(&mut helm);
    assert_eq!(
        (method, path.as_str()),
        (Method::Delete, "/api/credentials/free")
    );
    assert_eq!(credential_work(cx, "free"), Some(Work::Save));
    helm.reply(id, 409, Some(json!("a recipe uses this credential")));
    settle(cx);
    assert_eq!(
        label_of(cx, "credential-error").as_deref(),
        Some("a recipe uses this credential")
    );

    keys(cx, "space");
    let (id, _, _, _) = one_command(&mut helm);
    helm.reply(id, 200, Some(json!({})));
    settle(cx);
    assert!(!drawn(cx, "credential-editor"));
    let (_, method, path, _) = one_command(&mut helm);
    assert_eq!(
        (method, path.as_str()),
        (Method::Get, "/api/credentials"),
        "read again"
    );
}

/// SPEC.md: "Closing discards unsaved text without a recipe-style departure question", and Back returns to the
/// overview the same way. A draft that survived would put a secret back in front of the next person to press Add.
/// Specified: Back closes the editor with no question and nothing saved (only the catalog is read again, as the old
/// editor's Back reloaded it), focus on its list's Add; Add again shows empty fields; Cancel does the same; closing
/// Settings and opening it again shows the overview.
#[gpui::test]
fn back_cancel_and_closing_discard_the_draft_without_asking(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_credentials(&mut helm, cx, json!([]));
    press(cx, "credentials-add-endpoint");
    fill(cx, "credential-secret", "fictional-token");
    press(cx, "credential-back");
    assert!(!drawn(cx, "credential-editor"));
    assert_eq!(focused_id(cx).as_deref(), Some("credentials-add-endpoint"));
    press(cx, "credentials-add-endpoint");
    assert_eq!(
        secret(cx),
        (String::new(), String::new()),
        "a new editor starts empty"
    );
    fill(cx, "credential-secret", "fictional-token");
    press(cx, "credential-cancel");
    assert!(!drawn(cx, "credential-editor"));
    press(cx, "credentials-add-endpoint");
    keys(cx, "escape");
    let reads = sent(&mut helm);
    let sent: Vec<(Method, String)> = reads
        .iter()
        .map(|command| (command.method, command.path.clone()))
        .collect();
    let catalog_read = (Method::Get, "/api/credentials".to_owned());
    assert_eq!(
        sent,
        [catalog_read.clone(), catalog_read],
        "nothing was saved: Back and Cancel each read the catalog again"
    );
    answer(&mut helm, reads, &json!([]));
    settle(cx);
    show_credentials(&mut helm, cx, json!([]));
    assert!(
        !drawn(cx, "credential-editor"),
        "a new opening shows the overview"
    );
}

/// A screen share of the Codex editor can show which file is staged without revealing the account data. Choose shows
/// the filename and shape check without sending a command. An unreadable replacement keeps the selection. Clear
/// restores the paste box, where an edit supersedes an outstanding choice. Save sends the selected file's contents.
/// The agent choice also switches from Claude Code's setup-token help to Codex's shared refresh token warning.
#[gpui::test]
fn a_chosen_codex_login_is_hidden_and_saved_only_on_submit(cx: &mut TestAppContext) {
    const FILE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/credentials/example-auth.json"
    );
    const CONTENTS: &str = include_str!("../../tests/fixtures/credentials/example-auth.json");
    const PRIVATE_MARKER: &str = "fictional-credential-contents";

    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_credentials(&mut helm, cx, json!([]));
    press(cx, "credentials-add-agent-login");
    assert!(painted(cx).contains("Run claude setup-token"));
    assert!(!drawn(cx, "credential-choose-file"));
    choose_codex(cx);
    reach(cx, "credential-choose-file");
    let text = painted(cx);
    assert!(
        text.contains(
            "when one copy renews it, other copies cannot renew and need a fresh sign-in"
        ),
        "{text}"
    );
    assert!(
        text.contains("prefer a proxy or load balancer through an API endpoint"),
        "{text}"
    );
    assert!(text.contains("provider's terms of service"), "{text}");

    assert!(CONTENTS.contains(PRIVATE_MARKER));
    assert!(drawn(cx, "credential-secret"));
    press(cx, "credential-choose-file");
    assert!(
        cx.did_prompt_for_paths(),
        "the platform's file dialog opened"
    );
    cx.simulate_path_prompt_response(|_| Some(vec![std::path::PathBuf::from(FILE)]));
    settle(cx);
    assert_eq!(
        secret(cx),
        (String::new(), CONTENTS.to_owned()),
        "only the value held for Save contains the file"
    );
    let text = painted(cx);
    assert!(text.contains("example-auth.json"));
    assert!(text.contains("Validated"));
    assert!(!text.contains(PRIVATE_MARKER));
    assert!(!drawn(cx, "credential-secret"));
    assert_eq!(available(cx, "credential-clear-file"), Some(true));
    assert_eq!(sent(&mut helm), [], "choosing saves nothing");

    press(cx, "credential-choose-file");
    cx.simulate_path_prompt_response(|_| {
        Some(vec![
            std::path::PathBuf::from(FILE).join("unreadable-child"),
        ])
    });
    settle(cx);
    assert_eq!(
        label_of(cx, "credential-error").as_deref(),
        Some("Could not read that login file.")
    );
    assert_eq!(secret(cx).1, CONTENTS, "the earlier selection is kept");

    press(cx, "credential-clear-file");
    assert!(drawn(cx, "credential-secret"));
    assert!(!drawn(cx, "credential-clear-file"));
    assert_eq!(secret(cx), (String::new(), String::new()));

    const PASTE: &str = "{\"pasted\": true}";
    fill(cx, "credential-secret", PASTE);
    assert_eq!(secret(cx).1, PASTE);
    press(cx, "credential-choose-file");
    assert!(cx.did_prompt_for_paths());
    // An edit must win even if comparing the values before and after the chooser would say nothing changed.
    fill(cx, "credential-secret", "{}");
    fill(cx, "credential-secret", PASTE);
    cx.simulate_path_prompt_response(|_| Some(vec![std::path::PathBuf::from(FILE)]));
    settle(cx);
    assert_eq!(
        secret(cx).1,
        PASTE,
        "the typed text wins over the file chosen meanwhile"
    );
    assert!(painted(cx).contains(PASTE));
    assert!(!drawn(cx, "credential-clear-file"));

    press(cx, "credential-choose-file");
    cx.simulate_path_prompt_response(|_| Some(vec![std::path::PathBuf::from(FILE)]));
    settle(cx);
    assert!(
        drawn(cx, "credential-clear-file"),
        "the file is staged again"
    );
    fill(cx, "credential-name", "codex-login");
    press(cx, "credential-save");
    let (_, method, path, body) = one_command(&mut helm);
    assert_eq!((method, path.as_str()), (Method::Post, "/api/credentials"));
    assert_eq!(
        body,
        json!({ "name": "codex-login", "kind": "codex_login", "secret": CONTENTS })
    );
    assert!(!painted(cx).contains(PRIVATE_MARKER));
}

/// A Delete's reply can arrive after the user went Back and opened another editor; closing whatever editor is open
/// then would throw away a secret the user is typing (the old editor's `mounted` check dropped such a reply).
/// Specified: Delete of `free` is out, Back, Add endpoint token and a typed secret; the Delete's done reply leaves the
/// new editor open with its secret, and the catalog is read again.
#[gpui::test]
fn a_delete_answered_after_back_leaves_a_newer_editor_open(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let catalog = json!([entry("free", "endpoint_token", 1, &[])]);
    show_credentials(&mut helm, cx, catalog.clone());
    press(cx, "credential-edit-free");
    press(cx, "credential-delete");
    let (delete, method, _, _) = one_command(&mut helm);
    assert_eq!(method, Method::Delete, "the premise: the Delete is out");
    press(cx, "credential-back");
    // Back's read goes out after the Delete, so the helm answers the Delete first, whenever it answers.
    let back_reads = sent(&mut helm);
    press(cx, "credentials-add-endpoint");
    fill(cx, "credential-secret", "fictional-typed-secret");

    helm.reply(delete, 200, Some(json!({})));
    // The helm runs Back's read after the Delete, so it no longer lists `free`.
    answer(&mut helm, back_reads, &json!([]));
    settle(cx);
    assert!(
        drawn(cx, "credential-editor"),
        "the newer editor stays open"
    );
    assert_eq!(
        label_of(cx, "credential-editor-title").as_deref(),
        Some("New endpoint token")
    );
    assert_eq!(secret(cx).1, "fictional-typed-secret", "with its secret");
    let (_, method, path, _) = one_command(&mut helm);
    assert_eq!(
        (method, path.as_str()),
        (Method::Get, "/api/credentials"),
        "read again"
    );
}

/// SPEC.md: "Reference changes refresh an open credential editor without discarding its unsaved replacement text":
/// once Recipes shares the dialog, a recipe edit made while an editor stays open must free or block its Delete. The
/// old app copied each fresh read's use into the open editor. Specified: the editor of `used` shows its recipe and
/// an unavailable Delete; after Sessions and Credentials again, whose read lists `used` with no recipe, the editor
/// says no recipe uses it, Delete is available, and the typed secret is kept.
#[gpui::test]
fn an_open_editor_follows_a_fresh_read_and_keeps_its_secret(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_credentials(
        &mut helm,
        cx,
        json!([entry("used", "endpoint_token", 1, &["inference"])]),
    );
    press(cx, "credential-edit-used");
    fill(cx, "credential-secret", "fictional-typed-secret");
    assert_eq!(
        available(cx, "credential-delete"),
        Some(false),
        "the premise: in use"
    );

    click(cx, section_button(Section::Sessions));
    click(cx, section_button(Section::Credentials));
    let reads = sent(&mut helm);
    answer(
        &mut helm,
        reads,
        &json!([entry("used", "endpoint_token", 1, &[])]),
    );
    settle(cx);
    reach(cx, "credential-cancel");
    assert!(painted(cx).contains("No recipes use this credential."));
    assert_eq!(available(cx, "credential-delete"), Some(true));
    assert_eq!(secret(cx).1, "fictional-typed-secret", "the secret is kept");
}

/// The old editor's agent buttons emptied the secret: a Claude token left in the box would be saved as a Codex login
/// (or the other way round). Specified: a token typed for Claude Code is gone, from the field and the draft, once Codex
/// is chosen.
#[gpui::test]
fn switching_agents_empties_the_secret(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_credentials(&mut helm, cx, json!([]));
    press(cx, "credentials-add-agent-login");
    fill(cx, "credential-secret", "fictional-claude-token");
    assert_eq!(
        secret(cx).1,
        "fictional-claude-token",
        "the premise: a token typed"
    );
    choose_codex(cx);
    assert_eq!(secret(cx), (String::new(), String::new()));
}

/// Cancelling the platform's file dialog is the ordinary way out of it, and Choose file… is drawn busy while a choice
/// is out; a cancel that left it busy would make the button do nothing until Save or Back. Specified: Choose file…
/// opens the dialog; the dialog answered with no file leaves the box as it was, and Choose file… opens the dialog again.
#[gpui::test]
fn a_cancelled_file_dialog_frees_choose_file(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_credentials(&mut helm, cx, json!([]));
    press(cx, "credentials-add-agent-login");
    choose_codex(cx);
    press(cx, "credential-choose-file");
    assert!(cx.did_prompt_for_paths(), "the premise: the dialog opened");
    cx.simulate_path_prompt_response(|_| None);
    settle(cx);
    assert_eq!(
        secret(cx),
        (String::new(), String::new()),
        "the box is as it was"
    );
    press(cx, "credential-choose-file");
    assert!(
        cx.did_prompt_for_paths(),
        "Choose file… opens the dialog again"
    );
    cx.simulate_path_prompt_response(|_| None);
    settle(cx);
}

// ---- The Tensorlake API key ----------------------------------------------------------------------------------------

/// SPEC.md: Credentials "includes the same Tensorlake API key form as Tensorlake, with Save, Test and Clear", the key
/// is write-only, and "A save or clear ... updates both forms' saved-key metadata immediately". Each control must send
/// only the field it owns, and Test must use the saved key, never the field. Specified: with no key saved the field
/// says "No key saved"; a typed key's Save sends `PUT /api/tensorlake` with only the key under the preference row's
/// key, and the answer shows "API key saved", empties the field's draft and says a key is saved; Test sends `POST
/// /api/tensorlake/test` with no body and shows the count; Clear sends only the clear flag, shows "API key cleared"
/// and "No key saved", and empties a key typed meanwhile, as the old form did; a refused Save keeps the typed key with
/// the helm's words.
#[gpui::test]
fn the_tensorlake_key_saves_tests_and_clears_with_write_only_metadata(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_credentials(&mut helm, cx, json!([]));
    assert!(painted(cx).contains("No key saved"));
    assert_eq!(
        available(cx, "tensorlake-key-test"),
        Some(false),
        "nothing saved to test"
    );
    fill(cx, "tensorlake-key", "fictional-tensorlake-key");
    reach(cx, "tensorlake-key-save");
    keys(cx, "space");
    let (id, method, path, body) = one_command(&mut helm);
    assert_eq!((method, path.as_str()), (Method::Put, "/api/tensorlake"));
    assert_eq!(body, json!({ "api_key": "fictional-tensorlake-key" }));
    let preferences = cx.update(|_, cx| {
        session::current_operations(cx)
            .and_then(|operations| operations.read(cx).in_flight(&OpKey::preferences()))
    });
    assert_eq!(preferences, Some(Work::Save));
    helm.reply(id, 200, Some(account(true)));
    settle(cx);
    assert_eq!(
        label_of(cx, "tensorlake-key-outcome").as_deref(),
        Some("API key saved")
    );
    let text = painted(cx);
    assert!(
        text.contains("Key saved; paste a new one to replace it"),
        "{text}"
    );
    assert_eq!(key_typed(cx), "", "the field was emptied");

    reach(cx, "tensorlake-key-test");
    keys(cx, "space");
    let (id, method, path, body) = one_command(&mut helm);
    assert_eq!(
        (method, path.as_str(), body),
        (Method::Post, "/api/tensorlake/test", Value::Null)
    );
    helm.reply(id, 200, Some(json!({ "sandboxes": 2 })));
    settle(cx);
    assert_eq!(
        label_of(cx, "tensorlake-key-outcome").as_deref(),
        Some("The key works. It can see 2 sandboxes.")
    );

    fill(cx, "tensorlake-key", "fictional-unsaved-key");
    assert_eq!(
        key_typed(cx),
        "fictional-unsaved-key",
        "the premise: a key typed"
    );
    reach(cx, "tensorlake-key-clear");
    keys(cx, "space");
    let (id, _, _, body) = one_command(&mut helm);
    assert_eq!(body, json!({ "clear_api_key": true }));
    helm.reply(id, 200, Some(account(false)));
    settle(cx);
    assert_eq!(
        label_of(cx, "tensorlake-key-outcome").as_deref(),
        Some("API key cleared")
    );
    assert!(painted(cx).contains("No key saved"));
    assert_eq!(key_typed(cx), "", "Clear emptied the typed key");

    fill(cx, "tensorlake-key", "fictional-bad-key");
    reach(cx, "tensorlake-key-save");
    keys(cx, "space");
    let (id, _, _, _) = one_command(&mut helm);
    helm.reply(id, 400, Some(json!("the key is not valid")));
    settle(cx);
    assert_eq!(
        label_of(cx, "tensorlake-key-outcome").as_deref(),
        Some("the key is not valid")
    );
    keys(cx, "space");
    let (_, _, _, body) = one_command(&mut helm);
    assert_eq!(
        body,
        json!({ "api_key": "fictional-bad-key" }),
        "the typed key was kept"
    );
}

/// The form's controls must wait for the account read: before it answers, the form does not know whether a key is
/// saved, so Test and Clear would act on a guess, and a Save would overwrite a key the form never showed (the old "a
/// failed load seeds nothing editable"). Specified: before the section's reads answer, the field shows neither "No key
/// saved" nor "Key saved", typing into it leaves the draft empty, and Test, Clear and Save stay unavailable with a key
/// typed; once the account answers that no key is saved, the field says "No key saved" and a typed key makes Save
/// available.
#[gpui::test]
fn the_tensorlake_key_waits_for_the_account_read(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, section_button(Section::Credentials));
    let reads = sent(&mut helm);
    assert!(
        reads.iter().any(|read| read.path == "/api/tensorlake"),
        "the premise: the account read is out"
    );
    let text = painted(cx);
    assert!(
        !text.contains("No key saved")
            && !text.contains("Key saved; paste a new one to replace it"),
        "{text}"
    );
    fill(cx, "tensorlake-key", "fictional-early-key");
    assert_eq!(
        key_typed(cx),
        "",
        "the field is read-only until the account answers"
    );
    for control in [
        "tensorlake-key-test",
        "tensorlake-key-clear",
        "tensorlake-key-save",
    ] {
        assert_eq!(
            available(cx, control),
            Some(false),
            "{control} waits for the account"
        );
    }

    answer(&mut helm, reads, &json!([]));
    settle(cx);
    assert!(painted(cx).contains("No key saved"));
    fill(cx, "tensorlake-key", "fictional-tensorlake-key");
    assert_eq!(key_typed(cx), "fictional-tensorlake-key");
    assert_eq!(available(cx, "tensorlake-key-save"), Some(true));
}

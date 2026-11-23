//! Tests of the template editor, driven through the Templates dialog over the helm client's fake: opening a row or a
//! new template, editing, adding and removing fields, and Save, its refusals and its busy state.
//!
//! The dialog is opened from the dialog tests' header ([`super::dialog_tests::Header`]), as the list header's
//! Templates button will open it. The first editor a dialog opens reads the model catalog, which the tests answer
//! ([`open_editor`]); every step ends in one of the shared harness's step helpers, so the focus and layer invariants
//! are checked after each.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::launcher::{LaunchTemplate, LauncherKind, TemplateFields};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, HostView,
    KeyedEntry, PREFERENCES_KEY, Preferences,
};
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchTarget, Place};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::dialog_tests::{open_header, open_header_with, painted, push, template};
use super::fields::Draft;
use crate::components::EditCommand;
use crate::focus;
use crate::layers;
use crate::test_support::{self, any_open, click, frame, keys, meta, settle};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The templates most tests start with, in the helm's order (by name): one with a launch kind, which opens clean, one
/// without, which opens with its switch inferred, and a command launch.
pub(super) fn templates() -> Vec<LaunchTemplate> {
    vec![
        template(
            "codex-deep",
            TemplateFields {
                kind: Some(LauncherKind::Agent),
                agent: Some(LaunchHarness::Codex),
                model: Some(Some("gpt-6.1-sol".into())),
                effort: Some(Some(LaunchEffort::High.into())),
                ..TemplateFields::default()
            },
        ),
        template(
            "kindless",
            TemplateFields {
                agent: Some(LaunchHarness::Codex),
                ..TemplateFields::default()
            },
        ),
        template(
            "webbuilder",
            TemplateFields {
                kind: Some(LauncherKind::Command),
                command: Some("make dev".into()),
                ..TemplateFields::default()
            },
        ),
    ]
}

/// The model catalog the tests answer the editor's read with: two Codex models.
fn catalog() -> Value {
    json!([
        {"id": "gpt-6-luna", "harness": "codex", "efforts": ["low", "medium", "high"]},
        {"id": "gpt-6.1-sol", "harness": "codex", "efforts": ["low", "medium", "high"]},
    ])
}

/// Open the dialog from the header and click `opener` (a row's open button or "+ New template"), then answer the model
/// catalog read the first editor sends. Asserts that read was all that was sent, so a test's later commands are its
/// own.
pub(super) fn open_editor(helm: &mut FakeHelm, opener: &'static str, cx: &mut VisualTestContext) {
    click(cx, "templates-button");
    click(cx, opener);
    settle(cx);
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "only the catalog was read: {sent:?}");
    assert_eq!(
        (sent[0].method, sent[0].path.as_str()),
        (Method::Get, "/api/launch-catalog")
    );
    helm.reply(sent[0].id, 200, Some(catalog()));
    settle(cx);
}

/// The open editor's draft.
pub(super) fn draft(cx: &mut VisualTestContext) -> Draft {
    cx.update(|_, cx| {
        let dialog =
            layers::last_opened::<super::dialog::TemplatesDialog>(cx).expect("a dialog is open");
        let dialog = dialog.read(cx);
        dialog
            .editor
            .as_ref()
            .expect("an editor is open")
            .draft()
            .clone()
    })
}

/// Whether the open editor's name field holds focus.
pub(super) fn name_focused(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| {
        let dialog =
            layers::last_opened::<super::dialog::TemplatesDialog>(cx).expect("a dialog is open");
        let dialog = dialog.read(cx);
        dialog
            .editor
            .as_ref()
            .and_then(|editor| editor.name_focus(cx))
            .is_some_and(|name| name.is_focused(window))
    })
}

/// Replace all the text of the focused field with `text`, as selecting it all and typing does.
fn retype(cx: &mut VisualTestContext, text: &str) {
    cx.update(|window, cx| window.dispatch_action(EditCommand::SelectAll.action(), cx));
    cx.simulate_input(text);
    settle(cx);
}

/// The template writes sent since the last call: each one's path and body.
fn saves(helm: &mut FakeHelm) -> Vec<(String, Value)> {
    helm.commands()
        .into_iter()
        .map(|sent| {
            assert_eq!(sent.method, Method::Put, "only saves are sent: {sent:?}");
            (sent.path, sent.body.unwrap_or_default())
        })
        .collect()
}

/// Whether the element drawn under `id` reports itself pressed; `None` when it is not drawn.
fn pressed(cx: &mut VisualTestContext, id: &str) -> Option<bool> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| node.toggled == Some(gpui::Toggled::True))
}

/// Whether anything is drawn under `id`.
pub(super) fn drawn(cx: &mut VisualTestContext, id: &str) -> bool {
    frame(cx)
        .into_iter()
        .any(|node| node.element_id.as_deref() == Some(id))
}

/// The ids of the editor's field rows, in drawn order.
fn field_rows(cx: &mut VisualTestContext) -> Vec<String> {
    frame(cx)
        .into_iter()
        .filter_map(|node| node.element_id.map(|id| id.to_string()))
        .filter(|id| id.starts_with("templates-field-"))
        .collect()
}

// ---- Opening ---------------------------------------------------------------------------------------------------------

/// The old dialog opened with nothing in the editor and focus on "+ New template", and SPEC.md's editor is "the
/// selected template's editor": until a row is chosen there is nothing to edit. Specified: the pane says how to open
/// one, and focus starts on "+ New template".
#[gpui::test]
fn nothing_is_open_until_a_row_or_new_template_opens_one(cx: &mut TestAppContext) {
    let (_helm, cx) = open_header(cx, &templates(), None);
    click(cx, "templates-button");
    settle(cx);
    let text = painted(cx);
    assert!(
        text.contains("Choose a template to edit, or make a new one."),
        "{text}"
    );
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-new")
    );
}

/// A row opens its template in the editor: the heading says it edits a stored template, the row is drawn pressed so
/// the list shows which template is open, and focus moves to the name field, as the old dialog's Open did. Opening
/// the same row again must not throw away what was typed (the old dialog's "reselecting preserves edits" case).
#[gpui::test]
fn a_row_opens_its_template_pressed_with_focus_on_its_name(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    let text = painted(cx);
    assert!(text.contains("Edit template"), "{text}");
    assert!(!text.contains("Unsaved changes"), "it opens clean: {text}");
    assert_eq!(pressed(cx, "template-open-codex-deep"), Some(true));
    assert_eq!(pressed(cx, "template-open-webbuilder"), Some(false));
    assert!(name_focused(cx), "focus is on the name field");
    assert_eq!(
        field_rows(cx),
        vec![
            "templates-field-agent",
            "templates-field-model",
            "templates-field-effort"
        ],
        "one row per field it sets, in the editor's order"
    );

    cx.simulate_input("-two");
    settle(cx);
    click(cx, "template-open-codex-deep");
    settle(cx);
    assert_eq!(draft(cx).name, "codex-deep-two", "reopening kept the edit");
    assert!(painted(cx).contains("Unsaved changes"));
    assert!(helm.commands().is_empty(), "nothing was sent");
}

/// The rest of the old "reselecting the current template preserves edits and renames": the row the editor holds stays a
/// no-op for any field, not just the name, and still after a save, when the editor holds the saved template as its
/// baseline; a reopen that reloaded the stored copy would silently drop the edit or the rename being typed. Specified:
/// with `codex-deep`'s model edited, a click on its row asks nothing and keeps the edit; after that edit is saved, a
/// new name typed in the name field survives another click on the row, which again asks nothing and sends nothing.
#[gpui::test]
fn reopening_the_open_row_keeps_any_edit_also_after_a_save(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    let luna = Some(Some("gpt-6-luna".to_owned()));
    click(cx, "templates-suggestion-gpt-6-luna");
    assert_eq!(draft(cx).fields.model, luna, "the premise: the model edit");
    click(cx, "template-open-codex-deep");
    assert!(!drawn(cx, "prompt-confirm"), "no leave question");
    assert_eq!(draft(cx).fields.model, luna, "reopening kept the model");

    click(cx, "templates-save");
    let (id, saved) = one(&mut helm, Method::Put, "/api/templates/codex-deep");
    let mut stored = templates();
    stored[0].fields = serde_json::from_value(saved).expect("the fields decode");
    push(&mut helm, &stored, Availability::Available, cx);
    helm.reply(id, 200, None);
    settle(cx);
    assert!(
        !painted(cx).contains("Unsaved changes"),
        "the premise: saved"
    );
    click(cx, "templates-name");
    retype(cx, "codex-renamed");
    click(cx, "template-open-codex-deep");
    assert!(!drawn(cx, "prompt-confirm"), "no leave question");
    assert_eq!(draft(cx).name, "codex-renamed", "reopening kept the rename");
    assert!(helm.commands().is_empty(), "nothing was sent");
}

/// "+ New template" opens an empty draft on the Agent switch (the old editor's new draft), headed "New template",
/// with no row pressed and focus in its empty name field.
#[gpui::test]
fn new_template_opens_an_empty_draft(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "templates-new", cx);
    assert!(painted(cx).contains("New template"));
    assert_eq!(draft(cx), Draft::default());
    assert_eq!(pressed(cx, "template-open-codex-deep"), Some(false));
    assert!(name_focused(cx));
    assert!(field_rows(cx).is_empty(), "it sets nothing yet");
}

/// SPEC.md: "Existing kindless templates keep applying as stored; opening one infers its switch in an unsaved draft,
/// and saving stores that switch." Specified: a kindless template opens with the inferred switch chosen, the hint
/// saying so, and unsaved changes; its Save stores the switch.
#[gpui::test]
fn a_kindless_template_opens_with_its_inferred_switch_unsaved(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-kindless", cx);
    let text = painted(cx);
    assert!(
        text.contains("This stored template does not switch the launcher yet."),
        "{text}"
    );
    assert!(text.contains("Unsaved changes"), "{text}");
    assert_eq!(pressed(cx, "templates-switch-Agent"), Some(true));
    click(cx, "templates-save");
    assert_eq!(
        saves(&mut helm),
        vec![(
            "/api/templates/kindless".to_owned(),
            json!({"kind": "agent", "agent": "codex"})
        )]
    );
}

// ---- Saving ----------------------------------------------------------------------------------------------------------

/// The core of SPEC.md's Save: "Saving is explicit; Save is greyed out and does nothing while there are no unsaved
/// changes, and keeps keyboard focus after saving", and SPEC_impl.md's "Writes are serialized". Specified: a clean
/// Save sends nothing; after an edit (a model suggestion) Save writes the whole template once; while it is in flight a
/// second press sends nothing and focus stays on Save; once the helm has stored it, the draft is clean and Save, still
/// focused, is inert again.
#[gpui::test]
fn save_writes_the_whole_template_once_and_keeps_focus(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    // A click focuses the button it presses, as it does on Linux; pinned so the premise holds on every platform.
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    open_editor(&mut helm, "template-open-codex-deep", cx);
    click(cx, "templates-save");
    assert!(saves(&mut helm).is_empty(), "a clean Save does nothing");

    click(cx, "templates-suggestion-gpt-6-luna");
    assert!(painted(cx).contains("Unsaved changes"));
    click(cx, "templates-save");
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(
        (sent[0].method, sent[0].path.as_str()),
        (Method::Put, "/api/templates/codex-deep")
    );
    let saved = json!({"kind": "agent", "agent": "codex", "model": "gpt-6-luna", "effort": "high"});
    assert_eq!(sent[0].body, Some(saved.clone()));

    click(cx, "templates-save");
    assert!(
        helm.commands().is_empty(),
        "a press while saving sends nothing"
    );
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-save")
    );

    // The helm stores it and pushes the list before it replies, as the channel orders a command's changes.
    let mut stored = templates();
    stored[0].fields = serde_json::from_value(saved).expect("the fields decode");
    push(&mut helm, &stored, Availability::Available, cx);
    helm.reply(sent[0].id, 200, None);
    settle(cx);
    assert!(
        !painted(cx).contains("Unsaved changes"),
        "the saved draft is the baseline"
    );
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-save"),
        "Save kept focus"
    );
    click(cx, "templates-save");
    assert!(helm.commands().is_empty(), "Save is inert again");
}

/// SPEC.md, "One GUI at a time" (2026-10-22): "a GUI save that fails ... is reported to the user", and is never
/// retried. The editor keeps the draft so the user can correct it. Specified: a refused save shows the helm's words in
/// the status line, the draft stays unsaved, nothing is sent again, and the fields take edits again.
#[gpui::test]
fn a_refused_save_is_reported_kept_and_never_retried(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    click(cx, "templates-suggestion-gpt-6-luna");
    click(cx, "templates-save");
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "the premise: one save was sent");
    helm.reply(
        sent[0].id,
        503,
        Some(json!("fixture template save refused")),
    );
    settle(cx);
    let text = painted(cx);
    assert!(text.contains("fixture template save refused"), "{text}");
    assert!(text.contains("Unsaved changes"), "{text}");
    assert!(
        helm.commands().is_empty(),
        "the failed save was not retried"
    );
    click(cx, "templates-name");
    settle(cx);
    cx.simulate_input("-x");
    settle(cx);
    assert_eq!(
        draft(cx).name,
        "codex-deep-x",
        "the field takes edits again"
    );
}

/// The old editor refused a save before sending it, in a fixed order, and said why in the dialog's status line; a
/// test of the order keeps a later refactor from showing a less useful reason first. Specified, on a new template: a
/// name with whitespace is refused first; then the draft's own reason (an empty session name); then a name another
/// template has; and none of them sends anything. A free name then saves.
#[gpui::test]
fn save_refusals_come_in_the_old_order_and_send_nothing(cx: &mut TestAppContext) {
    let taken = "סקירה";
    let mut templates = templates();
    templates[0].name = taken.into();
    let (mut helm, cx) = open_header(cx, &templates, None);
    open_editor(&mut helm, "templates-new", cx);
    assert!(
        cx.debug_bounds("template-open-סקירה").is_some(),
        "the conflicting template is already listed"
    );
    cx.simulate_input("my codex");
    settle(cx);
    click(cx, "templates-add");
    click(cx, "templates-add-name");
    click(cx, "templates-save");
    assert!(
        painted(cx).contains("A new template name cannot contain whitespace"),
        "{}",
        painted(cx)
    );

    click(cx, "templates-name");
    retype(cx, taken);
    click(cx, "templates-save");
    assert!(
        painted(cx).contains("Type the session name, or remove its field"),
        "{}",
        painted(cx)
    );

    click(cx, "templates-remove-name");
    click(cx, "templates-save");
    let text = painted(cx);
    assert!(
        text.contains("A template named \"סקירה\" already exists"),
        "{text}"
    );
    // A long explanation and an RTL name share the line. The name must stay within its own direction boundary.
    assert!(
        frame(cx)
            .into_iter()
            .flat_map(|node| node.texts)
            .any(
                |line| crate::host_text::tests::paints_isolated(&line.text, taken)
                    && line.text.contains("already exists")
            )
    );
    assert!(saves(&mut helm).is_empty(), "no refused save was sent");

    click(cx, "templates-name");
    retype(cx, "fresh");
    click(cx, "templates-save");
    assert_eq!(
        saves(&mut helm),
        vec![("/api/templates/fresh".to_owned(), json!({"kind": "agent"}))]
    );
}

/// A new name cannot be proven free while the helm reports its templates unavailable: the list the editor checks
/// against is the store's, and an unavailable one is not "no templates". Specified: the save is refused with the
/// reason, and nothing is sent.
#[gpui::test]
fn a_new_name_is_refused_while_the_templates_are_unavailable(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "templates-new", cx);
    cx.simulate_input("fresh");
    settle(cx);
    push(
        &mut helm,
        &templates(),
        Availability::Unavailable {
            error: "database is locked".into(),
        },
        cx,
    );
    click(cx, "templates-save");
    let text = painted(cx);
    assert!(
        text.contains("The template list could not be read, so Farhelm cannot check that no other template has this name"),
        "{text}"
    );
    assert!(saves(&mut helm).is_empty());
}

// ---- Fields ----------------------------------------------------------------------------------------------------------

/// SPEC.md: "explicit add/remove actions". "+ Add field" lists only the fields the template's switch and agent type
/// offer and it does not set yet; choosing one adds its row in the editor's order; its ✕ removes it again. Escape with
/// the menu open closes only the menu, focus going back to "+ Add field", as the old dialog's Escape did.
#[gpui::test]
fn add_field_offers_unset_fields_and_escape_closes_only_its_menu(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    click(cx, "templates-add");
    assert!(drawn(cx, "templates-add-host"), "host is offered");
    assert!(
        drawn(cx, "templates-add-permissions"),
        "approvals are offered"
    );
    assert!(
        !drawn(cx, "templates-add-model"),
        "a set field is not offered"
    );
    assert!(
        !drawn(cx, "templates-add-command"),
        "a command field does not fit the Agent switch"
    );
    keys(cx, "escape");
    assert!(any_open(cx), "the dialog stays");
    assert!(!drawn(cx, "templates-add-menu"), "the menu closed");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-add")
    );

    click(cx, "templates-add");
    click(cx, "templates-add-host");
    assert!(!drawn(cx, "templates-add-menu"), "choosing closes the menu");
    assert_eq!(
        field_rows(cx),
        vec![
            "templates-field-agent",
            "templates-field-model",
            "templates-field-effort",
            "templates-field-host"
        ]
    );
    click(cx, "templates-remove-model");
    assert_eq!(draft(cx).fields.model, None, "removed, not reset");
    assert_eq!(
        field_rows(cx),
        vec![
            "templates-field-agent",
            "templates-field-effort",
            "templates-field-host"
        ]
    );
}

/// SPEC.md: "Incompatible fields stay visible and prevent saving until corrected or removed", and the switch "never
/// clears fields" (the old control). Specified: switching an agent template to Command keeps its model and effort,
/// each saying under it why it does not fit, and Save is refused with the first such field's reason.
#[gpui::test]
fn a_field_that_no_longer_fits_stays_and_refuses_saving(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    click(cx, "templates-switch-Command");
    assert_eq!(draft(cx).fields.model, Some(Some("gpt-6.1-sol".into())));
    assert!(drawn(cx, "templates-model-refusal"));
    assert!(drawn(cx, "templates-effort-refusal"));
    assert!(
        painted(cx).contains("Does not fit this launcher switch or agent type; remove it to save")
    );
    click(cx, "templates-save");
    assert!(
        painted(cx)
            .contains("Model: does not fit this launcher switch or agent type; remove it to save")
    );
    assert!(saves(&mut helm).is_empty());
}

/// A name holding invisible formatting is host text the editor cannot show raw (SPEC.md: "Invisible formatting is
/// shown escaped and read-only, retaining its raw value through unrelated saves"). Specified: the field shows the
/// escapes with the old hint, and an unrelated save writes under the raw name. Replacing the name is
/// `replace_name_clears_the_name_and_focuses_it`'s.
#[gpui::test]
fn an_escaped_name_saves_raw_until_replaced(cx: &mut TestAppContext) {
    let sneaky = vec![template(
        "sneaky\u{200B}x",
        TemplateFields {
            kind: Some(LauncherKind::Agent),
            ..TemplateFields::default()
        },
    )];
    let (mut helm, cx) = open_header(cx, &sneaky, None);
    open_editor(&mut helm, "template-open-sneaky\u{200B}x", cx);
    assert!(
        painted(cx).contains("This name contains invisible formatting, shown as escapes."),
        "{}",
        painted(cx)
    );
    click(cx, "templates-switch-Don't switch");
    click(cx, "templates-save");
    let path = ApiPath::api(&["templates", "sneaky\u{200B}x"]).expect("a path");
    assert_eq!(
        saves(&mut helm),
        vec![(path.as_str().to_owned(), json!({}))]
    );
}

/// "Replace name" is the one way to edit an escaped name: it clears the name and puts focus in the field, so typing
/// replaces it.
#[gpui::test]
fn replace_name_clears_the_name_and_focuses_it(cx: &mut TestAppContext) {
    let sneaky = vec![template("sneaky\u{200B}x", TemplateFields::default())];
    let (mut helm, cx) = open_header(cx, &sneaky, None);
    open_editor(&mut helm, "template-open-sneaky\u{200B}x", cx);
    click(cx, "templates-replace-name");
    settle(cx);
    assert_eq!(draft(cx).name, "");
    assert!(name_focused(cx));
    cx.simulate_input("plain");
    settle(cx);
    assert_eq!(draft(cx).name, "plain");
}

/// The session name's kind and the destination's kind are choices beside their text (SPEC.md: "base name | fixed name";
/// "a folder or a managed checkout"), and both reach the saved template: Fixed is stored, Base is the kind's absence,
/// and switching the destination keeps its text. Specified: a new template with a fixed session name and a managed
/// checkout saves exactly those, and the managed checkout shows its hint that each session gets its own checkout, which
/// moves to the trash when the session is deleted (the old editor's `.templates-hint`).
#[gpui::test]
fn the_name_kind_and_destination_kind_are_saved(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "templates-new", cx);
    cx.simulate_input("review-web");
    settle(cx);
    click(cx, "templates-add");
    click(cx, "templates-add-name");
    click(cx, "templates-session-name");
    settle(cx);
    cx.simulate_input("review");
    settle(cx);
    assert_eq!(
        draft(cx).fields.name.as_deref(),
        Some("review"),
        "the premise: the session name was typed"
    );
    click(cx, "templates-name-kind-Fixed name");
    click(cx, "templates-add");
    click(cx, "templates-add-destination");
    click(cx, "templates-destination");
    settle(cx);
    cx.simulate_input("acme/web");
    settle(cx);
    click(cx, "templates-destination-type-Managed checkout");
    let made = draft(cx);
    assert_eq!(
        (made.name.as_str(), made.fields.destination.clone()),
        (
            "review-web",
            Some(farhelm_proto::launcher::TemplateDestination::Github(
                "acme/web".into()
            ))
        ),
        "the premise: the destination was typed and switched"
    );
    assert!(drawn(cx, "templates-checkout-hint"), "the checkout's hint");
    assert!(
        painted(cx).contains("moves to the trash"),
        "{}",
        painted(cx)
    );
    // Four fields with their hints run the editor past the dialog's height; a taller window keeps Save in view, where a
    // click reaches it.
    cx.simulate_resize(gpui::size(gpui::px(1600.), gpui::px(1600.)));
    settle(cx);
    click(cx, "templates-save");
    assert_eq!(
        saves(&mut helm),
        vec![(
            "/api/templates/review-web".to_owned(),
            json!({
                "kind": "agent",
                "destination": {"github": "acme/web"},
                "name": "review",
                "name_kind": "fixed",
            })
        )]
    );
}

/// A host is named by its recorded install identity, and one no listed host has must not vanish from the template
/// (the old "Unavailable host (retained)"). Specified: such a template shows that label for its host.
#[gpui::test]
fn an_unknown_host_identity_is_kept_as_retained(cx: &mut TestAppContext) {
    let gone = vec![template(
        "elsewhere",
        TemplateFields {
            kind: Some(LauncherKind::Agent),
            host: Some("install-gone".into()),
            ..TemplateFields::default()
        },
    )];
    let (mut helm, cx) = open_header(cx, &gone, None);
    open_editor(&mut helm, "template-open-elsewhere", cx);
    assert!(
        painted(cx).contains("Unavailable host (retained)"),
        "{}",
        painted(cx)
    );
    assert_eq!(draft(cx).fields.host.as_deref(), Some("install-gone"));
}

// ---- Where its sessions run ------------------------------------------------------------------------------------------

/// A recipe as the helm pushes it, keyed by its name: `name` on `base`, built nowhere.
fn recipe(name: &str, base: Value) -> KeyedEntry {
    let value = json!({
        "name": name, "size": {"cpus": 2, "memory_gb": 4, "disk_gb": 20}, "base": base, "packages": [],
        "claude": null, "codex": null, "claude_login_credential": null, "codex_login_credential": null,
        "github_token_credential": null, "tunnels": [], "snapshot": null, "build": {"state": "idle"},
        "prepared": null, "preparation": {"state": "idle"}, "cleanup_pending": false,
    });
    KeyedEntry {
        key: name.into(),
        value,
    }
}

/// [`open_header`] with the pinned fixture hosts at `hosts` (0: host1, local, Podman usable; 1: host2, SSH, Podman not
/// installed; 2: host3, SSH with no check at all, read as "this host cannot run containers"; install identities
/// `identity1` to `identity3`) and the recipe catalog `recipes`, in that order.
fn open_fleet<'a>(
    cx: &'a mut TestAppContext,
    templates: &[LaunchTemplate],
    hosts: &[usize],
    recipes: Vec<KeyedEntry>,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let keys: Vec<String> = recipes.iter().map(|recipe| recipe.key.clone()).collect();
    let catalog = CollectionState {
        collection: Collection::Recipes,
        meta: meta(&keys.iter().map(String::as_str).collect::<Vec<_>>()),
        entries: recipes,
    };
    let hosts = crate::launcher::view_tests::fixture_hosts(hosts);
    open_header_with(cx, templates, None, vec![hosts, catalog])
}

/// What the control drawn under `id` reports as its value (a select's closed text), empty when it is not drawn.
fn value_of(cx: &mut VisualTestContext, id: &str) -> String {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .and_then(|node| node.value.map(|value| value.to_string()))
        .unwrap_or_default()
}

/// The open editor's draft's environment, as its `target` field stores it.
fn target(cx: &mut VisualTestContext) -> Option<LaunchTarget> {
    draft(cx).fields.target
}

/// A new environment of `recipe` at `place`.
fn environment(recipe: &str, place: Place) -> LaunchTarget {
    LaunchTarget::Environment {
        recipe: recipe.into(),
        place,
    }
}

/// The old acceptance case "template placement keeps Host editable after leaving an environment"
/// (`tensorlake-sandbox-sessions.spec.ts`): leaving an environment must leave a Host the user can fill in, and the save
/// must store a plain host, since the helm refuses a template holding a host and a placement. Specified: a legacy
/// `{recipe: session-ready}` template opens on New sandbox with Recipe showing `session-ready` and no Host select;
/// Existing host shows the Host select on "Choose a host"; identity2 chosen there stays through a second press of
/// Existing host; Save stores `host` with no `target` and no `recipe`; and a Host field added to another template then
/// starts on Existing host, present and empty.
#[gpui::test]
fn leaving_an_environment_keeps_the_host_editable(cx: &mut TestAppContext) {
    let ordinary = TemplateFields {
        kind: Some(LauncherKind::Command),
        command: Some("sleep 300".into()),
        yolo: Some(false),
        ..TemplateFields::default()
    };
    let legacy = TemplateFields {
        recipe: Some("session-ready".into()),
        ..TemplateFields::default()
    };
    let stored = vec![
        template("ordinary", ordinary),
        template("sandboxed", legacy),
    ];
    let catalog = vec![recipe(
        "session-ready",
        json!({"kind": "tensorlake_ubuntu"}),
    )];
    let (mut helm, cx) = open_fleet(cx, &stored, &[0, 1, 2], catalog);
    open_editor(&mut helm, "template-open-sandboxed", cx);
    assert_eq!(pressed(cx, "templates-runs-on-New sandbox"), Some(true));
    assert_eq!(value_of(cx, "templates-recipe"), "session-ready");
    assert!(!drawn(cx, "templates-host"), "no Host select there");

    click(cx, "templates-runs-on-Existing host");
    assert_eq!(value_of(cx, "templates-host"), "Choose a host");
    click(cx, "templates-host");
    click(cx, "templates-host-option-identity2");
    let host = |cx: &mut VisualTestContext| draft(cx).fields.host;
    assert_eq!(
        host(cx).as_deref(),
        Some("identity2"),
        "the premise: chosen"
    );
    click(cx, "templates-runs-on-Existing host");
    assert_eq!(
        host(cx).as_deref(),
        Some("identity2"),
        "the chosen segment changes nothing"
    );

    click(cx, "templates-save");
    let (id, body) = one(&mut helm, Method::Put, "/api/templates/sandboxed");
    assert_eq!(body["host"], json!("identity2"));
    assert!(
        body.get("target").is_none() && body.get("recipe").is_none(),
        "{body}"
    );
    let mut saved = stored.clone();
    saved[1].fields = serde_json::from_value(body).expect("the fields decode");
    push(&mut helm, &saved, Availability::Available, cx);
    helm.reply(id, 200, None);
    settle(cx);
    assert!(
        !painted(cx).contains("Unsaved changes"),
        "the premise: saved"
    );

    click(cx, "template-open-ordinary");
    settle(cx);
    assert_eq!(draft(cx).name, "ordinary", "the premise: ordinary opened");
    click(cx, "templates-add");
    click(cx, "templates-add-host");
    assert_eq!(host(cx).as_deref(), Some(""));
    assert_eq!(pressed(cx, "templates-runs-on-Existing host"), Some(true));
    assert_eq!(value_of(cx, "templates-host"), "Choose a host");
}

/// The old `PlacementPicker`'s authoring mode (SPEC.md, "Creation": the template editor does not check readiness):
/// entering a segment starts the template somewhere it can launch rather than half placed, and a template names a
/// recipe and place for later launches, so an unusable container host and an unbuilt sandbox recipe stay choosable
/// while a recipe whose base the provider cannot run does not. Specified, with `tl-image` (a Tensorlake image, which a
/// container cannot run) listed before `web` (managed Ubuntu): New container enters `web` on host1, the first container
/// host, dropping the host identity; host2, whose Podman is not installed, is offered with that reason and taken;
/// `tl-image` is refused there; New sandbox then enters `tl-image` at the Tensorlake account, where `web`, built
/// nowhere, can be picked; Save stores that as the `target`, with no `host` and no `recipe`.
#[gpui::test]
fn entering_an_environment_chooses_its_first_recipe_and_place(cx: &mut TestAppContext) {
    let on_host = TemplateFields {
        kind: Some(LauncherKind::Agent),
        host: Some("identity1".into()),
        ..TemplateFields::default()
    };
    let catalog = vec![
        recipe("tl-image", json!({"kind": "image", "image": "ubuntu"})),
        recipe("web", json!({"kind": "tensorlake_ubuntu"})),
    ];
    let (mut helm, cx) = open_fleet(cx, &[template("placed", on_host)], &[0, 1, 2], catalog);
    open_editor(&mut helm, "template-open-placed", cx);
    click(cx, "templates-runs-on-New container");
    let fields = draft(cx).fields;
    let entered = Some(environment("web", Place::PodmanHost(1)));
    assert_eq!(
        (fields.target, fields.host, fields.recipe),
        (entered, None, None)
    );

    click(cx, "templates-container-host");
    assert!(painted(cx).contains("Podman is not installed on this host"));
    click(cx, "templates-container-host-option-2");
    assert_eq!(target(cx), Some(environment("web", Place::PodmanHost(2))));
    click(cx, "templates-recipe");
    click(cx, "templates-recipe-option-tl-image");
    keys(cx, "escape");
    assert_eq!(target(cx), Some(environment("web", Place::PodmanHost(2))));

    click(cx, "templates-runs-on-New sandbox");
    assert_eq!(
        target(cx),
        Some(environment("tl-image", Place::TensorlakeAccount))
    );
    click(cx, "templates-recipe");
    click(cx, "templates-recipe-option-web");
    let sandbox = environment("web", Place::TensorlakeAccount);
    assert_eq!(target(cx), Some(sandbox.clone()));
    click(cx, "templates-save");
    let (_, body) = one(&mut helm, Method::Put, "/api/templates/placed");
    let saved: TemplateFields = serde_json::from_value(body.clone()).expect("the fields decode");
    assert_eq!(
        (saved.target, saved.host, saved.recipe),
        (Some(sandbox), None, None),
        "{body}"
    );
}

/// A recipe or container host the store stops listing must not be lost (the old "(retained)" labels), and since the
/// editor does not check readiness (SPEC.md, "Creation"), New container must be offered when its only host is
/// unusable, which the launcher's rule would hide. Specified, with only `tl-image` (no container base) and host2
/// (Podman not installed) listed: a container template of `gone` on host 9 shows both retained labels and keeps both; Existing host and
/// then New container enter host2 with no recipe, the Recipe select on "Choose a recipe".
#[gpui::test]
fn retained_choices_and_an_unusable_only_host_stay_in_reach(cx: &mut TestAppContext) {
    let kept = TemplateFields {
        kind: Some(LauncherKind::Agent),
        target: Some(environment("gone", Place::PodmanHost(9))),
        ..TemplateFields::default()
    };
    let tl_image = recipe("tl-image", json!({"kind": "image", "image": "ubuntu"}));
    let (mut helm, cx) = open_fleet(cx, &[template("kept", kept)], &[1], vec![tl_image]);
    open_editor(&mut helm, "template-open-kept", cx);
    assert_eq!(
        value_of(cx, "templates-recipe"),
        "Unavailable recipe (retained)"
    );
    assert_eq!(
        value_of(cx, "templates-container-host"),
        "Unavailable host (retained)"
    );
    assert_eq!(target(cx), Some(environment("gone", Place::PodmanHost(9))));

    click(cx, "templates-runs-on-Existing host");
    click(cx, "templates-runs-on-New container");
    assert_eq!(target(cx), Some(environment("", Place::PodmanHost(2))));
    assert_eq!(value_of(cx, "templates-recipe"), "Choose a recipe");
}

/// The template can keep a container choice after its host is forgotten. Pressing the selected New container
/// segment in that state must keep its recipe and place. An absent destination otherwise looks like Existing host.
#[gpui::test]
fn pressing_selected_new_container_with_no_hosts_keeps_the_template_placement(
    cx: &mut TestAppContext,
) {
    let retained = environment("gone", Place::PodmanHost(9));
    let fields = TemplateFields {
        kind: Some(LauncherKind::Agent),
        target: Some(retained.clone()),
        ..TemplateFields::default()
    };
    let (mut helm, cx) = open_fleet(cx, &[template("kept", fields)], &[], vec![]);
    open_editor(&mut helm, "template-open-kept", cx);
    cx.update(|_, cx| {
        let store = crate::session::current_store(cx).expect("the editor has a helm listing");
        let collections = store.read(cx).collections();
        assert_eq!(
            collections
                .meta(Collection::Hosts)
                .map(|meta| &meta.availability),
            Some(&Availability::Available),
            "the premise: the helm has served its host listing"
        );
        assert!(
            collections.ordered::<HostView>().is_empty(),
            "the premise: no host can hold containers"
        );
    });
    assert_eq!(target(cx), Some(retained.clone()));
    let segment = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("templates-runs-on-New container"))
        .expect("the retained New container segment is drawn");
    assert_eq!(segment.toggled, Some(gpui::Toggled::True));
    assert!(
        !segment.disabled && segment.flags.click,
        "the segment takes a press"
    );

    click(cx, "templates-runs-on-New container");

    assert_eq!(target(cx), Some(retained));
    assert!(
        !drawn(cx, "templates-host"),
        "the editor is still on New container"
    );
}

/// The Custom words live only in their input, so two things the old separate copy of the words gave must still hold:
/// words a stored template keeps beside another choice (one failing today's save rules) come back when Custom is
/// chosen, as the old editor's seeded words did, and words that do not parse survive an unrelated edit instead of being
/// put back to the draft's none. Specified: a template storing `--mode` and `ask me` beside the reset shows them once
/// Custom is chosen; `--mode '` typed there is still the field's text after a session name is added.
#[gpui::test]
fn custom_words_come_back_on_custom_and_survive_an_unrelated_edit(cx: &mut TestAppContext) {
    let words = vec!["--mode".to_owned(), "ask me".to_owned()];
    let stray = TemplateFields {
        kind: Some(LauncherKind::Agent),
        permissions: Some(None),
        permission_flags: Some(words.clone()),
        ..TemplateFields::default()
    };
    let (mut helm, cx) = open_header(cx, &[template("stray", stray)], None);
    open_editor(&mut helm, "template-open-stray", cx);
    assert!(
        !drawn(cx, "templates-flags"),
        "the premise: no words beside the reset"
    );
    // No agent type: the reset and all six approvals, so a select; Custom is its fourth option.
    click(cx, "templates-permissions");
    click(cx, "templates-permissions-option-3");
    assert_eq!(draft(cx).fields.permission_flags, Some(words));
    assert_eq!(flags_text(cx), spelled(&["--mode", "ask me"]));

    click(cx, "templates-flags");
    retype(cx, "--mode '");
    click(cx, "templates-add");
    click(cx, "templates-add-name");
    assert!(
        draft(cx).fields.name.is_some(),
        "the premise: the name was added"
    );
    assert_eq!(flags_text(cx), "--mode '", "the typed text stays");
}

// ---- The dialog around the editor ----------------------------------------------------------------------------------

/// Only Escape closes the "+ Add field" menu on its own; the header Close leaves the dialog, as the old dialog's Close
/// did whatever was open in the editor. A Close that only closed the menu would make the user click it twice.
/// Specified: with the add menu open, a click on the header Close closes the dialog.
#[gpui::test]
fn the_header_close_closes_the_dialog_with_the_add_menu_open(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    click(cx, "templates-add");
    assert!(
        drawn(cx, "templates-add-menu"),
        "the premise: the menu is open"
    );
    click(cx, "templates-close");
    assert!(!any_open(cx), "the dialog closed");
}

/// The dialog's writes go one at a time (SPEC_impl.md, "Launch templates": "Writes are serialized"): a rename writes
/// the preferences too, and two writes on that key at once would refuse one. Specified: while a ⚡ write is in flight,
/// Save sends nothing, though the draft has unsaved changes; once the write is answered, Save writes the template.
#[gpui::test]
fn save_waits_for_a_bolt_write_in_flight(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    click(cx, "templates-suggestion-gpt-6-luna");
    assert!(
        painted(cx).contains("Unsaved changes"),
        "the premise: unsaved changes"
    );
    click(cx, "template-bolt-codex-deep");
    let sent = helm.commands();
    assert_eq!(
        sent.len(),
        1,
        "the premise: the bolt's write is in flight: {sent:?}"
    );
    click(cx, "templates-save");
    assert!(
        helm.commands().is_empty(),
        "Save waits for the bolt's write"
    );
    helm.reply(sent[0].id, 204, None);
    settle(cx);
    click(cx, "templates-save");
    let saved = saves(&mut helm);
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_eq!(saved[0].0, "/api/templates/codex-deep");
}

/// SPEC_impl.md's drafts rule for a keystroke while saving: the text on screen stays the text sent, and an edit made
/// while the save is in flight does not reach the draft (the save's success would otherwise take a draft that was
/// never sent as saved). Specified: with focus put in the name field while the save is in flight, typing changes
/// nothing; once the save is answered, typing reaches the draft again.
#[gpui::test]
fn an_edit_while_saving_changes_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    click(cx, "templates-suggestion-gpt-6-luna");
    click(cx, "templates-save");
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "the premise: the save is in flight");
    click(cx, "templates-name");
    settle(cx);
    assert!(name_focused(cx), "the premise: focus is in the name field");
    cx.simulate_input("-x");
    settle(cx);
    assert_eq!(
        draft(cx).name,
        "codex-deep",
        "the edit did not reach the draft"
    );
    helm.reply(sent[0].id, 503, Some(json!("fixture refusal")));
    settle(cx);
    cx.simulate_input("-y");
    settle(cx);
    assert_eq!(
        draft(cx).name,
        "codex-deep-y",
        "the field takes edits again"
    );
}

/// The old "templates busy root stays keyboard-scrollable": the old editor disabled every control while a save was out,
/// so the dialog itself had to become a stop that the keys scroll, or a keyboard user could not get back to the top of
/// a tall editor. Here the controls stay focusable while busy (the deviation "While a save is in flight") and the
/// dialog's body scrolls whatever focus moves to into view, which is how SPEC.md's "Every scrollable region ... can be
/// reached and scrolled with the keyboard alone" holds for a region of controls. The body also takes Home from a control
/// inside it with no use for the key, which is the old case's own step: Home brought the busy editor back to its top.
/// Specified: in the shortest window, with `codex-deep` edited, reaching Save by Tab scrolls the name field out of the
/// body, and Enter on it sends the save; Shift+Tab, walked back to the name field while the save is still out, scrolls
/// the body until the whole field shows; Tab walked on to Save again scrolls the name field away, and Home there (Save is
/// a button, so the key is the body's, where a text field would keep it for its caret) scrolls the body back until the
/// whole name field shows, focus staying on Save; nothing more is sent.
#[gpui::test]
fn the_busy_editor_scrolls_back_to_its_name_by_keyboard(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    // Edited while the whole editor fits, since a click needs its target on screen.
    click(cx, "templates-suggestion-gpt-6-luna");
    assert!(
        painted(cx).contains("Unsaved changes"),
        "the premise: an edit to save"
    );
    // Then the shortest window SPEC.md allows, so that showing Save scrolls the editor's top well out of view.
    let height = gpui::px(560.);
    cx.simulate_resize(gpui::size(gpui::px(1600.), height));
    settle(cx);
    // Bounded walks: the editor has a few dozen stops at most, so each reaches its target or fails loudly.
    for _ in 0..40 {
        if test_support::focused_id(cx).as_deref() == Some("templates-save") {
            break;
        }
        keys(cx, "tab");
    }
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-save"),
        "the premise: Tab reached Save"
    );
    keys(cx, "enter");
    assert_eq!(helm.commands().len(), 1, "the premise: the save is out");
    // Shown whole: inside the body and inside the window, whichever of the frame's two scrolling parts moved.
    let name_shown = |cx: &mut VisualTestContext| {
        let name = test_support::bounds(cx, "templates-name");
        let body = test_support::bounds(cx, "templates-body");
        let shown = name.top() >= body.top().max(gpui::px(0.))
            && name.bottom() <= body.bottom().min(height);
        (shown, format!("{name:?} in {body:?}"))
    };
    let (shown, at) = name_shown(cx);
    assert!(!shown, "the premise: the name field is scrolled away: {at}");
    for _ in 0..40 {
        if name_focused(cx) {
            break;
        }
        keys(cx, "shift-tab");
    }
    assert!(name_focused(cx), "Shift+Tab reached the name field");
    let (shown, at) = name_shown(cx);
    assert!(shown, "and it was scrolled into view: {at}");
    for _ in 0..40 {
        if test_support::focused_id(cx).as_deref() == Some("templates-save") {
            break;
        }
        keys(cx, "tab");
    }
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-save"),
        "the premise: focus is on Save, a button, again"
    );
    let (shown, at) = name_shown(cx);
    assert!(
        !shown,
        "the premise: the name field is scrolled away again: {at}"
    );
    keys(cx, "home");
    let (shown, at) = name_shown(cx);
    assert!(shown, "Home scrolled the body back to its top: {at}");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-save"),
        "focus stayed on Save"
    );
    assert!(helm.commands().is_empty(), "nothing more was sent");
}

// ---- More fields ----------------------------------------------------------------------------------------------------

/// SPEC.md: "Invisible formatting is shown escaped and read-only, retaining its raw value through unrelated saves", for
/// every text field, not only the name: the old "older template command stays read-only without bypassing save
/// refusal" and "unsafe template model and folder stay raw until explicitly replaced" cases. Specified: typing into an
/// escaped command changes nothing, and an unrelated edit (an added YOLO answer) still cannot save it, since the helm's
/// shape rule refuses the command itself, so nothing is sent; on another template an unrelated save (an added effort
/// reset) sends the escaped model's raw bytes, and a model suggestion then replaces it, after which the escape hint is
/// gone.
#[gpui::test]
fn an_escaped_field_keeps_its_raw_value_until_replaced(cx: &mut TestAppContext) {
    let list = vec![
        template(
            "hidden-command",
            TemplateFields {
                kind: Some(LauncherKind::Command),
                command: Some("make\u{200B}dev".into()),
                ..TemplateFields::default()
            },
        ),
        template(
            "hidden-model",
            TemplateFields {
                kind: Some(LauncherKind::Agent),
                agent: Some(LaunchHarness::Codex),
                model: Some(Some("gpt\u{200B}x".into())),
                ..TemplateFields::default()
            },
        ),
    ];
    let (mut helm, cx) = open_header(cx, &list, None);
    // The escaped fields' two hints and the added rows run the editor past the dialog's height; a taller window keeps
    // Save in view, where a click reaches it.
    cx.simulate_resize(gpui::size(gpui::px(1600.), gpui::px(1600.)));
    settle(cx);
    open_editor(&mut helm, "template-open-hidden-command", cx);
    assert!(
        drawn(cx, "templates-command-escaped"),
        "the premise: the command is escaped"
    );
    click(cx, "templates-command");
    settle(cx);
    cx.simulate_input("x");
    settle(cx);
    assert_eq!(draft(cx).fields.command.as_deref(), Some("make\u{200B}dev"));
    click(cx, "templates-add");
    click(cx, "templates-add-yolo");
    click(cx, "templates-save");
    assert!(
        saves(&mut helm).is_empty(),
        "the command is refused before any write"
    );
    assert!(
        painted(cx).contains("A template's launch command cannot contain"),
        "{}",
        painted(cx)
    );

    // The added YOLO answer is unsaved: leaving asks, and Discard goes on.
    click(cx, "template-open-hidden-model");
    click(cx, "prompt-alternative-0");
    settle(cx);
    assert!(
        drawn(cx, "templates-model-escaped"),
        "the premise: the model is escaped"
    );
    click(cx, "templates-add");
    click(cx, "templates-add-effort");
    click(cx, "templates-save");
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "one save: {sent:?}");
    assert_eq!(sent[0].path, "/api/templates/hidden-model");
    assert_eq!(
        sent[0].body,
        Some(json!({"kind": "agent", "agent": "codex", "model": "gpt\u{200B}x", "effort": null}))
    );
    helm.reply(sent[0].id, 200, None);
    settle(cx);
    click(cx, "templates-suggestion-gpt-6-luna");
    assert_eq!(draft(cx).fields.model, Some(Some("gpt-6-luna".into())));
    assert!(
        !drawn(cx, "templates-model-escaped"),
        "the replaced model is editable"
    );
}

/// SPEC.md: the initial prompt "uses a multiline box that grows with its text", and "Newline and tab remain editable".
/// Specified: Enter in the prompt types a line break rather than saving, and the template is saved with the prompt's
/// lines as typed.
#[gpui::test]
fn the_initial_prompt_keeps_its_line_breaks(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "templates-new", cx);
    cx.simulate_input("prompted");
    settle(cx);
    click(cx, "templates-add");
    click(cx, "templates-add-prompt");
    click(cx, "templates-prompt");
    settle(cx);
    cx.simulate_input("first line");
    keys(cx, "enter");
    cx.simulate_input("second line");
    settle(cx);
    assert!(saves(&mut helm).is_empty(), "Enter did not save");
    assert_eq!(
        draft(cx).fields.prompt.as_deref(),
        Some("first line\nsecond line")
    );
    click(cx, "templates-save");
    assert_eq!(
        saves(&mut helm),
        vec![(
            "/api/templates/prompted".to_owned(),
            json!({"kind": "agent", "prompt": "first line\nsecond line"})
        )]
    );
}

/// SPEC.md: "saving a base name leaves no unsaved changes". A base name is stored as the kind's absence, so the
/// saved draft and what the helm pushes back agree. Specified: a new template with a base session name, once saved
/// and pushed back as the helm stores it, shows no unsaved changes, and neither does reopening it from the list, which
/// compares the stored template (not the editor's own saved draft) with what the editor shows.
#[gpui::test]
fn saving_a_base_name_leaves_no_unsaved_changes(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "templates-new", cx);
    cx.simulate_input("based");
    settle(cx);
    click(cx, "templates-add");
    click(cx, "templates-add-name");
    click(cx, "templates-session-name");
    settle(cx);
    cx.simulate_input("review");
    settle(cx);
    assert_eq!(pressed(cx, "templates-name-kind-Base name"), Some(true));
    click(cx, "templates-save");
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "the premise: one save");
    assert_eq!(
        sent[0].body,
        Some(json!({"kind": "agent", "name": "review"}))
    );
    let mut stored = templates();
    stored.push(template(
        "based",
        serde_json::from_value(json!({"kind": "agent", "name": "review"})).expect("fields decode"),
    ));
    push(&mut helm, &stored, Availability::Available, cx);
    helm.reply(sent[0].id, 200, None);
    settle(cx);
    assert!(!painted(cx).contains("Unsaved changes"), "{}", painted(cx));
    click(cx, "template-open-webbuilder");
    click(cx, "template-open-based");
    settle(cx);
    assert_eq!(
        draft(cx).name,
        "based",
        "the premise: the saved template is open again"
    );
    assert!(
        !painted(cx).contains("Unsaved changes"),
        "reopened: {}",
        painted(cx)
    );
}

/// A shell template can arrive with incompatible stored approvals. Show the actual mode and flag words with the
/// reason Save is refused, so the user can tell what removing the field would drop.
#[gpui::test]
fn a_shell_template_shows_the_permissions_that_keep_it_from_saving(cx: &mut TestAppContext) {
    let shell = template(
        "shell",
        TemplateFields {
            kind: Some(LauncherKind::Agent),
            agent: Some(LaunchHarness::Bash),
            permissions: Some(Some(farhelm_proto::LaunchPermission::Custom)),
            permission_flags: Some(vec!["--profile".into(), "test only".into()]),
            ..TemplateFields::default()
        },
    );
    let (mut helm, cx) = open_header(cx, &[shell], None);
    open_editor(&mut helm, "template-open-shell", cx);
    assert_eq!(draft(cx).fields.agent, Some(LaunchHarness::Bash));
    assert!(drawn(cx, "templates-permissions-refusal"));
    let nodes = frame(cx);
    for (id, label) in [
        ("templates-incompatible-permission-mode", "Custom"),
        (
            "templates-incompatible-permission-flags",
            "--profile 'test only'",
        ),
    ] {
        assert!(
            nodes.iter().any(|node| {
                node.element_id.as_deref() == Some(id) && node.label.as_deref() == Some(label)
            }),
            "the editor shows {label} under {id}"
        );
    }
    click(cx, "templates-name");
    retype(cx, "shell-renamed");
    assert_eq!(
        draft(cx).name,
        "shell-renamed",
        "the premise: a real edit to save"
    );
    click(cx, "templates-save");
    assert!(painted(cx).contains("Approvals: does not fit this launcher switch or agent type"));
    assert!(
        helm.commands().is_empty(),
        "the incompatible template cannot save"
    );
}

/// The template editor's Custom approvals carry their words (the old shared Custom flags block): choosing Custom shows
/// the words field, the words are saved parsed, and choosing another approval drops them, so a template never stores
/// words its choice does not use. Specified: Custom with `--yolo --sandbox off` saves those two words; switching to
/// Guarded then leaves no words in the draft.
#[gpui::test]
fn custom_approvals_carry_their_words(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "templates-new", cx);
    cx.simulate_input("custom");
    settle(cx);
    click(cx, "templates-add");
    click(cx, "templates-add-permissions");
    // No agent type: the reset and all six approvals, so a select; Custom is its fourth option.
    click(cx, "templates-permissions");
    click(cx, "templates-permissions-option-3");
    assert!(drawn(cx, "templates-flag-words") || drawn(cx, "templates-flags-error"));
    click(cx, "templates-flags");
    settle(cx);
    cx.simulate_input("--yolo --sandbox off");
    settle(cx);
    assert!(drawn(cx, "templates-flag-words"), "the words parse");
    click(cx, "templates-save");
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "one save: {sent:?}");
    assert_eq!(
        (sent[0].path.as_str(), sent[0].body.clone()),
        (
            "/api/templates/custom",
            Some(
                json!({"kind": "agent", "permissions": "custom", "permission_flags": ["--yolo", "--sandbox", "off"]})
            )
        )
    );
    helm.reply(sent[0].id, 200, None);
    settle(cx);
    click(cx, "templates-permissions");
    click(cx, "templates-permissions-option-1");
    assert_eq!(
        draft(cx).fields.permission_flags,
        None,
        "the words went with Custom"
    );
    assert!(!drawn(cx, "templates-flags"), "and so did their field");
}

/// Two Claude templates with Custom approvals and their words: `one` with `--mode`, `ask me` and `$HOME`, `other` with
/// `--mode` and `second template words`.
fn custom_templates() -> Vec<LaunchTemplate> {
    let custom = |name: &str, words: &[&str]| {
        template(
            name,
            TemplateFields {
                kind: Some(LauncherKind::Agent),
                agent: Some(LaunchHarness::Claude),
                permissions: Some(Some(farhelm_proto::LaunchPermission::Custom)),
                permission_flags: Some(words.iter().map(|word| (*word).to_owned()).collect()),
                ..TemplateFields::default()
            },
        )
    };
    vec![
        custom("one", &["--mode", "ask me", "$HOME"]),
        custom("other", &["--mode", "second template words"]),
    ]
}

/// `words` as the Custom words field spells them, quoted where a word needs it.
fn spelled(words: &[&str]) -> String {
    let words: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
    farhelm_proto::join_permission_flags(&words)
}

/// The Custom words field keeps what the user typed, quote and all, so a half-typed quote can be repaired in place; but
/// text that does not parse is no words, and saving it would store a template the helm refuses or one with other words
/// than shown (the old custom-permissions spec's template half). Each template's field shows its own stored words, and
/// leaving with Discard puts back what is stored. Specified: `one` opens with its words spelled `--mode 'ask me'
/// '$HOME'`; `--mode '` typed there shows the words error, Save sends nothing and says "Custom permissions need
/// permission flags"; `--mode "edited template words"` saves those two words; `other` then opens with its own words;
/// `--broken '` typed there and `one` opened with Discard shows `one`'s saved words and no error.
#[gpui::test]
fn custom_words_that_do_not_parse_refuse_save_and_each_template_shows_its_own(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = open_header(cx, &custom_templates(), None);
    open_editor(&mut helm, "template-open-one", cx);
    assert_eq!(
        flags_text(cx),
        "--mode 'ask me' '$HOME'",
        "the stored words, quoted where a word needs it"
    );
    assert!(drawn(cx, "templates-flag-words"), "the stored words parse");

    click(cx, "templates-flags");
    retype(cx, "--mode '");
    assert!(
        drawn(cx, "templates-flags-error"),
        "the words error is shown"
    );
    assert_eq!(flags_text(cx), "--mode '", "the text stays as typed");
    click(cx, "templates-save");
    assert!(saves(&mut helm).is_empty(), "nothing was saved");
    assert!(
        painted(cx).contains("Custom permissions need permission flags"),
        "{}",
        painted(cx)
    );

    click(cx, "templates-flags");
    retype(cx, "--mode \"edited template words\"");
    click(cx, "templates-save");
    let (id, body) = one(&mut helm, Method::Put, "/api/templates/one");
    assert_eq!(
        body["permission_flags"],
        json!(["--mode", "edited template words"])
    );
    // The helm pushes the stored template before its reply, as it does for every command.
    let mut stored = custom_templates();
    stored[0].fields.permission_flags = Some(vec!["--mode".into(), "edited template words".into()]);
    push(&mut helm, &stored, Availability::Available, cx);
    helm.reply(id, 200, None);
    settle(cx);
    assert!(
        !painted(cx).contains("Unsaved changes"),
        "the premise: saved"
    );

    click(cx, "template-open-other");
    settle(cx);
    assert_eq!(draft(cx).name, "other", "the premise: other opened");
    assert_eq!(
        flags_text(cx),
        spelled(&["--mode", "second template words"]),
        "the field shows other's words, not the text typed into one's"
    );

    click(cx, "templates-flags");
    retype(cx, "--broken '");
    assert!(
        drawn(cx, "templates-flags-error"),
        "the premise: broken words"
    );
    click(cx, "template-open-one");
    click(cx, "prompt-alternative-0");
    settle(cx);
    assert_eq!(draft(cx).name, "one");
    assert_eq!(
        flags_text(cx),
        spelled(&["--mode", "edited template words"]),
        "one's saved words"
    );
    assert!(!drawn(cx, "templates-flags-error"), "and no error");
}

/// What the open editor's Custom words field holds.
fn flags_text(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| {
        let dialog =
            layers::last_opened::<super::dialog::TemplatesDialog>(cx).expect("a dialog is open");
        let dialog = dialog.read(cx);
        dialog
            .editor
            .as_ref()
            .expect("an editor is open")
            .flags_text(cx)
    })
}

/// The old "older template resume_command / name stays read-only without bypassing save refusal" cases: a field shown
/// escaped is read-only, and a save of an unrelated edit must still refuse its raw value, or the editor would write a
/// template the helm refuses, or one whose launch shows characters the field could not. The command's case is
/// `an_escaped_field_keeps_its_raw_value_until_replaced`. Specified: for a command template whose resume command holds
/// a carriage return and line feed, and one whose session name holds an emoji joiner, typing into the escaped field
/// changes nothing, and Save after an added YOLO answer sends nothing and says "A template's resume command" (or
/// "session name") "cannot contain control or invisible formatting characters".
#[gpui::test]
fn read_only_resume_and_session_name_still_refuse_an_unrelated_save(cx: &mut TestAppContext) {
    let raw = |name: &str, fields: TemplateFields| {
        template(
            name,
            TemplateFields {
                kind: Some(LauncherKind::Command),
                command: Some("echo safe".into()),
                ..fields
            },
        )
    };
    let list = vec![
        raw(
            "raw-name",
            TemplateFields {
                name: Some("joined \u{1F469}\u{200D}\u{1F4BB}".into()),
                ..TemplateFields::default()
            },
        ),
        raw(
            "raw-resume",
            TemplateFields {
                resume_command: Some(Some("echo 'first\r\nsecond'".into())),
                ..TemplateFields::default()
            },
        ),
    ];
    let (mut helm, cx) = open_header(cx, &list, None);
    // Two escaped hints and an added row run the editor past the dialog's height; a taller window keeps Save in view.
    cx.simulate_resize(gpui::size(gpui::px(1600.), gpui::px(1600.)));
    settle(cx);
    let cases = [
        (
            "raw-name",
            "templates-session-name",
            "templates-name-escaped",
            "session name",
        ),
        (
            "raw-resume",
            "templates-resume",
            "templates-resume-escaped",
            "resume command",
        ),
    ];
    for (round, (name, input, escaped, label)) in cases.into_iter().enumerate() {
        if round == 0 {
            open_editor(&mut helm, "template-open-raw-name", cx);
        } else {
            // The previous round's added YOLO answer is unsaved: leaving asks, and Discard goes on.
            click(cx, "template-open-raw-resume");
            click(cx, "prompt-alternative-0");
            settle(cx);
        }
        assert_eq!(draft(cx).name, name, "the premise: {name} is open");
        assert!(
            drawn(cx, escaped),
            "the premise: {name}'s field is shown escaped"
        );
        let before = draft(cx).fields;
        click(cx, input);
        settle(cx);
        cx.simulate_input("x");
        settle(cx);
        // Wherever focus is, the stray `x` must reach nothing that would be saved: the fields and the template's own
        // name are as they were.
        assert_eq!(draft(cx).fields, before, "{name}: typing changed nothing");
        assert_eq!(draft(cx).name, name, "{name}: nor the template's name");
        click(cx, "templates-add");
        click(cx, "templates-add-yolo");
        click(cx, "templates-save");
        assert!(saves(&mut helm).is_empty(), "{name}: nothing was saved");
        let expected = format!(
            "A template's {label} cannot contain control or invisible formatting characters"
        );
        assert!(painted(cx).contains(&expected), "{name}: {}", painted(cx));
    }
}

/// The old "unsafe template model and folder stay raw until explicitly replaced" for the folder, and "initial prompt
/// template escapes formatting while preserving raw saved text": an escaped field's raw value must reach the helm
/// unchanged through a save of an unrelated edit, since the editor saves the whole template and an escaped spelling
/// written back would change the folder a launch starts in. A catalog model is offered escaped too, so a suggestion
/// never hides what it would write. Specified: a Codex template whose folder holds U+202E and whose prompt holds
/// U+202E, with a catalog offering `gpt` + zero-width space + `x`, shows both fields escaped and the suggestion as
/// `gpt<U+200B>x`; Save after an added effort reset sends the folder and the prompt byte for byte.
#[gpui::test]
fn escaped_folder_and_prompt_save_raw_and_a_hidden_suggestion_is_escaped(cx: &mut TestAppContext) {
    let list = vec![template(
        "raw-folder",
        TemplateFields {
            kind: Some(LauncherKind::Agent),
            agent: Some(LaunchHarness::Codex),
            // A model field, so the catalog's suggestions are drawn under it.
            model: Some(None),
            destination: Some(farhelm_proto::launcher::TemplateDestination::Folder(
                "/tmp/\u{202E}project".into(),
            )),
            prompt: Some("hidden\u{202E}text".into()),
            ..TemplateFields::default()
        },
    )];
    let (mut helm, cx) = open_header(cx, &list, None);
    cx.simulate_resize(gpui::size(gpui::px(1600.), gpui::px(1600.)));
    settle(cx);
    click(cx, "templates-button");
    click(cx, "template-open-raw-folder");
    settle(cx);
    let (id, _) = one(&mut helm, Method::Get, "/api/launch-catalog");
    helm.reply(
        id,
        200,
        Some(json!([{"id": "gpt\u{200B}x", "harness": "codex", "efforts": ["high"]}])),
    );
    settle(cx);
    assert!(
        drawn(cx, "templates-destination-escaped"),
        "the folder is escaped"
    );
    assert!(
        drawn(cx, "templates-prompt-escaped"),
        "the prompt is escaped"
    );
    assert!(painted(cx).contains("gpt<U+200B>x"), "{}", painted(cx));
    click(cx, "templates-add");
    click(cx, "templates-add-effort");
    click(cx, "templates-save");
    let saved = saves(&mut helm);
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_eq!(
        saved[0].1["destination"],
        json!({"folder": "/tmp/\u{202E}project"})
    );
    assert_eq!(saved[0].1["prompt"], json!("hidden\u{202E}text"));
}

/// The editor groups the selected agent's older models separately. Its buttons must keep applying the exact ID
/// from that group, so a user can edit an older template without switching it to the current model.
#[gpui::test]
fn the_editor_groups_older_models_for_the_selected_agent(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    cx.simulate_resize(gpui::size(gpui::px(1600.), gpui::px(1300.)));
    settle(cx);
    click(cx, "templates-button");
    click(cx, "template-open-codex-deep");
    settle(cx);
    let (catalog_read, _) = one(&mut helm, Method::Get, "/api/launch-catalog");
    helm.reply(catalog_read, 200, Some(json!([
        {"id":"classic", "family":"runner", "latest":false, "harness":"codex", "efforts":["high"]},
        {"id":"current", "family":"runner", "latest":true, "harness":"codex", "efforts":["high"]},
        {"id":"another-classic", "family":"runner", "latest":false, "harness":"claude", "efforts":["high"]},
    ])));
    settle(cx);
    assert_eq!(draft(cx).fields.agent, Some(LaunchHarness::Codex));
    assert!(!drawn(cx, "templates-suggestion-another-classic"));
    assert!(painted(cx).contains("Older models"));
    let heading = test_support::bounds(cx, "templates-older-models").origin.y;
    assert!(
        test_support::bounds(cx, "templates-suggestion-current")
            .origin
            .y
            < heading
    );
    assert!(
        heading
            < test_support::bounds(cx, "templates-suggestion-classic")
                .origin
                .y
    );

    click(cx, "templates-suggestion-classic");
    assert_eq!(draft(cx).fields.model, Some(Some("classic".into())));
}

/// The editor's quick-launch line tells the user what ⚡ does for the template in front of them, judging the mark on the
/// stored name and eligibility on the draft, as the old editor did (the old "the template mark moves …" case read it).
/// A wrong line would send the user to mark a template that is marked, or one that cannot launch. Specified: with ⚡ on
/// `codex-deep`, its editor says it "starts a session from this saved template"; `webbuilder`'s says "Not the quick
/// launch. The ⚡ in the list makes it one."; an agent template with no agent type says it "Can't be the quick launch".
#[gpui::test]
fn the_quick_launch_line_says_whether_the_template_holds_the_mark(cx: &mut TestAppContext) {
    let mut list = templates();
    list.push(template(
        "no-agent",
        TemplateFields {
            kind: Some(LauncherKind::Agent),
            ..TemplateFields::default()
        },
    ));
    let (mut helm, cx) = open_header(cx, &list, Some("codex-deep"));
    open_editor(&mut helm, "template-open-codex-deep", cx);
    let marked = "starts a session from this saved template";
    let unmarked = "Not the quick launch. The ⚡ in the list makes it one.";
    let ineligible = "Can't be the quick launch";
    let shown = painted(cx);
    assert!(
        shown.contains(marked) && !shown.contains(unmarked),
        "{shown}"
    );
    click(cx, "template-open-webbuilder");
    let shown = painted(cx);
    assert!(
        shown.contains(unmarked) && !shown.contains(marked),
        "{shown}"
    );
    click(cx, "template-open-no-agent");
    let shown = painted(cx);
    assert!(
        shown.contains(ineligible) && !shown.contains(unmarked),
        "{shown}"
    );
}

/// A wrapper prefix that does not parse stays as typed, so a half-typed quote is never lost, and refuses saving rather
/// than being saved as some other prefix. Specified: Farhelm's own placeholder typed as a wrapper prefix stays in the
/// field, Save sends nothing, and the status line gives the reason.
#[gpui::test]
fn a_wrapper_prefix_that_does_not_parse_refuses_save(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "templates-new", cx);
    cx.simulate_input("wrapped");
    settle(cx);
    click(cx, "templates-add");
    click(cx, "templates-add-wrapper");
    click(cx, "templates-wrapper");
    settle(cx);
    cx.simulate_input("env {farhelm_args}");
    settle(cx);
    assert_eq!(
        draft(cx).wrapper_input.as_deref(),
        Some("env {farhelm_args}"),
        "the premise: the text does not parse and is kept as typed"
    );
    click(cx, "templates-save");
    assert!(saves(&mut helm).is_empty(), "nothing was saved");
    assert!(drawn(cx, "templates-editor-status"), "the reason is said");
    assert!(painted(cx).contains("Wrapper prefix"), "{}", painted(cx));
}

// ---- Rename and Duplicate ------------------------------------------------------------------------------------------

/// The helm stores and pushes the preferences holding `marked` as the ⚡ template and `keys` as the number keys.
pub(super) fn push_preferences(
    helm: &mut FakeHelm,
    marked: Option<&str>,
    keys: &[(&str, &str)],
    cx: &mut VisualTestContext,
) {
    let preferences = Preferences {
        quick_launch_template: marked.map(str::to_owned),
        template_shortcuts: keys
            .iter()
            .map(|(key, name)| ((*key).to_owned(), (*name).to_owned()))
            .collect(),
        ..Preferences::default()
    };
    helm.push(CollectionChanges {
        collection: Collection::Preferences,
        meta: test_support::meta(&[PREFERENCES_KEY]),
        upsert: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::to_value(&preferences).expect("preferences encode"),
        }],
        remove: Vec::new(),
    });
    settle(cx);
}

/// The one command sent since the last call, which must be `method` on `path`: its id and body.
pub(super) fn one(helm: &mut FakeHelm, method: Method, path: &str) -> (u64, Value) {
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
    assert_eq!((sent[0].method, sent[0].path.as_str()), (method, path));
    (sent[0].id, sent[0].body.clone().unwrap_or_default())
}

/// `templates()` with `codex-deep` renamed to `codex-deeper` (the new name added, the old one still there or gone).
pub(super) fn renamed(old_kept: bool) -> Vec<LaunchTemplate> {
    let mut list = templates();
    let mut new = list[0].clone();
    new.name = "codex-deeper".into();
    list.insert(1, new);
    if !old_kept {
        list.remove(0);
    }
    list
}

/// SPEC.md: "a rename saves that draft under the new name and then deletes the old one", "Renaming repoints the mark
/// before deleting the old name", "Rename keeps a key". The order matters: deleting first would clear the mark and the
/// key through the helm's delete path, and a failure between steps must leave the old template. Specified: renaming the
/// ⚡ template that holds key 1 writes, one after another and each once the one before has settled, the template under
/// its new name, the mark naming it, the whole key map with key 1 moved and key 2 left alone, and the old name's
/// deletion; a Save press mid-rename sends nothing; afterwards the editor holds the new name, clean, with its row
/// pressed.
#[gpui::test]
fn a_rename_saves_moves_the_mark_and_key_then_deletes_the_old_name(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), Some("codex-deep"));
    push_preferences(
        &mut helm,
        Some("codex-deep"),
        &[("1", "codex-deep"), ("2", "webbuilder")],
        cx,
    );
    open_editor(&mut helm, "template-open-codex-deep", cx);
    cx.simulate_input("er");
    settle(cx);
    click(cx, "templates-save");
    let (id, body) = one(&mut helm, Method::Put, "/api/templates/codex-deeper");
    assert_eq!(
        body,
        json!({"kind": "agent", "agent": "codex", "model": "gpt-6.1-sol", "effort": "high"})
    );
    push(&mut helm, &renamed(true), Availability::Available, cx);
    helm.reply(id, 200, None);
    settle(cx);

    let (id, body) = one(&mut helm, Method::Put, "/api/preferences");
    assert_eq!(body, json!({"quick_launch_template": "codex-deeper"}));
    click(cx, "templates-save");
    assert!(
        helm.commands().is_empty(),
        "Save is busy while the rename runs"
    );
    push_preferences(
        &mut helm,
        Some("codex-deeper"),
        &[("1", "codex-deep"), ("2", "webbuilder")],
        cx,
    );
    helm.reply(id, 204, None);
    settle(cx);

    let (id, body) = one(&mut helm, Method::Put, "/api/preferences");
    assert_eq!(
        body,
        json!({"template_shortcuts": {"1": "codex-deeper", "2": "webbuilder"}})
    );
    push_preferences(
        &mut helm,
        Some("codex-deeper"),
        &[("1", "codex-deeper"), ("2", "webbuilder")],
        cx,
    );
    helm.reply(id, 204, None);
    settle(cx);

    let (id, _) = one(&mut helm, Method::Delete, "/api/templates/codex-deep");
    let remaining = renamed(false);
    let state = super::dialog_tests::templates_state(&remaining, Availability::Available);
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: vec!["codex-deep".into()],
    });
    helm.reply(id, 204, None);
    settle(cx);
    assert!(helm.commands().is_empty(), "the rename is done");
    let text = painted(cx);
    assert!(!text.contains("Unsaved changes"), "{text}");
    assert!(text.contains("Edit template"), "{text}");
    assert_eq!(pressed(cx, "template-open-codex-deeper"), Some(true));
}

/// A template that holds neither the mark nor a key has nothing to move: its rename is the save under the new name and
/// the old name's deletion, with no preference write between them.
#[gpui::test]
fn a_rename_with_nothing_bound_goes_straight_to_the_deletion(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    cx.simulate_input("er");
    settle(cx);
    click(cx, "templates-save");
    let (id, _) = one(&mut helm, Method::Put, "/api/templates/codex-deeper");
    push(&mut helm, &renamed(true), Availability::Available, cx);
    helm.reply(id, 200, None);
    settle(cx);
    one(&mut helm, Method::Delete, "/api/templates/codex-deep");
}

/// A rename that fails between its steps must say so and keep the old template, which still holds the mark (the old
/// dialog's words: "the old template was kept"), and the rename's own preference write must not be reported by the
/// list's refusal line as one of the list's. Specified: after a number-key write from the list succeeded (so the list
/// has a write of its own on record), a rename whose mark move is refused says the new name was saved but the mark could
/// not be moved and the old template was kept; the list's "was not saved" line stays hidden; nothing more is sent, the
/// old name is not deleted, and the draft stays unsaved.
#[gpui::test]
fn a_refused_mark_move_stops_the_rename_and_keeps_the_old_template(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), Some("codex-deep"));
    open_editor(&mut helm, "template-open-codex-deep", cx);
    click(cx, "template-key-webbuilder");
    click(cx, "templates-key-5-item");
    let (id, body) = one(&mut helm, Method::Put, "/api/preferences");
    assert_eq!(
        body,
        json!({"template_shortcuts": {"5": "webbuilder"}}),
        "the premise: the list wrote a key of its own"
    );
    push_preferences(&mut helm, Some("codex-deep"), &[("5", "webbuilder")], cx);
    helm.reply(id, 204, None);
    settle(cx);
    click(cx, "templates-name");
    retype(cx, "codex-deeper");
    click(cx, "templates-save");
    let (id, _) = one(&mut helm, Method::Put, "/api/templates/codex-deeper");
    push(&mut helm, &renamed(true), Availability::Available, cx);
    helm.reply(id, 200, None);
    settle(cx);
    let (id, _) = one(&mut helm, Method::Put, "/api/preferences");
    helm.reply(id, 503, Some(json!("fixture preference save refused")));
    settle(cx);
    let text = painted(cx);
    assert!(
        text.contains(
            "The new name was saved, but the quick launch mark could not be moved; the old template was kept"
        ),
        "{text}"
    );
    assert!(
        !text.contains("was not saved"),
        "the list's own refusal line does not take the editor's write for its own: {text}"
    );
    assert!(text.contains("Unsaved changes"), "{text}");
    assert!(helm.commands().is_empty(), "the old name was not deleted");
}

/// Moving the old name's number key is a step of its own, and its refusal must stop the rename before the old name is
/// deleted, since the helm's delete clears the old name's key. Specified: renaming a template that holds key 1 but not
/// ⚡, with the key map's write refused, says its number key could not be moved and the old template was kept, and
/// sends no deletion.
#[gpui::test]
fn a_refused_key_move_stops_the_rename_and_keeps_the_old_template(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    push_preferences(&mut helm, None, &[("1", "codex-deep")], cx);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    cx.simulate_input("er");
    settle(cx);
    click(cx, "templates-save");
    let (id, _) = one(&mut helm, Method::Put, "/api/templates/codex-deeper");
    push(&mut helm, &renamed(true), Availability::Available, cx);
    helm.reply(id, 200, None);
    settle(cx);
    let (id, body) = one(&mut helm, Method::Put, "/api/preferences");
    assert_eq!(body, json!({"template_shortcuts": {"1": "codex-deeper"}}));
    helm.reply(id, 503, Some(json!("fixture preference save refused")));
    settle(cx);
    let text = painted(cx);
    assert!(
        text.contains(
            "The new name was saved, but its number key could not be moved; the old template was kept"
        ),
        "{text}"
    );
    assert!(helm.commands().is_empty(), "the old name was not deleted");
}

/// Without the shortcut bindings the rename cannot tell whether the old name holds ⚡ or a key, and deleting it would
/// lose them through the helm's delete path, so it stops (the old dialog's "shortcut bindings could not be read").
/// Specified: when the helm reports its preferences unavailable by the time the new name is saved, the status line
/// says the bindings could not be read and the old template was kept, with the helm's reason, and nothing more is sent.
#[gpui::test]
fn unreadable_bindings_stop_the_rename_before_any_move(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), Some("codex-deep"));
    open_editor(&mut helm, "template-open-codex-deep", cx);
    cx.simulate_input("er");
    settle(cx);
    click(cx, "templates-save");
    let (id, _) = one(&mut helm, Method::Put, "/api/templates/codex-deeper");
    push(&mut helm, &renamed(true), Availability::Available, cx);
    helm.push(CollectionChanges {
        collection: Collection::Preferences,
        meta: CollectionMeta {
            availability: Availability::Unavailable {
                error: "database is locked".into(),
            },
            ..test_support::meta(&[PREFERENCES_KEY])
        },
        upsert: Vec::new(),
        remove: Vec::new(),
    });
    helm.reply(id, 200, None);
    settle(cx);
    let text = painted(cx);
    assert!(
        text.contains(
            "The new name was saved, but shortcut bindings could not be read; the old template was kept: database is locked"
        ),
        "{text}"
    );
    assert!(
        helm.commands().is_empty(),
        "no move and no deletion was sent"
    );
}

/// The old dialog showed only the helm's words when deleting the old name was refused, and so does this one (recorded
/// in the parity file). Specified: renaming a template that holds neither ⚡ nor a key, with the deletion of the old
/// name refused, shows the helm's words in the status line and leaves the draft unsaved.
#[gpui::test]
fn a_refused_delete_of_the_old_name_shows_the_helms_words(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    cx.simulate_input("er");
    settle(cx);
    click(cx, "templates-save");
    let (id, _) = one(&mut helm, Method::Put, "/api/templates/codex-deeper");
    push(&mut helm, &renamed(true), Availability::Available, cx);
    helm.reply(id, 200, None);
    settle(cx);
    let (id, _) = one(&mut helm, Method::Delete, "/api/templates/codex-deep");
    helm.reply(id, 409, Some(json!("fixture delete refused")));
    settle(cx);
    let text = painted(cx);
    assert!(text.contains("fixture delete refused"), "{text}");
    assert!(text.contains("Unsaved changes"), "{text}");
    assert!(helm.commands().is_empty(), "nothing more was sent");
}

/// SPEC.md: "Duplicate opens an unsaved copy named `<name>-copy`, then `<name>-copy-2` and so on until free". The copy
/// is of the saved template, not of edits the user has not saved (the old dialog copied its baseline), and it is a new
/// template that needs Save. Specified: with `codex-deep-copy` taken and an unsaved model edit, Duplicate, once the
/// edit is discarded, opens a new template named `codex-deep-copy-2` holding the stored model, unsaved, with focus on
/// its name and no Duplicate of its own; its Save writes it under that name.
#[gpui::test]
fn duplicate_opens_an_unsaved_copy_of_the_saved_template(cx: &mut TestAppContext) {
    let mut list = templates();
    let mut taken = list[0].clone();
    taken.name = "codex-deep-copy".into();
    list.insert(1, taken);
    let (mut helm, cx) = open_header(cx, &list, None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    click(cx, "templates-suggestion-gpt-6-luna");
    assert!(
        painted(cx).contains("Unsaved changes"),
        "the premise: an unsaved edit"
    );
    click(cx, "templates-duplicate");
    click(cx, "prompt-alternative-0");
    settle(cx);
    let copy = draft(cx);
    assert_eq!(copy.name, "codex-deep-copy-2");
    assert_eq!(copy.fields.model, Some(Some("gpt-6.1-sol".into())));
    let text = painted(cx);
    assert!(text.contains("New template"), "{text}");
    assert!(text.contains("Unsaved changes"), "{text}");
    assert!(name_focused(cx));
    assert!(
        !drawn(cx, "templates-duplicate"),
        "a new template has no Duplicate"
    );
    assert!(helm.commands().is_empty(), "Duplicate sends nothing");
    click(cx, "templates-save");
    one(&mut helm, Method::Put, "/api/templates/codex-deep-copy-2");
}

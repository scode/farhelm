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
use farhelm_proto::ui_channel::Availability;
use farhelm_proto::{LaunchEffort, LaunchHarness};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::dialog::Opened;
use super::dialog_tests::{any_open, open_header, painted, push, template};
use super::fields::Draft;
use crate::components::EditCommand;
use crate::focus;
use crate::test_support::{self, click, frame, keys, settle};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The templates most tests start with, in the helm's order (by name): one with a launch kind, which opens clean, one
/// without, which opens with its switch inferred, and a command launch.
fn templates() -> Vec<LaunchTemplate> {
    vec![
        template(
            "codex-deep",
            TemplateFields {
                kind: Some(LauncherKind::Agent),
                agent: Some(LaunchHarness::Codex),
                model: Some(Some("gpt-6.1-sol".into())),
                effort: Some(Some(LaunchEffort::High)),
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
fn open_editor(helm: &mut FakeHelm, opener: &'static str, cx: &mut VisualTestContext) {
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
fn draft(cx: &mut VisualTestContext) -> Draft {
    cx.update(|_, cx| {
        let dialog = cx
            .try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade())
            .expect("a dialog is open");
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
fn name_focused(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| {
        let dialog = cx
            .try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade())
            .expect("a dialog is open");
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
fn drawn(cx: &mut VisualTestContext, id: &str) -> bool {
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
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "templates-new", cx);
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
    retype(cx, "codex-deep");
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
        text.contains("A template named \"codex-deep\" already exists"),
        "{text}"
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

/// The session name's kind and the destination's kind are choices beside their text (SPEC.md: "base name | fixed
/// name"; "a folder or a managed checkout"), and both reach the saved template: Fixed is stored, Base is the kind's
/// absence, and switching the destination keeps its text. Specified: a new template with a fixed session name and a
/// managed checkout saves exactly those.
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

    click(cx, "template-open-hidden-model");
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

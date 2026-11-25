//! Golden scenes for templates: the Templates dialog, its editor and the template launcher. The scene list itself,
//! `SCENES` with each scene's name and size, stays in `mod.rs`.

use crate::focus::Mark;
use crate::layers;
use crate::{session, test_support};
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::launcher::TemplateFields;
use farhelm_proto::ui_channel::{Collection, CollectionState, KeyedEntry, PREFERENCES_KEY};
use farhelm_proto::{LaunchHarness, LaunchPermission};
use gpui::{AnyElement, App, IntoElement, Window, div};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{Pending, SceneState, take_pending};

/// The Templates dialog as the list header's Templates button opens it (reference `templates-dialog`): three templates
/// with summaries of different shapes, the first holding ⚡ (so its row has the chord line and the filled amber bolt)
/// and the second holding number key 2 and an unavailable approval mode. Focus starts on the dialog itself, so no
/// ring shows. The fake and the connection are kept for the scene's life, like Settings'.
pub(super) fn templates_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let templates = [
        serde_json::json!({"name": "my-codex", "fields": {"kind": "agent", "agent": "codex", "model": "gpt-6-luna", "effort": "high"}}),
        serde_json::json!({"name": "review", "fields": {"agent": "claude", "permissions": "approve", "name": "review"}}),
        serde_json::json!({"name": "webbuilder", "fields": {"kind": "command", "command": "make dev", "yolo": false, "destination": {"folder": "/src/web"}}}),
    ];
    helm.connect(vec![
        CollectionState {
            collection: Collection::Templates,
            meta: test_support::meta(&["my-codex", "review", "webbuilder"]),
            entries: templates
                .iter()
                .map(|value| KeyedEntry {
                    key: value["name"].as_str().unwrap_or_default().into(),
                    value: value.clone(),
                })
                .collect(),
        },
        CollectionState {
            collection: Collection::Preferences,
            meta: test_support::meta(&[PREFERENCES_KEY]),
            entries: vec![KeyedEntry {
                key: PREFERENCES_KEY.into(),
                value: serde_json::json!({
                    "quick_launch_template": "my-codex",
                    "template_shortcuts": {"2": "review"},
                }),
            }],
        },
    ]);
    // Open once the connection has a UI session, as Settings' scene does. The scene's ring handle stands in for the
    // Templates button.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::templates::open(&opener, &mark, window, cx);
        });
    });
}

/// The reference set's templates for the editor scenes (references `template-editor` and `template-number-menu`): a
/// placement template with a long wrapper summary, the one the editor opens, and a Claude one, with number keys
/// 1 and 8 bound as the reference stack bound them. The launcher scene shows the ellipsis in the wrapper's face.
const EDITOR_SCENE_TEMPLATES: [(&str, &str); 3] = [
    (
        "api-repo",
        r#"{"host": "install-build-box", "destination": {"folder": "/src/api"}, "wrapper_prefix": ["env", "CACHE_DIR=/var/cache/agent-builds", "WORKSPACE_MODE=review-only-with-template-preflight-checks"]}"#,
    ),
    (
        "codex-deep",
        r#"{"kind": "agent", "agent": "codex", "model": "gpt-6.1-sol", "effort": "high"}"#,
    ),
    (
        "quick-claude",
        r#"{"kind": "agent", "agent": "claude", "permissions": "yolo"}"#,
    ),
];

/// The editor scenes' snapshot: [`EDITOR_SCENE_TEMPLATES`] and number keys 1 and 8 on `codex-deep` and
/// `quick-claude`, nothing marked ⚡. The template list's scene shows the same reference stack.
fn editor_scene_collections() -> Vec<CollectionState> {
    vec![
        CollectionState {
            collection: Collection::Templates,
            meta: test_support::meta(&["api-repo", "codex-deep", "quick-claude"]),
            entries: EDITOR_SCENE_TEMPLATES
                .iter()
                .map(|(name, fields)| KeyedEntry {
                    key: (*name).into(),
                    value: serde_json::json!({
                        "name": name,
                        "fields": serde_json::from_str::<serde_json::Value>(fields).unwrap_or_default(),
                    }),
                })
                .collect(),
        },
        CollectionState {
            collection: Collection::Preferences,
            meta: test_support::meta(&[PREFERENCES_KEY]),
            entries: vec![KeyedEntry {
                key: PREFERENCES_KEY.into(),
                value: serde_json::json!({
                    "template_shortcuts": {"1": "codex-deep", "8": "quick-claude"},
                }),
            }],
        },
    ]
}

/// The Templates dialog with `codex-deep` open in its editor, as a click on its row opens it (reference
/// `template-editor`): focus in its name field, where opening a row puts it, and its model suggestions from the
/// catalog read the editor sends, which is answered at once with the reference stack's four Codex models. The fake,
/// the connection and the watch are kept for the scene's life, like Settings'.
pub(super) fn template_editor_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    open_template_editor(state, "codex-deep", None, window, cx);
}

/// An older template can still be opened, but Undo could not resave its invalid custom model.
pub(super) fn template_invalid_custom_model_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    open_template_editor(state, "old-custom", None, window, cx);
}

/// Show the incompatible choice without erasing it when the template uses the Agent tab.
pub(super) fn template_generic_agent_refusal_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    open_template_editor(state, "generic-agent", None, window, cx);
}

/// The same literal effort is valid without an agent and incompatible with an explicit built-in agent.
pub(super) fn template_literal_effort_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    open_template_editor(state, "effort-only", None, window, cx);
}

pub(super) fn template_literal_effort_refusal_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    open_template_editor(state, "codex-effort", None, window, cx);
}

/// Open the template that sets both Host and Destination so the shared switch and separate fields are visible.
pub(super) fn template_destination_editor_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let mut hosts = super::hosts(&[super::remote(2, "build.example.invalid").alias("Build box")]);
    hosts.entries[0].value["identity"] = serde_json::json!("install-build-box");
    open_template_editor(state, "api-repo", Some(hosts), window, cx);
}

/// Use the same partial template, and rest on its real wrapper field to show the help from the editor.
pub(super) fn template_wrapper_help_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    template_destination_editor_setup(state, window, cx);
    window.set_driver_enabled(true);
    state.hover.set(Some("templates-wrapper"));
}

/// The same stored definition in the editor and the approval card, including a permission a shell cannot use.
/// Its launch kind is omitted so the compact list has room for the stored approval mode.
pub(super) fn shell_template_fields() -> TemplateFields {
    TemplateFields {
        agent: Some(LaunchHarness::Bash),
        permissions: Some(Some(LaunchPermission::Custom)),
        permission_flags: Some(vec!["--profile".into(), "review only".into()]),
        ..TemplateFields::default()
    }
}

/// Open a stored shell template in the ordinary editor so it shows the value and the existing save refusal.
pub(super) fn shell_template_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    open_template_editor(state, "shell-profile", None, window, cx);
}

fn open_template_editor(
    state: &SceneState,
    template: &'static str,
    hosts: Option<CollectionState>,
    window: &mut Window,
    cx: &mut App,
) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let mut snapshot = editor_scene_collections();
    let special = match template {
        "old-custom" => {
            snapshot.push(super::custom_agent_state());
            Some(
                serde_json::json!({"kind": "agent", "agent": "custom", "custom_agent": "acme-code", "model": "BAD MODEL"}),
            )
        }
        "generic-agent" => Some(serde_json::json!({"kind": "agent", "agent": "custom"})),
        "effort-only" => Some(serde_json::json!({"kind": "agent", "effort": "deep_focus"})),
        "codex-effort" => Some(
            serde_json::json!({"kind": "agent", "agent": "codex", "effort": "deep_focus_".repeat(23)}),
        ),
        _ => None,
    };
    if let Some(fields) = special {
        snapshot[0] = CollectionState {
            collection: Collection::Templates,
            meta: test_support::meta(&[template]),
            entries: vec![KeyedEntry {
                key: template.into(),
                value: serde_json::json!({"name": template, "fields": fields}),
            }],
        };
    }
    if template == "shell-profile" {
        let templates = &mut snapshot[0];
        templates.meta = test_support::meta(&["shell-profile", "shell-direct", "quick-claude"]);
        templates.entries[0] = KeyedEntry {
            key: template.into(),
            value: serde_json::json!({"name": template, "fields": shell_template_fields()}),
        };
        // The compact list shows a complete mode on a second template without flag words after it.
        templates.entries[1] = KeyedEntry {
            key: "shell-direct".into(),
            value: serde_json::json!({
                "name": "shell-direct", "fields": {"agent": "zsh", "permissions": "yolo"}
            }),
        };
    }
    snapshot.extend(hosts);
    helm.connect(snapshot);
    let handle = window.window_handle();
    let opener = state.ring.clone();
    let helm = Rc::new(RefCell::new(helm));
    let answering = helm.clone();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::templates::open(&opener, &mark, window, cx);
            if let Some(dialog) =
                layers::last_opened::<crate::templates::dialog::TemplatesDialog>(cx)
            {
                dialog.update(cx, |dialog, cx| dialog.open_template(template, window, cx));
            }
        });
        let mut helm = answering.borrow_mut();
        for read in helm.commands() {
            if read.path == "/api/launch-catalog" {
                let models = ["gpt-6-luna", "gpt-5.6-terra", "gpt-6.1-sol", "gpt-6-astra"];
                let catalog: Vec<serde_json::Value> = models
                    .iter()
                    .map(|id| {
                        serde_json::json!({"id": id, "harness": "codex", "efforts": ["low", "medium", "high"]})
                    })
                    .collect();
                helm.reply(read.id, 200, Some(serde_json::Value::Array(catalog)));
            }
        }
    });
}

/// [`template_editor_setup`], then (reference `template-number-menu`) `codex-deep`'s number-key menu open over it, as
/// a click on its key button opens it: the template's name over "Number shortcut" in the shared menu header, then No
/// shortcut and the ten keys, 1 held by this template and 8 by `quick-claude`.
pub(super) fn template_number_menu_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    template_editor_setup(state, window, cx);
    state
        .keep
        .borrow_mut()
        .push(Box::new(Pending(Cell::new(true))));
}

/// Nothing on the page, and once the dialog has drawn `codex-deep`'s key button, one click of it after that frame.
pub(super) fn template_number_menu_scene(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let dialog = layers::last_opened::<crate::templates::dialog::TemplatesDialog>(cx);
    if let Some(dialog) = dialog
        && dialog.read(cx).key_button_drawn("codex-deep")
        && take_pending(state)
    {
        window.defer(cx, move |window, cx| {
            dialog.update(cx, |dialog, cx| {
                dialog.toggle_key_menu("codex-deep", window, cx)
            })
        });
    }
    div().into_any_element()
}

/// The template list as Cmd+J opens it (reference `template-launcher`): the editor scenes' snapshot
/// ([`editor_scene_collections`]), so `codex-deep` and `quick-claude` show their key chips and `api-repo`, first, is
/// selected; focus in the search field. The list asks the session list to show Sessions after a pick, and the scene has
/// none, so it is handed a list that is gone. The fake, the connection and the watch are kept for the scene's life,
/// like Settings'.
pub(super) fn template_launcher_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(editor_scene_collections());
    let handle = window.window_handle();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let _ = handle.update(cx, |_, window, cx| {
            crate::session_list::quick_switcher::open(
                gpui::WeakEntity::new_invalid(),
                crate::session_list::quick_switcher::Mode::Templates,
                window,
                cx,
            );
        });
    });
}

/// [`template_editor_setup`], then (reference `template-leave-question`) the name changed to `codex-deep-unsaved` and the
/// header Close pressed: "Unsaved changes" in the heading behind the centered leave question, focus on its Save.
pub(super) fn template_leave_question_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    template_editor_setup(state, window, cx);
    state
        .keep
        .borrow_mut()
        .push(Box::new(Pending(Cell::new(true))));
}

/// Nothing on the page, and once the dialog has drawn its editor, the edit and the Close after that frame.
pub(super) fn template_leave_question_scene(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let dialog = layers::last_opened::<crate::templates::dialog::TemplatesDialog>(cx);
    if let Some(dialog) = dialog
        && dialog.read(cx).key_button_drawn("codex-deep")
        && take_pending(state)
    {
        window.defer(cx, move |window, cx| {
            dialog.update(cx, |dialog, cx| dialog.ask_leave_for_golden(window, cx))
        });
    }
    div().into_any_element()
}

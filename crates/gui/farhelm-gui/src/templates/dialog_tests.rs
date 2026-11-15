//! Tests of the Templates dialog over the helm client's fake: what opens it, what its list shows and how the list
//! follows the store, the hint lines, and closing.
//!
//! The dialog is opened the way the list header's Templates button will open it: a button in a root view of the test's
//! own, whose click takes a mark and calls [`super::open`] (or [`super::open_with_hint`], as a shortcut will). Every
//! step ends in one of the shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::LaunchHarness;
use farhelm_proto::launcher::{LaunchTemplate, LauncherKind, TemplateFields};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, KeyedEntry,
    PREFERENCES_KEY, Preferences,
};
use gpui::{
    Context, FocusHandle, IntoElement, ParentElement as _, Render, Styled as _, TestAppContext,
    VisualTestContext, Window, div, px, size,
};

use super::Hint;
use crate::components::{Button, HoverHelp, Role};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::test_support::{self, click, frame, keys, meta, settle, started};
use crate::{layers, root};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The root view the tests open Templates from: the Templates button the list header will draw, and a second button
/// standing in for a shortcut that found nothing to launch and opens Templates with a hint.
pub(super) struct Header {
    pub(super) root: root::Attachment,
    /// The Templates button's focus handle, which the dialog returns focus to when it closes.
    pub(super) templates: FocusHandle,
}

impl Header {
    /// The header of a new window, its Templates button with a focus handle of its own.
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Header {
        Header {
            root: root::attach(window, cx),
            templates: cx.focus_handle().tab_stop(true),
        }
    }
}

impl Render for Header {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let templates = self.templates.clone();
        let content = div()
            .size_full()
            .p(px(20.))
            .child(
                Button::new(
                    "templates-button",
                    "Templates",
                    super::button_help(cx),
                    Role::Button,
                )
                .focus(&self.templates)
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    super::open(&templates, &mark, window, cx);
                }),
            )
            .child(
                Button::new(
                    "unbound-key",
                    "Unbound number key",
                    HoverHelp::tooltip("Stands in for a number key with no template"),
                    Role::Button,
                )
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    super::open_with_hint(Hint::NumberKey, &mark, window, cx);
                }),
            );
        root::wrap(content, &self.root, window, cx)
    }
}

pub(super) fn template(name: &str, fields: TemplateFields) -> LaunchTemplate {
    LaunchTemplate {
        name: name.into(),
        fields,
    }
}

/// The two templates most tests start with, in the helm's order (by name).
fn templates() -> Vec<LaunchTemplate> {
    vec![
        template(
            "my-codex",
            TemplateFields {
                agent: Some(LaunchHarness::Codex),
                model: Some(Some("gpt-6-luna".into())),
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

/// The templates collection holding `templates`, in their order, available or not.
pub(super) fn templates_state(
    templates: &[LaunchTemplate],
    availability: Availability,
) -> CollectionState {
    let keys: Vec<&str> = templates
        .iter()
        .map(|template| template.name.as_str())
        .collect();
    CollectionState {
        collection: Collection::Templates,
        meta: CollectionMeta {
            availability,
            ..meta(&keys)
        },
        entries: templates
            .iter()
            .map(|template| KeyedEntry {
                key: template.name.clone(),
                value: serde_json::to_value(template).unwrap(),
            })
            .collect(),
    }
}

/// The preferences collection, with `marked` as the ⚡ template.
pub(super) fn preferences_state(marked: Option<&str>) -> CollectionState {
    let preferences = Preferences {
        quick_launch_template: marked.map(str::to_owned),
        ..Preferences::default()
    };
    CollectionState {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::to_value(&preferences).unwrap(),
        }],
    }
}

/// The helm pushes the templates collection as `templates`.
pub(super) fn push(
    helm: &mut FakeHelm,
    templates: &[LaunchTemplate],
    availability: Availability,
    cx: &mut VisualTestContext,
) {
    let state = templates_state(templates, availability);
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
}

/// The app over the fake helm holding `templates` and a preference row marking `marked`, in a window large enough for
/// the whole dialog. Nothing is open yet.
pub(super) fn open_header<'a>(
    cx: &'a mut TestAppContext,
    templates: &[LaunchTemplate],
    marked: Option<&str>,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![
        templates_state(templates, Availability::Available),
        preferences_state(marked),
    ]);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, Header::new);
    window.simulate_resize(size(px(1600.), px(1000.)));
    settle(window);
    (helm, window)
}

pub(super) fn any_open(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| layers::any_open(window, cx))
}

/// Every line of text the window painted, joined with single spaces.
pub(super) fn painted(cx: &mut VisualTestContext) -> String {
    let lines: Vec<String> = frame(cx)
        .into_iter()
        .flat_map(|node| node.texts.into_iter().map(|line| line.text.to_string()))
        .collect();
    lines
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The accessible names of the list's rows, in drawn order.
fn row_names(cx: &mut VisualTestContext) -> Vec<String> {
    frame(cx)
        .into_iter()
        .filter(|node| {
            node.element_id
                .as_deref()
                .is_some_and(|id| id.starts_with("template-row-"))
        })
        .filter_map(|node| node.label.map(|label| label.to_string()))
        .collect()
}

// ---- Opening and the list -------------------------------------------------------------------------------------------

/// SPEC.md: "A list of names and summaries sits beside the selected template's editor". The Templates button opens one
/// dialog titled Templates whose list shows every template in the helm's order, each name over its summary, with the
/// count under the list. This is what every later piece of the dialog builds on.
#[gpui::test]
fn the_button_opens_the_list_of_names_and_summaries(cx: &mut TestAppContext) {
    let (_helm, cx) = open_header(cx, &templates(), None);
    click(cx, "templates-button");
    assert!(any_open(cx), "the button opened the dialog");
    assert_eq!(row_names(cx), vec!["my-codex", "webbuilder"]);
    let text = painted(cx);
    for expected in [
        "Templates",
        "Saved edits to combine in the session launcher",
        "Codex · gpt-6-luna",
        "Command launch · Command",
        "2 templates",
    ] {
        assert!(
            text.contains(expected),
            "{expected:?} is painted in {text:?}"
        );
    }
    assert!(
        !text.contains("Give a template a number key"),
        "no hint without a shortcut"
    );
}

/// The list draws from the store, so a template saved anywhere (another editor, an agent, the command line) shows up
/// in an open dialog at once, and the count follows; nothing in the dialog holds a copy that could go stale.
#[gpui::test]
fn the_list_follows_the_store_while_open(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    click(cx, "templates-button");
    let mut grown = templates();
    grown.insert(0, template("aaa-review", TemplateFields::default()));
    push(&mut helm, &grown, Availability::Available, cx);
    assert_eq!(row_names(cx), vec!["aaa-review", "my-codex", "webbuilder"]);
    let text = painted(cx);
    assert!(text.contains("Sets nothing yet"), "{text}");
    assert!(text.contains("3 templates"), "{text}");
}

/// When the helm cannot read its templates, the dialog says so in the helm's words and keeps showing the templates it
/// had, rather than an empty list that would read as "you have no templates".
#[gpui::test]
fn an_unavailable_list_says_so_and_keeps_the_last_templates(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    click(cx, "templates-button");
    push(
        &mut helm,
        &templates(),
        Availability::Unavailable {
            error: "database is locked".into(),
        },
        cx,
    );
    let text = painted(cx);
    assert!(
        text.contains("Templates unavailable: database is locked"),
        "{text}"
    );
    assert_eq!(row_names(cx), vec!["my-codex", "webbuilder"]);
}

/// Template names are host-supplied text (an agent may have written one), so a name carrying invisible formatting is
/// drawn escaped, never as the raw characters that could make two names look alike. (The name may be cut short in the
/// narrow list column, so only the escape's start is checked.)
#[gpui::test]
fn a_template_name_is_drawn_as_host_text(cx: &mut TestAppContext) {
    let sneaky = vec![template("my\u{200B}codex", TemplateFields::default())];
    let (_helm, cx) = open_header(cx, &sneaky, None);
    click(cx, "templates-button");
    let text = painted(cx);
    assert!(
        text.contains("my<U+200B"),
        "the invisible character is drawn escaped: {text}"
    );
    assert!(
        !text.contains("my\u{200B}codex"),
        "the raw character is never painted"
    );
}

/// A shortcut that found nothing to launch opens Templates with the line saying how to bind one (SPEC.md: "Unbound
/// keys open Templates with a binding hint"); the Templates button opens it without.
#[gpui::test]
fn a_shortcut_opens_the_dialog_with_its_hint(cx: &mut TestAppContext) {
    let (_helm, cx) = open_header(cx, &templates(), None);
    click(cx, "unbound-key");
    assert!(any_open(cx));
    let text = painted(cx);
    assert!(
        text.contains("Give a template a number key with the key button beside its ⚡;"),
        "{text}"
    );
}

/// Escape closes the dialog, as its header Close does; nothing is unsaved yet to ask about.
#[gpui::test]
fn escape_closes_the_dialog(cx: &mut TestAppContext) {
    let (_helm, cx) = open_header(cx, &templates(), None);
    click(cx, "templates-button");
    assert!(any_open(cx), "the premise: the dialog is open");
    keys(cx, "escape");
    assert!(!any_open(cx));
}

/// The Templates button names itself as where closing returns focus, so focus comes back to it on every platform. On
/// macOS a click leaves a button unfocused, and returning focus to what held it before the click would leave the user
/// somewhere other than the button they used (the old app returned focus to the Templates button by name). Specified:
/// with clicks not focusing controls, as on macOS, the button opens the dialog without holding focus, and Escape puts
/// focus on it.
#[gpui::test]
fn closing_returns_focus_to_the_templates_button(cx: &mut TestAppContext) {
    let (_helm, cx) = open_header(cx, &templates(), None);
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    assert_ne!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-button"),
        "the premise: the button does not hold focus before the click"
    );
    click(cx, "templates-button");
    assert!(any_open(cx), "the premise: the dialog opened");
    keys(cx, "escape");
    assert!(!any_open(cx));
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-button")
    );
}

/// A shortcut that opens Templates with a hint is a key press, which moved no focus, so closing returns focus to
/// wherever it was (SPEC.md: "Closing something the user opened returns focus to where it was before it opened"),
/// usually the terminal. Specified: with focus on one control (the Templates button) and clicks not focusing controls,
/// as on macOS, a press of the stand-in shortcut's button opens Templates through the hint entry point, and Escape puts
/// focus back on the Templates button, where it was, not on the button that was pressed.
#[gpui::test]
fn closing_a_hint_opened_dialog_returns_focus_where_it_was(cx: &mut TestAppContext) {
    let (_helm, cx) = open_header(cx, &templates(), None);
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    // Tab to the Templates button: the header has only a few stops, so a bounded walk finds it or fails loudly.
    for _ in 0..4 {
        if test_support::focused_id(cx).as_deref() == Some("templates-button") {
            break;
        }
        keys(cx, "tab");
    }
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-button"),
        "the premise: focus starts on the Templates button"
    );
    click(cx, "unbound-key");
    assert!(any_open(cx), "the premise: the dialog opened");
    assert_ne!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-button"),
        "the premise: focus moved into the dialog"
    );
    keys(cx, "escape");
    assert!(!any_open(cx));
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-button")
    );
}

/// The header Close closes the dialog through the layer manager's dismissal, the same path Escape takes, so the editor's
/// leave question can later hold both.
#[gpui::test]
fn the_header_close_closes_the_dialog(cx: &mut TestAppContext) {
    let (_helm, cx) = open_header(cx, &templates(), None);
    click(cx, "templates-button");
    click(cx, "templates-close");
    assert!(!any_open(cx));
}

// ---- The Templates button's hover help --------------------------------------------------------------------------------

/// SPEC.md: "Templates' hover ... advertise the chord and marked name". With no mark the button's help says how to get
/// one; with one it names the marked template as a host value, read from the store each time the button draws, so a
/// mark set anywhere shows at once.
#[gpui::test]
fn the_button_help_names_the_chord_and_the_marked_template(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    let help = cx.update(|_, cx| super::button_help(cx));
    assert_eq!(
        help,
        HoverHelp::tooltip(format!(
            "Templates: mark one with ⚡ for quick launch ({})",
            super::QUICK_LAUNCH_SHORTCUT_NAME
        ))
    );

    let state = preferences_state(Some("my-codex"));
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
    let help = cx.update(|_, cx| super::button_help(cx));
    assert_eq!(
        help,
        HoverHelp::sentence(
            HostSentence::new("templates-button-help")
                .text("Templates: quick launch ")
                .host(&HostText::from_host("my-codex"))
                .text(format!(" ({})", super::QUICK_LAUNCH_SHORTCUT_NAME))
        )
    );
}

/// The count line's singular, as the old list spelled it.
#[test]
fn one_template_is_counted_in_the_singular() {
    assert_eq!(super::dialog::count_text(1), "1 template");
    assert_eq!(super::dialog::count_text(0), "0 templates");
}

// ---- ⚡ and the number keys ------------------------------------------------------------------------------------------

/// The one command sent since the last call, which must be a preference write: its id and its body.
fn one_write(helm: &mut FakeHelm) -> (u64, serde_json::Value) {
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
    assert_eq!(
        (sent[0].method, sent[0].path.as_str()),
        (farhelm_helm_client::Method::Put, "/api/preferences")
    );
    (sent[0].id, sent[0].body.clone().unwrap_or_default())
}

/// The helm stores and pushes `preferences`, then replies to the write `id`, in that order, as the channel delivers a
/// command's changes before its reply.
fn store_and_reply(
    helm: &mut FakeHelm,
    id: u64,
    preferences: &Preferences,
    cx: &mut VisualTestContext,
) {
    let state = CollectionState {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::to_value(preferences).unwrap(),
        }],
    };
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    helm.reply(id, 204, None);
    settle(cx);
}

/// Whether the bolt of `name` is drawn lit (reported pressed); `None` when it is not drawn.
fn bolt_lit(cx: &mut VisualTestContext, name: &str) -> Option<bool> {
    let id = format!("template-bolt-{name}");
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id.as_str()))
        .map(|node| node.toggled == Some(gpui::Toggled::True))
}

/// SPEC.md: "at most one saved template is marked with ⚡ ... Clicking another bolt moves it; clicking the lit bolt
/// clears it." (That a click means what the drawn bolt showed, even when the store changed before the redraw, is pinned
/// by `shortcuts_tests::a_bolt_click_writes_what_the_drawn_bolt_meant`: here it would depend on whether a push lands
/// before the click is dispatched.) A click writes only the mark's field, the template's name to mark it and `null` to clear it, and the
/// list shows the mark once the helm has stored it, never before.
#[gpui::test]
fn the_bolt_marks_and_clears_through_the_preferences(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    click(cx, "templates-button");
    click(cx, "template-bolt-my-codex");
    let (id, body) = one_write(&mut helm);
    assert_eq!(
        body,
        serde_json::json!({"quick_launch_template": "my-codex"})
    );
    let marked = Preferences {
        quick_launch_template: Some("my-codex".into()),
        ..Preferences::default()
    };
    assert_eq!(
        bolt_lit(cx, "my-codex"),
        Some(false),
        "nothing shows before the helm stores it"
    );
    store_and_reply(&mut helm, id, &marked, cx);
    assert_eq!(bolt_lit(cx, "my-codex"), Some(true));

    click(cx, "template-bolt-my-codex");
    let (_, body) = one_write(&mut helm);
    assert_eq!(body, serde_json::json!({"quick_launch_template": null}));
}

/// A template that cannot launch on its own cannot be marked: its bolt is unavailable. A marked template that no
/// longer qualifies keeps an available bolt, so the mark can still be cleared (SPEC.md: "A marked template that no
/// longer qualifies can still be unmarked").
#[gpui::test]
fn an_ineligible_bolt_is_unavailable_unless_it_is_lit(cx: &mut TestAppContext) {
    let mut list = templates();
    list.push(template(
        "partial",
        TemplateFields {
            model: Some(Some("gpt-6-luna".into())),
            ..TemplateFields::default()
        },
    ));
    let (mut helm, cx) = open_header(cx, &list, None);
    click(cx, "templates-button");
    click(cx, "template-bolt-partial");
    assert!(
        helm.commands().is_empty(),
        "an unavailable bolt writes nothing"
    );

    // An agent marked the partial template (the helm checks eligibility on agent writes, but an edit can make a marked
    // template partial afterwards). Its lit bolt still clears the mark.
    let state = preferences_state(Some("partial"));
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
    click(cx, "template-bolt-partial");
    let (_, body) = one_write(&mut helm);
    assert_eq!(body, serde_json::json!({"quick_launch_template": null}));
}

/// SPEC.md: "Each row's key button opens a menu with No shortcut, then 1–9 and 0; every option has a description,
/// occupied keys name their template there, and choosing one moves it." The button itself is named for its template
/// ("Number shortcut my-codex"), as the old one was, so rows read apart. Choosing a key closes the menu and writes the
/// whole map from what the store holds, so the key leaves the template that held it.
#[gpui::test]
fn the_key_menu_offers_every_key_and_moves_the_chosen_one(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    let held = Preferences {
        template_shortcuts: [("2".to_owned(), "webbuilder".to_owned())]
            .into_iter()
            .collect(),
        ..Preferences::default()
    };
    let state = CollectionState {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::to_value(&held).unwrap(),
        }],
    };
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
    click(cx, "templates-button");
    let key_label = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("template-key-my-codex"))
        .and_then(|node| node.label.map(|label| label.to_string()));
    assert_eq!(
        key_label.as_deref(),
        Some("Number shortcut my-codex"),
        "the key button names its template, so rows read apart"
    );
    click(cx, "template-key-my-codex");
    let items: Vec<String> = frame(cx)
        .into_iter()
        .filter(|node| node.role == Some(gpui::Role::MenuItem))
        .filter_map(|node| node.label.map(|label| label.to_string()))
        .collect();
    let mut expected = vec!["No shortcut".to_owned()];
    expected.extend(
        super::shortcuts::KEYS
            .iter()
            .map(|key| super::number_key_name(key)),
    );
    assert_eq!(items, expected);
    let text = painted(cx);
    assert!(text.contains("webbuilder (moves here)"), "{text}");
    assert!(text.contains("No template uses this key"), "{text}");

    click(cx, "templates-key-2-item");
    assert!(!any_open_menu(cx), "choosing closes the menu");
    let (_, body) = one_write(&mut helm);
    assert_eq!(
        body,
        serde_json::json!({"template_shortcuts": {"2": "my-codex"}})
    );
}

/// A refused write is reported under the panes in the helm's words, and the list goes on showing what the helm stores
/// (SPEC.md, "One GUI at a time": a failed save is reported, then discarded).
#[gpui::test]
fn a_refused_write_is_reported_and_nothing_changes(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    click(cx, "templates-button");
    click(cx, "template-bolt-my-codex");
    let (id, _) = one_write(&mut helm);
    helm.reply(
        id,
        503,
        Some(serde_json::json!("fixture preference save refused")),
    );
    settle(cx);
    let text = painted(cx);
    assert!(
        text.contains("The quick-launch mark was not saved: fixture preference save refused"),
        "{text}"
    );
    assert_eq!(
        bolt_lit(cx, "my-codex"),
        Some(false),
        "the list shows what the helm stores"
    );
}

/// Whether a popover menu is open over the dialog.
fn any_open_menu(cx: &mut VisualTestContext) -> bool {
    frame(cx)
        .into_iter()
        .any(|node| node.element_id.as_deref() == Some("templates-key-menu"))
}

/// A ⚡ write keeps focus where the user is: the bolt waits (busy) while the write is in flight rather than giving up
/// focus, which would fall to the dialog itself and lose the user's place in the list; and neither Space nor Escape
/// closes the dialog meanwhile, so a refusal can still be reported. The old dialog put focus back after its write for
/// the same reason.
#[gpui::test]
fn a_write_in_flight_leaves_focus_and_the_dialog_alone(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    // A click focuses the button it presses, as it does on Linux; pinned so the premise holds on every platform.
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    click(cx, "templates-button");
    click(cx, "template-bolt-my-codex");
    let (id, _) = one_write(&mut helm);
    let bolt = Some("template-bolt-my-codex");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        bolt,
        "the premise: the click focused the bolt"
    );
    keys(cx, "space");
    keys(cx, "escape");
    assert!(
        any_open(cx),
        "neither Space nor Escape closes the dialog while the write is in flight"
    );
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        bolt,
        "the busy bolt kept focus"
    );
    assert!(helm.commands().is_empty(), "and nothing else was sent");
    let marked = Preferences {
        quick_launch_template: Some("my-codex".into()),
        ..Preferences::default()
    };
    store_and_reply(&mut helm, id, &marked, cx);
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        bolt,
        "focus stays after the reply"
    );
}

/// A template holding a key is named in the menu as host text: a name carrying invisible formatting is drawn escaped.
#[gpui::test]
fn the_key_menu_names_holders_as_host_text(cx: &mut TestAppContext) {
    let mut list = templates();
    list.push(template("sneaky\u{200B}x", TemplateFields::default()));
    let (mut helm, cx) = open_header(cx, &list, None);
    let held = Preferences {
        template_shortcuts: [("3".to_owned(), "sneaky\u{200B}x".to_owned())]
            .into_iter()
            .collect(),
        ..Preferences::default()
    };
    let state = CollectionState {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::to_value(&held).unwrap(),
        }],
    };
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
    click(cx, "templates-button");
    click(cx, "template-key-my-codex");
    assert!(any_open_menu(cx), "the premise: the menu is open");
    let text = painted(cx);
    assert!(text.contains("sneaky<U+200B>x (moves here)"), "{text}");
}

/// The key menu belongs to its row: when the template goes (deleted elsewhere, renamed by an agent), its row and the
/// button the menu is anchored to go, and the menu closes with them rather than staying open for a template that no
/// longer exists. The remaining case of the old `popover-template-movement.spec.ts`.
#[gpui::test]
fn the_key_menu_closes_when_its_template_is_gone(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    click(cx, "templates-button");
    click(cx, "template-key-my-codex");
    assert!(any_open_menu(cx), "the premise: the menu is open");
    let remaining: Vec<LaunchTemplate> = templates()
        .into_iter()
        .filter(|template| template.name != "my-codex")
        .collect();
    let state = templates_state(&remaining, Availability::Available);
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: vec!["my-codex".into()],
    });
    settle(cx);
    assert!(!any_open_menu(cx), "the menu closed with its row");
    assert!(any_open(cx), "the dialog stays");
}

/// The key menu closes back onto its key button (the old menu returned focus there), also on macOS, where the click
/// that opened it left the button unfocused and returning focus to what held it before would put the user back
/// elsewhere. Specified: with clicks not focusing controls, the key button opens the menu without holding focus, and
/// Escape closes only the menu and puts focus on the button.
#[gpui::test]
fn escape_from_the_key_menu_returns_focus_to_its_button(cx: &mut TestAppContext) {
    let (_helm, cx) = open_header(cx, &templates(), None);
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    click(cx, "templates-button");
    assert_ne!(
        test_support::focused_id(cx).as_deref(),
        Some("template-key-my-codex"),
        "the premise: the key button does not hold focus before the click"
    );
    click(cx, "template-key-my-codex");
    assert!(any_open_menu(cx), "the premise: the menu is open");
    keys(cx, "escape");
    assert!(!any_open_menu(cx), "Escape closed the menu");
    assert!(any_open(cx), "and only the menu");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("template-key-my-codex")
    );
}

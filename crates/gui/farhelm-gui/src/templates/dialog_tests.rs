//! Tests of the Templates dialog over the helm client's fake: what opens it, what its list shows and how the list
//! follows the store, the hint lines, and closing.
//!
//! The dialog is opened the way the list header's Templates button will open it: a button in a root view of the test's
//! own, whose click takes a mark and calls [`super::open`] (or [`super::open_with_hint`], as a shortcut will). Every
//! step ends in one of the shared harness's step helpers, so the focus and layer invariants are checked after each.
//! The one test whose outcome is text layout, the number keys' digit widths, uses the real text system instead, since
//! the test context's gives every character the same width.

use std::sync::Arc;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::LaunchHarness;
use farhelm_proto::launcher::{LaunchTemplate, LauncherKind, TemplateFields};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, KeyedEntry,
    PREFERENCES_KEY, Preferences,
};
use gpui::{
    AppContext as _, Context, DriverNode, FocusHandle, HeadlessAppContext, IntoElement,
    ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext, Window, div, px,
    size,
};
use gpui_component::Root;

use super::Hint;
use super::dispatch_tests::push as push_state;
use crate::components::{Button, HoverHelp, Role};
use crate::focus::{self, Mark};
use crate::host_text::tests::painted_words;
use crate::host_text::{HostSentence, HostText};
use crate::session_list::host_rail::tests::texts_within;
use crate::test_support::{self, any_open, click, frame, keys, meta, settle, started};
use crate::{assets, bootstrap, root};

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
    open_header_with(cx, templates, marked, Vec::new())
}

/// [`open_header`], with `more` collections in the helm's first snapshot (hosts, recipes): the store ignores a push to
/// a collection that snapshot did not install.
pub(super) fn open_header_with<'a>(
    cx: &'a mut TestAppContext,
    templates: &[LaunchTemplate],
    marked: Option<&str>,
    more: Vec<CollectionState>,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut helm, _connection) = started(cx);
    let mut snapshot = vec![
        templates_state(templates, Availability::Available),
        preferences_state(marked),
    ];
    snapshot.extend(more);
    helm.connect(snapshot);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, Header::new);
    // Tall enough that the editor's whole pane, its status line under the panes included, fits the dialog's height
    // cap (the window less 5% above and below), so tests read every line without scrolling.
    window.simulate_resize(size(px(1600.), px(1200.)));
    settle(window);
    (helm, window)
}

/// Every line of text the window painted, joined with single spaces.
pub(super) fn painted(cx: &mut VisualTestContext) -> String {
    let lines: Vec<String> = frame(cx)
        .into_iter()
        .flat_map(|node| node.texts.into_iter().map(|line| painted_words(&line.text)))
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

/// The window can be as narrow as 720 pixels (SPEC.md), and the old dialog clamped itself to the window less its
/// backdrop's 20-pixel margins (`min(960px, 100%)`). Without that the 960-pixel dialog, centered, ran past both sides
/// of a narrow window, its Close among what was cut off. Specified: in a 720-by-560 window the opened dialog is 20
/// pixels clear of each side edge.
#[gpui::test]
fn the_dialog_fits_the_narrowest_window(cx: &mut TestAppContext) {
    let (_helm, cx) = open_header(cx, &templates(), None);
    cx.simulate_resize(size(px(720.), px(560.)));
    settle(cx);
    assert_eq!(
        cx.update(|window, _| window.viewport_size().width),
        px(720.),
        "the premise: the window is the narrowest SPEC.md allows"
    );
    click(cx, "templates-button");
    assert!(any_open(cx), "the premise: the dialog opened");
    let card = test_support::bounds(cx, "templates");
    assert_eq!(card.left(), px(20.), "{card:?}");
    assert_eq!(card.right(), px(700.), "{card:?}");
}

/// Escape closes the dialog, as its header Close does, when there is nothing unsaved to ask about.
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
/// leave question holds both.
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

/// A key write is the whole map, built from the bindings the store holds, so one sent while the helm cannot read its
/// preferences would rest on a map that may be stale, and a mark write on a bolt drawn from it; the old list refused
/// either edit when its fresh read of the preferences failed. Specified: with key 2 on `webbuilder` and then the
/// preferences unavailable ("database is locked"), the key menu still shows key 2 on `webbuilder` (the store keeps the
/// last map, so a write built from it would look right); choosing key 1 for `my-codex` sends nothing and says "The
/// number key was not saved; shortcut bindings could not be read: database is locked", and a click on `my-codex`'s
/// bolt sends nothing and says the same of the quick-launch mark; once the preferences are readable again, the bolt
/// sends its write and the refusal is gone.
#[gpui::test]
fn unreadable_bindings_refuse_a_key_or_mark_write(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    let held = Preferences {
        template_shortcuts: [("2".to_owned(), "webbuilder".to_owned())]
            .into_iter()
            .collect(),
        ..Preferences::default()
    };
    let held_state = CollectionState {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::to_value(&held).unwrap(),
        }],
    };
    push_state(&mut helm, cx, held_state.clone());
    push_state(
        &mut helm,
        cx,
        super::dispatch_tests::unreadable("database is locked"),
    );
    click(cx, "templates-button");
    click(cx, "template-key-my-codex");
    let text = painted(cx);
    assert!(
        text.contains("webbuilder (moves here)"),
        "the premise: the store kept the last map: {text}"
    );
    click(cx, "templates-key-1-item");
    assert!(helm.commands().is_empty(), "no key write was sent");
    let text = painted(cx);
    assert!(
        text.contains(
            "The number key was not saved; shortcut bindings could not be read: database is locked"
        ),
        "{text}"
    );

    click(cx, "template-bolt-my-codex");
    assert!(helm.commands().is_empty(), "no mark write was sent");
    let text = painted(cx);
    assert!(
        text.contains(
            "The quick-launch mark was not saved; shortcut bindings could not be read: database is locked"
        ),
        "{text}"
    );

    push_state(&mut helm, cx, held_state);
    click(cx, "template-bolt-my-codex");
    let (_, body) = one_write(&mut helm);
    assert_eq!(
        body,
        serde_json::json!({"quick_launch_template": "my-codex"})
    );
    assert!(!painted(cx).contains("not saved"), "the refusal is gone");
}

/// Before the preferences arrive the store holds no number keys at all, and a key write built from that empty map
/// would replace every key the helm holds with the one just chosen. Specified: over a helm that pushed no preferences,
/// choosing key 1 for `my-codex` sends nothing and says "The number key was not saved; shortcut bindings could not be
/// read: the shortcut bindings have not arrived yet".
#[gpui::test]
fn bindings_that_never_arrived_refuse_a_key_write(cx: &mut TestAppContext) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![templates_state(&templates(), Availability::Available)]);
    cx.run_until_parked();
    let (_, cx) = test_support::open_view(cx, Header::new);
    cx.simulate_resize(size(px(1600.), px(1200.)));
    settle(cx);
    click(cx, "templates-button");
    click(cx, "template-key-my-codex");
    click(cx, "templates-key-1-item");
    assert!(helm.commands().is_empty(), "no key write was sent");
    let text = painted(cx);
    assert!(
        text.contains(
            "The number key was not saved; shortcut bindings could not be read: the shortcut bindings have not \
             arrived yet"
        ),
        "{text}"
    );
}

/// The helm stores and pushes number keys `keys` (each a digit and the template name it holds), with no ⚡ mark.
fn push_keys(helm: &mut FakeHelm, keys: &[(&str, &str)], cx: &mut VisualTestContext) {
    let preferences = Preferences {
        template_shortcuts: keys
            .iter()
            .map(|(key, name)| ((*key).to_owned(), (*name).to_owned()))
            .collect(),
        ..Preferences::default()
    };
    push_state(
        helm,
        cx,
        crate::session_list::tests::preferences_state(preferences),
    );
}

/// Another writer (an agent, the command line, another window) can change the number keys while Templates is open. The
/// old "number keys move, follow rename, and stay freed after undo" case checked that the next key write keeps such a
/// change; a write built from what the dialog last knew would silently undo the other writer's key. The old key buttons
/// showed the keys as the dialog's own reads and writes had left them, so another writer's change stayed undrawn until
/// the dialog was reopened, where these draw what the store holds (the parity file's deviation "The key buttons follow
/// pushed bindings"). Specified: with Templates open and no key bound, a push binding key 1 to `my-codex` redraws that
/// template's key button with key 1's chord; choosing key 2 for `webbuilder` then writes both keys.
#[gpui::test]
fn a_key_pushed_while_open_is_drawn_and_kept_by_the_next_key_write(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    click(cx, "templates-button");
    let draws = |cx: &mut VisualTestContext, chord: String| {
        texts_within(&frame(cx), "template-key-my-codex")
            .iter()
            .any(|line| line.text.as_ref() == chord.as_str())
    };
    assert!(
        draws(cx, super::shortcuts::key_button_text(None)),
        "the premise: my-codex's key button shows no key"
    );
    push_keys(&mut helm, &[("1", "my-codex")], cx);
    assert!(
        draws(cx, super::number_key_name("1")),
        "the pushed key is drawn"
    );
    click(cx, "template-key-webbuilder");
    click(cx, "templates-key-2-item");
    let (_, body) = one_write(&mut helm);
    assert_eq!(
        body,
        serde_json::json!({"template_shortcuts": {"1": "my-codex", "2": "webbuilder"}})
    );
}

/// The key menu says which template holds each key, and a template's own key must read as its own ("This template"),
/// not as a key to move to it, or the user would take it from themselves (the old "number keys move, follow rename …"
/// case read it). Specified: with key 2 on `my-codex`, `my-codex`'s menu says "This template" for key 2 and "No
/// template uses this key" for the others, and offers no "(moves here)".
#[gpui::test]
fn the_key_menu_names_the_templates_own_key_as_this_template(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    push_keys(&mut helm, &[("2", "my-codex")], cx);
    click(cx, "templates-button");
    click(cx, "template-key-my-codex");
    assert!(any_open_menu(cx), "the premise: the menu is open");
    let text = painted(cx);
    assert!(text.contains("This template"), "{text}");
    assert!(text.contains("No template uses this key"), "{text}");
    assert!(!text.contains("(moves here)"), "{text}");
}

/// A refused number-key write is said as the write it answers, in the helm's words, and the list goes on showing the
/// keys the helm stores (SPEC.md, "One GUI at a time": a failed save is reported, then discarded). The ⚡ write's twin
/// is `a_refused_write_is_reported_and_nothing_changes`; the old "a failed number-key save is reported and never saved
/// later" ran it for the keys. Specified: choosing key 1 for `my-codex`, refused with 503, shows "The number key was
/// not saved: fixture key save refused", and `my-codex`'s key menu, opened again, shows no key as its own.
#[gpui::test]
fn a_refused_key_write_is_reported_and_the_stored_keys_stay(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    click(cx, "templates-button");
    click(cx, "template-key-my-codex");
    click(cx, "templates-key-1-item");
    let (id, body) = one_write(&mut helm);
    assert_eq!(
        body,
        serde_json::json!({"template_shortcuts": {"1": "my-codex"}}),
        "the premise: the key write went out"
    );
    helm.reply(id, 503, Some(serde_json::json!("fixture key save refused")));
    settle(cx);
    let text = painted(cx);
    assert!(
        text.contains("The number key was not saved: fixture key save refused"),
        "{text}"
    );
    click(cx, "template-key-my-codex");
    assert!(any_open_menu(cx), "the premise: the menu opened again");
    assert!(
        !painted(cx).contains("This template"),
        "no key is shown as my-codex's"
    );
}

/// A second click on the key button whose menu is open only closes it, as the old toggle did (its pointerdown timing is
/// superseded). Reopening it at once would leave no way to close the menu by the button that opened it. Specified: a
/// click on `template-key-my-codex` opens its menu, and a second click closes it with no menu open.
#[gpui::test]
fn a_second_click_on_the_key_button_closes_its_menu(cx: &mut TestAppContext) {
    let (_helm, cx) = open_header(cx, &templates(), None);
    click(cx, "templates-button");
    click(cx, "template-key-my-codex");
    assert!(any_open_menu(cx), "the premise: the menu is open");
    click(cx, "template-key-my-codex");
    assert!(!any_open_menu(cx), "the second click closed it");
    assert!(any_open(cx), "the dialog stays");
}

/// A summary longer than its row must be cut at the row's edge (a recorded deviation: no ellipsis), or the row's opener
/// would grow with its text and run under the editor beside the list (the old "long summaries stay reachable …"). The
/// row itself always takes the list's width, so it is the opener, the element that would overflow, whose bounds are
/// checked. Specified: a command template whose command is 400 characters long draws its opener, bolt and key button
/// inside the `templates-list` column.
#[gpui::test]
fn a_long_summary_stays_inside_its_row(cx: &mut TestAppContext) {
    let mut list = templates();
    list[1].fields.command = Some("x".repeat(400));
    let (_helm, cx) = open_header(cx, &list, None);
    click(cx, "templates-button");
    let nodes = frame(cx);
    let bounds_of = |id: &str| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .map(|node| node.bounds)
            .unwrap_or_else(|| panic!("{id} is drawn"))
    };
    let column = bounds_of("templates-list");
    for control in [
        "template-open-webbuilder",
        "template-bolt-webbuilder",
        "template-key-webbuilder",
    ] {
        let bounds = bounds_of(control);
        assert!(bounds.right() <= column.right(), "{control}: {bounds:?}");
    }
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

/// The header Close is held while a ⚡ write is out, as Escape is, so that the write's refusal is still reported in the
/// dialog; the old "quick launch honors a held local mark against the shared read" pressed that Close, which
/// [`a_write_in_flight_leaves_focus_and_the_dialog_alone`] does not. Specified: a click on the header Close while the
/// write is in flight leaves the dialog open, and once the helm has stored the mark and replied, the same click closes
/// it.
#[gpui::test]
fn the_header_close_waits_for_a_write_in_flight(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    click(cx, "templates-button");
    click(cx, "template-bolt-my-codex");
    let (id, _) = one_write(&mut helm);
    click(cx, "templates-close");
    assert!(
        cx.debug_bounds("templates").is_some(),
        "Close held the dialog while the write is out"
    );
    let marked = Preferences {
        quick_launch_template: Some("my-codex".into()),
        ..Preferences::default()
    };
    store_and_reply(&mut helm, id, &marked, cx);
    click(cx, "templates-close");
    assert!(
        cx.debug_bounds("templates").is_none(),
        "and closes it once the write is answered"
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
/// longer exists. This is the case the foundations parity row leaves to port for the old
/// `popover-template-movement.spec.ts`, whose own cases (a menu closing or staying when another row's change moved its
/// button) SPEC.md's "Things changing underneath" supersedes; no old case removed the menu's own template. Specified:
/// with `my-codex`'s key menu open, a push removing `my-codex` closes the menu and leaves the dialog open.
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

/// The four templates [`the_key_menu_follows_its_button_through_what_the_helm_pushes`] lists, in the helm's order (by
/// name). A renamed one becomes `z`, which sorts after all of them.
const MOVING: [&str; 4] = ["a", "b", "c", "d"];

/// What one case of [`the_key_menu_follows_its_button_through_what_the_helm_pushes`] pushes while a key menu is open.
#[derive(Clone, Copy, Debug)]
enum Change {
    /// The ⚡ mark moves to this template, whose row gains the chord line, and off the row that held it.
    MarkTo(&'static str),
    /// The first template is renamed the second elsewhere (by an agent, say): its row goes and the new name's appears.
    Rename(&'static str, &'static str),
}

/// The helm pushes the templates collection as `names`, in that order, each setting nothing, with `removed` taken out.
fn push_names(helm: &mut FakeHelm, names: &[&str], removed: &[&str], cx: &mut VisualTestContext) {
    let list: Vec<LaunchTemplate> = names
        .iter()
        .map(|name| template(name, TemplateFields::default()))
        .collect();
    let state = templates_state(&list, Availability::Available);
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: removed.iter().map(|name| (*name).to_owned()).collect(),
    });
    settle(cx);
}

/// SPEC.md, "Things changing underneath": "Focus follows the thing, not the position", and "Opening and closing":
/// "Nothing closes a dialog, menu, popup list or confirmation that it did not open". The old app closed a template's
/// open number-key menu whenever another change moved its button (`popover-template-movement.spec.ts` at reference
/// commit `24bc0580`, whose six cases this inverts). Pushes move the button in two ways: the ⚡ mark adds a chord line
/// to the marked row, growing it (a button is centered in its row, so its own row growing moves it too), and a rename
/// re-sorts the list by name. Specified: over templates `a` to `d`, in a window tall enough that every key menu opens
/// below its button, with a key menu opened by a click, each row's push leaves the menu open and moves it by as much
/// as its button, which moves the way the row's last column says. The window's height is part of the premise: a menu
/// that fits on neither side of its button is pinned to the window's bottom, and one with no room below opens above
/// it (`layers::popover_origin`), so in a shorter window a menu can change sides or stay pinned while its button moves,
/// which is the layer manager's placement rule, not a menu failing to follow.
#[gpui::test]
fn the_key_menu_follows_its_button_through_what_the_helm_pushes(cx: &mut TestAppContext) {
    use Change::{MarkTo, Rename};
    use std::cmp::Ordering::{Equal, Greater, Less};

    let list: Vec<LaunchTemplate> = MOVING
        .iter()
        .map(|name| template(name, TemplateFields::default()))
        .collect();
    let (mut helm, cx) = open_header(cx, &list, None);
    // Tall enough that every key menu opens below its button, which each case checks after opening. The menu, its
    // header over eleven keys, is roughly 600 pixels tall by its items' sizes; in `open_header`'s 1200-pixel window the
    // top row's menu fits on neither side of its button and the others' only above.
    cx.simulate_resize(size(px(1600.), px(2000.)));
    settle(cx);
    click(cx, "templates-button");
    for (marked, key, change, way) in [
        (Some("c"), "template-key-b", MarkTo("a"), Greater),
        (Some("c"), "template-key-a", MarkTo("a"), Greater),
        (Some("c"), "template-key-a", MarkTo("b"), Equal),
        (Some("c"), "template-key-d", MarkTo("a"), Equal),
        (None, "template-key-b", Rename("a", "z"), Less),
        (None, "template-key-a", Rename("c", "z"), Equal),
    ] {
        let case = format!("{key} through {change:?}");
        push_state(&mut helm, cx, preferences_state(marked));
        assert_eq!(row_names(cx), MOVING, "{case}: the premise: the list");
        if let Some(marked) = marked {
            assert_eq!(bolt_lit(cx, marked), Some(true), "{case}: the premise");
        }
        click(cx, key);
        // `any_open_menu` draws a fresh frame, which `bounds` then reads.
        assert!(any_open_menu(cx), "{case}: the premise: the menu is open");
        let button = test_support::bounds(cx, key);
        let menu = test_support::bounds(cx, "templates-key-menu");
        assert!(
            (menu.top() - button.bottom() - px(4.)).abs() < px(1.),
            "{case}: the premise: the menu opened 4 pixels below its button: {menu:?} under {button:?}"
        );
        let (button, menu) = (button.top(), menu.top());

        match change {
            MarkTo(to) => {
                push_state(&mut helm, cx, preferences_state(Some(to)));
                assert_eq!(bolt_lit(cx, to), Some(true), "{case}: the premise");
            }
            Rename(from, to) => {
                let mut names: Vec<&str> =
                    MOVING.into_iter().filter(|name| *name != from).collect();
                names.push(to);
                push_names(&mut helm, &names, &[from], cx);
                assert_eq!(row_names(cx), names, "{case}: the premise: re-sorted");
            }
        }
        assert!(any_open_menu(cx), "{case}: the menu is still open");
        let moved = test_support::bounds(cx, key).top() - button;
        let followed = test_support::bounds(cx, "templates-key-menu").top() - menu;
        assert!(
            (followed - moved).abs() < px(1.),
            "{case}: the menu moved {followed:?} with its button's {moved:?}"
        );
        // Rounded to a whole pixel, so layout rounding does not read as a move.
        assert_eq!(
            f32::from(moved).round().partial_cmp(&0.),
            Some(way),
            "{case}: the button moved {moved:?}"
        );

        // Back to the four templates with no menu open, for the next case.
        keys(cx, "escape");
        assert!(!any_open_menu(cx), "{case}: Escape closed the menu");
        if let Rename(_, to) = change {
            push_names(&mut helm, &MOVING, &[to], cx);
        }
    }
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

// ---- The number keys' digits, with the real text system -------------------------------------------------------------

/// The Templates dialog over [`Header`], over a helm holding `templates` and `preferences`, laid out with the real text
/// system and the bundled fonts in a headless app (no renderer, which layout does not need) in a 1600-by-1200 window:
/// the app, for measuring text in its faces, and the driver record of the dialog's second frame. Where closing would
/// return focus does not matter here, so the dialog is given a fresh handle for it.
fn laid_out_dialog(
    templates: &[LaunchTemplate],
    preferences: Preferences,
) -> (HeadlessAppContext, Vec<DriverNode>) {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx = HeadlessAppContext::with_asset_source(text_system, Arc::new(assets::Assets));
    cx.update(bootstrap::install).unwrap();
    let (mut fake, handle, events) = FakeHelm::new();
    let _connection = cx.update(|cx| crate::session::start(handle, events, None, cx));
    fake.connect(vec![
        templates_state(templates, Availability::Available),
        crate::session_list::tests::preferences_state(preferences),
    ]);
    cx.run_until_parked();
    let window = cx
        .open_window(size(px(1600.), px(1200.)), |window, cx| {
            let header = cx.new(|cx| Header::new(window, cx));
            cx.new(|cx| Root::new(header, window, cx))
        })
        .unwrap();
    let opened = cx
        .update_window(window.into(), |_, window, cx| {
            let opener = cx.focus_handle();
            let mark = Mark::take(cx);
            super::open(&opener, &mark, window, cx)
        })
        .unwrap();
    assert!(opened.is_some(), "the premise: the dialog opened");
    cx.run_until_parked();
    let nodes = cx
        .update_window(window.into(), |_, window, cx| {
            window.set_driver_enabled(true);
            window.draw(cx).clear(cx);
            window.draw(cx).clear(cx);
            window.driver_nodes().to_vec()
        })
        .unwrap();
    (cx, nodes)
}

/// The old stylesheet drew a key button's chord in the monospace face with tabular figures (`.templates-number-key`),
/// so the rows' chords line up and a chord's width does not depend on its digit; the interface face's figures are
/// proportional. The test context's text system gives every character the same width, so only the real one can tell.
/// Specified, with the real text system: two templates holding the keys 1 and 8, the old case's pair, paint their key
/// buttons' chords each as many advances of the monospace face, at the button's 10 pixels, as the chord has
/// characters; and the interface face's 1 and 8 differ in advance at that size by more than a pixel (the premise: in
/// that face the two chords would differ in width). The old case also compared the key buttons themselves, which take
/// their width from their chord, so the two buttons are equally wide too.
#[test]
fn the_number_keys_digits_are_drawn_at_equal_widths() {
    let digits = ["1", "8"];
    let list: Vec<LaunchTemplate> = digits
        .iter()
        .map(|digit| template(&format!("key-{digit}"), TemplateFields::default()))
        .collect();
    let preferences = Preferences {
        template_shortcuts: digits
            .iter()
            .map(|digit| ((*digit).to_owned(), format!("key-{digit}")))
            .collect(),
        ..Preferences::default()
    };
    let (mut cx, nodes) = laid_out_dialog(&list, preferences);
    // `KEY_TEXT` in `dialog.rs`: the size the key button draws its chord at.
    let text_size = px(10.);
    let (mono, one, eight) = cx.update(|cx| {
        let text_system = cx.text_system();
        let advance = |family: &'static str, character: char| {
            let face = text_system.resolve_font(&gpui::font(family));
            f32::from(
                text_system
                    .advance(face, text_size, character)
                    .unwrap()
                    .width,
            )
        };
        (
            advance(assets::TERMINAL_FONT, '0'),
            advance(assets::UI_FONT, '1'),
            advance(assets::UI_FONT, '8'),
        )
    });
    assert!(
        eight - one > 1.,
        "premise: the interface face's figures are proportional ({one} vs {eight})"
    );
    let mut buttons = Vec::new();
    for digit in digits {
        let chord = super::number_key_name(digit);
        let id = format!("template-key-key-{digit}");
        let button = nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id.as_str()))
            .unwrap_or_else(|| panic!("{id} is drawn"));
        buttons.push(f32::from(button.bounds.size.width));
        let painted = texts_within(&nodes, &id);
        let line = painted
            .iter()
            .find(|line| line.text.as_ref() == chord.as_str())
            .unwrap_or_else(|| panic!("{id} paints {chord:?}; it painted {painted:?}"));
        let drawn = f32::from(line.bounds.size.width);
        let expected = chord.chars().count() as f32 * mono;
        assert!(
            (drawn - expected).abs() < 0.5,
            "the {digit} chord is {drawn} wide, not {expected} ({mono} per monospace advance)"
        );
    }
    assert!(
        (buttons[0] - buttons[1]).abs() < 0.5,
        "the key buttons are equally wide: {buttons:?}"
    );
}

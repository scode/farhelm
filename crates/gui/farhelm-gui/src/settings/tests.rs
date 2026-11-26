//! Tests of the Settings dialog over the helm client's fake: how the gear opens it and where focus starts and returns,
//! which sections it lists and what switching between them keeps, what each switch sends and shows while its save is
//! in flight, after a refusal and after the dialog closed.
//!
//! The dialog is opened the way the host rail's gear opens it: a button in a root view of the test's own, whose click
//! takes a mark and calls [`super::open`] with its own focus handle. Every step ends in one of the shared harness's step
//! helpers, so the focus and layer invariants are checked after each.
//!
//! The helpers marked `pub(super)` are shared with the Credentials and Recipes sections' tests
//! (`super::credential_tests`, `super::recipe_tests`), which open the dialog the same way.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::LaunchHarness;
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionState, HostView, KeyedEntry,
    PREFERENCES_KEY, Preferences,
};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement as _, Render,
    Styled as _, TestAppContext, Toggled, VisualTestContext, Window, WindowAppearance, div, px,
    size,
};
use serde_json::{Value, json};

use super::choices::Choice;
use super::{Section, TrustAgent};
use crate::components::{Button, HoverHelp, Role};
use crate::desktop::updates::fake::{Feed, Seen, fake_host};
use crate::focus::{self, Mark};
use crate::main_view::MainView;
use crate::operations::{OpKey, Work};
use crate::test_support::{
    self, any_open, bounds, click, focused, focused_id, frame, keys, label, meta, one_command,
    settle, started,
};
use crate::{layers, root, session, theme};
use farhelm_input_mapping::Platform;

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The root view the tests open Settings from: the gear the sidebar draws on its host rail, as a plain button with
/// a focus handle the test holds, after another control ("elsewhere", standing in for whatever held focus before the
/// gear was clicked, the terminal usually).
struct Rail {
    root: root::Attachment,
    gear: FocusHandle,
}

impl Render for Rail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let gear = self.gear.clone();
        let content = div()
            .size_full()
            .p(px(20.))
            .child(Button::new(
                "elsewhere",
                "Elsewhere",
                HoverHelp::tooltip("A control away from the gear"),
                Role::Button,
            ))
            .child(
                Button::new(
                    "gear",
                    "Settings",
                    HoverHelp::tooltip("Settings: choices that apply to every host on this helm"),
                    Role::Button,
                )
                .focus(&self.gear)
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    super::open(&gear, &mark, window, cx);
                }),
            );
        root::wrap(content, &self.root, window, cx)
    }
}

/// The section button that shows `section`, also used to check which control kept focus.
pub(super) fn section_button(section: Section) -> &'static str {
    section.button_id()
}

/// Select a section through the sidebar. The agent pages can be outside the clipped part of the navigation.
pub(super) fn click_section(cx: &mut VisualTestContext, section: Section) {
    let selector = section_button(section);
    let viewport = bounds(cx, "settings-navigation-region");
    let row = bounds(cx, selector);
    if row.bottom() > viewport.bottom() || row.top() < viewport.top() {
        cx.simulate_event(gpui::ScrollEvent {
            position: viewport.center(),
            delta: gpui::ScrollDelta::Pixels(gpui::point(
                px(0.),
                viewport.center().y - row.center().y,
            )),
            ..Default::default()
        });
        settle(cx);
    }
    assert!(shows_whole(cx, selector, "settings-navigation-region"));
    click(cx, selector);
}

/// Keep a fake desktop updater as the app's, as the desktop app does at startup when its updater runs, with automatic
/// updates starting at `automatic`. Its automatic-updates write succeeds unless `refuse` is set when it is called, and
/// every value it is asked to write, saved or not, lands in `writes`, so a test sees each press reach the updater.
/// The feed and command count let the App tests drive progress without starting an actual update.
fn keep_updater(
    cx: &mut VisualTestContext,
    automatic: bool,
    refuse: &Rc<Cell<bool>>,
    writes: &Rc<RefCell<Vec<bool>>>,
) -> (Rc<Feed>, Rc<Seen>) {
    let (feed, seen) = (Rc::new(Feed::default()), Rc::new(Seen::default()));
    seen.automatic.set(automatic);
    let mut host = fake_host(&feed, &seen, false);
    let (refuse, writes) = (refuse.clone(), writes.clone());
    host.set_automatic_updates = Box::new(move |on| {
        writes.borrow_mut().push(on);
        if refuse.get() {
            Err("The choice could not be saved".to_string())
        } else {
            Ok(())
        }
    });
    cx.update(|_, cx| crate::desktop::updates::keep(Some(host), cx));
    settle(cx);
    (feed, seen)
}

/// The preferences collection holding `preferences`.
fn preferences_state(preferences: &Preferences) -> CollectionState {
    CollectionState {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::to_value(preferences).unwrap(),
        }],
    }
}

/// The helm pushes `preferences` as its row, as it does before replying to a save that changed it.
pub(super) fn push(helm: &mut FakeHelm, preferences: &Preferences, cx: &mut VisualTestContext) {
    let state = preferences_state(preferences);
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
}

/// The app over the fake helm holding `preferences`, in an active window as tall as a laptop screen's showing the
/// gear, with the gear's focus handle. Nothing is open yet.
pub(super) fn open_rail<'a>(
    cx: &'a mut TestAppContext,
    preferences: &Preferences,
) -> (FakeHelm, &'a mut VisualTestContext, FocusHandle) {
    open_rail_with(cx, preferences, Vec::new())
}

/// [`open_rail`], with `more` collections in the helm's first snapshot beside the preference row: the store applies
/// pushed changes only to a collection its snapshot installed, so a test that needs hosts (Recipes' place names, say)
/// puts them here.
pub(super) fn open_rail_with<'a>(
    cx: &'a mut TestAppContext,
    preferences: &Preferences,
    more: Vec<CollectionState>,
) -> (FakeHelm, &'a mut VisualTestContext, FocusHandle) {
    let (mut helm, _connection) = started(cx);
    let mut snapshot = vec![preferences_state(preferences)];
    snapshot.extend(more);
    helm.connect(snapshot);
    cx.run_until_parked();
    let gear = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let held = gear.clone();
    let (_, window) = test_support::open_view(cx, move |window, cx| Rail {
        root: root::attach(window, cx),
        gear: held,
    });
    window.simulate_resize(size(px(1600.), px(1000.)));
    settle(window);
    (helm, window, gear)
}

/// Click the gear and check that Settings opened.
pub(super) fn open_settings(cx: &mut VisualTestContext) {
    click(cx, "gear");
    assert!(any_open(cx), "the premise: the gear opened Settings");
}

/// The visible part of the Settings page, between its pinned heading and footer.
const DIALOG_BODY: &str = "settings-body";
/// All Settings forms put Save in the footer. The host settings and launcher checkout fields keep their own Save.
const FOOTER_SAVE: &str = "settings-footer-save";

/// Whether the element drawn under `selector` lies wholly inside, top to bottom, the element drawn under `region`: a
/// control in a scrolling region shows whole only when this holds.
fn shows_whole(cx: &mut VisualTestContext, selector: &'static str, region: &'static str) -> bool {
    let (inner, outer) = (bounds(cx, selector), bounds(cx, region));
    inner.top() >= outer.top() && inner.bottom() <= outer.bottom()
}

/// The commands the window sent since the last call.
pub(super) fn sent(helm: &mut FakeHelm) -> Vec<FakeCommand> {
    helm.commands()
}

/// The one command sent since the last call, which must be a preference save: its id and its body.
pub(super) fn one_save(helm: &mut FakeHelm) -> (u64, Value) {
    let sent = sent(helm);
    assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
    assert_eq!(
        (sent[0].method, sent[0].path.as_str()),
        (Method::Put, "/api/preferences")
    );
    (sent[0].id, sent[0].body.clone().unwrap_or_default())
}

/// Whether the switch drawn under `id` shows on; `None` when it is not drawn.
pub(super) fn switch_on(cx: &mut VisualTestContext, id: &str) -> Option<bool> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| node.toggled == Some(Toggled::True))
}

/// Whether anything is drawn under the element id `id`.
pub(super) fn drawn(cx: &mut VisualTestContext, id: &str) -> bool {
    frame(cx)
        .into_iter()
        .any(|node| node.element_id.as_deref() == Some(id))
}

/// The work in flight on the preference row.
pub(super) fn preference_work(cx: &mut VisualTestContext) -> Option<Work> {
    cx.update(|_, cx| {
        session::current_operations(cx)
            .and_then(|operations| operations.read(cx).in_flight(&OpKey::preferences()))
    })
}

/// Every line of text the window painted, joined with single spaces, so a sentence the dialog wrapped over several lines
/// reads as one.
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

/// The shown section's heading.
pub(super) fn heading(cx: &mut VisualTestContext) -> Option<String> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("settings-heading"))
        .and_then(|node| node.label.map(|label| label.to_string()))
}

// ---- Opening, the sections, and focus ------------------------------------------------------------------------------

/// The catalog owns navigation even when it is empty. An external delete before a PUT settles takes the user
/// back to Add while retaining the draft and its receipt. A delete confirmed on the page removes it entirely.
/// The previews and sidebar hover help isolate registered text so it cannot reorder the surrounding app words.
/// The retained draft also exposes the deletion and recovery instructions as a screen-reader status.
/// A busy key refuses either write with the shared reason and leaves the draft available for retry.
/// An unreadable catalog exposes the helm's reason on the first read and preserves the page on a failed refresh.
/// Settings search finds the saved agent by its name, ID and command, and opens the editor without a read.
#[gpui::test]
fn creating_and_deleting_a_custom_agent_follows_the_catalog(cx: &mut TestAppContext) {
    const DISPLAY_NAME: &str = "Example Code סוכן";
    const COMMAND: &str = "~/bin/כלי-42";
    let collection = Collection::CustomAgents;
    let status = |order: &[&str], availability| CollectionChanges {
        collection,
        meta: farhelm_proto::ui_channel::CollectionMeta {
            availability,
            ..meta(order)
        },
        upsert: vec![],
        remove: vec![],
    };
    let unavailable = || Availability::Unavailable {
        error: "The registration database is locked. Try again.".into(),
    };
    let (mut helm, cx, _gear) = open_rail_with(
        cx,
        &Preferences::default(),
        vec![CollectionState {
            collection,
            meta: status(&[], unavailable()).meta,
            entries: vec![],
        }],
    );
    open_settings(cx);
    click_section(cx, Section::AddCustomAgent);
    assert_eq!(heading(cx).as_deref(), Some("Add custom agent"));
    let catalog_error = "custom-agent-catalog-error";
    assert_eq!(
        label(cx, catalog_error).as_deref(),
        Some("Could not load custom agents: The registration database is locked. Try again.")
    );
    helm.push(status(&[], Availability::Available));
    settle(cx);
    assert!(!drawn(cx, catalog_error));
    fill(cx, "custom-agent-name", DISPLAY_NAME);
    fill(cx, "custom-agent-command", COMMAND);
    into_view(cx, "custom-agent-resumption");
    click(cx, "custom-agent-resumption");
    assert_eq!(switch_on(cx, "custom-agent-resumption"), Some(true));
    for id in ["custom-agent-start-preview", "custom-agent-resume-preview"] {
        into_view(cx, id);
        let lines = crate::session_list::tests::texts_within(&frame(cx), id);
        assert!(
            lines
                .iter()
                .any(|line| { crate::host_text::tests::paints_isolated(&line.text, COMMAND) }),
            "{id} isolates the command on its own: {lines:?}"
        );
    }
    click(cx, "custom-agent-save");
    let (create, body) = one_command(&mut helm, Method::Post, "/api/custom-agents");
    assert_eq!(
        body,
        json!({
            "id": "example-code",
            "display_name": DISPLAY_NAME,
            "command": COMMAND,
            "supports_resumption": true,
            "supports_reporting": false,
        })
    );
    helm.push(CollectionChanges {
        collection,
        meta: meta(&["example-code"]),
        upsert: vec![KeyedEntry {
            key: "example-code".into(),
            value: body.clone(),
        }],
        remove: vec![],
    });
    helm.reply(create, 201, Some(body));
    settle(cx);
    assert_eq!(heading(cx).as_deref(), Some(DISPLAY_NAME));

    let nav = "settings-section-custom-example-code";
    assert!(shows_whole(cx, nav, "settings-navigation-region"));
    let at = test_support::center(cx, nav);
    cx.simulate_mouse_move(at, None, gpui::Modifiers::none());
    test_support::advance(cx, crate::layers::HOVER_DELAY);
    let tooltip =
        crate::session_list::tests::texts_within(&frame(cx), "custom-agent-nav-help-example-code");
    assert!(
        tooltip
            .iter()
            .any(|line| { crate::host_text::tests::paints_isolated(&line.text, DISPLAY_NAME) }),
        "the tooltip isolates the registered name: {tooltip:?}"
    );

    fill(cx, "custom-agent-command", "~/bin/example-v2");
    let key = crate::operations::OpKey::custom_agent("example-code");
    let operations = cx.update(|_, cx| crate::session::current_operations(cx).expect("connected"));
    cx.update(|_, cx| {
        operations.update(cx, |store, cx| {
            store
                .hold(&key, crate::operations::Work::Save, cx)
                .expect("hold the saved registration's key");
        })
    });
    click(cx, "custom-agent-save");
    assert_eq!(
        label(cx, "custom-agent-error").as_deref(),
        Some("Not saved: another operation was in progress. Try again.")
    );
    click(cx, "custom-agent-delete");
    click(cx, "custom-agent-confirm-delete");
    assert_eq!(
        label(cx, "custom-agent-error").as_deref(),
        Some("Not deleted: another operation was in progress. Try again.")
    );
    assert!(drawn(cx, "custom-agent-save") && sent(&mut helm).is_empty());
    cx.update(|_, cx| operations.update(cx, |store, cx| store.release(&key, cx)));
    click(cx, "custom-agent-save");
    let (update, edited) = one_command(&mut helm, Method::Put, "/api/custom-agents/example-code");
    assert_eq!(edited["command"], "~/bin/example-v2");
    helm.push(CollectionChanges {
        collection,
        meta: meta(&[]),
        upsert: vec![],
        remove: vec!["example-code".into()],
    });
    settle(cx);
    assert_eq!(heading(cx).as_deref(), Some("Add custom agent"));
    helm.reply(
        update,
        404,
        Some(json!({"error": "Custom agent is no longer registered"})),
    );
    settle(cx);
    click_section(cx, Section::AddCustomAgent);
    click(cx, "settings-section-custom-example-code");
    let nodes = frame(cx);
    let notice = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("custom-agent-deleted-notice"))
        .expect("the retained draft exposes the deletion notice");
    assert_eq!(notice.role, Some(Role::Status));
    assert_eq!(
        notice.label.as_deref(),
        Some("This registration was deleted. Register it again to keep these edits.")
    );
    assert_eq!(
        label(cx, "custom-agent-save").as_deref(),
        Some("Register again")
    );
    click(cx, "custom-agent-save");
    let (register, recovered) = one_command(&mut helm, Method::Post, "/api/custom-agents");
    assert_eq!(recovered, edited, "the refused PUT did not lose the edit");
    helm.push(CollectionChanges {
        collection,
        meta: meta(&["example-code"]),
        upsert: vec![KeyedEntry {
            key: "example-code".into(),
            value: recovered.clone(),
        }],
        remove: vec![],
    });
    helm.reply(register, 201, Some(recovered));
    settle(cx);
    assert_eq!(heading(cx).as_deref(), Some(DISPLAY_NAME));
    helm.push(status(&["example-code"], unavailable()));
    settle(cx);
    assert!(drawn(cx, catalog_error) && drawn(cx, "custom-agent-delete"));
    assert_eq!(heading(cx).as_deref(), Some(DISPLAY_NAME));
    helm.push(status(&["example-code"], Availability::Available));
    settle(cx);
    assert!(!drawn(cx, catalog_error));

    click_section(cx, Section::Sessions);
    assert_eq!(heading(cx).as_deref(), Some("Sessions"));
    click(cx, "settings-search-field");
    for query in ["custom agents", "example-code", "example-v2", DISPLAY_NAME] {
        crate::test_support::select_all_text(cx);
        cx.simulate_input(query);
        settle(cx);
        assert!(drawn(
            cx,
            "settings-result-custom-example-code-custom-agent-command"
        ));
        if query == "custom agents" {
            assert!(drawn(
                cx,
                "settings-result-add-custom-agent-custom-agent-command"
            ));
        }
    }
    keys(cx, "enter");
    assert_eq!(heading(cx).as_deref(), Some(DISPLAY_NAME));
    assert!(drawn(cx, "custom-agent-command") && sent(&mut helm).is_empty());

    // Changes and Reply arrive together. A deliberate Delete abandons this edit instead of retaining it for recovery.
    fill(cx, "custom-agent-command", "~/bin/unsaved");
    assert!(cx.update(|_, cx| super::draft_changed_for_opened(
        Section::CustomAgent("example-code".into()),
        cx
    )));
    click(cx, "custom-agent-delete");
    assert!(sent(&mut helm).is_empty(), "Delete asks first");
    click(cx, "custom-agent-confirm-delete");
    let (delete, _) = one_command(&mut helm, Method::Delete, "/api/custom-agents/example-code");
    helm.push(CollectionChanges {
        collection,
        meta: meta(&[]),
        upsert: vec![],
        remove: vec!["example-code".into()],
    });
    helm.reply(delete, 204, None);
    settle(cx);
    assert_eq!(heading(cx).as_deref(), Some("Add custom agent"));
    assert!(!drawn(cx, "settings-section-custom-example-code"));
    keys(cx, "escape");
    assert!(!any_open(cx), "the deleted draft must not ask to be saved");
    assert!(
        sent(&mut helm).is_empty(),
        "closing must not re-register it"
    );
}

/// SPEC.md, "Session list": "The gear always opens sessions and focuses its first switch". A Settings that opened on
/// another section, or with focus outside it, would leave a keyboard user hunting for where they are. Specified: the
/// gear's click opens one dialog showing the Sessions heading, with Skip session confirmations holding focus.
#[gpui::test]
fn the_gear_opens_settings_at_sessions_with_focus_on_its_first_switch(cx: &mut TestAppContext) {
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    assert_eq!(heading(cx).as_deref(), Some("Sessions"));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(Choice::SkipSessionConfirmations.id()),
        "focus starts on the first switch"
    );
}

/// Settings groups the sections in SPEC.md's order and omits App when the updater is unavailable. The driver should
/// find the group labels and every section button in that order, with Sessions the only pressed button.
#[gpui::test]
fn the_dialog_lists_only_the_sections_this_build_has(cx: &mut TestAppContext) {
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    let listed: Vec<(String, Option<Toggled>)> = frame(cx)
        .into_iter()
        .filter(|node| {
            node.element_id.as_deref().is_some_and(|id| {
                id.starts_with("settings-section-") || id.starts_with("settings-group")
            })
        })
        .map(|node| {
            (
                node.label
                    .map(|label| label.to_string())
                    .unwrap_or_default(),
                node.toggled,
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            ("General".to_string(), None),
            ("Sessions".to_string(), Some(Toggled::True)),
            ("Appearance".to_string(), Some(Toggled::False)),
            ("Terminal".to_string(), Some(Toggled::False)),
            ("Notifications".to_string(), Some(Toggled::False)),
            ("Credentials".to_string(), Some(Toggled::False)),
            ("Where sessions run".to_string(), None),
            ("Hosts".to_string(), Some(Toggled::False)),
            ("Checkouts".to_string(), Some(Toggled::False)),
            ("Containers".to_string(), Some(Toggled::False)),
            ("Recipes".to_string(), Some(Toggled::False)),
            ("Tensorlake".to_string(), Some(Toggled::False)),
            ("Agents".to_string(), None),
            ("Cursor".to_string(), Some(Toggled::False)),
            ("Codex".to_string(), Some(Toggled::False)),
            ("Claude Code".to_string(), Some(Toggled::False)),
            ("Muse Code".to_string(), Some(Toggled::False)),
            ("Goose".to_string(), Some(Toggled::False)),
            ("Pi".to_string(), Some(Toggled::False)),
            ("OMP".to_string(), Some(Toggled::False)),
            ("OpenCode".to_string(), Some(Toggled::False)),
            ("Grok".to_string(), Some(Toggled::False)),
            ("Antigravity".to_string(), Some(Toggled::False)),
            ("Custom agents".to_string(), None),
            ("+ Add custom agent".to_string(), Some(Toggled::False)),
        ]
    );
}

/// App belongs at the end of General when the updater is available. Without it the updater's controls have no
/// target, so the preceding test checks that App is absent. With the updater, its button opens the App section and
/// keeps focus on the button.
#[gpui::test]
fn app_is_listed_under_general_while_the_desktop_updater_runs(cx: &mut TestAppContext) {
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    keep_updater(cx, true, &Rc::default(), &Rc::default());
    open_settings(cx);
    let listed: Vec<String> = frame(cx)
        .into_iter()
        .filter(|node| {
            node.element_id
                .as_deref()
                .is_some_and(|id| id.starts_with("settings-section-"))
        })
        .filter_map(|node| node.label.map(|label| label.to_string()))
        .collect();
    assert_eq!(
        listed.iter().map(String::as_str).collect::<Vec<_>>(),
        [
            "Sessions",
            "Appearance",
            "Terminal",
            "Notifications",
            "Credentials",
            "App",
            "Hosts",
            "Checkouts",
            "Containers",
            "Recipes",
            "Tensorlake",
            "Cursor",
            "Codex",
            "Claude Code",
            "Muse Code",
            "Goose",
            "Pi",
            "OMP",
            "OpenCode",
            "Grok",
            "Antigravity",
            "+ Add custom agent",
        ]
    );
    click(cx, section_button(Section::App));
    assert_eq!(heading(cx).as_deref(), Some("App"));
    assert_eq!(focused_id(cx).as_deref(), Some("settings-section-app"));
}

/// A publishing updater redraws the version row in an already-open App section. The check retains focus during
/// progress, so a repeat keypress neither starts another check nor presses the automatic-updates switch.
#[gpui::test]
fn app_version_checks_once_and_follows_the_updaters_busy_state(cx: &mut TestAppContext) {
    use super::app::{CHECK_ID, VERSION_ID};
    use crate::desktop::updates::fake::readout_saying;
    use crate::update_readout::Activity;

    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let writes = Rc::default();
    let (feed, seen) = keep_updater(cx, true, &Rc::default(), &writes);
    open_settings(cx);
    click(cx, section_button(Section::App));
    assert_eq!(
        label(cx, VERSION_ID).as_deref(),
        Some(farhelm_proto::BUILD_VERSION)
    );
    assert_eq!(label(cx, CHECK_ID).as_deref(), Some("Check for updates"));
    click(cx, CHECK_ID);
    assert_eq!(seen.checks.get(), 1);
    reach(cx, CHECK_ID);

    // Read the last painted frame. The shared label helper would refresh it and mask a missed updater notification.
    let check_in_frame = |cx: &mut VisualTestContext| {
        cx.update(|window, _| {
            window
                .driver_nodes()
                .iter()
                .find(|node| node.element_id.as_deref() == Some(CHECK_ID))
                .map(|node| (node.label.clone(), node.flags.focused, node.flags.click))
        })
    };

    for (activity, expected) in [
        (Activity::Checking, "Checking…"),
        (Activity::Installing("1.2.3".into()), "Installing…"),
    ] {
        let mut readout = readout_saying("progress");
        readout.activity = activity;
        feed.publish(Some(readout));
        settle(cx);
        assert_eq!(
            check_in_frame(cx),
            Some((Some(expected.into()), true, false))
        );
        keys(cx, "space");
        click(cx, CHECK_ID);
        assert_eq!(seen.checks.get(), 1, "{expected} refuses another check");
    }
    assert!(writes.borrow().is_empty());

    let mut readout = readout_saying("finished");
    readout.activity = Activity::UpToDate("1.2.3".into());
    feed.publish(Some(readout));
    settle(cx);
    assert_eq!(
        check_in_frame(cx),
        Some((Some("Check for updates".into()), true, true))
    );
    click(cx, CHECK_ID);
    assert_eq!(seen.checks.get(), 2);
}

/// The App switch reads the helm choice and writes the running version with it. This lets re-enabling defer
/// highlights until a later upgrade, even if this build's highlights have never been downloaded.
#[gpui::test]
fn enabling_upgrade_highlights_starts_from_the_running_release(cx: &mut TestAppContext) {
    let preferences = Preferences {
        show_whats_new: Some(false),
        ..Preferences::default()
    };
    let (mut helm, cx, _gear) = open_rail(cx, &preferences);
    let host = crate::whats_new::tests::test_host(Box::new(|_, _| {
        panic!("opening Settings needs no release lookup");
    }));
    let version = host.upgrade_notes.as_ref().unwrap().running.clone();
    cx.update(|_, cx| crate::desktop::updates::keep(Some(host), cx));
    open_settings(cx);
    click(cx, section_button(Section::App));
    let id = Choice::ShowWhatsNew.id();
    assert_eq!(
        switch_on(cx, id),
        Some(false),
        "premise: the helm opted out"
    );
    click(cx, id);
    let (_, body) = one_command(&mut helm, Method::Put, "/api/preferences");
    assert_eq!(
        body,
        json!({"show_whats_new": true, "last_whats_new_release": version})
    );
}

/// SPEC.md: the App switch installs updates automatically, "on by default", and "unlike the host choices it is a
/// setting of that app installation, kept with the app rather than by the helm"; a refused write must not show as
/// saved, since the user would then believe updates install on their own (the old switch: "Only a saved choice is shown
/// as the choice"). Specified: over an updater whose setting is on, the section shows its Mac scope and the switch on
/// with the on help. A press writes off through the updater, sends the helm nothing and holds no preference save; the
/// switch shows off with help naming the ways to update by hand. A press the updater refuses shows the switch still
/// off, with the updater's reason under it, which a section switch keeps. The next press, saved, turns it on and takes
/// the reason away.
#[gpui::test]
fn the_app_switch_writes_the_apps_own_setting_and_shows_only_what_was_saved(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let (refuse, writes) = (Rc::new(Cell::new(false)), Rc::new(RefCell::new(Vec::new())));
    keep_updater(cx, true, &refuse, &writes);
    open_settings(cx);
    click(cx, section_button(Section::App));
    let _ = sent(&mut helm);
    assert!(painted(cx).contains("This Mac"));
    assert_eq!(switch_on(cx, super::app::ID), Some(true));
    assert!(
        painted(cx)
            .contains("installs it in the background. Restarting Farhelm finishes the update.")
    );

    click(cx, super::app::ID);
    assert_eq!(*writes.borrow(), [false]);
    assert!(sent(&mut helm).is_empty(), "the helm was sent nothing");
    assert_eq!(
        preference_work(cx),
        None,
        "and no preference save holds its key"
    );
    assert_eq!(switch_on(cx, super::app::ID), Some(false));
    assert!(painted(cx).contains(
        "Farhelm installs a new release only when you check for updates (above, from the version in the sidebar, the ? \
         menu or this machine's update item) or run the installer yourself."
    ));
    assert!(!drawn(cx, super::app::ERROR_ID));

    refuse.set(true);
    click(cx, super::app::ID);
    assert_eq!(
        *writes.borrow(),
        [false, true],
        "the premise: the press reached the updater"
    );
    assert_eq!(
        switch_on(cx, super::app::ID),
        Some(false),
        "a refused write shows the setting in force"
    );
    assert!(painted(cx).contains("The choice could not be saved"));
    click(cx, section_button(Section::Sessions));
    click(cx, section_button(Section::App));
    assert!(
        drawn(cx, super::app::ERROR_ID),
        "the reason survives a section switch"
    );

    refuse.set(false);
    click(cx, super::app::ID);
    assert_eq!(switch_on(cx, super::app::ID), Some(true));
    assert!(
        !drawn(cx, super::app::ERROR_ID),
        "a saved press takes the reason away"
    );
    assert!(sent(&mut helm).is_empty());
}

/// App's switch writes the app's own setting through the updater, never through the operation store, so a helm save
/// on the preference row's key must not hold it back: every other Settings control is refused while such a save is in
/// flight, and a regression that routed App through the store would refuse it too, with the busy line. Specified: with
/// Skip session confirmations' save sent and unanswered (the premise: preference work in flight), a press on App's
/// switch reaches the updater as a write of off, the switch shows off, no busy line is drawn under it, and the
/// Sessions save is still the one in flight.
#[gpui::test]
fn the_app_switch_writes_while_a_preference_save_is_unanswered(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let writes = Rc::new(RefCell::new(Vec::new()));
    keep_updater(cx, true, &Rc::default(), &writes);
    open_settings(cx);
    click(cx, Choice::SkipSessionConfirmations.id());
    let _ = one_save(&mut helm);
    assert_eq!(
        preference_work(cx),
        Some(Work::Save),
        "the premise: the Sessions save is in flight"
    );

    click(cx, section_button(Section::App));
    click(cx, super::app::ID);
    assert_eq!(*writes.borrow(), [false], "the write reached the updater");
    assert_eq!(switch_on(cx, super::app::ID), Some(false));
    assert_eq!(
        busy_line(cx, super::app::ID),
        None,
        "nothing says it was refused"
    );
    assert_eq!(
        preference_work(cx),
        Some(Work::Save),
        "the Sessions save is still out"
    );
    assert!(sent(&mut helm).is_empty());
}

/// The header's "and for this device" is true only where App is listed, since App's switch is the one setting in the
/// dialog that belongs to this device; elsewhere it would claim device settings the dialog does not have (the
/// migration's orchestrator decided on 2026-11-15 that the words follow whether App is listed). Specified: with no
/// updater the header reads "For every host on this helm" without "and for this device"; once an updater is kept and
/// Settings opens again, it reads "For every host on this helm, and for this device".
#[gpui::test]
fn the_header_says_this_device_only_where_app_is_listed(cx: &mut TestAppContext) {
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    let without = painted(cx);
    assert!(without.contains("For every host on this helm"));
    assert!(!without.contains("and for this device"));
    keys(cx, "escape");
    assert!(!any_open(cx), "the premise: Settings closed");

    keep_updater(cx, true, &Rc::default(), &Rc::default());
    open_settings(cx);
    assert!(painted(cx).contains("For every host on this helm, and for this device"));
}

/// SPEC.md: "Escape and either close control return focus to the gear", and the old app returned it there on every
/// platform, also on macOS, where a click leaves a button unfocused, so focus would otherwise go back to whatever held it
/// before (the terminal). Run under macOS's click rule for that reason. Specified: with a click that focuses no button,
/// the gear's click leaves the gear unfocused, and nothing focuses it before Settings opens: Settings names it as where
/// closing returns focus. Escape, the header's "Close settings" and the footer's Close each close the dialog and leave
/// focus on the gear.
#[gpui::test]
fn escape_and_both_close_controls_return_focus_to_the_gear(cx: &mut TestAppContext) {
    let (_helm, cx, gear) = open_rail(cx, &Preferences::default());
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    test_support::check(cx);
    for close in ["escape", "settings-close", "settings-footer-close"] {
        // Start each round with focus away from the gear, as after typing in a terminal: the control before it, which
        // Tab reaches first from nothing focused and Shift+Tab from the gear.
        let away = if focused(cx, &gear) {
            "shift-tab"
        } else {
            "tab"
        };
        keys(cx, away);
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("elsewhere"),
            "{close}: the premise: focus is away from the gear"
        );
        open_settings(cx);
        if close == "escape" {
            keys(cx, "escape");
        } else {
            click(cx, close);
        }
        assert!(!any_open(cx), "{close} closed Settings");
        assert!(
            focused(cx, &gear),
            "{close} returned focus to the gear, which the click never focused"
        );
    }
}

/// Why this matters: Enter on a dialog itself presses the dialog's main action (SPEC.md, "Opening and closing"), and
/// Settings has none on purpose (`settings.rs`), since every choice in it saves on its own; a main action there would
/// toggle or save something the user never pressed. The old spec `settings.spec.ts`, "settings gap keeps focus while
/// text and container Enter keep their own behavior", pinned that container Enter changed no choice and wrote no
/// preference. Specified: with Settings open at Sessions and Skip session confirmations off, focus moved onto the
/// dialog itself, Enter leaves Settings open with the dialog itself still holding focus, the switch still off, and
/// nothing sent.
#[gpui::test]
fn enter_on_the_dialog_itself_does_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    let skip = Choice::SkipSessionConfirmations.id();
    assert_eq!(switch_on(cx, skip), Some(false), "the premise: it is off");
    // Whatever opening sent is not what this test is about.
    let _ = sent(&mut helm);
    // Focus on the dialog itself, as the layer manager recovers it there when the focused control goes; a click on
    // Settings' empty space keeps focus on the control in use instead (its `keeps_focus_on_empty_click`).
    cx.update(|window, cx| {
        let dialog = layers::top_container(window, cx).expect("Settings is open");
        let mark = Mark::take(cx);
        focus::focus_in_layer(&dialog, &mark, window, cx);
    });
    settle(cx);
    assert!(
        dialog_holds_focus(cx),
        "the premise: the dialog itself has focus"
    );

    keys(cx, "enter");
    assert!(any_open(cx), "Settings stayed open");
    assert!(dialog_holds_focus(cx), "focus stayed on the dialog itself");
    assert_eq!(
        switch_on(cx, skip),
        Some(false),
        "the switch did not toggle"
    );
    let commands = sent(&mut helm);
    assert!(commands.is_empty(), "Enter sent nothing: {commands:?}");
}

/// The colors of the 2-pixel rings painted in a fresh frame over the control drawn under `id`: a focus ring is a
/// painted quad with a 2-pixel border that meets the control's bounds (the switch's track has a one-pixel edge and its
/// knob none), as `components/tests/list_row.rs` finds a row's. The off switch's ring stands outside its track, which
/// ends at the control's right edge, so the quad is matched by meeting the control's bounds, not by lying inside them.
fn rings_at(cx: &mut VisualTestContext, id: &'static str) -> Vec<gpui::Hsla> {
    test_support::redraw(cx);
    let control = bounds(cx, id);
    let (scale, quads) = cx.update(|window, _| (window.scale_factor(), window.painted_quads()));
    let (control, width) = (control.scale(scale), px(2.).scale(scale));
    quads
        .into_iter()
        .filter(|quad| quad.border_widths.top == width && quad.bounds.intersects(&control))
        .map(|quad| quad.border_color)
        .collect()
}

/// Why this matters: a keyboard user must see which switch holds focus, and on a switch that is on, an accent ring
/// would sink into the accent track; the old stylesheet drew that ring in the on-fill ink inside the track
/// (`.control-switch:checked:focus-visible`, offset -3px), and the old spec `settings.spec.ts`, "settings focus stays
/// on its painted fill and descriptions stay with their labels", checked it on a focused switch that was on. Specified:
/// with Skip session confirmations off and Name sessions on quick launch on (the helm stored no choice), Tab from the
/// first switch to the naming switch adds a 2-pixel ring over it, in the on-fill ink only; Shift+Tab back adds one over
/// the first switch, in the accent only.
#[gpui::test]
fn a_focused_switch_that_is_on_wears_its_ring_in_the_on_fill_ink(cx: &mut TestAppContext) {
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    let (skip, naming) = (
        Choice::SkipSessionConfirmations.id(),
        Choice::NameSessionsOnQuickLaunch.id(),
    );
    assert_eq!(
        (switch_on(cx, skip), switch_on(cx, naming)),
        (Some(false), Some(true)),
        "the premise: the first switch is off and the naming switch on"
    );
    let (accent, on_fill) = cx.update(|_, cx| {
        let palette = theme::palette(cx);
        (theme::color(palette.accent), theme::color(palette.on_fill))
    });
    assert_ne!(accent, on_fill, "the premise: the two inks differ");

    // Other 2-pixel borders can meet a switch's bounds whether or not it holds focus (a track's own edge, a
    // neighbour's), so each check takes what focus adds: the quads over the switch focused, less those over it
    // unfocused.
    let unfocused = rings_at(cx, naming);
    keys(cx, "tab");
    assert_eq!(focused_id(cx).as_deref(), Some(naming));
    let ring = added(&unfocused, rings_at(cx, naming));
    assert!(
        !ring.is_empty() && ring.iter().all(|color| *color == on_fill),
        "the naming switch's ring, in the on-fill ink: {ring:?}"
    );
    let unfocused = rings_at(cx, skip);
    keys(cx, "shift-tab");
    assert_eq!(focused_id(cx).as_deref(), Some(skip));
    let ring = added(&unfocused, rings_at(cx, skip));
    assert!(
        !ring.is_empty() && ring.iter().all(|color| *color == accent),
        "the first switch's ring, in the accent: {ring:?}"
    );
}

/// The colors in `now` beyond those in `before`, each of `before`'s matched once: what a change painted over a
/// control, apart from what was there already.
fn added(before: &[gpui::Hsla], mut now: Vec<gpui::Hsla>) -> Vec<gpui::Hsla> {
    for color in before {
        if let Some(at) = now.iter().position(|seen| seen == color) {
            now.remove(at);
        }
    }
    now
}

/// Each card row keeps its description beside the toggle it explains. A description that overflows the row can read
/// as part of the next switch, especially when the copy wraps.
#[gpui::test]
fn each_switchs_help_sits_under_it_nearer_than_the_next_switch(cx: &mut TestAppContext) {
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    let choices = [
        Choice::SkipSessionConfirmations,
        Choice::NameSessionsOnQuickLaunch,
        Choice::ColorTerminalFrames,
    ];
    let lines: Vec<_> = frame(cx).into_iter().flat_map(|node| node.texts).collect();
    for choice in choices {
        let row = bounds(cx, choice.id());
        let title = lines
            .iter()
            .find(|line| line.text.as_ref() == choice.label())
            .expect("the switch's label is painted");
        let help = lines
            .iter()
            .find(|line| {
                let words = line.text.trim();
                !words.is_empty() && choice.help(false).starts_with(words)
            })
            .expect("the switch's description is painted");
        assert!(
            row.top() <= title.bounds.top(),
            "{choice:?} starts with its title"
        );
        assert!(
            title.bounds.top() < help.bounds.top(),
            "{choice:?} places help under the title"
        );
        assert!(
            help.bounds.bottom() <= row.bottom(),
            "{choice:?} contains its help"
        );
    }
}

/// The frame must not move when the user switches sections (SPEC.md: "The dialog keeps one height across sections,
/// capped by the window"); a section change that resized it would jump Close away from under the pointer. Specified: in
/// a window 1000 pixels tall, Settings is 760 by 780 pixels and stays exactly where it was, its header close control
/// too, when Hosts is shown; in a window 600 pixels tall it is 60 pixels shorter than the window (the frame's margins,
/// 5% of the height above and below, at least 24 pixels each).
#[gpui::test]
fn the_frame_keeps_one_size_across_sections_capped_by_the_window(cx: &mut TestAppContext) {
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    let frame_at_sessions = bounds(cx, "settings");
    let close_at_sessions = bounds(cx, "settings-close");
    assert_eq!(
        (frame_at_sessions.size.width, frame_at_sessions.size.height),
        (px(760.), px(780.))
    );
    click(cx, "settings-section-hosts");
    assert_eq!(heading(cx).as_deref(), Some("Hosts"));
    assert_eq!(
        bounds(cx, "settings"),
        frame_at_sessions,
        "the frame did not move"
    );
    assert_eq!(
        bounds(cx, "settings-close"),
        close_at_sessions,
        "nor did its close control"
    );

    cx.simulate_resize(size(px(1000.), px(600.)));
    settle(cx);
    // Capped by the window less the dialog margin above and below: 5% of the height, at least 24 pixels
    // (`max(24px, 5dvh)` in the old stylesheet), so 30 pixels each in a 600-pixel window.
    assert_eq!(bounds(cx, "settings").size.height, px(600. - 2. * 30.));
}

/// SPEC.md, "Reaching everything by keyboard": a control Tab reaches must come into view, and in a short window the
/// Sessions section is taller than the page viewport. The page itself must reveal the switch without moving the
/// heading or the section list. Specified: in a window 400 pixels tall, the last Sessions switch starts below the
/// fold. After two Tabs it has focus and lies wholly inside the viewport, with the heading and navigation unmoved.
#[gpui::test]
fn a_switch_tab_reaches_below_the_fold_scrolls_into_view(cx: &mut TestAppContext) {
    let frames = Choice::ColorTerminalFrames.id();
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    cx.simulate_resize(size(px(1000.), px(400.)));
    settle(cx);
    open_settings(cx);
    let body = bounds(cx, DIALOG_BODY);
    let heading = bounds(cx, "settings-heading");
    let sessions = bounds(cx, "settings-section-sessions");
    assert!(
        bounds(cx, frames).bottom() > body.bottom(),
        "the premise: the last switch is below the fold"
    );
    keys(cx, "tab tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(frames),
        "the premise: Tab reached it"
    );
    assert!(
        shows_whole(cx, frames, DIALOG_BODY),
        "the switch {:?} is inside the body's visible part {:?}",
        bounds(cx, frames),
        bounds(cx, DIALOG_BODY)
    );
    assert_eq!(bounds(cx, "settings-heading"), heading);
    assert_eq!(bounds(cx, "settings-section-sessions"), sessions);
}

/// At the minimum window height, the sidebar needs to scroll to reach the last agent.
/// Wheel the section list and verify it exposes that row while the search field and page stay put.
#[gpui::test]
fn long_settings_navigation_scrolls_without_moving_search_or_page(cx: &mut TestAppContext) {
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    cx.simulate_resize(size(px(720.), px(560.)));
    settle(cx);
    open_settings(cx);
    let list = bounds(cx, "settings-navigation-region");
    let bottom = section_button(Section::Agent(LaunchHarness::Antigravity));
    let search = bounds(cx, "settings-search-field");
    let heading = bounds(cx, "settings-heading");
    assert!(
        bounds(cx, bottom).bottom() > list.bottom(),
        "the last row starts below the fold"
    );
    cx.simulate_event(gpui::ScrollEvent {
        position: list.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-5000.))),
        ..Default::default()
    });
    settle(cx);
    assert!(shows_whole(cx, bottom, "settings-navigation-region"));
    assert_eq!(bounds(cx, "settings-search-field"), search);
    assert_eq!(bounds(cx, "settings-heading"), heading);
}

// ---- Saving the switches -------------------------------------------------------------------------------------------

/// Each switch writes one field of the shared preference row, and a sparse body naming only it keeps a stale copy of
/// another field from being written back over a change made since (an agent's, say): the helm's `put_preferences`
/// merges only the fields named. It is one request under the preference row's key, so the store shows it busy while it
/// runs. Specified: for every switch, in its section, a click sends `PUT /api/preferences` with only its field, set to
/// the opposite of the stored value, while the preference row's key holds a save; once the helm pushes the new row and
/// answers, the switch shows the new value and the key is free.
#[gpui::test]
fn each_switch_saves_its_own_preference_field_under_the_preferences_key(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    let mut row = Preferences::default();
    let table = [
        (
            Section::Sessions,
            Choice::SkipSessionConfirmations,
            "skip_session_confirmations",
            true,
        ),
        (
            Section::Sessions,
            Choice::NameSessionsOnQuickLaunch,
            "name_session_on_quick_launch",
            false,
        ),
        (
            Section::Sessions,
            Choice::ColorTerminalFrames,
            "color_terminal_frames",
            true,
        ),
        (
            Section::Hosts,
            Choice::AutoUpdateRemoteHosts,
            "auto_update_remote_hosts",
            false,
        ),
        (
            Section::Hosts,
            Choice::SkipHostSetupConfirmation,
            "skip_host_setup_confirmation",
            true,
        ),
        (
            Section::Hosts,
            Choice::SkipHostRemoveConfirmation,
            "skip_host_remove_confirmation",
            true,
        ),
    ];
    for (section, choice, field, sent_value) in table {
        click(cx, section_button(section));
        assert_eq!(
            switch_on(cx, choice.id()),
            Some(!sent_value),
            "{field}: the premise: the stored value shows"
        );
        click(cx, choice.id());
        let (id, body) = one_save(&mut helm);
        let mut only = serde_json::Map::new();
        only.insert(field.to_owned(), Value::Bool(sent_value));
        assert_eq!(body, Value::Object(only), "{field}: only its field is sent");
        assert_eq!(
            preference_work(cx),
            Some(Work::Save),
            "{field}: saved under the preferences key"
        );

        row = serde_json::from_value({
            let mut value = serde_json::to_value(&row).unwrap();
            value[field] = json!(sent_value);
            value
        })
        .unwrap();
        push(&mut helm, &row, cx);
        helm.reply(id, 204, None);
        settle(cx);
        assert_eq!(preference_work(cx), None, "{field}: the key is free again");
        assert_eq!(
            switch_on(cx, choice.id()),
            Some(sent_value),
            "{field}: the switch follows the stored value"
        );
    }
}

/// SPEC.md: "A failed save restores the last known value and shows an adjacent error", and SPEC_impl.md rules out an
/// optimistic copy: a switch that kept the refused value would claim a choice the helm does not hold. The old spec held
/// the reply to show the switch keeps the clicked value only while the save is pending. Specified: with naming unset
/// (on), a click sends `false` and the switch shows off while the save is in flight; the helm's refusal puts it back on,
/// with the failure line drawn below the switch; a later click that succeeds takes the line away.
#[gpui::test]
fn a_refused_save_restores_the_stored_value_and_shows_the_error_beside_the_switch(
    cx: &mut TestAppContext,
) {
    let naming = Choice::NameSessionsOnQuickLaunch.id();
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, naming);
    let (id, body) = one_save(&mut helm);
    assert_eq!(body, json!({ "name_session_on_quick_launch": false }));
    assert_eq!(
        switch_on(cx, naming),
        Some(false),
        "the pending value shows while the save is in flight"
    );

    helm.reply(id, 503, Some(json!("fixture preference save refused")));
    settle(cx);
    assert_eq!(
        switch_on(cx, naming),
        Some(true),
        "the stored value is back"
    );
    let failure = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("settings-name-quick-launches-error"))
        .map(|node| (node.role, node.label.map(|label| label.to_string())));
    assert_eq!(
        failure,
        Some((
            Some(Role::Alert),
            Some("Could not save whether to name quick-launched sessions. Try again.".to_string())
        ))
    );
    let switch = bounds(cx, "settings-name-quick-launches");
    let line = bounds(cx, "settings-name-quick-launches-error");
    assert!(
        line.top() >= switch.bottom(),
        "the failure line is below its switch"
    );

    click(cx, naming);
    let (id, _) = one_save(&mut helm);
    push(
        &mut helm,
        &Preferences {
            name_session_on_quick_launch: Some(false),
            ..Preferences::default()
        },
        cx,
    );
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(switch_on(cx, naming), Some(false));
    assert!(
        !drawn(cx, "settings-name-quick-launches-error"),
        "the successful save took the failure line away"
    );
}

/// The switches must show the same defaults before and after the helm sends an empty row. Naming and automatic remote
/// updates start on and respect a saved opt-out. Confirmation skips and frame coloring require an explicit opt-in.
/// Hints use the inverse of their saved dismissal, and the sound rules test the sound defaults.
#[test]
fn name_sessions_on_quick_launch_reads_as_on_when_the_helm_stored_no_choice() {
    let empty = Preferences::default();
    let naming = Choice::NameSessionsOnQuickLaunch;
    let updates = Choice::AutoUpdateRemoteHosts;
    let off = Preferences {
        name_session_on_quick_launch: Some(false),
        auto_update_remote_hosts: Some(false),
        ..Preferences::default()
    };
    for choice in [naming, updates] {
        assert!(choice.stored(None), "{choice:?} with no row");
        assert!(choice.stored(Some(&empty)), "{choice:?} with an empty row");
        assert!(
            !choice.stored(Some(&off)),
            "{choice:?} with an explicit false"
        );
    }
    let all_on: Preferences = serde_json::from_value(json!({
        "skip_session_confirmations": true,
        "color_terminal_frames": true,
        "skip_host_setup_confirmation": true,
        "skip_host_remove_confirmation": true,
    }))
    .unwrap();
    let hints = [
        Choice::ShowClaudeTrustHint,
        Choice::ShowCodexTrustHint,
        Choice::ShowMuseTrustHint,
        Choice::ShowAntigravityTrustHint,
        Choice::ShowHookSetupHint(crate::hook_setup::HookAgent::Grok),
        Choice::ShowHookSetupHint(crate::hook_setup::HookAgent::Antigravity),
    ];
    for &choice in Choice::ALL.iter().filter(|&&choice| {
        choice != naming
            && choice != updates
            && choice != Choice::ShowWhatsNew
            && choice != Choice::CheckAnnouncements
            && !hints.contains(&choice)
            && !SOUNDS.contains(&choice)
    }) {
        assert!(
            !choice.stored(None) && !choice.stored(Some(&empty)),
            "{choice:?} reads unset as off"
        );
        assert!(choice.stored(Some(&all_on)), "{choice:?} reads true as on");
    }
    let opted_out = Preferences {
        show_whats_new: Some(false),
        check_announcements: Some(false),
        ..Preferences::default()
    };
    assert!(Choice::ShowWhatsNew.stored(None) && Choice::ShowWhatsNew.stored(Some(&empty)));
    assert!(!Choice::ShowWhatsNew.stored(Some(&opted_out)));
    assert!(
        Choice::CheckAnnouncements.stored(None) && Choice::CheckAnnouncements.stored(Some(&empty))
    );
    assert!(!Choice::CheckAnnouncements.stored(Some(&opted_out)));
    // A hint's switch is its dismissal inverted: "Unset and false both allow the hint".
    let dismissed = |value: bool| -> Preferences {
        serde_json::from_value(json!({
            "claude_trust_hint_dismissed": value,
            "codex_trust_hint_dismissed": value,
            "muse_trust_hint_dismissed": value,
            "antigravity_trust_hint_dismissed": value,
            "grok_hook_hint_dismissed": value,
            "antigravity_hook_hint_dismissed": value,
        }))
        .unwrap()
    };
    for hint in hints {
        assert!(
            hint.stored(None) && hint.stored(Some(&empty)),
            "{hint:?} shows the hint unset"
        );
        assert!(
            hint.stored(Some(&dismissed(false))),
            "{hint:?} shows the hint for an explicit false"
        );
        assert!(
            !hint.stored(Some(&dismissed(true))),
            "{hint:?} hides the hint once dismissed"
        );
    }
}

// ---- Notifications: Sounds ----------------------------------------------------------------------------------------

/// The status sound switches, in the order the Sounds sub-heading draws them.
const SOUNDS: [Choice; 4] = [
    Choice::SoundWaiting,
    Choice::SoundApproval,
    Choice::SoundTurnFinished,
    Choice::SoundSshSignIn,
];

/// Each play button calls the existing player once with the row's sound and leaves its preference untouched.
#[gpui::test]
fn previewing_a_setting_plays_only_the_chosen_sound(cx: &mut TestAppContext) {
    use crate::sounds::{self, Sound};

    struct Played(Rc<RefCell<Vec<Sound>>>);
    impl sounds::Player for Played {
        fn play(&self, sound: Sound, _: &'static [u8]) {
            self.0.borrow_mut().push(sound);
        }
    }

    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_ntfy(&mut helm, cx, ntfy("https://ntfy.sh", "", "default", false));
    let played = Rc::new(RefCell::new(Vec::new()));
    cx.update(|_, cx| sounds::install_player(Rc::new(Played(played.clone())), cx));
    let toggle = Choice::SoundTurnFinished.id();
    assert_eq!(switch_on(cx, toggle), Some(false), "the sound is off");
    click(cx, "settings-sound-turn-finished-preview");
    assert_eq!(*played.borrow(), vec![Sound::TurnFinished]);
    assert_eq!(switch_on(cx, toggle), Some(false));
    assert!(sent(&mut helm).is_empty());
}

/// The Sounds sub-heading is how the user turns each status sound on or off, and each switch must write only its own
/// field of the shared row, so a stale copy of another choice is never written back (the helm's `put_preferences`).
/// Ported from the old spec's settings half (`sounds.spec.ts`, "status sounds follow the current view"): the switches'
/// names and defaults; a choice surviving a reopen is the helm's row here, not the device's storage. The defaults come
/// from the same reading of unset fields the sounds' owner uses (`sounds::rules`'s
/// `the_preference_row_reads_unset_fields_as_the_defaults`). Specified: Notifications shows the heading "Sounds" above
/// ntfy alerts and the four switches by name, SSH sign-in, waiting and approval on and turn finished off by default. A
/// click on each sends `PUT /api/preferences` with only its field, set to the opposite of what it showed, under the
/// preference row's key. After settlement the switch follows a later stored value that differs from the saved draft.
#[gpui::test]
fn each_sound_switch_saves_its_own_preference_field(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    // The ntfy form below the switches reads its settings when the section first shows; answered here, so the switches'
    // saves are answered in the order the fake helm requires.
    show_ntfy(&mut helm, cx, ntfy("https://ntfy.sh", "", "default", false));
    let drawn_at = |cx: &mut VisualTestContext, id: &str| {
        frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .map(|node| node.bounds)
    };
    let heading = drawn_at(cx, "settings-sounds-heading").expect("Sounds is drawn");
    let form = drawn_at(cx, "ntfy-heading").expect("ntfy alerts is drawn");
    assert!(
        heading.bottom() <= form.top(),
        "Sounds is above ntfy alerts"
    );
    let names: Vec<Option<String>> = SOUNDS.iter().map(|choice| label(cx, choice.id())).collect();
    assert_eq!(
        names,
        [
            Some("Session waiting".to_string()),
            Some("Approval request".to_string()),
            Some("Turn finished".to_string()),
            Some("SSH sign-in".to_string())
        ]
    );

    let mut row = Preferences::default();
    let fields = [
        "sound_waiting",
        "sound_approval",
        "sound_turn_finished",
        "sound_ssh_sign_in",
    ];
    for ((choice, field), shown) in SOUNDS
        .into_iter()
        .zip(fields)
        .zip([true, true, false, true])
    {
        assert_eq!(
            switch_on(cx, choice.id()),
            Some(shown),
            "{field}: the default shows"
        );
        click(cx, choice.id());
        let (id, body) = one_save(&mut helm);
        assert_eq!(
            body,
            json!({ field: !shown }),
            "{field}: only its field is sent"
        );
        assert_eq!(
            preference_work(cx),
            Some(Work::Save),
            "{field}: saved under the preferences key"
        );
        row = serde_json::from_value({
            let mut value = serde_json::to_value(&row).unwrap();
            value[field] = json!(!shown);
            value
        })
        .unwrap();
        push(&mut helm, &row, cx);
        helm.reply(id, 204, None);
        settle(cx);
        assert_eq!(
            switch_on(cx, choice.id()),
            Some(!shown),
            "{field}: the stored value shows"
        );
        let mut stored = serde_json::to_value(&row).unwrap();
        stored[field] = json!(shown);
        row = serde_json::from_value(stored).unwrap();
        push(&mut helm, &row, cx);
        assert_eq!(
            switch_on(cx, choice.id()),
            Some(shown),
            "{field}: follows a later push"
        );
    }
}

// ---- Sections keep their state; closing does not stop a save -------------------------------------------------------

/// SPEC.md: "Switching sections preserves unsaved fields, outcome lines and pending saves". A section rebuilt on each
/// switch would drop a pending save's value and a failure line the user has not read yet. It also pins the rule this
/// port has for a click while another preference save is in flight (the module docs): the operation store refuses it,
/// so it sends nothing and changes no switch. Specified: a naming save in flight, Sessions left for Hosts and shown
/// again still shows the pending value; a click on the on-by-default Hosts switch sends nothing and leaves it on; the naming
/// save refused while Hosts is shown, Sessions shows the stored value with the failure line.
#[gpui::test]
fn switching_sections_keeps_a_pending_save_and_a_failure_line(cx: &mut TestAppContext) {
    let naming = Choice::NameSessionsOnQuickLaunch.id();
    let updates = Choice::AutoUpdateRemoteHosts.id();
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, naming);
    let (id, _) = one_save(&mut helm);

    click(cx, "settings-section-hosts");
    assert!(
        switch_on(cx, naming).is_none(),
        "the premise: Sessions is hidden"
    );
    click(cx, "settings-section-sessions");
    assert_eq!(
        switch_on(cx, naming),
        Some(false),
        "the pending value is still shown"
    );

    click(cx, "settings-section-hosts");
    assert_eq!(
        switch_on(cx, updates),
        Some(true),
        "the premise: updates default on"
    );
    click(cx, updates);
    assert_eq!(
        sent(&mut helm),
        [],
        "a second preference save is refused while one is in flight"
    );
    assert_eq!(
        switch_on(cx, updates),
        Some(true),
        "and the switch it came from shows the stored value"
    );

    helm.reply(id, 503, Some(json!("fixture preference save refused")));
    settle(cx);
    click(cx, "settings-section-sessions");
    assert_eq!(switch_on(cx, naming), Some(true));
    assert!(
        drawn(cx, "settings-name-quick-launches-error"),
        "the failure line is still there"
    );
}

/// SPEC_impl.md, "Work that finishes later": a save is the operation store's work, which outlives the view that
/// started it, and the old app's Settings never cancelled a queued write on close. Closing the dialog must neither
/// cancel the save nor leave the key busy for good. Specified: a switch's save in flight when Escape closes Settings is
/// still on the preference row's key; the helm's push and reply settle it with no dialog left; Settings opened again
/// shows the saved value.
#[gpui::test]
fn a_save_in_flight_goes_on_after_the_dialog_closes(cx: &mut TestAppContext) {
    let frames = Choice::ColorTerminalFrames.id();
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, frames);
    let (id, _) = one_save(&mut helm);
    keys(cx, "escape");
    assert!(
        !any_open(cx),
        "the premise: Settings closed with the save in flight"
    );
    assert_eq!(
        preference_work(cx),
        Some(Work::Save),
        "the save is still in flight"
    );

    push(
        &mut helm,
        &Preferences {
            color_terminal_frames: Some(true),
            ..Preferences::default()
        },
        cx,
    );
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(preference_work(cx), None, "the reply settled it");
    open_settings(cx);
    assert_eq!(switch_on(cx, frames), Some(true));
}

// ---- Following the helm ---------------------------------------------------------------------------------------------

/// SPEC.md: a change to these preferences, "by an agent or the command line included, reaches the open window as it
/// happens", and the help now promises such a change shows right away; and "Each switch explains the current
/// behavior". The dialog draws the row from the store rather than a copy, and a copy would only be caught by a push that
/// comes with no save of the dialog's own, since a settling save redraws the dialog anyway. Specified: with Settings open
/// at Hosts and both host switches off, their help says Farhelm asks; the helm pushing a row that skips both
/// confirmations, with nothing sent, turns both switches on and their help to the without-asking wording; pushing them
/// off again brings the asking wording back.
#[gpui::test]
fn an_open_dialog_follows_a_change_made_elsewhere_with_its_help(cx: &mut TestAppContext) {
    let setup = Choice::SkipHostSetupConfirmation.id();
    let remove = Choice::SkipHostRemoveConfirmation.id();
    let asks = [
        "Farhelm shows what setup will change on a new host and asks before doing it.",
        "Farhelm asks before forgetting a host. Its sessions keep running.",
    ];
    let skips = [
        "Farhelm sets up a new host as soon as it has checked it, without asking.",
        "Farhelm forgets a host as soon as you choose remove. Its sessions keep running.",
    ];
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, "settings-section-hosts");
    assert_eq!(
        (switch_on(cx, setup), switch_on(cx, remove)),
        (Some(false), Some(false)),
        "the premise: both off"
    );
    let text = painted(cx);
    assert!(
        asks.iter().all(|help| text.contains(help)),
        "the premise: the help says Farhelm asks: {text}"
    );

    let skipping = Preferences {
        skip_host_setup_confirmation: Some(true),
        skip_host_remove_confirmation: Some(true),
        ..Preferences::default()
    };
    push(&mut helm, &skipping, cx);
    assert_eq!(
        (switch_on(cx, setup), switch_on(cx, remove)),
        (Some(true), Some(true)),
        "both followed the push"
    );
    let text = painted(cx);
    assert!(
        skips.iter().all(|help| text.contains(help)),
        "the help follows the value: {text}"
    );
    assert!(
        !asks.iter().any(|help| text.contains(help)),
        "and no longer says Farhelm asks: {text}"
    );
    assert_eq!(sent(&mut helm), [], "the dialog sent nothing of its own");

    push(&mut helm, &Preferences::default(), cx);
    assert_eq!(
        (switch_on(cx, setup), switch_on(cx, remove)),
        (Some(false), Some(false))
    );
    let text = painted(cx);
    assert!(
        asks.iter().all(|help| text.contains(help)),
        "the asking wording is back: {text}"
    );
}

// ---- Terminal ------------------------------------------------------------------------------------------------------

/// The terminal link option that shows as chosen, by its element id, or `None` when none does or the section is not
/// drawn.
fn link_chosen(cx: &mut VisualTestContext) -> Option<&'static str> {
    let nodes = frame(cx);
    [INTERCEPT, BOTH, PROGRAM].into_iter().find(|&id| {
        nodes.iter().any(|node| {
            node.element_id.as_deref() == Some(id) && node.toggled == Some(Toggled::True)
        })
    })
}

/// The three link options' element ids: the group's id and each option's label (`RadioGroup::new`), spelled out
/// because the harness finds elements by `'static` ids.
const INTERCEPT: &str = "settings-terminal-links-Open in Farhelm, don't pass the click on";
const BOTH: &str = "settings-terminal-links-Open in Farhelm and also pass the click on";
const PROGRAM: &str = "settings-terminal-links-Pass the click on only";

/// The preference row storing `word` as its terminal link choice.
fn link_row(word: &str) -> Preferences {
    Preferences {
        terminal_link_click: Some(word.to_owned()),
        ..Preferences::default()
    }
}

/// SPEC.md, "Session list": Terminal chooses between three link behaviors, and "The latter two choices explain the risk
/// of a second browser tab and that a program without mouse reporting cannot act on a click". Assistive technology and
/// the test bridge find the choice by the group's name and role, as the old browser test did ("terminal link segments
/// keep native keys and descriptions"). Specified: Terminal shows a radio group named "Terminal web links" whose three
/// options are radio buttons in the old order, Intercept chosen when the helm stores none, and each risk sentence on
/// screen; below the group, the switch "Allow tunnels from remote sessions" (a switch, off while the helm stores no
/// consent) with its old help under it; below the switch, the file-types field (`super::file_types_tests` has its
/// own). Decided for the Mac, the one platform that draws the switch.
#[gpui::test]
fn the_terminal_section_offers_the_link_choices_with_their_risks(cx: &mut TestAppContext) {
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    on_the_mac(cx);
    click(cx, "settings-section-terminal");
    assert_eq!(heading(cx).as_deref(), Some("Terminal"));
    let nodes = frame(cx);
    let role_and_name = |id: &str| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .map(|node| {
                (
                    node.role,
                    node.label.as_ref().map(|label| label.to_string()),
                )
            })
    };
    assert_eq!(
        role_and_name(super::terminal::ID),
        Some((
            Some(Role::RadioGroup),
            Some("Terminal web links".to_string())
        ))
    );
    for option in [INTERCEPT, BOTH, PROGRAM] {
        assert_eq!(
            role_and_name(option).and_then(|(role, _)| role),
            Some(Role::RadioButton),
            "{option} is a radio button"
        );
    }
    assert_eq!(link_chosen(cx), Some(INTERCEPT), "unset reads as Intercept");
    let (intercept, both, program) = (bounds(cx, INTERCEPT), bounds(cx, BOTH), bounds(cx, PROGRAM));
    assert!(
        intercept.bottom() <= both.top() && both.bottom() <= program.top(),
        "the choices are stacked in the old order"
    );
    let text = painted(cx);
    for risk in [
        "A program that opens links itself may open a second tab.",
        "without mouse reporting, clicking a link does nothing.",
    ] {
        assert!(text.contains(risk), "the risk is explained: {risk}: {text}");
    }
    assert_eq!(
        role_and_name(super::terminal::TUNNEL_ID),
        Some((
            Some(Role::Switch),
            Some("Allow tunnels from remote sessions".to_string())
        ))
    );
    assert_eq!(
        switch_on(cx, super::terminal::TUNNEL_ID),
        Some(false),
        "unset reads as off"
    );
    assert!(
        bounds(cx, super::terminal::TUNNEL_ID).top() >= program.bottom(),
        "the switch is below the link choices"
    );
    assert!(
        text.contains(
            "Open a remote session's localhost links through this Mac. Turning this off closes every open tunnel. \
             The next tunnel asks again."
        ),
        "the switch's help is on screen: {text}"
    );
    assert!(
        bounds(cx, super::file_types::ID).top() >= bounds(cx, super::terminal::TUNNEL_ID).bottom(),
        "the file-types field is below the switch"
    );
}

/// The link choice is one field of the shared row, so a stale copy of another field is never written back, and the
/// terminals follow only what the helm stores. Specified: a click on "Pass the click on only" sends
/// `PUT /api/preferences` with only `{"terminal_link_click": "program"}` under the preference row's key and shows that
/// choice while the save is in flight; once the helm pushes the row and answers, it shows from the store and the key is
/// free; a push of `both` from elsewhere, with nothing sent, then shows the middle choice.
#[gpui::test]
fn a_link_choice_saves_only_its_preference_field_and_follows_the_helm(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, "settings-section-terminal");
    click(cx, PROGRAM);
    let (id, body) = one_save(&mut helm);
    assert_eq!(body, json!({ "terminal_link_click": "program" }));
    assert_eq!(preference_work(cx), Some(Work::Save));
    assert_eq!(link_chosen(cx), Some(PROGRAM), "the pending value shows");

    push(&mut helm, &link_row("program"), cx);
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(preference_work(cx), None, "the key is free again");
    assert_eq!(link_chosen(cx), Some(PROGRAM), "the stored value shows");

    push(&mut helm, &link_row("both"), cx);
    assert_eq!(link_chosen(cx), Some(BOTH), "a change made elsewhere shows");
    assert_eq!(sent(&mut helm), [], "the dialog sent nothing of its own");
}

/// SPEC.md lists link clicks among the choices that "put back the saved choice and show an adjacent error when their
/// newest save fails", and with no optimistic copy a refused choice never reached a terminal. Specified: a click on
/// "Open in Farhelm and also pass the click on" shows it while the save is in flight; the helm's refusal brings the
/// stored Intercept back with the old failure line, an alert, below the choices.
#[gpui::test]
fn a_refused_link_choice_shows_the_stored_choice_and_a_failure_line(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, "settings-section-terminal");
    click(cx, BOTH);
    let (id, _) = one_save(&mut helm);
    assert_eq!(
        link_chosen(cx),
        Some(BOTH),
        "the premise: the pending value shows"
    );

    helm.reply(id, 503, Some(json!("fixture preference save refused")));
    settle(cx);
    assert_eq!(link_chosen(cx), Some(INTERCEPT), "the stored value is back");
    let failure = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(super::terminal::ERROR_ID))
        .map(|node| (node.role, node.label.map(|label| label.to_string())));
    assert_eq!(
        failure,
        Some((
            Some(Role::Alert),
            Some("Could not save how terminal links are clicked. Try again.".to_string())
        ))
    );
    assert!(
        bounds(cx, super::terminal::ERROR_ID).top() >= bounds(cx, super::terminal::ID).bottom(),
        "the failure line is below the choices"
    );
}

/// The helm validates `intercept`, `both` and `program`; anything else must read as the default rather than as no
/// choice, as the old app's upgrade default did. Specified: no row, an unset field and an unknown word read as
/// Intercept; `both` and `program` read as themselves.
#[test]
fn an_unset_or_unknown_link_choice_reads_as_intercept() {
    use super::terminal::{LinkClick, stored};
    assert_eq!(stored(None), LinkClick::Intercept);
    assert_eq!(stored(Some(&Preferences::default())), LinkClick::Intercept);
    assert_eq!(stored(Some(&link_row("hover"))), LinkClick::Intercept);
    assert_eq!(stored(Some(&link_row("both"))), LinkClick::Both);
    assert_eq!(stored(Some(&link_row("program"))), LinkClick::Program);
}

// ---- Tunnel consent ------------------------------------------------------------------------------------------------

/// The tunnel switch's element id, short for the tests below.
const TUNNELS: &str = super::terminal::TUNNEL_ID;

/// The preference row storing `consent` as the tunnel consent.
fn consent_row(consent: bool) -> Preferences {
    Preferences {
        tunnel_consent: Some(consent),
        ..Preferences::default()
    }
}

/// Whether the tunnel warning is up: its Allow is drawn (Settings itself is an open layer too, so `any_open` cannot
/// tell).
fn warning_up(cx: &mut VisualTestContext) -> bool {
    drawn(cx, "prompt-confirm")
}

/// The tunnel switch's failure line, by its role and accessible name, if drawn.
fn tunnel_failure(cx: &mut VisualTestContext) -> Option<(Option<Role>, Option<String>)> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(super::terminal::TUNNEL_ERROR_ID))
        .map(|node| (node.role, node.label.map(|label| label.to_string())))
}

/// Decide the open Settings dialog's tunnel switch for `platform` whatever machine runs the test, through the dialog's
/// test-only setter (the production path reads the same field, set from this build's platform). Takes effect at the next
/// draw.
fn decide_tunnels_for(cx: &mut VisualTestContext, platform: Platform) {
    cx.update(|_, cx| super::decide_tunnels_for_opened(platform, cx));
}

/// [`decide_tunnels_for`] the Mac, the platform that draws the switch.
fn on_the_mac(cx: &mut VisualTestContext) {
    decide_tunnels_for(cx, Platform::Mac);
}

/// Settings at Terminal over a row storing `preferences`, decided for the Mac, with the tunnel switch drawn.
fn show_terminal<'a>(
    cx: &'a mut TestAppContext,
    preferences: &Preferences,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (helm, cx, _gear) = open_rail(cx, preferences);
    open_settings(cx);
    on_the_mac(cx);
    click(cx, section_button(Section::Terminal));
    assert!(drawn(cx, TUNNELS), "the premise: the switch is drawn");
    (helm, cx)
}

/// Only the Mac opens tunnels (SPEC.md: "In the Mac app, a loopback link in a remote session's terminal offers
/// Cmd-click to tunnel"), and the switch, its help and its warning all speak of this Mac, so elsewhere the switch would
/// be a control for nothing with words about another machine. Specified: decided for Linux, Terminal draws the link
/// choices and no tunnel switch, and no word of its help; decided for the Mac, it draws the switch.
#[gpui::test]
fn the_tunnel_switch_is_drawn_only_on_the_mac(cx: &mut TestAppContext) {
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    decide_tunnels_for(cx, Platform::Linux);
    click(cx, section_button(Section::Terminal));
    assert!(
        drawn(cx, super::terminal::ID),
        "the premise: Terminal is shown"
    );
    assert!(!drawn(cx, TUNNELS), "no switch on Linux");
    let text = painted(cx);
    assert!(!text.contains("tunnel"), "nor its words: {text}");

    on_the_mac(cx);
    click(cx, section_button(Section::Sessions));
    click(cx, section_button(Section::Terminal));
    assert!(drawn(cx, TUNNELS), "the switch on the Mac");
}

/// SPEC.md: tunnel consent "is off until they accept the warning", so the switch must never grant it on its own, and
/// with no optimistic copy it must not claim consent the helm has not stored. Specified: a click on the off switch
/// sends nothing and puts up the warning; Cancel sends nothing, leaves the switch off with no failure line and hands
/// focus back to what held it when the warning opened, here the switch, which the click focused under the test's Linux
/// click rule (on a Mac a click leaves a checkbox unfocused, so focus goes back to what held it before); a second click and Allow send `PUT /api/preferences` with only `{"tunnel_consent": true}`
/// under the preference row's key, and the switch stays off while that save is in flight; once the helm pushes the
/// stored consent and answers, the switch is on.
#[gpui::test]
fn the_tunnel_switch_turns_on_only_through_the_warning(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_terminal(cx, &Preferences::default());
    click(cx, TUNNELS);
    assert!(warning_up(cx), "the click asked the warning");
    assert_eq!(sent(&mut helm), [], "and sent nothing yet");

    click(cx, "prompt-cancel");
    assert!(!warning_up(cx), "Cancel closed the warning");
    assert_eq!(sent(&mut helm), [], "Cancel saved nothing");
    assert_eq!(switch_on(cx, TUNNELS), Some(false));
    assert_eq!(tunnel_failure(cx), None, "a declined warning says nothing");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(TUNNELS),
        "focus is back on the switch"
    );

    click(cx, TUNNELS);
    click(cx, "prompt-confirm");
    let (id, body) = one_save(&mut helm);
    assert_eq!(body, json!({ "tunnel_consent": true }));
    assert_eq!(preference_work(cx), Some(Work::Save));
    assert_eq!(
        switch_on(cx, TUNNELS),
        Some(false),
        "no optimistic copy while Allow's save is in flight"
    );

    push(&mut helm, &consent_row(true), cx);
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(
        switch_on(cx, TUNNELS),
        Some(true),
        "the stored consent shows"
    );
    assert_eq!(tunnel_failure(cx), None);
}

/// A refused consent save must not look like consent (SPEC.md, "Errors and diagnostics": a failed save is reported
/// beside its control), and the old switch showed the helm's own words. Specified: Allow's save refused leaves the
/// switch off with the helm's words under it as an alert; the next click, which asks again, takes the line away.
#[gpui::test]
fn a_refused_allow_leaves_the_switch_off_with_the_helms_words(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_terminal(cx, &Preferences::default());
    click(cx, TUNNELS);
    click(cx, "prompt-confirm");
    let (id, _) = one_save(&mut helm);
    helm.reply(id, 503, Some(json!("fixture consent refusal")));
    settle(cx);
    assert_eq!(switch_on(cx, TUNNELS), Some(false));
    assert_eq!(
        tunnel_failure(cx),
        Some((
            Some(Role::Alert),
            Some("fixture consent refusal".to_string())
        ))
    );
    assert!(
        bounds(cx, super::terminal::TUNNEL_ERROR_ID).top() >= bounds(cx, TUNNELS).bottom(),
        "the line is below the switch"
    );

    click(cx, TUNNELS);
    assert!(warning_up(cx), "the premise: the click asked again");
    assert_eq!(tunnel_failure(cx), None, "the new press took the line away");
}

/// SPEC.md: Settings "can turn it off, closing existing tunnels", with no warning; and the goal's rule for every saved
/// control, that a failed save restores the stored value with an adjacent error, so a refused withdrawal never shows
/// tunnels off while the helm still allows them. Specified: with consent stored, a click sends `PUT /api/preferences`
/// with only `{"tunnel_consent": false}` at once, with no warning, and the switch shows off while it is in flight;
/// refused, the switch shows on again with the helm's words under it; a second click whose save the helm stores shows
/// off and takes the line away.
#[gpui::test]
fn turning_tunnels_off_saves_at_once_and_a_refusal_restores_on(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_terminal(cx, &consent_row(true));
    assert_eq!(switch_on(cx, TUNNELS), Some(true), "the premise: on");
    click(cx, TUNNELS);
    assert!(!warning_up(cx), "no warning to turn tunnels off");
    let (id, body) = one_save(&mut helm);
    assert_eq!(body, json!({ "tunnel_consent": false }));
    assert_eq!(
        switch_on(cx, TUNNELS),
        Some(false),
        "the pending value shows"
    );

    helm.reply(id, 503, Some(json!("fixture withdrawal refused")));
    settle(cx);
    assert_eq!(
        switch_on(cx, TUNNELS),
        Some(true),
        "the stored value is back"
    );
    assert_eq!(
        tunnel_failure(cx),
        Some((
            Some(Role::Alert),
            Some("fixture withdrawal refused".to_string())
        ))
    );

    click(cx, TUNNELS);
    let (id, _) = one_save(&mut helm);
    assert_eq!(
        tunnel_failure(cx),
        None,
        "a started save took the line away"
    );
    push(&mut helm, &consent_row(false), cx);
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(switch_on(cx, TUNNELS), Some(false));
}

/// Allow's save goes under the preference row's key, which every Settings save holds, so a warning asked while another
/// save is in flight could only end in a refusal after the user accepted it. Specified: with another switch's save in
/// flight, a click on the tunnel switch puts up no warning, sends nothing and draws the busy line under the switch;
/// once that save is answered, a click asks the warning and the line is gone.
#[gpui::test]
fn a_tunnel_press_during_another_save_asks_nothing_and_says_why(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_terminal(cx, &Preferences::default());
    click(cx, BOTH);
    let (id, _) = one_save(&mut helm);
    click(cx, TUNNELS);
    assert!(!warning_up(cx), "no warning while the key is held");
    assert_eq!(sent(&mut helm), []);
    assert_eq!(busy_line(cx, TUNNELS), busy_alert());

    helm.reply(id, 204, None);
    settle(cx);
    click(cx, TUNNELS);
    assert!(warning_up(cx), "the key is free: the warning is up");
    assert_eq!(busy_line(cx, TUNNELS), None, "the press went through");
}

/// Turning tunnels off saves under the same key, so a press that meets another save must say why it sent nothing, as
/// any switch's does, and leave the consent shown as stored. Specified: with consent stored and a link choice's save in
/// flight, a click on the switch sends nothing, still shows it on and draws the busy line under it.
#[gpui::test]
fn turning_tunnels_off_during_another_save_sends_nothing_and_says_why(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_terminal(cx, &consent_row(true));
    click(cx, BOTH);
    let _ = one_save(&mut helm);
    click(cx, TUNNELS);
    assert_eq!(sent(&mut helm), [], "the key held the press back");
    assert_eq!(
        switch_on(cx, TUNNELS),
        Some(true),
        "the stored consent shows"
    );
    assert_eq!(busy_line(cx, TUNNELS), busy_alert());
}

/// The press checks the key before asking, but a save can still start while the warning is up, and Allow's own save is
/// then refused before it is sent: the user accepted the warning, so the switch must say that nothing was saved rather
/// than nothing at all. Specified: with the warning up and a save started on the preference row's key meanwhile, Allow
/// sends nothing and the busy line shows under the switch as an alert, the switch still off.
#[gpui::test]
fn an_allow_whose_save_meets_another_says_so_under_the_switch(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_terminal(cx, &Preferences::default());
    click(cx, TUNNELS);
    assert!(warning_up(cx), "the premise: the warning is up");
    let started = cx.update(|_, cx| {
        let operations = session::current_operations(cx).expect("the premise: a UI session");
        let command = super::terminal::tunnels_off_command().expect("the path builds");
        operations.update(cx, |operations, cx| {
            operations.start(&OpKey::preferences(), command, cx, |_, _| {})
        })
    });
    assert!(started.is_ok(), "the premise: another save holds the key");
    let _ = one_save(&mut helm);

    click(cx, "prompt-confirm");
    assert_eq!(sent(&mut helm), [], "Allow's save could not start");
    assert_eq!(switch_on(cx, TUNNELS), Some(false));
    assert_eq!(
        tunnel_failure(cx),
        Some((
            Some(Role::Alert),
            Some("Not saved: another operation was in progress. Try again.".to_string())
        ))
    );
}

// ---- Appearance ----------------------------------------------------------------------------------------------------

/// The Appearance option that shows as chosen (`System`, `Dark` or `Light`), or `None` when none does or the section
/// is not drawn.
fn appearance_chosen(cx: &mut VisualTestContext) -> Option<&'static str> {
    let nodes = frame(cx);
    ["System", "Dark", "Light"].into_iter().find(|label| {
        let id = format!("{}-{label}", super::appearance::ID);
        nodes.iter().any(|node| {
            node.element_id.as_deref() == Some(id.as_str()) && node.toggled == Some(Toggled::True)
        })
    })
}

/// The preference row storing `word` as its appearance.
fn appearance_row(word: &str) -> Preferences {
    Preferences {
        appearance: Some(word.to_owned()),
        ..Preferences::default()
    }
}

/// Appearance is a helm preference now (the maintainer's decision of 2026-11-13), saved like every Settings choice:
/// one field of the shared row, so a stale copy of another field is never written back, and nothing applied before the
/// helm holds it. Specified: with no appearance stored, Appearance shows System chosen; a click on Dark sends
/// `PUT /api/preferences` with only `{"appearance": "dark"}` under the preference row's key, and Dark shows while the
/// save is in flight; once the helm pushes the row and answers, Dark shows from the store and the key is free; a click
/// on System then sends the word `system`, not a cleared field.
#[gpui::test]
fn an_appearance_choice_saves_only_its_preference_field(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, "settings-section-appearance");
    assert_eq!(heading(cx).as_deref(), Some("Appearance"));
    assert_eq!(
        appearance_chosen(cx),
        Some("System"),
        "unset reads as System"
    );

    click(cx, "settings-appearance-Dark");
    let (id, body) = one_save(&mut helm);
    assert_eq!(
        body,
        json!({ "appearance": "dark" }),
        "only its field is sent"
    );
    assert_eq!(preference_work(cx), Some(Work::Save));
    assert_eq!(
        appearance_chosen(cx),
        Some("Dark"),
        "the pending value shows while the save is in flight"
    );

    push(&mut helm, &appearance_row("dark"), cx);
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(preference_work(cx), None, "the key is free again");
    assert_eq!(
        appearance_chosen(cx),
        Some("Dark"),
        "the stored value shows"
    );

    click(cx, "settings-appearance-System");
    let (_, body) = one_save(&mut helm);
    assert_eq!(body, json!({ "appearance": "system" }));
}

/// SPEC.md: "A failed save restores the last known value and shows an adjacent error", and with no optimistic copy a
/// refused choice never took effect, so showing it chosen would claim a palette the helm does not hold. Dark is stored
/// so the stored value differs from the default and from the dialog's own seed. Specified: with Dark stored, Settings
/// opens Appearance showing Dark; a click on Light shows Light while the save is in flight; the helm's refusal brings
/// Dark back with the failure line, an alert, drawn below the group; the next save that starts takes the line away.
#[gpui::test]
fn a_refused_appearance_save_shows_the_stored_choice_and_a_failure_line(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &appearance_row("dark"));
    open_settings(cx);
    click(cx, "settings-section-appearance");
    assert_eq!(
        appearance_chosen(cx),
        Some("Dark"),
        "the premise: the stored Dark shows"
    );
    click(cx, "settings-appearance-Light");
    let (id, _) = one_save(&mut helm);
    assert_eq!(
        appearance_chosen(cx),
        Some("Light"),
        "the premise: the pending value shows"
    );

    helm.reply(id, 503, Some(json!("fixture preference save refused")));
    settle(cx);
    assert_eq!(
        appearance_chosen(cx),
        Some("Dark"),
        "the stored value is back"
    );
    let failure = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(super::appearance::ERROR_ID))
        .map(|node| (node.role, node.label.map(|label| label.to_string())));
    assert_eq!(
        failure,
        Some((
            Some(Role::Alert),
            Some("Could not save the color theme. Try again.".to_string())
        ))
    );
    assert!(
        bounds(cx, super::appearance::ERROR_ID).top() >= bounds(cx, super::appearance::ID).bottom(),
        "the failure line is below the group"
    );

    click(cx, "settings-appearance-System");
    let _ = one_save(&mut helm);
    assert!(
        !drawn(cx, super::appearance::ERROR_ID),
        "a new save took the failure line away"
    );
}

/// A choice made outside this dialog (through the helm's API, say) reaches the open window as it happens, as every
/// shared preference does, and the group reads it from the store rather than a copy taken at opening. A copy would only
/// show on a push that comes with no save of the dialog's own. Specified: with Appearance open showing System, the helm
/// pushing `light` with nothing sent shows Light, and pushing an empty row shows System again.
#[gpui::test]
fn an_open_appearance_section_follows_a_choice_made_elsewhere(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, "settings-section-appearance");
    assert_eq!(appearance_chosen(cx), Some("System"), "the premise");
    push(&mut helm, &appearance_row("light"), cx);
    assert_eq!(appearance_chosen(cx), Some("Light"));
    assert_eq!(sent(&mut helm), [], "the dialog sent nothing of its own");
    push(&mut helm, &Preferences::default(), cx);
    assert_eq!(appearance_chosen(cx), Some("System"));
}

/// SPEC.md: System is the default, and the helm reads a word it does not know back as unset; the window must agree, or
/// a row written by a newer build would leave this one showing no choice at all. Specified: no row, an unset field and
/// an unknown word read as System; `dark` and `light` read as Dark and Light.
#[test]
fn an_unset_or_unknown_appearance_reads_as_system() {
    use crate::theme::Preference;
    let stored = super::appearance::stored;
    assert_eq!(stored(None), Preference::System, "no row");
    assert_eq!(
        stored(Some(&Preferences::default())),
        Preference::System,
        "unset"
    );
    assert_eq!(
        stored(Some(&appearance_row("sepia"))),
        Preference::System,
        "unknown"
    );
    assert_eq!(stored(Some(&appearance_row("dark"))), Preference::Dark);
    assert_eq!(stored(Some(&appearance_row("light"))), Preference::Light);
}

// ---- The stored appearance on the window ---------------------------------------------------------------------------

/// The main window over the fake helm, before any snapshot: the window the app opens while it connects. The test
/// platform reports a light operating system.
fn open_main(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    let (helm, _connection) = started(cx);
    let (_, window) = test_support::open_view(cx, MainView::new);
    (helm, window)
}

/// Whether the window paints with the dark palette now.
fn dark(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| theme::palette(cx).dark)
}

/// SPEC.md: the choice is a helm preference "and a change applies live in the open window". The main view's
/// subscription hands the stored row to the theme; a palette that followed only this window's own saves would leave the
/// first snapshot and a reconnect's new snapshot unpainted. It also records what the window shows before the first
/// snapshot, which is a deviation from the old app (the parity file): the connecting screen is in the operating
/// system's palette. Specified: on a light system, the connecting window's palette is light; the first snapshot storing
/// Dark turns it dark; a push storing Light, with nothing sent, turns it light; a push storing nothing (System) after a
/// stored Dark leaves the system's light; after a reset the inert window keeps the palette in use (Dark), and the
/// reconnect's snapshot storing Light turns it light.
#[gpui::test]
fn the_stored_appearance_paints_the_window_from_the_first_snapshot_on(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_main(cx);
    assert!(
        !dark(cx),
        "before the snapshot the palette is the system's light"
    );
    helm.connect(vec![preferences_state(&appearance_row("dark"))]);
    settle(cx);
    assert!(dark(cx), "the first snapshot's Dark applies");
    push(&mut helm, &appearance_row("light"), cx);
    assert!(!dark(cx), "a pushed Light applies");
    push(&mut helm, &appearance_row("dark"), cx);
    assert!(dark(cx), "the premise: a pushed Dark applies");
    push(&mut helm, &Preferences::default(), cx);
    assert!(!dark(cx), "unset is System, which follows the light system");
    assert_eq!(sent(&mut helm), [], "applying a choice saves nothing");

    // A reset retires the UI session and its store; the reconnect's snapshot is a new store, which the subscription
    // must pick up by itself.
    push(&mut helm, &appearance_row("dark"), cx);
    assert!(dark(cx), "the premise: Dark is in use before the reset");
    helm.break_channel();
    settle(cx);
    assert!(
        cx.update(|_, cx| session::current_store(cx).is_none()),
        "the premise: the reset left the window inert"
    );
    assert!(dark(cx), "the inert window keeps the palette in use");
    helm.connect(vec![preferences_state(&appearance_row("light"))]);
    settle(cx);
    assert!(!dark(cx), "the reconnect's snapshot applies");
}

/// SPEC.md: "System follows each device's current appearance, including changes while Farhelm stays open; an explicit
/// choice overrides it." Specified: with System stored, the operating system turning dark turns the window dark and
/// turning light again turns it light; with Light stored, the system turning dark leaves the window light.
#[gpui::test]
fn system_follows_the_operating_system_and_an_explicit_choice_does_not(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_main(cx);
    helm.connect(vec![preferences_state(&Preferences::default())]);
    settle(cx);
    cx.update(|_, cx| theme::follow_system(WindowAppearance::Dark, cx));
    assert!(dark(cx), "System follows the system to dark");
    cx.update(|_, cx| theme::follow_system(WindowAppearance::Light, cx));
    assert!(!dark(cx), "and back to light");
    push(&mut helm, &appearance_row("light"), cx);
    cx.update(|_, cx| theme::follow_system(WindowAppearance::Dark, cx));
    assert!(!dark(cx), "an explicit Light ignores the system");
}

/// The agent build's `--appearance` start hook is a test hook for screenshot batches: a batch's dark captures must be
/// dark whatever the helm stores, so the stored choice must not replace it. Specified: with the hook forcing Dark, a
/// snapshot storing Light and a later push storing System leave the window dark.
///
/// The app is set up by hand rather than through [`open_main`]: in an agent build, [`crate::bootstrap::install`]
/// replaces the overrides with the ones read from the environment, which would drop the test's own. This installs the
/// overrides and then runs what a launch runs after reading them ([`crate::bootstrap::install_app_state`]), as the
/// start hook's own test does, so it holds whatever features the crate is built with.
#[gpui::test]
fn the_agent_builds_start_hook_wins_over_the_stored_appearance(cx: &mut TestAppContext) {
    cx.update(|cx| {
        cx.set_global(crate::agent_startup::Overrides {
            preference: Some(theme::Preference::Dark),
            ..Default::default()
        });
        crate::bootstrap::install_app_state(cx).unwrap();
    });
    let (mut helm, handle, events) = FakeHelm::new();
    let _connection = cx.update(|cx| session::start(handle, events, None, cx));
    cx.run_until_parked();
    let (_, cx) = test_support::open_view(cx, MainView::new);
    assert!(dark(cx), "the premise: the hook started the app dark");
    helm.connect(vec![preferences_state(&appearance_row("light"))]);
    settle(cx);
    assert!(dark(cx), "the snapshot's Light did not replace the hook");
    push(&mut helm, &Preferences::default(), cx);
    assert!(dark(cx));
}

// ---- Agent trust ---------------------------------------------------------------------------------------------------

/// The reads the window sent since the last call, as (id, path), all GETs.
pub(super) fn sent_reads(helm: &mut FakeHelm) -> Vec<(u64, String)> {
    sent(helm)
        .into_iter()
        .map(|command| {
            assert_eq!(command.method, Method::Get, "only reads: {command:?}");
            (command.id, command.path)
        })
        .collect()
}

/// The independent trust routes, indexed by the agent's stable trust slot.
const TRUST_PATHS: [&str; 5] = [
    "/api/checkout-claude-trust",
    "/api/checkout-agent-trust/codex",
    "/api/checkout-agent-trust/muse",
    "/api/checkout-agent-trust/pi",
    "/api/checkout-agent-trust/antigravity",
];

/// A help paragraph has text that a platform adapter can read as a label.
fn accessible_help_contains(cx: &mut VisualTestContext, expected: &str) -> bool {
    frame(cx).iter().any(|node| {
        node.element_id.is_some()
            && node.role == Some(Role::Label)
            && node
                .value
                .as_deref()
                .is_some_and(|text| text.contains(expected))
    })
}

/// Answer one trust read and, on Antigravity's page, the separate hook setup read. Neither changes the hint preference.
fn answer_trust(
    helm: &mut FakeHelm,
    reads: &[(u64, String)],
    enabled: bool,
    cx: &mut VisualTestContext,
) {
    let antigravity = reads[0].1 == TRUST_PATHS[4];
    assert_eq!(reads.len(), if antigravity { 2 } else { 1 });
    helm.reply(
        reads[0].0,
        200,
        Some(json!({ "enabled": enabled, "hint_dismissed": false })),
    );
    if antigravity {
        assert_eq!(reads[1].1, "/api/agent-hook-setup/antigravity");
        helm.reply(reads[1].0, 200, Some(json!({ "enabled": false })));
    }
    settle(cx);
}

/// Open Claude's trust page and return its read. The Sessions default sends no commands.
fn show_agent_trust(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<(u64, String)> {
    open_settings(cx);
    assert_eq!(sent(helm), [], "the premise: Sessions reads nothing");
    click_section(cx, Section::Agent(LaunchHarness::Claude));
    assert_eq!(heading(cx).as_deref(), Some("Claude Code"));
    sent_reads(helm)
}

/// Each agent page reads only its own choice on the first visit. A pending read keeps its switch unavailable. Claude's
/// answer stays in Claude's page while visiting Codex, and an agent with no settings has a discoverable empty page.
#[gpui::test]
fn agent_trust_reads_each_agent_once_when_shown_and_waits_for_the_answer(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let reads = show_agent_trust(&mut helm, cx);
    let paths: Vec<&str> = reads.iter().map(|(_, path)| path.as_str()).collect();
    assert_eq!(paths, [TRUST_PATHS[0]]);
    click(cx, TrustAgent::Claude.id());
    assert_eq!(sent(&mut helm), [], "an unknown choice takes no toggle");

    answer_trust(&mut helm, &reads, true, cx);
    assert_eq!(switch_on(cx, TrustAgent::Claude.id()), Some(true));
    assert!(accessible_help_contains(
        cx,
        "can rarely undo Farhelm's edit"
    ));
    assert!(!drawn(cx, TrustAgent::Codex.id()));

    click_section(cx, Section::Agent(LaunchHarness::Codex));
    let reads = sent_reads(&mut helm);
    assert_eq!(
        reads
            .iter()
            .map(|(_, path)| path.as_str())
            .collect::<Vec<_>>(),
        [TRUST_PATHS[1]]
    );
    answer_trust(&mut helm, &reads, false, cx);
    assert_eq!(switch_on(cx, TrustAgent::Codex.id()), Some(false));
    assert!(!drawn(cx, TrustAgent::Claude.id()));

    click_section(cx, Section::Agent(LaunchHarness::Cursor));
    assert_eq!(heading(cx).as_deref(), Some("Cursor"));
    assert!(accessible_help_contains(cx, super::NO_AGENT_SETTINGS));
    assert_eq!(sent(&mut helm), [], "an empty page reads nothing");

    click_section(cx, Section::Agent(LaunchHarness::Claude));
    assert_eq!(sent(&mut helm), [], "read once per opening");
    assert_eq!(switch_on(cx, TrustAgent::Claude.id()), Some(true));
}

/// An unknown setup switch cannot authorize a write. After reading Grok's choice, the switch writes to Grok alone,
/// and its separate hint switch can restore the launcher question and the session hint together.
#[gpui::test]
fn grok_hook_setup_and_its_hint_are_independent_settings(cx: &mut TestAppContext) {
    let (mut helm, cx, gear) = open_rail(
        cx,
        &Preferences {
            grok_hook_hint_dismissed: Some(true),
            ..Preferences::default()
        },
    );
    let agent = crate::hook_setup::HookAgent::Grok;
    cx.update(|window, cx| {
        super::open_at_hook_setup(agent, &gear, &Mark::take(cx), window, cx);
    });
    settle(cx);
    assert_eq!(heading(cx).as_deref(), Some("Grok"));
    assert_eq!(available(cx, agent.id()), Some(false));
    let reads = sent_reads(&mut helm);
    let [(read, path)] = reads.as_slice() else {
        panic!("only the Grok setup read: {reads:?}")
    };
    assert_eq!(path, "/api/agent-hook-setup/grok");
    click(cx, agent.id());
    assert!(sent(&mut helm).is_empty(), "unknown consent is unavailable");
    helm.reply(*read, 200, Some(json!({ "enabled": false })));
    settle(cx);
    assert_eq!(switch_on(cx, agent.id()), Some(false));
    assert_eq!(switch_on(cx, agent.dismissal_id()), Some(false));
    assert!(accessible_help_contains(cx, agent.removal()));
    click(cx, agent.id());
    let writes = helm.commands();
    let [write] = writes.as_slice() else {
        panic!("one setting write: {writes:?}")
    };
    assert_eq!(write.method, farhelm_helm_client::Method::Put);
    assert_eq!(write.path, "/api/agent-hook-setup/grok");
    assert_eq!(write.body, Some(json!({ "enabled": true })));
    helm.reply(write.id, 204, None);
    settle(cx);
    assert_eq!(switch_on(cx, agent.id()), Some(true));
    click(cx, agent.dismissal_id());
    let (_, body) = one_save(&mut helm);
    assert_eq!(body, json!({ "grok_hook_hint_dismissed": false }));
}

/// Open through the trust hint and answer the page's one read. Each caller begins with that agent's switch focused.
fn open_trust_at(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    gear: &FocusHandle,
    agent: TrustAgent,
) {
    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        super::open_at_agent_trust(agent, gear, &mark, window, cx);
    });
    settle(cx);
    let reads = sent_reads(helm);
    assert_eq!(
        reads.first().map(|(_, path)| path.as_str()),
        Some(TRUST_PATHS[agent.index()])
    );
    answer_trust(helm, &reads, false, cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(agent.id()),
        "the premise: {agent:?}'s switch has focus"
    );
}

/// The failure line drawn under `agent`'s block, by its accessible name.
fn trust_failure(cx: &mut VisualTestContext, agent: TrustAgent) -> Option<String> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(agent.error_id()))
        .and_then(|node| node.label.map(|label| label.to_string()))
}

/// A trust switch saves at once through its agent's own route, and the helm's done reply makes it the choice shown;
/// a refusal restores the choice read before with the helm's reason beside it (SPEC.md, "Errors and diagnostics").
/// Specified: Codex's switch sends `PUT /api/checkout-agent-trust/codex` with `{"enabled": true}` under the preference
/// row's key and shows on while it is in flight, then stays on once the helm answers. In a later opening Antigravity
/// sends `{"enabled": true}` to its route and a refusal restores off with "Could not save or read Antigravity trust settings:" and
/// the helm's words.
#[gpui::test]
fn a_trust_switch_saves_through_its_route_and_a_refusal_restores_it(cx: &mut TestAppContext) {
    let (mut helm, cx, gear) = open_rail(cx, &Preferences::default());
    open_trust_at(&mut helm, cx, &gear, TrustAgent::Codex);
    keys(cx, "space");
    let saves = sent(&mut helm);
    assert_eq!(saves.len(), 1, "one save: {saves:?}");
    assert_eq!(
        (
            saves[0].method,
            saves[0].path.as_str(),
            saves[0].body.clone()
        ),
        (
            Method::Put,
            TRUST_PATHS[1],
            Some(json!({ "enabled": true }))
        )
    );
    assert_eq!(preference_work(cx), Some(Work::Save));
    assert_eq!(switch_on(cx, TrustAgent::Codex.id()), Some(true), "pending");
    helm.reply(saves[0].id, 204, None);
    settle(cx);
    assert_eq!(switch_on(cx, TrustAgent::Codex.id()), Some(true), "saved");

    keys(cx, "escape");
    open_trust_at(&mut helm, cx, &gear, TrustAgent::Antigravity);
    assert!(accessible_help_contains(
        cx,
        "repository hooks, settings and commands are then trusted"
    ));
    assert!(accessible_help_contains(
        cx,
        "replaces Antigravity's saved list of trusted folders"
    ));
    keys(cx, "space");
    let saves = sent(&mut helm);
    assert_eq!(
        (saves[0].path.as_str(), saves[0].body.clone()),
        (TRUST_PATHS[4], Some(json!({ "enabled": true })))
    );
    helm.reply(saves[0].id, 503, Some(json!("fixture trust save refused")));
    settle(cx);
    assert_eq!(
        switch_on(cx, TrustAgent::Antigravity.id()),
        Some(false),
        "restored"
    );
    assert_eq!(
        trust_failure(cx, TrustAgent::Antigravity).as_deref(),
        Some("Could not save or read Antigravity trust settings: fixture trust save refused")
    );
}

/// Claude's failed read keeps its switch unavailable and explains the refusal. Codex's independent read still
/// supplies a usable switch when its page is opened.
#[gpui::test]
fn a_failed_trust_read_leaves_the_switch_unavailable_with_its_reason(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let reads = show_agent_trust(&mut helm, cx);
    helm.reply(reads[0].0, 503, Some(json!("fixture trust read refused")));
    settle(cx);
    click(cx, TrustAgent::Claude.id());
    assert_eq!(sent(&mut helm), [], "no toggle over an unknown choice");
    assert_eq!(
        trust_failure(cx, TrustAgent::Claude).as_deref(),
        Some("Could not save or read Claude trust settings: fixture trust read refused")
    );
    click_section(cx, Section::Agent(LaunchHarness::Codex));
    let reads = sent_reads(&mut helm);
    answer_trust(&mut helm, &reads, true, cx);
    assert_eq!(switch_on(cx, TrustAgent::Codex.id()), Some(true));
}

/// A read answers only the opening it was sent for: a slow reply from an earlier opening must not fill in a newer one,
/// which has its own read in flight. Each opening is a dialog of its own, so this guards against trust state shared
/// between openings. Close Settings while Claude's read is out, reopen the page, then answer in order. The closed
/// page's reply leaves the new switch unavailable until its own reply arrives.
#[gpui::test]
fn a_trust_read_reaches_only_the_opening_it_was_sent_for(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let first = show_agent_trust(&mut helm, cx);
    keys(cx, "escape");
    let second = show_agent_trust(&mut helm, cx);
    assert_eq!(second.len(), 1, "the new opening reads again");
    answer_trust(&mut helm, &first, true, cx);
    assert_eq!(
        switch_on(cx, TrustAgent::Claude.id()),
        Some(false),
        "the old answer reached nobody"
    );
    click(cx, TrustAgent::Claude.id());
    assert_eq!(sent(&mut helm), [], "and the switch is still unknown");
    answer_trust(&mut helm, &second, true, cx);
    assert_eq!(switch_on(cx, TrustAgent::Claude.id()), Some(true));
}

/// SPEC.md: a hint's Don't show again "stores a shared dismissal, reversible beside the switch". The hint switches are
/// Each agent's hint switch uses the same pushed dismissal as its band. Codex's switch starts on and a toggle writes
/// just Codex's dismissal. Antigravity's switch on its own page follows an independent dismissal pushed later.
#[gpui::test]
fn the_hint_switches_are_the_shared_dismissals_inverted(cx: &mut TestAppContext) {
    let (mut helm, cx, gear) = open_rail(cx, &Preferences::default());
    open_trust_at(&mut helm, cx, &gear, TrustAgent::Codex);
    assert_eq!(switch_on(cx, Choice::ShowCodexTrustHint.id()), Some(true));
    assert!(!drawn(cx, Choice::ShowClaudeTrustHint.id()));
    keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(Choice::ShowCodexTrustHint.id())
    );
    keys(cx, "space");
    let (id, body) = one_save(&mut helm);
    assert_eq!(body, json!({ "codex_trust_hint_dismissed": true }));
    helm.reply(id, 204, None);
    settle(cx);
    click_section(cx, Section::Agent(LaunchHarness::Antigravity));
    let reads = sent_reads(&mut helm);
    answer_trust(&mut helm, &reads, false, cx);
    assert_eq!(
        switch_on(cx, Choice::ShowAntigravityTrustHint.id()),
        Some(true)
    );
    push(
        &mut helm,
        &Preferences {
            antigravity_trust_hint_dismissed: Some(true),
            ..Preferences::default()
        },
        cx,
    );
    assert_eq!(
        switch_on(cx, Choice::ShowAntigravityTrustHint.id()),
        Some(false)
    );
}

/// The trust hint opens Antigravity's own page and waits for its switch to become available before taking focus.
/// The unread switch must not report a saved off value. Escape returns focus to the opening button, even when it
/// did not have focus when Settings opened.
#[gpui::test]
fn the_trust_hint_entry_point_focuses_its_agents_switch_once_known(cx: &mut TestAppContext) {
    let (mut helm, cx, gear) = open_rail(cx, &Preferences::default());
    assert!(
        !focused(cx, &gear),
        "the premise: the opening button does not hold focus"
    );
    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        super::open_at_agent_trust(TrustAgent::Antigravity, &gear, &mark, window, cx);
    });
    settle(cx);
    assert_eq!(heading(cx).as_deref(), Some("Antigravity"));
    assert!(
        dialog_holds_focus(cx),
        "the dialog itself holds focus while the reads are out"
    );
    assert_eq!(available(cx, TrustAgent::Antigravity.id()), Some(false));
    let switch = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(TrustAgent::Antigravity.id()))
        .expect("the unread trust switch is drawn");
    assert_eq!((switch.role, switch.toggled), (Some(Role::Label), None));
    let unavailable = format!("{}. Unavailable.", TrustAgent::Antigravity.label());
    assert_eq!(switch.label.as_deref(), Some(unavailable.as_str()));
    assert_eq!(switch.value.as_deref(), Some(unavailable.as_str()));
    let reads = sent_reads(&mut helm);
    assert_eq!(
        reads.first().map(|(_, path)| path.as_str()),
        Some(TRUST_PATHS[4])
    );
    answer_trust(&mut helm, &reads, false, cx);
    let switch = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(TrustAgent::Antigravity.id()))
        .expect("the trust read kept the switch visible");
    assert_eq!(
        (switch.role, switch.toggled),
        (Some(Role::Switch), Some(Toggled::False))
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(TrustAgent::Antigravity.id())
    );
    assert!(
        shows_whole(cx, TrustAgent::Antigravity.id(), DIALOG_BODY),
        "the move scrolled Antigravity's switch into view: {:?} in {:?}",
        bounds(cx, TrustAgent::Antigravity.id()),
        bounds(cx, DIALOG_BODY)
    );
    keys(cx, "escape");
    assert!(!any_open(cx), "the premise: Escape closed Settings");
    assert!(
        focused(cx, &gear),
        "closing returned focus to the opening button, which nothing focused before Settings opened"
    );
}

/// Whether the control drawn under `id` (a switch, a button) is available, which an unavailable one shows by taking
/// no focus; `None` when it is not drawn.
pub(super) fn available(cx: &mut VisualTestContext, id: &str) -> Option<bool> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| node.flags.focusable)
}

/// Whether the topmost dialog itself (its container, not a control in it) holds focus.
fn dialog_holds_focus(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| {
        layers::top_container(window, cx).is_some_and(|container| container.is_focused(window))
    })
}

/// SPEC.md, "Keyboard focus": a result that arrives after the user moved on moves no focus. The handover waits on a
/// read, and the user may tab away first; taking focus back then would yank it from where they put it. Specified:
/// opened for Codex, with the dialog itself holding focus while the reads are out, a Tab moves focus into the dialog;
/// once the reads answer, focus stays where the Tab put it and Codex's switch does not take it.
#[gpui::test]
fn a_trust_read_that_answers_after_the_user_moved_on_moves_no_focus(cx: &mut TestAppContext) {
    let (mut helm, cx, gear) = open_rail(cx, &Preferences::default());
    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        super::open_at_agent_trust(TrustAgent::Codex, &gear, &mark, window, cx);
    });
    settle(cx);
    assert!(
        dialog_holds_focus(cx),
        "the premise: the dialog waits for the read"
    );
    let reads = sent_reads(&mut helm);
    keys(cx, "tab");
    let moved_to = focused_id(cx);
    assert!(
        moved_to.is_some() && !dialog_holds_focus(cx),
        "the premise: the Tab moved focus to a control: {moved_to:?}"
    );
    answer_trust(&mut helm, &reads, false, cx);
    assert_eq!(
        available(cx, TrustAgent::Codex.id()),
        Some(true),
        "the premise: the reads answered and Codex's switch is drawn available"
    );
    assert_eq!(
        focused_id(cx),
        moved_to,
        "focus stays where the user put it"
    );
    assert_ne!(focused_id(cx).as_deref(), Some(TrustAgent::Codex.id()));
}

/// SPEC.md, "Keyboard focus", counts any click inside Farhelm as moving on, also one that leaves focus where it was:
/// Settings keeps focus on the dialog when the user clicks its empty space or its words. Only the mark taken at the
/// hint's click tells that click apart from no input at all, so this pins the handover's mark check (the Tab test
/// would still pass without it, since a Tab also takes focus off the dialog). Specified: opened for Codex, a click on
/// the section's heading leaves the dialog itself holding focus; once the reads answer, the dialog still holds focus
/// and Codex's switch does not take it.
#[gpui::test]
fn a_click_that_leaves_focus_on_the_dialog_still_stops_the_trust_handover(cx: &mut TestAppContext) {
    let (mut helm, cx, gear) = open_rail(cx, &Preferences::default());
    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        super::open_at_agent_trust(TrustAgent::Codex, &gear, &mark, window, cx);
    });
    settle(cx);
    let reads = sent_reads(&mut helm);
    click(cx, "settings-heading");
    assert!(
        dialog_holds_focus(cx),
        "the premise: the click left the dialog itself holding focus"
    );
    answer_trust(&mut helm, &reads, false, cx);
    assert_eq!(
        available(cx, TrustAgent::Codex.id()),
        Some(true),
        "the premise: the reads answered and Codex's switch is drawn available"
    );
    assert!(dialog_holds_focus(cx), "focus stayed on the dialog");
    assert_ne!(focused_id(cx).as_deref(), Some(TrustAgent::Codex.id()));
}

/// The dialog keeps each agent's state at the agent's number (`TrustAgent::index`), and draws the section in
/// `TrustAgent::ALL`'s order; the two are written separately, so a reordering of one would draw an agent with another's
/// state. Specified: each agent in `ALL` sits at its own number.
#[test]
fn trust_agents_are_listed_at_their_own_numbers() {
    for (place, agent) in TrustAgent::ALL.iter().enumerate() {
        assert_eq!(agent.index(), place, "{agent:?}");
    }
}

// ---- Checkouts -----------------------------------------------------------------------------------------------------

/// Which of Checkouts' two text fields.
#[derive(Clone, Copy)]
enum Field {
    Folder,
    PostClone,
}

impl Field {
    /// The field's selector, which a click focuses it through.
    fn selector(self) -> &'static str {
        match self {
            Field::Folder => "checkout-folder",
            Field::PostClone => "checkout-post-clone",
        }
    }

    fn path(self) -> &'static str {
        match self {
            Field::Folder => "/api/checkout-root",
            Field::PostClone => "/api/checkout-post-clone",
        }
    }
}

/// The open dialog's Checkouts field `field`.
fn entity(cx: &mut VisualTestContext, field: Field) -> Entity<super::CheckoutField> {
    cx.update(|_, cx| {
        let dialog = layers::last_opened::<super::SettingsDialog>(cx);
        let dialog = dialog.expect("the premise: Settings is open");
        let fields = dialog.read(cx).checkout_fields.as_ref();
        let fields = fields.expect("the premise: Checkouts was shown");
        match field {
            Field::Folder => fields.folder.clone(),
            Field::PostClone => fields.post_clone.clone(),
        }
    })
}

/// The text in the open dialog's Checkouts field.
fn text(cx: &mut VisualTestContext, field: Field) -> String {
    let entity = entity(cx, field);
    cx.update(|_, cx| entity.read(cx).text(cx))
}

/// Click `field`, check that the click put focus in it (so the keys that follow reach it, whether or not it takes
/// them), and type `typed` into it.
fn type_into(cx: &mut VisualTestContext, field: Field, typed: &str) {
    click(cx, field.selector());
    let entity = entity(cx, field);
    let focused = cx.update(|window, cx| entity.read(cx).focused(window, cx));
    assert!(focused, "the premise: the click put focus in the field");
    cx.simulate_input(typed);
    test_support::check(cx);
}

/// The reads Checkouts sends when first shown, by route: the folder's, the command's and the spare switch's.
const CHECKOUT_READS: [&str; 3] = [
    "/api/checkout-root",
    "/api/checkout-post-clone",
    "/api/checkout-spare",
];

/// Open Settings, show Checkouts and answer its three reads with `root`, `command` and `spare`.
pub(super) fn show_checkouts(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    root: Value,
    command: Value,
    spare: bool,
) {
    open_settings(cx);
    click(cx, section_button(Section::Checkouts));
    let reads = sent_reads(helm);
    let paths: Vec<&str> = reads.iter().map(|(_, path)| path.as_str()).collect();
    assert_eq!(
        paths, CHECKOUT_READS,
        "the premise: Checkouts read its routes"
    );
    helm.reply(reads[0].0, 200, Some(json!({ "root": root })));
    helm.reply(reads[1].0, 200, Some(json!({ "command": command })));
    helm.reply(reads[2].0, 200, Some(json!({ "enabled": spare })));
    settle(cx);
}

/// The one command sent since the last call, which must be a `PUT` of `path`: its id and body.
fn one_put(helm: &mut FakeHelm, path: &str) -> (u64, Value) {
    let sent = sent(helm);
    assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
    assert_eq!((sent[0].method, sent[0].path.as_str()), (Method::Put, path));
    (sent[0].id, sent[0].body.clone().unwrap_or_default())
}

/// The accessible name of what is drawn under `id`, if anything is.
pub(super) fn label_of(cx: &mut VisualTestContext, id: &str) -> Option<String> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .and_then(|node| node.label.map(|label| label.to_string()))
}

/// These routes are helm configuration the helm does not push, so the section reads them once when it is first shown.
/// Until a read answers, the empty field must not be taken for the stored value: a Save then would clear the folder or
/// command (the old "stays locked until its initial read completes"), and the switch would claim a choice nobody read.
/// An untouched Save must write nothing, or it would put the value read at opening back over a change the command line
/// made since. Specified: Sessions reads nothing; showing Checkouts sends one GET each to the folder, command and spare
/// routes; before they answer, typing changes neither field and Save, Use default's absence aside, and the switch send
/// nothing; once answered, the fields show the stored folder and the explicitly disabled command as empty, the switch
/// shows off, an untouched Save sends nothing, and showing Hosts and Checkouts again reads nothing more.
#[gpui::test]
fn checkouts_reads_its_routes_once_and_stays_locked_until_they_answer(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    assert_eq!(sent(&mut helm), [], "the premise: Sessions reads nothing");
    click(cx, section_button(Section::Checkouts));
    assert_eq!(heading(cx).as_deref(), Some("Checkouts"));
    let reads = sent_reads(&mut helm);
    let paths: Vec<&str> = reads.iter().map(|(_, path)| path.as_str()).collect();
    assert_eq!(paths, CHECKOUT_READS);

    type_into(cx, Field::Folder, "typed early");
    type_into(cx, Field::PostClone, "typed early");
    assert_eq!(text(cx, Field::Folder), "", "read-only until read");
    assert_eq!(text(cx, Field::PostClone), "", "read-only until read");
    assert!(!drawn(cx, FOOTER_SAVE));
    click(cx, super::checkouts::SPARE_ID);
    assert_eq!(sent(&mut helm), [], "nothing saves before the reads answer");

    helm.reply(reads[0].0, 200, Some(json!({ "root": "~/stored" })));
    helm.reply(reads[1].0, 200, Some(json!({ "command": "" })));
    helm.reply(reads[2].0, 200, Some(json!({ "enabled": false })));
    settle(cx);
    assert_eq!(text(cx, Field::Folder), "~/stored");
    assert_eq!(
        text(cx, Field::PostClone),
        "",
        "a disabled command shows empty"
    );
    assert_eq!(switch_on(cx, super::checkouts::SPARE_ID), Some(false));
    assert!(!drawn(cx, FOOTER_SAVE), "unchanged fields have no Save");
    click(cx, section_button(Section::Hosts));
    click(cx, section_button(Section::Checkouts));
    assert_eq!(sent(&mut helm), [], "read once per opening");
}

/// SPEC.md: "Settings saves each field explicitly ... and clears it when an all-whitespace field is saved"; the old
/// field also trimmed the folder it sent, and read the route again after a save so the field showed what the helm
/// stored rather than what was typed. The save is work under the preference row's key like every Settings save.
/// Specified: typing "  ~/edited  " and Save sends `PUT /api/checkout-root` with `{"root": "~/edited"}` while the
/// preference row's key holds a save, Save reads "Saving…" and the field takes no typing; the helm's done reply sends
/// one GET of the route and shows "Checkout folder saved", and its answer shows "~/edited"; typing hides the saved
/// line; emptying the field and Save sends `{"root": null}`.
#[gpui::test]
fn the_folder_saves_an_edited_draft_trimmed_and_reads_it_back(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_checkouts(&mut helm, cx, Value::Null, Value::Null, true);
    type_into(cx, Field::Folder, "  ~/edited  ");
    click(cx, FOOTER_SAVE);
    let (id, body) = one_put(&mut helm, Field::Folder.path());
    assert_eq!(body, json!({ "root": "~/edited" }));
    assert_eq!(preference_work(cx), Some(Work::Save));
    assert!(painted(cx).contains("Saving…"));
    // Typed into the field itself (Save holds focus after its click). The field is read-only, and the draft would put
    // back an edit that got in anyway, so this pins the visible rule, not which of the two guards held.
    type_into(cx, Field::Folder, "x");
    assert_eq!(
        text(cx, Field::Folder),
        "  ~/edited  ",
        "read-only while saving"
    );

    helm.reply(id, 204, None);
    settle(cx);
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1, "one read back: {reads:?}");
    assert_eq!(reads[0].1, Field::Folder.path());
    assert!(drawn(cx, "checkout-folder-saved"));
    helm.reply(reads[0].0, 200, Some(json!({ "root": "~/edited" })));
    settle(cx);
    assert_eq!(text(cx, Field::Folder), "~/edited");
    assert_eq!(
        label_of(cx, "checkout-folder-saved").as_deref(),
        Some("Checkout folder saved")
    );

    type_into(cx, Field::Folder, "/x");
    assert!(
        !drawn(cx, "checkout-folder-saved"),
        "an edit hides the line"
    );
    let erase = vec!["backspace"; text(cx, Field::Folder).chars().count()].join(" ");
    keys(cx, &erase);
    assert_eq!(
        text(cx, Field::Folder),
        "",
        "the premise: the field is empty"
    );
    click(cx, FOOTER_SAVE);
    let (_, body) = one_put(&mut helm, Field::Folder.path());
    assert_eq!(body, json!({ "root": null }), "an empty folder clears it");
}

/// Validation failures discard the edit just like a conflict. The alert must survive a fresh read and a section
/// switch, and an untouched Save must never replay the rejected path.
#[gpui::test]
fn a_refused_folder_discards_its_draft_and_keeps_the_reason_across_sections(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_checkouts(&mut helm, cx, Value::Null, Value::Null, true);
    type_into(cx, Field::Folder, "relative-folder");
    click(cx, FOOTER_SAVE);
    let (id, _) = one_put(&mut helm, Field::Folder.path());
    let reason = "fixture: not the relative path 'relative-folder'";
    helm.reply(id, 400, Some(json!(reason)));
    settle(cx);
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].1, Field::Folder.path());
    assert_eq!(
        text(cx, Field::Folder),
        "",
        "the rejected path was discarded"
    );
    helm.reply(reads[0].0, 200, Some(json!({ "root": "~/current" })));
    settle(cx);
    for _ in 0..2 {
        assert_eq!(text(cx, Field::Folder), "~/current");
        assert_eq!(
            label_of(cx, "checkout-folder-error").as_deref(),
            Some(reason)
        );
        click(cx, section_button(Section::Sessions));
        click(cx, section_button(Section::Checkouts));
    }
    assert!(!drawn(cx, FOOTER_SAVE));
    assert_eq!(
        sent(&mut helm),
        [],
        "the stored value is an untouched draft"
    );
}

/// A rejected command is never replayed while waiting for the new base, even if that read fails. An explicit
/// disable looks empty, so the first PUT proves the editor sends the raw stored value as its comparison base.
#[gpui::test]
fn a_stale_post_clone_save_waits_for_a_fresh_base_before_editing_again(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, section_button(Section::Checkouts));
    let reads = sent_reads(&mut helm);
    assert_eq!(
        reads
            .iter()
            .map(|(_, path)| path.as_str())
            .collect::<Vec<_>>(),
        CHECKOUT_READS
    );
    helm.reply(
        reads[0].0,
        200,
        Some(json!({ "root": null, "compare_on_save": true })),
    );
    helm.reply(
        reads[1].0,
        200,
        Some(json!({ "command": "", "compare_on_save": true })),
    );
    helm.reply(reads[2].0, 200, Some(json!({ "enabled": true })));
    settle(cx);
    assert_eq!(
        text(cx, Field::PostClone),
        "",
        "explicit disable shows empty"
    );
    type_into(cx, Field::PostClone, "original edit");
    click(cx, FOOTER_SAVE);
    let (id, body) = one_put(&mut helm, Field::PostClone.path());
    assert_eq!(body, json!({ "command": "original edit", "expected": "" }));
    let reason = "This checkout setting changed elsewhere.";
    helm.reply(id, 409, Some(json!(reason)));
    settle(cx);
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].1, Field::PostClone.path());
    type_into(cx, Field::PostClone, "typed before the reply");
    assert_eq!(text(cx, Field::PostClone), "", "discarded and read-only");
    assert!(!drawn(cx, FOOTER_SAVE));
    assert_eq!(sent(&mut helm), []);

    helm.reply(reads[0].0, 503, Some(json!("fixture: read failed")));
    settle(cx);
    assert_eq!(
        label_of(cx, "checkout-post-clone-error").as_deref(),
        Some(reason)
    );
    assert!(drawn(cx, "checkout-post-clone-read-error"));
    assert!(!drawn(cx, FOOTER_SAVE));
    click(cx, "checkout-post-clone-retry");
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].1, Field::PostClone.path());
    helm.reply(
        reads[0].0,
        200,
        Some(json!({ "command": "cli command", "compare_on_save": true })),
    );
    settle(cx);
    assert_eq!(text(cx, Field::PostClone), "cli command");
    assert_eq!(
        label_of(cx, "checkout-post-clone-error").as_deref(),
        Some(reason)
    );
    assert!(!drawn(cx, FOOTER_SAVE));
    assert_eq!(
        sent(&mut helm),
        [],
        "the refusal never becomes an edit again"
    );

    type_into(cx, Field::PostClone, " # new edit");
    let edited = text(cx, Field::PostClone);
    assert_ne!(edited, "cli command", "the new value is editable");
    click(cx, FOOTER_SAVE);
    let (_, body) = one_put(&mut helm, Field::PostClone.path());
    assert_eq!(
        body,
        json!({ "command": edited, "expected": "cli command" })
    );
}

/// SPEC.md: "The command field preserves all other spacing and newlines", only "an all-whitespace field" clears it,
/// and Enter in the multi-line field types a line break rather than saving. A command is executable text, so a client
/// that trimmed it or dropped a line would change what runs in every new checkout. Specified: a command typed as two
/// lines with leading and trailing spaces (Enter between them) is sent verbatim in `PUT /api/checkout-post-clone`, and
/// the helm's done reply reads the route back and shows "Post-clone command saved"; in a later opening, a field of
/// spaces and a line break sends `{"command": null}`.
#[gpui::test]
fn the_post_clone_command_keeps_its_spacing_and_only_all_whitespace_clears_it(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_checkouts(&mut helm, cx, Value::Null, Value::Null, true);
    type_into(cx, Field::PostClone, "  printf 'first'");
    keys(cx, "enter");
    cx.simulate_input("printf 'second'  ");
    let chosen = "  printf 'first'\nprintf 'second'  ";
    assert_eq!(text(cx, Field::PostClone), chosen);
    click(cx, FOOTER_SAVE);
    let (id, body) = one_put(&mut helm, Field::PostClone.path());
    assert_eq!(body, json!({ "command": chosen }));
    helm.reply(id, 204, None);
    settle(cx);
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1, "one read back: {reads:?}");
    helm.reply(reads[0].0, 200, Some(json!({ "command": chosen })));
    settle(cx);
    assert_eq!(
        label_of(cx, "checkout-post-clone-saved").as_deref(),
        Some("Post-clone command saved")
    );

    keys(cx, "escape");
    show_checkouts(&mut helm, cx, Value::Null, Value::Null, true);
    type_into(cx, Field::PostClone, "  ");
    keys(cx, "enter");
    cx.simulate_input(" ");
    assert_eq!(text(cx, Field::PostClone), "  \n ", "the premise");
    click(cx, FOOTER_SAVE);
    let (_, body) = one_put(&mut helm, Field::PostClone.path());
    assert_eq!(body, json!({ "command": null }));
}

/// The old field refused to save over a stored carriage return, because a browser's text area turned it into a line
/// feed before the draft saw it; GPUI Kit's multi-line field keeps it, so this port saves such a command instead (a
/// recorded deviation). This pins the premise that makes that safe: an edit keeps every carriage return the helm
/// stored. Specified: a stored `printf 'one\r\ntwo'` shows exactly, and after typing " # edited" into it, Save sends a
/// command that is the stored one with " # edited" inserted somewhere.
#[gpui::test]
fn a_stored_carriage_return_survives_an_edit(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let stored = "printf 'one\r\ntwo'";
    show_checkouts(&mut helm, cx, Value::Null, json!(stored), true);
    assert_eq!(text(cx, Field::PostClone), stored);
    type_into(cx, Field::PostClone, " # edited");
    click(cx, FOOTER_SAVE);
    let (_, body) = one_put(&mut helm, Field::PostClone.path());
    let sent = body["command"].as_str().unwrap_or_default().to_owned();
    assert!(sent.contains(" # edited"), "{sent:?}");
    assert_eq!(sent.replacen(" # edited", "", 1), stored);
}

/// An explicit save outlives the close gesture but Settings stays open until it settles. Escape asks, with no
/// Discard while the folder save is in flight. The done answer sends its read back and closes Settings.
#[gpui::test]
fn a_folder_save_in_flight_holds_settings_open_until_done(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_checkouts(&mut helm, cx, Value::Null, Value::Null, true);
    type_into(cx, Field::Folder, "~/survives-close");
    click(cx, FOOTER_SAVE);
    let (id, _) = one_put(&mut helm, Field::Folder.path());
    keys(cx, "escape");
    assert_eq!(
        label_of(cx, "prompt").as_deref(),
        Some("Save changes before closing?")
    );
    assert!(
        !drawn(cx, "prompt-alternative-0"),
        "the write is already out"
    );
    assert_eq!(preference_work(cx), Some(Work::Save));
    helm.reply(id, 204, None);
    settle(cx);
    assert!(!any_open(cx), "the completed save closes Settings");
    assert_eq!(preference_work(cx), None);
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].1, Field::Folder.path());
}

/// The spare switch follows Agent trust's rules: a failed read never counts as off, a flip saves at once through its
/// own route, and a refusal restores the stored choice with the helm's reason (the old "prepared checkout settings
/// preserve saved choices across reads, refusals and close"). SPEC.md also has the help link "the full earlier-clone
/// caveats". Specified: a refused read leaves the switch taking no click, with "Could not save or read prepared
/// checkout settings:" and the helm's words; in a new opening, read on, the documentation link is drawn as a link; a
/// click sends `PUT /api/checkout-spare` with `{"enabled": false}` under the preference row's key and shows off while
/// it is in flight; the helm's refusal shows on again with the reason; the next click, done, shows off.
#[gpui::test]
fn the_spare_switch_saves_through_its_route_and_a_refusal_restores_it(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, section_button(Section::Checkouts));
    let reads = sent_reads(&mut helm);
    let paths: Vec<&str> = reads.iter().map(|(_, path)| path.as_str()).collect();
    assert_eq!(
        paths, CHECKOUT_READS,
        "the premise: Checkouts read its routes"
    );
    helm.reply(reads[0].0, 200, Some(json!({ "root": null })));
    helm.reply(reads[1].0, 200, Some(json!({ "command": null })));
    helm.reply(reads[2].0, 503, Some(json!("fixture spare read refused")));
    settle(cx);
    click(cx, super::checkouts::SPARE_ID);
    assert_eq!(sent(&mut helm), [], "no toggle over an unknown choice");
    assert_eq!(
        label_of(cx, super::checkouts::SPARE_ERROR_ID).as_deref(),
        Some("Could not save or read prepared checkout settings: fixture spare read refused")
    );

    keys(cx, "escape");
    show_checkouts(&mut helm, cx, Value::Null, Value::Null, true);
    let link = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(super::checkouts::SPARE_DOCS_ID))
        .map(|node| (node.role, node.label.map(|label| label.to_string())));
    assert_eq!(
        link,
        Some((
            Some(Role::Link),
            Some("How prepared checkouts differ from a fresh clone ↗".to_owned())
        ))
    );
    click(cx, super::checkouts::SPARE_ID);
    let (id, body) = one_put(&mut helm, "/api/checkout-spare");
    assert_eq!(body, json!({ "enabled": false }));
    assert_eq!(preference_work(cx), Some(Work::Save));
    assert_eq!(
        switch_on(cx, super::checkouts::SPARE_ID),
        Some(false),
        "pending"
    );
    helm.reply(id, 503, Some(json!("fixture spare save refused")));
    settle(cx);
    assert_eq!(
        switch_on(cx, super::checkouts::SPARE_ID),
        Some(true),
        "restored"
    );
    assert_eq!(
        label_of(cx, super::checkouts::SPARE_ERROR_ID).as_deref(),
        Some("Could not save or read prepared checkout settings: fixture spare save refused")
    );
    click(cx, super::checkouts::SPARE_ID);
    let (id, _) = one_put(&mut helm, "/api/checkout-spare");
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(
        switch_on(cx, super::checkouts::SPARE_ID),
        Some(false),
        "saved"
    );
}

/// A root view drawing one host-scoped folder field, as the host settings dialog is to (SPEC.md, "Topology").
struct HostFolder {
    root: root::Attachment,
    field: Entity<super::CheckoutField>,
    /// How many times the field raised [`super::CheckoutUpdated`], as the launcher's refresh would see it.
    updates: usize,
    _updated: gpui::Subscription,
}

impl Render for HostFolder {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = div().size_full().p(px(20.)).child(self.field.clone());
        root::wrap(content, &self.root, window, cx)
    }
}

/// The shared folder field is also the host settings dialog's (SPEC.md, "Topology": "An empty folder uses the default
/// from Settings → Checkouts, shown as a placeholder; Use default clears a saved override"), and that dialog is built
/// later on this interface, so the host scope is pinned here. A host's folder is its own work key, so it never waits
/// on a Settings save. Specified: a field over host 2 reads `GET /api/hosts/2/checkout-root`; with an override of
/// "/srv/own" and a default of "~/def" it shows the override and offers Use default. Use default compares the last
/// loaded override, including after a conflict, and waits for the read before making another clear available. It
/// sends a null root under the host's key; the done reply reads the route back, and the answer (no
/// override) empties the field, shows "Default: ~/def" as its placeholder and takes Use default away. The field raises
/// `CheckoutUpdated` for a refresh that discovers the CLI edit and for the done save, so the launcher can refresh
/// what depends on the folder. A refusal on its own is not an update.
#[gpui::test]
fn a_host_folder_shows_the_default_as_placeholder_and_use_default_clears_it(
    cx: &mut TestAppContext,
) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![preferences_state(&Preferences::default())]);
    cx.run_until_parked();
    let (view, cx) = test_support::open_view(cx, |window, cx| {
        let field = cx.new(|cx| {
            super::CheckoutField::folder(
                super::FolderScope::Host("2".to_owned()),
                "Fixture note",
                window,
                cx,
            )
        });
        let updated = cx.subscribe(
            &field,
            |host: &mut HostFolder, _, _: &super::CheckoutUpdated, _| {
                host.updates += 1;
            },
        );
        HostFolder {
            root: root::attach(window, cx),
            field,
            updates: 0,
            _updated: updated,
        }
    });
    let updates = |cx: &mut VisualTestContext| cx.update(|_, cx| view.read(cx).updates);
    let path = "/api/hosts/2/checkout-root";
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1, "one read: {reads:?}");
    assert_eq!(reads[0].1, path);
    helm.reply(
        reads[0].0,
        200,
        Some(json!({ "root": "/srv/own", "default": "~/def", "compare_on_save": true })),
    );
    settle(cx);
    let shown =
        |cx: &mut VisualTestContext| cx.update(|_, cx| view.read(cx).field.read(cx).text(cx));
    assert_eq!(shown(cx), "/srv/own");

    click(cx, "checkout-folder-use-default");
    let (id, body) = one_put(&mut helm, path);
    assert_eq!(body, json!({ "root": null, "expected": "/srv/own" }));
    let host_work = cx.update(|_, cx| {
        session::current_operations(cx)
            .and_then(|operations| operations.read(cx).in_flight(&OpKey::host("2")))
    });
    assert_eq!(host_work, Some(Work::Save));
    assert_eq!(preference_work(cx), None, "not the preference row's key");
    assert_eq!(updates(cx), 0, "nothing saved yet");
    helm.reply(id, 409, Some(json!("fixture conflict")));
    settle(cx);
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].1, path);
    assert_eq!(updates(cx), 0, "a conflict alone does not signal an update");
    click(cx, "checkout-folder-use-default");
    assert_eq!(sent(&mut helm), [], "Use default waits for the read");
    helm.reply(
        reads[0].0,
        200,
        Some(json!({ "root": "/srv/cli", "default": "~/def", "compare_on_save": true })),
    );
    settle(cx);
    assert_eq!(shown(cx), "/srv/cli");
    assert_eq!(updates(cx), 1, "the reload found a new override");
    click(cx, "checkout-folder-use-default");
    let (id, body) = one_put(&mut helm, path);
    assert_eq!(body, json!({ "root": null, "expected": "/srv/cli" }));
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(updates(cx), 2, "the done save raised an update");
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1, "one read back: {reads:?}");
    helm.reply(
        reads[0].0,
        200,
        Some(json!({ "root": null, "default": "~/def", "compare_on_save": true })),
    );
    settle(cx);
    assert_eq!(shown(cx), "");
    assert!(painted(cx).contains("Default: ~/def"), "{}", painted(cx));
    assert!(!drawn(cx, "checkout-folder-use-default"));

    click(cx, "checkout-folder");
    let focused = cx.update(|window, cx| view.read(cx).field.read(cx).focused(window, cx));
    assert!(focused, "the premise: the click put focus in the field");
    cx.simulate_input("relative");
    click(cx, "checkout-folder-save");
    let (id, body) = one_put(&mut helm, path);
    assert_eq!(body, json!({ "root": "relative", "expected": null }));
    helm.reply(id, 400, Some(json!("fixture refusal")));
    settle(cx);
    assert_eq!(updates(cx), 2, "a refused save raises nothing by itself");
}

/// Both old fields offered Retry when their read failed, and a failed read must never pass for an empty value: Save
/// then would clear the folder. Specified: the folder's read refused, the field shows the helm's words as an alert, has
/// Retry and no Save, and takes no typing; Retry sends exactly one GET of the route; its answer shows the stored
/// folder, takes Retry away, brings Save back and makes the field editable.
#[gpui::test]
fn a_failed_folder_read_shows_its_reason_and_retry_reads_again(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    click(cx, section_button(Section::Checkouts));
    let first = sent_reads(&mut helm);
    let paths: Vec<&str> = first.iter().map(|(_, path)| path.as_str()).collect();
    assert_eq!(
        paths, CHECKOUT_READS,
        "the premise: Checkouts read its routes"
    );
    helm.reply(first[0].0, 503, Some(json!("fixture folder read refused")));
    helm.reply(first[1].0, 200, Some(json!({ "command": null })));
    helm.reply(first[2].0, 200, Some(json!({ "enabled": true })));
    settle(cx);
    assert_eq!(
        label_of(cx, "checkout-folder-error").as_deref(),
        Some("fixture folder read refused")
    );
    assert!(drawn(cx, "checkout-folder-retry"));
    assert!(!drawn(cx, FOOTER_SAVE), "Retry takes Save's place");
    type_into(cx, Field::Folder, "typed");
    assert_eq!(text(cx, Field::Folder), "", "still read-only");

    click(cx, "checkout-folder-retry");
    let again = sent_reads(&mut helm);
    let paths: Vec<&str> = again.iter().map(|(_, path)| path.as_str()).collect();
    assert_eq!(paths, [Field::Folder.path()], "Retry reads the route once");
    helm.reply(again[0].0, 200, Some(json!({ "root": "~/stored" })));
    settle(cx);
    assert_eq!(text(cx, Field::Folder), "~/stored");
    assert!(!drawn(cx, "checkout-folder-retry"));
    assert!(!drawn(cx, "checkout-folder-error"));
    assert!(!drawn(cx, FOOTER_SAVE));
    type_into(cx, Field::Folder, "/more");
    assert_eq!(text(cx, Field::Folder), "~/stored/more", "editable now");
    assert!(drawn(cx, FOOTER_SAVE));
}

/// SPEC.md: "Switching sections preserves unsaved fields", and Discard in the close question drops them. A draft
/// that a section switch dropped would lose what the user typed; one that outlived a discarded close would show text
/// the helm does not hold. Specified: a command typed but neither saved nor refused is still there after showing
/// Sessions and Checkouts again, with nothing sent; after Escape, Discard and a new opening, the field shows the stored
/// command.
#[gpui::test]
fn an_unsaved_draft_survives_a_section_switch_and_goes_with_the_dialog(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_checkouts(&mut helm, cx, Value::Null, json!("stored"), true);
    type_into(cx, Field::PostClone, " typed");
    let draft = text(cx, Field::PostClone);
    assert_ne!(draft, "stored", "the premise: the field holds a draft");
    click(cx, section_button(Section::Sessions));
    click(cx, section_button(Section::Checkouts));
    assert_eq!(text(cx, Field::PostClone), draft);
    assert_eq!(sent(&mut helm), [], "nothing was saved");
    keys(cx, "escape");
    click(cx, "prompt-alternative-0");
    show_checkouts(&mut helm, cx, Value::Null, json!("stored"), true);
    assert_eq!(text(cx, Field::PostClone), "stored");
}

/// Checkouts' single footer Save must serialize the two writes on the shared key. Its first read-back is still
/// sent as before, and the second field waits for the first acknowledgment even if the user asks to Close and then
/// chooses Keep editing.
#[gpui::test]
fn one_checkout_save_writes_both_changed_fields_in_order(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_checkouts(&mut helm, cx, Value::Null, Value::Null, true);
    fill(cx, Field::Folder.selector(), "~/work");
    fill(cx, Field::PostClone.selector(), "make setup");
    assert!(drawn(cx, FOOTER_SAVE), "the two fields are edited");
    click(cx, FOOTER_SAVE);
    let (folder, body) = one_put(&mut helm, Field::Folder.path());
    assert_eq!(body, json!({"root": "~/work"}));
    click(cx, "settings-footer-close");
    assert_eq!(
        label_of(cx, "prompt").as_deref(),
        Some("Save changes before closing?")
    );
    assert!(
        !drawn(cx, "prompt-alternative-0"),
        "an acknowledged save is still pending"
    );
    click(cx, "prompt-cancel");
    assert!(any_open(cx));
    assert!(
        sent(&mut helm).is_empty(),
        "the second write is still queued"
    );
    helm.reply(folder, 204, None);
    settle(cx);
    let requests = sent(&mut helm);
    assert_eq!(requests.len(), 2, "the refetch and second write");
    assert_eq!(
        (requests[0].method, requests[0].path.as_str()),
        (Method::Get, Field::Folder.path())
    );
    assert_eq!(
        (requests[1].method, requests[1].path.as_str()),
        (Method::Put, Field::PostClone.path())
    );
    assert_eq!(requests[1].body, Some(json!({"command": "make setup"})));
    helm.reply(requests[0].id, 200, Some(json!({"root": "~/work"})));
    helm.reply(requests[1].id, 204, None);
    settle(cx);
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].1, Field::PostClone.path());
}

/// Discarding Clear saved token restores the input's ability to accept a replacement.
#[gpui::test]
fn discarding_an_ntfy_clear_unlocks_the_token_field(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_ntfy(
        &mut helm,
        cx,
        ntfy("https://ntfy.sh", "alerts", "default", true),
    );
    reach(cx, "ntfy-clear-token");
    keys(cx, "space");
    assert_eq!(switch_on(cx, "ntfy-clear-token"), Some(true));
    click(cx, "settings-footer-discard");
    assert_eq!(switch_on(cx, "ntfy-clear-token"), Some(false));
    assert!(
        sent(&mut helm).is_empty(),
        "Discard does not write the token"
    );
    fill(cx, "ntfy-token", "replacement");
    let (_, body) = save_ntfy(&mut helm, cx);
    assert_eq!(body["token"], "replacement");
}

/// Closing must name hidden forms and keep later drafts when a save fails. The first refusal selects that form's
/// page. Checkouts remains unsaved and never sends a write over the refusal.
#[gpui::test]
fn close_save_stops_at_the_first_failure_and_shows_that_page(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    cx.simulate_resize(size(px(720.), px(560.)));
    show_checkouts(&mut helm, cx, Value::Null, Value::Null, true);
    fill(cx, Field::PostClone.selector(), "make setup");
    click(cx, section_button(Section::Notifications));
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].1, "/api/ntfy-alerts");
    helm.reply(
        reads[0].0,
        200,
        Some(ntfy("https://ntfy.sh", "stored", "default", false)),
    );
    settle(cx);
    fill(cx, "ntfy-topic", "edited");
    click(cx, section_button(Section::Sessions));
    assert_eq!(heading(cx).as_deref(), Some("Sessions"));
    click(cx, "settings-footer-close");
    assert_eq!(
        label_of(cx, "prompt").as_deref(),
        Some("Save changes before closing?")
    );
    assert!(painted(cx).contains("Unsaved: ntfy alerts, Checkout settings"));
    click(cx, "prompt-confirm");
    let (alerts, _) = one_put(&mut helm, "/api/ntfy-alerts");
    helm.reply(alerts, 400, Some(json!("fixture: cannot save alerts")));
    settle(cx);
    assert_eq!(heading(cx).as_deref(), Some("Notifications"));
    assert_eq!(
        label_of(cx, "ntfy-error").as_deref(),
        Some("fixture: cannot save alerts")
    );
    assert!(
        shows_whole(cx, "ntfy-error", DIALOG_BODY),
        "the refusal is visible without scrolling"
    );
    assert_eq!(sent(&mut helm), [], "the other page sent no write");
    click(cx, section_button(Section::Checkouts));
    assert_eq!(text(cx, Field::PostClone), "make setup");
}

/// A running page Save and a later close answer each authorize only the drafts they observed. After the folder
/// settles, the second checkout edit and a new Notifications server must stay unsaved until a fresh action.
#[gpui::test]
fn close_save_leaves_unlisted_forms_and_later_versions_unsaved(cx: &mut TestAppContext) {
    let (mut helm, cx, _) = open_rail(cx, &Preferences::default());
    show_checkouts(&mut helm, cx, Value::Null, Value::Null, true);
    click(cx, section_button(Section::Notifications));
    let reads = sent_reads(&mut helm);
    let [(read, path)] = reads.as_slice() else {
        panic!("expected one Notifications read before the held save");
    };
    assert_eq!(path, "/api/ntfy-alerts");
    helm.reply(
        *read,
        200,
        Some(ntfy("https://ntfy.sh", "saved", "default", true)),
    );
    settle(cx);
    click(cx, section_button(Section::Checkouts));
    fill(cx, Field::Folder.selector(), "~/approved");
    fill(cx, Field::PostClone.selector(), "make approved");
    click(cx, FOOTER_SAVE);
    let (folder, body) = one_put(&mut helm, Field::Folder.path());
    assert_eq!(body, json!({ "root": "~/approved" }));
    assert_eq!(preference_work(cx), Some(Work::Save));
    click(cx, section_button(Section::Terminal));
    fill(cx, super::file_types::ID, "md, pdf");
    keys(cx, "escape");
    assert!(painted(cx).contains("Unsaved: File types, Checkout settings"));
    click(cx, "prompt-confirm");

    click(cx, section_button(Section::Notifications));
    fill(cx, "ntfy-server", "https://new.example");
    assert!(drawn(cx, "ntfy-token-origin-hint"));
    click(cx, section_button(Section::Checkouts));
    fill(cx, Field::PostClone.selector(), "make later");
    assert_eq!(sent(&mut helm), []);

    helm.reply(folder, 204, None);
    settle(cx);
    let commands = sent(&mut helm);
    let [read, save] = commands.as_slice() else {
        panic!("expected the folder read-back and only the unchanged draft's save");
    };
    assert_eq!(
        (read.method, read.path.as_str()),
        (Method::Get, Field::Folder.path())
    );
    assert_eq!(
        (save.method, save.path.as_str()),
        (Method::Put, "/api/preferences")
    );
    assert_eq!(save.body, Some(json!({ "open_extensions": "md, pdf" })));
    helm.reply(read.id, 200, Some(json!({ "root": "~/approved" })));
    helm.reply(save.id, 200, None);
    settle(cx);
    assert_eq!(sent(&mut helm), []);
    assert!(any_open(cx));
    click(cx, section_button(Section::Notifications));
    assert!(drawn(cx, "ntfy-token-origin-hint"));
    click(cx, section_button(Section::Checkouts));
    assert_eq!(text(cx, Field::PostClone), "make later");
    click(cx, FOOTER_SAVE);
    assert_eq!(
        one_put(&mut helm, Field::PostClone.path()).1,
        json!({ "command": "make later" })
    );
}

// ---- Notifications: ntfy alerts ------------------------------------------------------------------------------------

/// The helm's ntfy settings as its route answers them, on `server` with the given topic, priority and token presence.
fn ntfy(server: &str, topic: &str, priority: &str, token_set: bool) -> Value {
    json!({ "enabled": false, "server_url": server, "topic": topic, "priority": priority, "token_set": token_set })
}

/// Open Settings and show Notifications, returning the id of the one read that sent.
fn show_notifications(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> u64 {
    open_settings(cx);
    assert_eq!(sent(helm), [], "the premise: Sessions reads nothing");
    click(cx, section_button(Section::Notifications));
    let reads = sent_reads(helm);
    let paths: Vec<&str> = reads.iter().map(|(_, path)| path.as_str()).collect();
    assert_eq!(paths, ["/api/ntfy-alerts"], "one read of the settings");
    reads[0].0
}

/// Show Notifications and answer its read with `stored`.
fn show_ntfy(helm: &mut FakeHelm, cx: &mut VisualTestContext, stored: Value) {
    let read = show_notifications(helm, cx);
    helm.reply(read, 200, Some(stored));
    settle(cx);
}

/// Turn the wheel by `delta` pixels over the page (negative scrolls toward the end). It turns over the page's
/// padding so the page takes the event even if a field such as Checkouts' multiline command also scrolls.
pub(super) fn wheel(cx: &mut VisualTestContext, delta: f32) {
    let body = bounds(cx, DIALOG_BODY);
    cx.simulate_event(gpui::ScrollEvent {
        position: gpui::point(body.left() + px(10.), body.center().y),
        delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(delta))),
        ..Default::default()
    });
    settle(cx);
}

/// Wheel the dialog's body until the element drawn under `selector` shows whole in it, as a user scrolls to a control
/// before clicking it. A click lands wherever the element's center is drawn, and below the body's fold that is the
/// footer or nothing: the Sounds sub-heading above the ntfy form puts the form's lower fields there.
pub(super) fn into_view(cx: &mut VisualTestContext, selector: &'static str) {
    for _ in 0..60 {
        if shows_whole(cx, selector, DIALOG_BODY) {
            return;
        }
        let below = bounds(cx, selector).top() > bounds(cx, DIALOG_BODY).top();
        wheel(cx, if below { -40. } else { 40. });
    }
    panic!("{selector} never showed whole in the dialog's body");
}

/// Replace the text of the field drawn under `selector` with `text`, as the user would, scrolling it into view first.
pub(super) fn fill(cx: &mut VisualTestContext, selector: &'static str, text: &str) {
    into_view(cx, selector);
    click(cx, selector);
    crate::test_support::select_all_text(cx);
    if text.is_empty() {
        keys(cx, "backspace");
    } else {
        cx.simulate_input(text);
        test_support::check(cx);
    }
}

/// Tab until the control with element id `id` holds focus, which scrolls it into view: the form is taller than the
/// dialog's body, so a click cannot reach its lower controls. Allow one pass through the presented focus targets,
/// including the navigation rows, so adding pages does not shorten the part of the form this helper can reach.
pub(super) fn reach(cx: &mut VisualTestContext, id: &str) {
    let presented = frame(cx);
    assert!(
        presented
            .iter()
            .any(|node| node.element_id.as_deref() == Some(id) && node.flags.focusable),
        "{id} must be focusable before stepping Tab"
    );
    let focusable = presented.iter().filter(|node| node.flags.focusable).count();
    for _ in 0..=focusable {
        if focused_id(cx).as_deref() == Some(id) {
            return;
        }
        keys(cx, "tab");
    }
    panic!("Tab never reached {id} in a frame with {focusable} focusable nodes");
}

/// Press Save, and return the one command it sent, which must be the settings' `PUT`: its id and body.
fn save_ntfy(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> (u64, Value) {
    reach(cx, FOOTER_SAVE);
    keys(cx, "space");
    let sent = sent(helm);
    assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
    assert_eq!(
        (sent[0].method, sent[0].path.as_str()),
        (Method::Put, "/api/ntfy-alerts")
    );
    (sent[0].id, sent[0].body.clone().unwrap_or_default())
}

/// The settings live behind a route the helm does not push, so the form reads them when the section first shows, and
/// until that answers the form's defaults are not the stored settings: a Save then would overwrite them (the old
/// "load refusal does not seed an editable default"). Specified: showing Notifications sends one GET; until it
/// answers, the switch takes no click and Save is unavailable; once answered, Save sends the stored server, topic and
/// priority with no token (keep the saved one), and the topic field holds the stored topic, so typing after it extends
/// it; showing Sessions and Notifications again reads nothing more.
#[gpui::test]
fn notifications_reads_its_settings_once_and_stays_locked_until_they_answer(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_notifications(&mut helm, cx);
    click(cx, "ntfy-enabled");
    assert_eq!(sent(&mut helm), [], "a form not read yet takes no edit");
    assert_eq!(available(cx, "ntfy-enabled"), Some(false));
    assert!(!drawn(cx, FOOTER_SAVE));

    helm.reply(
        read,
        200,
        Some(ntfy("https://ntfy.example", "saved", "high", true)),
    );
    settle(cx);
    assert_eq!(switch_on(cx, "ntfy-enabled"), Some(false));
    assert!(!drawn(cx, FOOTER_SAVE), "the stored values are unchanged");
    fill(cx, "ntfy-topic", "saved-more");
    let (id, body) = save_ntfy(&mut helm, cx);
    let expected = json!({ "enabled": false, "server_url": "https://ntfy.example", "topic": "saved-more",
                               "priority": "high", "token": null });
    assert_eq!(body, expected);
    assert_eq!(
        preference_work(cx),
        Some(Work::Save),
        "under the preferences key"
    );
    let stored = ntfy("https://ntfy.example", "saved-more", "high", true);
    helm.reply(id, 200, Some(stored));
    settle(cx);
    click(cx, section_button(Section::Sessions));
    click(cx, section_button(Section::Notifications));
    assert_eq!(sent(&mut helm), [], "read once per opening");
}

/// SPEC.md: the settings "are explicitly saved helm configuration", and "The stored token is never returned": the form
/// must send what was typed, show the helm's answer, and then hold no token, or the next Save would send it again,
/// possibly to another server. Specified: a server, topic and token typed, the switch turned on and High pressed in the
/// priority segments, Save sends all five in one `PUT` and shows "Working…" while it is out; the helm's answer shows
/// "ntfy alert settings saved"; a later edit sends the new topic with no token.
#[gpui::test]
fn a_save_sends_the_whole_form_and_holds_no_token_after(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_ntfy(&mut helm, cx, ntfy("https://ntfy.sh", "", "default", false));
    click(cx, "ntfy-enabled");
    fill(cx, "ntfy-server", "http://127.0.0.1:9");
    fill(cx, "ntfy-topic", "browser-alerts");
    fill(cx, "ntfy-token", "browser-token");
    reach(cx, "ntfy-priority-High");
    keys(cx, "space");
    assert_eq!(
        switch_on(cx, "ntfy-priority-High"),
        Some(true),
        "High shows pressed"
    );
    assert_eq!(sent(&mut helm), [], "nothing saves before Save");

    let (id, body) = save_ntfy(&mut helm, cx);
    let mut form = json!({ "enabled": true, "server_url": "http://127.0.0.1:9", "topic": "browser-alerts",
                           "priority": "high", "token": "browser-token" });
    assert_eq!(body, form);
    assert_eq!(label_of(cx, "ntfy-working").as_deref(), Some("Working…"));
    let stored = json!({ "enabled": true, "server_url": "http://127.0.0.1:9", "topic": "browser-alerts",
                         "priority": "high", "token_set": true });
    helm.reply(id, 200, Some(stored));
    settle(cx);
    assert_eq!(
        label_of(cx, "ntfy-saved").as_deref(),
        Some("ntfy alert settings saved")
    );
    assert!(!drawn(cx, "ntfy-working"));

    assert!(!drawn(cx, FOOTER_SAVE), "acknowledgment leaves no draft");
    fill(cx, "ntfy-topic", "next-topic");
    let (_, body) = save_ntfy(&mut helm, cx);
    form["topic"] = json!("next-topic");
    form["token"] = Value::Null;
    assert_eq!(body, form, "the token field was emptied");
}

/// SPEC.md: "Save refusals appear inline", and SPEC.md's "Switching sections preserves unsaved fields, outcome lines
/// and pending saves". The helm refuses a bad topic with its own fixed words, and the user must be able to correct the
/// form rather than retype it. Specified: a topic with a slash, saved and refused, shows the helm's words as an alert;
/// after showing Sessions and Notifications again the words are still there and Save sends the same topic; once
/// Settings is closed with Discard in the close question and opened again, the form reads the settings again and Save
/// sends the stored topic.
#[gpui::test]
fn a_refused_save_keeps_the_form_with_the_helms_words_until_closed(cx: &mut TestAppContext) {
    let refusal = "topic must contain at most 64 letters, numbers, underscores or dashes";
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_ntfy(
        &mut helm,
        cx,
        ntfy("https://ntfy.sh", "kept", "default", false),
    );
    fill(cx, "ntfy-topic", "bad/topic");
    let (id, _) = save_ntfy(&mut helm, cx);
    helm.reply(id, 400, Some(json!(refusal)));
    settle(cx);
    assert_eq!(label_of(cx, "ntfy-error").as_deref(), Some(refusal));
    assert!(!drawn(cx, "ntfy-saved"));

    click(cx, section_button(Section::Sessions));
    click(cx, section_button(Section::Notifications));
    assert_eq!(label_of(cx, "ntfy-error").as_deref(), Some(refusal));
    let (id, body) = save_ntfy(&mut helm, cx);
    assert_eq!(body["topic"], json!("bad/topic"), "the draft was kept");
    helm.reply(id, 400, Some(json!(refusal)));
    settle(cx);

    keys(cx, "escape");
    click(cx, "prompt-alternative-0");
    assert!(!any_open(cx), "the premise: Settings closed");
    show_ntfy(
        &mut helm,
        cx,
        ntfy("https://ntfy.sh", "kept", "default", false),
    );
    assert!(!drawn(cx, "ntfy-error"), "closing discarded the outcome");
    assert!(!drawn(cx, FOOTER_SAVE), "closing discarded the draft");
    reach(cx, "ntfy-enabled");
    keys(cx, "space");
    let (_, body) = save_ntfy(&mut helm, cx);
    assert_eq!(
        body["topic"],
        json!("kept"),
        "the new edit uses the stored topic"
    );
}

/// SPEC.md: "Changing to another server clears a saved token unless a new one is entered, and the form says so before
/// saving", and the token can be cleared on purpose. The hint must follow the helm's same-server rule, or it would
/// promise a removal that does not happen, or stay silent about one that does. Specified: with a token saved, another
/// server shows the hint; the saved server with a path added hides it; another server with a token typed hides it, and
/// emptying the token brings it back; Save then sends no token. The token field's placeholder, the form's only sign of
/// whether a token is saved, reads "Token saved; leave blank to keep it" on the saved server, "Saved token will be
/// cleared" on another, and "No token saved" once the helm answers that none is. Clear saved token holds the token field
/// read-only, as the old field was: text typed into it then never reaches the form, so with Clear turned off again Save
/// sends no token; with Clear on, Save sends an empty one.
#[gpui::test]
fn another_server_says_a_saved_token_will_be_cleared_before_saving(cx: &mut TestAppContext) {
    let hint = "ntfy-token-origin-hint";
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_ntfy(
        &mut helm,
        cx,
        ntfy("https://ntfy.example", "alerts", "default", true),
    );
    assert!(!drawn(cx, hint), "the premise: same server, no hint");
    assert!(painted(cx).contains("Token saved; leave blank to keep it"));
    fill(cx, "ntfy-server", "http://localhost:1");
    assert!(drawn(cx, hint), "another server shows the hint");
    assert!(painted(cx).contains("Changing the server clears the saved token when you save"));
    assert!(painted(cx).contains("Saved token will be cleared"));
    fill(cx, "ntfy-server", "https://ntfy.example/prefix");
    assert!(!drawn(cx, hint), "a path stays on the same server");
    fill(cx, "ntfy-server", "http://localhost:1");
    fill(cx, "ntfy-token", "replacement-token");
    assert!(!drawn(cx, hint), "a new token goes with the new server");
    fill(cx, "ntfy-token", "");
    assert!(drawn(cx, hint));

    let (id, body) = save_ntfy(&mut helm, cx);
    assert_eq!(
        (&body["server_url"], &body["token"]),
        (&json!("http://localhost:1"), &Value::Null)
    );
    helm.reply(
        id,
        200,
        Some(ntfy("http://localhost:1", "alerts", "default", false)),
    );
    settle(cx);
    assert!(!drawn(cx, hint), "no token is saved any more");
    assert!(painted(cx).contains("No token saved"));

    reach(cx, "ntfy-clear-token");
    keys(cx, "space");
    fill(cx, "ntfy-token", "typed-while-clearing");
    reach(cx, "ntfy-clear-token");
    keys(cx, "space");
    assert_eq!(
        switch_on(cx, "ntfy-clear-token"),
        Some(false),
        "the premise: Clear is off again"
    );
    assert!(!drawn(cx, FOOTER_SAVE), "the read-only field took no edit");
    assert!(sent(&mut helm).is_empty());
    reach(cx, "ntfy-clear-token");
    keys(cx, "space");
    let (_, body) = save_ntfy(&mut helm, cx);
    assert_eq!(body["token"], json!(""), "an explicit clear");
}

/// The old spec's rule: "Load refusal does not seed an editable default that could overwrite saved settings. Retry
/// obtains an authoritative snapshot before enabling writes." A form whose read failed must not let the defaults it
/// shows be saved over the stored settings, and must say why and let the user read again. Specified: the read refused,
/// its words show as an alert, the switch and Save are unavailable; Retry loading sends the read again, and once it
/// answers the alert is gone and the switch and Save are available.
#[gpui::test]
fn a_failed_read_shows_its_reason_and_retry_reads_again(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_notifications(&mut helm, cx);
    helm.reply(read, 500, Some(json!("could not read ntfy alert settings")));
    settle(cx);
    assert_eq!(
        label_of(cx, "ntfy-load-error").as_deref(),
        Some("could not read ntfy alert settings")
    );
    assert_eq!(available(cx, "ntfy-enabled"), Some(false));
    assert!(!drawn(cx, FOOTER_SAVE));

    reach(cx, "ntfy-retry");
    keys(cx, "space");
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1, "read again: {reads:?}");
    assert_eq!(reads[0].1, "/api/ntfy-alerts");
    helm.reply(
        reads[0].0,
        200,
        Some(ntfy("https://ntfy.sh", "", "default", false)),
    );
    settle(cx);
    assert!(!drawn(cx, "ntfy-load-error"));
    assert_eq!(available(cx, "ntfy-enabled"), Some(true));
    assert!(!drawn(cx, FOOTER_SAVE));
}

/// The topic field of the ntfy form drawn last.
fn topic_input(cx: &mut VisualTestContext) -> crate::components::TextInputState {
    cx.update(|_, cx| {
        let dialog = layers::last_opened::<super::SettingsDialog>(cx);
        let dialog = dialog.expect("the premise: Settings is open");
        let form = dialog.read(cx).ntfy.clone();
        form.expect("the premise: Notifications was shown")
            .read(cx)
            .topic_input()
    })
}

/// Linux Ctrl+F belongs to the focused field even when the caret cannot move. Bubbling at that boundary would
/// focus Settings search and send the next typed character there.
#[cfg(target_os = "linux")]
#[gpui::test]
fn ctrl_f_keeps_focus_in_a_settings_field_at_its_end(cx: &mut TestAppContext) {
    let (mut helm, cx, _) = open_rail(cx, &Preferences::default());
    show_ntfy(&mut helm, cx, ntfy("https://ntfy.sh", "", "default", false));
    fill(cx, "ntfy-topic", "ab");
    let topic = topic_input(cx);
    assert!(cx.update(|window, cx| topic.focus_handle(cx).is_focused(window)));

    keys(cx, "ctrl-a ctrl-f");
    cx.simulate_input("x");
    settle(cx);
    assert_eq!(cx.update(|_, cx| topic.text(cx)).as_ref(), "axb");

    keys(cx, "ctrl-e ctrl-f");
    assert!(cx.update(|window, cx| topic.focus_handle(cx).is_focused(window)));
    cx.simulate_input("y");
    settle(cx);
    assert_eq!(cx.update(|_, cx| topic.text(cx)).as_ref(), "axby");
}

/// SPEC_impl.md, "Drafts": while a save is in flight the value on screen is the one sent, and the bound field's tests
/// include "a keystroke while saving". This form binds its fields to its draft itself (the drafts module's
/// `DraftInput` saves one field per request), so the same legs are tested here: an edit taken while saving would be
/// marked saved by the reply although it was never sent. Specified: while the `PUT` is unanswered, typing into the
/// topic leaves the field as sent, the switch and the priority segments take no click, and Save sends nothing more; a
/// keystroke that still reaches the form while saving (here by lifting the field's read-only hold, as an event queued
/// before it took effect would) is refused, and the field shows the topic sent again; after the helm refuses, Save
/// sends the first body again. Which of the read-only hold and the put-back stopped the first keystroke is not told
/// apart here: either leaves the field as sent.
#[gpui::test]
fn nothing_edits_the_form_while_its_save_is_in_flight(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_ntfy(&mut helm, cx, ntfy("https://ntfy.sh", "", "default", false));
    fill(cx, "ntfy-topic", "sent-topic");
    let (id, first) = save_ntfy(&mut helm, cx);
    assert_eq!(preference_work(cx), Some(Work::Save), "the premise: saving");

    click(cx, "ntfy-topic");
    cx.simulate_input("typed");
    test_support::check(cx);
    click(cx, "ntfy-enabled");
    click(cx, "ntfy-priority-Urgent");
    assert_eq!(switch_on(cx, "ntfy-enabled"), Some(false));
    assert_eq!(
        switch_on(cx, "ntfy-priority-Urgent"),
        Some(false),
        "the segment took no click"
    );
    assert_eq!(switch_on(cx, "ntfy-priority-Default"), Some(true));
    reach(cx, FOOTER_SAVE);
    keys(cx, "space");
    assert_eq!(sent(&mut helm), [], "nothing more was sent");
    let topic = topic_input(cx);
    let text = |cx: &mut VisualTestContext| cx.update(|_, cx| topic.text(cx).to_string());
    assert_eq!(
        text(cx),
        "sent-topic",
        "typing while saving left the field as sent"
    );

    cx.update(|_, cx| topic.set_read_only(false, cx));
    click(cx, "ntfy-topic");
    cx.simulate_input("queued");
    settle(cx);
    assert_eq!(
        text(cx),
        "sent-topic",
        "the queued edit was refused and put back"
    );

    helm.reply(id, 503, Some(json!("fixture ntfy save refused")));
    settle(cx);
    let (_, again) = save_ntfy(&mut helm, cx);
    assert_eq!(again, first, "the form is still the one sent");
}

// ---- A press refused while another save holds the key -----------------------------------------------------------------

/// The role and accessible name of the line saying why a press on `control` sent nothing (`<control>-busy`), if drawn.
fn busy_line(cx: &mut VisualTestContext, control: &str) -> Option<(Option<Role>, Option<String>)> {
    let id = format!("{control}-busy");
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id.as_str()))
        .map(|node| (node.role, node.label.map(|label| label.to_string())))
}

/// The line as an alert with the shared words, as `busy_line` reads it.
fn busy_alert() -> Option<(Option<Role>, Option<String>)> {
    Some((Some(Role::Alert), Some(super::Refused::WORDS.to_owned())))
}

/// Why this matters: every Settings save holds the preference row's key and the operation store queues nothing, so a
/// press that meets another save sends nothing; the old app queued such a press, and one that silently does nothing
/// reads as a broken control. Specified: with one switch's save in flight, a click on another switch sends nothing,
/// leaves it showing the stored value and draws the busy line ([`super::Refused::WORDS`], "Not saved: another
/// operation was in progress. Try again.") as an alert under it; a click on Appearance's Dark moves the line under the
/// group, and a click on Terminal's second link choice under that group; back in Sessions, with the other switch
/// drawn, its line is gone; a second click on the saving switch moves the line there; once the save is answered, a
/// click on the other switch sends its save and no line is left.
#[gpui::test]
fn a_press_during_another_settings_save_says_why_it_sent_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    let saving = Choice::SkipSessionConfirmations.id();
    let other = Choice::ColorTerminalFrames.id();
    click(cx, saving);
    let (id, _) = one_save(&mut helm);
    assert_eq!(
        preference_work(cx),
        Some(Work::Save),
        "the premise: in flight"
    );

    click(cx, other);
    assert_eq!(sent(&mut helm), [], "the key held the press back");
    assert_eq!(switch_on(cx, other), Some(false), "the stored value shows");
    assert_eq!(busy_line(cx, other), busy_alert());
    let line = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(format!("{other}-busy").as_str()))
        .map(|node| node.bounds);
    assert!(
        line.is_some_and(|line| line.top() >= bounds(cx, other).bottom()),
        "the line {line:?} is below its switch"
    );

    click(cx, section_button(Section::Appearance));
    click(cx, "settings-appearance-Dark");
    assert_eq!(sent(&mut helm), []);
    assert_eq!(appearance_chosen(cx), Some("System"));
    assert_eq!(busy_line(cx, super::appearance::ID), busy_alert());
    click(cx, section_button(Section::Terminal));
    click(cx, BOTH);
    assert_eq!(sent(&mut helm), []);
    assert_eq!(link_chosen(cx), Some(INTERCEPT), "the stored choice shows");
    assert_eq!(busy_line(cx, super::terminal::ID), busy_alert());
    click(cx, section_button(Section::Sessions));
    assert_eq!(
        switch_on(cx, other),
        Some(false),
        "the premise: the switch is drawn"
    );
    assert_eq!(busy_line(cx, other), None, "only the latest press says it");

    click(cx, saving);
    assert_eq!(sent(&mut helm), [], "its own save is still in flight");
    assert_eq!(busy_line(cx, saving), busy_alert());

    helm.reply(id, 204, None);
    settle(cx);
    click(cx, other);
    let _ = one_save(&mut helm);
    assert_eq!(
        busy_line(cx, saving),
        None,
        "a started save took the line away"
    );
    assert_eq!(busy_line(cx, other), None);
}

/// Why this matters: Checkouts' fields, its spare switch and the ntfy form save under the same preference row's key as
/// every Settings choice, from views of their own, so each must say why its press sent nothing too. Specified: with
/// Notifications and Checkouts read, the spare switch's save in flight, a typed post-clone command's Save and the ntfy
/// form's Save send nothing and each draws the alert under its own control; a second click on the spare switch draws
/// it under the switch; once the spare's save is answered, the ntfy Save sends its `PUT` and its line goes, while the
/// post-clone field's line stays until that field's own save starts.
#[gpui::test]
fn the_fields_the_spare_switch_and_ntfy_say_why_a_press_sent_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_ntfy(&mut helm, cx, ntfy("https://ntfy.sh", "", "default", false));
    click(cx, section_button(Section::Checkouts));
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 3, "the premise: Checkouts read its routes");
    helm.reply(reads[0].0, 200, Some(json!({ "root": null })));
    helm.reply(reads[1].0, 200, Some(json!({ "command": null })));
    helm.reply(reads[2].0, 200, Some(json!({ "enabled": false })));
    settle(cx);

    click(cx, super::checkouts::SPARE_ID);
    let (id, _) = one_put(&mut helm, "/api/checkout-spare");
    assert_eq!(
        preference_work(cx),
        Some(Work::Save),
        "the premise: in flight"
    );
    type_into(cx, Field::PostClone, "make setup");
    click(cx, FOOTER_SAVE);
    assert_eq!(sent(&mut helm), [], "the key held the field back");
    assert_eq!(busy_line(cx, "checkout-post-clone"), busy_alert());
    into_view(cx, super::checkouts::SPARE_ID);
    click(cx, super::checkouts::SPARE_ID);
    assert_eq!(sent(&mut helm), []);
    assert_eq!(busy_line(cx, super::checkouts::SPARE_ID), busy_alert());

    click(cx, section_button(Section::Notifications));
    fill(cx, "ntfy-topic", "draft");
    reach(cx, FOOTER_SAVE);
    keys(cx, "space");
    assert_eq!(sent(&mut helm), [], "the key held the form back");
    assert_eq!(busy_line(cx, "ntfy"), busy_alert());

    cx.simulate_resize(size(px(720.), px(560.)));
    click(cx, section_button(Section::Sessions));
    keys(cx, "escape");
    click(cx, "prompt-confirm");
    assert!(!drawn(cx, "prompt"));
    assert_eq!(sent(&mut helm), []);
    assert!(shows_whole(cx, "ntfy-busy", DIALOG_BODY));

    cx.simulate_resize(size(px(1600.), px(1000.)));
    helm.reply(id, 204, None);
    settle(cx);
    let (id, _) = save_ntfy(&mut helm, cx);
    assert_eq!(busy_line(cx, "ntfy"), None, "its save started");
    helm.reply(id, 200, Some(ntfy("https://ntfy.sh", "", "default", false)));
    settle(cx);
    click(cx, section_button(Section::Checkouts));
    assert_eq!(
        busy_line(cx, "checkout-post-clone"),
        busy_alert(),
        "another view's save leaves the field's line"
    );
}

// ---- The body's foot ------------------------------------------------------------------------------------------------

/// The frame scrolls a long section inside the body, above the pinned footer, so the section's last line must clear
/// the footer once the body is scrolled to its end, or it can never be read whole. Checkouts is the tallest section,
/// taller than a capped dialog's body, and ends with the spare switch's documentation link. Specified: in
/// a window 560 pixels tall, Checkouts shown and its body wheeled far past its end, the link lies wholly inside the
/// body's visible part, whose lower edge is the footer's upper one (the frame's body and footer are sibling rows).
#[gpui::test]
fn the_tallest_sections_last_line_clears_the_footer_scrolled_to_the_end(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    cx.simulate_resize(size(px(720.), px(560.)));
    settle(cx);
    show_checkouts(&mut helm, cx, Value::Null, Value::Null, true);
    let body = bounds(cx, DIALOG_BODY);
    assert!(
        bounds(cx, super::checkouts::SPARE_DOCS_ID).bottom() > body.bottom(),
        "the premise: the link starts below the body's visible part"
    );
    cx.simulate_event(gpui::ScrollEvent {
        position: body.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-5000.))),
        ..Default::default()
    });
    settle(cx);
    assert!(
        shows_whole(cx, super::checkouts::SPARE_DOCS_ID, DIALOG_BODY),
        "the link {:?} is inside the body's visible part {:?}",
        bounds(cx, super::checkouts::SPARE_DOCS_ID),
        bounds(cx, DIALOG_BODY)
    );
}

/// SPEC.md: "Send test uses the saved configuration, even while disabled, and reports success or failure inline", and
/// the old spec's rule: "A test send is a separate request with no form payload. Drafts and both outcomes survive
/// navigation; closing discards drafts". The old form also locked itself while the test was out, so a Save could not
/// overtake it, and showed one outcome at a time. Specified: right after a done save, Send test sends one `POST
/// /api/ntfy-alerts/test` with no body, shows "Working…" and takes the saved line away; meanwhile Save and the switch
/// send and change nothing; the helm's refusal shows in its own words with the alert role; an edit takes that line
/// away; with an unsaved topic typed, a refusal stays after showing Sessions and Notifications again, beside the
/// unsaved topic; a test answered with no body shows "Test alert sent" with the status role, which also stays across
/// that switch; closing Settings with it showing (Discard in the close question) and opening it again shows no line,
/// and Save sends the stored topic, not the unsaved one.
#[gpui::test]
fn send_test_sends_nothing_of_the_form_and_reports_how_it_went(cx: &mut TestAppContext) {
    let refusal = "the ntfy server could not be reached";
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_ntfy(
        &mut helm,
        cx,
        ntfy("https://ntfy.sh", "saved-topic", "default", false),
    );
    let send_test = |helm: &mut FakeHelm, cx: &mut VisualTestContext| -> u64 {
        reach(cx, "ntfy-send-test");
        keys(cx, "space");
        let sent = sent(helm);
        assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
        assert_eq!(
            (sent[0].method, sent[0].path.as_str(), sent[0].body.clone()),
            (Method::Post, "/api/ntfy-alerts/test", None),
            "a test sends nothing of the form"
        );
        sent[0].id
    };
    let role_of = |cx: &mut VisualTestContext, id: &str| {
        frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .and_then(|node| node.role)
    };
    let switch_sections = |cx: &mut VisualTestContext| {
        click(cx, section_button(Section::Sessions));
        click(cx, section_button(Section::Notifications));
    };

    fill(cx, "ntfy-topic", "saved-again");
    let (id, _) = save_ntfy(&mut helm, cx);
    helm.reply(
        id,
        200,
        Some(ntfy("https://ntfy.sh", "saved-again", "default", false)),
    );
    settle(cx);
    assert!(drawn(cx, "ntfy-saved"), "the premise: the saved line shows");

    let id = send_test(&mut helm, cx);
    assert_eq!(label_of(cx, "ntfy-working").as_deref(), Some("Working…"));
    assert!(!drawn(cx, "ntfy-saved"), "a test took the saved line away");
    assert!(!drawn(cx, FOOTER_SAVE), "the saved form has no Save");
    click(cx, "ntfy-enabled");
    assert_eq!(
        sent(&mut helm),
        [],
        "nothing else is sent while the test is out"
    );
    assert_eq!(switch_on(cx, "ntfy-enabled"), Some(false));
    helm.reply(id, 502, Some(json!(refusal)));
    settle(cx);
    assert_eq!(label_of(cx, "ntfy-tested").as_deref(), Some(refusal));
    assert_eq!(role_of(cx, "ntfy-tested"), Some(Role::Alert));
    assert!(!drawn(cx, "ntfy-working"));
    fill(cx, "ntfy-topic", "unsaved-topic");
    assert!(!drawn(cx, "ntfy-tested"), "an edit took the line away");

    let id = send_test(&mut helm, cx);
    helm.reply(id, 502, Some(json!(refusal)));
    settle(cx);
    switch_sections(cx);
    assert_eq!(label_of(cx, "ntfy-tested").as_deref(), Some(refusal));
    let topic = topic_input(cx);
    let text = cx.update(|_, cx| topic.text(cx).to_string());
    assert_eq!(text, "unsaved-topic", "the unsaved topic stayed");

    let id = send_test(&mut helm, cx);
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(
        label_of(cx, "ntfy-tested").as_deref(),
        Some("Test alert sent")
    );
    assert_eq!(role_of(cx, "ntfy-tested"), Some(Role::Status));
    switch_sections(cx);
    assert_eq!(
        label_of(cx, "ntfy-tested").as_deref(),
        Some("Test alert sent"),
        "the success stayed across sections"
    );

    reach(cx, "ntfy-send-test");
    keys(cx, "escape");
    click(cx, "prompt-alternative-0");
    assert!(!any_open(cx), "the premise: Settings closed");
    show_ntfy(
        &mut helm,
        cx,
        ntfy("https://ntfy.sh", "saved-again", "default", false),
    );
    assert!(!drawn(cx, "ntfy-tested"), "closing discarded the line");
    let topic = topic_input(cx);
    assert_eq!(
        cx.update(|_, cx| topic.text(cx).to_string()),
        "saved-again",
        "closing discarded the draft"
    );
}

// ---- Hosts in the store ---------------------------------------------------------------------------------------------

/// The app with the hosts collection holding `listed`, as [`open_rail`] opens it otherwise.
pub(super) fn open_rail_with_hosts<'a>(
    cx: &'a mut TestAppContext,
    listed: &[HostView],
) -> (FakeHelm, &'a mut VisualTestContext, FocusHandle) {
    open_rail_with(
        cx,
        &Preferences::default(),
        vec![crate::hosts::tests::hosts(listed)],
    )
}

/// The helm pushes the hosts collection as holding `listed`, as it does when a host or its Podman check changes.
pub(super) fn push_hosts(helm: &mut FakeHelm, listed: &[HostView], cx: &mut VisualTestContext) {
    let state = crate::hosts::tests::hosts(listed);
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
}

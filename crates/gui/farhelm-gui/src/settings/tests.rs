//! Tests of the Settings dialog over the helm client's fake: how the gear opens it and where focus starts and returns,
//! which sections it lists and what switching between them keeps, what each switch sends and shows while its save is
//! in flight, after a refusal and after the dialog closed.
//!
//! The dialog is opened the way the sidebar's gear will open it: a button in a root view of the test's own, whose click
//! takes a mark and calls [`super::open`] with its own focus handle. Every step ends in one of the shared harness's step
//! helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, KeyedEntry, PREFERENCES_KEY, Preferences,
};
use gpui::{
    Context, FocusHandle, IntoElement, ParentElement as _, Render, Styled as _, TestAppContext,
    Toggled, VisualTestContext, Window, div, px, size,
};
use serde_json::{Value, json};

use super::Section;
use super::choices::Choice;
use crate::components::{Button, HoverHelp, Role};
use crate::focus::{self, Mark};
use crate::operations::{OpKey, Work};
use crate::test_support::{
    self, bounds, click, focused, focused_id, frame, keys, meta, settle, started,
};
use crate::{layers, root, session};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The root view the tests open Settings from: the gear the sidebar will draw on its host rail, as a plain button with
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

/// The section button that shows `section`. The harness finds elements by `'static` ids, so these are spelled out.
fn section_button(section: Section) -> &'static str {
    match section {
        Section::Sessions => "settings-section-sessions",
        Section::Hosts => "settings-section-hosts",
        Section::Terminal => "settings-section-terminal",
    }
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
fn push(helm: &mut FakeHelm, preferences: &Preferences, cx: &mut VisualTestContext) {
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
fn open_rail<'a>(
    cx: &'a mut TestAppContext,
    preferences: &Preferences,
) -> (FakeHelm, &'a mut VisualTestContext, FocusHandle) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![preferences_state(preferences)]);
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
fn open_settings(cx: &mut VisualTestContext) {
    click(cx, "gear");
    assert!(any_open(cx), "the premise: the gear opened Settings");
}

fn any_open(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| layers::any_open(window, cx))
}

/// The commands the window sent since the last call.
fn sent(helm: &mut FakeHelm) -> Vec<FakeCommand> {
    helm.commands()
}

/// The one command sent since the last call, which must be a preference save: its id and its body.
fn one_save(helm: &mut FakeHelm) -> (u64, Value) {
    let sent = sent(helm);
    assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
    assert_eq!(
        (sent[0].method, sent[0].path.as_str()),
        (Method::Put, "/api/preferences")
    );
    (sent[0].id, sent[0].body.clone().unwrap_or_default())
}

/// Whether the switch drawn under `id` shows on; `None` when it is not drawn.
fn switch_on(cx: &mut VisualTestContext, id: &str) -> Option<bool> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| node.toggled == Some(Toggled::True))
}

/// Whether anything is drawn under the element id `id`.
fn drawn(cx: &mut VisualTestContext, id: &str) -> bool {
    frame(cx)
        .into_iter()
        .any(|node| node.element_id.as_deref() == Some(id))
}

/// The work in flight on the preference row.
fn preference_work(cx: &mut VisualTestContext) -> Option<Work> {
    cx.update(|_, cx| {
        session::current_operations(cx)
            .and_then(|operations| operations.read(cx).in_flight(&OpKey::preferences()))
    })
}

/// Every line of text the window painted, joined with single spaces, so a sentence the dialog wrapped over several lines
/// reads as one.
fn painted(cx: &mut VisualTestContext) -> String {
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
fn heading(cx: &mut VisualTestContext) -> Option<String> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("settings-heading"))
        .and_then(|node| node.label.map(|label| label.to_string()))
}

// ---- Opening, the sections, and focus ------------------------------------------------------------------------------

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

/// SPEC.md lists every section, but this build has only Sessions, Hosts and Terminal; listing a section with nothing
/// behind it would offer a button that shows an empty pane. The list follows SPEC.md's order ("Sessions, Appearance,
/// Hosts, Recipes, Tensorlake, Containers, Credentials, Terminal, ..."), and the button of the shown section is the
/// pressed one. Specified: the section list holds exactly the Sessions, Hosts and Terminal buttons, in that order,
/// Sessions pressed and the others not.
#[gpui::test]
fn the_dialog_lists_only_the_sections_this_build_has(cx: &mut TestAppContext) {
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
    let listed: Vec<(String, Option<Toggled>)> = frame(cx)
        .into_iter()
        .filter(|node| {
            node.element_id
                .as_deref()
                .is_some_and(|id| id.starts_with("settings-section-"))
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
            ("Sessions".to_string(), Some(Toggled::True)),
            ("Hosts".to_string(), Some(Toggled::False)),
            ("Terminal".to_string(), Some(Toggled::False)),
        ]
    );
}

/// SPEC.md: "Escape and either close control return focus to the gear", and the old app returned it there on every
/// platform, also on macOS, where a click leaves a button unfocused, so focus would otherwise go back to whatever held it
/// before (the terminal). Run under macOS's click rule for that reason. Specified: with a click that focuses no button,
/// the gear's click leaves the gear unfocused only until Settings opens; Escape, the header's "Close settings" and the
/// footer's Close each close the dialog and leave focus on the gear.
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
        assert!(focused(cx, &gear), "{close} returned focus to the gear");
    }
}

/// The frame must not move when the user switches sections (SPEC.md: "The dialog keeps one height across sections,
/// capped by the window"); a section change that resized it would jump Close away from under the pointer. Specified: in
/// a window 1000 pixels tall, Settings is 760 by 780 pixels and stays exactly where it was, its header close control
/// too, when Hosts is shown; in a window 600 pixels tall it is 48 pixels shorter than the window (the frame's margins).
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
    assert_eq!(bounds(cx, "settings").size.height, px(600. - 48.));
}

/// SPEC.md, "Reaching everything by keyboard": a control Tab reaches must come into view, and in a short window the
/// Sessions section is taller than the body that holds it. Settings relies on the dialog frame's body being one of the
/// focus coordinator's scroll regions rather than scrolling anything itself, so this pins that its switches sit inside
/// that region. Specified: in a window 400 pixels tall, the last Sessions switch starts below the body's visible part;
/// after two Tabs from the first switch it holds focus and lies wholly inside the body's visible part.
#[gpui::test]
fn a_switch_tab_reaches_below_the_fold_scrolls_into_view(cx: &mut TestAppContext) {
    let frames = Choice::ColorTerminalFrames.id();
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    cx.simulate_resize(size(px(1000.), px(400.)));
    settle(cx);
    open_settings(cx);
    let body = bounds(cx, "settings-body");
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
    let (switch, body) = (bounds(cx, frames), bounds(cx, "settings-body"));
    assert!(
        switch.top() >= body.top() && switch.bottom() <= body.bottom(),
        "the switch {switch:?} is inside the body's visible part {body:?}"
    );
}

// ---- Saving the switches -------------------------------------------------------------------------------------------

/// Each switch writes one field of the shared preference row, and a sparse body naming only it keeps a stale copy of
/// another field from being written back over an agent's or another client's change (the helm's `put_preferences`). It
/// is one request under the preference row's key, so the store shows it busy while it runs. Specified: for every switch,
/// in its section, a click sends `PUT /api/preferences` with only its field, set to the opposite of the stored value,
/// while the preference row's key holds a save; once the helm pushes the new row and answers, the switch shows the new
/// value and the key is free.
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
            true,
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

/// SPEC.md: "Name sessions on quick launch is on by default, including when the helm has never stored a choice", while
/// every other switch here reads an unset field as off. Reading naming like the opt-outs would turn naming off on every
/// upgraded helm. Specified: with no preference row, an empty one, and one storing `false`, naming reads on, on and
/// off; every other switch reads off when unset and on when stored `true`.
#[test]
fn name_sessions_on_quick_launch_reads_as_on_when_the_helm_stored_no_choice() {
    let empty = Preferences::default();
    let naming = Choice::NameSessionsOnQuickLaunch;
    assert!(naming.stored(None), "no row");
    assert!(naming.stored(Some(&empty)), "an empty row");
    let off = Preferences {
        name_session_on_quick_launch: Some(false),
        ..Preferences::default()
    };
    assert!(!naming.stored(Some(&off)), "an explicit false");
    let all_on: Preferences = serde_json::from_value(json!({
        "skip_session_confirmations": true,
        "color_terminal_frames": true,
        "auto_update_remote_hosts": true,
        "skip_host_setup_confirmation": true,
        "skip_host_remove_confirmation": true,
    }))
    .unwrap();
    for &choice in Choice::ALL.iter().filter(|&&choice| choice != naming) {
        assert!(
            !choice.stored(None) && !choice.stored(Some(&empty)),
            "{choice:?} reads unset as off"
        );
        assert!(choice.stored(Some(&all_on)), "{choice:?} reads true as on");
    }
}

// ---- Sections keep their state; closing does not stop a save -------------------------------------------------------

/// SPEC.md: "Switching sections preserves unsaved fields, outcome lines and pending saves". A section rebuilt on each
/// switch would drop a pending save's value and a failure line the user has not read yet. It also pins the rule this
/// port has for a click while another preference save is in flight (the module docs): the operation store refuses it,
/// so it sends nothing and changes no switch. Specified: a naming save in flight, Sessions left for Hosts and shown
/// again still shows the pending value; a click on a Hosts switch meanwhile sends nothing and leaves it off; the naming
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
    click(cx, updates);
    assert_eq!(
        sent(&mut helm),
        [],
        "a second preference save is refused while one is in flight"
    );
    assert_eq!(
        switch_on(cx, updates),
        Some(false),
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
/// happens", and the help now promises other clients pick a choice up right away; and "Each switch explains the current
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
/// keep native keys and descriptions"). The tunnel switch waits for the terminal area's consent question and the
/// file-types field for file downloads, so neither may be drawn. Specified: Terminal shows a radio group named
/// "Terminal web links" whose three options are radio buttons in the old order, Intercept chosen when the helm stores
/// none, each risk sentence on screen, and no tunnel switch or file-types field.
#[gpui::test]
fn the_terminal_section_offers_the_link_choices_with_their_risks(cx: &mut TestAppContext) {
    let (_helm, cx, _gear) = open_rail(cx, &Preferences::default());
    open_settings(cx);
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
    assert!(!text.contains("tunnels"), "no tunnel switch yet: {text}");
    assert!(
        !text.contains("File types"),
        "no file-types field yet: {text}"
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

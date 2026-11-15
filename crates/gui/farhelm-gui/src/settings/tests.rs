//! Tests of the Settings dialog over the helm client's fake: how the gear opens it and where focus starts and returns,
//! which sections it lists and what switching between them keeps, what each switch sends and shows while its save is
//! in flight, after a refusal and after the dialog closed.
//!
//! The dialog is opened the way the host rail's gear opens it: a button in a root view of the test's own, whose click
//! takes a mark and calls [`super::open`] with its own focus handle. Every step ends in one of the shared harness's step
//! helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, KeyedEntry, PREFERENCES_KEY, Preferences,
};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, Global, IntoElement, ParentElement as _, Render,
    Styled as _, TestAppContext, Toggled, VisualTestContext, WeakEntity, Window, WindowAppearance,
    div, px, size,
};
use serde_json::{Value, json};

use super::choices::Choice;
use super::{Section, TrustAgent};
use crate::components::{Button, HoverHelp, Role};
use crate::focus::{self, Mark};
use crate::main_view::MainView;
use crate::operations::{OpKey, Work};
use crate::test_support::{
    self, bounds, click, focused, focused_id, frame, keys, meta, settle, started,
};
use crate::{layers, root, session, theme};

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

/// The section button that shows `section`. The harness finds elements by `'static` ids, so these are spelled out.
fn section_button(section: Section) -> &'static str {
    match section {
        Section::Sessions => "settings-section-sessions",
        Section::Hosts => "settings-section-hosts",
        Section::Appearance => "settings-section-appearance",
        Section::Terminal => "settings-section-terminal",
        Section::Checkouts => "settings-section-checkouts",
        Section::AgentTrust => "settings-section-agent-trust",
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

/// SPEC.md lists every section, but this build has only Sessions, Appearance, Hosts, Terminal, Checkouts and Agent
/// trust; listing a section with nothing behind it would offer a button that shows an empty pane. The list follows
/// SPEC.md's order ("Sessions, Appearance, Hosts, Recipes, Tensorlake, Containers, Credentials, Terminal, Checkouts,
/// Agent trust ..."), and the button of the shown section is the pressed one. Specified: the section list holds exactly
/// the Sessions, Appearance, Hosts, Terminal, Checkouts and Agent trust buttons, in that order, Sessions pressed and
/// the others not.
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
            ("Appearance".to_string(), Some(Toggled::False)),
            ("Hosts".to_string(), Some(Toggled::False)),
            ("Terminal".to_string(), Some(Toggled::False)),
            ("Checkouts".to_string(), Some(Toggled::False)),
            ("Agent trust".to_string(), Some(Toggled::False)),
        ]
    );
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
/// every other switch here but the trust hints' reads an unset field as off, and a trust hint's shows the hint unless
/// its dismissal is stored `true`. Reading naming like the opt-outs would turn naming off on every upgraded helm.
/// Specified: with no preference row, an empty one, and one storing `false`, naming reads on, on and off; every other
/// switch but the trust hints' reads off when unset and on when stored `true`; each trust hint's switch reads on when
/// unset or stored `false`, and off when stored `true`.
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
    let hints = [
        Choice::ShowClaudeTrustHint,
        Choice::ShowCodexTrustHint,
        Choice::ShowMuseTrustHint,
    ];
    for &choice in Choice::ALL
        .iter()
        .filter(|&&choice| choice != naming && !hints.contains(&choice))
    {
        assert!(
            !choice.stored(None) && !choice.stored(Some(&empty)),
            "{choice:?} reads unset as off"
        );
        assert!(choice.stored(Some(&all_on)), "{choice:?} reads true as on");
    }
    // A hint's switch is its dismissal inverted: "Unset and false both allow the hint".
    let dismissed = |value: bool| -> Preferences {
        serde_json::from_value(json!({
            "claude_trust_hint_dismissed": value,
            "codex_trust_hint_dismissed": value,
            "muse_trust_hint_dismissed": value,
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

/// Another client's choice (or one made through the helm's API) reaches the open window as it happens, as every shared
/// preference does, and the group reads it from the store rather than a copy taken at opening. A copy would only show
/// on a push that comes with no save of the dialog's own. Specified: with Appearance open showing System, the helm
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

/// SPEC.md: the choice is a helm preference "and a change reaches every open client live". The main view's
/// subscription hands the stored row to the theme; a palette that followed only this window's own saves would leave
/// other clients, the first snapshot and a reconnect's new snapshot unpainted. It also records what the window shows
/// before the first snapshot, which is a deviation from the old app (the parity file): the connecting screen is in the
/// operating system's palette. Specified: on a light system, the connecting window's palette is light; the first
/// snapshot storing Dark turns it dark; a push storing Light, with nothing sent, turns it light; a push storing nothing
/// (System) after a stored Dark leaves the system's light; after a reset the inert window keeps the palette in use
/// (Dark), and the reconnect's snapshot storing Light turns it light.
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
            window_size: None,
            preference: Some(theme::Preference::Dark),
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
fn sent_reads(helm: &mut FakeHelm) -> Vec<(u64, String)> {
    sent(helm)
        .into_iter()
        .map(|command| {
            assert_eq!(command.method, Method::Get, "only reads: {command:?}");
            (command.id, command.path)
        })
        .collect()
}

/// The four trust routes, in the section's order.
const TRUST_PATHS: [&str; 4] = [
    "/api/checkout-claude-trust",
    "/api/checkout-agent-trust/codex",
    "/api/checkout-agent-trust/muse",
    "/api/checkout-agent-trust/pi",
];

/// Answer `reads` (in the section's order) with each agent's trust as `enabled`, the hint not dismissed.
fn answer_trust<const N: usize>(
    helm: &mut FakeHelm,
    reads: &[(u64, String)],
    enabled: [bool; N],
    cx: &mut VisualTestContext,
) {
    for ((id, _), enabled) in reads.iter().zip(enabled) {
        helm.reply(
            *id,
            200,
            Some(json!({ "enabled": enabled, "hint_dismissed": false })),
        );
    }
    settle(cx);
}

/// Open Settings and show Agent trust, returning the reads that sent.
fn show_agent_trust(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<(u64, String)> {
    open_settings(cx);
    assert_eq!(sent(helm), [], "the premise: Sessions reads nothing");
    click(cx, "settings-section-agent-trust");
    assert_eq!(heading(cx).as_deref(), Some("Agent trust"));
    sent_reads(helm)
}

/// The trust switches live behind routes the helm does not push, so the section reads them once when it is shown, and
/// SPEC.md: "Failed settings reads never count as an off switch": a switch must not offer a toggle over a choice it
/// does not know. Specified: showing Agent trust sends one GET per agent, in the section's order; until they answer,
/// a click on a switch sends nothing; once answered (Claude on, the others off), each switch shows its agent's choice,
/// and showing another section and Agent trust again reads nothing more.
#[gpui::test]
fn agent_trust_reads_each_agent_once_when_shown_and_waits_for_the_answer(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let reads = show_agent_trust(&mut helm, cx);
    let paths: Vec<&str> = reads.iter().map(|(_, path)| path.as_str()).collect();
    assert_eq!(paths, TRUST_PATHS);
    click(cx, TrustAgent::Claude.id());
    assert_eq!(sent(&mut helm), [], "an unknown choice takes no toggle");

    answer_trust(&mut helm, &reads, [true, false, false, false], cx);
    let shown: Vec<Option<bool>> = TrustAgent::ALL
        .iter()
        .map(|agent| switch_on(cx, agent.id()))
        .collect();
    assert_eq!(shown, [Some(true), Some(false), Some(false), Some(false)]);
    click(cx, "settings-section-hosts");
    click(cx, "settings-section-agent-trust");
    assert_eq!(sent(&mut helm), [], "read once per opening");
}

/// Open Settings at Agent trust for `agent`, as its trust hint does, answer the reads with every agent's trust off, and
/// check that `agent`'s switch then holds focus. The trust hint's way in is also the test's way to a switch below the
/// dialog's fold, which a click cannot reach.
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
    answer_trust(helm, &reads, [false; 4], cx);
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
/// row's key and shows on while it is in flight, then stays on once the helm answers; in a later opening Pi's sends
/// `{"enabled": true}` to its route and, refused, shows off again with "Could not save or read Pi trust settings:" and
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
    open_trust_at(&mut helm, cx, &gear, TrustAgent::Pi);
    keys(cx, "space");
    let saves = sent(&mut helm);
    assert_eq!(
        (saves[0].path.as_str(), saves[0].body.clone()),
        (TRUST_PATHS[3], Some(json!({ "enabled": true })))
    );
    helm.reply(saves[0].id, 503, Some(json!("fixture trust save refused")));
    settle(cx);
    assert_eq!(switch_on(cx, TrustAgent::Pi.id()), Some(false), "restored");
    assert_eq!(
        trust_failure(cx, TrustAgent::Pi).as_deref(),
        Some("Could not save or read Pi trust settings: fixture trust save refused")
    );
}

/// A failed read must leave the switch unknown, not off, and say why. Specified: Claude's read refused, its switch
/// takes no click and its block shows "Could not save or read Claude trust settings:" with the helm's words, while
/// Codex's switch, read fine, shows its choice.
#[gpui::test]
fn a_failed_trust_read_leaves_the_switch_unavailable_with_its_reason(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let reads = show_agent_trust(&mut helm, cx);
    helm.reply(reads[0].0, 503, Some(json!("fixture trust read refused")));
    answer_trust(&mut helm, &reads[1..], [true; 3], cx);
    click(cx, TrustAgent::Claude.id());
    assert_eq!(sent(&mut helm), [], "no toggle over an unknown choice");
    assert_eq!(
        trust_failure(cx, TrustAgent::Claude).as_deref(),
        Some("Could not save or read Claude trust settings: fixture trust read refused")
    );
    assert_eq!(switch_on(cx, TrustAgent::Codex.id()), Some(true));
}

/// A read answers only the opening it was sent for: a slow reply from an earlier opening must not fill in a newer one,
/// which has its own read in flight. Each opening is a dialog of its own, so this guards against trust state shared
/// between openings (a cache), not against how the reply holds its dialog. Specified: Agent trust shown, Settings
/// closed before the reads answer and opened at Agent trust again, which reads again; answering the first opening's
/// reads (all on) leaves the new switches unknown and off; answering the new ones shows them.
#[gpui::test]
fn a_trust_read_reaches_only_the_opening_it_was_sent_for(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let first = show_agent_trust(&mut helm, cx);
    keys(cx, "escape");
    let second = show_agent_trust(&mut helm, cx);
    assert_eq!(second.len(), 4, "the new opening reads again");
    answer_trust(&mut helm, &first, [true; 4], cx);
    assert_eq!(
        switch_on(cx, TrustAgent::Claude.id()),
        Some(false),
        "the old answer reached nobody"
    );
    click(cx, TrustAgent::Claude.id());
    assert_eq!(sent(&mut helm), [], "and the switch is still unknown");
    answer_trust(&mut helm, &second, [true; 4], cx);
    assert_eq!(switch_on(cx, TrustAgent::Claude.id()), Some(true));
}

/// SPEC.md: a hint's Don't show again "stores a shared dismissal, reversible beside the switch". The hint switches are
/// that dismissal inverted, in the shared preference row the band reads. Specified: with nothing stored, Claude's,
/// Codex's and Muse's hint switches show on and Pi has none; Codex's, reached by Tab from Codex's trust switch and
/// turned off with Space, sends only `{"codex_trust_hint_dismissed": true}` to `PUT /api/preferences`; a pushed row
/// dismissing Muse's shows it off.
#[gpui::test]
fn the_hint_switches_are_the_shared_dismissals_inverted(cx: &mut TestAppContext) {
    let (mut helm, cx, gear) = open_rail(cx, &Preferences::default());
    open_trust_at(&mut helm, cx, &gear, TrustAgent::Codex);
    for hint in [
        Choice::ShowClaudeTrustHint,
        Choice::ShowCodexTrustHint,
        Choice::ShowMuseTrustHint,
    ] {
        assert_eq!(
            switch_on(cx, hint.id()),
            Some(true),
            "{hint:?} shows the hint"
        );
    }
    assert!(!drawn(cx, "settings-pi-trust-hint"), "Pi has no hint");
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
    push(
        &mut helm,
        &Preferences {
            muse_trust_hint_dismissed: Some(true),
            ..Preferences::default()
        },
        cx,
    );
    assert_eq!(switch_on(cx, Choice::ShowMuseTrustHint.id()), Some(false));
}

/// A trust hint's "Settings and caveats" opens Agent trust at its agent's switch (SPEC.md: "Trust hints instead open
/// agent trust at the relevant switch"), but the switch cannot hold focus until its read answers. Pi's read is
/// answered last, so a handover keyed on the wrong agent would focus an earlier switch and take focus off the dialog,
/// and Pi's switch would never get it. The switch is not scrolled into view yet (a recorded deviation), so this checks
/// focus only. Closing returns focus to the hint's button, which Settings names as its return target, as the gear's
/// entry point does. Specified: opened for Pi from a button that does not hold focus, Settings shows Agent trust with
/// the dialog itself holding focus; once the reads answer, Pi's switch holds focus; Escape then closes Settings and
/// focus goes to the button.
#[gpui::test]
fn the_trust_hint_entry_point_focuses_its_agents_switch_once_known(cx: &mut TestAppContext) {
    let (mut helm, cx, gear) = open_rail(cx, &Preferences::default());
    assert!(
        !focused(cx, &gear),
        "the premise: the opening button does not hold focus"
    );
    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        super::open_at_agent_trust(TrustAgent::Pi, &gear, &mark, window, cx);
    });
    settle(cx);
    assert_eq!(heading(cx).as_deref(), Some("Agent trust"));
    assert!(
        dialog_holds_focus(cx),
        "the dialog itself holds focus while the reads are out"
    );
    let reads = sent_reads(&mut helm);
    answer_trust(&mut helm, &reads, [false; 4], cx);
    assert_eq!(focused_id(cx).as_deref(), Some(TrustAgent::Pi.id()));
    keys(cx, "escape");
    assert!(!any_open(cx), "the premise: Escape closed Settings");
    assert!(
        focused(cx, &gear),
        "closing returned focus to the opening button, which nothing focused before Settings opened"
    );
}

/// Whether the switch drawn under `id` is available, which a disabled switch shows by taking no focus; `None` when it
/// is not drawn.
fn switch_available(cx: &mut VisualTestContext, id: &str) -> Option<bool> {
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
    answer_trust(&mut helm, &reads, [false; 4], cx);
    assert_eq!(
        switch_available(cx, TrustAgent::Codex.id()),
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
    answer_trust(&mut helm, &reads, [false; 4], cx);
    assert_eq!(
        switch_available(cx, TrustAgent::Codex.id()),
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

/// The Settings dialog opened last, for the tests that read a checkout field's text: GPUI Kit's field reports no
/// accessible value, so the frame cannot tell it. Set by `open_dialog` in test builds only.
pub(super) struct Opened(pub(super) WeakEntity<super::SettingsDialog>);

impl Global for Opened {}

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

    fn save(self) -> &'static str {
        match self {
            Field::Folder => "checkout-folder-save",
            Field::PostClone => "checkout-post-clone-save",
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
        let dialog = cx
            .try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade());
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
fn show_checkouts(
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
fn label_of(cx: &mut VisualTestContext, id: &str) -> Option<String> {
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
    click(cx, Field::Folder.save());
    click(cx, Field::PostClone.save());
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
    click(cx, Field::Folder.save());
    click(cx, Field::PostClone.save());
    assert_eq!(sent(&mut helm), [], "an untouched Save writes nothing");
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
    click(cx, Field::Folder.save());
    let (id, body) = one_put(&mut helm, Field::Folder.path());
    assert_eq!(body, json!({ "root": "~/edited" }));
    assert_eq!(preference_work(cx), Some(Work::Save));
    assert_eq!(
        label_of(cx, Field::Folder.save()).as_deref(),
        Some("Saving…")
    );
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
    click(cx, Field::Folder.save());
    let (_, body) = one_put(&mut helm, Field::Folder.path());
    assert_eq!(body, json!({ "root": null }), "an empty folder clears it");
}

/// SPEC.md: "Settings ... shows a refused value's reason", and a refused draft "stays editable"; "Switching sections
/// preserves unsaved fields, outcome lines and pending saves". The helm validates the folder's shape only, so its
/// refusal is the only check the user gets. Specified: a typed relative folder refused with the helm's words keeps its
/// text, shows those words as an alert, reads nothing back, and keeps both through showing Sessions and Checkouts
/// again; Save then sends the draft again.
#[gpui::test]
fn a_refused_folder_keeps_its_draft_and_reason_across_sections(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_checkouts(&mut helm, cx, Value::Null, Value::Null, true);
    type_into(cx, Field::Folder, "relative-folder");
    click(cx, Field::Folder.save());
    let (id, _) = one_put(&mut helm, Field::Folder.path());
    let reason = "fixture: not the relative path 'relative-folder'";
    helm.reply(id, 400, Some(json!(reason)));
    settle(cx);
    assert_eq!(sent(&mut helm), [], "a refusal reads nothing back");
    for _ in 0..2 {
        assert_eq!(text(cx, Field::Folder), "relative-folder");
        assert_eq!(
            label_of(cx, "checkout-folder-error").as_deref(),
            Some(reason)
        );
        click(cx, section_button(Section::Sessions));
        click(cx, section_button(Section::Checkouts));
    }
    click(cx, Field::Folder.save());
    let (_, body) = one_put(&mut helm, Field::Folder.path());
    assert_eq!(body, json!({ "root": "relative-folder" }));
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
    click(cx, Field::PostClone.save());
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
    click(cx, Field::PostClone.save());
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
    click(cx, Field::PostClone.save());
    let (_, body) = one_put(&mut helm, Field::PostClone.path());
    let sent = body["command"].as_str().unwrap_or_default().to_owned();
    assert!(sent.contains(" # edited"), "{sent:?}");
    assert_eq!(sent.replacen(" # edited", "", 1), stored);
}

/// SPEC_impl.md, "Work that finishes later": an explicit save is the operation store's work and outlives the dialog
/// (the old "a pending checkout folder save survives closing settings"). Specified: a folder save in flight when Escape
/// closes Settings still holds the preference row's key; the helm's reply frees it with nothing more sent, since no
/// field is left to read the route back for.
#[gpui::test]
fn a_folder_save_in_flight_goes_on_after_the_dialog_closes(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_checkouts(&mut helm, cx, Value::Null, Value::Null, true);
    type_into(cx, Field::Folder, "~/survives-close");
    click(cx, Field::Folder.save());
    let (id, _) = one_put(&mut helm, Field::Folder.path());
    keys(cx, "escape");
    assert!(!any_open(cx), "the premise: Settings closed");
    assert_eq!(preference_work(cx), Some(Work::Save));
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(preference_work(cx), None);
    assert_eq!(sent(&mut helm), []);
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
    /// How many times the field raised [`super::Saved`], as the launcher's refresh would see it.
    saved: usize,
    _saved: gpui::Subscription,
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
/// "/srv/own" and a default of "~/def" it shows the override and offers Use default; Use default sends
/// `{"root": null}` to that route under the host's key; the done reply reads the route back, and the answer (no
/// override) empties the field, shows "Default: ~/def" as its placeholder and takes Use default away. The field raises
/// `Saved` once for that done save (the launcher's hook to refresh what depends on the folder), and not for a later
/// save the helm refuses.
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
        let saved = cx.subscribe(&field, |host: &mut HostFolder, _, _: &super::Saved, _| {
            host.saved += 1;
        });
        HostFolder {
            root: root::attach(window, cx),
            field,
            saved: 0,
            _saved: saved,
        }
    });
    let saved = |cx: &mut VisualTestContext| cx.update(|_, cx| view.read(cx).saved);
    let path = "/api/hosts/2/checkout-root";
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1, "one read: {reads:?}");
    assert_eq!(reads[0].1, path);
    helm.reply(
        reads[0].0,
        200,
        Some(json!({ "root": "/srv/own", "default": "~/def" })),
    );
    settle(cx);
    let shown =
        |cx: &mut VisualTestContext| cx.update(|_, cx| view.read(cx).field.read(cx).text(cx));
    assert_eq!(shown(cx), "/srv/own");

    click(cx, "checkout-folder-use-default");
    let (id, body) = one_put(&mut helm, path);
    assert_eq!(body, json!({ "root": null }));
    let host_work = cx.update(|_, cx| {
        session::current_operations(cx)
            .and_then(|operations| operations.read(cx).in_flight(&OpKey::host("2")))
    });
    assert_eq!(host_work, Some(Work::Save));
    assert_eq!(preference_work(cx), None, "not the preference row's key");
    assert_eq!(saved(cx), 0, "nothing saved yet");
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(saved(cx), 1, "the done save raised Saved");
    let reads = sent_reads(&mut helm);
    assert_eq!(reads.len(), 1, "one read back: {reads:?}");
    helm.reply(
        reads[0].0,
        200,
        Some(json!({ "root": null, "default": "~/def" })),
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
    let (id, _) = one_put(&mut helm, path);
    helm.reply(id, 400, Some(json!("fixture refusal")));
    settle(cx);
    assert_eq!(saved(cx), 1, "a refused save raises nothing");
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
    assert!(!drawn(cx, Field::Folder.save()), "Retry takes Save's place");
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
    assert!(drawn(cx, Field::Folder.save()));
    type_into(cx, Field::Folder, "/more");
    assert_eq!(text(cx, Field::Folder), "~/stored/more", "editable now");
}

/// SPEC.md: "Switching sections preserves unsaved fields", and closing discards them (the dialog's module docs). A
/// draft that a section switch dropped would lose what the user typed; one that outlived the dialog would show text the
/// helm does not hold. Specified: a command typed but neither saved nor refused is still there after showing Sessions
/// and Checkouts again, with nothing sent; after Escape and a new opening, the field shows the stored command.
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
    show_checkouts(&mut helm, cx, Value::Null, json!("stored"), true);
    assert_eq!(text(cx, Field::PostClone), "stored");
}

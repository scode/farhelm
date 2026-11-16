//! Tests of the rail's notices bell and its panel: what the bell says, what the panel shows, the read mark every close
//! sends, Clear resolved, and the trash cleanup notice's Retry now and Open Trash.
//!
//! The pure rules (unread, the bell's words and ink, the order) are tested as functions. Everything a user sees or the
//! helm receives is tested in the real main view over the helm client's fake, with two connected hosts whose trash the
//! Trash button reads on its own; the fixtures here answer those reads. Controls are found by their element ids in
//! GPUI's driver record of the frame.

use farhelm_helm_client::collections::decode_changes;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::notices::Notice;
use farhelm_proto::ui_channel::{Collection, CollectionChanges, CollectionState, KeyedEntry};
use gpui::{DriverNode, Modifiers, TestAppContext, VisualTestContext, point, px, size};
use serde_json::{Value, json};

use super::{
    BELL_ID, CLEAR_ID, CLEAR_REFUSAL_ID, CLOSE_ID, COUNT_ID, OPEN_TRASH_ID_PREFIX, PANEL_ID,
    RETRY_ID_PREFIX, bell_help, bell_ink, bell_label, is_unread, newest_first,
};
use crate::hosts::tests::hosts;
use crate::local_time;
use crate::session_list::tests::{listing, open_main, painted};
use crate::test_support::{self, any_open, focused_id, frame, meta};
use crate::theme;
use crate::trash::tests::{checkout, local, remote};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// The trash cleanup notice `id` about checkout `checkout` on host `host`, raised `raised` seconds after a fixed
/// instant, as the helm serves it.
fn notice(id: i64, host: i64, checkout: &str, raised: i64, read: bool, resolved: bool) -> Value {
    json!({
        "id": id,
        "host": host,
        "raised_at": 1_794_591_000 + raised,
        "read": read,
        "resolved": resolved,
        "content": {
            "kind": "stuck_trash",
            "checkout_id": checkout,
            "name": format!("{checkout}-20261113T174055Z"),
            "repository": "acme/web",
            "bytes": 4096,
            "reason": "Permission denied",
            "at": 1_794_591_900,
        },
    })
}

/// The notices collection holding `notices`, in the order given.
fn notices(notices: &[Value]) -> CollectionState {
    let keys: Vec<String> = notices
        .iter()
        .map(|notice| notice["id"].to_string())
        .collect();
    let order: Vec<&str> = keys.iter().map(String::as_str).collect();
    CollectionState {
        collection: Collection::Notices,
        meta: meta(&order),
        entries: notices
            .iter()
            .map(|notice| KeyedEntry {
                key: notice["id"].to_string(),
                value: notice.clone(),
            })
            .collect(),
    }
}

/// The main view over an empty fleet on the local host and `build-box`, with `held` as the helm's notices, its first
/// trash reads answered: `build-box` (host 2) lists checkout `c1` stuck, and the local host nothing.
fn open<'a>(cx: &'a mut TestAppContext, held: &[Value]) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut fake, cx) = open_main(
        cx,
        vec![
            listing(&[], &[]),
            hosts(&[local(), remote()]),
            notices(held),
        ],
    );
    answer_trash(&mut fake, cx, true);
    (fake, cx)
}

/// Answer every pending command: each trash read with `build-box`'s checkout `c1` (stuck when `stuck`) or the local
/// host's empty trash, and anything else but a notice mark with an empty success. Returns the marks, unanswered.
fn answer_trash(fake: &mut FakeHelm, cx: &mut VisualTestContext, stuck: bool) -> Vec<(u64, Value)> {
    let mut marks = Vec::new();
    for command in fake.commands() {
        let body = command.body.clone().unwrap_or_default();
        if command.path == "/api/notices/marks" {
            marks.push((command.id, body));
            continue;
        }
        let reply = if command.path == "/api/checkout-trash/list" {
            let mut entry = checkout("c1", Some(4096));
            if stuck {
                entry["stuck"] = json!({ "reason": "Permission denied", "at": 1_794_591_900 });
            }
            let listed = if body["host"] == json!(2) {
                vec![entry]
            } else {
                Vec::new()
            };
            Some(crate::trash::tests::listing(&listed, false))
        } else {
            None
        };
        fake.reply(command.id, 200, reply);
    }
    test_support::settle(cx);
    marks
}

/// The notice marks sent since the last call, as their bodies, each answered with `status`.
fn marks(fake: &mut FakeHelm, cx: &mut VisualTestContext, status: u16) -> Vec<Value> {
    let marks = answer_trash(fake, cx, true);
    for (id, _) in &marks {
        fake.reply(*id, status, None);
    }
    test_support::settle(cx);
    marks.into_iter().map(|(_, body)| body).collect()
}

/// The node drawn under `id`, if any.
fn node<'a>(nodes: &'a [DriverNode], id: &str) -> Option<&'a DriverNode> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
}

/// Open the panel with a click on the bell, checking that it opened.
fn open_panel(cx: &mut VisualTestContext) {
    test_support::click(cx, BELL_ID);
    assert!(
        node(&frame(cx), PANEL_ID).is_some(),
        "the click opened the panel"
    );
}

/// Whether the control drawn under `id` is drawn unavailable: an unavailable button tracks no focus (as the Trash
/// dialog's tests tell one apart).
fn disabled(cx: &mut VisualTestContext, id: &str) -> bool {
    !node(&frame(cx), id)
        .unwrap_or_else(|| panic!("{id} is drawn"))
        .flags
        .focusable
}

/// A decoded notice from its wire form.
fn decoded(value: Value) -> Notice {
    serde_json::from_value(value).unwrap()
}

// ---- What the bell says ----------------------------------------------------------------------------------------------

/// SPEC.md: "Unread items make it filled red with an unread count; its accessible name reports that count too", and
/// resolved items "are quiet even if never opened". Counting a resolved item would keep the bell loud for a problem
/// that went away. Specified: a notice is unread only while unread and unresolved; the name is "Notifications: N
/// unread" and the help adds what the bell opens; the ink is the strong red while loud, the rail's quiet ink
/// otherwise, and the inverse ink on the open look's fill; the panel's order is newest raised first, then highest id.
#[test]
fn the_bell_counts_unread_unresolved_notices_and_the_panel_is_newest_first() {
    assert!(is_unread(&decoded(notice(1, 2, "c1", 0, false, false))));
    assert!(!is_unread(&decoded(notice(1, 2, "c1", 0, true, false))));
    assert!(!is_unread(&decoded(notice(1, 2, "c1", 0, false, true))));
    assert_eq!(bell_label(2), "Notifications: 2 unread");
    assert_eq!(
        bell_help(0),
        "Notifications: 0 unread; open notices about your hosts and Farhelm"
    );
    let palette = &theme::DARK;
    assert_eq!(bell_ink(palette, true, false), palette.danger_strong);
    assert_eq!(bell_ink(palette, false, false), palette.fg_1);
    assert_eq!(bell_ink(palette, true, true), palette.on_fill);
    let held: Vec<Notice> = [
        notice(1, 2, "a", 10, false, false),
        notice(3, 2, "b", 20, false, false),
        notice(2, 2, "c", 20, false, false),
    ]
    .into_iter()
    .map(decoded)
    .collect();
    let order: Vec<i64> = newest_first(held.iter().collect())
        .iter()
        .map(|notice| notice.id)
        .collect();
    assert_eq!(order, [3, 2, 1]);
}

/// The bell on the rail is the user's only sign of a host problem outside the Trash, so its name must say the count
/// a sighted user reads off the red. Specified: with one unread and one resolved notice, the bell is named
/// "Notifications: 1 unread" and draws its count; once the helm pushes the unread one as read, it is named
/// "Notifications: 0 unread" and draws no count.
#[gpui::test]
fn the_bell_reports_its_unread_count_in_its_name_and_beside_the_glyph(cx: &mut TestAppContext) {
    let held = [
        notice(1, 2, "c1", 0, false, false),
        notice(2, 1, "c2", 10, true, true),
    ];
    let (mut fake, cx) = open(cx, &held);
    let nodes = frame(cx);
    assert_eq!(
        node(&nodes, BELL_ID).and_then(|bell| bell.label.as_deref()),
        Some("Notifications: 1 unread")
    );
    assert!(node(&nodes, COUNT_ID).is_some(), "the count is drawn");
    let quiet = notices(&[notice(1, 2, "c1", 0, true, false), held[1].clone()]);
    fake.push(CollectionChanges {
        collection: Collection::Notices,
        meta: quiet.meta,
        upsert: quiet.entries,
        remove: Vec::new(),
    });
    test_support::settle(cx);
    let nodes = frame(cx);
    assert_eq!(
        node(&nodes, BELL_ID).and_then(|bell| bell.label.as_deref()),
        Some("Notifications: 0 unread")
    );
    assert!(
        node(&nodes, COUNT_ID).is_none(),
        "no count while nothing is unread"
    );
}

// ---- The panel -------------------------------------------------------------------------------------------------------

/// SPEC.md: "Clicking opens newest-first history beside the bell", and the trash cleanup notice "names the host and
/// checkout, shows how much space was last measured when known and a short reason, and explains that Farhelm tries
/// again when that host's supervisor restarts"; resolved items "stay greyed in the panel". Specified: a click on the
/// bell opens the panel named "Notifications" beside it (its left edge past the bell's right), headed "Notifications"
/// over "About your hosts and Farhelm"; the newer notice comes first; an unresolved notice paints its host's name with
/// ": trash could not be freed", the checkout's name, its repository and "4.0 KiB last measured", the reason, "Last
/// failed" with the failure's minute in the viewer's local time (the app's zone pinned three hours west of UTC here,
/// with no zone named) and the promise of a retry, with Retry now and Open Trash; the resolved one paints "Resolved"
/// and offers Open Trash but no Retry now.
#[gpui::test]
fn the_panel_lists_notices_newest_first_beside_the_bell(cx: &mut TestAppContext) {
    let held = [
        notice(1, 2, "c1", 20, false, false),
        notice(2, 1, "c2", 10, true, true),
    ];
    let (_fake, cx) = open(cx, &held);
    cx.simulate_resize(size(px(1200.), px(800.)));
    cx.update(|_, cx| local_time::pin(-3 * 3_600, cx));
    test_support::settle(cx);
    open_panel(cx);
    let nodes = frame(cx);
    let panel = node(&nodes, PANEL_ID).unwrap();
    assert_eq!(panel.label.as_deref(), Some("Notifications"));
    let bell = node(&nodes, BELL_ID).unwrap().bounds;
    assert!(panel.bounds.left() > bell.right(), "beside the bell");
    let top = |id: &str| {
        node(&nodes, id)
            .unwrap_or_else(|| panic!("{id}"))
            .bounds
            .top()
    };
    assert!(top("rail-notice-1") < top("rail-notice-2"), "newest first");
    let texts = painted(&nodes);
    for expected in [
        "Notifications",
        "About your hosts and Farhelm",
        "Permission denied",
        "Resolved",
        "Farhelm tries again when this host\u{2019}s supervisor restarts. You can also retry now.",
    ] {
        assert!(
            texts.iter().any(|text| text == expected),
            "{expected}: {texts:?}"
        );
    }
    let label =
        |id: &str| node(&nodes, id).and_then(|node| node.label.as_deref().map(str::to_owned));
    assert_eq!(
        label("rail-notice-1-title").as_deref(),
        Some("build-box: trash could not be freed")
    );
    assert_eq!(
        label("rail-notice-1-name").as_deref(),
        Some("Farhelm could not finish removing c1-20261113T174055Z.")
    );
    assert_eq!(
        label("rail-notice-1-checkout").as_deref(),
        Some("acme/web · 4.0 KiB last measured")
    );
    assert_eq!(
        label("rail-notice-1-time").as_deref(),
        Some("Last failed 2026-11-13 14:45")
    );
    assert!(node(&nodes, &format!("{RETRY_ID_PREFIX}1")).is_some());
    assert!(
        node(&nodes, &format!("{RETRY_ID_PREFIX}2")).is_none(),
        "no Retry for a resolved notice"
    );
    assert!(node(&nodes, &format!("{OPEN_TRASH_ID_PREFIX}2")).is_some());
}

/// The old panel said so when there was nothing to show, rather than drawing an empty box. Specified: with no notices
/// the bell is quiet and the panel paints "No notifications" between its header and its footer.
#[gpui::test]
fn an_empty_panel_says_there_are_no_notifications(cx: &mut TestAppContext) {
    let (_fake, cx) = open(cx, &[]);
    open_panel(cx);
    assert!(painted(&frame(cx)).contains(&"No notifications".to_owned()));
}

// ---- Closing marks what the panel showed read ------------------------------------------------------------------------

/// SPEC.md: "Escape, clicking outside, clicking the bell again or leaving the panel with keyboard focus closes it and
/// marks only the items it showed as read." A close path that forgot the mark would leave the bell loud after the
/// user read it, and a mark of an item never shown would hide it. Specified: with notices 1 and 3 unread and 2
/// resolved, each of Escape, the bell again, Close, a press elsewhere in the window, and Tab out past the panel's last
/// control closes the panel and sends one `POST /api/notices/marks` with `{"action": "read", "ids": [3, 1]}`, never
/// the resolved one (the helm's answer is left without a push, so each close sends again); Escape, the bell and Close
/// leave focus on the bell, and Tab out of the panel leaves it on Hosts, the control after the bell.
#[gpui::test]
fn every_close_path_marks_only_the_unread_notices_shown(cx: &mut TestAppContext) {
    let held = [
        notice(1, 2, "c1", 10, false, false),
        notice(2, 1, "c2", 20, true, true),
        notice(3, 2, "c3", 30, false, false),
    ];
    let (mut fake, cx) = open(cx, &held);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    cx.simulate_resize(size(px(1200.), px(800.)));
    test_support::settle(cx);
    let read = json!({ "action": "read", "ids": [3, 1] });
    type Close = fn(&mut VisualTestContext);
    let paths: [(&str, Close, Option<&str>); 5] = [
        (
            "Escape",
            |cx| test_support::keys(cx, "escape"),
            Some(BELL_ID),
        ),
        (
            "the bell",
            |cx| test_support::click(cx, BELL_ID),
            Some(BELL_ID),
        ),
        (
            "Close",
            |cx| test_support::click(cx, CLOSE_ID),
            Some(BELL_ID),
        ),
        (
            "a press elsewhere",
            |cx| {
                // The window's bottom right corner: the empty fleet's pane, clear of the panel, which a tall panel stretches
                // up the window's left side.
                cx.simulate_click(point(px(1150.), px(780.)), Modifiers::none());
                test_support::check(cx);
            },
            None,
        ),
        (
            "Tab out",
            |cx| {
                // From the panel: Open Trash on 3 (its Retry now is drawn but unavailable, since the host does not list
                // c3 stuck, so it is no Tab stop), Open Trash on 2, Retry now and Open Trash on 1, Clear resolved,
                // Close; the seventh Tab goes on out to Hosts.
                for _ in 0..7 {
                    test_support::keys(cx, "tab");
                }
            },
            Some("hosts-panel-toggle"),
        ),
    ];
    for (path, close, focus) in paths {
        open_panel(cx);
        assert!(
            marks(&mut fake, cx, 200).is_empty(),
            "{path}: opening sends nothing"
        );
        close(cx);
        test_support::settle(cx);
        assert!(!any_open(cx), "{path} closed the panel");
        assert_eq!(
            marks(&mut fake, cx, 200),
            std::slice::from_ref(&read),
            "{path}"
        );
        if let Some(focus) = focus {
            assert_eq!(focused_id(cx).as_deref(), Some(focus), "{path}");
        }
    }
}

/// What the close marks is what the user was shown, never what merely arrived: a notice the store holds when the
/// panel closes, but that no frame has drawn, must stay unread. GPUI's test mode draws a window as soon as a change is
/// applied, so the change and the close go in one update, which no frame interrupts. Specified: with notice 1 unread
/// and shown, notice 2 reaching the store and the panel closing in the same update sends a mark of `[1]` alone, and the
/// bell, with no read mark pushed yet, counts both unread.
#[gpui::test]
fn the_close_marks_only_what_a_frame_drew(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, &[notice(1, 2, "c1", 10, false, false)]);
    open_panel(cx);
    let both = notices(&[
        notice(2, 2, "c2", 20, false, false),
        notice(1, 2, "c1", 10, false, false),
    ]);
    let changes = decode_changes(CollectionChanges {
        collection: Collection::Notices,
        meta: both.meta,
        upsert: both.entries,
        remove: Vec::new(),
    })
    .unwrap();
    cx.update(|window, cx| {
        let store = crate::session::current_store(cx).expect("a UI session");
        store.update(cx, |store, cx| store.apply(&changes, cx));
        assert!(
            crate::layers::dismiss_top(window, cx),
            "premise: the panel was open"
        );
    });
    test_support::settle(cx);
    assert_eq!(
        marks(&mut fake, cx, 200),
        [json!({ "action": "read", "ids": [1] })]
    );
    assert_eq!(
        test_support::label(cx, BELL_ID).as_deref(),
        Some("Notifications: 2 unread"),
        "the helm has pushed no read mark yet"
    );
}

/// The old reader skipped a mark with nothing to acknowledge. Specified: with the only notice already read, opening and
/// closing the panel sends no mark.
#[gpui::test]
fn closing_with_nothing_unread_shown_sends_nothing(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, &[notice(1, 2, "c1", 10, true, false)]);
    open_panel(cx);
    test_support::keys(cx, "escape");
    assert!(
        marks(&mut fake, cx, 200).is_empty(),
        "nothing unread was shown"
    );
}

// ---- Clear resolved --------------------------------------------------------------------------------------------------

/// SPEC.md: resolved items "can be removed with **Clear resolved**", and a failed clear must surface. Specified: Clear
/// resolved, with notices 2 and 4 resolved and 1 unresolved, sends
/// `{"action": "clear_resolved", "ids": [4, 2]}` alone; a refusal "notices locked" shows in the panel, which stays
/// open.
#[gpui::test]
fn clear_resolved_removes_only_the_resolved_notices_shown(cx: &mut TestAppContext) {
    let held = [
        notice(1, 2, "c1", 10, false, false),
        notice(2, 1, "c2", 20, true, true),
        notice(4, 1, "c4", 40, true, true),
    ];
    let (mut fake, cx) = open(cx, &held);
    open_panel(cx);
    assert!(!disabled(cx, CLEAR_ID));
    test_support::click(cx, CLEAR_ID);
    let sent = answer_trash(&mut fake, cx, true);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(
        sent[0].1,
        json!({ "action": "clear_resolved", "ids": [4, 2] })
    );
    fake.reply(sent[0].0, 409, Some(json!("notices locked")));
    test_support::settle(cx);
    assert!(any_open(cx), "the panel stays open");
    assert_eq!(
        test_support::label(cx, CLEAR_REFUSAL_ID).as_deref(),
        Some("notices locked")
    );
}

/// Clear resolved removes resolved notices only, so with none shown it has nothing to do and is drawn unavailable, as
/// the old button was. Specified: with one unresolved notice, Clear resolved is unavailable.
#[gpui::test]
fn clear_resolved_is_unavailable_with_nothing_resolved(cx: &mut TestAppContext) {
    let (_fake, cx) = open(cx, &[notice(1, 2, "c1", 10, false, false)]);
    open_panel(cx);
    assert!(disabled(cx, CLEAR_ID), "nothing resolved to clear");
}

// ---- The trash cleanup notice's actions ------------------------------------------------------------------------------

/// SPEC.md: Retry now "asks for one attempt through the existing cleanup checks; it is unavailable without a current
/// connected host listing showing that same checkout stuck, or while another guarded operation prevents it". A Retry
/// that went out past those guards could act on a checkout the host no longer has. Specified: with `build-box`'s
/// listing showing `c1` stuck, Retry now is available and sends one trash delete of `["c1"]` on host 2's connection,
/// and is unavailable while that delete is in flight; once the host lists `c1` without a failure, it stays unavailable.
#[gpui::test]
fn retry_now_follows_the_trashs_guards(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, &[notice(1, 2, "c1", 10, false, false)]);
    open_panel(cx);
    let retry = "rail-notice-retry-1";
    assert_eq!(retry, format!("{RETRY_ID_PREFIX}1"));
    assert!(!disabled(cx, retry), "the listing shows c1 stuck");
    test_support::click(cx, retry);
    let deletes: Vec<_> = fake
        .commands()
        .into_iter()
        .filter(|command| command.path == "/api/checkout-trash/delete")
        .collect();
    assert_eq!(deletes.len(), 1, "{deletes:?}");
    assert_eq!(
        deletes[0].body.clone().unwrap_or_default(),
        json!({ "host": 2, "expected_incarnation": 1, "ids": ["c1"] })
    );
    test_support::settle(cx);
    assert!(
        disabled(cx, retry),
        "unavailable while the delete is in flight"
    );
    fake.reply(
        deletes[0].id,
        200,
        Some(json!({ "removed": ["c1"], "issues": [] })),
    );
    test_support::settle(cx);
    answer_trash(&mut fake, cx, false);
    assert!(disabled(cx, retry), "the listing no longer shows c1 stuck");
}

/// SPEC.md: "**Open Trash** opens the existing dialog, including while host management is visible ..., and closing
/// that dialog returns focus to the visible rail bell." Specified: with the Hosts panel shown, Open Trash closes the
/// notices panel (sending its read mark) and opens the Trash dialog with focus on its footer Close; Escape closes the
/// dialog with focus on the bell.
#[gpui::test]
fn open_trash_opens_the_dialog_and_closing_it_returns_to_the_bell(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, &[notice(1, 2, "c1", 10, false, false)]);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::click(cx, "hosts-panel-toggle");
    assert!(
        node(&frame(cx), "hosts-panel").is_some(),
        "premise: host management shows"
    );
    open_panel(cx);
    let open_trash = "rail-notice-open-trash-1";
    assert_eq!(open_trash, format!("{OPEN_TRASH_ID_PREFIX}1"));
    test_support::click(cx, open_trash);
    assert!(
        node(&frame(cx), PANEL_ID).is_none(),
        "the notices panel closed"
    );
    assert!(
        node(&frame(cx), "checkout-trash-dialog").is_some(),
        "the Trash dialog opened"
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("trash-close"),
        "on its footer Close"
    );
    assert_eq!(
        marks(&mut fake, cx, 200),
        [json!({ "action": "read", "ids": [1] })]
    );
    test_support::keys(cx, "escape");
    assert!(!any_open(cx), "Escape closed the Trash dialog");
    assert_eq!(focused_id(cx).as_deref(), Some(BELL_ID));
}

/// The old case "rail close marks only shown notices and preserves independent focus": a press on another control
/// while the panel is open is both an outside press, which closes the panel and marks what it showed, and a press on
/// that control, which must still take effect. Specified: with the panel open over one unread notice, a click on Hosts
/// closes the panel, sends the read mark of `[1]`, and shows host management.
#[gpui::test]
fn a_press_on_hosts_closes_the_panel_marks_it_and_shows_host_management(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, &[notice(1, 2, "c1", 10, false, false)]);
    assert!(
        node(&frame(cx), "hosts-panel").is_none(),
        "premise: the session list shows"
    );
    open_panel(cx);
    test_support::click(cx, "hosts-panel-toggle");
    assert!(
        node(&frame(cx), PANEL_ID).is_none(),
        "the press closed the panel"
    );
    assert_eq!(
        marks(&mut fake, cx, 200),
        [json!({ "action": "read", "ids": [1] })]
    );
    assert!(
        node(&frame(cx), "hosts-panel").is_some(),
        "and showed host management"
    );
}

//! Tests of a session row's notification bell and its list: what the bell says and how it looks, where the list opens
//! and what it shows, the read mark every close sends, Clear all, and the docs-link rule.
//!
//! The rules that are pure (the unread count, the bell's words and inks, the docs-link recogniser) are tested as
//! functions. Everything a user sees or the helm receives is tested in the real main view over the helm client's fake,
//! which hands the test every command the window sent and answers only when told; the session list's own fixtures build
//! the sessions. Controls are found by their element ids in GPUI's driver record of the frame, and links are checked
//! through GPUI's test platform, which records the address the app asked it to open, so nothing reaches a browser.

use farhelm_helm_client::collections::decode_changes;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::SessionNotification;
use farhelm_proto::ui_channel::{CollectionState, SessionRow};
use gpui::{DriverNode, Modifiers, Role, TestAppContext, VisualTestContext, point, px, size};
use serde_json::{Value, json};

use super::{
    CLEAR_ID, CLEAR_REFUSAL_ID, DOCS_LINK_PREFIX, LIST_ID, TextRun, bell_help, bell_hover_ink,
    bell_icon, bell_ink, bell_label, is_unread, newest_first, notification_runs, unread,
};
use crate::assets::Icon;
use crate::host_text::HostText;
use crate::host_text::tests::SPOOFING_CORPUS;
use crate::local_time;
use crate::session_list::tests::{
    Fixture, TEST_NOW, change, listing, listing_meta, open_main, painted, preferences, selected,
    session,
};
use crate::test_support::{self, any_open, focused_id, frame};
use crate::theme;

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// The bell of row `a`, the row every window test gives notifications.
const BELL_A: &str = "session-bell-a";

/// One notification in the helm's wire form: sequence number `seq`, recorded `ago` seconds before the test clock's
/// start, saying `text`.
fn note(seq: u64, ago: i64, text: &str) -> Value {
    json!({ "seq": seq, "at": TEST_NOW - ago, "text": text })
}

/// [`note`], resolved.
fn resolved(seq: u64, ago: i64, text: &str) -> Value {
    json!({ "seq": seq, "at": TEST_NOW - ago, "text": text, "resolved": true })
}

/// Session `a` with `notifications` (a JSON array) under the helm's read mark `read_through`.
fn belled(notifications: Value, read_through: u64) -> Fixture {
    session("a").notifications(notifications, read_through)
}

/// A window with `a` above a plain `b`, and `b` open (the visit history names it), so a click that opened `a` would
/// show as a change of selection.
fn snapshot(a: Fixture) -> Vec<CollectionState> {
    vec![
        listing(&["a", "b"], &[a, session("b")]),
        preferences(&["b"], None),
    ]
}

/// Push `a` as the helm would after a change to it, in the same order beside `b`.
fn push_a(fake: &mut FakeHelm, a: Fixture) {
    fake.push(change(listing_meta(&["a", "b"], 2, false), &[a], &[]));
}

/// The node drawn under `id`, if any.
fn node<'a>(nodes: &'a [DriverNode], id: &str) -> Option<&'a DriverNode> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
}

/// Every command the window sent since the last call that is about notifications, as (envelope id, path, body). The
/// others (opening a session sends its own, the remembered selection and the seen mark) are answered with an empty
/// success and dropped, so they neither pile up nor hide a notification command.
fn notification_commands(fake: &mut FakeHelm) -> Vec<(u64, String, Value)> {
    let mut found = Vec::new();
    for command in fake.commands() {
        if command.path.contains("/notifications/") {
            found.push((command.id, command.path, command.body.unwrap_or_default()));
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    found
}

/// A way the user closes the open list, as a step of a test.
type ClosePath = fn(&mut VisualTestContext);

/// The read mark through `through` on `a`, as (path, body).
fn read_mark(through: u64) -> (String, Value) {
    (
        "/api/sessions/a/notifications/read".into(),
        json!({ "through": through }),
    )
}

/// The notification commands sent since the last call, without their envelope ids, each answered with `status`.
fn answered(fake: &mut FakeHelm, cx: &mut VisualTestContext, status: u16) -> Vec<(String, Value)> {
    let sent = notification_commands(fake);
    for (id, _, _) in &sent {
        fake.reply(*id, status, None);
    }
    test_support::settle(cx);
    sent.into_iter()
        .map(|(_, path, body)| (path, body))
        .collect()
}

/// Open `a`'s list with a click on its bell, checking that it opened.
fn open_list(cx: &mut VisualTestContext) {
    test_support::click(cx, BELL_A);
    assert!(
        node(&frame(cx), LIST_ID).is_some(),
        "the click on the bell opened its list"
    );
}

/// The accessible name of `a`'s bell in a fresh frame, or `None` when no bell is drawn.
fn bell_name(cx: &mut VisualTestContext) -> Option<String> {
    test_support::label(cx, BELL_A)
}

/// A notification decoded from its wire form.
fn decoded(value: Value) -> SessionNotification {
    serde_json::from_value(value).unwrap()
}

// ---- What the bell says ----------------------------------------------------------------------------------------------

/// SPEC.md: the bell is "unmistakable when something is unread", and resolved entries "count as read even if the user
/// has never opened them". Counting a resolved entry would keep a bell loud for a problem that went away. Specified: an
/// entry is unread when it is unresolved and above the read mark; at the mark or below, or resolved above it, it is
/// read; a row's count is its unread entries.
#[test]
fn only_unresolved_entries_above_the_read_mark_are_unread() {
    assert!(is_unread(&decoded(note(5, 0, "x")), 4));
    assert!(!is_unread(&decoded(note(4, 0, "x")), 4));
    assert!(!is_unread(&decoded(resolved(9, 0, "x")), 4));
    let mut entry = session("a")
        .notifications(
            json!([
                note(6, 0, "x"),
                resolved(5, 0, "y"),
                note(4, 0, "z"),
                note(3, 0, "w")
            ]),
            3,
        )
        .entry();
    let row: SessionRow = serde_json::from_value(entry.value.take()).unwrap();
    assert_eq!(unread(&row), 2);
}

/// SPEC.md: the bell's "accessible name says how many are unread", since a screen reader cannot hear the red, and the
/// session's title is a host value, which must reach the accessible name escaped. Specified: the name is
/// "Notifications for <title>: N unread", or "none unread", with a hidden character in the title escaped; the hover
/// help gives the same count, then what a click does.
#[test]
fn the_bells_name_and_help_count_the_unread_and_escape_the_title() {
    let title = HostText::from_host("build\u{200B} \u{202E}lanif");
    let name = bell_label(&title, 2);
    assert_eq!(
        name,
        "Notifications for build<U+200B> <U+202E>lanif: 2 unread"
    );
    assert!(!name.contains('\u{202E}') && !name.contains('\u{200B}'));
    assert_eq!(
        bell_label(&HostText::from_host("plain"), 0),
        "Notifications for plain: none unread"
    );
    assert_eq!(bell_help(1), "Notifications: 1 unread; click to list them");
    assert_eq!(
        bell_help(0),
        "Notifications: none unread; click to list them"
    );
}

/// The bell's look carries the same news as its name: the old sheet drew an unread bell filled and in the strong red
/// (filled so it stays obvious without the red), a read one grey, an open one in the primary ink, and every bell on
/// the selected row in the inverse ink, red or not. Specified, in the dark palette: loud is the filled drawing in
/// `--danger-strong`; quiet is the outline in `--fg-2`, `--fg-0` under the pointer or while open; on the selected row
/// both are `--on-fill` and the pointer changes nothing; a loud bell does not brighten under the pointer either.
#[test]
fn the_bells_ink_and_drawing_follow_unread_selection_and_open() {
    let palette = &theme::DARK;
    assert_eq!(bell_icon(true), Icon::BellFilled);
    assert_eq!(bell_icon(false), Icon::Bell);
    assert_eq!(bell_ink(palette, true, false, false), palette.danger_strong);
    assert_eq!(bell_ink(palette, true, false, true), palette.danger_strong);
    assert_eq!(bell_ink(palette, false, false, false), palette.fg_2);
    assert_eq!(bell_ink(palette, false, false, true), palette.fg_0);
    assert_eq!(bell_ink(palette, true, true, false), palette.on_fill);
    assert_eq!(bell_ink(palette, false, true, true), palette.on_fill);
    assert_eq!(bell_hover_ink(palette, false, false), Some(palette.fg_0));
    assert_eq!(bell_hover_ink(palette, true, false), None);
    assert_eq!(bell_hover_ink(palette, false, true), None);
}

/// SPEC.md: "The list is newest first". The helm sends them so, but the list must not depend on arrival order.
/// Specified: entries come back by sequence number, highest first.
#[test]
fn the_list_is_newest_first_whatever_order_arrives() {
    let entries: Vec<SessionNotification> = [note(2, 0, "b"), note(9, 0, "c"), note(5, 0, "a")]
        .into_iter()
        .map(decoded)
        .collect();
    let order: Vec<u64> = newest_first(&entries)
        .iter()
        .map(|entry| entry.seq)
        .collect();
    assert_eq!(order, [9, 5, 2]);
}

// ---- The docs-link rule ----------------------------------------------------------------------------------------------

/// SPEC.md, "Status": in notification text, a link to a page of Farhelm's docs is cut out as a link, ending at
/// whitespace and without trailing sentence punctuation; the text around it stays plain. The notice that Farhelm ended
/// a session's old terminal ends with the docs page to read, and a user who cannot click it has to copy an address out
/// of a small list. Ported from the old app's test of the same name.
#[test]
fn a_docs_link_in_notification_text_is_cut_out() {
    use TextRun::{DocsLink, Plain};
    assert_eq!(
        notification_runs("See https://farhelm.io/docs/using/new-terminals/"),
        vec![
            Plain("See "),
            DocsLink("https://farhelm.io/docs/using/new-terminals/")
        ]
    );
    assert_eq!(
        notification_runs(
            "Read https://farhelm.io/docs/a/#b. Then https://farhelm.io/docs/c..d/?e=1&f=2), done"
        ),
        vec![
            Plain("Read "),
            DocsLink("https://farhelm.io/docs/a/#b"),
            Plain(". Then "),
            DocsLink("https://farhelm.io/docs/c..d/?e=1&f=2"),
            Plain("), done"),
        ]
    );
    assert_eq!(
        notification_runs(DOCS_LINK_PREFIX),
        vec![DocsLink(DOCS_LINK_PREFIX)]
    );
}

/// SPEC.md, "Status": "any other address stays plain text, since a host's supervisor must not be able to put a link to
/// anywhere it likes in front of the user". Notification text comes from a host, which the app does not trust, so a
/// clickable link must never lead anywhere but Farhelm's docs, and an odd-looking one costs only its clickability.
/// Specified: another site, another part of farhelm.io, plain http, look-alike hosts (`farhelm.io.evil`,
/// `farhelm.io.example.com`) and paths (`/docsx`), an address whose remainder holds a quote, angle bracket, backslash,
/// percent sign, `@` or invisible character, and a path with a `.` or `..` segment, literal or percent-encoded, all
/// stay plain text, whole; text without any link, empty text included, is one plain run; a refused candidate does not
/// hide a good link after it; and every link recognised spells exactly as host text's escaping would draw it, so the
/// link's unescaped words cannot show anything the escaping would have exposed.
#[test]
fn nothing_but_a_plain_docs_link_is_ever_a_link() {
    use TextRun::{DocsLink, Plain};
    for text in [
        "Go to https://example.com/docs/x now",
        "https://farhelm.io/blog/x",
        "http://farhelm.io/docs/x",
        "https://farhelm.io.evil/docs/x",
        "https://farhelm.io.example.com/docs/x",
        "https://farhelm.io/docsx",
        "https://farhelm.io/docs",
        "https://farhelm.io/docs/x\"y",
        "https://farhelm.io/docs/<b>",
        "https://farhelm.io/docs/x\\..\\y",
        "https://farhelm.io/docs/x\u{202E}y",
        "https://farhelm.io/docs/x\u{200B}y",
        "https://farhelm.io/docs/@evil.example",
        "https://farhelm.io/docs/../api/feedback",
        "https://farhelm.io/docs/a/./b",
        "https://farhelm.io/docs/%2e%2e/api/feedback",
        "https://farhelm.io/docs/.%2E/api/feedback",
        "",
    ] {
        assert_eq!(notification_runs(text), vec![Plain(text)], "{text:?}");
    }
    // Trailing dots are sentence punctuation and are trimmed before the check, so a trailing `..` leaves a link that
    // stays in the docs.
    assert_eq!(
        notification_runs("https://farhelm.io/docs/a/.."),
        vec![DocsLink("https://farhelm.io/docs/a/"), Plain("..")]
    );
    assert_eq!(
        notification_runs("https://farhelm.io/docs/<x> https://farhelm.io/docs/ok"),
        vec![
            Plain("https://farhelm.io/docs/<x> "),
            DocsLink("https://farhelm.io/docs/ok"),
        ]
    );
    for text in [
        "https://farhelm.io/docs/a-b_c~d/e.f?g=h&i+j#k",
        "see https://farhelm.io/docs/using/new-terminals/.",
    ] {
        for run in notification_runs(text) {
            if let DocsLink(url) = run {
                assert_eq!(farhelm_proto::text::escaped(url), url, "{url}");
            }
        }
    }
}

// ---- The bell on the row ---------------------------------------------------------------------------------------------

/// SPEC.md: "A session with notifications shows a bell on its sidebar row; a row without notifications shows none",
/// placed after the agent marks and before the last activity time, grey once everything is read. A bell on every row,
/// or one that pushed the age out of its column, would be noise. Specified: `a`, with two unread, has a button named
/// "Notifications for a: 2 unread" whose hover help gives the count, wholly between its agent track and its age and on
/// the age's line; `b`, with none, has no bell; once the helm pushes a read mark past both, the name says none unread.
#[gpui::test]
fn only_a_row_with_notifications_draws_a_bell_before_its_age(cx: &mut TestAppContext) {
    let notes = json!([note(2, 60, "newer problem"), note(1, 600, "older problem")]);
    let (mut fake, cx) = open_main(cx, snapshot(belled(notes.clone(), 0)));
    test_support::settle(cx);
    let nodes = frame(cx);
    let bell = node(&nodes, BELL_A).expect("a's bell is drawn");
    assert_eq!(bell.role, Some(Role::Button));
    assert_eq!(bell.label.as_deref(), Some("Notifications for a: 2 unread"));
    assert!(node(&nodes, "session-bell-b").is_none(), "b has no bell");
    let agent = node(&nodes, "session-row-a-agent")
        .expect("a's agent track")
        .bounds;
    let age = node(&nodes, "session-row-a-age").expect("a's age").bounds;
    assert!(bell.bounds.left() >= agent.right(), "after the agent marks");
    assert!(bell.bounds.right() <= age.left(), "before the age");
    let middle = |bounds: gpui::Bounds<gpui::Pixels>| bounds.center().y;
    assert!(
        (middle(bell.bounds) - middle(age)).abs() <= px(2.),
        "on the age's line"
    );
    let at = test_support::center(cx, BELL_A);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    test_support::advance(cx, crate::layers::HOVER_DELAY);
    assert_eq!(
        cx.update(|window, cx| crate::layers::shown_tooltip(window, cx))
            .map(|shown| shown.text.to_string())
            .as_deref(),
        Some("Notifications: 2 unread; click to list them")
    );
    push_a(&mut fake, belled(notes, 2));
    test_support::settle(cx);
    assert_eq!(
        bell_name(cx).as_deref(),
        Some("Notifications for a: none unread")
    );
}

// ---- Opening the list ------------------------------------------------------------------------------------------------

/// SPEC.md: clicking the bell "opens the session's notifications in a list in the style of the row's actions menu,
/// never clipped by the sidebar, and does not open the session". Opening the session on a bell click would yank the
/// user away from the session they were in. Specified: with `b` open, a click on `a`'s bell opens the list, named
/// "Notifications for a" and headed "Notifications", beside the bell (its left edge past the bell's right) and reaching
/// past the row's right edge, which is the sidebar's; `b` stays open and nothing is sent; focus is in the list but not
/// on Clear all; Escape closes it with focus back on the bell, also under the macOS click rule, and Enter on the bell
/// opens it again, still without opening `a`.
#[gpui::test]
fn the_bell_opens_its_list_beside_it_without_opening_the_session(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(belled(json!([note(1, 60, "one")]), 1)));
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    cx.simulate_resize(size(px(1200.), px(700.)));
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("b"), "premise: b is open");
    notification_commands(&mut fake);
    open_list(cx);
    let nodes = frame(cx);
    let list = node(&nodes, LIST_ID).expect("the list is drawn");
    assert_eq!(list.label.as_deref(), Some("Notifications for a"));
    assert!(painted(&nodes).contains(&"Notifications".to_owned()));
    let bell = node(&nodes, BELL_A).unwrap().bounds;
    let row = node(&nodes, "session-item-a").expect("a's row").bounds;
    assert!(list.bounds.left() > bell.right(), "beside the bell");
    assert!(
        list.bounds.right() > row.right(),
        "past the sidebar, not clipped by it"
    );
    assert_eq!(
        selected(cx).as_deref(),
        Some("b"),
        "the click opened nothing"
    );
    assert!(notification_commands(&mut fake).is_empty());
    let focused = focused_id(cx);
    assert!(
        cx.update(|window, cx| crate::layers::focus_layer_open(window, cx)),
        "focus is in the list"
    );
    assert_ne!(
        focused.as_deref(),
        Some(CLEAR_ID),
        "a stray Enter clears nothing"
    );
    test_support::keys(cx, "escape");
    assert!(!any_open(cx), "Escape closed the list");
    assert_eq!(focused_id(cx).as_deref(), Some(BELL_A));
    test_support::keys(cx, "enter");
    assert!(
        node(&frame(cx), LIST_ID).is_some(),
        "Enter on the bell opened it"
    );
    assert_eq!(selected(cx).as_deref(), Some("b"));
}

/// SPEC.md: "The list is newest first, each entry saying how long ago it happened, with the full time available on the
/// entry the way the row's activity age has it, and the entries new since the list was last opened set apart", and
/// resolved entries "remain visible, greyed and marked 'Resolved'", never new. Specified: under a read mark of 4, with
/// entries 4 (an hour ago), 5 (two minutes ago, unresolved) and 6 (resolved), the list draws 6, 5, 4 top to bottom; the
/// word New appears once and Resolved once; entry 5's age reads "2m ago" and is named by its full stamp in the viewer's
/// local time (two hours east of UTC here, which moves it into the next day); the bell counts one unread, the resolved
/// entry not among them.
#[gpui::test]
fn entries_are_newest_first_with_new_ones_set_apart_and_resolved_greyed(cx: &mut TestAppContext) {
    let notes = json!([
        note(4, 3_600, "already seen"),
        note(5, 120, "just happened"),
        resolved(6, 60, "went away"),
    ]);
    let (_fake, cx) = open_main(cx, snapshot(belled(notes, 4)));
    test_support::settle(cx);
    assert_eq!(
        bell_name(cx).as_deref(),
        Some("Notifications for a: 1 unread")
    );
    cx.update(|_, cx| local_time::pin(2 * 3_600, cx));
    open_list(cx);
    let nodes = frame(cx);
    let top = |seq: u64| {
        node(&nodes, &format!("notification-entry-{seq}"))
            .unwrap_or_else(|| panic!("entry {seq} is drawn"))
            .bounds
            .top()
    };
    assert!(top(6) < top(5) && top(5) < top(4), "newest first");
    let texts = painted(&nodes);
    assert_eq!(texts.iter().filter(|text| *text == "New").count(), 1);
    assert_eq!(texts.iter().filter(|text| *text == "Resolved").count(), 1);
    let age = node(&nodes, "notification-age-5").expect("entry 5's age");
    assert!(texts.contains(&"2m ago".to_owned()), "{texts:?}");
    assert_eq!(
        age.label.as_deref(),
        Some("Last activity 2025-07-02 01:57:58")
    );
}

/// SPEC_impl.md, "Host-supplied text": notification text is written by a host's supervisor, so it is drawn escaped,
/// like every host value, and a look-alike docs address is no link. Specified: each spoofing string in an entry's text
/// paints as its escaped form, with no link drawn; `https://farhelm.io.evil/docs/x` paints as plain text with no link.
#[gpui::test]
fn entry_text_is_host_text_and_a_look_alike_is_no_link(cx: &mut TestAppContext) {
    let mut notes: Vec<Value> = SPOOFING_CORPUS
        .iter()
        .enumerate()
        .map(|(at, (raw, _))| note(at as u64 + 1, 60, raw))
        .collect();
    notes.push(note(100, 60, "https://farhelm.io.evil/docs/x"));
    let (_fake, cx) = open_main(cx, snapshot(belled(Value::Array(notes), 1_000)));
    test_support::settle(cx);
    open_list(cx);
    let nodes = frame(cx);
    let texts = painted(&nodes);
    for (raw, shown) in SPOOFING_CORPUS {
        assert!(
            texts.iter().any(|text| text == shown),
            "{raw:?} paints as {shown:?}"
        );
    }
    assert!(
        texts
            .iter()
            .any(|text| text == "https://farhelm.io.evil/docs/x")
    );
    assert!(
        nodes.iter().all(|node| node.role != Some(Role::Link)),
        "no link is drawn"
    );
}

/// SPEC.md: a link to a page of Farhelm's docs "is clickable and opens the way the app's other external links do; any
/// other address stays plain text". Specified: in "Not <foreign> but see <docs>", exactly one link is drawn, named by
/// the docs address; the foreign address is painted as plain text; a click on the link asks the app's opener for
/// exactly the docs address and leaves the list open.
#[gpui::test]
fn only_a_docs_link_is_clickable_and_it_opens_through_the_apps_opener(cx: &mut TestAppContext) {
    let docs = "https://farhelm.io/docs/using/new-terminals/";
    let foreign = "https://example.com/docs/using/new-terminals/";
    let text = format!("Not {foreign} but see {docs}");
    let (_fake, cx) = open_main(cx, snapshot(belled(json!([note(1, 60, &text)]), 0)));
    test_support::settle(cx);
    open_list(cx);
    let nodes = frame(cx);
    let links: Vec<&DriverNode> = nodes
        .iter()
        .filter(|node| node.role == Some(Role::Link))
        .collect();
    assert_eq!(links.len(), 1, "one link");
    assert_eq!(links[0].label.as_deref(), Some(docs));
    assert!(
        painted(&nodes)
            .iter()
            .any(|painted| painted.contains(foreign)),
        "the foreign address is plain text"
    );
    test_support::click(cx, "notification-link-1-1");
    assert_eq!(cx.opened_url().as_deref(), Some(docs));
    assert!(any_open(cx), "the list stays open");
}

// ---- Closing marks what it showed read -------------------------------------------------------------------------------

/// SPEC.md: "Closing the list, by clicking away, pressing Escape, or clicking the bell again, marks everything it
/// showed as read", and an overlay also closes when its row goes. A close path that forgot the mark would leave a bell
/// loud after the user read it. Specified: with entries 5 (unread) and 4 under a mark of 4, each of Escape, a second
/// click on the bell and a press elsewhere in the window closes the list and sends exactly one
/// `PUT /api/sessions/a/notifications/read` with `{"through": 5}` (the helm's answer is left without a push, so each
/// close sends again); and the row leaving the list while the list is open closes it and sends the same mark.
#[gpui::test]
fn every_close_path_marks_what_the_list_showed_read(cx: &mut TestAppContext) {
    let notes = json!([
        note(5, 120, "just happened"),
        note(4, 3_600, "already seen")
    ]);
    let (mut fake, cx) = open_main(cx, snapshot(belled(notes, 4)));
    cx.simulate_resize(size(px(1200.), px(700.)));
    test_support::settle(cx);
    notification_commands(&mut fake);
    let paths: [(&str, ClosePath); 3] = [
        ("Escape", |cx| test_support::keys(cx, "escape")),
        ("the bell again", |cx| test_support::click(cx, BELL_A)),
        ("a press elsewhere", |cx| {
            cx.simulate_click(point(px(1100.), px(650.)), Modifiers::none());
            test_support::check(cx);
        }),
    ];
    for (path, close) in paths {
        open_list(cx);
        assert!(
            notification_commands(&mut fake).is_empty(),
            "{path}: opening sends nothing"
        );
        close(cx);
        test_support::settle(cx);
        assert!(!any_open(cx), "{path} closed the list");
        assert_eq!(answered(&mut fake, cx, 200), [read_mark(5)], "{path}");
    }
    open_list(cx);
    fake.push(change(listing_meta(&["b"], 1, false), &[], &["a"]));
    test_support::settle(cx);
    assert!(!any_open(cx), "the row's going closed the list");
    assert_eq!(answered(&mut fake, cx, 200), [read_mark(5)]);
}

/// SPEC.md's old acceptance case: closing a list that showed nothing unread has nothing to mark and sends nothing; the
/// old app skipped it too. Specified: with every entry at or below the read mark, opening and closing the list sends no
/// notification command.
#[gpui::test]
fn closing_a_list_with_nothing_unread_sends_nothing(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(belled(json!([note(3, 60, "old")]), 3)));
    test_support::settle(cx);
    notification_commands(&mut fake);
    open_list(cx);
    test_support::keys(cx, "escape");
    test_support::settle(cx);
    assert!(notification_commands(&mut fake).is_empty());
}

/// The list draws from the store, so a push while it is open shows at once; what the close marks is what the user was
/// shown, never what merely arrived. An entry the store already holds when the list closes, but that no frame has drawn
/// yet, must stay unread, or the user would never learn of it. GPUI's test mode draws a window as soon as a change is
/// applied, so the second half applies the change to the store and closes the list in one update, which no frame
/// interrupts: the list's release, which sends the mark, runs as that update's effects are flushed, ahead of the next
/// frame. Specified: with entry 1 unread, a push adding entry 2 while the list is open draws it, and Escape then marks
/// through 2; after the helm's push of that mark, with the list open again, entry 3 reaching the store and the list
/// closing in the same update sends nothing (the last frame showed nothing above the mark of 2), and the bell then
/// counts one unread. A close that marked through the store's newest entry instead would send a mark through 3.
#[gpui::test]
fn a_push_is_marked_read_only_once_the_list_has_shown_it(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(belled(json!([note(1, 60, "first")]), 0)));
    test_support::settle(cx);
    notification_commands(&mut fake);
    open_list(cx);
    push_a(
        &mut fake,
        belled(json!([note(2, 30, "second"), note(1, 60, "first")]), 0),
    );
    test_support::settle(cx);
    assert!(
        node(&frame(cx), "notification-entry-2").is_some(),
        "the open list shows the pushed entry"
    );
    test_support::keys(cx, "escape");
    assert_eq!(answered(&mut fake, cx, 200), [read_mark(2)]);
    let two = json!([note(2, 30, "second"), note(1, 60, "first")]);
    push_a(&mut fake, belled(two, 2));
    test_support::settle(cx);
    assert_eq!(
        bell_name(cx).as_deref(),
        Some("Notifications for a: none unread")
    );
    open_list(cx);
    let three = json!([
        note(3, 10, "third"),
        note(2, 30, "second"),
        note(1, 60, "first")
    ]);
    let changes = decode_changes(change(
        listing_meta(&["a", "b"], 2, false),
        &[belled(three, 2)],
        &[],
    ))
    .unwrap();
    cx.update(|window, cx| {
        let store = crate::session::current_store(cx).expect("a UI session");
        store.update(cx, |store, cx| store.apply(&changes, cx));
        let holds_three = store
            .read(cx)
            .collections()
            .get::<SessionRow>("a")
            .is_some_and(|row| row.info.notifications.iter().any(|entry| entry.seq == 3));
        assert!(holds_three, "premise: the store holds entry 3");
        assert!(
            crate::layers::dismiss_top(window, cx),
            "premise: the list was open to close"
        );
    });
    test_support::settle(cx);
    assert!(!any_open(cx), "premise: the list closed");
    assert!(
        notification_commands(&mut fake).is_empty(),
        "the close marks only what a frame showed, and that was already read"
    );
    assert_eq!(
        bell_name(cx).as_deref(),
        Some("Notifications for a: 1 unread")
    );
}

/// SPEC.md, "Errors and diagnostics": the read mark is best effort, and "a lost mark costs a bell that stays loud until
/// the list is next closed". The old acceptance case had the next close try again. Specified: a mark the helm refuses
/// shows nothing and leaves the bell loud, and the next open and close sends the same mark again.
#[gpui::test]
fn a_refused_read_mark_is_sent_again_on_the_next_close(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(belled(json!([note(1, 60, "retry me")]), 0)));
    test_support::settle(cx);
    notification_commands(&mut fake);
    open_list(cx);
    test_support::keys(cx, "escape");
    assert_eq!(answered(&mut fake, cx, 500), [read_mark(1)]);
    assert_eq!(
        bell_name(cx).as_deref(),
        Some("Notifications for a: 1 unread")
    );
    open_list(cx);
    test_support::keys(cx, "escape");
    assert_eq!(answered(&mut fake, cx, 200), [read_mark(1)]);
}

// ---- Clear all -------------------------------------------------------------------------------------------------------

/// SPEC.md: "A clear button removes them all, and the bell disappears until a new one arrives." Specified: Clear all
/// sends `PUT /api/sessions/a/notifications/cleared` with `{"through": 7}`, the newest entry shown; once the helm pushes
/// the emptied row (with its read mark at 7) and answers, the bell and the list are gone and no read mark follows; a
/// notification pushed after that brings the bell back with one unread.
#[gpui::test]
fn clear_all_removes_the_bell_until_a_new_notification_arrives(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(belled(json!([note(7, 30, "clear me")]), 0)));
    test_support::settle(cx);
    notification_commands(&mut fake);
    open_list(cx);
    test_support::click(cx, CLEAR_ID);
    let sent = notification_commands(&mut fake);
    assert_eq!(sent.len(), 1, "{sent:?}");
    let (id, path, body) = sent[0].clone();
    assert_eq!(
        (path.as_str(), body),
        (
            "/api/sessions/a/notifications/cleared",
            json!({ "through": 7 })
        )
    );
    push_a(&mut fake, belled(json!([]), 7));
    fake.reply(id, 200, None);
    test_support::settle(cx);
    assert!(bell_name(cx).is_none(), "the bell is gone");
    assert!(!any_open(cx), "the list went with it");
    assert!(
        notification_commands(&mut fake).is_empty(),
        "nothing unread is left to mark"
    );
    push_a(&mut fake, belled(json!([note(8, 0, "a new one")]), 7));
    test_support::settle(cx);
    assert_eq!(
        bell_name(cx).as_deref(),
        Some("Notifications for a: 1 unread")
    );
}

/// SPEC.md, "Errors and diagnostics": "a failed toggle or clear surfaces like any other operation". A clear that failed
/// silently would read as done. Specified: when the helm refuses the clear with "the session's host is not connected",
/// the list stays open with the bell, and under Clear all it says "Clear notifications: the session's host is not
/// connected".
#[gpui::test]
fn a_refused_clear_says_why_in_the_list(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(belled(json!([note(7, 30, "keep me")]), 7)));
    test_support::settle(cx);
    notification_commands(&mut fake);
    open_list(cx);
    test_support::click(cx, CLEAR_ID);
    let sent = notification_commands(&mut fake);
    assert_eq!(sent.len(), 1, "{sent:?}");
    fake.reply(
        sent[0].0,
        409,
        Some(json!("the session's host is not connected")),
    );
    test_support::settle(cx);
    assert!(any_open(cx), "the list stays open");
    assert!(bell_name(cx).is_some(), "the bell stays");
    assert_eq!(
        test_support::label(cx, CLEAR_REFUSAL_ID).as_deref(),
        Some("Clear notifications: the session's host is not connected")
    );
}

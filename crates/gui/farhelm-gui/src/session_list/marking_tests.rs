//! Tests of bulk marking ([`super::marking`]). First its rules as pure functions: the gesture a click's modifiers ask
//! for, what a gesture does to the marks over the order the list shows, the marks that count and the actions a frame
//! offers, the metadata actions' targets, and the targets an action may still act on when it runs. Ported from the old
//! app's `list/marking.rs` tests at reference commit `24bc0580`.
//!
//! Then the running list, in the real main view over the helm client's fake, as the list's other tests run, ported from
//! the old `e2e/tests/bulk-session-actions.spec.ts` cases that need no batch: real clicks with modifiers mark rows, and
//! the frame shows the marks, the bar and the menu, which the tests find by their ids and debug selectors (a row's
//! marker is `session-marked-<key>`, the bar's count `session-marked-count`, its ⋯ `session-marked-menu-toggle`, its
//! Clear `session-marked-clear`, the menu `session-marked-menu`); what the window sent is read from the fake.
//!
//! The pointer stays over the rows after a click, which takes the order hold. The tests that push a new listing say
//! where the pointer is first ([`away`]), so the rows are drawn in the order the test names.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{Availability, CollectionState};
use gpui::{
    AppContext as _, Context, Entity, IntoElement, Modifiers, Render, Styled, TestAppContext,
    VisualTestContext, Window, div, px,
};
use serde_json::{Value, json};

use super::host_rail::tests::filtered;
use super::marking::{
    CLEAR_ID, COUNT_ID, Gesture, MENU_ID, MENU_TOGGLE_ID, Marks, MetadataAction, Target, Write,
    admitted, effective, project,
};
use super::menu_tests::{drawn, label_of, push, snapshot};
use super::pins_tests::away;
use super::seen::Toggle;
use super::selection::Listed;
use super::tests::{
    Fixture, TEST_NOW, change, launcher_drawn, listing_meta, painted, row_order, selected, session,
};
use super::{NEW_ID, SessionList};
use crate::actions::{DELETE_SESSION_CHORD, NewSession, RENAME_SESSION_CHORD};
use crate::focus::{self, Mark};
use crate::layers::{self, Popover};
use crate::main_view::MainView;
use crate::operations::OpKey;
use crate::test_support::{self, focused_id, frame, started};
use crate::{session, trash};

/// Marks holding exactly `keys`, made by toggling each over a list that shows them.
fn marked(keys: &[&str]) -> Marks {
    let mut marks = Marks::default();
    for key in keys {
        marks.mark(keys, None, key, Gesture::Toggle);
    }
    marks
}

/// The marked keys among `among`, in its order.
fn marks_in<'a>(marks: &Marks, among: &[&'a str]) -> Vec<&'a str> {
    among
        .iter()
        .copied()
        .filter(|key| marks.contains(key))
        .collect()
}

/// Why this matters: SPEC.md, "Session list": "Cmd+click (macOS) or Ctrl+click (Linux) toggles one row's mark" and
/// "Shift takes precedence when both kinds of modifier are held"; the old app accepted both toggle keys on every
/// platform, and the rule still does (macOS turns a Ctrl-click into a secondary click before it gets here). Specified:
/// no modifier and Alt alone open; Ctrl, Cmd and both toggle; Shift with any of them marks a range.
#[test]
fn shift_wins_and_both_toggle_keys_work_everywhere() {
    let with = |shift, control, platform, alt| Modifiers {
        shift,
        control,
        platform,
        alt,
        function: false,
    };
    assert_eq!(
        Gesture::from_modifiers(&with(false, false, false, false)),
        Gesture::Open
    );
    assert_eq!(
        Gesture::from_modifiers(&with(false, false, false, true)),
        Gesture::Open
    );
    for toggle in [
        with(false, true, false, false),
        with(false, false, true, false),
        with(false, true, true, false),
    ] {
        assert_eq!(
            Gesture::from_modifiers(&toggle),
            Gesture::Toggle,
            "{toggle:?}"
        );
    }
    for range in [
        with(true, false, false, false),
        with(true, true, false, false),
        with(true, false, true, true),
    ] {
        assert_eq!(Gesture::from_modifiers(&range), Gesture::Range, "{range:?}");
    }
}

/// Why this matters: a range is what the user sees between two rows, so it follows the order shown (pins first, held),
/// not the helm's, and it replaces the earlier marking rather than adding to it (SPEC.md: Shift-click "marks the
/// range"). Specified: over d, b, a, c shown with b open, Shift on c marks b, a and c (both ends included) and drops an
/// earlier mark on d and on the hidden x; Shift on the open row marks only it.
#[test]
fn a_range_follows_the_order_shown_and_replaces_every_earlier_mark() {
    let shown = ["d", "b", "a", "c"];
    let mut marks = Marks::default();
    marks.mark(&["x", "d"], None, "x", Gesture::Toggle);
    marks.mark(&shown, None, "d", Gesture::Toggle);
    marks.mark(&shown, Some("b"), "c", Gesture::Range);
    assert_eq!(
        marks_in(&marks, &["d", "b", "a", "c", "x"]),
        ["b", "a", "c"]
    );

    marks.mark(&shown, Some("b"), "b", Gesture::Range);
    assert_eq!(marks_in(&marks, &["d", "b", "a", "c", "x"]), ["b"]);
}

/// Why this matters: SPEC.md: with the open session not displayed, Shift-click "marks only the clicked row", and a row
/// the list does not show cannot be clicked into a mark. Specified: with no open session, and with an open session the
/// list hides, Shift on c marks only c; a gesture on a key not shown changes nothing.
#[test]
fn a_hidden_or_missing_anchor_marks_only_the_clicked_row() {
    let shown = ["a", "b", "c"];
    for anchor in [None, Some("hidden")] {
        let mut marks = Marks::default();
        marks.mark(&shown, anchor, "c", Gesture::Range);
        assert_eq!(marks_in(&marks, &shown), ["c"], "anchor {anchor:?}");
    }
    let mut marks = marked(&["a"]);
    marks.mark(&shown, Some("a"), "elsewhere", Gesture::Range);
    marks.mark(&shown, Some("a"), "elsewhere", Gesture::Toggle);
    assert_eq!(marks_in(&marks, &["a", "b", "c", "elsewhere"]), ["a"]);
}

/// Why this matters: SPEC.md: Cmd+click (macOS) or Ctrl+click (Linux) "toggles one row's mark, including the open row",
/// "Filtering or opening Hosts retains hidden marks but never counts or acts on them", and "A plain accepted row click
/// opens that session and clears all marks". Specified: toggling the open row marks then unmarks it; a mark on a row
/// the list stops showing stays stored but is not effective, and comes back when the row is shown again; effective
/// marks follow the order shown, and an empty list shows none; a plain click clears every mark, the hidden ones too.
#[test]
fn toggling_keeps_hidden_marks_which_never_count() {
    let mut marks = Marks::default();
    marks.mark(&["a", "b"], Some("a"), "a", Gesture::Toggle);
    assert!(marks.contains("a"));
    marks.mark(&["a", "b"], Some("a"), "a", Gesture::Toggle);
    assert!(!marks.contains("a"));

    let marks = marked(&["a", "c"]);
    assert_eq!(effective(&["b", "c"], &marks), ["c"], "a is hidden");
    assert!(marks.contains("a"), "a hidden mark is kept");
    assert_eq!(effective(&["c", "b", "a"], &marks), ["c", "a"]);
    assert!(effective(&[] as &[&str], &marks).is_empty());

    let mut marks = marks;
    marks.mark(&["b", "c"], Some("b"), "b", Gesture::Open);
    assert!(marks.is_empty(), "a plain click forgets a hidden mark too");
}

/// Why this matters: work that clears its own targets when it finishes must not undo a choice made since (the old
/// app's interaction revision), while a pruned session is not a choice. Specified: each gesture, Clear and the menu's
/// opening counts one, a gesture on a row not shown counts too (the user made a choice), and pruning counts none.
#[test]
fn every_marking_choice_counts_and_pruning_does_not() {
    let mut marks = Marks::default();
    marks.mark(&["a"], None, "a", Gesture::Toggle);
    marks.mark(&["a"], None, "gone", Gesture::Toggle);
    marks.touch();
    marks.clear();
    assert_eq!(marks.revision(), 4);
    marks.mark(&["a"], None, "a", Gesture::Toggle);
    marks.prune(|key| key == "a");
    assert_eq!(marks.revision(), 5);
    assert!(!marks.contains("a"), "pruned");
}

/// Why this matters: SPEC.md has the marked menu offer "Pin (only unpinned marks), Unpin (only pinned), Mark read and
/// Mark unread (only marks whose own menu offers it)". Specified: an unpinned row is Pin's and a pinned one Unpin's; an
/// idle row with output nobody has seen is Mark read's, with the activity its row shows; an idle row seen is Mark
/// unread's; an exited row is neither's.
#[test]
fn each_metadata_action_takes_the_rows_its_own_menu_would() {
    let unpinned = session("a").row();
    let pinned = session("b").pinned(0).row();
    let unseen = session("c")
        .status(json!({"state": "idle"}))
        .activity(200)
        .seen(Some(100))
        .row();
    let seen_idle = session("d").status(json!({"state": "idle"})).row();
    let exited = session("e")
        .status(json!({"state": "exited", "exit_code": 0}))
        .row();
    let takes = |action: MetadataAction, row| action.target("k", row).is_some();
    assert!(takes(MetadataAction::Pin, &unpinned) && !takes(MetadataAction::Pin, &pinned));
    assert!(takes(MetadataAction::Unpin, &pinned) && !takes(MetadataAction::Unpin, &unpinned));
    assert_eq!(
        MetadataAction::Read.target("c", &unseen),
        Some(Target {
            key: "c".into(),
            write: Write::Seen(Toggle::Read(200)),
        })
    );
    assert_eq!(
        MetadataAction::Unpin
            .target("b", &pinned)
            .map(|target| target.write),
        Some(Write::Pin(false))
    );
    assert!(!takes(MetadataAction::Read, &seen_idle));
    assert!(takes(MetadataAction::Unread, &seen_idle) && !takes(MetadataAction::Unread, &unseen));
    for action in [MetadataAction::Read, MetadataAction::Unread] {
        assert!(!takes(action, &exited), "{action:?} on an ended row");
    }
}

/// Why this matters: SPEC.md: "the answer authorizes only the listed identities and drawn consequences" and a target
/// is never gained, so an action keeps the rows it drew with their drawn values. Specified: of drawn targets a (drawn
/// with activity 5), b and c, with b no longer marked, c no longer shown and d newly marked and shown, the action keeps
/// only a, still with activity 5.
#[test]
fn an_action_keeps_its_drawn_targets_still_shown_and_marked_and_gains_none() {
    let target = |key: &str, write| Target {
        key: key.into(),
        write,
    };
    let drawn = [
        target("a", Write::Seen(Toggle::Read(5))),
        target("b", Write::Pin(true)),
        target("c", Write::Pin(true)),
    ];
    let marks = marked(&["a", "c", "d"]);
    assert_eq!(
        admitted(&drawn, &["a", "b", "d"], &marks),
        [target("a", Write::Seen(Toggle::Read(5)))]
    );
}

/// Why this matters: the bar's count, the row look and the ⋯'s presence all come from one projection, so it must hide
/// what the list does not show and offer an action only where a shown row takes it; on main every row is pinned or
/// not, so some action always applies while any mark is shown, and the ⋯-omitted case of the old app (marks that only
/// retired containers held) has no rows to reach it yet. Specified: of a (unpinned), b (pinned, unseen) and c (marked
/// but not shown), with a and b shown, the frame shows a and b and offers Pin (a), Unpin (b), Mark read (b, at the
/// activity its row shows) and Mark unread (a) in the menu's order, then only Pin and Mark unread when b is not marked.
/// (While marks do not count, with Hosts shown or a failed listing, the list projects nothing at all; the running tests
/// cover that.)
#[test]
fn a_frame_counts_and_offers_only_the_marks_it_shows() {
    let rows = [
        session("a").row(),
        session("b")
            .pinned(1)
            .status(json!({"state": "idle"}))
            .activity(200)
            .seen(Some(100))
            .row(),
    ];
    let listed = [
        Listed {
            key: "a",
            row: &rows[0],
        },
        Listed {
            key: "b",
            row: &rows[1],
        },
    ];
    let marks = marked(&["a", "b", "c"]);
    /// Each offer's action with its targets' keys.
    fn offered(projection: &super::marking::Projection) -> Vec<(MetadataAction, Vec<&str>)> {
        projection
            .offers
            .iter()
            .map(|offer| {
                let keys = offer.targets.iter().map(|target| target.key.as_str());
                (offer.action, keys.collect())
            })
            .collect()
    }
    let all = project(&listed, &marks);
    assert_eq!(&all.shown[..], ["a", "b"]);
    assert_eq!(
        offered(&all),
        [
            (MetadataAction::Pin, vec!["a"]),
            (MetadataAction::Unpin, vec!["b"]),
            (MetadataAction::Read, vec!["b"]),
            (MetadataAction::Unread, vec!["a"]),
        ]
    );
    assert_eq!(
        all.offers[2].targets[0].write,
        Write::Seen(Toggle::Read(200)),
        "Mark read carries the activity drawn"
    );

    let one = project(&listed, &marked(&["a"]));
    assert_eq!(&one.shown[..], ["a"]);
    assert_eq!(
        offered(&one),
        [
            (MetadataAction::Pin, vec!["a"]),
            (MetadataAction::Unread, vec!["a"]),
        ]
    );
}

// ---- The running list: fixtures and helpers --------------------------------------------------------------------------

/// The main view over the fake helm, connected with `snapshot` in an active window, and the window's session list,
/// which the tests read the stored marks from.
fn open_list(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
) -> (FakeHelm, Entity<SessionList>, &mut VisualTestContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(snapshot);
    cx.run_until_parked();
    let (main, cx) = test_support::open_view(cx, MainView::new);
    let list = cx.update(|_, cx| main.read(cx).session_list().clone());
    (fake, list, cx)
}

/// A snapshot of unpinned, running sessions `keys` (each with no tabs) in that order, the first one open.
fn fleet(keys: &[&'static str]) -> Vec<CollectionState> {
    rows(keys.iter().map(|&key| session(key)).collect())
}

/// A snapshot of `fixtures` (each with no tabs) in that order, the first one open.
fn rows(fixtures: Vec<Fixture>) -> Vec<CollectionState> {
    let rows: Vec<(Fixture, usize)> = fixtures.into_iter().map(|fixture| (fixture, 0)).collect();
    snapshot(&rows, false)
}

fn ctrl() -> Modifiers {
    Modifiers {
        control: true,
        ..Modifiers::default()
    }
}

fn cmd() -> Modifiers {
    Modifiers {
        platform: true,
        ..Modifiers::default()
    }
}

fn shift() -> Modifiers {
    Modifiers {
        shift: true,
        ..Modifiers::default()
    }
}

/// Click the middle of what is drawn under `selector` with `modifiers` held, and let the window settle.
fn click_with(cx: &mut VisualTestContext, selector: &'static str, modifiers: Modifiers) {
    let at = test_support::center(cx, selector);
    cx.simulate_click(at, modifiers);
    test_support::settle(cx);
}

/// The rows of a to e, p, u and v that the last frame drew marked, by their marker, in that order.
fn markers(cx: &mut VisualTestContext) -> Vec<&'static str> {
    [
        ("a", "session-marked-a"),
        ("b", "session-marked-b"),
        ("c", "session-marked-c"),
        ("d", "session-marked-d"),
        ("e", "session-marked-e"),
        ("p", "session-marked-p"),
        ("u", "session-marked-u"),
        ("v", "session-marked-v"),
    ]
    .into_iter()
    .filter(|(_, marker)| cx.debug_bounds(marker).is_some())
    .map(|(key, _)| key)
    .collect()
}

/// Whether `key` is marked in the list's stored marks, shown or not.
fn stored(list: &Entity<SessionList>, key: &str, cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| list.update(cx, |list, cx| list.current.current(cx).marks.contains(key)))
}

/// The bar's count as it names itself ("2 marked"), or `None` while the bar is not drawn.
fn count(cx: &mut VisualTestContext) -> Option<String> {
    label_of(cx, COUNT_ID)
}

/// Every command the window sent since the last look, answered with success, as (method, path, body).
fn requests(fake: &mut FakeHelm) -> Vec<(Method, String, Value)> {
    fake.commands()
        .into_iter()
        .map(|command| {
            fake.reply(command.id, 200, None);
            (
                command.method,
                command.path,
                command.body.unwrap_or_default(),
            )
        })
        .collect()
}

/// `PUT /api/sessions/<key>/pin` with `pinned`.
fn pin_put(key: &str, pinned: bool) -> (Method, String, Value) {
    (
        Method::Put,
        format!("/api/sessions/{key}/pin"),
        json!({ "pinned": pinned }),
    )
}

/// One of the marked bar's controls whose focus handle a test can reach.
enum BarControl {
    Count,
    Menu,
}

/// Move focus to the bar's `control` the way the app moves it ([`focus::focus_control`]), so a test starts from it on
/// every platform: a click focuses a control on Linux only.
fn focus_bar_control(list: &Entity<SessionList>, control: BarControl, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let kept = &list.read(cx).marking;
        let handle = match control {
            BarControl::Count => kept.count_focus(),
            BarControl::Menu => kept.menu_focus(),
        }
        .clone();
        focus::focus_control(&handle, &Mark::take(cx), window, cx);
    });
    test_support::settle(cx);
}

/// Move focus to `key`'s row (its list item, which contains the row's controls) the way the app moves focus
/// ([`focus::focus_control`]), so a test starts there on every platform.
fn focus_row(list: &Entity<SessionList>, key: &str, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let handle = list.read(cx).row_focus.get(key).cloned();
        let handle = handle.expect("the premise: the row is listed");
        focus::focus_control(&handle, &Mark::take(cx), window, cx);
    });
    test_support::settle(cx);
}

/// Start a Rename of `key` through the operation store and leave it in flight, so `key` has a request out (its commands
/// are refused, as the row menu draws them).
fn start_rename(key: &str, cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        let operations = session::current_operations(cx).unwrap();
        let rename = super::rename::command(key, "renamed").unwrap();
        operations.update(cx, |operations, cx| {
            operations
                .start(&OpKey::session(key), rename, cx, |_, _| {})
                .unwrap()
        });
    });
}

/// The top edge of what the last frame drew under `selector`.
fn top(cx: &mut VisualTestContext, selector: &'static str) -> f32 {
    f32::from(test_support::bounds(cx, selector).top())
}

// ---- Gestures ------------------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md, "Session list": Shift+click marks the range from the open session "without opening it",
/// Cmd+click (macOS) or Ctrl+click (Linux) "toggles one row's mark, including the open row", and "a plain accepted row
/// click opens that session and clears all marks"; the old spec's case "range and toggle keep the open session".
/// Specified, by real clicks over a, b, c and d with a open: Ctrl on c marks c alone, and nothing is opened or sent;
/// Cmd on a adds the open row; Shift on c replaces both with a, b and c; Ctrl on b unmarks b; the open session is a
/// throughout, and the count reads "2 marked"; a plain click on d opens d and leaves nothing marked, stored or drawn.
#[gpui::test]
fn marking_clicks_open_nothing_and_a_plain_click_opens_and_clears(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(cx, fleet(&["a", "b", "c", "d"]));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    requests(&mut fake);

    click_with(cx, "session-row-c", ctrl());
    assert_eq!(markers(cx), ["c"]);
    assert_eq!(count(cx).as_deref(), Some("1 marked"));
    click_with(cx, "session-row-a", cmd());
    assert_eq!(markers(cx), ["a", "c"], "the open row toggles too");
    click_with(cx, "session-row-c", shift());
    assert_eq!(markers(cx), ["a", "b", "c"]);
    click_with(cx, "session-row-b", ctrl());
    assert_eq!(markers(cx), ["a", "c"]);
    assert_eq!(count(cx).as_deref(), Some("2 marked"));
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "no marking click opened anything"
    );
    assert_eq!(requests(&mut fake), [], "and none told the helm anything");

    test_support::click(cx, "session-row-d");
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("d"));
    assert_eq!(markers(cx), [] as [&str; 0]);
    assert_eq!(count(cx), None, "the bar went with the marks");
    assert!(!stored(&list, "a", cx) && !stored(&list, "c", cx));
}

/// Why this matters: SPEC.md: Shift+click marks the range "in displayed order", and a pinned row is displayed first
/// whatever the helm's order; a range taken over the helm's order would mark rows the user did not click between.
/// Specified: with a open and the helm serving a, u, p and v, p pinned (drawn p, a, u, v), Shift on p marks p and a,
/// the two rows between and including the open row and p on screen, and not u, which lies between them in the helm's
/// order.
#[gpui::test]
fn a_range_follows_the_order_on_screen_not_the_helms(cx: &mut TestAppContext) {
    let (_fake, _list, cx) = open_list(
        cx,
        rows(vec![
            session("a"),
            session("u"),
            session("p").pinned(1),
            session("v"),
        ]),
    );
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    assert_eq!(
        row_order(&frame(cx)),
        ["p", "a", "u", "v"],
        "the premise: pins first"
    );

    click_with(cx, "session-row-p", shift());
    assert_eq!(markers(cx), ["a", "p"]);
}

/// Why this matters: SPEC.md: "Row controls keep their own actions" (the menu button, the status dot, the bell); a
/// click on one must neither mark its row nor clear the marks, or reaching a row's menu would lose the user's marking.
/// Specified: with c marked and a open, a click on b's menu button opens b's menu, one on b's status dot sends b's seen
/// toggle, and one on b's bell opens its notification list, and after each c alone is still marked and a still open.
#[gpui::test]
fn row_controls_neither_mark_nor_clear(cx: &mut TestAppContext) {
    let bell = json!([{ "seq": 1, "at": TEST_NOW - 60, "text": "build finished" }]);
    let (mut fake, list, cx) = open_list(
        cx,
        rows(vec![
            session("a"),
            session("b").notifications(bell, 0),
            session("c"),
        ]),
    );
    requests(&mut fake);
    click_with(cx, "session-row-c", ctrl());
    assert_eq!(markers(cx), ["c"], "the premise");

    test_support::click(cx, "session-menu-b");
    assert!(drawn(cx, "session-menu"), "b's menu opened");
    test_support::keys(cx, "escape");
    assert_eq!(markers(cx), ["c"]);

    test_support::click(cx, "session-seen-b");
    test_support::settle(cx);
    assert!(
        requests(&mut fake)
            .iter()
            .any(|(_, path, _)| path == "/api/sessions/b/seen"),
        "the dot sent b's toggle"
    );
    assert_eq!(markers(cx), ["c"]);

    test_support::click(cx, "session-bell-b");
    assert!(test_support::any_open(cx), "b's notification list opened");
    test_support::keys(cx, "escape");
    assert_eq!(markers(cx), ["c"]);
    assert!(!stored(&list, "b", cx));
    assert_eq!(selected(cx).as_deref(), Some("a"));
}

// ---- What counts -----------------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md: "Filtering or opening Hosts retains hidden marks but never counts or acts on them"; the
/// old spec's "metadata acts only on displayed eligible rows" starts from a mark a filter hides. Specified: with b and
/// c marked, a filtered listing without b counts "1 marked" and draws only c marked while b stays stored; the complete
/// listing bringing b back counts "2 marked" again; with the Hosts panel shown no bar is drawn and both stay stored,
/// and back on Sessions the bar counts "2 marked".
#[gpui::test]
fn hidden_marks_survive_a_filter_and_hosts_and_are_never_counted(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(cx, fleet(&["a", "b", "c"]));
    click_with(cx, "session-row-b", ctrl());
    click_with(cx, "session-row-c", ctrl());
    assert_eq!(count(cx).as_deref(), Some("2 marked"), "the premise");
    away(cx);

    fake.push(change(filtered(&["a", "c"], 3, 2), &[], &["b"]));
    test_support::settle(cx);
    assert_eq!(count(cx).as_deref(), Some("1 marked"));
    assert_eq!(markers(cx), ["c"]);
    assert!(stored(&list, "b", cx), "a hidden mark is kept");

    fake.push(change(
        listing_meta(&["a", "b", "c"], 3, false),
        &[session("b")],
        &[],
    ));
    test_support::settle(cx);
    assert_eq!(count(cx).as_deref(), Some("2 marked"));
    assert_eq!(markers(cx), ["b", "c"]);

    test_support::click(cx, "hosts-panel-toggle");
    assert!(drawn(cx, "hosts-panel"), "the premise: Hosts shows");
    assert_eq!(count(cx), None, "no bar while Hosts shows");
    assert!(stored(&list, "b", cx) && stored(&list, "c", cx));
    test_support::click(cx, "hosts-panel-toggle");
    assert_eq!(count(cx).as_deref(), Some("2 marked"));
}

/// Why this matters: SPEC.md: "failed listings expose no targets", yet a failed read is no evidence that anything went,
/// so the marks must survive it. Specified: with b marked, a listing the helm could not read draws no bar and no
/// marker, and a Ctrl-click on a marks nothing; once the helm reads the listing again, b alone is marked.
#[gpui::test]
fn a_failed_listing_offers_nothing_and_forgets_nothing(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(cx, fleet(&["a", "b"]));
    click_with(cx, "session-row-b", ctrl());
    assert_eq!(markers(cx), ["b"], "the premise");
    away(cx);

    let mut failed = listing_meta(&["a", "b"], 2, false);
    failed.availability = Availability::Unavailable {
        error: "the helm could not read its sessions".into(),
    };
    fake.push(change(failed, &[], &[]));
    test_support::settle(cx);
    assert!(drawn(cx, "session-row-b"), "the premise: the rows stay");
    assert_eq!(count(cx), None);
    assert_eq!(markers(cx), [] as [&str; 0]);
    click_with(cx, "session-row-a", ctrl());
    assert!(
        !stored(&list, "a", cx),
        "no gesture marks over a failed listing"
    );
    away(cx);

    fake.push(change(listing_meta(&["a", "b"], 2, false), &[], &[]));
    test_support::settle(cx);
    assert_eq!(markers(cx), ["b"]);
    assert_eq!(count(cx).as_deref(), Some("1 marked"));
}

/// Why this matters: a mark is forgotten on the same evidence as a session (SPEC.md: "A complete authoritative listing
/// prunes vanished session keys, while a partial, filtered, failed or non-authoritative reply does not establish
/// disappearance"). Specified: with b and c marked, a cut listing and a filtered listing that leave c out keep c
/// stored; a complete listing without c forgets c and keeps b.
#[gpui::test]
fn only_a_complete_listing_forgets_a_vanished_mark(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(cx, fleet(&["a", "b", "c"]));
    click_with(cx, "session-row-b", ctrl());
    click_with(cx, "session-row-c", ctrl());
    assert_eq!(markers(cx), ["b", "c"], "the premise");
    away(cx);

    fake.push(change(listing_meta(&["a", "b"], 3, true), &[], &["c"]));
    test_support::settle(cx);
    assert!(stored(&list, "c", cx), "a cut listing proves nothing");
    fake.push(change(filtered(&["a", "b"], 3, 2), &[], &[]));
    test_support::settle(cx);
    assert!(stored(&list, "c", cx), "a filtered listing proves nothing");

    fake.push(change(listing_meta(&["a", "b"], 2, false), &[], &[]));
    test_support::settle(cx);
    assert!(!stored(&list, "c", cx), "a complete listing forgets c");
    assert!(stored(&list, "b", cx));
}

// ---- The bar -------------------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md: with any effective marks, the bar replaces "the ordinary title, count and icon buttons"
/// while "any truncation or unsupported-filter warning" remains; the old app kept the cut band beside its bar. Clear
/// hands focus to the order's title when the click put focus on it, since Clear itself goes with the bar, and leaves
/// it alone otherwise. Specified: over a cut listing, marking b replaces the title, Trash, Templates and New with "1
/// marked", the ⋯ and Clear while the cut band stays; under the Linux click rule (a click focuses the control), Clear
/// brings them all back, marks nothing and leaves focus on the title; under the macOS rule (a click focuses nothing),
/// with focus on b's row, Clear clears the marks and focus stays on the row.
#[gpui::test]
fn the_bar_replaces_the_heading_and_the_cut_band_stays(cx: &mut TestAppContext) {
    let mut cut = fleet(&["a", "b"]);
    cut[0].meta = listing_meta(&["a", "b"], 5, true);
    let (_fake, list, cx) = open_list(cx, cut);
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    let heading = [
        "session-order",
        trash::BUTTON_ID,
        "templates-button",
        NEW_ID,
    ];
    for control in heading {
        assert!(drawn(cx, control), "the premise: {control} is drawn");
    }
    assert!(
        drawn(cx, "session-count"),
        "the premise: the cut band shows"
    );

    click_with(cx, "session-row-b", ctrl());
    for control in heading {
        assert!(!drawn(cx, control), "{control} gave way to the bar");
    }
    for control in [COUNT_ID, MENU_TOGGLE_ID, CLEAR_ID, "session-count"] {
        assert!(drawn(cx, control), "{control} is drawn");
    }

    test_support::click(cx, CLEAR_ID);
    test_support::settle(cx);
    for control in heading {
        assert!(drawn(cx, control), "{control} is back");
    }
    assert!(!stored(&list, "b", cx));
    assert_eq!(focused_id(cx).as_deref(), Some("session-order"));

    click_with(cx, "session-row-b", ctrl());
    assert_eq!(markers(cx), ["b"], "the premise: b is marked again");
    focus_row(&list, "b", cx);
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    test_support::click(cx, CLEAR_ID);
    test_support::settle(cx);
    assert!(!stored(&list, "b", cx));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-item-b"),
        "the click focused nothing, so focus was never in the bar and stays on the row"
    );
}

/// Why this matters: the old count was `tabindex="-1"`: a target the app moves focus to (the bulk question's Confirm
/// will), never a stop Tab lands on between the list's controls. Specified: with b marked and focus on the ⋯, Shift+Tab
/// leaves the count out and Tab comes back to the ⋯; focus moved to the count's handle lands on the count.
#[gpui::test]
fn the_count_is_focusable_but_not_a_tab_stop(cx: &mut TestAppContext) {
    let (_fake, list, cx) = open_list(cx, fleet(&["a", "b"]));
    click_with(cx, "session-row-b", ctrl());
    test_support::click(cx, MENU_TOGGLE_ID);
    test_support::keys(cx, "escape");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(MENU_TOGGLE_ID),
        "the premise: the closed menu left focus on its toggle"
    );

    test_support::keys(cx, "shift-tab");
    assert_ne!(focused_id(cx).as_deref(), Some(COUNT_ID));
    test_support::keys(cx, "tab");
    assert_eq!(focused_id(cx).as_deref(), Some(MENU_TOGGLE_ID));

    focus_bar_control(&list, BarControl::Count, cx);
    assert_eq!(focused_id(cx).as_deref(), Some(COUNT_ID));
}

// ---- The marked menu -------------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md: "**Pin** applies only to unpinned marked rows and **Unpin** only to pinned ones. **Mark
/// read** and **Mark unread** apply only to marked rows whose own menus offer that action. Metadata choices reuse the
/// ordinary writes and row-local errors, and clear marks once their eligible writes have been dispatched"; the old
/// spec's "metadata acts only on displayed eligible rows". Specified: over a open and unpinned, b pinned with unseen
/// output, c exited and d, all marked and d then hidden by a filter, the menu is headed "3 marked" and offers Pin,
/// Unpin, Mark read and Mark unread top to bottom; Pin sends a's and c's pin and nothing else, the marks (d's too) are
/// gone before any reply, and focus is on the order's title; b marked again, Mark read sends b's seen mark at the
/// activity its row showed.
#[gpui::test]
fn metadata_writes_go_once_per_eligible_row_and_clear_the_marks(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(
        cx,
        rows(vec![
            session("a"),
            session("b")
                .pinned(1)
                .status(json!({"state": "idle"}))
                .activity(200)
                .seen(Some(100)),
            session("c").status(json!({"state": "exited", "exit_code": 0})),
            session("d"),
        ]),
    );
    requests(&mut fake);
    for row in [
        "session-row-a",
        "session-row-b",
        "session-row-c",
        "session-row-d",
    ] {
        click_with(cx, row, ctrl());
    }
    away(cx);
    fake.push(change(filtered(&["a", "b", "c"], 4, 3), &[], &["d"]));
    test_support::settle(cx);
    assert_eq!(
        count(cx).as_deref(),
        Some("3 marked"),
        "the premise: d is hidden"
    );

    test_support::click(cx, MENU_TOGGLE_ID);
    assert!(drawn(cx, MENU_ID));
    let texts = painted(&frame(cx));
    assert!(texts.iter().any(|text| text == "Session metadata"));
    let items = [
        "session-marked-menu-pin",
        "session-marked-menu-unpin",
        "session-marked-menu-read",
        "session-marked-menu-unread",
    ];
    let tops: Vec<f32> = items.into_iter().map(|item| top(cx, item)).collect();
    assert!(tops.is_sorted(), "in the menu's order: {tops:?}");
    assert_eq!(
        label_of(cx, "session-marked-menu-read").as_deref(),
        Some("Mark read")
    );

    test_support::click(cx, "session-marked-menu-pin");
    assert_eq!(count(cx), None, "cleared at dispatch, before any reply");
    assert!(!stored(&list, "d", cx), "the hidden mark too");
    assert_eq!(focused_id(cx).as_deref(), Some("session-order"));
    assert_eq!(
        requests(&mut fake),
        [pin_put("a", true), pin_put("c", true)]
    );

    click_with(cx, "session-row-b", ctrl());
    test_support::click(cx, MENU_TOGGLE_ID);
    test_support::click(cx, "session-marked-menu-read");
    test_support::settle(cx);
    assert_eq!(
        requests(&mut fake),
        [(
            Method::Put,
            "/api/sessions/b/seen".to_owned(),
            json!({ "seen_activity_at": 200 })
        )]
    );
}

/// Why this matters: SPEC.md: a refused row "gets the ordinary action-specific not-started message, including a row
/// engaged by its own question or rename", and the line lasts as the row's refusals do, until the session's next
/// operation starts (`outcome`'s rule). Without it a marked row with a request in flight would silently be left out.
/// Specified: with b's Rename in flight and a and b marked, Pin sends a's pin alone and b's row says "Pin: not started,
/// another operation was in progress. Try again."; the line stays once the Rename is answered, and goes when b's next
/// request (its seen toggle) starts.
#[gpui::test]
fn a_busy_row_gets_the_not_started_line_until_its_next_operation(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(cx, fleet(&["a", "b"]));
    requests(&mut fake);
    start_rename("b", cx);
    let rename = fake.commands();
    assert_eq!(rename.len(), 1, "the premise: b's Rename is in flight");

    click_with(cx, "session-row-a", ctrl());
    click_with(cx, "session-row-b", ctrl());
    test_support::click(cx, MENU_TOGGLE_ID);
    test_support::click(cx, "session-marked-menu-pin");
    test_support::settle(cx);
    // Read, not answered yet: the fake answers in the order commands were sent, and b's Rename went first.
    let pins = fake.commands();
    let sent: Vec<(Method, String, Value)> = pins
        .iter()
        .map(|command| {
            let body = command.body.clone().unwrap_or_default();
            (command.method, command.path.clone(), body)
        })
        .collect();
    assert_eq!(sent, [pin_put("a", true)]);
    let line = "Pin: not started, another operation was in progress. Try again.";
    assert_eq!(
        label_of(cx, "session-outcome-b-not-started").as_deref(),
        Some(line)
    );

    fake.reply(rename[0].id, 200, None);
    fake.reply(pins[0].id, 200, None);
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-b-not-started").as_deref(),
        Some(line),
        "a settle is not a start"
    );

    test_support::click(cx, "session-seen-b");
    test_support::settle(cx);
    assert!(
        requests(&mut fake)
            .iter()
            .any(|(_, path, _)| path == "/api/sessions/b/seen"),
        "the premise: b's next request started"
    );
    assert_eq!(label_of(cx, "session-outcome-b-not-started"), None);
}

/// Why this matters: marks belong to the UI session (the old app lost them on a reload), and a not-started line to the
/// session's operations, which a reset starts over; either one surviving a reset would speak for a connection that is
/// gone. Specified: with b's Rename in flight, a Pin of marked a and b leaving b's not-started line, and b marked
/// again, a broken channel and a new snapshot leave nothing marked, no bar, and no not-started line on b.
#[gpui::test]
fn a_reset_forgets_the_marks_and_the_not_started_lines(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(cx, fleet(&["a", "b"]));
    requests(&mut fake);
    start_rename("b", cx);
    assert_eq!(
        fake.commands().len(),
        1,
        "the premise: b's Rename is in flight"
    );
    click_with(cx, "session-row-a", ctrl());
    click_with(cx, "session-row-b", ctrl());
    test_support::click(cx, MENU_TOGGLE_ID);
    test_support::click(cx, "session-marked-menu-pin");
    test_support::settle(cx);
    assert!(
        label_of(cx, "session-outcome-b-not-started").is_some(),
        "the premise: b has its not-started line"
    );
    click_with(cx, "session-row-b", ctrl());
    assert_eq!(markers(cx), ["b"], "the premise: b is marked again");

    fake.break_channel();
    cx.run_until_parked();
    fake.connect(fleet(&["a", "b"]));
    test_support::settle(cx);
    assert!(!stored(&list, "b", cx));
    assert_eq!(count(cx), None);
    assert_eq!(label_of(cx, "session-outcome-b-not-started"), None);
}

/// Why this matters: the old menu closed when what it offered changed under it, so its items never act on an offer
/// the user did not see. Specified: with a marked and its menu open on Pin, the helm pushing a pinned closes the menu,
/// and a stays marked.
#[gpui::test]
fn the_marked_menu_closes_when_its_offers_change(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(cx, fleet(&["a", "b"]));
    click_with(cx, "session-row-a", ctrl());
    test_support::click(cx, MENU_TOGGLE_ID);
    assert!(drawn(cx, "session-marked-menu-pin"), "the premise");

    push(&mut fake, &["a", "b"], &[(session("a").pinned(1), 0)], cx);
    test_support::settle(cx);
    assert!(!drawn(cx, MENU_ID));
    assert!(stored(&list, "a", cx));
}

// ---- Escape ----------------------------------------------------------------------------------------------------------

/// A popover that never takes focus, for the Escape test: the root's Escape closes it, which shows that the key went on
/// past the list.
struct Floating;

impl Render for Floating {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(20.))
    }
}

/// Why this matters: SPEC.md: "Escape clears marks only from the session list, without preventing default handling";
/// the old spec's "Escape on Clear clears without preventing default". The window's own Escape (closing the top layer,
/// putting a tooltip away) must still happen. Specified: with b marked, focus moved to "N marked" (as the bulk
/// question's Confirm will move it) and a popover open that takes no focus, Escape clears the marks, the window's
/// Escape closes the popover, and focus goes to the order's title, since the count went with the bar.
#[gpui::test]
fn escape_in_the_list_clears_the_marks_and_goes_on(cx: &mut TestAppContext) {
    let (_fake, list, cx) = open_list(cx, fleet(&["a", "b"]));
    click_with(cx, "session-row-b", ctrl());
    assert_eq!(markers(cx), ["b"], "the premise");
    focus_bar_control(&list, BarControl::Count, cx);
    let anchor = cx
        .update(|_, cx| {
            list.update(cx, |list, cx| {
                list.current.current(cx).rows.borrow().anchor("a")
            })
        })
        .expect("the premise: a's row is drawn");
    let layer = cx
        .update(|window, cx| {
            let view = cx.new(|_| Floating);
            let mark = Mark::take(cx);
            layers::open_popover(
                Popover::floating(view, cx.focus_handle(), anchor),
                &mark,
                window,
                cx,
            )
        })
        .expect("the premise: the popover opened");
    test_support::settle(cx);
    assert_eq!(focused_id(cx).as_deref(), Some(COUNT_ID), "the premise");

    test_support::keys(cx, "escape");
    test_support::settle(cx);
    assert!(!stored(&list, "b", cx), "Escape cleared the marks");
    assert!(
        !test_support::is_open(cx, layer),
        "and went on to the window's Escape"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("session-order"));
}

/// Why this matters: Escape mostly comes from a row or a filter, not the bar; SPEC.md: "Escape clears marks only from
/// the session list", so the list's whole column must take it, and focus on a row is not the bar's to move (the row
/// stays). Specified: with b marked and focus on a's row, Escape clears the marks, and focus is still on a's row.
#[gpui::test]
fn escape_from_a_row_clears_the_marks_and_leaves_focus_on_the_row(cx: &mut TestAppContext) {
    let (_fake, list, cx) = open_list(cx, fleet(&["a", "b"]));
    click_with(cx, "session-row-b", ctrl());
    assert_eq!(markers(cx), ["b"], "the premise");
    focus_row(&list, "a", cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-item-a"),
        "the premise"
    );

    test_support::keys(cx, "escape");
    test_support::settle(cx);
    assert!(!stored(&list, "b", cx), "Escape cleared the marks");
    assert_eq!(count(cx), None);
    assert_eq!(focused_id(cx).as_deref(), Some("session-item-a"));
}

/// Why this matters: the old bar's Escape handed focus to the order's title before it cleared, as Clear did; without
/// it a keyboard user on Clear or the ⋯ loses their place in the sidebar when the bar goes. Specified: with b marked,
/// focus on the ⋯ and Tab taking it to Clear, Escape clears the marks and leaves focus on the order's title.
#[gpui::test]
fn escape_on_the_bar_hands_focus_to_the_order_title(cx: &mut TestAppContext) {
    let (_fake, list, cx) = open_list(cx, fleet(&["a", "b"]));
    click_with(cx, "session-row-b", ctrl());
    assert_eq!(markers(cx), ["b"], "the premise");
    focus_bar_control(&list, BarControl::Menu, cx);
    test_support::keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(CLEAR_ID),
        "the premise: Tab went on from the ⋯ to Clear"
    );

    test_support::keys(cx, "escape");
    test_support::settle(cx);
    assert!(!stored(&list, "b", cx));
    assert_eq!(count(cx), None);
    assert_eq!(focused_id(cx).as_deref(), Some("session-order"));
}

/// Why this matters: SPEC.md: Escape "yields to a menu, dialog or terminal that owns the key"; the old spec's "Escape
/// in a row menu and the ⋯ menu keeps marks". Specified: with b marked, Escape in a's row menu closes the menu and b
/// stays marked; Escape in the marked menu closes it, b stays marked and focus is back on the ⋯.
#[gpui::test]
fn escape_in_a_menu_keeps_the_marks(cx: &mut TestAppContext) {
    let (_fake, list, cx) = open_list(cx, fleet(&["a", "b"]));
    click_with(cx, "session-row-b", ctrl());
    assert_eq!(markers(cx), ["b"], "the premise");

    test_support::click(cx, "session-menu-a");
    assert!(drawn(cx, "session-menu"), "the premise");
    test_support::keys(cx, "escape");
    assert!(!drawn(cx, "session-menu"));
    assert!(stored(&list, "b", cx));

    test_support::click(cx, MENU_TOGGLE_ID);
    assert!(drawn(cx, MENU_ID), "the premise");
    test_support::keys(cx, "escape");
    assert!(!drawn(cx, MENU_ID));
    assert!(stored(&list, "b", cx));
    assert_eq!(focused_id(cx).as_deref(), Some(MENU_TOGGLE_ID));
}

// ---- Shortcuts, New and Trash while marked -------------------------------------------------------------------------

/// Why this matters: SPEC.md: "session shortcuts act only on the open session, whatever is marked", and Rename stays a
/// single-session action; the old spec's "shortcuts act on the open session". Specified: with a open and b marked, the
/// Rename shortcut renames a, and once that is answered, the Delete shortcut asks about a and its Confirm sends a's
/// Delete alone; b stays marked.
#[gpui::test]
fn session_shortcuts_act_on_the_open_session_while_another_is_marked(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(cx, fleet(&["a", "b"]));
    requests(&mut fake);
    click_with(cx, "session-row-b", ctrl());
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise");

    test_support::keys(cx, RENAME_SESSION_CHORD);
    assert!(drawn(cx, "rename"), "the editor opened");
    let select_all = if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    };
    test_support::keys(cx, select_all);
    test_support::keys(cx, "r e n a m e d enter");
    test_support::settle(cx);
    let paths: Vec<String> = requests(&mut fake)
        .into_iter()
        .map(|(_, path, _)| path)
        .collect();
    assert_eq!(paths, ["/api/sessions/a/rename"]);
    test_support::settle(cx);

    test_support::keys(cx, DELETE_SESSION_CHORD);
    assert!(drawn(cx, "prompt"), "the Delete question opened");
    test_support::keys(cx, "enter");
    test_support::settle(cx);
    let deletes: Vec<String> = requests(&mut fake)
        .into_iter()
        .filter(|(method, _, _)| *method == Method::Delete)
        .map(|(_, path, _)| path)
        .collect();
    assert_eq!(deletes, ["/api/sessions/a"]);
    assert!(stored(&list, "b", cx));
}

/// Why this matters: the bar hides New, and the old app kept New's shortcut opening the launcher meanwhile and sent
/// focus back to the ⋯ when the launcher was cancelled; the old spec's "marking retains New dispatch and launcher
/// cancellation focus". Specified: with b marked, New not drawn and focus on the ⋯, the New action opens the launcher,
/// and Escape closes it with focus back on the ⋯ (what held focus when it opened, the port's launcher rule).
#[gpui::test]
fn the_new_action_opens_the_launcher_while_marked(cx: &mut TestAppContext) {
    let (_fake, list, cx) = open_list(cx, fleet(&["a", "b"]));
    click_with(cx, "session-row-b", ctrl());
    assert!(!drawn(cx, NEW_ID), "the premise: New is hidden");
    focus_bar_control(&list, BarControl::Menu, cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(MENU_TOGGLE_ID),
        "the premise"
    );

    cx.dispatch_action(NewSession);
    test_support::settle(cx);
    assert!(launcher_drawn(cx));
    test_support::keys(cx, "escape");
    test_support::settle(cx);
    assert!(!launcher_drawn(cx), "Escape cancelled the launcher");
    assert_eq!(focused_id(cx).as_deref(), Some(MENU_TOGGLE_ID));
}

/// Why this matters: the bar hides the Trash button, and the old app kept the button's owner and any dialog it opened
/// meanwhile (the old spec's "retained mark keeps the open Trash dialog"); a dialog that closed or reopened under the
/// user because a mark appeared would lose their place. Specified: with the Trash dialog open, a mark appearing hides
/// the Trash button and draws the bar, and the same dialog stays open; Escape closes it and leaves the mark.
#[gpui::test]
fn the_trash_dialog_survives_marks_appearing(cx: &mut TestAppContext) {
    let (_fake, list, cx) = open_list(cx, fleet(&["a", "b"]));
    test_support::click(cx, trash::BUTTON_ID);
    test_support::settle(cx);
    assert!(drawn(cx, "checkout-trash-dialog"), "the premise");
    let dialog = cx.update(|_, cx| list.read(cx).trash.read(cx).dialog_view());
    assert!(dialog.is_some(), "the premise: the button keeps its dialog");

    cx.update(|_, cx| list.update(cx, |list, cx| list.row_clicked("b", Gesture::Toggle, cx)));
    test_support::settle(cx);
    assert!(!drawn(cx, trash::BUTTON_ID));
    assert_eq!(count(cx).as_deref(), Some("1 marked"));
    assert!(drawn(cx, "checkout-trash-dialog"));
    assert_eq!(
        cx.update(|_, cx| list.read(cx).trash.read(cx).dialog_view()),
        dialog,
        "the same dialog"
    );

    test_support::keys(cx, "escape");
    test_support::settle(cx);
    assert!(!drawn(cx, "checkout-trash-dialog"));
    assert!(stored(&list, "b", cx));
}

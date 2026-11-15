//! Tests of the session list: its rows and their order, the selection rules and what they tell the helm, the list's
//! and the pane's states, host text in a row, and what a reset does to all of it.
//!
//! The rules are tested as pure functions where they are pure (the automatic choice, the seen mark's memory, the count
//! line, the home folding); everything a user sees or the helm receives is tested in the real main view over the helm
//! client's fake, which hands the test every command the window sent. Rows are found by their open control's id, which
//! is the session's key, and read from GPUI's driver record of the frame (what each element painted and its
//! accessible name).

use std::time::Duration;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::{Collections, collections::decode_state};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, HostView,
    KeyedEntry, ListCounts, PREFERENCES_KEY, Preferences, SessionRow,
};
use gpui::{AppContext as _, DriverNode, Role, TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::row::{ITEM_ID_PREFIX, ROW_ID_PREFIX, abbreviate_home, host_label};
use super::selection::{self, Change, Listed, Open, Stamped};
use super::status::Live;
use super::{CountLine, SessionList, count_line, empty_fleet};
use crate::components::pulse_opacity;
use crate::host_text::tests::{SPOOFING_CORPUS, painted_words};
use crate::main_view::MainView;
use crate::notices::{self, Notice};
use crate::test_support::{frame, meta, session_entry, started};
use crate::{local_state, render_count, session_pane, test_support, theme, ticker};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// The wall clock's reading, in seconds since the epoch, when a test's app starts: GPUI's test clock starts at
/// `2025-07-01T23:59:58Z` and moves only as a test advances it, and [`ticker::wall_now`] reads it. A row whose age a
/// test or golden shows is stamped relative to this, so its age is the same on every run;
/// `row_tests::the_test_clock_starts_where_the_fixtures_expect` fails first if a GPUI upgrade moves the start.
pub(crate) const TEST_NOW: i64 = 1_751_414_398;

/// One session as the helm would push it: the helm's pinned list fixture with the fields these tests are about
/// replaced. Defaults: titled by its key, running, created and last active at 100, seen at 100 (so nothing is new), in
/// `/home/user/src/<key>`, with no annotation, on a connected host the fixture names `buildbox`, launched as the
/// fixture's Codex agent launch in YOLO with no status-line evidence, and with no notifications (the helm fixture's row
/// carries a resolved one, which would give every row a bell; a test that wants one names it). The screenshot goldens
/// build their fleet from it too.
#[derive(Clone)]
pub(crate) struct Fixture {
    pub(crate) key: &'static str,
    title: String,
    status: Value,
    created: i64,
    activity: i64,
    seen: Option<i64>,
    stale: bool,
    cwd: String,
    annotation: Option<String>,
    /// The registry id of the row's host, when the test names one; else the helm fixture's (2).
    host: Option<i64>,
    /// The helm's own name for the row's host, when the test names one; else the helm fixture's (`buildbox`).
    host_name: Option<String>,
    /// The session's launch in the helm's wire form, when the test names one; else the helm fixture's.
    launch: Option<Value>,
    /// Whether the supervisor saw the agent's status line in a mode without checks (`screen_yolo`).
    screen_yolo: bool,
    /// The session's notifications in the helm's wire form and its read mark, when the test names them; else none, and
    /// a read mark of 0.
    notifications: Option<(Value, u64)>,
    /// How many terminal tabs the row lists (`tab-0`, `tab-1`, ...); none unless a test asks.
    tabs: usize,
    /// The command line as the session recorded it, when the test names one; else the helm fixture's.
    invocation: Option<String>,
    /// The session's managed checkout in the helm's wire form (`null` for none), when the test names one; else the
    /// helm fixture's, a checkout of `octo/widgets`.
    working_copy: Option<Value>,
    /// Whether the session's launch provenance (`github_repo`) is cleared; else the helm fixture's, `octo/widgets`.
    no_launch_repository: bool,
}

pub(crate) fn session(key: &'static str) -> Fixture {
    Fixture {
        key,
        title: key.to_owned(),
        status: json!({"state": "running"}),
        created: 100,
        activity: 100,
        seen: Some(100),
        stale: false,
        cwd: format!("/home/user/src/{key}"),
        annotation: None,
        host: None,
        host_name: None,
        launch: None,
        screen_yolo: false,
        notifications: None,
        tabs: 0,
        invocation: None,
        working_copy: None,
        no_launch_repository: false,
    }
}

impl Fixture {
    /// The row lists `tabs` terminal tabs.
    pub(crate) fn tabs(mut self, tabs: usize) -> Self {
        self.tabs = tabs;
        self
    }

    pub(crate) fn title(mut self, title: &str) -> Self {
        self.title = title.to_owned();
        self
    }
    pub(crate) fn status(mut self, status: Value) -> Self {
        self.status = status;
        self
    }
    pub(crate) fn created(mut self, created: i64) -> Self {
        self.created = created;
        self
    }
    pub(crate) fn activity(mut self, activity: i64) -> Self {
        self.activity = activity;
        self
    }
    pub(crate) fn seen(mut self, seen: Option<i64>) -> Self {
        self.seen = seen;
        self
    }
    pub(crate) fn stale(mut self) -> Self {
        self.stale = true;
        self
    }
    pub(crate) fn cwd(mut self, cwd: &str) -> Self {
        self.cwd = cwd.to_owned();
        self
    }
    pub(crate) fn annotation(mut self, annotation: &str) -> Self {
        self.annotation = Some(annotation.to_owned());
        self
    }
    /// The row on the host with registry id `host`.
    pub(crate) fn host(mut self, host: i64) -> Self {
        self.host = Some(host);
        self
    }
    /// The helm's own name for the row's host (empty for a legacy row that has none).
    pub(crate) fn host_name(mut self, name: &str) -> Self {
        self.host_name = Some(name.to_owned());
        self
    }
    /// The session launched as `launch`, a `SessionLaunch` in the helm's wire form (`{"kind": "agent", ...}`).
    pub(crate) fn launch(mut self, launch: Value) -> Self {
        self.launch = Some(launch);
        self
    }
    /// The session with status-line evidence of a mode without checks, as the supervisor reports it.
    pub(crate) fn screen_yolo(mut self) -> Self {
        self.screen_yolo = true;
        self
    }
    /// The session with `notifications` (an array of `SessionNotification`s in the helm's wire form) and the helm's read
    /// mark `read_through`, as the helm lists a session whose row shows a bell.
    pub(crate) fn notifications(mut self, notifications: Value, read_through: u64) -> Self {
        self.notifications = Some((notifications, read_through));
        self
    }
    /// The session with `invocation` as its recorded command line (display only: its launch is unchanged).
    pub(crate) fn invocation(mut self, invocation: &str) -> Self {
        self.invocation = Some(invocation.to_owned());
        self
    }
    /// The session with no managed checkout (the helm fixture's row has one).
    pub(crate) fn unmanaged(mut self) -> Self {
        self.working_copy = Some(Value::Null);
        self
    }
    /// The session not launched as a managed checkout either: its launch provenance (`github_repo`), which the helm
    /// fixture's row has and a Clone starts a new checkout of, is cleared. With [`Self::unmanaged`], an ordinary folder
    /// session, as the launcher's Clone tests need.
    pub(crate) fn not_launched_as_checkout(mut self) -> Self {
        self.no_launch_repository = true;
        self
    }

    pub(crate) fn entry(&self) -> KeyedEntry {
        let mut entry = session_entry(self.key);
        let value = &mut entry.value;
        value["id"] = json!(self.key);
        value["title"] = json!(self.title);
        value["status"] = self.status.clone();
        value["created_at"] = json!(self.created);
        value["last_activity_at"] = json!(self.activity);
        value["seen_activity_at"] = json!(self.seen);
        value["stale"] = json!(self.stale);
        value["cwd"] = json!(self.cwd);
        value["annotation"] = json!(self.annotation);
        if let Some(host) = self.host {
            value["host"] = json!(host);
        }
        if let Some(name) = &self.host_name {
            value["host_name"] = json!(name);
        }
        if let Some(launch) = &self.launch {
            value["launch"] = launch.clone();
        }
        value["screen_yolo"] = json!(self.screen_yolo);
        let (notifications, read_through) =
            self.notifications.clone().unwrap_or_else(|| (json!([]), 0));
        value["notifications"] = notifications;
        value["notifications_read_through"] = json!(read_through);
        // Only the tabs the test asked for, whatever the helm fixture's row has: an open session's tabs each attach a
        // terminal of their own, which a test about the agent terminal would otherwise meet.
        value["tabs"] = json!(
            (0..self.tabs)
                .map(|tab| json!({ "id": format!("tab-{tab}") }))
                .collect::<Vec<_>>()
        );
        if let Some(invocation) = &self.invocation {
            value["invocation"] = json!(invocation);
        }
        if let Some(working_copy) = &self.working_copy {
            value["working_copy"] = working_copy.clone();
        }
        if self.no_launch_repository {
            value["github_repo"] = Value::Null;
        }
        entry
    }

    pub(super) fn row(&self) -> SessionRow {
        serde_json::from_value(self.entry().value).unwrap()
    }
}

/// The sessions collection holding `rows`, in the helm's `order`, with complete counts.
pub(crate) fn listing(order: &[&str], rows: &[Fixture]) -> CollectionState {
    CollectionState {
        collection: Collection::Sessions,
        meta: listing_meta(order, rows.len() as u64, false),
        entries: rows.iter().map(Fixture::entry).collect(),
    }
}

/// The preferences collection with this visit history and remembered last selection, from a helm that has already
/// shown the Welcome card. A helm whose preferences leave it unseen opens the card over the main view at its first
/// snapshot (`crate::welcome`), which would cover every control these fixtures' tests drive.
pub(crate) fn preferences(history: &[&str], last: Option<&str>) -> CollectionState {
    preferences_state(Preferences {
        visit_history: history.iter().map(|key| key.to_string()).collect(),
        last_selected: last.map(str::to_owned),
        welcome_seen: Some(true),
        ..Preferences::default()
    })
}

/// The preferences collection holding `preferences`, for a test that sets a field [`preferences`] does not.
pub(crate) fn preferences_state(preferences: Preferences) -> CollectionState {
    CollectionState {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::to_value(preferences).unwrap(),
        }],
    }
}

/// A change replacing the sessions collection's metadata with `meta` and upserting `rows`.
pub(crate) fn change(meta: CollectionMeta, rows: &[Fixture], remove: &[&str]) -> CollectionChanges {
    CollectionChanges {
        collection: Collection::Sessions,
        meta,
        upsert: rows.iter().map(Fixture::entry).collect(),
        remove: remove.iter().map(|key| key.to_string()).collect(),
    }
}

/// `listing(order, rows)`'s metadata alone.
pub(crate) fn listing_meta(order: &[&str], total: u64, truncated: bool) -> CollectionMeta {
    let mut meta = meta(order);
    meta.counts = Some(ListCounts {
        total,
        matching: None,
        truncated,
    });
    meta
}

/// `meta` with `key` as the session the helm carries outside the list's order because the query names it while its
/// filter or cap leaves it out (`selected_outside_query`); the key must not be in `meta`'s order.
pub(crate) fn carrying(meta: CollectionMeta, key: &str) -> CollectionMeta {
    assert!(
        !meta.order.iter().any(|listed| listed == key),
        "a carried session is outside the order"
    );
    CollectionMeta {
        selected_outside_query: Some(key.to_owned()),
        ..meta
    }
}

/// The collections a store would hold after this snapshot.
pub(crate) fn collections(snapshot: Vec<CollectionState>) -> Collections {
    let mut collections = Collections::default();
    let decoded: Vec<_> = snapshot
        .into_iter()
        .map(|state| decode_state(state).unwrap())
        .collect();
    collections.install(&decoded);
    collections
}

/// The real main view over the fake helm, connected with `snapshot`, in an active window, checked once settled.
pub(crate) fn open_main(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(snapshot);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, MainView::new);
    (fake, window)
}

/// The commands the window sent since the last call, as (path, body).
pub(crate) fn sent(fake: &mut FakeHelm) -> Vec<(String, Value)> {
    fake.commands()
        .into_iter()
        .map(|command| (command.path, command.body.unwrap_or_default()))
        .collect()
}

fn remembered(key: &str) -> (String, Value) {
    ("/api/preferences".into(), json!({ "last_selected": key }))
}

/// The session query the list sends when `key` becomes the open session under the unfiltered choice: the query in
/// force again, naming it (`filters`, "The query names the open session").
fn named(key: &str) -> (String, Value) {
    (
        farhelm_proto::ui_channel::SESSION_QUERY_PATH.into(),
        json!({ "selected": key }),
    )
}

fn seen(key: &str, activity: i64) -> (String, Value) {
    (
        format!("/api/sessions/{key}/seen"),
        json!({ "seen_activity_at": activity }),
    )
}

pub(crate) fn selected(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| local_state::selected(cx))
}

/// The keys of the rows drawn, top to bottom.
fn row_order(nodes: &[DriverNode]) -> Vec<String> {
    let mut rows: Vec<_> = nodes
        .iter()
        .filter(|node| node.role == Some(Role::Button))
        .filter_map(|node| {
            let key = node.element_id.as_deref()?.strip_prefix(ROW_ID_PREFIX)?;
            Some((node.bounds.origin.y, key.to_owned()))
        })
        .collect();
    rows.sort_by(|a, b| f32::from(a.0).total_cmp(&f32::from(b.0)));
    rows.into_iter().map(|(_, key)| key).collect()
}

/// Every line of text the frame painted, as words on screen (a sentence's isolates left out).
pub(crate) fn painted(nodes: &[DriverNode]) -> Vec<String> {
    nodes
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| painted_words(&text.text)))
        .collect()
}

/// Whether the launcher's dialog is drawn: what the list's New triggers (the heading's New, the quick switcher's New
/// row) open.
pub(crate) fn launcher_drawn(cx: &mut VisualTestContext) -> bool {
    cx.debug_bounds("launcher").is_some()
}

// ---- The rows ------------------------------------------------------------------------------------------------------

/// The helm sorts the list (the channel starts in the user's stored order), so the rows must be drawn in the order the
/// collection's metadata gives, never in the order entries arrived or by any order of the GUI's own; and a session
/// keeps its selection when a push moves it. Specified: entries pushed as a, b, c under the order c, a, b draw as c, a,
/// b; a pushed reorder to a, b, c (metadata only) redraws them so and leaves the open session open and nothing sent.
#[gpui::test]
fn rows_follow_the_pushed_order_and_the_selection_follows_its_session(cx: &mut TestAppContext) {
    let rows = [session("a"), session("b"), session("c")];
    let (mut fake, cx) = open_main(
        cx,
        vec![listing(&["c", "a", "b"], &rows), preferences(&["a"], None)],
    );
    assert_eq!(row_order(&frame(cx)), ["c", "a", "b"]);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    sent(&mut fake);

    fake.push(change(listing_meta(&["a", "b", "c"], 3, false), &[], &[]));
    cx.run_until_parked();
    assert_eq!(row_order(&frame(cx)), ["a", "b", "c"]);
    assert_eq!(selected(cx).as_deref(), Some("a"));
    assert_eq!(sent(&mut fake), [], "a reorder tells the helm nothing");
}

/// A row is where the user opens a session, and opening is also what the helm's visit history and the unseen marks are
/// built from. Specified: clicking an unopened row selects it, writes it as the `last_selected` preference, names it in
/// the session query and records its unseen output as seen at its latest activity, in that order; clicking the open row
/// again changes nothing and sends nothing.
#[gpui::test]
fn a_click_opens_a_session_remembers_it_and_marks_it_seen_once(cx: &mut TestAppContext) {
    let rows = [session("a"), session("b").activity(250).seen(Some(100))];
    let (mut fake, cx) = open_main(
        cx,
        vec![listing(&["a", "b"], &rows), preferences(&["a"], None)],
    );
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    sent(&mut fake);

    test_support::click(cx, "session-row-b");
    cx.run_until_parked();
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert_eq!(
        sent(&mut fake),
        [remembered("b"), named("b"), seen("b", 250)]
    );

    test_support::click(cx, "session-row-b");
    cx.run_until_parked();
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert_eq!(
        sent(&mut fake),
        [],
        "choosing the open session again sends nothing"
    );
}

/// The open session's row is how assistive technology tells which session the pane shows. The old app marked the open
/// row's button `aria-current`, which GPUI cannot express, so the port marks the row's list item selected instead (a
/// recorded deviation in the parity file), and nothing checked that mark until the frame record carried selection.
/// Specified: with session a open, a's list item says it is selected and b's says it is not; after a click opens b,
/// the two swap.
#[gpui::test]
fn the_open_sessions_list_item_says_it_is_selected(cx: &mut TestAppContext) {
    let rows = [session("a"), session("b")];
    let (_fake, cx) = open_main(
        cx,
        vec![listing(&["a", "b"], &rows), preferences(&["a"], None)],
    );
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    assert_eq!(
        item_selection(&frame(cx)),
        [("a".to_owned(), Some(true)), ("b".to_owned(), Some(false))]
    );

    test_support::click(cx, "session-row-b");
    cx.run_until_parked();
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    assert_eq!(
        item_selection(&frame(cx)),
        [("a".to_owned(), Some(false)), ("b".to_owned(), Some(true))]
    );
}

/// What each drawn row's list item says about selection, by session key in key order: the frame record's `selected`,
/// which is `None` only for an element that says nothing about it. Only elements with the list item role count, since
/// assistive technology reports selection on item-like roles alone, which is why the mark is on the item at all.
fn item_selection(nodes: &[DriverNode]) -> Vec<(String, Option<bool>)> {
    let mut items: Vec<_> = nodes
        .iter()
        .filter(|node| node.role == Some(Role::ListItem))
        .filter_map(|node| {
            let key = node.element_id.as_deref()?.strip_prefix(ITEM_ID_PREFIX)?;
            Some((key.to_owned(), node.selected))
        })
        .collect();
    items.sort();
    items
}

/// GPUI draws nothing for a window that is hidden, minimized or behind other windows, yet SPEC.md, "Status", records
/// the open session as seen in "a window behind other apps" the same way as in a focused one, and the selection must
/// not wait for the window to show either. So the rules run from what the list observes, never from drawing; before
/// that, they ran after each frame. Specified: a session list in no window at all, made before the first snapshot,
/// opens the latest visit once the snapshot arrives, remembers it with the helm and names it in the session query, and
/// sends the open session's seen mark when the helm pushes new output.
#[gpui::test]
fn the_list_settles_without_drawing(cx: &mut TestAppContext) {
    let (mut fake, _connection) = started(cx);
    let _list = cx.new(SessionList::new);
    fake.connect(vec![
        listing(&["a", "b"], &[session("a"), session("b")]),
        preferences(&["b"], None),
    ]);
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| local_state::selected(cx)).as_deref(),
        Some("b")
    );
    assert_eq!(sent(&mut fake), [remembered("b"), named("b")]);

    fake.push(change(
        listing_meta(&["a", "b"], 2, false),
        &[session("b").activity(150)],
        &[],
    ));
    cx.run_until_parked();
    assert_eq!(sent(&mut fake), [seen("b", 150)]);
}

/// A selection is dropped only on evidence: a session missing from a listing the helm's cap cut may just be past the
/// cut, and dropping it would close what the user had open (the old app's `absence_is_evidence`). Once a complete
/// listing shows it gone, the next one opens. Specified: with a open, a truncated push without a keeps a open and sends
/// nothing; a complete push without a opens the newest-created remaining session, remembers it and names it in the
/// session query.
#[gpui::test]
fn a_selection_moves_only_once_a_complete_listing_shows_it_gone(cx: &mut TestAppContext) {
    let rows = [
        session("a"),
        session("b").created(200),
        session("c").created(300),
    ];
    let (mut fake, cx) = open_main(
        cx,
        vec![
            listing(&["a", "b", "c"], &rows),
            preferences(&[], Some("a")),
        ],
    );
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    sent(&mut fake);

    fake.push(change(listing_meta(&["b", "c"], 500, true), &[], &["a"]));
    cx.run_until_parked();
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "a cut listing proves nothing"
    );
    assert_eq!(sent(&mut fake), []);

    fake.push(change(listing_meta(&["b", "c"], 2, false), &[], &[]));
    cx.run_until_parked();
    assert_eq!(selected(cx).as_deref(), Some("c"));
    assert_eq!(sent(&mut fake), [remembered("c"), named("c")]);
}

/// SPEC.md, "Status": the open session's output counts as seen as it arrives, but a manual "mark unread" of the open
/// session must stick until there is new output (the old app keyed its mark on the activity, never on the stamp, for
/// exactly this). A stale session's report is last-known, not new, so it is not marked. Specified: new activity on the
/// open session sends a seen mark at that activity; the helm then clearing the stamp with no new activity sends
/// nothing; new activity on the session while its host is disconnected sends nothing.
#[gpui::test]
fn new_output_on_the_open_session_is_marked_seen_but_a_manual_unread_sticks(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(
        cx,
        vec![listing(&["a"], &[session("a")]), preferences(&["a"], None)],
    );
    sent(&mut fake);
    let meta = || listing_meta(&["a"], 1, false);

    fake.push(change(meta(), &[session("a").activity(150)], &[]));
    cx.run_until_parked();
    assert_eq!(sent(&mut fake), [seen("a", 150)]);

    fake.push(change(
        meta(),
        &[session("a").activity(150).seen(None)],
        &[],
    ));
    cx.run_until_parked();
    assert_eq!(sent(&mut fake), [], "a mark-unread is not undone");

    fake.push(change(
        meta(),
        &[session("a").activity(170).seen(None).stale()],
        &[],
    ));
    cx.run_until_parked();
    assert_eq!(sent(&mut fake), [], "a stale session is not marked");
}

/// SPEC.md, "Losing the connection to the helm": after a reset the list is rebuilt from the new snapshot alone, the open
/// session stays open if it is still listed, the generic notice says the connection was lost, and Dismiss withdraws
/// it. The seen mark's memory belonged to the old connection and starts over too ("Everything else starts over"), so
/// the open session's unseen output is marked again even though its activity did not move. Specified: with b open and
/// seen at its activity, a break and a snapshot of a, b and c in which b's stamp is gone but its activity is the same
/// draws a, b, c, keeps b open, names b in the new channel's session query, sends b's seen mark at that same activity
/// and posts the notice; clicking the banner's Dismiss withdraws it.
#[gpui::test]
fn a_reset_rebuilds_the_list_keeps_a_listed_selection_and_shows_a_dismissable_notice(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(
        cx,
        vec![
            listing(&["a", "b"], &[session("a"), session("b")]),
            preferences(&["b"], None),
        ],
    );
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    sent(&mut fake);

    fake.break_channel();
    cx.run_until_parked();
    let rows = [session("a"), session("b").seen(None), session("c")];
    fake.connect(vec![
        listing(&["a", "b", "c"], &rows),
        preferences(&["b"], None),
    ]);
    cx.run_until_parked();
    let nodes = frame(cx);
    assert_eq!(row_order(&nodes), ["a", "b", "c"]);
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert_eq!(sent(&mut fake), [named("b"), seen("b", 100)]);
    assert_eq!(
        cx.update(|_, cx| notices::posted(cx)),
        [Notice::ConnectionLost]
    );

    // The click finds the banner's Dismiss where it was drawn, so it also shows the banner is on screen.
    test_support::click(cx, "notice-dismiss-ConnectionLost");
    assert_eq!(cx.update(|_, cx| notices::posted(cx)), []);
}

/// The seen mark's memory is kept per UI session ([`crate::session::PerSession`]), keyed on the session itself and
/// not only on the window going inert in between: the connection retires the old session whenever a new snapshot
/// arrives, and state made for the old one must not act for the new. Specified: with b open and seen, a second
/// snapshot delivered with no break before it, in which b's stamp is gone but its activity is unchanged, names b in the
/// new channel's session query and sends b's seen mark again.
#[gpui::test]
fn a_new_snapshot_without_a_break_starts_the_seen_memory_over(cx: &mut TestAppContext) {
    let snapshot = |b: Fixture| {
        vec![
            listing(&["a", "b"], &[session("a"), b]),
            preferences(&["b"], None),
        ]
    };
    let (mut fake, cx) = open_main(cx, snapshot(session("b")));
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    sent(&mut fake);

    fake.connect(snapshot(session("b").seen(None)));
    cx.run_until_parked();
    assert_eq!(sent(&mut fake), [named("b"), seen("b", 100)]);
}

/// A reset replaces the store under the list, which is not rebuilt, so the list has to hear the new store's changes
/// and stop hearing the retired one's (it follows the store through `session::observe_current`). The reset tests above
/// settle only on the new snapshot itself, which the connection's own notification covers; a list still subscribed to
/// the first store would pass them and then ignore every push after a reconnect. Specified: with a open, after a break
/// and a new snapshot of a, b and c that keeps a open, a complete pushed listing without a opens the newest-created
/// remaining session, c, remembers it and names it in the session query.
#[gpui::test]
fn after_a_reset_a_pushed_change_still_settles_the_selection(cx: &mut TestAppContext) {
    let snapshot = || {
        vec![
            listing(
                &["a", "b", "c"],
                &[
                    session("a"),
                    session("b").created(200),
                    session("c").created(300),
                ],
            ),
            preferences(&[], Some("a")),
        ]
    };
    let (mut fake, cx) = open_main(cx, snapshot());
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");

    fake.break_channel();
    cx.run_until_parked();
    fake.connect(snapshot());
    cx.run_until_parked();
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "the premise: a is still open under the new UI session"
    );
    sent(&mut fake);

    fake.push(change(listing_meta(&["b", "c"], 2, false), &[], &["a"]));
    cx.run_until_parked();
    assert_eq!(selected(cx).as_deref(), Some("c"));
    assert_eq!(sent(&mut fake), [remembered("c"), named("c")]);
}

/// Session titles, host names and directories are host text: a title could carry a direction override or a zero-width
/// character to pose as another session. Specified: for each spoofing string, a row titled with it paints the escaped
/// form, names its open control with the escaped form, and paints nothing holding a presentation-unsafe character;
/// a directory under a home is painted folded and escaped.
#[gpui::test]
fn a_rows_host_text_is_painted_and_named_escaped(cx: &mut TestAppContext) {
    // Fixture keys are `&'static str`; leaking a few short strings in a test is harmless.
    let keys: Vec<&'static str> = (0..SPOOFING_CORPUS.len())
        .map(|index| &*Box::leak(format!("h{index}").into_boxed_str()))
        .collect();
    let rows: Vec<_> = SPOOFING_CORPUS
        .iter()
        .zip(keys.iter().copied())
        .map(|((raw, _), key)| session(key).title(raw))
        .collect();
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
    // At the widest sidebar, so every escaped title fits its row whole beside the marks and the age: the assertions
    // below compare whole painted strings, and a title cut with an ellipsis would read as one painted unescaped.
    let preferences = preferences_state(Preferences {
        sidebar_width: Some(*farhelm_proto::ui_channel::SIDEBAR_WIDTH_RANGE.end()),
        ..Preferences::default()
    });
    let (_fake, cx) = open_main(cx, vec![listing(&order, &rows), preferences]);
    let nodes = frame(cx);
    let texts = painted(&nodes);
    for ((_, shown), key) in SPOOFING_CORPUS.iter().zip(keys.iter().copied()) {
        assert!(texts.iter().any(|text| text == shown), "{shown} is painted");
        let label = nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(&format!("{ROW_ID_PREFIX}{key}")))
            .and_then(|node| node.label.as_deref());
        assert_eq!(label, Some(*shown), "the row is named escaped");
    }
    assert!(
        !texts.iter().any(|text| text
            .chars()
            .any(farhelm_proto::text::is_presentation_unsafe)),
        "nothing painted holds a hidden character; painted: {texts:?}"
    );
    assert!(
        texts.iter().any(|text| text == "~/src/h0"),
        "the home is folded"
    );
}

/// The list is drawn uncached (the foundations decision of 2026-11-14), so what keeps it cheap is that a pushed change
/// draws it once: the selection rules run on every pushed change and, with nothing due, must not notify anything that
/// redraws the list.
/// Specified: a pushed change to a session that is not open renders the list exactly once.
#[gpui::test]
fn a_pushed_change_draws_the_list_once(cx: &mut TestAppContext) {
    let rows = [session("a"), session("b")];
    let (mut fake, cx) = open_main(
        cx,
        vec![listing(&["a", "b"], &rows), preferences(&["a"], None)],
    );
    cx.update(|_, cx| render_count::reset(cx));
    fake.push(change(
        listing_meta(&["a", "b"], 2, false),
        &[session("b").title("renamed")],
        &[],
    ));
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| render_count::count("session-list", cx)),
        1
    );
}

/// An empty fleet is announced only on proof (SPEC.md: "An unfiltered empty fleet still says 'No sessions'"; the old
/// pane's three honest states). Specified: an available, complete, empty listing paints "No sessions" in the list and
/// "No sessions — create one" in the pane, with the count 0.
#[gpui::test]
fn an_empty_fleet_says_so_in_the_list_and_the_pane(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, vec![listing(&[], &[])]);
    let texts = painted(&frame(cx));
    for line in [
        "No sessions",
        "No sessions — create one",
        "0",
        "Recently active",
    ] {
        assert!(texts.iter().any(|text| text == line), "{line}: {texts:?}");
    }
    assert_eq!(selected(cx), None);
    assert_eq!(sent(&mut fake), []);
}

// ---- The running pulse ---------------------------------------------------------------------------------------------

/// How many times the list rendered while a second passed on the executor's clock: every ticker step in that second,
/// and anything else that redrew the window.
fn list_renders_in_a_second(cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| render_count::reset(cx));
    test_support::advance(cx, Duration::from_secs(1));
    cx.update(|_, cx| render_count::count("session-list", cx))
}

/// An idle session, which never pulses.
fn idle(key: &'static str) -> Fixture {
    session(key).status(json!({"state": "idle"}))
}

/// The list is drawn uncached, so whatever wakes it redraws the whole list (and its window). The pulse caps that at
/// the shared phase's four steps a second, whatever the number of running rows, and never the ticker's ten.
/// Specified: with three running rows on connected hosts and an idle one, in an active window, a second redraws the
/// list exactly four times (ten ticks always hold four steps of 300 and 200 ms).
#[gpui::test]
fn running_rows_redraw_the_list_four_times_a_second(cx: &mut TestAppContext) {
    let rows = [session("a"), session("b"), session("c"), idle("d")];
    let (_fake, cx) = open_main(cx, vec![listing(&["a", "b", "c", "d"], &rows)]);
    assert!(
        cx.update(|window, _| window.is_window_active()),
        "premise: the window is active"
    );
    assert_eq!(list_renders_in_a_second(cx), 4);
}

/// SPEC.md, "Status", stands looping indicators still in an unfocused window "to save CPU and battery", and a running
/// session in a background window is the app's ordinary state for hours; under reduced motion nothing moves on its
/// own either. Skipping the redraws is not enough: a list that kept its pulse handle in an inactive window would keep
/// the ticker's timer firing ten times a second with nothing to show for it. Specified: with a running row in an
/// active window the tick count moves; once the window is inactive, a second leaves the tick count where it was; active
/// again, the count moves again; under reduced motion, a second moves neither the tick count nor the list's renders.
#[gpui::test]
fn the_tickers_timer_stops_while_inactive_and_under_reduced_motion(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, vec![listing(&["a"], &[session("a")])]);
    let ticks_in_a_second = |cx: &mut VisualTestContext| {
        let before = cx.update(|_, cx| ticker::ticks(cx));
        test_support::advance(cx, Duration::from_secs(1));
        cx.update(|_, cx| ticker::ticks(cx)) - before
    };
    assert!(
        ticks_in_a_second(cx) > 0,
        "premise: the running row's pulse ticks in an active window"
    );

    cx.deactivate_window();
    assert!(
        !cx.update(|window, _| window.is_window_active()),
        "premise: the window is inactive"
    );
    // The loop's last pending timer may still fire once, finding nobody to tick for; the count does not move for it.
    assert_eq!(ticks_in_a_second(cx), 0, "no ticks in an inactive window");

    cx.update(|window, _| window.activate_window());
    test_support::settle(cx);
    assert!(
        ticks_in_a_second(cx) > 0,
        "premise: active again, the pulse ticks again"
    );

    cx.update(|_, cx| {
        ticker::set_reduced_motion(true, cx);
        render_count::reset(cx);
    });
    assert_eq!(ticks_in_a_second(cx), 0, "no ticks under reduced motion");
    assert_eq!(
        cx.update(|_, cx| render_count::count("session-list", cx)),
        0,
        "no redraws under reduced motion"
    );
}

/// A handle kept after the last running row stops would redraw the list four times a second for nothing, for as long
/// as the window is open. Specified: once the helm pushes the only running row as idle, a second in an active window
/// redraws the list not once.
#[gpui::test]
fn a_row_that_stops_running_drops_the_pulse(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, vec![listing(&["a", "b"], &[session("a"), idle("b")])]);
    assert!(
        list_renders_in_a_second(cx) > 0,
        "premise: the running row pulses"
    );
    fake.push(change(
        listing_meta(&["a", "b"], 2, false),
        &[idle("a")],
        &[],
    ));
    test_support::settle(cx);
    assert_eq!(list_renders_in_a_second(cx), 0);
}

/// SPEC.md, "Status": "a session on an unreachable host shows a grey, still dot whatever its status says", because
/// motion reads as evidence that something is happening now, and nobody can know that of an unreachable host. Still
/// means no pulse handle at all, not a pulse drawn at full opacity. Specified: a list whose only running row is stale
/// redraws not once in a second in an active window, and the same row pushed again on a connected host makes it
/// redraw.
#[gpui::test]
fn a_stale_running_row_holds_no_pulse(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(
        cx,
        vec![listing(&["a", "b"], &[session("a").stale(), idle("b")])],
    );
    assert!(
        cx.update(|window, _| window.is_window_active()),
        "premise: the window is active"
    );
    assert_eq!(list_renders_in_a_second(cx), 0, "stale");

    fake.push(change(
        listing_meta(&["a", "b"], 2, false),
        &[session("a")],
        &[],
    ));
    test_support::settle(cx);
    assert!(
        list_renders_in_a_second(cx) > 0,
        "the same row pulses once its host is connected"
    );
}

// ---- The rules as pure functions -----------------------------------------------------------------------------------

/// Which listings may say "No sessions" and what the pane says: only an available, complete listing with no rows is an
/// empty fleet; a failed or cut one with no rows claims nothing, and before the list is read the pane says it is
/// loading.
#[test]
fn only_a_complete_available_empty_listing_is_an_empty_fleet() {
    let empty = collections(vec![listing(&[], &[])]);
    assert!(empty_fleet(&empty));
    assert_eq!(
        session_pane::placeholder(Some(&empty)),
        "No sessions — create one"
    );

    let mut failed = listing(&[], &[]);
    failed.meta.availability = Availability::Unavailable {
        error: "down".into(),
    };
    let failed = collections(vec![failed]);
    assert!(!empty_fleet(&failed));
    assert_eq!(session_pane::placeholder(Some(&failed)), "");

    let mut cut = listing(&[], &[]);
    cut.meta.counts = Some(ListCounts {
        total: 0,
        matching: None,
        truncated: true,
    });
    assert!(!empty_fleet(&collections(vec![cut])));

    let unread = collections(vec![preferences(&[], None)]);
    assert!(!empty_fleet(&unread));
    assert_eq!(
        session_pane::placeholder(Some(&unread)),
        "Loading sessions…"
    );
    assert_eq!(session_pane::placeholder(None), "");
}

/// The count never claims completeness for a cut list: a truncated listing trades the count for the warning, in SPEC's
/// words. Ported from the old app's `count_banner`.
#[test]
fn the_count_line_says_n_or_n_of_m_and_a_cut_list_warns() {
    let counts = |total, matching, truncated| CollectionMeta {
        counts: Some(ListCounts {
            total,
            matching,
            truncated,
        }),
        ..meta(&[])
    };
    assert_eq!(
        count_line(&counts(3, None, false), 3),
        CountLine::Count("3".into())
    );
    assert_eq!(
        count_line(&counts(5, Some(2), false), 2),
        CountLine::Count("2 of 5".into())
    );
    assert_eq!(
        count_line(&counts(700, None, true), 500),
        CountLine::Cut("Showing 500 of 700 sessions — could not read the list to the end".into())
    );
    assert_eq!(count_line(&meta(&[]), 4), CountLine::Count("4".into()));
}

/// The automatic choice (SPEC.md, "Session list"; the old `automatic_selection`). History outranks everything once it
/// holds anything, the remembered last selection counts only while history is empty, and the fallback is the newest
/// created by stamp (zero is unknown, not 1970; the first of equal stamps wins), never the excluded session.
#[test]
fn the_automatic_choice_prefers_history_then_the_last_selection_then_the_newest() {
    let rows = [
        session("a").created(0),
        session("b").created(200),
        session("c").created(300),
        session("d").created(300),
    ]
    .map(|fixture| (fixture.key, fixture.row()));
    let listed: Vec<Listed> = rows.iter().map(|(key, row)| Listed { key, row }).collect();
    let prefs = |history: &[&str], last: Option<&str>| Preferences {
        visit_history: history.iter().map(|key| key.to_string()).collect(),
        last_selected: last.map(str::to_owned),
        ..Preferences::default()
    };
    let choose = |preferences: Preferences, excluded| {
        selection::automatic(&listed, Some(&preferences), excluded)
    };
    assert_eq!(
        choose(prefs(&["gone", "b", "a"], Some("a")), None),
        Some("b")
    );
    assert_eq!(
        choose(prefs(&["gone"], Some("a")), None),
        Some("c"),
        "history rules once it exists"
    );
    assert_eq!(choose(prefs(&[], Some("a")), None), Some("a"));
    assert_eq!(
        choose(prefs(&[], None), None),
        Some("c"),
        "the first of the newest"
    );
    assert_eq!(
        choose(prefs(&["b"], None), Some("b")),
        Some("c"),
        "never the excluded one"
    );
    let unstamped: Vec<Listed> = listed[..1].to_vec();
    assert_eq!(selection::newest_created(&unstamped, None), Some("a"));
}

/// When the selection moves (the old app's auto-select effect and `absence_is_evidence`): a listed selection stays,
/// a missing one stays unless the listing proves it gone, a proven-gone one moves to the automatic choice or clears
/// when nothing is left, and nothing selected over rows opens one.
#[test]
fn a_selection_settles_only_on_evidence() {
    let rows = [session("a"), session("b")].map(|fixture| (fixture.key, fixture.row()));
    let listed: Vec<Listed> = rows.iter().map(|(key, row)| Listed { key, row }).collect();
    assert_eq!(
        selection::settle(Some("a"), &listed, true, None),
        Change::Keep
    );
    assert_eq!(
        selection::settle(Some("x"), &listed, false, None),
        Change::Keep
    );
    assert_eq!(
        selection::settle(Some("x"), &listed, true, None),
        Change::Select("a".into())
    );
    assert_eq!(
        selection::settle(None, &listed, false, None),
        Change::Select("a".into())
    );
    assert_eq!(selection::settle(Some("x"), &[], true, None), Change::Clear);
    assert_eq!(selection::settle(None, &[], true, None), Change::Keep);
}

/// The seen mark's memory (the old `mark_key`): due when the open session, its activity or its eligibility changes,
/// and then sent only for an eligible session with unseen output. Ended and stale sessions are never marked, and the
/// memory still moves for them, so their recovery is a change of its own. A hidden open session keeps the memory only
/// when it is its own, so a manual Mark unread survives a cut or filtered listing, and leaving for another hidden session
/// and coming back is still noticed. Specified: the cases below, each named by its assertion; a hidden open session is
/// due nothing; with the memory its own it stays, and its return with the same activity is due nothing; another hidden
/// session forgets the memory and sends nothing, and forgetting a forgotten memory is no change.
#[test]
fn the_seen_mark_is_due_on_a_new_session_or_activity_and_sent_only_when_eligible_and_unseen() {
    let due = |fixture: Fixture, stamped: &Stamped| {
        let row = fixture.row();
        selection::seen_due(
            Open::Shown(Listed {
                key: fixture.key,
                row: &row,
            }),
            stamped,
        )
    };
    let (memory, send) = due(session("a").activity(150), &Stamped::default()).unwrap();
    assert_eq!(send, Some(("a".to_owned(), 150)));
    assert!(
        due(session("a").activity(150).seen(None), &memory).is_none(),
        "keyed on activity, not the stamp"
    );
    assert_eq!(
        due(session("a").activity(150), &memory).map(|(_, send)| send),
        None
    );
    let (_, send) = due(session("a").activity(160).stale(), &memory).unwrap();
    assert_eq!(send, None, "stale");
    let ended = session("a")
        .activity(160)
        .status(json!({"state": "exited", "exit_code": 0}));
    assert_eq!(
        due(ended, &memory).map(|(_, send)| send),
        Some(None),
        "ended"
    );
    assert_eq!(
        due(session("b"), &memory).map(|(_, send)| send),
        Some(None),
        "nothing new"
    );
    assert!(selection::seen_due(Open::Nothing, &Stamped::default()).is_none());
    assert!(
        selection::seen_due(Open::Hidden("a"), &memory).is_none(),
        "hidden, its own memory: nothing due, the memory stays"
    );
    assert_eq!(
        selection::seen_due(Open::Hidden("b"), &memory),
        Some((Stamped::default(), None)),
        "hidden, another session's memory: forgotten, nothing sent"
    );
    assert!(selection::seen_due(Open::Hidden("b"), &Stamped::default()).is_none());
    assert!(
        due(session("a").activity(150).seen(None), &memory).is_none(),
        "shown again with the same activity: still nothing due"
    );
}

/// The old app's `abbreviate_home` cases: a personal home folds to `~`; dot segments, `/Users/Shared`, a bare account
/// and an empty account do not.
#[test]
fn only_a_personal_home_with_a_path_after_it_folds() {
    for (cwd, shown) in [
        ("/home/alice/src", "~/src"),
        ("/Users/bob/x/y", "~/x/y"),
        ("/home/alice", "/home/alice"),
        ("/home/alice/", "/home/alice/"),
        ("/home//x", "/home//x"),
        ("/home/alice/../bob/x", "/home/alice/../bob/x"),
        ("/Users/Shared/x", "/Users/Shared/x"),
        ("/srv/app", "/srv/app"),
    ] {
        assert_eq!(abbreviate_home(cwd), shown, "{cwd}");
    }
}

/// SPEC.md, "Status": "a session on an unreachable host shows a grey, still dot whatever its status says", since a
/// cached status is no evidence for a live color. Specified: on a row whose host is not connected, every live mark is
/// `--fg-2`, except on the selected row, where every mark is `--on-fill`; a connected running row keeps `--ok`.
#[test]
fn a_mark_on_a_disconnected_host_is_grey_whatever_its_status() {
    let palette = &theme::DARK;
    for live in [Live::Running, Live::Waiting, Live::IdleUnseen, Live::Idle] {
        assert_eq!(
            live.mark_color(palette, false, true),
            palette.fg_2,
            "{live:?}"
        );
        assert_eq!(
            live.mark_color(palette, true, true),
            palette.on_fill,
            "{live:?}"
        );
    }
    assert_eq!(Live::Running.mark_color(palette, false, false), palette.ok);
}

/// The pulse's look is the old `farhelm-status-pulse`: a linear dip to the minimum and back, held at each quarter's
/// start by `steps(4)`, so a step's opacity is where the stylesheet's would be at the same moment. Specified: at a 60%
/// minimum the eight steps read 1.0, 0.9, 0.8, 0.7, 0.6, 0.7, 0.8, 0.9; at 85% the dimmest is 0.85; the cycle
/// repeats past its end.
#[test]
fn the_pulse_dips_to_its_minimum_in_four_steps_and_climbs_back_in_four() {
    let levels: Vec<f32> = (0..ticker::PULSE_STEPS)
        .map(|step| pulse_opacity(step, 60))
        .collect();
    let expected = [1.0, 0.9, 0.8, 0.7, 0.6, 0.7, 0.8, 0.9];
    for (level, expected) in levels.iter().zip(expected) {
        assert!((level - expected).abs() < 1e-5, "{levels:?}");
    }
    assert!((pulse_opacity(4, 85) - 0.85).abs() < 1e-5);
    assert_eq!(pulse_opacity(ticker::PULSE_STEPS, 60), 1.0);
}

/// The opacity a mark draws at is the pulse's level only for a running mark on a connected host, at the shallower
/// minimum on the solid selection (the old sheet's `--selected-status-pulse-min`, since a deep dip loses the mark
/// against the fill). Specified, at the dimmest step: an ordinary running mark is at 0.6 in the dark appearance and
/// 0.73 in the light one, a selected one at 0.85 in either, and a stale running mark and every other live mark at 1.0.
#[test]
fn only_a_running_mark_on_a_connected_host_dims_and_less_when_selected() {
    let dimmest = ticker::PULSE_STEPS / 2;
    let opacity =
        |live: Live, palette, selected, stale| live.mark_opacity(palette, selected, stale, dimmest);
    let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
    assert!(close(
        opacity(Live::Running, &theme::DARK, false, false),
        0.6
    ));
    assert!(close(
        opacity(Live::Running, &theme::LIGHT, false, false),
        0.73
    ));
    for palette in [&theme::DARK, &theme::LIGHT] {
        assert!(close(opacity(Live::Running, palette, true, false), 0.85));
        assert_eq!(opacity(Live::Running, palette, false, true), 1.0, "stale");
        assert_eq!(opacity(Live::Running, palette, true, true), 1.0, "stale");
        for live in [Live::Waiting, Live::IdleUnseen, Live::Idle] {
            assert_eq!(opacity(live, palette, false, false), 1.0, "{live:?}");
        }
    }
}

/// The host part of a row names the host as the old app did. A sandbox or container host has no destination, so its
/// short name is empty, and drawing that gave rows a bare `:/path`; such a host shows its registry name instead.
/// Specified, over the helm's pinned host list: an aliased host shows its alias, the local host `Local`, a host with
/// neither destination nor alias its registry name, a host the registry does not list the name the helm sent with the
/// row, and with no name anywhere the label is empty (the line then shows the directory alone).
#[test]
fn a_host_without_a_short_name_shows_its_registry_name() {
    let hosts: Value = serde_json::from_str(include_str!(
        "../../../../farhelm-helm/http-contract/host-list.json"
    ))
    .unwrap();
    let host = |index: usize, bare: bool| -> HostView {
        let mut value = hosts["hosts"][index].clone();
        if bare {
            value["destination"] = Value::Null;
            value["alias"] = Value::Null;
        }
        serde_json::from_value(value).unwrap()
    };
    let row = session("a").row();
    assert_eq!(host_label(&row, Some(&host(1, false))), "alias2");
    assert_eq!(host_label(&row, Some(&host(0, false))), "Local");
    assert_eq!(host_label(&row, Some(&host(10, true))), "host11");
    assert_eq!(host_label(&row, None), "buildbox");
    let mut nameless = row.clone();
    nameless.host_name.clear();
    assert_eq!(host_label(&nameless, None), "");
}

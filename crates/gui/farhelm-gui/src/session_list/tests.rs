//! Tests of the session list: its rows and their order, the selection rules and what they tell the helm, the list's
//! and the pane's states, host text in a row, and what a reset does to all of it.
//!
//! The rules are tested as pure functions where they are pure (the automatic choice, the seen mark's memory, the count
//! line, the home folding); everything a user sees or the helm receives is tested in the real main view over the helm
//! client's fake, which hands the test every command the window sent. Rows are found by their open control's id, which
//! is the session's key, and read from GPUI's driver record of the frame (what each element painted and its
//! accessible name).

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::{Collections, collections::decode_state};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, HostView,
    KeyedEntry, ListCounts, PREFERENCES_KEY, Preferences, SessionRow,
};
use gpui::{AppContext as _, DriverNode, Role, TestAppContext, VisualTestContext};
use gpui_component::Root;
use serde_json::{Value, json};

use super::row::{ROW_ID_PREFIX, abbreviate_home, host_label};
use super::selection::{self, Change, Listed, Stamped};
use super::status::Live;
use super::{CountLine, SessionList, count_line, empty_fleet};
use crate::host_text::tests::SPOOFING_CORPUS;
use crate::main_view::MainView;
use crate::notices::{self, Notice};
use crate::session_tests::{meta, session_entry, started};
use crate::{local_state, render_count, session_pane, test_support, theme};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// One session as the helm would push it: the helm's pinned list fixture with the fields these tests are about
/// replaced. Defaults: titled by its key, running, created and last active at 100, seen at 100 (so nothing is new), in
/// `/home/user/src/<key>`, with no annotation, on a connected host the fixture names `buildbox`. The screenshot
/// goldens build their fleet from it too.
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
    }
}

impl Fixture {
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

    fn entry(&self) -> KeyedEntry {
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
        entry
    }

    fn row(&self) -> SessionRow {
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

/// The preferences collection with this visit history and remembered last selection.
pub(crate) fn preferences(history: &[&str], last: Option<&str>) -> CollectionState {
    let preferences = Preferences {
        visit_history: history.iter().map(|key| key.to_string()).collect(),
        last_selected: last.map(str::to_owned),
        ..Preferences::default()
    };
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
fn change(meta: CollectionMeta, rows: &[Fixture], remove: &[&str]) -> CollectionChanges {
    CollectionChanges {
        collection: Collection::Sessions,
        meta,
        upsert: rows.iter().map(Fixture::entry).collect(),
        remove: remove.iter().map(|key| key.to_string()).collect(),
    }
}

/// `listing(order, rows)`'s metadata alone.
fn listing_meta(order: &[&str], total: u64, truncated: bool) -> CollectionMeta {
    let mut meta = meta(order);
    meta.counts = Some(ListCounts {
        total,
        matching: None,
        truncated,
    });
    meta
}

/// The collections a store would hold after this snapshot.
fn collections(snapshot: Vec<CollectionState>) -> Collections {
    let mut collections = Collections::default();
    let decoded: Vec<_> = snapshot
        .into_iter()
        .map(|state| decode_state(state).unwrap())
        .collect();
    collections.install(&decoded);
    collections
}

/// The real main view over the fake helm, connected with `snapshot`, in an active window, checked once settled.
fn open_main(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(snapshot);
    cx.run_until_parked();
    let (_, window) = cx.add_window_view(|window, cx| {
        let main = cx.new(|cx| MainView::new(window, cx));
        Root::new(main, window, cx)
    });
    window.update(|window, _| window.activate_window());
    window.run_until_parked();
    test_support::check(window);
    (fake, window)
}

/// The commands the window sent since the last call, as (path, body).
fn sent(fake: &mut FakeHelm) -> Vec<(String, Value)> {
    fake.commands()
        .into_iter()
        .map(|command| (command.path, command.body.unwrap_or_default()))
        .collect()
}

fn remembered(key: &str) -> (String, Value) {
    ("/api/preferences".into(), json!({ "last_selected": key }))
}

fn seen(key: &str, activity: i64) -> (String, Value) {
    (
        format!("/api/sessions/{key}/seen"),
        json!({ "seen_activity_at": activity }),
    )
}

fn selected(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| local_state::selected(cx))
}

/// One frame's driver record: every element drawn, with its accessible name and the text it painted.
fn frame(cx: &mut VisualTestContext) -> Vec<DriverNode> {
    cx.update(|window, _| window.set_driver_enabled(true));
    test_support::redraw(cx);
    cx.update(|window, _| window.driver_nodes().to_vec())
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

/// Every line of text the frame painted.
fn painted(nodes: &[DriverNode]) -> Vec<String> {
    nodes
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect()
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
/// built from. Specified: clicking an unopened row selects it, writes it as the `last_selected` preference and records
/// its unseen output as seen at its latest activity, in that order; clicking the open row again changes nothing and
/// sends nothing.
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
    assert_eq!(sent(&mut fake), [remembered("b"), seen("b", 250)]);

    test_support::click(cx, "session-row-b");
    cx.run_until_parked();
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert_eq!(
        sent(&mut fake),
        [],
        "choosing the open session again sends nothing"
    );
}

/// GPUI draws nothing for a window that is hidden, minimized or behind other windows, yet SPEC.md, "Status", records
/// the open session as seen in "a window behind other apps" the same way as in a focused one, and the selection must
/// not wait for the window to show either. So the rules run from what the list observes, never from drawing; before
/// that, they ran after each frame. Specified: a session list in no window at all, made before the first snapshot,
/// opens the latest visit once the snapshot arrives and remembers it with the helm, and sends the open session's seen
/// mark when the helm pushes new output.
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
    assert_eq!(sent(&mut fake), [remembered("b")]);

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
/// nothing; a complete push without a opens the newest-created remaining session and remembers it.
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
    assert_eq!(sent(&mut fake), [remembered("c")]);
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
/// draws a, b, c, keeps b open, sends b's seen mark at that same activity and posts the notice; clicking the banner's
/// Dismiss withdraws it.
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
    assert_eq!(sent(&mut fake), [seen("b", 100)]);
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
/// snapshot delivered with no break before it, in which b's stamp is gone but its activity is unchanged, sends b's
/// seen mark again.
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
    assert_eq!(sent(&mut fake), [seen("b", 100)]);
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
    let (_fake, cx) = open_main(cx, vec![listing(&order, &rows)]);
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
/// memory still moves for them, so their recovery is a change of its own.
#[test]
fn the_seen_mark_is_due_on_a_new_session_or_activity_and_sent_only_when_eligible_and_unseen() {
    let due = |fixture: Fixture, stamped: &Stamped| {
        let row = fixture.row();
        selection::seen_due(
            Some(Listed {
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
    assert!(selection::seen_due(None, &Stamped::default()).is_none());
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

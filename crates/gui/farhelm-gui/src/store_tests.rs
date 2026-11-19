//! Tests of the store: a snapshot fills it, and a change pushed through it redraws the views that read it, also after
//! a reset replaced it; and in an agent build's staged run, the sessions a scenario names show its values.
//!
//! The helm is the helm client's fake: in process, ordered like the real channel, and moved only by the test, which is
//! what GPUI's deterministic scheduler needs. Each test stages exactly the feed it is about and lets the app drain it,
//! and checks the store against a fresh decode of the helm's whole state after each step.

use std::cell::Cell;
use std::rc::Rc;

use farhelm_helm_client::collections::decode_state;
use farhelm_helm_client::{ChannelState, PushedEntry};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, KeyedEntry, SessionRow,
};
use gpui::{Context, Entity, IntoElement, ParentElement, Render, TestAppContext, Window, div};

use crate::agent_startup::StagedValues;
use crate::host_text::HostText;
use crate::local_time::Zone;
use crate::render_count;
use crate::session;
use crate::session_list::age;
use crate::session_pane::details::Details;
use crate::store::HelmStore;
use crate::test_support::{check_store, current, meta, session_entry, sessions, started, state};
use crate::ticker;

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// A view that reaches the store through [`session::current_store`] while it draws and keeps no handle to it, the
/// pattern every view that shows helm state follows. It subscribes to nothing, so the only thing that can redraw it is
/// GPUI invalidating a window whose last frame read an entity that notified. `drawn` is how many sessions it drew last.
struct StoreReader {
    drawn: Rc<Cell<usize>>,
}

impl Render for StoreReader {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("reader", cx);
        let rows = session::current_store(cx).map_or(0, |store| {
            store.read(cx).collections().ordered::<SessionRow>().len()
        });
        self.drawn.set(rows);
        div().child(format!("{rows} sessions"))
    }
}

/// The current UI session's store, as a view would reach it.
fn current_store(cx: &mut TestAppContext) -> Option<Entity<HelmStore>> {
    cx.update(|cx| session::current_store(cx))
}

/// A change that adds the session `key` at the end of a list that held `before`.
fn adding(before: &[&str], key: &'static str) -> CollectionChanges {
    let mut order = before.to_vec();
    order.push(key);
    CollectionChanges {
        collection: Collection::Sessions,
        meta: meta(&order),
        upsert: vec![session_entry(key)],
        remove: vec![],
    }
}

// ---- The store ------------------------------------------------------------------------------------------------------

/// The store is fed only by the channel, and a change through it must redraw the views that read it: GPUI redraws only
/// for an entity that notifies, so a missed notification would leave the list showing a session that is gone
/// (SPEC_impl.md, "The store, and how views redraw"). Also checks the store against a fresh snapshot after each step.
#[gpui::test]
fn a_snapshot_fills_the_store_and_a_pushed_change_redraws_the_views_that_read_it(
    cx: &mut TestAppContext,
) {
    let (mut fake, connection) = started(cx);
    assert_eq!(state(&connection, cx), ChannelState::Connecting);
    assert!(
        current(&connection, cx).is_none(),
        "no session before the first snapshot"
    );

    fake.connect(vec![sessions(&["a", "b"])]);
    cx.run_until_parked();
    assert_eq!(state(&connection, cx), ChannelState::Connected);
    check_store(&connection, &[sessions(&["a", "b"])], cx);

    let drawn = Rc::new(Cell::new(0));
    let shown = drawn.clone();
    let _window = cx.add_window(move |_, _| StoreReader { drawn: shown });
    cx.run_until_parked();
    assert_eq!(drawn.get(), 2, "the premise: the reader drew the snapshot");
    cx.update(render_count::reset);

    fake.push(adding(&["a", "b"], "c"));
    cx.run_until_parked();
    check_store(&connection, &[sessions(&["a", "b", "c"])], cx);
    assert_eq!(
        cx.update(|cx| render_count::count("reader", cx)),
        1,
        "the change redrew the view that reads the store, once"
    );
    assert_eq!(drawn.get(), 3, "and it drew the change");
}

/// A reset replaces the store, and the views that read it are not rebuilt, so a view must still redraw when the new
/// store changes. Views look the store up at render and keep no handle (SPEC_impl.md, "The store, and how views
/// redraw"); one that kept the first store's handle, or observed it, would show the retired store forever. The reader
/// here holds no subscription, so a pass can only come from it reading the new store while it draws.
#[gpui::test]
fn a_change_pushed_after_a_reset_redraws_a_view_that_reads_the_store(cx: &mut TestAppContext) {
    let (mut fake, connection) = started(cx);
    fake.connect(vec![sessions(&["a"])]);
    cx.run_until_parked();
    let first = current_store(cx).unwrap();
    let drawn = Rc::new(Cell::new(0));
    let shown = drawn.clone();
    let _window = cx.add_window(move |_, _| StoreReader { drawn: shown });
    cx.run_until_parked();
    assert_eq!(
        drawn.get(),
        1,
        "the premise: the reader drew the first snapshot"
    );

    fake.break_channel();
    fake.connect(vec![sessions(&["a", "b"])]);
    cx.run_until_parked();
    let second = current_store(cx).unwrap();
    assert_ne!(
        first.entity_id(),
        second.entity_id(),
        "the premise: the reset replaced the store"
    );
    assert_eq!(
        drawn.get(),
        2,
        "the premise: the reader drew the new snapshot"
    );
    cx.update(render_count::reset);

    fake.push(adding(&["a", "b"], "c"));
    cx.run_until_parked();
    check_store(&connection, &[sessions(&["a", "b", "c"])], cx);
    assert_eq!(
        cx.update(|cx| render_count::count("reader", cx)),
        1,
        "the change to the new store redrew the reader, once"
    );
    assert_eq!(drawn.get(), 3, "and it drew the new store's change");
}

// ---- Staged session values (agent builds) ---------------------------------------------------------------------------

/// The session `key` as the helm's fixture row, retitled `title` (what staged values are matched by) and not stale:
/// the fixture's row is stale, and the details bar shows no age for a stale session.
fn titled(key: &str, title: &str) -> KeyedEntry {
    let mut entry = session_entry(key);
    entry.value["title"] = serde_json::json!(title);
    entry.value["stale"] = serde_json::json!(false);
    entry
}

/// A staged docs-media run shows each session's age and folder as its scenario designs them, which a fresh fleet in
/// scratch directories cannot produce for real (`agent_startup`, "Staged session values"). This is the README hero's
/// whole claim about those two values, so it must reach what the picture shows and touch nothing else.
///
/// Specified: with staged values installed, a session they name by title holds the staged folder and a last-activity
/// stamp the staged age before the anchor, from the first snapshot and again after the helm pushes it with its real
/// values; the session pane's details bar, which shows the open session's age and folder, reads `2m` and the staged
/// folder from it; and a session they do not name is exactly what the helm pushed.
#[gpui::test]
fn a_staged_session_shows_the_scenarios_age_and_folder_and_another_is_untouched(
    cx: &mut TestAppContext,
) {
    // Held for the test's length: the connection is what feeds the store.
    let (mut fake, _connection) = started(cx);
    let anchor = cx.update(|cx| age::seconds(ticker::wall_now(cx))).unwrap();
    let file =
        r#"{"sessions": [{"title": "staged", "age": 120, "cwd": "~/src/farhelm"}], "hosts": []}"#;
    cx.update(|cx| cx.set_global(StagedValues::parse(file, anchor).unwrap()));
    let snapshot = || CollectionState {
        collection: Collection::Sessions,
        meta: meta(&["a", "b"]),
        entries: vec![titled("a", "staged"), session_entry("b")],
    };
    let pushed = decode_state(snapshot()).unwrap();
    let PushedEntry::Session(real) = &pushed.entries[0].1 else {
        panic!("the fixture is a session");
    };
    assert_ne!(
        (real.info.last_activity_at, real.info.cwd.as_str()),
        (anchor - 120, "~/src/farhelm"),
        "the premise: the helm pushes other values than the staged ones"
    );

    let row = |key: &'static str, cx: &mut TestAppContext| {
        let store = current_store(cx).unwrap();
        cx.update(|cx| {
            store
                .read(cx)
                .collections()
                .get::<SessionRow>(key)
                .cloned()
                .unwrap()
        })
    };
    let untouched = |cx: &mut TestAppContext| {
        let PushedEntry::Session(expected) = &pushed.entries[1].1 else {
            panic!("the fixture is a session");
        };
        assert_eq!(&row("b", cx), expected, "a session not named is as pushed");
    };

    fake.connect(vec![snapshot()]);
    cx.run_until_parked();
    let staged = row("a", cx);
    assert_eq!(staged.info.last_activity_at, anchor - 120);
    assert_eq!(staged.info.cwd, "~/src/farhelm");
    let store = current_store(cx).unwrap();
    let collections = cx.update(|cx| store.read(cx).collections().clone());
    let details = Details::of(&staged, &collections, Some(anchor), Zone::UTC);
    assert_eq!(details.age.map(|age| age.short), Some("2m".to_owned()));
    assert_eq!(
        details.directory.shown,
        HostText::from_host("~/src/farhelm")
    );
    untouched(cx);

    // The helm pushes a session again whenever anything about it changes, always with its real values.
    fake.push(CollectionChanges {
        collection: Collection::Sessions,
        meta: meta(&["a", "b"]),
        upsert: snapshot().entries,
        remove: vec![],
    });
    cx.run_until_parked();
    let staged = row("a", cx);
    assert_eq!(
        (staged.info.last_activity_at, staged.info.cwd.as_str()),
        (anchor - 120, "~/src/farhelm"),
        "a pushed change keeps the staged values"
    );
    untouched(cx);
}

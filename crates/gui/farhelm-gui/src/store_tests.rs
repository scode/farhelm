//! Tests of the store: a snapshot fills it, and a change pushed through it redraws the views that read it, also after
//! a reset replaced it.
//!
//! The helm is the helm client's fake: in process, ordered like the real channel, and moved only by the test, which is
//! what GPUI's deterministic scheduler needs. Each test stages exactly the feed it is about and lets the app drain it,
//! and checks the store against a fresh decode of the helm's whole state after each step.

use std::cell::Cell;
use std::rc::Rc;

use farhelm_helm_client::ChannelState;
use farhelm_proto::ui_channel::{Collection, CollectionChanges, SessionRow};
use gpui::{Context, Entity, IntoElement, ParentElement, Render, TestAppContext, Window, div};

use crate::render_count;
use crate::session;
use crate::store::HelmStore;
use crate::test_support::{check_store, current, meta, session_entry, sessions, started, state};

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

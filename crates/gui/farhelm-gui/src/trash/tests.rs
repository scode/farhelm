//! Tests of the Trash button over the helm client's fake: which hosts it reads and when (on first sight, on a moved
//! `trash_epoch`, on a new connection, and only the hosts freeing space on the recheck schedule), each state of the
//! can, the bob under normal and reduced motion, a reset starting the reads over, and an old connection's answer
//! dropped.
//!
//! The fixtures at the top are shared with the dialog's tests and the screenshot goldens: three hosts (the local host,
//! the connected remote `build-box`, and `gpu-1`, which the helm cannot reach), archived checkouts as the supervisor
//! lists them, and helpers that take the fake's trash reads and answer them.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, HostView,
};
use gpui::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, div, px,
};
use serde_json::{Value, json};

use super::listing::Snapshot;
use super::{BUTTON_ID, RECHECK, TrashButton, bob_lift};
use crate::hosts::tests::{aliased, connected, connecting, host, hosts};
use crate::test_support::{self, frame, meta, started};
use crate::{root, ticker};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// The local host, connected, under the registry's own name "this machine".
pub(crate) fn local() -> HostView {
    host(1, "local", connected())
}

/// The connected remote `build-box`.
pub(crate) fn remote() -> HostView {
    aliased(
        host(2, "ssh", connected()),
        "build-box",
        "dev@build-box.invalid",
    )
}

/// `gpu-1`, which the helm is still trying to reach: listed, never read.
pub(crate) fn down() -> HostView {
    aliased(
        host(3, "ssh", connecting(2, Some("timed out"))),
        "gpu-1",
        "dev@gpu-1.invalid",
    )
}

/// `view` with its trash epoch at `epoch`, as the helm pushes it after the host's trash changed.
pub(crate) fn epoch(mut view: HostView, epoch: u64) -> HostView {
    view.trash_epoch = epoch;
    view
}

/// An archived checkout as the supervisor lists it: named `<id>-20261113T174055Z`, of `acme/web`, archived on
/// 2026-11-13 at 17:40:55 UTC, in `/work/.farhelm-trash`, taking `bytes`.
pub(crate) fn checkout(id: &str, bytes: Option<u64>) -> Value {
    json!({
        "id": id,
        "name": format!("{id}-20261113T174055Z"),
        "repository": "acme/web",
        "path": format!("/work/.farhelm-trash/{id}-20261113T174055Z"),
        "archived_at": 1_794_591_655_i64,
        "bytes": bytes,
    })
}

/// A capable host's listing of `checkouts`: it reports accepted removals as rows (`reports_retired`), and says whether
/// removal is active.
pub(crate) fn listing(checkouts: &[Value], removing: bool) -> Value {
    json!({
        "checkouts": checkouts,
        "issues": [],
        "removing": removing,
        "reports_retired": true,
    })
}

/// The commands the fake received since they were last taken.
pub(crate) fn take(helm: &mut FakeHelm) -> Vec<FakeCommand> {
    helm.commands()
}

/// The trash reads among `commands`, as (envelope id, host, whether sizes were asked for), checking that each names
/// the connection the fixtures put every host on.
pub(crate) fn reads(commands: &[FakeCommand]) -> Vec<(u64, i64, bool)> {
    commands
        .iter()
        .filter(|command| command.path == "/api/checkout-trash/list")
        .map(|command| {
            assert_eq!(command.method, Method::Post);
            let body = command.body.clone().unwrap_or_default();
            assert_eq!(body["expected_incarnation"], json!(1), "{body}");
            (
                command.id,
                body["host"].as_i64().unwrap_or_default(),
                body["measure_sizes"] == json!(true),
            )
        })
        .collect()
}

/// Answer every one of `commands` in order: a trash read with `answer(host)`'s status and body, anything else with
/// `other`'s.
pub(crate) fn answer(
    helm: &mut FakeHelm,
    commands: &[FakeCommand],
    answer: impl Fn(i64) -> (u16, Value),
    other: impl Fn(&FakeCommand) -> (u16, Value),
) {
    for command in commands {
        let (status, body) = if command.path == "/api/checkout-trash/list" {
            let host = command
                .body
                .as_ref()
                .and_then(|body| body["host"].as_i64())
                .unwrap_or_default();
            answer(host)
        } else {
            other(command)
        };
        helm.reply(command.id, status, Some(body));
    }
}

/// Answer every pending command: each trash read with `listing_of(host)`, and nothing else is expected.
pub(crate) fn answer_reads(helm: &mut FakeHelm, listing_of: impl Fn(i64) -> Value) {
    let commands = take(helm);
    answer(
        helm,
        &commands,
        |host| (200, listing_of(host)),
        |command| panic!("only trash reads were expected: {command:?}"),
    );
}

/// A root view holding one Trash button, wired as every root view is.
pub(crate) struct Harness {
    pub(crate) button: Entity<TrashButton>,
    root: root::Attachment,
}

impl Harness {
    /// A new Trash button in a root view attached to `window`, as every root view is: what [`open_button`] opens, and
    /// what a test opening a window of its own (one laid out with the real text system, say) builds itself.
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Harness {
            button: cx.new(TrashButton::new),
            root: root::attach(window, cx),
        }
    }
}

impl Render for Harness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = div().size_full().p(px(20.)).child(self.button.clone());
        root::wrap(content, &self.root, window, cx)
    }
}

/// A Trash button over the fake helm, connected with `snapshot`, in an active window, checked once settled. Its first
/// reads are out, unanswered.
pub(crate) fn open_button(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
) -> (FakeHelm, Entity<TrashButton>, &mut VisualTestContext) {
    let (mut helm, _connection) = started(cx);
    helm.connect(snapshot);
    cx.run_until_parked();
    let (harness, window) = test_support::open_view(cx, Harness::new);
    let button = window.update(|_, cx| harness.read(cx).button.clone());
    (helm, button, window)
}

/// What the button and dialog would draw now.
pub(crate) fn snapshot(button: &Entity<TrashButton>, cx: &mut VisualTestContext) -> Snapshot {
    cx.update(|_, cx| button.update(cx, |button, cx| button.snapshot(cx)))
}

/// The button's accessible name in a fresh frame.
fn button_name(cx: &mut VisualTestContext) -> String {
    test_support::label(cx, BUTTON_ID).unwrap_or_default()
}

/// Push the hosts collection holding `listed` as the helm's new state of it.
pub(crate) fn push_hosts(helm: &mut FakeHelm, listed: &[HostView]) {
    let state = hosts(listed);
    helm.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
}

/// Push that the registry could not be read, keeping the hosts listed under `order`.
pub(crate) fn push_registry_failure(helm: &mut FakeHelm, order: &[&str]) {
    helm.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: CollectionMeta {
            availability: Availability::Unavailable {
                error: "registry locked".into(),
            },
            ..meta(order)
        },
        upsert: Vec::new(),
        remove: Vec::new(),
    });
}

// ---- Reads ---------------------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md has the count event-driven, with no poll for idle hosts, and the old app read a host only
/// on its connection's first sight, its trash epoch moving, or a new connection; a read on every push would load every
/// host's supervisor whenever any session changed. Specified: on the first snapshot each connected host is read once,
/// without sizes, on its connection, and the unreachable host not at all; once answered, a push that leaves every epoch
/// alone reads nothing; a push that moves `build-box`'s epoch reads `build-box` alone; a new connection of the local
/// host reads it again.
#[gpui::test]
fn reads_follow_each_hosts_connection_and_trash_epoch_only(cx: &mut TestAppContext) {
    let (mut helm, _, cx) = open_button(cx, vec![hosts(&[local(), remote(), down()])]);
    let first = take(&mut helm);
    let sent: Vec<(i64, bool)> = reads(&first)
        .iter()
        .map(|(_, host, measure)| (*host, *measure))
        .collect();
    assert_eq!(
        sent,
        [(1, false), (2, false)],
        "one plain read per connected host"
    );
    assert_eq!(first.len(), 2, "and nothing else: {first:?}");
    answer(
        &mut helm,
        &first,
        |_| (200, listing(&[], false)),
        |_| (500, Value::Null),
    );
    test_support::settle(cx);

    let mut renamed = remote();
    renamed.alias = Some("build-box-2".into());
    push_hosts(&mut helm, &[local(), renamed.clone(), down()]);
    test_support::settle(cx);
    assert!(
        take(&mut helm).is_empty(),
        "a change that moves no epoch reads nothing"
    );

    push_hosts(&mut helm, &[local(), epoch(renamed, 7), down()]);
    test_support::settle(cx);
    let moved = take(&mut helm);
    assert_eq!(
        reads(&moved).iter().map(|read| read.1).collect::<Vec<_>>(),
        [2],
        "only the host whose trash changed is read"
    );
    answer(
        &mut helm,
        &moved,
        |_| (200, listing(&[], false)),
        |_| (500, Value::Null),
    );
    test_support::settle(cx);

    let mut reconnected = local();
    reconnected.incarnation = 2;
    push_hosts(&mut helm, &[reconnected, epoch(remote(), 7), down()]);
    test_support::settle(cx);
    let commands = take(&mut helm);
    assert_eq!(commands.len(), 1, "{commands:?}");
    let body = commands[0].body.clone().unwrap_or_default();
    assert_eq!(
        (body["host"].clone(), body["expected_incarnation"].clone()),
        (json!(1), json!(2)),
        "a new connection is read on that connection"
    );
}

/// Why this matters: SPEC.md: "The window rechecks only hosts that last reported active removal, every few seconds
/// until they report completion; no poll runs for idle hosts." Specified, with the local host freeing space and
/// `build-box` idle: nothing is read before the recheck is due; at three seconds the local host alone is read; a
/// listing still reporting removal is rechecked three seconds later; one that no longer reports it rights the can, and
/// nine more seconds read nothing.
#[gpui::test]
fn only_a_host_freeing_space_is_rechecked_until_it_reports_completion(cx: &mut TestAppContext) {
    let (mut helm, button, cx) = open_button(cx, vec![hosts(&[local(), remote()])]);
    answer_reads(&mut helm, |host| {
        listing(&[checkout("a", Some(4096))], host == 1)
    });
    test_support::settle(cx);
    assert!(
        snapshot(&button, cx).emptying(),
        "the premise: the local host reported removal"
    );

    test_support::advance(cx, RECHECK - std::time::Duration::from_millis(100));
    assert!(
        take(&mut helm).is_empty(),
        "nothing before the recheck is due"
    );
    test_support::advance(cx, std::time::Duration::from_millis(100));
    let due = take(&mut helm);
    assert_eq!(
        reads(&due).iter().map(|read| read.1).collect::<Vec<_>>(),
        [1]
    );
    answer(
        &mut helm,
        &due,
        |_| (200, listing(&[checkout("a", Some(4096))], true)),
        |_| (500, Value::Null),
    );
    test_support::settle(cx);
    assert!(snapshot(&button, cx).emptying(), "removal still reported");

    test_support::advance(cx, RECHECK);
    let again = take(&mut helm);
    assert_eq!(
        reads(&again).iter().map(|read| read.1).collect::<Vec<_>>(),
        [1],
        "rechecked again"
    );
    answer(
        &mut helm,
        &again,
        |_| (200, listing(&[], false)),
        |_| (500, Value::Null),
    );
    test_support::settle(cx);
    assert!(
        !snapshot(&button, cx).emptying(),
        "completion rights the can"
    );

    test_support::advance(cx, RECHECK * 3);
    assert!(take(&mut helm).is_empty(), "no recheck for an idle host");
}

/// Why this matters: SPEC.md: "A failed read retains the last removal report rather than implying that removal
/// finished", and a failed read means the host or its connection is in trouble; one that timed out held the window's
/// ordered command stream for the helm's whole 20-second host-wait bound, so the window must not keep sending more
/// (the other-dialogs area's coordinator's decision of 2026-11-15). Specified, with the local host freeing space: its
/// recheck read answered with a failure keeps the can over, and well past the recheck interval nothing more is read;
/// once the helm pushes the host's moved trash epoch, the host is read again.
#[gpui::test]
fn a_failed_recheck_ends_the_rechecks_until_the_trash_epoch_moves(cx: &mut TestAppContext) {
    let (mut helm, button, cx) = open_button(cx, vec![hosts(&[local(), remote()])]);
    answer_reads(&mut helm, |host| {
        listing(&[checkout("a", Some(4096))], host == 1)
    });
    test_support::advance(cx, RECHECK);
    let due = take(&mut helm);
    assert_eq!(
        reads(&due).iter().map(|read| read.1).collect::<Vec<_>>(),
        [1],
        "the premise: the recheck went out"
    );
    answer(
        &mut helm,
        &due,
        |_| (504, json!("the host did not answer in time")),
        |_| (500, Value::Null),
    );
    test_support::settle(cx);
    assert!(
        snapshot(&button, cx).emptying(),
        "a failed read keeps the last removal report"
    );

    test_support::advance(cx, RECHECK * 4);
    assert!(take(&mut helm).is_empty(), "no recheck after a failed read");
    assert!(snapshot(&button, cx).emptying(), "and the can stays over");

    push_hosts(&mut helm, &[epoch(local(), 1), remote()]);
    test_support::settle(cx);
    let read = take(&mut helm);
    assert_eq!(
        reads(&read).iter().map(|read| read.1).collect::<Vec<_>>(),
        [1],
        "the moved epoch reads the host"
    );
}

/// Why this matters: the trash reads are this window's own state, which SPEC_impl.md has a reset drop with everything
/// else the UI session held; a listing or removal report carried into the new session would show the old connection's
/// trash as current. Specified: with the local host's listing in and its removal reported, a broken and reopened channel
/// reads each connected host once for the new session, and until those answer nothing is counted and the can is
/// upright.
#[gpui::test]
fn a_reset_starts_the_reads_over(cx: &mut TestAppContext) {
    let (mut helm, button, cx) = open_button(cx, vec![hosts(&[local(), remote()])]);
    answer_reads(&mut helm, |host| {
        listing(&[checkout("a", Some(1))], host == 1)
    });
    test_support::settle(cx);
    assert_eq!(
        snapshot(&button, cx).count(),
        (2, false),
        "the premise: listings are in"
    );
    assert!(
        snapshot(&button, cx).emptying(),
        "the premise: removal is reported"
    );

    helm.break_channel();
    helm.connect(vec![hosts(&[local(), remote()])]);
    test_support::settle(cx);
    let fresh = take(&mut helm);
    assert_eq!(
        reads(&fresh).iter().map(|read| read.1).collect::<Vec<_>>(),
        [1, 2],
        "the new session reads every connected host"
    );
    let shown = snapshot(&button, cx);
    assert_eq!(shown.count(), (0, false), "nothing carried over");
    assert!(!shown.emptying(), "nor the removal report");
}

/// Why this matters: a host's reads are matched to their answers by number, and the entry kept for a host is made anew
/// when the host reconnects; an answer from the old connection that came back after the reconnect must not be taken for
/// the new connection's listing (it lists the old connection's trash, and would settle the new connection's guards).
/// Specified: with the local host's first read still out, a push of a new connection reads the host again; the old
/// read's answer, holding a checkout, is then dropped (the host still counts nothing and is still being read), and the
/// new read's answer is kept.
#[gpui::test]
fn an_answer_from_a_hosts_old_connection_is_dropped(cx: &mut TestAppContext) {
    let (mut helm, button, cx) = open_button(cx, vec![hosts(&[local()])]);
    let old = take(&mut helm);
    assert_eq!(old.len(), 1, "the premise: the first read is out");
    let mut reconnected = local();
    reconnected.incarnation = 2;
    push_hosts(&mut helm, &[reconnected]);
    test_support::settle(cx);
    let new = take(&mut helm);
    assert_eq!(new.len(), 1, "the new connection is read");
    assert_eq!(
        new[0].body.clone().unwrap_or_default()["expected_incarnation"],
        json!(2)
    );

    answer(
        &mut helm,
        &old,
        |_| (200, listing(&[checkout("old", Some(1))], false)),
        |_| (500, Value::Null),
    );
    test_support::settle(cx);
    let shown = snapshot(&button, cx);
    assert_eq!(
        shown.count(),
        (0, false),
        "the old connection's answer is dropped"
    );
    assert!(shown.hosts[0].pending, "and the new read is still awaited");

    answer(
        &mut helm,
        &new,
        |_| (200, listing(&[checkout("new", Some(1))], false)),
        |_| (500, Value::Null),
    );
    test_support::settle(cx);
    assert_eq!(
        snapshot(&button, cx).count(),
        (1, false),
        "the new connection's answer is kept"
    );
}

// ---- The can -------------------------------------------------------------------------------------------------------

/// Why this matters: the can is how the user sees, without opening anything, whether archived checkouts take space on
/// reachable hosts (SPEC.md: "an outline can when empty and an open-lid can and count when it holds checkouts on
/// reachable hosts"), and an unread host must not make a partial count look complete. Specified: before any answer
/// the button names a known count with hosts unchecked, can outlined; with both hosts' listings empty it is "empty";
/// with three checkouts across them the can is open and named "3 archived checkouts"; an unreachable host's archives
/// are never counted.
#[gpui::test]
fn the_can_is_outlined_when_empty_and_open_with_a_count_when_full(cx: &mut TestAppContext) {
    let (mut helm, button, cx) = open_button(cx, vec![hosts(&[local(), remote(), down()])]);
    assert_eq!(
        button_name(cx),
        "Checkout trash, 0 known archived checkouts, some entries or hosts unchecked"
    );
    assert!(
        !snapshot(&button, cx).summary().0,
        "outlined while nothing is known"
    );
    answer_reads(&mut helm, |_| listing(&[], false));
    test_support::settle(cx);
    assert_eq!(button_name(cx), "Checkout trash, empty");
    assert!(!snapshot(&button, cx).summary().0);

    push_hosts(&mut helm, &[epoch(local(), 1), epoch(remote(), 1), down()]);
    test_support::settle(cx);
    answer_reads(&mut helm, |host| {
        if host == 1 {
            listing(&[checkout("a", Some(1)), checkout("b", None)], false)
        } else {
            listing(&[checkout("c", Some(1))], false)
        }
    });
    test_support::settle(cx);
    assert_eq!(button_name(cx), "Checkout trash, 3 archived checkouts");
    let shown = snapshot(&button, cx);
    assert!(shown.summary().0, "the open-lid can");
    assert_eq!(shown.count(), (3, false));
    assert!(!shown.emptying(), "upright while nothing is freed");
    let count = frame(cx)
        .into_iter()
        .any(|node| node.texts.iter().any(|text| text.text.as_ref() == "3"));
    assert!(count, "the count is drawn beside the can");
}

/// Why this matters: SPEC.md has the can turn upside down and bob while space is freed, but stand still, upside down,
/// under Reduce Motion, and the ticker caps animation at ten ticks a second. Specified: the bob's steps over one second
/// are the old keyframes' (up three pixels and back, eight steps); with a host freeing space and normal motion the can
/// is lifted at some tick within a second; under reduced motion it is upside down (emptying) but never lifted, and the
/// ticker's count does not move.
#[gpui::test]
fn the_can_bobs_while_space_is_freed_and_stands_still_under_reduced_motion(
    cx: &mut TestAppContext,
) {
    let steps: Vec<f32> = (0..10).map(bob_lift).collect();
    assert_eq!(steps, [0., 0., 0.75, 1.5, 2.25, 3., 3., 2.25, 1.5, 0.75]);

    let (mut helm, button, cx) = open_button(cx, vec![hosts(&[local()])]);
    answer_reads(&mut helm, |_| listing(&[checkout("a", Some(1))], true));
    test_support::settle(cx);
    assert!(
        snapshot(&button, cx).emptying(),
        "the premise: space is being freed"
    );
    let mut lifted = false;
    for _ in 0..10 {
        test_support::advance(cx, ticker::TICK);
        lifted |= cx
            .update(|_, cx| button.read(cx).lift(cx))
            .is_some_and(|lift| lift > 0.);
    }
    assert!(lifted, "the can bobs");

    cx.update(|_, cx| ticker::set_reduced_motion(true, cx));
    // Right the can for a frame, so the bob drawn next starts under reduced motion.
    push_hosts(&mut helm, &[epoch(local(), 1)]);
    test_support::settle(cx);
    answer_reads(&mut helm, |_| listing(&[], false));
    test_support::settle(cx);
    assert!(
        !snapshot(&button, cx).emptying(),
        "the premise: the can is upright"
    );
    assert_eq!(
        cx.update(|_, cx| button.read(cx).lift(cx)),
        None,
        "and holds no bob"
    );
    push_hosts(&mut helm, &[epoch(local(), 2)]);
    test_support::settle(cx);
    answer_reads(&mut helm, |_| listing(&[checkout("a", Some(1))], true));
    test_support::settle(cx);
    assert!(
        snapshot(&button, cx).emptying(),
        "upside down under reduced motion too"
    );
    let before = cx.update(|_, cx| ticker::ticks(cx));
    test_support::advance(cx, std::time::Duration::from_secs(1));
    assert_eq!(
        cx.update(|_, cx| ticker::ticks(cx)),
        before,
        "nothing ticks"
    );
    assert_eq!(
        cx.update(|_, cx| button.read(cx).lift(cx)),
        Some(0.),
        "and the can stays put"
    );
}

/// Why this matters: SPEC.md, "Managed checkouts": "The session list's header places Trash, Templates and New in that
/// order", and "Session list" makes them compact icon buttons beside the title, which the heading draws as one
/// right-aligned group of 24-pixel controls. Specified, in the main window over one connected host holding one archived
/// checkout: the Trash button is drawn right of the order title and left of Templates, as high as Templates (24
/// pixels) with its top level with Templates', and named for what the trash holds.
#[gpui::test]
fn the_list_heading_places_trash_before_templates(cx: &mut TestAppContext) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![hosts(&[local()]), test_support::sessions(&[])]);
    cx.run_until_parked();
    let (_main, cx) = test_support::open_view(cx, crate::main_view::MainView::new);
    let commands = take(&mut helm);
    answer(
        &mut helm,
        &commands,
        |_| (200, listing(&[checkout("a", Some(1))], false)),
        |_| (200, Value::Null),
    );
    test_support::settle(cx);
    let nodes = frame(cx);
    let bounds = |id: &str| match nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
    {
        Some(node) => node.bounds,
        None => panic!("{id} was not drawn"),
    };
    let (title, trash, templates) = (
        bounds("session-order"),
        bounds(BUTTON_ID),
        bounds("templates-button"),
    );
    assert!(trash.left() >= title.right(), "Trash is right of the title");
    assert!(trash.right() <= templates.left(), "and left of Templates");
    assert_eq!(
        trash.size.height, templates.size.height,
        "as high as Templates"
    );
    assert_eq!(trash.size.height, px(24.));
    assert_eq!(
        trash.top(),
        templates.top(),
        "on the heading's row with Templates"
    );
    assert_eq!(
        test_support::label(cx, BUTTON_ID).as_deref(),
        Some("Checkout trash, 1 archived checkout")
    );
}

/// Why this matters: the operation store's two cross-area sets decide what a launch and a host registry change wait
/// for, and `operations.rs` asks the PR that adds a kind of work to pin its answers in its own tests. A trash delete
/// neither starts a session nor changes the registry, so neither side may refuse while one runs (SPEC.md: "Trash
/// deletes do not block other actions"). Specified: `Work::TrashDelete` is in neither set.
#[test]
fn a_trash_delete_neither_creates_a_session_nor_changes_the_registry() {
    use crate::operations::Work;
    assert!(!Work::TrashDelete.creates_a_session());
    assert!(!Work::TrashDelete.changes_host_registry());
}

//! Tests of the Trash dialog over the helm client's fake: opening and its measured reads, focus and Tab order, what
//! each host's card shows, an entry's immediate Delete and Retry and their answers, the settle pause after rows move,
//! and the refusals the dialog keeps (and when it lets them go).
//!
//! The dialog opens from the button's own press, as the user opens it, and the tests read what it drew from GPUI's
//! driver record of the frame by element id (`trash-entry-delete-<host>-<id>`, `trash-host-totals-<host>`,
//! `trash-notice-<host>-<n>-message` and so on). The fixtures are the button tests'.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::ui_channel::{Collection, CollectionChanges, HostView};
use gpui::{DriverNode, Pixels, TestAppContext, VisualTestContext, px, size};
use serde_json::{Value, json};

use super::SETTLE;
use crate::focus;
use crate::local_time;
use crate::operations::{Command, OpKey, Work};
use crate::session;
use crate::test_support::{self, focused_id, frame, meta};
use crate::trash::tests::{
    answer, answer_reads, checkout, down, epoch, listing, local, open_button, push_hosts,
    push_registry_failure, reads, remote, snapshot, take,
};
use crate::trash::{BUTTON_ID, Refused};

/// The node drawn under `id`, if any.
fn node<'a>(nodes: &'a [DriverNode], id: &str) -> Option<&'a DriverNode> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
}

/// The accessible name drawn under `id`; fails when nothing with a name was drawn there.
fn label(nodes: &[DriverNode], id: &str) -> String {
    match node(nodes, id).and_then(|node| node.label.as_deref()) {
        Some(label) => label.to_owned(),
        None => panic!("{id} was not drawn with a name"),
    }
}

/// Whether the control under `id` was drawn and can be pressed: an unavailable button tracks no focus.
fn enabled(nodes: &[DriverNode], id: &str) -> bool {
    match node(nodes, id) {
        Some(node) => node.flags.focusable,
        None => panic!("{id} was not drawn"),
    }
}

/// Whether the control under `id` was drawn and acts when pressed. Unlike [`enabled`], this tells a busy button, which
/// stays focusable but does nothing, from an available one: the asking buttons are drawn busy while unavailable.
fn pressable(nodes: &[DriverNode], id: &str) -> bool {
    match node(nodes, id) {
        Some(node) => node.flags.click,
        None => panic!("{id} was not drawn"),
    }
}

/// Whether some element painted exactly `text`.
fn painted(nodes: &[DriverNode], text: &str) -> bool {
    nodes
        .iter()
        .any(|node| node.texts.iter().any(|line| line.text.as_ref() == text))
}

/// The local host's two checkouts (`a` measured, `b` not) and `build-box`'s one, measured.
fn archives(host: i64) -> Value {
    match host {
        1 => listing(&[checkout("a", Some(4096)), checkout("b", None)], false),
        _ => listing(&[checkout("c", Some(48_234_496))], false),
    }
}

/// The three fixture hosts with their archives read, and the dialog opened from the button with its measured reads
/// answered and the settle pause over: where most tests start.
fn opened(
    cx: &mut TestAppContext,
) -> (
    FakeHelm,
    gpui::Entity<crate::trash::TrashButton>,
    &mut VisualTestContext,
) {
    let (mut helm, button, cx) = open_button(
        cx,
        vec![crate::hosts::tests::hosts(&[local(), remote(), down()])],
    );
    answer_reads(&mut helm, archives);
    test_support::settle(cx);
    test_support::click(cx, BUTTON_ID);
    answer_reads(&mut helm, archives);
    test_support::advance(cx, SETTLE);
    (helm, button, cx)
}

/// The one command sent since the last were taken, which must be a trash delete: its id and body.
fn one_delete(helm: &mut FakeHelm) -> (u64, Value) {
    test_support::one_command(helm, Method::Post, "/api/checkout-trash/delete")
}

/// Push the hosts collection without host `gone`, keeping `kept`, as a readable registry that dropped it.
fn drop_host(helm: &mut FakeHelm, kept: &[HostView], gone: i64) {
    let keys: Vec<String> = kept.iter().map(|host| HostView::key(host.id)).collect();
    let order: Vec<&str> = keys.iter().map(String::as_str).collect();
    helm.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&order),
        upsert: Vec::new(),
        remove: vec![HostView::key(gone)],
    });
}

/// Why this matters: SPEC.md has the dialog measure sizes when it opens, start on footer Close and Tab through header
/// close, the archive list, then the footer, and an entry's Delete is unavailable while its host's listing is not
/// settled. Specified: pressing the button opens the dialog with focus on Close and one sized read per connected host
/// (none for the unreachable one); while those reads are out every entry's Delete is unavailable, past the settle pause
/// too, and once answered (and the rows have settled) they are available; Tab from Close goes to the footer's Delete
/// all, then round to the header's close, the first host's "Delete all on", and its first entry's Delete.
#[gpui::test]
fn opening_measures_each_reachable_host_and_starts_on_footer_close(cx: &mut TestAppContext) {
    let (mut helm, _, cx) = open_button(
        cx,
        vec![crate::hosts::tests::hosts(&[local(), remote(), down()])],
    );
    answer_reads(&mut helm, archives);
    test_support::settle(cx);
    test_support::click(cx, BUTTON_ID);
    let measured = take(&mut helm);
    assert_eq!(
        reads(&measured)
            .iter()
            .map(|read| (read.1, read.2))
            .collect::<Vec<_>>(),
        [(1, true), (2, true)]
    );
    assert_eq!(focused_id(cx).as_deref(), Some("trash-close"));
    // Past the settle pause, so only the reads still out hold the Deletes.
    test_support::advance(cx, SETTLE);
    let nodes = frame(cx);
    assert!(
        !enabled(&nodes, "trash-entry-delete-1-a"),
        "unavailable while measuring"
    );
    assert_eq!(
        label(&nodes, "trash-host-totals-1"),
        "2 checkouts · Measuring…"
    );

    answer(
        &mut helm,
        &measured,
        |host| (200, archives(host)),
        |_| (500, Value::Null),
    );
    test_support::advance(cx, SETTLE);
    let nodes = frame(cx);
    assert!(enabled(&nodes, "trash-entry-delete-1-a"));
    assert!(enabled(&nodes, "trash-entry-delete-2-c"));
    test_support::keys(cx, "tab");
    assert_eq!(focused_id(cx).as_deref(), Some("trash-delete-all"));
    test_support::keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("checkout-trash-dialog-close")
    );
    test_support::keys(cx, "tab");
    assert_eq!(focused_id(cx).as_deref(), Some("trash-host-delete-1"));
    test_support::keys(cx, "tab");
    assert_eq!(focused_id(cx).as_deref(), Some("trash-entry-delete-1-a"));
}

/// Why this matters: the dialog is where the user decides what to copy out before cleanup, so each archive must name
/// its repository, archive time, size and folder, totals that miss a size must say they are partial, and an
/// unreachable host must stay visible without offering anything (SPEC.md). Specified: the local host's card names it
/// as the registry does, counts two checkouts with a partial size, and shows checkout `a`'s name, repository, archive
/// time (to the minute in the viewer's local time, the app's zone pinned eight hours west of UTC here) and the folder
/// it is in; the header's total covers the reachable hosts and says partial; `gpu-1`'s card says it
/// is not reachable and has no Delete, its "Delete all on" unavailable; the local host's is named for it.
#[gpui::test]
fn each_host_card_shows_its_archives_and_an_unreachable_host_offers_nothing(
    cx: &mut TestAppContext,
) {
    let (_helm, _, cx) = opened(cx);
    cx.update(|_, cx| local_time::pin(-8 * 3_600, cx));
    test_support::redraw(cx);
    let nodes = frame(cx);
    assert_eq!(label(&nodes, "trash-host-name-1"), "this machine");
    assert_eq!(
        label(&nodes, "trash-host-totals-1"),
        "2 checkouts · 4.0 KiB measured · partial"
    );
    assert_eq!(
        label(&nodes, "trash-host-totals-2"),
        "1 checkout · 46.0 MiB"
    );
    assert_eq!(label(&nodes, "trash-entry-name-1-a"), "a-20261113T174055Z");
    assert_eq!(label(&nodes, "trash-entry-repository-1-a"), "acme/web");
    assert_eq!(
        label(&nodes, "trash-entry-archived-1-a"),
        "Archived 2026-11-13 09:40"
    );
    assert_eq!(
        label(&nodes, "trash-folder-1-0"),
        "On disk: /work/.farhelm-trash"
    );
    assert!(painted(
        &nodes,
        "3 checkouts · 46.0 MiB measured · partial on reachable hosts"
    ));
    assert_eq!(
        label(&nodes, "trash-unreachable-3"),
        "Not reachable, so they can't be checked or deleted"
    );
    assert!(
        !nodes.iter().any(|node| node
            .element_id
            .as_deref()
            .is_some_and(|id| id.starts_with("trash-entry-delete-3-"))),
        "no Delete for an unreachable host"
    );
    assert!(!pressable(&nodes, "trash-host-delete-3"));
    assert_eq!(
        label(&nodes, "trash-host-delete-1"),
        "Delete all on this machine"
    );
    assert!(node(&nodes, "trash-empty").is_none());
}

/// Why this matters: an empty trash must say so rather than show nothing. Specified: with every reachable host's
/// listing empty, the dialog says "The trash is empty on reachable hosts." and draws no card for them.
#[gpui::test]
fn an_empty_trash_says_so(cx: &mut TestAppContext) {
    let (mut helm, _, cx) = open_button(cx, vec![crate::hosts::tests::hosts(&[local()])]);
    answer_reads(&mut helm, |_| listing(&[], false));
    test_support::click(cx, BUTTON_ID);
    answer_reads(&mut helm, |_| listing(&[], false));
    test_support::settle(cx);
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "trash-empty"),
        "The trash is empty on reachable hosts."
    );
    assert!(node(&nodes, "trash-host-1").is_none());
}

/// Why this matters: an entry's own Delete removes one checkout at once and keeps the dialog open; the trash allows one
/// delete of its own at a time but must not block the host's other actions; and a capable host's accepted row stays,
/// marked removing, until a later listing confirms it gone, with the listing read again once the delete settles
/// (SPEC.md). Specified: pressing `a`'s Delete sends one delete naming `a` on its host's connection and keeps the
/// dialog open; while it is out (past the settle pause, so only the delete holds them) every entry's Delete is
/// unavailable, both Close controls are not, the host's trash key is busy and its own key is not, and the can is over.
/// The helm pushes the host's advanced trash epoch before it replies, as it does for an accepted delete, and that push
/// reads the host once; the reply itself reads nothing more, and `a`'s row reads "Removing…", unavailable; a listing
/// without it removes the row, and the count follows.
#[gpui::test]
fn an_entrys_delete_runs_at_once_and_keeps_its_row_until_a_listing_confirms_it_gone(
    cx: &mut TestAppContext,
) {
    let (mut helm, button, cx) = opened(cx);
    test_support::click(cx, "trash-entry-delete-1-a");
    let (id, body) = one_delete(&mut helm);
    assert_eq!(
        body,
        json!({ "host": 1, "expected_incarnation": 1, "ids": ["a"] })
    );
    assert!(test_support::any_open(cx), "the dialog stays open");
    test_support::advance(cx, SETTLE);
    let nodes = frame(cx);
    for control in ["trash-entry-delete-1-b", "trash-entry-delete-2-c"] {
        assert!(!enabled(&nodes, control), "{control} waits for the delete");
    }
    assert!(enabled(&nodes, "trash-close") && enabled(&nodes, "checkout-trash-dialog-close"));
    let (trash_key, host_key) = cx.update(|_, cx| {
        let operations = session::current_operations(cx).unwrap();
        let operations = operations.read(cx);
        (
            operations.in_flight(&OpKey::trash("1")),
            operations.in_flight(&OpKey::host("1")),
        )
    });
    assert_eq!((trash_key, host_key), (Some(Work::TrashDelete), None));
    assert!(snapshot(&button, cx).emptying());

    push_hosts(&mut helm, &[epoch(local(), 1), remote(), down()]);
    test_support::settle(cx);
    let reread = take(&mut helm);
    assert_eq!(
        reads(&reread)
            .iter()
            .map(|read| (read.1, read.2))
            .collect::<Vec<_>>(),
        [(1, true)],
        "the pushed epoch reads the host"
    );
    helm.reply(id, 200, Some(json!({ "removed": ["a"], "issues": [] })));
    test_support::settle(cx);
    assert!(take(&mut helm).is_empty(), "the reply reads nothing more");
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "trash-entry-delete-1-a"),
        "Removing a-20261113T174055Z"
    );
    assert!(!enabled(&nodes, "trash-entry-delete-1-a"));
    assert!(node(&nodes, "trash-entry-removing-1-a").is_some());

    answer(
        &mut helm,
        &reread,
        |_| (200, listing(&[checkout("b", None)], true)),
        |_| (500, Value::Null),
    );
    test_support::advance(cx, SETTLE);
    let nodes = frame(cx);
    assert!(
        node(&nodes, "trash-entry-delete-1-a").is_none(),
        "the listing confirmed it gone"
    );
    assert!(enabled(&nodes, "trash-entry-delete-1-b"));
    assert_eq!(snapshot(&button, cx).count(), (2, false));
}

/// Why this matters: SPEC.md: "Close, header close and Escape remain available during that request", and a delete the
/// helm took is work of the window's operation store, not of the dialog, so closing must not cancel it (the old "one
/// trash entry deletes immediately and preserves neighbours", which dismissed the dialog with its request held).
/// Specified: Escape closes the dialog while `a`'s delete is out, the can stays over while it runs, and its answer
/// still reads the host again, without sizes now that the dialog is closed, and rights the can once that read says no
/// removal is active.
#[gpui::test]
fn closing_the_dialog_leaves_a_running_delete_to_finish(cx: &mut TestAppContext) {
    let (mut helm, button, cx) = opened(cx);
    test_support::click(cx, "trash-entry-delete-1-a");
    let (id, _) = one_delete(&mut helm);
    test_support::keys(cx, "escape");
    assert!(
        !test_support::any_open(cx),
        "Escape closed the dialog during the delete"
    );
    assert!(snapshot(&button, cx).emptying(), "the delete still runs");
    helm.reply(id, 200, Some(json!({ "removed": ["a"], "issues": [] })));
    test_support::settle(cx);
    let reread = take(&mut helm);
    assert_eq!(
        reads(&reread)
            .iter()
            .map(|read| (read.1, read.2))
            .collect::<Vec<_>>(),
        [(1, false)],
        "the host is read again, without sizes"
    );
    answer(
        &mut helm,
        &reread,
        |_| (200, listing(&[checkout("b", None)], false)),
        |_| (500, Value::Null),
    );
    test_support::settle(cx);
    assert!(!snapshot(&button, cx).emptying());
    assert_eq!(snapshot(&button, cx).count(), (2, false));
}

/// Why this matters: the helm bounds its wait on a host at 20 seconds and then answers a delete with a timeout (504)
/// while the cleanup goes on and holds the host's trash gate; a read sent then would wait the whole bound too, and since
/// the helm answers the window's commands one at a time, every later command would wait behind it (the other-dialogs
/// area's coordinator's decision of 2026-11-15). Specified: a delete answered 504 keeps its notice but sends no read,
/// also once its settle pause and a recheck's interval have passed; when the helm pushes the host's moved trash epoch,
/// as it does once the late cleanup lands, the host is read once.
#[gpui::test]
fn a_timed_out_delete_reads_nothing_until_the_trash_epoch_moves(cx: &mut TestAppContext) {
    let (mut helm, button, cx) = opened(cx);
    test_support::click(cx, "trash-entry-delete-1-a");
    let (id, _) = one_delete(&mut helm);
    helm.reply(
        id,
        504,
        Some(json!(
            "the host did not answer in time; its cleanup goes on"
        )),
    );
    test_support::advance(cx, crate::trash::RECHECK * 2);
    assert!(
        take(&mut helm).is_empty(),
        "no read after a timed-out delete"
    );
    assert!(
        snapshot(&button, cx).has_refusals(),
        "the premise: its notice is kept"
    );

    push_hosts(&mut helm, &[epoch(local(), 1), remote(), down()]);
    test_support::settle(cx);
    let read = take(&mut helm);
    assert_eq!(
        reads(&read).iter().map(|read| read.1).collect::<Vec<_>>(),
        [1],
        "the moved epoch reads the host"
    );
}

/// Why this matters: SPEC.md: "Refusals the helm reports remain visible on the next dialog opening, and the trash
/// button's tooltip tells the user to open the trash to see why", and a later delete of another checkout must not
/// settle that refusal. Specified: a delete the helm answers with a timeout leaves a notice under the host saying the
/// reply could not be confirmed, with the helm's words, and the button's help points into the dialog; Escape closes the
/// dialog and returns focus to the button; reopened, the notice is still there; a successful delete of `b` keeps it.
#[gpui::test]
fn a_refused_delete_stays_visible_on_the_next_opening(cx: &mut TestAppContext) {
    let (mut helm, button, cx) = opened(cx);
    test_support::click(cx, "trash-entry-delete-1-a");
    let (id, _) = one_delete(&mut helm);
    helm.reply(
        id,
        504,
        Some(json!("the host timed out; its cleanup goes on")),
    );
    test_support::settle(cx);
    answer_reads(&mut helm, archives);
    test_support::advance(cx, SETTLE);
    let notice = "The cleanup reply could not be confirmed; some folders may already be deleted. Rechecking the \
                  trash: the host timed out; its cleanup goes on";
    assert_eq!(label(&frame(cx), "trash-notice-1-0-message"), notice);
    assert!(
        snapshot(&button, cx)
            .help()
            .accessible_name()
            .ends_with("Open the trash to see why.")
    );

    test_support::keys(cx, "escape");
    assert!(!test_support::any_open(cx), "the premise: Escape closed it");
    assert_eq!(focused_id(cx).as_deref(), Some(BUTTON_ID));
    test_support::click(cx, BUTTON_ID);
    answer_reads(&mut helm, archives);
    test_support::advance(cx, SETTLE);
    assert_eq!(label(&frame(cx), "trash-notice-1-0-message"), notice);

    test_support::click(cx, "trash-entry-delete-1-b");
    let (id, _) = one_delete(&mut helm);
    helm.reply(id, 200, Some(json!({ "removed": ["b"], "issues": [] })));
    test_support::settle(cx);
    answer_reads(&mut helm, |_| listing(&[checkout("a", Some(4096))], false));
    test_support::advance(cx, SETTLE);
    assert_eq!(
        label(&frame(cx), "trash-notice-1-0-message"),
        notice,
        "another delete keeps it"
    );
}

/// Why this matters: SPEC.md lists a failed removal with its reason and time and a Retry, which "asks for one more
/// attempt through the same cleanup checks". Specified: a stuck row says "Could not free space" with its failure time
/// (the viewer's local time, the app's zone pinned ten hours east of UTC here, which moves it into the next day) and the
/// host's reason, its button reads Retry and is named for its checkout, and pressing it sends the same
/// one-checkout delete.
#[gpui::test]
fn a_failed_removal_offers_retry_through_the_same_delete(cx: &mut TestAppContext) {
    let (mut helm, _, cx) = open_button(cx, vec![crate::hosts::tests::hosts(&[local()])]);
    cx.update(|_, cx| local_time::pin(10 * 3_600, cx));
    let mut stuck = checkout("a", Some(4096));
    stuck["stuck"] =
        json!({ "reason": "Permission denied in the retained folder", "at": 1_794_591_715_i64 });
    let stuck_listing = listing(&[stuck], false);
    answer_reads(&mut helm, |_| stuck_listing.clone());
    test_support::click(cx, BUTTON_ID);
    answer_reads(&mut helm, |_| stuck_listing.clone());
    test_support::advance(cx, SETTLE);
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "trash-entry-stuck-1-a"),
        "Could not free space · 2026-11-14 03:41"
    );
    assert_eq!(
        label(&nodes, "trash-entry-stuck-reason-1-a"),
        "Permission denied in the retained folder"
    );
    assert_eq!(
        label(&nodes, "trash-entry-delete-1-a"),
        "Retry cleanup of a-20261113T174055Z"
    );
    test_support::click(cx, "trash-entry-delete-1-a");
    let (_, body) = one_delete(&mut helm);
    assert_eq!(body["ids"], json!(["a"]));
}

/// Why this matters: an entry's Delete has no question, so a click aimed at one row must not delete a row a re-read
/// moved under the pointer (the old "trash entry deletes pause after rows move under the pointer"). Specified: when a
/// re-read inserts a row, every entry's Delete is unavailable until the pause ends, still just before it, and
/// available once it has.
#[gpui::test]
fn rows_moving_pause_every_entrys_delete(cx: &mut TestAppContext) {
    let (mut helm, _, cx) = opened(cx);
    assert!(
        enabled(&frame(cx), "trash-entry-delete-1-a"),
        "the premise: settled"
    );
    push_hosts(&mut helm, &[epoch(local(), 1), remote(), down()]);
    test_support::settle(cx);
    answer_reads(&mut helm, |_| {
        listing(
            &[
                checkout("new", Some(1)),
                checkout("a", Some(4096)),
                checkout("b", None),
            ],
            false,
        )
    });
    test_support::settle(cx);
    assert!(!enabled(&frame(cx), "trash-entry-delete-1-new"));
    test_support::advance(cx, SETTLE - std::time::Duration::from_millis(1));
    assert!(
        !enabled(&frame(cx), "trash-entry-delete-1-a"),
        "still pausing"
    );
    test_support::advance(cx, std::time::Duration::from_millis(1));
    let nodes = frame(cx);
    assert!(
        enabled(&nodes, "trash-entry-delete-1-new") && enabled(&nodes, "trash-entry-delete-1-a")
    );
    assert!(take(&mut helm).is_empty(), "nothing was deleted");
}

/// Why this matters: cleanup must not act on a registry the helm could not read (the old "single trash delete requires
/// settled listings and a healthy host refresh"), and the user must be told why the deletes are unavailable.
/// Specified: when the registry fails, the dialog says host reachability could not be refreshed, with the helm's
/// words, and every entry's Delete is unavailable, also once the settle pause the failure started has ended.
#[gpui::test]
fn deletes_wait_for_a_readable_registry_and_say_why(cx: &mut TestAppContext) {
    let (mut helm, _, cx) = opened(cx);
    push_registry_failure(&mut helm, &["1", "2", "3"]);
    test_support::advance(cx, SETTLE);
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "trash-hosts-error"),
        "Host reachability could not be refreshed. Cleanup is disabled until it can be checked: registry locked"
    );
    for control in ["trash-entry-delete-1-a", "trash-entry-delete-2-c"] {
        assert!(!enabled(&nodes, control), "{control}");
    }
}

/// Why this matters: SPEC.md: "Notices for a host disappear when a successful host registry refresh confirms that the
/// host has been removed; disconnection or a failed refresh does not acknowledge them", and a late answer for a host
/// removed meanwhile must not bring one back (the old "a late failed trash reply cannot restore notices for a removed
/// host"). Specified: a refusal kept for `build-box` survives a failed registry read and the host's disconnection, and
/// goes once a readable registry drops the host; a delete on the local host answered after a readable registry
/// dropped it leaves no notice.
#[gpui::test]
fn notices_go_only_when_a_readable_registry_drops_their_host(cx: &mut TestAppContext) {
    let (mut helm, button, cx) = opened(cx);
    test_support::click(cx, "trash-entry-delete-2-c");
    let (id, _) = one_delete(&mut helm);
    helm.reply(id, 503, Some(json!("host unavailable")));
    test_support::settle(cx);
    answer_reads(&mut helm, archives);
    test_support::advance(cx, SETTLE);
    assert!(
        snapshot(&button, cx).has_refusals(),
        "the premise: a refusal is kept"
    );

    push_registry_failure(&mut helm, &["1", "2", "3"]);
    test_support::settle(cx);
    assert!(
        snapshot(&button, cx).has_refusals(),
        "a failed registry read keeps it"
    );
    let mut gone = remote();
    gone.state = crate::hosts::tests::connecting(1, None);
    push_hosts(&mut helm, &[local(), gone, down()]);
    test_support::settle(cx);
    assert!(
        snapshot(&button, cx).has_refusals(),
        "disconnection keeps it"
    );
    assert!(
        node(&frame(cx), "trash-notice-2-0").is_some(),
        "and the dialog still shows it"
    );
    drop_host(&mut helm, &[local(), down()], 2);
    test_support::settle(cx);
    assert!(
        !snapshot(&button, cx).has_refusals(),
        "a readable registry without the host drops it"
    );

    test_support::advance(cx, SETTLE);
    test_support::click(cx, "trash-entry-delete-1-a");
    let (id, _) = one_delete(&mut helm);
    drop_host(&mut helm, &[down()], 1);
    test_support::settle(cx);
    helm.reply(id, 503, Some(json!("host unavailable")));
    test_support::settle(cx);
    assert!(
        take(&mut helm).is_empty(),
        "a host the registry dropped is not read"
    );
    assert!(
        !snapshot(&button, cx).has_refusals(),
        "the late answer left nothing"
    );
}

/// SPEC.md, "Reaching everything by keyboard": "A region whose content holds nothing focusable gets its own tab stop
/// with an accessible name", and the old app's "trash list stays keyboard-scrollable while every list action is
/// disabled". While a host's measured read is out every entry's Delete is unavailable, so nothing in the list can take
/// focus; the old list was a stop of its own named "Archived checkouts". Specified: with the measured reads out and the settle pause over, the body draws its stop under
/// that name; once the reads are answered and the Deletes are available again, it draws none.
#[gpui::test]
fn the_archive_list_has_its_own_stop_while_every_action_is_unavailable(cx: &mut TestAppContext) {
    let (mut helm, _, cx) = open_button(
        cx,
        vec![crate::hosts::tests::hosts(&[local(), remote(), down()])],
    );
    answer_reads(&mut helm, archives);
    test_support::settle(cx);
    test_support::click(cx, BUTTON_ID);
    let measured = take(&mut helm);
    test_support::advance(cx, SETTLE);
    assert!(
        !enabled(&frame(cx), "trash-entry-delete-1-a"),
        "the premise: the Deletes are unavailable while measuring"
    );
    test_support::settle(cx);
    assert_eq!(
        test_support::label(cx, "checkout-trash-dialog-body-stop").as_deref(),
        Some("Archived checkouts")
    );

    answer(
        &mut helm,
        &measured,
        |host| (200, archives(host)),
        |_| (500, Value::Null),
    );
    test_support::advance(cx, SETTLE);
    assert!(
        enabled(&frame(cx), "trash-entry-delete-1-a"),
        "the premise: the Deletes are back"
    );
    test_support::settle(cx);
    assert_eq!(
        test_support::label(cx, "checkout-trash-dialog-body-stop"),
        None
    );
}

// ---- Confirmed deletes ---------------------------------------------------------------------------------------------

/// The delete commands among `commands`, as (host, body), in the order sent.
fn deletes(commands: &[FakeCommand]) -> Vec<(i64, Value)> {
    commands
        .iter()
        .filter(|command| command.path == "/api/checkout-trash/delete")
        .map(|command| {
            let body = command.body.clone().unwrap_or_default();
            (body["host"].as_i64().unwrap_or_default(), body)
        })
        .collect()
}

/// Why this matters: Delete all removes work for good on several hosts at once, so it must delete exactly what its
/// question named, even when a listing changes while the user reads it (SPEC.md: "Confirming a bulk delete closes the
/// dialog immediately and sends one request per host with its frozen IDs; the hosts progress independently"), and its
/// words must name the hosts, the count and the size with the warning. Specified: Delete all opens its question in the
/// region above the footer, focus on Delete permanently, naming three checkouts on this machine and build-box with
/// their partial size, then the warning; a new checkout `d` listed on the local host meanwhile changes neither the
/// question nor what is sent: confirming closes the dialog at once and sends one delete for the local host naming `a`
/// and `b` and one for build-box naming `c`, each on its host's connection, and none for the unreachable `gpu-1`;
/// the local host's answer settles its own trash key while build-box's delete is still out.
#[gpui::test]
fn delete_all_sends_one_frozen_delete_per_reachable_host(cx: &mut TestAppContext) {
    let (mut helm, _, cx) = opened(cx);
    test_support::click(cx, "trash-delete-all");
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    let asked =
        "Delete 3 archived checkouts (46.0 MiB measured · partial) on this machine, build-box?";
    let nodes = frame(cx);
    assert_eq!(label(&nodes, "trash-confirm-checkouts"), asked);
    assert_eq!(
        label(&nodes, "trash-confirm-warning"),
        "Work in them that was never committed or pushed is lost permanently."
    );

    push_hosts(&mut helm, &[epoch(local(), 1), remote(), down()]);
    test_support::settle(cx);
    answer_reads(&mut helm, |host| match host {
        1 => listing(
            &[
                checkout("a", Some(4096)),
                checkout("b", None),
                checkout("d", Some(1)),
            ],
            false,
        ),
        _ => archives(host),
    });
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "trash-host-totals-1"),
        "3 checkouts · 4.0 KiB measured · partial",
        "the premise: the listing moved under the question"
    );
    assert_eq!(label(&nodes, "trash-confirm-checkouts"), asked);

    test_support::click(cx, "prompt-confirm");
    assert!(!test_support::any_open(cx), "confirming closed the dialog");
    let sent = take(&mut helm);
    assert_eq!(
        deletes(&sent),
        [
            (
                1,
                json!({ "host": 1, "expected_incarnation": 1, "ids": ["a", "b"] })
            ),
            (
                2,
                json!({ "host": 2, "expected_incarnation": 1, "ids": ["c"] })
            ),
        ]
    );
    // The helm answers in the order the commands were sent, so the local host's comes first.
    let local_delete = sent.iter().find(|command| {
        command
            .body
            .as_ref()
            .is_some_and(|body| body["host"] == json!(1))
    });
    let Some(local_delete) = local_delete else {
        panic!("the local host's delete was sent");
    };
    helm.reply(
        local_delete.id,
        200,
        Some(json!({ "removed": ["a", "b"], "issues": [] })),
    );
    test_support::settle(cx);
    let in_flight = |key: &str, cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let operations = session::current_operations(cx).unwrap();
            operations.read(cx).in_flight(&OpKey::trash(key))
        })
    };
    assert_eq!(in_flight("1", cx), None);
    assert_eq!(in_flight("2", cx), Some(Work::TrashDelete));
}

/// Why this matters: a host's own Delete all must ask inside that host's card, not elsewhere in the dialog (SPEC.md:
/// "Per-host confirmations stay inside their host sections"), and must delete only that host's archives. Specified:
/// build-box's "Delete all on" asks in place, in a band within build-box's card, naming one checkout of 46.0 MiB on
/// build-box; confirming closes the dialog and sends one delete, for build-box only, naming `c`.
#[gpui::test]
fn a_hosts_delete_all_asks_inside_its_card_and_deletes_only_there(cx: &mut TestAppContext) {
    let (mut helm, _, cx) = opened(cx);
    test_support::click(cx, "trash-host-delete-2");
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "trash-confirm-checkouts"),
        "Delete 1 archived checkout (46.0 MiB) on build-box?"
    );
    let bounds = |id: &str| match node(&nodes, id) {
        Some(node) => node.bounds,
        None => panic!("{id} was not drawn"),
    };
    let (card, words) = (bounds("trash-host-2"), bounds("trash-confirm-checkouts"));
    assert!(
        words.top() >= card.top() && words.bottom() <= card.bottom(),
        "the question is inside build-box's card"
    );
    assert!(
        cx.debug_bounds("trash-host-confirmation-2").is_some(),
        "drawn in its band"
    );
    test_support::click(cx, "prompt-confirm");
    assert!(!test_support::any_open(cx));
    assert_eq!(
        deletes(&take(&mut helm)),
        [(
            2,
            json!({ "host": 2, "expected_incarnation": 1, "ids": ["c"] })
        )]
    );
}

/// Why this matters: Delete all must not offer a partial cleanup as if it were everything (SPEC.md: "The global action
/// skips unreachable hosts and waits until every reachable host can be read"). Specified: with the opening's sized
/// reads out, Delete all and each host's "Delete all on" are unavailable; with the local host answered and build-box
/// still out, Delete all is still unavailable while the local host's own is available; once every reachable host has
/// answered, Delete all is available although `gpu-1` was never read.
#[gpui::test]
fn delete_all_waits_for_every_reachable_host(cx: &mut TestAppContext) {
    let (mut helm, _, cx) = open_button(
        cx,
        vec![crate::hosts::tests::hosts(&[local(), remote(), down()])],
    );
    answer_reads(&mut helm, archives);
    test_support::settle(cx);
    test_support::click(cx, BUTTON_ID);
    let measured = take(&mut helm);
    let nodes = frame(cx);
    for control in [
        "trash-delete-all",
        "trash-host-delete-1",
        "trash-host-delete-2",
    ] {
        assert!(!pressable(&nodes, control), "{control} waits for the reads");
    }
    let (local_read, remote_read): (Vec<_>, Vec<_>) = measured
        .into_iter()
        .partition(|command| reads(std::slice::from_ref(command))[0].1 == 1);
    // The helm answers in the order the reads were sent: the local host's first.
    answer(
        &mut helm,
        &local_read,
        |host| (200, archives(host)),
        |_| (500, Value::Null),
    );
    test_support::settle(cx);
    let nodes = frame(cx);
    assert!(
        !pressable(&nodes, "trash-delete-all"),
        "build-box is still being read"
    );
    assert!(pressable(&nodes, "trash-host-delete-1"));
    answer(
        &mut helm,
        &remote_read,
        |host| (200, archives(host)),
        |_| (500, Value::Null),
    );
    test_support::settle(cx);
    assert!(pressable(&frame(cx), "trash-delete-all"));
}

/// Why this matters: cancelling must leave the keyboard user where they were (SPEC.md: "cancelling returns focus to
/// Delete all"), also on macOS, where a click does not focus the button it presses, so the dialog has to put focus on
/// the asking button itself, and the question's band must go with it. Specified, under the macOS click rule: Escape on
/// the global question cancels it, keeps the dialog open and returns focus to Delete all, and the question and the
/// footer's notices region are gone; a pointer click on Cancel in build-box's question, which under that rule focuses
/// no button and leaves the press on the dialog, returns focus to its "Delete all on", and its band is gone.
#[gpui::test]
fn cancelling_returns_focus_to_the_asking_button_and_takes_the_band_away(cx: &mut TestAppContext) {
    let (_helm, _, cx) = opened(cx);
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    test_support::click(cx, "trash-delete-all");
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    assert!(
        cx.debug_bounds("trash-global-confirmation").is_some(),
        "the premise: the band is drawn"
    );
    test_support::keys(cx, "escape");
    assert!(
        test_support::any_open(cx),
        "Escape answered only the question"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("trash-delete-all"));
    assert!(node(&frame(cx), "trash-confirm-checkouts").is_none());
    assert!(
        cx.debug_bounds("checkout-trash-dialog-notices").is_none(),
        "no notices region is left behind"
    );

    test_support::click(cx, "trash-host-delete-2");
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    assert!(
        cx.debug_bounds("trash-host-confirmation-2").is_some(),
        "the premise: build-box's band is drawn"
    );
    // A macOS click focuses no button: the press lands on the dialog, which takes focus, so the answer must still
    // return focus to the asking button.
    test_support::click(cx, "prompt-cancel");
    assert_eq!(focused_id(cx).as_deref(), Some("trash-host-delete-2"));
    assert!(cx.debug_bounds("trash-host-confirmation-2").is_none());
}

/// Why this matters: the old dialog held still while its band asked: no entry could be deleted under the question, and
/// Close or Escape dismissed the band before the dialog, so a stray Escape never closed the dialog over an unanswered
/// question; and a host's question closing moves the rows back, which must pause the entries' Deletes as any move
/// does. Specified: with the global question open, past the settle pause, an entry's Delete is unavailable and a press
/// on it sends nothing; the footer's Close dismisses the question, keeps the dialog open and returns focus to Delete
/// all; the entries' Deletes stay unavailable for the settle pause that the closing started, and then are available;
/// with build-box's question open the header's close does the same, focus going back to its "Delete all on"; with the
/// global question open and focus moved off it to the footer's Close, Escape dismisses the question, not the dialog;
/// with no question open, Escape closes the dialog.
#[gpui::test]
fn an_open_question_holds_the_dialog_and_close_or_escape_dismisses_it_first(
    cx: &mut TestAppContext,
) {
    let (mut helm, _, cx) = opened(cx);
    test_support::click(cx, "trash-delete-all");
    test_support::advance(cx, SETTLE);
    assert!(!enabled(&frame(cx), "trash-entry-delete-1-a"), "held");
    test_support::click(cx, "trash-entry-delete-1-a");
    assert!(take(&mut helm).is_empty(), "the held Delete sent nothing");

    test_support::click(cx, "trash-close");
    assert!(
        test_support::any_open(cx),
        "Close dismissed only the question"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("trash-delete-all"));
    assert!(cx.debug_bounds("trash-global-confirmation").is_none());
    assert!(
        !enabled(&frame(cx), "trash-entry-delete-1-a"),
        "the closing restarted the settle pause"
    );
    test_support::advance(cx, SETTLE);
    assert!(enabled(&frame(cx), "trash-entry-delete-1-a"));

    test_support::click(cx, "trash-host-delete-2");
    test_support::click(cx, "checkout-trash-dialog-close");
    assert!(test_support::any_open(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("trash-host-delete-2"));
    assert!(cx.debug_bounds("trash-host-confirmation-2").is_none());

    test_support::click(cx, "trash-delete-all");
    test_support::keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("trash-close"),
        "the premise: focus left the question for the footer's Close"
    );
    test_support::keys(cx, "escape");
    assert!(
        test_support::any_open(cx),
        "Escape dismissed only the question"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("trash-delete-all"));
    test_support::keys(cx, "escape");
    assert!(
        !test_support::any_open(cx),
        "with no question, Escape closes the dialog"
    );
}

/// Start `work` on `key` in the window's operation store and leave it unanswered, as other work of this window holds
/// the key the trash's guards read; its command is taken from the fake.
fn hold(helm: &mut FakeHelm, cx: &mut VisualTestContext, key: OpKey, work: Work) {
    cx.update(|_, cx| {
        let operations = session::current_operations(cx).unwrap();
        let path = ApiPath::api(&["work"]).unwrap();
        let command = Command::new(work, Method::Post, path, None, "POST /api/work");
        operations.update(cx, |operations, cx| {
            operations.start(&key, command, cx, |_, _| {}).unwrap();
        });
    });
    test_support::settle(cx);
    assert_eq!(take(helm).len(), 1, "the premise: the work went out");
}

/// Press the open question's Delete permanently and check what a refusal at the press must do: no delete is sent, the
/// dialog stays open, the question is gone (the prompt closes an inline question as its answer runs), and `reason` is
/// painted and named above the footer's actions.
fn confirm_refused(helm: &mut FakeHelm, cx: &mut VisualTestContext, reason: Refused) {
    let reason = reason.words();
    let reason = reason.as_str();
    test_support::click(cx, "prompt-confirm");
    assert!(deletes(&take(helm)).is_empty(), "nothing was sent");
    assert!(test_support::any_open(cx), "the dialog stays open");
    let nodes = frame(cx);
    assert!(
        node(&nodes, "trash-confirm-checkouts").is_none(),
        "the question is gone"
    );
    assert_eq!(label(&nodes, "trash-refusal"), reason);
    assert!(painted(&nodes, reason), "the reason is visible");
}

/// Why this matters: a confirmation authorizes only what its question showed, on the connection the host had then
/// (SPEC_impl.md, "Archived-checkout trash": confirmations freeze "observed host incarnations"), and a refusal at the
/// press must neither close the dialog over a delete that never went out nor leave the press silent (SPEC.md, "Errors
/// and diagnostics"). Specified: with build-box's question open, the helm reports build-box on a new connection; Delete
/// permanently then sends no delete, the dialog stays open, its question and band are gone, and the old dispatch's
/// "The host connection changed" reason shows; once build-box is read again, asking again takes the reason away.
#[gpui::test]
fn a_confirmation_on_a_reconnected_host_sends_nothing_and_says_why(cx: &mut TestAppContext) {
    let (mut helm, _, cx) = opened(cx);
    test_support::click(cx, "trash-host-delete-2");
    assert!(
        cx.debug_bounds("trash-host-confirmation-2").is_some(),
        "the premise: build-box's question is open"
    );
    let mut reconnected = remote();
    reconnected.incarnation = 2;
    push_hosts(&mut helm, &[local(), reconnected, down()]);
    test_support::settle(cx);
    let reread = take(&mut helm);
    confirm_refused(&mut helm, cx, Refused::Connection);
    assert!(cx.debug_bounds("trash-host-confirmation-2").is_none());

    answer(
        &mut helm,
        &reread,
        |host| (200, archives(host)),
        |_| (500, Value::Null),
    );
    test_support::advance(cx, SETTLE);
    test_support::click(cx, "trash-host-delete-2");
    let nodes = frame(cx);
    assert!(
        node(&nodes, "trash-confirm-checkouts").is_some(),
        "the premise: asked again"
    );
    assert!(
        node(&nodes, "trash-refusal").is_none(),
        "the new question took the reason away"
    );
}

/// Why this matters: the trash allows one delete of its own at a time (SPEC.md: "the trash control prevents
/// overlapping deletes of its own"), and a question already open when one starts must not send a second; the old
/// band's answer was unavailable then, while an answered question here must say why it did nothing. Specified: with
/// the global question open, a trash delete of this window's starts on the local host's trash key; Delete permanently
/// sends nothing, keeps the dialog open, and shows the busy reason ("Not deleted: another operation was in progress").
#[gpui::test]
fn a_confirmation_while_another_trash_delete_runs_sends_nothing_and_says_why(
    cx: &mut TestAppContext,
) {
    let (mut helm, _, cx) = opened(cx);
    test_support::click(cx, "trash-delete-all");
    hold(&mut helm, cx, OpKey::trash("1"), Work::TrashDelete);
    assert!(
        cx.debug_bounds("trash-global-confirmation").is_some(),
        "the premise: the question outlived the work's start"
    );
    confirm_refused(&mut helm, cx, Refused::Busy);
}

/// Why this matters: SPEC.md puts bulk cleanup under the hosts' operation guards, so a confirmation must not clean a
/// host whose own key other work holds (its removal, say), and the press must say why. Specified: with build-box's
/// question open, other work starts on build-box's own key; Delete permanently sends nothing, keeps the dialog open,
/// and shows the busy reason. An entry's own Delete on another host then goes out and takes the reason away, so the
/// line never sits beside a delete that did start.
#[gpui::test]
fn a_confirmation_while_its_host_is_busy_sends_nothing_and_says_why(cx: &mut TestAppContext) {
    let (mut helm, _, cx) = opened(cx);
    test_support::click(cx, "trash-host-delete-2");
    hold(&mut helm, cx, OpKey::host("2"), Work::Save);
    assert!(
        cx.debug_bounds("trash-host-confirmation-2").is_some(),
        "the premise: the question outlived the work's start"
    );
    confirm_refused(&mut helm, cx, Refused::Busy);

    test_support::advance(cx, SETTLE);
    assert!(
        enabled(&frame(cx), "trash-entry-delete-1-a"),
        "the premise: the local host's entry can be deleted"
    );
    assert!(
        node(&frame(cx), "trash-refusal").is_some(),
        "the premise: the reason outlived the pause"
    );
    test_support::click(cx, "trash-entry-delete-1-a");
    assert_eq!(
        deletes(&take(&mut helm)).len(),
        1,
        "the entry's delete went out"
    );
    assert!(
        node(&frame(cx), "trash-refusal").is_none(),
        "a delete that started took the reason away"
    );
}

/// Why this matters: cleanup must not act on a registry the helm could not read (the old "single trash delete requires
/// settled listings and a healthy host refresh"), also from a question asked before the registry failed, and the press
/// must say why. Specified: with the global question open, the registry fails; Delete permanently sends nothing, keeps
/// the dialog open, and shows the old dispatch's "The host connection changed" reason.
#[gpui::test]
fn a_confirmation_while_the_registry_is_unreadable_sends_nothing_and_says_why(
    cx: &mut TestAppContext,
) {
    let (mut helm, _, cx) = opened(cx);
    test_support::click(cx, "trash-delete-all");
    push_registry_failure(&mut helm, &["1", "2", "3"]);
    test_support::settle(cx);
    let nodes = frame(cx);
    assert!(
        node(&nodes, "trash-hosts-error").is_some(),
        "the premise: the registry is unreadable"
    );
    assert!(
        cx.debug_bounds("trash-global-confirmation").is_some(),
        "the premise: the question outlived the failure"
    );
    confirm_refused(&mut helm, cx, Refused::Connection);
}

/// Why this matters: Delete all can become unavailable while its question is up, and a cancelled question returns
/// focus to it; a button that could not take focus then would drop the keyboard user out of the dialog (the old app
/// fell back to Close for this). Specified: with the global question open, a pushed trash epoch sends a read of the
/// local host, which makes Delete all unavailable; Escape then cancels the question and focus is on Delete all, drawn
/// busy, and stays there once the read is answered and Delete all is available again.
#[gpui::test]
fn cancelling_returns_focus_to_delete_all_while_it_is_unavailable(cx: &mut TestAppContext) {
    let (mut helm, _, cx) = opened(cx);
    test_support::click(cx, "trash-delete-all");
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    push_hosts(&mut helm, &[epoch(local(), 1), remote(), down()]);
    test_support::settle(cx);
    let nodes = frame(cx);
    assert!(
        enabled(&nodes, "trash-delete-all") && !pressable(&nodes, "trash-delete-all"),
        "the premise: the re-read left Delete all busy"
    );
    test_support::keys(cx, "escape");
    assert!(test_support::any_open(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("trash-delete-all"));
    answer_reads(&mut helm, archives);
    test_support::advance(cx, SETTLE);
    assert!(pressable(&frame(cx), "trash-delete-all"));
    assert_eq!(focused_id(cx).as_deref(), Some("trash-delete-all"));
}

/// The top edge of what was drawn under element id `id` in a fresh frame; fails when nothing was.
fn top(cx: &mut VisualTestContext, id: &str) -> Pixels {
    match node(&frame(cx), id) {
        Some(node) => node.bounds.top(),
        None => panic!("{id} was not drawn"),
    }
}

/// Why this matters: SPEC.md puts the global question in "its own bounded, keyboard-scrollable region directly above
/// the footer". Focus starts on Delete permanently, at the question's end, so once the question outgrows its region
/// the line naming what it deletes sits above view, and without the region's page keys a keyboard user would confirm a
/// deletion whose count and hosts they cannot read. The old region was a scroll region the browser scrolled around the
/// focused answer; the frame's notices do so only when the dialog opts in.
///
/// Specified, in a window 400 pixels tall (shorter than the app's minimum, standing in for host names long enough to
/// make the question outgrow the region at any height), with the global question open: focus is on Delete permanently
/// and the question's first line starts above the region's top, scrolled there to show the answer; Home brings it to
/// the region's top or below; Page Down scrolls it back up, and End at least as far as focus had and on to the end,
/// where the question's band ends at the region's bottom; Page Up scrolls it back down; focus stays on Delete
/// permanently throughout, and the region draws no stop of its own while its answers can take focus.
#[gpui::test]
fn page_keys_scroll_the_global_questions_region_from_its_focused_answer(cx: &mut TestAppContext) {
    let (_helm, _, cx) = opened(cx);
    cx.simulate_resize(size(px(720.), px(400.)));
    test_support::settle(cx);
    test_support::click(cx, "trash-delete-all");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-confirm"),
        "the premise: Delete permanently holds focus"
    );
    let region = top(cx, "checkout-trash-dialog-notices");
    let revealed = top(cx, "trash-confirm-checkouts");
    assert!(
        revealed < region,
        "the premise: the question outgrows its region, scrolled to show its answer: {revealed:?} above {region:?}"
    );

    test_support::keys(cx, "home");
    let start = top(cx, "trash-confirm-checkouts");
    assert!(start >= region, "Home scrolled to the start: {start:?}");
    test_support::keys(cx, "pagedown");
    let paged = top(cx, "trash-confirm-checkouts");
    assert!(paged < start, "Page Down scrolled forward: {paged:?}");
    // The focus coordinator scrolls only far enough to show the answer, short of the band's padding under it, so
    // End's end is the band's bottom meeting the region's, not where focus left the question.
    test_support::keys(cx, "end");
    assert!(
        top(cx, "trash-confirm-checkouts") <= revealed,
        "End scrolled at least as far as focus had"
    );
    // By debug selector, which the band has and the driver record does not carry.
    assert_eq!(
        test_support::bounds(cx, "trash-global-confirmation").bottom(),
        test_support::bounds(cx, "checkout-trash-dialog-notices").bottom(),
        "End scrolled to the end: the band's bottom meets the region's"
    );
    let ended = top(cx, "trash-confirm-checkouts");
    test_support::keys(cx, "pageup");
    let back = top(cx, "trash-confirm-checkouts");
    assert!(
        back > ended,
        "Page Up scrolled back: {back:?} below {ended:?}"
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-confirm"),
        "focus stayed on the answer"
    );
    assert_eq!(
        test_support::label(cx, "checkout-trash-dialog-notices-stop"),
        None,
        "and the region is no stop while its answers can take focus"
    );
}

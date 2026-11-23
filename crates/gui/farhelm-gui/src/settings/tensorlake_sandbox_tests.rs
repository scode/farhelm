//! Tests of Settings → Tensorlake's **Sandboxes not on this helm** over the helm client's fake: when the list reads,
//! what a read shows, the rows and their cleanup states, Delete's question and its answers, and a read's failure.
//!
//! The dialog is opened from the gear as in the dialog's own tests (`super::tests`), and the section is shown as in
//! the section's own (`super::tensorlake_tests`), whose helpers these share. Every step ends in one of the shared
//! harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::Preferences;
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::recipe_tests::pressable;
use super::tensorlake_tests::{account, press, show_tensorlake};
use super::tests::{
    available, drawn, fill, into_view, label_of, open_rail, open_rail_with_hosts, painted,
    push_hosts, sent,
};
use crate::local_time;
use crate::test_support::{bounds, click, focused_id, settle};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// A leftover as `GET /api/tensorlake/orphans` lists it: `id` named `name`, running, from this helm, with no creation
/// time and no cleanup, which a test changes as it needs.
fn sandbox(id: &str, name: &str) -> Value {
    json!({ "id": id, "name": name, "created_at": null, "state": "running", "from_another_helm": false })
}

/// Model the wire response with discovery and the helm's cleanup snapshot in separate fields.
pub(super) fn leftover_list(sandboxes: &[Value], cleanups: &[Value]) -> Value {
    json!({ "discovery": { "status": "ok", "sandboxes": sandboxes }, "cleanups": cleanups })
}

/// Discovery can fail while the helm still knows which explicit cleanups are unfinished.
fn failed_discovery(reason: &str, cleanups: &[Value]) -> Value {
    json!({ "discovery": { "status": "failed", "reason": reason }, "cleanups": cleanups })
}

/// Show Tensorlake with a key saved, answering the account read, and return the list's read, which that answer sends.
fn show_with_key(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> FakeCommand {
    let read = show_tensorlake(helm, cx);
    helm.reply(read.id, 200, Some(account(true, false)));
    settle(cx);
    one_list_read(helm)
}

/// The one command sent since the last call, which must be the list's read.
fn one_list_read(helm: &mut FakeHelm) -> FakeCommand {
    let reads = sent(helm);
    assert_eq!(reads.len(), 1, "one read: {reads:?}");
    let read = reads.into_iter().next().unwrap();
    assert_eq!(
        (read.method, read.path.as_str()),
        (Method::Get, "/api/tensorlake/orphans")
    );
    read
}

/// What [`busy_state`] says of an available button, and of a busy one, which stays focusable but takes no press.
const READY: Option<(bool, bool)> = Some((true, true));
const BUSY: Option<(bool, bool)> = Some((true, false));

/// Whether the button under `id` takes focus and takes a press; `None` when it is not drawn. A busy button keeps focus
/// and takes no press, which tells it from an unavailable one, which takes neither.
fn busy_state(cx: &mut VisualTestContext, id: &str) -> Option<(bool, bool)> {
    available(cx, id).zip(pressable(cx, id))
}

/// Click what is drawn under `selector`, after scrolling the dialog's body to it, as a user would.
fn click_in_view(cx: &mut VisualTestContext, selector: &'static str) {
    into_view(cx, selector);
    click(cx, selector);
}

/// The status line's words: "Checking…" or "None found · checked …"; `None` when it is not drawn.
fn status(cx: &mut VisualTestContext) -> Option<String> {
    label_of(cx, "tensorlake-sandboxes-status")
}

// ---- Reading -------------------------------------------------------------------------------------------------------

/// SPEC.md: with a saved key the list "stays visible even when empty, with a small **Refresh** beside its heading. A
/// read shows **Checking…**, then an empty success shows **None found · checked HH:MM**". Without a key the list must
/// not read, since that read lists the user's Tensorlake account for nothing, and a key saved in this opening must read
/// it, since it is the first time a read can answer. Specified: with no key saved, the list is not drawn and nothing
/// reads it; a key saved in the section's form reads it once, and while that read is out the list shows "Checking…"
/// with Refresh busy (focusable, taking no press); answered empty, it says "None found · checked" and the hours and
/// minutes of the answer in the viewer's local time (the app's zone pinned five hours west of UTC, so the test clock's
/// 23:59:58 UTC reads 18:59) and Refresh takes presses again; Refresh pressed by keyboard reads once more and keeps
/// focus while its read is out and after it answers, as a busy button does; and a second key saved over the first reads
/// once more too, since the list it shows belongs to the key that was saved.
#[gpui::test]
fn a_saved_key_reads_the_list_which_says_when_it_found_none(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    cx.update(|_, cx| local_time::pin(-5 * 3_600, cx));
    let read = show_tensorlake(&mut helm, cx);
    helm.reply(read.id, 200, Some(account(false, false)));
    settle(cx);
    assert_eq!(sent(&mut helm), [], "no key, no read");
    assert!(!drawn(cx, "tensorlake-sandboxes-heading"));

    fill(cx, "tensorlake-key", "fictional-tensorlake-key");
    click(cx, "settings-footer-save");
    let save = sent(&mut helm);
    assert_eq!(save.len(), 1, "{save:?}");
    helm.reply(save[0].id, 200, Some(account(true, false)));
    settle(cx);
    let read = one_list_read(&mut helm);
    assert_eq!(status(cx).as_deref(), Some("Checking…"));
    assert_eq!(
        busy_state(cx, "tensorlake-sandboxes-refresh"),
        BUSY,
        "Refresh is busy while the read is out"
    );
    helm.reply(read.id, 200, Some(leftover_list(&[], &[])));
    settle(cx);
    assert_eq!(status(cx).as_deref(), Some("None found · checked 18:59"));
    assert_eq!(busy_state(cx, "tensorlake-sandboxes-refresh"), READY);
    press(cx, "tensorlake-sandboxes-refresh");
    let read = one_list_read(&mut helm);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("tensorlake-sandboxes-refresh"),
        "a Refresh pressed by keyboard keeps focus while its read is out"
    );
    helm.reply(read.id, 200, Some(leftover_list(&[], &[])));
    settle(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("tensorlake-sandboxes-refresh")
    );

    fill(cx, "tensorlake-key", "fictional-replacement-key");
    click(cx, "settings-footer-save");
    let save = sent(&mut helm);
    assert_eq!(save.len(), 1, "{save:?}");
    helm.reply(save[0].id, 200, Some(account(true, false)));
    settle(cx);
    let _ = one_list_read(&mut helm);
}

/// A transport error supplies no new cleanup snapshot. Keep the existing list with the refresh failure above it.
/// A later successful empty refresh is the evidence required to say "None found".
#[gpui::test]
fn a_failed_read_keeps_the_rows_and_a_refresh_recovers(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_with_key(&mut helm, cx);
    helm.reply(
        read.id,
        200,
        Some(leftover_list(
            &[sandbox("sb-1", "farhelm-host-k7q2-left-abcd")],
            &[],
        )),
    );
    settle(cx);
    assert!(painted(cx).contains("farhelm-host-k7q2-left-abcd"));

    click_in_view(cx, "tensorlake-sandboxes-refresh");
    let read = one_list_read(&mut helm);
    helm.reply(
        read.id,
        502,
        Some(json!("Tensorlake did not answer within 20 seconds")),
    );
    settle(cx);
    assert_eq!(
        label_of(cx, "tensorlake-sandboxes-error").as_deref(),
        Some("Could not refresh sandbox list: Tensorlake did not answer within 20 seconds")
    );
    assert!(
        painted(cx).contains("farhelm-host-k7q2-left-abcd"),
        "the row stays"
    );
    assert_eq!(status(cx), None, "no None found after a failure");

    click_in_view(cx, "tensorlake-sandboxes-refresh");
    let read = one_list_read(&mut helm);
    helm.reply(read.id, 200, Some(leftover_list(&[], &[])));
    settle(cx);
    assert!(!drawn(cx, "tensorlake-sandboxes-error"));
    assert!(status(cx).is_some_and(|status| status.starts_with("None found")));
}

/// Cleanup owns a sandbox's row even when a healthy discovery still includes that ID. If discovery then fails,
/// current cleanup status must replace the old status and completed cleanup must disappear without reviving it.
/// Other discoveries remain visible until the provider can supply a fresh list.
#[gpui::test]
fn discovery_failure_keeps_discoveries_and_reconciles_cleanup(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_with_key(&mut helm, cx);
    let retained = "farhelm-host-k7q2-still-listed";
    let stale = "farhelm-host-k7q2-discovered-copy";
    let cleanup_name = "farhelm-host-k7q2-cleanup-copy";
    let starts_during_outage = "farhelm-host-k7q2-starts-during-outage";
    let mut cleanup = sandbox("sb-2", cleanup_name);
    cleanup["cleanup"] = json!({ "phase": "running" });
    cleanup["state"] = json!("Cleanup in progress");
    helm.reply(
        read.id,
        200,
        Some(leftover_list(
            &[
                sandbox("sb-1", retained),
                sandbox("sb-2", stale),
                sandbox("sb-3", starts_during_outage),
            ],
            &[cleanup.clone()],
        )),
    );
    settle(cx);
    let shown = painted(cx);
    assert!(
        shown.contains(retained)
            && shown.contains(cleanup_name)
            && shown.contains(starts_during_outage),
        "{shown}"
    );
    assert!(!shown.contains(stale), "{shown}");
    assert_eq!(
        label_of(cx, "tensorlake-sandbox-2-delete").as_deref(),
        Some("Cleaning up…")
    );
    assert!(!drawn(cx, "tensorlake-sandbox-3-delete"));

    cleanup["cleanup"] = json!({ "phase": "failed", "reason": "snapshot sweep refused" });
    cleanup["state"] = json!("Cleanup unfinished");
    let mut newly_cleaning = sandbox("sb-3", starts_during_outage);
    newly_cleaning["cleanup"] = json!({ "phase": "running" });
    newly_cleaning["state"] = json!("Cleanup in progress");
    click_in_view(cx, "tensorlake-sandboxes-refresh");
    let read = one_list_read(&mut helm);
    helm.reply(
        read.id,
        200,
        Some(failed_discovery(
            "provider unavailable",
            &[cleanup, newly_cleaning],
        )),
    );
    settle(cx);
    let error = "Could not refresh sandbox list: provider unavailable";
    assert_eq!(
        label_of(cx, "tensorlake-sandboxes-error").as_deref(),
        Some(error)
    );
    assert!(
        bounds(cx, "tensorlake-sandboxes-error").bottom()
            <= bounds(cx, "tensorlake-sandbox-0").top(),
        "the refresh failure is above the first kept row"
    );
    assert_eq!(
        label_of(cx, "tensorlake-sandbox-1-cleanup-error").as_deref(),
        Some("Cleanup unfinished: snapshot sweep refused"),
    );
    assert_eq!(
        label_of(cx, "tensorlake-sandbox-1-delete").as_deref(),
        Some("Retry cleanup")
    );
    assert_eq!(
        label_of(cx, "tensorlake-sandbox-2-delete").as_deref(),
        Some("Cleaning up…")
    );
    assert!(!drawn(cx, "tensorlake-sandbox-3-delete"));
    assert!(!drawn(cx, "tensorlake-sandboxes-delete-error"));

    click_in_view(cx, "tensorlake-sandboxes-refresh");
    let read = one_list_read(&mut helm);
    helm.reply(
        read.id,
        200,
        Some(failed_discovery("provider unavailable", &[])),
    );
    settle(cx);
    let shown = painted(cx);
    assert!(shown.contains(retained), "{shown}");
    assert!(
        !shown.contains(stale)
            && !shown.contains(cleanup_name)
            && !shown.contains(starts_during_outage),
        "{shown}"
    );
    assert!(!drawn(cx, "tensorlake-sandbox-1-delete"));
    assert_eq!(
        label_of(cx, "tensorlake-sandboxes-error").as_deref(),
        Some(error)
    );
    assert_eq!(
        status(cx),
        None,
        "failed discovery has no empty success to show"
    );

    click_in_view(cx, "tensorlake-sandboxes-refresh");
    let read = one_list_read(&mut helm);
    helm.reply(read.id, 200, Some(leftover_list(&[], &[])));
    settle(cx);
    assert!(!drawn(cx, "tensorlake-sandboxes-error"));
    assert!(!drawn(cx, "tensorlake-sandbox-0-delete"));
    assert!(status(cx).is_some_and(|status| status.starts_with("None found")));
}

/// A key cleared must take the list with it: its rows belong to the key that was saved, and without a key the list is
/// not read again, so rows left on screen would never be corrected (the old "sandbox discovery ignores replies
/// superseded by key replacement or clear"). The UI channel answers in order, so a read sent before the Clear is
/// answered before it, but a read sent while the Clear is out (a Tensorlake host arriving) is answered after it, with no
/// key to list with. Specified: with a key saved and the list's read answered with a row, Clear in the section's form
/// sent, and a Tensorlake host pushed while it is out (which reads the list once more), the Clear's answer hides the
/// list, the later read's refusal draws neither the list nor its error, and nothing more is sent.
#[gpui::test]
fn a_clear_hides_the_list_and_its_rows(cx: &mut TestAppContext) {
    use crate::hosts::tests::{connected, host};
    let (mut helm, cx, _gear) = open_rail_with_hosts(cx, &[]);
    let read = show_with_key(&mut helm, cx);
    let mut cleanup = sandbox("sb-2", "farhelm-host-k7q2-cleaning-abcd");
    cleanup["cleanup"] = json!({ "phase": "running" });
    helm.reply(
        read.id,
        200,
        Some(leftover_list(
            &[sandbox("sb-1", "farhelm-host-k7q2-left-abcd")],
            &[cleanup.clone()],
        )),
    );
    settle(cx);
    assert!(painted(cx).contains("farhelm-host-k7q2-left-abcd"));
    assert!(
        drawn(cx, "tensorlake-sandbox-1-delete"),
        "premise: the cleanup is present"
    );
    press(cx, "tensorlake-key-clear");
    let clear = sent(&mut helm);
    assert_eq!(clear.len(), 1, "{clear:?}");
    // A Tensorlake host arriving while the Clear is out reads the list once more, with the key still saved.
    push_hosts(&mut helm, &[host(20, "tensorlake", connected())], cx);
    let late = one_list_read(&mut helm);
    helm.reply(clear[0].id, 200, Some(account(false, false)));
    settle(cx);
    assert!(!drawn(cx, "tensorlake-sandboxes-heading"));
    assert!(!painted(cx).contains("farhelm-host-k7q2-left-abcd"));
    assert!(!painted(cx).contains("farhelm-host-k7q2-cleaning-abcd"));
    helm.reply(
        late.id,
        200,
        Some(failed_discovery(
            "save an API key before listing sandboxes",
            &[cleanup],
        )),
    );
    settle(cx);
    assert!(
        !drawn(cx, "tensorlake-sandboxes-heading") && !drawn(cx, "tensorlake-sandboxes-error"),
        "the read sent before the Clear was answered shows nothing"
    );
    assert_eq!(sent(&mut helm), []);
}

/// What this helm owns changes when a Tensorlake host comes or goes, so the list must read again then; it lists the
/// user's whole Tensorlake account, so no other change of the fleet may read it (decided by the migration's
/// coordinator on 2026-11-15). Only the newest read's answer counts, so "Checking…" lasts until it answers. Specified:
/// with the list read, a push adding an SSH host reads nothing; a push adding a Tensorlake host reads the list once,
/// and a push removing it while that read is out reads once more; the first of the two answering with a row shows
/// neither the row nor a result, the list still checking, and the second answering empty says "None found"; the same
/// fleet pushed again reads nothing.
#[gpui::test]
fn only_a_tensorlake_host_coming_or_going_reads_the_list_again(cx: &mut TestAppContext) {
    use crate::hosts::tests::{connected, host};
    let ssh = host(7, "ssh", connected());
    let (mut helm, cx, _gear) = open_rail_with_hosts(cx, &[]);
    let read = show_with_key(&mut helm, cx);
    helm.reply(read.id, 200, Some(leftover_list(&[], &[])));
    settle(cx);

    push_hosts(&mut helm, std::slice::from_ref(&ssh), cx);
    assert_eq!(sent(&mut helm), [], "an SSH host reads nothing");
    let sandbox_host = host(20, "tensorlake", connected());
    push_hosts(&mut helm, &[ssh.clone(), sandbox_host], cx);
    let older = one_list_read(&mut helm);
    push_hosts(&mut helm, std::slice::from_ref(&ssh), cx);
    let newer = one_list_read(&mut helm);
    helm.reply(
        older.id,
        200,
        Some(leftover_list(
            &[sandbox("sb-1", "farhelm-host-k7q2-left-abcd")],
            &[],
        )),
    );
    settle(cx);
    assert!(!painted(cx).contains("farhelm-host-k7q2-left-abcd"));
    assert_eq!(status(cx).as_deref(), Some("Checking…"));
    helm.reply(newer.id, 200, Some(leftover_list(&[], &[])));
    settle(cx);
    assert!(status(cx).is_some_and(|status| status.starts_with("None found")));
    push_hosts(&mut helm, &[ssh], cx);
    assert_eq!(sent(&mut helm), [], "the same fleet again reads nothing");
}

/// A fleet update can order a new read before the old read replies. If cleanup starts and finishes between those
/// replies, only the older snapshot shows that the cached sandbox entered cleanup. Keep that fact without letting
/// the older reply end Checking or show its cleanup row.
#[gpui::test]
fn a_superseded_read_can_retire_a_cached_discovery(cx: &mut TestAppContext) {
    use crate::hosts::tests::{connected, host};
    let (mut helm, cx, _gear) = open_rail_with_hosts(cx, &[]);
    let read = show_with_key(&mut helm, cx);
    let retired = "farhelm-host-k7q2-to-retire";
    let retained = "farhelm-host-k7q2-retained";
    helm.reply(
        read.id,
        200,
        Some(leftover_list(
            &[sandbox("sb-1", retired), sandbox("sb-2", retained)],
            &[],
        )),
    );
    settle(cx);
    let shown = painted(cx);
    assert!(
        shown.contains(retired) && shown.contains(retained),
        "{shown}"
    );

    click_in_view(cx, "tensorlake-sandboxes-refresh");
    let older = one_list_read(&mut helm);
    push_hosts(&mut helm, &[host(20, "tensorlake", connected())], cx);
    let newer = one_list_read(&mut helm);
    let mut cleanup = sandbox("sb-1", "farhelm-host-k7q2-cleanup-copy");
    cleanup["cleanup"] = json!({ "phase": "running" });
    helm.reply(
        older.id,
        200,
        Some(failed_discovery("provider unavailable", &[cleanup])),
    );
    settle(cx);
    assert_eq!(status(cx).as_deref(), Some("Checking…"));
    assert!(!painted(cx).contains("farhelm-host-k7q2-cleanup-copy"));

    helm.reply(
        newer.id,
        200,
        Some(failed_discovery("provider unavailable", &[])),
    );
    settle(cx);
    let shown = painted(cx);
    assert!(
        shown.contains(retained) && !shown.contains(retired),
        "{shown}"
    );
    assert!(!drawn(cx, "tensorlake-sandbox-1-delete"));
}

// ---- Rows and Delete -----------------------------------------------------------------------------------------------

/// SPEC.md: each row "shows its creation time when available, its state, and **from another helm** when its name
/// carries another helm's tag", and "Unfinished cleanup remains in the list with its error and **Retry cleanup** ...
/// running cleanup shows **Cleaning up…**". These come from the helm's list, so they must show whatever the GUI did
/// before. Specified: a row's details are its state, "from another helm" when so marked and its creation time to the
/// second in the viewer's local time (the app's zone pinned an hour east of UTC here);
/// a failed cleanup's row shows "Cleanup unfinished:" with the helm's reason and offers Retry cleanup; a running
/// cleanup's row offers Cleaning up…, unavailable.
#[gpui::test]
fn rows_show_their_state_and_cleanup(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    cx.update(|_, cx| local_time::pin(3_600, cx));
    let read = show_with_key(&mut helm, cx);
    let mut other = sandbox("sb-2", "farhelm-host-zzzz-other-abcd");
    other["from_another_helm"] = json!(true);
    other["created_at"] = json!(1_794_647_100_000_i64);
    let mut failed = sandbox("sb-3", "farhelm-build-k7q2-recipe");
    failed["cleanup"] = json!({ "phase": "failed", "reason": "snapshot cleanup blocked" });
    let mut running = sandbox("sb-4", "farhelm-sess-k7q2-run-abcd");
    running["cleanup"] = json!({ "phase": "running" });
    running["state"] = json!("Cleanup in progress");
    helm.reply(
        read.id,
        200,
        Some(leftover_list(
            &[sandbox("sb-1", "farhelm-host-k7q2-own-abcd"), other],
            &[failed, running],
        )),
    );
    settle(cx);
    assert_eq!(
        label_of(cx, "tensorlake-sandbox-1-details").as_deref(),
        Some("running · from another helm · Created 2026-11-14 10:05:00")
    );
    assert_eq!(
        label_of(cx, "tensorlake-sandbox-0-details").as_deref(),
        Some("running")
    );
    assert_eq!(
        label_of(cx, "tensorlake-sandbox-2-cleanup-error").as_deref(),
        Some("Cleanup unfinished: snapshot cleanup blocked")
    );
    assert_eq!(
        label_of(cx, "tensorlake-sandbox-2-delete").as_deref(),
        Some("Retry cleanup")
    );
    assert_eq!(
        label_of(cx, "tensorlake-sandbox-3-delete").as_deref(),
        Some("Cleaning up…")
    );
    assert_eq!(available(cx, "tensorlake-sandbox-3-delete"), Some(false));
}

/// SPEC.md: "**Delete** asks plainly before terminating it and deleting its snapshots", and the helm answers a Delete
/// at its bound saying the cleanup carries on, which only a fresh read of the list shows. Specified: Delete asks
/// "Delete sandbox <name>?" with focus on Cancel and sends nothing; Cancel closes it with nothing sent; Delete sandbox
/// sends `DELETE /api/tensorlake/orphans/<id>`, and while it is out Refresh and the row's Delete are busy, the Delete
/// holding focus again once the question has answered (a disabled one would have dropped it); the helm's 504 shows its words under the list and reads the list again, which then shows the row cleaning up; a Delete
/// done later reads the list again too.
#[gpui::test]
fn delete_asks_sends_the_sandbox_and_reads_the_list_again(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_with_key(&mut helm, cx);
    helm.reply(
        read.id,
        200,
        Some(leftover_list(
            &[sandbox("sb-1", "farhelm-host-k7q2-left-abcd")],
            &[],
        )),
    );
    settle(cx);

    click_in_view(cx, "tensorlake-sandbox-0-delete");
    let shown = painted(cx);
    assert!(shown.contains("Delete sandbox"), "{shown}");
    assert!(
        shown.contains("Its files, sessions and snapshots will be deleted."),
        "{shown}"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    assert_eq!(sent(&mut helm), [], "asking sends nothing");
    click_in_view(cx, "prompt-cancel");
    assert!(!drawn(cx, "prompt-confirm"), "Cancel closed the question");
    assert_eq!(sent(&mut helm), [], "Cancel sends nothing");

    // By keyboard, so the Delete holds focus on every platform's click rule before the question takes it.
    press(cx, "tensorlake-sandbox-0-delete");
    click_in_view(cx, "prompt-confirm");
    let delete = sent(&mut helm);
    assert_eq!(delete.len(), 1, "{delete:?}");
    assert_eq!(
        (delete[0].method, delete[0].path.as_str()),
        (Method::Delete, "/api/tensorlake/orphans/sb-1")
    );
    assert_eq!(busy_state(cx, "tensorlake-sandboxes-refresh"), BUSY);
    assert_eq!(busy_state(cx, "tensorlake-sandbox-0-delete"), BUSY);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("tensorlake-sandbox-0-delete"),
        "the answered question gives focus back to the Delete, busy and still holding it"
    );
    let words = "Tensorlake did not finish within 20 seconds; the cleanup carries on, and Sandboxes not on this \
                 helm shows how it ends";
    helm.reply(delete[0].id, 504, Some(json!(words)));
    settle(cx);
    assert_eq!(
        label_of(cx, "tensorlake-sandboxes-delete-error").as_deref(),
        Some(words)
    );
    let read = one_list_read(&mut helm);
    let mut running = sandbox("sb-1", "farhelm-host-k7q2-left-abcd");
    running["cleanup"] = json!({ "phase": "running" });
    helm.reply(read.id, 200, Some(leftover_list(&[], &[running])));
    settle(cx);
    assert_eq!(
        label_of(cx, "tensorlake-sandbox-0-delete").as_deref(),
        Some("Cleaning up…")
    );

    click_in_view(cx, "tensorlake-sandboxes-refresh");
    let read = one_list_read(&mut helm);
    let mut failed = sandbox("sb-1", "farhelm-host-k7q2-left-abcd");
    failed["cleanup"] = json!({ "phase": "failed", "reason": "snapshot cleanup blocked" });
    helm.reply(read.id, 200, Some(leftover_list(&[], &[failed])));
    settle(cx);
    click_in_view(cx, "tensorlake-sandbox-0-delete");
    assert!(
        !drawn(cx, "tensorlake-sandbox-0-cleanup-error"),
        "the question takes the failed cleanup's line while it is open"
    );
    click_in_view(cx, "prompt-confirm");
    let delete = sent(&mut helm);
    assert_eq!(delete.len(), 1, "{delete:?}");
    helm.reply(delete[0].id, 204, None);
    settle(cx);
    assert!(!drawn(cx, "tensorlake-sandboxes-delete-error"));
    assert!(
        !drawn(cx, "tensorlake-sandbox-0-delete"),
        "successful Delete removes its ID immediately"
    );
    let read = one_list_read(&mut helm);
    helm.reply(read.id, 200, Some(leftover_list(&[], &[])));
    settle(cx);
    assert!(status(cx).is_some_and(|status| status.starts_with("None found")));
}

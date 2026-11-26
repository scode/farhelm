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

use super::Section;
use super::recipe_tests::pressable;
use super::tensorlake_tests::{account, press, show_tensorlake};
use super::tests::{
    available, drawn, fill, into_view, label_of, open_rail, open_rail_with_hosts, painted,
    push_hosts, section_button, sent,
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

// ---- A replaced key ------------------------------------------------------------------------------------------------

/// Show Tensorlake with a key saved and its list answered with one leftover, `sb-1` named
/// `farhelm-host-k7q2-left-abcd`, and open that row's Delete question: the state every replaced-key test starts from.
fn asking_to_delete(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    let read = show_with_key(helm, cx);
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
    assert!(
        drawn(cx, "prompt-confirm"),
        "premise: the Delete question is open"
    );
}

/// Type `key` into the key form shown now and save it from the footer, returning the one save that sends.
fn save_key(helm: &mut FakeHelm, cx: &mut VisualTestContext, key: &str) -> FakeCommand {
    fill(cx, "tensorlake-key", key);
    click(cx, "settings-footer-save");
    let save = sent(helm);
    assert_eq!(save.len(), 1, "{save:?}");
    assert_eq!(
        (save[0].method, save[0].path.as_str()),
        (Method::Put, "/api/tensorlake")
    );
    save.into_iter().next().unwrap()
}

/// A key replaced in Settings → Credentials must take back the Delete question and the rows the old key listed, so
/// nothing the old key found can be deleted under the new one.
///
/// Why: the Delete question names a sandbox the old key listed, and while Credentials is shown the Tensorlake section is
/// not drawn, which takes the question away without closing it. Before the fix it came back, still open, when
/// Tensorlake was shown again, with the old rows beside it, and confirming it would delete what the old key had listed.
/// Specified: with a row listed and its Delete question open, a key saved in Credentials and answered as done reads the
/// list once; showing Tensorlake again, before that read answers, draws no question and not the old row, and says
/// "Checking…"; the read's answer then shows the new key's row and its unfinished cleanup, offering Retry cleanup.
#[gpui::test]
fn a_key_replaced_in_credentials_retires_the_question_and_the_old_rows(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    asking_to_delete(&mut helm, cx);

    click(cx, section_button(Section::Credentials));
    let reads = sent(&mut helm);
    assert_eq!(reads.len(), 1, "{reads:?}");
    assert_eq!(reads[0].path, "/api/credentials");
    helm.reply(reads[0].id, 200, Some(json!([])));
    settle(cx);
    let save = save_key(&mut helm, cx, "fictional-replacement-key");
    helm.reply(save.id, 200, Some(account(true, false)));
    settle(cx);
    let read = one_list_read(&mut helm);

    click(cx, section_button(Section::Tensorlake));
    assert_eq!(
        sent(&mut helm),
        [],
        "showing Tensorlake again reads nothing"
    );
    assert!(
        !drawn(cx, "prompt-confirm"),
        "the old key's Delete question is gone"
    );
    assert!(!painted(cx).contains("farhelm-host-k7q2-left-abcd"));
    assert_eq!(status(cx).as_deref(), Some("Checking…"));
    let mut unfinished = sandbox("sb-8", "farhelm-host-m3p8-unfinished-wxyz");
    unfinished["cleanup"] = json!({ "phase": "failed", "reason": "snapshot cleanup blocked" });
    helm.reply(
        read.id,
        200,
        Some(leftover_list(
            &[sandbox("sb-9", "farhelm-host-m3p8-new-wxyz")],
            &[unfinished],
        )),
    );
    settle(cx);
    let shown = painted(cx);
    assert!(shown.contains("farhelm-host-m3p8-new-wxyz"), "{shown}");
    assert!(
        shown.contains("farhelm-host-m3p8-unfinished-wxyz"),
        "the new key's unfinished cleanup is shown: {shown}"
    );
    assert_eq!(
        label_of(cx, "tensorlake-sandbox-1-delete").as_deref(),
        Some("Retry cleanup")
    );
}

/// A key replaced in Settings → Tensorlake's own form must close the Delete question at once and ignore a read the old
/// key had out.
///
/// Why: as for Credentials, nothing the old key listed may be deleted once the key is replaced, and before the fix the
/// question stayed open. A read sent while the save is out (a Tensorlake host arriving) is ordered after the save, so
/// the helm runs it with the new key; its answer is still dropped, because the read the save's answer starts is newer
/// and only the newest round is shown. The fake answers that dropped read with the old row so that showing it would be
/// visible. Specified: with a row listed and its
/// Delete question open, a new key saved in the section's form, and a Tensorlake host pushed while the save is out
/// (which reads the list once), the save's done answer closes the question, drops the row and reads the list once
/// more; the earlier read's answer, with a row, shows nothing and leaves the list checking; the new read's empty answer
/// says "None found".
#[gpui::test]
fn a_key_replaced_in_tensorlake_retires_the_question_and_ignores_the_old_read(
    cx: &mut TestAppContext,
) {
    use crate::hosts::tests::{connected, host};
    let (mut helm, cx, _gear) = open_rail_with_hosts(cx, &[]);
    asking_to_delete(&mut helm, cx);

    let save = save_key(&mut helm, cx, "fictional-replacement-key");
    push_hosts(&mut helm, &[host(20, "tensorlake", connected())], cx);
    let old = one_list_read(&mut helm);
    assert!(
        drawn(cx, "prompt-confirm"),
        "premise: the question is still open while the save is out"
    );
    helm.reply(save.id, 200, Some(account(true, false)));
    settle(cx);
    let new = one_list_read(&mut helm);
    assert!(!drawn(cx, "prompt-confirm"), "the question closed");
    assert!(!painted(cx).contains("farhelm-host-k7q2-left-abcd"));
    helm.reply(
        old.id,
        200,
        Some(leftover_list(
            &[sandbox("sb-1", "farhelm-host-k7q2-left-abcd")],
            &[],
        )),
    );
    settle(cx);
    assert!(
        !painted(cx).contains("farhelm-host-k7q2-left-abcd"),
        "the old key's read shows nothing"
    );
    assert_eq!(status(cx).as_deref(), Some("Checking…"));
    helm.reply(new.id, 200, Some(leftover_list(&[], &[])));
    settle(cx);
    assert!(status(cx).is_some_and(|status| status.starts_with("None found")));
}

/// A key save whose done answer cannot be read must still retire the old key's question and rows at once, and read the
/// list again once the account says a key is saved.
///
/// Why: the helm always answers a save with its settings, but if the GUI cannot decode the answer, it knows the key
/// changed without knowing the new settings. Before the fix the account kept the settings from before the save until
/// its follow-up read answered, so the old question and rows stayed usable meanwhile. Specified: with a row listed and
/// its Delete question open, a new key saved and answered as done with a body that is not the settings closes the
/// question and drops the row at once and sends one read of the account, not of the list; that read's answer, a key
/// saved, reads the list once.
#[gpui::test]
fn an_unreadable_key_save_answer_retires_the_question_and_reads_again(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    asking_to_delete(&mut helm, cx);

    let save = save_key(&mut helm, cx, "fictional-replacement-key");
    helm.reply(save.id, 200, Some(json!("saved")));
    settle(cx);
    assert!(!drawn(cx, "prompt-confirm"), "the question closed at once");
    assert!(!painted(cx).contains("farhelm-host-k7q2-left-abcd"));
    let reads = sent(&mut helm);
    assert_eq!(reads.len(), 1, "one read: {reads:?}");
    assert_eq!(
        (reads[0].method, reads[0].path.as_str()),
        (Method::Get, "/api/tensorlake")
    );
    helm.reply(reads[0].id, 200, Some(account(true, false)));
    settle(cx);
    let read = one_list_read(&mut helm);
    helm.reply(read.id, 200, Some(leftover_list(&[], &[])));
    settle(cx);
    assert!(status(cx).is_some_and(|status| status.starts_with("None found")));
}

/// A Delete still out when the key is replaced must not touch the new key's list when it answers.
///
/// Why: the Delete was asked about a row the old key listed, which the key change dropped from the list, so its refusal
/// would describe a row no longer shown. The UI channel runs ordered commands one after another, so a Delete confirmed
/// while a key save is out runs after the save, and the read the save's answer starts is ordered after the Delete,
/// which means that read already shows what the Delete did; another read for the Delete would only repeat it. Before
/// the fix the refusal's words showed under the new key's list and the list was read once more. Specified: with a
/// row's Delete question open, a new key saved and the Delete confirmed while the save is out, the save's done answer
/// reads the list once; the Delete's refusal then shows no words and reads nothing; once the new read answers empty,
/// the list says "None found" and Refresh takes presses again.
#[gpui::test]
fn a_delete_answered_after_the_key_changed_leaves_the_new_list_alone(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    asking_to_delete(&mut helm, cx);
    let save = save_key(&mut helm, cx, "fictional-replacement-key");
    click_in_view(cx, "prompt-confirm");
    let delete = sent(&mut helm);
    assert_eq!(delete.len(), 1, "{delete:?}");
    assert_eq!(
        delete[0].method,
        Method::Delete,
        "premise: the Delete is out"
    );

    helm.reply(save.id, 200, Some(account(true, false)));
    settle(cx);
    let read = one_list_read(&mut helm);
    helm.reply(
        delete[0].id,
        504,
        Some(json!("the old key's cleanup carries on")),
    );
    settle(cx);
    assert!(!drawn(cx, "tensorlake-sandboxes-delete-error"));
    assert_eq!(sent(&mut helm), [], "the old key's Delete reads nothing");
    helm.reply(read.id, 200, Some(leftover_list(&[], &[])));
    settle(cx);
    assert!(status(cx).is_some_and(|status| status.starts_with("None found")));
    assert_eq!(busy_state(cx, "tensorlake-sandboxes-refresh"), READY);
}

/// A Delete confirmed while a Clear of the key is out must not bring the hidden list back when it answers.
///
/// Why: with no key saved the list is drawn only while it has something to show, and a Delete's refusal counts. Before
/// the fix a Delete answered after the Clear showed its refusal under a list that should be gone. Specified: with a
/// row's Delete question open, Clear pressed in the section's form and the Delete confirmed while the Clear is out,
/// the Clear's answer (no key saved) hides the list; the Delete's refusal then draws neither the heading nor its words,
/// and nothing more is sent.
#[gpui::test]
fn a_delete_answered_after_a_clear_leaves_the_list_hidden(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    asking_to_delete(&mut helm, cx);
    press(cx, "tensorlake-key-clear");
    let clear = sent(&mut helm);
    assert_eq!(clear.len(), 1, "{clear:?}");
    assert_eq!(
        (clear[0].method, clear[0].path.as_str()),
        (Method::Put, "/api/tensorlake")
    );
    click_in_view(cx, "prompt-confirm");
    let delete = sent(&mut helm);
    assert_eq!(delete.len(), 1, "{delete:?}");
    assert_eq!(
        delete[0].method,
        Method::Delete,
        "premise: the Delete went out after the Clear"
    );

    helm.reply(clear[0].id, 200, Some(account(false, false)));
    settle(cx);
    assert!(!drawn(cx, "prompt-confirm"), "the question is gone");
    assert!(
        !drawn(cx, "tensorlake-sandboxes-heading"),
        "the list is hidden"
    );
    helm.reply(
        delete[0].id,
        504,
        Some(json!("the old key's cleanup carries on")),
    );
    settle(cx);
    assert!(
        !drawn(cx, "tensorlake-sandboxes-heading")
            && !drawn(cx, "tensorlake-sandboxes-delete-error"),
        "the Delete's answer brings nothing back"
    );
    assert_eq!(sent(&mut helm), [], "nothing reads the list without a key");
}

/// A key cleared while its Delete question is still open must take the question, and with it the list, away.
///
/// Why: with no key saved the list is drawn only while it has something to show, and an open question counts. Before
/// the fix a Clear emptied the rows but kept the question, so the list stayed drawn, empty, with the old key's Delete
/// question under it, still able to delete a sandbox no saved key listed. Specified: with a row's Delete question open,
/// a Clear pressed in the section's form and answered (no key saved) leaves neither the question nor the list's heading
/// drawn, and sends nothing more.
#[gpui::test]
fn a_clear_answered_while_the_question_is_open_retires_it(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    asking_to_delete(&mut helm, cx);
    press(cx, "tensorlake-key-clear");
    let clear = sent(&mut helm);
    assert_eq!(clear.len(), 1, "{clear:?}");
    assert!(
        drawn(cx, "prompt-confirm"),
        "premise: the question is still open while the Clear is out"
    );
    helm.reply(clear[0].id, 200, Some(account(false, false)));
    settle(cx);
    assert!(!drawn(cx, "prompt-confirm"), "the question is gone");
    assert!(
        !drawn(cx, "tensorlake-sandboxes-heading"),
        "the list is hidden"
    );
    assert_eq!(sent(&mut helm), []);
}

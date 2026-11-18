//! Tests of an environment session's Delete (`environment_delete.rs`): which lane a click asks in from the hosts read,
//! each question's words from the row, the header and the Delete shortcut and that none is ever skipped, the exact
//! request of each lane, the Delete notice band's "stays" count and its retirement, a 404 counted as deleted, an
//! "unasked" answer asking the whole lane again at the row and at the header, the 504 at the helm's bound left to the
//! pushed state, a refusal shown, and an answer given once the environment is being deleted sending nothing.
//!
//! The window tests run the real main view over the helm client's fake with the environment fixtures of
//! `environment_tests.rs`: host 2 is the session's sandbox or container, named "owned-environment", and the first row
//! is the open session. Every command but a Delete is answered with success at once ([`deletes`]). The rules without a
//! window (the lane, the commands, the band's state, the `Work` variant's two answers) are tested on their own.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{Availability, Collection, CollectionChanges, HostView};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::delete::{self, MANAGED_CHECKOUT};
use super::delete_notice::{Shown, TEXT_ID};
use super::environment_delete::{Lane, command, lane};
use super::environment_tests::{
    Provider, connected, deleting, environment_host, open, push_host, registry, suspended_at,
};
use super::host_rail::tests::hover_help;
use super::menu_tests::{any_layer, drawn, ended, label_of};
use super::tests::{collections, painted, session};
use crate::actions::{DELETE_SESSION_CHORD, DELETE_SESSION_SHORTCUT_NAME};
use crate::operations::{HelmMarker, OpKey, Work};
use crate::session as ui_session;
use crate::test_support::{self, focused_id, frame, meta};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// The session lane's question line for a sandbox: its consequence, then the environment's name.
const SANDBOX_SESSION_LINE: &str = "Delete this session and its checkout? A checkout another session still uses stays. \
                                    If this is the last session in its sandbox, the sandbox goes too, with every \
                                    process and file in it and its snapshots. This cannot be undone. sandbox: \
                                    owned-environment";

/// The whole lane's question line for a sandbox: its warning, then the environment's name.
const SANDBOX_WHOLE_LINE: &str = "Farhelm cannot ask this sandbox's own Farhelm to delete just this session, so this \
                                  deletes the whole sandbox: every session, running process, file, checkout, unpushed \
                                  change and snapshot in it, including sessions Farhelm does not list because they \
                                  were started while it was not connected. This cannot be undone. Resuming or \
                                  updating the sandbox first lets Farhelm delete just this session. sandbox: \
                                  owned-environment";

/// The session lane's help, and its answer's, for a sandbox.
const SANDBOX_SESSION_HELP: &str =
    "Delete this session and its checkout; the sandbox goes too if this is its last session";

/// The whole lane's help, and its answer's, for a sandbox.
const SANDBOX_WHOLE_HELP: &str =
    "Delete this session together with its whole sandbox and everything in it";

/// What the helm answers when it could not ask the environment's own Farhelm (`sandbox.rs`'s `EnvironmentUnasked`).
const UNASKED: &str = "this environment's own Farhelm could not be asked to delete the session; nothing was \
                       decided, and an unanswered request may still have been acted on, which the next listing shows";

/// The Delete requests the window sent since the last call, as (id, path), left unanswered. Every other command (a
/// preference write, a seen mark) is answered with success at once; the fake answers in order, so this works only while
/// nothing sent before it waits.
fn deletes(fake: &mut FakeHelm) -> Vec<(u64, String)> {
    let mut found = Vec::new();
    for command in fake.commands() {
        if command.method == farhelm_helm_client::Method::Delete {
            found.push((command.id, command.path));
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    found
}

/// The one Delete sent since the last call, which must be on `path`: its id.
fn one_delete(fake: &mut FakeHelm, path: &str) -> u64 {
    let sent = deletes(fake);
    assert_eq!(
        sent.iter()
            .map(|(_, path)| path.as_str())
            .collect::<Vec<_>>(),
        [path],
        "exactly one Delete"
    );
    sent[0].0
}

/// Open the menu of the row whose menu button is `button`, and choose Delete.
fn choose_delete(cx: &mut VisualTestContext, button: &'static str) {
    test_support::click(cx, button);
    test_support::click(cx, "session-menu-delete");
    test_support::settle(cx);
}

/// Every line of text painted in a fresh frame.
fn texts(cx: &mut VisualTestContext) -> Vec<String> {
    painted(&frame(cx))
}

/// The work in flight on session `key`'s own key.
fn in_flight(cx: &mut VisualTestContext, key: &str) -> Option<Work> {
    cx.update(|_, cx| {
        ui_session::current_operations(cx)
            .and_then(|operations| operations.read(cx).in_flight(&OpKey::session(key)))
    })
}

/// Whether this window's own Delete of `key` succeeded (`delete::deleted_here`).
fn deleted_here(cx: &mut VisualTestContext, key: &str) -> bool {
    cx.update(|_, cx| delete::deleted_here(key, cx))
}

/// Push the hosts collection as a read that failed: its entries kept, its availability unavailable.
fn push_failed_hosts_read(fake: &mut FakeHelm, cx: &mut VisualTestContext) {
    let mut failed = meta(&[HostView::key(1).as_str(), HostView::key(2).as_str()]);
    failed.availability = Availability::Unavailable {
        error: "the hosts read failed".into(),
    };
    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: failed,
        upsert: Vec::new(),
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// Push the hosts collection without host 2, as the helm pushes a host it forgot.
fn push_host_gone(fake: &mut FakeHelm, cx: &mut VisualTestContext) {
    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&[HostView::key(1).as_str()]),
        upsert: Vec::new(),
        remove: vec![HostView::key(2)],
    });
    test_support::settle(cx);
}

// ---- The lane and the questions --------------------------------------------------------------------------------------

/// SPEC.md: the whole-sandbox Delete is offered only "when the helm cannot ask the sandbox's Farhelm", and the old app
/// decided that from a current hosts read alone, so that "an uncertain read can never widen the consent". Choosing it
/// from a stale or failed read, or from the provider, would destroy sessions the user was never warned about.
/// Specified: a host listed connected picks the session lane, one listed suspended the whole lane; with no hosts read
/// yet, with the host gone from a successful read, and with a failed read that still holds the suspended host, the
/// session lane.
#[test]
fn the_lane_is_whole_only_when_a_current_read_lists_the_host_not_connected() {
    let connected_host = environment_host(Provider::Sandbox, connected());
    let suspended = environment_host(Provider::Sandbox, suspended_at(0));
    assert_eq!(
        lane(&collections(vec![registry(&connected_host)]), 2),
        Lane::Session
    );
    assert_eq!(
        lane(&collections(vec![registry(&suspended)]), 2),
        Lane::Whole
    );
    assert_eq!(lane(&collections(Vec::new()), 2), Lane::Session, "pending");
    assert_eq!(
        lane(&collections(vec![registry(&suspended)]), 3),
        Lane::Session,
        "absent"
    );
    let mut failed = registry(&suspended);
    failed.meta.availability = Availability::Unavailable {
        error: "the hosts read failed".into(),
    };
    assert_eq!(lane(&collections(vec![failed]), 2), Lane::Session, "failed");
}

/// The two lanes are two distinct opt-ins at the helm, and neither may carry the ordinary Delete's guard, which the
/// helm refuses for an environment Delete. Specified: the session lane is `DELETE
/// /api/sessions/a?delete_environment_session=true` and the whole lane `DELETE /api/sessions/a?delete_sandbox=true`,
/// each as `Work::EnvironmentDelete`; that work creates no session and changes no registry entry, so launches and host
/// changes do not wait for it.
#[test]
fn each_lane_sends_its_own_flag_as_environment_delete_work() {
    for (lane, path) in [
        (
            Lane::Session,
            "/api/sessions/a?delete_environment_session=true",
        ),
        (Lane::Whole, "/api/sessions/a?delete_sandbox=true"),
    ] {
        let built = command("a", lane).unwrap();
        assert_eq!(built.work(), Work::EnvironmentDelete);
        assert_eq!(built.path().as_str(), path, "{lane:?}");
    }
    assert!(!Work::EnvironmentDelete.creates_a_session());
    assert!(!Work::EnvironmentDelete.changes_host_registry());
}

/// The old app's e2e "a connected sandbox's Delete asks its Farhelm and reports the sandbox that stays": with Skip
/// session confirmations on and nothing alive, an ordinary session is deleted unasked, but SPEC.md never skips an
/// environment Delete, since one outcome destroys the sandbox; and the count of sessions that stay is the sandbox's
/// own, which this window cannot derive from its list. Specified, skip on, a ended with no tabs and b beside it on a
/// connected sandbox: a's menu says Delete's lane help; Delete asks, with the session lane's consequence and the
/// sandbox's name, the quoted title, no managed-checkout line, and "Delete session" helped by the lane's help; nothing
/// is sent before the answer; the answer sends exactly `?delete_environment_session=true`, held on a's key as
/// environment Delete work; its success with a count of 2 shows "The sandbox stays: 2 other sessions are in it" in the
/// band and counts as this window's Delete; once the hosts read no longer lists the sandbox, the band is gone.
#[gpui::test]
fn a_connected_sandboxs_delete_asks_its_farhelm_and_says_what_stays(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(ended("a"), 0), (session("b"), 0)], true, &host);
    deletes(&mut fake);
    test_support::click(cx, "session-menu-a");
    assert!(
        texts(cx).iter().any(|text| text == SANDBOX_SESSION_HELP),
        "the menu's Delete names the session lane"
    );
    test_support::click(cx, "session-menu-delete");
    test_support::settle(cx);
    assert!(
        drawn(cx, "prompt"),
        "asked, though skipped and nothing alive"
    );
    assert_eq!(
        label_of(cx, "session-delete-consequence-a").as_deref(),
        Some(SANDBOX_SESSION_LINE)
    );
    assert_eq!(
        label_of(cx, "session-delete-title-a").as_deref(),
        Some("\"a\"")
    );
    assert!(
        !texts(cx).iter().any(|text| text == MANAGED_CHECKOUT),
        "no managed-checkout line"
    );
    assert_eq!(
        label_of(cx, "prompt-confirm").as_deref(),
        Some("Delete session")
    );
    assert_eq!(
        hover_help(cx, "prompt-confirm").as_deref(),
        Some(SANDBOX_SESSION_HELP)
    );
    assert_eq!(deletes(&mut fake), [], "nothing before the answer");

    test_support::click(cx, "prompt-confirm");
    let id = one_delete(&mut fake, "/api/sessions/a?delete_environment_session=true");
    assert_eq!(in_flight(cx, "a"), Some(Work::EnvironmentDelete));
    fake.reply(id, 200, Some(json!({"environment_remaining": 2})));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, TEXT_ID).as_deref(),
        Some("The sandbox stays: 2 other sessions are in it")
    );
    assert!(deleted_here(cx, "a"));
    assert!(!drawn(cx, "session-outcome-a"));

    push_host_gone(&mut fake, cx);
    assert!(!drawn(cx, TEXT_ID), "the count retired with the sandbox");
}

/// The old e2e "a connected container's Delete asks its Farhelm and reports the container that stays" and "a sandbox
/// destroyed elsewhere retires its stays count but keeps the checkout notice": the count is true only until the
/// environment goes, but a checkout notice the same reply carried must never be silent, and a failed hosts read says
/// nothing about whether it went. Specified, on a connected container: the question's line is the container's ("...
/// in its container, the container goes too, with every process and file in it. This cannot be undone. container:
/// owned-environment", no snapshots); a success with a count of 1 and a notice shows "The container stays: 1 other
/// session is in it. <notice>"; a failed hosts read leaves it; the container pushed as being deleted leaves the notice
/// alone.
#[gpui::test]
fn a_containers_count_retires_when_it_is_deleted_and_its_notice_stays(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Container, connected());
    let (mut fake, cx) = open(cx, &[(session("a"), 0), (session("b"), 0)], false, &host);
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    assert_eq!(
        label_of(cx, "session-delete-consequence-a").as_deref(),
        Some(
            "Delete this session and its checkout? A checkout another session still uses stays. If this is the last \
             session in its container, the container goes too, with every process and file in it. This cannot be \
             undone. container: owned-environment"
        )
    );
    test_support::click(cx, "prompt-confirm");
    let id = one_delete(&mut fake, "/api/sessions/a?delete_environment_session=true");
    let notice = "Farhelm left this session's checkout in place.";
    fake.reply(
        id,
        200,
        Some(json!({"environment_remaining": 1, "notice": notice})),
    );
    test_support::settle(cx);
    let counted = format!("The container stays: 1 other session is in it. {notice}");
    assert_eq!(label_of(cx, TEXT_ID), Some(counted.clone()));

    push_failed_hosts_read(&mut fake, cx);
    assert_eq!(
        label_of(cx, TEXT_ID),
        Some(counted),
        "a failed read proves nothing"
    );
    push_host(
        &mut fake,
        &environment_host(Provider::Container, deleting(None)),
        cx,
    );
    assert_eq!(label_of(cx, TEXT_ID).as_deref(), Some(notice));
}

/// The old e2e "owned suspended sessions keep Hosts visibility and mandatory whole-sandbox Delete", its Delete part: a
/// suspended sandbox's Farhelm cannot be asked, so the only Delete is the whole sandbox's, and its question must warn
/// about every session in it, listed or not, before anything is sent. Specified, skip on, a running on a suspended
/// sandbox: the header's Delete is helped by "Delete this session together with its whole sandbox and everything in
/// it, after you confirm (<shortcut>)"; it asks with the warning and the sandbox's name, answered "Delete session and
/// sandbox". Focus starts on Cancel because the Delete destroys everything in the sandbox. Confirming sends exactly
/// `?delete_sandbox=true`. Its success shows no band and counts as this window's Delete.
#[gpui::test]
fn a_suspended_sandboxs_delete_is_the_warned_whole_sandbox_delete(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, suspended_at(0));
    let (mut fake, cx) = open(cx, &[(session("a"), 0)], true, &host);
    deletes(&mut fake);
    assert_eq!(
        hover_help(cx, "session-header-delete"),
        Some(format!(
            "{SANDBOX_WHOLE_HELP}, after you confirm ({DELETE_SESSION_SHORTCUT_NAME})"
        ))
    );
    test_support::click(cx, "session-header-delete");
    assert_eq!(
        label_of(cx, "session-delete-consequence-a").as_deref(),
        Some(SANDBOX_WHOLE_LINE)
    );
    assert_eq!(
        label_of(cx, "prompt-confirm").as_deref(),
        Some("Delete session and sandbox")
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    assert_eq!(deletes(&mut fake), [], "nothing before the answer");
    test_support::click(cx, "prompt-confirm");
    let id = one_delete(&mut fake, "/api/sessions/a?delete_sandbox=true");
    fake.reply(id, 200, Some(json!({})));
    test_support::settle(cx);
    assert!(deleted_here(cx, "a"));
    assert!(!drawn(cx, TEXT_ID), "the whole Delete reports no count");
}

/// SPEC.md, "Container sessions": a paused container's Farhelm cannot be asked either, so the row too offers only the
/// whole-container Delete, whose warning must name the container's own losses (no snapshots, which only a sandbox has).
/// Specified, on a paused container: a's menu describes Delete as "Delete this session together with its whole
/// container and everything in it"; Delete asks with the container's warning and its name, answered "Delete session
/// and container" helped by that same description. Focus starts on Cancel because everything in the container would
/// be lost. Confirming sends exactly `?delete_sandbox=true`.
#[gpui::test]
fn a_paused_containers_row_asks_the_whole_container_delete(cx: &mut TestAppContext) {
    const CONTAINER_WHOLE_HELP: &str =
        "Delete this session together with its whole container and everything in it";
    let host = environment_host(Provider::Container, suspended_at(0));
    let (mut fake, cx) = open(cx, &[(session("a"), 0)], false, &host);
    deletes(&mut fake);
    test_support::click(cx, "session-menu-a");
    assert!(
        texts(cx).iter().any(|text| text == CONTAINER_WHOLE_HELP),
        "the menu's Delete names the whole lane"
    );
    test_support::click(cx, "session-menu-delete");
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-delete-consequence-a").as_deref(),
        Some(
            "Farhelm cannot ask this container's own Farhelm to delete just this session, so this deletes the \
             whole container: every session, running process, file, checkout and unpushed change in it, including \
             sessions Farhelm does not list because they were started while it was not connected. This cannot be \
             undone. Resuming or updating the container first lets Farhelm delete just this session. container: \
             owned-environment"
        )
    );
    assert_eq!(
        label_of(cx, "prompt-confirm").as_deref(),
        Some("Delete session and container")
    );
    assert_eq!(
        hover_help(cx, "prompt-confirm").as_deref(),
        Some(CONTAINER_WHOLE_HELP)
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    assert_eq!(deletes(&mut fake), [], "nothing before the answer");
    test_support::click(cx, "prompt-confirm");
    one_delete(&mut fake, "/api/sessions/a?delete_sandbox=true");
}

/// A real container run read "Stopping agent…" over a paused container while its whole-container Delete ran, where
/// nothing can be stopped; the old app said the same (it chose a pending Delete's words from the status the row drew,
/// an environment Delete included), so these are the ported words, pinned here so a change to them is a decision.
/// Specified, on a paused container with a running, stale and open: the suspended card shows first ("Session
/// suspended"); once the whole-container Delete is confirmed and while it is unanswered, the row's progress, the
/// header's progress in place of its actions and the scrim over the card are each named "Stopping agent…".
#[gpui::test]
fn a_whole_container_delete_says_what_the_row_showed_running(cx: &mut TestAppContext) {
    use crate::session_pane::unavailable::HEADING_ID;
    let host = environment_host(Provider::Container, suspended_at(0));
    let (mut fake, cx) = open(cx, &[(session("a").stale(), 0)], false, &host);
    deletes(&mut fake);
    assert_eq!(
        label_of(cx, HEADING_ID).as_deref(),
        Some("Session suspended"),
        "the premise: the suspended card shows"
    );
    test_support::click(cx, "session-header-delete");
    test_support::click(cx, "prompt-confirm");
    one_delete(&mut fake, "/api/sessions/a?delete_sandbox=true");
    test_support::settle(cx);
    assert_eq!(in_flight(cx, "a"), Some(Work::EnvironmentDelete));
    for id in [
        "session-delete-progress-a",
        "session-header-delete-progress-a",
        "terminal-delete-scrim",
    ] {
        assert_eq!(label_of(cx, id).as_deref(), Some("Stopping agent…"), "{id}");
    }
}

/// SPEC.md's header Delete "asks first in the same anchored way", and the Delete shortcut is that Delete; for an
/// environment session neither may take the skipped path, whose help ("Delete: remove this session and its state
/// (…)") would promise a Delete without a question. Specified, skip on, a ended with no tabs on a connected sandbox:
/// the header's Delete is helped by the session lane's help ", after you confirm (<shortcut>)"; it asks the session
/// lane's question; Cancel closes it; the Delete shortcut then asks the same question; Cancel again; nothing was sent.
#[gpui::test]
fn the_header_and_the_shortcut_always_ask_an_environment_delete(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(ended("a"), 0)], true, &host);
    deletes(&mut fake);
    assert_eq!(
        hover_help(cx, "session-header-delete"),
        Some(format!(
            "{SANDBOX_SESSION_HELP}, after you confirm ({DELETE_SESSION_SHORTCUT_NAME})"
        ))
    );
    test_support::click(cx, "session-header-delete");
    assert_eq!(
        label_of(cx, "session-delete-consequence-a").as_deref(),
        Some(SANDBOX_SESSION_LINE)
    );
    test_support::click(cx, "prompt-cancel");
    assert!(
        !any_layer(cx),
        "the premise: Cancel closed the header's question"
    );
    test_support::keys(cx, DELETE_SESSION_CHORD);
    assert_eq!(
        label_of(cx, "session-delete-consequence-a").as_deref(),
        Some(SANDBOX_SESSION_LINE),
        "the shortcut asks the same question"
    );
    test_support::click(cx, "prompt-cancel");
    assert_eq!(deletes(&mut fake), [], "nothing was sent");
}

// ---- "Unasked" ------------------------------------------------------------------------------------------------------

/// Confirm the open session-lane Delete, check that its question closed, then answer the request 409 with the helm's
/// "unasked" marker, which is what makes the whole-environment question reopen by itself.
fn reply_unasked(fake: &mut FakeHelm, cx: &mut VisualTestContext) {
    test_support::click(cx, "prompt-confirm");
    let id = one_delete(fake, "/api/sessions/a?delete_environment_session=true");
    assert!(!any_layer(cx), "the premise: the first question closed");
    fake.reply_marked(
        id,
        409,
        vec![HelmMarker::EnvironmentUnasked],
        Some(json!(UNASKED)),
    );
    test_support::settle(cx);
}

/// When the helm could not ask the sandbox's Farhelm, the row offers the warned whole-sandbox Delete. A fresh Enter
/// dismisses the new question without sending anything, since the user only agreed to the session-lane request.
/// The warning replaces the row's outcome line, and an explicit confirmation on a retry sends the whole Delete.
#[gpui::test]
fn a_rows_unasked_delete_offers_the_whole_sandbox_delete(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(session("a"), 0)], false, &host);
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    reply_unasked(&mut fake, cx);
    assert_eq!(
        label_of(cx, "session-delete-consequence-a").as_deref(),
        Some(SANDBOX_WHOLE_LINE)
    );
    assert_eq!(
        label_of(cx, "prompt-confirm").as_deref(),
        Some("Delete session and sandbox")
    );
    assert!(!drawn(cx, "session-outcome-a"), "the question says it");
    assert_eq!(deletes(&mut fake), [], "nothing before the warned answer");
    test_support::keys(cx, "enter");
    test_support::settle(cx);
    assert!(!any_layer(cx), "Enter dismisses the question");
    assert_eq!(
        deletes(&mut fake),
        [],
        "Enter sends no whole-sandbox Delete"
    );

    choose_delete(cx, "session-menu-a");
    reply_unasked(&mut fake, cx);
    test_support::click(cx, "prompt-confirm");
    let id = one_delete(&mut fake, "/api/sessions/a?delete_sandbox=true");
    fake.reply(id, 200, Some(json!({})));
    test_support::settle(cx);
    assert!(deleted_here(cx, "a"));
}

/// The header's whole-sandbox question can arrive while the user waits for the session-lane reply. A fresh Enter
/// dismisses it without authorizing deletion of the sandbox. An explicit confirmation on a retry sends the whole
/// Delete, whose refusal still shows on the row and below the header.
#[gpui::test]
fn the_headers_unasked_delete_reopens_as_the_whole_sandbox_delete(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(session("a"), 0)], false, &host);
    deletes(&mut fake);
    assert_eq!(
        hover_help(cx, "session-header-delete"),
        Some(format!(
            "{SANDBOX_SESSION_HELP}, after you confirm ({DELETE_SESSION_SHORTCUT_NAME})"
        ))
    );
    test_support::click(cx, "session-header-delete");
    reply_unasked(&mut fake, cx);
    assert_eq!(
        label_of(cx, "session-delete-consequence-a").as_deref(),
        Some(SANDBOX_WHOLE_LINE)
    );
    test_support::keys(cx, "enter");
    test_support::settle(cx);
    assert!(!any_layer(cx), "Enter dismisses the question");
    assert_eq!(
        deletes(&mut fake),
        [],
        "Enter sends no whole-sandbox Delete"
    );

    test_support::click(cx, "session-header-delete");
    reply_unasked(&mut fake, cx);
    test_support::click(cx, "prompt-confirm");
    let id = one_delete(&mut fake, "/api/sessions/a?delete_sandbox=true");
    fake.reply(id, 409, Some(json!("Tensorlake refused the deletion")));
    test_support::settle(cx);
    assert!(!any_layer(cx), "the whole lane asks nothing more");
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("Tensorlake refused the deletion")
    );
    assert_eq!(
        label_of(cx, "session-header-outcome-a").as_deref(),
        Some("Tensorlake refused the deletion")
    );
}

/// A completion must not open a dialog once the user has moved on (SPEC.md, "Who may move focus"), so an "unasked"
/// answer that arrives after a key press cannot ask; its words are then the only report and must stay. Specified: a
/// row's session-lane Delete answered, after a key press, 409 unasked opens nothing and leaves the helm's words on the
/// row.
#[gpui::test]
fn an_unasked_answer_after_the_user_moved_on_stays_on_the_row(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(session("a"), 0)], false, &host);
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    test_support::click(cx, "prompt-confirm");
    let id = one_delete(&mut fake, "/api/sessions/a?delete_environment_session=true");
    test_support::keys(cx, "escape");
    fake.reply_marked(
        id,
        409,
        vec![HelmMarker::EnvironmentUnasked],
        Some(json!(UNASKED)),
    );
    test_support::settle(cx);
    assert!(!any_layer(cx), "nothing opens behind the user's back");
    assert_eq!(label_of(cx, "session-outcome-a").as_deref(), Some(UNASKED));
}

// ---- Other answers -------------------------------------------------------------------------------------------------

/// The helm answers within 20 seconds and carries on past that (decided by the migration's orchestrator on
/// 2026-11-16), so the 504 that says so is routine: shown as a failure it would invite a second Delete while the first
/// still runs. A real refusal is different and must show. Specified: a session-lane Delete answered 504 leaves no line
/// on the row and frees a's key; a second Delete answered 409 "busy, nothing was deleted" shows those words on the row.
#[gpui::test]
fn a_504_at_the_bound_is_not_a_failure_but_a_refusal_shows(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(session("a"), 0)], false, &host);
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    test_support::click(cx, "prompt-confirm");
    let id = one_delete(&mut fake, "/api/sessions/a?delete_environment_session=true");
    fake.reply(
        id,
        504,
        Some(json!(
            "this environment's own Farhelm did not answer within 20 seconds; it may still delete this session"
        )),
    );
    test_support::settle(cx);
    assert!(!drawn(cx, "session-outcome-a"), "the 504 is not a failure");
    assert_eq!(in_flight(cx, "a"), None);

    choose_delete(cx, "session-menu-a");
    test_support::click(cx, "prompt-confirm");
    let id = one_delete(&mut fake, "/api/sessions/a?delete_environment_session=true");
    fake.reply(id, 409, Some(json!("busy, nothing was deleted")));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("busy, nothing was deleted")
    );
}

/// The old app treated a 404 as deleted: an earlier Delete, maybe another window's, sealed and destroyed the
/// environment, and a later Delete on the same environment without a count makes an earlier "stays" count untrue.
/// Specified: b's Delete with a count of 2 shows "The sandbox stays: 2 other sessions are in it"; a's Delete answered
/// 404 then shows no line on a's row, counts as this window's Delete, and takes the count away.
#[gpui::test]
fn a_404_counts_as_deleted_and_retires_the_count(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(session("a"), 0), (session("b"), 0)], false, &host);
    deletes(&mut fake);
    choose_delete(cx, "session-menu-b");
    test_support::click(cx, "prompt-confirm");
    let id = one_delete(&mut fake, "/api/sessions/b?delete_environment_session=true");
    fake.reply(id, 200, Some(json!({"environment_remaining": 2})));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, TEXT_ID).as_deref(),
        Some("The sandbox stays: 2 other sessions are in it"),
        "the premise: a count shows"
    );

    choose_delete(cx, "session-menu-a");
    test_support::click(cx, "prompt-confirm");
    let id = one_delete(&mut fake, "/api/sessions/a?delete_environment_session=true");
    fake.reply(id, 404, Some(json!("no such session")));
    test_support::settle(cx);
    assert!(!drawn(cx, "session-outcome-a"));
    assert!(deleted_here(cx, "a"));
    assert!(!drawn(cx, TEXT_ID), "the count is retired");
}

/// SPEC.md: a session whose environment is being deleted has its actions unavailable, and the old question's answer
/// was disabled once that began. A question opened just before must not send a Delete into a destruction under way.
/// Specified: a's Delete question opened on a connected sandbox, answered after the sandbox is pushed as being deleted,
/// sends nothing.
#[gpui::test]
fn an_answer_after_the_environment_began_being_deleted_sends_nothing(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(session("a"), 0)], false, &host);
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    assert!(drawn(cx, "prompt"), "the premise: the question is open");
    push_host(
        &mut fake,
        &environment_host(Provider::Sandbox, deleting(None)),
        cx,
    );
    test_support::click(cx, "prompt-confirm");
    assert_eq!(deletes(&mut fake), [], "nothing was sent");
    assert_eq!(in_flight(cx, "a"), None);
}

// ---- The band's rules, without a window ------------------------------------------------------------------------------

/// The old `environment_band`: a count describes one environment only until something else happens to it, while a
/// notice the same reply carried must outlive the count. Specified, by what the band shows (the count's words, then the
/// notice): a count for host 2 with a notice shows both; a success on host 3 without a count leaves them; a success on
/// host 2 without a count leaves the notice alone; a new count and then a plain notice from any Delete shows that
/// notice alone, which a later success on host 2 without a count leaves as it is, since the count is gone. The hosts
/// read's retirement, and the band's drawing of both parts as one sentence, are the window tests'.
#[test]
fn the_band_keeps_a_count_until_its_environment_moves_on() {
    let shows = |band: &Shown| {
        (
            band.kept.as_ref().map(|kept| kept.words.clone()),
            band.notice.clone(),
        )
    };
    let both = (Some("kept".to_owned()), Some("notice".to_owned()));
    let mut band = Shown::default();
    band.environment_delete(2, Some("kept".into()), Some("notice".into()));
    assert_eq!(shows(&band), both);
    band.environment_delete(3, None, None);
    assert_eq!(shows(&band), both, "another host");
    band.environment_delete(2, None, None);
    assert_eq!(shows(&band), (None, Some("notice".to_owned())));

    band.environment_delete(2, Some("kept".into()), None);
    assert_eq!(shows(&band), (Some("kept".to_owned()), None));
    band.notice(Some("other".into()));
    assert_eq!(shows(&band), (None, Some("other".to_owned())));
    band.environment_delete(2, None, None);
    assert_eq!(
        shows(&band),
        (None, Some("other".to_owned())),
        "no count left to retire"
    );
}

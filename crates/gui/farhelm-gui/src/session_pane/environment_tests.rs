//! Tests of an environment session in the open session's view: the header's Suspend or Resume left of Stop, with
//! Stop, Restart and Replace withheld while the environment is frozen and Clone and Delete kept, a pending Resume
//! refusing Delete, the header's Suspend asking under its button after the activity read, and every action and the
//! Delete shortcut refused while the environment is being deleted, with the reason in the help and the phase in the
//! details bar; and the suspended card's words, its Resume (or Resume host for an ordinary host) and a refused Resume
//! shown on the card.
//!
//! The window tests run the real main view over the helm client's fake with the session list's environment fixtures
//! (`session_list/environment_tests.rs`): host 2 is the open session's sandbox or container. The card's words are a
//! pure function ([`notice`]), tested without a window. The header's control is `session-header-environment`; the
//! card's Resume `session-unavailable-resume` and its refusal `session-unavailable-resume-failure`.

use farhelm_helm_client::Method;
use farhelm_proto::tensorlake::LifecycleDirection;
use farhelm_proto::ui_channel::HostStateView;
use gpui::TestAppContext;
use serde_json::json;

use super::details::{AGE_ID, DELETING_ID};
use super::header::{Action, ENVIRONMENT_ID};
use super::unavailable::{
    DETAIL_ID, EXPLANATION_ID, HEADING_ID, Notice, RESUME_FAILURE_ID, RESUME_ID, notice,
};
use crate::actions::DELETE_SESSION_CHORD;
use crate::local_time::Zone;
use crate::operations::{OpKey, Work};
use crate::session as ui_session;
use crate::session_list::environment_tests::{
    Provider, changing, connected, deleting, environment_host, idle, listing, one, open, pending,
    push_host, suspended_at,
};
use crate::session_list::host_rail::tests::hover_help;
use crate::session_list::menu_tests::{any_layer, drawn, label_of};
use crate::session_list::status::HostLookup;
use crate::session_list::tests::{selected, session};
use crate::test_support::{self, frame};

/// The left edge of the element drawn under `id`.
fn left(cx: &mut gpui::VisualTestContext, id: &str) -> f32 {
    let node = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("{id} is drawn"));
    f32::from(node.bounds.origin.x)
}

// ---- The header ----------------------------------------------------------------------------------------------------

/// SPEC.md, "Sandbox sessions": "The session toolbar offers Suspend immediately left of Stop", and "While suspended,
/// Resume is the filled primary control with a play glyph; Stop, Restart and Replace are disabled, while Clone and
/// Delete remain available"; a pending Resume refuses the session's competing actions (the old e2e "an owned session's
/// pending Resume survives switching and excludes its competing actions"). Specified: for a connected sandbox session,
/// Suspend is drawn just left of Stop with the provider's help, and Stop's help is "The sandbox keeps running and
/// billing until you suspend or delete it."; once suspended the control reads Resume with the help "Resume this
/// environment; preserved processes continue", a click on Stop, Restart, Replace or either caret opens and sends
/// nothing (no restart or replace either) while Delete still asks; Resume sends `POST /api/hosts/2/resume` with no
/// question, on the session's key, the control is named "Resuming…" with its label still
/// "Resume", and Delete and Clone open nothing while it is pending; after the helm's 504 at its bound, with the host
/// pushed resuming, they still open nothing; once the host is connected, Suspend is back and Delete asks again.
#[gpui::test]
fn the_header_suspends_and_resumes_beside_stop(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(idle("a"), 0)], false, &host);
    pending(&mut fake);
    let (control, stop) = (ENVIRONMENT_ID, Action::Stop.id());
    assert_eq!(label_of(cx, control).as_deref(), Some("Suspend"));
    assert!(
        left(cx, control) < left(cx, stop),
        "Suspend is left of Stop"
    );
    assert_eq!(
        hover_help(cx, "session-header-environment").as_deref(),
        Some(
            "Freeze the sandbox; its processes continue on Resume. Only snapshot storage bills while suspended."
        )
    );
    assert_eq!(
        hover_help(cx, "session-header-stop").as_deref(),
        Some("The sandbox keeps running and billing until you suspend or delete it.")
    );

    push_host(
        &mut fake,
        &environment_host(Provider::Sandbox, suspended_at(0)),
        cx,
    );
    assert_eq!(label_of(cx, control).as_deref(), Some("Resume"));
    assert_eq!(
        hover_help(cx, "session-header-environment").as_deref(),
        Some("Resume this environment; preserved processes continue")
    );
    for frozen in [
        "session-header-stop",
        "session-header-restart",
        "session-header-replace",
        "session-header-restart-options",
        "session-header-replace-options",
    ] {
        test_support::click(cx, frozen);
        assert!(!any_layer(cx), "{frozen} opens nothing while suspended");
    }
    assert!(pending(&mut fake).is_empty(), "nothing was sent");
    test_support::click(cx, "session-header-delete");
    assert!(drawn(cx, "prompt"), "Delete stays while suspended");
    test_support::click(cx, "prompt-cancel");

    test_support::click(cx, "session-header-environment");
    let (resume, _) = one(&mut fake, Method::Post, "/api/hosts/2/resume");
    assert!(!any_layer(cx), "Resume never asks");
    let in_flight = |cx: &mut gpui::VisualTestContext, key: OpKey| {
        cx.update(|_, cx| {
            ui_session::current_operations(cx)
                .and_then(|operations| operations.read(cx).in_flight(&key))
        })
    };
    assert_eq!(
        in_flight(cx, OpKey::session("a")),
        Some(Work::Resume),
        "the session's own request, as the old per-session claim"
    );
    assert_eq!(label_of(cx, control).as_deref(), Some("Resuming…"));
    assert!(
        crate::session_list::tests::painted(&frame(cx))
            .iter()
            .any(|text| text == "Resume"),
        "the label keeps its width"
    );
    let competing = |cx: &mut gpui::VisualTestContext, why: &str| {
        for action in ["session-header-delete", "session-header-clone"] {
            test_support::click(cx, action);
            assert!(!any_layer(cx), "{action} opens nothing {why}");
        }
    };
    competing(cx, "while the Resume is pending");
    fake.reply(
        resume,
        504,
        Some(json!(
            "resuming the host did not finish within 20 seconds; it carries on"
        )),
    );
    test_support::settle(cx);
    push_host(
        &mut fake,
        &environment_host(Provider::Sandbox, changing(LifecycleDirection::Resuming)),
        cx,
    );
    competing(cx, "while the provider is still resuming");
    push_host(
        &mut fake,
        &environment_host(Provider::Sandbox, connected()),
        cx,
    );
    assert_eq!(label_of(cx, control).as_deref(), Some("Suspend"));
    test_support::click(cx, "session-header-delete");
    assert!(drawn(cx, "prompt"), "Delete asks again");
}

/// A delayed activity reply must not hang a Suspend confirmation under the next session's Suspend control.
#[gpui::test]
fn a_suspend_question_waiting_on_activity_does_not_follow_the_selection(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Container, connected());
    let (mut fake, cx) = open(cx, &[(session("a"), 0), (session("b"), 0)], false, &host);
    pending(&mut fake);
    assert_eq!(selected(cx).as_deref(), Some("a"));
    test_support::click(cx, ENVIRONMENT_ID);
    let (read, _) = one(&mut fake, Method::Get, "/api/sessions?host=2");
    cx.update(|_, cx| crate::session_list::selection::open_automatically("b", cx));
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert_eq!(label_of(cx, ENVIRONMENT_ID).as_deref(), Some("Suspend"));

    fake.reply(
        read,
        200,
        Some(listing(&[session("a"), session("b")], false)),
    );
    test_support::settle(cx);
    assert!(!any_layer(cx), "the reply belonged to a's header");
    assert!(pending(&mut fake).is_empty(), "the reply sent no Suspend");
}

/// The header's Suspend is the row menu's: the same activity read and question, here under its own button. Specified:
/// with running a open on a connected container, the header's Suspend reads `GET /api/sessions?host=2`, whose answer
/// lists running a, so it asks "Suspend container?" with Cancel's help "Cancel: keep the container running"; the
/// answer sends `POST /api/hosts/2/suspend`; with the host pushed suspending, the control is named "Suspending…" and is
/// unavailable (a click reads nothing).
#[gpui::test]
fn the_headers_suspend_asks_after_its_read(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Container, connected());
    let (mut fake, cx) = open(cx, &[(session("a"), 0)], false, &host);
    pending(&mut fake);
    test_support::click(cx, "session-header-environment");
    let (read, _) = one(&mut fake, Method::Get, "/api/sessions?host=2");
    fake.reply(read, 200, Some(listing(&[session("a")], false)));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "prompt").as_deref(),
        Some("Suspend container?")
    );
    assert_eq!(
        hover_help(cx, "prompt-cancel").as_deref(),
        Some("Cancel: keep the container running")
    );
    test_support::click(cx, "prompt-confirm");
    let (suspend, _) = one(&mut fake, Method::Post, "/api/hosts/2/suspend");
    fake.reply(suspend, 200, Some(json!({})));
    test_support::settle(cx);

    push_host(
        &mut fake,
        &environment_host(
            Provider::Container,
            changing(LifecycleDirection::Suspending),
        ),
        cx,
    );
    assert_eq!(label_of(cx, ENVIRONMENT_ID).as_deref(), Some("Suspending…"));
    test_support::click(cx, "session-header-environment");
    assert!(
        pending(&mut fake).is_empty(),
        "a changing environment offers nothing"
    );
}

/// The old header withheld the whole Restart and Replace splits while the environment was frozen, and a caret menu
/// opened just before the environment froze must not open the launcher on it either. Specified: with the Replace caret's
/// menu opened on a connected sandbox session, the host is pushed suspended; the menu's Replace with… then opens no
/// launcher.
#[gpui::test]
fn a_caret_menu_opened_before_a_freeze_opens_no_launcher(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(idle("a"), 0)], false, &host);
    pending(&mut fake);
    test_support::click(cx, "session-header-replace-options");
    assert!(
        drawn(cx, "session-header-replace-options-menu"),
        "the premise: the menu is open"
    );
    push_host(
        &mut fake,
        &environment_host(Provider::Sandbox, suspended_at(0)),
        cx,
    );
    test_support::click(cx, "session-header-replace-options-with");
    assert!(
        !crate::session_list::tests::launcher_drawn(cx),
        "no launcher on a frozen environment"
    );
}

/// SPEC.md, "Sandbox sessions": a session whose environment is being deleted has its actions unavailable, Delete and
/// Clone included, and the old header's disabled Stop, Clone and Delete said why in their tooltips, while its details
/// bar showed the host's phase in the age's place. The Delete shortcut is the header's Delete, so it must refuse too, or
/// it would send the helm a Delete it can only refuse. Specified: with idle a open on a container whose deletion failed,
/// the hover help of Stop, Clone and Delete is "Deleting this session's container failed, so its actions are
/// unavailable. Retry in Hosts finishes the deletion"; the details bar draws "Deleting failed" with the same help and
/// no age; a click on Suspend, Stop, Restart, Replace, either caret, Clone or Delete, and the Delete shortcut, opens
/// nothing and sends nothing; once the host is connected, the age is back and the Delete shortcut and the header's
/// Delete each ask.
#[gpui::test]
fn every_header_action_is_refused_while_the_environment_is_being_deleted(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Container, deleting(Some("podman rm failed")));
    let (mut fake, cx) = open(cx, &[(idle("a"), 0)], false, &host);
    pending(&mut fake);
    let reason = "Deleting this session's container failed, so its actions are unavailable. Retry in Hosts \
                  finishes the deletion";
    for action in [
        "session-header-stop",
        "session-header-clone",
        "session-header-delete",
    ] {
        assert_eq!(
            hover_help(cx, action).as_deref(),
            Some(reason),
            "{action} says why"
        );
    }
    assert_eq!(
        label_of(cx, DELETING_ID).as_deref(),
        Some("Deleting failed")
    );
    assert_eq!(hover_help(cx, DELETING_ID).as_deref(), Some(reason));
    assert!(!drawn(cx, AGE_ID), "the phase takes the age's place");

    for control in [
        ENVIRONMENT_ID,
        "session-header-stop",
        "session-header-restart",
        "session-header-replace",
        "session-header-restart-options",
        "session-header-replace-options",
        "session-header-clone",
        "session-header-delete",
    ] {
        test_support::click(cx, control);
        assert!(!any_layer(cx), "{control} opens nothing");
        assert!(
            !crate::session_list::tests::launcher_drawn(cx),
            "{control} opens no launcher"
        );
    }
    test_support::keys(cx, DELETE_SESSION_CHORD);
    assert!(!any_layer(cx), "the Delete shortcut asks nothing");
    let sent: Vec<_> = fake
        .commands()
        .into_iter()
        .map(|command| (command.method, command.path))
        .collect();
    assert_eq!(sent, [], "nothing was sent");

    push_host(
        &mut fake,
        &environment_host(Provider::Container, connected()),
        cx,
    );
    assert!(drawn(cx, AGE_ID), "the age is back");
    test_support::keys(cx, DELETE_SESSION_CHORD);
    assert!(drawn(cx, "prompt"), "the Delete shortcut asks again");
    test_support::click(cx, "prompt-cancel");
    test_support::click(cx, "session-header-delete");
    assert!(drawn(cx, "prompt"), "Delete asks again");
}

// ---- The suspended card --------------------------------------------------------------------------------------------

/// A stale session can remain visible while its environment is being deleted. Its card must say why it is unavailable.
#[gpui::test]
fn a_cached_session_card_explains_deletion_in_progress(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Container, deleting(None));
    let (_fake, cx) = open(cx, &[(session("a").stale(), 0)], false, &host);
    assert_eq!(
        label_of(cx, EXPLANATION_ID).as_deref(),
        Some("This session's container is being deleted, so its actions are unavailable.")
    );
}

/// SPEC.md: "The unavailable card says **Session suspended**, explains preserved processes and snapshot-storage
/// billing, and offers Resume", and a failed resume shows on the card. Specified: with stale a open on a suspended
/// sandbox, the card's heading reads "Session suspended", its explanation the provider's, its diagnostics start
/// "Suspended outside Farhelm, noticed at"; its Resume sends `POST /api/hosts/2/resume`; the helm's 409 shows on the
/// card under the explanation and not again above the terminal.
#[gpui::test]
fn the_suspended_card_says_so_and_resumes(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, suspended_at(0));
    let shell = crate::session_list::environment_tests::bash_session("a").stale();
    let (mut fake, cx) = open(cx, &[(shell, 0)], false, &host);
    pending(&mut fake);
    assert_eq!(
        label_of(cx, HEADING_ID).as_deref(),
        Some("Session suspended")
    );
    assert_eq!(
        label_of(cx, EXPLANATION_ID).as_deref(),
        Some(
            "This session's sandbox is suspended. Its processes are preserved and continue when you resume it. \
             Only snapshot storage bills while suspended."
        )
    );
    assert!(
        label_of(cx, DETAIL_ID)
            .is_some_and(|detail| detail.starts_with("Suspended outside Farhelm, noticed at ")),
        "{:?}",
        label_of(cx, DETAIL_ID)
    );
    assert_eq!(label_of(cx, RESUME_ID).as_deref(), Some("Resume"));
    test_support::click(cx, RESUME_ID);
    let (resume, _) = one(&mut fake, Method::Post, "/api/hosts/2/resume");
    fake.reply(resume, 409, Some(json!("Tensorlake refused the resume")));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, RESUME_FAILURE_ID).as_deref(),
        Some("Tensorlake refused the resume")
    );
    assert!(
        !drawn(cx, "session-header-outcome-a"),
        "shown once, on the card"
    );
}

/// The card's words per host (the old `unavailable_notice` and `stale_session_notice`'s `resume_host`): only a
/// session's own suspended environment is "Session suspended", a stopped container is never called suspended, and an
/// ordinary host's card offers Resume host. Specified: an ordinary suspended host keeps "Session unavailable — host
/// suspended" and offers a Resume of host 2 with no provider; a stopped container reads "Session unavailable —
/// environment stopped" and offers its own Resume; one that is starting again reads "Session unavailable — starting
/// environment" with "Starting the container again; sessions that were running come back ended, or interrupted if its
/// host restarted, with Restart where a conversation was captured." (SPEC.md's two cases, not the old card's "return as
/// interrupted") and offers none; a suspended sandbox's Tensorlake failure leads its diagnostics.
#[test]
fn the_cards_words_and_resume_follow_the_host() {
    let words = |host| {
        let notice: Notice = notice(
            "buildbox",
            HostLookup::Known(&host),
            None,
            |_| None,
            Zone::UTC,
        );
        (
            notice.heading(),
            notice.explanation.accessible_name().to_string(),
            notice
                .detail
                .map(|detail| detail.accessible_name().to_string()),
            notice
                .resume
                .map(|offer| (offer.host, offer.environment.is_some())),
        )
    };
    let ordinary = crate::hosts::tests::host(2, "ssh", suspended_at(0));
    let (heading, _, _, resume) = words(ordinary);
    assert_eq!(heading, "Session unavailable — host suspended");
    assert_eq!(resume, Some((2, false)), "Resume host");

    let stopped = HostStateView::Stopped {
        start_failed: None,
        waiting_for_place: false,
    };
    let (heading, _, _, resume) = words(environment_host(Provider::Container, stopped));
    assert_eq!(heading, "Session unavailable — environment stopped");
    assert_eq!(resume, Some((2, true)));

    let starting = HostStateView::TensorlakeChanging {
        starting: true,
        direction: LifecycleDirection::Resuming,
        requested_at: 0,
    };
    let (heading, explanation, _, resume) = words(environment_host(Provider::Container, starting));
    assert_eq!(heading, "Session unavailable — starting environment");
    assert_eq!(
        explanation,
        "Starting the container again; sessions that were running come back ended, or interrupted if its host \
         restarted, with Restart where a conversation was captured."
    );
    assert_eq!(resume, None);

    let failed = HostStateView::Suspended {
        observed_at: 0,
        suspended_by_farhelm_at: None,
        resume_failed: Some("quota".into()),
    };
    let (heading, _, detail, _) = words(environment_host(Provider::Sandbox, failed));
    assert_eq!(heading, "Session suspended");
    assert!(
        detail.as_deref().is_some_and(
            |detail| detail.starts_with("Tensorlake could not resume the sandbox: quota. ")
        ),
        "{detail:?}"
    );
}

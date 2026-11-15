//! Tests of answering an approval card: the command, the 700 ms pause, the first answer winning, a refusal kept on the
//! card and a late answer's notice, in process against the helm client's fake.
//!
//! They matter because the answer is the one moment an agent's request turns into action: an answer sent while the
//! list was changing could approve the request that slid under the pointer, a second answer could act twice, and a
//! refusal or a late answer reported wrongly would leave the user believing a request was approved or a host setting
//! untouched.

use std::time::Duration;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::approvals::{ApprovalAction, ApprovalAnswer, ApprovalSession, PendingApproval};
use farhelm_proto::ui_channel::{Collection, CollectionChanges, CollectionState, KeyedEntry};
use gpui::{AppContext as _, Entity, TestAppContext};
use serde_json::{Value, json};

use super::answers::{self, ARM_DELAY, Arming, Sent};
use crate::host_text::HostText;
use crate::notices::{self, Notice};
use crate::operations::{NotStarted, OpKey, Work};
use crate::test_support::{meta, started};

/// A waiting stop request `id` from host "box".
fn stop(id: &str) -> PendingApproval {
    PendingApproval {
        id: id.to_owned(),
        host_id: 1,
        host_name: "box".to_owned(),
        session: ApprovalSession {
            id: "asker".to_owned(),
            title: None,
            host_name: None,
        },
        action: ApprovalAction::Stop {
            target: ApprovalSession {
                id: "target".to_owned(),
                title: None,
                host_name: None,
            },
        },
        own_host_session_action: false,
        expires_at_ms: 1,
    }
}

/// The approvals collection holding `approvals`, oldest first, as the helm snapshots it.
fn waiting(approvals: &[PendingApproval]) -> CollectionState {
    let ids: Vec<&str> = approvals
        .iter()
        .map(|approval| approval.id.as_str())
        .collect();
    CollectionState {
        collection: Collection::Approvals,
        meta: meta(&ids),
        entries: approvals
            .iter()
            .map(|approval| KeyedEntry {
                key: approval.id.clone(),
                value: serde_json::to_value(approval).expect("an approval encodes"),
            })
            .collect(),
    }
}

/// The app connected to the fake helm with `approvals` waiting, and an armed answer bar.
fn connected(cx: &mut TestAppContext, approvals: &[PendingApproval]) -> (FakeHelm, Entity<Arming>) {
    let (mut fake, _connection) = started(cx);
    fake.connect(vec![waiting(approvals)]);
    cx.run_until_parked();
    let arming = cx.new(|_| Arming::default());
    let ids = approvals
        .iter()
        .map(|approval| approval.id.clone())
        .collect();
    let expanded = approvals.first().map(|approval| approval.id.clone());
    arming.update(cx, |arming, cx| arming.observe(ids, expanded, cx));
    cx.executor().advance_clock(ARM_DELAY);
    cx.run_until_parked();
    assert!(
        arming.read_with(cx, |arming, _| arming.armed()),
        "premise: the bar is armed"
    );
    (fake, arming)
}

/// Press `answer` on `approval`.
fn press(
    cx: &mut TestAppContext,
    arming: &Entity<Arming>,
    approval: &PendingApproval,
    answer: ApprovalAnswer,
) -> Result<Sent, NotStarted> {
    cx.update(|cx| answers::send(&approval.id, answer, None, arming, cx))
}

/// The commands the window sent since the last call, as (id, path, body).
fn sent(fake: &mut FakeHelm) -> Vec<(u64, String, Value)> {
    fake.commands()
        .into_iter()
        .map(|command| (command.id, command.path, command.body.unwrap_or_default()))
        .collect()
}

/// The notices showing now.
fn posted(cx: &mut TestAppContext) -> Vec<Notice> {
    cx.update(|cx| notices::posted(cx))
}

/// The refusal kept on the request's key, for its card to show.
fn failure(cx: &mut TestAppContext, id: &str) -> Option<HostText> {
    cx.update(|cx| {
        let operations = crate::session::current_operations(cx)?;
        let failure = operations.read(cx).failure(&OpKey::approval(id))?;
        Some(failure.message.clone())
    })
}

/// An answer is `POST /api/approvals/{id}`, under the request's own approval id with every path character the id
/// holds encoded. Why: the helm binds an answer to the id it names, and an id must never retarget the request.
#[test]
fn an_answer_is_a_post_to_its_request_with_the_answer_word() {
    let command =
        answers::command("a b/c", ApprovalAnswer::AlwaysAllowOwnHost, None).expect("a valid id");
    assert_eq!(command.work(), Work::Answer);
    assert_eq!(command.path().as_str(), "/api/approvals/a%20b%2Fc");
}

/// A host add's Allow carries the destination the add dialog holds then, in the shape the helm requires on that answer
/// alone.
#[gpui::test]
fn a_host_add_allow_carries_the_dialogs_destination(cx: &mut TestAppContext) {
    let (mut fake, arming) = connected(cx, &[stop("r")]);
    let pressed = cx.update(|cx| {
        answers::send(
            "r",
            ApprovalAnswer::Allow,
            Some("ops@new.example".into()),
            &arming,
            cx,
        )
    });
    assert_eq!(pressed, Ok(Sent::Sent));
    cx.run_until_parked();
    assert_eq!(
        sent(&mut fake)
            .into_iter()
            .map(|(_, path, body)| (path, body))
            .collect::<Vec<_>>(),
        [(
            "/api/approvals/r".to_owned(),
            json!({ "answer": "allow", "host_add": { "destination": "ops@new.example" } })
        )]
    );
}

/// SPEC.md: answer buttons pause for 700 ms whenever the waiting list or the expanded request changes, so a click
/// already aimed at the previous request cannot answer the next one. The newest change's pause decides: an earlier
/// pause running out must not arm a card that changed since, and showing the same thing again starts no pause.
#[gpui::test]
fn the_answer_bar_pauses_after_every_change_and_only_the_newest_pause_arms_it(
    cx: &mut TestAppContext,
) {
    let (_fake, arming) = connected(cx, &[stop("a"), stop("b")]);
    let armed = |cx: &mut TestAppContext| arming.read_with(cx, |arming, _| arming.armed());

    arming.update(cx, |arming, cx| {
        arming.observe(vec!["a".into(), "b".into()], Some("a".into()), cx)
    });
    assert!(
        armed(cx),
        "showing the same list and card again starts no pause"
    );

    arming.update(cx, |arming, cx| {
        arming.observe(vec!["a".into(), "b".into()], Some("b".into()), cx)
    });
    assert!(!armed(cx), "a different expanded request disarms at once");
    cx.executor().advance_clock(Duration::from_millis(400));
    cx.run_until_parked();
    arming.update(cx, |arming, cx| {
        arming.observe(vec!["b".into()], Some("b".into()), cx)
    });
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    assert!(
        !armed(cx),
        "the first pause ran out, but the list changed since and its own pause has not"
    );
    cx.executor().advance_clock(Duration::from_millis(399));
    cx.run_until_parked();
    assert!(!armed(cx), "still inside the newest pause");
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert!(armed(cx), "700 ms after the newest change");
}

/// A press while the bar is paused sends nothing, checked when the press arrives rather than when the buttons were
/// drawn: a click already on its way when the list changed must not reach the next request.
#[gpui::test]
fn a_press_during_the_pause_sends_nothing(cx: &mut TestAppContext) {
    let (mut fake, arming) = connected(cx, &[stop("a")]);
    arming.update(cx, |arming, cx| {
        arming.observe(vec!["a".into(), "b".into()], Some("a".into()), cx)
    });
    assert_eq!(
        press(cx, &arming, &stop("a"), ApprovalAnswer::Allow),
        Ok(Sent::Paused)
    );
    cx.run_until_parked();
    assert_eq!(sent(&mut fake), []);
}

/// The first answer pressed wins: while one answer to a request is in flight, another is refused without being sent,
/// so a double click or a second button cannot answer twice.
#[gpui::test]
fn the_first_answer_pressed_wins(cx: &mut TestAppContext) {
    let (mut fake, arming) = connected(cx, &[stop("a")]);
    assert_eq!(
        press(cx, &arming, &stop("a"), ApprovalAnswer::Allow),
        Ok(Sent::Sent)
    );
    assert_eq!(
        press(cx, &arming, &stop("a"), ApprovalAnswer::Deny),
        Err(NotStarted::Busy(Work::Answer))
    );
    cx.run_until_parked();
    let commands = sent(&mut fake);
    assert_eq!(
        commands
            .iter()
            .map(|(_, path, body)| (path.as_str(), body.clone()))
            .collect::<Vec<_>>(),
        [("/api/approvals/a", json!({ "answer": "allow" }))]
    );
}

/// An answer the helm takes leaves nothing behind: no refusal on the card and no notice; the helm's removal of the
/// request, pushed before its reply, is what takes the card away.
#[gpui::test]
fn an_accepted_answer_leaves_no_refusal_or_notice(cx: &mut TestAppContext) {
    let (mut fake, arming) = connected(cx, &[stop("a")]);
    press(cx, &arming, &stop("a"), ApprovalAnswer::Deny).expect("sent");
    cx.run_until_parked();
    let (id, _, body) = sent(&mut fake).remove(0);
    assert_eq!(body, json!({ "answer": "deny" }));
    fake.push(CollectionChanges {
        collection: Collection::Approvals,
        meta: meta(&[]),
        upsert: vec![],
        remove: vec!["a".into()],
    });
    fake.reply(id, 204, None);
    cx.run_until_parked();
    assert_eq!((failure(cx, "a"), posted(cx)), (None, vec![]));
}

/// SPEC.md: an Always allow whose setting cannot be turned on approves nothing and leaves the card up with the
/// reason; an answer refused on a card that still waits keeps the request answerable. The refusal stays on the
/// request's key for the card to show, and no late notice claims the request had gone.
#[gpui::test]
fn a_refusal_of_a_waiting_request_stays_on_its_card(cx: &mut TestAppContext) {
    let (mut fake, arming) = connected(cx, &[stop("a")]);
    press(cx, &arming, &stop("a"), ApprovalAnswer::AlwaysAllow).expect("sent");
    cx.run_until_parked();
    let (id, _, body) = sent(&mut fake).remove(0);
    assert_eq!(body, json!({ "answer": "always_allow" }));
    fake.reply(
        id,
        409,
        Some(json!("the requesting host now reports a different install")),
    );
    cx.run_until_parked();
    assert_eq!(
        failure(cx, "a"),
        Some(HostText::from_host(
            "the requesting host now reports a different install"
        ))
    );
    assert_eq!(posted(cx), vec![]);
    assert_eq!(
        press(cx, &arming, &stop("a"), ApprovalAnswer::Allow),
        Ok(Sent::Sent),
        "the request is still answerable"
    );
    cx.run_until_parked();
    let resent = sent(&mut fake);
    assert_eq!(
        resent
            .iter()
            .map(|(_, path, body)| (path.as_str(), body.clone()))
            .collect::<Vec<_>>(),
        [("/api/approvals/a", json!({ "answer": "allow" }))],
        "the second answer reached the helm"
    );
}

/// Only the helm's 410 is a late answer. A refusal that arrives as the request goes away at about the same time (an
/// Always allow refused because the asking host's connection ended, whose removal the helm pushed before replying)
/// wrote no host setting, so it must not post the late notice, whose Always-allow form says the setting may have been
/// turned on; the refusal stays on the request's key instead.
#[gpui::test]
fn a_refusal_racing_the_requests_removal_is_not_a_late_answer(cx: &mut TestAppContext) {
    let (mut fake, arming) = connected(cx, &[stop("a")]);
    press(cx, &arming, &stop("a"), ApprovalAnswer::AlwaysAllow).expect("sent");
    cx.run_until_parked();
    let (id, _, _) = sent(&mut fake).remove(0);
    fake.push(CollectionChanges {
        collection: Collection::Approvals,
        meta: meta(&[]),
        upsert: vec![],
        remove: vec!["a".into()],
    });
    fake.reply(
        id,
        409,
        Some(json!("the requesting host's connection is gone")),
    );
    cx.run_until_parked();
    assert_eq!(posted(cx), vec![]);
    assert_eq!(
        failure(cx, "a"),
        Some(HostText::from_host(
            "the requesting host's connection is gone"
        ))
    );
}

/// SPEC_impl.md, "Permission prompts for agent actions": an answer that arrives after its request stopped waiting
/// (answered elsewhere, expired, its session deleted) did not take effect, and the user is told so; after an Always
/// allow, the notice adds that the host's setting may still have been turned on, since the helm stores it before it
/// looks for the request. One late notice shows at a time, in the form of the latest late answer.
#[gpui::test]
fn a_late_answer_posts_its_notice_in_the_answers_form(cx: &mut TestAppContext) {
    let (mut fake, arming) = connected(cx, &[stop("a"), stop("b")]);
    // The request expires on the helm and its removal arrives before the answer's 410.
    press(cx, &arming, &stop("a"), ApprovalAnswer::Allow).expect("sent");
    cx.run_until_parked();
    let (id, _, _) = sent(&mut fake).remove(0);
    fake.push(CollectionChanges {
        collection: Collection::Approvals,
        meta: meta(&["b"]),
        upsert: vec![],
        remove: vec!["a".into()],
    });
    fake.reply(
        id,
        410,
        Some(json!("approval request a is no longer waiting")),
    );
    cx.run_until_parked();
    assert_eq!(posted(cx), vec![Notice::LateAnswer { always: false }]);

    press(cx, &arming, &stop("b"), ApprovalAnswer::AlwaysAllow).expect("sent");
    cx.run_until_parked();
    let (id, _, _) = sent(&mut fake).remove(0);
    fake.push(CollectionChanges {
        collection: Collection::Approvals,
        meta: meta(&[]),
        upsert: vec![],
        remove: vec!["b".into()],
    });
    fake.reply(
        id,
        410,
        Some(json!("approval request b is no longer waiting")),
    );
    cx.run_until_parked();
    assert_eq!(
        posted(cx),
        vec![Notice::LateAnswer { always: true }],
        "the newer late answer's form replaces the older"
    );
    assert!(
        Notice::LateAnswer { always: true }
            .text()
            .contains("may still have been turned on")
    );
    assert!(
        !Notice::LateAnswer { always: false }
            .text()
            .contains("turned on")
    );
}

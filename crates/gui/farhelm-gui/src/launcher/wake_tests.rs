//! Deterministic window tests of an existing host's wake before launch. The fake's Resume reply and host pushes
//! can advance independently, and all budget and retry checks advance the GPUI clock.

use std::time::Duration;

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::tensorlake::LifecycleDirection;
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, HostStateView, HostView, KeyedEntry,
};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::environment_launch_tests::answer_except;
use super::launch_tests::{command_draft, creates};
use super::view_tests::{fixture_hosts, launcher_with_hosts, shown_refusal};
use crate::session_list::environment_tests::{changing, connected, suspended_at};
use crate::test_support::{advance, click, label, meta, settle};

const RESUME: &str = "/api/hosts/12/resume";

/// One real launcher on the pinned sandbox suspension, whose home is available before history can be read.
fn open(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext, HostView) {
    let hosts = fixture_hosts(&[0, 11]);
    let host: HostView = serde_json::from_value(hosts.entries[1].value.clone()).unwrap();
    assert_eq!(host.id, 12);
    assert!(matches!(host.state, HostStateView::Suspended { .. }));
    open_on(cx, host)
}

/// Use the same install and pending home folder across lifecycle phases, with no stored folder to take.
fn open_on(
    cx: &mut TestAppContext,
    host: HostView,
) -> (FakeHelm, &mut VisualTestContext, HostView) {
    let mut hosts = fixture_hosts(&[0, 11]);
    hosts.entries[1].value = serde_json::to_value(&host).unwrap();
    let mut draft = command_draft();
    draft.choose_host(host.id, "~".into());
    let (helm, cx) = launcher_with_hosts(cx, draft, hosts);
    (helm, cx, host)
}

/// The launch must send Resume alone, hide the form, and keep the destination held while it waits.
fn press(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> FakeCommand {
    click(cx, "launcher-launch");
    let sent = wake_commands(helm, cx);
    let [resume] = &sent[..] else {
        panic!("one Resume expected, got {sent:?}")
    };
    assert_eq!(resume.path, RESUME);
    assert!(label(cx, "launcher-launch").is_none());
    assert!(label(cx, "launch-wake").is_some());
    assert!(cx.update(|_, cx| crate::hosts::launch_in_flight(cx)));
    assert!(cx.update(|_, cx| super::launch::host_lifecycle_refusal(12, cx).is_some()));
    resume.clone()
}

/// Preserve the order of any lifecycle or create commands the test will answer explicitly.
fn wake_commands(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    answer_except(helm, cx, |command| {
        command.path == RESUME || command.path == "/api/sessions"
    })
}

/// Push the actual host row, preserving the installation and replacing its connection only when it reconnects.
pub(crate) fn push(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    host: &mut HostView,
    state: HostStateView,
) {
    if matches!(state, HostStateView::Connected { .. }) {
        host.incarnation += 1;
    }
    host.state = state;
    helm.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&["1", "12"]),
        upsert: vec![KeyedEntry {
            key: HostView::key(host.id),
            value: serde_json::to_value(&*host).unwrap(),
        }],
        remove: Vec::new(),
    });
    settle(cx);
}

/// A visible New press on the home folder uses that folder on the fresh connection. The accepted Resume can
/// arrive first, so the form stays hidden and the capsule describes the wake until a create is possible.
#[gpui::test]
fn a_wake_uses_the_fresh_connection_and_the_unsuffixed_host_name(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = open(cx);
    assert!(super::view_tests::draft(cx).folder_default_waiting());
    assert!(label(cx, "launcher-launch").unwrap().contains('~'));
    let resume = press(&mut helm, cx);
    assert!(label(cx, "launch-wake").unwrap().starts_with("Resuming "));
    helm.reply(resume.id, 200, None);
    settle(cx);
    assert!(wake_commands(&mut helm, cx).is_empty());
    assert!(label(cx, "launcher-launch").is_none());
    assert!(label(cx, "launch-wake").is_some());

    push(&mut helm, cx, &mut host, connected());
    // A history reply must never get to choose a different folder for this press.
    let sent = answer_except(&mut helm, cx, |command| {
        command.path == "/api/sessions" || command.path == "/api/launch-history?host=12"
    });
    let [create] = &sent[..] else {
        panic!("one create expected, got {sent:?}")
    };
    assert_eq!(create.path, "/api/sessions");
    let body = create.body.as_ref().unwrap();
    assert_eq!(body["host"], host.id);
    assert_eq!(body["expected_incarnation"], host.incarnation);
    assert_eq!(body["cwd"], "~");
    advance(cx, Duration::from_secs(2));
    let words = label(cx, "launch-progress").expect("the slow-create pill");
    assert!(words.contains("host12"), "{words}");
    assert!(!words.contains("Suspended"), "{words}");
    assert!(label(cx, "launch-wake").is_none());
    helm.reply(create.id, 409, Some(json!("The test refused the create")));
    settle(cx);
}

/// The bounded reply can precede Resuming on the channel. The preparation stays visible through both phases, and
/// the provider's prolonged Resume does not spend the sixty seconds allowed for reconnection.
#[gpui::test]
fn a_504_before_the_provider_phase_keeps_the_launch_preparing(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = open(cx);
    let resume = press(&mut helm, cx);
    helm.reply(resume.id, 504, Some(json!("The resume carries on")));
    settle(cx);
    assert!(wake_commands(&mut helm, cx).is_empty());
    assert!(label(cx, "launch-wake").is_some());
    push(
        &mut helm,
        cx,
        &mut host,
        changing(LifecycleDirection::Resuming),
    );
    advance(cx, Duration::from_secs(75));
    assert!(wake_commands(&mut helm, cx).is_empty());
    assert!(label(cx, "launch-wake").is_some());
    push(&mut helm, cx, &mut host, connected());
    let sent = creates(&mut helm, cx);
    assert_eq!(
        sent.len(),
        1,
        "the accepted Resume's phase preserved the budget"
    );
    helm.reply(sent[0].id, 409, Some(json!("The test refused the create")));
    settle(cx);
}

/// A suspension stored before a busy Resume is stale. Retry each second while it still shows. Re-suspension
/// with a new observation must abandon the original launch before its minute runs out.
#[gpui::test]
fn a_busy_wake_retries_then_fails_on_a_new_suspension(cx: &mut TestAppContext) {
    use farhelm_proto::ui_channel::HelmMarker;

    let (mut helm, cx, mut host) = open(cx);
    let resume = press(&mut helm, cx);
    let initial = host.state.clone();
    helm.reply_marked(
        resume.id,
        409,
        vec![HelmMarker::HostBusy],
        Some(json!("The host is busy")),
    );
    settle(cx);
    push(&mut helm, cx, &mut host, initial);
    assert!(wake_commands(&mut helm, cx).is_empty());
    advance(cx, Duration::from_millis(999));
    assert!(wake_commands(&mut helm, cx).is_empty(), "no early retry");
    advance(cx, Duration::from_millis(1));
    let retry = wake_commands(&mut helm, cx);
    assert_eq!(retry.len(), 1);
    assert_eq!(retry[0].path, RESUME);
    helm.reply(retry[0].id, 200, None);
    settle(cx);
    let HostStateView::Suspended { observed_at, .. } = host.state else {
        panic!("starting suspension")
    };
    push(&mut helm, cx, &mut host, suspended_at(observed_at + 1));
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some(
            "Not launched. The host was suspended again while it was waking; resume it and launch again"
        ),
    );
    assert!(wake_commands(&mut helm, cx).is_empty());
    assert!(label(cx, "launch-wake").is_none());
}

/// An ordinary Resume refusal is preparation failure. Matching a status or busy-looking prose alone would retry
/// the wrong rejection and could subsequently launch an unwanted session.
#[gpui::test]
fn a_wake_refusal_reveals_the_draft_without_a_create(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = open(cx);
    let resume = press(&mut helm, cx);
    // The channel can publish a changed row before sending the explanation in the reply.
    push(&mut helm, cx, &mut host, suspended_at(0));
    assert!(label(cx, "launch-wake").is_some());
    helm.reply(resume.id, 409, Some(json!("The host is busy")));
    settle(cx);
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. Resuming the host failed: The host is busy"),
    );
    advance(cx, Duration::from_secs(1));
    assert!(wake_commands(&mut helm, cx).is_empty());
    assert!(label(cx, "launch-wake").is_none());
    assert!(cx.update(|_, cx| !crate::hosts::launch_in_flight(cx)));
}

/// Resume can finish after its reply bound, with Connecting and new suspension timestamps already published
/// by the time the provider diagnosis reaches the window.
#[gpui::test]
fn a_late_resume_failure_keeps_the_provider_diagnosis(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = open(cx);
    let resume = press(&mut helm, cx);
    helm.reply(resume.id, 504, Some(json!("The resume carries on")));
    settle(cx);
    push(
        &mut helm,
        cx,
        &mut host,
        changing(LifecycleDirection::Resuming),
    );
    push(
        &mut helm,
        cx,
        &mut host,
        crate::hosts::tests::connecting(1, None),
    );
    assert!(label(cx, "launch-wake").is_some());
    push(
        &mut helm,
        cx,
        &mut host,
        HostStateView::Suspended {
            observed_at: 900,
            suspended_by_farhelm_at: None,
            resume_failed: Some("The provider rejected Resume".into()),
        },
    );
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. Tensorlake could not resume this host; check its details"),
    );
    assert!(wake_commands(&mut helm, cx).is_empty());
    assert!(label(cx, "launch-wake").is_none());
}

/// A stopped container can start on the same one press. Its pill names the action the provider is performing
/// and the create still waits for a fresh connection.
#[gpui::test]
fn a_stopped_container_starts_before_creating(cx: &mut TestAppContext) {
    let mut host: HostView =
        serde_json::from_value(fixture_hosts(&[11]).entries[0].value.clone()).unwrap();
    host.kind = "podman".into();
    host.state = HostStateView::Stopped {
        waiting_for_place: false,
        start_failed: None,
    };
    let (mut helm, cx, mut host) = open_on(cx, host);
    let resume = press(&mut helm, cx);
    assert!(label(cx, "launch-wake").unwrap().starts_with("Starting "));
    helm.reply(resume.id, 200, None);
    settle(cx);
    assert!(wake_commands(&mut helm, cx).is_empty());
    push(&mut helm, cx, &mut host, connected());
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one create expected, got {sent:?}")
    };
    assert_eq!(
        create.body.as_ref().unwrap()["expected_incarnation"],
        host.incarnation
    );
    helm.reply(create.id, 409, Some(json!("The test refused the create")));
    settle(cx);
}

/// Another surface can hold the host lifecycle lock in either direction. Seeing that phase before the press
/// leaves the form unavailable, so it never starts a competing Resume.
#[gpui::test]
fn a_host_already_changing_refuses_launch(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = open(cx);
    for direction in [LifecycleDirection::Suspending, LifecycleDirection::Resuming] {
        push(&mut helm, cx, &mut host, changing(direction));
        assert_eq!(
            crate::session_list::host_rail::tests::hover_help(cx, "launcher-launch").as_deref(),
            Some("Choose a connected host before launching"),
        );
        click(cx, "launcher-launch");
        assert!(wake_commands(&mut helm, cx).is_empty());
        assert!(label(cx, "launch-wake").is_none());
    }
}

/// Replace with promises to retire the source only after its own create. A source action during Resume could
/// invalidate that consent, so it is held through preparation and released before a refused draft comes back.
#[gpui::test]
fn a_replacement_holds_its_source_through_a_wake_refusal(cx: &mut TestAppContext) {
    use crate::session_list::environment_tests::{
        Provider, bash_session, environment_host, open as open_list,
    };

    let host = environment_host(Provider::Sandbox, suspended_at(100));
    let (mut helm, cx) = open_list(cx, &[(bash_session("a"), 0)], false, &host);
    answer_except(&mut helm, cx, |_| false);
    click(cx, "session-menu-a");
    click(cx, "session-menu-replace-with");
    assert!(
        label(cx, "launcher-launch").is_some(),
        "the row opens Replace with"
    );
    assert!(cx.update(|_, cx| !crate::session_list::menu::refused("a", cx)));
    click(cx, "launcher-launch");
    let sent = answer_except(&mut helm, cx, |command| {
        command.path == "/api/hosts/2/resume" || command.path == "/api/sessions/a/replace"
    });
    let [resume] = &sent[..] else {
        panic!("only Resume expected, got {sent:?}")
    };
    assert_eq!(resume.path, "/api/hosts/2/resume");
    assert!(cx.update(|_, cx| crate::session_list::menu::refused("a", cx)));
    helm.reply(resume.id, 409, Some(json!("Resume failed")));
    settle(cx);
    assert!(cx.update(|_, cx| !crate::session_list::menu::refused("a", cx)));
    assert!(shown_refusal(cx).is_some());
}

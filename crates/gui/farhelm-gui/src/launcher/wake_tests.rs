//! Deterministic window tests of an existing host's wake before launch. The fake's Resume reply and host pushes
//! can advance independently, and all budget and retry checks advance the GPUI clock.

use std::time::Duration;

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::tensorlake::LifecycleDirection;
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, HostStateView, HostView, KeyedEntry, WakeOutcome, WakeResult,
};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::draft::Mode;
use super::environment_launch_tests::answer_except;
use super::launch_tests::{command_draft, creates};
use super::view_tests::{fixture_hosts, launcher_with_hosts, shown_refusal};
use super::yolo_tests::{refuse_for_yolo, yolo_command_draft};
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

/// Send an ordinary YOLO request to a connected environment. The folder is
/// already chosen so the initial press tests dispatch, independent of history.
fn yolo_dispatched(
    cx: &mut TestAppContext,
    mode: Mode,
    verified: bool,
) -> (FakeHelm, &mut VisualTestContext, HostView, FakeCommand) {
    let mut hosts = fixture_hosts(&[0, 11]);
    let mut host: HostView = serde_json::from_value(hosts.entries[1].value.clone()).unwrap();
    if !verified {
        host.identity = None;
    }
    assert_eq!(host.identity.is_some(), verified);
    host.state = connected_as(&host);
    hosts.entries[1].value = serde_json::to_value(&host).unwrap();
    let mut draft = yolo_command_draft();
    draft.mode = mode;
    draft.choose_host(host.id, "~".into());
    draft.edit_folder("/work".into());
    let (mut helm, cx) = launcher_with_hosts(cx, draft, hosts);
    assert_published_host(cx, &host);
    let first = press_yolo_create(&mut helm, cx, &host);
    (helm, cx, host, first)
}

/// Keep the registry identity and the connection identity consistent, including
/// the unverified fixture whose connection has no identity yet.
fn connected_as(host: &HostView) -> HostStateView {
    let mut state = connected();
    if let HostStateView::Connected { identity, .. } = &mut state {
        identity.clone_from(&host.identity);
    }
    state
}

/// Witness the exact host phase and installation in the GUI's store before
/// relying on a suspended, reconnected or retargeted fixture.
pub(super) fn assert_published_host(cx: &mut VisualTestContext, expected: &HostView) {
    cx.update(|_, cx| {
        let store = crate::session::current_store(cx).unwrap();
        let host = store
            .read(cx)
            .collections()
            .get::<HostView>(&HostView::key(expected.id))
            .unwrap();
        assert_eq!(host.identity, expected.identity);
        assert_eq!(host.incarnation, expected.incarnation);
        assert_eq!(host.state, expected.state);
    });
}

/// A new press requires its own permission even in a launcher that recovered
/// from an earlier question or a refused create.
fn press_yolo_create(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    host: &HostView,
) -> FakeCommand {
    click(cx, "launcher-launch");
    let sent = wake_commands(helm, cx);
    let [create] = &sent[..] else {
        panic!("one create expected, got {sent:?}")
    };
    assert_eq!(create.path, "/api/sessions");
    let body = create.body.as_ref().unwrap();
    assert_eq!(body["host"], host.id);
    assert_eq!(body["expected_incarnation"], host.incarnation);
    assert_eq!(body["cwd"], "/work");
    assert_eq!(body["command"]["yolo"], true);
    assert!(
        body.get("confirm_yolo")
            .is_none_or(|value| value == &json!(false))
    );
    create.clone()
}

/// Drive a one-off answer with a suspended target. Only a resumed connection
/// to the same verified installation may see the confirmation bit.
fn answer_yolo_during_suspension(cx: &mut TestAppContext, mode: Mode) {
    let (mut helm, cx, mut host, first) = yolo_dispatched(cx, mode, true);
    refuse_for_yolo(&mut helm, cx, first.id);
    assert!(label(cx, "prompt-confirm").is_some());
    push(&mut helm, cx, &mut host, suspended_at(100));
    assert_published_host(cx, &host);
    assert!(matches!(host.state, HostStateView::Suspended { .. }));
    assert!(label(cx, "prompt-confirm").is_some());
    assert!(wake_commands(&mut helm, cx).is_empty());
    click(cx, "prompt-confirm");
    let sent = wake_commands(&mut helm, cx);
    let [resume] = &sent[..] else {
        panic!("one Resume expected, got {sent:?}")
    };
    assert_eq!(resume.path, RESUME);
    helm.reply(resume.id, 200, None);
    settle(cx);
    assert!(wake_commands(&mut helm, cx).is_empty());
    assert!(label(cx, "launch-wake").is_some());
    let connection = connected_as(&host);
    push(&mut helm, cx, &mut host, connection);
    assert_published_host(cx, &host);
    let sent = wake_commands(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one resumed create expected, got {sent:?}")
    };
    assert_eq!(create.path, "/api/sessions");
    let body = create.body.as_ref().unwrap();
    assert_eq!(body["host"], host.id);
    assert_eq!(body["expected_incarnation"], host.incarnation);
    assert_eq!(body["cwd"], "/work");
    assert_eq!(body["command"], first.body.as_ref().unwrap()["command"]);
    assert_eq!(body["confirm_yolo"], true);

    helm.reply(
        create.id,
        409,
        Some(json!("Fixture refused the confirmed launch")),
    );
    settle(cx);
    assert!(label(cx, "prompt-confirm").is_none());
    let fresh = press_yolo_create(&mut helm, cx, &host);
    refuse_for_yolo(&mut helm, cx, fresh.id);
    assert!(label(cx, "prompt-confirm").is_some());
}

/// New keeps one confirmed dispatch through wake and releases it after the reply.
#[gpui::test]
fn new_keeps_one_yolo_confirmation_through_wake(cx: &mut TestAppContext) {
    answer_yolo_during_suspension(cx, Mode::New);
}

/// Clone uses the same destination pipeline. Its first press also has no permission
/// to bypass the destination host's confirmation.
#[gpui::test]
fn clone_keeps_one_yolo_confirmation_through_wake(cx: &mut TestAppContext) {
    answer_yolo_during_suspension(cx, Mode::Clone);
}

/// The refusal names the previous installation even if its registry row kept
/// the same ID. It cannot ask for confirmation on behalf of a replacement.
#[gpui::test]
fn a_yolo_refusal_from_an_earlier_installation_cannot_ask(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host, first) = yolo_dispatched(cx, Mode::New, true);
    host.identity = Some("replacement-installation".into());
    let connection = connected_as(&host);
    push(&mut helm, cx, &mut host, connection);
    assert_published_host(cx, &host);
    refuse_for_yolo(&mut helm, cx, first.id);
    assert!(label(cx, "prompt-confirm").is_none());
    assert!(
        shown_refusal(cx)
            .unwrap()
            .contains("original host installation")
    );
    assert!(wake_commands(&mut helm, cx).is_empty());
    let fresh = press_yolo_create(&mut helm, cx, &host);
    refuse_for_yolo(&mut helm, cx, fresh.id);
    assert!(label(cx, "prompt-confirm").is_some());
}

/// An older supervisor can ask on its existing connection. The retry keeps that connection's token and the
/// original folder and applies permission once.
#[gpui::test]
fn an_unverified_installation_accepts_yolo_on_the_same_connection(cx: &mut TestAppContext) {
    let (mut helm, cx, host, first) = yolo_dispatched(cx, Mode::New, false);
    assert_ne!(host.incarnation, 0);
    refuse_for_yolo(&mut helm, cx, first.id);
    assert!(label(cx, "prompt-confirm").is_some());
    click(cx, "prompt-confirm");
    let sent = wake_commands(&mut helm, cx);
    let [retry] = &sent[..] else {
        panic!("one create expected, got {sent:?}");
    };
    assert_eq!(retry.path, first.path);
    let mut expected = first.body.unwrap();
    expected["confirm_yolo"] = json!(true);
    assert_eq!(retry.body.as_ref(), Some(&expected));
}

/// A sleeping identity-less host can wake from a fresh launch, but an earlier answer has no connection to follow.
#[gpui::test]
fn an_unverified_yolo_answer_cannot_wake_a_host(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host, first) = yolo_dispatched(cx, Mode::New, false);
    refuse_for_yolo(&mut helm, cx, first.id);
    assert!(label(cx, "prompt-confirm").is_some());
    push(&mut helm, cx, &mut host, suspended_at(100));
    assert_published_host(cx, &host);
    assert!(matches!(host.state, HostStateView::Suspended { .. }));
    click(cx, "prompt-confirm");
    assert!(wake_commands(&mut helm, cx).is_empty());
    assert!(
        shown_refusal(cx)
            .unwrap()
            .contains("original host installation")
    );
}

/// The same registry entry can reconnect without gaining an identity. The old prompt cannot authorize that client.
#[gpui::test]
fn an_unverified_yolo_answer_cannot_follow_a_reconnect(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host, first) = yolo_dispatched(cx, Mode::New, false);
    refuse_for_yolo(&mut helm, cx, first.id);
    assert!(label(cx, "prompt-confirm").is_some());
    let previous = host.incarnation;
    let connection = connected_as(&host);
    push(&mut helm, cx, &mut host, connection);
    assert_published_host(cx, &host);
    assert!(host.identity.is_none());
    assert_ne!(host.incarnation, previous);
    click(cx, "prompt-confirm");
    assert!(wake_commands(&mut helm, cx).is_empty());
    assert!(shown_refusal(cx).is_some());
    let fresh = press_yolo_create(&mut helm, cx, &host);
    refuse_for_yolo(&mut helm, cx, fresh.id);
    assert!(label(cx, "prompt-confirm").is_some());
}

/// A replacement can be resumable while the old question is still visible.
/// The stale answer cannot send Resume even though an ordinary press could.
#[gpui::test]
fn a_stale_yolo_answer_cannot_wake_a_replacement_installation(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host, first) = yolo_dispatched(cx, Mode::New, true);
    refuse_for_yolo(&mut helm, cx, first.id);
    host.identity = Some("replacement-installation".into());
    push(&mut helm, cx, &mut host, suspended_at(100));
    assert_published_host(cx, &host);
    assert!(label(cx, "prompt-confirm").is_some());
    assert!(wake_commands(&mut helm, cx).is_empty());

    click(cx, "prompt-confirm");
    assert!(label(cx, "launch-wake").is_none());
    assert!(
        shown_refusal(cx)
            .unwrap()
            .contains("original host installation")
    );
    assert!(wake_commands(&mut helm, cx).is_empty());
}

/// A wake already admitted for a specific installation cannot authorize the
/// fresh connection when its registry row now names a different one.
#[gpui::test]
fn replacing_the_installation_during_wake_retires_yolo_consent(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host, first) = yolo_dispatched(cx, Mode::New, true);
    refuse_for_yolo(&mut helm, cx, first.id);
    push(&mut helm, cx, &mut host, suspended_at(100));
    assert_published_host(cx, &host);
    assert!(label(cx, "prompt-confirm").is_some());
    click(cx, "prompt-confirm");
    let sent = wake_commands(&mut helm, cx);
    let [resume] = &sent[..] else {
        panic!("one Resume expected, got {sent:?}")
    };
    assert_eq!(resume.path, RESUME);
    helm.reply(resume.id, 200, None);
    settle(cx);
    assert!(label(cx, "launch-wake").is_some());

    host.identity = Some("replacement-installation".into());
    let connection = connected_as(&host);
    push(&mut helm, cx, &mut host, connection);
    assert_published_host(cx, &host);
    assert!(label(cx, "launch-wake").is_none());
    assert!(
        shown_refusal(cx)
            .unwrap()
            .contains("different installation")
    );
    assert!(wake_commands(&mut helm, cx).is_empty());
    let fresh = press_yolo_create(&mut helm, cx, &host);
    refuse_for_yolo(&mut helm, cx, fresh.id);
    assert!(label(cx, "prompt-confirm").is_some());
}

/// Dispatch the real source launch while its original destination is connected.
fn yolo_source_dispatched<'a>(
    cx: &'a mut TestAppContext,
    route: &str,
    open: impl FnOnce(&mut gpui::Window, &mut gpui::App) -> bool,
) -> (FakeHelm, &'a mut VisualTestContext, HostView, FakeCommand) {
    let mut hosts = fixture_hosts(&[0, 8]);
    let mut host: HostView = serde_json::from_value(hosts.entries[1].value.clone()).unwrap();
    host.kind = "tensorlake".into();
    assert!(host.identity.is_some());
    assert_eq!(host.id, 9);
    host.state = connected_as(&host);
    hosts.entries[1].value = serde_json::to_value(&host).unwrap();
    let (mut helm, cx) = super::entry_tests::window_on(cx, Some("a"), hosts, Vec::new());
    assert_published_host(cx, &host);
    assert!(cx.update(open), "the source launcher is available");
    settle(cx);
    super::folder_default_tests::edit_draft(cx, |draft| {
        draft.choose_effort(Some(farhelm_proto::LaunchEffort::Medium));
    });
    super::view_tests::answer_catalog(
        &mut helm,
        cx,
        Some(json!([{ "id": "gpt-5", "harness": "codex", "efforts": ["high", "medium"] }])),
    );
    let draft = super::view_tests::draft(cx);
    assert_eq!(draft.agent.model.as_deref(), Some("gpt-5"));
    assert_eq!(
        draft.agent_selection().unwrap().effort,
        Some(farhelm_proto::LaunchEffort::Medium.into()),
    );
    assert!(source_commands(&mut helm, cx, route).is_empty());
    assert!(label(cx, "launcher-source-warning").is_some());
    click(cx, "launcher-launch");
    let sent = source_commands(&mut helm, cx, route);
    let [initial] = &sent[..] else {
        let refusal = shown_refusal(cx);
        let help = super::view_tests::launch_help(cx);
        panic!(
            "one source dispatch on {route} expected, got {sent:?}; refusal={refusal:?}; help={help:?}"
        );
    };
    assert_eq!(initial.path, route);
    (helm, cx, host, initial.clone())
}

const RESUME_SOURCE: &str = "/api/hosts/9/resume";

/// Leave source lifecycle traffic unanswered while draining independent reads.
fn source_commands(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    route: &str,
) -> Vec<FakeCommand> {
    answer_except(helm, cx, |command| {
        command.path == route || command.path == RESUME_SOURCE
    })
}

/// Update the source's own host, keeping the other fixture host visible.
fn publish_source_host(helm: &mut FakeHelm, cx: &mut VisualTestContext, host: &HostView) {
    helm.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&["1", "9"]),
        upsert: vec![KeyedEntry {
            key: HostView::key(host.id),
            value: serde_json::to_value(host).unwrap(),
        }],
        remove: Vec::new(),
    });
    settle(cx);
    assert_published_host(cx, host);
}

/// End the source and its shell tab while the answered launch is still held.
/// Recomputing either source guard after wake would now dispatch different fields.
fn finish_source_during_wake(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    use farhelm_proto::{SessionStatus, ui_channel::SessionRow};
    let read_source = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            crate::session::current_store(cx)
                .unwrap()
                .read(cx)
                .collections()
                .get::<SessionRow>("a")
                .unwrap()
                .clone()
        })
    };
    let mut source = read_source(cx);
    assert_eq!(source.info.status, SessionStatus::Running);
    assert_eq!(source.info.tabs.len(), 1);
    source.info.status = SessionStatus::Exited { exit_code: Some(0) };
    source.info.tabs.clear();
    helm.push(CollectionChanges {
        collection: Collection::Sessions,
        meta: crate::session_list::tests::listing_meta(&["a", "b", "c", "d"], 5, false),
        upsert: vec![KeyedEntry {
            key: "a".into(),
            value: serde_json::to_value(source).unwrap(),
        }],
        remove: Vec::new(),
    });
    settle(cx);
    let current = read_source(cx);
    assert_eq!(
        current.info.status,
        SessionStatus::Exited { exit_code: Some(0) }
    );
    assert!(current.info.tabs.is_empty());
}

/// Restart and Replace keep the source permission the user actually saw,
/// with just the dispatch connection refreshed.
fn answer_yolo_for_source(
    cx: &mut TestAppContext,
    route: &str,
    open: impl FnOnce(&mut gpui::Window, &mut gpui::App) -> bool,
) {
    let (mut helm, cx, mut host, initial) = yolo_source_dispatched(cx, route, open);
    let mut expected = initial.body.clone().unwrap();
    assert_eq!(expected["confirm_yolo"], false);
    if route.ends_with("/replace") {
        assert_eq!(expected["only_if_nothing_alive"], false);
        assert_eq!(expected["only_if_agent_ended"], false);
    } else {
        assert_eq!(expected["stop_if_running"], true);
        assert_eq!(expected["expected_incarnation"], host.incarnation);
    }
    refuse_for_yolo(&mut helm, cx, initial.id);
    host.state = suspended_at(100);
    publish_source_host(&mut helm, cx, &host);
    assert!(label(cx, "prompt-confirm").is_some());
    assert!(label(cx, "launcher-source-warning").is_some());
    click(cx, "prompt-confirm");
    let sent = source_commands(&mut helm, cx, route);
    let [resume] = &sent[..] else {
        panic!("one source Resume expected, got {sent:?}")
    };
    assert_eq!(resume.path, RESUME_SOURCE);
    helm.reply(resume.id, 200, None);
    settle(cx);
    assert!(source_commands(&mut helm, cx, route).is_empty());
    finish_source_during_wake(&mut helm, cx);
    assert!(label(cx, "launch-wake").is_some());

    host.incarnation += 1;
    host.state = connected_as(&host);
    publish_source_host(&mut helm, cx, &host);
    let sent = source_commands(&mut helm, cx, route);
    let [confirmed] = &sent[..] else {
        panic!("one confirmed source request expected, got {sent:?}")
    };
    assert_eq!(confirmed.path, route);
    expected["confirm_yolo"] = json!(true);
    if let Some(create) = expected
        .get_mut("with")
        .filter(|with| with.get("cwd").is_some())
    {
        create["expected_incarnation"] = json!(host.incarnation);
    } else {
        expected["expected_incarnation"] = json!(host.incarnation);
    }
    assert_eq!(confirmed.body.as_ref(), Some(&expected));
}

/// The refusal of a source operation must use the install recorded at
/// dispatch, even when the row retargeted while the operation was out.
fn source_refusal_after_retarget(
    cx: &mut TestAppContext,
    route: &str,
    open: impl FnOnce(&mut gpui::Window, &mut gpui::App) -> bool,
) {
    let (mut helm, cx, mut host, initial) = yolo_source_dispatched(cx, route, open);
    host.identity = Some("replacement-installation".into());
    host.incarnation += 1;
    host.state = connected_as(&host);
    publish_source_host(&mut helm, cx, &host);
    refuse_for_yolo(&mut helm, cx, initial.id);
    assert!(label(cx, "prompt-confirm").is_none());
    assert!(
        shown_refusal(cx)
            .unwrap()
            .contains("original host installation")
    );
    assert!(source_commands(&mut helm, cx, route).is_empty());
}

/// Replace with cannot mistake the currently registered host for its original target.
#[gpui::test]
fn a_replace_with_yolo_refusal_cannot_authorize_a_replacement_installation(
    cx: &mut TestAppContext,
) {
    source_refusal_after_retarget(cx, "/api/sessions/a/replace", |window, cx| {
        super::entry::open_replace_with("a", window, cx)
    });
}

/// Restart with's source key alone cannot authorize a different installation.
#[gpui::test]
fn a_restart_with_yolo_refusal_cannot_authorize_a_replacement_installation(
    cx: &mut TestAppContext,
) {
    source_refusal_after_retarget(cx, "/api/sessions/a/restart", |window, cx| {
        super::entry::open_restart_with("a", window, cx)
    });
}

/// Replace with carries both answers across wake, for the same folder and source.
#[gpui::test]
fn replace_with_carries_yolo_and_source_guards_through_wake(cx: &mut TestAppContext) {
    answer_yolo_for_source(cx, "/api/sessions/a/replace", |window, cx| {
        super::entry::open_replace_with("a", window, cx)
    });
}

/// Restart with may wake before spending its one-time permission to stop a working source.
#[gpui::test]
fn restart_with_carries_yolo_and_stop_consent_through_wake(cx: &mut TestAppContext) {
    answer_yolo_for_source(cx, "/api/sessions/a/restart", |window, cx| {
        super::entry::open_restart_with("a", window, cx)
    });
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
    push_host(helm, host, state);
    settle(cx);
}

/// Share the host update with screenshot fixtures, which settle via their own frame loop.
pub(crate) fn push_host(helm: &mut FakeHelm, host: &mut HostView, state: HostStateView) {
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

/// Provider work pauses the deadline. A host that then stays Connecting gives the draft back after one minute.
#[gpui::test]
fn a_folder_wake_releases_the_draft_after_the_connection_deadline(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = open(cx);
    let resume = press(&mut helm, cx);
    helm.reply(resume.id, 200, None);
    let resuming = changing(LifecycleDirection::Resuming);
    push(&mut helm, cx, &mut host, resuming);
    advance(cx, Duration::from_secs(60));
    let connecting = crate::hosts::tests::connecting(1, None);
    push(&mut helm, cx, &mut host, connecting);
    advance(cx, Duration::from_secs(59));
    assert!(label(cx, "launch-wake").is_some());
    advance(cx, Duration::from_secs(1));
    let reason =
        "The host resumed but did not connect within 60 seconds; try again when it connects";
    assert_eq!(
        shown_refusal(cx).unwrap(),
        format!("Not launched. {reason}")
    );
    assert!(label(cx, "launch-wake").is_none());
    assert!(wake_commands(&mut helm, cx).is_empty());
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
    let (mut helm, cx, mut host) = open_on(cx, stopped_container(None));
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

// ---- The helm's tagged wake outcome ---------------------------------------------------------------------------------
//
// The helm answers a Resume within 20 seconds; past that it says only that the Resume carries on, and publishes how
// the Resume ended on the host's hosts-list entry, under the request tag the launcher sent. These tests pin that the
// launcher sends its own tag, acts at once on a failure under that tag, and ignores every other outcome.

/// The tag a host's wake fixture already carries in `host-list.json`: another sender's, never the launcher's.
const FIXTURE_TAG: &str = "contract-fixture";

/// The fixture's registered host turned into a container that is Stopped, with `start_failed` as the container host's
/// last automatic-start diagnosis. Its installation and connection identity stay the sandbox fixture's, which the wake
/// only compares with themselves. The late-failure golden scene starts from it too.
pub(crate) fn stopped_container(start_failed: Option<&str>) -> HostView {
    let mut host: HostView =
        serde_json::from_value(fixture_hosts(&[11]).entries[0].value.clone()).unwrap();
    host.kind = "podman".into();
    host.state = HostStateView::Stopped {
        waiting_for_place: false,
        start_failed: start_failed.map(str::to_owned),
    };
    host
}

/// The request tag a launcher wake's Resume carried, asserting that it carried one in the helm's own body type, so a
/// change to either side's spelling fails here.
fn tag_of(resume: &FakeCommand) -> String {
    let body = resume
        .body
        .clone()
        .expect("the launcher's Resume carries a body");
    let body: farhelm_proto::ui_channel::ResumeHostBody =
        serde_json::from_value(body).expect("the body is the helm's resume body");
    body.request
        .expect("the launcher's Resume carries a request tag")
}

/// Publish `host` again in its current state, now listing `outcome` under `request` as its latest tagged Resume, as
/// the helm's hosts list does once that Resume ends.
fn push_outcome(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    host: &mut HostView,
    request: &str,
    outcome: WakeResult,
) {
    host.wake_outcome = Some(WakeOutcome {
        request: request.to_owned(),
        outcome,
    });
    let state = host.state.clone();
    push(helm, cx, host, state);
}

/// The executor's clock, which `advance` moves and the wake's budget reads.
fn now(cx: &mut VisualTestContext) -> std::time::Instant {
    cx.update(|_, cx| cx.background_executor().now())
}

/// The launcher is the one sender that ends a wait on the helm's tagged outcome, so it must tag every Resume it
/// sends, a busy retry's included, with one tag per wake, and two wakes must never share one: a shared tag would let
/// the first wake's failure end the second. The tag also must not be the fixture's, which the launcher ignores.
#[gpui::test]
fn the_launchers_resume_carries_one_tag_per_wake(cx: &mut TestAppContext) {
    use farhelm_proto::ui_channel::HelmMarker;

    let (mut helm, cx, mut host) = open(cx);
    let first = press(&mut helm, cx);
    let tag = tag_of(&first);
    assert!(tag.starts_with("launcher-"), "{tag}");
    assert_ne!(tag, FIXTURE_TAG);
    helm.reply_marked(
        first.id,
        409,
        vec![HelmMarker::HostBusy],
        Some(json!("The host is busy")),
    );
    settle(cx);
    let initial = host.state.clone();
    push(&mut helm, cx, &mut host, initial);
    advance(cx, Duration::from_secs(1));
    let retry = wake_commands(&mut helm, cx);
    let [retry] = &retry[..] else {
        panic!("one busy retry expected, got {retry:?}")
    };
    assert_eq!(retry.path, RESUME);
    assert_eq!(tag_of(retry), tag, "a busy retry keeps the wake's tag");
    helm.reply(retry.id, 409, Some(json!("The sandbox is gone")));
    settle(cx);
    assert!(shown_refusal(cx).is_some(), "the first wake ended");

    let second = press(&mut helm, cx);
    assert_ne!(tag_of(&second), tag, "a new wake mints a new tag");
}

/// A container host checks its container over SSH before it takes the connection hold, so on a slow container host
/// the helm can answer 504 while the host still reads Stopped with no Starting hold, and then fail: nothing in the
/// pushed state ever changes. The draft must come back as soon as the helm publishes that failure under this wake's
/// tag, with the helm's reason behind the container's start lead, not after the minute with a generic timeout.
#[gpui::test]
fn a_late_container_failure_before_starting_returns_the_draft_with_its_reason(
    cx: &mut TestAppContext,
) {
    const WHY: &str = "Podman could not inspect the container: ssh timed out";
    let (mut helm, cx, mut host) = open_on(cx, stopped_container(None));
    let resume = press(&mut helm, cx);
    let tag = tag_of(&resume);
    assert!(label(cx, "launch-wake").unwrap().starts_with("Starting "));
    helm.reply(resume.id, 504, Some(json!("The resume carries on")));
    settle(cx);
    let started = now(cx);
    let stopped = host.state.clone();
    push(&mut helm, cx, &mut host, stopped);
    advance(cx, Duration::from_secs(30));
    assert!(
        label(cx, "launch-wake").is_some(),
        "an unchanged Stopped host alone keeps the wake waiting"
    );
    assert!(shown_refusal(cx).is_none());

    push_outcome(
        &mut helm,
        cx,
        &mut host,
        &tag,
        WakeResult::Failed { reason: WHY.into() },
    );
    assert_eq!(
        shown_refusal(cx).unwrap(),
        format!("Not launched. Starting the host failed: {WHY}"),
    );
    assert!(
        now(cx) - started < Duration::from_secs(60),
        "before the budget"
    );
    assert!(label(cx, "launch-wake").is_none());
    assert!(wake_commands(&mut helm, cx).is_empty(), "no create escapes");
    assert!(cx.update(|_, cx| !crate::hosts::launch_in_flight(cx)));
}

/// A sandbox whose earlier Resume already failed shows that diagnosis before the press. When the new Resume fails the
/// same way, the pushed `resume_failed` does not change, which the state watch reads as nothing new; only the tagged
/// outcome says this wake failed. The draft must come back at once, with the helm's reason in the sandbox's wording.
#[gpui::test]
fn a_repeated_sandbox_failure_returns_the_draft_at_once(cx: &mut TestAppContext) {
    const WHY: &str = "Tensorlake could not resume this host: the sandbox is unavailable";
    let mut host: HostView =
        serde_json::from_value(fixture_hosts(&[11]).entries[0].value.clone()).unwrap();
    let HostStateView::Suspended { resume_failed, .. } = &mut host.state else {
        panic!("the fixture's host is suspended")
    };
    *resume_failed = Some(WHY.into());
    let (mut helm, cx, mut host) = open_on(cx, host);
    // The provider's re-probe after the hold ends publishes the same diagnosis it showed before the press.
    let failed_again = host.state.clone();
    let resume = press(&mut helm, cx);
    let tag = tag_of(&resume);
    helm.reply(resume.id, 504, Some(json!("The resume carries on")));
    settle(cx);
    push(
        &mut helm,
        cx,
        &mut host,
        changing(LifecycleDirection::Resuming),
    );
    push(&mut helm, cx, &mut host, failed_again);
    assert!(
        label(cx, "launch-wake").is_some(),
        "the same diagnosis again is not a new one to the state watch"
    );

    push_outcome(
        &mut helm,
        cx,
        &mut host,
        &tag,
        WakeResult::Failed { reason: WHY.into() },
    );
    assert_eq!(
        shown_refusal(cx).unwrap(),
        format!("Not launched. Resuming the host failed: {WHY}"),
    );
    assert!(label(cx, "launch-wake").is_none());
    assert!(wake_commands(&mut helm, cx).is_empty());
}

/// The container counterpart: the automatic start already failed and left `start_failed`, and the launcher's own
/// start fails with the same text. The draft must come back at once in the container's start wording, which is what
/// the stopped container's wake says throughout.
#[gpui::test]
fn a_repeated_container_start_failure_returns_the_draft_at_once(cx: &mut TestAppContext) {
    const WHY: &str = "podman start failed: no space left on device";
    let (mut helm, cx, mut host) = open_on(cx, stopped_container(Some(WHY)));
    let resume = press(&mut helm, cx);
    let tag = tag_of(&resume);
    helm.reply(resume.id, 504, Some(json!("The resume carries on")));
    settle(cx);
    push(
        &mut helm,
        cx,
        &mut host,
        HostStateView::TensorlakeChanging {
            starting: true,
            direction: LifecycleDirection::Resuming,
            requested_at: 1,
        },
    );
    let failed_again = HostStateView::Stopped {
        waiting_for_place: false,
        start_failed: Some(WHY.into()),
    };
    push(&mut helm, cx, &mut host, failed_again);
    assert!(
        label(cx, "launch-wake").is_some(),
        "the same diagnosis again is not a new one to the state watch"
    );

    push_outcome(
        &mut helm,
        cx,
        &mut host,
        &tag,
        WakeResult::Failed { reason: WHY.into() },
    );
    assert_eq!(
        shown_refusal(cx).unwrap(),
        format!("Not launched. Starting the host failed: {WHY}"),
    );
    assert!(label(cx, "launch-wake").is_none());
    assert!(wake_commands(&mut helm, cx).is_empty());
}

/// A stopped container's start refused in the reply itself names the start, not a resume: the reply's lead is the
/// provider's, like the rest of the container's wake (SPEC.md, "Container sessions").
#[gpui::test]
fn a_refused_container_start_says_start(cx: &mut TestAppContext) {
    let (mut helm, cx, _) = open_on(cx, stopped_container(None));
    let resume = press(&mut helm, cx);
    helm.reply(
        resume.id,
        409,
        Some(json!("the container is changing state; try again")),
    );
    settle(cx);
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. Starting the host failed: the container is changing state; try again"),
    );
    assert!(label(cx, "launch-wake").is_none());
}

/// The helm keeps one outcome per host, whoever sent its Resume: the fixture's sender, another window, or an earlier
/// wake of this launcher. None of those says this wake's Resume failed, so a failure under any other tag must leave
/// the wake waiting; only the state watch and the minute can end it then.
#[gpui::test]
fn a_failure_under_another_tag_is_ignored(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = open(cx);
    let fixture = host
        .wake_outcome
        .clone()
        .expect("the fixture lists an outcome");
    assert_eq!(fixture.request, FIXTURE_TAG);
    assert!(matches!(fixture.outcome, WakeResult::Failed { .. }));

    // An earlier wake of this same launcher, refused, leaves its tag behind.
    let earlier = press(&mut helm, cx);
    let earlier_tag = tag_of(&earlier);
    helm.reply(
        earlier.id,
        409,
        Some(json!("The test refused the first wake")),
    );
    settle(cx);
    assert!(shown_refusal(cx).is_some());

    let resume = press(&mut helm, cx);
    assert!(shown_refusal(cx).is_none(), "the second wake is under way");
    helm.reply(resume.id, 504, Some(json!("The resume carries on")));
    settle(cx);
    for other in [FIXTURE_TAG, earlier_tag.as_str()] {
        push_outcome(
            &mut helm,
            cx,
            &mut host,
            other,
            WakeResult::Failed {
                reason: "Another sender's Resume failed".into(),
            },
        );
        assert!(
            label(cx, "launch-wake").is_some(),
            "{other} is not this wake's"
        );
        assert!(shown_refusal(cx).is_none());
    }
    push(&mut helm, cx, &mut host, connected());
    let sent = creates(&mut helm, cx);
    assert_eq!(sent.len(), 1, "the connection still completes the wake");
    helm.reply(sent[0].id, 409, Some(json!("The test refused the create")));
    settle(cx);
}

/// `Ready` says only that the provider finished, not that the host connected, so under this wake's own tag it must
/// neither end the wake nor send the create: the wait for a fresh connection goes on, and the connection still
/// completes it.
#[gpui::test]
fn a_ready_outcome_does_not_end_the_wait(cx: &mut TestAppContext) {
    let (mut helm, cx, mut host) = open(cx);
    let resume = press(&mut helm, cx);
    let tag = tag_of(&resume);
    helm.reply(resume.id, 504, Some(json!("The resume carries on")));
    settle(cx);
    push(
        &mut helm,
        cx,
        &mut host,
        crate::hosts::tests::connecting(1, None),
    );
    push_outcome(&mut helm, cx, &mut host, &tag, WakeResult::Ready);
    assert!(label(cx, "launch-wake").is_some());
    assert!(shown_refusal(cx).is_none());
    assert!(wake_commands(&mut helm, cx).is_empty(), "no early create");
    push(&mut helm, cx, &mut host, connected());
    let sent = creates(&mut helm, cx);
    assert_eq!(sent.len(), 1);
    helm.reply(sent[0].id, 409, Some(json!("The test refused the create")));
    settle(cx);
}

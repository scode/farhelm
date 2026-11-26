//! Tests of environment sessions in the list (`environment.rs`): a sandbox's and a container's row marks, words and
//! hover help, the Suspended badge and its absence on a stopped container, the row menu's Suspend, Resume and Stop and
//! suspend with their preconditions, questions and the exact requests they send, the fresh per-host activity read before
//! a Suspend, the verb captured at the click, the 504 at the helm's bound shown as still pending, and refusals; and a
//! session whose environment is being deleted, with its phase in place of the stale cause and every menu command
//! refused until the host leaves that phase. The open session's header and suspended card are
//! `session_pane/environment_tests.rs`'s, and bulk admission's refusal of a session being deleted is `bulk_tests.rs`'s.
//!
//! Everything runs in the real main view over the helm client's fake, as the list's other lifecycle tests do, and reads
//! what the window sent from the fake and what it drew from GPUI's driver record. An environment session is seeded as
//! the helm lists one: an ordinary session on host 2, which the registry lists as session-owned with an environment
//! described for it ([`environment_host`]); its phase is the host's pushed state. No running app can show one (the
//! end-to-end harness seeds no sandbox or container), so these and the pane's tests are the evidence.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::tensorlake::LifecycleDirection;
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, HelmMarker, HostStateView, HostView,
    KeyedEntry, RefreshView, SessionRow,
};
use farhelm_proto::{EnvironmentHostView, LaunchTarget, Place, SessionStatus};
use gpui::{Role, TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::environment::needs_confirmation;
use super::host_rail::tests::{hover_help, texts_within};
use super::lifecycle_tests::node_bounds;
use super::menu_tests::{any_layer, drawn, label_of, snapshot};
use super::tests::{
    Fixture, choose_menu_item as choose, launcher_drawn, open_main, painted_frame as texts, session,
};
use crate::environment_ui::{ProviderUi, target_label};
use crate::host_text::HostSentence;
use crate::launcher::{draft::Tab, folder_default_tests::edit_draft};
use crate::local_time::Zone;
use crate::test_support::{self, focused_id, frame, meta};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// The two providers, as the fixtures seed their hosts.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Provider {
    /// A Tensorlake sandbox of the `dev` recipe.
    Sandbox,
    /// A Podman container of the `dev` recipe on the place host the helm names "Build machine".
    Container,
}

/// A connected host, as the helm pushes one.
pub(crate) fn connected() -> HostStateView {
    HostStateView::Connected {
        identity: Some("id-env".into()),
        build_version: "0.40.0".into(),
        old_version: false,
        newer_version: false,
        refresh: RefreshView::Ok { sessions: 1 },
    }
}

/// A structured Bash launch, so the session's actions name its shell even if its main process has ended.
pub(crate) fn bash_session(key: &'static str) -> Fixture {
    session(key).launch(json!({
        "kind": "agent",
        "selection": {"harness": "bash", "permissions": null},
        "start": ["bash", "{farhelm_args}"],
        "resume": null,
    }))
}

/// A host suspended outside Farhelm, noticed at `observed` (unix ms).
pub(crate) fn suspended_at(observed: i64) -> HostStateView {
    HostStateView::Suspended {
        observed_at: observed,
        suspended_by_farhelm_at: None,
        resume_failed: None,
    }
}

/// A host whose provider is changing it, in `direction`.
pub(crate) fn changing(direction: LifecycleDirection) -> HostStateView {
    HostStateView::TensorlakeChanging {
        starting: false,
        direction,
        requested_at: 0,
    }
}

/// A host being deleted, as the helm pushes one: its environment's destruction under way (`failure` `None`), or failed
/// with the provider's words `failure` and waiting for Retry in Hosts.
pub(crate) fn deleting(failure: Option<&str>) -> HostStateView {
    HostStateView::Deleting {
        failure: failure.map(str::to_owned),
    }
}

/// Host 2, the fixtures' `buildbox`, as the session-owned environment `provider` made for its session, in `state`:
/// the shape the helm lists a new environment session's host in.
pub(crate) fn environment_host(provider: Provider, state: HostStateView) -> HostView {
    let (kind, place, place_name) = match provider {
        Provider::Sandbox => ("tensorlake", Place::TensorlakeAccount, None),
        Provider::Container => ("podman", Place::PodmanHost(1), Some("Build machine".into())),
    };
    let mut host = crate::hosts::tests::host(2, kind, state);
    host.is_session_owned = true;
    host.environment = Some(EnvironmentHostView {
        name: "owned-environment".into(),
        place_name,
        target: LaunchTarget::Environment {
            recipe: "dev".into(),
            place,
        },
        tunnels: None,
        credential_warning: None,
    });
    host
}

/// The registry holding the local host and `host`.
pub(crate) fn registry(host: &HostView) -> CollectionState {
    let local = crate::hosts::tests::host(1, "local", connected());
    CollectionState {
        collection: Collection::Hosts,
        meta: meta(&[HostView::key(1).as_str(), HostView::key(2).as_str()]),
        entries: [local, host.clone()]
            .into_iter()
            .map(|view| KeyedEntry {
                key: HostView::key(view.id),
                value: serde_json::to_value(view).unwrap(),
            })
            .collect(),
    }
}

/// Push `host` as changed in the registry, as the helm pushes a new phase.
pub(crate) fn push_host(fake: &mut FakeHelm, host: &HostView, cx: &mut VisualTestContext) {
    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&[HostView::key(1).as_str(), HostView::key(2).as_str()]),
        upsert: vec![KeyedEntry {
            key: HostView::key(host.id),
            value: serde_json::to_value(host).unwrap(),
        }],
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// The real main view over `rows` (each with its tab count; the first one open), Skip session confirmations as `skip`
/// says, and the registry with `host`.
pub(crate) fn open<'a>(
    cx: &'a mut TestAppContext,
    rows: &[(Fixture, usize)],
    skip: bool,
    host: &HostView,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let mut snapshot = snapshot(rows, skip);
    snapshot.push(registry(host));
    open_main(cx, snapshot)
}

/// Whether `path` is one of the requests these tests are about or must see if it is sent: the activity read, a Suspend,
/// a Resume, a Stop, a Stop and suspend, and the Restart and Replace a frozen environment withholds.
fn is_environment(path: &str) -> bool {
    path.starts_with("/api/sessions?host=")
        || [
            "/suspend",
            "/resume",
            "/stop",
            "/stop-and-suspend",
            "/restart",
            "/replace",
        ]
        .iter()
        .any(|end| path.ends_with(end))
}

/// The environment requests the window sent since the last call, as (id, method, path, body), left unanswered. A
/// host activity check must be able to answer behind an outstanding ordered command. Other commands (a preference
/// write, a seen mark) are answered with success at once.
pub(crate) fn pending(fake: &mut FakeHelm) -> Vec<(u64, Method, String, Value)> {
    let mut found = Vec::new();
    for command in fake.commands() {
        if is_environment(&command.path) {
            if command.method == Method::Get && command.path.starts_with("/api/sessions?host=") {
                assert_eq!(command.lane, farhelm_helm_client::Lane::Unordered);
            }
            found.push((
                command.id,
                command.method,
                command.path,
                command.body.unwrap_or_default(),
            ));
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    found
}

/// The one environment request sent since the last call, which must be `method` on `path`; its id and body.
pub(crate) fn one(fake: &mut FakeHelm, method: Method, path: &str) -> (u64, Value) {
    let sent = pending(fake);
    assert_eq!(
        sent.iter()
            .map(|(_, method, path, _)| (*method, path.as_str()))
            .collect::<Vec<_>>(),
        [(method, path)],
        "exactly one request"
    );
    let (id, _, _, body) = sent.into_iter().next().unwrap();
    (id, body)
}

/// A `GET /api/sessions` answer listing `rows`, cut by the listing cap when `truncated` says so.
pub(crate) fn listing(rows: &[Fixture], truncated: bool) -> Value {
    json!({
        "sessions": rows.iter().map(|row| row.entry().value).collect::<Vec<_>>(),
        "total": rows.len(),
        "truncated": truncated,
    })
}

/// `key` idle, its agent waiting for nothing: a status that lets a Suspend go ahead unasked.
pub(crate) fn idle(key: &'static str) -> Fixture {
    session(key).status(json!({"state": "idle"}))
}

/// The ids of the menu items drawn in a fresh frame, top to bottom.
fn menu_items(cx: &mut VisualTestContext) -> Vec<String> {
    let mut items: Vec<_> = frame(cx)
        .into_iter()
        .filter(|node| node.role == Some(Role::MenuItem))
        .filter_map(|node| Some((node.bounds.origin.y, node.element_id?.to_string())))
        .collect();
    items.sort_by(|a, b| f32::from(a.0).total_cmp(&f32::from(b.0)));
    items.into_iter().map(|(_, id)| id).collect()
}

// ---- Marks and words -----------------------------------------------------------------------------------------------

/// SPEC.md names a sandbox session's host "<recipe> sandbox" and a container's "<recipe> container on <host>", each with
/// its own mark in the session's metadata, and every mark that carries meaning has hover text. Reading the provider
/// from the host's kind string, or naming the registry's internal name, would label the row wrongly. Specified: a
/// sandbox session's locality mark is named "Sandbox", draws the cube, and helps with "Sandbox: runs in its own
/// disposable Tensorlake sandbox", and its last line reads "dev sandbox"; a container session's is named "Container",
/// draws the box, helps with "Container: runs in its own disposable container on one of your hosts", and reads "dev
/// container on Build machine"; the menu's header ends with " · dev sandbox".
#[gpui::test]
fn an_environment_rows_mark_and_host_name_are_its_providers(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(session("a"), 0)], false, &host);
    pending(&mut fake);
    assert_eq!(
        label_of(cx, "session-row-a-host").as_deref(),
        Some("Sandbox")
    );
    assert!(
        drawn(cx, "session-row-a-host:icons/environment-sandbox.svg"),
        "the cube is drawn"
    );
    assert_eq!(
        hover_help(cx, "session-row-a-host").as_deref(),
        Some("Sandbox: runs in its own disposable Tensorlake sandbox")
    );
    assert!(texts(cx).iter().any(|text| text == "dev sandbox"));
    test_support::click(cx, "session-menu-a");
    assert!(
        label_of(cx, "session-menu-summary-a")
            .is_some_and(|summary| summary.ends_with(" · dev sandbox")),
        "{:?}",
        label_of(cx, "session-menu-summary-a")
    );
    test_support::keys(cx, "escape");

    let container = environment_host(Provider::Container, connected());
    push_host(&mut fake, &container, cx);
    assert_eq!(
        label_of(cx, "session-row-a-host").as_deref(),
        Some("Container")
    );
    assert!(drawn(
        cx,
        "session-row-a-host:icons/environment-container.svg"
    ));
    assert_eq!(
        hover_help(cx, "session-row-a-host").as_deref(),
        Some("Container: runs in its own disposable container on one of your hosts")
    );
    assert!(
        texts(cx)
            .iter()
            .any(|text| text == "dev container on Build machine")
    );
}

/// SPEC.md: "Suspended rows have grey titles and a pause-marked **Suspended** badge", but a stopped container "has its
/// own state and is never labelled Suspended". A badge on a stopped container would promise that Resume continues its
/// processes, which a start does not. Specified: a session on a suspended sandbox draws the badge named "Suspended" with
/// its help "Suspended: this session's environment is frozen; Resume continues its processes" in place of the stale
/// cause; once the host is a stopped container, no badge is drawn and the cause reads "Environment stopped".
#[gpui::test]
fn a_suspended_row_has_its_badge_and_a_stopped_container_never_does(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, suspended_at(0));
    let (mut fake, cx) = open(cx, &[(session("a").stale(), 0)], false, &host);
    pending(&mut fake);
    assert_eq!(
        label_of(cx, "session-row-a-suspended").as_deref(),
        Some("Suspended")
    );
    assert_eq!(
        hover_help(cx, "session-row-a-suspended").as_deref(),
        Some("Suspended: this session's environment is frozen; Resume continues its processes")
    );
    assert!(
        !drawn(cx, "session-row-a-cause"),
        "the badge replaces the cause"
    );

    let stopped = environment_host(
        Provider::Container,
        HostStateView::Stopped {
            start_failed: None,
            waiting_for_place: false,
        },
    );
    push_host(&mut fake, &stopped, cx);
    assert!(
        !drawn(cx, "session-row-a-suspended"),
        "a stopped container is not suspended"
    );
    assert_eq!(
        label_of(cx, "session-row-a-cause").as_deref(),
        Some("Environment stopped")
    );
}

// ---- The menu ------------------------------------------------------------------------------------------------------

/// The old menu put the environment's commands at the head of the Clone group, offered Stop and suspend only while the
/// environment runs, and withheld the supervisor's lifecycle verbs while it was frozen. Specified: a connected sandbox
/// session's menu includes the ordinary viewer actions before Suspend and Stop and suspend. Once suspended it reads
/// the same with Resume in Suspend's place and no Stop and suspend, Replace and Stop ignore a click (the menu stays
/// open), while Stop's description says the sandbox keeps billing, and Replace with… still acts, since its launcher
/// resumes the host first.
#[gpui::test]
fn the_menu_offers_the_environment_commands_by_phase(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(session("a"), 0)], false, &host);
    pending(&mut fake);
    test_support::click(cx, "session-menu-a");
    assert_eq!(
        menu_items(cx),
        [
            "session-menu-rename",
            "session-menu-seen",
            "session-menu-pin",
            "session-menu-group",
            "session-menu-mute",
            "session-menu-suspend",
            "session-menu-stop-and-suspend",
            "session-menu-clone",
            "session-menu-replace-with",
            "session-menu-replace",
            "session-menu-stop",
            "session-menu-delete",
        ]
    );
    test_support::keys(cx, "escape");

    push_host(
        &mut fake,
        &environment_host(Provider::Sandbox, suspended_at(0)),
        cx,
    );
    test_support::click(cx, "session-menu-a");
    assert_eq!(
        menu_items(cx),
        [
            "session-menu-rename",
            "session-menu-seen",
            "session-menu-pin",
            "session-menu-group",
            "session-menu-mute",
            "session-menu-resume",
            "session-menu-clone",
            "session-menu-replace-with",
            "session-menu-replace",
            "session-menu-stop",
            "session-menu-delete",
        ]
    );
    assert!(
        texts(cx)
            .iter()
            .any(|text| text
                == "The sandbox keeps running and billing until you suspend or delete it."),
        "Stop's description is the provider's"
    );
    for item in ["session-menu-replace", "session-menu-stop"] {
        test_support::click(cx, item);
        assert!(
            drawn(cx, "session-menu"),
            "{item} is unavailable while frozen"
        );
    }
    assert!(pending(&mut fake).is_empty(), "nothing was sent");
    // Replace with stays available on a suspended sandbox that offers Resume: the launcher it opens resumes the host
    // before sending anything, so choosing it closes the menu.
    test_support::click(cx, "session-menu-replace-with");
    assert!(
        !drawn(cx, "session-menu"),
        "Replace with acts while the environment offers Resume"
    );
}

/// The focus coordinator's reveal (its module docs, after SPEC.md's "Reaching everything by keyboard"): the control
/// focus lands on is scrolled into view. An environment session's two extra commands make its menu's commands taller
/// than the menu's cap in the smallest window, and Up on the closed button opens the menu on its last command, Delete,
/// which the layer manager focuses before the menu has a position to scroll to. Specified: in a window of the app's
/// least size, where the menu opened on Rename leaves Delete below its bottom, a sandbox session's menu opened with Up
/// from its focused button focuses Delete, drawn wholly inside the menu, with Rename scrolled out above the menu's top.
#[gpui::test]
fn up_opens_an_environment_menu_scrolled_to_its_delete(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(session("a"), 0)], false, &host);
    pending(&mut fake);
    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    test_support::settle(cx);
    // Opened once on Rename, to check the premise, and closed, so focus is on the button, where Up opens the menu.
    test_support::click(cx, "session-menu-a");
    let menu = test_support::bounds(cx, "session-menu");
    let delete = node_bounds(cx, "session-menu-delete");
    assert!(
        delete.bottom() > menu.bottom(),
        "the premise: the commands do not fit, so Delete starts below the menu's bottom: {delete:?} in {menu:?}"
    );
    test_support::keys(cx, "escape");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-a"));

    test_support::keys(cx, "up");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-delete"));
    let menu = test_support::bounds(cx, "session-menu");
    let rename = node_bounds(cx, "session-menu-rename");
    assert!(
        rename.top() < menu.top(),
        "the menu scrolled to Delete: {rename:?} in {menu:?}"
    );
    let delete = node_bounds(cx, "session-menu-delete");
    assert!(
        menu.top() <= delete.top() && delete.bottom() <= menu.bottom(),
        "Delete is in view: {delete:?} in {menu:?}"
    );
}

/// The old menu's Clone said where the new session goes, and a Clone of an environment session opens the launcher on
/// a fresh environment, not the source's. Specified: Clone's description reads "New session in a new environment, keep
/// this one" in the menu of a session on a sandbox, and again once its host is a container, and "New session, keep this
/// one" in the menu of a session on the local host.
#[gpui::test]
fn clone_describes_a_new_environment_for_an_environment_session(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let rows = [(session("a"), 0), (session("b").host(1), 0)];
    let (mut fake, cx) = open(cx, &rows, false, &host);
    pending(&mut fake);
    let describes = |button: &'static str, cx: &mut VisualTestContext| {
        test_support::click(cx, button);
        let texts = texts_within(&frame(cx), "session-menu-clone");
        test_support::keys(cx, "escape");
        texts
            .into_iter()
            .map(|text| text.text.to_string())
            .collect::<Vec<_>>()
    };
    let environment = "New session in a new environment, keep this one".to_owned();
    assert_eq!(
        label_of(cx, "session-row-a-host").as_deref(),
        Some("Sandbox"),
        "the premise"
    );
    assert!(describes("session-menu-a", cx).contains(&environment));
    push_host(
        &mut fake,
        &environment_host(Provider::Container, connected()),
        cx,
    );
    assert_eq!(
        label_of(cx, "session-row-a-host").as_deref(),
        Some("Container"),
        "the premise"
    );
    assert!(describes("session-menu-a", cx).contains(&environment));
    assert!(describes("session-menu-b", cx).contains(&"New session, keep this one".to_owned()));
}

// ---- Suspend's activity read ----------------------------------------------------------------------------------------

/// SPEC.md: "Immediate Suspend requires a connected host after the activity read and a complete listing with
/// conclusive non-working statuses." The pushed list is filtered by the window's query and may be cut, so a Suspend
/// authorized from it could freeze an agent the list never showed. Specified: with the pushed list showing only idle a,
/// Suspend from a's menu first sends `GET /api/sessions?host=2` and nothing else; a complete answer listing idle a
/// sends `POST /api/hosts/2/suspend` with no body and asks nothing; the same Suspend answered with a listing that also
/// holds a running b, and then with an idle but truncated listing, asks "Suspend sandbox?" each time, sending nothing
/// before the answer.
#[gpui::test]
fn suspend_reads_its_hosts_own_listing_before_going_ahead(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(idle("a"), 0)], false, &host);
    pending(&mut fake);

    choose(cx, "session-menu-a", "session-menu-suspend");
    let (read, _) = one(&mut fake, Method::Get, "/api/sessions?host=2");
    fake.reply(read, 200, Some(listing(&[idle("a")], false)));
    test_support::settle(cx);
    assert!(!any_layer(cx), "a complete, idle listing asks nothing");
    let (suspend, body) = one(&mut fake, Method::Post, "/api/hosts/2/suspend");
    assert_eq!(body, Value::Null, "Suspend carries no body");
    fake.reply(suspend, 200, Some(json!({})));
    test_support::settle(cx);

    for (answer, why) in [
        (
            listing(&[idle("a"), session("b")], false),
            "a running session the pushed list never showed",
        ),
        (listing(&[idle("a")], true), "a listing the cap cut"),
    ] {
        choose(cx, "session-menu-a", "session-menu-suspend");
        let (read, _) = one(&mut fake, Method::Get, "/api/sessions?host=2");
        fake.reply(read, 200, Some(answer));
        test_support::settle(cx);
        assert_eq!(
            label_of(cx, "prompt").as_deref(),
            Some("Suspend sandbox?"),
            "{why}"
        );
        assert!(
            pending(&mut fake).is_empty(),
            "nothing before the answer ({why})"
        );
        test_support::click(cx, "prompt-cancel");
        assert!(
            pending(&mut fake).is_empty(),
            "Cancel sends nothing ({why})"
        );
    }

    // A connected host can still be serving its previous list after a failed refresh.
    choose(cx, "session-menu-a", "session-menu-suspend");
    let (read, _) = one(&mut fake, Method::Get, "/api/sessions?host=2");
    let mut phase = connected();
    if let HostStateView::Connected { refresh, .. } = &mut phase {
        *refresh = RefreshView::Failed {
            error: "reading activity failed".into(),
        };
    }
    push_host(&mut fake, &environment_host(Provider::Sandbox, phase), cx);
    fake.reply(read, 200, Some(listing(&[idle("a")], false)));
    test_support::settle(cx);
    assert_eq!(label_of(cx, "prompt").as_deref(), Some("Suspend sandbox?"));
    assert!(
        pending(&mut fake).is_empty(),
        "a cached idle listing needs consent"
    );
}

/// The usual Stop answers describe the tabs after shell exit. The extra answer also names the environment-wide
/// suspend, and its note explains that only the processes still running can resume.
#[gpui::test]
fn a_tab_only_stop_question_helps_with_the_remaining_tabs(cx: &mut TestAppContext) {
    let ended = bash_session("a").status(json!({"state": "exited", "exit_code": 0}));
    let host = environment_host(Provider::Container, connected());
    let (_fake, cx) = open(cx, &[(ended, 1)], false, &host);
    choose(cx, "session-menu-a", "session-menu-stop");
    assert!(
        texts(cx)
            .iter()
            .any(|text| text.starts_with("Close 1 terminal tab?")
                && text.ends_with("The closed tabs stay closed."))
    );
    assert_eq!(
        hover_help(cx, "prompt-confirm").as_deref(),
        Some("Close this session's terminal tabs")
    );
    assert_eq!(
        hover_help(cx, "prompt-cancel").as_deref(),
        Some("Cancel: leave the terminal tabs open")
    );
    assert_eq!(
        hover_help(cx, "prompt-alternative-0").as_deref(),
        Some("Close this session's terminal tabs, then suspend the whole environment")
    );
}

/// Consent names an operation, not a toggle against the latest phase: another viewer may suspend the environment while
/// this window reads its activity, and answering Suspend must never turn into Resume (the old
/// `captured_suspend_and_resume_keep_their_endpoint_after_a_phase_change`). Specified: Suspend is chosen while the
/// sandbox is connected; before the read answers, the host is pushed suspended; the read's answer then asks (the host
/// no longer reads connected), with the question's line "Suspend the whole sandbox? Every process in it freezes,
/// including any other sessions, and continues on Resume. Freeze the sandbox; its processes continue on Resume. Only
/// snapshot storage bills while suspended.", and its Suspend sends `POST /api/hosts/2/suspend`, never `/resume`.
#[gpui::test]
fn a_suspend_answered_after_the_phase_changed_still_suspends(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(idle("a"), 0)], false, &host);
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-suspend");
    let (read, _) = one(&mut fake, Method::Get, "/api/sessions?host=2");
    push_host(
        &mut fake,
        &environment_host(Provider::Sandbox, suspended_at(0)),
        cx,
    );
    fake.reply(read, 200, Some(listing(&[idle("a")], false)));
    test_support::settle(cx);
    assert_eq!(label_of(cx, "prompt").as_deref(), Some("Suspend sandbox?"));
    assert!(texts(cx).iter().any(|text| text
        == "Suspend the whole sandbox? Every process in it freezes, including any other sessions, and continues on \
            Resume. Freeze the sandbox; its processes continue on Resume. Only snapshot storage bills while \
            suspended."));
    test_support::click(cx, "prompt-confirm");
    one(&mut fake, Method::Post, "/api/hosts/2/suspend");
}

/// SPEC.md, "Creation": "Suspend and Resume are not changes to the host registry … Launches on other hosts and host
/// changes elsewhere proceed while one is pending" (the old `tensorlake-sandbox-sessions.spec.ts`, "a held sandbox
/// Suspend and a launch on another host proceed side by side"). A provider can take a while to freeze a sandbox, and a
/// Suspend counted as a launch-excluding change would block New or Launch for all of it. A typed folder lets this
/// draft proceed without waiting for the ordinary folder default. The test observes the create while the Suspend
/// reply is still held.
#[gpui::test]
fn a_suspend_in_flight_leaves_new_available(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(
        cx,
        &[(session("b").host(1), 0), (idle("a"), 0)],
        false,
        &host,
    );
    assert_eq!(super::tests::selected(cx).as_deref(), Some("b"));
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-suspend");
    let (read, _) = one(&mut fake, Method::Get, "/api/sessions?host=2");
    fake.reply(read, 200, Some(listing(&[idle("a")], false)));
    test_support::settle(cx);
    one(&mut fake, Method::Post, "/api/hosts/2/suspend");
    let suspending = cx.update(|_, cx| {
        crate::session::current_operations(cx).is_some_and(|operations| {
            operations
                .read(cx)
                .any_in_flight(|work| matches!(work, crate::operations::Work::Suspend))
        })
    });
    assert!(suspending, "the premise: the Suspend is in flight");

    assert!(
        cx.update(|_, cx| crate::launcher::entry::new_available(cx)),
        "a pending Suspend holds no launch back"
    );
    assert!(
        !cx.update(|_, cx| crate::hosts::host_change_in_flight(cx)),
        "nothing Launch refuses on is in flight"
    );
    test_support::click(cx, super::NEW_ID);
    assert!(launcher_drawn(cx), "New opened the launcher");
    let draft = crate::launcher::view_tests::shown_draft(cx);
    assert_eq!(
        draft.host,
        Some(1),
        "New is on a different host from the Suspend"
    );
    assert_eq!(draft.target, None, "New is on the existing host");

    edit_draft(cx, |draft| {
        draft.choose_tab(Tab::Command);
        draft.edit_folder("/tmp".into());
        draft.edit_command("true".into());
        draft.choose_command_yolo(false);
    });
    let draft = crate::launcher::view_tests::shown_draft(cx);
    assert_eq!(draft.command.command, "true");
    assert_eq!(
        draft.command.yolo,
        Some(false),
        "the command is ready to launch"
    );
    assert_eq!(draft.folder.value(), "/tmp");
    test_support::click(cx, "launcher-launch");
    cx.run_until_parked();
    let sent = fake.commands();
    let creates: Vec<_> = sent
        .iter()
        .filter(|command| command.method == Method::Post && command.path == "/api/sessions")
        .collect();
    let [create] = creates.as_slice() else {
        panic!("expected one create with Suspend still held: {sent:?}");
    };
    let body = create.body.as_ref().expect("the create carries its draft");
    assert_eq!(
        body["host"], 1,
        "the create runs on the connected local host"
    );
    assert_eq!(body["cwd"], "/tmp");
}

// ---- Stop and suspend ----------------------------------------------------------------------------------------------

/// When the shell has exited, the row's Stop and suspend describes closing its tabs and freezing the environment.
/// The confirmation keeps the ended-program guard even though it never asks to stop the shell.
#[gpui::test]
fn stop_and_suspend_from_the_menu_describes_only_the_remaining_tabs(cx: &mut TestAppContext) {
    let ended = bash_session("a").status(json!({"state": "exited", "exit_code": 0}));
    let host = environment_host(Provider::Container, connected());
    let (mut fake, cx) = open(cx, &[(ended.clone(), 1)], false, &host);
    pending(&mut fake);
    test_support::click(cx, "session-menu-a");
    let action = "session-menu-stop-and-suspend";
    assert!(texts(cx).iter().any(
        |text| text == "Close this session's terminal tabs, then suspend the whole environment"
    ));
    let help = "Close this session's terminal tabs, then suspend the whole container";
    assert_eq!(hover_help(cx, action).as_deref(), Some(help));
    test_support::click(cx, action);
    let (read, _) = one(&mut fake, Method::Get, "/api/sessions?host=2");
    fake.reply(read, 200, Some(listing(&[ended], false)));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "prompt").as_deref(),
        Some("Stop and suspend container?")
    );
    assert!(
        texts(cx)
            .iter()
            .any(|text| text.starts_with(help) && text.contains("The closed tabs stay closed."))
    );
    assert_eq!(hover_help(cx, "prompt-confirm").as_deref(), Some(help));
    assert_eq!(
        hover_help(cx, "prompt-cancel").as_deref(),
        Some("Cancel: leave the terminal tabs open and keep the container running")
    );
    assert!(pending(&mut fake).is_empty(), "nothing before an answer");
    test_support::click(cx, "prompt-confirm");
    let (_, body) = one(&mut fake, Method::Post, "/api/sessions/a/stop-and-suspend");
    assert_eq!(
        body,
        json!({"refuse_if_tabs_open": false, "refuse_if_agent_running": true})
    );
}

/// Stop and suspend uses the shown ended agent and tab count if a listing arrives before its click. The activity
/// read can ask a broader question, but a quiet environment sends exactly the guarded Stop and suspend.
#[gpui::test]
fn stop_and_suspend_keeps_its_verb_and_guards_across_a_push(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Container, connected());
    let (mut fake, cx) = open(cx, &[(super::menu_tests::ended("a"), 0)], true, &host);
    pending(&mut fake);
    test_support::click(cx, "session-menu-a");
    assert_eq!(
        hover_help(cx, "session-menu-stop-and-suspend").as_deref(),
        Some("Finish stopping this session, then suspend the whole container"),
        "the ended row lists no tabs"
    );
    test_support::show_next_frame(cx);
    let at = test_support::center(cx, "session-menu-stop-and-suspend");
    let idle = session("a").status(json!({"state": "idle"}));
    let mut change = super::tests::change(super::tests::listing_meta(&["a"], 1, false), &[], &[]);
    change.upsert.push(super::menu_tests::entry(&idle, 1));
    let change = farhelm_helm_client::collections::decode_changes(change).unwrap();
    cx.update(|window, cx| {
        let store = crate::session::current_store(cx).unwrap();
        let shown = store.read(cx).collections().get::<SessionRow>("a").unwrap();
        assert!(shown.info.status.has_ended() && shown.info.tabs.is_empty());
        store.update(cx, |store, cx| store.apply(&change, cx));
        let row = store.read(cx).collections().get::<SessionRow>("a").unwrap();
        assert!(!row.info.status.has_ended() && row.info.tabs.len() == 1);
        test_support::click_before_redraw(at, window, cx);
    });
    let (read, _) = one(&mut fake, Method::Get, "/api/sessions?host=2");
    fake.reply(read, 200, Some(listing(&[idle], false)));
    test_support::settle(cx);
    let (_, body) = one(&mut fake, Method::Post, "/api/sessions/a/stop-and-suspend");
    assert_eq!(
        body,
        json!({"refuse_if_tabs_open": true, "refuse_if_agent_running": true})
    );
}

/// SPEC.md: "The row menu always offers Stop and suspend, so it remains reachable when ordinary confirmations are
/// skipped", and it keeps "the host-wide activity confirmation"; its request carries the Stop guards its click saw,
/// even when the listing refreshes during the read (the old e2e "Stop and suspend sends the guards its click saw").
/// Specified: with confirmations skipped and a running shell listed with no tabs, Stop and suspend reads the host's
/// activity and asks "Stop and suspend sandbox?" The question names the shell and says the other processes freeze.
/// A push meanwhile giving it a tab and an ended shell changes nothing. The answer sends exactly
/// `POST /api/sessions/a/stop-and-suspend` with `{refuse_if_tabs_open: true, refuse_if_agent_running: false}` and no
/// separate Suspend; the helm's 409 shows on a's row as "Stop: <its words>", and the same line below the open
/// session's header.
#[gpui::test]
fn stop_and_suspend_asks_after_its_read_and_sends_the_clicked_guards(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(bash_session("a"), 0)], true, &host);
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-stop-and-suspend");
    let (read, _) = one(&mut fake, Method::Get, "/api/sessions?host=2");
    super::menu_tests::push(
        &mut fake,
        &["a"],
        &[(
            bash_session("a").status(json!({"state": "exited", "exit_code": 0})),
            1,
        )],
        cx,
    );
    fake.reply(read, 200, Some(listing(&[bash_session("a")], false)));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "prompt").as_deref(),
        Some("Stop and suspend sandbox?")
    );
    assert!(texts(cx).iter().any(|text| text.starts_with(
        "Stop this session's shell and close its terminal tabs, then suspend the whole sandbox? Every other process \
         freezes and continues on Resume. The stopped shell and closed tabs do not continue."
    )), "the question names the shell and the processes that can resume");
    assert!(pending(&mut fake).is_empty(), "nothing before the answer");
    test_support::click(cx, "prompt-confirm");
    let (id, body) = one(&mut fake, Method::Post, "/api/sessions/a/stop-and-suspend");
    assert_eq!(
        body,
        json!({"refuse_if_tabs_open": true, "refuse_if_agent_running": false})
    );
    fake.reply(id, 409, Some(json!("this session has terminal tabs open")));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("Stop: this session has terminal tabs open")
    );
    assert_eq!(
        label_of(cx, "session-header-outcome-a").as_deref(),
        Some("Stop: this session has terminal tabs open"),
        "the open session's header shows the same refusal"
    );
}

/// The helm's wording is deliberately noncommittal about whether the work still runs. Keep the fixture long enough to
/// exercise line wrapping in the sidebar and the menu as well as the selected session.
pub(crate) const STOP_AND_SUSPEND_NOTICE: &str = "Stop and suspend did not finish within 20 seconds and may or may not still be running. \
     The environment suspends only if Stop finishes cleanly. Check the session list for the result.";

/// Send through the Stop question's alternate answer path. The other tests exercise opening and answering the question.
fn send_stop_and_suspend(fake: &mut FakeHelm, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| {
        super::environment::stop_and_suspend("a", 2, Default::default(), cx);
    });
    test_support::settle(cx);
    one(fake, Method::Post, "/api/sessions/a/stop-and-suspend").0
}

/// Pin the stored outcome separately from what's drawn. A hidden notice could otherwise reappear on Resume.
fn stop_and_suspend_line(cx: &mut VisualTestContext) -> Option<crate::operations::Failure> {
    cx.update(|_, cx| super::outcome::current_line("a", cx))
}

/// A deadline arriving while the host is still connected must explain the unknown outcome on each surface. The
/// host's pushed suspension retires the record, including when the user subsequently resumes from Hosts.
#[gpui::test]
fn a_marked_stop_and_suspend_timeout_shows_until_the_host_suspends(cx: &mut TestAppContext) {
    let mut host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(bash_session("a"), 0)], true, &host);
    pending(&mut fake);
    let id = send_stop_and_suspend(&mut fake, cx);
    cx.update(|_, cx| {
        let environment = super::environment::of_key("a", cx).expect("the session has its host");
        assert!(matches!(
            environment.host.state,
            HostStateView::Connected { .. }
        ));
    });
    fake.reply_marked(
        id,
        504,
        vec![HelmMarker::StopAndSuspendReplyBound],
        Some(json!(STOP_AND_SUSPEND_NOTICE)),
    );
    test_support::settle(cx);
    assert_eq!(
        stop_and_suspend_line(cx).unwrap().tone,
        crate::operations::Tone::Notice { host: 2 }
    );
    for id in ["session-outcome-a", "session-header-outcome-a"] {
        assert_eq!(label_of(cx, id).as_deref(), Some(STOP_AND_SUSPEND_NOTICE));
    }
    test_support::click(cx, "session-menu-a");
    assert_eq!(
        label_of(cx, "session-menu-outcome-a").as_deref(),
        Some(STOP_AND_SUSPEND_NOTICE)
    );
    test_support::keys(cx, "escape");
    host.state = suspended_at(0);
    push_host(&mut fake, &host, cx);
    assert!(
        stop_and_suspend_line(cx).is_none(),
        "suspension clears the record"
    );

    test_support::click(cx, "hosts-panel-toggle");
    test_support::click(cx, "host-resume-2");
    let (id, _) = one(&mut fake, Method::Post, "/api/hosts/2/resume");
    fake.reply(id, 200, None);
    host.state = connected();
    push_host(&mut fake, &host, cx);
    assert!(
        stop_and_suspend_line(cx).is_none(),
        "a host action cannot reveal an old notice"
    );
}

/// The host push can beat the deadline reply. A notice is already obsolete in either settling phase, so none may
/// survive on a settled host. This pins that outcome, not the mechanism: the reply's own check and the session list's
/// clearing of settled notices each prevent it, so the test passes with either alone.
#[gpui::test]
fn a_marked_reply_does_not_create_a_notice_on_a_settled_host(cx: &mut TestAppContext) {
    let mut host = environment_host(Provider::Container, connected());
    let (mut fake, cx) = open(cx, &[(bash_session("a"), 0)], true, &host);
    pending(&mut fake);
    for phase in [suspended_at(0), deleting(None)] {
        host.state = connected();
        push_host(&mut fake, &host, cx);
        let id = send_stop_and_suspend(&mut fake, cx);
        host.state = phase;
        push_host(&mut fake, &host, cx);
        fake.reply_marked(
            id,
            504,
            vec![HelmMarker::StopAndSuspendReplyBound],
            Some(json!(STOP_AND_SUSPEND_NOTICE)),
        );
        test_support::settle(cx);
        assert!(stop_and_suspend_line(cx).is_none(), "{:?}", host.state);
    }
}

/// The deletion push clears the stored line even while the host is still listed. Starting a new session action
/// clears it earlier, at admission, so an unanswered request cannot leave the old outcome on screen.
#[gpui::test]
fn deletion_and_the_next_session_action_clear_stop_and_suspend_notices(cx: &mut TestAppContext) {
    let mut host = environment_host(Provider::Container, connected());
    let (mut fake, cx) = open(cx, &[(bash_session("a"), 0)], true, &host);
    pending(&mut fake);
    let id = send_stop_and_suspend(&mut fake, cx);
    fake.reply_marked(
        id,
        504,
        vec![HelmMarker::StopAndSuspendReplyBound],
        Some(json!(STOP_AND_SUSPEND_NOTICE)),
    );
    test_support::settle(cx);
    assert!(
        stop_and_suspend_line(cx).is_some(),
        "premise: the first notice is stored"
    );

    let id = send_stop_and_suspend(&mut fake, cx);
    assert!(
        stop_and_suspend_line(cx).is_none(),
        "a new action has its own outcome"
    );
    fake.reply_marked(
        id,
        504,
        vec![HelmMarker::StopAndSuspendReplyBound],
        Some(json!(STOP_AND_SUSPEND_NOTICE)),
    );
    test_support::settle(cx);
    assert!(
        stop_and_suspend_line(cx).is_some(),
        "premise: the new notice is stored"
    );
    host.state = deleting(None);
    push_host(&mut fake, &host, cx);
    assert!(
        stop_and_suspend_line(cx).is_none(),
        "deleting clears the record"
    );
}

/// A supervisor timeout also answers 504. Its words cannot make it a notice, even if they resemble the helm's copy.
#[gpui::test]
fn an_unmarked_stop_and_suspend_timeout_is_a_stop_failure(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(bash_session("a"), 0)], true, &host);
    pending(&mut fake);
    let id = send_stop_and_suspend(&mut fake, cx);
    fake.reply(id, 504, Some(json!(STOP_AND_SUSPEND_NOTICE)));
    test_support::settle(cx);
    assert_eq!(
        stop_and_suspend_line(cx).unwrap().tone,
        crate::operations::Tone::Danger
    );
    let expected = format!("Stop: {STOP_AND_SUSPEND_NOTICE}");
    for id in ["session-outcome-a", "session-header-outcome-a"] {
        assert_eq!(label_of(cx, id).as_deref(), Some(expected.as_str()));
    }
    test_support::click(cx, "session-menu-a");
    assert_eq!(
        label_of(cx, "session-menu-outcome-a").as_deref(),
        Some(expected.as_str())
    );
}

/// The old Stop question of an environment session said what plain Stop leaves running and offered Stop and suspend as
/// a third answer, so a user who meant to stop billing need not go back to the menu. Specified: Stop of a sandbox
/// shell with one tab asks, its consequence reading "Stop the shell and close 1 terminal tab? The sandbox keeps
/// running and billing until you suspend or delete it. Stop and suspend freezes the remaining processes; they continue
/// on Resume. The stopped shell and closed tabs do not continue."; its Stop and suspend answer sends
/// `POST /api/sessions/a/stop-and-suspend` with the question's guards and reads nothing first.
#[gpui::test]
fn an_environment_sessions_stop_question_offers_stop_and_suspend(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(bash_session("a"), 1)], false, &host);
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-stop");
    assert!(texts(cx).iter().any(|text| text
        == "Stop the shell and close 1 terminal tab? The sandbox keeps running and billing until you suspend or \
            delete it. Stop and suspend freezes the remaining processes; they continue on Resume. The stopped shell \
            and closed tabs do not continue."));
    assert_eq!(
        label_of(cx, "prompt-alternative-0").as_deref(),
        Some("Stop and suspend")
    );
    test_support::click(cx, "prompt-alternative-0");
    let (_, body) = one(&mut fake, Method::Post, "/api/sessions/a/stop-and-suspend");
    assert_eq!(
        body,
        json!({"refuse_if_tabs_open": false, "refuse_if_agent_running": false})
    );
}

/// A launch can target the same machine that runs a session about to stop. Keep ordinary Stop available while
/// withholding the extra answer that would freeze the launch's host.
#[gpui::test]
fn a_launch_holds_its_hosts_stop_and_suspend_answer(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, connected());
    let (mut fake, cx) = open(cx, &[(bash_session("a"), 1)], false, &host);
    pending(&mut fake);
    let create = crate::launcher::launch_tests::launch_to(&mut fake, cx, host.id);
    assert_eq!(
        create.body.unwrap()["host"],
        host.id,
        "the create targets this session's host"
    );
    choose(cx, "session-menu-a", "session-menu-stop");
    assert_eq!(
        hover_help(cx, "prompt-alternative-0").as_deref(),
        Some("A launch on this host is in progress; try again once it finishes")
    );
    test_support::click(cx, "prompt-alternative-0");
    assert_eq!(
        label_of(cx, "prompt-confirm").as_deref(),
        Some("Confirm stop")
    );
    // The create is still first in the fake's reply queue. Set aside any unrelated requests until it answers.
    let before_stop = fake.commands();
    assert!(
        before_stop
            .iter()
            .all(|command| !is_environment(&command.path)),
        "the disabled answer sent nothing"
    );
    test_support::click(cx, "prompt-confirm");
    let after_stop = fake.commands();
    assert_eq!(
        after_stop
            .iter()
            .filter(|command| is_environment(&command.path))
            .map(|command| (command.method, command.path.as_str()))
            .collect::<Vec<_>>(),
        [(Method::Post, "/api/sessions/a/stop")],
    );
    fake.reply(create.id, 409, Some(json!("The host refused the launch")));
    for command in before_stop.into_iter().chain(after_stop) {
        fake.reply(command.id, 200, None);
    }
    test_support::settle(cx);
}

// ---- Pending and refused --------------------------------------------------------------------------------------------

/// The helm answers Resume within 20 seconds and a Tensorlake Resume routinely takes longer, so the 504 that says it
/// carries on is routine: shown as a failure, it would tell the user to try again while the resume is under way.
/// Specified: Resume from a suspended sandbox's menu sends `POST /api/hosts/2/resume` at once, reading nothing; the row
/// shows "Resuming…"; the helm's 504 leaves no line on the row, and with the host pushed resuming the row still says
/// "Resuming…"; once the host is connected it says nothing more; a later Resume refused with a 409 shows on the row as
/// "Resume: <its words>".
#[gpui::test]
fn a_resume_past_the_bound_stays_pending_and_a_refusal_shows(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, suspended_at(0));
    let (mut fake, cx) = open(cx, &[(session("a").stale(), 0)], false, &host);
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-resume");
    let (resume, _) = one(&mut fake, Method::Post, "/api/hosts/2/resume");
    assert_eq!(
        label_of(cx, "session-environment-progress-a").as_deref(),
        Some("Resuming…")
    );
    fake.reply(
        resume,
        504,
        Some(json!(
            "resuming the host did not finish within 20 seconds; it carries on"
        )),
    );
    test_support::settle(cx);
    assert!(!drawn(cx, "session-outcome-a"), "the 504 is not a failure");
    push_host(
        &mut fake,
        &environment_host(Provider::Sandbox, changing(LifecycleDirection::Resuming)),
        cx,
    );
    assert_eq!(
        label_of(cx, "session-environment-progress-a").as_deref(),
        Some("Resuming…")
    );
    push_host(
        &mut fake,
        &environment_host(Provider::Sandbox, connected()),
        cx,
    );
    assert!(!drawn(cx, "session-environment-progress-a"));

    push_host(
        &mut fake,
        &environment_host(Provider::Sandbox, suspended_at(0)),
        cx,
    );
    choose(cx, "session-menu-a", "session-menu-resume");
    let (resume, _) = one(&mut fake, Method::Post, "/api/hosts/2/resume");
    fake.reply(resume, 409, Some(json!("Tensorlake refused the resume")));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("Resume: Tensorlake refused the resume")
    );
}

/// A container's Suspend and Resume take the same path at the helm as a sandbox's (`environment::lifecycle::perform`),
/// which pushes the host's changing phase (the wire's `tensorlake-changing`, whatever the provider) while the change
/// runs, so a 504 at the bound must leave a container session looking as busy as a sandbox session does: shown idle,
/// the user would Delete or Suspend again under a pause still running. Specified: Suspend of idle a on a connected
/// container reads the host's activity (complete and idle, so it asks nothing) and sends `POST /api/hosts/2/suspend`;
/// the helm's 504 leaves no line on the row; with the host pushed suspending the row says "Suspending…" and the menu's
/// Delete ignores a click; once the host is pushed suspended the row shows its Suspended badge and no pending mark.
#[gpui::test]
fn a_container_suspend_past_the_bound_stays_pending_and_held(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Container, connected());
    let (mut fake, cx) = open(cx, &[(idle("a"), 0)], false, &host);
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-suspend");
    let (read, _) = one(&mut fake, Method::Get, "/api/sessions?host=2");
    fake.reply(read, 200, Some(listing(&[idle("a")], false)));
    test_support::settle(cx);
    let (suspend, _) = one(&mut fake, Method::Post, "/api/hosts/2/suspend");
    fake.reply(
        suspend,
        504,
        Some(json!(
            "suspending the host did not finish within 20 seconds; it carries on"
        )),
    );
    test_support::settle(cx);
    assert!(!drawn(cx, "session-outcome-a"), "the 504 is not a failure");
    push_host(
        &mut fake,
        &environment_host(
            Provider::Container,
            changing(LifecycleDirection::Suspending),
        ),
        cx,
    );
    assert_eq!(
        label_of(cx, "session-environment-progress-a").as_deref(),
        Some("Suspending…")
    );
    choose(cx, "session-menu-a", "session-menu-delete");
    assert!(
        drawn(cx, "session-menu"),
        "Delete is held while the pause runs"
    );
    test_support::keys(cx, "escape");
    push_host(
        &mut fake,
        &environment_host(Provider::Container, suspended_at(0)),
        cx,
    );
    assert!(!drawn(cx, "session-environment-progress-a"));
    assert_eq!(
        label_of(cx, "session-row-a-suspended").as_deref(),
        Some("Suspended")
    );
}

// ---- Being deleted ---------------------------------------------------------------------------------------------------

/// The words a sandbox session gives for every action being unavailable while its sandbox is being deleted.
const SANDBOX_DELETING: &str =
    "This session's sandbox is being deleted, so its actions are unavailable";

/// SPEC.md, "Sandbox sessions": while the helm destroys an environment, "its sessions show as being deleted with their
/// actions unavailable". The generic stale cause ("Host being deleted") says nothing about the actions, so the old row
/// put the host's phase in its place with why as its help. Specified: a stale session on a sandbox being deleted draws,
/// in place of its cause, a mark named "Deleting…" helped by "This session's sandbox is being deleted, so its actions
/// are unavailable"; with the host a container whose deletion failed, the mark is named "Deleting failed" and helped by
/// "Deleting this session's container failed, so its actions are unavailable. Retry in Hosts finishes the deletion";
/// with the host connected again, no mark is drawn and the cause ("Refreshing session") is back.
#[gpui::test]
fn a_row_being_deleted_shows_the_phase_in_place_of_its_cause(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, deleting(None));
    let (mut fake, cx) = open(cx, &[(session("a").stale(), 0)], false, &host);
    pending(&mut fake);
    assert_eq!(
        label_of(cx, "session-row-a-deleting").as_deref(),
        Some("Deleting…")
    );
    assert_eq!(
        hover_help(cx, "session-row-a-deleting").as_deref(),
        Some(SANDBOX_DELETING)
    );
    assert!(
        !drawn(cx, "session-row-a-cause"),
        "the phase replaces the cause"
    );

    let failed = environment_host(Provider::Container, deleting(Some("podman rm failed")));
    push_host(&mut fake, &failed, cx);
    // The pointer leaves the mark first: help already showing stays as it was shown until the pointer moves on.
    hover_help(cx, "session-row-a-host");
    assert_eq!(
        label_of(cx, "session-row-a-deleting").as_deref(),
        Some("Deleting failed")
    );
    assert_eq!(
        hover_help(cx, "session-row-a-deleting").as_deref(),
        Some(
            "Deleting this session's container failed, so its actions are unavailable. Retry in Hosts finishes the \
             deletion"
        )
    );

    push_host(
        &mut fake,
        &environment_host(Provider::Container, connected()),
        cx,
    );
    assert!(!drawn(cx, "session-row-a-deleting"));
    assert_eq!(
        label_of(cx, "session-row-a-cause").as_deref(),
        Some("Refreshing session")
    );
}

/// SPEC.md, "Sandbox sessions": a session whose environment is being deleted has its actions unavailable, Delete and
/// Clone included. The helm is destroying the environment, so a Delete would be a doomed request and a Clone would copy
/// a session on its way out; the old menu disabled every command and said why in Clone's and Delete's descriptions.
/// Specified: with idle a on a sandbox being deleted, the menu lists Rename, Mark unread, Pin to top, Mute, Suspend, Stop and
/// suspend, Clone, Replace with…, Replace, Stop and Delete, and a click on each leaves the menu open and opens nothing
/// (no question, no launcher); Clone's and Delete's descriptions both read "This session's sandbox is being deleted, so
/// its actions are unavailable"; nothing at all is sent; once the host is connected, the menu's Delete asks again.
#[gpui::test]
fn every_menu_command_is_refused_while_the_environment_is_being_deleted(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, deleting(None));
    let (mut fake, cx) = open(cx, &[(idle("a"), 0)], false, &host);
    pending(&mut fake);
    test_support::click(cx, "session-menu-a");
    let items = [
        "session-menu-rename",
        "session-menu-seen",
        "session-menu-pin",
        "session-menu-group",
        "session-menu-mute",
        "session-menu-suspend",
        "session-menu-stop-and-suspend",
        "session-menu-clone",
        "session-menu-replace-with",
        "session-menu-replace",
        "session-menu-stop",
        "session-menu-delete",
    ];
    assert_eq!(menu_items(cx), items, "the premise: the menu is open");
    assert_eq!(
        texts(cx)
            .iter()
            .filter(|text| *text == SANDBOX_DELETING)
            .count(),
        2,
        "Clone's and Delete's descriptions give the reason"
    );
    for item in items {
        test_support::click(cx, item);
        assert!(drawn(cx, "session-menu"), "{item} does nothing");
        assert!(!drawn(cx, "prompt"), "{item} asks nothing");
        assert!(
            !drawn(cx, "session-menu-mute-hour"),
            "the disabled Mute cannot open its child"
        );
        assert!(
            !super::tests::launcher_drawn(cx),
            "{item} opens no launcher"
        );
    }
    let sent: Vec<_> = fake
        .commands()
        .into_iter()
        .map(|command| (command.method, command.path))
        .collect();
    assert_eq!(sent, [], "nothing was sent");
    test_support::keys(cx, "escape");

    push_host(
        &mut fake,
        &environment_host(Provider::Sandbox, connected()),
        cx,
    );
    choose(cx, "session-menu-a", "session-menu-delete");
    assert!(drawn(cx, "prompt"), "Delete asks again");
}

// ---- The rules, without a window -------------------------------------------------------------------------------------

/// The activity rule Hosts' own Suspend will share: a stale, unknown or running status says nothing about whether the
/// agent is idle now, so a listing holding one asks first. Specified: stale anything asks; fresh Unknown and Running
/// ask; fresh Waiting, Idle, Exited, Error and Interrupted do not.
#[test]
fn only_conclusive_fresh_statuses_let_suspend_go_ahead() {
    for status in [SessionStatus::Unknown, SessionStatus::Running] {
        assert!(needs_confirmation(&status, false), "{status:?}");
    }
    for status in [
        SessionStatus::Waiting,
        SessionStatus::Idle,
        SessionStatus::Exited { exit_code: Some(0) },
        SessionStatus::Error { detail: "x".into() },
        SessionStatus::Interrupted,
    ] {
        assert!(!needs_confirmation(&status, false), "{status:?}");
        assert!(needs_confirmation(&status, true), "stale {status:?}");
    }
}

/// The card's diagnostics name who suspended the environment from the recorded completion, never an inferred timeout,
/// and keep a provider's failed resume first. Specified, for both providers (word for word the old
/// `suspended_detail`): a sandbox suspended outside Farhelm reads "Suspended outside Farhelm, noticed at <stamp>.
/// Tensorlake suspends this sandbox after 30 minutes without a connection from Farhelm."; one Farhelm suspended reads
/// "Suspended from Farhelm at <stamp>." with Tensorlake's failed resume first; a container reads "Suspension noticed at
/// <stamp>." or "Resume failed: <reason>. Suspended from Farhelm at <stamp>.", each stamp in the viewer's zone (UTC
/// here); and the host names read "dev sandbox",
/// "dev container on Build machine", or "host 7" for a place the helm did not name.
#[test]
fn the_suspended_detail_and_the_names_are_the_providers() {
    use farhelm_proto::ProviderKind;
    let detail = |kind, by: Option<i64>, failed: Option<&str>| {
        ProviderUi(kind)
            .suspended_detail(HostSentence::new("detail"), 0, by, failed, Zone::UTC)
            .accessible_name()
            .to_string()
    };
    assert_eq!(
        detail(ProviderKind::Tensorlake, None, None),
        "Suspended outside Farhelm, noticed at 1970-01-01 00:00:00. Tensorlake suspends this sandbox after 30 \
         minutes without a connection from Farhelm."
    );
    assert_eq!(
        detail(ProviderKind::Tensorlake, Some(60_000), Some("quota")),
        "Tensorlake could not resume the sandbox: quota. Suspended from Farhelm at 1970-01-01 00:01:00."
    );
    assert_eq!(
        detail(ProviderKind::Podman, None, None),
        "Suspension noticed at 1970-01-01 00:00:00."
    );
    assert_eq!(
        detail(ProviderKind::Podman, Some(0), Some("paused")),
        "Resume failed: paused. Suspended from Farhelm at 1970-01-01 00:00:00."
    );
    let sandbox = LaunchTarget::Environment {
        recipe: "dev".into(),
        place: Place::TensorlakeAccount,
    };
    let container = LaunchTarget::Environment {
        recipe: "dev".into(),
        place: Place::PodmanHost(7),
    };
    assert_eq!(target_label(&sandbox, None), "dev sandbox");
    assert_eq!(
        target_label(&container, Some("Build machine")),
        "dev container on Build machine"
    );
    assert_eq!(target_label(&container, None), "dev container on host 7");
}

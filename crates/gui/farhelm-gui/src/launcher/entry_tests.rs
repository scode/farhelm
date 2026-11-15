//! Tests of the launcher's New entry points in the real main view over the helm client's fake: New's prefill from the
//! open session, New named's name, Cmd+N's action, and New refused while a launch is pending or a host change is in
//! flight.
//!
//! The fleet: host1 (the helm's own host, kind `local`), host9 (a connected ssh host recorded as install `identity9`)
//! and host11 (a sandbox a session owns), with session `a` on host9's install, `b` on host9's row but listed under an
//! install the row no longer names, and `c` on host11.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::LaunchPermission;
use farhelm_proto::ui_channel::{CollectionState, Preferences};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::draft::{LaunchDraft, Tab};
use super::entry::{new_available, open_new, open_new_named};
use super::launch_tests::{HostChange, creates, start_host_change};
use super::view_tests::{Opened, draft, fixture_hosts, shown_refusal};
use crate::actions::NewSession;
use crate::local_state;
use crate::main_view::MainView;
use crate::session_list::tests::{listing, preferences_state, session};
use crate::test_support::{self, any_open, settle, started};

/// The sessions: `a` on host9 as its install `identity9`, `b` on host9's row as the install `old-install` it no longer
/// names, `c` on the sandbox host11, and `d` on host5 as its recorded install `identity5`, which host5 now contradicts
/// (its state is an identity mismatch).
fn fleet() -> CollectionState {
    let mut state = listing(
        &["a", "b", "c", "d"],
        &[
            session("a").host(9),
            session("b").host(9),
            session("c").host(11),
            session("d").host(5),
        ],
    );
    let identities = ["identity9", "old-install", "identity11", "identity5"];
    for (entry, identity) in state.entries.iter_mut().zip(identities) {
        entry.value["host_identity"] = json!(identity);
    }
    state
}

/// The main view over the fake helm with hosts 1, 9 and 11 and `yolo` as the remembered permission: with [`fleet`] and
/// `selected` selected, or, for `None`, with no sessions at all, since the selection rules open a listed session by
/// themselves and "no open session" needs nothing to open.
fn window_with<'a>(
    cx: &'a mut TestAppContext,
    selected: Option<&str>,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut helm, _connection) = started(cx);
    let sessions = if selected.is_some() {
        fleet()
    } else {
        listing(&[], &[])
    };
    helm.connect(vec![
        sessions,
        fixture_hosts(&[0, 4, 8, 10]),
        preferences_state(Preferences {
            remembered_permissions: Some("yolo".into()),
            ..Preferences::default()
        }),
    ]);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, MainView::new);
    if let Some(key) = selected {
        window.update(|_, cx| {
            local_state::select(key, cx);
        });
    }
    settle(window);
    (helm, window)
}

/// Run New in the window, as its button's handler would, and settle.
fn new(cx: &mut VisualTestContext) -> bool {
    let opened = cx.update(open_new);
    settle(cx);
    opened
}

/// The launcher opened last, by its entity id.
fn opened(cx: &mut VisualTestContext) -> Option<gpui::EntityId> {
    cx.update(|_, cx| {
        cx.try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade())
            .map(|launcher| launcher.entity_id())
    })
}

/// The session the window has open.
fn selected(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| local_state::selected(cx))
}

/// The host and folder the open launcher's draft holds.
fn destination(cx: &mut VisualTestContext) -> (Option<i64>, String) {
    let draft: LaunchDraft = draft(cx);
    (draft.host, draft.folder.value().clone())
}

/// SPEC.md, "Creation": the host "defaults to the host of the currently open session", and the folder to "the selected
/// session's directory when that session is on the target host's installation". New's whole point is to start next to
/// what the user is looking at. Specified: with a open on host9's install, New opens on host9 in a's folder, with the
/// remembered permission, and nothing touched.
#[gpui::test]
fn new_opens_beside_the_open_session(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("a"));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    assert!(new(cx), "New opened");
    assert_eq!(destination(cx), (Some(9), "/home/user/src/a".to_owned()));
    let draft = draft(cx);
    assert_eq!(
        draft.agent.permissions,
        Some(LaunchPermission::Yolo),
        "with the remembered permission"
    );
    assert!(
        !draft.folder.is_touched(),
        "a prefilled folder is a seed, not an edit"
    );
}

/// SPEC.md, "Host": with no open session, New uses "the helm's own host"; "Working directory": else "`~`, which each
/// host expands to its own home". Specified: nothing selected, New opens on host1 (the local row) in `~`.
#[gpui::test]
fn new_without_an_open_session_starts_on_the_helms_own_host(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, None);
    assert_eq!(
        cx.update(|_, cx| local_state::selected(cx)),
        None,
        "the premise: no session is open"
    );
    assert!(new(cx));
    assert_eq!(destination(cx), (Some(1), "~".to_owned()));
}

/// SPEC.md, "Host": "a row retargeted or adopted onto a different install after the session was selected falls back to
/// the helm's own host rather than silently aiming the create at the successor". Aiming at the successor would start a
/// session on a machine the user never chose. Specified: with b open (listed under an install host9's row no longer
/// names), New opens on host1 in `~`, not on host9 in b's folder.
#[gpui::test]
fn new_does_not_follow_a_retargeted_host_row(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("b"));
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    assert!(new(cx));
    assert_eq!(destination(cx), (Some(1), "~".to_owned()));
}

/// A sandbox a session owns is no place for an unrelated session, and the host picker leaves such hosts out, so New
/// from an open sandbox session falls back to the helm's own host until the environments work brings SPEC.md's recipe
/// preselection. Specified: with c open on host11, New opens on host1 in `~`.
#[gpui::test]
fn new_from_a_sandbox_session_starts_on_the_helms_own_host(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("c"));
    assert_eq!(selected(cx).as_deref(), Some("c"), "the premise: c is open");
    assert!(new(cx));
    assert_eq!(destination(cx), (Some(1), "~".to_owned()));
}

/// SPEC.md, the quick switcher's "New session named X": "Picking it opens New with that name and the default launch
/// target and directory (Creation), plus remembered permission defaults." Specified: New named "fix-login" with a open
/// opens New beside a with that name, entered as a hand edit.
#[gpui::test]
fn new_named_opens_new_with_the_name(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("a"));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    assert!(cx.update(|window, cx| open_new_named("fix-login", window, cx)));
    settle(cx);
    let draft = draft(cx);
    assert_eq!(draft.name.value(), "fix-login");
    assert!(draft.name.is_touched(), "the name counts as typed");
    assert_eq!(destination(cx), (Some(9), "/home/user/src/a".to_owned()));
}

/// SPEC.md, "Desktop window chrome": "Cmd+N in the Mac desktop app opens New with the same prefill ... unless a modal is
/// open". The chord is bound on the Mac only, so this dispatches its action, which is what the binding sends.
/// Specified: the action opens New; dispatched again while New is open, it opens nothing more.
#[gpui::test]
fn the_new_session_action_opens_new_unless_a_dialog_is_open(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, None);
    assert!(!any_open(cx), "the premise: nothing is open");
    cx.dispatch_action(NewSession);
    settle(cx);
    assert!(any_open(cx), "the action opened New");
    let first = opened(cx);
    cx.dispatch_action(NewSession);
    settle(cx);
    assert_eq!(
        opened(cx),
        first,
        "a second New did not open over the first"
    );
}

/// SPEC.md, "Creation": while a launch is pending "the same launcher remains busy and cannot accept a second launch or be
/// replaced with another draft", and Cmd+N works "unless ... New is disabled". Specified: after a press of Launch sends
/// a create, New is unavailable and opens nothing; once the helm refuses the create after the user moved on, New is
/// available again and brings back that launcher with its refusal instead of a fresh one.
#[gpui::test]
fn new_waits_for_a_pending_launch_and_then_brings_back_its_refusal(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, None);
    assert!(new(cx));
    cx.update(|_, cx| {
        let launcher = cx
            .try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade());
        launcher
            .expect("New opened a launcher")
            .update(cx, |launcher, cx| {
                launcher.draft.choose_tab(Tab::Command);
                launcher.draft.edit_command("true".into());
                launcher.draft.choose_command_yolo(false);
                cx.notify();
            });
    });
    settle(cx);
    test_support::click(cx, "launcher-launch");
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("the press sent one create, not {}", sent.len());
    };
    let create = create.id;
    assert!(!any_open(cx), "the premise: the launcher hid");
    assert!(
        !cx.update(|_, cx| new_available(cx)),
        "New is unavailable while the create is out"
    );
    assert!(!new(cx), "and opens nothing");
    assert!(!any_open(cx));
    test_support::keys(cx, "x");
    helm.reply(create, 409, Some(json!("host1 is not connected")));
    settle(cx);
    assert!(
        cx.update(
            |_, cx| crate::notices::posted(cx).contains(&crate::notices::Notice::LaunchNotStarted)
        ),
        "the premise: the refusal waits behind its notice"
    );
    assert!(!any_open(cx), "and nothing opened");
    assert!(
        cx.update(|_, cx| new_available(cx)),
        "New is available again"
    );
    assert!(new(cx), "New opened");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. host1 is not connected"),
        "New brought back the refused launcher, not a fresh one"
    );
}

/// SPEC.md, "Host": New aims at the install the user was looking at. A host whose reported identity contradicts its
/// recorded one is waiting for the user to decide which install it is, so it cannot be that install yet, whatever its
/// registry row still records (the old app's comparison excluded it too). Specified: with d open on host5, in an
/// identity mismatch, New opens on host1 in `~`.
#[gpui::test]
fn new_does_not_follow_a_host_in_an_identity_mismatch(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("d"));
    assert_eq!(selected(cx).as_deref(), Some("d"), "the premise: d is open");
    assert!(new(cx));
    assert_eq!(destination(cx), (Some(1), "~".to_owned()));
}

/// SPEC.md, "Working directory": the folder is prefilled "for the target host", and "Changing the host re-seeds a
/// folder the user has not typed or picked". So the open session's folder belongs to its host, not to the draft: it
/// goes when another host is picked and comes back when its own is picked again. Specified: with a open, New opens on
/// host9 in a's folder; picking host1 re-seeds `~`; picking host9 again brings a's folder back.
#[gpui::test]
fn picking_a_host_re_seeds_its_own_folder(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("a"));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    assert!(new(cx));
    assert_eq!(destination(cx), (Some(9), "/home/user/src/a".to_owned()));
    test_support::click(cx, "launcher-host");
    test_support::click(cx, "launcher-host-option-1");
    assert_eq!(destination(cx), (Some(1), "~".to_owned()));
    test_support::click(cx, "launcher-host");
    test_support::click(cx, "launcher-host-option-9");
    assert_eq!(destination(cx), (Some(9), "/home/user/src/a".to_owned()));
}

/// New is unavailable while a host change is in flight, as the old New button was disabled while the page's one
/// operation token was held by a host change (SPEC.md, "Creation": "Launches and changes to the host registry still
/// exclude each other"); a probe counts by the migration's orchestrator's decision, though the old token left it out. Specified: with a host probe in flight New is unavailable and opens nothing; once it is answered New opens.
#[gpui::test]
fn new_waits_for_a_host_change(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, None);
    let path = start_host_change(cx, HostChange::Probe);
    cx.run_until_parked();
    let probe = helm
        .commands()
        .into_iter()
        .find(|command| command.path == path)
        .expect("the premise: the probe is in flight");
    assert!(
        !cx.update(|_, cx| new_available(cx)),
        "New is unavailable while the probe is out"
    );
    assert!(!new(cx), "and opens nothing");
    helm.reply(probe.id, 200, Some(json!({})));
    settle(cx);
    assert!(new(cx), "with the probe answered, New opens");
}

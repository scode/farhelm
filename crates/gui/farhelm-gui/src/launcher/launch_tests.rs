//! Tests of launching from the launcher, in the real main view over the helm client's fake: the one create a press of
//! Launch sends, the launcher hiding while it is out, and what its reply opens or brings back, with and without the user
//! having moved on.
//!
//! Every test launches a Command tab draft (`true`, not YOLO, on host1 in `~`), the smallest complete draft: what is
//! under test is the submit path, which every complete draft shares. The fake answers commands only when told, so each
//! test holds the create's reply until it has set up the moment the reply arrives in.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{CollectionChanges, CollectionState, HostView};
use gpui::{Keystroke, VisualTestContext};
use serde_json::json;

use super::draft::{LaunchDraft, Tab};
use super::view_tests::{launcher_on, name_of, read, shown_refusal};
use crate::local_state;
use crate::notices::{self, Notice};
use crate::session_list::tests::{listing, session};
use crate::test_support::{self, any_open, keys, settle};

/// The smallest complete draft: a Command tab launch of `true`, asserted not YOLO, on host1 in `~`. The session pane's
/// tests launch it too, for a first launch over an empty fleet.
pub(crate) fn command_draft() -> LaunchDraft {
    let mut draft = LaunchDraft::new(Some(1), "~".into(), None);
    draft.choose_tab(Tab::Command);
    draft.edit_command("true".into());
    draft.choose_command_yolo(false);
    draft
}

/// The creates (`POST /api/sessions`) the window sent since the last call. Every other command (the catalog read, the
/// selection remembered) is answered at once, so the fake's in-order replies stay possible while a create waits; the
/// catalog read is answered with an empty catalog ([`creates_answering`]).
pub(super) fn creates(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    creates_answering(helm, cx, &json!([]))
}

/// The launch history of a host that has launched nothing yet, as the helm answers it.
fn empty_history() -> serde_json::Value {
    json!({ "launches": [], "folders": [], "checkout_config_revision": 1 })
}

/// [`creates`], answering the catalog read with `catalog` and every launch-history read with [`empty_history`], round
/// after round until a round sends nothing but creates, so a create that waited for an answer (a quick launch's
/// catalog read, or New's folder default, which settles on `~` from that empty history) is among them. The templates
/// area's tests depend on both answers.
pub(crate) fn creates_answering(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    catalog: &serde_json::Value,
) -> Vec<FakeCommand> {
    creates_answering_with(helm, cx, catalog, &empty_history()).0
}

/// [`creates_answering`] with every launch-history read answered with `history`: the creates, left unanswered, and how
/// many history reads the window made, for tests that check whether a quick launch waited for the host's last folder.
/// Every other command is answered as it arrives.
pub(crate) fn creates_answering_with(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    catalog: &serde_json::Value,
    history: &serde_json::Value,
) -> (Vec<FakeCommand>, usize) {
    held_answering(helm, cx, "/api/sessions", catalog, history)
}

/// The commands to `path` (a create's `/api/sessions`, a Replace with's `/api/sessions/{id}/replace`) the window sent
/// since the last call, left unanswered, with the catalog read answered with an empty catalog and every launch-history
/// read with [`empty_history`], as [`creates`] answers them.
pub(super) fn sent_to(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    path: &str,
) -> Vec<FakeCommand> {
    held_answering(helm, cx, path, &json!([]), &empty_history()).0
}

/// The one loop behind [`creates_answering_with`] and [`sent_to`]: hold back the commands to `path` and answer every
/// other one as it arrives, round after round until a round answers nothing, so a request that waited for an answer is
/// among those held. Returns them with the number of history reads answered.
fn held_answering(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    path: &str,
    catalog: &serde_json::Value,
    history: &serde_json::Value,
) -> (Vec<FakeCommand>, usize) {
    let (mut held, mut history_reads) = (Vec::new(), 0);
    loop {
        cx.run_until_parked();
        let mut answered = false;
        for command in helm.commands() {
            if command.path == path {
                held.push(command);
            } else if command.path.ends_with("launch-catalog") {
                helm.reply(command.id, 200, Some(catalog.clone()));
                answered = true;
            } else if command.path.starts_with("/api/launch-history") {
                // The host's history: New's folder default settles from it (a quick launch waits for it).
                history_reads += 1;
                helm.reply(command.id, 200, Some(history.clone()));
                answered = true;
            } else {
                helm.reply(command.id, 200, None);
                answered = true;
            }
        }
        settle(cx);
        if !answered {
            return (held, history_reads);
        }
    }
}

/// Open the launcher on [`command_draft`], press Launch, and return the one create it sent, checking that it sent
/// exactly one and hid.
fn launched(cx: &mut gpui::TestAppContext) -> (FakeHelm, &mut VisualTestContext, FakeCommand) {
    launched_with(cx, command_draft())
}

/// [`launched`] with `draft`, which must be complete.
pub(super) fn launched_with(
    cx: &mut gpui::TestAppContext,
    draft: LaunchDraft,
) -> (FakeHelm, &mut VisualTestContext, FakeCommand) {
    let (helm, cx) = launcher_on(cx, draft);
    pressed(helm, cx)
}

/// [`launched_with`] over the fake helm's first snapshot `state` (`view_tests::launcher_over`), for tests that need
/// another fleet around the launch.
pub(super) fn launched_over(
    cx: &mut gpui::TestAppContext,
    state: Vec<CollectionState>,
    draft: LaunchDraft,
) -> (FakeHelm, &mut VisualTestContext, FakeCommand) {
    let (helm, cx) = super::view_tests::launcher_over(cx, draft, state);
    pressed(helm, cx)
}

/// Press Launch on the open launcher and return the one create it sent, checking that opening sent none, the press
/// sent exactly one, and the launcher hid.
fn pressed(
    mut helm: FakeHelm,
    cx: &mut VisualTestContext,
) -> (FakeHelm, &mut VisualTestContext, FakeCommand) {
    let create = press(&mut helm, cx, "/api/sessions");
    (helm, cx, create)
}

/// Press Launch on the open launcher and return the one request to `path` it sent ([`sent_to`]), checking that nothing
/// to `path` was waiting before the press, the press sent exactly one, and the launcher hid while it is out.
pub(super) fn press(helm: &mut FakeHelm, cx: &mut VisualTestContext, path: &str) -> FakeCommand {
    assert!(
        sent_to(helm, cx, path).is_empty(),
        "the premise: nothing to {path} before the press"
    );
    test_support::click(cx, "launcher-launch");
    let sent = sent_to(helm, cx, path);
    let [request] = &sent[..] else {
        panic!("one press sent one request to {path}, not {}", sent.len());
    };
    assert!(!any_open(cx), "the launcher hid while its request is out");
    request.clone()
}

/// The helm lists session `key` beside `a` (as it does before answering the create that made it) and answers create
/// `id` with that session, as `POST /api/sessions` does.
pub(crate) fn created(helm: &mut FakeHelm, cx: &mut VisualTestContext, id: u64, key: &'static str) {
    created_as(helm, cx, id, &["a", key], session(key));
}

/// [`created`] for any fleet: the helm lists `made` among `order`, then answers create `id` with it.
pub(super) fn created_as(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    id: u64,
    order: &[&str],
    made: crate::session_list::tests::Fixture,
) {
    helm.push(crate::session_list::tests::change(
        crate::session_list::tests::listing_meta(order, order.len() as u64, false),
        std::slice::from_ref(&made),
        &[],
    ));
    settle(cx);
    helm.reply(id, 200, Some(made.entry().value));
    settle(cx);
}

/// The session the window has selected.
pub(super) fn selected(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| local_state::selected(cx))
}

/// Whether the notice offering a refused launch is showing.
fn notice_posted(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| notices::posted(cx).contains(&Notice::LaunchNotStarted))
}

/// SPEC.md, "Creation": "one intended create yields one session or a clear error", and "After submitting a launch ...
/// the launcher hides while the request is pending". This is the submit path every New and Clone goes through, so its
/// body is the contract with the helm's create route. Specified: a press of Launch on a complete Command tab draft sends
/// one `POST /api/sessions` carrying the draft's host, folder and command as the shared client body, and the launcher's
/// dialog closes at once, before any reply.
#[gpui::test]
fn launching_sends_one_create_and_hides_the_launcher(cx: &mut gpui::TestAppContext) {
    let (_helm, _cx, create) = launched(cx);
    let body = create.body.expect("a create carries a body");
    assert_eq!(body["host"], json!(1));
    assert_eq!(body["cwd"], json!("~"));
    assert_eq!(body["command"]["command"], json!("true"));
    assert_eq!(body["confirm_yolo"], json!(false));
}

/// SPEC.md, "Creation": "A successful New or Clone opens its session only while the submitted launcher opening is
/// still current and the user has pressed no key or pointer button anywhere in the page since their last answer to
/// it", and then "ordinary sidebar selection applies". Specified: with no input after the press, the reply naming the
/// new session b selects b.
#[gpui::test]
fn a_launch_the_user_waited_for_opens_its_session(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched(cx);
    let id = create.id;
    assert_ne!(
        selected(cx).as_deref(),
        Some("b"),
        "the premise: b is not selected yet"
    );
    created(&mut helm, cx, id, "b");
    assert_eq!(
        selected(cx).as_deref(),
        Some("b"),
        "the launch opened its session"
    );
    assert!(
        test_support::terminal_focused(cx),
        "as a click on its row would: its terminal took focus"
    );
    assert!(!any_open(cx), "nothing reopened over it");
}

/// SPEC.md, "Creation": "Otherwise the session appears in the sidebar, the selected session and its remembered
/// selection stay as they were, and focus, panels, menus and dialogs stay as the user left them." This is what lets a
/// user keep working while a launch is out. Specified: a key press after Launch, then the reply naming b: the selection
/// stays where it was and nothing opens.
#[gpui::test]
fn a_launch_the_user_moved_on_from_moves_nothing(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched(cx);
    let id = create.id;
    let before = selected(cx);
    keys(cx, "x");
    created(&mut helm, cx, id, "b");
    assert_eq!(
        selected(cx),
        before,
        "the selection stayed where the user left it"
    );
    assert!(!any_open(cx), "nothing opened");
}

/// SPEC.md, "Creation": "A failure or follow-up question restores the same launcher with its draft if the user has not
/// moved on since submitting it", and every refused launch "shows as a bordered alert with a warning icon beside
/// Launch, starting "Not launched." before the refusal's own words". The draft must survive, or a refusal costs the
/// user everything they typed. Specified: the helm refuses the create; the same launcher returns with its command and
/// the helm's words after "Not launched."; an edit takes the alert away.
#[gpui::test]
fn a_refused_launch_brings_the_launcher_back_with_the_helms_words(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched(cx);
    let id = create.id;
    helm.reply(id, 409, Some(json!("the folder does not exist")));
    settle(cx);
    assert!(any_open(cx), "the launcher came back");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. the folder does not exist")
    );
    assert_eq!(
        read(cx, |launcher, _, _| launcher.draft.command.command.clone()),
        "true",
        "with its draft"
    );
    test_support::click(cx, "launcher-name-row");
    keys(cx, "n");
    assert_eq!(shown_refusal(cx), None, "an edit takes the refusal away");
}

/// Specified: a 409 saying the folder does not exist brings the launcher back; fixing the folder to `/srv` and pressing
/// Launch sends that folder, and a success opens session b while the user waits. Why: preserving the form must leave it
/// usable for another attempt, not stuck after the refusal ("create dialog surfaces a precondition failure, preserves
/// the form, and creates no session", `e2e/tests/terminal.spec.ts` at `24bc0580`, its second half).
#[gpui::test]
fn a_second_press_after_a_refusal_launches_the_fixed_draft(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched(cx);
    helm.reply(create.id, 409, Some(json!("the folder does not exist")));
    settle(cx);
    assert!(any_open(cx), "the launcher came back");
    super::folder_default_tests::edit_draft(cx, |draft| draft.edit_folder("/srv".into()));
    let retry = press(&mut helm, cx, "/api/sessions");
    assert_eq!(retry.body.as_ref().unwrap()["cwd"], json!("/srv"));
    created(&mut helm, cx, retry.id, "b");
    assert_eq!(selected(cx).as_deref(), Some("b"));
}

/// A create that fails after the host's registry row moved to another installation while it was out comes back with
/// the old launcher's generic words, not the helm's own. Why: those words came from the earlier installation and could
/// describe a machine the draft no longer targets (launcher-enter.spec's "launcher shows a generic refusal after its
/// target changes"). Specified: host1 is retargeted (a new recorded identity) while the create is out, then the create
/// is refused; the launcher returns saying "A launch for an earlier setup was refused" after "Not launched.", never the
/// helm's text.
#[gpui::test]
fn a_refusal_after_the_host_moved_installation_is_generic(cx: &mut gpui::TestAppContext) {
    assert_eq!(
        after_the_host_moved(cx, 409).as_deref(),
        Some("Not launched. A launch for an earlier setup was refused")
    );
}

/// The timeout half of [`a_refusal_after_the_host_moved_installation_is_generic`]: the helm's host-reply bound cut the
/// create off, so it may still start, and the generic words must say its outcome is unknown rather than "refused" (the
/// old launcher's words for this case). Specified: host1 retargeted while the create is out, then the helm's timeout;
/// the launcher returns saying "A launch for an earlier setup timed out; its outcome is unknown", with no "Not
/// launched." lead.
#[gpui::test]
fn a_timeout_after_the_host_moved_installation_is_generic(cx: &mut gpui::TestAppContext) {
    assert_eq!(
        after_the_host_moved(cx, 504).as_deref(),
        Some("A launch for an earlier setup timed out; its outcome is unknown")
    );
}

/// Launch, retarget host1 onto another installation while the create is out (checked in the store), then answer the
/// create with `status` and the helm's own words; returns the refusal the launcher shows once it is back.
fn after_the_host_moved(cx: &mut gpui::TestAppContext, status: u16) -> Option<String> {
    let (mut helm, cx, create) = launched(cx);
    let mut state = super::view_tests::fixture_hosts(&[0]);
    state.entries[0].value["identity"] = json!("replacement-install");
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
    let identity = cx.update(|_, cx| {
        let store = crate::session::current_store(cx).expect("a store");
        store
            .read(cx)
            .collections()
            .get::<HostView>(&HostView::key(1))
            .and_then(|host| host.identity.clone())
    });
    assert_eq!(
        identity.as_deref(),
        Some("replacement-install"),
        "the premise: host1 names another installation"
    );
    helm.reply(create.id, status, Some(json!("old-target private words")));
    settle(cx);
    assert!(any_open(cx), "the launcher came back");
    shown_refusal(cx)
}

/// SPEC.md, "Creation": after a refusal the user moved on from, "a notice offers it, and the launcher returns when the
/// user opens that notice"; "Keyboard focus": "A launch that fails after the user moved on shows a notice, and clicking
/// it brings back the launcher with its draft and error". Reopening by itself would take focus from whatever the user
/// went on to. Specified: a key press after Launch, then a refusal: nothing opens and the notice shows; its Open
/// launcher brings the launcher back with the refusal, and the notice goes.
#[gpui::test]
fn a_refusal_the_user_moved_on_from_waits_behind_a_notice(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched(cx);
    let id = create.id;
    keys(cx, "x");
    helm.reply(id, 409, Some(json!("host1 is not connected")));
    settle(cx);
    assert!(
        !any_open(cx),
        "nothing opened over what the user went on to"
    );
    assert!(notice_posted(cx), "the notice offers the launcher");
    assert_eq!(
        name_of(cx, "notice-open-launcher").as_deref(),
        Some("Open launcher")
    );
    test_support::click(cx, "notice-open-launcher");
    assert!(any_open(cx), "Open launcher brought it back");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. host1 is not connected")
    );
    assert!(!notice_posted(cx), "the notice went with it");
}

/// SPEC.md, "Creation": "A held Enter never launches". A single-line field reports Enter on every auto-repeat, so
/// without the launcher's own check a key held down from an earlier dialog would launch the moment the launcher
/// opened. Specified: focus in the Name field; a held Enter there sends nothing and leaves the launcher open; a fresh
/// Enter launches.
#[gpui::test]
fn a_held_enter_in_a_field_never_launches(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, command_draft());
    creates(&mut helm, cx);
    test_support::click(cx, "launcher-name-row");
    assert!(
        read(cx, |launcher, window, cx| launcher
            .name
            .focus_handle(cx)
            .is_focused(window)),
        "the premise: the Name field has focus"
    );
    cx.simulate_event(gpui::KeyDownEvent {
        native_key: None,
        keystroke: Keystroke::parse("enter").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    settle(cx);
    assert!(
        creates(&mut helm, cx).is_empty(),
        "a held Enter sent nothing"
    );
    assert!(any_open(cx), "and the launcher stayed open");
    keys(cx, "enter");
    assert_eq!(creates(&mut helm, cx).len(), 1, "a fresh Enter launches");
}

/// SPEC.md, "Creation": "A held Enter never launches". The host button handles its own Enter, so inject only the
/// repeat with the list closed. An earlier fresh key would hide a broken repeat guard behind a launch already in flight.
#[gpui::test]
fn a_held_enter_on_the_closed_host_button_never_launches(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, command_draft());
    assert!(creates(&mut helm, cx).is_empty());
    test_support::click(cx, "launcher-host");
    assert!(super::view_tests::has_node(cx, "launcher-host-list"));
    keys(cx, "escape");
    assert!(!super::view_tests::has_node(cx, "launcher-host-list"));
    assert!(
        read(cx, |launcher, window, _| launcher
            .host_focus
            .is_focused(window)),
        "the premise: Enter will reach the closed host button"
    );
    cx.simulate_event(gpui::KeyDownEvent {
        native_key: None,
        keystroke: Keystroke::parse("enter").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    settle(cx);
    assert!(creates(&mut helm, cx).is_empty(), "a repeat did not launch");
    assert!(any_open(cx), "the launcher stayed open");
    assert!(!super::view_tests::has_node(cx, "launcher-host-list"));
    keys(cx, "enter");
    assert_eq!(creates(&mut helm, cx).len(), 1, "a fresh Enter launches");
}

/// The notice offers a launcher that lives with the UI session (decided by the maintainer for the migration: drafts do
/// not survive a reset). After a reset there is nothing left for Open launcher to bring back, so a notice still showing
/// would offer nothing. Specified: a refusal the user moved on from posts the notice; the channel breaks and a new UI
/// session connects; the notice is gone.
#[gpui::test]
fn a_reset_withdraws_the_notice_with_its_launcher(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched(cx);
    keys(cx, "x");
    helm.reply(create.id, 409, Some(json!("host1 is not connected")));
    settle(cx);
    assert!(
        notice_posted(cx),
        "the premise: the notice offers the launcher"
    );
    helm.break_channel();
    helm.connect(vec![listing(&["a"], &[session("a")])]);
    settle(cx);
    assert!(
        !notice_posted(cx),
        "the reset took the notice with its launcher"
    );
}

/// SPEC.md's after-close rule ("Opening and closing") guards Enter after the launcher closes. The second tap lands on
/// whatever regains focus; the focus coordinator neither counts it nor delivers it there. Without that, a double Enter
/// would keep the new session from opening and type a line break into the terminal. Specified: Enter in the Name field
/// launches, a second Enter follows at once, and the reply naming b still opens b with its terminal focused; the second
/// Enter sent no second create.
#[gpui::test]
fn a_repeated_enter_on_launch_still_opens_the_session(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, command_draft());
    creates(&mut helm, cx);
    test_support::click(cx, "launcher-name-row");
    keys(cx, "enter");
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one Enter sent one create, not {}", sent.len());
    };
    let id = create.id;
    assert!(!any_open(cx), "the premise: the launcher hid");
    keys(cx, "enter");
    assert!(
        creates(&mut helm, cx).is_empty(),
        "the repeated Enter sent nothing"
    );
    created(&mut helm, cx, id, "b");
    assert_eq!(
        selected(cx).as_deref(),
        Some("b"),
        "the repeat did not count as moving on"
    );
    assert!(
        test_support::terminal_focused(cx),
        "b's terminal took focus"
    );
}

/// Work one of the hosts area's dialogs starts, for the exclusion tests: three host-registry changes, and a host
/// settings save, which is not one.
#[derive(Clone, Copy, Debug)]
pub(crate) enum HostChange {
    /// Add host's probe of a new destination, under a fresh host-add key.
    Probe,
    /// A new host's setup, under a fresh host-add key.
    Setup,
    /// A save on a registered host's own key: host1's alias, as its settings dialog sends it.
    HostSave,
    /// Remove host for host2, which the window does not list, so only its kind of work can count it.
    Remove,
}

/// Start `change` in the window's operation store as the hosts area does, and return the path it sends, so a test
/// can find and answer it. The session list's tests use the probe too, to make New unavailable for its triggers (the
/// heading's New, the quick switcher's New row) without staging a launch.
pub(crate) fn start_host_change(cx: &mut VisualTestContext, change: HostChange) -> &'static str {
    use crate::operations::{Command, OpKey, Work};
    use farhelm_helm_client::{ApiPath, Method};
    let (key, command, path) = match change {
        HostChange::Probe => (
            OpKey::new_host_add(),
            crate::hosts::add::probe_command("builder@example.test", "", false).unwrap(),
            "/api/hosts/probe",
        ),
        HostChange::Setup => (
            OpKey::new_host_add(),
            Command::new(
                Work::Setup,
                Method::Post,
                ApiPath::api(&["hosts", "provision"]).unwrap(),
                Some(json!({})),
                "POST /api/hosts/provision",
            ),
            "/api/hosts/provision",
        ),
        HostChange::HostSave => (
            OpKey::host("1"),
            Command::new(
                Work::Save,
                Method::Put,
                ApiPath::api(&["hosts", "1", "alias"]).unwrap(),
                Some(json!({ "alias": "builder" })),
                "PUT /api/hosts/[host]/alias",
            ),
            "/api/hosts/1/alias",
        ),
        HostChange::Remove => (
            OpKey::host("2"),
            Command::new(
                Work::RemoveHost,
                Method::Delete,
                ApiPath::api(&["hosts", "2"]).unwrap(),
                None,
                "DELETE /api/hosts/[host]",
            ),
            "/api/hosts/2",
        ),
    };
    cx.update(|_, cx| {
        let operations = crate::session::current_operations(cx).expect("a UI session");
        operations
            .update(cx, |operations, cx| {
                operations.start(&key, command, cx, |_, _| {})
            })
            .expect("the host change started");
    });
    settle(cx);
    path
}

/// SPEC.md, "Creation": "Launches and changes to the host registry still exclude each other." A launch that picked
/// its host while the registry was changing could aim at an entry the change is about to rewrite, which is why the old
/// app held one token for both. Each kind of change counts: a new host's probe (the migration's orchestrator decided
/// it counts, though the old token left it out) and setup, and a host's removal. Specified: for each, while it is in
/// flight a press of Launch on a complete draft sends no create and says why after "Not launched."; once it is
/// answered, the same press launches.
#[gpui::test]
fn launch_waits_for_a_host_change(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, command_draft());
    assert!(
        creates(&mut helm, cx).is_empty(),
        "the premise: opening sends no create"
    );
    for change in [HostChange::Probe, HostChange::Setup, HostChange::Remove] {
        let path = start_host_change(cx, change);
        cx.run_until_parked();
        let sent = helm.commands();
        let [command] = &sent[..] else {
            panic!(
                "{change:?}: only the host change is out, not {} commands",
                sent.len()
            );
        };
        assert_eq!(
            command.path, path,
            "{change:?}: the premise: the change is in flight"
        );
        test_support::click(cx, "launcher-launch");
        assert!(
            helm.commands().is_empty(),
            "{change:?}: Launch sent nothing"
        );
        assert_eq!(
            shown_refusal(cx).as_deref(),
            Some("Not launched. Wait for the host change to finish"),
            "{change:?}"
        );
        assert!(any_open(cx), "{change:?}: the launcher stayed open");
        helm.reply(command.id, 200, Some(json!({})));
        settle(cx);
        assert_eq!(
            shown_refusal(cx),
            None,
            "{change:?}: the reason went with the change"
        );
    }
    test_support::click(cx, "launcher-launch");
    assert_eq!(
        creates(&mut helm, cx).len(),
        1,
        "with every change answered, Launch launches"
    );
}

/// The migration's orchestrator decided on 2026-11-14 that a launch does not wait for a save of a registered host's
/// settings, its destination and alias included: that window is one short save, and the hosts side is to refuse a
/// destination change during a launch instead. Host1 is a host the window lists, so the save is on a listed host's own
/// key, which an earlier version counted. Specified: while host1's alias save is in flight, a press of Launch on a
/// complete draft sends its create.
#[gpui::test]
fn launch_does_not_wait_for_a_host_settings_save(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, command_draft());
    assert!(
        creates(&mut helm, cx).is_empty(),
        "the premise: opening sends no create"
    );
    assert!(
        cx.update(|_, cx| {
            crate::session::current_store(cx).is_some_and(|store| {
                store
                    .read(cx)
                    .collections()
                    .ordered::<farhelm_proto::ui_channel::HostView>()
                    .iter()
                    .any(|host| host.id == 1)
            })
        }),
        "the premise: host1 is a host the window lists"
    );
    let path = start_host_change(cx, HostChange::HostSave);
    cx.run_until_parked();
    let sent = helm.commands();
    assert!(
        sent.iter().any(|command| command.path == path),
        "the premise: the save is in flight"
    );
    test_support::click(cx, "launcher-launch");
    cx.run_until_parked();
    let creates: Vec<_> = helm
        .commands()
        .into_iter()
        .filter(|command| command.path == "/api/sessions")
        .collect();
    assert_eq!(creates.len(), 1, "Launch launched beside the save");
}

/// The words the helm's bounded create answers with when the host does not answer in time
/// (`sessions::within_host_reply_bound`). Shared with the Restart with and Replace with tests, whose routes are bounded
/// the same way.
pub(super) const TIMED_OUT: &str = "the host did not answer within 20 seconds; the request is still being \
                                   carried out, and the session list will show its result";

/// A Restart with's or Replace with's reply of the host's timeout, after the user moved on, settles as one that may
/// still happen: answer request `id` with [`TIMED_OUT`] after a key press, then check that no launcher opened, that
/// `key`'s row holds `work`'s line with those words while it waits, and that the notice brings the launcher back
/// showing them alone, without a refusal's lead. Shared by the two modes' tests, which differ only in how they press.
pub(super) fn times_out_after_moving_on(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    id: u64,
    key: &str,
    work: crate::operations::Work,
) {
    keys(cx, "x");
    helm.reply(id, 504, Some(json!(TIMED_OUT)));
    settle(cx);
    assert!(!any_open(cx), "nothing opened, since the user moved on");
    let line = cx.update(|_, cx| crate::session_list::outcome::current_line(key, cx));
    let line = line.expect("the row has a line while the launcher waits");
    assert_eq!((line.work, line.message.raw()), (work, TIMED_OUT));
    test_support::click(cx, "notice-open-launcher");
    assert!(any_open(cx), "the notice brought the launcher back");
    assert_eq!(shown_refusal(cx).as_deref(), Some(TIMED_OUT));
}

/// SPEC_impl.md, "The UI channel": a bounded command answers "the host timed out" while the work carries on, so a
/// timed-out create may still start a session. Calling it "Not launched." would invite a second launch of the same
/// draft; the old app waited without a deadline and never had to say this. Specified: the helm answers the create with
/// a 504 and no marker; the launcher comes back with the helm's words alone, no "Not launched." lead, and its draft.
#[gpui::test]
fn a_timed_out_create_comes_back_without_not_launched(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched(cx);
    helm.reply(create.id, 504, Some(json!(TIMED_OUT)));
    settle(cx);
    assert!(any_open(cx), "the launcher came back");
    assert_eq!(shown_refusal(cx).as_deref(), Some(TIMED_OUT));
    assert_eq!(
        read(cx, |launcher, _, _| launcher.draft.command.command.clone()),
        "true",
        "with its draft"
    );
}

/// A create the helm can prove never reached the host carries its marker, and was not launched whatever the status, so
/// it keeps the ordinary lead. Specified: a 504 marked as never accepted by the host shows "Not launched." before the
/// helm's words.
#[gpui::test]
fn a_create_the_host_never_accepted_is_not_launched(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched(cx);
    helm.reply_marked(
        create.id,
        504,
        vec![crate::operations::HelmMarker::CreateUnaccepted],
        Some(json!("the create did not reach host1")),
    );
    settle(cx);
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. the create did not reach host1")
    );
}

/// The notice for a timed-out create the user moved on from must not say "Not launched." either. Specified: a key
/// press after Launch, then a 504: the notice for a launch that may still start is posted, not the not-launched one, and
/// its Open launcher brings the launcher back with the helm's words.
#[gpui::test]
fn a_timed_out_create_the_user_moved_on_from_has_its_own_notice(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched(cx);
    keys(cx, "x");
    helm.reply(create.id, 504, Some(json!(TIMED_OUT)));
    settle(cx);
    let posted = cx.update(|_, cx| notices::posted(cx));
    assert_eq!(posted, vec![Notice::LaunchMayStillStart]);
    assert_eq!(
        name_of(cx, "notice-open-launcher").as_deref(),
        Some("Open launcher"),
        "its banner offers the launcher"
    );
    test_support::click(cx, "notice-open-launcher");
    assert_eq!(shown_refusal(cx).as_deref(), Some(TIMED_OUT));
    assert!(
        cx.update(|_, cx| notices::posted(cx)).is_empty(),
        "the notice went"
    );
}

// ---- The pills -------------------------------------------------------------------------------------------------------

/// Whether the frame painted the line `text` anywhere (the pill's seconds are their own text element).
fn painted(cx: &mut VisualTestContext, text: &str) -> bool {
    crate::session_list::tests::painted(&test_support::frame(cx))
        .iter()
        .any(|line| line == text)
}

/// SPEC.md, "Creation": "after about two seconds, a small pending pill appears at the bottom of the window with a
/// spinner, Launching ..., the captured host's name, the session name when supplied, and elapsed seconds", announced
/// once, not every second, and gone when the launcher returns (the old `launch-progress.spec.ts` case "launcher status
/// waits two seconds, counts up and ends on a real refusal"). Without it a slow launch hides the launcher with no sign
/// that anything is happening. Specified: a launch of "slow" on host1 shows nothing for 1.9 seconds; at 2.1 seconds the
/// pill is a status named "Launching “slow” on host1…" painting "2 s", and a second later "3 s" under the same name; a
/// refusal then takes it down and brings the launcher back with the refusal.
#[gpui::test]
fn a_slow_launch_shows_its_progress_pill_after_two_seconds(cx: &mut gpui::TestAppContext) {
    let mut draft = command_draft();
    draft.edit_name("slow".into());
    let (mut helm, cx, create) = launched_with(cx, draft);
    test_support::advance(cx, std::time::Duration::from_millis(1_900));
    assert_eq!(
        name_of(cx, "launch-progress"),
        None,
        "nothing before two seconds"
    );
    test_support::advance(cx, std::time::Duration::from_millis(200));
    let name = name_of(cx, "launch-progress");
    assert_eq!(name.as_deref(), Some("Launching “slow” on host1…"));
    assert!(is_status(cx, "launch-progress"), "announced as a status");
    assert!(painted(cx, "2 s"), "it counts from the press");
    test_support::advance(cx, std::time::Duration::from_secs(1));
    assert!(painted(cx, "3 s"), "and counts up");
    assert_eq!(name_of(cx, "launch-progress"), name, "under the same name");
    helm.reply(create.id, 409, Some(json!("host1 is not connected")));
    settle(cx);
    assert_eq!(
        name_of(cx, "launch-progress"),
        None,
        "the reply took it down"
    );
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. host1 is not connected")
    );
}

/// A quick reply must not flash a spinner or leave a counter behind (the old case "a launch answered below the
/// threshold keeps the ordinary refusal"). Specified: a create refused at once shows its refusal, and no pill shows
/// three seconds later.
#[gpui::test]
fn a_quick_reply_shows_no_progress_pill(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched(cx);
    helm.reply(create.id, 400, Some(json!("fixture launch refused")));
    settle(cx);
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. fixture launch refused"),
        "the premise: the refusal came back"
    );
    test_support::advance(cx, std::time::Duration::from_secs(3));
    assert_eq!(name_of(cx, "launch-progress"), None);
}

/// SPEC.md, "Creation": when a successful New does not open its session because the user moved on, "the row appears,
/// nothing else moves, and a display-only pill says Started “name” for about four seconds"; a launch that opens its
/// session needs no report. Specified: after a key press while the create is out, its success leaves the selection
/// alone and shows a status named "Started “made”", gone four seconds later; a success the user waited for opens the
/// session and shows no report.
#[gpui::test]
fn a_launch_that_does_not_open_reports_started(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched(cx);
    keys(cx, "x");
    created(&mut helm, cx, create.id, "made");
    assert_ne!(
        selected(cx).as_deref(),
        Some("made"),
        "the premise: the user moved on, so nothing opened"
    );
    assert_eq!(
        name_of(cx, "launch-started-report").as_deref(),
        Some("Started “made”")
    );
    test_support::advance(cx, std::time::Duration::from_millis(3_900));
    assert!(
        name_of(cx, "launch-started-report").is_some(),
        "up for four seconds"
    );
    test_support::advance(cx, std::time::Duration::from_millis(200));
    assert_eq!(name_of(cx, "launch-started-report"), None);
}

/// The other half of [`a_launch_that_does_not_open_reports_started`]: a launch the user waited for opens its session,
/// and the open session is its own report. Specified: with no press meanwhile, the success opens "made" and shows no
/// "Started" report.
#[gpui::test]
fn a_launch_that_opens_its_session_reports_nothing(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched(cx);
    created(&mut helm, cx, create.id, "made");
    assert_eq!(
        selected(cx).as_deref(),
        Some("made"),
        "the premise: it opened"
    );
    assert_eq!(name_of(cx, "launch-started-report"), None);
}

/// The node drawn under `id` is a status, which assistive technology announces when it appears (the old pill's
/// `role="status"`).
pub(crate) fn is_status(cx: &mut VisualTestContext, id: &str) -> bool {
    test_support::frame(cx)
        .iter()
        .any(|node| node.element_id.as_deref() == Some(id) && node.role == Some(gpui::Role::Status))
}

/// The seconds must move on in a window behind others too, since the wait is the user's time, while the spinner stops
/// there (SPEC_impl.md, "Animation time: the ticker": a looping animation runs only in the active window; a count of
/// seconds runs on the ticker's once-a-second clock regardless). Without the clock the count would freeze until
/// something else redrew the pill. Specified: with the pill up and the window deactivated, a second's advance redraws
/// the pill, which then paints "3 s".
#[gpui::test]
fn the_progress_pill_counts_in_an_inactive_window(cx: &mut gpui::TestAppContext) {
    let (_helm, cx, _create) = launched(cx);
    test_support::advance(cx, std::time::Duration::from_millis(2_100));
    assert!(
        is_status(cx, "launch-progress"),
        "the premise: the pill shows"
    );
    cx.deactivate_window();
    settle(cx);
    assert!(
        !cx.update(|window, _| window.is_window_active()),
        "the premise: the window is inactive"
    );
    cx.update(|_, cx| crate::render_count::reset(cx));
    test_support::advance(cx, std::time::Duration::from_secs(1));
    assert!(
        cx.update(|_, cx| crate::render_count::count("launch-progress", cx)) > 0,
        "the clock redrew the pill"
    );
    assert!(painted(cx, "3 s"));
}

/// A slow launch that succeeds while the user is elsewhere hands the one launch pill from its progress to its report,
/// and neither keeps New busy (SPEC.md, "Creation": the report "does not keep the launcher or its draft busy").
/// Specified: with the progress pill up and a key pressed meanwhile, the success replaces it with "Started “made”", and
/// New opens while the report shows.
#[gpui::test]
fn a_slow_success_hands_the_pill_to_its_started_report(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched(cx);
    test_support::advance(cx, std::time::Duration::from_millis(2_100));
    assert!(
        is_status(cx, "launch-progress"),
        "the premise: the pill shows"
    );
    keys(cx, "x");
    created(&mut helm, cx, create.id, "made");
    assert_eq!(name_of(cx, "launch-progress"), None, "the progress went");
    assert!(is_status(cx, "launch-started-report"));
    assert_eq!(
        name_of(cx, "launch-started-report").as_deref(),
        Some("Started “made”")
    );
    assert!(cx.update(super::entry::open_new), "New opens meanwhile");
    settle(cx);
    assert!(any_open(cx));
}

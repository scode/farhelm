//! Tests of launching from the launcher, in the real main view over the helm client's fake: the one create a press of
//! Launch sends, the launcher hiding while it is out, and what its reply opens or brings back, with and without the user
//! having moved on.
//!
//! Every test launches a Command tab draft (`true`, not YOLO, on host1 in `~`), the smallest complete draft: what is
//! under test is the submit path, which every complete draft shares. The fake answers commands only when told, so each
//! test holds the create's reply until it has set up the moment the reply arrives in.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{CollectionChanges, CollectionState};
use gpui::{Keystroke, VisualTestContext};
use serde_json::json;

use super::draft::{LaunchDraft, Tab};
use super::view_tests::{launcher_on, name_of, read, shown_refusal};
use crate::local_state;
use crate::notices::{self, Notice};
use crate::session_list::tests::{listing, session};
use crate::test_support::{self, any_open, keys, settle};

/// The smallest complete draft: a Command tab launch of `true`, asserted not YOLO, on host1 in `~`.
fn command_draft() -> LaunchDraft {
    let mut draft = LaunchDraft::new(Some(1), "~".into(), None);
    draft.choose_tab(Tab::Command);
    draft.edit_command("true".into());
    draft.choose_command_yolo(false);
    draft
}

/// The creates (`POST /api/sessions`) the window sent since the last call. Every other command (the catalog read, the
/// selection remembered) is answered at once, so the fake's in-order replies stay possible while a create waits.
pub(super) fn creates(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    cx.run_until_parked();
    let mut creates = Vec::new();
    for command in helm.commands() {
        if command.path == "/api/sessions" {
            creates.push(command);
        } else if command.path.ends_with("launch-catalog") {
            helm.reply(command.id, 200, Some(json!([])));
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    settle(cx);
    creates
}

/// Open the launcher on [`command_draft`], press Launch, and return the one create it sent, checking that it sent
/// exactly one and hid.
fn launched(cx: &mut gpui::TestAppContext) -> (FakeHelm, &mut VisualTestContext, FakeCommand) {
    let (mut helm, cx) = launcher_on(cx, command_draft());
    assert!(
        creates(&mut helm, cx).is_empty(),
        "the premise: opening sends no create"
    );
    test_support::click(cx, "launcher-launch");
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one press sent one create, not {}", sent.len());
    };
    assert!(!any_open(cx), "the launcher hid while its create is out");
    let create = create.clone();
    (helm, cx, create)
}

/// The helm lists session `key` (as it does before answering the create that made it) and answers create `id` with that
/// session, as `POST /api/sessions` does.
fn created(helm: &mut FakeHelm, cx: &mut VisualTestContext, id: u64, key: &'static str) {
    let state: CollectionState = listing(&["a", key], &[session("a"), session(key)]);
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
    helm.reply(id, 200, Some(session(key).entry().value));
    settle(cx);
}

/// The session the window has selected.
fn selected(cx: &mut VisualTestContext) -> Option<String> {
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

/// SPEC.md, "Creation": moving on does not count "an accidental double-click or repeated Enter on the launcher's own
/// submit". Launch closes the launcher's dialog through the layer manager, so the second Enter of a double press lands on
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

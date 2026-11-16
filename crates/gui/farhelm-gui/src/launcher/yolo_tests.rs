//! Tests of the YOLO question: which reason it gives and when it offers guarded (no GPUI), its flows in the launcher
//! over the helm client's fake, from the helm's YOLO refusal to the launch each answer sends, and the plain Replace
//! question the sidebar asks through `entry::ask_yolo_for_replace`, on a page of its own.
//!
//! The helm's refusal is a 409 carrying `HelmMarker::YoloConfirmation`, as the helm sends it for a YOLO launch on a
//! host that asks first. The fake answers commands only when told, so each test answers the create, then the host
//! write.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::HelmMarker;
use farhelm_proto::{LaunchHarness, LaunchPermission, LaunchSelection};
use gpui::VisualTestContext;
use serde_json::json;

use super::draft::{LaunchDraft, Tab};
use super::launch_tests::{created, creates, launched_with, selected};
use super::view_tests::{name_of, read};
use super::yolo::YoloReason;
use crate::test_support::{self, any_open, focused_id, keys, settle};

/// An agent selection on `harness` with `permissions`, nothing else chosen.
fn selection(harness: LaunchHarness, permissions: Option<LaunchPermission>) -> LaunchSelection {
    LaunchSelection {
        harness,
        model: None,
        effort: None,
        permissions,
        permission_flags: None,
        workspace_trust: None,
        wrapper_prefix: None,
        container: None,
    }
}

/// SPEC.md, "Creation": the question says "why this launch is one (YOLO was chosen where the agent type offers other
/// modes, the agent type has no guarded mode, the launch uses the agent's own setting which Farhelm cannot override,
/// custom flags whose effect Farhelm does not check, or the command was asserted to be YOLO)". Specified: a command
/// launch is Asserted; Custom flags are CustomFlags; YOLO chosen on Codex is ChosenPermission; Pi, which offers only
/// YOLO, is OnlyMode; Codex with no permission stored is OwnSetting.
#[test]
fn each_yolo_launch_gets_its_own_reason() {
    assert_eq!(YoloReason::of_launch(None), YoloReason::Asserted);
    let custom = selection(LaunchHarness::Codex, Some(LaunchPermission::Custom));
    assert_eq!(
        YoloReason::of_launch(Some(&custom)),
        YoloReason::CustomFlags
    );
    let chosen = selection(LaunchHarness::Codex, Some(LaunchPermission::Yolo));
    assert_eq!(
        YoloReason::of_launch(Some(&chosen)),
        YoloReason::ChosenPermission
    );
    let pi = selection(LaunchHarness::Pi, Some(LaunchPermission::Yolo));
    assert_eq!(
        YoloReason::of_launch(Some(&pi)),
        YoloReason::OnlyMode(LaunchHarness::Pi)
    );
    let own = selection(LaunchHarness::Codex, None);
    assert_eq!(YoloReason::of_launch(Some(&own)), YoloReason::OwnSetting);
}

/// SPEC.md: "an explicit YOLO choice on an agent that offers guarded mode also offers starting the same launch guarded
/// instead". Offering it for anything else would quietly change a launch the user did not choose YOLO for. Specified:
/// YOLO chosen on Codex offers guarded; an own-setting Codex launch, custom flags, Pi and a command do not.
#[test]
fn only_an_explicit_yolo_choice_offers_guarded() {
    let chosen = selection(LaunchHarness::Codex, Some(LaunchPermission::Yolo));
    assert!(YoloReason::of_launch(Some(&chosen)).offers_guarded(Some(&chosen)));
    for other in [
        selection(LaunchHarness::Codex, None),
        selection(LaunchHarness::Codex, Some(LaunchPermission::Custom)),
        selection(LaunchHarness::Pi, Some(LaunchPermission::Yolo)),
    ] {
        assert!(
            !YoloReason::of_launch(Some(&other)).offers_guarded(Some(&other)),
            "{other:?}"
        );
    }
    assert!(!YoloReason::Asserted.offers_guarded(None));
}

/// A Command tab launch of `true`, asserted YOLO, on host1 in `~`.
fn yolo_command_draft() -> LaunchDraft {
    let mut draft = LaunchDraft::new(Some(1), "~".into(), None);
    draft.choose_tab(Tab::Command);
    draft.edit_command("true".into());
    draft.choose_command_yolo(true);
    draft
}

/// A Codex launch with YOLO chosen, on host1 in `~`.
fn yolo_codex_draft() -> LaunchDraft {
    let mut draft = LaunchDraft::new(Some(1), "~".into(), Some(LaunchPermission::Yolo));
    let _ = draft.choose_harness(LaunchHarness::Codex, None, None);
    draft.choose_permissions(Some(LaunchPermission::Yolo));
    draft
}

/// The helm refuses create `id` for YOLO, as it does on a host that asks first. Shared with the quick-launch tests
/// (`templates::dispatch_tests`), whose hidden launches meet the same refusal.
pub(crate) fn refuse_for_yolo(helm: &mut FakeHelm, cx: &mut VisualTestContext, id: u64) {
    helm.reply_marked(
        id,
        409,
        vec![HelmMarker::YoloConfirmation],
        Some(json!("YOLO launches on host1 need --yolo")),
    );
    settle(cx);
}

/// SPEC.md, "Creation": the GUI answers the helm's YOLO refusal "with a prominent confirmation, shown with the control
/// or surface that started the launch", that names the host and says why; "Cancel keeps initial focus"; the helm's own
/// sentence, written for the command line, is not shown. Specified: after the refusal the launcher is back with the
/// question, focus on Cancel, its one-off answer "Start YOLO session anyway", the asserted command's reason, and no
/// "Not launched." alert.
#[gpui::test]
fn a_yolo_refusal_asks_the_question_with_cancel_focused(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched_with(cx, yolo_command_draft());
    refuse_for_yolo(&mut helm, cx, create.id);
    assert!(any_open(cx), "the launcher came back");
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    assert_eq!(
        name_of(cx, "prompt-confirm").as_deref(),
        Some("Start YOLO session anyway")
    );
    assert_eq!(
        name_of(cx, "prompt-alternative-0").as_deref(),
        Some("Start, and don't ask again on this host"),
        "a command offers no guarded answer, so the permanent one comes first"
    );
    assert_eq!(
        name_of(cx, "yolo-reason").as_deref(),
        Some("This command was asserted to bypass its safety checks."),
        "the question says why this launch is YOLO"
    );
    assert_eq!(
        name_of(cx, "launcher-refusal"),
        None,
        "the helm's own sentence is not shown"
    );
}

/// The one-off answer launches the same draft with consent (SPEC.md: it "retries with the override only when the user
/// confirms"). Specified: "Start YOLO session anyway" sends one create carrying `confirm_yolo`, and the launcher hides.
#[gpui::test]
fn the_one_off_answer_launches_with_consent(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched_with(cx, yolo_command_draft());
    refuse_for_yolo(&mut helm, cx, create.id);
    test_support::click(cx, "prompt-confirm");
    let sent = creates(&mut helm, cx);
    let [retry] = &sent[..] else {
        panic!("one create, not {}", sent.len());
    };
    assert_eq!(retry.body.as_ref().unwrap()["confirm_yolo"], json!(true));
    assert!(!any_open(cx), "the launcher hid again");
}

/// SPEC.md: the permanent answer "first sets the host to start YOLO sessions without asking, exactly as its settings
/// would, and then retries with the override". Specified: it sends the host settings' own write for host1, and only
/// once that is answered a create with `confirm_yolo`.
#[gpui::test]
fn the_permanent_answer_stops_asking_then_launches(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched_with(cx, yolo_command_draft());
    refuse_for_yolo(&mut helm, cx, create.id);
    test_support::click(cx, "prompt-alternative-0");
    cx.run_until_parked();
    let sent = helm.commands();
    let [write] = &sent[..] else {
        panic!("only the host write is out, not {} commands", sent.len());
    };
    assert_eq!(write.path, "/api/hosts/1/yolo-without-asking");
    assert_eq!(write.body, Some(json!({ "yolo_without_asking": true })));
    helm.reply(write.id, 200, Some(json!({})));
    settle(cx);
    let sent = creates(&mut helm, cx);
    let [retry] = &sent[..] else {
        panic!("one create after the write, not {}", sent.len());
    };
    assert_eq!(retry.body.as_ref().unwrap()["confirm_yolo"], json!(true));
}

/// SPEC.md: "if changing that setting fails, nothing is started and the confirmation stays up with the reason". And
/// while the write is out, Escape must not close the launcher under it, or the write's success would start a session
/// the user dismissed. Specified: Escape during the write leaves the launcher open; a refused write sends no create and
/// keeps the question up, saying it could not stop asking for host1, why, and that nothing started.
#[gpui::test]
fn a_failed_stop_asking_starts_nothing_and_keeps_the_question(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched_with(cx, yolo_command_draft());
    refuse_for_yolo(&mut helm, cx, create.id);
    test_support::click(cx, "prompt-alternative-0");
    cx.run_until_parked();
    let write = helm
        .commands()
        .pop()
        .expect("the premise: a command is out");
    assert_eq!(
        write.path, "/api/hosts/1/yolo-without-asking",
        "the premise: it is the host write"
    );
    // Focus outside the question, whose own frozen answers take Escape themselves: this is the launcher's Escape.
    test_support::click(cx, "launcher-name-row");
    assert!(
        read(cx, |launcher, window, cx| launcher
            .name
            .focus_handle(cx)
            .is_focused(window)),
        "the premise: focus is in the Name field, outside the question"
    );
    keys(cx, "escape");
    assert!(
        any_open(cx),
        "Escape kept the launcher open while the write is out"
    );
    helm.reply(write.id, 500, Some(json!("disk full.")));
    settle(cx);
    assert!(creates(&mut helm, cx).is_empty(), "nothing started");
    assert_eq!(
        name_of(cx, "yolo-stop-asking-failed").as_deref(),
        Some(
            "Could not stop asking for host1: disk full. This answer started nothing; host1 may still ask before YOLO \
             launches."
        )
    );
    assert!(any_open(cx), "the question stayed up");
}

/// SPEC.md: the guarded answer starts "the same launch guarded instead, without YOLO confirmation", and it comes before
/// the YOLO answers. Specified: for Codex with YOLO chosen, the first extra answer is "Start guarded session instead",
/// and it sends a create with Guarded permissions and no consent.
#[gpui::test]
fn the_guarded_answer_launches_guarded_without_consent(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched_with(cx, yolo_codex_draft());
    refuse_for_yolo(&mut helm, cx, create.id);
    assert_eq!(
        name_of(cx, "prompt-alternative-0").as_deref(),
        Some("Start guarded session instead")
    );
    test_support::click(cx, "prompt-alternative-0");
    let sent = creates(&mut helm, cx);
    let [retry] = &sent[..] else {
        panic!("one create, not {}", sent.len());
    };
    let body = retry.body.as_ref().unwrap();
    assert_eq!(body["confirm_yolo"], json!(false));
    assert_eq!(body["launch"]["permissions"], json!("guarded"));
}

/// Cancel goes back to the launcher with nothing started (the old answer's help: "go back without starting
/// anything"). Specified: Cancel sends nothing, the launcher stays open, and the question goes.
#[gpui::test]
fn cancel_starts_nothing_and_keeps_the_launcher(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched_with(cx, yolo_command_draft());
    refuse_for_yolo(&mut helm, cx, create.id);
    test_support::click(cx, "prompt-cancel");
    assert!(creates(&mut helm, cx).is_empty(), "Cancel started nothing");
    assert!(any_open(cx), "the launcher stayed open");
    assert_eq!(name_of(cx, "prompt-confirm"), None, "the question went");
}

/// While the permanent answer's host write is out, the answer owns the launch: a press of Launch would send a create
/// without consent beside the one the answer sends with it, and closing the launcher would let the write change the
/// host's setting with nothing left to report it. An edit of the draft retires the question, so the write's success
/// must not launch a draft the user never confirmed. Specified: during the write, Launch sends nothing and Close and
/// Cancel leave the launcher open; after an edit of the name, the write's success sends no create.
#[gpui::test]
fn the_launcher_holds_still_while_stop_asking_is_out(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched_with(cx, yolo_command_draft());
    refuse_for_yolo(&mut helm, cx, create.id);
    test_support::click(cx, "prompt-alternative-0");
    cx.run_until_parked();
    let write = helm
        .commands()
        .pop()
        .expect("the premise: a command is out");
    assert_eq!(
        write.path, "/api/hosts/1/yolo-without-asking",
        "the premise: it is the host write"
    );
    test_support::click(cx, "launcher-launch");
    cx.run_until_parked();
    assert!(
        helm.commands().is_empty(),
        "Launch sent nothing while the write is out"
    );
    test_support::click(cx, "launcher-close");
    assert!(any_open(cx), "Close left the launcher open");
    test_support::click(cx, "launcher-cancel");
    assert!(any_open(cx), "Cancel left the launcher open");
    test_support::click(cx, "launcher-name-row");
    keys(cx, "n");
    assert_eq!(
        read(cx, |launcher, _, _| launcher.draft.name.value().clone()),
        "n",
        "the premise: the edit reached the draft"
    );
    helm.reply(write.id, 200, Some(json!({})));
    settle(cx);
    assert!(
        creates(&mut helm, cx).is_empty(),
        "after the edit retired the question, the write's success launched nothing"
    );
}

/// SPEC.md, "Creation": a launch opens its session only if the user "has pressed no key or pointer button anywhere in
/// the page since their last answer to it". For the permanent answer that answer is its press, not the host write's
/// reply, so a click made while the write is out counts. Specified: after "Start, and don't ask again", a click on the
/// Folder row during the write, then the write's and the create's success: the new session b is not opened.
#[gpui::test]
fn a_click_during_stop_asking_keeps_the_new_session_from_opening(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, create) = launched_with(cx, yolo_command_draft());
    refuse_for_yolo(&mut helm, cx, create.id);
    test_support::click(cx, "prompt-alternative-0");
    cx.run_until_parked();
    let write = helm
        .commands()
        .pop()
        .expect("the premise: a command is out");
    assert_eq!(
        write.path, "/api/hosts/1/yolo-without-asking",
        "the premise: it is the host write"
    );
    test_support::click(cx, "launcher-folder-row");
    helm.reply(write.id, 200, Some(json!({})));
    settle(cx);
    let sent = creates(&mut helm, cx);
    let [retry] = &sent[..] else {
        panic!("one create after the write, not {}", sent.len());
    };
    created(&mut helm, cx, retry.id, "b");
    assert_ne!(
        selected(cx).as_deref(),
        Some("b"),
        "the click during the write counted as moving on"
    );
}

// ---- The plain Replace question -------------------------------------------------------------------------------------

/// A page drawing the plain Replace question where the sidebar would: the question in a shared cell the test fills.
struct ReplacePage {
    root: crate::root::Attachment,
    shown: std::rc::Rc<std::cell::RefCell<Option<crate::prompt::Inline<super::entry::YoloAsk>>>>,
}

impl gpui::Render for ReplacePage {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::{ParentElement as _, Styled as _};
        let content = gpui::div()
            .size_full()
            .children(self.shown.borrow().clone());
        crate::root::wrap(content, &self.root, window, cx)
    }
}

/// What a plain Replace question's test watches: the retries it asked for, and whether the user had moved on since the
/// answer when the last one ran (what the caller would combine with its own Replace's flag).
struct Retries {
    count: std::rc::Rc<std::cell::Cell<u32>>,
    moved_on: std::rc::Rc<std::cell::Cell<bool>>,
}

/// The page over the fake helm with host1 and the plain Replace question asked for a command launch asserted YOLO, the
/// user's answer to Replace marked first; with `press_between`, a key is pressed after that answer and before the
/// question arrives, as when the user went on to something else while the Replace was out.
fn replace_question(
    cx: &mut gpui::TestAppContext,
    press_between: bool,
) -> (FakeHelm, &mut VisualTestContext, Retries) {
    let (mut helm, _connection) = test_support::started(cx);
    helm.connect(vec![super::view_tests::fixture_hosts(&[0])]);
    cx.run_until_parked();
    let shown = std::rc::Rc::default();
    let drawn = std::rc::Rc::clone(&shown);
    let (_, cx) = test_support::open_view(cx, move |window, cx| ReplacePage {
        root: crate::root::attach(window, cx),
        shown: drawn,
    });
    let retries = Retries {
        count: std::rc::Rc::default(),
        moved_on: std::rc::Rc::default(),
    };
    let (counted, moved) = (retries.count.clone(), retries.moved_on.clone());
    let launch = farhelm_proto::SessionLaunch::Command(farhelm_proto::CommandLaunch {
        command: "true".into(),
        yolo: true,
        agent: None,
        resume: None,
    });
    let mark = cx.update(|_, cx| crate::focus::Mark::take(cx));
    if press_between {
        keys(cx, "x");
    }
    cx.update(|window, cx| {
        let question = super::entry::ask_yolo_for_replace(
            1,
            crate::host_text::HostText::from_host("host1"),
            &launch,
            &mark,
            move |mark, _, cx| {
                counted.set(counted.get() + 1);
                moved.set(crate::focus::has_moved_on(&mark, cx));
            },
            window,
            cx,
        );
        shown.replace(Some(question));
    });
    test_support::redraw(cx);
    (helm, cx, retries)
}

/// SPEC.md, "Creation": for a plain Replace the GUI answers the helm's YOLO refusal with the same confirmation, "shown
/// with the control or surface that started the launch" (the sidebar's row, header or card), and with no guarded
/// answer, since plain Replace keeps the stored launch; Cancel takes focus when the user has not moved on. Specified:
/// the question opens focused on Cancel, says the command was asserted YOLO, offers the permanent answer first among
/// the extras and nothing after it, and the one-off answer asks the caller to retry once.
#[gpui::test]
fn the_plain_replace_question_offers_no_guarded_answer_and_retries(cx: &mut gpui::TestAppContext) {
    let (_helm, cx, retries) = replace_question(cx, false);
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    assert_eq!(
        name_of(cx, "yolo-reason").as_deref(),
        Some("This command was asserted to bypass its safety checks.")
    );
    assert_eq!(
        name_of(cx, "prompt-alternative-0").as_deref(),
        Some("Start, and don't ask again on this host")
    );
    assert_eq!(
        name_of(cx, "prompt-alternative-1"),
        None,
        "no guarded answer"
    );
    test_support::click(cx, "prompt-confirm");
    assert_eq!(retries.count.get(), 1, "the one-off answer retried once");
}

/// SPEC.md: the permanent answer "first sets the host to start YOLO sessions without asking ... and then retries with
/// the override"; for a plain Replace the retry is the sidebar's own. Specified: the permanent answer sends the host
/// settings' write for host1, asks for no retry until it is answered, then exactly one.
#[gpui::test]
fn the_plain_replace_permanent_answer_writes_then_retries(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, retries) = replace_question(cx, false);
    test_support::click(cx, "prompt-alternative-0");
    cx.run_until_parked();
    let write = helm
        .commands()
        .into_iter()
        .find(|command| command.path == "/api/hosts/1/yolo-without-asking")
        .expect("the host write was sent");
    assert_eq!(
        retries.count.get(),
        0,
        "no retry before the write is answered"
    );
    helm.reply(write.id, 200, Some(json!({})));
    settle(cx);
    assert_eq!(retries.count.get(), 1, "one retry after the write");
}

/// SPEC.md: "a plain Replace's question, which arrives after its reply, takes focus only if the user has not moved on
/// since answering Replace". Taking focus then would pull the user out of whatever they went on to. Specified: with a
/// key pressed after the answer to Replace, the question shows but focus does not go to its Cancel.
#[gpui::test]
fn the_plain_replace_question_leaves_focus_once_the_user_moved_on(cx: &mut gpui::TestAppContext) {
    let (_helm, cx, _retries) = replace_question(cx, true);
    assert_eq!(
        name_of(cx, "prompt-confirm").as_deref(),
        Some("Start YOLO session anyway"),
        "the premise: the question shows"
    );
    assert_ne!(
        focused_id(cx).as_deref(),
        Some("prompt-cancel"),
        "it took no focus"
    );
}

/// The permanent answer's retry runs from the host write's reply, after any press the user made meanwhile, so it hands
/// the caller the answer's mark: a click during the write must count as moving on for what the Replace may open, as it
/// does for the launcher's own launch. Specified: after the permanent answer, a click during the write; the retry,
/// once the write succeeds, sees that the user has moved on since the answer.
#[gpui::test]
fn the_plain_replace_retry_carries_the_answers_mark(cx: &mut gpui::TestAppContext) {
    let (mut helm, cx, retries) = replace_question(cx, false);
    test_support::click(cx, "prompt-alternative-0");
    cx.run_until_parked();
    let write = helm
        .commands()
        .into_iter()
        .find(|command| command.path == "/api/hosts/1/yolo-without-asking")
        .expect("the premise: the host write is out");
    keys(cx, "x");
    helm.reply(write.id, 200, Some(json!({})));
    settle(cx);
    assert_eq!(retries.count.get(), 1, "the premise: the retry ran");
    assert!(
        retries.moved_on.get(),
        "the key pressed during the write counted as moving on"
    );
}

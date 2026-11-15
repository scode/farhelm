//! Tests of Replace with in the real main view over the helm client's fake: what it opens with, the warning about its
//! source and the precondition a press sends, and how its reply settles, alike with plain Replace's.
//!
//! The fleet is the entry tests' (`entry_tests::window_with`): session `a` runs on host9 as its recorded install, and is
//! the open session unless a test opens another.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::draft::Mode;
use super::entry::open_replace_with;
use super::entry_tests::{destination, window_with};
use super::view_tests::{draft, name_of, shown_refusal};
use crate::local_state;
use crate::session_list::tests::{change, listing_meta, session};
use crate::test_support::{self, any_open, keys, settle};

/// Open Replace with of `key`, as the row menu's handler would, and settle.
fn replace_with(cx: &mut VisualTestContext, key: &str) -> bool {
    let opened = cx.update(|window, cx| open_replace_with(key, window, cx));
    settle(cx);
    opened
}

/// The Replace requests (`POST /api/sessions/{id}/replace`) the window sent since the last call. Every other command
/// is answered at once, so a Replace's reply can wait. The catalog read fails: the fleet's sessions carry the helm
/// fixture's Codex launch, whose stored model an empty catalog would refuse, and an unread catalog refuses no choice.
fn replaces(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    cx.run_until_parked();
    let mut sent = Vec::new();
    for command in helm.commands() {
        if command.path.ends_with("/replace") {
            sent.push(command);
        } else if command.path.ends_with("launch-catalog") {
            helm.reply(
                command.id,
                503,
                Some(json!("catalog unavailable in this test")),
            );
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    settle(cx);
    sent
}

/// Press Launch on the open Replace with and return the one Replace it sent, checking that it sent exactly one and hid.
fn pressed(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> FakeCommand {
    assert!(
        replaces(helm, cx).is_empty(),
        "the premise: nothing sent yet"
    );
    test_support::click(cx, "launcher-launch");
    let sent = replaces(helm, cx);
    let [replace] = &sent[..] else {
        panic!(
            "one press sent one Replace, not {} (the launcher says {:?})",
            sent.len(),
            name_of(cx, "launcher-refusal")
        );
    };
    assert!(!any_open(cx), "the launcher hid while its Replace is out");
    replace.clone()
}

/// Make the helm list `a` exited with no terminal tab (nothing alive), as a push while the launcher stays open. The
/// fixture's row lists one tab, so the push clears them.
fn a_exits(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    let exited = session("a")
        .host(9)
        .status(json!({"state": "exited", "exit_code": 0}));
    let mut pushed = change(
        listing_meta(&["a", "b", "c", "d"], 5, false),
        &[exited],
        &[],
    );
    pushed.upsert[0].value["tabs"] = json!([]);
    helm.push(pushed);
    settle(cx);
}

/// SPEC.md, "Replace with": the launcher "pre-filled with the source's host, existing directory, title, and launch",
/// keeping "the source's own host — clone is the way to start a session on a different host". Starting it elsewhere
/// would be refused by the helm, so the host must be held. Specified: Replace with of a opens in Replace with mode on
/// host9, held, in a's folder, named "a".
#[gpui::test]
fn replace_with_prefills_the_source_and_holds_its_host(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("a"));
    assert!(replace_with(cx, "a"), "Replace with opened");
    let draft = draft(cx);
    assert_eq!(draft.mode, Mode::ReplaceWith { source: "a".into() });
    assert_eq!(destination(cx), (Some(9), "/home/user/src/a".to_owned()));
    assert_eq!(draft.name.value(), "a");
    assert!(
        !test_support::frame(cx)
            .iter()
            .any(|node| node.element_id.as_deref() == Some("launcher-host")),
        "the host picker is not offered; the host is held"
    );
}

/// The launch button is the confirmation of the source's delete, so it must say what that delete stops, and the request
/// must authorize only that (SPEC.md, "Replace with"; the old `replace_with_warning`). Specified: with a running, the
/// launcher shows Replace's warning ending "the session launched here takes its place.", and Launch sends one
/// `POST /api/sessions/a/replace` whose create targets host9 in a's folder and whose guard is unconditional (neither
/// precondition flag), the progress pill then saying "Replacing".
#[gpui::test]
fn replace_with_warns_about_a_running_source_and_sends_its_guard(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(replace_with(cx, "a"));
    let warning = name_of(cx, "launcher-source-warning").unwrap_or_default();
    // The fixture's row lists one open tab, which the warning names first, as plain Replace's question does.
    assert!(
        warning.contains("still running — replacing kills the agent")
            && warning.ends_with("the session launched here takes its place."),
        "{warning}"
    );
    let replace = pressed(&mut helm, cx);
    assert_eq!(replace.path, "/api/sessions/a/replace");
    let body = replace.body.expect("a Replace carries a body");
    assert_eq!(body["with"]["host"], json!(9));
    assert_eq!(body["with"]["cwd"], json!("/home/user/src/a"));
    assert_eq!(body["only_if_nothing_alive"], json!(false));
    assert_eq!(body["only_if_agent_ended"], json!(false));
    test_support::advance(cx, std::time::Duration::from_millis(2_100));
    assert_eq!(
        name_of(cx, "launch-progress").as_deref(),
        Some("Replacing “a” on host9…")
    );
    // The source's row and header show the same wait as for a plain Replace.
    assert_eq!(
        cx.update(|_, cx| {
            crate::session_list::outcome::waited_seconds("a", crate::operations::Work::Replace, cx)
        }),
        Some(2)
    );
}

/// The warning follows the source while the launcher stays open, and the guard matches what is drawn at the press, so a
/// source that ended meanwhile is replaced only if it is still ended (SPEC.md: "launching carries the precondition
/// matching what the launcher showed at the click"). Specified: once a is listed exited with no tabs, the warning is
/// gone and a press sends `only_if_nothing_alive`.
#[gpui::test]
fn replace_with_follows_the_source_and_sends_the_guard_it_shows(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(replace_with(cx, "a"));
    assert!(
        name_of(cx, "launcher-source-warning").is_some(),
        "the premise: a runs"
    );
    a_exits(&mut helm, cx);
    assert_eq!(name_of(cx, "launcher-source-warning"), None);
    let body = pressed(&mut helm, cx).body.expect("a body");
    assert_eq!(body["only_if_nothing_alive"], json!(true));
}

/// The helm lists `replacement` on host9 in `source`'s place (it creates the replacement and deletes the source
/// before it answers), then answers Replace `id` with the replacement and no delete notice. `order` is the listing
/// after the swap.
fn replaced(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    id: u64,
    source: &str,
    replacement: &'static str,
    order: &[&str],
) {
    helm.push(change(
        listing_meta(order, order.len() as u64, false),
        &[session(replacement).host(9)],
        &[source],
    ));
    settle(cx);
    helm.reply(
        id,
        200,
        // The reply is the replacement's own session with no delete notice beside its fields (`SessionReplaceReply`).
        Some(session(replacement).host(9).entry().value),
    );
    settle(cx);
}

/// The window's open session.
fn selected(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| local_state::selected(cx))
}

/// Replace with has "exactly Replace's create-then-delete contract": its replacement opens where plain Replace's does,
/// only while its source is still the open session, which the selection rules keep open through the helm's removal of
/// it until the reply. Specified: with a open, a success whose listing has already dropped a, and lists a2 last (so a
/// fallback selection after a went would pick another row), opens the replacement a2;
/// then, a Replace with of a2 answered after c was opened (with no key press, so only the open session differs), the
/// success leaves c open.
#[gpui::test]
fn replace_with_opens_its_replacement_only_where_the_source_was_open(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(replace_with(cx, "a"));
    let replace = pressed(&mut helm, cx);
    replaced(&mut helm, cx, replace.id, "a", "a2", &["b", "c", "d", "a2"]);
    assert_eq!(selected(cx).as_deref(), Some("a2"));

    assert!(replace_with(cx, "a2"));
    let replace = pressed(&mut helm, cx);
    cx.update(|_, cx| local_state::select("c", cx));
    settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("c"), "the premise: c is open");
    replaced(
        &mut helm,
        cx,
        replace.id,
        "a2",
        "a3",
        &["b", "c", "d", "a3"],
    );
    assert_eq!(selected(cx).as_deref(), Some("c"));
}

/// The YOLO question belongs to the launcher, not to the source: a host that asks before YOLO launches refuses the
/// Replace with until the user answers, and that refusal must not show on the source's row and header as a failed
/// Replace, before or after a Cancel (the old `nonblocking-launch.spec.ts` case "Replace with YOLO cancellation leaves
/// no source failure"). Replace with's warning stays under the question, since its answers launch with the warning's
/// guard. Specified: the refusal brings the launcher back with the question and the warning; the store holds no line
/// for a then, nor after Cancel.
#[gpui::test]
fn the_yolo_question_of_a_replace_with_leaves_no_failure_on_the_source(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(replace_with(cx, "a"));
    let replace = pressed(&mut helm, cx);
    helm.reply_marked(
        replace.id,
        409,
        vec![crate::operations::HelmMarker::YoloConfirmation],
        Some(json!("YOLO launches on host9 need --yolo")),
    );
    settle(cx);
    assert!(any_open(cx), "the premise: the launcher is back");
    assert!(
        test_support::frame(cx)
            .iter()
            .any(|node| node.element_id.as_deref() == Some("prompt-cancel")),
        "the premise: it asks the question"
    );
    assert!(
        name_of(cx, "launcher-source-warning").is_some(),
        "the warning stays beside the question"
    );
    let line = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| crate::session_list::outcome::current_line("a", cx))
    };
    assert_eq!(line(cx), None, "no failure on the source while asked");
    test_support::click(cx, "prompt-cancel");
    assert_eq!(line(cx), None, "nor after Cancel");
}

/// A refused Replace with comes back in the launcher with its draft, as any refused launch does, and the source's row
/// keeps the same words through the operation store, as a refused plain Replace's does (SPEC.md: "the same failure
/// reporting"). Specified: a 409 with the helm's words brings the launcher back showing "Not launched. <words>", and the
/// store's line for a is that Replace refusal.
#[gpui::test]
fn a_refused_replace_with_comes_back_and_stays_on_the_source(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(replace_with(cx, "a"));
    let replace = pressed(&mut helm, cx);
    helm.reply(replace.id, 409, Some(json!("a is busy on the host")));
    settle(cx);
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. a is busy on the host")
    );
    let line = cx.update(|_, cx| crate::session_list::outcome::current_line("a", cx));
    let line = line.expect("the source's row has a line");
    assert_eq!(line.work, crate::operations::Work::Replace);
    assert_eq!(line.message.raw(), "a is busy on the host");
}

/// A Replace with shares the source's key with every other request about the source, and the operation store admits
/// one at a time, so Launch waits while the source has another request out rather than sending one the store turns
/// away. Specified: with a plain Replace of a in flight, a press of Launch in Replace with of a sends nothing and says
/// "Wait for this session's other operation to finish".
#[gpui::test]
fn replace_with_waits_while_its_source_is_busy(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(replace_with(cx, "a"));
    cx.update(|window, cx| {
        crate::session_list::replace::send(
            "a",
            farhelm_proto::DeleteGuard::Unconditional,
            crate::session_list::replace_yolo::Surface::Row,
            window.window_handle(),
            cx,
        )
    });
    let busy = replaces(&mut helm, cx);
    assert_eq!(busy.len(), 1, "the premise: a plain Replace of a is out");
    test_support::click(cx, "launcher-launch");
    assert!(replaces(&mut helm, cx).is_empty(), "nothing more was sent");
    assert!(any_open(cx), "the launcher stays");
    let said = name_of(cx, "launcher-refusal").unwrap_or_default();
    assert!(
        said.contains("Wait for this session's other operation to finish"),
        "{said}"
    );
}

/// Replace with deletes its source only through its launch button; anything else leaves the source alone (the old
/// `replace.spec.ts` case "cancelling a replace-with composer leaves the source untouched and creates nothing").
/// Specified: Replace with of a closed by Escape sends nothing, and a stays the open session.
#[gpui::test]
fn cancelling_replace_with_sends_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(replace_with(cx, "a"));
    keys(cx, "escape");
    assert!(!any_open(cx), "the premise: Escape closed it");
    assert!(replaces(&mut helm, cx).is_empty());
    assert_eq!(selected(cx).as_deref(), Some("a"));
}

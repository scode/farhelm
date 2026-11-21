//! Tests of Replace with in the real main view over the helm client's fake: what it opens with, the warning about its
//! source and the precondition a press sends, and how its reply settles, alike with plain Replace's.
//!
//! The fleet is the entry tests' (`entry_tests::window_with`): session `a` runs on host9 as its recorded install, and is
//! the open session unless a test opens another.

use std::time::Duration;

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::draft::Mode;
use super::entry::open_replace_with;
use super::entry_tests::{destination, environment_window, window_with};
use super::folder_default_tests::edit_draft;
use super::view_tests::{Field, click_in_body, draft, focused, name_of, shown_refusal};
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

/// Give the live Replace source a checkout whose working directory is below
/// its root. The hint follows the directory the launch would actually use.
fn source_in_checkout(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    key: &'static str,
    host: i64,
    cwd: &str,
) {
    let source = session(key).host(host).cwd(cwd);
    let mut pushed = change(
        listing_meta(&["a", "b", "c", "d"], 5, false),
        &[source],
        &[],
    );
    pushed.upsert[0].value["host_identity"] = json!(format!("identity{host}"));
    pushed.upsert[0].value["canonical_cwd"] = json!("/work/widget/lib");
    pushed.upsert[0].value["working_copy"] = json!({
        "id": "checkout-1",
        "repo": {"owner": "acme", "name": "widget"},
        "canonical_path": "/work/widget",
        "origin_session_id": key
    });
    helm.push(pushed);
    settle(cx);
}

/// Answer the existing membership-only check, without browsing the directory.
/// The launcher must get the answer about the path it will actually send.
fn answer_folder_check(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    path: &str,
    resolved: &str,
) {
    assert!(replaces(helm, cx).is_empty());
    test_support::advance(cx, Duration::from_millis(160));
    let commands = helm.commands();
    let [check] = &commands[..] else {
        panic!("expected just the folder check, got {commands:?}");
    };
    assert_eq!(check.path, "/api/browse-directory");
    let body = check.body.as_ref().expect("the folder check sends a path");
    assert_eq!(body["cwd"], json!(path));
    assert_eq!(body["membership_only"], json!(true));
    helm.reply(
        check.id,
        200,
        Some(json!({"cwd": resolved, "parent": null, "children": [], "truncated": false})),
    );
    settle(cx);
}

/// Keeping the source's subdirectory also keeps its checkout changes. The
/// mark and named help disappear when the Folder field is edited elsewhere
/// and return for the canonical spelling Browse would use.
#[gpui::test]
fn replace_with_explains_the_sources_checkout_under_folder(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    source_in_checkout(&mut helm, cx, "a", 9, "/linked/widget/lib");
    assert!(replace_with(cx, "a"));
    assert_eq!(destination(cx), (Some(9), "/linked/widget/lib".into()));
    let folder = "launcher-replacement-checkout-folder";
    let mark = "launcher-replacement-checkout-mark";
    let help = "This session's managed checkout of acme/widget. The replacement keeps working in it, with its changes.";
    assert_eq!(name_of(cx, folder), None, "the source alias needs checking");
    answer_folder_check(&mut helm, cx, "/linked/widget/lib", "/work/widget/lib");
    assert_eq!(name_of(cx, folder).as_deref(), Some(help));
    assert_eq!(
        name_of(cx, mark).as_deref(),
        Some("Managed checkout of acme/widget")
    );

    click_in_body(cx, "launcher-folder");
    assert!(focused(cx, Field::Folder), "the edit goes to Folder");
    keys(cx, "end");
    cx.simulate_input("/tmp");
    settle(cx);
    assert_eq!(draft(cx).folder.value(), "/linked/widget/lib/tmp");
    assert_eq!(name_of(cx, folder), None);
    assert_eq!(name_of(cx, mark), None);

    crate::test_support::select_all_text(cx);
    cx.simulate_input("/work/widget/lib");
    settle(cx);
    assert_eq!(draft(cx).folder.value(), "/work/widget/lib");
    assert_eq!(name_of(cx, folder).as_deref(), Some(help));
    assert_eq!(
        name_of(cx, mark).as_deref(),
        Some("Managed checkout of acme/widget")
    );
}

/// A source's recorded alias can now name some other directory. In that
/// case leaving Folder as it opened would not preserve the source checkout.
#[gpui::test]
fn replace_with_does_not_promise_a_repointed_checkout_folder(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    source_in_checkout(&mut helm, cx, "a", 9, "/linked/widget/lib");
    assert!(replace_with(cx, "a"));
    assert_eq!(destination(cx), (Some(9), "/linked/widget/lib".into()));
    answer_folder_check(&mut helm, cx, "/linked/widget/lib", "/unrelated/lib");
    assert_eq!(name_of(cx, "launcher-replacement-checkout-folder"), None);
    assert_eq!(name_of(cx, "launcher-replacement-checkout-mark"), None);
}

/// The new clone can use another repository. The warning still names the
/// source's checkout and explains why its edits could end up in the trash.
/// Returning to Folder puts the checkout's own hint back.
#[gpui::test]
fn replace_with_warns_where_about_the_sources_checkout(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    source_in_checkout(&mut helm, cx, "a", 9, "/work/widget/lib");
    assert!(replace_with(cx, "a"));
    let folder = "launcher-replacement-checkout-folder";
    let warning = "launcher-replacement-checkout-warning";
    assert!(name_of(cx, folder).is_some(), "the source has a checkout");
    assert_eq!(name_of(cx, warning), None);
    test_support::click(cx, "launcher-destination-Managed checkout");
    edit_draft(cx, |draft| {
        draft.choose_repository(farhelm_proto::parse_github_repo("other/repo").unwrap());
    });
    assert_eq!(
        draft(cx).checkout.and_then(|checkout| checkout.repo),
        Some(farhelm_proto::parse_github_repo("other/repo").unwrap()),
        "the replacement's repository differs from the source's"
    );
    assert_eq!(name_of(cx, folder), None);
    assert_eq!(
        name_of(cx, warning).as_deref(),
        Some(
            "Starts from a fresh clone without the changes in this session's checkout of acme/widget. If this replaces its last session, the old checkout moves to the trash."
        )
    );
    test_support::click(cx, "launcher-destination-Folder");
    settle(cx);
    assert!(name_of(cx, folder).is_some());
    assert_eq!(name_of(cx, warning), None);
}

/// Replacing into a fresh checkout in the source's own sandbox releases the
/// old checkout through Replace's usual archive path. The trash remains in
/// the sandbox and goes away when the sandbox is deleted.
#[gpui::test]
fn replace_with_warns_that_the_checkouts_trash_stays_inside_the_sandbox(cx: &mut TestAppContext) {
    let (mut helm, cx) = environment_window(cx, "c", "web");
    source_in_checkout(&mut helm, cx, "c", 11, "/work/widget/lib");
    assert!(replace_with(cx, "c"));
    assert_eq!(draft(cx).host, Some(11));
    assert!(name_of(cx, "launcher-replacement-checkout-folder").is_some());
    test_support::click(cx, "launcher-destination-Managed checkout");
    settle(cx);
    assert!(draft(cx).checkout.is_some());
    assert_eq!(
        name_of(cx, "launcher-replacement-checkout-warning").as_deref(),
        Some(
            "Starts from a fresh clone without the changes in this session's checkout of acme/widget. If this replaces its last session, the old checkout moves to the trash inside the sandbox or container. Deleting the sandbox or container also deletes its trash."
        )
    );
}

/// Launch provenance alone does not tell us that the source still uses a
/// managed checkout. Offer only the ordinary Folder controls in this case.
#[gpui::test]
fn replace_with_of_an_unmanaged_source_has_no_checkout_hint(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    let source = session("a").host(9).unmanaged();
    helm.push(change(
        listing_meta(&["a", "b", "c", "d"], 5, false),
        &[source],
        &[],
    ));
    settle(cx);
    assert!(replace_with(cx, "a"));
    assert_eq!(name_of(cx, "launcher-replacement-checkout-folder"), None);
    assert_eq!(name_of(cx, "launcher-replacement-checkout-mark"), None);
    test_support::click(cx, "launcher-destination-Managed checkout");
    settle(cx);
    assert!(draft(cx).checkout.is_some());
    assert_eq!(name_of(cx, "launcher-replacement-checkout-warning"), None);
}

/// The launch button is the confirmation of the source's delete, so it must say what that delete stops, and the request
/// must authorize only that (SPEC.md, "Replace with"; the old `replace_with_warning`). Specified: with a running, the
/// launcher shows Replace's warning ending "the session launched here takes its place.", and Launch sends one
/// `POST /api/sessions/a/replace` whose create targets host9 in a's folder and whose guard is unconditional (neither
/// precondition flag), the progress pill then saying "Replacing". If the helm asks about YOLO, the permanent answer
/// takes the warning from that press so a later restart cannot broaden what the host-setting reply authorizes.
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
    helm.reply_marked(
        replace.id,
        409,
        vec![crate::operations::HelmMarker::YoloConfirmation],
        Some(json!("YOLO launches on host9 need --yolo")),
    );
    settle(cx);
    a_exits(&mut helm, cx);
    assert_eq!(name_of(cx, "launcher-source-warning"), None);
    assert!(replaces(&mut helm, cx).is_empty());
    test_support::click(cx, "prompt-alternative-1");
    cx.run_until_parked();
    let sent = helm.commands();
    let [write] = &sent[..] else {
        panic!("expected only the host setting, got {sent:?}");
    };
    assert_eq!(write.path, "/api/hosts/9/yolo-without-asking");
    helm.push(change(
        listing_meta(&["a", "b", "c", "d"], 5, false),
        &[session("a").host(9)],
        &[],
    ));
    settle(cx);
    assert!(name_of(cx, "launcher-source-warning").is_some());
    helm.reply(write.id, 200, Some(json!({})));
    let [retry] = &replaces(&mut helm, cx)[..] else {
        panic!("one retry")
    };
    assert_eq!(
        retry.body.as_ref().unwrap()["only_if_nothing_alive"],
        json!(true)
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
/// after the swap. The push must keep the current selection until the success has settled.
fn replaced(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    id: u64,
    source: &str,
    replacement: &'static str,
    order: &[&str],
) {
    let before = selected(cx);
    helm.push(change(
        listing_meta(order, order.len() as u64, false),
        &[session(replacement).host(9)],
        &[source],
    ));
    settle(cx);
    assert_eq!(
        selected(cx),
        before,
        "the replacement is listed before the reply"
    );
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
/// it until the reply. With a open, typing after submission leaves it selected. A success whose listing drops a
/// and lists a2 last (so fallback would pick another row) still opens a2;
/// then, a Replace with of a2 answered after c was opened (with no key press, so only the open session differs), the
/// success leaves c open.
#[gpui::test]
fn replace_with_opens_its_replacement_only_where_the_source_was_open(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(replace_with(cx, "a"));
    let replace = pressed(&mut helm, cx);
    keys(cx, "x");
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "typing keeps the source open"
    );
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

/// The completion of Replace with leaves the keyboard with the terminal the user actually switched to.
#[gpui::test]
fn replace_with_leaves_the_keyboard_in_the_session_switched_to(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(replace_with(cx, "a"));
    let replace = pressed(&mut helm, cx);
    test_support::click(cx, "session-row-c");
    assert_eq!(selected(cx).as_deref(), Some("c"), "the premise: c is open");
    assert!(
        test_support::terminal_focused(cx),
        "c's terminal has the keyboard"
    );
    replaced(&mut helm, cx, replace.id, "a", "a2", &["b", "c", "d", "a2"]);
    assert!(
        cx.update(|_, cx| super::entry::new_available(cx)),
        "the reply freed the launcher"
    );
    assert_eq!(selected(cx).as_deref(), Some("c"));
    assert!(
        test_support::terminal_focused(cx),
        "the keyboard stayed in c's terminal"
    );
}

/// A definite missing-folder refusal leaves the original destination eligible for another press once it has been
/// repaired outside the launcher. The fake scripts the host's changed answer without requiring a draft edit.
#[gpui::test]
fn replace_with_can_retry_the_unchanged_draft_after_a_folder_refusal(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(replace_with(cx, "a"));
    let folder_error = "working directory does not exist: /home/user/src/a";
    let first = pressed(&mut helm, cx);
    helm.reply_marked(
        first.id,
        409,
        vec![crate::operations::HelmMarker::CreateUnaccepted],
        Some(json!(folder_error)),
    );
    settle(cx);
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some(format!("Not launched. {folder_error}").as_str())
    );
    assert!(name_of(cx, "launcher-folder-problem").is_some());
    let retry = pressed(&mut helm, cx);
    assert_eq!(
        retry.body, first.body,
        "the retry keeps the original folder and launch choices"
    );
    replaced(&mut helm, cx, retry.id, "a", "a2", &["b", "c", "d", "a2"]);
    assert_eq!(selected(cx).as_deref(), Some("a2"));
}

/// A complete fleet push may remove the source before Replace with is refused. Keep the source selected while
/// the request waits, then show the refusal and fall back by visit history rather than listing order.
#[gpui::test]
fn a_refused_replace_with_falls_back_after_the_source_disappears(cx: &mut TestAppContext) {
    use crate::session_list::tests::preferences;
    use farhelm_proto::ui_channel::CollectionChanges;

    let (mut helm, cx) = window_with(cx, Some("a"));
    let history = preferences(&["a", "c"], Some("a"));
    helm.push(CollectionChanges {
        collection: history.collection,
        meta: history.meta,
        upsert: history.entries,
        remove: Vec::new(),
    });
    settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("a"));
    assert!(replace_with(cx, "a"));
    let replace = pressed(&mut helm, cx);
    helm.push(change(
        listing_meta(&["b", "c", "d"], 3, false),
        &[],
        &["a"],
    ));
    settle(cx);
    assert!(!crate::session_list::menu_tests::drawn(cx, "session-row-a"));
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "the unanswered Replace keeps its source selected"
    );

    helm.reply_marked(
        replace.id,
        403,
        vec![crate::operations::HelmMarker::CreateUnaccepted],
        Some(json!("replacement refused after external removal")),
    );
    settle(cx);
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. replacement refused after external removal")
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
    a_exits(&mut helm, cx);
    assert_eq!(name_of(cx, "launcher-source-warning"), None);
    test_support::click(cx, "prompt-cancel");
    assert!(replaces(&mut helm, cx).is_empty(), "Cancel sends nothing");
    assert!(name_of(cx, "prompt-cancel").is_none());
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

// ---- A timeout, and a refusal before sending ---------------------------------------------------------------------------

/// A Replace with whose reply is the host's timeout may still replace its source: its row says so once the reply
/// comes, even with the launcher out of sight, and the launcher, brought back from its notice, shows the helm's words
/// without "Not launched." (nonblocking-launch.spec's "Replace with settles its unknown before its launcher can return").
/// Specified: a key press after Launch, then a 504 with the bounded route's words: no launcher opens, and the store's
/// line for a is a Replace with those words; Open launcher brings the launcher back showing them alone.
#[gpui::test]
fn a_timed_out_replace_with_settles_its_row_before_the_launcher_returns(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(replace_with(cx, "a"));
    let replace = pressed(&mut helm, cx);
    super::launch_tests::times_out_after_moving_on(
        &mut helm,
        cx,
        replace.id,
        "a",
        crate::operations::Work::Replace,
    );
}

/// Whether a's row reports the refused Stop of the next test.
fn reports_the_stop(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| crate::session_list::outcome::current_line("a", cx))
        .is_some_and(|line| {
            line.work == crate::operations::Work::Stop && line.message.raw() == "stop refused here"
        })
}

/// A Replace with refused before anything is sent leaves the source's row as it was, an earlier Stop's report included
/// (nonblocking-launch.spec's "a locally refused Replace with preserves the source's earlier Stop report"). Why: a press
/// that sent nothing has no outcome of its own to put on the row. Specified: a's Stop, from its row menu, is refused
/// with "stop refused here"; then Replace with of a, given a wrapper prefix that does not parse, is pressed: nothing is
/// sent, the launcher says why, and a's line is still that Stop refusal.
#[gpui::test]
fn a_locally_refused_replace_with_keeps_the_sources_stop_report(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    replaces(&mut helm, cx);
    test_support::click(cx, "session-menu-a");
    test_support::click(cx, "session-menu-stop");
    // a's agent is working, so its Stop asks first; the answer is not what is under test.
    test_support::click(cx, "prompt-confirm");
    cx.run_until_parked();
    let mut stopped = false;
    for command in helm.commands() {
        if command.path == "/api/sessions/a/stop" {
            helm.reply(command.id, 409, Some(json!("stop refused here")));
            stopped = true;
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    settle(cx);
    assert!(stopped, "the premise: a's Stop was sent");
    assert!(
        reports_the_stop(cx),
        "the premise: a's row reports its Stop"
    );

    assert!(replace_with(cx, "a"));
    edit_draft(cx, |draft| draft.edit_wrapper("'unclosed".into()));
    test_support::click(cx, "launcher-launch");
    cx.run_until_parked();
    let launches: Vec<String> = helm
        .commands()
        .into_iter()
        .filter(|command| command.method == farhelm_helm_client::Method::Post)
        .map(|command| command.path)
        .filter(|path| {
            path == "/api/sessions"
                || path.ends_with("/replace")
                || path.ends_with("/resume")
                || path.contains("/environment")
        })
        .collect();
    assert_eq!(launches, Vec::<String>::new(), "nothing was sent");
    assert!(
        shown_refusal(cx).is_some_and(|refusal| refusal.contains("wrapper prefix")),
        "the launcher says why: {:?}",
        shown_refusal(cx)
    );
    assert!(reports_the_stop(cx), "a's row still reports its Stop");
}

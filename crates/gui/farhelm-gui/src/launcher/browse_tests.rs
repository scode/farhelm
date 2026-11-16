//! Tests of Browse in the real launcher over the helm client's fake: what it asks the host, what its listing offers,
//! what taking an entry does, when a listing is no longer shown, and where Browse is not offered.
//!
//! Ported from the old `e2e/tests/sidebar.spec.ts` ("composer path actions keep typing inert and browse the selected
//! remote host", "composer rejects held and queued stale browse activation after destination changes", and Browse's
//! place in "composer destination block keeps host, browse, folder, and grouped choices together") and `launcher-enter.spec.ts` ("launcher action-button Enter keeps reset browse and cancel") at
//! reference commit `24bc0580`.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::draft::{DestinationKind, LaunchDraft};
use super::view_tests::{draft, fixture_hosts, launcher_with_hosts, name_of};
use crate::test_support::{self, keys, settle};

/// New on host1 in `/tmp`.
fn new_in_tmp(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "/tmp".into(), None),
        fixture_hosts(&[0]),
    )
}

/// Answer, in order, every command the window sent since the last call, except the browse request (a
/// `POST /api/browse-directory` that is not the folder check's `membership_only` one), which is returned unanswered.
/// Everything else gets no body.
fn browse_request(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Option<FakeCommand> {
    cx.run_until_parked();
    let mut browse = None;
    for command in helm.commands() {
        let is_browse = command.path == "/api/browse-directory"
            && command
                .body
                .as_ref()
                .is_some_and(|body| body["membership_only"] == json!(false));
        if is_browse && browse.is_none() {
            browse = Some(command);
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    // The answers can change what is drawn (a failed read's line), so a later click finds its target where it is now.
    settle(cx);
    browse
}

/// `/tmp`'s listing on the host: its parent, two folders, and more than were listed.
fn tmp_listing() -> Value {
    json!({
        "cwd": "/tmp",
        "parent": "/",
        "children": ["/tmp/a", "/tmp/b\u{202e}"],
        "truncated": true,
    })
}

/// Browse asks the chosen host, on its connection, for the folder as typed, and lists what it answers: the folder
/// itself, its parent, that there are more, then each folder (a host value drawn escaped); taking one makes it the
/// folder as the user's own choice and closes the listing. Browse sits in the Host row, beside the picker. Why: SPEC.md, "explicit browsing
/// asks the selected supervisor for one bounded directory level" (old "composer path actions … browse the selected
/// remote host" and "launcher action-button Enter keeps reset browse and cancel").
#[gpui::test]
fn browse_lists_one_level_and_taking_a_folder_chooses_it(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_in_tmp(cx);
    assert!(
        browse_request(&mut helm, cx).is_none(),
        "the premise: opening asks nothing"
    );
    let (button, host_row) = (
        test_support::center(cx, "launcher-browse"),
        test_support::bounds(cx, "launcher-host-row"),
    );
    assert!(host_row.contains(&button), "Browse sits in the Host row");
    test_support::click(cx, "launcher-browse");
    let request = browse_request(&mut helm, cx).expect("Browse asked the host");
    assert_eq!(
        request.body,
        Some(json!({
            "host": 1,
            "cwd": "/tmp",
            "expected_incarnation": 4,
            "membership_only": false,
        }))
    );
    helm.reply(request.id, 200, Some(tmp_listing()));
    settle(cx);
    assert_eq!(
        name_of(cx, "launcher-browse-use").as_deref(),
        Some("Use /tmp")
    );
    assert_eq!(
        name_of(cx, "launcher-browse-parent").as_deref(),
        Some("Parent: /")
    );
    assert!(name_of(cx, "launcher-browse-truncated").is_some());
    assert_eq!(name_of(cx, "launcher-browse-0").as_deref(), Some("/tmp/a"));
    assert_eq!(
        name_of(cx, "launcher-browse-1").as_deref(),
        Some("/tmp/b<U+202E>"),
        "a host value is shown escaped"
    );
    test_support::click(cx, "launcher-browse-1");
    let taken = draft(cx);
    assert_eq!(
        taken.folder.value(),
        "/tmp/b\u{202e}",
        "the host's own spelling is taken"
    );
    assert!(taken.folder.is_touched(), "as the user's own choice");
    assert_eq!(
        name_of(cx, "launcher-browse-use"),
        None,
        "the listing closes"
    );
    assert!(
        browse_request(&mut helm, cx).is_none(),
        "and taking it asks nothing more"
    );
}

/// A host that cannot list the folder answers with its words, shown in place of the listing. Why: the old launcher
/// showed the helm's refusal as Browse's error line rather than disabling Browse for a host that is not connected.
#[gpui::test]
fn the_hosts_refusal_is_shown_in_place_of_the_listing(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_in_tmp(cx);
    test_support::click(cx, "launcher-browse");
    let request = browse_request(&mut helm, cx).expect("Browse asked the host");
    helm.reply(
        request.id,
        409,
        Some(json!("working directory does not exist: /tmp")),
    );
    settle(cx);
    assert!(
        name_of(cx, "launcher-browse-error").is_some_and(|words| words.contains("does not exist")),
        "the host's words"
    );
    assert_eq!(name_of(cx, "launcher-browse-use"), None);
}

/// A listing is about the folder it was asked for. An answer that arrives after the folder changed and changed back
/// is dropped all the same, and a listing hidden by an edit does not come back when the edit is undone. Why: taking an
/// entry of a listing for another moment's folder would choose a folder the user is no longer looking at (old
/// "composer rejects held and queued stale browse activation after destination changes").
#[gpui::test]
fn an_edit_of_the_folder_forgets_the_listing(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_in_tmp(cx);
    test_support::click(cx, "launcher-browse");
    let request = browse_request(&mut helm, cx).expect("Browse asked the host");
    test_support::click(cx, "launcher-folder-row");
    keys(cx, "x");
    keys(cx, "backspace");
    assert_eq!(
        draft(cx).folder.value(),
        "/tmp",
        "the premise: back on the folder asked about"
    );
    helm.reply(request.id, 200, Some(tmp_listing()));
    settle(cx);
    assert_eq!(
        name_of(cx, "launcher-browse-use"),
        None,
        "an answer from before the change is dropped"
    );

    test_support::click(cx, "launcher-browse");
    let request = browse_request(&mut helm, cx).expect("Browse asked again");
    helm.reply(request.id, 200, Some(tmp_listing()));
    settle(cx);
    assert!(
        name_of(cx, "launcher-browse-use").is_some(),
        "the premise: listed"
    );
    test_support::click(cx, "launcher-folder-row");
    keys(cx, "x");
    assert_eq!(name_of(cx, "launcher-browse-use"), None, "an edit hides it");
    keys(cx, "backspace");
    assert_eq!(
        name_of(cx, "launcher-browse-use"),
        None,
        "and undoing the edit does not bring it back"
    );
}

/// A reconnect of the host forgets the listing: it was a look at the host on its earlier connection. Why: the old
/// listing was "a live observation" that expired on a reconnect.
#[gpui::test]
fn a_reconnect_forgets_the_listing(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_in_tmp(cx);
    test_support::click(cx, "launcher-browse");
    let request = browse_request(&mut helm, cx).expect("Browse asked the host");
    helm.reply(request.id, 200, Some(tmp_listing()));
    settle(cx);
    assert!(
        name_of(cx, "launcher-browse-use").is_some(),
        "the premise: listed"
    );
    let mut hosts = fixture_hosts(&[0]);
    let incarnation = hosts.entries[0].value["incarnation"]
        .as_u64()
        .expect("the premise: the fixture host has a connection");
    hosts.entries[0].value["incarnation"] = json!(incarnation + 1);
    helm.push(farhelm_proto::ui_channel::CollectionChanges {
        collection: hosts.collection,
        meta: hosts.meta,
        upsert: hosts.entries,
        remove: Vec::new(),
    });
    settle(cx);
    assert_eq!(name_of(cx, "launcher-browse-use"), None);
}

/// The search's "Browse this path" row makes the typed path the folder and browses it. Why: it is the keyboard's way
/// to Browse, after "Use this path" (old `ComposerSearchResult::BrowsePath`).
#[gpui::test]
fn browse_this_path_from_search_sets_the_folder_and_browses_it(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_in_tmp(cx);
    cx.simulate_input("~/src");
    settle(cx);
    assert!(
        browse_request(&mut helm, cx).is_none(),
        "the premise: typing asks nothing"
    );
    assert_eq!(
        name_of(cx, "launcher-search-option-1").as_deref(),
        Some("Browse this path: ~/src"),
        "the premise: the row after Use this path"
    );
    keys(cx, "down");
    keys(cx, "enter");
    assert_eq!(draft(cx).folder.value(), "~/src");
    let request = browse_request(&mut helm, cx).expect("the row browsed");
    assert_eq!(request.body.as_ref().unwrap()["cwd"], json!("~/src"));
}

/// Browse is not offered for a managed checkout, whose folder does not exist yet, or in Restart with, which holds its
/// destination. Why: the old launcher drew no Browse there.
#[gpui::test]
fn browse_is_not_offered_where_the_folder_cannot_change(cx: &mut TestAppContext) {
    let (_helm, cx) = new_in_tmp(cx);
    test_support::frame(cx);
    assert!(
        cx.debug_bounds("launcher-browse").is_some(),
        "the premise: offered"
    );
    cx.update(|_, cx| {
        let launcher =
            crate::layers::last_opened::<super::view::Launcher>(cx).expect("a launcher is open");
        launcher.update(cx, |launcher, cx| {
            launcher.draft.choose_destination(DestinationKind::Checkout);
            cx.notify();
        });
    });
    settle(cx);
    test_support::frame(cx);
    assert!(
        cx.debug_bounds("launcher-browse").is_none(),
        "not for a managed checkout"
    );

    let mut source = super::draft_tests::session(super::view_tests::codex(None, None, None));
    source.restart_offer = farhelm_proto::RestartOffer::Resume;
    let (_helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::restart_with("missing", &source, Some(1)).unwrap(),
        fixture_hosts(&[0]),
    );
    test_support::frame(cx);
    assert!(
        cx.debug_bounds("launcher-host-row").is_some(),
        "the premise: Restart with's launcher is drawn"
    );
    assert!(
        cx.debug_bounds("launcher-browse").is_none(),
        "nor in Restart with"
    );
}

/// Replace with holds its host but not its folder, so Browse sits beside the held host. Why: the old launcher showed it
/// there.
#[gpui::test]
fn browse_is_offered_beside_replace_withs_held_host(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(super::view_tests::codex(None, None, None));
    let (_helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::replace_with("a", &source, Some(1)),
        fixture_hosts(&[0]),
    );
    assert!(
        draft(cx).holds(super::draft::Field::Host),
        "the premise: Replace with holds its host"
    );
    test_support::frame(cx);
    assert!(cx.debug_bounds("launcher-browse").is_some());
}

/// Choosing another host forgets the listing: it named the folders of the host it was asked of. Why: taking one would
/// launch on the new host in a folder listed on the old one.
#[gpui::test]
fn a_host_change_forgets_the_listing(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "/tmp".into(), None),
        fixture_hosts(&[0, 4]),
    );
    test_support::click(cx, "launcher-browse");
    let request = browse_request(&mut helm, cx).expect("Browse asked the host");
    helm.reply(request.id, 200, Some(tmp_listing()));
    settle(cx);
    assert!(
        name_of(cx, "launcher-browse-use").is_some(),
        "the premise: listed"
    );
    let other = fixture_hosts(&[0, 4]).entries[1]
        .key
        .parse::<i64>()
        .expect("the premise: a second host");
    cx.update(|_, cx| {
        let launcher =
            crate::layers::last_opened::<super::view::Launcher>(cx).expect("a launcher is open");
        launcher.update(cx, |launcher, cx| {
            launcher.draft.choose_host(other, "/tmp".into());
            cx.notify();
        });
    });
    settle(cx);
    assert_eq!(draft(cx).host, Some(other), "the premise: the host changed");
    assert_eq!(name_of(cx, "launcher-browse-use"), None);
}

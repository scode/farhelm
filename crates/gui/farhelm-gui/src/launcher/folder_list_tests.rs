//! Tests of the folder field's dropdown and the launch-history read behind it: the rules for which recent folders show
//! and how a long one is labelled, without a window, then the real launcher over the helm client's fake: the read the
//! launcher sends, the rows it draws from the answer, the keys and clicks that take a row, and an answer that arrives
//! for a host the launcher has left.
//!
//! The old app's folder-picker tests at reference commit `24bc0580` (`folder_dropdown_*` in `list/create_form.rs`) are
//! the oracle for the pure rules; the view cases port the composer cases of `sidebar.spec.ts` that need no Browse.

use farhelm_helm_client::fake::FakeHelm;
use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext, VisualTestContext,
};
use serde_json::json;

use super::draft::LaunchDraft;
use super::view::{recent_folders_for_tests as recent_folders, tail_label_for_tests as tail_label};
use super::view_tests::{draft, fixture_hosts, launcher_with_hosts, name_of, read, shown_refusal};
use crate::layers;
use crate::test_support::{self, keys, settle};

// ---- The rules -----------------------------------------------------------------------------------------------------

/// A seeded folder (the open session's, `~`, a Clone's) shows every recent folder, while one the user typed in filters
/// them by what is typed, ignoring case; either way at most eight, newest first as the helm sent them. Why: a seed is
/// not a search, and filtering on it would hide the recents the user opened the list to see (old
/// `folder_dropdown_filters_only_typed_text_and_caps_matches`).
#[test]
fn recent_folders_filter_only_typed_text_and_keep_eight() {
    let folders: Vec<String> = (0..10).map(|n| format!("/src/app{n}")).collect();
    let all = recent_folders(folders.iter().map(String::as_str), "~", false);
    assert_eq!(all.len(), 8, "a seed filters nothing but the count");
    assert_eq!(all[0], "/src/app0", "newest first, as sent");
    let typed = recent_folders(folders.iter().map(String::as_str), "APP1", true);
    assert_eq!(typed, ["/src/app1"]);
    assert!(recent_folders(folders.iter().map(String::as_str), "nope", true).is_empty());
}

/// A label keeps a long path's end behind an ellipsis, at most 36 characters, and never keeps part of an escaped
/// character: an escape the cut would split goes with the cut-off start. Why: the end of a path is what tells two
/// recent folders apart, and half of a `<U+202E>` could pass for path text (old `folder_dropdown_labels_*`).
#[test]
fn labels_keep_the_end_without_splitting_an_escape() {
    assert_eq!(tail_label("~/src"), "~/src");
    let long = format!("/{}/end", "a".repeat(40));
    let label = tail_label(&long);
    assert_eq!(label.chars().count(), 36);
    assert!(label.starts_with('…') && label.ends_with("/end"), "{label}");
    // The escape `<U+202E>` sits where the cut falls: it is dropped whole, never shown in part.
    let path = format!("/{}\u{202e}{}", "b".repeat(30), "c".repeat(30));
    let label = tail_label(&path);
    assert!(!label.contains("U+202E") && !label.contains('>'), "{label}");
    assert!(label.ends_with(&"c".repeat(30)), "{label}");
    // A shorter path keeps its escape whole and escaped.
    assert_eq!(tail_label("/x\u{202e}y"), "/x<U+202E>y");
    // A literal `<U+` typed in a path is escaped too (its `<` as `<U+003C>`), so a cut there drops that escape whole.
    let literal = format!("/{}<U+{}", "d".repeat(28), "e".repeat(30));
    let label = tail_label(&literal);
    assert!(!label.contains("003C") && !label.contains('>'), "{label}");
    assert!(label.ends_with(&"e".repeat(30)), "{label}");
}

// ---- The launcher --------------------------------------------------------------------------------------------------

/// The launch-history reads the window sent since the last call, as (command id, path); every other command is answered
/// at once with an empty body.
fn history_reads(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<(u64, String)> {
    cx.run_until_parked();
    let mut reads = Vec::new();
    for command in helm.commands() {
        if command.path.starts_with("/api/launch-history") {
            reads.push((command.id, command.path));
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    reads
}

/// The helm's history answer with `folders`, newest first.
fn history(folders: &[&str]) -> serde_json::Value {
    json!({
        "launches": [],
        "folders": folders.iter().map(|folder| json!({
            "host": 1,
            "canonical_cwd": folder,
            "canonical_proven": true,
            "display_cwd": folder,
            "created_at": 1,
            "creation_seq": null,
        })).collect::<Vec<_>>(),
        "checkout_config_revision": 1,
    })
}

/// New on host1 (connected, local) at `~`, with host2 (connecting) beside it, the history read answered with `folders`.
fn launcher_with_history<'a>(
    cx: &'a mut TestAppContext,
    folders: &[&str],
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "~".into(), None),
        fixture_hosts(&[0, 1]),
    );
    let reads = history_reads(&mut helm, cx);
    let [(id, path)] = &reads[..] else {
        panic!("one history read, not {reads:?}");
    };
    assert!(path.ends_with("launch-history?host=1"), "{path}");
    helm.reply(*id, 200, Some(history(folders)));
    settle(cx);
    (helm, cx)
}

/// Whether the folder list was drawn in the last frame.
fn list_shown(cx: &mut VisualTestContext) -> bool {
    test_support::frame(cx);
    cx.debug_bounds("launcher-folder-list").is_some()
}

/// Focus the folder field, which opens its list.
fn focus_folder(cx: &mut VisualTestContext) {
    test_support::click(cx, "launcher-folder-row");
    assert!(
        read(cx, |launcher, window, cx| launcher
            .folder
            .focus_handle(cx)
            .is_focused(window)),
        "the premise: the click focused the folder field"
    );
}

/// The launcher reads the chosen host's history when it opens, and focusing the folder field lists its recent folders
/// under the Recent heading, then Home and Local home under Reset, each named for assistive technology. Why: SPEC.md,
/// "Working directory": the folder field's dropdown offers "up to eight Recent folders, newest first", "Reset with Home
/// and Local home".
#[gpui::test]
fn focusing_the_folder_lists_recent_folders_and_resets(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_with_history(cx, &["/src/new", "/src/old"]);
    focus_folder(cx);
    assert!(list_shown(cx), "focusing the field opens the list");
    assert_eq!(
        name_of(cx, "launcher-folder-0").as_deref(),
        Some("/src/new")
    );
    assert_eq!(
        name_of(cx, "launcher-folder-1").as_deref(),
        Some("/src/old")
    );
    assert_eq!(
        name_of(cx, "launcher-folder-2").as_deref(),
        Some("Reset folder to home")
    );
    assert_eq!(
        name_of(cx, "launcher-folder-3").as_deref(),
        Some("Reset destination to local home")
    );
}

/// Group headings do no picking. Clicking one leaves the folder field and its list alone, and a cut row still
/// offers the full path on hover.
#[gpui::test]
fn a_folder_heading_keeps_focus_and_a_cut_row_shows_its_full_path(cx: &mut TestAppContext) {
    let long = "/src/a-long-parent-directory-that-will-be-cut/saved-project";
    let (_helm, cx) = launcher_with_history(cx, &[long]);
    focus_folder(cx);
    assert!(list_shown(cx));
    let nodes = test_support::frame(cx);
    let heading = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("launcher-folder-recent"))
        .expect("the Recent heading is drawn");
    cx.simulate_click(heading.bounds.center(), gpui::Modifiers::none());
    settle(cx);
    assert!(list_shown(cx), "the heading keeps the list open");
    assert!(
        super::view_tests::focused(cx, super::view_tests::Field::Folder),
        "the keyboard stays in the field"
    );
    assert!(
        crate::session_list::tests::painted(&test_support::frame(cx))
            .iter()
            .any(|text| text.starts_with('…') && text.ends_with("/saved-project"))
    );
    let full = crate::session_list::host_rail::tests::hover_help(cx, "launcher-folder-0");
    assert_eq!(full.as_deref(), Some(long));
}

/// Launch is a separate pointer action even while the folder popup is open. Closing it at the press cannot
/// move the action away from the release, and its highlighted history does not replace the typed folder.
#[gpui::test]
fn a_press_on_launch_uses_the_typed_folder_with_its_list_open(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_with_history(cx, &["/src/recent"]);
    super::folder_default_tests::edit_draft(cx, |draft| {
        draft.choose_tab(super::draft::Tab::Command);
        draft.edit_command("true".into());
        draft.choose_command_yolo(false);
    });
    focus_folder(cx);
    keys(cx, super::view_tests::SELECT_ALL);
    cx.simulate_input("/src");
    settle(cx);
    keys(cx, "down");
    assert_eq!(
        super::view_tests::selected_state(cx, "launcher-folder-0"),
        Some(true)
    );
    assert_eq!(
        draft(cx).folder.value(),
        "/src",
        "the selected row is still only offered"
    );
    assert!(list_shown(cx), "the list is open at mouse-down");
    assert!(super::launch_tests::creates(&mut helm, cx).is_empty());
    let at = test_support::center(cx, "launcher-launch");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    settle(cx);
    assert!(!list_shown(cx), "mouse-down closed the list");
    assert!(test_support::bounds(cx, "launcher-launch").contains(&at));
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    let sent = super::launch_tests::creates(&mut helm, cx);
    let [create] = sent.as_slice() else {
        panic!("one launch at the same pointer position: {sent:?}");
    };
    assert_eq!(create.body.as_ref().unwrap()["cwd"], "/src");
}

/// An arrow highlights a row and Enter takes it without launching; with the list open and nothing highlighted, Enter
/// launches as from any field (here an incomplete draft says why) and closes the list, so the refused launch's controls
/// below the field are not left under it. Why: SPEC.md: "In the editable folder field, Enter chooses an
/// arrow-highlighted dropdown entry without launching; with no highlighted entry it launches as before", and "Enter that
/// submits a launch also closes the list, whether the launch is accepted or refused".
#[gpui::test]
fn enter_takes_a_highlighted_row_and_otherwise_launches(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_with_history(cx, &["/src/new", "/src/old"]);
    focus_folder(cx);
    keys(cx, "down down enter");
    let taken = draft(cx);
    assert_eq!(taken.folder.value(), "/src/old");
    assert!(
        taken.folder.is_touched(),
        "a taken row is the user's own choice"
    );
    assert_eq!(shown_refusal(cx), None, "taking a row launched nothing");
    assert!(!list_shown(cx), "and closed the list");
    keys(cx, "x");
    assert!(
        list_shown(cx),
        "the premise: typing opened the list, with nothing highlighted"
    );
    keys(cx, "enter");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. Choose a structured harness before launching"),
        "with nothing highlighted, Enter presses Launch"
    );
    assert!(!list_shown(cx), "and closes the list, refused or not");
}

/// Clone filters using the underlying folder after an edit. The visible spelling can also be a different path on disk.
#[gpui::test]
fn a_clone_folder_edit_filters_by_the_underlying_path(cx: &mut TestAppContext) {
    let raw = "/src/work\u{200b}box";
    let literal = "/src/work<U+200B>box";
    let mut source = super::draft_tests::session(super::view_tests::codex(None, None, None));
    source.cwd = raw.into();
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::clone_of(&source, Some(1)),
        fixture_hosts(&[0]),
    );
    let reads = history_reads(&mut helm, cx);
    let [(id, path)] = &reads[..] else {
        panic!("one history read, not {reads:?}");
    };
    assert!(path.ends_with("launch-history?host=1"), "{path}");
    helm.reply(*id, 200, Some(history(&[literal, raw])));
    settle(cx);
    focus_folder(cx);
    assert_eq!(name_of(cx, "launcher-folder-0"), Some(tail_label(literal)));
    assert_eq!(name_of(cx, "launcher-folder-1"), Some(tail_label(raw)));
    keys(cx, "end x backspace");
    assert!(draft(cx).folder.is_touched());
    assert_eq!(name_of(cx, "launcher-folder-0"), Some(tail_label(raw)));
    assert_eq!(
        name_of(cx, "launcher-folder-1").as_deref(),
        Some("Reset folder to home")
    );
}

/// Typing filters the recents by what is typed (ignoring case), drops the highlight so Enter launches rather than
/// taking a row, and reopens a list Escape closed; a click takes a row. Why: once the user types, the list is a search
/// of their history (old `folder_suggestions`), and SPEC.md says "typing reopens it".
#[gpui::test]
fn typing_filters_reopens_and_drops_the_highlight(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_with_history(cx, &["/src/new", "/src/old"]);
    focus_folder(cx);
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.folder.text(cx).to_string()),
        "~",
        "the premise: the field holds the one-character seed, so one Backspace empties it"
    );
    keys(cx, "escape");
    assert!(!list_shown(cx), "the premise: Escape closed the list");
    keys(cx, "backspace");
    assert!(list_shown(cx), "typing reopens it");
    keys(cx, "down");
    cx.simulate_input("OLD");
    settle(cx);
    assert_eq!(
        name_of(cx, "launcher-folder-0").as_deref(),
        Some("/src/old")
    );
    assert_eq!(
        name_of(cx, "launcher-folder-1").as_deref(),
        Some("Reset folder to home"),
        "only the matching recent folder is left"
    );
    keys(cx, "enter");
    assert!(
        shown_refusal(cx).is_some(),
        "typing dropped the highlight, so Enter pressed Launch instead of taking a row"
    );
    assert_eq!(draft(cx).folder.value(), "OLD");
    // Enter closed the list; leaving the field and coming back opens it again, still filtered by what was typed.
    test_support::click(cx, "launcher-search-row");
    focus_folder(cx);
    test_support::click(cx, "launcher-folder-0");
    assert_eq!(draft(cx).folder.value(), "/src/old");
}

/// A picked folder is not a search: after taking Home, the next list shows every recent folder again. Why: SPEC.md:
/// "Untouched prefilled fields show the newest eight; typing filters", and the old case "composer folder dropdown
/// preserves keyboard launch and grouped history" expected all eight after Reset folder to home.
#[gpui::test]
fn a_picked_folder_does_not_filter_the_next_list(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_with_history(cx, &["/src/new", "/src/old"]);
    focus_folder(cx);
    keys(cx, "down down down enter");
    let taken = draft(cx);
    assert!(
        taken.folder.value() == "~" && taken.folder.is_touched(),
        "the premise: Home was taken (the seed was untouched)"
    );
    assert_eq!(shown_refusal(cx), None, "and nothing launched");
    test_support::click(cx, "launcher-search-row");
    focus_folder(cx);
    assert_eq!(
        name_of(cx, "launcher-folder-0").as_deref(),
        Some("/src/new")
    );
    assert_eq!(
        name_of(cx, "launcher-folder-1").as_deref(),
        Some("/src/old")
    );
}

/// Local home makes the local host the target with `~` as its folder, as the old Reset row did; Home keeps the host.
/// Why: Local home is a deliberate choice of this machine, not just a folder. A Clone's inherited host starts
/// unchecked for Save as template. Local home counts as a hand pick so the user can carry the local host in a template.
#[gpui::test]
fn local_home_moves_and_marks_the_host_chosen(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::clone_of(
            &super::draft_tests::session(super::view_tests::codex(None, None, None)),
            Some(2),
        ),
        fixture_hosts(&[0, 1]),
    );
    assert!(
        history_reads(&mut helm, cx).is_empty(),
        "the premise: host2 is not connected, so its history is not read"
    );
    assert_eq!(draft(cx).host, Some(2));
    assert!(
        !draft(cx).host_chosen,
        "the premise: the remote host was inherited"
    );
    focus_folder(cx);
    assert!(list_shown(cx));
    assert_eq!(
        name_of(cx, "launcher-folder-empty").as_deref(),
        Some("No recent folders")
    );
    // Rows: Home (0), Local home (1).
    keys(cx, "down down enter");
    let moved = draft(cx);
    assert_eq!(moved.host, Some(1));
    assert_eq!(moved.folder.value(), "~");
    assert!(moved.host_chosen, "Local home counts as choosing the host");
}

/// Local home explicitly picks the host even when the local host was already shown. Save as template is where
/// that otherwise invisible change matters: its Host checkbox becomes checked.
#[gpui::test]
fn local_home_checks_the_template_host_even_when_already_local(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_with_history(cx, &[]);
    test_support::click(cx, "launcher-harness-Codex");
    assert_eq!(draft(cx).host, Some(1));
    let host_checkbox = |cx: &mut VisualTestContext| {
        test_support::frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some("save-template-field-host"))
            .and_then(|node| node.toggled)
    };

    test_support::click(cx, "launcher-save-template");
    assert_eq!(host_checkbox(cx), Some(gpui::Toggled::False));
    keys(cx, "escape");
    assert!(host_checkbox(cx).is_none(), "Escape closed the panel");

    focus_folder(cx);
    assert_eq!(
        name_of(cx, "launcher-folder-1").as_deref(),
        Some("Reset destination to local home")
    );
    keys(cx, "down down enter");
    assert_eq!(draft(cx).host, Some(1), "the visible host is unchanged");
    test_support::click(cx, "launcher-save-template");
    assert_eq!(host_checkbox(cx), Some(gpui::Toggled::True));
}

/// host1 and host2 from the helm's fixture, both connected (the fixture's host2 is connecting).
fn two_connected_hosts() -> farhelm_proto::ui_channel::CollectionState {
    let mut hosts = fixture_hosts(&[0, 1]);
    let mut connected = hosts.entries[0].value["state"].clone();
    // host2 is its own installation, not a second claim on host1's.
    connected["identity"] = serde_json::json!("identity2");
    hosts.entries[1].value["state"] = connected;
    hosts
}

/// Each host gets its own read at once, and an answer that arrives after the launcher moved to another host is not
/// shown for it: the list only ever shows the chosen host's folders, then that host's own answer. Why: SPEC.md, "Recent
/// setups": history is per host installation, and a late answer must not show another host's folders.
#[gpui::test]
fn a_late_answer_for_another_host_is_dropped(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "~".into(), None),
        two_connected_hosts(),
    );
    let reads = history_reads(&mut helm, cx);
    let [(first, path)] = &reads[..] else {
        panic!("one history read, not {reads:?}");
    };
    assert!(path.ends_with("host=1"), "{path}");
    assert!(
        read(cx, |launcher, window, cx| launcher
            .search
            .focus_handle(cx)
            .is_focused(window)),
        "the premise: focus starts in the search box"
    );
    cx.simulate_input("host:host2");
    settle(cx);
    keys(cx, "enter");
    assert_eq!(draft(cx).host, Some(2), "the premise: host2 is chosen");
    let reads = history_reads(&mut helm, cx);
    let [(second, path)] = &reads[..] else {
        panic!("one read for host2, not {reads:?}");
    };
    assert!(path.ends_with("host=2"), "{path}");
    // The fake answers in order, so host1's read is answered first; it must not show for host2.
    helm.reply(*first, 200, Some(history(&["/src/host1-only"])));
    settle(cx);
    focus_folder(cx);
    assert_eq!(
        name_of(cx, "launcher-folder-empty").as_deref(),
        Some("No recent folders"),
        "host1's late answer is not shown for host2"
    );
    helm.reply(*second, 200, Some(history(&["/src/host2"])));
    settle(cx);
    assert_eq!(
        name_of(cx, "launcher-folder-0").as_deref(),
        Some("/src/host2"),
        "host2's own answer shows"
    );
}

/// Replace with keeps its host, so the list offers no Local home, which would move it. Why: SPEC.md, "Replace with"
/// keeps the source's host.
#[gpui::test]
fn replace_with_offers_no_local_home(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(super::view_tests::codex(None, None, None));
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::replace_with("a", &source, Some(1)),
        fixture_hosts(&[0, 1]),
    );
    history_reads(&mut helm, cx);
    focus_folder(cx);
    let names: Vec<String> = (0..4)
        .filter_map(|index| name_of(cx, &format!("launcher-folder-{index}")))
        .collect();
    assert!(
        names.contains(&"Reset folder to home".to_owned()),
        "{names:?}"
    );
    assert!(
        !names.contains(&"Reset destination to local home".to_owned()),
        "{names:?}"
    );
}

/// Escape closes the list before the launcher, so a second Escape closes the launcher; Tab closes the list and moves
/// focus on. Why: the list is the field's, and the keys that leave it must not also act on the dialog (the old case
/// "composer folder dropdown preserves keyboard launch and grouped history").
#[gpui::test]
fn escape_and_tab_close_the_list_first(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_with_history(cx, &["/src/new"]);
    focus_folder(cx);
    keys(cx, "tab");
    assert!(!list_shown(cx), "Tab closes the list");
    assert!(
        !read(cx, |launcher, window, cx| launcher
            .folder
            .focus_handle(cx)
            .is_focused(window)),
        "and moves focus on"
    );
    focus_folder(cx);
    assert!(list_shown(cx), "the premise: the list is open again");
    keys(cx, "escape");
    assert!(!list_shown(cx), "Escape closes the list");
    assert!(
        cx.update(|_, cx| layers::last_opened::<super::view::Launcher>(cx).is_some()),
        "and leaves the launcher open"
    );
    keys(cx, "escape");
    assert!(
        cx.update(|_, cx| layers::last_opened::<super::view::Launcher>(cx).is_none()),
        "a second Escape closes the launcher"
    );
}

/// A folder taken from the composer search's "Use this path" is a pick too: after typing in the folder and then taking
/// a path from search, the next list shows every recent folder again. Why: the old launcher cleared its typed flag on
/// every folder write that was not typing, so only typing ever filters.
#[gpui::test]
fn a_path_from_search_does_not_filter_the_next_list(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_with_history(cx, &["/src/new", "/src/old"]);
    focus_folder(cx);
    keys(cx, "backspace");
    cx.simulate_input("new");
    settle(cx);
    assert_eq!(
        name_of(cx, "launcher-folder-1").as_deref(),
        Some("Reset folder to home"),
        "the premise: typing filtered the list to one recent folder"
    );
    test_support::click(cx, "launcher-search-row");
    cx.simulate_input("folder:/srv/app");
    settle(cx);
    keys(cx, "enter");
    assert_eq!(
        draft(cx).folder.value(),
        "/srv/app",
        "the premise: search took the path"
    );
    focus_folder(cx);
    assert_eq!(
        name_of(cx, "launcher-folder-0").as_deref(),
        Some("/src/new")
    );
    assert_eq!(
        name_of(cx, "launcher-folder-1").as_deref(),
        Some("/src/old")
    );
}

/// A history answer that arrives while a row is highlighted drops the highlight, so Enter launches instead of taking
/// whatever row the new recents pushed under it. Why: the rows are recomputed from the history, and an index into the
/// old rows would point at a row the arrow never reached.
#[gpui::test]
fn a_history_answer_drops_the_highlight(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "/srv".into(), None),
        fixture_hosts(&[0, 1]),
    );
    let reads = history_reads(&mut helm, cx);
    let [(id, _)] = &reads[..] else {
        panic!("one history read, not {reads:?}");
    };
    focus_folder(cx);
    keys(cx, "down");
    assert_eq!(
        name_of(cx, "launcher-folder-0").as_deref(),
        Some("Reset folder to home"),
        "the premise: Home is the highlighted first row while no history is in"
    );
    helm.reply(*id, 200, Some(history(&["/src/new"])));
    settle(cx);
    keys(cx, "enter");
    assert!(
        shown_refusal(cx).is_some(),
        "Enter pressed Launch rather than taking the row now at the old index"
    );
    assert_eq!(draft(cx).folder.value(), "/srv", "and took no folder");
}

//! Tests of the composer search box in the real launcher over the helm client's fake: what typing shows, what Enter,
//! the arrows, Escape and a click on a row do, and which rows a mode refuses. Which rows a query offers, and in what
//! order, is `search_tests`' (the search's own rules, without a window); these tests show that the box draws those
//! rows and that taking one makes the same edit as the hand control it stands for.
//!
//! Every step ends in one of the shared harness's step helpers, so the focus and layer invariants are checked after
//! each.

use farhelm_proto::{LaunchHarness, LaunchPermission, RestartOffer};
use gpui::{TestAppContext, VisualTestContext};

use super::draft::{LaunchDraft, Tab};
use super::recents_view_tests::{
    answer_history, chosen, creates, history, launched, new_in_tmp, two_setups,
};
use super::view_tests::{
    answer_catalog, codex, draft, fixture_hosts, launcher_on, launcher_with_hosts, name_of, read,
    search_focused, search_text, selected_state, shown_refusal,
};
use crate::test_support::{self, keys, settle};

/// Whether the search results were drawn in the last frame.
fn results_shown(cx: &mut VisualTestContext) -> bool {
    test_support::frame(cx);
    cx.debug_bounds("launcher-search-results").is_some()
}

/// Type `text` into the focused field, as one input, and let it settle.
fn type_in(cx: &mut VisualTestContext, text: &str) {
    cx.simulate_input(text);
    settle(cx);
}

/// New on host1 at `~`, with focus where the launcher starts it.
fn new_launcher(
    cx: &mut TestAppContext,
) -> (farhelm_helm_client::fake::FakeHelm, &mut VisualTestContext) {
    launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None))
}

/// SPEC.md, "Creation": New "starts focus on search", and a word and Enter make one choice: typing `codex` shows the
/// results with the agent type, and Enter chooses it, empties the box, closes the results and leaves focus in the box
/// for the next word. Why: this is the keyboard path a whole setup is built by, word, Enter, word, Enter.
#[gpui::test]
fn a_word_and_enter_choose_it_and_empty_the_box(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    assert!(search_focused(cx), "focus starts in the search box");
    type_in(cx, "codex");
    assert!(results_shown(cx), "typing shows the results");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Harness: Codex")
    );
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(search_text(cx), "", "the box is emptied");
    assert!(!results_shown(cx), "and the results close");
    assert!(search_focused(cx), "focus stays in the box");
}

/// Text that matches nothing offers "Set session name", and Enter names the session, the name field showing it. Why:
/// typing a name and pressing Enter is how the old launcher named a session from search (SPEC.md: "the visible Set
/// session name row for unmatched text").
#[gpui::test]
fn unmatched_text_and_enter_name_the_session(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    type_in(cx, "fix parser");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Set session name: fix parser")
    );
    keys(cx, "enter");
    assert_eq!(draft(cx).name.value(), "fix parser");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.name.text(cx).to_string()),
        "fix parser",
        "the name field follows the draft"
    );
    // The box is empty again, so a second Enter presses the main action (the old "Clone search Enter picks an
    // unmatched name before a second Enter launches"); this New has no agent type, so it says why instead.
    keys(cx, "enter");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. Choose a structured harness before launching")
    );
}

/// Before an agent type is chosen, `yolo` (bare, or after `perms:`) chooses YOLO and leaves the agent unchosen. Why:
/// SPEC.md, "Creation": "On the Agent tab, Guarded and YOLO can also be chosen before a harness, by their bare words
/// or `perms:` choices" (the old `${query} selects YOLO before choosing a harness`).
#[gpui::test]
fn yolo_from_search_before_an_agent_type(cx: &mut TestAppContext) {
    for query in ["yolo", "perms:yolo"] {
        let (_helm, cx) = new_launcher(cx);
        type_in(cx, query);
        keys(cx, "enter");
        let draft = draft(cx);
        assert_eq!(
            draft.agent.permissions,
            Some(LaunchPermission::Yolo),
            "{query}"
        );
        assert_eq!(draft.agent.harness, None, "{query}");
    }
}

/// Enter on an empty box presses the main action, so an incomplete draft says why beside it (SPEC.md: "Enter on an
/// empty box launches only a complete selection"); a box holding only spaces is a query, and says it has nothing to
/// offer instead of pressing. Why: Enter in the box must never launch by accident while a query is in it.
#[gpui::test]
fn enter_on_an_empty_box_presses_launch_and_spaces_do_not(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    type_in(cx, "  ");
    keys(cx, "enter");
    assert_eq!(shown_refusal(cx), None, "spaces are a query, not a press");
    assert_eq!(
        name_of(cx, "launcher-search-message").as_deref(),
        Some("Type a search or clear the search box to launch")
    );
    keys(cx, "backspace backspace enter");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. Choose a structured harness before launching")
    );
}

/// The arrows move the highlighted row, wrapping, and Enter takes the highlighted one; a click takes the row clicked.
/// `c` names no row exactly, so the first row is highlighted at first, and Up from it wraps to the last. Why: Enter
/// must take the row the user moved to, which is the one drawn highlighted.
#[gpui::test]
fn arrows_move_the_highlight_and_enter_or_a_click_takes_a_row(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    type_in(cx, "c");
    assert_eq!(
        name_of(cx, "launcher-search-option-1").as_deref(),
        Some("Harness: Claude"),
        "the premise: Claude is the second row"
    );
    keys(cx, "down enter");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Claude));
    type_in(cx, "c");
    let rows = read(cx, |launcher, _, cx| launcher.search_results(cx).rows);
    assert_eq!(
        rows.last(),
        Some(&super::search::Row::Harness(LaunchHarness::OpenCode)),
        "the premise: OpenCode is the last row"
    );
    keys(cx, "up enter");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::OpenCode));
    type_in(cx, "cod");
    test_support::click(cx, "launcher-search-option-0");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Codex));
    assert!(search_focused(cx), "a click leaves focus in the box");
}

/// Escape closes open results and keeps what was typed; with them closed it is the dialog's, and closes the launcher.
/// Enter on closed results opens them again rather than taking a row the user cannot see. Why: SPEC.md, "Escape or a
/// click outside closes the rows without clearing the query".
#[gpui::test]
fn escape_closes_the_results_and_keeps_the_query(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    type_in(cx, "cod");
    keys(cx, "escape");
    assert!(!results_shown(cx), "Escape closes the results");
    assert_eq!(search_text(cx), "cod", "and keeps the query");
    assert_eq!(draft(cx).agent.harness, None);
    keys(cx, "enter");
    assert!(results_shown(cx), "Enter opens them again");
    assert_eq!(draft(cx).agent.harness, None, "without taking a row");
    keys(cx, "escape escape");
    assert!(
        cx.update(|_, cx| cx
            .try_global::<super::view_tests::Opened>()
            .is_none_or(|opened| opened.0.upgrade().is_none())),
        "Escape with the results closed closes the launcher"
    );
}

/// A labelled query with no match says why in the results, in the warning ink, and Enter changes nothing. Why: a
/// scoped query that matched nothing must not quietly become a name.
#[gpui::test]
fn a_scoped_miss_says_why_and_enter_changes_nothing(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    type_in(cx, "host:nowhere");
    assert_eq!(
        name_of(cx, "launcher-search-message").as_deref(),
        Some("No host matches")
    );
    let before = draft(cx);
    keys(cx, "enter");
    assert_eq!(draft(cx), before, "nothing changed");
    assert_eq!(search_text(cx), "host:nowhere", "and the query stays");
}

/// `host:` rows choose the host as the host picker does, and `folder:` a path as typing it would. Why: these are the
/// destination's hand edits, reached from the keyboard.
#[gpui::test]
fn host_and_folder_rows_set_the_destination(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "~".into(), None),
        fixture_hosts(&[0, 1]),
    );
    type_in(cx, "host:host2");
    keys(cx, "enter");
    assert_eq!(draft(cx).host, Some(2));
    type_in(cx, "folder:/src/app");
    keys(cx, "enter");
    assert_eq!(draft(cx).folder.value(), "/src/app");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.folder.text(cx).to_string()),
        "/src/app",
        "the folder field follows the draft"
    );
}

/// An agent type or model taken on the Command tab switches to the Agent tab, where it belongs, even a model of the
/// agent type already chosen (which changes no agent). Why: SPEC.md, "Command launch": search on that tab "still
/// offers agent types, models and recent setups, which switch to the Agent tab".
#[gpui::test]
fn a_model_taken_on_the_command_tab_switches_to_the_agent_tab(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher(cx);
    answer_catalog(
        &mut helm,
        cx,
        Some(serde_json::json!([{ "id": "gpt-5", "harness": "codex", "efforts": [] }])),
    );
    test_support::click(cx, "launcher-tabs-Command");
    assert_eq!(
        draft(cx).tab,
        Tab::Command,
        "the premise: the click chose Command"
    );
    test_support::click(cx, "launcher-search-row");
    assert!(search_focused(cx), "the premise: the click focused the box");
    type_in(cx, "codex");
    keys(cx, "enter");
    let chosen = draft(cx);
    assert_eq!(chosen.agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(
        chosen.tab,
        Tab::Agent,
        "an agent type taken on Command switches to Agent"
    );
    // Back on Command with Codex chosen, a Codex model changes no agent: only the row's own switch moves the tab.
    test_support::click(cx, "launcher-tabs-Command");
    assert_eq!(draft(cx).tab, Tab::Command, "the premise: back on Command");
    test_support::click(cx, "launcher-search-row");
    type_in(cx, "gpt-5");
    keys(cx, "enter");
    let draft = draft(cx);
    assert_eq!(draft.tab, Tab::Agent);
    assert_eq!(draft.agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(draft.agent.model.as_deref(), Some("gpt-5"));
}

/// Enter on an empty box launches a complete draft, once; a query that offers nothing sends nothing; and a held Enter
/// right after taking a row, which empties the box, sends nothing either (SPEC.md: "A held Enter never launches").
/// Why: the auto-repeat of the Enter that took a row would otherwise land on the emptied box and launch the setup
/// the user was still building.
#[gpui::test]
fn only_a_fresh_enter_on_an_empty_box_launches(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, super::launch_tests::command_draft());
    super::launch_tests::creates(&mut helm, cx);
    assert!(search_focused(cx), "the premise: focus starts in the box");
    type_in(cx, "host:nowhere");
    assert_eq!(
        name_of(cx, "launcher-search-message").as_deref(),
        Some("No host matches"),
        "the premise: the query offers nothing"
    );
    keys(cx, "enter");
    assert!(
        super::launch_tests::creates(&mut helm, cx).is_empty(),
        "a query that offers nothing sends nothing"
    );
    for _ in 0.."host:nowhere".len() {
        keys(cx, "backspace");
    }
    type_in(cx, "fix parser");
    keys(cx, "enter");
    assert_eq!(
        draft(cx).name.value(),
        "fix parser",
        "the premise: Enter took the row"
    );
    assert!(
        search_focused(cx) && search_text(cx).is_empty(),
        "the premise: the box has focus and is empty"
    );
    cx.simulate_event(gpui::KeyDownEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    settle(cx);
    assert!(
        super::launch_tests::creates(&mut helm, cx).is_empty(),
        "a held Enter sends nothing"
    );
    keys(cx, "enter");
    assert_eq!(
        super::launch_tests::creates(&mut helm, cx).len(),
        1,
        "a fresh Enter on the empty box launches"
    );
}

/// Custom taken from search moves focus on to its flags, as choosing it by hand does; every other row leaves focus in
/// the box. Why: Custom is useless without its flags, and the old launcher moved focus there the same way.
#[gpui::test]
fn custom_from_search_moves_focus_to_its_flags(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    type_in(cx, "codex");
    keys(cx, "enter");
    type_in(cx, "perms:custom");
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.permissions, Some(LaunchPermission::Custom));
    settle(cx);
    assert!(
        read(cx, |launcher, window, cx| launcher
            .flags
            .focus_handle(cx)
            .is_focused(window)),
        "focus moved to the flags"
    );
}

/// Restart with refuses a row that would change what it holds, even one naming the current value, and says why above
/// the box in the old words; the box keeps its text and the draft is unchanged. Rows it does not hold still apply.
/// Why: SPEC.md, "Restart with": "Search results and recent setups that set a held field are still refused even when
/// they name its current value".
#[gpui::test]
fn restart_with_refuses_a_held_field_and_says_why(cx: &mut TestAppContext) {
    let mut source = super::draft_tests::session(codex(None, None, None));
    source.restart_offer = RestartOffer::Resume;
    let restart = LaunchDraft::restart_with("missing", &source, Some(1)).unwrap();
    let (_helm, cx) = launcher_on(cx, restart);
    test_support::click(cx, "launcher-search-row");
    assert!(search_focused(cx), "the premise: the click focused the box");
    let before = draft(cx);
    type_in(cx, "codex");
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("agent type cannot change here: this dialog keeps agent type fixed for restart")
    );
    assert_eq!(draft(cx), before, "nothing changed");
    assert_eq!(search_text(cx), "codex", "the box keeps its text");
    assert!(
        !results_shown(cx),
        "the results close, so they cannot cover the reason"
    );
    keys(cx, "backspace backspace backspace backspace backspace");
    type_in(cx, "folder:/elsewhere");
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("destination cannot change here: this dialog keeps destination fixed for restart")
    );
    for _ in 0.."folder:/elsewhere".len() {
        keys(cx, "backspace");
    }
    type_in(cx, "name:x");
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("session name cannot change here: this dialog keeps session name fixed for restart")
    );
    for _ in 0.."name:x".len() {
        keys(cx, "backspace");
    }
    assert_eq!(
        name_of(cx, "launcher-search-refusal"),
        None,
        "typing takes the refusal away"
    );
    type_in(cx, "yolo");
    keys(cx, "enter");
    assert_eq!(
        draft(cx).agent.permissions,
        Some(LaunchPermission::Yolo),
        "a permission is not held"
    );
}

/// Clone and Replace with start focus in the search box too; only Restart with starts on Cancel. Why: SPEC.md,
/// "Ordinary launches still start focus on search", and the old Clone and Replace with tests typed into search first.
#[gpui::test]
fn clone_and_replace_with_start_focus_in_the_search_box(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(codex(None, None, None));
    for opened in [
        LaunchDraft::clone_of(&source, Some(1)),
        LaunchDraft::replace_with("a", &source, Some(1)),
    ] {
        let (_helm, cx) = launcher_on(cx, opened);
        assert!(search_focused(cx), "focus starts in the search box");
    }
}

/// Replace with keeps its host fixed, so a host row is refused above the box with Replace with's word, and the host
/// stays (a recorded deviation: the old search did not refuse it). Why: Replace with keeps its source's host (SPEC.md:
/// "Replace with can change the harness or folder", not the host), so a search must not move it.
#[gpui::test]
fn replace_with_refuses_a_host_row(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(codex(None, None, None));
    let (_helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::replace_with("a", &source, Some(1)),
        fixture_hosts(&[0, 1]),
    );
    type_in(cx, "host:host2");
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("host cannot change here: this dialog keeps host fixed for replace")
    );
    assert_eq!(draft(cx).host, Some(1), "the host stays");
}

// ---- Templates -------------------------------------------------------------------------------------------------------

/// The templates collection as the helm sends it: `partial` sets only a folder, `go` is a command launch of `true`,
/// and `nowhere` names a host no install has; with `unavailable`, marked unavailable with the helm's words for why.
fn templates_state(unavailable: Option<&str>) -> farhelm_proto::ui_channel::CollectionState {
    use farhelm_proto::launcher::{
        LaunchTemplate, LauncherKind, TemplateDestination, TemplateFields,
    };
    use farhelm_proto::ui_channel::{Availability, Collection, CollectionState, KeyedEntry};
    let all = [
        LaunchTemplate {
            name: "partial".into(),
            fields: TemplateFields {
                destination: Some(TemplateDestination::Folder("/srv/partial".into())),
                ..TemplateFields::default()
            },
        },
        LaunchTemplate {
            name: "go".into(),
            fields: TemplateFields {
                kind: Some(LauncherKind::Command),
                command: Some("true".into()),
                yolo: Some(false),
                ..TemplateFields::default()
            },
        },
        LaunchTemplate {
            name: "nowhere".into(),
            fields: TemplateFields {
                host: Some("no-such-install".into()),
                ..TemplateFields::default()
            },
        },
        LaunchTemplate {
            name: "reset-codex".into(),
            fields: TemplateFields {
                agent: Some(LaunchHarness::Codex),
                permissions: Some(None),
                ..TemplateFields::default()
            },
        },
        LaunchTemplate {
            name: "reset-goose".into(),
            fields: TemplateFields {
                agent: Some(LaunchHarness::Goose),
                permissions: Some(None),
                ..TemplateFields::default()
            },
        },
        // An agent and nothing about permissions: applied over an explicit Guarded, it escalates.
        LaunchTemplate {
            name: "to-opencode".into(),
            fields: TemplateFields {
                agent: Some(LaunchHarness::OpenCode),
                ..TemplateFields::default()
            },
        },
    ];
    let mut meta = test_support::meta(&[
        "partial",
        "go",
        "nowhere",
        "reset-codex",
        "reset-goose",
        "to-opencode",
    ]);
    if let Some(error) = unavailable {
        meta.availability = Availability::Unavailable {
            error: error.into(),
        };
    }
    CollectionState {
        collection: Collection::Templates,
        meta,
        entries: all
            .iter()
            .map(|template| KeyedEntry {
                key: template.name.clone(),
                value: serde_json::to_value(template).unwrap(),
            })
            .collect(),
    }
}

/// New on host1 at `~` in the real main view over the fake helm, whose first snapshot carries [`templates_state`].
fn new_launcher_with_templates<'a>(
    cx: &'a mut TestAppContext,
    unavailable: Option<&str>,
) -> (
    farhelm_helm_client::fake::FakeHelm,
    &'a mut VisualTestContext,
) {
    launcher_with_templates_on(cx, unavailable, LaunchDraft::new(Some(1), "~".into(), None))
}

/// [`new_launcher_with_templates`] with the launcher opened on `draft` instead of New's bare draft.
fn launcher_with_templates_on<'a>(
    cx: &'a mut TestAppContext,
    unavailable: Option<&str>,
    draft: LaunchDraft,
) -> (
    farhelm_helm_client::fake::FakeHelm,
    &'a mut VisualTestContext,
) {
    let (mut helm, _connection) = crate::test_support::started(cx);
    helm.connect(vec![
        crate::session_list::tests::listing(&["a"], &[crate::session_list::tests::session("a")]),
        fixture_hosts(&[0]),
        crate::session_list::tests::preferences(&[], None),
        templates_state(unavailable),
    ]);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, crate::main_view::MainView::new);
    window.update(|window, cx| {
        let mark = crate::focus::Mark::take(cx);
        assert!(
            super::view::open(draft, &mark, window, cx).is_some(),
            "the launcher opened"
        );
    });
    settle(window);
    (helm, window)
}

/// SPEC.md, "Creation": "`tl:name` in the search box offers templates by name, and accepting one applies it", and
/// applying keeps the launch kind unless the template sets one. Taking the row is the keyboard path to a template's
/// edits, so it must make them exactly as applying elsewhere does. Specified: with New open and its catalog read,
/// `tl:partial` offers "Template: partial", and Enter puts its folder in the draft, keeps the Agent tab and empties the
/// box; `tl:go` and Enter then switch to the Command tab with the command `true`, the template's own launch kind.
#[gpui::test]
fn a_tl_row_applies_its_template_and_keeps_the_launch_kind(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    type_in(cx, "tl:partial");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Template: partial")
    );
    keys(cx, "enter");
    let applied = draft(cx);
    assert_eq!(applied.folder.value(), "/srv/partial", "the folder applied");
    assert_eq!(applied.tab, Tab::Agent, "the launch kind kept");
    assert_eq!(search_text(cx), "", "the box is emptied");

    type_in(cx, "tl:go");
    keys(cx, "enter");
    let applied = draft(cx);
    assert_eq!(applied.tab, Tab::Command, "the template's launch kind");
    assert_eq!(applied.command.command, "true");
}

/// A template whose agent cannot run an explicitly chosen permission raises the escalation banner, as the hand switch
/// would, judged once for the whole template. Why: SPEC.md applies a template's edits "in the same way and with the same
/// effects as making them by hand" (old "template permissions are compared after the complete template transaction").
#[gpui::test]
fn a_tl_row_that_escalates_raises_the_banner(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    type_in(cx, "perms:guarded");
    keys(cx, "enter");
    assert_eq!(super::escalation_tests::banner(cx), None, "the premise");
    type_in(cx, "tl:to-opencode");
    keys(cx, "enter");
    assert_eq!(
        draft(cx).agent.harness,
        Some(LaunchHarness::OpenCode),
        "the premise"
    );
    assert_eq!(
        super::escalation_tests::banner(cx).as_deref(),
        Some("Permission changed: Guarded → YOLO, because OpenCode does not support Guarded.")
    );
}

/// SPEC.md: a template that does not apply "is refused with a message naming the field, and nothing from it is
/// applied", and the old launcher said so above its search box. Applying part of it would leave a draft the user did
/// not ask for. Specified: `tl:nowhere` and Enter leave the folder `~` and show the refusal, naming `nowhere`, in the
/// launcher's note, drawn in the search row above the box.
#[gpui::test]
fn a_refused_tl_row_changes_nothing_and_says_why_above_the_box(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    type_in(cx, "tl:nowhere");
    keys(cx, "enter");
    assert_eq!(draft(cx).folder.value(), "~", "nothing applied");
    let shown = name_of(cx, "launcher-note").expect("the refusal is shown");
    assert!(shown.contains("nowhere"), "{shown:?}");
    let (note, field) = (
        test_support::bounds(cx, "launcher-note"),
        test_support::bounds(cx, "launcher-search-box"),
    );
    assert!(
        note.bottom() <= field.top(),
        "the refusal is drawn above the box: {note:?} over {field:?}"
    );
}

/// A taken `tl:` row waits for New's catalog read, and the store can change meanwhile: the batch applies the template
/// the store holds when it applies, and says so in the old launcher's words when it cannot (the old `tl:` acceptance's
/// "No template is named …; it may have been deleted" and "Couldn't load templates: …; search tl: and retry").
/// Applying a template the user saw deleted, or saying nothing, would hide what happened. Specified: `tl:partial` and
/// Enter before the catalog answers, then `partial` deleted, then the answer: the folder stays `~` and the note reads
/// `No template is named "partial"; it may have been deleted`; in a second launcher, the same press with the templates
/// made unavailable ("busy") meanwhile says "Couldn't load templates: busy; search tl: and retry".
#[gpui::test]
fn a_tl_row_waiting_for_the_catalog_refuses_when_its_template_goes(cx: &mut TestAppContext) {
    use farhelm_proto::ui_channel::{Collection, CollectionChanges};
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    type_in(cx, "tl:partial");
    keys(cx, "enter");
    helm.push(CollectionChanges {
        collection: Collection::Templates,
        meta: test_support::meta(&["go", "nowhere"]),
        upsert: Vec::new(),
        remove: vec!["partial".into()],
    });
    settle(cx);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    assert_eq!(draft(cx).folder.value(), "~", "nothing applied");
    assert_eq!(
        name_of(cx, "launcher-note").as_deref(),
        Some("No template is named \"partial\"; it may have been deleted")
    );
}

/// The unavailable half of [`a_tl_row_waiting_for_the_catalog_refuses_when_its_template_goes`]: the old words for a
/// failed read, with the helm's, when the templates become unavailable while the taken row waits for the catalog.
#[gpui::test]
fn a_tl_row_waiting_for_the_catalog_refuses_when_the_templates_become_unavailable(
    cx: &mut TestAppContext,
) {
    use farhelm_proto::ui_channel::{Availability, Collection, CollectionChanges};
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    type_in(cx, "tl:partial");
    keys(cx, "enter");
    let mut meta = test_support::meta(&["partial", "go", "nowhere"]);
    meta.availability = Availability::Unavailable {
        error: "busy".into(),
    };
    helm.push(CollectionChanges {
        collection: Collection::Templates,
        meta,
        upsert: Vec::new(),
        remove: Vec::new(),
    });
    settle(cx);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    assert_eq!(draft(cx).folder.value(), "~", "nothing applied");
    assert_eq!(
        name_of(cx, "launcher-note").as_deref(),
        Some("Couldn't load templates: busy; search tl: and retry")
    );
}

/// SPEC.md: a template list that cannot be read is "a reason shown in the results area", in the old launcher's words.
/// Saying "No template matches" instead would tell the user their templates are gone. Specified: with the helm's
/// templates unavailable ("busy"), `tl:` shows "Couldn't load templates: busy" in the results and offers no row.
#[gpui::test]
fn unavailable_templates_are_the_reason_in_the_results(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher_with_templates(cx, Some("busy"));
    type_in(cx, "tl:");
    assert!(results_shown(cx), "the results open with the reason");
    assert_eq!(
        name_of(cx, "launcher-search-reason").as_deref(),
        Some("Couldn't load templates: busy")
    );
    assert_eq!(name_of(cx, "launcher-search-option-0"), None, "no row");
}

// ---- Rows from the launch history ----------------------------------------------------------------------------------

/// The helm's history answer holding no launches and the one saved folder `path`.
fn saved_folder(path: &str) -> serde_json::Value {
    let mut answer = history(Vec::new());
    answer["folders"] = serde_json::json!([{
        "host": 1,
        "canonical_cwd": path,
        "canonical_proven": true,
        "display_cwd": path,
        "created_at": 1,
        "creation_seq": null,
    }]);
    answer
}

/// A recent setup taken from search fills the draft as a click on the strip's row does (its choices, its folder, the
/// Agent tab), empties the box and keeps focus there, and launches nothing, from Enter either. Why: the old search's
/// Recent row only filled; SPEC.md's "Enter on a focused row launches" is the strip's row, which has focus, not a
/// search row picked from the box (old `sidebar.spec.ts` composer search recents, and `apply_search_result`'s Recent
/// arm).
#[gpui::test]
fn a_recent_setup_from_search_fills_and_launches_nothing(cx: &mut TestAppContext) {
    // The launcher starts at home, so only taking the setup can put the folder in `/tmp`.
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "~".into(), None),
        fixture_hosts(&[0]),
    );
    answer_history(&mut helm, cx, &two_setups());
    assert_eq!(
        draft(cx).folder.value(),
        "~",
        "the premise: the launcher starts at home"
    );
    test_support::click(cx, "launcher-tabs-Command");
    test_support::click(cx, "launcher-search-row");
    type_in(cx, "recent:claude");
    let name = name_of(cx, "launcher-search-option-0").unwrap_or_default();
    assert!(
        name.starts_with("Recent setup: /tmp · ") && name.contains(" · Claude · "),
        "the premise: the Claude setup is the first row: {name}"
    );
    keys(cx, "enter");
    let filled = draft(cx);
    assert_eq!(filled.tab, Tab::Agent);
    assert_eq!(filled.agent.harness, Some(LaunchHarness::Claude));
    assert_eq!(filled.folder.value(), "/tmp");
    assert_eq!(search_text(cx), "");
    assert!(search_focused(cx));
    assert!(
        creates(&mut helm, cx).is_empty(),
        "Enter here launches nothing"
    );
}

/// A saved folder taken from search becomes the folder, and the field shows it (that the folder list then stops
/// filtering is `folder_list_tests::a_path_from_search_does_not_filter_the_next_list`, through the same arm). Why:
/// SPEC.md, "Creation": the launch-composer search matches "folders and recent setups", and "Folder search uses that
/// bounded history".
#[gpui::test]
fn a_saved_folder_from_search_sets_the_folder(cx: &mut TestAppContext) {
    let (_helm, cx) = new_in_tmp(cx, &saved_folder("/srv/project"));
    type_in(cx, "project");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Use this path: /srv/project"),
        "the premise: the saved folder is offered"
    );
    keys(cx, "enter");
    assert_eq!(draft(cx).folder.value(), "/srv/project");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.folder.text(cx).to_string()),
        "/srv/project",
        "the folder field follows the draft"
    );
}

/// A model from a past launch that the catalog does not list chooses the agent type it ran with and the model as that
/// type's custom id. Why: a custom model typed once is offered again, and it only means something to its own agent
/// type (old `search_offers_a_used_custom_model_with_its_harness`, taken).
#[gpui::test]
fn a_past_model_from_search_chooses_its_agent_type(cx: &mut TestAppContext) {
    let answer = history(vec![launched(
        "/tmp",
        &chosen(LaunchHarness::Claude, Some("custom-x"), None, None),
    )]);
    let (_helm, cx) = new_in_tmp(cx, &answer);
    assert_eq!(draft(cx).agent.harness, None, "the premise: no agent type");
    type_in(cx, "model:custom");
    keys(cx, "enter");
    let chosen = draft(cx);
    assert_eq!(chosen.agent.harness, Some(LaunchHarness::Claude));
    assert_eq!(chosen.agent.model.as_deref(), Some("custom-x"));
    assert_eq!(chosen.agent.model_owner, Some(LaunchHarness::Claude));
}

/// Restart with refuses a recent setup and a saved folder from search with the destination's words, and Replace with
/// takes a recent setup (it holds only the host). A setup with a retired container choice is refused above the box
/// with the shared reason. Why: SPEC.md, "Restart with": "Search results and recent setups that set a held field are
/// still refused", and a container setup supports Delete only (old `retired_recent_is_refused_without_changing_the_draft`).
#[gpui::test]
fn history_rows_are_refused_where_they_cannot_apply(cx: &mut TestAppContext) {
    let mut answer = history(vec![launched(
        "/src/project",
        &chosen(LaunchHarness::Codex, None, None, None),
    )]);
    answer["folders"] = saved_folder("/srv/project")["folders"].clone();
    let mut source = super::draft_tests::session(codex(None, None, None));
    source.restart_offer = RestartOffer::Resume;
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::restart_with("missing", &source, Some(1)).unwrap(),
        fixture_hosts(&[0]),
    );
    answer_history(&mut helm, cx, &answer);
    test_support::click(cx, "launcher-search-row");
    let before = draft(cx);
    type_in(cx, "recent:");
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("destination cannot change here: this dialog keeps destination fixed for restart")
    );
    assert_eq!(draft(cx), before, "nothing changed");
    for _ in 0.."recent:".len() {
        keys(cx, "backspace");
    }
    type_in(cx, "folder:project");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Use this path: /srv/project"),
        "the premise: the saved folder is offered"
    );
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("destination cannot change here: this dialog keeps destination fixed for restart")
    );
    assert_eq!(draft(cx), before, "nothing changed");

    let replaced = super::draft_tests::session(codex(None, None, None));
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::replace_with("a", &replaced, Some(1)),
        fixture_hosts(&[0]),
    );
    answer_history(&mut helm, cx, &answer);
    type_in(cx, "recent:project");
    keys(cx, "enter");
    assert_eq!(name_of(cx, "launcher-search-refusal"), None);
    assert_eq!(
        draft(cx).folder.value(),
        "/src/project",
        "Replace with takes it"
    );

    let mut retired = chosen(LaunchHarness::Codex, None, None, None);
    retired.container = Some(
        serde_json::from_value(
            serde_json::json!({ "engine": "podman", "image": "old", "api_proxy_port": 1 }),
        )
        .unwrap(),
    );
    let (_helm, cx) = new_in_tmp(cx, &history(vec![launched("/tmp", &retired)]));
    let before = draft(cx);
    type_in(cx, "recent:");
    keys(cx, "enter");
    assert!(
        name_of(cx, "launcher-search-refusal").is_some_and(|words| words.contains("Delete only")),
        "the shared reason is said above the box"
    );
    assert_eq!(draft(cx), before, "nothing changed");
    assert_eq!(search_text(cx), "recent:", "the box keeps its text");
}

/// A saved folder taken from search while a managed checkout is chosen leaves the checkout for that folder, and keeps
/// the tab and its command as they were. Why: the folder is an ordinary launch's destination, so it must not clone a
/// repository or take a checkout's ownership just because one was chosen before (old `github-checkouts.spec.ts`, "an
/// unlabeled folder result replaces fresh intent without cloning").
#[gpui::test]
fn a_saved_folder_from_search_leaves_a_managed_checkout(cx: &mut TestAppContext) {
    let mut checkout = LaunchDraft::new(Some(1), "/tmp".into(), None);
    checkout.choose_tab(Tab::Command);
    checkout.choose_destination(super::draft::DestinationKind::Checkout);
    checkout.choose_repository(farhelm_proto::GithubRepo {
        owner: "acme".into(),
        name: "bar".into(),
    });
    let (mut helm, cx) = launcher_with_hosts(cx, checkout, fixture_hosts(&[0]));
    answer_history(&mut helm, cx, &saved_folder("/srv/project"));
    assert_eq!(
        draft(cx).destination_kind(),
        super::draft::DestinationKind::Checkout,
        "the premise: a managed checkout is chosen"
    );
    type_in(cx, "project");
    keys(cx, "enter");
    let after = draft(cx);
    assert_eq!(
        after.destination_kind(),
        super::draft::DestinationKind::Folder
    );
    assert_eq!(after.folder.value(), "/srv/project");
    assert_eq!(after.tab, Tab::Command, "the tab stays");
}

/// Type `o`, move the highlight down one row, let `answer` rebuild the rows, and press Enter: what the launcher chose
/// shows which row Enter took. Returns the agent type the first row of `o` names, which is the row the text names.
fn arrow_then_answer(
    cx: &mut TestAppContext,
    answer: impl FnOnce(&mut farhelm_helm_client::fake::FakeHelm, &mut VisualTestContext),
) -> (Option<LaunchHarness>, LaunchHarness) {
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "~".into(), None),
        fixture_hosts(&[0]),
    );
    type_in(cx, "o");
    let first = read(cx, |launcher, _, cx| launcher.search_results(cx).rows);
    let (Some(super::search::Row::Harness(named)), Some(_)) = (first.first(), first.get(1)) else {
        panic!("the premise: `o` names an agent type first and offers another row: {first:?}");
    };
    let named = *named;
    keys(cx, "down");
    assert_eq!(
        selected_state(cx, "launcher-search-option-1"),
        Some(true),
        "the premise: Down highlighted the second row"
    );
    answer(&mut helm, cx);
    let rows = read(cx, |launcher, _, cx| launcher.search_results(cx).rows);
    assert_ne!(rows, first, "the premise: the answer rebuilt the rows");
    keys(cx, "enter");
    (draft(cx).agent.harness, named)
}

/// A history answer that arrives while the results are open puts the highlight back on the row the text names, so
/// Enter takes that row rather than whatever row now sits at the index the arrow left. Why: the highlight is an index
/// into rows the answer rebuilds (it adds saved folders, recent setups and past models); the folder list drops its
/// highlight for the same reason.
#[gpui::test]
fn a_history_answer_resets_the_search_highlight(cx: &mut TestAppContext) {
    let (chosen_harness, named) = arrow_then_answer(cx, |helm, cx| {
        answer_history(
            helm,
            cx,
            &history(vec![launched(
                "/tmp",
                &chosen(LaunchHarness::Codex, Some("o-model"), None, None),
            )]),
        )
    });
    assert_eq!(
        chosen_harness,
        Some(named),
        "Enter took the row the text names"
    );
}

/// The catalog's answer resets the highlight the same way: it adds its models and drops past models it turns out to
/// list. Why: as for the history answer, Enter must not take a row the arrow never reached.
#[gpui::test]
fn a_catalog_answer_resets_the_search_highlight(cx: &mut TestAppContext) {
    let (chosen_harness, named) = arrow_then_answer(cx, |helm, cx| {
        answer_catalog(
            helm,
            cx,
            Some(serde_json::json!([{ "id": "o-cat", "harness": "codex", "efforts": [] }])),
        )
    });
    assert_eq!(
        chosen_harness,
        Some(named),
        "Enter took the row the text names"
    );
}

/// New with `harness` and `chosen` picked by hand, then `tl:<template>` (a template of that agent type with
/// `permissions: null`) taken with Enter and Launch pressed: checks that the template reset the permission, and returns
/// the permission the one create carried.
fn reset_by_template_and_launch(
    cx: &mut TestAppContext,
    harness: LaunchHarness,
    chosen: LaunchPermission,
    template: &str,
) -> serde_json::Value {
    let mut start = LaunchDraft::new(Some(1), "~".into(), None);
    let _ = start.choose_harness(harness, None, None);
    start.choose_permissions(Some(chosen));
    let (mut helm, cx) = launcher_with_templates_on(cx, None, start);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    assert_eq!(
        draft(cx).agent.permissions,
        Some(chosen),
        "the premise: the hand choice"
    );
    type_in(cx, &format!("tl:{template}"));
    keys(cx, "enter");
    let applied = draft(cx);
    assert_eq!(applied.agent.harness, Some(harness));
    assert_eq!(
        applied.agent.permissions, None,
        "the template reset the permission"
    );
    test_support::click(cx, "launcher-launch");
    let sent = super::launch_tests::creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one create, not {}", sent.len());
    };
    create.body.clone().unwrap_or_default()["launch"]["permissions"].clone()
}

/// A template's `permissions: null` is "a real template reset, not a visible permission click" (the old
/// terminal-create-idempotency spec): it puts back the agent type's own default, overriding a choice the user made by
/// hand, and the launch carries that default. Keeping the hand choice would launch with a permission the template meant
/// to clear. Specified: New with Codex and YOLO chosen by hand, `tl:reset-codex` and Enter leave the permission unset
/// (the default), and Launch sends a create with `guarded`, Codex's default.
#[gpui::test]
fn a_tl_reset_of_codexs_permission_launches_guarded(cx: &mut TestAppContext) {
    let sent = reset_by_template_and_launch(
        cx,
        LaunchHarness::Codex,
        LaunchPermission::Yolo,
        "reset-codex",
    );
    assert_eq!(sent, serde_json::json!("guarded"));
}

/// The Goose twin of the case above, whose default is the other way round (the old spec ran both): a reset must give
/// the agent type's own default, not a fixed one. Specified: New with Goose and Approve chosen by hand,
/// `tl:reset-goose` and Enter leave the permission unset, and Launch sends a create with `yolo`, Goose's default.
#[gpui::test]
fn a_tl_reset_of_gooses_permission_launches_yolo(cx: &mut TestAppContext) {
    let sent = reset_by_template_and_launch(
        cx,
        LaunchHarness::Goose,
        LaunchPermission::Approve,
        "reset-goose",
    );
    assert_eq!(sent, serde_json::json!("yolo"));
}

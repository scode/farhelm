//! Tests of Custom permission flags in the real launcher over the helm client's fake: the flags as the user typed them
//! surviving the choices that keep the agent, going with Reset choices, refusing Launch while they hold no word, and
//! reaching the create (and Replace with's replace) as the words they parse to. Ported from the old app's
//! `custom-permissions` spec and the Custom cases of its `launcher-enter` spec at reference commit `24bc0580`.
//!
//! The flags are a draft, not their words. SPEC.md, "Launch composer", keeps Custom flags as literal words owned by the
//! agent type they were written for, so the editor holds whatever was typed (an unclosed quote, extra spaces) until the
//! user changes it, and only the request carries the parsed words. Most tests here pin one of those two halves: the
//! text kept byte for byte, or the words sent.
//!
//! Every step ends in one of the shared harness's step helpers, so the focus and layer invariants are checked after
//! each. The fake answers commands in the order they were sent, so each test answers what the window sent before
//! answering a create or Replace it held. A case with several drafts is one test per draft over a shared driver, so
//! each draft gets its own app, window and fake. When the same-agent case ran both drafts in one test, its second
//! draft failed checks the first passed (the search row not drawn, then the search box not focused), and a test of one
//! draft passed them; the cause was not established ([`same_agent_choices_keep`]).

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{CollectionState, Preferences};
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::draft::LaunchDraft;
use super::launch_tests::{creates, launched_with, press};
use super::search_view_tests::type_in;
use super::view_tests::{
    Field, answer_catalog, click_in_body, draft, fixture_hosts, focused, has_node, launch_help,
    launcher_on, launcher_over, model_catalog, model_text, name_of, read, search_focused,
    search_text, shown_refusal,
};
use super::yolo_tests::refuse_for_yolo;
use crate::session_list::tests::{listing, open_main, preferences, preferences_state, session};
use crate::test_support::{self, any_open, keys, settle};

// ---- Helpers ---------------------------------------------------------------------------------------------------------

/// The text the open launcher's flags field holds.
fn flags_text(cx: &mut VisualTestContext) -> String {
    read(cx, |launcher, _, cx| launcher.flags.text(cx).to_string())
}

/// New on host1 in `~`, with `catalog` answered when given, the agent chip `chip` clicked and then Custom, checked to
/// have moved focus into the flags field (where the tests type).
fn custom_launcher<'a>(
    cx: &'a mut TestAppContext,
    chip: &'static str,
    catalog: Option<Value>,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    if let Some(catalog) = catalog {
        answer_catalog(&mut helm, cx, Some(catalog));
    }
    test_support::click(cx, chip);
    test_support::click(cx, "launcher-permissions-Custom");
    assert_eq!(draft(cx).agent.permissions, Some(LaunchPermission::Custom));
    assert!(
        focused(cx, Field::Flags),
        "the premise: choosing Custom put focus in its flags"
    );
    (helm, cx)
}

/// The helm refuses request `id`; the launcher, which the user has not moved on from, comes back with its draft.
fn refuse(helm: &mut FakeHelm, cx: &mut VisualTestContext, id: u64) {
    helm.reply(id, 400, Some(json!("refused by this test")));
    settle(cx);
    assert!(any_open(cx), "the launcher came back after the refusal");
}

// ---- Choices that keep or end the flags ------------------------------------------------------------------------------

/// Reset choices ends Claude's ownership of its flags while the remembered YOLO survives, so Codex chosen from search
/// afterwards starts its own Custom empty and launches only the words typed for it (custom-permissions.spec's "reset
/// then agent search cannot restore another agent's custom flags"). Why: flags are written for one agent's command
/// line, and nothing but the user's own typing may put words into another agent's Custom. The old case noted that its
/// regression needed a remembered permission both agents offer, so the helm here remembers YOLO. Specified: after
/// Reset there is no agent, the remembered YOLO and no flags; Codex's Custom then starts empty, and Launch sends Codex,
/// Custom and the words typed for it, without consent.
#[gpui::test]
fn reset_then_another_agents_custom_starts_empty(cx: &mut TestAppContext) {
    let remembered_yolo = preferences_state(Preferences {
        remembered_permissions: Some("yolo".into()),
        // A helm that has shown the Welcome card, which would otherwise open over the main view (`crate::welcome`).
        welcome_seen: Some(true),
        ..Preferences::default()
    });
    let (mut helm, cx) = launcher_over(
        cx,
        LaunchDraft::new(Some(1), "~".into(), Some(LaunchPermission::Yolo)),
        vec![
            listing(&["a"], &[session("a")]),
            fixture_hosts(&[0]),
            remembered_yolo,
        ],
    );
    test_support::click(cx, "launcher-harness-Claude");
    assert_eq!(
        draft(cx).agent.permissions,
        Some(LaunchPermission::Yolo),
        "the premise: Claude shows YOLO"
    );
    test_support::click(cx, "launcher-permissions-Custom");
    assert!(
        focused(cx, Field::Flags),
        "the premise: Custom focused its flags"
    );
    type_in(cx, "  --mode \"claude only\"   ");
    assert_eq!(
        draft(cx).agent.permission_flags,
        "  --mode \"claude only\"   ",
        "the premise: Claude's flags were typed"
    );

    test_support::click(cx, "launcher-reset");
    let reset = draft(cx);
    assert_eq!(reset.agent.harness, None);
    assert_eq!(
        reset.agent.permissions,
        Some(LaunchPermission::Yolo),
        "the helm's remembered YOLO, which Reset reads from the preferences"
    );
    assert_eq!(
        reset.agent.permission_flags, "",
        "the Reset ended Claude's flags"
    );

    test_support::click(cx, "launcher-search-row");
    type_in(cx, "harness:codex");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Harness: Codex"),
        "the premise: search offers Codex"
    );
    keys(cx, "enter");
    assert_eq!(search_text(cx), "", "the row was taken");
    let codex = draft(cx);
    assert_eq!(codex.agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(codex.agent.permissions, Some(LaunchPermission::Yolo));

    test_support::click(cx, "launcher-permissions-Custom");
    assert_eq!(flags_text(cx), "", "Codex's Custom starts empty");
    assert_eq!(draft(cx).agent.permission_flags, "");
    assert!(!has_node(cx, "launcher-flag-words"), "and shows no words");
    assert!(focused(cx, Field::Flags), "with focus in its flags");

    type_in(cx, "--mode \"codex only\"");
    let create = press(&mut helm, cx, "/api/sessions");
    let body = create.body.expect("a create carries a body");
    assert_eq!(body["launch"]["harness"], json!("codex"));
    assert_eq!(body["launch"]["permissions"], json!("custom"));
    assert_eq!(
        body["launch"]["permission_flags"],
        json!(["--mode", "codex only"])
    );
    assert_eq!(body["confirm_yolo"], json!(false), "no consent was given");
}

/// The choices that keep the agent keep the flags exactly as typed: a model from search, the agent's own row from
/// search, its chip, and Enter on that chip (custom-permissions.spec's "same-agent search and chips preserve custom
/// permission drafts byte for byte", whose two drafts are the two tests below). Why: the flags are a draft until
/// launch, and rebuilding the field from the words they parse to would close an unfinished quote for the user or eat
/// the spacing they typed. The agent switch that does end them is
/// `draft_tests::custom_flags_stay_with_their_agent_and_their_choice`'s. Specified, on Claude with Custom and the flags
/// `typed` (which parse to `words`, said under the field, or do not parse at all): `model:sonnet` and `harness:claude`
/// from search and a click on the Claude chip each leave Custom chosen with the field and the draft as typed; for flags
/// that do not parse, which Launch refuses, so does Enter on the chip.
///
/// The launcher opens on the draft, so focus starts in the search box and stays there as each row is taken (typing
/// into the flags is `view_tests::custom_opens_its_flags_editor_and_enter_there_only_chooses`'s). Each draft is a test
/// of its own, over its own app and window: a diagnostic run kept focus in the search box through every step in a
/// test's first window, while the one-test version of this case failed that premise once its second draft opened a
/// second window in the same app.
fn same_agent_choices_keep(cx: &mut TestAppContext, typed: &str, words: Option<&str>) {
    let mut start = LaunchDraft::new(Some(1), "~".into(), None);
    let _ = start.choose_harness(LaunchHarness::Claude, None, None);
    start.choose_permissions(Some(LaunchPermission::Custom));
    start.edit_permission_flags(typed.into());
    let (mut helm, cx) = launcher_on(cx, start);
    answer_catalog(&mut helm, cx, Some(model_catalog()));
    assert_eq!(
        flags_text(cx),
        typed,
        "the premise: the field shows {typed:?}"
    );
    match words {
        None => assert!(
            has_node(cx, "launcher-flags-error"),
            "the premise: {typed:?} does not parse"
        ),
        Some(words) => assert_eq!(
            name_of(cx, "launcher-flag-words").as_deref(),
            Some(words),
            "the premise: {typed:?} parses"
        ),
    }
    for (query, row) in [
        ("model:sonnet", "Model: sonnet (Claude)"),
        ("harness:claude", "Harness: Claude"),
    ] {
        assert!(search_focused(cx), "the premise: the search box has focus");
        type_in(cx, query);
        assert_eq!(
            name_of(cx, "launcher-search-option-0").as_deref(),
            Some(row),
            "the premise: search offers {row}"
        );
        keys(cx, "enter");
        assert_eq!(search_text(cx), "", "{query}: the row was taken");
        let after = draft(cx);
        assert_eq!(
            after.agent.permissions,
            Some(LaunchPermission::Custom),
            "{query}"
        );
        assert_eq!(after.agent.permission_flags, typed, "{query}: the draft");
        assert_eq!(flags_text(cx), typed, "{query}: the field");
    }
    test_support::click(cx, "launcher-harness-Claude");
    assert_eq!(flags_text(cx), typed, "the Claude chip keeps {typed:?}");
    if words.is_none() {
        assert_eq!(
            test_support::focused_id(cx).as_deref(),
            Some("launcher-harness-Claude"),
            "the premise: the click focused the chip"
        );
        keys(cx, "enter");
        assert!(
            shown_refusal(cx).is_some(),
            "the premise: Enter on the chip pressed Launch, which refused"
        );
        assert_eq!(flags_text(cx), typed, "Enter on the chip keeps {typed:?}");
        assert_eq!(draft(cx).agent.permission_flags, typed);
    }
}

/// [`same_agent_choices_keep`] for an unclosed quote, which does not parse, so Launch refuses and Enter on the chip is
/// checked too.
#[gpui::test]
fn same_agent_choices_keep_unclosed_flags_as_typed(cx: &mut TestAppContext) {
    same_agent_choices_keep(cx, "--permission-mode 'ask me", None);
}

/// [`same_agent_choices_keep`] for flags with spacing around and between their words, which parse to three words.
#[gpui::test]
fn same_agent_choices_keep_spaced_flags_as_typed(cx: &mut TestAppContext) {
    same_agent_choices_keep(
        cx,
        "  --mode \"ask me\"   '$HOME'  ",
        Some("Permission arguments: --mode, ask me, $HOME"),
    );
}

/// A typed custom model keeps an effort the agent's catalog offers, whatever state the flags are in
/// (custom-permissions.spec's "custom model acceptance keeps compatible effort with incomplete permission flags", whose
/// two drafts are the two tests below). Why: flags that do not parse yet make the whole launch incomplete, but they are
/// repairable on their own, and the effort is an independent choice that taking a model must not quietly change
/// because of them. Specified, on Claude with Custom, the flags `typed` and High: the model `id` typed into the open
/// model list and taken with Enter is chosen and shown in the closed field, with High and the flags unchanged.
fn a_custom_model_keeps_its_effort(cx: &mut TestAppContext, typed: &str, id: &str) {
    let (_helm, cx) = custom_launcher(cx, "launcher-harness-Claude", Some(model_catalog()));
    if !typed.is_empty() {
        type_in(cx, typed);
    }
    test_support::click(cx, "launcher-effort-High");
    assert_eq!(
        draft(cx).agent.effort,
        Some(LaunchEffort::High),
        "the premise: High is chosen"
    );
    assert!(
        launch_help(cx).is_some_and(|help| help.contains("custom permission flags")),
        "the premise: {typed:?} leaves the launch incomplete"
    );
    test_support::click(cx, "launcher-model-row");
    assert!(
        focused(cx, Field::Model),
        "the premise: the click focused the model field"
    );
    type_in(cx, id);
    assert!(
        has_node(cx, "launcher-model-list"),
        "the premise: typing left the list open"
    );
    keys(cx, "enter");
    assert!(
        !has_node(cx, "launcher-model-list"),
        "Enter took the id and closed the list"
    );
    let after = draft(cx);
    assert_eq!(after.agent.model.as_deref(), Some(id));
    assert_eq!(model_text(cx), id, "the closed field shows the id");
    assert_eq!(
        after.agent.effort,
        Some(LaunchEffort::High),
        "{typed:?}: the effort stays"
    );
    assert_eq!(after.agent.permission_flags, typed, "and so do the flags");
}

/// [`a_custom_model_keeps_its_effort`] with Custom chosen and no flags typed yet.
#[gpui::test]
fn a_custom_model_keeps_its_effort_beside_empty_flags(cx: &mut TestAppContext) {
    a_custom_model_keeps_its_effort(cx, "", "private-permission-test-model-0");
}

/// [`a_custom_model_keeps_its_effort`] with flags holding an unclosed quote.
#[gpui::test]
fn a_custom_model_keeps_its_effort_beside_unclosed_flags(cx: &mut TestAppContext) {
    a_custom_model_keeps_its_effort(cx, "--mode '", "private-permission-test-model-1");
}

// ---- What Launch refuses and sends -----------------------------------------------------------------------------------

/// Flags holding no word refuse Launch, with the flags' own reason both as Launch's hover help and after a press from
/// the field (launcher-enter.spec's "launcher Enter names invalid custom permission flags", its empty and blank drafts,
/// the two tests below; the unclosed quote is `request_tests::refusals_name_the_first_thing_to_fix`). Why: flags
/// holding no word are a repairable editor problem, not a catalog mismatch, so the reason must name the flags, or the
/// user would go looking at the model and effort; and it must be said here, before anything reaches the helm.
/// Specified, on Codex with Custom and the flags `typed`: Launch's help and, after Enter in the field, the line beside
/// Launch give that reason, and no create is sent.
fn wordless_flags_refuse_launch(cx: &mut TestAppContext, typed: &str) {
    const REASON: &str = "Enter valid custom permission flags before launching";
    let (mut helm, cx) = custom_launcher(cx, "launcher-harness-Codex", None);
    if !typed.is_empty() {
        type_in(cx, typed);
    }
    assert_eq!(
        flags_text(cx),
        typed,
        "the premise: the field holds {typed:?}"
    );
    assert_eq!(launch_help(cx).as_deref(), Some(REASON), "{typed:?}");
    assert!(
        focused(cx, Field::Flags),
        "the premise: the flags still hold focus"
    );
    keys(cx, "enter");
    assert_eq!(
        shown_refusal(cx),
        Some(format!("Not launched. {REASON}")),
        "{typed:?}"
    );
    assert!(
        creates(&mut helm, cx).is_empty(),
        "{typed:?}: nothing was sent"
    );
}

/// [`wordless_flags_refuse_launch`] with Custom chosen and nothing typed.
#[gpui::test]
fn empty_flags_refuse_launch_and_say_why(cx: &mut TestAppContext) {
    wordless_flags_refuse_launch(cx, "");
}

/// [`wordless_flags_refuse_launch`] with two spaces typed, which parse to no word.
#[gpui::test]
fn blank_flags_refuse_launch_and_say_why(cx: &mut TestAppContext) {
    wordless_flags_refuse_launch(cx, "  ");
}

/// The create carries the flags as the words they parse to, so spacing typed after them keeps the field as typed and
/// the request as it was, and only a further word changes the request (launcher-enter.spec's "valid ${editor} spacing
/// preserves raw text and retry words", its custom flags variant; the retry key it also compared is gone, since the GUI
/// sends none). Why: the field is the user's text and the words are the launch; a trailing space must neither vanish
/// from the field nor reach the agent as an empty argument. Specified, on Codex with Custom: `--mode ask` sends
/// `--mode`, `ask`; a trailing space then stays in the field and the same create goes out again; a further `x` sends
/// `--mode`, `ask`, `x`.
#[gpui::test]
fn a_trailing_space_keeps_the_words_and_a_new_word_changes_them(cx: &mut TestAppContext) {
    let (mut helm, cx) = custom_launcher(cx, "launcher-harness-Codex", None);
    type_in(cx, "--mode ask");
    let first = press(&mut helm, cx, "/api/sessions");
    let body = first.body.clone().expect("a create carries a body");
    assert_eq!(body["launch"]["permission_flags"], json!(["--mode", "ask"]));
    refuse(&mut helm, cx, first.id);

    click_in_body(cx, "launcher-flags-row");
    assert!(
        focused(cx, Field::Flags),
        "the premise: the click focused the flags"
    );
    keys(cx, "end space");
    assert_eq!(flags_text(cx), "--mode ask ", "the field keeps the space");
    assert_eq!(
        name_of(cx, "launcher-flag-words").as_deref(),
        Some("Permission arguments: --mode, ask"),
        "the words are unchanged"
    );
    let same = press(&mut helm, cx, "/api/sessions");
    assert_eq!(same.body, first.body, "the same create, word for word");
    refuse(&mut helm, cx, same.id);
    assert_eq!(
        flags_text(cx),
        "--mode ask ",
        "the space survived the press"
    );

    click_in_body(cx, "launcher-flags-row");
    assert!(
        focused(cx, Field::Flags),
        "the premise: the click focused the flags again"
    );
    keys(cx, "end x");
    assert_eq!(flags_text(cx), "--mode ask x");
    let changed = press(&mut helm, cx, "/api/sessions");
    let body = changed.body.expect("a create carries a body");
    assert_eq!(
        body["launch"]["permission_flags"],
        json!(["--mode", "ask", "x"])
    );
}

/// The words a session was launched with, holding a space, a literal `$HOME` and a zero-width space.
const STORED: [&str; 4] = ["--mode", "ask me", "$HOME", "hidden\u{200b}word"];

/// Session `src` on host1 (its recorded install, so a Clone keeps the host), ended, launched as Claude with Custom and
/// the [`STORED`] words, with host1 and the preferences around it. The launcher reads only the selection; the stored
/// command lines are the shortest the wire form takes.
fn custom_source_fleet() -> Vec<CollectionState> {
    let launch = json!({
        "kind": "agent",
        "selection": { "harness": "claude", "permissions": "custom", "permission_flags": STORED },
        "start": ["claude", "{farhelm_args}"],
    });
    let mut sessions = listing(
        &["src"],
        &[session("src")
            .host(1)
            .status(json!({ "state": "exited", "exit_code": 0 }))
            .unmanaged()
            .not_launched_as_checkout()
            .launch(launch)],
    );
    sessions.entries[0].value["host_identity"] = json!("identity1");
    vec![sessions, fixture_hosts(&[0]), preferences(&[], None)]
}

/// Clone and Replace with of a Custom launch send its stored words, and after an edit the edited ones
/// (custom-permissions.spec's "${action} preserves custom words and shows the stored YOLO provenance", for clone and
/// replace-with, the two tests below; the row's shield and help are the session list's). Why: the stored words hold
/// characters the field can only show spelled out, so what is sent must come from the stored words, not from the
/// field's spelling of them. Specified, with `mode` opened on `src` by `open`: the field shows `--mode 'ask me' '$HOME'
/// hidden<U+200B>word`; Launch sends one request to `path` whose launch, at `launch_at`, is Claude with Custom and
/// exactly the stored words; after a refusal, the field replaced by `--mode "edited words"` sends those two words.
fn sends_the_stored_words_then_an_edits(
    cx: &mut TestAppContext,
    mode: &str,
    open: fn(&str, &mut gpui::Window, &mut gpui::App) -> bool,
    path: &str,
    launch_at: &str,
) {
    let (mut helm, cx) = open_main(cx, custom_source_fleet());
    assert!(
        cx.update(|window, cx| open("src", window, cx)),
        "{mode} opened"
    );
    settle(cx);
    assert_eq!(
        flags_text(cx),
        "--mode 'ask me' '$HOME' hidden<U+200B>word",
        "{mode}: the stored words, quoted back and spelled out"
    );
    let first = press(&mut helm, cx, path);
    let body = first.body.clone().expect("the request carries a body");
    let launch = body
        .pointer(launch_at)
        .expect("the request carries a launch");
    assert_eq!(launch["harness"], json!("claude"), "{mode}");
    assert_eq!(launch["permissions"], json!("custom"), "{mode}");
    assert_eq!(
        launch["permission_flags"],
        json!(STORED),
        "{mode}: the stored words, raw"
    );
    refuse(&mut helm, cx, first.id);

    click_in_body(cx, "launcher-flags-row");
    assert!(
        focused(cx, Field::Flags),
        "the premise: the click focused the flags"
    );
    crate::test_support::select_all_text(cx);
    type_in(cx, "--mode \"edited words\"");
    assert_eq!(flags_text(cx), "--mode \"edited words\"");
    let edited = press(&mut helm, cx, path);
    let body = edited.body.clone().expect("the request carries a body");
    assert_eq!(
        body.pointer(launch_at)
            .map(|launch| &launch["permission_flags"]),
        Some(&json!(["--mode", "edited words"])),
        "{mode}: the edit's words"
    );
}

/// [`sends_the_stored_words_then_an_edits`] for Clone, whose create carries the launch at its top level.
#[gpui::test]
fn clone_sends_the_stored_words_then_an_edits(cx: &mut TestAppContext) {
    sends_the_stored_words_then_an_edits(
        cx,
        "Clone",
        super::entry::open_clone,
        "/api/sessions",
        "/launch",
    );
}

/// [`sends_the_stored_words_then_an_edits`] for Replace with, whose replace carries the create under `with`.
#[gpui::test]
fn replace_with_sends_the_stored_words_then_an_edits(cx: &mut TestAppContext) {
    sends_the_stored_words_then_an_edits(
        cx,
        "Replace with",
        super::entry::open_replace_with,
        "/api/sessions/src/replace",
        "/with/launch",
    );
}

/// A Custom launch the helm refuses for YOLO asks the question with the custom-flags reason, after a create carrying
/// the flags' words (custom-permissions.spec's "custom permission words reach the real YOLO refusal with their own
/// explanation"). Why: words that name a guarded mode of the agent are still unchecked by Farhelm, so the question must
/// not blame a YOLO choice the user did not make. Which reason each launch gets is
/// `yolo_tests::each_yolo_launch_gets_its_own_reason`; this is the reason drawn for the create the helm refused.
/// Specified: Claude with Custom `--permission-mode auto` sends those two words; the helm's YOLO refusal brings the
/// launcher back asking, with the custom-flags reason.
#[gpui::test]
fn custom_flags_reach_the_yolo_question_with_their_own_reason(cx: &mut TestAppContext) {
    let mut start = LaunchDraft::new(Some(1), "~".into(), None);
    let _ = start.choose_harness(LaunchHarness::Claude, None, None);
    start.choose_permissions(Some(LaunchPermission::Custom));
    start.edit_permission_flags("--permission-mode auto".into());
    let (mut helm, cx, create) = launched_with(cx, start);
    let body = create.body.as_ref().expect("a create carries a body");
    assert_eq!(body["launch"]["permissions"], json!("custom"));
    assert_eq!(
        body["launch"]["permission_flags"],
        json!(["--permission-mode", "auto"])
    );
    refuse_for_yolo(&mut helm, cx, create.id);
    assert!(any_open(cx), "the launcher came back");
    assert_eq!(
        name_of(cx, "prompt-confirm").as_deref(),
        Some("Start YOLO session anyway"),
        "the premise: the YOLO question is asked"
    );
    assert_eq!(
        name_of(cx, "yolo-reason").as_deref(),
        Some(
            "This launch uses custom permission flags. Farhelm does not check what they do, so it treats the \
             session as YOLO."
        )
    );
}

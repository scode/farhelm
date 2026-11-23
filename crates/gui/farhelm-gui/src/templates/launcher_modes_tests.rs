//! Tests of template shortcuts in the launchers that are not New's prefill (Clone, Replace with and Restart with) and of
//! the sequence's Tab completion in the launcher's form, in the real main view over the helm client's fake. Ported from
//! the old `launcher-template-shortcuts.spec.ts` cases "a number shortcut applies to Clone's draft", "Replace with
//! refuses a whole sequence and yields to another modal" and "typed launcher Tab completion is immediate and preserves
//! spaced names", and the old `restart-with.spec.ts` cases "restart with refuses held shortcut batches and bolt
//! templates" and "restart with applies matching bolt and shortcut templates without restarting", at reference commit
//! `24bc0580`.
//!
//! The Replace with case's "yields to another modal" half runs in Clone with the Templates dialog over it: the rule (a
//! shortcut acts on the launcher only while the launcher is the top layer that takes the keyboard) is the same in every
//! mode, and Clone's draft makes a later press's effect easy to read. The page's half is
//! `super::dispatch_tests::the_chord_and_number_keys_do_nothing_under_another_dialog`.
//!
//! The Clone and Replace with tests run over one session, `a`, on host9 as its recorded install `identity9`, with hosts
//! 1, 5, 9 and 11 registered, and these templates: `renamer` sets only the session name, and `elsewhere` names host1's
//! install `identity1`. The launcher's catalog read fails, as the Replace with tests answer it, since an unread catalog
//! refuses no choice and `a` carries the helm fixture's Codex launch, whose stored model an empty catalog would refuse.
//! The Restart with tests list `a` idle in the plain folder `/home/user/src/a` instead (`restart_window`), each with its
//! own templates, ⚡ and number keys. The Tab test uses the dispatcher tests' window and templates
//! (`super::dispatch_tests`).

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::launcher::{
    LaunchTemplate, LauncherKind, TemplateDestination, TemplateFields, TemplateNameKind,
};
use farhelm_proto::ui_channel::{Availability, Preferences};
use farhelm_proto::{LaunchEffort, LaunchHarness};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::dialog_tests::{template, templates_state};
use super::dispatch_tests::{self, chord, note, number_key, text};
use crate::actions::SEQUENCE_PREFIX_CHORD;
use crate::launcher::draft::Mode;
use crate::launcher::entry::{open_clone, open_replace_with, open_restart_with};
use crate::launcher::view_tests::{draft, fixture_hosts, search_focused, search_text};
use crate::layers;
use crate::main_view::MainView;
use crate::session_list::tests::{Fixture, listing, preferences_state, session};
use crate::test_support::{self, any_open, focused_id, keys, label, settle, started};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The real main view over the fake helm with session `a` open (the module docs' fleet), `renamer` and `elsewhere`
/// stored, and number keys `bound` (each a digit and the template name it holds).
fn fleet_window<'a>(
    cx: &'a mut TestAppContext,
    bound: &[(&str, &str)],
) -> (FakeHelm, &'a mut VisualTestContext) {
    let templates = [
        template(
            "elsewhere",
            TemplateFields {
                host: Some("identity1".into()),
                ..TemplateFields::default()
            },
        ),
        template(
            "renamer",
            TemplateFields {
                name: Some("applied by shortcut".into()),
                ..TemplateFields::default()
            },
        ),
    ];
    window_over(cx, session("a").host(9).tabs(1), &templates, None, bound)
}

/// The real main view over the fake helm with `a`, as the caller builds it, as the only session, recorded on install
/// `identity9` and open, hosts 1, 5, 9 and 11 registered, `templates` stored, ⚡ on `marked` if anything, and number
/// keys `bound`.
fn window_over<'a>(
    cx: &'a mut TestAppContext,
    a: Fixture,
    templates: &[LaunchTemplate],
    marked: Option<&str>,
    bound: &[(&str, &str)],
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut helm, _connection) = started(cx);
    let mut sessions = listing(&["a"], &[a]);
    sessions.entries[0].value["host_identity"] = json!("identity9");
    helm.connect(vec![
        sessions,
        fixture_hosts(&[0, 4, 8, 10]),
        preferences_state(Preferences {
            quick_launch_template: marked.map(str::to_owned),
            template_shortcuts: bound
                .iter()
                .map(|(key, name)| ((*key).to_owned(), (*name).to_owned()))
                .collect(),
            // A helm that has shown the Welcome card, which would otherwise open over the main view (`crate::welcome`).
            welcome_seen: Some(true),
            ..Preferences::default()
        }),
        templates_state(templates, Availability::Available),
    ]);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, MainView::new);
    window.update(|_, cx| crate::local_state::select("a", cx));
    settle(window);
    (helm, window)
}

/// The creates (`POST /api/sessions`), Replaces (`POST /api/sessions/{id}/replace`) and restarts
/// (`POST /api/sessions/{id}/restart`) sent since the last call, every other command answered at once and the catalog
/// read refused (the module docs say why), round after round until a round sends nothing else, so a press waiting for
/// the catalog has applied by the time this returns.
fn launches(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    let mut launches = Vec::new();
    loop {
        cx.run_until_parked();
        let mut answered = false;
        for command in helm.commands() {
            if command.path == "/api/sessions"
                || command.path.ends_with("/replace")
                || command.path.ends_with("/restart")
            {
                launches.push(command);
            } else if command.path.ends_with("launch-catalog") {
                helm.reply(
                    command.id,
                    503,
                    Some(json!("catalog unavailable in this test")),
                );
                answered = true;
            } else {
                helm.reply(command.id, 200, None);
                answered = true;
            }
        }
        settle(cx);
        if !answered {
            return launches;
        }
    }
}

/// What the open launcher's Name field holds.
fn name_value(cx: &mut VisualTestContext) -> String {
    draft(cx).name.value().to_string()
}

// ---- Clone and Replace with ----------------------------------------------------------------------------------------

/// SPEC.md: a template shortcut "applies its template to that draft and leaves Launch to the user" while the session
/// launcher is the only visible dialog, and Clone is that launcher too. A number key that launched, or reopened New, from
/// Clone would discard the copy the user is preparing, and applying by shortcut "leaves focus and search text alone".
/// Specified: with key 1 on `renamer`, Clone of `a` opened with focus in its search box and `q` typed there, key 1 puts
/// "applied by shortcut" in the Name field, keeps the launcher open in Clone mode with focus and `q` in its search box,
/// and sends no create.
#[gpui::test]
fn a_number_key_applies_to_clones_draft(cx: &mut TestAppContext) {
    let (mut helm, cx) = fleet_window(cx, &[("1", "renamer")]);
    assert!(
        cx.update(|window, cx| open_clone("a", window, cx)),
        "the premise: Clone opened"
    );
    settle(cx);
    assert!(launches(&mut helm, cx).is_empty(), "opening sends nothing");
    assert_ne!(name_value(cx), "applied by shortcut", "the premise");
    assert!(
        search_focused(cx),
        "the premise: Clone opens with focus in its search box"
    );
    cx.simulate_input("q");
    settle(cx);
    assert_eq!(search_text(cx), "q", "the premise: a query is typed");

    number_key(cx, 1);
    assert!(launches(&mut helm, cx).is_empty(), "nothing was launched");
    assert_eq!(name_value(cx), "applied by shortcut");
    assert!(matches!(draft(cx).mode, Mode::Clone), "still Clone's draft");
    assert!(any_open(cx), "Launch is left to the user");
    assert!(search_focused(cx), "focus stays in the search box");
    assert_eq!(search_text(cx), "q", "and so does the query");
}

/// SPEC.md: a template "that changes a field the current launcher mode holds fixed, such as the host in Replace with" is
/// refused with nothing from it applied, and a refusal "does not discard a later press"; a completed sequence is one
/// refusal unit. Applying part of a refused sequence would leave a draft the user never asked for, and losing the next
/// press would make the shortcuts seem dead. Specified: in Replace with of `a`, key 1 (`elsewhere`, another host) shows
/// a refusal saying its host cannot change and leaves the name alone; the unbound key 9 says "No template is bound to key
/// 9" and opens no Templates; the sequence 2 1 and Enter (`renamer`, then `elsewhere`) is refused whole, the name still
/// unchanged; key 2 alone then applies `renamer`; nothing is sent.
#[gpui::test]
fn replace_with_refuses_a_host_change_whole_and_takes_a_later_press(cx: &mut TestAppContext) {
    let (mut helm, cx) = fleet_window(cx, &[("1", "elsewhere"), ("2", "renamer")]);
    assert!(
        cx.update(|window, cx| open_replace_with("a", window, cx)),
        "the premise: Replace with opened"
    );
    settle(cx);
    assert!(launches(&mut helm, cx).is_empty(), "opening sends nothing");
    assert!(
        matches!(draft(cx).mode, Mode::ReplaceWith { .. }),
        "the premise"
    );
    let before = name_value(cx);

    number_key(cx, 1);
    let shown = note(cx).expect("elsewhere refused");
    assert!(shown.contains("its host cannot change here"), "{shown:?}");
    assert_eq!(
        name_value(cx),
        before,
        "the refused template changed nothing"
    );

    number_key(cx, 9);
    assert_eq!(note(cx).as_deref(), Some("No template is bound to key 9"));
    assert!(
        !text(cx).contains("Give a template a number key"),
        "Templates did not open"
    );

    keys(cx, &format!("{SEQUENCE_PREFIX_CHORD} 2 1 enter"));
    let shown = note(cx).expect("the sequence refused");
    assert!(shown.contains("its host cannot change here"), "{shown:?}");
    assert_eq!(
        name_value(cx),
        before,
        "renamer, queued before the refused template, did not apply either"
    );

    number_key(cx, 2);
    assert_eq!(
        name_value(cx),
        "applied by shortcut",
        "a later press applies"
    );
    assert!(launches(&mut helm, cx).is_empty(), "nothing was sent");
    assert!(any_open(cx), "Replace with stays open");
}

// ---- Tab in the launcher's sequence --------------------------------------------------------------------------------

/// SPEC.md: "Tab queues the full name when the word uniquely identifies a template", and in the launcher Enter applies
/// the queued templates to the draft without launching; applying by shortcut "leaves focus and search text alone". A Tab
/// that waited, or an apply that ate the user's query, would break the typing the sequence is for. Specified: with New
/// open, `keep` typed into its search box, the prefix, `par` and Tab queue `partial` at once (one entry in the armed
/// sequence, the word gone); Enter puts `partial`'s folder in the draft, sends nothing, and leaves `keep` in the search
/// box with focus.
#[gpui::test]
fn tab_completes_in_the_launchers_sequence_and_enter_applies_it(cx: &mut TestAppContext) {
    let (mut helm, cx) = dispatch_tests::window(cx, None);
    assert!(
        cx.update(crate::launcher::entry::open_new),
        "the premise: New opened"
    );
    assert!(
        dispatch_tests::creates(&mut helm, cx).is_empty(),
        "opening New sends no create"
    );
    assert!(
        search_focused(cx),
        "the premise: focus starts in the search box"
    );
    cx.simulate_input("keep");
    settle(cx);
    assert_eq!(search_text(cx), "keep", "the premise: the query is typed");

    keys(cx, &format!("{SEQUENCE_PREFIX_CHORD} p a r tab"));
    let sequence = cx
        .update(|_, cx| super::armed::armed_sequence(cx))
        .expect("the sequence is armed");
    assert_eq!(
        sequence.entries(),
        [super::sequence::Entry::Word("partial".into())],
        "Tab queued the full name"
    );
    assert_eq!(sequence.word(), "", "and the word is gone");

    keys(cx, "enter");
    assert_eq!(draft(cx).folder.value(), "/srv/partial", "partial applied");
    assert!(
        dispatch_tests::creates(&mut helm, cx).is_empty(),
        "nothing was launched"
    );
    assert_eq!(search_text(cx), "keep", "the query stayed");
    assert!(search_focused(cx), "and so did focus");
}

/// SPEC.md: "Other visible dialogs keep the keyboard", and a shortcut applies to the launcher only "while the session
/// launcher is the only visible dialog" (the old Replace with case's "yields to another modal" half, whose fixture was
/// a modal dialog over the launcher). A key that edited the draft under another dialog would change the launcher
/// behind what the user is looking at. The app has no path that opens Templates over the launcher; the test opens it
/// there through `templates::open` as a stand-in for any dialog above it, as the old case used a synthetic modal.
/// Specified: with key 2 on `renamer`, Clone of `a` with the Templates dialog opened over it, key 2 leaves Clone's Name
/// field unchanged and the Templates dialog open; once that dialog is closed with Escape, key 2 applies `renamer` to
/// Clone's draft.
#[gpui::test]
fn a_number_key_does_nothing_while_a_dialog_is_open_over_the_launcher(cx: &mut TestAppContext) {
    let (mut helm, cx) = fleet_window(cx, &[("2", "renamer")]);
    assert!(
        cx.update(|window, cx| open_clone("a", window, cx)),
        "the premise: Clone opened"
    );
    settle(cx);
    assert!(launches(&mut helm, cx).is_empty(), "opening sends nothing");
    let before = name_value(cx);
    let opened = cx.update(|window, cx| {
        let opener = cx.focus_handle();
        let mark = crate::focus::Mark::take(cx);
        super::open(&opener, &mark, window, cx)
    });
    settle(cx);
    assert!(opened.is_some(), "the premise: Templates opened over Clone");
    let templates_open = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| layers::last_opened::<super::dialog::TemplatesDialog>(cx).is_some())
    };
    assert!(templates_open(cx), "the premise: the dialog is open");
    number_key(cx, 2);
    assert!(launches(&mut helm, cx).is_empty(), "nothing was sent");
    assert_eq!(
        name_value(cx),
        before,
        "the key changed nothing under the dialog"
    );
    assert!(templates_open(cx), "and the dialog stays");
    keys(cx, "escape");
    assert!(!templates_open(cx), "the premise: Escape closed Templates");
    number_key(cx, 2);
    assert_eq!(
        name_value(cx),
        "applied by shortcut",
        "with the dialog gone it applies"
    );
}

/// The other side of the rule above, the one the dispatcher's docs state: a list that never takes focus, as the
/// launcher's Host list, does not stop a shortcut, since the keyboard is still the launcher's (the old shortcuts gave
/// way only to a modal dialog). Specified: with key 2 on `renamer`, Clone of `a` with its Host list open, key 2 applies
/// `renamer` to Clone's draft and sends nothing.
#[gpui::test]
fn a_number_key_applies_while_the_host_list_is_open(cx: &mut TestAppContext) {
    let (mut helm, cx) = fleet_window(cx, &[("2", "renamer")]);
    assert!(
        cx.update(|window, cx| open_clone("a", window, cx)),
        "the premise: Clone opened"
    );
    settle(cx);
    assert!(launches(&mut helm, cx).is_empty(), "opening sends nothing");
    assert_ne!(name_value(cx), "applied by shortcut", "the premise");
    test_support::click(cx, "launcher-host");
    assert!(
        cx.debug_bounds("launcher-host-list").is_some(),
        "the premise: the Host list is open"
    );
    number_key(cx, 2);
    assert_eq!(name_value(cx), "applied by shortcut", "the key applied");
    assert!(launches(&mut helm, cx).is_empty(), "nothing was sent");
}

// ---- Restart with --------------------------------------------------------------------------------------------------

/// The main view with Restart with of `a` open (checked as opened, sending nothing, on a's stored high effort, and with
/// focus on Cancel, where Restart with starts it: the focus the first test presses its shortcuts from, a button rather
/// than a text field, as in the old cases; the second test moves focus at once), `a` listed idle on host9 so a restart
/// asks no consent, and in a plain folder rather than the helm fixture's managed checkout so a template can repeat it;
/// `templates`, ⚡ on `marked` and number keys `bound` as [`window_over`] takes them.
fn restart_window<'a>(
    cx: &'a mut TestAppContext,
    templates: &[LaunchTemplate],
    marked: &str,
    bound: &[(&str, &str)],
) -> (FakeHelm, &'a mut VisualTestContext) {
    let a = session("a")
        .host(9)
        .tabs(1)
        .unmanaged()
        .not_launched_as_checkout()
        .status(json!({"state": "idle"}));
    let (mut helm, cx) = window_over(cx, a, templates, Some(marked), bound);
    assert!(
        cx.update(|window, cx| open_restart_with("a", window, cx)),
        "the premise: Restart with opened"
    );
    settle(cx);
    assert!(launches(&mut helm, cx).is_empty(), "opening sends nothing");
    assert_eq!(
        draft(cx).agent.effort,
        Some(LaunchEffort::High),
        "the premise"
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("launcher-cancel"),
        "the premise: Restart with starts on Cancel"
    );
    (helm, cx)
}

/// Press Restart with's main action, check that it sent nothing, and return the refusal it shows beside the action. (The
/// dispatcher tests' `refusal` reads the same element but is private to them.)
fn refused_press(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Option<String> {
    test_support::click(cx, "launcher-launch");
    assert!(launches(helm, cx).is_empty(), "the press sent nothing");
    label(cx, "launcher-refusal")
}

/// SPEC.md, "Restart with": templates and template shortcuts "may repeat a held value as a no-op; an actual change is
/// refused without applying any edits", and SPEC.md's sequence rule: in the launcher "each completed sequence is one
/// refusal unit". Applying the allowed part of a refused sequence, or a ⚡ template that switches the launch kind, would
/// leave a restart the user never asked for. Specified: in Restart with of `a` (Codex, effort high), the sequence 1 2
/// (`allowed-low` setting effort low, then `held-name` renaming the session) is refused naming the session name, the
/// effort still high; the ⚡ chord on `held-bolt` (a command launch with effort low) is refused naming the launch kind;
/// the main action is then still refused as unchanged, and nothing is created or restarted.
///
/// Not checked here, as the old case did: the sequence pill reading "apply" and clearing (`super::armed_tests` cover the
/// pill), and the held agent type, host and folder still shown with their editors hidden (the Restart with view tests,
/// `crate::launcher::view_tests::restart_with_starts_on_cancel_and_says_restart_error`); the effort kept high stands in
/// for "nothing applied".
#[gpui::test]
fn restart_with_refuses_a_held_change_from_a_sequence_or_the_chord_whole(cx: &mut TestAppContext) {
    let templates = [
        template(
            "allowed-low",
            TemplateFields {
                effort: Some(Some(LaunchEffort::Low.into())),
                ..TemplateFields::default()
            },
        ),
        template(
            "held-name",
            TemplateFields {
                name: Some("a-other".into()),
                ..TemplateFields::default()
            },
        ),
        template(
            "held-bolt",
            TemplateFields {
                kind: Some(LauncherKind::Command),
                effort: Some(Some(LaunchEffort::Low.into())),
                ..TemplateFields::default()
            },
        ),
    ];
    let bound = [("1", "allowed-low"), ("2", "held-name")];
    let (mut helm, cx) = restart_window(cx, &templates, "held-bolt", &bound);

    keys(cx, &format!("{SEQUENCE_PREFIX_CHORD} 1 2 enter"));
    let shown = note(cx).expect("the sequence refused");
    assert!(
        shown.contains("its session name cannot change here"),
        "{shown:?}"
    );
    assert_eq!(
        draft(cx).agent.effort,
        Some(LaunchEffort::High),
        "allowed-low, queued before the refused template, did not apply"
    );

    chord(cx);
    let shown = note(cx).expect("held-bolt refused");
    assert!(
        shown.contains("its launch kind cannot change here"),
        "{shown:?}"
    );
    assert_eq!(draft(cx).agent.effort, Some(LaunchEffort::High));
    assert_eq!(
        refused_press(&mut helm, cx).as_deref(),
        Some("Restart error. Change a setting before restarting")
    );
}

/// SPEC.md, "Restart with": "A template containing only equal held values applies silently as nothing", its main action
/// is "inactive until a setting changes", and a shortcut applying to the launcher leaves the launch to the user. Equal
/// held values that were refused would make every saved context template useless here, and a shortcut that restarted
/// would take the restart, which stops the agent, out of the user's hands. Specified: in Restart with of `a`,
/// `tl:matching-context` and Enter in the search box (`matching-context`: a's host by its install, its folder with a
/// trailing slash and its title as a fixed name) say nothing, neither above the box nor as a shortcut's note, and leave
/// the action refused as unchanged; a template row is not one of the search results SPEC.md refuses for naming a held
/// field even at its current value. The ⚡ chord on `matching-bolt` (an agent launch of Codex with effort low) then sets
/// low, marked as changed from high, with nothing said or sent; choosing High again leaves the action refused as
/// unchanged; the sequence 1 2 (`matching-bolt`, then `matching-context` by its number key) sets low again, sending
/// nothing; a press then sends the one restart, with effort low, which the old case left unpressed: it shows the
/// "nothing restarted" checks can see a restart.
///
/// Not checked here, as the old case did: the action available right after the chord (the changed marker stands in,
/// and the final press shows it after the sequence), the sequence pill (`super::armed_tests`), the held context
/// (`crate::launcher::view_tests::restart_with_starts_on_cancel_and_says_restart_error`), and Cancel refocused before
/// the chord and the sequence. Here the refused press just before each clicks the main action, which takes focus where
/// a click focuses a button (`crate::focus::CLICK_FOCUSES_CONTROLS`: Linux, where this suite runs), so there the chord
/// and the sequence, its closing Enter included, are pressed from that button; on macOS focus stays in the search box
/// after `tl:`.
#[gpui::test]
fn restart_with_applies_matching_shortcuts_without_restarting(cx: &mut TestAppContext) {
    let templates = [
        template(
            "matching-bolt",
            TemplateFields {
                kind: Some(LauncherKind::Agent),
                agent: Some(LaunchHarness::Codex),
                effort: Some(Some(LaunchEffort::Low.into())),
                ..TemplateFields::default()
            },
        ),
        template(
            "matching-context",
            TemplateFields {
                host: Some("identity9".into()),
                destination: Some(TemplateDestination::Folder("/home/user/src/a/".into())),
                name: Some("a".into()),
                name_kind: Some(TemplateNameKind::Fixed),
                ..TemplateFields::default()
            },
        ),
    ];
    let bound = [("1", "matching-bolt"), ("2", "matching-context")];
    let (mut helm, cx) = restart_window(cx, &templates, "matching-bolt", &bound);

    test_support::click(cx, "launcher-search-row");
    assert!(search_focused(cx), "the premise: the search box has focus");
    cx.simulate_input("tl:matching-context");
    settle(cx);
    assert_eq!(
        label(cx, "launcher-search-option-0").as_deref(),
        Some("Template: matching-context"),
        "the premise: the template is offered"
    );
    keys(cx, "enter");
    assert!(
        launches(&mut helm, cx).is_empty(),
        "taking the row sent nothing"
    );
    assert_eq!(search_text(cx), "", "the row was taken");
    assert_eq!(
        (note(cx), label(cx, "launcher-search-refusal")),
        (None, None),
        "the held values applied as nothing"
    );
    assert_eq!(
        refused_press(&mut helm, cx).as_deref(),
        Some("Restart error. Change a setting before restarting")
    );

    chord(cx);
    assert_eq!(note(cx), None);
    assert_eq!(draft(cx).agent.effort, Some(LaunchEffort::Low));
    assert_eq!(
        label(cx, "launcher-changed-effort").as_deref(),
        Some("● Changed · was high")
    );
    assert!(
        launches(&mut helm, cx).is_empty(),
        "the chord restarted nothing"
    );

    test_support::click(cx, "launcher-effort-High");
    assert_eq!(
        draft(cx).agent.effort,
        Some(LaunchEffort::High),
        "the premise"
    );
    assert_eq!(
        refused_press(&mut helm, cx).as_deref(),
        Some("Restart error. Change a setting before restarting"),
        "back at the stored launch"
    );
    keys(cx, &format!("{SEQUENCE_PREFIX_CHORD} 1 2 enter"));
    assert_eq!(note(cx), None);
    assert_eq!(draft(cx).agent.effort, Some(LaunchEffort::Low));
    assert!(
        launches(&mut helm, cx).is_empty(),
        "the sequence restarted nothing"
    );

    test_support::click(cx, "launcher-launch");
    let sent = launches(&mut helm, cx);
    let [restart] = &sent[..] else {
        panic!("the press sent one restart, not {}", sent.len());
    };
    assert_eq!(restart.path, "/api/sessions/a/restart");
    let body = restart.body.as_ref().expect("a restart carries a body");
    assert_eq!(body["with"]["effort"], json!("low"), "{body}");
}

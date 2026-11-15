//! Tests of template shortcuts in the launchers that are not New's prefill (Clone and Replace with) and of the sequence's
//! Tab completion in the launcher's form, in the real main view over the helm client's fake. Ported from the old
//! `launcher-template-shortcuts.spec.ts` cases "a number shortcut applies to Clone's draft", "Replace with refuses a
//! whole sequence and yields to another modal" and "typed launcher Tab completion is immediate and preserves spaced
//! names" at reference commit `24bc0580`.
//!
//! NOTE: Not tested here: the "yields to another modal" half of the Replace with case, a template shortcut doing
//! nothing while a dialog sits over the launcher. The page's half is
//! `super::dispatch_tests::the_chord_and_number_keys_do_nothing_under_another_dialog`.
//!
//! The Clone and Replace with tests run over one session, `a`, on host9 as its recorded install `identity9`, with hosts
//! 1, 5, 9 and 11 registered, and these templates: `renamer` sets only the session name, and `elsewhere` names host1's
//! install `identity1`. The launcher's catalog read fails, as the Replace with tests answer it, since an unread catalog
//! refuses no choice and `a` carries the helm fixture's Codex launch, whose stored model an empty catalog would refuse.
//! The Tab test uses the dispatcher tests' window and templates (`super::dispatch_tests`).

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::launcher::TemplateFields;
use farhelm_proto::ui_channel::{Availability, Preferences};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::dialog_tests::{template, templates_state};
use super::dispatch_tests::{self, note, number_key, text};
use crate::actions::SEQUENCE_PREFIX_CHORD;
use crate::launcher::draft::Mode;
use crate::launcher::entry::{open_clone, open_replace_with};
use crate::launcher::view_tests::{draft, fixture_hosts, search_focused, search_text};
use crate::main_view::MainView;
use crate::session_list::tests::{listing, preferences_state, session};
use crate::test_support::{self, any_open, keys, settle, started};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The real main view over the fake helm with session `a` open (the module docs' fleet), `renamer` and `elsewhere`
/// stored, and number keys `bound` (each a digit and the template name it holds).
fn fleet_window<'a>(
    cx: &'a mut TestAppContext,
    bound: &[(&str, &str)],
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut helm, _connection) = started(cx);
    let mut sessions = listing(&["a"], &[session("a").host(9).tabs(1)]);
    sessions.entries[0].value["host_identity"] = json!("identity9");
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
    helm.connect(vec![
        sessions,
        fixture_hosts(&[0, 4, 8, 10]),
        preferences_state(Preferences {
            template_shortcuts: bound
                .iter()
                .map(|(key, name)| ((*key).to_owned(), (*name).to_owned()))
                .collect(),
            // A helm that has shown the Welcome card, which would otherwise open over the main view (`crate::welcome`).
            welcome_seen: Some(true),
            ..Preferences::default()
        }),
        templates_state(&templates, Availability::Available),
    ]);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, MainView::new);
    window.update(|_, cx| crate::local_state::select("a", cx));
    settle(window);
    (helm, window)
}

/// The creates (`POST /api/sessions`) and Replaces (`POST /api/sessions/{id}/replace`) sent since the last call, every
/// other command answered at once and the catalog read refused (the module docs say why), round after round until a
/// round sends nothing else, so a press waiting for the catalog has applied by the time this returns.
fn launches(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    let mut launches = Vec::new();
    loop {
        cx.run_until_parked();
        let mut answered = false;
        for command in helm.commands() {
            if command.path == "/api/sessions" || command.path.ends_with("/replace") {
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

//! Tests of the Keyboard shortcuts overview: the table on each platform and its template rows as pure functions, the
//! drift check that ties every chord the table shows to the keymap, and the dialog in the real main view over the helm
//! client's fake (opening from a focused terminal and from the `?` menu, the refusals, the rows drawn from pushed
//! preferences, and where focus goes when it closes).
//!
//! The table tests run both platforms' tables wherever the suite runs, since a table is a pure function of the
//! platform. The drift test reads the keymap the bootstrap installed, which is this platform's; for the other
//! platform's chords it goes through the terminal key table, which spells both platforms' forms of every app shortcut
//! side by side, and through the physical-key table, which does too. The dialog tests press the chord in this
//! platform's form; no test depends on glyph widths, so the simulated platform's fake text system is enough.

use std::collections::BTreeMap;

use farhelm_input_mapping::Platform;
use farhelm_proto::ui_channel::{
    CollectionChanges, CollectionState, PREFERENCES_KEY, Preferences, SessionRow,
};
use gpui::{
    Action, KeyContext, KeybindingKeystroke, Keystroke, Modifiers, TestAppContext,
    VisualTestContext, WeakEntity, point, px,
};

use super::{Chord, KeyboardShortcuts, Keys, Name, Row, sections, wording};
use crate::actions::{
    self, BeginSequence, CONTEXT, DecreaseTerminalTextSize, DeleteSession,
    IncreaseTerminalTextSize, KeyOwner, NewSession, NextTerminalTab, PHYSICAL_KEYS,
    PreviousTerminalTab, QuickLaunch, QuickSwitcher, RenameSession, ShowKeyboardShortcuts,
    TERMINAL_KEYS, THIS_PLATFORM, TemplateList, TemplateNumberKey,
};
use crate::layers;
use crate::session_list::tests::{listing, preferences_state, session};
use crate::templates::shortcuts::Bindings;
use crate::test_support::{
    self, MainWindow, any_open, click, focused_id, frame, keys, open_main, settle, terminal_focused,
};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// The chord on this platform (SPEC.md, "Desktop window chrome"), in GPUI's keystroke syntax. Pressed by the simulated
/// platform, it carries no native identity, so the physical-key table matches it by its US key.
const CHORD: &str = match THIS_PLATFORM {
    Platform::Mac => "cmd-/",
    Platform::Linux => "ctrl-shift-/",
};

/// The dialog frame's id, drawn only while the overview is open.
const DIALOG: &str = "keyboard-shortcuts";

/// The header's Close, where focus starts.
const CLOSE: &str = "keyboard-shortcuts-close";

/// The `?` button's id.
const HELP_BUTTON: &str = "app-help-toggle";

/// The real main view over the fake helm, shared with the other modules whose chords must open something from a
/// focused terminal; [`MainWindow::terminal_live`] makes its terminal live.
type Main<'a> = MainWindow<'a>;

/// Sessions a and b, with a the last visited, so a is open at startup, and preferences holding `preferences`'s
/// bindings. The inert `welcome_seen` value is set as `session_list::tests::preferences` sets it; nothing reads it
/// since the startup Welcome card was removed.
fn fleet(preferences: Preferences) -> Vec<CollectionState> {
    vec![
        listing(&["a", "b"], &[session("a"), session("b")]),
        preferences_state(seen(Preferences {
            visit_history: vec!["a".to_owned()],
            ..preferences
        })),
    ]
}

/// `preferences` with the Welcome card marked seen, for every preference row these tests push.
fn seen(preferences: Preferences) -> Preferences {
    Preferences {
        welcome_seen: Some(true),
        ..preferences
    }
}

/// Whether the overview is drawn.
fn open(cx: &mut VisualTestContext) -> bool {
    frame(cx)
        .iter()
        .any(|node| node.element_id.as_deref() == Some(DIALOG))
}

/// The overview opened last, as a weak handle that may since have died, so a test can tell one opening from another;
/// `None` when none was opened.
fn opened(cx: &mut VisualTestContext) -> Option<WeakEntity<KeyboardShortcuts>> {
    cx.update(|_, cx| layers::last_opened_weak::<KeyboardShortcuts>(cx))
}

/// Whether some text node of the frame reads exactly `text`.
fn shows(cx: &mut VisualTestContext, text: &str) -> bool {
    frame(cx)
        .iter()
        .any(|node| node.texts.iter().any(|shown| shown.text.as_ref() == text))
}

/// Each row of `platform`'s table as (group, what it names, its chip), with a template's name raw.
fn table(platform: Platform, bindings: &Bindings) -> Vec<(String, String, String)> {
    sections(platform, bindings)
        .into_iter()
        .flat_map(|section| {
            section.rows.into_iter().map(move |Row { name, keys }| {
                // The dialog's own wording, with the template's raw name where the dialog draws it as host text.
                let (words, template) = wording(&name);
                let name = format!("{words}{}", template.map_or("", |(_, template)| template));
                let keys = match keys {
                    Keys::Chord(chord) => chord.label(),
                    Keys::Typed(keys) => keys.to_owned(),
                };
                (section.title.to_owned(), name, keys)
            })
        })
        .collect()
}

/// `rows` as owned triples, for comparing with [`table`].
fn owned(rows: &[(&str, &str, &str)]) -> Vec<(String, String, String)> {
    rows.iter()
        .map(|(group, name, keys)| (group.to_string(), name.to_string(), keys.to_string()))
        .collect()
}

// ---- The table -----------------------------------------------------------------------------------------------------

/// SPEC.md: "Each shortcut shows only the form this client handles", "New appears only in the Mac desktop app", and the
/// groups are Sessions, Launching and Terminal with "the keys inside a quick-launch sequence". The decision that the
/// overview shows Mac or Linux forms by the client it runs in, and that Linux has no New, was the GPUI migration's
/// orchestrator's, decided on 2026-11-13. A wrong chord here teaches the user a key that does nothing. The old
/// overview's rows and spellings are the spec for the rest (`keyboard_shortcuts.rs` and its browser test at reference
/// commit `24bc0580`). Specified, with nothing marked and no number key assigned: Linux's table row for row in
/// Ctrl+Shift forms (Delete on Backspace, text size on − and +, no New), and the Mac's the same rows in Command forms
/// with New first under Sessions and Command+Shift for text size.
#[test]
fn each_platform_shows_only_its_own_forms_with_new_on_the_mac_alone() {
    let none = Bindings::default();
    let sequence = [
        (
            "Launching",
            "In a sequence: type a template name",
            "Characters (digits continue a word)",
        ),
        ("Launching", "In a sequence: queue the word", "Space"),
        ("Launching", "In a sequence: complete a unique name", "Tab"),
        ("Launching", "In a sequence: edit the word", "Backspace"),
        (
            "Launching",
            "In a sequence: add a numbered template (no word in progress)",
            "1–9, 0",
        ),
    ];
    let mut linux = owned(&[
        ("Sessions", "Quick switcher", "Ctrl+Shift+K"),
        ("Sessions", "Delete session", "Ctrl+Shift+Backspace"),
        ("Sessions", "Rename session", "Ctrl+Shift+L"),
        ("Sessions", "Keyboard shortcuts", "Ctrl+Shift+/"),
        ("Launching", "Templates", "Ctrl+Shift+J"),
        ("Launching", "Quick launch: none marked", "Ctrl+Shift+P"),
        ("Launching", "Launch a numbered template", "Ctrl+Shift+0–9"),
        ("Launching", "Begin launch sequence", "Ctrl+Shift+X"),
    ]);
    linux.extend(owned(&sequence));
    linux.extend(owned(&[
        (
            "Launching",
            "In a sequence: add a numbered template after any unfinished word",
            "Ctrl+Shift+0–9",
        ),
        (
            "Launching",
            "In a sequence: add the ⚡ template",
            "Ctrl+Shift+P",
        ),
        ("Launching", "In a sequence: launch", "Enter"),
        ("Launching", "In a sequence: cancel", "Escape"),
        ("Terminal", "Smaller terminal text", "Ctrl+Shift+−"),
        ("Terminal", "Larger terminal text", "Ctrl+Shift++"),
        ("Terminal", "Previous terminal tab", "Ctrl+Shift+["),
        ("Terminal", "Next terminal tab", "Ctrl+Shift+]"),
    ]));
    assert_eq!(table(Platform::Linux, &none), linux);

    let mut mac = owned(&[
        ("Sessions", "New session", "⌘N"),
        ("Sessions", "Quick switcher", "⌘K"),
        ("Sessions", "Delete session", "⌘W"),
        ("Sessions", "Rename session", "⌘L"),
        ("Sessions", "Keyboard shortcuts", "⌘/"),
        ("Launching", "Templates", "⌘J"),
        ("Launching", "Quick launch: none marked", "⌘P"),
        ("Launching", "Launch a numbered template", "⌘0–9"),
        ("Launching", "Begin launch sequence", "⌘X"),
    ]);
    mac.extend(owned(&sequence));
    mac.extend(owned(&[
        (
            "Launching",
            "In a sequence: add a numbered template after any unfinished word",
            "⌘0–9",
        ),
        ("Launching", "In a sequence: add the ⚡ template", "⌘P"),
        ("Launching", "In a sequence: launch", "Enter"),
        ("Launching", "In a sequence: cancel", "Escape"),
        ("Terminal", "Smaller terminal text", "⌘+Shift+−"),
        ("Terminal", "Larger terminal text", "⌘+Shift++"),
        ("Terminal", "Previous terminal tab", "⌘+Shift+["),
        ("Terminal", "Next terminal tab", "⌘+Shift+]"),
    ]));
    assert_eq!(table(Platform::Mac, &none), mac);
}

/// SPEC.md: the overview "names the current ⚡ template, or says none is marked, and lists assigned number keys with
/// their template names". The rows are how a user finds out what their keys start, so a missing or misordered key is a
/// key they cannot find. Specified: with ⚡ on `deploy` and keys 0, 1 and 5 assigned, the ⚡ row names `deploy`, and
/// the generic number row is followed by one row per assigned key in the keys' order (1, 5, then 0), each with its own
/// chord in the platform's form, before Begin launch sequence.
#[test]
fn the_template_rows_name_the_marked_template_and_each_assigned_key_in_order() {
    let bindings = Bindings {
        marked: Some("deploy".to_owned()),
        keys: BTreeMap::from([
            ("0".to_owned(), "zed".to_owned()),
            ("1".to_owned(), "api".to_owned()),
            ("5".to_owned(), "web".to_owned()),
        ]),
    };
    for (platform, modifier) in [(Platform::Linux, "Ctrl+Shift+"), (Platform::Mac, "⌘")] {
        let launching: Vec<(String, String)> = table(platform, &bindings)
            .into_iter()
            .filter(|(group, ..)| group == "Launching")
            .take(7)
            .map(|(_, name, keys)| (name, keys))
            .collect();
        let expected: Vec<(String, String)> = [
            ("Templates", "J"),
            ("Quick launch: ⚡ deploy", "P"),
            ("Launch a numbered template", "0–9"),
            ("1 · api", "1"),
            ("5 · web", "5"),
            ("0 · zed", "0"),
            ("Begin launch sequence", "X"),
        ]
        .into_iter()
        .map(|(name, key)| (name.to_owned(), format!("{modifier}{key}")))
        .collect();
        assert_eq!(launching, expected, "{platform:?}");
    }
}

// ---- The drift check -----------------------------------------------------------------------------------------------

/// What a row's chord must run, as the drift test knows it.
enum Runs {
    /// This action, through a binding in [`CONTEXT`] or a row of [`PHYSICAL_KEYS`].
    Action(Box<dyn Action>),
    /// Every template number key, one row of [`PHYSICAL_KEYS`] per digit (the generic "0–9" row).
    NumberKeys,
}

/// What the row naming `name` runs. Panics on a row this test has no answer for, so a new row cannot slip past it.
fn runs(name: &Name) -> Runs {
    let action = |action: &dyn Action| Runs::Action(action.boxed_clone());
    match name {
        Name::Words("New session") => action(&NewSession),
        Name::Words("Quick switcher") => action(&QuickSwitcher),
        Name::Words("Delete session") => action(&DeleteSession),
        Name::Words("Rename session") => action(&RenameSession),
        Name::Words("Keyboard shortcuts") => action(&ShowKeyboardShortcuts),
        Name::Words("Begin launch sequence") => action(&BeginSequence),
        Name::Words("Smaller terminal text") => action(&DecreaseTerminalTextSize),
        Name::Words("Larger terminal text") => action(&IncreaseTerminalTextSize),
        Name::Words("Previous terminal tab") => action(&PreviousTerminalTab),
        Name::Words("Next terminal tab") => action(&NextTerminalTab),
        // The armed sequence recognizes the same physical number chords as direct launch.
        Name::Words(
            "Launch a numbered template"
            | "In a sequence: add a numbered template after any unfinished word",
        ) => Runs::NumberKeys,
        Name::Words("Templates") => action(&TemplateList),
        // The ⚡ chord inside an armed sequence is the same chord: the sequence's key rules read it from the constants
        // the binding is built from.
        Name::QuickLaunch(_) | Name::Words("In a sequence: add the ⚡ template") => {
            action(&QuickLaunch)
        }
        Name::NumberKey { digit, .. } => action(&TemplateNumberKey {
            digit: digit.parse().unwrap(),
        }),
        other => panic!("the drift test does not know what {other:?} runs; add it to `runs`"),
    }
}

/// `chord` in GPUI's keystroke syntax: its modifiers, and its key as a US layout names it unshifted (`=` for the chip's
/// `+`, `-` for its `−`).
fn keystroke(chord: Chord) -> Keystroke {
    let key = match chord.key {
        "+" => "=".to_owned(),
        "−" => "-".to_owned(),
        other => other.to_lowercase(),
    };
    let modifiers = [
        (chord.modifiers.command, "cmd-"),
        (chord.modifiers.control, "ctrl-"),
        (chord.modifiers.shift, "shift-"),
    ];
    let prefix: String = modifiers
        .into_iter()
        .filter_map(|(held, name)| held.then_some(name))
        .collect();
    // `Keystroke::parse` keeps its source for `'static`; a test can afford to leak a few short strings.
    Keystroke::parse(Box::leak(format!("{prefix}{key}").into_boxed_str())).unwrap()
}

/// Whether `typed` is the chord `bound` names, the way GPUI matches a binding.
fn same(typed: &Keystroke, bound: &'static str) -> bool {
    typed.should_match(&KeybindingKeystroke::from_keystroke(
        Keystroke::parse(bound).unwrap(),
    ))
}

/// The name of the action the installed keymap runs for `typed` in Farhelm's root context, if any.
fn bound(typed: &Keystroke, cx: &gpui::App) -> Option<&'static str> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let context = [KeyContext::parse(CONTEXT).unwrap()];
    let (bindings, _) = keymap.bindings_for_input(std::slice::from_ref(typed), &context);
    bindings.first().map(|binding| binding.action().name())
}

/// The app shortcut row of [`TERMINAL_KEYS`] whose chord on `platform` is `typed`.
fn app_row(platform: Platform, typed: &Keystroke) -> Option<&'static actions::TerminalKey> {
    TERMINAL_KEYS.iter().find(|row| {
        row.owner == KeyOwner::App && row.chord(platform).is_some_and(|chord| same(typed, chord))
    })
}

/// Check that `chord`, shown on `platform`, runs what `runs` says, failing with `what` (the row) on a mismatch.
fn check_chord(platform: Platform, chord: Chord, runs: Runs, what: &str, cx: &gpui::App) {
    match runs {
        Runs::Action(action) => {
            let typed = keystroke(chord);
            if let Some(row) = PHYSICAL_KEYS
                .iter()
                .find(|row| row.action.partial_eq(action.as_ref()))
            {
                assert_eq!(
                    (row.key.us, row.modifiers(platform)),
                    (typed.key.as_str(), chord.modifiers),
                    "{what} on {platform:?}: the physical-key row of {} differs",
                    action.name()
                );
            } else if platform == THIS_PLATFORM {
                assert_eq!(
                    bound(&typed, cx),
                    Some(action.name()),
                    "{what} on {platform:?}: the keymap does not bind its chord to {}",
                    action.name()
                );
            } else {
                // The other platform's keymap is not installed here, so its chord is tied to the action through the
                // terminal table's row, which spells both platforms' forms of the same app shortcut.
                let row = app_row(platform, &typed).unwrap_or_else(|| {
                    panic!("{what} on {platform:?}: no app shortcut row has this chord")
                });
                match row.chord(THIS_PLATFORM) {
                    Some(here) => assert_eq!(
                        bound(&Keystroke::parse(here).unwrap(), cx),
                        Some(action.name()),
                        "{what} on {platform:?}: its row's chord here is not bound to {}",
                        action.name()
                    ),
                    // A Mac-only shortcut (New), whose binding the keymap builds through a function the tests can call.
                    None => {
                        let binding = actions::new_session_binding();
                        assert_eq!(binding.action().name(), action.name(), "{what}");
                        assert!(
                            binding
                                .keystrokes()
                                .first()
                                .is_some_and(|bound| typed.should_match(bound)),
                            "{what} on {platform:?}: New's binding has another chord"
                        );
                    }
                }
            }
        }
        Runs::NumberKeys => {
            let digits: Vec<_> = PHYSICAL_KEYS
                .iter()
                .filter(|row| {
                    row.action
                        .as_any()
                        .downcast_ref::<TemplateNumberKey>()
                        .is_some()
                })
                .collect();
            assert_eq!(digits.len(), 10, "ten number keys");
            for row in digits {
                assert_eq!(
                    row.modifiers(platform),
                    chord.modifiers,
                    "{what} on {platform:?}: number key {} differs",
                    row.key.us
                );
            }
        }
    }
}

/// The table is a static copy of the chords, ported from the old app rather than read from the keymap (the keymap's
/// tables are rules other features are judged by), so nothing but this test stops the two drifting apart: a chord
/// changed in the keymap would keep being advertised here, teaching a key that does nothing. Specified, on both
/// platforms, with ⚡ marked and all ten number keys assigned so every row is drawn: every chord the table shows runs
/// the action its row names. A shortcut matched by position agrees with its physical-key row (key and exact
/// modifiers). Any other shortcut is bound to its action by this platform's installed keymap in Farhelm's context, and,
/// for the other platform, its chord is that action's app-shortcut row in the terminal key table (New, the Mac's alone,
/// through its binding function). Quick launch is checked as bound to its action alone and inside a sequence.
/// The menu hints on this platform must spell the same chords as the dialog rows.
#[gpui::test]
fn every_chord_the_table_shows_runs_the_action_its_row_names(cx: &mut TestAppContext) {
    test_support::install(cx);
    let bindings = Bindings {
        marked: Some("deploy".to_owned()),
        keys: crate::templates::shortcuts::KEYS
            .into_iter()
            .map(|digit| (digit.to_owned(), format!("t{digit}")))
            .collect(),
    };
    cx.update(|cx| {
        for platform in [Platform::Mac, Platform::Linux] {
            let mut chords = 0;
            for section in sections(platform, &bindings) {
                for row in section.rows {
                    if let Keys::Chord(chord) = row.keys {
                        chords += 1;
                        let what = format!("{:?}", row.name);
                        check_chord(platform, chord, runs(&row.name), &what, cx);
                        if platform == THIS_PLATFORM {
                            let hint = match row.name {
                                Name::Words("Delete session") => {
                                    Some(actions::DELETE_SESSION_SHORTCUT_NAME)
                                }
                                Name::Words("Rename session") => {
                                    Some(actions::RENAME_SESSION_SHORTCUT_NAME)
                                }
                                Name::Words("Keyboard shortcuts") => {
                                    Some(actions::KEYBOARD_SHORTCUTS_SHORTCUT_NAME)
                                }
                                _ => None,
                            };
                            if let Some(hint) = hint {
                                assert_eq!(hint, chord.label(), "{what}'s menu hint");
                            }
                        }
                    }
                }
            }
            assert!(
                chords > 20,
                "premise: every row was drawn ({chords} chords)"
            );
        }
    });
}

// ---- Opening from the chord -----------------------------------------------------------------------------------------

/// SPEC.md: the chord opens Keyboard shortcuts "including from a focused terminal", "The chord never reaches the
/// terminal's program", and closing "restores the opener's focus, including the terminal". The terminal is where focus
/// usually is when the user reaches for the chord, and a slash leaking into a shell types into the user's command line.
/// Specified: with a's terminal live and focused, the chord opens the overview with focus on its Close, and the program
/// receives nothing; Escape closes it and focus is home again, on the terminal.
#[gpui::test]
fn the_chord_from_a_terminal_opens_the_overview_and_escape_returns_home(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet(Preferences::default()));
    // Asserts the premise itself: a's terminal is live and focused, and its input count starts here.
    main.terminal_live();
    keys(main.cx, CHORD);
    assert!(open(main.cx), "the chord opened the overview");
    assert_eq!(focused_id(main.cx).as_deref(), Some(CLOSE));
    assert_eq!(
        main.program_input(),
        0,
        "the chord never reached the program"
    );
    keys(main.cx, "escape");
    assert!(!any_open(main.cx), "Escape closed it");
    assert!(terminal_focused(main.cx), "focus is back on the terminal");
}

/// SPEC.md: the chord "does nothing while another modal is open", which in this app means any dialog or menu holding
/// the keyboard, as for the quick switcher's chord; this dialog itself counts, so a second press cannot stack a second
/// overview. Opening over a menu would leave two layers fighting over the keyboard. Specified: with b's row menu open,
/// the chord leaves the menu the only layer and opens no overview; with the overview open, the chord opens no second
/// one and leaves the first open.
#[gpui::test]
fn the_chord_does_nothing_over_a_menu_or_over_the_overview_itself(cx: &mut TestAppContext) {
    let main = open_main(cx, fleet(Preferences::default()));
    click(main.cx, "session-menu-b");
    assert!(any_open(main.cx), "the premise: b's menu is open");
    keys(main.cx, CHORD);
    assert!(any_open(main.cx), "the menu is still open");
    assert!(!open(main.cx), "no overview opened over the menu");
    assert!(opened(main.cx).is_none(), "and none was made");
    keys(main.cx, "escape");
    assert!(!any_open(main.cx), "the premise: Escape closed the menu");

    keys(main.cx, CHORD);
    let first = opened(main.cx).expect("the chord opened the overview");
    keys(main.cx, CHORD);
    let last = opened(main.cx).expect("an overview was recorded");
    assert_eq!(
        first.entity_id(),
        last.entity_id(),
        "the second press opened no second overview"
    );
    assert!(open(main.cx), "the first is still open");
}

/// SPEC.md: the chord does nothing "before preferences finish loading"; here that is a snapshot without the preferences
/// entry, the same stand-in the quick switcher's chord uses. The overview names bindings it would not have yet.
/// Specified: with the session list arrived but no preferences entry in the store, the chord opens nothing. Help
/// still offers the dialog, and omits the chip until the chord can open it too.
#[gpui::test]
fn the_chord_does_nothing_before_the_preferences_arrive(cx: &mut TestAppContext) {
    let main = open_main(cx, vec![listing(&["a"], &[session("a")])]);
    let (listed, preferences) = main.cx.update(|_, cx| {
        let store =
            crate::session::current_store(cx).expect("the premise: the window is connected");
        let collections = store.read(cx).collections();
        (
            collections.get::<SessionRow>("a").is_some(),
            collections.get::<Preferences>(PREFERENCES_KEY).is_some(),
        )
    });
    assert!(
        listed && !preferences,
        "the premise: the session list arrived and the preferences did not"
    );
    keys(main.cx, CHORD);
    assert!(!any_open(main.cx), "nothing opens before the preferences");

    click(main.cx, HELP_BUTTON);
    let texts = crate::session_list::host_rail::tests::texts_within(
        &frame(main.cx),
        "help-menu-keyboard-shortcuts",
    );
    assert!(texts.iter().any(|text| text.text == "Keyboard shortcuts"));
    assert!(
        !texts
            .iter()
            .any(|text| text.text == actions::KEYBOARD_SHORTCUTS_SHORTCUT_NAME)
    );
    click(main.cx, "help-menu-keyboard-shortcuts");
    assert!(open(main.cx), "the menu item can open the dialog");
}

// ---- The rows from pushed preferences -------------------------------------------------------------------------------

/// Push `preferences` (with the Welcome card marked seen and a still the last visited) as the helm's new preference
/// row, and let the window take it.
fn push_preferences(main: &mut Main<'_>, preferences: Preferences) {
    let state = preferences_state(seen(Preferences {
        visit_history: vec!["a".to_owned()],
        ..preferences
    }));
    main.helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: vec![],
    });
    settle(main.cx);
}

/// SPEC.md: the overview "names the current ⚡ template, or says none is marked, and lists assigned number keys with
/// their template names", and refreshes them each time it opens, so a mark an agent or the command line moved shows at
/// the next opening; this dialog reads them from the store as it draws, so a change pushed while it is open shows at
/// once too, as the module docs promise. The names are the helm's raw values, which an agent may write, so they are
/// drawn as host text: a hidden character shows escaped. Specified: with ⚡ on a name holding U+202E and key 1 on
/// `api`, the ⚡ row reads "Quick launch: ⚡ dep<U+202E>loy" and the key's "1 · api"; a change pushed while the dialog
/// is open, ⚡ on `next` and key 2 on `web` alone, shows in it as "Quick launch: ⚡ next" and "2 · web", with no row for
/// key 1; after closing, a change that clears the mark and moves the key to 0 on `zed` shows at the next opening as
/// "Quick launch: none marked" and "0 · zed", with no row for key 2.
#[gpui::test]
fn the_template_rows_follow_the_pushed_preferences_while_open_and_at_each_opening(
    cx: &mut TestAppContext,
) {
    let marked = Preferences {
        quick_launch_template: Some("dep\u{202E}loy".to_owned()),
        template_shortcuts: BTreeMap::from([("1".to_owned(), "api".to_owned())]),
        ..Preferences::default()
    };
    let mut main = open_main(cx, fleet(marked));
    keys(main.cx, CHORD);
    let label = |cx: &mut VisualTestContext, id| test_support::label(cx, id);
    assert_eq!(
        label(main.cx, "keyboard-shortcuts-quick-launch").as_deref(),
        Some("Quick launch: ⚡ dep<U+202E>loy")
    );
    assert_eq!(
        label(main.cx, "keyboard-shortcuts-number-1").as_deref(),
        Some("1 · api")
    );

    push_preferences(
        &mut main,
        Preferences {
            quick_launch_template: Some("next".to_owned()),
            template_shortcuts: BTreeMap::from([("2".to_owned(), "web".to_owned())]),
            ..Preferences::default()
        },
    );
    assert!(open(main.cx), "the premise: the overview is still open");
    assert_eq!(
        label(main.cx, "keyboard-shortcuts-quick-launch").as_deref(),
        Some("Quick launch: ⚡ next")
    );
    assert_eq!(
        label(main.cx, "keyboard-shortcuts-number-2").as_deref(),
        Some("2 · web")
    );
    assert_eq!(label(main.cx, "keyboard-shortcuts-number-1"), None);
    keys(main.cx, "escape");
    assert!(
        !any_open(main.cx),
        "the premise: Escape closed the overview"
    );

    push_preferences(
        &mut main,
        Preferences {
            template_shortcuts: BTreeMap::from([("0".to_owned(), "zed".to_owned())]),
            ..Preferences::default()
        },
    );
    keys(main.cx, CHORD);
    assert!(open(main.cx), "the premise: the overview opened again");
    assert!(
        shows(main.cx, "Quick launch: none marked"),
        "the cleared mark shows"
    );
    assert_eq!(label(main.cx, "keyboard-shortcuts-quick-launch"), None);
    assert_eq!(label(main.cx, "keyboard-shortcuts-number-2"), None);
    assert_eq!(
        label(main.cx, "keyboard-shortcuts-number-0").as_deref(),
        Some("0 · zed")
    );
}

// ---- Opening from the `?` menu ---------------------------------------------------------------------------------------

/// The Help menu advertises the chord that opens the same dialog. Check the chip before choosing the item and
/// check that focus starts on Close in the overview.
fn open_from_menu(cx: &mut VisualTestContext) {
    click(cx, HELP_BUTTON);
    let texts = crate::session_list::host_rail::tests::texts_within(
        &frame(cx),
        "help-menu-keyboard-shortcuts",
    );
    assert!(
        texts
            .iter()
            .any(|text| text.text == actions::KEYBOARD_SHORTCUTS_SHORTCUT_NAME)
    );
    click(cx, "help-menu-keyboard-shortcuts");
    assert!(open(cx), "the item opened the overview");
    assert!(
        !frame(cx)
            .iter()
            .any(|node| node.element_id.as_deref() == Some("help-menu")),
        "the menu closed"
    );
    assert_eq!(focused_id(cx).as_deref(), Some(CLOSE));
}

/// SPEC.md: "The same dialog is also available from the **?** menu", and "Escape, Close or a click outside closes the
/// ordinary modal and restores the opener's focus, including ... the **?** button". This runs under the macOS click
/// rule, where the click that opened the menu left the button unfocused. NOTE: The menu's own close puts focus on the
/// `?` button before the overview opens, so what held focus at the opening is that button too; this test cannot tell
/// the dialog's named return target (`ReturnTo::Control`) from its ordinary one, only that focus ends on the button.
/// The old dialog's body was a stop of its own named "Keyboard shortcuts" within the same Tab loop as Close, so a long
/// table can be scrolled by keyboard. Specified: opened from the item, the overview starts on Close; Tab reaches the
/// body's stop, named "Keyboard shortcuts", and Tab again returns to Close; then, opened afresh each time, a click on
/// Close, Escape and a press on the veil outside each close it with focus on the `?` button.
#[gpui::test]
fn the_menu_item_opens_the_overview_and_every_close_returns_to_the_help_button(
    cx: &mut TestAppContext,
) {
    let main = open_main(cx, fleet(Preferences::default()));
    main.cx
        .update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::check(main.cx);
    assert_ne!(
        focused_id(main.cx).as_deref(),
        Some(HELP_BUTTON),
        "premise: focus is away from the ? button, so landing on it is the menu's and the overview's doing"
    );
    open_from_menu(main.cx);
    assert_eq!(
        test_support::label(main.cx, "keyboard-shortcuts-body-stop").as_deref(),
        Some("Keyboard shortcuts")
    );
    keys(main.cx, "tab");
    assert_eq!(
        focused_id(main.cx).as_deref(),
        Some("keyboard-shortcuts-body-stop")
    );
    keys(main.cx, "tab");
    assert_eq!(focused_id(main.cx).as_deref(), Some(CLOSE));

    for how in ["close", "escape", "outside"] {
        if how != "close" {
            open_from_menu(main.cx);
        }
        match how {
            "close" => click(main.cx, CLOSE),
            "escape" => keys(main.cx, "escape"),
            _ => {
                main.cx
                    .simulate_click(point(px(4.), px(4.)), Modifiers::none());
                settle(main.cx);
            }
        }
        assert!(!any_open(main.cx), "{how} closed the overview");
        assert_eq!(focused_id(main.cx).as_deref(), Some(HELP_BUTTON), "{how}");
    }
}

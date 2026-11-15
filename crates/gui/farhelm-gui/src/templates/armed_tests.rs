//! Tests of the armed quick-launch sequence: how a key is sorted (pure, on both platforms, ported from the old
//! classifier's oracle `js-tests/template-shortcuts.test.js` at reference commit `24bc0580`), and everything else in the
//! real main view over the helm client's fake: arming, what the capture takes and lets through, each cancel, the pill's
//! content and place, Enter and the reset.
//!
//! The window tests press keys the way the platform delivers them (`test_support::keys`, or a raw key-down for a held
//! repeat). The open session's terminal is a fake stream the test brings live, so "never reaches the program" is read
//! off the input requests that stream received. The pill is found by its `sequence-pill` selector and read from the
//! frame's painted text. Every step ends in one of the shared harness's step helpers, so the focus and layer invariants
//! are checked after each.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::{FakeTerminal, FakeTerminalRequest};
use farhelm_helm_client::terminal::TerminalAttach;
use farhelm_input_mapping::Platform;
use farhelm_proto::grid::GridOutput;
use farhelm_proto::launcher::{LaunchTemplate, TemplateFields};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, KeyedEntry,
    Preferences,
};
use gpui::{
    AppContext as _, InteractiveElement as _, IntoElement, KeyDownEvent, Keystroke, Modifiers,
    NativeKey, NativeKeyIdentity, ParentElement as _, Styled as _, TestAppContext,
    VisualTestContext, div, px,
};
use serde_json::json;

use super::armed::{Key, PillView, armed_sequence, classify, is_armed};
use super::sequence::{Entry, Input, Sequence};
use super::shortcuts::Bindings;
use crate::actions::{self, BeginSequence, SEQUENCE_PREFIX_CHORD};
use crate::components::{ENTRANCE, HoverHelp, TextInput, TextInputState};
use crate::layers::{self, Anchor, Pill, PillPlacement};
use crate::main_view::MainView;
use crate::session_list::tests::{listing, painted, preferences_state, session};
use crate::test_support::{self, advance, bounds, frame, keys, meta, settle, started};
use crate::ticker;

// ---- Sorting keys ----------------------------------------------------------------------------------------------------

/// `chord` as the simulated platform delivers it: no native identity, and the character it types where it types one.
fn typed(chord: &'static str) -> KeyDownEvent {
    KeyDownEvent {
        native_key: None,
        keystroke: Keystroke::parse(chord).unwrap().with_simulated_ime(),
        is_held: false,
        prefer_character_input: false,
    }
}

/// A key-down with a native identity, typing `key_char` (or nothing) with no modifiers, as a real keyboard sends it.
fn native(identity: NativeKeyIdentity, key: &str, key_char: Option<&str>) -> KeyDownEvent {
    KeyDownEvent {
        native_key: Some(NativeKey {
            identity,
            shift: false,
        }),
        keystroke: Keystroke {
            modifiers: Modifiers::none(),
            key: key.into(),
            key_char: key_char.map(str::to_owned),
        },
        is_held: false,
        prefer_character_input: false,
    }
}

/// The app chords on `platform`: its number-key modifiers with 3, its ⚡ chord and its prefix.
fn chords(platform: Platform) -> [&'static str; 3] {
    match platform {
        Platform::Mac => ["cmd-3", "cmd-p", "cmd-x"],
        Platform::Linux => ["ctrl-shift-3", "ctrl-shift-p", "ctrl-shift-x"],
    }
}

/// The old classifier's oracle is the specification of which keys belong to a sequence, and a key sorted wrong either
/// leaks into the terminal's program or silently drops typing. Specified, on both platforms: the number chord queues its
/// digit and the ⚡ chord the mark, by the platform's modifiers; the prefix is the prefix; a bare digit is a number key
/// carrying the character its layout prints (`é` on the key at 1's position); letters, Shift capitals and an astral
/// character are characters; Space with or without Shift, plain Tab and plain Backspace are the model's; plain Enter
/// resolves; Escape with any modifiers cancels.
#[test]
fn sequence_keys_are_sorted_as_the_old_classifier_did() {
    for platform in [Platform::Mac, Platform::Linux] {
        let [number, mark, prefix] = chords(platform);
        let sorted = |event: &KeyDownEvent| classify(platform, event);
        assert_eq!(
            sorted(&typed(number)),
            Key::Input(Input::NumberChord("3".into()))
        );
        assert_eq!(sorted(&typed(mark)), Key::Input(Input::MarkChord));
        assert_eq!(sorted(&typed(prefix)), Key::Prefix);
        let digit = Input::Digit {
            key: "3".into(),
            text: '3',
        };
        assert_eq!(sorted(&typed("3")), Key::Input(digit));
        let french_one = match platform {
            Platform::Mac => NativeKeyIdentity::Mac(0x12),
            Platform::Linux => NativeKeyIdentity::Xkb("AE01".into()),
        };
        let layout_digit = Input::Digit {
            key: "1".into(),
            text: 'é',
        };
        assert_eq!(
            sorted(&native(french_one, "é", Some("é"))),
            Key::Input(layout_digit),
            "{platform:?}: the key at 1's position is number key 1, typing what it prints"
        );
        for (chord, c) in [("a", 'a'), ("shift-a", 'A'), ("p", 'p')] {
            assert_eq!(
                sorted(&typed(chord)),
                Key::Input(Input::Character(c)),
                "{chord}"
            );
        }
        let crab = native(NativeKeyIdentity::Xkb("AD07".into()), "u", Some("🦀"));
        assert_eq!(sorted(&crab), Key::Input(Input::Character('🦀')));
        for space in ["space", "shift-space"] {
            assert_eq!(sorted(&typed(space)), Key::Input(Input::Space), "{space}");
        }
        assert_eq!(sorted(&typed("tab")), Key::Tab);
        assert_eq!(sorted(&typed("backspace")), Key::Input(Input::Backspace));
        assert_eq!(sorted(&typed("enter")), Key::Enter);
        for escape in ["escape", "shift-escape", "ctrl-escape"] {
            assert_eq!(sorted(&typed(escape)), Key::Escape, "{escape}");
        }
    }
}

/// Every key the sequence does not take must cancel it and reach its normal target, so it must sort as `Other`: an
/// over-eager rule would eat a chord or an arrow the user meant for the terminal. Specified, from the oracle, on both
/// platforms: Shift+Tab, Shift+Backspace, Shift+Enter and Ctrl+Enter, arrows and function keys, letters with Ctrl or
/// Alt, the number and ⚡ chords with a modifier too many, a key that types nothing (a dead key) or a control
/// character, and a keypad digit, which never starts a word.
#[test]
fn other_keys_sort_as_cancelling_and_passing_on() {
    for platform in [Platform::Mac, Platform::Linux] {
        let sorted = |event: &KeyDownEvent| classify(platform, event);
        let extra = match platform {
            Platform::Mac => ["cmd-shift-3", "cmd-shift-p", "cmd-alt-3"],
            Platform::Linux => ["ctrl-shift-alt-3", "ctrl-shift-alt-p", "ctrl-3"],
        };
        for chord in [
            "shift-tab",
            "shift-backspace",
            "shift-enter",
            "ctrl-enter",
            "down",
            "left",
            "f5",
            "ctrl-a",
            "alt-a",
        ]
        .into_iter()
        .chain(extra)
        {
            assert_eq!(sorted(&typed(chord)), Key::Other, "{platform:?}: {chord}");
        }
        let dead = native(
            NativeKeyIdentity::Xkb("AD11".into()),
            "dead_circumflex",
            None,
        );
        assert_eq!(sorted(&dead), Key::Other, "a dead key types nothing yet");
        let control = native(NativeKeyIdentity::Xkb("AD07".into()), "u", Some("\u{1}"));
        assert_eq!(sorted(&control), Key::Other, "a control character");
        let keypad = match platform {
            Platform::Mac => NativeKeyIdentity::Mac(0x55),
            Platform::Linux => NativeKeyIdentity::Xkb("KP3".into()),
        };
        assert_eq!(
            sorted(&native(keypad, "3", Some("3"))),
            Key::Other,
            "{platform:?}: a keypad digit"
        );
    }
}

// ---- The window ------------------------------------------------------------------------------------------------------

/// The templates the tests start with, in the helm's order: a ⚡ template, a number key's, and names a word can match.
const TEMPLATES: [&str; 9] = [
    "my-codex",
    "other-web",
    "web",
    "web-a\u{200b}",
    "web-b",
    "web-c",
    "web-d",
    "web-e",
    "webbuilder",
];

/// The templates collection holding `names`, in that order, available or not.
fn templates(names: &[&str], availability: Availability) -> CollectionState {
    CollectionState {
        collection: Collection::Templates,
        meta: CollectionMeta {
            availability,
            ..meta(names)
        },
        entries: names
            .iter()
            .map(|name| KeyedEntry {
                key: (*name).into(),
                value: serde_json::to_value(LaunchTemplate {
                    name: (*name).into(),
                    fields: TemplateFields::default(),
                })
                .unwrap(),
            })
            .collect(),
    }
}

/// The preferences collection: session a visited, `webbuilder` holding ⚡, and number key 1 bound to `one`.
fn bindings(one: &str) -> CollectionState {
    preferences_state(Preferences {
        visit_history: vec!["a".into()],
        quick_launch_template: Some("webbuilder".into()),
        template_shortcuts: BTreeMap::from([("1".to_owned(), one.to_owned())]),
        // A helm that has shown the Welcome card, which would otherwise open over the main view (`crate::welcome`).
        welcome_seen: Some(true),
        ..Preferences::default()
    })
}

/// `state` as a change the helm pushes.
fn change(state: CollectionState) -> CollectionChanges {
    CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    }
}

/// The real main view over the fake helm, and the fake terminals its pane opened.
struct Main<'a> {
    cx: &'a mut VisualTestContext,
    helm: FakeHelm,
    terminals: Rc<RefCell<Vec<FakeTerminal>>>,
}

/// The main view connected with session a open (or no sessions at all), [`TEMPLATES`] and [`bindings`], its pane's
/// terminal live and focused when there is one.
fn open_main(cx: &mut TestAppContext, with_session: bool) -> Main<'_> {
    let (mut helm, connection) = started(cx);
    let terminals: Rc<RefCell<Vec<FakeTerminal>>> = Rc::default();
    let record = terminals.clone();
    cx.update(|cx| {
        connection.update(cx, |connection, _| {
            connection.serve_terminals(move |attach: TerminalAttach| {
                let (terminal, handle, events) = FakeTerminal::open(&attach);
                record.borrow_mut().push(terminal);
                (handle, events)
            })
        })
    });
    let sessions = if with_session {
        listing(&["a"], &[session("a")])
    } else {
        listing(&[], &[])
    };
    helm.connect(vec![
        sessions,
        bindings("my-codex"),
        templates(&TEMPLATES, Availability::Available),
    ]);
    cx.run_until_parked();
    let (_, cx) = test_support::open_view(cx, MainView::new);
    let mut main = Main {
        cx,
        helm,
        terminals,
    };
    if with_session {
        main.terminal_live();
        assert!(
            test_support::terminal_focused(main.cx),
            "the premise: a's terminal has focus"
        );
        main.program_input();
    }
    main
}

impl Main<'_> {
    /// The pane's terminal attaches and replays a complete screen, so it goes live and takes input.
    fn terminal_live(&mut self) {
        {
            let mut terminals = self.terminals.borrow_mut();
            let terminal = &mut terminals[0];
            let asked = terminal.attach();
            let screen = [
                json!({"t":"hello","cols":asked.cols,"rows":asked.rows,"history_cap":0}),
                json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
                    "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
                json!({"t":"frame","rev":1,"cols":asked.cols,"rows":asked.rows,
                    "cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":false}}),
            ];
            for message in screen {
                terminal.output(serde_json::from_value::<GridOutput>(message).unwrap());
            }
            terminal.replay_complete();
        }
        settle(self.cx);
    }

    /// How many input requests the terminal's program was sent since the last call.
    fn program_input(&mut self) -> usize {
        self.terminals.borrow_mut()[0]
            .received()
            .into_iter()
            .filter(|request| matches!(request, FakeTerminalRequest::Input { .. }))
            .count()
    }

    /// Press the prefix, which must arm a sequence.
    fn arm(&mut self) {
        keys(self.cx, SEQUENCE_PREFIX_CHORD);
        assert!(self.armed(), "the premise: the prefix armed a sequence");
    }

    fn armed(&mut self) -> bool {
        self.cx.update(|_, cx| is_armed(cx))
    }

    /// The armed sequence's queue and unfinished word.
    fn state(&mut self) -> (Vec<Entry>, String) {
        let sequence = self.cx.update(|_, cx| armed_sequence(cx));
        let sequence = sequence.expect("the premise: a sequence is armed");
        (sequence.entries().to_vec(), sequence.word().to_owned())
    }

    /// Every line of text the frame painted, joined with single spaces.
    fn text(&mut self) -> String {
        painted(&frame(self.cx)).join(" ")
    }

    fn pill_drawn(&mut self) -> bool {
        self.cx.debug_bounds("sequence-pill").is_some()
    }

    /// Send a key-down for `chord` as the platform reports an auto-repeat of a key held down.
    fn held(&mut self, chord: &'static str) {
        let mut event = typed(chord);
        event.is_held = true;
        self.cx.simulate_event(event);
        settle(self.cx);
    }
}

/// Assert the sequence is over: nothing armed, no capture left in the slot, and no pill drawn.
fn assert_ended(main: &mut Main<'_>, why: &str) {
    assert!(!main.armed(), "{why}: no sequence is armed");
    assert!(
        !main.cx.update(|_, cx| actions::key_capture_armed(cx)),
        "{why}: the capture is disarmed"
    );
    assert!(!main.pill_drawn(), "{why}: the pill is gone");
}

/// SPEC.md: the prefix works from a focused terminal, which "counts as the terminal, not as a text field", and while
/// armed "typed characters never reach the terminal's program". A sequence key that leaked would type into the agent.
/// Specified: from the open session's terminal the prefix shows the pill and sends nothing; letters, a bare digit,
/// Space and Backspace then build the sequence (a digit with no word queues its key, Space queues the word) and none
/// reaches the program.
#[gpui::test]
fn the_prefix_from_a_terminal_arms_a_sequence_that_takes_every_key(cx: &mut TestAppContext) {
    let mut main = open_main(cx, true);
    main.arm();
    assert!(main.text().contains("Launch sequence:"), "the pill shows");
    keys(main.cx, "1 w e b x backspace space shift-c");
    assert_eq!(
        main.state(),
        (
            vec![Entry::Number("1".into()), Entry::Word("web".into())],
            "C".into()
        )
    );
    assert_eq!(main.program_input(), 0, "nothing reached the program");
    assert!(
        test_support::terminal_focused(main.cx),
        "the terminal kept focus"
    );
}

/// With no session open the pill sits over the main area, and the prefix must work from there too (SPEC.md: "the main
/// area with no session open"). Specified: over an empty fleet the prefix arms a sequence whose pill is centered on
/// the main pane, within a pixel.
#[gpui::test]
fn the_prefix_with_no_session_open_centers_the_pill_on_the_main_area(cx: &mut TestAppContext) {
    let mut main = open_main(cx, false);
    main.arm();
    let (pill, pane) = (
        bounds(main.cx, "sequence-capsule"),
        bounds(main.cx, "main-pane"),
    );
    let (at, want) = (pill.center(), pane.center());
    assert!(
        (at.x - want.x).abs() <= px(1.) && (at.y - want.y).abs() <= px(1.),
        "the pill at {at:?} is centered on the main pane at {want:?}"
    );
}

/// SPEC.md: "Text fields outside the session launcher keep their normal Cut or text-direction key". On Linux the prefix
/// is a chord GPUI Kit's field binds to nothing, so without the text field's own row it would arm a sequence while the
/// user is typing in a filter. Specified: with a Kit field focused, the prefix runs no `BeginSequence` and leaves the
/// field's text alone; on a button beside it, the prefix runs it.
#[gpui::test]
fn a_text_field_keeps_the_prefix(cx: &mut TestAppContext) {
    let begun = Rc::new(Cell::new(0));
    let field: Rc<RefCell<Option<TextInputState>>> = Rc::new(RefCell::new(None));
    let button = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (counted, kept, focus) = (begun.clone(), field.clone(), button.clone());
    let cx = test_support::open(cx, move |window, cx| {
        let state = kept
            .borrow_mut()
            .get_or_insert_with(|| TextInputState::new(window, cx))
            .clone();
        let counted = counted.clone();
        div()
            .flex()
            .gap(px(8.))
            .on_action(move |_: &BeginSequence, _, _| counted.set(counted.get() + 1))
            .child(
                div()
                    .debug_selector(|| "field-box".into())
                    .w(px(120.))
                    .child(TextInput::new(
                        "field",
                        &state,
                        "Filter",
                        HoverHelp::tooltip("A test field"),
                    )),
            )
            .child(test_support::button("control", &focus))
            .into_any_element()
    });
    let state = field.borrow().clone().expect("the field was drawn");
    test_support::click(cx, "field-box");
    keys(cx, "a b");
    keys(cx, SEQUENCE_PREFIX_CHORD);
    assert_eq!(begun.get(), 0, "the prefix in the field began nothing");
    assert_eq!(cx.update(|_, cx| state.text(cx)).as_ref(), "ab");

    keys(cx, "tab");
    assert!(
        test_support::focused(cx, &button),
        "the premise: the button has focus"
    );
    keys(cx, SEQUENCE_PREFIX_CHORD);
    assert_eq!(begun.get(), 1, "outside a text field the prefix begins");
}

/// SPEC.md: "Escape cancels and is consumed". Escape reaching the terminal would send ESC to the agent, which in some
/// harnesses interrupts its turn. Specified: Escape ends the sequence (no capture, no pill) and the program is sent
/// nothing; the next key goes to the program as usual.
#[gpui::test]
fn escape_cancels_and_never_reaches_the_program(cx: &mut TestAppContext) {
    let mut main = open_main(cx, true);
    main.arm();
    keys(main.cx, "w escape");
    assert_ended(&mut main, "after Escape");
    assert_eq!(main.program_input(), 0, "neither key reached the program");
    keys(main.cx, "q");
    assert_eq!(main.program_input(), 1, "typing reaches the program again");
}

/// Deliver the next display frame, as the platform's frame source would, let the redraw it causes settle, and return
/// how many frame requests were waiting for it: what a running fade asks for to draw again (the same reading the
/// entrance's own tests use; the test platform has no frame source of its own).
fn next_frame(cx: &mut VisualTestContext) -> usize {
    let waiting = cx.update(|window, cx| window.simulate_next_frame(cx));
    settle(cx);
    waiting
}

/// SPEC.md: the pill comes in with "a short fade ... switched off under reduced motion, never looping". The entrance is
/// keyed so each armed sequence fades once: a fade that restarted on every typed key would flicker, and one that never
/// ran again would leave the second sequence without its cue. Showing the pill asks for one ordinary redraw either way,
/// so a running fade is told apart by asking again on the frame after it. Specified: just after arming, a second frame
/// is still asked for (the fade runs); once the fade is over, a typed key asks for no frame beyond its own redraw (no
/// restart); a new sequence after a cancel fades again; with motion reduced, nothing is asked for after the first frame.
#[gpui::test]
fn the_pill_fades_in_once_per_sequence(cx: &mut TestAppContext) {
    let mut main = open_main(cx, true);
    main.arm();
    next_frame(main.cx);
    assert!(
        next_frame(main.cx) > 0,
        "arming starts the fade, which keeps asking"
    );
    advance(main.cx, ENTRANCE * 2);
    next_frame(main.cx);
    assert_eq!(next_frame(main.cx), 0, "the premise: the fade is over");
    keys(main.cx, "w");
    assert_eq!(
        main.state().1,
        "w",
        "the premise: the key joined the armed sequence"
    );
    next_frame(main.cx);
    assert_eq!(
        next_frame(main.cx),
        0,
        "a typed key does not start the fade again"
    );
    keys(main.cx, "escape");
    assert_ended(&mut main, "after Escape");
    main.arm();
    next_frame(main.cx);
    assert!(next_frame(main.cx) > 0, "a new sequence fades in again");
    keys(main.cx, "escape");

    main.cx.update(|_, cx| ticker::set_reduced_motion(true, cx));
    main.arm();
    next_frame(main.cx);
    assert_eq!(
        next_frame(main.cx),
        0,
        "with motion reduced the pill appears at once"
    );
}

/// SPEC.md: "Non-sequence keys, including arrows, other chords and Shift+Enter, cancel and reach their normal target".
/// A cancelling key that was swallowed would lose the user's keystroke. Specified: an arrow ends the sequence and is
/// sent to the program, once; so is Shift+Enter on a new sequence.
#[gpui::test]
fn a_non_sequence_key_cancels_and_reaches_the_program(cx: &mut TestAppContext) {
    let mut main = open_main(cx, true);
    for key in ["down", "shift-enter"] {
        main.arm();
        keys(main.cx, key);
        assert_ended(&mut main, key);
        assert_eq!(main.program_input(), 1, "{key} reached the program once");
    }
}

/// SPEC.md: "Meta, Control and Shift on their own do not cancel", and "Repeated held keys add nothing". The user holds
/// Shift for a capital or Ctrl+Shift for a number chord, so a lone modifier must not end the sequence; and a held digit
/// that queued its template on every repeat would launch it many times. Specified: modifier changes alone keep the
/// sequence armed and unchanged; a digit held for two repeats queues once; a held prefix (the arming press's own
/// repeats) changes nothing; a fresh prefix ends the sequence without reaching the program.
#[gpui::test]
fn modifiers_alone_and_held_repeats_change_nothing(cx: &mut TestAppContext) {
    let mut main = open_main(cx, true);
    main.arm();
    for modifiers in [
        Modifiers::shift(),
        Modifiers::control(),
        Modifiers::command(),
    ] {
        main.cx.simulate_modifiers_change(modifiers);
        settle(main.cx);
    }
    main.cx.simulate_modifiers_change(Modifiers::none());
    settle(main.cx);
    assert!(main.armed(), "lone modifiers did not cancel");
    main.held(SEQUENCE_PREFIX_CHORD);
    keys(main.cx, "1");
    main.held("1");
    main.held("1");
    assert_eq!(
        main.state(),
        (vec![Entry::Number("1".into())], String::new())
    );
    keys(main.cx, SEQUENCE_PREFIX_CHORD);
    assert_ended(&mut main, "after a second prefix");
    assert_eq!(main.program_input(), 0, "nothing reached the program");
}

/// SPEC.md: "a click, switching windows or opening a dialog cancels". A sequence left armed after the user clicked
/// elsewhere or left the window would take the next keys they type somewhere else. Specified: a press in the window
/// ends the sequence; so does the window going inactive.
#[gpui::test]
fn a_press_or_the_window_going_inactive_cancels(cx: &mut TestAppContext) {
    let mut main = open_main(cx, true);
    main.arm();
    // On the terminal, under the pill: the pill takes no press, so the press is the terminal's, and it cancels.
    test_support::click(main.cx, "main-pane");
    assert_ended(&mut main, "after a press");

    main.arm();
    assert!(
        main.cx.update(|window, _| window.is_window_active()),
        "premise: the window is active before it goes inactive"
    );
    main.cx.deactivate_window();
    settle(main.cx);
    assert_ended(&mut main, "after the window went inactive");
}

/// SPEC.md: other visible dialogs keep the keyboard, and opening one cancels. The launcher counts as such a dialog until
/// it can say it is open (the module docs' NOTE). Specified: with a dialog open the prefix arms nothing; a dialog that
/// opens while a sequence is armed ends it at the next key, and the sequence does not take that key.
#[gpui::test]
fn a_dialog_keeps_the_keyboard_from_the_prefix_and_an_armed_sequence(cx: &mut TestAppContext) {
    let mut main = open_main(cx, true);
    let start = test_support::handle(main.cx);
    test_support::open_dialog(main.cx, vec![("ok", start.clone())], &start);
    keys(main.cx, SEQUENCE_PREFIX_CHORD);
    assert!(!main.armed(), "the prefix armed nothing over the dialog");
    // Closed with Escape, as a user would: the focus coordinator drops a fresh press of the key that closed a dialog
    // for a moment (a repeated key must not act again), and it ties a close it cannot attribute to the last key
    // pressed, which here would be the prefix itself.
    keys(main.cx, "escape");
    settle(main.cx);
    assert!(
        !test_support::any_open(main.cx),
        "the premise: the dialog closed"
    );

    main.arm();
    let start = test_support::handle(main.cx);
    test_support::open_dialog(main.cx, vec![("ok", start.clone())], &start);
    keys(main.cx, "w");
    assert_ended(&mut main, "after a key with a dialog open");
}

/// SPEC.md: the pill shows "queued entries as chips and the word in progress", and "a list below the pill shows at most
/// five matching template names, exact matches before prefixes"; names are host text; Tab completes against the
/// current templates and does nothing while they are unavailable. Specified: number key 1, ⚡ and an unbound 9 show as
/// `my-codex`, `webbuilder` and "Unbound key 9"; the word `web` lists `web` then the first four prefix matches (one
/// escaped, `web-e` and `other-web` absent) under the pill without moving it; Tab queues `web`; with the templates
/// unavailable, Tab leaves `webb` as typed.
#[gpui::test]
fn the_pill_shows_chips_the_word_and_its_candidates(cx: &mut TestAppContext) {
    let mut main = open_main(cx, true);
    main.arm();
    let [_, mark, _] = chords(actions::THIS_PLATFORM);
    keys(main.cx, &format!("1 {mark} 9"));
    let before = bounds(main.cx, "sequence-capsule");
    keys(main.cx, "w e b");
    let text = main.text();
    for expected in [
        "my-codex",
        "webbuilder",
        "Unbound key 9",
        "web-a<U+200B>",
        "web-d",
        "Type names or number keys · Tab complete · Enter launch · Esc cancel",
    ] {
        assert!(text.contains(expected), "{expected:?} in {text:?}");
    }
    assert!(
        !text.contains("web-e") && !text.contains("other-web"),
        "{text:?}"
    );
    let (pill, names) = (
        bounds(main.cx, "sequence-capsule"),
        bounds(main.cx, "sequence-candidates"),
    );
    assert_eq!(
        pill.center().y,
        before.center().y,
        "the names did not move the pill"
    );
    assert!(
        names.top() >= pill.bottom(),
        "the names hang below the pill"
    );

    keys(main.cx, "tab");
    assert_eq!(main.state().0.last(), Some(&Entry::Word("web".into())));
    assert!(!main.text().contains("web-a<U+200B>"), "no word, no names");

    main.helm.push(change(templates(
        &TEMPLATES,
        Availability::Unavailable {
            error: "busy".into(),
        },
    )));
    settle(main.cx);
    keys(main.cx, "w e b b tab");
    assert_eq!(main.state().1, "webb", "Tab did nothing while unavailable");
}

/// SPEC.md: "Bindings refresh when the prefix starts; Enter captures the bound names currently shown in the pill". A
/// binding moved by an agent while the user types must not change what the chip says or what Enter launches. Specified:
/// number key 1 rebound after the prefix still shows the name it had when the prefix was pressed.
#[gpui::test]
fn bindings_are_copied_when_the_prefix_starts(cx: &mut TestAppContext) {
    let mut main = open_main(cx, true);
    main.arm();
    main.helm.push(change(bindings("web-b")));
    settle(main.cx);
    keys(main.cx, "1");
    let text = main.text();
    assert!(
        text.contains("my-codex") && !text.contains("web-b"),
        "{text:?}"
    );
}

/// SPEC.md: "Enter with nothing queued cancels without opening anything", and Enter ends the sequence whatever it then
/// does; what a resolved sequence launches waits for the quick-launch dispatcher. Specified: Enter on an empty sequence
/// and on a resolved one each end it, open no layer, send the helm nothing and never reach the program.
#[gpui::test]
fn enter_ends_the_sequence_and_opens_nothing_without_an_unbound_key(cx: &mut TestAppContext) {
    let mut main = open_main(cx, true);
    main.helm.commands();
    for typed in ["enter", "1 w e b enter"] {
        main.arm();
        keys(main.cx, typed);
        assert_ended(&mut main, typed);
        assert!(!test_support::any_open(main.cx), "{typed}: nothing opened");
    }
    assert!(main.helm.commands().is_empty(), "the helm was sent nothing");
    assert_eq!(main.program_input(), 0, "Enter never reached the program");
}

/// SPEC.md: "An unbound key opens Templates at Enter with the binding hint outside the launcher". Specified: Enter on a
/// sequence holding the unbound key 9 ends it and opens the Templates dialog with the number-key hint.
#[gpui::test]
fn enter_with_an_unbound_key_opens_templates_with_its_hint(cx: &mut TestAppContext) {
    let mut main = open_main(cx, true);
    main.arm();
    keys(main.cx, "9");
    assert!(
        !test_support::any_open(main.cx),
        "the key alone opens nothing"
    );
    keys(main.cx, "enter");
    assert_ended(&mut main, "after Enter");
    assert!(test_support::any_open(main.cx), "Templates opened");
    let text = main.text();
    assert!(text.contains("Give a template a number key"), "{text:?}");
}

/// SPEC.md, "Losing the connection to the helm": an armed sequence starts over after a reset. A capture that outlived
/// it would go on eating keys behind the inert window. Specified: a reset (the window displaced) ends the sequence and
/// leaves nothing in the slot.
#[gpui::test]
fn a_reset_disarms_the_sequence(cx: &mut TestAppContext) {
    let mut main = open_main(cx, true);
    main.arm();
    main.helm.displace();
    settle(main.cx);
    assert_ended(&mut main, "after the reset");
}

/// Inside the launcher the pill says "apply" and sits at the bottom of the main area so it does not cover the launcher
/// (SPEC.md). Nothing arms that form yet (the module docs' NOTE), so its view is shown directly. Specified: the launcher
/// form's pill paints "Apply sequence:" and "Enter apply", with its bottom edge 14 pixels above the anchor's.
#[gpui::test]
fn the_launcher_form_says_apply_and_sits_at_the_bottom(cx: &mut TestAppContext) {
    let out: Rc<RefCell<Option<Anchor>>> = Rc::new(RefCell::new(None));
    let kept = out.clone();
    let cx = test_support::open(cx, move |window, cx| {
        let anchor = Anchor::for_element("area-anchor", window, cx);
        kept.replace(Some(anchor.clone()));
        div()
            .debug_selector(|| "area".into())
            .relative()
            .w(px(700.))
            .h(px(300.))
            .child(anchor.track())
            .into_any_element()
    });
    let anchor = out.borrow().clone().expect("the area was drawn");
    cx.update(|window, cx| {
        let view = cx.new(|_| PillView {
            sequence: Sequence::new(Bindings::default()),
            in_launcher: true,
        });
        let pill = Pill {
            view: view.into(),
            anchor,
            placement: PillPlacement::Bottom,
        };
        layers::show_pill(pill, window, cx)
    })
    .expect("the pill shows");
    settle(cx);
    let text = painted(&frame(cx)).join(" ");
    assert!(
        text.contains("Apply sequence:") && text.contains("Enter apply"),
        "{text:?}"
    );
    let (capsule, area) = (bounds(cx, "sequence-capsule"), bounds(cx, "area"));
    assert!(
        (area.bottom() - px(14.) - capsule.bottom()).abs() <= px(1.),
        "the pill keeps 14 pixels above the area's bottom edge: {capsule:?} in {area:?}"
    );
}

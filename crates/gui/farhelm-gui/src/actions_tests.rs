//! Tests of the keymap where a terminal has focus: which keys reach the terminal, which stay Farhelm's, and that no
//! chord prefix holds a key back (SPEC_impl.md, "Actions and keymap").
//!
//! Each test opens its window through the shared harness ([`test_support::open_view`]: GPUI Kit's `Root` around a root
//! view wired through [`crate::root`], with the app's whole keymap from the bootstrap), on a scene holding a terminal
//! stand-in: an element in [`TERMINAL_CONTEXT`] whose key listener records every key that reaches it, as the
//! terminal's will. Reaching the listener is not enough to show a key passed through, because GPUI lets a key go on
//! when the binding that matched it has no handler or its handler declines (Farhelm's Dismiss with nothing open, Kit's
//! Copy with no Kit selection). So the tests also read the keymap's resolution in the focused terminal's real context
//! stack, which shows whether any binding would run at all.
//!
//! The last section tests the repeat guard: a held key's auto-repeats are key-down events with `is_held` set, sent
//! with `simulate_event`, since `simulate_keystrokes` only ever sends fresh presses.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Action as _, App, AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    KeyBinding, KeyContext, KeyDownEvent, KeyUpEvent, Keystroke, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, actions, div, point, px,
};

use crate::actions::{
    CONTEXT, DeleteSession, KeyOwner, RenameSession, TERMINAL_CONTEXT, TERMINAL_KEYS,
    TEXT_INPUT_CONTEXT, THIS_PLATFORM, owner_on, terminal_key_owner,
};
use crate::components::{Button, HoverHelp, Role, TextInput, TextInputState};
use crate::focus::{self, OnPress};
use crate::{layers, root, test_support};
use farhelm_input_mapping::Platform;

actions!(keymap_tests, [Probe]);

// ---- Harness ---------------------------------------------------------------------------------------------------------

/// The scene's root view: a button (somewhere for Tab to go) and the terminal stand-in, wired through [`root`] like the
/// main view. Its own root view rather than [`test_support::open`]'s scene, because the actions it counts are handled
/// on the root element, as Farhelm's root handles its own: with nothing else focused, focus rests on the root, and an
/// action dispatched from there reaches only handlers on the root itself.
struct Scene {
    root: root::Attachment,
    terminal: FocusHandle,
    button: FocusHandle,
    /// Every key that reached the terminal stand-in's listener, in order.
    received: Rc<RefCell<Vec<Keystroke>>>,
    /// How many times a [`Probe`] binding ran: a test binds one to an app shortcut to see it still works.
    probes: Rc<Cell<usize>>,
    /// The name of every session shortcut action that ran, in order: what the session list's handlers will see.
    ran: Rc<RefCell<Vec<&'static str>>>,
}

impl Render for Scene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let received = self.received.clone();
        let probes = self.probes.clone();
        let (deletes, renames) = (self.ran.clone(), self.ran.clone());
        let content = div()
            .size_full()
            .flex()
            .gap(px(8.))
            .p(px(20.))
            .on_action(move |_: &Probe, _, _| probes.set(probes.get() + 1))
            .on_action(move |action: &DeleteSession, _, _| deletes.borrow_mut().push(action.name()))
            .on_action(move |action: &RenameSession, _, _| renames.borrow_mut().push(action.name()))
            .child(
                Button::new(
                    "control",
                    "control",
                    HoverHelp::tooltip("A test control"),
                    Role::Button,
                )
                .focus(&self.button),
            )
            .child(focus::track(
                div()
                    .key_context(TERMINAL_CONTEXT)
                    .debug_selector(|| "terminal".into())
                    .relative()
                    .size(px(100.))
                    // A terminal consumes every key that reaches it, as the real one does for what it sends.
                    .on_key_down(move |event, _, cx| {
                        received.borrow_mut().push(event.keystroke.clone());
                        cx.stop_propagation();
                    }),
                &self.terminal,
                OnPress::Focus,
            ));
        root::wrap(content, &self.root, window, cx)
    }
}

/// What a test holds on to: the window and the scene's handles and records.
struct Opened<'a> {
    cx: &'a mut VisualTestContext,
    terminal: FocusHandle,
    received: Rc<RefCell<Vec<Keystroke>>>,
    probes: Rc<Cell<usize>>,
    ran: Rc<RefCell<Vec<&'static str>>>,
}

/// Open the scene through the shared harness. Nothing holds focus but the root until a test focuses something.
fn open(cx: &mut TestAppContext) -> Opened<'_> {
    test_support::install(cx);
    let (scene, cx) = test_support::open_view(cx, |window, cx| Scene {
        root: root::attach(window, cx),
        terminal: cx.focus_handle().tab_stop(true),
        button: cx.focus_handle().tab_stop(true),
        received: Rc::new(RefCell::new(Vec::new())),
        probes: Rc::new(Cell::new(0)),
        ran: Rc::new(RefCell::new(Vec::new())),
    });
    let (terminal, received, probes, ran) = cx.update(|_, cx| {
        let scene = scene.read(cx);
        (
            scene.terminal.clone(),
            scene.received.clone(),
            scene.probes.clone(),
            scene.ran.clone(),
        )
    });
    Opened {
        cx,
        terminal,
        received,
        probes,
        ran,
    }
}

impl Opened<'_> {
    /// Click the terminal stand-in, which focuses it on every platform (a terminal is tracked with
    /// [`OnPress::Focus`]), and assert that it took focus.
    fn focus_terminal(&mut self) {
        test_support::click(self.cx, "terminal");
        assert!(
            self.terminal_focused(),
            "the premise: the terminal stand-in holds focus"
        );
    }

    /// Whether the terminal stand-in holds focus now.
    fn terminal_focused(&mut self) -> bool {
        let terminal = self.terminal.clone();
        self.cx.update(|window, _| terminal.is_focused(window))
    }

    /// Press `chord` and return whether it reached the terminal's listener, as that same key, and nothing else did.
    fn reaches_terminal(&mut self, chord: &'static str) -> bool {
        self.received.borrow_mut().clear();
        test_support::keys(self.cx, chord);
        let expected = parse(chord);
        let received = self.received.borrow();
        received.len() == 1
            && received[0].key == expected.key
            && received[0].modifiers == expected.modifiers
    }

    /// The focused element's key context stack, outermost first.
    fn stack(&mut self) -> Vec<KeyContext> {
        self.cx.update(|window, _| window.context_stack())
    }

    /// Send a key-down for `chord`: a fresh press, or with `held` an auto-repeat of the key already held down, which is
    /// how the platform reports a repeat.
    fn key_down(&mut self, chord: &'static str, held: bool) {
        self.cx.simulate_event(KeyDownEvent {
            native_key: None,
            keystroke: parse(chord),
            is_held: held,
            prefer_character_input: false,
        });
        test_support::settle(self.cx);
    }

    /// Send the key-up that releases `chord`.
    fn key_up(&mut self, chord: &'static str) {
        self.cx.simulate_event(KeyUpEvent {
            native_key: None,
            keystroke: parse(chord),
        });
        test_support::settle(self.cx);
    }

    /// Hold `chord` down for a press and two auto-repeats, then release it.
    fn hold(&mut self, chord: &'static str) {
        self.key_down(chord, false);
        self.key_down(chord, true);
        self.key_down(chord, true);
        self.key_up(chord);
    }

    /// How many times the session shortcut action named `name` has run.
    fn runs_of(&self, name: &str) -> usize {
        self.ran.borrow().iter().filter(|ran| **ran == name).count()
    }

    /// Bind the session shortcuts in Farhelm's context on this platform's chords, as the session list will, and return
    /// each chord with the name of the action it runs.
    fn bind_session_shortcuts(&mut self) -> [(&'static str, &'static str); 2] {
        let shortcuts = session_shortcuts();
        self.cx.update(|_, cx| {
            cx.bind_keys([
                KeyBinding::new(shortcuts[0].0, DeleteSession, Some(CONTEXT)),
                KeyBinding::new(shortcuts[1].0, RenameSession, Some(CONTEXT)),
            ])
        });
        shortcuts
    }
}

/// The session Delete and Rename chords on this platform (SPEC.md: Cmd+W and Cmd+L on a Mac, Ctrl+Shift+Backspace and
/// Ctrl+Shift+L on Linux), each with the name of its action.
fn session_shortcuts() -> [(&'static str, &'static str); 2] {
    let (delete, rename) = match THIS_PLATFORM {
        Platform::Mac => ("cmd-w", "cmd-l"),
        Platform::Linux => ("ctrl-shift-backspace", "ctrl-shift-l"),
    };
    [
        (delete, DeleteSession.name()),
        (rename, RenameSession.name()),
    ]
}

/// `chord` in GPUI's keystroke syntax, as a keystroke; a chord that does not parse is a broken test or table row.
fn parse(chord: &'static str) -> Keystroke {
    match Keystroke::parse(chord) {
        Ok(keystroke) => keystroke,
        Err(error) => panic!("{chord} does not parse: {error}"),
    }
}

/// A plain key-down event for `keystroke`, with no physical key: what [`owner_on`] is asked about. Native key data
/// does not matter to the table yet, which matches only the keystroke.
fn down(keystroke: Keystroke) -> KeyDownEvent {
    KeyDownEvent {
        native_key: None,
        keystroke,
        is_held: false,
        prefer_character_input: false,
    }
}

/// What in the keymap breaks the terminal's rules in context `stack`: a binding's first key that GPUI would hold back
/// as a chord prefix there, and a key the program owns that some binding would take. Walks every binding's first key,
/// which covers every key any binding mentions; a key no binding mentions resolves to nothing anyway.
fn terminal_keymap_problems(stack: &[KeyContext], cx: &App) -> Vec<String> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let os = cx.platform_info().os;
    let mut problems = Vec::new();
    for binding in keymap.bindings() {
        let Some(first) = binding.keystrokes().first() else {
            continue;
        };
        let typed = first.inner().clone();
        let (resolved, pending) = keymap.bindings_for_input(std::slice::from_ref(&typed), stack);
        if pending {
            problems.push(format!(
                "{} starts a chord bound where a terminal is, so GPUI would hold it back",
                typed.unparse(os)
            ));
        }
        if owner_on(THIS_PLATFORM, &down(typed.clone())) == KeyOwner::Program
            && let Some(winner) = resolved.first()
        {
            problems.push(format!(
                "{} runs {} instead of reaching the terminal's program",
                typed.unparse(os),
                winner.action().name()
            ));
        }
    }
    problems.sort();
    problems.dedup();
    problems
}

// ---- The table -------------------------------------------------------------------------------------------------------

/// The table is read by GPUI's own matching, on both platforms: every chord parses, no two rows name the same keystroke
/// on a platform, each chord's owner is the one its row gives, and keys the table does not list are the program's. The
/// Mac's answers are checked here because the suite runs mostly on Linux. A typo in a chord would otherwise only show
/// as a key that silently went to the wrong owner, or as a panic inside the bootstrap's `KeyBinding::new`.
#[gpui::test]
fn every_chord_in_the_table_answers_with_its_own_owner_on_both_platforms(_: &mut TestAppContext) {
    for platform in [Platform::Mac, Platform::Linux] {
        let mut seen: Vec<(Keystroke, &str)> = Vec::new();
        for key in TERMINAL_KEYS {
            let Some(chord) = key.chord(platform) else {
                continue;
            };
            let keystroke = match Keystroke::parse(chord) {
                Ok(keystroke) => keystroke,
                Err(error) => panic!("{chord} ({}) does not parse: {error}", key.what),
            };
            if let Some((_, earlier)) = seen.iter().find(|(other, _)| {
                other.key == keystroke.key && other.modifiers == keystroke.modifiers
            }) {
                panic!(
                    "{chord} on {platform:?} is in two rows: {earlier} and {}",
                    key.what
                );
            }
            assert_eq!(
                owner_on(platform, &down(keystroke.clone())),
                key.owner,
                "{chord} on {platform:?} ({})",
                key.what
            );
            seen.push((keystroke, key.what));
        }
    }
    let program = |platform, chord| owner_on(platform, &down(parse(chord))) == KeyOwner::Program;
    assert!(
        program(Platform::Linux, "a"),
        "a plain letter is the program's"
    );
    assert!(program(Platform::Linux, "enter"));
    assert!(
        program(Platform::Linux, "ctrl-k"),
        "SPEC.md: plain Ctrl+K off macOS keeps the program's own behavior"
    );
    assert!(
        program(Platform::Linux, "cmd-k"),
        "Linux's quick switcher is Ctrl+Shift+K, so Super+K is the program's"
    );
    assert!(
        program(Platform::Mac, "ctrl-shift-k"),
        "the Mac's quick switcher is Cmd+K, so Ctrl+Shift+K is the program's"
    );
    assert!(
        program(Platform::Mac, "cmd-shift-k"),
        "a chord is matched exactly: an extra modifier makes it the program's"
    );
    assert_eq!(
        terminal_key_owner(&down(parse(match THIS_PLATFORM {
            Platform::Mac => "cmd-k",
            Platform::Linux => "ctrl-shift-k",
        }))),
        KeyOwner::App,
        "the public query answers for this platform"
    );
}

// ---- Keys in a focused terminal --------------------------------------------------------------------------------------

/// SPEC_impl.md requires "a test that every pass-through key reaches the terminal with the app's bindings loaded".
/// For every row of the table on this platform that is the program's or the terminal's own: no binding resolves in the
/// focused terminal (so no Farhelm or Kit action could take it, whether or not its handler would decline), the key
/// reaches the terminal's listener, and focus stays on the terminal (Tab and Option-Tab did not move it, though the
/// button beside it is a tab stop).
#[gpui::test]
fn every_key_the_terminal_or_its_program_owns_reaches_the_terminal(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    opened.focus_terminal();
    let stack = opened.stack();
    for key in TERMINAL_KEYS
        .iter()
        .filter(|key| key.owner != KeyOwner::App)
    {
        let Some(chord) = key.chord(THIS_PLATFORM) else {
            continue;
        };
        let resolved = opened.cx.update(|_, cx| {
            cx.key_bindings()
                .borrow()
                .bindings_for_input(&[parse(chord)], &stack)
                .0
                .iter()
                .map(|binding| binding.action().name())
                .collect::<Vec<_>>()
        });
        assert!(
            resolved.is_empty(),
            "{chord} ({}) resolves to {resolved:?} in a focused terminal",
            key.what
        );
        assert!(
            opened.reaches_terminal(chord),
            "{chord} ({}) reached the terminal: {:?}",
            key.what,
            opened.received.borrow()
        );
        assert!(
            opened.terminal_focused(),
            "{chord} ({}) left focus on the terminal",
            key.what
        );
    }
}

/// The app's shortcuts keep working from a focused terminal: the terminal's context binds none of them away, so a
/// feature's binding in Farhelm's context runs and the program never sees the key. No feature binds them yet, so the
/// test binds a probe to each, as a feature will.
#[gpui::test]
fn app_shortcuts_bound_in_farhelms_context_still_work_from_a_terminal(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    let chords: Vec<&'static str> = TERMINAL_KEYS
        .iter()
        .filter(|key| key.owner == KeyOwner::App)
        .filter_map(|key| key.chord(THIS_PLATFORM))
        .collect();
    assert!(
        !chords.is_empty(),
        "the premise: this platform has app shortcuts"
    );
    opened.cx.update(|_, cx| {
        cx.bind_keys(
            chords
                .iter()
                .map(|chord| KeyBinding::new(chord, Probe, Some(CONTEXT))),
        )
    });
    opened.focus_terminal();
    for chord in chords {
        let before = opened.probes.get();
        assert!(
            !opened.reaches_terminal(chord),
            "{chord} did not reach the terminal"
        );
        assert_eq!(opened.probes.get(), before + 1, "{chord} ran its binding");
    }
}

/// Farhelm binds Escape to Dismiss, which closes a tooltip when no dialog is open, so without the terminal's own
/// binding an Escape meant for the program would close a tooltip instead, and with no layer open the key would still
/// reach the terminal, hiding the mistake from a test that only checks delivery (SPEC_impl.md: "Escape from a focused terminal goes to its program"). The premise shows Escape does close
/// the tooltip when focus is not in the terminal, so the tooltip surviving is the terminal context's doing.
#[gpui::test]
fn escape_in_a_terminal_goes_to_the_program_and_leaves_a_tooltip_open(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    // The tooltip's owner token stands for a control the test does not draw; it is kept for the whole test, because
    // its release would take the tooltip down by itself.
    let owner = opened.cx.update(|_, cx| cx.new(|_| layers::TooltipOwner));
    let show = |opened: &mut Opened| {
        opened.cx.update(|window, cx| {
            layers::show_tooltip_at(&owner, "help".into(), point(px(5.), px(5.)), window, cx)
        });
        test_support::settle(opened.cx);
        opened
            .cx
            .update(|window, cx| layers::shown_tooltip(window, cx).is_some())
    };

    assert!(show(&mut opened), "the premise: the tooltip shows");
    assert!(!opened.reaches_terminal("escape"));
    let shown = opened
        .cx
        .update(|window, cx| layers::shown_tooltip(window, cx).is_some());
    assert!(
        !shown,
        "the premise: outside the terminal, Escape closes the tooltip"
    );

    opened.focus_terminal();
    assert!(show(&mut opened), "the premise: the tooltip shows again");
    assert!(
        opened.reaches_terminal("escape"),
        "Escape reached the terminal"
    );
    let shown = opened
        .cx
        .update(|window, cx| layers::shown_tooltip(window, cx).is_some());
    assert!(shown, "and the tooltip stayed open");
}

// ---- Chord prefixes --------------------------------------------------------------------------------------------------

/// The walk SPEC_impl.md asks for, over the app's whole keymap (GPUI Kit's and Farhelm's bindings) in a focused
/// terminal's real context stack: no binding's first key is held back as a chord prefix, and no key the program owns
/// resolves to a binding. A binding added later in a context that contains a terminal (Farhelm's, Kit's root, or none)
/// fails here until the table says who owns its key.
#[gpui::test]
fn the_keymap_takes_no_key_from_a_terminal_and_holds_no_chord_prefix_there(
    cx: &mut TestAppContext,
) {
    let mut opened = open(cx);
    opened.focus_terminal();
    let stack = opened.stack();
    assert!(
        stack
            .iter()
            .any(|context| context.contains(TERMINAL_CONTEXT))
            && stack.iter().any(|context| context.contains(CONTEXT)),
        "the premise: the stack is a terminal's inside Farhelm's root: {stack:?}"
    );
    let problems = opened
        .cx
        .update(|_, cx| terminal_keymap_problems(&stack, cx));
    assert!(problems.is_empty(), "{problems:#?}");
}

/// Why the rule exists, and that the walk above would catch a break of it. A chord bound in a context outside the
/// terminal does not touch it: its first key reaches the terminal at once. The same chord bound in Farhelm's context,
/// which contains the terminal, makes GPUI hold that key back until the next key shows whether the chord goes on, here
/// with no time limit, because Ctrl+K is not bound on its own and the stand-in accepts no text input. The key then
/// arrives late, together with the next one, and the walk reports the chord.
#[gpui::test]
fn a_chord_prefix_holds_keys_back_only_in_a_context_that_contains_the_terminal(
    cx: &mut TestAppContext,
) {
    let mut opened = open(cx);
    opened.focus_terminal();
    opened
        .cx
        .update(|_, cx| cx.bind_keys([KeyBinding::new("ctrl-k ctrl-s", Probe, Some("Elsewhere"))]));
    assert!(
        opened.reaches_terminal("ctrl-k"),
        "a chord bound elsewhere left Ctrl+K to the terminal"
    );

    opened
        .cx
        .update(|_, cx| cx.bind_keys([KeyBinding::new("ctrl-k ctrl-s", Probe, Some(CONTEXT))]));
    assert!(
        !opened.reaches_terminal("ctrl-k"),
        "with the chord in Farhelm's context, Ctrl+K is held back"
    );
    let pending = opened
        .cx
        .update(|window, _| window.has_pending_keystrokes());
    assert!(pending, "GPUI is waiting for the chord's second key");
    let stack = opened.stack();
    let problems = opened
        .cx
        .update(|_, cx| terminal_keymap_problems(&stack, cx));
    assert!(
        problems.iter().any(|problem| problem.contains("ctrl-k")),
        "the walk caught it: {problems:?}"
    );

    // GPUI gives up on a held chord key after a second only when that key alone is bound too or types text into an
    // element that accepts text input; Ctrl+K here is neither, so it waits for the next key with no time limit.
    test_support::advance(opened.cx, Duration::from_secs(2));
    assert!(
        opened.received.borrow().is_empty(),
        "two seconds on, Ctrl+K is still held back"
    );
    test_support::keys(opened.cx, "a");
    let received = opened.received.borrow();
    let keys: Vec<&str> = received.iter().map(|key| key.key.as_str()).collect();
    assert_eq!(
        keys,
        ["k", "a"],
        "the next key released Ctrl+K, late, and then itself"
    );
    assert_eq!(opened.probes.get(), 0, "the chord never completed");
}

// ---- Shortcuts that act once per press -------------------------------------------------------------------------------

/// SPEC.md: the session Delete and Rename shortcuts "ignore repeat". A GPUI binding runs its action on every
/// auto-repeat, so without the repeat guard holding Cmd+W (or Ctrl+Shift+Backspace) would ask to delete the session
/// once per repeat. Holding either shortcut runs its action once, for the press; a fresh press after the key is
/// released runs it again, so the guard is not a lockout that outlives the hold.
#[gpui::test]
fn a_held_session_shortcut_runs_once_and_again_after_a_release(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    let shortcuts = opened.bind_session_shortcuts();
    let stack = opened.stack();
    assert!(
        !opened.terminal_focused() && stack.iter().any(|context| context.contains(CONTEXT)),
        "the premise: focus is outside the terminal, inside Farhelm's root: {stack:?}"
    );
    for (chord, name) in shortcuts {
        opened.hold(chord);
        assert_eq!(opened.runs_of(name), 1, "holding {chord} ran {name} once");
        opened.hold(chord);
        assert_eq!(
            opened.runs_of(name),
            2,
            "a fresh press of {chord} after its release ran {name} again"
        );
    }
}

/// The same from a focused terminal, where it matters most: SPEC.md has both shortcuts work from terminal focus
/// "without sending their chord to the program". A held repeat must neither run the action again nor fall through to
/// the terminal, which would otherwise see a key no binding took and could pass it to the program.
#[gpui::test]
fn a_held_session_shortcut_in_a_terminal_runs_once_and_never_reaches_it(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    let shortcuts = opened.bind_session_shortcuts();
    opened.focus_terminal();
    for (chord, name) in shortcuts {
        opened.hold(chord);
        assert_eq!(opened.runs_of(name), 1, "holding {chord} ran {name} once");
        opened.hold(chord);
        assert_eq!(
            opened.runs_of(name),
            2,
            "a fresh press of {chord} after its release ran {name} again"
        );
    }
    assert!(
        opened.received.borrow().is_empty(),
        "no press or repeat reached the terminal: {:?}",
        opened.received.borrow()
    );
}

/// The guard is for the listed actions only. A held key bound to anything else runs its binding on every repeat, and a
/// held key no binding takes reaches the terminal on every repeat: a program reads held arrows, Backspace and letters
/// that way, and a guard that swallowed them would break typing.
#[gpui::test]
fn a_repeat_of_a_key_not_bound_to_a_listed_action_is_untouched(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    opened.bind_session_shortcuts();
    opened
        .cx
        .update(|_, cx| cx.bind_keys([KeyBinding::new("ctrl-shift-y", Probe, Some(CONTEXT))]));
    opened.focus_terminal();
    opened.hold("ctrl-shift-y");
    assert_eq!(
        opened.probes.get(),
        3,
        "an unlisted binding ran for the press and each repeat"
    );
    opened.received.borrow_mut().clear();
    opened.hold("a");
    let received: Vec<String> = opened
        .received
        .borrow()
        .iter()
        .map(|key| key.key.clone())
        .collect();
    assert_eq!(
        received,
        ["a", "a", "a"],
        "a held letter reached the terminal for the press and each repeat"
    );
}

/// The guard follows the binding that would run, not the chord. SPEC.md has the Linux Delete chord leave text editors
/// other than the terminal alone, so where a deeper context binds the same chord to something else (here the
/// terminal's context, standing in for a text field's), holding the chord repeats that binding and never runs Delete.
#[gpui::test]
fn a_listed_chord_that_a_deeper_binding_takes_keeps_its_repeats(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    let [(delete, name), _] = opened.bind_session_shortcuts();
    opened
        .cx
        .update(|_, cx| cx.bind_keys([KeyBinding::new(delete, Probe, Some(TERMINAL_CONTEXT))]));
    opened.focus_terminal();
    opened.hold(delete);
    assert_eq!(
        opened.probes.get(),
        3,
        "the deeper binding ran for the press and each repeat"
    );
    assert_eq!(opened.runs_of(name), 0, "{name} never ran");
}

/// Why this matters: SPEC.md, "Desktop window chrome": "The Linux chord leaves text editors other than the terminal
/// alone." Ctrl+Shift+Backspace deletes the open session, and Farhelm's binding of it sits above every text field, so
/// without the text field's own `NoAction` row a user deleting words in the Rename field or a filter would be asked to
/// delete their session instead. The row names GPUI Kit's input context, which Kit keeps private, so this also catches
/// a Kit upgrade renaming it.
///
/// Specification: a focused Kit text field has [`TEXT_INPUT_CONTEXT`] in its context stack; with the Delete shortcut
/// bound on the Linux chord in Farhelm's context, the chord pressed in the field runs nothing and leaves its text as it
/// was; pressed on a button, it runs Delete.
#[gpui::test]
fn the_linux_delete_chord_leaves_a_text_field_alone(cx: &mut TestAppContext) {
    the_linux_delete_chord_leaves_this_field_alone(cx, TextInputState::new);
}

/// Why this matters: the field that keeps line breaks is Kit's multi-line text area, not its single-line input, and
/// the Rename editor draws it. The `NoAction` row reaches it only because Kit's text area sets the same key context;
/// if a Kit upgrade gave it a context of its own, the Delete chord typed while renaming would delete the session.
///
/// Specification: as for the single-line field, with the field from `TextInputState::new_keeping_line_breaks`.
#[gpui::test]
fn the_linux_delete_chord_leaves_the_field_that_keeps_line_breaks_alone(cx: &mut TestAppContext) {
    the_linux_delete_chord_leaves_this_field_alone(cx, TextInputState::new_keeping_line_breaks);
}

/// The body of the two Delete chord tests above, for a field whose state `make` makes.
fn the_linux_delete_chord_leaves_this_field_alone(
    cx: &mut TestAppContext,
    make: fn(&mut Window, &mut App) -> TextInputState,
) {
    let chord = "ctrl-shift-backspace";
    let ran = Rc::new(Cell::new(0));
    let field: Rc<RefCell<Option<TextInputState>>> = Rc::new(RefCell::new(None));
    let button_focus = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (counted, kept, focus) = (ran.clone(), field.clone(), button_focus.clone());
    let cx = test_support::open(cx, move |window, cx| {
        let state = kept
            .borrow_mut()
            .get_or_insert_with(|| make(window, cx))
            .clone();
        let counted = counted.clone();
        div()
            .flex()
            .gap(px(8.))
            .on_action(move |_: &DeleteSession, _, _| counted.set(counted.get() + 1))
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
            .child(
                Button::new(
                    "control",
                    "control",
                    HoverHelp::tooltip("A test control"),
                    Role::Button,
                )
                .focus(&focus),
            )
            .into_any_element()
    });
    cx.update(|_, cx| cx.bind_keys([KeyBinding::new(chord, DeleteSession, Some(CONTEXT))]));
    let state = field.borrow().clone().expect("the field was drawn");

    test_support::click(cx, "field-box");
    test_support::keys(cx, "a b");
    let stack = cx.update(|window, _| window.context_stack());
    assert!(
        stack
            .iter()
            .any(|context| context.contains(TEXT_INPUT_CONTEXT)),
        "the premise: the focused field is in Kit's input context: {stack:?}"
    );
    test_support::keys(cx, chord);
    assert_eq!(ran.get(), 0, "the chord in the field ran nothing");
    assert_eq!(
        cx.update(|_, cx| state.text(cx)).as_ref(),
        "ab",
        "and left the field's text alone"
    );

    test_support::keys(cx, "tab");
    assert!(
        cx.update(|window, _| button_focus.is_focused(window)),
        "the premise: the button has focus"
    );
    test_support::keys(cx, chord);
    assert_eq!(ran.get(), 1, "outside a text field the chord runs Delete");
}

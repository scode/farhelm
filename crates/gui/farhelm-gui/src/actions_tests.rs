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
//! Later sections test the repeat guard and the physical-key table: a held key's auto-repeats are key-down events with
//! `is_held` set, sent with `simulate_event`, since `simulate_keystrokes` only ever sends fresh presses. The last
//! section tests the key capture slot, which takes every key ahead of all of the above; that a reset disarms it is
//! tested with the reset itself, in `session_tests.rs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Action as _, App, AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    KeyBinding, KeyContext, KeyDownEvent, KeyUpEvent, Keystroke, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, actions, div, point, px,
};

use crate::actions::{
    CONTEXT, Captured, DecreaseTerminalTextSize, DeleteSession, HideApp, HideOtherApps,
    IncreaseTerminalTextSize, KeyOwner, MAC_MENU_CHORDS, MinimizeWindow, NewSession, PHYSICAL_KEYS,
    PhysicalKey, QuickLaunch, QuickSwitcher, Quit, RenameSession, ShowKeyboardShortcuts,
    TERMINAL_CONTEXT, TERMINAL_KEYS, TEXT_INPUT_CONTEXT, THIS_PLATFORM, TemplateList,
    TemplateNumberKey, ToggleFullScreen, arm_key_capture, disarm_key_capture, key_capture_armed,
    new_session_binding, owner_on, physical_key_at, physical_on, terminal_key_owner,
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
    /// The name of every session shortcut action that ran, in order: what the session list's handlers will see. The
    /// physical-key shortcuts' actions are recorded here too.
    ran: Rc<RefCell<Vec<&'static str>>>,
    /// The digit of every [`TemplateNumberKey`] that ran, in order.
    digits: Rc<RefCell<Vec<u8>>>,
}

impl Render for Scene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let received = self.received.clone();
        let probes = self.probes.clone();
        let (deletes, renames, switchers) = (self.ran.clone(), self.ran.clone(), self.ran.clone());
        let news = self.ran.clone();
        let (quick_launches, lists) = (self.ran.clone(), self.ran.clone());
        let (shortcuts, bigger, smaller, numbers) = (
            self.ran.clone(),
            self.ran.clone(),
            self.ran.clone(),
            (self.ran.clone(), self.digits.clone()),
        );
        let content = div()
            .size_full()
            .flex()
            .gap(px(8.))
            .p(px(20.))
            .on_action(move |_: &Probe, _, _| probes.set(probes.get() + 1))
            .on_action(move |action: &DeleteSession, _, _| deletes.borrow_mut().push(action.name()))
            .on_action(move |action: &RenameSession, _, _| renames.borrow_mut().push(action.name()))
            .on_action(move |action: &QuickSwitcher, _, _| {
                switchers.borrow_mut().push(action.name())
            })
            .on_action(move |action: &NewSession, _, _| news.borrow_mut().push(action.name()))
            .on_action(move |action: &QuickLaunch, _, _| {
                quick_launches.borrow_mut().push(action.name())
            })
            .on_action(move |action: &TemplateList, _, _| lists.borrow_mut().push(action.name()))
            .on_action(move |action: &ShowKeyboardShortcuts, _, _| {
                shortcuts.borrow_mut().push(action.name())
            })
            .on_action(move |action: &IncreaseTerminalTextSize, _, _| {
                bigger.borrow_mut().push(action.name())
            })
            .on_action(move |action: &DecreaseTerminalTextSize, _, _| {
                smaller.borrow_mut().push(action.name())
            })
            .on_action(move |action: &TemplateNumberKey, _, _| {
                numbers.0.borrow_mut().push(action.name());
                numbers.1.borrow_mut().push(action.digit);
            })
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
    digits: Rc<RefCell<Vec<u8>>>,
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
        digits: Rc::new(RefCell::new(Vec::new())),
    });
    let (terminal, received, probes, ran, digits) = cx.update(|_, cx| {
        let scene = scene.read(cx);
        (
            scene.terminal.clone(),
            scene.received.clone(),
            scene.probes.clone(),
            scene.ran.clone(),
            scene.digits.clone(),
        )
    });
    Opened {
        cx,
        terminal,
        received,
        probes,
        ran,
        digits,
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
    fn bind_session_shortcuts(&mut self) -> [(&'static str, &'static str); 3] {
        let shortcuts = session_shortcuts();
        self.cx.update(|_, cx| {
            cx.bind_keys([
                KeyBinding::new(shortcuts[0].0, DeleteSession, Some(CONTEXT)),
                KeyBinding::new(shortcuts[1].0, RenameSession, Some(CONTEXT)),
                KeyBinding::new(shortcuts[2].0, QuickSwitcher, Some(CONTEXT)),
            ])
        });
        shortcuts
    }
}

/// The session Delete and Rename chords and the quick switcher's on this platform (SPEC.md: Cmd+W, Cmd+L and Cmd+K on a
/// Mac, Ctrl+Shift+Backspace, Ctrl+Shift+L and Ctrl+Shift+K on Linux), each with the name of its action. Spelled out
/// here rather than read from `actions.rs`, so a chord changed there by mistake fails these tests.
fn session_shortcuts() -> [(&'static str, &'static str); 3] {
    let (delete, rename, switcher) = match THIS_PLATFORM {
        Platform::Mac => ("cmd-w", "cmd-l", "cmd-k"),
        Platform::Linux => ("ctrl-shift-backspace", "ctrl-shift-l", "ctrl-shift-k"),
    };
    [
        (delete, DeleteSession.name()),
        (rename, RenameSession.name()),
        (switcher, QuickSwitcher.name()),
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
/// For every row of the table on this platform that is the program's or the terminal's own, except the terminal's
/// commands that have an action (the next test has those): no binding resolves in the focused terminal (so no Farhelm
/// or Kit action could take it, whether or not its handler would decline), the key reaches the terminal's listener,
/// and focus stays on the terminal (Tab and Option-Tab did not move it, though the button beside it is a tab stop).
#[gpui::test]
fn every_key_the_terminal_or_its_program_owns_reaches_the_terminal(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    opened.focus_terminal();
    let stack = opened.stack();
    for key in TERMINAL_KEYS
        .iter()
        .filter(|key| key.owner != KeyOwner::App && key.bind.is_none())
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

/// The terminal's own commands with an action (paste and the page keys) are bound where a terminal has focus, and only
/// there. A binding placed before the table's `NoAction` rows would be hidden by them, and an action on a row the
/// terminal does not own would bind a key the table gives to the app or the program, so this pins both on both
/// platforms: every row with an action ([`TerminalKey::bind`]) is a [`KeyOwner::Terminal`] row, and on this platform
/// the first binding GPUI tries for its chord in a focused terminal is its own action.
#[gpui::test]
fn the_terminals_own_actions_take_their_keys_in_a_focused_terminal(cx: &mut TestAppContext) {
    for key in TERMINAL_KEYS.iter().filter(|key| key.bind.is_some()) {
        assert_eq!(
            key.owner,
            KeyOwner::Terminal,
            "{} has an action, so it must be the terminal's own",
            key.what
        );
    }
    let mut opened = open(cx);
    opened.focus_terminal();
    let stack = opened.stack();
    let mut checked = 0;
    for key in TERMINAL_KEYS {
        let (Some(bind), Some(chord)) = (key.bind, key.chord(THIS_PLATFORM)) else {
            continue;
        };
        let action = bind(chord).action().name();
        let resolved = opened.cx.update(|_, cx| {
            cx.key_bindings()
                .borrow()
                .bindings_for_input(&[parse(chord)], &stack)
                .0
                .first()
                .map(|binding| binding.action().name())
        });
        assert_eq!(resolved, Some(action), "{chord} in a focused terminal");
        checked += 1;
    }
    assert!(
        checked > 0,
        "the premise: this platform binds a terminal action"
    );
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
    let owner = opened
        .cx
        .update(|_, cx| cx.new(|_| layers::TooltipOwner::default()));
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

/// SPEC.md: the session Delete and Rename shortcuts "ignore repeat", and the quick switcher's "does nothing" while its
/// own dialog is open. A GPUI binding runs its action on every auto-repeat, so without the repeat guard holding Cmd+W
/// (or Ctrl+Shift+Backspace) would ask to delete the session once per repeat. Holding any of the three runs its action
/// once, for the press; a fresh press after the key is released runs it again, so the guard is not a lockout that
/// outlives the hold.
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

/// The Mac app menu's chords are bound only on macOS, where these tests do not run, so their table is checked here as
/// data: each chord runs the action its menu item names (AppKit's standard chords, as the old app's menu items had
/// them), each is an app shortcut in the terminal's table on the Mac (so a focused terminal's program never receives
/// it), and none is Cmd+W, which must stay the open session's Delete (SPEC.md, "Desktop window chrome"). Spelled out
/// here rather than read from `actions.rs`, so a chord or action changed there by mistake fails.
#[gpui::test]
fn the_mac_menu_chords_run_their_actions_and_are_app_shortcuts_in_a_terminal(
    _: &mut TestAppContext,
) {
    let expected = [
        ("cmd-q", Quit.name()),
        ("cmd-h", HideApp.name()),
        ("alt-cmd-h", HideOtherApps.name()),
        ("cmd-m", MinimizeWindow.name()),
        ("ctrl-cmd-f", ToggleFullScreen.name()),
    ];
    let table: Vec<(&str, &str)> = MAC_MENU_CHORDS
        .iter()
        .map(|(chord, bind)| (*chord, bind(chord).action().name()))
        .collect();
    assert_eq!(table, expected);
    for (chord, _) in expected {
        assert_ne!(chord, "cmd-w", "Cmd+W stays the session's Delete");
        assert_eq!(
            owner_on(Platform::Mac, &down(parse(chord))),
            KeyOwner::App,
            "{chord} is an app shortcut in a focused terminal on a Mac"
        );
    }
}

/// The same from a focused terminal, where it matters most: SPEC.md has the session shortcuts work from terminal focus
/// "without sending their chord to the program", and the quick switcher open "including from a focused terminal". A
/// held repeat must neither run the action again nor fall through to the terminal, which would otherwise see a key no
/// binding took and could pass it to the program.
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
    let [(delete, name), ..] = opened.bind_session_shortcuts();
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

// ---- Shortcuts matched by the physical key ---------------------------------------------------------------------------

/// The native identity of the key a US layout prints `us` on, unshifted, on `platform`: written out from the
/// platforms' own tables (XKB position names; macOS `kVK_ANSI_*` virtual keycodes), independently of the table under
/// test, so a wrong code in a row fails here.
fn identity(platform: Platform, us: &str) -> gpui::NativeKeyIdentity {
    let (xkb, mac) = match us {
        "1" => ("AE01", 0x12),
        "2" => ("AE02", 0x13),
        "3" => ("AE03", 0x14),
        "4" => ("AE04", 0x15),
        "5" => ("AE05", 0x17),
        "6" => ("AE06", 0x16),
        "7" => ("AE07", 0x1A),
        "8" => ("AE08", 0x1C),
        "9" => ("AE09", 0x19),
        "0" => ("AE10", 0x1D),
        "-" => ("AE11", 0x1B),
        "=" => ("AE12", 0x18),
        "/" => ("AB10", 0x2C),
        other => panic!("no oracle entry for {other}"),
    };
    match platform {
        Platform::Linux => gpui::NativeKeyIdentity::Xkb(xkb.into()),
        Platform::Mac => gpui::NativeKeyIdentity::Mac(mac),
    }
}

/// What a US layout prints on the key printing `us` unshifted, with Shift held.
fn shifted(us: &str) -> &'static str {
    match us {
        "1" => "!",
        "2" => "@",
        "3" => "#",
        "4" => "$",
        "5" => "%",
        "6" => "^",
        "7" => "&",
        "8" => "*",
        "9" => "(",
        "0" => ")",
        "-" => "_",
        "=" => "+",
        "/" => "?",
        other => panic!("no shifted character for {other}"),
    }
}

/// A key-down as the platform delivers it, with a native identity: `printed` is the logical key the layout produced,
/// `modifiers` the logical keystroke's modifiers (GPUI drops Shift from a symbol there) and `shift` the original Shift.
fn native_down(
    identity: gpui::NativeKeyIdentity,
    printed: &str,
    modifiers: gpui::Modifiers,
    shift: bool,
    held: bool,
) -> KeyDownEvent {
    KeyDownEvent {
        native_key: Some(gpui::NativeKey { identity, shift }),
        keystroke: Keystroke {
            modifiers,
            key: printed.to_string(),
            key_char: None,
        },
        is_held: held,
        prefer_character_input: false,
    }
}

/// The key-down a US layout delivers for `row` on `platform`: the oracle's identity for its key, its modifiers, and
/// the symbol the layout prints with Shift held when the row holds Shift, with Shift dropped from the logical modifiers
/// as GPUI does for a symbol.
fn us_down(platform: Platform, row: &PhysicalKey, held: bool) -> KeyDownEvent {
    let chord = row.modifiers(platform);
    let printed = if chord.shift {
        shifted(row.key.us)
    } else {
        row.key.us
    };
    let modifiers = gpui::Modifiers {
        platform: chord.command,
        control: chord.control,
        ..Default::default()
    };
    native_down(
        identity(platform, row.key.us),
        printed,
        modifiers,
        chord.shift,
        held,
    )
}

/// The row whose action is `action`.
fn row_of(action: &dyn gpui::Action) -> &'static PhysicalKey {
    PHYSICAL_KEYS
        .iter()
        .find(|row| row.action.partial_eq(action))
        .expect("the table has the row")
}

/// SPEC.md matches the number keys, Keyboard shortcuts and terminal text size by the key's position. Every row's
/// identities agree with the platforms' tables, and every row answers its own chord on both platforms, from the native
/// identity a real key carries, and from the logical key alone when the event has no identity (the bridge's and the
/// tests' injected keys). The Mac's rows are checked here on Linux through the platform parameter, as the terminal key
/// table's are, so no Mac-only code is needed.
#[gpui::test]
fn every_physical_row_answers_its_own_chord_on_both_platforms(_: &mut TestAppContext) {
    for row in PHYSICAL_KEYS {
        let key = row.key.us;
        assert_eq!(
            (identity(Platform::Linux, key), identity(Platform::Mac, key)),
            (
                gpui::NativeKeyIdentity::Xkb(row.key.xkb.into()),
                gpui::NativeKeyIdentity::Mac(row.key.mac)
            ),
            "the row for {key} names the platforms' identities of that key"
        );
        for platform in [Platform::Mac, Platform::Linux] {
            let found = physical_on(platform, &us_down(platform, row, false));
            assert!(
                found.is_some_and(|found| found.action.partial_eq(row.action)),
                "{platform:?}: {key} with {:?} runs {}",
                row.modifiers(platform),
                row.action.name()
            );
            let chord = row.modifiers(platform);
            let logical = down(Keystroke {
                modifiers: gpui::Modifiers {
                    platform: chord.command,
                    control: chord.control,
                    shift: chord.shift,
                    ..Default::default()
                },
                key: key.to_string(),
                key_char: None,
            });
            assert!(
                physical_on(platform, &logical)
                    .is_some_and(|found| found.action.partial_eq(row.action)),
                "{platform:?}: without a native identity, {key} falls back to its US key"
            );
        }
    }
}

/// "Independent of the printed character": on a French layout the key at 1's position prints `&` (the Mac event
/// below) and `1` only with Shift (the Linux one), and on a German layout the key at the US slash's position prints
/// `-` (`_` with Shift). The position decides, so the first is number
/// key 1 and the second Keyboard shortcuts; read by what they print, the first would be no shortcut and the second
/// would be no shortcut either. That a present identity is never second-guessed by the printed character is what the
/// keypad cases in the next test pin.
#[gpui::test]
fn the_position_decides_whatever_the_layout_prints(_: &mut TestAppContext) {
    let command = gpui::Modifiers {
        platform: true,
        ..Default::default()
    };
    let control = gpui::Modifiers {
        control: true,
        ..Default::default()
    };
    let french_one_mac = native_down(identity(Platform::Mac, "1"), "&", command, false, false);
    let french_one_linux = native_down(identity(Platform::Linux, "1"), "1", control, true, false);
    for (platform, event) in [
        (Platform::Mac, french_one_mac),
        (Platform::Linux, french_one_linux),
    ] {
        assert!(
            physical_key_at(&event)
                .is_some_and(|row| row.action.partial_eq(&TemplateNumberKey { digit: 1 })),
            "{platform:?}: the key at 1's position is number key 1's"
        );
        assert!(
            physical_on(platform, &event)
                .is_some_and(|row| row.action.partial_eq(&TemplateNumberKey { digit: 1 })),
            "{platform:?}: the key at 1's position is number key 1 whatever it prints"
        );
    }
    let german_slash = native_down(identity(Platform::Linux, "/"), "_", control, true, false);
    assert!(
        physical_on(Platform::Linux, &german_slash)
            .is_some_and(|row| row.action.partial_eq(&ShowKeyboardShortcuts)),
        "the key at the US slash's position opens Keyboard shortcuts though it prints a minus"
    );
}

/// SPEC.md: "the numeric keypad is excluded", and Keyboard shortcuts match "with no extra modifiers". A keypad digit
/// with the number-key modifiers is not a number key, even though it prints the digit (so a fallback that ran despite
/// the identity would take it), and a row's key with one modifier too many (Option, Fn, Shift on a Mac's number key,
/// Ctrl beside Command) is not that row.
#[gpui::test]
fn keypad_digits_and_extra_modifiers_do_not_match(_: &mut TestAppContext) {
    let mods = |platform, control, alt, shift, function| gpui::Modifiers {
        platform,
        control,
        alt,
        shift,
        function,
    };
    let keypad_linux = native_down(
        gpui::NativeKeyIdentity::Xkb("KP1".into()),
        "1",
        mods(false, true, false, false, false),
        true,
        false,
    );
    let keypad_mac = native_down(
        gpui::NativeKeyIdentity::Mac(0x53),
        "1",
        mods(true, false, false, false, false),
        false,
        false,
    );
    assert!(physical_key_at(&keypad_linux).is_none() && physical_key_at(&keypad_mac).is_none());
    assert!(physical_on(Platform::Linux, &keypad_linux).is_none());
    assert!(physical_on(Platform::Mac, &keypad_mac).is_none());
    let (one_linux, one_mac) = (identity(Platform::Linux, "1"), identity(Platform::Mac, "1"));
    let slash_mac = identity(Platform::Mac, "/");
    for (platform, event) in [
        (
            Platform::Linux,
            native_down(
                one_linux.clone(),
                "!",
                mods(false, true, true, false, false),
                true,
                false,
            ),
        ),
        (
            Platform::Linux,
            native_down(
                one_linux.clone(),
                "!",
                mods(false, true, false, false, true),
                true,
                false,
            ),
        ),
        (
            Platform::Linux,
            native_down(
                one_linux,
                "1",
                mods(false, true, false, false, false),
                false,
                false,
            ),
        ),
        (
            Platform::Mac,
            native_down(
                one_mac.clone(),
                "!",
                mods(true, false, false, false, false),
                true,
                false,
            ),
        ),
        (
            Platform::Mac,
            native_down(
                one_mac,
                "1",
                mods(true, true, false, false, false),
                false,
                false,
            ),
        ),
        (
            Platform::Mac,
            native_down(
                slash_mac,
                "/",
                mods(true, false, true, false, false),
                false,
                false,
            ),
        ),
    ] {
        assert!(
            physical_on(platform, &event).is_none(),
            "{platform:?}: {event:?} matches no row"
        );
    }
}

/// The point of the table, where it matters most: with a terminal focused, each row's chord, as a real key event
/// carrying its native identity, runs the row's action and never reaches the terminal (and so never its program).
/// Before the table these chords went to the program (Ctrl+Shift+- is readline's undo on Linux, which SPEC.md lets
/// text size take over).
#[gpui::test]
fn every_physical_shortcut_runs_from_a_focused_terminal_and_never_reaches_it(
    cx: &mut TestAppContext,
) {
    let mut opened = open(cx);
    opened.focus_terminal();
    for row in PHYSICAL_KEYS {
        opened.ran.borrow_mut().clear();
        opened.digits.borrow_mut().clear();
        opened.cx.simulate_event(us_down(THIS_PLATFORM, row, false));
        test_support::settle(opened.cx);
        assert_eq!(
            *opened.ran.borrow(),
            [row.action.name()],
            "{} ran its action once",
            row.key.us
        );
        if let Some(number) = row.action.as_any().downcast_ref::<TemplateNumberKey>() {
            assert_eq!(
                *opened.digits.borrow(),
                [number.digit],
                "the number key carried its digit"
            );
        }
    }
    assert!(
        opened.received.borrow().is_empty(),
        "no physical shortcut reached the terminal: {:?}",
        opened.received.borrow()
    );
    assert!(opened.terminal_focused(), "the terminal kept focus");
}

/// SPEC.md: number keys and Keyboard shortcuts act once per held press ("held keys act once"; "repeat ... do not open
/// it"), while text size steps on every repeat of a held chord, as the old app did (its terminal script stepped with no
/// repeat check). Neither the press nor any repeat reaches the terminal.
#[gpui::test]
fn a_held_number_key_acts_once_and_a_held_text_size_chord_repeats(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    opened.focus_terminal();
    for (row, runs) in [
        (row_of(&TemplateNumberKey { digit: 4 }), 1),
        (row_of(&ShowKeyboardShortcuts), 1),
        (row_of(&IncreaseTerminalTextSize), 3),
        (row_of(&DecreaseTerminalTextSize), 3),
    ] {
        opened.ran.borrow_mut().clear();
        for held in [false, true, true] {
            opened.cx.simulate_event(us_down(THIS_PLATFORM, row, held));
            test_support::settle(opened.cx);
        }
        assert_eq!(
            opened.runs_of(row.action.name()),
            runs,
            "holding {} ran {runs} times",
            row.key.us
        );
    }
    assert!(
        opened.received.borrow().is_empty(),
        "no press or repeat reached the terminal"
    );
}

/// Keys the bridge injects and simulated test keys carry no native identity, and the table falls back to the logical
/// key for them, so an end-to-end step can press these shortcuts. Typed here as plain keystrokes, which is what the
/// bridge sends.
#[gpui::test]
fn a_shortcut_without_a_native_identity_falls_back_to_the_logical_key(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    opened.focus_terminal();
    let (number, text_size) = match THIS_PLATFORM {
        Platform::Mac => ("cmd-7", "cmd-shift-="),
        Platform::Linux => ("ctrl-shift-7", "ctrl-shift-="),
    };
    test_support::keys(opened.cx, number);
    test_support::keys(opened.cx, text_size);
    assert_eq!(*opened.digits.borrow(), [7], "{number} ran number key 7");
    assert_eq!(
        opened.runs_of(IncreaseTerminalTextSize.name()),
        1,
        "{text_size} stepped the text size"
    );
    assert!(
        opened.received.borrow().is_empty(),
        "neither reached the terminal"
    );
}

/// The table takes only its exact chords. The same keys with no modifiers, with Shift alone, or (on Linux) with Ctrl
/// alone are the program's: a shell reads digits, `!`, `=` and `/` all the time, and Ctrl+digit too.
#[gpui::test]
fn keys_at_those_positions_without_the_chord_reach_the_program(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    opened.focus_terminal();
    let control = gpui::Modifiers {
        control: true,
        ..Default::default()
    };
    let plain = gpui::Modifiers::default();
    let events = [
        native_down(identity(THIS_PLATFORM, "1"), "1", plain, false, false),
        native_down(identity(THIS_PLATFORM, "1"), "!", plain, true, false),
        native_down(identity(THIS_PLATFORM, "1"), "1", control, false, false),
        native_down(identity(THIS_PLATFORM, "="), "=", plain, false, false),
        native_down(identity(THIS_PLATFORM, "/"), "/", plain, false, false),
    ];
    for event in &events {
        opened.cx.simulate_event(event.clone());
        test_support::settle(opened.cx);
    }
    assert!(
        opened.ran.borrow().is_empty(),
        "no shortcut ran: {:?}",
        opened.ran.borrow()
    );
    let received: Vec<String> = opened
        .received
        .borrow()
        .iter()
        .map(|key| key.key.clone())
        .collect();
    assert_eq!(
        received,
        ["1", "!", "1", "=", "/"],
        "every key reached the terminal"
    );
}

/// The docs promise more than the terminal case: a row's key goes no further even where nothing handles its action
/// yet, and a focused text field never sees it. Specified: in a window with no handler for any physical-key action, with
/// a text field focused, a typed letter reaches the field (its text, and a key listener around it, see it), while a
/// number-key chord and a text-size chord reach neither, and the field keeps focus.
#[gpui::test]
fn a_physical_shortcut_with_no_handler_never_reaches_a_focused_text_field(cx: &mut TestAppContext) {
    let field: Rc<RefCell<Option<TextInputState>>> = Rc::new(RefCell::new(None));
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let (kept, heard) = (field.clone(), seen.clone());
    let cx = test_support::open(cx, move |window, cx| {
        let state = kept
            .borrow_mut()
            .get_or_insert_with(|| TextInputState::new(window, cx))
            .clone();
        let heard = heard.clone();
        div()
            .child(
                div()
                    .debug_selector(|| "field-box".into())
                    // Every key-down the field does not stop passes here on its way out: what reached the field.
                    .on_key_down(move |event, _, _| {
                        heard.borrow_mut().push(event.keystroke.key.clone())
                    })
                    .w(px(120.))
                    .child(TextInput::new(
                        "field",
                        &state,
                        "Filter",
                        HoverHelp::tooltip("A test field"),
                    )),
            )
            .into_any_element()
    });
    let state = field.borrow().clone().expect("the field was drawn");
    test_support::click(cx, "field-box");
    test_support::keys(cx, "a");
    assert_eq!(
        cx.update(|_, cx| state.text(cx)).as_ref(),
        "a",
        "the premise: the field takes typing"
    );
    assert_eq!(
        *seen.borrow(),
        ["a"],
        "the premise: the listener around the field hears its keys"
    );
    for action in [
        &TemplateNumberKey { digit: 2 } as &dyn gpui::Action,
        &IncreaseTerminalTextSize,
    ] {
        cx.simulate_event(us_down(THIS_PLATFORM, row_of(action), false));
        test_support::settle(cx);
    }
    assert_eq!(
        *seen.borrow(),
        ["a"],
        "neither chord reached the field: {:?}",
        seen.borrow()
    );
    assert_eq!(
        cx.update(|_, cx| state.text(cx)).as_ref(),
        "a",
        "and neither typed into it"
    );
    let stack = cx.update(|window, _| window.context_stack());
    assert!(
        stack
            .iter()
            .any(|context| context.contains(TEXT_INPUT_CONTEXT)),
        "the field kept focus: {stack:?}"
    );
}

// ---- The key capture slot --------------------------------------------------------------------------------------------

/// Every key-down a test capture was handed, in order: its logical key and whether it was a held repeat.
type Seen = Rc<RefCell<Vec<(String, bool)>>>;

/// The app shortcut a test binds a [`Probe`] to (the template list's chord), so a test can see whether a key reached
/// Farhelm's bindings. The keymap binds the chord to [`TemplateList`] too; the probe's binding, added after it in the
/// same context, is the one GPUI tries first.
fn probe_chord() -> &'static str {
    match THIS_PLATFORM {
        Platform::Mac => "cmd-j",
        Platform::Linux => "ctrl-shift-j",
    }
}

impl Opened<'_> {
    /// Bind a [`Probe`] to [`probe_chord`] in Farhelm's context, as a feature binds an app shortcut.
    fn bind_probe(&mut self) {
        self.cx
            .update(|_, cx| cx.bind_keys([KeyBinding::new(probe_chord(), Probe, Some(CONTEXT))]));
    }

    /// Arm the key capture slot with a handler that records each key-down it is handed and answers what `answer` says
    /// for it, and assert that the slot took it. Returns the record and a count of the times a reset told the holder.
    fn arm(
        &mut self,
        answer: impl Fn(&KeyDownEvent) -> Captured + 'static,
    ) -> (Seen, Rc<Cell<usize>>) {
        let seen = Seen::default();
        let resets = Rc::new(Cell::new(0));
        let (record, told) = (seen.clone(), resets.clone());
        let armed = self.cx.update(|_, cx| {
            arm_key_capture(
                move |event, _, _| {
                    record
                        .borrow_mut()
                        .push((event.keystroke.key.clone(), event.is_held));
                    answer(event)
                },
                move |_| told.set(told.get() + 1),
                cx,
            )
        });
        assert!(armed, "the premise: the slot was idle and armed");
        (seen, resets)
    }

    /// Whether the key capture slot is armed now.
    fn capture_armed(&mut self) -> bool {
        self.cx.update(|_, cx| key_capture_armed(cx))
    }
}

/// The keys `seen` recorded, without the held flag.
fn keys_of(seen: &Seen) -> Vec<String> {
    seen.borrow().iter().map(|(key, _)| key.clone()).collect()
}

/// SPEC.md, "Quick-launch sequences": while armed, typed characters never reach the terminal's program, and a held key
/// adds nothing, which only the handler can decide if it sees the repeats. The slot exists because no focused element
/// can take keys from a focused terminal without taking focus. Specified: with a terminal focused and the slot armed,
/// a letter, an app shortcut bound in Farhelm's context, Escape, a number-key chord and a held letter all reach the
/// handler, repeats marked held, and none reaches the terminal, the binding or the number key's action; a modifier
/// tapped alone reaches nothing (SPEC.md: Shift on its own does not cancel); focus stays put. Once disarmed, keys go
/// their normal way again, and the explicit disarm does not run the reset callback.
#[gpui::test]
fn an_armed_capture_takes_every_key_down_before_the_terminal_bindings_and_rows(
    cx: &mut TestAppContext,
) {
    let mut opened = open(cx);
    opened.bind_probe();
    opened.focus_terminal();
    let number = row_of(&TemplateNumberKey { digit: 3 });
    let number_key = us_down(THIS_PLATFORM, number, false).keystroke.key;
    let (seen, resets) = opened.arm(|_| Captured::Consumed);

    test_support::keys(opened.cx, "a");
    test_support::keys(opened.cx, probe_chord());
    test_support::keys(opened.cx, "escape");
    opened
        .cx
        .simulate_event(us_down(THIS_PLATFORM, number, false));
    test_support::settle(opened.cx);
    opened.hold("b");
    let shift = gpui::Modifiers {
        shift: true,
        ..Default::default()
    };
    opened.cx.simulate_modifiers_change(shift);
    opened.cx.simulate_modifiers_change(gpui::Modifiers::none());
    test_support::settle(opened.cx);

    let probe_key = parse(probe_chord()).key;
    let expected: Vec<(String, bool)> = [
        ("a", false),
        (probe_key.as_str(), false),
        ("escape", false),
        (number_key.as_str(), false),
        ("b", false),
        ("b", true),
        ("b", true),
    ]
    .iter()
    .map(|(key, held)| (key.to_string(), *held))
    .collect();
    assert_eq!(*seen.borrow(), expected, "the handler saw every key-down");
    assert!(
        opened.received.borrow().is_empty(),
        "nothing reached the terminal: {:?}",
        opened.received.borrow()
    );
    assert_eq!(
        opened.probes.get(),
        0,
        "the app shortcut's binding did not run"
    );
    assert!(
        opened.digits.borrow().is_empty(),
        "the number key did not run"
    );
    assert!(opened.terminal_focused(), "the terminal kept focus");
    assert!(opened.capture_armed(), "the slot is still armed");

    assert!(
        opened.cx.update(|_, cx| disarm_key_capture(cx)),
        "disarming found the slot armed"
    );
    assert!(!opened.capture_armed());
    assert!(
        opened.reaches_terminal("a"),
        "a letter reaches the terminal again"
    );
    assert!(!opened.reaches_terminal(probe_chord()));
    assert_eq!(opened.probes.get(), 1, "the app shortcut runs again");
    assert_eq!(
        seen.borrow().len(),
        expected.len(),
        "the handler saw nothing more"
    );
    assert_eq!(resets.get(), 0, "an explicit disarm runs no reset callback");
}

/// SPEC.md: "Escape cancels and is consumed". Farhelm binds Escape to Dismiss, which would close the topmost layer or
/// the tooltip instead, so the capture must see Escape before that binding does; the layer manager needs no part in
/// it. Specified: with focus outside any terminal, a tooltip showing and the slot armed, Escape reaches the handler and
/// the tooltip stays; once disarmed, the same Escape closes it, which shows the binding was live all along.
#[gpui::test]
fn escape_reaches_an_armed_capture_before_the_dismiss_binding(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    // Kept for the whole test, because the owner token's release would take the tooltip down by itself.
    let owner = opened
        .cx
        .update(|_, cx| cx.new(|_| layers::TooltipOwner::default()));
    opened.cx.update(|window, cx| {
        layers::show_tooltip_at(&owner, "help".into(), point(px(5.), px(5.)), window, cx)
    });
    test_support::settle(opened.cx);
    let shown = |opened: &mut Opened| {
        opened
            .cx
            .update(|window, cx| layers::shown_tooltip(window, cx).is_some())
    };
    assert!(shown(&mut opened), "the premise: the tooltip shows");
    let (seen, _) = opened.arm(|_| Captured::Consumed);

    test_support::keys(opened.cx, "escape");
    assert_eq!(keys_of(&seen), ["escape"], "the handler saw Escape");
    assert!(shown(&mut opened), "and Dismiss never ran");

    assert!(opened.cx.update(|_, cx| disarm_key_capture(cx)));
    test_support::keys(opened.cx, "escape");
    assert!(
        !shown(&mut opened),
        "the premise: unarmed, Escape closes the tooltip"
    );
}

/// SPEC.md: "Non-sequence keys, including arrows, other chords and Shift+Enter, cancel and reach their normal
/// target." Specified: a handler's Cancel disarms the slot, and the key it cancelled on goes where it would have with
/// nothing armed: an arrow to the focused terminal, an app shortcut to its binding, a number-key chord to its
/// physical-key row; the next key does not reach the handler.
#[gpui::test]
fn a_cancel_disarms_the_capture_and_the_key_goes_on_to_its_normal_target(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    opened.bind_probe();
    opened.focus_terminal();
    // Consumes plain single characters, as a sequence does with a word's letters, and cancels on anything else.
    let answer = |event: &KeyDownEvent| {
        if event.keystroke.modifiers.modified() || event.keystroke.key.chars().count() != 1 {
            Captured::Cancel
        } else {
            Captured::Consumed
        }
    };

    let (seen, resets) = opened.arm(answer);
    test_support::keys(opened.cx, "a");
    assert!(
        opened.received.borrow().is_empty(),
        "the premise: the letter was consumed"
    );
    assert!(
        opened.reaches_terminal("down"),
        "the arrow it cancelled on reached the terminal"
    );
    assert!(!opened.capture_armed(), "the cancel disarmed the slot");
    assert!(
        opened.reaches_terminal("b"),
        "the next key reached the terminal"
    );
    assert_eq!(keys_of(&seen), ["a", "down"], "and not the handler");

    let (seen, _) = opened.arm(answer);
    assert!(!opened.reaches_terminal(probe_chord()));
    assert_eq!(
        opened.probes.get(),
        1,
        "the chord it cancelled on ran its binding"
    );
    assert_eq!(seen.borrow().len(), 1);
    assert!(!opened.capture_armed());

    let (seen, _) = opened.arm(answer);
    let number = row_of(&TemplateNumberKey { digit: 5 });
    opened
        .cx
        .simulate_event(us_down(THIS_PLATFORM, number, false));
    test_support::settle(opened.cx);
    assert_eq!(
        *opened.digits.borrow(),
        [5],
        "the number key it cancelled on ran its row"
    );
    assert_eq!(seen.borrow().len(), 1);
    assert!(!opened.capture_armed());
    assert_eq!(resets.get(), 0, "a cancel runs no reset callback");
}

/// There is one slot, so a second arm while it is held must not silently take the keys from the first holder, which
/// would go on showing its mode with nothing feeding it. Specified: arming an armed slot is refused and drops the
/// refused closures; keys keep going to the first handler; once the first holder disarms, the second can arm.
#[gpui::test]
fn arming_an_armed_capture_is_refused_until_it_is_disarmed(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    opened.focus_terminal();
    let (first, _) = opened.arm(|_| Captured::Consumed);
    let second = Seen::default();
    let record = second.clone();
    let armed = opened.cx.update(|_, cx| {
        arm_key_capture(
            move |event, _, _| {
                record
                    .borrow_mut()
                    .push((event.keystroke.key.clone(), event.is_held));
                Captured::Consumed
            },
            |_| {},
            cx,
        )
    });
    assert!(!armed, "the second arm was refused");
    assert_eq!(Rc::strong_count(&second), 1, "and its handler was dropped");

    test_support::keys(opened.cx, "a");
    assert_eq!(
        keys_of(&first),
        ["a"],
        "the first handler still gets the keys"
    );
    assert!(second.borrow().is_empty());

    assert!(opened.cx.update(|_, cx| disarm_key_capture(cx)));
    let (third, _) = opened.arm(|_| Captured::Consumed);
    test_support::keys(opened.cx, "b");
    assert_eq!(keys_of(&third), ["b"], "after a disarm the slot arms again");
    assert_eq!(keys_of(&first), ["a"]);
}

/// The handler runs with the window and the app in hand and will open and close layers and end its own mode (Enter
/// launches, then the sequence is over), so the slot must not hold anything borrowed while it runs. Specified: a
/// handler that closes the shown tooltip through the layer manager, disarms the slot and answers Consumed has the key
/// consumed, the tooltip closed and the slot disarmed; an arm from inside the handler is refused; the next key reaches
/// the terminal.
#[gpui::test]
fn the_capture_handler_may_close_a_layer_and_disarm_from_inside(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    opened.focus_terminal();
    let owner = opened
        .cx
        .update(|_, cx| cx.new(|_| layers::TooltipOwner::default()));
    opened.cx.update(|window, cx| {
        layers::show_tooltip_at(&owner, "help".into(), point(px(5.), px(5.)), window, cx)
    });
    test_support::settle(opened.cx);
    assert!(
        opened
            .cx
            .update(|window, cx| layers::shown_tooltip(window, cx).is_some()),
        "the premise: the tooltip shows"
    );
    let inner_arm = Rc::new(Cell::new(None));
    let inner = inner_arm.clone();
    let armed = opened.cx.update(|_, cx| {
        arm_key_capture(
            move |_, window, cx| {
                layers::dismiss_top(window, cx);
                disarm_key_capture(cx);
                inner.set(Some(arm_key_capture(
                    |_, _, _| Captured::Consumed,
                    |_| {},
                    cx,
                )));
                Captured::Consumed
            },
            |_| {},
            cx,
        )
    });
    assert!(armed, "the premise: the slot armed");

    test_support::keys(opened.cx, "enter");
    assert!(opened.received.borrow().is_empty(), "Enter was consumed");
    assert!(
        !opened
            .cx
            .update(|window, cx| layers::shown_tooltip(window, cx).is_some()),
        "the handler closed the tooltip"
    );
    assert_eq!(
        inner_arm.get(),
        Some(false),
        "the arm from inside the handler was refused"
    );
    assert!(!opened.capture_armed(), "the handler's disarm held");
    assert!(
        opened.reaches_terminal("a"),
        "the next key reached the terminal"
    );
}

/// A handler may dispatch something itself before it decides, and GPUI's action handlers stop propagation by default,
/// so a Cancel must not depend on the handler leaving GPUI's propagation flag alone, or the key it cancelled on would
/// silently go nowhere. Specified: a handler that stops propagation and then answers Cancel still has the key reach
/// the focused terminal, and the slot disarms.
#[gpui::test]
fn a_cancel_lets_the_key_go_on_even_after_the_handler_stopped_propagation(cx: &mut TestAppContext) {
    let mut opened = open(cx);
    opened.focus_terminal();
    let armed = opened.cx.update(|_, cx| {
        arm_key_capture(
            |_, _, cx| {
                cx.stop_propagation();
                Captured::Cancel
            },
            |_| {},
            cx,
        )
    });
    assert!(armed, "the premise: the slot armed");
    assert!(
        opened.reaches_terminal("down"),
        "the arrow it cancelled on reached the terminal"
    );
    assert!(!opened.capture_armed(), "the cancel disarmed the slot");
}

/// SPEC.md, "Desktop window chrome": "Cmd+N in the Mac desktop app opens New ..., even from a focused terminal"; the
/// Linux app has no New shortcut. The binding is made on macOS only, where these tests do not run, so its chord and
/// action are checked as data, spelled out here rather than read from `actions.rs`, and the binding itself is bound by
/// hand to check that a held Cmd+N opens New once (the orchestrator decided on 2026-11-14 that a held opening shortcut
/// acts once per press). Specified: the binding is Cmd+N to `NewSession` in Farhelm's context; Cmd+N is an app
/// shortcut in a focused terminal on a Mac and is listed for no Linux chord; holding it runs `NewSession` once.
#[gpui::test]
fn cmd_n_opens_new_on_the_mac_only_and_once_per_press(cx: &mut TestAppContext) {
    let binding = new_session_binding();
    assert_eq!(binding.action().name(), NewSession.name());
    let chords: Vec<&Keystroke> = binding.keystrokes().iter().map(|key| key.inner()).collect();
    assert_eq!(chords, [&parse("cmd-n")]);
    assert_eq!(
        owner_on(Platform::Mac, &down(parse("cmd-n"))),
        KeyOwner::App,
        "Cmd+N is an app shortcut in a focused terminal on a Mac"
    );
    let row = TERMINAL_KEYS
        .iter()
        .find(|row| row.what == "open New")
        .expect("New has a row in the terminal's table");
    assert_eq!((row.mac, row.linux), (Some("cmd-n"), None));
    let mut opened = open(cx);
    opened
        .cx
        .update(|_, cx| cx.bind_keys([new_session_binding()]));
    opened.hold("cmd-n");
    assert_eq!(
        opened.runs_of(NewSession.name()),
        1,
        "a held Cmd+N ran New once"
    );
}

/// SPEC.md, "Quick launch": the ⚡ chord is Cmd+P on a Mac and Ctrl+Shift+P elsewhere, it works from a focused terminal,
/// and "A held chord starts only once; the chord never reaches the terminal"; SPEC.md, "Desktop window chrome": the
/// template list's Cmd+J or Ctrl+Shift+J "never reaches the terminal's program", and like every opening shortcut a held
/// chord opens it once (the migration's orchestrator decided on 2026-11-14 that a held opening shortcut acts once per
/// press). A chord that leaked would type into the agent, and one that repeated would start a session per repeat.
/// Specified, for each chord, spelled out here: it runs its action (`QuickLaunch`, `TemplateList`) from a focused
/// terminal without reaching it, and held for two repeats runs it once more.
#[gpui::test]
fn the_launch_chords_run_once_per_press_from_a_terminal(cx: &mut TestAppContext) {
    let mac = matches!(THIS_PLATFORM, Platform::Mac);
    let rows = [
        (
            if mac { "cmd-p" } else { "ctrl-shift-p" },
            QuickLaunch.name(),
        ),
        (
            if mac { "cmd-j" } else { "ctrl-shift-j" },
            TemplateList.name(),
        ),
    ];
    let mut opened = open(cx);
    opened.focus_terminal();
    for (chord, action) in rows {
        assert!(
            !opened.reaches_terminal(chord),
            "{chord} never reaches the terminal"
        );
        assert_eq!(opened.runs_of(action), 1, "a press of {chord} ran {action}");
        opened.hold(chord);
        assert_eq!(
            opened.runs_of(action),
            2,
            "a held {chord} ran {action} once more"
        );
    }
}

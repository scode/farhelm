//! Tests of the keymap where a terminal has focus: which keys reach the terminal, which stay Farhelm's, and that no
//! chord prefix holds a key back (SPEC_impl.md, "Actions and keymap").
//!
//! Each test opens a window the way the app does (GPUI Kit's `Root` around a root view wired with [`focus::root`]),
//! with the app's whole keymap from the bootstrap, and a terminal stand-in: an element in [`TERMINAL_CONTEXT`] whose
//! key listener records every key that reaches it, as the terminal's will. Reaching the listener is not enough to show
//! a key passed through, because GPUI lets a key go on when the binding that matched it has no handler or its handler
//! declines (Farhelm's Dismiss with nothing open, Kit's Copy with no Kit selection). So the tests also read the
//! keymap's resolution in the focused terminal's real context stack, which shows whether any binding would run at all.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement, KeyBinding,
    KeyContext, KeyDownEvent, Keystroke, Modifiers, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, Window, actions, div, point, px,
};
use gpui_component::Root;

use crate::actions::{
    CONTEXT, KeyOwner, TERMINAL_CONTEXT, TERMINAL_KEYS, THIS_PLATFORM, owner_on, terminal_key_owner,
};
use crate::components::{Button, HoverHelp, Role};
use crate::focus::{self, OnPress};
use crate::{layers, test_support};
use farhelm_input_mapping::Platform;

actions!(keymap_tests, [Probe]);

// ---- Harness ---------------------------------------------------------------------------------------------------------

/// A root view wired like the main view, drawing a button (somewhere for Tab to go) and the terminal stand-in.
struct Scene {
    focus: focus::Attachment,
    _layers: layers::Attachment,
    terminal: FocusHandle,
    button: FocusHandle,
    /// Every key that reached the terminal stand-in's listener, in order.
    received: Rc<RefCell<Vec<Keystroke>>>,
    /// How many times a [`Probe`] binding ran: a test binds one to an app shortcut to see it still works.
    probes: Rc<Cell<usize>>,
}

impl Render for Scene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let received = self.received.clone();
        let probes = self.probes.clone();
        focus::root(div(), &self.focus)
            .size_full()
            .flex()
            .gap(px(8.))
            .p(px(20.))
            .on_action(move |_: &Probe, _, _| probes.set(probes.get() + 1))
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
            ))
            .children(layers::overlay(window, cx))
    }
}

/// What a test holds on to: the window and the scene's handles and records.
struct Opened<'a> {
    cx: &'a mut VisualTestContext,
    terminal: FocusHandle,
    received: Rc<RefCell<Vec<Keystroke>>>,
    probes: Rc<Cell<usize>>,
}

/// Open the scene, bootstrapped like the app, and focus the terminal stand-in by clicking it (a press focuses a
/// terminal on every platform).
fn open(cx: &mut TestAppContext) -> Opened<'_> {
    test_support::install(cx);
    let received = Rc::new(RefCell::new(Vec::new()));
    let probes = Rc::new(Cell::new(0));
    let terminal = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (scene_received, scene_probes, scene_terminal) =
        (received.clone(), probes.clone(), terminal.clone());
    let (_, cx) = cx.add_window_view(|window, cx| {
        let scene = cx.new(|cx| Scene {
            focus: focus::attach(window, cx),
            _layers: layers::attach(window, cx),
            terminal: scene_terminal,
            button: cx.focus_handle().tab_stop(true),
            received: scene_received,
            probes: scene_probes,
        });
        Root::new(scene, window, cx)
    });
    // Simulated windows start inactive, and an inactive window has no focus path for keys to follow.
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let mut opened = Opened {
        cx,
        terminal,
        received,
        probes,
    };
    opened.check();
    opened
}

impl Opened<'_> {
    /// Run the harness's per-step invariant checks (focus and layers), as every GUI test step does.
    fn check(&mut self) {
        self.cx.update(test_support::check_invariants);
    }

    /// Click the terminal stand-in, which focuses it on every platform (a terminal is tracked with
    /// [`OnPress::Focus`]), and assert that it took focus.
    fn focus_terminal(&mut self) {
        let at = match self.cx.debug_bounds("terminal") {
            Some(bounds) => bounds.center(),
            None => panic!("the terminal stand-in was not drawn"),
        };
        self.cx.simulate_click(at, Modifiers::none());
        self.check();
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
        self.cx.simulate_keystrokes(chord);
        self.check();
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
        opened.cx.run_until_parked();
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
    opened.cx.executor().advance_clock(Duration::from_secs(2));
    opened.cx.run_until_parked();
    assert!(
        opened.received.borrow().is_empty(),
        "two seconds on, Ctrl+K is still held back"
    );
    opened.cx.simulate_keystrokes("a");
    let received = opened.received.borrow();
    let keys: Vec<&str> = received.iter().map(|key| key.key.as_str()).collect();
    assert_eq!(
        keys,
        ["k", "a"],
        "the next key released Ctrl+K, late, and then itself"
    );
    assert_eq!(opened.probes.get(), 0, "the chord never completed");
}

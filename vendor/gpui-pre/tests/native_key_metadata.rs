// Modified for Farhelm: changed or added by the patch layers in vendor/patches/gpui/ (vendor/README.md, "GPUI").
//! Focused native-origin dispatch tests using the public fake-platform API.

use gpui::{
    Context, Entity, FocusHandle, IntoElement, KeyBinding, KeyDownEvent, Keystroke, Modifiers,
    PlatformInput, Render, TestAppContext, VisualTestContext, Window, actions,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

actions!(native_input_test, [TestAction, SecondaryTestAction]);

/// A terminal-like raw sink exposes leakage after an interceptor or binding.
/// These events are explicitly injected; this view proves dispatch semantics,
/// not that any OS produced the supplied physical metadata.
struct NativeInputTestView {
    focus_handle: FocusHandle,
    downs: Rc<RefCell<Vec<KeyDownEvent>>>,
    ups: Rc<RefCell<Vec<gpui::KeyUpEvent>>>,
    actions: Rc<Cell<usize>>,
}

impl Render for NativeInputTestView {
    /// Bind raw sinks to the focus path so handled app input cannot be mistaken
    /// for a terminal delivery merely because dispatch constructed an event.
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        use gpui::{InteractiveElement as _, Styled as _};
        let downs = self.downs.clone();
        let ups = self.ups.clone();
        let actions = self.actions.clone();
        gpui::div()
            .key_context("Terminal")
            .track_focus(&self.focus_handle)
            .size_full()
            .on_key_down(move |event, _, _| downs.borrow_mut().push(event.clone()))
            .on_key_up(move |event, _, _| ups.borrow_mut().push(event.clone()))
            .on_action(move |_: &TestAction, _, _| actions.set(actions.get() + 1))
    }
}

/// Use real fake-platform dispatch so matching, phase and observer behavior
/// are tested together rather than copied into a test-only dispatcher.
fn native_input_view(
    cx: &mut TestAppContext,
    bindings: impl IntoIterator<Item = KeyBinding>,
) -> (Entity<NativeInputTestView>, &mut VisualTestContext) {
    cx.update(|cx| cx.bind_keys(bindings));
    let (view, cx) = cx.add_window_view(|_, cx| NativeInputTestView {
        focus_handle: cx.focus_handle(),
        downs: Rc::default(),
        ups: Rc::default(),
        actions: Rc::default(),
    });
    let focus = cx.update(|_, cx| view.read(cx).focus_handle.clone());
    cx.update(|window, cx| {
        window.focus(&focus, cx);
        window.activate_window();
    });
    (view, cx)
}

/// A physical app reservation must see the original repeat/preference fields
/// before the same logical chord can invoke an action or reach a bubble-phase
/// terminal sink. This does not test capture listeners or suppress matching Up.
#[gpui::test]
fn native_key_metadata_interceptor_precedes_logical_matching(cx: &mut TestAppContext) {
    let (view, cx) = native_input_view(
        cx,
        [KeyBinding::new("ctrl-a", TestAction, Some("Terminal"))],
    );
    let original = KeyDownEvent {
        native_key: Some(gpui::NativeKey {
            identity: gpui::NativeKeyIdentity::Xkb("AC08".into()),
            shift: true,
        }),
        keystroke: Keystroke::parse("ctrl-a").unwrap(),
        is_held: true,
        prefer_character_input: true,
    };
    let seen = Rc::new(RefCell::new(Vec::new()));
    let _interceptor = cx.update(|_, cx| {
        cx.intercept_keystrokes({
            let seen = seen.clone();
            move |event, _, cx| {
                let down = event.key_down.as_ref().expect("original Down available");
                seen.borrow_mut().push(down.clone());
                if matches!(
                    &down.native_key,
                    Some(gpui::NativeKey { identity: gpui::NativeKeyIdentity::Xkb(name), .. })
                        if name.as_ref() == "AC08"
                ) {
                    cx.stop_propagation();
                }
            }
        })
    });
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _observer = cx.update(|_, cx| {
        cx.observe_keystrokes({
            let observed = observed.clone();
            move |event, _, _| observed.borrow_mut().push(event.key_down.clone())
        })
    });
    let result =
        cx.update(|window, cx| window.dispatch_event(PlatformInput::KeyDown(original.clone()), cx));
    assert!(!result.propagate);
    assert_eq!(&*seen.borrow(), &[original.clone()]);
    assert!(observed.borrow().is_empty());
    view.read_with(cx, |view, _| {
        assert_eq!(view.actions.get(), 0);
        assert!(view.downs.borrow().is_empty());
    });

    // Physical metadata must not change ordinary logical keybinding matching.
    let other = KeyDownEvent {
        native_key: Some(gpui::NativeKey {
            identity: gpui::NativeKeyIdentity::Xkb("AC01".into()),
            shift: false,
        }),
        is_held: false,
        ..original
    };
    cx.update(|window, cx| {
        window.dispatch_event(PlatformInput::KeyDown(other.clone()), cx);
    });
    view.read_with(cx, |view, _| {
        assert_eq!(view.actions.get(), 1);
        assert!(view.downs.borrow().is_empty());
    });
    assert_eq!(&*observed.borrow(), &[Some(other)]);
}

/// Press/repeat and release preserve their own snapshots through raw handlers;
/// Linux XKB tests separately cover how native Shift survives normalization.
#[gpui::test]
fn native_key_metadata_down_up_and_repeat(cx: &mut TestAppContext) {
    let (view, cx) = native_input_view(cx, []);
    let press = KeyDownEvent {
        native_key: Some(gpui::NativeKey {
            identity: gpui::NativeKeyIdentity::Xkb("AE12".into()),
            shift: true,
        }),
        keystroke: Keystroke::parse("ctrl-+").unwrap(),
        is_held: false,
        prefer_character_input: false,
    };
    let repeat = KeyDownEvent {
        is_held: true,
        ..press.clone()
    };
    let release = gpui::KeyUpEvent {
        native_key: Some(gpui::NativeKey {
            identity: gpui::NativeKeyIdentity::Xkb("AE12".into()),
            shift: false,
        }),
        keystroke: press.keystroke.clone(),
    };
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _observer = cx.update(|_, cx| {
        cx.observe_keystrokes({
            let observed = observed.clone();
            move |event, _, _| observed.borrow_mut().push(event.key_down.clone())
        })
    });
    cx.update(|window, cx| {
        window.dispatch_event(PlatformInput::KeyDown(press.clone()), cx);
        window.dispatch_event(PlatformInput::KeyDown(repeat.clone()), cx);
        window.dispatch_event(PlatformInput::KeyUp(release.clone()), cx);
    });
    view.read_with(cx, |view, _| {
        assert_eq!(&*view.downs.borrow(), &[press.clone(), repeat.clone()]);
        let ups = view.ups.borrow();
        assert_eq!(ups.len(), 1);
        assert_eq!(ups[0].native_key, release.native_key);
    });
    assert_eq!(&*observed.borrow(), &[Some(press), Some(repeat)]);
}

/// Parsed input has a real synthetic Down snapshot but no physical origin;
/// pending-prefix replay must not invent an original event it never stored.
#[gpui::test]
fn native_key_metadata_synthetic_and_replay_absence(cx: &mut TestAppContext) {
    let (_, cx) = native_input_view(
        cx,
        [
            KeyBinding::new("ctrl-b", TestAction, Some("Terminal")),
            KeyBinding::new("ctrl-b h", SecondaryTestAction, Some("Terminal")),
        ],
    );
    let intercepted = Rc::new(RefCell::new(Vec::new()));
    let _interceptor = cx.update(|_, cx| {
        cx.intercept_keystrokes({
            let intercepted = intercepted.clone();
            move |event, _, _| intercepted.borrow_mut().push(event.key_down.clone())
        })
    });
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _observer = cx.update(|_, cx| {
        cx.observe_keystrokes({
            let observed = observed.clone();
            move |event, _, _| {
                observed.borrow_mut().push((
                    event.key_down.clone(),
                    event.keystroke.clone(),
                    event
                        .action
                        .as_ref()
                        .is_some_and(|action| action.as_any().is::<TestAction>()),
                ))
            }
        })
    });
    cx.simulate_keystrokes("x");
    let replayed = Keystroke::parse("ctrl-b").unwrap();
    assert!(
        intercepted.borrow()[0]
            .as_ref()
            .unwrap()
            .native_key
            .is_none()
    );
    assert!(
        observed.borrow()[0]
            .0
            .as_ref()
            .unwrap()
            .native_key
            .is_none()
    );
    // A deferred native-origin prefix must not acquire a fabricated original
    // snapshot when the pending algorithm replays only its logical Keystroke.
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                native_key: Some(gpui::NativeKey {
                    identity: gpui::NativeKeyIdentity::Xkb("AB05".into()),
                    shift: false,
                }),
                keystroke: Keystroke::parse("ctrl-b").unwrap(),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
    });
    assert!(
        intercepted
            .borrow()
            .last()
            .unwrap()
            .as_ref()
            .unwrap()
            .native_key
            .is_some()
    );
    cx.simulate_keystrokes("x");
    assert!(
        observed
            .borrow()
            .iter()
            .any(|(down, keystroke, action)| down.is_none() && keystroke == &replayed && *action),
        "the shorter binding must replay without original Down"
    );
    cx.simulate_modifiers_change(Modifiers::shift());
    cx.simulate_modifiers_change(Modifiers::none());
    assert!(
        intercepted.borrow().last().unwrap().is_none(),
        "a modifier tap is not an original Down"
    );
}

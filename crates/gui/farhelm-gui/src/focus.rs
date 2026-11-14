//! The focus coordinator: the one owner of keyboard focus, implementing SPEC.md's "Keyboard focus".
//!
//! SPEC_impl.md, "Focus: the focus coordinator". Focus used to be decided one bug at a time, mostly in code that ran
//! when something finished later. Here every focus move goes through this module, which holds the one exemption from
//! the bans on GPUI's focus calls (`crates/gui/clippy.toml`); in GPUI a focus move also redraws the whole window and
//! drops pending keystrokes, one more reason for one owner. What it owns:
//!
//! - **"The user moved on."** A counter that any key press or click inside Farhelm advances, except a modifier alone, a
//!   repeat of the same gesture (auto-repeat, a double click, Enter pressed again on the control that already has focus),
//!   scrolling, pointer movement and switching apps, none of which reach it. Key presses are counted where GPUI shows
//!   every keystroke before any binding runs (a keystroke interceptor): a key that a binding handles, Farhelm's own Tab
//!   and Escape or GPUI Kit's editing keys in a text input, never reaches an ordinary key listener, and SPEC.md counts
//!   every key press. Work that finishes later takes a [`Mark`]
//!   when the user starts it and asks [`has_moved_on`] before it moves focus or opens anything. The layer manager refuses
//!   to open a layer for a mark the user has moved on from, which is how "things the user did not start never open over
//!   what they are doing" is kept.
//! - **Home.** When nothing holds focus and no layer is open, focus goes home, which SPEC.md's "Where focus rests"
//!   defines: the selected session's terminal, New when there are no sessions, a stopped session's card when the
//!   selected session has stopped. The views that own those set it: [`set_home`] in general, [`choose_terminal`] when
//!   the user chooses a session (its terminal becomes home and takes focus), and [`replace_home`] when home itself goes
//!   away (a shell tab exits, and the agent tab takes its place). Only when none of them is on screen does focus rest
//!   on the window's root view, which is not a tab stop, so a key press still starts inside Farhelm's key context.
//! - **The dialog stack's trap and restore**, together with the layer manager, which records who opened each layer and
//!   what was focused at the time. Tab and Shift+Tab stay inside the topmost layer; closing a layer returns focus to
//!   where it was before it opened; when the focused control disappears inside a dialog or menu, focus moves to the
//!   closest remaining control there (by the controls' last drawn positions), and to the layer itself only when no
//!   control is left. A menu traps Tab like a dialog (SPEC.md: "Inside a dialog or menu, focus stays inside").
//! - **Which press focuses an element.** Every focusable element is tracked through [`track`], which suppresses GPUI's
//!   own focus-on-press (GPUI focuses any tracked element on mouse-down, a move no lint could see) and applies the
//!   coordinator's rule instead: on Linux a click focuses the control it lands on; on macOS it does not, as in WebKit
//!   there. `track_focus` itself is banned outside this module. The macOS half is ported from the old app, which ran in
//!   WebKit. On Linux the old desktop app built against WebKitGTK but was released only for the Mac, so there is no
//!   shipped behavior to keep, and a click follows the platform.
//! - **Which controls Tab reaches** ([`TabReach`]). On macOS, Tab follows the system's Keyboard navigation setting
//!   exactly as the old app did: with it off, Tab reaches text fields and pop-ups only and skips buttons, and
//!   Option-Tab reaches every control; with it on, Tab reaches every control. Linux has no such setting, and Tab
//!   reaches every control there. The desktop binary supplies how to read the setting ([`follow_tab_reach`]), and the
//!   coordinator reads it on every Tab and Shift+Tab press, as WebKit follows the setting's change at once: the user
//!   can flip it from the keyboard (Control-F7 by default) without leaving the app. The migration's orchestrator
//!   decided this on 2026-11-13, under the rule that the GPUI app ports the old one rather than redesigning it: the Mac
//!   keeps the old app's behavior, and Linux follows the GTK and Qt convention.
//!
//! NOTE: Agent approval cards, which keep focus while a dialog is open, do not exist yet, so [`invariant_violation`]
//! knows only the topmost dialog.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Bounds, Context, Div, ElementId, Entity, FocusHandle, Global,
    InteractiveElement, Keystroke, KeystrokeEvent, MouseButton, MouseDownEvent, ParentElement,
    Pixels, Styled, Subscription, WeakEntity, WeakFocusHandle, Window, WindowId, canvas,
};

use crate::actions::{
    self, Dismiss, FocusNext, FocusNextControl, FocusPrevious, FocusPreviousControl,
};
use crate::layers;

/// Whether a click focuses the control it lands on, on this platform. WebKit on macOS does not focus a button on click,
/// and the old app behaved accordingly there; on Linux, where the old app was never released and the new one follows
/// its platform, a click does.
pub const CLICK_FOCUSES_CONTROLS: bool = !cfg!(target_os = "macos");

/// Which controls a plain Tab reaches. Option-Tab always reaches every control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabReach {
    /// Every focusable control, buttons included: Linux, and macOS with the system's Keyboard navigation setting on.
    EveryControl,
    /// Text fields and pop-ups only, skipping buttons: macOS with Keyboard navigation off, the system default there.
    /// SPEC.md: "keyboard-only users reach buttons with Keyboard navigation or Option-Tab".
    TextFieldsAndPopups,
}

/// Where the coordinator asks which controls Tab reaches. The desktop binary supplies it, because reading a platform
/// setting needs platform code the GUI crates do not carry.
pub type TabReachSource = Rc<dyn Fn() -> TabReach>;

/// The platform rules the coordinator applies, kept as app state so a test can apply another platform's rule: the Mac
/// rules would otherwise be untestable on Linux, where the suite runs most.
struct Rules {
    click_focuses_controls: bool,
    /// Asked on every Tab and Shift+Tab press; without one, Tab reaches every control.
    tab_reach_source: Option<TabReachSource>,
}

impl Global for Rules {}

/// Apply the click rule of the platform that does `focuses` (a test seam: the suite runs on Linux, and the macOS rule
/// needs a test too). Requires [`install`]: a test that forgot it fails here rather than silently keeping the default.
#[cfg(test)]
pub(crate) fn set_click_focuses_controls(focuses: bool, cx: &mut App) {
    cx.global_mut::<Rules>().click_focuses_controls = focuses;
}

fn click_focuses_controls(cx: &App) -> bool {
    cx.try_global::<Rules>()
        .map_or(CLICK_FOCUSES_CONTROLS, |rules| rules.click_focuses_controls)
}

/// Follow `source` for which controls Tab reaches: it is asked on every Tab and Shift+Tab press, so a change to the
/// setting applies to the next press. The bootstrap calls this with what the host supplies, after [`install`]; called
/// before it, it installs the default rules first rather than dropping the source.
pub fn follow_tab_reach(source: TabReachSource, cx: &mut App) {
    if !cx.has_global::<Rules>() {
        cx.set_global(default_rules());
    }
    cx.global_mut::<Rules>().tab_reach_source = Some(source);
}

/// Which controls a plain Tab reaches right now: the source's answer, or every control without one.
fn tab_reach(cx: &App) -> TabReach {
    cx.try_global::<Rules>()
        .and_then(|rules| rules.tab_reach_source.as_ref())
        .map_or(TabReach::EveryControl, |source| source())
}

/// This platform's click rule, and Tab reaching every control until a host supplies the setting to follow.
fn default_rules() -> Rules {
    Rules {
        click_focuses_controls: CLICK_FOCUSES_CONTROLS,
        tab_reach_source: None,
    }
}

// ---- "The user moved on" ---------------------------------------------------------------------------------------------

/// The app-wide moved-on counter and what it needs to recognise a repeated key.
#[derive(Default)]
struct MovedOn {
    count: u64,
    /// The last key press that counted, and what had focus when it did: the same key again on the same control is a
    /// repeat of the gesture (Enter pressed twice on a submit), not a new one.
    last_key: Option<(Keystroke, Option<WeakFocusHandle>)>,
}

impl Global for MovedOn {}

/// A point in the user's input, taken when they start something that may finish later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark(u64);

impl Mark {
    /// The user's input as of now. Take it when the user starts the work, not when the work finishes: a mark taken late
    /// says the user has not moved on, which is exactly the lie the rule exists to prevent.
    pub fn take(cx: &App) -> Self {
        Mark(cx.try_global::<MovedOn>().map_or(0, |moved| moved.count))
    }
}

/// Whether the user pressed a key or clicked inside Farhelm since `mark` was taken. Once they have, a result may not
/// move focus, open anything or switch sessions; it shows up where it belongs instead.
pub fn has_moved_on(mark: &Mark, cx: &App) -> bool {
    Mark::take(cx) != *mark
}

/// The keystroke interceptor that feeds the counter, kept alive for the app's lifetime.
struct KeyWatch {
    _interceptor: Subscription,
}

impl Global for KeyWatch {}

/// Count a key press, unless it repeats the last one on the same control. Runs for every keystroke before any binding
/// is matched; a modifier pressed alone arrives without a key-down and is not a press.
fn note_key(event: &KeystrokeEvent, window: &Window, cx: &mut App) {
    let Some(down) = &event.key_down else {
        return;
    };
    if down.is_held || !cx.has_global::<MovedOn>() {
        return;
    }
    let focused = window.focused(cx);
    let moved = cx.global_mut::<MovedOn>();
    let repeat = moved.last_key.as_ref().is_some_and(|(key, on)| {
        *key == down.keystroke && on.as_ref().and_then(WeakFocusHandle::upgrade) == focused
    });
    moved.last_key = Some((
        down.keystroke.clone(),
        focused.map(|focus| focus.downgrade()),
    ));
    if !repeat {
        moved.count += 1;
    }
}

/// Count a click, unless it is the second or later click of a double click.
fn note_click(event: &MouseDownEvent, cx: &mut App) {
    if event.click_count > 1 || !cx.has_global::<MovedOn>() {
        return;
    }
    let moved = cx.global_mut::<MovedOn>();
    moved.count += 1;
    moved.last_key = None;
}

// ---- Per-window state ------------------------------------------------------------------------------------------------

/// One window's focus state. Kept per window: multi-window support is a watch-list item, and the one thing it asks of
/// the foundations now is that focus and overlays are not shared between windows.
pub struct WindowFocus {
    home: Option<WeakFocusHandle>,
    /// The root view's own handle: it holds focus when nothing else does. GPUI routes a key press from the focused
    /// element up, and with nothing focused it starts at the window's root element, above Farhelm's key context, so
    /// without this Tab and Escape would reach GPUI Kit's bindings instead of Farhelm's.
    root: WeakFocusHandle,
    /// Where each focusable control was last drawn and what kind it is, recorded as frames are prepared: what "the
    /// closest remaining control" is measured against when the focused one disappears, and how Tab knows a button.
    /// Entries for controls no longer drawn stay until pruned; only controls drawn in the current frame are ever chosen.
    controls: Vec<Recorded>,
    /// When to next drop the entries of released controls, so the list stays near the number drawn.
    prune_at: usize,
    /// The focused control and where it was when last drawn, so the closest one to it can take over if it disappears.
    /// Kept with its handle: the position is used only when that control is the one that vanished, never as a stand-in
    /// for some other element that held focus without recording a position.
    last_focused: Option<(WeakFocusHandle, Bounds<Pixels>)>,
}

/// One recorded control: its handle, where it was last drawn, and its kind.
struct Recorded {
    control: WeakFocusHandle,
    bounds: Bounds<Pixels>,
    kind: ControlKind,
}

/// What kind of tab stop a control is, for [`TabReach`]: a plain Tab skips buttons where only text fields and pop-ups
/// are reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ControlKind {
    /// A text field or pop-up: always in Tab's order.
    Field,
    /// A button or menu item: in Tab's order only where Tab reaches every control.
    Button,
}

/// Where each window's focus state lives, by window. Weak, so a closed window's state goes with its root view.
#[derive(Default)]
struct Windows(HashMap<WindowId, WeakEntity<WindowFocus>>);

impl Global for Windows {}

/// Install the app-wide state, and the keystroke interceptor that counts key presses. The bootstrap calls this before
/// any window opens.
pub fn install(cx: &mut App) {
    cx.set_global(MovedOn::default());
    cx.set_global(Windows::default());
    // Until the host says otherwise, Tab reaches every control: the rule wherever there is no setting to follow.
    cx.set_global(default_rules());
    let watch = cx.intercept_keystrokes(|event, window, cx| note_key(event, window, cx));
    cx.set_global(KeyWatch {
        _interceptor: watch,
    });
}

/// What a root view keeps to have the coordinator look after its window: the window's focus state and the listener that
/// recovers focus when its holder disappears. Dropping it detaches the coordinator from the window.
pub struct Attachment {
    root: FocusHandle,
    _state: Entity<WindowFocus>,
    _lost: Subscription,
}

/// Attach the coordinator to the root view's window. Every root view does this once, from its constructor, through
/// [`crate::root::attach`], and draws through [`crate::root::wrap`], which applies [`root`].
pub fn attach<V: 'static>(window: &mut Window, cx: &mut Context<V>) -> Attachment {
    // Not a tab stop: Tab moves from the root to the first control and never back to the root.
    let root = cx.focus_handle().tab_stop(false);
    let state = cx.new(|_| WindowFocus {
        home: None,
        root: root.downgrade(),
        controls: Vec::new(),
        prune_at: 64,
        last_focused: None,
    });
    let id = window.window_handle().window_id();
    // Checked first: `global_mut` panics on a missing global, and a bare test fixture may not have installed it.
    if cx.has_global::<Windows>() {
        let windows = cx.global_mut::<Windows>();
        windows.0.retain(|_, state| state.upgrade().is_some());
        windows.0.insert(id, state.downgrade());
    }
    let lost = cx.on_focus_lost(window, |_, window, cx| {
        recover(window, cx);
        // GPUI calls this listener while it finishes a frame, when a redraw request is ignored and a focus move made
        // here schedules no frame of its own. The move takes effect for keys at once, but the drawn frame keeps saying
        // nothing is focused (no focus events, no ring, the test bridge reporting no focus) until something else
        // redraws, which in an idle window can be never: dismissing the connection-lost banner left it that way. So
        // ask for one more frame once this one is done.
        window.defer(cx, |window, _| window.refresh());
    });
    // After the window's first frame exists, not inside its construction (a cookbook trap).
    window.defer(cx, recover);
    Attachment {
        root,
        _state: state,
        _lost: lost,
    }
}

fn state(window: &Window, cx: &App) -> Option<Entity<WindowFocus>> {
    let id = window.window_handle().window_id();
    cx.try_global::<Windows>()?.0.get(&id)?.upgrade()
}

/// Set the window's home: where focus goes whenever nothing holds it and no layer is open. The selected session's
/// terminal is home (SPEC.md, "Where focus rests"); `None` leaves the window without one. If nothing holds focus now, it
/// goes home at once, but home never takes focus from a control that holds it.
pub fn set_home(home: Option<&FocusHandle>, window: &mut Window, cx: &mut App) {
    let Some(state) = state(window, cx) else {
        return;
    };
    state.update(cx, |state, _| state.home = home.map(FocusHandle::downgrade));
    if holds_nothing(window, cx) && !layers::any_open(window, cx) {
        recover(window, cx);
    }
}

/// The user chose a session, so its terminal `terminal` becomes home and takes focus (SPEC.md, "Where focus rests":
/// "Choosing a session, including the one already selected, focuses its terminal"), taking it from whatever control
/// holds it, such as the session's row the user clicked.
///
/// `chosen` is the mark taken when the user chose. A choice that completes later (the terminal finishing loading, an
/// open the helm accepts after a reply) passes the mark from the moment of choosing; once the user has moved on since,
/// the terminal only becomes home, and takes focus only if nothing holds it (SPEC.md: the session filter typed into
/// meanwhile keeps focus when the terminal arrives). With a layer open the same holds, since focus never leaves the
/// topmost dialog or menu for the page behind it: a dialog that chose a session (the quick switcher) closes itself
/// first, or closing it returns focus to its opener rather than to the terminal.
pub fn choose_terminal(terminal: &FocusHandle, chosen: &Mark, window: &mut Window, cx: &mut App) {
    let Some(state) = state(window, cx) else {
        return;
    };
    if has_moved_on(chosen, cx) || layers::any_open(window, cx) {
        set_home(Some(terminal), window, cx);
        return;
    }
    state.update(cx, |state, _| state.home = Some(terminal.downgrade()));
    apply(Move::To(terminal), window, cx);
}

/// Home is going away and `next` takes its place: a shell tab exited, and the session's agent tab is home now (SPEC.md,
/// "Cases that settled disputed points": "A shell tab exits on its own while the user types in it. The session's agent
/// tab takes focus.").
///
/// If the old home held focus, `next` takes it at once (the window's root if `next` is `None`), so typing goes on in
/// the agent tab whether or not the exited tab is still drawn: focus moves on its own only once its holder is gone from
/// a drawn frame, and nothing says when the exited tab's view takes it out. Otherwise this is [`set_home`]: a
/// control that holds focus, such as the session filter, keeps it, and `next` takes focus only once nothing does. Call
/// it while the old home's focus handle is still alive, or "held focus" cannot be told and only the second rule
/// applies.
pub fn replace_home(next: Option<&FocusHandle>, window: &mut Window, cx: &mut App) {
    let held = home(window, cx).is_some_and(|old| old.contains_focused(window, cx));
    set_home(next, window, cx);
    if held && !layers::any_open(window, cx) {
        recover(window, cx);
    }
}

/// Whether nothing holds focus in SPEC.md's sense: no element at all, or only the root view, which stands in for
/// "nothing" so that keys still reach Farhelm's bindings.
fn holds_nothing(window: &Window, cx: &App) -> bool {
    match window.focused(cx) {
        None => true,
        Some(focused) => state(window, cx)
            .and_then(|state| state.read(cx).root.upgrade())
            .is_some_and(|root| root == focused),
    }
}

fn home(window: &Window, cx: &App) -> Option<FocusHandle> {
    state(window, cx)?.read(cx).home.as_ref()?.upgrade()
}

/// Where focus rests when nothing else holds it and no layer is open: home if the window has one, its root otherwise.
fn resting_place(window: &Window, cx: &App) -> Option<FocusHandle> {
    home(window, cx).or_else(|| state(window, cx)?.read(cx).root.upgrade())
}

// ---- The root's wiring -----------------------------------------------------------------------------------------------

/// Wire a root view's outermost element: Farhelm's key context, the root's focus handle, the focus actions, and the
/// clicks the moved-on counter watches (key presses it sees through its keystroke interceptor). Clicks are seen through
/// a window-wide listener, because a dialog's veil occludes the root from the pointer.
pub fn root(element: Div, attachment: &Attachment) -> Div {
    track_raw(element.key_context(actions::CONTEXT), &attachment.root)
        // A press that no control took (the page's empty space) leaves nothing focused, as a click on a web page's
        // background did, and then focus rests where it rests: home if there is one. GPUI would otherwise focus the root
        // itself, and home would never get focus back. A dialog's veil keeps such presses from reaching here.
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
            if !window.default_prevented() {
                window.prevent_default();
                recover(window, cx);
            }
        })
        .on_action(|_: &FocusNext, window, cx| {
            let every = tab_reach(cx) == TabReach::EveryControl;
            step(true, every, window, cx);
        })
        .on_action(|_: &FocusPrevious, window, cx| {
            let every = tab_reach(cx) == TabReach::EveryControl;
            step(false, every, window, cx);
        })
        .on_action(|_: &FocusNextControl, window, cx| step(true, true, window, cx))
        .on_action(|_: &FocusPreviousControl, window, cx| step(false, true, window, cx))
        .on_action(|_: &Dismiss, window, cx| {
            if !layers::dismiss_top(window, cx) {
                cx.propagate();
            }
        })
        .child(
            canvas(
                |_, _, _| (),
                |_, _, window, _| {
                    window.on_mouse_event(|event: &MouseDownEvent, phase, _, cx| {
                        if phase == gpui::DispatchPhase::Capture {
                            note_click(event, cx);
                        }
                    })
                },
            )
            .absolute()
            .size_0(),
        )
}

// ---- Tracking focusable elements -------------------------------------------------------------------------------------

/// What a press with the pointer does to focus on a tracked element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnPress {
    /// A control (a button, a menu item): it takes focus where the platform's click rule says so ([`pressed`]). It also
    /// counts as a button for [`TabReach`]: where Tab reaches text fields and pop-ups only, a plain Tab skips it. A
    /// pop-up control (a select or combo box) must therefore not use this kind, or it drops out of Tab's order there.
    Control,
    /// An element that takes focus whenever it is pressed, on every platform: a terminal, which a click types into, a
    /// pop-up control, or a dialog whose empty space takes focus (the dialogs SPEC.md does not list as keeping it on the
    /// control in use). A tab stop of this kind counts as a text field or pop-up for [`TabReach`]: Tab always reaches it.
    Focus,
    /// A press leaves focus where it is: a layer's container, whose empty space keeps focus on the control in use.
    Keep,
}

/// Make `element` the element that `handle` focuses, with the coordinator, not GPUI, deciding what a press does
/// (`on_press`). This is the only way Farhelm code tracks focus: GPUI focuses any element with `track_focus` on
/// mouse-down unless something prevents it first, a focus move outside the coordinator that no lint sees, so
/// `track_focus` is banned everywhere else.
///
/// A tab stop's position is recorded as frames are prepared, for "the closest remaining control" ([`record_position`]).
pub fn track<E: InteractiveElement + ParentElement>(
    element: E,
    handle: &FocusHandle,
    on_press: OnPress,
) -> E {
    let pressed_handle = handle.clone();
    let element =
        track_raw(element, handle).on_mouse_down(MouseButton::Left, move |_, window, cx| {
            // Runs before GPUI's own focus-on-press, which a prevented default skips.
            window.prevent_default();
            match on_press {
                OnPress::Control => pressed(&pressed_handle, window, cx),
                OnPress::Focus => apply(Move::To(&pressed_handle), window, cx),
                OnPress::Keep => {}
            }
        });
    let kind = match on_press {
        OnPress::Control => ControlKind::Button,
        OnPress::Focus | OnPress::Keep => ControlKind::Field,
    };
    record_as(element, handle, kind)
}

/// Record where the tab stop `handle` focuses is drawn, for "the closest remaining control", without tracking its focus.
///
/// For a control whose focus something else tracks: GPUI Kit's text input tracks its own focus handle (its focus moves
/// on a click are the accepted exception in SPEC_impl.md), so [`track`] cannot wrap it, yet a text field is as much a
/// candidate for taking focus over as a button. `element` must draw where the control is (its wrapper). A handle that
/// is not a tab stop records nothing. The control counts as a text field for [`TabReach`]: it is in Tab's order on every
/// platform.
pub fn record_position<E: ParentElement>(element: E, handle: &FocusHandle) -> E {
    record_as(element, handle, ControlKind::Field)
}

/// [`record_position`] for a control of `kind`.
fn record_as<E: ParentElement>(element: E, handle: &FocusHandle, kind: ControlKind) -> E {
    if !handle.tab_stop {
        return element;
    }
    let control = handle.downgrade();
    element.child(
        canvas(
            move |bounds, window, cx| record(&control, bounds, kind, window, cx),
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full(),
    )
}

/// The one `track_focus` call in the GUI crates.
#[allow(
    clippy::disallowed_methods,
    reason = "the focus coordinator is the one owner of tracked focus"
)]
fn track_raw<E: InteractiveElement>(element: E, handle: &FocusHandle) -> E {
    element.track_focus(handle)
}

/// Record where `control` (of `kind`) was drawn this frame, and whether it held focus there.
fn record(
    control: &WeakFocusHandle,
    bounds: Bounds<Pixels>,
    kind: ControlKind,
    window: &Window,
    cx: &mut App,
) {
    let Some(state) = state(window, cx) else {
        return;
    };
    let focused = control
        .upgrade()
        .is_some_and(|handle| handle.is_focused(window));
    state.update(cx, |state, _| {
        if focused {
            state.last_focused = Some((control.clone(), bounds));
        }
        if let Some(entry) = state
            .controls
            .iter_mut()
            .find(|entry| &entry.control == control)
        {
            entry.bounds = bounds;
            entry.kind = kind;
            return;
        }
        if state.controls.len() >= state.prune_at {
            state
                .controls
                .retain(|entry| entry.control.upgrade().is_some());
            state.prune_at = (state.controls.len() * 2).max(64);
        }
        state.controls.push(Recorded {
            control: control.clone(),
            bounds,
            kind,
        });
    });
}

/// The control inside `container` that is drawn now and sits closest to where the focused control last was, or `None`
/// when that position is not known to belong to the control that vanished.
fn closest_control(container: &FocusHandle, window: &Window, cx: &App) -> Option<FocusHandle> {
    let state = state(window, cx)?;
    let state = state.read(cx);
    let (last, bounds) = state.last_focused.as_ref()?;
    // The recorded control is still drawn in the layer, so it is not what lost focus: whatever did recorded no position,
    // and measuring from this one would pick a neighbor of the wrong control.
    if last
        .upgrade()
        .is_some_and(|handle| container.contains(&handle, window))
    {
        return None;
    }
    let lost = bounds.center();
    state
        .controls
        .iter()
        .filter_map(|entry| Some((entry.control.upgrade()?, entry.bounds.center())))
        // `contains` reads the frame just drawn, so a control not drawn in it is never chosen.
        .filter(|(control, _)| control.tab_stop && container.contains(control, window))
        .min_by(|(_, a), (_, b)| distance_squared(*a, lost).total_cmp(&distance_squared(*b, lost)))
        .map(|(control, _)| control)
}

/// The squared distance between two points: it orders by distance the same as the distance itself, without a root.
fn distance_squared(a: gpui::Point<Pixels>, b: gpui::Point<Pixels>) -> f32 {
    let (dx, dy) = (f32::from(a.x - b.x), f32::from(a.y - b.y));
    dx * dx + dy * dy
}

// ---- Moving focus ----------------------------------------------------------------------------------------------------

/// The one focus move the coordinator makes, in one of three forms.
enum Move<'a> {
    To(&'a FocusHandle),
    Next,
    Previous,
}

/// Every focus move in the GUI crates goes through here.
#[allow(
    clippy::disallowed_methods,
    reason = "the focus coordinator is the one owner of focus moves"
)]
fn apply(target: Move, window: &mut Window, cx: &mut App) {
    match target {
        Move::To(handle) => window.focus(handle, cx),
        Move::Next => window.focus_next(cx),
        Move::Previous => window.focus_prev(cx),
    }
}

/// Tab or Shift+Tab, and Option-Tab: the next or previous tab stop, staying inside the topmost layer when one is open
/// (SPEC.md: every modal dialog keeps Tab within its controls, and inside a dialog or menu focus stays inside). With
/// `every` false, buttons are skipped ([`TabReach::TextFieldsAndPopups`]); Option-Tab and [`TabReach::EveryControl`]
/// pass `true`.
///
/// When no stop qualifies (a dialog of buttons only, with Tab reaching text fields only), focus stays where it was:
/// moving it to the dialog itself would let a following Enter press the dialog's main action from a Cancel the user
/// had chosen. Only focus that started outside the trap is taken to the trap's container.
fn step(forward: bool, every: bool, window: &mut Window, cx: &mut App) {
    let trap = layers::trap(window, cx);
    let start = window.focused(cx);
    let started_inside = trap
        .as_ref()
        .is_some_and(|trap| trap.contains_focused(window, cx));
    // GPUI's tab order is window-wide and knows nothing of buttons, so walk it until focus lands on a stop that
    // qualifies. The walk ends at the first stop seen twice, so it never cycles forever.
    let mut seen = vec![start.clone()];
    loop {
        apply(
            if forward { Move::Next } else { Move::Previous },
            window,
            cx,
        );
        let focused = window.focused(cx);
        let inside = trap
            .as_ref()
            .is_none_or(|trap| trap.contains_focused(window, cx));
        if inside && (every || !is_button(focused.as_ref(), window, cx)) {
            return;
        }
        if seen.contains(&focused) {
            break;
        }
        seen.push(focused);
    }
    match (&trap, &start) {
        (Some(trap), _) if !started_inside => apply(Move::To(trap), window, cx),
        (_, Some(start)) => apply(Move::To(start), window, cx),
        (Some(trap), None) => apply(Move::To(trap), window, cx),
        (None, None) => recover(window, cx),
    }
}

/// Whether `focused` is a control recorded as a button. A handle never recorded (the root, a layer's container) is not.
fn is_button(focused: Option<&FocusHandle>, window: &Window, cx: &App) -> bool {
    let (Some(focused), Some(state)) = (focused, state(window, cx)) else {
        return false;
    };
    state.read(cx).controls.iter().any(|entry| {
        entry.kind == ControlKind::Button && entry.control.upgrade().as_ref() == Some(focused)
    })
}

/// A control was pressed with the pointer: focus it where a click focuses controls (Linux), leave focus alone where it
/// does not (macOS). [`track`] calls this for [`OnPress::Control`] elements.
fn pressed(handle: &FocusHandle, window: &mut Window, cx: &mut App) {
    if click_focuses_controls(cx) {
        apply(Move::To(handle), window, cx);
    }
}

/// Focus `handle` because a layer just opened and it is where that layer starts (the layer manager calls this).
pub(crate) fn enter_layer(handle: &FocusHandle, window: &mut Window, cx: &mut App) {
    apply(Move::To(handle), window, cx);
}

/// Return focus to `opener` because the layer it opened closed, or recover if it is gone (the layer manager calls this).
pub(crate) fn return_to(opener: Option<&WeakFocusHandle>, window: &mut Window, cx: &mut App) {
    match opener.and_then(WeakFocusHandle::upgrade) {
        // If the opener is no longer drawn, GPUI reports the focus as lost on the next frame and [`recover`] runs then.
        Some(opener) => apply(Move::To(&opener), window, cx),
        None => recover(window, cx),
    }
}

/// Nothing holds focus (its holder disappeared, or a layer closed and its opener is gone): focus stays inside the
/// topmost layer if one is open, and otherwise goes home. Inside a layer it moves to the closest remaining control
/// (SPEC.md, "Opening and closing"), and to the layer itself only when no control is left; never to the page or a
/// terminal behind it.
fn recover(window: &mut Window, cx: &mut App) {
    if let Some(container) = layers::top_container(window, cx) {
        let target = closest_control(&container, window, cx).unwrap_or(container);
        apply(Move::To(&target), window, cx);
    } else if let Some(resting) = resting_place(window, cx) {
        apply(Move::To(&resting), window, cx);
    }
}

/// The focus handle of a control drawn under `key` (keyed element state, so it lives as long as the control is drawn in
/// consecutive frames). A tab stop: controls are reached with Tab. `key` must differ from the control's own element id,
/// because GPUI keys element state by path and type.
pub fn control_handle(key: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> FocusHandle {
    window
        .use_keyed_state(key, cx, |_, cx| {
            ControlFocus(cx.focus_handle().tab_stop(true))
        })
        .read(cx)
        .0
        .clone()
}

/// A control's focus handle, kept in its element state.
struct ControlFocus(FocusHandle);

/// Whether `handle` shows the keyboard focus ring: it holds focus and the user's last input was the keyboard, as the
/// old `:focus-visible` (a ring for the keyboard user who cannot see where they are, not on every click).
pub fn shows_ring(handle: &FocusHandle, window: &Window) -> bool {
    handle.is_focused(window) && window.last_input_was_keyboard()
}

/// What breaks SPEC.md's focus rules in `window` right now, if anything: exactly one element holds focus (GPUI holds at
/// most one, so this checks that something does), and with a dialog open it is inside the topmost one or a layer opened
/// above it. The test harness runs this after every step.
pub fn invariant_violation(window: &Window, cx: &App) -> Option<String> {
    if state(window, cx).is_some() && window.focused(cx).is_none() {
        return Some("nothing holds focus".to_string());
    }
    (!layers::focus_is_where_dialogs_allow(window, cx))
        .then(|| "a dialog is open but focus is outside it and every layer above it".to_string())
}

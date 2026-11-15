//! The macOS title band: the sidebar's app bar and the session pane's top row together behave like a classic title bar
//! (SPEC.md, "Desktop window chrome"), ported from the old app's `window_chrome.rs` and `assets/click-detail.js` at
//! reference commit `24bc0580`.
//!
//! SPEC.md's rule: pressing anywhere on the band's empty background and moving drags the window, and double-clicking
//! there zooms it (repeating restores the prior frame); a single click without movement never zooms. Empty background is
//! everything in the band but its controls (buttons, tabs, the version readout, the sidebar's resize edge) and its text;
//! the wordmark, the gaps and the band's top and bottom edges count as background, and so does the main pane's half with
//! no session open. Controls, text and the terminal keep their own interactions.
//!
//! The old app had to classify each press in JavaScript and carry the verdict across Dioxus's event transport. Here the
//! press reaches the band's rows through GPUI's own propagation, after everything inside them:
//!
//! - **Controls claim a press by preventing its default**, which every Farhelm control already does (the focus
//!   coordinator prevents the default on every control it tracks, so GPUI never focuses one by itself, and GPUI Kit's
//!   buttons prevent it too). A row sees that in the bubble phase and leaves the press alone.
//! - **Text claims a press through [`claims_presses`]**, which marks the press without preventing its default: a press
//!   on text is still an ordinary press on the page's empty space for the focus rules, so focus goes home as on any
//!   other empty press, but it does not move the window.
//! - **What a press does is [`decide`]**, the old `decide_press`: a primary press on empty background drags; a repeat
//!   (GPUI's click count, which on macOS is AppKit's, so it follows the system's double-click speed) zooms, but only when
//!   the press before it, anywhere in the window, was also a primary press on the same row's empty background, so a
//!   press on a tab followed quickly by one beside it neither zooms nor drags (the old "eligibility"). No repeat ever
//!   drags, so a triple-click cannot fling the window. Nothing acts in native fullscreen, where the window has no frame
//!   to move or zoom. A zoom is always a zoom, whatever the system's "double-click a window's title bar to" setting,
//!   because the old app's band toggled the window's maximized state the same way and SPEC.md says the double-click
//!   zooms (GPUI's `titlebar_double_click`, which follows that setting, is not used).
//! - **Text has to opt in.** Anything in a band row that is text, not a control, must be wrapped in
//!   [`claims_presses`], or a press on it moves the window; a new row's text is the easy one to miss.
//!
//! Which press came before is seen by a window-wide listener the main view draws once ([`watch_presses`]): a press
//! anywhere first clears the record, and a press the band acts on records its row again, so any other press in between
//! breaks the pair. The same listeners start an armed drag once the pointer has moved past a small threshold.
//!
//! The band is macOS only; on Linux the rows are drawn without it (SPEC.md: "The Linux window layout retains its
//! existing appearance and gains no drag or zoom behavior"). The same press rule also serves the title bar the Linux
//! window draws for itself where the compositor draws none ([`title_bar_row`]; `linux_title_bar`), which is not part of
//! the app's layout but a stand-in for the compositor's own bar.

use gpui::{
    App, Global, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Styled, Window, canvas, px,
};

/// Whether this build has the band at all: the macOS window only.
pub const ON: bool = cfg!(target_os = "macos");

/// The band's height in the macOS window: the sidebar's app bar and the session's top row are both this tall there
/// (the old macOS window's `--sidebar-title-height`), and the strips that stand in for the top row are too.
pub const HEIGHT: Pixels = px(48.);

/// The app bar's left padding in the macOS window, which keeps the traffic lights clear of the wordmark: the old
/// `.macos-window .app-bar { padding-left: 88px }`, from the lights' place at 12 by 16 points and their width.
const TRAFFIC_LIGHT_ROOM: Pixels = px(88.);

/// Where AppKit draws the traffic lights in the macOS window: 12 points from the left and 16 from the top, vertically
/// centered in the 48-point band, as the old app placed them (`set_traffic_light_inset(12, 16)` at reference commit
/// `24bc0580`). The bootstrap's window options ask for it.
pub const TRAFFIC_LIGHTS: gpui::Point<Pixels> = gpui::point(px(12.), px(16.));

/// The app bar's left padding that keeps the traffic lights clear, or `None` where there are none to keep clear: off
/// macOS, and in native fullscreen, which hides them (the inset goes with them, as in the GPUI prototype's shell).
pub fn traffic_light_room(window: &Window) -> Option<Pixels> {
    (ON && !window.is_fullscreen()).then_some(TRAFFIC_LIGHT_ROOM)
}

/// One of the rows that make up the band. Two presses pair up for a zoom only on the same row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// The sidebar's app bar (the wordmark and the version readout).
    AppBar,
    /// The open session's top row (tabs and session actions).
    SessionTop,
    /// A strip with nothing else in it: over the main pane's top with no session open, standing in for the session's top
    /// row, and across the connection screens' top.
    EmptyPane,
    /// The strip above the notice banners, which keeps a posted notice clear of the traffic lights.
    AboveBanners,
    /// The title bar the Linux window draws for itself under a compositor that draws none (`linux_title_bar`).
    LinuxTitleBar,
}

/// What one press on the band does. Exactly one of these per press: no press both drags and zooms. A drag starts only
/// once the pointer moves with the button held (see [`band`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressAction {
    /// Start the window's native move; it moves only if the pointer moves, so a click alone does nothing.
    Drag,
    /// Zoom the window, or restore it from zoom.
    Zoom,
    /// Leave the press to whatever else handles it.
    None,
}

/// What a primary press does, from what is known about it: whether it landed on the band's empty background (nothing
/// inside the row claimed it), its click count, whether the press before it was a primary press on the same row's
/// empty background, and whether the window is in native fullscreen. Presses with another button never reach it: the
/// row listens for the primary button only.
///
/// A repeat with no such predecessor does nothing at all, not even a drag: its pair partner was somewhere else, so this
/// press starts no gesture. It still counts as the predecessor of the next press, so two background presses in a row
/// always pair up whatever came before them.
pub fn decide(background: bool, click_count: usize, paired: bool, fullscreen: bool) -> PressAction {
    if !background || fullscreen {
        return PressAction::None;
    }
    if click_count < 2 {
        PressAction::Drag
    } else if paired {
        PressAction::Zoom
    } else {
        PressAction::None
    }
}

/// The press record the band keeps across presses, for pairing a repeat with the press before it.
#[derive(Default)]
struct Presses {
    /// The row whose empty background the previous press was a primary press on, if it was one.
    previous: Option<Row>,
    /// The same for the press being handled now, once a row has acted on it.
    current: Option<Row>,
    /// Whether something inside a row claimed the press being handled now ([`claims_presses`]).
    claimed: bool,
    /// What starts the window's move, armed by a press that decided to drag and waiting for the pointer to move with
    /// the button still held (see [`band`]). Run by [`watch_presses`], which sees every move in the window, so a drag
    /// that leaves the row at once (one started on its bottom edge, moving down) still starts.
    armed: Option<(Act, gpui::Point<Pixels>)>,
}

/// How far the pointer must move with the button held before a band press becomes a drag: a click that wobbles by a
/// pixel or two stays a click, so it can still be the first half of a double-click (once AppKit's window move starts,
/// it takes the release, and the next press no longer counts as a second click). GPUI's own drag start uses the same
/// distance.
const DRAG_THRESHOLD: Pixels = px(2.);

/// What a decided press does: the window's move or zoom in the app, a recorder in tests.
type Act = std::rc::Rc<dyn Fn(PressAction, &mut Window, &mut App)>;

impl Global for Presses {}

/// The record, created on first use (it is the band's own state, not app-wide state the bootstrap installs).
fn presses(cx: &mut App) -> &mut Presses {
    cx.default_global::<Presses>()
}

/// The window-wide listeners the band needs, drawn once by the main view, anywhere in its tree (a listener a frame
/// registers through `on_mouse_event` sees every event in the window whatever is under the pointer): every press first
/// clears the press record, in the capture phase so it runs before any row handles the press; the first move with the
/// primary button held starts a drag a row's press armed; and a release disarms it.
pub fn watch_presses() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |_, _, window, _| {
            window.on_mouse_event(|_: &MouseDownEvent, phase, _, cx| {
                if phase == gpui::DispatchPhase::Capture {
                    let presses = presses(cx);
                    presses.previous = presses.current.take();
                    presses.claimed = false;
                    presses.armed = None;
                }
            });
            window.on_mouse_event(|event: &MouseMoveEvent, phase, window, cx| {
                if phase != gpui::DispatchPhase::Bubble {
                    return;
                }
                let presses = presses(cx);
                if event.pressed_button != Some(MouseButton::Left) {
                    presses.armed = None;
                    return;
                }
                let far = |from: gpui::Point<Pixels>| {
                    let d = event.position - from;
                    let (dx, dy, limit) =
                        (f32::from(d.x), f32::from(d.y), f32::from(DRAG_THRESHOLD));
                    dx * dx + dy * dy > limit * limit
                };
                if presses.armed.as_ref().is_some_and(|(_, from)| far(*from))
                    && let Some((act, _)) = presses.armed.take()
                {
                    act(PressAction::Drag, window, cx);
                }
            });
            window.on_mouse_event(|_: &MouseUpEvent, _, _, cx| presses(cx).armed = None);
        },
    )
    .absolute()
    .size_0()
}

/// Mark `element` (text in the band: the version readout until it becomes a control, a label) as keeping its presses:
/// a press on it neither drags nor zooms the window. Its default is left alone, so the focus rules treat the press as
/// any other press on the page's empty space.
pub fn claims_presses<E: InteractiveElement>(element: E) -> E {
    element.on_mouse_down(MouseButton::Left, |_, _, cx| presses(cx).claimed = true)
}

/// Make `element` one of the band's rows: a press on its empty background drags the window, and a double-click there
/// zooms it (see the module docs). Only on macOS; elsewhere the row is returned as it was.
pub fn row<E: InteractiveElement>(element: E, row: Row) -> E {
    if ON {
        band(element, row, move |action, window, cx| {
            act(row, action, window, cx)
        })
    } else {
        element
    }
}

/// Make `element` the Linux window's drawn title bar: the band's press rule on every platform (the bar is drawn only on
/// Linux, but tests run its rule everywhere), so a press on its background and a move drag the window and a
/// double-click maximizes or restores it, as a compositor's own title bar does.
pub(crate) fn title_bar_row<E: InteractiveElement>(element: E) -> E {
    band(element, Row::LinuxTitleBar, move |action, window, cx| {
        act(Row::LinuxTitleBar, action, window, cx)
    })
}

/// [`row`] on every platform, with what a decided press does passed in (`act` in the app), so tests on GPUI's fake
/// platform see each decision instead of a native drag that platform cannot make.
pub(super) fn band<E: InteractiveElement>(
    element: E,
    row: Row,
    act: impl Fn(PressAction, &mut Window, &mut App) + 'static,
) -> E {
    // A press that decides to drag only arms the move; the first move of the pointer with the button still held starts
    // it. Starting AppKit's window move on the press itself hands the rest of the click to AppKit's drag loop, which
    // takes the release, and the next press then never counts as the second click of a double-click: on the test Mac
    // (2026-11-14) no double-click on the band ever zoomed that way. Arming on the press and moving on motion is how
    // the GPUI prototype's band did it, and a click without motion now does nothing at all, as SPEC.md asks. The motion
    // is watched window-wide ([`watch_presses`]), not on the row, since the pointer may leave the row with its first move.
    let act: Act = std::rc::Rc::new(act);
    element.on_mouse_down(MouseButton::Left, move |event, window, cx| {
        let prevented = window.default_prevented();
        let presses = presses(cx);
        let background = !prevented && !presses.claimed;
        let paired = presses.previous == Some(row);
        if background {
            presses.current = Some(row);
        }
        let action = decide(
            background,
            event.click_count,
            paired,
            window.is_fullscreen(),
        );
        if action == PressAction::Drag {
            presses.armed = Some((act.clone(), event.position));
        } else {
            act(action, window, cx);
        }
    })
}

/// A strip of the band with nothing in it, [`HEIGHT`] tall and as wide as its container: what stands in for the
/// session's top row with no session open (the old `.main-empty-titlebar`), laid over the top of the empty pane, and
/// the room above the notice banners. Only on macOS; `None` elsewhere.
pub fn strip(id: &'static str, which: Row) -> Option<impl IntoElement> {
    ON.then(|| row(gpui::div().id(id).w_full().h(HEIGHT).flex_none(), which))
}

/// Where tests of the real app's rows see what the band decided (`desktop::tests::mac`): GPUI's test platform cannot
/// move or zoom a window, so with this installed a decided press is recorded instead of performed. Test builds only.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct Decisions(pub(crate) Vec<(Row, PressAction)>);

#[cfg(test)]
impl Global for Decisions {}

/// Perform a press `row` decided on the real window (or, in a test that installed [`Decisions`], record it).
fn act(row: Row, action: PressAction, window: &mut Window, cx: &mut App) {
    #[cfg(test)]
    if cx.has_global::<Decisions>() {
        cx.default_global::<Decisions>().0.push((row, action));
        return;
    }
    #[cfg(not(test))]
    let _ = (row, cx);
    match action {
        PressAction::Drag => window.start_window_move(),
        PressAction::Zoom => window.zoom_window(),
        PressAction::None => {}
    }
}

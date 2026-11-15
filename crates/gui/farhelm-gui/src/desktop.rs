//! The desktop integration: what the app does with its host platform beyond drawing its window. That is the menu bar
//! and its shortcuts, quitting when the main window closes, the main window's saved frame, its title band and window
//! decorations, and the open-source notices the app menu opens.
//!
//! NOTE: This is area code, not a foundation (`docs/gpui/foundation-ownership.md`, "What is not a foundation"). It
//! belongs to the desktop-integration area, which builds it here and under `desktop/`. It started as the stub the
//! bootstrap's seams were cut against; the menu bar (`menu`) and quitting on close are built, and each function still
//! documented as a stub does nothing yet, so that part of the app behaves as it did before the seams existed.
//!
//! The bootstrap's contract with this module is the signatures of the functions below, the names of [`FrameHost`] and
//! [`FrameFollow`], and the four window options [`dress_window`] may set, nothing more: it passes a `FrameHost`
//! through whole and never reads a field of either type, so the area reshapes every type here (the frame host's
//! fields, the follower's contents, the frame value types the host sees) without touching `bootstrap.rs`. Only a
//! changed signature, or a window option beyond those four, is a bootstrap change.
//!
//! The bootstrap calls into this module at fixed points, and each point is fixed for a reason the area cannot change
//! from here:
//!
//! - [`install_menus`] runs last in the bootstrap's install, after every key binding exists, because GPUI's
//!   `set_menus` reads the shortcuts it shows once, when it runs (SPEC_impl.md, "GPUI cookbook: traps", "Mac menus
//!   read the keymap once").
//! - [`keep_licenses`] runs before the window opens, so the notices window can never be asked for text it lacks.
//! - [`restore_frame`] runs before the window opens and its answer goes into the window's options, so the window's
//!   first frame is already at its saved place (an agent run's `--window-size` still wins over it).
//! - [`dress_window`] fills in the window's options before it opens, so the title band and decorations are chosen at
//!   open time, which is the only time GPUI takes them.
//! - [`quit_when_closed`] and [`follow_frame`] run once the main window exists, because both need its handle.
//!
//! Nothing in the bootstrap interprets what passes through these calls: the host's frame callbacks reach this module
//! as they are, and the conversion between the host's frames and GPUI's window bounds lives here.

use std::time::Duration;

use gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Global, Pixels, Size, Subscription,
    Window, WindowBounds, WindowOptions, point, px, size,
};

use crate::ticker;

pub(crate) mod menu;
#[cfg(test)]
mod tests;

// ---- The menu bar and quitting -------------------------------------------------------------------------------------

/// Register the menu items' app-level handlers and, on macOS, set the menu bar (`menu`: the old app's app menu and
/// Edit menu, with no Close Window).
///
/// The menu's actions and their chords are declared in `actions.rs` like every other key and are never bound here:
/// the actions and keymap foundation is the one place Farhelm binds keys. The bootstrap calls this after every binding
/// is in place, so the menu shows them; a binding added after it would never show in a Mac menu.
pub fn install_menus(cx: &mut App) {
    menu::install(cx, cfg!(target_os = "macos"));
}

/// What keeps the main window's close wired to quitting: dropping the subscription would silently undo it, so the app
/// holds it as a global for its whole life.
struct QuitWhenClosed {
    _closed: Subscription,
}

impl Global for QuitWhenClosed {}

/// Quit the app when `main` closes, on every platform, so the host's quit handler runs as it does for any other quit
/// (SPEC.md, "Desktop window chrome": "the window close button and Cmd+Q still close the app"). Only the main window's
/// close quits: a second window (the notices window) closing must not.
///
/// Needed on both platforms, for different reasons: on macOS GPUI keeps an app running after its last window closes,
/// and on Linux it quits only once every window has, so an open notices window would keep it running.
pub fn quit_when_closed(main: AnyWindowHandle, cx: &mut App) {
    when_closed(main, |cx| cx.quit(), cx);
}

/// Run `quit` when `main` closes, and only then: [`quit_when_closed`] with the quit passed in, because GPUI's test
/// platform ignores a quit, so a test needs its own to see whether one was asked for.
fn when_closed(main: AnyWindowHandle, quit: impl Fn(&mut App) + 'static, cx: &mut App) {
    let main = main.window_id();
    let closed = cx.on_window_closed(move |cx, closed| {
        if closed == main {
            quit(cx);
        }
    });
    cx.set_global(QuitWhenClosed { _closed: closed });
}

// ---- Open-source notices --------------------------------------------------------------------------------------------

/// Keep the host's source of the license notices (`Host::licenses`), for the "Open-Source Notices" window the app
/// menu opens. It is a function rather than the text because the text is close to a megabyte and is needed only while
/// that window is open: call it when the window opens, and do not hold its answer for the life of the app. The
/// bootstrap calls this once, before the window opens. The stub drops it.
pub fn keep_licenses(_licenses: fn() -> String, _cx: &mut App) {}

// ---- The main window's frame ----------------------------------------------------------------------------------------
//
// SPEC.md, "Desktop window chrome": the app remembers its last ordinary window frame and whether it was maximized, and
// restores the frame only when it fits a connected display. The rules are the desktop crate's (its `window_state`); the
// window side only reports what the window is and puts the window where the host says.

/// A rectangle in GPUI's logical pixels: a window's frame (its origin and content size, [`observed`]) or a display's
/// bounds, in the coordinates GPUI places windows in (on macOS, relative to the primary display; see [`facts`]).
///
/// Logical, not physical: before a window exists GPUI reports displays in logical pixels only
/// (`PlatformDisplay::bounds` and its siblings) and gives no scale factor, and a window's scale factor applies to one
/// display only, so a frame saved in physical pixels could not be converted back before the window opens, nor across
/// displays with different scales. Rounded to whole pixels, which is all a saved frame needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl FrameRect {
    /// `bounds`, rounded to whole pixels (a negative size, which no window or display has, reads as zero).
    fn of(bounds: Bounds<Pixels>) -> Self {
        let whole = |value: Pixels| f32::from(value).round();
        FrameRect {
            x: whole(bounds.origin.x) as i32,
            y: whole(bounds.origin.y) as i32,
            width: whole(bounds.size.width).max(0.) as u32,
            height: whole(bounds.size.height).max(0.) as u32,
        }
    }

    /// The same rectangle as GPUI bounds.
    fn bounds(self) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(self.x as f32), px(self.y as f32)),
            size: size(px(self.width as f32), px(self.height as f32)),
        }
    }
}

/// What the host needs to know to place or remember the window, as plain values so no GPUI type crosses into the
/// host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameFacts {
    /// The displays the window is placed among: on macOS the primary display only, on Linux every display, largest
    /// first ([`facts`]).
    pub displays: Vec<FrameRect>,
    /// The size the window opens at when the host restores nothing.
    pub default_size: (u32, u32),
    /// Whether the position reported now means anything: false on Linux (Wayland reports none), and on macOS while the
    /// window is on a display other than the primary one ([`facts`]); the host then keeps its prior position.
    pub positions_trusted: bool,
}

/// Where the host wants the window to open: its frame (origin and content size, as [`Observed`] reports it), and
/// whether it starts maximized (with `frame` the frame it restores to).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub frame: FrameRect,
    pub maximized: bool,
}

/// One observation of the main window, raw: the host decides what to remember of it (a maximized or fullscreen frame
/// must not replace the last ordinary one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observed {
    /// The window's frame while it is neither maximized nor fullscreen: its origin and its content size, which is what
    /// it opens at again ([`observed`]). Not meaningful while it is maximized or fullscreen; the host keeps its last
    /// ordinary frame then.
    pub frame: FrameRect,
    pub maximized: bool,
    pub fullscreen: bool,
}

/// Called once, before the window opens, with the facts of the moment; answers where to open, or `None` for the
/// default frame centered on the primary display.
pub type RestoreFrame = Box<dyn FnOnce(FrameFacts) -> Option<Placement>>;

/// Called on the main thread whenever the main window's frame, maximized or fullscreen state changes, on every pixel of
/// a drag. It must return quickly, and should only remember: [`Settled`] says when to save.
pub type ObserveFrame = Box<dyn FnMut(FrameFacts, Observed)>;

/// Called on the main thread once the main window has stopped changing for [`SETTLE`]: the moment to save what the
/// observer was last told.
pub type Settled = Box<dyn FnMut()>;

/// How long the main window must stay unchanged before the host hears that it settled: long enough that a drag or a
/// resize in progress (whose reports come many times a second) is one change, short enough that a crash soon after
/// loses little.
pub const SETTLE: Duration = Duration::from_secs(1);

/// The host's half of the window frame (`Host::window_frame`): how a launch reads the saved frame, and how the host
/// hears about every change to it. A host with nowhere to save a frame (tests, any host without a state directory)
/// passes none, and the window opens at the default frame every time. The bootstrap passes it through whole.
pub struct FrameHost {
    pub restore: RestoreFrame,
    pub observe: ObserveFrame,
    pub settled: Settled,
}

/// What [`restore_frame`] hands on to [`follow_frame`] across the window's opening: opaque to the bootstrap, which
/// only carries it from one call to the other.
pub struct FrameFollow {
    observe: Option<(ObserveFrame, Settled)>,
}

/// The facts of the moment, as seen from `window` once it is open: the displays the window is placed among, the default
/// frame's size, and whether a position means anything right now.
///
/// NOTE: GPUI's displays and positions differ by platform, and this is where that is absorbed. On macOS it reports
/// every display at the origin, gives a window's bounds relative to the display the window is on, and opens a window on
/// the primary display unless told otherwise (the bootstrap names no display). So on macOS only the primary display is
/// reported, and a position counts only while the window is on it: a window last moved to another display reopens on
/// the primary one, at the last place it had there or centered (a recorded deviation from the old app, which saved
/// global positions). On Linux the app runs only on Wayland (shipped builds compile no X11), which reports no positions
/// and no primary display, so every position is untrusted and the displays come largest first, which is the one a
/// fallback frame is sized for.
fn facts(window: Option<&Window>, cx: &App) -> FrameFacts {
    let primary = cx.primary_display();
    let primary_id = primary.as_ref().map(|display| display.id());
    let (displays, positions_trusted) = if cfg!(target_os = "linux") {
        let mut all: Vec<_> = cx
            .displays()
            .into_iter()
            .map(|display| FrameRect::of(display.bounds()))
            .collect();
        all.sort_by_key(|rect| std::cmp::Reverse(u64::from(rect.width) * u64::from(rect.height)));
        (all, false)
    } else {
        let on_primary = window
            .is_none_or(|window| window.display(cx).map(|display| display.id()) == primary_id);
        (
            primary
                .into_iter()
                .map(|display| FrameRect::of(display.bounds()))
                .collect(),
            on_primary,
        )
    };
    let default = FrameRect::of(Bounds {
        origin: point(px(0.), px(0.)),
        size: crate::bootstrap::DEFAULT_WINDOW_SIZE,
    });
    FrameFacts {
        displays,
        default_size: (default.width, default.height),
        positions_trusted,
    }
}

/// What the main window reports about its frame now. The frame is the window's origin with its content size: the
/// content size is what a window opens at (on macOS GPUI opens a window by its content rectangle, so saving the outer
/// size would grow the window by its title bar on every launch until the title band makes the two equal; on Wayland
/// they are the same). While the window is maximized or fullscreen the host keeps the ordinary frame it had before, so
/// the frame reported then is not used.
fn observed(window: &Window, cx: &App) -> Observed {
    let frame = Bounds {
        origin: window.bounds().origin,
        size: window.viewport_size(),
    };
    Observed {
        frame: FrameRect::of(frame),
        maximized: maximized(window, cx),
        fullscreen: window.is_fullscreen(),
    }
}

/// How far a macOS window's size may be from its display's visible area and still count as zoomed ([`maximized`]).
const ZOOM_SLACK: Pixels = px(2.);

/// Whether the window is maximized (zoomed, on macOS).
///
/// On macOS GPUI answers this by comparing the window's size with the display's visible area exactly, and a window GPUI
/// itself zoomed as it opened (a launch restoring a maximized window) comes out a point short of that area, so it read
/// as not maximized, and the next save forgot that it was (seen on the test Mac, 2026-11-14: 1024 by 681 points on a
/// 1024 by 682 visible area). Within [`ZOOM_SLACK`] of the visible area counts as zoomed here. The cost: a window the
/// user sizes by hand to fill the display within a point or two is remembered as zoomed, which reopens it the same
/// size. Elsewhere GPUI's own answer stands.
fn maximized(window: &Window, cx: &App) -> bool {
    let gpui_says = window.is_maximized();
    if gpui_says || !cfg!(target_os = "macos") {
        return gpui_says;
    }
    window
        .display(cx)
        .is_some_and(|display| fills(window.bounds().size, display.visible_bounds().size))
}

/// Whether a window of `size` fills a display's visible area of `area`, within [`ZOOM_SLACK`] each way ([`maximized`]).
pub(crate) fn fills(size: Size<Pixels>, area: Size<Pixels>) -> bool {
    let near = |a: Pixels, b: Pixels| (f32::from(a) - f32::from(b)).abs() <= f32::from(ZOOM_SLACK);
    near(size.width, area.width) && near(size.height, area.height)
}

/// The GPUI bounds a host's placement opens the window at.
fn bounds_of(placement: Placement) -> WindowBounds {
    let bounds = placement.frame.bounds();
    if placement.maximized {
        WindowBounds::Maximized(bounds)
    } else {
        WindowBounds::Windowed(bounds)
    }
}

/// Ask the host where the main window opens: the bounds GPUI opens it at, or `None` for the default frame, and the
/// follower [`follow_frame`] takes once the window exists. The bootstrap calls this once, before the window opens,
/// with the host's frame as it came.
pub fn restore_frame(
    frame: Option<FrameHost>,
    cx: &mut App,
) -> (Option<WindowBounds>, FrameFollow) {
    let Some(FrameHost {
        restore,
        observe,
        settled,
    }) = frame
    else {
        return (None, FrameFollow { observe: None });
    };
    let placement = restore(facts(None, cx));
    (
        placement.map(bounds_of),
        FrameFollow {
            observe: Some((observe, settled)),
        },
    )
}

/// The entity that holds the host's callbacks and the subscription to the main window's bounds: GPUI hands bounds
/// changes only to an entity's observer, so the follower is one, kept alive by [`Following`] for the app's life.
struct Follower {
    observe: ObserveFrame,
    settled: Settled,
    /// The settled call, waiting; every change moves it later, and dropping it cancels it.
    settling: Option<ticker::Deadline>,
    _bounds: Option<Subscription>,
}

impl Follower {
    /// Report what `window` is now, and tell the host it settled once it has stayed so for [`SETTLE`].
    fn report(&mut self, window: &Window, cx: &mut Context<Self>) {
        (self.observe)(facts(Some(window), cx), observed(window, cx));
        let handle = window.window_handle();
        let follower = cx.weak_entity();
        self.settling = Some(ticker::after(SETTLE, cx, move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = follower.update(cx, |follower, cx| follower.settle(window, cx));
            });
        }));
    }

    /// The window has stayed unchanged for [`SETTLE`]: report what it is now, read afresh rather than taken from its
    /// last change, then tell the host. Defensive: a window's maximized or fullscreen state is not guaranteed to come
    /// with a bounds change, and the frame saved should be the window's state when it settled, whatever event last
    /// reported it.
    fn settle(&mut self, window: &Window, cx: &mut Context<Self>) {
        (self.observe)(facts(Some(window), cx), observed(window, cx));
        (self.settled)();
    }
}

/// Keeps the main window's follower alive: dropping it would silently stop the host from hearing about the frame.
struct Following {
    _follower: gpui::Entity<Follower>,
}

impl Global for Following {}

/// Report every change to `main`'s frame to the host, for as long as the window exists: GPUI calls its bounds
/// observers on every move and resize, which covers maximizing, leaving it and fullscreen too, since each changes the
/// window's bounds. The window's state at the moment it is followed is reported too, so the host starts from the frame
/// the window really opened at.
///
/// Once the window has stopped changing for [`SETTLE`], the host hears that it settled, once (a later change starts the
/// wait again): the desktop app saves the frame then, so a drag is written once, when it ends.
pub fn follow_frame(main: AnyWindowHandle, follow: FrameFollow, cx: &mut App) {
    let Some((observe, settled)) = follow.observe else {
        return;
    };
    let follower = cx.new(|_| Follower {
        observe,
        settled,
        settling: None,
        _bounds: None,
    });
    let followed = main.update(cx, |_, window, cx| {
        follower.update(cx, |follower, cx| {
            follower._bounds =
                Some(cx.observe_window_bounds(window, |follower, window, cx| {
                    follower.report(window, cx)
                }));
            follower.report(window, cx);
        })
    });
    if followed.is_err() {
        tracing::warn!("the main window was gone before its frame could be followed");
        return;
    }
    cx.set_global(Following {
        _follower: follower,
    });
}

// ---- The title band and decorations ---------------------------------------------------------------------------------

/// Set how the main window's frame is drawn, in the options the bootstrap built for it: the macOS title band (a
/// transparent title bar, the traffic lights moved into the band, and the app owning the drag) and, on Linux, which
/// side draws the decorations.
///
/// This sets only `titlebar`'s `appears_transparent` and `traffic_light_position`, `app_owns_titlebar_drag` and
/// `window_decorations`. The rest of the options (the title, the bounds, the minimum size, the app id, the frame
/// intervals) are the bootstrap's and stay as it set them.
///
/// NOTE: A transparent title bar with app-owned dragging leaves the window without any drag area until the band that
/// moves the window exists, so these values change together with the band, never before it.
///
/// The stub leaves the options as they are, which is GPUI's defaults for these fields.
pub fn dress_window(_options: &mut WindowOptions, _cx: &App) {}

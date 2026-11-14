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

use gpui::{AnyWindowHandle, App, Global, Subscription, WindowBounds, WindowOptions};

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

/// A rectangle in physical pixels, the unit the host saves frames in: a saved frame must mean the same thing on
/// displays with different scale factors, which logical pixels do not.
///
/// NOTE: Before a window exists GPUI reports displays in logical pixels only (`PlatformDisplay::bounds` and its
/// siblings) and gives no scale factor; `Window::scale_factor` is the only one, and it needs a window. So a
/// physical-pixel frame cannot be converted exactly before the window opens. Saving logical pixels with a display id,
/// or correcting the frame once the window exists, are the ways around it; the unit here is the area's choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// What the host needs to know to place or remember the window, as plain values so no GPUI type crosses into the
/// host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameFacts {
    /// The connected displays, primary first.
    pub displays: Vec<PhysicalRect>,
    /// The outer frame size the window opens at when the host restores nothing.
    pub default_size: (u32, u32),
    /// Whether the platform reports global window positions at all. False on Wayland, where every position this
    /// module reports is meaningless and the host keeps its prior one.
    pub positions_trusted: bool,
}

/// Where the host wants the window to open: its outer frame, and whether it starts maximized (with `frame` the size
/// it restores to).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub frame: PhysicalRect,
    pub maximized: bool,
}

/// One observation of the main window, raw: the host decides what to remember of it (a maximized or fullscreen frame
/// must not replace the last ordinary one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observed {
    /// The outer frame.
    pub frame: PhysicalRect,
    pub maximized: bool,
    pub fullscreen: bool,
}

/// Called once, before the window opens, with the facts of the moment; answers where to open, or `None` for the
/// default frame centered on the primary display.
pub type RestoreFrame = Box<dyn FnOnce(FrameFacts) -> Option<Placement>>;

/// Called on the main thread whenever the main window's frame, maximized or fullscreen state changes. It must return
/// quickly: the host throttles its own writes.
pub type ObserveFrame = Box<dyn FnMut(FrameFacts, Observed)>;

/// The host's half of the window frame (`Host::window_frame`): how a launch reads the saved frame, and how the host
/// hears about every change to it. A host with nowhere to save a frame (tests, any host without a state directory)
/// passes none, and the window opens at the default frame every time. The bootstrap passes it through whole.
pub struct FrameHost {
    pub restore: RestoreFrame,
    pub observe: ObserveFrame,
}

/// What [`restore_frame`] hands on to [`follow_frame`] across the window's opening: opaque to the bootstrap, which
/// only carries it from one call to the other.
pub struct FrameFollow {
    _observe: Option<ObserveFrame>,
}

/// Ask the host where the main window opens: the bounds GPUI opens it at, or `None` for the default frame, and the
/// follower [`follow_frame`] takes once the window exists. The bootstrap calls this once, before the window opens,
/// with the host's frame as it came.
///
/// The stub never calls `restore` and answers `None`.
pub fn restore_frame(
    frame: Option<FrameHost>,
    _cx: &mut App,
) -> (Option<WindowBounds>, FrameFollow) {
    let follow = FrameFollow {
        _observe: frame.map(|frame| frame.observe),
    };
    (None, follow)
}

/// Report every change to `main`'s frame to the host, for as long as the window exists.
///
/// The stub drops the follower without calling the host.
pub fn follow_frame(_main: AnyWindowHandle, _follow: FrameFollow, _cx: &mut App) {}

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

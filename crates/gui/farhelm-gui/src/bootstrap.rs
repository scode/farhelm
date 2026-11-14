//! The bootstrap: install every piece of app-wide state, then open the main window.
//!
//! SPEC_impl.md, "Startup and quit: the bootstrap": one sequence installs all app-wide state before anything runs,
//! because GPUI panics when code reads a global that is not installed yet, and startup callbacks can fire earlier than
//! one expects (another GPUI app aborted 176 ms after launch that way). Order matters here: Kit's own init first (it
//! installs its theme and the text input's actions), then the bundled fonts, then Farhelm's theme, which re-projects
//! Kit's theme from Farhelm's tokens, then the layer manager's slot and, in test builds, the render counter. Only then
//! does a window open. Farhelm's keymap joins this sequence, after Kit's, with the actions and keymap owner.
//!
//! Nothing here may panic: on macOS this runs inside an AppKit callback, where a panic aborts the process. Failures are
//! returned, and [`run`] logs them and quits.

use gpui::{
    App, AppContext as _, Bounds, Size, WindowBounds, WindowHandle, WindowOptions, px, size,
};
use gpui_component::Root;

use crate::assets::{self, Assets};
use crate::layers;
use crate::main_view::MainView;
use crate::render_count;
use crate::theme::{self, Preference};

/// The smallest window the app supports, in logical pixels.
///
/// The old app set no minimum; its reference screenshots show 716 by 560 as the smallest size at which the sidebar at
/// its default width and the main pane's 320-pixel floor still fit side by side. 720 by 560 rounds that up. SPEC.md
/// states the minimum.
pub const MIN_WINDOW_SIZE: Size<gpui::Pixels> = size(px(720.), px(560.));

/// The window's size on a first launch, before any saved frame exists, fitted to the display: the old app's safe
/// fallback (`safe_fallback` in the desktop app's window state, 1100 by 760), which fits a 1440 by 900 display with room
/// to spare. SPEC.md, "Desktop window chrome": a first launch opens at a safe size centered on a current display.
pub const DEFAULT_WINDOW_SIZE: Size<gpui::Pixels> = size(px(1100.), px(760.));

/// The window's title as the system shows it (window lists, the dock's window menu).
pub const WINDOW_TITLE: &str = "Farhelm";

/// The application identity Wayland compositors and the macOS bundle use.
pub const APP_ID: &str = "io.farhelm.Farhelm";

/// Install all app-wide state. Call once, inside the GPUI app, before opening any window.
pub fn install(cx: &mut App) -> anyhow::Result<()> {
    gpui_component::init(cx);
    cx.text_system().add_fonts(assets::ui_fonts())?;
    // The persisted choice comes from the local preference store once it exists; until then every launch starts at
    // System, the old app's default.
    theme::install(Preference::System, cx.window_appearance(), cx);
    layers::install(cx);
    render_count::install(cx);
    Ok(())
}

/// The main window's options: title, app identity, minimum size and a first-launch frame centered on the display.
pub fn window_options(cx: &App) -> WindowOptions {
    // Never larger than the display: a first launch on a small screen gets the whole screen, not an overflowing frame.
    let first = match cx.primary_display() {
        Some(display) => {
            let screen = display.bounds().size;
            size(
                DEFAULT_WINDOW_SIZE.width.min(screen.width),
                DEFAULT_WINDOW_SIZE.height.min(screen.height),
            )
        }
        None => DEFAULT_WINDOW_SIZE,
    };
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, first, cx))),
        window_min_size: Some(MIN_WINDOW_SIZE),
        app_id: Some(APP_ID.into()),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some(WINDOW_TITLE.into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Open the main window: Kit's `Root` around Farhelm's main view.
///
/// The builder closure only builds views. Anything that changes app state or redraws (here, matching the theme to the
/// window's appearance) runs once the window exists: work inside `open_window`'s builder is a cookbook trap.
pub fn open_main_window(cx: &mut App) -> anyhow::Result<WindowHandle<Root>> {
    let handle = cx.open_window(window_options(cx), |window, cx| {
        let main = cx.new(|cx| MainView::new(window, cx));
        cx.new(|cx| Root::new(main, window, cx))
    })?;
    handle.update(cx, |_, window, cx| {
        theme::follow_system(window.appearance(), cx)
    })?;
    Ok(handle)
}

/// Run the desktop GUI until it quits. The desktop binary calls this after it has started the helm it embeds.
pub fn run() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            if let Err(error) = install(cx).and_then(|()| open_main_window(cx).map(|_| ())) {
                tracing::error!("the window could not open: {error:#}");
                cx.quit();
            }
        });
}

//! The bootstrap: install every piece of app-wide state, then open the main window.
//!
//! SPEC_impl.md, "Startup and quit: the bootstrap": one sequence installs all app-wide state before anything runs,
//! because GPUI panics when code reads a global that is not installed yet, and startup callbacks can fire earlier than
//! one expects (another GPUI app aborted 176 ms after launch that way). Order matters here. In [`run`], before any of
//! it, an agent build starts the test-automation bridge (so a run whose window fails to open can still be inspected),
//! and the host's quit wiring is installed (so a quit requested while the window is still opening is not lost). Then
//! [`install`]: in an agent build, the startup overrides a screenshot batch passes (`agent_startup`), so the theme and
//! the first window see them; then Kit's own init (it installs its theme and the text input's actions), then the
//! bundled fonts, then Farhelm's theme, which re-projects Kit's theme from Farhelm's tokens, then Farhelm's key bindings
//! (after Kit's, so a deeper Farhelm context wins where both bind a key), the focus coordinator's and the layer
//! manager's registries, the ticker, the notice center and the local state that survives resets, and, in test builds,
//! the render counter. Then the coordinator starts following the host's answer for which controls Tab reaches, and the
//! UI channel to the helm opens, so the window's first frame already reflects where the connection stands. Only then
//! does a window open.
//!
//! Nothing here may panic: on macOS this runs inside an AppKit callback, where a panic aborts the process. Failures are
//! returned, and [`run`] hands them to the host's [`Host::on_startup_failure`], which owns how the process reports them.

use std::future::Future;
use std::pin::Pin;

use gpui::{
    App, AppContext as _, Bounds, Global, Size, Subscription, Task, WindowBounds, WindowHandle,
    WindowOptions, px, size,
};
use gpui_component::Root;

use crate::assets::{self, Assets};
use crate::main_view::MainView;
use crate::render_count;
use crate::theme::{self, Preference};
use crate::{actions, focus, layers, local_state, notices, session, ticker};

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

/// The app's identity on Linux: the Wayland app id and the X11 window class. It matches the bundle identifier
/// `scripts/install.sh` writes into the macOS app's Info.plist, so the app has one name everywhere. On macOS GPUI
/// ignores it (its `set_app_id` does nothing); the app's identity there, and anything keyed by it, comes from that
/// Info.plist.
pub const APP_ID: &str = "org.scode.farhelm.desktop";

/// Install all app-wide state. Call once, inside the GPUI app, before opening any window.
pub fn install(cx: &mut App) -> anyhow::Result<()> {
    #[cfg(feature = "agent")]
    cx.set_global(crate::agent_startup::from_env()?);
    install_app_state(cx)
}

/// Everything [`install`] does after an agent build has read its startup overrides. Split out so a test can install
/// overrides of its own (tests never set the process environment) and then run exactly what a launch runs.
pub(crate) fn install_app_state(cx: &mut App) -> anyhow::Result<()> {
    gpui_component::init(cx);
    cx.text_system().add_fonts(assets::ui_fonts())?;
    cx.text_system().add_fonts(assets::terminal_fonts())?;
    // The persisted choice will come from the helm's preferences (the maintainer decided on 2026-11-13 that appearance
    // is a helm preference, not a per-device one); until then every launch starts at System, the old app's default (an
    // agent run may force light or dark, see `starting_preference`).
    theme::install(starting_preference(cx), cx.window_appearance(), cx);
    actions::install(cx);
    focus::install(cx);
    layers::install(cx);
    ticker::install(cx);
    notices::install(cx);
    local_state::install(cx);
    render_count::install(cx);
    Ok(())
}

/// The appearance preference a launch starts with: System, unless an agent run forced one
/// ([`crate::agent_startup`]). Test builds take this one too, so tests can install overrides; they start at System
/// unless a test does.
#[cfg(any(feature = "agent", test))]
fn starting_preference(cx: &App) -> Preference {
    crate::agent_startup::current(cx)
        .preference
        .unwrap_or(Preference::System)
}

/// The appearance preference a launch starts with: System, the old app's default.
#[cfg(not(any(feature = "agent", test)))]
fn starting_preference(_cx: &App) -> Preference {
    Preference::System
}

/// The size a first launch asks for before fitting it to the display: [`DEFAULT_WINDOW_SIZE`], unless an agent run
/// asked for another ([`crate::agent_startup`]). Test builds take this one too, as with `starting_preference`.
#[cfg(any(feature = "agent", test))]
fn first_launch_size(cx: &App) -> Size<gpui::Pixels> {
    crate::agent_startup::current(cx)
        .window_size
        .unwrap_or(DEFAULT_WINDOW_SIZE)
}

/// The size a first launch asks for before fitting it to the display.
#[cfg(not(any(feature = "agent", test)))]
fn first_launch_size(_cx: &App) -> Size<gpui::Pixels> {
    DEFAULT_WINDOW_SIZE
}

/// The main window's options: title, app identity, minimum size and a first-launch frame centered on the display.
pub fn window_options(cx: &App) -> WindowOptions {
    // Never larger than the display: a first launch on a small screen gets the whole screen, not an overflowing frame.
    let wanted = first_launch_size(cx);
    let first = match cx.primary_display() {
        Some(display) => {
            let screen = display.bounds().size;
            size(
                wanted.width.min(screen.width),
                wanted.height.min(screen.height),
            )
        }
        None => wanted,
    };
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, first, cx))),
        window_min_size: Some(MIN_WINDOW_SIZE),
        app_id: Some(APP_ID.into()),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some(WINDOW_TITLE.into()),
            ..Default::default()
        }),
        // Inactive windows draw at full rate, as the old app's did, instead of GPUI's 30 frames a second (the
        // maintainer decided this on 2026-11-13); TODO.md revisits the cost to battery while a background window shows
        // long agent output.
        inactive_frame_interval: None,
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

/// What the process that embeds the GUI hands it: how the process asks the app to quit, and what to do while it does.
///
/// The GUI does not own the process. The desktop binary started the helm the GUI talks to and owns its shutdown, and it
/// turns operating-system requests (SIGTERM, Ctrl+C) into a quit; the GUI only runs the event loop in between.
pub struct Host {
    /// Resolves when the process wants the app to quit. The app then quits the ordinary way, running [`Self::on_quit`].
    /// A future that never resolves is fine: the app then quits only when its user quits it.
    pub quit_requested: Pin<Box<dyn Future<Output = ()>>>,
    /// Runs once while the app quits, on the main thread, inside GPUI's quit deadline (`gpui::SHUTDOWN_TIMEOUT`,
    /// 200 ms), so it may only start slow work, never wait for it. On macOS the process exits right after the quit
    /// handlers, so anything still running then dies with it; on Linux [`run`] returns and the caller can finish the
    /// work.
    pub on_quit: Box<dyn FnOnce()>,
    /// Runs, on the main thread, when the app cannot start: installing app-wide state or opening the main window
    /// failed, so there is no window to show the error in. It receives the error's full description. The host decides
    /// how a windowless failure reaches the user (the desktop binary shows its native refusal and exits); if it
    /// returns, the app quits.
    pub on_startup_failure: Box<dyn FnOnce(String)>,
    /// Which controls a plain Tab reaches on this platform: the system's Keyboard navigation setting on macOS, every
    /// control elsewhere (decided on 2026-11-13 for the GPUI migration: the Mac keeps the old app's behavior, and the
    /// Linux app follows the GTK/Qt convention because Linux has no such setting). Asked on every Tab and Shift+Tab
    /// press, on the main thread; it must answer quickly and never fail (a setting it cannot read answers the
    /// platform's default).
    pub tab_reach: Box<dyn Fn() -> focus::TabReach>,
    /// The helm this window talks to: the embedded helm's address and the device credential it minted for this
    /// process. `None` opens no channel, so the window stays at "Connecting to the helm…" (useful only in tests).
    pub helm: Option<farhelm_helm_client::HelmEndpoint>,
    /// The operating system's reduced-motion setting, read by the host at startup: the GUI does not query the platform
    /// itself (see the ticker).
    pub reduced_motion: bool,
}

/// What keeps the app's quit wiring alive for the life of the app: the task waiting on [`Host::quit_requested`] and the
/// quit handler's registration. Dropping either would silently cancel it, so the app holds both as a global.
struct QuitWiring {
    _requested: Task<()>,
    _on_quit: Subscription,
}

impl Global for QuitWiring {}

/// Run the desktop GUI until it quits. The desktop binary calls this after it has started the helm it embeds.
///
/// On Linux this returns once the app has quit (by default when its last window closes). On macOS it never returns:
/// quitting ends the process.
pub fn run(host: Host) {
    let Host {
        quit_requested,
        on_quit,
        on_startup_failure,
        tab_reach,
        helm,
        reduced_motion,
    } = host;
    let tab_reach: focus::TabReachSource = std::rc::Rc::from(tab_reach);
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            // First, so a run under the bridge can still be inspected when the window fails to open. A no-op unless
            // the app was launched by the bridge's command line (it reads `GPUI_AGENT_SOCKET`).
            #[cfg(feature = "agent")]
            gpui_agent::init(cx);
            let mut on_quit = Some(on_quit);
            let on_quit = cx.on_app_quit(move |_| {
                if let Some(on_quit) = on_quit.take() {
                    on_quit();
                }
                async {}
            });
            let requested = cx.spawn(async move |cx| {
                quit_requested.await;
                cx.update(|cx| cx.quit());
            });
            cx.set_global(QuitWiring {
                _requested: requested,
                _on_quit: on_quit,
            });
            let started = install(cx).and_then(|()| {
                focus::follow_tab_reach(tab_reach, cx);
                ticker::set_reduced_motion(reduced_motion, cx);
                // Headless Mutter reports every window inactive forever, and the bridge's runs are where an agent
                // watches the app animate, so agent builds count windows as active (SPEC_impl.md, "GPUI cookbook").
                #[cfg(feature = "agent")]
                ticker::set_active_override(Some(true), cx);
                if let Some(endpoint) = helm {
                    session::connect(endpoint, cx)?;
                }
                open_main_window(cx).map(|_| ())
            });
            if let Err(error) = started {
                tracing::error!("the window could not open: {error:#}");
                on_startup_failure(format!("{error:#}"));
                cx.quit();
            }
        });
}

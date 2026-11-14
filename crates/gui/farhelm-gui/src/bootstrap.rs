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
//! the render counter. Last come the desktop integration's menus, after every key binding exists, because a Mac menu
//! reads the shortcuts it shows once, when it is set. Then the coordinator starts following the host's answer for which
//! controls Tab reaches, the ticker takes the host's reduced-motion setting (and, in an agent build, counts every window
//! as active, so a run under headless Mutter, which reports every window inactive, still animates), the desktop
//! integration keeps the host's source of the license notices, and the UI channel to the helm opens, so the window's
//! first frame already reflects where the connection stands. Only then does a window open, at the frame the host
//! restored if it restored one; once it is open, the window is handed to the desktop integration, which makes its close
//! quit the app and tells the host about every change to its frame.
//!
//! The desktop integration ([`crate::desktop`]) is area code. The bootstrap only calls into it at those points and
//! passes the host's values through untouched, reading no field of the desktop module's types, so the area fills in
//! menus, quitting on close, the saved frame, the title band and the notices without changing this file.
//!
//! Nothing here may panic: on macOS this runs inside an AppKit callback, where a panic aborts the process. Failures are
//! returned, and [`run`] hands them to the host's [`Host::on_startup_failure`], which owns how the process reports them.

use std::future::Future;
use std::pin::Pin;

use gpui::{
    App, AppContext as _, Global, Size, Subscription, Task, WindowBounds, WindowHandle,
    WindowOptions, px, size,
};
use gpui_component::Root;

use crate::assets::{self, Assets};
use crate::main_view::MainView;
use crate::render_count;
use crate::theme::{self, Preference};
use crate::{actions, desktop, focus, layers, local_state, notices, session, ticker};

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
    // Every launch starts at System, the old app's default, until the first snapshot brings the choice stored in the
    // helm's preferences, which the main view then follows (`settings::follow_stored_appearance`). An agent run may
    // force light or dark instead, and then keeps it for the whole run (see `starting_preference`).
    theme::install(starting_preference(cx), cx.window_appearance(), cx);
    actions::install(cx);
    focus::install(cx);
    layers::install(cx);
    ticker::install(cx);
    notices::install(cx);
    local_state::install(cx);
    render_count::install(cx);
    // Last: see the module docs.
    desktop::install_menus(cx);
    Ok(())
}

// ---- Reading what the bootstrap installed ----------------------------------------------------------------------------

/// The app-wide state [`install`] put in place as `T`, for a foundation reading its own global: `None` only when the
/// bootstrap did not install it, which is a bug in the bootstrap or in a test's setup, never a state the app runs in.
///
/// GPUI's own `global` panics on a missing global, and on macOS that panic would abort the app inside an AppKit
/// callback, so a foundation never calls it on its own state. This is the one place that decides what a missing global
/// does instead: a debug build (every test, and every agent build) fails at once with an assertion naming the global,
/// so an omission fails the first test that reaches it rather than quietly turning a feature into one that does
/// nothing; a release build logs an error and the caller carries on without the global, doing what its doc says it
/// does then.
///
/// Not for state that is optional by design (the helm connection, which some tests never start; agent startup
/// overrides, which only agent builds install; state a module creates on first use): those keep their own lookups.
pub(crate) fn installed<T: Global>(cx: &App) -> Option<&T> {
    let global = cx.try_global::<T>();
    report_if_missing::<T>(global.is_some());
    global
}

/// [`installed`], for a foundation that changes its own global.
pub(crate) fn installed_mut<T: Global>(cx: &mut App) -> Option<&mut T> {
    let present = cx.has_global::<T>();
    report_if_missing::<T>(present);
    if present {
        Some(cx.global_mut::<T>())
    } else {
        None
    }
}

/// Report a global the bootstrap should have installed, unless it is `present`: an error in the log always, and a
/// failed assertion in a debug build.
fn report_if_missing<T>(present: bool) {
    let name = std::any::type_name::<T>();
    if !present {
        tracing::error!("{name} is missing: the bootstrap did not install it before it was read");
    }
    debug_assert!(
        present,
        "{name} is missing: the bootstrap did not install it before it was read"
    );
}

/// The appearance preference a launch starts with: System, the old app's default, unless an agent run forced one
/// ([`crate::agent_startup`]). Only agent builds install overrides from the environment, so every other launch starts
/// at System until the first snapshot brings the stored choice; a test may install overrides of its own.
fn starting_preference(cx: &App) -> Preference {
    crate::agent_startup::current(cx)
        .preference
        .unwrap_or(Preference::System)
}

/// The size a first launch asks for before fitting it to the display: [`DEFAULT_WINDOW_SIZE`], unless an agent run
/// asked for another ([`crate::agent_startup`]), as with `starting_preference`.
fn first_launch_size(cx: &App) -> Size<gpui::Pixels> {
    crate::agent_startup::current(cx)
        .window_size
        .unwrap_or(DEFAULT_WINDOW_SIZE)
}

/// The main window's options when no saved frame was restored: a first launch, and every launch of a host that keeps
/// no frame. See [`window_options_at`].
pub fn window_options(cx: &App) -> WindowOptions {
    window_options_at(None, cx)
}

/// The main window's options: title, app identity, minimum size, the desktop integration's chrome, and where it opens.
///
/// It opens at `restored`, the frame the host restored ([`desktop::restore_frame`]), maximized or not as that says.
/// Without one it opens at the first-launch size centered on the display. An agent run's window size wins over a
/// restored frame: a screenshot batch labels each capture with the size it asked for, and the machine's saved frame
/// must not change that.
pub fn window_options_at(restored: Option<WindowBounds>, cx: &App) -> WindowOptions {
    let agent_size = crate::agent_startup::current(cx).window_size;
    let window_bounds = match restored {
        Some(restored) if agent_size.is_none() => restored,
        _ => WindowBounds::centered(first_launch_fitted_size(cx), cx),
    };
    let mut options = WindowOptions {
        window_bounds: Some(window_bounds),
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
    };
    desktop::dress_window(&mut options, cx);
    options
}

/// The first-launch size fitted to the primary display: never larger than the display, so a first launch on a small
/// screen gets the whole screen, not an overflowing frame.
fn first_launch_fitted_size(cx: &App) -> Size<gpui::Pixels> {
    let wanted = first_launch_size(cx);
    match cx.primary_display() {
        Some(display) => {
            let screen = display.bounds().size;
            size(
                wanted.width.min(screen.width),
                wanted.height.min(screen.height),
            )
        }
        None => wanted,
    }
}

/// Open the main window, at `restored` if the host restored a frame ([`window_options_at`]): Kit's `Root` around
/// Farhelm's main view.
///
/// The builder closure only builds views. Anything that changes app state or redraws (here, matching the theme to the
/// window's appearance) runs once the window exists: work inside `open_window`'s builder is a cookbook trap.
pub fn open_main_window(
    restored: Option<WindowBounds>,
    cx: &mut App,
) -> anyhow::Result<WindowHandle<Root>> {
    let handle = cx.open_window(window_options_at(restored, cx), |window, cx| {
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
    /// Builds the license notices that must travel with the binary, as the "Open-Source Notices" window shows them. A
    /// function, not the text, because the text is close to a megabyte and is needed only while that window is open.
    /// Handed to the desktop integration before the window opens.
    pub licenses: fn() -> String,
    /// How the main window's frame is restored at launch and remembered while it changes. `None` opens the window at
    /// the default frame centered on the display every time (tests, and any host with nowhere to save a frame).
    pub window_frame: Option<desktop::FrameHost>,
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
/// On Linux this returns once the app has quit (when its main window closes, among other ways). On macOS it never
/// returns: quitting ends the process.
pub fn run(host: Host) {
    let Host {
        quit_requested,
        on_quit,
        on_startup_failure,
        tab_reach,
        helm,
        reduced_motion,
        licenses,
        window_frame,
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
                desktop::keep_licenses(licenses, cx);
                if let Some(endpoint) = helm {
                    session::connect(endpoint, cx)?;
                }
                let (restored, follow) = desktop::restore_frame(window_frame, cx);
                let main: gpui::AnyWindowHandle = open_main_window(restored, cx)?.into();
                desktop::quit_when_closed(main, cx);
                desktop::follow_frame(main, follow, cx);
                Ok(())
            });
            if let Err(error) = started {
                tracing::error!("the window could not open: {error:#}");
                on_startup_failure(format!("{error:#}"));
                cx.quit();
            }
        });
}

#[cfg(test)]
mod tests {
    use gpui::{Bounds, TestAppContext, point};

    use super::*;
    use crate::agent_startup::Overrides;
    use crate::test_support;

    /// A saved frame that is not centered and not the default size, so a window opened at the first-launch frame can
    /// never be mistaken for it.
    fn saved() -> Bounds<gpui::Pixels> {
        Bounds {
            origin: point(px(40.), px(30.)),
            size: size(px(800.), px(600.)),
        }
    }

    /// The window opens where the host restored it, and in the state it restored: a relaunch that came back centered at
    /// the default size, or unmaximized, would lose the user's arrangement every launch. Specifies that a restored
    /// frame, windowed or maximized, is the main window's opening bounds unchanged.
    #[gpui::test]
    fn a_restored_frame_is_where_the_main_window_opens(cx: &mut TestAppContext) {
        test_support::install(cx);
        cx.update(|cx| {
            // The premise: no agent window size, which would win over the restored frame.
            assert_eq!(crate::agent_startup::current(cx).window_size, None);
            for restored in [
                WindowBounds::Windowed(saved()),
                WindowBounds::Maximized(saved()),
            ] {
                let options = window_options_at(Some(restored), cx);
                assert_eq!(options.window_bounds, Some(restored));
            }
        });
    }

    /// A screenshot batch labels each capture with the window size it asked for, so a frame the machine saved from
    /// an earlier run must not replace it. Specifies that an agent run's window size wins over a restored frame, and
    /// opens centered at that size as it does without one.
    #[gpui::test]
    fn an_agent_window_size_wins_over_a_restored_frame(cx: &mut TestAppContext) {
        test_support::install(cx);
        cx.update(|cx| {
            let narrow = size(px(900.), px(700.));
            cx.set_global(Overrides {
                window_size: Some(narrow),
                preference: None,
            });
            let options = window_options_at(Some(WindowBounds::Maximized(saved())), cx);
            assert_eq!(
                options.window_bounds,
                Some(WindowBounds::Windowed(Bounds::centered(None, narrow, cx)))
            );
        });
    }
}

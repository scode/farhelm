//! The Mac menu bar: the app menu and Edit, with no Close Window, ported from the old app's `desktop/menu.rs` at
//! reference commit `24bc0580`.
//!
//! The old app rebuilt its toolkit's default menu without Close Window, because that item hard-wires Cmd+W and macOS
//! hands a menu's key equivalent the key before the page sees it. Cmd+W belongs to the open session's Delete (SPEC.md,
//! "Desktop window chrome": "The Mac app's Window menu has no Close Window item, leaving Cmd+W to session deletion; the
//! window close button and Cmd+Q still close the app"). GPUI offers every key to the focused element's bindings before
//! the menu, so here the risk is a later change adding a close item or a Cmd+W binding of its own; the tests guard the
//! whole description, as the old test did.
//!
//! The arrangement is the old one. Its first submenu, titled "Window", is what macOS shows as the app menu (AppKit
//! takes the first menu as the application menu whatever its title), and GPUI also makes a menu titled "Window" the
//! app's windows menu, as the old app did with that same submenu, so AppKit lists the open windows at its foot. Edit
//! dispatches the text field's own editing actions ([`EditCommand`]), so it acts on a focused text field; elsewhere its
//! items are disabled, except Copy, which GPUI Kit's window root also handles and which does nothing there. The old
//! debug-only Help menu held the web view's developer tools and Float on Top, which went with the web view; it is not
//! ported. The one item the old menu lacked is Open-Source Notices, above Quit, which opens the notices window
//! (`licenses`; the maintainer decided on 2026-11-13 that the notices live there).
//!
//! Every item dispatches a declared action (SPEC_impl.md, "Actions and keymap"): the window and app commands are
//! Farhelm's (`actions.rs`), handled app-wide here because a menu command has no element of its own; their chords are
//! bound by the keymap before this installs the menu, because `set_menus` reads the keymap once (SPEC_impl.md, "GPUI
//! cookbook: traps").
//!
//! Linux gets no menu bar at all: GPUI draws none there and the app has no in-window menu besides the `?` menu (decided
//! by the migration's orchestrator on 2026-11-13). The description is built on every platform so the tests, which run
//! on GPUI's fake platform under Linux, check exactly what macOS installs.

use gpui::{App, Menu, MenuItem, OsAction};

use super::licenses;
use crate::actions::{
    HideApp, HideOtherApps, MinimizeWindow, OpenSourceNotices, Quit, ShowAllApps, ToggleFullScreen,
    ZoomWindow,
};
use crate::bootstrap::WINDOW_TITLE;
use crate::components::EditCommand;

/// The menu bar macOS installs, in the old app's order. Item names follow what the old app's menu toolkit showed on a
/// Mac (muda 0.17's predefined items: "Hide Farhelm", "Zoom", "Quit Farhelm"), with the app named by its window title,
/// since a devbuild runs as a bare binary whose process name is not the product's.
pub(crate) fn menus() -> Vec<Menu> {
    vec![
        Menu::new("Window").items([
            MenuItem::action("Toggle Full Screen", ToggleFullScreen),
            MenuItem::separator(),
            MenuItem::action(format!("Hide {WINDOW_TITLE}"), HideApp),
            MenuItem::action("Hide Others", HideOtherApps),
            MenuItem::action("Show All", ShowAllApps),
            MenuItem::action("Zoom", ZoomWindow),
            MenuItem::action("Minimize", MinimizeWindow),
            MenuItem::separator(),
            MenuItem::action(licenses::TITLE, OpenSourceNotices),
            MenuItem::separator(),
            MenuItem::action(format!("Quit {WINDOW_TITLE}"), Quit),
        ]),
        Menu::new("Edit").items([
            edit("Undo", EditCommand::Undo, OsAction::Undo),
            edit("Redo", EditCommand::Redo, OsAction::Redo),
            MenuItem::separator(),
            edit("Cut", EditCommand::Cut, OsAction::Cut),
            edit("Copy", EditCommand::Copy, OsAction::Copy),
            edit("Paste", EditCommand::Paste, OsAction::Paste),
            MenuItem::separator(),
            edit("Select All", EditCommand::SelectAll, OsAction::SelectAll),
        ]),
    ]
}

/// An Edit item: the text field's own action, tagged with the matching OS action so AppKit treats it as the standard
/// editing command (system panels and services see the usual selectors).
fn edit(name: &'static str, command: EditCommand, os: OsAction) -> MenuItem {
    MenuItem::Action {
        name: name.into(),
        action: command.action(),
        os_action: Some(os),
        checked: false,
        disabled: false,
    }
}

/// Register the menu commands' app-wide handlers and, when `mac` is set, install the menu bar.
///
/// The handlers are registered on every platform, so an action dispatched by a test (or by a future Linux control) is
/// never silently unhandled; only the menu bar itself is Mac-only. `mac` is a parameter rather than a `cfg` so tests on
/// GPUI's fake platform run the Mac installation. Call it after every key binding exists: `set_menus` captures the
/// chords it shows when it runs.
pub(crate) fn install(cx: &mut App, mac: bool) {
    // Quitting the ordinary way runs the host's quit handler once (`bootstrap::Host::on_quit`), as Cmd+Q, the close
    // button and SIGTERM all must (SPEC.md, "Desktop window chrome": Cmd+Q and the close button close the app).
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &HideApp, cx| cx.hide());
    cx.on_action(|_: &HideOtherApps, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAllApps, cx| cx.unhide_other_apps());
    cx.on_action(|_: &MinimizeWindow, cx| {
        with_active_window(cx, |window| window.minimize_window())
    });
    cx.on_action(|_: &ZoomWindow, cx| with_active_window(cx, |window| window.zoom_window()));
    cx.on_action(|_: &ToggleFullScreen, cx| {
        with_active_window(cx, |window| window.toggle_fullscreen())
    });
    // Deferred like the window commands below: the window brought forward may be the very one this was dispatched in.
    cx.on_action(|_: &OpenSourceNotices, cx| cx.defer(licenses::open));
    if mac {
        cx.set_menus(menus());
    }
}

/// Window commands act on the key window, as AppKit's own items do. With none (every window minimized, say) they do
/// nothing; a window that went away meanwhile is not an error.
///
/// Deferred, because GPUI dispatches a menu item's action (and a key's) from inside the active window's own update, and
/// GPUI refuses to update a window from inside its own update (it answers "window not found"): done at once, Zoom,
/// Minimize and Toggle Full Screen did nothing at all from the menu (found on the test Mac, 2026-11-14). A refusal is
/// logged rather than dropped, so another such case shows in the log.
fn with_active_window(cx: &mut App, act: impl FnOnce(&mut gpui::Window) + 'static) {
    cx.defer(move |cx| {
        if let Some(window) = cx.active_window()
            && let Err(error) = window.update(cx, |_, window, _| act(window))
        {
            tracing::debug!(%error, "a window command found no window to act on");
        }
    });
}

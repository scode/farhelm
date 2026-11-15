//! The title bar the Linux window draws for itself where the compositor draws none.
//!
//! A Wayland compositor that does not draw window decorations (GNOME's Mutter, which the headless test runs use) leaves
//! a window with no title bar at all, so it could not be moved, maximized or closed with the pointer. Under such a
//! compositor the window draws a minimal bar of its own: the window's title, then fixed Minimize, Maximize (or Restore)
//! and Close buttons, with a drag on its background and a double-click that maximizes or restores, through the macOS
//! band's own press rule (`title_band::title_bar_row`). Where the compositor draws its title bar (KDE's KWin), this
//! draws nothing. The migration's coordinator settled on 2026-11-13 that the Linux app draws this bar; it is recorded
//! as a deviation from SPEC.md's "the Linux window layout retains its existing appearance", which assumed a
//! compositor's bar. The GPUI prototype's extras (following the desktop's button layout, a right-click window menu) are
//! left out.
//!
//! GPUI asks every Linux window for the compositor's decorations (`dress_window` makes that explicit) and falls back to
//! drawing them itself when the compositor declines, which it reports through `Window::window_decorations`. That answer
//! can come after the window opens, so the views that draw the bar redraw on the window's appearance changes.
//!
//! Its buttons act on their own window directly, not by dispatching the menu's declared Minimize and Zoom actions:
//! those act on the active window, which headless Mutter never reports, and the notices window's bar must act on the
//! notices window whichever window has focus. A drag from the bar, and a click on its buttons, leave keyboard focus
//! where it was, as a compositor's title bar does, though Tab still reaches the buttons. Both were decided with the
//! coordinator on 2026-11-14; the macOS band instead sends focus home, since a press on its background is a press on the
//! page. The
//! bar is inert while a dialog is open, as the band is: the dialog's veil covers it, and the compositor's own keys
//! still move and close the window.

use gpui::{
    AnyElement, App, Decorations, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    Role, SharedString, Size, StatefulInteractiveElement as _, Styled as _, Window, div, px, size,
};

use super::title_band;
use crate::assets::Icon;
use crate::components::{HoverHelp, IconButton};
use crate::theme::{self, metrics};

/// The bar's height: GPUI Kit's Linux title bar height, and the GPUI prototype's.
pub(crate) const HEIGHT: Pixels = px(34.);

/// The width of the border GPUI Kit draws around a client-decorated window, inside its surface (`BORDER_SIZE` in Kit's
/// `window_border.rs`, which Kit keeps private). The compositor's minimum size counts it, so the content-based minimum
/// allows for it. A Kit upgrade that changes it shows in the screenshot batch's frame check.
const KIT_BORDER: f32 = 1.;

/// Whether the window draws the bar: on Linux, when the compositor left the decorations to the window, and not in
/// fullscreen (where GNOME hides its own header bar too). Never on macOS, whose window has the native title bar or the
/// title band.
pub(crate) fn shows(decorations: Decorations, fullscreen: bool) -> bool {
    cfg!(target_os = "linux") && matches!(decorations, Decorations::Client { .. }) && !fullscreen
}

/// The shadow margin GPUI Kit leaves around a client-decorated window, inside its surface (`SHADOW_SIZE` in Kit's
/// `window_border.rs`, private there). A window's opening size counts the surface, so it allows for it.
const KIT_SHADOW: f32 = 20.;

/// How much a Linux window's opening size grows so that its content gets the size the app asked for: Kit's shadow
/// margin and border on each side, and the drawn bar. Like [`minimum_allowance`], allowed whatever the compositor does.
pub(crate) fn opening_allowance() -> Size<Pixels> {
    let frame = 2. * (KIT_SHADOW + KIT_BORDER);
    size(px(frame), px(frame) + HEIGHT)
}

/// How much a Linux window's minimum size grows so that its content, not its frame, keeps the app's minimum: Kit's
/// border on each side, and the drawn bar's height. Allowed for whatever the compositor turns out to do, since the
/// minimum is set before the window opens and the decorations are known only after; under a compositor that draws its
/// own bar the window is then that much larger than strictly needed (accepted by the coordinator, 2026-11-14), and both
/// desktops leave the same room for the app.
pub(crate) fn minimum_allowance() -> Size<Pixels> {
    size(px(2. * KIT_BORDER), px(2. * KIT_BORDER) + HEIGHT)
}

/// The bar for `window`, titled `title`, its Close button's hover help `close_help`; `None` where the window does not
/// draw one ([`shows`]).
pub(crate) fn bar(
    id: &'static str,
    title: &'static str,
    close_help: &'static str,
    window: &Window,
    cx: &App,
) -> Option<AnyElement> {
    shows(window.window_decorations(), window.is_fullscreen())
        .then(|| draw(id, title, close_help, window.is_maximized(), cx))
}

/// Redraw `window` whenever its appearance changes, which is how a compositor's late answer about decorations arrives:
/// a view that draws the bar holds this for its window's life, so the bar appears or goes as the answer says.
pub(crate) fn follow_decorations(window: &Window) -> gpui::Subscription {
    window.observe_window_appearance(|window, _| window.refresh())
}

/// What one of the bar's buttons does to its window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Minimize,
    /// Maximize, or restore a maximized window.
    Zoom,
    Close,
}

/// Where tests see the bar's commands: GPUI's test platform cannot minimize or zoom a window, so with this installed a
/// command is recorded instead of performed. Test builds only.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct Commands(pub(crate) Vec<Command>);

#[cfg(test)]
impl gpui::Global for Commands {}

/// Perform `command` on `window` (or, in a test that installed [`Commands`], record it). Closing the main window quits
/// the app through `desktop::quit_when_closed`, the ordinary way; closing the notices window closes only it.
fn perform(command: Command, window: &mut Window, cx: &mut App) {
    #[cfg(test)]
    if cx.has_global::<Commands>() {
        cx.default_global::<Commands>().0.push(command);
        return;
    }
    #[cfg(not(test))]
    let _ = cx;
    match command {
        Command::Minimize => window.minimize_window(),
        Command::Zoom => window.zoom_window(),
        Command::Close => window.remove_window(),
    }
}

/// The bar itself, for a window that is `maximized` or not.
pub(crate) fn draw(
    id: &'static str,
    title: &'static str,
    close_help: &'static str,
    maximized: bool,
    cx: &App,
) -> AnyElement {
    let palette = theme::palette(cx);
    let button =
        |button_id: &'static str, icon: Icon, label: &'static str, help: &'static str, command| {
            IconButton::new(
                button_id,
                icon,
                label,
                HoverHelp::tooltip(help),
                Role::Button,
            )
            // A compositor's title buttons never take focus, so a click on one leaves the terminal typed into.
            .keeps_focus_on_press()
            .on_click(move |_, window, cx| perform(command, window, cx))
        };
    let (zoom_icon, zoom_label, zoom_help) = if maximized {
        (Icon::Restore, "Restore", "Restore the window's size")
    } else {
        (Icon::Maximize, "Maximize", "Maximize the window")
    };
    let row = div()
        .id(id)
        // Tests find the bar by this; it records nothing outside test builds.
        .debug_selector(move || id.to_string())
        .role(Role::Group)
        .aria_label(SharedString::from(title))
        .flex()
        .flex_row()
        .items_center()
        .flex_none()
        .w_full()
        .h(HEIGHT)
        .pl(px(12.))
        .pr(px(4.))
        .gap(px(2.))
        .bg(theme::color(palette.bg_1))
        .border_b_1()
        .border_color(theme::color(palette.border_dim))
        // Registered before the band's press rule, so it runs after it (GPUI runs an element's bubbling listeners
        // last-registered first): the rule decides on the untouched press, and then the press is marked taken, so the
        // focus coordinator leaves keyboard focus where it was instead of sending it home.
        .on_any_mouse_down(|_, window, _| window.prevent_default())
        // The title is background, as a compositor's title is: a press on it drags.
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(metrics::TEXT_MD)
                .text_color(theme::color(palette.fg_1))
                .child(title),
        )
        .child(button(
            "title-bar-minimize",
            Icon::Minimize,
            "Minimize",
            "Minimize the window",
            Command::Minimize,
        ))
        .child(button(
            "title-bar-maximize",
            zoom_icon,
            zoom_label,
            zoom_help,
            Command::Zoom,
        ))
        .child(button(
            "title-bar-close",
            Icon::Close,
            "Close",
            close_help,
            Command::Close,
        ));
    title_band::title_bar_row(row).into_any_element()
}

#[cfg(test)]
mod tests;

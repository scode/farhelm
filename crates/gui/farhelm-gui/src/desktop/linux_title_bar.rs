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
//! The module also says which part of the window holds the app's content once Kit's frame and the bar are drawn
//! ([`content_area`]).
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
    AnyElement, App, Bounds, Decorations, Edges, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Role, SharedString, Size, StatefulInteractiveElement as _,
    Styled as _, Window, div, point, px, size,
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

// ---- Where the content sits -----------------------------------------------------------------------------------------

/// The part of `window` that holds the app's content, in window coordinates: inside the frame GPUI Kit draws around a
/// client-decorated window (its shadow margin and border) and below the bar the root view draws at its top. GPUI's
/// `Window::viewport_size` includes both, so anything sized or placed against it could take room that is not there:
/// a tall host progress card and a dialog at its height limit covered the bar, as settings-and-hosts and templates
/// reported on 2026-11-15, and the expanded host rail took the main pane below its floor under Mutter, where the frame
/// is 42 pixels wide (found by the migration's review gate the same day). The layer manager keeps its dialogs,
/// popovers and tooltips inside this, the notices panels cap their height by it, and the dialog frame, the host
/// progress card and the sidebar's shown width size themselves against it, as the old app's page was the whole of its
/// window. Under a compositor that draws the decorations, and on macOS, it is the whole window.
///
/// It follows the window's state the way Kit's frame and the bar do: a maximized, fullscreen or tiled window has no
/// shadow margin or border on its tiled sides (GPUI reports every side tiled for a fullscreen window, and usually for
/// a maximized one), and a fullscreen one draws no bar ([`bar_height`]).
pub(crate) fn content_area(window: &Window) -> Bounds<Pixels> {
    content_area_in(
        window.viewport_size(),
        window.window_decorations(),
        window.client_inset(),
        bar_height(window),
    )
}

/// The height the bar takes at the top of the root view's box: [`HEIGHT`] where the window draws it ([`shows`]), else
/// nothing. The root view's box already sits inside Kit's frame, so this is the whole of what a box filling the root
/// view (the dialog veil) pads by to cover [`content_area`] alone.
pub(crate) fn bar_height(window: &Window) -> Pixels {
    if shows(window.window_decorations(), window.is_fullscreen()) {
        HEIGHT
    } else {
        px(0.)
    }
}

/// [`content_area`] as a pure function of the surface's size `viewport`, the window's `decorations`, the shadow margin
/// GPUI Kit set as the window's client `inset` (`None` before Kit's frame first drew, when Kit's own 20 pixels apply)
/// and a bar `bar` tall at the top. A window whose decorations are the compositor's has no frame; on a client-decorated
/// one each side has the shadow margin and then the border, except a side the window is tiled against (maximized, or
/// snapped to that edge), where Kit draws neither. Off Linux the windows are never client-decorated (GPUI's macOS
/// backend reports server decorations), which is why this reads the decorations instead of checking the platform: on
/// macOS the area is the whole window. The area never goes below zero wide or tall: a surface narrower than its frame
/// holds no content.
///
/// NOTE: This mirrors `window_content_insets` (and the `client_frame_insets` it builds on) in GPUI Kit's
/// `src/window_border.rs`, gpui-component 0.9.1, which Kit keeps `pub(crate)`. It takes the same inputs that function
/// reads from the window, so a Kit upgrade must be checked against that function: a changed shadow, border or tiling
/// rule there puts dialogs, tooltips and the sidebar's room wrong here, and `tests.rs` pins the rule as of 0.9.1
/// (`the_content_width_leaves_out_kits_frame_on_each_untiled_side` and the content area tests after it). Kit's public
/// `window_paddings` gives the shadow part alone, but not the border: a side it pads by nothing may be tiled (no
/// border) or carry a zero inset (a border), so the tiling is read here in any case.
pub(crate) fn content_area_in(
    viewport: Size<Pixels>,
    decorations: Decorations,
    inset: Option<Pixels>,
    bar: Pixels,
) -> Bounds<Pixels> {
    let frame = match decorations {
        Decorations::Server => Edges::default(),
        Decorations::Client { tiling } => {
            let edge = inset.unwrap_or(px(KIT_SHADOW)) + px(KIT_BORDER);
            let side = |tiled: bool| if tiled { px(0.) } else { edge };
            Edges {
                top: side(tiling.top),
                right: side(tiling.right),
                bottom: side(tiling.bottom),
                left: side(tiling.left),
            }
        }
    };
    let origin = point(frame.left, frame.top + bar);
    let extent = size(
        (viewport.width - frame.left - frame.right).max(px(0.)),
        (viewport.height - frame.top - bar - frame.bottom).max(px(0.)),
    );
    Bounds {
        origin,
        size: extent,
    }
}

// ---- The bar --------------------------------------------------------------------------------------------------------

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

/// Where the bar's commands go instead of to the window, while one is installed: a test installs it, since GPUI's test
/// platform cannot minimize or zoom a window, and the app never does. It exists in every build, so the buttons a test
/// clicks go through the same [`perform`] the app's do, and only the window behind them is swapped
/// (SPEC_impl.md, "GPUI cookbook: traps", on `cfg(test)` branches).
#[derive(Default)]
pub(crate) struct Commands(pub(crate) Vec<Command>);

impl gpui::Global for Commands {}

/// Perform `command` on `window`, or record it while [`Commands`] is installed. Closing the main window quits the app
/// through `desktop::quit_when_closed`, the ordinary way; closing the notices window closes only it.
fn perform(command: Command, window: &mut Window, cx: &mut App) {
    if cx.has_global::<Commands>() {
        cx.default_global::<Commands>().0.push(command);
        return;
    }
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
                HoverHelp::tooltip(help).below(),
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

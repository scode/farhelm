//! Tests of the Linux window's own title bar: when it is drawn, what it holds, what its buttons and its background do,
//! and the sizes that keep the app's content at its minimum. GPUI's test platform reports the compositor's decorations
//! on every window, so the drawing tests draw the bar itself ([`super::draw`]); [`super::shows`] is tested on its own.
//! Window commands and drags are recorded, not performed: the test platform cannot minimize, zoom or move a window.
//! The content area the bar and Kit's frame leave is tested as a function of the window's state
//! ([`super::content_area_in`]), since no test window can report client decorations.

use gpui::{
    Decorations, FocusHandle, IntoElement as _, Modifiers, MouseButton, ParentElement as _, Pixels,
    Point, Styled as _, TestAppContext, Tiling, VisualTestContext, WindowOptions, div, point, px,
    size,
};

use super::{
    Command, Commands, HEIGHT, bar_height, content_area, content_area_in, draw, minimum_allowance,
    opening_allowance, shows,
};
use crate::desktop::title_band::{self, Decisions, PressAction, Row};
use crate::test_support;

/// The bar drawn for a window that is `maximized` or not, beside one other control (for the focus case), with the
/// band's press watcher as the main window draws it, and command and drag recorders installed.
fn open_bar(cx: &mut TestAppContext, maximized: bool) -> (&mut VisualTestContext, FocusHandle) {
    test_support::install(cx);
    let other = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let drawn_other = other.clone();
    let window = test_support::open(cx, move |_, cx| {
        div()
            .size_full()
            .child(draw(
                "test-title-bar",
                "Farhelm",
                "Close Farhelm",
                maximized,
                cx,
            ))
            .child(test_support::button("other", &drawn_other))
            .child(title_band::watch_presses())
            .into_any_element()
    });
    window.update(|_, cx| {
        cx.set_global(Commands::default());
        cx.set_global(Decisions::default());
    });
    (window, other)
}

/// The commands the bar's buttons recorded.
fn commands(cx: &mut VisualTestContext) -> Vec<Command> {
    cx.update(|_, cx| cx.global::<Commands>().0.clone())
}

/// The press decisions the bar's background recorded.
fn decisions(cx: &mut VisualTestContext) -> Vec<(Row, PressAction)> {
    cx.update(|_, cx| cx.global::<Decisions>().0.clone())
}

/// A primary press at `at` that moves a few points before its release when `moving`, as a drag does, `count` the
/// platform's click count.
fn press(cx: &mut VisualTestContext, at: Point<Pixels>, count: usize, moving: bool) {
    cx.simulate_event(gpui::MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: count,
        first_mouse: false,
    });
    if moving {
        cx.simulate_event(gpui::MouseMoveEvent {
            position: at + point(px(6.), px(4.)),
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        });
    }
    cx.simulate_event(gpui::MouseUpEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: count,
    });
    test_support::settle(cx);
}

/// The window draws its own bar only where it must: on Linux when the compositor left the decorations to it, and
/// never in fullscreen; under a compositor that draws its own title bar, and on macOS, a second bar would stack under
/// the native one. Specified: client decorations draw it on Linux and nowhere else; server decorations and fullscreen
/// never draw it.
#[test]
fn the_bar_shows_only_on_linux_without_the_compositors_decorations() {
    let client = Decorations::Client {
        tiling: Tiling::default(),
    };
    assert_eq!(shows(client, false), cfg!(target_os = "linux"));
    assert!(!shows(client, true), "never in fullscreen");
    assert!(
        !shows(Decorations::Server, false),
        "never under the compositor's bar"
    );
}

/// SPEC.md: the window cannot be made smaller than 720 by 560, which is about what the app has room for; on Linux GPUI
/// Kit's frame and the drawn bar sit inside the window, so the minimum and the opening size grow by them, and only on
/// Linux. Specified: the minimum grows by Kit's 1 px border each way and the bar's 34 px; the opening size by Kit's
/// 20 px shadow and 1 px border each way and the bar; the main window's options carry the grown minimum and ask for the
/// compositor's decorations on Linux, and are unchanged on macOS.
#[gpui::test]
fn the_linux_minimum_keeps_the_apps_content_at_its_size(cx: &mut TestAppContext) {
    assert_eq!(minimum_allowance(), size(px(2.), px(2.) + HEIGHT));
    assert_eq!(opening_allowance(), size(px(42.), px(42.) + HEIGHT));
    test_support::install(cx);
    let minimum = size(px(720.), px(560.));
    let mut options = WindowOptions {
        window_min_size: Some(minimum),
        ..Default::default()
    };
    cx.update(|cx| crate::desktop::dress_window(&mut options, cx));
    if cfg!(target_os = "linux") {
        assert_eq!(options.window_min_size, Some(size(px(722.), px(596.))));
        assert_eq!(
            options.window_decorations,
            Some(gpui::WindowDecorations::Server)
        );
    } else {
        assert_eq!(options.window_min_size, Some(minimum));
    }
}

/// Every control needs a label and a role, and the bar's maximize button must say what it will do: restore a maximized
/// window, maximize any other. Specified: the bar is a group named for the window's title, 34 px tall; it holds
/// Minimize, Maximize and Close buttons with their labels and the button role; drawn for a maximized window, the
/// middle one is Restore.
#[gpui::test]
fn the_bar_holds_the_title_and_three_named_buttons(cx: &mut TestAppContext) {
    let label = |cx: &mut VisualTestContext, id: &str| {
        test_support::frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .and_then(|node| node.label.map(|label| label.to_string()))
    };
    let (cx, _) = open_bar(cx, false);
    assert_eq!(label(cx, "test-title-bar").as_deref(), Some("Farhelm"));
    assert_eq!(label(cx, "title-bar-minimize").as_deref(), Some("Minimize"));
    assert_eq!(label(cx, "title-bar-maximize").as_deref(), Some("Maximize"));
    assert_eq!(label(cx, "title-bar-close").as_deref(), Some("Close"));
    let bar = test_support::bounds(cx, "test-title-bar");
    assert_eq!(bar.size.height, HEIGHT);
    for id in [
        "title-bar-minimize",
        "title-bar-maximize",
        "title-bar-close",
    ] {
        let role = test_support::frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .and_then(|node| node.role);
        assert_eq!(role, Some(gpui::Role::Button), "{id}");
    }
}

/// A maximized window's middle button restores its size, and says so. Specified: drawn for a maximized window, the
/// button is labelled Restore.
#[gpui::test]
fn a_maximized_windows_bar_offers_restore(cx: &mut TestAppContext) {
    let (cx, _) = open_bar(cx, true);
    let label = test_support::frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("title-bar-maximize"))
        .and_then(|node| node.label.map(|label| label.to_string()));
    assert_eq!(label.as_deref(), Some("Restore"));
}

/// The drawn bar replaces a compositor's, so its buttons must do what a compositor's do, to this window. Specified: a
/// click on Minimize, Maximize and Close records those commands, in order, and nothing else.
#[gpui::test]
fn the_bars_buttons_minimize_maximize_and_close_their_window(cx: &mut TestAppContext) {
    let (cx, _) = open_bar(cx, false);
    test_support::click(cx, "title-bar-minimize");
    test_support::click(cx, "title-bar-maximize");
    test_support::click(cx, "title-bar-close");
    assert_eq!(
        commands(cx),
        [Command::Minimize, Command::Zoom, Command::Close]
    );
    assert_eq!(
        decisions(cx),
        [(Row::LinuxTitleBar, PressAction::None); 3],
        "each press on a button is decided as nothing: the button keeps it"
    );
    let minimize = test_support::center(cx, "title-bar-minimize");
    press(cx, minimize, 1, true);
    assert!(
        !decisions(cx).contains(&(Row::LinuxTitleBar, PressAction::Drag)),
        "a press on a button that moves never drags"
    );
}

/// Without a compositor's bar, the drawn bar is the only way to move or maximize the window with the pointer, so its
/// background follows the macOS band's press rule, and a drag from it must not take keyboard focus out of what the user
/// was typing in, as a compositor's title bar never does. Specified: a press on the title that moves drags the window;
/// a press without motion does nothing; a double-click there maximizes; and a drag leaves focus on the control that had
/// it.
#[gpui::test]
fn the_bars_background_drags_and_zooms_and_leaves_focus_alone(cx: &mut TestAppContext) {
    let (cx, other) = open_bar(cx, false);
    cx.update(|window, cx| {
        let mark = crate::focus::Mark::take(cx);
        crate::focus::focus_control(&other, &mark, window, cx)
    });
    test_support::settle(cx);
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("other"),
        "the premise: another control holds focus"
    );
    let bar = test_support::bounds(cx, "test-title-bar");
    let title = point(bar.left() + px(30.), bar.center().y);
    press(cx, title, 1, true);
    assert_eq!(decisions(cx), [(Row::LinuxTitleBar, PressAction::Drag)]);
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("other"),
        "a drag leaves focus where it was"
    );
    press(cx, title, 1, false);
    assert_eq!(
        decisions(cx).len(),
        1,
        "a click without motion does nothing"
    );
    press(cx, title, 2, false);
    assert_eq!(
        decisions(cx).last().copied(),
        Some((Row::LinuxTitleBar, PressAction::Zoom))
    ); // After the zoom: any press in between breaks the pair a double-click needs.
    cx.simulate_event(gpui::MouseDownEvent {
        button: MouseButton::Right,
        position: title,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        button: MouseButton::Right,
        position: title,
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    test_support::settle(cx);
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("other"),
        "a right press on the bar leaves focus where it was too"
    );
}

/// A compositor's title buttons never take keyboard focus, so a click on the drawn bar's buttons must leave the terminal
/// (or whatever held focus) typed into: otherwise the next Enter would press Maximize again (granted as a focus
/// exception by the migration's coordinator on 2026-11-14). Specified: with another control focused, a click on each of
/// the three buttons leaves focus on it.
///
/// Linux only, because the promise is. The bar is never drawn on macOS ([`super::shows`]), and there
/// `Button::keeps_focus_on_press` deliberately does nothing, since the Mac's own click rule already focuses no
/// button. The harness runs Linux's click rule on every OS (`test_support::install`), a pairing no Mac build of the
/// app ever has, so on a Mac this test's clicks would focus the buttons and fail it without saying anything about
/// the app.
#[cfg(target_os = "linux")]
#[gpui::test]
fn a_click_on_the_bars_buttons_leaves_focus_where_it_was(cx: &mut TestAppContext) {
    let (cx, other) = open_bar(cx, false);
    cx.update(|window, cx| {
        let mark = crate::focus::Mark::take(cx);
        crate::focus::focus_control(&other, &mark, window, cx)
    });
    test_support::settle(cx);
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("other"),
        "the premise: another control holds focus"
    );
    for id in [
        "title-bar-minimize",
        "title-bar-maximize",
        "title-bar-close",
    ] {
        test_support::click(cx, id);
        assert_eq!(
            test_support::focused_id(cx).as_deref(),
            Some("other"),
            "{id} kept focus where it was"
        );
    }
}

// ---- The content area -----------------------------------------------------------------------------------------------

/// A rectangle from its left, top, width and height.
fn rect(left: f32, top: f32, width: f32, height: f32) -> gpui::Bounds<Pixels> {
    gpui::Bounds::new(point(px(left), px(top)), size(px(width), px(height)))
}

/// The sidebar sizes itself against the window's content, and a frame counted as content gave it 42 pixels that were
/// not there: under Mutter the expanded rail took the main pane below its 320-pixel floor (the migration's review gate,
/// 2026-11-15). GPUI's test platform reports the compositor's decorations on every window, so the frame is pinned here
/// as a pure rule, on the content area's width. Specified: a window with the compositor's decorations is all content;
/// a client-decorated one loses Kit's 20-pixel shadow and 1-pixel border on each side (a 762-pixel surface holds 720
/// pixels of content), or the shadow margin Kit set instead of its default; a side tiled against the screen's edge
/// loses neither, so a maximized window is all content; and nothing goes below zero.
#[test]
fn the_content_width_leaves_out_kits_frame_on_each_untiled_side() {
    let tiled = |left, right| Decorations::Client {
        tiling: Tiling {
            top: false,
            left,
            right,
            bottom: false,
        },
    };
    let width = |surface: f32, decorations, inset| {
        content_area_in(size(px(surface), px(600.)), decorations, inset, px(0.))
            .size
            .width
    };
    assert_eq!(width(762., Decorations::Server, None), px(762.));
    assert_eq!(width(762., tiled(false, false), None), px(720.));
    assert_eq!(width(762., tiled(false, false), Some(px(10.))), px(740.));
    assert_eq!(width(762., tiled(true, false), None), px(741.));
    assert_eq!(
        width(
            762.,
            Decorations::Client {
                tiling: Tiling::tiled()
            },
            None
        ),
        px(762.)
    );
    assert_eq!(width(30., tiled(false, false), None), px(0.));
}

/// Why this matters: the layer manager fits dialogs and tooltips, and the dialog frame sizes itself, into this
/// rectangle. Were it the whole window, a tall card or a dialog at its height limit would cover the Linux window's own
/// title bar and reach into the shadow GPUI Kit draws around the window, as a host progress card and the launcher did
/// on 2026-11-15.
///
/// Specification, for the 1642 by 1076 window those reports measured, with client decorations and the bar drawn: the
/// content area is the window less Kit's 20 px shadow margin and 1 px border on each side and the 34 px bar at the top,
/// (21, 55, 1600 by 1000), whose center row is 555. The same holds before Kit has reported its margin (Kit's 20 px is
/// assumed), and a margin Kit reports otherwise is used (10 px gives an 11 px frame); a zero margin still leaves the
/// border. A surface too short for its frame and the bar holds nothing, rather than a negative height.
#[test]
fn the_content_area_lies_inside_kits_frame_and_below_the_bar() {
    let viewport = size(px(1642.), px(1076.));
    let client = Decorations::Client {
        tiling: Tiling::default(),
    };
    let area = content_area_in(viewport, client, Some(px(20.)), HEIGHT);
    assert_eq!(area, rect(21., 55., 1600., 1000.));
    assert_eq!(area.center().y, px(555.));
    assert_eq!(
        content_area_in(viewport, client, None, HEIGHT),
        area,
        "Kit's own margin until it reports one"
    );
    assert_eq!(
        content_area_in(viewport, client, Some(px(10.)), HEIGHT),
        rect(11., 45., 1620., 1020.)
    );
    assert_eq!(
        content_area_in(viewport, client, Some(px(0.)), HEIGHT),
        rect(1., 35., 1640., 1040.)
    );
    assert_eq!(
        content_area_in(size(px(1642.), px(50.)), client, None, HEIGHT).size,
        size(px(1600.), px(0.))
    );
}

/// Why this matters: Kit draws no shadow margin or border on a side the window is tiled against, and GPUI reports a
/// fullscreen window, and usually a maximized one, as tiled on every side, so an area that kept the frame there would
/// leave a 21 px gap along the screen's edge; and a fullscreen window draws no bar, so the area must not keep its room
/// either.
///
/// Specification, in a 1000 by 800 window with client decorations: tiled on every side (maximized) with the bar, the
/// content area is the whole window less the 34 px at the top; tiled only at the top and left (snapped to a corner),
/// the frame stays on the right and bottom alone; with no bar (fullscreen, which is also tiled), it is the whole
/// window.
#[test]
fn the_content_area_drops_the_frame_on_tiled_sides_and_the_bar_in_fullscreen() {
    let viewport = size(px(1000.), px(800.));
    let tiled = |top, right, bottom, left| Decorations::Client {
        tiling: Tiling {
            top,
            right,
            bottom,
            left,
        },
    };
    let all = tiled(true, true, true, true);
    assert_eq!(
        content_area_in(viewport, all, Some(px(20.)), HEIGHT),
        rect(0., 34., 1000., 766.)
    );
    assert_eq!(
        content_area_in(
            viewport,
            tiled(true, false, false, true),
            Some(px(20.)),
            HEIGHT
        ),
        rect(0., 34., 979., 745.)
    );
    assert_eq!(
        content_area_in(viewport, all, Some(px(20.)), px(0.)),
        rect(0., 0., 1000., 800.)
    );
}

/// Why this matters: under a compositor that draws the decorations, and on macOS (which never reports client
/// decorations, and whose window has the native title bar or the title band), nothing of the window is the app's
/// chrome, so placement must stay exactly what it was before the content area existed.
///
/// Specification: with server decorations and no bar, the content area is the whole 1000 by 800 window; and a window
/// on GPUI's test platform, which reports server decorations as both platforms' compositor-decorated windows do, gets
/// its whole viewport from [`content_area`] and no bar height from [`bar_height`].
#[gpui::test]
fn without_client_decorations_the_content_area_is_the_whole_window(cx: &mut TestAppContext) {
    let viewport = size(px(1000.), px(800.));
    assert_eq!(
        content_area_in(viewport, Decorations::Server, None, px(0.)),
        rect(0., 0., 1000., 800.)
    );
    let window = test_support::open(cx, |_, _| div().size_full().into_any_element());
    window.simulate_resize(viewport);
    window.run_until_parked();
    let (area, bar, viewport) = window.update(|window, _| {
        (
            content_area(window),
            bar_height(window),
            window.viewport_size(),
        )
    });
    assert_eq!(area, gpui::Bounds::new(point(px(0.), px(0.)), viewport));
    assert_eq!(bar, px(0.));
}

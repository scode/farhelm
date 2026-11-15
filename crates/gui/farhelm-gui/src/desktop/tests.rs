//! Tests of the desktop module: the Mac menu bar's description and its commands, and the main window's close quitting
//! the app.
//!
//! They run on GPUI's fake platform, under Linux too: the menu bar is installed with `mac` set, which records the menus
//! exactly as macOS would receive them. What only a real macOS window shows (the bar itself, AppKit's handling of the
//! window list, a real quit) is checked on the test Mac, recorded in the parity file.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{Action as _, Keystroke, OsAction, OwnedMenuItem, TestAppContext, div};

use super::{
    FrameFacts, FrameHost, FrameRect, Observed, Placement, follow_frame, menu, restore_frame,
    when_closed,
};
use crate::actions::{
    DeleteSession, HideApp, HideOtherApps, MinimizeWindow, Quit, ShowAllApps, TEXT_INPUT_CONTEXT,
    ToggleFullScreen, ZoomWindow,
};
use crate::test_support;

/// One item as the platform received it: a separator, or an item's name, its action's name and its OS action.
#[derive(Debug, PartialEq)]
enum Item {
    Separator,
    Action(String, &'static str, Option<&'static str>),
}

/// An OS action by name, for comparing and printing (GPUI's type has neither `Debug` nor a name of its own).
fn os_name(os: Option<OsAction>) -> Option<&'static str> {
    os.map(|os| match os {
        OsAction::Cut => "cut",
        OsAction::Copy => "copy",
        OsAction::Paste => "paste",
        OsAction::SelectAll => "select all",
        OsAction::Undo => "undo",
        OsAction::Redo => "redo",
    })
}

/// The menus the fake platform recorded, as `(title, items)`.
fn installed_menus(cx: &mut TestAppContext) -> Vec<(String, Vec<Item>)> {
    let Some(menus) = cx.update(|cx| cx.get_menus()) else {
        panic!("the fake platform keeps the menus it was given");
    };
    menus
        .into_iter()
        .map(|menu| {
            let items = menu
                .items
                .into_iter()
                .map(|item| match item {
                    OwnedMenuItem::Separator => Item::Separator,
                    OwnedMenuItem::Action {
                        name,
                        action,
                        os_action,
                        ..
                    } => Item::Action(name, action.name(), os_name(os_action)),
                    _ => panic!("a submenu or system menu, which the description never makes"),
                })
                .collect();
            (menu.name.to_string(), items)
        })
        .collect()
}

/// An expected item: its name, its action and its OS action.
fn item(name: &str, action: &'static str, os: Option<OsAction>) -> Item {
    Item::Action(name.to_string(), action, os_name(os))
}

/// Cmd+W must stay the open session's Delete and never become Close Window (SPEC.md, "Desktop window chrome"), and the
/// rest of the menu bar is the old app's (`desktop/menu.rs` at reference commit `24bc0580`, whose own test guarded the
/// same description). Restoring a close item, or binding Cmd+W to anything but Delete, would let Cmd+W close the only
/// window, which quits the app. So: the whole bar as installed, in order, with each item's action; no item named Close;
/// and every binding of Cmd+W in the app's keymap (Kit's and Farhelm's, as at startup) runs Delete. The Edit items
/// dispatch the text field's own actions, which a focused field binds in its own key context: that is what makes them
/// act on a field (and Copy, which GPUI Kit's window root also handles, nowhere it could copy anything else).
#[gpui::test]
fn the_mac_menu_is_the_old_apps_with_no_close_window(cx: &mut TestAppContext) {
    test_support::install(cx);
    cx.update(|cx| menu::install(cx, true));
    let edit = |name: &str, action: &'static str, os| item(name, action, Some(os));
    assert_eq!(
        installed_menus(cx),
        [
            (
                "Window".to_string(),
                vec![
                    item("Toggle Full Screen", ToggleFullScreen.name(), None),
                    Item::Separator,
                    item("Hide Farhelm", HideApp.name(), None),
                    item("Hide Others", HideOtherApps.name(), None),
                    item("Show All", ShowAllApps.name(), None),
                    item("Zoom", ZoomWindow.name(), None),
                    item("Minimize", MinimizeWindow.name(), None),
                    Item::Separator,
                    item("Quit Farhelm", Quit.name(), None),
                ],
            ),
            (
                "Edit".to_string(),
                vec![
                    edit("Undo", "input::Undo", OsAction::Undo),
                    edit("Redo", "input::Redo", OsAction::Redo),
                    Item::Separator,
                    edit("Cut", "input::Cut", OsAction::Cut),
                    edit("Copy", "input::Copy", OsAction::Copy),
                    edit("Paste", "input::Paste", OsAction::Paste),
                    Item::Separator,
                    edit("Select All", "input::SelectAll", OsAction::SelectAll),
                ],
            ),
        ]
    );
    let menus = installed_menus(cx);
    let cmd_w = match Keystroke::parse("cmd-w") {
        Ok(keystroke) => keystroke,
        Err(error) => panic!("cmd-w does not parse: {error}"),
    };
    let edit_actions: Vec<&'static str> = menus[1]
        .1
        .iter()
        .filter_map(|found| match found {
            Item::Action(_, action, _) => Some(*action),
            Item::Separator => None,
        })
        .collect();
    cx.update(|cx| {
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        let mut cmd_w_bindings = 0;
        for binding in keymap.bindings() {
            let keys = binding.keystrokes();
            if keys.len() == 1 && keys[0].inner() == &cmd_w {
                cmd_w_bindings += 1;
                assert_eq!(
                    binding.action().name(),
                    DeleteSession.name(),
                    "Cmd+W must reach the open session's Delete, not another action"
                );
            }
        }
        // Cmd+W is bound only on macOS (Linux's Delete chord is Ctrl+Shift+Backspace); there the loop must have seen
        // it, or it proved nothing. On Linux the Mac chord table's own test covers the chords the menu shows.
        if cfg!(target_os = "macos") {
            assert!(cmd_w_bindings > 0, "the premise: Delete is bound to Cmd+W");
        }
        for action in &edit_actions {
            assert!(
                keymap.bindings().any(|binding| {
                    binding.action().name() == *action
                        && binding
                            .predicate()
                            .is_some_and(|predicate| predicate.to_string() == TEXT_INPUT_CONTEXT)
                }),
                "a focused text field handles {action} itself, so the Edit item acts on it"
            );
        }
    });
}

/// Every app or window command in the menu has an app-wide handler, so its item is enabled whatever holds focus and a
/// click on it is never silently dropped. The Edit items are enabled exactly where their action is handled, which is
/// what GPUI asks before macOS enables an item: all six in a focused text field, and elsewhere only Copy, which GPUI
/// Kit's window root also handles (copying Kit's own selected text, of which Farhelm draws none). Asked in a real
/// window wrapped in Kit's root, with the field focused and then a control.
#[gpui::test]
fn the_menus_commands_are_handled_app_wide_and_the_edit_items_by_a_focused_field(
    cx: &mut TestAppContext,
) {
    use crate::components::{Button, HoverHelp, Role, TextInput, TextInputState};
    use gpui::{InteractiveElement as _, IntoElement as _, ParentElement as _, Styled as _, px};
    let field: Rc<std::cell::RefCell<Option<TextInputState>>> = Rc::default();
    let button = Rc::new(std::cell::RefCell::new(None));
    let (kept, kept_button) = (field.clone(), button.clone());
    let cx = test_support::open(cx, move |window, cx| {
        let state = kept
            .borrow_mut()
            .get_or_insert_with(|| TextInputState::new(window, cx))
            .clone();
        let handle = kept_button
            .borrow_mut()
            .get_or_insert_with(|| cx.focus_handle().tab_stop(true))
            .clone();
        div()
            .child(
                div()
                    .debug_selector(|| "field-box".into())
                    .w(px(120.))
                    .child(TextInput::new(
                        "field",
                        &state,
                        "Filter",
                        HoverHelp::tooltip("A test field"),
                    )),
            )
            .child(
                Button::new(
                    "control",
                    "Control",
                    HoverHelp::tooltip("A control"),
                    Role::Button,
                )
                .focus(&handle),
            )
            .into_any_element()
    });
    let available = |cx: &mut gpui::VisualTestContext| -> Vec<(String, bool)> {
        menu::menus()
            .into_iter()
            .flat_map(|menu| {
                menu.items
                    .into_iter()
                    .filter_map(move |item| match item {
                        gpui::MenuItem::Action { name, action, .. } => {
                            Some((format!("{} > {name}", menu.name), action))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
            .map(|(name, action)| {
                // The app's question, not the window's: it counts the app-wide handlers too, as macOS's does. Asked
                // from the app rather than inside the window's own update, which it enters itself.
                let on = cx.cx.update(|cx| cx.is_action_available(action.as_ref()));
                (name, on)
            })
            .collect()
    };
    test_support::click(cx, "field-box");
    let Some(state) = field.borrow().clone() else {
        panic!("the field was drawn");
    };
    assert!(
        cx.update(|window, cx| state.focus_handle(cx).is_focused(window)),
        "the premise: the text field holds focus"
    );
    for (name, on) in available(cx) {
        assert!(on, "{name} is enabled in a focused text field");
    }
    let Some(handle) = button.borrow().clone() else {
        panic!("the control was drawn");
    };
    cx.update(|window, cx| {
        crate::focus::focus_control(&handle, &crate::focus::Mark::take(cx), window, cx)
    });
    test_support::settle(cx);
    assert!(
        cx.update(|window, _| handle.is_focused(window)),
        "the premise: the control, not the field, holds focus"
    );
    for (name, on) in available(cx) {
        let expected = !name.starts_with("Edit") || name == "Edit > Copy";
        assert_eq!(on, expected, "{name} with a control focused");
    }
}

/// SPEC.md, "Desktop window chrome": the window's close button quits the app. On macOS GPUI would otherwise keep the
/// app running with no window, and on Linux an open second window (the Open-Source Notices window) would. So closing
/// the main window asks for one quit, and closing any other window asks for none. The quit is passed in because GPUI's
/// test platform ignores a real one.
#[gpui::test]
fn closing_the_main_window_quits_and_closing_another_does_not(cx: &mut TestAppContext) {
    test_support::install(cx);
    let (_, main) = test_support::open_view(cx, |_, _| Blank);
    let (_, mut other) = test_support::another_window(main, |_, _| Blank);
    let quits = Rc::new(Cell::new(0));
    let counted = quits.clone();
    main.update(|window, cx| {
        when_closed(
            window.window_handle(),
            move |_| counted.set(counted.get() + 1),
            cx,
        )
    });
    assert_eq!(
        main.update(|_, cx| cx.windows().len()),
        2,
        "the premise: two windows"
    );
    other.update(|window, _| window.remove_window());
    other.run_until_parked();
    assert_eq!(
        main.update(|_, cx| cx.windows().len()),
        1,
        "the other window closed"
    );
    assert_eq!(quits.get(), 0, "another window's close does not quit");
    main.update(|window, _| window.remove_window());
    main.run_until_parked();
    assert_eq!(quits.get(), 1, "the main window's close quits, once");
}

/// An empty root view for the window tests.
struct Blank;

impl gpui::Render for Blank {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        div()
    }
}

/// The window commands must act when they arrive the way a menu click or a key does: GPUI dispatches those from inside
/// the active window's own update, and a handler that updated that window directly did nothing at all (Zoom, Minimize
/// and Toggle Full Screen were dead in the real menu bar). Specified: Toggle Full Screen dispatched through the focused
/// window puts it in full screen, and again takes it out. (Full screen is the one window command GPUI's test platform
/// carries out; the others share the same path.)
#[gpui::test]
fn a_window_command_dispatched_in_the_window_acts_on_it(cx: &mut TestAppContext) {
    test_support::install(cx);
    let (_, window) = test_support::open_view(cx, |_, _| Blank);
    let fullscreen =
        |window: &mut gpui::VisualTestContext| window.update(|window, _| window.is_fullscreen());
    assert!(
        !fullscreen(window),
        "the premise: the window starts windowed"
    );
    assert!(
        window.update(|window, cx| cx.active_window() == Some(window.window_handle())),
        "the premise: it is the active window, which the command acts on"
    );
    window.dispatch_action(ToggleFullScreen);
    test_support::settle(window);
    assert!(fullscreen(window), "Toggle Full Screen entered full screen");
    window.dispatch_action(ToggleFullScreen);
    test_support::settle(window);
    assert!(!fullscreen(window), "and left it again");
}

// ---- The window's frame --------------------------------------------------------------------------------------------

/// What a frame host saw: the facts each call carried and every observation, in order.
#[derive(Default)]
struct HostLog {
    restored_with: Vec<FrameFacts>,
    observed: Vec<Observed>,
    /// How many times the host heard that the window settled.
    settled: usize,
}

/// A frame host that answers `placement` and records everything it is told.
fn recording_host(placement: Option<Placement>) -> (FrameHost, Rc<std::cell::RefCell<HostLog>>) {
    let log = Rc::new(std::cell::RefCell::new(HostLog::default()));
    let (restored, observed, settled) = (log.clone(), log.clone(), log.clone());
    let host = FrameHost {
        restore: Box::new(move |facts| {
            restored.borrow_mut().restored_with.push(facts);
            placement
        }),
        observe: Box::new(move |_, seen| observed.borrow_mut().observed.push(seen)),
        settled: Box::new(move || settled.borrow_mut().settled += 1),
    };
    (host, log)
}

/// SPEC.md, "Desktop window chrome": a launch restores the saved frame, and its maximized state. The host's answer is
/// what the window opens at: its frame as GPUI bounds, maximized or not as it says, and no answer means the window's
/// own default. The host is asked once, before the window opens, with the connected displays (the test platform's one,
/// the one display) and the default size; on Linux, whose app runs only on Wayland, positions are reported untrusted.
#[gpui::test]
fn the_hosts_placement_is_where_the_window_opens(cx: &mut TestAppContext) {
    test_support::install(cx);
    let frame = FrameRect {
        x: 40,
        y: 30,
        width: 900,
        height: 650,
    };
    for maximized in [false, true] {
        let (host, log) = recording_host(Some(Placement { frame, maximized }));
        let (bounds, _) = cx.update(|cx| restore_frame(Some(host), cx));
        let expected = gpui::Bounds {
            origin: gpui::point(gpui::px(40.), gpui::px(30.)),
            size: gpui::size(gpui::px(900.), gpui::px(650.)),
        };
        assert_eq!(
            bounds,
            Some(if maximized {
                gpui::WindowBounds::Maximized(expected)
            } else {
                gpui::WindowBounds::Windowed(expected)
            })
        );
        let log = log.borrow();
        assert_eq!(log.restored_with.len(), 1, "asked once");
        let facts = &log.restored_with[0];
        let primary = cx.update(|cx| cx.primary_display().map(|display| display.bounds()));
        assert_eq!(
            facts
                .displays
                .first()
                .map(|display| (display.width, display.height)),
            primary.map(|bounds| {
                (
                    f32::from(bounds.size.width) as u32,
                    f32::from(bounds.size.height) as u32,
                )
            }),
            "the display comes first"
        );
        assert_eq!(facts.default_size, (1100, 760));
        assert_eq!(facts.positions_trusted, !cfg!(target_os = "linux"));
    }
    let (host, _) = recording_host(None);
    assert_eq!(cx.update(|cx| restore_frame(Some(host), cx)).0, None);
    assert_eq!(
        cx.update(|cx| restore_frame(None, cx)).0,
        None,
        "no host, no restore"
    );
}

/// The host remembers the frame only if it hears about every change: following the main window reports its frame at
/// once (so the host starts from where the window really opened) and again on every resize, as the window's frame
/// (origin and content size) in whole logical pixels, not maximized and not fullscreen here. The host's observer
/// outlives the call that set it up: it is still called on a later resize.
#[gpui::test]
fn following_the_window_reports_its_frame_at_once_and_on_every_change(cx: &mut TestAppContext) {
    test_support::install(cx);
    let (host, log) = recording_host(None);
    let (_, follow) = cx.update(|cx| restore_frame(Some(host), cx));
    let (_, window) = test_support::open_view(cx, |_, _| Blank);
    // The test platform opens a window the size of its display, which on macOS counts as zoomed (`super::fills`), so
    // it is made smaller first: this test is about an ordinary window.
    window.simulate_resize(gpui::size(gpui::px(1100.), gpui::px(760.)));
    test_support::settle(window);
    // From the app, not from inside the window's own update, as the bootstrap calls it once the window is open.
    let handle = window.update(|window, _| window.window_handle());
    window.cx.update(|cx| follow_frame(handle, follow, cx));
    test_support::settle(window);
    let first = log.borrow().observed.first().copied();
    let opened = window.update(|window, _| window.bounds());
    assert_eq!(
        first.map(|seen| (
            seen.frame.width,
            seen.frame.height,
            seen.maximized,
            seen.fullscreen
        )),
        Some((
            f32::from(opened.size.width).round() as u32,
            f32::from(opened.size.height).round() as u32,
            false,
            false
        )),
        "reported at once"
    );
    window.simulate_resize(gpui::size(gpui::px(901.4), gpui::px(700.6)));
    test_support::settle(window);
    let last = log.borrow().observed.last().copied();
    assert_eq!(
        last.map(|seen| (seen.frame.width, seen.frame.height)),
        Some((901, 701)),
        "a resize is reported, rounded to whole pixels"
    );
}

/// The host saves the frame when the window has settled, not on every report: a drag reports on every pixel, and GPUI
/// leaves the quit no time to wait on a disk. So once the window has stopped changing for [`super::SETTLE`], the host
/// hears that it settled; a change before that moves the moment later (a drag is one change), and after it nothing more
/// comes until the window changes again: it is one call, not a loop. The window's first report, when it is followed,
/// counts as a change.
#[gpui::test]
fn the_host_hears_once_that_the_window_settled_after_it_stops_changing(cx: &mut TestAppContext) {
    let settle = super::SETTLE;
    test_support::install(cx);
    let (host, log) = recording_host(None);
    let (_, follow) = cx.update(|cx| restore_frame(Some(host), cx));
    let (_, window) = test_support::open_view(cx, |_, _| Blank);
    let handle = window.update(|window, _| window.window_handle());
    window.cx.update(|cx| follow_frame(handle, follow, cx));
    test_support::settle(window);
    let settled = || log.borrow().settled;
    assert_eq!(settled(), 0, "not before the interval");
    test_support::advance(window, settle);
    assert_eq!(settled(), 1, "the opened window settles once");
    let half = settle / 2;
    window.simulate_resize(gpui::size(gpui::px(800.), gpui::px(600.)));
    test_support::advance(window, half);
    window.simulate_resize(gpui::size(gpui::px(810.), gpui::px(600.)));
    test_support::advance(window, half);
    assert_eq!(
        settled(),
        1,
        "a change before the interval is up moves the moment later"
    );
    test_support::advance(window, half);
    assert_eq!(
        settled(),
        2,
        "once still for the interval after the last change"
    );
    assert_eq!(
        log.borrow().observed.last().map(|seen| seen.frame.width),
        Some(810),
        "after the host was told the last frame"
    );
    test_support::advance(window, settle * 3);
    assert_eq!(settled(), 2, "and nothing more while it stays still");
}

/// The slack that makes a macOS window GPUI zoomed at open count as zoomed ([`super::maximized`]): GPUI's own check
/// wants the window's size to equal the display's visible area exactly, and such a window came out a point short (1024
/// by 681 on a 1024 by 682 area, on the test Mac), so it read as unzoomed and was saved so. Within two points each way
/// fills the area; three points short does not.
#[test]
fn a_window_within_two_points_of_the_visible_area_fills_it() {
    let size = |w: f32, h: f32| gpui::size(gpui::px(w), gpui::px(h));
    let area = size(1024., 682.);
    assert!(super::fills(size(1024., 681.), area));
    assert!(super::fills(size(1022., 684.), area));
    assert!(!super::fills(size(1024., 679.), area));
    assert!(!super::fills(size(760., 580.), area));
}

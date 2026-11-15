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
    title_band, when_closed,
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

// ---- The title band ------------------------------------------------------------------------------------------------

/// The press decision is the band's whole gesture contract (the old `decide_press` table at reference commit
/// `24bc0580`): a primary press on empty background drags, a repeat zooms only with a paired predecessor and never
/// drags, a repeat without one does nothing at all, and presses on what the row's contents claimed, presses with
/// another button and anything in fullscreen do nothing. A wrong row here either moves the window from under a button or
/// leaves the band dead.
#[test]
fn the_press_decision_drags_zooms_or_leaves_the_press_alone() {
    use title_band::{PressAction, decide};
    // Presses with another button never reach `decide`: a row listens for the primary button only, which the scene
    // test below shows.
    for (count, paired) in [(0, false), (1, false), (1, true)] {
        assert_eq!(decide(true, count, paired, false), PressAction::Drag);
    }
    for count in [2, 3, 7] {
        assert_eq!(decide(true, count, true, false), PressAction::Zoom);
        assert_eq!(
            decide(true, count, false, false),
            PressAction::None,
            "a repeat whose partner was elsewhere starts nothing"
        );
    }
    for (count, paired) in [(1, false), (1, true), (2, true), (2, false)] {
        assert_eq!(
            decide(false, count, paired, false),
            PressAction::None,
            "a claimed press keeps its own interaction"
        );
    }
    for (count, paired) in [(1, false), (2, true)] {
        assert_eq!(decide(true, count, paired, true), PressAction::None);
    }
}

use title_band::{PressAction, Row};

/// A window with two band rows side by side (as the app bar and the session top row are), a stand-in for the page
/// outside the band below them, and the window-wide press watcher, wired through the focus coordinator's root as the
/// main view is. The app bar holds a control, a text claiming its presses and an image standing in for the wordmark,
/// with gaps between them.
struct Band {
    root: crate::root::Attachment,
    button: gpui::FocusHandle,
    /// Every decision a band row made, in order.
    decided: Rc<std::cell::RefCell<Vec<(Row, PressAction)>>>,
    /// How many times the control's own click ran.
    clicks: Rc<Cell<usize>>,
}

impl gpui::Render for Band {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use crate::components::{Button, HoverHelp, Role};
        use gpui::{InteractiveElement as _, ParentElement as _, Styled as _, px};
        let row = |id: &'static str, which: Row, decided: Rc<std::cell::RefCell<Vec<_>>>| {
            title_band::band(
                div()
                    .id(id)
                    .debug_selector(move || id.into())
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(40.))
                    .px(px(40.))
                    .h(px(48.))
                    .w(px(400.)),
                which,
                move |action, _, _| decided.borrow_mut().push((which, action)),
            )
        };
        let clicks = self.clicks.clone();
        let app_bar = row("band-app-bar", Row::AppBar, self.decided.clone())
            .child(
                div()
                    .debug_selector(|| "band-wordmark".into())
                    .w(px(60.))
                    .h(px(18.)),
            )
            .child(
                Button::new(
                    "band-button",
                    "Control",
                    HoverHelp::tooltip("A control"),
                    Role::Button,
                )
                .focus(&self.button)
                .on_click(move |_, _, _| clicks.set(clicks.get() + 1)),
            )
            .child(title_band::claims_presses(
                div()
                    .id("band-text")
                    .debug_selector(|| "band-text".into())
                    .child("text"),
            ));
        let top_row = row("band-top-row", Row::SessionTop, self.decided.clone());
        let content = div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().flex().flex_row().child(app_bar).child(top_row))
            .child(
                div()
                    .debug_selector(|| "outside".into())
                    .w(px(400.))
                    .h(px(200.)),
            )
            .child(title_band::watch_presses());
        crate::root::wrap(content, &self.root, window, cx)
    }
}

/// What a band test holds on to.
struct OpenBand<'a> {
    cx: &'a mut gpui::VisualTestContext,
    button: gpui::FocusHandle,
    decided: Rc<std::cell::RefCell<Vec<(Row, PressAction)>>>,
    clicks: Rc<Cell<usize>>,
}

/// Open the band scene through the shared harness.
fn open_band(cx: &mut TestAppContext) -> OpenBand<'_> {
    test_support::install(cx);
    let decided = Rc::new(std::cell::RefCell::new(Vec::new()));
    let clicks = Rc::new(Cell::new(0));
    let (recorded, counted) = (decided.clone(), clicks.clone());
    let (scene, cx) = test_support::open_view(cx, move |window, cx| Band {
        root: crate::root::attach(window, cx),
        button: cx.focus_handle().tab_stop(true),
        decided: recorded,
        clicks: counted,
    });
    let button = cx.update(|_, cx| scene.read(cx).button.clone());
    OpenBand {
        cx,
        button,
        decided,
        clicks,
    }
}

impl OpenBand<'_> {
    /// Press `button` at `at` as the `count`th click of a series, move the pointer a few pixels with the button held
    /// (where a drag starts), release it, and return what the band decided for that press (nothing when no row saw
    /// it, or when nothing started).
    fn press_at(
        &mut self,
        at: gpui::Point<gpui::Pixels>,
        button: gpui::MouseButton,
        count: usize,
    ) -> Option<(Row, PressAction)> {
        self.press_moving(at, button, count, true)
    }

    /// [`Self::press_at`], with the pointer moved between press and release only when `moving`.
    fn press_moving(
        &mut self,
        at: gpui::Point<gpui::Pixels>,
        button: gpui::MouseButton,
        count: usize,
        moving: bool,
    ) -> Option<(Row, PressAction)> {
        let before = self.decided.borrow().len();
        self.cx.simulate_event(gpui::MouseDownEvent {
            button,
            position: at,
            modifiers: gpui::Modifiers::none(),
            click_count: count,
            first_mouse: false,
        });
        if moving {
            self.cx.simulate_event(gpui::MouseMoveEvent {
                position: at + gpui::point(gpui::px(3.), gpui::px(2.)),
                pressed_button: Some(button),
                modifiers: gpui::Modifiers::none(),
            });
        }
        self.cx.simulate_event(gpui::MouseUpEvent {
            button,
            position: at,
            modifiers: gpui::Modifiers::none(),
            click_count: count,
        });
        test_support::settle(self.cx);
        let decided = self.decided.borrow();
        assert!(decided.len() <= before + 1, "one decision per press");
        decided.get(before).copied()
    }

    /// A primary press on the center of what was drawn under `selector`.
    fn press(&mut self, selector: &'static str, count: usize) -> Option<(Row, PressAction)> {
        let at = test_support::center(self.cx, selector);
        self.press_at(at, gpui::MouseButton::Left, count)
    }
}

/// SPEC.md, "Desktop window chrome": empty background is everything in the band but its controls and its text; the
/// wordmark, the gaps and the band's edges count as background. The old acceptance list (`window-chrome.spec.ts` and
/// `click-detail.test.js` at reference commit `24bc0580`): a press on the wordmark, in a gap, on the row's top and
/// bottom edge and at its far end drags; a press on a control keeps the control's own click and does not drag; a press
/// on text does neither. And a press on empty band background is still an empty-space press for the focus rules
/// (SPEC.md, "Keyboard focus"): the band leaves its default alone, so focus leaves the control that held it.
#[gpui::test]
fn presses_on_empty_band_background_drag_and_controls_and_text_keep_theirs(
    cx: &mut TestAppContext,
) {
    let mut band = open_band(cx);
    let drag = Some((Row::AppBar, PressAction::Drag));
    let bar = test_support::bounds(band.cx, "band-app-bar");
    let wordmark = test_support::bounds(band.cx, "band-wordmark");
    let gap = gpui::point(wordmark.right() + gpui::px(20.), bar.center().y);
    assert_eq!(band.press("band-wordmark", 1), drag, "the wordmark");
    assert_eq!(
        band.press_at(gap, gpui::MouseButton::Left, 1),
        drag,
        "a gap"
    );
    for (name, at) in [
        ("the top edge", gpui::point(gap.x, bar.top() + gpui::px(1.))),
        (
            "the bottom edge",
            gpui::point(gap.x, bar.bottom() - gpui::px(1.)),
        ),
        (
            "the far end",
            gpui::point(bar.right() - gpui::px(2.), bar.center().y),
        ),
    ] {
        assert_eq!(
            band.press_at(at, gpui::MouseButton::Left, 1),
            drag,
            "{name}"
        );
    }
    assert_eq!(
        band.press("band-top-row", 1),
        Some((Row::SessionTop, PressAction::Drag)),
        "the other row's empty background"
    );
    assert_eq!(
        band.press("band-button", 1),
        Some((Row::AppBar, PressAction::None))
    );
    assert_eq!(band.clicks.get(), 1, "the control kept its click");
    assert_eq!(
        band.press("band-text", 1),
        Some((Row::AppBar, PressAction::None))
    );
    assert_eq!(
        band.press("outside", 1),
        None,
        "outside the band no row acts"
    );
    band.cx.update(|window, cx| {
        crate::focus::focus_control(&band.button, &crate::focus::Mark::take(cx), window, cx)
    });
    test_support::settle(band.cx);
    assert!(
        band.cx.update(|window, _| band.button.is_focused(window)),
        "the premise: the control holds focus"
    );
    band.press_at(gap, gpui::MouseButton::Left, 1);
    assert!(
        !band.cx.update(|window, _| band.button.is_focused(window)),
        "a press on empty band background is an empty-space press: focus left the control"
    );
}

/// SPEC.md: "a single click without movement never zooms", and a click is not a drag either: the window's move starts
/// only once the pointer moves with the button held, so a plain click on empty band background decides nothing that
/// runs. Starting the move on the press itself was what kept AppKit from ever counting a double-click there (the move
/// took the release). The click still counts as the first click of a pair: a double-click made of two motionless
/// clicks zooms.
#[gpui::test]
fn a_click_without_movement_starts_nothing_and_still_pairs_for_a_zoom(cx: &mut TestAppContext) {
    let mut band = open_band(cx);
    let at = test_support::center(band.cx, "band-wordmark");
    assert_eq!(
        band.press_moving(at, gpui::MouseButton::Left, 1, false),
        None,
        "a click starts nothing"
    );
    assert_eq!(
        band.press_moving(at, gpui::MouseButton::Left, 2, false),
        Some((Row::AppBar, PressAction::Zoom)),
        "its second click zooms"
    );
}

/// A click that wobbles by a pixel with the button held is still a click, not a drag: the move starts only past a
/// small threshold, because once AppKit's window move starts it takes the release and the next press no longer counts
/// as a double-click's second click. Specified: a press moved by one pixel starts nothing, and a second click right
/// after it zooms; a press that leaves its row with its first move (at the row's bottom edge, moving down) still starts
/// the drag, since the move is watched across the whole window, not on the row.
#[gpui::test]
fn a_wobble_is_still_a_click_and_a_drag_that_leaves_the_row_still_starts(cx: &mut TestAppContext) {
    let mut band = open_band(cx);
    let at = test_support::center(band.cx, "band-wordmark");
    let before = band.decided.borrow().len();
    band.cx.simulate_event(gpui::MouseDownEvent {
        button: gpui::MouseButton::Left,
        position: at,
        modifiers: gpui::Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    band.cx.simulate_event(gpui::MouseMoveEvent {
        position: at + gpui::point(gpui::px(1.), gpui::px(0.)),
        pressed_button: Some(gpui::MouseButton::Left),
        modifiers: gpui::Modifiers::none(),
    });
    band.cx.simulate_event(gpui::MouseUpEvent {
        button: gpui::MouseButton::Left,
        position: at,
        modifiers: gpui::Modifiers::none(),
        click_count: 1,
    });
    test_support::settle(band.cx);
    assert_eq!(
        band.decided.borrow().len(),
        before,
        "a one-pixel wobble started nothing"
    );
    assert_eq!(
        band.press_moving(at, gpui::MouseButton::Left, 2, false),
        Some((Row::AppBar, PressAction::Zoom)),
        "and it still pairs for a zoom"
    );
    let bar = test_support::bounds(band.cx, "band-app-bar");
    let edge = gpui::point(at.x, bar.bottom() - gpui::px(1.));
    band.cx.simulate_event(gpui::MouseDownEvent {
        button: gpui::MouseButton::Left,
        position: edge,
        modifiers: gpui::Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    band.cx.simulate_event(gpui::MouseMoveEvent {
        position: edge + gpui::point(gpui::px(0.), gpui::px(10.)),
        pressed_button: Some(gpui::MouseButton::Left),
        modifiers: gpui::Modifiers::none(),
    });
    test_support::settle(band.cx);
    assert_eq!(
        band.decided.borrow().last().copied(),
        Some((Row::AppBar, PressAction::Drag)),
        "a drag whose first move leaves the row starts"
    );
}

/// A double-click on empty band background zooms the window, and repeating it zooms again (restoring the prior frame);
/// no repeat drags, so a triple-click cannot fling the window. A repeat counts only when the press before it was also a
/// primary press on the same row's empty background: after a press on a control or text, outside the band, with another
/// button, or on the other row, the repeat does nothing at all (the old "eligibility", `click-detail.test.js`). That
/// repeat still pairs with the next one, so two background presses in a row always zoom whatever came before them.
#[gpui::test]
fn a_double_click_zooms_only_when_both_presses_were_on_the_same_rows_background(
    cx: &mut TestAppContext,
) {
    let mut band = open_band(cx);
    let (drag, zoom, none) = (
        Some((Row::AppBar, PressAction::Drag)),
        Some((Row::AppBar, PressAction::Zoom)),
        Some((Row::AppBar, PressAction::None)),
    );
    assert_eq!(band.press("band-wordmark", 1), drag);
    assert_eq!(band.press("band-wordmark", 2), zoom);
    assert_eq!(
        band.press("band-wordmark", 3),
        zoom,
        "a triple-click zooms again, never drags"
    );

    assert_eq!(band.press("band-button", 1), none);
    assert_eq!(band.press("band-wordmark", 2), none, "after a control");
    assert_eq!(
        band.press("band-wordmark", 3),
        zoom,
        "the ineligible repeat pairs with the next"
    );

    for (name, interrupt) in [("text", "band-text"), ("outside the band", "outside")] {
        band.press("band-wordmark", 1);
        band.press(interrupt, 1);
        assert_eq!(
            band.press("band-wordmark", 2),
            none,
            "after a press on {name}"
        );
    }
    band.press("band-wordmark", 1);
    let wordmark = test_support::center(band.cx, "band-wordmark");
    band.press_at(wordmark, gpui::MouseButton::Right, 1);
    assert_eq!(
        band.press("band-wordmark", 2),
        none,
        "after a press with another button"
    );
    band.press("band-top-row", 1);
    assert_eq!(
        band.press("band-wordmark", 2),
        none,
        "after a press on the other row"
    );
}

/// The band needs the window to give it the title bar: on macOS the window opens with a transparent title bar, the
/// traffic lights at 12 by 16 points (vertically centered in the 48-point band, where the old app put them) and the
/// app owning the drag, since the band's rows do the dragging. Elsewhere the options keep GPUI's defaults, and in
/// every build inactive windows still draw at full rate (decided by the maintainer on 2026-11-13), which dressing the
/// window must not undo.
#[gpui::test]
fn the_window_options_give_the_band_the_title_bar_only_on_macos(cx: &mut TestAppContext) {
    test_support::install(cx);
    let options = cx.update(|cx| crate::bootstrap::window_options(cx));
    let Some(titlebar) = options.titlebar else {
        panic!("the main window has title bar options");
    };
    assert_eq!(titlebar.appears_transparent, title_band::ON);
    assert_eq!(
        titlebar.traffic_light_position,
        title_band::ON.then_some(title_band::TRAFFIC_LIGHTS)
    );
    assert_eq!(options.app_owns_titlebar_drag, title_band::ON);
    assert_eq!(options.inactive_frame_interval, None);
}

// ---- The band's rows in the macOS window ---------------------------------------------------------------------------

/// The layout half of the band, in the real main view over the helm client's fake. macOS only, because the band
/// exists only there (`title_band::ON`); these run on the test Mac. A press on the real rows starts a native move,
/// which GPUI's test platform cannot make, so presses are the scene tests' above.
#[cfg(target_os = "macos")]
mod mac {
    use farhelm_helm_client::fake::FakeHelm;
    use gpui::{Bounds, Pixels, TestAppContext, VisualTestContext, px, size};

    use super::title_band::{self, PressAction, Row};

    use crate::main_view::MainView;
    use crate::session_list::tests::{listing, session};
    use crate::test_support::{self, frame, started};

    /// The real main view over the fake helm, connected with `sessions` listed (the first one open), in an active
    /// window of the default size. The fake is returned so the test keeps it, and with it the channel, alive.
    fn open<'a>(
        cx: &'a mut TestAppContext,
        sessions: &[&'static str],
    ) -> (FakeHelm, &'a mut VisualTestContext) {
        let (mut fake, _connection) = started(cx);
        let rows: Vec<_> = sessions.iter().map(|key| session(key)).collect();
        fake.connect(vec![listing(sessions, &rows)]);
        cx.run_until_parked();
        let (_, window) = test_support::open_view(cx, MainView::new);
        (fake, window)
    }

    /// Where the element with id `id` was drawn in a fresh frame (GPUI's driver record, which names elements by id).
    fn drawn(cx: &mut VisualTestContext, id: &str) -> Bounds<Pixels> {
        match frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(id))
        {
            Some(node) => node.bounds,
            None => panic!("{id} was not drawn"),
        }
    }

    /// SPEC.md, "Desktop window chrome": the traffic lights sit in the sidebar's top row, and the sidebar's app bar
    /// and the session pane's top row, both 48 pixels, form the title band across the window's top. The old
    /// `window-chrome.spec.ts` checked the same: both rows at the top, both 48 high. The app bar keeps 88 pixels clear
    /// on its left for the lights, so the wordmark starts after them, and gives the room back in native fullscreen,
    /// which hides them.
    #[gpui::test]
    fn the_band_rows_are_48_high_at_the_top_and_the_app_bar_keeps_the_lights_clear(
        cx: &mut TestAppContext,
    ) {
        let (_fake, cx) = open(cx, &["s1"]);
        test_support::settle(cx);
        let bar = test_support::bounds(cx, "app-bar");
        let top = test_support::bounds(cx, "session-header");
        assert_eq!((bar.top(), bar.size.height), (px(0.), px(48.)));
        assert_eq!((top.top(), top.size.height), (px(0.), px(48.)));
        let wordmark = drawn(cx, "app-wordmark");
        assert!(
            wordmark.left() >= px(88.),
            "the wordmark starts after the lights: {wordmark:?}"
        );
        cx.update(|window, _| window.toggle_fullscreen());
        test_support::redraw(cx);
        let wordmark = drawn(cx, "app-wordmark");
        assert!(
            wordmark.left() < px(88.),
            "fullscreen gives the room back: {wordmark:?}"
        );
    }

    /// SPEC.md: "With no session open, the main pane's half of the band is empty background too." The old app laid a
    /// 48-pixel strip over the empty pane's top (`.main-empty-titlebar`), as wide as the pane, and checked that a press
    /// there hits the strip rather than the pane's text behind it.
    #[gpui::test]
    fn with_no_session_open_a_band_strip_covers_the_panes_top(cx: &mut TestAppContext) {
        let (_fake, cx) = open(cx, &[]);
        test_support::settle(cx);
        let strip = drawn(cx, "main-empty-titlebar");
        let pane = drawn(cx, "main-pane");
        assert_eq!(strip.top(), px(0.));
        assert_eq!(strip.size.height, px(48.));
        assert_eq!(
            (strip.left(), strip.size.width),
            (pane.left(), pane.size.width)
        );
    }

    /// SPEC.md: "In narrow macOS windows, the app-level row stays fixed above both scrolling panes so horizontal
    /// scrolling cannot move application controls underneath native window buttons." Once the window is no wider
    /// than the sidebar and the pane's floor (the old `width + 321` cutoff), the app bar runs across the whole window's
    /// top and both panes start below it; a wider window keeps it in the sidebar's column.
    #[gpui::test]
    fn in_a_narrow_window_the_app_bar_runs_above_both_panes(cx: &mut TestAppContext) {
        let (_fake, cx) = open(cx, &["s1"]);
        test_support::settle(cx);
        let wide = test_support::bounds(cx, "app-bar");
        let sidebar = drawn(cx, "sidebar");
        assert!(
            wide.right() <= sidebar.right(),
            "the premise: a wide window keeps it in the sidebar's column: {wide:?} in {sidebar:?}"
        );
        let narrow = sidebar.size.width + px(321.);
        cx.simulate_resize(size(narrow, px(600.)));
        test_support::redraw(cx);
        let bar = test_support::bounds(cx, "app-bar");
        assert_eq!(
            (bar.left(), bar.top(), bar.size.width),
            (px(0.), px(0.), narrow)
        );
        let sidebar = drawn(cx, "sidebar");
        let top = test_support::bounds(cx, "session-header");
        assert!(
            sidebar.top() >= px(48.) && top.top() >= px(48.),
            "both panes start below it"
        );
    }

    /// What the band decided for a primary press at `at` that then moves a few points, with the window's real rows (the decisions
    /// are recorded instead of performed: GPUI's test platform cannot move or zoom a window).
    fn press_at(cx: &mut VisualTestContext, at: gpui::Point<Pixels>) -> Option<(Row, PressAction)> {
        cx.update(|_, cx| cx.set_global(title_band::Decisions::default()));
        cx.simulate_event(gpui::MouseDownEvent {
            button: gpui::MouseButton::Left,
            position: at,
            modifiers: gpui::Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        });
        cx.simulate_event(gpui::MouseMoveEvent {
            position: at + gpui::point(px(3.), px(2.)),
            pressed_button: Some(gpui::MouseButton::Left),
            modifiers: gpui::Modifiers::none(),
        });
        cx.simulate_event(gpui::MouseUpEvent {
            button: gpui::MouseButton::Left,
            position: at,
            modifiers: gpui::Modifiers::none(),
            click_count: 1,
        });
        test_support::settle(cx);
        let decided = cx.update(|_, cx| cx.default_global::<title_band::Decisions>().0.clone());
        assert!(decided.len() <= 1, "one decision per press: {decided:?}");
        decided.first().copied()
    }

    /// The band is wired into the real rows, not only the stand-in scene's: in the main view with a session open, a
    /// press on the app bar's wordmark and on the session top row's empty left region drags; a press on the version
    /// text and on a session action keeps theirs; and a press at the band's height on the sidebar's resize edge, which
    /// lies outside both rows, decides nothing. With no session open, the strip over the empty pane drags.
    #[gpui::test]
    fn the_real_rows_drag_from_their_background_and_leave_their_controls_and_text_alone(
        cx: &mut TestAppContext,
    ) {
        let (_fake, cx) = open(cx, &["s1"]);
        test_support::settle(cx);
        let drag = |row| Some((row, PressAction::Drag));
        let center = |cx: &mut VisualTestContext, id| drawn(cx, id).center();
        let at = center(cx, "app-wordmark");
        assert_eq!(press_at(cx, at), drag(Row::AppBar), "the wordmark");
        let at = test_support::center(cx, crate::session_pane::header::TABS_SLOT);
        assert_eq!(
            press_at(cx, at),
            drag(Row::SessionTop),
            "the top row's empty left region"
        );
        let at = test_support::center(cx, "app-version");
        assert_eq!(
            press_at(cx, at),
            Some((Row::AppBar, PressAction::None)),
            "the version text"
        );
        let at = center(cx, "session-header-stop");
        assert_eq!(
            press_at(cx, at),
            Some((Row::SessionTop, PressAction::None)),
            "a session action"
        );
        let divider = test_support::bounds(cx, "sidebar-divider");
        let at = gpui::point(divider.center().x, px(24.));
        assert_eq!(press_at(cx, at), None, "the sidebar's resize edge");
    }

    /// See the test above: with no session open, the strip over the empty pane is the band's other half.
    #[gpui::test]
    fn with_no_session_open_the_empty_panes_strip_drags(cx: &mut TestAppContext) {
        let (_fake, cx) = open(cx, &[]);
        test_support::settle(cx);
        let at = drawn(cx, "main-empty-titlebar").center();
        assert_eq!(press_at(cx, at), Some((Row::EmptyPane, PressAction::Drag)));
    }

    /// SPEC.md: startup screens keep their content clear of the native controls, and with the app owning the title
    /// bar's drag the window must still be movable there: while the window is not connected (here it never connects),
    /// a strip of the band runs across its top, 48 points tall, and a press on it drags.
    #[gpui::test]
    fn the_connection_screens_strip_keeps_the_lights_clear_and_drags(cx: &mut TestAppContext) {
        test_support::install(cx);
        let (_, cx) = test_support::open_view(cx, MainView::new);
        test_support::settle(cx);
        let strip = drawn(cx, "connection-titlebar");
        assert_eq!((strip.top(), strip.size.height), (px(0.), px(48.)));
        assert_eq!(
            press_at(cx, strip.center()),
            Some((Row::EmptyPane, PressAction::Drag))
        );
    }
}

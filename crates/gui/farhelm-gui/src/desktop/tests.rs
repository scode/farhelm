//! Tests of the desktop module: the Mac menu bar's description and its commands, and the main window's close quitting
//! the app.
//!
//! They run on GPUI's fake platform, under Linux too: the menu bar is installed with `mac` set, which records the menus
//! exactly as macOS would receive them. What only a real macOS window shows (the bar itself, AppKit's handling of the
//! window list, a real quit) is checked on the test Mac, recorded in the parity file.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{Action as _, Keystroke, OsAction, OwnedMenuItem, TestAppContext, div};

use super::{menu, when_closed};
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
/// window, which quits the app. So: the whole bar as installed, in order, with each item's action; no item named
/// Close; and every binding of Cmd+W in the app's keymap (Kit's and Farhelm's, as at startup) runs Delete. The Edit
/// items dispatch the text field's own actions, which a focused field binds in its own key context: that is what
/// makes them act on a field (and Copy, which GPUI Kit's window root also handles, nowhere it could copy anything else).
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
        // Cmd+W is bound only on macOS (Linux's Delete chord is Ctrl+Shift+Backspace); there the loop must have seen it,
        // or it proved nothing. On Linux the Mac chord table's own test covers the chords the menu shows.
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
/// Kit's window root also handles (copying Kit's own selected text, of which Farhelm draws none). Asked in a real window
/// wrapped in Kit's root, with the field focused and then a control.
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
                // The app's question, not the window's: it counts the app-wide handlers too, as macOS's does. Asked from
                // the app rather than inside the window's own update, which it enters itself.
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

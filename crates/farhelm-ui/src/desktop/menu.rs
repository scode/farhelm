//! Keep Cmd-W available to the page while preserving native Mac menu actions.
//!
//! Dioxus 0.7's default menu is private, and its predefined Close Window item
//! reserves Cmd-W before the webview can see it. Reconstruct that menu without
//! Close Window. The description stays independent of native menu objects so
//! Linux can guard this contract without a display server or AppKit.

/// The native actions retained from Dioxus's menu, plus its debug commands.
///
/// This closed set deliberately has no arbitrary accelerator or Close Window
/// entry: Cmd-W belongs to session deletion, including when no session is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
    Fullscreen,
    Separator,
    Hide,
    HideOthers,
    ShowAll,
    Maximize,
    Minimize,
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    ToggleDeveloperTools,
    FloatOnTop,
}

/// A submenu in the actual installed menu; tests inspect this same description.
#[derive(Debug, PartialEq, Eq)]
struct Submenu {
    title: &'static str,
    items: &'static [Item],
}

// Preserve dioxus-desktop 0.7.10's Window/Edit/debug-Help arrangement. Its
// default builder is private, so replacing the menu is how Close Window can
// be omitted without losing the remaining native commands.
const MENUS: &[Submenu] = &[
    Submenu {
        title: "Window",
        items: &[
            Item::Fullscreen,
            Item::Separator,
            Item::Hide,
            Item::HideOthers,
            Item::ShowAll,
            Item::Maximize,
            Item::Minimize,
            Item::Separator,
            Item::Quit,
        ],
    },
    Submenu {
        title: "Edit",
        items: &[
            Item::Undo,
            Item::Redo,
            Item::Separator,
            Item::Cut,
            Item::Copy,
            Item::Paste,
            Item::Separator,
            Item::SelectAll,
        ],
    },
    Submenu {
        title: "Help",
        items: &[Item::ToggleDeveloperTools, Item::FloatOnTop],
    },
];

/// Match Dioxus's debug-only Help menu without constructing native UI objects.
fn description(debug: bool) -> &'static [Submenu] {
    if debug { MENUS } else { &MENUS[..2] }
}

/// Override only the Mac menu; other desktop targets retain Dioxus's default.
///
/// The constant condition keeps the materializer type-checked on Linux desktop
/// builds, while no GTK menu objects are constructed there. Window callbacks
/// and close behavior remain on the incoming configuration, so the close button
/// still saves the window frame before Dioxus removes the window.
#[cfg(native_desktop)]
pub(super) fn configure(config: dioxus::desktop::Config) -> dioxus::desktop::Config {
    if cfg!(target_os = "macos") {
        config.with_menu(build(cfg!(debug_assertions)).expect("construct Mac menu"))
    } else {
        config
    }
}

/// Preserve the native selectors and debug command IDs Dioxus already handles.
///
/// Only assigning the special AppKit menu roles needs macOS-only APIs. Keep
/// construction itself portable so Linux desktop checks catch muda API drift.
#[cfg(native_desktop)]
fn build(debug: bool) -> dioxus::desktop::muda::Result<dioxus::desktop::muda::Menu> {
    use dioxus::desktop::muda;

    let menu = muda::Menu::new();
    for section in description(debug) {
        let submenu = muda::Submenu::new(section.title, true);
        for item in section.items {
            item.append_to(&submenu)?;
        }
        menu.append(&submenu)?;

        // These roles enable AppKit's Window/Help integration beyond merely
        // giving a submenu the expected visible title.
        #[cfg(target_os = "macos")]
        match section.title {
            "Window" => submenu.set_as_windows_menu_for_nsapp(),
            "Help" => submenu.set_as_help_menu_for_nsapp(),
            _ => {}
        }
    }
    Ok(menu)
}

#[cfg(native_desktop)]
impl Item {
    /// Materialize only the allowed commands, retaining native editing and Quit.
    ///
    /// Debug commands carry Dioxus's existing IDs and no accelerator. Standard
    /// actions use muda's predefined items to retain their native responders.
    fn append_to(self, menu: &dioxus::desktop::muda::Submenu) -> dioxus::desktop::muda::Result<()> {
        use dioxus::desktop::muda::{MenuItem, PredefinedMenuItem};

        let item = match self {
            Self::Fullscreen => PredefinedMenuItem::fullscreen(None),
            Self::Separator => PredefinedMenuItem::separator(),
            Self::Hide => PredefinedMenuItem::hide(None),
            Self::HideOthers => PredefinedMenuItem::hide_others(None),
            Self::ShowAll => PredefinedMenuItem::show_all(None),
            Self::Maximize => PredefinedMenuItem::maximize(None),
            Self::Minimize => PredefinedMenuItem::minimize(None),
            Self::Quit => PredefinedMenuItem::quit(None),
            Self::Undo => PredefinedMenuItem::undo(None),
            Self::Redo => PredefinedMenuItem::redo(None),
            Self::Cut => PredefinedMenuItem::cut(None),
            Self::Copy => PredefinedMenuItem::copy(None),
            Self::Paste => PredefinedMenuItem::paste(None),
            Self::SelectAll => PredefinedMenuItem::select_all(None),
            Self::ToggleDeveloperTools => {
                return menu.append(&MenuItem::with_id(
                    "dioxus-toggle-dev-tools",
                    "Toggle Developer Tools",
                    true,
                    None,
                ));
            }
            Self::FloatOnTop => {
                return menu.append(&MenuItem::with_id(
                    "dioxus-float-top",
                    "Float on Top (dev mode only)",
                    true,
                    None,
                ));
            }
        };
        menu.append(&item)
    }
}

#[cfg(test)]
mod tests {
    use super::{Item, Submenu, description};

    /// Cmd-W must remain Farhelm's delete-session shortcut, never native Close.
    ///
    /// Muda's predefined Close Window item hard-wires Cmd-W, which macOS takes
    /// before the page sees it. Restoring it silently makes Cmd-W quit Farhelm
    /// again. Guard the complete production description in release and debug
    /// modes so an added menu entry cannot acquire that chord unnoticed. This
    /// checks the menu contract on Linux without constructing native objects.
    #[test]
    fn mac_menu_preserves_session_delete_chord() {
        let expected = [
            Submenu {
                title: "Window",
                items: &[
                    Item::Fullscreen,
                    Item::Separator,
                    Item::Hide,
                    Item::HideOthers,
                    Item::ShowAll,
                    Item::Maximize,
                    Item::Minimize,
                    Item::Separator,
                    Item::Quit,
                ],
            },
            Submenu {
                title: "Edit",
                items: &[
                    Item::Undo,
                    Item::Redo,
                    Item::Separator,
                    Item::Cut,
                    Item::Copy,
                    Item::Paste,
                    Item::Separator,
                    Item::SelectAll,
                ],
            },
            Submenu {
                title: "Help",
                items: &[Item::ToggleDeveloperTools, Item::FloatOnTop],
            },
        ];
        let reason = "native menu changes must leave Cmd-W to session deletion; \
                      Close Window consumes it before the page receives the key";
        assert_eq!(description(false), &expected[..2], "{reason}");
        assert_eq!(description(true), &expected, "{reason}");
    }
}

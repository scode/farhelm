//! The keys of a menu a toggle opens, beyond the layer manager's: Down and Up on the closed toggle open the menu on its
//! first or last item, and Home and End inside the open menu move to its first or last item.
//!
//! SPEC.md, "Terminal experience", has the session view's split menus follow "the app's shared keyboard rules: ...
//! arrows and Home/End move between choices", and the old app's menus opened from a closed toggle with Down or Up
//! (`closed_toggle_key_intent` in `menu_panel.rs` at reference commit `24bc0580`). The layer manager's menu context
//! binds only Up and Down inside an open menu, so these two handlers are what the owner of a menu opened from a toggle
//! adds, rather than each such menu keeping its own copy.
//!
//! Neither draws anything or takes focus itself: [`toggle_arrow_keys`] goes on the toggle (through its button's
//! `on_key_down`), and [`home_end_keys`] on the open menu's panel. Each key is taken only without modifiers, and a held
//! Down or Up opens nothing, so a key still held from closing the menu does not open it again.

use gpui::{App, FocusHandle, KeyDownEvent, Window};

use crate::focus::{self, Mark};

/// Which item a menu opened from its toggle starts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuStart {
    /// The first item: a click, Enter, Space, or Down on the closed toggle.
    First,
    /// The last item: Up on the closed toggle.
    Last,
}

/// A key handler for a closed menu's toggle: Down calls `open` with [`MenuStart::First`] and Up with
/// [`MenuStart::Last`], and both stop there; any other key, or one with a modifier, goes on. An open menu holds focus,
/// so these never reach the toggle while it is open.
pub fn toggle_arrow_keys(
    open: impl Fn(MenuStart, &mut Window, &mut App) + 'static,
) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static {
    move |event, window, cx| {
        let start = match event.keystroke.key.as_str() {
            "down" => MenuStart::First,
            "up" => MenuStart::Last,
            _ => return,
        };
        if event.keystroke.modifiers.modified() {
            return;
        }
        cx.stop_propagation();
        if event.is_held {
            return;
        }
        open(start, window, cx);
    }
}

/// A key handler for an open menu's panel: Home moves focus to `first` and End to `last`, through the focus
/// coordinator's move inside a layer; any other key, or one with a modifier, goes on.
pub fn home_end_keys(
    first: FocusHandle,
    last: FocusHandle,
) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static {
    move |event, window, cx| {
        let target = match event.keystroke.key.as_str() {
            "home" => &first,
            "end" => &last,
            _ => return,
        };
        if event.keystroke.modifiers.modified() {
            return;
        }
        cx.stop_propagation();
        let mark = Mark::take(cx);
        focus::focus_in_layer(target, &mark, window, cx);
    }
}

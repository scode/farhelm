//! The app's actions and the keys bound to them, declared in one place.
//!
//! SPEC_impl.md, "Actions and keymap": buttons, menu items and keys dispatch the same action, and every action is
//! declared here, so a key and the control it stands for can never drift apart. This file starts with the actions the
//! focus coordinator and the layer manager need; the actions and keymap owner (the next phase) adds the
//! rest, the terminal pass-through table and the chord-prefix test.
//!
//! Every binding lives in [`CONTEXT`], the key context the root view sets on everything Farhelm draws. GPUI Kit's `Root`
//! binds Tab and Shift+Tab in its own context, which sits above Farhelm's root, and moves focus itself when it handles
//! them; a binding in a deeper context wins, so these bindings keep Tab with the focus coordinator. A Kit text input
//! binds its own keys in its own, deeper context and passes on the ones it does not use (Tab and Escape included).

use gpui::{App, KeyBinding, actions};

/// The key context the root view sets, and the one every Farhelm binding is declared in.
pub const CONTEXT: &str = "Farhelm";

actions!(
    farhelm,
    [
        /// Move focus to the next control, staying inside the topmost dialog when one is open.
        FocusNext,
        /// Move focus to the previous control, staying inside the topmost dialog when one is open.
        FocusPrevious,
        /// Move focus to the next control of any kind, buttons included, whatever the Keyboard navigation setting:
        /// macOS's Option-Tab (decision D-2).
        FocusNextControl,
        /// Move focus to the previous control of any kind: Option-Shift-Tab.
        FocusPreviousControl,
        /// Close the topmost layer (a dialog, a menu or a tooltip), returning focus to where it was before it opened.
        Dismiss,
    ]
);

/// Bind the keys. The bootstrap calls this after GPUI Kit's own init, so Farhelm's bindings are added after Kit's.
pub fn install(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", FocusNext, Some(CONTEXT)),
        KeyBinding::new("shift-tab", FocusPrevious, Some(CONTEXT)),
        // Option-Tab on macOS. Bound on every platform: where Tab already reaches every control it does the same as Tab,
        // and on Linux the window manager usually keeps Alt-Tab for itself.
        KeyBinding::new("alt-tab", FocusNextControl, Some(CONTEXT)),
        KeyBinding::new("alt-shift-tab", FocusPreviousControl, Some(CONTEXT)),
        KeyBinding::new("escape", Dismiss, Some(CONTEXT)),
    ]);
}

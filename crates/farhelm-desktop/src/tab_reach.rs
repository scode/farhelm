//! Which controls a plain Tab reaches, read from the platform for the GUI's focus coordinator.
//!
//! On macOS, Tab follows the system's Keyboard navigation setting exactly as the old app did, because WebKit followed
//! it: with the setting off (the system default), Tab reaches text fields and pop-ups only and Option-Tab reaches
//! buttons too; with it on, Tab reaches every control. Linux has no such setting, and Tab reaches every control there,
//! the GTK and Qt convention. The migration's orchestrator decided this on 2026-11-13: the Mac keeps the old app's
//! behavior (the GPUI app ports the old one rather than redesigning it), and Linux, where the old app never shipped,
//! follows its platform.
//!
//! The read lives in this binary rather than the GUI crate because it needs platform code, and the GUI crates carry no
//! platform crates of their own. The coordinator asks on every Tab and Shift+Tab press, so a change made while the app
//! runs (in System Settings, or with Control-F7 without leaving the app) applies to the next press, as it did in the
//! old app's WebKit. The read is cheap and never fails: a preference it cannot read answers the system default.

use farhelm_gui::focus::TabReach;

/// Which controls Tab reaches on this platform, as the user has set it right now.
pub(crate) fn current() -> TabReach {
    #[cfg(target_os = "macos")]
    {
        from_keyboard_ui_mode(macos::keyboard_ui_mode())
    }
    #[cfg(not(target_os = "macos"))]
    {
        TabReach::EveryControl
    }
}

/// The macOS rule, from the stored value of the global `AppleKeyboardUIMode` preference, which System Settings' Keyboard
/// navigation switch writes: `2` when it is on (`3` on older systems, whose "All controls" choice set the same bit), `0`
/// or absent when it is off. Tab reaches every control exactly when that bit is set. Kept separate from the read so the
/// rule is testable on any platform.
#[cfg_attr(
    not(target_os = "macos"),
    allow(
        dead_code,
        reason = "only the macOS read calls it; elsewhere it is kept, with its tests, so the rule is testable there"
    )
)]
pub(crate) fn from_keyboard_ui_mode(mode: Option<i64>) -> TabReach {
    match mode {
        Some(mode) if mode & 2 != 0 => TabReach::EveryControl,
        _ => TabReach::TextFieldsAndPopups,
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use core_foundation::base::TCFType as _;
    use core_foundation::string::CFString;
    use core_foundation_sys::base::Boolean;
    use core_foundation_sys::preferences::{
        CFPreferencesAppSynchronize, CFPreferencesGetAppIntegerValue,
        kCFPreferencesCurrentApplication,
    };

    /// The current value of the `AppleKeyboardUIMode` preference, or `None` when it is not set (or not a number). Read
    /// for the current application, which falls through to the global domain System Settings writes, and synchronized
    /// first because this process caches preferences while another process changes this one; this is how WebKit's
    /// `WKFullKeyboardAccessWatcher` reads it.
    pub(super) fn keyboard_ui_mode() -> Option<i64> {
        let key = CFString::from_static_string("AppleKeyboardUIMode");
        let mut valid: Boolean = 0;
        // SAFETY: Core Foundation's preference functions are thread-safe and take borrowed references: `key` lives for
        // the whole call, `kCFPreferencesCurrentApplication` is a static constant, and `valid` is a valid out-pointer.
        let mode = unsafe {
            CFPreferencesAppSynchronize(kCFPreferencesCurrentApplication);
            CFPreferencesGetAppIntegerValue(
                key.as_concrete_TypeRef(),
                kCFPreferencesCurrentApplication,
                &mut valid,
            )
        };
        (valid != 0).then_some(mode as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mapping from the stored preference to Tab's reach is the whole macOS rule; a wrong bit here would
    /// silently make Tab skip buttons for a keyboard-only user who turned Keyboard navigation on.
    #[farhelm_testtrace::test]
    fn the_keyboard_navigation_bit_decides_what_tab_reaches() {
        assert_eq!(from_keyboard_ui_mode(None), TabReach::TextFieldsAndPopups);
        assert_eq!(
            from_keyboard_ui_mode(Some(0)),
            TabReach::TextFieldsAndPopups
        );
        assert_eq!(from_keyboard_ui_mode(Some(2)), TabReach::EveryControl);
        assert_eq!(from_keyboard_ui_mode(Some(3)), TabReach::EveryControl);
        assert_eq!(
            from_keyboard_ui_mode(Some(1)),
            TabReach::TextFieldsAndPopups
        );
    }
}

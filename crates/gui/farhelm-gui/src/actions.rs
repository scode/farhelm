//! The app's actions and the keys bound to them, declared in one place, and which keys a focused terminal keeps.
//!
//! SPEC_impl.md, "Actions and keymap": buttons, menu items and keys dispatch the same action, and every action is
//! declared here, so a key and the control it stands for can never drift apart. This file holds the actions the focus
//! coordinator and the layer manager need, and the terminal's key table ([`TERMINAL_KEYS`]). The features add their own
//! actions as they are built.
//!
//! Every binding lives in [`CONTEXT`], the key context the root view sets on everything Farhelm draws. GPUI Kit's `Root`
//! binds Tab and Shift+Tab in its own context, which sits above Farhelm's root, and moves focus itself when it handles
//! them; a binding in a deeper context wins, so these bindings keep Tab with the focus coordinator. A Kit text input
//! binds its own keys in its own, deeper context and passes on the ones it does not use (Tab and Escape included).
//!
//! ## A focused terminal
//!
//! A session terminal sets [`TERMINAL_CONTEXT`] inside [`CONTEXT`]. GPUI matches key bindings before any key listener
//! sees the key, so a binding anywhere in a terminal's context stack (Farhelm's Tab and Escape, Kit's Tab and Copy)
//! would take the key from the terminal's program. SPEC.md: "While a terminal has focus, Tab, Escape, Enter and
//! ordinary keys keep going to its program." [`TERMINAL_KEYS`] names every chord whose owner is not simply the program:
//! Farhelm's own shortcuts that still work from a terminal, and the terminal's own commands. It also names the keys
//! that some binding outside the terminal would otherwise catch, which [`install`] binds to GPUI's `NoAction` in the
//! terminal's context so that they reach the terminal. Every key not in the table goes to the program. The keymap tests
//! walk the whole keymap to keep it that way: no binding other than an app shortcut or the terminal's own resolves in a
//! focused terminal.
//!
//! No context that contains a terminal may hold a multi-key chord prefix (SPEC_impl.md): GPUI holds the first key of a
//! chord back until the next key shows whether the chord goes on, giving up after a second only when that key alone is
//! bound too or types text into an element that accepts text input, and otherwise waiting with no time limit (typing
//! into a terminal without a text input handler included). The program would see typing stall. The keymap tests fail
//! on one.
//!
//! The terminal itself asks [`terminal_key_owner`] about each key that reaches it, rather than deciding again which
//! keys are Farhelm's: an app shortcut whose feature has no binding yet still reaches the terminal's listener, and must
//! not be sent to the program either.

use farhelm_input_mapping::Platform;
use gpui::{App, KeyBinding, KeyDownEvent, KeybindingKeystroke, Keystroke, NoAction, actions};

/// The key context the root view sets, and the one every Farhelm binding is declared in.
pub const CONTEXT: &str = "Farhelm";

/// The platform this build runs on, in the terms the terminal's input mapping uses.
pub const THIS_PLATFORM: Platform = if cfg!(target_os = "macos") {
    Platform::Mac
} else {
    Platform::Linux
};

/// The key context a session terminal sets on the element that holds its focus. It sits inside [`CONTEXT`], so every
/// Farhelm binding applies in a terminal too, except the keys [`TERMINAL_KEYS`] gives the terminal or its program.
pub const TERMINAL_CONTEXT: &str = "Terminal";

actions!(
    farhelm,
    [
        /// Move focus to the next control, staying inside the topmost dialog when one is open.
        FocusNext,
        /// Move focus to the previous control, staying inside the topmost dialog when one is open.
        FocusPrevious,
        /// Move focus to the next control of any kind, buttons included, whatever the Keyboard navigation setting:
        /// macOS's Option-Tab (the migration's orchestrator decided on 2026-11-13 that Tab follows the platform, as
        /// the focus coordinator describes).
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
    // `NoAction` in the deeper context suppresses every binding for the same keys further out, Farhelm's and Kit's
    // alike, so the key goes on to the terminal's own listener. App shortcuts are left alone: their features bind them
    // in `CONTEXT`, and those bindings must keep working from a terminal.
    //
    // Binding order matters in the terminal's own context too. GPUI ranks bindings of equal depth by when they were
    // added, latest first, and a `NoAction` suppresses everything ranked after it. So a terminal action bound in
    // `TERMINAL_CONTEXT` (copy or paging, when the terminal gets them) works only if it is bound after these rows;
    // bound before them, its key silently does nothing. The same ranking lets a binding with no context that is added
    // after this one beat a row here; the keymap walk test catches that case.
    cx.bind_keys(
        TERMINAL_KEYS
            .iter()
            .filter(|key| key.owner != KeyOwner::App)
            .filter_map(|key| key.chord(THIS_PLATFORM))
            .map(|chord| KeyBinding::new(chord, NoAction, Some(TERMINAL_CONTEXT))),
    );
}

// ---- The terminal's key table ----------------------------------------------------------------------------------------

/// Who a key pressed in a focused terminal belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOwner {
    /// One of Farhelm's own shortcuts, which works from a focused terminal and never reaches its program (SPEC.md
    /// lists each one as working "including from a focused terminal" or as never reaching the program). Its feature
    /// binds it in [`CONTEXT`]; until that feature exists the key reaches the terminal, which must still not send it.
    App,
    /// The terminal's own command, such as scrolling a page: it never reaches the program, and nothing outside the
    /// terminal takes it. Until the terminal binds an action for one, the key reaches the terminal's listener, where
    /// `farhelm-input-mapping` still classifies paging and select all (its own copy of that rule goes when the
    /// keymap takes it over).
    Terminal,
    /// The program in the terminal. Every key not in [`TERMINAL_KEYS`] is the program's; the table lists the ones that
    /// some binding outside the terminal would otherwise take.
    Program,
}

/// One row of [`TERMINAL_KEYS`]: a chord in GPUI's keystroke syntax on each platform (`None` where the platform has no
/// such chord), who owns it, and what it is.
#[derive(Clone, Copy, Debug)]
pub struct TerminalKey {
    pub mac: Option<&'static str>,
    pub linux: Option<&'static str>,
    pub owner: KeyOwner,
    pub what: &'static str,
}

impl TerminalKey {
    /// The chord on `platform`, if it has one there.
    pub fn chord(&self, platform: Platform) -> Option<&'static str> {
        match platform {
            Platform::Mac => self.mac,
            Platform::Linux => self.linux,
        }
    }
}

/// Every chord a focused terminal does not simply send to its program, and every chord it sends there that some binding
/// outside the terminal would otherwise take.
///
/// The app shortcuts are the ones SPEC.md says work from a focused terminal or never reach its program, with the old
/// app's modifiers at the reference commit: Command on a Mac, Ctrl+Shift on Linux. The terminal's own keys are the old
/// grid terminal's (Shift+PageUp/PageDown, and Cmd+A on a Mac), plus the Mac's Cmd+C and Cmd+V, which were WebKit's copy
/// and paste in the old app's terminal. Linux has no copy or paste chord here: neither SPEC.md nor the old app's terminal
/// settles one, so it waits for the paste work.
///
/// NOTE: Not in this table, because a key binding cannot express them: the number keys (Cmd or Ctrl+Shift with 0-9),
/// the Keyboard shortcuts dialog (with `/`) and terminal text size (with `=` and `-`). SPEC.md and the old app match
/// those by the physical key, which a binding's logical key does not capture (on Linux, GPUI reports Ctrl+Shift+1 as
/// Ctrl+`!` on a US layout and as something else on others). They join a physical-key table when their features are
/// built; until then a focused terminal sends them to its program.
pub const TERMINAL_KEYS: &[TerminalKey] = &[
    // Farhelm's shortcuts that work from a focused terminal.
    app("cmd-k", "ctrl-shift-k", "open the quick switcher"),
    app("cmd-j", "ctrl-shift-j", "open the template list"),
    app("cmd-p", "ctrl-shift-p", "quick launch the marked template"),
    app("cmd-x", "ctrl-shift-x", "begin a quick-launch sequence"),
    TerminalKey {
        mac: Some("cmd-n"),
        // SPEC.md: the Linux app has no New shortcut.
        linux: None,
        owner: KeyOwner::App,
        what: "open New",
    },
    app("cmd-w", "ctrl-shift-backspace", "delete the open session"),
    app("cmd-l", "ctrl-shift-l", "rename the open session"),
    // The terminal's own commands.
    both(
        "shift-pageup",
        KeyOwner::Terminal,
        "scroll the terminal up a page",
    ),
    both(
        "shift-pagedown",
        KeyOwner::Terminal,
        "scroll the terminal down a page",
    ),
    mac_only(
        "cmd-a",
        KeyOwner::Terminal,
        "select all of the terminal's text",
    ),
    mac_only(
        "cmd-c",
        KeyOwner::Terminal,
        "copy the terminal's selection (GPUI Kit's root binds it to its Copy)",
    ),
    mac_only("cmd-v", KeyOwner::Terminal, "paste into the terminal"),
    // Keys the program gets that something outside the terminal binds.
    both(
        "tab",
        KeyOwner::Program,
        "Tab (the focus coordinator's, and GPUI Kit's root's)",
    ),
    both(
        "shift-tab",
        KeyOwner::Program,
        "Shift+Tab (the focus coordinator's, and GPUI Kit's root's)",
    ),
    both(
        "alt-tab",
        KeyOwner::Program,
        "Option-Tab (the focus coordinator's)",
    ),
    both(
        "alt-shift-tab",
        KeyOwner::Program,
        "Option-Shift-Tab (the focus coordinator's)",
    ),
    both(
        "escape",
        KeyOwner::Program,
        "Escape (Dismiss's, which would close a tooltip instead; SPEC_impl.md: Escape from a focused terminal goes to \
         its program)",
    ),
    TerminalKey {
        mac: None,
        linux: Some("ctrl-c"),
        owner: KeyOwner::Program,
        what: "Ctrl+C, the program's interrupt (GPUI Kit's root binds it to its Copy off macOS)",
    },
    TerminalKey {
        mac: Some("cmd-alt-i"),
        linux: Some("ctrl-shift-i"),
        owner: KeyOwner::Program,
        what: "GPUI Kit's inspector toggle, bound in debug builds only",
    },
];

/// An app shortcut: Command on a Mac, Ctrl+Shift on Linux.
const fn app(mac: &'static str, linux: &'static str, what: &'static str) -> TerminalKey {
    TerminalKey {
        mac: Some(mac),
        linux: Some(linux),
        owner: KeyOwner::App,
        what,
    }
}

/// A chord that is the same on both platforms.
const fn both(chord: &'static str, owner: KeyOwner, what: &'static str) -> TerminalKey {
    TerminalKey {
        mac: Some(chord),
        linux: Some(chord),
        owner,
        what,
    }
}

/// A chord only a Mac has.
const fn mac_only(chord: &'static str, owner: KeyOwner, what: &'static str) -> TerminalKey {
    TerminalKey {
        mac: Some(chord),
        linux: None,
        owner,
        what,
    }
}

/// Who the key pressed in `event`, in a focused terminal on this platform, belongs to. The terminal calls this for every
/// key that reaches its listener: a [`KeyOwner::App`] key is not sent to the program (the terminal hands it to
/// `farhelm-input-mapping` as an app shortcut), and any other key goes on to the terminal's own input mapping. While a
/// shortcut has no binding, the input mapping also keeps its held repeats from the program; once it is bound, the
/// binding sees every repeat and this is never asked.
///
/// A key a binding handled never reaches the terminal, so this answers only for the keys that do. It matches the way
/// GPUI matches a binding ([`Keystroke::should_match`]), so a key counts as a chord here exactly when a binding for
/// that chord would take it. It takes the whole key-down event, not only its keystroke, because the shortcuts this
/// table cannot hold yet (the NOTE on [`TERMINAL_KEYS`]) match the physical key, which only the event's `native_key`
/// carries; today only the keystroke is read.
pub fn terminal_key_owner(event: &KeyDownEvent) -> KeyOwner {
    owner_on(THIS_PLATFORM, event)
}

/// [`terminal_key_owner`] on `platform`, so the Mac's answers can be tested where the suite runs, on Linux.
pub fn owner_on(platform: Platform, event: &KeyDownEvent) -> KeyOwner {
    let keystroke = &event.keystroke;
    TERMINAL_KEYS
        .iter()
        .find(|key| {
            key.chord(platform)
                .and_then(|chord| Keystroke::parse(chord).ok())
                .is_some_and(|chord| {
                    keystroke.should_match(&KeybindingKeystroke::from_keystroke(chord))
                })
        })
        .map_or(KeyOwner::Program, |key| key.owner)
}

//! The app's actions and the keys bound to them, declared in one place, and which keys a focused terminal keeps.
//!
//! SPEC_impl.md, "Actions and keymap": buttons, menu items and keys dispatch the same action, and every action is
//! declared here, so a key and the control it stands for can never drift apart. This file holds the actions the focus
//! coordinator and the layer manager need, the session shortcuts' actions, the terminal's key table
//! ([`TERMINAL_KEYS`]), the shortcuts matched by the physical key ([`PHYSICAL_KEYS`]), the list of actions a held key
//! runs only once (`ONCE_PER_PRESS`), the one slot through which a feature can take every key for a while
//! ([`arm_key_capture`]), and the Mac app menu's window and app commands with their chords (`MAC_MENU_CHORDS`; the
//! desktop module builds the menu and handles them). The features add their own actions as they are built.
//!
//! Every binding lives in [`CONTEXT`], the key context the root view sets on everything Farhelm draws. GPUI Kit's
//! `Root` binds Tab and Shift+Tab in its own context, which sits above Farhelm's root, and moves focus itself when it
//! handles them; a binding in a deeper context wins, so these bindings keep Tab with the focus coordinator. A Kit text
//! input binds its own keys in its own, deeper context ([`TEXT_INPUT_CONTEXT`]) and passes on the ones it does not use
//! (Tab and Escape included). A key Farhelm binds in [`CONTEXT`] therefore reaches a focused text field's ancestors and
//! runs there, unless that deeper context says otherwise: SPEC.md has the Linux Delete chord, Ctrl+Shift+Backspace,
//! leave text editors other than the terminal alone, so [`install`] binds it to GPUI's `NoAction` in Kit's input
//! context. A menu sets [`MENU_CONTEXT`], where Up and Down walk its items.
//!
//! Extension points (`docs/gpui/foundation-ownership.md`), besides an action and its binding in [`CONTEXT`]: a row in
//! [`TERMINAL_KEYS`] (with its action, once the terminal command it names has one), a row in [`PHYSICAL_KEYS`] for a
//! shortcut SPEC.md matches by key position, an action in `ONCE_PER_PRESS`, a chord in `TEXT_FIELD_KEYS` for an app
//! shortcut SPEC.md says a text field keeps, and arming the key capture slot ([`arm_key_capture`]) for a mode SPEC.md
//! says takes every key without taking focus.
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
//!
//! ## Taking every key for a while: the key capture slot
//!
//! SPEC.md's quick-launch sequence takes every key while it is armed, without taking focus: "While armed, typed
//! characters never reach the terminal's program", Escape cancels it rather than closing a layer, and a key that is not
//! part of the sequence cancels it and still reaches its normal target. Focus cannot move to a capturing element, so
//! the keystroke interceptor, which sees each key-down before bindings, the physical-key rows and the focused element,
//! is the place for it. It holds one app-wide slot ([`arm_key_capture`]): while the slot is armed, every key-down in
//! every window goes to its handler first, held repeats included (the event's `is_held` says which), and the handler
//! answers [`Captured::Consumed`], which stops the key there, or [`Captured::Cancel`], which disarms the slot and lets
//! the key go on through the rest of the interceptor and GPUI's dispatch as if nothing had been armed. Modifiers
//! pressed on their own never reach the handler: GPUI reports them as modifier changes, which carry no key-down.
//!
//! The holder disarms the slot itself ([`disarm_key_capture`]) when the mode ends for its own reasons. A reset of the
//! connection to the helm disarms it too, and that is the one disarm the holder hears about, through the callback it
//! armed with, because nothing else tells it the mode is over.
//!
//! NOTE: The slot sees key-downs only, and some text reaches the focused element's input handler without one: on macOS,
//! a key the input method takes while it composes (marked text, a CJK input source, a dead key's accent) arrives as
//! committed text, never as a key-down, and on Wayland a text-input commit of several characters and a dead key
//! followed by a key it does not compose with are inserted directly. So with the slot armed, a Japanese input source or
//! a French `^` still puts text into a focused terminal's program or a text field. The slot cannot stop it; the text
//! receiver is the place to refuse it, by dropping committed and marked text while [`key_capture_armed`] says so. For
//! a terminal that is its input handler, and for a text field its own input handling. Neither does that yet.
//!
//! ## Shortcuts that act once per press
//!
//! GPUI runs a key binding's action on every auto-repeat of a held key: binding matching reads only the keystroke, and
//! a repeat's keystroke is the same as the press's. SPEC.md says some shortcuts "ignore repeat" (session Delete and
//! Rename among them), so `ONCE_PER_PRESS` lists their actions, and a keystroke interceptor that [`install`] sets up
//! stops a held repeat before GPUI matches it whenever the binding GPUI would try first for that key is one of them.
//! The repeat then neither runs the action again nor reaches anything behind the binding: not the focused element's
//! key listeners, not a focused terminal and so not its program, and not the platform's text input. (GPUI still offers
//! an intercepted key to the first key listener on the focus path, in the capture phase; nothing Farhelm draws listens
//! there.)
//!
//! The guard asks what the key would run, not which chord it is, because a chord means different things in different
//! places: SPEC.md has the Linux Delete chord, Ctrl+Shift+Backspace, leave text editors other than the terminal alone,
//! so where a text field's own binding wins, holding that chord keeps repeating as the field wants. A key that is not
//! bound to a listed action keeps every repeat, which a terminal's program needs for held arrows and letters.
//!
//! The guard relies on the platform marking repeats, which macOS and Wayland both do. (GPUI's X11 backend does not, but
//! Farhelm builds GPUI without X11.)
//!
//! NOTE: The guard does not cover SPEC.md's "ignore composition". IME composition is invisible here: GPUI's key-down
//! event has no composing flag and an interceptor cannot reach the focused input handler. On macOS GPUI offers a key to
//! the input method first while it composes, and a key the input method hands back is dispatched like any other, so a
//! listed shortcut typed mid-composition still runs. The element that owns the input handler knows it is composing and
//! is the place to refuse it; for a terminal, that comes with the terminal's input method work. For the
//! [`PHYSICAL_KEYS`] rows that place is the action's handler instead: the interceptor stops their keys before the
//! focused element sees them, so it cannot refuse one itself, and SPEC.md's "composition do not open it" for Keyboard
//! shortcuts (the old app checked `isComposing`) is the handler's to keep, by asking the focused input whether it is
//! composing (marked text) before it opens anything. That handler is not built here.
//!
//! ## Shortcuts matched by the physical key
//!
//! SPEC.md matches some shortcuts by where the key sits rather than what it prints: the template number keys ("the
//! physical digit key, independent of the printed character; the numeric keypad is excluded"), Keyboard shortcuts ("the
//! physical Slash key, with no extra modifiers") and terminal text size ("the US-layout positions of = and −"). A key
//! binding cannot express that, because GPUI matches bindings by the logical key: on Linux it reports Ctrl+Shift+1 as
//! Ctrl+`!` on a US layout and as something else on others. So [`PHYSICAL_KEYS`] lists them by key, and the same
//! keystroke interceptor that guards repeats matches them before any binding or listener sees the key: it dispatches
//! the row's action to the focused element and stops the key, so neither a focused terminal's program nor a text field
//! receives it, whether or not a feature handles the action yet. A held repeat of a row whose action is in
//! `ONCE_PER_PRESS` is stopped without dispatching; any other row acts on every repeat.
//!
//! A row names its key by the key event's native identity (an XKB position name on Linux, a virtual keycode on macOS),
//! which the vendored GPUI's native-key layer adds (`vendor/README.md`). [`physical_key_at`] finds a row by key alone,
//! and is public so the quick-launch sequence can recognize its bare number keys the same way.

use farhelm_input_mapping::Platform;
use gpui::{
    Action, App, Global, KeyBinding, KeyContext, KeyDownEvent, KeybindingKeystroke, Keystroke,
    KeystrokeEvent, NativeKeyIdentity, NoAction, Subscription, Window, actions,
};

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

/// The key context GPUI Kit's text input sets on the element that holds its focus (`Input` in gpui-base, private
/// there). Named here so Farhelm can bind keys that a text field must keep; the keymap tests check that a focused
/// [`crate::components::TextInput`] really has it, so a Kit upgrade that renames it fails a test rather than silently
/// handing those keys back to the app.
pub const TEXT_INPUT_CONTEXT: &str = "Input";

/// The key context a menu sets on its panel ([`crate::layers::panel`]). Up and Down are bound in it to the
/// every-control focus moves, so they walk every item whatever the Keyboard navigation setting (SPEC.md: "Pop-up
/// suggestion lists and menus use their own arrow keys"), and wrap at both ends, because the move stays inside the
/// topmost layer and GPUI's tab order is a cycle. The old menus did the same (`sidebar.spec.ts` at reference commit
/// `24bc0580`: "the actions menu walks every item and wraps at both ends").
pub const MENU_CONTEXT: &str = "Menu";

/// The Delete shortcut's chord on this platform: Cmd+W on a Mac, Ctrl+Shift+Backspace on Linux (SPEC.md, "Desktop window
/// chrome"). The same chords as its row in [`TERMINAL_KEYS`].
pub(crate) const DELETE_SESSION_CHORD: &str = if cfg!(target_os = "macos") {
    "cmd-w"
} else {
    "ctrl-shift-backspace"
};

/// The Delete shortcut as hover help names it on this platform, in the old app's spelling (`SESSION_DELETE_SHORTCUT` at
/// reference commit `24bc0580`): the session header's Delete names it.
pub(crate) const DELETE_SESSION_SHORTCUT_NAME: &str = if cfg!(target_os = "macos") {
    "⌘W"
} else {
    "Ctrl+Shift+Backspace"
};

/// The Rename shortcut's chords (SPEC.md, "Desktop window chrome"): Cmd+L on a Mac, Ctrl+Shift+L on Linux. Spelled
/// once here, for its binding, its row in [`TERMINAL_KEYS`] and the tests.
const RENAME_SESSION_MAC: &str = "cmd-l";
const RENAME_SESSION_LINUX: &str = "ctrl-shift-l";

/// The Rename shortcut's chord on this platform.
pub(crate) const RENAME_SESSION_CHORD: &str = if cfg!(target_os = "macos") {
    RENAME_SESSION_MAC
} else {
    RENAME_SESSION_LINUX
};

/// The Rename shortcut as hover help names it on this platform, in the old app's spelling (`SESSION_RENAME_SHORTCUT`
/// at reference commit `24bc0580`).
pub(crate) const RENAME_SESSION_SHORTCUT_NAME: &str = if cfg!(target_os = "macos") {
    "⌘L"
} else {
    "Ctrl+Shift+L"
};

/// The quick switcher's chords (SPEC.md, "Desktop window chrome"): Cmd+K on a Mac, Ctrl+Shift+K on Linux. Spelled once
/// here, for its binding, its row in [`TERMINAL_KEYS`] and the tests.
const QUICK_SWITCHER_MAC: &str = "cmd-k";
const QUICK_SWITCHER_LINUX: &str = "ctrl-shift-k";

/// The quick switcher's chord on this platform.
pub(crate) const QUICK_SWITCHER_CHORD: &str = if cfg!(target_os = "macos") {
    QUICK_SWITCHER_MAC
} else {
    QUICK_SWITCHER_LINUX
};

/// New's chord in the Mac app (SPEC.md, "Desktop window chrome"); the Linux app has none. Spelled once here, for its
/// binding and its row in [`TERMINAL_KEYS`]; the tests spell it out on their own.
pub(crate) const NEW_SESSION_MAC: &str = "cmd-n";

/// New's binding (Cmd+N to [`NewSession`] in [`CONTEXT`]), which [`install`] binds on macOS only. A function so the tests,
/// which do not run on macOS, can check and bind it.
pub(crate) fn new_session_binding() -> KeyBinding {
    KeyBinding::new(NEW_SESSION_MAC, NewSession, Some(CONTEXT))
}

/// The quick-launch sequence prefix's chords (SPEC.md, "Quick-launch sequences"): Cmd+X on a Mac, Ctrl+Shift+X on
/// Linux. Spelled once here, for its binding, its row in [`TERMINAL_KEYS`], the sequence's own key rules and the tests.
pub(crate) const SEQUENCE_PREFIX_MAC: &str = "cmd-x";
pub(crate) const SEQUENCE_PREFIX_LINUX: &str = "ctrl-shift-x";

/// The sequence prefix's chord on this platform.
pub(crate) const SEQUENCE_PREFIX_CHORD: &str = if cfg!(target_os = "macos") {
    SEQUENCE_PREFIX_MAC
} else {
    SEQUENCE_PREFIX_LINUX
};

/// The ⚡ chord (SPEC.md, "Quick launch": Cmd+P on a Mac, Ctrl+Shift+P on Linux), spelled once for its binding to
/// [`QuickLaunch`], its row in [`TERMINAL_KEYS`] and an armed sequence, which queues the marked template on it.
pub(crate) const QUICK_LAUNCH_MAC: &str = "cmd-p";
pub(crate) const QUICK_LAUNCH_LINUX: &str = "ctrl-shift-p";

/// The ⚡ chord on this platform.
pub(crate) const QUICK_LAUNCH_CHORD: &str = if cfg!(target_os = "macos") {
    QUICK_LAUNCH_MAC
} else {
    QUICK_LAUNCH_LINUX
};

/// The chords a focused text field keeps for itself although Farhelm binds them elsewhere. [`install`] binds each to
/// `NoAction` in [`TEXT_INPUT_CONTEXT`], which hides every binding further out for the same chord, so the key goes on
/// to the field's own handling (here, nothing: Kit binds none of them). Bound on every platform: on macOS the Delete
/// chord is Cmd+W and Ctrl+Shift+Backspace is not Farhelm's, so the row changes nothing there.
const TEXT_FIELD_KEYS: &[&str] = &[
    // SPEC.md, "Desktop window chrome": "The Linux chord leaves text editors other than the terminal alone."
    "ctrl-shift-backspace",
    // SPEC.md, "Desktop window chrome": "Text fields outside the session launcher keep their normal Cut or
    // text-direction key". On Linux the prefix is Ctrl+Shift+X, which Kit binds to nothing, so without this a focused
    // field would hand it on to the prefix's binding. On a Mac the prefix is Cmd+X, which Kit's own Cut binding in its
    // input context already outranks; this row changes nothing there.
    SEQUENCE_PREFIX_LINUX,
];

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
        /// Delete the open session through its header Delete action, with that action's confirmation and busy rules
        /// (SPEC.md: Cmd+W on a Mac, Ctrl+Shift+Backspace on Linux). Runs once per press (`ONCE_PER_PRESS`).
        DeleteSession,
        /// Open Rename for the open session (SPEC.md: Cmd+L on a Mac, Ctrl+Shift+L on Linux). Runs once per press
        /// (`ONCE_PER_PRESS`).
        RenameSession,
        /// Open the quick switcher, to find and open any session by typing part of its title, host or folder
        /// (SPEC.md: Cmd+K on a Mac, Ctrl+Shift+K on Linux, also from a focused terminal). Runs once per press
        /// (`ONCE_PER_PRESS`).
        QuickSwitcher,
        /// Open New, the session launcher with New's prefill (SPEC.md, "Desktop window chrome": Cmd+N in the Mac
        /// app only, also from a focused terminal; the Linux app has no New shortcut). Runs once per press
        /// (`ONCE_PER_PRESS`).
        NewSession,
        /// Begin a quick-launch sequence, which takes every key until Enter, Escape or a cancel (SPEC.md,
        /// "Quick-launch sequences": Cmd+X on a Mac, Ctrl+Shift+X on Linux, also from a focused terminal; a text field
        /// keeps the chord, `TEXT_FIELD_KEYS`). Runs once per press (`ONCE_PER_PRESS`).
        BeginSequence,
        /// Quick launch the template marked ⚡, or apply it to an open launcher's draft (SPEC.md, "Quick launch": Cmd+P on
        /// a Mac, Ctrl+Shift+P on Linux, the operating system's chord, also from a focused terminal). Runs once per press
        /// (`ONCE_PER_PRESS`).
        QuickLaunch,
        /// Paste the clipboard's text into the focused terminal, as a paste its program can tell from typing. The
        /// terminal handles it; its keys are its rows of [`TERMINAL_KEYS`], and a menu item that dispatches it reaches
        /// the focused terminal the same way.
        Paste,
        /// Scroll the focused terminal back through its history by a page less a line (SPEC.md, "Reaching everything
        /// by keyboard": Shift+PageUp; the old app's step). The terminal handles it; its key is its row of
        /// [`TERMINAL_KEYS`], so the program never gets the key. Acts on every repeat of a held key, as the old app
        /// did.
        ScrollTerminalPageUp,
        /// Scroll the focused terminal toward its live screen by a page less a line (Shift+PageDown; otherwise as
        /// [`ScrollTerminalPageUp`]).
        ScrollTerminalPageDown,
        /// Select everything the focused terminal can show, its history included (Cmd+A on a Mac, the old app's
        /// terminal key). The terminal handles it; selecting does not copy.
        SelectAllInTerminal,
        /// Copy the focused terminal's selection to the clipboard (Cmd+C on a Mac, WebKit's copy in the old app's
        /// terminal). The terminal handles it; with nothing selected it does nothing.
        CopyTerminalSelection,
        /// Open the Keyboard shortcuts dialog (SPEC.md, "Desktop window chrome": Cmd+/ on a Mac, Ctrl+Shift+/ on
        /// Linux, by the physical Slash key; a row in [`PHYSICAL_KEYS`]). Runs once per press (`ONCE_PER_PRESS`).
        ShowKeyboardShortcuts,
        /// Step every open terminal's text size up (SPEC.md, "Terminal experience": Cmd+Shift or Ctrl+Shift with the
        /// key at the US position of =; a row in [`PHYSICAL_KEYS`]). Acts on every repeat of a held chord, as the old
        /// app did.
        IncreaseTerminalTextSize,
        /// Step every open terminal's text size down (the key at the US position of −; otherwise as
        /// [`IncreaseTerminalTextSize`]).
        DecreaseTerminalTextSize,
        /// Quit the app the ordinary way, so every quit path runs the host's quit handler once
        /// (`bootstrap::Host::on_quit`; SPEC.md, "Desktop window chrome": Cmd+Q closes the app). The Mac app menu's Quit
        /// item and Cmd+Q; handled app-wide by the desktop module.
        Quit,
        /// Hide the app (the Mac app menu's Hide, Cmd+H); handled app-wide by the desktop module.
        HideApp,
        /// Hide every other app (the Mac app menu's Hide Others, Option-Cmd+H).
        HideOtherApps,
        /// Show every app again (the Mac app menu's Show All).
        ShowAllApps,
        /// Minimize the active window (the Mac app menu's Minimize, Cmd+M).
        MinimizeWindow,
        /// Zoom the active window, or restore it from zoom (the Mac app menu's Zoom, which has no key).
        ZoomWindow,
        /// Enter or leave native fullscreen (the Mac app menu's Toggle Full Screen, Ctrl+Cmd+F).
        ToggleFullScreen,
        /// Open the Open-Source Notices window, or bring the open one forward (the Mac app menu's Open-Source Notices,
        /// which has no key; on Linux the `?` menu's item). Handled app-wide by the desktop module.
        OpenSourceNotices,
    ]
);

/// Builds one chord's binding to its action ([`MAC_MENU_CHORDS`]): a function, because GPUI builds a binding from a
/// concrete action type.
type BindChord = fn(&'static str) -> KeyBinding;

/// The Mac app menu's chords, in AppKit's standard spelling (the old app's menu items carried the same ones): bound in
/// [`CONTEXT`] by [`install`] on macOS only, before the desktop module installs the menu that shows them. Each is an
/// app shortcut row in [`TERMINAL_KEYS`], so a focused terminal's program never receives it. Linux has no menu bar and
/// binds none of them; its window's close button quits (SPEC.md, "Desktop window chrome").
pub(crate) const MAC_MENU_CHORDS: &[(&str, BindChord)] = &[
    ("cmd-q", |chord| KeyBinding::new(chord, Quit, Some(CONTEXT))),
    ("cmd-h", |chord| {
        KeyBinding::new(chord, HideApp, Some(CONTEXT))
    }),
    ("alt-cmd-h", |chord| {
        KeyBinding::new(chord, HideOtherApps, Some(CONTEXT))
    }),
    ("cmd-m", |chord| {
        KeyBinding::new(chord, MinimizeWindow, Some(CONTEXT))
    }),
    ("ctrl-cmd-f", |chord| {
        KeyBinding::new(chord, ToggleFullScreen, Some(CONTEXT))
    }),
];

/// Paste's chords. The Mac's is the system's; the old app's terminal got it from WebKit. Linux has two because the
/// desktop terminals there do (Ctrl+Shift+V, since plain Ctrl+V is the program's, and Shift+Insert); the old app had
/// no Linux window, so this is the GPUI migration's choice, recorded in the terminal area's goal on 2026-11-14.
const PASTE_MAC: &str = "cmd-v";
const PASTE_LINUX: &str = "ctrl-shift-v";
const PASTE_LINUX_INSERT: &str = "shift-insert";

/// Start the template that holds number key `digit` (SPEC.md, "Template number shortcuts": Cmd+digit on a Mac,
/// Ctrl+Shift+digit on Linux, by the physical digit key; ten rows in [`PHYSICAL_KEYS`]). One action carries the digit,
/// rather than one action per key, so a feature handles every number key in one place. Runs once per press
/// (`ONCE_PER_PRESS`). Never built from JSON: no keymap file names it, and the physical rows are its only source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, gpui::Action)]
#[action(namespace = farhelm, no_json)]
pub struct TemplateNumberKey {
    /// The key's digit, 0 to 9 (SPEC.md lists them 1–9 then 0).
    pub digit: u8,
}

/// Bind the keys. The bootstrap calls this after GPUI Kit's own init, so Farhelm's bindings are added after Kit's.
pub fn install(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", FocusNext, Some(CONTEXT)),
        KeyBinding::new("shift-tab", FocusPrevious, Some(CONTEXT)),
        // Option-Tab on macOS. Bound on every platform: where Tab already reaches every control it does the same as
        // Tab, and on Linux the window manager usually keeps Alt-Tab for itself.
        KeyBinding::new("alt-tab", FocusNextControl, Some(CONTEXT)),
        KeyBinding::new("alt-shift-tab", FocusPreviousControl, Some(CONTEXT)),
        KeyBinding::new("escape", Dismiss, Some(CONTEXT)),
        KeyBinding::new("down", FocusNextControl, Some(MENU_CONTEXT)),
        KeyBinding::new("up", FocusPreviousControl, Some(MENU_CONTEXT)),
        // SPEC.md, "Desktop window chrome". The main view handles it for the open session; it works from a focused
        // terminal because its `TERMINAL_KEYS` row is an app shortcut, and a text field keeps the Linux chord
        // (`TEXT_FIELD_KEYS`).
        KeyBinding::new(DELETE_SESSION_CHORD, DeleteSession, Some(CONTEXT)),
        // SPEC.md, "Desktop window chrome". The main view hands it to the session list's Rename editor for the open
        // session; it works from a focused terminal because its `TERMINAL_KEYS` row is an app shortcut.
        KeyBinding::new(RENAME_SESSION_CHORD, RenameSession, Some(CONTEXT)),
        // SPEC.md, "Desktop window chrome". The main view hands it to the session list's quick switcher; it works from
        // a focused terminal because its `TERMINAL_KEYS` row is an app shortcut.
        KeyBinding::new(QUICK_SWITCHER_CHORD, QuickSwitcher, Some(CONTEXT)),
        // SPEC.md, "Quick-launch sequences". The main view hands it to the templates module, which arms the sequence;
        // it works from a focused terminal because its `TERMINAL_KEYS` row is an app shortcut, and a text field keeps
        // it (`TEXT_FIELD_KEYS`, and on a Mac GPUI Kit's own Cut).
        KeyBinding::new(SEQUENCE_PREFIX_CHORD, BeginSequence, Some(CONTEXT)),
        // SPEC.md, "Quick launch". The main view hands it to the templates module's quick-launch dispatcher; it works
        // from a focused terminal because its `TERMINAL_KEYS` row is an app shortcut.
        KeyBinding::new(QUICK_LAUNCH_CHORD, QuickLaunch, Some(CONTEXT)),
    ]);
    if cfg!(target_os = "macos") {
        // SPEC.md, "Desktop window chrome": Cmd+N in the Mac app only. The main view hands it to the launcher's New; it
        // works from a focused terminal because its `TERMINAL_KEYS` row is an app shortcut.
        cx.bind_keys([new_session_binding()]);
        // Before the desktop module installs the Mac menu, which shows each item's chord only if it is bound by then.
        cx.bind_keys(MAC_MENU_CHORDS.iter().map(|(chord, bind)| bind(chord)));
    }
    cx.bind_keys(
        TEXT_FIELD_KEYS
            .iter()
            .map(|chord| KeyBinding::new(chord, NoAction, Some(TEXT_INPUT_CONTEXT))),
    );
    // `NoAction` in the deeper context suppresses every binding for the same keys further out, Farhelm's and Kit's
    // alike, so the key goes on to the terminal's own listener. App shortcuts are left alone: their features bind them
    // in `CONTEXT`, and those bindings must keep working from a terminal.
    //
    // Binding order matters in the terminal's own context too. GPUI ranks bindings of equal depth by when they were
    // added, latest first, and a `NoAction` suppresses everything ranked after it. So the terminal's own actions (the
    // rows' `bind`) work only because they are bound after these rows; bound before them, their keys would silently do
    // nothing. The same ranking lets a binding with no context that is added after this one beat a row here; the keymap
    // walk test catches that case.
    cx.bind_keys(
        TERMINAL_KEYS
            .iter()
            .filter(|key| key.owner != KeyOwner::App)
            .filter_map(|key| key.chord(THIS_PLATFORM))
            .map(|chord| KeyBinding::new(chord, NoAction, Some(TERMINAL_CONTEXT))),
    );
    cx.bind_keys(TERMINAL_KEYS.iter().filter_map(|key| {
        let chord = key.chord(THIS_PLATFORM)?;
        key.bind.map(|bind| bind(chord))
    }));
    let interceptor = cx.intercept_keystrokes(intercept);
    cx.set_global(KeyInterceptor {
        _interceptor: interceptor,
    });
}

/// The keystroke interceptor, which sees every key before GPUI matches a binding for it: it hands the key to an armed
/// key capture first, runs the shortcuts [`PHYSICAL_KEYS`] matches by position, and stops a held repeat of a key that
/// would run a `ONCE_PER_PRESS` action.
///
/// The capture goes first so that it sees the number-key chords and Escape before their rows and bindings take them; a
/// key it cancels on is judged by the rest as it would be with nothing armed (a held repeat stays a repeat).
///
/// Before all of that, the focus coordinator records the key for "the user moved on" (`focus::note_press`), so that a
/// layer the capture or a row's action closes is taken as closed by this press. A key it swallows, a repeat of a press
/// that closed a layer, still goes to an armed capture first, which decides what a repeat does, and is then stopped
/// before any row or binding acts on it.
///
/// It does not check GPUI's character-input preference, under which bindings would not run at all: on macOS and
/// Wayland only GPUI's replay of a held-back chord key asks for it, and a replay never reaches an interceptor. Were a
/// platform to ask for it, dropping a listed shortcut's repeat would still be what SPEC.md wants.
fn intercept(event: &KeystrokeEvent, window: &mut Window, cx: &mut App) {
    let Some(down) = &event.key_down else {
        return;
    };
    let swallowed = crate::focus::note_press(down, window, cx);
    if captured(down, window, cx) || swallowed {
        cx.stop_propagation();
    } else if let Some(row) = physical_on(THIS_PLATFORM, down) {
        if !(down.is_held && once_per_press(row.action)) {
            window.dispatch_action(row.action.boxed_clone(), cx);
        }
        cx.stop_propagation();
    } else if down.is_held && runs_once_per_press(&down.keystroke, &event.context_stack, cx) {
        cx.stop_propagation();
    }
}

// ---- The key capture slot --------------------------------------------------------------------------------------------

/// What a key capture's handler does with a key-down it was handed ([`arm_key_capture`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Captured {
    /// The handler took the key, and it goes no further: no physical-key row, binding, focused element, terminal
    /// program or text input sees it. The slot stays armed.
    Consumed,
    /// The capture is over and the key is not its: the slot disarms, and the key goes on as though nothing had been
    /// armed, to its row, its binding or the focused element. SPEC.md's non-sequence keys, which cancel the sequence
    /// and still reach their normal target.
    Cancel,
}

/// A capture's key handler: handed each key-down, with the window it was typed in.
type CaptureKey = Box<dyn FnMut(&KeyDownEvent, &mut Window, &mut App) -> Captured>;

/// What a capture runs when a reset of the connection disarms it.
type CaptureReset = Box<dyn FnOnce(&mut App)>;

/// The one app-wide key capture slot. A global rather than a field of the interceptor because arming happens from
/// anywhere in the app (an action handler), and the interceptor closure cannot be reached from there.
#[derive(Default)]
enum KeyCapture {
    #[default]
    Idle,
    Armed {
        on_key: CaptureKey,
        on_reset: CaptureReset,
    },
    /// The handler is running. It is out of the slot while it runs, so it can be handed the app it lives in, and so
    /// may disarm the slot or open and close layers itself (arming is refused until it returns). `on_reset` is `None`
    /// once something disarmed the slot meanwhile, in which case the handler is dropped instead of put back when it
    /// returns.
    Running { on_reset: Option<CaptureReset> },
}

impl Global for KeyCapture {}

/// Arm the app's one key capture slot: until it is disarmed, every key-down in every window goes to `on_key` before
/// anything else sees it, a focused terminal's program and the dismiss binding on Escape included (the module docs'
/// "Taking every key for a while"). The arming key's own held repeats arrive there too, since the handler decides
/// what a repeat does. `on_reset` runs once if a reset of the connection to the helm disarms the slot; it does not run
/// on [`disarm_key_capture`] or on a [`Captured::Cancel`], which the holder already knows about.
///
/// Returns false, dropping both closures, when the slot is already armed or its handler is running: a second mode
/// that took the keys from the first would leave the first showing as armed with nothing feeding it, so the holder
/// that armed it disarms it before anyone else can arm. A handler that disarms the slot and wants a new capture arms
/// it after it returns.
#[must_use = "arming is refused while the slot is armed"]
pub fn arm_key_capture(
    on_key: impl FnMut(&KeyDownEvent, &mut Window, &mut App) -> Captured + 'static,
    on_reset: impl FnOnce(&mut App) + 'static,
    cx: &mut App,
) -> bool {
    let slot = cx.default_global::<KeyCapture>();
    if !matches!(slot, KeyCapture::Idle) {
        return false;
    }
    *slot = KeyCapture::Armed {
        on_key: Box::new(on_key),
        on_reset: Box::new(on_reset),
    };
    true
}

/// Disarm the key capture slot, from its holder or its own handler; keys go their normal way from the next one on.
/// Returns whether the slot was armed. Runs no callback. Called from inside the handler, it leaves the key being
/// handled to the handler's answer: a handler that ends the mode on a key it keeps (Enter, say) disarms and answers
/// [`Captured::Consumed`].
pub fn disarm_key_capture(cx: &mut App) -> bool {
    let slot = cx.default_global::<KeyCapture>();
    match slot {
        KeyCapture::Idle => false,
        KeyCapture::Armed { .. } => {
            *slot = KeyCapture::Idle;
            true
        }
        KeyCapture::Running { on_reset } => on_reset.take().is_some(),
    }
}

/// Whether the key capture slot is armed (a running handler that has not disarmed it counts). For a text receiver
/// deciding whether to drop input-method text (the module docs' NOTE), not for drawing: the slot notifies no view when
/// it changes, so a holder keeps its own mode state.
pub fn key_capture_armed(cx: &App) -> bool {
    matches!(
        cx.try_global::<KeyCapture>(),
        Some(KeyCapture::Armed { .. } | KeyCapture::Running { on_reset: Some(_) })
    )
}

/// Disarm the key capture slot for a reset of the connection to the helm, and tell its holder through its `on_reset`.
/// The UI-session owner calls this as the window goes inert, beside the layer manager's `close_all`. SPEC.md, "Losing
/// the connection to the helm": apart from the selected session, the window and settings, "Everything else starts
/// over: open dialogs and menus, unsaved edits and half-typed launcher drafts included", and a half-typed sequence is
/// one of those; a capture that outlived the reset would also go on eating keys after its holder's mode had ended.
pub(crate) fn reset_key_capture(cx: &mut App) {
    let slot = cx.default_global::<KeyCapture>();
    let on_reset = match std::mem::take(slot) {
        KeyCapture::Idle => None,
        KeyCapture::Armed { on_reset, .. } => Some(on_reset),
        KeyCapture::Running { on_reset } => {
            // The handler is still out of the slot; it returns to find the capture over.
            *slot = KeyCapture::Running { on_reset: None };
            on_reset
        }
    };
    // Run after the slot is clear, so the callback may arm a new capture, unless a handler is running: the `Running`
    // branch above is defensive, since a reset arrives from the channel's feed, never from inside a key handler.
    if let Some(on_reset) = on_reset {
        on_reset(cx);
    }
}

/// Hand `down` to the armed capture, if there is one, and say whether it consumed the key. A [`Captured::Cancel`]
/// leaves the key propagating, whatever the handler's own dispatching did to GPUI's propagation flag, so the rest of
/// the interceptor and GPUI see it as they would with nothing armed.
fn captured(down: &KeyDownEvent, window: &mut Window, cx: &mut App) -> bool {
    // Read first: reaching the slot mutably queues a global-observer effect, which every key would pay for.
    if !matches!(
        cx.try_global::<KeyCapture>(),
        Some(KeyCapture::Armed { .. })
    ) {
        return false;
    }
    let slot = cx.default_global::<KeyCapture>();
    let mut on_key = match std::mem::take(slot) {
        KeyCapture::Armed { on_key, on_reset } => {
            *slot = KeyCapture::Running {
                on_reset: Some(on_reset),
            };
            on_key
        }
        other => {
            *slot = other;
            return false;
        }
    };
    let answer = on_key(down, window, cx);
    let slot = cx.default_global::<KeyCapture>();
    // Arming is refused while the handler runs, so the slot is still `Running` here, and left `Idle` by this take. It
    // is put back only if neither the answer nor a disarm from inside the handler ended the capture.
    let still_armed = match std::mem::take(slot) {
        KeyCapture::Running { on_reset } => on_reset,
        _ => None,
    };
    if let (Some(on_reset), Captured::Consumed) = (still_armed, answer) {
        *slot = KeyCapture::Armed { on_key, on_reset };
    }
    match answer {
        Captured::Consumed => true,
        Captured::Cancel => {
            cx.propagate();
            false
        }
    }
}

// ---- Shortcuts that act once per press -------------------------------------------------------------------------------

/// The actions a held key runs once, on the press, and not again on each auto-repeat: the ones behind SPEC.md's
/// shortcuts that "ignore repeat". A shortcut opts in by having its action listed here; its binding stays where every
/// binding is, in [`install`], or its row in [`PHYSICAL_KEYS`], whose dispatcher drops a held repeat of a listed
/// action itself. The module docs say what the guard covers and what it cannot.
///
/// A listed action's handler must take every key that reaches it, even when it does nothing (Delete with no session
/// open). The guard judges a repeat by the first binding GPUI tries for the key; a handler that declined would send
/// the press on to the next binding or to the key listeners, and its repeats would not follow it there.
///
/// Entries are compared by action name, not value, so the one [`TemplateNumberKey`] entry covers every digit.
const ONCE_PER_PRESS: &[&dyn Action] = &[
    // One entry per line, so that a new shortcut only adds a line (the shape `docs/gpui/foundation-ownership.md` asks
    // of the lists areas append to); this comment is also what keeps rustfmt from joining the entries.
    &DeleteSession,
    &RenameSession,
    &TemplateNumberKey { digit: 0 },
    &ShowKeyboardShortcuts,
    // SPEC.md does not name this one: its "ignore repeat" ("Desktop window chrome") covers the two session shortcuts.
    // The migration's orchestrator decided on 2026-11-14 that a held opening shortcut acts once per press, so it is
    // listed the same way and a held chord opens the switcher once; a repeat would only reach the ground's handler,
    // which refuses while the switcher is open, and listed, it never runs the opener again.
    &QuickSwitcher,
    // Opening New, like opening the switcher above: a held Cmd+N opens it once.
    &NewSession,
    // SPEC.md, "Quick-launch sequences": "Repeated held keys add nothing". While a sequence is armed its own capture
    // takes the prefix's repeats; listed, a repeat that arrives after a cancel (a click while the chord is still held)
    // cannot arm a new sequence either.
    &BeginSequence,
    // SPEC.md, "Quick launch": "A held chord starts only once".
    &QuickLaunch,
    // The Mac app menu's chords (`MAC_MENU_CHORDS`): a GPUI binding runs on every repeat of a held key, so without
    // these a held Ctrl+Cmd+F would flip full screen on and off, and a held Cmd+M, Cmd+H or Cmd+Q act again on each
    // repeat, which none of them can usefully do.
    &ToggleFullScreen,
    &MinimizeWindow,
    &HideApp,
    &HideOtherApps,
    &Quit,
];

/// Whether `action` is one a held key runs only once: its name is listed in [`ONCE_PER_PRESS`].
fn once_per_press(action: &dyn Action) -> bool {
    ONCE_PER_PRESS
        .iter()
        .any(|listed| listed.name() == action.name())
}

/// Keeps the keystroke interceptor ([`intercept`]) subscribed for the app's lifetime.
struct KeyInterceptor {
    _interceptor: Subscription,
}

impl Global for KeyInterceptor {}

/// Whether `keystroke`, typed where the focused element's context stack is `context_stack`, would run a
/// [`ONCE_PER_PRESS`] action: whether the binding GPUI tries first for it is bound to one. A deeper context's binding
/// for the same chord wins, as it does in GPUI's dispatch, and a `NoAction` row hides the bindings it suppresses.
///
/// Matches the key alone, as GPUI does when no chord is pending. The listed shortcuts are single keys bound in
/// [`CONTEXT`], which holds no chord prefix (SPEC_impl.md forbids one in any context that contains a terminal), so
/// there the press settles any pending chord before its repeats arrive. A deeper context that started a chord with one
/// of them would have its repeats judged as if the chord were not there.
fn runs_once_per_press(keystroke: &Keystroke, context_stack: &[KeyContext], cx: &App) -> bool {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let (bindings, _) = keymap.bindings_for_input(std::slice::from_ref(keystroke), context_stack);
    bindings
        .first()
        .is_some_and(|binding| once_per_press(binding.action()))
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
    /// terminal takes it. Once the command has an action, its row's [`TerminalKey::bind`] binds the key to it in
    /// [`TERMINAL_CONTEXT`]; until then the key reaches the terminal's listener, which drops it.
    /// (`farhelm-input-mapping` still classifies paging and select all as well, and a test compares the two. Its paging
    /// rule also covers Shift+PageUp and Shift+PageDown with more modifiers held, which the terminal pages on, as the
    /// old app did.)
    Terminal,
    /// The program in the terminal. Every key not in [`TERMINAL_KEYS`] is the program's; the table lists the ones that
    /// some binding outside the terminal would otherwise take.
    Program,
}

/// One row of [`TERMINAL_KEYS`]: a chord in GPUI's keystroke syntax on each platform (`None` where the platform has no
/// such chord), who owns it, what it is, and, for a terminal command that has an action, how its chord binds to it.
#[derive(Clone, Copy, Debug)]
pub struct TerminalKey {
    pub mac: Option<&'static str>,
    pub linux: Option<&'static str>,
    pub owner: KeyOwner,
    pub what: &'static str,
    /// For a [`KeyOwner::Terminal`] row whose command has an action: binds a chord of the row to that action in
    /// [`TERMINAL_CONTEXT`], so the binding takes the key before the terminal's listener would drop it (a function
    /// rather than the action itself, because GPUI builds a binding from a concrete action type; a test reads the
    /// action back off a built binding). [`install`] binds it after the table's `NoAction` rows, because GPUI ranks
    /// bindings of equal depth latest first and a `NoAction` hides every binding ranked after it. `None` on every
    /// other row; the keymap tests fail on one outside a terminal row.
    pub bind: Option<fn(&'static str) -> KeyBinding>,
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
/// grid terminal's (Shift+PageUp/PageDown, and Cmd+A on a Mac), plus the Mac's Cmd+C and Cmd+V, which were WebKit's
/// copy and paste in the old app's terminal, and Linux's paste chords (the paste rows below). Linux has no copy chord:
/// neither SPEC.md nor the old app's terminal settles one, and every completed selection is copied as it is made.
///
/// NOTE: Not in this table, because a key binding cannot express them: the number keys (Cmd or Ctrl+Shift with 0-9),
/// the Keyboard shortcuts dialog (with `/`) and terminal text size (with `=` and `-`), which SPEC.md matches by the
/// physical key. They are rows of [`PHYSICAL_KEYS`], whose interceptor takes them before any binding or the terminal
/// sees them, so they never reach the program either.
pub const TERMINAL_KEYS: &[TerminalKey] = &[
    // Farhelm's shortcuts that work from a focused terminal.
    app(
        QUICK_SWITCHER_MAC,
        QUICK_SWITCHER_LINUX,
        "open the quick switcher",
    ),
    app("cmd-j", "ctrl-shift-j", "open the template list"),
    app(
        QUICK_LAUNCH_MAC,
        QUICK_LAUNCH_LINUX,
        "quick launch the marked template",
    ),
    app(
        SEQUENCE_PREFIX_MAC,
        SEQUENCE_PREFIX_LINUX,
        "begin a quick-launch sequence",
    ),
    TerminalKey {
        mac: Some(NEW_SESSION_MAC),
        // SPEC.md: the Linux app has no New shortcut.
        linux: None,
        owner: KeyOwner::App,
        what: "open New",
        bind: None,
    },
    app("cmd-w", "ctrl-shift-backspace", "delete the open session"),
    app(
        RENAME_SESSION_MAC,
        RENAME_SESSION_LINUX,
        "rename the open session",
    ),
    // The Mac app menu's chords (`MAC_MENU_CHORDS`); Linux has no menu bar.
    mac_app("cmd-q", "quit the app"),
    mac_app("cmd-h", "hide the app"),
    mac_app("alt-cmd-h", "hide every other app"),
    mac_app("cmd-m", "minimize the window"),
    mac_app("ctrl-cmd-f", "enter or leave full screen"),
    // The terminal's own commands.
    TerminalKey {
        mac: Some("shift-pageup"),
        linux: Some("shift-pageup"),
        owner: KeyOwner::Terminal,
        what: "scroll the terminal up a page",
        bind: Some(bind_page_up),
    },
    TerminalKey {
        mac: Some("shift-pagedown"),
        linux: Some("shift-pagedown"),
        owner: KeyOwner::Terminal,
        what: "scroll the terminal down a page",
        bind: Some(bind_page_down),
    },
    TerminalKey {
        mac: Some("cmd-a"),
        linux: None,
        owner: KeyOwner::Terminal,
        what: "select all of the terminal's text",
        bind: Some(bind_select_all),
    },
    TerminalKey {
        mac: Some("cmd-c"),
        linux: None,
        owner: KeyOwner::Terminal,
        what: "copy the terminal's selection (outranking GPUI Kit's root binding of Copy)",
        bind: Some(bind_copy),
    },
    TerminalKey {
        mac: Some(PASTE_MAC),
        linux: Some(PASTE_LINUX),
        owner: KeyOwner::Terminal,
        what: "paste into the terminal",
        bind: Some(bind_paste),
    },
    TerminalKey {
        mac: None,
        linux: Some(PASTE_LINUX_INSERT),
        owner: KeyOwner::Terminal,
        what: "paste into the terminal (Shift+Insert, which the program no longer gets)",
        bind: Some(bind_paste),
    },
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
        bind: None,
    },
    TerminalKey {
        mac: Some("cmd-alt-i"),
        linux: Some("ctrl-shift-i"),
        owner: KeyOwner::Program,
        what: "GPUI Kit's inspector toggle, bound in debug builds only",
        bind: None,
    },
];

/// Paste's binding, for its rows of [`TERMINAL_KEYS`].
fn bind_paste(chord: &'static str) -> KeyBinding {
    KeyBinding::new(chord, Paste, Some(TERMINAL_CONTEXT))
}

/// Shift+PageUp's binding, for its row of [`TERMINAL_KEYS`].
fn bind_page_up(chord: &'static str) -> KeyBinding {
    KeyBinding::new(chord, ScrollTerminalPageUp, Some(TERMINAL_CONTEXT))
}

/// Shift+PageDown's binding, for its row of [`TERMINAL_KEYS`].
fn bind_page_down(chord: &'static str) -> KeyBinding {
    KeyBinding::new(chord, ScrollTerminalPageDown, Some(TERMINAL_CONTEXT))
}

/// Cmd+A's binding, for its row of [`TERMINAL_KEYS`].
fn bind_select_all(chord: &'static str) -> KeyBinding {
    KeyBinding::new(chord, SelectAllInTerminal, Some(TERMINAL_CONTEXT))
}

/// Cmd+C's binding, for its row of [`TERMINAL_KEYS`].
fn bind_copy(chord: &'static str) -> KeyBinding {
    KeyBinding::new(chord, CopyTerminalSelection, Some(TERMINAL_CONTEXT))
}

/// An app shortcut: Command on a Mac, Ctrl+Shift on Linux.
const fn app(mac: &'static str, linux: &'static str, what: &'static str) -> TerminalKey {
    TerminalKey {
        mac: Some(mac),
        linux: Some(linux),
        owner: KeyOwner::App,
        what,
        bind: None,
    }
}

/// An app shortcut that exists only in the Mac app (the app menu's chords).
const fn mac_app(mac: &'static str, what: &'static str) -> TerminalKey {
    TerminalKey {
        mac: Some(mac),
        linux: None,
        owner: KeyOwner::App,
        what,
        bind: None,
    }
}

/// A chord that is the same on both platforms.
const fn both(chord: &'static str, owner: KeyOwner, what: &'static str) -> TerminalKey {
    TerminalKey {
        mac: Some(chord),
        linux: Some(chord),
        owner,
        what,
        bind: None,
    }
}

/// Who the key pressed in `event`, in a focused terminal on this platform, belongs to. The terminal calls this for every
/// key that reaches its listener: it drops a [`KeyOwner::App`] or [`KeyOwner::Terminal`] key, press and held repeats
/// alike, and hands a [`KeyOwner::Program`] key to its input mapping. Once an app shortcut is bound, the binding takes
/// the key and this is never asked; a held repeat then runs the binding again, unless its action is in
/// `ONCE_PER_PRESS`, whose guard stops the repeat before the binding or the terminal sees it.
///
/// A key a binding handled never reaches the terminal, so this answers only for the keys that do. It matches the way
/// GPUI matches a binding ([`Keystroke::should_match`]), so a key counts as a chord here exactly when a binding for
/// that chord would take it. The shortcuts matched by the physical key never get here: [`PHYSICAL_KEYS`]' interceptor
/// stops them first.
pub fn terminal_key_owner(event: &KeyDownEvent) -> KeyOwner {
    owner_on(THIS_PLATFORM, event)
}

/// [`terminal_key_owner`] on `platform`, so the Mac's answers can be tested where the suite runs, on Linux. Crate-only:
/// the app asks about the platform it runs on, through [`terminal_key_owner`]; only the tests ask about another.
pub(crate) fn owner_on(platform: Platform, event: &KeyDownEvent) -> KeyOwner {
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

// ---- Shortcuts matched by the physical key ---------------------------------------------------------------------------

/// A key named by its place on the keyboard rather than by what it prints: its native identity on each platform, and
/// the character a US layout prints there unshifted, for events that carry no identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalKeyId {
    /// The XKB position name, on Linux: `AE01` is the main row's first digit key whatever the layout prints on it, and
    /// the keypad's keys have names of their own (`KP1`).
    pub xkb: &'static str,
    /// The macOS virtual keycode (the `kVK_ANSI_*` codes), which also names a position rather than a character.
    pub mac: u16,
    /// What a US layout prints on the key unshifted.
    pub us: &'static str,
}

impl PhysicalKeyId {
    /// Whether `event` was typed at this key: by the event's native identity when it has one, and by its logical key
    /// as a US layout prints it unshifted when it has none.
    ///
    /// The native identity is what SPEC.md asks for ("independent of the printed character"), and it is never
    /// second-guessed: an identity that names another key is another key, even when the layout printed this one's
    /// character. The logical fallback applies only to events without any identity, and for the chords in
    /// [`PHYSICAL_KEYS`] it does not undercut that rule: on real hardware the events without an identity are
    /// input-method commits of ASCII text and replayed keystrokes (`vendor/README.md`, "The patch layers"), never a Cmd
    /// or Ctrl+Shift chord. It exists so the keys the test bridge injects, and simulated test keys, can press these
    /// shortcuts.
    ///
    /// NOTE: For a bare key (the quick-launch sequence's number keys, which take no modifiers) that reasoning does not
    /// hold: a digit an input method commits, the keypad's included, has no identity and reads as the main row's
    /// digit here. A caller that matches bare keys decides whether that matters to it.
    pub fn typed_in(&self, event: &KeyDownEvent) -> bool {
        match &event.native_key {
            Some(native) => match &native.identity {
                NativeKeyIdentity::Xkb(name) => name.as_ref() == self.xkb,
                NativeKeyIdentity::Mac(code) => *code == self.mac,
            },
            None => event.keystroke.key == self.us,
        }
    }
}

/// The key at `xkb` (Linux), `mac` (macOS), printing `us` on a US layout.
const fn key(xkb: &'static str, mac: u16, us: &'static str) -> PhysicalKeyId {
    PhysicalKeyId { xkb, mac, us }
}

/// The modifiers a physical-key shortcut is typed with, exactly: Option (Alt) and Fn are never part of one, and any
/// modifier beyond these keeps the chord from matching (SPEC.md: "with no extra modifiers").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChordModifiers {
    /// Command on a Mac (GPUI's `platform` modifier, Super on Linux).
    pub command: bool,
    /// Ctrl.
    pub control: bool,
    /// Shift, as the key event's original Shift (a symbol's logical keystroke drops it).
    pub shift: bool,
}

/// Command alone.
const COMMAND: ChordModifiers = ChordModifiers {
    command: true,
    control: false,
    shift: false,
};
/// Command and Shift.
const COMMAND_SHIFT: ChordModifiers = ChordModifiers {
    command: true,
    control: false,
    shift: true,
};
/// Ctrl and Shift, the Linux app's form of every app shortcut.
const CONTROL_SHIFT: ChordModifiers = ChordModifiers {
    command: false,
    control: true,
    shift: true,
};

/// One row of [`PHYSICAL_KEYS`]: the key, its exact modifiers on each platform, and the action it runs.
#[derive(Clone, Copy)]
pub struct PhysicalKey {
    pub key: PhysicalKeyId,
    pub mac: ChordModifiers,
    pub linux: ChordModifiers,
    pub action: &'static dyn Action,
}

impl PhysicalKey {
    /// The modifiers this row is typed with on `platform`.
    pub fn modifiers(&self, platform: Platform) -> ChordModifiers {
        match platform {
            Platform::Mac => self.mac,
            Platform::Linux => self.linux,
        }
    }
}

/// Every shortcut SPEC.md matches by key position rather than by the key's printed character, with the modifiers the
/// old app used at the reference commit: Command on a Mac and Ctrl+Shift on Linux, and Command+Shift on a Mac for
/// terminal text size. A row's action runs from anywhere in the window, a focused terminal and a text field included,
/// and the key goes no further (the module docs' "Shortcuts matched by the physical key"). Its feature adds the
/// action's handler; until then the key does nothing at all.
///
/// A row is the whole of a new shortcut: its key's identities are data in the row. One row per line, so that a new
/// shortcut only adds a line.
#[rustfmt::skip]
pub const PHYSICAL_KEYS: &[PhysicalKey] = &[
    // SPEC.md, "Template number shortcuts": 1–9 then 0, the main row's digits. The Mac's digit keycodes are not in
    // numeric order.
    number_key(key("AE01", 0x12, "1"), &TemplateNumberKey { digit: 1 }),
    number_key(key("AE02", 0x13, "2"), &TemplateNumberKey { digit: 2 }),
    number_key(key("AE03", 0x14, "3"), &TemplateNumberKey { digit: 3 }),
    number_key(key("AE04", 0x15, "4"), &TemplateNumberKey { digit: 4 }),
    number_key(key("AE05", 0x17, "5"), &TemplateNumberKey { digit: 5 }),
    number_key(key("AE06", 0x16, "6"), &TemplateNumberKey { digit: 6 }),
    number_key(key("AE07", 0x1A, "7"), &TemplateNumberKey { digit: 7 }),
    number_key(key("AE08", 0x1C, "8"), &TemplateNumberKey { digit: 8 }),
    number_key(key("AE09", 0x19, "9"), &TemplateNumberKey { digit: 9 }),
    number_key(key("AE10", 0x1D, "0"), &TemplateNumberKey { digit: 0 }),
    // SPEC.md, "Desktop window chrome": Keyboard shortcuts, at the US slash's key.
    PhysicalKey { key: key("AB10", 0x2C, "/"), mac: COMMAND, linux: CONTROL_SHIFT, action: &ShowKeyboardShortcuts },
    // SPEC.md, "Terminal experience": text size, at the US positions of = and −.
    PhysicalKey { key: key("AE12", 0x18, "="), mac: COMMAND_SHIFT, linux: CONTROL_SHIFT, action: &IncreaseTerminalTextSize },
    PhysicalKey { key: key("AE11", 0x1B, "-"), mac: COMMAND_SHIFT, linux: CONTROL_SHIFT, action: &DecreaseTerminalTextSize },
];

/// A template number key's row: Command+digit on a Mac, Ctrl+Shift+digit on Linux.
const fn number_key(key: PhysicalKeyId, action: &'static dyn Action) -> PhysicalKey {
    PhysicalKey {
        key,
        mac: COMMAND,
        linux: CONTROL_SHIFT,
        action,
    }
}

/// The first row of [`PHYSICAL_KEYS`] whose key `event` was typed at, whatever modifiers were held: for code that matches
/// these keys bare, such as the quick-launch sequence's number keys (the row's action says which digit:
/// `row.action.as_any().downcast_ref::<TemplateNumberKey>()`). [`PhysicalKeyId::typed_in`] says how the key is read,
/// and its NOTE what a bare match should know.
pub fn physical_key_at(event: &KeyDownEvent) -> Option<&'static PhysicalKey> {
    PHYSICAL_KEYS.iter().find(|row| row.key.typed_in(event))
}

/// The row of [`PHYSICAL_KEYS`] that `event` presses on `platform`, if any: its key and exactly its modifiers. Shift is
/// read from the native event when it has one, because the logical keystroke drops Shift from a symbol (on Linux,
/// Ctrl+Shift+1 arrives as Ctrl+`!`). Crate-only, with the platform passed in like [`owner_on`], so the Mac's rows can
/// be tested on Linux; the app asks about its own platform.
pub(crate) fn physical_on(
    platform: Platform,
    event: &KeyDownEvent,
) -> Option<&'static PhysicalKey> {
    let modifiers = &event.keystroke.modifiers;
    if modifiers.alt || modifiers.function {
        return None;
    }
    let held = ChordModifiers {
        command: modifiers.platform,
        control: modifiers.control,
        shift: event
            .native_key
            .as_ref()
            .map_or(modifiers.shift, |native| native.shift),
    };
    PHYSICAL_KEYS
        .iter()
        .find(|row| row.key.typed_in(event) && row.modifiers(platform) == held)
}

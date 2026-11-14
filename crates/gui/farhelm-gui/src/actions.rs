//! The app's actions and the keys bound to them, declared in one place, and which keys a focused terminal keeps.
//!
//! SPEC_impl.md, "Actions and keymap": buttons, menu items and keys dispatch the same action, and every action is
//! declared here, so a key and the control it stands for can never drift apart. This file holds the actions the focus
//! coordinator and the layer manager need, the session shortcuts' actions, the terminal's key table
//! ([`TERMINAL_KEYS`]), and the list of actions a held key runs only once (`ONCE_PER_PRESS`). The features add their
//! own actions as they are built.
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
//! [`TERMINAL_KEYS`], with a row in [`TERMINAL_BINDINGS`] once the terminal command it names has an action, an action
//! in `ONCE_PER_PRESS`, and a chord in `TEXT_FIELD_KEYS` for an app shortcut SPEC.md says a text field keeps.
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
//! is the place to refuse it; for a terminal, that comes with the terminal's input method work.

use farhelm_input_mapping::Platform;
use gpui::{
    Action, App, Global, KeyBinding, KeyContext, KeyDownEvent, KeybindingKeystroke, Keystroke,
    KeystrokeEvent, NoAction, Subscription, Window, actions,
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

/// The chords a focused text field keeps for itself although Farhelm binds them elsewhere. [`install`] binds each to
/// `NoAction` in [`TEXT_INPUT_CONTEXT`], which hides every binding further out for the same chord, so the key goes on
/// to the field's own handling (here, nothing: Kit binds none of them). Bound on every platform: on macOS the Delete
/// chord is Cmd+W and Ctrl+Shift+Backspace is not Farhelm's, so the row changes nothing there.
const TEXT_FIELD_KEYS: &[&str] = &[
    // SPEC.md, "Desktop window chrome": "The Linux chord leaves text editors other than the terminal alone."
    "ctrl-shift-backspace",
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
        /// Paste the clipboard's text into the focused terminal, as a paste its program can tell from typing. The
        /// terminal handles it; its keys are in [`TERMINAL_BINDINGS`], and a menu item that dispatches it reaches the
        /// focused terminal the same way.
        Paste,
    ]
);

/// The terminal's own commands that have an action: each binds its chords on each platform in [`TERMINAL_CONTEXT`],
/// and every chord must be a [`KeyOwner::Terminal`] row of [`TERMINAL_KEYS`] (the keymap tests check both). The
/// terminal element handles the action, so the binding takes the key before the terminal's key listener would drop it.
///
/// [`install`] binds these after the table's `NoAction` rows, because GPUI ranks bindings of equal depth latest first
/// and a `NoAction` hides every binding ranked after it.
pub const TERMINAL_BINDINGS: &[TerminalBinding] = &[
    // One entry per line, the shape the lists areas append to keep.
    TerminalBinding {
        mac: &[PASTE_MAC],
        linux: &[PASTE_LINUX, PASTE_LINUX_INSERT],
        bind: |chord| KeyBinding::new(chord, Paste, Some(TERMINAL_CONTEXT)),
    },
];

/// Paste's chords. The Mac's is the system's; the old app's terminal got it from WebKit. Linux has two because the
/// desktop terminals there do (Ctrl+Shift+V, since plain Ctrl+V is the program's, and Shift+Insert); the old app had
/// no Linux window, so this is the GPUI migration's choice, recorded in the terminal area's goal on 2026-11-14.
const PASTE_MAC: &str = "cmd-v";
const PASTE_LINUX: &str = "ctrl-shift-v";
const PASTE_LINUX_INSERT: &str = "shift-insert";

/// One of the terminal's own commands with an action, in [`TERMINAL_BINDINGS`]: its chords on each platform, and the
/// binding of one chord to its action in [`TERMINAL_CONTEXT`] (a function rather than the action itself, because
/// GPUI builds a binding from a concrete action type; a test reads the action back off a built binding).
pub struct TerminalBinding {
    pub mac: &'static [&'static str],
    pub linux: &'static [&'static str],
    pub bind: fn(&'static str) -> KeyBinding,
}

impl TerminalBinding {
    /// The chords on `platform`.
    pub fn chords(&self, platform: Platform) -> &'static [&'static str] {
        match platform {
            Platform::Mac => self.mac,
            Platform::Linux => self.linux,
        }
    }
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
    ]);
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
    // added, latest first, and a `NoAction` suppresses everything ranked after it. So the terminal's own actions
    // (`TERMINAL_BINDINGS`) work only because they are bound after these rows; bound before them, their keys would
    // silently do nothing. The same ranking lets a binding with no context that is added after this one beat a row
    // here; the keymap walk test catches that case.
    cx.bind_keys(
        TERMINAL_KEYS
            .iter()
            .filter(|key| key.owner != KeyOwner::App)
            .filter_map(|key| key.chord(THIS_PLATFORM))
            .map(|chord| KeyBinding::new(chord, NoAction, Some(TERMINAL_CONTEXT))),
    );
    cx.bind_keys(TERMINAL_BINDINGS.iter().flat_map(|binding| {
        binding
            .chords(THIS_PLATFORM)
            .iter()
            .map(|chord| (binding.bind)(chord))
    }));
    let interceptor = cx.intercept_keystrokes(hold_back_repeat);
    cx.set_global(RepeatGuard {
        _interceptor: interceptor,
    });
}

// ---- Shortcuts that act once per press -------------------------------------------------------------------------------

/// The actions a held key runs once, on the press, and not again on each auto-repeat: the ones behind SPEC.md's
/// shortcuts that "ignore repeat". A shortcut opts in by having its action listed here; its binding stays where every
/// binding is, in [`install`]. The module docs say what the guard covers and what it cannot.
///
/// A listed action's handler must take every key that reaches it, even when it does nothing (Delete with no session
/// open). The guard judges a repeat by the first binding GPUI tries for the key; a handler that declined would send
/// the press on to the next binding or to the key listeners, and its repeats would not follow it there.
const ONCE_PER_PRESS: &[&dyn Action] = &[
    // One entry per line, so that a new shortcut only adds a line (the shape `docs/gpui/foundation-ownership.md` asks
    // of the lists areas append to); this comment is also what keeps rustfmt from joining the entries.
    &DeleteSession,
    &RenameSession,
];

/// Keeps the repeat guard's keystroke interceptor subscribed for the app's lifetime.
struct RepeatGuard {
    _interceptor: Subscription,
}

impl Global for RepeatGuard {}

/// The repeat guard: stop a held repeat of a key that would run a [`ONCE_PER_PRESS`] action, before GPUI matches any
/// binding for it.
///
/// It does not check GPUI's character-input preference, under which bindings would not run at all: on macOS and
/// Wayland only GPUI's replay of a held-back chord key asks for it, and a replay never reaches an interceptor. Were a
/// platform to ask for it, dropping a listed shortcut's repeat would still be what SPEC.md wants.
fn hold_back_repeat(event: &KeystrokeEvent, _: &mut Window, cx: &mut App) {
    let Some(down) = &event.key_down else {
        return;
    };
    if down.is_held && runs_once_per_press(&down.keystroke, &event.context_stack, cx) {
        cx.stop_propagation();
    }
}

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
    bindings.first().is_some_and(|binding| {
        ONCE_PER_PRESS
            .iter()
            .any(|listed| listed.partial_eq(binding.action()))
    })
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
    /// terminal takes it. Until the terminal binds an action for one, the key reaches the terminal's listener, which
    /// drops it. (`farhelm-input-mapping` still classifies paging and select all as well; its own copy of that rule
    /// goes when the keymap takes it over, and a test compares the two meanwhile.)
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
/// and paste in the old app's terminal, and Linux's paste chords (see [`TERMINAL_BINDINGS`]). Linux has no copy chord
/// here yet: neither SPEC.md nor the old app's terminal settles one, so it waits for the copy work.
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
    app(
        RENAME_SESSION_MAC,
        RENAME_SESSION_LINUX,
        "rename the open session",
    ),
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
    TerminalKey {
        mac: Some(PASTE_MAC),
        linux: Some(PASTE_LINUX),
        owner: KeyOwner::Terminal,
        what: "paste into the terminal",
    },
    TerminalKey {
        mac: None,
        linux: Some(PASTE_LINUX_INSERT),
        owner: KeyOwner::Terminal,
        what: "paste into the terminal (Shift+Insert, which the program no longer gets)",
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
/// key that reaches its listener: it drops a [`KeyOwner::App`] or [`KeyOwner::Terminal`] key, press and held repeats
/// alike, and hands a [`KeyOwner::Program`] key to its input mapping. Once an app shortcut is bound, the binding takes
/// the key and this is never asked; a held repeat then runs the binding again, unless its action is in
/// `ONCE_PER_PRESS`, whose guard stops the repeat before the binding or the terminal sees it.
///
/// A key a binding handled never reaches the terminal, so this answers only for the keys that do. It matches the way
/// GPUI matches a binding ([`Keystroke::should_match`]), so a key counts as a chord here exactly when a binding for
/// that chord would take it. It takes the whole key-down event, not only its keystroke, because the shortcuts this
/// table cannot hold yet (the NOTE on [`TERMINAL_KEYS`]) match the physical key, which only the event's `native_key`
/// carries; today only the keystroke is read.
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

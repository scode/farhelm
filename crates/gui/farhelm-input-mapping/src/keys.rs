//! The keydown decision, from the pinned grid-input.js / xterm.js 6.0.0 Keyboard.ts and CoreBrowserTerminal key
//! filters (MIT; notices in LICENSE-PORTED). It decides whether a native key event goes to the terminal, stays local
//! or is left to text input; the supervisor encodes the bytes.

use farhelm_proto::grid::GridInput;

/// GPUI's modifier fields, including the function bit the grid protocol omits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub platform: bool,
    pub function: bool,
}

impl Modifiers {
    /// The existing wire bits; function is not representable as a modifier.
    pub fn bits(self) -> u8 {
        u8::from(self.shift)
            | (u8::from(self.alt) << 1)
            | (u8::from(self.control) << 2)
            | (u8::from(self.platform) << 3)
    }
}

/// Optional native position supplied by a frontend, without importing its UI types.
/// Unknown/synthetic origin stays None; logical labels never become physical data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeIdentity<'a> {
    Xkb(&'a str),
    Mac(u16),
}

/// Original Shift is independent of the logical symbol normalization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeKey<'a> {
    pub identity: NativeIdentity<'a>,
    pub shift: bool,
}

/// The fields of GPUI 0.3.12 `KeyDownEvent` that affect input policy.
///
/// There is no supplied DOM `code`, keyCode, AltGraph bit, or composing flag.
/// The narrow vendored exposure supplies native position and original Shift.
/// Borrow these values from the event; do not populate them from DOM fixtures in
/// a native conformance test. `is_held` suppresses app shortcuts, not terminal repeat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyDown<'a> {
    pub native: Option<NativeKey<'a>>,
    pub modifiers: Modifiers,
    pub key: &'a str,
    pub key_char: Option<&'a str>,
    pub is_held: bool,
    pub prefer_character_input: bool,
}

/// Viewer platform, independent of the machine running a pure conformance test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Linux,
}

/// Composition state maintained by the focused InputHandler owner.
///
/// Set `marked` from replace-and-mark/unmark callbacks. The owner must retain it
/// through an Enter keydown which commits preedit: key metadata alone cannot tell
/// that Enter from a terminal Enter. Clear `dead_pending` after a committed insert.
/// A composing key is left propagating so GPUI can perform its text replacement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Composition {
    pub marked: bool,
    pub dead_pending: bool,
}

/// Routing context supplied before the terminal is allowed to consume a key.
///
/// App shortcut resolution and enabled-state checks belong to the live control.
/// `app_shortcut` means that check succeeded; the mapper only prevents repeat and
/// composition from activating it. Modal and text-field keys never reach a terminal.
#[derive(Clone, Copy, Debug, Default)]
pub struct Route {
    pub terminal_focused: bool,
    pub modal_open: bool,
    pub app_shortcut: bool,
}

/// One decision for the owner; only `Send` is a wire message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Send(GridInput),
    /// Let GPUI reach the InputHandler; its committed callback sends text once.
    Text {
        cancel: bool,
    },
    Composition,
    AppShortcut,
    /// A recognized app shortcut repeated; consume it without reactivating it
    /// or leaking its terminal meaning through the repeat guard.
    Suppress,
    PageUp,
    PageDown,
    SelectAll,
    /// The focused modal/field or another application control owns this event.
    Unhandled,
    /// No safe inference exists (including an Fn-modified key).
    Unsupported,
}

/// Decide app-versus-terminal ownership, then map the bounded native key subset.
///
/// Without native metadata `code` stays empty and symbol Shift uses the tested
/// US inference. Native metadata restores original Shift and printable keypad
/// routes only; it does not turn arbitrary layout labels into DOM physical codes.
pub fn map_key(
    mut event: KeyDown<'_>,
    platform: Platform,
    composition: &mut Composition,
    route: Route,
) -> Decision {
    if let Some(native) = event.native {
        event.modifiers.shift = native.shift;
    }
    if composition.marked {
        return Decision::Composition;
    }
    let dead = event.key.starts_with("dead_") || event.key == "Dead" || event.key == "AltGraph";
    if route.app_shortcut
        && !route.modal_open
        && !event.prefer_character_input
        && !composition.dead_pending
        && !dead
    {
        return if event.is_held {
            Decision::Suppress
        } else {
            Decision::AppShortcut
        };
    }
    if !route.terminal_focused || route.modal_open {
        return Decision::Unhandled;
    }
    if event.modifiers.function {
        return Decision::Unsupported;
    }
    if dead {
        composition.dead_pending = true;
        return Decision::Text { cancel: false };
    }
    let Some(mut key) = infer(event) else {
        return Decision::Text { cancel: false };
    };
    if let Some(native) = event.native {
        // The actual event's Shift supersedes the old US-symbol assumption.
        key.mods = 0;
        if let Some((kc, code)) = keypad(native.identity, event.key) {
            key.kc = kc;
            key.code = code;
        }
    }
    // Option is character input on macOS. Linux Level3 can arrive without an
    // Alt bit, so a differing committed character must survive the key label.
    if event.prefer_character_input
        || (platform == Platform::Mac
            && event.modifiers.alt
            && !event.modifiers.control
            && !event.modifiers.platform
            && key.kc > 47)
        || (event
            .key_char
            .is_some_and(|text| text != key.key && !text.is_ascii())
            && !event.modifiers.control
            && !event.modifiers.platform)
        || (event
            .key_char
            .is_some_and(|text| text != key.key && text != event.key.to_ascii_uppercase())
            && event.modifiers.control
            && event.modifiers.alt)
        || (key.kc >= 48
            && event.key_char.is_some_and(|text| text != key.key)
            && !event.modifiers.control
            && !event.modifiers.alt
            && !event.modifiers.platform
            && !event.modifiers.shift)
    {
        return Decision::Text { cancel: false };
    }
    key.mods |= event.modifiers.bits();
    let outcome = client_action(
        &key,
        platform == Platform::Mac,
        false,
        false,
        composition.dead_pending,
    );
    composition.dead_pending = outcome.pending;
    match outcome.action {
        Action::Send => Decision::Send(GridInput::Key {
            key: key.key,
            code: key.code,
            kc: key.kc,
            mods: key.mods,
            mac: platform == Platform::Mac,
        }),
        Action::Pass => Decision::Text {
            cancel: outcome.cancel,
        },
        Action::Ignore => Decision::Composition,
        Action::PageUp => Decision::PageUp,
        Action::PageDown => Decision::PageDown,
        Action::SelectAll => Decision::SelectAll,
    }
}

/// Only printable keypad labels use numeric keypad keyCodes. NumLock-off End,
/// Home and arrows keep their named-key route despite sharing a keypad position.
fn keypad(identity: NativeIdentity<'_>, logical: &str) -> Option<(u32, String)> {
    let digit = match identity {
        NativeIdentity::Xkb(name) => name
            .strip_prefix("KP")
            .filter(|s| s.len() == 1)
            .and_then(|s| s.parse::<u32>().ok()),
        NativeIdentity::Mac(code) => match code {
            0x52 => Some(0),
            0x53..=0x59 => Some(u32::from(code - 0x52)),
            0x5b => Some(8),
            0x5c => Some(9),
            _ => None,
        },
    };
    if let Some(digit) = digit.filter(|digit| *digit < 10 && logical == digit.to_string()) {
        return Some((96 + digit, format!("Numpad{digit}")));
    }
    let (symbol, kc, code) = match identity {
        NativeIdentity::Xkb("KPMU") | NativeIdentity::Mac(0x43) => ("*", 106, "NumpadMultiply"),
        NativeIdentity::Xkb("KPAD") | NativeIdentity::Mac(0x45) => ("+", 107, "NumpadAdd"),
        NativeIdentity::Xkb("KPSU") | NativeIdentity::Mac(0x4e) => ("-", 109, "NumpadSubtract"),
        NativeIdentity::Xkb("KPDL") | NativeIdentity::Mac(0x41) => (".", 110, "NumpadDecimal"),
        NativeIdentity::Xkb("KPDV") | NativeIdentity::Mac(0x4b) => ("/", 111, "NumpadDivide"),
        _ => return None,
    };
    (logical == symbol).then(|| (kc, code.into()))
}

/// DOM-shaped policy input, private so a frontend cannot bypass native inference.
/// The full DOM table tests below validate this policy separately from the bounded
/// native projection. Keeping that distinction prevents overstating conformance.
pub(crate) struct ClientKey {
    pub key: String,
    pub code: String,
    pub kc: u32,
    pub mods: u8,
}

/// Recover only fields with a named-key or US ASCII interpretation.
fn infer(event: KeyDown<'_>) -> Option<ClientKey> {
    let named = match event.key {
        "enter" => Some(("Enter", 13)),
        "backspace" => Some(("Backspace", 8)),
        "tab" => Some(("Tab", 9)),
        "escape" => Some(("Escape", 27)),
        "left" => Some(("ArrowLeft", 37)),
        "up" => Some(("ArrowUp", 38)),
        "right" => Some(("ArrowRight", 39)),
        "down" => Some(("ArrowDown", 40)),
        "home" => Some(("Home", 36)),
        "end" => Some(("End", 35)),
        "insert" => Some(("Insert", 45)),
        "delete" => Some(("Delete", 46)),
        "pageup" => Some(("PageUp", 33)),
        "pagedown" => Some(("PageDown", 34)),
        "space" => Some((" ", 32)),
        _ => None,
    };
    if let Some((key, kc)) = named {
        return Some(ClientKey {
            key: key.into(),
            code: String::new(),
            kc,
            mods: 0,
        });
    }
    if let Some(number) = event
        .key
        .strip_prefix('f')
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| (1..=12).contains(n))
    {
        return Some(ClientKey {
            key: format!("F{number}"),
            code: String::new(),
            kc: number + 111,
            mods: 0,
        });
    }
    let mut chars = event.key.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || !ch.is_ascii() {
        return None;
    }
    let (kc, shifted) = ascii_key(ch)?;
    let key = if ch.is_ascii_alphabetic() && event.modifiers.shift {
        ch.to_ascii_uppercase()
    } else {
        ch
    };
    Some(ClientKey {
        key: key.to_string(),
        code: String::new(),
        kc,
        mods: u8::from(shifted),
    })
}

/// US ASCII-to-keyCode inference; this is not a physical keyboard mapping.
pub(crate) fn ascii_key(ch: char) -> Option<(u32, bool)> {
    if ch.is_ascii_alphabetic() {
        return Some((ch.to_ascii_uppercase() as u32, false));
    }
    if ch.is_ascii_digit() {
        return Some((ch as u32, false));
    }
    if ch == ' ' {
        return Some((32, false));
    }
    for (plain, shifted, kc) in [
        ('0', ')', 48),
        ('1', '!', 49),
        ('2', '@', 50),
        ('3', '#', 51),
        ('4', '$', 52),
        ('5', '%', 53),
        ('6', '^', 54),
        ('7', '&', 55),
        ('8', '*', 56),
        ('9', '(', 57),
        (';', ':', 186),
        ('=', '+', 187),
        (',', '<', 188),
        ('-', '_', 189),
        ('.', '>', 190),
        ('/', '?', 191),
        ('`', '~', 192),
        ('[', '{', 219),
        ('\\', '|', 220),
        (']', '}', 221),
        ('\'', '"', 222),
    ] {
        if ch == plain {
            return Some((kc, false));
        }
        if ch == shifted {
            return Some((kc, true));
        }
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Send,
    Pass,
    Ignore,
    PageUp,
    PageDown,
    SelectAll,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Outcome {
    pub action: Action,
    pub cancel: bool,
    pub pending: bool,
}

/// Port of pinned grid-input.js keydownAction and its sequence-existence checks.
/// This computes ownership only; it does not encode even one terminal byte.
/// The broad fixture Cartesian space is useful here even when no native event
/// could have the fixture's simultaneous keyCode, key, and physical code.
pub(crate) fn client_action(
    key: &ClientKey,
    mac: bool,
    windows: bool,
    alt_graph: bool,
    before: bool,
) -> Outcome {
    let ClientKey {
        key,
        code,
        kc,
        mods,
    } = key;
    if *kc == 229 {
        return Outcome {
            action: Action::Ignore,
            cancel: false,
            pending: before,
        };
    }
    let shift = mods & 1 != 0;
    let alt = mods & 2 != 0;
    let ctrl = mods & 4 != 0;
    let meta = mods & 8 != 0;
    let mut pending = before || key == "Dead" || key == "AltGraph";
    let mut action = Action::Pass;
    let mut cancel = false;
    let sequence = match *kc {
        0 => matches!(
            key.as_str(),
            "UIKeyInputUpArrow"
                | "UIKeyInputDownArrow"
                | "UIKeyInputLeftArrow"
                | "UIKeyInputRightArrow"
        ),
        8 => true,
        9 => {
            cancel = !shift;
            true
        }
        13 | 27 => {
            cancel = true;
            true
        }
        37..=40 => !meta,
        45 => !shift && !ctrl,
        46 | 35 | 36 | 112..=123 => true,
        33 | 34 => {
            if shift {
                action = if *kc == 33 {
                    Action::PageUp
                } else {
                    Action::PageDown
                };
            }
            !shift
        }
        _ if ctrl && !shift && !alt && !meta => {
            (65..=90).contains(kc) || matches!(*kc, 32 | 51..=56 | 219..=221)
        }
        _ if !mac && alt && !meta => {
            let dead = key == "Dead" && code.starts_with("Key");
            cancel = dead
                && !((65..=90).contains(kc) || matches!(*kc, 32 | 48..=57 | 186..=192 | 219..=222));
            (65..=90).contains(kc) || matches!(*kc, 32 | 48..=57 | 186..=192 | 219..=222) || dead
        }
        _ if mac && !alt && !ctrl && !shift && meta => {
            if *kc == 65 {
                action = Action::SelectAll;
            }
            false
        }
        _ if !key.is_empty()
            && !ctrl
            && !alt
            && !meta
            && *kc >= 48
            && key.encode_utf16().count() == 1 =>
        {
            true
        }
        _ => ctrl && matches!(key.as_str(), "_" | "@"),
    };
    if matches!(action, Action::PageUp | Action::PageDown) {
        return Outcome {
            action,
            cancel: true,
            pending,
        };
    }
    let third =
        (mac && alt && !ctrl && !meta) || (windows && ((alt && ctrl && !meta) || alt_graph));
    if third && (*kc == 0 || *kc > 47) {
        return Outcome {
            action,
            cancel: false,
            pending,
        };
    }
    if sequence {
        let capital =
            key.len() == 1 && key.as_bytes()[0].is_ascii_uppercase() && !ctrl && !alt && !meta;
        if !capital {
            if pending {
                pending = false;
            } else {
                action = Action::Send;
                cancel = true;
            }
        }
    }
    Outcome {
        action,
        cancel,
        pending,
    }
}

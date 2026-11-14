//! Three distinct checks: DOM policy fidelity, bounded native projection, and
//! pinned byte-oracle fidelity. Only the native projection tests the mapper.

use super::*;
use crate::keys::{Action, ClientKey, Outcome, ascii_key, client_action};
use crate::pointer::*;
use farhelm_proto::grid::{GridInput, MouseEventKind};
use farhelm_supervisor::grid_input::{InputEncoder, KeyPlatform, encode_key};
use farhelm_terminal_emulator::{Modes, MouseEncoding, MouseReporting, Size};
use serde_json::Value;
use std::collections::BTreeMap;

/// Comments are part of the copied fixture, not JSON records.
fn rows(text: &str) -> Vec<Value> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// Explicit modes avoid dependence on an emulator's process or environment.
fn modes(cursor: bool) -> Modes {
    Modes {
        bracketed_paste: false,
        mouse_reporting: MouseReporting::Off,
        mouse_encoding: MouseEncoding::Default,
        focus_reporting: false,
        application_cursor: cursor,
        application_keypad: false,
        alternate_screen: false,
        cursor_visible: true,
    }
}

/// Paste chunks must keep CRLF together and stay within both wire budgets.
/// Real supervisor encoding proves that one grouped insertion produces one
/// bracket pair and the same normalized text, including hostile end markers.
#[test]
fn pasted_text_preserves_group_boundaries_and_real_supervisor_bytes() {
    let text = format!("{}\r\n😀\n\x1b[201~end", "a".repeat(32767));
    let parts = pasted_text(&text);
    assert!(parts.len() > 1);
    let mut joined = String::new();
    let mut output = Vec::new();
    let mut encoder = InputEncoder::new();
    let mut mode = modes(false);
    mode.bracketed_paste = true;
    for (index, part) in parts.iter().enumerate() {
        let GridInput::Paste { data, last } = part else {
            panic!("paste became another input kind")
        };
        assert_eq!(*last, index + 1 == parts.len());
        assert!(data.len() <= farhelm_proto::grid::GRID_MAX_PASTE_PART_BYTES);
        assert!(!data.ends_with('\r'));
        let wire = part.encode().unwrap();
        assert_eq!(GridInput::decode(&wire).unwrap(), *part);
        joined.push_str(data);
        output.extend(
            encoder
                .encode(
                    part,
                    &mode,
                    Size {
                        columns: 80,
                        lines: 24,
                    },
                )
                .unwrap(),
        );
    }
    assert_eq!(joined, text);
    let expected = format!("\x1b[200~{}\r😀\rend\x1b[201~", "a".repeat(32767));
    assert_eq!(output, expected.as_bytes());
    assert!(pasted_text("").is_empty());

    let escaped = format!("{}\r\n", "\0".repeat(40000));
    let escaped_parts = pasted_text(&escaped);
    assert!(escaped_parts.len() >= 4);
    for part in escaped_parts {
        assert!(part.encode().unwrap().len() <= farhelm_proto::grid::GRID_MAX_INPUT_MESSAGE);
    }
}

/// Hex rendering is only for comparing the fixture's presentation of bytes.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// All four fixture variants must agree with Farhelm's *actual* encoder.
/// This is an oracle sanity check, not evidence that native fields contain DOM
/// physical identity. Native conformance is measured separately below.
#[test]
fn pinned_key_bytes_match_actual_supervisor_encoder() {
    let rows = rows(include_str!(
        "../../../farhelm-supervisor/tests/fixtures/grid-keys.jsonl"
    ));
    assert_eq!(rows.len(), 1578);
    for row in rows {
        for mods in 0..16 {
            for mac in [false, true] {
                for cursor in [false, true] {
                    let expected = &row[3][mods];
                    let expected = if expected.is_array() {
                        &expected[usize::from(mac) * 2 + usize::from(cursor)]
                    } else {
                        expected
                    };
                    let bytes = encode_key(
                        row[1].as_str().unwrap(),
                        row[2].as_str().unwrap(),
                        row[0].as_u64().unwrap() as u32,
                        mods as u8,
                        cursor,
                        if mac {
                            KeyPlatform::Mac
                        } else {
                            KeyPlatform::Other
                        },
                    );
                    assert_eq!(
                        hex(&bytes),
                        expected.as_str().unwrap(),
                        "row={row}, mods={mods}, mac={mac}, cursor={cursor}"
                    );
                }
            }
        }
    }
}

/// Decode the pinned letter format without deriving expectations from policy.
fn outcome(letter: char) -> Outcome {
    let (action, cancel, pending) = match letter {
        'a' => (Action::Pass, false, false),
        'b' => (Action::Pass, false, true),
        'c' => (Action::Send, true, false),
        'd' => (Action::Pass, true, false),
        'e' => (Action::PageUp, true, false),
        'f' => (Action::PageUp, true, true),
        'g' => (Action::PageDown, true, false),
        'h' => (Action::PageDown, true, true),
        'i' => (Action::Ignore, false, false),
        'j' => (Action::Ignore, false, true),
        'k' => (Action::SelectAll, false, false),
        'l' => (Action::SelectAll, false, true),
        other => panic!("unknown outcome {other}"),
    };
    Outcome {
        action,
        cancel,
        pending,
    }
}

/// Every DOM policy case (including Windows/AltGraph, which GPUI cannot expose)
/// pins the ownership port. This does not project DOM fields into native input.
#[test]
fn complete_dom_client_policy_matches_pinned_table() {
    let rows = rows(include_str!("../fixtures/keydown-actions.jsonl"));
    let patterns: BTreeMap<_, _> = rows
        .iter()
        .filter(|row| row[0] == "p")
        .map(|row| (row[1].as_str().unwrap(), row[2].as_str().unwrap()))
        .collect();
    let mut checked = 0;
    for row in rows.iter().filter(|row| row[0] != "p") {
        for (platform, (mac, windows)) in [(false, false), (true, false), (false, true)]
            .into_iter()
            .enumerate()
        {
            let pattern = patterns[row[platform + 3].as_str().unwrap()];
            for mods in 0..16 {
                for alt_graph in [false, true] {
                    for before in [false, true] {
                        let index = mods * 4 + usize::from(alt_graph) * 2 + usize::from(before);
                        let key = ClientKey {
                            key: row[1].as_str().unwrap().into(),
                            code: row[2].as_str().unwrap().into(),
                            kc: row[0].as_u64().unwrap() as u32,
                            mods: mods as u8,
                        };
                        assert_eq!(
                            client_action(&key, mac, windows, alt_graph, before),
                            outcome(pattern.as_bytes()[index] as char),
                            "row={row}, platform={platform}, mods={mods}, altGraph={alt_graph}, before={before}"
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    eprintln!(
        "DOM policy: {} rows, {checked} modifier/platform/state cases",
        checked / (3 * 64)
    );
}

/// Independent US-layout projection of fixture shapes into actual GPUI fields.
/// Selection checks key/code/kc consistency, then discards DOM metadata. Shifted
/// symbols use GPUI Linux's observed source normalization (Shift is cleared).
/// Cases outside this projection are counted, not passed through as native data.
fn native_projection(row: &Value, mods: u8) -> Option<(String, Option<String>, Modifiers)> {
    let kc = row[0].as_u64()? as u32;
    let key = row[1].as_str()?;
    let code = row[2].as_str()?;
    let named = [
        (8, "Backspace", "backspace"),
        (9, "Tab", "tab"),
        (13, "Enter", "enter"),
        (27, "Escape", "escape"),
        (32, " ", "space"),
        (33, "PageUp", "pageup"),
        (34, "PageDown", "pagedown"),
        (35, "End", "end"),
        (36, "Home", "home"),
        (37, "ArrowLeft", "left"),
        (38, "ArrowUp", "up"),
        (39, "ArrowRight", "right"),
        (40, "ArrowDown", "down"),
        (45, "Insert", "insert"),
        (46, "Delete", "delete"),
    ];
    let mut native_key = None;
    for (number, dom, native) in named {
        if kc == number
            && key == dom
            && (code == dom || (kc == 32 && code == "Space") || (kc == 13 && code == "NumpadEnter"))
        {
            native_key = Some(native.to_string());
        }
    }
    if (112..=123).contains(&kc) && key == format!("F{}", kc - 111) && code == key {
        native_key = Some(key.to_ascii_lowercase());
    }
    let mut symbol = false;
    let mut character = None;
    if key.len() == 1 && key.is_ascii() && kc != 32 {
        let ch = key.chars().next().unwrap();
        if let Some((number, shifted)) = ascii_key(ch) {
            let canonical_code = if ch.is_ascii_alphabetic() {
                format!("Key{}", ch.to_ascii_uppercase())
            } else if kc <= 57 {
                format!("Digit{}", char::from_u32(kc).unwrap())
            } else {
                match kc {
                    186 => "Semicolon",
                    187 => "Equal",
                    188 => "Comma",
                    189 => "Minus",
                    190 => "Period",
                    191 => "Slash",
                    192 => "Backquote",
                    219 => "BracketLeft",
                    220 => "Backslash",
                    221 => "BracketRight",
                    222 => "Quote",
                    _ => "",
                }
                .to_string()
            };
            let actual_shift = mods & 1 != 0;
            let consistent = if ch.is_ascii_alphabetic() {
                ch.is_ascii_uppercase() == actual_shift
            } else {
                shifted == actual_shift
            };
            if number == kc && code == canonical_code && consistent {
                native_key = Some(key.to_ascii_lowercase());
                symbol = !ch.is_ascii_alphabetic();
                if mods & 12 == 0 {
                    character = Some(key.to_string());
                }
            }
        }
    }
    Some((
        native_key?,
        character,
        Modifiers {
            shift: mods & 1 != 0 && !symbol,
            alt: mods & 2 != 0,
            control: mods & 4 != 0,
            platform: mods & 8 != 0,
            function: false,
        },
    ))
}

/// Replay the expressible part of the client-action fixture directly from
/// source-modeled native fields. AltGraph=true is deliberately excluded because
/// GPUI has no such field; dead-pending is owner state and is tested both ways.
#[test]
fn native_subset_replays_client_action_fixture() {
    let rows = rows(include_str!("../fixtures/keydown-actions.jsonl"));
    let patterns: BTreeMap<_, _> = rows
        .iter()
        .filter(|row| row[0] == "p")
        .map(|row| (row[1].as_str().unwrap(), row[2].as_str().unwrap()))
        .collect();
    let mut checked = 0;
    for row in rows.iter().filter(|row| row[0] != "p") {
        for mods in 0..16 {
            let Some((native, text, modifiers)) = native_projection(row, mods) else {
                continue;
            };
            for mac in [false, true] {
                let pattern = patterns[row[if mac { 4 } else { 3 }].as_str().unwrap()];
                for before in [false, true] {
                    let expected = outcome(
                        pattern.as_bytes()[usize::from(mods) * 4 + usize::from(before)] as char,
                    );
                    let mut state = Composition {
                        marked: false,
                        dead_pending: before,
                    };
                    let actual = map_key(
                        KeyDown {
                            native: None,
                            key: &native,
                            key_char: text.as_deref(),
                            modifiers,
                            is_held: false,
                            prefer_character_input: false,
                        },
                        if mac { Platform::Mac } else { Platform::Linux },
                        &mut state,
                        terminal(),
                    );
                    let action = match actual {
                        Decision::Send(_) => Action::Send,
                        Decision::Text { cancel } => {
                            assert_eq!(cancel, expected.cancel);
                            Action::Pass
                        }
                        Decision::SelectAll => Action::SelectAll,
                        Decision::PageUp => Action::PageUp,
                        Decision::PageDown => Action::PageDown,
                        other => panic!("unexpected {other:?}"),
                    };
                    assert_eq!(
                        action, expected.action,
                        "row={row}, mods={mods}, mac={mac}, before={before}"
                    );
                    assert_eq!(state.dead_pending, expected.pending);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 896);
    eprintln!("Native client-action fixture: {checked} platform/modifier/dead-state decisions");
}

/// The subset classifier is checked against both independent fixture tables.
/// Empty `code` in produced messages is intentional: the encoder does not need
/// physical identity for these admitted keys, and inventing it would hide loss.
#[test]
fn source_modeled_native_us_subset_matches_decisions_and_real_bytes() {
    let key_rows = rows(include_str!(
        "../../../farhelm-supervisor/tests/fixtures/grid-keys.jsonl"
    ));
    let mut classification = BTreeMap::<&str, usize>::new();
    for row in &key_rows {
        let key = row[1].as_str().unwrap();
        let code = row[2].as_str().unwrap();
        let label = if (0..16).any(|mods| native_projection(row, mods).is_some()) {
            "bounded US projection (some modifiers)"
        } else if row[0] == 229 {
            "IME code 229: owner state required"
        } else if code.starts_with("Numpad") {
            "keypad location lost (Enter alias is in projection)"
        } else if key.starts_with("UIKeyInput") {
            "iOS-only key names, outside target platforms"
        } else if key == "Dead" {
            "dead-key tuples: no physical code in GPUI"
        } else if !key.is_ascii() {
            "international/non-BMP text or physical-position cases"
        } else if key.is_empty() || key == "Unidentified" {
            "empty/unidentified: no stable identity"
        } else {
            "outside US projection: Cartesian or other key/code/kc tuples"
        };
        *classification.entry(label).or_default() += 1;
    }
    eprintln!("Fixture row classification: {classification:#?}");
    let mut native = 0;
    let mut excluded = 0;
    for row in key_rows {
        for mods in 0..16 {
            let Some((native_key, text, modifiers)) = native_projection(&row, mods) else {
                excluded += 1;
                continue;
            };
            native += 1;
            for mac in [false, true] {
                let event = KeyDown {
                    native: None,
                    key: &native_key,
                    key_char: text.as_deref(),
                    modifiers,
                    is_held: false,
                    prefer_character_input: false,
                };
                let expected = client_action(
                    &ClientKey {
                        key: row[1].as_str().unwrap().into(),
                        code: row[2].as_str().unwrap().into(),
                        kc: row[0].as_u64().unwrap() as u32,
                        mods,
                    },
                    mac,
                    false,
                    false,
                    false,
                );
                let actual = map_key(
                    event,
                    if mac { Platform::Mac } else { Platform::Linux },
                    &mut Composition::default(),
                    terminal(),
                );
                let action = match &actual {
                    Decision::Send(_) => Action::Send,
                    Decision::Text { .. } => Action::Pass,
                    Decision::PageUp => Action::PageUp,
                    Decision::PageDown => Action::PageDown,
                    Decision::SelectAll => Action::SelectAll,
                    other => panic!("unexpected {other:?}, row={row}, mods={mods}"),
                };
                assert_eq!(action, expected.action, "row={row}, mods={mods}, mac={mac}");
                if let Decision::Text { cancel } = actual {
                    assert_eq!(cancel, expected.cancel, "row={row}, mods={mods}");
                }
                if let Decision::Send(ref message) = actual {
                    message.validate().unwrap();
                    assert!(matches!(message, GridInput::Key { code, .. } if code.is_empty()));
                    for cursor in [false, true] {
                        let expected = &row[3][usize::from(mods)];
                        let expected = if expected.is_array() {
                            &expected[usize::from(mac) * 2 + usize::from(cursor)]
                        } else {
                            expected
                        };
                        let bytes = InputEncoder::new()
                            .encode(
                                message,
                                &modes(cursor),
                                Size {
                                    columns: 80,
                                    lines: 24,
                                },
                            )
                            .unwrap();
                        assert_eq!(
                            hex(&bytes),
                            expected.as_str().unwrap(),
                            "row={row}, mods={mods}, mac={mac}, cursor={cursor}"
                        );
                    }
                }
            }
        }
    }
    eprintln!(
        "Native US projection: {native} admitted row/modifier pairs, {excluded} outside projection; {} platform decisions",
        native * 2
    );
    assert!(native > 500);
    assert_eq!(native + excluded, 1578 * 16);
}

/// Tests default to a terminal owner; production must supply actual focus.
fn terminal() -> Route {
    Route {
        terminal_focused: true,
        ..Route::default()
    }
}

/// Compact native events keep semantic tests independent of DOM data.
fn event(key: &str) -> KeyDown<'_> {
    KeyDown {
        native: None,
        key,
        key_char: None,
        modifiers: Modifiers::default(),
        is_held: false,
        prefer_character_input: false,
    }
}

/// Actual optional native position removes the demonstrated keypad ambiguity;
/// the old no-origin projection remains a separate, bounded conformance claim.
#[test]
fn native_metadata_preserves_keypad_routes_and_original_shift() {
    for (identity, expected_send) in [
        (NativeIdentity::Xkb("AE01"), true),
        (NativeIdentity::Xkb("KP1"), false),
        (NativeIdentity::Mac(0x12), true),
        (NativeIdentity::Mac(0x53), false),
    ] {
        let key = KeyDown {
            native: Some(NativeKey {
                identity,
                shift: false,
            }),
            key_char: Some("1"),
            modifiers: Modifiers {
                alt: true,
                ..Modifiers::default()
            },
            ..event("1")
        };
        // Linux Meta bytes are the proved collision contract; Mac position is
        // translated here under the same policy to isolate identity from Option.
        let decision = map_key(
            key,
            Platform::Linux,
            &mut Composition::default(),
            terminal(),
        );
        assert_eq!(matches!(decision, Decision::Send(_)), expected_send);
        if let Decision::Send(ref message) = decision {
            assert_eq!(
                hex(&InputEncoder::new()
                    .encode(
                        message,
                        &modes(false),
                        Size {
                            columns: 80,
                            lines: 24
                        }
                    )
                    .unwrap()),
                "1b31"
            );
        }
    }
    let end = KeyDown {
        native: Some(NativeKey {
            identity: NativeIdentity::Xkb("KP1"),
            shift: false,
        }),
        ..event("end")
    };
    assert!(matches!(
        map_key(
            end,
            Platform::Linux,
            &mut Composition::default(),
            terminal()
        ),
        Decision::Send(GridInput::Key { kc: 35, .. })
    ));
    let underscore = KeyDown {
        native: Some(NativeKey {
            identity: NativeIdentity::Xkb("UNKN"),
            shift: false,
        }),
        modifiers: Modifiers {
            control: true,
            ..Modifiers::default()
        },
        ..event("_")
    };
    assert_eq!(
        map_key(
            underscore,
            Platform::Linux,
            &mut Composition::default(),
            terminal()
        ),
        Decision::Text { cancel: false }
    );
}

/// Enter mid-preedit must not reach either submit or the terminal. A committed
/// insertion is sent once; repeated identical real commits remain intentional.
#[test]
fn composition_and_app_routing_have_one_owner() {
    let mut state = Composition {
        marked: true,
        dead_pending: false,
    };
    let shortcut = Route {
        app_shortcut: true,
        ..terminal()
    };
    assert_eq!(
        map_key(event("enter"), Platform::Linux, &mut state, shortcut),
        Decision::Composition
    );
    state.marked = false;
    assert_eq!(
        map_key(event("k"), Platform::Linux, &mut state, shortcut),
        Decision::AppShortcut
    );
    assert!(matches!(
        map_key(
            KeyDown {
                is_held: true,
                ..event("k")
            },
            Platform::Linux,
            &mut state,
            shortcut
        ),
        Decision::Suppress
    ));
    assert_eq!(
        map_key(
            event("enter"),
            Platform::Linux,
            &mut state,
            Route {
                modal_open: true,
                ..shortcut
            }
        ),
        Decision::Unhandled
    );
    assert_eq!(
        map_key(
            event("enter"),
            Platform::Linux,
            &mut state,
            Route::default()
        ),
        Decision::Unhandled
    );
    assert_eq!(
        committed_text("相"),
        vec![GridInput::Text { data: "相".into() }]
    );
    assert_eq!(
        committed_text("aa"),
        vec![GridInput::Text { data: "aa".into() }]
    );
    assert_eq!(
        map_key(event("dead_acute"), Platform::Linux, &mut state, shortcut),
        Decision::Text { cancel: false }
    );
    assert!(
        state.dead_pending,
        "a dead-key start must suppress app shortcuts before the next key"
    );
}

/// These meaningful paths use GPUI's text result rather than reconstructing the
/// resulting character from the shortcut label. AltGr has no dedicated GPUI bit.
#[test]
fn option_altgr_dead_keys_and_preferred_character_use_commit_path() {
    let mut state = Composition::default();
    let option = KeyDown {
        key_char: Some("ß"),
        modifiers: Modifiers {
            alt: true,
            ..Modifiers::default()
        },
        ..event("s")
    };
    assert_eq!(
        map_key(option, Platform::Mac, &mut state, terminal()),
        Decision::Text { cancel: false }
    );
    let altgr = KeyDown {
        key_char: Some("@"),
        ..event("q")
    };
    assert_eq!(
        map_key(altgr, Platform::Linux, &mut state, terminal()),
        Decision::Text { cancel: false }
    );
    assert_eq!(
        map_key(event("dead_acute"), Platform::Linux, &mut state, terminal()),
        Decision::Text { cancel: false }
    );
    assert!(state.dead_pending);
    let composed = KeyDown {
        key_char: Some("é"),
        ..event("eacute")
    };
    assert_eq!(
        map_key(composed, Platform::Linux, &mut state, terminal()),
        Decision::Text { cancel: false }
    );
    state.dead_pending = false; // InputHandler's committed replacement clears this.
    assert_eq!(
        committed_text("é"),
        vec![GridInput::Text { data: "é".into() }]
    );
    assert_eq!(
        map_key(
            KeyDown {
                prefer_character_input: true,
                key_char: Some("a"),
                ..event("a")
            },
            Platform::Linux,
            &mut state,
            terminal()
        ),
        Decision::Text { cancel: false }
    );
}

/// Control and Shift+Enter must use actual supervisor bytes, independent of the
/// fixture projection. Shift+Enter's two bytes stay in one encoded message.
#[test]
fn meaningful_native_chords_have_exact_supervisor_bytes() {
    for (key, modifiers, expected) in [
        (
            "enter",
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
            "1b0d",
        ),
        (
            "c",
            Modifiers {
                control: true,
                ..Modifiers::default()
            },
            "03",
        ),
        (
            "[",
            Modifiers {
                control: true,
                ..Modifiers::default()
            },
            "1b",
        ),
        (
            "x",
            Modifiers {
                alt: true,
                ..Modifiers::default()
            },
            "1b78",
        ),
    ] {
        let Decision::Send(message) = map_key(
            KeyDown {
                modifiers,
                ..event(key)
            },
            Platform::Linux,
            &mut Composition::default(),
            terminal(),
        ) else {
            panic!("key {key} not sent");
        };
        let bytes = InputEncoder::new()
            .encode(
                &message,
                &modes(false),
                Size {
                    columns: 80,
                    lines: 24,
                },
            )
            .unwrap();
        assert_eq!(hex(&bytes), expected);
    }
}

/// AppKit gives Enter a newline key_char even though it is a named terminal key.
/// The text-difference filter must apply only to printable keys, otherwise even
/// an ordinary native Mac Enter would accidentally become text input.
#[test]
fn appkit_named_key_character_does_not_bypass_key_mapping() {
    for shift in [false, true] {
        let e = KeyDown {
            key_char: Some("\n"),
            modifiers: Modifiers {
                shift,
                ..Modifiers::default()
            },
            ..event("enter")
        };
        let Decision::Send(message) =
            map_key(e, Platform::Mac, &mut Composition::default(), terminal())
        else {
            panic!("AppKit Enter not sent");
        };
        assert_eq!(
            hex(&InputEncoder::new()
                .encode(
                    &message,
                    &modes(false),
                    Size {
                        columns: 80,
                        lines: 24
                    }
                )
                .unwrap()),
            if shift { "1b0d" } else { "0d" }
        );
    }
}

/// Real XKB evdev/pc105/us Alt+1 and Alt+NumLock-Keypad1 yield distinct raw
/// keysyms but identical full exposed GPUI KeyDown fields after normalization.
/// The probe script independently checks raw XKB inputs. This test records the
/// lost distinction; it must not be counted as successful native conformance.
#[test]
fn standard_keypad_location_collision_cannot_preserve_both_legacy_routes() {
    let top = KeyDown {
        key_char: Some("1"),
        modifiers: Modifiers {
            alt: true,
            ..Modifiers::default()
        },
        ..event("1")
    };
    let keypad = top;
    assert_eq!(top, keypad);
    let a = map_key(
        top,
        Platform::Linux,
        &mut Composition::default(),
        terminal(),
    );
    let b = map_key(
        keypad,
        Platform::Linux,
        &mut Composition::default(),
        terminal(),
    );
    assert_eq!(a, b);
    assert_eq!(
        hex(&encode_key("1", "Digit1", 49, 2, false, KeyPlatform::Other)),
        "1b31"
    );
    assert_eq!(
        hex(&encode_key(
            "1",
            "Numpad1",
            97,
            2,
            false,
            KeyPlatform::Other
        )),
        ""
    );
    assert_eq!(
        client_action(
            &ClientKey {
                key: "1".into(),
                code: "Digit1".into(),
                kc: 49,
                mods: 2
            },
            false,
            false,
            false,
            false
        )
        .action,
        Action::Send
    );
    assert_eq!(
        client_action(
            &ClientKey {
                key: "1".into(),
                code: "Numpad1".into(),
                kc: 97,
                mods: 2
            },
            false,
            false,
            false,
            false
        )
        .action,
        Action::Pass
    );
}

/// The normalized underscore collision really loses information, but the two
/// fixture states are not proof that a normal installed layout produces both.
#[test]
fn normalized_symbol_shift_collision_is_recorded_as_an_assumption() {
    let e = KeyDown {
        modifiers: Modifiers {
            control: true,
            ..Modifiers::default()
        },
        ..event("_")
    };
    let Decision::Send(GridInput::Key { mods, .. }) =
        map_key(e, Platform::Linux, &mut Composition::default(), terminal())
    else {
        panic!("US inferred shortcut missing");
    };
    assert_eq!(mods, 5, "US shifted-symbol inference must remain visible");
    assert_eq!(
        hex(&encode_key("_", "Minus", 189, 4, false, KeyPlatform::Other)),
        ""
    );
    assert_eq!(
        hex(&encode_key("_", "Minus", 189, 5, false, KeyPlatform::Other)),
        "1f"
    );
}

/// UTF-8 and JSON bounds are distinct: control-heavy text can hit the wire limit
/// first, and a split must never tear an emoji or lose a byte of the commit.
#[test]
fn committed_text_respects_both_bounds_and_round_trips() {
    for original in [
        "相😀".repeat(10000),
        "\0".repeat(30000),
        "\\\"\n".repeat(15000),
        String::new(),
    ] {
        let messages = committed_text(&original);
        let mut joined = String::new();
        for message in messages {
            message.validate().unwrap();
            message.encode().unwrap();
            let GridInput::Text { data } = message else {
                unreachable!()
            };
            joined.push_str(&data);
        }
        assert_eq!(joined, original);
    }
}

/// Mouse fixture cases start in cell space, exactly as GridInput does. These
/// test mode admission against the actual encoder; pixel rounding is separate.
#[test]
fn mouse_table_admission_and_actual_supervisor_bytes() {
    let rows = rows(include_str!(
        "../../../farhelm-supervisor/tests/fixtures/grid-mouse.jsonl"
    ));
    let count = rows.len();
    for row in rows {
        let kind = match row[0].as_str().unwrap() {
            "down" => MouseEventKind::Down,
            "up" => MouseEventKind::Up,
            "move" => MouseEventKind::Move,
            "wheel" => MouseEventKind::Wheel,
            other => panic!("kind {other}"),
        };
        let mode = match row[6].as_str().unwrap() {
            "NONE" => MouseMode::Off,
            "VT200" => MouseMode::Click,
            "DRAG" => MouseMode::Drag,
            "ANY" => MouseMode::Motion,
            other => panic!("mode {other}"),
        };
        for mods in 0..8 {
            let button = row[1].as_i64().unwrap() as i8;
            let mut modes = modes(false);
            modes.mouse_reporting = match mode {
                MouseMode::Off => MouseReporting::Off,
                MouseMode::Click => MouseReporting::Clicks,
                MouseMode::Drag => MouseReporting::Drag,
                MouseMode::Motion => MouseReporting::Motion,
            };
            modes.mouse_encoding = if row[7] == "SGR" {
                MouseEncoding::Sgr
            } else {
                MouseEncoding::Default
            };
            let message = GridInput::Mouse {
                kind,
                button,
                x: row[2].as_u64().unwrap() as u16,
                y: row[3].as_u64().unwrap() as u16,
                mods: mods as u8,
            };
            let bytes = if mode_reports(mode, kind, button) {
                InputEncoder::new()
                    .encode(
                        &message,
                        &modes,
                        Size {
                            columns: row[4].as_u64().unwrap() as u16,
                            lines: row[5].as_u64().unwrap() as u16,
                        },
                    )
                    .unwrap()
            } else {
                Vec::new()
            };
            let expected = if row[8].is_array() {
                &row[8][mods]
            } else {
                &row[8]
            };
            assert_eq!(
                hex(&bytes),
                expected.as_str().unwrap(),
                "row={row}, mods={mods}"
            );
        }
    }
    eprintln!("Mouse: {count} rows, {} modifier cases", count * 8);
}

/// The cell is the old app's, so a pane fits as many columns and rows as it did there. A larger cell would also open
/// gaps between the box-drawing and block glyphs programs draw their borders with; whether those glyphs meet across
/// rows at a given text size is up to the renderer (the GUI terminal module's `Baselines`), not this rectangle.
///
/// Spec: for JetBrains Mono at 14 px (an 8.4 px advance, 18.48 px ascent plus descent), the height is rounded up to
/// whole device pixels (19 at ratio 1, 37 device pixels at ratio 2) and spread over the rows so the screen is a whole
/// number of logical pixels (an odd device height over an odd row count rounds the screen up); the width stays the
/// advance, adjusted so a row is a whole number of logical pixels; a zero size or an unusable measurement gives none.
#[test]
fn cells_are_the_old_apps_rounding_of_the_measured_character() {
    let cell = |cols, rows, ratio| {
        Geometry::for_character(8.4, 18.48, cols, rows, ratio)
            .map(|g| (g.cell_width, g.cell_height, g.columns, g.rows))
    };
    assert_eq!(cell(80, 24, 1.), Some((8.4, 19., 80, 24)));
    assert_eq!(cell(80, 24, 2.), Some((8.4, 18.5, 80, 24)));
    assert_eq!(cell(36, 8, 2.), Some((302. / 36., 18.5, 36, 8)));
    assert_eq!(cell(6, 3, 2.), Some((50. / 6., 56. / 3., 6, 3)));
    assert_eq!(cell(0, 24, 1.), None);
    assert_eq!(cell(80, 0, 1.), None);
    assert!(Geometry::for_character(f64::NAN, 18.48, 80, 24, 1.).is_none());
    assert!(Geometry::for_character(8.4, 0., 80, 24, 1.).is_none());
    assert!(Geometry::for_character(8.4, 18.48, 80, 24, -2.).is_none());
}

/// Motion equality is against the last report of any kind and ignores Meta;
/// wheel fractions and focus gating live in the client, not the byte encoder.
#[test]
fn pointer_rounding_dedup_wheel_and_focus_contracts() {
    let geometry = Geometry {
        cell_width: 10.0,
        cell_height: 20.0,
        columns: 80,
        rows: 24,
    };
    assert_eq!(geometry.cell(10.0, 20.0), Some((0, 0)));
    assert_eq!(geometry.cell(10.001, 20.001), Some((1, 1)));
    assert_eq!(geometry.cell(-1.0, 10000.0), Some((0, 23)));
    assert_eq!(geometry.cell(f64::NAN, 0.0), None);
    let mut d = MouseDeduper::default();
    assert!(d.admit(MouseEventKind::Down, 0, 1, 1, 0));
    assert!(d.admit(MouseEventKind::Move, 0, 1, 1, 0));
    assert!(!d.admit(MouseEventKind::Move, 0, 1, 1, 8));
    assert!(d.admit(MouseEventKind::Move, 0, 1, 1, 1));
    assert!(d.admit(MouseEventKind::Wheel, 0, 1, 1, 1));
    assert!(d.admit(MouseEventKind::Move, 0, 1, 1, 1));
    d.reset();
    assert!(d.admit(MouseEventKind::Move, 0, 1, 1, 1));
    let mut wheel = WheelAccumulator::default();
    for _ in 0..3 {
        assert_eq!(wheel.consume(20.0, WheelUnit::Pixels, 20.0, 24, 0), 0.0);
    }
    assert_eq!(wheel.consume(20.0, WheelUnit::Pixels, 20.0, 24, 0), 1.0);
    assert_eq!(wheel.consume(100.0, WheelUnit::Pixels, 20.0, 24, 1), 0.0);
    assert_eq!(wheel.consume(-1.0, WheelUnit::Lines, 20.0, 24, 4), -5.0);
    assert!(!wheel_goes_to_program(MouseMode::Off, false));
    assert!(wheel_goes_to_program(MouseMode::Off, true));
    assert_eq!(focus_report(true, false), None);
    let mut modes = modes(true);
    modes.alternate_screen = true;
    modes.focus_reporting = true;
    let mut encoder = InputEncoder::new();
    assert_eq!(
        encoder
            .encode(
                &GridInput::Mouse {
                    kind: MouseEventKind::Wheel,
                    button: 0,
                    x: 0,
                    y: 0,
                    mods: 0
                },
                &modes,
                Size {
                    columns: 80,
                    lines: 24
                }
            )
            .unwrap(),
        b"\x1bOA"
    );
    assert_eq!(
        encoder
            .encode(
                &focus_report(false, true).unwrap(),
                &modes,
                Size {
                    columns: 80,
                    lines: 24
                }
            )
            .unwrap(),
        b"\x1b[O"
    );
}

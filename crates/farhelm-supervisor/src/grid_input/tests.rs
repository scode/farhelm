//! Tests of the grid input encoder: keys and mouse reports against golden
//! tables the vendored xterm.js computes, and pastes, the cases the mouse
//! table leaves out, focus reports and `Input` framing against the bytes
//! today's terminal sends, worked out from the vendored bundle's code (the
//! module docs name the functions).

use super::*;
use farhelm_proto::grid::{GRID_MAX_PASTE_PART_BYTES, GRID_MAX_TEXT_INPUT_BYTES};
use terminal_host::protocol::ATOMIC_INPUT;

// ---- Helpers ----------------------------------------------------------------

/// A shell's modes: nothing set, the normal screen.
fn plain() -> Modes {
    Modes {
        bracketed_paste: false,
        mouse_reporting: MouseReporting::Off,
        mouse_encoding: MouseEncoding::Default,
        focus_reporting: false,
        application_cursor: false,
        application_keypad: false,
        alternate_screen: false,
        cursor_visible: true,
    }
}

const SIZE: Size = Size {
    columns: 80,
    lines: 24,
};

/// The largest screen the grid format allows, for coordinates past the
/// default encoding's limit.
const LARGEST: Size = Size {
    columns: 1000,
    lines: 1000,
};

/// A `key` message from a viewer that is not on a Mac.
fn key(key: &str, code: &str, kc: u32, mods: u8) -> GridInput {
    GridInput::Key {
        key: key.to_owned(),
        code: code.to_owned(),
        kc,
        mods,
        mac: false,
    }
}

fn paste(data: &str, last: bool) -> GridInput {
    GridInput::Paste {
        data: data.to_owned(),
        last,
    }
}

fn mouse(kind: MouseEventKind, button: i8, x: u16, y: u16, mods: u8) -> GridInput {
    GridInput::Mouse {
        kind,
        button,
        x,
        y,
        mods,
    }
}

/// Encode one message on a fresh encoder, which must accept it.
fn encode(input: &GridInput, modes: &Modes, size: Size) -> Vec<u8> {
    InputEncoder::new()
        .encode(input, modes, size)
        .expect("a lone message breaks no rule")
}

fn reporting(mode: MouseReporting, encoding: MouseEncoding) -> Modes {
    Modes {
        mouse_reporting: mode,
        mouse_encoding: encoding,
        ..plain()
    }
}

fn hex(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2), "odd hex {text:?}");
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex digit"))
        .collect()
}

// ---- Keys -------------------------------------------------------------------

/// Keyboard parity with today's terminal, for every key at once: the golden
/// table is the sequence the vendored xterm.js's own `evaluateKeyboardEvent`
/// computes, with Farhelm's Shift+Enter merge, for every key code below 256
/// (and the keys and codes its branches read), every modifier set, both
/// platforms and both cursor-key modes. Every row must match: a difference
/// is a key that types something else in a terminal host than in a tmux
/// terminal. The JS harness keeps the table in step with the bundle. Which
/// keydowns xterm.js's `_keyDown` sends at all is the grid client's to
/// decide and test, so the table covers keydowns it would drop too.
#[test]
fn every_key_encodes_as_the_vendored_xterm_js_computes_it() {
    let table = include_str!("../../tests/fixtures/xterm-keys.jsonl");
    let variants = [
        (KeyPlatform::Other, false),
        (KeyPlatform::Other, true),
        (KeyPlatform::Mac, false),
        (KeyPlatform::Mac, true),
    ];
    let mut compared = 0;
    let mut key_codes = std::collections::BTreeSet::new();
    for line in table.lines().filter(|line| !line.starts_with('#')) {
        let row: serde_json::Value = serde_json::from_str(line).expect("a JSON row");
        let kc = u32::try_from(row[0].as_u64().expect("key code")).expect("small key code");
        let key = row[1].as_str().expect("key");
        let code = row[2].as_str().expect("code");
        let sent = row[3].as_array().expect("one entry per modifier set");
        assert_eq!(sent.len(), 16, "row {line}");
        key_codes.insert(kc);
        for (mods, entry) in sent.iter().enumerate() {
            let mods = u8::try_from(mods).expect("16 modifier sets");
            for (index, (platform, application_cursor)) in variants.into_iter().enumerate() {
                let want = match entry {
                    serde_json::Value::String(same) => hex(same),
                    serde_json::Value::Array(each) => hex(each[index].as_str().expect("hex")),
                    other => panic!("unexpected entry {other} in {line}"),
                };
                let got = encode_key(key, code, kc, mods, application_cursor, platform);
                assert_eq!(
                    got, want,
                    "key code {kc} key {key:?} code {code:?} mods {mods} {platform:?} \
                     application cursor {application_cursor}"
                );
                compared += 1;
            }
        }
    }
    assert!(
        (0..256).all(|kc| key_codes.contains(&kc)),
        "the table must cover every key code below 256"
    );
    assert!(compared > 90_000, "only {compared} comparisons");
}

/// SPEC.md promises Shift+Enter inserts a newline in agents: ESC CR, and in
/// one write, since programs that tell a lone Escape apart by read boundary
/// (Codex) read a split pair as Escape then Enter. One message's bytes are
/// one `Input` frame of at most `ATOMIC_INPUT` bytes, which the host offers
/// to the pty in one write. Other modifiers keep xterm.js's own encoding.
#[test]
fn shift_enter_is_esc_cr_in_one_atomic_input_frame() {
    for code in ["Enter", "NumpadEnter"] {
        let bytes = encode(&key("Enter", code, 13, modifiers::SHIFT), &plain(), SIZE);
        assert_eq!(bytes, b"\x1b\r");
        let frames: Vec<&[u8]> = input_frames(&bytes).collect();
        assert_eq!(frames, [b"\x1b\r".as_slice()]);
        assert!(frames[0].len() <= ATOMIC_INPUT);
    }
    let enter = |mods| encode(&key("Enter", "Enter", 13, mods), &plain(), SIZE);
    assert_eq!(enter(0), b"\r");
    assert_eq!(enter(modifiers::ALT), b"\x1b\r");
    assert_eq!(enter(modifiers::SHIFT | modifiers::ALT), b"\x1b\r");
    assert_eq!(enter(modifiers::SHIFT | modifiers::CTRL), b"\r");
}

/// Escape is a lone ESC (SPEC.md's other promised key), and Alt+Escape two.
#[test]
fn escape_is_one_esc() {
    let escape = |mods| encode(&key("Escape", "Escape", 27, mods), &plain(), SIZE);
    assert_eq!(escape(0), b"\x1b");
    assert_eq!(escape(modifiers::ALT), b"\x1b\x1b");
}

/// The encoder reads application cursor keys from the program's current
/// modes, not from anything the client sent: arrows switch from CSI to SS3
/// when a full-screen program sets DECCKM, and modified arrows keep their
/// CSI form either way.
#[test]
fn arrows_follow_the_programs_cursor_key_mode() {
    let application = Modes {
        application_cursor: true,
        ..plain()
    };
    let up = key("ArrowUp", "ArrowUp", 38, 0);
    assert_eq!(encode(&up, &plain(), SIZE), b"\x1b[A");
    assert_eq!(encode(&up, &application, SIZE), b"\x1bOA");
    let shift_up = key("ArrowUp", "ArrowUp", 38, modifiers::SHIFT);
    assert_eq!(encode(&shift_up, &application, SIZE), b"\x1b[1;2A");
}

/// xterm.js has no application keypad: the keypad's keys type their
/// characters whatever DECKPAM says, so the encoder ignores that mode too.
#[test]
fn the_keypad_types_its_characters_in_either_keypad_mode() {
    let keypad = Modes {
        application_keypad: true,
        ..plain()
    };
    for (kc, character, code) in [(97, "1", "Numpad1"), (107, "+", "NumpadAdd")] {
        let input = key(character, code, kc, 0);
        assert_eq!(encode(&input, &plain(), SIZE), character.as_bytes());
        assert_eq!(encode(&input, &keypad, SIZE), character.as_bytes());
    }
}

/// The viewer's platform changes some keys' bytes, and the encoder takes it
/// from each `key` message's `mac` field: off a Mac, Alt is Meta (ESC plus
/// the key); on a Mac, Option is left to the browser, and Ctrl+Option+Shift
/// with `_` is plain Ctrl+_. One encoder serves both, since the field is per
/// message.
#[test]
fn alt_is_meta_only_off_a_mac() {
    let mut encoder = InputEncoder::new();
    let mut encode_on = |mac, input: GridInput| {
        let GridInput::Key {
            key,
            code,
            kc,
            mods,
            ..
        } = input
        else {
            unreachable!("a key message")
        };
        let input = GridInput::Key {
            key,
            code,
            kc,
            mods,
            mac,
        };
        encoder.encode(&input, &plain(), SIZE).unwrap()
    };
    let alt_b = || key("b", "KeyB", 66, modifiers::ALT);
    assert_eq!(encode_on(false, alt_b()), b"\x1bb");
    assert_eq!(encode_on(true, alt_b()), b"");
    let underscore = || {
        key(
            "_",
            "Minus",
            189,
            modifiers::CTRL | modifiers::ALT | modifiers::SHIFT,
        )
    };
    assert_eq!(encode_on(false, underscore()), b"\x1b_");
    assert_eq!(encode_on(true, underscore()), b"\x1f");
}

/// Text (typed characters the browser resolved, composition results) is
/// written exactly as sent, as UTF-8, control characters included.
#[test]
fn text_is_written_as_is() {
    let text = GridInput::Text {
        data: "héllo 😀\x03\r\n".to_owned(),
    };
    assert_eq!(encode(&text, &plain(), SIZE), "héllo 😀\x03\r\n".as_bytes());
}

// ---- Paste ------------------------------------------------------------------

/// Bracketed paste (mode 2004) is how agents tell a paste from typing: a
/// pasted newline must not submit. The markers wrap the paste when the
/// program asked, and every CRLF and LF becomes CR as xterm.js converts
/// them, so a pasted line ends as a typed Enter would.
#[test]
fn a_paste_is_bracketed_when_the_program_asked_and_its_line_endings_converted() {
    let bracketed = Modes {
        bracketed_paste: true,
        ..plain()
    };
    let text = paste("one\r\ntwo\nthree\rfour\n\n", true);
    assert_eq!(
        encode(&text, &bracketed, SIZE),
        b"\x1b[200~one\rtwo\rthree\rfour\r\r\x1b[201~"
    );
    assert_eq!(encode(&text, &plain(), SIZE), b"one\rtwo\rthree\rfour\r\r");
}

/// A long paste arrives in parts; the program must still see one paste:
/// one opening marker before the first part, one closing marker after the
/// last, none between.
#[test]
fn a_paste_in_parts_is_bracketed_once_around_the_whole() {
    let bracketed = Modes {
        bracketed_paste: true,
        ..plain()
    };
    let mut encoder = InputEncoder::new();
    let mut part = |data, last| {
        encoder
            .encode(&paste(data, last), &bracketed, SIZE)
            .unwrap()
    };
    assert_eq!(part("one\n", false), b"\x1b[200~one\r");
    assert_eq!(part("two\n", false), b"two\r");
    assert_eq!(part("three", true), b"three\x1b[201~");
    // The next paste is a new one.
    assert_eq!(part("four", true), b"\x1b[200~four\x1b[201~");
}

/// Converting part by part must give what converting the whole paste gives,
/// even where a CRLF straddles two parts (which the grid client avoids, but
/// a CRLF counted twice would paste a blank line).
#[test]
fn a_crlf_split_across_parts_converts_as_one_line_ending() {
    let mut encoder = InputEncoder::new();
    let mut part = |data, last| encoder.encode(&paste(data, last), &plain(), SIZE).unwrap();
    assert_eq!(part("x\r", false), b"x\r");
    assert_eq!(part("\ny\r", false), b"y\r");
    assert_eq!(part("\r", false), b"\r");
    assert_eq!(part("\n\n", true), b"\r");
}

/// The CR carry must survive a part with nothing in it: a refactor that
/// reset it on an empty part would turn the CRLF of `x\r`, ``, `\n` into
/// two line endings.
#[test]
fn the_cr_carry_survives_an_empty_middle_part() {
    let mut encoder = InputEncoder::new();
    let mut part = |data, last| encoder.encode(&paste(data, last), &plain(), SIZE).unwrap();
    assert_eq!(part("x\r", false), b"x\r");
    assert_eq!(part("", false), b"");
    assert_eq!(part("\n", true), b"");
}

/// Sanitizing part by part must give what sanitizing the whole paste gives:
/// an end-of-paste marker split after its ESC goes whole, and a held start
/// of a marker that turns out not to be one loses only its ESC, at the next
/// part or at the paste's end.
#[test]
fn an_end_of_paste_marker_split_across_parts_is_still_removed_whole() {
    let mut encoder = InputEncoder::new();
    let mut part = |data, last| encoder.encode(&paste(data, last), &plain(), SIZE).unwrap();
    assert_eq!(part("a\x1b", false), b"a");
    assert_eq!(part("[20", false), b"");
    assert_eq!(part("1~b\x1b[20", false), b"b");
    assert_eq!(part("x\x1b\x1b[2", true), b"[20x[2");
}

/// Today's page checks a paste for emptiness before sanitizing it, so a
/// paste of nothing but ESC still writes the two markers (around nothing);
/// the emptiness check must stay on the text as sent.
#[test]
fn a_paste_that_sanitizes_to_nothing_still_writes_its_markers() {
    let bracketed = Modes {
        bracketed_paste: true,
        ..plain()
    };
    assert_eq!(
        encode(&paste("\x1b", true), &bracketed, SIZE),
        b"\x1b[200~\x1b[201~"
    );
}

/// The grid sender must hold its own writes to the program (query answers,
/// prompt sends) while a paste is open, or the program reads them as pasted
/// text; `paste_open` is how it knows, from the first part until the last
/// one or `finish`.
#[test]
fn paste_open_spans_the_first_part_to_the_last() {
    let mut encoder = InputEncoder::new();
    assert!(!encoder.paste_open());
    encoder.encode(&paste("a", false), &plain(), SIZE).unwrap();
    assert!(encoder.paste_open());
    encoder.encode(&paste("b", true), &plain(), SIZE).unwrap();
    assert!(!encoder.paste_open());
    encoder.encode(&paste("c", false), &plain(), SIZE).unwrap();
    encoder.finish();
    assert!(!encoder.paste_open());
    encoder
        .encode(&paste("whole", true), &plain(), SIZE)
        .unwrap();
    assert!(!encoder.paste_open(), "a one-part paste never stays open");
}

/// Pasted text must not be able to end the bracketed paste around it and
/// have what follows run as typed commands: every ESC is dropped, an
/// end-of-paste marker whole, as Farhelm's page does today.
#[test]
fn pasted_text_cannot_close_the_bracketed_paste() {
    let bracketed = Modes {
        bracketed_paste: true,
        ..plain()
    };
    let hostile = paste("ls\x1b[201~rm -rf x\n\x1b[200~\x1b", true);
    assert_eq!(
        encode(&hostile, &bracketed, SIZE),
        b"\x1b[200~lsrm -rf x\r[200~\x1b[201~"
    );
}

/// The markers must pair up: whether a paste is bracketed is decided when
/// it begins, so a program changing mode 2004 while the parts arrive gets
/// neither a lone opening marker nor a lone closing one.
#[test]
fn bracketing_is_decided_when_the_paste_begins() {
    let bracketed = Modes {
        bracketed_paste: true,
        ..plain()
    };
    let mut encoder = InputEncoder::new();
    assert_eq!(
        encoder
            .encode(&paste("a", false), &bracketed, SIZE)
            .unwrap(),
        b"\x1b[200~a"
    );
    assert_eq!(
        encoder.encode(&paste("b", true), &plain(), SIZE).unwrap(),
        b"b\x1b[201~"
    );
    assert_eq!(
        encoder.encode(&paste("c", false), &plain(), SIZE).unwrap(),
        b"c"
    );
    assert_eq!(
        encoder.encode(&paste("d", true), &bracketed, SIZE).unwrap(),
        b"d"
    );
}

/// An attachment that ends mid-paste (the viewer closed, the connection
/// dropped) must not leave the program in paste mode, reading everything
/// typed later as pasted text: `finish` gives the closing marker, once, and
/// only for a bracketed paste that was opened.
#[test]
fn an_interrupted_paste_gets_its_closing_marker_from_finish() {
    let bracketed = Modes {
        bracketed_paste: true,
        ..plain()
    };
    let mut encoder = InputEncoder::new();
    assert_eq!(encoder.finish(), None, "no paste, nothing to close");
    encoder
        .encode(&paste("half", false), &bracketed, SIZE)
        .unwrap();
    assert_eq!(encoder.finish(), Some(PASTE_END));
    assert_eq!(encoder.finish(), None, "closed once");

    encoder
        .encode(&paste("half", false), &plain(), SIZE)
        .unwrap();
    assert_eq!(
        encoder.finish(),
        None,
        "an unbracketed paste has no marker to close"
    );

    encoder
        .encode(&paste("whole", true), &bracketed, SIZE)
        .unwrap();
    assert_eq!(encoder.finish(), None, "a finished paste is closed already");
}

/// The format's rule that a paste's parts are contiguous: any other message
/// between them would land inside the pasted text, so it is refused, and
/// the paste stays open for `finish` to close as the attachment ends.
#[test]
fn input_between_the_parts_of_a_paste_is_refused() {
    let bracketed = Modes {
        bracketed_paste: true,
        focus_reporting: true,
        ..plain()
    };
    let interruptions = [
        (key("a", "KeyA", 65, 0), "key"),
        (
            GridInput::Text {
                data: "a".to_owned(),
            },
            "text",
        ),
        (mouse(MouseEventKind::Down, 0, 0, 0, 0), "mouse"),
        (GridInput::Focus { focused: false }, "focus"),
    ];
    for (input, kind) in interruptions {
        let mut encoder = InputEncoder::new();
        encoder
            .encode(&paste("a", false), &bracketed, SIZE)
            .unwrap();
        assert_eq!(
            encoder.encode(&input, &bracketed, SIZE),
            Err(InputError::PasteInterrupted(kind))
        );
        assert_eq!(encoder.finish(), Some(PASTE_END));
    }
}

/// Today's page drops an empty paste before xterm.js would write a pair of
/// markers around nothing; an empty part inside a longer paste is just
/// empty.
#[test]
fn an_empty_paste_writes_nothing() {
    let bracketed = Modes {
        bracketed_paste: true,
        ..plain()
    };
    assert_eq!(encode(&paste("", true), &bracketed, SIZE), b"");
    let mut encoder = InputEncoder::new();
    assert_eq!(
        encoder.encode(&paste("", false), &bracketed, SIZE).unwrap(),
        b"\x1b[200~"
    );
    assert_eq!(
        encoder.encode(&paste("", true), &bracketed, SIZE).unwrap(),
        b"\x1b[201~"
    );
}

// ---- Mouse ------------------------------------------------------------------

/// Mouse parity with today's terminal: the golden table is what the
/// vendored xterm.js's own mouse service (its protocols' filters, event
/// codes, encodings and `triggerMouseEvent`'s checks) sends for every kind
/// of grid mouse event, every Shift, Alt and Ctrl combination, each
/// reporting mode the emulator tracks, both of xterm.js's encodings, and
/// coordinates at the screen's edge and the default encoding's limit. A
/// difference is a click or scroll a program reads differently in a
/// terminal host. The JS harness keeps the table in step with the bundle;
/// UTF-8 coordinates and the alternate screen's arrow keys, which are not in
/// the mouse service, are tested below.
#[test]
fn every_mouse_event_encodes_as_the_vendored_xterm_js_reports_it() {
    let table = include_str!("../../tests/fixtures/xterm-mouse.jsonl");
    let mut compared = 0;
    for line in table.lines().filter(|line| !line.starts_with('#')) {
        let row: serde_json::Value = serde_json::from_str(line).expect("a JSON row");
        let field = |index: usize| row[index].as_u64().expect("a number");
        let kind = match row[0].as_str().expect("kind") {
            "down" => MouseEventKind::Down,
            "up" => MouseEventKind::Up,
            "move" => MouseEventKind::Move,
            "wheel" => MouseEventKind::Wheel,
            other => panic!("unknown kind {other} in {line}"),
        };
        let button = i8::try_from(row[1].as_i64().expect("button")).expect("small button");
        let [x, y, columns, lines] =
            [2, 3, 4, 5].map(|index| u16::try_from(field(index)).expect("a grid coordinate"));
        let mode = match row[6].as_str().expect("protocol") {
            "NONE" => MouseReporting::Off,
            "VT200" => MouseReporting::Clicks,
            "DRAG" => MouseReporting::Drag,
            "ANY" => MouseReporting::Motion,
            other => panic!("unknown protocol {other} in {line}"),
        };
        let encoding = match row[7].as_str().expect("encoding") {
            "DEFAULT" => MouseEncoding::Default,
            "SGR" => MouseEncoding::Sgr,
            other => panic!("unknown encoding {other} in {line}"),
        };
        let modes = reporting(mode, encoding);
        let size = Size { columns, lines };
        for mods in 0..8u8 {
            let want = match &row[8] {
                serde_json::Value::String(same) => hex(same),
                serde_json::Value::Array(each) => {
                    hex(each[usize::from(mods)].as_str().expect("hex"))
                }
                other => panic!("unexpected entry {other} in {line}"),
            };
            let got = encode(&mouse(kind, button, x, y, mods), &modes, size);
            assert_eq!(got, want, "{line} with mods {mods}");
            compared += 1;
        }
    }
    assert!(compared > 6_000, "only {compared} comparisons");
}

/// Each reporting mode reports what xterm.js's protocol reports and drops
/// the rest: 1000 presses, releases and the wheel; 1002 also motion with a
/// button held; 1003 all motion. With reporting off nothing is sent. The
/// client filters by the mode it last heard of, which can be stale, so the
/// encoder's own check is the one that counts.
#[test]
fn each_reporting_mode_reports_only_its_events() {
    use MouseEventKind::{Down, Move, Up, Wheel};
    let events = [
        (Down, 0, "\x1b[<0;3;2M"),
        (Up, 0, "\x1b[<0;3;2m"),
        (Wheel, 1, "\x1b[<65;3;2M"),
        (Move, 0, "\x1b[<32;3;2M"),
        (Move, -1, "\x1b[<35;3;2M"),
    ];
    let reports = [
        (MouseReporting::Off, [false, false, false, false, false]),
        (MouseReporting::Clicks, [true, true, true, false, false]),
        (MouseReporting::Drag, [true, true, true, true, false]),
        (MouseReporting::Motion, [true, true, true, true, true]),
    ];
    for (mode, reported) in reports {
        let modes = reporting(mode, MouseEncoding::Sgr);
        for ((kind, button, sgr), reported) in events.into_iter().zip(reported) {
            let want = if reported {
                sgr.as_bytes()
            } else {
                b"".as_slice()
            };
            assert_eq!(
                encode(&mouse(kind, button, 2, 1, 0), &modes, SIZE),
                want,
                "{mode:?} {kind:?} button {button}"
            );
        }
    }
}

/// The default encoding: `CSI M` and three bytes, each value plus 32, with
/// 1-based coordinates; a release says button 3, since the encoding cannot
/// say which was released. Modifiers add Shift 4, Alt 8 and Ctrl 16; Meta
/// is not reported.
#[test]
fn the_default_encoding_writes_three_bytes() {
    let modes = reporting(MouseReporting::Motion, MouseEncoding::Default);
    let report = |kind, button, mods| encode(&mouse(kind, button, 4, 2, mods), &modes, SIZE);
    assert_eq!(report(MouseEventKind::Down, 0, 0), b"\x1b[M %#");
    assert_eq!(report(MouseEventKind::Down, 2, 0), b"\x1b[M\"%#");
    assert_eq!(report(MouseEventKind::Up, 1, 0), b"\x1b[M#%#");
    assert_eq!(report(MouseEventKind::Move, 0, 0), b"\x1b[M@%#");
    assert_eq!(report(MouseEventKind::Move, -1, 0), b"\x1b[MC%#");
    assert_eq!(
        report(
            MouseEventKind::Down,
            0,
            modifiers::SHIFT | modifiers::ALT | modifiers::CTRL
        ),
        b"\x1b[M<%#"
    );
    assert_eq!(
        report(MouseEventKind::Down, 0, modifiers::META),
        b"\x1b[M %#"
    );
}

/// The default encoding cannot express a column or row past 223 (a byte
/// past 255), and xterm.js sends nothing rather than a wrong cell. SGR and
/// UTF-8 coordinates have no such limit within the largest grid.
#[test]
fn the_default_encoding_drops_events_past_its_byte_limit() {
    let default = reporting(MouseReporting::Clicks, MouseEncoding::Default);
    let down = |x, y| mouse(MouseEventKind::Down, 0, x, y, 0);
    assert_eq!(
        encode(&down(222, 222), &default, LARGEST),
        b"\x1b[M \xff\xff"
    );
    assert_eq!(encode(&down(223, 0), &default, LARGEST), b"");
    assert_eq!(encode(&down(0, 223), &default, LARGEST), b"");
    let sgr = reporting(MouseReporting::Clicks, MouseEncoding::Sgr);
    assert_eq!(
        encode(&down(999, 999), &sgr, LARGEST),
        b"\x1b[<0;1000;1000M"
    );
    let utf8 = reporting(MouseReporting::Clicks, MouseEncoding::Utf8);
    assert_eq!(
        encode(&down(999, 0), &utf8, LARGEST),
        "\x1b[M \u{408}!".as_bytes()
    );
}

/// SGR (1006) keeps the released button and ends a release with `m`; the
/// wheel is buttons 64 (up) and 65 (down), which always end with `M`.
#[test]
fn sgr_reports_say_which_button_was_released() {
    let modes = reporting(MouseReporting::Clicks, MouseEncoding::Sgr);
    let report = |kind, button, mods| encode(&mouse(kind, button, 0, 0, mods), &modes, SIZE);
    assert_eq!(report(MouseEventKind::Down, 2, 0), b"\x1b[<2;1;1M");
    assert_eq!(report(MouseEventKind::Up, 2, 0), b"\x1b[<2;1;1m");
    assert_eq!(report(MouseEventKind::Wheel, 0, 0), b"\x1b[<64;1;1M");
    assert_eq!(report(MouseEventKind::Wheel, 1, 0), b"\x1b[<65;1;1M");
    assert_eq!(
        report(MouseEventKind::Wheel, 0, modifiers::CTRL),
        b"\x1b[<80;1;1M"
    );
}

/// UTF-8 coordinates (1005), which the emulator tracks though xterm.js
/// does not: the default encoding's values, each a UTF-8 character, so a
/// value past 127 takes two bytes where the default encoding would write
/// one byte the program reads as broken UTF-8.
#[test]
fn utf8_reports_encode_large_values_as_characters() {
    let modes = reporting(MouseReporting::Clicks, MouseEncoding::Utf8);
    let down = |x| encode(&mouse(MouseEventKind::Down, 0, x, 0, 0), &modes, LARGEST);
    assert_eq!(down(94), b"\x1b[M \x7f!");
    assert_eq!(down(95), b"\x1b[M \xc2\x80!");
    let up = encode(&mouse(MouseEventKind::Up, 0, 0, 0, 0), &modes, SIZE);
    assert_eq!(up, b"\x1b[M#!!");
}

/// A program that asks for SGR (1006) and also for UTF-8 coordinates
/// (1005) gets SGR reports, whichever it set first, as from tmux and
/// today's xterm.js; resetting 1006 leaves UTF-8. The modes are read from a
/// real emulator fed the program's output, since the order is what the
/// emulator used to get wrong (it kept only the mode set last).
#[test]
fn sgr_wins_over_utf8_whichever_was_set_first() {
    use farhelm_terminal_emulator::{Collected, Emulator, FeedMode};
    let modes_after = |output: &[u8]| {
        let mut emulator = Emulator::new(SIZE, 0);
        let mut effects = Collected::default();
        emulator.feed(
            output,
            std::time::Instant::now(),
            FeedMode::Live,
            &mut effects,
        );
        emulator.modes()
    };
    let down = mouse(MouseEventKind::Down, 0, 199, 0, 0);
    for output in [
        &b"\x1b[?1000h\x1b[?1006h\x1b[?1005h"[..],
        b"\x1b[?1000h\x1b[?1005h\x1b[?1006h",
        b"\x1b[?1000;1005;1006h",
    ] {
        let modes = modes_after(output);
        assert_eq!(modes.mouse_encoding, MouseEncoding::Sgr, "after {output:?}");
        assert_eq!(
            encode(&down, &modes, LARGEST),
            b"\x1b[<0;200;1M",
            "after {output:?}"
        );
    }
    let modes = modes_after(b"\x1b[?1000h\x1b[?1006h\x1b[?1005h\x1b[?1006l");
    assert_eq!(modes.mouse_encoding, MouseEncoding::Utf8);
    assert_eq!(encode(&down, &modes, LARGEST), "\x1b[M \u{e8}!".as_bytes());
}

/// The screen may have shrunk since the client sent an event; one past the
/// current screen is dropped, as xterm.js drops one outside its grid.
#[test]
fn events_past_the_current_screen_are_dropped() {
    let modes = reporting(MouseReporting::Clicks, MouseEncoding::Sgr);
    assert_eq!(
        encode(&mouse(MouseEventKind::Down, 0, 80, 0, 0), &modes, SIZE),
        b""
    );
    assert_eq!(
        encode(&mouse(MouseEventKind::Down, 0, 0, 24, 0), &modes, SIZE),
        b""
    );
    assert_eq!(
        encode(&mouse(MouseEventKind::Down, 0, 79, 23, 0), &modes, SIZE),
        b"\x1b[<0;80;24M"
    );
}

/// Scrolling in a full-screen program without mouse reporting (a pager, an
/// editor) moves through it with arrow keys, one per wheel step, as
/// xterm.js does on a screen without scrollback; on the normal screen the
/// wheel scrolls the viewer's history and sends nothing; with reporting on,
/// the wheel is a mouse report even on the alternate screen.
#[test]
fn the_wheel_on_the_alternate_screen_without_reporting_sends_arrows() {
    let alternate = Modes {
        alternate_screen: true,
        ..plain()
    };
    let wheel = |button| mouse(MouseEventKind::Wheel, button, 0, 0, modifiers::SHIFT);
    assert_eq!(encode(&wheel(0), &alternate, SIZE), b"\x1b[A");
    assert_eq!(encode(&wheel(1), &alternate, SIZE), b"\x1b[B");
    let application = Modes {
        application_cursor: true,
        ..alternate
    };
    assert_eq!(encode(&wheel(0), &application, SIZE), b"\x1bOA");
    assert_eq!(encode(&wheel(1), &application, SIZE), b"\x1bOB");
    assert_eq!(encode(&wheel(0), &plain(), SIZE), b"");
    let reporting_alternate = Modes {
        mouse_reporting: MouseReporting::Clicks,
        mouse_encoding: MouseEncoding::Sgr,
        ..alternate
    };
    assert_eq!(
        encode(&wheel(0), &reporting_alternate, SIZE),
        b"\x1b[<68;1;1M"
    );
    // Other events with reporting off send nothing, alternate screen or not.
    assert_eq!(
        encode(&mouse(MouseEventKind::Down, 0, 0, 0, 0), &alternate, SIZE),
        b""
    );
}

// ---- Focus ------------------------------------------------------------------

/// Focus reports go only to a program that asked for them (mode 1004);
/// anything else would read them as stray input.
#[test]
fn focus_is_reported_only_under_mode_1004() {
    let focus = |focused| GridInput::Focus { focused };
    assert_eq!(encode(&focus(true), &plain(), SIZE), b"");
    assert_eq!(encode(&focus(false), &plain(), SIZE), b"");
    let reporting = Modes {
        focus_reporting: true,
        ..plain()
    };
    assert_eq!(encode(&focus(true), &reporting, SIZE), b"\x1b[I");
    assert_eq!(encode(&focus(false), &reporting, SIZE), b"\x1b[O");
}

// ---- Input frames -----------------------------------------------------------

/// The host refuses an `Input` longer than `MAX_INPUT` or empty, and offers
/// one of at most `ATOMIC_INPUT` bytes to the pty in one write; frames are
/// cut at `MAX_INPUT` only, so anything that fits stays whole.
#[test]
fn input_frames_cut_only_past_the_hosts_limit() {
    assert_eq!(input_frames(b"").count(), 0);
    let fits = vec![b'x'; MAX_INPUT];
    assert_eq!(input_frames(&fits).collect::<Vec<_>>(), [fits.as_slice()]);
    let over = vec![b'x'; MAX_INPUT + 1];
    let frames: Vec<usize> = input_frames(&over).map(<[u8]>::len).collect();
    assert_eq!(frames, [MAX_INPUT, 1]);
}

/// Every message's bytes fit one frame, so the grid sender writes one
/// `Input` per message: the largest text and the largest paste part
/// (bracketed, with both markers) stay under `MAX_INPUT`.
#[test]
fn the_largest_messages_fit_one_input_frame() {
    let text = GridInput::Text {
        data: "é".repeat(GRID_MAX_TEXT_INPUT_BYTES / 2),
    };
    let bytes = encode(&text, &plain(), SIZE);
    assert_eq!(input_frames(&bytes).count(), 1);
    let bracketed = Modes {
        bracketed_paste: true,
        ..plain()
    };
    let part = paste(&"\n".repeat(GRID_MAX_PASTE_PART_BYTES), true);
    let bytes = encode(&part, &bracketed, SIZE);
    assert_eq!(
        bytes.len(),
        GRID_MAX_PASTE_PART_BYTES + PASTE_START.len() + PASTE_END.len()
    );
    assert_eq!(input_frames(&bytes).count(), 1);
}

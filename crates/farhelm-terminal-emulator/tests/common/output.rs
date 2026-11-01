//! Generated terminal output for the property tests: a mix of text, wide and
//! multi-byte characters, and the escape sequences that change the parser's
//! and the terminal's state, including synchronized updates, strings (OSC,
//! DCS) with both terminators, cancelled sequences, queries, and raw bytes.
//!
//! Hyperlinks always carry an explicit id: alacritty numbers id-less links
//! from a process-wide counter, so two emulators fed the same output would
//! hold different ids and compare unequal for a reason that has nothing to do
//! with the test.
//!
//! Update starts come in every spelling vte accepts, not only the exact
//! `ESC [ ? 2026 h` it searches for inside an open update: with parameters
//! around 2026, with a control byte or DEL between its digits, split across
//! tokens, or after a lone ESC and a control byte. Those spellings are where
//! vte's handling of an open update depends on how output is cut into
//! calls.
//!
//! [`stream`] includes the mode changes where the emulator deliberately
//! departs from alacritty (mouse tracking resets, modes 47 and 1047);
//! [`alacritty_stream`] leaves them out, for comparing against a bare
//! alacritty terminal.

use proptest::prelude::*;

/// Output where the emulator follows tmux rather than alacritty.
const DEPARTURES: &[&[u8]] = &[
    b"\x1b[?47h",
    b"\x1b[?47l",
    b"\x1b[?1047h",
    b"\x1b[?1047l",
    b"\x1b[?1000l",
    b"\x1b[?1001l",
    b"\x1b[?1002l",
    b"\x1b[?1003h",
    b"\x1b[?1003l",
];

/// One piece of terminal output, with or without the [`DEPARTURES`].
fn token(departures: bool) -> impl Strategy<Value = Vec<u8>> {
    let mut fixed: Vec<&'static [u8]> = vec![
        b"\r\n",
        b"\t",
        b"\x08",
        b"\x07",
        "é".as_bytes(),
        "中".as_bytes(),
        "😀".as_bytes(),
        "e\u{301}".as_bytes(),
        b"\x1b[?2026h",
        b"\x1b[?2026l",
        b"\x1b[?2026;1h",
        b"\x1b[?1;2026h",
        b"\x1b[?02026h",
        b"\x1b[?2026l\x1b[?2026;1h",
        b"\x1b[?2026l\x1b[?2026h",
        b"\x1b[?202",
        b"6h",
        b"\x1b[?20\r26h",
        b"\x1b[?20\x7f26h",
        b"\x1b[?20\x0026h",
        b"\x1b\x07",
        b"\x1b\r[?2026h",
        b"\x1b[?1049h",
        b"\x1b[?1049l",
        b"\x1b[?2004h",
        b"\x1b[?1000h",
        b"\x1b[?1006h",
        b"\x1b[?1h",
        b"\x1b[?6h",
        b"\x1b[?6l",
        b"\x1b[?7l",
        b"\x1b[?7h",
        b"\x1b[?3h",
        b"\x1b[?12h",
        b"\x1b[?25l",
        b"\x1b[4h",
        b"\x1b[4l",
        b"\x1b[20h",
        b"\x1b=",
        b"\x1b>",
        b"\x1b7",
        b"\x1b8",
        b"\x1bM",
        b"\x1bD",
        b"\x1bE",
        b"\x1bH",
        b"\x1b#8",
        b"\x1b(0",
        b"\x1b(B",
        b"\x0e",
        b"\x0f",
        b"\x1bc",
        b"\x1b[22t",
        b"\x1b[23t",
        b"\x1b[3g",
        b"\x1b[1 q",
        b"\x1b[5 q",
        b"\x1b[>1u",
        b"\x1b[>4;2m",
        b"\x1b]4;1;#123456\x07",
        b"\x1b]104\x07",
        b"\x1b]11;#202020\x1b\\",
        b"\x1b]8;id=x;http://a\x1b\\",
        b"\x1b]8;;\x1b\\",
        b"\x1b]52;c;aGk=\x07",
        b"\x1b]22;pointer\x07",
        b"\x1bP1$qm\x1b\\",
        b"\x1b_Gf=100;AAAA\x1b\\",
        b"\x1b[12\x18",
        b"\x1b]2;cancelled\x1a",
        b"\x1b[c",
        b"\x1b[>c",
        b"\x1b[6n",
        b"\x1b[?2026$p",
        b"\x1b[?12$p",
        b"\x1b]11;?\x07",
        b"\x1b[14t",
    ];
    if departures {
        fixed.extend_from_slice(DEPARTURES);
    }
    prop_oneof![
        4 => "[a-z ]{1,12}".prop_map(String::into_bytes),
        4 => prop::sample::select(fixed).prop_map(<[u8]>::to_vec),
        2 => (0u16..30, 0u16..30, prop::sample::select(b"mHJKABCDGdSTLMPX@bIZrfe`".to_vec()))
            .prop_map(|(a, b, final_byte)| format!("\x1b[{a};{b}{}", final_byte as char).into_bytes()),
        1 => (0u16..256, 0u16..256, 0u16..256)
            .prop_map(|(r, g, b)| format!("\x1b[38;2;{r};{g};{b}m\x1b[48;5;{r}m").into_bytes()),
        1 => "[a-z;]{0,10}".prop_map(|title| format!("\x1b]2;{title}\x07").into_bytes()),
        1 => "[a-z]{0,10}".prop_map(|title| format!("\x1b]0;{title}\x1b\\").into_bytes()),
        1 => any::<u8>().prop_map(|byte| vec![byte]),
    ]
}

/// A stream of up to `max_tokens` pieces of output.
pub fn stream(max_tokens: usize) -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(token(true), 0..max_tokens).prop_map(|tokens| tokens.concat())
}

/// A stream without the [`DEPARTURES`], which a bare alacritty terminal
/// interprets the same way as the emulator.
pub fn alacritty_stream(max_tokens: usize) -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(token(false), 0..max_tokens).prop_map(|tokens| tokens.concat())
}

/// A stream and the lengths of the reads it arrives in.
pub fn chunked_stream(max_tokens: usize) -> impl Strategy<Value = (Vec<u8>, Vec<usize>)> {
    (stream(max_tokens), prop::collection::vec(1usize..48, 1..64))
}

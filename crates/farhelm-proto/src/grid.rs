//! Grid streams: how a terminal that runs in a terminal host reaches its
//! viewer, as a grid of styled runs rather than raw terminal bytes.
//!
//! NOTE: nothing sends these messages yet, and until the helm can relay them
//! it never asks for them (see "Attach and the stream kind"). This module is
//! the contract the supervisor's sender, its input encoder, the helm's relay
//! and the browser's grid client are built against.
//!
//! # Why this shape
//!
//! The supervisor owns the terminal emulator for host terminals, so what a
//! viewer needs is the emulator's screen, not the program's bytes. Three
//! choices follow from where the stream lives:
//!
//! - **Not a permanent format.** The helm and the supervisor speak one
//!   `PROTOCOL_VERSION` (an older helm sees a newer supervisor as Too new and
//!   must be upgraded), and the browser's JavaScript is served by the helm.
//!   So the grid stream changes with ordinary protocol bumps and has no
//!   version of its own. Contrast the terminal host's protocol and the
//!   snapshot format, which are permanent.
//! - **JSON messages.** The control channel is already JSON so a human can
//!   read a trace; the browser parses JSON without a hand-written binary
//!   decoder to get bounds wrong in; the prototype's renderer bench was driven
//!   by JSON row frames. Short keys keep it compact. If a measurement later
//!   shows the encoding is the bottleneck, a binary encoding is an ordinary
//!   protocol bump.
//! - **The helm relays without interpreting.** Grid messages travel on the
//!   attachment's data channel (the one `ControlMsg::Attach` named), exactly
//!   where terminal bytes travel for tmux terminals, and the helm relays each
//!   data frame as one WebSocket binary message in each direction, unchanged.
//!   All the helm learns is the client's `grid` capability on
//!   `ControlMsg::Attach` and the [`crate::StreamKind`] in
//!   `ControlMsg::Attached`.
//!
//! # Attach and the stream kind
//!
//! A client that can render grids says so with `Attach::grid`; the supervisor
//! answers with the stream kind in `Attached::stream`, `grid` only for an
//! attach that asked. A client that cannot render grids (every client before
//! protocol 68) attaching to a host terminal is refused with a clear error;
//! that refusal is permanent behavior, not a placeholder. `ReplayComplete`,
//! `Detached`, `Resize`, `PauseOutput`/`ResumeOutput` and the stall detach
//! keep their meaning for both stream kinds; for grid streams, pause means
//! "stop sending screen frames" and the supervisor coalesces (see "Flow
//! control"). The grid client itself sends no pause or resume: it applies
//! messages as they arrive and draws at most once per animation frame, so no
//! backlog builds up in the page.
//!
//! Until the helm can relay grid streams (a data-frame limit and a
//! byte-bounded queue fit for [`GRID_MAX_MESSAGE`] on grid channels, and each
//! browser input message forwarded as one frame), it sends `grid: false`
//! whatever the browser asked; the change that makes the helm a grid relay
//! passes the request through. The helm also refuses an `Attached` that names
//! `grid` for an attach that did not ask for it, rather than tell a browser
//! that cannot draw grids to expect one.
//!
//! The helm tells the browser the stream kind in a
//! `{"type":"stream","kind":"grid"|"bytes"}` text frame, so the client picks
//! its terminal component per attachment (xterm.js for `bytes`, the grid
//! client for `grid`). The ordering contract for the browser: on every
//! attachment, the stream notice comes before the first binary data frame
//! and before `replay_complete`; `pong` and `detached` may come before it and
//! settle nothing.
//!
//! # Framing
//!
//! Each data frame on a grid channel carries exactly one message: a UTF-8
//! JSON object whose `t` field names its type. Data frames never split or
//! join messages. A supervisor-to-client message ([`GridOutput`]) is at most
//! [`GRID_MAX_MESSAGE`] bytes of JSON; a client-to-supervisor message
//! ([`GridInput`]) is at most [`GRID_MAX_INPUT_MESSAGE`]. Senders split large
//! content (history, a screen of many long rows, a long clipboard write)
//! across messages as the message types allow.
//!
//! A receiver closes the attachment (supervisor) or reports a protocol error
//! and reconnects (client) on a message that is too large, is not a JSON
//! object, has an unknown `t`, or breaks a bound or an ordering rule.
//! Unknown fields are ignored, so an additive field within one protocol
//! version is harmless. Both sides enforce what they receive, and
//! [`GridOutput::encode`]/[`GridInput::encode`] refuse what the other side
//! would reject, so a sender bug surfaces at the sender.
//!
//! # Supervisor to client
//!
//! ## `hello`: first message of every attachment, exactly once
//!
//! `{"t":"hello","cols":C,"rows":R,"history_cap":N}`: the grid's size at
//! attach and the most history lines the client keeps (the supervisor's
//! scrollback cap; the client drops the oldest lines past it).
//!
//! ## Rows and runs, shared by screen and history
//!
//! A row is `{"w":bool,"r":[run...]}`: `w` is true when the row's text
//! continues on the next row (a soft wrap; selection joins wrapped rows
//! without a newline). A run is `[col, width, text, style]` or
//! `[col, width, text, style, link]` ([`Run`]):
//!
//! - `col` is the first column (0-based) and `width` the cells it covers
//!   (at least 1). Runs are in increasing column order, do not overlap, and
//!   end at or before the row's width: the committing frame's `cols` for a
//!   screen row, the message's own `cols` for a history row. Columns no run
//!   covers are blank in the default style.
//! - `text` is what the cells show, never empty, and at most
//!   [`GRID_MAX_CELL_TEXT_BYTES`] bytes per cell (a character and up to 16
//!   combining characters, each at most 4 bytes of UTF-8). A run whose text is
//!   all printable ASCII has `width` equal to the text's length. Any other run
//!   holds exactly one cell's content, the cell's character with its
//!   combining characters, at `width` 1, or 2 for a wide character; the client
//!   lays it out as one box of that width. The browser cannot reproduce the
//!   emulator's grouping of characters into cells (combining marks, joiners
//!   and variation selectors as the emulator counts them), so the format never
//!   asks it to. Spacer cells of wide characters are not sent.
//! - `style` is `[flags, fg, bg]` or `[flags, fg, bg, underline_color]`
//!   ([`Style`]); `flags` is the bit set in [`style_flags`]. A color
//!   ([`Color`]) is `-1` for the default, `0..=255` for a palette index the
//!   client maps through its theme (Ghostty's palette, `terminal-theme.js`),
//!   or `0x1000000 + 0xRRGGBB` for a direct color. When a program has
//!   redefined a palette entry (OSC 4), the sender sends the redefined color
//!   as a direct color instead of the index, so the client never needs the
//!   program's palette.
//! - `link` is an index into the message's `links` array, for a run inside an
//!   OSC 8 hyperlink.
//!
//! A message that carries rows may carry `"links":[[id, uri], ...]`
//! ([`Link`]), at most [`GRID_MAX_LINKS`] entries, `id` at most
//! [`GRID_MAX_LINK_ID_BYTES`] and `uri` at most [`GRID_MAX_LINK_URI_BYTES`]
//! (the emulator's own caps). `id` is the program's explicit OSC 8 id, or a
//! sender-chosen id for a link without one. Two runs belong to the same link
//! (for the mismatch warning, which compares across rows of one link) when
//! their entries have equal `id` and `uri`, including across messages. The
//! client treats `uri` as data and opens only what the existing link-scheme
//! rules allow (`terminal-links.js`). An absent `links` is an empty one.
//!
//! The sender must fit every message in [`GRID_MAX_MESSAGE`], and a row
//! cannot be split across messages. A row's runs alone always fit (a
//! 1000-column row of one-cell runs with maximal text is a few hundred KB of
//! JSON even with every quote escaped), but its links may not: a hostile
//! program can put more than [`GRID_MAX_LINKS`] distinct links on one row, or
//! fill 256 maximal links with characters JSON escapes. When a row cannot be
//! sent with all its links, the sender drops the `link` of as many of its
//! runs as it must and sends the text and styles unchanged.
//!
//! ## Screen updates: `rows` then `frame`
//!
//! A screen update is zero or more `rows` messages followed by one `frame`,
//! which commits it. The client applies nothing until the `frame` arrives, so
//! a frame is never half-drawn.
//!
//! - `{"t":"rows","rows":[[y, row], ...],"links":[...]}`: replace screen row
//!   `y` (0-based, below the committing frame's `rows`) with `row`. A later
//!   entry for the same `y` before the `frame` replaces an earlier one, so a
//!   client never needs to buffer more than one row per screen line.
//! - `{"t":"frame","rev":n,"cols":C,"rows":R,"shift":k,"cursor":{...}}`:
//!   commit. In order: if `cols`/`rows` differ from the client's, the client
//!   resizes its screen (rows it has no update for become blank); then, if
//!   `shift` (`0 <= k <= rows`, absent means 0) is set, it moves every screen
//!   row up by `k` (the top `k` rows leave the screen; they arrive as history
//!   before the frame, see "Ordering") and blanks the bottom `k`; then it
//!   applies the buffered `rows`; then it draws. `rev` grows by one or more
//!   per frame (screen frames may be skipped; see "Flow control"). `cursor`
//!   is `{"x":col,"y":row,"visible":bool,"shape":"block"|"underline"|"bar",
//!   "blink":bool}` and lies on the screen (`x < cols`, `y < rows`). The
//!   default cursor (one no program has styled) is sent with `blink: true`,
//!   because today's terminal blinks unless the program says otherwise
//!   (alacritty's own default is steady).
//!
//! A frame that changes `cols` or `rows` comes with `rows` for every non-blank
//! row, since the client blanks every row a resizing frame does not update.
//!
//! The sender diffs against what it last sent this client, so after the
//! first frame only changed rows are sent; `shift` lets ordinary scrolling
//! output send just the new rows.
//!
//! ## History
//!
//! The client's history is the lines above the screen, oldest first,
//! numbered by an absolute line number that only grows (the number of lines
//! that have ever scrolled into the emulator's history, from the vendored
//! counter).
//!
//! - `{"t":"history","first":L,"cols":C,"rows":[row, ...],"links":[...]}`:
//!   append rows as lines `L`, `L+1`, ..., written at width `C` (every run
//!   ends at or before `C`). The client keeps each history line's width, so
//!   joining soft-wrapped lines for selection keeps their trailing blanks
//!   even when the screen is now a different width. `first` and the last
//!   line's number are at most [`GRID_MAX_LINE_NUMBER`]; an empty `rows` is
//!   allowed for a `first` within that bound. The first `history` after a
//!   `hello` or `history_reset` may start at any `L`; every later one starts
//!   exactly where the previous one ended, and a discontinuity is a protocol
//!   error. The client keeps at most `history_cap` lines, dropping the
//!   oldest.
//! - `{"t":"history_reset"}`: the client discards its history and the next
//!   `history` messages rebuild it. Sent when history changed in a way an
//!   append cannot express: a resize that reflowed it, a program erasing
//!   scrollback (`CSI 3 J`), or the client falling so far behind that lines it
//!   never received were evicted.
//!
//! Rows that scroll off the screen always reach the client as `history` (or
//! after a `history_reset`), even while screen frames are being skipped:
//! history is never silently lost (SPEC.md: flow control never drops output).
//!
//! ## `clipboard`: live only
//!
//! `{"t":"clipboard","target":"clipboard"|"primary","text":s,"last":bool}`:
//! the program wrote to a clipboard (OSC 52). Sent only for live output,
//! never for replayed output, so a rebuild never repeats a clipboard write. A
//! write longer than one message is split into contiguous parts of at most
//! [`GRID_MAX_CLIPBOARD_PART_BYTES`] of text each (and within
//! [`GRID_MAX_MESSAGE`] once encoded), split only at character boundaries,
//! all naming the same target, the last with `last: true`. The whole write is
//! at most [`GRID_MAX_CLIPBOARD_BYTES`]; the sender drops a larger one. The
//! client writes the text to the clipboard when the last part arrives, as
//! today's terminal honors OSC 52 writes. Reads are never answered: there is
//! no clipboard-read message.
//!
//! The client honors only `target: "clipboard"`, as today's terminal honors
//! only OSC 52 writes to the clipboard selector: `primary` writes are not
//! copied to the system clipboard, because a program setting its selection
//! must not overwrite what the user copied. One accepted difference from
//! today: the emulator maps an empty selector (and a list starting with `c`)
//! to the clipboard, where today's terminal ignored them.
//!
//! ## `state`: on attach after `hello`, then whenever it changes
//!
//! `{"t":"state", ...}` with every field present ([`TerminalState`] has the
//! meaning of each): the mouse reporting mode, focus reporting, whether the
//! alternate screen shows, the redefined default colors, whether recovery
//! trimmed history, and the program's exit.
//!
//! ## Ordering
//!
//! A screen update (its `rows` and the `frame` that commits it) is
//! contiguous: no other message comes between them. The `history` lines a
//! frame's `shift` pushes off the screen arrive before that frame. `state`,
//! `history`, `history_reset` and `clipboard` come only between screen
//! updates, and the parts of one clipboard write are contiguous.
//!
//! ## Attach sequence
//!
//! `hello`, then `state` (nothing else may come between them; the client
//! needs the modes before it can act on a screen), then a full screen (`rows`
//! for every non-blank row, then `frame`), then the history as `history`
//! messages oldest first, then the existing `ReplayComplete` control
//! message, then live updates. The screen comes before the history so the
//! visible terminal is usable at once.
//!
//! # Client to supervisor
//!
//! One [`GridInput`] per data frame: keys the client does not handle itself,
//! typed and composed text, paste parts, mouse events in cell coordinates,
//! and focus changes. Resize stays the existing control message; the grid
//! client sends no pause or resume (see "Attach and the stream kind").
//!
//! A paste is split into parts of at most [`GRID_MAX_PASTE_PART_BYTES`] of
//! text that also stay within [`GRID_MAX_INPUT_MESSAGE`] once encoded (a part
//! of newlines doubles in size when escaped), and text input the same way.
//! Parts split only at character boundaries (a JavaScript string sliced
//! between the halves of a surrogate pair encodes as a lone surrogate, which
//! the supervisor refuses) and never between a CR and the LF after it, so
//! line-ending conversion sees the same text it would see whole. The parts of
//! one paste are contiguous: no other input message comes between them. If
//! the attachment ends in the middle of a paste, the supervisor writes the
//! closing bracketed-paste marker, so the program is not left in paste mode.
//! The supervisor's input encoder enforces the paste rules; a single message
//! cannot show them.
//!
//! # Flow control
//!
//! The supervisor's per-attachment sender coalesces whenever it cannot
//! proceed (the shared writer queue is full, or the client paused): it stops
//! sending screen updates and, when it can send again, sends one update from
//! what it last sent to the current screen (any number of revisions skipped).
//! History is different: lines that entered history while the client was
//! behind are sent as `history` appends; if more lines arrived than the
//! history cap holds (some were evicted before they were sent), or history
//! changed shape, it sends `history_reset` and the current history instead.
//! Clipboard writes are bounded too: while behind, the sender keeps only the
//! latest unsent write per target (a newer write replaces an older one not
//! yet started), and always finishes a write whose first part has gone out.
//! The stall timeout and stall detach stay as they are.
//!
//! # What this module checks
//!
//! [`GridOutput::decode`] and [`GridInput::decode`] check everything one
//! message can show on its own: size, JSON shape, the `t` tag, and every
//! bound in the table below that does not depend on earlier messages. The
//! rules that do depend on them belong to the receiver's view of the stream,
//! and [`GridStreamCheck`] applies them in Rust: `hello` first and once,
//! `state` right after it, a screen row within its committing frame's
//! height and width, `rev` growing, history continuity, the ordering rules
//! above, and a clipboard write's parts and total. One ordering rule is
//! beyond any receiver: that the history a frame's shift pushes off arrives
//! before that frame, since telling which lines a shift pushed off needs the
//! screen's contents. The sender's own tests carry that one.
//!
//! | Bound | Value |
//! | --- | --- |
//! | supervisor-to-client message | [`GRID_MAX_MESSAGE`], 1 MiB of JSON |
//! | client-to-supervisor message | [`GRID_MAX_INPUT_MESSAGE`], 64 KiB of JSON |
//! | `cols`, `rows` (also `history` `cols`) | `1..=`[`GRID_MAX_DIMENSION`] (1000) |
//! | `history_cap` | at most [`GRID_MAX_HISTORY_CAP`] (100000) |
//! | links per message | [`GRID_MAX_LINKS`] (256) |
//! | link id / uri | 250 / 2083 bytes |
//! | run end (`col + width`) | the row's width (see "Rows and runs") |
//! | run text | [`GRID_MAX_CELL_TEXT_BYTES`] (68) bytes per cell |
//! | non-ASCII run width | 1 or 2 |
//! | clipboard part / whole write | 512 KiB / 8 MiB of text |
//! | text input / paste part | 16 KiB / 32 KiB |
//! | `key` / `code` | 64 bytes each |
//! | `rev`, history line numbers | at most [`GRID_MAX_LINE_NUMBER`] (2^53 - 1) |
//!
//! # Golden fixtures
//!
//! `crates/farhelm-proto/grid-fixtures/` holds one JSON file per message, for
//! this module's tests and for the browser grid client's JS unit tests (which
//! the grid client's own change adds). `output/` is supervisor to client and
//! `input/` client to supervisor; under each, `valid/` fixtures decode and
//! encode back to the same JSON, `tolerated/` fixtures decode (unknown
//! fields, explicitly empty optional fields) but need not encode back to
//! themselves, and `invalid/` fixtures must be rejected, each for the one
//! reason its name gives. Size limits and the rules spanning messages have no
//! fixtures; the tests construct those.

use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// ---- Bounds -----------------------------------------------------------------

/// Most bytes of JSON in one supervisor-to-client grid message.
///
/// Larger than [`crate::MAX_TERMINAL_DATA_LEN`] because a row cannot be split
/// across messages and one 1000-column row with its links can exceed 64 KiB.
/// NOTE: the helm still applies `MAX_TERMINAL_DATA_LEN` to every incoming
/// terminal data frame, which is one reason it does not ask for grid streams
/// yet (module docs, "Attach and the stream kind").
pub const GRID_MAX_MESSAGE: usize = 1024 * 1024;

/// Most bytes of JSON in one client-to-supervisor grid message: the same
/// bound every terminal data frame already has.
///
/// NOTE: the helm cuts browser input into 32 KiB data frames today
/// (`INPUT_CHUNK` in its client), which would split a larger grid message
/// across frames; that is the other reason it does not ask for grid streams
/// yet.
pub const GRID_MAX_INPUT_MESSAGE: usize = crate::MAX_TERMINAL_DATA_LEN;

/// Largest `cols` or `rows` a grid has; the smallest is 1. Also bounds every
/// cell coordinate (`y`, `col + width`, mouse `x` and `y`) that cannot be
/// checked against the stream's current size.
pub const GRID_MAX_DIMENSION: u16 = 1000;

/// Largest `history_cap` a `hello` may announce.
pub const GRID_MAX_HISTORY_CAP: u32 = 100_000;

/// Most entries in one message's `links`.
pub const GRID_MAX_LINKS: usize = 256;

/// Longest OSC 8 `id`, in bytes: the vendored emulator's
/// `MAX_HYPERLINK_ID_BYTES`.
pub const GRID_MAX_LINK_ID_BYTES: usize = 250;

/// Longest OSC 8 URI, in bytes: the vendored emulator's
/// `MAX_HYPERLINK_URI_BYTES` (2083, the limit VTE and iTerm2 use).
pub const GRID_MAX_LINK_URI_BYTES: usize = 2083;

/// Most bytes of run text per cell the run covers: one character and up to
/// 16 combining characters (the vendored emulator's `MAX_ZEROWIDTH_CHARS`),
/// each at most 4 bytes of UTF-8.
pub const GRID_MAX_CELL_TEXT_BYTES: usize = 4 * (1 + 16);

/// Most bytes of text in one `text` input message.
pub const GRID_MAX_TEXT_INPUT_BYTES: usize = 16 * 1024;

/// Most bytes of text in one `paste` part.
pub const GRID_MAX_PASTE_PART_BYTES: usize = 32 * 1024;

/// Most bytes of text in one `clipboard` part. Half of [`GRID_MAX_MESSAGE`],
/// so that a part of ordinary text fits once encoded; a part whose text JSON
/// escapes heavily has to be smaller still.
pub const GRID_MAX_CLIPBOARD_PART_BYTES: usize = 512 * 1024;

/// Most bytes of text in one whole clipboard write, across its parts. The
/// sender drops a larger write rather than send part of it.
pub const GRID_MAX_CLIPBOARD_BYTES: usize = 8 * 1024 * 1024;

/// Longest `key` or `code` in a `key` input message, in bytes. The browser's
/// values are short names (`"ArrowUp"`, `"KeyA"`) or one character.
pub const GRID_MAX_KEY_BYTES: usize = 64;

/// Largest `rev` and absolute history line number: JavaScript's largest
/// exactly representable integer (`Number.MAX_SAFE_INTEGER`).
///
/// The client compares these for continuity and ordering, and a larger value
/// would silently round in the browser, so a gap or a repeat could go
/// unnoticed. At a million lines a second the counter takes centuries to get
/// here.
pub const GRID_MAX_LINE_NUMBER: u64 = (1 << 53) - 1;

/// The `flags` bits of a [`Style`].
///
/// Bits above [`style_flags::BLINK`] are reserved: senders never set them and
/// receivers ignore them, the same tolerance unknown fields get.
pub mod style_flags {
    pub const BOLD: u16 = 1;
    pub const DIM: u16 = 2;
    pub const ITALIC: u16 = 4;
    pub const UNDERLINE: u16 = 8;
    pub const DOUBLE_UNDERLINE: u16 = 16;
    pub const UNDERCURL: u16 = 32;
    pub const DOTTED_UNDERLINE: u16 = 64;
    pub const DASHED_UNDERLINE: u16 = 128;
    pub const INVERSE: u16 = 256;
    pub const HIDDEN: u16 = 512;
    pub const STRIKETHROUGH: u16 = 1024;
    pub const BLINK: u16 = 2048;
}

/// The `mods` bits of `key` and `mouse` input messages. Any other bit is a
/// bound violation.
pub mod modifiers {
    pub const SHIFT: u8 = 1;
    /// Alt, or Option on a Mac.
    pub const ALT: u8 = 2;
    pub const CTRL: u8 = 4;
    /// Meta, or Cmd on a Mac.
    pub const META: u8 = 8;
    /// Every defined bit.
    pub const ALL: u8 = SHIFT | ALT | CTRL | META;
}

// ---- Errors -----------------------------------------------------------------

/// Why a grid message was refused, on either side.
///
/// The variants say which kind of problem it was; nothing decides behavior
/// from the text. Every one of them means the same thing to a receiver: the
/// peer broke the format, so the attachment ends.
#[derive(Debug, thiserror::Error)]
pub enum GridError {
    /// The message is larger than its direction allows.
    #[error("grid message is {len} bytes, over the {max}-byte limit")]
    TooLarge { len: usize, max: usize },
    /// Not JSON, an unknown `t`, a missing or mistyped field, or a value no
    /// type here can hold (a color out of range, say).
    #[error("grid message is malformed: {0}")]
    Json(#[from] serde_json::Error),
    /// Well-formed, but a bound or a rule of the format is broken.
    #[error("grid message breaks a bound: {0}")]
    Bound(String),
}

/// Shorthand for a [`GridError::Bound`] with a formatted message.
macro_rules! bound {
    ($($arg:tt)*) => {
        GridError::Bound(format!($($arg)*))
    };
}

/// Refuse a message whose JSON is larger than `max`, before or after parsing.
fn check_len(len: usize, max: usize) -> Result<(), GridError> {
    if len > max {
        return Err(GridError::TooLarge { len, max });
    }
    Ok(())
}

/// Refuse a message that is not a JSON object, before parsing it.
///
/// serde reads an internally tagged enum from an array too (the tag first,
/// then the fields in order), so `["hello",80,24,0]` would otherwise decode as
/// a `hello` the browser's client refuses. Checked on the bytes, because
/// the tagged enum's own decoder cannot be told to want an object; the
/// structs inside a message use [`object_only`] instead.
fn check_object(bytes: &[u8]) -> Result<(), GridError> {
    match bytes
        .iter()
        .find(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
    {
        Some(b'{') => Ok(()),
        _ => Err(bound!("the message is not a JSON object")),
    }
}

// ---- Colors and styles ------------------------------------------------------

/// The wire value a direct color starts at: `0x1000000 + 0xRRGGBB`.
const DIRECT_COLOR_BASE: i64 = 0x100_0000;

/// One color of a [`Style`] or of [`TerminalState`], as the integer the wire
/// carries: `-1`, a palette index, or `0x1000000 + 0xRRGGBB`.
///
/// Typed rather than a bare integer so a renderer cannot hand an unchecked
/// number to CSS; any other integer fails to decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// The default foreground, background or underline color, whichever the
    /// position means.
    Default,
    /// A palette index the client maps through its theme.
    Palette(u8),
    /// A direct color. Also how a palette entry the program redefined (OSC 4)
    /// travels.
    Rgb { r: u8, g: u8, b: u8 },
}

impl Color {
    /// The integer this color is on the wire.
    pub fn to_wire(self) -> i64 {
        match self {
            Color::Default => -1,
            Color::Palette(index) => i64::from(index),
            Color::Rgb { r, g, b } => {
                DIRECT_COLOR_BASE + (i64::from(r) << 16 | i64::from(g) << 8 | i64::from(b))
            }
        }
    }

    /// The color a wire integer means, or `None` for an integer that is no
    /// color (below -1, between 256 and `0x1000000`, or past `0x1FFFFFF`).
    pub fn from_wire(value: i64) -> Option<Color> {
        match value {
            -1 => Some(Color::Default),
            0..=255 => Some(Color::Palette(value as u8)),
            _ if (DIRECT_COLOR_BASE..DIRECT_COLOR_BASE + 0x100_0000).contains(&value) => {
                let rgb = value - DIRECT_COLOR_BASE;
                Some(Color::Rgb {
                    r: (rgb >> 16) as u8,
                    g: (rgb >> 8) as u8,
                    b: rgb as u8,
                })
            }
            _ => None,
        }
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(self.to_wire())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = i64::deserialize(deserializer)?;
        Color::from_wire(value)
            .ok_or_else(|| de::Error::custom(format!("{value} is not a grid color")))
    }
}

/// How a run's cells are drawn: `[flags, fg, bg]` or
/// `[flags, fg, bg, underline_color]` on the wire.
///
/// `underline_color` keeps whether it was sent: an absent one and an explicit
/// `-1` both mean the default underline color, but each encodes back to what
/// arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// The [`style_flags`] bit set.
    pub flags: u16,
    pub fg: Color,
    pub bg: Color,
    pub underline_color: Option<Color>,
}

impl Style {
    /// Default colors and no attributes: what a blank cell looks like.
    pub const PLAIN: Style = Style {
        flags: 0,
        fg: Color::Default,
        bg: Color::Default,
        underline_color: None,
    };
}

impl Serialize for Style {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let len = 3 + usize::from(self.underline_color.is_some());
        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&self.flags)?;
        seq.serialize_element(&self.fg)?;
        seq.serialize_element(&self.bg)?;
        if let Some(color) = &self.underline_color {
            seq.serialize_element(color)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Style {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StyleVisitor;
        impl<'de> Visitor<'de> for StyleVisitor {
            type Value = Style;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a style array [flags, fg, bg] or [flags, fg, bg, underline_color]")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Style, A::Error> {
                let flags = required(&mut seq, 0, &self)?;
                let fg = required(&mut seq, 1, &self)?;
                let bg = required(&mut seq, 2, &self)?;
                let underline_color = seq.next_element()?;
                no_more(&mut seq, 4, &self)?;
                Ok(Style {
                    flags,
                    fg,
                    bg,
                    underline_color,
                })
            }
        }
        deserializer.deserialize_seq(StyleVisitor)
    }
}

/// The next element of a fixed-shape array, which must be there.
fn required<'de, T: Deserialize<'de>, A: SeqAccess<'de>>(
    seq: &mut A,
    index: usize,
    expected: &dyn de::Expected,
) -> Result<T, A::Error> {
    seq.next_element()?
        .ok_or_else(|| de::Error::invalid_length(index, expected))
}

/// Refuse elements past an array's longest shape.
///
/// Arrays are positional, so an extra element is not an unknown field that
/// can be ignored: it would be a shape this format does not have.
fn no_more<'de, A: SeqAccess<'de>>(
    seq: &mut A,
    max: usize,
    expected: &dyn de::Expected,
) -> Result<(), A::Error> {
    if seq.next_element::<IgnoredAny>()?.is_some() {
        return Err(de::Error::invalid_length(max + 1, expected));
    }
    Ok(())
}

// ---- Rows, runs and links ---------------------------------------------------

/// One run of cells sharing a style: `[col, width, text, style]` or
/// `[col, width, text, style, link]` on the wire. See the module docs, "Rows
/// and runs", for what each part means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub col: u16,
    pub width: u16,
    pub text: String,
    pub style: Style,
    /// Index into the carrying message's `links`.
    pub link: Option<u16>,
}

impl Serialize for Run {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let len = 4 + usize::from(self.link.is_some());
        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&self.col)?;
        seq.serialize_element(&self.width)?;
        seq.serialize_element(&self.text)?;
        seq.serialize_element(&self.style)?;
        if let Some(link) = &self.link {
            seq.serialize_element(link)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Run {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RunVisitor;
        impl<'de> Visitor<'de> for RunVisitor {
            type Value = Run;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str(
                    "a run array [col, width, text, style] or [col, width, text, style, link]",
                )
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Run, A::Error> {
                let col = required(&mut seq, 0, &self)?;
                let width = required(&mut seq, 1, &self)?;
                let text = required(&mut seq, 2, &self)?;
                let style = required(&mut seq, 3, &self)?;
                let link = seq.next_element()?;
                no_more(&mut seq, 5, &self)?;
                Ok(Run {
                    col,
                    width,
                    text,
                    style,
                    link,
                })
            }
        }
        deserializer.deserialize_seq(RunVisitor)
    }
}

impl Run {
    /// Check this run on its own: a width of at least 1, an end within
    /// `row_width`, text within its byte budget and not empty, the width rule
    /// for ASCII and non-ASCII text, and a link index inside `links_len`.
    fn validate(&self, row_width: u16, links_len: usize) -> Result<(), GridError> {
        if self.width == 0 {
            return Err(bound!("run at column {} has width 0", self.col));
        }
        if self.end() > u32::from(row_width) {
            return Err(bound!(
                "run at column {} with width {} ends past column {row_width}",
                self.col,
                self.width
            ));
        }
        if self.text.is_empty() {
            return Err(bound!("run at column {} has no text", self.col));
        }
        let budget = GRID_MAX_CELL_TEXT_BYTES * usize::from(self.width);
        if self.text.len() > budget {
            return Err(bound!(
                "run at column {} has {} bytes of text for {} cells, over {budget}",
                self.col,
                self.text.len(),
                self.width
            ));
        }
        // Plain ASCII is the one text whose cells a client can count by
        // itself; anything else is a single cell's content, so the client
        // never has to guess how the emulator grouped characters into cells.
        let plain_ascii = self.text.bytes().all(|b| (0x20..0x7f).contains(&b));
        if plain_ascii && self.text.len() != usize::from(self.width) {
            return Err(bound!(
                "ASCII run at column {} has {} characters but width {}",
                self.col,
                self.text.len(),
                self.width
            ));
        }
        if !plain_ascii && self.width > 2 {
            return Err(bound!(
                "non-ASCII run at column {} has width {}, but holds one cell of width 1 or 2",
                self.col,
                self.width
            ));
        }
        if let Some(link) = self.link
            && usize::from(link) >= links_len
        {
            return Err(bound!(
                "run at column {} names link {link}, but the message has {links_len}",
                self.col
            ));
        }
        Ok(())
    }

    /// The column after this run's last cell.
    pub fn end(&self) -> u32 {
        u32::from(self.col) + u32::from(self.width)
    }
}

/// One row of cells: `{"w":bool,"r":[run...]}` on the wire, and only as
/// an object (see `object_only`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    /// The row's text continues on the next row (a soft wrap).
    #[serde(rename = "w")]
    pub wraps: bool,
    #[serde(rename = "r")]
    pub runs: Vec<Run>,
}

impl<'de> Deserialize<'de> for Row {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Fields {
            #[serde(rename = "w")]
            wraps: bool,
            #[serde(rename = "r")]
            runs: Vec<Run>,
        }
        let Fields { wraps, runs } = object_only(deserializer)?;
        Ok(Row { wraps, runs })
    }
}

/// Decode `T` from a JSON object and nothing else.
///
/// A derived struct decoder also takes an array of its fields in order, so
/// without this `[false,[]]` would decode as a row and
/// `[0,0,true,"block",true]` as a cursor, where the browser's client, which
/// checks for objects, refuses them. Every struct the format writes as an
/// object decodes through here (the outermost message is checked on its
/// bytes, by [`check_object`]).
fn object_only<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    struct ObjectOnly<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectOnly<T> {
        type Value = T;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a JSON object")
        }
        fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
            T::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_map(ObjectOnly(std::marker::PhantomData))
}

impl Row {
    /// Check every run against `row_width`, and that runs are in increasing
    /// column order without overlapping.
    fn validate(&self, row_width: u16, links_len: usize) -> Result<(), GridError> {
        let mut next_free = 0u32;
        for run in &self.runs {
            if u32::from(run.col) < next_free {
                return Err(bound!(
                    "run at column {} overlaps or precedes the run before it",
                    run.col
                ));
            }
            run.validate(row_width, links_len)?;
            next_free = run.end();
        }
        Ok(())
    }

    /// The column after this row's last run, 0 for a blank row.
    pub fn end(&self) -> u32 {
        self.runs.last().map_or(0, Run::end)
    }
}

/// A screen row and where it goes: `[y, row]` on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "(u16, Row)", into = "(u16, Row)")]
pub struct ScreenRow {
    /// The screen row this replaces, 0-based from the top.
    pub y: u16,
    pub row: Row,
}

impl From<(u16, Row)> for ScreenRow {
    fn from((y, row): (u16, Row)) -> Self {
        ScreenRow { y, row }
    }
}

impl From<ScreenRow> for (u16, Row) {
    fn from(screen_row: ScreenRow) -> Self {
        (screen_row.y, screen_row.row)
    }
}

/// One OSC 8 hyperlink a run can point at: `[id, uri]` on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "(String, String)", into = "(String, String)")]
pub struct Link {
    /// The program's explicit OSC 8 id, or one the sender chose for a link
    /// without one. Runs whose links have equal `id` and `uri` are one link.
    pub id: String,
    /// The target, as the program wrote it. Data, never trusted: the client
    /// opens only what its link-scheme rules allow.
    pub uri: String,
}

impl From<(String, String)> for Link {
    fn from((id, uri): (String, String)) -> Self {
        Link { id, uri }
    }
}

impl From<Link> for (String, String) {
    fn from(link: Link) -> Self {
        (link.id, link.uri)
    }
}

/// Check a message's `links`: how many, and each entry's lengths.
fn validate_links(links: &[Link]) -> Result<(), GridError> {
    if links.len() > GRID_MAX_LINKS {
        return Err(bound!(
            "{} links in one message, over {GRID_MAX_LINKS}",
            links.len()
        ));
    }
    for link in links {
        if link.id.len() > GRID_MAX_LINK_ID_BYTES {
            return Err(bound!(
                "link id of {} bytes, over {GRID_MAX_LINK_ID_BYTES}",
                link.id.len()
            ));
        }
        if link.uri.len() > GRID_MAX_LINK_URI_BYTES {
            return Err(bound!(
                "link URI of {} bytes, over {GRID_MAX_LINK_URI_BYTES}",
                link.uri.len()
            ));
        }
    }
    Ok(())
}

/// Check a `cols`/`rows` pair against `1..=GRID_MAX_DIMENSION`.
fn validate_size(what: &str, cols: u16, rows: u16) -> Result<(), GridError> {
    for (name, value) in [("cols", cols), ("rows", rows)] {
        if !(1..=GRID_MAX_DIMENSION).contains(&value) {
            return Err(bound!(
                "{what} {name} is {value}, outside 1..={GRID_MAX_DIMENSION}"
            ));
        }
    }
    Ok(())
}

// ---- Supervisor to client ---------------------------------------------------

/// Where the cursor is and how it looks, carried by every `frame`, as an
/// object only (see `object_only`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Cursor {
    /// Column, 0-based, below the frame's `cols`.
    pub x: u16,
    /// Row, 0-based, below the frame's `rows`.
    pub y: u16,
    pub visible: bool,
    pub shape: CursorShape,
    pub blink: bool,
}

impl<'de> Deserialize<'de> for Cursor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Fields {
            x: u16,
            y: u16,
            visible: bool,
            shape: CursorShape,
            blink: bool,
        }
        let Fields {
            x,
            y,
            visible,
            shape,
            blink,
        } = object_only(deserializer)?;
        Ok(Cursor {
            x,
            y,
            visible,
            shape,
            blink,
        })
    }
}

/// The cursor's shape, as the program set it (DECSCUSR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorShape {
    Block,
    Underline,
    Bar,
}

/// Which mouse events the program asked for, and so which ones the client
/// sends.
///
/// Also decides when a plain drag goes to the program instead of making a
/// local selection: under anything but `Off`, only Shift-drag (or Option-drag
/// on a Mac) selects locally. One exception to "sends only what the program
/// asked for": while [`TerminalState::alt_screen`] is true and this is `Off`,
/// the client still sends `wheel` messages, which the supervisor turns into
/// arrow keys as today's terminal does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseMode {
    /// No mouse reporting.
    Off,
    /// Presses and releases (modes 9 and 1000).
    Click,
    /// Also motion while a button is held (mode 1002).
    Drag,
    /// All motion (mode 1003).
    Motion,
}

/// How the program ended, as the client shows it in place of tmux's "Pane is
/// dead" banner: `{"code":n}` or `{"signal":n}` on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitState {
    /// It exited with this status.
    Code(i32),
    /// This signal ended it.
    Signal(i32),
}

/// The terminal's modes and status that are not cells: sent after `hello`
/// and again whenever any of it changes, always whole.
///
/// Every field must be present when decoded, `null` ones included: the
/// message is small, and a sender that left one out is broken rather than
/// saying "unchanged".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalState {
    pub mouse: MouseMode,
    /// The program asked for focus in/out reports (mode 1004); the client
    /// sends `focus` input only while this is true.
    pub focus: bool,
    /// The alternate screen shows. The client shows no history while it
    /// does: scrolling a full-screen program's view is the program's
    /// business, so wheel steps go to the program, as mouse reports when
    /// mouse reporting is on and as arrow keys (made by the supervisor)
    /// when it is off.
    pub alt_screen: bool,
    /// `None` for the theme's foreground, or the direct color a program set
    /// with OSC 10. Never a palette index or `-1`.
    #[serde(deserialize_with = "present")]
    pub default_fg: Option<Color>,
    /// `None` for the theme's background, or the direct color a program set
    /// with OSC 11. Never a palette index or `-1`.
    #[serde(deserialize_with = "present")]
    pub default_bg: Option<Color>,
    /// Recovery lost some history (a journal gap); the client says that
    /// history was trimmed.
    pub history_trimmed: bool,
    /// `None` while the program runs.
    #[serde(deserialize_with = "present")]
    pub exit: Option<ExitState>,
}

/// Decode an `Option` field that must be present, `null` or not.
///
/// serde reads a missing `Option` field as `None` unless the field has a
/// `deserialize_with`, so naming this one is what makes absence an error.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(deserializer)
}

/// Which clipboard an OSC 52 write names. The emulator maps OSC 52's `c` to
/// the clipboard and `p` and `s` to the primary selection, and drops any
/// other name before it gets here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardTarget {
    Clipboard,
    Primary,
}

/// A supervisor-to-client grid message. See the module docs for each type's
/// meaning and the order they arrive in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum GridOutput {
    /// The first message of every attachment, exactly once.
    Hello {
        cols: u16,
        rows: u16,
        history_cap: u32,
    },
    /// Screen rows the next `frame` commits.
    Rows {
        rows: Vec<ScreenRow>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        links: Vec<Link>,
    },
    /// Commits the screen update the `rows` before it started.
    Frame {
        rev: u64,
        cols: u16,
        rows: u16,
        /// Rows the screen moves up before the buffered rows apply.
        #[serde(default, skip_serializing_if = "is_zero")]
        shift: u16,
        cursor: Cursor,
    },
    /// History lines `first`, `first + 1`, ... appended oldest first, written
    /// at width `cols`.
    History {
        first: u64,
        cols: u16,
        rows: Vec<Row>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        links: Vec<Link>,
    },
    /// The client discards its history; `history` messages rebuild it.
    HistoryReset,
    /// The terminal's modes and status, whole.
    State(TerminalState),
    /// One part of a clipboard write (OSC 52), live output only.
    Clipboard {
        target: ClipboardTarget,
        text: String,
        /// This part completes the write; the client copies on it.
        last: bool,
    },
}

/// `skip_serializing_if` for `shift`, which is absent when 0.
fn is_zero(value: &u16) -> bool {
    *value == 0
}

impl GridOutput {
    /// Decode one data frame's payload, refusing anything a client must
    /// refuse on its own: too large, malformed, an unknown `t`, or a bound
    /// one message can show it breaks.
    pub fn decode(bytes: &[u8]) -> Result<GridOutput, GridError> {
        check_len(bytes.len(), GRID_MAX_MESSAGE)?;
        check_object(bytes)?;
        let message: GridOutput = serde_json::from_slice(bytes)?;
        message.validate()?;
        Ok(message)
    }

    /// Encode as one data frame's payload, refusing a message the client
    /// would reject, so a sender bug fails here rather than as a client's
    /// protocol error.
    pub fn encode(&self) -> Result<Vec<u8>, GridError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)?;
        check_len(bytes.len(), GRID_MAX_MESSAGE)?;
        Ok(bytes)
    }

    /// Check every bound this message can break by itself. Size is not one
    /// of them (it is a property of the encoding); see [`Self::decode`].
    pub fn validate(&self) -> Result<(), GridError> {
        match self {
            GridOutput::Hello {
                cols,
                rows,
                history_cap,
            } => {
                validate_size("hello", *cols, *rows)?;
                if *history_cap > GRID_MAX_HISTORY_CAP {
                    return Err(bound!(
                        "history_cap {history_cap} is over {GRID_MAX_HISTORY_CAP}"
                    ));
                }
            }
            GridOutput::Rows { rows, links } => {
                validate_links(links)?;
                for screen_row in rows {
                    if screen_row.y >= GRID_MAX_DIMENSION {
                        return Err(bound!(
                            "screen row {} is past the largest grid",
                            screen_row.y
                        ));
                    }
                    // The committing frame's width is checked by the
                    // stream check; alone, a screen row is bounded only by
                    // the largest grid.
                    screen_row.row.validate(GRID_MAX_DIMENSION, links.len())?;
                }
            }
            GridOutput::Frame {
                rev,
                cols,
                rows,
                shift,
                cursor,
            } => {
                validate_size("frame", *cols, *rows)?;
                if *rev > GRID_MAX_LINE_NUMBER {
                    return Err(bound!("frame rev {rev} is over {GRID_MAX_LINE_NUMBER}"));
                }
                if shift > rows {
                    return Err(bound!("frame shift {shift} is more than its {rows} rows"));
                }
                if cursor.x >= *cols || cursor.y >= *rows {
                    return Err(bound!(
                        "cursor at ({}, {}) is outside a {cols}x{rows} screen",
                        cursor.x,
                        cursor.y
                    ));
                }
            }
            GridOutput::History {
                first,
                cols,
                rows,
                links,
            } => {
                if !(1..=GRID_MAX_DIMENSION).contains(cols) {
                    return Err(bound!(
                        "history cols is {cols}, outside 1..={GRID_MAX_DIMENSION}"
                    ));
                }
                validate_links(links)?;
                // `first` on its own, which also covers an empty append, and
                // then the last line, `first + len - 1`, written as an
                // addition so it cannot overflow.
                if *first > GRID_MAX_LINE_NUMBER {
                    return Err(bound!(
                        "history first line {first} is over {GRID_MAX_LINE_NUMBER}"
                    ));
                }
                let len = rows.len() as u64;
                if first + len > GRID_MAX_LINE_NUMBER + 1 {
                    return Err(bound!(
                        "history lines {first}.. ({len} of them) run past line \
                         {GRID_MAX_LINE_NUMBER}"
                    ));
                }
                for row in rows {
                    row.validate(*cols, links.len())?;
                }
            }
            GridOutput::HistoryReset => {}
            GridOutput::Clipboard { text, .. } => {
                if text.len() > GRID_MAX_CLIPBOARD_PART_BYTES {
                    return Err(bound!(
                        "clipboard part of {} bytes, over {GRID_MAX_CLIPBOARD_PART_BYTES}",
                        text.len()
                    ));
                }
            }
            GridOutput::State(state) => {
                for (name, color) in [
                    ("default_fg", state.default_fg),
                    ("default_bg", state.default_bg),
                ] {
                    if let Some(color) = color
                        && !matches!(color, Color::Rgb { .. })
                    {
                        return Err(bound!(
                            "state {name} is {}, not a direct color",
                            color.to_wire()
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

// ---- The stream as a whole --------------------------------------------------

/// The rules of a supervisor-to-client stream that span messages, applied to
/// messages one at a time in the order they arrive.
///
/// [`GridOutput::decode`] checks each message alone; this checks what only
/// the sequence shows: `hello` first and only once, `state` right after it,
/// a `frame`'s buffered `rows` within its height and width, `rev` growing,
/// history appends continuing where the last one ended, screen updates and
/// clipboard writes arriving whole with nothing else inside them, and a
/// clipboard write's parts naming one target and staying within
/// [`GRID_MAX_CLIPBOARD_BYTES`].
/// It is what a Rust receiver of a grid stream runs (the browser's grid
/// client applies the same rules in JavaScript), and what sender tests use to
/// show a stream they produced is one a client accepts. The one ordering rule
/// it cannot see is named in the module docs, "What this module checks".
///
/// It keeps no cell contents, only the few numbers the rules need, plus the
/// heights and right edges of the rows waiting for their `frame`. A refused
/// message leaves it unchanged.
#[derive(Debug, Default)]
pub struct GridStreamCheck {
    /// Set once `hello` has arrived.
    started: bool,
    /// Set once the `state` that must follow `hello` has arrived.
    has_state: bool,
    /// The last committed frame's `rev`, if any frame has committed.
    last_rev: Option<u64>,
    /// A `rows` message has opened a screen update that no `frame` has
    /// committed yet. Only `rows` and `frame` may come while it is open.
    update_open: bool,
    /// The right edge of each screen row buffered in the open update, by
    /// row. A later `rows` entry for the same row replaces the earlier one,
    /// as it does on the screen, so this never holds more than
    /// [`GRID_MAX_DIMENSION`] entries however many `rows` messages arrive
    /// before a `frame`. A client buffering rows can bound itself the same
    /// way.
    pending: std::collections::BTreeMap<u16, u32>,
    /// The line number the next `history` must start at, or `None` right
    /// after `hello` or `history_reset`, when any start is allowed.
    next_history_line: Option<u64>,
    /// The clipboard write whose last part has not arrived: its target and
    /// the bytes of text so far. Only its next part may come while it is
    /// open.
    clipboard: Option<(ClipboardTarget, usize)>,
}

impl GridStreamCheck {
    pub fn new() -> Self {
        Self::default()
    }

    /// Accept the next message of the stream, or say which cross-message rule
    /// it breaks. A refused message leaves the check unchanged, though a
    /// receiver ends the attachment on any refusal anyway.
    pub fn accept(&mut self, message: &GridOutput) -> Result<(), GridError> {
        let name = Self::name(message);
        if let GridOutput::Hello { .. } = message {
            if self.started {
                return Err(bound!("a second hello on one attachment"));
            }
            self.started = true;
            return Ok(());
        }
        if !self.started {
            return Err(bound!("{name} before hello"));
        }
        if !self.has_state {
            let GridOutput::State(_) = message else {
                return Err(bound!("{name} between hello and the first state"));
            };
            self.has_state = true;
            return Ok(());
        }
        if self.clipboard.is_some() && !matches!(message, GridOutput::Clipboard { .. }) {
            return Err(bound!("{name} inside a clipboard write"));
        }
        if self.update_open
            && !matches!(message, GridOutput::Rows { .. } | GridOutput::Frame { .. })
        {
            return Err(bound!("{name} inside a screen update"));
        }
        match message {
            GridOutput::Hello { .. } => unreachable!("handled above"),
            GridOutput::Rows { rows, .. } => {
                self.update_open = true;
                self.pending.extend(rows.iter().map(|r| (r.y, r.row.end())));
            }
            GridOutput::Frame {
                rev, cols, rows, ..
            } => {
                if let Some(last) = self.last_rev
                    && *rev <= last
                {
                    return Err(bound!("frame rev {rev} does not follow rev {last}"));
                }
                for (y, end) in &self.pending {
                    if y >= rows {
                        return Err(bound!("screen row {y} committed by a frame of {rows} rows"));
                    }
                    if *end > u32::from(*cols) {
                        return Err(bound!(
                            "screen row {y} ends at column {end}, past the frame's {cols} cols"
                        ));
                    }
                }
                self.pending.clear();
                self.update_open = false;
                self.last_rev = Some(*rev);
            }
            GridOutput::History { first, rows, .. } => {
                if let Some(expected) = self.next_history_line
                    && *first != expected
                {
                    return Err(bound!(
                        "history starts at line {first}, but the last append ended before \
                         line {expected}"
                    ));
                }
                self.next_history_line = Some(first.saturating_add(rows.len() as u64));
            }
            GridOutput::HistoryReset => self.next_history_line = None,
            GridOutput::State(_) => {}
            GridOutput::Clipboard { target, text, last } => {
                let so_far = match self.clipboard {
                    Some((open, _)) if open != *target => {
                        return Err(bound!(
                            "a clipboard part for {target:?} inside a write to {open:?}"
                        ));
                    }
                    Some((_, so_far)) => so_far,
                    None => 0,
                };
                let total = so_far + text.len();
                if total > GRID_MAX_CLIPBOARD_BYTES {
                    return Err(bound!(
                        "clipboard write of at least {total} bytes, over \
                         {GRID_MAX_CLIPBOARD_BYTES}"
                    ));
                }
                self.clipboard = (!last).then_some((*target, total));
            }
        }
        Ok(())
    }

    /// The message's `t`, for refusals.
    fn name(message: &GridOutput) -> &'static str {
        match message {
            GridOutput::Hello { .. } => "hello",
            GridOutput::Rows { .. } => "rows",
            GridOutput::Frame { .. } => "frame",
            GridOutput::History { .. } => "history",
            GridOutput::HistoryReset => "history_reset",
            GridOutput::State(_) => "state",
            GridOutput::Clipboard { .. } => "clipboard",
        }
    }
}

// ---- Client to supervisor ---------------------------------------------------

/// What a `mouse` input message reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseEventKind {
    Down,
    Up,
    Move,
    /// The wheel moved; `button` 0 is up, 1 is down. One message per
    /// browser wheel event that scrolls at least one line, which is what
    /// today's xterm.js sends (one report, or one arrow key, per such event).
    Wheel,
}

/// A client-to-supervisor grid message: input the supervisor encodes into
/// bytes for the program, from the program's own modes.
///
/// The client sends only what the program asked for (`mouse` per
/// [`TerminalState::mouse`], except `wheel` on the alternate screen, see
/// [`MouseMode`]; `focus` while [`TerminalState::focus`] is true); the
/// supervisor still drops what the program's actual mode does not report,
/// since the state the client acted on may be stale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum GridInput {
    /// A keydown the client does not handle itself (its own shortcuts, like
    /// Cmd+K/J, Ctrl+Shift+K/J and the text-size keys, never get here).
    /// `key`, `code` and `kc` (`keyCode`) are the browser's values; the
    /// supervisor encodes them the way the vendored xterm.js
    /// `evaluateKeyboardEvent` does with Farhelm's options, including
    /// Shift+Enter as ESC CR in one write. Printable keys that xterm.js sends
    /// at keydown (key code 48 and up) arrive here too, so the supervisor
    /// produces their bytes exactly as that function does; capitals, space,
    /// Option/AltGr characters, dead keys and composition results arrive as
    /// `text`.
    Key {
        key: String,
        code: String,
        kc: u32,
        /// The [`modifiers`] bit set.
        mods: u8,
    },
    /// Text to type as is: the printable characters that do not arrive as
    /// `key` (see there), and composition results.
    Text { data: String },
    /// One part of a paste. The supervisor wraps the whole paste, from the
    /// first part through the one with `last` set, in bracketed-paste
    /// markers when the program asked for them, and converts line endings as
    /// xterm.js does. Parts split only at character boundaries and never
    /// between a CR and its LF, arrive with no other input between them, and
    /// a paste the attachment's end interrupts still gets its closing marker
    /// (module docs, "Client to supervisor"). The client's classification of
    /// pasted content stays on the client and is unchanged.
    Paste { data: String, last: bool },
    /// A mouse event in cell coordinates, 0-based, within the screen.
    /// `button` is 0 left, 1 middle, 2 right for `down`, `up` and `move`
    /// (and -1 for motion with no button held); for `wheel` it is 0 up and 1
    /// down.
    Mouse {
        kind: MouseEventKind,
        button: i8,
        x: u16,
        y: u16,
        /// The [`modifiers`] bit set.
        mods: u8,
    },
    /// Focus moved into (`true`) or out of the terminal.
    Focus {
        #[serde(rename = "in")]
        focused: bool,
    },
}

impl GridInput {
    /// Decode one data frame's payload, refusing anything the supervisor
    /// must refuse: too large, malformed, an unknown `t`, or a broken bound.
    pub fn decode(bytes: &[u8]) -> Result<GridInput, GridError> {
        check_len(bytes.len(), GRID_MAX_INPUT_MESSAGE)?;
        check_object(bytes)?;
        let message: GridInput = serde_json::from_slice(bytes)?;
        message.validate()?;
        Ok(message)
    }

    /// Encode as one data frame's payload, refusing what the supervisor
    /// would reject.
    pub fn encode(&self) -> Result<Vec<u8>, GridError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)?;
        check_len(bytes.len(), GRID_MAX_INPUT_MESSAGE)?;
        Ok(bytes)
    }

    /// Check every bound of this message. Mouse coordinates are checked
    /// against [`GRID_MAX_DIMENSION`] only: the screen may have been resized
    /// since the client sent the event, so the supervisor's encoder handles a
    /// coordinate past the current screen itself.
    pub fn validate(&self) -> Result<(), GridError> {
        match self {
            GridInput::Key {
                key, code, mods, ..
            } => {
                for (name, value) in [("key", key), ("code", code)] {
                    if value.len() > GRID_MAX_KEY_BYTES {
                        return Err(bound!(
                            "{name} of {} bytes, over {GRID_MAX_KEY_BYTES}",
                            value.len()
                        ));
                    }
                }
                validate_mods(*mods)?;
            }
            GridInput::Text { data } => {
                if data.len() > GRID_MAX_TEXT_INPUT_BYTES {
                    return Err(bound!(
                        "text input of {} bytes, over {GRID_MAX_TEXT_INPUT_BYTES}",
                        data.len()
                    ));
                }
            }
            GridInput::Paste { data, .. } => {
                if data.len() > GRID_MAX_PASTE_PART_BYTES {
                    return Err(bound!(
                        "paste part of {} bytes, over {GRID_MAX_PASTE_PART_BYTES}",
                        data.len()
                    ));
                }
            }
            GridInput::Mouse {
                kind,
                button,
                x,
                y,
                mods,
            } => {
                let buttons = match kind {
                    MouseEventKind::Down | MouseEventKind::Up => 0..=2,
                    MouseEventKind::Move => -1..=2,
                    MouseEventKind::Wheel => 0..=1,
                };
                if !buttons.contains(button) {
                    return Err(bound!("mouse {kind:?} with button {button}"));
                }
                if *x >= GRID_MAX_DIMENSION || *y >= GRID_MAX_DIMENSION {
                    return Err(bound!("mouse at ({x}, {y}) is past the largest grid"));
                }
                validate_mods(*mods)?;
            }
            GridInput::Focus { .. } => {}
        }
        Ok(())
    }
}

/// Refuse a `mods` value with a bit [`modifiers`] does not define.
fn validate_mods(mods: u8) -> Result<(), GridError> {
    if mods & !modifiers::ALL != 0 {
        return Err(bound!("modifier bits {mods:#x} include undefined ones"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod tests;

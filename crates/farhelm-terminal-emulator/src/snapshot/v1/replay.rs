//! Version 1's reading rules for the stream: what a restore accepts before
//! it feeds a single byte, and the work it lets the stream take.
//!
//! A blob comes back from the terminal host, which keeps it for as long as it
//! runs and does not look inside it, so the decoder treats it as untrusted.
//! The stream is program output in form, and a few bytes of program output
//! can make an emulator do a great deal of work: `CSI 65535 b` (REP) prints
//! one character 65,535 times, and a line feed at the bottom of a screen
//! 10,000 columns wide clears a row of 10,000 cells. Fed blindly, a forged
//! stream under a megabyte would hold the shared supervisor for billions of
//! operations. So before feeding, [`check`] reads the whole stream once and
//! refuses it unless:
//!
//! - every sequence in it is one that `paint` writes, with the parameters it
//!   writes ([`Token`]); REP, carriage return, every C0 control but LF, HT
//!   and SO, every query, and every mode `paint` never sets are refused, as
//!   is invalid UTF-8 and an OSC ended by BEL;
//! - the work feeding it would take, as [`Model`] estimates it from above,
//!   stays within [`work_limit`]: a few units per byte, plus a few per cell
//!   of the grids it paints.
//!
//! The estimate charges what makes alacritty's work grow faster than the
//! bytes: every line feed and every wrap a scroll of a whole row (plus the
//! screen's height when a partial scroll region makes alacritty swap rows),
//! every erase of the screen or switch of screens all its cells, every
//! erase of characters as many cells as it clears, a tab after the tab stops
//! were changed a row's width, every print in insert mode a row's width, and
//! a title push its title (and the title stack's depth, once it is full).
//! The cursor's column and pending wrap are followed through the stream so
//! that only a print that can wrap is charged a scroll; where the model
//! cannot tell (after a tab, or a switch of screens), it assumes the worst.
//!
//! A stream that `paint` wrote stays well within the limit: it scrolls each
//! history row in once, wraps it at most once and erases it at most once,
//! switches screens at most three times and erases a screen once, and every
//! other sequence costs about its length. The limit allows twice that, so a
//! forged blob costs at most a few times what the largest real one does.
//!
//! These rules are part of version 1 and frozen with it: [`WORK_PER_BYTE`],
//! [`WORK_PER_CELL`] and [`WORK_CELLS_CAP`] are version 1's own constants,
//! not this build's policy. The cap ties version 1 to grids of about today's
//! size: a stream costs up to about five units a cell (a scroll, a wrap and
//! an erase for every row), so a later build that raised its restore bound
//! (`super::super::MAX_RESTORE_CELLS`, today 2^24 cells) past about five
//! times today's could write version 1 blobs this reader refuses. A bound
//! that large needs a new version.

use unicode_width::UnicodeWidthChar;

use super::super::SnapshotError;
use super::wire::malformed;

/// Work units each stream byte may take ([`work_limit`]).
const WORK_PER_BYTE: u64 = 4;
/// Work units each cell of the grids a stream paints may take.
const WORK_PER_CELL: u64 = 16;
/// The most cells [`work_limit`] counts, whatever a header claims: twice
/// the most a restore allocates today, so that every blob an encoder of
/// this version writes is within it.
const WORK_CELLS_CAP: u64 = 1 << 25;
/// What a tab costs while the tab stops are a fresh terminal's, one every
/// eight columns: alacritty walks the columns to the next stop.
const DEFAULT_TAB_WORK: u64 = 8;
/// The deepest the title stack gets (`TITLE_STACK_MAX_DEPTH` in the vendored
/// `alacritty_terminal`); a push onto a full stack moves every entry.
const TITLE_STACK_DEPTH: u64 = 4096;
/// The longest title alacritty keeps (`MAX_TITLE_BYTES`), which a push copies.
const MAX_TITLE: u64 = 4096;

/// The most work a stream may take: [`WORK_PER_BYTE`] for each of its bytes
/// and [`WORK_PER_CELL`] for each cell of the grids it paints (both screens
/// and `history` lines of history, at most [`WORK_CELLS_CAP`] of them).
fn work_limit(len: usize, columns: u16, lines: u16, history: usize) -> u64 {
    let rows = 2 * u64::from(lines) + history as u64;
    let cells = u64::from(columns).saturating_mul(rows).min(WORK_CELLS_CAP);
    WORK_PER_BYTE * len as u64 + WORK_PER_CELL * cells
}

/// Check `stream` against version 1's rules (module docs) for a terminal of
/// `columns` by `lines` whose blob says the stream rebuilds `history` lines.
/// `Ok` means it may be fed, and holds the work it was counted at and the
/// limit, for tests of the margin.
pub(super) fn check(
    stream: &[u8],
    columns: u16,
    lines: u16,
    history: usize,
) -> Result<Work, SnapshotError> {
    let limit = work_limit(stream.len(), columns, lines, history);
    let mut model = Model::new(columns, lines);
    let mut tokens = Tokens::new(stream);
    while let Some(token) = tokens.next()? {
        model.apply(&token);
        if model.work > limit {
            return Err(SnapshotError::TooMuchWork { limit });
        }
    }
    Ok(Work {
        counted: model.work,
        limit,
    })
}

/// What [`check`] counted for a stream it accepted, and its limit.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(test), allow(dead_code, reason = "read by the margin test"))]
pub(super) struct Work {
    pub(super) counted: u64,
    pub(super) limit: u64,
}

/// The most stream bytes the decoder feeds in one call. Effects queue until
/// a feed returns (a title event per OSC 2, say), so a longer piece of a
/// forged stream of titles could hold several times its size in queued
/// events; 64 KiB keeps that to a few hundred kilobytes, like live reads.
pub(super) const FEED_PIECE: usize = 64 * 1024;

/// Where the decoder ends a feed of `stream` (already checked), and whether
/// the cursor template's extra may have changed there, so that it shares it
/// (`wire::Extras::share_template`): the end of every SGR that sets or
/// removes an underline color or resets, and of every OSC 8. Pieces are also
/// ended at the first token boundary [`FEED_PIECE`] bytes after the last
/// end, and at the end of the stream.
pub(super) fn feed_points(stream: &[u8]) -> impl Iterator<Item = (usize, bool)> + '_ {
    let mut tokens = Tokens::new(stream);
    let mut last = 0;
    let mut done = false;
    std::iter::from_fn(move || {
        while !done {
            let Some(token) = tokens.next().ok().flatten() else {
                done = true;
                return (last < stream.len() || stream.is_empty()).then_some((stream.len(), true));
            };
            let changes = token.changes_template();
            if changes || tokens.at - last >= FEED_PIECE {
                last = tokens.at;
                return Some((tokens.at, changes));
            }
        }
        None
    })
}

// ---------------------------------------------------------------------------
// Tokens: the sequences `paint` writes.

/// The most parameters (with subparameters) vte keeps for a CSI.
const MAX_PARAMS: usize = 32;

/// One sequence of the stream, or one printed character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    Print(char),
    LineFeed,
    Tab,
    ShiftOut,
    /// DECSC, `ESC 7`.
    Save,
    /// DECRC, `ESC 8`.
    Restore,
    /// HTS, `ESC H`.
    SetTabStop,
    /// DECKPAM, `ESC =`.
    KeypadApplication,
    /// A character set designation, `ESC ( B` and the like.
    Charset,
    /// CUP, `CSI l ; c H`: the column, from 0.
    Position(u64),
    /// CHA, `CSI n G`: the column, from 0.
    Column(u64),
    /// CUF, `CSI n C`.
    Forward(u64),
    /// ECH, `CSI n X`.
    Erase(u64),
    /// ED, `CSI 2 J`.
    EraseScreen,
    /// TBC, `CSI 3 g`.
    ClearTabStops,
    /// DECSTBM, `CSI t ; b r`: top and bottom as given (0 or absent for the
    /// defaults).
    Region(u64, u64),
    /// `CSI 22 t`, pushing the title.
    PushTitle,
    /// DECSCUSR, `CSI n SP q`.
    CursorStyle,
    /// SGR; whether it can change the template's extra (an underline color
    /// set or removed, or a reset), and its parameter count.
    Attributes {
        extra: bool,
        params: u64,
    },
    /// `CSI ? 1049 h` or `l`.
    AlternateScreen(bool),
    /// `CSI ? 6 h`, which homes the cursor.
    Origin,
    /// `CSI 4 h`.
    Insert,
    /// Any other mode `paint` sets or resets.
    Mode,
    /// OSC 2, with its payload's length.
    Title(u64),
    /// OSC 8, with its payload's length.
    Hyperlink(u64),
    /// OSC 4, 10, 11 or 12.
    Color(u64),
}

impl Token {
    /// See [`feed_points`].
    fn changes_template(&self) -> bool {
        matches!(
            self,
            Token::Attributes { extra: true, .. } | Token::Hyperlink(_)
        )
    }
}

/// The private modes `paint` sets or resets with `CSI ? n h` / `l`, apart
/// from 1049 and 6 (their own tokens) and 7 (only ever reset).
const PRIVATE_MODES: [u16; 11] = [25, 1, 1000, 1002, 1003, 2004, 1006, 1005, 1004, 1007, 1042];

/// Reads a stream as [`Token`]s, refusing anything else as malformed.
struct Tokens<'a> {
    data: &'a [u8],
    /// The offset just past the token read last.
    at: usize,
}

/// A CSI's parameters as vte groups them: each parameter with its
/// subparameters (`4:2` is one parameter of two values).
struct Params {
    values: [u16; MAX_PARAMS],
    /// How many values were read, subparameters included.
    len: usize,
    /// Where each parameter starts in `values`, and one past the last
    /// complete one.
    starts: [usize; MAX_PARAMS + 1],
    /// How many parameters are complete.
    count: usize,
}

impl Params {
    fn get(&self, index: usize) -> &[u16] {
        &self.values[self.starts[index]..self.starts[index + 1]]
    }

    /// Exactly one parameter without subparameters, and its value.
    fn single(&self) -> Option<u16> {
        (self.count == 1 && self.get(0).len() == 1).then(|| self.get(0)[0])
    }

    /// The `index`th parameter's value, `default` when it is absent or 0 (as
    /// vte reads a cursor move's count or position).
    fn or(&self, index: usize, default: u64) -> u64 {
        match (index < self.count).then(|| self.get(index)[0]) {
            None | Some(0) => default,
            Some(value) => u64::from(value),
        }
    }

    /// How many values there are, subparameters included.
    fn values(&self) -> usize {
        self.len
    }
}

fn unexpected(what: impl std::fmt::Display) -> SnapshotError {
    malformed(format!(
        "the stream holds {what}, which version 1 never writes"
    ))
}

impl<'a> Tokens<'a> {
    fn new(data: &'a [u8]) -> Tokens<'a> {
        Tokens { data, at: 0 }
    }

    fn byte(&mut self) -> Result<u8, SnapshotError> {
        let byte = *self
            .data
            .get(self.at)
            .ok_or_else(|| malformed("the stream ends inside a sequence"))?;
        self.at += 1;
        Ok(byte)
    }

    /// The next token, `None` at the end of the stream.
    fn next(&mut self) -> Result<Option<Token>, SnapshotError> {
        let Some(&first) = self.data.get(self.at) else {
            return Ok(None);
        };
        self.at += 1;
        let token = match first {
            b'\n' => Token::LineFeed,
            b'\t' => Token::Tab,
            0x0e => Token::ShiftOut,
            0x1b => self.escape()?,
            0x20..=0x7e => Token::Print(char::from(first)),
            0x80..=0xff => Token::Print(self.utf8(first)?),
            other => return Err(unexpected(format_args!("the control byte {other:#04x}"))),
        };
        Ok(Some(token))
    }

    /// A character encoded in UTF-8 that starts with `first`.
    fn utf8(&mut self, first: u8) -> Result<char, SnapshotError> {
        let len = match first {
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => return Err(malformed("the stream is not UTF-8")),
        };
        let start = self.at - 1;
        let end = start + len;
        let bytes = self
            .data
            .get(start..end)
            .ok_or_else(|| malformed("the stream ends inside a character"))?;
        let text = std::str::from_utf8(bytes).map_err(|_| malformed("the stream is not UTF-8"))?;
        self.at = end;
        Ok(text.chars().next().expect("a decoded character"))
    }

    fn escape(&mut self) -> Result<Token, SnapshotError> {
        Ok(match self.byte()? {
            b'7' => Token::Save,
            b'8' => Token::Restore,
            b'H' => Token::SetTabStop,
            b'=' => Token::KeypadApplication,
            b'(' | b')' | b'*' | b'+' => match self.byte()? {
                b'B' | b'0' => Token::Charset,
                other => return Err(unexpected(format_args!("character set {other:#04x}"))),
            },
            b'[' => self.csi()?,
            b']' => self.osc()?,
            other => return Err(unexpected(format_args!("ESC {:?}", char::from(other)))),
        })
    }

    fn csi(&mut self) -> Result<Token, SnapshotError> {
        let private = self.data.get(self.at) == Some(&b'?');
        if private {
            self.at += 1;
        }
        let mut params = Params {
            values: [0; MAX_PARAMS],
            len: 0,
            starts: [0; MAX_PARAMS + 1],
            count: 0,
        };
        // vte's grouping: `;` ends a parameter, `:` a subparameter; an empty
        // one is 0. A CSI with no digits and no separators has none.
        let mut value: u32 = 0;
        let mut digits = 0;
        let mut any = false;
        let mut intermediate = false;
        let final_byte = loop {
            let byte = self.byte()?;
            match byte {
                b'0'..=b'9' if !intermediate => {
                    digits += 1;
                    if digits > 5 {
                        return Err(unexpected("a CSI parameter over 65535"));
                    }
                    value = value * 10 + u32::from(byte - b'0');
                    any = true;
                }
                b';' | b':' if !intermediate => {
                    push_value(&mut params, value, byte == b':')?;
                    value = 0;
                    digits = 0;
                    any = true;
                }
                b' ' if !intermediate => intermediate = true,
                0x40..=0x7e => break byte,
                other => {
                    return Err(unexpected(format_args!(
                        "the byte {other:#04x} inside a CSI"
                    )));
                }
            }
        };
        if any {
            push_value(&mut params, value, false)?;
        }
        let plain = !private && !intermediate;
        let token = match final_byte {
            b'H' if plain && params.count <= 2 => Token::Position(params.or(1, 1) - 1),
            b'G' if plain && params.count <= 1 => Token::Column(params.or(0, 1) - 1),
            b'C' if plain && params.count <= 1 => Token::Forward(params.or(0, 1)),
            b'X' if plain && params.count <= 1 => Token::Erase(params.or(0, 1)),
            b'J' if plain && params.single() == Some(2) => Token::EraseScreen,
            b'g' if plain && params.single() == Some(3) => Token::ClearTabStops,
            b'r' if plain && (params.count == 0 || params.count == 2) => {
                let top = params.or(0, 0);
                let bottom = params.or(1, 0);
                Token::Region(top, bottom)
            }
            b't' if plain && params.single() == Some(22) => Token::PushTitle,
            b'q' if !private && intermediate && matches!(params.single(), Some(1..=6)) => {
                Token::CursorStyle
            }
            b'm' if plain => sgr(&params)?,
            b'h' if plain && params.single() == Some(4) => Token::Insert,
            b'h' if plain && params.single() == Some(20) => Token::Mode,
            b'h' | b'l' if private && params.count == 1 => {
                let set = final_byte == b'h';
                match params.single() {
                    Some(1049) => Token::AlternateScreen(set),
                    Some(6) if set => Token::Origin,
                    // Autowrap, only ever turned off, at the stream's end.
                    Some(7) if !set => Token::Mode,
                    Some(mode) if PRIVATE_MODES.contains(&mode) => Token::Mode,
                    _ => return Err(unexpected("a private mode")),
                }
            }
            other => {
                return Err(unexpected(format_args!(
                    "the CSI sequence ending {:?}",
                    char::from(other)
                )));
            }
        };
        Ok(token)
    }

    /// An OSC, which `paint` always ends with ST (`ESC \`).
    fn osc(&mut self) -> Result<Token, SnapshotError> {
        let start = self.at;
        let end = loop {
            match self.byte()? {
                0x1b => break self.at - 1,
                0x20..=0xff => {}
                other => {
                    return Err(unexpected(format_args!(
                        "the byte {other:#04x} inside an OSC"
                    )));
                }
            }
        };
        if self.byte()? != b'\\' {
            return Err(unexpected("an OSC not ended by ST"));
        }
        let payload = &self.data[start..end];
        let len = payload.len() as u64;
        let (kind, rest) = match payload.iter().position(|&byte| byte == b';') {
            Some(split) => (&payload[..split], &payload[split + 1..]),
            None => return Err(unexpected("an OSC without parameters")),
        };
        Ok(match kind {
            b"2" => Token::Title(len),
            b"8" if rest == b";" || link_payload(rest) => Token::Hyperlink(len),
            b"4" => match rest.iter().position(|&byte| byte == b';') {
                Some(split) if is_digits(&rest[..split]) && is_rgb(&rest[split + 1..]) => {
                    Token::Color(len)
                }
                _ => return Err(unexpected("a malformed OSC 4")),
            },
            b"10" | b"11" | b"12" if is_rgb(rest) => Token::Color(len),
            _ => return Err(unexpected("an OSC of a kind version 1 never writes")),
        })
    }
}

/// Add `value` to the parameter being read, which ends with it unless a
/// subparameter follows (`:`).
fn push_value(
    params: &mut Params,
    value: u32,
    subparameter_follows: bool,
) -> Result<(), SnapshotError> {
    if params.len >= MAX_PARAMS {
        return Err(unexpected("a CSI with more than 32 parameters"));
    }
    // At most five digits, so at most 99,999; vte saturates at 65,535.
    params.values[params.len] = value.min(u32::from(u16::MAX)) as u16;
    params.len += 1;
    if !subparameter_follows {
        params.count += 1;
        params.starts[params.count] = params.len;
    }
    Ok(())
}

/// The SGR parameters `paint` writes: attributes on and off, the eight and
/// sixteen colors and their defaults, `38`/`48`/`58` with `5;n` or
/// `2;r;g;b`, `59`, and the underline styles `4:2` to `4:5`.
fn sgr(params: &Params) -> Result<Token, SnapshotError> {
    let mut extra = params.count == 0;
    let mut index = 0;
    while index < params.count {
        let param = params.get(index);
        index += 1;
        match *param {
            [0] => extra = true,
            [
                1..=4
                | 7..=9
                | 22..=24
                | 27..=29
                | 30..=37
                | 39
                | 40..=47
                | 49
                | 90..=97
                | 100..=107,
            ] => {}
            [4, 2..=5] => {}
            [59] => extra = true,
            [code @ (38 | 48 | 58)] => {
                let takes = match (index < params.count).then(|| params.get(index)) {
                    Some([5]) => 2,
                    Some([2]) => 4,
                    _ => return Err(unexpected("a malformed SGR color")),
                };
                if index + takes > params.count
                    || (index..index + takes)
                        .any(|i| params.get(i).len() != 1 || params.get(i)[0] > 255)
                {
                    return Err(unexpected("a malformed SGR color"));
                }
                index += takes;
                extra |= code == 58;
            }
            _ => return Err(unexpected("an SGR parameter")),
        }
    }
    Ok(Token::Attributes {
        extra,
        params: params.values() as u64,
    })
}

/// `id=<id>;<uri>`: an OSC 8 that `paint` writes for a link.
fn link_payload(rest: &[u8]) -> bool {
    let Some(rest) = rest.strip_prefix(b"id=") else {
        return false;
    };
    match rest.iter().position(|&byte| byte == b';') {
        Some(split) => !rest[..split].contains(&b':') && split + 1 < rest.len(),
        None => false,
    }
}

fn is_digits(bytes: &[u8]) -> bool {
    !bytes.is_empty() && bytes.len() <= 3 && bytes.iter().all(u8::is_ascii_digit)
}

/// `rgb:hh/hh/hh`, as `paint` writes colors.
fn is_rgb(bytes: &[u8]) -> bool {
    let Some(rest) = bytes.strip_prefix(b"rgb:") else {
        return false;
    };
    rest.len() == 8
        && rest[2] == b'/'
        && rest[5] == b'/'
        && [0, 1, 3, 4, 6, 7]
            .iter()
            .all(|&i| rest[i].is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// The work model.

/// An upper bound on what feeding the tokens so far made alacritty do, and
/// as much of the terminal's state as that needs: where the cursor is on its
/// row, the saved cursors, the scroll region and a few modes.
struct Model {
    columns: u64,
    lines: u64,
    /// The cursor's column, and whether a wrap is pending; when the model
    /// cannot tell, the last column with a wrap pending (the worst case: the
    /// next print wraps).
    column: u64,
    pending: bool,
    /// The saved cursors' columns and pending wraps, primary and alternate.
    saved: [(u64, bool); 2],
    alternate: bool,
    /// Whether the scroll region is the whole screen, which alacritty
    /// scrolls by rotating; inside a smaller one it swaps rows.
    full_region: bool,
    tabs_default: bool,
    insert: bool,
    title: u64,
    pushes: u64,
    work: u64,
}

impl Model {
    fn new(columns: u16, lines: u16) -> Model {
        Model {
            columns: u64::from(columns),
            lines: u64::from(lines),
            column: 0,
            pending: false,
            saved: [(0, false); 2],
            alternate: false,
            full_region: true,
            tabs_default: true,
            insert: false,
            title: 0,
            pushes: 0,
            work: 0,
        }
    }

    fn last(&self) -> u64 {
        self.columns - 1
    }

    /// What scrolling one row takes: clearing it (and growing history by
    /// alacritty's thousand-row steps, a row's worth per row on average),
    /// and swapping every row when a partial region is set.
    fn scroll(&mut self) {
        self.work += 2 * self.columns;
        if !self.full_region {
            self.work += self.lines;
        }
    }

    fn screen(&self) -> u64 {
        self.columns * self.lines
    }

    fn apply(&mut self, token: &Token) {
        self.work += 1;
        match *token {
            Token::Print(c) => self.print(c),
            Token::LineFeed => self.scroll(),
            Token::Tab => {
                if self.pending {
                    self.scroll();
                    self.column = 0;
                    self.pending = false;
                } else {
                    self.work += if self.tabs_default {
                        DEFAULT_TAB_WORK
                    } else {
                        self.columns
                    };
                    self.column = self.last();
                }
            }
            Token::ShiftOut | Token::KeypadApplication | Token::Charset | Token::Mode => {}
            Token::CursorStyle => {}
            Token::Save => self.saved[usize::from(self.alternate)] = (self.column, self.pending),
            Token::Restore => {
                (self.column, self.pending) = self.saved[usize::from(self.alternate)];
            }
            Token::SetTabStop => self.tabs_default = false,
            Token::ClearTabStops => {
                self.tabs_default = false;
                self.work += self.columns;
            }
            Token::Position(column) | Token::Column(column) => {
                self.column = column.min(self.last());
                self.pending = false;
            }
            Token::Forward(count) => {
                self.column = (self.column + count).min(self.last());
                self.pending = false;
            }
            Token::Erase(count) => self.work += count.min(self.columns),
            Token::EraseScreen => self.work += 2 * self.screen(),
            Token::Region(top, bottom) => {
                let top = top.max(1);
                let bottom = if bottom == 0 { self.lines } else { bottom };
                // alacritty ignores a region whose top is not above its
                // bottom, as given.
                if top < bottom {
                    self.full_region = top == 1 && bottom >= self.lines;
                    self.column = 0;
                    self.pending = false;
                }
            }
            Token::PushTitle => {
                self.work += self.title;
                if self.pushes >= TITLE_STACK_DEPTH {
                    self.work += TITLE_STACK_DEPTH;
                }
                self.pushes += 1;
            }
            Token::Attributes { params, .. } => self.work += params,
            Token::AlternateScreen(set) => {
                self.work += self.screen();
                if set && !self.alternate {
                    self.saved[0] = (self.column, self.pending);
                    self.alternate = true;
                } else if !set && self.alternate {
                    self.alternate = false;
                    (self.column, self.pending) = self.saved[0];
                }
            }
            Token::Origin => {
                self.column = 0;
                self.pending = false;
            }
            Token::Insert => self.insert = true,
            Token::Title(len) => {
                self.work += len;
                self.title = len.min(MAX_TITLE);
            }
            Token::Hyperlink(len) | Token::Color(len) => self.work += len,
        }
    }

    /// A printed character, as alacritty's `Term::input` moves the cursor.
    fn print(&mut self, c: char) {
        let width = match c.width() {
            None | Some(0) => return,
            Some(width) => width as u64,
        };
        if width > self.columns {
            return;
        }
        if self.pending {
            self.scroll();
            self.column = 0;
            self.pending = false;
        }
        if width == 2 && self.column == self.last() {
            // A wide character in the last column leaves a leading spacer
            // there and wraps.
            self.scroll();
            self.column = 0;
        }
        if self.insert {
            self.work += self.columns;
        }
        if self.column + width >= self.columns {
            self.column = self.last();
            self.pending = true;
        } else {
            self.column += width;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(stream: &[u8]) -> Result<Vec<Token>, SnapshotError> {
        let mut tokens = Tokens::new(stream);
        let mut all = Vec::new();
        while let Some(token) = tokens.next()? {
            all.push(token);
        }
        Ok(all)
    }

    /// Each sequence `paint` writes reads as its token, with the parameters
    /// it was written with.
    ///
    /// Why: a sequence of the encoder's that the checker refused would make
    /// every blob holding it unrestorable, and v1 is frozen. Spec: the
    /// module docs; the sequences are those in `paint`.
    #[test]
    fn the_painter_s_sequences_are_accepted() {
        let cases: &[(&[u8], Token)] = &[
            (b"x", Token::Print('x')),
            ("中".as_bytes(), Token::Print('中')),
            ("\u{85}".as_bytes(), Token::Print('\u{85}')),
            (b"\n", Token::LineFeed),
            (b"\t", Token::Tab),
            (b"\x0e", Token::ShiftOut),
            (b"\x1b7", Token::Save),
            (b"\x1b8", Token::Restore),
            (b"\x1bH", Token::SetTabStop),
            (b"\x1b=", Token::KeypadApplication),
            (b"\x1b)0", Token::Charset),
            (b"\x1b+B", Token::Charset),
            (b"\x1b[12;34H", Token::Position(33)),
            (b"\x1b[H", Token::Position(0)),
            (b"\x1b[7G", Token::Column(6)),
            (b"\x1b[C", Token::Forward(1)),
            (b"\x1b[12C", Token::Forward(12)),
            (b"\x1b[X", Token::Erase(1)),
            (b"\x1b[200X", Token::Erase(200)),
            (b"\x1b[2J", Token::EraseScreen),
            (b"\x1b[3g", Token::ClearTabStops),
            (b"\x1b[3;5r", Token::Region(3, 5)),
            (b"\x1b[r", Token::Region(0, 0)),
            (b"\x1b[22t", Token::PushTitle),
            (b"\x1b[6 q", Token::CursorStyle),
            (
                b"\x1b[0;1;4:3;38;5;200;48;2;1;2;3;58;5;9m",
                Token::Attributes {
                    extra: true,
                    params: 15,
                },
            ),
            (
                b"\x1b[22;2;39;49m",
                Token::Attributes {
                    extra: false,
                    params: 4,
                },
            ),
            (
                b"\x1b[59m",
                Token::Attributes {
                    extra: true,
                    params: 1,
                },
            ),
            (b"\x1b[?1049h", Token::AlternateScreen(true)),
            (b"\x1b[?1049l", Token::AlternateScreen(false)),
            (b"\x1b[?6h", Token::Origin),
            (b"\x1b[?7l", Token::Mode),
            (b"\x1b[?25l", Token::Mode),
            (b"\x1b[?2004h", Token::Mode),
            (b"\x1b[4h", Token::Insert),
            (b"\x1b[20h", Token::Mode),
            (b"\x1b]2;a title; with ; semicolons\x1b\\", Token::Title(28)),
            (b"\x1b]8;id=a;http://e/;x\x1b\\", Token::Hyperlink(18)),
            (b"\x1b]8;;\x1b\\", Token::Hyperlink(3)),
            (b"\x1b]4;200;rgb:ff/00/80\x1b\\", Token::Color(18)),
            (b"\x1b]11;rgb:01/02/03\x1b\\", Token::Color(15)),
        ];
        for &(stream, token) in cases {
            assert_eq!(tokens(stream), Ok(vec![token]), "{}", stream.escape_ascii());
        }
    }

    /// Sequences `paint` never writes are refused, as are broken ones.
    ///
    /// Why: the checker is what keeps a forged stream from making the
    /// supervisor do unbounded work or reach state the format does not
    /// describe; REP is the review's amplifier. Spec: the module docs.
    #[test]
    fn everything_else_is_refused() {
        let cases: &[&[u8]] = &[
            b"x\x1b[65535b",
            b"\r",
            b"\x07",
            b"\x7f",
            b"\x1b[?2026h",
            b"\x1b[?1000;1006h",
            b"\x1b[?9h",
            b"\x1b[6n",
            b"\x1b[c",
            b"\x1bc",
            b"\x1b#8",
            b"\x1b[1J",
            b"\x1b[K",
            b"\x1b[L",
            b"\x1b[S",
            b"\x1b[5@",
            b"\x1b[4l",
            b"\x1b[38;5m",
            b"\x1b[38;5;256m",
            b"\x1b[5m",
            b"\x1b[3;1;2r",
            b"\x1b[123456C",
            b"\x1b]52;c;aGk=\x1b\\",
            b"\x1b]2;title\x07",
            b"\x1b]8;id=a:b;http://e/\x1b\\",
            b"\x1b]8;id=a;\x1b\\",
            b"\x1b]4;1;#102030\x1b\\",
            b"\x1bP1$r\x1b\\",
            b"\x1b[3",
            b"\xe4\xb8",
            b"\xc0\x80",
            b"\xed\xa0\x80",
        ];
        for &stream in cases {
            assert!(
                matches!(tokens(stream), Err(SnapshotError::Malformed(_))),
                "{}",
                stream.escape_ascii()
            );
        }
        let many = format!("\x1b[{}m", vec!["1"; 33].join(";"));
        assert!(tokens(many.as_bytes()).is_err());
    }

    /// The model charges a print a scroll only when it can wrap: after a
    /// position it knows, prints that fill the row up to its last column do
    /// not, and the print after a pending wrap does.
    ///
    /// Why: a model that charged every print a row would refuse real
    /// snapshots of wide terminals; one that charged no print would let a
    /// stream of prints on a one-column terminal scroll for free. Spec:
    /// `Model::print`.
    #[test]
    fn prints_are_charged_a_scroll_only_when_they_wrap() {
        let mut model = Model::new(100, 4);
        model.apply(&Token::Position(0));
        for _ in 0..100 {
            model.apply(&Token::Print('x'));
        }
        assert_eq!(model.work, 101);
        assert!(model.pending);
        model.apply(&Token::Print('x'));
        assert_eq!(model.work, 102 + 200);
        model.apply(&Token::Position(99));
        model.apply(&Token::Print('中'));
        assert_eq!(
            model.column, 2,
            "a wide character wraps from the last column"
        );
    }
}

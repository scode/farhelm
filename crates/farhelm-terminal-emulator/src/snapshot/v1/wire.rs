//! The binary encoding of version 1's fields: integers, strings, and the
//! cells, colors and cursors the fixups carry.
//!
//! Every integer is big-endian, as on the terminal host's wire. Cells are
//! written in this module's own terms, never in `alacritty_terminal`'s
//! (its flag bits, its enum discriminants): those may change with any
//! upgrade, while a version 1 blob must read the same forever. The two tables
//! below pin the mapping, and a value outside them makes a blob malformed
//! rather than being guessed at.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use alacritty_terminal::grid::Cursor;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::{
    Cell, CellExtra, ExtraStorage, Flags, Hyperlink, MAX_ZEROWIDTH_CHARS,
};
use alacritty_terminal::term::{MAX_HYPERLINK_ID_BYTES, MAX_HYPERLINK_URI_BYTES};
use alacritty_terminal::vte::ansi::{
    CharsetIndex, Color, Hyperlink as VteHyperlink, NamedColor, Rgb, StandardCharset,
};

use super::super::SnapshotError;

/// Cell flags as version 1 numbers them. The values happen to equal
/// alacritty 0.26.0's, but they are version 1's own and never change.
/// Composite flags (`BOLD_ITALIC`, `ALL_UNDERLINES`) are their parts.
const FLAG_BITS: [(Flags, u16); 15] = [
    (Flags::INVERSE, 1 << 0),
    (Flags::BOLD, 1 << 1),
    (Flags::ITALIC, 1 << 2),
    (Flags::UNDERLINE, 1 << 3),
    (Flags::WRAPLINE, 1 << 4),
    (Flags::WIDE_CHAR, 1 << 5),
    (Flags::WIDE_CHAR_SPACER, 1 << 6),
    (Flags::DIM, 1 << 7),
    (Flags::HIDDEN, 1 << 8),
    (Flags::STRIKEOUT, 1 << 9),
    (Flags::LEADING_WIDE_CHAR_SPACER, 1 << 10),
    (Flags::DOUBLE_UNDERLINE, 1 << 11),
    (Flags::UNDERCURL, 1 << 12),
    (Flags::DOTTED_UNDERLINE, 1 << 13),
    (Flags::DASHED_UNDERLINE, 1 << 14),
];

/// Named colors as version 1 numbers them: the color slot each names, the
/// way xterm and alacritty count slots (0 to 15 the ANSI colors, 256 on the
/// special ones).
const NAMED_COLORS: [(NamedColor, u16); 29] = [
    (NamedColor::Black, 0),
    (NamedColor::Red, 1),
    (NamedColor::Green, 2),
    (NamedColor::Yellow, 3),
    (NamedColor::Blue, 4),
    (NamedColor::Magenta, 5),
    (NamedColor::Cyan, 6),
    (NamedColor::White, 7),
    (NamedColor::BrightBlack, 8),
    (NamedColor::BrightRed, 9),
    (NamedColor::BrightGreen, 10),
    (NamedColor::BrightYellow, 11),
    (NamedColor::BrightBlue, 12),
    (NamedColor::BrightMagenta, 13),
    (NamedColor::BrightCyan, 14),
    (NamedColor::BrightWhite, 15),
    (NamedColor::Foreground, 256),
    (NamedColor::Background, 257),
    (NamedColor::Cursor, 258),
    (NamedColor::DimBlack, 259),
    (NamedColor::DimRed, 260),
    (NamedColor::DimGreen, 261),
    (NamedColor::DimYellow, 262),
    (NamedColor::DimBlue, 263),
    (NamedColor::DimMagenta, 264),
    (NamedColor::DimCyan, 265),
    (NamedColor::DimWhite, 266),
    (NamedColor::BrightForeground, 267),
    (NamedColor::DimForeground, 268),
];

/// Builds a blob.
#[derive(Default)]
pub(super) struct Writer {
    pub(super) buf: Vec<u8>,
}

impl Writer {
    pub(super) fn u8(&mut self, value: u8) {
        self.buf.push(value);
    }

    pub(super) fn u16(&mut self, value: u16) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    pub(super) fn u64(&mut self, value: u64) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    pub(super) fn i32(&mut self, value: i32) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    /// A length, which every version 1 count and size fits in: a blob is at
    /// most the host's 16 MiB, and nothing in a terminal is counted past
    /// `u32::MAX` before that.
    pub(super) fn len(&mut self, len: usize) {
        self.u32(u32::try_from(len).expect("a snapshot field longer than 4 GiB"));
    }

    /// A length-prefixed byte string.
    pub(super) fn bytes(&mut self, bytes: &[u8]) {
        self.len(bytes.len());
        self.buf.extend_from_slice(bytes);
    }

    pub(super) fn char(&mut self, c: char) {
        self.u32(u32::from(c));
    }

    pub(super) fn color(&mut self, color: Color) {
        match color {
            Color::Named(named) => {
                self.u8(0);
                self.u16(named_slot(named));
            }
            Color::Indexed(index) => {
                self.u8(1);
                self.u8(index);
            }
            Color::Spec(Rgb { r, g, b }) => {
                self.u8(2);
                self.u8(r);
                self.u8(g);
                self.u8(b);
            }
        }
    }

    /// A cell, whole: character, colors, flags, zero-width characters,
    /// underline color and hyperlink.
    pub(super) fn cell(&mut self, cell: &Cell) {
        self.char(cell.c);
        self.color(cell.fg);
        self.color(cell.bg);
        self.u16(flags_to_bits(cell.flags));
        let zerowidth = cell.zerowidth().unwrap_or(&[]);
        // At most `V1_MAX_ZEROWIDTH` (16): the emulator's cap is no higher
        // (asserted with the limits below).
        self.u8(u8::try_from(zerowidth.len()).expect("zero-width characters are capped at 16"));
        for &c in zerowidth {
            self.char(c);
        }
        match cell.underline_color() {
            None => self.u8(0),
            Some(color) => {
                self.u8(1);
                self.color(color);
            }
        }
        match cell.hyperlink() {
            None => self.u8(0),
            Some(link) => {
                self.u8(1);
                self.bytes(link.id().as_bytes());
                self.bytes(link.uri().as_bytes());
            }
        }
    }

    /// A cursor: position, pending wrap, template cell and character sets.
    pub(super) fn cursor(&mut self, cursor: &Cursor<Cell>) {
        self.i32(cursor.point.line.0);
        self.len(cursor.point.column.0);
        self.u8(u8::from(cursor.input_needs_wrap));
        self.cell(&cursor.template);
        for index in CHARSET_INDEXES {
            self.u8(match cursor.charsets[index] {
                StandardCharset::Ascii => 0,
                StandardCharset::SpecialCharacterAndLineDrawing => 1,
            });
        }
    }
}

/// How many bytes [`Writer::color`] writes for `color`.
fn color_len(color: Color) -> usize {
    match color {
        Color::Named(_) => 3,
        Color::Indexed(_) => 2,
        Color::Spec(_) => 4,
    }
}

/// How many bytes [`Writer::cell`] writes for `cell`, without writing it:
/// for measuring a blob before deciding what goes in it.
pub(super) fn cell_len(cell: &Cell) -> usize {
    let zerowidth = cell.zerowidth().map_or(0, <[char]>::len);
    let underline = cell.underline_color().map_or(0, color_len);
    let link = cell
        .hyperlink()
        .map_or(0, |link| 4 + link.id().len() + 4 + link.uri().len());
    4 + color_len(cell.fg) + color_len(cell.bg) + 2 + 1 + 4 * zerowidth + 1 + underline + 1 + link
}

/// How a restore makes its cells' extras (combining characters, underline
/// color, hyperlink): through the restored terminal's extra storage, so that
/// they count against the same limit as the program's own, and shared among
/// cells and cursor templates whose extras hold the same, as cells printed
/// from one template share theirs.
///
/// Without the sharing, a restore could need far more extras than the
/// terminal it rebuilds: a snapshot of many fixup cells sharing one extra
/// (an underline color over a row of orphaned wide-character halves) would
/// restore to an extra per cell, and a stream that sets a template's extra
/// in another order than the program did (which the painter does whenever
/// it paints cells in a different order) would copy it where the program
/// shared it, until the storage is full and the rest of the cells lose
/// their attributes. Both happened before this existed.
///
/// The table must not keep alive what no cell or cursor owns any more: an
/// extra it held after its cells were gone would stay charged for the rest
/// of the restore, and a review found a terminal at 57% of the limit
/// restoring with 30,285 cells stripped, because the table had kept every
/// template the stream passed through. So an extra only the table holds is
/// dropped: the template's previous extra as soon as the template moves on
/// (the common case, checked each time), and any other whenever the table
/// has doubled since its last sweep, or the storage is close to full, and
/// once more after the stream. Links stay in their table for the whole
/// restore; the encoder never sets a link it prints nothing with, so only a
/// forged stream can make one outlive its cells, within the same limit.
///
/// Links are told apart by content: one table holds each distinct `id` and
/// URI once, and extras are keyed by the link's index in it, so that a key
/// costs a few bytes rather than a copy of a URI, and the template of a
/// stream that keeps one link costs a pointer comparison per change rather
/// than hashing the URI again.
pub(super) struct Extras {
    storage: ExtraStorage,
    /// Every link seen, by index; `link_hashes` finds one by content.
    links: Vec<Hyperlink>,
    link_hashes: HashMap<u64, Vec<u32>>,
    /// The link found last, which the next lookup usually asks for again.
    last_link: Option<(Hyperlink, u32)>,
    /// One extra for each distinct content, by [`extra_key`]. Held strongly,
    /// so that an extra in it is shared and never changed in place (a cell
    /// changing it copies it), which keeps every key true to its extra.
    shared: HashMap<Vec<u8>, Arc<CellExtra>>,
    /// The key of the template extra [`Extras::share_template`] left in
    /// place last.
    template_key: Option<Vec<u8>>,
    /// How many entries `shared` had after its last sweep.
    swept_at: usize,
}

/// How much room the extra storage must have left before a share sweeps the
/// table for extras only it holds.
const SWEEP_ROOM: usize = 1 << 20;
/// How many entries the table must have gained since its last sweep before
/// a share sweeps it for want of room.
const SWEEP_GROWTH: usize = 4096;

impl Extras {
    pub(super) fn new(storage: ExtraStorage) -> Extras {
        Extras {
            storage,
            links: Vec::new(),
            link_hashes: HashMap::new(),
            last_link: None,
            shared: HashMap::new(),
            template_key: None,
            swept_at: 0,
        }
    }

    /// Drop every extra only the table holds (see the type docs). Called after
    /// the stream, before the fixups, and by [`Extras::share_template`].
    pub(super) fn sweep(&mut self) {
        self.shared.retain(|_, extra| Arc::strong_count(extra) > 1);
        self.swept_at = self.shared.len();
    }

    /// Replace `template`'s extra with the shared one of the same content,
    /// or make it the shared one: after a stream sequence that may have given
    /// the cursor template a new extra (SGR, OSC 8), so that the cells
    /// printed from it share whatever extra earlier cells of that content
    /// got.
    pub(super) fn share_template(&mut self, template: &mut Cell) {
        // The template's previous extra: nothing to do while the template
        // still holds it, and dropped when only the table does.
        if let Some(key) = self.template_key.take()
            && let Some(previous) = self.shared.get(&key)
        {
            if template
                .extra
                .as_ref()
                .is_some_and(|extra| Arc::ptr_eq(extra, previous))
            {
                self.template_key = Some(key);
                return;
            }
            if Arc::strong_count(previous) == 1 {
                self.shared.remove(&key);
            }
        }
        // Each sweep costs the whole table, so it waits for the table to
        // double, or near the limit for a few thousand more entries.
        let grown = self.shared.len().saturating_sub(self.swept_at);
        if self.shared.len() > 2 * self.swept_at.max(512)
            || (grown >= SWEEP_GROWTH && !self.storage.has_room(SWEEP_ROOM))
        {
            self.sweep();
        }
        let Some(extra) = &template.extra else {
            return;
        };
        let zerowidth = template.zerowidth().unwrap_or(&[]).to_vec();
        let underline = template.underline_color();
        let link = template.hyperlink();
        if zerowidth.is_empty() && underline.is_none() && link.is_none() {
            // An extra emptied by a change looks the same as none.
            template.extra = None;
            return;
        }
        let link = link.map(|link| self.link_index(link));
        let key = extra_key(&zerowidth, underline, link);
        match self.shared.get(&key) {
            Some(shared) => template.extra = Some(shared.clone()),
            None => {
                self.shared.insert(key.clone(), extra.clone());
            }
        }
        self.template_key = Some(key);
    }

    /// The index of `link` in the link table, adding it if no link of that
    /// content is there yet. `link` is a live one, already charged.
    fn link_index(&mut self, link: Hyperlink) -> u32 {
        if let Some((last, index)) = &self.last_link
            && *last == link
        {
            return *index;
        }
        let hash = link_hash(link.id(), link.uri());
        let index = match self.find_link(hash, link.id(), link.uri()) {
            Some(index) => index,
            None => self.add_link(hash, link.clone()),
        };
        self.last_link = Some((link, index));
        index
    }

    /// The table's link of this content, or a new one made through the
    /// storage; `None` when the storage has no room for it.
    fn decoded_link(&mut self, id: String, uri: String) -> Option<(Hyperlink, u32)> {
        let hash = link_hash(&id, &uri);
        if let Some(index) = self.find_link(hash, &id, &uri) {
            return Some((self.links[index as usize].clone(), index));
        }
        let link = self.storage.link(VteHyperlink { id: Some(id), uri })?;
        let index = self.add_link(hash, link.clone());
        Some((link, index))
    }

    fn find_link(&self, hash: u64, id: &str, uri: &str) -> Option<u32> {
        self.link_hashes.get(&hash)?.iter().copied().find(|&index| {
            let link = &self.links[index as usize];
            link.id() == id && link.uri() == uri
        })
    }

    fn add_link(&mut self, hash: u64, link: Hyperlink) -> u32 {
        let index = u32::try_from(self.links.len()).expect("fewer links than a blob has bytes");
        self.links.push(link);
        self.link_hashes.entry(hash).or_default().push(index);
        index
    }
}

/// A hash of a link's content, for finding it in [`Extras`]' link table.
fn link_hash(id: &str, uri: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    (id, uri).hash(&mut hasher);
    hasher.finish()
}

/// What tells two extras apart: their combining characters, underline color
/// and link (its index in [`Extras`]' link table), in a few bytes.
fn extra_key(zerowidth: &[char], underline: Option<Color>, link: Option<u32>) -> Vec<u8> {
    let mut key = Writer::default();
    key.u8(zerowidth.len() as u8);
    for &c in zerowidth {
        key.char(c);
    }
    match underline {
        None => key.u8(0),
        Some(color) => {
            key.u8(1);
            key.color(color);
        }
    }
    match link {
        None => key.u8(0),
        Some(index) => {
            key.u8(1);
            key.u32(index);
        }
    }
    key.buf
}

// ---------------------------------------------------------------------------
// Version 1's own limits on a fixup cell.
//
// These are part of the format and never change: a blob written today must
// read the same way in every later build, whatever that build's emulator
// does. They equal the vendored caps of the time version 1 was fixed
// (`alacritty_terminal::term::cell::MAX_ZEROWIDTH_CHARS`,
// `alacritty_terminal::term::MAX_HYPERLINK_ID_BYTES` and
// `MAX_HYPERLINK_URI_BYTES`), but the reader goes by these, not by the
// emulator's: lowering an emulator cap later must not make old blobs
// malformed. The encoder writes cells as the emulator holds them, so it
// stays within these only while the emulator's caps do; the assertions
// below stop the build if a cap is ever raised past version 1's, which then
// needs either a cut in the encoder or a new version.

/// The most combining characters a version 1 cell carries.
pub(super) const V1_MAX_ZEROWIDTH: usize = 16;
/// The longest hyperlink `id` a version 1 cell carries, in bytes.
pub(super) const V1_MAX_LINK_ID_BYTES: usize = 250;
/// The longest hyperlink URI a version 1 cell carries, in bytes.
pub(super) const V1_MAX_LINK_URI_BYTES: usize = 2083;

const _: () = assert!(
    MAX_ZEROWIDTH_CHARS <= V1_MAX_ZEROWIDTH,
    "the emulator keeps more combining characters per cell than version 1 carries"
);
const _: () = assert!(
    MAX_HYPERLINK_ID_BYTES <= V1_MAX_LINK_ID_BYTES,
    "the emulator keeps longer hyperlink ids than version 1 carries"
);
const _: () = assert!(
    MAX_HYPERLINK_URI_BYTES <= V1_MAX_LINK_URI_BYTES,
    "the emulator keeps longer hyperlink URIs than version 1 carries"
);

/// The most `;` a URI that came through OSC 8 holds: vte keeps 16 parameters
/// (`MAX_OSC_PARAMS`) and drops the rest of the sequence, and a link's URI is
/// what follows its first two (`8` and the link's parameters), joined again
/// with `;`.
const MAX_URI_SEMICOLONS: usize = 13;

/// Refuse a fixup's link that OSC 8 could not have given a cell: one that
/// live output cannot make cannot be in a blob the encoder wrote, and a
/// longer one would restore state outside what the caps allow. An `id`
/// with `:` or `;` cannot pass OSC 8's parameter syntax; neither field ever
/// holds a C0 control (the parser drops them inside an OSC, and a stream
/// that wrote one would be refused); a URI is never empty (an empty one
/// ends the link) and never holds more than [`MAX_URI_SEMICOLONS`] `;`; and
/// both are within version 1's length limits ([`V1_MAX_LINK_ID_BYTES`],
/// [`V1_MAX_LINK_URI_BYTES`]), which generated `id`s (`<n>_alacritty`) are
/// too.
fn check_link(id: &str, uri: &str) -> Result<(), SnapshotError> {
    let control = |c: char| (c as u32) < 0x20;
    if uri.is_empty()
        || uri.len() > V1_MAX_LINK_URI_BYTES
        || id.len() > V1_MAX_LINK_ID_BYTES
        || id.contains([':', ';'])
        || id.chars().any(control)
        || uri.chars().any(control)
        || uri.matches(';').count() > MAX_URI_SEMICOLONS
    {
        return Err(malformed("a hyperlink that OSC 8 cannot carry"));
    }
    Ok(())
}

/// The four character set slots, in version 1's order.
const CHARSET_INDEXES: [CharsetIndex; 4] = [
    CharsetIndex::G0,
    CharsetIndex::G1,
    CharsetIndex::G2,
    CharsetIndex::G3,
];

/// Reads a blob, refusing anything out of place as malformed.
pub(super) struct Reader<'a> {
    data: &'a [u8],
}

/// A malformed-blob error naming what was wrong.
pub(super) fn malformed(what: impl Into<String>) -> SnapshotError {
    SnapshotError::Malformed(what.into())
}

impl<'a> Reader<'a> {
    pub(super) fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data }
    }

    /// Whether everything has been read.
    pub(super) fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], SnapshotError> {
        if self.data.len() < len {
            return Err(malformed("the blob ends early"));
        }
        let (head, rest) = self.data.split_at(len);
        self.data = rest;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], SnapshotError> {
        Ok(self.take(N)?.try_into().expect("take returned N bytes"))
    }

    pub(super) fn u8(&mut self) -> Result<u8, SnapshotError> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16, SnapshotError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    pub(super) fn u32(&mut self) -> Result<u32, SnapshotError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub(super) fn u64(&mut self) -> Result<u64, SnapshotError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    pub(super) fn i32(&mut self) -> Result<i32, SnapshotError> {
        Ok(i32::from_be_bytes(self.array()?))
    }

    pub(super) fn len(&mut self) -> Result<usize, SnapshotError> {
        Ok(self.u32()? as usize)
    }

    /// A count of records of at least `min_record_bytes` each, refused when
    /// the rest of the blob cannot hold that many, so that a corrupt count
    /// cannot make the reader reserve memory the blob never backs.
    pub(super) fn count(&mut self, min_record_bytes: usize) -> Result<usize, SnapshotError> {
        let count = self.len()?;
        if count.saturating_mul(min_record_bytes) > self.data.len() {
            return Err(malformed("a count larger than the blob"));
        }
        Ok(count)
    }

    pub(super) fn bytes(&mut self) -> Result<&'a [u8], SnapshotError> {
        let len = self.len()?;
        self.take(len)
    }

    fn string(&mut self) -> Result<String, SnapshotError> {
        let bytes = self.bytes()?;
        String::from_utf8(bytes.to_vec()).map_err(|_| malformed("a string that is not UTF-8"))
    }

    pub(super) fn bool(&mut self) -> Result<bool, SnapshotError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(malformed(format!("{other} where a boolean belongs"))),
        }
    }

    pub(super) fn char(&mut self) -> Result<char, SnapshotError> {
        let value = self.u32()?;
        char::from_u32(value).ok_or_else(|| malformed(format!("{value:#x} is not a character")))
    }

    pub(super) fn color(&mut self) -> Result<Color, SnapshotError> {
        match self.u8()? {
            0 => {
                let slot = self.u16()?;
                NAMED_COLORS
                    .iter()
                    .find(|&&(_, known)| known == slot)
                    .map(|&(named, _)| Color::Named(named))
                    .ok_or_else(|| malformed(format!("named color {slot}")))
            }
            1 => Ok(Color::Indexed(self.u8()?)),
            2 => Ok(Color::Spec(Rgb {
                r: self.u8()?,
                g: self.u8()?,
                b: self.u8()?,
            })),
            other => Err(malformed(format!("color kind {other}"))),
        }
    }

    /// A cell. Its extra (combining characters, underline color, hyperlink)
    /// is made through `extras`, which charges it to the terminal's extra
    /// storage like the program's own, within the same limit, and shares one
    /// extra among cells whose extras hold the same. A cell no live output can
    /// make is refused: more combining characters than a cell keeps, or a
    /// link OSC 8 cannot carry ([`check_link`]).
    pub(super) fn cell(&mut self, extras: &mut Extras) -> Result<Cell, SnapshotError> {
        let mut cell = Cell {
            c: self.char()?,
            fg: self.color()?,
            bg: self.color()?,
            flags: bits_to_flags(self.u16()?)?,
            ..Cell::default()
        };
        let count = self.u8()? as usize;
        if count > V1_MAX_ZEROWIDTH {
            return Err(malformed(format!("{count} combining characters on a cell")));
        }
        let mut zerowidth = Vec::with_capacity(count);
        for _ in 0..count {
            zerowidth.push(self.char()?);
        }
        let underline = if self.bool()? {
            Some(self.color()?)
        } else {
            None
        };
        let link = if self.bool()? {
            let (id, uri) = (self.string()?, self.string()?);
            check_link(&id, &uri)?;
            Some((id, uri))
        } else {
            None
        };
        if zerowidth.is_empty() && underline.is_none() && link.is_none() {
            return Ok(cell);
        }
        // A link the storage has no room for is left off, like an OSC 8 then;
        // the cell is not shared, since its extra is not what was asked for.
        let (link, link_index, complete) = match link {
            None => (None, None, true),
            Some((id, uri)) => match extras.decoded_link(id, uri) {
                Some((link, index)) => (Some(link), Some(index), true),
                None => (None, None, false),
            },
        };
        let key = extra_key(&zerowidth, underline, link_index);
        if complete && let Some(shared) = extras.shared.get(&key) {
            cell.extra = Some(shared.clone());
            return Ok(cell);
        }
        let storage = &extras.storage;
        for &c in &zerowidth {
            storage.push_zerowidth(&mut cell, c);
        }
        storage.set_underline_color(&mut cell, underline);
        storage.set_hyperlink(&mut cell, link.clone());
        // Shared only if the storage had room for all of it.
        let made = cell.zerowidth().unwrap_or(&[]) == zerowidth.as_slice()
            && cell.underline_color() == underline
            && cell.hyperlink() == link;
        if complete
            && made
            && let Some(extra) = &cell.extra
        {
            extras.shared.insert(key, extra.clone());
        }
        Ok(cell)
    }

    /// A cursor; its position is checked by the caller, which knows the size.
    pub(super) fn cursor(&mut self, extras: &mut Extras) -> Result<Cursor<Cell>, SnapshotError> {
        let line = self.i32()?;
        let column = self.len()?;
        let input_needs_wrap = self.bool()?;
        let template = self.cell(extras)?;
        // Combining characters go to the cell before the cursor, never to
        // its template.
        if template
            .zerowidth()
            .is_some_and(|zerowidth| !zerowidth.is_empty())
        {
            return Err(malformed("a cursor template with combining characters"));
        }
        let mut cursor = Cursor {
            point: Point::new(Line(line), Column(column)),
            template,
            input_needs_wrap,
            ..Cursor::default()
        };
        for index in CHARSET_INDEXES {
            cursor.charsets[index] = match self.u8()? {
                0 => StandardCharset::Ascii,
                1 => StandardCharset::SpecialCharacterAndLineDrawing,
                other => return Err(malformed(format!("character set {other}"))),
            };
        }
        Ok(cursor)
    }
}

/// The color slot version 1 writes for `named` (see [`NAMED_COLORS`]).
fn named_slot(named: NamedColor) -> u16 {
    NAMED_COLORS
        .iter()
        .find(|&&(known, _)| known == named)
        .map(|&(_, slot)| slot)
        .expect("NAMED_COLORS lists every named color")
}

fn flags_to_bits(flags: Flags) -> u16 {
    FLAG_BITS
        .iter()
        .filter(|(flag, _)| flags.contains(*flag))
        .fold(0, |bits, (_, bit)| bits | bit)
}

fn bits_to_flags(bits: u16) -> Result<Flags, SnapshotError> {
    let mut flags = Flags::empty();
    let mut known = 0;
    for &(flag, bit) in &FLAG_BITS {
        known |= bit;
        if bits & bit != 0 {
            flags |= flag;
        }
    }
    if bits & !known != 0 {
        return Err(malformed(format!(
            "unknown cell flags {:#x}",
            bits & !known
        )));
    }
    Ok(flags)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every alacritty cell flag that is not a composite has a version 1 bit,
    /// and the bits survive a round trip.
    ///
    /// Why: a flag missing from the table would be dropped silently from every
    /// fixup cell that has it. Spec: `FLAG_BITS`.
    #[test]
    fn every_flag_has_a_bit() {
        let all = FLAG_BITS
            .iter()
            .fold(Flags::empty(), |all, (flag, _)| all | *flag);
        assert_eq!(all, Flags::all());
        for (flag, bit) in FLAG_BITS {
            assert_eq!(flags_to_bits(flag), bit);
            assert_eq!(bits_to_flags(bit), Ok(flag));
        }
        assert!(bits_to_flags(1 << 15).is_err());
    }

    /// `cell_len` is exactly what `Writer::cell` writes, for cells with and
    /// without each kind of extra.
    ///
    /// Why: the size limit decides how much history fits from measured
    /// lengths; an inexact one makes the encoder paint again, or overshoot.
    #[test]
    fn cell_len_matches_the_writer() {
        let storage = ExtraStorage::default();
        let mut cells = vec![Cell::default()];
        let mut rich = Cell {
            c: '中',
            fg: Color::Indexed(3),
            bg: Color::Spec(Rgb { r: 1, g: 2, b: 3 }),
            flags: Flags::WIDE_CHAR | Flags::BOLD,
            ..Cell::default()
        };
        rich.push_zerowidth('\u{301}');
        rich.push_zerowidth('\u{308}');
        rich.set_underline_color(Some(Color::Named(NamedColor::Red)));
        rich.set_hyperlink(storage.link(VteHyperlink {
            id: Some("id".to_owned()),
            uri: "http://u".to_owned(),
        }));
        cells.push(rich);
        for cell in cells {
            let mut writer = Writer::default();
            writer.cell(&cell);
            assert_eq!(cell_len(&cell), writer.buf.len());
        }
    }

    /// Version 1's limits on a fixup cell equal the vendored caps today.
    ///
    /// Why: they were copied from those caps when version 1 was fixed, and
    /// the build already stops if a cap rises past them. A cap lowered later
    /// would not break anything (old blobs still read, by version 1's
    /// limits), but it means the emulator now refuses at runtime what the
    /// format carries, so it fails here to make that change deliberate:
    /// update this test, never the limits. Spec: the limits' comment above.
    #[test]
    fn version_1_s_limits_match_today_s_caps() {
        assert_eq!(V1_MAX_ZEROWIDTH, MAX_ZEROWIDTH_CHARS);
        assert_eq!(V1_MAX_LINK_ID_BYTES, MAX_HYPERLINK_ID_BYTES);
        assert_eq!(V1_MAX_LINK_URI_BYTES, MAX_HYPERLINK_URI_BYTES);
    }

    /// Every named color round-trips through its slot number.
    ///
    /// Why: the table is written by hand and a duplicate or missing slot would
    /// turn one color into another, or fail to decode, in every blob that
    /// uses it. Spec: `NAMED_COLORS`.
    #[test]
    fn every_named_color_round_trips() {
        for (named, _) in NAMED_COLORS {
            let mut writer = Writer::default();
            writer.color(Color::Named(named));
            let mut reader = Reader::new(&writer.buf);
            assert_eq!(reader.color(), Ok(Color::Named(named)));
            assert!(reader.is_empty());
        }
    }
}

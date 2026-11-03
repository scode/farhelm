// Modified for Farhelm: caps the zero-width characters stored per cell
// (`MAX_ZEROWIDTH_CHARS`), charges cell extras and hyperlinks to their
// terminal's extra storage (`ExtraStorage`), and leaves a cell's extra alone
// when a change to it changes nothing. See vendor/README.md in the Farhelm
// repository.

use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use bitflags::bitflags;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::grid::{self, GridCell};
use crate::index::Column;
use crate::vte::ansi::{Color, Hyperlink as VteHyperlink, NamedColor};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct Flags: u16 {
        const INVERSE                   = 0b0000_0000_0000_0001;
        const BOLD                      = 0b0000_0000_0000_0010;
        const ITALIC                    = 0b0000_0000_0000_0100;
        const BOLD_ITALIC               = 0b0000_0000_0000_0110;
        const UNDERLINE                 = 0b0000_0000_0000_1000;
        const WRAPLINE                  = 0b0000_0000_0001_0000;
        const WIDE_CHAR                 = 0b0000_0000_0010_0000;
        const WIDE_CHAR_SPACER          = 0b0000_0000_0100_0000;
        const DIM                       = 0b0000_0000_1000_0000;
        const DIM_BOLD                  = 0b0000_0000_1000_0010;
        const HIDDEN                    = 0b0000_0001_0000_0000;
        const STRIKEOUT                 = 0b0000_0010_0000_0000;
        const LEADING_WIDE_CHAR_SPACER  = 0b0000_0100_0000_0000;
        const DOUBLE_UNDERLINE          = 0b0000_1000_0000_0000;
        const UNDERCURL                 = 0b0001_0000_0000_0000;
        const DOTTED_UNDERLINE          = 0b0010_0000_0000_0000;
        const DASHED_UNDERLINE          = 0b0100_0000_0000_0000;
        const ALL_UNDERLINES            = Self::UNDERLINE.bits() | Self::DOUBLE_UNDERLINE.bits()
                                        | Self::UNDERCURL.bits() | Self::DOTTED_UNDERLINE.bits()
                                        | Self::DASHED_UNDERLINE.bits();
    }
}

/// Farhelm patch: the most zero-width (combining) characters one cell stores;
/// [`Cell::push_zerowidth`] drops any beyond it.
///
/// Upstream appends without limit, so a program printing one base character
/// followed by millions of combining marks grows a single cell by four bytes
/// per mark. Sixteen covers real text with room to spare: stacked diacritics
/// and Hangul jamo use two or three per cell, and a subdivision flag such as
/// Scotland's (a base emoji plus six tag characters) uses six. tmux, whose
/// screen Farhelm reads today, stores at most 32 bytes of UTF-8 per cell, which
/// is fewer marks than this in practice.
pub const MAX_ZEROWIDTH_CHARS: usize = 16;

/// Counter for hyperlinks without explicit ID.
static HYPERLINK_ID_SUFFIX: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Hyperlink {
    inner: Arc<HyperlinkInner>,
}

impl Hyperlink {
    pub fn new<T: ToString>(id: Option<T>, uri: String) -> Self {
        let inner = Arc::new(HyperlinkInner::new(id, uri));
        Self { inner }
    }

    pub fn id(&self) -> &str {
        &self.inner.id
    }

    pub fn uri(&self) -> &str {
        &self.inner.uri
    }
}

impl From<VteHyperlink> for Hyperlink {
    fn from(value: VteHyperlink) -> Self {
        Self::new(value.id, value.uri)
    }
}

impl From<Hyperlink> for VteHyperlink {
    fn from(val: Hyperlink) -> Self {
        VteHyperlink { id: Some(val.id().to_owned()), uri: val.uri().to_owned() }
    }
}

/// Farhelm patch: the extra storage of one terminal, in bytes, shared by the
/// cell extras and hyperlinks charged to it, and never more than
/// `MAX_EXTRA_STORAGE_BYTES`.
///
/// A hyperlink made through [`ExtraStorage::link`] is charged its `id` and URI
/// lengths plus `HYPERLINK_COST`; a cell extra (the allocation holding a
/// cell's combining characters, underline color and hyperlink) is charged
/// `CELL_EXTRA_COST`, and so is every copy made of a shared extra when one
/// cell's attributes change. Each charge is given back when what it was
/// charged for is dropped, so what is used is what the terminal's live extras
/// cost.
///
/// The bound is hard: every change that would allocate a new extra goes
/// through this type's setters, which check for room first. A change there is
/// no room for degrades the cell instead: an added combining character or
/// underline color is dropped, and an attribute removed from an extra other
/// cells share (which would need a copy) takes the cell's whole extra with it,
/// so the cell loses its other extras too. A program cannot make the storage
/// grow past the limit, whatever it saves and restores.
///
/// Extras and links made by upstream's own constructors are not charged to
/// anything; the terminal never makes one.
#[derive(Debug, Default, Clone)]
pub struct ExtraStorage {
    used: Arc<AtomicUsize>,
}

impl ExtraStorage {
    /// The bytes charged to this storage by extras and links still alive.
    pub fn used(&self) -> usize {
        self.used.load(Ordering::Relaxed)
    }

    /// Whether `bytes` more fit under `MAX_EXTRA_STORAGE_BYTES`.
    pub fn has_room(&self, bytes: usize) -> bool {
        self.used().saturating_add(bytes) <= crate::term::MAX_EXTRA_STORAGE_BYTES
    }

    /// A new link charged to this storage, or `None` if it and the cell extra
    /// that will carry it would take the storage past
    /// `MAX_EXTRA_STORAGE_BYTES`.
    pub fn link(&self, link: VteHyperlink) -> Option<Hyperlink> {
        let mut inner = HyperlinkInner::new(link.id, link.uri);
        let bytes = inner.id.len() + inner.uri.len() + crate::term::HYPERLINK_COST;
        if !self.has_room(bytes + crate::term::CELL_EXTRA_COST) {
            return None;
        }
        inner.charge = Some(self.charge(bytes));
        Some(Hyperlink { inner: Arc::new(inner) })
    }

    /// Add `character` to `cell`'s combining characters, or drop it if that
    /// needs a new extra and there is no room for one.
    pub fn push_zerowidth(&self, cell: &mut Cell, character: char) {
        if !Self::change_allocates(cell) || self.has_room(crate::term::CELL_EXTRA_COST) {
            cell.push_zerowidth(character);
            self.adopt(cell);
        }
    }

    /// Set `cell`'s underline color. Without room for the extra the change
    /// needs, the color falls back to the default; and when that removal
    /// would itself need a copy of a shared extra, the cell's whole extra is
    /// dropped (see the type docs).
    pub fn set_underline_color(&self, cell: &mut Cell, color: Option<Color>) {
        if cell.underline_color() == color {
            return;
        }
        let drops = color.is_none()
            && cell.extra.as_ref().is_none_or(|extra| {
                extra.zerowidth.is_empty() && extra.hyperlink.is_none()
            });
        if drops || !Self::change_allocates(cell) || self.has_room(crate::term::CELL_EXTRA_COST)
        {
            cell.set_underline_color(color);
            self.adopt(cell);
        } else {
            cell.extra = None;
        }
    }

    /// Set `cell`'s hyperlink (made with [`ExtraStorage::link`], or `None`).
    /// Without room for the extra the change needs, the cell's whole extra
    /// is dropped, the link with it (see the type docs).
    pub fn set_hyperlink(&self, cell: &mut Cell, hyperlink: Option<Hyperlink>) {
        if cell.hyperlink() == hyperlink {
            return;
        }
        let drops = hyperlink.is_none()
            && cell.extra.as_ref().is_none_or(|extra| {
                extra.zerowidth.is_empty() && extra.underline_color.is_none()
            });
        if drops || !Self::change_allocates(cell) || self.has_room(crate::term::CELL_EXTRA_COST)
        {
            cell.set_hyperlink(hyperlink);
            self.adopt(cell);
        } else {
            cell.extra = None;
        }
    }

    /// [`Cell::clear_wide`], dropping the cell's whole extra when clearing its
    /// combining characters would need a copy of a shared extra and there is
    /// no room for one.
    pub fn clear_wide(&self, cell: &mut Cell) {
        let clears = cell.zerowidth().is_some_and(|zerowidth| !zerowidth.is_empty());
        if clears && Self::change_allocates(cell) && !self.has_room(crate::term::CELL_EXTRA_COST) {
            cell.extra = None;
        }
        cell.clear_wide();
        self.adopt(cell);
    }

    /// Whether changing `cell`'s extra makes a new one: it has none, or
    /// shares it with other cells (and copies it on write).
    fn change_allocates(cell: &Cell) -> bool {
        cell.extra.as_ref().is_none_or(|extra| Arc::strong_count(extra) > 1)
    }

    /// Charge `cell`'s extra to this storage if nothing is charged for it yet
    /// and no other cell shares it: a fresh extra that one of [`Cell`]'s
    /// setters made, after its caller checked for room.
    fn adopt(&self, cell: &mut Cell) {
        if let Some(extra) = cell.extra.as_mut().and_then(Arc::get_mut) {
            if extra.charge.is_none() {
                extra.charge = Some(self.charge(crate::term::CELL_EXTRA_COST));
            }
        }
    }

    fn charge(&self, bytes: usize) -> StorageCharge {
        self.used.fetch_add(bytes, Ordering::Relaxed);
        StorageCharge { bytes, used: self.used.clone() }
    }
}

/// Farhelm patch: one extra's or link's charge to an [`ExtraStorage`], given
/// back on drop. A copy is charged again, to the same storage.
#[derive(Debug)]
struct StorageCharge {
    bytes: usize,
    used: Arc<AtomicUsize>,
}

impl Clone for StorageCharge {
    fn clone(&self) -> Self {
        self.used.fetch_add(self.bytes, Ordering::Relaxed);
        StorageCharge { bytes: self.bytes, used: self.used.clone() }
    }
}

impl Drop for StorageCharge {
    fn drop(&mut self) {
        self.used.fetch_sub(self.bytes, Ordering::Relaxed);
    }
}

// The charge is bookkeeping, not part of a link's or extra's identity: they
// stay equal (and hash alike) whatever they are charged to.
impl PartialEq for StorageCharge {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for StorageCharge {}

impl Hash for StorageCharge {
    fn hash<H: Hasher>(&self, _: &mut H) {}
}

#[derive(Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
struct HyperlinkInner {
    /// Identifier for the given hyperlink.
    id: String,

    /// Resource identifier of the hyperlink.
    uri: String,

    /// Farhelm patch: what this link is charged to, if anything.
    #[cfg_attr(feature = "serde", serde(skip))]
    charge: Option<StorageCharge>,
}

impl HyperlinkInner {
    pub fn new<T: ToString>(id: Option<T>, uri: String) -> Self {
        let id = match id {
            Some(id) => id.to_string(),
            None => {
                let mut id = HYPERLINK_ID_SUFFIX.fetch_add(1, Ordering::Relaxed).to_string();
                id.push_str("_alacritty");
                id
            },
        };

        Self { id, uri, charge: None }
    }
}

/// Trait for determining if a reset should be performed.
pub trait ResetDiscriminant<T> {
    /// Value based on which equality for the reset will be determined.
    fn discriminant(&self) -> T;
}

impl<T: Copy> ResetDiscriminant<T> for T {
    fn discriminant(&self) -> T {
        *self
    }
}

impl ResetDiscriminant<Color> for Cell {
    fn discriminant(&self) -> Color {
        self.bg
    }
}

/// Dynamically allocated cell content.
///
/// This storage is reserved for cell attributes which are rarely set. This allows reducing the
/// allocation required ahead of time for every cell, with some additional overhead when the extra
/// storage is actually required.
#[derive(Default, Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CellExtra {
    zerowidth: Vec<char>,
    underline_color: Option<Color>,
    hyperlink: Option<Hyperlink>,

    /// Farhelm patch: what this extra is charged to, if anything.
    #[cfg_attr(feature = "serde", serde(skip))]
    charge: Option<StorageCharge>,
}

/// Content and attributes of a single cell in the terminal grid.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Cell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub flags: Flags,
    pub extra: Option<Arc<CellExtra>>,
}

impl Default for Cell {
    #[inline]
    fn default() -> Cell {
        Cell {
            c: ' ',
            bg: Color::Named(NamedColor::Background),
            fg: Color::Named(NamedColor::Foreground),
            flags: Flags::empty(),
            extra: None,
        }
    }
}

impl Cell {
    /// Zerowidth characters stored in this cell.
    #[inline]
    pub fn zerowidth(&self) -> Option<&[char]> {
        self.extra.as_ref().map(|extra| extra.zerowidth.as_slice())
    }

    /// Write a new zerowidth character to this cell.
    #[inline]
    pub fn push_zerowidth(&mut self, character: char) {
        // Farhelm patch: see `MAX_ZEROWIDTH_CHARS`.
        if self.zerowidth().is_some_and(|zerowidth| zerowidth.len() >= MAX_ZEROWIDTH_CHARS) {
            return;
        }
        let extra = self.extra.get_or_insert(Default::default());
        Arc::make_mut(extra).zerowidth.push(character);
    }

    /// Remove all wide char data from a cell.
    #[inline(never)]
    pub fn clear_wide(&mut self) {
        self.flags.remove(Flags::WIDE_CHAR);
        // Farhelm patch: only with combining characters to clear, so that an
        // extra shared with other cells is not copied for nothing.
        if let Some(extra) = self.extra.as_mut().filter(|extra| !extra.zerowidth.is_empty()) {
            Arc::make_mut(extra).zerowidth = Vec::new();
        }
        self.c = ' ';
    }

    /// Set underline color on the cell.
    pub fn set_underline_color(&mut self, color: Option<Color>) {
        // If we reset color and we don't have zerowidth we should drop extra storage.
        if color.is_none()
            && self
                .extra
                .as_ref()
                .is_none_or(|extra| extra.zerowidth.is_empty() && extra.hyperlink.is_none())
        {
            self.extra = None;
        } else if self.underline_color() != color {
            // Farhelm patch: the `if`. Setting the color it has already would
            // copy an extra shared with other cells for nothing, once per
            // repeat.
            let extra = self.extra.get_or_insert(Default::default());
            Arc::make_mut(extra).underline_color = color;
        }
    }

    /// Underline color stored in this cell.
    #[inline]
    pub fn underline_color(&self) -> Option<Color> {
        self.extra.as_ref()?.underline_color
    }

    /// Set hyperlink.
    pub fn set_hyperlink(&mut self, hyperlink: Option<Hyperlink>) {
        let should_drop = hyperlink.is_none()
            && self
                .extra
                .as_ref()
                .is_none_or(|extra| extra.zerowidth.is_empty() && extra.underline_color.is_none());

        if should_drop {
            self.extra = None;
        } else if self.extra.as_ref().is_none_or(|extra| extra.hyperlink != hyperlink) {
            // Farhelm patch: the condition, for the reason in
            // `set_underline_color`.
            let extra = self.extra.get_or_insert(Default::default());
            Arc::make_mut(extra).hyperlink = hyperlink;
        }
    }

    /// Hyperlink stored in this cell.
    #[inline]
    pub fn hyperlink(&self) -> Option<Hyperlink> {
        self.extra.as_ref()?.hyperlink.clone()
    }
}

impl GridCell for Cell {
    #[inline]
    fn is_empty(&self) -> bool {
        (self.c == ' ' || self.c == '\t')
            && self.bg == Color::Named(NamedColor::Background)
            && self.fg == Color::Named(NamedColor::Foreground)
            && !self.flags.intersects(
                Flags::INVERSE
                    | Flags::ALL_UNDERLINES
                    | Flags::STRIKEOUT
                    | Flags::WRAPLINE
                    | Flags::WIDE_CHAR_SPACER
                    | Flags::LEADING_WIDE_CHAR_SPACER,
            )
            && self.extra.as_ref().map(|extra| extra.zerowidth.is_empty()) != Some(false)
    }

    #[inline]
    fn flags(&self) -> &Flags {
        &self.flags
    }

    #[inline]
    fn flags_mut(&mut self) -> &mut Flags {
        &mut self.flags
    }

    #[inline]
    fn reset(&mut self, template: &Self) {
        *self = Cell { bg: template.bg, ..Cell::default() };
    }
}

impl From<Color> for Cell {
    #[inline]
    fn from(color: Color) -> Self {
        Self { bg: color, ..Cell::default() }
    }
}

/// Get the length of occupied cells in a line.
pub trait LineLength {
    /// Calculate the occupied line length.
    fn line_length(&self) -> Column;
}

impl LineLength for grid::Row<Cell> {
    fn line_length(&self) -> Column {
        let mut length = Column(0);

        if self[Column(self.len() - 1)].flags.contains(Flags::WRAPLINE) {
            return Column(self.len());
        }

        for (index, cell) in self[..].iter().rev().enumerate() {
            if cell.c != ' '
                || cell.extra.as_ref().map(|extra| extra.zerowidth.is_empty()) == Some(false)
            {
                length = Column(self.len() - index);
                break;
            }
        }

        length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::mem;

    use crate::grid::Row;
    use crate::index::Column;

    #[test]
    fn cell_size_is_below_cap() {
        // Expected cell size on 64-bit architectures.
        const EXPECTED_CELL_SIZE: usize = 24;

        // Ensure that cell size isn't growing by accident.
        assert!(mem::size_of::<Cell>() <= EXPECTED_CELL_SIZE);
    }

    #[test]
    fn line_length_works() {
        let mut row = Row::<Cell>::new(10);
        row[Column(5)].c = 'a';

        assert_eq!(row.line_length(), Column(6));
    }

    #[test]
    fn line_length_works_with_wrapline() {
        let mut row = Row::<Cell>::new(10);
        row[Column(9)].flags.insert(super::Flags::WRAPLINE);

        assert_eq!(row.line_length(), Column(10));
    }
}

//! File paths a program printed, as terminal links: the old web client's detection in `terminal-files.js` at reference
//! commit 24bc0580 (`pathTokens`, `cutPathLink`, `pathLinks`, `softPathLinks` and `sameRange`), ported to the grid
//! model's rows. The old client's `oscTarget`, which also made a program's `file://` hyperlink a file link, is not
//! ported: every hyperlink that is not a web address is inert (SPEC.md, "Terminal experience").
//!
//! NOTE: This is recognition only. A path link grants nothing: the session's host decides what the path names (a
//! relative path is resolved against the session's directory, never a shell's later one), and the terminal must
//! show that host's answer in a hover before a click may transfer anything (SPEC.md, "Terminal experience").
//!
//! The rules, from SPEC.md: absolute and `~/` paths, and relative paths containing a slash or a file extension, count;
//! plain words, version numbers and scheme-bearing text do not, and file names with whitespace are not supported.
//! Diagnostic `:line:column` suffixes and trailing sentence punctuation stay outside the link. A path that a soft wrap
//! splits joins as one logical line. As a deliberate exception, a path a program cut with its own line break joins
//! across one isolated pair of rows when the continuation could not have fit on the head row (see
//! [`GridModel::cut_path_link`]).
//!
//! A program's hyperlink (OSC 8) owns its cells, whatever its scheme and whether or not it is concealed: a printed path
//! never includes one of them, and when a cut is joined, a hyperlink's space is never taken for the padding or
//! indentation the join skips. Otherwise plain text beside a hyperlink could borrow its label, as `/tmp/` followed by a
//! hyperlink labelled `report.pdf` would name `/tmp/report.pdf`, a file the program never printed as a path.
//!
//! Offsets here are byte offsets into the recognition text, each byte mapped to the cell that owns it. The old code
//! counted UTF-16 units; its 8192-character bounds are kept in UTF-16 units, so the same output reaches the same
//! limit.

use crate::{
    GridModel, LinkKind, Position, Selection, SelectionMode, TerminalLink, constant_regex,
    links::is_js_space,
};
use regex::Regex;
use std::sync::LazyLock;

/// The longest text, in UTF-16 units, that recognition reads: a logical line, or one row of a cut. Past it a line
/// has no path links at all, rather than a truncated token whose apparent start would change what counts as a path.
const MAX_TEXT_UNITS: usize = 8192;

/// How many rows past its first a soft-wrapped logical line may reach and still have path links: 129 rows in all,
/// as in the old client (its `> 128` checks). The walk back from the asked row to the line's start gives up at the
/// same distance, so a huge wrapped line is never read whole.
const MAX_WRAP_ROWS: u64 = 128;

/// The columns a TUI may leave free at the right of a row it breaks itself. A continuation that would have fit in the
/// head row with more than this to spare was not moved by a line break, so it is not joined.
const TUI_MARGIN: i32 = 4;

/// Version-like tokens (`1.2.3`, `v1.5x`, `1.2.3-rc.1+build.5`) that are never paths. Each dotted component starts with
/// a digit; hyphens belong only to the suffix, which is `+build`, `-pre` or `-pre+build`. The old client shaped the
/// pattern this way to avoid quadratic backtracking in the browser; the regex crate matches in linear time anyway, and
/// the shape is kept because it defines which tokens count as versions. `\d` and `\w` are spelled out as ASCII
/// classes because JavaScript's are ASCII and the regex crate's are Unicode.
static NUMERIC_VERSION: LazyLock<Regex> = LazyLock::new(|| {
    constant_regex(
        r"(?i)^[+-]?v?[0-9]+(?:\.[0-9][a-z0-9_]*)+(?:\+[0-9A-Z_a-z.-]+|-[0-9A-Z_a-z.-]*|-(?:[0-9A-Z_a-z-]|\.[0-9])*\+[0-9A-Z_a-z.-]+)?$",
    )
});
static SCHEME: LazyLock<Regex> = LazyLock::new(|| constant_regex(r"(?i)^[a-z][a-z0-9+.-]*:"));
static EXTENSION: LazyLock<Regex> =
    LazyLock::new(|| constant_regex(r"(?i)[^/.]+\.[a-z0-9][a-z0-9_-]*$"));
static LOCATION: LazyLock<Regex> = LazyLock::new(|| constant_regex(r":[0-9]+(?::[0-9]+)?$"));

/// Quotes and brackets end a path, so a file name containing one cannot be offered from the terminal.
const DELIMITERS: &[char] = &['"', '\'', '`', '<', '>', '(', ')', '{', '}', '[', ']'];
/// Sentence punctuation dropped from a path's end.
const SENTENCE_END: &[char] = &['.', ',', ';', '!', '?'];
/// What the old client stripped from a whole whitespace-delimited chunk's end before judging its scheme.
const CHUNK_END: &[char] = &[
    '.', ',', ';', '!', '?', '"', '\'', '`', '<', '>', '(', ')', '{', '}', '[', ']',
];

/// The C0 controls and DEL, which no path may contain.
fn is_control(ch: char) -> bool {
    ch <= '\u{1f}' || ch == '\u{7f}'
}

/// Text that names a URI rather than a file: a scheme, `://` anywhere, or a `//` network path.
fn scheme_bearing(text: &str) -> bool {
    SCHEME.is_match(text) || text.contains("://") || text.starts_with("//")
}

/// `text` without a trailing `:line` or `:line:column` suffix.
fn without_location(text: &str) -> &str {
    LOCATION
        .find(text)
        .map_or(text, |found| &text[..found.start()])
}

/// The maximal runs of `text` containing no `separator`, with their byte offsets.
fn runs(text: &str, separator: impl Fn(char) -> bool) -> Vec<(usize, &str)> {
    let mut found = Vec::new();
    let mut start = None;
    for (at, ch) in text.char_indices() {
        match (separator(ch), start) {
            (true, Some(from)) => {
                found.push((from, &text[from..at]));
                start = None;
            }
            (false, None) => start = Some(at),
            _ => {}
        }
    }
    if let Some(from) = start {
        found.push((from, &text[from..]));
    }
    found
}

/// What ends a path token: whitespace (JavaScript's, which the old pattern's `\s` was), quotes and brackets.
fn separates(ch: char) -> bool {
    is_js_space(ch) || DELIMITERS.contains(&ch)
}

/// The byte ranges of `text` that are paths, in order. A whole whitespace-delimited chunk that is scheme-bearing gives
/// nothing, before it is split at quotes and brackets: otherwise `javascript:alert(document.domain)` would donate
/// `document.domain` once its scheme-bearing prefix was split off.
fn path_tokens(text: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    for (chunk_at, chunk) in runs(text, is_js_space) {
        let unwrapped = without_location(
            chunk
                .trim_start_matches(DELIMITERS)
                .trim_end_matches(CHUNK_END),
        );
        if SCHEME.is_match(unwrapped) || chunk.contains("://") || unwrapped.starts_with("//") {
            continue;
        }
        for (piece_at, piece) in runs(chunk, |ch| DELIMITERS.contains(&ch)) {
            let path = without_location(piece.trim_end_matches(SENTENCE_END));
            // A slash covers absolute and `~/` paths as well as relative ones. A version is refused even though
            // it has a file-extension shape (`1.5x`); nothing with a slash matches the version pattern, so
            // `./1.5x` stays a path.
            if path.is_empty()
                || path.chars().any(is_control)
                || scheme_bearing(path)
                || NUMERIC_VERSION.is_match(path)
                || !(path.contains('/') || EXTENSION.is_match(path))
            {
                continue;
            }
            let start = chunk_at + piece_at;
            found.push((start, start + path.len()));
        }
    }
    found
}

/// The cell one byte of a recognition text came from, with the boundary after that cell.
#[derive(Clone, Copy)]
struct Owner {
    line: u64,
    col: u16,
    end: u16,
    /// Whether a program's hyperlink owns the cell ([`crate::GridRow::owns_cell`]). Such a byte is still
    /// read, so tokens split where they would on screen, but no path may include it and no cut may skip it.
    hyperlinked: bool,
}

/// Text read from rows for recognition, each byte mapped to its cell, with its length in UTF-16 units for the bounds.
#[derive(Default)]
struct CellText {
    text: String,
    owners: Vec<Owner>,
    units: usize,
}

impl CellText {
    fn push(&mut self, text: &str, owner: Owner) {
        self.text.push_str(text);
        self.owners.extend(std::iter::repeat_n(owner, text.len()));
        self.units += text.encode_utf16().count();
    }

    /// The length of the text without its trailing padding: the spaces after its last other character, stopping at
    /// a hyperlink's cell, which is the program's text and never padding even when it is a space.
    fn padded_end(&self) -> usize {
        let unpadded = self
            .text
            .bytes()
            .zip(&self.owners)
            .rposition(|(byte, owner)| byte != b' ' || owner.hyperlinked);
        unpadded.map_or(0, |at| at + 1)
    }

    /// The length of the text's indentation: its leading spaces, stopping at a hyperlink's cell for the same reason
    /// as [`CellText::padded_end`].
    fn indentation(&self) -> usize {
        self.text
            .bytes()
            .zip(&self.owners)
            .position(|(byte, owner)| byte != b' ' || owner.hyperlinked)
            .unwrap_or(self.text.len())
    }

    /// Whether a program's hyperlink owns any cell of bytes `start..end`, which no printed path may include.
    fn hyperlinked(&self, start: usize, end: usize) -> bool {
        self.owners
            .get(start..end)
            .is_some_and(|owners| owners.iter().any(|owner| owner.hyperlinked))
    }

    /// Append `other[from..]`, owners included.
    fn extend_from(&mut self, other: &CellText, from: usize) {
        let text = &other.text[from..];
        self.text.push_str(text);
        self.owners.extend_from_slice(&other.owners[from..]);
        self.units += text.encode_utf16().count();
    }

    /// The path at bytes `start..end` as a link: one range per row it reaches, covering exactly its own cells, so a
    /// cut path's padding and indentation are not part of it. Evidence is two rows either side, which a cut reads to
    /// rule out a chain; a missing row counts too, since history may still arrive there.
    ///
    /// This builds the link whoever owns the cells; callers check [`CellText::hyperlinked`] first, because a cut
    /// must still count as a possible cut when it is refused for ownership.
    fn link(&self, start: usize, end: usize) -> Option<TerminalLink> {
        let mut ranges: Vec<Selection> = Vec::new();
        for owner in self.owners.get(start..end)? {
            match ranges.last_mut() {
                Some(range) if range.end.line == owner.line => {
                    range.end.col = range.end.col.max(owner.end);
                }
                _ => ranges.push(Selection::new(
                    Position {
                        line: owner.line,
                        col: owner.col,
                    },
                    Position {
                        line: owner.line,
                        col: owner.end,
                    },
                    SelectionMode::Linear,
                )),
            }
        }
        let (first, last) = (ranges.first()?.start.line, ranges.last()?.end.line);
        Some(TerminalLink {
            kind: LinkKind::Path,
            uri: self.text.get(start..end)?.to_owned(),
            ranges,
            evidence: (first.saturating_sub(2)..=last.saturating_add(2)).collect(),
            label: String::new(),
            osc_label_ambiguous: false,
        })
    }
}

/// A link's first and last cell boundary, `None` without ranges.
fn span(link: &TerminalLink) -> Option<(Position, Position)> {
    Some((link.ranges.first()?.start, link.ranges.last()?.end))
}

impl TerminalLink {
    /// The file this link names on the session's host: a printed path, as printed. `None` for every other link,
    /// including a program's `file://` hyperlink, which is inert like every hyperlink that is not a web address: its
    /// target is whatever the program sent, under a label the program chose, so it never names a file to download.
    pub fn file_path(&self) -> Option<String> {
        match &self.kind {
            LinkKind::Path => Some(self.uri.clone()),
            LinkKind::Osc { .. } | LinkKind::Text => None,
        }
    }

    /// Whether both links start and end at the same cell boundaries: the old client's `sameRange`, which a file
    /// hover's identity and its redraw check use. Looser than `==`, which also compares the rows between and the
    /// evidence. A link without ranges is never the same as anything, so identity fails closed.
    pub fn same_range(&self, other: &TerminalLink) -> bool {
        span(self).zip(span(other)).is_some_and(|(a, b)| a == b)
    }
}

impl GridModel {
    /// The printed paths touching `line`, each a [`LinkKind::Path`] link: those of its soft-wrapped logical line, and
    /// a program-cut path joining it with the row above or below. A cut replaces the two fragments it joins.
    ///
    /// No path includes a cell a program's hyperlink owns, of any scheme and concealed or not (see the module docs).
    /// Printed web addresses are not looked at here; they own their cells first in [`GridModel::path_link_at`].
    /// [`GridModel::row_links`] and [`GridModel::link_at`] never return paths.
    pub fn path_links(&self, line: u64) -> Vec<TerminalLink> {
        let cuts = [
            line.checked_sub(2),
            line.checked_sub(1),
            Some(line),
            line.checked_add(1),
        ]
        .map(|head| head.and_then(|head| self.cut_path_link(head)));
        // Rows that could each be cut are a chain, which stays under the ordinary rules: the exception must never
        // grow into a reconstruction of three rows. A cut over a hyperlink's cells still counts here; it is dropped
        // only after, and before it is weighed against the fragments, so it never displaces a valid one either.
        let joined: Vec<TerminalLink> = [1, 2]
            .into_iter()
            .filter(|&at| cuts[at - 1].is_none() && cuts[at + 1].is_none())
            .filter_map(|at| cuts[at].clone())
            .filter_map(|(cut, hyperlinked)| (!hyperlinked).then_some(cut))
            .collect();
        let overlaps = |link: &TerminalLink| {
            joined.iter().any(|cut| {
                span(link)
                    .zip(span(cut))
                    .is_some_and(|((start, end), (cut_start, cut_end))| {
                        start < cut_end && cut_start < end
                    })
            })
        };
        let mut found: Vec<TerminalLink> = self
            .soft_path_links(line)
            .into_iter()
            .filter(|link| !overlaps(link))
            .collect();
        found.extend(joined);
        found
    }

    /// The printed path covering `position`, unless a program hyperlink or a printed web address owns that cell:
    /// those come first, so a path never replaces a program's explicit target, even in concealed cells.
    pub fn path_link_at(&self, position: Position) -> Option<TerminalLink> {
        if self.has_osc_link_at(position) || self.link_at(position).is_some() {
            return None;
        }
        self.path_links(position.line)
            .into_iter()
            .find(|link| link.covers(position))
    }

    /// Whether shown line `number` soft-wraps onto the next; `false` for a line that is not shown.
    fn wraps(&self, number: u64) -> bool {
        self.selection_line(number)
            .is_some_and(|(_, row)| self.wraps_after(number, row))
    }

    /// Append `line`'s cells to `into`, a blank for each uncovered column and nothing for the emulator's unused
    /// wide-wrap pad, each marked with whether a program's hyperlink owns it; return its width, `None` when the line
    /// is not shown.
    fn read_cells(&self, line: u64, into: &mut CellText) -> Option<u16> {
        let (_, row) = self.selection_line(line)?;
        self.walk_cells(line, false, |col, width, text| {
            into.push(
                text,
                Owner {
                    line,
                    col,
                    end: col + width,
                    hyperlinked: row.owns_cell(col),
                },
            );
        })
    }

    /// Paths on `line`'s soft-wrapped logical line that touch `line`; none when that line spans more rows than
    /// [`MAX_WRAP_ROWS`] allows or holds more than [`MAX_TEXT_UNITS`] units.
    fn soft_path_links(&self, line: u64) -> Vec<TerminalLink> {
        let mut first = line;
        while first > 0 && self.wraps(first - 1) {
            if line - first > MAX_WRAP_ROWS {
                return Vec::new();
            }
            first -= 1;
        }
        let mut text = CellText::default();
        let mut at = first;
        while self.selection_line(at).is_some() && (at == first || self.wraps(at - 1)) {
            if at - first > MAX_WRAP_ROWS || text.units > MAX_TEXT_UNITS {
                return Vec::new();
            }
            self.read_cells(at, &mut text);
            at += 1;
        }
        if text.units > MAX_TEXT_UNITS {
            return Vec::new();
        }
        path_tokens(&text.text)
            .into_iter()
            .filter(|&(start, end)| !text.hyperlinked(start, end))
            .filter_map(|(start, end)| text.link(start, end))
            .filter(|link| {
                span(link).is_some_and(|(start, end)| start.line <= line && line <= end.line)
            })
            .collect()
    }

    /// A path a program cut with its own line break between `head` and the next row, joined: SPEC.md's deliberate
    /// exception to conservative recognition, for a TUI that breaks a long path while leaving columns for its right
    /// margin.
    ///
    /// Neither row may be part of a soft wrap. The head row, ignoring trailing padding, must end in a token that is
    /// itself a path (so a plain word above a path cannot donate a head), and the next row must start, after
    /// indentation, with a continuation that does not begin with punctuation. The continuation must not have fit in
    /// the head row's remaining columns with [`TUI_MARGIN`] to spare, counted in columns, not characters. The joined
    /// token is judged by the ordinary rules over the whole joined line, so a scheme-bearing head still cannot donate
    /// a file-looking suffix.
    ///
    /// The flag says whether a program's hyperlink owns any of the joined path's cells. Such a cut is never offered,
    /// but it is still returned: [`GridModel::path_links`] counts it as a possible cut when it looks for a chain, so
    /// that refusing it never lets the cut beside it join two rows of a three-row path.
    fn cut_path_link(&self, head: u64) -> Option<(TerminalLink, bool)> {
        let tail = head.checked_add(1)?;
        // The old client asked whether the row after the tail continues it, which a missing row never does.
        if self.has_screen_break_after(head)
            || head.checked_sub(1).is_some_and(|above| self.wraps(above))
            || self.wraps(head)
            || (self.wraps(tail) && self.selection_line(tail + 1).is_some())
        {
            return None;
        }
        let mut a = CellText::default();
        let head_cols = self.read_cells(head, &mut a)?;
        let mut b = CellText::default();
        self.read_cells(tail, &mut b)?;
        if a.units > MAX_TEXT_UNITS || b.units > MAX_TEXT_UNITS {
            return None;
        }
        // Padding and indentation stop at a hyperlink's cell: a space a hyperlink owns stays, and then separates.
        let end = a.padded_end();
        let (last_at, last) = runs(&a.text[..end], separates).pop()?;
        let indent = b.indentation();
        let rest = &b.text[indent..];
        let next_len = rest.find(separates).unwrap_or(rest.len());
        if last_at + last.len() != end
            || next_len == 0
            || rest.starts_with([',', ';', '!', '?', ':'])
            || !path_tokens(&a.text[..end])
                .iter()
                .any(|&(start, _)| start == last_at)
        {
            return None;
        }
        let remaining = i32::from(head_cols) - i32::from(a.owners.get(end - 1)?.end);
        let tail_columns = i32::from(b.owners.get(indent + next_len - 1)?.end)
            - i32::from(b.owners.get(indent)?.col);
        if remaining - TUI_MARGIN >= tail_columns {
            return None;
        }
        a.text.truncate(end);
        a.owners.truncate(end);
        a.units = a.text.encode_utf16().count();
        a.extend_from(&b, indent);
        if a.units > MAX_TEXT_UNITS {
            return None;
        }
        let (start, finish) = path_tokens(&a.text)
            .into_iter()
            .find(|&(start, finish)| start == last_at && finish > end)?;
        Some((a.link(start, finish)?, a.hyperlinked(start, finish)))
    }
}

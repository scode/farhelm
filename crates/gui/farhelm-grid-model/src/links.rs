//! Cell-owned terminal links, adapted from the old web client's grid-buffer.js (reference commit 24bc0580).
//! Search windows and generated OSC 8 identity rules match the MIT/xterm-derived
//! client (LICENSE-PORTED). Opening and disclosure admission belong to the UI.

use crate::{GridModel, Link, Position, Selection, SelectionMode, constant_regex};
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, LazyLock},
};
use url::Url;

// JavaScript \s differs from Unicode White_Space (notably BOM and NEL). Keep
// the detector's boundaries tied to its actual ECMAScript source.
pub(super) const JS_SPACE: &str =
    r"\x09-\x0D\x20\u{A0}\u{1680}\u{2000}-\u{200A}\u{2028}\u{2029}\u{202F}\u{205F}\u{3000}\u{FEFF}";
static URLS: LazyLock<Regex> = LazyLock::new(|| {
    constant_regex(
        &r#"(?:https?|HTTPS?):[/]{2}[^%s"'!*(){}|\\^<>`]*[^%s"':,.!?{}|\\^~\[\]`()<>]"#
            .replace("%s", JS_SPACE),
    )
});

/// OSC 8 owns its cells even when its URI scheme is inert; prose detection
/// must never replace a program's explicit target with its visible label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkKind {
    Osc { id: String },
    Text,
}

/// An immutable lookup result over absolute cell ranges. Evidence includes
/// neighboring rows whose edits can extend the target or change its label.
/// It grants no permission to open: callers revalidate before using a hover.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalLink {
    pub kind: LinkKind,
    pub uri: String,
    pub ranges: Vec<Selection>,
    pub evidence: BTreeSet<u64>,
}

impl TerminalLink {
    /// Half-open coverage includes wide-cell padding owned by a linked glyph.
    pub fn covers(&self, position: Position) -> bool {
        self.ranges
            .iter()
            .any(|range| range.start <= position && position < range.end)
    }
}

/// Only a parsed HTTP(S) target may reach the native browser opener. Plain
/// links additionally reject raw gaps/controls before parsing can erase them.
/// The returned serialization has a normalized scheme/host; file, data and
/// platform-specific URL handlers can never be dispatched through this API.
pub fn web_target(uri: &str, plain: bool) -> Option<Url> {
    if plain
        && (!uri
            .get(..7)
            .is_some_and(|s| s.eq_ignore_ascii_case("http://"))
            && !uri
                .get(..8)
                .is_some_and(|s| s.eq_ignore_ascii_case("https://"))
            || uri
                .chars()
                .any(|ch| is_js_space(ch) || ch <= '\u{1f}' || ch == '\u{7f}'))
    {
        return None;
    }
    let parsed = Url::parse(uri).ok()?;
    (matches!(parsed.scheme(), "http" | "https")
        && parsed.host_str().is_some_and(|host| !host.is_empty()))
    .then_some(parsed)
}

/// ECMAScript whitespace for the pinned policy, without locale or process state.
pub(super) fn is_js_space(ch: char) -> bool {
    matches!(ch, '\u{9}'..='\u{d}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

/// The addon's second boundary rejects host spellings the URL parser rewrites
/// (for example Unicode hosts and noncanonical numeric addresses in plain text).
fn host_prefixed(text: &str) -> bool {
    let Ok(url) = Url::parse(text) else {
        return false;
    };
    let auth = if !url.username().is_empty() {
        format!(
            "{}{}@",
            url.username(),
            match url.password() {
                Some(password) if !password.is_empty() => format!(":{password}"),
                _ => String::new(),
            }
        )
    } else {
        String::new()
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    text.to_lowercase()
        .starts_with(&format!("{}://{auth}{host}{port}", url.scheme()).to_lowercase())
}

/// Resolve numeric emulator-generated ids only through contiguous pieces of the
/// same URI. The per-question cache bounds repeated row lookbacks to linear work
/// and shares existing Arc<Link> values rather than cloning long target strings.
pub(super) struct Identities<'a> {
    model: &'a GridModel,
    rows: BTreeMap<u64, Vec<Option<Arc<Link>>>>,
}

impl<'a> Identities<'a> {
    pub(super) fn new(model: &'a GridModel) -> Self {
        Self {
            model,
            rows: BTreeMap::new(),
        }
    }

    /// Explicit ids never inherit an adjacent link merely because its URI agrees.
    fn joins(a: Option<&Arc<Link>>, b: Option<&Arc<Link>>) -> bool {
        let generated = |link: &Link| {
            link.id
                .strip_suffix("_alacritty")
                .is_some_and(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))
        };
        a.zip(b)
            .is_some_and(|(a, b)| generated(a) && generated(b) && a.uri == b.uri)
    }

    /// A soft-wrap seam can omit only the emulator's unused wide-cell pad.
    fn previous_piece(&self, line: u64) -> bool {
        let Some((_, row)) = self.model.selection_line(line) else {
            return false;
        };
        let Some(first) = row.runs.first().filter(|run| run.col == 0) else {
            return false;
        };
        let Some(previous) = line.checked_sub(1) else {
            return false;
        };
        let Some((cols, row)) = self
            .model
            .selection_line(previous)
            .filter(|(_, row)| row.wraps)
        else {
            return false;
        };
        let Some(last) = row.runs.last() else {
            return false;
        };
        Self::joins(last.link.as_ref(), first.link.as_ref())
            && (last.col + last.width == cols
                || (last.col + last.width == cols - 1
                    && self.model.pads_wide_wrap(previous, cols, row)))
    }

    /// A lookup follows at most 128 predecessor rows, exactly as the pinned
    /// identity resolver. Cache values contain no back-reference to grid rows.
    pub(super) fn row(&mut self, line: u64) -> Vec<Option<Arc<Link>>> {
        let mut pending = Vec::new();
        let mut at = line;
        for _ in 0..=128 {
            if self.rows.contains_key(&at) || self.model.selection_line(at).is_none() {
                break;
            }
            pending.push(at);
            if !self.previous_piece(at) {
                break;
            }
            let Some(previous) = at.checked_sub(1) else {
                break;
            };
            at = previous;
        }
        for at in pending.into_iter().rev() {
            // Only lines whose row exists were pushed above, so this never skips in practice.
            let Some((_, row)) = self.model.selection_line(at) else {
                continue;
            };
            let above = at
                .checked_sub(1)
                .and_then(|previous| self.rows.get(&previous))
                .filter(|_| self.previous_piece(at));
            let mut keys: Vec<Option<Arc<Link>>> = Vec::with_capacity(row.runs.len());
            for (index, run) in row.runs.iter().enumerate() {
                let before = index.checked_sub(1).and_then(|i| row.runs.get(i));
                let key = if before.is_some_and(|before| {
                    before.col + before.width == run.col
                        && Self::joins(before.link.as_ref(), run.link.as_ref())
                }) {
                    keys[index - 1].clone()
                } else if let Some(above) = above.filter(|_| index == 0) {
                    above.last().cloned().flatten()
                } else {
                    run.link.clone()
                };
                keys.push(key);
            }
            self.rows.insert(at, keys);
        }
        self.rows.get(&line).cloned().unwrap_or_default()
    }
}

impl GridModel {
    /// Every OSC 8 stretch on this row precedes plain URLs. Text detection walks
    /// only the pinned 2048-character neighboring window and preserves UTF-16
    /// ownership across soft wraps; even inert OSC targets keep their cells.
    pub fn row_links(&self, line: u64) -> Vec<TerminalLink> {
        let Some((_, row)) = self.selection_line(line) else {
            return Vec::new();
        };
        let mut identities = Identities::new(self);
        let keys = identities.row(line);
        let mut found: Vec<TerminalLink> = Vec::new();
        for (run, key) in row.runs.iter().zip(keys) {
            let Some(key) = key else { continue };
            if let Some(last) = found.last_mut().filter(|last| {
                matches!(&last.kind, LinkKind::Osc { id } if *id == key.id)
                    && last.uri == key.uri
                    && last.ranges[0].end.col == run.col
            }) {
                last.ranges[0].end.col = run.col + run.width;
                continue;
            }
            let mut first = line;
            let carries = |keys: Vec<Option<Arc<Link>>>| {
                keys.iter()
                    .flatten()
                    .any(|link| link.as_ref() == key.as_ref())
            };
            while line - first < 128
                && first
                    .checked_sub(1)
                    .is_some_and(|above| carries(identities.row(above)))
            {
                first -= 1;
            }
            let mut last = line;
            while last - line < 128
                && last
                    .checked_add(1)
                    .is_some_and(|below| carries(identities.row(below)))
            {
                last += 1;
            }
            found.push(TerminalLink {
                kind: LinkKind::Osc { id: key.id.clone() },
                uri: key.uri.clone(),
                ranges: vec![Selection::new(
                    Position { line, col: run.col },
                    Position {
                        line,
                        col: run.col + run.width,
                    },
                    SelectionMode::Linear,
                )],
                evidence: (first.saturating_sub(1)..=last.saturating_add(1)).collect(),
            });
        }
        let Some((first, last)) = self.url_window(line) else {
            return found;
        };
        let cells = self.word_cells(first, last);
        let text =
            String::from_utf16_lossy(&cells.iter().map(|cell| cell.unit).collect::<Vec<_>>());
        for matched in URLS.find_iter(&text) {
            let start = text[..matched.start()].encode_utf16().count();
            let end = start + matched.as_str().encode_utf16().count();
            let matched_cells = &cells[start..end];
            if !matched_cells.iter().any(|cell| cell.position.line == line)
                || !host_prefixed(matched.as_str())
            {
                continue;
            }
            // A regex match here is never empty, so it always has a first and last cell.
            let (Some(a), Some(b)) = (matched_cells.first(), matched_cells.last()) else {
                continue;
            };
            let mut ranges = Vec::new();
            for number in a.position.line..=b.position.line {
                let Some((cols, row)) = self.selection_line(number) else {
                    continue;
                };
                let from = if number == a.position.line {
                    a.position.col
                } else {
                    0
                };
                let to = if number == b.position.line {
                    b.position.col + b.width
                } else {
                    cols
                };
                if row
                    .runs
                    .iter()
                    .any(|run| run.link.is_some() && run.col < to && run.col + run.width > from)
                {
                    ranges.clear();
                    break;
                }
                ranges.push(Selection::new(
                    Position {
                        line: number,
                        col: from,
                    },
                    Position {
                        line: number,
                        col: to,
                    },
                    SelectionMode::Linear,
                ));
            }
            if !ranges.is_empty() {
                found.push(TerminalLink {
                    kind: LinkKind::Text,
                    uri: matched.as_str().into(),
                    ranges,
                    evidence: (first.saturating_sub(1)..=last.saturating_add(1)).collect(),
                });
            }
        }
        found
    }

    /// Absolute ranges never make an off-buffer or alternate-hidden row active.
    pub fn link_at(&self, position: Position) -> Option<TerminalLink> {
        self.row_links(position.line)
            .into_iter()
            .find(|link| link.covers(position))
    }

    /// Match the addon's neighboring-row search, whose budget counts text units,
    /// not rows or glyphs. A blank anywhere in a neighboring row ends that side.
    ///
    /// `None` when `line` has no row; `row_links` checks that before asking.
    fn url_window(&self, line: u64) -> Option<(u64, u64)> {
        let (cols, row) = self.selection_line(line)?;
        let mut first = line;
        let mut last = line;
        if !row.text(0, cols).starts_with(' ') {
            let mut count = 0;
            while let Some(above) = first.checked_sub(1) {
                let Some((cols, row)) = self.selection_line(above).filter(|(_, row)| row.wraps)
                else {
                    break;
                };
                if count >= 2048 {
                    break;
                }
                let text = row.text(0, cols);
                first = above;
                count += text.encode_utf16().count();
                if text.contains(' ') {
                    break;
                }
            }
        }
        let mut count = 0;
        loop {
            if count >= 2048 || !self.selection_line(last).is_some_and(|(_, row)| row.wraps) {
                break;
            }
            let Some(next) = last.checked_add(1) else {
                break;
            };
            let Some((cols, row)) = self.selection_line(next) else {
                break;
            };
            let text = row.text(0, cols);
            last = next;
            count += text.encode_utf16().count();
            if text.contains(' ') {
                break;
            }
        }
        Some((first, last))
    }
}

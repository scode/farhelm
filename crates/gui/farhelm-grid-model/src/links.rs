//! Cell-owned terminal links, adapted from the old web client's grid-buffer.js (reference commit 24bc0580).
//! Search windows and generated OSC 8 identity rules match the MIT/xterm-derived
//! client (LICENSE-PORTED). Opening and disclosure admission belong to the UI.

use crate::{
    GridModel, GridRow, GridRun, Link, Position, Selection, SelectionMode, constant_regex,
    style_flags,
};
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
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
    /// A program's OSC 8 hyperlink, with `uri` its target as sent: a web address, a `file://` target
    /// (`TerminalLink::file_path`), or an inert other scheme.
    Osc { id: String },
    /// A web address printed as text, with `uri` the matched text.
    Text,
    /// A file path a program printed (`crate::paths`), with `uri` the path as printed. It is never a URL: recognition
    /// refuses every scheme-bearing token, so [`web_target`] never accepts one, and only the session's host can say
    /// what file it names. [`GridModel::row_links`] and [`GridModel::link_at`] never return one;
    /// [`GridModel::path_links`] does.
    Path,
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
    /// A program hyperlink's visible text on its row (its runs' text, joined), so a relabel of the same cells and
    /// target makes it a different link: what the user agreed to under a resting pointer, or between a press and its
    /// release, includes the label the destination display judged. Only this row's text: SPEC.md lets the display skip
    /// later redraws of other rows, and a label spread over rows is judged whole by the display itself. Empty for a
    /// printed web address, whose text is its target (`uri`).
    pub label: String,
    /// Whether a multi-row OSC 8 label has another visible stretch with the same tag on this row. Revealing
    /// that stretch can change the disclosure to a warning even when the hovered stretch itself stays the same.
    pub osc_label_ambiguous: bool,
}

impl TerminalLink {
    /// Half-open coverage includes the second column of a linked two-column glyph.
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

/// The longest link, in UTF-8 bytes, that a tunnel is offered for: the helm's own cap on a URL it opens (its
/// "the URL is too long (maximum 4096 bytes)" refusal), so the offer never promises what the helm refuses.
const TUNNEL_URL_MAX: usize = 4096;

/// The port a link names when it points at a remote host's own loopback, which is what the Mac app offers to reach
/// through a tunnel (SPEC.md's localhost-tunnel paragraph); `None` for any other link.
///
/// The link must be a plain web address by [`web_target`]'s strict rule, whatever kind of link it is (the old app's
/// `tunnelTarget` held OSC 8 hyperlinks to it too), and at most 4096 bytes. Its host, as the URL parser normalizes it
/// (so `127.1` and `0x7f000001` are `127.0.0.1`, and a trailing dot is dropped), must be `localhost` or a name under
/// it, an address in `127.0.0.0/8`, `::1`, or the unspecified `0.0.0.0` or `::` that a server bound to all
/// addresses prints. LAN addresses, other single-label names and IPv4-mapped IPv6 addresses do not count. The port is
/// the explicit one, else the scheme's default (80, 443); port 0 is refused.
///
/// This only decides the offer. The helm classifies the link again when asked to open the tunnel and is the authority.
pub fn tunnel_port(uri: &str) -> Option<u16> {
    if uri.len() > TUNNEL_URL_MAX {
        return None;
    }
    let url = web_target(uri, true)?;
    let loopback = match url.host()? {
        url::Host::Domain(name) => {
            let name = name.trim_end_matches('.');
            name == "localhost" || name.ends_with(".localhost")
        }
        url::Host::Ipv4(address) => address.is_loopback() || address.is_unspecified(),
        url::Host::Ipv6(address) => address.is_loopback() || address.is_unspecified(),
    };
    let port = url.port_or_known_default()?;
    (loopback && port != 0).then_some(port)
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

/// Conceal removes a run from visible OSC 8 identity, including continuation across a row boundary.
/// Raw tags still hold their cells against printed-link fallback.
fn visible_link(run: &GridRun) -> Option<&Arc<Link>> {
    run.link
        .as_ref()
        .filter(|_| run.style.flags & style_flags::HIDDEN == 0)
}

/// Resolve numeric emulator-generated ids only through contiguous pieces of the
/// same URI that the terminal draws. The per-question cache bounds repeated row lookbacks to linear work
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

    /// Index of the preceding row's visible link at a soft-wrap seam. Padding and conceal break the seam.
    fn previous_piece(&self, line: u64) -> Option<usize> {
        let (_, row) = self.model.selection_line(line)?;
        let first = row.runs.first().filter(|run| run.col == 0)?;
        let previous = line.checked_sub(1)?;
        let (cols, row) = self
            .model
            .selection_line(previous)
            .filter(|(_, row)| self.model.wraps_after(previous, row))?;
        let text_end = row.text_columns(cols);
        let index = row
            .runs
            .partition_point(|run| run.col < text_end)
            .checked_sub(1)?;
        let last = &row.runs[index];
        (last.col + last.width >= text_end && Self::joins(visible_link(last), visible_link(first)))
            .then_some(index)
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
            if self.previous_piece(at).is_none() {
                break;
            }
            let Some(previous) = at.checked_sub(1) else {
                break;
            };
            at = previous;
        }
        for at in pending.into_iter().rev() {
            // Only lines whose row exists were pushed above, so this never skips in practice.
            let Some((cols, row)) = self.model.selection_line(at) else {
                continue;
            };
            let above = at
                .checked_sub(1)
                .and_then(|previous| self.rows.get(&previous))
                .and_then(|keys| keys.get(self.previous_piece(at)?));
            let text_end = row.text_columns(cols);
            let mut keys: Vec<Option<Arc<Link>>> = Vec::with_capacity(row.runs.len());
            for (index, run) in row.runs.iter().enumerate() {
                let Some(link) = visible_link(run).filter(|_| run.col < text_end) else {
                    keys.push(None);
                    continue;
                };
                let before = index.checked_sub(1).and_then(|i| row.runs.get(i));
                let key = if before.is_some_and(|before| {
                    before.col + before.width == run.col
                        && Self::joins(visible_link(before), Some(link))
                }) {
                    keys[index - 1].clone()
                } else if let Some(above) = above.filter(|_| index == 0) {
                    above.clone()
                } else {
                    Some(link.clone())
                };
                keys.push(key);
            }
            self.rows.insert(at, keys);
        }
        self.rows.get(&line).cloned().unwrap_or_default()
    }
}

impl GridRow {
    /// Any program link owning cells in this text interval. Concealed and inert links also own their cells,
    /// so neither a printed URL nor a pathname can take over.
    fn has_osc_link(&self, from: u16, to: u16) -> bool {
        self.runs
            .iter()
            .any(|run| run.link.is_some() && run.col < to && run.col + run.width > from)
    }
}

impl GridModel {
    /// Every visible OSC 8 stretch on this row precedes plain URLs. All OSC targets keep ownership of their cells.
    /// Text detection walks only the pinned 2048-character neighboring window and preserves UTF-16 ownership across
    /// soft wraps.
    /// Concealed cells break a printed URL: a plain click opens its address without showing it first, so it may only
    /// open what the terminal actually draws.
    pub fn row_links(&self, line: u64) -> Vec<TerminalLink> {
        let Some((cols, row)) = self.selection_line(line) else {
            return Vec::new();
        };
        let mut identities = Identities::new(self);
        let keys = identities.row(line);
        let mut found: Vec<TerminalLink> = Vec::new();
        let mut previous_stretch = HashMap::new();
        let text_end = row.text_columns(cols);
        for (run, key) in row.runs.iter().zip(&keys) {
            let Some(key) = key else { continue };
            let end = (run.col + run.width).min(text_end);
            let text = if end < run.col + run.width && run.is_plain_flow() {
                &run.text[..usize::from(end - run.col)]
            } else {
                &run.text
            };
            if let Some(last) = found.last_mut().filter(|last| {
                matches!(&last.kind, LinkKind::Osc { id } if *id == key.id)
                    && last.uri == key.uri
                    && last.ranges[0].end.col == run.col
            }) {
                last.ranges[0].end.col = end;
                last.label.push_str(text);
                continue;
            }
            let mut first = line;
            let carries = |keys: Vec<Option<Arc<Link>>>| {
                keys.iter()
                    .flatten()
                    .any(|link| link.as_ref() == key.as_ref())
            };
            while line - first < 128
                && first.checked_sub(1).is_some_and(|above| {
                    !self.has_screen_break_after(above) && carries(identities.row(above))
                })
            {
                first -= 1;
            }
            let mut last = line;
            while last - line < 128
                && !self.has_screen_break_after(last)
                && last
                    .checked_add(1)
                    .is_some_and(|below| carries(identities.row(below)))
            {
                last += 1;
            }
            // A lone row's stretches have separate disclosures. If the label crosses rows, a second visible
            // stretch here prevents reconstructing it, even when our own range and label did not change.
            let earlier = if first != last {
                previous_stretch.insert((key.id.as_str(), key.uri.as_str()), found.len())
            } else {
                None
            };
            if let Some(index) = earlier {
                found[index].osc_label_ambiguous = true;
            }
            found.push(TerminalLink {
                kind: LinkKind::Osc { id: key.id.clone() },
                uri: key.uri.clone(),
                ranges: vec![Selection::new(
                    Position { line, col: run.col },
                    Position { line, col: end },
                    SelectionMode::Linear,
                )],
                evidence: (first.saturating_sub(1)..=last.saturating_add(1)).collect(),
                label: text.to_owned(),
                osc_label_ambiguous: earlier.is_some(),
            });
        }
        let Some((first, last)) = self.url_window(line) else {
            return found;
        };
        let cells = self.word_cells(first, last, true);
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
                    row.text_columns(cols)
                };
                if from >= to {
                    continue;
                }
                if row.has_osc_link(from, to) {
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
                    label: String::new(),
                    osc_label_ambiguous: false,
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

    /// Raw OSC 8 ownership at a real cell, even when conceal keeps it out of `link_at`.
    /// Use the same visibility and padding boundaries as the link lookup.
    pub(crate) fn has_osc_link_at(&self, position: Position) -> bool {
        self.selection_line(position.line)
            .is_some_and(|(cols, row)| {
                position.col < row.text_columns(cols)
                    && row.has_osc_link(position.col, position.col + 1)
            })
    }

    /// Bound the neighboring search in UTF-16 units. A real blank anywhere in a neighboring row ends that side.
    /// Even a row with no text consumes one unit, so a run of padding-only rows cannot scan the whole history.
    ///
    /// `None` when `line` has no row; `row_links` checks that before asking.
    fn url_window(&self, line: u64) -> Option<(u64, u64)> {
        let (cols, row) = self.selection_line(line)?;
        let mut first = line;
        let mut last = line;
        if !row.text(0, row.text_columns(cols)).starts_with(' ') {
            let mut count = 0;
            while let Some(above) = first.checked_sub(1) {
                let Some((cols, row)) = self
                    .selection_line(above)
                    .filter(|(_, row)| self.wraps_after(above, row))
                else {
                    break;
                };
                if count >= 2048 {
                    break;
                }
                let text = row.text(0, row.text_columns(cols));
                first = above;
                count += text.encode_utf16().count().max(1);
                if text.contains(' ') {
                    break;
                }
            }
        }
        let mut count = 0;
        loop {
            if count >= 2048
                || !self
                    .selection_line(last)
                    .is_some_and(|(_, row)| self.wraps_after(last, row))
            {
                break;
            }
            let Some(next) = last.checked_add(1) else {
                break;
            };
            let Some((cols, row)) = self.selection_line(next) else {
                break;
            };
            let text = row.text(0, row.text_columns(cols));
            last = next;
            count += text.encode_utf16().count().max(1);
            if text.contains(' ') {
                break;
            }
        }
        Some((first, last))
    }
}

//! What a program hyperlink's hover says before a click opens it: the real destination, and a warning when the link's
//! visible text gives a reason to doubt it.
//!
//! Target disclosure is data, not browser authorization: native geometry and gesture state must still establish that
//! the current disclosure is visible before opening. The destination is disclosed in full, never shortened: a long one
//! hides its differing part in the omitted middle as easily as anywhere else, and the desktop app wraps it instead. The
//! label's and address's presentation (their escaping and shortening) ports terminal-links.js (LICENSE-PORTED); the
//! warning does not, and follows SPEC.md's terminal links paragraph.
//!
//! The warning has five reasons, in the order the hover lists them:
//!
//! - the text mentions a web address that names another place (mismatch), spells one with look-alike or invisible
//!   characters (disguised), or puts login details before the real site (credentials). [`crate::link_claims`] finds
//!   and judges the addresses;
//! - the whole text couldn't be read, so an address could hide in what wasn't (incomplete);
//! - part of the text is drawn where the user can't see it (hidden).
//!
//! The work splits in two. The **label reader** ([`GridModel::read_label`]) walks out from the hovered row over every
//! row the same hyperlink continues on, collects each row's visible stretches and the seam before it, and notes unseen
//! text with the same destination. The **verdict** ([`verdict`]) turns that into the warning. Unseen text never reaches
//! the label: identity already leaves it out ([`crate::links`]'s `visible_link`), exactly as conceal always did.
//!
//! A soft seam joins a hyperlink that reaches the row's last text column and continues at the start of the next.
//! The two rows draw like a wrap whether the terminal wrapped the text or the program moved to the next row itself.
//! The first reading separates a hard seam like a space when an address occurs before any letter in the segment
//! that follows. If that reading mismatches, the verdict tries joining every hard seam. Unseen text with the
//! link's destination at a seam keeps it separated in both readings.

use crate::link_claims::{self, MismatchKind, RowText, Seam};
use crate::links::{Identities, is_js_space};
use crate::{GridModel, LinkKind, TerminalLink, web_target};
use std::collections::VecDeque;
use std::ops::Range;
use url::Url;

/// Fully rendered text for one web target. Neither the host nor the target is ever shortened, and the host is never
/// replaced with userinfo. This owns no grid rows and cannot outlive the UI's independent evidence check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkDisclosure {
    /// Parsed authority host, including a nondefault port and IPv6 brackets.
    pub host: String,
    /// The destination in full: the URL parser's serialization, which is what a click opens, retaining empty
    /// query/fragment delimiters. It is ASCII (the parser percent-encodes and punycodes everything else), so a byte
    /// offset into it is also a character offset, which [`LinkDisclosure::difference`] relies on.
    pub target: String,
    /// Why the hover should make the user doubt the link, if it should. `None` is the ordinary display: the text
    /// mentions no other place, or nothing in it is out of sight.
    pub warning: Option<LinkWarning>,
}

/// The loud form of a hover: its reasons, and the text that lets the user see for themselves.
///
/// All text here is the program's, unescaped: the desktop app draws it through its host-text element, which escapes
/// invisible and direction-changing characters itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkWarning {
    /// At least one reason, each at most once, in [`WarningReason`]'s order. At most one of the address reasons
    /// (mismatch, disguised, credentials) appears: the first mismatching address's.
    pub reasons: Vec<WarningReason>,
    /// The visible label, every row of it, trimmed and shortened to the old client's budget. A seam that separates
    /// shows as a space, one that joins as nothing, and separate stretches of one row are spaced apart. Empty when the
    /// label was too long to read ([`WarningReason::Incomplete`] alone), so no part of an unread label is ever shown as
    /// if whole.
    pub label: String,
    /// The first address in the label that doesn't name the destination, as written (trailing decoration such as a
    /// sentence's full stop left off) and shortened. Present exactly when an address reason is.
    pub address: Option<String>,
    /// That address as the URL parser reads it, with the destination's scheme when it has none of its own: what the
    /// destination is compared against to show where they differ ([`LinkDisclosure::difference`]). `None` when it
    /// doesn't parse.
    pub address_parsed: Option<String>,
    /// How many other addresses in the label don't name the destination either.
    pub more: usize,
}

/// One reason a hover warns. The order of the variants is the order the hover lists them in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WarningReason {
    /// The text mentions a web address that names a different place, or one that doesn't parse.
    Mismatch,
    /// The text spells an address with look-alike, compatibility or invisible characters where a browser reads the
    /// place.
    Disguised,
    /// The text puts login details before the real host (`github.com@evil.test/`).
    Credentials,
    /// The whole text couldn't be checked: it is longer than the reader's budget, or a row of a multi-row label shows it
    /// in pieces that can't be put back together, and one of them mentions an address or unseen text with the link's
    /// destination is on its rows.
    Incomplete,
    /// Part of the text, with this link's destination, is concealed or drawn too close to its background to read.
    Hidden,
}

impl LinkDisclosure {
    /// The part of [`target`](Self::target) the hover marks as differing from what the link text says, as a byte range
    /// into it.
    ///
    /// The mark is the single middle of the target left once it and the first mismatching address's parsed form
    /// ([`LinkWarning::address_parsed`]) drop their common prefix and suffix. Only a plain mismatch is marked. A
    /// disguised address or one with login details names its place in a way the parsed form doesn't show (look-alike
    /// characters fold away, userinfo is not the host), so a mark there would point at the wrong part or at nothing;
    /// those reasons explain themselves instead, and the hover shows that address escaped. `None` also when the address
    /// doesn't parse, or when the target holds nothing the address lacks (`https://example.test/a/b` claimed against
    /// `https://example.test/a`): an empty mark would draw nothing a user could see.
    pub fn difference(&self) -> Option<Range<usize>> {
        let warning = self.warning.as_ref()?;
        if !warning.reasons.contains(&WarningReason::Mismatch) {
            return None;
        }
        differing_middle(warning.address_parsed.as_deref()?, &self.target)
    }
}

/// The target-side middle that remains once `claimed` and `target` drop their longest common prefix and then their
/// longest common suffix, as a byte range into `target`; `None` when it is empty.
///
/// One middle, not a diff: the user is shown where the two first and last disagree, which for a spoof is the host or
/// the path the label lied about, and anything finer (several marks, an edit script) would ask them to read an
/// algorithm's output. The suffix is taken only from what the prefix left of the shorter string, so the two never
/// overlap: `https://a.test/xx` against `https://a.test/xxx` marks the one extra `x`, not a range running backwards.
/// Both strings are URL serializations and so ASCII, but the comparison steps by character so a non-ASCII input
/// still yields a range on character boundaries.
fn differing_middle(claimed: &str, target: &str) -> Option<Range<usize>> {
    let prefix: usize = claimed
        .chars()
        .zip(target.chars())
        .take_while(|(a, b)| a == b)
        .map(|(_, b)| b.len_utf8())
        .sum();
    let room = claimed.len().min(target.len()) - prefix;
    let suffix: usize = claimed[prefix..]
        .chars()
        .rev()
        .zip(target[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(_, b)| b.len_utf8())
        .scan(0, |taken, len| {
            *taken += len;
            (*taken <= room).then_some(len)
        })
        .sum();
    let middle = prefix..target.len() - suffix;
    (!middle.is_empty()).then_some(middle)
}

impl From<MismatchKind> for WarningReason {
    fn from(kind: MismatchKind) -> Self {
        match kind {
            MismatchKind::Mismatch => Self::Mismatch,
            MismatchKind::Disguised => Self::Disguised,
            MismatchKind::Credentials => Self::Credentials,
        }
    }
}

/// Reveal invisible controls and escape literal notation so output cannot pose as our escape syntax. Variation
/// selectors remain readable, as in the pin.
///
/// This is the app's one escape (`farhelm_proto::text::escaped`), the table the command line shares. The pinned
/// JavaScript escaped Unicode's whole format category through a regular expression; the shared table lists that
/// category in full, so the oracle below still pins the same output. The model itself never escapes a disclosure (the
/// desktop app does, as it draws); the oracle tests use this to compare against the old client's escaped answers.
#[cfg(test)]
fn visible(text: &str) -> String {
    farhelm_proto::text::escaped(text)
}

/// Shorten the program's text a warning quotes (its label and the address it found), never the destination. The pin
/// budgets 300 UTF-16 units and names the omitted middle. A boundary inside a surrogate pair renders as replacement
/// text, just as a native text widget would render the JS slice's unpaired surrogate.
///
/// The quoted text is evidence for the reasons above it, and the destination below it is what the user decides on, so
/// only the destination has to be whole (SPEC.md: the hover shows the full destination).
fn shortened(text: &str) -> String {
    let units: Vec<_> = text.encode_utf16().collect();
    if units.len() <= 300 {
        return text.into();
    }
    format!(
        "{} …[{} characters]… {}",
        String::from_utf16_lossy(&units[..140]),
        units.len() - 280,
        String::from_utf16_lossy(&units[units.len() - 140..])
    )
}

// ---------------------------------------------------------------------------------------------------------------------
// The label reader
// ---------------------------------------------------------------------------------------------------------------------

/// The most rows a label may span and still be read: a label on more is incomplete. One identity lookup reaches 129
/// rows back ([`Identities`]), one more than this, so a hyperlink continued over more rows than the budget is over budget
/// from whichever row the pointer rests on, never read whole from one end and in part from the other.
const MAX_ROWS: usize = 128;

/// The most UTF-16 units of visible text a label may hold and still be read, the old client's join budget. Each of a
/// one-row label's independent stretches has a budget of its own.
const MAX_UNITS: usize = 8192;

/// One contiguous visible piece of the hovered hyperlink on one row.
struct Stretch {
    /// The column the piece starts at, which is how the hovered piece of a one-row label is found again.
    from: u16,
    text: String,
}

/// How a label row continues the one before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowSeam {
    /// The hyperlink's piece on the row before reaches that row's last text column and this row's piece starts at
    /// column 0 ([`GridModel::edge_seam`]): the rows read as one line. That is what a wrap draws, and also what a
    /// program draws when it fills a row and moves to the next itself; the wrap flag isn't consulted, because the user
    /// can't tell the two apart and reads `git` at the edge above `hub.com/login` as `github.com/login`.
    Soft,
    /// The program continued the hyperlink on a new row itself. It separates or joins depending on what follows.
    Hard,
    /// Like `Hard`, but unseen text with this destination lies between the two rows' pieces. It always separates: the
    /// user saw a gap there, so no reading of the label may stitch the pieces into one address.
    Break,
}

/// What one row holds of the hovered hyperlink.
struct LabelRow {
    stretches: Vec<Stretch>,
    /// UTF-16 units over all the row's stretches, for the budget.
    units: usize,
    /// Per run: whether it is a visible run of the hovered hyperlink. A soft seam needs it of the runs at the seam.
    keyed: Vec<bool>,
    /// Unseen text with this destination before the first stretch, and after the last one.
    hidden_before: bool,
    hidden_after: bool,
    /// Unseen text with this destination anywhere on the row: the hidden reason's evidence.
    hidden: bool,
    /// How this row continues the previous label row; `None` on the first.
    seam_before: Option<RowSeam>,
}

/// A hyperlink's label as the reader found it.
enum Label {
    /// Longer than the budget. Nothing read is evidence of anything, so the verdict is incomplete and shows no text.
    OverBudget,
    Read {
        /// Top to bottom. Every row has at least one stretch.
        rows: Vec<LabelRow>,
        /// The column the hovered stretch starts at, on the one row of a one-row label.
        hovered: u16,
    },
}

impl GridModel {
    /// The hover's text for a program hyperlink with a web destination, `None` for any other link.
    ///
    /// The label is read and judged afresh on every call. A resting hover's display keeps the answer it got when the
    /// hover started; [`TerminalLink`]'s equality is how the display learns it must ask again (SPEC.md: the warning
    /// need not follow later redraws of other rows while the pointer rests).
    pub fn link_disclosure(&self, link: &TerminalLink) -> Option<LinkDisclosure> {
        let target = web_target(&link.uri, matches!(link.kind, LinkKind::Text))?;
        let warning = match self.read_label(link) {
            Some(label) => verdict(label, &target),
            None => None,
        };
        Some(LinkDisclosure {
            host: target[url::Position::BeforeHost..url::Position::AfterPort].into(),
            target: target.as_str().into(),
            warning,
        })
    }

    /// Read the hovered program hyperlink's label: every row the hyperlink continues on, outward from the hovered one.
    ///
    /// `None` for a printed link, which has no label of its own, and for a link whose row no longer shows it.
    ///
    /// The walk uses one identity lookup for its whole duration and resolves the hovered row first, so its idea of which
    /// cells are this hyperlink is the hit test's ([`GridModel::row_links`] resolves the same way): rows above are then
    /// answered from the lookup's cache wherever the hovered row's resolution already passed through them. It continues
    /// through rows that show the hyperlink in more than one stretch, and stops at a row that shows none of it (a row
    /// that is entirely unseen included), at the boundary between the primary history and an alternate screen, or past
    /// the budget.
    fn read_label(&self, link: &TerminalLink) -> Option<Label> {
        let LinkKind::Osc { id } = &link.kind else {
            return None;
        };
        let range = link.ranges.first()?;
        let line = range.start.line;
        let mut identities = Identities::new(self);
        let hovered = self.label_row(&mut identities, line, id, &link.uri);
        if hovered.stretches.is_empty() {
            return None;
        }
        let mut units = hovered.units;
        let mut first = line;
        let mut rows = VecDeque::from([hovered]);
        for up in [true, false] {
            let mut at = line;
            while let Some(next) = if up {
                at.checked_sub(1)
            } else {
                at.checked_add(1)
            } {
                if self.has_screen_break_after(at.min(next)) {
                    break;
                }
                let row = self.label_row(&mut identities, next, id, &link.uri);
                if row.stretches.is_empty() {
                    break;
                }
                units += row.units;
                if rows.len() >= MAX_ROWS || units > MAX_UNITS {
                    return Some(Label::OverBudget);
                }
                if up {
                    rows.push_front(row);
                    first = next;
                } else {
                    rows.push_back(row);
                }
                at = next;
            }
        }
        let mut rows: Vec<LabelRow> = rows.into();
        if let [row] = rows.as_slice() {
            // A one-row label's stretches are separate labels, each with its own budget; only the hovered one counts.
            let over = row
                .stretches
                .iter()
                .find(|stretch| stretch.from == range.start.col)
                .map_or(row.units, |stretch| stretch.text.encode_utf16().count());
            if over > MAX_UNITS {
                return Some(Label::OverBudget);
            }
        }
        for index in 1..rows.len() {
            // The walk read consecutive lines from `first`, so row `index` is line `first + index`.
            let soft = self
                .edge_seam(first + index as u64)
                .is_some_and(|seam| rows[index - 1].keyed.get(seam) == Some(&true))
                && rows[index].keyed.first() == Some(&true);
            rows[index].seam_before = Some(if soft {
                RowSeam::Soft
            } else if rows[index - 1].hidden_after || rows[index].hidden_before {
                RowSeam::Break
            } else {
                RowSeam::Hard
            });
        }
        Some(Label::Read {
            rows,
            hovered: range.start.col,
        })
    }

    /// One row of the label walk: the hyperlink `(id, uri)`'s visible stretches on `line`, and the unseen text there with
    /// the same destination. A row that doesn't exist reads as one with no stretches, which ends the walk.
    ///
    /// Unseen text is matched by destination alone, not by link identity: an unseen run has no visible identity to
    /// match ([`crate::links`]'s `visible_link`), and a second hyperlink to the same place on the row sharing the
    /// evidence errs toward warning.
    fn label_row(
        &self,
        identities: &mut Identities<'_>,
        line: u64,
        id: &str,
        uri: &str,
    ) -> LabelRow {
        let mut out = LabelRow {
            stretches: Vec::new(),
            units: 0,
            keyed: Vec::new(),
            hidden_before: false,
            hidden_after: false,
            hidden: false,
            seam_before: None,
        };
        let Some((cols, row)) = self.selection_line(line) else {
            return out;
        };
        let text_end = row.text_columns(cols);
        out.keyed = identities
            .row(line)
            .iter()
            .map(|key| {
                key.as_ref()
                    .is_some_and(|key| key.id == id && key.uri == uri)
            })
            .collect();
        let mut spans: Vec<(u16, u16)> = Vec::new();
        let mut inside = false;
        // Columns of unseen runs with this destination that have something to draw.
        let mut hidden: Vec<u16> = Vec::new();
        for (run, &keyed) in row.runs.iter().zip(&out.keyed) {
            if run.col >= text_end {
                break;
            }
            if keyed {
                match spans.last_mut() {
                    Some((_, to)) if inside && *to == run.col => *to = run.col + run.width,
                    _ => spans.push((run.col, run.col + run.width)),
                }
            }
            inside = keyed;
            if run.link.as_ref().is_some_and(|link| link.uri == uri) && self.hides_ink(run) {
                hidden.push(run.col);
            }
        }
        out.stretches = spans
            .iter()
            .map(|&(from, to)| Stretch {
                from,
                text: row.text(from, to.min(text_end)),
            })
            .collect();
        out.units = out
            .stretches
            .iter()
            .map(|stretch| stretch.text.encode_utf16().count())
            .sum();
        if let (Some(&(start, _)), Some(&(_, end))) = (spans.first(), spans.last()) {
            out.hidden_before = hidden.iter().any(|&col| col < start);
            out.hidden_after = hidden.iter().any(|&col| col >= end);
        }
        out.hidden = !hidden.is_empty();
        out
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The verdict
// ---------------------------------------------------------------------------------------------------------------------

/// The warning a label earns against a web destination, `None` for the ordinary display.
///
/// - Over budget: incomplete, with no text.
/// - A label on several rows, one of which shows it in more than one stretch: the stretches can't be put back into one
///   text, so each is scanned alone, and the label is incomplete when any of them mentions an address, or when unseen
///   text with its destination is on its rows. A gap between stretches is how a program hides part of a label from the
///   comparison, so the warning stays even when each piece on its own names the destination.
/// - Otherwise the label is complete. A one-row label with several stretches is the hovered stretch alone: each is its
///   own label, as the old client had it. It mismatches when it does with its hard seams read as [`separates`] says
///   and also with every hard seam joined; the reason is the first reading's.
///
/// Every label adds hidden when unseen text with its destination is on one of its rows.
fn verdict(label: Label, target: &Url) -> Option<LinkWarning> {
    let Label::Read { rows, hovered } = label else {
        return Some(LinkWarning {
            reasons: vec![WarningReason::Incomplete],
            label: String::new(),
            address: None,
            address_parsed: None,
            more: 0,
        });
    };
    let hidden = rows.iter().any(|row| row.hidden);
    let mut reasons = Vec::new();
    let mut address = None;
    let mut address_parsed = None;
    let mut more = 0;
    let shown = if rows.len() > 1 && rows.iter().any(|row| row.stretches.len() > 1) {
        let mentions = rows.iter().flat_map(|row| &row.stretches).any(|stretch| {
            !link_claims::addresses(&[RowText::first(&stretch.text)], &[]).is_empty()
        });
        if mentions || hidden {
            reasons.push(WarningReason::Incomplete);
        }
        // For display only: each row's stretches spaced apart, its seams read as for a complete label.
        let texts: Vec<String> = rows
            .iter()
            .map(|row| {
                let texts: Vec<&str> = row
                    .stretches
                    .iter()
                    .map(|stretch| stretch.text.as_str())
                    .collect();
                texts.join(" ")
            })
            .collect();
        let pieces: Vec<(&str, Option<RowSeam>)> = texts
            .iter()
            .zip(&rows)
            .map(|(text, row)| (text.as_str(), row.seam_before))
            .collect();
        read_seams(&pieces).label
    } else {
        let pieces: Vec<(&str, Option<RowSeam>)> = match rows.as_slice() {
            [row] => {
                let stretch = row
                    .stretches
                    .iter()
                    .find(|stretch| stretch.from == hovered)
                    .or(row.stretches.first())?;
                vec![(stretch.text.as_str(), None)]
            }
            _ => rows
                .iter()
                .map(|row| (row.stretches[0].text.as_str(), row.seam_before))
                .collect(),
        };
        let complete = judge_complete(&pieces, target);
        if let Some(found) = complete.mismatch {
            reasons.push(found.kind.into());
            address = Some(shortened(&found.text));
            address_parsed = found.parsed;
            more = found.more;
        }
        complete.label
    };
    if hidden {
        reasons.push(WarningReason::Hidden);
    }
    (!reasons.is_empty()).then(|| LinkWarning {
        reasons,
        label: shortened(shown.trim_matches(is_js_space)),
        address,
        address_parsed,
        more,
    })
}

/// The first mismatching address of a complete label, ready for the hover.
struct Found {
    kind: MismatchKind,
    /// The address's reported form, as written.
    text: String,
    parsed: Option<String>,
    more: usize,
}

/// A complete label's judgment and the text the hover shows for it.
struct Complete {
    /// `Some` when the label mismatches in both readings.
    mismatch: Option<Found>,
    /// The label with each separating seam as a space and each joining one as nothing.
    label: String,
}

/// A label's rows with the two readings of their hard seams, and the text the hover shows for it.
struct Seams<'a> {
    rows: Vec<RowText<'a>>,
    /// Per hard seam, in order: whether it separates in the first reading ([`separates`], or a break).
    separate: Vec<bool>,
    /// Per hard seam: whether it separates with every hard seam joined (only a break does).
    joined: Vec<bool>,
    /// The rows with each separating seam as a space and each joining one as nothing.
    label: String,
}

/// Read a label's seams, one piece per row with the seam before it.
fn read_seams<'a>(pieces: &[(&'a str, Option<RowSeam>)]) -> Seams<'a> {
    let rows: Vec<RowText<'a>> = pieces
        .iter()
        .map(|&(text, seam)| RowText {
            text,
            seam_before: seam.map(|seam| match seam {
                RowSeam::Soft => Seam::Soft,
                RowSeam::Hard | RowSeam::Break => Seam::Hard,
            }),
        })
        .collect();
    let mut separate = Vec::new();
    let mut joined = Vec::new();
    let mut label = String::new();
    for (index, &(text, seam)) in pieces.iter().enumerate() {
        let cut = match seam {
            None | Some(RowSeam::Soft) => None,
            Some(RowSeam::Break) => Some((true, true)),
            Some(RowSeam::Hard) => Some((separates(&rows[index..]), false)),
        };
        if let Some((per_seam, all_joined)) = cut {
            separate.push(per_seam);
            joined.push(all_joined);
            if per_seam {
                label.push(' ');
            }
        }
        label.push_str(text);
    }
    Seams {
        rows,
        separate,
        joined,
        label,
    }
}

/// Judge a complete label, one piece per row with the seam before it.
fn judge_complete(pieces: &[(&str, Option<RowSeam>)], target: &Url) -> Complete {
    let Seams {
        rows,
        separate,
        joined,
        label,
    } = read_seams(pieces);
    let judgement = link_claims::judge(&rows, &separate, target);
    let mismatch = judgement.first_mismatch.and_then(|(index, kind)| {
        // Joining every hard seam can only turn a mismatch into a match when some seam separated.
        if separate != joined
            && link_claims::judge(&rows, &joined, target)
                .first_mismatch
                .is_none()
        {
            return None;
        }
        let address = &judgement.addresses[index];
        let range = if address.trimmed.is_empty() {
            address.span.clone()
        } else {
            address.trimmed.clone()
        };
        Some(Found {
            kind,
            text: link_claims::label_text(&rows)[range].to_owned(),
            parsed: judgement.first_mismatch_url.clone(),
            more: judgement.mismatches - 1,
        })
    });
    Complete { mismatch, label }
}

/// Whether the hard seam before `rows[0]` separates like whitespace: the segment after it (its rows up to the next hard
/// seam, soft-joined) mentions an address with no letter before it in the segment.
///
/// That is how a program that prints an address on a row of its own reads (`Preview` above `https://github.com/`, or a
/// numbered `1.https://…`), and joining would glue the word before into the scheme. A segment that starts with anything
/// else continues the row before it (`github.` above `com/login`, `https://github.com/a` above `/b`), so the seam joins
/// and a host or path split over rows is judged whole.
fn separates(rows: &[RowText<'_>]) -> bool {
    let mut segment = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if index > 0 && row.seam_before != Some(Seam::Soft) {
            break;
        }
        segment.push(RowText::first(row.text));
        if index > 0 {
            segment[index].seam_before = Some(Seam::Soft);
        }
    }
    let Some(address) = link_claims::addresses(&segment, &[]).into_iter().next() else {
        return false;
    };
    !link_claims::label_text(&segment)[..address.span.start]
        .chars()
        .any(char::is_alphabetic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The mark is the one thing in the hover that points at a place, so it must cover exactly where the destination
    /// departs from what the text claimed: too little and the user doesn't see the host they're really going to, a
    /// range that runs backwards or splits a character can't be drawn at all. Spec: the mark is the target's part left
    /// after the longest common prefix, then the longest common suffix of what remains of the shorter string (so the
    /// two never overlap); it is `None` when that part is empty, also when the claim is longer than the target; and a
    /// non-ASCII difference is marked whole.
    #[test]
    fn the_difference_is_the_targets_middle_between_a_common_prefix_and_suffix() {
        for (claimed, target, marked) in [
            (
                "https://github.com/login",
                "https://evil.test/login",
                Some("evil.test"),
            ),
            ("http://example.test/", "https://example.test/", Some("s")),
            (
                "https://example.test/",
                "https://example.test/?q",
                Some("?q"),
            ),
            ("https://a.test/xx", "https://a.test/xxx", Some("x")),
            ("https://a.test/abc", "https://a.test/abcabc", Some("abc")),
            ("https://x.test/", "https://y.test/", Some("y")),
            ("https://x.test/é", "https://x.test/è", Some("è")),
            ("https://example.test/a", "https://example.test/a", None),
            ("https://example.test/a/b", "https://example.test/a", None),
            ("https://a.test/abcabc", "https://a.test/abc", None),
        ] {
            // Slicing also proves the range lies on character boundaries inside the target.
            assert_eq!(
                differing_middle(claimed, target).map(|range| &target[range]),
                marked,
                "{claimed} against {target}"
            );
        }
    }

    /// The exceptions to the pinned oracle's safety check, `tests/fixtures/links-oracle-exceptions.json`, after checking
    /// the whole file: every entry has a known kind, a key and a rule, and no key appears twice. Returns the `policy`
    /// entries' keys as `(text, uri)`.
    fn policy_exceptions() -> BTreeSet<(String, String)> {
        let file: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/links-oracle-exceptions.json"
        ))
        .unwrap();
        let mut seen = BTreeSet::new();
        let mut policy = BTreeSet::new();
        for entry in file["exceptions"].as_array().unwrap() {
            let kind = entry["kind"].as_str().unwrap();
            assert!(
                matches!(kind, "policy" | "grid"),
                "unknown exception kind: {entry}"
            );
            assert!(
                entry["rule"].as_str().is_some_and(|rule| !rule.is_empty()),
                "an exception names the behavior that explains it: {entry}"
            );
            assert!(
                seen.insert((kind.to_owned(), entry["key"].to_string())),
                "duplicate exception: {entry}"
            );
            if kind == "policy" {
                policy.insert((
                    entry["key"]["text"].as_str().unwrap().to_owned(),
                    entry["key"]["uri"].as_str().unwrap().to_owned(),
                ));
            }
        }
        policy
    }

    /// The old web client's recorded answers no longer bind the warning (SPEC.md's terminal links paragraph replaced its
    /// address-shape rule), but they still guard one direction: a label the old client warned on, whose destination is a
    /// web address, must still warn, unless a checked-in exception names the behavior that makes it quiet. Answers it
    /// was quiet on are not asserted; the address tables in `link_claims/tests.rs` are the quiet side's specification.
    /// Every non-web destination must be one the warning never looks at, and their count is pinned so a change to
    /// `web_target` can't quietly move cases out of the check. The presentation half (escaping, shortening, parsed
    /// targets and hosts) still binds exactly. The generator verified both source hashes first.
    #[test]
    fn pinned_disclosure_policy_oracle() {
        let oracle: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/links-oracle.json")).unwrap();
        let exceptions = policy_exceptions();
        let mut used = BTreeSet::new();
        let mut quiet = Vec::new();
        let mut not_web = 0;
        for case in oracle["policy"].as_array().unwrap() {
            let text = case["text"].as_str().unwrap();
            let uri = case["uri"].as_str().unwrap();
            let Some(target) = web_target(uri, false) else {
                not_web += 1;
                continue;
            };
            if !case["mismatch"].as_bool().unwrap() {
                continue;
            }
            let warns = link_claims::judge(&[RowText::first(text)], &[], &target)
                .first_mismatch
                .is_some();
            let key = (text.to_owned(), uri.to_owned());
            if exceptions.contains(&key) {
                assert!(
                    !warns,
                    "an exception for a case that still warns: {text:?} -> {uri:?}"
                );
                used.insert(key);
            } else if !warns {
                quiet.push(key);
            }
        }
        assert!(quiet.is_empty(), "no longer warn: {quiet:#?}");
        assert_eq!(not_web, 414, "policy cases with a non-web destination");
        let unused: Vec<_> = exceptions.difference(&used).collect();
        assert!(unused.is_empty(), "unused policy exceptions: {unused:?}");
        for case in oracle["formats"].as_array().unwrap() {
            let text = case["text"].as_str().unwrap();
            assert_eq!(visible(text), case["visible"].as_str().unwrap());
            assert_eq!(shortened(text), case["shortened"].as_str().unwrap());
        }
        for case in oracle["targets"].as_array().unwrap() {
            let uri = case["uri"].as_str().unwrap();
            let parsed = Url::parse(uri).ok();
            assert_eq!(
                parsed.as_ref().map(|url| url.as_str()).unwrap_or(uri),
                case["displayed"].as_str().unwrap(),
                "{uri}"
            );
            assert_eq!(
                web_target(uri, true).is_some(),
                case["plain"].as_bool().unwrap(),
                "{uri}"
            );
            let host = parsed
                .as_ref()
                .filter(|url| url.host_str().is_some())
                .map(|url| &url[url::Position::BeforeHost..url::Position::AfterPort]);
            assert_eq!(host, case["host"].as_str(), "{uri}");
        }
    }
}

//! What web addresses a hyperlink's visible text mentions, and whether each one names where the link goes.
//!
//! A program's OSC 8 hyperlink shows one text and opens another. The terminal warns when the text mentions a web
//! address anywhere in it that differs from the destination, and stays quiet on honest decoration: quotes, arrows,
//! emoji, bullets, sentence punctuation, words around a matching address, emails, handles and file names. This module
//! is that judgment as a pure function of the label's text, row by row, and the destination. It knows nothing about
//! grids, colours or identities; the caller (the hover, `link_disclosure.rs`) reads the label and decides how its rows
//! join.
//!
//! Here's the TLDR of the mechanics:
//!
//! - The label is cut into tokens at whitespace and at row seams the caller says separate.
//! - Each token is folded into a recognition view ([`fold`]): compatibility forms, look-alike punctuation and
//!   invisible characters are normalized away, so `ｅxample。test` is recognized as `example.test`.
//! - Addresses are found at anchors in that view (`://`, `www.`, a dotted name, `@` before a host, and a few explicit
//!   URL spellings), each extended once over the original characters ([`scan`]). Emails, SCP-style git remotes and
//!   handles claim nothing.
//! - Each address is compared with the destination twice, as written and with its trailing decoration trimmed
//!   ([`compare`]). It matches when either form does. Recognition that had to change the scheme or authority to see
//!   the address, and login details before a host, always count as a mismatch.
//!
//! Every byte range this module returns indexes [`label_text`] of the rows it was given: the rows' texts joined with
//! nothing between them, whatever their seams.
//!
//! Scanning is linear in the label's length. No start position is retried, and every per-character loop advances
//! over text no earlier loop of the same kind covered; the step-count test pins that on adversarial inputs.

mod compare;
mod endings;
mod fold;
mod scan;
#[cfg(test)]
mod tests;

use std::ops::Range;
use url::Url;

/// One row of a hyperlink's label, as the caller read it from the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowText<'a> {
    /// The row's visible label text. Unseen text is already left out by the caller.
    pub text: &'a str,
    /// How this row continues the one before it. `None` on the first row; a `None` anywhere else joins like a soft
    /// seam, so a caller that forgets it never splits an address in two.
    pub seam_before: Option<Seam>,
}

impl<'a> RowText<'a> {
    /// A row with no seam before it: a label's first row, or the whole of a one-row label.
    pub fn first(text: &'a str) -> Self {
        Self {
            text,
            seam_before: None,
        }
    }
}

/// The boundary between two rows of one label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seam {
    /// The hyperlink continues across the row edge as a wrap would draw it, so the seam always joins.
    Soft,
    /// Anything else. Whether it separates like whitespace or joins is the caller's call, passed per hard seam.
    Hard,
}

/// How an address was spelled, which decides how it is parsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressKind {
    /// Found at `://`, `http:`/`https:` without `//`, or a bare `//`. Those with a scheme name parse as written; a
    /// bare `//` borrows the destination's scheme.
    Scheme,
    /// `www.` followed by a dotted name. Borrows the destination's scheme.
    Www,
    /// A dotted name with a qualifying ending, an IP address or `localhost` with a path or port, or a host after an
    /// `@`. Borrows the destination's scheme.
    Host,
}

/// One address a label mentions. Ranges index [`label_text`] and always hold the original characters, never the
/// recognition view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address {
    /// The address as written, through its extent: up to the token's end, a terminator or an unbalanced `)` or `]`.
    pub span: Range<usize>,
    /// The same start, with trailing decoration (sentence punctuation, Markdown emphasis, non-ASCII symbols and
    /// punctuation, emoji) and trailing invisible characters removed. Equal to `span` when there is none. Mismatch
    /// reasons and the parsed form a hover shows come from this form.
    pub trimmed: Range<usize>,
    /// The host the raw form names: after any userinfo, before any port. Empty for an address with no authority,
    /// such as `file:///tmp/x`.
    pub host: Range<usize>,
    pub kind: AddressKind,
    /// The raw form's authority holds an `@` (a look-alike one included), so what comes before it is login details
    /// and the real host is what follows.
    pub has_userinfo: bool,
    /// The raw form has no `:`, `/`, `?`, `#` or `@`, so it names only a site (`github.com`, `github.com,`). Such an
    /// address matches any destination on that host. Judged on the raw form on purpose: `github.com?` is not a bare
    /// site name even though its trimmed form is.
    pub host_only: bool,
    /// Whether the address names its own scheme and so parses exactly as written.
    written: bool,
    /// What recognition found for the raw and the trimmed form, in that order.
    forms: [Form; 2],
}

/// Recognition facts for one form of an address, fixed at scan time because they need the recognition view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Form {
    /// Recognition had to change the scheme, the authority or the character ending the authority to see this
    /// address, or the host's ending is a non-ASCII look-alike of a real one. Such a form never matches.
    disguised: bool,
    /// The form's authority holds an `@`. Such a form never matches.
    userinfo: bool,
}

/// Why an address doesn't name the destination, in the order a hover explains it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MismatchKind {
    /// It parses to another place, or doesn't parse at all.
    Mismatch,
    /// It is spelled with look-alike, compatibility or invisible characters where a browser reads the place.
    Disguised,
    /// It puts login details before the real host.
    Credentials,
}

/// The verdict on one label against one destination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Judgement {
    /// Every address the label mentions, in reading order.
    pub addresses: Vec<Address>,
    /// The first address that doesn't match, by index into `addresses`, and why. `None` means the label matches:
    /// every address names the destination, or there are none.
    pub first_mismatch: Option<(usize, MismatchKind)>,
    /// How many addresses don't match.
    pub mismatches: usize,
    /// For the first mismatching address, the URL parser's serialization of its reported form (the trimmed one,
    /// unless trimming left nothing), when that parses. Its own scheme is kept; an address without one has the
    /// destination's. This is what a hover compares the destination with to mark where they differ.
    pub first_mismatch_url: Option<String>,
}

/// The text every range in this module indexes: the rows' texts joined with nothing between them.
pub fn label_text(rows: &[RowText<'_>]) -> String {
    rows.iter().map(|row| row.text).collect()
}

/// Every address the label mentions, in reading order.
///
/// `separate` holds one entry per hard seam, in order: `true` cuts the label there as whitespace would, `false` joins
/// the rows. A missing entry joins. Soft seams always join.
pub fn addresses(rows: &[RowText<'_>], separate: &[bool]) -> Vec<Address> {
    let (label, splits) = layout(rows, separate);
    scan_label(&label, &splits)
}

/// Judge a label against a web destination.
///
/// The caller has already established that `target` is a web destination (http or https with a host). Addresses
/// without their own scheme borrow `target`'s, so for anything else the result means nothing.
pub fn judge(rows: &[RowText<'_>], separate: &[bool], target: &Url) -> Judgement {
    let (label, splits) = layout(rows, separate);
    let addresses = scan_label(&label, &splits);
    let mut first_mismatch = None;
    let mut first_mismatch_url = None;
    let mut mismatches = 0;
    for (index, address) in addresses.iter().enumerate() {
        if let Some(kind) = compare::mismatch(&label, address, target) {
            mismatches += 1;
            if first_mismatch.is_none() {
                first_mismatch = Some((index, kind));
                first_mismatch_url = compare::reported_url(&label, address, target);
            }
        }
    }
    Judgement {
        addresses,
        first_mismatch,
        mismatches,
        first_mismatch_url,
    }
}

/// The joined label and the byte offsets where a separating seam cuts it.
fn layout(rows: &[RowText<'_>], separate: &[bool]) -> (String, Vec<usize>) {
    let mut label = String::new();
    let mut splits = Vec::new();
    let mut hard = 0;
    for row in rows {
        if row.seam_before == Some(Seam::Hard) {
            if separate.get(hard).copied().unwrap_or(false) {
                splits.push(label.len());
            }
            hard += 1;
        }
        label.push_str(row.text);
    }
    (label, splits)
}

/// Whitespace that separates tokens: JavaScript's whitespace class without U+FEFF, plus the Braille blank and the
/// Hangul fillers, which draw nothing.
///
/// U+FEFF is left out because it is a format character a program can slip into a host unseen (`exa<U+FEFF>mple.com`):
/// cutting there would split the disguised host into two harmless words, while keeping it lets recognition fold it
/// away and the comparison see the disguise.
fn separates(ch: char) -> bool {
    (crate::links::is_js_space(ch) && ch != '\u{feff}')
        || matches!(
            ch,
            '\u{2800}' | '\u{115f}' | '\u{1160}' | '\u{3164}' | '\u{ffa0}'
        )
}

/// Cut the label into tokens and scan each. `splits` are the byte offsets of separating seams, ascending.
fn scan_label(label: &str, splits: &[usize]) -> Vec<Address> {
    let mut out = Vec::new();
    let mut splits = splits.iter().copied().peekable();
    let mut token: Option<usize> = None;
    for (offset, ch) in label.char_indices() {
        step();
        while splits.next_if(|&split| split < offset).is_some() {}
        if splits.next_if_eq(&offset).is_some()
            && let Some(start) = token.take()
        {
            scan::scan(label, start..offset, &mut out);
        }
        if separates(ch) {
            if let Some(start) = token.take() {
                scan::scan(label, start..offset, &mut out);
            }
        } else {
            token.get_or_insert(offset);
        }
    }
    if let Some(start) = token {
        scan::scan(label, start..label.len(), &mut out);
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Step counting
//
// The scanner promises linear time, and a regression to quadratic would only show on hostile labels nobody types
// by hand. Test builds count the scanner's per-character work so a test can hold it to a budget; other builds compile
// the counters to nothing.
// ---------------------------------------------------------------------------------------------------------------

#[cfg(test)]
thread_local! {
    static STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Count one unit of per-character scanner work.
#[inline]
fn step() {
    steps(1);
}

/// Count `count` units of scanner work done at once, such as the bytes a regex search moved over.
#[inline]
fn steps(count: usize) {
    #[cfg(test)]
    STEPS.with(|steps| steps.set(steps.get() + count));
    #[cfg(not(test))]
    let _ = count;
}

/// The work counted on this thread since the last call, which also resets the count.
#[cfg(test)]
fn take_steps() -> usize {
    STEPS.with(|steps| steps.replace(0))
}

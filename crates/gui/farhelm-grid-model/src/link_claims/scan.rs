//! Finding the addresses in one token.
//!
//! The scanner walks the token's recognition view once, left to right, and stops at **anchors**: places where an
//! address can be recognized. Each anchor is classified once and either rejected or extended into an address; an
//! accepted address's text is skipped, so a URL inside another URL's query belongs to the first. No start position is
//! ever retried.
//!
//! The anchors, in the order a position is tried:
//!
//! - `://` after a run of scheme characters with a letter in it: a scheme address (`https://`, `Visithttps://`).
//! - `http:` or `https:` without `//`, at the token start or after a non-letter: browsers accept `https:github.com`
//!   and `https:/github.com`, so a label can too.
//! - `//` before an authority, at the token start or after a non-letter: a protocol-relative address.
//! - `@` before a host that would count on its own: a host address whose userinfo is the text before the `@`
//!   (`support@paypal.com/login`, `github.com:secret@evil.test/x`).
//! - `www.` before a dotted name, at the token start or after a non-letter.
//! - a dotted name (from one regex pass, taken maximally), a bracketed IPv6 address or `localhost`, when its ending
//!   qualifies ([`Scanner::qualify`]) and its start is not glued to a path or another name
//!   ([`Scanner::host_may_start`]).
//!
//! Emails, SCP-style git remotes and handles claim nothing ([`suppressed`]). Their spans hide the anchors inside them,
//! and only those: an address anchored before such a span extends over it like over any other text, so
//! `https://paypal.com/signin?email=you@example.com` is one address.
//!
//! Recognition happens in the folded view; the extent of an address is judged on original characters
//! ([`Scanner::accept`]).

use super::endings;
use super::fold::{Cluster, Folded, OTHER_SCRIPT_LETTER};
use super::{Address, AddressKind, Form, step, steps};
use crate::constant_regex;
use icu_normalizer::DecomposingNormalizerBorrowed;
use regex::Regex;
use std::net::Ipv6Addr;
use std::ops::Range;
use std::sync::LazyLock;

/// Maximal runs of labels (letters with any marks the recognition view kept, numbers, `-`) joined by dots, with an optional final root dot. Single labels match
/// too, so `localhost` is found in the same pass; only `localhost` among them can anchor.
static NAME: LazyLock<Regex> =
    LazyLock::new(|| constant_regex(r"[\p{L}\p{M}\p{N}-]+(?:\.[\p{L}\p{M}\p{N}-]+)*\.?"));

/// Emails (and the SCP remotes that start like one) and handles. The local part's class is the usual email one; it
/// stops at any other character, so `mailto:alice@example.com` yields `alice@example.com`. A handle's name takes dots,
/// so the whole handle is matched (`@nytimes.com`), and `@name@host` is one handle.
static CLAIMLESS: LazyLock<Regex> = LazyLock::new(|| {
    constant_regex(concat!(
        r"[A-Za-z0-9._%+-]+@[\p{L}\p{M}\p{N}-]+(?:\.[\p{L}\p{M}\p{N}-]+)+",
        r"|@[\p{L}\p{M}\p{N}_.-]+(?:@[\p{L}\p{M}\p{N}-]+(?:\.[\p{L}\p{M}\p{N}-]+)+)?",
    ))
});

// Unicode general categories and scripts, tested one character at a time through [`is`]. Rust's `char` methods
// answer different questions (`is_alphabetic` is the Alphabetic property, not the letter category), and the
// checks these serve are defined in categories.
static LETTER: LazyLock<Regex> = LazyLock::new(|| constant_regex(r"\p{L}"));
static NUMBER: LazyLock<Regex> = LazyLock::new(|| constant_regex(r"\p{N}"));
static DIGIT: LazyLock<Regex> = LazyLock::new(|| constant_regex(r"\p{Nd}"));
/// Letters of the scripts whose letters can pass for ASCII ones. See [`lookalike_letter`] for which of them count.
static CONFUSABLE_LETTER: LazyLock<Regex> =
    LazyLock::new(|| constant_regex(r"[\p{L}&&[\p{Latin}\p{Greek}\p{Cyrillic}]]"));
static PUNCTUATION: LazyLock<Regex> = LazyLock::new(|| constant_regex(r"\p{P}"));
static SYMBOL: LazyLock<Regex> = LazyLock::new(|| constant_regex(r"[\p{So}\p{Sm}\p{Sk}]"));
static BRACKET_OR_QUOTE: LazyLock<Regex> =
    LazyLock::new(|| constant_regex(r"[\p{Ps}\p{Pe}\p{Pi}\p{Pf}\p{Quotation_Mark}]"));

/// Whether `ch` is in the single-character class `class`.
fn is(class: &Regex, ch: char) -> bool {
    class.is_match(ch.encode_utf8(&mut [0; 4]))
}

/// A Unicode letter (general category L), in any script.
fn letter(ch: char) -> bool {
    is(&LETTER, ch)
}

/// Characters that end an address wherever they appear: the ASCII ones a URL never holds unescaped in running text,
/// and every non-ASCII bracket or quote, which wraps an address rather than belonging to it (`“https://a.test/”`,
/// `「https://a.test/」`). CJK sentence punctuation is deliberately not here: `。` is a look-alike dot and must stay
/// inside a host for the disguise check to see it.
fn terminator(ch: char) -> bool {
    matches!(ch, '<' | '>' | '"' | '`' | '{' | '}' | '|')
        || (!ch.is_ascii() && is(&BRACKET_OR_QUOTE, ch))
}

/// Trailing decoration the trimmed form drops: the ASCII sentence punctuation `. , : ; ! ?`, the Markdown emphasis
/// characters `* _ ~`, `'`, and any non-ASCII symbol (arrows, emoji) or non-ASCII punctuation (`…`, `，`, `。`).
///
/// No other ASCII character is decoration. A trailing `/`, `&`, `-`, `#`, `=`, `%`, `+` or `@` is part of what a
/// browser opens, so `https://a.test/x//` and `?q=1&` keep it and are compared with it, as strict comparison
/// requires. Non-ASCII punctuation is dropped without a bracket check: a non-ASCII closing bracket is a terminator, so
/// it never reaches an extent, and the only closers inside one are ASCII `)` and `]`, which are never decoration.
fn decoration(ch: char) -> bool {
    matches!(
        ch,
        '.' | ',' | ':' | ';' | '!' | '?' | '*' | '_' | '~' | '\''
    ) || (!ch.is_ascii() && (is(&SYMBOL, ch) || is(&PUNCTUATION, ch)))
}

/// Characters a scheme can be spelled with as far as recognition goes: Unicode letters and digits, `+`, `.` and `-`.
/// Wider than a real scheme on purpose, so a look-alike scheme (`httрs` with a Cyrillic `р`) is still recognized and
/// then fails to parse.
fn scheme_char(ch: char) -> bool {
    letter(ch) || is(&DIGIT, ch) || matches!(ch, '+' | '.' | '-')
}

/// Characters that can start an authority after `//` or `https:`.
fn authority_char(ch: char) -> bool {
    letter(ch) || is(&NUMBER, ch) || matches!(ch, '[' | '-' | '.' | '_' | '~' | '%')
}

/// Can be in an email's local part, so an `@` after it is not a handle's.
fn local_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-')
}

/// ASCII bracket depth since an address's start. An address never takes a `)` or `]` it didn't open: that closer
/// belongs to a wrapper around the address (`(https://a.test/)`, `[docs](https://a.test/)`).
#[derive(Default)]
struct Depth {
    paren: usize,
    bracket: usize,
}

impl Depth {
    /// Take `ch` into the count. False, with the count unchanged, for a closer with nothing open to close.
    fn admits(&mut self, ch: char) -> bool {
        match ch {
            '(' => self.paren += 1,
            '[' => self.bracket += 1,
            ')' if self.paren == 0 => return false,
            ')' => self.paren -= 1,
            ']' if self.bracket == 0 => return false,
            ']' => self.bracket -= 1,
            _ => {}
        }
        true
    }
}

/// Where an `@` anchor's address starts: right after the last terminator or unbalanced closer before the `@`, and
/// never before the end of the previous accepted address.
///
/// It moves forward only, over each cluster once per token, which keeps `@`-heavy tokens linear: every `@` anchor
/// asks about a later cluster than the one before, and an accepted address restarts it at its end, which is later
/// still.
struct Segments {
    at: usize,
    start: usize,
    depth: Depth,
}

impl Segments {
    fn new() -> Self {
        Self {
            at: 0,
            start: 0,
            depth: Depth::default(),
        }
    }

    /// Restart at cluster `at`, the end of an accepted address.
    fn restart(&mut self, at: usize) {
        self.at = at;
        self.start = at;
        self.depth = Depth::default();
    }

    /// Where an address whose `@` is cluster `at` starts.
    fn start_before(&mut self, at: usize, clusters: &[Cluster]) -> usize {
        while self.at < at {
            step();
            let ch = clusters[self.at].base;
            if terminator(ch) || !self.depth.admits(ch) {
                self.start = self.at + 1;
                self.depth = Depth::default();
            }
            self.at += 1;
        }
        self.start
    }
}

/// An anchor that qualified, before it is extended.
struct Anchor {
    /// First cluster of the address.
    start: usize,
    /// First cluster of its authority: after `://` (or `https:`, or `//`); the `@` itself for an `@` anchor, whose
    /// userinfo is fixed by the anchor; else the start itself.
    authority: usize,
    kind: AddressKind,
    /// It names its own scheme and parses as written.
    written: bool,
    /// `\` ends its authority, as it does for the URL parser in http and https. True for every address that borrows the
    /// destination's scheme, which is always one of those.
    backslash_ends_authority: bool,
}

/// Find every address in the token at `token` (a byte range of `label`) and append them to `out`.
pub(super) fn scan(label: &str, token: Range<usize>, out: &mut Vec<Address>) {
    let folded = Folded::new(label, token.clone());
    let suppressed = suppressed(&folded.text);
    let mut names = Vec::new();
    let mut from = 0;
    for name in NAME.find_iter(&folded.text) {
        steps(name.end() - from);
        from = name.end();
        names.push(name.range());
    }
    steps(folded.text.len() - from);
    Scanner {
        token_end: token.end,
        folded,
        names,
        lower: 0,
        segments: Segments::new(),
        out,
    }
    .run(&suppressed);
}

/// The spans (bytes of the recognition view) of emails, SCP remotes and handles, ascending and disjoint.
///
/// - An email is a local part, `@` and a dotted name, not followed by `/`, `?`, `#` or `:` and a digit (those make it
///   userinfo before a host), and whose local part doesn't start with `www.` (`www.paypal.com@evil.test` is a site
///   name in front of another host).
/// - An SCP remote is an email followed by `:` and a non-digit (`git@github.com:org/repo.git`). Its path can look like
///   anything, so it claims the rest of the token.
/// - A handle is `@name` or `@name@host` whose `@` starts the token or follows a character that can't be in an email's
///   local part, and which isn't followed by `/`, `?`, `#` or `:` and a digit.
fn suppressed(text: &str) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(found) = CLAIMLESS.find_at(text, from) {
        steps(found.end() - from);
        let mut after = text[found.end()..].chars();
        let next = after.next();
        let second = after.next();
        if found.as_str().starts_with('@') {
            if text[..found.start()]
                .chars()
                .next_back()
                .is_some_and(local_char)
            {
                // The `@` belongs to an email shape whose domain didn't qualify. An email can still start right
                // after it (`x@name@host.com` holds `name@host.com`), so search again from there.
                from = found.start() + 1;
                continue;
            }
            // Followed by a path, or by a port, the `@` puts userinfo before a host (`notion.so:@evil.com:443/x`),
            // as it does after an email shape.
            let port = next == Some(':') && second.is_some_and(|ch| ch.is_ascii_digit());
            if !matches!(next, Some('/' | '?' | '#')) && !port {
                spans.push(found.range());
            }
        } else if !found
            .as_str()
            .get(..4)
            .is_some_and(|start| start.eq_ignore_ascii_case("www."))
        {
            match (next, second) {
                (Some('/' | '?' | '#'), _) => {}
                (Some(':'), Some(digit)) if digit.is_ascii_digit() => {}
                (Some(':'), Some(_)) => {
                    spans.push(found.start()..text.len());
                    return spans;
                }
                _ => spans.push(found.range()),
            }
        }
        from = found.end();
    }
    steps(text.len() - from);
    spans
}

/// Whether a dotted name (recognition view, without a root dot) ends in a look-alike of a real ending: `paypal.cоm`
/// with a Cyrillic `о`, `раураl.сом`. Such a host is always disguised, whatever it is compared with, because it reads
/// as a familiar name while a browser goes somewhere else.
///
/// The signature is narrow on purpose. The last label isn't a real ending, has at least two letters, and holds a
/// look-alike letter ([`lookalike_letter`]); and the name mixes in ASCII letters somewhere. Without the mix, a glued
/// abbreviation in ordinary prose (`т.е.`, `ул.Ленина`, `π.χ.`) would read as a disguised host, and without the two
/// letters `u.ä.` would. Accepted gap: a spoof spelled entirely in look-alike
/// letters (`раураl.сом` with a Cyrillic `l` too) isn't caught here; with a scheme or path in front of it, it is still
/// compared, and mismatches.
fn lookalike_ending(name: &str) -> bool {
    steps(name.len());
    let Some((_, last)) = name.rsplit_once('.') else {
        return false;
    };
    last.chars().filter(|&ch| letter(ch)).count() >= 2
        && last.chars().any(lookalike_letter)
        && name.chars().any(|ch| ch.is_ascii_alphabetic())
        && !endings::a_label(last)
            .as_deref()
            .is_some_and(endings::is_real)
}

/// A non-ASCII letter a reader can take for an ASCII one: a Cyrillic or Greek letter (`о`, `ο`), or a Latin letter
/// that isn't an ASCII letter with an accent (`ı`, `ɑ`, `ɡ`).
///
/// An accented Latin letter (`é`, `ü`, `ñ`, whose canonical decomposition starts with an ASCII letter) and `ß` are
/// left out: they are how German, French and Spanish are spelled, so a sentence glued across a full stop
/// (`gut.Übrigens`, `fichier.été`, `Straße.Köln`) would otherwise read as a disguised host. A spoof needs a letter that
/// passes for the plain ASCII one, and those do not decompose to it.
fn lookalike_letter(ch: char) -> bool {
    !ch.is_ascii()
        && !matches!(ch, '\u{df}' | '\u{1e9e}')
        && is(&CONFUSABLE_LETTER, ch)
        && !DecomposingNormalizerBorrowed::new_nfd()
            .normalize(ch.encode_utf8(&mut [0; 4]))
            .starts_with(|first: char| first.is_ascii_alphabetic())
}

/// One token's scan in progress.
struct Scanner<'a> {
    token_end: usize,
    folded: Folded,
    /// Every maximal name in the recognition view, ascending.
    names: Vec<Range<usize>>,
    /// The cluster where the last accepted address ended. Nothing before it can anchor or start an address.
    lower: usize,
    /// Where an `@` anchor's userinfo may start.
    segments: Segments,
    out: &'a mut Vec<Address>,
}

impl Scanner<'_> {
    /// The single left-to-right pass. Positions inside a suppressed span or an accepted address are jumped over
    /// whole; every other position is tried once against the anchors its character can start, and the `names` cursor
    /// only moves forward with it.
    fn run(mut self, suppressed: &[Range<usize>]) {
        let mut at = 0;
        let mut span = 0;
        let mut name = 0;
        while at < self.folded.text.len() {
            step();
            while suppressed.get(span).is_some_and(|span| span.end <= at) {
                span += 1;
            }
            if let Some(span) = suppressed.get(span).filter(|span| span.start <= at) {
                at = span.end;
                continue;
            }
            while self.names.get(name).is_some_and(|name| name.end <= at) {
                name += 1;
            }
            let covering = self
                .names
                .get(name)
                .filter(|name| name.start <= at)
                .cloned();
            let Some(ch) = self.folded.text[at..].chars().next() else {
                break;
            };
            let anchor = match ch {
                ':' => self.scheme(at),
                '/' => self.slashes(at),
                'h' | 'H' => self.http(at),
                '@' => self.at_sign(at, self.names.get(name).cloned()),
                _ => None,
            }
            .or_else(|| {
                matches!(ch, 'w' | 'W')
                    .then(|| self.www(at, covering.as_ref()))
                    .flatten()
            })
            .or_else(|| self.host(at, covering.as_ref()));
            at = match anchor {
                Some(anchor) => match self.accept(anchor) {
                    Some(end) => self.folded.start_of[end].max(at + ch.len_utf8()),
                    None => at + ch.len_utf8(),
                },
                None => at + ch.len_utf8(),
            };
        }
    }

    /// The recognition view.
    fn text(&self) -> &str {
        &self.folded.text
    }

    /// The folded character before byte `at` is a letter. False at the token start.
    fn after_letter(&self, at: usize) -> bool {
        self.text()[..at].chars().next_back().is_some_and(letter)
    }

    /// `://` after a scheme. The scheme is the run of scheme characters before it, from the run's first letter, so
    /// `1.https://`, `-https://` and `__https://` are `https` while `Visithttps://` is `Visithttps` (and warns). Where
    /// a letter of another script than Latin, Greek or Cyrillic runs straight into ASCII letters, the scheme starts at
    /// the last such change (`詳細はhttps://` is `https`), because those scripts don't put a space before a URL. At
    /// least one character the extent takes must follow `://`: `Format: https://` and `Prefix URLs with "https://"`
    /// mention no address.
    fn scheme(&self, at: usize) -> Option<Anchor> {
        let text = self.text();
        if !text[at..].starts_with("://") || at + 3 >= text.len() {
            return None;
        }
        let floor = self.folded.start_of[self.lower];
        let mut run = at;
        while let Some(ch) = text[floor..run].chars().next_back() {
            if !scheme_char(ch) {
                break;
            }
            step();
            run -= ch.len_utf8();
        }
        let mut start = run;
        let mut previous = None;
        for (offset, ch) in text[run..at].char_indices() {
            step();
            if previous.is_some_and(|previous| is(&OTHER_SCRIPT_LETTER, previous))
                && ch.is_ascii_alphabetic()
            {
                start = run + offset;
            }
            previous = Some(ch);
        }
        let (offset, _) = text[start..at].char_indices().find(|&(_, ch)| letter(ch))?;
        let scheme = &text[start + offset..at];
        if !self.extent_takes(at + 3) {
            return None;
        }
        Some(Anchor {
            start: self.folded.cluster(start + offset),
            authority: self.folded.cluster(at + 3),
            kind: AddressKind::Scheme,
            written: true,
            backslash_ends_authority: scheme.eq_ignore_ascii_case("http")
                || scheme.eq_ignore_ascii_case("https"),
        })
    }

    /// `http:` or `https:` followed by `/`, `\` or an authority character but not by `//` (which is the scheme
    /// anchor's). Only at the token start or after a non-letter, which keeps `Visithttps://` one scheme. The
    /// authority starts after any run of `/` and `\`, as the URL parser reads it, and a character the extent takes
    /// must follow that run (`http:/<x>` is no address).
    fn http(&self, at: usize) -> Option<Anchor> {
        let text = self.text();
        if self.after_letter(at) {
            return None;
        }
        let length = if text
            .get(at..at + 6)
            .is_some_and(|s| s.eq_ignore_ascii_case("https:"))
        {
            6
        } else if text
            .get(at..at + 5)
            .is_some_and(|s| s.eq_ignore_ascii_case("http:"))
        {
            5
        } else {
            return None;
        };
        let after = at + length;
        if text[after..].starts_with("//") {
            return None;
        }
        let mut authority = after;
        while text[authority..].starts_with(['/', '\\']) {
            step();
            authority += 1;
        }
        let next = text[authority..].chars().next()?;
        if (authority == after && !authority_char(next)) || !self.extent_takes(authority) {
            return None;
        }
        Some(Anchor {
            start: self.folded.cluster(at),
            authority: self.folded.cluster(authority),
            kind: AddressKind::Scheme,
            written: true,
            backslash_ends_authority: true,
        })
    }

    /// `//` followed by an authority, at the token start or after a non-letter (`//intranet/`). It borrows the
    /// destination's scheme.
    fn slashes(&self, at: usize) -> Option<Anchor> {
        let text = self.text();
        if !text[at..].starts_with("//") || self.after_letter(at) {
            return None;
        }
        if !authority_char(text[at + 2..].chars().next()?) {
            return None;
        }
        Some(Anchor {
            start: self.folded.cluster(at),
            authority: self.folded.cluster(at + 2),
            kind: AddressKind::Scheme,
            written: false,
            backslash_ends_authority: true,
        })
    }

    /// An `@` followed by a host that would count on its own (`name` is the first name not ending before the `@`,
    /// which is the one right after it if any). The address starts where [`Segments`] says, and everything from there
    /// to the `@` is userinfo, even a `/` (`a/b@paypal.com/login` puts `a/b` before `paypal.com`). With none
    /// (`@nytimes.com/x`) there is no address: nothing is put before the host.
    fn at_sign(&mut self, at: usize, name: Option<Range<usize>>) -> Option<Anchor> {
        let host = at + 1;
        let qualifies = if self.text()[host..].starts_with('[') {
            self.ipv6(host).is_some()
        } else {
            self.qualify(name.filter(|name| name.start == host)?)
        };
        if !qualifies {
            return None;
        }
        let at = self.folded.cluster(at);
        let start = self.segments.start_before(at, &self.folded.clusters);
        if start >= at {
            return None;
        }
        Some(Anchor {
            start,
            authority: at,
            kind: AddressKind::Host,
            written: false,
            backslash_ends_authority: true,
        })
    }

    /// `www.` followed by a dotted name of at least two labels, at the token start or after a non-letter. Its ending
    /// doesn't have to qualify: `www.` says it's a site. `www.intranet/x` has one label after `www.` and is no address
    /// (an accepted gap).
    fn www(&self, at: usize, name: Option<&Range<usize>>) -> Option<Anchor> {
        let text = self.text();
        if !text
            .get(at..at + 4)
            .is_some_and(|s| s.eq_ignore_ascii_case("www."))
            || self.after_letter(at)
        {
            return None;
        }
        let name = name.filter(|name| at + 4 < name.end)?;
        let rest = &text[at + 4..name.end];
        steps(rest.len());
        if !rest.strip_suffix('.').unwrap_or(rest).contains('.') {
            return None;
        }
        Some(Anchor {
            start: self.folded.cluster(at),
            authority: self.folded.cluster(at),
            kind: AddressKind::Www,
            written: false,
            backslash_ends_authority: true,
        })
    }

    /// A dotted name, `localhost` or a bracketed IPv6 address starting at `at`, with a start that isn't glued to
    /// something else and an ending that qualifies.
    fn host(&self, at: usize, name: Option<&Range<usize>>) -> Option<Anchor> {
        let qualifies = if self.text()[at..].starts_with('[') {
            self.host_may_start(at) && self.ipv6(at).is_some()
        } else {
            let name = name.filter(|name| name.start == at)?;
            self.host_may_start(at) && self.qualify(name.clone())
        };
        if !qualifies {
            return None;
        }
        Some(Anchor {
            start: self.folded.cluster(at),
            authority: self.folded.cluster(at),
            kind: AddressKind::Host,
            written: false,
            backslash_ends_authority: true,
        })
    }

    /// A host can start at the token start, or after a character that is not a letter, number, `/`, `\`, `@` or `.`,
    /// or after a run of `. _ * ~` that follows such a character or the token start. So `…github.com`, `.evil.test/x`,
    /// `_github.com_`, `Login:github.com/` and `(github.com)` start, while `/tmp/example.test/x` and `src/docs.rs/x`
    /// (a path) and `@nytimes.com` (a handle, or a host after userinfo that the `@` anchor owns) don't.
    ///
    /// A host starts when either reading of the text before it allows it: the original characters, or the
    /// recognition view. Each catches starts the other hides. On the originals, `Loading…github.com` and `／github.com`
    /// start, though folding spells `…` as three dots after a letter and the fullwidth solidus as `/`. In the
    /// recognition view, `⑴paypal.com` (folded to `(1)paypal.com`), `Loginːpaypal.com` (a modifier-letter colon) and
    /// `xͺpaypal.com` start, though their originals put a number or a letter right before the host. Requiring both
    /// would let either disguise glue a host to the text before it and hide it; a path or a handle is still a barrier in
    /// both readings.
    fn host_may_start(&self, at: usize) -> bool {
        let barrier =
            |ch: char| letter(ch) || is(&NUMBER, ch) || matches!(ch, '/' | '\\' | '@' | '.');
        let text = self.text();
        let mut start = at;
        while let Some(ch @ ('.' | '_' | '*' | '~')) = text[..start].chars().next_back() {
            step();
            start -= ch.len_utf8();
        }
        if text[..start]
            .chars()
            .next_back()
            .is_none_or(|ch| !barrier(ch))
        {
            return true;
        }
        let mut cluster = self.folded.cluster(at);
        if self.folded.start_of[cluster] != at {
            // The name begins inside a cluster, after the first character it folds to: nothing original separates
            // it from what comes before, so the recognition view's answer above stands.
            return false;
        }
        let clusters = &self.folded.clusters;
        while cluster > 0 && matches!(clusters[cluster - 1].base, '.' | '_' | '*' | '~') {
            step();
            cluster -= 1;
        }
        cluster == 0 || !barrier(clusters[cluster - 1].base)
    }

    /// Whether the name at `name` (bytes of the recognition view) counts as a host. It counts when:
    ///
    /// - its last label is a bare ending, whatever follows;
    /// - it has a look-alike ending ([`lookalike_ending`]), whatever follows (`paypal.cоm` with a Cyrillic `о`);
    /// - its last label is a real ending and `/` follows, possibly after `:port` (`docs.rs/src`, `a.test:8080/`);
    /// - it is a four-octet IPv4 address or `localhost`, and `/` or `:port` follows. A bare `1.2.3.4` or `localhost`
    ///   in prose doesn't count, and neither does a shortened IPv4 like `127.1`.
    fn qualify(&self, name: Range<usize>) -> bool {
        let text = self.text();
        let follow = name.end;
        let whole = &text[name];
        let whole = whole.strip_suffix('.').unwrap_or(whole);
        steps(whole.len());
        let mut labels = 0;
        let mut last = "";
        let mut previous = None;
        let mut octets = true;
        for label in whole.split('.') {
            labels += 1;
            if labels > 1 {
                previous = Some(last);
            }
            last = label;
            octets &= label.len() <= 3
                && label.bytes().all(|byte| byte.is_ascii_digit())
                && label.parse::<u16>().is_ok_and(|octet| octet <= 255);
        }
        if labels == 1 {
            return whole.eq_ignore_ascii_case("localhost") && self.slash_or_port(follow);
        }
        if labels == 4 && octets {
            return self.slash_or_port(follow);
        }
        let ending = endings::a_label(last);
        if ending
            .as_deref()
            .is_some_and(|ending| endings::is_bare(ending, previous))
            || lookalike_ending(whole)
        {
            return true;
        }
        ending.as_deref().is_some_and(endings::is_real) && self.slash_after_port(follow)
    }

    /// Whether the extent can take the cluster whose recognition view starts at byte `at`: it exists and is neither a
    /// terminator nor a closing `)` or `]`. A scheme with nothing the extent takes after it names nothing.
    fn extent_takes(&self, at: usize) -> bool {
        self.folded
            .clusters
            .get(self.folded.cluster(at))
            .is_some_and(|cluster| !terminator(cluster.base) && !matches!(cluster.base, ')' | ']'))
    }

    /// `/`, or `:` and a digit, at byte `at`.
    fn slash_or_port(&self, at: usize) -> bool {
        let rest = &self.text()[at..];
        rest.starts_with('/')
            || (rest.starts_with(':') && rest[1..].starts_with(|ch: char| ch.is_ascii_digit()))
    }

    /// `/` at byte `at`, or after `:` and at least one digit there.
    fn slash_after_port(&self, at: usize) -> bool {
        let text = self.text();
        let mut at = at;
        if text[at..].starts_with(':') {
            let digits = text[at + 1..]
                .bytes()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
            steps(digits);
            if digits == 0 {
                return false;
            }
            at += 1 + digits;
        }
        text[at..].starts_with('/')
    }

    /// A bracketed IPv6 address at byte `at`: the byte after its `]`.
    fn ipv6(&self, at: usize) -> Option<usize> {
        let text = self.text();
        let inside = text[at + 1..]
            .bytes()
            .take_while(|byte| byte.is_ascii_hexdigit() || matches!(byte, b':' | b'.'))
            .count();
        steps(inside);
        let end = at + 1 + inside;
        let address = &text[at + 1..end];
        (text[end..].starts_with(']')
            && address.contains(':')
            && address.parse::<Ipv6Addr>().is_ok())
        .then_some(end + 1)
    }

    /// Extend an anchor into an address over the original characters, record it, and return the cluster after it.
    /// `None`, recording nothing, for a scheme whose trimmed form keeps nothing after its separator.
    ///
    /// The authority runs from the anchor's authority start to the first `/`, `?`, `#`, `\` (for http and https),
    /// terminator, or unbalanced `)` or `]`; the last `@` inside it splits off userinfo. The extent then continues to
    /// the token's end, a terminator or an unbalanced `)` or `]`, so balanced brackets stay in (`page_(name)`).
    ///
    /// Every fact about the address comes from this one walk over its clusters, so accepting costs one step per
    /// cluster it covers.
    fn accept(&mut self, anchor: Anchor) -> Option<usize> {
        let clusters = &self.folded.clusters;
        let mut depth = Depth::default();
        // The first cluster recognition changed, among those a disguise is judged on: the scheme, the authority and
        // the character ending the authority.
        let mut first_changed = None;
        // Whether the raw text so far has a character that makes the address more than a site name.
        let mut structured = false;
        let mut note = |at: usize, cluster: &Cluster, judged: bool| {
            if judged && cluster.changed {
                first_changed.get_or_insert(at);
            }
            structured |= matches!(cluster.base, ':' | '/' | '?' | '#' | '@');
        };
        for (at, cluster) in clusters
            .iter()
            .enumerate()
            .take(anchor.authority)
            .skip(anchor.start)
        {
            step();
            note(at, cluster, true);
            depth.admits(cluster.base);
        }

        // The authority. The host is what follows its last `@`, up to its first `:` outside IPv6 brackets.
        let mut at = anchor.authority;
        let mut first_at = None;
        let mut host_start = anchor.authority;
        let mut host_end = None;
        let mut host_brackets = 0usize;
        while let Some(cluster) = clusters.get(at) {
            step();
            let ch = cluster.base;
            if matches!(ch, '/' | '?' | '#')
                || (anchor.backslash_ends_authority && ch == '\\')
                || terminator(ch)
                || !depth.admits(ch)
            {
                break;
            }
            note(at, cluster, true);
            if cluster.at {
                first_at.get_or_insert(at);
                host_start = at + 1;
                host_end = None;
                host_brackets = 0;
            } else {
                match ch {
                    '[' => host_brackets += 1,
                    ']' => host_brackets = host_brackets.saturating_sub(1),
                    ':' if host_brackets == 0 => {
                        host_end.get_or_insert(at);
                    }
                    _ => {}
                }
            }
            at += 1;
        }
        let authority_end = at;
        let host_end = host_end.unwrap_or(authority_end);

        // The rest of the extent. Its first cluster is the one ending the authority, which a disguise also covers.
        while let Some(cluster) = clusters.get(at) {
            step();
            if terminator(cluster.base) || !depth.admits(cluster.base) {
                break;
            }
            note(at, cluster, at == authority_end);
            at += 1;
        }
        // Trailing clusters that fold to nothing (a format character such as U+FEFF or U+200B) leave the address
        // altogether: they draw nothing, like a leading one, which never enters an address. Left in the raw form they
        // would be percent-encoded into the path, and a label that names exactly its destination
        // (`…/wiki/Yahoo!` followed by a zero-width space) would fail in both forms.
        let start_of = &self.folded.start_of;
        while at > anchor.authority && start_of[at - 1] == start_of[at] {
            step();
            at -= 1;
        }
        let end = at;
        // Trailing decoration comes off the trimmed form, and so do trailing clusters that fold to nothing (a format
        // character such as U+FEFF): like a leading invisible character, it draws nothing, so there is nothing for it to
        // disguise, and leaving it in would percent-encode it into the path and fail an honest label.
        let mut trimmed = end;
        while trimmed > anchor.start
            && (decoration(clusters[trimmed - 1].base)
                || start_of[trimmed - 1] == start_of[trimmed])
        {
            step();
            trimmed -= 1;
        }
        // A scheme with nothing but decoration after its separator names nothing: `Use https://, not http://` and
        // `Links start with https://.` talk about URLs.
        if anchor.kind == AddressKind::Scheme && trimmed <= anchor.authority {
            return None;
        }
        let host_start = host_start.min(end);
        let host_end = host_end.min(end);

        // A look-alike ending makes every form disguised, whichever anchor found the address, so `paypal.cоm/login`,
        // `https://paypal.cоm/login` and `www.paypal.cоm` read the same.
        let host_view = &self.folded.text[start_of[host_start]..start_of[host_end]];
        steps(host_view.len());
        let lookalike = NAME
            .find(host_view)
            .filter(|name| name.start() == 0)
            .is_some_and(|name| {
                let name = name.as_str();
                lookalike_ending(name.strip_suffix('.').unwrap_or(name))
            });

        let form = |form_end: usize| Form {
            disguised: lookalike
                || first_changed.is_some_and(|changed| changed < (authority_end + 1).min(form_end)),
            userinfo: first_at.is_some_and(|first| first < authority_end.min(form_end)),
        };
        let forms = [form(end), form(trimmed)];
        let byte = |at: usize| {
            clusters
                .get(at)
                .map_or(self.token_end, |cluster| cluster.range.start)
        };
        self.out.push(Address {
            span: byte(anchor.start)..byte(end),
            trimmed: byte(anchor.start)..byte(trimmed),
            host: byte(host_start)..byte(host_end),
            kind: anchor.kind,
            has_userinfo: forms[0].userinfo,
            host_only: !structured,
            written: anchor.written,
            forms,
        });
        self.lower = end;
        self.segments.restart(end);
        Some(end)
    }
}

//! The recognition view of one token: the text the scanner matches address patterns against.
//!
//! A label can spell an address with characters that look like the ASCII a browser parses (a fullwidth letter, a
//! division slash, an ideographic full stop) or with characters that draw nothing at all (a zero-width space inside
//! a host). Matching ASCII patterns against the text as written would let those pass as prose, which is exactly the
//! disguise a warning exists for. So recognition runs on a folded copy, built one extended grapheme cluster at a
//! time:
//!
//! 1. NFKC, which turns compatibility forms (fullwidth letters and digits, `…`, `ﬁ`) into their plain equivalents;
//! 2. the look-alike table ([`lookalike`]), for punctuation NFKC leaves alone but a reader takes for `:`, `/`, `.` or
//!    `@`;
//! 3. format characters removed, and combining marks too when the cluster's base is not a letter of another script
//!    than Latin, Greek or Cyrillic.
//!
//! Marks are kept on other scripts' letters because there they are spelling, not decoration: a Devanagari or Thai
//! host is written with them, and taking them out would make every such host look changed. On Latin, Greek and
//! Cyrillic letters (and on digits, punctuation and symbols) a mark that NFKC didn't compose away is the kind of
//! thing a disguise is made of (`http\u{34f}s`, stacked accents, a variation selector).
//!
//! Every folded character remembers the original cluster it came from, so whatever the scanner finds in the folded
//! copy maps back to whole original clusters, and the comparison can ask whether recognition had to change anything
//! to see the address. That question compares the recognition view with the NFC form of the original, so an accent
//! typed decomposed (`bu\u{308}cher.de`) is the same text as its precomposed form, as it is to IDNA. Boundaries, terminators and trimming never look at the folded copy: they judge each
//! cluster's first original character.

use super::step;
use crate::constant_regex;
use icu_normalizer::ComposingNormalizerBorrowed;
use regex::Regex;
use std::ops::Range;
use std::sync::LazyLock;
use unicode_segmentation::UnicodeSegmentation;

// Characters that draw nothing of their own or only modify the glyph before them: format characters (a zero-width
// space, a directionality mark) and combining marks (a variation selector, an accent). The supervisor keeps
// zero-width characters with the cell before them, so a label can carry them anywhere while the screen shows a
// clean address.
static FORMAT: LazyLock<Regex> = LazyLock::new(|| constant_regex(r"\p{Cf}"));
static MARK: LazyLock<Regex> = LazyLock::new(|| constant_regex(r"\p{M}"));

/// Letters of the scripts other than Latin, Greek and Cyrillic. Their combining marks are spelling and stay in the
/// recognition view, and their prose runs into a URL without a space, which is where a scheme starts.
pub(super) static OTHER_SCRIPT_LETTER: LazyLock<Regex> =
    LazyLock::new(|| constant_regex(r"[\p{L}&&[^\p{Latin}\p{Greek}\p{Cyrillic}]]"));

fn is(class: &Regex, ch: char) -> bool {
    class.is_match(ch.encode_utf8(&mut [0; 4]))
}

/// The look-alike table: punctuation a reader takes for one of the four characters that structure an address, and
/// which NFKC does not already turn into it. Shared with the hover's current mismatch check, so the two agree on
/// what counts as a look-alike.
pub(crate) fn lookalike(ch: char) -> char {
    match ch {
        '\u{02d0}' | '\u{02f8}' | '\u{2236}' | '\u{a789}' | '\u{ff1a}' | '\u{fe13}'
        | '\u{fe55}' => ':',
        '\u{2215}' | '\u{2044}' | '\u{29f8}' | '\u{ff0f}' | '\u{2571}' => '/',
        '\u{3002}' | '\u{ff0e}' | '\u{ff61}' | '\u{2024}' => '.',
        '\u{ff20}' | '\u{fe6b}' => '@',
        other => other,
    }
}

/// One extended grapheme cluster of the original token.
pub(super) struct Cluster {
    /// Its bytes in the label.
    pub(super) range: Range<usize>,
    /// Its first character as written. Terminators, brackets, authority ends and trimming judge this, never the
    /// folded form, so a look-alike `/` does not end an authority (and the authority it then extends is disguised).
    pub(super) base: char,
    /// Recognition changed it: its folded form is not the text as written, in NFC.
    pub(super) changed: bool,
    /// Its folded form holds an `@`, real or look-alike.
    pub(super) at: bool,
}

/// A token's clusters and their recognition view.
pub(super) struct Folded {
    pub(super) clusters: Vec<Cluster>,
    /// The recognition view: every cluster's folded form, concatenated.
    pub(super) text: String,
    /// For each byte of `text`, the index of the cluster it came from.
    pub(super) cluster_of: Vec<usize>,
    /// For each cluster, the byte of `text` where its folded form starts, plus one final entry holding `text`'s
    /// length. A cluster that folds to nothing starts where the next one does.
    pub(super) start_of: Vec<usize>,
}

impl Folded {
    /// Fold the token at `token` (a byte range of `label`).
    pub(super) fn new(label: &str, token: Range<usize>) -> Self {
        let nfkc = ComposingNormalizerBorrowed::new_nfkc();
        let nfc = ComposingNormalizerBorrowed::new_nfc();
        let mut folded = Self {
            clusters: Vec::new(),
            text: String::new(),
            cluster_of: Vec::new(),
            start_of: Vec::new(),
        };
        for (offset, original) in label[token.clone()].grapheme_indices(true) {
            step();
            let index = folded.clusters.len();
            let start = folded.text.len();
            folded.start_of.push(start);
            // A grapheme cluster is never empty; the fallback only keeps the type total.
            let base = original.chars().next().unwrap_or(' ');
            let keeps_marks = is(&OTHER_SCRIPT_LETTER, base);
            for ch in nfkc.normalize(original).chars() {
                let ch = lookalike(ch);
                if !is(&FORMAT, ch) && (keeps_marks || !is(&MARK, ch)) {
                    folded.text.push(ch);
                }
            }
            folded.cluster_of.resize(folded.text.len(), index);
            let view = &folded.text[start..];
            folded.clusters.push(Cluster {
                range: token.start + offset..token.start + offset + original.len(),
                base,
                changed: view != nfc.normalize(original),
                at: view.contains('@'),
            });
        }
        folded.start_of.push(folded.text.len());
        folded
    }

    /// The cluster the folded byte `at` came from; the cluster count for the end of the text.
    pub(super) fn cluster(&self, at: usize) -> usize {
        self.cluster_of
            .get(at)
            .copied()
            .unwrap_or(self.clusters.len())
    }
}

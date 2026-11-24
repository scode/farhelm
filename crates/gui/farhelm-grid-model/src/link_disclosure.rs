//! Target disclosure is data, not browser authorization. These pure policies
//! port terminal-links.js (LICENSE-PORTED); native geometry and gesture state
//! must still establish that the current disclosure is visible before opening.

use crate::constant_regex;
use crate::{
    GridModel, LinkKind, TerminalLink,
    link_claims::{comparable, lookalike},
    links::{Identities, JS_SPACE, is_js_space},
    web_target,
};
use regex::Regex;
use std::sync::LazyLock;
use url::Url;

/// Address-shaped labels begin with a scheme, `www.`, or a dotted host followed by a path. Scheme and host
/// letters include Unicode so a lookalike still enters the comparison. The optional userinfo keeps a familiar
/// name before an `@` from hiding a different host. A leading dot can be the first visible character of an
/// ambiguous host fragment, which still needs to warn. The classes are allow-lists, so they only see text that
/// `INVISIBLE` has already cleaned: no format character or combining mark reaches them.
static ADDRESS: LazyLock<Regex> = LazyLock::new(|| {
    constant_regex(concat!(
        r"(?i)^(?:(?P<scheme>\p{L}[\p{L}\p{N}+.-]*://)|www\.|",
        r"(?:[\p{L}\p{N}._%+~!$&'*,;=-]+(?::[\p{L}\p{N}._%+~!$&'*,;=-]*)?@)?",
        r"\.?[\p{L}\p{N}_-]+(?:\.[\p{L}\p{N}_-]+)*\.\p{L}{2,}(?::[0-9]+)?/)"
    ))
});
/// Characters that draw nothing of their own, or only modify the glyph before them: format characters such as
/// a zero-width space or a directionality mark, and combining marks such as a variation selector. The supervisor
/// keeps zero-width characters with the preceding cell, so a label can carry them anywhere while the screen shows
/// a clean address. Recognition ignores them so such a label still counts as an address; the comparison then
/// parses the original text, where they make the parse fail or name a different place.
static INVISIBLE: LazyLock<Regex> = LazyLock::new(|| constant_regex(r"[\p{Cf}\p{M}]"));
static LEADING_BLANKS: LazyLock<Regex> = LazyLock::new(|| {
    constant_regex(
        &r"^(?:[^\p{L}\p{N}\x00-\x7F]|[\u{115F}\u{1160}\u{3164}\u{FFA0}]|[%s])+"
            .replace("%s", JS_SPACE),
    )
});

/// Fully rendered text for one web target. The actual host is never shortened
/// or replaced with userinfo. A claimed address appears only with the mismatch
/// warning; ordinary prose labels do not claim that their words name a URL.
/// This owns no grid rows and cannot outlive the UI's independent evidence check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkDisclosure {
    /// Parsed authority host, including a nondefault port and IPv6 brackets.
    pub host: String,
    /// Serialized destination, retaining empty query/fragment delimiters. Long
    /// targets name the omitted middle; the separate host always remains whole.
    pub target: String,
    /// Escaped address-shaped program text requiring the loud mismatch warning.
    pub claimed: Option<String>,
    /// The same text unescaped (trimmed, and shortened like `claimed` but counted before escaping), for a UI that
    /// escapes host-supplied text itself: the desktop app draws it through its host-text element, which would escape
    /// `claimed` a second time.
    pub claimed_text: Option<String>,
}

/// How a label claims to name a web address. Only a label without a scheme borrows the target's scheme.
enum AddressShape {
    Scheme,
    Host,
}

/// Whether a character puts nothing visible between a delimiter and its neighbors: whitespace, a format
/// character or combining mark (`INVISIBLE`), or one of the blank fillers `LEADING_BLANKS` also skips. The
/// supervisor keeps zero-width characters with the cell before them, so a label can carry them right after a
/// closing bracket while the screen shows a plain pair.
fn draws_nothing_beside_a_delimiter(ch: char) -> bool {
    is_js_space(ch)
        || matches!(
            ch,
            '\u{115f}' | '\u{1160}' | '\u{3164}' | '\u{ffa0}' | '\u{2800}'
        )
        || INVISIBLE.is_match(ch.encode_utf8(&mut [0; 4]))
}

/// The address a label claims when it shows one inside matching outer pairs of `<>`, `()`, `[]`, `""` or
/// `''`, peeled layer by layer; otherwise the label itself, trimmed. Only matching outer pairs come off, so prose
/// around an address stays part of the label and keeps the ordinary display. A peeled label whose first word
/// then ends in a closing delimiter (`[https://a.test/] [docs]`) is a phrase after an address, not an address in a
/// pair, so it is returned unpeeled: such a label gets the ordinary display even when its leading address names
/// another place, a known limit. The hover still displays the original text either way.
///
/// Pairs are found on a view of each layer with characters that draw nothing trimmed from both ends, so a
/// zero-width character or blank filler beside a delimiter cannot hide the pair. The address the peel ends on
/// is trimmed of whitespace only: an invisible character between the innermost delimiter and the address stays
/// in the compared text, where it fails the parse and warns.
fn strip_matching_wrappers(text: &str) -> &str {
    let label = text.trim_matches(is_js_space);
    let mut text = label;
    loop {
        let view = text.trim_matches(draws_nothing_beside_a_delimiter);
        let closing = match view.as_bytes().first() {
            Some(b'<') => '>',
            Some(b'(') => ')',
            Some(b'[') => ']',
            Some(b'"') => '"',
            Some(b'\'') => '\'',
            _ => break,
        };
        // Every recognized opener is one byte. Search after it so a lone quote is not a pair.
        let Some(inner) = view[1..].strip_suffix(closing) else {
            break;
        };
        text = inner.trim_matches(is_js_space);
    }
    // A bracket or quote before a word break can end an address followed by prose. Check after peeling so
    // spaces between genuinely nested delimiters stay legal. When nothing was peeled, `text` is `label`.
    if text
        .split_once(is_js_space)
        .is_some_and(|(first, _)| first.ends_with(['>', ')', ']', '"', '\'']))
    {
        return label;
    }
    text
}

/// Judge lookalike punctuation, leading blanks and invisible characters only when recognizing the address.
/// Parse the original text in the form recognized here, so a lookalike scheme cannot be reinterpreted as a host.
fn address_shape(text: &str) -> Option<AddressShape> {
    let text = LEADING_BLANKS.replace(text, "");
    let text = INVISIBLE.replace_all(&text, "");
    let gated: String = text.chars().map(lookalike).collect();
    let claim = ADDRESS.captures(&gated)?;
    Some(if claim.name("scheme").is_some() {
        AddressShape::Scheme
    } else {
        AddressShape::Host
    })
}

/// A URL-shaped label that cannot be parsed is a warning, not quiet prose.
/// Scheme-less labels borrow only the target's scheme, never its authority.
fn mismatch(text: &str, uri: &str) -> bool {
    let claimed = strip_matching_wrappers(text);
    let Some(shape) = address_shape(claimed) else {
        return false;
    };
    let Ok(target) = Url::parse(uri) else {
        return true;
    };
    let named = match shape {
        AddressShape::Scheme => Url::parse(claimed),
        AddressShape::Host => Url::parse(&format!("{}://{claimed}", target.scheme())),
    };
    named.map_or(true, |named| comparable(&named) != comparable(&target))
}

/// Reveal invisible controls and escape literal notation so output cannot pose
/// as our escape syntax. Variation selectors remain readable, as in the pin.
///
/// This is the app's one escape (`farhelm_proto::text::escaped`), the table the
/// command line shares. The pinned JavaScript escaped Unicode's whole format
/// category through a regular expression; the shared table lists that category
/// in full, so the oracle below still pins the same output.
fn visible(text: &str) -> String {
    farhelm_proto::text::escaped(text)
}

/// The pin budgets 300 UTF-16 units and names the omitted middle. A boundary
/// inside a surrogate pair renders as replacement text, just as a native text
/// widget would render the JS slice's unpaired surrogate.
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

impl GridModel {
    /// Read current tagged text, including cursor-drawn hard row breaks. A join
    /// is evidence only within 128 total rows / 8192 UTF-16 units and with one
    /// contiguous occurrence per row. Ambiguous or bounded URL fragments warn
    /// even when the hovered fragment alone equals the target.
    pub fn link_disclosure(&self, link: &TerminalLink) -> Option<LinkDisclosure> {
        let target = web_target(&link.uri, matches!(link.kind, LinkKind::Text))?;
        let (text, fragment) = self.link_label(link);
        let warning = text.as_ref().filter(|text| {
            if fragment {
                address_shape(strip_matching_wrappers(text)).is_some()
            } else {
                mismatch(text, &link.uri)
            }
        });
        Some(LinkDisclosure {
            host: target[url::Position::BeforeHost..url::Position::AfterPort].into(),
            target: shortened(target.as_str()),
            claimed: warning.map(|text| shortened(&visible(text.trim_matches(is_js_space)))),
            claimed_text: warning.map(|text| shortened(text.trim_matches(is_js_space))),
        })
    }

    /// An isolated repeated tag is complete. Across rows, any ambiguous run or
    /// budget exhaustion returns the hovered row as a fragment, never a quiet
    /// assertion that the complete program-controlled label names this target.
    /// A joined run whose rows each show an address on their own returns the hovered row as a whole label
    /// (not a fragment) when the rows all repeat the target or the join does not look like one address.
    fn link_label(&self, link: &TerminalLink) -> (Option<String>, bool) {
        let LinkKind::Osc { id } = &link.kind else {
            return (None, false);
        };
        let Some(range) = link.ranges.first() else {
            return (None, false);
        };
        let Some((cols, row)) = self.selection_line(range.start.line) else {
            return (None, false);
        };
        let fragment = row
            .text(range.start.col, range.end.col.min(row.text_columns(cols)))
            .trim_end_matches(' ')
            .to_owned();
        let mut identities = Identities::new(self);
        let mut read = |line| {
            let Some((cols, row)) = self.selection_line(line) else {
                return (0, String::new());
            };
            let mut count = 0;
            let mut from = 0;
            let mut to = 0;
            let mut inside = false;
            for (run, key) in row.runs.iter().zip(identities.row(line)) {
                let tagged = key
                    .as_ref()
                    .is_some_and(|key| key.id == *id && key.uri == link.uri);
                if tagged {
                    if !inside || run.col != to {
                        count += 1;
                        if count == 1 {
                            from = run.col;
                        }
                    }
                    to = run.col + run.width;
                }
                inside = tagged;
            }
            (
                count,
                if count == 1 {
                    row.text(from, to.min(row.text_columns(cols)))
                } else {
                    String::new()
                },
            )
        };
        let line = range.start.line;
        let above = line
            .checked_sub(1)
            .filter(|&above| !self.has_screen_break_after(above))
            .map(&mut read)
            .unwrap_or_default();
        let below = line
            .checked_add(1)
            .filter(|_| !self.has_screen_break_after(line))
            .map(&mut read)
            .unwrap_or_default();
        if above.0 == 0 && below.0 == 0 {
            return (Some(fragment), false);
        }
        let (count, center) = read(line);
        if count != 1 || center.encode_utf16().count() > 8192 {
            return (Some(fragment), true);
        }
        let mut characters = center.encode_utf16().count();
        let mut rows = std::collections::VecDeque::from([center]);
        for direction in [-1, 1] {
            let mut at = line;
            while let Some(next) = if direction < 0 {
                at.checked_sub(1)
            } else {
                at.checked_add(1)
            } {
                if self.has_screen_break_after(at.min(next)) {
                    break;
                }
                let (count, text) = read(next);
                if count == 0 {
                    break;
                }
                characters += text.encode_utf16().count();
                if count != 1 || rows.len() >= 128 || characters > 8192 {
                    return (Some(fragment), true);
                }
                if direction < 0 {
                    rows.push_front(text);
                } else {
                    rows.push_back(text);
                }
                at = next;
            }
        }
        let joined = rows.iter().map(String::as_str).collect::<String>();
        let joined = joined.trim_matches(is_js_space);
        // When every row shows an address of its own, the hovered row is the claim to judge if each row
        // repeats the target, or if the rows' brackets or quotes keep the join from looking like one address
        // (`<https://a.test/>` above `https://b.test/`). Judging the join there would quietly lose the claim.
        if rows
            .iter()
            .all(|text| address_shape(strip_matching_wrappers(text)).is_some())
            && (rows.iter().all(|text| !mismatch(text, &link.uri))
                || address_shape(strip_matching_wrappers(joined)).is_none())
        {
            return (Some(fragment), false);
        }
        (Some(joined.to_owned()), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected answers come from the pinned shipping JavaScript, including
    /// Unicode lookalikes, strict parsing, query/fragment and credential cases.
    /// This is a cross-language contract check, not a Rust policy duplicated
    /// inside the test. The generator verifies both source hashes first.
    #[test]
    fn pinned_disclosure_policy_oracle() {
        let oracle: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/links-oracle.json")).unwrap();
        for case in oracle["policy"].as_array().unwrap() {
            let text = case["text"].as_str().unwrap();
            let uri = case["uri"].as_str().unwrap();
            // The old client accepted literal escape notation as part of a scheme. The bracket pair encloses
            // only the notation, so this label gets the ordinary display. An actual directionality mark warns.
            let expected = if text.trim_matches(is_js_space) == "<U+202E>https://example.com/x" {
                false
            } else {
                case["mismatch"].as_bool().unwrap()
            };
            assert_eq!(mismatch(text, uri), expected, "{text:?} -> {uri:?}");
        }
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

    /// `<https://a.test/>` is a common way to write a link, so a bracketed or quoted familiar address that goes
    /// elsewhere is exactly the spoof the warning exists for; an earlier stricter scheme rule silenced it. The
    /// address inside matching pairs, nested and with spaces around the layers, is compared with the target.
    #[test]
    fn bracketed_and_quoted_addresses_are_compared_with_the_target() {
        for label in [
            "<https://example.test/>",
            "\"https://example.test/\"",
            "'https://example.test/'",
            "(https://example.test/)",
            "[https://example.test/]",
            " ( \" < '\u{a0}https://example.test/\u{a0}' > \" ) ",
            "[example.test/]",
        ] {
            assert!(!mismatch(label, "https://example.test/"), "{label:?}");
            assert!(mismatch(label, "https://other.test/"), "{label:?}");
        }
    }

    /// A program's label can contain a URL without claiming that the whole label is an address. Punctuation
    /// that separates the prose from the URL must keep the ordinary target disclosure.
    #[test]
    fn prose_glued_to_an_address_keeps_the_ordinary_disclosure() {
        for label in [
            "See (https://example.test/x)",
            "<https://example.test/x)",
            "[https://example.test/x] [docs]",
            "( [https://example.test/x] [docs] )",
            "ticket-42(https://example.test/tickets/42)",
            "ticket-42(example.test/tickets/42)",
            "[label](https://example.test/x)",
            "label[example.test/x]",
            "javascript:alert(1)",
        ] {
            assert!(!mismatch(label, "https://other.test/"), "{label:?}");
        }
    }

    /// Peeling must stop at the outer pair: taking brackets that belong to a URL path (as wiki addresses use)
    /// would compare a different address than the one shown, warning on an honest label or hiding a changed one.
    #[test]
    fn punctuation_inside_a_wrapped_address_is_part_of_the_path() {
        let label = "(https://example.test/page_(name))";
        assert!(!mismatch(label, "https://example.test/page_(name)"));
        assert!(mismatch(label, "https://example.test/page_name"));
    }

    /// Zero-width characters ride along with the visible cell before them, so a label can carry one right after
    /// a closing bracket, between nested delimiters or beside a blank filler while the screen shows a plain
    /// bracketed address. Each must not hide the pair, or the familiar address would go elsewhere unwarned. One
    /// just inside a delimiter, before the scheme, stays in the compared address and warns even against the
    /// address it spells, since the parse fails.
    #[test]
    fn characters_that_draw_nothing_beside_a_delimiter_do_not_hide_the_pair() {
        for label in [
            "<https://example.test/>\u{200b}",
            "<https://example.test/>\u{2060}",
            "<https://example.test/>\u{fe0f}",
            "<https://example.test/>\u{34f}",
            "(\u{200b}\"https://example.test/\")",
            "(\"https://example.test/\"\u{200b})",
            "\u{3164}<https://example.test/>",
            "<https://example.test/>\u{3164}",
            "<https://example.test/>\u{2800}",
        ] {
            assert!(!mismatch(label, "https://example.test/"), "{label:?}");
            assert!(mismatch(label, "https://other.test/"), "{label:?}");
        }
        assert!(mismatch(
            "<\u{200b}https://example.test/>",
            "https://example.test/"
        ));
    }

    /// The narrower syntax must still recognize labels that imitate addresses with different characters
    /// or put a familiar name in the credentials of an unrelated host.
    #[test]
    fn disguised_addresses_still_warn() {
        for label in [
            "httрs://example.test/x",
            "httpsː//example.test/x",
            "example。test/x",
            "exаmple.test/x",
            "cafe\u{301}.test/x",
            "\u{202e}https://example.test/x",
            "https://example.test@other.test/x",
            "example.test@other.test/x",
            "example.test:secret@other.test/x",
            "example.test:example!@other.test/x",
        ] {
            assert!(mismatch(label, "https://other.test/x"), "{label:?}");
        }
    }

    /// Zero-width characters ride along with the visible cell before them, so a label can look exactly like a
    /// familiar address while carrying one in its scheme, its host or before a credentials `@`. A lookalike `@`
    /// can stand in for the real one. Each must still warn rather than count as prose.
    #[test]
    fn invisible_characters_inside_an_address_still_warn() {
        for label in [
            "ht\u{200b}tps://example.test/x",
            "http\u{34f}s://example.test/x",
            "http\u{fe0f}s://example.test/x",
            "exam\u{200b}ple.test/x",
            "example.test\u{200b}@other.test/x",
            "example.test\u{ff20}other.test/x",
            "example.test\u{fe6b}other.test/x",
        ] {
            assert!(mismatch(label, "https://other.test/x"), "{label:?}");
        }
    }

    /// A scheme-shaped claim is compared as written. Borrowing the destination's scheme could produce a valid
    /// URL from an address whose original scheme cannot be parsed.
    #[test]
    fn a_lookalike_scheme_is_not_reinterpreted_as_a_hostname() {
        for label in ["httрs://example.test/x", "\u{200b}https://example.test/x"] {
            let reinterpreted = format!("https://{label}");
            let target = Url::parse(&reinterpreted).unwrap();
            assert!(Url::parse(label).is_err(), "{label:?}");
            assert!(mismatch(label, target.as_str()), "{label:?}");
        }
    }

    /// A scheme-less label can carry a full URL as query data. It borrows the target's scheme while
    /// retaining the query, so the honest label and the same-looking label with a changed query diverge.
    #[test]
    fn a_query_value_can_contain_a_scheme_without_making_the_label_absolute() {
        let label = "www.example.test?next=https://other.test";
        assert!(!mismatch(
            label,
            "https://www.example.test/?next=https://other.test"
        ));
        assert!(mismatch(
            label,
            "https://www.example.test/?next=https://else.test"
        ));
    }
}

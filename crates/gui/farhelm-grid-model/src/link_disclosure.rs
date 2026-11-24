//! Target disclosure is data, not browser authorization. These pure policies
//! port terminal-links.js (LICENSE-PORTED); native geometry and gesture state
//! must still establish that the current disclosure is visible before opening.

use crate::constant_regex;
use crate::{
    GridModel, LinkKind, TerminalLink,
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

/// A label can quote or bracket the whole address. Remove only matching outer pairs so prose
/// around an address stays part of the label. The hover still displays the original text.
fn strip_matching_wrappers(text: &str) -> &str {
    let label = text.trim_matches(is_js_space);
    let mut text = label;
    loop {
        let closing = match text.as_bytes().first() {
            Some(b'<') => '>',
            Some(b'(') => ')',
            Some(b'[') => ']',
            Some(b'"') => '"',
            Some(b'\'') => '\'',
            _ => break,
        };
        // Every recognized opener is one byte. Search after it so a lone quote is not a pair.
        let Some(inner) = text[1..].strip_suffix(closing) else {
            break;
        };
        text = inner.trim_matches(is_js_space);
    }
    // A bracket or quote before a word break can end an address followed by prose.
    // Check after peeling so spaces between genuinely nested delimiters stay legal.
    if text.len() != label.len()
        && text
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
    let gated: String = text
        .chars()
        .map(|ch| match ch {
            '\u{02d0}' | '\u{02f8}' | '\u{2236}' | '\u{a789}' | '\u{ff1a}' | '\u{fe13}'
            | '\u{fe55}' => ':',
            '\u{2215}' | '\u{2044}' | '\u{29f8}' | '\u{ff0f}' | '\u{2571}' => '/',
            '\u{3002}' | '\u{ff0e}' | '\u{ff61}' | '\u{2024}' => '.',
            '\u{ff20}' | '\u{fe6b}' => '@',
            other => other,
        })
        .collect();
    let claim = ADDRESS.captures(&gated)?;
    Some(if claim.name("scheme").is_some() {
        AddressShape::Scheme
    } else {
        AddressShape::Host
    })
}

/// Compare places, retaining credentials, query and fragment. Only one final
/// path slash and percent-escape hex case are immaterial in the pinned policy.
fn comparable(url: &Url) -> String {
    let path = url.path().strip_suffix('/').unwrap_or(url.path());
    let query = url
        .query()
        .filter(|q| !q.is_empty())
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let fragment = url
        .fragment()
        .filter(|q| !q.is_empty())
        .map(|q| format!("#{q}"))
        .unwrap_or_default();
    let text = format!(
        "{}://{}:{}@{}{path}{query}{fragment}",
        url.scheme(),
        url.username(),
        url.password().unwrap_or(""),
        &url[url::Position::BeforeHost..url::Position::AfterPort]
    );
    let mut bytes = text.into_bytes();
    for index in 0..bytes.len().saturating_sub(2) {
        if bytes[index] == b'%'
            && bytes[index + 1].is_ascii_hexdigit()
            && bytes[index + 2].is_ascii_hexdigit()
        {
            bytes[index + 1].make_ascii_uppercase();
            bytes[index + 2].make_ascii_uppercase();
        }
    }
    // Only ASCII hex digits were case-changed, so the bytes are still the valid UTF-8 they came from.
    String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
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
        if rows.iter().all(|text| {
            address_shape(strip_matching_wrappers(text)).is_some() && !mismatch(text, &link.uri)
        }) {
            return (Some(fragment), false);
        }
        let joined = rows.iter().map(String::as_str).collect::<String>();
        let joined = joined.trim_matches(is_js_space);
        // Different delimiters can make the join lose its address shape even though each
        // row spells out its own wrapped address. Keep the hovered row's claim in that case.
        if rows.iter().all(|text| {
            let label = text.trim_matches(is_js_space);
            let address = strip_matching_wrappers(label);
            address.len() != label.len() && address_shape(address).is_some()
        }) && address_shape(strip_matching_wrappers(joined)).is_none()
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

    /// A label can present an address inside several layers of punctuation. Judge the address inside
    /// the matching pairs, including when the pieces have spaces around them.
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

    /// Brackets that belong to a URL path survive peeling the pair around the whole address.
    #[test]
    fn punctuation_inside_a_wrapped_address_is_part_of_the_path() {
        let label = "(https://example.test/page_(name))";
        assert!(!mismatch(label, "https://example.test/page_(name)"));
        assert!(mismatch(label, "https://example.test/page_name"));
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

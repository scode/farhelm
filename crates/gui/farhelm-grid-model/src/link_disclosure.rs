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

static ADDRESS: LazyLock<Regex> = LazyLock::new(|| {
    constant_regex(
        &r"(?i)^(?:[^%s/:]+://|www\.|[^%s/]+\.\p{L}{2,}(?::[0-9]+)?/)".replace("%s", JS_SPACE),
    )
});
static LEADING_BLANKS: LazyLock<Regex> = LazyLock::new(|| {
    constant_regex(
        &r"^(?:[^\p{L}\p{N}\x00-\x7F]|[\u{115F}\u{1160}\u{3164}\u{FFA0}]|[%s])+"
            .replace("%s", JS_SPACE),
    )
});
static UNSAFE: LazyLock<Regex> = LazyLock::new(|| {
    constant_regex(
        r"[\p{Cc}\p{Cf}\u{034F}\u{115F}\u{1160}\u{17B4}-\u{17B5}\u{2028}\u{2029}\u{3164}\u{FFA0}\u{E0000}-\u{E007F}]",
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
}

/// Judge lookalike punctuation only at the claim gate. Feeding the normalized
/// text to the URL parser would turn a forgery into an apparently valid match.
fn claims_address(text: &str) -> bool {
    let text = LEADING_BLANKS.replace(text, "");
    let gated: String = text
        .chars()
        .map(|ch| match ch {
            '\u{02d0}' | '\u{02f8}' | '\u{2236}' | '\u{a789}' | '\u{ff1a}' | '\u{fe13}'
            | '\u{fe55}' => ':',
            '\u{2215}' | '\u{2044}' | '\u{29f8}' | '\u{ff0f}' | '\u{2571}' => '/',
            '\u{3002}' | '\u{ff0e}' | '\u{ff61}' | '\u{2024}' => '.',
            other => other,
        })
        .collect();
    ADDRESS.is_match(&gated)
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
    let claimed = text.trim_matches(is_js_space);
    if !claims_address(claimed) {
        return false;
    }
    let Ok(target) = Url::parse(uri) else {
        return true;
    };
    let absolute = claimed.find("://").is_some_and(|end| {
        end > 0
            && !claimed[..end]
                .chars()
                .any(|ch| is_js_space(ch) || matches!(ch, '/' | ':'))
    });
    let named = if absolute {
        claimed.to_owned()
    } else {
        format!("{}://{claimed}", target.scheme())
    };
    Url::parse(&named).map_or(true, |named| comparable(&named) != comparable(&target))
}

/// Reveal invisible controls and escape literal notation so output cannot pose
/// as our escape syntax. Variation selectors remain readable, as in the pin.
fn visible(text: &str) -> String {
    let mut result = String::new();
    for (index, ch) in text.char_indices() {
        if (ch == '<' && text[index..].starts_with("<U+"))
            || UNSAFE.is_match(ch.encode_utf8(&mut [0; 4]))
        {
            result.push_str(&format!("<U+{:04X}>", ch as u32));
        } else {
            result.push(ch);
        }
    }
    result
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
                claims_address(text.trim_matches(is_js_space))
            } else {
                mismatch(text, &link.uri)
            }
        });
        Some(LinkDisclosure {
            host: target[url::Position::BeforeHost..url::Position::AfterPort].into(),
            target: shortened(target.as_str()),
            claimed: warning.map(|text| shortened(&visible(text.trim_matches(is_js_space)))),
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
        let Some((_, row)) = self.selection_line(range.start.line) else {
            return (None, false);
        };
        let fragment = row
            .text(range.start.col, range.end.col)
            .trim_end_matches(' ')
            .to_owned();
        let mut identities = Identities::new(self);
        let mut read = |line| {
            let Some((_, row)) = self.selection_line(line) else {
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
                    row.text(from, to)
                } else {
                    String::new()
                },
            )
        };
        let line = range.start.line;
        let above = line.checked_sub(1).map(&mut read).unwrap_or_default();
        let below = line.checked_add(1).map(&mut read).unwrap_or_default();
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
            claims_address(text.trim_matches(is_js_space)) && !mismatch(text, &link.uri)
        }) {
            (Some(fragment), false)
        } else {
            (
                Some(
                    rows.into_iter()
                        .collect::<String>()
                        .trim_matches(is_js_space)
                        .to_owned(),
                ),
                false,
            )
        }
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
            assert_eq!(
                mismatch(text, uri),
                case["mismatch"].as_bool().unwrap(),
                "{text:?} -> {uri:?}"
            );
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
}

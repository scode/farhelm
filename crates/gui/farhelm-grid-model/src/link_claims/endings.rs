//! Which domain endings make a dotted name read as a web address.
//!
//! A label full of dotted words (`README.md`, `java.io.File`, `logger.info`, `Farhelm.app`) mustn't warn, yet
//! `paypal.com` on its own plainly names a site. Two lists settle it:
//!
//! - **Real endings** are the root zone's top-level domains plus the reserved names `test`, `example`, `invalid`,
//!   `localhost` and `internal`. A dotted name with a real ending counts once a `/` follows it (possibly after a
//!   port): `docs.rs/src` is an address, `main.rs:12:5` is not.
//! - **Bare endings** count with nothing after them: `com`, `org`, `net`, `edu`, `gov`, `io`, `dev`, `co`, `me`, and
//!   the two-letter country endings except those that collide with file extensions or common words (`md`, `rs`,
//!   `sh`, `py`, `pl`, `in`, `do`, `so` and the rest of [`EXCLUDED`]). An excluded ending still counts after a
//!   generic second level such as `co` (`google.co.in`).
//!
//! The root zone list is a snapshot of IANA's published file, `tlds.txt` beside this module, with its version line
//! kept as its first line. SPEC_impl.md says how to refresh it. A refresh can change which bare names count, since
//! the bare list includes every two-letter ending the snapshot holds.
//!
//! Lookups are by lowercase IDNA A-label, the form IANA lists internationalized endings in. The A-label comes from the
//! URL parser's own host parsing, so it is the same conversion a browser applies to the host.

use std::collections::HashSet;
use std::sync::LazyLock;

/// IANA's root zone list (`https://data.iana.org/TLD/tlds-alpha-by-domain.txt`), one upper-case A-label per line
/// after a `#` version line.
const SNAPSHOT: &str = include_str!("tlds.txt");

/// Reserved names that never enter the root zone but name real places on test and private networks (RFC 2606's four,
/// and `internal`, which ICANN reserved for private use).
const RESERVED: [&str; 5] = ["test", "example", "invalid", "localhost", "internal"];

/// Endings that count with nothing after them, besides the two-letter ones.
const ALWAYS_BARE: [&str; 9] = ["com", "org", "net", "edu", "gov", "io", "dev", "co", "me"];

/// Two-letter endings that don't count bare. Most collide with file extensions (`notes.md`, `main.rs`, `build.sh`) or
/// short words (`in`, `do`, `so`, `am`), where a bare dotted name is far more often a file or prose than a site.
/// Accepted cost: bare sites on these endings (`docs.rs`, `notion.so`) need a path or a scheme to count.
const EXCLUDED: [&str; 36] = [
    "ac", "ai", "am", "cc", "cl", "cr", "cu", "do", "gd", "gs", "in", "la", "ly", "ma", "md", "mk",
    "ml", "mm", "mo", "ms", "nu", "pl", "pm", "ps", "pt", "py", "re", "rs", "sc", "sh", "so", "ss",
    "st", "sv", "td", "tf",
];

/// Second levels after which an excluded two-letter ending still counts bare: `google.co.in` is a site even though
/// `in` alone is not.
const GENERIC_SECOND_LEVEL: [&str; 7] = ["co", "com", "org", "net", "gov", "edu", "ac"];

/// Every real ending, lowercase.
static REAL: LazyLock<HashSet<String>> = LazyLock::new(|| {
    SNAPSHOT
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_ascii_lowercase)
        .chain(RESERVED.iter().map(|name| (*name).to_owned()))
        .collect()
});

/// Every bare ending, lowercase.
static BARE: LazyLock<HashSet<String>> = LazyLock::new(|| {
    REAL.iter()
        .filter(|ending| ending.len() == 2 && ending.bytes().all(|byte| byte.is_ascii_lowercase()))
        .filter(|ending| !EXCLUDED.contains(&ending.as_str()))
        .cloned()
        .chain(ALWAYS_BARE.iter().map(|ending| (*ending).to_owned()))
        .collect()
});

/// The lowercase A-label of one label of the recognition view, or `None` when the URL parser would not accept it as a
/// domain (a label it reads as a number, or one IDNA refuses).
pub(super) fn a_label(label: &str) -> Option<String> {
    if label.is_ascii() {
        return Some(label.to_ascii_lowercase());
    }
    match url::Host::parse(label) {
        Ok(url::Host::Domain(domain)) => Some(domain),
        _ => None,
    }
}

/// Whether `ending` (a lowercase A-label) is a real ending.
pub(super) fn is_real(ending: &str) -> bool {
    REAL.contains(ending)
}

/// Whether a dotted name whose last label is `ending` (a lowercase A-label) counts with nothing after it.
/// `previous` is the label before it as written, if any.
pub(super) fn is_bare(ending: &str, previous: Option<&str>) -> bool {
    BARE.contains(ending)
        || (EXCLUDED.contains(&ending)
            && previous.is_some_and(|previous| {
                GENERIC_SECOND_LEVEL
                    .iter()
                    .any(|level| previous.eq_ignore_ascii_case(level))
            }))
}

/// The snapshot's version line, for the test that the file still is the IANA list it claims to be.
#[cfg(test)]
pub(super) fn snapshot_version() -> Option<&'static str> {
    SNAPSHOT
        .lines()
        .next()
        .filter(|line| line.starts_with("# Version "))
}

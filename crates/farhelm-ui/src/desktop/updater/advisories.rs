//! Authenticated, forward-compatible information for installations already in
//! the field. Unknown directives must stay harmless: a future publisher cannot
//! require every installed client to understand its new information first.
//!
//! This module knows no installation state. A verified file replaces the whole
//! in-run snapshot; an unavailable or invalid one never lifts a withdrawal.

use std::collections::BTreeMap;

use anyhow::{Context, bail, ensure};

/// Both the first line and the signature's purpose. Neither carries a format
/// version: extensions use new directives, which old clients ignore.
pub(super) const PURPOSE: &str = "farhelm advisories";
pub(super) const MAX_BYTES: usize = 64 * 1024;
const MAX_LINE_BYTES: usize = 4096;
const MAX_REASON_CHARS: usize = 240;

/// Stable versions withdrawn by the last successfully authenticated file.
/// Bare versions match the running and Installed records. The publisher can
/// name a future or nonexistent stable tag; clients have no release registry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Advisories {
    revoked: BTreeMap<String, String>,
}

impl Advisories {
    /// Look up the publisher's bounded, plain-text reason for this version.
    pub(super) fn reason(&self, version: &str) -> Option<&str> {
        self.revoked.get(version).map(String::as_str)
    }
}

/// Authenticate before interpreting any bytes, using the same ring and
/// prehashed minisign rules as release checksums but a different purpose.
pub(super) fn verify(ring: &[&str], body: &[u8], signature: &[u8]) -> anyhow::Result<Advisories> {
    ensure!(
        signature.len() <= super::SIGNATURE_MAX_BYTES,
        "advisories signature over its cap"
    );
    ensure!(body.len() <= MAX_BYTES, "advisories over their cap");
    farhelm_helm::verify_signed_metadata(ring, PURPOSE, body, signature)?;
    parse(body)
}

/// Accept only the fixed header and UTF-8, then interpret known lines locally.
/// Bad known directives cannot discard other withdrawals. Last valid duplicate
/// wins, so a publisher can correct a reason without making a file unusable.
/// The byte cap also bounds allocation and the number of possible lines.
pub(super) fn parse(body: &[u8]) -> anyhow::Result<Advisories> {
    ensure!(body.len() <= MAX_BYTES, "advisories over their cap");
    let text = std::str::from_utf8(body).context("advisories are not UTF-8")?;
    let mut lines = text.lines();
    ensure!(
        lines.next() == Some(PURPOSE),
        "advisories header does not match"
    );
    let mut advisories = Advisories::default();
    for (index, line) in lines.enumerate() {
        let directive = line.split_whitespace().next();
        if directive != Some("revoked") {
            continue;
        }
        match revocation(line) {
            Ok((version, reason)) => {
                advisories.revoked.insert(version, reason);
            }
            Err(error) => {
                tracing::warn!("updater: ignoring advisories line {}: {error:#}", index + 2)
            }
        }
    }
    Ok(advisories)
}

/// `revoked vX.Y.Z reason`, with exactly one ASCII space at each field
/// boundary and the rest of the line a free-text reason. Spaces within the
/// reason are not extra arguments. Control characters are never displayed.
fn revocation(line: &str) -> anyhow::Result<(String, String)> {
    ensure!(line.len() <= MAX_LINE_BYTES, "revocation line over its cap");
    let rest = line
        .strip_prefix("revoked ")
        .context("invalid revocation directive")?;
    let (tag, reason) = rest.split_once(' ').context("missing revocation reason")?;
    let version = tag
        .strip_prefix('v')
        .context("missing release tag prefix")?;
    let parsed = semver::Version::parse(version).context("invalid release tag")?;
    if !parsed.pre.is_empty() || !parsed.build.is_empty() {
        bail!("revocation must name a stable release");
    }
    let reason: String = reason
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_REASON_CHARS)
        .collect();
    let reason = reason.trim();
    ensure!(!reason.is_empty(), "missing revocation reason");
    Ok((version.to_string(), reason.to_string()))
}

/// Missing files mean no new information. Every other failure is an error for
/// the worker to log, never a release-verification refusal or an install gate.
type FetchFn<'a> = dyn Fn(&str, usize) -> anyhow::Result<Option<Vec<u8>>> + 'a;

/// Fetch a complete optional publication and authenticate it before replacement.
/// The injected transport distinguishes absence from every failed request.
pub(super) fn fetch(ring: &[&str], fetch: &FetchFn<'_>) -> anyhow::Result<Option<Advisories>> {
    let Some(body) = fetch("advisories", MAX_BYTES)? else {
        return Ok(None);
    };
    let signature = fetch("advisories.minisig", super::SIGNATURE_MAX_BYTES)?
        .context("advisories signature is missing")?;
    verify(ring, &body, &signature).map(Some)
}

/// A started request may finish after the setting changes; the next request
/// must not begin. Keep that boundary beside the two-file fetch so neither an
/// initial body nor its subsequent signature can bypass it.
pub(super) fn fetch_if_allowed(
    ring: &[&str],
    allowed: &dyn Fn() -> bool,
    transport: &FetchFn<'_>,
) -> anyhow::Result<Option<Advisories>> {
    fetch(ring, &|name, cap| {
        ensure!(
            allowed(),
            "automatic updates were turned off during the check"
        );
        transport(name, cap)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const FUTURE: &[u8] = include_bytes!("../../../tests/fixtures/advisories/future");
    const SIGNATURE: &[u8] = include_bytes!("../../../tests/fixtures/advisories/future.minisig");
    const KEY: &str = include_str!("../../../tests/fixtures/advisories/test-key.pub");

    /// Only the public test key is retained; the fixture's secret never lived
    /// in a file and cannot be needed by test execution.
    fn ring() -> [&'static str; 1] {
        [KEY.lines().nth(1).unwrap()]
    }

    /// New publishers must be able to add directives without disabling old
    /// clients' known withdrawals. Keep this signed future fixture permanent.
    #[farhelm_testtrace::test]
    fn a_future_file_authenticates_and_ignores_unknown_directives() {
        assert!(
            std::str::from_utf8(FUTURE)
                .unwrap()
                .contains("future-policy")
        );
        let parsed = verify(&ring(), FUTURE, SIGNATURE).unwrap();
        assert_eq!(parsed.revoked.len(), 3);
        assert_eq!(parsed.reason("1.0.0"), Some("running release issue"));
        assert_eq!(parsed.reason("999.0.0"), Some("not yet published"));
        assert!(parsed.reason("9.0.0").is_none());
    }

    /// A valid release signature cannot be reused for advisories. Test actual
    /// signed envelopes as well as corrupt bytes so a parser-only check cannot
    /// accidentally appear to establish the signature boundary.
    #[farhelm_testtrace::test]
    fn authentication_refuses_every_wrong_envelope() {
        verify(&ring(), FUTURE, SIGNATURE).unwrap();
        let mut different = FUTURE.to_vec();
        different[0] ^= 1;
        let mut forged = SIGNATURE.to_vec();
        let purpose = b"farhelm advisories";
        let at = forged
            .windows(purpose.len())
            .position(|part| part == purpose)
            .unwrap();
        forged[at] ^= 1;
        for signature in [
            b"".as_slice(),
            b"garbage".as_slice(),
            include_bytes!("../../../tests/fixtures/advisories/legacy.minisig"),
            include_bytes!("../../../tests/fixtures/advisories/wrong-purpose.minisig"),
            forged.as_slice(),
        ] {
            assert!(verify(&ring(), FUTURE, signature).is_err());
        }
        assert!(verify(&ring(), &different, SIGNATURE).is_err());
        assert!(verify(&ring(), SIGNATURE, FUTURE).is_err());
        assert!(
            verify(
                &ring(),
                FUTURE,
                &vec![b'x'; super::super::SIGNATURE_MAX_BYTES + 1]
            )
            .is_err()
        );
        assert!(verify(&[], FUTURE, SIGNATURE).is_err());
        assert!(verify(farhelm_helm::RELEASE_KEY_RING, FUTURE, SIGNATURE).is_err());
        // A well-formed, independent test key distinguishes a wrong ring from
        // a malformed configured key. Both must refuse this valid signature.
        let other = include_str!("../../../tests/fixtures/updater-release/test-key.pub");
        assert_ne!(other.lines().nth(1).unwrap(), ring()[0]);
        assert!(verify(&[other.lines().nth(1).unwrap()], FUTURE, SIGNATURE).is_err());
    }

    /// CRLF and a final unterminated line are harmless publishing choices.
    /// Invalid headers or UTF-8 are whole-file failures, never a cleared map.
    #[farhelm_testtrace::test]
    fn only_valid_text_and_the_fixed_header_can_replace_memory() {
        for body in [
            b"".as_slice(),
            b"other\nrevoked v1.0.0 issue",
            b"\xff",
            b"farhelm advisories\n\xff",
            b"farhelm advisories\0\n",
        ] {
            assert!(parse(body).is_err(), "{body:?}");
        }
        assert!(parse(&vec![b'x'; MAX_BYTES + 1]).is_err());
        for body in [
            "farhelm advisories\nrevoked v1.0.0 reason",
            "farhelm advisories\r\nrevoked v1.0.0 reason\r\n",
        ] {
            assert_eq!(
                parse(body.as_bytes()).unwrap().reason("1.0.0"),
                Some("reason")
            );
        }
        let empty = include_bytes!("../../../tests/fixtures/advisories/empty");
        let signature = include_bytes!("../../../tests/fixtures/advisories/empty.minisig");
        assert_eq!(
            verify(&ring(), empty, signature).unwrap(),
            Advisories::default()
        );
    }

    /// Malformed known lines and unknown lines cannot erase good withdrawals;
    /// the grammar names stable tags only and treats the reason as free text.
    #[farhelm_testtrace::test]
    fn malformed_directives_are_ignored_without_losing_valid_ones() {
        let malformed = [
            "revoked",
            "revoked v1.0.0",
            "revoked v1.0.0 ",
            "revoked  v1.0.0 reason",
            "revoked\tv1.0.0 reason",
            "revoked 1.0.0 reason",
            "revoked v01.0.0 reason",
            "revoked v1.0 reason",
            "revoked v1.0.0-rc.1 reason",
            "revoked v1.0.0+build reason",
            "revoked v0.0.0-unreleased reason",
            "revoked v1.0.0\0 reason",
        ];
        let baseline = parse(format!("{PURPOSE}\nrevoked v1.0.0 original").as_bytes()).unwrap();
        for line in malformed {
            assert_eq!(
                parse(format!("{PURPOSE}\n{line}").as_bytes()).unwrap(),
                Advisories::default(),
                "{line:?}"
            );
            assert_eq!(
                parse(format!("{PURPOSE}\nrevoked v1.0.0 original\n{line}").as_bytes()).unwrap(),
                baseline,
                "{line:?}"
            );
        }
        let body = format!(
            "{PURPOSE}\nrevoked v1.0.0 original\nunknown\0 future arguments\nrevoked v1.0.0 final reason with spaces\n"
        );
        let parsed = parse(body.as_bytes()).unwrap();
        assert_eq!(parsed.revoked.len(), 1);
        assert_eq!(parsed.reason("1.0.0"), Some("final reason with spaces"));
        let too_long = format!(
            "{PURPOSE}\nrevoked v1.0.0 original\nrevoked v1.0.0 {}",
            "x".repeat(MAX_LINE_BYTES)
        );
        assert_eq!(
            parse(too_long.as_bytes()).unwrap().reason("1.0.0"),
            Some("original")
        );
        let many = format!("{PURPOSE}\n{}revoked v1.0.0 last", "unknown\n".repeat(5000));
        assert_eq!(
            parse(many.as_bytes()).unwrap().reason("1.0.0"),
            Some("last")
        );
    }

    /// Publisher-supplied prose is text, never control input, and Unicode is
    /// capped by characters rather than split in the middle of a UTF-8 scalar.
    #[farhelm_testtrace::test]
    fn reasons_drop_controls_and_stay_bounded() {
        let body = format!(
            "{PURPOSE}\nrevoked v1.0.0 a\0b\t\u{7f}c\nrevoked v2.0.0 {}",
            "é".repeat(300)
        );
        let parsed = parse(body.as_bytes()).unwrap();
        assert_eq!(parsed.reason("1.0.0"), Some("abc"));
        assert_eq!(
            parsed.reason("2.0.0").unwrap().chars().count(),
            MAX_REASON_CHARS
        );
        assert!(
            parse(format!("{PURPOSE}\nrevoked v1.0.0 \0\t\u{7f}").as_bytes())
                .unwrap()
                .revoked
                .is_empty()
        );
    }

    /// Untrusted content has no panicking indexing or hidden parser premise.
    /// Include arbitrary bytes and valid-header noise so rejection at the header
    /// alone cannot make this robustness check vacuous.
    #[farhelm_testtrace::test]
    fn arbitrary_bytes_never_panic() {
        let mut seed = 1u64;
        for len in 0..512 {
            let mut body = Vec::new();
            for _ in 0..len {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                body.push((seed >> 32) as u8);
            }
            let _ = parse(&body);
            let mut prefixed = format!("{PURPOSE}\nrevoked ").into_bytes();
            prefixed.extend_from_slice(&body);
            let _ = parse(&prefixed);
        }
    }

    /// Only absence of the advisory body is quiet. Once a body exists its
    /// signature is required; failures and partial publications cannot lift it.
    #[farhelm_testtrace::test]
    fn optional_fetch_distinguishes_absence_from_invalid_publication() {
        let calls = std::cell::RefCell::new(Vec::new());
        assert!(
            fetch(&ring(), &|name, cap| {
                calls.borrow_mut().push((name.to_string(), cap));
                Ok(None)
            })
            .unwrap()
            .is_none()
        );
        assert_eq!(*calls.borrow(), [("advisories".to_string(), MAX_BYTES)]);
        for signature in [
            None,
            Some(Vec::new()),
            Some(b"garbage".to_vec()),
            Some(vec![b'x'; super::super::SIGNATURE_MAX_BYTES + 1]),
        ] {
            let result = fetch(&ring(), &|name, _| {
                Ok(if name == "advisories" {
                    Some(FUTURE.to_vec())
                } else {
                    signature.clone()
                })
            });
            assert!(result.is_err());
        }
        for failure in [
            "HTTP 500",
            "connection refused",
            "DNS failure",
            "timeout",
            "over cap",
            "incomplete body",
            "HTTP URL refused",
        ] {
            for failed_name in ["advisories", "advisories.minisig"] {
                assert!(
                    fetch(&ring(), &|name, _| {
                        if name == failed_name {
                            anyhow::bail!("{failure}");
                        }
                        Ok(Some(FUTURE.to_vec()))
                    })
                    .is_err()
                );
            }
        }
        let value = fetch(&ring(), &|name, cap| {
            let body = if name == "advisories" {
                FUTURE
            } else {
                SIGNATURE
            };
            assert!(body.len() <= cap);
            Ok(Some(body.to_vec()))
        })
        .unwrap()
        .unwrap();
        assert!(value.reason("1.0.0").is_some());
    }
}

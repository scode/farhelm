//! The scanner and comparison against tables of example labels, the cases the hover's current mismatch check pins,
//! and a linear-time budget.
//!
//! The tables are the quiet-side specification as much as the loud one: a label they list as mentioning no address
//! must keep mentioning none, or honest links start warning.

use super::*;

/// One label on one row.
fn row(text: &str) -> [RowText<'_>; 1] {
    [RowText {
        text,
        seam_before: None,
    }]
}

/// What the scanner found, in a form a table can spell: the raw text, the trimmed text, the host text, the kind and
/// whether it has userinfo.
type Found = (String, String, String, AddressKind, bool);

fn found(label: &str) -> Vec<Found> {
    addresses(&row(label), &[])
        .into_iter()
        .map(|address| {
            (
                label[address.span].to_owned(),
                label[address.trimmed].to_owned(),
                label[address.host].to_owned(),
                address.kind,
                address.has_userinfo,
            )
        })
        .collect()
}

fn one(span: &str, trimmed: &str, host: &str, kind: AddressKind, userinfo: bool) -> Vec<Found> {
    vec![(span.into(), trimmed.into(), host.into(), kind, userinfo)]
}

fn verdict(label: &str, target: &str) -> Option<MismatchKind> {
    let target = Url::parse(target).unwrap();
    judge(&row(label), &[], &target)
        .first_mismatch
        .map(|(_, kind)| kind)
}

/// The hover's current check's signature, so the cases ported from it keep their assertions word for word.
fn mismatch(label: &str, uri: &str) -> bool {
    verdict(label, uri).is_some()
}

use AddressKind::{Host, Scheme, Www};

// ---------------------------------------------------------------------------------------------------------------
// Finding addresses
// ---------------------------------------------------------------------------------------------------------------

/// What each example label mentions, with exact spans, trimmed forms, hosts and kinds: the warning's reason and the
/// text a hover quotes come from them, and a label that silently yields one address too many or too few is either a
/// false alarm or a missed spoof. Covers prose, wrappers, decoration, userinfo, suppressed emails and remotes, file
/// names and code that must stay quiet, and the explicit spellings that count.
#[test]
fn example_labels_mention_exactly_these_addresses() {
    let github_login = one(
        "https://github.com/login",
        "https://github.com/login",
        "github.com",
        Scheme,
        false,
    );
    let github_x = one(
        "https://github.com/x",
        "https://github.com/x",
        "github.com",
        Scheme,
        false,
    );
    let rows: Vec<(&str, Vec<Found>)> = vec![
        // Prose, list markers and a label word glued with a colon around one address.
        ("See https://github.com/login", github_login.clone()),
        ("- https://github.com/login", github_login.clone()),
        (
            "Login:github.com/login",
            one(
                "github.com/login",
                "github.com/login",
                "github.com",
                Host,
                false,
            ),
        ),
        // Wrappers stay out of the address.
        (
            "[docs](https://github.com/)",
            one(
                "https://github.com/",
                "https://github.com/",
                "github.com",
                Scheme,
                false,
            ),
        ),
        (
            "ticket-42(https://x/t/42)",
            one("https://x/t/42", "https://x/t/42", "x", Scheme, false),
        ),
        (
            "(https://github.com/).",
            one(
                "https://github.com/",
                "https://github.com/",
                "github.com",
                Scheme,
                false,
            ),
        ),
        // Unicode quotes, an emoji and an arrow before the address.
        ("\u{201c}https://github.com/x\u{201d}", github_x.clone()),
        ("\u{1f517} https://github.com/x", github_x.clone()),
        ("\u{27a1}\u{fe0f}https://github.com/x", github_x.clone()),
        // Trailing punctuation is in the raw form and out of the trimmed one.
        (
            "https://github.com/guide.",
            one(
                "https://github.com/guide.",
                "https://github.com/guide",
                "github.com",
                Scheme,
                false,
            ),
        ),
        (
            "\u{8be6}\u{89c1}https://github.com/x\u{ff0c}",
            one(
                "https://github.com/x\u{ff0c}",
                "https://github.com/x",
                "github.com",
                Scheme,
                false,
            ),
        ),
        // A familiar name before an `@` is userinfo, and the host is what follows.
        (
            "support@paypal.com/login",
            one(
                "support@paypal.com/login",
                "support@paypal.com/login",
                "paypal.com",
                Host,
                true,
            ),
        ),
        (
            "example.test@other.test/x",
            one(
                "example.test@other.test/x",
                "example.test@other.test/x",
                "other.test",
                Host,
                true,
            ),
        ),
        // An email in the query belongs to the URL.
        (
            "https://paypal.com/signin?email=you@example.com",
            one(
                "https://paypal.com/signin?email=you@example.com",
                "https://paypal.com/signin?email=you@example.com",
                "paypal.com",
                Scheme,
                false,
            ),
        ),
        // Balanced brackets stay in; the wrapper's closer doesn't.
        (
            "(https://x/page_(name))",
            one(
                "https://x/page_(name)",
                "https://x/page_(name)",
                "x",
                Scheme,
                false,
            ),
        ),
        (
            "[https://x/][docs]",
            one("https://x/", "https://x/", "x", Scheme, false),
        ),
        // A backslash ends an http authority, as it does for the browser.
        (
            "https://evil.test\\@github.com/",
            one(
                "https://evil.test\\@github.com/",
                "https://evil.test\\@github.com/",
                "evil.test",
                Scheme,
                false,
            ),
        ),
        (
            "github.com@evil.test/x",
            one(
                "github.com@evil.test/x",
                "github.com@evil.test/x",
                "evil.test",
                Host,
                true,
            ),
        ),
        (
            "github.com:p^ss@evil.test/x",
            one(
                "github.com:p^ss@evil.test/x",
                "github.com:p^ss@evil.test/x",
                "evil.test",
                Host,
                true,
            ),
        ),
        (
            "https://github.com@evil.test/",
            one(
                "https://github.com@evil.test/",
                "https://github.com@evil.test/",
                "evil.test",
                Scheme,
                true,
            ),
        ),
        // Emails, a mailto, a handle and an SCP remote claim nothing.
        ("alice@example.com", vec![]),
        ("john.dev@example.com", vec![]),
        ("github.com@evil.com", vec![]),
        ("mailto:alice@example.com", vec![]),
        ("@nytimes.com", vec![]),
        ("git@github.com:org/repo.git", vec![]),
        // File names, code, versions, prose and bare local names mention no address.
        ("README.md", vec![]),
        ("main.rs:12:5", vec![]),
        ("java.io.File", vec![]),
        ("logger.info", vec![]),
        ("Farhelm.app", vec![]),
        ("1.2.3/", vec![]),
        ("Python 3.12/3.13", vec![]),
        ("localhost", vec![]),
        ("127.1:8080/", vec![]),
        ("click here", vec![]),
        ("#4746", vec![]),
        // One address each.
        (
            "paypal.com",
            one("paypal.com", "paypal.com", "paypal.com", Host, false),
        ),
        (
            "github.com:8443",
            one(
                "github.com:8443",
                "github.com:8443",
                "github.com",
                Host,
                false,
            ),
        ),
        (
            "docs.rs/src/main.rs",
            one(
                "docs.rs/src/main.rs",
                "docs.rs/src/main.rs",
                "docs.rs",
                Host,
                false,
            ),
        ),
        (
            "192.168.0.1/",
            one("192.168.0.1/", "192.168.0.1/", "192.168.0.1", Host, false),
        ),
        (
            "localhost:3000/",
            one(
                "localhost:3000/",
                "localhost:3000/",
                "localhost",
                Host,
                false,
            ),
        ),
        (
            "//intranet/",
            one("//intranet/", "//intranet/", "intranet", Scheme, false),
        ),
        (
            "https://intranet/",
            one(
                "https://intranet/",
                "https://intranet/",
                "intranet",
                Scheme,
                false,
            ),
        ),
    ];
    for (label, expected) in rows {
        assert_eq!(found(label), expected, "{label:?}");
    }
}

/// The edges of recognition: where a scheme starts, which starts a host may have, which `@` makes userinfo, what
/// suppression covers and which endings count. Each one is a decided edge a later change could move by accident.
#[test]
fn recognition_edges_hold() {
    // A scheme starts at its run's first letter, or after other-script prose glued to it.
    for label in [
        "1.https://a.test/",
        "-https://a.test/",
        "__https://a.test/",
        "\u{8a73}\u{7d30}\u{306f}https://a.test/",
    ] {
        assert_eq!(
            found(label),
            one(
                "https://a.test/",
                "https://a.test/",
                "a.test",
                Scheme,
                false
            ),
            "{label:?}"
        );
    }
    assert_eq!(
        found("Visithttps://a.test/"),
        one(
            "Visithttps://a.test/",
            "Visithttps://a.test/",
            "a.test",
            Scheme,
            false
        )
    );
    // A Cyrillic letter in the scheme keeps it whole, and it doesn't parse.
    assert_eq!(
        found("htt\u{440}s://a.test/x"),
        one(
            "htt\u{440}s://a.test/x",
            "htt\u{440}s://a.test/x",
            "a.test",
            Scheme,
            false
        )
    );
    // A scheme needs something after `://`; `file:///tmp/x` has it, and no host.
    assert_eq!(found("Format: https://"), vec![]);
    assert_eq!(
        found("file:///tmp/x"),
        one("file:///tmp/x", "file:///tmp/x", "", Scheme, false)
    );
    assert_eq!(
        found("https://example.test:bogus/"),
        one(
            "https://example.test:bogus/",
            "https://example.test:bogus/",
            "example.test",
            Scheme,
            false
        )
    );
    // Spellings browsers accept without `//`.
    assert_eq!(
        found("https:github.com"),
        one(
            "https:github.com",
            "https:github.com",
            "github.com",
            Scheme,
            false
        )
    );
    assert_eq!(
        found("https:/github.com"),
        one(
            "https:/github.com",
            "https:/github.com",
            "github.com",
            Scheme,
            false
        )
    );
    // Where a host may start.
    assert_eq!(
        found("\u{2026}github.com"),
        one("github.com", "github.com", "github.com", Host, false)
    );
    assert_eq!(
        found(".evil.test/x"),
        one("evil.test/x", "evil.test/x", "evil.test", Host, false)
    );
    assert_eq!(
        found("_github.com_"),
        one("github.com_", "github.com", "github.com_", Host, false)
    );
    assert_eq!(
        found("Login:github.com/"),
        one("github.com/", "github.com/", "github.com", Host, false)
    );
    assert_eq!(
        found("(github.com)"),
        one("github.com", "github.com", "github.com", Host, false)
    );
    for label in ["/tmp/example.test/x", "src/docs.rs/x", "@nytimes.com"] {
        assert_eq!(found(label), vec![], "{label:?}");
    }
    // An `@` before a host that counts on its own makes the text before it userinfo, from after the last terminator.
    assert_eq!(
        found("\"support@paypal.com/login\""),
        one(
            "support@paypal.com/login",
            "support@paypal.com/login",
            "paypal.com",
            Host,
            true
        )
    );
    assert_eq!(
        found("example.test:secret@other.test/x"),
        one(
            "example.test:secret@other.test/x",
            "example.test:secret@other.test/x",
            "other.test",
            Host,
            true
        )
    );
    assert_eq!(
        found("example.test\u{ff20}other.test/x"),
        one(
            "example.test\u{ff20}other.test/x",
            "example.test\u{ff20}other.test/x",
            "other.test",
            Host,
            true
        )
    );
    // With nothing before the `@` there is no userinfo, and no address.
    assert_eq!(found("@nytimes.com/x"), vec![]);
    // A local part starting with `www.` is a site name, not an email.
    assert_eq!(
        found("www.paypal.com@evil.test"),
        one(
            "www.paypal.com@evil.test",
            "www.paypal.com@evil.test",
            "evil.test",
            Www,
            true
        )
    );
    assert_eq!(found("Email:bob@corp.com"), vec![]);
    assert_eq!(found("git@github.com:docs.rs/src"), vec![]);
    // An address anchored before a suppressed span extends over it.
    assert_eq!(
        found("github.com:p@evil.com"),
        one(
            "github.com:p@evil.com",
            "github.com:p@evil.com",
            "evil.com",
            Host,
            true
        )
    );
    // An excluded two-letter ending still counts after a generic second level.
    assert_eq!(
        found("google.co.in"),
        one("google.co.in", "google.co.in", "google.co.in", Host, false)
    );
    assert_eq!(found("example.in"), vec![]);
    // A look-alike ending counts with nothing after it; a real internationalized ending needs a path.
    assert_eq!(
        found("paypal.c\u{43e}m"),
        one(
            "paypal.c\u{43e}m",
            "paypal.c\u{43e}m",
            "paypal.c\u{43e}m",
            Host,
            false
        )
    );
    assert_eq!(
        found("\u{43f}\u{440}\u{438}\u{43c}\u{435}\u{440}.\u{440}\u{444}"),
        vec![]
    );
    assert_eq!(
        found("\u{43f}\u{440}\u{438}\u{43c}\u{435}\u{440}.\u{440}\u{444}/x"),
        one(
            "\u{43f}\u{440}\u{438}\u{43c}\u{435}\u{440}.\u{440}\u{444}/x",
            "\u{43f}\u{440}\u{438}\u{43c}\u{435}\u{440}.\u{440}\u{444}/x",
            "\u{43f}\u{440}\u{438}\u{43c}\u{435}\u{440}.\u{440}\u{444}",
            Host,
            false
        )
    );
    // `www.` needs two labels after it.
    assert_eq!(
        found("www.example.test/x"),
        one(
            "www.example.test/x",
            "www.example.test/x",
            "www.example.test",
            Www,
            false
        )
    );
    assert_eq!(found("www.intranet/x"), vec![]);
    // A bracketed IPv6 address counts.
    assert_eq!(
        found("[::1]:8080/x"),
        one("[::1]:8080/x", "[::1]:8080/x", "[::1]", Host, false)
    );
}

/// Tokens end at whitespace and at a seam the caller says separates; any other seam joins the rows into one token.
/// That is what lets `Visit` above `https://github.com/` read as prose before an address in one layout and as the
/// glued scheme `Visithttps` in another.
#[test]
fn separating_seams_cut_tokens_and_joining_seams_do_not() {
    let rows = [
        RowText {
            text: "Visit",
            seam_before: None,
        },
        RowText {
            text: "https://github.com/",
            seam_before: Some(Seam::Hard),
        },
    ];
    let label = label_text(&rows);
    let spans = |separate: &[bool]| -> Vec<String> {
        addresses(&rows, separate)
            .into_iter()
            .map(|address| label[address.span].to_owned())
            .collect()
    };
    assert_eq!(spans(&[true]), ["https://github.com/"]);
    assert_eq!(spans(&[false]), ["Visithttps://github.com/"]);
    assert_eq!(spans(&[]), ["Visithttps://github.com/"]);
    let soft = [
        rows[0],
        RowText {
            seam_before: Some(Seam::Soft),
            ..rows[1]
        },
    ];
    assert_eq!(
        addresses(&soft, &[true]).len(),
        1,
        "a soft seam joins whatever `separate` says, since it holds entries for hard seams only"
    );
    assert_eq!(
        label[addresses(&soft, &[true])[0].span.clone()].to_owned(),
        "Visithttps://github.com/"
    );
    // U+FEFF is not whitespace here, so it can't split a disguised host into harmless words.
    assert_eq!(found("https://exa\u{feff}mple.com/x").len(), 1);
}

// ---------------------------------------------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------------------------------------------

/// The verdict on each example label against its destination, with the reason each warning gives. The reason is
/// asserted, not just warn or quiet, because the hover's wording comes from it: a disguise reported as a plain mismatch
/// hides why the link is dangerous.
#[test]
fn example_labels_get_these_verdicts() {
    use MismatchKind::{Credentials, Disguised, Mismatch};
    let rows: [(&str, &str, Option<MismatchKind>); 22] = [
        (
            "See https://github.com/login",
            "https://evil.test/",
            Some(Mismatch),
        ),
        (
            "See https://github.com/login",
            "https://github.com/login",
            None,
        ),
        ("Read more on github.com", "https://github.com/o/r", None),
        (
            "Read more on github.com",
            "https://evil.test/",
            Some(Mismatch),
        ),
        ("github.com?", "https://github.com/o/r", Some(Mismatch)),
        ("github.com:443", "https://github.com/o/r", Some(Mismatch)),
        (
            "Fix redirects from https://example.com/login",
            "https://github.com/o/r/issues/42",
            Some(Mismatch),
        ),
        (
            "\u{201c}https://github.com/x\u{201d}",
            "https://github.com/x",
            None,
        ),
        (
            "https://github.com/guide.",
            "https://github.com/guide",
            None,
        ),
        (
            "\u{8be6}\u{89c1}https://github.com/x\u{ff0c}",
            "https://github.com/x",
            None,
        ),
        (
            "paypal.c\u{43e}m/login",
            "https://evil.test/",
            Some(Disguised),
        ),
        (
            "support@paypal.com/login",
            "https://evil.test/",
            Some(Credentials),
        ),
        (
            "https://paypal.com/signin?email=you@example.com",
            "https://evil.test/",
            Some(Mismatch),
        ),
        (
            "https://github.com/o/r/pull/\u{2026}",
            "https://github.com/o/r/pull/4746",
            Some(Mismatch),
        ),
        (
            "http://github.com/x",
            "https://github.com/x",
            Some(Mismatch),
        ),
        (
            "https://exa\u{feff}mple.com/x",
            "https://example.com/x",
            Some(Disguised),
        ),
        (
            "https://\u{ff45}xample.com/x",
            "https://example.com/x",
            Some(Disguised),
        ),
        (
            "https://ja.wikipedia.org/wiki/\u{6771}\u{4eac}",
            "https://ja.wikipedia.org/wiki/\u{6771}\u{4eac}",
            None,
        ),
        ("alice@example.com", "https://evil.test/", None),
        ("alice@example.com", "https://example.com/", None),
        // The look-alike ending stays disguised even against the look-alike destination itself.
        (
            "paypal.c\u{43e}m/login",
            "https://paypal.c\u{43e}m/login",
            Some(Disguised),
        ),
        // Credentials stay a mismatch even when the host after them is the destination's.
        (
            "support@paypal.com/login",
            "https://paypal.com/login",
            Some(Credentials),
        ),
    ];
    for (label, target, expected) in rows {
        assert_eq!(verdict(label, target), expected, "{label:?} -> {target:?}");
    }
}

/// The non-ASCII-ending clause exists for look-alike endings, which only Latin, Greek and Cyrillic letters can
/// make. Japanese and Chinese prose has no spaces and uses `。`, which recognition folds to a dot, so a clause open to
/// every script would read a plain sentence as a disguised host and warn on honest labels. Pins both directions:
/// prose in those scripts, with `。` or an ASCII dot, mentions nothing, while Cyrillic and Greek look-alike endings
/// still count and are disguised.
#[test]
fn only_confusable_scripts_make_a_look_alike_ending() {
    for label in [
        "\u{3053}\u{308c}\u{306f}\u{6587}\u{3067}\u{3059}\u{3002}\u{6b21}\u{306e}\u{6587}",
        "\u{6587}\u{3067}\u{3059}.\u{6b21}",
        "\u{8bf7}\u{770b}\u{3002}\u{6587}\u{6863}",
    ] {
        assert_eq!(found(label), vec![], "{label:?}");
        assert_eq!(verdict(label, "https://evil.test/"), None, "{label:?}");
    }
    // A Cyrillic `о` and a Greek `ο` in place of the `o` of `com`.
    for label in ["paypal.c\u{43e}m", "paypal.c\u{3bf}m/login"] {
        assert_eq!(found(label).len(), 1, "{label:?}");
        assert_eq!(
            verdict(label, "https://evil.test/"),
            Some(MismatchKind::Disguised),
            "{label:?}"
        );
    }
}

/// Only sentence punctuation, emphasis characters and non-ASCII decoration are trimmed. Any other trailing ASCII
/// punctuation is part of what a browser opens, so trimming it would make a label that names a different URL match:
/// an extra trailing `/` or a dangling `&` must still warn under strict comparison.
#[test]
fn trailing_url_punctuation_is_not_decoration() {
    for (label, target) in [
        ("https://x.test/a//", "https://x.test/a"),
        ("https://x.test/a?q=1&", "https://x.test/a?q=1"),
        ("https://x.test/a-", "https://x.test/a"),
        ("https://x.test/a=", "https://x.test/a"),
        ("https://x.test/a%", "https://x.test/a"),
        ("https://x.test/a+", "https://x.test/a"),
        ("https://x.test/a@", "https://x.test/a"),
    ] {
        let address = &addresses(&row(label), &[])[0];
        assert_eq!(address.trimmed, address.span, "{label:?}");
        assert_eq!(
            verdict(label, target),
            Some(MismatchKind::Mismatch),
            "{label:?}"
        );
    }
    // A trailing `#` stays too; it matches only because an empty fragment is immaterial to the comparison.
    let address = &addresses(&row("https://x.test/a#"), &[])[0];
    assert_eq!(address.trimmed, address.span);
    assert_eq!(verdict("https://x.test/a#", "https://x.test/a"), None);
    // Sentence punctuation and non-ASCII decoration still come off.
    for label in [
        "https://x.test/a.",
        "https://x.test/a,",
        "https://x.test/a*",
        "https://x.test/a\u{2026}",
        "https://x.test/a\u{3002}",
        "https://x.test/a\u{2192}",
    ] {
        assert_eq!(verdict(label, "https://x.test/a"), None, "{label:?}");
    }
}

/// Every address counts, not just the first: one honest address doesn't vouch for a second that goes elsewhere, and
/// the count feeds the hover's "and N more".
#[test]
fn every_address_is_judged_and_counted() {
    let target = Url::parse("https://a.test/").unwrap();
    let judgement = judge(
        &row("https://a.test/ or https://b.test/ or c.test/"),
        &[],
        &target,
    );
    assert_eq!(judgement.addresses.len(), 3);
    assert_eq!(judgement.first_mismatch, Some((1, MismatchKind::Mismatch)));
    assert_eq!(judgement.mismatches, 2);
}

/// A handle whose host is followed by a port is userinfo before that host, just as an email shape followed by a
/// port is. Suppressing it as a handle would hide the credentials `@` when the name in front of it doesn't count on
/// its own, and the hover's current check warns on all of these.
#[test]
fn an_at_before_a_host_with_a_port_is_userinfo_not_a_handle() {
    for (label, target) in [
        ("notion.so:@evil.com:443/login", "https://evil.com/login"),
        (
            "example.test:@evil.test:8443/login",
            "https://evil.test:8443/login",
        ),
        (
            "example.test:example!@other.test:8443/x",
            "https://other.test:8443/x",
        ),
        ("notion.so!@evil.com:443/login", "https://evil.com/login"),
    ] {
        assert_eq!(
            verdict(label, target),
            Some(MismatchKind::Credentials),
            "{label:?}"
        );
    }
    // With nothing before the `@` there is still no userinfo and no address.
    assert_eq!(found("@evil.test:8443/x"), vec![]);
}

/// Where a host may start is judged on the original characters, not the recognition view. Folding turns `…` into three
/// dots and a fullwidth solidus into `/`, which would glue the host to the word before it and hide the address.
#[test]
fn a_host_start_is_judged_on_the_original_characters() {
    for (label, span) in [
        ("Loading\u{2026}github.com/x", "github.com/x"),
        ("a\u{2026}github.com", "github.com"),
        ("\u{ff0f}github.com", "github.com"),
    ] {
        let found = found(label);
        assert_eq!(found.len(), 1, "{label:?}");
        assert_eq!(
            (found[0].0.as_str(), found[0].2.as_str()),
            (span, "github.com"),
            "{label:?}"
        );
        assert!(verdict(label, "https://evil.test/").is_some(), "{label:?}");
    }
}

/// The reason comes from the trimmed form. Sentence punctuation that folding changes (`。`, `…`, `，`) sits in a
/// host-only address's authority, so the raw form reads as disguised; the honest reason for these is a plain
/// mismatch, and a hover saying "look-alike characters" about a full stop would be wrong.
#[test]
fn trailing_punctuation_that_folds_does_not_make_a_mismatch_disguised() {
    for label in [
        "\u{8be6}\u{89c1}https://evil.test\u{3002}",
        "See https://evil.test\u{2026}",
        "https://evil.test\u{ff0c}",
    ] {
        assert_eq!(
            verdict(label, "https://github.com/"),
            Some(MismatchKind::Mismatch),
            "{label:?}"
        );
    }
}

/// Trailing characters that fold to nothing draw nothing and leave the address, raw form included, as a leading one
/// never enters it. Otherwise they would be percent-encoded into the path and an honest label would warn, even one
/// whose own trimming would take decoration off past what it names (`/😀` before the invisible character).
#[test]
fn trailing_invisible_characters_leave_the_address() {
    let label = " https://example.com/x\u{feff}";
    assert_eq!(
        found(label),
        one(
            "https://example.com/x",
            "https://example.com/x",
            "example.com",
            Scheme,
            false
        )
    );
    for (label, target) in [
        (" https://example.com/x\u{feff}", "https://example.com/x"),
        (
            " \thttps://example.com/\u{1f600}\u{feff}",
            "https://example.com/\u{1f600}",
        ),
        (
            "https://en.wikipedia.org/wiki/Yahoo!\u{200b}",
            "https://en.wikipedia.org/wiki/Yahoo!",
        ),
    ] {
        assert_eq!(verdict(label, target), None, "{label:?}");
    }
}

/// A look-alike ending needs the `paypal.cоm` signature: a last label of at least two letters holding a Latin, Greek or
/// Cyrillic non-ASCII letter, in a name that also has ASCII letters. Glued abbreviations in Cyrillic, Greek or German
/// prose have no ASCII letters or a one-letter ending, and must not read as disguised hosts.
#[test]
fn prose_abbreviations_are_not_look_alike_endings() {
    for label in [
        "\u{41f}\u{43e}\u{434}\u{440}\u{43e}\u{431}\u{43d}\u{435}\u{435}, \u{442}.\u{435}. \u{434}\u{43e}\u{43a}",
        "\u{41e}\u{444}\u{438}\u{441}: \u{443}\u{43b}.\u{41b}\u{435}\u{43d}\u{438}\u{43d}\u{430}",
        "\u{3b4}\u{3b5}\u{3af}\u{3c4}\u{3b5} \u{3c0}.\u{3c7}. \u{3c4}\u{3bf}\u{3bd}",
        "Preise u.\u{e4}. siehe Liste",
    ] {
        assert_eq!(found(label), vec![], "{label:?}");
    }
    // `paypal.cоm` (Cyrillic о), `paypal.cοm` (Greek ο), and `раураl.сом` (Cyrillic but for an ASCII `l`).
    for label in [
        "paypal.c\u{43e}m",
        "paypal.c\u{3bf}m/login",
        "\u{440}\u{430}\u{443}\u{440}\u{430}l.\u{441}\u{43e}\u{43c}",
    ] {
        assert_eq!(found(label).len(), 1, "{label:?}");
        assert_eq!(
            verdict(label, "https://evil.test/"),
            Some(MismatchKind::Disguised),
            "{label:?}"
        );
    }
}

/// Accented Latin letters are how German, French and Spanish are spelled, so a sentence glued across a full stop must
/// not read as a host with a look-alike ending. Only letters that pass for a plain ASCII letter (Cyrillic, Greek, and
/// Latin ones that don't decompose to an ASCII letter) make one.
#[test]
fn accented_latin_letters_do_not_make_a_look_alike_ending() {
    for label in [
        "Das ist gut.\u{dc}brigens",
        "Ver m\u{e1}s.Informaci\u{f3}n",
        "C'est fini.Apr\u{e8}s",
        "z.B.\u{c4}nderungen",
        "fichier.\u{e9}t\u{e9}",
        "Caf\u{e9}.R\u{e9}sum\u{e9}",
        "Stra\u{df}e.K\u{f6}ln",
    ] {
        assert_eq!(found(label), vec![], "{label:?}");
        assert_eq!(verdict(label, "https://evil.test/"), None, "{label:?}");
    }
    for label in [
        "paypal.c\u{43e}m",
        "paypal.c\u{3bf}m/login",
        "\u{440}\u{430}\u{443}\u{440}\u{430}l.\u{441}\u{43e}\u{43c}",
        "github.\u{131}o",
    ] {
        assert_eq!(found(label).len(), 1, "{label:?}");
        assert_eq!(
            verdict(label, "https://evil.test/"),
            Some(MismatchKind::Disguised),
            "{label:?}"
        );
    }
}

/// A host's start is allowed when either the original characters or the recognition view allow it. Compatibility
/// characters such as `⑴` and `㈠` fold to bracketed numbers, and a modifier-letter colon or a Greek iota subscript is a
/// letter as written but punctuation once folded; with either reading alone, each would glue `paypal.com` to the text
/// before it and hide it. A path or a handle still blocks the start in both readings.
#[test]
fn a_host_starts_when_either_reading_allows_it() {
    for label in [
        "\u{2474}paypal.com/login",
        "\u{2474}paypal.com",
        "\u{2474}.paypal.com/login",
        "\u{3220}paypal.com/login",
        "Login\u{2d0}paypal.com/login",
        "x\u{37a}paypal.com/login",
    ] {
        let found = found(label);
        assert_eq!(found.len(), 1, "{label:?}");
        assert_eq!(found[0].2, "paypal.com", "{label:?}");
        assert!(verdict(label, "https://evil.test/").is_some(), "{label:?}");
    }
    for label in ["/tmp/example.test/x", "src/docs.rs/x", "@nytimes.com"] {
        assert_eq!(found(label), vec![], "{label:?}");
    }
}

/// A scheme followed by nothing but decoration is a word about URLs, not an address that fails to parse and warns.
#[test]
fn a_scheme_followed_only_by_decoration_is_no_address() {
    for label in [
        "Use https://, not http://",
        "Links start with https://.",
        "https://\u{2026}",
    ] {
        assert_eq!(found(label), vec![], "{label:?}");
        assert_eq!(verdict(label, "https://evil.test/"), None, "{label:?}");
    }
}

/// A look-alike ending is disguised however the address is spelled, and even against the look-alike destination
/// itself: the label reads as the familiar site whichever anchor found it.
#[test]
fn a_look_alike_ending_is_disguised_in_every_spelling() {
    for label in [
        "paypal.c\u{43e}m/login",
        "https://paypal.c\u{43e}m/login",
        "www.paypal.c\u{43e}m/login",
        "x@paypal.c\u{43e}m/login",
    ] {
        for target in [
            "https://evil.test/",
            "https://paypal.c\u{43e}m/login",
            "https://www.paypal.c\u{43e}m/login",
        ] {
            assert_eq!(
                verdict(label, target),
                Some(MismatchKind::Disguised),
                "{label:?} -> {target:?}"
            );
        }
    }
}

/// Combining marks are spelling in scripts other than Latin, Greek and Cyrillic, and an accent typed decomposed is
/// the same text as its precomposed form. Neither may make an honest internationalized host disguised against itself;
/// a mark NFKC can't compose onto a Latin letter still does.
#[test]
fn honest_marks_do_not_disguise_a_host() {
    for (label, target) in [
        ("https://bu\u{308}cher.de/", "https://b\u{fc}cher.de/"),
        ("bu\u{308}cher.de", "https://b\u{fc}cher.de/x"),
        (
            "https://\u{939}\u{93f}\u{928}\u{94d}\u{926}\u{940}.\u{92d}\u{93e}\u{930}\u{924}/",
            "https://\u{939}\u{93f}\u{928}\u{94d}\u{926}\u{940}.\u{92d}\u{93e}\u{930}\u{924}/",
        ),
    ] {
        assert_eq!(verdict(label, target), None, "{label:?}");
    }
    assert_eq!(
        verdict("https://bu\u{34f}cher.de/", "https://bucher.de/"),
        Some(MismatchKind::Disguised)
    );
}

/// A scheme names nothing unless the extent can take what follows it: a scheme quoted in prose or closed by a bracket
/// is a word about URLs, not an address that fails to parse and warns.
#[test]
fn a_scheme_with_nothing_the_extent_takes_after_it_is_no_address() {
    for label in [
        "Prefix URLs with \"https://\"",
        "(see https://)",
        "http:/<x>",
    ] {
        assert_eq!(found(label), vec![], "{label:?}");
        assert_eq!(verdict(label, "https://evil.test/"), None, "{label:?}");
    }
}

/// The userinfo of an `@` address is everything from its start to the `@`, so the host is what follows the `@` even
/// when a `/` comes before it, and the reason is the credentials one.
#[test]
fn an_at_address_reads_its_host_after_the_at() {
    assert_eq!(
        found("a/b@paypal.com/login"),
        one(
            "a/b@paypal.com/login",
            "a/b@paypal.com/login",
            "paypal.com",
            Host,
            true
        )
    );
    assert_eq!(
        verdict("a/b@paypal.com/login", "https://paypal.com/login"),
        Some(MismatchKind::Credentials)
    );
}

/// The examples the documented behavior gives beside the tables: an unparseable address warns, Markdown emphasis
/// around an address matches, a division slash disguises, bare site names (with trailing punctuation or `www.`) are
/// host-only, a browser-accepted IPv4 spelling counts in an explicit URL, and strict comparison warns on an escaped
/// `~`, an added `www.` and an extra query or fragment.
#[test]
fn documented_examples_beside_the_tables_hold() {
    use MismatchKind::{Disguised, Mismatch};
    for (label, target, expected) in [
        (
            "https://example.test:bogus/",
            "https://example.test/",
            Some(Mismatch),
        ),
        ("*https://github.com/x*", "https://github.com/x", None),
        ("**github.com**", "https://github.com/o/r", None),
        ("~~https://github.com/x~~", "https://github.com/x", None),
        ("_https://github.com/x_", "https://github.com/x", None),
        (
            "github.com\u{2215}login",
            "https://github.com/login",
            Some(Disguised),
        ),
        ("github.com,", "https://github.com/o/r", None),
        ("www.github.com", "https://www.github.com/o/r", None),
        ("https://127.1/", "https://127.0.0.1/", None),
        (
            "https://a.test/%7euser",
            "https://a.test/~user",
            Some(Mismatch),
        ),
        (
            "https://www.github.com/x",
            "https://github.com/x",
            Some(Mismatch),
        ),
        (
            "https://github.com/x?a=1",
            "https://github.com/x",
            Some(Mismatch),
        ),
        (
            "https://github.com/x#y",
            "https://github.com/x",
            Some(Mismatch),
        ),
    ] {
        assert_eq!(verdict(label, target), expected, "{label:?} -> {target:?}");
    }
    for (label, host_only) in [
        ("github.com,", true),
        ("www.github.com", true),
        ("github.com?", false),
        ("github.com/x", false),
    ] {
        assert_eq!(
            addresses(&row(label), &[])[0].host_only,
            host_only,
            "{label:?}"
        );
    }
    assert_eq!(
        found("https://127.1/"),
        one("https://127.1/", "https://127.1/", "127.1", Scheme, false)
    );
}

/// The first mismatch carries the parsed form a hover marks the destination's difference against: the trimmed form's
/// serialization, with the destination's scheme lent to an address without one, and nothing when it doesn't parse.
#[test]
fn the_first_mismatch_carries_its_parsed_form() {
    let target = Url::parse("https://evil.test/").unwrap();
    let url = |label: &str| judge(&row(label), &[], &target).first_mismatch_url;
    assert_eq!(
        url("See https://github.com/guide."),
        Some("https://github.com/guide".into())
    );
    assert_eq!(
        url("github.com/x and https://evil.test/"),
        Some("https://github.com/x".into())
    );
    assert_eq!(url("https://example.test:bogus/"), None);
    assert_eq!(url("https://evil.test/"), None);
}

// ---------------------------------------------------------------------------------------------------------------
// Cases ported from the hover's current check (`link_disclosure.rs`), with identical assertions
// ---------------------------------------------------------------------------------------------------------------

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

/// Taking brackets that belong to a URL path (as wiki addresses use) would compare a different address than the one
/// shown, warning on an honest label or hiding a changed one.
#[test]
fn punctuation_inside_a_wrapped_address_is_part_of_the_path() {
    let label = "(https://example.test/page_(name))";
    assert!(!mismatch(label, "https://example.test/page_(name)"));
    assert!(mismatch(label, "https://example.test/page_name"));
}

/// Labels that imitate addresses with different characters, or put a familiar name in the credentials of an
/// unrelated host, must still warn.
#[test]
fn disguised_addresses_still_warn() {
    for label in [
        "htt\u{440}s://example.test/x",
        "https\u{2d0}//example.test/x",
        "example\u{3002}test/x",
        "ex\u{430}mple.test/x",
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
    for label in [
        "htt\u{440}s://example.test/x",
        "\u{200b}https://example.test/x",
    ] {
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

// ---------------------------------------------------------------------------------------------------------------
// Endings and linear time
// ---------------------------------------------------------------------------------------------------------------

/// The ending lists are data a refresh replaces, so pin that the snapshot still is IANA's file and that the derived
/// lists come out as documented in `endings.rs`: real endings include the reserved names and internationalized ones by
/// A-label, and bare ones leave out the excluded two-letter endings except after a generic second level.
#[test]
fn the_ending_lists_come_from_the_snapshot() {
    assert!(
        endings::snapshot_version().is_some(),
        "tlds.txt starts with IANA's version line"
    );
    for real in [
        "com",
        "info",
        "app",
        "rs",
        "md",
        "test",
        "example",
        "invalid",
        "localhost",
        "internal",
        "xn--p1ai",
    ] {
        assert!(endings::is_real(real), "{real}");
    }
    assert!(!endings::is_real("file"));
    assert_eq!(
        endings::a_label("\u{440}\u{444}").as_deref(),
        Some("xn--p1ai")
    );
    assert_eq!(endings::a_label("COM").as_deref(), Some("com"));
    for bare in [
        "com", "org", "net", "edu", "gov", "io", "dev", "co", "me", "uk", "de", "jp",
    ] {
        assert!(endings::is_bare(bare, None), "{bare}");
    }
    for excluded in ["md", "rs", "sh", "py", "in", "so", "ai", "info", "app"] {
        assert!(!endings::is_bare(excluded, None), "{excluded}");
    }
    assert!(endings::is_bare("in", Some("co")));
    assert!(endings::is_bare("in", Some("AC")));
    assert!(!endings::is_bare("in", Some("example")));
}

/// Scanning must stay linear in the label's length, or a program can stall the hover with a crafted label. The
/// inputs are the shapes that make a restarting scanner quadratic: many anchor candidates that each fail late, many
/// `@` signs, a long shared host after many possible userinfo starts, and a long run of trailing decoration. Each runs
/// at the label budget of 8,192 UTF-16 units and must cost at most twelve counted steps per unit. Twelve, not a
/// tighter figure, because the regex passes and some label checks count bytes while the budget counts UTF-16 units, so
/// a label of three-byte characters (`詳a` repeated before a scheme) costs more per unit than ASCII.
#[test]
fn scanning_takes_at_most_twelve_steps_per_unit() {
    const UNITS: usize = 8192;
    let inputs = [
        ",a".repeat(UNITS / 2),
        "x!".repeat(UNITS / 2),
        "@a".repeat(UNITS / 2),
        format!("{}@{}zz/", ",a".repeat(2000), "b.".repeat(2000)),
        format!("https://a.test/{}", "!".repeat(UNITS - 15)),
        format!("a.test/{}", ".".repeat(UNITS - 7)),
        "a.".repeat(UNITS / 2),
        "a@b.".repeat(UNITS / 4),
        "x@a.test:".repeat(UNITS / 9),
        "https://".repeat(UNITS / 8),
        "www.".repeat(UNITS / 4),
        format!("{}://x", "a".repeat(UNITS - 5)),
        format!("{}://x", "\u{8a73}a".repeat(UNITS / 2 - 3)),
    ];
    let target = Url::parse("https://a.test/").unwrap();
    for input in inputs {
        let units = input.encode_utf16().count();
        assert!(units <= UNITS, "{units}");
        take_steps();
        let judgement = judge(&row(&input), &[], &target);
        let steps = take_steps();
        assert!(
            steps <= 12 * units,
            "{steps} steps for {units} units starting {:?} ({} addresses)",
            input.chars().take(16).collect::<String>(),
            judgement.addresses.len()
        );
    }
}

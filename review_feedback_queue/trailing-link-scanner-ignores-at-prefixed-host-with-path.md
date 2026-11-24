# A host with a path after a bare `@` (`@paypal.com/login`) claims nothing

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

Link text `@paypal.com/login` (or `Sign in: @paypal.com/signin`) on a link that goes to an attacker's site shows no
warning, though the text plainly names paypal.com and a page on it. Without the leading `@` the same text warns.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer (2026-11-24): a warning bypass where the hover still shows the full real
destination.

### Reviewer record (`g01-scanner-src/f5`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/scan.rs:503`. Summary: a host with a path glued after a bare
`@` (`@paypal.com/login`) claims nothing, so a spoof prefixed with `@` is silent

Revision (turn 2, supersedes the previous turn-2 revision, whose last sentence lost two code spans to shell quoting):
adds the multi-row interaction and corrects the line to the tip's `at_sign` (scan.rs:503).

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has it
(scan.rs:503 `at_sign`, scan.rs:587 `host_may_start`, scan.rs:231 `suppressed`; test tests.rs:446 pins the behavior).

TLDR: Link text `@paypal.com/login` (or `Sign in: @paypal.com/signin`) on a link that goes to an attacker's site shows
no warning, though the text plainly names paypal.com and a page on it. Without the leading `@` the same text warns.

Details: `suppressed` does not treat `@paypal.com/login` as a handle, because a `/` follows. The `@` anchor
(`Scanner::at_sign`) computes the userinfo start with `Segments::start_before`; with nothing before the `@` it gets
`start >= at` and returns None ('nothing is put before the host'). The fallback `host()` at the `@` position fails (no
name starts there), and at `p` `host_may_start` treats `@` as a barrier in both readings, by design so that 'the `@`
anchor owns' it. Net: no anchor owns it, no Address, `judge` reports no mismatch. tests.rs:446 asserts
`found("@nytimes.com/x") == []`. SPEC.md (Terminal, OSC 8 paragraph) defines an address to include 'a dotted host with
any real ending followed by a path' found 'anywhere in the text ... glued to punctuation'; its claimless list names only
handles (`@name`), emails and SCP remotes, and the code itself declines to treat this shape as a handle. The gap list
('The warning is an aid, not a guarantee: ...') does not include it. To the URL parser `https://@paypal.com/login` is
paypal.com with empty userinfo. Fix: in `at_sign`, when there is no userinfo, anchor a Host address at the host after
the `@` (start = the cluster after `@`, no userinfo), or let `host_may_start` accept a host right after an `@` that
`suppressed` did not claim and that starts the token or follows a non-local character; change tests.rs:446 accordingly.

Multi-row interaction: a hyperlink a program continues on a new row itself, with a row ending in `@` above a row holding
`paypal.com/login`, has a first reading that separates the seam and mismatches, and a joined second reading
`@paypal.com/login` that finds no address; link_disclosure.rs treats that empty reading as a match and drops the warning
(see f9 and g03's finding (b)).

Suggested bucket: high. Defeats the spoof warning with one ordinary ASCII prefix character on a text that clearly names
a well-known site; not `highest` only because the hover still always shows the real target in full with its host
emphasized; if the maintainer treats a missed spoof warning as security-impacting in itself, `highest`.

Confidence: confirmed (source and the existing test establish that `@host/path` yields no address).

proposed_drop: none. The code comment calls it intended, but SPEC.md does not exempt it: it is neither a handle (the
code excludes the path-followed shape from handles) nor in the spec's gap list.

possible_cover: none known for the single-row case; the multi-row case overlaps g03's finding (b) and f9.

### Also reported by `g02-scanner-tests/f1`

Location: `crates/gui/farhelm-grid-model/src/link_claims/tests.rs:446`. Summary: Tests lock in that `@host/path` (e.g.
`@paypal.com/login`) mentions no address, so it never warns

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24 (tests and scanner); the tip
e159ffc79ab147be44e92a92b59571650a4f322d still has it.

TLDR: A program can underline the text `@paypal.com/login` (or `@bank.com:443/login`) and point the hyperlink at any
other site, and the hover gives no warning, although the text plainly names a familiar site with a path. One extra
character before the `@` (`x@paypal.com/login`) does warn, about login details.

Details: tests.rs:446 asserts `found("@nytimes.com/x") == vec![]` (comment: "With nothing before the `@` there is no
userinfo, and no address") and tests.rs:779 asserts `found("@evil.test:8443/x") == vec![]`. Source trace for
`@paypal.com/login` at the tip: `suppressed()` (scan.rs:231-275) finds `@paypal.com` with the handle alternative but
does not suppress it because `/` follows (scan.rs:252-255; the code's own handle definition, scan.rs:229-230, says a
handle 'isn't followed by `/`, `?`, `#` or `:` and a digit'). So it is not a handle. The `@` anchor (`at_sign`,
scan.rs:503-525) then gets `start = segments.start_before(0) = 0` and returns None because `start >= at` (scan.rs:515).
The host anchor at `paypal.com` is refused by `host_may_start` (scan.rs:587-615) because `@` is a barrier in both
readings. Net: no address, `judge` reports no mismatch against `https://evil.test/`. SPEC.md 'An address counts anywhere
in the text: ... or glued to punctuation', counts 'a dotted host whose ending is a common site ending' and lists only
'Email addresses, handles (`@name`) and SCP-style git remotes' as claiming nothing; its accepted-gap list (SPEC.md
~3638-3647) names a `_` glued before a host but not an `@`. `@paypal.com/login` is neither an email nor (by the
scanner's own definition) a handle, so the tests pin a quiet that the spec does not grant. Fix: treat an `@` with
nothing before it, followed by a qualifying host and a path or port, as an address (empty userinfo is still userinfo for
the URL parser: `https://@paypal.com/login` parses with host paypal.com), or at least let the host anchor start after
such an `@`; update tests 446/779 to assert a warning; or, if this is intended, add it to SPEC.md's accepted gaps.
Verify by reading the trace above; a one-line `verdict("@paypal.com/login", "https://evil.test/")` assertion would show
`None` today.

Suggested bucket: other. The authoritative destination display still shows the real host, so harm requires the user to
misread a correct display (bucket rule of 2026-10-22); if the maintainer counts a missed phishing warning as
security-impacting this would move to highest, which should be surfaced at triage.

Confidence: confirmed (source trace; not executed).

proposed_drop: none (possible argument: the maintainer may read `@name/...` as a handle under SPEC.md's 'handles
(`@name`) ... claim nothing'; the implementation's own handle definition excludes it).

possible_cover: none known. Overlaps g01-scanner-src (the scanner behavior itself).

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g01-scanner-src/f5`, `g02-scanner-tests/f1`.

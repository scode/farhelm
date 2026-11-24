# The spec's link comparison leeways are no longer tested

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e

## TLDR

Honest link text that differs from its destination only in ways the spec says to ignore (a trailing `/`, `%7e` versus
`%7E`, an empty `?` before `#`) is no longer covered by any test. A change that broke these rules would make honest
links start warning, and nothing would fail.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g02-scanner-tests/f13`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/tests.rs:1028`. Summary: Oracle's quiet side was dropped for
the scanner tables, which miss the spec's comparison leeways (trailing slash, escape case, empty `?`)

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e (dropped the oracle's quiet-side assertion), with the tables
from 18845174e7599d1def2461bccb38b94a175c5b24. The tip e159ffc79ab147be44e92a92b59571650a4f322d still has it.

TLDR: Honest link text that differs from its destination only in ways the spec says to ignore (a trailing `/`, `%7e`
versus `%7E`, an empty `?` before `#`) is no longer covered by any test. A change that broke these rules would make
honest links start warning, and nothing would fail.

Details: Before ba0da505, `pinned_disclosure_policy_oracle` asserted the old client's answers both ways
(`assert_eq!(mismatch(text, uri), expected)`), and the oracle's `policy` cases include quiet pairs such as
`https://example.com/x/` -> `https://example.com/x`, `https://example.com/x?#` -> `https://example.com/x`
(tests/fixtures/links-oracle.json, first entries). ba0da505 changed it to check only cases the old client warned on
(link_disclosure.rs:735-759), and its docstring (link_disclosure.rs:720-723) now says 'Answers it was quiet on are not
asserted; the address tables in `link_claims/tests.rs` are the quiet side's specification.' The scanner tests' module
docstring says the same ('The tables are the quiet-side specification as much as the loud one'). But SPEC.md's leeways
('only the URL parser's own normalization, one trailing slash, the case of percent-escapes and an empty `?` or `#` are
ignored'), implemented in `compare::comparable` (compare.rs:88-122), are asserted in link_claims/tests.rs only for the
empty `#` (`https://x.test/a#`, tests.rs ~713-716). No row pairs a label and destination that differ by one trailing
path slash, no row has `%7e` against `%7E` (only `%7e` against `~`, a mismatch, tests.rs:1053), and an empty `?` is only
ever removed as trailing decoration before comparison, never reaching `comparable`'s empty-query rule (an `?#` label
would). A grep of tests/links.rs and tests.rs found no such quiet-direction case either. Fix: add rows to
`example_labels_get_these_verdicts` or a dedicated test: `("https://a.test/x/", "https://a.test/x", None)`,
`("https://a.test/x", "https://a.test/x/", None)`, `("https://a.test/%7euser", "https://a.test/%7Euser", None)`,
`("https://a.test/x?#", "https://a.test/x", None)`, and the opposites that must still warn (`/x//`, already present).

Suggested bucket: other. Test coverage; current behavior is correct by source reading of compare.rs:88-122.

Confidence: confirmed (source and git history: `git log -L` on the oracle test shows the `assert_eq!` replaced in
ba0da505; not executed).

proposed_drop: none.

possible_cover: none known. The oracle test itself is g03-scope code; the gap is in this group's tables, which that test
names as the quiet-side spec.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g02-scanner-tests/f13`.

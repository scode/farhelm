# No test exercises the joined second reading of a multi-row link

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e

## TLDR

No user-visible effect today. SPEC.md says link text that mismatches when a row break is read as a space is read again
with the breaks joined, and only warns if that also mismatches. Nothing tests the quiet outcome, so if that second
reading broke, honest links that a program split over rows at a dot in the host would start warning, and no test would
notice.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Related: `trailing-link-disclosure-joined-reading-treats-no-address-as-match.md`; a quiet test and a loud
test should land with that fix.

### Reviewer record (`g04-disclosure-tests/f6`)

Location: `crates/gui/farhelm-grid-model/tests/links.rs:1189`. Summary: No test exercises the joined second reading that
quiets a label whose rows only match when every break is joined

Revision (turn 3): cross-group input says the rescue this finding calls untested also has a live defect (g03 f2 / g01
f9: a joined reading with no address counts as no mismatch, so a small glued fragment before a host on the next row
silences the warning). That defect is recorded by those groups and not here. It strengthens the point: a quiet-case test
and a loud-case test (fragment such as `x_` or `/` before `evil.test/x` on the next row) should land together when the
rescue is fixed. Substance and bucket otherwise unchanged; possible_cover now names g03 f2 / g01 f9 as the related
defect records.

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e (still at tip e159ffc79ab147be44e92a92b59571650a4f322d).

TLDR: No user-visible effect today. SPEC.md says link text that mismatches when a row break is read as a space is read
again with the breaks joined, and only warns if that also mismatches. Nothing tests the quiet outcome, so if that second
reading broke, honest links that a program split over rows at a dot in the host would start warning, and no test would
notice.

Details: link_disclosure.rs:593-601 (judge_complete) drops a first-reading mismatch when `separate != joined` and the
all-joined judgment has no mismatch. The docstring of labels_over_several_rows_join_and_separate_at_their_seams
(tests/links.rs:1182-1192) states the rule ('A label that mismatches read that way warns only if it also mismatches with
every break joined') and calls its cases 'the cases that decided the rule', but none of its cases (tests/links.rs
~1197-1281) has a first reading that mismatches and a joined reading that matches: every separating case either matches
in the first reading or mismatches in both. The only multi-row case that reaches the second judgment is oracle grid 3
(https://example.com above .evil.test/x to https://example.com/x), which mismatches in both and is excepted from the
safety check (see f1), and the oracle asserts nothing for answers the old client gave quietly. Replacing the rescue at
lines 595-601 with an unconditional mismatch would therefore leave the suite green. Fix: add a quiet case such as rows
`https://example.com` / `.evil.test/x` under a hyperlink to https://example.com.evil.test/x (SPEC.md names this as an
accepted quiet gap: 'a host continued after a break (`https://github.com` above `.evil.test/`) that joins into the
target is quiet'), or an honest host split at a dot such as `https://docs` / `.example.com/x` to
https://docs.example.com/x, asserting None from every row with the label showing the space.

Suggested bucket: other — missing test for documented behavior; no shipped bug shown.

Confidence: likely — premise: I enumerated the cases in labels_over_several_rows_join_and_separate_at_their_seams and
grepped tests/links.rs for other multi-row hard-break cases; the other tests with hard breaks
(a_bare_address_row_beside_a_bracketed_one_keeps_its_claim, repeated_quoted_addresses_each_match_the_destination,
revealing_a_second_stretch...) either match in the first reading or use a Break seam where both readings separate. I did
not run a mutation.

proposed_drop: none

possible_cover: none

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g04-disclosure-tests/f6`.

# A scanner test docstring misstates how a host start is judged

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

No user-visible effect. A test's explanation states the opposite of the rule the code follows, so a maintainer trusting
it could 'fix' the code into hiding addresses that are glued to folded punctuation, such as `⑴paypal.com`.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g02-scanner-tests/f15`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/tests.rs:782`. Summary: Test docstring says a host start is
judged on the original characters 'not the recognition view'; code and a later test use either reading

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has
it.

TLDR: No user-visible effect. A test's explanation states the opposite of the rule the code follows, so a maintainer
trusting it could 'fix' the code into hiding addresses that are glued to folded punctuation, such as `⑴paypal.com`.

Details: `a_host_start_is_judged_on_the_original_characters` (tests.rs:782-785) is documented as 'Where a host may start
is judged on the original characters, not the recognition view.' The rule in `host_may_start` (scan.rs:580-586, 587-615)
is that a host starts when either the original characters or the recognition view allow it, and the same file's
`a_host_starts_when_either_reading_allows_it` (tests.rs:909-916) pins the recognition-view half (`\u{2474}paypal.com`,
`Login\u{2d0}paypal.com`), warning that 'with either reading alone, each would glue `paypal.com` to the text before it
and hide it'. The assertions in the first test (`Loading…github.com/x`, `／github.com`) are correct; only its first
sentence is wrong. Following it literally (dropping the recognition-view check) would break the second test's spoof
cases. Fix: reword it to 'A host may start where the original characters allow it, even when the recognition view would
glue it to the word before: ...', and point to the either-reading test.

Suggested bucket: other. A misleading comment, though one sitting on a spoof-relevant rule.

Confidence: confirmed (source reading).

proposed_drop: none.

possible_cover: none.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g02-scanner-tests/f15`.

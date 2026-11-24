# The row-join letter test counts a spacing mark the scanner folds away, so an address is joined away in every reading

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e

## TLDR

as before. A familiar address on its own row, preceded by one spacing mark such as `ः`, under a row ending in `/` of the
same link, gets no warning.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer (2026-11-24); the reviewer suggested `highest`. Needs both this fix and
the joined-reading fix (`trailing-link-disclosure-joined-reading-treats-no-address-as-match.md`), or the scanner-side
fix in `trailing-link-scanner-spacing-mark-after-slash-hides-host.md`.

### Reviewer record (`g03-disclosure-src/f9`)

Location: `crates/gui/farhelm-grid-model/src/link_disclosure.rs:639`. Summary: A mark the scanner folds away still
counts as a letter for `separates`, so a row's address is joined away in every reading

Revision (turn 5): fix text corrected after g01 f14; route, bucket and confidence unchanged. The earlier records' TLDR,
Details and premise check stand.

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has
it.

TLDR: as before. A familiar address on its own row, preceded by one spacing mark such as `ः`, under a row ending in `/`
of the same link, gets no warning.

Fix (corrected): changing `separates` alone is not enough. If it judged "a letter before the address" by `\p{L}` on
cluster bases, the seam would separate and the first reading (`/ ःpaypal.com/login`) would find `paypal.com/login` and
mismatch, but the joined second reading `/ःpaypal.com/login` still finds no address (the mark joins the `/` cluster and
folds away; g01 f14), and the current `judge_complete` (link_disclosure.rs:595-600) cancels on that. Both parts are
required: (a) the letter test matching the scanner's notion of a letter, and (b) f2's containment rule, cancelling a
first-reading mismatch only when the joined reading covers that address with one that matches the destination. A
scanner-side change in `Scanner::host_may_start` (scan.rs:587) that stops a cell-occupying mark glued to `/` or `\\`
from hiding a host (g01 f14) would also close the single-row form and make (b) less critical, but (b) is still needed
for the `x_`/`/` routes of f2. I agree with g01's assessment.

Suggested bucket: highest; `high` if the maintainer treats the warning as secondary to the always-shown destination.

Confidence: confirmed (source and the width crate's documented rules; not executed).

proposed_drop: none.

possible_cover: g01 f14 (scanner-side half), f2 (containment rule).

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g03-disclosure-src/f9`.

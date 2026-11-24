# The joined second reading of a multi-row link cancels the warning when it finds no address at all

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e

## TLDR

as before. A program can show `paypal.com/login` on a row of its own, send the link elsewhere, and keep the hover quiet
by ending the row above with a small piece of the same link such as `x_` or `/`.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer (2026-11-24); g03 suggested `highest`. g01's first fix suggestion (cancel
only when the joined reading finds a matching address) is not enough; see g03's counterexample in the details.

### Reviewer record (`g03-disclosure-src/f2`)

Location: `crates/gui/farhelm-grid-model/src/link_disclosure.rs:593`. Summary: Joined reading quiets a mismatch by
making the address unrecognizable, not by matching

Revision (turn 3): the conclusion, bucket and evidence are unchanged; this adds a constraint on the fix after g01 f9
proposed that the joined reading cancel a warning only when it finds at least one matching address. That fix is not
enough.

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has
it.

TLDR: as before. A program can show `paypal.com/login` on a row of its own, send the link elsewhere, and keep the hover
quiet by ending the row above with a small piece of the same link such as `x_` or `/`.

Details: as in the earlier records (judge_complete, link_disclosure.rs:585-616, accepts a joined reading whose
`first_mismatch` is `None`; `link_claims::Judgement::first_mismatch` is `None` both when every address matches and when
there is none). A fix that cancels only when the joined reading finds at least one matching address still loses the
warning when row 0 carries an honest address before the barrier. Example, destination `https://evil.test/`: row 0
`https://evil.test/ see_` short of the edge (Hard seam), row 1 `paypal.com/login`. First reading (the seam separates,
link_disclosure.rs:625-642): `https://evil.test/` matches and `paypal.com/login` mismatches, so Mismatch. Joined
reading: `https://evil.test/ see_paypal.com/login`; `https://evil.test/` still matches, while `see_paypal.com/login`
holds no host (scan.rs:587-615, `_` after a letter), so `first_mismatch` is `None` with one matching address present.
Under the proposed fix the hover is quiet. The fix has to tie the cancellation to the mismatching address itself: cancel
only when each first-reading mismatching address lies inside (its span is covered by) a joined-reading address that
matches the destination, which is the honest case the rule exists for (a host or path the program broke over rows
joining into the destination). Equivalently, any first-reading mismatch whose text the joined reading leaves outside
every matching address keeps the warning.

Suggested bucket: highest; `high` if the maintainer treats the warning as secondary to the always-shown destination.

Confidence: confirmed for the route (source); the fix constraint is a design argument from the same trace.

proposed_drop: none. The code follows SPEC.md 3616-3617 ("warns only when both readings mismatch"), but its stated
purpose and listed accepted limits concern a join into the target, not a join that erases the address.

possible_cover: g01 f9 (same root cause from the scanner side, with the weaker fix); g04 f6 (test gap).

### Also reported by `g01-scanner-src/f9`

Location:
`crates/gui/farhelm-grid-model/src/link_claims.rs (Judgement); crates/gui/farhelm-grid-model/src/link_disclosure.rs:592-600`.
Summary: `Judgement::first_mismatch == None` means both 'all match' and 'no address', and the joined second reading
treats the latter as exoneration; many scanner barriers make the joined reading empty

Revision (turn 5): fix text corrected after g03's counterexample. Letting the joined reading cancel whenever it finds at
least one address and no mismatch is not enough: with destination `https://evil.test/`, rows `https://evil.test/ see_`
and `paypal.com/login` give a joined reading whose only address is the matching `https://evil.test/`, while
`see_paypal.com/login` hides the second address. The fix should let the joined reading cancel only when every
mismatching address of the first reading lies inside (its span is covered by) an address of the joined reading that
matches the destination. Both readings index the same `label_text`, since separating seams only add token cuts and not
characters, so the span comparison is direct. Also see f14: a spacing mark after `/` empties the joined reading the same
way.

Scanner-side analysis of g03's cross-group finding (b) (orchestrator input, turn 2).

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24 introduced `Judgement` with the conflated meaning;
ba0da50536236ca934f3956d67a0c35e4b57586e made link_disclosure.rs consume it in the two-reading rule. The tip
e159ffc79ab147be44e92a92b59571650a4f322d still has both.

TLDR: When a program splits its link text over rows itself, a tiny fragment at the end of the earlier row can make the
address on the next row disappear in the 'everything joined' reading, and that empty reading is taken as proof the link
is honest, so a link whose text says `paypal.com/login` on a row of its own but goes elsewhere shows no warning.

Details: `Judgement::first_mismatch` is documented (link_claims.rs, `Judgement`) as '`None` means the label matches:
every address names the destination, or there are none.' `judge_complete` (link_disclosure.rs:592-600) drops the first
reading's mismatch whenever `link_claims::judge(&rows, &joined, target).first_mismatch.is_none()`, so a joined reading
with zero addresses exonerates. The first reading separates the seam because `separates` (link_disclosure.rs ~625-641)
finds `paypal.com/login` with no letter before it. In the joined reading, any glued prefix that `link_claims` refuses to
read through empties it. Scanner rules that do so (each established in scan.rs at the tip): (1) `_` (and `* ~ .` runs)
after a letter: `x_` + `paypal.com/login` -> `x_paypal.com/login`, hidden by `host_may_start` (scan.rs:587), the
spec-accepted single-row gap '`secure_paypal.com/login` hides it'; (2) `/` or `\` before the host: `x/` +
`paypal.com/login` -> a path, `host_may_start` barrier; (3) a bare `@`: `@` + `paypal.com/login` -> f5, `at_sign`
returns None (scan.rs:503); (4) an email shape followed by `:` and a non-digit: `a@b.co:` + `paypal.com/login` -> SCP
span to the token's end (`suppressed`, scan.rs:231), so the whole rest of the token claims nothing. Each is reasonable
for one row, where the text really reads glued, but the joined reading is a hypothesis the code tries only to forgive a
mismatch, and an empty result is no evidence of honesty. Natural fix point: in `judge_complete`, let the joined reading
cancel the mismatch only when every mismatching address of the first reading lies inside an address of the joined
reading that matches the destination (span containment on the shared `label_text`). The weaker check 'the joined reading
finds at least one address and no mismatch' is not enough (see the revision note above). Alternatively, add a
`matched`/`addresses.is_empty()` distinction to `Judgement` so callers cannot conflate them. Nothing in link_claims
needs to change for this; the barriers are right for single-row text.

On g03's finding (a) (pieces of one row judged one by one when they can't be put together): `link_claims::addresses` has
no notion of an address fragment, so `Sign in at github` and `.com/login` each yield nothing (`.com/login`: the only
name is the single label `com`, which `qualify` rejects, scan.rs:624). The scanner offers no fix point short of a new
'could be part of an address' predicate (a piece starting with `.` + name + `/`, or ending in a dotted name); the
cleaner fix is in link_disclosure.rs, e.g. also scanning the concatenation of a row's pieces and treating any address
found only there as making the text incomplete.

Suggested bucket: high, matching f1/f2/f5: the warning is bypassed with ordinary ASCII; not `highest` only because the
hover always shows the real destination in full with its host emphasized; if the maintainer treats a missed spoof
warning as security-impacting in itself, `highest`. Bucket should follow g03's finding, which this supplements.

Confidence: confirmed for the scanner rules and for the `judge_complete` condition (source); the end-to-end route
depends on a program emitting the hyperlink over two rows with a hard seam (CR LF or cursor movement), which SPEC.md
describes as supported input.

proposed_drop: none.

possible_cover: g03's finding (b) on the joined second reading (same root cause in link_disclosure.rs); this record adds
the scanner-side triggers and the `Judgement` contract.

### Also reported by `g02-scanner-tests/f11`

Location: `crates/gui/farhelm-grid-model/src/link_claims/tests.rs:525`. Summary: Scanner tests vs g03's split-address
issues: no test makes them worse

Revision: corrected test line references. Not reported (cross-group check requested by the orchestrator). g03 reports
that split addresses are judged per piece when the label has a gap, and that the joined reading counts 'no address
found' as 'no mismatch'. On the scanner side: `Judgement::first_mismatch` is documented as 'None means the label
matches: every address names the destination, or there are none' (link_claims.rs:133-135), and tests assert
`verdict(..) == None` for labels with no address (e.g. tests.rs:936-947,
`a_scheme_followed_only_by_decoration_is_no_address`). That conflation is the scanner's documented contract, but
`Judgement::addresses` is returned alongside it, so a caller can tell 'none found' from 'all match'; the scanner tests
do not stop the caller from doing so. `separating_seams_cut_tokens_and_joining_seams_do_not` (tests.rs:525ff) pins that
a missing `separate` entry and a soft seam join, which is the safe direction (an address is never split by the scanner
on its own). No scanner test pins per-piece judging or the joined-reading rule; those live in link_disclosure.rs (g03).
Refuted as a g02 issue: link_claims.rs:131-135 exposes `addresses`, so the fix is in the caller. Could be highest: no
(as a g02 record; g03 owns the underlying issue).

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g03-disclosure-src/f2`, `g01-scanner-src/f9`, `g02-scanner-tests/f11`.

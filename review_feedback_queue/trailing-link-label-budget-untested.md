# The 8,192 UTF-16 unit link-label budget has no test

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e

## TLDR

No user-visible effect today. SPEC.md promises that link text longer than 8,192 characters is reported as 'couldn't be
checked' from every row; nothing tests that, so a change that dropped or broke it would ship unnoticed, and a long link
text could then be judged on a part of it or show no doubt at all.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g04-disclosure-tests/f5`)

Location: `crates/gui/farhelm-grid-model/src/link_disclosure.rs:308`. Summary: The 8,192 UTF-16 unit label budget has no
test asserting the incomplete hover

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e (still at tip e159ffc79ab147be44e92a92b59571650a4f322d).

TLDR: No user-visible effect today. SPEC.md promises that link text longer than 8,192 characters is reported as
'couldn't be checked' from every row; nothing tests that, so a change that dropped or broke it would ship unnoticed, and
a long link text could then be judged on a part of it or show no doubt at all.

Details: SPEC.md's terminal-links paragraph ("Text over more than 128 rows or 8,192 UTF-16 units is incomplete from
every row, and no text is shown") has two budgets. The row budget is pinned by
an_explicit_label_over_the_row_budget_is_incomplete_from_every_row and
a_generated_chain_over_the_row_budget_is_incomplete_from_every_row (tests/links.rs). The unit budget,
link_disclosure.rs:307-310 (multi-row walk) and 321-331 (a one-row label's hovered stretch, with its own budget per
stretch), has no assertion anywhere: tests/links.rs has no case over 8,192 units, link_disclosure.rs's own tests are
only the_difference_is_the_targets_middle_between_a_common_prefix_and_suffix and pinned_disclosure_policy_oracle, and
the oracle's grid 12 (100 rows of 90 `a`s to https://example.com/x, 9,000 units) recorded the old client as quiet, so
pinned_cell_links_and_label_join_oracle asserts nothing about its warning (only claimed answers are checked).
Consequences of an unnoticed regression: if the branch returned a judged or `None` label instead of OverBudget, a long
label could hide a mismatching address beyond what is read or be judged differently from different rows. Fix: add a test
like the row-budget one with rows wide enough to cross 8,192 units in under 128 rows (e.g. grid 12's shape), asserting
over_budget() from first, middle and last row and quiet just under the edge; plus a one-row case with two stretches
where only the hovered stretch is over budget (each stretch gets the budget, per SPEC_impl.md 'a one-row label's
stretches each get the unit budget').

Suggested bucket: other — missing test coverage for a safety-relevant rule; no shipped bug shown.

Confidence: confirmed (grep for 8192/MAX_UNITS/units in tests/links.rs finds nothing; oracle grid 12 inspected via its
JSON).

proposed_drop: none

possible_cover: none

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g04-disclosure-tests/f5`.

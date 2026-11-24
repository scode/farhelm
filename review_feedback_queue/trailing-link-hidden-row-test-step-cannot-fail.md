# A hidden-row link test step cannot fail

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e

## TLDR

No user-visible effect today. A test that claims to pin the spec's statement that a resting hover only re-judges hidden
text on the hovered row does not exercise that statement, so a later change to how other rows of a link are watched
would go unnoticed by it.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g04-disclosure-tests/f2`)

Location: `crates/gui/farhelm-grid-model/tests/links.rs:1826`. Summary: Test's 'another row is not watched' step cannot
fail: the other row is not part of the hovered hyperlink's label

Revision (turn 2): location corrected to the tip line of the assertion message (1826); substance unchanged.

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e (still at tip e159ffc79ab147be44e92a92b59571650a4f322d).

TLDR: No user-visible effect today. A test that claims to pin the spec's statement that a resting hover only re-judges
hidden text on the hovered row does not exercise that statement, so a later change to how other rows of a link are
watched would go unnoticed by it.

Details: a_run_turning_hidden_on_the_hovered_row_changes_the_link (crates/gui/farhelm-grid-model/tests/links.rs, final
step 'another row is not watched') recolors row 1's `y` to the background and asserts the link at row 0 still compares
equal. But row 1's `y` belongs to a different explicit hyperlink id (`other`, link index 1), so the label walk for `tag`
stops at row 1 (link_disclosure.rs:303-305, label_row keys on id and uri) and row 1 is not one of the label's rows. A
fresh judgment of row 0 would therefore also be unchanged, and TerminalLink equality never depended on row 1, so the
assertion holds whether or not non-hovered label rows are watched. The docstring's 'A same-target hyperlink on another
row turning hidden leaves it equal too: only the hovered row is watched (SPEC.md)' describes a distinction the scenario
cannot make. To actually pin SPEC.md's 'need not follow later redraws of other rows while the pointer rests, though text
on the hovered row turning hidden or visible is judged again', row 1 should continue the same `tag` hyperlink (visible
text plus a run that turns unseen), so a fresh link_disclosure would add Hidden while the hovered link stays equal; or
drop the claim from the docstring.

Suggested bucket: other — test-quality gap in maintainer tooling, no user-facing behavior wrong.

Confidence: confirmed (label_row/read_label at link_disclosure.rs:279-423 stop the walk at a row with no stretch of the
same id and uri).

proposed_drop: none

possible_cover: none

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g04-disclosure-tests/f2`.

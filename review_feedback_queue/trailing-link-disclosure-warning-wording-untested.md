# No test pins the link warning's quoted-text heading or its "and N more" line

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e

## TLDR

The hover warning's wording under the reasons ("Link text (partial):" for text that couldn't be checked whole, nothing
at all for text too long to read, and "and N more" when several addresses differ) is required by the spec but no test
checks it, so a later change could show a partial label as if it were the whole text, or drop the count of other
mismatching addresses, without any test failing.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g05-gui-disclosure/f5`)

Location: `crates/gui/farhelm-gui/src/terminal/disclosure.rs:249`. Summary: No test pins the warning's quoted-text
heading choice or its "and N more" line

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e (heading choice) and e159ffc79ab147be44e92a92b59571650a4f322d
("and N more"); the tip e159ffc79ab147be44e92a92b59571650a4f322d still has both untested.

TLDR: The hover warning's wording under the reasons ("Link text (partial):" for text that couldn't be checked whole,
nothing at all for text too long to read, and "and N more" when several addresses differ) is required by the spec but no
test checks it, so a later change could show a partial label as if it were the whole text, or drop the count of other
mismatching addresses, without any test failing.

Details: DisclosureView::new (crates/gui/farhelm-gui/src/terminal/disclosure.rs, the `let text = match &warning.address`
block, about lines 249-258 at the tip) picks among four outcomes: "Text mentions:" with the address, no text when the
label is empty (over budget), "Link text (partial):" when the reasons include Incomplete, and "Link text:" otherwise;
panel() (about lines 361-367) adds a `terminal-link-disclosure-more` line "and {more} more". SPEC.md's terminal links
paragraph fixes this wording ("Under the reasons it shows the first address that differs after 'Text mentions:', with
'and N more' when other addresses in the text differ too, or else the visible text after 'Link text:', marked partial
when it couldn't be checked whole"; "Text over more than 128 rows or 8,192 UTF-16 units is incomplete from every row,
and no text is shown"). In crates/gui/farhelm-gui/src/terminal/attached/tests.rs only "Text mentions:" is asserted
(a_two_row_lookalike_hyperlink_warns_on_both_rows_and_still_opens); a grep for 'Link text', 'partial', 'and 1 more' and
'terminal-link-disclosure-more' across crates/gui/farhelm-gui/src finds no assertion
(a_long_lookalike_label_wraps_inside_the_window's doc names "Link text:" but asserts only geometry).
crates/gui/farhelm-gui/src/terminal/disclosure/tests.rs has one test, about visibility. The goldens scene
terminal-link-disclosures draws "and 1 more" but is a pixel hash, which does not say what a diff means. The ordering of
the match arms matters (the empty-label arm must precede the Incomplete arm, or an over-budget label would get a "Link
text (partial):" heading over nothing). Fix: a small unit test in disclosure/tests.rs that builds LinkDisclosure values
(as golden_displays does) for each of the four branches and more > 0, and checks the painted text, or the Warning
struct, for the heading and the "and N more" line.

Suggested bucket: other — a test-coverage gap with no current misbehavior.

Confidence: confirmed (source and grep establish the absence; the current code's branches read correctly).

proposed_drop: none

possible_cover: grid-model tests in crates/gui/farhelm-grid-model/src/link_disclosure.rs may pin the LinkWarning fields
(label emptied over budget, more counts) that feed these branches; that covers the model's half, not the GUI's choice of
heading.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g05-gui-disclosure/f5`.

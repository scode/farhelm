# The link oracle's grid exceptions switch the safety check off for the excepted case

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e

## TLDR

No user-visible effect today. The regression test meant to stop terminal-link spoofs from silently losing their warning
exempts one recorded spoof (a link whose text reads `https://example.com` on one row and `.evil.test/x` on the next, but
goes to example.com/x) in a way that would also let that spoof go completely quiet without any test failing.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. g03 confirmed the tip does warn for that grid, so this is a test-strength gap only.

### Reviewer record (`g04-disclosure-tests/f1`)

Location: `crates/gui/farhelm-grid-model/tests/links.rs:1076`. Summary: Oracle grid exceptions switch the safety check
off entirely; the excepted spoof (example.com above .evil.test/x) could go quiet unnoticed

Revision (turn 2): location corrected to the tip line of the exception branch (1076, was given as a patch line);
substance unchanged. Synthesis also confirms f6 is a sibling: the only grid case reaching the joined second reading is
this excepted one.

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e (still at tip e159ffc79ab147be44e92a92b59571650a4f322d).

TLDR: No user-visible effect today. The regression test meant to stop terminal-link spoofs from silently losing their
warning exempts one recorded spoof (a link whose text reads `https://example.com` on one row and `.evil.test/x` on the
next, but goes to example.com/x) in a way that would also let that spoof go completely quiet without any test failing.

Details: In pinned_cell_links_and_label_join_oracle (crates/gui/farhelm-grid-model/tests/links.rs, the
`if exceptions.contains(&key)` branch around line 1068), an excepted key only asserts `!holds`. `holds` is false both
when the hover warns with a different label (what the exception describes) and when the hover shows no warning at all.
The two `grid` entries in tests/fixtures/links-oracle-exceptions.json (grid 3, lines 0 and 1) state in their rule that
the case 'still warns, from both rows, and still mismatches with the break joined', but nothing enforces it. Grid 3 in
links-oracle.json is rows `https://example.com` / `.evil.test/x` (40 cols, hard break) under one explicit hyperlink to
https://example.com/x, i.e. exactly the 'host continued after a break' spoof the label reader's seam rules decide. I
found no other test pinning this shape: grep for `.evil.test` in tests/links.rs and src/link_disclosure.rs finds only
credential/mismatch one-row cases, and labels_over_several_rows_join_and_separate_at_their_seams has no case where a
separating break follows a host the target shares. A future change to the seam rule or the joined second reading (e.g. a
bare site name matching the target host in the first reading plus a regression that drops the `evil.test/x` claim or the
joined mismatch) would make this spoof quiet with every test green. Fix: for an excepted grid key, still require the
safety direction in the new shape, e.g. assert the hover warns with Mismatch from that row, and ideally give each grid
exception an expected `label` (here `https://example.com .evil.test/x`) that the test compares; or add the case to
labels_over_several_rows_join_and_separate_at_their_seams as a loud case.

Suggested bucket: other — test-quality gap; the shipped behavior is believed correct today.

Confidence: confirmed (the branch logic is in the test; the absence of another pin is from grep of tests/links.rs and
src/link_disclosure.rs).

proposed_drop: none

possible_cover: none

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g04-disclosure-tests/f1`.

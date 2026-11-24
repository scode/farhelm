# link_disclosure.rs module docs describe a soft seam too narrowly

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e

## TLDR

No user-visible effect. The file's top comment describes the rule for joining rows wrongly, which could lead a future
change to break the deliberate joining of rows a program filled to the edge itself.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g03-disclosure-src/f3`)

Location: `crates/gui/farhelm-grid-model/src/link_disclosure.rs:23`. Summary: Module docs say a soft seam is a terminal
wrap; the code also treats a program's edge-filling CR LF as soft

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has
it.

TLDR: No user-visible effect. The file's top comment describes the rule for joining rows wrongly, which could lead a
future change to break the deliberate joining of rows a program filled to the edge itself.

Details: crates/gui/farhelm-grid-model/src/link_disclosure.rs:23-25 says "A soft seam is where the terminal wrapped the
text ... A hard seam is everything else: a program that ends a row and continues its hyperlink on the next." The code
and the same file's `RowSeam::Soft` doc (lines 210-214) say the opposite for one case: a seam is soft whenever
`GridModel::edge_seam` finds the pieces meeting edge to edge, "the wrap flag isn't consulted", so a program that fills a
row and writes CR LF gets a soft seam. SPEC.md 3612-3614 ("whether the terminal wrapped it or the program moved to the
next row itself") and SPEC_impl.md 3515-3518 agree with the code. `link_claims::Seam::Soft` (link_claims.rs:61-62, "The
terminal wrapped the text") has the same wording, though it is the caller's to decide. Fix: reword the module docs (and
`Seam::Soft`) to say a soft seam is where the hyperlink's text meets edge to edge across the rows, as a wrap draws it,
whoever moved to the next row.

Suggested bucket: other. Documentation only, with no security or data consequence.

Confidence: confirmed.

proposed_drop: none.

possible_cover: none known.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g03-disclosure-src/f3`.

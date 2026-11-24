# A gap anywhere in a multi-row link silences an address split across its rows

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e

## TLDR

A program can underline text that visibly spells a familiar address across a line wrap (for example `Sign in at github`
at the end of one row and `.com/login` at the start of the next), point the link somewhere else, and stop the hover from
warning just by leaving one unlinked cell somewhere in the link's text (for example an unlinked space before a final
`now`). Without that gap the same link warns that the text mentions a different address. The full destination is still
shown, but the mismatch warning the hover exists to give is gone.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer (2026-11-24); the reviewer suggested `highest`.

### Reviewer record (`g03-disclosure-src/f1`)

Location: `crates/gui/farhelm-grid-model/src/link_disclosure.rs:457`. Summary: A gap anywhere in a multi-row link
silences an address split across its rows

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e (introduced the verdict's multi-stretch branch); the tip
e159ffc79ab147be44e92a92b59571650a4f322d still has it unchanged.

TLDR: A program can underline text that visibly spells a familiar address across a line wrap (for example
`Sign in at github` at the end of one row and `.com/login` at the start of the next), point the link somewhere else, and
stop the hover from warning just by leaving one unlinked cell somewhere in the link's text (for example an unlinked
space before a final `now`). Without that gap the same link warns that the text mentions a different address. The full
destination is still shown, but the mismatch warning the hover exists to give is gone.

Details: In `verdict` (crates/gui/farhelm-grid-model/src/link_disclosure.rs:457-481), when the label has more than one
row and any row has more than one stretch, the code only scans each stretch on its own with
`link_claims::addresses(&[RowText::first(&stretch.text)], &[])` and raises `Incomplete` if some stretch mentions an
address (or unseen text exists). It never runs `link_claims::judge` on any joined reading, and the joined text it builds
via `read_seams` is used only for display. So an address that exists only across a seam (soft or hard-joined) is
invisible to the check as soon as any row anywhere in the label is split into stretches. Concretely, with destination
`https://evil.test/`: row 0 `Sign in at github` reaching the row's last text column, row 1 `.com/login` at column 0, an
unlinked space, then `now` in the same hyperlink. Per-stretch scans: `Sign in at github` has no dot, so no anchor;
`.com/login` gives only the single label `com` to the NAME regex (scan.rs:40-41), and single labels other than
`localhost` never anchor (scan.rs:38-39); `now` has nothing. So `mentions` is false, `hidden` is false, `reasons` stays
empty, and `verdict` returns `None` (ordinary display). Remove the `now` stretch and the label takes the `else` branch,
the soft seam joins `Sign in at github.com/login`, the host anchor finds `github.com/login`, and the hover warns
Mismatch. The gap need not be near the address: any row with two stretches (row 5 `a`, unlinked cell, `b`) switches the
whole label to per-stretch scanning. The gap must be ordinary unlinked text (a plain space looks the same as a linked
one), not unseen text with the same destination, which would add Hidden.

Spec: SPEC.md 3612-3613 promises "Rows join where the hyperlink's text fills one row to its last column and continues at
the start of the next", and 3618-3619 only says that the pieces of a split row "can't be put back together, and the text
is incomplete when any piece mentions an address"; it does not say a split elsewhere stops rows that do meet edge to
edge from joining. SPEC_impl.md 3521-3524 reads "incomplete when a multi-row label has a row with several stretches and
any stretch mentions an address ... otherwise the per-seam reading judged by `link_claims::judge`", which literally says
the per-seam reading is judged when no stretch mentions an address; the code judges nothing in that case. The accepted
limit at SPEC.md 3638-3640 ("a program that chooses its own line breaks or uses separate hyperlinks can still split an
address into pieces none of which looks like one") covers a program's own breaks or separate hyperlinks; here the split
address is in one hyperlink across a terminal wrap, which the spec says joins.

How to verify: build a grid as tests/links.rs:738-777 does, with row 0 `Sign in at github` filling the row (soft seam
per `GridModel::edge_seam`), row 1 `.com/login` + an unlinked space + `now`, one tag to `https://evil.test/`;
`link_disclosure(..).warning` is `None`. Drop `now` and it is `Some([Mismatch])`.

Fix: in the multi-stretch branch, also scan the joined readings for addresses. The narrowest change is to treat the
label as incomplete when either seam reading of the display text (`read_seams(&pieces)` with its `separate` and `joined`
vectors, stretches spaced apart) mentions an address, not only when a single stretch does. Add a test with a
soft-seam-split address plus an unrelated split row.

Suggested bucket: highest. It defeats the spoofing warning for a program-controlled layout through an established code
route; the always-shown full destination is what remains, which is the argument for `high` if the maintainer treats the
warning as secondary.

Confidence: confirmed (source establishes it; not executed).

proposed_drop: none. The nearest spec text is SPEC.md 3638-3640, "The warning is an aid, not a guarantee ... a program
that chooses its own line breaks or uses separate hyperlinks can still split an address into pieces none of which looks
like one"; I do not think it covers one hyperlink wrapped by the terminal, which 3612-3613 says joins.

possible_cover: none known.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g03-disclosure-src/f1`.

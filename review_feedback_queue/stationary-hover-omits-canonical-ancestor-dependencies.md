# A stationary link hover can miss a required mismatch warning

Reviewed commit: 8b4f1e16496292ce18d556f0bc40a80ef42b4b8f

## TLDR

A terminal program can change which surrounding text a hyperlink warning must compare without refreshing an already
visible hover. The hover can keep its ordinary target display where a fresh hover would show a mismatch warning. The
actual destination remains correctly disclosed; this source route does not establish credential loss, data loss or
opening a different destination.

## Details

Suggested bucket from the original reviewer: **high**. Recording bucket: **highest**, because the fresh independent
consultation could not clearly agree that omitting this anti-spoofing warning is outside security impact. The queue's
conservative exclusion rule requires retaining that priority on ambiguity. Confidence: **confirmed source route**;
credential loss, a substituted destination and other stronger harm remain unestablished. `could_be_highest: true`.
Proposed drop: none.

The boundary row recorded as hover evidence can derive its canonical hyperlink identity from an ancestor outside that
evidence. At the reviewed commit, `crates/farhelm-ui/assets/grid-buffer.js:303–335` resolves canonical identities, while
`:415–424` records the tagged extent and its immediate boundary. It does not retain every ancestor consulted to resolve
that boundary. The late-answer evidence at `:458–475` has the same omission for the recorded scenario.

The source example uses three rows with the same target, `https://bad.example/x`. Row 0 fills the width with URL-like
text naming another destination, has soft wrap enabled and id `3_alacritty`. Row 1 begins with suffix text, is
unwrapped, and has id `2_alacritty`. Row 2 contains the full correct target and id `1_alacritty`. Initially, row 1
resolves to id 3, so hovering row 2 records only rows 1, 2 and the following boundary row. That comparison is quiet.
Repainting only row 0 with the same text and target but explicit id `1_alacritty`, retaining its wrap, makes row 1
inherit id 1. A fresh `terminal-links.js:467–530` comparison now joins rows 0–2 and warns; the active hover's recorded
rows have not changed.

This input is reachable through the recorded terminal path. `vendor/alacritty_terminal/src/term/cell.rs:279–290`
preserves explicit ids, including generated-looking spellings. `grid_output.rs:393–429` compares and sends changed rows,
and its `link_key:707–713` preserves id and URI. `grid-model.js:513–524,560–563` replaces only sent rows. Identically
redrawing row 1 need not send a replacement because the supervisor compares row contents.

The caller does not supply a broader invalidation. `grid-terminal.js:1234–1247,1334–1347` tracks returned evidence;
`:1250–1253` clears the row cache without clearing the active hover; `:1272–1283,1380–1390` retains a covering hover
whose evidence is unchanged. Link release at `:1614–1619` uses that same currentness check. At the inclusive tip,
`receive:1891–1896` and `drain:1809–1821` still call the cache-only invalidation, while `paint:849–882` calls the
unchanged hover refresh.

`SPEC.md`, “Terminal experience,” `:2314–2332`, requires mismatch comparison over joined hyperlink text. `SPEC_impl.md`,
“Terminal widget: xterm.js island,” `:1705–1708`, requires adapter identity and captured hover/late-answer evidence to
agree. Accepting generated-looking explicit ids does not accept stale dependency evidence. No repair appears through
`c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`: grid-buffer and terminal-links are unchanged, and grid-terminal changes only
a guidance comment.

Retain the transitive rows consulted during canonical identity discovery, including boundary ancestors and wide-wrap
lookahead, or invalidate on an equivalent dependency closure. A focused regression should change only the ancestor row
while the pointer stays still and check both hover refresh and a late answer.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established.

`g022/r1` concerns the repaired basic anonymous direct-continuation false warning, a different trigger from this
canonical-boundary dependency defect. `g031-l3` concerns the fresh comparison contract for an accepted cursor-tail
exception and is also distinct.

The existing highest item `plain-url-hover-omits-neighbor-dependencies.md` concerns a printed URL extended across a
wrapped row while a click keeps the old target. This record concerns an OSC hyperlink's canonical-identity ancestors and
a newly required mismatch warning; its actual URI stays the same. Their producers, consequences and dependency closures
differ, so one record does not fully cover the other.

Caveats: no runtime or network reproduction is established. The minimal three-row scenario deliberately uses explicit
generated-looking ids. The independent consultation retained the priority ambiguity; it did not establish a harmful
opening or certify the source trace by experiment.

Review provenance: automated committed-code trailing review, run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, reviewer
`g025`, original ID `g025-l2`; findings log lines 3, 4 and latest line 12. The original owner and potentially-highest
flag are preserved.

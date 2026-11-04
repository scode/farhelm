# Selector policy drops ordinary formatting input from copied text

Reviewed commit: bed9c094f7b6848ad43ab95b0d16b298ab2b85f4

## TLDR

New terminals discard ordinary formatting and filler codepoints even when the output contains no emoji or variation
selector. Copying that terminal text loses those codepoints. The delivery report presents selector support without
identifying this broader input policy or its independent copying consequence.

## Details

The implementation references in this record are at `af585f5ba3d90f5826febc24f7954913b0a32205`, with the affected paths
unchanged through `c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`.
`vendor/alacritty_terminal/src/term/mod.rs:1443–1450,1510–1514` returns before scalar/suffix storage for the
ignored-format class, including U+200B, bidi marks, BOM and U+3164/U+FFA0 fillers. The preceding revision's input path
at `1235–1259` retained ordinary zero-width marks. For example, `A + U+200B + B` becomes `AB`, without a selector
involved.

The lost input cannot survive a later copy: `crates/farhelm-supervisor/src/grid_output.rs:144–208` transmits stored cell
characters and suffixes; `crates/farhelm-ui/assets/grid-buffer.js:69–91,126–148` builds selection text from those runs;
`grid-terminal.js:1015–1019,1492–1497,1602–1610` passes it to Copy/copy-on-select.

The delivery report at `plans/reports/vs16-emoji-width.report.md:31–35` describes this as a bounded selector-support
subset. The approved plan at `7e14f9dcd95920992302e6d488c66f06ce030079:plans/queue/vs16-emoji-width.md:20–23,505–515`
concerns emoji/text selectors and two named edge exceptions. Its `423–426` requires agreement for a material scope
expansion or weakened guarantee and says a review finding or log entry is not authorization. Neither the terminal
fidelity/copying contract nor the implementation specification's two selector exceptions explicitly accepts blanket
ordinary-codepoint deletion.

The later landing report at `8e5ccbf028bbdab142d7a98636546058691eb175:plans/reports/vs16-emoji-width.report.md:163–165`
accurately warns that copied text loses formatting/filler input. That disclosure does not itself settle the product
policy or reconcile the product specification and user-facing caveats. Preserve ordinary codepoints while handling
selector predecessors, or explicitly settle that broader policy and document its consequences.

## Confidence and priority

Confidence: confirmed source path for codepoint/copy loss; the product-contract and scope decision remains unresolved.
No runtime experiment or independent Ghostty parity assertion is claimed.

This record makes **no filler-width, occupied-column or column-shift claim**. `Cargo.lock:6705–6708` pins
`unicode-width 0.2.3`; its locally read `tests/tests.rs:80–85` gives both U+3164 and U+FFA0 width `Some(0)`. The earlier
comment-derived layout claim is withdrawn. Those tests were read, not executed.

Original reviewer suggested bucket: `other`, for uncommon-character text/copy fidelity. Recording bucket: **highest**. A
fresh independent priority consultation could not clearly place deletion from retained terminal text outside loss of
user data, so the queue's ambiguity rule keeps highest. No source-file corruption, security route or concrete
irreversible useful-work sequence is established. The original `could_be_highest: false` assessment is preserved as
reviewer provenance, not the final priority judgment. The refuted filler-width claim contributes no severity evidence.
`proposed_drop: null`.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established.

Grouped findings: `g034-l5`, `g043-l6`, `g044-l4`, `g045-l5` and `g046-l6` identify this same ignored-format deletion.
`g043-f7` is a separate printable V/T dispatcher outside this ignored-format record. `g035-l3` is a separate
report-attribution issue.

## Provenance

Review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, owner `g034`, ID `g034-l5`, latest `g034/findings.jsonl` physical
line13; grouped findings `g043-l6`, `g044-l4`, `g045-l5` and `g046-l6`. The latest record retains the copy-loss finding
while withdrawing the unsupported filler-layout consequence and adding direct pinned-dependency evidence.

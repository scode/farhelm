# The emoji landing report names the wrong vendored patch

Reviewed commit: 8e5ccbf028bbdab142d7a98636546058691eb175

## TLDR

The landing report directs a maintainer to the parser patch for two cosmetic edits that actually belong to the terminal
emulator patch. Following the recorded cleanup points at the wrong crate.

## Details

Suggested bucket: **other**. Confidence: **confirmed by source**. Proposed drop: none. Possible cover: none found. This
is a report-location error, distinct from the live Unicode text-retention findings.

`plans/reports/vs16-emoji-width.report.md:166–167` attributes a moved import and reflowed comment to the vendored
parser's patch. Neither appears in `vendor/patches/vte-0.15.0.diff`. They occur in
`vendor/patches/alacritty_terminal-0.26.0.diff:491–500,600–609`, under the terminal emulator's `term/mod.rs` diff.
SPEC_impl.md, "The supervisor's terminal emulator", and the report's own 137–138 distinguish these crates. The report
and patches remain unchanged through c5ae8548439cc5bfa33c11ef7ce8da6129d8072a.

Change the attribution to the terminal emulator. The cosmetic edits have no demonstrated runtime consequence; this
finding does not request changes to either upstream tree. Targeted specification, Planned, unlanded-plan, known-bug,
queue and ledger checks found no matching cover or filter. Provenance: review run
`ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, `g035/g035-l3`. No runtime check ran.

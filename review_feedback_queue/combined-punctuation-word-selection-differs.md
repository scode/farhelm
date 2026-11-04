# Word selection classifies combined punctuation by its base instead of the whole cell

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Double-clicking unlinked text beside punctuation carrying a combining mark can select a shorter word in a grid terminal
than in an old terminal.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source behavior
difference and supported producer, no runtime reproduction or payload construction.

Source locations: 7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-buffer.js:262-264.

grid-buffer.js:248-254 groups each combined cell, but separator at :262 tests separators.includes(text[i]) on only its
first UTF-16 code unit; :263-264 stops at that base punctuation. Vendored vendor/xterm.js line1 _isCharWordSeparator
tests width!=0 and wordSeparator.indexOf(cell.getChars())>=0, so punctuation plus combining mark does not equal a member
of the ASCII separator list; its _getWordAt calls that predicate for neighboring cells and traverses through them. The
supplied default WORD_SEPARATORS at grid-buffer.js:49-50 is exactly the upstream list, so the differing granularity, not
configuration, causes the discrepancy. Ordinary producer verified statically at tip:
vendor/alacritty_terminal/src/term/mod.rs:1235-1259 attaches a zero-width character to the preceding cell without a
base-punctuation exclusion, and crates/farhelm-supervisor/src/grid_output.rs:166-168,192-208 emits base plus those marks
as one non-ASCII run. mappedText:196-204 preserves that whole cell text and its mapping, so the input needed by the
differing predicate survives the producer. No vendor behavior or native timing premise remains for this source
difference.

Possible coverage and caveats: none exact. wordRange's :229-235 contract explicitly says it follows xterm _getWordAt.
SPEC_impl.md:1297-1304 permits documented file-header departures, but neither buffer header nor component header
declares a combined-separator exception; the generic word-selection description does not settle this narrower
classification. Required targeted searches across SPEC.md,SPEC_impl.md,TODO Planned,unlanded plans/queue,BUGS.md,queue
INDEX and TRIAGE_OUTCOMES.md found no matching acceptance, scheduled fix or existing issue. The normal word-only/header
ambiguity in f5 concerns the separate hovered-link preference, not this unlinked word boundary. The rare
transient/diagnostic filter does not clearly cover deterministic repeated selection extent for supported combined text.
Suggested fix if xterm behavior is intended: have the separator predicate compare the entire mapped cell string, rather
than only text[i], and pin that distinction in a word-range test.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g171/f8`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f;
persists at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb, where helper and vendored xterm are unchanged.. This
was a source-only review; runtime occurrence and frequency were not measured.

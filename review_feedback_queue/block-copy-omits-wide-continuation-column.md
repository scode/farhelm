# Column copy drops a selected wide-character continuation column

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A rectangular terminal copy omits the leading blank corresponding to a wide-character continuation cell on a row whose
left selection boundary cuts through that character, shifting that row's copied content left relative to other selected
rows.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. source behavior
difference confirmed; no runtime reproduction required.

Source locations: 7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-buffer.js:363-375.

columnText:363-375 applies identical column bounds but delegates to rowText, whose :82-84 omits a wide run beginning
before from without emitting a blank for the selected continuation. T grid-terminal.js:1105-1110 starts off-Mac Alt-drag
column mode; :1065-1074 retains the corners' shared columns; :1014-1018 calls columnText, so an intermediate row with a
wide character crossing that shared boundary reaches this branch. T vendor/xterm.js line1 named
SelectionService.selectionText excerpt shows column mode passes raw shared bounds to translateBufferLineToString for
every row; BufferLine.translateToString emits WHITESPACE_CELL_CHAR at an empty continuation cell. _handleSingleClick's
adjustment concerns only its anchor row, not the intermediate row, so does not remove the route. Internal independent
evidence: grid-buffer.js:532-533 and :549-555 represent/read a continuation as an empty width-zero cell and emit a space
when translation starts there. The assigned test grid-buffer.test.js:266-270 expressly pins that adapter result, while
the columnText test at :376-386 uses only ASCII and misses the interaction.

Possible coverage and caveats: explicit ambiguity, not assumed coverage. The module header at grid-buffer.js:27-30 says
a wide character counts once for the cell it starts in; rowText:61-64 expressly omits a character whose first cell is
outside the range. SPEC_impl.md:1297-1304 allows file-header departures from xterm behavior. Those statements could be
intended to accept this rule for column copy as well, but columnText:359-361 separately promises the same columns as
xterm, and the adapter/test explicitly preserve the continuation blank. No spec states which rule wins for a
partial-wide column selection, so retain the mismatch with its possible acceptance basis. Required coverage search
checked SPEC.md, SPEC_impl.md, empty TODO Planned, plans/queue, BUGS.md, queue INDEX/matching file and
TRIAGE_OUTCOMES.md. T TODO.md:240-246 is emoji width determination, not copying an already-wide cell;
review_feedback_queue/classic-mouse-snapshot-copy-blocks-later-links.md concerns selection dismissal under classic mouse
reporting, not copy extent. No exact planned fix/BUGS/queue/ledger match found. FILTER.md's rare transient trigger does
not cover deterministic block-copy behavior. Suggested fix if parity is intended: preserve the selected continuation as
a blank in column extraction without changing the declared linear rowText contract.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g171/f6`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f;
unchanged helper/tests at tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime
occurrence and frequency were not measured.

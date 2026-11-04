# An active selection drag survives resets that invalidate its line anchors

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

If history resets, the terminal resizes, or the alternate screen switches while a local text drag is held, the next drag
movement can recreate a selection using the invalidated old line positions.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by static
source; no runtime reproduction or frequency measurement. No security, loss of user-owned work, or availability
consequence is asserted.

Source locations:
7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-terminal.js:897-903,1026-1030,1578-1580.

R noteDamage897-903 handles size, historyReset and alternate-screen switching by clearSelection(), but
clearSelection1026-1030 only sets selection=null; it does not stop gesture or clear gesture.anchor/anchorRange. The
existing document gesture persists; subsequent onDocumentMouseMove1569-1581 calls extendTo using that old anchor, whose
char/word/line branches restore selection. The drag-scroll interval(R1159-1167 approximately; exact T1166-1173) is even
more direct: it calls paint(), which can invalidate selection in noteDamage, then immediately calls extendTo with the
retained gesture. Exact T ranges: noteDamage897-908; clearSelection1026-1030; extendTo1066-1087; drag timer1166-1173;
pointer move1575-1587; stopGesture1670-1675.

If history resets, the terminal resizes, or the alternate screen switches while a local text drag is held, the next drag
movement can recreate a selection using the invalidated old line positions. Releasing can copy different text from the
original anchored selection instead of leaving the invalidated selection cleared. Details/source basis: R
grid-terminal.js:85-88 explicitly promises that these transitions clear a selection;891-903 says line-number anchors are
void and calls clearSelection. However1026-1030 clears only selection, leaving the active gesture and gesture.anchor or
anchorRange untouched. The next onDocumentMouseMove at1569-1580 calls extendTo;1060-1081 rebuilds selection from that
old anchor. The drag-scroll interval at1160-1167 can do this in the same callback: paint() clears the selection for
damage, then extendTo recreates it from the surviving gesture. The matching release1584-1604 subsequently reads/copies
selectionText. At T the corresponding verified source is
grid-terminal.js:897-908,1026-1030,1066-1087,1166-1173,1575-1609,1663-1675. stopGesture would clear the gesture and its
scroll timer, but noteDamage does not call it or otherwise retire local-selection ownership. R-to-T diff changes
selection midpoint rounding and input report helper calls, not this lifecycle. Ordinary producer: T
crates/farhelm-supervisor/src/grid_output.rs:494-514,554-559,635-668 records normal size/history-erasure/palette changes
as layout changes and returns HistoryReset::Relayout, with a new numbering contract. service/grid_stream.rs:590-598
sends HistoryReset before replacement history. T grid-model.js:597-603,657-667 applies that reset as
damage.historyReset. Therefore this does not require a hostile peer, an old release or an exceptional failure. A held
drag can span the ordinary reset; a continuing move/auto-scroll tick is already the shipped input path.

Possible coverage and caveats: none exact found. T SPEC.md:2285-2303 describes completed local selection copying;
SPEC_impl.md:1297-1305 says the grid reproduces terminal behavior with its file-header departures. The R file
header85-88 establishes the clear-on-reset contract explicitly. T SPEC.md:2128-2138 and SPEC_impl.md:2442-2465 describe
ordinary history-reset producers; they do not accept carrying selection anchors through reset. TODO.md Planned23-25 is
empty. TODO.md:215-221 concerns history memory representation, not selection invalidation. Required searches covered
SPEC.md, SPEC_impl.md, TODO.md, unlanded plans/queue, BUGS.md, queue INDEX/matching files and TRIAGE_OUTCOMES.md.
Matching queue history-fixture-reset-races-selection-save.md(full file) is session-history test preference writes, not
terminal text selection; secondary-press-suppresses-primary-selection-copy.md is a different gesture ownership cause.
TRIAGE_OUTCOMES.md:7333-7353 concerns retained clipboard recopy after a new plain click under mouse reporting, not a
held local drag through anchor invalidation. FILTER.md rare-glitch/subsecond filters are not an established exact match:
the active drag can persist for arbitrary time across a normal reset, rather than requiring a person to win a subsecond
answer window; if triage establishes the narrower rarity/recoverability match, that remains a policy decision, not a
source refutation.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g160/f3`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f (R).
Confirmed persistent at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (T).. This was a source-only review;
runtime occurrence and frequency were not measured.

# Double-click selects only one word of a hovered multiword hyperlink

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A stable hovered hyperlink with multiword text copies only the clicked word on double-click in a grid terminal, whereas
xterm selects the entire hovered link range.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source behavior
difference, with product-contract interpretation caveat below; no runtime reproduction.

Source locations: 7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-terminal.js:1090-1098.

grid-buffer.js:237-266 implements the word-only extent; grid-terminal.js:1090-1098 calls it unconditionally for detail 2
although the press handler already retains the hovered link at :1681-1707. At tip these consumer ranges are :1096-1104
and :1687-1713. Fresh static excerpts of the unchanged vendor/xterm.js SelectionService._handleDoubleClick and
_selectWordAtCursor show the former delegates to the latter, which prefers currentLink.link.range over the normal word
search. Tip grid-terminal.js:1590-1609 copies the selected text through onCopy; copy-on-select.js:79-82 permits the
primary local-selection gesture, including a forced selection under mouse tracking. The known display/paint race is not
needed for this stable-hover case.

Possible coverage and caveats: SPEC_impl.md:1297-1304 says the grid reproduces xterm selection/link behavior and its
file headers declare departures; grid-terminal.js:75-90 says a double click selects a word. That generic description
could be intended as a deliberate word-only rule, but does not explicitly say it omits xterm's hovered-link exception.
Record rather than silently treating this ambiguity as coverage. The buffer header has no such declared exception
either. TRIAGE_OUTCOMES.md:8667-8684 concerns accidental activation during selection, not this extent. Targeted searches
across all required coverage sources found no exact Planned item, plan, BUGS entry, queue file or ledger decision about
this hovered-link selection difference; TODO Planned is empty. FILTER.md's rare transient trigger does not fit an
ordinary deterministic double-click. Fix direction if parity is intended: have the selection caller prefer the captured
hovered link range for a double-click, retaining wordRange for unlinked text.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g171/f5`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f;
persists at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime occurrence and frequency
were not measured.

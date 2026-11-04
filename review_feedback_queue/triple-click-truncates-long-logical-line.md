# Triple-click silently truncates long wrapped lines

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Triple-clicking a retained terminal logical line that extends more than 1000 wrapped rows from the click copies only
part of that line.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source, no
runtime reproduction.

Source locations: 7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-buffer.js:270-275.

crates/farhelm-ui/assets/grid-buffer.js:155-173 defaults wrappedSpan to 1000 rows either way; lineRange at 270-275 calls
it without overriding that cap and advertises a whole soft-wrapped line. At that same immutable commit,
grid-terminal.js:1090-1098 uses lineRange for a triple click, and 1014-1018 copies that partial range. A logical line
extending more than 1000 retained wrapped rows from the click is therefore truncated; repeating the gesture produces the
same result. The vendored xterm.js SelectionService._selectLineAt calls getWrappedRangeForLine, whose backward/forward
loops run to the actual logical-line boundaries (bounded static excerpts read from assets/vendor/xterm.js at the
reviewed commit), without this 1000-row truncation. Proposed fix: let this boundary-only traversal run to the retained
source boundaries; unlike wordRange it need not allocate mapped character records.

grid-buffer.js:155-173 defaults wrappedSpan to 1000 rows in each direction, and lineRange:270-275 calls that default. At
the reviewed commit grid-terminal.js:1090-1098 selects that range and :1014-1018 copies it; at pinned tip the selection
caller is :1096-1104. Vendored xterm.js SelectionService._selectLineAt and Buffer.getWrappedRangeForLine traverse to
actual retained logical-line boundaries with no such 1000-row truncation, confirmed by fresh bounded excerpts at tip.
git diff from reviewed commit to tip is empty for grid-buffer.js and grid-buffer.test.js; grid-terminal changes concern
nearest-boundary rounding and input-report helpers, not line selection.

Possible coverage and caveats: none exact. SPEC_impl.md:1297-1304 requires grid reproduction of selection behavior with
file-header departures; neither file header declares a triple-click truncation. The adjacent contract at
SPEC_impl.md:3512-3517 bounds file-link detection, not selection. Tip TRIAGE_OUTCOMES.md:11304-11314 concerns recursive
selection inside vendored xterm and explicitly excludes first-party code. All required coverage sources searched;
TODO.md Planned remains empty and no queue/BUGS/plan match was found. The deterministic incomplete range is not a rare
transient filter match. Fix direction: boundary-only line traversal can use the retained source bounds without
allocating word-search mapping records.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g171/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f;
persists at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime occurrence and frequency
were not measured.

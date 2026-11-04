# Wide-wrap inference removes a real boundary space from copies and printed URL detection

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A real space printed at the right edge before a wide character can disappear when copied, and printed-URL detection can
incorrectly append the following wide text to the URL instead of stopping at that space.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static route;
no runtime reproduction or payload construction. Potentially-highest flag: yes for independent review of the named
displayed-target guarantee, but no credential loss, arbitrary-origin substitution or other concrete security harm is
established; accepted host-prefix matches retain their host in the examined path. No higher bucket is claimed from
hypothetical downstream server behavior.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-ui/assets/grid-buffer.js:101-105.

grid-buffer.js:101-105 infers synthetic wide-wrap padding solely from wrapped flag, absence of a last-column run and a
leading wide character on the next row; selectionText:138-140 and mappedText:207-208 then omit that column. The producer
at T crates/farhelm-supervisor/src/grid_output.rs:211-233 drops/trims default-style spaces even if printed, as its
:135-143 docs explicitly say. Thus the buffer helper's claim at :99 that a printed blank is covered by a run is not true
for ordinary default-style blanks. A row filled through its last cell by a real space can legitimately wrap before a
following wide character, and trimming removes the evidence that would distinguish that space from
LEADING_WIDE_CHAR_SPACER padding.

A real space printed at the right edge before a wide character can disappear when copied, and printed-URL detection can
incorrectly append the following wide text to the URL instead of stopping at that space. Source mechanism:
vendor/alacritty_terminal/src/term/mod.rs:1287-1288 writes an ordinary width-one space, :1324-1327 makes its last-column
position pending-wrap, and :1270-1273 wraps before the next character. wrapline:1099-1119 sets WRAPLINE without a
synthetic spacer; the distinct placeholder branch:1290-1296 is not reached after pending wrap has moved to column zero.
grid_output.rs:144-213 encodes that row, then :216-233 removes default-style boundary spaces. This loses the evidence on
which padsWideWrap at grid-buffer.js:101-105 relies. The heuristic mistakes that row for synthetic padding and drops one
cell from selectionText:138-140 and mappedText:207-208. Assigned grid-buffer.test.js:361-363 claims printed-space
preservation but manually sends a run ending in that default-style space, which the real sender trims. This is an actual
producer/fixture distinction, not a runtime-timing premise. Additional checked consequence: urlWindow:437-464 includes
the preceding wrapped row (including a space-bearing boundary row); rowLinks:322-333 matches over mappedText after that
real space is removed. URL_PATTERN:52 accepts non-whitespace wide text in a path, and hostPrefixed:424-433 only verifies
the unchanged scheme/credential/host prefix. isPlainWebUrl at terminal-links.js:90-101 accepts the resulting
no-whitespace path; terminal.js:1300-1313 retains that URI, :1233-1252 exempts ordinary printed URLs from target
disclosure and opens the retained URI on an admitted click. paintRow at grid-terminal.js:720-744 keeps column geometry
and does not concatenate the rows' visible text. Therefore copy/word extraction and ordinary printed-link extent can
differ from the original whitespace-separated output without a paint race.

Possible coverage and caveats: grid-buffer.js:32-34 broadly documents loss of printed trailing spaces, and
SPEC_impl.md:1297-1304 allows file-header departures; however :21-27 expressly preserves wrapped-row blanks except
actual wide padding, :99 promises printed blanks are covered by a run, and the assigned test claims exactly this
preservation. SPEC.md:2327-2331 justifies undisclosed plain URLs because shown text is what opens. These scopes
conflict; do not silently treat the generic printed-space omission as accepting this extra wrapped-boundary inference or
URL extension. SPEC_impl.md:1671-1682's actual-space preservation rule is about tagged OSC hover text, not the unlinked
default-style space here (linked spaces are retained by the producer), so it is not claimed as direct coverage. Full
required-source searches found no exact Planned item, unlanded plan, BUGS entry, queue item or ledger decision for this
cause; the path-name trailing-space presentation decisions are different. The deterministic ordinary-output behavior
does not meet the rare timing/diagnostic filter. Proposed fix direction: carry reliable wide-padding evidence or
explicitly document and accept the ambiguity, and test through the actual row encoder rather than a manually retained
default blank.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g171/f7`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb;
assigned helper/test patch introduced at 7509285e647ea86ccd2cee7905675dc87dad9b6f and unchanged at tip.. This was a
source-only review; runtime occurrence and frequency were not measured.

# A wide run can hide a separately erased second cell's background

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A program erasing only the second column of a wide glyph with a different background can leave grid viewers painting the
old first-cell background over that erased column.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static source
producer and encoded omission; no runtime reproduction, timing or physical observation premise required.

Source locations: crates/farhelm-supervisor/src/grid_output.rs:192-208.

at tip vendor/alacritty_terminal/src/term/mod.rs:1712-1727 erases only the requested cells with the current background;
it does not clear the preceding cell's WIDE_CHAR flag. The query wrapper delegates erase_chars unchanged
(queries.rs:323). Assigned row_from_cells checks the first cell's WIDE_CHAR flag, emits width2 whenever another column
exists (:192-206), then advances by2 (:208), without inspecting whether the following cell is still a matching spacer or
has been separately cleared/recolored. The independently erased column is therefore not transmitted.

A program erasing only the second column of a wide glyph with a different background can leave grid viewers painting the
old first-cell background over that erased column. Details/source basis at tip: VTE ansi.rs:1818 routes ordinary ECH to
erase_chars; QueryHandler queries.rs:283-295,323 delegates it unchanged. Term::goto at term/mod.rs:1347-1362 permits
positioning on the second cell without snapping to the head. erase_chars :1712-1727 replaces only the requested cells
using the current background; Cell::from(Color) at term/cell.rs:466-470 resets other fields/flags, so the erased cell is
no longer a wide spacer, while the preceding head retains WIDE_CHAR. R's corresponding erase code :1706-1722
independently confirms the same producer. Emulator::feed emulator.rs:151-160 parses and routes effects without repairing
this pair. Assigned row_from_cells grid_output.rs:192-208 nevertheless emits width2 for any WIDE_CHAR with room in the
row and skips the next cell without checking its flags/style. Browser grid-terminal.js:720-741 draws that single
two-column box with the head style, so the separately erased/recolored cell cannot appear. Snapshot painter
paint.rs:596-621 explicitly requires matching second-cell flags/colors/extras before classifying a wide Pair; mismatches
remain Fix, corroborating that arbitrary live cells need not retain this pair invariant. Suggested remedy: distinguish
intact wide pairs from independently changed cells and preserve the latter's background/attributes; choose the remaining
glyph representation consistently with emulator rendering rather than assuming adjacency proves a matching spacer.

Possible coverage and caveats: none exact after searching all required coverage sources at tip. SPEC.md Terminal
experience2080-2391 retains color fidelity at2085-2086; SPEC_impl grid-client1706-1736/Grid-stream2442-2482 have no
partial-wide-erasure exception, and Terminal snapshots2549-2598 requires preserving live cells via fixups rather than
accepting visual losses. TODO Planned23-25 empty. No matching plan/BUGS/queue/ledger entry found. Generic protocol
grid.rs:107-116 says wide-character spacers are omitted, but this cell has been erased to a new ordinary blank and no
longer has the spacer flag, so that is not exact coverage. Earlier emoji/xterm-only plan and ledger items do not change
this converter. Rare-self-correcting filter does not clearly apply: ordinary cursor-addressed erase can create the state
and reconnect repeats the same conversion. Dedup search of current sibling logs using erased-second-cell/half-wide terms
matched only this local finding; a larger preliminary broad keyword result clipped and was not credited as a complete
read. Distinct from f1's leading wrap placeholder, g171/f6's copy of an intact continuation, and g171/f7's trimmed
wrap-boundary-space heuristic; no new security consequence inferred.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g173/f5`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 70cd68a03c101493e0adbf00bb27bb8004adfe41;
encoder persists unchanged at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review;
runtime occurrence and frequency were not measured.

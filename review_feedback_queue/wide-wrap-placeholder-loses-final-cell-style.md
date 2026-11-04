# Wide-character wraps discard the last column's visible style

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A wide glyph wrapping from the final column leaves a default-background hole where the terminal actually holds a styled
blank; both screen and history use the conversion.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source
behavior; no runtime reproduction authorized.

Source locations: crates/farhelm-supervisor/src/grid_output.rs:153-159.

grid_output.rs:153-159 skips LEADING_WIDE_CHAR_SPACER before reading style. At tip
vendor/alacritty_terminal/src/term/mod.rs:1290-1296 writes that placeholder with write_at_cursor(' '), and :1123-1159
copies the pen's colors/flags. grid-terminal.js:721-744 draws only sent runs, and :790-796 paints uncovered terminal
area in default background, so no downstream paint recovers the missing style. Preserve a leading placeholder's visible
blank styling separately from an ordinary wide glyph's second cell, whose box already covers its style.

grid_output.rs:153-159 skips LEADING_WIDE_CHAR_SPACER before reading its style. At the same immutable commit
vendor/alacritty_terminal/src/term/mod.rs:1290-1297 creates that placeholder with write_at_cursor(' '), and :1123-1159
copies the current foreground, background, flags and extra into it; the placeholder is blank text, not necessarily
default style. E.g. a colored-background wide glyph at the final column leaves a styled blank there that the encoder
omits. The protocol says omitted cells are default-style blanks (crates/farhelm-proto/src/grid.rs:98-106).

Possible coverage and caveats: protocol grid.rs:107-116 says spacer cells are not sent while explaining how a wide cell
occupies one two-column box. A leading placeholder occupies the previous row outside that box; no text explicitly
accepts losing its style. Retain that wording ambiguity for triage, not a proposed exclusion. Full SPEC.md Terminal
experience at tip :2080-2391 read in synthesis; color-fidelity promise :2085-2086 and no matching exception. SPEC_impl
The grid client :1706-1736 and Grid streams :2442-2482 read in full; hyperlink-padding omission (:1662-1705, initial
pass) is text comparison only. TODO Planned :23-25 remains empty. Required coverage searches at fixed tip found no exact
cause/consequence/scope match in BUGS, unlanded plans, queue or ledger. Initial xterm-only plan/ledger matches remain
distinct. FILTER rare-self-correcting rule does not cover this ordinary colored wide-character margin write whose resend
preserves the error. Cross-group synthesis: g158/f1 was read; it resolves raw-vs-effective bright palette indices for
ordinary bold text, while this finding omits a styled cell before any palette mapping. Distinct causes and producers.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g173/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 70cd68a03c101493e0adbf00bb27bb8004adfe41. Tip
persistence verified at ca9b0a19856dc136f63ad2d14f1076027edce1eb by empty git diff for assigned encoder/client and
grid-stream sender, plus direct tip reads.. This was a source-only review; runtime occurrence and frequency were not
measured.

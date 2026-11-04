# Bold colors ignore redefined bright palette entries

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A program redefining bright terminal colors still gets the original theme colors for bold text.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static source
behavior, no runtime reproduction authorized or performed.

Source locations: crates/farhelm-ui/assets/grid-style.js:160; crates/farhelm-supervisor/src/grid_output.rs:258-288.

At reviewed commit and tip, grid_output.rs:238-288 resolves the raw cell foreground/background/underline indices to
palette indices or direct colors before the client knows whether bold/inverse will select a different index.
grid-style.js:145-166 swaps inverse colors and brightens palette indices 0-7 from its fixed theme. When entry 9 is
redefined and entry 1 is unchanged, bold red is sent as BOLD plus Palette(1), so the browser draws its old palette[9].
The override for entry 9 never reaches it. Recoloring or reconnecting repeats the same conversion. The raw cell
storage/OSC4 producer was established in the initial pass at 70cd68a03c101493e0adbf00bb27bb8004adfe41
vendor/vte/src/ansi.rs:1415-1439,1883-1910 and vendor/alacritty_terminal/src/term/mod.rs:1850-1857,2103-2117. Synthesis
additionally checked tip farhelm-proto/src/grid.rs:95-145 in full: :117-124 promises overridden entries become direct
colors so the browser need not know the program palette, but does not account for later bold index selection. Tip
vendor/xterm.js:1, targeted complete snippets containing drawBoldTextInBrightColors, confirms the reference DOM renderer
selects index+8 before reading S.ansi[index], for foreground and underline; the default option is true. Same cause
affects inverse effective foreground and bold underline color.

Possible coverage and caveats: none found. Initial full relevant SPEC.md Terminal experience section (2080-2391) and
SPEC_impl.md The grid client (1706-1736) plus Grid streams (2442-2482) were retained in context. Current coverage search
across SPEC.md, SPEC_impl.md, TODO.md, BUGS.md, unlanded plans/queue, queue INDEX, and TRIAGE_OUTCOMES.md found only the
same palette-redefinition specification statements, no accepted omission or cause-removing plan. TODO Planned is empty
(:23-25). The ledger color-list-parsing-performs-quadratic-work.md (:7577-7593) accepts hostile-input resource
exhaustion, not this ordinary incorrect rendering. FILTER rare-self-correcting display glitches does not cover colors
that remain wrong after refresh/reconnect. Cross-group synthesis: read g058/f2, g069/f1 and g173/f1 logs. g058 loses
palette before tmux migration seeding; g069 loses it during host-history Restart painting; g173 omits a styled
leading-wide spacer before style mapping. All are distinct from this live ordinary ASCII/BOLD mapping failure, whose
trigger needs no Restart, history seed or wide glyph. No duplicate or contradiction found.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g158/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 70cd68a03c101493e0adbf00bb27bb8004adfe41;
persists unchanged through pinned inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (empty git diff for
grid-style.js and grid_output.rs, plus direct immutable tip reads). Assigned mapper introduced in
7d52f1cda4c301e8811acf23263b6ab66ce1930a.. This was a source-only review; runtime occurrence and frequency were not
measured.

# Tmux migration discards redefined palette colors before building the history seed

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Restarting an old tmux session into a terminal host changes retained scrollback colors when the old program redefined
palette entries.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by static
source, replacing the initial investigated uncertainty. Exact dependency: parent-supplied official tmux 3.7c release
archive, SHA-256 7c60cae9a0e25288e2e24750aafc9e8800fc7fd4555e447e1b29ee4201cfb3bf, pinned in the project source pins. In
that source, input.c:2903-2949 accepts ordinary OSC4 and changes the separate palette via colour_palette_set,
colour.c:1083-1123 stores/looks up that mapping, and input.c:2448-2456 retains an SGR foreground as its index. Actual
display substitutes the palette in tty.c:2764-2785. In contrast, cmd-capture-pane.c:151-254 calls grid_string_cells on
the stored grid; grid.c:787-904 serializes indexed foreground/background/underline references, :1037-1053 calls those
emitters, :1084-1125 reads each stored cell unchanged. Neither capture nor serialization reads the palette or emits its
definitions. Farhelm tmux.rs:4105-4110 requests size/history only, :4163-4174 requests that escaped capture;
host_restart.rs:819-884 passes the lines into snapshot/mod.rs:735-780, which creates a fresh scratch emulator and only
feeds the captured lines. The lost palette values are already absent before the history encoder runs. SPEC.md at the
pinned tip, Lifecycle operations:1359-1401 (especially 1374-1382), promises scrollback text with its colors, styles and
links above a blank screen. Coverage: SPEC_impl.md:2195-2265 fully read, expressly omitting multi-row link
continuations, custom tab stops and setting lines lost at byte cuts, not palette values for an untrimmed ordinary
capture; Terminal snapshots:2549-2597 fully read, promises blank/default modes. TODO Planned:23-25 empty. Searches of
plans/queue, BUGS, queue INDEX/files and TRIAGE_OUTCOMES found no matching palette migration disposition;
color-list-parsing-performs-quadratic-work ledger section:7577-7592 accepts hostile resource exhaustion in browser
parsing, a different trigger/consequence/scope. FILTER rare display/diagnostic filter does not cover persistently
recolored retained history.

Source locations: fba73468a9cb09c217a39da2be7f2c7cea064cb8:crates/farhelm-supervisor/src/tmux.rs:4105-4110,4163-4174.

tmux.rs:4105-4110 captures only size and history length, and :4163-4174 uses capture-pane -e; its new test at :8501-8526
expects indexed red escape sequences. host_restart.rs:871-884 passes those lines to history_seed_from_lines_until.
snapshot/mod.rs:735-767 creates a fresh emulator, designates G1 and feeds the captured lines; no palette transfer is
supplied. Checked snapshot/v1/paint.rs:758-799: carried styles preserve named/indexed colors as SGR indices. Checked
queries.rs:18-28: documented tmux OSC4 query handling reaches xterm.js, which is not evidence of capture serialization.
Checked SPEC.md at ca9b0a19856dc136f63ad2d14f1076027edce1eb:1374-1396,2080-2091 and SPEC_impl.md:2195-2265,2520-2545:
preserved colors are promised, while documented migration omissions cover links, tab stops and styles lost at byte cuts,
not redefined palette entries. Coverage search: TODO Planned is empty (:23-25); no matching terminal-palette migration
item found in BUGS.md, queue INDEX/files, TRIAGE_OUTCOMES.md or plans/queue; UI appearance palette items have different
trigger and scope.

Possible coverage and caveats: snapshot/mod.rs:495-504 explicitly says default colors, and SPEC_impl.md:2583-2586 says
default modes; those clarify the new program state but do not clearly accept recoloring retained history. Preserve this
interpretive caveat for triage. Cross-group synthesis: g069/f1 is a separate downstream omission: paint_history at tip
paint.rs:413-448 omits term.colors(), whereas full paint preserves it at :281-296; Pen::of and push_color retain indices
at :677-683,788-799. Both affect the same user contract, but fixing only g069 leaves this capture-side loss intact
because the scratch emulator never receives the old palette; conversely feeding an old palette alone still reaches g069.
Retain f2 as the distinct tmux-capture cause and coordinate repairs without counting the shared encoder defect again
here. A repair must preserve resolved history colors across both boundaries while leaving new program defaults fresh.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g058/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is fba73468a9cb09c217a39da2be7f2c7cea064cb8.
Persists at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb: git diff between those revisions is empty for
tmux.rs, service/core/host_restart.rs, snapshot/mod.rs and snapshot/v1/paint.rs; relevant tip lines additionally read..
This was a source-only review; runtime occurrence and frequency were not measured.

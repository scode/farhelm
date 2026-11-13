# Host-to-host Restart recolors retained history after palette changes

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Restart into a fresh terminal host changes retained scrollback colors when the previous program redefined terminal
colors.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static source
behavior, no runtime work.

Source locations:
c345799ed43ddbd6e0c577f8f2a0add4dd051c6a:crates/farhelm-terminal-emulator/src/snapshot/v1/mod.rs:239-258.

encode_history delegates directly to paint_history. At the reviewed commit, paint.rs:391-415 repaints cells without the
term.colors() palette that full paint preserves at 256-271. Pen::of copies the cells' named/indexed references unchanged
(644-650); push_color emits those same SGR references (742-752), not their resolved RGB values. Consequently a line
printed in SGR 31 after OSC 4 redefines entry 1 restores against the fresh default entry 1 after Restart. At the
inclusive tip grid_output.rs:267-289 resolves a redefined entry to RGB for the viewer, proving this changes the
displayed history rather than merely internal representation. SPEC.md at the inclusive tip, Lifecycle
operations:1374-1384, promises retained scrollback text with its colors, styles and links above a blank screen in
default modes.

Restart into a fresh terminal host changes retained scrollback colors when the previous program redefined terminal
colors. Source: reviewed snapshot/v1/paint.rs:391-415 omits the palette that full paint preserves at 256-271;
Pen::of:644-650 and push_color:742-752 retain named/indexed references. At tip, v1/mod.rs:242-259 and
paint.rs:413-449,677-683,788-799 retain this behavior; snapshot/mod.rs:495-550 calls this encoder directly,
hosts/terminal.rs:3528-3570 invokes history_seed_until on the owned old emulator with no palette-resolution stage, and
service/core/host_restart.rs:683-697 uses its blob. The viewer resolves stored indices using the new emulator palette
(tip grid_output.rs:267-289, read in initial pass).

Possible coverage and caveats: reviewed snapshot/mod.rs:370-379 and tip :495-504 explicitly say default colors; tip
SPEC_impl.md Terminal snapshots:2583-2586 says default modes. Tip SPEC.md Lifecycle operations:1359-1401 promises old
scrollback with colors/styles/links above a blank screen in default modes, so whether default-colors wording accepts
recoloring existing history remains a contract ambiguity for triage. Coverage rechecked across SPEC.md, SPEC_impl.md,
TODO Planned (tip :23-25 empty), BUGS.md, plans/queue, queue INDEX and files, TRIAGE_OUTCOMES.md. No exact matching
prior decision/item found. FILTER rare self-correcting display glitches does not cover history whose colors stay
changed. Synthesis with g058/f2: related contract, distinct omission boundaries; g058/f2's boundary was the palette
missing from history captured from an old tmux terminal for Restart, a path that no longer exists now that old terminals
are ended at startup rather than moved into hosts.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g069/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is c345799ed43ddbd6e0c577f8f2a0add4dd051c6a; tip
persistence verified at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime occurrence and
frequency were not measured.

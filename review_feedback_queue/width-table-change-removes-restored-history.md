# Width-table history restoration remains an investigated uncertainty

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A changed character-width table could displace restored history or fixups, but no supported writing-and-reading build
pair with the required width change was found. Permanent history loss is not clearly covered by the accepted
restore-and-redraw limit.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. unverified after investigation

Source locations: c345799ed43ddbd6e0c577f8f2a0add4dd051c6a:crates/farhelm-terminal-emulator/src/snapshot/mod.rs:77-105.

Actual exclusive base, introducing commit and pinned tip use the same unicode-width0.2.3 checksum and fingerprint
calculation; no intervening upgrade found. Tests mutate metadata only. Preserve g071/f2, a033's useful checks, a034's
corrected AMBIGUOUS verdict and a035's ambiguity. Needed settling evidence is a concrete supported changed-table build
pair and affected retained state, not an imagined future dependency.

snapshot/mod.rs:77-109 explicitly documents width-table mismatch, overwrite of newly wide characters, a newly wide
last-column character scrolling an extra history row, misaligned fixups, and persistent history differences until they
scroll away. v1/mod.rs:481-485 (original code line offsets; mandatory patch p0179:487-492) returns width_table_changed
rather than refusing restore.

An emulator width-table change across updates can leave retained history with blank characters or shifted rows that
redraw cannot recover. Source basis and persistence: reviewed and tip ca9b0a19856dc136f63ad2d14f1076027edce1eb
snapshot/mod.rs:77-105 explicitly document changed-width overwrites, last-column wrapping, extra history scroll and
misaligned fixups; reviewed v1/mod.rs:481-486 and tip :499-504 return width_table_changed rather than refusing restore.

Possible coverage and caveats: that policy may accept the full consequence, but independent review must decide whether
its scope includes unrecoverable history changes. Coverage rechecked for width-table/Unicode-width/snapshot-history
terms in SPEC.md, SPEC_impl.md, TODO Planned (:23-25 empty), BUGS.md, unlanded plans/queue, queue INDEX/files,
TRIAGE_OUTCOMES.md; no separate matching disposition found. The emoji streaming plan's unrelated xterm
timing/Unicode-width mention is not coverage. Synthesis: this is separate from f1 (palette state absent during Restart),
and neither source confirms the other. Remains open solely as a potentially-highest proposed exclusion; no fresh
independent reviewer verdict occurred in this session, and agreement by another primary reviewer would not provide one.

Additional reviewed locations: g071/f2: crates/farhelm-terminal-emulator/src/snapshot/v1/paint.rs:884-910; tip:925-945.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g069/f2`, `g071/f2`.
Original append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs.
Locations refer to the named reviewed revisions; the original revision context is
c345799ed43ddbd6e0c577f8f2a0add4dd051c6a.. This was a source-only review; runtime occurrence and frequency were not
measured.

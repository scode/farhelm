# Cursor-template validation runs after budget exhaustion can erase the invalid marks

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A malformed terminal checkpoint can be accepted during recovery when its cursor template contains forbidden combining
characters and the attribute-storage budget is exhausted.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by static
source/control-flow and fixed-limit arithmetic; no runtime reproduction or payload was constructed.

Source locations:
c345799ed43ddbd6e0c577f8f2a0add4dd051c6a:crates/farhelm-terminal-emulator/src/snapshot/v1/wire.rs:674-685.

wire.rs:617-624 retains the input mark count only inside Reader::cell; :654-668 applies marks through the lossy storage
setters. Reader::cursor at :678-685 validates only the already-built template, so it misses a nonempty encoded mark list
after all requested marks have been dropped. vendor/alacritty_terminal/src/term/cell.rs:145-151 drops a mark requiring
an extra when the remaining budget is below CELL_EXTRA_COST. Source reachability: snapshot/v1/mod.rs:405 sweeps before
fixups, then :416-430 accepts repeated fixup targets without another sweep, and :431-449 subsequently reads cursor
fixups. wire.rs:664-668 retains each distinct successfully made extra in Extras.shared even when its target is
overwritten. The 64 MiB budget and 160-byte charge (vendor/alacritty_terminal/src/term/mod.rs:89-95) allow 419430
distinct underline-only extras, leaving 64 bytes, insufficient for another extra. RGB underline content gives more than
that many distinct keys; 419430 ordinary 28-byte cell-fixup records require under 12 MiB, within the host's 16 MiB blob
limit. No large live grid is needed. An invalid cursor mark list not already shared is then dropped before cursor
validation. The resulting cursor itself is valid; this finding claims malformed-input acceptance only, not a dangerous
returned cursor. Contract: snapshot/v1/mod.rs:74-83 explicitly declares a cursor template with combining characters
malformed; tests/snapshot.rs:541-565 separately expects its rejection. SPEC_impl.md at pinned tip:2571-2576 requires a
typed refusal for corrupt/forged blobs and valid fixups.

Reader::cell reads the wire combining-character count, then creates the extra through ExtraStorage::push_zerowidth,
which can drop the requested marks when there is no room. Reader::cursor checks only the resulting template.zerowidth(),
after that lossy construction; it no longer knows the wire template was invalid. The decode fixup loop can retain
distinct fixup extras in the sharing table even after overwriting the same target, so reaching the storage cap does not
require a large live screen.

a malformed terminal checkpoint can be accepted during recovery when its cursor template contains forbidden combining
characters and the attribute-storage budget is exhausted. Farhelm silently restores a normalized template instead of
returning the promised malformed-checkpoint error.

Possible coverage and caveats: no exact coverage found after searches of SPEC.md, SPEC_impl.md, TODO.md Planned,
plans/queue, BUGS.md, review_feedback_queue/INDEX.md and TRIAGE_OUTCOMES.md. TODO.md Planned is empty at pinned
tip:23-24. SPEC_impl.md:2588-2597 and snapshot/mod.rs:116-131 allow normal attribute degradation when the budget is
full; they do not explicitly permit accepting this forbidden encoded cursor-template field. The rare-diagnostic filter
is not a clear complete match because a requested decode succeeds despite its specified malformed-input condition.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g074/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is c345799ed43ddbd6e0c577f8f2a0add4dd051c6a;
persists at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime occurrence and frequency
were not measured.

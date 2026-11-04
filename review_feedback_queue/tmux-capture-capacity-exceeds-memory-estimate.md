# Tmux history capture retains vector capacity beyond its documented memory bound

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Restart migration of dense styled history can retain a capture allocation near 128 MiB despite the documented roughly 64
MiB capture peak, adding avoidable supervisor memory pressure while history is encoded.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source-level
capacity/bound mismatch on the inspected Rust 1.99.1 standard library; physical RSS and practical exhaustion are
unmeasured and not claimed.

Source locations: fba73468a9cb09c217a39da2be7f2c7cea064cb8:crates/farhelm-supervisor/src/tmux.rs:5007-5017.

tmux.rs:5014 appends to tail before :5015 tests length > 2 * max_bytes, then :5016 drains old bytes without reducing
capacity. Production max_bytes is 32 MiB (host_restart.rs:257-273,819-829; terminal-host protocol.rs:41 MAX_SNAPSHOT is
16 MiB), and reads are up to 64 KiB (tmux.rs:2003-2006,4499-4509). On crossing the 64 MiB compaction threshold, a
near/full 64 MiB vector can grow to approximately 128 MiB before drain; drain retains that allocation. Final trimming at
tmux.rs:4200-4208 also only drains, and the Vec is moved into PaneHistory and then the blocking history encoder rather
than immediately released. The capture payload remains logically bounded; the defect is allocated capacity, not
unbounded retained content. Inspected installed official Rust source identified by its distribution manifest as 1.99.1
(58cf8f9c3 2026-10-13), date 2026-10-15: alloc/src/vec/mod.rs:3605-3606 extend_from_slice -> spec_extend;
vec/spec_extend.rs:50-56 -> append_elements; vec/mod.rs:2959-2962 -> reserve; raw_vec/mod.rs:503-533,652-674 uses
max(old_capacity * 2, required_capacity); vec/mod.rs:3022-3049 and vec/drain.rs:173-238 only move retained elements/set
length and never shrink allocation. No runtime/payload construction was performed. Contract: SPEC_impl.md at tip Moving
an old session into a host:2195-2265, especially 2220-2222, explicitly promises about 64 MiB capture peak at twice the
32 MiB bound; host_restart.rs:271-272 and tmux.rs:4074-4079 repeat it. A capture above that threshold is admitted by the
row-bound path, which can serialize independent true-color foreground/background for each stored cell (official tmux
3.7c grid.c:787-881,1037-1047 previously read); no allocation-aware cap exists in HistoryCapture. Coverage: searched
SPEC.md, SPEC_impl.md, TODO Planned, BUGS.md, unlanded plans/queue, queue INDEX/files and TRIAGE_OUTCOMES.md for
capture/capacity/allocation/compaction memory cases. TODO Planned:23-25 empty. Near-term TODO Assess the grid terminal
history memory:215-221 concerns browser JavaScript representation, not supervisor capture allocation. Ledger
unbounded-aggregate-capture-replies.md:12332-12340 concerns an older parser without a whole-reply cap, bounded by rows
times width and discarded without a demonstrated trigger; the new streaming HistoryCapture has a whole-reply logical
bound but exceeds its explicit allocation estimate on standard growth, so trigger/cause/scope do not match exactly. No
exact covering queue item, plan, known bug or ledger decision found.

Possible coverage and caveats: the older aggregate-capture ledger decision is named for human interpretation, but is not
treated as exact coverage. Repair direction: compact before an append would cross the chosen allocation ceiling, or use
a buffer whose capacity is explicitly bounded; shrinking after growth alone does not restore the claimed peak.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g058/f3`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is fba73468a9cb09c217a39da2be7f2c7cea064cb8;
persists at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (relevant tmux.rs and host_restart.rs diff empty; tip
lines read).. This was a source-only review; runtime occurrence and frequency were not measured.

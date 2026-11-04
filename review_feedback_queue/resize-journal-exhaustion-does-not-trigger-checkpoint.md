# Quiet-terminal resizes can exhaust the journal without a new checkpoint

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Enough applied size changes on a quiet terminal can consume its retained journal without making a new checkpoint due. A
later crash can lose recoverable output. The newer small-tail rule can lose that history on an orderly stop too, despite
reporting success. The required count is extreme; both conditional routes are source-established.

## Details

### Additional orderly-stop consequence

Additional reviewed commit: 25f85df8b9d09481d2075aaca2565f1bc8ad0aa7; unchanged through
c5ae8548439cc5bfa33c11ef7ce8da6129d8072a. Previously an orderly stop could save the live emulator even after resize
records had evicted old output from the host journal. The new skip removes that opportunity when processed output since
the snapshot is below its threshold.

`crates/farhelm-supervisor/src/hosts/terminal/checkpoint.rs:87–91` counts only output offsets. The gather at
`service/core/host_handover.rs:331–340` aborts a small-tail runtime without encoding, and the runtime applies the same
test at `hosts/terminal.rs:3656–3662`. Meanwhile each applied resize costs 64 journal bytes; trimming at
`crates/terminal-host/src/journal.rs:475–509` can remove the original output while the connected emulator still retains
it. With 64 output bytes and no snapshot, 524,288 subsequent applied alternating resizes exhaust the default 32 MiB cap.
The live reader has already consumed the old output, so this trimming need not make it encounter a gap. Shutdown sees
only a 64-byte output tail, skips, and reports success. The next adoption has neither that output nor a snapshot of the
intact emulator. This sequence is source-derived, not runtime-reproduced; coalesced or unchanged sizes do not count.

SPEC_impl.md, "Across supervisor restarts", at the reviewed commit 2354–2358 says skips retain the snapshot and all
later output for "the same terminal" while excluding resizes from admission. That policy does not explicitly accept a
previously exhausted journal or loss on an otherwise successful orderly stop. SPEC.md's accepted gaps concern absent or
behind supervisors and oversized/fast program output, not this ordinary resize producer. The existing record already
retains the coverage ambiguity; no new acceptance or filter resolves it. The checkpoint-performance plans are landed,
TODO's Planned bucket is empty, and targeted known-bug, ledger and queue searches found no separate cover for the added
consequence.

Suggested bucket remains **highest**. Confidence: **source-confirmed conditional history-loss route**, requiring an
extreme cumulative resize count; no files or processes are alleged lost. Keep the automatic-trigger defect and this new
shutdown interaction in one record. A repair must account for charged resizes or establish that the saved snapshot and
journal can still reconstruct the live terminal before skipping. Provenance: review run
`ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, `g051/g051-l4`.

### Original automatic-checkpoint consequence

Suggested bucket: **highest**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source route; extreme cumulative resize count and later crash required

Source locations:
0d3cf3c5b4648ea9d07f9d1ed5057aff586f56d4:crates/farhelm-supervisor/src/hosts/terminal/checkpoint.rs:155-160.

Count actually applied distinct resizes, not coalesced identical requests. Coalescing does not bound the lifetime total.
Output-only checkpoint policy and accepted unavoidable journal gaps do not clearly accept this preventable resize
pressure; independent exclusion remains ambiguous.

checkpoint due-ness counts only position.offset minus attempted_at.offset, ignoring position.resizes
(checkpoint.rs:155-160), and the default floor is 1 MiB. The host journal charges 64 bytes per resize (tip
crates/terminal-host/src/protocol.rs:44-49; journal.rs:230-240), trimming oldest output or resizes when its cost exceeds
the cap (journal.rs:471-509). At the default 32 MiB cap (tip hosts/spawn.rs:90-100), 524,288 resize records consume the
cap by themselves. With positive output below the checkpoint floor and no snapshot yet, enough later resizes trim that
output without a checkpoint ever becoming due. Shipped producer: a measured dimension change invokes
grid-terminal.js:439-459; terminal.js:5323-5330 supplies sendResize, which sends dimensions at 6028-6034, and resize
events/ResizeObserver refit at 6076-6108. The supervisor's attached-owner handler dispatches it at
service/handlers.rs:2088-2166, service/terminals.rs:267-280 forwards to the host runtime, and
hosts/terminal.rs:2251-2260,2884-2898 sends changed sizes to the host. The host unconditionally journals each applied
request at terminal-host/src/host.rs:804-819. Coalescing and same-size suppression reduce the number of applied changes
but do not bound the lifetime count. Runtime resize/gap handling does not force an otherwise-not-due checkpoint (tip
hosts/terminal.rs:3665-3720,3786-3798). A later non-orderly supervisor exit cannot supply the orderly checkpoint;
recovery has only the remaining host journal.

checkpoint due-ness counts only position.offset minus attempted_at.offset, ignoring position.resizes; the unit test
explicitly requires a resize alone not to be due. The host journal counts output bytes plus RECORD_COST for each resize
and trims oldest records once capped (ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/terminal-host/SPEC.md:173-189).
Thus resize growth can trim output without making a new checkpoint due.

A long-lived terminal with little new output can lose recoverable terminal output after enough window-size changes and a
later supervisor crash, even though the supervisor had kept reading the terminal.

Possible coverage and caveats: those exact specification paragraphs and terminal-host/SPEC.md:173-189 (substrate
trimming contract); trigger/consequence coverage is ambiguous, not treated as settled. Other coverage checks: TODO.md
Planned is empty at tip 23-24; targeted resize/journal/checkpoint/RECORD_COST searches of SPEC.md, SPEC_impl.md,
TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, plans/queue and review_feedback_queue including INDEX found no same
trigger/consequence/scope entry. FILTER.md does not cover a retained-output loss route.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g133/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 0d3cf3c5b4648ea9d07f9d1ed5057aff586f56d4.. This
was a source-only review; runtime occurrence and frequency were not measured.

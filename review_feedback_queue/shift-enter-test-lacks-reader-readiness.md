# Shift+Enter integration test substitutes a delay for its blocked-reader premise

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

This integration test can report success without establishing the already-waiting reader its documentation requires.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source-level
oracle/premise gap; no measured flake, reproduced false pass or production defect claimed. Important synthesis caveat:
host.rs:4536-4569 already has a deterministic datagram-backed test proving the host offers a small Input in one write;
host.rs:591-594 implements that write. Therefore this finding does NOT mean single-write behavior lacks all coverage. It
concerns the misleadingly strong integration-test contract and timing substitute. A fix can keep end-to-end byte
delivery coverage and rely on the deterministic host test for the syscall boundary, or replace the timing premise with a
real observation.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm/tests/terminal_host_client.rs:691-703.

terminal_host_client.rs:692 prints ready before starting dd; :697-700 observe ready, sleep 200 ms, then send ESC+CR; no
source synchronization establishes a blocked dd read. Both bytes can be available before that read, so :701-703
establish the observed read contents but not the promised write boundary.

line 692 emits ready before starting dd; lines 697-700 observe ready, sleep 200 ms, then send ESC+CR. No observation
establishes dd is blocked in read. If dd starts late, the PTY can queue both bytes before its first read, so separate
writes can still satisfy lines 701-703. Fix: establish an observable blocked-reader premise, or observe the host write
boundary directly with a deterministic test.

this integration test can report success without establishing the already-waiting reader its documentation requires.

Possible coverage and caveats: none of the permitted coverage sources accepts this test premise; separate lower-level
test coverage limits consequence but is not spec/TODO/queue/ledger coverage of this defect. SPEC.md:2094-2101 requires
the behavior; SPEC_impl.md:1594-1608 describes its rationale. TODO Planned is empty; searches of unlanded plans/queue,
BUGS.md, INDEX/matching queue items and TRIAGE_OUTCOMES.md found no exact matching decision.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g109/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb;
introduced by assigned commit 1eef3d64e36cd31b56e5b7e815fa3cc183b55637. git diff between those immutable commits for
this file is empty.. This was a source-only review; runtime occurrence and frequency were not measured.

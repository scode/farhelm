# Stop regression never establishes that Stop is waiting for the exit record

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The regression can miss Stop ceasing to wait for final terminal output, leaving a lost opportunity to return the
complete launch-failure explanation undetected.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source-level
test-oracle gap; no missing source premise retained.

Source locations:
7962307589c2a87859fdca3a0eea788fb2389f17:crates/farhelm-supervisor/src/service/core/tests/host_terminals.rs:668-693.

at the originating commit the test spawns Stop at service/core/tests/host_terminals.rs:668-680, immediately queues
output and exit at681-682 and checks the final error at688-693. At T those exact operations survive at673-698. The test
does not witness the handler entering its final-state wait before releasing the record. Its own T docstring at652-663
expressly permits the record to precede Stop's probe. A concrete competing implementation replaces the awaited wait with
an immediate final-state readiness check: the no-record test at T:582-650 still records its expected plain exit; this
test can pass on its permitted already-recorded schedule; and T:724-751 still proves the original helper waits,
independently of whether Stop awaits it. This is a static oracle distinction, not a claim of an observed flake or a
runtime-verified mutation result. T service/handlers.rs:971-975 still correctly awaits holds_final_state, so no
production Stop defect is established. Proposed fix: witness Stop pending at its final-state wait before releasing the
exit record, then assert the complete launch failure.

Possible coverage and caveats: none exact. At T, SPEC_impl.md:1973-2003 requires Stop to wait for the final record up to
two seconds; TODO.md:23-25 has no Planned items. Searches of SPEC.md, SPEC_impl.md, BUGS.md, plans/queue, review queue
INDEX and matching files, and TRIAGE_OUTCOMES.md found no exact trigger/consequence/scope cover. The similar queue item
shutdown-wait-test-does-not-witness-waiting.md:1-18 and TRIAGE_OUTCOMES.md:19276-19298 concern supervisor shutdown
waiting for launch registration, not a session Stop waiting for its exit record. FILTER.md's rare runtime
diagnostic/session-record exceptions do not cover a persistent test-oracle gap. Interface check: T
crates/farhelm/tests/e2e/host_terminals.rs:34-38 delegates the on-cue Stop/exit-record boundary to these unit tests. Its
Stop regression at281-332 tests the live-child termination path and separately waits for Exited after Stop returns
(301-309), so it does not establish the missing dead-child wait boundary. Searches for
holds_final_state/HOST_EXIT_RECORD_WAIT and exact test names across supervisor and e2e source found no separate
deterministic integration witness.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g138/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7962307589c2a87859fdca3a0eea788fb2389f17;
persistence verified at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (T).. This was a source-only review;
runtime occurrence and frequency were not measured.

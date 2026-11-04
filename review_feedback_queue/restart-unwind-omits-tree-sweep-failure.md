# Restart unwind contract omits ambiguous process-tree cleanup failures

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The authoritative implementation specification incorrectly says a failed Restart restores the stopped old terminal
whenever the replacement host can be ended; failed cleanup of processes the replacement started is another ambiguous
path, leaving no displayed terminal.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source/document
mismatch.

Source locations: 32e20f84647951a8ea717b3c712a037c231d92dd:SPEC_impl.md:2067-2075; persists at
ca9b0a19856dc136f63ad2d14f1076027edce1eb:SPEC_impl.md:2157-2165.

SPEC_impl.md:2067-2075 says a failed replacement is unwound definitively and only an unconfirmed replacement-host end
retains the launching record. At that immutable commit
crates/farhelm-supervisor/src/service/core/host_restart.rs:741-749 handles unconfirmed host end, but :750-755 removes
launch artifacts and the host record before :759-771 returns RelaunchFailure::ambiguous on a failed reap_process_tree.
That second ambiguous path contradicts the exhaustive condition and the claim that both hosts are then reachable by
their launch records: the replacement host is already ended and its record was removed. The same documentation remains
at ca9b0a19856dc136f63ad2d14f1076027edce1eb:SPEC_impl.md:2157-2165. Suggested correction: document the unconfirmed
process-tree cleanup branch separately, including its retained row/no-terminal result and which host records remain.

The authoritative implementation specification incorrectly says a failed Restart restores the stopped old terminal
whenever the replacement host can be ended; failed cleanup of processes the replacement started is another ambiguous
path, leaving no displayed terminal. At the pinned tip, service/core/host_restart.rs:1190-1198 returns ambiguous for
unconfirmed host shutdown, then :1199-1205 removes launch artifacts/host record, and :1208-1220 independently returns
ambiguous when reap_process_tree fails. service/core.rs:13507-13512 retains Launching and :13557-13579 publishes no
terminal and aborts the previous host runtime. SPEC_impl.md:2157-2165 still says only unconfirmed replacement-host end
retains the launching record, and that both hosts remain reachable by records. The failed-sweep path can have an already
ended replacement host and a removed host record. Adjacent migration wording at :2250-2253 inherits the same incomplete
exhaustive condition; this is the same documentation-contract omission, not a separate runtime bug. Suggested
correction: distinguish confirmed unwind from failed process cleanup and describe actual row/terminal/record
disposition.

Possible coverage and caveats: none found. T:SPEC.md:1534-1542 supports conservative failure on unconfirmed cleanup; it
does not accept an inaccurate implementation contract. Re-searched pinned-tip
SPEC/SPEC_impl/TODO/BUGS/plans/queue/queue/ledger for the cleanup/ambiguity wording. Matching plan
tab-output-hangup.md:1-139 concerns unintended tab signals and tmux ownership checks, not documenting failed
agent-restart sweeps; no trigger/consequence/scope match. Previously read empty Planned bucket, BUGS, matching queue
replacement-cleanup-missing-pane-root-source-gap.md and ambiguous-restart ledger retain their nonmatching scopes.
Runtime diagnostic/status filters do not cover this persistent specification mismatch.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g010/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 32e20f84647951a8ea717b3c712a037c231d92dd.. This
was a source-only review; runtime occurrence and frequency were not measured.

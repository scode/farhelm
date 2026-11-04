# Tab-end backoff regression measures observation time rather than retry time

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The retry regression can accept cleanup observed after 900 ms without proving that the retry itself waited for the
required delay.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed test-evidence
mismatch by source; no runtime false pass, actual flake, or production timing defect established.

Source locations:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm/tests/e2e/host_tabs.rs:1618-1624,1639,1656-1679.

the fault seam records only call counts at 1590-1595; failed_at starts after Close returns at 1611; elapsed >=900ms is
asserted only after polling final host/record cleanup and awaiting an additional listing at 1628-1647. An early
successful second attempt followed by delayed observer scheduling satisfies every assertion, and delayed reply delivery
can shorten the measured interval even with correct retry timing. Source contrast:
service/core/host_tabs.rs:174-184,266-294 at the same commit schedules retry_at from failure, not receipt of the reply,
and end_tab_host:1060-1063 calls the fault seam at the attempt boundary. Record both attempt timestamps in that seam and
assert their difference, while keeping cleanup/count assertions.

the retry regression can accept cleanup observed after 900 ms without proving that the retry itself waited for the
required delay. At tip host_tabs.rs:1618-1624 the fault hook stores only a count; 1639 starts time after Close's reply
is observed; 1656-1679 polls completed host/record cleanup, awaits another listing, then asserts elapsed >=900ms and
exactly two hook calls. No assertion records the retry attempt's time. An early attempt after the initial
held/record/host premises at 1646-1654, followed by later cleanup/observer scheduling, has the same final observations.
This is a source-confirmed oracle gap, not a measured flake or claim of an observed schedule. Accessible source confirms
separate async tasks via hook_identity.rs:214-216 and harness.rs:999-1006; testtrace-macros/src/lib.rs:28-38,247-259
selects current-thread and an unpaused clock here, so no multithread execution assumption is made. Production
host_tabs.rs:1022-1059 runs an awaited marker-only sweep after the first failed end, then mark_end_failed at 1051;
mark_end_failed:297-307 schedules retry_at from that later instant, and reap_due:192-198 checks it. Correction to
original proposed fix: two fault-hook timestamps alone also include the pre-scheduling sweep and can credit cleanup time
as backoff. Capture the retry-scheduling boundary and next end-attempt boundary, or use a deterministic clock/gate
proving no attempt before the deadline, retaining final cleanup/count assertions.

Possible coverage and caveats: no exact acceptance/planned/queue coverage found in the initial focused search across all
required sources. SPEC_impl.md:2078-2119, especially 2111-2114, explicitly specifies retry delay. Unit tests of
retry_delay arithmetic and reap_due selection do not establish this integration oracle's observation boundary.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g182/f3`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7987d070e91684f4c52df7695136cb87e66bd94f;
unchanged at ca9b0a19856dc136f63ad2d14f1076027edce1eb except preceding line shifts.. This was a source-only review;
runtime occurrence and frequency were not measured.

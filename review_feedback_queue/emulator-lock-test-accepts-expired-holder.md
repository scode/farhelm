# Emulator-lock regression silently accepts its holder expiring before measurement

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The test for nonblocking terminal input can pass after its emulator lock has already expired, so it can miss a
regression that would wait for that lock.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
fixture state and acceptance path by static source; no measured mutation survival/false pass/flake or current production
input failure.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb
crates/farhelm-supervisor/src/service/grid_stream/tests.rs:1305-1307,1320-1337.

the test for nonblocking terminal input can pass after its emulator lock has already expired, so it can miss a
regression that would wait for that lock. Source: the holder sends ready while inside host.read, then discards
recv_timeout(WAIT)'s result (tests.rs:1305-1307). WAIT is ten seconds (:31). The stopwatch begins only inside a
separately scheduled spawn_blocking closure (:1320-1323), so delay before that closure starts is excluded. If the
closure starts after the holder's bound, the emulator is unlocked and even lock-waiting input can complete in under the
asserted second (:1330-1332). host.read returns Result<R, EmulatorLost> where R is this closure's unit result
(hosts/terminal.rs:2059-2082); holder.await's two expects (:1334-1337) therefore check task/reader success but never
distinguish timeout from the intended release. Tokio 1.53.2 src/task/blocking.rs:83-121 describes independently
scheduled blocking work, including queueing, and provides no start-before-holder-deadline contract. This finding does
not assert that this small fixture saturates the default blocking pool: delayed main-thread or measured-worker
scheduling is the conditional trigger, whose frequency is unmeasured. Fix: propagate the holder's recv_timeout result
and assert successful receipt after join, preserving the finite cleanup bound; optionally coordinate measurement start
under the held-lock premise.

Possible coverage and caveats: none found after searches across SPEC.md, SPEC_impl.md, TODO.md Planned, BUGS.md,
unlanded plans/queue, full previously read queue INDEX and matching queue files, and TRIAGE_OUTCOMES.md for this symbol
and emulator-lock/holder-timeout/recv_timeout/stopwatch terms. Existing full Grid streams contract requires input not
waiting on the emulator; it does not accept an expired test premise. Broad blocking-pool references concern application
work, not this test's ignored timeout. FILTER.md's rare safe-failure/diagnostic acceptance is not established as an
exact match to a regression false-pass oracle; keep for triage if that boundary is judged broader.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g178/f4`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb. Test
introduced at assigned commit 70cd68a03c101493e0adbf00bb27bb8004adfe41 and verified unchanged through tip during earlier
passes.. This was a source-only review; runtime occurrence and frequency were not measured.

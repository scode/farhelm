# Running-runtime cancellation regression does not establish cancellation before releasing its encoder

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The regression that promises to cancel a Restart caller during history encoding can instead let that caller finish
normally, without rejecting the failed cancellation premise.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source-level
fixture-boundary gap; scheduling frequency unmeasured.

Source locations: crates/farhelm-supervisor/src/service/core/tests/host_terminals.rs:2962-2964 at
ca9b0a19856dc136f63ad2d14f1076027edce1eb.

host_terminals.rs:2930-2979 runs multi-threaded, observes EncodeGate entry, calls seeding.abort(), immediately
gate.open(), and never awaits/checks seeding. Exact pinned primary dependency Tokio 1.53.2 (tip Cargo.lock:6277-6280),
src/runtime/task/harness.rs:118-128 queues remote cancellation; :192-229 polls an already-running future to completion
and accepts Poll::Ready without checking cancellation, cancelling only after Pending. Therefore an already-running
caller poll may be descheduled after sending its seed request but before polling its answer; the parent sees encoder
gate entry, requests cancellation and opens the gate; the runtime completes seed_now (tip hosts/terminal.rs:3535-3569)
and sends the answer; the caller's existing poll resumes and returns Ready normally. The test checks no JoinError and
accepts that unexercised-cancellation route. This establishes a missing premise and permitted source schedule, not a
measured flake or a demonstrated production history-loss bug. Require seeding.await to yield a cancelled JoinError while
the encode gate is still closed, before gate.open.

Possible coverage and caveats: no exact acceptance found in initial coverage searches. The browser-effect-witness
discard at TRIAGE_OUTCOMES.md:20368-20377 addresses another caller/mechanism, while the contained-test-fix decision at
:11580-11595 does not exempt this gap.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g141/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb;
introduced by 32e20f84647951a8ea717b3c712a037c231d92dd:2952-2954.. This was a source-only review; runtime occurrence and
frequency were not measured.

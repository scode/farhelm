# Async process verification leaves stale signalling authority

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A process can exit and its number be reused after asynchronous identity verification but before Farhelm sends its
signal, allowing unrelated work to be signalled. The accepted adjacent synchronous window explicitly excludes this
scheduling gap.

## Details

Suggested bucket: **highest**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source route; actual PID reuse unobserved

Source locations: crates/farhelm-supervisor/src/hosts/process.rs:455-466,489-501,539-546.

Preserve the in-range async introduction and pinned process.rs455–466/489–546 route. A semaphore limits concurrent
process-table work, not the age of verified authority. Same-run fixture PID acceptance does not cover production async
signalling.

this patch moves verify to WalkGate::run/spawn_blocking (489-500; gate 340-351) but leaves SIGTERM and SIGKILL on the
awaiting async task at 455 and 466. verify's last identity check therefore precedes a scheduler/channel handoff; signal
at 539-556 performs no fresh identity read. A completed true result can remain queued while the async task is not
polled. The nominal ten-second timeout does not establish freshness: the Cargo.lock-pinned Tokio 1.53.2
time/timeout.rs:211-221 polls the completed inner JoinHandle before its expired timer, so an already-ready true result
can still succeed after an arbitrarily delayed poll. Keep the final identity check and signal adjacent in the same
blocking operation (or use a kernel process handle); preserve the existing no-signal-on-cancel semantics when choosing
the implementation.

verify performs its final start-identity read at process.rs:521, returns true at 533 on a blocking thread through
WalkGate:340-351, then terminate_within awaits that result and later calls numeric kill at 455/466 and 539-546 without
any new identity comparison. Shipped routes reach it after failed protocol shutdown or a host that has not ended yet
(hosts/stop.rs:486-495,516-537,578-604); the host may still exit while termination is pending. This is asynchronous work
between the identity observation and signal, not the adjacent synchronous syscall window accepted elsewhere. Synthesis
checks: pinned Tokio 1.53.2 runtime/mod.rs:279-304 provides only a workload-dependent MAX_DELAY and no task ordering
guarantee; timeout.rs:211-221 polls a ready result before its expired timer. The ten-second wrapper thus does not impose
a short signal-authority lifetime. After the worker verifies, another runnable process can outlive/reuse the departing
host's numeric PID before the waiting task is next scheduled; once resumed, the stale true result authorizes kill. No
further shipped prerequisite is needed beyond that concurrent exit/reuse schedule.

Terminal-host cleanup can signal an unrelated process if the host exits and its PID is recycled while a completed
verification awaits its async continuation.

Possible coverage and caveats: those exact rules accept only short bounded PID reuse and explicitly exclude bare numbers
carried across asynchronous work. Same-run development-tooling reuse cannot cover this production supervisor path. TODO
Planned is empty; previous searches of SPEC, implementation spec, unlanded plans, BUGS, queue/index and ledger found no
exact async-handoff acceptance or remedy. f3 deliberate same-account imitation and g018/f1/g019/f1 ordinary socket-name
collision are different triggers and are not coverage for f2.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g108/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is e354c3287db51075ae463dc61df5e257afabb6db;
verified persistent at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime
occurrence and frequency were not measured.

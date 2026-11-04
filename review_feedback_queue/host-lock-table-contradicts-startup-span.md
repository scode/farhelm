# The host-adoption lock table falsely forbids holding the lock across a host connection

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Farhelm's maintenance documentation states a lifecycle-lock constraint that startup deliberately violates, obscuring
which recovery operation may perform host I/O while holding the lock.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static source
contradiction.

Source locations: a4345e86a3f51f4c5abf3b2e97737991df900142:crates/farhelm-supervisor/src/service/core.rs:4660-4665.

Farhelm's maintenance documentation states a lifecycle-lock constraint that startup deliberately violates, obscuring
which recovery operation may perform host I/O while holding the lock. Details/source basis: A core.rs:4660-4665 says
host_adoption is Never held across a connection to a host, immediately after describing serve's reload holding it from
collection of kept terminals through map replacement. But A :8017 acquires the guard, :8031-8045 awaits
reload_sessions_with_hosts under it, and :8054-8055 replaces the map then drops the guard. reload_sessions_with_hosts
calls attribute_host_rows at A :6670; host_handover.rs:537-625 scans the registry and adopts hosts before returning. T
preserves the same scope at core.rs:8317-8368 and host_handover.rs:665-768. This is ordinary startup with recoverable
hosts, not an exceptional schedule. The no-connection critical section is accurate for re-adoption's install step (A
host_handover.rs:946-955 probes before install; :1052-1080 locks only around the swap), whereas serve intentionally
serializes the whole reload to avoid a swap into the map being replaced. Correct the table to qualify that prohibition
for re-adoption, while documenting the startup exception; no locking-behavior change is requested.

Possible coverage and caveats: no exact acceptance or duplicate found. Coverage searches included SPEC.md, SPEC_impl.md,
TODO.md's empty Planned bucket (:23-25), BUGS.md, all plans/queue, queue INDEX/matching adoption item and
TRIAGE_OUTCOMES.md. T SPEC_impl.md:2360-2388 describes the intended whole-reload versus re-adoption distinction and
supports correcting the table rather than accepting its unqualified prohibition. TODO's mid-life-retry and checkpoint
items concern different work; the queue/ledger surviving-tab-adoption item concerns terminal attribution, not this lock
contract. FILTER.md's diagnostic filters require a rare trigger and do not cover an always-inaccurate ownership comment.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g093/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is a4345e86a3f51f4c5abf3b2e97737991df900142 (A);
still present at ca9b0a19856dc136f63ad2d14f1076027edce1eb (T), core.rs:4848-4853.. This was a source-only review;
runtime occurrence and frequency were not measured.

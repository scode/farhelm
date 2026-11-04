# Stderr-readiness test can pass without establishing a live stderr holder

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The regression intended to keep terminal opens from waiting on unrelated stderr holders can report success without its
held-open-pipe premise.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
test-oracle gap; no established current production defect or observed fixture failure.

Source locations: crates/farhelm-supervisor/src/hosts/spawn.rs:785-811@1512cdb6e7db601f8b07cfe596bddf101b1a3e45;
tip:968-995.

the shell starts sleep 30 and exits 0 (:793), but the test silently accepts absent/malformed holder PID (:799-801),
never establishes that the background command successfully started or remains alive with stderr open, then asserts only
successful spawn within ten seconds (:806-811). If the background command fails or exits early, waiting for stderr EOF
would satisfy the same observations. This violates the supplied checklist's fixture-premise and competing-mechanism
requirements.

tip hosts/spawn.rs:976 starts /bin/sleep 30 asynchronously and explicitly exits the starter 0; :981 awaits startup;
:982-984 silently accepts missing/unparseable holder PID; :989-994 checks only successful return within ten seconds. The
asynchronous command's exec/early failure is not propagated by the explicit exit 0. The test neither asserts a
successfully running holder nor a retained pipe at its measurement boundary, so a failed/early-exited holder permits
both exit-status readiness and incorrect EOF-waiting implementations to satisfy these observations. This is a
conditional false-pass route from the source, not a measured failure of sleep on supported machines.

Possible coverage and caveats: no exact acceptance in the initial pinned searches of SPEC.md, SPEC_impl.md, empty TODO
Planned, unlanded plans/queue, BUGS.md, queue INDEX/matching records or TRIAGE_OUTCOMES.md. Supplied test-authoring
checklist requires the fixture premise and an observable distinguishing mechanisms. SPEC_impl.md:6795-6803 was re-read:
same-run PID signaling acceptance addresses original PID versus reused PID for cleanup (f4), not whether a child held
stderr during this test. Keep f2 separate from f4. FILTER rare-diagnostic acceptance is not a reason to turn an
unchecked fixture failure into a valid success witness; filter expressly excludes success reported for a failed
operation. Fix direction: a controlled stderr holder with a readiness witness and bounded cleanup.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g019/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 1512cdb6e7db601f8b07cfe596bddf101b1a3e45;
mechanism rechecked at pinned tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime
occurrence and frequency were not measured.

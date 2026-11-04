# Rejected-checkpoint assertion runs before rejection is processed

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The checkpoint regression can pass if the supervisor wrongly records a rejected checkpoint as accepted.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static
false-pass route, not a measured flake; no reproduction performed.

Source locations: crates/farhelm-supervisor/src/hosts/terminal/tests.rs:869-878.

snapshot_request (tests.rs:797-814) sends SnapshotRejected and returns; the None assertion at 869-878 has no
runtime-processing fence. The test attribute defaults to CurrentThread (farhelm-testtrace-macros/src/lib.rs:247-259;
farhelm-testtrace/src/lib.rs:1128-1142). Server::send uses write_all (tests.rs:137-143). Pinned tokio 1.53.2
(Cargo.lock:6276-6280) WriteAll::poll returns Ready after available writes (registry src/io/util/write_all.rs:34-54);
UnixStream forwards to poll_evented's direct socket write (src/net/unix/stream.rs:1084-1090;
src/io/poll_evented.rs:229-269). Therefore the rejection write can return and the assertion can read old None in the
same current-thread test-task poll, before the separate runtime resumes. HostConnection::snapshot waits for its own
decoded reply (hosts/client.rs:887-912). The subsequent STATUS_ANSWER (tests.rs:880-884) fences processing, but no
checkpoint assertion follows before the accepted snapshot replaces it (885-897). Production rejection handling is
correct (hosts/terminal.rs:2593-2609); no shipped recovery defect is claimed.

snapshot_request at tests.rs:797-814 only sends SnapshotRejected and returns;
checkpoints_are_taken_at_clean_reads_once_due immediately asserts terminal.status().checkpoint == None at 878, without
waiting for the runtime to consume that reply. The test later receives STATUS_ANSWER at 880-884, which provides a
processing fence, but never checks checkpoint again before the later accepted snapshot overwrites it at 885-897.

Possible coverage and caveats: none. Coverage at pinned tip: full SPEC_impl.md Recovery and checkpoints bullet 1901-1972
requires retaining the previous accepted snapshot (1940-1941), not this test gap; SPEC.md Terminal experience 2125-2165
does not accept it. TODO.md Planned 23-24 is empty. Targeted searches of SPEC.md, SPEC_impl.md, TODO.md, BUGS.md,
plans/queue, queue INDEX and queue files, and TRIAGE_OUTCOMES.md found no matching trigger/consequence/scope. FILTER.md
has no full match: ordinary ready-socket scheduling and regression false pass are outside its rare transient/diagnostic
boundaries. Cross-group synthesis: g137/f1 is a different, withdrawn notice-subscription concern; its current-thread
reasoning is consistent with this finding, because synchronous subscription precedes replay-task polling whereas this
assertion precedes rejection-task polling. g137/f2 concerns fragmented real-host records; g139/f1 concerns an
unobservable Shutdown; neither is this assertion/fence defect.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g134/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 0d3cf3c5b4648ea9d07f9d1ed5057aff586f56d4.. This
was a source-only review; runtime occurrence and frequency were not measured.

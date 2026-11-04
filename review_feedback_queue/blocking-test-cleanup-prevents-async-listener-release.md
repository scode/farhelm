# Blocking directory cleanup waits on an aborted stand-in task that cannot run

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A normal successful supervisor unit test waits for the five-second host-cleanup deadline and retains its state directory
because directory teardown blocks the same executor needed to release its aborted stand-in listener.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source
lifecycle defect for the named normal-success current-thread caller, without runtime reproduction or an
observed/measured test duration.

Source locations: crates/farhelm-teststate/src/lib.rs:741-765 (ca9b0a19856dc136f63ad2d14f1076027edce1eb).

A normal successful supervisor unit test waits for the five-second host-cleanup deadline and retains its state directory
because directory teardown blocks the same executor needed to release its aborted stand-in listener. Exact
normal-success route: crates/farhelm-supervisor/src/service/core/tests/host_terminals.rs:1178-1205 creates StateDir
before other = start_test_host_at(...), proves that other still lives after the refusal, and returns without an async
shutdown/join. The stand-in places its UnixListener inside tokio::spawn (hosts/terminal/test_host.rs:395-439), while its
Drop at 101-105 only requests task.abort and synchronously kills/reaps its child (123-128). The default
farhelm_testtrace runtime is current-thread: macros src/lib.rs:28-38,184-188,247-259; support src/lib.rs:1126-1142.
Reverse local destruction therefore requests cancellation, drops the supervisor, then runs StateDir's synchronous tmux
cleanup and TestDir Drop without another executor turn (core.rs:19726-19740,19758-19786). The listener is still bound in
the not-yet-cancelled future. lib.rs:741-765 calls synchronous hosts::stop_hosts_within; hosts.rs:107-149,319-372
connects, writes shutdown and waits for a response until HOST_STOP_DEADLINE (five seconds). The listener cannot process
Shutdown or be dropped by cancellation on the blocked current-thread executor, so the socket and directory are retained
as unconfirmed. Runtime resumption/shutdown can then drop the listener, but no second TestDir cleanup runs; the kept
directory remains for a later process's sweep. Exact dependency check: Cargo.lock:6277-6280 pins Tokio 1.53.2, source
commit ff0c4068a4ba5f3af0fe3c5657ba42879b6ea599. Its task/join.rs:227-228 calls remote_abort; task/harness.rs:111-127
schedules cancellation; task/state.rs:303-333 sets cancelled/notified; task/harness.rs:193-228,500-506 drops the future
only when cancellation is polled. This resolves the cancellation-lifetime premise with source, not a hypothetical
measurement. Fix direction: make the stand-in's owner explicitly abort-and-await shutdown and unlink its socket before
StateDir drops, or provide a synchronously closable listener owner. An async yield alone does not remove the stale
socket file and still pays the refusal window; close/unlink should be explicit.

Possible coverage and caveats: none exact. Searched SPEC.md, SPEC_impl.md, TODO.md Planned (empty at tip:23-25),
unlanded plans/queue, BUGS.md, queue INDEX/files and TRIAGE_OUTCOMES.md. The full matching SPEC_impl.md:2504-2515
requires host cleanup and retention for uncertain hosts; it does not accept an in-process stand-in preventing its own
normal teardown. TRIAGE_OUTCOMES.md:11976-11986 discards a different shutdown-race test oracle requiring a production
seam, not this test-helper cleanup fix. review_feedback_queue/container-test-panic-cleanup-precedes-late-creator.md:1-59
is a different failure/late-allocation concern. signal-test-child-lacks-unwind-cleanup.md:1-90 concerns failure before
process ownership; here ownership exists and kills/reaps the child, but the owned listener future has not finished
cancellation on normal return. FILTER rare-safe-failure does not match this ordinary normal-success trigger; the
hung-private-tmux filter does not cover this async stand-in lifecycle. A focused search of current sibling finding logs
found no same aborted-stand-in/current-thread/TestDir issue; parent-listed detached Python fixture and
child-before-owner findings remain separate.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g030/f4`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb. The
assigned cleanup implementation was added by 1512cdb6e7db601f8b07cfe596bddf101b1a3e45 and persists at tip; the concrete
integration caller is reviewed at tip.. This was a source-only review; runtime occurrence and frequency were not
measured.

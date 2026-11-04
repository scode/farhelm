# Late listener binding can leave the unfinished-start test blocked until nextest termination

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A delayed test listener can turn the cleanup regression into a five-minute runner timeout instead of a bounded fixture
failure.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source schedule; scheduling occurrence is not measured and no latent flake is claimed.

Source locations: crates/farhelm-supervisor/src/hosts/spawn.rs:561-584,907-914@1512cdb6e7db601f8b07cfe596bddf101b1a3e45;
tip:743-767,1090-1097.

listen_at_recorded polls for a recorded path with a ten-second deadline (:561-574), binds (:575), then blocks on accept
without a deadline (:582); the read timeout (:583) applies only after accept. The test gives spawn_host_within two
seconds (:913) and then joins (:914). If the stand-in records its path but the listener is not scheduled to bind until
after the start deadline, the starter is killed and cleanup returns Abandoned because no socket exists (:389-393). The
listener can then bind and wait for a connection that will never arrive, blocking join. .config/nextest.toml:18-21 at
pinned tip imposes an outer five-minute termination boundary; the test itself cannot finish or cleanly report this
failure.

A delayed test listener can turn the cleanup regression into a five-minute runner timeout instead of a bounded fixture
failure. Exact source schedule: tip spawn.rs:1095 starts the listener thread, :1096 gives startup two seconds, and :1097
joins it synchronously. The listener can see the recorded path yet be delayed before binding at :758. If startup's
timeout kills/reaps the first stage while that path is still absent, end_unreported_host returns immediately at
:572-576. When the listener resumes, accept at :765 has no timeout and no future client; :766 bounds only reads after
acceptance. There is no internal return path from this schedule. Full .config/nextest.toml:1-73 was checked: :20
configures period 60 seconds, terminate-after 5 (300 seconds), grace-period 5 seconds; none of the later overrides
changes this test's timeout. This corrects any reading of the earlier report as an unbounded entire nextest command: it
is an internally unbounded fixture/async-thread join with outer termination at the configured 300-second boundary and
five-second grace.

Possible coverage and caveats: FILTER.md's full 'Rare, self-correcting glitches and imprecise diagnostics' section was
re-read. A narrow-schedule test timeout followed by a successful retry may count as an operation that fails safely, but
whether that filter intends to cover a fixture that cannot itself finish and requires runner process termination is not
explicit. I retain that triage ambiguity rather than unilaterally excluding the finding or claiming the filter forbids
it. The hung-tmux/systemd filter does not match this UnixListener dependency. Initial coverage searches of SPECs, empty
Planned, plans/queue, BUGS, queue and ledger found no exact alternative coverage. Fix direction: bound listener
acceptance and coordinate readiness while preserving the intentionally unfinished first stage.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g019/f3`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 1512cdb6e7db601f8b07cfe596bddf101b1a3e45;
persistence rechecked at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime occurrence
and frequency were not measured.

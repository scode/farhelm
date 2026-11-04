# Mixed-session test treats a missing sink PID as completed cleanup

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The migration regression cannot detect incomplete old-output-client reaping through the predicate it calls its reap
check.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed test-oracle
defect; no production security, data-loss or agent-stop bug claimed.

Source locations: fba73468a9cb09c217a39da2be7f2c7cea064cb8:crates/farhelm/tests/e2e/host_migrations.rs:1298-1309.

host_migrations.rs:1298-1309 treats session_sink_pid returning None as proof the old tmux output client was reaped and
everything triggered by its end ran. At this immutable commit, service/terminals.rs:2898-2905 returns None for every
Reaping entry. SessionSinkLease::drop publishes Reaping at 1518-1522 before spawning and awaiting shutdown at 1534-1537;
the sink task also clears the PID before actual shutdown at 1627-1633 and 1647-1664. core.rs:15980-15995 drops the
backend and then notifies Detached without waiting for its sink lease shutdown, so successful close_tab and
expect_detached do not establish that boundary either. The loop can therefore take zero iterations and its diagnostic
about a client never being reaped is not supported by its predicate. Capture the owned sink PID before close and await
its exit, or expose/use the already-owned completion barrier; establish any separate callback boundary independently.

The assigned patch polls session_sink_pid until None, then claims everything triggered by the tmux session ending has
run. At this commit, service/terminals.rs:2898-2905 returns None for a Reaping registry entry. SessionSinkLease::drop
publishes Reaping at 1518-1522 before spawning and awaiting shutdown at 1534-1537. The sink loop also clears the PID
before shutdown at 1627-1633 and 1647-1664. None is therefore not the named reaped/completed boundary.

The migration regression cannot detect incomplete old-output-client reaping through the predicate it calls its reap
check. Exact trigger: the test closes its last tmux tab with a live attachment (1285-1293), then polls session_sink_pid
for absence (1298-1309). At tip, service/core.rs:16086-16095 joins the per-terminal forwarder, then drops the backend
and announces Detached. Backend drop releases the last sink lease. service/terminals.rs:1574-1593 replaces the Live
entry with Reaping before spawning and awaiting owned.shutdown. session_sink_pid at 2954-2960 returns None for Reaping,
so the test can exit this loop before that shutdown future has completed, including when it is still pending. The loop
task itself clears its PID before awaiting shutdown at 1682-1689 and 1703-1719. This is a source-established false
completion oracle, not a measured flake or an assertion that pending sink cleanup changes the agent outcome. The joined
forwarder and pending sink shutdown are distinct lifetimes. Suggested repair: retain the already available owned sink
PID before close and await that exact process exit, or await an actual sink-reap completion signal; keep the
agent-survival assertions separate.

Possible coverage and caveats: TRIAGE_OUTCOMES.md:11976-11986 discarded a different shutdown-race test because
deterministic repair needed a production hook. Capturing the existing PID here needs no such hook; trigger and tested
boundary differ. Initial full coverage-section reads and a pinned-tip repeat search across SPEC.md, SPEC_impl.md, TODO
Planned (empty), BUGS.md, queue INDEX/files, plans/queue and TRIAGE_OUTCOMES.md found no same trigger/consequence/scope
acceptance. SPEC_impl.md:2195-2265 specifies mixed-session independence, not an exception for false test completion
predicates. Sink-reaper residual decisions at TRIAGE_OUTCOMES.md:4645-4673 and 11547-11562 concern production failure
paths rather than this test oracle. No FILTER.md rule covers the ordinary coverage gap.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g192/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is fba73468a9cb09c217a39da2be7f2c7cea064cb8.. This
was a source-only review; runtime occurrence and frequency were not measured.

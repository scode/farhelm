# Reserved-host preservation test can mistake its temporary starter for the host

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The reserved-session preservation regression can blame correct startup cleanup for a fixture launcher exiting normally,
because its readiness check can save that temporary launcher as the terminal-host process.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source-level
ambiguity and compatible conditional failure path; no measured frequency, observed flake or product defect is claimed.

Source locations: 81234061e87b7599c157b7b855a468c9c4b2414e:crates/farhelm/tests/e2e/host_reclaim.rs:898-904,964-980.

host_reclaim.rs:864 releases the gate before the interrupted create, :898-904 accepts the first PID returned by
host_process and saves it as reserved_pid; :964-980 later requires that PID still alive. At the reviewed commit
host_terminals.rs:119-134 matches any process with adjacent --socket and the recorded path in its command line, without
distinguishing the first stage from the daemon. host_creates.rs:1109-1129 wraps that same invocation in a shell and
execs the native first stage with the original arguments. hosts/spawn.rs:350-352 supplies those arguments and :486-503
gives this stage the spec without waiting for its exit. terminal-host/src/spawn.rs:50-122 shows that the first stage
stays alive until its own forked host reports readiness and then returns normally. There is no fixture ordering forcing
that first stage to have finished before :898-904; the second keeper launch is a different socket and waits only for
itself. An ordinary unscheduled first stage or its readiness-wait interval therefore satisfies the supposed host-running
premise. Once it exits, the later survival assertion fails despite the actual reserved host surviving. These helper
paths were checked at tip too: host_terminals.rs:119-134, hosts/spawn.rs:349-379,486-503,
terminal-host/src/spawn.rs:50-122.

The reserved-session preservation regression can blame correct startup cleanup for a fixture launcher exiting normally,
because its readiness check can save that temporary launcher as the terminal-host process. Exact

Possible coverage and caveats: No exact accepted behavior, Planned task, unlanded-plan fix, BUGS entry, queue item or
ledger decision found in searches of all required coverage sources. TODO Planned is empty (:23-25); BUGS.md and queue
INDEX were read in full. SPEC_impl.md at tip Starting a host (:1763-1774) distinguishes readiness from the first stages
mere existence, and launch-record rules (:2044-2077) recognize that a recorded stage can still be starting. FILTER.md
Rare, self-correcting glitches could be interpreted broadly to include retryable test failures, but it does not clearly
accept a fixture selecting the wrong process; retain under the unclear-match rule.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g188/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 81234061e87b7599c157b7b855a468c9c4b2414e;
persists at ca9b0a19856dc136f63ad2d14f1076027edce1eb, with no changes to host_reclaim.rs and no changes to the gated
helper (the intervening host_creates.rs diff only adds two earlier tests).. This was a source-only review; runtime
occurrence and frequency were not measured.

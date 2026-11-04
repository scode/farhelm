# Last-output regression restarts before its exit-output trap is established

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The final-output regression can reject correct output preservation because its agent has published readiness before
installing the exit-output trap.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source
readiness-ordering defect, conditional on scheduling; no observed or measured flake claimed. Tip persistence and causal
path: host_relaunches.rs:59-74 prints OLD_SCREEN_MARK before the TERM/HUP trap command; :101-127 returns after creation
and observing that marker/modes/title. The test at :1611-1620 immediately restarts, then requires last words 01.
Additional static checks resolved hidden readiness and stop premises: harness.rs:1250-1278 creates the command session
and records a synthetic conversation, with no fixture script acknowledgment. host_creates.rs:37-45 disables systemd
scopes. Restart core.rs:13019-13046 calls stop_live_agent before seed acquisition (:13075-13088); sweep.rs:1947-1995
calls reap_process_tree, :1359-1476 always reaches the process-tree mechanism even without scopes, and :1111-1139 sends
SIGTERM before its grace. There is no stop-side handshake requiring the shell to install its trap first. A fixture shell
descheduled after emitting the marker can therefore receive the terminating signal before executing trap; the test's
expected output was never promised by its observed premise. Install the trap before the readiness marker, or await a
dedicated trap-installed oracle.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm/tests/e2e/host_relaunches.rs:1611-1620.

the_seed_can_hold_what_the_old_program_printed_as_it_stopped calls history_session_with(..., true), then immediately
Restart. history_agent_with at lines 59-74 emits OLD_SCREEN_MARK before expanding wait, whose first command installs the
TERM/HUP trap that prints the last words. history_session_with at lines 110-126 waits only for that screen marker and
terminal modes/title, all emitted before the trap. If the fixture shell is descheduled after printing the marker,
Restart can stop it before the trap exists, and history will contain no last words even with correct production
draining. Install the trap before the observed readiness marker, or publish and await an explicit trap-installed oracle
without removing the intended stop/output boundary.

Possible coverage and caveats: none exact found in initial and repeated targeted searches of SPEC.md, SPEC_impl.md,
Planned TODO, plans/queue, BUGS.md, queue INDEX/files and TRIAGE_OUTCOMES.md. SPEC_impl.md:2120-2194 was read in full
during synthesis; :2135-2144 accepts production settle/encoding fallbacks, not missing fixture trap readiness. The test
itself :1596-1606 disclaims proving that the drain wait caused retention. g183/f1 and g131/f1 concern those production
fallbacks and remain separate; this finding asserts no product output loss and supplies no independent exclusion verdict
for theirs. The rare-operation-failure filter does not clearly settle false test regressions; retain that limited
coverage ambiguity rather than silently applying it.

Additional reviewed locations: g185/f1:
32e20f84647951a8ea717b3c712a037c231d92dd:crates/farhelm/tests/e2e/host_relaunches.rs:59-74,110-126; tip
ca9b0a19856dc136f63ad2d14f1076027edce1eb:same ranges.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g186/f2`, `g185/f1`.
Original append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs.
Locations refer to the named reviewed revisions; the original revision context is
ca9b0a19856dc136f63ad2d14f1076027edce1eb; introduced in assigned 32e20f84647951a8ea717b3c712a037c231d92dd (original test
lines 1607-1621).. This was a source-only review; runtime occurrence and frequency were not measured.

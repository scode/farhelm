# Outage recovery test can credit an ordinary probe

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The outage regression's observed probe may come from normal polling rather than the recovery nudge it claims to test.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The inspected probe owner and counter do not distinguish the competing producers at the assertion boundary. Actual
renderer/poll timing and a regression escaping the oracle remain unobserved. This is an investigated test-attribution
concern, not an observed product recovery failure.

Source locations: `e2e/tests/terminal-multihost.spec.ts:2985-3015`. Historical revision qualifiers in those references
are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Separate discovery pass 2. Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. TLDR: the browser test can pass
without the immediate host-recovery wakeup it claims to verify if enough of its ordinary reconnect wait elapses during
UI checks and supervisor restoration. This is a test-evidence gap, not an established production reconnect defect. Test
sets a 30000 ms background probe at 2950 under a 180000 ms total budget (2948), observes probing at 2985, then performs
presentation/density/focus/screenshot work through 3011 and awaits restoreRemoteSupervisor at 3012. The helper's
host-connected poll permits 90000 ms (735-781). Only afterward does it impose 10000 ms recovery allowances (3013-3015).
Observing the probing phase does not pin remaining time. Source mechanism: reconnectStep selects the ordinary probe
interval at assets/terminal.js:1364-1368; scheduleReconnect independently arms its setTimeout at 1520-1535; host
recovery separately clears that timer and calls runReconnect at 2915-2923. Removing only that nudge still leaves the
ordinary timer. Compatible conditional schedule: the last failed attempt arms the probe; UI/screenshot/restoration work
consumes about 23 seconds while the browser timer remains live; the host is back; the ordinary probe at 30 seconds
finishes inside the test's later 10-second reveal allowance. The same replay/snapshot/focus/input assertions then pass
for the competing mechanism. No absolute deadline, ordinary-attempt counter or trigger witness excludes this schedule.
Suggested fix: place the ordinary probe beyond the whole bounded test budget, or observe/control its original deadline
and identify the recovery-triggered attempt. Suggested bucket other, private test quality. Confidence: confirmed
conditional source mechanism, actual occurrence unmeasured; retained as unverified after investigation. Missing
observation: an actual successful test ordering with enough of the current probe interval consumed before host
restoration and recovery by the scheduled timer instead of the nudge. Source checks establish a compatible route, not
its frequency; a retained attempt/timer trace or authorized missing-nudge mutation run with controlled delays would
settle occurrence. Per task no runtime, test or experiment was performed. Actual checks: read the complete test and
restoration helper, test override producer, both reconnect scheduling branches and the neighboring focus-recovery test;
searched JS/browser tests and all prescribed coverage sources for another exact nudge oracle or accepted weakness. No
exact cover found. SPEC_impl.md:672-680 requires the availability nudge; it does not accept weak test evidence.
TRIAGE_OUTCOMES.md:10131-10147 covers another test's deadline-reset behavior during tab switching, with a different
trigger and regression, not this outage/nudge seam. Planned is empty; no exact BUGS, queue item or unlanded repair
found. No filter's complete trigger/consequence match established. proposed_drop none; possible_cover none exact.
Provenance: g053 second separate discovery, original f4 event appended immediately before additional investigation.
Inherited at exclusive base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c:e2e/tests/terminal-multihost.spec.ts:2940-3018,
with the same 30-second setting and relative allowances. Not introduced by casing patch
b91fffda992b0dd586b0a90564b20bac89482db2.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g053/f4`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.

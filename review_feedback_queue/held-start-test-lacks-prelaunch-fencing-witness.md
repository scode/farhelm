# Held-start fencing regression observes completion but not pre-launch ordering

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The regression can pass when the next agent launches before an earlier outstanding first stage is fenced, so it does not
protect the before-launch ordering its assertion claims.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed
source-observability gap; no current production ordering defect, runtime mutation, overlap occurrence or frequency
claimed.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm/tests/e2e/host_relaunches.rs:1219-1236.

a_restart_fences_an_earlier_restarts_held_start_first keeps the earlier stage behind a closed gate, awaits the complete
second Restart at 1219-1222, then checks that stage is gone at 1223-1226 and only opens its gate at 1227. A broken
implementation that launches the replacement first and then kills the held earlier stage before returning would satisfy
all these observations and the agent-count assertion at 1236, while violating the stated before-launch order.

at tip :1219-1222 the entire second Restart finishes before :1223-1226 checks the earlier starter is gone; the test
opens its old gate only at :1227, after that check. held_host_program at :1105-1111 blocks only the first armed stage by
creating the held directory; later starts skip that gate. It does not inspect the earlier stage during the new launch.
Thus launch-new, end-held-old, return produces the same asserted process counts, agent count, new screen and
dead-starter result as the required end-held-old, launch-new, return order. No existing assertion in this test
distinguishes them. Tip production currently orders them correctly: service/core.rs:12998-13001 awaits
settle_outstanding_launches before the stop/claim/relaunch; :13354-13372 eventually invokes relaunch_into_terminal.
host_restart.rs:948-969 documents the safety boundary and :1029-1032 awaits settling each earlier first stage. Fix by
adding an observation at the next host-start boundary that the earlier starter is already gone, using a wrapper
assertion or precise seam while preserving the held-start premise.

Possible coverage and caveats: none exact found. Required searches covered SPEC.md, SPEC_impl.md, TODO.md Planned (empty
:23-25), BUGS.md, unlanded plans/queue, queue INDEX/files and TRIAGE_OUTCOMES.md for held-start/fencing/order/settle
identifiers. SPEC_impl.md:2120-2194 (full section read previously) specifically places outstanding-start settling before
restart work at :2123-2129. SPEC.md:1512-1556 gives the general cleanup contract and exact-crash exception; that
exception does not establish this test's asserted ordering or accept its oracle gap.
review_feedback_queue/late-output-test-does-not-distinguish-flush-order.md was read in full: it concerns emulator
flush-before-parse, a distinct implementation and fixture. Parent global map and g185's current four finding events
contain no duplicate of this order oracle; g185/f1 corroborates g186/f2 and g185/f2 adds a cleanup caller for g186/f1.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g186/f3`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb;
introduced by assigned 32e20f84647951a8ea717b3c712a037c231d92dd at host_relaunches.rs:1218-1235, with test contract at
:1131-1147.. This was a source-only review; runtime occurrence and frequency were not measured.

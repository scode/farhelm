# Host-backed teardown removes the owner-exit test's exit barrier

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: the test promising Resume survives the owner and reporter exiting can assert Resume before their
exit has been established, weakening its named oracle. Changed source: host-backed OwnedRuntime registrations carry
child: None (core.rs:22811-22814); teardown sends the group signal and waits only for Some(child) (22545-22551).
kill_owner drains these guards and returns (23048-23052). Before this commit, teardown unconditionally called
Child::wait (parent revision core.rs:24083-24088), so it normally established at least the direct runtime's exit before
returning. The new path removes that existing owner-exit barrier. The old test did not independently wait for every
grandchild either; do not attribute that pre-existing reporter limitation wholly to this patch. Caller:
omp_accepted_capture_survives_owner_exit_and_reload explicitly promises 'after the runtime and reporter are gone'
(24026-24029), yet calls kill_owner and immediately asserts offer (24056-24061), then drops/reopens the supervisor and
asserts again (24062-24080), with no identity-checked gone/zombie premise. Checks of possible implicit barriers:
OmpAdmission::offer (23228-23237) calls session_snapshot; session_snapshot (7448-7477) reads the stored row and derives
restart_offer solely from launch/capture/ownership version, without consulting terminal liveness. new_with_exe reaches
reload_sessions_with_hosts (5945-5961,6168-6184); host attribution adopts the tagged host without requiring its child
exited (core/host_handover.rs:764-790,818-822); HostTerminal::adopt_connected explicitly accepts both Child::Running and

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g114/c4`, `g115/l1`.

### g114/c4

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. Synthesis revision: confirms no implicit barrier in the offer
or reload callers. This finding supersedes the initial c4 dismissal; durable persistence correctness does not establish
the test's promised post-exit premise. TLDR: the test promising Resume survives the owner and reporter exiting can
assert Resume before their exit has been established, weakening its named oracle. Changed source: host-backed
OwnedRuntime registrations carry child: None (core.rs:22811-22814); teardown sends the group signal and waits only for
Some(child) (22545-22551). kill_owner drains these guards and returns (23048-23052). Before this commit, teardown
unconditionally called Child::wait (parent revision core.rs:24083-24088), so it normally established at least the direct
runtime's exit before returning. The new path removes that existing owner-exit barrier. The old test did not
independently wait for every grandchild either; do not attribute that pre-existing reporter limitation wholly to this
patch. Caller: omp_accepted_capture_survives_owner_exit_and_reload explicitly promises 'after the runtime and reporter
are gone' (24026-24029), yet calls kill_owner and immediately asserts offer (24056-24061), then drops/reopens the
supervisor and asserts again (24062-24080), with no identity-checked gone/zombie premise. Checks of possible implicit
barriers: OmpAdmission::offer (23228-23237) calls session_snapshot; session_snapshot (7448-7477) reads the stored row
and derives restart_offer solely from launch/capture/ownership version, without consulting terminal liveness.
new_with_exe reaches reload_sessions_with_hosts (5945-5961,6168-6184); host attribution adopts the tagged host without
requiring its child exited (core/host_handover.rs:764-790,818-822); HostTerminal::adopt_connected explicitly accepts
both Child::Running and Child::Reaped (hosts/terminal.rs:1778-1786), establishes a replay fence rather than an exit
fence (1792-1821), spawns its runtime and returns (1926-1948). Reload reads the resulting status once, rather than
waiting for an exit (core.rs:6691-6706). Therefore neither assertion's call chain supplies the missing owner-exit
precondition. SIGKILL delivery alone is not an observed exit. Contract: .agents/test-authoring.md requires 'Assert the
fixture premise before the action or measurement that depends on it' and 'Assert an observable that distinguishes the
competing mechanisms.' An already established live-owner Resume offer can satisfy the first assertion without testing
preservation after exit; the test's name and specification commit to that boundary. Fix: retain the pinned
runtime/reporter identities and await their gone/reused/zombie boundary before measuring the post-exit offer or
reopening, with timeout diagnostics inside the owned fixture's lifetime. Suggested bucket: other (test coverage
regression). Could-be-highest: no independent security or loss-of-work consequence established here; f1 separately
retains the strongest unsafe group-identity consequence. Confidence: confirmed by source and caller analysis; no
execution or reproduction. proposed_drop: false; initial c4 proposed drop retracted. possible_cover: caller reports
g115/l1 independently identifies this missing barrier, so reconcile as the same underlying test-premise finding. No
queue/coverage sources searched. Open checks for this finding: none.

Returned proposal (claims-073, event 10): **retain-verified**, suggested bucket **other**.

The test no longer establishes the owner-exit premise before asserting Resume. The initial dismissal is explicitly
superseded: correct durable storage does not show that this named test boundary ran. This is separate from the unsafe
PGID consequence in g114/f1.

Original stronger consequence: The promised post-owner-exit Resume oracle can instead observe the already available
live-owner offer. The initial proposed dismissal was retracted; no independent security or loss-of-work consequence is
established for this test gap, while g114/f1 retains that separate highest consequence.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the source-confirmed regression in the test's owner-exit precondition, without inventing a
production persistence defect.

Existing parent disposition (event 10): verified. Parent explicitly accepted named retain-verified proposal from
claims-073 after inspecting it. The test no longer establishes the owner-exit premise before asserting Resume. The
initial dismissal is explicitly superseded: correct durable storage does not show that this named test boundary ran.
This is separate from the unsafe PGID consequence in g114/f1. Retain the source-confirmed regression in the test's
owner-exit precondition, without inventing a production persistence defect. Checked source/results: ["At
1ed482dde220452e424973faf38a5e1318986333, core.rs:22811-22814 registers child=None; :22545-22551 sends SIGKILL and waits
only for Some(child); :23048-23052 drains cleanup synchronously without observing exit. Parent core.rs:24083-24088
unconditionally waits for its direct child, so an existing owner-exit barrier was removed. The old test also did not
wait for every grandchild; preserve that pre-existing reporter limitation.","The test promises runtime/reporter gone at
:24026-24029, calls kill_owner at :24056 and immediately asserts offer at :24057-24061, then reopens and asserts again
at :24062-24080. No gone/zombie/identity premise appears between signal and measurement.","offer at :23228-23237 calls
session_snapshot, which :7448-7477 derives solely from durable launch/capture/ownership fields. It does not wait for
liveness. host_handover.rs:764-790,818-822 adopts a matching host; hosts/terminal.rs:1778-1786 accepts Running and
Reaped, and :1926-1948 returns after spawning the runtime. Reopening supplies no required child-exit barrier.","Pinned
tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db core.rs:22489-22495,24000-24024 retains the same gap. No actual false pass,
test failure or runtime observation is claimed."] Current coverage/results: ["SPEC.md Durability and resume describes
persistence, not permission to omit a promised test premise. SPEC_impl.md Terminal host distinguishes live child
identity from processed exit; neither implicitly waits for fixture owner exit. .agents/test-authoring.md explicitly
requires the premise and a distinguishing observable.","TODO.md Planned empty. Exact test, fixture, owner-exit and
barrier searches in unlanded plans, BUGS.md, INDEX.md and active TRIAGE_OUTCOMES.md found no matching cover. Existing
unrelated missing-readiness items have different fixtures/boundaries. FILTER.md supplies no complete filter for an
ordinary missing test premise.","The history reports possible same-root g115/l1. It is outside the named batch and was
not read; parent should reconcile via the underlying key."] Exact cited cover: [] Original stronger consequence and
missing premise remain preserved: "The promised post-owner-exit Resume oracle can instead observe the already available
live-owner offer. The initial proposed dismissal was retracted; no independent security or loss-of-work consequence is
established for this test gap, while g114/f1 retains that separate highest consequence."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g114 record](records/review-through-00de0b6c/groups/g114.json). Record ID: `c4`.

### g115/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: the test explicitly intended to protect the Resume
offer after an OMP agent exits can pass without proving that the agent exited. The durable-write/reopen assertions still
provide persistence evidence; this finding concerns the additional after-owner-exit premise, not a claim that OMP
capture actually fails or that the test proves nothing. Source: spawn_one (22931–22968) now uses start_in_host; its
live_root at 22806–22810 returns a pinned root but the fixture retains only its pid as OwnedRuntime.group, with child
None (22811–22814). The reporter identity is retained by the test as peer (22966–22968, 24048–24054), but the test does
not use it to confirm exit. teardown at 22545–22551 sends group SIGKILL and returns without a wait for child None;
kill_process_group at 22582–22587 also ignores the syscall result. kill_owner at 23048–23052 has no later barrier. The
first offer assertion follows directly at 24056–24060. session_snapshot at 7448–7478 reads only the durable row and
computes the offer, so the competing stored-capture mechanism explains why the test passes while the owner is still
alive; it does not establish that the owner is gone. Reopening at 24068 proves durable reconstruction but never asserts
the runtime/reporter's identities are gone or zombie. The test's stated contract at 24026–24029 explicitly says after
the runtime and reporter are gone. The former direct-child path called Child::wait before returning, establishing at
least runtime exit; the migration removes that synchronization. Successful signal delivery is not an exit-readiness
oracle, and even a failed cleanup can go unobserved by these persistence assertions. This is not an assertion that a
kill failure occurred in execution. Test-review contract: 'Assert the fixture premise before the action or measurement
that depends on it. A successful setup command alone does not establish the state the assertion needs.' Fix direction:
retain the runtime root's start identity before signaling (start_in_host currently discards it), retain the existing
reporter identity, establish both live before teardown, and await wait_pid_gone at 22634–22650 for both before asserting
the offer. Waiting afterward fixes the oracle only; it does NOT fix the distinct unsafe group-signal lifetime issue
supplied by parent as g114/f1. That separate interface finding says a host can reap the child and release the numeric
group id before an eventual unconditional negative-PID SIGKILL reaches an unrelated reused group. Preserve g114/f1
independently; no deduplication with l1 is proposed. Checked subsequent core.rs changes through pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db: only f560058da19e02716e6f943883a9508b447aff4e and
08399d092d20815ced6619c26df2391944ae2b4a touch this file, and the full scoped diff changes terminal-kind reply fields
and comments, not this fixture, teardown or test; no later repair found. Suggested bucket: other, confined to test
evidence. Could-be-highest: no established product security or loss-of-work route for l1; g114/f1 has a distinct
consequence and is not being lowered here. Confidence: confirmed by source; no runtime observation. proposed_drop:
false. possible_cover: SPEC.md First-class harnesses (5253–5266) accepts partial OMP activity/session integration and
says such gaps are not worth code review; parent should scope-match this allowance to the existing test's missing
prerequisite rather than drop automatically. No excluded coverage sources searched. Material caveat: SIGKILL normally
acts quickly, and persistence itself is tested; the missing named lifecycle boundary is the defect. This revision
reconciles g114/c4's persistence objection while preserving that valid persistence evidence. Discovery-turn interface
reconciliation before new search: the supplied latest g114/c4 now independently confirms the missing owner-exit barrier
and retracts its earlier persistence-based dismissal, agreeing it duplicates l1. Its source checks additionally
establish HostTerminal adoption/reload accepts both running and reaped children, so reopening is not an implicit exit
fence. Preserve the limited regression: former Child::wait established runtime exit, not every reporter descendant. The
pre-existing lack of an independent reporter wait is not wholly attributed to this change. Supplied g114/f1 separately
requires the entire old group/session to be empty and reaped before numeric reuse; surviving descendants prevent reuse.
This does not change l1 severity or merge that distinct safety finding.

Returned proposal (claims-074, event 12): **retain-verified**, suggested bucket **other**.

The migrated fixture sends a signal and returns without proving owner exit before a test that explicitly claims that
premise. Persistence/reopen assertions remain useful but do not prove the missing lifecycle boundary.

Original stronger consequence: The test meant to preserve Resume after an OMP agent exits can pass before owner exit. No
product capture failure or lost user work was established. SIGKILL ordinarily acts quickly, and persistence itself is
still tested. Supplied g114/f1's numeric-group reuse safety claim remains separate and is not lowered or refuted here.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record the missing owner-exit prerequisite as other. The supplied history reports g114/c4 agrees it
duplicates this cause, but c4 is not an assigned named record here, so no cross-batch alias verdict is made. A
post-signal wait repairs this oracle only, not the distinct stale numeric-group cleanup risk.

Existing parent disposition (event 12): duplicate. Parent read full claims074 and earlier claims073g114/c4. Exact same
OMP fixture replaces direct Child::wait with bare PGID signal and childNone, then immediately measures durable Resume
with no owner-exit witness. Join verified test-oracle group, preserving original owner/reportergone wording and the
limit that old test never independently joined every descendant. No observed false pass/productcapture failure. Separate
g114/f1 retained highest stale-group wrong-target signaling route is neither refuted nor lowered by this test-quality
consolidation. Current coverage/filters found no exact acceptance of the missing test premise.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g115 record](records/review-through-00de0b6c/groups/g115.json). Record ID: `l1`.

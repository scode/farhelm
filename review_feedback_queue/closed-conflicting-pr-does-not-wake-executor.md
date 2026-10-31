# Closing a conflicting PR does not wake an executor

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A waiting plan can remain unstarted after the last conflicting PR closes, until unrelated activity or a manual drain.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The picker includes open plan PRs in its conflict set, but the watcher and wake-check observe plan-tree/file states
rather than PR membership. Closing the last conflicting PR without a queue write changes availability without changing
their baseline. The executable scheduling defect predates the reviewed range. Existing bounded landing-baseline and
question-deferral records have different triggers. The operational-artifact discard's application to this maintained
executable behavior remains ambiguous.

Source locations:
`scripts/plans-watch.sh:12-23,325-344; scripts/plans-queue.py:1187-1258; plans/.agents/executing.md:19-31 at d6645f771bee8ed03a66486ef8755834937d759d`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. New discovery beyond f2 and excluded accounting/scope records;
preexisting executable scheduling defect in interfaces used by assigned block/claim/landing transitions, not attributed
to an introduction in this range. TLDR: after the maintainer closes a conflicting plan's remaining draft PR, another
plan can remain unstarted until an unrelated queue change or a manual drain, even though the conflict that made the
executor wait has gone. Established source sequence: A is pending with no dependency on B. B is blocked (or pending, not
in-flight) and has an open PR touching the same functions. Picking counts any listed plan with open plan/<slug>/* PRs as
unlanded work, regardless of state, and skips A (executing.md:19-31). The executor starts its watcher at that last-pick
baseline; watching.md:14-27 prescribes this, and :18 says an idle executor has no maximum wait. The maintainer then
closes B's last PR without changing plans/. Such standalone rejection is contemplated by executing.md:75-77, while
landing.md:55-57 explicitly recognizes PRs closed without merging as a state requiring a later decision rather than an
automatic queue transition. B can stay blocked awaiting that decision, so there is no required concurrent queue write
that removes it as a conflict. On the next poll, plans-watch.sh:325-344 sees the same plans tree and never calls
wake-check. Even another irrelevant plans edit does not guarantee recovery: plans-queue.py:1187-1258 compares queue/file
states, not the open-PR set, so A remains the same eligible item as at baseline. The last open PR closing removes B from
the picking conflict set, but neither layer observes that availability change. A fresh manual drain could pick A;
ordinary idle monitoring does not perform it. This requires no narrow timing window. Suggested correction: give
monitoring a bounded way to re-evaluate a changed open-PR conflict set, or require an observable queue event for this
supported handoff; do not use the current header's assertion that closing PRs cannot make work available as a proof.
Tests inspected: test-plans-queue.py:1083-1268 changes queue/plan/report snapshots for wakes, and :1274-1401 tests
PR-aware status separately; test-plans-watch.sh:237-350 uses tree changes/stub verdicts. These do not establish wake-up
for a PR-only conflict-set change. No tests or runtime reproduction run. Confidence: confirmed conditional source
mechanism and contract mismatch; actual occurrence/frequency unmeasured. Suggested bucket other (maintainer work
scheduling), no security or user-work-loss consequence established and not potentially highest. Coverage checked by
searches across SPEC.md, SPEC_impl.md, TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, queue/index and referenced scheduling
feedback. TODO Planned is empty; pinned plans queue is empty. No exact matching known bug, plan, queue item or ledger
decision found. landing-baseline-delays-new-block.md:15-26 concerns a newly blocked question already captured in the
monitor's fresh baseline and a bounded 110-minute delay, not an executor missing a PR-only conflict-set change.
plan-deferral-survives-replacement-question.md:15-26 concerns stale deferral marks, not PR observations.
TRIAGE_OUTCOMES.md:9084-9100 repairs mixed-revision tree/verdict caching, not this trigger. possible_cover:
TRIAGE_OUTCOMES.md:20479-20495 excludes operational records; its application to a maintained executable watcher
scheduling defect is ambiguous, as the existing scheduling queue files preserve. Parent explicitly confirms
accounting/scope records are excluded, which is not the asserted defect here. FILTER rare glitches do not exactly cover
a durable wait after an ordinary PR close. proposed_drop: none; retain coverage ambiguity for parent synthesis/triage.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g001/f3`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.

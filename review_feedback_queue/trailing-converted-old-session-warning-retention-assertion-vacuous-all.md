# Require the converted session's resolved warning to remain present

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the new test can report success when retirement discards the converted session's
previous warning, although the required behavior is to retain it as resolved. This is a test-oracle defect, not an
observed notification loss in Farhelm. Source: a_dead_old_agents_scoped_job_is_ended_and_the_session_restarts_in_a_host
plants an old warning at old_terminals.rs:305, then its check_notices closure at :344-357 filters for OLD_WARNING and
calls .all on the result. An empty result passes. Both invocations at :359 and :365 therefore accept that the newly
interrupted row's warning vanished on the first restart or disappeared on the second. Only the new ended notice has a
presence/count assertion. The other new test's .any resolved-warning assertion at :472-476 concerns the already-exited
ended row, which is explicitly not converted, so it does not distinguish discarding warnings only for converted rows.
Required contract: SPEC.md:2807 says 'every supervisor start marks any unresolved one resolved'; :2823 says 'Resolved
entries remain visible'; :2827-2828 requires notification persistence and the ten-most-recent cap. This fixture plants
one old warning and adds one ended notice, so cap eviction is not an explanation. Sensitivity checks: core.rs:7337-7382
separately commits row conversions and builds the ended list before record_ended; core/old_terminals.rs:525-550 records
an ended notice for each converted id, then resolves old warnings globally. A regression that drops an old warning only
for those converted ids would satisfy the current first test and leave the already-ended-row asser

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g157/l4`.

### g157/l4

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 16 (finding, open):

Reviewed commit: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. TLDR: the new test can report success when retirement
discards the converted session's previous warning, although the required behavior is to retain it as resolved. This is a
test-oracle defect, not an observed notification loss in Farhelm. Source:
a_dead_old_agents_scoped_job_is_ended_and_the_session_restarts_in_a_host plants an old warning at old_terminals.rs:305,
then its check_notices closure at :344-357 filters for OLD_WARNING and calls .all on the result. An empty result passes.
Both invocations at :359 and :365 therefore accept that the newly interrupted row's warning vanished on the first
restart or disappeared on the second. Only the new ended notice has a presence/count assertion. The other new test's
.any resolved-warning assertion at :472-476 concerns the already-exited ended row, which is explicitly not converted, so
it does not distinguish discarding warnings only for converted rows. Required contract: SPEC.md:2807 says 'every
supervisor start marks any unresolved one resolved'; :2823 says 'Resolved entries remain visible'; :2827-2828 requires
notification persistence and the ten-most-recent cap. This fixture plants one old warning and adds one ended notice, so
cap eviction is not an explanation. Sensitivity checks: core.rs:7337-7382 separately commits row conversions and builds
the ended list before record_ended; core/old_terminals.rs:525-550 records an ended notice for each converted id, then
resolves old warnings globally. A regression that drops an old warning only for those converted ids would satisfy the
current first test and leave the already-ended-row assertion satisfied. store.rs:11665-11792 checks generic
cap/dedup/reopen/deletion; it does not invoke this conversion-and-notice integration path. Targeted
OLD_WARNING/record_ended/OldTerminalEnded searches across these e2e and immediate notification/store tests found no
positive retained-warning assertion for a converted row. Production currently performs an UPDATE at store.rs:5814-5830;
that implementation correctness is not a replacement for a distinguishing regression assertion. Fix: in check_notices,
require exactly the planted old warning to be present and resolved (or at least a positive presence assertion alongside
all-resolved), on both supervisor starts. Suggested bucket: other (maintainer test correctness/regression protection
only). Confidence: confirmed source and assertion logic, no runtime execution or reproduction. proposed_drop: false.
Could-be-highest: no established shipped security/work-loss route; original suspected consequence remains missing
notification-retention coverage, not a production deletion allegation. possible_cover: none in the supplied neighbour
records; prohibited coverage sources not searched. Caveat: absence search is scoped to the relevant named test/caller
sources; unrelated notification unit tests may cover other retention mechanisms. Later-tip applicability is confirmed:
at 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db, old_terminals.rs:355-360 retains the same filter-plus-all assertion, called
on both starts at :363 and :369. The only later commits touching this test file are the missing-tmux skip and comment
cleanup, not an oracle repair. All introduction premises at the reviewed new-file commit are settled.

Returned proposal (claims-100, event 16): **retain-verified**, suggested bucket **other**.

The test plants an old warning, then accepts an empty filtered warning set on both supervisor starts. It therefore
cannot distinguish retention as resolved from deletion specifically for a converted session. This is a test sensitivity
defect, not observed production notification loss.

Original stronger consequence: From its first lead onward, the concern was lost regression protection for retaining the
converted session's prior warning as resolved. It never established shipped warning deletion, security harm or loss of
user-owned work. Preserve the distinction from the already-ended-row assertion and from correct production UPDATE
behavior.

Unresolved premise or limitation: No source premise is missing for the assertion gap. Runtime frequency and an actual
production warning deletion are neither established nor required for this test-only claim. Absence search is limited to
the named integration test and immediate notification/store tests.

Proposed basis: Record the test-only defect at other priority. Require a positive exact warning presence/count assertion
as well as resolved state on both starts. Preserve source-confirmed confidence and the absence of a runtime
reproduction.

Existing parent disposition (event 16): verified. Parent explicitly accepted named retain-verified proposal from
claims-100 after inspecting it. The test plants an old warning, then accepts an empty filtered warning set on both
supervisor starts. It therefore cannot distinguish retention as resolved from deletion specifically for a converted
session. This is a test sensitivity defect, not observed production notification loss. Record the test-only defect at
other priority. Require a positive exact warning presence/count assertion as well as resolved state on both starts.
Preserve source-confirmed confidence and the absence of a runtime reproduction. Checked source/results: ["At
dbfc69751215dbf15e1d31d5bed73c3754c4ebc4, old_terminals.rs:305 plants the warning; :340-343 proves the session was
converted to Interrupted; :344-357 requires one new ended notice but uses filter(OLD_WARNING).all(resolved), with no
presence check. Calls at :359 and :365 repeat that vacuous assertion.","The same file's :466-476 positively checks
retention only on the already-ended row that was not converted. A regression deleting warnings only for converted rows
would evade both tests.","At the same hash, core.rs:7337-7382 commits conversions, collects converted Interrupted
sessions and calls record_ended; core/old_terminals.rs:525-547 records ended notices then resolves old warnings.
store.rs:5814-5830 currently uses UPDATE, so current production deletion is not alleged.","store.rs:11665-11792 checks
generic notification deduplication, reopening, capping and delete cleanup; it does not drive this converted-row
integration route. Targeted OLD_WARNING/OldTerminalEnded/record_ended searches in the immediate test and notification
sources find no positive converted-row retained-warning assertion.","At inclusive tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db, old_terminals.rs:348-369 retains the same assertion and both calls. No runtime
execution or mutation experiment."] Current coverage/results: ["SPEC.md:2803-2828 requires startup resolution, visible
resolved entries, persistence across starts and the ten-entry cap. This two-notice fixture cannot justify warning
deletion by cap eviction. SPEC_impl.md:2780-2822 and :5682-5688 describe conversion and resolution; neither accepts
discarding the warning or this missing test assertion.","TODO.md's Planned bucket is empty. Focused searches across
plans/queue found tmux-end-old-terminals.md:95-100 requiring old-warning resolution and a separate persistent ended
notice; this describes intended product behavior and does not fix the new test's presence assertion. Tooling/docs tmux
plans leave the startup-shutdown tests in place and do not specify this oracle repair.","Current INDEX.md and queue-file
searches found notification-cleanup-row-premise.md. Read it fully: it concerns schema reconstruction and the premise
before DELETE, not retention across conversion/startup. It is neither exact coverage nor a same-root alias; extending it
would mix distinct defects.","BUGS.md has no matching notification issue. Searches of active TRIAGE_OUTCOMES.md for
notification, warning, old-terminal and conversion terms found no matching decision. No archive read.","FILTER.md has no
full match: this is a deterministic maintainer-test assertion gap, not a rare runtime diagnostic-loss trigger."] Exact
cited cover: [] Original stronger consequence and missing premise remain preserved: "From its first lead onward, the
concern was lost regression protection for retaining the converted session's prior warning as resolved. It never
established shipped warning deletion, security harm or loss of user-owned work. Preserve the distinction from the
already-ended-row assertion and from correct production UPDATE behavior."; "No source premise is missing for the
assertion gap. Runtime frequency and an actual production warning deletion are neither established nor required for this
test-only claim. Absence search is limited to the named integration test and immediate notification/store tests.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g157 record](records/review-through-00de0b6c/groups/g157.json). Record ID: `l4`.

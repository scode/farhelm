# Real-container prerequisites require a payload the producer no longer stages

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: preparing fresh payloads then starting the real Podman tests fails before exercising Farhelm,
because the test still requires a tmux binary the producer has stopped supplying. Old staged files can mask the
mismatch.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g067/l3`, `g070/l2`, `g093/f1`, `g147/c4`.

### g067/l3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d. TLDR: preparing fresh payloads then starting the real Podman
tests fails before exercising Farhelm, because the test still requires a tmux binary the producer has stopped supplying.
Old staged files can mask the mismatch. Details: same-revision prerequisites at lines 190-195 asserts both
farhelm-{target}.tar.gz and tmux-{target} exist. scripts/build-podman-test-payloads.sh in this commit removes the pinned
tmux build, static check, install, move into the payload directory and final tmux existence check, retaining only the
farhelm archive. A fresh script-created directory satisfies the producer contract but fails the consumer's is_file
assertion. The producer reuses its directory with mkdir -p and never removes an earlier tmux file, so old staged output
can conceal the mismatch. This violates the same-revision staging prerequisite at the consumer's lines 175-177. Exact
repair is now source-confirmed: d751ff1633bb479c7bf53920f2e4ea4ba932a1c2 replaces the two-file loop with a single
farhelm archive assertion and explains that provisioning no longer pushes static tmux. The assigned later patch
08399d092d20815ced6619c26df2391944ae2b4a merely removes that parenthetical explanation; it is not the behavioral repair.
Tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the corrected single-archive prerequisite. Preserve the defect at
500548b and its later repair rather than claiming a current-tip failure. Suggested bucket: other (maintainer
runtime-validation startup failure). Confidence: confirmed by direct intermediate source, producer patch, exact consumer
repair patch and tip source; no execution. proposed_drop: false; repaired-in-range status is explicit for parent
filtering. possible_cover: parent-supplied g093/f1 describes the same producer/consumer mismatch; retain for parent
deduplication. Could-be-highest: false. This revision settles the exact repair premise while preserving the original
intermediate consequence.

Returned proposal (claims-043, event 12): **needs-parent**, suggested bucket **other**.

The intermediate defect is source-established, and its later repair is also established. Preserve both facts; this is
not a current-tip startup failure. The available disposition vocabulary has no repaired-in-range category.

Original stronger consequence: Freshly prepared payloads cause the real Podman suite to fail before resources are
created. No product work-loss consequence was claimed.

Unresolved premise or limitation: No source premise is missing for the historical mismatch or repair. Parent must
reconcile the supplied g093/f1 producer-side identity; that record is outside this named batch and was not inspected.

Proposed basis: Keep the historical finding and explicit repair in the recording audit. Do not silently refute it from
correct tip code or describe it as still actionable. Parent decides whether repaired-in-range findings belong in its
resulting queue.

Existing parent disposition (event 12): rejected. Parent read claims043 in full. The intermediate500548b9 producer stops
staging tmux while the consumer still requires it; fresh staging fails, stale staging may hide it. This is a real
historical defect, not refuted retroactively. The later d751ff16 consumer explicitly repairs that mismatch and tip
retains the single-archive assertion. Preserve original g093/f1 and g070/l2 counterparts for final reconciliation; no
current actionable remainder from this exact pairing and no highest consequence. Historical source and repair are
recorded in the audit rather than an obsolete queue item.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g067 record](records/review-through-00de0b6c/groups/g067.json). Record ID: `l3`.

### g070/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d. TLDR: fresh same-revision Podman payloads fail the real
test's prerequisites because the producer no longer stages tmux-$target but the consumer still requires it. An existing
payload directory retaining an older tmux file masks the failure. Suggested bucket: other (test admission); confidence:
confirmed by source, no execution; proposed_drop: false; possible_cover: producer-side counterpart g093/f1.
Could-be-highest: no security or user-work-loss route.

The commit's producer diff removes build-pinned-tmux-ci.sh, both tmux install/move steps, and its final tmux existence
check from scripts/build-podman-test-payloads.sh. At the same revision podman_environments.rs:190-195 still checks both
the farhelm archive and tmux-$target before fixture creation. The new producer builds only the archive at script lines
40-49 and admits the directory at lines 59-64. This establishes the intermediate failure without a test run.

Repair verified: assigned commit d751ff1633bb479c7bf53920f2e4ea4ba932a1c2 replaces that two-file loop with archive-only
checking at consumer lines 190-197. Pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains archive-only checking at
lines 190-196. Thus this is a historical defect introduced at 500548b and repaired by d751ff, rather than a tip defect
or grounds for dropping the historical record. Candidate c3 evaluates only the repair's validity. Distinct from l1's
resource-lifetime concern. No runtime tests or reproduction.

Returned proposal (claims-045, event 9): **retain-verified**, suggested bucket **other**.

The intermediate producer/consumer mismatch is established by pinned source within the review range. It prevents
maintainer test admission on a freshly staged directory. It was repaired later in the same range; preserve that
qualification rather than presenting it as a tip defect. No security or user-work-loss route is claimed.

Original stronger consequence: Fresh same-revision payloads fail real Podman test admission before fixture creation; an
existing tmux file can mask the failure. All supplied revisions keep the impact confined to maintainer tests. No
stronger security or user-work consequence was withdrawn or lowered.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the source-confirmed historical defect with introduction and in-range repair together. Parent
should reconcile the reported g093/f1 counterpart before choosing a feedback filename; this consultation does not decide
that alias.

Existing parent disposition (event 9): duplicate. Parent read complete claims045; this is the same500548b9 intermediate
producer/consumer mismatch as g067/l3: script stages no tmux, fresh consumer still requires it before fixture creation.
Corrected prereq caller citation1600-1601 is preserved. d751ff16 repairs the consumer and00de retains it. Join canonical
historical-repaired group, preserving original defect, stale-output masking and later repair, not claiming it never
existed or persists at tip. No highest consequence; g093/f1 remains to reconcile separately.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g070 record](records/review-through-00de0b6c/groups/g070.json). Record ID: `l2`.

### g093/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d. TLDR: This intermediate revision blocks a fresh real Podman
lifecycle run before its scenario: setup stops producing tmux while the test still requires it. The later commit
d751ff1633bb479c7bf53920f2e4ea4ba932a1c2 repairs the consumer; the defect is absent at the pinned tip. Original defect:
scripts/build-podman-test-payloads.sh:35-49 builds and stages only farhelm-{target}.tar.gz, while
crates/farhelm/tests/e2e/podman_environments.rs:190-196 still checks that archive and tmux-{target}.
run_managed_acceptance calls prerequisites before fixture setup; .config/nextest.toml selects the producer for this
module. A fresh payload directory fails the test; cached tmux can mask it. Exact repair:
d751ff1633bb479c7bf53920f2e4ea4ba932a1c2 changes podman_environments.rs:190-198 from the two-file loop to let archive =
format!("farhelm-{target}.tar.gz") and a single is_file assertion. 08399d092d20815ced6619c26df2391944ae2b4a only
shortens the explanatory comment, leaving that repaired behavior. At pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:190-197 the single-file assertion remains. Suggested bucket: other (maintainer
test tooling). Confidence: confirmed by historical patches and tip source, not execution. proposed_drop: false; retain
the original defect because intermediate revisions are in scope, explicitly marked repaired. possible_cover: parent
reports g067/g070 own Podman tests and will receive this input; no queue or coverage sources consulted.
Could-be-highest: no; no catastrophic or user-data route. No premise remains unresolved for this finding.

Returned proposal (claims-061, event 9): **needs-parent**, suggested bucket **other**.

The intermediate defect is source-established and within the range, but its exact repair is also within the range.
Preserve it as resolved historical review evidence; do not describe it as a residual tip defect. Parent must decide how
resolved-in-range findings are represented rather than silently dropping the history or opening an item with no
remaining repair.

Original stronger consequence: A fresh real Podman lifecycle run cannot reach its scenario because the producer stopped
staging tmux while the consumer still asserted it. Cached tmux can mask the mismatch. The reviewer never alleged
user-data loss and explicitly preserved the in-range repair.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Confirmed historical defect, repaired by d751ff16 before the pinned tip; retain that attribution in the
review record and ask the parent to classify it as resolved-in-range.

Existing parent disposition (event 9): duplicate. Parent read full claims061 and now reconciles exact producer-side
counterpart with g067/l3 and g070/l2.500548b9 stops staging tmux while same-revision consumer190-196 requires it before
fixture creation; fresh output fails, cached output can conceal. d751ff16 explicitly repairs consumer;00de retains
archive-only admission. Preserve all original IDs, source facts and introduction/repair as historical true defect, with
no residual queue item or false claim it never occurred. No highest consequence.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g093 record](records/review-through-00de0b6c/groups/g093.json). Record ID: `f1`.

### g147/c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d. TLDR: at this intermediate revision, fresh container-test
payload setup succeeds but the real test fails for a tmux payload that setup no longer produces. Source-confirmed route:
scripts/build-podman-test-payloads.sh:40-49 writes only farhelm-<target>.tar.gz, :59-62 validates only it and exports
the path, while the same revision's crates/farhelm/tests/e2e/podman_environments.rs:185-194 requires both that archive
and tmux-<target>. The worker's tar-of-directory layout transports the incomplete output unchanged. Synthesis resolved
the later endpoint premise: at d751ff1633bb479c7bf53920f2e4ea4ba932a1c2 the consumer's :190-196 explicitly requires only
the archive and explains the retired tmux payload; the producer at that revision also emits only the archive. Thus the
mismatch is present at 500548b and corrected by d751ff1. The earlier 04d85f0c98a9b635709d00b056719a0baa7fd3fc pair was
also consistent, then requiring and emitting both files. Neither consistent endpoint retracts the established
intermediate consequence; the charter explicitly includes intermediate changes. Contract: SPEC_impl.md, Testing,
requires coverable real-stack testing; producer and consumer artifact contracts must agree. Suggested bucket: other
(maintainer test failure). Confidence: confirmed from source only, no runtime observation. proposed_drop: false; retain
for the parent to reconcile historical handling. possible_cover: caller identifies the same later mismatch as g093/f1
and has routed consumer ownership to g067/g070; d751ff1's consumer correction is now source-verified here.
Could-be-highest: no. Caveats: old or operator-supplied directories retaining tmux can conceal the failure. No
reproduction or runtime test was performed. This revision preserves the original candidate's narrow no-layout-mismatch
conclusion at 04d85f0 and adds exact temporal boundaries rather than claiming worker correctness proves every producer
revision.

Returned proposal (claims-094, event 10): **retain-verified**, suggested bucket **other**.

The intermediate defect is established, and its repair within the review range is also established. Retain this
historical evidence for the parent's disposition; do not describe it as a current outstanding failure.

Original stronger consequence: Freshly prepared real-container tests fail before their scenario because setup omits a
file still demanded by the consumer. Earlier c4 refuted layout/scope mismatch only at 04d85f0; later records established
the 500548b mismatch and d751ff1 repair. Existing output or explicit directories retaining tmux conceal it.

Unresolved premise or limitation: No premise missing for the conditional intermediate failure. Parent must decide
handling of an already repaired historical defect and reconcile the reported external alias.

Proposed basis: Source-confirmed intermediate maintainer-test failure, repaired later in range. No established security
or user-work-loss route.

Existing parent disposition (event 10): duplicate. Correct the parent032 grouping typo after actual canonical-key
lookup. Same500548 producer-retirement/consumer-required-tmux mismatch as067l3/070l2/093f1, repaired by d751 consumer
archive-only expectation. All032 historical source/fresh-versus-stale-fixture/repair/no-current-remnant reasoning
remains; this correction only joins the actual existing group, removing erroneous duplicate-only group. No defect
dropped on a new premise.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g147 record](records/review-through-00de0b6c/groups/g147.json). Record ID: `c4`.

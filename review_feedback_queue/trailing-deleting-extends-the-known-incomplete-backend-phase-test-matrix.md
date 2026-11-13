# Deleting extends the known incomplete backend phase-test matrix

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The new deleting phase is also omitted from the backend test that claims every host-state phase matches its serialized tag. This is another assigned variant widening the already-known incomplete matrix reported by canonical g024/l2, not a new independent test root cause.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g027/l7`.

### g027/l7

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 19 (candidate, open):

Reviewed commit: 222bf6253bcd168db04bcbb1df20180c88306998

TLDR: The new deleting phase is also omitted from the backend test that claims every host-state phase matches its serialized tag. This is another assigned variant widening the already-known incomplete matrix reported by canonical g024/l2, not a new independent test root cause.

Details: HostStateView::Deleting is added with serde rename deleting at hosts.rs:278-279 and a separately written phase() literal at 310. phase() docs:283-296 justify the duplicate vocabulary by promising phase_matches_the_serialized_tag compares every variant. The test docs:1807-1815 repeat that universal guarantee. Its complete array:1818-1871 constructs Connecting, Unreachable, Connected, VersionSkew, IdentityMismatch, IdentityUnverified, Duplicate, two TensorlakeChanging directions, Suspended and Retired; it omits both Stopped and the newly added Deleting. The sole assertion:1872-1877 compares only those constructed cases. A mismatch in either new Deleting spelling therefore leaves this test satisfied. Scoped source search finds HostStateView::Deleting only in production phase/From matches, not another fixture in hosts.rs. The complete reviewed-to-tip hosts.rs diff is empty. The omission is introduced for Deleting by this exact assigned enum widening; it is not dismissed because the older Stopped omission predates this commit.

Contract: the test's explicit every-variant promise and supplied test-authoring requirement to distinguish competing mechanisms. The production spellings currently agree, which does not refute the independent oracle/documentation defect. Canonical g024/l2 records the same backend matrix missing Stopped at 56659839785691f76da3af5d8f51b511cff4ba8b. One matrix repair should include both variants and preserve both introduction commits. The refreshed g031/f8 covers the UI crate's different matrix, not this backend comparison, and is not used as a refutation.

Suggested bucket: other (maintainer test assurance). Could-be-highest: no runtime security/work-loss route asserted. Confidence: confirmed by full enum/docs/matrix comparison and unchanged endpoint, source-only. proposed_drop: duplicate/common-remediation proposal with g024/l2 only; not a correctness rejection, accepted parent drop, or claim that production correctness cures the test. possible_cover: exact parent-provided g024/l2, same backend test/root cause with the additional Deleting variant and 222bf625 provenance supplied here; related UI g031/f8 remains separate. Material caveat: this records additional evidence for a known matrix defect rather than claiming an independently novel issue. Revision: l7 source lead closed as a duplicate candidate after fixture/alternate-construction/later-repair checks; original suspected false-assurance consequence preserved. No tests, builds, reproductions, checkout edits or live actions.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g027 record](records/review-through-00de0b6c/groups/g027.json). Record ID: `l7`.


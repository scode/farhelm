# Fake-spawn cleanup can leave launch policies enabled

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A failed fake-spawn cleanup can skip restoration of shared launch policies.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Policy restoration follows fallible owned-resource cleanup without its own guaranteed path. The defect is pre-existing
private-fixture contamination; no production permission change or user-work loss was established.

Source locations: `e2e/tests/spawn.spec.ts:158-165`. Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Final discovery record, correcting supporting line numbers after numbered source inspection. Reviewed commit
d6645f771bee8ed03a66486ef8755834937d759d. TLDR: an early cleanup failure in the fake-spawn browser test can leave its
private shared helm accepting YOLO launches and agent commands without asking, contaminating later tests. At
spawn.spec.ts:100 the test enables YOLO; at 143-148 its Always allow answer is verified to enable
commands_without_asking. Finally awaits driver closure and child/parent cleanup (159-162), then scratch removal (163),
before policy restoration (164-165). cleanupSession throws on transport failure or non-404 refused stop/delete
(helpers/fleet.ts:559-570). Any such failure skips both restores; failure of the first restore also skips the second.
The evidence fixture is passive (helpers/evidence.ts:29-64), spawn has no general teardown hook, and the private stack
persists across projects with restoration delegated to each spec (e2e/playwright.config.ts:98-108). Outer-run stack
shutdown bounds the impact. This is private test contamination, not a live-installation or product user-work-loss claim.
Independent cleanup steps or nested finally, already used by the real-agent sibling (spawn.spec.ts:302-345), avoid the
gap. It is pre-existing at assigned commit bd97d312165a02c351903f6bdf2dffcbe9ef8f33, whose edit changes only a UI
string. Suggested bucket: other. Confidence: confirmed conditional source mechanism; occurrence and downstream false
results unmeasured. proposed_drop: none. Coverage: searched SPEC.md/SPEC_impl.md, TODO Planned (empty), BUGS.md,
plans/queue, INDEX and matching queue/ledger material; no exact coverage. TRIAGE_OUTCOMES.md:10827-10847 covers
real-Claude workspace-forget, 17182-17216 names notification/shortcut/checkout blocks, and
checkout-test-skips-new-spare-setting-restoration.md concerns different files/settings. possible_cover: broader intent
of the checkout-specific discard-unless-trivial decision at 20338-20351, whose named trigger/scope differ; retain that
question for parent synthesis/triage. No applicable FILTER match for durable shared policy. Provenance: g066 separate
discovery; original f1 and all revisions preserved.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g066/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.

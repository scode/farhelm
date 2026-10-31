# Cleanup Stop transport rejection skips Delete

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A transport error from a cleanup Stop can prevent Delete even when the helper already knows the fake session's ID.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The awaited cleanup-array construction short-circuits on a rejected transport before reaching Delete. An HTTP error
response is different. Version-matched Playwright source confirms rejection/retry behavior. This is a helper-owned
test-resource leak, not the pre-ID late-create cleanup gap.

Source locations: `e2e/tests/helpers/fleet.ts:559-568; e2e/tests/quick-launch.spec.ts:405-406,579`. Historical revision
qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed d6645f771bee8ed03a66486ef8755834937d759d. New discovery owner/ID a031/d1. TLDR: after a transient failure of a
test's Stop request, its known fake session can remain on the shared test stack because cleanup never attempts Delete,
even though that separate request can independently finish removal. Source proof: cleanupSession awaits post(stop) while
constructing the array before reaching await delete. A rejected Stop promise aborts array construction, skipping Delete.
An HTTP error response alone is not this trigger: with an ordinary returned response both calls execute before statuses
are checked. Digest-checked locked Playwright1.64 coreBundle.js:60754-60793,60866-60883 forwards API requests without
converting transport exceptions to responses; server request retries at27805-27827 default maxRetries to0 and rethrow
network errors. This request failure does not by itself dispose the API context. SPEC.md1431-1436 says Delete removes
sessions in any state and terminates running processes; helm sessions.rs3058-3103 directly invokes independent fenced
Delete without a prior Stop prerequisite. cleanupOwnedSteps (cleanup.ts6-18) catches the helper failure and continues
other named resources, but cannot execute the skipped Delete inside the failed step. New typed tests call the unchanged
helper for known created/source IDs (quick-launch.spec.ts405-406,579); preference reset hooks do not remove these rows.
Attribution limit: same helper implementation already exists at exclusive base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c
fleet.ts557-566, so this is an inherited helper defect reused by new tests, not a claim the reviewed commits introduced
it. Bucket OTHER, test-owned fake fixtures only. Confidence confirmed conditional failure path; actual recoverable
network failure and residual session not reproduced, no runtime-frequency claim. Coverage checks: SPEC_impl5687-5700
accepts hostile-account/concurrent-run assumptions, not same-run transport cleanup; TODO Planned is empty; BUGS headings
have no match; plans/queue searches found no repair for helper-internal Stop/Delete failure.
checkout-test-skips-new-spare-setting-restoration.md1-86 covers skipped caller restoration, not this skipped Delete.
Ledger14417-14447,14681-14703,15107-15137,17199-17216 covers named callers' independent resources;15130-15132 explicitly
left fleet.ts semantics unchanged, which is a scope limit, not acceptance of this defect. Existing catalog-cleanup item
concerns observer/pre-POST/settlement ownership, not transport rejection of Stop for a known ID; ledger20197-20209
four-file scope remains untouched. proposed_drop false; possible_cover those adjacent named-block decisions only, no
exact trigger/consequence/scope match found. Suggested correction if separately authorized: attempt Stop and Delete
independently in their current order and aggregate errors. No checkout writes, tests, builds or experiments.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a031/d1`, `a049/f6`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.

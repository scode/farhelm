# Name regression registers cleanup after assertions

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A name-field test can fail after creating a session but before registering that session for cleanup.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The accepted session's ID reaches cleanup registration only after fallible observations. A failure in between leaves the
shared-stack fixture outside that test's teardown inventory. This is an owned test-session lifetime gap; no product
data-loss consequence was established.

Source locations: `e2e/tests/sidebar.spec.ts:7610-7618,7626-7637`. Historical revision qualifiers in those references
are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d. TLDR: If the name-field browser test catches an otherwise
successful create carrying the wrong title, it can leave the accepted fixture session running on its shared test stack.
The test's POST route at 7584-7590 records request bodies and continues them to the real helm. Response waits at
7610-7615 and 7626-7631 therefore can observe accepted real creates. Request-body assertions at 7616-7617 and 7632-7633
run before created.push(response.id) at 7618 and 7634. In particular, omission of the typed title is a valid create
(SPEC.md:639-645 allows omission and automatic naming), but fails the first title assertion. The returned ID is then
never registered; finally at 7636-7637 cleans only registered IDs, so it skips that accepted session. The same ordering
affects a valid wrong title in the second create. This is a failure-path defect in the test, not evidence that the
current product loses names. Its source existed at exclusive base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c:
sidebar.spec.ts:7602-7610,7618-7629; assigned casing commit 08aaac8a4a5bcf91726a1d3bfcfc980de4116971 changes nearby
selectors, not the ownership ordering. Actual checks: full current name test 7570-7640 and historical block 7570-7638
read; searched sidebar/evidence/global setup for cleanup hooks and found none for per-test session sweeping.
evidence.ts:31-65 wraps timeline/context observation only. playwright.config.ts:56-107 shares long-lived stacks across
serial cases; resetStack is a terminal-family per-file beforeAll at terminal-suite.ts:137-222, and sidebar does not
install that hook. cleanupSession at fleet.ts:553-569 requires an ID. start-stack.sh:146-159,211-254 installs an owned
fake Codex even in real-Claude mode; the consequence is confined to test-owned sessions, not vendor work or
live-installation resources. The outer webServer shutdown at playwright.config.ts:115-159 provides eventual stack
cleanup, so no indefinite machine-process leak is claimed. Suggested fix: register the response's owned session before
asserting the request-body contract, and preserve that registration on assertion failure. Suggested bucket: other.
Confidence: confirmed conditional source failure path; the condition is an accepted valid wrong-title request that this
test is intended to detect. No runtime regression, reproduction, or resulting downstream test failure was observed; no
scheduling premise is required for the registration-order failure. proposed_drop: none. possible_cover:
review_feedback_queue/catalog-cleanup-can-start-untracked-test-session.md and TRIAGE_OUTCOMES.md:20197-20209 cover
pending async creates after observer teardown in other named files; this finding has already captured a create response
and loses registration at an assertion in sidebar.spec.ts. The eight-block cleanup decision at
TRIAGE_OUTCOMES.md:17199-17216 explicitly names other files and addresses independent releases/late replies. Trigger and
scope do not match. Coverage searches completed across SPEC.md, SPEC_impl.md, empty TODO Planned bucket, BUGS.md,
plans/queue, queue index and relevant queue file, and TRIAGE_OUTCOMES.md; no exact cover found. No FILTER.md rule covers
this ordinary test-assertion ownership gap. Provenance: new finding from the required separate g039 discovery pass,
after the initial and synthesis passes. No security, user-work-loss, or highest consequence claimed. Source-only; no
tests, builds, experiments or live actions.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g039/f2`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.

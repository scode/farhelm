# Title retry test changes the folder too, masking the behavior it claims to verify

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The title-change retry test also changes the folder, hiding its target regression.

## Details

F232 — **definite** — `e2e/tests/terminal-create-idempotency.spec.ts:201–208`;
`e2e/tests/terminal-create-idempotency.spec.ts:202` — Title retry test changes the folder too, masking the behavior it
claims to verify

A client that renews request keys only for folder changes can satisfy the test's distinct-key assertion even when
title-only renewal is broken. Changing both inputs removes the intended distinction. Keep the folder and all other
values unchanged, edit only the title, observe the second request, and require two nonempty distinct keys.

## Evidence and triage context

- e2e/tests/terminal-create-idempotency.spec.ts:176 names title-edit key renewal as the tested behavior.
- e2e/tests/terminal-create-idempotency.spec.ts:193–199 establishes a failed create using an invalid folder.
- e2e/tests/terminal-create-idempotency.spec.ts:201 changes the title and :202 independently changes the folder before
  submitting at :203.
- e2e/tests/terminal-create-idempotency.spec.ts:207–208 checks only the number of captured keys and their inequality.
- crates/farhelm-ui/src/list/create_form.rs:4238–4255 compares submission bindings and mints keys; :5389–5400 currently
  clears the key on title input. These current protections do not make the test distinguish title-only regressions.
- e2e/tests/terminal-create-idempotency.spec.ts:176 names title-edit invalidation as the property under test.
- e2e/tests/terminal-create-idempotency.spec.ts:201-202 changes both title and folder; :207-208 checks only request
  count and key inequality.
- crates/farhelm-ui/src/list/create_form.rs:5269-5286 independently clears intent_key on folder input.
- crates/farhelm-ui/src/list/create_form.rs:917-929 includes both cwd and title in IntentBinding; :5389-5400 separately
  invalidates on title input.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- review_feedback_queue/birth-oracle.md:14–27 records another defective test oracle, but its trigger is external stat
  failure and its scope is checkout birth-time checks. It does not cover this title/folder confound. No exact coverage
  was found.

Caveats:

- No current product title-key defect is claimed.
- Removing only the title input handler's key reset may still be caught by the production binding comparison; the
  demonstrated competing behavior is title-insensitive renewal with folder-sensitive renewal.
- Static counterexample, not a runtime mutation test.
- This confirms a false-pass defect in the test, not a current product idempotency failure.
- No mutation test or runtime test was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_11_cor:p1:F2`,
`test_infrastructure_11_sec:p1:F3`.

- `test_infrastructure_11_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `test_infrastructure_11_sec:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.

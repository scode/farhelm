# Keyboard-launch fixture has the same separate setup ownership gap

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Keyboard-launch setup independently leaves changed policy after failure.

## Details

F260 — **definite** — `e2e/tests/sidebar.spec.ts:5986` — Keyboard-launch fixture has the same separate setup ownership
gap

This caller also enables YOLO before its restoring finally is active. Failure after the policy write but before setup
completes leaves subsequent tests on unintended confirmation settings. Move the enabling await inside this test's
restoration guard, preserving cleanup ownership even when the helper reports failure after a committed write.

## Evidence and triage context

- e2e/tests/sidebar.spec.ts:5986-5987: setLocalYoloWithoutAsking(true) must finish before try begins. Its rejection
  bypasses restoration at 6028-6030.
- e2e/tests/helpers/fleet.ts:321-332: successful mutation is followed by a separate fallible registry read and
  assertion.
- crates/farhelm-helm/src/store.rs:4905-4910: the setting persists once its transaction commits; failure of the helper's
  later verification does not roll it back.
- crates/farhelm-helm/src/yolo_guard.rs:61-69: the persisted true value changes admission of subsequent YOLO launches.
- e2e/playwright.config.ts:83-100,142-163: subsequent tests retain the same helm even though browser lifetimes are
  separate.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Conditional on post-commit failure; no runtime reproduction was performed.
- Private stack state and fake structured harnesses bound the demonstrated impact to test correctness:
  e2e/start-stack.sh:123,211-234,324-326,431-435.
- A later explicit policy reset can end the contamination.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_10_cor:p3:F2`.

- `test_infrastructure_10_cor:p3:F2`: confidence as filed: definite; suggested bucket as filed: other.

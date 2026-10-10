# YOLO refusal fixture unintentionally puts the page into version-mismatch mode

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The YOLO-refusal fixture silently changes the page into build mismatch.

## Details

F253 — **definite** — `e2e/tests/yolo-guard.spec.ts:285` — YOLO refusal fixture unintentionally puts the page into
version-mismatch mode

The fabricated refusal uses an empty build stamp. In the independently selected spec, that deterministically latches
mismatch and disables automatic updates, so subsequent recovery assertions run in a different state from the normal
refusal-and-retry flow. Seed the response stamp from the live helm before interception and verify that refusal leaves
the page without build skew.

## Evidence and triage context

- e2e/tests/yolo-guard.spec.ts:27 imports fulfillAsHelm; :282-285 uses it for the host-setting refusal. The file
  contains no resetStack or installTerminalSuiteHooks call.
- e2e/tests/helpers/terminal-suite.ts:90 initializes HELM_BUILD to an empty string; :124 inserts it into the response;
  :147-149 initializes it only through resetStack, registered by the optional hook at :196-197.
- e2e/playwright.config.ts:126-132 creates a separate project for each spec. The fixture cannot rely on another spec
  initializing its module state.
- crates/farhelm-ui/src/list/create_form.rs:4357-4368 invokes the host-setting operation and restores the question on
  refusal; crates/farhelm-ui/src/yolo_confirm.rs:116-125 calls the actual API helper.
- crates/farhelm-ui/src/api.rs:3435-3448 sends that request through send; :1249 classifies its build stamp before
  returning the refusal.
- crates/farhelm-ui/src/skew.rs:183-185 classifies an empty stamp as Silent skew; :215-219 latches the mismatch and
  ignores later matching responses.
- crates/farhelm-ui/src/feed.rs:507-510 withdraws the invalidation feed after skew.
- e2e/tests/yolo-guard.spec.ts:303-334 continues the failure-and-retry assertions on that same page.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:3630-3634 requires withdrawal of unattended behavior on build mismatch. This explains the fixture's
  unintended effect; it does not accept an unstamped mock in a current-build recovery test.
- TRIAGE_OUTCOMES.md:6870-6885, yolo-launcher-cancel.md, covers consent restored by a queued answer after cancellation.
  Its trigger, consequence and production-code scope differ from this response-stamping fixture defect.

Caveats:

- Does not establish a production YOLO authorization bypass.
- The existing assertions still check refusal, absence of a create request, consent withdrawal and a successful retry;
  their entire security value is not invalidated.
- No browser execution performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_15_cor:p1:F2`.

- `test_infrastructure_15_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.

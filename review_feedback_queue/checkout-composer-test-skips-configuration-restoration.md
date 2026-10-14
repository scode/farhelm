# A failed override cleanup skips configuration restoration

Reviewed commit: 3d0bd43a7f9cf2f87adcb0077f011e9d5ddcb05d

## TLDR

One cleanup failure can leave later browser tests using this test’s checkout folder and configuration.

## Details

Suggested bucket: `other`. Confidence: confirmed by static failure-path tracing; not reproduced. Source: dedicated
`gpt-6.1-sol` high adversarial review. Proposed drop: none.

e2e/tests/github-checkout-composer.spec.ts:222–229 awaits clear-root before restoring the global value and removing the
temporary directory. The CLI wrapper at 132–134 rejects on nonzero exit or its 10-second timeout. If clearing the
successfully installed host override fails, all remaining cleanup is skipped, leaving the override pointing at the
fixture root. Subsequent tests use the same private stack and can fail their missing-root premises. Run independent
cleanup steps despite earlier failures and report the collected errors. Scope: the owned browser-test stack, not user
installations.

Possible cover: review_feedback_queue/single-trash-test-skips-configuration-restoration.md concerns a different test and
fixture. TRIAGE_OUTCOMES.md heading history-fixtures-policy-restoration-skip-remaining-cleanup.md concerns policy
restoration, browser contexts and sessions. Neither matches this fixture’s trigger, configuration and scope completely.

Current-main status: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703, at the same lines. No fixing
commit found.

The full commit diff and relevant failure paths were read. No runtime reproduction was run.

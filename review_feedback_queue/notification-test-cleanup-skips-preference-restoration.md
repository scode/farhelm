# Failed notification cleanup leaves shared test state behind

Reviewed commit: dc6a990bf5cbb7fd5fb61166e455e871be6a5649

## TLDR

A failed notification-test cleanup can leave its sessions and shared preferences behind, affecting later browser tests.

## Details

Suggested bucket: `other`. Confidence: confirmed statically, conditional on a cleanup request failing; failure injection
was not run. Source: separate adversarial trailing reviewer. The mechanism predates this commit and remains in the
changed test's teardown. Proposed drop: none.

`e2e/tests/notifications.spec.ts:91` removes each ID from `created` before awaiting `cleanupSession`. A transport
failure or refusal aborts the loop and skips `resetPreferences` at `92`. The strengthened test changes shared selection
at `104`; the file already changed it at `360`. `helpers/fleet.ts:520–529` confirms cleanup can throw, and `1078–1094`
restores the shared preferences. Unrouting at `90` can also fail before these releases run.

Attempt each independent release and preference restoration regardless of earlier failures, collecting failures while
preserving the test body's original error. Verify with controlled teardown failures, rather than relying on a passing
ordinary test to establish failure-path cleanup.

Possible cover: the completed `sound-tests-skip-remaining-cleanup-and-preferences.md` ledger outcome concerns the same
mechanism in `sounds.spec.ts`, not notifications. No exact Planned item, BUGS entry, queue item or accepted exception
was found. The rare-glitch filter does not clearly cover retained fixture processes and persistent shared preferences.

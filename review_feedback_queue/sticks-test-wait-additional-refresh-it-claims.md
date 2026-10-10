# The “sticks” test does not wait for the additional refresh it claims

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The unread-stability test can finish after only the first render.

## Details

F237 — **definite** — `e2e/tests/sidebar.spec.ts:7586–7609`; `e2e/tests/sidebar.spec.ts:7609` — The “sticks” test does
not wait for the additional refresh it claims

The request that initially produces unread state also satisfies the later count comparison. An automatic mark-read
arriving afterward can overwrite that state after every assertion has already passed. Request dispatch is not a
rendering boundary. Establish a fresh baseline after unread appears, then await a distinct completed refresh and
rendered result, including settlement of relevant automatic writes.

## Evidence and triage context

- e2e/tests/sidebar.spec.ts:7586–7595 captures the baseline before the mutation and then observes its initial unread
  result.
- e2e/tests/sidebar.spec.ts:7604–7611 compares against that same baseline and immediately rechecks the display.
- e2e/tests/helpers/fleet.ts:877–888 counts GET request events, not completion or rendering.
- SPEC.md:1075–1082 requires a manual unread mark to persist until reopening or new activity.
- crates/farhelm-ui/src/session_view.rs:1184–1209 currently excludes seen state from the reactive key; this does not
  repair the test oracle.
- e2e/tests/sidebar.spec.ts:7587 captures the count before clicking mark unread at 7591.
- e2e/tests/sidebar.spec.ts:7593–7595 observes the initial unread display.
- e2e/tests/sidebar.spec.ts:7609 still compares against the pre-click count.
- e2e/tests/helpers/fleet.ts:879–882 increments on request dispatch.
- SPEC.md:1079–1082 specifies persistence beyond the initial manual change.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- This establishes an invalid regression oracle, not a current product overwrite.
- No runtime mutation test was performed.
- Duplicate input retained separately.
- No claim that current production code exhibits the overwrite.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_10_cor:p1:F4`,
`test_infrastructure_10_sec:p1:F2`.

- `test_infrastructure_10_cor:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.
- `test_infrastructure_10_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.

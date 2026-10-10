# Reconnect deadline test permits the deadline-reset regression

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The reconnect test could pass even when switching restarts its deadline.

## Details

F241 — **possible** — `e2e/tests/terminal-reconnect.spec.ts:1632`; `e2e/tests/terminal-reconnect.spec.ts:1632–1643` —
Reconnect deadline test permits the deadline-reset regression

Four finite selections can finish around one second, after which a wrongly restarted 1,500 ms timer still recovers
within the test's 4,000 ms allowance. No mutation experiment ran; the test misses a possible schedule rather than
establishing a current product failure. Continue verified switching through recovery and require recovery before the
stimulus ends, or record evidence distinguishing the original deadline from the last restart.

## Evidence and triage context

- e2e/tests/terminal-reconnect.spec.ts:1610-1612 sets every retry delay to 1,500 ms; :1623 records the starting time.
- e2e/tests/terminal-reconnect.spec.ts:1632-1643 performs four selections with 250 ms waits, stops changing selection,
  then accepts a remount before 4,000 ms. With small action overhead, resetting at the fourth selection allows recovery
  around 2,250 ms and satisfies the assertion.
- e2e/tests/helpers/terminal-suite.ts:456-460 clicks the requested terminal and verifies visibility; it introduces no
  requirement that churn continue until recovery.
- crates/farhelm-ui/src/session_view.rs:1684, :1730, :1744 and :1798-1823 carry selection changes into terminal
  specifications and call farhelmTerm.sync.
- crates/farhelm-ui/assets/terminal.js:2871-2872 currently restricts applyCapabilityChange to genuine capability
  changes. Calling it on ordinary syncs would reach :1467 and scheduleReconnect; :1621-1635 clears the existing timeout
  and starts a full new delay.
- e2e/tests/terminal-reconnect.spec.ts:112-122 requires a different socket, but imposes no original-deadline constraint.
  The subsequent marker assertion at :1644-1645 also accepts a delayed successful recovery.
- terminal-reconnect.spec.ts:1610–1612 sets 1500 ms rungs; :1632–1635 performs four selections with 250 ms gaps;
  :1638–1643 accepts remount before 4000 ms. helpers/terminal-suite.ts:456–460 adds no fixed long delay.
  terminal.js:1620–1635 shows resetting a scheduled rung is clearTimeout followed by a fresh step.delayMs. No runtime
  mutation established the complete counterfactual, so retain as possible rather than definite.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No runtime mutation experiment was performed. The false-pass execution follows from the timers and assertions; it need
  not occur under every machine load.
- The frozen production code contains the capability-change guard. This finding does not establish a current product
  reconnection failure.
- SPEC.md:1232-1238 requires automatic recovery. TODO.md:33-43 plans supervisor request-loop work, not this browser
  test. No matching acceptance, queue item or ledger disposition was identified.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_13_cor:p1:F1`,
`test_infrastructure_13_sec:p1:C2`.

- `test_infrastructure_13_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `test_infrastructure_13_sec:p1:C2`: confidence as filed: possible; suggested bucket as filed: not separately tagged in
  candidate list.

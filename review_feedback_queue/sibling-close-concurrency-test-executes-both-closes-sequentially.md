# The sibling-close concurrency test executes both closes sequentially

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The sibling-close test could pass with a global serialization guard.

## Details

F251 — **possible** — `e2e/tests/terminal-tabs.spec.ts:1177` — The sibling-close concurrency test executes both closes
sequentially

The second close starts only after the first tab disappears. A global guard held until the first close completes can
therefore satisfy every assertion, including request counts, despite the claimed concurrent-close scenario. No current
product guard defect is established. Hold A's close response, observe B's DELETE while A remains in flight, and release
held responses unconditionally.

## Evidence and triage context

- e2e/tests/terminal-tabs.spec.ts:1170–1179 confirms A and waits for its removal before :1183–1184 starts B.
- e2e/tests/terminal-tabs.spec.ts:1187 verifies one DELETE per tab but no overlap.
- crates/farhelm-ui/src/session_view.rs:1581 currently guards by tab ID; :1592–1609 applies successful close state;
  :1620 releases that tab's guard.
- terminal-tabs.spec.ts:1144-1151 states that a sibling can close alongside A and identifies a global guard as the
  regression. :1177-1185 waits for A's removal before initiating B, and :1187 only checks one DELETE per ID.
  session_view.rs:1577-1583 and :1620 show the actual per-tab guard lifetime. Hold A's close and observe B's request
  before releasing A to test the stated property. This is a concrete false-pass defect, not evidence that the current
  product has a global guard.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- The duplicate-DELETE assertion remains useful.
- No current global-lock product defect is established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_14_cor:p1:F4`,
`test_infrastructure_14_sec:p1:C2`.

- `test_infrastructure_14_cor:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.
- `test_infrastructure_14_sec:p1:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.

# Pending-mount cancellation test cannot distinguish cancellation from a surviving retry

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The pending-mount test cannot tell cancelled retries from surviving retries.

## Details

F250 — **definite** — `e2e/tests/terminal-tabs.spec.ts:1041`; `e2e/tests/terminal-tabs.spec.ts:1042` — Pending-mount
cancellation test cannot distinguish cancellation from a surviving retry

After the target element disappears, a leaked retry timer cannot mount anything, producing the same mounted-island list
as successful cancellation. Waiting longer for no island therefore still misses a timer that continues indefinitely.
Establish a pending attempt and observe cancellation directly, or use a controlled target and clock that make a
surviving retry detectable.

## Evidence and triage context

- e2e/tests/terminal-tabs.spec.ts:1019–1027 withholds Terminal and observes only the agent island; :1029–1032 removes
  the tab.
- e2e/tests/terminal-tabs.spec.ts:1037–1043 restores Terminal and polls the already-existing island list.
- crates/farhelm-ui/assets/terminal.js:3243–3264 requires the DOM element and otherwise schedules another retry every 50
  ms.
- crates/farhelm-ui/assets/terminal.js:2912–2915 calls unmount for departures; :5554–5559 currently clears the pending
  timer and map entry.
- terminal-tabs.spec.ts:544-545 observes mounted islands only. terminal.js:3243-3263 can keep retrying every 50 ms when
  the removed tab has no DOM element, without creating an island. Thus a regression leaving the pending retry alive can
  satisfy the test's island assertion even beyond its first immediate pass.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"citation": "TRIAGE_OUTCOMES.md:5429, takeover-latch-misses-attaching-tabs.md", "comparison": "Different trigger and
  consequence: takeover during attachment and loss of attachment ownership. It does not cover this test's inability to
  detect a retry surviving tab removal."}

Caveats:

- Current production code cancels the pending attempt.
- The fixture also lacks a direct witness that the attempt entered its pending state.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_14_cor:p1:F3`,
`test_infrastructure_14_sec:p1:C3`.

- `test_infrastructure_14_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
- `test_infrastructure_14_sec:p1:C3`: confidence as filed: definite; suggested bucket as filed: not separately tagged in
  candidate list.

# Wedged-browser teardown test can pass before browser cleanup finishes

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The wedged-browser test observes a detach that does not prove handler completion.

## Details

F277 — **definite** — `crates/farhelm-helm/src/terminal.rs:1261` — Wedged-browser teardown test can pass before browser
cleanup finishes

Queue overflow and a supervisor detach message can independently generate the same upstream Detach frame accepted by the
test. It can therefore pass while the browser handler or outbound task remains blocked. Current production notice sends
are bounded. Establish blocked sending with a controlled gate and observe completion owned directly by the handler or
outbound task.

## Evidence and triage context

- crates/farhelm-helm/src/terminal.rs:1205-1209 and 1259-1286: claims upstream Detach proves the blocked browser notice
  was abandoned, then accepts the first matching Detach.
- crates/farhelm-helm/src/client.rs:2008-2011 and 1970-1978: terminal-queue overflow removes the entry and independently
  initiates Detach.
- crates/farhelm-helm/src/client.rs:1795-1820: the scripted supervisor Detached also initiates an acknowledgement when
  the entry still exists.
- crates/farhelm-helm/src/client.rs:2042-2048: Detach is sent by a separately spawned task.
- crates/farhelm-helm/src/terminal.rs:630-664 and 727-730: production bounds notice sends and separately observes
  outbound-task completion; the test does not observe that completion.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TODO.md:283-286 concerns product stall attribution, not this test's completion oracle.
- TRIAGE_OUTCOMES.md:113-126 and 835-852 concern production detach waits and notification ownership, not proof that this
  browser handler ended.

Caveats:

- No runtime or mutation reproduction.
- The fixed flood also does not independently establish a blocked WebSocket send.
- This is a test defect; current production notice sends are bounded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `hc_general:p2:F1`.

- `hc_general:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.

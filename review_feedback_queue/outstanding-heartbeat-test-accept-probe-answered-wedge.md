# Outstanding-heartbeat test can accept a probe answered before the wedge

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The heartbeat test could count a probe answered before the intended wedge.

## Details

F243 — **possible** — `e2e/tests/terminal-reconnect.spec.ts:1547` — Outstanding-heartbeat test can accept a probe
answered before the wedge

Its ping assertion accepts any earlier recorded probe, including one answered before the terminal handler is silenced.
Revocation before the next probe then exercises no outstanding-answer deadline. That schedule was not reproduced, and
other expiry guards limit the claimed regression. Establish a terminal-socket baseline at silencing and require a later
ping plus evidence that its answer was not processed before revocation.

## Evidence and triage context

- e2e/tests/terminal-reconnect.spec.ts:1488-1495 configures the outstanding-answer variant with an 800 ms idle interval
  and a 12,000 ms answer deadline.
- e2e/tests/terminal-reconnect.spec.ts:1500-1507 records matching frames from every page WebSocket before
  openOwnTerminal is called at :1515. The collection is neither socket-specific nor reset at silencing.
- e2e/tests/terminal-reconnect.spec.ts:1538-1545 completes additional setup and replaces the current terminal's
  onmessage handler. At :1548-1550, pings.length > 0 can already be satisfied by pre-silencing traffic.
- crates/farhelm-ui/assets/terminal.js:5394 starts the heartbeat on socket open; :4529 sends the probe.
  crates/farhelm-helm/src/terminal.rs:679-693 accepts that ping and requests a pong.
- crates/farhelm-ui/assets/terminal.js:4602-4614 calls armHeartbeat for received frames; :4482-4486 and :4505 clear the
  outstanding deadline. Thus a healthy response before silencing can leave only the next idle timer armed.
- e2e/tests/terminal-reconnect.spec.ts:1553-1564 then triggers build-skew revocation without proving a new unanswered
  probe. The final checks at :1569-1587 verify subsequent silence and socket survival, but cannot establish the missing
  prior state.
- crates/farhelm-ui/assets/terminal.js:4537-4538 independently rechecks capability when an outstanding deadline expires.
  Consequently, merely retaining that timer while preserving this guard is not evidence of harmful revocation failure.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- The unverified premise is that a healthy probe is answered during setup and revocation reaches the page before the
  next post-silencing probe.
- The final identity assertion can catch harmful outstanding-deadline behavior when the intended unanswered-probe
  premise actually holds.
- Suggested wording should focus on failure to establish the outstanding-probe case, rather than claiming that timer
  cancellation is the only production safeguard.
- SPEC_impl.md:3630-3634 requires heartbeat and automatic reconnect to stop under build mismatch; it does not accept
  this missing test premise.
- TRIAGE_OUTCOMES.md:4630-4649 addresses server-side event-feed liveness timers postponed by revision writes. It does
  not cover browser terminal-heartbeat revocation or this test's pre-silencing ping counter.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_13_cor:p1:F3`.

- `test_infrastructure_13_cor:p1:F3`: confidence as filed: possible; suggested bucket as filed: other.

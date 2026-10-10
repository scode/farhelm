# Wedged-browser test accepts an independent cleanup signal

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The cleanup signal in the wedged-browser test has independent producers.

## Details

F280 — **definite** — `crates/farhelm-helm/src/terminal.rs:1266` — Wedged-browser test accepts an independent cleanup
signal

Both connection-reader cleanup paths can send the precise Detach frame used as success evidence through a separate task.
The frame therefore cannot establish release of the blocked browser handler's resources. The fixed flood also does not
prove the sink is blocked. Use controlled blocked-sink setup and an observable completion boundary owned by the handler
or outbound task.

## Evidence and triage context

- crates/farhelm-helm/src/terminal.rs:1266-1286: waits for a matching Detach rather than browser-handler completion.
- crates/farhelm-helm/src/client.rs:2008-2011 and 1970-1978: overflowing the terminal queue independently releases
  upstream.
- crates/farhelm-helm/src/client.rs:1795-1820: receiving the scripted Detached independently releases upstream.
- crates/farhelm-helm/src/client.rs:2042-2048: the release sender is a separate task.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TODO.md:283-286 concerns stall-cause attribution.
- TRIAGE_OUTCOMES.md:113-126 and 835-852 concern production teardown behavior, not this regression-test oracle.

Caveats:

- No runtime or mutation reproduction.
- Current production notice sends are bounded.
- The fixed flood does not independently prove the sink is blocked.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `hc_systems:p1:F2`.

- `hc_systems:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.

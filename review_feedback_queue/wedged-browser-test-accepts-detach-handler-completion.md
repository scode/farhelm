# Wedged-browser test accepts detach before handler completion

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A matching detach frame can precede browser-handler cleanup.

## Details

F279 — **definite** — `crates/farhelm-helm/src/terminal.rs:1276` — Wedged-browser test accepts detach before handler
completion

The assertion accepts an upstream release frame that the connection reader can emit on terminal-queue overflow or a
scripted supervisor detach. Neither producer requires the browser handler to finish, so a reintroduced indefinitely
blocked notice can escape the test. Observe handler or outbound-task completion directly and establish blocked sending
deterministically; current production sends remain bounded.

## Evidence and triage context

- crates/farhelm-helm/src/terminal.rs:1238-1257: floods output and explicitly sends supervisor Detached.
- crates/farhelm-helm/src/terminal.rs:1266-1286: success requires only observing one matching Detach.
- crates/farhelm-helm/src/client.rs:1970-1978 and 2008-2011: overflow independently sends Detach.
- crates/farhelm-helm/src/client.rs:1795-1820 and 2042-2048: supervisor Detached independently produces the same frame
  through a spawned sender.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TODO.md:283-286 addresses product attribution, not this assertion.
- TRIAGE_OUTCOMES.md:113-126 addresses a production detach deadline.

Caveats:

- No runtime or mutation reproduction.
- Current production notice sends are bounded at terminal.rs:630-664.
- The flood's blocked-send premise is not directly observed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `hc_lifecycle:p2:F1`.

- `hc_lifecycle:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.

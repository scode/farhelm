# Wedged-browser test may encounter queue backpressure before a blocked socket write

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Queue overflow can satisfy the wedged-browser test before socket cleanup.

## Details

F290 — **definite** — `crates/farhelm-helm/src/terminal.rs:1276` — Wedged-browser test may encounter queue backpressure
before a blocked socket write

The fixture floods far more frames than the terminal event queue holds. Overflow independently emits the same Detach
that the test treats as completion of blocked browser teardown. A still-blocked handler can therefore coexist with
success. Establish a blocked socket send deterministically and observe handler or outbound-task completion directly
rather than inferring it from upstream release.

## Evidence and triage context

- crates/farhelm-helm/src/terminal.rs:1238–1246: the test floods 2,000 terminal frames.
- crates/farhelm-helm/src/terminal.rs:1276–1284: any matching upstream Detach satisfies its teardown assertion.
- crates/farhelm-helm/src/client.rs:73: the terminal event queue holds 256 events.
- crates/farhelm-helm/src/client.rs:2010–2011: overflow calls detach_stalled_terminal.
- crates/farhelm-helm/src/client.rs:1970–1978,2042–2047: that helper independently spawns an upstream Detach send.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"matched": null, "possible": null}

Caveats:

- The current handler has bounded teardown.
- No mutation experiment was run.
- The original socket-backpressure premise is also unobserved, but the independently emitted Detach is the concrete
  discriminating defect.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_sec_07:p1:C7`.

- `gap_helm_connections_sec_07:p1:C7`: confidence as filed: definite; suggested bucket as filed: not separately tagged
  in candidate list.

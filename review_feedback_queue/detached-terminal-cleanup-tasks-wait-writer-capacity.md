# Detached-terminal cleanup tasks wait for writer capacity

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Repeated detach cleanup could accumulate tasks waiting for writer capacity.

## Details

F325 — **possible** — `crates/farhelm-helm/src/client.rs:2042` — Detached-terminal cleanup tasks wait for writer
capacity

Each cleanup spawns an independent send task. Recurring backpressure with continued writer progress can keep tasks
waiting without triggering the progress-based connection timeout; cancellation eventually ends them but does not
establish a bound while the connection survives. Sustained growth and practical exhaustion were not demonstrated. Assess
and bound outstanding cleanup ownership, for example by coalescing or limiting pending detach work.

## Evidence and triage context

- client.rs:2042-2048 spawns one independently waiting send per cleanup; callers include terminal overflow at 1970-1978,
  closed receivers at 2013-2022 and explicit detach at 3279-3281. Writer cancellation ends waits, but its progress-based
  timeout at 1456-1464 does not itself prove a bound while progress continues. SPEC_impl.md:64-69 is not exact coverage
  for locally generated cleanup-task accumulation.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No ordinary sustained-growth reproduction or rate argument established.
- No work loss or practical exhaustion confirmed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `hc_systems:p1:C8`.

- `hc_systems:p1:C8`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.

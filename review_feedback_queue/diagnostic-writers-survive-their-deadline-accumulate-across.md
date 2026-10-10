# Diagnostic writers survive their deadline and accumulate across test attempts

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Diagnostic timeouts leave writers running across repeated attempts.

## Details

F335 — **definite** — `scripts/record-test-run.py:306–323` — Diagnostic writers survive their deadline and accumulate
across test attempts

The recorder starts a daemon writer and stops waiting after 100 ms without stopping its write. Blocked stderr retains
one thread per in-process attempt; full nonblocking stderr instead triggers continuous retries. Repetition can exhaust
limits or contaminate timing measurements, while standalone process exit limits accumulation. Bound delivery across
invocations with owned workers and bounded buffering, and wait or stop under nonblocking backpressure rather than
spinning.

## Evidence and triage context

- scripts/record-test-run.py:306–323 creates and waits for diagnostic writers; :1649 emits one each attempt.
- scripts/test_hunt.py:192–225 repeatedly invokes the recorder in one process; :56–64 permits 1,000 attempts.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None found. FILTER.md’s diagnostic-only filter does not cover resource disruption.

Caveats:

- Requires blocked or persistent backpressure on stderr. Standalone invocations discard daemon threads at process exit.
  No runtime reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_15_sec:p2:F1`.

- `test_infrastructure_15_sec:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.

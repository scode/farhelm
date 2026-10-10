# Cancelled lock waiters can leave permanent registry entries

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Cancelled lock waiters leave retained registry keys.

## Details

F199 — **definite** — `crates/farhelm-supervisor/src/service/core.rs:1721` — Cancelled lock waiters can leave permanent
registry entries

A departing lock holder sees the waiter's strong reference and retains the map entry. If the waiter is then cancelled
before constructing its cleanup guard, the weak allocation and key have no remaining cleanup owner. Distinct abandoned
keys accumulate until registry destruction. Make waiting participation cancellation-safe or prune expired entries while
preserving exactly one active lock per key; practical memory pressure remains unmeasured.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:1687 installs weak entries and does not prune unrelated expired keys.
- crates/farhelm-supervisor/src/service/core.rs:1718 constructs cleanup ownership only after lock acquisition; line 1769
  also permits timeout before guard construction.
- crates/farhelm-supervisor/src/service/core.rs:1878 removes an entry only when an acquired guard drops and sees at most
  one strong reference.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- Not the separate provisioning lock-map item. The transient-glitch filter does not clearly cover retained allocations
  that remain until reuse or supervisor destruction.

Caveats:

- The retained allocation is small per distinct key.
- A later successful claim for the same key can clean it up.
- No occurrence rate or security impact was established.
- A subsequent successful claim of the same key can clean it up.
- The proposed title should avoid calling every retained entry permanent.
- No occurrence rate, material memory pressure or security impact established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_sec_01:p1:F4`.

- `gap_supervisor_state_sec_01:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.

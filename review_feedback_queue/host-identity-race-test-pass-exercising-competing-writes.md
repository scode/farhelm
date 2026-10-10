# Host-identity race test can pass without exercising competing writes

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The identity-race test can pass after the write guard is removed.

## Details

F166 — **definite** — `crates/farhelm-supervisor/src/store.rs:8782` — Host-identity race test can pass without
exercising competing writes

One apparent racer can finish before the other's initial read, letting the second return an existing identity without
competing to write. Even synchronizing absent reads is insufficient if both writes precede both readbacks. Force both
reads to see absence, control completion before the second write attempt, and require both returned identities and the
durable row to agree.

## Evidence and triage context

- crates/farhelm-supervisor/src/store.rs:8755–8774: the test claims to discriminate removal of the conditional upsert
  guard.
- crates/farhelm-supervisor/src/store.rs:8779–8802: join! submits two calls but establishes no competing-write boundary.
- crates/farhelm-supervisor/src/db.rs:128–142: calls use independently scheduled spawn_blocking closures.
- crates/farhelm-supervisor/src/store.rs:5244–5253: observing an existing identity returns without writing.
- crates/farhelm-supervisor/src/store.rs:5255–5271: the conditional upsert and final read provide the actual protection.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TODO.md:594–609 proposes reconsidering helm-side identity-recording machinery in Maybe later; it does not cover this
  supervisor-store test or reside in Planned.

Caveats:

- The production conditional upsert is present.
- Production ownership gating further limits competing mint callers; this finding concerns the explicit store-level
  regression contract.
- No mutation test was run.
- Synchronizing only the two absent reads is insufficient for deterministic discrimination if both writes can precede
  both final reads.
- Production currently retains the conditional guard.
- Production state-directory ownership also limits competing mint callers.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_08:p1:F1`.

- `gap_supervisor_state_cor_08:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.

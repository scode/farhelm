# Ended-session replay test also accepts relaunch under the same identity

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Ended-session replay can relaunch work without failing its test.

## Details

F159 — **definite** — `crates/farhelm-supervisor/src/service/core.rs:25457` — Ended-session replay test also accepts
relaunch under the same identity

A replay implementation can execute create again under the existing reserved session identity and still return one row,
the same identifier, and a Created reservation. Those assertions do not distinguish recovery from duplicate execution.
Require preservation of the Exited state and exit code 1, together with an observable absence of any launch attempt.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:25437–25440: the fixture records an exited session.
- crates/farhelm-supervisor/src/service/core.rs:25457–25471: assertions check only identity, row count, and Created
  reservation.
- crates/farhelm-supervisor/src/store.rs:3818–3822: the current outcome guard prevents exited-row takeover.
- crates/farhelm-supervisor/src/store.rs:3871–3917: takeover deletes and reinserts under the same identity.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- The current production guard prevents the described relaunch.
- No mutation test was run.
- The production outcome guard is present.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_04:p1:F3`.

- `gap_supervisor_state_cor_04:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.

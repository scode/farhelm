# Cgroup cleanup masks the fork-quiescing regression

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A scope can eliminate the fork storm before quiescing is tested.

## Details

F177 — **definite** — `crates/farhelm/tests/e2e/session_lifecycle.rs:6613` — Cgroup cleanup masks the fork-quiescing
regression

The fixture's systemd process scope can be killed before the portable sweep takes its first snapshot. Removing the
sweep's fork-stopping and repeated enumeration can therefore leave final death checks passing through group cleanup
alone. Disable scopes and verify the unscoped launch before Stop, making success depend on controlling the portable fork
race.

## Evidence and triage context

- crates/farhelm/tests/e2e/session_lifecycle.rs:6575-6608 describes the SIGSTOP/re-enumeration regression the test must
  detect.
- crates/farhelm/tests/e2e/session_lifecycle.rs:6613 enables default scopes; :6640 confirms a grandchild and :6642
  confirms marked processes, but neither check excludes scope containment.
- crates/farhelm/tests/e2e/session_lifecycle.rs:6646 stops; :6648-6652 only requires an empty final marker scan.
- crates/farhelm-fixtures/src/fake_agent.rs:363-378 starts the bounded fork storm without cgroup migration.
- crates/farhelm-supervisor/src/service/sweep.rs:1398 kills scopes before :1406 starts the portable sweep.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- The documented historical mutation result may be valid for its original substrate; it does not refute this scoped
  false-pass path.
- No mutation test was run.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_08_cor:p1:F4`.

- `cli_installation_08_cor:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.

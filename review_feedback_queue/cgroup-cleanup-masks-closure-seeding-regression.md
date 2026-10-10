# Cgroup cleanup masks the closure-seeding regression

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Scope teardown masks failure to discover an unmarked child.

## Details

F176 — **definite** — `crates/farhelm/tests/e2e/session_lifecycle.rs:5587` — Cgroup cleanup masks the closure-seeding
regression

Removing the child's environment marker does not remove either fixture process from the systemd launch scope. Cleanup of
that process group can kill both without portable ancestry expansion working, leaving the regression green. Disable
scopes and assert their absence, while preserving the marker and live-parent premises needed to distinguish discovery of
the unmarked descendant.

## Evidence and triage context

- crates/farhelm/tests/e2e/session_lifecycle.rs:5553-5560 promises to detect treating marker matches as leaves rather
  than closure roots.
- crates/farhelm/tests/e2e/session_lifecycle.rs:5587 uses the default harness; :5618-5627 checks marker presence and
  absence; :5636-5646 checks the parent relationship; :5648-5651 stops and checks death.
- crates/farhelm-fixtures/src/fake_agent.rs:2655-2658 uses env -u for the child but performs no cgroup migration.
- crates/farhelm-supervisor/src/service/sweep.rs:1398-1406 kills the scope before walking the portable process closure.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- The existing marker and parent-premise checks are useful but do not exclude the alternative cleanup mechanism.
- No mutation test was run.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_08_cor:p1:F3`.

- `cli_installation_08_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.

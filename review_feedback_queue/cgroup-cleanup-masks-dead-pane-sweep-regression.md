# Cgroup cleanup masks the dead-pane sweep regression

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The dead-pane sweep test can pass because scope cleanup kills the survivor.

## Details

F175 — **definite** — `crates/farhelm/tests/e2e/session_lifecycle.rs:5514` — Cgroup cleanup masks the dead-pane sweep
regression

Killing the pane process removes the ancestry root but leaves the daemon in its systemd launch scope, a separately owned
process group. Stop can kill that group even when the portable dead-pane sweep is skipped. Disable scopes and verify the
unscoped premise before killing the agent, so the final death assertion depends on the intended discovery path.

## Evidence and triage context

- crates/farhelm/tests/e2e/session_lifecycle.rs:5500 promises detection of a skipped dead-pane sweep.
- crates/farhelm/tests/e2e/session_lifecycle.rs:5514 uses default scopes; :5545 kills the agent, then :5549 calls Stop
  and :5550 checks daemon death.
- crates/farhelm-fixtures/src/fake_agent.rs:2655 leaves the daemon in its inherited cgroup.
- crates/farhelm-supervisor/src/service/sweep.rs:1398 performs scope cleanup before portable sweeping.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact existing coverage. Distinct fixture and regression premise from the preceding marker-discovery finding.

Caveats:

- Requires successful scoped launch.
- No mutation execution.
- This fixture is independently editable from the other scope-masked tests.
- Requires a successful scoped launch; no mutation execution.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_08_cor:p1:F2`.

- `cli_installation_08_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.

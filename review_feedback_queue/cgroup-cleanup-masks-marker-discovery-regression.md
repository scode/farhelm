# Cgroup cleanup masks the marker-discovery regression

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Scope cleanup hides a broken portable marker-discovery path.

## Details

F174 — **definite** — `crates/farhelm/tests/e2e/session_lifecycle.rs:5465` — Cgroup cleanup masks the marker-discovery
regression

On systemd hosts, the fixture daemon remains in its launch scope, a process group that cleanup can terminate together.
That group dies even if portable environment-marker discovery is removed, satisfying the death-only assertions. The test
misses its promised unscoped discovery behavior. Disable scopes for the fixture and verify that launch recorded no scope
before exercising cleanup.

## Evidence and triage context

- crates/farhelm/tests/e2e/session_lifecycle.rs:5454 promises to detect missing marker injection/enumeration, but :5465
  uses default harness scopes.
- crates/farhelm/tests/e2e/harness.rs:1546 selects default seams.
- crates/farhelm-supervisor/src/service/core.rs:1060 selects the real systemd scope manager.
- crates/farhelm-fixtures/src/fake_agent.rs:2655 reparents without leaving the inherited cgroup.
- crates/farhelm-supervisor/src/service/sweep.rs:1398 kills scopes before the process sweep at :1406.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- Production scope-first cleanup is legitimate; no existing coverage of this test isolation defect found.

Caveats:

- Conditional on obtaining a working scope.
- No mutation test was run.
- SPEC_impl.md:2115-2138 accepts production scope-first cleanup, not this test's inability to isolate its promised
  mechanism.
- Conditional on successful scoped launch; no mutation run. Keep this fixture separately editable from the next one.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_08_cor:p1:F1`.

- `cli_installation_08_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.

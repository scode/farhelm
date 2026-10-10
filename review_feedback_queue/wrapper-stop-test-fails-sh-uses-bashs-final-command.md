# Wrapper-stop test fails when sh uses Bash’s final-command optimization

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A valid shell optimization breaks the wrapper-stop fixture.

## Details

F188 — **definite** — `crates/farhelm/tests/e2e/wrapper_launch.rs:661` — Wrapper-stop test fails when sh uses Bash’s
final-command optimization

Some shells replace themselves with the final sleep command. The fixture then observes sleep as the reported child and
never gets the grandchild it requires, failing before product Stop behavior is exercised. Use a command that explicitly
keeps the intermediate shell alive, so the expected process topology is a fixture guarantee rather than an assumption
about shell optimization.

## Evidence and triage context

- crates/farhelm/tests/e2e/wrapper_launch.rs:661 unconditionally requires a grandchild.
- crates/farhelm-fixtures/src/fake_agent.rs:349 selects sleep 3600; :2496 invokes sh -c without a following command.
- crates/farhelm/tests/e2e/harness.rs:2214 fails when that process never forks.
- crates/farhelm/tests/e2e/wrapper_launch.rs:627 documents the same final-command optimization and its workaround for
  the outer wrapper.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact coverage identified.

Caveats:

- Applies when sh performs the final-command optimization; a dash-based host can pass.
- I did not rerun or independently inspect the source report's recorded shell probe.
- Shell dependent; a dash-based host can satisfy the fixture assumption.
- No shell probe or test was run during this verification.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_09_cor:p1:F1`.

- `cli_installation_09_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.

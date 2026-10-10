# Git fixture setup can modify or delete the caller’s repository

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Git test fixtures can modify a repository outside their temporary directory.

## Details

F17 — **definite** — `crates/farhelm-supervisor/src/repository_discovery.rs:985` — Git fixture setup can modify or
delete the caller’s repository

Fixture Git commands inherit repository-selection and configuration overrides. An absolute `GIT_DIR` can therefore
redirect configuration writes despite the fixture's working directory. If earlier setup steps succeed, another fixture
can relocate that repository's metadata into temporary storage that is later deleted, risking unpublished history.
Remove inherited Git repository and configuration overrides from child commands and isolate their configuration without
changing the environment of the process running the tests.

## Evidence and triage context

- crates/farhelm-supervisor/src/repository_discovery.rs:984–991: fixture Git commands inherit repository-selection and
  configuration environment variables.
- crates/farhelm-supervisor/src/repository_discovery.rs:564–573: the fixture writes include.path using that command
  builder.
- crates/farhelm-supervisor/src/repository_discovery.rs:510–540: another fixture invokes init --separate-git-dir after
  adding origin.
- scripts/record-test-run.py:1212–1221: the recorder scrubs FARHELM_* variables, not GIT_* variables.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- The metadata-relocation consequence requires the preceding remote add to succeed, including absence of a conflicting
  origin.
- The configuration-write consequence does not require that additional premise.
- No destructive reproduction was run.
- The relocation branch additionally requires earlier commands, including remote add, to succeed.
- No destructive reproduction was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_01:p1:F1`.

- `gap_supervisor_state_cor_01:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.

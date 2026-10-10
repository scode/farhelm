# Directory witness can be read before pwd writes it

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The directory-witness test reads publication before it is complete.

## Details

F184 — **definite** — `crates/farhelm/tests/e2e/session_lifecycle.rs:2081` — Directory witness can be read before pwd
writes it

Shell redirection creates the witness file before `pwd` writes its contents. The test treats readability as readiness
and can therefore read an empty file, rejecting a correctly launched directory. Require a complete newline-terminated
witness, or publish the finished witness by rename, so the assertion waits for data rather than mere file existence.

## Evidence and triage context

- crates/farhelm/tests/e2e/session_lifecycle.rs:1608 launches shell redirection into the witness.
- crates/farhelm/tests/e2e/session_lifecycle.rs:2081 returns on any successful read, including an empty file.
- crates/farhelm/tests/e2e/session_lifecycle.rs:1622 immediately compares the returned contents with the directory.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact coverage identified; no unambiguous FILTER match.

Caveats:

- The publication window was not reproduced.
- This establishes a test race, not a shipped working-directory expansion defect.
- No runtime reproduction.
- This concerns the acceptance test, not a demonstrated product cwd-expansion defect.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_07_cor:p1:F4`.

- `cli_installation_07_cor:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.

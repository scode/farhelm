# The generic-launch assertion uses a different argv encoding from its fixture

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The generic-launch test mistakes quoting differences for changed arguments.

## Details

F138 — **definite** — `crates/farhelm/tests/e2e/hook_identity.rs:918`; `crates/farhelm/tests/e2e/hook_identity.rs:919` —
The generic-launch assertion uses a different argv encoding from its fixture

The fixture and expected marker serialize command arguments differently. A valid binary pathname containing spaces or
shell metacharacters can consequently produce unequal strings despite identical argument vectors. Parse the marker with
the shell-word parser and compare vectors, or serialize both sides identically, so quoting representation does not
create a false launch regression.

## Evidence and triage context

- hook_identity.rs:888–904 constructs and correctly shell-quotes the requested argument vector. fake_agent.rs:881–884
  serializes actual argv with shell_words::join. hook_identity.rs:918–921 compares it with a plain-space join.
  harness.rs:325–335 derives the fixture executable from Cargo's target directory.
- hook_identity.rs:888–904 launches the correctly quoted requested vector. fake_agent.rs:884 emits
  shell_words::join(std::env::args()), while hook_identity.rs:919–920 expects requested.join(" ").

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- The asserted exact-argv contract is hook_identity.rs:868–880. No exact Planned, BUGS.md, queue, ledger, or filter
  coverage found.
- No exact Planned, BUGS.md, queue, ledger, or filter coverage found; the advertised argv-preservation contract is
  hook_identity.rs:868–880.

Caveats:

- No relocated-checkout execution. The shipped launch can be correct while this assertion fails.
- No relocated-checkout execution. No shipped argument mutation established. Same location as
  cli_installation_05_sec:p1:F2.
- No relocated-checkout execution. No shipped argument mutation was demonstrated.
- No relocated-checkout execution. Same-location duplicate of cli_installation_05_cor:p1:F7; no shipped launch
  corruption established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_05_cor:p1:F7`,
`cli_installation_05_sec:p1:F2`.

- `cli_installation_05_cor:p1:F7`: confidence as filed: definite; suggested bucket as filed: other.
- `cli_installation_05_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.

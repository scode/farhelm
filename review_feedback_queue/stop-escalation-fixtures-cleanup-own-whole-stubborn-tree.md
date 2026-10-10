# Stop-escalation fixture’s cleanup does not own its whole stubborn tree

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The stubborn Stop fixture lacks cleanup ownership of its whole process tree.

## Details

F186 — **definite** — `crates/farhelm/tests/e2e/session_lifecycle.rs:4567` — Stop-escalation fixture’s cleanup does not
own its whole stubborn tree

On macOS the PID guard cannot perform its promised cleanup. On shells that retain an intermediate shell, killing only
that shell also leaves its independently running, signal-ignoring sleep, potentially for an hour. A failure before
normal Stop can therefore leak fixture work. Arm portable ownership and cleanup for the complete tree before subsequent
fallible operations.

## Evidence and triage context

- crates/farhelm/tests/e2e/session_lifecycle.rs:4567 guards only the reported child PID.
- crates/farhelm-fixtures/src/fake_agent.rs:357 starts a TERM/HUP-ignoring shell command; :2496 spawns it through sh.
- crates/farhelm/tests/e2e/harness.rs:2397 skips cleanup without procfs identity; :2403 kills only one PID.
- crates/farhelm-teststate/src/tmux/guard.rs:85 shuts down the tmux server rather than owning this stubborn descendant
  tree.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact coverage identified. The hung-dependency filter does not cover ordinary assertion failures.

Caveats:

- The extra Linux descendant depends on the selected sh retaining its process rather than tail-execing sleep.
- No failure-path reproduction was run.
- This is leaked fixture work, not established destruction of unrelated user work.
- The extra Linux descendant depends on shell behavior.
- No unrelated user-process destruction is established.
- No failure-path reproduction performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_07_cor:p1:F6`.

- `cli_installation_07_cor:p1:F6`: confidence as filed: definite; suggested bucket as filed: other.

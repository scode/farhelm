# Dispatch-overlap fixture has the same incomplete cleanup, armed later

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The dispatch-overlap fixture arms incomplete cleanup after fallible setup.

## Details

F187 — **definite** — `crates/farhelm/tests/e2e/session_lifecycle.rs:4628` — Dispatch-overlap fixture has the same
incomplete cleanup, armed later

Attachment and readiness checks occur before installing its PID guard, so failure there has no PID cleanup at all. Later
failures still inherit the guard's macOS limitations and incomplete ownership of the stubborn descendant. Arm portable
complete-tree cleanup before attachment and readiness assertions, so every exit path owns the external fixture it
created.

## Evidence and triage context

- crates/farhelm/tests/e2e/session_lifecycle.rs:4624 performs a fallible readiness wait before installing the guard at
  :4628.
- crates/farhelm/tests/e2e/harness.rs:2397 requires procfs identity and :2403 kills only the recorded process.
- crates/farhelm-fixtures/src/fake_agent.rs:357 creates the signal-ignoring fixture.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact coverage identified.

Caveats:

- Shares a helper-level mechanism with cli_installation_07_cor:p1:F6, but this caller and its ordering are separate edit
  sites.
- No runtime reproduction.
- Preserve this caller separately from F6 even if both use a shared replacement cleanup helper.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_07_cor:p1:F7`.

- `cli_installation_07_cor:p1:F7`: confidence as filed: definite; suggested bucket as filed: other.

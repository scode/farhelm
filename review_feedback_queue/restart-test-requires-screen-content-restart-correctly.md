# Restart test requires screen content that Restart correctly clears

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The restart test waits for screen content that correct restart erases.

## Details

F173 — **definite** — `crates/farhelm/tests/e2e/restart_with_resume.rs:1605` — Restart test requires screen content that
Restart correctly clears

The first-run marker remains on the visible screen rather than retained history. Restart correctly clears it, yet the
new attachment must show that marker before the second-run marker to satisfy the test. A quiet, correctly relaunched
fixture consequently times out. Assert the first marker before restarting, then independently require the complete
second marker from the new attachment.

## Evidence and triage context

- crates/farhelm/tests/e2e/restart_with_resume.rs:1539 creates an 80x24 env-echo fixture; :1595 attaches afresh and
  :1605 requires the previous first marker before second.
- crates/farhelm-fixtures/src/fake_agent.rs:820 prints the environment marker, then :1493 prints only a short
  banner/ready/prompt sequence.
- crates/farhelm-supervisor/src/tmux.rs:2889 respawns the retained pane.
- SPEC.md:764 explicitly accepts clearing the previous visible screen.
- crates/farhelm/tests/e2e/restart_with_resume.rs:1539 creates an 80-by-24 session; :1605 requires the first marker
  followed by the second in the new attachment.
- crates/farhelm-fixtures/src/fake_agent.rs:820 writes the first marker; :1487 emits only a few startup lines before
  waiting for input.
- crates/farhelm/tests/e2e/harness.rs:1809 requires both markers.
- SPEC_impl.md:1110 specifies respawn preserving history while reinitializing the visible grid.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- The restart specification confirms the test mismatch; it does not cover it as acceptable test behavior.
- No exact coverage identified. This is an executable false-failure oracle, not merely outdated commentary.

Caveats:

- No runtime reproduction.
- Incidental shell output could push the marker into history and hide the defect.
- SPEC.md:762-767 accepts clearing the product's visible screen; it does not accept a test that requires its
  preservation.
- No runtime reproduction; incidental shell output can hide the defect by pushing the marker into history.
- Inspection only; no runtime test.
- The shell must successfully source the fixture rc file. Unexpected additional startup output could incidentally scroll
  the marker, but the fixture does not establish that premise.
- Unexpected shell startup output could incidentally scroll the first marker into history.
- No runtime reproduction performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_07_sec:p1:F1`,
`cli_installation_07_cor:p1:F1`.

- `cli_installation_07_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `cli_installation_07_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.

# Partial shell-result parsing

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Prefix-only shell waits could parse incomplete directory or environment results.

## Details

F292 — **possible** — `crates/farhelm/tests/e2e/terminal_tabs.rs:376` — Partial shell-result parsing

The directory check waits for its opening marker and immediately parses the suffix; the separate environment check
likewise proceeds before requiring its closing delimiter. Split delivery can then produce false failures on correct
shell output. Timing was not reproduced and filter coverage remains unresolved. Require the complete framed result at
both editable waits before parsing or canonicalizing it.

## Evidence and triage context

- terminal_tabs.rs:370-390 waits only for CWD[ and immediately parses/canonicalizes the suffix; the separate site at
  :3609-3626 waits only for ENV[ and parses before requiring ]. run_in_shell at :158-170 adds no completeness barrier.
  FILTER.md, 'Rare, self-correcting glitches and imprecise diagnostics', does not explicitly settle test-failure
  coverage. Preserve both independently editable sites if recorded.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_09_cor:p1:C1`.

- `cli_installation_09_cor:p1:C1`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.

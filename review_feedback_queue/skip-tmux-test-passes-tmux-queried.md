# The “skip tmux” test also passes when tmux is queried

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The skip-tmux test also passes when tmux is unnecessarily queried.

## Details

F160 — **definite** — `crates/farhelm-supervisor/src/service/handlers.rs:8207` — The “skip tmux” test also passes when
tmux is queried

Its stimulus makes an unconditional tmux query return a successful empty map, allowing the expected terminal-less
listing even with the prohibited query. The oracle therefore misses the independence from tmux failures that it claims
to protect. Observe query invocation directly, or use a verified unclassified query failure that would propagate if
querying occurred.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/handlers.rs:8188–8218: the test kills the private tmux server and claims a pane
  query would return an error.
- crates/farhelm-supervisor/src/service/listing.rs:168–179: production currently skips pane_states for terminal-less
  entries.
- crates/farhelm-supervisor/src/tmux.rs:3651–3668: pane_states returns an empty successful map for definitive server
  absence.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- The production conditional remains present.
- No mutation test was run.
- Production currently contains the correct conditional.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_06:p1:F1`.

- `gap_supervisor_state_cor_06:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.

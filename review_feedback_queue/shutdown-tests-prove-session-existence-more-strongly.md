# Shutdown tests prove session existence more strongly than agent liveness

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Supervisor-shutdown tests could accept a dead agent in a retained pane.

## Details

F291 — **possible** — `crates/farhelm/tests/e2e/supervisor_stop.rs:271` — Shutdown tests prove session existence more
strongly than agent liveness

After shutdown, the tests check acknowledgements and tmux session existence. Because panes remain after agent exit,
killing the agent can leave every post-stop assertion satisfied despite the claimed continuation of running work. This
is a possible oracle gap, not observed product loss. Retain an agent identity and verify its liveness or new progress
after shutdown, independently of session existence.

## Evidence and triage context

- supervisor_stop.rs:3-5 and :54-62 describe sessions continuing to run and the survival assertion guarding against
  shutdown taking them down. The fixture proves readiness before stopping at :181-192, but after shutdown :225-276
  checks exit status, output-disable acknowledgments, and only tmux has-session.
  crates/farhelm-supervisor/src/tmux.rs:2040 enables remain-on-exit, so agent death need not remove that session.
  review_feedback_queue/shutdown-expiry.md:14-24 concerns timeout-driven unsafe client shutdown and possible server
  abort, not this dead-agent/retained-pane oracle; BUGS.md:34-42 excludes planned stops from its accepted abrupt-death
  residual.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_08_cor:p1:C6`.

- `cli_installation_08_cor:p1:C6`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.

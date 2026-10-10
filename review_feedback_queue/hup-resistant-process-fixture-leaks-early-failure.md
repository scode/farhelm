# The HUP-resistant process fixture leaks on early failure

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Early failure leaves HUP-resistant fixture processes behind.

## Details

F153 — **definite** — `crates/farhelm-supervisor/src/service/core.rs:16364` — The HUP-resistant process fixture leaks on
early failure

The fixture deliberately creates descendants that evade marker cleanup and survive tmux's hangup signal. A read or
assertion failure before manual cleanup unwinds without an owner that terminates them; shutting down the private tmux
server does not supply equivalent cleanup. Install an unwind-safe process owner before fallible setup, covering both
shell and child identities.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:16289–16294: the fixture disables usable scopes.
- crates/farhelm-supervisor/src/service/core.rs:16336–16350: its shell clears the environment, ignores HUP, and starts
  sleep 1000.
- crates/farhelm-supervisor/src/service/core.rs:16353–16380: fallible reads and assertions precede process cleanup.
- crates/farhelm-supervisor/src/service/core.rs:16391–16405: manual cleanup itself performs a fallible process read.
- crates/farhelm-supervisor/src/service/core.rs:14972–15000: StateDir destruction kills the tmux server.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- This is a fixture leak, not established loss of production work.
- The descendants are launched through tmux; cleanup of the test runner’s own process group does not establish ownership
  of them.
- The leaked shell/job are fixtures, not demonstrated surviving production work.
- The sleep has a finite lifetime.
- No runtime reproduction was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_02:p1:F3`.

- `gap_supervisor_state_cor_02:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.

# interrupted tab creation leaves live work outside Delete’s consent check

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Interrupted tab creation could leave live work outside Delete's warning check.

## Details

F84 — **possible** — `crates/farhelm-supervisor/src/service/core.rs:12225` — interrupted tab creation leaves live work
outside Delete’s consent check

A supervisor crash after creating a terminal window but before publishing its tab identity can leave a surviving shell
absent from tab listings. After the agent ends, Delete or Replace can classify the session as having nothing alive,
while whole-session teardown still kills that shell. The complete crash sequence was not reproduced. Recover incomplete
window creation before serving requests, or conservatively refuse no-live-work deletion when a live window remains
unclassified.

## Evidence and triage context

- core.rs:12225-12228 creates a live tab window before size operations and marker publication at :12302-12309.
  terminals.rs:247-249 excludes unmarked windows. core.rs:11672-11685 uses that discovery for NothingAlive consent;
  handlers.rs:1064-1087 proceeds to teardown. teardown.rs:426-435 sweeps the whole session and :615-618 removes its tmux
  session.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:834-846 requires refusal when an unconfirmed Delete/Replace would stop a live tab.
  TRIAGE_OUTCOMES.md:2197-2225 addresses stale liveness using marked-window discovery, not an interrupted unmarked
  creation. BUGS.md:8-51 concerns tmux aborting; this case requires tmux to survive. FILTER.md:86-94 explicitly excludes
  missing required consent and work loss.

Caveats:

- Requires a supervisor crash after window creation but before tab identity publication, survival of tmux and the shell,
  and later Delete or Replace after the agent ends.
- No runtime reproduction.
- The shell may be killed by the whole-session sweep before kill-session; this strengthens the consequence rather than
  requiring kill-session to be the first destructive step.
- No runtime reproduction. Requires abrupt supervisor death in the publication gap, surviving tmux/shell and a later
  NothingAlive deletion after the agent ends. Request-cancellation ownership does not prevent a supervisor-process
  crash.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ss_systems:p2:F1`.

- `ss_systems:p2:F1`: confidence as filed: possible; suggested bucket as filed: highest.

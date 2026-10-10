# Failed-relaunch rollback can forget a surviving process scope

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Failed restart recovery could forget a surviving replacement scope.

## Details

F27 — **possible** — `crates/farhelm-supervisor/src/service/core.rs:11289` — Failed-relaunch rollback can forget a
surviving process scope

A session previously launched without a systemd process scope can attempt a scoped restart, fail, and finish portable
cleanup without confirming that group is gone. Recovery restores the previous unscoped setting, losing the replacement
scope's cleanup obligation. Invisible descendants could then survive Stop or run alongside another Restart. Require
confirmed scope cleanup before definitive rollback; otherwise retain the replacement generation's scope selection. The
sequence was not reproduced, and a healthy Delete may still recover it.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:11247 permits cleanup after an absent tmux result; line 11289 passes
  Warn and line 11297 returns definitive failure.
- crates/farhelm-supervisor/src/service/sweep.rs:1409 returns success for an unconfirmed scope when the portable sweep
  succeeds under Warn.
- crates/farhelm-supervisor/src/service/core.rs:10584 restores prior.scoped for definitive failure; store.rs:4155
  persists that prior scope selection.
- crates/farhelm-supervisor/src/service/handlers.rs:942 and core.rs:10290 reap only the entry's recorded scope for later
  Stop and Restart.
- SPEC.md:870 requires confirmation from the scope even when the portable sweep finds nothing.
- core.rs:10870-10894 routes spawn failure through unwind_failed_relaunch. :11283-11297 uses ScopeKillFailure::Warn and
  returns definitive after a clean portable sweep. sweep.rs:1409-1419 allows that success despite an unconfirmed scope
  kill; :1058-1063 explains invisible detached descendants. core.rs:10584-10599 restores prior.scoped,
  store.rs:4163-4175 persists it without reverting generation, and core.rs:10290-10301 subsequently checks only the
  recorded scope.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- TODO.md:492 and TRIAGE_OUTCOMES.md:6119 explicitly cover failed creation dropping its session, not restart restoring a
  prior unscoped selection. The partial-operation filter is close, but its full application is not established: this
  changes subsequent cleanup authority and permits later false success, beyond merely recording the failed operation
  inaccurately.
- SPEC.md:850-855 forbids restarting alongside prior descendants. TRIAGE_OUTCOMES.md:3448-3467 covers cleanup before
  relaunch, not failed-new-launch unwind. :6119-6140 and TODO.md:492-499 cover failed creation, not this Restart
  rollback. No exact existing coverage.

Caveats:

- Requires an unscoped prior run, scoped replacement, failed relaunch, unconfirmed scope cleanup and a descendant
  invisible to the portable sweep.
- A later Delete with a usable manager can rediscover the scope; permanent unrecoverability is not established.
- Actual loss of user work was not demonstrated; the surviving-process and overlapping-launch implications were
  independently considered.
- Requires failed relaunch, prior unscoped selection, unconfirmed scope cleanup and a descendant invisible to the
  portable sweep.
- A later healthy Delete can rediscover generation scopes through teardown.rs:389; do not claim the processes are
  unrecoverable.
- No actual user-work loss or runtime reproduction.
- Independent location from cleanup_replacement_window.
- Requires an unscoped prior run, a scoped attempted restart, unconfirmed scope cleanup, and a survivor invisible to the
  portable sweep.
- No runtime reproduction was performed.
- Delete may still recover the scope through broader enumeration; concurrent-launch work damage was not reproduced.
- Control flow confirmed; surviving-process interleaving untested. Requires unscoped-to-scoped transition, unconfirmed
  scope cleanup and a descendant invisible to the portable sweep. Work corruption itself was not reproduced.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_sec_03:p1:F1`, `ss_lifecycle:p1:F1`.

- `gap_supervisor_state_sec_03:p1:F1`: confidence as filed: definite, conditional on the described failure sequence;
  suggested bucket as filed: other.
- `ss_lifecycle:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.

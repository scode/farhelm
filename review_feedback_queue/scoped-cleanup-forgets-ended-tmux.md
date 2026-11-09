# Scoped cleanup forgets jobs after their tmux server ends

Reviewed commit: 5f235d92f7184cdf949cf9d45e86dc19e71ff4df

## TLDR

After a session-started tmux server ends, its background jobs can survive a later successful Stop, Restart or Delete.
Cleanup looks for a currently running server and forgets the jobs it previously moved outside the session's group.

## Details

Suggested bucket: **high**. Confidence: **source-confirmed conditional mechanism**, not a runtime reproduction.
Surviving jobs and overlap on ordinary Restart establish this bucket; no security or user-work-loss route is claimed.

`crates/farhelm-supervisor/src/scope.rs:1150-1166` considers current members only; missing or zombie members do not
require fallback (`scope.rs:151-159`). `service/sweep.rs:1253-1260` retains containment selection but no historical tmux
evidence. Its fast return at `1522-1549` can therefore trust a still-live original scope after a nested server ended.

A complete sequence is a scoped agent starting stock systemd-linked tmux with `new-session -d`, then continuing
independent foreground work. The assigned regression uses this shape at
`crates/farhelm/tests/e2e/session_lifecycle.rs:5917-5920` and checks the separate scope and hangup-immunity premises at
`5974-6016`. A later normal tmux `kill-server` targets the server itself;
[upstream command](https://github.com/tmux/tmux/blob/3.4/cmd-kill-server.c) and
[server teardown](https://github.com/tmux/tmux/blob/3.4/server.c) do not terminate the independent foreground agent.
`SPEC_impl.md:5056-5057` at this revision explicitly establishes survival of hangup-immune pane jobs when the server
ends. Those jobs remain in sibling scopes while the original scope stays live. Later cleanup finds no server, ends the
agent, and skips the marker sweep that previously found those jobs. An already-unloaded original scope would force
fallback; this sequence deliberately preserves it until inspection.

`SPEC.md:1840-1859` and "Ownership during cleanup and provisioning", lines 4700-4710, preserve ordinary tmux cleanup.
This is distinct from a server born after inspection: no timing against the later cleanup is needed. Current-member
inspection cannot establish absence of historical migrated jobs; retain sufficient lifetime evidence or fallback.

There is a narrower caller variant at `service/core/host_restart.rs:1189-1216`: failed Restart unwind ends its new
terminal host before this inspection. Primary tmux 3.7c source establishes that a user server with `exit-unattached`
enabled can disappear on last-client exit before inspection. This variant also requires normal `exit-empty` behavior, no
blocking internal tmux job, successful pane migration, another surviving original-scope member and readable inherited
markers. Default detach does not establish server death. Whether the nondefault user-server option is covered by the
stock-implementation support boundary remains unresolved; keep that caveat. Its proven consequence is leftover
failed-attempt jobs with the original error visible and session retained. **Normal same-supervisor retry overlap is
refuted**: the abandoned generation's now-absent scope forces marker fallback before retry. That variant's narrower
consequence is other priority and must not inflate the main finding. The main sequence above needs neither that option
nor a failed Restart.

Possible coverage: no exact existing item, accepted limit, known bug or unlanded plan covers the main sequence; Planned
was empty. Successful-cleanup reporting excludes the rare-glitch filter. Private-server interference and deliberate
self-migration exclusions do not describe this ordinary user-server route.

Provenance: run `ee465fa5-0b29-4ce7-b1fe-8fabdb679cfe`, `cleanup-scope/f2`, `cleanup-sweep/l4`, and caller-variant
records `cleanup-callers/f5` / `cleanup-scope/f7`. Their append-only histories retain the corrected confidence and
withdrawn retry-overlap claim, primary-source references, and exact investigation limits.

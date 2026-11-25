# Supervisor probe SIGKILLs a possibly-reused process-group id after reaping its child

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

In a rare race, a failing supervisor probe could kill an unrelated process group on the user's own machine: the probe
reaps its child (freeing the pid that is also the group id), then unconditionally SIGKILLs the saved group id, which a
newly spawned process creating its own group could already have reused.

## Details

**other.** Source inspection; commissioner confirmed the kill-after-reap ordering. Bucketed by what is established: the
unsafe ordering. The harmful route (pid wraparound plus a new process creating its own group within the microsecond
reap→kill window) is NOT established; in practice the kill is a no-op (ESRCH). It would move to highest only if that
route were shown (killing user processes). Not dropped under the unestablished-route filter because the ordering defect
is concrete and inconsistent with the sibling helper that does it correctly; triage may judge it an accepted theoretical
race.

`crates/farhelm-helm/src/provisioning/backend.rs:2811-2825` (group id captured at 2604): in the `handshake_io_failure`
arm, `child.wait()` has reaped the probe child, freeing its pid; then `libc::kill(-(pid as i32), SIGKILL)` fires
unconditionally on the saved group id. The comment's safety claim — "The group id cannot have been reused while any such
member is alive, which is the only case the kill is for" — covers the helper-alive case (an ssh ProxyCommand holding the
stderr pipe) but not the no-helper case: with no member alive the id can be reused by a newly spawned process that
creates its own group (Farhelm's own `isolate_process_group` calls `process_group(0)`), and the kill would hit that
unrelated group. `terminate_child` (backend.rs:2435-2443) avoids this by killing BEFORE reap via `child.id()`.

Verify: reason about pid wraparound between reap and kill (or stress pid allocation on a busy machine while probes
fail); fix shape: skip the post-reap group kill unless a helper is known alive, kill before reaping as `terminate_child`
does, or verify group membership before signalling.

Reviewer metadata (area 5 chunk B, provisioning backend second half): suggested bucket: other; confidence: likely — race
traced, trigger not demonstrated; proposed_drop: none; possible_cover: none known. Commissioner validation: ordering
confirmed at the cited lines; coverage searched (queue — the pane-owner pid-reuse item is a different scope; TODO.md,
BUGS.md, TRIAGE_OUTCOMES.md — no pid-reuse decision; plans/queue) — no match.

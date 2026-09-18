# Duplicate freeze clobbers a retarget, losing the edit

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Editing a host's destination while it sits frozen as a duplicate can silently lose the edit — the host stays frozen on
the old destination until you edit it a second time.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible. Coordinator verified.

Both `Duplicate` re-publish sites — the loop-top frozen check after the `twin_holding` await (manager.rs:2840-2844:
`set_state(HostState::Duplicate...)`) and the post-attempt arm (manager.rs:2942-2944) — re-assert the freeze with NO
`taken_nudge` seam, then `hold()` consumes any pending nudge as a mere regime flag. A retarget landing during the
`twin_holding` await (or a dial completing just before the nudge is sent) gets clobbered: the actor overwrites the
manager's fresh `Connecting` with `Duplicate{old identity}`, and the next pass re-freezes on the old identity (loop top
reloads the row but the Duplicate state short-circuits before dialing) without ever dialing the new destination.

The edit is lost until a second edit/twin change; plain retry cannot heal it (re-nudge just re-freezes). Every other
post-attempt arm self-heals because only Duplicate re-asserts from published status at loop top — and `serve` already
implements exactly the missing seam ("publishing this connection's health now would overwrite the state the manager just
published for the edited row", manager.rs:3388-3394). Concrete trigger (retarget during a Duplicate hold), user-visible
effect (host stays frozen on the old identity despite the edit). Possible.

Suggested fix: check `taken_nudge()` before `set_state(Duplicate…)` in both arms and skip the publish when set (loop
reloads the new row) — the same seam `serve` uses at :3392.

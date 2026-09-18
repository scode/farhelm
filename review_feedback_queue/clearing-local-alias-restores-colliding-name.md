# Clearing the local alias can restore a colliding display name

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Clearing the local machine's custom name can silently break targeting it: if an ssh host happens to use the default
name, the local host — the default place new sessions run — stops accepting agent commands.

## Details

Source: pre-pr-review-swarm, area stores, 2026-09-17. Confidence: definite. Independent companion to
`ssh-destination-collides-with-local-display-name.md`, needed even with that fix: register the colliding name while the
local row is aliased (no collision then), then clear the local alias — only the clear path sees that transition.
Coordinator verified.

`update_alias`'s clear arm (store.rs:3785-3795) restores the row's derived name and runs it through the NARROW
`alias_collision` check only. Clearing the LOCAL row's alias restores `"this machine"`; an ssh destination spelled
exactly that lives in the `destination` column, not `alias`, so the check passes and the write commits → the same
incoherent state as the destination-collision finding (both rows unresolvable, local host silently not an agent target)
via a routine rename-clear. The clear comment shows the author considered this shape ("Skipping this on clear would let
a host silently reclaim its raw destination") but covered only reclaim-against-alias.

Suggested fix: in the clear arm, when the restored derived name could collide with a destination (only the address-less
local row can), refuse `AliasTaken` if any other ssh row's destination equals it; add a test (destination planted +
local alias set → clear refused while the collision stands).

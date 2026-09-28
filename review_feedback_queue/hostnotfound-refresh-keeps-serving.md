# An identity-less actor keeps serving after its registry row is deleted

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In the rare paths that delete a host without stopping its connection, a host whose supervisor reports no identity keeps
showing connected and keeps a connection to that machine open after it was supposedly removed.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Narrowed from the original finding (pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7`,
`F14 / COR-HOSTNOTFOUND-KEEPS-SERVING`). An actor re-reads its registry row only between connections. For a host with an
identity, a refresh whose cache write is refused with `HostNotFound` now ends the connection (`refresh_once` in
`manager.rs`), so the actor reloads its row, finds it gone, and retires.

What remains is the identity-less case. Such a host never writes the store during a refresh, so it never sees
`HostNotFound`: if its row is deleted without stopping its actor, it stays `Connected`, keeps its ssh transport, keeps
draining, and can still answer agent upcalls until the next successful reconcile drops the actor. A fix needs the
identity-less refresh branch to learn that the row is gone, for example by checking row existence on each refresh.

User-visible consequence: in the rare paths that delete a host's row without stopping its connection (the original
finding cites the remove case of its F12), an identity-less host keeps showing as connected.

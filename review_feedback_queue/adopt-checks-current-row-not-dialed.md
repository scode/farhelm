# Adopt checks the current row instead of the dialed configuration

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In a narrow retarget race, approving an adoption can record the previous machine's identity on the edited host and
discard its last-known session list.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F10 / COR-ADOPT-USES-CURRENT-ROW`, tagged **possible**. Anchors and title: `manager.rs:2351`, `manager.rs:2376` — Adopt
checks the manager's current row instead of the configuration the mismatch was observed under

Adopting a new identity (`ConnectionManager::adopt`, `manager.rs:2350`) calls `store.adopt_identity` with a `DialedAs`,
a snapshot of the connection-defining fields (destination, remote binary, remote state dir). The store refuses with
`StaleAttempt` if the row's current fields differ from that snapshot. The intent is that an identity reported by one
endpoint cannot be adopted onto a row that now points somewhere else.

`adopt` builds that snapshot from `handle.row` (read at `manager.rs:2351`, passed at `:2376`), which is the manager's
_current_ copy of the row. It does not use the configuration the mismatch was actually observed under, and cannot:
`HostState::IdentityMismatch` carries only `recorded` and `reported`.

Consider the known race in queue item `stale-dial-outcome-publishes-over-retarget-nudge`. A mismatch observed against
the old destination is published after a retarget has already updated `handle.row` to the new destination. The adopt
then compares new with new and passes. The CAS on the recorded identity also passes, because a retarget leaves the
stored identity alone (`store.rs:3864-3870`). The old machine's identity is written onto the retargeted entry, and the
entry's cache and history are purged in the same transaction. The existing queue item assumes this adopt "then 409s via
the store's DialedAs refusal"; it does not.

Suggested fix: carry the attempt's `DialedAs` inside `HostState::IdentityMismatch` and pass that to `adopt_identity`.

User-visible consequence: in a narrow retarget race, approving an adoption can record the previous machine's identity on
the edited host and discard its last-known session list.

Restater note: the finding's second trigger, "after a failed reconcile following `set_destination`", does not hold as
stated. `sync_registry` is what updates `handle.row`, and it can only fail at its initial `list_hosts` read, before it
touches anything. After a failed reconcile, `handle.row` still holds the _old_ configuration. That case produces the
opposite error: a genuine mismatch from the new destination, which the actor dials after `retry_now` reloads the row
from the database, is wrongly refused as `StaleAttempt` until the next successful reconcile. The suggested fix (carry
the dialed configuration in the mismatch state) addresses both directions. The retarget-race trigger is confirmed
against the code.

# Adopt unconditionally overwrites state after the commit await

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

In a rare coincidence, adopting a host's identity can leave the host stuck showing "connecting" with nothing behind it
until the next reconcile pass.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible, low. Coordinator verified (one of two
reported races; the other was rejected).

`adopt` reads state/row/status under the map lock (manager.rs:2281-2291), drops the lock, awaits `store.adopt_identity`,
then UNCONDITIONALLY overwrites status with `Connecting` — stealing and retiring whatever client is published — with no
state/handle revalidation (manager.rs:2331-2343). Two races were examined; only the second survives: (a) REJECTED — a
concurrent `retry_now` cannot get a fresh client published inside the commit→`send_modify` microsecond window (any dial
completing there used pre-commit config and fails the `DialedAs` check; any post-commit dial takes milliseconds, landing
long after the publish); (b) KEPT — an actor panic in the window lets the supervisor publish `Retired`, which adopt then
resurrects as `Connecting` with no actor behind it (stuck until the next reconcile if the follow-up revive fails).

The structural gap (blind overwrite after an await, no `claim_is_current`-style `ptr_eq` or still-`IdentityMismatch`
check) violates the codebase's own discipline for post-await publishes. Narrow coincidence + self-healing, hence low.
Possible, low.

Suggested fix: make the post-commit publish conditional inside the `send_modify` (only overwrite if state is still that
`IdentityMismatch` + same handle), or re-validate under the map lock.

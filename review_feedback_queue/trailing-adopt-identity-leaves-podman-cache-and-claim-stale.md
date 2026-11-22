# Adopting a new host identity leaves Podman cache and unused place claims stale

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

After wiping/reinstalling a container host (or recycling its address) and adopting its new identity in Hosts, Farhelm
can keep showing the old machine's Podman check as usable for up to five minutes, fail new container launches or recipe
builds with a misleading error saying the SSH destination may have changed outside Farhelm, and keep holding an unused
claim on the old machine that persistently blocks another host entry pointing to it — even its verified connected owner
— until the adopted host is refreshed.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/hosts.rs:1570`.

When a host entry's SSH destination now reaches a wiped/reinstalled or recycled machine, its actor enters
`HostState::IdentityMismatch`, which causes `PodmanPlaces::view` and `ready` to refuse new work via
`ordinary_identity_refusal` (`crates/farhelm-helm/src/environment/podman_probe.rs:524, 556, 876-889`). When the user
clicks Adopt (`POST /api/hosts/{id}/adopt` -> `hosts.rs:1570-1582` -> `ConnectionManager::adopt` at
`manager.rs:3174-3278` -> `HelmStore::adopt_identity` at `store.rs:8847-8927`), `adopt` updates `hosts.host_identity`,
clears `session_cache`, resets permissions, sets the actor state to `HostState::Connecting`, and bumps `events`, but
NEVER invalidates `PodmanPlaces`'s `cache.probes` or `cache.refreshing` for `host` (unlike `PodmanPlaces::retarget` and
`set_ssh_tailscale`), and `adopt_identity` NEVER releases an unused `podman_place_claims` row for `host`. Two concrete
failure modes follow:

1. **Stale cache & broken launches/builds for up to 5 minutes**: If `host` had a successful cached probe within
   `FRESH_FOR` (300s, `podman_probe.rs:26`) before the mismatch and no probe ran during the mismatch (or an in-flight
   pre-adopt probe finishes after adoption), `ordinary_identity_refusal` immediately becomes `None`,
   `PodmanPlaces::view(&row)` (`podman_probe.rs:521-549`) flips back to `usable: true` with the superseded machine's
   `architecture`/`version`/`checked_at`, and `PodmanPlaces::fresh(&store, host)` (`podman_probe.rs:675-715`) passes
   `confirm_cached_admission` without re-probing `M_new` (because `podman_place_claims` still holds
   `(host, M_old, uid)`). Launches and rebuilds then run `VERIFY_PLACE_AND_EXEC` expecting `M_old` against `M_new` and
   fail with `CONTAINER_PLACE_UNVERIFIED` ("Its SSH destination may have changed outside Farhelm. Restore the original
   destination, then try again.").
2. **Persistent cross-restart stale unused claim locks out another entry for `M_old`**: If an address was recycled from
   `M_old` to `M_new` and `M_old` is now registered under another host entry `B`, adopting `M_new` on `host` moves
   `host` from `IdentityMismatch` to `Connected` with a valid, non-refused `host_identity` while leaving
   `(host, M_old, uid)` in `podman_place_claims`. When `B` probes `M_old`, `claim_podman_place` (`store/podman.rs:432`)
   evaluates `supervisor.is_some() && !refused` as true and refuses `B` as a twin of `host` — even if `B` is the
   connected, verified ordinary owner of `M_old`, and even across helm restarts — until `host` is refreshed. Fix
   direction: Inside `HelmStore::adopt_identity` (after verifying adoption succeeds), delete
   `podman_place_claims WHERE host_id = ?1` when `podman::referenced_place_reason(&tx, host)?.is_none()` (retaining the
   claim if containers/images/helpers still reference `host`), and serialize with `PodmanPlaces`
   (`invalidate_route(host)` under `claim_publication`) so `view()` requires a fresh check, pre-adopt in-flight probes
   cannot publish, and `fresh()` re-probes the newly adopted machine.

Reviewer metadata (g35-podman-identity-probe/l5): suggested bucket: high (cannot be in highest: `VERIFY_PLACE_AND_EXEC`
in `run_verified` rejects container commands when the new machine's `/etc/machine-id` does not match the cached claim,
and `inspect`/`exists`/`pull_base` run before creating unrecoverable records, so no user data or credentials are lost;
however, it materially degrades a common workflow — wiping/reinstalling a host or recycling an address and clicking
Adopt — by immediately marking the new machine usable with the old machine's capabilities, failing every container
launch/build for up to 5 minutes with a misleading error, and leaving any unused claim on the old machine persistently
locked by the adopted host across restarts).; confidence: confirmed; possible cover: none (cross-group corroboration:
g36/f5 and g37/f8).

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`, `crates/farhelm-helm/src/store.rs:8847-8927`
(g36-podman-identity-store-build/f5):

Distinct from and extending `g35/l5` (which noted that `adopt_host` / `manager.adopt` does not invalidate
`PodmanPlaces`'s in-memory cache for up to 5 minutes), `HelmStore::adopt_identity`
(`crates/farhelm-helm/src/store.rs:8847-8927`) updates `hosts.host_identity = new` and purges `session_cache` and
history partitions, treating adoption as a full install-incarnation boundary, but never releases an unused row in
`podman_place_claims`. Route (source-verified):

1. Host A (`alpha.example`) was connected to machine M1 (supervisor `S1`), claimed `(A, M1, 1000)` in
   `podman_place_claims`, and has no containers or recipe artifacts (`referenced_place_reason(A) == None`).
2. Outside Farhelm, `alpha.example` now points to machine M2 (supervisor `S2`). Host A enters
   `HostState::IdentityMismatch { recorded: S1, reported: S2 }`.
3. User clicks Adopt on Host A (`hosts.rs:1570` -> `manager.rs:3174` -> `store.rs:8847`). `adopt_identity` commits
   `hosts.host_identity = S2` while leaving `(A, M1, 1000)` in `podman_place_claims`. Host A reconnects to M2 and enters
   `HostState::Connected { identity: Some(S2) }`.
4. Host B (`beta.example`, which actually reaches M1 and is `Connected { identity: Some(S1) }`) probes M1 (even after
   `FRESH_FOR` expires or after a full helm restart) and calls `settle_claim` ->
   `claim_podman_place(B, (M1, 1000), Some(S1), None)` (`store/podman.rs:381-452`).
5. `claim_podman_place` finds `twin` `owner = A` with `supervisor = Some(S2)`. Because `supervisor.is_some()` and
   `refused_incumbent` is `None`, call 1 returns `PlaceClaim::Twin { owner: A }` (`store/podman.rs:432-437`).
6. `PodmanPlaces::settle_claim` (`podman_probe.rs:609-618`) inspects `self.ordinary_state(&A)`, sees
   `HostState::Connected { identity: Some(S2) }` (NOT `Duplicate`, `IdentityMismatch`, or `IdentityUnverified`), sets
   `verified = None`, and returns `PlaceClaim::Twin { owner: A }`. Result: Host B — the verified ordinary owner of M1 —
   is durably locked out of claiming M1 across restarts until the user explicitly presses Refresh on Host A (or
   removes/retargets Host A). (Note: when Host A DOES still have containers or recipe artifacts at M1,
   `podman_place_claims` must be preserved so cleanup/status remain anchored to M1; only an unused claim where
   `referenced_place_reason(&tx, host)?.is_none()` should be released.) Fix direction: inside
   `HelmStore::adopt_identity`'s immediate transaction (`store.rs:8861-8923`), if
   `podman::referenced_place_reason(&tx, host)?.is_none()`, execute
   `DELETE FROM podman_place_claims WHERE host_id = ?1`, and invalidate `PodmanPlaces` for `host` in `adopt_host` /
   `manager.adopt` (coordinating with `g35/l5`). Add a store test covering adoption releasing an unused claim while
   retaining a referenced claim.

Reviewer metadata (g36-podman-identity-store-build/f5): suggested bucket: high (could NOT be in highest: admission
lockout rather than security or data loss; materially degrades UX because the lockout persists across helm restarts
until the user knows to refresh an unrelated host); confidence: confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`, `crates/farhelm-helm/src/store.rs:8892`
(g37-podman-retarget-probe-store/f8):

`HelmStore::adopt_identity` (`crates/farhelm-helm/src/store.rs:8847-8927`) purges `session_cache` and history tables for
the superseded installation, but does not delete an unused `podman_place_claims` row for `host`. Likewise
`hosts::adopt_host` (`crates/farhelm-helm/src/hosts.rs:1570-1582`) does not invalidate `PodmanPlaces` (`cache.probes` /
`cache.refreshing`) for `host`. This produces two distinct failures:

1. Immediate 5-minute launch/build failure on the adopted host (`g35/l5`): within `FRESH_FOR` (300s) of a prior probe on
   the old machine, the adopted host reconnects under the new identity (clearing `ordinary_refusal`),
   `PodmanPlaces::fresh` -> `confirm_cached_admission` (`crates/farhelm-helm/src/environment/podman_probe.rs:701-715`)
   succeeds using the predecessor's cached `ReadyPlace` and still-present `podman_place_claims` row, and
   `VERIFY_PLACE_AND_EXEC` (`podman_probe.rs:42-51, 155-190`) then exits 125 against the new machine, failing with
   `CONTAINER_PLACE_UNVERIFIED` ("Its SSH destination may have changed outside Farhelm. Restore the original
   destination, then try again.").
2. Persistent cross-restart Twin lockout of the vacated machine (`g36/f5`): because `host` now has `host_identity = new`
   and is `Connected` (no ordinary identity refusal), its stale unused `podman_place_claims` row for the old machine
   survives helm restarts and satisfies `supervisor.is_some() && !refused` in `HelmStore::claim_podman_place`
   (`crates/farhelm-helm/src/store/podman.rs:427-438`). A separate host entry that is the verified ordinary owner of the
   vacated machine cannot displace the unused claim and remains locked out as `PlaceClaim::Twin` until `host` is probed
   again or retargeted.

Fix: On adoption, delete `podman_place_claims WHERE host_id = ?1` when `referenced_place_reason(&tx, host)?.is_none()`
(keeping the claim if existing containers/recipe artifacts still reference it) and invalidate `PodmanPlaces`'s cached
probe/refresh permit for `host` under `claim_publication` (mirroring `PodmanPlaces::retarget`).

Reviewer metadata (g37-podman-retarget-probe-store/f8): suggested bucket: high (not highest: refuses new container
launches/builds on a freshly adopted host for up to 5 minutes and persistently locks out the verified ordinary owner of
the vacated machine across helm restarts until the adopted host is re-probed or retargeted; no data loss or credential
exposure); confidence: confirmed; possible cover: none (corroborates g35/l5 and g36/f5).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g35-podman-identity-probe/l5`,
`g36-podman-identity-store-build/f5`, `g37-podman-retarget-probe-store/f8`.

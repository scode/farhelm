# claim_podman_place retains an unused host's stale claim after a Twin refusal

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

If an unused SSH host's destination changes outside Farhelm so it now reaches a machine already claimed by another host,
Refresh rightly refuses it as a duplicate, but it silently keeps its old machine claim on disk. Another host entry
pointing to that old machine can then be wrongly refused as a duplicate of the moved host.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/store/podman.rs:406-443`.

In `HelmStore::claim_podman_place` (`crates/farhelm-helm/src/store/podman.rs:406-443`), when a host entry `host` already
has a row `previous` in `podman_place_claims` for `(M1, U1)` and a probe now observes a different `(M2, U2)`:

1. Lines 407-411 check `previous != identity && referenced_place_reason(&tx, host)?.is_some()`. When `host` has no
   containers, recipe images, or helpers, this is false and execution continues.
2. Lines 412-438 look up any existing claimant `owner` for `(M2, U2)`. If `owner` already claims `(M2, U2)` and cannot
   be displaced, line 437 returns `Ok(Some(PlaceClaim::Twin { owner, name }))` immediately and drops the transaction
   without committing.
3. Because `(host, M1, U1)` was never deleted from `podman_place_claims`, `host` continues to own `(M1, U1)` durably
   (even across restarts) despite having zero resources there and no longer reaching `(M1, U1)`. When a third host entry
   `other` pointing to `(M1, U1)` is probed (especially a Podman-only host without a verified ordinary supervisor, where
   `owns_supervisor` is false, or when `host` has a recorded `host_identity` without an active refusal),
   `claim_podman_place` for `other` sees `(host, M1, U1)` in `podman_place_claims` and refuses `other` as
   `PlaceClaim::Twin { owner: host, ... }`. Fix direction: when `previous.is_some_and(|p| p != identity)` and
   `referenced_place_reason(&tx, host)?.is_none()`, delete `host`'s stale `podman_place_claims` row and commit before
   returning `Ok(Some(PlaceClaim::Twin { owner, name }))` (or release it before the twin check and commit on the Twin
   branch). Add a store test covering an unused host moving onto a twin's endpoint and a third host subsequently
   claiming the vacated endpoint.

Reviewer metadata (g36-podman-identity-store-build/l3): suggested bucket: other (could NOT be in highest: causes
false-positive Twin admission refusal on another host entry rather than security or data loss); confidence: confirmed;
possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g36-podman-identity-store-build/l3`.

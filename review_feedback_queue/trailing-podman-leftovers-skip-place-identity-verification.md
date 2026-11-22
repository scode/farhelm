# Leftover container listing and deletion skip place-identity verification on claimed hosts

Reviewed commit: 86be6c582492cddb7413e5a176d84bb29eb76da1

## TLDR

If a container host's SSH destination changes outside Farhelm to reach a different machine or account, deleting an
unregistered leftover container in Settings -> Containers can report success without deleting the container on the
original host, and refreshing the leftover list can silently report no leftovers instead of flagging the changed
destination.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman.rs:832-865`.

Exhaustive audit of every Podman command call site in `crates/farhelm-helm/src/environment/podman.rs` and
`podman/build.rs` confirms that `Podman::delete_leftover` (`podman.rs:832-865`, lines 834 and 864) and
`Podman::list_owned` (`podman.rs:1585-1628`, lines 1591 and 1610) are the ONLY callers using
`impl From<HostId> for PlaceTarget<'_>` (`podman.rs:140-147`, which hardcodes `identity: None`). Every other Podman
engine command uses `PlaceTarget::from(binding)`, `PlaceTarget::claimed(place, identity)`, or `recipe_place_identity`
(`build.rs:904-910`). When a container host has a durable place claim (already probed, or locked by existing
containers/recipes) and its SSH alias moves externally to another machine/account with an empty Podman store: (1)
`delete_leftover` runs `podman container exists -- <id>` without `VERIFY_PLACE_AND_EXEC`, observes exit 1 (absent),
returns `Ok(())` at line 835, and `DELETE /api/container-orphans` replies 200 OK and bumps `FleetEvents` while the
leftover container remains untouched on the original host; (2) `list_owned` runs `podman ps` unverified, receives `[]`,
and `GET /api/container-orphans` hides all leftovers on that host (including interrupted session containers with no
cached sessions yet that `leftovers()` includes via `binding.place_host` at lines 889-891) instead of returning an
error. In addition, the fake command model in `crates/farhelm-helm/src/environment/podman/tests.rs:220-227` was not
updated to return `[]` for `ps` when `model.place_identity.is_some()`, and no test exercises `delete_leftover` or
`leftovers` under `MOVED_ENDPOINT`. Verification/fix: resolve the optional durable claim via `recipe_place_identity` (or
a shared `place_target` helper on `Podman` calling `self.place(host)` then `self.0.store.podman_place_identity(host)`)
in both `delete_leftover` and `list_owned`, and remove `impl From<HostId> for PlaceTarget<'_>` so future callers cannot
accidentally bypass verification. Unclaimed/twin hosts still resolve to `identity: None` (preserving `tests.rs:1081` and
`tests.rs:1629`), while claimed hosts fail closed with `CONTAINER_PLACE_UNVERIFIED`. Add moved-place tests for
`delete_leftover` and `leftovers` in `tests.rs`.

Reviewer metadata (g38-podman-retarget-build-tests/l2): suggested bucket: other (cannot be in highest: does not lose
user data, credentials, or user-owned work; row-backed session containers remain protected by binding.place_identity,
while unregistered leftovers are left running/allocated on the original host and hidden or falsely reported deleted);
confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g38-podman-retarget-build-tests/l2`.

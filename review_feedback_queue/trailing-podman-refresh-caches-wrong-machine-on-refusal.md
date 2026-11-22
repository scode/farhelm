# PodmanPlaces::refresh caches the wrong machine's ReadyPlace on admission refusals

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

If a container host's SSH alias temporarily points to another Linux machine with Podman and Farhelm checks it, restoring
the SSH alias back to the original machine can still leave existing containers unable to check status, reconnect, or
delete (when Podman lives at a different path on the two hosts) and can send the other machine's proxy settings into a
recovering container, until the user manually presses Refresh or restarts Farhelm.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman_probe.rs:823`.

Bucket note: placed in `high`; reviewer surfaced that injecting the moved-to host's cached proxy credentials into the
original host's container during recovery could cross into `highest`.

In `PodmanPlaces::refresh` (`crates/farhelm-helm/src/environment/podman_probe.rs:823-867`), ALL four admission-refusal
branches still overwrite `cache.probes[host]` with
`CachedProbe { route, result: Ok(wrong_machine_ready), admission_refusal: Some(...), .. }`:

1. `ordinary_refusal = Some(_)` -> `CLAIM_NEEDS_REFRESH` (line 824): the most common case when a Farhelm-managed SSH
   host moves outside Farhelm to another Farhelm-managed host (`IdentityMismatch`/`Duplicate`); `settle_claim` is
   skipped entirely yet the other host's `ReadyPlace` is cached.
2. `PlaceClaim::Changed` (line 828): Farhelm proved the route reaches a different `(machine_id, uid)` than the host's
   durable claim while resources still reference it, yet caches the wrong machine's `ReadyPlace`.
3. `PlaceClaim::Twin` (line 827): if a host without an established claim temporarily points to a twin host, the twin's
   `ReadyPlace` is cached.
4. `identity: Err(IDENTITY_UNREADABLE | IDENTITY_TOOLS_MISSING)` (line 835): if the moved-to host runs Podman but lacks
   `sha256sum`/`tr` or readable `/etc/machine-id`, its unverified `ReadyPlace` is cached.

`PodmanPlaces::runtime` (`podman_probe.rs:643-654`) then serves ANY cached `Ok(ready)` whose `probe.route == route`
indefinitely, without checking `admission_refusal`, `ready.identity`, or freshness.

Consequences once the user follows `CONTAINER_PLACE_UNVERIFIED` ("Restore the original destination, then try again")
without manually clicking Refresh:

1. Broken container operations and Delete Retry: `BaseRoute` text is unchanged because the SSH alias moved outside
   Farhelm. `runtime()` keeps returning the other machine's `ReadyPlace`, including its absolute `podman` path
   (`podman.rs:249-256, 387-401, 417-420`). If the two hosts install `podman` at different paths (e.g.,
   `/usr/bin/podman` vs `/usr/local/bin/podman` or `/run/current-system/sw/bin/podman`), `VERIFY_PLACE_AND_EXEC`
   verifies the restored host and then `exec`s the wrong path (exit 127), failing `status`, `destroy`/Delete Retry,
   `suspend`/`resume`, and `PodmanExec` indefinitely until manual Refresh or helm restart.
2. Cross-host proxy leak on unadmitted container recovery: if a container on the original host already recorded
   `container_id` but not yet `remote_farhelm`/`remote_state_dir`, retrying `Podman::create`
   (`crates/farhelm-helm/src/environment/podman.rs:1302-1305, 1428-1454`) skips `places.fresh()` and calls
   `self.script(...)` (`podman.rs:441-442, 679, 709`), which reads `ready.proxy` from `runtime()` and pipes the other
   machine's cached proxy settings into the original host's container.

Fix direction: Do not overwrite an existing matched `ReadyPlace` with a probe whose identity conflicts with the durable
claim or fails verification (`PlaceClaim::Changed`, `ordinary_refusal`, `identity: Err`), and/or have `runtime()`
re-probe when `admission_refusal.is_some()` or when the cached `ready.identity` does not match the caller's requested
`PlaceTarget.identity` / durable claim.

Reviewer metadata (g37-podman-retarget-probe-store/f2): suggested bucket: high (could be highest: ReadyPlace.proxy can
carry proxy credentials per podman_probe.rs:3, and the established route below injects the moved-to host's cached
proxies into the original host's container during recovery; surface whether cross-host proxy transfer between two user
SSH destinations crosses highest); confidence: confirmed; possible cover: none (corroborated by g38/l1).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g37-podman-retarget-probe-store/f2`.

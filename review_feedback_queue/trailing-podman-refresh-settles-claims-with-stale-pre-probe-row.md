# PodmanPlaces::refresh settles claims with a stale pre-probe host row

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

If a host's Farhelm setup or update updates its remote install paths while a container host check is in flight, the
check can ignore an active installation identity refusal (duplicate, mismatch, or unverified) and claim the container
host slot for the refused entry — blocking another valid entry from using that machine — or fail to transfer an unused
claim to a newly connected host.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman_probe.rs:808`.

In `crates/farhelm-helm/src/environment/podman_probe.rs:808-858`, `PodmanPlaces::refresh` reads
`row = current_row(&store, host).await` before running the multi-command remote `probe()` (line 812). After `probe()`
finishes and `claim_publication` is acquired (line 813), lines 817-848 pass that pre-probe `row` to
`service.ordinary_state(&row)`, `service.settle_claim(&store, &row, ...)`, and
`host_display_name(row.kind, row.destination.as_deref(), row.alias.as_deref())`, and only re-read
`current_row(&store, host)` afterwards at line 851. `ordinary_state(&row)` calls
`ConnectionManager::state_for_registry_row(&row)` (`crates/farhelm-helm/src/manager.rs:2457-2470`), which returns `None`
if `reconfigured(&handle.row, row)` is true. `reconfigured()` (`crates/farhelm-helm/src/manager.rs:1655-1660`) treats
`remote_farhelm` and `remote_state_dir` changes as reconfigurations, whereas provisioning/discovery writes to those
columns do not invalidate `PodmanPlaces`'s probe permit. Consequently, if Setup/Update updates
`remote_farhelm`/`remote_state_dir` and `sync_registry()` replaces the actor while `probe()` runs:

1. If the new actor hits `HostState::Duplicate`, `IdentityMismatch`, or `IdentityUnverified`, `ordinary_state(&row)`
   returns `None`, so `ordinary_refusal` is `None`. `settle_claim` then calls `store.claim_podman_place`
   (`crates/farhelm-helm/src/store/podman.rs:394-449`), whose `WHERE` clause only fences `id`, `entry_uuid`,
   `destination`, and `tailscale` (putting `remote_farhelm`/`remote_state_dir` only in the `SELECT` expression for
   `owns_supervisor`). The refused entry therefore claims the `(machine_id, uid)` row in `podman_place_claims` and
   caches `admission_refusal: None` (even though `SPEC.md:5289-5290` and
   `ordinary_identity_mismatch_refuses_container_admission` require a refused actor not to claim or block the place).
2. Conversely, if the new actor connects cleanly (`HostState::Connected { identity: Some(..) }`), `ordinary_state(&row)`
   returns `None`, leaving `verified_supervisor = None` and `owns_supervisor = false`, so the connected owner fails to
   take an unused claim from a twin.
3. If `alias` was edited during `probe()`, the displaced twin's cached refusal message at lines 843-848 displays the
   stale name. Fix direction: Re-read `current = current_row(&store, host).await` under `claim_publication` immediately
   after `refreshing.is_current` (line 816), verify
   `current.entry_uuid == row.entry_uuid && base_route(&current).ok().as_ref() == Some(&route)`, and use `&current` for
   `ordinary_state`, `settle_claim`, `host_display_name`, and cache publication.

Reviewer metadata (g35-podman-identity-probe/l1): suggested bucket: other (cannot be in highest: subsequent
view/ready/fresh reads re-read the current row and still refuse new work on the refused host itself, so this does not
cause data loss, credential exposure, or unauthorized execution; it misassigns the Podman place claim and can block
another host entry or miss a twin takeover).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g35-podman-identity-probe/l1`.

# Retargeting an SSH host bypasses ssh_route_lock, skips retiring Tailscale masters, and races key cleanup

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

Editing a Tailscale SSH host's destination while a host check, setup/update/uninstall plan, SSH sign-in, container host
probe, or reconnect is running can race with those operations—deleting the host's pinned keys out from under an active
SSH command, leaving stale private key files or old shared Tailscale SSH connections behind, or letting a planning check
probe the new destination while building a plan against the old host details.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/hosts.rs:917-965`.

In `crates/farhelm-helm/src/hosts.rs:917-965`, `retarget` (handling `POST /api/hosts/{id}/destination`) acquires
`host_provision_lock(host)` and `host_write_lock(host)`, updates the destination, calls
`state.manager.sync_registry().await`, and then calls `state.provisioning.forget_tailscale_keys(&old).await` at
line 964. Unlike `set_tailscale` (`crates/farhelm-helm/src/hosts.rs:993-1037`) and `delete_registered_host`
(`crates/farhelm-helm/src/provisioning/service.rs:2582-2597`), `retarget` does not acquire
`state.manager.ssh_route_lock(host)` exclusively, does not park the host actor via `hold_connection`, and does not
retire old Tailscale ControlMasters via `release_shared_connections(&old)` before committing the destination edit and
deleting `old`'s private pin directory (`remove_dir_all` on `<state>/tailscale/<sha256(old_dest)>` via
`crates/farhelm-helm/src/ssh/tailscale.rs:404-410`). Three lock-free readers rely exclusively on
`ssh_route_lock(host).read_owned()` (none hold `host_provision_lock` or `host_write_lock` during their SSH commands) to
keep a host's SSH route and pin directory stable:

1. `ProvisioningService::lock_ssh_plan` (`crates/farhelm-helm/src/provisioning/service.rs:3718-3734`, called by
   `probe_host` at line 980, `plan_update_owned` at line 1667, and `plan_uninstall_owned` at line 2013).
2. `SetupSignIn::start` (`crates/farhelm-helm/src/provisioning/setup_sign_in.rs:147-164`).
3. `PodmanPlaces::refresh` (`crates/farhelm-helm/src/environment/podman_probe.rs:796-812`). Consequences:

- Concurrent pin deletion / orphaned pin recreation: While any of those three readers—or the host's own `HostActor` dial
  (which `sync_registry()` only nudges asynchronously without waiting for it to park)—is preparing or launching `ssh`
  for `old.destination`, `retarget` deletes `<state>/tailscale/<sha256(old_dest)>`. If deletion lands before `ssh` opens
  `UserKnownHostsFile`, OpenSSH fails spuriously on a missing known-hosts file; if `TailscaleSshAccess::prepare`'s
  `spawn_blocking(write_known_hosts)` (`crates/farhelm-helm/src/ssh/tailscale.rs:99-102`) runs after
  `forget_tailscale_keys(&old)`, it recreates private pin files for a destination the registry no longer owns.
- Unretired old Tailscale ControlMasters (aligns with `g43` note): For a Tailscale host (`old.tailscale == true`),
  `retarget` relies only on `sync_registry()` rather than parking the actor and calling
  `release_shared_connections(&old)`, leaving the old destination's Tailscale ControlMaster(s) under
  `/tmp/fhs-<scope(old)>` alive for `ControlPersist=60` even after its key directory was removed.
- Split destination during `plan_update_owned`: `lock_ssh_plan` (`service.rs:3725-3733`) checks
  `DialedAs::of(row) == DialedAs::of(&current)` under `ssh_route_lock.read_owned()` specifically to keep a registered
  plan on one side of a route edit, after which `plan_update_owned` calls `probe_target_for_row(&row)`
  (`service.rs:1687`, which re-reads the store via `resolve_host_access`). Because `retarget` omits `ssh_route_lock`, a
  concurrent destination edit can land between lines 1667 and 1687 so `plan_update_owned` probes the new destination
  while keeping `effective_row = row.clone()` (`service.rs:1698`) from the old one.
- Skipped cleanup on reconcile error: If `sync_registry()` fails (`hosts.rs:941-962`), `retarget` returns early at line
  957 and never calls `forget_tailscale_keys(&old)` even though the new destination was already committed. Fix
  direction: In `retarget`, acquire `ssh_route_lock(host).write_owned()` (ordered after `host_provision_lock` and before
  `host_write_lock`, matching `set_tailscale` and `delete_registered_host`), park the actor with `hold_connection` and
  call `release_shared_connections(&old)` when `old.tailscale` is set (and the destination actually changes), commit the
  destination, run `forget_tailscale_keys(&old)` whenever the commit succeeded (even if `sync_registry()` subsequently
  fails), and release the hold after reconciliation.

Reviewer metadata (g44-tailscale-hosts-transport/l2): suggested bucket: other (could NOT be in highest: deleting a
Tailscale known-hosts pin file under an in-flight ssh fails closed because TailscaleSshAccess::options sets
StrictHostKeyChecking=yes and GlobalKnownHostsFile=/dev/null at crates/farhelm-helm/src/ssh/tailscale.rs:121-122, and
provisioning plan confirmation re-checks the target destination before mutating a host).; confidence: confirmed.;
possible cover: none..

Additional source record at `74fa1951bac92f5a78757100534498c458d86806`,
`crates/farhelm-helm/src/provisioning/service.rs:2614-2634` (g45-tailscale-provisioning-podman/f3):

In this commit, `ProvisioningService::lock_ssh_plan` (`crates/farhelm-helm/src/provisioning/service.rs:3718-3734`),
`SetupSignIn::start` (`crates/farhelm-helm/src/provisioning/setup_sign_in.rs:147-164`), and `PodmanPlaces::refresh`
(`crates/farhelm-helm/src/environment/podman_probe.rs:796-813`) all acquire
`manager.ssh_route_lock(host).read_owned().await` specifically to serialize with SSH route edits (their comments
explicitly say "Keep a registered plan on the same side of a route edit as its network work" and "Wait for a route
edit"). Both `hosts::set_tailscale` (`crates/farhelm-helm/src/hosts.rs:993-1030`) and
`ProvisioningService::delete_registered_host` (`crates/farhelm-helm/src/provisioning/service.rs:2582-2597`) take
`ssh_route_lock(host)` exclusively before mutating the row and calling `ProvisioningService::forget_tailscale_keys`
(`service.rs:2616-2634`). However, `hosts::retarget` (`crates/farhelm-helm/src/hosts.rs:917-977`, cross-group interface
with g44) acquires `host_provision_lock` and `host_write_lock` but never acquires `ssh_route_lock(host)` before
rewriting the destination and calling `state.provisioning.forget_tailscale_keys(&old).await` (`hosts.rs:964`).
Consequently, if a Tailscale host is retargeted from `old` to `new` while `probe_owned`, `plan_update_owned`,
`plan_uninstall_owned`, `SetupSignIn::start`, or `PodmanPlaces::refresh` holds `ssh_route_lock(host).read_owned()`: (1)
`forget_tailscale_keys(&old)` runs `remove_dir_all(<state>/tailscale/<sha256(old)>)` while the concurrent operation's
SSH process still needs its `UserKnownHostsFile`, or before `TailscaleSshAccess::prepare` recreates the pin directory
after cleanup; and (2) if `probe_owned` (for an existing row, `new_entry: None`) discovers a running supervisor on `old`
after `retarget` moved the row to `new`, `register_discovered` -> `store.register_probed_ssh_host_with_mode(old, ...)`
(`service.rs:1357`, `store.rs:7700-7760`) finds no row with `destination = old` and inserts a brand-new host row for
`old` (the exact race `delete_registered_host` cites at `service.rs:2580-2581`). Fix direction: synchronize
`hosts::retarget` with `ssh_route_lock(host)` (taking care not to introduce long stalls behind interactive sign-in
prompts, per `f1`) and release old Tailscale shared connections if needed.

Reviewer metadata (g45-tailscale-provisioning-podman/f3): suggested bucket: other (not highest: causes transient SSH
failures, orphaned Tailscale key pins, or a re-inserted host row on concurrent probe+retarget, without data loss or
security boundary bypass); confidence: confirmed; possible cover: none (note: cross-group interface with
g44-tailscale-hosts-transport in `crates/farhelm-helm/src/hosts.rs:917-977`).

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`, `crates/farhelm-helm/src/hosts.rs:963`
(g35-podman-identity-probe/c1):

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da Suggested bucket: not applicable (not reported; could not be
in highest). Confidence: confirmed Possible cover: none Proposed drop: explicitly accepted as best-effort by
`SPEC_impl.md:6155` ("Turning the mode off or forgetting the host clears its private key snapshots on a best-effort
basis"), and introduced by commit 74fa1951b (#4638, outside group g35's commit e114a47f4). Specifically,
`hosts.rs:928-965` does not acquire `ssh_route_lock(host)` around `retarget` + `forget_tailscale_keys(&old)`, so an
abandoned `PodmanPlaces::refresh` probe still running on `old` could recreate a Tailscale known-hosts snapshot after
`forget_tailscale_keys` deletes it; because snapshot cleanup is explicitly best-effort and does not weaken host-key
verification on active routes, this is not a bug.

Reviewer metadata (g35-podman-identity-probe/c1): suggested bucket: not applicable (not reported; could not be in
highest).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g44-tailscale-hosts-transport/l2`,
`g45-tailscale-provisioning-podman/f3`, `g35-podman-identity-probe/c1`.

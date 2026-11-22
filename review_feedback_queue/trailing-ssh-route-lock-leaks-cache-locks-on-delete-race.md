# ConnectionManager::ssh_route_lock leaks cache_locks entries when lock-free callers race host deletion

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

Removing an SSH host while a background probe, planning check, or SSH sign-in is starting for that host can leave a
small orphaned entry in the helm's in-memory host lock table for the lifetime of the process.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/manager.rs:2802-2810`.

`ConnectionManager::ssh_route_lock` (`crates/farhelm-helm/src/manager.rs:2802-2810`) documents "Call only with the ID of
a registry row, including before its first actor is installed", and unconditionally inserts a new `HostLocks` entry via
`self.cache_locks.lock()...entry(host).or_default().ssh_route.clone()`. However, three callers invoke
`ssh_route_lock(host)` without holding `host_provision_lock(host)` (so they can race `delete_registered_host` at
`crates/farhelm-helm/src/provisioning/service.rs:2573-2601`):

- `SetupSignIn::start` (`crates/farhelm-helm/src/provisioning/setup_sign_in.rs:146-148`)
- `ProvisioningService::lock_ssh_plan` (`crates/farhelm-helm/src/provisioning/service.rs:3725-3726`)
- `PodmanPlaces::refresh` (`crates/farhelm-helm/src/environment/podman_probe.rs:791-804`, which also invokes
  `manager.ssh_route_lock(host)` before matching `row.kind == HostKind::Ssh`) If `delete_registered_host` deletes the
  host and runs `self.manager.forget_cache_lock(host)` (`service.rs:2599`) in the window between the caller's initial DB
  lookup and its `ssh_route_lock(host)` call, `ssh_route_lock` re-inserts a fresh `HostLocks` entry for that removed
  `HostId`. Since `HostId` is never reused and `forget_cache_lock(host)` will never be called again for that id, the
  entry remains in `self.cache_locks` until the helm exits. Fix direction: Store `ssh_route` locks as
  `Weak<tokio::sync::RwLock<()>>` (matching `SignIn::gates` in `crates/farhelm-helm/src/ssh_sign_in.rs:92, 105-115` and
  `ProvisionLockGuard` in `manager.rs`) or prune entries whose strong count drops to 1 when the caller's post-lock
  `host_row` lookup observes `HostNotFound`.

Reviewer metadata (g44-tailscale-hosts-transport/l6): suggested bucket: other (could NOT be in highest: bounded
per-raced-deletion in-memory map entry leak with no security or data-loss impact).; confidence: confirmed.; possible
cover: none..

Additional source record at `74fa1951bac92f5a78757100534498c458d86806`,
`crates/farhelm-helm/src/provisioning/service.rs:2599` (g45-tailscale-provisioning-podman/f9):

`ProvisioningService::delete_registered_host` (`crates/farhelm-helm/src/provisioning/service.rs:2588-2599`) deletes the
host row and calls `self.manager.forget_cache_lock(host)` (`manager.rs:2827-2832`) to remove `host` from
`ConnectionManager::cache_locks`. However, `ConnectionManager::ssh_route_lock(host)` (`manager.rs:2802-2810`)
unconditionally calls `self.cache_locks.lock().entry(host).or_default()`. Three `g45` call sites read the host row from
SQLite and then call `manager.ssh_route_lock(host)` without holding any lock across that gap: (1) `SetupSignIn::start`
(`crates/farhelm-helm/src/provisioning/setup_sign_in.rs:146-148`), (2) `ProvisioningService::lock_ssh_plan`
(`crates/farhelm-helm/src/provisioning/service.rs:3725-3726`, called from `probe_owned`, `plan_update_owned`, and
`plan_uninstall_owned`), and (3) `PodmanPlaces::refresh`
(`crates/farhelm-helm/src/environment/podman_probe.rs:791-808`). If `delete_registered_host` finishes
`forget_cache_lock(host)` between the initial row read and the `ssh_route_lock(host)` call, `ssh_route_lock(host)`
recreates the `cache_locks` map entry, the subsequent re-read fails with `HostNotFound`, and nothing ever removes that
map entry again. Fix direction: either store weak references in `cache_locks` (like `SignIn::gates` in
`ssh_sign_in.rs:92,105-115` or `provision_locks`), or have `ssh_route_lock` only look up existing entries / remove newly
inserted entries if the post-lock `host_row` lookup returns `HostNotFound` while no other holder exists.

Reviewer metadata (g45-tailscale-provisioning-podman/f9): suggested bucket: other (not highest: small per-raced-deletion
memory leak in `ConnectionManager::cache_locks`, no security or data-loss impact); confidence: confirmed; possible
cover: none (corroborates cross-group lead `g44/l6` from `g44-tailscale-hosts-transport` on the `g45` caller side).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g44-tailscale-hosts-transport/l6`,
`g45-tailscale-provisioning-podman/f9`.

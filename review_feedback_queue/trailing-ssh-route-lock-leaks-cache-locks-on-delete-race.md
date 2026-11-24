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

Additional source record at `1b3fe49af86979aebc2719965682a17f8c6366ac`, `crates/farhelm-helm/src/manager.rs:2814`
(g32_remove_host_signin_lock_helm/f2):

Follow-up observation at `1b3fe49af86979aebc2719965682a17f8c6366ac`: commit `1ec7724e3cd0e8353c75af6f9b65842795518e9d`
removed `ssh_route_lock(host).write_owned().await` from `delete_registered_host`, widening the concurrency window in
which lock-free callers can re-insert a leaked `cache_locks` entry after `forget_cache_lock`.

Commit `1ec7724e3cd0e8353c75af6f9b65842795518e9d` removed `self.manager.ssh_route_lock(host).write_owned().await` from
`ProvisioningService::delete_registered_host` (`crates/farhelm-helm/src/provisioning/service.rs:2635-2660` at
`1b3fe49af86979aebc2719965682a17f8c6366ac`) so host removal does not wait on in-flight route readers.
`delete_registered_host` still calls `self.manager.forget_cache_lock(host)` (line 2657) to remove the deleted host's
entry from `ConnectionManager::cache_locks` (`crates/farhelm-helm/src/manager.rs:2839-2844`). However, every reader of
`ssh_route_lock` reads the host row first and only later calls `manager.ssh_route_lock(host)`:
`ProvisioningService::lock_ssh_plan` (`service.rs:3795`, called from `probe_owned`, `plan_update_owned`, and
`plan_uninstall_owned`), `SetupSignIn::start` (`crates/farhelm-helm/src/provisioning/setup_sign_in.rs:318`), and
`PodmanPlaces::refresh` (`crates/farhelm-helm/src/environment/podman_probe.rs:800`). Because
`ConnectionManager::ssh_route_lock` (`manager.rs:2814-2822`) unconditionally calls
`self.cache_locks.lock().entry(host).or_default()`, if `delete_registered_host` removes the row and runs
`forget_cache_lock(host)` between the reader's initial row lookup and its `ssh_route_lock(host)` call,
`ssh_route_lock(host)` re-inserts a fresh `HostLocks` entry for the already-deleted `HostId`. The reader immediately
re-checks `host_row(host)` / `current_row`, sees `HostNotFound`, and returns, leaving the re-inserted `cache_locks`
entry behind permanently (since `hosts.id` is `INTEGER PRIMARY KEY AUTOINCREMENT`, that `HostId` is never reused and
`forget_cache_lock` is never called for it again). This also violates `ssh_route_lock`'s own doc contract
(`manager.rs:2813`: "Call only with the ID of a registry row") and breaks `host_write_lock(deleted_id)` returning
`None`.

How to verify: Trace `probe_owned` (`service.rs:908` -> `1006` -> `3795`), `plan_update_owned` (`service.rs:1711` ->
`1712` -> `3795`), or `podman_probe.rs:791` -> `800` interleaved with `delete_registered_host` (`service.rs:2643` ->
`2657`) and `manager.rs:2814-2844`.

Fix: Either make `ssh_route_lock` / `cache_locks` store `Weak<HostLocks>` (or a separate `Weak<RwLock<()>>` map pruned
like `SignIn::gates` in `ssh_sign_in.rs:100-105`), or return `Option<Arc<RwLock<()>>>` from `ssh_route_lock` when no
live entry exists (ensuring an actor or lock entry is created only under registration/reconciliation) and remove any
freshly inserted entry if the subsequent `host_row` check returns `HostNotFound`.

Reviewer metadata (g32_remove_host_signin_lock_helm/f2): suggested bucket: other (Minor bounded-per-raced-removal
in-memory map leak with no security, data-loss, or user-visible UX impact.); confidence: confirmed (established directly
from source analysis of `manager.rs`, `service.rs`, `setup_sign_in.rs`, and `podman_probe.rs`.); possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g32_remove_host_signin_lock_helm/f2`.
